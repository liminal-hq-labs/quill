// The page view's vertical scroll in pixels: an empty or short page scrolls down its whole sheet, a long document to the foot of its
// last one, and the keys, the thumb and the wheel agree with the position.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use proptest::prelude::*;
use quill_app::view::{PageGeom, PAGE_GAP, PAGE_TOP, SCROLL_LINE, SHEET_SHADOW};
use quill_app::{App, Key};
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

/// A letter sheet is 11 inches, 1,056 pixels, high.
const SHEET: i32 = 1056;

fn doc(lines: u32, view_h: i32) -> (App<Fixed>, HostStore, MemFiles) {
	let mut s = HostStore::new();
	let mut f = MemFiles::default();
	let mut app = App::with_measure(Fixed);
	app.reset(&mut s).unwrap();
	for i in 0..lines {
		app.key(&mut s, &mut f, Key::Char(b'a' + (i % 26) as u8));
		if i + 1 < lines {
			app.key(&mut s, &mut f, Key::Enter);
		}
	}
	app.key(&mut s, &mut f, Key::CtrlHome);
	app.set_view_height(view_h);
	(app, s, f)
}

/// Where the foot of the last sheet is in the window, shadow and a little desk included, when the page is scrolled as far as it goes.
fn foot_at_max(app: &mut App<Fixed>, s: &HostStore) -> i32 {
	let max = app.vscroll_max(s);
	app.set_vscroll_pos(s, max);
	let g = PageGeom::of(app, s);
	let last = app.page_total(s).max(1) as i32 - 1;
	g.page_y(last, 0) + g.page_h + SHEET_SHADOW + PAGE_TOP
}

#[test]
fn an_empty_page_scrolls_to_the_foot_of_its_sheet() {
	let (mut app, s, _) = doc(0, 300);
	assert_eq!(app.vscroll_pos(&s), 0);
	let max = app.vscroll_max(&s);
	assert!(max > 0, "an empty page has something to scroll: {max}");
	assert_eq!(foot_at_max(&mut app, &s), 300, "to the foot of the sheet");
	assert_eq!(app.vscroll_pos(&s), max);
	assert!(app.set_vscroll_pos(&s, 0), "and back");
	assert_eq!(app.vscroll_pos(&s), 0);
}

#[test]
fn a_page_that_fits_the_window_has_nothing_to_scroll() {
	let (app, s, _) = doc(3, SHEET + 400);
	assert_eq!(app.vscroll_max(&s), 0);
}

#[test]
fn nothing_scrolls_before_the_window_height_is_known() {
	let (mut app, s, _) = doc(0, 0);
	assert_eq!(app.vscroll_max(&s), 0);
	assert!(!app.set_vscroll_pos(&s, 100));
	assert_eq!(app.vscroll_pos(&s), 0);
}

#[test]
fn a_long_document_scrolls_to_the_foot_of_its_last_sheet() {
	// 140 lines of 16 pixels are two sheets and a bit.
	let (mut app, s, _) = doc(140, 380);
	assert!(app.page_total(&s) >= 2);
	assert_eq!(foot_at_max(&mut app, &s), 380);
	assert!(app.vscroll_max(&s) > SHEET + PAGE_GAP);
}

#[test]
fn the_pixel_position_is_what_was_asked_for() {
	let (mut app, s, _) = doc(140, 380);
	let max = app.vscroll_max(&s);
	for want in [
		0,
		1,
		15,
		16,
		17,
		500,
		999,
		1000,
		1066,
		1067,
		2000,
		max - 1,
		max,
	] {
		app.set_vscroll_pos(&s, want);
		assert_eq!(app.vscroll_pos(&s), want.min(max), "asked for {want}");
	}
	app.set_vscroll_pos(&s, -5);
	assert_eq!(app.vscroll_pos(&s), 0, "never above the first line");
	app.set_vscroll_pos(&s, i32::MAX);
	assert_eq!(app.vscroll_pos(&s), max, "never past the foot");
}

#[test]
fn a_line_scroll_puts_the_page_back_on_a_line() {
	let (mut app, s, _) = doc(10, 380);
	app.set_vscroll_pos(&s, 100);
	app.scroll_to(&s, 3);
	assert_eq!(app.top_line(), 3);
	assert_eq!(app.vscroll_pos(&s), 3 * 16, "the lines are 16 pixels apart");
}

#[test]
fn typing_on_a_scrolled_empty_page_brings_the_caret_back() {
	let (mut app, mut s, mut f) = doc(0, 300);
	app.set_vscroll_pos(&s, 400);
	assert_eq!(app.vscroll_pos(&s), 400);
	app.key(&mut s, &mut f, Key::Char(b'x'));
	assert_eq!(app.vscroll_pos(&s), 0, "the first line is in view again");
}

