// quill-strike: the QUILL.FNT reader, the strike measure and the strike choice.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A strike is one face, style and size drawn into bitmaps. `QUILL.FNT` holds many; [`Strikes::parse`] checks the index of the file
//! once, and after that the advances, the metrics and [`Strikes::strike_for`] cannot fail. [`StrikeMeasure`] lays text out with the
//! same integers the painter draws with. The byte layout is in `docs/dos-fnt-format.md`.

#![no_std]
#![deny(
	clippy::unwrap_used,
	clippy::expect_used,
	clippy::panic,
	clippy::indexing_slicing,
	clippy::unreachable,
	clippy::todo
)]

#[cfg(feature = "builder")]
extern crate std;

#[cfg(feature = "builder")]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
pub mod builder;
#[cfg(feature = "builder")]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
pub mod testfont;

use quill_core::doc::cflag;
use quill_core::layout::{FontMetrics, FontRef, Measure, GLYPHS};

/// The first four bytes of a `QUILL.FNT`.
pub const MAGIC: [u8; 4] = *b"QFNT";
/// The format version this reader understands.
pub const VERSION: u16 = 1;
/// Bytes in the header.
pub const HEADER_LEN: usize = 32;
/// Bytes in a directory entry.
pub const DIR_LEN: usize = 24;
/// Bytes in one box of a strike's box table.
pub const BOX_LEN: usize = 6;
/// Bytes in a strike's box table: one box for each of the 224 characters.
pub const BOXES_LEN: usize = GLYPHS * BOX_LEN;
/// The most bytes a strike's data (boxes and bitmaps) takes: one `Store` chunk.
pub const MAX_DATA_LEN: u32 = 32768;
/// The most strikes a file holds.
pub const MAX_STRIKES: usize = 512;
/// The tallest em height of a strike, in pixels.
pub const MAX_PIXELS: u8 = 64;
/// The widest and tallest glyph bitmap, in pixels.
pub const MAX_GLYPH_SIDE: usize = 64;
/// The most bytes one glyph bitmap takes: 64 rows of 64 pixels at 2 bits.
pub const MAX_GLYPH_BYTES: usize = MAX_GLYPH_SIDE * (MAX_GLYPH_SIDE * 2 / 8);
/// The scales a strike is drawn at.
pub const MAX_SCALE: u8 = 3;

/// The em height in pixels a font asks for at 96 dots to the inch: the formula the Windows build gives `CreateFontA`.
pub fn wanted_px(f: &FontRef) -> u32 {
	(u32::from(f.size) * 96 + 72) / 144
}

/// CRC-32 (the zlib polynomial) of `data`, continuing from `crc` (start with 0).
pub fn crc32_update(crc: u32, data: &[u8]) -> u32 {
	let mut c = !crc;
	for b in data {
		c ^= u32::from(*b);
		for _ in 0..8 {
			c = if c & 1 != 0 {
				0xEDB8_8320 ^ (c >> 1)
			} else {
				c >> 1
			};
		}
	}
	!c
}

/// CRC-32 of `data`.
pub fn crc32(data: &[u8]) -> u32 {
	crc32_update(0, data)
}

/// Why a file was refused.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Error {
	/// The bytes given end before the index does.
	Truncated,
	/// Not a `QUILL.FNT`, or a version this reader does not know.
	BadMagic,
	/// A count, length or reserved field in the header is wrong.
	BadHeader,
	/// A directory entry is wrong, or out of order.
	BadStrike,
	/// A strike's data lies outside the file or overlaps another's.
	Range,
}

/// Where the bytes of a strike's data come from: a file the shell reads with a seek, or a slice on the host.
pub trait StrikeSource {
	/// Reads up to `out.len()` bytes starting at `offset` of the file, and returns how many were read.
	fn read_at(&mut self, offset: u32, out: &mut [u8]) -> usize;
}

/// A [`StrikeSource`] over bytes in memory.
pub struct SliceSource<'a>(pub &'a [u8]);

impl StrikeSource for SliceSource<'_> {
	fn read_at(&mut self, offset: u32, out: &mut [u8]) -> usize {
		let from = (offset as usize).min(self.0.len());
		let n = out.len().min(self.0.len() - from);
		if let (Some(d), Some(s)) = (out.get_mut(..n), self.0.get(from..from + n)) {
			d.copy_from_slice(s);
		}
		n
	}
}

/// One strike of a file, by its place in the directory.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct StrikeId(u16);

impl StrikeId {
	/// The place in the directory, from 0.
	pub fn index(self) -> u16 {
		self.0
	}

	/// The first strike of a file, which every file has.
	pub fn default_first(_of: &Strikes<'_>) -> Self {
		StrikeId(0)
	}

