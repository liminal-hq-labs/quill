// QUILL32.EXE: Quill for Windows 95, 98 and ME.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The editor in a Win32 window. This first Win32 build draws the same text-mode screen as the DOS build, cell by cell, in the
//! system's OEM fixed font; the GDI page with real fonts and the native menus come later (see docs/progress.md).
//!
//! `QUILL32.EXE [FILE] [/MONO]` opens a file. `/PLAY KEYS.TXT /DUMP OUT.TXT` plays a key script (the tokens of
//! `tests/scenarios`) and writes every screen it shows as golden frames, the way the DOS build does; `/HOLD` keeps the window
//! open afterwards for a screenshot.
#![no_std]
#![no_main]

extern crate alloc;

/// Times the rest of the block (the `bench` feature; nothing at all in the product).
#[cfg(feature = "bench")]
macro_rules! span {
	($p:ident) => {
		let _span = $crate::bench::span($crate::bench::P::$p);
	};
}
#[cfg(not(feature = "bench"))]
macro_rules! span {
	($p:ident) => {};
}

#[cfg(feature = "bench")]
mod bench;
mod chrome;
mod dialogs;
mod gdi;
mod pdf;
mod rt;
mod scheme;
mod sys;
mod tips;
mod ui;
mod view;

use alloc::string::String;
use alloc::vec::Vec;
use core::ffi::c_void;
use core::fmt::Write;
use core::mem::MaybeUninit;
use core::ptr::{null, null_mut};

use gdi::GdiMeasure;
use quill_app::recovery::Why;
use quill_app::{App, Command, Dir, Files, Key, SaveMode, Scheme};
use quill_formats::io::{Sink, Source};
use quill_grid::Grid;
use quill_heap::HeapStore;
use sys::*;

const CLASS: &[u8] = b"Quill32\0";

/// The client size Quill asks for when the screen has room: the page at 96 dots per inch with a little desk either side, and about
/// 600 lines of it.
const PREFERRED_W: i32 = 860;
const PREFERRED_H: i32 = 600;

/// The part of the screen that is not taskbar, or the whole screen when the system will not say.
unsafe fn work_area() -> Rect {
	let mut r = Rect {
		left: 0,
		top: 0,
		right: 0,
		bottom: 0,
	};
	if SystemParametersInfoA(SPI_GETWORKAREA, 0, core::ptr::addr_of_mut!(r).cast(), 0) == 0
		|| r.right <= r.left
		|| r.bottom <= r.top
	{
		r = Rect {
			left: 0,
			top: 0,
			right: GetSystemMetrics(SM_CXSCREEN).max(640),
			bottom: GetSystemMetrics(SM_CYSCREEN).max(480),
		};
	}
	r
}

/// The 16 VGA colours as Windows COLORREFs (0x00BBGGRR).
const VGA: [u32; 16] = [
	0x000000, 0xAA0000, 0x00AA00, 0xAAAA00, 0x0000AA, 0xAA00AA, 0x0055AA, 0xAAAAAA, 0x555555, 0xFF5555, 0x55FF55, 0xFFFF55,
	0x5555FF, 0xFF55FF, 0x55FFFF, 0xFFFFFF,
];

struct Win32Source(Handle);

impl Source for Win32Source {
	fn read(&mut self, out: &mut [u8]) -> usize {
		let mut got = 0u32;
		// SAFETY: `out` is a valid buffer of the length given; the handle came from CreateFileA.
		let ok = unsafe { ReadFile(self.0, out.as_mut_ptr(), out.len() as u32, &mut got, null_mut()) };
		if ok == 0 {
			0
		} else {
			got as usize
		}
	}
}

impl Drop for Win32Source {
	fn drop(&mut self) {
		// SAFETY: the handle is ours.
		unsafe { CloseHandle(self.0) };
	}
}

struct Win32Sink(Handle);

impl Sink for Win32Sink {
	fn write(&mut self, data: &[u8]) -> bool {
		let mut put = 0u32;
		// SAFETY: `data` is a valid buffer; the handle came from CreateFileA.
		let ok = unsafe { WriteFile(self.0, data.as_ptr(), data.len() as u32, &mut put, null_mut()) };
		ok != 0 && put as usize == data.len()
	}
}

struct Win32Files;

fn c_name(name: &[u8]) -> Vec<u8> {
	let mut v = name.to_vec();
	v.push(0);
	v
}

/// Opens a NUL-terminated path for reading.
fn open_path(n: &[u8]) -> Option<Win32Source> {
	// SAFETY: `n` is NUL-terminated.
	let h = unsafe {
		CreateFileA(
			n.as_ptr(),
			GENERIC_READ,
			FILE_SHARE_READ,
			null(),
			OPEN_EXISTING,
			0,
			null_mut(),
		)
	};
	(h != INVALID_HANDLE_VALUE).then_some(Win32Source(h))
}

impl Files for Win32Files {
	type R = Win32Source;
	type W = Win32Sink;

	fn open(&mut self, name: &[u8]) -> Option<Win32Source> {
		if let Some(s) = open_path(&c_name(name)) {
			return Some(s);
		}
		// A data file named without a folder (the dictionaries, the user dictionary) is also looked for beside the program.
		if !name.iter().any(|b| *b == b'\\' || *b == b':') {
			let mut path = [0u8; 260];
			// SAFETY: `path` holds 260 bytes, as the call is told.
			let n = unsafe { GetModuleFileNameA(null_mut(), path.as_mut_ptr(), 260) } as usize;
			let dir = path
				.get(..n)
				.and_then(|p| p.iter().rposition(|b| *b == b'\\'))
				.map_or(0, |i| i + 1);
			let mut full = path.get(..dir).unwrap_or(&[]).to_vec();
			full.extend_from_slice(name);
			full.push(0);
			return open_path(&full);
		}
		None
	}

	fn create(&mut self, name: &[u8]) -> Option<Win32Sink> {
		let n = c_name(name);
		// SAFETY: `n` is NUL-terminated.
		let h = unsafe { CreateFileA(n.as_ptr(), GENERIC_WRITE, 0, null(), CREATE_ALWAYS, 0, null_mut()) };
		(h != INVALID_HANDLE_VALUE).then_some(Win32Sink(h))
	}

	fn close(&mut self, w: Win32Sink) -> bool {
		// SAFETY: the handle is ours. The buffers go to the disk before the handle is closed, and either step can say the disk is full.
		unsafe {
			let flushed = FlushFileBuffers(w.0) != 0;
			CloseHandle(w.0) != 0 && flushed
		}
	}

	fn save_mode(&self) -> SaveMode {
		SaveMode::TempRename
	}

	fn remove(&mut self, name: &[u8]) -> bool {
		// SAFETY: `c_name` ends in a NUL.
		unsafe { DeleteFileA(c_name(name).as_ptr()) != 0 }
	}

	/// Windows NT and later replace the new name in one step (`MoveFileExA`, looked up at run time because Windows 95, 98 and Me
	/// only stub it); the older systems refuse a name that is taken, and the provided `replace` removes the old file and tries again.
	fn rename(&mut self, from: &[u8], to: &[u8]) -> bool {
		let (a, b) = (c_name(from), c_name(to));
		// SAFETY: both names end in a NUL; the function found has the signature `MoveFileExA` documents.
		unsafe {
			let kernel = GetModuleHandleA(b"kernel32.dll\0".as_ptr());
			if !kernel.is_null() {
				let p = GetProcAddress(kernel, b"MoveFileExA\0".as_ptr());
				if !p.is_null() {
					let ex: unsafe extern "system" fn(*const u8, *const u8, u32) -> i32 = core::mem::transmute(p);
					// MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH
					if ex(a.as_ptr(), b.as_ptr(), 9) != 0 {
						return true;
					}
				}
			}
			MoveFileA(a.as_ptr(), b.as_ptr()) != 0
		}
	}

	fn len(&mut self, name: &[u8]) -> Option<u32> {
		let n = c_name(name);
		// SAFETY: `n` is NUL-terminated.
		let h = unsafe {
			CreateFileA(
				n.as_ptr(),
				GENERIC_READ,
				FILE_SHARE_READ,
				null(),
				OPEN_EXISTING,
				0,
				null_mut(),
			)
		};
		if h == INVALID_HANDLE_VALUE {
			return None;
		}
		// SAFETY: the handle is ours.
		let size = unsafe {
			let size = GetFileSize(h, null_mut());
			CloseHandle(h);
			size
		};
		(size != u32::MAX).then_some(size)
	}

	/// Where a recovery copy goes when it has no place beside its document: `RECOVER\` beside the program, the program's own folder,
	/// then the temporary folder.
	fn recovery_dir(&mut self, n: u8, out: &mut [u8]) -> usize {
		let mut path = [0u8; 300];
		if n == 2 {
			// SAFETY: the buffer is 300 bytes, and the call is told 280.
			return unsafe { dialogs::temp_dir(&mut path[..280]) }
				.filter(|t| *t <= out.len())
				.map_or(0, |t| {
					out[..t].copy_from_slice(&path[..t]);
					t
				});
		}
		// SAFETY: `path` holds 300 bytes and the call is told 260.
		let len = unsafe { GetModuleFileNameA(null_mut(), path.as_mut_ptr(), 260) } as usize;
		let dir = path
			.get(..len)
			.and_then(|p| p.iter().rposition(|b| *b == b'\\'))
			.map_or(0, |i| i + 1);
		let extra: &[u8] = if n == 0 { b"RECOVER\\" } else { b"" };
		if n > 2 || dir == 0 || dir + extra.len() > out.len() {
			return 0;
		}
		out[..dir].copy_from_slice(&path[..dir]);
		out[dir..dir + extra.len()].copy_from_slice(extra);
		dir + extra.len()
	}
}

