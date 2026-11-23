// The Store trait: chunks of at most 32 KB that the core copies values in and out of.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// The largest chunk a store hands out. A chunk never crosses a 64 KB selector boundary on Win16.
pub const MAX_CHUNK: usize = 32768;

/// A handle to one chunk.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct ChunkId(pub u16);

/// Chunked document memory. The core never holds a reference into it: it copies.
pub trait Store {
	/// Allocates a chunk of at least `len` bytes (at most [`MAX_CHUNK`]). `None` when memory is short.
	fn alloc(&mut self, len: u16) -> Option<ChunkId>;
	/// Releases a chunk.
	fn free(&mut self, id: ChunkId);
	/// Copies bytes out of a chunk. Bytes past the chunk read as zero.
	fn read(&self, id: ChunkId, offset: u16, out: &mut [u8]);
	/// Copies bytes into a chunk. Bytes past the chunk are dropped.
	fn write(&mut self, id: ChunkId, offset: u16, data: &[u8]);
	/// Copies within or between chunks, with `memmove` semantics when the ranges overlap.
	fn copy(&mut self, from: (ChunkId, u16), to: (ChunkId, u16), len: u16);
}
