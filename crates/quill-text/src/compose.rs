// The precomposed letters Windows-1252 has, as a base and one combining mark (CORE-214).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Quill never normalizes stored text. This table is for two narrow uses: drawing a base letter and a combining mark
//! that compose to a Windows-1252 letter (`e` and U+0301) as that letter, and comparing in Find through the same table,
//! so `é` finds `é` however each was written. It is read from `UnicodeData.txt`'s canonical decompositions of the
//! letters of Windows-1252 that have one: eight combining marks over ASCII letters.
//!
//! A word is `base << 24 | (mark - 0x300) << 16 | letter`, sorted, so a base and a mark are found by binary search on
//! their top sixteen bits.

use crate::unitab::Seq;

const fn m(base: u32, mark: u32, letter: u32) -> u32 {
	(base << 24) | ((mark - 0x300) << 16) | letter
}

// BEGIN GENERATED LETTERS (scripts/gen-unicode.py)
/// The letters, sorted by base then mark: one word each.
#[rustfmt::skip]
pub(crate) const LETTERS: [u32; 58] = [
	m(0x41, 0x0300, 0x00C0), m(0x41, 0x0301, 0x00C1), m(0x41, 0x0302, 0x00C2), m(0x41, 0x0303, 0x00C3),
	m(0x41, 0x0308, 0x00C4), m(0x41, 0x030A, 0x00C5), m(0x43, 0x0327, 0x00C7), m(0x45, 0x0300, 0x00C8),
	m(0x45, 0x0301, 0x00C9), m(0x45, 0x0302, 0x00CA), m(0x45, 0x0308, 0x00CB), m(0x49, 0x0300, 0x00CC),
	m(0x49, 0x0301, 0x00CD), m(0x49, 0x0302, 0x00CE), m(0x49, 0x0308, 0x00CF), m(0x4E, 0x0303, 0x00D1),
	m(0x4F, 0x0300, 0x00D2), m(0x4F, 0x0301, 0x00D3), m(0x4F, 0x0302, 0x00D4), m(0x4F, 0x0303, 0x00D5),
	m(0x4F, 0x0308, 0x00D6), m(0x53, 0x030C, 0x0160), m(0x55, 0x0300, 0x00D9), m(0x55, 0x0301, 0x00DA),
	m(0x55, 0x0302, 0x00DB), m(0x55, 0x0308, 0x00DC), m(0x59, 0x0301, 0x00DD), m(0x59, 0x0308, 0x0178),
	m(0x5A, 0x030C, 0x017D), m(0x61, 0x0300, 0x00E0), m(0x61, 0x0301, 0x00E1), m(0x61, 0x0302, 0x00E2),
	m(0x61, 0x0303, 0x00E3), m(0x61, 0x0308, 0x00E4), m(0x61, 0x030A, 0x00E5), m(0x63, 0x0327, 0x00E7),
	m(0x65, 0x0300, 0x00E8), m(0x65, 0x0301, 0x00E9), m(0x65, 0x0302, 0x00EA), m(0x65, 0x0308, 0x00EB),
	m(0x69, 0x0300, 0x00EC), m(0x69, 0x0301, 0x00ED), m(0x69, 0x0302, 0x00EE), m(0x69, 0x0308, 0x00EF),
	m(0x6E, 0x0303, 0x00F1), m(0x6F, 0x0300, 0x00F2), m(0x6F, 0x0301, 0x00F3), m(0x6F, 0x0302, 0x00F4),
	m(0x6F, 0x0303, 0x00F5), m(0x6F, 0x0308, 0x00F6), m(0x73, 0x030C, 0x0161), m(0x75, 0x0300, 0x00F9),
	m(0x75, 0x0301, 0x00FA), m(0x75, 0x0302, 0x00FB), m(0x75, 0x0308, 0x00FC), m(0x79, 0x0301, 0x00FD),
	m(0x79, 0x0308, 0x00FF), m(0x7A, 0x030C, 0x017E),
];
// END GENERATED LETTERS

/// The table is whole words, sorted.
pub(crate) fn valid(table: Seq<'_>) -> bool {
	let whole = match table {
		Seq::Words(_) => true,
		Seq::Bytes(b) => b.len() % 4 == 0,
	};
	whole && (1..table.len()).all(|i| table.get(i - 1) < table.get(i))
}

pub(crate) fn compose_in(table: Seq<'_>, base: u32, mark: u32) -> Option<char> {
	let key = mark.checked_sub(0x300).filter(|offset| *offset < 0x100)?;
	if base > 0xFF {
		return None;
	}
	let key = (base << 8) | key;
	let (mut low, mut high) = (0, table.len());
	while low < high {
		let middle = low + (high - low) / 2;
		let word = table.get(middle);
		if word >> 16 == key {
			return char::from_u32(word & 0xFFFF);
		}
		if word >> 16 < key {
			low = middle + 1;
		} else {
			high = middle;
		}
	}
	None
}

pub(crate) fn decompose_in(table: Seq<'_>, letter: u32) -> Option<(char, char)> {
	// Under sixty entries, in no order by letter: a scan is the smallest code.
	(0..table.len())
		.map(|i| table.get(i))
		.find(|word| u32::from(*word as u16) == letter)
		.and_then(|word| {
			let base = char::from_u32(word >> 24)?;
			let mark = char::from_u32(0x300 + ((word >> 16) & 0xFF))?;
			Some((base, mark))
		})
}

/// The Windows-1252 letter that `base` and the combining `mark` compose to, such as `é` for `e` and U+0301.
pub fn compose(base: char, mark: char) -> Option<char> {
	compose_in(Seq::Words(&LETTERS), u32::from(base), u32::from(mark))
}

/// The base and the combining mark of a Windows-1252 letter, such as `e` and U+0301 for `é`.
pub fn decompose(letter: char) -> Option<(char, char)> {
	decompose_in(Seq::Words(&LETTERS), u32::from(letter))
}
