// quill-grid: text-mode screens as a grid of CP437 cells with attributes.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The grid is what DOS text mode shows and what the host's golden screens record, so one drawing routine gives both.

#![no_std]
#![deny(
	clippy::unwrap_used,
	clippy::expect_used,
	clippy::panic,
	clippy::indexing_slicing
)]

use core::fmt;

use quill_core::layout::{FontMetrics, FontRef, Measure, GLYPHS};

/// Columns of every grid.
pub const COLS: usize = 80;

/// One cell: a CP437 character and a VGA attribute (background in the high nibble).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Cell {
	/// CP437 character code.
	pub ch: u8,
	/// Attribute byte.
	pub attr: u8,
}

/// Attribute colours.
pub mod attr {
	/// Light grey on blue: regular text in the colour scheme.
	pub const TEXT: u8 = 0x17;
	/// Bright white on blue: bold.
	pub const BOLD: u8 = 0x1F;
	/// Light cyan on blue: italic.
	pub const ITALIC: u8 = 0x1B;
	/// Black on light grey: menus and the status line.
	pub const CHROME: u8 = 0x70;
	/// Black on light grey, selection.
	pub const SELECT: u8 = 0x70;
}

/// A screen of `ROWS` rows (25 or 50) of 80 columns.
#[derive(Clone)]
pub struct Grid<const ROWS: usize> {
	cells: [[Cell; COLS]; ROWS],
}

impl<const ROWS: usize> Grid<ROWS> {
	/// A grid of spaces in `attr`.
	pub const fn new(attr: u8) -> Self {
		Grid {
			cells: [[Cell { ch: b' ', attr }; COLS]; ROWS],
		}
	}

	/// The cell at (column, row).
	pub fn get(&self, x: usize, y: usize) -> Option<Cell> {
		self.cells.get(y).and_then(|r| r.get(x)).copied()
	}

	/// Sets one cell; outside the grid does nothing.
	pub fn put(&mut self, x: usize, y: usize, ch: u8, attr: u8) {
		if let Some(c) = self.cells.get_mut(y).and_then(|r| r.get_mut(x)) {
			*c = Cell { ch, attr };
		}
	}

	/// Writes CP437 bytes from (x, y), clipped at the right edge.
	pub fn text(&mut self, x: usize, y: usize, s: &[u8], attr: u8) {
		for (i, b) in s.iter().enumerate() {
			self.put(x + i, y, *b, attr);
		}
	}

	/// Fills a rectangle.
	pub fn fill(&mut self, x: usize, y: usize, w: usize, h: usize, ch: u8, attr: u8) {
		for yy in y..y + h {
			for xx in x..x + w {
				self.put(xx, yy, ch, attr);
			}
		}
	}

	/// Draws a single-line box.
	pub fn frame(&mut self, x: usize, y: usize, w: usize, h: usize, attr: u8) {
		if w < 2 || h < 2 {
			return;
		}
		for xx in x + 1..x + w - 1 {
			self.put(xx, y, 0xC4, attr);
			self.put(xx, y + h - 1, 0xC4, attr);
		}
		for yy in y + 1..y + h - 1 {
			self.put(x, yy, 0xB3, attr);
			self.put(x + w - 1, yy, 0xB3, attr);
		}
		self.put(x, y, 0xDA, attr);
		self.put(x + w - 1, y, 0xBF, attr);
		self.put(x, y + h - 1, 0xC0, attr);
		self.put(x + w - 1, y + h - 1, 0xD9, attr);
	}

