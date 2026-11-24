// PNG: every colour type and bit depth, filtered rows, by rows through a streaming inflater; interlaced files are refused.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use quill_formats::io::Source;
use quill_inflate::{inflate, WINDOW};

use crate::{ImageError, MAX_ROW, MAX_SIDE};

/// The widest row of raw (filtered) bytes: 16-bit RGBA across the widest picture.
const MAX_RAW: usize = MAX_SIDE as usize * 8 + 1;

fn be32<S: Source>(r: &mut S) -> Option<u32> {
	let mut b = [0u8; 4];
	(r.read(&mut b) == 4).then(|| crate::be32_bytes(b))
}

fn skip<S: Source>(r: &mut S, mut n: u32) -> Option<()> {
	let mut b = [0u8; 64];
	while n > 0 {
		let k = n.min(64) as usize;
		if r.read(b.get_mut(..k)?) != k {
			return None;
		}
		n -= k as u32;
	}
	Some(())
}

/// A running CRC-32 (the PNG chunk checksum), by nibbles so the table is 64 bytes.
#[derive(Copy, Clone)]
struct Crc(u32);

impl Crc {
	const TABLE: [u32; 16] = [
		0x0000_0000,
		0x1DB7_1064,
		0x3B6E_20C8,
		0x26D9_30AC,
		0x76DC_4190,
		0x6B6B_51F4,
		0x4DB2_6158,
		0x5005_713C,
		0xEDB8_8320,
		0xF00F_9344,
		0xD6D6_A3E8,
		0xCB61_B38C,
		0x9B64_C2B0,
		0x86D3_D2D4,
		0xA00A_E278,
		0xBDBD_F21C,
	];

	fn new() -> Self {
		Crc(0xFFFF_FFFF)
	}

	fn byte(&mut self, b: u8) {
		let mut c = self.0 ^ u32::from(b);
		for _ in 0..2 {
			c = (c >> 4) ^ Self::TABLE.get((c & 15) as usize).copied().unwrap_or(0);
		}
		self.0 = c;
	}

	fn bytes(&mut self, d: &[u8]) {
		for &b in d {
			self.byte(b);
		}
	}

	fn value(self) -> u32 {
		!self.0
	}
}

/// Reads the four checksum bytes after a chunk's data and compares them with `crc`.
fn check_crc<S: Source>(r: &mut S, crc: Crc) -> Result<(), ImageError> {
	(be32(r) == Some(crc.value()))
		.then_some(())
		.ok_or(ImageError::BadFile)
}

struct Header {
	width: u32,
	height: u32,
	depth: u8,
	colour: u8,
}

fn channels(colour: u8) -> usize {
	match colour {
		2 => 3,
		4 => 2,
		6 => 4,
		_ => 1,
	}
}

fn paeth(a: u8, b: u8, c: u8) -> u8 {
	let (ia, ib, ic) = (i16::from(a), i16::from(b), i16::from(c));
	let p = ia + ib - ic;
	let (pa, pb, pc) = ((p - ia).abs(), (p - ib).abs(), (p - ic).abs());
	if pa <= pb && pa <= pc {
		a
	} else if pb <= pc {
		b
	} else {
		c
	}
}

/// Undoes the row filter in place: `row` is the filtered row, `prev` the unfiltered row above (zeros at the top).
fn unfilter(filter: u8, row: &mut [u8], prev: &[u8], bpp: usize) -> bool {
	for i in 0..row.len() {
		let left = if i >= bpp {
			row.get(i - bpp).copied().unwrap_or(0)
		} else {
			0
		};
		let up = prev.get(i).copied().unwrap_or(0);
		let ul = if i >= bpp {
			prev.get(i - bpp).copied().unwrap_or(0)
		} else {
			0
		};
		let add = match filter {
			0 => 0,
			1 => left,
			2 => up,
			3 => ((u16::from(left) + u16::from(up)) / 2) as u8,
			4 => paeth(left, up, ul),
			_ => return false,
		};
		if let Some(b) = row.get_mut(i) {
			*b = b.wrapping_add(add);
		}
	}
	true
}

/// Reads sample `i` of `depth` bits from a row.
fn sample(row: &[u8], i: usize, depth: u8) -> u16 {
	match depth {
		16 => u16::from_be_bytes([
			row.get(i * 2).copied().unwrap_or(0),
			row.get(i * 2 + 1).copied().unwrap_or(0),
		]),
		8 => u16::from(row.get(i).copied().unwrap_or(0)),
		d => {
			let per = 8 / usize::from(d);
			let byte = row.get(i / per).copied().unwrap_or(0);
			let shift = 8 - usize::from(d) * (i % per + 1);
			u16::from((byte >> shift) & ((1u8 << d) - 1))
		}
	}
}

