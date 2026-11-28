// The Unicode block of a character, for the status line, and the short names of invisible characters (CORE-228).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! About 75 blocks a document is likely to hold have a name here, from Latin-1 Supplement to Halfwidth and Fullwidth
//! Forms and the emoji blocks; every other code point is [`Block::OTHER`], named `Other`. The status line reads
//! `U+0416 Cyrillic: this character cannot be shown here.` with the block's label, which is the `Blocks.txt` name unless it
//! is long (`Pictographs` for Miscellaneous Symbols and Pictographs).
//!
//! The short names are the labels Reveal Codes puts on an invisible character: `[ZWJ]`, `[ZWSP]`, `[WJ]`, `[LRM]`,
//! `[RLM]`, `[VS16]`, `[BOM]` and five more, twelve in all.

use crate::unitab::{lookup, pack as r, Seq};

/// A block of Unicode that has a name here: the place of its label in the list of names, counting from 1, or 0 for any other.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Block(pub u8);

impl Block {
	/// Every code point that is in no named block.
	pub const OTHER: Block = Block(0);
}

/// An invisible character's short name: up to four ASCII letters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShortName([u8; 4]);

impl ShortName {
	/// The letters, such as `ZWJ`.
	pub fn as_str(&self) -> &str {
		let length = self.0.iter().position(|b| *b == 0).unwrap_or(4);
		self.0
			.get(..length)
			.and_then(|letters| core::str::from_utf8(letters).ok())
			.unwrap_or("")
	}
}

// BEGIN GENERATED RANGES (scripts/gen-unicode.py)
/// Start of each run of code points in one block, and the block's id (0 for any other).
#[rustfmt::skip]
pub(crate) const RANGES: [u32; 101] = [
	r(0x0000, 0), r(0x0080, 1), r(0x0100, 2), r(0x0180, 3), r(0x0250, 4),
	r(0x02B0, 5), r(0x0300, 6), r(0x0370, 7), r(0x0400, 8), r(0x0500, 9),
	r(0x0530, 10), r(0x0590, 11), r(0x0600, 12), r(0x0700, 13), r(0x0750, 0),
	r(0x0780, 14), r(0x07C0, 15), r(0x0800, 0), r(0x0900, 16), r(0x0980, 17),
	r(0x0A00, 18), r(0x0A80, 19), r(0x0B00, 20), r(0x0B80, 21), r(0x0C00, 22),
	r(0x0C80, 23), r(0x0D00, 24), r(0x0D80, 25), r(0x0E00, 26), r(0x0E80, 27),
	r(0x0F00, 28), r(0x1000, 29), r(0x10A0, 30), r(0x1100, 31), r(0x1200, 32),
	r(0x1380, 0), r(0x13A0, 33), r(0x1400, 34), r(0x1680, 0), r(0x1780, 35),
	r(0x1800, 36), r(0x18B0, 0), r(0x1D00, 37), r(0x1D80, 0), r(0x1E00, 38),
	r(0x1F00, 39), r(0x2000, 40), r(0x2070, 41), r(0x20A0, 42), r(0x20D0, 0),
	r(0x2100, 43), r(0x2150, 44), r(0x2190, 45), r(0x2200, 46), r(0x2300, 47),
	r(0x2400, 0), r(0x2460, 48), r(0x2500, 49), r(0x2580, 50), r(0x25A0, 51),
	r(0x2600, 52), r(0x2700, 53), r(0x27C0, 0), r(0x3000, 54), r(0x3040, 55),
	r(0x30A0, 56), r(0x3100, 57), r(0x3130, 58), r(0x3190, 0), r(0x3400, 59),
	r(0x4DC0, 0), r(0x4E00, 60), r(0xA000, 0), r(0xAC00, 61), r(0xD7B0, 0),
	r(0xF900, 62), r(0xFB00, 63), r(0xFB50, 64), r(0xFE00, 65), r(0xFE10, 0),
	r(0xFE70, 66), r(0xFF00, 67), r(0xFFF0, 0), r(0x1F000, 68), r(0x1F030, 0),
	r(0x1F0A0, 69), r(0x1F100, 70), r(0x1F200, 0), r(0x1F300, 71), r(0x1F600, 72),
	r(0x1F650, 0), r(0x1F680, 73), r(0x1F700, 0), r(0x1F900, 74), r(0x1FA00, 0),
	r(0x1FA70, 75), r(0x1FB00, 0), r(0x20000, 76), r(0x2A6E0, 0), r(0xE0000, 77),
	r(0xE0080, 0),
];
// END GENERATED RANGES

