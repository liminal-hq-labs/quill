// Dialog focus: whichever keys move it, a text-cell dialog shows exactly one focused control.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use proptest::prelude::*;
use quill_app::{App, Key, Scheme};
use quill_grid::Grid;
use quill_host::script::{parse_token, MemFiles};
use quill_host::HostStore;

/// The attribute of the focused control inside a dialog's chrome.
const FOCUS: u8 = 0x07;

/// How many controls look focused in the dialog of height `h` on `g`: runs of the focus attribute in the dialog (not counting the text fields at `fields`, which
/// share it), plus one when the hardware cursor sits in a field.
fn focused(g: &Grid<25>, cursor: bool, h: usize, fields: &[(usize, usize, usize)]) -> usize {
	let mut n = usize::from(cursor);
	for y in 9..8 + h {
		let mut in_run = false;
		for x in 9..69 {
			let cell = g.get(x, y).unwrap();
			let field = fields
				.iter()
				.any(|(fy, x0, x1)| *fy == y && (*x0..*x1).contains(&x));
			let on = cell.attr == FOCUS && !field;
			if on && !in_run {
				n += 1;
			}
			in_run = on;
		}
	}
	n
}

fn app_with_dialog(
	keys: &str,
	scheme: Scheme,
) -> (App<quill_grid::GridMeasure>, HostStore, MemFiles) {
	let mut s = HostStore::new();
	let mut f = MemFiles::default();
	let mut app = App::new();
	app.reset(&mut s).unwrap();
	app.set_scheme(scheme);
	for t in keys.split_whitespace() {
		app.key(&mut s, &mut f, parse_token(t).unwrap());
	}
	(app, s, f)
}

fn nav_key() -> impl Strategy<Value = Key> {
	prop_oneof![
		Just(Key::Tab),
		Just(Key::Left),
		Just(Key::Right),
		Just(Key::Up),
		Just(Key::Down),
		Just(Key::Char(b' ')),
		Just(Key::Char(b'a')),
		Just(Key::Backspace),
	]
}

fn check(keys: &str, scheme: Scheme, walk: &[Key], h: usize, fields: &[(usize, usize, usize)]) {
	let (mut app, mut s, mut f) = app_with_dialog(keys, scheme);
	let mut g = Grid::<25>::new(0);
	for k in walk {
		app.key(&mut s, &mut f, *k);
		let c = quill_app::draw(&app, &s, &mut g);
		// The Find fields are the only ones with a cursor; a focused button or box leaves it off.
		assert_eq!(focused(&g, c.is_some(), h, fields), 1, "after {k:?}");
	}
}

proptest! {
	#[test]
	fn the_save_changes_question_always_shows_one_focused_button(walk in prop::collection::vec(nav_key(), 0..40), mono in any::<bool>()) {
		let scheme = if mono { Scheme::Mono } else { Scheme::Colour };
		// A typed letter modifies the document; File > Exit asks.
		let (mut app, mut s, mut f) = app_with_dialog("x f10 end enter", scheme);
		let mut g = Grid::<25>::new(0);
		let c = quill_app::draw(&app, &s, &mut g);
		prop_assert_eq!(focused(&g, c.is_some(), 7, &[]), 1);
		// Yes is the one at first.
		prop_assert_eq!(g.get(17, 12).unwrap().attr, FOCUS);
		for k in walk {
			app.key(&mut s, &mut f, k);
			let c = quill_app::draw(&app, &s, &mut g);
			// Letters and Backspace do nothing here (y and n would answer, so they are not in the walk).
			prop_assert_eq!(focused(&g, c.is_some(), 7, &[]), 1, "after {:?}", k);
		}
	}

	#[test]
	fn find_and_replace_always_show_one_focused_control(walk in prop::collection::vec(nav_key(), 0..40), replace in any::<bool>(), mono in any::<bool>()) {
		let scheme = if mono { Scheme::Mono } else { Scheme::Colour };
		let open = if replace { "f10 right right right down down enter" } else { "f10 right right right enter" };
		let fields: &[(usize, usize, usize)] = if replace { &[(10, 24, 58), (11, 24, 58)] } else { &[(10, 24, 58)] };
		check(open, scheme, &walk, if replace { 11 } else { 9 }, fields);
	}
}
