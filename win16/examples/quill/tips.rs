// Tooltips for the toolbar: Windows 3.1 has no tooltip control, so a small popup window shows the name of the button or box the
// pointer rests on, and the status bar says what it does.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A timer ticks every 100 ms and asks where the pointer is: over a toolbar button, over one of the three drop-down boxes (which
//! take the mouse for themselves, so the main window never hears it move over them) or somewhere else. Resting on the same one
//! for half a second shows the tip, a popup `STATIC` control made topmost without being activated; moving to another one, a click,
//! a key, a menu or leaving the window takes it away. The status bar shows the sentence of the thing under the pointer at once.
//! The popup's colours come from the parent's `WM_CTLCOLOR`, answered in `main.rs` with `tip_brush`.

use quill_chrome::view::{self, Act};
use quill_chrome::zoom_field::ZoomPart;
use w16::*;

use crate::chrome;
use crate::gdi::{QApp, np};

const WS_POPUP: u32 = 0x8000_0000;
const WS_BORDER: u32 = 0x0080_0000;
const SS_CENTER: u32 = 1;
const WM_SETFONT: u32 = 0x0030;
const SWP_NOACTIVATE: u32 = 0x0010;
const SWP_SHOWWINDOW: u32 = 0x0040;
const HWND_TOPMOST: u32 = 0xFFFF;
/// The timer that polls the pointer.
pub const TIMER: u32 = 2;
/// Ticks of 100 ms the pointer must rest for.
const DELAY: u8 = 5;
/// A light yellow, as a COLORREF (blue, green, red).
pub const TIP_BACK: u32 = 0x00C0_FFFF;

/// What the pointer rests on.
#[derive(Copy, Clone, PartialEq, Eq)]
enum Target {
	None,
	/// A toolbar button, by its row and x.
	Button(u8, i32),
	/// One of the drop-down boxes: 0 style, 1 face, 2 size.
	Combo(u8),
	/// A part of the zoom control on the status bar.
	Zoom(ZoomPart),
}

struct Tip {
	target: Target,
	ticks: u8,
	/// The popup window, or 0.
	wnd: u32,
	/// A click or a key ended this tip; no other shows until the pointer moves to something else.
	quiet: bool,
	brush: u32,
}

static mut TIP: Tip = Tip {
	target: Target::None,
	ticks: 0,
	wnd: 0,
	quiet: false,
	brush: 0,
};

fn tip() -> &'static mut Tip {
	unsafe { &mut *core::ptr::addr_of_mut!(TIP) }
}

/// The brush the popup's `WM_CTLCOLOR` answers with, made once.
pub fn tip_brush() -> u32 {
	let t = tip();
	if t.brush == 0 {
		t.brush = unsafe { CreateSolidBrush(TIP_BACK) };
	}
	t.brush
}

/// True for the popup itself, so the parent can colour it.
pub fn is_tip(wnd: u32) -> bool {
	let t = tip();
	t.wnd != 0 && t.wnd == wnd
}

/// What the pointer is over, from the cursor position in the window's client area.
fn target_at(hwnd: u32, app: &QApp) -> Target {
	let mut pt = [0u16; 2];
	unsafe {
		GetCursorPos(np(pt.as_mut_ptr()));
		let at = u32::from(pt[0]) | u32::from(pt[1]) << 16;
		let under = WindowFromPoint(at);
		let c = chrome::chrome();
		if under == c.style {
			return Target::Combo(0);
		}
		if under == c.face {
			return Target::Combo(1);
		}
		if under == c.size {
			return Target::Combo(2);
		}
		if under != hwnd {
			return Target::None;
		}
		ScreenToClient(hwnd, np(pt.as_mut_ptr()));
	}
	let (x, y) = (i32::from(pt[0] as i16), i32::from(pt[1] as i16));
	if let Some(part) = crate::zoom_part_at(hwnd, x, y) {
		return Target::Zoom(part);
	}
	match view::button_at(app, x, y) {
		Some((b, _)) => Target::Button(b.row, b.x),
		None => Target::None,
	}
}

/// The button at a row and x.
fn button(row: u8, x: i32) -> Option<view::Button> {
	view::TOOLBAR
		.buttons
		.iter()
		.find(|b| b.row == row && b.x == x)
		.copied()
}

/// The string resources of the zoom parts' words: the tooltips are 31 to 34 and the status sentences 35 to 38, each in the order
/// Page Width, minus, box, plus (`resources.json`; the data segment has no room for them as strings in the program).
fn zoom_string(part: ZoomPart, sentence: bool) -> u32 {
	let place = match part {
		ZoomPart::Reset => 0,
		ZoomPart::Minus => 1,
		ZoomPart::Field => 2,
		ZoomPart::Plus => 3,
	};
	31 + place + if sentence { 4 } else { 0 }
}

/// Loads a string resource into the start of `out` and returns its length.
fn load(id: u32, out: &mut [u8]) -> usize {
	let n = unsafe { LoadString(w16::HINST, id, np(out.as_mut_ptr()), out.len() as u32) } as usize;
	n.min(out.len())
}

/// Copies `text` to the start of `out` and returns the length.
fn copy(text: &str, out: &mut [u8]) -> usize {
	let n = text.len().min(out.len());
	if let (Some(d), Some(s)) = (out.get_mut(..n), text.as_bytes().get(..n)) {
		d.copy_from_slice(s);
	}
	n
}

