// The window's behaviour: scrolling, the pointer in the page and on the toolbars, hints and tooltips, the print preview's bar, the
// recent-file menu and the settings file.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;
use core::ptr::{null, null_mut};

use quill_app::{Command, Grain};

use crate::chrome;
use crate::gdi;
use crate::sys::*;
use crate::view::{self, dev, log, DESK_PAD, LINE_STEP};
use crate::{client_size, shell, Shell};

/// Height of the bar of buttons that replaces the toolbars while the print preview shows.
pub const PV_BAR: i32 = 30;
/// Control ids of the preview's buttons.
pub const ID_PV_PRINT: usize = 401;
pub const ID_PV_NEXT: usize = 402;
pub const ID_PV_PREV: usize = 403;
pub const ID_PV_PAGES: usize = 404;
pub const ID_PV_ZOOM: usize = 405;
pub const ID_PV_CLOSE: usize = 406;
/// Menu ids of the recent files.
pub const ID_RECENT: usize = 600;
/// Timers: the pointer poll while a hint shows, and the scrolling while a selection is dragged past the edge.
pub const TIMER_HINT: usize = 2;
pub const TIMER_DRAG: usize = 3;

const WS_CHILD: u32 = 0x4000_0000;
const WS_TABSTOP: u32 = 0x0001_0000;
const BS_PUSHBUTTON: u32 = 0;
const WM_SETFONT: u32 = 0x0030;
const WM_SETTEXT: u32 = 0x000C;

/// A click in progress: when and where the last one was, and how many in a row, for double and triple clicks.
#[derive(Copy, Clone)]
pub struct Click {
	pub time: u32,
	pub x: i32,
	pub y: i32,
	pub count: u8,
}

/// A selection being dragged: the range the first click selected, and by what the drag grows it.
#[derive(Copy, Clone)]
pub struct SelDrag {
	pub start: (u32, u32),
	pub grain: Grain,
	/// The last pointer position, for the timer that scrolls while the pointer is outside the page.
	pub last: (i32, i32),
}

/// Which toolbar part the pointer is over, for the status line and the lit button.
#[derive(Copy, Clone, PartialEq)]
pub enum Hover {
	None,
	Button(usize),
	Combo(usize),
	Preview(usize),
}

/// The page area: under the toolbars and the ruler (or the preview's bar), above the status line.
pub unsafe fn page_area(hwnd: Handle, sh: &Shell) -> Rect {
	let (w, h) = client_size(hwnd);
	let status = if sh.app.show_status() { sh.ch } else { 0 };
	let (vw, hh) = bar_sizes();
	let top = crate::chrome_height(sh);
	Rect {
		left: 0,
		top,
		right: w - if sh.vbar_on { vw } else { 0 },
		bottom: (h - status - if sh.hbar_on { hh } else { 0 }).max(top),
	}
}

/// The width of a vertical scroll bar and the height of a horizontal one.
fn bar_sizes() -> (i32, i32) {
	// SM_CXVSCROLL and SM_CYHSCROLL.
	unsafe { (GetSystemMetrics(2).max(8), GetSystemMetrics(3).max(8)) }
}

/// Makes the two scroll bars as child windows, hidden: they belong to the page area and nowhere else, so they are controls placed in
/// it and not the window's own bars, which would run the whole height of the window beside the toolbars.
pub unsafe fn create_bars(hwnd: Handle, instance: Handle) -> (Handle, Handle) {
	let make = |style: u32| {
		CreateWindowExA(
			0,
			b"SCROLLBAR\0".as_ptr(),
			null(),
			WS_CHILD | style,
			0,
			0,
			10,
			10,
			hwnd,
			null_mut(),
			instance,
			null(),
		)
	};
	// SBS_HORZ is 0 and SBS_VERT 1.
	(make(1), make(0))
}

/// Puts the scroll bars along the right and bottom edges of the page area, and shows the ones that are needed.
unsafe fn place_bars(hwnd: Handle, sh: &Shell) {
	let area = page_area(hwnd, sh);
	let (vw, hh) = bar_sizes();
	MoveWindow(sh.vbar, area.right, area.top, vw, area.bottom - area.top, 1);
	ShowWindow(sh.vbar, if sh.vbar_on { 5 } else { 0 });
	MoveWindow(sh.hbar, area.left, area.bottom, area.right - area.left, hh, 1);
	ShowWindow(sh.hbar, if sh.hbar_on { 5 } else { 0 });
}

/// How many sheets of paper the body takes, as the last paint or scroll update found it.
fn pages(sh: &Shell) -> u32 {
	sh.pages.max(1)
}

/// The zoom now in percent, for a window `client_w` wide.
pub fn pct(sh: &Shell, client_w: i32) -> i32 {
	view::zoom_pct(&sh.app, client_w)
}

