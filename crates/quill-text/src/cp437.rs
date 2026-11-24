// CP437 (the DOS screen's code page) to Unicode, and between CP437 and Windows-1252 bytes.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The tables here are the ones the text-mode screens use. They are checked against `quill-grid`'s byte for
//! byte until the grid adopts this crate.

const HIGH: [char; 128] = [
	'Ç', 'ü', 'é', 'â', 'ä', 'à', 'å', 'ç', 'ê', 'ë', 'è', 'ï', 'î', 'ì', 'Ä', 'Å', 'É', 'æ', 'Æ',
	'ô', 'ö', 'ò', 'û', 'ù', 'ÿ', 'Ö', 'Ü', '¢', '£', '¥', '₧', 'ƒ', 'á', 'í', 'ó', 'ú', 'ñ', 'Ñ',
	'ª', 'º', '¿', '⌐', '¬', '½', '¼', '¡', '«', '»', '░', '▒', '▓', '│', '┤', '╡', '╢', '╖', '╕',
	'╣', '║', '╗', '╝', '╜', '╛', '┐', '└', '┴', '┬', '├', '─', '┼', '╞', '╟', '╚', '╔', '╩', '╦',
	'╠', '═', '╬', '╧', '╨', '╤', '╥', '╙', '╘', '╒', '╓', '╫', '╪', '┘', '┌', '█', '▄', '▌', '▐',
	'▀', 'α', 'ß', 'Γ', 'π', 'Σ', 'σ', 'µ', 'τ', 'Φ', 'Θ', 'Ω', 'δ', '∞', 'φ', 'ε', '∩', '≡', '±',
	'≥', '≤', '⌠', '⌡', '÷', '≈', '°', '∙', '·', '√', 'ⁿ', '²', '■', '\u{a0}',
];

/// The Unicode character for a CP437 code. Codes below 0x20 show as spaces, so a dump holds no control characters.
pub fn to_char(b: u8) -> char {
	match b {
		0x07 => '•',
		0x20..=0x7E => char::from(b),
		0x80..=0xFF => HIGH.get(usize::from(b - 0x80)).copied().unwrap_or('?'),
		_ => ' ',
	}
}

/// The CP437 code for a Windows-1252 character, or the shaded block when CP437 has no form for it. Control codes show as
/// the shaded block too: this is the screen's conversion, for display. [`to_ansi`] is the one for files.
pub const fn from_ansi(c: u8) -> u8 {
	match c {
		0x20..=0x7E => c,
		0x95 => 0x07, // bullet
		0xA1 => 0xAD,
		0xA2 => 0x9B,
		0xA3 => 0x9C,
		0xA5 => 0x9D,
		0xAA => 0xA6,
		0xAB => 0xAE,
		0xAC => 0xAA,
		0xB0 => 0xF8,
		0xB1 => 0xF1,
		0xB2 => 0xFD,
		0xB5 => 0xE6,
		0xB7 => 0xFA,
		0xBA => 0xA7,
		0xBB => 0xAF,
		0xBC => 0xAC,
		0xBD => 0xAB,
		0xBF => 0xA8,
		0xC4 => 0x8E,
		0xC5 => 0x8F,
		0xC6 => 0x92,
		0xC7 => 0x80,
		0xC9 => 0x90,
		0xD1 => 0xA5,
		0xD6 => 0x99,
		0xDC => 0x9A,
		0xDF => 0xE1,
		0xE0 => 0x85,
		0xE1 => 0xA0,
		0xE2 => 0x83,
		0xE4 => 0x84,
		0xE5 => 0x86,
		0xE6 => 0x91,
		0xE7 => 0x87,
		0xE8 => 0x8A,
		0xE9 => 0x82,
		0xEA => 0x88,
		0xEB => 0x89,
		0xEC => 0x8D,
		0xED => 0xA1,
		0xEE => 0x8C,
		0xEF => 0x8B,
		0xF1 => 0xA4,
		0xF2 => 0x95,
		0xF3 => 0xA2,
		0xF4 => 0x93,
		0xF6 => 0x94,
		0xF7 => 0xF6,
		0xF9 => 0x97,
		0xFA => 0xA3,
		0xFB => 0x96,
		0xFC => 0x81,
		0xFF => 0x98,
		_ => 0xB1,
	}
}

/// The shaded block `from_ansi` returns for a character CP437 cannot show.
const SHADE: u8 = 0xB1;

/// Windows-1252 for CP437 codes 0x80 to 0xFF: the inverse of `from_ansi`, with `?` where Windows-1252 has no form.
const TO_ANSI: [u8; 128] = {
	let mut t = [b'?'; 128];
	let mut a: u16 = 0x80;
	while a <= 0xFF {
		let c = from_ansi(a as u8);
		if c >= 0x80 && c != SHADE {
			// `c - 0x80` is below 128 because `c` is a byte of at least 0x80.
			#[allow(clippy::indexing_slicing)]
			{
				t[(c - 0x80) as usize] = a as u8;
			}
		}
		a += 1;
	}
	t
};

/// The Windows-1252 byte for a CP437 byte read from a file: ASCII and control codes unchanged, the letters and
/// signs both code pages have converted, and `?` for the rest (box drawing, Greek and the like).
pub fn to_ansi(b: u8) -> u8 {
	match b {
		0x00..=0x7F => b,
		_ => TO_ANSI.get(usize::from(b - 0x80)).copied().unwrap_or(b'?'),
	}
}
