// Scripts the v1 builds draw as placeholders, and the format characters that draw nothing (CORE-226).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Right-to-left and complex scripts are kept in logical order and drawn as placeholders: Hebrew, Arabic and its
//! supplements and presentation forms, Syriac, Thaana, NKo, Samaritan, Mandaic, the Indic blocks from Devanagari to
//! Sinhala, Thai, Lao, Tibetan, Myanmar, Khmer, Mongolian, and the bidirectional controls. The ranges are those blocks of
//! `Blocks.txt` (the generator names them), so a character of a block the plan does not list answers `false`.
//!
//! The invisible characters are the format characters and variation selectors that draw no ink and no advance: the soft
//! hyphen, the combining grapheme joiner, the zero width space, joiners, the bidirectional controls, the word joiner and
//! invisible operators, the byte order mark, variation selectors and tags.

use crate::unitab::{lookup, pack as r, Seq};

const SHAPING: u8 = 1;
const INVISIBLE: u8 = 2;

// BEGIN GENERATED RANGES (scripts/gen-unicode.py)
/// Start of each run of code points with one set of flags: bit 0 needs shaping or bidi, bit 1 is invisible.
#[rustfmt::skip]
pub(crate) const RANGES: [u32; 47] = [
	r(0x0000, 0), r(0x00AD, 2), r(0x00AE, 0), r(0x034F, 2), r(0x0350, 0),
	r(0x0590, 1), r(0x061C, 3), r(0x061D, 1), r(0x10A0, 0), r(0x1780, 1),
	r(0x180E, 3), r(0x180F, 1), r(0x18B0, 0), r(0x19E0, 1), r(0x1A00, 0),
	r(0x1CD0, 1), r(0x1D00, 0), r(0x200B, 2), r(0x200E, 3), r(0x2010, 0),
	r(0x202A, 3), r(0x202F, 0), r(0x2060, 2), r(0x2065, 0), r(0x2066, 3),
	r(0x206A, 2), r(0x2070, 0), r(0xA8E0, 1), r(0xA900, 0), r(0xA9E0, 1),
	r(0xAA00, 0), r(0xAA60, 1), r(0xAA80, 0), r(0xFB1D, 1), r(0xFE00, 2),
	r(0xFE10, 0), r(0xFE70, 1), r(0xFEFF, 3), r(0xFF00, 0), r(0x11660, 1),
	r(0x11680, 0), r(0xE0001, 2), r(0xE0002, 0), r(0xE0020, 2), r(0xE0080, 0),
	r(0xE0100, 2), r(0xE01F0, 0),
];
// END GENERATED RANGES

pub(crate) fn needs_shaping_or_bidi_in(table: Seq<'_>, cp: u32) -> bool {
	lookup(table, cp) & SHAPING != 0
}

pub(crate) fn is_invisible_in(table: Seq<'_>, cp: u32) -> bool {
	lookup(table, cp) & INVISIBLE != 0
}

/// Whether `c` is in a script a v1 build draws as a placeholder, or is a bidirectional control.
pub fn needs_shaping_or_bidi(c: char) -> bool {
	needs_shaping_or_bidi_in(Seq::Words(&RANGES), u32::from(c))
}

/// Whether `c` is a format character that draws nothing.
pub fn is_invisible(c: char) -> bool {
	is_invisible_in(Seq::Words(&RANGES), u32::from(c))
}
