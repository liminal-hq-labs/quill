// StoreClipboard: the clipboard held in Store chunks, and `Held`, the same buffers over a store somebody else owns.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use quill_core::store::{ReadStore, Store};
use quill_core::text::ByteBuf;
use quill_formats::io::{Sink, Source};

use crate::{ClipWrite, Clipboard, Flavour, Flavours};

/// The most RTF the store clipboard holds, in bytes.
pub const MAX_RTF: u32 = 1024 * 1024;
/// The most text the store clipboard holds, in bytes.
pub const MAX_TEXT: u32 = 1024 * 1024;
/// The most DIB data the store clipboard holds, in bytes: a picture's own limit.
pub const MAX_DIB: u32 = quill_core::doc::MAX_PICTURE_BYTES;

/// Chunks of the stand-alone clipboard's RTF and text, and of its DIB.
const CHUNKS: usize = 256;

const fn limit(f: Flavour) -> u32 {
	match f {
		Flavour::Rtf => MAX_RTF,
		Flavour::Text1252 => MAX_TEXT,
		Flavour::Dib => MAX_DIB,
	}
}

/// The flavours of a clipboard, as chunk lists. The chunks themselves are in a [`Store`] that the caller passes to every call, so the
/// editor can keep a copy in its document's store (and a few hundred bytes of its own) instead of a buffer of its own.
///
/// `CH` is how many chunks the RTF and the text may take, and `DCH` how many the DIB may take (zero for a build that carries a picture
/// inside RTF only). Chunks are allocated as the data arrives, so an empty clipboard holds none.
pub struct Held<const CH: usize, const DCH: usize> {
	rtf: ByteBuf<CH>,
	text: ByteBuf<CH>,
	dib: ByteBuf<DCH>,
	ready: Flavours,
}

impl<const CH: usize, const DCH: usize> Held<CH, DCH> {
	/// An empty clipboard whose RTF and text chunks are `chunk` bytes (at most 32 KB) and whose DIB chunks are 16 KB.
	pub const fn new(chunk: usize) -> Self {
		Held {
			rtf: ByteBuf::new(chunk),
			text: ByteBuf::new(chunk),
			dib: ByteBuf::new(16384),
			ready: Flavours::NONE,
		}
	}

	/// The flavours that were committed and are still held.
	pub const fn ready(&self) -> Flavours {
		self.ready
	}

	/// True when nothing is held.
	pub const fn is_empty(&self) -> bool {
		self.ready.is_empty()
	}

	/// Bytes held for `f`, or zero when it is not held.
	pub fn len(&self, f: Flavour) -> u32 {
		if self.ready.has(f) {
			self.held(f)
		} else {
			0
		}
	}

	/// Frees every chunk.
	pub fn clear<S: Store + ?Sized>(&mut self, s: &mut S) {
		self.rtf.clear(s);
		self.text.clear(s);
		self.dib.clear(s);
		self.ready = Flavours::NONE;
	}

	fn drop_one<S: Store + ?Sized>(&mut self, s: &mut S, f: Flavour) {
		match f {
			Flavour::Rtf => self.rtf.clear(s),
			Flavour::Text1252 => self.text.clear(s),
			Flavour::Dib => self.dib.clear(s),
		}
		self.ready = Flavours(self.ready.0 & !f.bit());
	}

	fn held(&self, f: Flavour) -> u32 {
		match f {
			Flavour::Rtf => self.rtf.len(),
			Flavour::Text1252 => self.text.len(),
			Flavour::Dib => self.dib.len(),
		}
	}

	fn append<S: Store + ?Sized>(&mut self, s: &mut S, f: Flavour, data: &[u8]) -> bool {
		if u64::from(self.held(f)) + data.len() as u64 > u64::from(limit(f)) {
			return false;
		}
		match f {
			Flavour::Rtf => self.rtf.append(s, data),
			Flavour::Text1252 => self.text.append(s, data),
			Flavour::Dib => self.dib.append(s, data),
		}
		.is_ok()
	}

