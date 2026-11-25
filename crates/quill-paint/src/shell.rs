// The graphics shell's screen: a menu bar, the ruler, the page on its desk and a status line, at 640 by 480.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Windows 3.1 chrome around [`paint_page`]: a 20-pixel menu bar, the toolbar and the formatting bar (28 pixels each), the 22-pixel
//! ruler of `view.rs`, the page area, scroll bars 16 pixels wide, and a 20-pixel status line. Which of them show is the View menu's
//! and `QUILL.INI`'s choice ([`Layout`]); a Letter sheet is 816 pixels wide, so the page is scrolled sideways until the 6.5-inch text
//! column fills the area beside the vertical bar.

use quill_app::mouse::{PageHit, PageSpace, PointerTarget};
use quill_app::picture::WINDOW;
use quill_app::picture_geom::{HANDLE, SLOP};
use quill_app::view::{
	hscroll_max, page_width, page_x0, ruler_marks, ruler_ticks, tab_x, text_extent, PageGeom,
	RulerState, PAGE_GAP, PAGE_PAD, PAGE_TOP, RULER_H,
};
use quill_app::{status_text, App, Overlay};
use quill_core::layout::{to_units, Measure};
use quill_core::store::{ReadStore, Store};

use crate::layout::Layout;
use crate::page::{paint_page, PageInfo, PageView};
use crate::pointer::{View, KIND_W};
use crate::scroll;
use crate::ui::{bevel, Ui, FACE, INK, LIGHT, SHADOW};
use crate::{dialog, drop, menu, tip, toolbar};
use crate::{Rect, Surface};

/// The screen's width.
pub const SCREEN_W: i32 = 640;
/// The screen's height.
pub const SCREEN_H: i32 = 480;
/// The height of the menu bar.
pub const MENU_H: i32 = 20;
/// The height of the status line.
pub const STATUS_H: i32 = 20;

/// Tells the app how big the page area is, so scrolling keeps the caret in view and Page Up and Page Down move a screenful, and
/// brings the sideways scroll back inside what the content needs. The page area changes with the View menu (a hidden toolbar gives
/// its rows to the page) and the content with every edit, so the shell calls this after every key.
pub fn sync_view<M: Measure + Clone, S: ReadStore + ?Sized>(app: &mut App<M>, s: &S) {
	let page = Layout::of(app).page;
	app.set_view_height(page.height());
	app.set_rows((page.height() / 16).max(1) as usize);
	hclamp(app, s);
}

/// The text column's left edge is at the left edge of a page area `client_w` wide: where the page is scrolled sideways when nothing is
/// wider than the area.
fn hbase<M: Measure + Clone>(app: &App<M>, client_w: i32) -> i32 {
	let margin = to_units(app.doc().page.margin_l, 96);
	(margin + PAGE_PAD).min(hscroll_max(app, client_w)).max(0)
}

/// How far the page is scrolled sideways in a page area `client_w` wide: the text column at the left edge, plus the app's sideways
/// position (the scroll bar's, the wheel's and the caret's).
pub fn hscroll<M: Measure + Clone>(app: &App<M>, client_w: i32) -> i32 {
	hbase(app, client_w) + app.hscroll_pos().max(0)
}

/// The sideways scroll's range in pixels, from what the lines shown reach: the text column, or a picture or a tab stop that goes past
/// it, against the width of the area. 0 when nothing is wider, and when the whole sheet fits (the page is then centred).
pub fn hrange<M: Measure + Clone, S: ReadStore + ?Sized>(
	app: &App<M>,
	s: &S,
	client_w: i32,
) -> i32 {
	if client_w >= page_width(app) + 2 * PAGE_PAD {
		return 0;
	}
	let ml = to_units(app.doc().page.margin_l, 96);
	let col = to_units(app.doc().page.text_w(), 96);
	let lines = (app.page_rows() as u32).max(1) * 3;
	let content = col.max(app.content_right(s, lines));
	(ml + PAGE_PAD - hbase(app, client_w) + content - client_w).max(0)
}

/// Brings the app's sideways position inside 0 to [`hrange`] for the page area as it is now.
pub fn hclamp<M: Measure + Clone, S: ReadStore + ?Sized>(app: &mut App<M>, s: &S) {
	let w = Layout::of(app).page.width();
	let to = app.hscroll_pos().clamp(0, hrange(app, s, w));
	app.set_hscroll_pos(to);
}

