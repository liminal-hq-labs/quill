// The mouse on the text screen: clicks, drags, double and triple clicks, the wheel, and the menus and dialogs by cell.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use proptest::prelude::*;
use quill_app::mouse::{Ctx, Pointer, TextCells};
use quill_app::{App, Command, Key};
use quill_grid::Grid;
use quill_host::script::MemFiles;
use quill_host::HostStore;
use quill_mouse::fake::FakeInt33;
use quill_mouse::{Ev, Mouse};

type Rig = (App, HostStore, MemFiles, Pointer);

const TEXT: TextCells = TextCells { rows: 23 };

fn setup(text: &str) -> Rig {
	let mut s = HostStore::new();
	let mut app = App::new();
	app.reset(&mut s).unwrap();
	let mut f = MemFiles::default();
	app.set_rows(23);
	for b in text.bytes() {
		match b {
			b'\n' => app.key(&mut s, &mut f, Key::Enter),
			_ => app.key(&mut s, &mut f, Key::Char(b)),
		}
	}
	(app, s, f, Pointer::default())
}

fn feed(r: &mut Rig, ev: Ev, ctx: Ctx) {
	r.3.handle(&mut r.0, &mut r.1, &mut r.2, &TEXT, ev, ctx);
}

fn click(r: &mut Rig, x: i32, y: i32, tick: u32) {
	let ctx = Ctx { shift: false, tick };
	feed(r, Ev::Down { x, y }, ctx);
	feed(r, Ev::Up { x, y }, ctx);
}

fn prose(words: &[(u8, bool)]) -> String {
	let mut out = String::new();
	for (n, para) in words {
		for _ in 0..=(*n % 9) {
			out.push('w');
		}
		out.push(' ');
		if *para && *n % 5 == 0 {
			out.push('\n');
		}
	}
	out
}

fn screen(r: &Rig) -> String {
	let mut g = Grid::<25>::new(0);
	quill_app::draw(&r.0, &r.1, &mut g);
	let mut out = String::new();
	g.dump(1, &mut out).unwrap();
	out
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(200))]

	// A click anywhere on the page puts the caret where `pos_at_cell` says, drops the selection, and never goes past the document.
	#[test]
	fn a_click_places_the_caret_at_the_cell(words in prop::collection::vec((any::<u8>(), any::<bool>()), 1..80), col in 10i32..70, row in 1i32..24) {
		let mut r = setup(&prose(&words));
		let want = r.0.pos_at_cell(&r.1, col, row);
		click(&mut r, col, row, 0);
		prop_assert_eq!(r.0.caret(), want);
		prop_assert!(r.0.selection().is_none());
		prop_assert!(want < r.0.doc().body.len());
	}

	// The caret `draw` shows is where a click puts it: every position of the document, drawn and clicked.
	#[test]
	fn clicking_the_drawn_caret_gives_the_position_back(words in prop::collection::vec((any::<u8>(), any::<bool>()), 1..40), pick in any::<u16>()) {
		let mut r = setup(&prose(&words));
		let len = r.0.doc().body.len();
		let pos = u32::from(pick) % len.max(1);
		r.0.place_caret(&r.1, pos, false);
		let mut g = Grid::<25>::new(0);
		let Some((col, row)) = quill_app::draw(&r.0, &r.1, &mut g) else { return Ok(()) };
		let (_, line, _) = r.0.caret_xy(&r.1).unwrap();
		// The end of a wrapped line is the start of the next, and a click there stops short of it (as `pos_at_point` says).
		let wrapped_end = pos == line.start + line.len && !matches!(r.0.doc().body.read_byte(&r.1, pos), Some(0x0D | 0x0B) | None);
		prop_assume!(!wrapped_end);
		click(&mut r, col as i32, row as i32, 0);
		prop_assert_eq!(r.0.caret(), pos, "cell {},{}", col, row);
	}

	// A drag from one cell to another selects between the two positions, whichever way it goes, with the caret at the end of it.
	#[test]
	fn a_drag_selects_between_its_ends(words in prop::collection::vec((any::<u8>(), any::<bool>()), 1..80), a in (10i32..70, 1i32..24), b in (10i32..70, 1i32..24)) {
		let mut r = setup(&prose(&words));
		let (pa, pb) = (r.0.pos_at_cell(&r.1, a.0, a.1), r.0.pos_at_cell(&r.1, b.0, b.1));
		let ctx = Ctx { shift: false, tick: 0 };
		feed(&mut r, Ev::Down { x: a.0, y: a.1 }, ctx);
		feed(&mut r, Ev::Move { x: b.0, y: b.1 }, ctx);
		feed(&mut r, Ev::Up { x: b.0, y: b.1 }, ctx);
		prop_assert_eq!(r.0.caret(), pb);
		if pa == pb {
			prop_assert!(r.0.selection().is_none());
		} else {
			prop_assert_eq!(r.0.selection(), Some((pa.min(pb), pa.max(pb))));
		}
		// Moving after the button went up changes nothing.
		feed(&mut r, Ev::Move { x: 0, y: 1 }, ctx);
		prop_assert_eq!(r.0.caret(), pb);
	}

	// Double click takes the word under the pointer, triple the paragraph; a second click elsewhere or too late is a new click.
	#[test]
	fn double_and_triple_clicks_take_a_word_and_a_paragraph(words in prop::collection::vec((any::<u8>(), any::<bool>()), 1..60), col in 10i32..70, row in 1i32..24) {
		let mut r = setup(&prose(&words));
		let pos = r.0.pos_at_cell(&r.1, col, row);
		let word = r.0.word_range(&r.1, pos);
		let para = r.0.paragraph_range(&r.1, pos);
		click(&mut r, col, row, 0);
		click(&mut r, col, row, 3);
		let sel = |app: &App| app.selection().unwrap_or((app.caret(), app.caret()));
		if col >= 10 {
			prop_assert_eq!(sel(&r.0), if word.0 == word.1 { (word.0, word.0) } else { word });
		}
		click(&mut r, col, row, 6);
		let last = r.0.doc().body.len() - 1;
		let para = (para.0.min(last), para.1.min(last));
		prop_assert_eq!(sel(&r.0), para);
		// Nine ticks later it is a new click: a caret again.
		click(&mut r, col, row, 40);
		prop_assert!(r.0.selection().is_none());
	}
}

