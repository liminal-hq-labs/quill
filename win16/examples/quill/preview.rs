// Print preview on Windows 3.x: the pages as they will print, one or two to a screen, under a bar of push buttons.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The preview takes over the client area. The toolbars, the ruler, the boxes and the page's scroll bars are put away while it shows, and
// the six buttons (Print, Next Page, Previous Page, Two Pages, Zoom, Close) are child windows made when it opens and destroyed when it
// closes, so they cost nothing while it is shut. Their captions, the status line's words and the key hints are string resources
// (`resources.json`, 20 to 30), because the data segment has no room for them. The state is a handful of bytes of the shell: a flag
// byte, the first page shown and the scroll position at actual size. Each sheet is drawn by `gdi::paint_sheet` at the percentage that
// fits it to the window (W16-81).
use crate::gdi::{self, FACE, Rect16, SHADOW, np};
use crate::{app, chrome, client_size, sh, store};
use quill_app::Command;
use quill_chrome::view;
use quill_core::layout::to_units;
use w16::*;

/// The first of the six buttons' control ids: Print, Next Page, Previous Page, Two Pages, Zoom and Close, in that order.
pub const ID_FIRST: u32 = 420;
const BUTTONS: u32 = 6;
/// The bar the buttons sit on, at the top of the window.
const BAR_H: i32 = 32;
const BUTTON_Y: i32 = 4;
const BUTTON_H: i32 = 24;
/// Each button's width.
const WIDTHS: [i32; BUTTONS as usize] = [64, 96, 124, 100, 104, 64];
/// The gap round the sheets, and between two of them.
const GAP: i32 = 16;
/// How far an arrow key moves a sheet that is shown at its actual size.
const STEP: i32 = 16;

// The string resources.
const STR_BUTTONS: u32 = 20;
const STR_ONE_PAGE: u32 = 26;
const STR_WHOLE_PAGE: u32 = 27;
const STR_PAGE: u32 = 28;
const STR_OF: u32 = 29;
const STR_KEYS: u32 = 30;

// The state's flag byte.
const OPEN: u8 = 1;
const TWO: u8 = 2;
const ACTUAL: u8 = 4;

const WM_SETFONT: u32 = 0x0030;
const WS_CHILD: u32 = 0x4000_0000;
const WS_VISIBLE: u32 = 0x1000_0000;

/// Whether the preview is showing.
pub fn is_open() -> bool {
	sh().pv_flags & OPEN != 0
}

/// Where everything is for a client area `w` by `h`: the percentage, the size of a sheet and the sheets' first corner.
struct Geo {
	pct: i32,
	pages: u32,
	sheets: i32,
	sheet_w: i32,
	sheet_h: i32,
	area_h: i32,
	left: i32,
	top: i32,
	/// How far the sheets can scroll at actual size, sideways and down.
	hmax: i32,
	vmax: i32,
}

fn status_h() -> i32 {
	if app().show_status() {
		chrome::status_h()
	} else {
		0
	}
}

fn geometry(w: i32, h: i32) -> Geo {
	let (a, s) = (app(), store());
	let state = sh();
	let area_h = (h - BAR_H - status_h()).max(1);
	let (pw, ph) = (
		view::page_width(a),
		to_units(a.doc().page.paper_h, view::DPI),
	);
	let sheets = if state.pv_flags & TWO != 0 { 2 } else { 1 };
	let pct = if state.pv_flags & ACTUAL != 0 {
		100
	} else {
		((w - GAP * (sheets + 1)) * 100 / (pw * sheets))
			.min((area_h - 2 * GAP) * 100 / ph)
			.clamp(10, 100)
	};
	let (sheet_w, sheet_h) = (view::dev(pw, pct), view::dev(ph, pct));
	let content_w = GAP + sheets * sheet_w + (sheets - 1) * GAP + GAP;
	let content_h = sheet_h + 2 * GAP;
	let (hmax, vmax) = ((content_w - w).max(0), (content_h - area_h).max(0));
	let (hs, vs) = (
		i32::from(state.pv_hs).clamp(0, hmax),
		i32::from(state.pv_vs).clamp(0, vmax),
	);
	Geo {
		pct,
		pages: a.page_total(s).max(1),
		sheets,
		sheet_w,
		sheet_h,
		area_h,
		left: if content_w <= w {
			(w - content_w) / 2 + GAP
		} else {
			GAP - hs
		},
		top: BAR_H
			+ if content_h <= area_h {
				(area_h - sheet_h) / 2
			} else {
				GAP - vs
			},
		hmax,
		vmax,
	}
}

