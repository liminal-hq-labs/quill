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

use quill_app::view::{self, Act};
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

/// The tooltip's text, and the sentence for the status bar.
fn words(t: Target) -> (&'static str, &'static str) {
	match t {
		Target::None => ("", ""),
		Target::Combo(i) => (quill_app::combo_tip(i), quill_app::combo_hint(i)),
		Target::Button(row, x) => match button(row, x).map(|b| b.act) {
			Some(Act::Cmd(c)) => (quill_app::command_tip(c), quill_app::command_hint(c)),
			_ => (quill_app::COLOUR_TIP, quill_app::COLOUR_HINT),
		},
	}
}

/// The status-bar sentence for what the pointer rests on, or an empty string.
pub fn hint() -> &'static str {
	words(tip().target).1
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

/// Pops the tip up under the button or box.
fn show(hwnd: u32, app: &QApp, t: Target, text: &str) {
	let c = chrome::chrome();
	let mut at = [0u16; 2];
	// Under the toolbar button, or under the box.
	let (x, y) = match t {
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
	let n = text.len().min(36);
	buf[0] = b' ';
	buf.get_mut(1..=n)
		.map(|d| d.copy_from_slice(text.as_bytes().get(..n).unwrap_or(&[])));
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
			let (text, _) = words(now);
			show(hwnd, app, now, text);
		}
	}
	false
}