/// Keeps both scroll bars and both scroll positions inside what the window and the document allow.
pub unsafe fn update_scroll(hwnd: Handle, sh: &mut Shell) {
	if sh.grid {
		return;
	}
	// Which bars show decides the page area and the page area decides which bars show: settle it in two passes.
	let preview = sh.preview;
	sh.vbar_on = preview.is_none_or(|pv| pv.actual);
	sh.hbar_on = false;
	for _ in 0..2 {
		let area = page_area(hwnd, sh);
		let (aw, ah) = (area.right - area.left, area.bottom - area.top);
		let (hmax, vmax) = match preview {
			Some(pv) => {
				let (_, pwd, phd) = gdi::preview_scale(&sh.app, aw, ah, pv.two, pv.actual);
				let n = if pv.two { 2 } else { 1 };
				if pv.actual {
					((16 + n * pwd + (n - 1) * 16 + 16 - aw).max(0), (phd + 32 - ah).max(0))
				} else {
					(0, 0)
				}
			}
			None => (view::hscroll_max(&sh.app, aw), 1),
		};
		sh.hbar_on = hmax > 0;
		if preview.is_some() {
			sh.vbar_on = preview.is_some_and(|pv| pv.actual) && vmax > 0;
		}
	}
	place_bars(hwnd, sh);
	let area = page_area(hwnd, sh);
	let (aw, ah) = (area.right - area.left, area.bottom - area.top);
	if let Some(mut pv) = sh.preview {
		let (_, pwd, phd) = gdi::preview_scale(&sh.app, aw, ah, pv.two, pv.actual);
		let n = if pv.two { 2 } else { 1 };
		let (cw, chh) = (16 + n * pwd + (n - 1) * 16 + 16, phd + 32);
		let (hmax, vmax) = if pv.actual {
			((cw - aw).max(0), (chh - ah).max(0))
		} else {
			(0, 0)
		};
		pv.hscroll = pv.hscroll.clamp(0, hmax);
		pv.vscroll = pv.vscroll.clamp(0, vmax);
		sh.preview = Some(pv);
		set_bar(sh.hbar, hmax, aw, pv.hscroll, 1);
		set_bar(sh.vbar, vmax, ah, pv.vscroll, 1);
		return;
	}
	sh.pages = gdi::body_pages(&sh.app, &sh.store);
	let p = pct(sh, aw);
	let view_h = log(ah, p);
	let vmax = view::vscroll_max(&sh.app, sh.pages, view_h);
	sh.vpos = sh.vpos.clamp(0, vmax);
	let hmax = view::hscroll_max(&sh.app, aw);
	sh.hscroll = sh.hscroll.clamp(0, hmax);
	set_bar(sh.hbar, hmax, aw, sh.hscroll, 1);
	let k = view::bar_scale(vmax + view_h);
	set_bar(sh.vbar, vmax, view_h, sh.vpos, k);
}

/// Sets a scroll bar for positions 0 to `max` with a thumb of `page`, everything divided by `k` to fit 16 bits.
unsafe fn set_bar(bar: Handle, max: i32, page: i32, pos: i32, k: i32) {
	let info = ScrollInfo {
		size: core::mem::size_of::<ScrollInfo>() as u32,
		mask: SIF_ALL,
		min: 0,
		max: (max + page) / k - 1,
		page: (page / k).max(1) as u32,
		pos: pos / k,
		track_pos: 0,
	};
	// SB_CTL: the bar is a control of its own.
	SetScrollInfo(bar, 2, &info, 1);
}

/// The window changed size: lays the page, the ruler, the preview's bar and the scroll bars out again.
pub unsafe fn relayout(hwnd: Handle, sh: &mut Shell) {
	preview_layout(hwnd, sh);
	update_scroll(hwnd, sh);
	tips_rebuild(hwnd, sh);
	InvalidateRect(hwnd, null(), 0);
}

/// Scrolls the page down to `pos` page units and paints it again when that moved it.
pub unsafe fn scroll_v(hwnd: Handle, sh: &mut Shell, pos: i32) {
	if let Some(mut pv) = sh.preview {
		pv.vscroll = pos;
		sh.preview = Some(pv);
		update_scroll(hwnd, sh);
		InvalidateRect(hwnd, null(), 0);
		return;
	}
	let before = sh.vpos;
	sh.vpos = pos;
	update_scroll(hwnd, sh);
	if sh.vpos != before {
		crate::invalidate_page(hwnd, sh);
	}
}

/// Scrolls the page sideways to `pos` pixels.
pub unsafe fn scroll_h(hwnd: Handle, sh: &mut Shell, pos: i32) {
	if let Some(mut pv) = sh.preview {
		pv.hscroll = pos;
		sh.preview = Some(pv);
		update_scroll(hwnd, sh);
		InvalidateRect(hwnd, null(), 0);
		return;
	}
	let before = sh.hscroll;
	sh.hscroll = pos;
	update_scroll(hwnd, sh);
	if sh.hscroll != before {
		crate::invalidate_ruler(hwnd, sh);
		crate::invalidate_page(hwnd, sh);
	}
}

