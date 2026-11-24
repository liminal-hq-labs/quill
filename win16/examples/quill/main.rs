// Quill for Windows 3.x: the editor controller on the Win16 runtime, in the text view (the DOS screen drawn in the OEM font).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// `QUILL.EXE [FILE]` opens a file. `/PLAY KEYS.TXT /DUMP OUT.TXT` plays a key script and writes every screen, as the other builds do, and
// ends Windows when it is done; with `/PLAY` names are looked for beside the program. The code is split over several segments (see
// `segments.toml`); the document is in global memory (`store.rs`).
#![no_std]
#![no_main]

mod store;

use core::mem::MaybeUninit;
use quill_app::{App, Command, Dir, Files, Key, Scheme};
use quill_formats::io::{Sink, Source};
use quill_grid::Grid;
use store::FarStore;
use w16::*;

export_proc!(QuillWndProc, wnd_proc);

const WM_KEYDOWN: u32 = 0x0100;
const WM_CHAR: u32 = 0x0102;
const WM_TIMER: u32 = 0x0113;
const WM_ERASEBKGND: u32 = 0x0014;
const WS_OVERLAPPEDWINDOW: u32 = 0x00CF_0000;
const CW_USEDEFAULT: u32 = 0x8000;
const OEM_FIXED_FONT: u32 = 10;
const MF_STRING: u32 = 0;
const MF_POPUP: u32 = 0x10;
const VGA: [u32; 16] = [
	0x000000, 0xAA0000, 0x00AA00, 0xAAAA00, 0x0000AA, 0xAA00AA, 0x0055AA, 0xAAAAAA, 0x555555,
	0xFF5555, 0x55FF55, 0xFFFF55, 0x5555FF, 0xFF55FF, 0x55FFFF, 0xFFFFFF,
];

static mut STORE: FarStore = FarStore::new();
static mut APP: MaybeUninit<App> = MaybeUninit::uninit();
static mut CMDS: [Command; 64] = [Command::New; 64];
static mut NCMDS: usize = 0;
static mut CELL: (u32, u32) = (8, 16);
/// The program's own directory, ending in a backslash, when names should be looked for there.
static mut HERE: [u8; 100] = [0; 100];
static mut HERE_LEN: usize = 0;
static mut USE_HERE: bool = false;

fn app() -> &'static mut App {
	unsafe { (*core::ptr::addr_of_mut!(APP)).assume_init_mut() }
}

fn store() -> &'static mut FarStore {
	unsafe { &mut *core::ptr::addr_of_mut!(STORE) }
}

// ---- files ---------------------------------------------------------------------------------------------------------------------

struct Win16Files;
struct FileSource(u32);
struct FileSink(u32);

/// `name` with the program's directory in front when that is on, and a zero at the end.
fn c_name(name: &[u8], out: &mut [u8; 128]) -> usize {
	let mut n = 0;
	let here = unsafe { &*core::ptr::addr_of!(HERE) };
	let hl = unsafe { HERE_LEN };
	if unsafe { USE_HERE } && hl > 0 && !name.iter().any(|b| *b == b'\\' || *b == b':') {
		for b in &here[..hl] {
			out[n] = *b;
			n += 1;
		}
	}
	for b in name {
		if n + 1 >= out.len() {
			break;
		}
		out[n] = *b;
		n += 1;
	}
	out[n] = 0;
	n
}

impl Source for FileSource {
	fn read(&mut self, out: &mut [u8]) -> usize {
		let n = out.len().min(0x7FFF);
		let got = unsafe {
			_lread(
				self.0,
				far(data_selector(), out.as_mut_ptr() as u32),
				n as u32,
			)
		};
		if got == 0xFFFF { 0 } else { got as usize }
	}
}

impl Drop for FileSource {
	fn drop(&mut self) {
		unsafe { _lclose(self.0) };
	}
}

impl Sink for FileSink {
	fn write(&mut self, data: &[u8]) -> bool {
		let mut at = 0;
		while at < data.len() {
			let n = (data.len() - at).min(0x7000);
			let put = unsafe { _lwrite(self.0, data[at..].as_ptr() as u32, n as u32) };
			if put as usize != n {
				return false;
			}
			at += n;
		}
		true
	}
}

impl Files for Win16Files {
	type R = FileSource;
	type W = FileSink;

	fn open(&mut self, name: &[u8]) -> Option<FileSource> {
		let mut buf = [0u8; 128];
		c_name(name, &mut buf);
		let h = unsafe { _lopen(buf.as_ptr() as u32, 0) };
		(h != HFILE_ERROR).then_some(FileSource(h))
	}

