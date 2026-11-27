// The window geometry the graphical builds share: where the toolbar's buttons, the ruler and the page are, and what a click hits.
//
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Everything here is in pixels of a 96-dot-per-inch screen with the client area's top left at (0, 0). The platform draws (GDI on
// Windows 3.1 and on Windows 95 alike) and passes the mouse in; the numbers come from one place, so the two builds lay the window out
// the same way.
use quill_core::doc::cflag;
use quill_core::layout::{to_units, Measure};
use quill_core::store::ReadStore;

pub use quill_app::scroll::{PageGeom, DPI, PAGE_GAP, PAGE_TOP, SCROLL_LINE, SHEET_SHADOW};
use quill_app::{App, Command};

/// Height of each of the two toolbar rows: a 24-pixel button and two pixels either side.
pub const TB_H: i32 = 28;
/// Height of the ruler.
pub const RULER_H: i32 = 22;
/// A toolbar button's size.
pub const BTN_W: i32 = 24;
pub const BTN_H: i32 = 24;
/// The gap between the window's edge and the page when the page is wider than the window.
pub const PAGE_PAD: i32 = 8;
/// Twips to a pixel at 96 dots per inch.
pub const TWIPS_PX: i32 = 15;

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
	pub fn of<M: Measure + Clone, S: ReadStore + ?Sized>(app: &App<M>, s: &S) -> Self {
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
pub fn is_on<M: Measure + Clone, S: ReadStore + ?Sized>(app: &App<M>, s: &S, t: Toggle) -> bool {
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
pub fn signature<M: Measure + Clone, S: ReadStore + ?Sized>(app: &App<M>, s: &S) -> u32 {
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
	mix(u32::from(app.zoom()));
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

/// A rectangle as left, top, width and height.
pub type Box4 = (i32, i32, i32, i32);

/// Where the page's scroll bars go in a client area `w` by `h`: the vertical bar takes the right edge from `top` (the bottom of the
/// toolbars and the ruler) down to the status bar, the horizontal bar the bottom of the page area, and where they meet is the
/// corner box. A bar never reaches up beside the toolbars or down beside the status bar.
pub struct ScrollLayout {
	/// The vertical bar.
	pub vbar: Box4,
	/// The horizontal bar, when the page is wider than the window.
	pub hbar: Option<Box4>,
	/// The box where the two bars meet, when both show.
	pub corner: Option<Box4>,
	/// The width the page has to lay out in: the window less the vertical bar.
	pub page_w: i32,
	/// The bottom of the page area.
	pub page_bottom: i32,
}

/// The scroll bar layout for a client area `w` by `h` with the page area starting at `top`, a status bar `status` high (0 when it is
/// hidden), bars `bar_w` wide and `bar_h` high, and the horizontal bar shown or not.
pub fn scroll_layout(
	w: i32,
	h: i32,
	top: i32,
	status: i32,
	bar_w: i32,
	bar_h: i32,
	show_h: bool,
) -> ScrollLayout {
	let bottom = (h - status).max(top);
	let hh = if show_h { bar_h } else { 0 };
	let page_bottom = (bottom - hh).max(top);
	let page_w = (w - bar_w).max(0);
	ScrollLayout {
		vbar: (page_w, top, bar_w, page_bottom - top),
		hbar: show_h.then_some((0, page_bottom, page_w, hh)),
		corner: show_h.then_some((page_w, page_bottom, bar_w, hh)),
		page_w,
		page_bottom,
	}
}

/// How far the page can scroll sideways in a client area `client_w` wide: zero when it fits.
pub fn hscroll_max<M: Measure + Clone>(app: &App<M>, client_w: i32) -> i32 {
	(page_width(app) + 2 * PAGE_PAD - client_w).max(0)
}

/// The smallest and largest zoom a view draws, in percent.
pub const ZOOM_MIN: i32 = 25;
pub const ZOOM_MAX: i32 = 200;

/// The zoom in percent a window `client_w` pixels wide shows now: the one chosen, or for Page Width (0) the largest that fits the paper
/// with [`PAGE_PAD`] either side, never past 100.
pub fn zoom_pct<M: Measure + Clone>(app: &App<M>, client_w: i32) -> i32 {
	match app.zoom() {
		0 => ((client_w - 2 * PAGE_PAD) * 100 / page_width(app).max(1)).clamp(ZOOM_MIN, 100),
		z => i32::from(z).clamp(ZOOM_MIN, ZOOM_MAX),
	}
}

/// A length in page units (pixels at 96 dots per inch) as device pixels at `pct` percent, rounded down. A shell that draws at a zoom
/// takes every position through this one function, so two edges that meet in the layout meet on the screen too.
pub fn dev(v: i32, pct: i32) -> i32 {
	v.saturating_mul(pct).div_euclid(100)
}

/// A length in device pixels at `pct` percent as page units, rounded down: what a click and the height of a window are in the layout.
pub fn log(v: i32, pct: i32) -> i32 {
	v.saturating_mul(100).div_euclid(pct.max(1))
}

/// [`hscroll_max`] at `pct` percent: how far the page can scroll sideways in device pixels.
pub fn hscroll_max_at<M: Measure + Clone>(app: &App<M>, client_w: i32, pct: i32) -> i32 {
	(dev(page_width(app), pct) + 2 * PAGE_PAD - client_w).max(0)
}

/// [`text_extent`] at `pct` percent, in device pixels: where the text's left edge is and the width of the column.
pub fn text_extent_at<M: Measure + Clone>(
	app: &App<M>,
	client_w: i32,
	hscroll: i32,
	pct: i32,
) -> (i32, i32) {
	let page = &app.doc().page;
	let x0 = page_x0(client_w, dev(page_width(app), pct), hscroll);
	let left = x0 + dev(to_units(page.margin_l, DPI), pct);
	(left, dev(to_units(page.text_w(), DPI), pct))
}

/// Where the text's left edge is in the window, and the text width, with the page scrolled `hscroll` pixels.
pub fn text_extent<M: Measure + Clone>(app: &App<M>, client_w: i32, hscroll: i32) -> (i32, i32) {
	let page = &app.doc().page;
	let x0 = page_x0(client_w, page_width(app), hscroll);
	let left = x0 + to_units(page.margin_l, DPI);
	let w = to_units(page.text_w(), DPI);
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
	ruler_hit_with(x, y, top, text_left, text_w, st, 22)
}

/// [`ruler_hit`] for a ruler whose tab-kind button is `kind_w` pixels wide at the left end (0 when the shell draws none, so the
/// markers and the scale are reachable from the first pixel).
pub fn ruler_hit_with(
	x: i32,
	y: i32,
	top: i32,
	text_left: i32,
	text_w: i32,
	st: &RulerState,
	kind_w: i32,
) -> Option<Hit> {
	ruler_hit_at(x, y, top, (text_left, text_w), st, kind_w, 100)
}

/// [`ruler_hit_with`] for a ruler drawn at `pct` percent: `column` is the text's left edge and width in device pixels, and a position on
/// the scale comes back in twips, as it would at 100.
pub fn ruler_hit_at(
	x: i32,
	y: i32,
	top: i32,
	column: (i32, i32),
	st: &RulerState,
	kind_w: i32,
	pct: i32,
) -> Option<Hit> {
	let (text_left, text_w) = column;
	let y = y - top;
	if !(0..RULER_H).contains(&y) {
		return None;
	}
	if x < kind_w {
		return Some(Hit::Kind);
	}
	let near = |a: i32, b: i32| (a - b).abs() <= 4;
	let (fx, lx, rx) = ruler_marks_at(st, text_left, text_w, pct);
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
		if near(x, tab_x_at(*pos, text_left, pct)) && y >= 8 {
			return Some(Hit::Tab(*pos));
		}
	}
	if x >= text_left && x <= text_left + text_w && (8..15).contains(&y) {
		return Some(Hit::Scale(log(x - text_left, pct) * TWIPS_PX));
	}
	None
}

/// Applies the pointer position `x` to a drag, as a preview.
pub fn drag_to(d: &mut Drag, x: i32, text_left: i32, text_w: i32) {
	drag_to_at(d, x, text_left, text_w, 100);
}

/// [`drag_to`] on a ruler drawn at `pct` percent.
pub fn drag_to_at(d: &mut Drag, x: i32, text_left: i32, text_w: i32, pct: i32) {
	let wide = log(text_w, pct) * TWIPS_PX;
	let at = log(x - text_left, pct) * TWIPS_PX;
	let t = snap(at).clamp(0, wide);
	match d.hit {
		Hit::First => d.state.first = t,
		Hit::Left => d.state.left = t,
		Hit::Both => {
			let delta = t - d.start.left;
			d.state.left = t;
			d.state.first = (d.start.first + delta).max(0);
		}
		Hit::Right => d.state.right = snap(wide - at).clamp(0, wide),
		_ => {}
	}
}

/// Where a pixel of the ruler's scale is: the x of each tick from the text's left edge, every eighth of an inch, with its height (0 for
/// a whole inch, which carries a number instead).
pub fn ruler_ticks(text_w: i32) -> impl Iterator<Item = (i32, i32, i32)> {
	ruler_ticks_at(text_w, 100)
}

/// [`ruler_ticks`] for a ruler drawn at `pct` percent, whose scale is `text_w` device pixels wide.
pub fn ruler_ticks_at(text_w: i32, pct: i32) -> impl Iterator<Item = (i32, i32, i32)> {
	(0..=(log(text_w, pct) * 8 / i32::from(DPI))).map(move |i| {
		let h = if i % 8 == 0 {
			0
		} else if i % 4 == 0 {
			3
		} else {
			5
		};
		(dev(i * 12, pct), h, i / 8)
	})
}

/// Where the markers sit on the ruler: the first-line, left and right indents' x, for a text edge at `text_left`.
pub fn ruler_marks(st: &RulerState, text_left: i32, text_w: i32) -> (i32, i32, i32) {
	ruler_marks_at(st, text_left, text_w, 100)
}

/// [`ruler_marks`] for a ruler drawn at `pct` percent.
pub fn ruler_marks_at(st: &RulerState, text_left: i32, text_w: i32, pct: i32) -> (i32, i32, i32) {
	(
		text_left + dev(st.first / TWIPS_PX, pct),
		text_left + dev(st.left / TWIPS_PX, pct),
		text_left + text_w - dev(st.right / TWIPS_PX, pct),
	)
}

/// A tab stop's x on the ruler.
pub fn tab_x(pos: i32, text_left: i32) -> i32 {
	tab_x_at(pos, text_left, 100)
}

/// [`tab_x`] for a ruler drawn at `pct` percent.
pub fn tab_x_at(pos: i32, text_left: i32, pct: i32) -> i32 {
	text_left + dev(pos / TWIPS_PX, pct)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn dev_and_log_round_down_and_agree_at_whole_percents() {
		assert_eq!(dev(816, 200), 1632);
		assert_eq!(dev(816, 50), 408);
		assert_eq!(dev(1056, 150), 1584);
		assert_eq!(dev(-1, 50), -1);
		assert_eq!(log(1632, 200), 816);
		assert_eq!(log(-1, 200), -1);
		for pct in [25, 50, 75, 100, 150, 200] {
			for v in [0, 1, 7, 96, 815, 2112] {
				// A length taken to the screen and back never grows, and loses less than a pixel's worth of page units.
				let back = log(dev(v, pct), pct);
				assert!(back <= v && v - back <= 100 / pct + 1, "{v} at {pct}");
			}
		}
	}

	#[test]
	fn a_sheet_at_the_top_zooms_fit_in_sixteen_bits() {
		// 8.5 by 11 inches at 96 dots per inch, and the largest zoom: what Windows 3.x's 16-bit coordinates have to hold.
		assert!(dev(816, ZOOM_MAX) <= i32::from(i16::MAX));
		assert!(dev(1056, ZOOM_MAX) <= i32::from(i16::MAX));
	}

	#[test]
	fn the_ruler_at_100_is_the_ruler_at_96_dots_to_the_inch() {
		let st = RulerState {
			left: 720,
			first: 360,
			right: 180,
			tabs: [(0, 0); 16],
			ntabs: 0,
		};
		assert_eq!(ruler_marks(&st, 100, 576), (124, 148, 664));
		assert_eq!(ruler_marks_at(&st, 100, 576, 100), (124, 148, 664));
		assert_eq!(tab_x(2880, 100), 292);
		assert_eq!(ruler_ticks(96).count(), 9);
		assert!(ruler_ticks(96).eq(ruler_ticks_at(96, 100)));
	}

	#[test]
	fn the_ruler_at_a_zoom_puts_its_marks_where_the_text_is() {
		let st = RulerState {
			left: 720,
			first: 360,
			right: 180,
			tabs: [(0, 0); 16],
			ntabs: 0,
		};
		// A column of 6 inches at 50 %: 288 pixels, an inch (a whole tick, labelled) every 48 and the indents half as far from the edges.
		assert_eq!(ruler_marks_at(&st, 100, 288, 50), (112, 124, 100 + 288 - 6));
		assert_eq!(ruler_ticks_at(288, 50).count(), 49);
		assert_eq!(ruler_ticks_at(288, 50).nth(8), Some((48, 0, 1)));
		assert_eq!(ruler_ticks_at(288, 50).nth(4), Some((24, 3, 0)));
		// A click on the scale at 200 % is the twips the text has at 100 %, and a drag takes the indent there.
		let hit = ruler_hit_at(100 + 192, 10, 0, (100, 1152), &st, 0, 200);
		assert_eq!(hit, Some(Hit::Scale(1440)));
		let mut d = Drag {
			hit: Hit::Left,
			state: st,
			start: st,
		};
		drag_to_at(&mut d, 100 + 192, 100, 1152, 200);
		assert_eq!(d.state.left, 1440);
		drag_to_at(&mut d, 100 + 100_000, 100, 1152, 200);
		assert_eq!(d.state.left, 576 * TWIPS_PX);
	}
}