/// What one scroll bar message means as a new position. `now` is where it is, `page` how much a page of it is, `thumb` what the
/// thumb says (already multiplied back by the bar's scale).
fn bar_target(code: usize, now: i32, line: i32, page: i32, thumb: i32, max: i32) -> Option<i32> {
	Some(match code {
		SB_LINEUP => now - line,
		SB_LINEDOWN => now + line,
		SB_PAGEUP => now - page,
		SB_PAGEDOWN => now + page,
		SB_THUMBTRACK | SB_THUMBPOSITION => thumb,
		SB_TOP => 0,
		SB_BOTTOM => max,
		_ => return None,
	})
}

/// A `WM_VSCROLL`: `code` is what the user did and `thumb` is where the box is.
pub unsafe fn vscroll_by(hwnd: Handle, sh: &mut Shell, code: usize, thumb: i32) {
	let area = page_area(hwnd, sh);
	let (aw, ah) = (area.right - area.left, area.bottom - area.top);
	if let Some(pv) = sh.preview {
		let line = LINE_STEP;
		if let Some(to) = bar_target(code, pv.vscroll, line, ah - line, thumb, i32::MAX / 2) {
			scroll_v(hwnd, sh, to);
		}
		return;
	}
	let p = pct(sh, aw);
	let view_h = log(ah, p);
	let vmax = view::vscroll_max(&sh.app, pages(sh), view_h);
	let k = view::bar_scale(vmax + view_h);
	let line = log(LINE_STEP * 3, p);
	if let Some(to) = bar_target(code, sh.vpos, line, view_h - line, thumb * k, vmax) {
		scroll_v(hwnd, sh, to);
	}
}

/// A `WM_HSCROLL`.
pub unsafe fn hscroll_by(hwnd: Handle, sh: &mut Shell, code: usize, thumb: i32) {
	let area = page_area(hwnd, sh);
	let w = area.right - area.left;
	let now = match sh.preview {
		Some(pv) => pv.hscroll,
		None => sh.hscroll,
	};
	if let Some(to) = bar_target(code, now, LINE_STEP, w - LINE_STEP, thumb, i32::MAX / 2) {
		scroll_h(hwnd, sh, to);
	}
}

/// The mouse wheel turned `delta` (120 to the notch, positive away from the user): three lines a notch, or the pages of the preview.
pub unsafe fn wheel(hwnd: Handle, sh: &mut Shell, delta: i32) {
	if sh.preview.is_some() {
		if let Some(pv) = sh.preview {
			if pv.actual {
				scroll_v(hwnd, sh, pv.vscroll - delta * 3 * LINE_STEP / 120);
			} else {
				preview_step(hwnd, sh, if delta > 0 { -1 } else { 1 });
			}
		}
		return;
	}
	let area = page_area(hwnd, sh);
	let p = pct(sh, area.right - area.left);
	let step = log(delta * 3 * LINE_STEP / 120, p);
	let to = sh.vpos - step;
	scroll_v(hwnd, sh, to);
}

/// Scrolls so the caret is in view, with a little room above and below it, and sideways when the page is wider than the window.
pub unsafe fn ensure_visible(hwnd: Handle, sh: &mut Shell) {
	if sh.grid || sh.preview.is_some() || !sh.app.editing() {
		return;
	}
	let area = page_area(hwnd, sh);
	let (aw, ah) = (area.right - area.left, area.bottom - area.top);
	let p = pct(sh, aw);
	let view_h = log(ah, p);
	let stride = view::stride(&sh.app);
	let Some((x, top, h)) = sh.app.point_of(&sh.store, sh.app.caret(), stride) else {
		return;
	};
	let margin = (view_h / 4).min(48);
	let (t, b) = (top + DESK_PAD, top + DESK_PAD + h);
	let mut to = sh.vpos;
	if t - to < margin {
		to = t - margin;
	} else if b - to > view_h - margin {
		to = b - view_h + margin;
	}
	let mut hs = sh.hscroll;
	if view::hscroll_max(&sh.app, aw) > 0 {
		let at = view::PAGE_PAD + dev(x, p);
		let slack = 32.min(aw / 4);
		if at - hs < slack {
			hs = at - slack;
		} else if at - hs > aw - slack {
			hs = at - (aw - slack);
		}
	}
	let (before_v, before_h) = (sh.vpos, sh.hscroll);
	sh.vpos = to;
	sh.hscroll = hs;
	update_scroll(hwnd, sh);
	if (before_v, before_h) != (sh.vpos, sh.hscroll) {
		crate::invalidate_ruler(hwnd, sh);
		crate::invalidate_page(hwnd, sh);
	}
}

