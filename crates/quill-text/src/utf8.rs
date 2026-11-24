// Reading text as UTF-8 when it is valid and as Windows-1252 otherwise, and writing Windows-1252 text as UTF-8.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The C4 rule: a text file whose bytes are valid UTF-8 is read as UTF-8; any other file is Windows-1252. Text
//! that is only ASCII is both, and is read as Windows-1252. The document holds Windows-1252 bytes, so a character
//! with no Windows-1252 form is refused: it becomes `?` and is counted, so the caller can tell the user.
//!
//! The rule cannot tell Windows-1252 text that happens to be valid UTF-8 (`Ã©`, bytes C3 A9) from UTF-8 (`é`).

use crate::cp1252;

/// The UTF-8 byte order mark. A file that starts with it, and is valid UTF-8, loses it on reading.
pub const BOM: [u8; 3] = [0xEF, 0xBB, 0xBF];

/// How many characters had no Windows-1252 form and were replaced by `?`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Refused {
	pub count: usize,
}

/// What [`decode_1252`] found.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Decoded {
	/// The input was valid UTF-8 and held a multi-byte character or a byte order mark. False for ASCII-only
	/// input and for Windows-1252 text.
	pub utf8: bool,
	/// The characters that could not be kept. Always zero for Windows-1252 text.
	pub refused: Refused,
}

/// Turns the bytes of a text file into Windows-1252 bytes, passed one at a time to `put`.
///
/// Valid UTF-8 is converted character by character; anything else is already Windows-1252 and passes through
/// unchanged. The output is never longer than the input.
pub fn decode_1252(src: &[u8], mut put: impl FnMut(u8)) -> Decoded {
	let Ok(text) = core::str::from_utf8(src) else {
		for &b in src {
			put(b);
		}
		return Decoded::default();
	};
	let body = text.strip_prefix('\u{FEFF}').unwrap_or(text);
	let mut out = Decoded {
		utf8: !text.is_ascii(),
		refused: Refused::default(),
	};
	for c in body.chars() {
		match cp1252::from_char(c) {
			Some(b) => put(b),
			None => {
				out.refused.count = out.refused.count.saturating_add(1);
				put(b'?');
			}
		}
	}
	out
}

/// Writes Windows-1252 bytes as UTF-8, passed one byte at a time to `put`. Every byte has a character, so
/// nothing is refused. No byte order mark is written.
pub fn encode_1252(src: &[u8], mut put: impl FnMut(u8)) {
	let mut buf = [0u8; 4];
	for &b in src {
		for &u in cp1252::to_char(b).encode_utf8(&mut buf).as_bytes() {
			put(u);
		}
	}
}