// BEGIN GENERATED NAMES (scripts/gen-unicode.py)
/// Each label after the first is a length byte and its letters; the first block is id 1.
pub(crate) const NAMES: &[u8] = concat!(
	"\x12Latin-1 Supplement",
	"\x10Latin Extended-A",
	"\x10Latin Extended-B",
	"\x0eIPA Extensions",
	"\x10Modifier Letters",
	"\x0fCombining Marks",
	"\x05Greek",
	"\x08Cyrillic",
	"\x13Cyrillic Supplement",
	"\x08Armenian",
	"\x06Hebrew",
	"\x06Arabic",
	"\x06Syriac",
	"\x06Thaana",
	"\x03NKo",
	"\x0aDevanagari",
	"\x07Bengali",
	"\x08Gurmukhi",
	"\x08Gujarati",
	"\x05Oriya",
	"\x05Tamil",
	"\x06Telugu",
	"\x07Kannada",
	"\x09Malayalam",
	"\x07Sinhala",
	"\x04Thai",
	"\x03Lao",
	"\x07Tibetan",
	"\x07Myanmar",
	"\x08Georgian",
	"\x0bHangul Jamo",
	"\x08Ethiopic",
	"\x08Cherokee",
	"\x12Canadian Syllabics",
	"\x05Khmer",
	"\x09Mongolian",
	"\x13Phonetic Extensions",
	"\x19Latin Extended Additional",
	"\x0eGreek Extended",
	"\x0bPunctuation",
	"\x14Super and Subscripts",
	"\x10Currency Symbols",
	"\x12Letterlike Symbols",
	"\x0cNumber Forms",
	"\x06Arrows",
	"\x0bMathematics",
	"\x09Technical",
	"\x16Enclosed Alphanumerics",
	"\x0bBox Drawing",
	"\x0eBlock Elements",
	"\x10Geometric Shapes",
	"\x07Symbols",
	"\x08Dingbats",
	"\x0fCJK Punctuation",
	"\x08Hiragana",
	"\x08Katakana",
	"\x08Bopomofo",
	"\x15Hangul Jamo (compat.)",
	"\x15CJK Ideographs Ext. A",
	"\x0eCJK Ideographs",
	"\x10Hangul Syllables",
	"\x16CJK Compat. Ideographs",
	"\x12Presentation Forms",
	"\x1bArabic Presentation Forms-A",
	"\x13Variation Selectors",
	"\x1bArabic Presentation Forms-B",
	"\x0fFullwidth Forms",
	"\x0dMahjong Tiles",
	"\x0dPlaying Cards",
	"\x18Enclosed Alphanumerics 2",
	"\x0bPictographs",
	"\x09Emoticons",
	"\x09Transport",
	"\x0dPictographs 2",
	"\x0dPictographs 3",
	"\x15CJK Ideographs Ext. B",
	"\x04Tags",
)
.as_bytes();
// END GENERATED NAMES

// BEGIN GENERATED SHORT (scripts/gen-unicode.py)
/// The short names, sorted by code point: two words each, the code point and four letters.
#[rustfmt::skip]
pub(crate) const SHORT: [u32; 24] = [
	0x00AD, 0x00594853, // SHY
	0x034F, 0x004A4743, // CGJ
	0x061C, 0x004D4C41, // ALM
	0x200B, 0x5053575A, // ZWSP
	0x200C, 0x4A4E575A, // ZWNJ
	0x200D, 0x004A575A, // ZWJ
	0x200E, 0x004D524C, // LRM
	0x200F, 0x004D4C52, // RLM
	0x2060, 0x00004A57, // WJ
	0xFE0E, 0x35315356, // VS15
	0xFE0F, 0x36315356, // VS16
	0xFEFF, 0x004D4F42, // BOM
];
// END GENERATED SHORT

/// The labels are well formed and the short names are whole words, sorted.
pub(crate) fn valid(names: &[u8], short: Seq<'_>) -> bool {
	let whole = match short {
		Seq::Words(_) => true,
		Seq::Bytes(b) => b.len() % 8 == 0,
	};
	let mut at = 0;
	while at < names.len() {
		let Some(length) = names.get(at) else {
			return false;
		};
		at += 1 + usize::from(*length);
	}
	let pairs = short.len() / 2;
	whole && at == names.len() && (1..pairs).all(|i| short.get(2 * (i - 1)) < short.get(2 * i))
}

pub(crate) fn block_in(table: Seq<'_>, cp: u32) -> Block {
	Block(lookup(table, cp))
}

pub(crate) fn name_in(names: &[u8], block: Block) -> &str {
	let mut at = 0;
	let mut id = 1;
	while let Some(length) = names.get(at).copied() {
		let end = at + 1 + usize::from(length);
		if id == block.0 {
			return names
				.get(at + 1..end)
				.and_then(|label| core::str::from_utf8(label).ok())
				.unwrap_or("Other");
		}
		at = end;
		id += 1;
	}
	"Other"
}

pub(crate) fn short_name_in(short: Seq<'_>, cp: u32) -> Option<ShortName> {
	let (mut low, mut high) = (0, short.len() / 2);
	while low < high {
		let middle = low + (high - low) / 2;
		let found = short.get(2 * middle);
		if found == cp {
			return Some(ShortName(short.get(2 * middle + 1).to_le_bytes()));
		}
		if found < cp {
			low = middle + 1;
		} else {
			high = middle;
		}
	}
	None
}

/// The block of `c`.
pub fn block(c: char) -> Block {
	block_in(Seq::Words(&RANGES), u32::from(c))
}

/// The label of a block: `Cyrillic`, `Pictographs`, `Other`.
pub fn name(block: Block) -> &'static str {
	name_in(NAMES, block)
}

/// The short name of an invisible character: `ZWJ` for U+200D, `BOM` for U+FEFF.
pub fn short_name(c: char) -> Option<ShortName> {
	short_name_in(Seq::Words(&SHORT), u32::from(c))
}
