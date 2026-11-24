// StoreClipboard: the clipboard held in Store chunks.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use quill_core::store::Store;
use quill_core::text::ByteBuf;
use quill_formats::io::{Sink, Source};

use crate::{ClipWrite, Clipboard, Flavour, Flavours};

/// The most RTF the store clipboard holds, in bytes.
pub const MAX_RTF: u32 = 1024 * 1024;
/// The most text the store clipboard holds, in bytes.
pub const MAX_TEXT: u32 = 1024 * 1024;
/// The most DIB data the store clipboard holds, in bytes: a picture's own limit.
pub const MAX_DIB: u32 = quill_core::doc::MAX_PICTURE_BYTES;

const CHUNKS: usize = 256;
type Buf = ByteBuf<CHUNKS>;

const fn limit(f: Flavour) -> u32 {
	match f {
		Flavour::Rtf => MAX_RTF,
		Flavour::Text1252 => MAX_TEXT,
		Flavour::Dib => MAX_DIB,
	}
}

/// A clipboard that keeps its flavours in the chunks of a store of its own, so a copy outlives the document it came from and
/// costs the document nothing. DOS, the terminal build, the host tools and the tests use it, and a shell without a system
/// clipboard falls back to it.
pub struct StoreClipboard<S: Store> {
	store: S,
	rtf: Buf,
	text: Buf,
	dib: Buf,
	ready: Flavours,
}

impl<S: Store> StoreClipboard<S> {
	/// An empty clipboard over `store`.
	pub const fn new(store: S) -> Self {
		StoreClipboard {
			store,
			rtf: Buf::new(4096),
			text: Buf::new(4096),
			dib: Buf::new(16384),
			ready: Flavours::NONE,
		}
	}

	/// The store, for leak checks.
	pub fn store(&self) -> &S {
		&self.store
	}

	/// Empties the clipboard and gives the store back.
	pub fn into_store(mut self) -> S {
		Clipboard::clear(&mut self);
		self.store
	}

	/// Bytes held for `f`, or zero when it is not on the clipboard.
	pub fn len(&self, f: Flavour) -> u32 {
		if self.ready.has(f) {
			self.buf(f).len()
		} else {
			0
		}
	}

	/// True when nothing is on the clipboard.
	pub fn is_empty(&self) -> bool {
		self.ready.is_empty()
	}

	fn buf(&self, f: Flavour) -> &Buf {
		match f {
			Flavour::Rtf => &self.rtf,
			Flavour::Text1252 => &self.text,
			Flavour::Dib => &self.dib,
		}
	}
}

/// Writes one flavour into a [`StoreClipboard`]; refuses bytes past the flavour's limit or the store's memory.
pub struct StoreWriter<'a, S: Store> {
	store: &'a mut S,
	buf: &'a mut Buf,
	ready: &'a mut Flavours,
	flavour: Flavour,
	done: bool,
}

impl<S: Store> Sink for StoreWriter<'_, S> {
	fn write(&mut self, data: &[u8]) -> bool {
		if u64::from(self.buf.len()) + data.len() as u64 > u64::from(limit(self.flavour)) {
			return false;
		}
		self.buf.append(self.store, data).is_ok()
	}
}

impl<S: Store> ClipWrite for StoreWriter<'_, S> {
	fn commit(mut self) -> bool {
		self.done = true;
		*self.ready = self.ready.with(self.flavour);
		true
	}
}

impl<S: Store> Drop for StoreWriter<'_, S> {
	fn drop(&mut self) {
		if !self.done {
			self.buf.clear(self.store);
		}
	}
}

/// Reads one flavour out of a [`StoreClipboard`].
pub struct StoreReader<'a, S: Store> {
	store: &'a S,
	buf: &'a Buf,
	pos: u32,
}

impl<S: Store> Source for StoreReader<'_, S> {
	fn read(&mut self, out: &mut [u8]) -> usize {
		let n = (self.buf.len() - self.pos).min(out.len() as u32) as usize;
		if let Some(part) = out.get_mut(..n) {
			self.buf.read(self.store, self.pos, part);
		}
		self.pos += n as u32;
		n
	}
}

impl<S: Store> Clipboard for StoreClipboard<S> {
	type W<'a>
		= StoreWriter<'a, S>
	where
		Self: 'a;
	type R<'a>
		= StoreReader<'a, S>
	where
		Self: 'a;

	fn clear(&mut self) -> bool {
		self.rtf.clear(&mut self.store);
		self.text.clear(&mut self.store);
		self.dib.clear(&mut self.store);
		self.ready = Flavours::NONE;
		true
	}

	fn offers(&mut self) -> Flavours {
		self.ready
	}

	fn put(&mut self, f: Flavour) -> Option<Self::W<'_>> {
		let buf = match f {
			Flavour::Rtf => &mut self.rtf,
			Flavour::Text1252 => &mut self.text,
			Flavour::Dib => &mut self.dib,
		};
		buf.clear(&mut self.store);
		self.ready = Flavours(self.ready.0 & !f.bit());
		Some(StoreWriter {
			store: &mut self.store,
			buf,
			ready: &mut self.ready,
			flavour: f,
			done: false,
		})
	}

	fn get(&mut self, f: Flavour) -> Option<Self::R<'_>> {
		self.ready.has(f).then(|| StoreReader {
			store: &self.store,
			buf: self.buf(f),
			pos: 0,
		})
	}
}
