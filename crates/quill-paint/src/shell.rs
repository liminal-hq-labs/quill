// The graphics shell's screen: a menu bar, the ruler, the page on its desk and a status line, at 640 by 480.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Windows 3.1 chrome around [`paint_page`]: a 20-pixel menu bar, the toolbar and the formatting bar (28 pixels each), the 22-pixel
//! ruler of `view.rs`, the page area, scroll bars 16 pixels wide, and a 20-pixel status line. Which of them show is the View menu's
//! and `QUILL.INI`'s choice ([`Layout`]); a Letter sheet is 816 pixels wide, so the page is scrolled sideways until the 6.5-inch text
//! column fills the area beside the vertical bar.

use quill_app::picture::WINDOW;
use quill_app::view::{
	hscroll_max, page_width, ruler_marks, ruler_ticks, tab_x, text_extent, RulerState, PAGE_PAD,
	RULER_H,
};
use quill_app::{status_text, App, Overlay};
use quill_core::layout::{to_units, Measure};
use quill_core::store::Store;

use crate::layout::Layout;
use crate::page::{paint_page, PageInfo, PageView};
use crate::scroll;
use crate::ui::{bevel, Ui, FACE, INK, LIGHT, SHADOW};
use crate::{dialog, menu, toolbar};
use crate::{Rect, Surface};

/// The screen's width.
pub const SCREEN_W: i32 = 640;
/// The screen's height.
pub const SCREEN_H: i32 = 480;
/// The height of the menu bar.
pub const MENU_H: i32 = 20;
/// The height of the status line.
pub const STATUS_H: i32 = 20;

/// Tells the app how big the page area is, so scrolling keeps the caret in view and Page Up and Page Down move a screenful. The page
/// area changes with the View menu (a hidden toolbar gives its rows to the page), so the shell calls this after every key.
pub fn sync_view<M: Measure + Clone>(app: &mut App<M>) {
	let page = Layout::of(app).page;
	app.set_view_height(page.height());
	app.set_rows((page.height() / 16).max(1) as usize);
}

/// How far the page is scrolled sideways in a page area `client_w` wide: until the text column starts at the left edge, or as far as
/// the page can go.
pub fn hscroll<M: Measure + Clone>(app: &App<M>, client_w: i32) -> i32 {
	let margin = to_units(app.doc().page.margin_l, 96);
	(margin + PAGE_PAD).min(hscroll_max(app, client_w)).max(0)
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
	r: Rect,
	client_w: i32,
	hs: i32,
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
fn scroll_bars<Sf: Surface, M: Measure + Clone, St: Store + ?Sized>(
	sf: &mut Sf,
	app: &App<M>,
	s: &St,
	lay: &Layout,
	hs: i32,
) {
	let total = app.line_total(s) as i32;
	let view = (app.page_rows() as i32).min(total);
	let v = scroll::parts(lay.vbar, true, app.top_line() as i32, total, view);
	scroll::draw(sf, &v, total > view);
	if let Some(hbar) = lay.hbar {
		let sheet = page_width(app) + 2 * PAGE_PAD;
		let h = scroll::parts(hbar, false, hs, sheet, lay.page.width());
		scroll::draw(sf, &h, sheet > lay.page.width());
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
	let ui = Ui::new(sf, m);
	let open = app.menu_open();
	menu::draw_bar(sf, &ui, open.map(|(m, _)| m));
	toolbar::draw(sf, &ui, app, s, &lay);
	if let Some(r) = lay.ruler {
		ruler(sf, &ui, app, s, r, lay.page.width(), hs);
	}
	scroll_bars(sf, app, s, &lay, hs);
	if let Some(r) = lay.status {
		status_line(sf, &ui, app, s, r);
	}
	if let Some((menu, item)) = open {
		menu::draw_menu(sf, &ui, app, s, menu, item);
	}
	let mut ov = Overlay::default();
	if app.overlay(s, &mut ov) {
		dialog::draw_dialog(sf, &ui, &ov);
	}
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
