// The toolbar's and ruler's commands: face, size, colour, indents and tab stops, each undoable, and the chrome toggles that persist.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use quill_app::{App, Command, Dir, Key, CHROME_RULER};
use quill_host::script::MemFiles;
use quill_host::HostStore;

fn app_with(text: &str) -> (App, HostStore, MemFiles) {
	let mut s = HostStore::new();
	let mut f = MemFiles::default();
	let mut app = App::new();
	app.reset(&mut s).unwrap();
	for b in text.bytes() {
		app.key(&mut s, &mut f, Key::Char(b));
	}
	(app, s, f)
}

#[test]
fn face_size_and_colour_apply_to_the_selection_and_show_at_the_caret() {
	let (mut app, mut s, mut f) = app_with("hello");
	app.key(&mut s, &mut f, Key::Home);
	for _ in 0..5 {
		app.key(&mut s, &mut f, Key::Select(Dir::Right));
	}
	app.set_face(&mut s, b"Arial", 2);
	app.set_size(&mut s, 40);
	app.set_colour(&mut s, Some((255, 0, 0)));
	let cs = app.char_state(&s);
	assert_eq!(&cs.face[..usize::from(cs.face_len)], b"Arial");
	assert_eq!((cs.size, cs.colour), (40, Some((255, 0, 0))));
	app.set_colour(&mut s, None);
	assert_eq!(app.char_state(&s).colour, None);
	app.command(&mut s, &mut f, Command::Undo);
	assert_eq!(app.char_state(&s).colour, Some((255, 0, 0)));
}

#[test]
fn indents_and_tab_stops_set_from_the_ruler() {
	let (mut app, mut s, _f) = app_with("a line");
	app.set_indents(&mut s, 720, -360, 360);
	let pf = app.para_state(&s);
	assert_eq!((pf.left, pf.first, pf.right), (720, -360, 360));
	app.add_tab(&mut s, 1440, 1);
	app.add_tab(&mut s, 2880, 3);
	let mut tabs = [(0, 0); 16];
	assert_eq!(app.tab_stops(&s, &mut tabs), 2);
	assert_eq!(&tabs[..2], &[(1440, 1), (2880, 3)]);
	app.remove_tab(&mut s, 1440);
	assert_eq!(app.tab_stops(&s, &mut tabs), 1);
	assert_eq!(tabs[0], (2880, 3));
}

#[test]
fn hiding_the_ruler_is_remembered_in_quill_ini() {
	let (mut app, mut s, mut f) = app_with("");
	assert!(app.show_toolbar() && app.show_ruler() && app.show_status());
	app.command(&mut s, &mut f, Command::Chrome(CHROME_RULER));
	assert!(!app.show_ruler());
	app.save_settings(&mut f);
	let mut again = App::new();
	again.load_settings(&mut f);
	assert!(again.show_toolbar() && !again.show_ruler() && again.show_status());
}

#[test]
fn a_style_preset_is_one_undo_step_and_the_box_reads_it_back() {
	let (mut app, mut s, mut f) = app_with("Chapter one");
	app.key(&mut s, &mut f, Key::Enter);
	for b in b"Some text." {
		app.key(&mut s, &mut f, Key::Char(*b));
	}
	app.key(&mut s, &mut f, Key::CtrlHome);
	assert_eq!(app.style_of(&s), Some(0), "plain text is Normal");
	app.apply_style(&mut s, 1);
	assert_eq!(app.style_of(&s), Some(1));
	let cs = app.char_state(&s);
	assert_eq!(cs.size, 32);
	assert_ne!(cs.flags & 1, 0, "bold");
	// The second paragraph is untouched.
	app.key(&mut s, &mut f, Key::Down);
	assert_eq!(app.style_of(&s), Some(0));
	// A direct change makes the box blank.
	app.key(&mut s, &mut f, Key::CtrlHome);
	app.set_indents(&mut s, 100, 0, 0);
	assert_eq!(app.style_of(&s), None);
	// One Undo takes back the indent, one more takes back the whole preset.
	app.command(&mut s, &mut f, Command::Undo);
	assert_eq!(app.style_of(&s), Some(1));
	app.command(&mut s, &mut f, Command::Undo);
	assert_eq!(app.style_of(&s), Some(0));
	assert_eq!(app.char_state(&s).size, 24);
	// Every preset reads back as itself.
	for n in 0..quill_app::STYLE_NAMES.len() {
		app.apply_style(&mut s, n);
		assert_eq!(app.style_of(&s), Some(n), "{}", quill_app::STYLE_NAMES[n]);
	}
}
