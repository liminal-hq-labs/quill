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

use quill_app::{CHROME_FORMAT, CHROME_RULER, CHROME_STATUS, CHROME_TOOLBAR};
use quill_chrome::view::TOOLBAR;
use quill_paint::layout::{Layout, BAR};
use quill_paint::scroll::{self, Part};
use quill_paint::toolbar::{box_at, box_rect, button_at, button_rect, Box3};

fn overlaps(a: quill_paint::Rect, b: quill_paint::Rect) -> bool {
	!a.intersect(b).is_empty()
}

fn parts_of(l: &Layout) -> Vec<quill_paint::Rect> {
	let mut v = vec![l.menu, l.page, l.vbar];
	v.extend(l.tool);
	v.extend(l.format);
	v.extend(l.ruler);
	v.extend(l.hbar);
	v.extend(l.corner);
	v.extend(l.status);
	v
}

#[test]
fn the_parts_of_the_screen_never_overlap_and_cover_all_of_it_for_every_choice_of_bars() {
	for bits in 0..16u8 {
		let (mut app, mut s, mut f) = open_doc("letter.rtf");
		let _ = (&mut s, &mut f);
		for bit in [CHROME_TOOLBAR, CHROME_RULER, CHROME_STATUS, CHROME_FORMAT] {
			if bits & bit == 0 {
				app.toggle_chrome(bit);
			}
		}
		let l = Layout::of(&app);
		let parts = parts_of(&l);
		let mut area = 0;
		for (i, a) in parts.iter().enumerate() {
			assert!(
				a.left >= 0 && a.top >= 0 && a.right <= SCREEN_W && a.bottom <= SCREEN_H,
				"bits {bits}: part {i} is on the screen"
			);
			area += a.width() * a.height();
			for b in &parts[i + 1..] {
				assert!(!overlaps(*a, *b), "bits {bits}: {a:?} and {b:?} overlap");
			}
		}
		assert_eq!(
			area,
			SCREEN_W * SCREEN_H,
			"bits {bits}: the parts tile the screen"
		);
		assert_eq!(l.tool.is_some(), bits & CHROME_TOOLBAR != 0);
		assert_eq!(l.format.is_some(), bits & CHROME_FORMAT != 0);
		assert_eq!(l.ruler.is_some(), bits & CHROME_RULER != 0);
		assert_eq!(l.status.is_some(), bits & CHROME_STATUS != 0);
		assert_eq!(l.vbar.width(), BAR);
	}
}

#[test]
fn quill_ini_chooses_the_bars() {
	let (mut app, _s, mut f) = new_app();
	f.files.insert(
		"QUILL.INI".into(),
		b"[Quill]\r\nToolbar=0\r\nFormattingBar=0\r\nRuler=1\r\nStatusBar=0\r\n".to_vec(),
	);
	app.load_settings(&mut f);
	let l = Layout::of(&app);
	assert!(l.tool.is_none() && l.format.is_none() && l.status.is_none());
	assert!(l.ruler.is_some());
	// The page takes the rows the bars gave up: below the menu and the ruler, down to the horizontal bar.
	assert_eq!(l.page.top, MENU_H + 22);
	assert_eq!(l.page.bottom, SCREEN_H - BAR);
}

#[test]
fn every_toolbar_button_and_box_is_hit_by_its_own_centre() {
	let (app, _s, _f) = open_doc("letter.rtf");
	let l = Layout::of(&app);
	for i in 0..TOOLBAR.buttons.len() {
		let r = button_rect(&l, i).expect("both rows show");
		assert_eq!(r.width(), 24);
		assert!(r.right <= SCREEN_W - BAR + 16);
		assert_eq!(
			button_at(&l, (r.left + r.right) / 2, (r.top + r.bottom) / 2),
			Some(i)
		);
	}
	for b in [Box3::Style, Box3::Face, Box3::Size] {
		let r = box_rect(&l, b).unwrap();
		assert_eq!(
			box_at(&l, (r.left + r.right) / 2, (r.top + r.bottom) / 2),
			Some(b)
		);
	}
	assert_eq!(button_at(&l, 600, l.tool.unwrap().top + 10), None);
}

#[test]
fn a_hidden_row_has_no_buttons() {
	let (mut app, _s, _f) = open_doc("letter.rtf");
	app.toggle_chrome(CHROME_TOOLBAR);
	let l = Layout::of(&app);
	let first_row = TOOLBAR.buttons.iter().position(|b| b.row == 0).unwrap();
	assert!(button_rect(&l, first_row).is_none());
	assert!(
		box_rect(&l, Box3::Face).is_some(),
		"the formatting bar still shows"
	);
}

#[test]
fn a_scroll_bar_has_its_parts_in_order_and_the_thumb_follows_the_position() {
	let bar = quill_paint::Rect::new(624, 98, 640, 444);
	let mut last_top = 0;
	for pos in [0, 10, 50, 100] {
		let p = scroll::parts(bar, true, pos, 200, 100);
		assert_eq!(p.up.bottom, p.track.top);
		assert_eq!(p.track.bottom, p.down.top);
		assert!(p.thumb.top >= p.track.top && p.thumb.bottom <= p.track.bottom);
		assert!(
			p.thumb.top >= last_top,
			"the thumb moves down as the position grows"
		);
		last_top = p.thumb.top;
		assert_eq!(
			scroll::hit(
				&p,
				(p.thumb.left + p.thumb.right) / 2,
				(p.thumb.top + p.thumb.bottom) / 2
			),
			Some(Part::Thumb)
		);
		assert_eq!(
			scroll::pos_at(&p, p.thumb.top, 100),
			pos,
			"dragging the thumb back gives the position"
		);
	}
	let p = scroll::parts(bar, true, 0, 200, 100);
	assert_eq!(scroll::hit(&p, 630, p.up.top + 2), Some(Part::LineUp));
	assert_eq!(
		scroll::hit(&p, 630, p.down.bottom - 2),
		Some(Part::LineDown)
	);
	assert_eq!(
		scroll::hit(&p, 630, p.thumb.bottom + 3),
		Some(Part::PageDown)
	);
	let p = scroll::parts(bar, true, 100, 200, 100);
	assert_eq!(scroll::hit(&p, 630, p.thumb.top - 3), Some(Part::PageUp));
	assert_eq!(scroll::hit(&p, 100, 100), None);
	// Nothing to scroll: the thumb is the whole track and there is nowhere to drag.
	let p = scroll::parts(bar, true, 0, 50, 50);
	assert_eq!(p.travel, 0);
	assert_eq!(scroll::pos_at(&p, 300, 0), 0);
	// A horizontal bar is the same with the axes swapped.
	let h = scroll::parts(
		quill_paint::Rect::new(0, 444, 624, 460),
		false,
		100,
		400,
		200,
	);
	assert_eq!(h.up.right, h.track.left);
	assert!(h.thumb.left > h.track.left && h.thumb.right < h.track.right);
}