struct Shell {
	app: alloc::boxed::Box<App<GdiMeasure>>,
	/// The golden-frame mode: the text grid in the OEM font instead of the GDI page.
	grid: bool,
	/// The commands behind the menu items, by menu id.
	cmds: Vec<Command>,
	store: HeapStore,
	screen_dc: Handle,
	pics: gdi::PicCache,
	/// The print preview, when it is showing.
	preview: Option<gdi::PreviewView>,
	combos: Option<chrome::Combos>,
	/// The TrueType faces on the display, for the font box and the Fonts dialog.
	faces: Vec<Vec<u8>>,
	instance: Handle,
	/// The kind of tab stop a click on the ruler makes: 0 left, 1 centre, 2 right, 3 decimal.
	ruler_kind: u8,
	drag: Option<chrome::Drag>,
	files: Win32Files,
	cw: i32,
	ch: i32,
	dump: Option<(Handle, u32)>,
	hold: bool,
	/// How far the page is scrolled sideways, in pixels; zero unless the window is narrower than the page.
	hscroll: i32,
	/// How far the page is scrolled down: the page unit at the top of the view, counted from the first sheet's top edge.
	vpos: i32,
	/// The sheets of paper the body takes, as the last scroll update found them.
	pages: u32,
	/// The vertical scroll the body view had before a header or footer was opened, for when it comes back.
	saved_vpos: i32,
	/// The story the last command left the view on, to notice a change.
	last_view: u8,
	/// The last click, for double and triple clicks, and the selection a drag is growing.
	click: Option<ui::Click>,
	sel: Option<ui::SelDrag>,
	/// A picture handle held down: the drag that sizes the selected picture.
	pic_drag: Option<quill_app::picture_geom::HandleDrag>,
	/// What the pointer is over on the toolbars, and whether the status line is showing its hint.
	hover: ui::Hover,
	hint_on: bool,
	/// The toolbar button the mouse button went down on.
	pressed: Option<usize>,
	/// The tooltip control, the preview's push buttons, and the registered message a Windows 95 wheel mouse sends.
	tips: Option<tips::Tips>,
	pv_btns: [Handle; 6],
	wheel_msg: u32,
	/// The status bar control, its height (0 when hidden), the text now in each pane, and how many pages the preview shows.
	sb: Handle,
	status_h: i32,
	sb_text: [Vec<u8>; 9],
	/// The status bar's own window procedure, which the shell's replaces to hear a click on the zoom box (0 until it does).
	sb_proc: isize,
	pv_pages: u32,
	/// The scroll bars, which are controls placed in the page area, and whether each shows.
	vbar: Handle,
	hbar: Handle,
	vbar_on: bool,
	hbar_on: bool,
	/// Whether a rule closes the recent files in the File menu.
	recent_rule: bool,
	/// The off-screen picture every paint draws into before one copy to the window.
	back: Option<Back>,
	/// What is on the screen has changed since Check Spelling as You Type was told, so the idle timer asks again before it checks.
	spell_view: bool,
	/// The positions whose marks changed since the page was last painted for them, and how many batches ago that was.
	spell_chg: (u32, u32),
	spell_since: u8,
	/// Where each scroll bar (vertical, horizontal) was last put and what it was last told to show, so a key that changes neither makes
	/// no call.
	bar_placed: [Option<ui::Placed>; 2],
	bar_info: [Option<(i32, u32, i32)>; 2],
	/// The caret's blink phase and period.
	blink: ui::Blink,
	/// The title the window has now.
	title: Vec<u8>,
	/// A shutdown block reason is set on the window (Windows Vista and later), because the document has unsaved work.
	blocked: bool,
}

/// An off-screen bitmap, and the memory device context that holds it, the size of the window's client area.
struct Back {
	dc: Handle,
	bmp: Handle,
	old: Handle,
	w: i32,
	h: i32,
}

impl Back {
	/// Makes a bitmap `w` by `h` compatible with the screen, or None when the system has no memory for it.
	unsafe fn new(screen: Handle, w: i32, h: i32) -> Option<Back> {
		let dc = CreateCompatibleDC(screen);
		if dc.is_null() {
			return None;
		}
		let bmp = CreateCompatibleBitmap(screen, w.max(1), h.max(1));
		if bmp.is_null() {
			DeleteDC(dc);
			return None;
		}
		let old = SelectObject(dc, bmp);
		Some(Back { dc, bmp, old, w, h })
	}

	unsafe fn free(self) {
		SelectObject(self.dc, self.old);
		DeleteObject(self.bmp);
		DeleteDC(self.dc);
	}
}

static mut SHELL: *mut Shell = null_mut();

fn shell() -> Option<&'static mut Shell> {
	// SAFETY: single-threaded; set once in `start` before any window message.
	unsafe { SHELL.as_mut() }
}

fn read_whole(name: &[u8]) -> Vec<u8> {
	let mut f = Win32Files;
	let mut out = Vec::new();
	if let Some(mut r) = f.open(name) {
		let mut buf = [0u8; 512];
		loop {
			let n = r.read(&mut buf);
			if n == 0 {
				break;
			}
			out.extend_from_slice(buf.get(..n).unwrap_or(&[]));
		}
	}
	out
}

fn map_vk(vk: usize, shift: bool, ctrl: bool) -> Option<Key> {
	let dir = |d: Dir, plain: Key| if shift { Key::Select(d) } else { plain };
	Some(match vk {
		0x25 => dir(Dir::Left, Key::Left),
		0x27 => dir(Dir::Right, Key::Right),
		0x26 => dir(Dir::Up, Key::Up),
		0x28 => dir(Dir::Down, Key::Down),
		0x24 if ctrl => Key::CtrlHome,
		0x23 if ctrl => Key::CtrlEnd,
		0x24 => dir(Dir::Home, Key::Home),
		0x23 => dir(Dir::End, Key::End),
		0x21 => dir(Dir::PageUp, Key::PageUp),
		0x22 => dir(Dir::PageDown, Key::PageDown),
		0x2E => Key::Delete,
		0x2D => Key::Insert,
		0x70..=0x7B => Key::F((vk - 0x6F) as u8),
		_ => return None,
	})
}

/// Draws rows `from..to` of the grid as text cells with their attributes, at pixel (`left`, `top`).
unsafe fn draw_rows(dc: Handle, g: &Grid<25>, from: usize, to: usize, sh: &Shell, left: i32, top: i32) {
	for (n, y) in (from..to).enumerate() {
		let mut x = 0usize;
		while x < 80 {
			let a = g.get(x, y).map_or(0x07, |c| c.attr);
			let mut run = [0u8; 80];
			let mut k = 0;
			while x + k < 80 {
				let Some(c) = g.get(x + k, y) else {
					break;
				};
				if c.attr != a {
					break;
				}
				if let Some(slot) = run.get_mut(k) {
					*slot = if c.ch == 0 { b' ' } else { c.ch };
				}
				k += 1;
			}
			let fg = VGA.get(usize::from(a & 15)).copied().unwrap_or(0xAAAAAA);
			let bg = VGA.get(usize::from(a >> 4)).copied().unwrap_or(0);
			SetTextColor(dc, fg);
			SetBkColor(dc, bg);
			SetTextAlign(dc, 0);
			SetBkMode(dc, OPAQUE);
			let rect = Rect {
				left: left + x as i32 * sh.cw,
				top: top + n as i32 * sh.ch,
				right: left + (x + k) as i32 * sh.cw,
				bottom: top + (n as i32 + 1) * sh.ch,
			};
			ExtTextOutA(dc, rect.left, rect.top, ETO_OPAQUE, &rect, run.as_ptr(), k as u32, null());
			x += k;
		}
	}
}

/// The low and high 16 bits of a message's `lParam`, as signed pixel positions.
fn xy(lparam: isize) -> (i32, i32) {
	((lparam & 0xFFFF) as i16 as i32, ((lparam >> 16) & 0xFFFF) as i16 as i32)
}

