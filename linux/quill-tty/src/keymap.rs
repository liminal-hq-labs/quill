// Terminal key events to the editor's keys.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// A terminal sends bytes, and a legacy one cannot tell some keys apart: Ctrl+I is the Tab byte (0x09) and Ctrl+M the Enter
// byte (0x0D). The kitty keyboard protocol (`quill-tty` asks for it when the terminal answers) reports them as they were
// pressed, and then Ctrl+I is Italic. Without it Tab stays Tab and Italic is on the Character menu. Ctrl+H (Replace) and
// Ctrl+J (Align Justified) survive in raw mode: crossterm reads 0x08 and 0x0A as those chords, and a terminal's Backspace is
// 0x7F. A terminal that sends 0x08 for Backspace therefore opens Replace until it is told otherwise.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use quill_app::{Dir, Key};

use crate::cp1252;

/// The editor's key for a terminal key event; keys Quill has no use for, and key releases, give `None`.
pub fn map_key(ev: &KeyEvent) -> Option<Key> {
	if ev.kind == KeyEventKind::Release {
		return None;
	}
	let m = ev.modifiers;
	// Page Width: Ctrl+0 where the terminal reports it (the kitty protocol does) and Alt+0 where it cannot, which the editor reads as
	// the DOS screens' Alt+0 (DOS-105).
	if ev.code == KeyCode::Char('0')
		&& (m.contains(KeyModifiers::CONTROL) || m.contains(KeyModifiers::ALT))
		&& !m.contains(KeyModifiers::SUPER)
	{
		return Some(Key::Alt(b'0'));
	}
	if m.contains(KeyModifiers::ALT) || m.contains(KeyModifiers::SUPER) {
		return None;
	}
	let ctrl = m.contains(KeyModifiers::CONTROL);
	let shift = m.contains(KeyModifiers::SHIFT);
	let moved = |plain: Key, dir: Dir| {
		if ctrl {
			None
		} else if shift {
			Some(Key::Select(dir))
		} else {
			Some(plain)
		}
	};
	match ev.code {
		KeyCode::Char(c) if ctrl => c
			.is_ascii_alphabetic()
			.then(|| Key::Ctrl(c.to_ascii_lowercase() as u8)),
		KeyCode::Char(c) => cp1252::encode(c)
			.filter(|b| *b >= 0x20 && *b != 0x7F)
			.map(Key::Char),
		KeyCode::Enter => Some(Key::Enter),
		KeyCode::Tab if !ctrl => Some(Key::Tab),
		KeyCode::BackTab => Some(Key::BackTab),
		KeyCode::Backspace => Some(Key::Backspace),
		KeyCode::Delete => Some(Key::Delete),
		KeyCode::Esc => Some(Key::Esc),
		KeyCode::Insert => Some(Key::Insert),
		KeyCode::F(n @ 1..=12) => Some(Key::F(n)),
		KeyCode::Left => moved(Key::Left, Dir::Left),
		KeyCode::Right => moved(Key::Right, Dir::Right),
		KeyCode::Up => moved(Key::Up, Dir::Up),
		KeyCode::Down => moved(Key::Down, Dir::Down),
		KeyCode::PageUp => moved(Key::PageUp, Dir::PageUp),
		KeyCode::PageDown => moved(Key::PageDown, Dir::PageDown),
		KeyCode::Home if ctrl && !shift => Some(Key::CtrlHome),
		KeyCode::End if ctrl && !shift => Some(Key::CtrlEnd),
		KeyCode::Home => moved(Key::Home, Dir::Home),
		KeyCode::End => moved(Key::End, Dir::End),
		_ => None,
	}
}

/// The keys a pasted text types: its characters, with line ends as Enter and tabs as Tab. Characters Windows-1252 lacks are skipped.
pub fn paste_keys(text: &str) -> Vec<Key> {
	let mut out = Vec::new();
	let mut chars = text.chars().peekable();
	while let Some(c) = chars.next() {
		match c {
			'\r' => {
				if chars.peek() == Some(&'\n') {
					chars.next();
				}
				out.push(Key::Enter);
			}
			'\n' => out.push(Key::Enter),
			'\t' => out.push(Key::Tab),
			c => out.extend(
				cp1252::encode(c)
					.filter(|b| *b >= 0x20 && *b != 0x7F)
					.map(Key::Char),
			),
		}
	}
	out
}

