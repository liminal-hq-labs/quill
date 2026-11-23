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

mod rt;
mod sys;

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write;
use core::mem::MaybeUninit;
use core::ptr::{null, null_mut};

use quill_app::{App, Dir, Files, Key, Scheme};
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

impl Files for Win32Files {
	type R = Win32Source;
	type W = Win32Sink;

	fn open(&mut self, name: &[u8]) -> Option<Win32Source> {
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
		(h != INVALID_HANDLE_VALUE).then_some(Win32Source(h))
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
	app: alloc::boxed::Box<App>,
	store: HeapStore,
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
		WM_DESTROY => {
			PostQuitMessage(0);
			0
		}
		_ => DefWindowProcA(hwnd, msg, wparam, lparam),
	}
}

unsafe fn press(hwnd: Handle, k: Key) {
	if let Some(sh) = shell() {
		sh.app.key(&mut sh.store, &mut sh.files, k);
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
	for y in 0..25usize {
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
	let mut app = alloc::boxed::Box::new(App::new());
	let _ = app.reset(&mut store);
	let mut files = Win32Files;
	let mut play_file: Option<Vec<u8>> = None;
	let mut dump_file: Option<Vec<u8>> = None;
	let mut hold = false;
	let mut it = argv.iter().skip(1);
	let mut scripted_open: Option<Vec<u8>> = None;
	while let Some(a) = it.next() {
		let up = a.to_ascii_uppercase();
		match up.as_slice() {
			b"/MONO" => app.set_scheme(Scheme::Mono),
			b"/HOLD" => hold = true,
			b"/PLAY" => play_file = it.next().cloned(),
			b"/DUMP" => dump_file = it.next().cloned(),
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
	let playing = play_file.is_some();
	let sh = alloc::boxed::Box::new(Shell {
		app,
		store,
		files,
		cw: 8,
		ch: 16,
		dump,
		hold: hold && playing || (!playing),
	});
	// SAFETY: single-threaded start-up; the boxes live for the whole process.
	unsafe {
		SHELL = alloc::boxed::Box::into_raw(sh);
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
			icon: null_mut(),
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
		let style = WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX | WS_VISIBLE;
		let mut r = Rect {
			left: 0,
			top: 0,
			right: 80 * cw,
			bottom: 25 * ch,
		};
		AdjustWindowRect(&mut r, style, 0);
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
			null_mut(),
			instance,
			null(),
		);
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