#[test]
fn the_wheel_scrolls_three_lines_a_notch_and_stops_at_the_ends() {
	let text: String = (0..60).map(|i| format!("line {i}\n")).collect();
	let mut r = setup(&text);
	let ctx = Ctx::default();
	feed(&mut r, Ev::Wheel(0), ctx);
	r.0.scroll_to(&r.1, 0);
	assert_eq!(r.0.top_line(), 0);
	feed(&mut r, Ev::Wheel(2), ctx);
	assert_eq!(r.0.top_line(), 6);
	feed(&mut r, Ev::Wheel(-1), ctx);
	assert_eq!(r.0.top_line(), 3);
	feed(&mut r, Ev::Wheel(-100), ctx);
	assert_eq!(r.0.top_line(), 0);
	feed(&mut r, Ev::Wheel(10_000), ctx);
	assert_eq!(r.0.top_line(), r.0.line_total(&r.1) - 1);
}

#[test]
fn shift_click_extends_the_selection_from_the_caret() {
	let mut r = setup("one two three four five six\nseven eight nine ten");
	click(&mut r, 12, 1, 0);
	let from = r.0.caret();
	feed(
		&mut r,
		Ev::Down { x: 30, y: 1 },
		Ctx {
			shift: true,
			tick: 100,
		},
	);
	feed(
		&mut r,
		Ev::Up { x: 30, y: 1 },
		Ctx {
			shift: true,
			tick: 100,
		},
	);
	let to = r.0.caret();
	assert_eq!(r.0.selection(), Some((from.min(to), from.max(to))));
	assert!(to > from);
}

#[test]
fn a_click_in_the_left_margin_selects_the_line() {
	let mut r = setup("first line of the text\nsecond line");
	click(&mut r, 2, 1, 0);
	let (lo, hi) = r.0.selection().expect("a line is selected");
	assert_eq!(lo, 0);
	assert!(hi >= 22);
}

#[test]
fn clicks_outside_the_page_leave_the_caret_alone() {
	let mut r = setup("some text");
	let at = r.0.caret();
	click(&mut r, 5, 24, 0);
	click(&mut r, 5, 30, 0);
	assert_eq!(r.0.caret(), at);
}

