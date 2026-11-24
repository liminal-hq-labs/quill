// Quill for Windows 3.x: the editor in a Windows 3.1 window, with a menu bar, two toolbar rows, a ruler, the page drawn with GDI
// fonts, a status bar and scroll bars.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// `QUILL.EXE [FILE]` opens a file. `/PLAY KEYS.TXT /DUMP OUT.TXT` plays a key script and writes every screen, as the other builds do,
// and ends Windows when it is done; that test mode lays the document out for the DOS text grid and shows it in the OEM font, so its
// frames can be compared with the host's goldens. With `/PLAY` names are looked for beside the program. The code is split over
// several segments (see `segments.json`); the document is in global memory (`store.rs`).
#![no_std]
#![no_main]

mod chrome;
mod dialogs;
mod gdi;
mod store;

use core::mem::MaybeUninit;
use gdi::{Gdi16, QApp, Rect16, np};
use quill_app::view::{self, Act, Hit};
use quill_app::{Command, Dir, Files, Key, Scheme};
use quill_formats::io::{Sink, Source};
use quill_grid::Grid;
use store::FarStore;
use w16::*;

export_proc!(QuillWndProc, wnd_proc);

const WM_SIZE: u32 = 0x0005;
const WM_CLOSE: u32 = 0x0010;
const WM_ERASEBKGND: u32 = 0x0014;
const WM_KEYDOWN: u32 = 0x0100;
const WM_CHAR: u32 = 0x0102;
const WM_TIMER: u32 = 0x0113;
const WM_HSCROLL: u32 = 0x0114;
const WM_VSCROLL: u32 = 0x0115;
const WM_MOUSEMOVE: u32 = 0x0200;
const WM_LBUTTONDOWN: u32 = 0x0201;
const WM_LBUTTONUP: u32 = 0x0202;
const WS_OVERLAPPEDWINDOW: u32 = 0x00CF_0000;
const WS_VSCROLL: u32 = 0x0020_0000;
const WS_HSCROLL: u32 = 0x0010_0000;
const WS_CLIPCHILDREN: u32 = 0x0200_0000;
const CW_USEDEFAULT: u32 = 0x8000;
const OEM_FIXED_FONT: u32 = 10;
const MF_STRING: u32 = 0;
const MF_POPUP: u32 = 0x10;
const MK_SHIFT: u32 = 4;
const SB_HORZ: u32 = 0;
const SB_VERT: u32 = 1;
const VGA: [u32; 16] = [
	0x000000, 0xAA0000, 0x00AA00, 0xAAAA00, 0x0000AA, 0xAA00AA, 0x0055AA, 0xAAAAAA, 0x555555,
	0xFF5555, 0x55FF55, 0xFFFF55, 0x5555FF, 0xFF55FF, 0x55FFFF, 0xFFFFFF,
];

/// The client area Quill asks for when the screen has room: the 8.5 inch page at 96 dots per inch with a little desk either side,
/// and about 600 pixels of toolbars, ruler and page. A 640 by 480 or 800 by 600 screen opens the window maximised instead (D91).
const PREFERRED_W: i32 = 860;
const PREFERRED_H: i32 = 600;

static mut STORE: FarStore = FarStore::new();
static mut APP: MaybeUninit<QApp> = MaybeUninit::uninit();
static mut CMDS: [Command; 64] = [Command::New; 64];
static mut NCMDS: usize = 0;
static mut CELL: (u32, u32) = (8, 16);
/// The program's own directory, ending in a backslash, when names should be looked for there.
static mut HERE: [u8; 100] = [0; 100];
static mut HERE_LEN: usize = 0;
static mut USE_HERE: bool = false;
/// A memory device context compatible with the screen: the editor measures text with it, for the life of the program.
pub static mut MEASURE_DC: u32 = 0;