	fn read(&self, s: &dyn ReadStore, f: Flavour, pos: u32, out: &mut [u8]) {
		match f {
			Flavour::Rtf => self.rtf.read(s, pos, out),
			Flavour::Text1252 => self.text.read(s, pos, out),
			Flavour::Dib => self.dib.read(s, pos, out),
		}
	}

	/// Starts writing `f` into chunks of `s`, replacing what was held for it.
	pub fn put<'a, S: Store>(
		&'a mut self,
		s: &'a mut S,
		f: Flavour,
	) -> Option<StoreWriter<'a, S, CH, DCH>> {
		if f == Flavour::Dib && DCH == 0 {
			return None;
		}
		self.drop_one(s, f);
		Some(StoreWriter {
			store: s,
			held: self,
			flavour: f,
			done: false,
		})
	}

	/// Starts reading `f`, or `None` when it is not held. With no `store` the reader gets its bytes through [`Source::read_in`], from
	/// the store the reader of the data passes it; with one, from that.
	pub fn get<'a>(
		&'a self,
		store: Option<&'a dyn ReadStore>,
		f: Flavour,
	) -> Option<StoreReader<'a, CH, DCH>> {
		self.ready.has(f).then_some(StoreReader {
			store,
			held: self,
			flavour: f,
			pos: 0,
		})
	}

	/// This clipboard over the store `s`, as a [`Clipboard`] for a call that writes it.
	pub fn over<'a, S: Store>(&'a mut self, s: &'a mut S) -> HeldClip<'a, S, CH, DCH> {
		HeldClip {
			held: self,
			store: s,
		}
	}

	/// This clipboard as a [`Clipboard`] for a call that only reads it: the reader of the data lends the readers the store.
	pub fn reading(&mut self) -> HeldRead<'_, CH, DCH> {
		HeldRead { held: self }
	}
}

/// A [`Held`] clipboard together with the store its chunks are in.
pub struct HeldClip<'a, S: Store, const CH: usize, const DCH: usize> {
	held: &'a mut Held<CH, DCH>,
	store: &'a mut S,
}

impl<S: Store, const CH: usize, const DCH: usize> Clipboard for HeldClip<'_, S, CH, DCH> {
	type W<'a>
		= StoreWriter<'a, S, CH, DCH>
	where
		Self: 'a;
	type R<'a>
		= StoreReader<'a, CH, DCH>
	where
		Self: 'a;

	fn clear(&mut self) -> bool {
		self.held.clear(self.store);
		true
	}

	fn offers(&mut self) -> Flavours {
		self.held.ready
	}

	fn put(&mut self, f: Flavour) -> Option<Self::W<'_>> {
		self.held.put(self.store, f)
	}

	fn get(&mut self, f: Flavour) -> Option<Self::R<'_>> {
		self.held.get(Some(&*self.store), f)
	}
}

/// A [`Held`] clipboard that can only be read, from a reader that lends it the store (see [`Held::reading`]).
pub struct HeldRead<'a, const CH: usize, const DCH: usize> {
	held: &'a mut Held<CH, DCH>,
}

/// The writer of a clipboard that cannot be written. It cannot be made.
pub enum NoWriter {}

impl Sink for NoWriter {
	fn write(&mut self, _: &[u8]) -> bool {
		false
	}
}

impl ClipWrite for NoWriter {
	fn commit(self) -> bool {
		false
	}
}

impl<const CH: usize, const DCH: usize> Clipboard for HeldRead<'_, CH, DCH> {
	type W<'a>
		= NoWriter
	where
		Self: 'a;
	type R<'a>
		= StoreReader<'a, CH, DCH>
	where
		Self: 'a;

	fn clear(&mut self) -> bool {
		false
	}

	fn offers(&mut self) -> Flavours {
		self.held.ready
	}

	fn put(&mut self, _: Flavour) -> Option<Self::W<'_>> {
		None
	}

	fn get(&mut self, f: Flavour) -> Option<Self::R<'_>> {
		self.held.get(None, f)
	}
}

