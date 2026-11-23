// Byte-stream traits the readers and writers work over.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// Where a reader gets its bytes.
pub trait Source {
	/// Reads up to `out.len()` bytes and returns how many. Fewer than asked means the end of the input.
	fn read(&mut self, out: &mut [u8]) -> usize;
}

/// Where a writer puts its bytes.
pub trait Sink {
	/// Writes every byte, or returns false (the disk is full).
	fn write(&mut self, data: &[u8]) -> bool;
}

/// A source over a byte slice.
pub struct SliceSource<'a> {
	data: &'a [u8],
}

impl<'a> SliceSource<'a> {
	/// Reads from the front of `data`.
	pub const fn new(data: &'a [u8]) -> Self {
		SliceSource { data }
	}
}

impl Source for SliceSource<'_> {
	fn read(&mut self, out: &mut [u8]) -> usize {
		let n = out.len().min(self.data.len());
		let (head, rest) = self.data.split_at(n);
		for (o, i) in out.iter_mut().zip(head) {
			*o = *i;
		}
		self.data = rest;
		n
	}
}

/// A sink that only counts.
#[derive(Default)]
pub struct Counter(pub u64);

impl Sink for Counter {
	fn write(&mut self, data: &[u8]) -> bool {
		self.0 += data.len() as u64;
		true
	}
}
