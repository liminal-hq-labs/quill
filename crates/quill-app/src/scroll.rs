// The editor's scroll state: the page view's vertical geometry and the methods of `App` that scroll, hit-test and click in it.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// `App`'s scroll fields are private, so the methods that use them stay beside it. The free geometry of the window around them lives in
// `quill_chrome::view`, which re-exports the constants and `PageGeom` from here.
use quill_core::layout::{to_units, Line, Measure};
use quill_core::store::ReadStore;

use crate::App;

/// The grey gap between pages, and above the first sheet at the scroll origin.
pub const PAGE_GAP: i32 = 10;
pub const PAGE_TOP: i32 = 8;
/// What an arrow of the vertical scroll bar moves the page, a notch of the wheel three times, in pixels.
pub const SCROLL_LINE: i32 = 16;
/// How far a sheet's shadow reaches below and to the right of it.
pub const SHEET_SHADOW: i32 = 5;
/// Pixels per inch of the page view.
pub const DPI: u16 = 96;

/// The page view's vertical geometry: pages one under another with a grey gap. At the scroll origin the first sheet's top is `PAGE_TOP`
/// below the area's top, with its top margin showing, as in Word's print layout (CORE-233).
#[derive(Copy, Clone, Debug)]
pub struct PageGeom {
	pub page_h: i32,
	pub margin_t: i32,
	/// The y of the area's top (less `PAGE_TOP`), in the pages' own coordinates: 0 is the top of the first sheet.
	pub top: i32,
}

impl PageGeom {
	/// The geometry for the document as it is scrolled now.
	pub fn of<M: Measure + Clone, S: ReadStore + ?Sized>(app: &App<M>, s: &S) -> Self {
		let page = &app.doc().page;
		let mut g = PageGeom {
			page_h: to_units(page.paper_h, DPI),
			margin_t: to_units(page.margin_t, DPI),
			top: 0,
		};
		// With the first line at the top, `vsub` counts from the top of the sheet (the margin shows); from any later line it counts from
		// that line's top.
		let base = match app.top_line() {
			0 => 0,
			first => app.line_at(s, first).map_or(0, |l| g.cum(&l)),
		};
		g.top = base + app.vsub;
		g
	}

	/// A line's top in the pages' own coordinates.
	pub fn cum(&self, l: &Line) -> i32 {
		(l.page as i32 - 1) * (self.page_h + PAGE_GAP) + self.margin_t + l.y
	}

	/// A line's top in the window, for a page area whose top is `area_top`.
	pub fn line_y(&self, l: &Line, area_top: i32) -> i32 {
		self.cum(l) - self.top + PAGE_TOP + area_top
	}

	/// The top of page `p` (from 0) in the window.
	pub fn page_y(&self, p: i32, area_top: i32) -> i32 {
		p * (self.page_h + PAGE_GAP) - (self.top - PAGE_TOP) + area_top
	}
}

impl<M: Measure + Clone> App<M> {
	/// Tells scrolling how many pixels high the page area is (`view::PageGeom`'s layout), so the caret's line is kept inside it.
	pub fn set_view_height(&mut self, pixels: i32) {
		self.view_h = pixels.max(0);
	}

	/// How far the page is scrolled sideways past the text column at the left edge, in pixels.
	pub fn hscroll_pos(&self) -> i32 {
		self.hpos
	}

	/// Sets the sideways scroll (never negative); the shell keeps it within what the content needs.
	pub fn set_hscroll_pos(&mut self, pos: i32) {
		self.hpos = pos.max(0);
	}

	/// True when the caret or the text has changed since the sideways scroll last followed the caret.
	pub fn caret_moved_since_follow(&self) -> bool {
		self.hseen != (self.caret, self.doc.body.revision())
	}

	/// Records that the sideways scroll has followed the caret as it is now.
	pub fn mark_followed(&mut self) {
		self.hseen = (self.caret, self.doc.body.revision());
	}

	/// The furthest right any of the `count` lines from the first one shown reaches, from the left margin, in pixels: a picture or a
	/// tab wider than the column pushes it past the column's width.
	pub fn content_right<S: ReadStore + ?Sized>(&self, s: &S, count: u32) -> i32 {
		let first = self.top;
		let mut right = 0;
		self.for_each_line_from(s, first, |i, l| {
			if i >= first.saturating_add(count) {
				return false;
			}
			if i >= first {
				right = right.max(l.x + l.width);
			}
			true
		});
		right
	}