unsafe extern "system" fn wnd_proc(hwnd: Handle, msg: u32, wparam: usize, lparam: isize) -> isize {
	match msg {
		WM_PAINT => {
			paint(hwnd);
			0
		}
		WM_KEYDOWN => {
			let shift = GetKeyState(VK_SHIFT) < 0;
			let ctrl = GetKeyState(VK_CONTROL) < 0;
			let alt = GetKeyState(VK_MENU) < 0;
			// The shortcuts come from the key itself: with Ctrl held the character message for I is the one for Tab, for H the one for
			// Backspace, and a program that waits for it inserts a tab where Italic was meant.
			// Ctrl+Shift+P writes the PDF; plain Ctrl+P is Print.
			if ctrl && shift && !alt && wparam == 0x50 {
				if let Some(sh) = shell() {
					if !sh.grid {
						pdf::print_to_pdf(hwnd, sh);
					}
				}
				return 0;
			}
			if let Some(a) = quill_app::accel::from_vk(wparam as u16, ctrl, shift, alt) {
				match a {
					quill_app::accel::Accel::Key(k) => press(hwnd, k),
					quill_app::accel::Accel::Command(c) => run_command(hwnd, c),
				}
				return 0;
			}
			if let Some(k) = map_vk(wparam, shift, ctrl) {
				press(hwnd, k);
			}
			0
		}
		// Alt+Backspace is Undo: the system sends it as a system key.
		0x0104 => {
			if let Some(quill_app::accel::Accel::Command(c)) =
				quill_app::accel::from_vk(wparam as u16, GetKeyState(VK_CONTROL) < 0, GetKeyState(VK_SHIFT) < 0, true)
			{
				run_command(hwnd, c);
				return 0;
			}
			DefWindowProcA(hwnd, msg, wparam, lparam)
		}
		WM_CHAR => {
			let c = wparam as u8;
			// What Ctrl turns a letter into is not text: the shortcut was handled when the key went down.
			let ctrl = GetKeyState(VK_CONTROL) < 0 && GetKeyState(VK_MENU) >= 0;
			if ctrl && quill_app::accel::is_ctrl_character(c) {
				return 0;
			}
			let k = match c {
				0x0D => Some(Key::Enter),
				0x08 => Some(Key::Backspace),
				0x09 if GetKeyState(VK_SHIFT) < 0 => Some(Key::BackTab),
				0x09 => Some(Key::Tab),
				0x1B => Some(Key::Esc),
				1..=26 => Some(Key::Ctrl(c + 0x60)),
				0x7F => None,
				0x20..=0xFF => Some(Key::Char(c)),
				_ => None,
			};
			if let Some(k) = k {
				press(hwnd, k);
			}
			0
		}
		WM_TIMER => {
			match wparam {
				ui::TIMER_HINT => {
					if let Some(sh) = shell() {
						ui::poll_hover(hwnd, sh);
					}
				}
				ui::TIMER_SPELL => {
					if let Some(sh) = shell() {
						ui::spell_tick(hwnd, sh);
					}
				}
				ui::TIMER_BLINK => {
					if let Some(sh) = shell() {
						ui::blink_tick(hwnd, sh);
					}
				}
				ui::TIMER_DRAG => {
					if let Some(sh) = shell() {
						if let Some(d) = sh.sel {
							ui::page_drag(hwnd, sh, d.last.0, d.last.1);
						}
					}
				}
				_ => play(hwnd),
			}
			0
		}
		// Every pixel of the client area is painted from the off-screen picture, so the system never erases it first.
		WM_ERASEBKGND => 1,
		// The user chose another colour scheme: read it again and repaint the chrome, the page's desk and the combo boxes.
		WM_SYSCOLORCHANGE => {
			apply_scheme();
			if let Some(sh) = shell() {
				if let Some(cb) = &sh.combos {
					for h in [cb.style, cb.face, cb.size] {
						InvalidateRect(h, null(), 1);
					}
				}
			}
			InvalidateRect(hwnd, null(), 0);
			0
		}
		// The caret shows, and blinks, only while the window has the keyboard focus.
		WM_SETFOCUS | WM_KILLFOCUS => {
			if let Some(sh) = shell() {
				ui::blink_focus(hwnd, sh, msg == WM_SETFOCUS);
			}
			0
		}
		// Control Panel changed the caret's blink time (or anything else): read it again.
		WM_SETTINGCHANGE => {
			if let Some(sh) = shell() {
				ui::blink_restart(hwnd, sh);
			}
			0
		}
		WM_SIZE => {
			if wparam != SIZE_MINIMIZED {
				if let Some(sh) = shell() {
					ui::relayout(hwnd, sh);
				}
			}
			0
		}
		WM_HSCROLL => {
			if let Some(sh) = shell() {
				ui::hscroll_by(hwnd, sh, wparam & 0xFFFF, ((wparam >> 16) & 0xFFFF) as u16 as i32);
			}
			// A scroll bar control takes the keyboard when it is clicked; typing belongs to the page.
			if wparam & 0xFFFF == 8 {
				SetFocus(hwnd);
			}
			0
		}
		0x0115 => {
			if let Some(sh) = shell() {
				ui::vscroll_by(hwnd, sh, wparam & 0xFFFF, ((wparam >> 16) & 0xFFFF) as u16 as i32);
			}
			if wparam & 0xFFFF == 8 {
				SetFocus(hwnd);
			}
			0
		}
		// WM_MOUSEWHEEL, on Windows 98, Me and NT: the distance is the high word of `wParam`.
		0x020A => {
			if let Some(sh) = shell() {
				let delta = ((wparam >> 16) & 0xFFFF) as u16 as i16 as i32;
				// MK_CONTROL (8) in the low word: Ctrl and the wheel zoom about the pointer, which this message gives in screen pixels.
				if wparam & 8 != 0 && !sh.grid && sh.preview.is_none() {
					let (x, y) = (lparam as i16 as i32, (lparam >> 16) as i16 as i32);
					ui::wheel_zoom(hwnd, sh, delta, x, y);
				} else {
					ui::wheel(hwnd, sh, delta);
				}
			}
			0
		}
		// WM_DRAWITEM: the status bar asks for the zoom buttons' insides.
		0x002B => {
			let item = &*(lparam as *const DrawItem);
			if let Some(sh) = shell() {
				if item.window == sh.sb {
					ui::draw_zoom_control(item);
					return 1;
				}
			}
			0
		}
		WM_COMMAND => {
			if lparam != 0 {
				combo_changed(hwnd, wparam);
			} else {
				menu_command(hwnd, wparam & 0xFFFF);
			}
			0
		}
		// Notifications to this window are in ANSI: the tooltip control asks, and answers with ANSI text (`NFR_ANSI`) when told so.
		0x0055 => 1,
		// The tooltip control asks for the text of the button it is about to explain.
		0x004E => {
			if let Some(sh) = shell() {
				if ui::notify(sh, lparam) {
					return 0;
				}
			}
			DefWindowProcA(hwnd, msg, wparam, lparam)
		}
		// The hint for the menu item that is highlighted, on the status line; none when the menu closes.
		0x011F => {
			if let Some(sh) = shell() {
				let (id, flags) = (wparam & 0xFFFF, (wparam >> 16) & 0xFFFF);
				let text = if flags == 0xFFFF && lparam == 0 || flags & 0x10 != 0 {
					""
				} else {
					ui::menu_hint(sh, id)
				};
				sh.app.set_message(text);
				sh.hint_on = !text.is_empty();
				ui::status_update(hwnd, sh);
			}
			0
		}
		// The pointer over the page is an I-beam; over a child window (a drop-down box or a preview button) it gives the hint.
		0x0020 => {
			if let Some(sh) = shell() {
				let over = wparam as Handle;
				if over != hwnd {
					let h = ui::hover_of_child(sh, over);
					ui::set_hover(hwnd, sh, h);
				} else if lparam & 0xFFFF == 1 && !sh.grid {
					let mut p = Point { x: 0, y: 0 };
					GetCursorPos(&mut p);
					let mut o = Point { x: 0, y: 0 };
					ClientToScreen(hwnd, &mut o);
					let (x, y) = (p.x - o.x, p.y - o.y);
					if sh.preview.is_none() && ui::in_page(hwnd, sh, x, y) {
						SetCursor(LoadCursorA(null_mut(), IDC_IBEAM as *const u8));
						return 1;
					}
				}
			}
			DefWindowProcA(hwnd, msg, wparam, lparam)
		}
		0x0201 => {
			let (x, y) = xy(lparam);
			mouse_down(hwnd, x, y);
			0
		}
		0x0200 => {
			let (x, y) = xy(lparam);
			mouse_move(hwnd, x, y);
			0
		}
		0x0202 => {
			let (x, y) = xy(lparam);
			mouse_up(hwnd, x, y);
			0
		}
		// WM_CONTEXTMENU, from the right button or the keyboard (`lParam` is -1): a misspelt word offers its corrections. Anywhere else the
		// system has nothing to show.
		0x007B => {
			if lparam == -1 {
				spell_menu(hwnd, None);
			} else {
				let (sx, sy) = xy(lparam);
				let mut o = Point { x: 0, y: 0 };
				ClientToScreen(hwnd, &mut o);
				spell_menu(hwnd, Some((sx - o.x, sy - o.y)));
			}
			0
		}
		0x0010 => {
			// The close box asks like File > Exit, so unsaved changes are asked about.
			let grid = shell().is_some_and(|s| s.grid);
			if grid {
				DestroyWindow(hwnd);
			} else {
				run_command(hwnd, Command::Exit);
			}
			0
		}
		0x0011 => {
			// WM_QUERYENDSESSION: Windows is about to log off or shut down. A modified document asks like File > Exit; a cancel or a
			// failed save answers FALSE, which stops the shutdown.
			match shell() {
				Some(sh) if !sh.grid => isize::from(end_session_query(hwnd, sh)),
				_ => 1,
			}
		}
		0x0016 => {
			// WM_ENDSESSION: when wParam is TRUE the session really ends, so no more questions: a copy of the unsaved text goes aside.
			if wparam != 0 {
				if let Some(sh) = shell() {
					if !sh.grid {
						write_recovery(sh);
					}
				}
			}
			0
		}
		WM_DESTROY => {
			if let Some(sh) = shell() {
				dialogs::shutdown_block(hwnd, sh, false);
			}
			if let Some(sh) = shell() {
				if !sh.grid {
					ui::ini_save(sh);
				}
			}
			PostQuitMessage(0);
			0
		}
		_ => {
			// A wheel on Windows 95 has no message of its own: the IntelliPoint driver sends a registered one to the window that has
			// the focus, with the distance in `wParam`.
			if let Some(sh) = shell() {
				if sh.wheel_msg != 0 && msg == sh.wheel_msg {
					let delta = wparam as u16 as i16 as i32;
					// The driver's message carries no key state: read Ctrl from the thread's own, and the pointer from the cursor.
					if GetKeyState(VK_CONTROL) < 0 && !sh.grid && sh.preview.is_none() {
						let mut at = Point { x: 0, y: 0 };
						GetCursorPos(&mut at);
						ui::wheel_zoom(hwnd, sh, delta, at.x, at.y);
					} else {
						ui::wheel(hwnd, sh, delta);
					}
					return 0;
				}
			}
			DefWindowProcA(hwnd, msg, wparam, lparam)
		}
	}
}

/// Numbers the commands of `items` in the order the resource writer numbers them (an item with a submenu numbers its entries, not
/// itself), and adds them to the pop-up `pop` when there is one (the menu is not from the resource).
unsafe fn add_items(sh: &mut Shell, pop: Handle, items: &[quill_app::Item]) {
	for it in items {
		if it.rule && !pop.is_null() {
			AppendMenuA(pop, MF_SEPARATOR, 0, null());
		}
		let mut text: Vec<u8> = it.label.chars().map(|c| c as u32 as u8).collect();
		if !it.accel.is_empty() {
			text.push(b'\t');
			text.extend_from_slice(it.accel.as_bytes());
		}
		text.push(0);
		if it.sub.is_empty() {
			sh.cmds.push(it.command);
			if !pop.is_null() {
				AppendMenuA(pop, MF_STRING, 99 + sh.cmds.len(), text.as_ptr());
			}
		} else {
			let sub = if pop.is_null() { null_mut() } else { CreatePopupMenu() };
			add_items(sh, sub, it.sub);
			if !pop.is_null() {
				AppendMenuA(pop, MF_POPUP, sub as usize, text.as_ptr());
			}
		}
	}
}

/// Builds the native menu bar from the app's own menu tree: one definition for every build.
unsafe fn build_menu(sh: &mut Shell, instance: Handle) -> Handle {
	// The menu bar normally comes from the resource (win9x/resources/quill.json, written from the same tree), with the same ids.
	let from_res = LoadMenuA(instance, 1 as *const u8);
	let bar = if from_res.is_null() { CreateMenu() } else { null_mut() };
	for m in 0..quill_app::MENU_COUNT {
		let items = quill_app::menu_items(m);
		if items.is_empty() {
			continue;
		}
		let pop = if from_res.is_null() { CreatePopupMenu() } else { null_mut() };
		add_items(sh, pop, items);
		if !pop.is_null() {
			let mut title = quill_app::menu_title(m).to_vec();
			title.push(0);
			AppendMenuA(bar, MF_POPUP, pop as usize, title.as_ptr());
		}
	}
	let menu = if from_res.is_null() { bar } else { from_res };
	pdf::add_menu_item(menu);
	menu
}

#[repr(C)]
struct OpenFileNameA {
	size: u32,
	owner: Handle,
	instance: Handle,
	filter: *const u8,
	custom: *mut u8,
	max_custom: u32,
	filter_index: u32,
	file: *mut u8,
	max_file: u32,
	title_file: *mut u8,
	max_title: u32,
	initial_dir: *const u8,
	title: *const u8,
	flags: u32,
	file_offset: u16,
	file_ext: u16,
	def_ext: *const u8,
	cust_data: isize,
	hook: *const c_void,
	template: *const u8,
}

#[link(name = "comdlg32", kind = "raw-dylib", import_name_type = "undecorated")]
extern "system" {
	fn GetOpenFileNameA(ofn: *mut OpenFileNameA) -> i32;
	fn GetSaveFileNameA(ofn: *mut OpenFileNameA) -> i32;
}

/// The common Print dialog's structure (packed, as Win32 declares it).
#[repr(C, packed(1))]
struct PrintDlgA {
	size: u32,
	owner: Handle,
	dev_mode: Handle,
	dev_names: Handle,
	dc: Handle,
	flags: u32,
	from_page: u16,
	to_page: u16,
	min_page: u16,
	max_page: u16,
	copies: u16,
	instance: Handle,
	cust_data: isize,
	print_hook: *const c_void,
	setup_hook: *const c_void,
	print_template: *const u8,
	setup_template: *const u8,
	print_res: Handle,
	setup_res: Handle,
}

#[link(name = "comdlg32", kind = "raw-dylib", import_name_type = "undecorated")]
extern "system" {
	fn PrintDlgA(pd: *mut PrintDlgA) -> i32;
}

/// Shows the common Print dialog's setup page (`flags` PD_PRINTSETUP) and releases what it returns.
unsafe fn print_dialog(hwnd: Handle, flags: u32) -> bool {
	let mut pd = PrintDlgA {
		size: 66,
		owner: hwnd,
		dev_mode: null_mut(),
		dev_names: null_mut(),
		dc: null_mut(),
		flags,
		from_page: 0,
		to_page: 0,
		min_page: 0,
		max_page: 0,
		copies: 1,
		instance: null_mut(),
		cust_data: 0,
		print_hook: null(),
		setup_hook: null(),
		print_template: null(),
		setup_template: null(),
		print_res: null_mut(),
		setup_res: null_mut(),
	};
	PrintDlgA(&mut pd) != 0
}