	/// The rows, as 16-bit words (character in the low byte, attribute in the high byte), for B800h.
	pub fn words(&self) -> impl Iterator<Item = u16> + '_ {
		self.cells
			.iter()
			.flatten()
			.map(|c| u16::from(c.ch) | (u16::from(c.attr) << 8))
	}

	/// Fills the grid from 16-bit words as `words` gives them (the screen read back from video memory).
	pub fn load_words(&mut self, mut next: impl FnMut(usize) -> u16) {
		let mut i = 0;
		for row in self.cells.iter_mut() {
			for c in row.iter_mut() {
				let w = next(i);
				*c = Cell {
					ch: w as u8,
					attr: (w >> 8) as u8,
				};
				i += 1;
			}
		}
	}

	/// Writes the screen as a golden frame: `=== screen N ===`, the rows as UTF-8 text, then `--- attributes ---` and
	/// the attributes as two hex digits per cell.
	pub fn dump<W: fmt::Write>(&self, n: u32, w: &mut W) -> fmt::Result {
		writeln!(w, "=== screen {n} ===")?;
		for row in &self.cells {
			for c in row {
				w.write_char(cp437(c.ch))?;
			}
			w.write_char('\n')?;
		}
		writeln!(w, "--- attributes ---")?;
		for row in &self.cells {
			for c in row {
				write!(w, "{:02X}", c.attr)?;
			}
			w.write_char('\n')?;
		}
		Ok(())
	}
}

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
pub fn cp437(b: u8) -> char {
	match b {
		0x20..=0x7E => char::from(b),
		0x80..=0xFF => HIGH.get(usize::from(b - 0x80)).copied().unwrap_or('?'),
		_ => ' ',
	}
}

/// The T4 hello screen: a menu bar, a blue page with regular, bold and italic text, a box and a status line.
pub fn hello<const ROWS: usize>() -> Grid<ROWS> {
	let mut g = Grid::<ROWS>::new(attr::TEXT);
	g.fill(0, 0, COLS, 1, b' ', attr::CHROME);
	g.text(
		1,
		0,
		b"File  Edit  Insert  Find  Character  Paragraph  Document  Help",
		attr::CHROME,
	);
	g.text(2, 3, b"Quill for DOS", attr::BOLD);
	g.text(
		2,
		5,
		b"One word processor for DOS, Windows 3.1 and Windows 95.",
		attr::TEXT,
	);
	g.text(2, 6, b"Bold is bright white, ", attr::TEXT);
	g.text(24, 6, b"italic is cyan", attr::ITALIC);
	g.text(38, 6, b".", attr::TEXT);
	g.frame(2, 8, 40, 5, attr::TEXT);
	g.text(4, 10, b"\xB0\xB1\xB2 shaded box \xDB\xDF\xDC", attr::TEXT);
	let last = ROWS.saturating_sub(1);
	g.fill(0, last, COLS, 1, b' ', attr::CHROME);
	g.text(
		1,
		last,
		b"HELLO.QLD   Pg 1 Ln 1 Col 1   Times New Roman 12",
		attr::CHROME,
	);
	g
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn clipped_writes_do_not_panic() {
		let mut g = Grid::<25>::new(0x07);
		g.text(78, 24, b"abcdef", 0x07);
		g.put(200, 200, b'x', 0x07);
		assert_eq!(g.get(79, 24).map(|c| c.ch), Some(b'b'));
		assert_eq!(g.get(80, 0), None);
	}
}

/// The CP437 code for a Windows ANSI character (code page 1252), or the shaded block when CP437 has no form for it.
pub fn ansi_to_cp437(c: u8) -> u8 {
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

/// DOS text mode: every character 1/10 inch wide and every line 1/6 inch tall (a 12-point typewriter), so a unit across
/// is one column and a unit down is one row.
#[derive(Copy, Clone, Debug, Default)]
pub struct GridMeasure;

impl Measure for GridMeasure {
	fn resolution(&self) -> (u16, u16) {
		(10, 6)
	}

	fn metrics(&mut self, _font: &FontRef) -> FontMetrics {
		FontMetrics {
			ascent: 1,
			descent: 0,
			leading: 0,
		}
	}

	fn widths(&mut self, _font: &FontRef, out: &mut [u16; GLYPHS]) {
		out.fill(1);
	}
}
