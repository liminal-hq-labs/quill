// The zoom on the graphics screen: the page drawn at a percentage, what a click means at each, the scroll bars, and the control.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod common;

use common::*;
use quill_app::mouse::{Ctx, Ev};
use quill_chrome::view::{dev, log, page_width, page_x0, PageGeom, PAGE_PAD};
use quill_chrome::zoom_field::{ZoomPart, ZOOM_LEVELS, ZOOM_MARGIN};
use quill_core::layout::{to_units, FontRef};
use quill_host::script::MemFiles;
use quill_host::HostStore;
use quill_paint::drop::{self, Pop};
use quill_paint::layout::Layout;
use quill_paint::pointer::Gfx;
use quill_paint::raster::{Planar4, StrikeFont, PLANAR_BYTES};
use quill_paint::shell::{self, paint_screen_view, SCREEN_W};
use quill_paint::zoombox;
use quill_paint::{Rgb, Surface};
use quill_picture::WINDOW;
use quill_strike::{SliceSource, StrikeMeasure};

struct ScreenJob<'a> {
	app: &'a TApp,
	s: &'a HostStore,
}

impl Job for ScreenJob<'_> {
	fn run<Sf: Surface>(&mut self, sf: &mut Sf) {
		let mut m = StrikeMeasure::new(strikes());
		let mut window = Box::new([0u8; WINDOW]);
		paint_screen_view(
			sf,
			self.app,
			&mut m,
			self.s,
			&mut window,
			&Default::default(),
		);
	}
}

fn screen(app: &TApp, s: &HostStore) -> Frame {
	frame(Mode::C256, &mut ScreenJob { app, s })
}

fn zoomed(name: &str, zoom: u16) -> (TApp, HostStore, MemFiles) {
	let (mut app, s, f) = open_doc(name);
	app.set_zoom(zoom);
	shell::sync_view(&mut app, &s);
	(app, s, f)
}

#[test]
fn the_control_leaves_a_margin_at_the_right_edge_and_each_part_has_its_own_clicks() {
	let l = zoombox::layout();
	assert_eq!(l.right, SCREEN_W);
	assert_eq!(l.right - l.reset.1, ZOOM_MARGIN);
	assert_eq!(ZOOM_MARGIN, 8);
	// Page Width's button is right of the plus, 20 pixels wide.
	assert_eq!(l.plus.1, l.reset.0);
	assert_eq!(l.reset.1 - l.reset.0, 20);
	let (app, _, _) = open_doc("letter.rtf");
	let lay = Layout::of(&app);
	let y = lay.status.unwrap().top + 10;
	assert_eq!(zoombox::part_at(&lay, l.minus.0, y), Some(ZoomPart::Minus));
	assert_eq!(
		zoombox::part_at(&lay, l.minus.1 - 1, y),
		Some(ZoomPart::Minus)
	);
	assert_eq!(zoombox::part_at(&lay, l.field.0, y), Some(ZoomPart::Field));
	assert_eq!(zoombox::part_at(&lay, l.plus.0, y), Some(ZoomPart::Plus));
	assert_eq!(
		zoombox::part_at(&lay, l.plus.1 - 1, y),
		Some(ZoomPart::Plus)
	);
	assert_eq!(zoombox::part_at(&lay, l.plus.1, y), Some(ZoomPart::Reset));
	assert_eq!(zoombox::part_at(&lay, l.reset.0, y), Some(ZoomPart::Reset));
	assert_eq!(
		zoombox::part_at(&lay, l.reset.1 - 1, y),
		Some(ZoomPart::Reset)
	);
	assert_eq!(zoombox::part_at(&lay, l.reset.1, y), None);
	assert_eq!(zoombox::part_at(&lay, SCREEN_W - 1, y), None);
	// The status text ends 3 pixels before the control's first button.
	assert_eq!(zoombox::text_right(), l.minus.0 - 3);
	assert_eq!(zoombox::part_at(&lay, l.minus.0 - 1, y), None);
	// Above the status line is the page, not the control.
	assert_eq!(
		zoombox::part_at(&lay, l.plus.0, lay.status.unwrap().top - 1),
		None
	);
}