/// What the window keeps between messages.
struct Shell {
	/// The test mode: the DOS text grid in the OEM font, for the golden frames.
	grid: bool,
	hinst: u32,
	/// The off-screen picture every paint draws into before one copy to the window, and its size.
	back_dc: u32,
	back_bmp: u32,
	back_old: u32,
	back_w: i32,
	back_h: i32,
	/// How far the page is scrolled sideways, in pixels; zero unless the window is narrower than the page.
	hscroll: i32,
	/// The kind of tab stop a click on the ruler makes: 0 left, 1 centre, 2 right, 3 decimal.
	ruler_kind: u8,
	drag: Option<view::Drag>,
	/// The left button went down in the text and is still down.
	selecting: bool,
}

static mut SH: Shell = Shell {
	grid: false,
	hinst: 0,
	back_dc: 0,
	back_bmp: 0,
	back_old: 0,
	back_w: 0,
	back_h: 0,
	hscroll: 0,
	ruler_kind: 0,
	drag: None,
	selecting: false,
};

fn app() -> &'static mut QApp {
	unsafe { (*core::ptr::addr_of_mut!(APP)).assume_init_mut() }
}

fn store() -> &'static mut FarStore {
	unsafe { &mut *core::ptr::addr_of_mut!(STORE) }
}

fn sh() -> &'static mut Shell {
	unsafe { &mut *core::ptr::addr_of_mut!(SH) }
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

/// Paints the test mode's text grid, cell by cell, in the OEM font.
fn paint_grid(dc: u32) {
	unsafe {
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
	}
}

fn client_size(hwnd: u32) -> (i32, i32) {
	let mut r = Rect16::default();
	unsafe { GetClientRect(hwnd, np(&mut r)) };
	(i32::from(r.right), i32::from(r.bottom))
}

/// The rectangle the page shows in: under the toolbars and the ruler, above the status bar.
fn page_area(hwnd: u32) -> Rect16 {
	let (w, h) = client_size(hwnd);
	let status = if app().show_status() {
		chrome::status_h()
	} else {
		0
	};
	Rect16::new(0, view::chrome_height(app()), w, h - status)
}

/// Makes (or remakes, after the window changed size) the off-screen bitmap. True when it is new and holds nothing yet.
fn ensure_back(wdc: u32, w: i32, h: i32) -> bool {
	let sh = sh();
	if sh.back_dc != 0 && sh.back_w == w && sh.back_h == h {
		return false;
	}
	unsafe {
		if sh.back_dc != 0 {
			SelectObject(sh.back_dc, sh.back_old);
			DeleteObject(sh.back_bmp);
			DeleteDC(sh.back_dc);
			sh.back_dc = 0;
		}
		let dc = CreateCompatibleDC(wdc);
		if dc == 0 {
			return true;
		}
		let bmp = CreateCompatibleBitmap(wdc, w.max(1) as u32, h.max(1) as u32);
		if bmp == 0 {
			DeleteDC(dc);
			return true;
		}
		sh.back_old = SelectObject(dc, bmp);
		sh.back_dc = dc;
		sh.back_bmp = bmp;
		sh.back_w = w;
		sh.back_h = h;
	}
	true
}

/// Handles WM_PAINT: draws the damaged part of the window off screen, then copies it to the window with one `BitBlt`, so the steps
/// of the drawing (the grey desk, the white page, the bevels, then the text) are never on the screen.
fn paint(hwnd: u32) {
	unsafe {
		let mut ps = core::mem::zeroed::<PaintStruct>();
		let wdc = BeginPaint(hwnd, np(&mut ps));
		if sh().grid {
			paint_grid(wdc);
			EndPaint(hwnd, np(&ps));
			return;
		}
		let (w, h) = client_size(hwnd);
		let r = ps.rect;
		let mut damage = Rect16::new(
			i32::from(r[0]),
			i32::from(r[1]),
			i32::from(r[2]),
			i32::from(r[3]),
		);
		if ensure_back(wdc, w, h) {
			damage = Rect16::new(0, 0, w, h);
		}
		let back = sh().back_dc;
		paint_into(if back != 0 { back } else { wdc }, damage, w, h);
		if back != 0 {
			BitBlt(
				wdc,
				damage.left as u32,
				damage.top as u32,
				(damage.right - damage.left) as u32,
				(damage.bottom - damage.top) as u32,
				back,
				damage.left as u32,
				damage.top as u32,
				gdi::SRCCOPY,
			);
		}
		EndPaint(hwnd, np(&ps));
	}
}