/// Prints the document: through the common Print dialog (`ask`), or on the default printer without asking. `output` sends the job to a
/// file. Returns false when there was no printer or the job did not start.
unsafe fn print_job(hwnd: Handle, sh: &Shell, ask: bool, output: Option<&[u8]>) -> bool {
	// PD_RETURNDC, PD_USEDEVMODECOPIESANDCOLLATE, PD_NOSELECTION; the dialog is skipped with PD_RETURNDEFAULT.
	let mut flags = 0x0100 | 0x0004_0000 | 0x0004;
	if !ask {
		flags |= 0x0400;
	}
	let pages = gdi::page_count(&sh.app, &sh.store, sh.screen_dc, 96).min(9999) as u16;
	let mut pd = PrintDlgA {
		size: 66,
		owner: hwnd,
		dev_mode: null_mut(),
		dev_names: null_mut(),
		dc: null_mut(),
		flags,
		from_page: 1,
		to_page: pages,
		min_page: 1,
		max_page: pages,
		copies: 1,
		instance: null_mut(),
		cust_data: 0,
		print_hook: null(),
		setup_hook: null(),
		print_template: null(),
		setup_template: null(),
		print_res: null_mut(),
		setup_res: null_mut(),
	};
	if PrintDlgA(&mut pd) == 0 || pd.dc.is_null() {
		return false;
	}
	let dc = pd.dc;
	let (mut from, mut to) = (1u32, u32::MAX);
	// PD_PAGENUMS: the range the user chose.
	if pd.flags & 0x0002 != 0 {
		from = u32::from(pd.from_page);
		to = u32::from(pd.to_page);
	}
	let title: Vec<u8> = if sh.app.name().is_empty() {
		b"(Untitled)".to_vec()
	} else {
		sh.app.name().to_vec()
	};
	let ok = gdi::print_document(dc, &sh.app, &sh.store, &title, output, from, to);
	DeleteDC(dc);
	ok
}

/// What the Explorer-style common dialog tells its hook about a change of the file type: the header, then the dialog's own structure.
#[repr(C)]
struct OfNotifyA {
	hwnd_from: Handle,
	id_from: usize,
	code: u32,
	ofn: *const OpenFileNameA,
	file: *const u8,
}

/// `CDN_TYPECHANGE`: `CDN_FIRST` is 0U - 601U and the type change is six after it.
const CDN_TYPECHANGE: u32 = 0u32.wrapping_sub(607);
/// The messages that read and write the dialog's controls (`CDM_FIRST` is `WM_USER` + 100).
const CDM_GETSPEC: u32 = 0x0464;
const CDM_SETCONTROLTEXT: u32 = 0x0468;
const CDM_SETDEFEXT: u32 = 0x046A;
/// The file name box of the Explorer-style dialog (`cmb13`) and of the older one (`edt1`).
const CMB13: usize = 0x047C;
const EDT1: usize = 0x0480;
/// The extensions the Save As types write, by the number of the type in the list.
const SAVE_EXTS: [&[u8]; 3] = [b"qld", b"rtf", b"txt"];

/// Replaces the extension of `name` with `ext` when it has one of the Save As types' extensions, and adds it when it has none.
fn with_extension(name: &[u8], ext: &[u8]) -> Vec<u8> {
	let dot = name.iter().rposition(|b| *b == b'.');
	let stem = match dot {
		Some(d)
			if name
				.get(d + 1..)
				.is_some_and(|e| SAVE_EXTS.iter().any(|k| e.eq_ignore_ascii_case(k))) =>
		{
			&name[..d]
		}
		_ => name,
	};
	let mut out = stem.to_vec();
	out.push(b'.');
	out.extend_from_slice(ext);
	out
}

/// The Save As dialog's hook: when the type changes, the file name stays and only its extension follows the type, so a name already
/// typed is never typed again. Windows 95's common dialog sends `CDN_TYPECHANGE` to a hook in the Explorer style.
unsafe extern "system" fn save_hook(dlg: Handle, msg: u32, _wparam: usize, lparam: isize) -> usize {
	if msg != 0x004E || lparam == 0 {
		return 0;
	}
	let nm = &*(lparam as *const OfNotifyA);
	if nm.code != CDN_TYPECHANGE || nm.ofn.is_null() {
		return 0;
	}
	let Some(ext) = (*nm.ofn).filter_index.checked_sub(1).and_then(|i| SAVE_EXTS.get(i as usize)) else {
		return 0;
	};
	let parent = GetParent(dlg);
	let mut spec = [0u8; 264];
	let n = SendMessageA(parent, CDM_GETSPEC, 260, spec.as_mut_ptr() as isize);
	if n <= 1 {
		return 0;
	}
	let name = spec.get(..(n as usize - 1).min(260)).unwrap_or(&[]);
	if name.iter().any(|b| *b == b'*' || *b == b'?') {
		return 0;
	}
	let mut new = with_extension(name, ext);
	new.push(0);
	// Which box holds the name depends on the dialog's style; the one that is not there ignores the message.
	SendMessageA(parent, CDM_SETCONTROLTEXT, CMB13, new.as_ptr() as isize);
	SendMessageA(parent, CDM_SETCONTROLTEXT, EDT1, new.as_ptr() as isize);
	let mut def = ext.to_vec();
	def.push(0);
	SendMessageA(parent, CDM_SETDEFEXT, 0, def.as_ptr() as isize);
	0
}

/// Which file dialog.
#[derive(Copy, Clone, PartialEq)]
enum FileKind {
	Open,
	Save,
	Picture,
}

/// Shows the common Open, Save As or Insert Picture dialog (the Explorer style, no hook) and returns the chosen path. It starts in the
/// folder of the last one, and the folder of what was chosen is remembered for the next.
unsafe fn file_dialog(hwnd: Handle, sh: &mut Shell, kind: FileKind) -> Option<Vec<u8>> {
	let mut file = [0u8; 260];
	let docs: &[u8] =
		b"Quill Documents (*.qld)\0*.qld\0Rich Text Format (*.rtf)\0*.rtf\0Text Files (*.txt)\0*.txt\0All Files (*.*)\0*.*\0\0";
	let pics: &[u8] =
		b"Pictures (*.bmp;*.png)\0*.bmp;*.png\0Bitmaps (*.bmp)\0*.bmp\0PNG Pictures (*.png)\0*.png\0All Files (*.*)\0*.*\0\0";
	let filter = if kind == FileKind::Picture { pics } else { docs };
	let mut dir = sh.app.last_folder().to_vec();
	dir.push(0);
	// Save As starts with the document's own file name, so it is never typed again, in the type that matches its extension.
	let mut filter_index = 1;
	if kind == FileKind::Save {
		let own = sh.app.name();
		let base = own
			.iter()
			.rposition(|b| *b == b'\\' || *b == b':')
			.map_or(own, |i| &own[i + 1..]);
		let n = base.len().min(259);
		file[..n].copy_from_slice(&base[..n]);
		if let Some(d) = base.iter().rposition(|b| *b == b'.') {
			let e = &base[d + 1..];
			if let Some(i) = SAVE_EXTS.iter().position(|k| e.eq_ignore_ascii_case(k)) {
				filter_index = i as u32 + 1;
			}
		}
	}
	let title: &[u8] = match kind {
		FileKind::Open => b"Open\0",
		FileKind::Save => b"Save As\0",
		FileKind::Picture => b"Insert Picture\0",
	};
	let mut ofn = OpenFileNameA {
		size: 76,
		owner: hwnd,
		instance: null_mut(),
		filter: filter.as_ptr(),
		custom: null_mut(),
		max_custom: 0,
		filter_index,
		file: file.as_mut_ptr(),
		max_file: 260,
		title_file: null_mut(),
		max_title: 0,
		initial_dir: if dir.len() > 1 { dir.as_ptr() } else { null() },
		title: title.as_ptr(),
		// OFN_EXPLORER | OFN_HIDEREADONLY | OFN_PATHMUSTEXIST, then OFN_OVERWRITEPROMPT (save) or OFN_FILEMUSTEXIST (open)
		// ... and for Save As, OFN_ENABLEHOOK so the file name follows a change of type.
		flags: 0x0008_0000 | 0x0004 | 0x0800 | if kind == FileKind::Save { 0x0002 | 0x0020 } else { 0x1000 },
		file_offset: 0,
		file_ext: 0,
		def_ext: if kind == FileKind::Save { b"qld\0".as_ptr() } else { null() },
		cust_data: 0,
		hook: if kind == FileKind::Save {
			save_hook as *const c_void
		} else {
			null()
		},
		template: null(),
	};
	let ok = if kind == FileKind::Save {
		GetSaveFileNameA(&mut ofn)
	} else {
		GetOpenFileNameA(&mut ofn)
	};
	if ok == 0 {
		return None;
	}
	let mut n = file.iter().position(|b| *b == 0).unwrap_or(0);
	if kind == FileKind::Save {
		// The dialog adds the default extension itself, but a name that reaches here without one gets the chosen type's, and the
		// overwrite prompt then asks about the name that will be written.
		let typed = n;
		n = quill_app::savename::complete_name(&mut file, n, ofn.filter_index);
		if n != typed
			&& file.get(..n).is_some_and(|f| sh.files.open(f).is_some())
			// MB_YESNO | MB_ICONQUESTION
			&& MessageBoxA(hwnd, b"That file already exists. Replace it?\0".as_ptr(), b"Quill\0".as_ptr(), 0x24) != 6
		{
			return None;
		}
	}
	let path = file.get(..n).map(|f| f.to_vec())?;
	sh.app.remember_folder(&path);
	Some(path)
}

/// Saves the current document, asking for a name first when it has none. False when the user cancelled or the save failed.
unsafe fn save_current(hwnd: Handle, sh: &mut Shell) -> bool {
	if sh.app.name().is_empty() {
		let Some(path) = file_dialog(hwnd, sh, FileKind::Save) else {
			return false;
		};
		return sh.app.save_as(&mut sh.store, &mut sh.files, &path);
	}
	sh.app.command(&mut sh.store, &mut sh.files, Command::Save);
	!sh.app.modified()
}

/// WM_QUERYENDSESSION: true when Windows may end the session (`quill_app::session::end_session_allowed`, CORE-7).
unsafe fn end_session_query(hwnd: Handle, sh: &mut Shell) -> bool {
	if !sh.app.unsaved_work() {
		return true;
	}
	let name = sh.app.name().to_vec();
	let answer = dialogs::confirm_save(hwnd, &name, Command::Exit) as u32;
	let saved = answer == quill_app::session::ANSWER_SAVE && save_current(hwnd, sh);
	if saved {
		retitle(hwnd, sh);
	}
	let allowed = quill_app::session::end_session_allowed(true, answer, saved);
	if !allowed {
		// ReactOS goes on to shut down whatever the answer, and sends no WM_ENDSESSION, so the text is copied aside before it is refused.
		write_recovery(sh);
	}
	allowed
}

/// Writes a recovery copy of the document when it has unsaved work: beside the original, or in the central folder for a document
/// with no name (`recovery_dir`).
unsafe fn write_recovery(sh: &mut Shell) {
	sh.app.save_recovery(&mut sh.store, &mut sh.files, Why::EndOfSession);
}

/// Opens QUILL.HLP (beside the program) in WinHelp at its contents.
unsafe fn open_help(hwnd: Handle) {
	const HELP_CONTENTS: u32 = 3;
	let mut path = [0u8; 280];
	// SAFETY: `path` holds 280 bytes and the call is told 260.
	let n = unsafe { GetModuleFileNameA(null_mut(), path.as_mut_ptr(), 260) } as usize;
	let dir = path
		.get(..n)
		.and_then(|p| p.iter().rposition(|b| *b == b'\\'))
		.map_or(0, |i| i + 1);
	let name = b"QUILL.HLP\0";
	if let Some(dst) = path.get_mut(dir..dir + name.len()) {
		dst.copy_from_slice(name);
	}
	// SAFETY: `path` is NUL-terminated.
	unsafe { WinHelpA(hwnd, path.as_ptr(), HELP_CONTENTS, 0) };
}