/// Page, in page units, the pixel (`x`, `y`) of the window is over, measured from the first sheet's top left corner: the layout's own
/// space, where the core's hit-testing works.
pub unsafe fn to_page(hwnd: Handle, sh: &Shell, x: i32, y: i32) -> (i32, i32) {
	let area = page_area(hwnd, sh);
	let aw = area.right - area.left;
	let p = pct(sh, aw);
	let x0 = view::page_x0(aw, dev(view::paper(&sh.app).0, p), sh.hscroll);
	(log(x - x0, p), log(y - area.top, p) + sh.vpos - DESK_PAD)
}

/// True when the pixel is over the page area.
pub unsafe fn in_page(hwnd: Handle, sh: &Shell, x: i32, y: i32) -> bool {
	let a = page_area(hwnd, sh);
	x >= a.left && x < a.right && y >= a.top && y < a.bottom
}

/// Moves the vertical scroll, and the caret by the same distance, as Page Up and Page Down do: the caret keeps its place in the
/// window. At the end of the text the view still moves a screenful, so the pages and the desk below the last line can be reached.
pub unsafe fn page_key(hwnd: Handle, sh: &mut Shell, down: bool, extend: bool) {
	let area = page_area(hwnd, sh);
	let (aw, ah) = (area.right - area.left, area.bottom - area.top);
	let p = pct(sh, aw);
	let view_h = log(ah, p);
	let stride = view::stride(&sh.app);
	let y0 = sh.app.point_of(&sh.store, sh.app.caret(), stride).map_or(0, |(_, t, _)| t);
	sh.app.set_rows(((view_h / 16).max(2)) as usize);
	sh.app.move_page(&sh.store, down, extend);
	let y1 = sh.app.point_of(&sh.store, sh.app.caret(), stride).map_or(0, |(_, t, _)| t);
	let screen = (view_h - 32).max(16);
	let moved = y1 - y0;
	// The view goes with the caret by as much as it moved; when the caret could not move (it was already at an end), by a screenful.
	let by = if moved == 0 {
		if down {
			screen
		} else {
			-screen
		}
	} else {
		moved
	};
	sh.vpos += by;
	update_scroll(hwnd, sh);
	ensure_visible(hwnd, sh);
	InvalidateRect(hwnd, null(), 0);
}

// ---- the pointer in the page -------------------------------------------------------------------------------------------------------

/// The page position under a pointer at (`x`, `y`).
unsafe fn pos_under(hwnd: Handle, sh: &Shell, x: i32, y: i32) -> u32 {
	let (px, py) = to_page(hwnd, sh, x, y);
	sh.app.pos_at_point(&sh.store, px, py, view::stride(&sh.app))
}

/// True when (`x`, `y`) is in the white space left of the text, where a click selects a line.
unsafe fn in_left_margin(hwnd: Handle, sh: &Shell, x: i32, y: i32) -> bool {
	let (px, _) = to_page(hwnd, sh, x, y);
	let margin = quill_core::layout::to_units(sh.app.doc().page.margin_l, 96);
	px < margin - 4 && px >= -dev(view::PAGE_PAD, 100)
}

/// A press of the left button in the page: places the caret, extends the selection, or starts a selection by word, line or paragraph.
pub unsafe fn page_down(hwnd: Handle, sh: &mut Shell, x: i32, y: i32) {
	if !sh.app.editing() {
		return;
	}
	let shift = GetKeyState(VK_SHIFT) < 0;
	let now = GetTickCount();
	let near = |c: &Click| (c.x - x).abs() <= 4 && (c.y - y).abs() <= 4;
	let count = match sh.click {
		Some(c) if now.wrapping_sub(c.time) <= GetDoubleClickTime() && near(&c) => c.count.min(2) + 1,
		_ => 1,
	};
	sh.click = Some(Click { time: now, x, y, count });
	let margin = in_left_margin(hwnd, sh, x, y);
	let pos = pos_under(hwnd, sh, x, y);
	let grain = match (margin, count) {
		(true, 1) => Grain::Line,
		(true, _) => Grain::Paragraph,
		(false, 1) => Grain::Char,
		(false, 2) => Grain::Word,
		(false, _) => Grain::Paragraph,
	};
	let start = if grain == Grain::Char {
		(pos, pos)
	} else {
		sh.app.grain_range(&sh.store, pos, grain)
	};
	if grain == Grain::Char {
		sh.app.place_caret(&sh.store, pos, shift);
	} else {
		sh.app.select_range(&sh.store, start.0, start.1);
	}
	sh.sel = Some(SelDrag {
		start: if grain == Grain::Char && shift {
			sh.app.selection().unwrap_or(start)
		} else {
			start
		},
		grain,
		last: (x, y),
	});
	SetCapture(hwnd);
	SetTimer(hwnd, TIMER_DRAG, 50, null());
	ensure_visible(hwnd, sh);
	InvalidateRect(hwnd, null(), 0);
}