/// Draws the window into `dc`, limited to `damage`: the toolbar rows, the ruler, the page and the status bar.
fn paint_into(dc: u32, damage: Rect16, w: i32, h: i32) {
	let (app, s) = (app(), store());
	let (h0, h1) = view::rows(app);
	let tb = h0 + h1;
	let rl = if app.show_ruler() { view::RULER_H } else { 0 };
	let status = if app.show_status() {
		chrome::status_h()
	} else {
		0
	};
	let (dt, db) = (i32::from(damage.top), i32::from(damage.bottom));
	unsafe {
		let saved = SaveDC(dc);
		IntersectClipRect(
			dc,
			damage.left as u32,
			damage.top as u32,
			damage.right as u32,
			damage.bottom as u32,
		);
		if tb > 0 && dt < tb {
			chrome::paint_toolbar(dc, w, app, s);
		}
		if rl > 0 && dt < tb + rl && db > tb {
			let (left, tw) = view::text_extent(app, w, sh().hscroll);
			let st = sh()
				.drag
				.map_or_else(|| view::RulerState::of(app, s), |d| d.state);
			chrome::paint_ruler(dc, tb, w, left, tw, &st, sh().ruler_kind);
		}
		let area = Rect16::new(0, tb + rl, w, h - status);
		if db > tb + rl && dt < h - status {
			let rows = gdi::paint_page(dc, area, sh().hscroll, app, s);
			app.set_rows(rows);
			app.set_view_height(h - status - tb - rl);
		}
		if status > 0 && db > h - status {
			let mut t = [0u8; 96];
			let n = quill_app::status_text(app, s, &mut t);
			chrome::paint_status(dc, h - status, w, t.get(..n).unwrap_or(&[]));
		}
		RestoreDC(dc, saved);
	}
	chrome::sync_combos(app, s);
}

/// Asks for the page and the status bar to be painted again, and nothing above them.
fn invalidate_page(hwnd: u32) {
	let (w, h) = client_size(hwnd);
	let r = Rect16::new(0, view::chrome_height(app()), w, h);
	unsafe { InvalidateRect(hwnd, gdi::fp(&r), 0) };
}

/// Asks for the ruler alone to be painted again.
fn invalidate_ruler(hwnd: u32) {
	if !app().show_ruler() {
		return;
	}
	let (w, _) = client_size(hwnd);
	let top = view::chrome_height(app()) - view::RULER_H;
	let r = Rect16::new(0, top, w, top + view::RULER_H);
	unsafe { InvalidateRect(hwnd, gdi::fp(&r), 0) };
}

fn invalidate_all(hwnd: u32) {
	unsafe { InvalidateRect(hwnd, 0, 0) };
}

/// Sets both scroll bars from the document and the window: the vertical one in lines, the sideways one in pixels (hidden when the
/// page fits).
fn update_scroll(hwnd: u32) {
	if sh().grid {
		return;
	}
	let (app, s) = (app(), store());
	let (w, _) = client_size(hwnd);
	let lines = app.line_total(s).saturating_sub(1).min(32000);
	let hmax = view::hscroll_max(app, w).min(32000);
	let sh = sh();
	sh.hscroll = sh.hscroll.clamp(0, hmax);
	unsafe {
		SetScrollRange(hwnd, SB_VERT, 0, lines, 0);
		SetScrollPos(hwnd, SB_VERT, app.top_line().min(lines), 1);
		SetScrollRange(hwnd, SB_HORZ, 0, hmax as u32, 0);
		SetScrollPos(hwnd, SB_HORZ, sh.hscroll as u32, 1);
	}
}