/// Scrolls sideways by `dx` pixels (negative is left), as far as the content allows. True when the view moved.
pub fn hscroll_by<M: Measure + Clone, S: ReadStore + ?Sized>(
	app: &mut App<M>,
	s: &S,
	dx: i32,
) -> bool {
	let w = Layout::of(app).page.width();
	let before = app.hscroll_pos();
	app.set_hscroll_pos(before.saturating_add(dx).clamp(0, hrange(app, s, w)));
	app.hscroll_pos() != before
}

/// Scrolls sideways until the caret is in view, when the caret or the text has changed since the last time: a key, or a press in the
/// page. A scroll bar moves the view away from the caret and nothing here brings it back.
pub fn follow_caret<M: Measure + Clone, S: ReadStore + ?Sized>(app: &mut App<M>, s: &S) {
	if !app.caret_moved_since_follow() {
		return;
	}
	app.mark_followed();
	if !app.editing() {
		return;
	}
	let w = Layout::of(app).page.width();
	let Some((_, _, x)) = app.caret_xy(s) else {
		return;
	};
	let margin = to_units(app.doc().page.margin_l, 96);
	// The caret's column in the area, as the page is scrolled now.
	let at = PAGE_PAD + margin + x - hscroll(app, w);
	let slack = 32.min(w / 4);
	let pos = app.hscroll_pos();
	let to = if at < slack {
		pos - (slack - at)
	} else if at > w - slack {
		pos + (at - (w - slack))
	} else {
		pos
	};
	app.set_hscroll_pos(to.clamp(0, hrange(app, s, w)));
}

fn tab_mark<Sf: Surface + ?Sized>(sf: &mut Sf, kind: u8, x: i32, y: i32) {
	// A left tab is an L, centred a T, right a mirrored L, decimal a T with a dot.
	sf.vline(x, y - 5, y + 1, INK);
	match kind {
		1 => sf.hline(x - 2, x + 3, y, INK),
		2 => sf.hline(x - 4, x + 1, y, INK),
		3 => {
			sf.hline(x - 2, x + 3, y, INK);
			sf.hline(x + 2, x + 3, y - 3, INK);
		}
		_ => sf.hline(x, x + 5, y, INK),
	}
}

fn marker_down<Sf: Surface + ?Sized>(sf: &mut Sf, x: i32, top: i32) {
	// The first-line indent: a white triangle pointing down, outlined.
	for i in 0..5 {
		sf.hline(x - 4 + i, x + 5 - i, top + i, LIGHT);
	}
	sf.hline(x - 4, x + 5, top - 1, INK);
	sf.vline(x - 5, top - 1, top + 1, INK);
	sf.vline(x + 5, top - 1, top + 1, INK);
}

fn marker_up<Sf: Surface + ?Sized>(sf: &mut Sf, x: i32, top: i32) {
	for i in 0..5 {
		sf.hline(x - i, x + i + 1, top + i, LIGHT);
	}
	sf.hline(x - 4, x + 5, top + 5, LIGHT);
}

fn ruler<Sf: Surface + ?Sized, M: Measure + Clone, St: Store + ?Sized>(
	sf: &mut Sf,
	ui: &Ui<Sf::Font>,
	app: &App<M>,
	s: &St,
	r: Rect,
	(client_w, hs): (i32, i32),
	(state, kind): (Option<RulerState>, u8),
) {
	let top = r.top;
	sf.clip(Some(r));
	sf.fill_rect(r, FACE);
	sf.hline(0, SCREEN_W, top + RULER_H - 1, SHADOW);
	let (text_left, text_w) = text_extent(app, client_w, hs);
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
	let st = state.unwrap_or_else(|| RulerState::of(app, s));
	// The button at the left end that sets the kind of tab a click on the scale makes, showing the kind. The text column starts at
	// the left edge, so the markers and the tab stops are drawn over the button (and take a press before it does).
	let b = Rect::new(0, top, KIND_W, top + RULER_H - 1);
	sf.fill_rect(b, FACE);
	bevel(sf, Rect::new(2, top + 2, 20, top + 20), true);
	tab_mark(sf, kind, 8 + i32::from(kind == 2) * 3, top + 14);
	for (pos, k) in st.stops() {
		tab_mark(sf, *k, tab_x(*pos, text_left), top + 15);
	}
	let (fx, lx, rx) = ruler_marks(&st, text_left, text_w);
	marker_down(sf, fx, top + 1);
	marker_up(sf, lx, top + 14);
	marker_up(sf, rx, top + 14);
	sf.clip(None);
}