/// The pointer moved with the button down in the page: grows the selection to the character under it, and scrolls when it is past the
/// top or bottom of the page area.
pub unsafe fn page_drag(hwnd: Handle, sh: &mut Shell, x: i32, y: i32) {
	let Some(mut d) = sh.sel else {
		return;
	};
	d.last = (x, y);
	sh.sel = Some(d);
	let area = page_area(hwnd, sh);
	let p = pct(sh, area.right - area.left);
	if y < area.top {
		sh.vpos -= log(((area.top - y) / 2).max(4), p);
	} else if y >= area.bottom {
		sh.vpos += log(((y - area.bottom) / 2).max(4), p);
	}
	update_scroll(hwnd, sh);
	let pos = pos_under(hwnd, sh, x, y.clamp(area.top, area.bottom - 1));
	sh.app.drag_select(&sh.store, d.start, pos, d.grain);
	InvalidateRect(hwnd, null(), 0);
}

/// The button went up: the selection is what it is.
pub unsafe fn page_up(hwnd: Handle, sh: &mut Shell) {
	if sh.sel.take().is_some() {
		KillTimer(hwnd, TIMER_DRAG);
		ReleaseCapture();
	}
}

// ---- hints and tooltips ------------------------------------------------------------------------------------------------------------

/// The sentence for a part of the window under the pointer.
fn hint_text(sh: &Shell, h: Hover) -> &'static str {
	match h {
		Hover::None => "",
		Hover::Button(i) => match chrome::layout().buttons.get(i).map(|b| b.act) {
			Some(chrome::Act::Cmd(c)) => quill_app::command_hint(c),
			Some(chrome::Act::Colour) => "Choose a colour for the selected text",
			_ => "Check the spelling of the document",
		},
		Hover::Combo(0) => "Choose a paragraph style",
		Hover::Combo(1) => "Choose a font",
		Hover::Combo(_) => "Choose a size for the text",
		Hover::Preview(i) => {
			let _ = sh;
			match i {
				0 => "Print the document",
				1 => "Show the next page",
				2 => "Show the previous page",
				3 => "Show one page or two pages at a time",
				4 => "Switch between the whole page and its actual size",
				_ => "Close print preview and go back to the document",
			}
		}
	}
}

/// What a toolbar button or a drop-down box is called in its tooltip.
fn tip_text(sh: &Shell, id: usize, is_window: bool) -> Vec<u8> {
	if is_window {
		let h = id as Handle;
		if let Some(cb) = &sh.combos {
			if h == cb.style {
				return b"Style".to_vec();
			}
			if h == cb.face {
				return b"Font".to_vec();
			}
			if h == cb.size {
				return b"Font Size".to_vec();
			}
		}
		let names: [&[u8]; 6] = [
			b"Print",
			b"Next Page",
			b"Previous Page",
			b"One Page or Two Pages",
			b"Zoom",
			b"Close",
		];
		return sh
			.pv_btns
			.iter()
			.position(|b| *b == h)
			.and_then(|i| names.get(i))
			.map_or_else(Vec::new, |n| n.to_vec());
	}
	match chrome::layout().buttons.get(id).map(|b| b.act) {
		Some(chrome::Act::Cmd(c)) => quill_app::command_tip(c).as_bytes().to_vec(),
		Some(chrome::Act::Colour) => b"Font Colour".to_vec(),
		_ => b"Spelling".to_vec(),
	}
}

/// Answers a tooltip's request for text (`WM_NOTIFY`).
pub unsafe fn notify(sh: &Shell, lparam: isize) -> bool {
	crate::tips::answer(lparam, |id, w| tip_text(sh, id, w))
}

/// Gives the tooltip control a tool for each button that shows now: the toolbar's painted buttons, the drop-down boxes, or the
/// preview's push buttons.
pub unsafe fn tips_rebuild(hwnd: Handle, sh: &mut Shell) {
	let Some(mut tips) = sh.tips.take() else {
		return;
	};
	tips.clear(hwnd);
	if sh.preview.is_some() {
		for b in sh.pv_btns {
			tips.add_window(hwnd, b);
		}
	} else if !sh.grid {
		let n = chrome::layout().buttons.len();
		for i in 0..n {
			if let Some((r, b)) = chrome::button_rect(&sh.app, i) {
				let shown = if b.row == 0 {
					sh.app.show_toolbar()
				} else {
					sh.app.show_format_bar()
				};
				if shown {
					tips.add_rect(hwnd, i, r);
				}
			}
		}
		if sh.app.show_format_bar() {
			if let Some(cb) = &sh.combos {
				for h in [cb.style, cb.face, cb.size] {
					tips.add_window(hwnd, h);
				}
			}
		}
	}
	sh.tips = Some(tips);
}

