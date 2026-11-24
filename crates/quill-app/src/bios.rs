// Keys from the BIOS: what INT 16h returns for a key, as the editor's `Key`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! INT 16h AH=00h returns an ASCII code in AL and a scan code in AH. With Ctrl held, a letter comes back as the control code 1 to 26,
//! so Ctrl+I has the code of Tab (9), Ctrl+H that of Backspace (8), Ctrl+M that of Enter (13) and Ctrl+J, with Ctrl+Enter, the line
//! feed (10): the ASCII code cannot tell them apart, and the scan code can, because Tab, Backspace and Enter have scan codes of their
//! own. Alt with a key returns 0 in AL and the key's scan code in AH.

use crate::Key;

/// The letter or digit an Alt scan code stands for, as a lower-case character.
fn alt_letter(scan: u8) -> Option<u8> {
	const ROW1: &[u8; 10] = b"qwertyuiop";
	const ROW2: &[u8; 9] = b"asdfghjkl";
	const ROW3: &[u8; 7] = b"zxcvbnm";
	match scan {
		0x10..=0x19 => ROW1.get(usize::from(scan - 0x10)).copied(),
		0x1E..=0x26 => ROW2.get(usize::from(scan - 0x1E)).copied(),
		0x2C..=0x32 => ROW3.get(usize::from(scan - 0x2C)).copied(),
		0x78..=0x80 => Some(b'1' + (scan - 0x78)),
		0x81 => Some(b'0'),
		_ => None,
	}
}

/// Turns a BIOS key (ASCII code, scan code) into a `Key`; keys Quill has no use for come back as `None`.
pub fn decode(ascii: u8, scan: u8) -> Option<Key> {
	Some(match (ascii, scan) {
		(0 | 0xE0, 0x48) => Key::Up,
		(0 | 0xE0, 0x50) => Key::Down,
		(0 | 0xE0, 0x4B) => Key::Left,
		(0 | 0xE0, 0x4D) => Key::Right,
		(0 | 0xE0, 0x47) => Key::Home,
		(0 | 0xE0, 0x4F) => Key::End,
		(0 | 0xE0, 0x49) => Key::PageUp,
		(0 | 0xE0, 0x51) => Key::PageDown,
		(0 | 0xE0, 0x53) => Key::Delete,
		(0 | 0xE0, 0x52) => Key::Insert,
		(0, 0x77) => Key::CtrlHome,
		(0, 0x75) => Key::CtrlEnd,
		(0, 0x3B..=0x44) => Key::F(scan - 0x3B + 1),
		(0, 0x85) => Key::F(11),
		(0, 0x86) => Key::F(12),
		(0, 0x0E) => Key::Alt(8),
		(0, s) if alt_letter(s).is_some() => Key::Alt(alt_letter(s)?),
		// The keys with a code of their own: the scan code says whether it was the key or Ctrl with a letter.
		(0x0D, 0x1C | 0xE0) => Key::Enter,
		(0x08, 0x0E) => Key::Backspace,
		(0x09, 0x0F) => Key::Tab,
		(0x1B, 0x01) => Key::Esc,
		// Ctrl+Enter, Ctrl+[ and Ctrl+Backspace come back as other keys' codes; they are not shortcuts.
		(0x0A, 0x1C) | (0x1B, _) | (0x7F, _) => return None,
		(0x01..=0x1A, _) => Key::Ctrl(ascii + 0x60),
		(0x20..=0xFF, _) => Key::Char(ascii),
		_ => return None,
	})
}