	/// The first line to show so that line `idx` (`l`), at or after the current first line, fits in the page area: the current one when
	/// it already does.
	pub(crate) fn top_to_show<S: ReadStore + ?Sized>(&self, s: &S, idx: u32, l: &Line) -> u32 {
		let g = PageGeom::of(self, s);
		let room = self.view_h - PAGE_TOP;
		let need = g.cum(l) + (l.ascent + l.descent).max(1) - room;
		if need <= g.top {
			return self.top;
		}
		let mut top = idx;
		let from = self.top;
		self.lines_from(s, from, |i, line| {
			if i < from {
				return true;
			}
			if i >= idx {
				return false;
			}
			if g.cum(line) >= need {
				top = i;
				return false;
			}
			true
		});
		top
	}

	/// How many lines the body has, laid out for the app's device.
	pub fn line_total<S: ReadStore + ?Sized>(&self, s: &S) -> u32 {
		self.line_count(s)
	}

	/// How many lines, from the first one shown, fit whole in the page area (at least one): what a vertical scroll bar's thumb stands
	/// for. A heading, a picture or a larger type makes a line taller than the text screen's row, so this is fewer than the rows the
	/// shell set; a bar that used the rows would call a document of a few tall lines "nothing to scroll".
	pub fn lines_in_view<S: ReadStore + ?Sized>(&self, s: &S) -> u32 {
		let g = PageGeom::of(self, s);
		let room = self.view_h - PAGE_TOP;
		let first = self.top;
		let mut n = 0u32;
		self.lines_from(s, first, |i, l| {
			if i < first {
				return true;
			}
			if g.cum(l) + (l.ascent + l.descent).max(1) - g.top > room {
				return false;
			}
			n += 1;
			true
		});
		n.max(1)
	}

	/// Shows line `top` first (a scroll bar moved). The caret stays where it is until a key moves it.
	pub fn scroll_to<S: ReadStore + ?Sized>(&mut self, s: &S, top: u32) {
		self.top = top.min(self.line_count(s).saturating_sub(1));
		self.vsub = 0;
	}

	/// Where the page is scrolled to in pixels: 0 with the top of the first sheet `PAGE_TOP` below the top of the page area, and a pixel
	/// more for each the pages have moved up past it. A vertical scroll bar of the page view stands on this, not on lines, so a page with one line
	/// (an empty one) still scrolls and the whole of its sheet comes into view.
	pub fn vscroll_pos<S: ReadStore + ?Sized>(&self, s: &S) -> i32 {
		PageGeom::of(self, s).top.max(0)
	}

	/// How far the page can scroll ([`App::vscroll_pos`]'s greatest value): until the bottom of the last sheet, with its shadow and
	/// a little desk, is at the bottom of the page area. 0 when the page area's height is not known yet, and never less than where the
	/// page is now (the caret can scroll past it, onto the desk).
	pub fn vscroll_max<S: ReadStore + ?Sized>(&self, s: &S) -> i32 {
		if self.view_h <= 0 {
			return 0;
		}
		let g = PageGeom::of(self, s);
		let pages = self.page_total(s).max(1) as i32;
		let bottom = (pages - 1) * (g.page_h + PAGE_GAP) + g.page_h;
		let end = bottom + SHEET_SHADOW + PAGE_TOP - (self.view_h - PAGE_TOP);
		end.max(g.top).max(0)
	}

	/// True when the page area is drawn in pixels (a window or the graphics screen), so the page scrolls by [`App::vscroll_pos`];
	/// false on the text screen, which scrolls by lines.
	pub fn pixel_view(&self) -> bool {
		self.view_h > 0
	}

	/// How many pixels of the page area the thumb of a bar on [`App::vscroll_pos`] stands for: the page area's height.
	pub fn vscroll_view(&self) -> i32 {
		self.view_h.max(1)
	}

	/// Scrolls the page to `pos` pixels (see [`App::vscroll_pos`]), within 0 and [`App::vscroll_max`]: the first line shown is the last
	/// one that starts at or above it (the first line counts from the top of its sheet) and the rest is the pixels past that. The caret stays where it is. Returns true when
	/// the view moved.
	pub fn set_vscroll_pos<S: ReadStore + ?Sized>(&mut self, s: &S, pos: i32) -> bool {
		let pos = pos.clamp(0, self.vscroll_max(s));
		let g = PageGeom::of(self, s);
		if self.line_at(s, 0).is_none() {
			return false;
		}
		let (before_top, before_sub) = (self.top, self.vsub);
		let goal = pos;
		// Start a page before the one the goal is on: checkpoints are per page.
		let page = ((goal / (g.page_h + PAGE_GAP).max(1)) as u32).max(1);
		let mut found = (0u32, 0);
		self.lines_from_page(s, page, |i, l| {
			let c = g.cum(l);
			if c > goal {
				return false;
			}
			found = (i, c);
			true
		});
		// From the first line the count starts at the top of the sheet, not at the line.
		if found.0 == 0 {
			found.1 = 0;
		}
		self.top = found.0;
		self.vsub = goal - found.1;
		(self.top, self.vsub) != (before_top, before_sub)
	}