/// Shows (or, with `Hover::None`, takes away) the hint for what the pointer is over, on the status line, and keeps a timer going to
/// notice the pointer leaving the window.
pub unsafe fn set_hover(hwnd: Handle, sh: &mut Shell, h: Hover) {
	let changed_button = match (sh.hover, h) {
		(Hover::Button(a), Hover::Button(b)) => a != b,
		(Hover::Button(_), _) | (_, Hover::Button(_)) => true,
		_ => false,
	};
	if sh.hover == h {
		return;
	}
	sh.hover = h;
	if h == Hover::None {
		KillTimer(hwnd, TIMER_HINT);
		if sh.hint_on {
			sh.app.set_message("");
			sh.hint_on = false;
		}
	} else {
		SetTimer(hwnd, TIMER_HINT, 120, null());
		sh.app.set_message(hint_text(sh, h));
		sh.hint_on = true;
	}
	if changed_button {
		invalidate_toolbar(hwnd, sh);
	}
	invalidate_status(hwnd, sh);
}

unsafe fn invalidate_toolbar(hwnd: Handle, sh: &Shell) {
	let (w, _) = client_size(hwnd);
	let (h0, h1) = chrome::rows(&sh.app);
	let r = Rect {
		left: 0,
		top: 0,
		right: w,
		bottom: h0 + h1,
	};
	InvalidateRect(hwnd, &r, 0);
}

/// Asks for the status line to be painted again.
pub unsafe fn invalidate_status(hwnd: Handle, sh: &Shell) {
	if !sh.app.show_status() {
		return;
	}
	let (w, h) = client_size(hwnd);
	let r = Rect {
		left: 0,
		top: h - sh.ch,
		right: w,
		bottom: h,
	};
	InvalidateRect(hwnd, &r, 0);
}

/// Works out what the pointer at client position (`x`, `y`) is over, among the toolbar's buttons.
pub unsafe fn hover_at(sh: &Shell, x: i32, y: i32) -> Hover {
	if sh.preview.is_some() {
		return Hover::None;
	}
	match chrome::button_index(&sh.app, x, y) {
		Some((i, _)) => Hover::Button(i),
		None => Hover::None,
	}
}

/// The polling timer: clears the hint when the pointer has left the window or the buttons it was over.
pub unsafe fn poll_hover(hwnd: Handle, sh: &mut Shell) {
	let mut p = Point { x: 0, y: 0 };
	GetCursorPos(&mut p);
	let mut origin = Point { x: 0, y: 0 };
	ClientToScreen(hwnd, &mut origin);
	let (x, y) = (p.x - origin.x, p.y - origin.y);
	let (w, h) = client_size(hwnd);
	let inside = x >= 0 && y >= 0 && x < w && y < h;
	match sh.hover {
		Hover::Button(_) => {
			let now = if inside { hover_at(sh, x, y) } else { Hover::None };
			if now != sh.hover {
				set_hover(hwnd, sh, now);
			}
		}
		Hover::Combo(_) | Hover::Preview(_) => {
			if !inside {
				set_hover(hwnd, sh, Hover::None);
			}
		}
		Hover::None => {
			KillTimer(hwnd, TIMER_HINT);
		}
	}
}

/// A child window under the pointer, as `WM_SETCURSOR` reports it: the hover it stands for.
pub fn hover_of_child(sh: &Shell, child: Handle) -> Hover {
	if let Some(cb) = &sh.combos {
		if sh.preview.is_none() {
			if child == cb.style {
				return Hover::Combo(0);
			}
			if child == cb.face {
				return Hover::Combo(1);
			}
			if child == cb.size {
				return Hover::Combo(2);
			}
		}
	}
	match sh.pv_btns.iter().position(|b| *b == child) {
		Some(i) if sh.preview.is_some() => Hover::Preview(i),
		_ => Hover::None,
	}
}

// ---- the print preview ---------------------------------------------------------------------------------------------------------------

/// Makes the preview's push buttons, hidden, as children of the window.
pub unsafe fn create_preview_buttons(hwnd: Handle, instance: Handle) -> [Handle; 6] {
	let font = GetStockObject(17);
	let mut out = [null_mut(); 6];
	let labels: [&[u8]; 6] = [
		b"&Print\0",
		b"&Next Page\0",
		b"Pre&vious Page\0",
		b"&Two Pages\0",
		b"&Zoom\0",
		b"&Close\0",
	];
	for (i, label) in labels.iter().enumerate() {
		let h = CreateWindowExA(
			0,
			b"BUTTON\0".as_ptr(),
			label.as_ptr(),
			WS_CHILD | WS_TABSTOP | BS_PUSHBUTTON,
			0,
			0,
			10,
			10,
			hwnd,
			(ID_PV_PRINT + i) as Handle,
			instance,
			null(),
		);
		SendMessageA(h, WM_SETFONT, font as usize, 0);
		if let Some(slot) = out.get_mut(i) {
			*slot = h;
		}
	}
	out
}

