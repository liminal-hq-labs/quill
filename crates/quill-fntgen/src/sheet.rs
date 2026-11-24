// Proof sheets: text set with the strikes of a QUILL.FNT, as the DOS shell will draw it, written as PNG.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use tiny_skia::Pixmap;

use crate::faceset::{cp1252, FACE_NAMES, STYLE_NAMES};
use crate::fnt::Font;
use crate::render::{pixel, POINT_SIZES};

/// Width of a sheet in screen pixels: a 640 pixel mode 12h screen.
pub const SHEET_W: u32 = 640;

/// The sample text of a size: a short pangram and figures that fit the strike's width.
const SAMPLE: &str = "Hamburgefonstiv 0123 The quick brown fox jumps over \u{c9}t\u{e9} \u{e7}a";

/// The colours of the four levels of a 2-bit pixel (and 0 and 3 of a 1-bit one): white, silver, grey and black.
const LEVELS: [[u8; 3]; 4] = [[255, 255, 255], [192, 192, 192], [128, 128, 128], [0, 0, 0]];

fn to_byte(c: char) -> u8 {
	(0x20..=0xFFu8).find(|b| cp1252(*b) == c).unwrap_or(b'?')
}

/// A white canvas of `SHEET_W` pixels across, grown as lines are drawn.
pub struct Canvas {
	rows: Vec<Vec<u8>>,
}

impl Canvas {
	/// An empty canvas.
	pub fn new() -> Canvas {
		Canvas { rows: Vec::new() }
	}

	fn ensure(&mut self, h: usize) {
		while self.rows.len() < h {
			self.rows.push(vec![255; SHEET_W as usize * 3]);
		}
	}

	/// Sets a pixel when it is on the canvas.
	pub fn put(&mut self, x: i32, y: i32, c: [u8; 3]) {
		if x < 0 || y < 0 || x >= SHEET_W as i32 {
			return;
		}
		self.ensure(y as usize + 1);
		self.rows[y as usize][x as usize * 3..][..3].copy_from_slice(&c);
	}

	/// Draws `text` with strike `s` of `font`, pen at `x` and baseline at `base`, clipped at the right edge. Returns the pen's end.
	pub fn text(&mut self, font: &Font, s: usize, x: i32, base: i32, text: &str) -> i32 {
		let e = &font.entries[s];
		let mut pen = x;
		for ch in text.chars() {
			let i = usize::from(to_byte(ch) - 0x20);
			let g = font.glyph(s, i);
			for gy in 0..u32::from(g.h) {
				for gx in 0..u32::from(g.w) {
					let v = pixel(&g, e.bpp, gx, gy);
					if v != 0 {
						let c = if e.bpp == 1 {
							LEVELS[3]
						} else {
							LEVELS[usize::from(v)]
						};
						self.put(
							pen + i32::from(g.x) + gx as i32,
							base - i32::from(g.y) + gy as i32,
							c,
						);
					}
				}
			}
			pen += i32::from(font.advance(s, i));
			if pen >= SHEET_W as i32 {
				break;
			}
		}
		pen
	}

	/// A thin grey rule across the sheet.
	pub fn rule(&mut self, y: i32) {
		for x in 0..SHEET_W as i32 {
			self.put(x, y, [192, 192, 192]);
		}
	}

	/// The canvas enlarged `scale` times by pixel repetition, as a PNG, with `pad` rows of white below.
	pub fn png(mut self, scale: u32, pad: usize) -> Vec<u8> {
		let h = self.rows.len() + pad;
		self.ensure(h);
		let mut pix = Pixmap::new(SHEET_W * scale, h as u32 * scale).expect("a sheet");
		let data = pix.data_mut();
		for (y, row) in self.rows.iter().enumerate() {
			for x in 0..SHEET_W as usize {
				for dy in 0..scale as usize {
					for dx in 0..scale as usize {
						let at = (((y * scale as usize + dy) * SHEET_W as usize * scale as usize)
							+ x * scale as usize + dx)
							* 4;
						data[at..at + 3].copy_from_slice(&row[x * 3..][..3]);
						data[at + 3] = 255;
					}
				}
			}
		}
		pix.encode_png().expect("png")
	}
}

impl Default for Canvas {
	fn default() -> Self {
		Self::new()
	}
}

/// The strike used for headings: Sans regular, the largest size up to 12 pt the file has, else the first strike.
fn heading_strike(font: &Font) -> usize {
	font.find(1, 0, 24)
		.or_else(|| font.find(1, 0, 20))
		.unwrap_or(0)
}

/// Every face's regular style at every size, one line each: the sheet to compare sizes by eye.
pub fn size_sheet(font: &Font, title: &str) -> Vec<u8> {
	let mut c = Canvas::new();
	let hs = heading_strike(font);
	let mut y = 4;
	c.text(font, hs, 4, y + i32::from(font.entries[hs].ascent), title);
	y += i32::from(font.entries[hs].ascent) + i32::from(font.entries[hs].descent) + 6;
	for face in 0..3u8 {
		c.rule(y);
		y += 3;
		let name = FACE_NAMES[usize::from(face)];
		c.text(font, hs, 4, y + i32::from(font.entries[hs].ascent), name);
		y += i32::from(font.entries[hs].ascent) + i32::from(font.entries[hs].descent) + 3;
		for pt in POINT_SIZES {
			let Some(s) = font.find(face, 0, pt * 2) else {
				continue;
			};
			let e = &font.entries[s];
			y += i32::from(e.ascent);
			let x = c.text(font, s, 4, y, &format!("{pt} pt "));
			c.text(font, s, x, y, SAMPLE);
			y += i32::from(e.descent) + i32::from(e.leading) + 3;
		}
	}
	c.png(2, 8)
}

/// All four styles of every face at 10, 12 and 18 pt: the sheet for bold and italic.
pub fn style_sheet(font: &Font, title: &str) -> Vec<u8> {
	let mut c = Canvas::new();
	let hs = heading_strike(font);
	let mut y = 4;
	c.text(font, hs, 4, y + i32::from(font.entries[hs].ascent), title);
	y += i32::from(font.entries[hs].ascent) + i32::from(font.entries[hs].descent) + 6;
	for face in 0..3u8 {
		c.rule(y);
		y += 3;
		for pt in [10u16, 12, 18] {
			for style in 0..4u8 {
				let Some(s) = font.find(face, style, pt * 2) else {
					continue;
				};
				let e = &font.entries[s];
				y += i32::from(e.ascent);
				let label = format!(
					"{} {} {pt} pt ",
					FACE_NAMES[usize::from(face)],
					STYLE_NAMES[usize::from(style)]
				);
				let x = c.text(font, s, 4, y, &label);
				c.text(font, s, x, y, "Hamburgefonstiv 0123 fox");
				y += i32::from(e.descent) + i32::from(e.leading) + 2;
			}
		}
	}
	c.png(2, 8)
}
