// A store that two borrowers use in turn: how a clipboard kept in the document's own store can be written while the document is read.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::cell::RefCell;
use core::ops::Range;

use quill_core::doc::Doc;
use quill_core::store::{ChunkId, DynStore, ReadStore, Store};
use quill_text::codepage::CodePage;

use crate::convert::{build, empty_doc, put_all};
use crate::{paste_fragment, ClipError, Flavour, Flavours, Held};

/// One handle on a store behind a `RefCell`. Every call borrows the store for its own length and never calls back out, so two
/// handles never overlap; a call that finds it borrowed does nothing (an allocation fails, a read gives zeros).
struct Shared<'a, 'b>(&'a RefCell<&'b mut dyn Store>);

impl ReadStore for Shared<'_, '_> {
	fn read(&self, id: ChunkId, offset: u16, out: &mut [u8]) {
		match self.0.try_borrow() {
			Ok(s) => s.read(id, offset, out),
			Err(_) => out.fill(0),
		}
	}

	fn to_unicode(&self, cp: CodePage, byte: u8) -> char {
		match self.0.try_borrow() {
			Ok(s) => s.to_unicode(cp, byte),
			// Never borrowed here; the tables are not named so that a build that serves them from a resource does not link them.
			Err(_) => '?',
		}
	}
}

impl Store for Shared<'_, '_> {
	fn alloc(&mut self, len: u16) -> Option<ChunkId> {
		self.0.try_borrow_mut().ok()?.alloc(len)
	}

	fn free(&mut self, id: ChunkId) {
		if let Ok(mut s) = self.0.try_borrow_mut() {
			s.free(id);
		}
	}

	fn write(&mut self, id: ChunkId, offset: u16, data: &[u8]) {
		if let Ok(mut s) = self.0.try_borrow_mut() {
			s.write(id, offset, data);
		}
	}

	fn copy(&mut self, from: (ChunkId, u16), to: (ChunkId, u16), len: u16) {
		if let Ok(mut s) = self.0.try_borrow_mut() {
			s.copy(from, to, len);
		}
	}
}

impl<const CH: usize, const DCH: usize> Held<CH, DCH> {
	/// [`copy`] into this clipboard, whose chunks are in the same store `s` as the document's. Emptied first, as `copy` does.
	///
	/// The selection is built into a scratch document with the store as any editing does; only the writing of the clipboard's bytes,
	/// which reads the scratch document while it adds chunks, goes through two handles on the one store.
	#[inline(never)]
	pub fn copy_from<S: Store + ?Sized>(
		&mut self,
		doc: &Doc,
		s: &mut S,
		range: Range<u32>,
		want: Flavours,
	) -> Result<(), ClipError> {
		if range.start >= range.end || range.end > doc.body.len() {
			return Err(ClipError::Empty);
		}
		self.clear(s);
		let mut scratch = empty_doc();
		let result = match build(doc, s, &range, &mut scratch) {
			Ok(chars) => {
				let mut dynamic = DynStore(&mut *s);
				let cell = RefCell::new(&mut dynamic as &mut dyn Store);
				let (reading, mut writing) = (Shared(&cell), Shared(&cell));
				let mut clip = self.over(&mut writing);
				put_all(doc, &reading, &range, &scratch, chars, want, &mut clip)
			}
			Err(e) => Err(e),
		};
		scratch.reset(s);
		result
	}

	/// [`paste_fragment`] from this clipboard, whose chunks are in the same store `s` as the fragment `out`: the reader of the data lends
	/// the clipboard's readers the store.
	#[inline(never)]
	pub fn paste_to<S: Store + ?Sized>(
		&mut self,
		prefer: &[Flavour],
		s: &mut S,
		out: &mut Doc,
	) -> Result<Flavour, ClipError> {
		paste_fragment(&mut self.reading(), prefer, s, out)
	}
}
