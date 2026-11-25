// The pointer on the graphics screen: a click at a pixel puts the caret where the page was painted under it, and a drag selects between
// two pixels.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod common;

use common::*;
use proptest::prelude::*;
use quill_app::mouse::{Ctx, Pointer};
use quill_app::view::{page_width, page_x0, PageGeom};
use quill_core::layout::to_units;
use quill_mouse::Ev;
use quill_paint::layout::Layout;
use quill_paint::shell::{self, hscroll, pos_at_pixel, Pixels};

fn rig(name: &str) -> (TApp, quill_host::HostStore, quill_host::script::MemFiles) {
	let (mut app, s, f) = open_doc(name);
	shell::sync_view(&mut app);
	(app, s, f)
}

/// The page area as the app's chrome flags lay it out.
fn page_area(app: &TApp) -> quill_paint::Rect {
	Layout::of(app).page
}

fn press(
	p: &mut Pointer,
	app: &mut TApp,
	s: &mut quill_host::HostStore,
	f: &mut quill_host::script::MemFiles,
	evs: &[Ev],
	tick: u32,
) {
	for e in evs {
		let space = Pixels::of(app);
		p.handle(app, s, f, &space, *e, Ctx { shift: false, tick });
	}
}

/// Where the painter puts the caret at the position, in screen pixels: its x and the middle of its line.
fn caret_pixel(app: &TApp, s: &quill_host::HostStore) -> Option<(i32, i32)> {
	let (_, l, x) = app.caret_xy(s)?;
	let g = PageGeom::of(app, s);
	let area = page_area(app);
	let x0 = page_x0(area.width(), page_width(app), hscroll(app, area.width()));
	let tx = area.left + x0 + to_units(app.doc().page.margin_l, 96) + x;
	Some((tx, g.line_y(&l, area.top) + (l.ascent + l.descent) / 2))
}

#[test]
fn clicking_where_the_caret_is_painted_gives_the_position_back() {
	let (mut app, mut s, mut f) = rig("letter.rtf");
	let mut ptr = Pointer::default();
	let area = page_area(&app);
	let len = app.doc().body.len().saturating_sub(1);
	let mut tested = 0;
	for pos in 0..len {
		app.place_caret(&s, pos, false);
		let Some((x, y)) = caret_pixel(&app, &s) else {
			continue;
		};
		if !(area.left..area.right).contains(&x)
			|| !(area.top..area.bottom).contains(&y)
		{
			continue;
		}
		let (_, line, _) = app.caret_xy(&s).unwrap();
		let wrapped_end = pos == line.start + line.len
			&& !matches!(app.doc().body.read_byte(&s, pos), Some(0x0D | 0x0B) | None);
		let mut zero_before = false;
		app.walk_chars(&s, &line, |p, _, _, w, _, _| {
			zero_before |= p + 1 == pos && w == 0
		});
		if wrapped_end || zero_before {
			continue;
		}
		let want = pos;
		app.place_caret(&s, 0, false);
		press(
			&mut ptr,
			&mut app,
			&mut s,
			&mut f,
			&[Ev::Down { x, y }, Ev::Up { x, y }],
			1000 * pos,
		);
		assert_eq!(app.caret(), want, "click at ({x},{y})");
		tested += 1;
	}
	assert!(tested > 100, "only {tested} positions were on the screen");
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(300))]

	// A press anywhere in the page area puts the caret where `pos_at_pixel` says, and one anywhere else (the bars) leaves it.
	#[test]
	fn a_click_is_the_position_under_the_pixel(x in 0i32..640, y in 0i32..480) {
		let (mut app, mut s, mut f) = rig("letter.rtf");
		let mut ptr = Pointer::default();
		app.place_caret(&s, 3, false);
		let area = page_area(&app);
		let inside = (area.left..area.right).contains(&x) && (area.top..area.bottom).contains(&y);
		let hit = pos_at_pixel(&app, &s, x, y);
		press(&mut ptr, &mut app, &mut s, &mut f, &[Ev::Down { x, y }, Ev::Up { x, y }], 0);
		if !inside {
			prop_assert_eq!(app.caret(), 3);
		} else if hit.margin {
			// A click left of the text takes a line.
			prop_assert!(app.selection().is_some() || app.doc().body.len() <= 1);
		} else {
			prop_assert_eq!(app.caret(), hit.pos);
			prop_assert!(app.selection().is_none());
		}
		prop_assert!(app.caret() < app.doc().body.len());
	}

	// A drag between two pixels in the text selects the characters between them, whichever way it goes.
	#[test]
	fn a_drag_selects_between_the_pixels(a in (30i32..600, 50i32..440), b in (30i32..600, 50i32..440)) {
		let (mut app, mut s, mut f) = rig("letter.rtf");
		let mut ptr = Pointer::default();
		let (ha, hb) = (pos_at_pixel(&app, &s, a.0, a.1), pos_at_pixel(&app, &s, b.0, b.1));
		prop_assume!(!ha.margin && !hb.margin);
		press(&mut ptr, &mut app, &mut s, &mut f, &[
			Ev::Down { x: a.0, y: a.1 },
			Ev::Move { x: b.0, y: b.1 },
			Ev::Up { x: b.0, y: b.1 },
		], 0);
		prop_assert_eq!(app.caret(), hb.pos);
		if ha.pos == hb.pos {
			prop_assert!(app.selection().is_none());
		} else {
			prop_assert_eq!(app.selection(), Some((ha.pos.min(hb.pos), ha.pos.max(hb.pos))));
		}
	}

	// Two presses within the double-click time and place are a double click and a third a triple; a press too late, or too far, is a click.
	#[test]
	fn clicks_count_by_time_and_place(x in 30i32..600, y in 50i32..300, late in 10u32..200, far in 5i32..60) {
		let (mut app, mut s, mut f) = rig("letter.rtf");
		let mut ptr = Pointer::default();
		let hit = pos_at_pixel(&app, &s, x, y);
		prop_assume!(!hit.margin);
		let last = app.doc().body.len() - 1;
		let clip = |r: (u32, u32)| (r.0.min(last), r.1.min(last));
		let sel = |app: &TApp| app.selection().unwrap_or((app.caret(), app.caret()));
		let word = clip(app.word_range(&s, hit.pos));
		let para = clip(app.paragraph_range(&s, hit.pos));
		let click = |ptr: &mut Pointer, app: &mut TApp, s: &mut _, f: &mut _, x: i32, y: i32, t: u32| {
			press(ptr, app, s, f, &[Ev::Down { x, y }, Ev::Up { x, y }], t);
		};
		click(&mut ptr, &mut app, &mut s, &mut f, x, y, 0);
		click(&mut ptr, &mut app, &mut s, &mut f, x, y, 4);
		prop_assert_eq!(sel(&app), if word.0 == word.1 { (word.0, word.0) } else { word });
		click(&mut ptr, &mut app, &mut s, &mut f, x, y, 8);
		prop_assert_eq!(sel(&app), if para.0 == para.1 { (para.0, para.0) } else { para });
		// Too late: a click again, which drops the selection.
		click(&mut ptr, &mut app, &mut s, &mut f, x, y, 8 + late);
		prop_assert!(app.selection().is_none());
		// Too far from the last press: a click again.
		click(&mut ptr, &mut app, &mut s, &mut f, x, y, 1000);
		click(&mut ptr, &mut app, &mut s, &mut f, x + far, y, 1002);
		prop_assert!(app.selection().is_none());
	}
}