#[cfg(test)]
mod tests {
	use super::*;

	fn ev(code: KeyCode, m: KeyModifiers) -> KeyEvent {
		KeyEvent::new(code, m)
	}

	#[test]
	fn ctrl_0_and_alt_0_are_page_width() {
		for m in [KeyModifiers::CONTROL, KeyModifiers::ALT] {
			assert_eq!(map_key(&ev(KeyCode::Char('0'), m)), Some(Key::Alt(b'0')));
		}
		assert_eq!(
			map_key(&ev(KeyCode::Char('0'), KeyModifiers::NONE)),
			Some(Key::Char(b'0'))
		);
		assert_eq!(map_key(&ev(KeyCode::Char('1'), KeyModifiers::ALT)), None);
	}

	#[test]
	fn plain_and_shifted_keys() {
		let n = KeyModifiers::NONE;
		assert_eq!(map_key(&ev(KeyCode::Char('a'), n)), Some(Key::Char(b'a')));
		assert_eq!(
			map_key(&ev(KeyCode::Char('A'), KeyModifiers::SHIFT)),
			Some(Key::Char(b'A'))
		);
		assert_eq!(map_key(&ev(KeyCode::Char('é'), n)), Some(Key::Char(0xE9)));
		assert_eq!(map_key(&ev(KeyCode::Char('€'), n)), Some(Key::Char(0x80)));
		assert_eq!(map_key(&ev(KeyCode::Char('Ω'), n)), None);
		assert_eq!(
			map_key(&ev(KeyCode::Left, KeyModifiers::SHIFT)),
			Some(Key::Select(Dir::Left))
		);
		assert_eq!(
			map_key(&ev(KeyCode::End, KeyModifiers::SHIFT)),
			Some(Key::Select(Dir::End))
		);
		assert_eq!(
			map_key(&ev(KeyCode::Home, KeyModifiers::CONTROL)),
			Some(Key::CtrlHome)
		);
		assert_eq!(map_key(&ev(KeyCode::Left, KeyModifiers::CONTROL)), None);
		assert_eq!(map_key(&ev(KeyCode::F(10), n)), Some(Key::F(10)));
		assert_eq!(map_key(&ev(KeyCode::F(13), n)), None);
		assert_eq!(map_key(&ev(KeyCode::Char('x'), KeyModifiers::ALT)), None);
	}

	#[test]
	fn control_letters() {
		let c = KeyModifiers::CONTROL;
		assert_eq!(map_key(&ev(KeyCode::Char('s'), c)), Some(Key::Ctrl(b's')));
		assert_eq!(map_key(&ev(KeyCode::Char('S'), c)), Some(Key::Ctrl(b's')));
		assert_eq!(map_key(&ev(KeyCode::Char('1'), c)), None);
		// With the kitty protocol Ctrl+I arrives as itself; without it the terminal sent the Tab byte.
		assert_eq!(map_key(&ev(KeyCode::Char('i'), c)), Some(Key::Ctrl(b'i')));
		assert_eq!(
			map_key(&ev(KeyCode::Tab, KeyModifiers::NONE)),
			Some(Key::Tab)
		);
	}

	#[test]
	fn releases_are_ignored() {
		let mut k = ev(KeyCode::Char('a'), KeyModifiers::NONE);
		k.kind = KeyEventKind::Release;
		assert_eq!(map_key(&k), None);
		k.kind = KeyEventKind::Repeat;
		assert_eq!(map_key(&k), Some(Key::Char(b'a')));
	}

	#[test]
	fn pasted_text_becomes_keys() {
		assert_eq!(
			paste_keys("a\r\nb\tΩ"),
			vec![Key::Char(b'a'), Key::Enter, Key::Char(b'b'), Key::Tab]
		);
	}
}
