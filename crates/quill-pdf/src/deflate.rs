// A zlib (RFC 1950) encoder: fixed Huffman codes (RFC 1951) over a hash-chain LZ77 matcher with a bounded chain.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Fixed codes need no tables in the output and no frequency pass, and a page of text or a screen-sized picture compresses to a
// small fraction either way; the matcher finds the repeats and the codes are the ones a decoder knows without being told. The
// result is never larger than the input plus five bytes in every 65,535 and the wrapper: a block that would grow is stored.

use alloc::vec::Vec;

const WINDOW: usize = 32768;
const HASH_BITS: u32 = 15;
const MIN_MATCH: usize = 3;
const MAX_MATCH: usize = 258;
/// How many earlier positions with the same hash the matcher tries before it settles for the best so far.
const MAX_CHAIN: usize = 32;
/// A match this long is taken at once.
const GOOD_ENOUGH: usize = 64;
const NONE: u32 = u32::MAX;

const LENGTH_BASE: [u16; 29] = [
	3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
	163, 195, 227, 258,
];
const LENGTH_EXTRA: [u8; 29] = [
	0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DIST_BASE: [u16; 30] = [
	1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
	2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DIST_EXTRA: [u8; 30] = [
	0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
	13,
];

/// The Adler-32 checksum of `data`.
pub fn adler32(data: &[u8]) -> u32 {
	let (mut a, mut b) = (1u32, 0u32);
	// 5,552 bytes is the most that can be summed before the sums must be reduced.
	for chunk in data.chunks(5552) {
		for &byte in chunk {
			a += u32::from(byte);
			b += a;
		}
		a %= 65521;
		b %= 65521;
	}
	(b << 16) | a
}

struct Bits<'a> {
	out: &'a mut Vec<u8>,
	acc: u32,
	count: u32,
}

impl Bits<'_> {
	/// Puts the low `n` bits of `value`, least significant first (up to 16 at a time).
	fn put(&mut self, value: u32, n: u32) {
		self.acc |= value << self.count;
		self.count += n;
		while self.count >= 8 {
			self.out.push(self.acc as u8);
			self.acc >>= 8;
			self.count -= 8;
		}
	}

	/// Puts a Huffman code of `n` bits, which goes most significant bit first.
	fn code(&mut self, code: u32, n: u32) {
		let mut rev = 0;
		for i in 0..n {
			rev |= ((code >> i) & 1) << (n - 1 - i);
		}
		self.put(rev, n);
	}

	fn flush(&mut self) {
		if self.count > 0 {
			self.out.push(self.acc as u8);
			self.acc = 0;
			self.count = 0;
		}
	}

	/// A literal or length symbol of the fixed code.
	fn symbol(&mut self, sym: u32) {
		match sym {
			0..=143 => self.code(0x30 + sym, 8),
			144..=255 => self.code(0x190 + (sym - 144), 9),
			256..=279 => self.code(sym - 256, 7),
			_ => self.code(0xC0 + (sym - 280), 8),
		}
	}
}

fn hash(a: u8, b: u8, c: u8) -> usize {
	let v = (u32::from(a) << 16) | (u32::from(b) << 8) | u32::from(c);
	(v.wrapping_mul(0x9E37_79B1) >> (32 - HASH_BITS)) as usize
}

fn length_symbol(len: usize) -> (u32, u32, u32) {
	let mut i = LENGTH_BASE.len() - 1;
	while i > 0 && usize::from(LENGTH_BASE.get(i).copied().unwrap_or(3)) > len {
		i -= 1;
	}
	let base = usize::from(LENGTH_BASE.get(i).copied().unwrap_or(3));
	(
		257 + i as u32,
		u32::from(LENGTH_EXTRA.get(i).copied().unwrap_or(0)),
		(len - base) as u32,
	)
}

fn dist_symbol(dist: usize) -> (u32, u32, u32) {
	let mut i = DIST_BASE.len() - 1;
	while i > 0 && usize::from(DIST_BASE.get(i).copied().unwrap_or(1)) > dist {
		i -= 1;
	}
	let base = usize::from(DIST_BASE.get(i).copied().unwrap_or(1));
	(
		i as u32,
		u32::from(DIST_EXTRA.get(i).copied().unwrap_or(0)),
		(dist - base) as u32,
	)
}