	/// An id no file has, to test that a stranger's id is harmless.
	pub fn out_of_range() -> Self {
		StrikeId(u16::MAX)
	}
}

/// What the directory says about a strike.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct StrikeInfo {
	/// The face, an index into the face table.
	pub face: u8,
	/// Bit 0 bold, bit 1 italic.
	pub style: u8,
	/// The size it was drawn for, in half points.
	pub size: u16,
	/// The em height in pixels.
	pub pixels: u8,
	/// Pixels above the baseline.
	pub ascent: u8,
	/// Pixels below the baseline.
	pub descent: u8,
	/// External leading in pixels.
	pub leading: u8,
	/// Bits per pixel of the bitmaps: 1 or 2.
	pub bpp: u8,
	/// Where the strike's data starts in the file.
	pub data_off: u32,
	/// How long it is.
	pub data_len: u32,
}

/// One glyph's box. Its bitmap is in the buffer [`Strikes::glyph`] filled.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Glyph {
	/// Pixels from the pen position to the bitmap's left edge.
	pub left: i32,
	/// Pixels from the baseline up to the bitmap's top row.
	pub top: i32,
	/// Bitmap width in pixels.
	pub width: usize,
	/// Bitmap height in pixels.
	pub height: usize,
	/// Bytes in a bitmap row.
	pub stride: usize,
	/// Bits per pixel: 1 or 2.
	pub bpp: u8,
}

impl Glyph {
	/// The ink at column `x` of row `y` of the bitmap in `bits`, from 0 (paper) to 3 (full ink). One-bit bitmaps give 0 or 3.
	pub fn level(&self, bits: &[u8], x: usize, y: usize) -> u8 {
		if x >= self.width || y >= self.height {
			return 0;
		}
		let Some(b) = bits.get(y * self.stride + x * usize::from(self.bpp) / 8) else {
			return 0;
		};
		if self.bpp == 1 {
			if (b >> (7 - (x & 7))) & 1 != 0 {
				3
			} else {
				0
			}
		} else {
			(b >> (6 - 2 * (x & 3))) & 3
		}
	}
}

/// A parsed `QUILL.FNT` index, borrowing the bytes it was parsed from.
#[derive(Copy, Clone, Debug)]
pub struct Strikes<'a> {
	index: &'a [u8],
	strikes: u16,
}

fn rd16(b: &[u8], at: usize) -> u16 {
	match b.get(at..at + 2) {
		Some(&[a, c]) => u16::from_le_bytes([a, c]),
		_ => 0,
	}
}

fn rd32(b: &[u8], at: usize) -> u32 {
	match b.get(at..at + 4) {
		Some(&[a, c, d, e]) => u32::from_le_bytes([a, c, d, e]),
		_ => 0,
	}
}

fn zero(b: &[u8], from: usize, to: usize) -> bool {
	b.get(from..to).is_some_and(|s| s.iter().all(|v| *v == 0))
}

fn counts(head: &[u8]) -> Result<(u16, u32), Error> {
	if head.len() < HEADER_LEN {
		return Err(Error::Truncated);
	}
	if head.get(..4) != Some(&MAGIC) || rd16(head, 4) != VERSION {
		return Err(Error::BadMagic);
	}
	let strikes = rd16(head, 6);
	if !(1..=MAX_STRIKES as u16).contains(&strikes)
		|| usize::from(rd16(head, 8)) != GLYPHS
		|| head.get(10) != Some(&0x20)
		|| head.get(11) != Some(&(DIR_LEN as u8))
		|| rd32(head, 12) != HEADER_LEN as u32
		|| !zero(head, 24, 32)
	{
		return Err(Error::BadHeader);
	}
	let len = (HEADER_LEN + (DIR_LEN + GLYPHS) * usize::from(strikes)) as u32;
	Ok((strikes, len))
}

/// How many bytes of the file the index (the header, the directory and the advances) takes, read from the first bytes of it (at
/// least [`HEADER_LEN`]). A shell reads that many into memory and gives them to [`Strikes::parse`].
pub fn index_len(head: &[u8]) -> Result<u32, Error> {
	counts(head).map(|c| c.1)
}

