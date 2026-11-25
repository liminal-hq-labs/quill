// Windows bitmaps: 1, 4, 8 and 24 bits a pixel, uncompressed, bottom-up or top-down, by rows.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use quill_formats::io::Source;

use crate::{ImageError, MAX_SIDE};

/// What the headers of a bitmap say.
#[derive(Clone)]
pub struct BmpInfo {
	/// Pixels across.
	pub width: u32,
	/// Pixels down.
	pub height: u32,
	/// Bits a pixel: 1, 4, 8 or 24.
	pub bpp: u16,
	/// True when the first row in the file is the top row (a negative height).
	pub top_down: bool,
	palette: [[u8; 3]; 256],
	colours: usize,
	row_bytes: usize,
}

fn u16le<S: Source>(r: &mut S) -> Option<u16> {
	let mut b = [0u8; 2];
	(r.read(&mut b) == 2).then(|| u16::from_le_bytes(b))
}

fn u32le<S: Source>(r: &mut S) -> Option<u32> {
	let mut b = [0u8; 4];
	(r.read(&mut b) == 4).then(|| u32::from_le_bytes(b))
}

fn skip<S: Source>(r: &mut S, mut n: usize) -> Option<()> {
	let mut b = [0u8; 64];
	while n > 0 {
		let k = n.min(64);
		if r.read(b.get_mut(..k)?) != k {
			return None;
		}
		n -= k;
	}
	Some(())
}

impl BmpInfo {
	/// The colour of palette entry `i`. An index past the table, which writers that count `biClrUsed` by the colours in use leave in
	/// their pixels, is black, as Windows draws it, and not a reason to refuse the picture.
	fn colour(&self, i: usize) -> [u8; 3] {
		if i >= self.colours {
			return [0; 3];
		}
		self.palette.get(i).copied().unwrap_or([0; 3])
	}
}

/// Reads the file header, the info header and the palette, leaving `r` at the first row of pixels.
pub fn read_info<S: Source>(r: &mut S) -> Result<BmpInfo, ImageError> {
	let bad = ImageError::BadFile;
	let mut sig = [0u8; 2];
	if r.read(&mut sig) != 2 || &sig != b"BM" {
		return Err(bad);
	}
	u32le(r).ok_or(bad)?; // file size
	u32le(r).ok_or(bad)?; // reserved
	let data_at = u32le(r).ok_or(bad)? as usize;
	let hsize = u32le(r).ok_or(bad)? as usize;
	let (w, h, bpp, compression, used);
	if hsize == 12 {
		// The OS/2 core header: 16-bit sizes, no compression, three-byte palette entries.
		w = i32::from(u16le(r).ok_or(bad)?);
		h = i32::from(u16le(r).ok_or(bad)?);
		u16le(r).ok_or(bad)?;
		bpp = u16le(r).ok_or(bad)?;
		compression = 0;
		used = 0;
	} else if hsize >= 40 {
		w = u32le(r).ok_or(bad)? as i32;
		h = u32le(r).ok_or(bad)? as i32;
		u16le(r).ok_or(bad)?;
		bpp = u16le(r).ok_or(bad)?;
		compression = u32le(r).ok_or(bad)?;
		skip(r, 12).ok_or(bad)?;
		used = u32le(r).ok_or(bad)? as usize;
		u32le(r).ok_or(bad)?;
		skip(r, hsize - 40).ok_or(bad)?;
	} else {
		return Err(bad);
	}
	if compression != 0 {
		return Err(ImageError::Unsupported);
	}
	if !matches!(bpp, 1 | 4 | 8 | 24) {
		return Err(if matches!(bpp, 16 | 32) {
			ImageError::Unsupported
		} else {
			bad
		});
	}
	let top_down = h < 0;
	let (width, height) = (w.unsigned_abs(), h.unsigned_abs());
	if w <= 0 || height == 0 || width > MAX_SIDE || height > MAX_SIDE {
		return Err(ImageError::TooLarge);
	}
	let mut palette = [[0u8; 3]; 256];
	let mut colours = 0;
	let mut consumed = 14 + hsize;
	if bpp <= 8 {
		colours = if used == 0 {
			1usize << bpp
		} else {
			used.min(1 << bpp)
		};
		let entry = if hsize == 12 { 3 } else { 4 };
		// The pixels start where the file header says. A writer that puts them right after a shorter table than the header declares
		// (an 8-bit bitmap with 16 colours and no `biClrUsed`) has a palette of the size it wrote; reading the declared size would
		// swallow the first rows.
		if data_at >= consumed {
			colours = colours.min((data_at - consumed) / entry);
		}
		for i in 0..colours {
			let mut e = [0u8; 4];
			let n = e.get_mut(..entry).ok_or(bad)?;
			if r.read(n) != entry {
				return Err(bad);
			}
			if let Some(p) = palette.get_mut(i) {
				*p = [e[2], e[1], e[0]];
			}
		}
		consumed += colours * entry;
	}
	skip(r, data_at.saturating_sub(consumed)).ok_or(bad)?;
	let row_bytes = (usize::from(bpp) * width as usize).div_ceil(32) * 4;
	Ok(BmpInfo {
		width,
		height,
		bpp,
		top_down,
		palette,
		colours,
		row_bytes,
	})
}

/// Decodes the rows in the order the file holds them, calling `f(y, rgb)` with the picture row number from the top and its pixels as
/// three bytes each (red, green, blue). A bottom-up bitmap gives the last row first.
pub fn decode_rows<S: Source>(
	r: &mut S,
	info: &BmpInfo,
	mut f: impl FnMut(u32, &[u8]),
) -> Result<(), ImageError> {
	let mut raw = [0u8; ((crate::MAX_SIDE as usize * 24).div_ceil(32)) * 4];
	let mut rgb = [0u8; crate::MAX_ROW];
	let w = info.width as usize;
	for n in 0..info.height {
		let line = raw.get_mut(..info.row_bytes).ok_or(ImageError::BadFile)?;
		if r.read(line) != info.row_bytes {
			return Err(ImageError::BadFile);
		}
		for x in 0..w {
			let px: [u8; 3] = match info.bpp {
				24 => match line.get(x * 3..x * 3 + 3) {
					Some([b, g, r]) => [*r, *g, *b],
					_ => return Err(ImageError::BadFile),
				},
				8 => {
					let i = usize::from(*line.get(x).ok_or(ImageError::BadFile)?);
					info.colour(i)
				}
				4 => {
					let b = *line.get(x / 2).ok_or(ImageError::BadFile)?;
					let i = usize::from(if x % 2 == 0 { b >> 4 } else { b & 15 });
					info.colour(i)
				}
				_ => {
					let b = *line.get(x / 8).ok_or(ImageError::BadFile)?;
					let i = usize::from((b >> (7 - x % 8)) & 1);
					info.colour(i)
				}
			};
			let o = rgb.get_mut(x * 3..x * 3 + 3).ok_or(ImageError::BadFile)?;
			o.copy_from_slice(&px);
		}
		let y = if info.top_down {
			n
		} else {
			info.height - 1 - n
		};
		f(y, rgb.get(..w * 3).ok_or(ImageError::BadFile)?);
	}
	Ok(())
}