fn status_line<Sf: Surface + ?Sized, M: Measure + Clone, St: Store + ?Sized>(
	sf: &mut Sf,
	ui: &Ui<Sf::Font>,
	app: &App<M>,
	s: &St,
	r: Rect,
) {
	sf.clip(Some(r));
	sf.fill_rect(r, FACE);
	sf.hline(0, SCREEN_W, r.top, INK);
	let inset = Rect::new(4, r.top + 3, SCREEN_W - 4, r.bottom - 2);
	bevel(sf, inset, false);
	let mut buf = [0u8; 96];
	let n = status_text(app, s, &mut buf);
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

/// Draws the two scroll bars and the box where they meet.
fn scroll_bars<Sf: Surface + ?Sized, M: Measure + Clone, St: Store + ?Sized>(
	sf: &mut Sf,
	app: &App<M>,
	s: &St,
	lay: &Layout,
) {
	let total = app.line_total(s) as i32;
	let view = (app.lines_in_view(s) as i32).min(total);
	let v = scroll::parts(lay.vbar, true, app.top_line() as i32, total, view);
	scroll::draw(sf, &v, total > view);
	if let Some(hbar) = lay.hbar {
		// The thumb is the width of the area over the width of what it can show, and sits at the sideways scroll.
		let w = lay.page.width();
		let range = hrange(app, s, w);
		let h = scroll::parts(hbar, false, app.hscroll_pos(), range + w, w);
		scroll::draw(sf, &h, range > 0);
	}
	if let Some(c) = lay.corner {
		sf.fill_rect(c, FACE);
	}
}

/// Paints the whole screen: the page and its desk first, then the bars, the ruler, the scroll bars and the status line over their
/// strips, then an open menu or dialog. `m` is a measure for the interface font's metrics; `window` is the picture decoder's working
/// memory.
pub fn paint_screen<Sf: Surface, M: Measure + Clone, St: Store + ?Sized, Me: Measure>(
	sf: &mut Sf,
	app: &App<M>,
	m: &mut Me,
	s: &St,
	window: &mut [u8; WINDOW],
) -> PageInfo {
	paint_screen_view(sf, app, m, s, window, &View::default())
}

/// [`paint_screen`] with a gesture in progress shown: the button the pointer holds, an open drop-down list, a tip, the ruler as a
/// drag has it.
pub fn paint_screen_view<Sf: Surface, M: Measure + Clone, St: Store + ?Sized, Me: Measure>(
	sf: &mut Sf,
	app: &App<M>,
	m: &mut Me,
	s: &St,
	window: &mut [u8; WINDOW],
	view: &View,
) -> PageInfo {
	let lay = Layout::of(app);
	let hs = hscroll(app, lay.page.width());
	let info = paint_page(
		sf,
		app,
		s,
		&PageView {
			area: lay.page,
			hscroll: hs,
		},
		window,
	);
	// The chrome is drawn through a trait object: the two surfaces share a font type, so one copy of it serves both modes, and a
	// call through the table costs nothing next to the pixels it fills.
	chrome(sf, app, m, s, &lay, hs, view);
	info
}

#[inline(never)]
fn chrome<Sf: Surface, M: Measure + Clone, St: Store + ?Sized, Me: Measure>(
	sf: &mut Sf,
	app: &App<M>,
	m: &mut Me,
	s: &St,
	lay: &Layout,
	hs: i32,
	view: &View,
) {
	let ui = Ui::new(sf, m);
	toolbar::draw(sf, &ui, app, s, lay, view.button);
	if let Some(r) = lay.ruler {
		ruler(
			sf,
			&ui,
			app,
			s,
			r,
			(lay.page.width(), hs),
			(view.ruler, view.kind),
		);
	}
	rest(sf, app, s, lay, view, &ui);
}

/// The bars that are not the toolbar, and what is drawn over them: through a trait object, so the two surfaces share one copy.
#[inline(never)]
fn rest<F: Copy, M: Measure + Clone, St: Store + ?Sized>(
	sf: &mut dyn Surface<Font = F>,
	app: &App<M>,
	s: &St,
	lay: &Layout,
	view: &View,
	ui: &Ui<F>,
) {
	let open = app.menu_open();
	menu::draw_bar(sf, ui, open.map(|(m, _)| m));
	scroll_bars(sf, app, s, lay);
	if let Some(r) = lay.status {
		status_line(sf, ui, app, s, r);
	}
	over(sf, app, s, lay, view, ui);
}

/// What is drawn over the bars: an open menu, a list, a tip, a dialog. Only one of them is there on most frames, so it is drawn through
/// a trait object and the two surfaces share one copy.
fn over<F: Copy, M: Measure + Clone, St: Store + ?Sized>(
	sf: &mut dyn Surface<Font = F>,
	app: &App<M>,
	s: &St,
	lay: &Layout,
	view: &View,
	ui: &Ui<F>,
) {
	if let Some((menu, item)) = app.menu_open() {
		menu::draw_menu(sf, ui, app, s, menu, item);
	}
	if let Some((pop, hot)) = view.drop {
		drop::draw(sf, ui, lay, pop, hot);
	}
	if let (Some(t), true) = (view.tip, app.editing()) {
		tip::draw(sf, ui, lay, t);
	}
	let mut ov = Overlay::default();
	if app.overlay(s, &mut ov) {
		dialog::draw_dialog_held(sf, ui, &ov, view.widget);
	}
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
	let sp = page_space_at(app, s, x, y);
	let (px, py) = (sp.x, sp.y);
	let g = PageGeom::of(app, s);
	let margin = to_units(app.doc().page.margin_l, 96);
	PageHit {
		pos: app.pos_at_point(s, px, py, g.page_h + PAGE_GAP),
		margin: px < margin - 4 && px >= -PAGE_PAD,
	}
}

/// The screen pixel (`x`, `y`) in the page's own units, with the pages' spacing and the size of a picture's handles at 96 to the inch.
pub fn page_space_at<M: Measure + Clone, St: quill_core::store::ReadStore + ?Sized>(
	app: &App<M>,
	s: &St,
	x: i32,
	y: i32,
) -> PageSpace {
	let area = Layout::of(app).page;
	let g = PageGeom::of(app, s);
	PageSpace {
		x: x - area.left - page_x0(area.width(), page_width(app), hscroll(app, area.width())),
		y: y - area.top - PAGE_TOP + g.top,
		stride: g.page_h + PAGE_GAP,
		handle: HANDLE,
		slop: SLOP,
	}
}

/// The graphics screen's page as a pointer space: pixels of the 640 by 480 screen. Its rectangle is where the View menu's bars leave
/// it (`Layout::of`), so a `Pixels` is made from the app with [`Pixels::of`] before each event. Only the page is here: the menus,
/// dialogs, bars, lists, ruler and scroll bars are worked by [`crate::pointer::Gfx`], which needs the release and the held state that
/// the `press_chrome`, `press_overlay` and `hover_overlay` hooks of [`PointerTarget`] cannot carry, and hands the page's events to the
/// app's `Pointer` with this as the target.
#[derive(Copy, Clone, Debug)]
pub struct Pixels {
	page: Rect,
}

impl Pixels {
	/// The pointer space for the screen as the app's chrome flags lay it out now.
	pub fn of<M: Measure + Clone>(app: &App<M>) -> Pixels {
		Pixels {
			page: Layout::of(app).page,
		}
	}
}

impl<M: Measure + Clone> PointerTarget<M> for Pixels {
	fn page_hit<St: quill_core::store::ReadStore + ?Sized>(
		&self,
		app: &App<M>,
		s: &St,
		x: i32,
		y: i32,
	) -> Option<PageHit> {
		let a = self.page;
		(x >= a.left && x < a.right && y >= a.top && y < a.bottom)
			.then(|| pos_at_pixel(app, s, x, y))
	}

	fn page_space<St: quill_core::store::ReadStore + ?Sized>(
		&self,
		app: &App<M>,
		s: &St,
		x: i32,
		y: i32,
	) -> Option<PageSpace> {
		Some(page_space_at(app, s, x, y))
	}

	fn slop(&self) -> i32 {
		4
	}

	fn edge(&self, y: i32) -> i32 {
		if y < self.page.top {
			-1
		} else if y >= self.page.bottom {
			1
		} else {
			0
		}
	}

	fn clamp_y(&self, y: i32) -> i32 {
		y.clamp(self.page.top, self.page.bottom - 1)
	}
}