/// Shows every dialog the editor has asked for, as a real dialog (D86), and applies the answers.
unsafe fn handle_dialogs(hwnd: Handle) {
	if let Some(sh) = shell() {
		if sh.app.take_help() {
			open_help(hwnd);
		}
	}
	while let Some(sh) = shell() {
		let Some(d) = sh.app.take_dialog() else {
			return;
		};
		#[cfg(feature = "bench")]
		bench::dialog_taken();
		match d {
			quill_app::Dialog::About => dialogs::about(hwnd, sh),
			quill_app::Dialog::Open => {
				if let Some(path) = file_dialog(hwnd, sh, FileKind::Open) {
					if sh.app.open(&mut sh.store, &mut sh.files, &path).is_err() {
						sh.app.set_message("Quill cannot open that file.");
					}
				}
			}
			quill_app::Dialog::SaveAs => {
				if let Some(path) = file_dialog(hwnd, sh, FileKind::Save) {
					sh.app.save_as(&mut sh.store, &mut sh.files, &path);
				}
			}
			quill_app::Dialog::Picture => {
				if let Some(path) = file_dialog(hwnd, sh, FileKind::Picture) {
					if sh.app.insert_picture(&mut sh.store, &mut sh.files, &path).is_err() {
						sh.app.set_message("Quill cannot use that picture.");
					}
				}
			}
			quill_app::Dialog::Print => {
				ui::refresh_clock(sh);
				if !print_job(hwnd, sh, true, None) {
					sh.app.set_message("Quill could not print.");
				}
			}
			quill_app::Dialog::Fonts => dialogs::fonts(hwnd, sh),
			quill_app::Dialog::Tabs => dialogs::tabs(hwnd, sh),
			quill_app::Dialog::Find(replace) => dialogs::find(hwnd, sh, replace),
			quill_app::Dialog::Spelling => dialogs::spelling(hwnd, sh),
			quill_app::Dialog::ConfirmSave(c) => {
				let name = sh.app.name().to_vec();
				match dialogs::confirm_save(hwnd, &name, c) {
					6 => {
						if save_current(hwnd, sh) {
							sh.app.run_confirmed(&mut sh.store, &mut sh.files, c);
						}
					}
					7 => {
						sh.app.discard_changes();
						sh.app.run_confirmed(&mut sh.store, &mut sh.files, c);
					}
					_ => {}
				}
			}
		}
	}
}

unsafe fn menu_command(hwnd: Handle, id: usize) {
	let Some(sh) = shell() else {
		return;
	};
	// A recent file.
	if (ui::ID_RECENT..ui::ID_RECENT + quill_app::recent::RECENT_MAX).contains(&id) {
		run_command(hwnd, Command::Recent((id - ui::ID_RECENT) as u8));
		return;
	}
	if id == pdf::ID_PRINT_PDF {
		if !sh.grid {
			pdf::print_to_pdf(hwnd, sh);
		}
		return;
	}
	let Some(c) = id.checked_sub(100).and_then(|i| sh.cmds.get(i)).copied() else {
		return;
	};
	run_command(hwnd, c);
}

/// Runs a command from the menu or the toolbar.
unsafe fn run_command(hwnd: Handle, c: Command) {
	let Some(sh) = shell() else {
		return;
	};
	ui::blink_restart(hwnd, sh);
	if matches!(c, Command::ZoomIn | Command::ZoomOut) {
		// The same step the buttons take, from the zoom shown now (which for Page Width depends on the window).
		let (w, _) = client_size(hwnd);
		sh.app.set_shown_zoom(view::zoom_pct(&sh.app, w));
	}
	match c {
		Command::Preview if !sh.grid => {
			let first = 1;
			ui::preview_open(hwnd, sh, first, false);
		}
		Command::PrintSetup if !sh.grid => {
			// The common dialog's setup page: choose the printer and paper.
			print_dialog(hwnd, 0x0040);
		}
		_ => sh.app.command(&mut sh.store, &mut sh.files, c),
	}
	if !sh.grid {
		handle_dialogs(hwnd);
	}
	if !sh.grid {
		sync_chrome(hwnd, sh);
		if matches!(
			c,
			Command::Chrome(_)
				| Command::Zoom(_)
				| Command::ZoomIn
				| Command::ZoomOut
				| Command::Recent(_)
				| Command::Open
				| Command::SaveAs
				| Command::Save
				| Command::SpellAsYouType
		) {
			ui::ini_save(sh);
		}
		ui::spell_arm(hwnd, sh);
	}
	retitle(hwnd, sh);
	story_changed(sh);
	ui::update_scroll(hwnd, sh);
	ui::ensure_visible(hwnd, sh);
	InvalidateRect(hwnd, null(), 0);
	if sh.app.quit() {
		DestroyWindow(hwnd);
	}
}

/// When the view moved to a header or a footer (or came back), the page scrolls to the top of it and back to where it was.
fn story_changed(sh: &mut Shell) {
	let now = sh.app.story_view();
	if now == sh.last_view {
		return;
	}
	if sh.last_view == 0 {
		sh.saved_vpos = sh.vpos;
	}
	sh.vpos = if now == 0 { sh.saved_vpos } else { 0 };
	sh.last_view = now;
}

/// A face or size was chosen in one of the boxes, or one of the preview's buttons was pressed.
unsafe fn combo_changed(hwnd: Handle, wparam: usize) {
	let Some(sh) = shell() else {
		return;
	};
	let (id, code) = (wparam & 0xFFFF, wparam >> 16);
	if (ui::ID_PV_PRINT..=ui::ID_PV_CLOSE).contains(&id) {
		ui::preview_command(hwnd, sh, id);
		return;
	}
	if code != 1 {
		return; // CBN_SELCHANGE
	}
	if id == chrome::ID_STYLE {
		let at = SendMessageA(sh.combos.as_ref().map_or(null_mut(), |c| c.style), 0x0147, 0, 0);
		if at >= 0 {
			sh.app.apply_style(&mut sh.store, at as usize);
		}
	} else if let Some(choice) = sh.combos.as_ref().and_then(|c| c.choice(id)) {
		if id == chrome::ID_FACE {
			sh.app.set_face(&mut sh.store, &choice, chrome::family_of(&choice));
		} else if let Some(pts) = core::str::from_utf8(&choice).ok().and_then(|t| t.trim().parse::<u16>().ok()) {
			sh.app.set_size(&mut sh.store, pts * 2);
		}
	}
	SetFocus(hwnd);
	InvalidateRect(hwnd, null(), 0);
}

/// Reads the system colour scheme and rebuilds what was made from it.
unsafe fn apply_scheme() {
	scheme::refresh();
	chrome::recolour_strip();
}

/// Fills `r` in `dc` with one solid colour.
unsafe fn fill_rect(dc: Handle, r: &Rect, colour: u32) {
	let b = CreateSolidBrush(colour);
	FillRect(dc, r, b);
	DeleteObject(b);
}

pub(crate) unsafe fn client_size(hwnd: Handle) -> (i32, i32) {
	let mut r = Rect {
		left: 0,
		top: 0,
		right: 0,
		bottom: 0,
	};
	GetClientRect(hwnd, &mut r);
	(r.right, r.bottom)
}

/// The height of the toolbars and the ruler together (or of the preview's bar), which is where the page starts.
pub(crate) fn chrome_height(sh: &Shell) -> i32 {
	if sh.grid {
		return 0;
	}
	if sh.preview.is_some() {
		return ui::PV_BAR;
	}
	let (h0, h1) = chrome::rows(&sh.app);
	h0 + h1 + if sh.app.show_ruler() { chrome::RULER_H } else { 0 }
}

/// Asks for the page, the text below the toolbars and the ruler, and the status line to be painted again, and nothing above them.
pub(crate) unsafe fn invalidate_page(hwnd: Handle, sh: &Shell) {
	if sh.grid || sh.preview.is_some() {
		InvalidateRect(hwnd, null(), 0);
		return;
	}
	let (w, h) = client_size(hwnd);
	let r = Rect {
		left: 0,
		top: chrome_height(sh),
		right: w,
		bottom: h,
	};
	InvalidateRect(hwnd, &r, 0);
}

/// Asks for the ruler alone to be painted again.
pub(crate) unsafe fn invalidate_ruler(hwnd: Handle, sh: &Shell) {
	if !sh.app.show_ruler() || sh.grid || sh.preview.is_some() {
		return;
	}
	let (w, _) = client_size(hwnd);
	let top = chrome_height(sh) - chrome::RULER_H;
	let r = Rect {
		left: 0,
		top,
		right: w,
		bottom: top + chrome::RULER_H,
	};
	InvalidateRect(hwnd, &r, 0);
}

/// Where the ruler is and where the text's left edge is, for the mouse: the ruler's top, the text's left edge and its width in pixels.
unsafe fn ruler_geometry(hwnd: Handle, sh: &Shell) -> (i32, i32, i32, i32) {
	let area = ui::page_area(hwnd, sh);
	let w = area.right - area.left;
	let pct = ui::pct(sh, w);
	let (left, tw) = chrome::text_extent(&sh.app, w, sh.hscroll, pct);
	let (h0, h1) = chrome::rows(&sh.app);
	(h0 + h1, left, tw, pct)
}

unsafe fn mouse_down(hwnd: Handle, x: i32, y: i32) {
	let Some(sh) = shell() else {
		return;
	};
	if sh.preview.is_some() || sh.grid {
		return;
	}
	if let Some((i, _)) = chrome::button_index(&sh.app, x, y) {
		// A button acts when the mouse button comes back up over it, as a Windows 95 button does; until then it shows pressed.
		sh.pressed = Some(i);
		SetCapture(hwnd);
		ui::set_hover(hwnd, sh, ui::Hover::Button(i));
		ui::status_update(hwnd, sh);
		InvalidateRect(hwnd, null(), 0);
		return;
	}
	if sh.app.show_ruler() {
		let (top, left, w, pct) = ruler_geometry(hwnd, sh);
		let st = chrome::RulerState::of(&sh.app, &sh.store);
		match chrome::ruler_hit(x, y, top, left, w, &st, pct) {
			Some(chrome::Hit::Kind) => sh.ruler_kind = (sh.ruler_kind + 1) % 4,
			Some(chrome::Hit::Scale(t)) => sh.app.add_tab(&mut sh.store, chrome::snap(t), sh.ruler_kind),
			Some(hit) => {
				sh.drag = Some(chrome::Drag {
					hit,
					state: st,
					start: st,
				});
				SetCapture(hwnd);
			}
			None => {}
		}
		if y < top + chrome::RULER_H {
			InvalidateRect(hwnd, null(), 0);
			return;
		}
	}
	if ui::in_page(hwnd, sh, x, y) {
		ui::page_down(hwnd, sh, x, y);
		ui::spell_arm(hwnd, sh);
	}
}

/// The ids of the pop-up menu on a misspelt word: up to five suggestions, then Ignore All and Add to Dictionary.
const ID_SPELL: usize = 700;
const ID_SPELL_IGNORE: usize = 705;
const ID_SPELL_ADD: usize = 706;
const MF_GRAYED: u32 = 1;
pub(crate) const TPM_RIGHTBUTTON: u32 = 2;
pub(crate) const TPM_RETURNCMD: u32 = 0x100;

