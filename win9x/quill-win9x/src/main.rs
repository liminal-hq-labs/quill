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

mod chrome;
mod dialogs;
mod gdi;
mod rt;
mod sys;

use alloc::string::String;
use alloc::vec::Vec;
use core::ffi::c_void;
use core::fmt::Write;
use core::mem::MaybeUninit;
use core::ptr::{null, null_mut};

use gdi::GdiMeasure;
use quill_app::{App, Command, Dir, Files, Key, Scheme};
use quill_formats::io::{Sink, Source};
use quill_grid::Grid;
use quill_heap::HeapStore;
use sys::*;

const CLASS: &[u8] = b"Quill32\0";

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
		// SAFETY: the handle is ours.
		unsafe { CloseHandle(w.0) };
		true
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
	/// The print preview, when it is showing: the first page shown, and whether two pages are.
	preview: Option<(u32, bool)>,
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

unsafe extern "system" fn wnd_proc(hwnd: Handle, msg: u32, wparam: usize, lparam: isize) -> isize {
	match msg {
		WM_PAINT => {
			paint(hwnd);
			0
		}
		WM_KEYDOWN => {
			let shift = GetKeyState(VK_SHIFT) < 0;
			let ctrl = GetKeyState(VK_CONTROL) < 0;
			if let Some(k) = map_vk(wparam, shift, ctrl) {
				press(hwnd, k);
			}
			0
		}
		WM_CHAR => {
			let c = wparam as u8;
			let k = match c {
				0x0D => Some(Key::Enter),
				0x08 => Some(Key::Backspace),
				0x09 => Some(Key::Tab),
				0x1B => Some(Key::Esc),
				1..=26 => Some(Key::Ctrl(c + 0x60)),
				0x20..=0xFF => Some(Key::Char(c)),
				_ => None,
			};
			if let Some(k) = k {
				press(hwnd, k);
			}
			0
		}
		WM_TIMER => {
			play(hwnd);
			0
		}
		WM_ERASEBKGND => 1,
		WM_COMMAND => {
			if lparam != 0 {
				combo_changed(hwnd, wparam);
			} else {
				menu_command(hwnd, wparam & 0xFFFF);
			}
			0
		}
		0x0201 => {
			mouse_down(hwnd, (lparam & 0xFFFF) as i16 as i32, (lparam >> 16) as i16 as i32);
			0
		}
		0x0200 => {
			mouse_move(hwnd, (lparam & 0xFFFF) as i16 as i32);
			0
		}
		0x0202 => {
			mouse_up(hwnd, (lparam & 0xFFFF) as i16 as i32);
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
		WM_DESTROY => {
			PostQuitMessage(0);
			0
		}
		_ => DefWindowProcA(hwnd, msg, wparam, lparam),
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
		for it in items {
			let mut text: Vec<u8> = it.label.chars().map(|c| c as u32 as u8).collect();
			if !it.accel.is_empty() {
				text.push(b'\t');
				text.extend_from_slice(it.accel.as_bytes());
			}
			text.push(0);
			sh.cmds.push(it.command);
			if !pop.is_null() {
				AppendMenuA(pop, MF_STRING, 99 + sh.cmds.len(), text.as_ptr());
			}
		}
		if !pop.is_null() {
			let mut title = quill_app::menu_title(m).to_vec();
			title.push(0);
			AppendMenuA(bar, MF_POPUP, pop as usize, title.as_ptr());
		}
	}
	if from_res.is_null() {
		bar
	} else {
		from_res
	}
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
		filter_index: 1,
		file: file.as_mut_ptr(),
		max_file: 260,
		title_file: null_mut(),
		max_title: 0,
		initial_dir: if dir.len() > 1 { dir.as_ptr() } else { null() },
		title: title.as_ptr(),
		// OFN_EXPLORER | OFN_HIDEREADONLY | OFN_PATHMUSTEXIST, then OFN_OVERWRITEPROMPT (save) or OFN_FILEMUSTEXIST (open)
		flags: 0x0008_0000 | 0x0004 | 0x0800 | if kind == FileKind::Save { 0x0002 } else { 0x1000 },
		file_offset: 0,
		file_ext: 0,
		def_ext: if kind == FileKind::Save { b"qld\0".as_ptr() } else { null() },
		cust_data: 0,
		hook: null(),
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
	let n = file.iter().position(|b| *b == 0).unwrap_or(0);
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

/// Shows every dialog the editor has asked for, as a real dialog (D86), and applies the answers.
unsafe fn handle_dialogs(hwnd: Handle) {
	while let Some(sh) = shell() {
		let Some(d) = sh.app.take_dialog() else {
			return;
		};
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
	match c {
		Command::Preview if !sh.grid => sh.preview = Some((1, false)),
		Command::PrintSetup if !sh.grid => {
			// The common dialog's setup page: choose the printer and paper.
			print_dialog(hwnd, 0x0040);
		}
		_ => sh.app.command(&mut sh.store, &mut sh.files, c),
	}
	if !sh.grid {
		handle_dialogs(hwnd);
	}
	if let Command::Chrome(_) = c {
		sync_chrome(hwnd, sh);
	}
	retitle(hwnd, sh);
	InvalidateRect(hwnd, null(), 0);
	if sh.app.quit() {
		DestroyWindow(hwnd);
	}
}

/// A face or size was chosen in one of the boxes.
unsafe fn combo_changed(hwnd: Handle, wparam: usize) {
	let Some(sh) = shell() else {
		return;
	};
	let (id, code) = (wparam & 0xFFFF, wparam >> 16);
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

/// Where the ruler is and where the text's left edge is, for the mouse.
unsafe fn ruler_geometry(hwnd: Handle, sh: &Shell) -> (i32, i32, i32) {
	let mut r = Rect {
		left: 0,
		top: 0,
		right: 0,
		bottom: 0,
	};
	GetClientRect(hwnd, &mut r);
	let (left, w) = chrome::text_extent(&sh.app, r.right);
	let (h0, h1) = chrome::rows(&sh.app);
	let top = h0 + h1;
	(top, left, w)
}

unsafe fn mouse_down(hwnd: Handle, x: i32, y: i32) {
	let Some(sh) = shell() else {
		return;
	};
	if sh.preview.is_some() || sh.grid {
		return;
	}
	{
		if let Some((b, row_top)) = chrome::button_at(&sh.app, x, y) {
			match b.act {
				chrome::Act::Cmd(c) => run_command(hwnd, c),
				chrome::Act::Colour => {
					if let Some(choice) = chrome::colour_menu(hwnd, b.x, row_top + chrome::TB_H) {
						sh.app.set_colour(&mut sh.store, choice);
					}
				}
				chrome::Act::None => {
					run_command(hwnd, Command::Spelling);
					return;
				}
			}
			InvalidateRect(hwnd, null(), 0);
			return;
		}
	}
	if sh.app.show_ruler() {
		let (top, left, w) = ruler_geometry(hwnd, sh);
		let st = chrome::RulerState::of(&sh.app, &sh.store);
		match chrome::ruler_hit(x, y, top, left, w, &st) {
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
		InvalidateRect(hwnd, null(), 0);
	}
}

unsafe fn mouse_move(hwnd: Handle, x: i32) {
	let Some(sh) = shell() else {
		return;
	};
	if let Some(mut d) = sh.drag {
		let (_, left, w) = ruler_geometry(hwnd, sh);
		chrome::drag_to(&mut d, x, left, w);
		sh.drag = Some(d);
		InvalidateRect(hwnd, null(), 0);
	}
}

unsafe fn mouse_up(hwnd: Handle, x: i32) {
	let Some(sh) = shell() else {
		return;
	};
	let Some(mut d) = sh.drag.take() else {
		return;
	};
	ReleaseCapture();
	let (_, left, w) = ruler_geometry(hwnd, sh);
	chrome::drag_to(&mut d, x, left, w);
	match d.hit {
		chrome::Hit::First | chrome::Hit::Left | chrome::Hit::Both | chrome::Hit::Right => {
			sh.app
				.set_indents(&mut sh.store, d.state.left, d.state.first - d.state.left, d.state.right);
		}
		_ => {}
	}
	InvalidateRect(hwnd, null(), 0);
}

/// Shows or hides the combo boxes with the toolbar, ticks the menu items and lays the page out again.
unsafe fn sync_chrome(hwnd: Handle, sh: &mut Shell) {
	if let Some(cb) = &sh.combos {
		cb.show(&sh.app);
	}
	let menu = GetMenu(hwnd);
	if !menu.is_null() {
		for (i, c) in sh.cmds.iter().enumerate() {
			if let Command::Chrome(bit) = c {
				let on = sh.app.chrome_on(*bit);
				CheckMenuItem(menu, 100 + i as u32, if on { 0x8 } else { 0 });
			}
		}
	}
	InvalidateRect(hwnd, null(), 0);
}

unsafe fn retitle(hwnd: Handle, sh: &Shell) {
	let mut t = b"Quill - ".to_vec();
	if sh.app.name().is_empty() {
		t.extend_from_slice(b"(Untitled)");
	} else {
		t.extend_from_slice(sh.app.name());
	}
	t.push(0);
	SetWindowTextA(hwnd, t.as_ptr());
}

unsafe fn press(hwnd: Handle, k: Key) {
	if let Some(sh) = shell() {
		if let Some((first, two)) = sh.preview {
			let (_, _, pages) = gdi::preview_pages(&sh.app, &sh.store, sh.screen_dc);
			let step = if two { 2 } else { 1 };
			match k {
				Key::Esc => sh.preview = None,
				Key::PageDown | Key::Down | Key::Right if first + step <= pages => sh.preview = Some((first + step, two)),
				Key::PageUp | Key::Up | Key::Left => sh.preview = Some((first.saturating_sub(step).max(1), two)),
				Key::Home | Key::CtrlHome => sh.preview = Some((1, two)),
				Key::End | Key::CtrlEnd => sh.preview = Some((pages, two)),
				Key::Char(b'T' | b't') => sh.preview = Some((first, !two)),
				Key::Char(b'P' | b'p') | Key::Ctrl(b'p') => {
					if !print_job(hwnd, sh, true, None) {
						sh.app.set_message("Quill could not print.");
					}
				}
				_ => {}
			}
			InvalidateRect(hwnd, null(), 0);
			return;
		}
		sh.app.key(&mut sh.store, &mut sh.files, k);
		if !sh.grid {
			handle_dialogs(hwnd);
		}
		retitle(hwnd, sh);
		InvalidateRect(hwnd, null(), 0);
		if sh.app.quit() {
			DestroyWindow(hwnd);
		}
	}
}

unsafe fn paint(hwnd: Handle) {
	let Some(sh) = shell() else {
		return;
	};
	let mut ps = MaybeUninit::<PaintStruct>::uninit();
	let dc = BeginPaint(hwnd, ps.as_mut_ptr());
	SelectObject(dc, GetStockObject(OEM_FIXED_FONT));
	let mut g = Grid::<25>::new(0);
	quill_app::draw(&sh.app, &sh.store, &mut g);
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
		let (h0, h1) = chrome::rows(&sh.app);
		let tb = if sh.preview.is_none() { h0 + h1 } else { 0 };
		let rl = if sh.app.show_ruler() && sh.preview.is_none() {
			chrome::RULER_H
		} else {
			0
		};
		let status = if sh.app.show_status() { sh.ch } else { 0 };
		let area = Rect {
			left: 0,
			top: tb + rl,
			right: r.right,
			bottom: r.bottom - status,
		};
		if sh.preview.is_none() {
			if tb > 0 {
				chrome::paint_toolbar(dc, r.right, &sh.app, &sh.store);
				if let Some(cb) = sh.combos.as_mut() {
					cb.sync(&sh.app, &sh.store);
				}
			}
			if rl > 0 {
				let (left, w) = chrome::text_extent(&sh.app, r.right);
				let st = sh
					.drag
					.map_or_else(|| chrome::RulerState::of(&sh.app, &sh.store), |d| d.state);
				chrome::paint_ruler(dc, tb, r.right, left, w, &st, sh.ruler_kind);
			}
		}
		if let Some((first, two)) = sh.preview {
			let pages = gdi::paint_preview(dc, area, &sh.app, &sh.store, sh.screen_dc, first, two, &mut sh.pics);
			SelectObject(dc, GetStockObject(OEM_FIXED_FONT));
			let mut line = alloc::format!(
				"Print Preview: page {first} of {pages}   PgUp PgDn: page   T: one or two pages   P: print   Esc: close"
			)
			.into_bytes();
			line.truncate(200);
			SetBkMode(dc, OPAQUE);
			SetBkColor(dc, 0x00AA_AAAA);
			SetTextColor(dc, 0);
			ExtTextOutA(
				dc,
				0,
				r.bottom - sh.ch,
				ETO_OPAQUE,
				&Rect {
					left: 0,
					top: r.bottom - sh.ch,
					right: r.right,
					bottom: r.bottom,
				},
				line.as_ptr(),
				line.len() as u32,
				null(),
			);
			EndPaint(hwnd, ps.as_ptr());
			return;
		}
		let rows = gdi::paint_page(dc, area, &sh.app, &sh.store, &mut sh.pics);
		SelectObject(dc, GetStockObject(OEM_FIXED_FONT));
		sh.app.set_rows(rows);
		if !sh.app.editing() {
			draw_rows(dc, &g, 7, 21, sh, (r.right - 80 * sh.cw).max(0) / 2, 40);
		}
		if status > 0 {
			draw_rows(dc, &g, 24, 25, sh, 0, r.bottom - sh.ch);
		}
		EndPaint(hwnd, ps.as_ptr());
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
	EndPaint(hwnd, ps.as_ptr());
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
	app.load_settings(&mut files);
	app.set_about(if text_view { "Host test run" } else { "Windows 9x, Win32" });
	let mut play_file: Option<Vec<u8>> = None;
	let mut dump_file: Option<Vec<u8>> = None;
	let mut hold = false;
	let mut print_now = false;
	let mut preview_start = false;
	let mut print_to: Option<Vec<u8>> = None;
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
		preview: preview_start.then_some((1, false)),
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
	});
	// SAFETY: single-threaded start-up; the boxes live for the whole process.
	unsafe {
		SHELL = alloc::boxed::Box::into_raw(sh);
		if print_now || print_to.is_some() {
			// `/P FILE` prints on the default printer and exits; `/PO OUT` sends the job to a file instead of the port.
			let ok = SHELL
				.as_ref()
				.is_some_and(|s| print_job(null_mut(), s, false, print_to.as_deref()));
			ExitProcess(if ok { 0 } else { 1 });
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
			WS_OVERLAPPEDWINDOW | WS_VISIBLE
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
				right: 860,
				bottom: 600 + ch,
			}
		};
		AdjustWindowRect(&mut r, style, i32::from(!menu.is_null()));
		let hwnd = CreateWindowExA(
			0,
			CLASS.as_ptr(),
			b"Quill - (Untitled)\0".as_ptr(),
			style,
			20,
			20,
			r.right - r.left,
			r.bottom - r.top,
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
				let cb = chrome::create_combos(hwnd, instance, &sh.faces);
				cb.show(&sh.app);
				sh.combos = Some(cb);
				sync_chrome(hwnd, sh);
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
