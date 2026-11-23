// A raw-deflate (RFC 1951) decoder that streams: bytes in through a closure, bytes out through a closure, a caller-owned 32 KB window.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Grown from Lt. Fizz 2's `fizz-pack` decoder (`~/source/experiments/fizz-2/crates/fizz-pack/src/inflate.rs`, which follows zlib's
// `puff.c`): the same canonical-code tables computed rather than stored, but the output no longer has to fit one buffer. Every byte
// goes to the sink and into a ring window that back-references read from, so a PNG row or a pack chunk can be consumed as it appears.

#![no_std]
#![forbid(unsafe_code)]
#![deny(
	clippy::unwrap_used,
	clippy::expect_used,
	clippy::panic,
	clippy::indexing_slicing
)]

/// The size of the window a stream may refer back into.
pub const WINDOW: usize = 32768;

/// Why a deflate stream was rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InflateError {
	/// The input ended before the final block did.
	Truncated,
	/// A block type of 3, or a stored block whose length and its complement disagree.
	BadBlock,
	/// An over-subscribed Huffman table, a code-length repeat with nothing to repeat, or a block with no end-of-block code.
	BadTable,
	/// A bit pattern that is no code, or a length or distance symbol that does not exist.
	BadSymbol,
	/// A back-reference that reaches before the start of the output.
	BadDistance,
	/// The sink refused a byte (it wanted no more).
	Stopped,
}

const MAX_BITS: usize = 15;
const MAX_LIT: usize = 288;

struct Bits<I: FnMut() -> Option<u8>> {
	next: I,
	buf: u32,
	cnt: u32,
}

impl<I: FnMut() -> Option<u8>> Bits<I> {
	fn take(&mut self, n: u32) -> Result<u32, InflateError> {
		while self.cnt < n {
			let b = (self.next)().ok_or(InflateError::Truncated)?;
			self.buf |= u32::from(b) << self.cnt;
			self.cnt += 8;
		}
		let v = self.buf & ((1u32 << n) - 1);
		self.buf >>= n;
		self.cnt -= n;
		Ok(v)
	}
}

struct Table<'t> {
	count: [u16; MAX_BITS + 1],
	symbol: &'t mut [u16],
}

impl Table<'_> {
	fn build(&mut self, lengths: &[u8]) -> Result<(), InflateError> {
		self.count = [0; MAX_BITS + 1];
		for &l in lengths {
			*self
				.count
				.get_mut(usize::from(l))
				.ok_or(InflateError::BadTable)? += 1;
		}
		let mut left = 1i32;
		let mut offs = [0u16; MAX_BITS + 2];
		for len in 1..=MAX_BITS {
			let c = *self.count.get(len).ok_or(InflateError::BadTable)?;
			left = (left << 1) - i32::from(c);
			if left < 0 {
				return Err(InflateError::BadTable);
			}
			let o = *offs.get(len).ok_or(InflateError::BadTable)?;
			*offs.get_mut(len + 1).ok_or(InflateError::BadTable)? = o + c;
		}
		for (sym, &l) in lengths.iter().enumerate() {
			if l != 0 {
				let o = offs.get_mut(usize::from(l)).ok_or(InflateError::BadTable)?;
				*self
					.symbol
					.get_mut(usize::from(*o))
					.ok_or(InflateError::BadTable)? = sym as u16;
				*o += 1;
			}
		}
		Ok(())
	}

	fn decode<I: FnMut() -> Option<u8>>(&self, bits: &mut Bits<I>) -> Result<u16, InflateError> {
		let (mut code, mut first, mut index) = (0i32, 0i32, 0i32);
		for len in 1..=MAX_BITS {
			code |= bits.take(1)? as i32;
			let count = i32::from(*self.count.get(len).ok_or(InflateError::BadSymbol)?);
			if code - count < first {
				return self
					.symbol
					.get((index + code - first) as usize)
					.copied()
					.ok_or(InflateError::BadSymbol);
			}
			index += count;
			first = (first + count) << 1;
			code <<= 1;
		}
		Err(InflateError::BadSymbol)
	}
}

fn base_extra(sym: u16, group: u32, first: u32) -> (u32, u32) {
	let extra = |i: u32| (i >> group).saturating_sub(1);
	let base = first + (0..u32::from(sym)).map(|i| 1u32 << extra(i)).sum::<u32>();
	(base, extra(u32::from(sym)))
}

struct Out<'w, O: FnMut(u8) -> bool> {
	emit: O,
	window: &'w mut [u8; WINDOW],
	at: usize,
	total: usize,
}

impl<O: FnMut(u8) -> bool> Out<'_, O> {
	fn put(&mut self, b: u8) -> Result<(), InflateError> {
		if let Some(w) = self.window.get_mut(self.at % WINDOW) {
			*w = b;
		}
		self.at += 1;
		self.total += 1;
		if (self.emit)(b) {
			Ok(())
		} else {
			Err(InflateError::Stopped)
		}
	}
}

