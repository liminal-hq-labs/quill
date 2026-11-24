// Windows-1252 to Unicode, for the 224 characters layout measures.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

const HIGH: [char; 32] = [
	'€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8d}', 'Ž', '\u{8f}',
	'\u{90}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9d}', 'ž', 'Ÿ',
];

/// The Unicode character of a Windows-1252 byte.
pub fn to_char(b: u8) -> char {
	match b {
		0x80..=0x9F => HIGH
			.get(usize::from(b - 0x80))
			.copied()
			.unwrap_or('\u{FFFD}'),
		_ => char::from(b),
	}
}