#[test]
fn page_width_is_the_largest_zoom_up_to_100_that_fits() {
	for name in ["letter.rtf", "wide.rtf"] {
		let (mut app, _, _) = open_doc(name);
		app.set_zoom(0);
		let at = shell::pct(&app);
		let sheet = page_width(&app);
		assert!(dev(sheet, at) + 2 * PAGE_PAD <= SCREEN_W - 16, "{at}");
		assert!(
			at == 100 || dev(sheet, at + 1) + 2 * PAGE_PAD > SCREEN_W - 16,
			"{at}"
		);
	}
	let (mut app, _, _) = open_doc("letter.rtf");
	app.set_zoom(0);
	assert_eq!(shell::pct(&app), 74);
	app.set_zoom(150);
	assert_eq!(shell::pct(&app), 150);
}

#[test]
fn a_click_lands_on_the_character_drawn_there_at_every_zoom() {
	for zoom in [0u16, 50, 75, 100, 125, 150, 200] {
		let (mut app, s, _) = zoomed("letter.rtf", zoom);
		let at = shell::pct(&app);
		let (mut checked, mut exact) = (0, 0);
		for pos in (0..300).step_by(7) {
			app.place_caret(&s, pos, false);
			shell::sync_view(&mut app, &s);
			shell::follow_caret(&mut app, &s);
			let lay = Layout::of(&app);
			let Some((_, l, x)) = app.caret_xy(&s) else {
				continue;
			};
			let g = PageGeom::of(&app, &s);
			let hs = shell::hscroll(&app, lay.page.width());
			let x0 = lay.page.left + page_x0(lay.page.width(), dev(page_width(&app), at), hs);
			let margin = to_units(app.doc().page.margin_l, 96);
			let px = x0 + dev(margin + x, at);
			let py = lay.page.top + dev(g.line_y(&l, 0), at) + dev(l.ascent, at) / 2;
			if px < lay.page.left
				|| px >= lay.page.right
				|| py < lay.page.top
				|| py >= lay.page.bottom
			{
				continue;
			}
			let hit = shell::pos_at_pixel(&app, &s, px, py);
			// A position at the end of one line and the start of the next is one place on the screen to within a character.
			assert!(
				hit.pos.abs_diff(app.caret()) <= 1,
				"zoom {zoom}, position {pos}, pixel {px},{py}: {}",
				hit.pos
			);
			exact += u32::from(hit.pos == app.caret());
			checked += 1;
		}
		assert!(
			checked > 5,
			"zoom {zoom}: only {checked} positions were on the screen"
		);
		assert!(
			exact * 10 >= checked * 9,
			"zoom {zoom}: {exact} of {checked} exact"
		);
	}
}

#[test]
fn the_sheet_is_drawn_at_the_zoom() {
	for zoom in [50u16, 75, 100, 150] {
		let (app, s, _) = zoomed("letter.rtf", zoom);
		let fr = screen(&app, &s);
		let lay = Layout::of(&app);
		let at = i32::from(zoom);
		let hs = shell::hscroll(&app, lay.page.width());
		let w = dev(page_width(&app), at);
		let x0 = page_x0(lay.page.width(), w, hs);
		// A row inside the sheet's top margin, which has no text on it, and so no other pixel than the paper.
		let y = (lay.page.top + 12) as usize;
		let white = fr.pixels[y * 640 + (x0.max(0) + 1) as usize];
		if x0 >= 1 {
			assert_eq!(
				fr.pixels[y * 640 + (x0 - 1) as usize],
				0,
				"the edge, zoom {zoom}"
			);
			assert_ne!(white, 0, "zoom {zoom}");
		}
		if x0 + w < lay.page.width() {
			assert_eq!(
				fr.pixels[y * 640 + (x0 + w - 1) as usize],
				white,
				"right, zoom {zoom}"
			);
			assert_eq!(
				fr.pixels[y * 640 + (x0 + w) as usize],
				0,
				"right edge, zoom {zoom}"
			);
		}
	}
}

