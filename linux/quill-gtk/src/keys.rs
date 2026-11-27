// The keyboard: GTK's key events turned into the editor's own `Key`, as the other shells do.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// `map_key` takes a keyval and the modifier state and says what the editor should do: a `Key` of its own (arrows, Home and End,
// the page keys, Backspace, Delete, Enter, Tab, Escape, Insert, a function key, Shift with a movement key as `Select`), a
// movement the editor has no key for (a word, the ends of the document, with or without Shift), or a character to type. Characters
// normally arrive from the input method (`GtkIMContext` commits), which filters a printable key before the key event is
// delivered; `Text` is what a key that was not filtered would type, so the mapping is whole without a display. Ctrl with a letter
// is the editor's `Key::Ctrl`, though the menu's accelerators take those first. Alt belongs to the menus.
//
// `events_for` is the inverse for the keys a script holds, so a test can send a scenario through the mapping.

use gtk::gdk::{Key as K, ModifierType as M};
use quill_app::{Dir, Key};

/// A movement the editor has no key for.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Move {
	/// A word to the left.
	WordLeft,
	/// A word to the right.
	WordRight,
	/// To the start of the document.
	DocStart,
	/// To the end of the document.
	DocEnd,
}

/// What a key press means.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Mapped {
	/// A key of the editor's own.
	Edit(Key),
	/// A movement, extending the selection when `extend`.
	Move { to: Move, extend: bool },
	/// A character to type, when the input method did not take the key.
	Text(char),
	/// Nothing: a key the editor does not use, or one that belongs to a menu or the desktop.
	Ignore,
}

fn dir_of(key: K) -> Option<Dir> {
	Some(match key {
		K::Left | K::KP_Left => Dir::Left,
		K::Right | K::KP_Right => Dir::Right,
		K::Up | K::KP_Up => Dir::Up,
		K::Down | K::KP_Down => Dir::Down,
		K::Home | K::KP_Home => Dir::Home,
		K::End | K::KP_End => Dir::End,
		K::Page_Up | K::KP_Page_Up => Dir::PageUp,
		K::Page_Down | K::KP_Page_Down => Dir::PageDown,
		_ => return None,
	})
}

fn plain_key(d: Dir) -> Key {
	match d {
		Dir::Left => Key::Left,
		Dir::Right => Key::Right,
		Dir::Up => Key::Up,
		Dir::Down => Key::Down,
		Dir::Home => Key::Home,
		Dir::End => Key::End,
		Dir::PageUp => Key::PageUp,
		Dir::PageDown => Key::PageDown,
	}
}

/// What the key `key` with the modifiers `state` means.
pub fn map_key(key: K, state: M) -> Mapped {
	// Alt belongs to the menus' mnemonics, and Super to the desktop.
	if state.intersects(M::ALT_MASK | M::SUPER_MASK | M::META_MASK) {
		return Mapped::Ignore;
	}
	let ctrl = state.contains(M::CONTROL_MASK);
	let shift = state.contains(M::SHIFT_MASK);
	if let Some(d) = dir_of(key) {
		return match (ctrl, d) {
			(true, Dir::Left) => Mapped::Move {
				to: Move::WordLeft,
				extend: shift,
			},
			(true, Dir::Right) => Mapped::Move {
				to: Move::WordRight,
				extend: shift,
			},
			(true, Dir::Home) if shift => Mapped::Move {
				to: Move::DocStart,
				extend: true,
			},
			(true, Dir::End) if shift => Mapped::Move {
				to: Move::DocEnd,
				extend: true,
			},
			(true, Dir::Home) => Mapped::Edit(Key::CtrlHome),
			(true, Dir::End) => Mapped::Edit(Key::CtrlEnd),
			(true, _) => Mapped::Ignore,
			(false, d) if shift => Mapped::Edit(Key::Select(d)),
			(false, d) => Mapped::Edit(plain_key(d)),
		};
	}
	if ctrl {
		// Ctrl with a letter is the editor's, except Backspace and the like, which it has no use for.
		return match key.to_unicode() {
			Some(c) if c.is_ascii_alphabetic() => {
				let c = c.to_ascii_lowercase();
				// Ctrl+Shift+Z is Redo, as GTK's habit has it.
				if shift && c == 'z' {
					Mapped::Edit(Key::Ctrl(b'y'))
				} else if shift {
					Mapped::Ignore
				} else {
					Mapped::Edit(Key::Ctrl(c as u8))
				}
			}
			_ => Mapped::Ignore,
		};
	}
	match key {
		K::Return | K::KP_Enter | K::ISO_Enter => Mapped::Edit(Key::Enter),
		K::Tab | K::KP_Tab if !shift => Mapped::Edit(Key::Tab),
		K::BackSpace => Mapped::Edit(Key::Backspace),
		K::Delete | K::KP_Delete => Mapped::Edit(Key::Delete),
		K::Escape => Mapped::Edit(Key::Esc),
		K::Insert | K::KP_Insert => Mapped::Edit(Key::Insert),
		K::F1
		| K::F2
		| K::F3
		| K::F4
		| K::F5
		| K::F6
		| K::F7
		| K::F8
		| K::F9
		| K::F10
		| K::F11
		| K::F12
			if !shift =>
		{
			let n = key
				.name()
				.and_then(|n| n.strip_prefix('F').and_then(|d| d.parse().ok()));
			n.map_or(Mapped::Ignore, |n| Mapped::Edit(Key::F(n)))
		}
		_ => match key.to_unicode() {
			Some(c) if !c.is_control() => Mapped::Text(c),
			_ => Mapped::Ignore,
		},
	}
}