/// Decodes the raw deflate stream that `next` supplies, calling `emit` with each output byte (it returns false to stop). `window` is
/// the caller's 32 KB ring. Returns the number of bytes produced.
pub fn inflate<I, O>(next: I, emit: O, window: &mut [u8; WINDOW]) -> Result<usize, InflateError>
where
	I: FnMut() -> Option<u8>,
	O: FnMut(u8) -> bool,
{
	let mut bits = Bits {
		next,
		buf: 0,
		cnt: 0,
	};
	let mut out = Out {
		emit,
		window,
		at: 0,
		total: 0,
	};
	loop {
		let last = bits.take(1)?;
		match bits.take(2)? {
			0 => stored(&mut bits, &mut out)?,
			kind @ (1 | 2) => {
				let mut lsym = [0u16; MAX_LIT];
				let mut dsym = [0u16; 32];
				let mut lit = Table {
					count: [0; MAX_BITS + 1],
					symbol: &mut lsym,
				};
				let mut dist = Table {
					count: [0; MAX_BITS + 1],
					symbol: &mut dsym,
				};
				if kind == 1 {
					let mut l = [8u8; MAX_LIT];
					for (i, v) in l.iter_mut().enumerate() {
						*v = match i {
							144..=255 => 9,
							256..=279 => 7,
							_ => 8,
						};
					}
					lit.build(&l)?;
					dist.build(&[5; 32])?;
				} else {
					dynamic(&mut bits, &mut lit, &mut dist)?;
				}
				codes(&mut bits, &lit, &dist, &mut out)?;
			}
			_ => return Err(InflateError::BadBlock),
		}
		if last == 1 {
			return Ok(out.total);
		}
	}
}

fn stored<I: FnMut() -> Option<u8>, O: FnMut(u8) -> bool>(
	bits: &mut Bits<I>,
	out: &mut Out<'_, O>,
) -> Result<(), InflateError> {
	bits.buf = 0;
	bits.cnt = 0;
	let mut h = [0u8; 4];
	for b in &mut h {
		*b = (bits.next)().ok_or(InflateError::Truncated)?;
	}
	let (len, nlen) = (
		u16::from_le_bytes([h[0], h[1]]),
		u16::from_le_bytes([h[2], h[3]]),
	);
	if len != !nlen {
		return Err(InflateError::BadBlock);
	}
	for _ in 0..len {
		let b = (bits.next)().ok_or(InflateError::Truncated)?;
		out.put(b)?;
	}
	Ok(())
}

fn dynamic<I: FnMut() -> Option<u8>>(
	bits: &mut Bits<I>,
	lit: &mut Table,
	dist: &mut Table,
) -> Result<(), InflateError> {
	const ORDER: [u8; 19] = [
		16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
	];
	let nlen = bits.take(5)? as usize + 257;
	let ndist = bits.take(5)? as usize + 1;
	let ncode = bits.take(4)? as usize + 4;
	if nlen > 286 || ndist > 30 {
		return Err(InflateError::BadTable);
	}
	let mut lengths = [0u8; 320];
	for &o in ORDER.iter().take(ncode) {
		*lengths
			.get_mut(usize::from(o))
			.ok_or(InflateError::BadTable)? = bits.take(3)? as u8;
	}
	lit.build(lengths.get(..19).ok_or(InflateError::BadTable)?)?;
	lengths = [0; 320];
	let mut i = 0;
	while i < nlen + ndist {
		let sym = lit.decode(bits)?;
		let (value, repeat) = match sym {
			0..=15 => (sym as u8, 1),
			16 => (
				*lengths
					.get(..i)
					.and_then(|s| s.last())
					.ok_or(InflateError::BadTable)?,
				3 + bits.take(2)?,
			),
			17 => (0, 3 + bits.take(3)?),
			_ => (0, 11 + bits.take(7)?),
		};
		let end = i + repeat as usize;
		lengths
			.get_mut(i..end)
			.filter(|_| end <= nlen + ndist)
			.ok_or(InflateError::BadTable)?
			.fill(value);
		i = end;
	}
	if lengths.get(256).copied() == Some(0) {
		return Err(InflateError::BadTable);
	}
	lit.build(lengths.get(..nlen).ok_or(InflateError::BadTable)?)?;
	dist.build(
		lengths
			.get(nlen..nlen + ndist)
			.ok_or(InflateError::BadTable)?,
	)
}

fn codes<I: FnMut() -> Option<u8>, O: FnMut(u8) -> bool>(
	bits: &mut Bits<I>,
	lit: &Table,
	dist: &Table,
	out: &mut Out<'_, O>,
) -> Result<(), InflateError> {
	loop {
		let sym = lit.decode(bits)?;
		if sym < 256 {
			out.put(sym as u8)?;
			continue;
		}
		if sym == 256 {
			return Ok(());
		}
		if sym > 285 {
			return Err(InflateError::BadSymbol);
		}
		let len = if sym == 285 {
			258
		} else {
			let (base, extra) = base_extra(sym - 257, 2, 3);
			base + bits.take(extra)?
		};
		let dsym = dist.decode(bits)?;
		if dsym > 29 {
			return Err(InflateError::BadSymbol);
		}
		let (base, extra) = base_extra(dsym, 1, 1);
		let d = (base + bits.take(extra)?) as usize;
		if d > out.at.min(WINDOW) {
			return Err(InflateError::BadDistance);
		}
		for _ in 0..len {
			let b = out
				.window
				.get((out.at - d) % WINDOW)
				.copied()
				.ok_or(InflateError::BadDistance)?;
			out.put(b)?;
		}
	}
}