/// Scrolls sideways, when the window is narrower than the page, until the caret is in view.
fn follow_caret(hwnd: u32) {
	let (app, s) = (app(), store());
	let (w, _) = client_size(hwnd);
	if sh().grid || !app.editing() || view::hscroll_max(app, w) == 0 {
		return;
	}
	if let Some((_, _, x)) = app.caret_xy(s) {
		let margin = quill_core::layout::to_units(app.doc().page.margin_l, view::DPI);
		let at = view::PAGE_PAD + margin + x;
		let slack = 32.min(w / 4);
		let sh = sh();
		if at - sh.hscroll < slack {
			sh.hscroll = at - slack;
		} else if at - sh.hscroll > w - slack {
			sh.hscroll = at - (w - slack);
		}
		update_scroll(hwnd);
	}
}

/// Ticks the View menu's toolbar, ruler and status bar items and shows or hides the boxes with the formatting row.
fn sync_chrome(hwnd: u32) {
	chrome::show_combos(app());
	unsafe {
		let menu = GetMenu(hwnd);
		if menu != 0 {
			let cmds = &*core::ptr::addr_of!(CMDS);
			for (i, c) in cmds.iter().take(NCMDS).enumerate() {
				if let Command::Chrome(bit) = c {
					let on = app().chrome_on(*bit);
					CheckMenuItem(menu, 100 + i as u32, if on { 8 } else { 0 });
				}
			}
		}
	}
}

/// The title bar: `Quill - NAME.EXT`, or `(Untitled)`.
fn retitle(hwnd: u32) {
	let mut t = [0u8; 80];
	let head = b"Quill - ";
	t[..head.len()].copy_from_slice(head);
	let mut n = head.len();
	let name = app().name();
	let shown: &[u8] = if name.is_empty() { b"(Untitled)" } else { name };
	for b in shown.iter().take(t.len() - n - 1) {
		t[n] = *b;
		n += 1;
	}
	t[n] = 0;
	unsafe { SetWindowText(hwnd, np(t.as_ptr())) };
}

/// After a key or a command: dialogs the editor asked for, help, the title, the scroll bars, and a repaint of what changed.
fn after_edit(hwnd: u32, before: u32) {
	if sh().grid {
		after(hwnd);
		return;
	}
	handle_dialogs(hwnd);
	if app().take_help() {
		open_help(hwnd);
	}
	retitle(hwnd);
	let scrolled = sh().hscroll;
	update_scroll(hwnd);
	follow_caret(hwnd);
	if before != view::signature(app(), store()) || scrolled != sh().hscroll {
		invalidate_all(hwnd);
	} else {
		invalidate_page(hwnd);
	}
	if app().quit() {
		unsafe { DestroyWindow(hwnd) };
	}
}

fn press(hwnd: u32, k: Key) {
	let before = view::signature(app(), store());
	app().key(store(), &mut Win16Files, k);
	after_edit(hwnd, before);
}

/// Runs a command from the menu or the toolbar.
fn run_command(hwnd: u32, c: Command) {
	let before = view::signature(app(), store());
	if c == Command::PrintSetup {
		// The common dialog's setup page: the printer and the paper.
		dialogs::print_dialog(hwnd, true);
	} else {
		app().command(store(), &mut Win16Files, c);
	}
	if let Command::Chrome(_) = c {
		sync_chrome(hwnd);
		invalidate_all(hwnd);
	}
	after_edit(hwnd, before);
}

/// Shows the dialogs the editor asked for (`dialogs.rs`).
fn handle_dialogs(hwnd: u32) {
	dialogs::handle(hwnd);
}

/// "Save the changes to NAME before ...?" for the unsaved-changes question.
fn confirm_text(c: Command, out: &mut [u8; 120]) -> usize {
	let name = app().name();
	let shown: &[u8] = if name.is_empty() { b"(Untitled)" } else { name };
	let what: &[u8] = match c {
		Command::Exit => b" before Quill closes?",
		Command::Open => b" before another document opens?",
		_ => b" before a new document starts?",
	};
	let mut n = 0;
	for part in [&b"Save the changes to "[..], shown, what] {
		for b in part {
			if n + 1 < out.len() {
				out[n] = *b;
				n += 1;
			}
		}
	}
	n
}

/// Opens QUILL.HLP (beside the program, from GetModuleFileName) in WinHelp at its contents.
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