	fn create(&mut self, name: &[u8]) -> Option<FileSink> {
		let mut buf = [0u8; 128];
		c_name(name, &mut buf);
		let h = unsafe { _lcreat(buf.as_ptr() as u32, 0) };
		(h != HFILE_ERROR).then_some(FileSink(h))
	}

	fn close(&mut self, w: FileSink) -> bool {
		unsafe { _lclose(w.0) };
		true
	}
}

// ---- the screen ----------------------------------------------------------------------------------------------------------------

fn paint(hwnd: u32) {
	unsafe {
		let mut ps = core::mem::zeroed::<PaintStruct>();
		let dc = BeginPaint(hwnd, &mut ps as *mut PaintStruct as u32);
		SelectObject(dc, GetStockObject(OEM_FIXED_FONT));
		let mut g = Grid::<25>::new(0);
		quill_app::draw(app(), store(), &mut g);
		let (cw, ch) = CELL;
		for y in 0..25usize {
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
					run[k] = if c.ch == 0 { b' ' } else { c.ch };
					k += 1;
				}
				SetTextColor(dc, VGA[usize::from(a & 15)]);
				SetBkColor(dc, VGA[usize::from(a >> 4)]);
				TextOut(
					dc,
					x as u32 * cw,
					y as u32 * ch,
					run.as_ptr() as u32,
					k as u32,
				);
				x += k;
			}
		}
		EndPaint(hwnd, &mut ps as *mut PaintStruct as u32);
	}
}

fn press(hwnd: u32, k: Key) {
	app().key(store(), &mut Win16Files, k);
	after(hwnd);
}

/// Opens QUILL.HLP (beside the program) in WinHelp at its contents.
fn open_help(hwnd: u32) {
	const HELP_CONTENTS: u32 = 3;
	let mut path = [0u8; 120];
	unsafe {
		let here = &*core::ptr::addr_of!(HERE);
		let n = HERE_LEN.min(here.len());
		path[..n].copy_from_slice(&here[..n]);
		let name = b"QUILL.HLP\0";
		if n + name.len() <= path.len() {
			path[n..n + name.len()].copy_from_slice(name);
			WinHelp(hwnd, path.as_ptr() as u32, HELP_CONTENTS, 0);
		}
	}
}

fn after(hwnd: u32) {
	unsafe {
		if app().take_help() {
			open_help(hwnd);
		}
		InvalidateRect(hwnd, 0, 1);
		if app().quit() {
			DestroyWindow(hwnd);
		}
	}
}

