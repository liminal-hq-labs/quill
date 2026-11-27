// A page setup read from a damaged file can hold any numbers; laying the document out must not overflow on them.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use quill_core::doc::{Doc, StoryKind};
use quill_core::layout::layout_story;
use quill_host::{GridMeasure, HostStore};

#[test]
fn extreme_margins_do_not_overflow_the_layout() {
	for (paper, margin) in [
		(i32::MIN, i32::MAX),
		(i32::MAX, i32::MIN),
		(i32::MIN, i32::MIN),
		(0, i32::MAX),
		(i32::MAX, i32::MAX),
	] {
		let mut s = HostStore::new();
		let mut d = Doc::new();
		d.init(&mut s).unwrap();
		d.body.load(&mut s, b"Some text\r", 0, 0).unwrap();
		d.page.paper_w = paper;
		d.page.paper_h = paper;
		d.page.margin_l = margin;
		d.page.margin_r = margin;
		d.page.margin_t = margin;
		d.page.margin_b = margin;
		layout_story(&d, &s, &mut GridMeasure, StoryKind::Body, |_| {});
	}
}

#[test]
fn page_setup_arithmetic_saturates() {
	let mut s = HostStore::new();
	let mut d = Doc::new();
	d.init(&mut s).unwrap();
	d.page.paper_w = i32::MIN;
	d.page.margin_l = i32::MAX;
	d.page.margin_r = i32::MAX;
	d.page.paper_h = i32::MAX;
	d.page.margin_t = i32::MIN;
	d.page.margin_b = i32::MIN;
	assert_eq!(d.page.text_w(), i32::MIN);
	assert_eq!(d.page.text_h(), i32::MAX);
}

#[test]
fn an_rtf_with_extreme_page_numbers_draws_views_and_prints_without_a_panic() {
	use quill_app::{App, Command, Key};
	use quill_chrome::view;
	use quill_grid::{Grid, GridMeasure};
	use quill_host::script::MemFiles;
	for rtf in [
		"{\\rtf1\\paperw2147483647\\paperh2147483647\\margl2147483647\\margr2147483647\\margt2147483647\\margb2147483647 Hello\\par}",
		"{\\rtf1\\paperw-2147483647\\paperh-2147483647\\margl2147483647\\margr-2147483647\\margt-2147483647\\margb2147483647 Hello\\par}",
		"{\\rtf1\\paperw0\\margl2147483647\\margr2147483647\\margt2147483647\\margb2147483647 Hello\\par}",
	] {
		let mut s = HostStore::new();
		let mut app: App<GridMeasure> = App::new();
		app.reset(&mut s).unwrap();
		let mut f = MemFiles::default();
		f.files.insert("D.RTF".into(), rtf.as_bytes().to_vec());
		if app.open(&mut s, &mut f, b"D.RTF").is_err() {
			continue;
		}
		let mut g = Grid::<25>::new(0);
		quill_app::draw(&app, &s, &mut g);
		let _ = view::text_extent(&app, 640, 0);
		app.key(&mut s, &mut f, Key::Char(b'x'));
		app.command(&mut s, &mut f, Command::Print);
		quill_app::draw(&app, &s, &mut g);
	}
}
