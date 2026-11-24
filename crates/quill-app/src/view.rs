// The window geometry the Windows builds share: where the toolbar's buttons, the ruler and the page are, what a click hits, and the
// mouse and scroll commands that act on the page.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Everything here is in pixels of a 96-dot-per-inch screen with the client area's top left at (0, 0). The platform draws (GDI on
// Windows 3.1 and on Windows 95 alike) and passes the mouse in; the numbers come from one place, so the two builds lay the window out
// the same way.
use quill_core::doc::cflag;
use quill_core::layout::{to_units, Line, Measure};
use quill_core::store::Store;

use crate::{App, Command};

/// Height of each of the two toolbar rows: a 24-pixel button and two pixels either side.
pub const TB_H: i32 = 28;
/// Height of the ruler.
pub const RULER_H: i32 = 22;
/// A toolbar button's size.
pub const BTN_W: i32 = 24;
pub const BTN_H: i32 = 24;
/// The gap between the window's edge and the page when the page is wider than the window.
pub const PAGE_PAD: i32 = 8;
/// The grey gap between pages, and above the first line shown.
pub const PAGE_GAP: i32 = 10;
pub const PAGE_TOP: i32 = 8;
/// Pixels per inch of the page view.
pub const DPI: u16 = 96;
/// A pixel at 96 dots per inch is 15 twips.
const TWIPS_PX: i32 = 15;

/// What a toolbar button does.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Act {
	Cmd(Command),
	/// Drops the 16-colour list.
	Colour,
}

/// What makes a button show pressed.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Toggle {
	No,
	Bold,
	Italic,
	Underline,
	Align(u8),
	Bullets,
	Numbering,
}

/// One toolbar button: its row (0 standard, 1 formatting), left edge, glyph in `assets/toolbar/toolbar.bmp`, action and toggle.
#[derive(Copy, Clone, Debug)]
pub struct Button {
	pub row: u8,
	pub x: i32,
	pub glyph: i32,
	pub act: Act,
	pub toggle: Toggle,
}

/// Index of the colour glyph, which shows the colour at the caret under the A.
pub const GLYPH_COLOUR: i32 = 13;
/// How many buttons the two rows hold.
pub const BUTTONS: usize = 21;

/// Where the toolbar's parts are.
pub struct Toolbar {
	pub buttons: [Button; BUTTONS],
	/// Left edge and width of the style, face and size boxes on the formatting row.
	pub style: (i32, i32),
	pub face: (i32, i32),
	pub size: (i32, i32),
}

const fn button(row: u8, x: i32, glyph: i32, act: Act, toggle: Toggle) -> Button {
	Button {
		row,
		x,
		glyph,
		act,
		toggle,
	}
}

const fn cmd(c: Command) -> Act {
	Act::Cmd(c)
}

