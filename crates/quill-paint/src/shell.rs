// The graphics shell's screen: a menu bar, the ruler, the page on its desk and a status line, at 640 by 480.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Plain Windows 3.1 chrome around [`paint_page`]. The layout is fixed: a 20-pixel menu bar, the 22-pixel ruler of `view.rs`, the page
//! area (the page is scrolled sideways so the 6.5-inch text column fits, because a Letter sheet is 816 pixels wide) and a 20-pixel
//! status line. Menus and dialogs are not drawn yet: while one is open the status line says so.

use quill_app::mouse::{PageHit, PointerTarget};
use quill_app::picture::WINDOW;
use quill_app::view::{
	hscroll_max, page_width, page_x0, ruler_marks, ruler_ticks, tab_x, text_extent, PageGeom,
	RulerState, PAGE_GAP, PAGE_PAD, PAGE_TOP, RULER_H,
};
use quill_app::{menu_title, status_text, App, MENU_COUNT};
use quill_core::layout::{to_units, FontRef, Measure};
use quill_core::store::Store;

use crate::page::{paint_page, PageInfo, PageView};
use crate::{Rect, Rgb, Surface};

/// The screen's width.
pub const SCREEN_W: i32 = 640;
/// The screen's height.
pub const SCREEN_H: i32 = 480;
/// The height of the menu bar.
pub const MENU_H: i32 = 20;
/// The height of the status line.
pub const STATUS_H: i32 = 20;
/// The top of the ruler.
pub const RULER_TOP: i32 = MENU_H;
/// The part of the screen the page takes.
pub const PAGE_AREA: Rect = Rect::new(0, MENU_H + RULER_H, SCREEN_W, SCREEN_H - STATUS_H);

const FACE: Rgb = Rgb::SILVER;
const SHADOW: Rgb = Rgb::GREY;
const LIGHT: Rgb = Rgb::WHITE;
const INK: Rgb = Rgb::BLACK;

/// How the app is to be told the page area's size, so scrolling keeps the caret in view.
pub fn view_height() -> i32 {
	PAGE_AREA.height()
}

/// How many lines of body text the page area shows at once, for Page Up and Page Down.
pub fn view_rows() -> usize {
	(PAGE_AREA.height() / 16).max(1) as usize
}

/// How far the page is scrolled sideways: until the text column starts at the page padding, or as far as the page can go.
pub fn hscroll<M: Measure + Clone>(app: &App<M>) -> i32 {
	let margin = to_units(app.doc().page.margin_l, 96);
	margin.min(hscroll_max(app, SCREEN_W)).max(0)
}

fn ui_font() -> FontRef {
	let mut face = [0u8; 31];
	let name = b"Arial";
	for (d, s) in face.iter_mut().zip(name) {
		*d = *s;
	}
	FontRef {
		face,
		face_len: name.len() as u8,
		family: 2,
		size: 20,
		flags: 0,
	}
}

/// The user-interface font as the surface draws it, with its baseline offset.
struct Ui<F: Copy> {
	font: F,
	ascent: i32,
	height: i32,
}

impl<F: Copy> Ui<F> {
	fn new<Sf: Surface<Font = F>, Me: Measure>(sf: &mut Sf, m: &mut Me) -> Self {
		let fr = ui_font();
		let met = m.metrics(&fr);
		Ui {
			font: sf.font(&fr),
			ascent: met.ascent,
			height: met.ascent + met.descent,
		}
	}

	/// Draws `text` with its top at `y`; returns the x after it.
	fn text<Sf: Surface<Font = F>>(
		&self,
		sf: &mut Sf,
		x: i32,
		y: i32,
		text: &[u8],
		fg: Rgb,
	) -> i32 {
		let mut x = x;
		for chunk in text.chunks(32) {
			let mut run = [(0u8, 0i32); 32];
			let mut n = 0;
			for (slot, &ch) in run.iter_mut().zip(chunk) {
				*slot = (ch, x);
				n += 1;
				x += sf.advance(self.font, ch);
			}
			if let Some(r) = run.get(..n) {
				sf.glyph_run(self.font, y + self.ascent, r, fg, false);
			}
		}
		x
	}

	fn width<Sf: Surface<Font = F>>(&self, sf: &mut Sf, text: &[u8]) -> i32 {
		text.iter().map(|c| sf.advance(self.font, *c)).sum()
	}
}

fn bevel<Sf: Surface>(sf: &mut Sf, r: Rect, raised: bool) {
	let (tl, br) = if raised {
		(LIGHT, SHADOW)
	} else {
		(SHADOW, LIGHT)
	};
	sf.hline(r.left, r.right, r.top, tl);
	sf.vline(r.left, r.top, r.bottom, tl);
	sf.hline(r.left, r.right, r.bottom - 1, br);
	sf.vline(r.right - 1, r.top, r.bottom, br);
}

