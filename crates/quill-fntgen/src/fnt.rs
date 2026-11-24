// The QUILL.FNT writer and a validating reader, to the layout in docs/dos-fnt-format.md.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::render::{Glyph, Strike, GLYPHS};

/// The four bytes that start the file.
pub const MAGIC: [u8; 4] = *b"QFNT";
/// The format version this code writes and reads.
pub const VERSION: u16 = 1;
/// Bytes in the header.
pub const HEADER_LEN: usize = 32;
/// Bytes in one directory entry.
pub const ENTRY_LEN: usize = 24;
/// Bytes in one glyph box.
pub const BOX_LEN: usize = 6;
/// Bytes of the box table of a strike.
pub const BOXES_LEN: usize = GLYPHS * BOX_LEN;
/// The most a strike's boxes and bitmaps may take: one `Store` chunk.
pub const MAX_DATA_LEN: usize = 32 * 1024;

const CRC_TABLE: [u32; 256] = {
	let mut t = [0u32; 256];
	let mut i = 0;
	while i < 256 {
		let mut c = i as u32;
		let mut k = 0;
		while k < 8 {
			c = if c & 1 != 0 {
				0xEDB8_8320 ^ (c >> 1)
			} else {
				c >> 1
			};
			k += 1;
		}
		t[i] = c;
		i += 1;
	}
	t
};

/// CRC-32 (the zlib polynomial) of `data`.
pub fn crc32(data: &[u8]) -> u32 {
	let mut c = 0xFFFF_FFFFu32;
	for b in data {
		c = CRC_TABLE[usize::from((c as u8) ^ *b)] ^ (c >> 8);
	}
	!c
}

/// Why a file was refused.
#[derive(Debug, PartialEq, Eq)]
pub enum FntError {
	/// Shorter than a header or a table says it is.
	Truncated,
	/// Wrong magic.
	Magic,
	/// A version this reader does not know.
	Version,
	/// The checksum does not match.
	Checksum,
	/// An offset, length or field is outside the file or the rules.
	Range,
}

/// Where a strike's tables sit in the file.
#[derive(Clone, Debug)]
pub struct Entry {
	/// The strike's header fields; the bitmaps are in `Font::glyph`.
	pub face: u8,
	/// Style bits.
	pub style: u8,
	/// Size in half points.
	pub half_points: u16,
	/// Em pixels.
	pub px: u8,
	/// Ascent.
	pub ascent: u8,
	/// Descent.
	pub descent: u8,
	/// External leading.
	pub leading: u8,
	/// Bits per pixel.
	pub bpp: u8,
	/// Offset of the 224 advances.
	pub adv_offset: u32,
	/// Offset of the boxes; the bitmaps follow them.
	pub data_offset: u32,
	/// Bytes of boxes and bitmaps.
	pub data_len: u32,
}

/// Writes the file for `strikes`, in the order given.
///
/// # Panics
///
/// When a strike's data is over [`MAX_DATA_LEN`] or the file is over 4 GiB: the generator is meant to be run on its own faces.
pub fn write(strikes: &[Strike]) -> Vec<u8> {
	let dir_len = strikes.len() * ENTRY_LEN;
	let adv_base = HEADER_LEN + dir_len;
	let mut data_off = adv_base + strikes.len() * GLYPHS;
	let mut dir = Vec::new();
	let mut advs = Vec::new();
	let mut body = Vec::new();
	for (i, s) in strikes.iter().enumerate() {
		let mut boxes = Vec::with_capacity(BOXES_LEN);
		let mut bitmaps = Vec::new();
		for g in &s.glyphs {
			boxes.extend_from_slice(&[g.x as u8, g.y as u8, g.w, g.h]);
			let off = if g.bits.is_empty() { 0 } else { bitmaps.len() };
			boxes.extend_from_slice(
				&u16::try_from(off)
					.expect("strike bitmaps over 64 KB")
					.to_le_bytes(),
			);
			bitmaps.extend_from_slice(&g.bits);
		}
		let data_len = boxes.len() + bitmaps.len();
		assert!(data_len <= MAX_DATA_LEN, "strike {i} is {data_len} bytes");
		dir.extend_from_slice(&[s.face, s.style]);
		dir.extend_from_slice(&s.half_points.to_le_bytes());
		dir.extend_from_slice(&[s.px, s.ascent, s.descent, s.leading, s.bpp, 0, 0, 0]);
		dir.extend_from_slice(&((adv_base + i * GLYPHS) as u32).to_le_bytes());
		dir.extend_from_slice(&((data_off) as u32).to_le_bytes());
		dir.extend_from_slice(&(data_len as u32).to_le_bytes());
		advs.extend_from_slice(&s.advances);
		body.extend_from_slice(&boxes);
		body.extend_from_slice(&bitmaps);
		data_off += data_len;
	}
	let mut out = Vec::with_capacity(data_off);
	out.extend_from_slice(&MAGIC);
	out.extend_from_slice(&VERSION.to_le_bytes());
	out.extend_from_slice(&(strikes.len() as u16).to_le_bytes());
	out.extend_from_slice(&(GLYPHS as u16).to_le_bytes());
	out.push(0x20);
	out.push(ENTRY_LEN as u8);
	out.extend_from_slice(&(HEADER_LEN as u32).to_le_bytes());
	out.extend_from_slice(&(data_off as u32).to_le_bytes());
	out.extend_from_slice(&[0; 12]);
	out.extend_from_slice(&dir);
	out.extend_from_slice(&advs);
	out.extend_from_slice(&body);
	let crc = crc32(&out[HEADER_LEN..]);
	out[20..24].copy_from_slice(&crc.to_le_bytes());
	out
}