fn map_vk(vk: u32, shift: bool, ctrl: bool) -> Option<Key> {
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

// ---- the script ----------------------------------------------------------------------------------------------------------------

/// The script text lives in a block of global memory, read a word at a time, so the data segment keeps its room for the stack.
struct Play {
	text: u32,
	len: usize,
	at: usize,
	frames: u32,
	out: u32,
	staged: usize,
}

static mut PLAY: Option<Play> = None;

const STAGE_LEN: usize = 512;
static mut STAGE: [u8; STAGE_LEN] = [0; STAGE_LEN];

impl Play {
	fn put(&mut self, data: &[u8]) {
		for &b in data {
			if self.staged == STAGE_LEN {
				self.flush();
			}
			unsafe { STAGE[self.staged] = b };
			self.staged += 1;
		}
	}

	fn flush(&mut self) {
		if self.staged > 0 {
			unsafe {
				_lwrite(
					self.out,
					core::ptr::addr_of!(STAGE) as u32,
					self.staged as u32,
				)
			};
			self.staged = 0;
		}
	}

	fn snapshot(&mut self) {
		let mut g = Grid::<25>::new(0);
		quill_app::draw(app(), store(), &mut g);
		self.frames += 1;
		let n = self.frames;
		g.dump_bytes(n, &mut |piece| self.put(piece));
	}

	fn byte(&self, at: usize) -> u8 {
		if at >= self.len {
			return 0;
		}
		far_byte(self.text, at as u32)
	}

	/// The next word of the script (comments and blanks skipped) in `w`, and its length; 0 at the end.
	fn word(&mut self, w: &mut [u8; 24]) -> usize {
		loop {
			let b = self.byte(self.at);
			if self.at >= self.len {
				return 0;
			}
			if b == b'#' {
				while self.at < self.len && self.byte(self.at) != b'\n' {
					self.at += 1;
				}
			} else if b <= b' ' {
				self.at += 1;
			} else {
				break;
			}
		}
		let mut n = 0;
		while self.at < self.len {
			let b = self.byte(self.at);
			if b <= b' ' {
				break;
			}
			if n < w.len() {
				w[n] = b;
				n += 1;
			}
			self.at += 1;
		}
		n
	}
}

static mut CRUMBS_MADE: bool = false;

/// A breadcrumb for debugging: the text, and the segment registers, stack pointer, flags and the high half of ESP, appended to
/// CRUMBS.LOG with the file closed again at once (an unflushed file is lost when the task faults).
fn crumb(what: &[u8]) {
	unsafe {
		let mut buf = [0u8; 128];
		c_name(b"CRUMBS.LOG", &mut buf);
		let h = if CRUMBS_MADE {
			let h = _lopen(buf.as_ptr() as u32, 1);
			if h != HFILE_ERROR {
				_llseek(h, 0, 2);
			}
			h
		} else {
			CRUMBS_MADE = true;
			_lcreat(buf.as_ptr() as u32, 0)
		};
		if h == HFILE_ERROR {
			return;
		}
		let (es, ds, ss, sp): (u16, u16, u16, u16);
		core::arch::asm!("mov {0:x}, es", "mov {1:x}, ds", "mov {2:x}, ss", "mov {3:x}, sp", out(reg) es, out(reg) ds, out(reg) ss, out(reg) sp, options(nomem, nostack));
		let fl: u16;
		core::arch::asm!("pushf", "pop {0:x}", out(reg) fl, options(nomem));
		let esp_hi: u16;
		core::arch::asm!("mov {0:e}, esp", "shr {0:e}, 16", out(reg) esp_hi, options(nomem, nostack));
		let mut line = [0u8; 64];
		let mut n = 0;
		for b in what {
			line[n] = *b;
			n += 1;
		}
		for v in [es, ds, ss, sp, fl, esp_hi] {
			line[n] = b' ';
			n += 1;
			for i in (0..4).rev() {
				let d = ((v >> (i * 4)) & 15) as u8;
				line[n] = if d < 10 { b'0' + d } else { b'A' + d - 10 };
				n += 1;
			}
		}
		line[n] = b'\n';
		n += 1;
		_lwrite(h, line.as_ptr() as u32, n as u32);
		_lclose(h);
	}
}

fn sp() -> u32 {
	let v: u16;
	unsafe { core::arch::asm!("mov {0:x}, sp", out(reg) v, options(nomem, nostack)) };
	u32::from(v)
}

fn trace(pl: &mut Play, what: &[u8], v: u32) {
	let mut line = [0u8; 40];
	let mut n = 0;
	for b in what {
		line[n] = *b;
		n += 1;
	}
	for i in (0..4).rev() {
		let d = ((v >> (i * 4)) & 15) as u8;
		line[n] = if d < 10 { b'0' + d } else { b'A' + d - 10 };
		n += 1;
	}
	line[n] = b'\n';
	n += 1;
	unsafe { _lwrite(pl.out, line.as_ptr() as u32, n as u32) };
}

fn play(hwnd: u32) {
	let Some(pl) = (unsafe { (*core::ptr::addr_of_mut!(PLAY)).as_mut() }) else {
		return;
	};
	unsafe { KillTimer(hwnd, 1) };
	if pl.frames == 0 {
		pl.snapshot();
	}
	let mut w = [0u8; 24];
	loop {
		let n = pl.word(&mut w);
		if n == 0 {
			break;
		}
		let word = &w[..n];
		if word[0] == b'@' {
			// `@open` and `@copy` take a name: the first was handled before the run, and neither is a key.
			if word == b"@open" || word == b"@copy" {
				pl.word(&mut w);
			}
			continue;
		}
		let Some(k) = core::str::from_utf8(word)
			.ok()
			.and_then(quill_app::script::parse_token)
		else {
			continue;
		};
		app().key(store(), &mut Win16Files, k);
		pl.snapshot();
		if app().quit() {
			break;
		}
	}
	app().save_settings(&mut Win16Files);
	pl.put(b"=== end ===\n");
	pl.flush();
	unsafe { _lclose(pl.out) };
	unsafe { PLAY = None };
	after(hwnd);
	unsafe { DestroyWindow(hwnd) };
}

/// Reads the script `name` into a block of global memory. Applies `@mono`, and returns the block, its length and the file `@open` names.
fn load_script(name: &[u8]) -> Option<(u32, usize, [u8; 32], usize)> {
	let mut src = Win16Files.open(name)?;
	let h = unsafe { GlobalAlloc(0x0042, 16384) } as u16;
	if h == 0 {
		return None;
	}
	let far_ptr = unsafe { GlobalLock(u32::from(h)) };
	let mut len = 0usize;
	let mut buf = [0u8; 256];
	loop {
		let got = src.read(&mut buf);
		if got == 0 || len + got > 16384 {
			break;
		}
		unsafe { far_write(far_ptr + len as u32, buf.as_ptr(), got) };
		len += got;
	}
	// A pass over the words for the directives.
	let mut pl = Play {
		text: far_ptr,
		len,
		at: 0,
		frames: 0,
		out: 0,
		staged: 0,
	};
	let mut opened = ([0u8; 32], 0usize);
	let mut w = [0u8; 24];
	loop {
		let n = pl.word(&mut w);
		if n == 0 {
			break;
		}
		if &w[..n] == b"@mono" {
			app().set_scheme(Scheme::Mono);
		} else if &w[..n] == b"@open" {
			let m = pl.word(&mut w);
			opened.0[..m].copy_from_slice(&w[..m]);
			opened.1 = m;
		}
	}
	Some((far_ptr, len, opened.0, opened.1))
}

// ---- the window ----------------------------------------------------------------------------------------------------------------

extern "C" fn wnd_proc(hwnd: u32, msg: u32, wparam: u32, lparam: u32) -> u32 {
	unsafe {
		match msg {
			WM_PAINT => {
				paint(hwnd);
				0
			}
			WM_ERASEBKGND => 1,
			WM_KEYDOWN => {
				let shift = GetKeyState(0x10) & 0x8000 != 0;
				let ctrl = GetKeyState(0x11) & 0x8000 != 0;
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
			WM_COMMAND => {
				let id = wparam as usize;
				if id >= 100 && id - 100 < NCMDS {
					let c = (*core::ptr::addr_of!(CMDS))[id - 100];
					app().command(store(), &mut Win16Files, c);
					after(hwnd);
				}
				0
			}
			WM_DESTROY => {
				PostQuitMessage(0);
				0
			}
			_ => DefWindowProc(hwnd, msg, wparam, lparam),
		}
	}
}

fn build_menu() -> u32 {
	unsafe {
		let bar = CreateMenu();
		for m in 0..quill_app::MENU_COUNT {
			let items = quill_app::menu_items(m);
			if items.is_empty() {
				continue;
			}
			let pop = CreateMenu();
			for it in items {
				let mut text = [0u8; 64];
				let mut n = 0;
				for c in it.label.chars() {
					text[n] = c as u32 as u8;
					n += 1;
				}
				if !it.accel.is_empty() {
					text[n] = b'\t';
					n += 1;
					for c in it.accel.chars() {
						text[n] = c as u32 as u8;
						n += 1;
					}
				}
				let id = NCMDS;
				if id < 64 {
					(*core::ptr::addr_of_mut!(CMDS))[id] = it.command;
					NCMDS += 1;
				}
				AppendMenu(pop, MF_STRING, 100 + id as u32, text.as_ptr() as u32);
			}
			let mut title = [0u8; 24];
			let t = quill_app::menu_title(m);
			title[..t.len()].copy_from_slice(t);
			AppendMenu(bar, MF_POPUP, pop, title.as_ptr() as u32);
		}
		bar
	}
}

#[unsafe(no_mangle)]
pub extern "C" fn w16_main(hinst: u32, hprev: u32, show: u32) -> u32 {
	// `/FAULT` installs the ToolHelp handler that logs the registers of a fault to FAULT.LOG.
	unsafe {
		let a = &mut *core::ptr::addr_of_mut!(APP);
		a.write(App::new());
		let _ = app().reset(store());
	}
	// The command line: `[FILE] [/PLAY KEYS] [/DUMP OUT]`.
	let mut cmd = [0u8; 128];
	let mut n = command_line(&mut cmd);
	// The program's own directory.
	unsafe {
		let p = &mut *core::ptr::addr_of_mut!(HERE);
		let len = GetModuleFileName(hinst, p.as_mut_ptr() as u32, 96) as usize;
		let mut end = len;
		while end > 0 && p[end - 1] != b'\\' {
			end -= 1;
		}
		HERE_LEN = end;
	}
	// Windows 3.1's `run=` line takes no arguments, so a test harness leaves them in QUILL.CMD beside the program.
	if n == 0 {
		unsafe { USE_HERE = true };
		if let Some(mut f) = Win16Files.open(b"QUILL.CMD") {
			n = f.read(&mut cmd[..127]);
			while n > 0 && cmd[n - 1] <= b' ' {
				n -= 1;
			}
		}
		unsafe { USE_HERE = false };
	}
	let mut play_file: Option<([u8; 32], usize)> = None;
	let mut dump_file: Option<([u8; 32], usize)> = None;
	let mut open: Option<([u8; 32], usize)> = None;
	let mut words = cmd[..n].split(|b| *b == b' ').filter(|w| !w.is_empty());
	let mut take = |w: Option<&[u8]>| {
		let w = w?;
		let mut b = [0u8; 32];
		let m = w.len().min(32);
		b[..m].copy_from_slice(&w[..m]);
		Some((b, m))
	};
	while let Some(w) = words.next() {
		if w.eq_ignore_ascii_case(b"/PLAY") {
			play_file = take(words.next());
		} else if w.eq_ignore_ascii_case(b"/DUMP") {
			dump_file = take(words.next());
		} else if w.eq_ignore_ascii_case(b"/FAULT") {
			// Logs the registers of a General Protection Fault to FAULT.LOG (a ToolHelp handler).
			w16::fault::install();
		} else if w.eq_ignore_ascii_case(b"/MONO") {
			app().set_scheme(Scheme::Mono);
		} else if w[0] != b'/' {
			open = take(Some(w));
		}
	}
	// With a script, files are found beside the program.
	unsafe { USE_HERE = play_file.is_some() };
	app().load_settings(&mut Win16Files);
	app().set_about(if play_file.is_some() {
		"Host test run"
	} else {
		"Windows 3.x, Win16"
	});
	let mut script = None;
	if let Some((name, len)) = &play_file {
		if let Some((text, tlen, o, ol)) = load_script(&name[..*len]) {
			script = Some((text, tlen));
			if ol > 0 {
				open = Some((o, ol));
			}
		}
	}
	if let Some((name, len)) = &open {
		let _ = app().open(store(), &mut Win16Files, &name[..*len]);
	}
	unsafe {
		if let (Some((name, len)), true) = (&dump_file, play_file.is_some()) {
			let mut buf = [0u8; 128];
			c_name(&name[..*len], &mut buf);
			let out = _lcreat(buf.as_ptr() as u32, 0);
			if let Some((text, tlen)) = script {
				PLAY = Some(Play {
					text,
					len: tlen,
					at: 0,
					frames: 0,
					out,
					staged: 0,
				});
			}
		}
		// The size of a cell in the OEM font.
		let dc = GetDC(0);
		SelectObject(dc, GetStockObject(OEM_FIXED_FONT));
		let mut tm = [0u8; 64];
		GetTextMetrics(dc, tm.as_mut_ptr() as u32);
		ReleaseDC(0, dc);
		let h = u32::from(tm[0]) | u32::from(tm[1]) << 8;
		let w = u32::from(tm[10]) | u32::from(tm[11]) << 8;
		if h > 0 && w > 0 {
			CELL = (w, h);
		}
		let (cw, ch) = CELL;
		if hprev == 0 {
			let class = WndClass {
				style: 0,
				wndproc: code_selector() << 16 | QuillWndProc as usize as u32 & 0xFFFF,
				cls_extra: 0,
				wnd_extra: 0,
				instance: hinst as u16,
				icon: LoadIcon(hinst, far(0, int_resource(1))) as u16,
				cursor: LoadCursor(0, far(0, int_resource(32512))) as u16,
				background: 0,
				menu: 0,
				class_name: far(data_selector(), cs!("QuillWin16")),
			};
			RegisterClass(&class as *const WndClass as u32);
		}
		let menu = build_menu();
		let frame_w = 80 * cw + 2 * GetSystemMetrics(32);
		let frame_h =
			25 * ch + 2 * GetSystemMetrics(33) + GetSystemMetrics(4) + GetSystemMetrics(15) + 1;
		let hwnd = CreateWindow(
			cs!("QuillWin16"),
			cs!("Quill"),
			WS_OVERLAPPEDWINDOW,
			CW_USEDEFAULT,
			0,
			frame_w,
			frame_h,
			0,
			menu,
			hinst,
			0,
		);
		SetMenu(hwnd, menu);
		ShowWindow(hwnd, show.max(1));
		UpdateWindow(hwnd);
		if (*core::ptr::addr_of!(PLAY)).is_some() {
			SetTimer(hwnd, 1, 300, 0);
		}
		let mut msg = core::mem::zeroed::<Msg>();
		while GetMessage(&mut msg as *mut Msg as u32, 0, 0, 0) != 0 {
			TranslateMessage(&mut msg as *mut Msg as u32);
			DispatchMessage(&mut msg as *mut Msg as u32);
		}
		if play_file.is_some() {
			ExitWindows(0, 0);
		}
	}
	0
}
