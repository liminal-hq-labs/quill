// A copy-on-write store (`CowStore`) and a read-only snapshot of it (`SnapView`): the prototype of J11.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::Arc;

use quill_core::doc::Doc;
use quill_core::store::{ChunkId, Store, MAX_CHUNK};

type Chunk = Arc<Vec<u8>>;

/// Chunks behind `Arc`s. A write copies a chunk only when a snapshot still shares it.
#[derive(Default)]
pub struct CowStore {
	chunks: Vec<Option<Chunk>>,
	free: Vec<u16>,
	/// Bytes copied because a snapshot shared the chunk.
	pub copied: u64,
	/// Chunks copied for the same reason.
	pub copies: u64,
	/// Chunks copied since `mark`, by index (so a chunk written many times counts once).
	touched: Vec<u16>,
}

/// A read-only view of a store as it was: the chunk table cloned, one `Arc` bump per chunk.
pub struct SnapView {
	chunks: Vec<Option<Chunk>>,
}

impl CowStore {
	/// An empty store.
	pub fn new() -> Self {
		Self::default()
	}

	/// The store as it is now, shared with the live one until the live one writes.
	pub fn snapshot(&self) -> SnapView {
		SnapView {
			chunks: self.chunks.clone(),
		}
	}

	/// Live chunks.
	pub fn live(&self) -> usize {
		self.chunks.iter().filter(|c| c.is_some()).count()
	}

	/// Chunks of the live store that a snapshot still shares.
	pub fn shared(&self) -> usize {
		self.chunks
			.iter()
			.flatten()
			.filter(|c| Arc::strong_count(c) > 1)
			.count()
	}

	/// Starts counting distinct chunks copied.
	pub fn mark(&mut self) {
		self.touched.clear();
	}

	/// Distinct chunks copied since `mark`.
	pub fn distinct_copies(&self) -> usize {
		self.touched.len()
	}

	fn write_at(&mut self, id: ChunkId, offset: u16, data: &[u8]) {
		if let Some(Some(c)) = self.chunks.get_mut(usize::from(id.0)) {
			if Arc::strong_count(c) > 1 {
				self.copied += c.len() as u64;
				self.copies += 1;
				if !self.touched.contains(&id.0) {
					self.touched.push(id.0);
				}
			}
			let b = Arc::make_mut(c);
			let start = usize::from(offset);
			if let Some(dst) = b.get_mut(start..) {
				let n = dst.len().min(data.len());
				dst[..n].copy_from_slice(&data[..n]);
			}
		}
	}
}

fn read_from(chunks: &[Option<Chunk>], id: ChunkId, offset: u16, out: &mut [u8]) {
	out.fill(0);
	if let Some(Some(b)) = chunks.get(usize::from(id.0)) {
		if let Some(src) = b.get(usize::from(offset)..) {
			let n = src.len().min(out.len());
			out[..n].copy_from_slice(&src[..n]);
		}
	}
}

impl Store for CowStore {
	fn alloc(&mut self, len: u16) -> Option<ChunkId> {
		if usize::from(len) > MAX_CHUNK {
			return None;
		}
		let block = Arc::new(vec![0u8; usize::from(len)]);
		if let Some(i) = self.free.pop() {
			self.chunks[usize::from(i)] = Some(block);
			return Some(ChunkId(i));
		}
		let i = u16::try_from(self.chunks.len()).ok()?;
		self.chunks.push(Some(block));
		Some(ChunkId(i))
	}

	fn free(&mut self, id: ChunkId) {
		if let Some(slot) = self.chunks.get_mut(usize::from(id.0)) {
			if slot.take().is_some() {
				self.free.push(id.0);
			}
		}
	}

	fn read(&self, id: ChunkId, offset: u16, out: &mut [u8]) {
		read_from(&self.chunks, id, offset, out);
	}

	fn write(&mut self, id: ChunkId, offset: u16, data: &[u8]) {
		self.write_at(id, offset, data);
	}

	fn copy(&mut self, from: (ChunkId, u16), to: (ChunkId, u16), len: u16) {
		let mut tmp = vec![0u8; usize::from(len)];
		self.read(from.0, from.1, &mut tmp);
		self.write_at(to.0, to.1, &tmp);
	}
}

/// A snapshot reads like a store. It never writes: J2's `ReadStore` split makes that a type, here the write calls do nothing.
impl Store for SnapView {
	fn alloc(&mut self, _: u16) -> Option<ChunkId> {
		None
	}
	fn free(&mut self, _: ChunkId) {}
	fn read(&self, id: ChunkId, offset: u16, out: &mut [u8]) {
		read_from(&self.chunks, id, offset, out);
	}
	fn write(&mut self, _: ChunkId, _: u16, _: &[u8]) {}
	fn copy(&mut self, _: (ChunkId, u16), _: (ChunkId, u16), _: u16) {}
}

/// A copy of the document's header: the chunk ids, counts and format tables, not the chunks. This is what `Doc::header_copy` will
/// be in J11; the spike cannot add it to `quill-core`, so it copies the bytes, and refuses to if `Doc` ever owns heap memory.
pub fn header_copy(doc: &Doc) -> Doc {
	assert!(
		!std::mem::needs_drop::<Doc>(),
		"Doc owns something: a bitwise header copy is no longer sound"
	);
	// SAFETY: `Doc` has no destructor (asserted above) and holds only integers, arrays of them and `Cell`s of integers; a bitwise
	// copy is a second, independent value, and the original is left as it was.
	unsafe { std::ptr::read(doc) }
}

/// The size of the header copy.
pub fn header_bytes() -> usize {
	std::mem::size_of::<Doc>()
}
