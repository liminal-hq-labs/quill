// The "before" measurements: real `App` runs with a counting measure, for the m2-letter script and for typing into a document of
// about 1 MB, in the text screen's paint and in the page view's paint (as the Windows 95 shell calls it).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::time::Instant;

use quill_app::{App, Key};
use quill_core::layout::Measure;
use quill_grid::{Grid, GridMeasure};
use quill_host::script::{parse_script, MemFiles};
use quill_host::HostStore;

use crate::corpus;
use crate::counting::{Counting, Screen96, Shared, Tally};

/// One key (or one paint) of a run.
#[derive(Clone, Debug, Default)]
pub struct Row {
	/// What was done.
	pub what: String,
	/// `layout_story` calls the key made.
	pub key_passes: u64,
	/// `layout_story` calls the paint made.
	pub paint_passes: u64,
	/// Paragraphs in the document then.
	pub paras: u32,
	/// Microseconds the key took.
	pub key_us: f64,
	/// Microseconds the paint took.
	pub paint_us: f64,
}

fn paras_of<M: Measure + Clone>(app: &App<M>, s: &HostStore) -> u32 {
	let t = &app.doc().body;
	let mut buf = vec![0u8; t.len() as usize];
	let n = t.read(s, 0, &mut buf);
	buf[..n].iter().filter(|b| **b == 0x0D).count() as u32
}

/// Plays tests/scenarios/m2-letter.keys on the text screen, one row per key.
pub fn letter_script(trace: bool) -> (Vec<Row>, Tally) {
	let text =
		std::fs::read_to_string(corpus::root().join("tests/scenarios/m2-letter.keys")).unwrap();
	let sc = parse_script(&text);
	let mut files = MemFiles::default();
	for name in &sc.copy {
		let bytes = std::fs::read(corpus::root().join("tests/docs").join(name)).unwrap();
		files.files.insert(name.to_ascii_uppercase(), bytes);
	}
	let mut s = HostStore::new();
	let (m, tally) = Counting::new(GridMeasure, trace);
	let mut app = App::with_measure(m);
	app.reset(&mut s).unwrap();
	app.load_settings(&mut files);
	app.set_about("Spike");
	app.set_rows(23);
	if let Some(name) = &sc.open {
		app.open(&mut s, &mut files, name.as_bytes()).unwrap();
	}
	let mut g = Grid::<25>::new(0);
	let mut rows = Vec::new();
	let mut last = tally.borrow().layout_story;
	quill_app::draw(&app, &s, &mut g);
	for k in &sc.keys {
		let t0 = Instant::now();
		app.key(&mut s, &mut files, *k);
		let key_us = t0.elapsed().as_secs_f64() * 1e6;
		let after_key = tally.borrow().layout_story;
		let t1 = Instant::now();
		quill_app::draw(&app, &s, &mut g);
		let paint_us = t1.elapsed().as_secs_f64() * 1e6;
		let after_paint = tally.borrow().layout_story;
		rows.push(Row {
			what: format!("{k:?}"),
			key_passes: after_key - last,
			paint_passes: after_paint - after_key,
			paras: paras_of(&app, &s),
			key_us,
			paint_us,
		});
		last = after_paint;
		if app.quit() {
			break;
		}
	}
	let t = tally.borrow().clone();
	(rows, t)
}

/// Which paint to do after a key.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Paint {
	/// `quill_app::draw` into the 80 by 25 grid (DOS, and the text view).
	Text,
	/// What the Windows 95 shell does after a key and in `WM_PAINT` (see `typing`).
	Page,
}

/// Opens `name` (bytes `bytes`) in an app on measure `m`, puts the caret at the middle line, and types `keys` characters, one row
/// per key. The paint is `paint`.
pub fn typing<M: Measure + Clone>(
	m: M,
	tally: &Shared,
	name: &str,
	bytes: Vec<u8>,
	keys: usize,
	paint: Paint,
) -> Vec<Row> {
	let mut files = MemFiles::default();
	files.files.insert(name.to_string(), bytes);
	let mut s = HostStore::new();
	let mut app = App::with_measure(m);
	app.reset(&mut s).unwrap();
	app.set_rows(23);
	app.set_view_height(600);
	app.open(&mut s, &mut files, name.as_bytes()).unwrap();
	let mid = app.line_total(&s) / 2;
	app.click(&s, mid, 0, false);
	let mut g = Grid::<25>::new(0);
	let mut rows = Vec::new();
	let mut last = tally.borrow().layout_story;
	for i in 0..keys {
		let c = b'a' + (i % 26) as u8;
		let t0 = Instant::now();
		app.key(&mut s, &mut files, Key::Char(c));
		if paint == Paint::Page {
			// ensure_visible: where the caret is, to scroll it into view.
			let _ = app.point_of(&s, app.caret(), 1);
		}
		let key_us = t0.elapsed().as_secs_f64() * 1e6;
		let after_key = tally.borrow().layout_story;
		let t1 = Instant::now();
		quill_app::draw(&app, &s, &mut g);
		if paint == Paint::Page {
			// paint_into: body_pages, paint_page's line loop and the caret (win9x gdi.rs).
			let mut pages = 1;
			app.for_each_line(&s, |_, l| pages = pages.max(l.page));
			app.for_each_line(&s, |_, _| {});
			let _ = app.caret_xy(&s);
		}
		let paint_us = t1.elapsed().as_secs_f64() * 1e6;
		let after_paint = tally.borrow().layout_story;
		rows.push(Row {
			what: format!("type {}", c as char),
			key_passes: after_key - last,
			paint_passes: after_paint - after_key,
			paras: paras_of(&app, &s),
			key_us,
			paint_us,
		});
		last = after_paint;
	}
	rows
}

/// The numbers a table needs from rows.
pub fn summarise(rows: &[Row]) -> (f64, f64, f64, f64, u32) {
	let n = rows.len().max(1) as f64;
	let kp = rows.iter().map(|r| r.key_passes as f64).sum::<f64>() / n;
	let pp = rows.iter().map(|r| r.paint_passes as f64).sum::<f64>() / n;
	let ku = rows.iter().map(|r| r.key_us).sum::<f64>() / n;
	let pu = rows.iter().map(|r| r.paint_us).sum::<f64>() / n;
	let paras = rows.last().map_or(0, |r| r.paras);
	(kp, pp, ku, pu, paras)
}

/// A page-view measure with counting.
pub fn counting_screen(trace: bool) -> (Counting<Screen96>, Shared) {
	Counting::new(Screen96, trace)
}
