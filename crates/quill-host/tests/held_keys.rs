// Held keys: a key the keyboard repeats acts again only if it is meant to (text, movement, Undo); a toggle acts once per press.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use quill_app::{App, Command, Key};
use quill_grid::GridMeasure;
use quill_host::script::MemFiles;
use quill_host::HostStore;

type Editor = (App<GridMeasure>, HostStore, MemFiles);

fn editor() -> Editor {
	let mut s = HostStore::new();
	let mut app = App::new();
	app.reset(&mut s).unwrap();
	(app, s, MemFiles::default())
}

fn press(e: &mut Editor, keys: &[Key]) {
	for k in keys {
		e.0.key(&mut e.1, &mut e.2, *k);
	}
}

/// What a shell does with the repeats of a held key: `n` more of `k` after the press.
fn hold(e: &mut Editor, k: Key, n: usize) {
	press(e, &[k]);
	for _ in 0..n {
		e.0.key_held(&mut e.1, &mut e.2, k);
	}
}

fn on(e: &Editor, c: Command) -> bool {
	e.0.menu_mark(&e.1, c).checked
}

#[test]
fn a_held_toggle_acts_once() {
	for (key, command) in [
		(b'i', Command::Italic),
		(b'b', Command::Bold),
		(b'u', Command::Underline),
	] {
		let mut e = editor();
		press(&mut e, &[Key::Char(b'a'), Key::Char(b'b'), Key::Ctrl(b'a')]);
		// An even number of repeats after the press: the old behaviour would end with it off again.
		hold(&mut e, Key::Ctrl(key), 30);
		assert!(on(&e, command), "{command:?} is on after one press held");
		// The next press (a new hold) turns it off, once.
		hold(&mut e, Key::Ctrl(key), 31);
		assert!(
			!on(&e, command),
			"{command:?} is off after the second press"
		);
	}
}

#[test]
fn a_held_alignment_or_insert_acts_once() {
	let mut e = editor();
	press(&mut e, &[Key::Char(b'a')]);
	hold(&mut e, Key::Ctrl(b'e'), 10);
	assert!(on(&e, Command::Align(1)));
	assert!(!e.0.key_repeats(Key::Insert));
	assert!(!e.0.key_repeats(Key::F(8)));
	assert!(!e.0.key_repeats(Key::Ctrl(b's')));
}

#[test]
fn a_held_text_key_types_repeatedly() {
	let mut e = editor();
	hold(&mut e, Key::Char(b'a'), 29);
	assert_eq!(e.0.caret(), 30);
}

#[test]
fn held_arrows_and_backspace_repeat() {
	let mut e = editor();
	press(
		&mut e,
		&[
			Key::Char(b'a'),
			Key::Char(b'b'),
			Key::Char(b'c'),
			Key::Char(b'd'),
		],
	);
	hold(&mut e, Key::Left, 2);
	assert_eq!(e.0.caret(), 1);
	hold(&mut e, Key::Right, 1);
	assert_eq!(e.0.caret(), 3);
	hold(&mut e, Key::Backspace, 2);
	assert_eq!(e.0.caret(), 0);
}

#[test]
fn held_undo_and_redo_repeat() {
	let e = editor();
	assert!(e.0.key_repeats(Key::Ctrl(b'z')));
	assert!(e.0.key_repeats(Key::Ctrl(b'y')));
	assert!(Command::Undo.repeats() && Command::Redo.repeats() && Command::Zoom(0).repeats());
	assert!(!Command::Bold.repeats() && !Command::Italic.repeats() && !Command::Super.repeats());
	assert!(
		!Command::Chrome(1).repeats() && !Command::Align(1).repeats() && !Command::Save.repeats()
	);
}

#[test]
fn a_held_key_in_a_menu_moves_but_does_not_choose() {
	let mut e = editor();
	press(&mut e, &[Key::F(10)]);
	assert!(e.0.key_repeats(Key::Down));
	assert!(!e.0.key_repeats(Key::Enter));
	assert!(!e.0.key_repeats(Key::Char(b'o')));
	assert!(!e.0.key_repeats(Key::Esc));
}
