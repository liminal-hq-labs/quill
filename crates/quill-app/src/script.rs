// Key tokens: the names the test scenarios use for keys, as DOSBox's AUTOTYPE spells them.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::Key;

/// Turns one token into a key: a letter or digit, or a name such as `enter`, `f10`, `down`, `space`.
pub fn parse_token(t: &str) -> Option<Key> {
	Some(match t {
		"enter" => Key::Enter,
		"bspace" => Key::Backspace,
		"delete" => Key::Delete,
		"esc" => Key::Esc,
		"tab" => Key::Tab,
		"left" => Key::Left,
		"right" => Key::Right,
		"up" => Key::Up,
		"down" => Key::Down,
		"home" => Key::Home,
		"end" => Key::End,
		"pageup" => Key::PageUp,
		"pagedown" => Key::PageDown,
		"space" => Key::Char(b' '),
		"period" => Key::Char(b'.'),
		"minus" => Key::Char(b'-'),
		"insert" => Key::Insert,
		_ => {
			let b = t.as_bytes();
			if b.len() == 1 && b.first().is_some_and(u8::is_ascii_alphanumeric) {
				return b.first().map(|c| Key::Char(*c));
			}
			let n = t.strip_prefix('f')?.parse::<u8>().ok()?;
			Key::F(n)
		}
	})
}