/// How tall and wide the ink of a screen's text is: the rectangle round every pixel that is neither paper, desk nor chrome.
fn ink_box(fr: &Frame, from: (i32, i32), to: (i32, i32)) -> Option<(i32, i32, i32, i32)> {
	let mut b: Option<(i32, i32, i32, i32)> = None;
	for y in from.1..to.1 {
		for x in from.0..to.0 {
			if fr.pixels[(y * 640 + x) as usize] == 0 {
				b = Some(match b {
					None => (x, y, x, y),
					Some((a, c, d, e)) => (a.min(x), c.min(y), d.max(x), e.max(y)),
				});
			}
		}
	}
	b
}

#[test]
fn the_text_shrinks_with_the_zoom_and_stays_inside_the_sheet() {
	let width_of = |zoom: u16| {
		let (app, s, _) = zoomed("letter.rtf", zoom);
		let lay = Layout::of(&app);
		let fr = screen(&app, &s);
		let w = dev(page_width(&app), i32::from(zoom));
		let x0 = page_x0(lay.page.width(), w, shell::hscroll(&app, lay.page.width()));
		let from = ((x0 + 2).max(0), lay.page.top + 4);
		let b = ink_box(
			&fr,
			from,
			((x0 + w - 2).min(lay.page.width()), lay.page.bottom - 4),
		)
		.unwrap();
		(b.2 - b.0, b.3 - b.1)
	};
	let (w100, h100) = width_of(100);
	let (w50, h50) = width_of(50);
	let (w150, _) = width_of(150);
	assert!((w50 * 2 - w100).abs() <= 24, "{w50} {w100}");
	assert!((h50 * 2 - h100).abs() <= 24, "{h50} {h100}");
	assert!(w150 > w100 - 8, "{w150} {w100}");
}

fn glyph_ink(font: StrikeFont, ch: u8) -> Vec<u8> {
	let mut src = SliceSource(font_bytes());
	let mut buf = vec![0u8; PLANAR_BYTES];
	let mut sf = Planar4::new(&mut buf, *strikes(), &mut src).unwrap();
	sf.fill_rect(quill_paint::Rect::new(0, 0, 640, 480), Rgb::WHITE);
	sf.glyph_run(font, 100, &[(ch, 100)], Rgb::BLACK, true);
	planar_pixels(&buf)
}

fn roman(size: u16) -> FontRef {
	let mut face = [0u8; 31];
	face[..15].copy_from_slice(b"Times New Roman");
	FontRef {
		face,
		face_len: 15,
		family: 1,
		size,
		flags: 0,
	}
}

#[test]
fn a_resample_to_the_strikes_own_size_is_the_strike() {
	let mut src = SliceSource(font_bytes());
	let mut buf = vec![0u8; PLANAR_BYTES];
	let mut sf = Planar4::new(&mut buf, *strikes(), &mut src).unwrap();
	for size in [20u16, 24, 48] {
		let plain = sf.font(&roman(size));
		let have = (u32::from(strikes().strike(plain.id).pixels) * u32::from(plain.scale)) as u8;
		let same = StrikeFont {
			want: have,
			..plain
		};
		for ch in *b"AHMgjQ%" {
			assert_eq!(
				glyph_ink(plain, ch),
				glyph_ink(same, ch),
				"size {size}, {}",
				ch as char
			);
		}
	}
}

