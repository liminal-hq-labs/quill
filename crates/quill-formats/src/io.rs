// Byte-stream traits the readers and writers work over.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// Where a reader gets its bytes.
pub trait Source {
	/// Reads up to `out.len()` bytes and returns how many. Fewer than asked means the end of the input.
	fn read(&mut self, out: &mut [u8]) -> usize;

	/// [`Source::read`] for a source whose bytes are in the store the reader is building into (the editor's clipboard keeps its data in
	/// the document's chunks, and the reader holds the store for its own writes, so it lends the source a view of it). A source that
	/// has its bytes of its own ignores `store`.
	fn read_in(&mut self, store: &dyn quill_core::store::ReadStore, out: &mut [u8]) -> usize {
		let _ = store;
		self.read(out)
	}
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

/// The bytes a `Buffered` sink gathers before it calls the sink behind it.
pub const BUFFER_LEN: usize = 1024;

/// A sink that gathers small writes into one block of `BUFFER_LEN` bytes, so a writer that puts out a field at a time makes one call
/// to the disk for about every kilobyte. A write at least as large as the block goes straight through. The first refusal is kept: later
/// writes do nothing, and `finish` reports it.
pub struct Buffered<'a, S: Sink + ?Sized> {
	out: &'a mut S,
	buf: [u8; BUFFER_LEN],
	len: usize,
	total: u32,
	failed: bool,
}

impl<'a, S: Sink + ?Sized> Buffered<'a, S> {
	/// Buffers writes to `out`.
	pub fn new(out: &'a mut S) -> Self {
		Buffered {
			out,
			buf: [0; BUFFER_LEN],
			len: 0,
			total: 0,
			failed: false,
		}
	}

	fn flush(&mut self) -> bool {
		if self.len > 0 {
			let ok = self.buf.get(..self.len).is_some_and(|b| self.out.write(b));
			self.failed |= !ok;
			self.len = 0;
		}
		!self.failed
	}

	/// Writes what is left and returns how many bytes went through in all, or `None` if the sink behind refused any of them.
	pub fn finish(mut self) -> Option<u32> {
		self.flush().then_some(self.total)
	}
}

impl<S: Sink + ?Sized> Sink for Buffered<'_, S> {
	fn write(&mut self, data: &[u8]) -> bool {
		if self.failed {
			return false;
		}
		self.total = self.total.saturating_add(data.len() as u32);
		if self.len + data.len() <= BUFFER_LEN {
			if let Some(d) = self.buf.get_mut(self.len..self.len + data.len()) {
				d.copy_from_slice(data);
				self.len += data.len();
				return true;
			}
		}
		if !self.flush() {
			return false;
		}
		if data.len() >= BUFFER_LEN {
			self.failed |= !self.out.write(data);
		} else if let Some(d) = self.buf.get_mut(..data.len()) {
			d.copy_from_slice(data);
			self.len = data.len();
		}
		!self.failed
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	struct Calls(usize, usize, usize);

	impl Sink for Calls {
		fn write(&mut self, data: &[u8]) -> bool {
			self.0 += 1;
			self.1 += data.len();
			self.1 <= self.2
		}
	}

	#[test]
	fn small_writes_become_one_call_per_block() {
		let mut c = Calls(0, 0, usize::MAX);
		let mut b = Buffered::new(&mut c);
		for _ in 0..1000 {
			assert!(b.write(&[7; 10]));
		}
		assert_eq!(b.finish(), Some(10_000));
		assert_eq!(c.1, 10_000);
		assert!(c.0 <= 11, "{} calls", c.0);
	}

	#[test]
	fn a_large_write_goes_straight_through_after_what_was_gathered() {
		let mut c = Calls(0, 0, usize::MAX);
		let mut b = Buffered::new(&mut c);
		assert!(b.write(b"head"));
		assert!(b.write(&[1; 5000]));
		assert_eq!(b.finish(), Some(5004));
		assert_eq!((c.0, c.1), (2, 5004));
	}

	#[test]
	fn the_first_refusal_is_kept() {
		let mut c = Calls(0, 0, 1500);
		let mut b = Buffered::new(&mut c);
		let mut ok = true;
		for _ in 0..400 {
			ok &= b.write(&[1; 10]);
		}
		assert!(!ok);
		assert_eq!(b.finish(), None);
	}
}
