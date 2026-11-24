// Windows-1252 (the ANSI code page of Windows 3.1 and 95) to Unicode and back.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Bytes 0x00 to 0x7F are ASCII and 0xA0 to 0xFF are Latin-1, so only 0x80 to 0x9F need a table. The five
//! bytes Windows leaves undefined (0x81, 0x8D, 0x8F, 0x90 and 0x9D) map to the C1 control of the same number,
//! as Windows itself and the WHATWG encoding standard do, so every byte round-trips.

/// Unicode for bytes 0x80 to 0x9F.
const C1: [char; 32] = [
	'€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8d}', 'Ž', '\u{8f}',
	'\u{90}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9d}', 'ž', 'Ÿ',
];

/// The Unicode character for a Windows-1252 byte.
pub fn to_char(b: u8) -> char {
	match b {
		0x80..=0x9F => C1.get(usize::from(b - 0x80)).copied().unwrap_or('\u{FFFD}'),
		_ => char::from(b),
	}
}

/// The Windows-1252 byte for a character, or `None` when the code page has no form for it.
pub fn from_char(c: char) -> Option<u8> {
	match u32::from(c) {
		0x00..=0x7F | 0xA0..=0xFF => u8::try_from(u32::from(c)).ok(),
		// Everything else is in the 0x80 to 0x9F table, which holds a C1 control only for the five undefined bytes.
		_ => {
			let i = C1.iter().position(|&x| x == c)?;
			u8::try_from(i).ok()?.checked_add(0x80)
		}
	}
}