#[test]
fn a_resample_to_half_the_size_has_half_the_ink_and_the_same_baseline() {
	let mut src = SliceSource(font_bytes());
	let mut buf = vec![0u8; PLANAR_BYTES];
	let font = Planar4::new(&mut buf, *strikes(), &mut src)
		.unwrap()
		.font(&roman(24));
	let have = (u32::from(strikes().strike(font.id).pixels) * u32::from(font.scale)) as u8;
	let half = StrikeFont {
		want: have / 2,
		..font
	};
	let bounds = |pixels: &[u8]| {
		let fr = Frame {
			native: vec![],
			pixels: pixels.to_vec(),
		};
		// Planar pixels are 0 for black; the ink is whatever is not white (15 on the 16-colour screen).
		let inked: Vec<u8> = fr.pixels.iter().map(|p| u8::from(*p != 15)).collect();
		let mut top = i32::MAX;
		let mut bottom = i32::MIN;
		let mut left = i32::MAX;
		let mut right = i32::MIN;
		for (n, v) in inked.iter().enumerate() {
			if *v != 0 {
				let (x, y) = ((n % 640) as i32, (n / 640) as i32);
				top = top.min(y);
				bottom = bottom.max(y);
				left = left.min(x);
				right = right.max(x);
			}
		}
		(left, top, right, bottom)
	};
	let full = bounds(&glyph_ink(font, b'H'));
	let small = bounds(&glyph_ink(half, b'H'));
	// A capital stands on the baseline (row 100 here): its foot is the same row, its height and width are half, give or take a pixel.
	assert_eq!(full.3, small.3);
	assert!(
		((full.3 - full.1 + 1) - 2 * (small.3 - small.1 + 1)).abs() <= 2,
		"{full:?} {small:?}"
	);
	assert!(
		((full.2 - full.0 + 1) - 2 * (small.2 - small.0 + 1)).abs() <= 2,
		"{full:?} {small:?}"
	);
}

struct Rig {
	app: TApp,
	s: HostStore,
	f: MemFiles,
	g: Gfx,
	tick: u32,
}

impl Rig {
	fn new(name: &str, zoom: u16) -> Rig {
		let (app, s, f) = zoomed(name, zoom);
		Rig {
			app,
			s,
			f,
			g: Gfx::new(&mut StrikeMeasure::new(strikes())),
			tick: 0,
		}
	}

	fn ev(&mut self, ev: Ev) {
		let ctx = Ctx {
			shift: false,
			tick: self.tick,
		};
		self.g
			.handle(&mut self.app, &mut self.s, &mut self.f, ev, ctx);
	}

	fn click(&mut self, (x, y): (i32, i32)) {
		self.tick += 1;
		self.ev(Ev::Move { x, y });
		self.ev(Ev::Down { x, y });
		self.ev(Ev::Up { x, y });
		self.tick += 100;
	}

	fn part(&self, part: ZoomPart) -> (i32, i32) {
		let l = zoombox::layout();
		let x = match part {
			ZoomPart::Minus => l.minus.0 + 5,
			ZoomPart::Field => l.field.0 + 5,
			ZoomPart::Plus => l.plus.0 + 5,
			ZoomPart::Reset => l.reset.0 + 5,
		};
		(x, Layout::of(&self.app).status.unwrap().top + 10)
	}
}

#[test]
fn the_buttons_step_through_the_levels_and_grey_at_the_ends() {
	let mut r = Rig::new("letter.rtf", 0);
	// From Page Width (74 percent): plus goes to 75, then up the list to 200, where it stops.
	for want in [75u16, 100, 125, 150, 200, 200] {
		r.click(r.part(ZoomPart::Plus));
		assert_eq!(r.app.zoom(), want);
	}
	for want in [150u16, 125, 100, 75, 50, 50, 50] {
		r.click(r.part(ZoomPart::Minus));
		assert_eq!(r.app.zoom(), want);
	}
	// Page Width is left by minus too: it steps from the 74 shown.
	r.app.set_zoom(0);
	r.click(r.part(ZoomPart::Minus));
	assert_eq!(r.app.zoom(), 50);
}

