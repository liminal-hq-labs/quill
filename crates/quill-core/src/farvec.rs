// FarVec: a growable array of fixed-size records spread over Store chunks.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::marker::PhantomData;

use crate::bytes::{Pod, Reader, Writer, MAX_POD};
use crate::store::{ChunkId, Store, MAX_CHUNK};
use crate::{Error, Result};

/// How many spare chunks a vector can hold for a reserved run of inserts.
pub const SPARES: usize = 16;

/// A spare chunk's id, or none, in two bytes (an `Option<ChunkId>` takes four, and a vector holds sixteen of them).
#[derive(Copy, Clone)]
struct Spare(u16);

impl Spare {
	const NONE: Spare = Spare(u16::MAX);

	const fn of(c: ChunkId) -> Spare {
		Spare(c.0)
	}

	const fn is_none(&self) -> bool {
		self.0 == u16::MAX
	}

	fn take(&mut self) -> Option<ChunkId> {
		let id = (!self.is_none()).then_some(ChunkId(self.0));
		self.0 = u16::MAX;
		id
	}
}

/// A growable array on top of a [`Store`]. Elements never straddle a chunk. `CH` is the most chunks it can hold.
///
/// The vector does not own the store: every call that touches memory takes it, so many vectors share one store.
pub struct FarVec<T: Pod, const CH: usize> {
	chunks: [ChunkId; CH],
	counts: [u16; CH],
	nchunks: usize,
	len: u32,
	per: u16,
	spares: [Spare; SPARES],
	_t: PhantomData<T>,
}

impl<T: Pod, const CH: usize> Default for FarVec<T, CH> {
	fn default() -> Self {
		Self::new()
	}
}

impl<T: Pod, const CH: usize> FarVec<T, CH> {
	/// An empty vector with the full 32 KB chunk size.
	pub const fn new() -> Self {
		Self::with_chunk_bytes(MAX_CHUNK)
	}

	/// An empty vector whose chunks hold `bytes` bytes (at least one element, at most 32 KB). Tests use small chunks to
	/// meet chunk edges quickly.
	pub const fn with_chunk_bytes(bytes: usize) -> Self {
		let bytes = if bytes > MAX_CHUNK { MAX_CHUNK } else { bytes };
		let mut per = bytes / T::SIZE;
		if per == 0 {
			per = 1;
		}
		FarVec {
			chunks: [ChunkId(0); CH],
			counts: [0; CH],
			nchunks: 0,
			len: 0,
			per: per as u16,
			spares: [Spare::NONE; SPARES],
			_t: PhantomData,
		}
	}

	/// Number of elements.
	pub const fn len(&self) -> u32 {
		self.len
	}

	/// True when there are no elements.
	pub const fn is_empty(&self) -> bool {
		self.len == 0
	}

	fn chunk_bytes(&self) -> u16 {
		(usize::from(self.per) * T::SIZE) as u16
	}

	fn size16() -> u16 {
		T::SIZE as u16
	}

	/// Finds the chunk and element offset of element `i`, which must exist.
	fn locate(&self, i: u32) -> Option<(usize, u16)> {
		let mut acc = 0u32;
		for ci in 0..self.nchunks {
			let c = u32::from(*self.counts.get(ci)?);
			if i < acc + c {
				return Some((ci, (i - acc) as u16));
			}
			acc += c;
		}
		None
	}

	/// Finds where to insert before element `i` (`i` may equal the length). A boundary prefers the earlier chunk.
	fn locate_insert(&self, i: u32) -> Option<(usize, u16)> {
		let mut acc = 0u32;
		for ci in 0..self.nchunks {
			let c = u32::from(*self.counts.get(ci)?);
			if i <= acc + c {
				return Some((ci, (i - acc) as u16));
			}
			acc += c;
		}
		None
	}

	fn take_chunk<S: Store + ?Sized>(&mut self, s: &mut S) -> Option<ChunkId> {
		for slot in self.spares.iter_mut() {
			if let Some(c) = slot.take() {
				return Some(c);
			}
		}
		s.alloc(self.chunk_bytes())
	}

	fn spare_count(&self) -> usize {
		self.spares.iter().filter(|s| !s.is_none()).count()
	}

	/// Makes sure `extra` inserts, together, cannot fail for lack of memory. On error nothing has changed.
	pub fn reserve<S: Store + ?Sized>(&mut self, s: &mut S, extra: u32) -> Result<()> {
		let needed = (extra.div_ceil(u32::from(self.per)) + 1) as usize;
		if needed > SPARES {
			return Err(Error::TooLarge);
		}
		let slots = CH - self.nchunks.min(CH);
		let want = needed.min(slots);
		if needed > slots {
			// The chunk table cannot hold the worst case of one split per insert, so every chunk must have room for the
			// whole run: then no insert meets a full chunk.
			let min_free = (0..self.nchunks)
				.map(|c| u32::from(self.per) - u32::from(self.counts.get(c).copied().unwrap_or(0)))
				.min()
				.unwrap_or(u32::from(self.per));
			if min_free < extra {
				return Err(Error::NoMemory);
			}
		}
		while self.spare_count() < want {
			let Some(c) = s.alloc(self.chunk_bytes()) else {
				return Err(Error::NoMemory);
			};
			match self.spares.iter_mut().find(|x| x.is_none()) {
				Some(slot) => *slot = Spare::of(c),
				None => {
					s.free(c);
					break;
				}
			}
		}
		Ok(())
	}

