// The chrome's geometry: where the menu titles, the pull-down items and every dialog's controls are, the numbers a pointer hits.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod common;

use common::*;
use quill_app::{menu_items_for, Command, Key, Overlay, Wk, MENU_COUNT};
use quill_paint::dialog::{hit, widget_rect, window_rect};
use quill_paint::menu::{menu_geom, title_rects};
use quill_paint::shell::{MENU_H, SCREEN_H, SCREEN_W};

const ADV: i32 = 9;

#[test]
fn the_titles_tile_the_bar_and_fit_the_screen() {
	let r = title_rects(|_| ADV);
	for (i, t) in r.iter().enumerate() {
		assert_eq!((t.top, t.bottom), (0, MENU_H - 1));
		assert!(t.width() > 0);
		if i > 0 {
			assert_eq!(
				r[i - 1].right,
				t.left,
				"titles touch, so every x on the bar is some menu or none"
			);
		}
	}
	assert!(r[MENU_COUNT as usize - 1].right <= SCREEN_W);
}

#[test]
fn every_menu_fits_and_its_items_are_hit_by_their_own_centres() {
	let titles = title_rects(|_| ADV);
	for menu in 0..MENU_COUNT {
		let recents: Vec<&[u8]> = if menu == 0 {
			vec![&b"A:\\LONG\\NAME.QLD"[..], b"B.RTF", b"C.QLD", b"D.TXT"]
		} else {
			vec![]
		};
		let g = menu_geom(menu, true, titles[menu as usize], &recents, |_| ADV);
		assert!(
			g.rect.left >= 0 && g.rect.right <= SCREEN_W,
			"menu {menu} is inside the screen sideways"
		);
		assert!(
			g.rect.bottom <= SCREEN_H,
			"menu {menu} is inside the screen downwards"
		);
		assert_eq!(
			g.rows().len(),
			menu_items_for(menu, true).len() + recents.len()
		);
		let mut last = g.rect.top;
		for (i, row) in g.rows().iter().enumerate() {
			assert!(row.rect.top >= last, "rows do not overlap");
			last = row.rect.bottom;
			assert!(row.rect.left > g.rect.left && row.rect.right < g.rect.right);
			let (cx, cy) = (
				(row.rect.left + row.rect.right) / 2,
				(row.rect.top + row.rect.bottom) / 2,
			);
			assert_eq!(g.item_at(cx, cy), Some(i));
			assert!(g.contains(cx, cy));
		}
		assert_eq!(
			g.item_at(g.rect.left, g.rect.top),
			None,
			"the border is not an item"
		);
		assert!(last <= g.rect.bottom);
	}
}

fn dialogs() -> Vec<(&'static str, Overlay)> {
	let mut out = Vec::new();
	for (name, c) in [
		("open", Command::Open),
		("save as", Command::SaveAs),
		("find", Command::Find),
		("replace", Command::Replace),
		("fonts", Command::Fonts),
		("tabs", Command::Tabs),
		("about", Command::About),
		("picture", Command::Picture),
	] {
		let (mut app, mut s, mut f) = open_doc("plain.qld");
		app.command(&mut s, &mut f, c);
		let mut o = Overlay::default();
		assert!(app.overlay(&s, &mut o), "{name} is a dialog");
		out.push((name, o));
	}
	// The Save changes question.
	let (mut app, mut s, mut f) = open_doc("plain.qld");
	app.key(&mut s, &mut f, Key::Char(b'x'));
	app.command(&mut s, &mut f, Command::Exit);
	let mut o = Overlay::default();
	assert!(app.overlay(&s, &mut o));
	out.push(("confirm", o));
	out
}

#[test]
fn every_dialog_fits_and_its_controls_are_hit_by_their_own_centres() {
	for (name, o) in dialogs() {
		let win = window_rect(&o);
		assert!(
			win.left >= 0 && win.top >= 0 && win.right <= SCREEN_W && win.bottom <= SCREEN_H,
			"{name} is on the screen"
		);
		let mut focused = 0;
		for (i, w) in o.widgets().iter().enumerate() {
			let r = widget_rect(&o, w);
			assert!(
				r.left >= win.left && r.right <= win.right,
				"{name}: widget {i} is inside the window sideways"
			);
			assert!(
				r.top >= win.top && r.bottom <= win.bottom,
				"{name}: widget {i} is inside the window downwards"
			);
			if w.focus {
				focused += 1;
			}
			if w.kind != Wk::Label {
				let (cx, cy) = ((r.left + r.right) / 2, (r.top + r.bottom) / 2);
				assert_eq!(hit(&o, cx, cy), Some(i), "{name}: widget {i}");
			}
		}
		assert!(focused <= 1, "{name}: one visible focus, not {focused}");
		assert_eq!(hit(&o, 0, 0), None);
	}
}

#[test]
fn the_save_changes_question_has_exactly_one_focused_button_and_it_follows_tab() {
	for n in 0..4 {
		let (mut app, mut s, mut f) = open_doc("plain.qld");
		app.key(&mut s, &mut f, Key::Char(b'x'));
		app.command(&mut s, &mut f, Command::Exit);
		for _ in 0..n {
			app.key(&mut s, &mut f, Key::Tab);
		}
		let mut o = Overlay::default();
		assert!(app.overlay(&s, &mut o));
		let buttons: Vec<_> = o
			.widgets()
			.iter()
			.filter(|w| w.kind == Wk::Button)
			.collect();
		assert_eq!(buttons.len(), 3);
		let on: Vec<usize> = buttons
			.iter()
			.enumerate()
			.filter(|(_, b)| b.focus)
			.map(|(i, _)| i)
			.collect();
		assert_eq!(on, vec![n % 3]);
		assert!(
			buttons.iter().all(|b| b.default == b.focus),
			"the heavy border follows the focus"
		);
	}
}

#[test]
fn menu_marks_follow_the_document() {
	let (mut app, mut s, mut f) = open_doc("plain.qld");
	let m = |app: &TApp, s: &quill_host::HostStore, c| app.menu_mark(s, c);
	assert!(
		m(&app, &s, Command::Cut).disabled && m(&app, &s, Command::Copy).disabled,
		"nothing selected"
	);
	assert!(!m(&app, &s, Command::Bold).checked);
	assert!(m(&app, &s, Command::Chrome(1)).checked, "the toolbar shows");
	let aligned = (0..4)
		.filter(|a| m(&app, &s, Command::Align(*a)).checked)
		.count();
	assert_eq!(aligned, 1, "one alignment is checked");
	app.command(&mut s, &mut f, Command::SelectAll);
	assert!(!m(&app, &s, Command::Cut).disabled);
	app.command(&mut s, &mut f, Command::Bold);
	assert!(m(&app, &s, Command::Bold).checked);
	app.command(&mut s, &mut f, Command::Chrome(1));
	assert!(!m(&app, &s, Command::Chrome(1)).checked);
}