/// The tooltip's text, put at the start of `out`: its length.
fn tip_text(t: Target, out: &mut [u8]) -> usize {
	match t {
		Target::None => 0,
		Target::Combo(i) => copy(quill_app::combo_tip(i), out),
		Target::Button(row, x) => match button(row, x).map(|b| b.act) {
			Some(Act::Cmd(c)) => copy(quill_app::command_tip(c), out),
			_ => copy(quill_app::COLOUR_TIP, out),
		},
		Target::Zoom(part) => load(zoom_string(part, false), out),
	}
}

/// The sentence for the status bar, put at the start of `out`: its length, 0 when the pointer rests on nothing.
fn hint_text(t: Target, out: &mut [u8]) -> usize {
	match t {
		Target::None => 0,
		Target::Combo(i) => copy(quill_app::combo_hint(i), out),
		Target::Button(row, x) => match button(row, x).map(|b| b.act) {
			Some(Act::Cmd(c)) => copy(quill_app::command_hint(c), out),
			_ => copy(quill_app::COLOUR_HINT, out),
		},
		Target::Zoom(part) => load(zoom_string(part, true), out),
	}
}

/// The status-bar sentence for what the pointer rests on, at the start of `out`: its length, or 0.
pub fn hint(out: &mut [u8]) -> usize {
	hint_text(tip().target, out)
}

/// Takes the tip away.
pub fn hide() {
	let t = tip();
	if t.wnd != 0 {
		unsafe { DestroyWindow(t.wnd) };
		t.wnd = 0;
	}
}

/// A click, a key or a menu: the tip goes and stays away until the pointer rests on something else.
pub fn dismiss() {
	hide();
	let t = tip();
	t.quiet = true;
	t.ticks = 0;
}

/// Where part `part` of the zoom control sits in a bar `width` wide: its `left..right`.
fn zoom_range(width: i32, part: ZoomPart) -> (i32, i32) {
	chrome::zoom_layout(width).map_or((0, 0), |l| match part {
		ZoomPart::Minus => l.minus,
		ZoomPart::Field => l.field,
		ZoomPart::Plus => l.plus,
		ZoomPart::Reset => l.reset,
	})
}

/// Pops the tip up under the button or box.
fn show(hwnd: u32, app: &QApp, t: Target) {
	let c = chrome::chrome();
	let mut at = [0u16; 2];
	// Under the toolbar button, or under the box; a part of the zoom control is on the bottom line, so its tip goes above it.
	let (x, y) = match t {
		Target::Zoom(_) => (0, 0),
		Target::Button(row, bx) => {
			let (h0, _) = view::rows(app);
			let top = if row == 0 { 0 } else { h0 };
			(bx, top + 2 + view::BTN_H + 2)
		}
		_ => {
			let (cx, _) = match t {
				Target::Combo(0) => view::TOOLBAR.style,
				Target::Combo(1) => view::TOOLBAR.face,
				_ => view::TOOLBAR.size,
			};
			(cx, view::TB_H + 3 + 24)
		}
	};
	at[0] = x as u16;
	at[1] = y as u16;
	let mut buf = [0u8; 40];
	let n = tip_text(t, buf.get_mut(1..37).unwrap_or(&mut []));
	buf[0] = b' ';
	buf[n + 1] = b' ';
	let len = n + 2;
	unsafe {
		ClientToScreen(hwnd, np(at.as_mut_ptr()));
		let dc = GetDC(hwnd);
		let old = SelectObject(dc, c.ui_font);
		let ext = GetTextExtent(dc, np(buf.as_ptr()), len as u32);
		SelectObject(dc, old);
		ReleaseDC(hwnd, dc);
		let w = i32::from(ext as u16) + 6;
		let h = i32::from((ext >> 16) as u16) + 4;
		if let Target::Zoom(part) = t {
			let (cw, ch) = crate::client_size(hwnd);
			let (left, _) = zoom_range(cw, part);
			at[0] = left.min(cw - w).max(0) as u16;
			at[1] = (ch - chrome::status_h() - h - 2).max(0) as u16;
			ClientToScreen(hwnd, np(at.as_mut_ptr()));
		}
		let tw = CreateWindow(
			cs!("STATIC"),
			np(buf.as_ptr()),
			WS_POPUP | WS_BORDER | SS_CENTER,
			u32::from(at[0]),
			u32::from(at[1]),
			w as u32,
			h as u32,
			hwnd,
			0,
			crate::sh().hinst,
			0,
		);
		if tw != 0 {
			SendMessage(tw, WM_SETFONT, c.ui_font, 0);
			SetWindowPos(
				tw,
				HWND_TOPMOST,
				u32::from(at[0]),
				u32::from(at[1]),
				w as u32,
				h as u32,
				SWP_NOACTIVATE | SWP_SHOWWINDOW,
			);
			tip().wnd = tw;
		}
	}
}

/// One tick of the pointer timer. Returns true when what the status bar says changed.
pub fn tick(hwnd: u32, app: &QApp) -> bool {
	let now = if unsafe { IsIconic(hwnd) } == 0 {
		target_at(hwnd, app)
	} else {
		Target::None
	};
	let t = tip();
	if now != t.target {
		hide();
		t.target = now;
		t.ticks = 0;
		t.quiet = false;
		return true;
	}
	if now != Target::None && !t.quiet && t.wnd == 0 {
		t.ticks += 1;
		if t.ticks >= DELAY {
			show(hwnd, app, now);
		}
	}
	false
}