/// The context menu was asked for, with the pointer at (`x`, `y`) in the client area or, from the keyboard (Shift+F10 or the menu key),
/// at the caret: over a misspelt word it opens a menu of the word's suggestions, Ignore All and Add to Dictionary.
unsafe fn spell_menu(hwnd: Handle, at: Option<(i32, i32)>) {
	let Some(sh) = shell() else {
		return;
	};
	if sh.preview.is_some() || sh.grid || !sh.app.spell_live() || !sh.app.editing() {
		return;
	}
	let (pos, x, y) = match at {
		Some((x, y)) if ui::in_page(hwnd, sh, x, y) => (ui::pos_under(hwnd, sh, x, y), x, y),
		Some(_) => return,
		None => {
			let Some((x, y)) = ui::caret_point(hwnd, sh) else {
				return;
			};
			(sh.app.caret(), x, y)
		}
	};
	let Some((lo, _)) = sh.app.spell_mark_at(&sh.store, pos) else {
		return;
	};
	let sg = sh.app.spell_suggestions_at(&sh.store, lo);
	let menu = CreatePopupMenu();
	let shown = sg.len().min(5);
	for i in 0..shown {
		if let Some(c) = sg.get(i) {
			let mut text = c.text.get(..usize::from(c.len)).unwrap_or(&[]).to_vec();
			text.push(0);
			AppendMenuA(menu, MF_STRING, ID_SPELL + i, text.as_ptr());
		}
	}
	if shown == 0 {
		AppendMenuA(menu, MF_STRING | MF_GRAYED, 0, b"(No suggestions)\0".as_ptr());
	}
	AppendMenuA(menu, MF_SEPARATOR, 0, null());
	AppendMenuA(menu, MF_STRING, ID_SPELL_IGNORE, b"Ignore All\0".as_ptr());
	AppendMenuA(menu, MF_STRING, ID_SPELL_ADD, b"Add to Dictionary\0".as_ptr());
	let mut at = Point { x, y };
	ClientToScreen(hwnd, &mut at);
	let id = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_RIGHTBUTTON, at.x, at.y, 0, hwnd, null()) as usize;
	DestroyMenu(menu);
	sh.app.set_message("");
	sh.hint_on = false;
	let done = match id {
		ID_SPELL..=704 => sg.get(id - ID_SPELL).is_some_and(|c| {
			let with = c.text.get(..usize::from(c.len)).unwrap_or(&[]);
			sh.app.spell_replace_at(&mut sh.store, lo, with)
		}),
		ID_SPELL_IGNORE => sh.app.spell_ignore_at(&mut sh.store, lo),
		ID_SPELL_ADD => sh.app.spell_add_at(&mut sh.store, &mut sh.files, lo),
		_ => false,
	};
	if done {
		retitle(hwnd, sh);
		story_changed(sh);
		ui::update_scroll(hwnd, sh);
		ui::ensure_visible(hwnd, sh);
		invalidate_page(hwnd, sh);
		ui::spell_arm(hwnd, sh);
	}
	ui::status_update(hwnd, sh);
}

unsafe fn mouse_move(hwnd: Handle, x: i32, y: i32) {
	let Some(sh) = shell() else {
		return;
	};
	if let Some(mut d) = sh.drag {
		let (_, left, w, pct) = ruler_geometry(hwnd, sh);
		chrome::drag_to(&mut d, x, left, w, pct);
		sh.drag = Some(d);
		invalidate_ruler(hwnd, sh);
		return;
	}
	if sh.pic_drag.is_some() {
		ui::picture_drag(hwnd, sh, x, y);
		return;
	}
	if sh.sel.is_some() {
		ui::page_drag(hwnd, sh, x, y);
		return;
	}
	if sh.pressed.is_some() {
		return;
	}
	let h = ui::hover_at(sh, x, y);
	ui::set_hover(hwnd, sh, h);
}

unsafe fn mouse_up(hwnd: Handle, x: i32, y: i32) {
	let Some(sh) = shell() else {
		return;
	};
	if let Some(i) = sh.pressed.take() {
		ReleaseCapture();
		let over = chrome::button_index(&sh.app, x, y).map(|(j, _)| j);
		InvalidateRect(hwnd, null(), 0);
		if over == Some(i) {
			if let Some((r, b)) = chrome::button_rect(&sh.app, i) {
				match b.act {
					chrome::Act::Cmd(c) => run_command(hwnd, c),
					chrome::Act::Colour => {
						let top = r.top - 2;
						if let Some(choice) = chrome::colour_menu(hwnd, b.x, top + chrome::TB_H) {
							sh.app.set_colour(&mut sh.store, choice);
						}
					}
					chrome::Act::None => run_command(hwnd, Command::Spelling),
				}
			}
			InvalidateRect(hwnd, null(), 0);
		}
		return;
	}
	if sh.pic_drag.is_some() {
		ui::picture_up(hwnd, sh);
		return;
	}
	if sh.sel.is_some() {
		ui::page_up(hwnd, sh);
		return;
	}
	let Some(mut d) = sh.drag.take() else {
		return;
	};
	ReleaseCapture();
	let (_, left, w, pct) = ruler_geometry(hwnd, sh);
	chrome::drag_to(&mut d, x, left, w, pct);
	match d.hit {
		chrome::Hit::First | chrome::Hit::Left | chrome::Hit::Both | chrome::Hit::Right => {
			sh.app
				.set_indents(&mut sh.store, d.state.left, d.state.first - d.state.left, d.state.right);
		}
		_ => {}
	}
	InvalidateRect(hwnd, null(), 0);
}

/// Shows or hides the combo boxes with the toolbar, ticks the menu items, rewrites the recent files in the File menu and lays the
/// page out again.
unsafe fn sync_chrome(hwnd: Handle, sh: &mut Shell) {
	if let Some(cb) = &sh.combos {
		cb.show(&sh.app, sh.preview.is_some());
	}
	let menu = GetMenu(hwnd);
	if !menu.is_null() {
		for (i, c) in sh.cmds.iter().enumerate() {
			let on = match c {
				Command::Chrome(bit) => Some(sh.app.chrome_on(*bit)),
				Command::Zoom(z) => Some(sh.app.zoom() == *z),
				Command::SpellAsYouType => Some(sh.app.spell_live()),
				_ => None,
			};
			if let Some(on) = on {
				CheckMenuItem(menu, 100 + i as u32, if on { 0x8 } else { 0 });
			}
		}
	}
	ui::status_layout(hwnd, sh);
	ui::sync_recent_menu(hwnd, sh);
	ui::tips_rebuild(hwnd, sh);
	InvalidateRect(hwnd, null(), 0);
}

/// Puts the document's name in the title bar when it is not there already.
unsafe fn retitle(hwnd: Handle, sh: &mut Shell) {
	// Every command and key comes through here, so the shutdown block follows the dirty flag; it calls only when that changes.
	let dirty = sh.app.unsaved_work();
	if dirty != sh.blocked {
		dialogs::shutdown_block(hwnd, sh, dirty);
	}
	let mut t = b"Quill - ".to_vec();
	if sh.app.name().is_empty() {
		t.extend_from_slice(b"(Untitled)");
	} else {
		t.extend_from_slice(sh.app.name());
	}
	t.push(0);
	if sh.title != t {
		SetWindowTextA(hwnd, t.as_ptr());
		sh.title = t;
	}
}

unsafe fn press(hwnd: Handle, k: Key) {
	#[cfg(feature = "bench")]
	bench::key_begin();
	{
		span!(KTotal);
		press_key(hwnd, k);
	}
	#[cfg(feature = "bench")]
	bench::key_end();
}

unsafe fn press_key(hwnd: Handle, k: Key) {
	let Some(sh) = shell() else {
		return;
	};
	ui::blink_restart(hwnd, sh);
	if let Some(pv) = sh.preview {
		let (_, _, pages) = gdi::preview_pages(&sh.app, &sh.store, sh.screen_dc);
		let line = view::LINE_STEP * 3;
		match k {
			Key::Esc => ui::preview_close(hwnd, sh),
			Key::PageDown => ui::preview_step(hwnd, sh, 1),
			Key::PageUp => ui::preview_step(hwnd, sh, -1),
			Key::Down if pv.actual => ui::scroll_v(hwnd, sh, pv.vscroll + line),
			Key::Up if pv.actual => ui::scroll_v(hwnd, sh, pv.vscroll - line),
			Key::Right if pv.actual => ui::scroll_h(hwnd, sh, pv.hscroll + line),
			Key::Left if pv.actual => ui::scroll_h(hwnd, sh, pv.hscroll - line),
			Key::Down | Key::Right => ui::preview_step(hwnd, sh, 1),
			Key::Up | Key::Left => ui::preview_step(hwnd, sh, -1),
			Key::Home | Key::CtrlHome | Key::End | Key::CtrlEnd => {
				let mut v = pv;
				v.first = if matches!(k, Key::Home | Key::CtrlHome) { 1 } else { pages };
				v.vscroll = 0;
				sh.preview = Some(v);
				ui::update_scroll(hwnd, sh);
				InvalidateRect(hwnd, null(), 0);
			}
			Key::Char(b'T' | b't') => ui::preview_command(hwnd, sh, ui::ID_PV_PAGES),
			Key::Char(b'Z' | b'z') => ui::preview_command(hwnd, sh, ui::ID_PV_ZOOM),
			Key::Char(b'P' | b'p') | Key::Ctrl(b'p') => ui::preview_command(hwnd, sh, ui::ID_PV_PRINT),
			_ => {}
		}
		return;
	}
	if !sh.grid && sh.app.editing() {
		let page = match k {
			Key::PageDown => Some((true, false)),
			Key::PageUp => Some((false, false)),
			Key::Select(Dir::PageDown) => Some((true, true)),
			Key::Select(Dir::PageUp) => Some((false, true)),
			_ => None,
		};
		if let Some((down, extend)) = page {
			ui::page_key(hwnd, sh, down, extend);
			return;
		}
	}
	let before = {
		span!(KSig);
		chrome::signature(&sh.app, &sh.store)
	};
	{
		span!(KApp);
		sh.app.key(&mut sh.store, &mut sh.files, k);
	}
	if !sh.grid {
		span!(KDialogs);
		handle_dialogs(hwnd);
	}
	{
		span!(KTitle);
		retitle(hwnd, sh);
	}
	if sh.grid {
		InvalidateRect(hwnd, null(), 0);
	} else {
		// A key changes the page and the status line; the toolbars and the ruler are painted again only when they show something new.
		let hs = sh.hscroll;
		{
			span!(KStory);
			story_changed(sh);
		}
		{
			// The bars are told once, after the caret is in view; before that only the window's own arithmetic runs.
			span!(KScroll);
			ui::settle_scroll(hwnd, sh);
		}
		{
			span!(KEnsure);
			ui::ensure_visible_with(hwnd, sh, false);
		}
		{
			span!(KScroll);
			ui::update_scroll(hwnd, sh);
		}
		let after = {
			span!(KSig);
			chrome::signature(&sh.app, &sh.store)
		};
		{
			span!(KInval);
			// The toolbars and the ruler do not depend on how far the page is scrolled down, so a key that only does that paints the
			// page area.
			if before != after || hs != sh.hscroll {
				InvalidateRect(hwnd, null(), 0);
			} else {
				invalidate_page(hwnd, sh);
			}
		}
		{
			span!(KArm);
			ui::spell_arm(hwnd, sh);
		}
	}
	if sh.app.quit() {
		DestroyWindow(hwnd);
	}
}