fn to8(v: u16, depth: u8) -> u8 {
	match depth {
		16 => (v >> 8) as u8,
		8 => v as u8,
		d => (u32::from(v) * 255 / ((1u32 << d) - 1)) as u8,
	}
}

fn blend(c: u8, a: u8) -> u8 {
	((u16::from(c) * u16::from(a) + 255 * (255 - u16::from(a))) / 255) as u8
}

/// Decodes a PNG, calling `f(y, rgb)` for each row from the top. Transparency is blended over white. `window` is the inflater's 32 KB.
/// Returns the size in pixels.
pub fn decode_rows<S: Source>(
	r: &mut S,
	window: &mut [u8; WINDOW],
	mut f: impl FnMut(u32, &[u8]),
) -> Result<(u32, u32), ImageError> {
	let bad = ImageError::BadFile;
	let mut sig = [0u8; 8];
	if r.read(&mut sig) != 8 || sig != [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A] {
		return Err(bad);
	}
	let mut hdr: Option<Header> = None;
	let mut palette = [[0u8; 3]; 256];
	let mut pal_n = 0usize;
	loop {
		let len = be32(r).ok_or(bad)?;
		let mut kind = [0u8; 4];
		if r.read(&mut kind) != 4 {
			return Err(bad);
		}
		match &kind {
			b"IHDR" => {
				if len != 13 {
					return Err(bad);
				}
				let mut b = [0u8; 13];
				if r.read(&mut b) != 13 {
					return Err(bad);
				}
				let (w, h) = (
					crate::be32_bytes([b[0], b[1], b[2], b[3]]),
					crate::be32_bytes([b[4], b[5], b[6], b[7]]),
				);
				let (depth, colour, interlace) = (b[8], b[9], b[12]);
				let ok = match colour {
					0 => matches!(depth, 1 | 2 | 4 | 8 | 16),
					3 => matches!(depth, 1 | 2 | 4 | 8),
					2 | 4 | 6 => matches!(depth, 8 | 16),
					_ => false,
				};
				if !ok || b[10] != 0 || b[11] != 0 {
					return Err(bad);
				}
				if interlace != 0 {
					return Err(ImageError::Unsupported);
				}
				if w == 0 || h == 0 || w > MAX_SIDE || h > MAX_SIDE {
					return Err(ImageError::TooLarge);
				}
				let mut crc = Crc::new();
				crc.bytes(&kind);
				crc.bytes(&b);
				check_crc(r, crc)?;
				hdr = Some(Header {
					width: w,
					height: h,
					depth,
					colour,
				});
			}
			b"PLTE" => {
				if len % 3 != 0 || len > 768 {
					return Err(bad);
				}
				pal_n = (len / 3) as usize;
				let mut crc = Crc::new();
				crc.bytes(&kind);
				for i in 0..pal_n {
					let mut e = [0u8; 3];
					if r.read(&mut e) != 3 {
						return Err(bad);
					}
					crc.bytes(&e);
					if let Some(p) = palette.get_mut(i) {
						*p = e;
					}
				}
				check_crc(r, crc)?;
			}
			b"IDAT" => {
				let h = hdr.as_ref().ok_or(bad)?;
				return image_data(r, window, h, &palette, pal_n, len, &mut f)
					.map(|()| (h.width, h.height));
			}
			b"IEND" => return Err(bad),
			_ => skip(r, len.checked_add(4).ok_or(bad)?).ok_or(bad)?,
		}
	}
}

