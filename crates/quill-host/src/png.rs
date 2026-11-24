// A minimal PNG writer (1-bit greyscale, stored deflate blocks) for the reference page images.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

fn crc32(data: &[u8]) -> u32 {
	let mut c = 0xFFFF_FFFFu32;
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

fn adler32(data: &[u8]) -> u32 {
	let (mut a, mut b) = (1u32, 0u32);
	for d in data {
		a = (a + u32::from(*d)) % 65521;
		b = (b + a) % 65521;
	}
	(b << 16) | a
}

fn chunk(out: &mut Vec<u8>, id: &[u8; 4], data: &[u8]) {
	out.extend_from_slice(&(data.len() as u32).to_be_bytes());
	let mut body = id.to_vec();
	body.extend_from_slice(data);
	out.extend_from_slice(&body);
	out.extend_from_slice(&crc32(&body).to_be_bytes());
}

/// Encodes a 1-bit image; `rows` holds one packed row per entry (most significant bit first, 1 is white).
pub fn encode_1bit(width: u32, height: u32, rows: &[Vec<u8>]) -> Vec<u8> {
	let mut raw = Vec::new();
	for r in rows {
		raw.push(0);
		raw.extend_from_slice(r);
	}
	let mut z = vec![0x78, 0x01];
	let mut chunks = raw.chunks(65535).peekable();
	while let Some(c) = chunks.next() {
		z.push(u8::from(chunks.peek().is_none()));
		z.extend_from_slice(&(c.len() as u16).to_le_bytes());
		z.extend_from_slice(&(!(c.len() as u16)).to_le_bytes());
		z.extend_from_slice(c);
	}
	z.extend_from_slice(&adler32(&raw).to_be_bytes());
	let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
	let mut ihdr = Vec::new();
	ihdr.extend_from_slice(&width.to_be_bytes());
	ihdr.extend_from_slice(&height.to_be_bytes());
	ihdr.extend_from_slice(&[1, 0, 0, 0, 0]);
	chunk(&mut out, b"IHDR", &ihdr);
	chunk(&mut out, b"IDAT", &z);
	chunk(&mut out, b"IEND", &[]);
	out
}

struct Bits {
	out: Vec<u8>,
	acc: u32,
	n: u32,
}

impl Bits {
	/// Writes `n` bits of `v`, least significant bit first (header fields and extra bits).
	fn put(&mut self, v: u32, n: u32) {
		self.acc |= v << self.n;
		self.n += n;
		while self.n >= 8 {
			self.out.push(self.acc as u8);
			self.acc >>= 8;
			self.n -= 8;
		}
	}

	/// Writes a Huffman code of `n` bits, most significant bit first.
	fn code(&mut self, v: u32, n: u32) {
		let mut r = 0;
		for i in 0..n {
			r |= ((v >> i) & 1) << (n - 1 - i);
		}
		self.put(r, n);
	}

	fn literal(&mut self, b: u8) {
		if b < 144 {
			self.code(0x30 + u32::from(b), 8);
		} else {
			self.code(0x190 + u32::from(b) - 144, 9);
		}
	}

	/// A match of `len` bytes (3 to 258) at distance 1.
	fn run(&mut self, len: usize) {
		const T: [(usize, u32); 29] = [
			(3, 0),
			(4, 0),
			(5, 0),
			(6, 0),
			(7, 0),
			(8, 0),
			(9, 0),
			(10, 0),
			(11, 1),
			(13, 1),
			(15, 1),
			(17, 1),
			(19, 2),
			(23, 2),
			(27, 2),
			(31, 2),
			(35, 3),
			(43, 3),
			(51, 3),
			(59, 3),
			(67, 4),
			(83, 4),
			(99, 4),
			(115, 4),
			(131, 5),
			(163, 5),
			(195, 5),
			(227, 5),
			(258, 0),
		];
		let i = T.iter().rposition(|(base, _)| *base <= len).unwrap_or(0);
		let sym = 257 + i as u32;
		if sym < 280 {
			self.code(sym - 256, 7);
		} else {
			self.code(0xC0 + sym - 280, 8);
		}
		let (base, extra) = T[i];
		self.put((len - base) as u32, extra);
		self.code(0, 5);
	}
}

/// Deflates with the fixed Huffman code and one kind of match: a run of the byte before.
fn deflate_runs(raw: &[u8]) -> Vec<u8> {
	let mut b = Bits {
		out: vec![0x78, 0x01],
		acc: 0,
		n: 0,
	};
	b.put(1, 1);
	b.put(1, 2);
	let mut i = 0;
	while i < raw.len() {
		b.literal(raw[i]);
		let mut j = i + 1;
		while j < raw.len() && raw[j] == raw[i] {
			j += 1;
		}
		let mut left = j - (i + 1);
		let mut at = i + 1;
		while left >= 3 {
			let n = left.min(258);
			b.run(n);
			left -= n;
			at += n;
		}
		for _ in 0..left {
			b.literal(raw[i]);
			at += 1;
		}
		i = at;
	}
	b.code(0, 7);
	if b.n > 0 {
		b.put(0, 8 - b.n);
	}
	let mut z = b.out;
	z.extend_from_slice(&adler32(raw).to_be_bytes());
	z
}

/// Encodes an indexed image: `pixels` holds one palette index per byte, row by row; `depth` is 4 or 8 bits in the file. Rows are
/// filtered "up" and deflated with runs, which shrinks the flat areas of a screen to almost nothing.
pub fn encode_indexed(
	width: u32,
	height: u32,
	depth: u8,
	palette: &[[u8; 3]],
	pixels: &[u8],
) -> Vec<u8> {
	let w = width as usize;
	let stride = (w * usize::from(depth)).div_ceil(8);
	let mut rows: Vec<Vec<u8>> = Vec::new();
	for y in 0..height as usize {
		let src = &pixels[y * w..(y + 1) * w];
		let mut row = vec![0u8; stride];
		for (x, v) in src.iter().enumerate() {
			if depth == 4 {
				row[x / 2] |= (v & 15) << (if x % 2 == 0 { 4 } else { 0 });
			} else {
				row[x] = *v;
			}
		}
		rows.push(row);
	}
	let mut raw = Vec::new();
	for (y, row) in rows.iter().enumerate() {
		raw.push(if y == 0 { 0 } else { 2 });
		for (x, v) in row.iter().enumerate() {
			raw.push(if y == 0 {
				*v
			} else {
				v.wrapping_sub(rows[y - 1][x])
			});
		}
	}
	let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
	let mut ihdr = Vec::new();
	ihdr.extend_from_slice(&width.to_be_bytes());
	ihdr.extend_from_slice(&height.to_be_bytes());
	ihdr.extend_from_slice(&[depth, 3, 0, 0, 0]);
	chunk(&mut out, b"IHDR", &ihdr);
	let plte: Vec<u8> = palette.iter().flatten().copied().collect();
	chunk(&mut out, b"PLTE", &plte);
	chunk(&mut out, b"IDAT", &deflate_runs(&raw));
	chunk(&mut out, b"IEND", &[]);
	out
}

/// Encodes a truecolour image: `rgb` holds red, green, blue bytes, row by row. Rows are filtered "up" and deflated with runs.
pub fn encode_rgb(width: u32, height: u32, rgb: &[u8]) -> Vec<u8> {
	let stride = width as usize * 3;
	let mut raw = Vec::with_capacity((stride + 1) * height as usize);
	for y in 0..height as usize {
		let row = &rgb[y * stride..(y + 1) * stride];
		raw.push(if y == 0 { 0 } else { 2 });
		for (x, v) in row.iter().enumerate() {
			raw.push(if y == 0 {
				*v
			} else {
				v.wrapping_sub(rgb[(y - 1) * stride + x])
			});
		}
	}
	let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
	let mut ihdr = Vec::new();
	ihdr.extend_from_slice(&width.to_be_bytes());
	ihdr.extend_from_slice(&height.to_be_bytes());
	ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
	chunk(&mut out, b"IHDR", &ihdr);
	chunk(&mut out, b"IDAT", &deflate_runs(&raw));
	chunk(&mut out, b"IEND", &[]);
	out
}