	/// Reads element `i`.
	pub fn get<S: Store + ?Sized>(&self, s: &S, i: u32) -> Option<T> {
		let (ci, off) = self.locate(i)?;
		let c = *self.chunks.get(ci)?;
		let mut buf = [0u8; MAX_POD];
		let b = buf.get_mut(..T::SIZE)?;
		s.read(c, off * Self::size16(), b);
		Some(T::read(&mut Reader::new(b)))
	}

	/// Replaces element `i`.
	pub fn set<S: Store + ?Sized>(&mut self, s: &mut S, i: u32, v: T) -> Result<()> {
		let (ci, off) = self.locate(i).ok_or(Error::Range)?;
		let c = *self.chunks.get(ci).ok_or(Error::Range)?;
		self.put(s, c, off, v)
	}

	fn put<S: Store + ?Sized>(&self, s: &mut S, c: ChunkId, off: u16, v: T) -> Result<()> {
		let mut buf = [0u8; MAX_POD];
		let b = buf.get_mut(..T::SIZE).ok_or(Error::Range)?;
		v.write(&mut Writer::new(b));
		s.write(c, off * Self::size16(), b);
		Ok(())
	}

	/// Appends an element.
	pub fn push<S: Store + ?Sized>(&mut self, s: &mut S, v: T) -> Result<()> {
		self.insert(s, self.len, v)
	}

	/// Inserts before element `i` (`i` may be the length). On error the vector is unchanged.
	pub fn insert<S: Store + ?Sized>(&mut self, s: &mut S, i: u32, v: T) -> Result<()> {
		if i > self.len {
			return Err(Error::Range);
		}
		if self.nchunks == 0 {
			if CH == 0 {
				return Err(Error::NoMemory);
			}
			let c = self.take_chunk(s).ok_or(Error::NoMemory)?;
			if let Some(x) = self.chunks.first_mut() {
				*x = c;
			}
			if let Some(x) = self.counts.first_mut() {
				*x = 0;
			}
			self.nchunks = 1;
		}
		let (mut ci, mut off) = self.locate_insert(i).ok_or(Error::Range)?;
		if self.counts.get(ci).copied() == Some(self.per) {
			// The chunk is full: open a new chunk after it and move the tail there.
			if self.nchunks >= CH {
				return Err(Error::NoMemory);
			}
			let n = self.take_chunk(s).ok_or(Error::NoMemory)?;
			let count = self.counts.get(ci).copied().unwrap_or(0);
			let c = self.chunks.get(ci).copied().ok_or(Error::Range)?;
			self.chunks.copy_within(ci + 1..self.nchunks, ci + 2);
			self.counts.copy_within(ci + 1..self.nchunks, ci + 2);
			self.nchunks += 1;
			let tail = count - off;
			if tail > 0 {
				s.copy((c, off * Self::size16()), (n, 0), tail * Self::size16());
			}
			if let Some(x) = self.chunks.get_mut(ci + 1) {
				*x = n;
			}
			if let Some(x) = self.counts.get_mut(ci + 1) {
				*x = tail;
			}
			if let Some(x) = self.counts.get_mut(ci) {
				*x = off;
			}
			if off == self.per {
				ci += 1;
				off = 0;
			}
		}
		let c = self.chunks.get(ci).copied().ok_or(Error::Range)?;
		let count = self.counts.get(ci).copied().unwrap_or(0);
		let tail = count - off;
		if tail > 0 {
			let sz = Self::size16();
			s.copy((c, off * sz), (c, (off + 1) * sz), tail * sz);
		}
		self.put(s, c, off, v)?;
		if let Some(x) = self.counts.get_mut(ci) {
			*x = count + 1;
		}
		self.len += 1;
		Ok(())
	}

	/// Removes and returns element `i`.
	pub fn remove<S: Store + ?Sized>(&mut self, s: &mut S, i: u32) -> Option<T> {
		let (ci, off) = self.locate(i)?;
		let v = self.get(s, i)?;
		let c = *self.chunks.get(ci)?;
		let count = *self.counts.get(ci)?;
		let tail = count - off - 1;
		let sz = Self::size16();
		if tail > 0 {
			s.copy((c, (off + 1) * sz), (c, off * sz), tail * sz);
		}
		if let Some(x) = self.counts.get_mut(ci) {
			*x = count - 1;
		}
		self.len -= 1;
		if count == 1 && self.nchunks > 1 {
			s.free(c);
			self.chunks.copy_within(ci + 1..self.nchunks, ci);
			self.counts.copy_within(ci + 1..self.nchunks, ci);
			self.nchunks -= 1;
		}
		Some(v)
	}

	/// Removes and returns the last element.
	pub fn pop<S: Store + ?Sized>(&mut self, s: &mut S) -> Option<T> {
		let n = self.len.checked_sub(1)?;
		self.remove(s, n)
	}

	/// Frees every chunk, spares included, and empties the vector.
	pub fn clear<S: Store + ?Sized>(&mut self, s: &mut S) {
		for ci in 0..self.nchunks {
			if let Some(c) = self.chunks.get(ci) {
				s.free(*c);
			}
		}
		for slot in self.spares.iter_mut() {
			if let Some(c) = slot.take() {
				s.free(c);
			}
		}
		self.nchunks = 0;
		self.len = 0;
		self.counts = [0; CH];
	}

	/// Number of chunks in use (for tests and size reports).
	pub const fn chunk_count(&self) -> usize {
		self.nchunks
	}
}