#[test]
fn the_page_width_button_sets_page_width_and_does_nothing_at_it() {
	let mut r = Rig::new("letter.rtf", 150);
	r.click(r.part(ZoomPart::Reset));
	assert_eq!(r.app.zoom(), 0);
	// At Page Width it is greyed and a click leaves the setting alone.
	r.click(r.part(ZoomPart::Reset));
	assert_eq!(r.app.zoom(), 0);
	assert!(r.g.view().drop.is_none());
	// A press that leaves it before the release runs nothing.
	r.app.set_zoom(100);
	let (x, y) = r.part(ZoomPart::Reset);
	r.ev(Ev::Down { x, y });
	assert_eq!(r.g.view().zoom, Some(ZoomPart::Reset), "sunk while held");
	r.ev(Ev::Move { x: 300, y: 300 });
	r.ev(Ev::Up { x: 300, y: 300 });
	assert_eq!(r.app.zoom(), 100);
}

/// The pixels of the Page Width button's face that are drawn in `colour`.
fn reset_pixels(fr: &Frame, app: &TApp, colour: u8) -> usize {
	let l = zoombox::layout();
	let st = Layout::of(app).status.unwrap();
	let mut n = 0;
	for y in st.top + 3..st.bottom - 2 {
		for x in l.reset.0 + 3..l.reset.1 - 3 {
			n += usize::from(fr.pixels[(y * 640 + x) as usize] == colour);
		}
	}
	n
}

#[test]
fn the_page_width_glyph_is_black_when_live_and_embossed_at_page_width() {
	let (app, s, _) = zoomed("letter.rtf", 150);
	let live = screen(&app, &s);
	let (app0, s0, _) = zoomed("letter.rtf", 0);
	let grey = screen(&app0, &s0);
	// Ink is black (0) on the 256-colour frame; the bevels are grey and white, so the button's inner pixels count only glyph ink.
	let ink = 0;
	let live_ink = reset_pixels(&live, &app, ink);
	let grey_ink = reset_pixels(&grey, &app0, ink);
	assert!(live_ink >= 20, "{live_ink}");
	assert_eq!(grey_ink, 0, "the greyed glyph has no ink");
}

#[test]
fn a_press_that_leaves_the_button_runs_nothing_and_the_margin_is_dead() {
	let mut r = Rig::new("letter.rtf", 100);
	let (x, y) = r.part(ZoomPart::Plus);
	r.ev(Ev::Down { x, y });
	r.ev(Ev::Move { x: 300, y: 300 });
	r.ev(Ev::Up { x: 300, y: 300 });
	assert_eq!(r.app.zoom(), 100);
	let y = Layout::of(&r.app).status.unwrap().top + 10;
	r.click((SCREEN_W - 3, y));
	assert_eq!(r.app.zoom(), 100);
	assert!(r.g.view().drop.is_none());
}

#[test]
fn the_box_lists_the_levels_and_a_choice_sets_the_zoom() {
	let mut r = Rig::new("letter.rtf", 75);
	r.click(r.part(ZoomPart::Field));
	let (pop, hot) = r.g.view().drop.expect("the list opens");
	assert_eq!(pop, Pop::Zoom);
	assert_eq!(hot, 5, "75 percent is lit");
	assert_eq!(pop.count(), ZOOM_LEVELS.len());
	let lay = Layout::of(&r.app);
	let list = drop::list_rect(&lay, Pop::Zoom).unwrap();
	assert_eq!(
		list.bottom,
		lay.status.unwrap().top,
		"it rises from the status line"
	);
	// Choose 150, the third entry.
	r.click((list.left + 10, drop::item_rect(list, 2).top + 5));
	assert_eq!(r.app.zoom(), 150);
	assert!(r.g.view().drop.is_none());
	// Page Width, the first.
	r.click(r.part(ZoomPart::Field));
	let list = drop::list_rect(&Layout::of(&r.app), Pop::Zoom).unwrap();
	r.click((list.left + 10, drop::item_rect(list, 0).top + 5));
	assert_eq!(r.app.zoom(), 0);
	// A press elsewhere shuts the list and changes nothing.
	r.click(r.part(ZoomPart::Field));
	r.click((300, 200));
	assert!(r.g.view().drop.is_none());
	assert_eq!(r.app.zoom(), 0);
	// The keys move in it: down, down, Enter chooses the third.
	r.click(r.part(ZoomPart::Field));
	for k in [
		quill_app::Key::Down,
		quill_app::Key::Down,
		quill_app::Key::Enter,
	] {
		assert!(r.g.key(&mut r.app, &mut r.s, k));
	}
	assert_eq!(r.app.zoom(), 150);
}