	/// Scrolls the page by `delta` pixels (negative up) as a bar does; true when it moved.
	pub fn vscroll_by<S: ReadStore + ?Sized>(&mut self, s: &S, delta: i32) -> bool {
		let at = self.vscroll_pos(s);
		self.set_vscroll_pos(s, at.saturating_add(delta))
	}

	/// The line under the window's `y` in a page area whose top is `area_top`, as its index: the line `y` is in, the nearer of the two
	/// lines around a gap (space before or after a paragraph, a page break), the first line shown above the text and the last below it.
	pub fn line_at_y<S: ReadStore + ?Sized>(&self, s: &S, area_top: i32, y: i32) -> Option<u32> {
		let g = PageGeom::of(self, s);
		let mut above: Option<(u32, i32)> = None;
		let mut below: Option<(u32, i32)> = None;
		let first = self.top;
		self.lines_from(s, first, |i, l| {
			if i < first {
				return true;
			}
			let top = g.line_y(l, area_top);
			if above.is_none() || top <= y {
				above = Some((i, top + l.ascent + l.descent));
			} else if below.is_none() {
				below = Some((i, top));
			}
			// Lines only go down from here: once one is below `y` nothing later can change the answer.
			below.is_none()
		});
		match (above, below) {
			(Some((_, bottom)), Some((b, top))) if y >= bottom && top - y < y - bottom => Some(b),
			(Some((a, _)), _) => Some(a),
			_ => None,
		}
	}

	/// A click in the text: the caret goes to the character nearest `x` (pixels from the left margin) on line `line`. With `extend`
	/// (Shift held) the selection runs from where the caret was; without it, a drag that follows selects from here.
	pub fn click<S: ReadStore + ?Sized>(&mut self, s: &S, line: u32, x: i32, extend: bool) {
		let Some(l) = self.line_at(s, line) else {
			return;
		};
		self.end_typing();
		let pos = self.pos_at_x(s, &l, x);
		if extend {
			if self.anchor.is_none() {
				self.anchor = Some(self.caret);
			}
		} else {
			self.anchor = Some(pos);
		}
		self.caret = pos;
		self.want_x = -1;
	}

	/// The pointer moved to `x` on line `line` with the button down: the selection runs from the click to here.
	pub fn drag_select_line<S: ReadStore + ?Sized>(&mut self, s: &S, line: u32, x: i32) {
		let Some(l) = self.line_at(s, line) else {
			return;
		};
		let pos = self.pos_at_x(s, &l, x);
		if self.anchor.is_none() {
			self.anchor = Some(self.caret);
		}
		self.caret = pos;
		self.want_x = -1;
	}
}

/// The levels the box offers, top to bottom: Page Width (0), then percentages from largest to smallest. Every one is a value
/// `App::set_zoom` keeps as it is.
pub const ZOOM_LEVELS: [u16; 7] = [0, 200, 150, 125, 100, 75, 50];

/// The longest text the box or one of its entries shows (`Page Width`, `200%`), so a shell can keep a small buffer.
pub const ZOOM_TEXT_MAX: usize = 10;

/// The text for a zoom: `Page Width` for 0, otherwise the percentage and its sign. Writes into `out` and returns the length.
pub fn zoom_text(zoom: u16, out: &mut [u8; ZOOM_TEXT_MAX]) -> usize {
	if zoom == 0 {
		let words = b"Page Width";
		out.copy_from_slice(words);
		return words.len();
	}
	let mut digits = [0u8; 5];
	let mut count = 0;
	let mut rest = zoom;
	while rest > 0 {
		if let Some(slot) = digits.get_mut(count) {
			*slot = b'0' + (rest % 10) as u8;
		}
		count += 1;
		rest /= 10;
	}
	let mut len = 0;
	for k in (0..count.min(digits.len())).rev() {
		if let (Some(d), Some(o)) = (digits.get(k), out.get_mut(len)) {
			*o = *d;
			len += 1;
		}
	}
	if let Some(o) = out.get_mut(len) {
		*o = b'%';
		len += 1;
	}
	len
}

/// The position of `zoom` in [`ZOOM_LEVELS`], or `None` for a zoom the list does not hold (a `QUILL.INI` value such as 90).
pub fn zoom_level_index(zoom: u16) -> Option<usize> {
	ZOOM_LEVELS.iter().position(|&l| l == zoom)
}

impl<M: Measure + Clone> App<M> {
	/// The box's text now: the view's zoom as `Page Width` or a percentage. Returns the length written to `out`.
	pub fn zoom_field_text(&self, out: &mut [u8; ZOOM_TEXT_MAX]) -> usize {
		zoom_text(self.zoom(), out)
	}

	/// Which entry of [`ZOOM_LEVELS`] is the view's zoom, for the tick in the list.
	pub fn zoom_field_current(&self) -> Option<usize> {
		zoom_level_index(self.zoom())
	}
}