/// Puts the preview's buttons in their bar (or hides them when the preview is not showing), and the toolbars' drop-down boxes away
/// while it is.
pub unsafe fn preview_layout(_hwnd: Handle, sh: &mut Shell) {
	let showing = sh.preview.is_some();
	let widths = [60, 84, 100, 92, 64, 64];
	let mut x = 6;
	for (h, w) in sh.pv_btns.iter().zip(widths) {
		if showing {
			MoveWindow(*h, x, 3, w, 24, 1);
		}
		ShowWindow(*h, if showing { 5 } else { 0 });
		x += w + 4;
	}
	if let Some(cb) = &sh.combos {
		cb.show(&sh.app, showing);
	}
	if let Some(pv) = sh.preview {
		let label: &[u8] = if pv.two { b"One &Page\0" } else { b"&Two Pages\0" };
		SendMessageA(sh.pv_btns[3], WM_SETTEXT, 0, label.as_ptr() as isize);
		let zoom: &[u8] = if pv.actual { b"Whole &Page\0" } else { b"&Zoom\0" };
		SendMessageA(sh.pv_btns[4], WM_SETTEXT, 0, zoom.as_ptr() as isize);
	}
}

/// Opens the print preview at page `first`, leaving the page's own scroll position untouched for when it closes.
pub unsafe fn preview_open(hwnd: Handle, sh: &mut Shell, first: u32, two: bool) {
	sh.preview = Some(gdi::PreviewView {
		first,
		two,
		actual: false,
		hscroll: 0,
		vscroll: 0,
	});
	set_hover(hwnd, sh, Hover::None);
	preview_layout(hwnd, sh);
	update_scroll(hwnd, sh);
	tips_rebuild(hwnd, sh);
	SetFocus(hwnd);
	InvalidateRect(hwnd, null(), 0);
}

/// Closes the preview and puts the window as it was: the page at the scroll position it had, centred or not as before.
pub unsafe fn preview_close(hwnd: Handle, sh: &mut Shell) {
	sh.preview = None;
	set_hover(hwnd, sh, Hover::None);
	preview_layout(hwnd, sh);
	update_scroll(hwnd, sh);
	tips_rebuild(hwnd, sh);
	SetFocus(hwnd);
	InvalidateRect(hwnd, null(), 0);
}

/// Shows the page `step` pages on (or back, when negative) from the first one shown.
pub unsafe fn preview_step(hwnd: Handle, sh: &mut Shell, dir: i32) {
	let Some(mut pv) = sh.preview else {
		return;
	};
	let (_, _, total) = gdi::preview_pages(&sh.app, &sh.store, sh.screen_dc);
	let step = if pv.two { 2 } else { 1 };
	let next = (pv.first as i32 + dir * step).clamp(1, total as i32) as u32;
	pv.first = next;
	pv.vscroll = 0;
	sh.preview = Some(pv);
	update_scroll(hwnd, sh);
	InvalidateRect(hwnd, null(), 0);
}

/// A click on one of the preview's buttons.
pub unsafe fn preview_command(hwnd: Handle, sh: &mut Shell, id: usize) {
	let Some(mut pv) = sh.preview else {
		return;
	};
	match id {
		ID_PV_PRINT => {
			if !crate::print_job(hwnd, sh, true, None) {
				sh.app.set_message("Quill could not print.");
			}
		}
		ID_PV_NEXT => preview_step(hwnd, sh, 1),
		ID_PV_PREV => preview_step(hwnd, sh, -1),
		ID_PV_PAGES => {
			pv.two = !pv.two;
			sh.preview = Some(pv);
			preview_layout(hwnd, sh);
		}
		ID_PV_ZOOM => {
			pv.actual = !pv.actual;
			pv.hscroll = 0;
			pv.vscroll = 0;
			sh.preview = Some(pv);
			preview_layout(hwnd, sh);
		}
		ID_PV_CLOSE => {
			preview_close(hwnd, sh);
			return;
		}
		_ => {}
	}
	update_scroll(hwnd, sh);
	SetFocus(hwnd);
	InvalidateRect(hwnd, null(), 0);
}

// ---- the recent files in the menu ----------------------------------------------------------------------------------------------------