fn u16_at(d: &[u8], at: usize) -> Result<u16, FntError> {
	let b = d.get(at..at + 2).ok_or(FntError::Truncated)?;
	Ok(u16::from_le_bytes([b[0], b[1]]))
}

fn u32_at(d: &[u8], at: usize) -> Result<u32, FntError> {
	let b = d.get(at..at + 4).ok_or(FntError::Truncated)?;
	Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

/// A parsed file: the directory over the file's bytes.
pub struct Font<'a> {
	data: &'a [u8],
	/// The directory.
	pub entries: Vec<Entry>,
}

impl<'a> Font<'a> {
	/// Checks every offset, length and the checksum of `data` once, so lookups afterwards cannot fail.
	pub fn parse(data: &'a [u8]) -> Result<Font<'a>, FntError> {
		if data.len() < HEADER_LEN {
			return Err(FntError::Truncated);
		}
		if data[..4] != MAGIC {
			return Err(FntError::Magic);
		}
		if u16_at(data, 4)? != VERSION {
			return Err(FntError::Version);
		}
		let count = usize::from(u16_at(data, 6)?);
		if usize::from(u16_at(data, 8)?) != GLYPHS
			|| data[10] != 0x20
			|| usize::from(data[11]) != ENTRY_LEN
		{
			return Err(FntError::Range);
		}
		let dir = u32_at(data, 12)? as usize;
		if u32_at(data, 16)? as usize != data.len() {
			return Err(FntError::Truncated);
		}
		if crc32(&data[HEADER_LEN..]) != u32_at(data, 20)? {
			return Err(FntError::Checksum);
		}
		let mut entries = Vec::with_capacity(count);
		for i in 0..count {
			let at = dir + i * ENTRY_LEN;
			let e = data.get(at..at + ENTRY_LEN).ok_or(FntError::Truncated)?;
			let entry = Entry {
				face: e[0],
				style: e[1],
				half_points: u16::from_le_bytes([e[2], e[3]]),
				px: e[4],
				ascent: e[5],
				descent: e[6],
				leading: e[7],
				bpp: e[8],
				adv_offset: u32_at(e, 12)?,
				data_offset: u32_at(e, 16)?,
				data_len: u32_at(e, 20)?,
			};
			let adv_end = entry.adv_offset as usize + GLYPHS;
			let data_end = entry.data_offset as usize + entry.data_len as usize;
			if !(1..=2).contains(&entry.bpp)
				|| entry.face > 2
				|| entry.style > 3
				|| adv_end > data.len()
				|| data_end > data.len()
				|| entry.data_len as usize > MAX_DATA_LEN
				|| (entry.data_len as usize) < BOXES_LEN
			{
				return Err(FntError::Range);
			}
			let bitmaps = entry.data_len as usize - BOXES_LEN;
			let boxes = &data[entry.data_offset as usize..][..BOXES_LEN];
			for b in boxes.chunks(BOX_LEN) {
				let (w, h) = (usize::from(b[2]), usize::from(b[3]));
				let off = usize::from(u16::from_le_bytes([b[4], b[5]]));
				let len = h * (w * usize::from(entry.bpp)).div_ceil(8);
				if off + len > bitmaps {
					return Err(FntError::Range);
				}
			}
			entries.push(entry);
		}
		Ok(Font { data, entries })
	}

	/// The strike of `face` and `style` at `half_points`, if the file has it.
	pub fn find(&self, face: u8, style: u8, half_points: u16) -> Option<usize> {
		self.entries
			.iter()
			.position(|e| e.face == face && e.style == style && e.half_points == half_points)
	}

	/// The advance of character `0x20 + i` in strike `s`.
	pub fn advance(&self, s: usize, i: usize) -> u8 {
		self.data[self.entries[s].adv_offset as usize + i]
	}

	/// The glyph of character `0x20 + i` in strike `s`.
	pub fn glyph(&self, s: usize, i: usize) -> Glyph {
		let e = &self.entries[s];
		let base = e.data_offset as usize;
		let b = &self.data[base + i * BOX_LEN..][..BOX_LEN];
		let (w, h) = (b[2], b[3]);
		let off = usize::from(u16::from_le_bytes([b[4], b[5]]));
		let len = usize::from(h) * (usize::from(w) * usize::from(e.bpp)).div_ceil(8);
		Glyph {
			x: b[0] as i8,
			y: b[1] as i8,
			w,
			h,
			bits: self.data[base + BOXES_LEN + off..][..len].to_vec(),
		}
	}
}