#[test]
fn a_key_brings_the_caret_back_from_a_page_scrolled_past_it() {
	let (mut app, mut s, mut f) = doc(40, 300);
	app.set_vscroll_pos(&s, 5 * 16 + 3);
	app.key(&mut s, &mut f, Key::Down);
	assert_eq!(
		app.vscroll_pos(&s),
		SCROLL_LINE,
		"the caret's line, the second, is at the top"
	);
}

#[test]
fn page_down_and_page_up_scroll_an_empty_page_a_screenful() {
	let (mut app, mut s, mut f) = doc(0, 300);
	app.key(&mut s, &mut f, Key::PageDown);
	let step = 300 - 2 * PAGE_TOP;
	assert_eq!(
		app.vscroll_pos(&s),
		step,
		"a screenful less a little overlap"
	);
	app.key(&mut s, &mut f, Key::PageDown);
	assert_eq!(app.vscroll_pos(&s), 2 * step);
	app.key(&mut s, &mut f, Key::PageUp);
	assert_eq!(app.vscroll_pos(&s), step);
	for _ in 0..10 {
		app.key(&mut s, &mut f, Key::PageDown);
	}
	assert_eq!(
		app.vscroll_pos(&s),
		app.vscroll_max(&s),
		"and stops at the foot"
	);
	for _ in 0..10 {
		app.key(&mut s, &mut f, Key::PageUp);
	}
	assert_eq!(app.vscroll_pos(&s), 0);
}

#[test]
fn ctrl_end_and_ctrl_home_go_to_the_ends_of_an_empty_page() {
	let (mut app, mut s, mut f) = doc(0, 300);
	app.key(&mut s, &mut f, Key::CtrlEnd);
	assert_eq!(app.vscroll_pos(&s), app.vscroll_max(&s));
	app.key(&mut s, &mut f, Key::CtrlEnd);
	assert_eq!(
		app.vscroll_pos(&s),
		app.vscroll_max(&s),
		"again, still at the foot"
	);
	app.key(&mut s, &mut f, Key::CtrlHome);
	assert_eq!(app.vscroll_pos(&s), 0);
}

#[test]
fn page_down_moves_the_caret_before_it_moves_the_page() {
	let (mut app, mut s, mut f) = doc(100, 300);
	app.key(&mut s, &mut f, Key::PageDown);
	assert!(app.caret() > 0, "the caret goes down a screenful of lines");
	assert_eq!(
		app.vscroll_pos(&s) % SCROLL_LINE,
		0,
		"and the page is on a line"
	);
}

#[test]
fn a_step_scrolls_pixels_and_stops_at_both_ends() {
	let (mut app, s, _) = doc(0, 300);
	assert!(app.vscroll_by(&s, 3 * SCROLL_LINE));
	assert_eq!(app.vscroll_pos(&s), 3 * SCROLL_LINE);
	assert!(app.vscroll_by(&s, -10 * SCROLL_LINE));
	assert_eq!(app.vscroll_pos(&s), 0);
	assert!(!app.vscroll_by(&s, -SCROLL_LINE), "already at the top");
	assert!(app.vscroll_by(&s, i32::MAX));
	assert!(!app.vscroll_by(&s, 1), "and at the foot");
}

proptest! {
	#![proptest_config(ProptestConfig { cases: 48, ..ProptestConfig::default() })]

	/// For any length of document and any window height, the range ends with the foot of the last sheet in the window, the position
	/// reads back as set and the page never scrolls past the range or above the top.
	#[test]
	fn the_range_covers_the_whole_page(lines in 0u32..180, view_h in 60i32..900, want in -200i32..9000) {
		let (mut app, s, _) = doc(lines, view_h);
		let max = app.vscroll_max(&s);
		prop_assert!(max >= 0);
		app.set_vscroll_pos(&s, want);
		prop_assert_eq!(app.vscroll_pos(&s), want.clamp(0, max));
		if max > 0 {
			prop_assert_eq!(foot_at_max(&mut app, &s), view_h);
		} else {
			// Nothing to scroll: the sheets, shadow and desk all fit, or the first line already is the lowest the page goes.
			let g = PageGeom::of(&app, &s);
			let last = app.page_total(&s).max(1) as i32 - 1;
			let foot = g.page_y(last, 0) + g.page_h + SHEET_SHADOW + PAGE_TOP;
			prop_assert!(foot <= view_h || view_h < SHEET);
		}
	}
}
