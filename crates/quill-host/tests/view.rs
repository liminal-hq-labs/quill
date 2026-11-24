// The shared window geometry: the toolbar's layout, what a click on the toolbar, the ruler or the page hits, and the status text.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use quill_app::view::{self, Act, Hit, PageGeom, RulerState, Toggle, TOOLBAR};
use quill_app::{App, Command, Key, CHROME_TOOLBAR};
use quill_core::layout::{FontMetrics, FontRef, Measure, GLYPHS};
use quill_host::script::MemFiles;
use quill_host::HostStore;

/// A screen at 96 dots per inch where every character is 8 pixels wide and a line 16 high.
#[derive(Clone)]
struct Fixed;

impl Measure for Fixed {
	fn resolution(&self) -> (u16, u16) {
		(96, 96)
	}
	fn metrics(&mut self, _: &FontRef) -> FontMetrics {
		FontMetrics {
			ascent: 12,
			descent: 4,
			leading: 0,
		}
	}
	fn widths(&mut self, _: &FontRef, out: &mut [u16; GLYPHS]) {
		out.fill(8);
	}
}

fn app_with(text: &str) -> (App<Fixed>, HostStore, MemFiles) {
	let mut s = HostStore::new();
	let mut f = MemFiles::default();
	let mut app = App::with_measure(Fixed);
	app.reset(&mut s).unwrap();
	app.set_native_dialogs(true);
	for b in text.bytes() {
		let k = if b == b'\n' { Key::Enter } else { Key::Char(b) };
		app.key(&mut s, &mut f, k);
	}
	(app, s, f)
}

#[test]
fn the_toolbar_is_laid_out_as_the_win32_build_drew_it() {
	let row0: Vec<i32> = TOOLBAR
		.buttons
		.iter()
		.filter(|b| b.row == 0)
		.map(|b| b.x)
		.collect();
	let row1: Vec<i32> = TOOLBAR
		.buttons
		.iter()
		.filter(|b| b.row == 1)
		.map(|b| b.x)
		.collect();
	assert_eq!(row0, [4, 28, 52, 76, 100, 132, 156, 180, 212, 236, 268]);
	assert_eq!(row1, [312, 336, 360, 384, 416, 440, 464, 488, 520, 544]);
	assert_eq!(
		(TOOLBAR.style, TOOLBAR.face, TOOLBAR.size),
		((4, 104), (112, 140), (256, 48))
	);
	let glyphs: Vec<i32> = TOOLBAR.buttons.iter().map(|b| b.glyph).collect();
	assert_eq!(
		glyphs,
		[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 20, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19]
	);
	// Both rows end well inside a 640-pixel window.
	assert!(TOOLBAR.buttons.iter().all(|b| b.x + view::BTN_W < 600));
}

#[test]
fn a_click_on_the_toolbar_finds_its_button_and_follows_the_hidden_rows() {
	let (mut app, mut s, mut f) = app_with("");
	let (b, top) = view::button_at(&app, 30, 10).unwrap();
	assert_eq!((b.act, top), (Act::Cmd(Command::Open), 0));
	let (b, top) = view::button_at(&app, 313, 30).unwrap();
	assert_eq!(
		(b.act, b.toggle, top),
		(Act::Cmd(Command::Bold), Toggle::Bold, 28)
	);
	assert!(
		view::button_at(&app, 30, 1).is_none(),
		"the bevel's top edge is not the button"
	);
	assert!(
		view::button_at(&app, 125, 10).is_none(),
		"the gap between groups"
	);
	assert_eq!(view::chrome_height(&app), 2 * view::TB_H + view::RULER_H);
	app.command(&mut s, &mut f, Command::Chrome(CHROME_TOOLBAR));
	let (b, top) = view::button_at(&app, 313, 10).unwrap();
	assert_eq!((b.act, top), (Act::Cmd(Command::Bold), 0));
	assert_eq!(view::chrome_height(&app), view::TB_H + view::RULER_H);
}

#[test]
fn the_page_is_centred_when_it_fits_and_scrolls_sideways_when_it_does_not() {
	let (app, _s, _f) = app_with("");
	let page_w = view::page_width(&app);
	assert_eq!(page_w, 816, "8.5 inches at 96 dpi");
	assert_eq!(view::page_x0(1000, page_w, 0), 92);
	assert_eq!(view::hscroll_max(&app, 1000), 0);
	// A 640 by 480 screen: the page starts at the padding and can scroll by the rest.
	assert_eq!(view::page_x0(632, page_w, 0), view::PAGE_PAD);
	assert_eq!(view::page_x0(632, page_w, 100), view::PAGE_PAD - 100);
	assert_eq!(view::hscroll_max(&app, 632), 816 + 16 - 632);
	let (left, w) = view::text_extent(&app, 1000, 0);
	assert_eq!((left, w), (92 + 120, 816 - 240));
}