impl<'a> Strikes<'a> {
	/// Checks the index at the start of a file `file_len` bytes long, which `index` begins with (more bytes are ignored). Every
	/// offset and length the index holds is checked here; the boxes and bitmaps are checked when read, and the checksum by
	/// [`Strikes::verify`].
	pub fn parse(index: &'a [u8], file_len: u32) -> Result<Self, Error> {
		let (strikes, ilen) = counts(index)?;
		if rd32(index, 16) != file_len {
			return Err(Error::BadHeader);
		}
		let index = index.get(..ilen as usize).ok_or(Error::Truncated)?;
		let me = Strikes { index, strikes };
		let adv_base = (HEADER_LEN + DIR_LEN * usize::from(strikes)) as u32;
		let mut prev_key = None;
		let mut prev_end = ilen;
		for i in 0..strikes {
			let at = me.dir_at(i);
			let b = |o: usize| index.get(at + o).copied().unwrap_or(0xFF);
			let (face, style, size, pixels, bpp) = (b(0), b(1), rd16(index, at + 2), b(4), b(8));
			let (adv, off, len) = (
				rd32(index, at + 12),
				rd32(index, at + 16),
				rd32(index, at + 20),
			);
			if face > 2
				|| style > 3 || size == 0
				|| !(1..=MAX_PIXELS).contains(&pixels)
				|| !(1..=2).contains(&bpp)
				|| !zero(index, at + 9, at + 12)
			{
				return Err(Error::BadStrike);
			}
			let key = (face, style, size);
			if prev_key.is_some_and(|p| p >= key) {
				return Err(Error::BadStrike);
			}
			prev_key = Some(key);
			if adv != adv_base + (GLYPHS as u32) * u32::from(i) {
				return Err(Error::Range);
			}
			if len < BOXES_LEN as u32 || len > MAX_DATA_LEN {
				return Err(Error::BadStrike);
			}
			let end = off.checked_add(len).ok_or(Error::Range)?;
			if off < prev_end || end > file_len {
				return Err(Error::Range);
			}
			prev_end = end;
		}
		Ok(me)
	}

	/// Reads the whole file through `src` and compares its CRC-32 (of everything after the header) with the header's. Slow from a
	/// floppy, so the shell chooses whether to ask.
	pub fn verify<S: StrikeSource + ?Sized>(&self, src: &mut S) -> bool {
		let (file_len, want) = (rd32(self.index, 16), rd32(self.index, 20));
		let mut crc = 0;
		let mut at = HEADER_LEN as u32;
		let mut buf = [0u8; 256];
		while at < file_len {
			let n = ((file_len - at) as usize).min(buf.len());
			let Some(chunk) = buf.get_mut(..n) else {
				return false;
			};
			if src.read_at(at, chunk) != n {
				return false;
			}
			crc = crc32_update(crc, chunk);
			at += n as u32;
		}
		crc == want
	}

	fn dir_at(&self, i: u16) -> usize {
		HEADER_LEN + DIR_LEN * usize::from(i)
	}

	/// How many strikes the file holds.
	pub fn strike_count(&self) -> u16 {
		self.strikes
	}

	/// Every strike, in directory order.
	pub fn ids(&self) -> impl Iterator<Item = StrikeId> {
		(0..self.strikes).map(StrikeId)
	}

	/// The directory entry of a strike. An id from another file may be out of range and gives an empty entry.
	pub fn strike(&self, id: StrikeId) -> StrikeInfo {
		if id.0 >= self.strikes {
			return StrikeInfo::default();
		}
		let at = self.dir_at(id.0);
		let b = |o: usize| self.index.get(at + o).copied().unwrap_or(0);
		StrikeInfo {
			face: b(0),
			style: b(1),
			size: rd16(self.index, at + 2),
			pixels: b(4),
			ascent: b(5),
			descent: b(6),
			leading: b(7),
			bpp: b(8),
			data_off: rd32(self.index, at + 16),
			data_len: rd32(self.index, at + 20),
		}
	}

	/// The advance in pixels of character `ch` (0x20 and up) in a strike, before any scaling; 0 for anything else.
	pub fn advance(&self, id: StrikeId, ch: u8) -> u8 {
		if id.0 >= self.strikes || ch < 0x20 {
			return 0;
		}
		let at = self.dir_at(id.0);
		let base = rd32(self.index, at + 12) as usize;
		self.index
			.get(base + usize::from(ch - 0x20))
			.copied()
			.unwrap_or(0)
	}

	/// The strike and scale (1 to 3) that draw `f` best: the face by the font's family code (1 roman is face 0, 2 swiss face 1, 3
	/// modern face 2, anything else face 0, and the lowest face with strikes when that one has none); the style with the fewest bits
	/// wrong; then the strike and scale whose pixel size is nearest the one the font's size asks for at 96 dpi. A tie goes to the
	/// smaller scale and then the smaller strike. The measure and the painter both call this, so they cannot disagree.
	pub fn strike_for(&self, f: &FontRef) -> (StrikeId, u8) {
		self.strike_for_px(f, wanted_px(f))
	}

