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