/// A clipboard that keeps its flavours in the chunks of a store of its own, so a copy outlives the document it came from and
/// costs the document nothing. DOS, the terminal build, the host tools and the tests use it, and a shell without a system
/// clipboard falls back to it.
pub struct StoreClipboard<S: Store> {
	store: S,
	held: Held<CHUNKS, CHUNKS>,
}

impl<S: Store> StoreClipboard<S> {
	/// An empty clipboard over `store`.
	pub const fn new(store: S) -> Self {
		StoreClipboard {
			store,
			held: Held::new(4096),
		}
	}

	/// The store, for leak checks.
	pub fn store(&self) -> &S {
		&self.store
	}

	/// Empties the clipboard and gives the store back.
	pub fn into_store(mut self) -> S {
		self.held.clear(&mut self.store);
		self.store
	}

	/// Bytes held for `f`, or zero when it is not on the clipboard.
	pub fn len(&self, f: Flavour) -> u32 {
		self.held.len(f)
	}

	/// True when nothing is on the clipboard.
	pub fn is_empty(&self) -> bool {
		self.held.is_empty()
	}
}

/// Writes one flavour into a [`Held`] clipboard; refuses bytes past the flavour's limit or the store's memory.
pub struct StoreWriter<'a, S: Store, const CH: usize = CHUNKS, const DCH: usize = CHUNKS> {
	store: &'a mut S,
	held: &'a mut Held<CH, DCH>,
	flavour: Flavour,
	done: bool,
}

impl<S: Store, const CH: usize, const DCH: usize> Sink for StoreWriter<'_, S, CH, DCH> {
	fn write(&mut self, data: &[u8]) -> bool {
		self.held.append(self.store, self.flavour, data)
	}
}

impl<S: Store, const CH: usize, const DCH: usize> ClipWrite for StoreWriter<'_, S, CH, DCH> {
	fn commit(mut self) -> bool {
		self.done = true;
		self.held.ready = self.held.ready.with(self.flavour);
		true
	}
}

impl<S: Store, const CH: usize, const DCH: usize> Drop for StoreWriter<'_, S, CH, DCH> {
	fn drop(&mut self) {
		if !self.done {
			self.held.drop_one(self.store, self.flavour);
		}
	}
}

/// Reads one flavour out of a [`Held`] clipboard.
pub struct StoreReader<'a, const CH: usize = CHUNKS, const DCH: usize = CHUNKS> {
	store: Option<&'a dyn ReadStore>,
	held: &'a Held<CH, DCH>,
	flavour: Flavour,
	pos: u32,
}

impl<const CH: usize, const DCH: usize> StoreReader<'_, CH, DCH> {
	fn take(&mut self, s: &dyn ReadStore, out: &mut [u8]) -> usize {
		let n = (self.held.held(self.flavour) - self.pos).min(out.len() as u32) as usize;
		if let Some(part) = out.get_mut(..n) {
			self.held.read(s, self.flavour, self.pos, part);
		}
		self.pos += n as u32;
		n
	}
}

impl<const CH: usize, const DCH: usize> Source for StoreReader<'_, CH, DCH> {
	fn read(&mut self, out: &mut [u8]) -> usize {
		match self.store {
			Some(s) => self.take(s, out),
			None => 0,
		}
	}

	fn read_in(&mut self, store: &dyn ReadStore, out: &mut [u8]) -> usize {
		self.take(self.store.unwrap_or(store), out)
	}
}

impl<S: Store> Clipboard for StoreClipboard<S> {
	type W<'a>
		= StoreWriter<'a, S>
	where
		Self: 'a;
	type R<'a>
		= StoreReader<'a>
	where
		Self: 'a;

	fn clear(&mut self) -> bool {
		self.held.clear(&mut self.store);
		true
	}

	fn offers(&mut self) -> Flavours {
		self.held.ready
	}

	fn put(&mut self, f: Flavour) -> Option<Self::W<'_>> {
		self.held.put(&mut self.store, f)
	}

	fn get(&mut self, f: Flavour) -> Option<Self::R<'_>> {
		self.held.get(Some(&self.store), f)
	}
}