/// Handles WM_PAINT: draws the whole picture off-screen, then copies only the damaged rectangle to the window in one call, so the
/// steps of the drawing (the grey desk, the white page, the toolbar's bevels, then the text) are never on the screen.
unsafe fn paint(hwnd: Handle) {
	let Some(sh) = shell() else {
		return;
	};
	#[cfg(feature = "bench")]
	let t_paint = bench::paint_begin();
	let mut ps = MaybeUninit::<PaintStruct>::uninit();
	let wdc = BeginPaint(hwnd, ps.as_mut_ptr());
	let mut damage = ps.assume_init_ref().paint;
	let (cw, chh) = client_size(hwnd);
	let stale = sh.back.as_ref().is_none_or(|b| b.w != cw || b.h != chh);
	if stale {
		if let Some(old) = sh.back.take() {
			old.free();
		}
		sh.back = Back::new(wdc, cw, chh);
		// A new bitmap holds nothing yet, so the whole client area is painted into it.
		damage = Rect {
			left: 0,
			top: 0,
			right: cw,
			bottom: chh,
		};
	}
	match sh.back.as_ref().map(|b| b.dc) {
		Some(mdc) => {
			paint_into(hwnd, mdc, sh, damage);
			span!(PBlit);
			BitBlt(
				wdc,
				damage.left,
				damage.top,
				damage.right - damage.left,
				damage.bottom - damage.top,
				mdc,
				damage.left,
				damage.top,
				SRCCOPY,
			);
		}
		None => paint_into(hwnd, wdc, sh, damage),
	}
	EndPaint(hwnd, ps.as_ptr());
	#[cfg(feature = "bench")]
	bench::paint_end(sh, t_paint);
}

/// Draws the window into `dc`, limited to the `damage` rectangle.
unsafe fn paint_into(hwnd: Handle, dc: Handle, sh: &mut Shell, damage: Rect) {
	if !sh.grid {
		span!(PClock);
		ui::status_update(hwnd, sh);
	}
	let saved = SaveDC(dc);
	IntersectClipRect(dc, damage.left, damage.top, damage.right, damage.bottom);
	SelectObject(dc, GetStockObject(OEM_FIXED_FONT));
	let mut g = Grid::<25>::new(0);
	// The text grid is the golden-frame screen and what shows over the page while the app is not editing; the page view needs none of it.
	if sh.grid || !sh.app.editing() {
		span!(PGrid);
		quill_app::draw(&sh.app, &sh.store, &mut g);
	}
	let (first, last) = if sh.grid {
		(0usize, 25usize)
	} else {
		// The page view above, then the status line, and the app's own dialogs over the page as text cells.
		let mut r = Rect {
			left: 0,
			top: 0,
			right: 0,
			bottom: 0,
		};
		GetClientRect(hwnd, &mut r);
		{
			span!(PGrey);
			fill_rect(dc, &r, scheme::face());
		}
		let (h0, h1) = chrome::rows(&sh.app);
		let tb = if sh.preview.is_none() { h0 + h1 } else { 0 };
		let rl = if sh.app.show_ruler() && sh.preview.is_none() {
			chrome::RULER_H
		} else {
			0
		};
		let area = ui::page_area(hwnd, sh);
		let pct = ui::pct(sh, area.right - area.left);
		if sh.preview.is_none() {
			span!(PChrome);
			if tb > 0 {
				// The boxes follow the caret even when the toolbar's own pixels are not damaged.
				if let Some(cb) = sh.combos.as_mut() {
					cb.sync(&sh.app, &sh.store);
				}
				if damage.top < tb {
					let hover = match sh.hover {
						ui::Hover::Button(i) => Some(i),
						_ => None,
					};
					#[cfg(feature = "bench")]
					bench::chrome();
					chrome::paint_toolbar(dc, r.right, &sh.app, &sh.store, hover, sh.pressed);
				}
			}
			if rl > 0 && damage.top < tb + rl && damage.bottom > tb {
				let (left, w) = chrome::text_extent(&sh.app, area.right - area.left, sh.hscroll, pct);
				let st = sh
					.drag
					.map_or_else(|| chrome::RulerState::of(&sh.app, &sh.store), |d| d.state);
				#[cfg(feature = "bench")]
				bench::chrome();
				chrome::paint_ruler(dc, tb, r.right, left, w, &st, sh.ruler_kind, pct);
			}
		}
		if let Some(pv) = sh.preview {
			// The bar the preview's own buttons sit on.
			fill_rect(
				dc,
				&Rect {
					left: 0,
					top: 0,
					right: r.right,
					bottom: ui::PV_BAR,
				},
				scheme::face(),
			);
			fill_rect(
				dc,
				&Rect {
					left: 0,
					top: ui::PV_BAR - 1,
					right: r.right,
					bottom: ui::PV_BAR,
				},
				scheme::shadow(),
			);
			let size = gdi::paint_preview(dc, area, &sh.app, &sh.store, sh.screen_dc, &pv, &mut sh.pics);
			if sh.pv_pages != size.pages {
				sh.pv_pages = size.pages;
				ui::status_update(hwnd, sh);
			}
			RestoreDC(dc, saved);
			return;
		}
		let (rows, _) = gdi::paint_page(
			dc,
			area,
			sh.hscroll,
			sh.vpos,
			pct,
			damage,
			&sh.app,
			&sh.store,
			sh.screen_dc,
			&mut sh.pics,
			sh.blink.shows(),
		);
		SelectObject(dc, GetStockObject(OEM_FIXED_FONT));
		sh.app.set_rows(rows);
		if !sh.app.editing() {
			draw_rows(dc, &g, 7, 21, sh, (r.right - 80 * sh.cw).max(0) / 2, 40);
		}
		RestoreDC(dc, saved);
		return;
	};
	for y in first..last {
		// One ExtTextOut per run of equal attributes.
		let mut x = 0usize;
		while x < 80 {
			let a = g.get(x, y).map_or(0x07, |c| c.attr);
			let mut run = [0u8; 80];
			let mut n = 0;
			while x + n < 80 {
				let Some(c) = g.get(x + n, y) else {
					break;
				};
				if c.attr != a {
					break;
				}
				if let Some(slot) = run.get_mut(n) {
					*slot = if c.ch == 0 { b' ' } else { c.ch };
				}
				n += 1;
			}
			let fg = VGA.get(usize::from(a & 15)).copied().unwrap_or(0xAAAAAA);
			let bg = VGA.get(usize::from(a >> 4)).copied().unwrap_or(0);
			SetTextColor(dc, fg);
			SetBkColor(dc, bg);
			let rect = Rect {
				left: x as i32 * sh.cw,
				top: y as i32 * sh.ch,
				right: (x + n) as i32 * sh.cw,
				bottom: (y as i32 + 1) * sh.ch,
			};
			ExtTextOutA(dc, rect.left, rect.top, ETO_OPAQUE, &rect, run.as_ptr(), n as u32, null());
			x += n;
		}
	}
	RestoreDC(dc, saved);
}

struct Play {
	keys: Vec<Key>,
	at: usize,
	frames: String,
	n: u32,
}

static mut PLAY: *mut Play = null_mut();

unsafe fn play(hwnd: Handle) {
	let (Some(sh), Some(pl)) = (shell(), PLAY.as_mut()) else {
		return;
	};
	let snapshot = |sh: &Shell, pl: &mut Play| {
		let mut g = Grid::<25>::new(0);
		quill_app::draw(&sh.app, &sh.store, &mut g);
		pl.n += 1;
		let _ = g.dump(pl.n, &mut pl.frames);
	};
	if pl.at == 0 && pl.n == 0 {
		snapshot(sh, pl);
	}
	while let Some(k) = pl.keys.get(pl.at).copied() {
		pl.at += 1;
		sh.app.key(&mut sh.store, &mut sh.files, k);
		snapshot(sh, pl);
		if sh.app.quit() {
			break;
		}
	}
	sh.app.save_settings(&mut sh.files);
	let _ = writeln!(pl.frames, "=== end ===");
	if let Some((h, _)) = sh.dump {
		let mut out = Win32Sink(h);
		out.write(pl.frames.as_bytes());
		CloseHandle(h);
		sh.dump = None;
	}
	InvalidateRect(hwnd, null(), 0);
	if !sh.hold {
		DestroyWindow(hwnd);
	}
}

/// The widths and vertical metrics of the faces Quill's samples use, as the screen reports them: the table the host's
/// `CapturedMeasure` lays out from. One line per face, size (half points) and style.
fn capture(dc: Handle) -> String {
	use quill_core::layout::{FontRef, Measure};
	let mut out = String::from(
		"# quill captured metrics 1: face|half points|flags (1 bold, 2 italic)|ascent|descent|leading|widths of 32 to 255\n",
	);
	let mut m = GdiMeasure {
		dc,
		grid: false,
		dpi: 96,
	};
	for (face, family) in [("Times New Roman", 1u8), ("Arial", 2), ("Courier New", 3)] {
		for size in [16u16, 18, 20, 22, 24, 28, 32, 36, 40, 48] {
			for flags in 0..4u16 {
				let mut f = [0u8; 31];
				for (d, s) in f.iter_mut().zip(face.bytes()) {
					*d = s;
				}
				let fr = FontRef {
					face: f,
					face_len: face.len() as u8,
					family,
					size,
					flags,
				};
				let met = m.metrics(&fr);
				let mut w = [0u16; 224];
				m.widths(&fr, &mut w);
				let _ = write!(out, "{face}|{size}|{flags}|{}|{}|{}|", met.ascent, met.descent, met.leading);
				for (i, x) in w.iter().enumerate() {
					let _ = write!(out, "{}{x}", if i == 0 { "" } else { "," });
				}
				out.push('\n');
			}
		}
	}
	out
}

fn args() -> Vec<Vec<u8>> {
	// SAFETY: GetCommandLineA returns a NUL-terminated string that lives as long as the process.
	let mut p = unsafe { GetCommandLineA() };
	let mut all = Vec::new();
	let mut cur = Vec::new();
	let mut quoted = false;
	loop {
		// SAFETY: reading up to the terminating NUL.
		let b = unsafe { *p };
		if b == 0 {
			break;
		}
		match b {
			b'"' => quoted = !quoted,
			b' ' if !quoted => {
				if !cur.is_empty() {
					all.push(core::mem::take(&mut cur));
				}
			}
			_ => cur.push(b),
		}
		// SAFETY: stays inside the string.
		p = unsafe { p.add(1) };
	}
	if !cur.is_empty() {
		all.push(cur);
	}
	all
}