/// Loads string resource `id` into `out`, null-terminated, and returns its length.
fn string(id: u32, out: &mut [u8]) -> usize {
	let n = unsafe { LoadString(w16::HINST, id, np(out.as_mut_ptr()), (out.len() - 1) as u32) }
		as usize;
	let n = n.min(out.len() - 1);
	if let Some(end) = out.get_mut(n) {
		*end = 0;
	}
	n
}

/// Gives button `i` the caption of string resource `id`.
fn caption(hwnd: u32, i: u32, id: u32) {
	let mut text = [0u8; 24];
	string(id, &mut text);
	unsafe { SetWindowText(GetDlgItem(hwnd, ID_FIRST + i), np(text.as_ptr())) };
}

/// Sets the Two Pages and Zoom buttons' captions to what pressing them does now.
fn captions(hwnd: u32) {
	let flags = sh().pv_flags;
	caption(
		hwnd,
		3,
		if flags & TWO != 0 {
			STR_ONE_PAGE
		} else {
			STR_BUTTONS + 3
		},
	);
	caption(
		hwnd,
		4,
		if flags & ACTUAL != 0 {
			STR_WHOLE_PAGE
		} else {
			STR_BUTTONS + 4
		},
	);
}

/// Opens the preview at the first page, with the toolbars, the ruler, the boxes and the scroll bars put away.
#[inline(never)]
pub fn open(hwnd: u32) {
	if is_open() || sh().grid {
		return;
	}
	let state = sh();
	state.pv_flags = OPEN;
	state.pv_page = 1;
	state.pv_hs = 0;
	state.pv_vs = 0;
	unsafe {
		let ui_font = chrome::chrome().ui_font;
		let mut x = 6;
		for (i, w) in WIDTHS.iter().enumerate() {
			let b = CreateWindow(
				cs!("BUTTON"),
				cs!(""),
				WS_CHILD | WS_VISIBLE,
				x as u32,
				BUTTON_Y as u32,
				*w as u32,
				BUTTON_H as u32,
				hwnd,
				ID_FIRST + i as u32,
				state.hinst,
				0,
			);
			SendMessage(b, WM_SETFONT, ui_font, 0);
			caption(hwnd, i as u32, STR_BUTTONS + i as u32);
			x += w + 4;
		}
		for b in [
			chrome::chrome().style,
			chrome::chrome().face,
			chrome::chrome().size,
		] {
			ShowWindow(b, 0);
		}
		ShowWindow(state.vbar, 0);
		ShowWindow(state.hbar, 0);
		SetFocus(hwnd);
		InvalidateRect(hwnd, 0, 0);
	}
}

/// Closes the preview and puts the window back as it was: the boxes, the bars and the page at the scroll position it had.
#[inline(never)]
pub fn close(hwnd: u32) {
	if !is_open() {
		return;
	}
	let state = sh();
	state.pv_flags = 0;
	unsafe {
		for i in 0..BUTTONS {
			DestroyWindow(GetDlgItem(hwnd, ID_FIRST + i));
		}
		ShowWindow(state.vbar, 5);
		// The bars are placed and given their ranges again by the next layout.
		state.sb_cw = -1;
		state.sb_range = [0xFFFF; 2];
		state.sb_pos = [0xFFFF; 2];
	}
	chrome::show_combos(app());
	crate::update_scroll(hwnd);
	unsafe {
		SetFocus(hwnd);
		InvalidateRect(hwnd, 0, 0);
	}
}

/// Shows the page `dir` sheets on, or back when negative.
fn step(hwnd: u32, dir: i32) {
	let (w, h) = client_size(hwnd);
	let g = geometry(w, h);
	let state = sh();
	let by = if state.pv_flags & TWO != 0 { 2 } else { 1 };
	state.pv_page = (i32::from(state.pv_page) + dir * by).clamp(1, g.pages as i32) as u16;
	state.pv_vs = 0;
	unsafe { InvalidateRect(hwnd, 0, 0) };
}

/// Shows page `n` first, clamped to the document.
fn goto(hwnd: u32, n: i32) {
	let (w, h) = client_size(hwnd);
	let g = geometry(w, h);
	let state = sh();
	let last = (g.pages as i32 - (g.sheets - 1)).max(1);
	state.pv_page = n.clamp(1, last) as u16;
	state.pv_vs = 0;
	unsafe { InvalidateRect(hwnd, 0, 0) };
}

