// The standard 14 text fonts: which one a Quill font maps to, and the width of each Windows-1252 byte in it.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::widths;

/// A family of the standard 14 text fonts.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Family {
	/// Helvetica, which Arial matches advance for advance.
	Sans,
	/// Times, which Times New Roman matches advance for advance.
	Serif,
	/// Courier, which Courier New matches advance for advance.
	Mono,
}

/// One of the twelve text fonts among the standard 14 (Symbol and Zapf Dingbats are not text fonts).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Font {
	/// The family.
	pub family: Family,
	/// Bold.
	pub bold: bool,
	/// Italic, which Helvetica and Courier call oblique.
	pub italic: bool,
}

/// How many fonts there are, which is the width of a page's font mask.
pub const FONT_COUNT: usize = 12;

const NAMES: [&str; FONT_COUNT] = [
	"Helvetica",
	"Helvetica-Oblique",
	"Helvetica-Bold",
	"Helvetica-BoldOblique",
	"Times-Roman",
	"Times-Italic",
	"Times-Bold",
	"Times-BoldItalic",
	"Courier",
	"Courier-Oblique",
	"Courier-Bold",
	"Courier-BoldOblique",
];

/// Whether `haystack` holds `needle`, ignoring case (ASCII).
fn contains(haystack: &[u8], needle: &[u8]) -> bool {
	haystack
		.windows(needle.len())
		.any(|w| w.eq_ignore_ascii_case(needle))
}

impl Font {
	/// The font for a Quill font: a face name and the document's family code (0 any, 1 roman, 2 swiss, 3 modern, 4 script,
	/// 5 decorative) with the bold and italic switches. The name decides when it says what the face is like (`Courier New`,
	/// `Times New Roman`, `Arial`, and the usual neighbours); otherwise the family code does; a face that is neither is Helvetica,
	/// the plain choice. Only the glyph shapes are borrowed: where each character goes comes from the layout.
	pub fn pick(face: &[u8], family: u8, bold: bool, italic: bool) -> Font {
		const MONO: [&[u8]; 9] = [
			b"courier",
			b"monospace",
			b"consolas",
			b"lucida console",
			b"fixedsys",
			b"terminal",
			b"typewriter",
			b"andale",
			b"letter gothic",
		];
		const SERIF: [&[u8]; 12] = [
			b"times",
			b"serif",
			b"georgia",
			b"garamond",
			b"palatino",
			b"book antiqua",
			b"century",
			b"cambria",
			b"bookman",
			b"roman",
			b"minion",
			b"baskerville",
		];
		const SANS: [&[u8]; 9] = [
			b"arial",
			b"helvetica",
			b"sans",
			b"verdana",
			b"tahoma",
			b"calibri",
			b"segoe",
			b"trebuchet",
			b"univers",
		];
		let named = |list: &[&[u8]]| list.iter().any(|n| contains(face, n));
		let kind = if named(&MONO) {
			Family::Mono
		} else if named(&SANS) {
			Family::Sans
		} else if named(&SERIF) {
			Family::Serif
		} else {
			match family {
				1 => Family::Serif,
				3 => Family::Mono,
				_ => Family::Sans,
			}
		};
		Font {
			family: kind,
			bold,
			italic,
		}
	}

	/// The font's place among the twelve, which is its bit in a page's font mask.
	pub fn index(self) -> usize {
		let base = match self.family {
			Family::Sans => 0,
			Family::Serif => 4,
			Family::Mono => 8,
		};
		base + usize::from(self.bold) * 2 + usize::from(self.italic)
	}

	/// The font at place `index` of the twelve.
	pub fn from_index(index: usize) -> Font {
		Font {
			family: match index / 4 {
				0 => Family::Sans,
				1 => Family::Serif,
				_ => Family::Mono,
			},
			bold: index & 2 != 0,
			italic: index & 1 != 0,
		}
	}

	/// The font's PDF base font name.
	pub fn base_name(self) -> &'static str {
		NAMES.get(self.index()).copied().unwrap_or("Helvetica")
	}

	/// The advance of Windows-1252 byte `byte` in thousandths of an em, and 0 for a control character (which is never drawn).
	pub fn width(self, byte: u8) -> u16 {
		if byte < widths::FIRST {
			return 0;
		}
		let table: &[u16; 224] = match (self.family, self.bold, self.italic) {
			(Family::Mono, _, _) => return 600,
			(Family::Sans, false, _) => &widths::HELVETICA,
			(Family::Sans, true, _) => &widths::HELVETICA_BOLD,
			(Family::Serif, false, false) => &widths::TIMES,
			(Family::Serif, true, false) => &widths::TIMES_BOLD,
			(Family::Serif, false, true) => &widths::TIMES_ITALIC,
			(Family::Serif, true, true) => &widths::TIMES_BOLD_ITALIC,
		};
		table
			.get(usize::from(byte - widths::FIRST))
			.copied()
			.unwrap_or(0)
	}
}