#[test]
fn the_ruler_finds_its_markers_and_a_drag_moves_them_in_sixteenths() {
	let (mut app, mut s, _f) = app_with("a line");
	app.set_indents(&mut s, 720, -360, 360);
	app.add_tab(&mut s, 2880, 2);
	let st = RulerState::of(&app, &s);
	assert_eq!(
		(st.left, st.first, st.right, st.stops()),
		(720, 360, 360, &[(2880, 2)][..])
	);
	let (top, left, w) = (78, 100, 576);
	let (fx, lx, rx) = view::ruler_marks(&st, left, w);
	assert_eq!((fx, lx, rx), (124, 148, 652));
	assert_eq!(
		view::ruler_hit(fx, top + 3, top, left, w, &st),
		Some(Hit::First)
	);
	assert_eq!(
		view::ruler_hit(lx, top + 14, top, left, w, &st),
		Some(Hit::Left)
	);
	assert_eq!(
		view::ruler_hit(lx, top + 20, top, left, w, &st),
		Some(Hit::Both)
	);
	assert_eq!(
		view::ruler_hit(rx, top + 14, top, left, w, &st),
		Some(Hit::Right)
	);
	assert_eq!(
		view::ruler_hit(view::tab_x(2880, left), top + 10, top, left, w, &st),
		Some(Hit::Tab(2880))
	);
	assert_eq!(
		view::ruler_hit(10, top + 10, top, left, w, &st),
		Some(Hit::Kind)
	);
	assert_eq!(
		view::ruler_hit(300, top + 10, top, left, w, &st),
		Some(Hit::Scale(3000))
	);
	assert_eq!(view::ruler_hit(300, top + 30, top, left, w, &st), None);
	let mut d = view::Drag {
		hit: Hit::Both,
		state: st,
		start: st,
	};
	view::drag_to(&mut d, left + 97, left, w);
	assert_eq!(
		(d.state.left, d.state.first),
		(1440, 1080),
		"snapped to 1/16 inch, first line moved with it"
	);
	assert_eq!(view::snap(1000), 990);
	let ticks: Vec<_> = view::ruler_ticks(96).collect();
	assert_eq!(ticks.len(), 9);
	assert_eq!(ticks[8], (96, 0, 1));
}

#[test]
fn a_click_places_the_caret_and_a_drag_or_a_shift_click_selects() {
	let (mut app, s, _f) = app_with("hello world\nsecond line\nthird");
	app.click(&s, 0, 6 * 8, false);
	assert_eq!((app.caret(), app.selection()), (6, None));
	app.drag_to(&s, 0, 11 * 8);
	assert_eq!(app.selection(), Some((6, 11)));
	app.click(&s, 1, 3 * 8 + 3, false);
	assert_eq!(
		(app.caret(), app.selection()),
		(12 + 3, None),
		"nearest character boundary"
	);
	app.click(&s, 2, 2 * 8, true);
	assert_eq!(
		app.selection(),
		Some((15, 24 + 2)),
		"shift extends from the old caret"
	);
	app.click(&s, 2, 500, false);
	assert_eq!(app.caret(), 24 + 5, "past the end of a line is its end");
}

#[test]
fn a_y_in_the_window_finds_its_line_and_the_scroll_bar_moves_the_view() {
	let (mut app, s, _f) = app_with("one\ntwo\nthree\nfour");
	let area_top = 78;
	let g = PageGeom::of(&app, &s);
	let mut tops = Vec::new();
	app.for_each_line(&s, |_, l| tops.push(g.line_y(l, area_top)));
	assert_eq!(tops[0], area_top + view::PAGE_TOP);
	assert_eq!(tops[1] - tops[0], 16);
	assert_eq!(app.line_at_y(&s, area_top, tops[2] + 5), Some(2));
	assert_eq!(
		app.line_at_y(&s, area_top, 0),
		Some(0),
		"above the text: the first line shown"
	);
	assert_eq!(
		app.line_at_y(&s, area_top, 4000),
		Some(3),
		"below the text: the last line"
	);
	assert_eq!(app.line_total(&s), 4);
	app.scroll_to(&s, 2);
	assert_eq!(app.top_line(), 2);
	assert_eq!(
		app.line_at_y(&s, area_top, area_top + view::PAGE_TOP + 1),
		Some(2)
	);
	app.scroll_to(&s, 99);
	assert_eq!(app.top_line(), 3, "never past the last line");
}

#[test]
fn the_status_text_is_the_text_screens_bottom_row_in_ansi() {
	let (mut app, mut s, mut f) = app_with("caf\u{e9}");
	let mut out = [0u8; 96];
	let n = quill_app::status_text(&app, &s, &mut out);
	let text = String::from_utf8_lossy(&out[..n]).into_owned();
	assert!(text.starts_with(" (Untitled)   Pg 1 Ln 1 Col "), "{text}");
	assert!(text.contains("Times New Roman 12"), "{text}");
	app.set_message("Saved");
	let n = quill_app::status_text(&app, &s, &mut out);
	assert_eq!(&out[..n], b" Saved");
	app.key(&mut s, &mut f, Key::Left);
	let mut tiny = [0u8; 4];
	assert_eq!(quill_app::status_text(&app, &s, &mut tiny), 4);
}