/// Moves the sheets by (`dx`, `dy`) when they are shown at actual size.
fn scroll(hwnd: u32, dx: i32, dy: i32) {
	if sh().pv_flags & ACTUAL == 0 {
		return;
	}
	let (w, h) = client_size(hwnd);
	let g = geometry(w, h);
	let state = sh();
	state.pv_hs = (i32::from(state.pv_hs) + dx).clamp(0, g.hmax) as i16;
	state.pv_vs = (i32::from(state.pv_vs) + dy).clamp(0, g.vmax) as i16;
	unsafe { InvalidateRect(hwnd, 0, 0) };
}

/// A press of button `i` (or of its key).
pub fn command(hwnd: u32, i: u32) {
	match i {
		0 => crate::run_command(hwnd, Command::Print),
		1 => step(hwnd, 1),
		2 => step(hwnd, -1),
		3 | 4 => {
			let state = sh();
			state.pv_flags ^= if i == 3 { TWO } else { ACTUAL };
			state.pv_hs = 0;
			state.pv_vs = 0;
			captions(hwnd);
			unsafe { InvalidateRect(hwnd, 0, 0) };
		}
		_ => {
			close(hwnd);
			return;
		}
	}
	unsafe { SetFocus(hwnd) };
}

/// A `WM_COMMAND` from a control: true when it was one of the preview's buttons.
pub fn button(hwnd: u32, id: u32) -> bool {
	match id.checked_sub(ID_FIRST) {
		Some(i) if i < BUTTONS && is_open() => {
			command(hwnd, i);
			true
		}
		_ => false,
	}
}

/// A key while the preview shows: page keys change the sheet, the arrows move a sheet at actual size, `P`, `T` and `Z` press the
/// buttons of those names and Esc closes. Every other key is inert.
#[inline(never)]
pub fn key(hwnd: u32, vk: u32) {
	match vk {
		0x1B => close(hwnd),
		0x22 => step(hwnd, 1),
		0x21 => step(hwnd, -1),
		0x24 => goto(hwnd, 1),
		0x23 => goto(hwnd, i32::MAX),
		0x28 | 0x27 if sh().pv_flags & ACTUAL == 0 => step(hwnd, 1),
		0x26 | 0x25 if sh().pv_flags & ACTUAL == 0 => step(hwnd, -1),
		0x28 => scroll(hwnd, 0, STEP),
		0x26 => scroll(hwnd, 0, -STEP),
		0x27 => scroll(hwnd, STEP, 0),
		0x25 => scroll(hwnd, -STEP, 0),
		0x50 => command(hwnd, 0),
		0x54 => command(hwnd, 3),
		0x5A => command(hwnd, 4),
		_ => {}
	}
}

/// The status line: `Page N of M`, then the keys.
fn status_text(first: u32, pages: u32, out: &mut [u8; 96]) -> usize {
	let mut n = string(STR_PAGE, out);
	n += chrome::dec(first, &mut out[n..]);
	let mut word = [0u8; 8];
	let k = string(STR_OF, &mut word);
	out[n..n + k].copy_from_slice(&word[..k]);
	n += k;
	n += chrome::dec(pages, &mut out[n..]);
	let mut keys = [0u8; 80];
	let k = string(STR_KEYS, &mut keys).min(out.len() - n);
	out[n..n + k].copy_from_slice(&keys[..k]);
	n + k
}

/// Draws the bar, the desk, the sheets and the status line into `dc`, limited to `damage`.
#[inline(never)]
pub fn paint(dc: u32, damage: Rect16, w: i32, h: i32) {
	let (a, s) = (app(), store());
	let g = geometry(w, h);
	let first = u32::from(sh().pv_page).clamp(1, g.pages);
	gdi::fill(dc, 0, 0, w, BAR_H, FACE);
	gdi::fill(dc, 0, BAR_H - 1, w, 1, SHADOW);
	gdi::fill(dc, 0, BAR_H, w, g.area_h, SHADOW);
	unsafe {
		let saved = SaveDC(dc);
		IntersectClipRect(dc, 0, BAR_H as u32, w as u32, (BAR_H + g.area_h) as u32);
		for k in 0..g.sheets {
			let p = first + k as u32;
			if p > g.pages {
				break;
			}
			let x = g.left + k * (g.sheet_w + GAP);
			if x < w && x + g.sheet_w + 5 > 0 {
				gdi::paint_sheet(dc, (x, g.top), damage, g.pct, a, s, p, g.pages);
			}
		}
		RestoreDC(dc, saved);
	}
	let status = status_h();
	if status > 0 && i32::from(damage.bottom) > h - status {
		let mut text = [0u8; 96];
		let n = status_text(first, g.pages, &mut text);
		chrome::paint_status(dc, h - status, w, text.get(..n).unwrap_or(&[]), &[], 100);
	}
}
