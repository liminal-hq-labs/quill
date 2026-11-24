// Windows-1252: the document's character set, to and from the Unicode a terminal and a Linux path use.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// The characters of 0x80 to 0x9F; `\u{0}` marks the five codes Windows leaves undefined.
const HIGH: [char; 32] = [
	'€', '\0', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\0', 'Ž', '\0', '\0', '‘',
	'’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\0', 'ž', 'Ÿ',
];

/// The Unicode character for a Windows-1252 byte. An undefined code gives the C1 control of the same number.
pub fn decode(b: u8) -> char {
	match b {
		0x80..=0x9F => match HIGH.get(usize::from(b - 0x80)) {
			Some('\0') | None => char::from(b),
			Some(c) => *c,
		},
		_ => char::from(b),
	}
}

/// The Windows-1252 byte for a character, or `None` when the set has no such character.
pub fn encode(c: char) -> Option<u8> {
	let n = u32::from(c);
	if n < 0x80 || (0xA0..=0xFF).contains(&n) {
		return u8::try_from(n).ok();
	}
	let i = HIGH.iter().position(|h| *h == c && *h != '\0')?;
	u8::try_from(0x80 + i).ok()
}

/// A Linux file name as the editor sees it: UTF-8 turned into Windows-1252, or `None` if some character does not fit.
pub fn name_to_ansi(name: &str) -> Option<Vec<u8>> {
	name.chars().map(encode).collect()
}

/// The text of a name the editor typed or stored.
pub fn ansi_to_string(name: &[u8]) -> String {
	name.iter().map(|b| decode(*b)).collect()
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn every_byte_round_trips_except_the_undefined_codes() {
		for b in 0..=255u8 {
			let c = decode(b);
			if matches!(b, 0x81 | 0x8D | 0x8F | 0x90 | 0x9D) {
				assert_eq!(u32::from(c), u32::from(b));
				continue;
			}
			assert_eq!(encode(c), Some(b), "byte {b:#04x} via {c:?}");
		}
	}

	#[test]
	fn characters_outside_the_set_are_refused() {
		assert_eq!(encode('Ω'), None);
		assert_eq!(encode('\u{81}'), None);
		assert_eq!(name_to_ansi("café.rtf"), Some(b"caf\xE9.rtf".to_vec()));
		assert_eq!(name_to_ansi("日本.rtf"), None);
	}
}