fn image_data<S: Source>(
	r: &mut S,
	window: &mut [u8; WINDOW],
	h: &Header,
	palette: &[[u8; 3]; 256],
	pal_n: usize,
	first_len: u32,
	f: &mut impl FnMut(u32, &[u8]),
) -> Result<(), ImageError> {
	let bad = ImageError::BadFile;
	let ch = channels(h.colour);
	let bits = ch * usize::from(h.depth);
	let row_bytes = (bits * h.width as usize).div_ceil(8);
	let bpp = bits.div_ceil(8).max(1);
	if row_bytes + 1 > MAX_RAW {
		return Err(ImageError::TooLarge);
	}
	if h.colour == 3 && pal_n == 0 {
		return Err(ImageError::BadFile);
	}
	let mut remain = first_len;
	let mut ended = false;
	let mut crc = Crc::new();
	crc.bytes(b"IDAT");
	let mut crc_bad = false;
	let mut next = || -> Option<u8> {
		loop {
			if ended {
				return None;
			}
			if remain > 0 {
				let mut b = [0u8; 1];
				if r.read(&mut b) != 1 {
					ended = true;
					return None;
				}
				remain -= 1;
				crc.bytes(&b);
				return b.first().copied();
			}
			// The chunk's checksum, then the next chunk if it is more image data.
			if be32(r) != Some(crc.value()) {
				crc_bad = true;
				ended = true;
				return None;
			}
			let len = be32(r)?;
			let mut kind = [0u8; 4];
			if r.read(&mut kind) != 4 || &kind != b"IDAT" {
				ended = true;
				return None;
			}
			crc = Crc::new();
			crc.bytes(&kind);
			remain = len;
		}
	};
	// The two-byte zlib header; the Adler-32 after the stream and each chunk's CRC are checked.
	next().ok_or(bad)?;
	next().ok_or(bad)?;

	let mut cur = [0u8; MAX_RAW];
	let mut prev = [0u8; MAX_RAW];
	let mut rgb = [0u8; MAX_ROW];
	let (mut pos, mut y) = (0usize, 0u32);
	let mut filter = 0u8;
	let mut failure: Option<ImageError> = None;
	let (mut ad_a, mut ad_b) = (1u32, 0u32);
	let res = inflate(
		&mut next,
		|b| {
			ad_a = (ad_a + u32::from(b)) % 65521;
			ad_b = (ad_b + ad_a) % 65521;
			if y >= h.height {
				// More data than the picture holds.
				failure = Some(ImageError::BadFile);
				return false;
			}
			if pos == 0 {
				filter = b;
			} else if let Some(slot) = cur.get_mut(pos - 1) {
				*slot = b;
			}
			pos += 1;
			if pos <= row_bytes {
				return true;
			}
			pos = 0;
			let (Some(row), Some(up)) = (cur.get_mut(..row_bytes), prev.get(..row_bytes)) else {
				failure = Some(ImageError::BadFile);
				return false;
			};
			if !unfilter(filter, row, up, bpp) {
				failure = Some(ImageError::BadFile);
				return false;
			}
			let w = h.width as usize;
			for x in 0..w {
				let px: [u8; 3] = match h.colour {
					0 => {
						let g = to8(sample(row, x, h.depth), h.depth);
						[g, g, g]
					}
					3 => {
						let i = usize::from(sample(row, x, h.depth));
						if i >= pal_n {
							failure = Some(ImageError::BadFile);
							return false;
						}
						palette.get(i).copied().unwrap_or([0; 3])
					}
					2 => [
						to8(sample(row, x * 3, h.depth), h.depth),
						to8(sample(row, x * 3 + 1, h.depth), h.depth),
						to8(sample(row, x * 3 + 2, h.depth), h.depth),
					],
					4 => {
						let g = to8(sample(row, x * 2, h.depth), h.depth);
						let a = to8(sample(row, x * 2 + 1, h.depth), h.depth);
						let v = blend(g, a);
						[v, v, v]
					}
					_ => {
						let a = to8(sample(row, x * 4 + 3, h.depth), h.depth);
						[
							blend(to8(sample(row, x * 4, h.depth), h.depth), a),
							blend(to8(sample(row, x * 4 + 1, h.depth), h.depth), a),
							blend(to8(sample(row, x * 4 + 2, h.depth), h.depth), a),
						]
					}
				};
				if let Some(o) = rgb.get_mut(x * 3..x * 3 + 3) {
					o.copy_from_slice(&px);
				}
			}
			f(y, rgb.get(..w * 3).unwrap_or(&[]));
			y += 1;
			core::mem::swap(&mut cur, &mut prev);
			true
		},
		window,
	);
	if let Some(e) = failure {
		return Err(e);
	}
	if res.is_err() || y != h.height {
		return Err(bad);
	}
	// The zlib checksum; then nothing more in the data, which makes the closure check the last chunk's CRC.
	let mut ad = [0u8; 4];
	for v in ad.iter_mut() {
		*v = next().ok_or(bad)?;
	}
	if crate::be32_bytes(ad) != (ad_b << 16) | ad_a || next().is_some() {
		return Err(bad);
	}
	if crc_bad {
		return Err(bad);
	}
	Ok(())
}

/// Reads just the size of a PNG: its width and height in pixels.
pub fn read_size<S: Source>(r: &mut S) -> Result<(u32, u32), ImageError> {
	let bad = ImageError::BadFile;
	let mut head = [0u8; 24];
	if r.read(&mut head) != 24
		|| head.get(..8) != Some(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A][..])
		|| head.get(12..16) != Some(b"IHDR")
	{
		return Err(bad);
	}
	let w = crate::be32_bytes([head[16], head[17], head[18], head[19]]);
	let h = crate::be32_bytes([head[20], head[21], head[22], head[23]]);
	if w == 0 || h == 0 || w > MAX_SIDE || h > MAX_SIDE {
		return Err(ImageError::TooLarge);
	}
	Ok((w, h))
}
