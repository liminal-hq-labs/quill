// Simple case folding for the alphabetic scripts (CORE-217).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `CaseFolding.txt` statuses C and S, for Latin (all blocks, and the fullwidth letters), Greek, Cyrillic, Armenian,
//! Georgian and Cherokee, as runs. Find with Match case off folds both sides, and spelling folds to look a word up.
//! `ß` folds to itself (full folding to `ss` is later) and the Turkish dotless and dotted I fold by the default rule.
//!
//! A run is two words. The first is `start << 8 | (length - 1)` (so `r(start, length - 1)`); the second is `kind << 24` and a 24-bit signed delta.
//! Kind 0 adds the delta to every code point of the run (`A` to `Z` add 32); kind 1 adds it to every other one, the first
//! included (Latin Extended-A pairs a capital and a small letter side by side).

use crate::unitab::{pack as r, Seq};

/// The second word of a run: its kind and its delta.
const fn d(kind: u32, delta: i32) -> u32 {
	(kind << 24) | (delta as u32 & 0x00FF_FFFF)
}

// BEGIN GENERATED RUNS (scripts/gen-unicode.py)
/// Runs of simple case folding, two words each: where it starts, how long it is, its kind and the delta.
#[rustfmt::skip]
pub(crate) const RUNS: [u32; 388] = [
	r(0x0041, 25), d(0, 32), r(0x00B5, 0), d(0, 775),
	r(0x00C0, 22), d(0, 32), r(0x00D8, 6), d(0, 32),
	r(0x0100, 46), d(1, 1), r(0x0132, 4), d(1, 1),
	r(0x0139, 14), d(1, 1), r(0x014A, 44), d(1, 1),
	r(0x0178, 0), d(0, -121), r(0x0179, 4), d(1, 1),
	r(0x017F, 0), d(0, -268), r(0x0181, 0), d(0, 210),
	r(0x0182, 2), d(1, 1), r(0x0186, 0), d(0, 206),
	r(0x0187, 0), d(0, 1), r(0x0189, 1), d(0, 205),
	r(0x018B, 0), d(0, 1), r(0x018E, 0), d(0, 79),
	r(0x018F, 0), d(0, 202), r(0x0190, 0), d(0, 203),
	r(0x0191, 0), d(0, 1), r(0x0193, 0), d(0, 205),
	r(0x0194, 0), d(0, 207), r(0x0196, 0), d(0, 211),
	r(0x0197, 0), d(0, 209), r(0x0198, 0), d(0, 1),
	r(0x019C, 0), d(0, 211), r(0x019D, 0), d(0, 213),
	r(0x019F, 0), d(0, 214), r(0x01A0, 4), d(1, 1),
	r(0x01A6, 0), d(0, 218), r(0x01A7, 0), d(0, 1),
	r(0x01A9, 0), d(0, 218), r(0x01AC, 0), d(0, 1),
	r(0x01AE, 0), d(0, 218), r(0x01AF, 0), d(0, 1),
	r(0x01B1, 1), d(0, 217), r(0x01B3, 2), d(1, 1),
	r(0x01B7, 0), d(0, 219), r(0x01B8, 0), d(0, 1),
	r(0x01BC, 0), d(0, 1), r(0x01C4, 0), d(0, 2),
	r(0x01C5, 0), d(0, 1), r(0x01C7, 0), d(0, 2),
	r(0x01C8, 0), d(0, 1), r(0x01CA, 0), d(0, 2),
	r(0x01CB, 16), d(1, 1), r(0x01DE, 16), d(1, 1),
	r(0x01F1, 0), d(0, 2), r(0x01F2, 2), d(1, 1),
	r(0x01F6, 0), d(0, -97), r(0x01F7, 0), d(0, -56),
	r(0x01F8, 38), d(1, 1), r(0x0220, 0), d(0, -130),
	r(0x0222, 16), d(1, 1), r(0x023A, 0), d(0, 10795),
	r(0x023B, 0), d(0, 1), r(0x023D, 0), d(0, -163),
	r(0x023E, 0), d(0, 10792), r(0x0241, 0), d(0, 1),
	r(0x0243, 0), d(0, -195), r(0x0244, 0), d(0, 69),
	r(0x0245, 0), d(0, 71), r(0x0246, 8), d(1, 1),
	r(0x0345, 0), d(0, 116), r(0x0370, 2), d(1, 1),
	r(0x0376, 0), d(0, 1), r(0x037F, 0), d(0, 116),
	r(0x0386, 0), d(0, 38), r(0x0388, 2), d(0, 37),
	r(0x038C, 0), d(0, 64), r(0x038E, 1), d(0, 63),
	r(0x0391, 16), d(0, 32), r(0x03A3, 8), d(0, 32),
	r(0x03C2, 0), d(0, 1), r(0x03CF, 0), d(0, 8),
	r(0x03D0, 0), d(0, -30), r(0x03D1, 0), d(0, -25),
	r(0x03D5, 0), d(0, -15), r(0x03D6, 0), d(0, -22),
	r(0x03D8, 8), d(1, 1), r(0x03F0, 0), d(0, -54),
	r(0x03F1, 0), d(0, -48), r(0x03F4, 0), d(0, -60),
	r(0x03F5, 0), d(0, -64), r(0x03F7, 0), d(0, 1),
	r(0x03F9, 0), d(0, -7), r(0x03FA, 0), d(0, 1),
	r(0x03FD, 2), d(0, -130), r(0x0400, 15), d(0, 80),
	r(0x0410, 31), d(0, 32), r(0x0460, 32), d(1, 1),
	r(0x048A, 52), d(1, 1), r(0x04C0, 0), d(0, 15),
	r(0x04C1, 12), d(1, 1), r(0x04D0, 94), d(1, 1),
	r(0x0531, 37), d(0, 48), r(0x10A0, 37), d(0, 7264),
	r(0x10C7, 0), d(0, 7264), r(0x10CD, 0), d(0, 7264),
	r(0x13F8, 5), d(0, -8), r(0x1C80, 0), d(0, -6222),
	r(0x1C81, 0), d(0, -6221), r(0x1C82, 0), d(0, -6212),
	r(0x1C83, 1), d(0, -6210), r(0x1C85, 0), d(0, -6211),
	r(0x1C86, 0), d(0, -6204), r(0x1C87, 0), d(0, -6180),
	r(0x1C88, 0), d(0, 35267), r(0x1C89, 0), d(0, 1),
	r(0x1C90, 42), d(0, -3008), r(0x1CBD, 2), d(0, -3008),
	r(0x1E00, 148), d(1, 1), r(0x1E9B, 0), d(0, -58),
	r(0x1E9E, 0), d(0, -7615), r(0x1EA0, 94), d(1, 1),
	r(0x1F08, 7), d(0, -8), r(0x1F18, 5), d(0, -8),
	r(0x1F28, 7), d(0, -8), r(0x1F38, 7), d(0, -8),
	r(0x1F48, 5), d(0, -8), r(0x1F59, 6), d(1, -8),
	r(0x1F68, 7), d(0, -8), r(0x1F88, 7), d(0, -8),
	r(0x1F98, 7), d(0, -8), r(0x1FA8, 7), d(0, -8),
	r(0x1FB8, 1), d(0, -8), r(0x1FBA, 1), d(0, -74),
	r(0x1FBC, 0), d(0, -9), r(0x1FBE, 0), d(0, -7173),
	r(0x1FC8, 3), d(0, -86), r(0x1FCC, 0), d(0, -9),
	r(0x1FD3, 0), d(0, -7235), r(0x1FD8, 1), d(0, -8),
	r(0x1FDA, 1), d(0, -100), r(0x1FE3, 0), d(0, -7219),
	r(0x1FE8, 1), d(0, -8), r(0x1FEA, 1), d(0, -112),
	r(0x1FEC, 0), d(0, -7), r(0x1FF8, 1), d(0, -128),
	r(0x1FFA, 1), d(0, -126), r(0x1FFC, 0), d(0, -9),
	r(0x2126, 0), d(0, -7517), r(0x212A, 0), d(0, -8383),
	r(0x212B, 0), d(0, -8262), r(0x2132, 0), d(0, 28),
	r(0x2160, 15), d(0, 16), r(0x2183, 0), d(0, 1),
	r(0x2C60, 0), d(0, 1), r(0x2C62, 0), d(0, -10743),
	r(0x2C63, 0), d(0, -3814), r(0x2C64, 0), d(0, -10727),
	r(0x2C67, 4), d(1, 1), r(0x2C6D, 0), d(0, -10780),
	r(0x2C6E, 0), d(0, -10749), r(0x2C6F, 0), d(0, -10783),
	r(0x2C70, 0), d(0, -10782), r(0x2C72, 0), d(0, 1),
	r(0x2C75, 0), d(0, 1), r(0x2C7E, 1), d(0, -10815),
	r(0xA640, 44), d(1, 1), r(0xA680, 26), d(1, 1),
	r(0xA722, 12), d(1, 1), r(0xA732, 60), d(1, 1),
	r(0xA779, 2), d(1, 1), r(0xA77D, 0), d(0, -35332),
	r(0xA77E, 8), d(1, 1), r(0xA78B, 0), d(0, 1),
	r(0xA78D, 0), d(0, -42280), r(0xA790, 2), d(1, 1),
	r(0xA796, 18), d(1, 1), r(0xA7AA, 0), d(0, -42308),
	r(0xA7AB, 0), d(0, -42319), r(0xA7AC, 0), d(0, -42315),
	r(0xA7AD, 0), d(0, -42305), r(0xA7AE, 0), d(0, -42308),
	r(0xA7B0, 0), d(0, -42258), r(0xA7B1, 0), d(0, -42282),
	r(0xA7B2, 0), d(0, -42261), r(0xA7B3, 0), d(0, 928),
	r(0xA7B4, 14), d(1, 1), r(0xA7C4, 0), d(0, -48),
	r(0xA7C5, 0), d(0, -42307), r(0xA7C6, 0), d(0, -35384),
	r(0xA7C7, 2), d(1, 1), r(0xA7CB, 0), d(0, -42343),
	r(0xA7CC, 0), d(0, 1), r(0xA7D0, 0), d(0, 1),
	r(0xA7D6, 4), d(1, 1), r(0xA7DC, 0), d(0, -42561),
	r(0xA7F5, 0), d(0, 1), r(0xAB70, 79), d(0, -38864),
	r(0xFB05, 0), d(0, 1), r(0xFF21, 25), d(0, 32),
];
// END GENERATED RUNS