/// Row 0: New, Open, Save, Print, Print Preview; Cut, Copy, Paste; Undo, Redo; Spelling. Row 1: the style, face and size boxes,
/// Bold, Italic, Underline, the colour; the four alignments; Bullets and Numbering. Groups are 8 pixels apart.
pub const TOOLBAR: Toolbar = {
	use Toggle::No;
	let g = 8;
	let w = BTN_W;
	let (a, b, c) = (4, 4 + 5 * w + g, 4 + 8 * w + 2 * g);
	let d = c + 2 * w + g;
	let style = (4, 104);
	let face = (style.0 + style.1 + 4, 140);
	let size = (face.0 + face.1 + 4, 48);
	let e = size.0 + size.1 + g;
	let f = e + 4 * w + g;
	let h = f + 4 * w + g;
	Toolbar {
		buttons: [
			button(0, a, 0, cmd(Command::New), No),
			button(0, a + w, 1, cmd(Command::Open), No),
			button(0, a + 2 * w, 2, cmd(Command::Save), No),
			button(0, a + 3 * w, 3, cmd(Command::Print), No),
			button(0, a + 4 * w, 4, cmd(Command::Preview), No),
			button(0, b, 5, cmd(Command::Cut), No),
			button(0, b + w, 6, cmd(Command::Copy), No),
			button(0, b + 2 * w, 7, cmd(Command::Paste), No),
			button(0, c, 8, cmd(Command::Undo), No),
			button(0, c + w, 9, cmd(Command::Redo), No),
			button(0, d, 20, cmd(Command::Spelling), No),
			button(1, e, 10, cmd(Command::Bold), Toggle::Bold),
			button(1, e + w, 11, cmd(Command::Italic), Toggle::Italic),
			button(1, e + 2 * w, 12, cmd(Command::Underline), Toggle::Underline),
			button(1, e + 3 * w, GLYPH_COLOUR, Act::Colour, No),
			button(1, f, 14, cmd(Command::Align(0)), Toggle::Align(0)),
			button(1, f + w, 15, cmd(Command::Align(1)), Toggle::Align(1)),
			button(1, f + 2 * w, 16, cmd(Command::Align(2)), Toggle::Align(2)),
			button(1, f + 3 * w, 17, cmd(Command::Align(3)), Toggle::Align(3)),
			button(1, h, 18, cmd(Command::Bullets), Toggle::Bullets),
			button(1, h + w, 19, cmd(Command::Numbering), Toggle::Numbering),
		],
		style,
		face,
		size,
	}
};

/// The sizes the size box offers, in points.
pub const SIZES: [u16; 15] = [8, 9, 10, 11, 12, 14, 16, 18, 20, 24, 28, 32, 36, 48, 72];

/// The sixteen colours of the colour list: name and red, green, blue.
pub const COLOURS: [(&str, (u8, u8, u8)); 16] = [
	("Black", (0, 0, 0)),
	("Maroon", (128, 0, 0)),
	("Green", (0, 128, 0)),
	("Olive", (128, 128, 0)),
	("Navy", (0, 0, 128)),
	("Purple", (128, 0, 128)),
	("Teal", (0, 128, 128)),
	("Silver", (192, 192, 192)),
	("Grey", (128, 128, 128)),
	("Red", (255, 0, 0)),
	("Lime", (0, 255, 0)),
	("Yellow", (255, 255, 0)),
	("Blue", (0, 0, 255)),
	("Fuchsia", (255, 0, 255)),
	("Aqua", (0, 255, 255)),
	("White", (255, 255, 255)),
];

/// A guess at a face's family from its name (1 roman, 2 swiss, 3 modern), for a document that records only the name.
pub fn family_of(name: &[u8]) -> u8 {
	let has = |w: &[u8]| {
		name.windows(w.len())
			.any(|x| x.iter().zip(w).all(|(a, b)| a.to_ascii_lowercase() == *b))
	};
	if has(b"courier") || has(b"mono") || has(b"console") || has(b"fixed") {
		3
	} else if has(b"arial")
		|| has(b"helv")
		|| has(b"sans")
		|| has(b"tahoma")
		|| has(b"verdana")
		|| has(b"trebuchet")
	{
		2
	} else {
		1
	}
}

/// What a click on the ruler is aimed at.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Hit {
	/// The first-line indent marker.
	First,
	/// The left indent marker alone.
	Left,
	/// The box under it, which moves the left and first-line indents together.
	Both,
	/// The right indent marker.
	Right,
	/// The tab stop at this position in twips.
	Tab(i32),
	/// The scale, at this position in twips from the left margin.
	Scale(i32),
	/// The button that cycles the kind of tab stop.
	Kind,
}

/// A paragraph's indents and tab stops as the ruler shows them, possibly changed by a drag in progress.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct RulerState {
	pub left: i32,
	pub first: i32,
	pub right: i32,
	pub tabs: [(i32, u8); 16],
	pub ntabs: usize,
}

impl RulerState {
	/// The state at the caret.
	pub fn of<M: Measure + Clone, S: Store + ?Sized>(app: &App<M>, s: &S) -> Self {
		let pf = app.para_state(s);
		let mut tabs = [(0, 0u8); 16];
		let ntabs = app.tab_stops(s, &mut tabs);
		RulerState {
			left: pf.left,
			first: pf.left + pf.first,
			right: pf.right,
			tabs,
			ntabs,
		}
	}

