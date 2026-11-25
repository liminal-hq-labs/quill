// The command layer: what a key, a committed string or a movement does to the editor, with no window and no particular layout.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// `Document` calls these over its `App`, which is laid out for the screen at 96 pixels to the inch with real outline fonts. They
// are generic over the app's measure so the replay test can send the host's key scripts through the very same functions over the
// text grid the scripts' goldens were made on (`tests/replay.rs`): the bytes a script saves depend on where its Down and Page Down
// keys go, which is the layout's.

use quill_app::{App, Dialog, Files, Key};
use quill_core::layout::Measure;
use quill_core::store::Store;

use crate::keys::{Mapped, Move};

/// What typing a string did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Typed {
	/// How many characters went into the document.
	pub inserted: usize,
	/// The characters Windows-1252 has no form for, which were refused.
	pub refused: Vec<char>,
}

/// What a mapped key did besides changing the document.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Outcome {
	/// The dialog the editor asked for (a function key's command), if any.
	pub dialog: Option<Dialog>,
	/// What a typed character did.
	pub typed: Typed,
}

/// Types a string the way an input method commits it: each character that Windows-1252 has a form for goes in as the key for that
/// byte, a line break is Enter and a tab is Tab, other control characters are dropped, and any other character is refused (D188).
pub fn type_text<M: Measure + Clone, S: Store + ?Sized, F: Files>(
	app: &mut App<M>,
	s: &mut S,
	f: &mut F,
	text: &str,
) -> Typed {
	let mut out = Typed::default();
	for c in text.chars() {
		let k = match c {
			'\n' | '\r' => Key::Enter,
			'\t' => Key::Tab,
			c if c.is_control() => continue,
			c => match quill_tty::cp1252::encode(c) {
				Some(b) => Key::Char(b),
				None => {
					out.refused.push(c);
					continue;
				}
			},
		};
		app.key(s, f, k);
		out.inserted += 1;
	}
	out
}

/// Moves the caret a word left or right, which the editor has no key for, extending the selection when asked. A word is what a
/// double click takes: its letters and the spaces after it.
pub fn move_by_word<M: Measure + Clone, S: Store + ?Sized>(
	app: &mut App<M>,
	s: &mut S,
	left: bool,
	extend: bool,
) {
	let at = app.caret();
	let t = &app.doc().body;
	let kind = |p: u32| match t.read_byte(s, p) {
		None | Some(0x0D) => 2u8,
		Some(b' ' | 0x09) => 1,
		_ => 0,
	};
	let to = if left {
		let mut p = at;
		while p > 0 && kind(p - 1) == 1 {
			p -= 1;
		}
		if p > 0 {
			let k = kind(p - 1);
			p -= 1;
			while k != 2 && p > 0 && kind(p - 1) == k {
				p -= 1;
			}
		}
		p
	} else if kind(at) == 2 {
		at + 1
	} else {
		app.word_range(s, at).1
	};
	app.place_caret(s, to, extend);
}

/// Does what a mapped key means.
pub fn apply<M: Measure + Clone, S: Store + ?Sized, F: Files>(
	app: &mut App<M>,
	s: &mut S,
	f: &mut F,
	m: Mapped,
) -> Outcome {
	let mut out = Outcome::default();
	match m {
		Mapped::Edit(k) => {
			app.key(s, f, k);
			out.dialog = app.take_dialog();
		}
		Mapped::Text(c) => out.typed = type_text(app, s, f, &c.to_string()),
		Mapped::Move { to, extend } => match to {
			Move::WordLeft => move_by_word(app, s, true, extend),
			Move::WordRight => move_by_word(app, s, false, extend),
			Move::DocStart => app.place_caret(s, 0, extend),
			Move::DocEnd => app.place_caret(s, u32::MAX, extend),
		},
		Mapped::Ignore => {}
	}
	out
}