/// The longest match for the bytes at `pos` among the earlier positions on its hash chain: its length and distance.
fn longest(data: &[u8], pos: usize, head: &[u32], prev: &[u32], h: usize) -> (usize, usize) {
	let limit = (data.len() - pos).min(MAX_MATCH);
	let mut best = (0usize, 0usize);
	let mut candidate = head.get(h).copied().unwrap_or(NONE);
	let mut chain = 0;
	while candidate != NONE && chain < MAX_CHAIN {
		let at = candidate as usize;
		let dist = pos - at;
		if dist > WINDOW {
			break;
		}
		let mut len = 0;
		while len < limit && data.get(at + len) == data.get(pos + len) {
			len += 1;
		}
		if len > best.0 {
			best = (len, dist);
			if len >= GOOD_ENOUGH || len == limit {
				break;
			}
		}
		let next = prev.get(at % WINDOW).copied().unwrap_or(NONE);
		// A link that does not move backwards belongs to a slot the window has since reused.
		if next == NONE || (next as usize) >= at {
			break;
		}
		candidate = next;
		chain += 1;
	}
	best
}

/// Inserts `pos` in the hash chains.
fn insert(data: &[u8], pos: usize, head: &mut [u32], prev: &mut [u32]) {
	if let (Some(&a), Some(&b), Some(&c)) = (data.get(pos), data.get(pos + 1), data.get(pos + 2)) {
		let h = hash(a, b, c);
		if let Some(slot) = prev.get_mut(pos % WINDOW) {
			*slot = head.get(h).copied().unwrap_or(NONE);
		}
		if let Some(slot) = head.get_mut(h) {
			*slot = pos as u32;
		}
	}
}

fn fixed_block(data: &[u8], out: &mut Vec<u8>) {
	let mut bits = Bits {
		out,
		acc: 0,
		count: 0,
	};
	// Final block, fixed Huffman codes.
	bits.put(1, 1);
	bits.put(1, 2);
	let mut head: Vec<u32> = alloc::vec![NONE; 1 << HASH_BITS];
	let mut prev: Vec<u32> = alloc::vec![NONE; WINDOW];
	let mut pos = 0;
	while pos < data.len() {
		let (mut len, mut dist) = (0, 0);
		if let (Some(&a), Some(&b), Some(&c)) =
			(data.get(pos), data.get(pos + 1), data.get(pos + 2))
		{
			let h = hash(a, b, c);
			(len, dist) = longest(data, pos, &head, &prev, h);
		}
		if len >= MIN_MATCH {
			let (sym, extra_bits, extra) = length_symbol(len);
			bits.symbol(sym);
			bits.put(extra, extra_bits);
			let (dsym, dbits, dextra) = dist_symbol(dist);
			bits.code(dsym, 5);
			bits.put(dextra, dbits);
			for p in pos..pos + len {
				insert(data, p, &mut head, &mut prev);
			}
			pos += len;
		} else {
			bits.symbol(u32::from(data.get(pos).copied().unwrap_or(0)));
			insert(data, pos, &mut head, &mut prev);
			pos += 1;
		}
	}
	bits.symbol(256);
	bits.flush();
}

fn stored_blocks(data: &[u8], out: &mut Vec<u8>) {
	if data.is_empty() {
		out.extend_from_slice(&[1, 0, 0, 0xFF, 0xFF]);
		return;
	}
	let mut chunks = data.chunks(65535).peekable();
	while let Some(chunk) = chunks.next() {
		out.push(u8::from(chunks.peek().is_none()));
		let n = chunk.len() as u16;
		out.extend_from_slice(&n.to_le_bytes());
		out.extend_from_slice(&(!n).to_le_bytes());
		out.extend_from_slice(chunk);
	}
}

/// Appends `data` as a zlib stream to `out`.
pub fn zlib(data: &[u8], out: &mut Vec<u8>) {
	out.extend_from_slice(&[0x78, 0x01]);
	let start = out.len();
	fixed_block(data, out);
	let stored_size = data.len() + 5 * data.len().div_ceil(65535).max(1);
	if out.len() - start > stored_size {
		out.truncate(start);
		stored_blocks(data, out);
	}
	out.extend_from_slice(&adler32(data).to_be_bytes());
}