/// A table's runs are whole, in order and apart.
pub(crate) fn valid(table: Seq<'_>) -> bool {
	let whole = match table {
		Seq::Words(_) => true,
		Seq::Bytes(b) => b.len() % 8 == 0,
	};
	if !whole {
		return false;
	}
	let runs = table.len() / 2;
	(0..runs).all(|i| {
		let first = table.get(2 * i);
		let end = (first >> 8) + (first & 0xFF) + 1;
		let kind = table.get(2 * i + 1) >> 24;
		kind <= 1
			&& end <= crate::unitab::LAST + 1
			&& (i + 1 == runs || end <= table.get(2 * i + 2) >> 8)
	})
}

pub(crate) fn fold_in(table: Seq<'_>, cp: u32) -> u32 {
	// The last run that starts at or before `cp`.
	let (mut low, mut high) = (0, table.len() / 2);
	while low < high {
		let middle = low + (high - low) / 2;
		if table.get(2 * middle) >> 8 <= cp {
			low = middle + 1;
		} else {
			high = middle;
		}
	}
	let Some(run) = low.checked_sub(1) else {
		return cp;
	};
	let (first, second) = (table.get(2 * run), table.get(2 * run + 1));
	let offset = cp - (first >> 8);
	if offset > (first & 0xFF) || (second >> 24 == 1 && offset % 2 == 1) {
		return cp;
	}
	// The delta is 24 bits, signed.
	let delta = ((second & 0x00FF_FFFF) << 8) as i32 >> 8;
	cp.checked_add_signed(delta).unwrap_or(cp)
}

/// The simple case folding of `c`, or `c` itself.
pub fn fold(c: char) -> char {
	char::from_u32(fold_in(Seq::Words(&RUNS), u32::from(c))).unwrap_or(c)
}