/// The key press that gives `k`, for sending a script through the mapping: the keyval and the modifiers, or `None` for a key GTK
/// has no press for (the `Alt` keys, which the menus own).
pub fn events_for(k: Key) -> Option<(K, M)> {
	let n = M::empty();
	let sel = |key| Some((key, M::SHIFT_MASK));
	Some(match k {
		Key::Char(b) => (char_keyval(quill_tty::cp1252::decode(b)), n),
		Key::Enter => (K::Return, n),
		Key::Backspace => (K::BackSpace, n),
		Key::Delete => (K::Delete, n),
		Key::Esc => (K::Escape, n),
		Key::Tab => (K::Tab, n),
		Key::Left => (K::Left, n),
		Key::Right => (K::Right, n),
		Key::Up => (K::Up, n),
		Key::Down => (K::Down, n),
		Key::Home => (K::Home, n),
		Key::End => (K::End, n),
		Key::PageUp => (K::Page_Up, n),
		Key::PageDown => (K::Page_Down, n),
		Key::CtrlHome => (K::Home, M::CONTROL_MASK),
		Key::CtrlEnd => (K::End, M::CONTROL_MASK),
		Key::Select(d) => {
			return sel(match d {
				Dir::Left => K::Left,
				Dir::Right => K::Right,
				Dir::Up => K::Up,
				Dir::Down => K::Down,
				Dir::Home => K::Home,
				Dir::End => K::End,
				Dir::PageUp => K::Page_Up,
				Dir::PageDown => K::Page_Down,
			})
		}
		Key::Insert => (K::Insert, n),
		Key::F(i) => (K::from_name(format!("F{i}"))?, n),
		Key::Ctrl(c) => (char_keyval(char::from(c)), M::CONTROL_MASK),
		// Shift+Tab (the previous picture) has no press in this client yet: `ISO_Left_Tab` is ignored above.
		Key::Alt(_) | Key::BackTab => return None,
	})
}

fn char_keyval(c: char) -> K {
	// SAFETY: every `u32` is a keyval to GDK; `Key` is a plain wrapper around one.
	unsafe { gtk::glib::translate::FromGlib::from_glib(gtk::gdk::unicode_to_keyval(c as u32)) }
}

#[cfg(test)]
mod tests {
	use super::*;

	fn m(key: K, state: M) -> Mapped {
		map_key(key, state)
	}

	#[test]
	fn the_movement_keys_are_the_editors_and_shift_selects() {
		let n = M::empty();
		assert_eq!(m(K::Left, n), Mapped::Edit(Key::Left));
		assert_eq!(m(K::Page_Down, n), Mapped::Edit(Key::PageDown));
		assert_eq!(m(K::Home, n), Mapped::Edit(Key::Home));
		assert_eq!(m(K::KP_End, n), Mapped::Edit(Key::End));
		assert_eq!(
			m(K::Right, M::SHIFT_MASK),
			Mapped::Edit(Key::Select(Dir::Right))
		);
		assert_eq!(
			m(K::End, M::SHIFT_MASK),
			Mapped::Edit(Key::Select(Dir::End))
		);
		assert_eq!(m(K::Home, M::CONTROL_MASK), Mapped::Edit(Key::CtrlHome));
		assert_eq!(m(K::End, M::CONTROL_MASK), Mapped::Edit(Key::CtrlEnd));
	}