#[test]
fn the_view_follows_the_zoom_in_page_units() {
	let (mut app, s, _) = zoomed("scroll.rtf", 100);
	let lay = Layout::of(&app);
	assert_eq!(app.vscroll_view(), lay.page.height());
	app.set_zoom(50);
	shell::sync_view(&mut app, &s);
	let lay = Layout::of(&app);
	assert_eq!(app.vscroll_view(), log(lay.page.height(), 50));
	// At 50 percent the whole sheet fits the width: no sideways bar. At 150 it does not.
	assert!(lay.hbar.is_none());
	app.set_zoom(150);
	shell::sync_view(&mut app, &s);
	assert!(Layout::of(&app).hbar.is_some());
	assert!(shell::hrange(&app, &s, 624) > 0);
}

#[test]
fn an_arrow_of_the_scroll_bar_moves_sixteen_screen_pixels_at_any_zoom() {
	for zoom in [50u16, 100, 200] {
		let mut r = Rig::new("scroll.rtf", zoom);
		let lay = Layout::of(&r.app);
		let before = r.app.vscroll_pos(&r.s);
		r.click((lay.vbar.left + 8, lay.vbar.bottom - 8));
		let moved = r.app.vscroll_pos(&r.s) - before;
		assert_eq!(moved, log(16, i32::from(zoom)), "zoom {zoom}");
		let on_screen = dev(moved, i32::from(zoom));
		assert!((15..=16).contains(&on_screen), "zoom {zoom}: {on_screen}");
	}
}

#[test]
fn the_ruler_follows_the_zoom() {
	let (app, s, _) = zoomed("letter.rtf", 50);
	let lay = Layout::of(&app);
	let fr = screen(&app, &s);
	// The scale is a white strip between two black lines over the text column: half the width of the 100 percent column (6.5 inches,
	// 624 pixels), and the page's own x0 is 0 only when it fills the window, so read the column's edges.
	let r = lay.ruler.unwrap();
	let y = (r.top + 9) as usize;
	let (left, w) = quill_chrome::view::text_extent_at(&app, 624, shell::hscroll(&app, 624), 50);
	assert_eq!(w, to_units(app.doc().page.text_w(), 96) / 2);
	assert_eq!(fr.pixels[y * 640 + (left - 1) as usize], 0);
	assert_eq!(fr.pixels[y * 640 + (left + w) as usize], 0);
	assert_eq!(
		fr.pixels[y * 640 + (left + 2) as usize],
		quill_paint::palette::index256(Rgb::WHITE)
	);
}

#[test]
fn quill_ini_zoom_overrides_the_start() {
	let (mut app, _s, mut f) = new_app();
	assert_eq!(app.zoom(), shell::START_ZOOM);
	f.files
		.insert("QUILL.INI".into(), b"[Quill]\r\nZoom=150\r\n".to_vec());
	app.load_settings(&mut f);
	assert_eq!(app.zoom(), 150);
	f.files
		.insert("QUILL.INI".into(), b"[Quill]\r\nZoom=Width\r\n".to_vec());
	app.load_settings(&mut f);
	assert_eq!(app.zoom(), 0);
}
