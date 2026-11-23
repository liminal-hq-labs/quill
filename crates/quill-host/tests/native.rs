// Native dialogs: with them on, the editor asks its shell for each dialog and never draws one in text cells or opens a text menu.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use quill_app::{App, Command, Dialog, FindAction, Key};
use quill_grid::Grid;
use quill_host::script::MemFiles;
use quill_host::HostStore;

fn setup(text: &str) -> (App, HostStore, MemFiles) {
	let mut s = HostStore::new();
	let mut f = MemFiles::default();
	let mut app = App::new();
	app.reset(&mut s).unwrap();
	app.set_native_dialogs(true);
	for b in text.bytes() {
		app.key(&mut s, &mut f, Key::Char(b));
	}
	(app, s, f)
}

/// True when the screen holds a box-drawing corner: what the text-cell dialogs and menus are made of.
fn has_box(app: &App, s: &HostStore) -> bool {
	let mut g = Grid::<25>::new(0);
	quill_app::draw(app, s, &mut g);
	(0..25).any(|y| {
		(0..80).any(|x| {
			matches!(
				g.get(x, y).map(|c| c.ch),
				Some(0xDA | 0xC9 | 0xBF | 0xBB | 0xC0 | 0xC8)
			)
		})
	})
}

#[test]
fn every_dialog_command_becomes_a_request_and_the_page_stays_clean() {
	let (mut app, mut s, mut f) = setup("hello");
	app.discard_changes();
	for (c, want) in [
		(Command::Open, Dialog::Open),
		(Command::SaveAs, Dialog::SaveAs),
		(Command::Save, Dialog::SaveAs),
		(Command::Picture, Dialog::Picture),
		(Command::Print, Dialog::Print),
		(Command::Fonts, Dialog::Fonts),
		(Command::Tabs, Dialog::Tabs),
		(Command::About, Dialog::About),
		(Command::Find, Dialog::Find(false)),
		(Command::Replace, Dialog::Find(true)),
	] {
		app.command(&mut s, &mut f, c);
		assert_eq!(app.take_dialog(), Some(want), "{c:?}");
		assert_eq!(app.take_dialog(), None, "a request is taken once");
		assert!(!has_box(&app, &s), "{c:?} drew a text-cell box");
	}
	// The shortcuts and F10 reach the same place or nowhere.
	app.key(&mut s, &mut f, Key::Ctrl(b'o'));
	assert_eq!(app.take_dialog(), Some(Dialog::Open));
	app.key(&mut s, &mut f, Key::F(10));
	assert!(!has_box(&app, &s), "F10 opened a text menu");
	assert_eq!(app.take_dialog(), None);
}

#[test]
fn unsaved_changes_ask_before_new_open_and_exit_and_then_go_on() {
	let (mut app, mut s, mut f) = setup("some text");
	assert!(app.modified());
	app.command(&mut s, &mut f, Command::New);
	assert_eq!(app.take_dialog(), Some(Dialog::ConfirmSave(Command::New)));
	assert!(!has_box(&app, &s));
	// Don't save: the shell discards and runs the command.
	app.discard_changes();
	app.run_confirmed(&mut s, &mut f, Command::New);
	assert_eq!(app.take_dialog(), None);
	let mut b = [0u8; 8];
	assert_eq!(
		app.doc().body.read(&s, 0, &mut b),
		1,
		"the new document is empty"
	);
	app.key(&mut s, &mut f, Key::Char(b'x'));
	app.command(&mut s, &mut f, Command::Exit);
	assert_eq!(app.take_dialog(), Some(Dialog::ConfirmSave(Command::Exit)));
	assert!(!app.quit());
	app.run_confirmed(&mut s, &mut f, Command::Exit);
	assert!(app.quit());
}

#[test]
fn the_dialogs_answers_apply_through_the_same_state_layer() {
	let (mut app, mut s, mut f) = setup("one two one three one");
	// Find and Replace.
	app.key(&mut s, &mut f, Key::CtrlHome);
	assert!(app.find_text(&mut s, b"two", b"", false, false, FindAction::Next));
	assert!(!app.find_text(&mut s, b"absent", b"", false, false, FindAction::Next));
	app.find_text(&mut s, b"one", b"1", false, false, FindAction::ReplaceAll);
	let mut b = [0u8; 32];
	let n = app.doc().body.read(&s, 0, &mut b);
	assert_eq!(&b[..n - 1], b"1 two 1 three 1");
	assert_eq!(app.last_find().0, b"one");
	// Fonts: face, size and styles together.
	app.key(&mut s, &mut f, Key::CtrlHome);
	for _ in 0..3 {
		app.key(&mut s, &mut f, Key::Select(quill_app::Dir::Right));
	}
	app.set_font(&mut s, b"Arial", 2, 28, 1 | 4);
	let cs = app.char_state(&s);
	assert_eq!((cs.size, cs.flags & 0xF), (28, 1 | 4));
	assert_eq!(&cs.face[..usize::from(cs.face_len)], b"Arial");
	// Tabs with a leader, then all cleared.
	app.set_tab(&mut s, 1440, 2, 1);
	let mut tabs = [(0, 0); 16];
	assert_eq!(app.tab_stops(&s, &mut tabs), 1);
	app.clear_tabs(&mut s);
	assert_eq!(app.tab_stops(&s, &mut tabs), 0);
	// The folder of the last file dialog is remembered in the settings.
	app.remember_folder(b"C:\\DOCS\\LETTERS\\A.QLD");
	assert_eq!(app.last_folder(), b"C:\\DOCS\\LETTERS\\");
	app.save_settings(&mut f);
	let mut again = App::new();
	again.load_settings(&mut f);
	assert_eq!(again.last_folder(), b"C:\\DOCS\\LETTERS\\");
}

#[test]
fn the_text_build_still_draws_its_own_dialogs() {
	let mut s = HostStore::new();
	let mut f = MemFiles::default();
	let mut app = App::new();
	app.reset(&mut s).unwrap();
	app.command(&mut s, &mut f, Command::About);
	assert_eq!(app.take_dialog(), None);
	assert!(has_box(&app, &s), "text mode shows About in a box");
}