fn menu_bar<Sf: Surface, M: Measure + Clone>(sf: &mut Sf, ui: &Ui<Sf::Font>, app: &App<M>) {
	let bar = Rect::new(0, 0, SCREEN_W, MENU_H);
	sf.clip(Some(bar));
	sf.fill_rect(bar, FACE);
	sf.hline(0, SCREEN_W, MENU_H - 1, INK);
	let y = (MENU_H - 1 - ui.height) / 2;
	let mut x = 10;
	for m in 0..MENU_COUNT {
		let title = menu_title(m);
		// An ampersand marks the letter that opens the menu with Alt: it is not drawn, and the letter after it is underlined.
		for (i, part) in title.split(|c| *c == b'&').enumerate() {
			let (mark, rest) = match part.split_first() {
				Some((c, rest)) if i > 0 => (Some(*c), rest),
				_ => (None, part),
			};
			if let Some(c) = mark {
				let w = sf.advance(ui.font, c);
				ui.text(sf, x, y, &[c], INK);
				sf.hline(x, x + w, y + ui.height, INK);
				x += w;
			}
			x = ui.text(sf, x, y, rest, INK);
		}
		x += 18;
	}
	let _ = app;
	sf.clip(None);
}

fn tab_mark<Sf: Surface>(sf: &mut Sf, kind: u8, x: i32, y: i32) {
	// A left tab is an L, centred a T, right a mirrored L, decimal a T with a dot.
	sf.vline(x, y - 5, y + 1, INK);
	match kind {
		1 => sf.hline(x - 2, x + 3, y, INK),
		2 => sf.hline(x - 4, x + 1, y, INK),
		_ => sf.hline(x, x + 5, y, INK),
	}
}

fn marker_down<Sf: Surface>(sf: &mut Sf, x: i32, top: i32) {
	// The first-line indent: a white triangle pointing down, outlined.
	for i in 0..5 {
		sf.hline(x - 4 + i, x + 5 - i, top + i, LIGHT);
	}
	sf.hline(x - 4, x + 5, top - 1, INK);
	sf.vline(x - 5, top - 1, top + 1, INK);
	sf.vline(x + 5, top - 1, top + 1, INK);
}

fn marker_up<Sf: Surface>(sf: &mut Sf, x: i32, top: i32) {
	for i in 0..5 {
		sf.hline(x - i, x + i + 1, top + i, LIGHT);
	}
	sf.hline(x - 4, x + 5, top + 5, LIGHT);
}

fn ruler<Sf: Surface, M: Measure + Clone, St: Store + ?Sized>(
	sf: &mut Sf,
	ui: &Ui<Sf::Font>,
	app: &App<M>,
	s: &St,
	hs: i32,
) {
	let top = RULER_TOP;
	let r = Rect::new(0, top, SCREEN_W, top + RULER_H);
	sf.clip(Some(r));
	sf.fill_rect(r, FACE);
	sf.hline(0, SCREEN_W, top + RULER_H - 1, SHADOW);
	let (text_left, text_w) = text_extent(app, SCREEN_W, hs);
	// The scale: white over the text width, grey in the margins.
	sf.fill_rect(
		Rect::new(text_left, top + 8, text_left + text_w, top + 15),
		LIGHT,
	);
	sf.vline(text_left - 1, top + 8, top + 15, INK);
	sf.vline(text_left + text_w, top + 8, top + 15, INK);
	for (dx, h, inch) in ruler_ticks(text_w) {
		let x = text_left + dx;
		if h > 0 {
			sf.vline(x, top + 8 + h, top + 15 - h, INK);
		} else if inch > 0 {
			let mut digits = [0u8; 4];
			let mut n = 0;
			let mut v = inch;
			while v > 0 && n < digits.len() {
				if let Some(d) = digits.get_mut(n) {
					*d = b'0' + (v % 10) as u8;
				}
				v /= 10;
				n += 1;
			}
			if let Some(d) = digits.get_mut(..n) {
				d.reverse();
				ui.text(sf, x - 2, top + 5, d, INK);
			}
		}
	}
	let st = RulerState::of(app, s);
	for (pos, kind) in st.stops() {
		tab_mark(sf, *kind, tab_x(*pos, text_left), top + 15);
	}
	let (fx, lx, rx) = ruler_marks(&st, text_left, text_w);
	marker_down(sf, fx, top + 1);
	marker_up(sf, lx, top + 14);
	marker_up(sf, rx, top + 14);
	sf.clip(None);
}

fn status_line<Sf: Surface, M: Measure + Clone, St: Store + ?Sized>(
	sf: &mut Sf,
	ui: &Ui<Sf::Font>,
	app: &App<M>,
	s: &St,
) {
	let r = Rect::new(0, SCREEN_H - STATUS_H, SCREEN_W, SCREEN_H);
	sf.clip(Some(r));
	sf.fill_rect(r, FACE);
	sf.hline(0, SCREEN_W, r.top, INK);
	let inset = Rect::new(4, r.top + 3, SCREEN_W - 4, r.bottom - 2);
	bevel(sf, inset, false);
	let mut buf = [0u8; 96];
	let n = if app.editing() {
		status_text(app, s, &mut buf)
	} else {
		let msg = b"A menu or dialog is open and is not drawn in graphics yet; Esc closes it";
		for (d, c) in buf.iter_mut().zip(msg) {
			*d = *c;
		}
		msg.len().min(buf.len())
	};
	let y = inset.top + (inset.height() - ui.height) / 2;
	if let Some(t) = buf.get(..n) {
		let max = SCREEN_W - 16;
		let mut t = t;
		while ui.width(sf, t) > max && !t.is_empty() {
			t = t.get(..t.len() - 1).unwrap_or(&[]);
		}
		ui.text(sf, inset.left + 4, y, t, INK);
	}
	sf.clip(None);
}