#[no_mangle]
pub extern "C" fn start() -> ! {
	let argv = args();
	let mut store = HeapStore::new();
	// SAFETY: the screen's device context lives for the whole process and is only used from this thread.
	let screen_dc = unsafe { GetDC(null_mut()) };
	let text_view = argv
		.iter()
		.skip(1)
		.any(|a| a.eq_ignore_ascii_case(b"/TEXT") || a.eq_ignore_ascii_case(b"/PLAY"));
	let mut app = alloc::boxed::Box::new(App::with_measure(GdiMeasure {
		dc: screen_dc,
		grid: text_view,
		dpi: 96,
	}));
	let _ = app.reset(&mut store);
	// The windowed build never draws a dialog in text cells (D86); the text view that the golden runs use keeps them.
	app.set_native_dialogs(!text_view);
	let mut files = Win32Files;
	if text_view {
		app.load_settings(&mut files);
	} else {
		// The Windows build keeps its recent files apart from the Win16 build's, in the Windows directory's QUILL.INI.
		app.set_recent_section("Recent32");
	}
	app.set_about(if text_view { "Host test run" } else { "Windows 9x, Win32" });
	let mut play_file: Option<Vec<u8>> = None;
	let mut dump_file: Option<Vec<u8>> = None;
	let mut hold = false;
	let mut print_now = false;
	let mut preview_start = false;
	let mut print_to: Option<Vec<u8>> = None;
	let mut pdf_to: Option<Vec<u8>> = None;
	let mut it = argv.iter().skip(1);
	let mut scripted_open: Option<Vec<u8>> = None;
	while let Some(a) = it.next() {
		let up = a.to_ascii_uppercase();
		match up.as_slice() {
			b"/MONO" => app.set_scheme(Scheme::Mono),
			b"/HOLD" => hold = true,
			b"/PLAY" => play_file = it.next().cloned(),
			b"/DUMP" => dump_file = it.next().cloned(),
			b"/P" => print_now = true,
			b"/PREVIEW" => preview_start = true,
			b"/PO" => print_to = it.next().cloned(),
			b"/PDF" => pdf_to = it.next().cloned(),
			b"/LINES" | b"/CAPTURE" => {
				it.next();
			}
			_ if up.first() == Some(&b'/') => {}
			_ => scripted_open = Some(a.clone()),
		}
	}
	let mut keys = Vec::new();
	if let Some(pf) = &play_file {
		let text = read_whole(pf);
		let text = String::from_utf8_lossy(&text).into_owned();
		for line in text.lines() {
			let line = line.split('#').next().unwrap_or("");
			let mut words = line.split_whitespace();
			while let Some(w) = words.next() {
				match w {
					"@mono" => app.set_scheme(Scheme::Mono),
					"@open" => scripted_open = words.next().map(|s| s.as_bytes().to_vec()),
					"@copy" => {
						words.next();
					}
					_ => {
						if let Some(k) = quill_app::script::parse_token(w) {
							keys.push(k);
						}
					}
				}
			}
		}
	}
	if let Some(name) = &scripted_open {
		let _ = app.open(&mut store, &mut files, name);
	}
	let dump = dump_file.and_then(|n| {
		let c = c_name(&n);
		// SAFETY: `c` is NUL-terminated.
		let h = unsafe { CreateFileA(c.as_ptr(), GENERIC_WRITE, 0, null(), CREATE_ALWAYS, 0, null_mut()) };
		(h != INVALID_HANDLE_VALUE).then_some((h, 0))
	});
	let write_text = |name: &[u8], text: &str| {
		let c = c_name(name);
		// SAFETY: `c` is NUL-terminated.
		let h = unsafe { CreateFileA(c.as_ptr(), GENERIC_WRITE, 0, null(), CREATE_ALWAYS, 0, null_mut()) };
		if h != INVALID_HANDLE_VALUE {
			let mut out = Win32Sink(h);
			out.write(text.as_bytes());
			// SAFETY: the handle is ours.
			unsafe { CloseHandle(h) };
		}
	};
	if let Some(name) = argv
		.iter()
		.skip(1)
		.position(|a| a.eq_ignore_ascii_case(b"/CAPTURE"))
		.and_then(|i| argv.get(i + 2))
	{
		write_text(name, &capture(screen_dc));
		// SAFETY: ends the process.
		unsafe { ExitProcess(0) }
	}
	if let Some(name) = argv
		.iter()
		.skip(1)
		.position(|a| a.eq_ignore_ascii_case(b"/LINES"))
		.and_then(|i| argv.get(i + 2))
	{
		let mut out = String::new();
		let mut m = GdiMeasure {
			dc: screen_dc,
			grid: false,
			dpi: 96,
		};
		let _ = quill_core::layout::dump_story(
			app.doc(),
			&store,
			&mut m,
			"windows-96",
			quill_core::doc::StoryKind::Body,
			&mut out,
		);
		write_text(name, &out);
		// SAFETY: ends the process.
		unsafe { ExitProcess(0) }
	}
	let playing = play_file.is_some();
	let sh = alloc::boxed::Box::new(Shell {
		app,
		grid: text_view,
		cmds: Vec::new(),
		store,
		screen_dc,
		pics: gdi::PicCache::new(),
		preview: None,
		combos: None,
		faces: Vec::new(),
		instance: null_mut(),
		ruler_kind: 0,
		drag: None,
		files,
		cw: 8,
		ch: 16,
		dump,
		hold: hold && playing || (!playing),
		hscroll: 0,
		vpos: 0,
		pages: 1,
		saved_vpos: 0,
		last_view: 0,
		click: None,
		sel: None,
		pic_drag: None,
		hover: ui::Hover::None,
		hint_on: false,
		pressed: None,
		tips: None,
		pv_btns: [null_mut(); 6],
		wheel_msg: 0,
		recent_rule: false,
		sb: null_mut(),
		status_h: 0,
		sb_text: Default::default(),
		sb_proc: 0,
		pv_pages: 1,
		vbar: null_mut(),
		hbar: null_mut(),
		vbar_on: false,
		hbar_on: false,
		back: None,
		spell_view: true,
		spell_chg: (0, 0),
		spell_since: 0,
		bar_placed: [None; 2],
		bar_info: [None; 2],
		title: Vec::new(),
		blink: ui::Blink::new(false),
		blocked: false,
	});
	// SAFETY: single-threaded start-up; the boxes live for the whole process.
	unsafe {
		SHELL = alloc::boxed::Box::into_raw(sh);
		if print_now || print_to.is_some() {
			if let Some(s) = SHELL.as_mut() {
				ui::refresh_clock(s);
			}
			// `/P FILE` prints on the default printer and exits; `/PO OUT` sends the job to a file instead of the port.
			let ok = SHELL
				.as_ref()
				.is_some_and(|s| print_job(null_mut(), s, false, print_to.as_deref()));
			ExitProcess(if ok { 0 } else { 1 });
		}
		// `/PDF OUT` writes the document as a PDF and exits.
		if let Some(out) = &pdf_to {
			if let Some(s) = SHELL.as_mut() {
				ui::refresh_clock(s);
				let ok = pdf::write_document(s, out);
				ExitProcess(if ok { 0 } else { 1 });
			}
		}
		if playing {
			PLAY = alloc::boxed::Box::into_raw(alloc::boxed::Box::new(Play {
				keys,
				at: 0,
				frames: String::new(),
				n: 0,
			}));
		}
		let instance = GetModuleHandleA(null());
		let class = WndClassA {
			style: CS_OWNDC,
			wnd_proc,
			cls_extra: 0,
			wnd_extra: 0,
			instance,
			icon: LoadIconA(instance, 1 as *const u8),
			cursor: LoadCursorA(null_mut(), IDC_ARROW as *const u8),
			background: null_mut(),
			menu_name: null(),
			class_name: CLASS.as_ptr(),
		};
		RegisterClassA(&class);
		apply_scheme();
		// Measure the OEM fixed font to size the window to 80 by 25 cells.
		let dc = GetDC(null_mut());
		SelectObject(dc, GetStockObject(OEM_FIXED_FONT));
		let mut tm = MaybeUninit::<TextMetricA>::zeroed();
		GetTextMetricsA(dc, tm.as_mut_ptr());
		ReleaseDC(null_mut(), dc);
		let tm = tm.assume_init();
		if let Some(s) = SHELL.as_mut() {
			if tm.ave_char_width > 0 && tm.height > 0 {
				s.cw = tm.ave_char_width;
				s.ch = tm.height;
			}
		}
		let (cw, ch) = SHELL.as_ref().map_or((8, 16), |s| (s.cw, s.ch));
		let gridview = text_view;
		let style = if gridview {
			WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX | WS_VISIBLE
		} else {
			WS_OVERLAPPEDWINDOW | WS_VISIBLE | WS_CLIPCHILDREN
		};
		let menu = match SHELL.as_mut() {
			Some(s) if !gridview => build_menu(s, instance),
			_ => null_mut(),
		};
		let mut r = if gridview {
			Rect {
				left: 0,
				top: 0,
				right: 80 * cw,
				bottom: 25 * ch,
			}
		} else {
			Rect {
				left: 0,
				top: 0,
				right: PREFERRED_W,
				bottom: PREFERRED_H + ch,
			}
		};
		AdjustWindowRect(&mut r, style, i32::from(!menu.is_null()));
		let (mut x, mut y, mut w, mut h) = (20, 20, r.right - r.left, r.bottom - r.top);
		let mut style = style;
		if !gridview {
			// The window goes in the middle of the work area (the screen less the taskbar), and when it will not fit there it opens
			// maximised, so a 640 by 480 or 800 by 600 screen shows the whole of it.
			let wa = work_area();
			let (aw, ah) = (wa.right - wa.left, wa.bottom - wa.top);
			if w > aw || h > ah {
				style |= WS_MAXIMIZE;
				w = w.min(aw);
				h = h.min(ah);
			}
			x = wa.left + (aw - w) / 2;
			y = wa.top + (ah - h) / 2;
		}
		let hwnd = CreateWindowExA(
			0,
			CLASS.as_ptr(),
			b"Quill - (Untitled)\0".as_ptr(),
			style,
			x,
			y,
			w,
			h,
			null_mut(),
			menu,
			instance,
			null(),
		);
		if let Some(sh) = shell() {
			retitle(hwnd, sh);
			if !gridview {
				sh.faces = chrome::enumerate_faces(sh.screen_dc);
				sh.instance = instance;
				// Windows 95 checks spelling as you type unless QUILL.INI says otherwise.
				sh.app.set_spell_live_default(true);
				ui::ini_load(sh);
				// A scripted run, `QUILL_NO_BLINK` in the environment and `[Quill] Blink=0` keep the caret steady.
				sh.blink.allowed = sh.blink.allowed && !playing && !ui::env_set(b"QUILL_NO_BLINK\0");
				#[cfg(feature = "bench")]
				bench::init();
				let cb = chrome::create_combos(hwnd, instance, &sh.faces);
				cb.show(&sh.app, false);
				sh.combos = Some(cb);
				sh.pv_btns = ui::create_preview_buttons(hwnd, instance);
				(sh.vbar, sh.hbar) = ui::create_bars(hwnd, instance);
				sh.sb = ui::create_status(hwnd, instance);
				ui::status_hook(sh);
				ui::status_layout(hwnd, sh);
				sh.wheel_msg = RegisterWindowMessageA(b"MSWHEEL_ROLLMSG\0".as_ptr());
				sh.tips = tips::Tips::new(hwnd, instance);
				sync_chrome(hwnd, sh);
				ui::update_scroll(hwnd, sh);
				ui::blink_restart(hwnd, sh);
				if preview_start {
					ui::preview_open(hwnd, sh, 1, false);
				}
			}
		}
		if playing {
			SetTimer(hwnd, 1, 200, null());
		}
		let mut msg = MaybeUninit::<Msg>::uninit();
		while GetMessageA(msg.as_mut_ptr(), null_mut(), 0, 0) > 0 {
			TranslateMessage(msg.as_ptr());
			DispatchMessageA(msg.as_ptr());
		}
		ExitProcess(0)
	}
}