/// After a key in the test mode: help, a repaint of everything, and the end of the program when the editor is done.
fn after(hwnd: u32) {
	unsafe {
		if app().take_help() {
			open_help(hwnd);
		}
		InvalidateRect(hwnd, 0, 0);
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

// ---- the mouse and the scroll bars ---------------------------------------------------------------------------------------------

fn mouse_down(hwnd: u32, x: i32, y: i32, shift: bool) {
	if sh().grid {
		return;
	}
	let (app, s) = (app(), store());
	let (h0, h1) = view::rows(app);
	if y < h0 + h1 {
		if let Some((b, top)) = view::button_at(app, x, y) {
			match b.act {
				Act::Cmd(c) => run_command(hwnd, c),
				Act::Colour => chrome::colour_menu(hwnd, b.x, top + view::TB_H),
			}
		}
		return;
	}
	let chrome_h = view::chrome_height(app);
	if app.show_ruler() && y < chrome_h {
		let (w, _) = client_size(hwnd);
		let (left, tw) = view::text_extent(app, w, sh().hscroll);
		let st = view::RulerState::of(app, s);
		let before = view::signature(app, s);
		match view::ruler_hit(x, y, h0 + h1, left, tw, &st) {
			Some(Hit::Kind) => sh().ruler_kind = (sh().ruler_kind + 1) % 4,
			Some(Hit::Scale(t)) => app.add_tab(s, view::snap(t), sh().ruler_kind),
			Some(hit) => {
				sh().drag = Some(view::Drag {
					hit,
					state: st,
					start: st,
				});
				unsafe { SetCapture(hwnd) };
			}
			None => {}
		}
		invalidate_ruler(hwnd);
		if before != view::signature(app, s) {
			invalidate_all(hwnd);
		}
		return;
	}
	let area = page_area(hwnd);
	if y >= i32::from(area.bottom) {
		return;
	}
	if let Some((line, tx)) = gdi::hit(app, s, area, sh().hscroll, x, y) {
		let before = view::signature(app, s);
		app.click(s, line, tx, shift);
		sh().selecting = true;
		unsafe { SetCapture(hwnd) };
		if before != view::signature(app, s) {
			invalidate_all(hwnd);
		} else {
			invalidate_page(hwnd);
		}
	}
}

fn mouse_move(hwnd: u32, x: i32, y: i32) {
	let (app, s) = (app(), store());
	if let Some(mut d) = sh().drag {
		let (w, _) = client_size(hwnd);
		let (left, tw) = view::text_extent(app, w, sh().hscroll);
		view::drag_to(&mut d, x, left, tw);
		sh().drag = Some(d);
		invalidate_ruler(hwnd);
		return;
	}
	if !sh().selecting {
		return;
	}
	// Dragging past the top or the bottom of the page scrolls a line at a time.
	let area = page_area(hwnd);
	let (top, bottom) = (i32::from(area.top), i32::from(area.bottom));
	if y < top && app.top_line() > 0 {
		app.scroll_to(s, app.top_line() - 1);
	} else if y >= bottom {
		app.scroll_to(s, app.top_line() + 1);
	}
	let y = y.clamp(top, bottom - 1);
	if let Some((line, tx)) = gdi::hit(app, s, area, sh().hscroll, x, y) {
		app.drag_select_line(s, line, tx);
		update_scroll(hwnd);
		invalidate_page(hwnd);
	}
}

fn mouse_up(hwnd: u32, x: i32) {
	let (app, s) = (app(), store());
	sh().selecting = false;
	unsafe { ReleaseCapture() };
	let Some(mut d) = sh().drag.take() else {
		return;
	};
	let (w, _) = client_size(hwnd);
	let (left, tw) = view::text_extent(app, w, sh().hscroll);
	view::drag_to(&mut d, x, left, tw);
	if matches!(d.hit, Hit::First | Hit::Left | Hit::Both | Hit::Right) {
		app.set_indents(s, d.state.left, d.state.first - d.state.left, d.state.right);
	}
	invalidate_all(hwnd);
}

/// A scroll bar message: `code` is what the user did and `thumb` where the box is.
fn scroll_bar(hwnd: u32, vertical: bool, code: u32, thumb: u32) {
	let (app, s) = (app(), store());
	if vertical {
		let top = app.top_line() as i32;
		let area = page_area(hwnd);
		let page = (i32::from(area.bottom - area.top) / 18).max(1);
		let to = match code {
			0 => top - 1,
			1 => top + 1,
			2 => top - page,
			3 => top + page,
			4 | 5 => thumb as i32,
			6 => 0,
			7 => i32::MAX / 2,
			_ => return,
		};
		app.scroll_to(s, to.max(0) as u32);
		update_scroll(hwnd);
		invalidate_page(hwnd);
	} else {
		let (w, _) = client_size(hwnd);
		let now = sh().hscroll;
		let to = match code {
			0 => now - 16,
			1 => now + 16,
			2 => now - w,
			3 => now + w,
			4 | 5 => thumb as i32,
			6 => 0,
			7 => i32::MAX / 2,
			_ => return,
		};
		sh().hscroll = to;
		update_scroll(hwnd);
		if sh().hscroll != now {
			invalidate_ruler(hwnd);
			invalidate_page(hwnd);
		}
	}
}

/// A face, size or style was chosen in one of the boxes.
fn combo_changed(hwnd: u32, id: u32) {
	let (app, s) = (app(), store());
	if id == chrome::ID_STYLE {
		if let Some(at) = chrome::chosen_index(id) {
			app.apply_style(s, at);
		}
	} else {
		let mut buf = [0u8; 40];
		if let Some(n) = chrome::choice(id, &mut buf) {
			let text = buf.get(..n).unwrap_or(&[]);
			if id == chrome::ID_FACE {
				app.set_face(s, text, view::family_of(text));
			} else {
				let pts = text.iter().fold(0u16, |v, b| {
					if b.is_ascii_digit() {
						v.saturating_mul(10).saturating_add(u16::from(b - b'0'))
					} else {
						v
					}
				});
				if pts > 0 {
					app.set_size(s, pts * 2);
				}
			}
		}
	}
	unsafe { SetFocus(hwnd) };
	invalidate_all(hwnd);
}

// ---- the script ----------------------------------------------------------------------------------------------------------------

/// The script text lives in a block of global memory, read a word at a time, so the data segment keeps its room for the stack.
struct Play {
	text: u32,
	len: usize,
	at: usize,
	frames: u32,
	out: u32,
	stage: [u8; 512],
	staged: usize,
}

static mut PLAY: Option<Play> = None;

impl Play {
	fn put(&mut self, data: &[u8]) {
		for &b in data {
			if self.staged == self.stage.len() {
				self.flush();
			}
			self.stage[self.staged] = b;
			self.staged += 1;
		}
	}

	fn flush(&mut self) {
		if self.staged > 0 {
			unsafe { _lwrite(self.out, self.stage.as_ptr() as u32, self.staged as u32) };
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
	write_stack_log();
	pl.put(b"=== end ===\n");
	pl.flush();
	unsafe { _lclose(pl.out) };
	unsafe { PLAY = None };
	after(hwnd);
	unsafe { DestroyWindow(hwnd) };
}

/// Writes STACK.LOG: how much of the stack the run used and how much it had.
fn write_stack_log() {
	let (top, low) = unsafe { (store::TOP_SP, store::MIN_SP) };
	let used = u32::from(top.saturating_sub(low));
	let log = w16::log();
	let margin = u32::from(low.saturating_sub(store::stack_floor()));
	log.s("stack used ")
		.dec(used)
		.s(" of ")
		.dec(STACK_SIZE)
		.s(" margin ")
		.dec(margin)
		.s(" top ")
		.hex(u32::from(top), 4)
		.s(" low ")
		.hex(u32::from(low), 4)
		.nl();
	let mut buf = [0u8; 128];
	c_name(b"STACK.LOG", &mut buf);
	write_file(buf.as_ptr(), log.bytes());
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
		stage: [0; 512],
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

fn lo16(v: u32) -> i32 {
	i32::from(v as u16 as i16)
}

fn hi16(v: u32) -> i32 {
	i32::from((v >> 16) as u16 as i16)
}

extern "C" fn wnd_proc(hwnd: u32, msg: u32, wparam: u32, lparam: u32) -> u32 {
	unsafe {
		match msg {
			WM_PAINT => {
				paint(hwnd);
				0
			}
			// Every pixel of the client area is painted from the off-screen picture, so the system never erases it first.
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
				// Ctrl+H, Ctrl+I and Ctrl+M arrive as Backspace, Tab and Enter; the Ctrl key tells them apart.
				let ctrl = GetKeyState(0x11) & 0x8000 != 0;
				let k = match c {
					1..=26 if ctrl => Some(Key::Ctrl(c + 0x60)),
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
			WM_SIZE => {
				// SIZE_MINIMIZED: the class icon stands in for the window.
				if wparam != 1 {
					update_scroll(hwnd);
					invalidate_all(hwnd);
				}
				0
			}
			WM_VSCROLL | WM_HSCROLL => {
				scroll_bar(hwnd, msg == WM_VSCROLL, wparam & 0xFFFF, lparam & 0xFFFF);
				0
			}
			WM_LBUTTONDOWN => {
				mouse_down(hwnd, lo16(lparam), hi16(lparam), wparam & MK_SHIFT != 0);
				0
			}
			WM_MOUSEMOVE => {
				mouse_move(hwnd, lo16(lparam), hi16(lparam));
				0
			}
			WM_LBUTTONUP => {
				mouse_up(hwnd, lo16(lparam));
				0
			}
			WM_COMMAND => {
				// Windows 3.1: wParam is the item or control id; a control puts its notification in the high word of lParam.
				let id = wparam & 0xFFFF;
				if lparam != 0 {
					// CBN_SELCHANGE
					if lparam >> 16 == 1 {
						combo_changed(hwnd, id);
					}
				} else if (chrome::ID_COLOUR..=chrome::ID_COLOUR + 16).contains(&id) {
					let rgb = (id > chrome::ID_COLOUR)
						.then(|| {
							view::COLOURS
								.get((id - chrome::ID_COLOUR - 1) as usize)
								.map(|c| c.1)
						})
						.flatten();
					app().set_colour(store(), rgb);
					invalidate_all(hwnd);
				} else if id >= 100 && ((id - 100) as usize) < NCMDS {
					let c = (*core::ptr::addr_of!(CMDS))[(id - 100) as usize];
					if sh().grid {
						app().command(store(), &mut Win16Files, c);
						after(hwnd);
					} else {
						run_command(hwnd, c);
					}
				}
				0
			}
			WM_CLOSE => {
				// The close box asks like File > Exit, so unsaved changes are asked about.
				if sh().grid {
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

/// Builds the editor into its static, laid out for `measure`. Apart from `w16_main`, so that `App::with_measure()`'s 11 KB temporary is
/// gone from the stack before the message loop runs (it was live for the whole run, and with it the stack ran past its bottom into
/// the program's statics).
#[inline(never)]
fn init_app(measure: Gdi16) {
	unsafe {
		let a = &mut *core::ptr::addr_of_mut!(APP);
		a.write(QApp::with_measure(measure));
		let _ = app().reset(store());
	}
}

/// The stack and local heap sizes `build.sh` gives the packer (it reads these two lines). The stack, the shadow return stack, the data
/// and the heap must fit one 64 KB segment, and a play-back reports how much of the stack it used (`STACK.LOG`): running past the
/// bottom overwrites the program's statics, which showed up as four zero bytes in the screen dump.
const STACK_SIZE: u32 = 0x7400;
const HEAP_SIZE: u32 = 0x1000;

/// Where the window goes and how big it is: the preferred size, centred, or maximised when the screen is smaller (Windows 3.1 has no
/// taskbar, so the work area is the screen). Returns x, y, width, height and whether to maximise.
fn placement(gridview: bool, cw: u32, ch: u32) -> (u32, u32, u32, u32, bool) {
	unsafe {
		let sm = |i: u32| GetSystemMetrics(i) as u16 as i16 as i32;
		let (frame_x, frame_y, caption, menu) = (sm(32), sm(33), sm(4), sm(15));
		if gridview {
			let w = 80 * cw as i32 + 2 * frame_x;
			let h = 25 * ch as i32 + 2 * frame_y + caption + menu + 1;
			return (CW_USEDEFAULT, 0, w as u32, h as u32, false);
		}
		let (vbar, hbar) = (sm(2), sm(3));
		let w = PREFERRED_W + 2 * frame_x + vbar;
		let h = PREFERRED_H + chrome::status_h() + 2 * frame_y + caption + menu + hbar;
		let (sw, shh) = (sm(0).max(640), sm(1).max(480));
		if w > sw || h > shh {
			return (0, 0, sw as u32, shh as u32, true);
		}
		(
			((sw - w) / 2) as u32,
			((shh - h) / 2) as u32,
			w as u32,
			h as u32,
			false,
		)
	}
}

#[unsafe(no_mangle)]
pub extern "C" fn w16_main(hinst: u32, hprev: u32, show: u32) -> u32 {
	store::note_top();
	// The command line: `[FILE] [/PLAY KEYS] [/DUMP OUT] [/MONO] [/FAULT]`.
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
	let mut mono = false;
	let mut words = cmd[..n].split(|b| *b == b' ').filter(|w| !w.is_empty());
	let take = |w: Option<&[u8]>| {
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
			mono = true;
		} else if w[0] != b'/' {
			open = take(Some(w));
		}
	}
	let gridview = play_file.is_some();
	unsafe {
		MEASURE_DC = CreateCompatibleDC(0);
		sh().grid = gridview;
		sh().hinst = hinst;
	}
	init_app(Gdi16 {
		dc: unsafe { MEASURE_DC },
		grid: gridview,
	});
	if mono {
		app().set_scheme(Scheme::Mono);
	}
	// The windowed build never draws a dialog in text cells (D86); the test mode keeps them, as its goldens have them.
	app().set_native_dialogs(!gridview);
	// With a script, files are found beside the program.
	unsafe { USE_HERE = gridview };
	app().load_settings(&mut Win16Files);
	app().set_about(if gridview {
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
		if let (Some((name, len)), true) = (&dump_file, gridview) {
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
					stage: [0; 512],
					staged: 0,
				});
			}
		}
		// The size of a cell in the OEM font, for the test mode.
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
		if !gridview {
			chrome::read_faces();
			chrome::init(hinst);
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
		let style = if gridview {
			WS_OVERLAPPEDWINDOW
		} else {
			WS_OVERLAPPEDWINDOW | WS_VSCROLL | WS_HSCROLL | WS_CLIPCHILDREN
		};
		let (x, y, w, h, maximise) = placement(gridview, cw, ch);
		let hwnd = CreateWindow(
			cs!("QuillWin16"),
			cs!("Quill"),
			style,
			x,
			y,
			w,
			h,
			0,
			menu,
			hinst,
			0,
		);
		if !gridview {
			chrome::create_combos(hwnd, hinst);
			sync_chrome(hwnd);
			retitle(hwnd);
			update_scroll(hwnd);
		}
		// SW_SHOWMAXIMIZED when the window does not fit, unless Windows asked for it minimised (2 or 7).
		let how = if maximise && !matches!(show, 2 | 6 | 7) {
			3
		} else {
			show.max(1)
		};
		ShowWindow(hwnd, how);
		UpdateWindow(hwnd);
		if (*core::ptr::addr_of!(PLAY)).is_some() {
			SetTimer(hwnd, 1, 300, 0);
		}
		let mut msg = core::mem::zeroed::<Msg>();
		while GetMessage(&mut msg as *mut Msg as u32, 0, 0, 0) != 0 {
			TranslateMessage(&mut msg as *mut Msg as u32);
			DispatchMessage(&mut msg as *mut Msg as u32);
		}
		if gridview {
			ExitWindows(0, 0);
		} else {
			app().save_settings(&mut Win16Files);
			// How deep the stack went in this session, beside the program, for `tools/check_stack_log.py`.
			USE_HERE = true;
			write_stack_log();
		}
	}
	0
}