#[test]
fn the_driver_in_text_mode_clicks_the_cell_under_it() {
	let mut r = setup("alpha beta gamma delta epsilon zeta eta theta");
	let mut m = Mouse::open(FakeInt33::default(), 80, 25, (8, 8)).unwrap();
	let want = r.0.pos_at_cell(&r.1, 30, 1);
	// Anywhere inside the cell (eight driver units wide and high) is the cell.
	m.driver().move_to(30 * 8 + 5, 8 + 7);
	m.driver().press();
	m.driver().release();
	let mut evs = Vec::new();
	m.poll(|e| evs.push(e));
	for e in evs {
		feed(&mut r, e, Ctx::default());
	}
	assert_eq!(r.0.caret(), want);
}

// ---- menus and dialogs by cell ------------------------------------------------------------------------------------------------------

#[test]
fn the_menu_bar_opens_a_menu_and_an_item_runs() {
	let mut r = setup("text");
	// File is at column 1 of the bar; New is its first item, on row 2; the second click on the title closes it.
	click(&mut r, 2, 0, 0);
	assert_eq!(r.0.menu_open(), Some((0, 0)));
	click(&mut r, 2, 0, 50);
	assert_eq!(r.0.menu_open(), None);
	click(&mut r, 2, 0, 100);
	// Edit is the next title; a click on it switches menus.
	click(&mut r, 8, 0, 150);
	assert_eq!(r.0.menu_open(), Some((1, 0)));
	// A click outside the box closes the menu without running anything.
	click(&mut r, 70, 20, 200);
	assert_eq!(r.0.menu_open(), None);
	assert!(r.0.editing());
}

#[test]
fn hovering_moves_the_highlight_of_an_open_menu() {
	let mut r = setup("text");
	click(&mut r, 2, 0, 0);
	feed(&mut r, Ev::Move { x: 5, y: 5 }, Ctx::default());
	let (_, item) = r.0.menu_open().unwrap();
	assert!(item > 0);
}

#[test]
fn every_item_of_every_menu_runs_the_command_the_keys_run() {
	// Clicking item i of menu m is the same as Alt-opening the menu, stepping to item i and pressing Enter.
	for menu in 0..quill_app::MENU_COUNT {
		let items = quill_app::menu_items_for(menu, true);
		if items.is_empty() {
			continue;
		}
		let (col, _, _) = quill_app::menu_box(menu, true, 0);
		for i in 0..items.len() {
			if matches!(items[i].command, Command::Exit) {
				continue;
			}
			let mut by_mouse = setup("hello world\nsecond");
			let mut by_keys = setup("hello world\nsecond");
			by_keys.0.key(&mut by_keys.1, &mut by_keys.2, Key::F(10));
			for _ in 0..menu {
				by_keys.0.key(&mut by_keys.1, &mut by_keys.2, Key::Right);
			}
			by_mouse.3 = Pointer::default();
			// Open by mouse: the title's column, then the item's row (rules take a row of their own).
			let at = [1usize, 7, 13, 21, 27, 38, 49, 59, 65][usize::from(menu)] as i32;
			click(&mut by_mouse, at + 1, 0, 0);
			let mut row = 2;
			for (k, it) in items.iter().enumerate() {
				if it.rule {
					row += 1;
				}
				if k == i {
					break;
				}
				row += 1;
			}
			for _ in 0..i {
				by_keys.0.key(&mut by_keys.1, &mut by_keys.2, Key::Down);
			}
			by_keys.0.key(&mut by_keys.1, &mut by_keys.2, Key::Enter);
			click(&mut by_mouse, col as i32 + 3, row, 60);
			assert_eq!(
				screen(&by_mouse),
				screen(&by_keys),
				"menu {menu} item {i} ({})",
				items[i].label
			);
		}
	}
}

fn grid_of(r: &Rig) -> Grid<25> {
	let mut g = Grid::<25>::new(0);
	quill_app::draw(&r.0, &r.1, &mut g);
	g
}