	/// The tab stops that are set.
	pub fn stops(&self) -> &[(i32, u8)] {
		self.tabs.get(..self.ntabs).unwrap_or(&[])
	}
}

/// The state of a drag on the ruler.
#[derive(Copy, Clone, Debug)]
pub struct Drag {
	pub hit: Hit,
	pub state: RulerState,
	pub start: RulerState,
}

/// The heights of the standard and formatting rows as they are showing.
pub fn rows<M: Measure + Clone>(app: &App<M>) -> (i32, i32) {
	(
		i32::from(app.show_toolbar()) * TB_H,
		i32::from(app.show_format_bar()) * TB_H,
	)
}

/// The height of the toolbars and the ruler together, which is where the page starts.
pub fn chrome_height<M: Measure + Clone>(app: &App<M>) -> i32 {
	let (a, b) = rows(app);
	a + b + i32::from(app.show_ruler()) * RULER_H
}

/// Which toolbar button is at (`x`, `y`), and the top of its row.
pub fn button_at<M: Measure + Clone>(app: &App<M>, x: i32, y: i32) -> Option<(Button, i32)> {
	let (h0, h1) = rows(app);
	let (row, top) = if y < h0 {
		(0u8, 0)
	} else if y < h0 + h1 {
		(1u8, h0)
	} else {
		return None;
	};
	if !(top + 2..top + 2 + BTN_H).contains(&y) {
		return None;
	}
	TOOLBAR
		.buttons
		.iter()
		.find(|b| b.row == row && x >= b.x && x < b.x + BTN_W)
		.map(|b| (*b, top))
}

/// Whether the toggle's property holds at the caret.
pub fn is_on<M: Measure + Clone, S: Store + ?Sized>(app: &App<M>, s: &S, t: Toggle) -> bool {
	match t {
		Toggle::No => false,
		Toggle::Bold => app.char_state(s).flags & cflag::BOLD != 0,
		Toggle::Italic => app.char_state(s).flags & cflag::ITALIC != 0,
		Toggle::Underline => app.char_state(s).flags & cflag::UNDERLINE != 0,
		Toggle::Align(n) => app.para_state(s).align == n,
		Toggle::Bullets => app.para_state(s).flags & 1 != 0,
		Toggle::Numbering => app.para_state(s).flags & 4 != 0,
	}
}

/// A number that changes whenever anything the toolbars and the ruler show changes: the rows, the toggles, the colour, the indents and
/// the tabs. A key that leaves it alone needs no repaint of them.
pub fn signature<M: Measure + Clone, S: Store + ?Sized>(app: &App<M>, s: &S) -> u32 {
	let mut h: u32 = 0x811c_9dc5;
	let mut mix = |v: u32| {
		for b in v.to_le_bytes() {
			h = (h ^ u32::from(b)).wrapping_mul(0x0100_0193);
		}
	};
	let (r0, r1) = rows(app);
	mix(r0 as u32);
	mix(r1 as u32);
	mix(u32::from(app.show_ruler()));
	let cs = app.char_state(s);
	let pf = app.para_state(s);
	mix(u32::from(cs.flags));
	mix(u32::from(pf.align));
	mix(u32::from(pf.flags));
	if let Some((r, g, b)) = cs.colour {
		mix(1 + u32::from(r) + (u32::from(g) << 8) + (u32::from(b) << 16));
	}
	let st = RulerState::of(app, s);
	mix(st.left as u32);
	mix(st.first as u32);
	mix(st.right as u32);
	for t in st.stops() {
		mix(t.0 as u32);
		mix(u32::from(t.1));
	}
	h
}

/// Where the page's left edge is in a client area `client_w` wide: centred when the page fits with its padding, and otherwise at the
/// padding, shifted left by `hscroll`.
pub fn page_x0(client_w: i32, page_w: i32, hscroll: i32) -> i32 {
	if client_w >= page_w + 2 * PAGE_PAD {
		(client_w - page_w) / 2
	} else {
		PAGE_PAD - hscroll
	}
}