#[test]
fn the_signature_changes_with_what_the_toolbar_shows_and_not_with_typing() {
	let (mut app, mut s, mut f) = app_with("abc");
	let a = view::signature(&app, &s);
	app.key(&mut s, &mut f, Key::Char(b'd'));
	assert_eq!(view::signature(&app, &s), a);
	app.command(&mut s, &mut f, Command::Bold);
	assert_ne!(view::signature(&app, &s), a);
	assert!(view::is_on(&app, &s, Toggle::Bold));
	assert_eq!(view::family_of(b"Courier New"), 3);
	assert_eq!(view::family_of(b"ARIAL"), 2);
	assert_eq!(view::family_of(b"Times New Roman"), 1);
}

#[test]
fn a_click_in_the_space_after_a_paragraph_goes_to_the_nearer_line() {
	let (mut app, mut s, mut f) = app_with("Title\nbody");
	app.key(&mut s, &mut f, Key::CtrlHome);
	app.apply_style(&mut s, 4);
	let area_top = 0;
	let g = PageGeom::of(&app, &s);
	let mut boxes = Vec::new();
	app.for_each_line(&s, |_, l| {
		let top = g.line_y(l, area_top);
		boxes.push((top, top + l.ascent + l.descent));
	});
	let (title_bottom, body_top) = (boxes[0].1, boxes[1].0);
	assert_eq!(body_top - title_bottom, 16, "Title has 12 points after it");
	assert_eq!(app.line_at_y(&s, area_top, title_bottom + 3), Some(0));
	assert_eq!(app.line_at_y(&s, area_top, body_top - 3), Some(1));
	assert_eq!(app.line_at_y(&s, area_top, boxes[0].0 + 1), Some(0));
}

#[test]
fn typing_past_the_bottom_of_a_page_scrolls_by_pixels_and_gaps() {
	let mut text = String::from("first");
	for _ in 0..70 {
		text.push('\n');
	}
	text.push_str("last");
	let (mut app, mut s, mut f) = app_with("");
	app.set_view_height(300);
	app.set_rows(40);
	for b in text.bytes() {
		let k = if b == b'\n' { Key::Enter } else { Key::Char(b) };
		app.key(&mut s, &mut f, k);
	}
	let (idx, l, _) = app.caret_xy(&s).unwrap();
	assert!(l.page >= 2, "the text runs onto a second page");
	let g = PageGeom::of(&app, &s);
	let y = g.line_y(&l, 0);
	assert!(app.top_line() < idx);
	assert!(y + 16 <= 300, "the caret's line is inside the area: {y}");
	assert!(
		y + 16 > 300 - 32,
		"and the view moved no further than it had to: {y}"
	);
	app.key(&mut s, &mut f, Key::CtrlHome);
	assert_eq!(app.top_line(), 0);
}

#[test]
fn the_scroll_bars_sit_in_the_page_area_only() {
	use quill_app::view::scroll_layout;
	// 800 by 600 client, toolbars and ruler 100 high, a status bar 20 high, bars 16 wide.
	let l = scroll_layout(800, 600, 100, 20, 16, 16, false);
	assert_eq!(l.vbar, (784, 100, 16, 480));
	assert_eq!((l.hbar, l.corner), (None, None));
	assert_eq!((l.page_w, l.page_bottom), (784, 580));
	// With the horizontal bar the page area loses its height, and the corner box fills where the bars meet.
	let l = scroll_layout(800, 600, 100, 20, 16, 16, true);
	assert_eq!(l.vbar, (784, 100, 16, 464));
	assert_eq!(l.hbar, Some((0, 564, 784, 16)));
	assert_eq!(l.corner, Some((784, 564, 16, 16)));
	// Hidden status bar: the bars reach the bottom of the window. A window too small for the chrome never gives a negative size.
	assert_eq!(
		scroll_layout(800, 600, 100, 0, 16, 16, false).vbar,
		(784, 100, 16, 500)
	);
	let tiny = scroll_layout(50, 90, 100, 20, 16, 16, true);
	assert!(tiny.vbar.3 >= 0 && tiny.hbar.unwrap().2 >= 0);
	// The bars start below the toolbars and end above the status bar at every size.
	for h in [300, 480, 600, 1000] {
		for show in [false, true] {
			let l = scroll_layout(640, h, 100, 20, 16, 16, show);
			assert_eq!(l.vbar.1, 100);
			assert_eq!(l.vbar.1 + l.vbar.3, l.page_bottom);
			assert!(l.page_bottom + if show { 16 } else { 0 } <= h - 20);
		}
	}
}