/// The cells of every `< Name >` button the dialog drew, as (column, row, name).
fn buttons(g: &Grid<25>) -> Vec<(i32, i32, String)> {
	let mut out = Vec::new();
	for y in 8..25 {
		let row: Vec<u8> = (0..80).map(|x| g.get(x, y).unwrap().ch).collect();
		let mut x = 0;
		while x < 80 {
			if row[x] == b'<' && row.get(x + 1) == Some(&b' ') {
				if let Some(end) = (x + 2..80.min(x + 20)).find(|e| row[*e] == b'>') {
					let name = String::from_utf8_lossy(&row[x + 2..end]).trim().to_string();
					if !name.is_empty() {
						out.push((x as i32, y as i32, name));
					}
					x = end;
				}
			}
			x += 1;
		}
	}
	out
}

#[test]
fn clicking_a_dialog_button_does_what_its_keys_do() {
	// Each dialog is drawn, its buttons are found on the grid, and a click on each is compared with the keys that press it.
	let open = |r: &mut Rig, keys: &[Key]| {
		for k in keys {
			r.0.key(&mut r.1, &mut r.2, *k);
		}
	};
	// File > Open, and the Find dialog (Find > Find) and the Save changes question are the three with buttons the text mode draws.
	type Press<'a> = (&'a str, Vec<Key>);
	let cases: [(&str, Vec<Key>, Vec<Press>); 3] = [
		(
			"open",
			vec![Key::F(10), Key::Down, Key::Enter],
			vec![("OK", vec![Key::Enter]), ("Cancel", vec![Key::Esc])],
		),
		(
			"find",
			vec![Key::F(10), Key::Right, Key::Right, Key::Right, Key::Enter],
			vec![
				("Find Next", vec![Key::Tab, Key::Tab, Key::Tab, Key::Enter]),
				(
					"Close",
					vec![Key::Tab, Key::Tab, Key::Tab, Key::Tab, Key::Enter],
				),
			],
		),
		(
			"confirm",
			vec![Key::Char(b'x'), Key::F(10), Key::Enter],
			vec![
				("Yes", vec![Key::Char(b'y')]),
				("No", vec![Key::Char(b'n')]),
				("Cancel", vec![Key::Esc]),
			],
		),
	];
	for (name, setup_keys, presses) in cases {
		for (label, keys) in presses {
			let mut by_mouse = setup("");
			let mut by_keys = setup("");
			open(&mut by_mouse, &setup_keys);
			open(&mut by_keys, &setup_keys);
			let g = grid_of(&by_mouse);
			let Some((x, y, _)) = buttons(&g).into_iter().find(|(_, _, n)| n == label) else {
				panic!("{name}: no {label} button drawn");
			};
			click(&mut by_mouse, x + 2, y, 0);
			open(&mut by_keys, &keys);
			assert_eq!(screen(&by_mouse), screen(&by_keys), "{name}: {label}");
		}
	}
}

fn cell_text(g: &Grid<25>, x: usize, y: usize, n: usize) -> String {
	(x..x + n)
		.map(|c| g.get(c, y).unwrap().ch as char)
		.collect()
}

#[test]
fn clicking_a_check_box_or_its_label_toggles_it_and_a_field_takes_the_focus() {
	let mut r = setup("");
	for k in [Key::F(10), Key::Right, Key::Right, Key::Right, Key::Enter] {
		r.0.key(&mut r.1, &mut r.2, k);
	}
	assert_eq!(cell_text(&grid_of(&r), 10, 11, 3), "[ ]");
	click(&mut r, 11, 11, 0);
	assert_eq!(cell_text(&grid_of(&r), 10, 11, 3), "[x]");
	// The label is part of the control.
	click(&mut r, 18, 11, 100);
	assert_eq!(cell_text(&grid_of(&r), 10, 11, 3), "[ ]");
	click(&mut r, 40, 11, 200);
	assert_eq!(cell_text(&grid_of(&r), 30, 11, 3), "[x]");
	// A click in the find field puts the cursor there: what is typed goes into it.
	click(&mut r, 30, 10, 300);
	r.0.key(&mut r.1, &mut r.2, Key::Char(b'q'));
	assert_eq!(cell_text(&grid_of(&r), 24, 10, 1), "q");
}

#[test]
fn a_click_outside_a_dialog_button_changes_nothing() {
	let mut r = setup("");
	for k in [Key::F(10), Key::Down, Key::Enter] {
		r.0.key(&mut r.1, &mut r.2, k);
	}
	let before = screen(&r);
	click(&mut r, 2, 2, 0);
	click(&mut r, 30, 12, 50);
	click(&mut r, 70, 20, 100);
	assert_eq!(screen(&r), before);
}