/// The page's width in pixels.
pub fn page_width<M: Measure + Clone>(app: &App<M>) -> i32 {
	to_units(app.doc().page.paper_w, DPI)
}

/// How far the page can scroll sideways in a client area `client_w` wide: zero when it fits.
pub fn hscroll_max<M: Measure + Clone>(app: &App<M>, client_w: i32) -> i32 {
	(page_width(app) + 2 * PAGE_PAD - client_w).max(0)
}

/// Where the text's left edge is in the window, and the text width, with the page scrolled `hscroll` pixels.
pub fn text_extent<M: Measure + Clone>(app: &App<M>, client_w: i32, hscroll: i32) -> (i32, i32) {
	let page = &app.doc().page;
	let x0 = page_x0(client_w, page_width(app), hscroll);
	let left = x0 + to_units(page.margin_l, DPI);
	let w = to_units(page.paper_w - page.margin_l - page.margin_r, DPI);
	(left, w)
}

/// A position in twips, snapped to a sixteenth of an inch.
pub fn snap(twips: i32) -> i32 {
	(twips + 45).div_euclid(90) * 90
}

/// What is at (`x`, `y`) on the ruler whose top is at `top`.
pub fn ruler_hit(
	x: i32,
	y: i32,
	top: i32,
	text_left: i32,
	text_w: i32,
	st: &RulerState,
) -> Option<Hit> {
	let y = y - top;
	if !(0..RULER_H).contains(&y) {
		return None;
	}
	if x < 22 {
		return Some(Hit::Kind);
	}
	let near = |a: i32, b: i32| (a - b).abs() <= 4;
	let (fx, lx, rx) = (
		text_left + st.first / TWIPS_PX,
		text_left + st.left / TWIPS_PX,
		text_left + text_w - st.right / TWIPS_PX,
	);
	if y < 9 && near(x, fx) {
		return Some(Hit::First);
	}
	if y >= 19 && near(x, lx) {
		return Some(Hit::Both);
	}
	if y >= 12 && near(x, lx) {
		return Some(Hit::Left);
	}
	if y >= 12 && near(x, rx) {
		return Some(Hit::Right);
	}
	for (pos, _) in st.stops() {
		if near(x, text_left + pos / TWIPS_PX) && y >= 8 {
			return Some(Hit::Tab(*pos));
		}
	}
	if x >= text_left && x <= text_left + text_w && (8..15).contains(&y) {
		return Some(Hit::Scale((x - text_left) * TWIPS_PX));
	}
	None
}

/// Applies the pointer position `x` to a drag, as a preview.
pub fn drag_to(d: &mut Drag, x: i32, text_left: i32, text_w: i32) {
	let t = snap((x - text_left) * TWIPS_PX).clamp(0, text_w * TWIPS_PX);
	match d.hit {
		Hit::First => d.state.first = t,
		Hit::Left => d.state.left = t,
		Hit::Both => {
			let delta = t - d.start.left;
			d.state.left = t;
			d.state.first = (d.start.first + delta).max(0);
		}
		Hit::Right => {
			d.state.right =
				snap(text_w * TWIPS_PX - (x - text_left) * TWIPS_PX).clamp(0, text_w * TWIPS_PX)
		}
		_ => {}
	}
}

/// Where a pixel of the ruler's scale is: the x of each tick from the text's left edge, every eighth of an inch, with its height (0 for
/// a whole inch, which carries a number instead).
pub fn ruler_ticks(text_w: i32) -> impl Iterator<Item = (i32, i32, i32)> {
	(0..=(text_w * 8 / i32::from(DPI))).map(|i| {
		let h = if i % 8 == 0 {
			0
		} else if i % 4 == 0 {
			3
		} else {
			5
		};
		(i * 12, h, i / 8)
	})
}