/// Paints the whole screen: the page and its desk first, then the menu bar, the ruler and the status line over their strips. `m` is
/// a measure for the interface font's metrics; `window` is the picture decoder's working memory.
pub fn paint_screen<Sf: Surface, M: Measure + Clone, St: Store + ?Sized, Me: Measure>(
	sf: &mut Sf,
	app: &App<M>,
	m: &mut Me,
	s: &St,
	window: &mut [u8; WINDOW],
) -> PageInfo {
	let hs = hscroll(app);
	let info = paint_page(
		sf,
		app,
		s,
		&PageView {
			area: PAGE_AREA,
			hscroll: hs,
		},
		window,
	);
	let ui = Ui::new(sf, m);
	menu_bar(sf, &ui, app);
	ruler(sf, &ui, app, s, hs);
	status_line(sf, &ui, app, s);
	info
}

/// What `[Quill] Graphics=` and the command line ask for.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Graphics {
	/// 80 by 25 text, the default.
	#[default]
	Text,
	/// The best 640 by 480 the card gives: VBE with 256 colours, else mode 12h.
	Best,
	/// Mode 12h only.
	Planar16,
}

impl Graphics {
	/// The value of `Graphics=` in the settings text: 0 text, 1 best, 16 mode 12h.
	pub fn from_ini(ini: &[u8]) -> Self {
		let mut g = Graphics::Text;
		for line in ini.split(|b| *b == b'\n') {
			let line = line.strip_suffix(b"\r").unwrap_or(line);
			if let Some(v) = line.strip_prefix(&b"Graphics="[..]) {
				g = match v.trim_ascii() {
					b"1" => Graphics::Best,
					b"16" => Graphics::Planar16,
					_ => Graphics::Text,
				};
			}
		}
		g
	}

	/// What one command-line argument asks for, if it is one of `/G`, `/GFX`, `/G16` and `/T` (a dash may stand for the slash).
	pub fn from_arg(arg: &str) -> Option<Self> {
		let a = arg.strip_prefix(['/', '-'])?;
		if a.eq_ignore_ascii_case("g") || a.eq_ignore_ascii_case("gfx") {
			Some(Graphics::Best)
		} else if a.eq_ignore_ascii_case("g16") {
			Some(Graphics::Planar16)
		} else if a.eq_ignore_ascii_case("t") {
			Some(Graphics::Text)
		} else {
			None
		}
	}
}

/// The page position under the screen pixel (`x`, `y`), as the page is painted: the page's left edge is `page_x0` from the area, the
/// line at the top of the area is `PageGeom::top`, and the pages are stacked a page and a gap apart. A point in the white space left
/// of the text is a margin hit.
pub fn pos_at_pixel<M: Measure + Clone, St: quill_core::store::ReadStore + ?Sized>(
	app: &App<M>,
	s: &St,
	x: i32,
	y: i32,
) -> PageHit {
	let area = PAGE_AREA;
	let g = PageGeom::of(app, s);
	let px = x - area.left - page_x0(area.width(), page_width(app), hscroll(app));
	let py = y - area.top - PAGE_TOP + g.top;
	let margin = to_units(app.doc().page.margin_l, 96);
	PageHit {
		pos: app.pos_at_point(s, px, py, g.page_h + PAGE_GAP),
		margin: px < margin - 4 && px >= -PAGE_PAD,
	}
}

/// The graphics screen as a pointer space: pixels of the 640 by 480 screen. Only the page takes the pointer for now. The menus and
/// dialogs are not drawn in graphics yet, so there are no hit tests for them here; when they are, this is where they go (the
/// `press_chrome`, `press_overlay` and `hover_overlay` methods of [`PointerTarget`]).
#[derive(Copy, Clone, Debug, Default)]
pub struct Pixels;

impl<M: Measure + Clone> PointerTarget<M> for Pixels {
	fn page_hit<St: quill_core::store::ReadStore + ?Sized>(
		&self,
		app: &App<M>,
		s: &St,
		x: i32,
		y: i32,
	) -> Option<PageHit> {
		let a = PAGE_AREA;
		(x >= a.left && x < a.right && y >= a.top && y < a.bottom)
			.then(|| pos_at_pixel(app, s, x, y))
	}

	fn slop(&self) -> i32 {
		4
	}

	fn edge(&self, y: i32) -> i32 {
		if y < PAGE_AREA.top {
			-1
		} else if y >= PAGE_AREA.bottom {
			1
		} else {
			0
		}
	}

	fn clamp_y(&self, y: i32) -> i32 {
		y.clamp(PAGE_AREA.top, PAGE_AREA.bottom - 1)
	}
}