/// Rewrites the recent files in the File menu: numbered items between the Print group and Exit, each with a mnemonic digit and a path
/// shortened to fit, set off by rules.
pub unsafe fn sync_recent_menu(hwnd: Handle, sh: &mut Shell) {
	let menu = GetMenu(hwnd);
	if menu.is_null() {
		return;
	}
	let file = GetSubMenu(menu, 0);
	if file.is_null() {
		return;
	}
	// Take the old ones away, and the rule that closed them.
	for i in 0..quill_app::recent::RECENT_MAX {
		DeleteMenu(file, (ID_RECENT + i) as u32, MF_BYCOMMAND);
	}
	let count = GetMenuItemCount(file);
	if sh.recent_rule && count > 1 {
		DeleteMenu(file, (count - 2) as u32, MF_BYPOSITION);
	}
	sh.recent_rule = false;
	let mut pos = (GetMenuItemCount(file) - 1).max(0) as u32;
	let mut any = false;
	let names: Vec<Vec<u8>> = sh.app.recent_files().map(<[u8]>::to_vec).collect();
	for (i, name) in names.iter().enumerate() {
		let mut short = [0u8; 48];
		let n = quill_app::recent::shorten_path(name, 40, &mut short);
		let mut text = alloc::format!("&{} ", i + 1).into_bytes();
		for b in short.get(..n).unwrap_or(&[]) {
			text.push(*b);
			if *b == b'&' {
				text.push(b'&');
			}
		}
		text.push(0);
		InsertMenuA(file, pos, MF_BYPOSITION | MF_STRING, ID_RECENT + i, text.as_ptr());
		pos += 1;
		any = true;
	}
	if any {
		InsertMenuA(file, pos, MF_BYPOSITION | MF_SEPARATOR, 0, null());
		sh.recent_rule = true;
	}
}

/// The status line hint for the menu item `id`, or an empty one: the sentence for the command behind it.
pub fn menu_hint(sh: &Shell, id: usize) -> &'static str {
	if (ID_RECENT..ID_RECENT + quill_app::recent::RECENT_MAX).contains(&id) {
		return quill_app::command_hint(Command::Recent(0));
	}
	id.checked_sub(100)
		.and_then(|i| sh.cmds.get(i))
		.map_or("", |c| quill_app::command_hint(*c))
}

// ---- the settings file ---------------------------------------------------------------------------------------------------------------

const INI: &[u8] = b"QUILL.INI\0";

fn z(text: &[u8]) -> Vec<u8> {
	let mut v = text.to_vec();
	v.push(0);
	v
}

/// Reads the settings `QUILL.INI` in the Windows directory holds (the SPEC's settings file) into the editor.
pub unsafe fn ini_load(sh: &mut Shell) {
	let recent = z(b"Recent32");
	let keys: [(&[u8], &[u8]); 11] = [
		(b"Recent32", b"File1"),
		(b"Recent32", b"File2"),
		(b"Recent32", b"File3"),
		(b"Recent32", b"File4"),
		(b"Quill", b"Language"),
		(b"Quill", b"Folder"),
		(b"Quill", b"Zoom"),
		(b"Quill", b"Toolbar"),
		(b"Quill", b"Ruler"),
		(b"Quill", b"StatusBar"),
		(b"Quill", b"FormattingBar"),
	];
	let _ = recent;
	for (section, key) in keys {
		let mut buf = [0u8; 264];
		let n = GetPrivateProfileStringA(
			z(section).as_ptr(),
			z(key).as_ptr(),
			b"\0".as_ptr(),
			buf.as_mut_ptr(),
			260,
			INI.as_ptr(),
		) as usize;
		if n > 0 {
			sh.app.apply_setting(section, key, buf.get(..n).unwrap_or(&[]));
		}
	}
}

/// Writes the editor's settings to `QUILL.INI`, one key at a time, so the other sections the Win16 build keeps there stay as they are.
pub unsafe fn ini_save(sh: &Shell) {
	sh.app.each_setting(|section, key, value| {
		WritePrivateProfileStringA(z(section).as_ptr(), z(key).as_ptr(), z(value).as_ptr(), INI.as_ptr());
	});
	// The recent files that are no longer on the list.
	let have = sh.app.recent_files().count();
	for i in have..quill_app::recent::RECENT_MAX {
		let key = alloc::format!("File{}\0", i + 1);
		WritePrivateProfileStringA(b"Recent32\0".as_ptr(), key.as_ptr(), null(), INI.as_ptr());
	}
}

/// Gives the editor the date and the time as the system shows them, for the Date and Time fields: the short date and the time without
/// seconds in the user's own formats (`GetDateFormatA` and `GetTimeFormatA` with no time given mean now).
pub unsafe fn refresh_clock(sh: &mut Shell) {
	// LOCALE_USER_DEFAULT, DATE_SHORTDATE and TIME_NOSECONDS.
	let mut buf = [0u8; 64];
	let n = GetDateFormatA(0x0400, 1, null(), null(), buf.as_mut_ptr(), 64);
	if n > 1 {
		if let Ok(t) = core::str::from_utf8(buf.get(..n as usize - 1).unwrap_or(&[])) {
			sh.app.set_print_date(t);
		}
	}
	let n = GetTimeFormatA(0x0400, 2, null(), null(), buf.as_mut_ptr(), 64);
	if n > 1 {
		if let Ok(t) = core::str::from_utf8(buf.get(..n as usize - 1).unwrap_or(&[])) {
			sh.app.set_print_time(t);
		}
	}
}

/// Used to reach the shell from a callback.
pub fn with_shell(f: impl FnOnce(&mut Shell)) {
	if let Some(sh) = shell() {
		f(sh);
	}
}