/// Where the markers sit on the ruler: the first-line, left and right indents' x, for a text edge at `text_left`.
pub fn ruler_marks(st: &RulerState, text_left: i32, text_w: i32) -> (i32, i32, i32) {
	(
		text_left + st.first / TWIPS_PX,
		text_left + st.left / TWIPS_PX,
		text_left + text_w - st.right / TWIPS_PX,
	)
}

/// A tab stop's x on the ruler.
pub fn tab_x(pos: i32, text_left: i32) -> i32 {
	text_left + pos / TWIPS_PX
}

/// The page view's vertical geometry: pages one under another with a grey gap, the first line shown at `PAGE_TOP`.
#[derive(Copy, Clone, Debug)]
pub struct PageGeom {
	pub page_h: i32,
	pub margin_t: i32,
	/// The y of the first line shown, in the pages' own coordinates.
	pub top: i32,
}

impl PageGeom {
	/// The geometry for the document as it is scrolled now.
	pub fn of<M: Measure + Clone, S: Store + ?Sized>(app: &App<M>, s: &S) -> Self {
		let page = &app.doc().page;
		let mut g = PageGeom {
			page_h: to_units(page.paper_h, DPI),
			margin_t: to_units(page.margin_t, DPI),
			top: 0,
		};
		let first = app.top_line();
		let mut top = 0;
		app.for_each_line(s, |i, l| {
			if i == first {
				top = g.cum(l);
			}
		});
		g.top = top;
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

	/// The first line to show so that line `idx` (`l`), at or after the current first line, fits in the page area: the current one when
	/// it already does.
	pub(crate) fn top_to_show<S: Store + ?Sized>(&self, s: &S, idx: u32, l: &Line) -> u32 {
		let g = PageGeom::of(self, s);
		let room = self.view_h - PAGE_TOP;
		let need = g.cum(l) + (l.ascent + l.descent).max(1) - room;
		if need <= g.top {
			return self.top;
		}
		let mut top = idx;
		let from = self.top;
		self.lines(s, |i, line| {
			if i >= from && i < top && g.cum(line) >= need {
				top = i;
			}
		});
		top
	}

	/// How many lines the body has, laid out for the app's device.
	pub fn line_total<S: Store + ?Sized>(&self, s: &S) -> u32 {
		self.line_count(s)
	}

	/// Shows line `top` first (a scroll bar moved). The caret stays where it is until a key moves it.
	pub fn scroll_to<S: Store + ?Sized>(&mut self, s: &S, top: u32) {
		self.top = top.min(self.line_count(s).saturating_sub(1));
	}

	/// The line under the window's `y` in a page area whose top is `area_top`, as its index: the line `y` is in, the nearer of the two
	/// lines around a gap (space before or after a paragraph, a page break), the first line shown above the text and the last below it.
	pub fn line_at_y<S: Store + ?Sized>(&self, s: &S, area_top: i32, y: i32) -> Option<u32> {
		let g = PageGeom::of(self, s);
		let mut above: Option<(u32, i32)> = None;
		let mut below: Option<(u32, i32)> = None;
		let first = self.top;
		self.lines(s, |i, l| {
			if i < first {
				return;
			}
			let top = g.line_y(l, area_top);
			if above.is_none() || top <= y {
				above = Some((i, top + l.ascent + l.descent));
			} else if below.is_none() {
				below = Some((i, top));
			}
		});
		match (above, below) {
			(Some((_, bottom)), Some((b, top))) if y >= bottom && top - y < y - bottom => Some(b),
			(Some((a, _)), _) => Some(a),
			_ => None,
		}
	}

	/// A click in the text: the caret goes to the character nearest `x` (pixels from the left margin) on line `line`. With `extend`
	/// (Shift held) the selection runs from where the caret was; without it, a drag that follows selects from here.
	pub fn click<S: Store + ?Sized>(&mut self, s: &S, line: u32, x: i32, extend: bool) {
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
	pub fn drag_select<S: Store + ?Sized>(&mut self, s: &S, line: u32, x: i32) {
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
