// A QUILL.FNT writer for tests: the bytes of a font built in memory, with no checks, so damaged files can be made from it too.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::vec::Vec;

use crate::{crc32, BOXES_LEN, DIR_LEN, HEADER_LEN, MAGIC, VERSION};
use quill_core::layout::GLYPHS;

/// One glyph: its box and its packed bitmap (rows of `ceil(width * bpp / 8)` bytes, the leftmost pixel in the top bits).
#[derive(Clone, Debug, Default)]
pub struct GlyphSpec {
	/// Pixels from the pen to the left edge.
	pub left: i8,
	/// Pixels from the baseline to the top row.
	pub top: i8,
	/// Width in pixels.
	pub width: u8,
	/// Height in pixels.
	pub height: u8,
	/// The packed rows.
	pub bits: Vec<u8>,
}

/// One strike.
#[derive(Clone, Debug)]
pub struct StrikeSpec {
	/// The face: 0 serif, 1 swiss, 2 modern.
	pub face: u8,
	/// Bit 0 bold, bit 1 italic.
	pub style: u8,
	/// Half points.
	pub size: u16,
	/// Em height in pixels.
	pub pixels: u8,
	/// Pixels above the baseline.
	pub ascent: u8,
	/// Pixels below the baseline.
	pub descent: u8,
	/// External leading.
	pub leading: u8,
	/// Bits per pixel: 1 or 2.
	pub bpp: u8,
	/// The advance of each of the 224 characters.
	pub advances: [u8; GLYPHS],
	/// The glyphs that have ink, by character.
	pub glyphs: Vec<(u8, GlyphSpec)>,
}

/// The bytes of a file holding `strikes`, which the caller keeps sorted by face, style and size. Nothing is checked, so damaged
/// files can be made from it.
pub fn build(strikes: &[StrikeSpec]) -> Vec<u8> {
	let index_len = HEADER_LEN + (DIR_LEN + GLYPHS) * strikes.len();
	let mut blocks: Vec<Vec<u8>> = Vec::new();
	for s in strikes {
		let mut boxes = std::vec![0u8; BOXES_LEN];
		let mut bitmaps: Vec<u8> = Vec::new();
		for (ch, g) in &s.glyphs {
			let at = usize::from(ch - 0x20) * 6;
			let off = bitmaps.len() as u16;
			boxes[at] = g.left as u8;
			boxes[at + 1] = g.top as u8;
			boxes[at + 2] = g.width;
			boxes[at + 3] = g.height;
			boxes[at + 4..at + 6].copy_from_slice(&off.to_le_bytes());
			bitmaps.extend_from_slice(&g.bits);
		}
		boxes.extend_from_slice(&bitmaps);
		blocks.push(boxes);
	}
	let total = index_len + blocks.iter().map(Vec::len).sum::<usize>();
	let mut out = Vec::with_capacity(total);
	out.extend_from_slice(&MAGIC);
	out.extend_from_slice(&VERSION.to_le_bytes());
	out.extend_from_slice(&(strikes.len() as u16).to_le_bytes());
	out.extend_from_slice(&(GLYPHS as u16).to_le_bytes());
	out.extend_from_slice(&[0x20, DIR_LEN as u8]);
	out.extend_from_slice(&(HEADER_LEN as u32).to_le_bytes());
	out.extend_from_slice(&(total as u32).to_le_bytes());
	out.extend_from_slice(&[0; 4]); // the checksum, filled in below
	out.extend_from_slice(&[0; 8]);
	let adv_base = HEADER_LEN + DIR_LEN * strikes.len();
	let mut at = index_len;
	for (i, (s, b)) in strikes.iter().zip(&blocks).enumerate() {
		out.extend_from_slice(&[s.face, s.style]);
		out.extend_from_slice(&s.size.to_le_bytes());
		out.extend_from_slice(&[s.pixels, s.ascent, s.descent, s.leading, s.bpp, 0, 0, 0]);
		out.extend_from_slice(&((adv_base + GLYPHS * i) as u32).to_le_bytes());
		out.extend_from_slice(&(at as u32).to_le_bytes());
		out.extend_from_slice(&(b.len() as u32).to_le_bytes());
		at += b.len();
	}
	for s in strikes {
		out.extend_from_slice(&s.advances);
	}
	for b in &blocks {
		out.extend_from_slice(b);
	}
	let sum = crc32(&out[HEADER_LEN..]);
	out[20..24].copy_from_slice(&sum.to_le_bytes());
	out
}