	#[test]
	fn ctrl_gives_words_and_the_ends_of_the_document_with_or_without_shift() {
		let c = M::CONTROL_MASK;
		let cs = M::CONTROL_MASK | M::SHIFT_MASK;
		assert_eq!(
			m(K::Left, c),
			Mapped::Move {
				to: Move::WordLeft,
				extend: false
			}
		);
		assert_eq!(
			m(K::Right, cs),
			Mapped::Move {
				to: Move::WordRight,
				extend: true
			}
		);
		assert_eq!(
			m(K::Home, cs),
			Mapped::Move {
				to: Move::DocStart,
				extend: true
			}
		);
		assert_eq!(
			m(K::End, cs),
			Mapped::Move {
				to: Move::DocEnd,
				extend: true
			}
		);
		assert_eq!(m(K::Up, c), Mapped::Ignore);
	}

	#[test]
	fn the_editing_keys_map_and_a_locked_numlock_changes_nothing() {
		let n = M::empty();
		assert_eq!(m(K::Return, n), Mapped::Edit(Key::Enter));
		assert_eq!(m(K::KP_Enter, n), Mapped::Edit(Key::Enter));
		assert_eq!(m(K::Tab, n), Mapped::Edit(Key::Tab));
		assert_eq!(m(K::ISO_Left_Tab, M::SHIFT_MASK), Mapped::Ignore);
		assert_eq!(m(K::BackSpace, n), Mapped::Edit(Key::Backspace));
		assert_eq!(m(K::Delete, n), Mapped::Edit(Key::Delete));
		assert_eq!(m(K::Escape, n), Mapped::Edit(Key::Esc));
		assert_eq!(m(K::Insert, n), Mapped::Edit(Key::Insert));
		assert_eq!(m(K::F7, n), Mapped::Edit(Key::F(7)));
		assert_eq!(m(K::F12, n), Mapped::Edit(Key::F(12)));
		assert_eq!(
			m(K::Left, M::LOCK_MASK | M::from_bits_truncate(1 << 4)),
			Mapped::Edit(Key::Left)
		);
	}

	#[test]
	fn ctrl_letters_are_the_editors_and_alt_is_the_menus() {
		let c = M::CONTROL_MASK;
		assert_eq!(m(K::b, c), Mapped::Edit(Key::Ctrl(b'b')));
		assert_eq!(m(K::B, c), Mapped::Edit(Key::Ctrl(b'b')));
		assert_eq!(m(K::z, c | M::SHIFT_MASK), Mapped::Edit(Key::Ctrl(b'y')));
		assert_eq!(m(K::k, c | M::SHIFT_MASK), Mapped::Ignore);
		assert_eq!(m(K::BackSpace, c), Mapped::Ignore);
		assert_eq!(m(K::f, M::ALT_MASK), Mapped::Ignore);
		assert_eq!(m(K::Left, M::ALT_MASK), Mapped::Ignore);
	}

	#[test]
	fn a_printable_key_the_input_method_left_types_its_character() {
		assert_eq!(m(K::a, M::empty()), Mapped::Text('a'));
		assert_eq!(m(K::A, M::SHIFT_MASK), Mapped::Text('A'));
		assert_eq!(m(K::space, M::empty()), Mapped::Text(' '));
		assert_eq!(m(K::eacute, M::empty()), Mapped::Text('\u{e9}'));
		assert_eq!(m(K::Shift_L, M::empty()), Mapped::Ignore);
	}

	#[test]
	fn every_key_a_script_holds_survives_the_mapping() {
		let mut keys = vec![
			Key::Enter,
			Key::Backspace,
			Key::Delete,
			Key::Esc,
			Key::Tab,
			Key::Left,
			Key::Right,
			Key::Up,
			Key::Down,
			Key::Home,
			Key::End,
			Key::PageUp,
			Key::PageDown,
			Key::CtrlHome,
			Key::CtrlEnd,
			Key::Insert,
		];
		keys.extend((1..=12).map(Key::F));
		keys.extend((b'a'..=b'z').map(Key::Ctrl));
		for d in [
			Dir::Left,
			Dir::Right,
			Dir::Up,
			Dir::Down,
			Dir::Home,
			Dir::End,
			Dir::PageUp,
			Dir::PageDown,
		] {
			keys.push(Key::Select(d));
		}
		for k in keys {
			let (key, state) = events_for(k).unwrap();
			assert_eq!(map_key(key, state), Mapped::Edit(k), "{k:?}");
		}
		// Every printable Windows-1252 character comes back as itself.
		for b in (0x20..=0xFFu8).filter(|b| !matches!(b, 0x7F | 0x81 | 0x8D | 0x8F | 0x90 | 0x9D)) {
			let (key, state) = events_for(Key::Char(b)).unwrap();
			let want = quill_tty::cp1252::decode(b);
			assert_eq!(map_key(key, state), Mapped::Text(want), "{b:#x}");
		}
	}
}