	/// [`Strikes::strike_for`] for a size given in pixels, which a zoomed page asks for (the font's size at the zoom).
	pub fn strike_for_px(&self, f: &FontRef, want_px: u32) -> (StrikeId, u8) {
		let want_style =
			(u8::from(f.flags & cflag::BOLD != 0)) | (u8::from(f.flags & cflag::ITALIC != 0) << 1);
		let mut face = match f.family {
			2 => 1u8,
			3 => 2,
			_ => 0,
		};
		if !self.ids().any(|id| self.strike(id).face == face) {
			face = self.ids().map(|id| self.strike(id).face).min().unwrap_or(0);
		}
		let mut style = None::<(u32, u8)>;
		for id in self.ids() {
			let s = self.strike(id);
			if s.face == face {
				let d = (s.style ^ want_style).count_ones();
				if style.is_none_or(|(bd, _)| d < bd) {
					style = Some((d, s.style));
				}
			}
		}
		let style = style.map_or(0, |s| s.1);
		let mut best = None::<((u32, u8, u8), StrikeId, u8)>;
		for id in self.ids() {
			let s = self.strike(id);
			if s.face != face || s.style != style {
				continue;
			}
			for scale in 1..=MAX_SCALE {
				let px = u32::from(s.pixels) * u32::from(scale);
				let key = (px.abs_diff(want_px), scale, s.pixels);
				if best.is_none_or(|(k, _, _)| key < k) {
					best = Some((key, id, scale));
				}
			}
		}
		best.map_or((StrikeId(0), 1), |(_, id, scale)| (id, scale))
	}

	/// Reads the box and bitmap of character `ch` of strike `id` through `src` into `bits`. `None` for a character below 0x20, a
	/// short read, or a box or bitmap that does not lie inside the strike's block, however the file was damaged. A blank character
	/// gives a glyph with no width or height.
	pub fn glyph<S: StrikeSource + ?Sized>(
		&self,
		src: &mut S,
		id: StrikeId,
		ch: u8,
		bits: &mut [u8; MAX_GLYPH_BYTES],
	) -> Option<Glyph> {
		if ch < 0x20 || id.0 >= self.strikes {
			return None;
		}
		let s = self.strike(id);
		let mut b = [0u8; BOX_LEN];
		let at = s
			.data_off
			.checked_add((usize::from(ch - 0x20) * BOX_LEN) as u32)?;
		if src.read_at(at, &mut b) != BOX_LEN {
			return None;
		}
		let [left, top, w, h, o0, o1] = b;
		let (w, h) = (usize::from(w), usize::from(h));
		if w > MAX_GLYPH_SIDE || h > MAX_GLYPH_SIDE {
			return None;
		}
		let bpp = s.bpp;
		let stride = (w * usize::from(bpp)).div_ceil(8);
		let glyph = Glyph {
			left: i32::from(left as i8),
			top: i32::from(top as i8),
			width: w,
			height: h,
			stride,
			bpp,
		};
		let n = stride * h;
		if n == 0 {
			return Some(Glyph {
				width: 0,
				height: 0,
				stride: 0,
				..glyph
			});
		}
		let off = BOXES_LEN as u32 + u32::from(u16::from_le_bytes([o0, o1]));
		if off + n as u32 > s.data_len {
			return None;
		}
		let out = bits.get_mut(..n)?;
		if src.read_at(s.data_off.checked_add(off)?, out) != n {
			return None;
		}
		Some(glyph)
	}
}

/// Layout's view of the strikes: 96 dots per inch and each strike's own integer advances and metrics, times 2 or 3 for a strike
/// drawn larger. Cloning it copies a reference.
#[derive(Copy, Clone, Debug)]
pub struct StrikeMeasure<'a> {
	strikes: &'a Strikes<'a>,
}

impl<'a> StrikeMeasure<'a> {
	/// A measure over a parsed file.
	pub fn new(strikes: &'a Strikes<'a>) -> Self {
		StrikeMeasure { strikes }
	}

	/// The file.
	pub fn strikes(&self) -> &'a Strikes<'a> {
		self.strikes
	}
}

impl Measure for StrikeMeasure<'_> {
	fn resolution(&self) -> (u16, u16) {
		(96, 96)
	}

	fn metrics(&mut self, font: &FontRef) -> FontMetrics {
		let (id, scale) = self.strikes.strike_for(font);
		let s = self.strikes.strike(id);
		let k = i32::from(scale);
		FontMetrics {
			ascent: i32::from(s.ascent) * k,
			descent: i32::from(s.descent) * k,
			leading: i32::from(s.leading) * k,
		}
	}

	fn widths(&mut self, font: &FontRef, out: &mut [u16; GLYPHS]) {
		let (id, scale) = self.strikes.strike_for(font);
		for (i, w) in out.iter_mut().enumerate() {
			*w = u16::from(self.strikes.advance(id, 0x20u8.wrapping_add(i as u8)))
				* u16::from(scale);
		}
	}
}
