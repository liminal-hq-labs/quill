// The Store trait: chunks of at most 32 KB that the core copies values in and out of.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// The largest chunk a store hands out. A chunk never crosses a 64 KB selector boundary on Win16.
pub const MAX_CHUNK: usize = 32768;

/// A handle to one chunk.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct ChunkId(pub u16);

/// The reading half of a store: copying bytes out of a chunk. Everything that only looks at a document takes this.
pub trait ReadStore {
	/// Copies bytes out of a chunk. Bytes past the chunk read as zero.
	fn read(&self, id: ChunkId, offset: u16, out: &mut [u8]);

	/// The Unicode character for `byte` of a Windows code page, for the RTF reader (CORE-61). The default is `quill-text`'s
	/// table; a build that cannot keep the tables in its data segment (Win16) serves them from a resource instead.
	fn to_unicode(&self, cp: quill_text::codepage::CodePage, byte: u8) -> char {
		quill_text::codepage::to_unicode(cp, byte)
	}
}

/// Chunked document memory. The core never holds a reference into it: it copies.
pub trait Store: ReadStore {
	/// Allocates a chunk of at least `len` bytes (at most [`MAX_CHUNK`]). `None` when memory is short.
	fn alloc(&mut self, len: u16) -> Option<ChunkId>;
	/// Releases a chunk.
	fn free(&mut self, id: ChunkId);
	/// Copies bytes into a chunk. Bytes past the chunk are dropped.
	fn write(&mut self, id: ChunkId, offset: u16, data: &[u8]);
	/// Copies within or between chunks, with `memmove` semantics when the ranges overlap.
	fn copy(&mut self, from: (ChunkId, u16), to: (ChunkId, u16), len: u16);
}

/// A [`ReadStore`] seen through a trait object. A reader or writer that works on `&dyn ReadStore` is compiled once, whatever store
/// type a build uses, which keeps the executables of the retro builds small.
pub struct DynRead<'a, S: ReadStore + ?Sized>(pub &'a S);

impl<S: ReadStore + ?Sized> ReadStore for DynRead<'_, S> {
	fn read(&self, id: ChunkId, offset: u16, out: &mut [u8]) {
		self.0.read(id, offset, out);
	}

	fn to_unicode(&self, cp: quill_text::codepage::CodePage, byte: u8) -> char {
		self.0.to_unicode(cp, byte)
	}
}

/// A [`Store`] seen through a trait object, as [`DynRead`] is for reading.
pub struct DynStore<'a, S: Store + ?Sized>(pub &'a mut S);

impl<S: Store + ?Sized> ReadStore for DynStore<'_, S> {
	fn read(&self, id: ChunkId, offset: u16, out: &mut [u8]) {
		self.0.read(id, offset, out);
	}

	fn to_unicode(&self, cp: quill_text::codepage::CodePage, byte: u8) -> char {
		self.0.to_unicode(cp, byte)
	}
}

impl<S: Store + ?Sized> Store for DynStore<'_, S> {
	fn alloc(&mut self, len: u16) -> Option<ChunkId> {
		self.0.alloc(len)
	}

	fn free(&mut self, id: ChunkId) {
		self.0.free(id);
	}

	fn write(&mut self, id: ChunkId, offset: u16, data: &[u8]) {
		self.0.write(id, offset, data);
	}

	fn copy(&mut self, from: (ChunkId, u16), to: (ChunkId, u16), len: u16) {
		self.0.copy(from, to, len);
	}
}
