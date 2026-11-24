// The piece table: document text over two append-only buffers, with character formats on the pieces and an undo log.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::cell::Cell;

use crate::bytes::{Pod, Reader, Writer};
use crate::farvec::FarVec;
use crate::store::{ChunkId, ReadStore, Store, MAX_CHUNK};
use crate::{Error, Result};

/// The most characters a body story holds.
pub const MAX_TEXT: u32 = 1_048_575;

/// Where a piece's characters come from.
pub const SRC_ORIGINAL: u8 = 0;
/// The add buffer: text typed or pasted.
pub const SRC_ADD: u8 = 1;
/// An inline object: one `0x01` character, `start` indexes the object table.
pub const SRC_OBJECT: u8 = 2;

/// A run of characters from one buffer, with one character format.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Piece {
	/// `SRC_ORIGINAL`, `SRC_ADD` or `SRC_OBJECT`.
	pub source: u8,
	/// Offset in the buffer (or the object index).
	pub start: u32,
	/// Characters.
	pub len: u32,
	/// Character format index.
	pub cfmt: u16,
	/// Paragraph format index, used by the paragraph marks (`0x0D`) this piece holds.
	pub pfmt: u16,
}

impl Pod for Piece {
	const SIZE: usize = 16;
	fn write(&self, w: &mut Writer<'_>) {
		w.u8(self.source);
		w.u8(0);
		w.u16(self.cfmt);
		w.u16(self.pfmt);
		w.u16(0);
		w.u32(self.start);
		w.u32(self.len);
	}
	fn read(r: &mut Reader<'_>) -> Self {
		let source = r.u8();
		r.u8();
		let cfmt = r.u16();
		let pfmt = r.u16();
		r.u16();
		let start = r.u32();
		let len = r.u32();
		Piece {
			source,
			start,
			len,
			cfmt,
			pfmt,
		}
	}
}

/// An append-only byte buffer spread over chunks.
pub struct ByteBuf<const CH: usize> {
	chunks: [ChunkId; CH],
	n: usize,
	len: u32,
	chunk: u32,
}

impl<const CH: usize> ByteBuf<CH> {
	/// An empty buffer with chunks of `chunk_bytes` (at most 32 KB).
	pub const fn new(chunk_bytes: usize) -> Self {
		let c = if chunk_bytes > MAX_CHUNK {
			MAX_CHUNK
		} else if chunk_bytes == 0 {
			1
		} else {
			chunk_bytes
		};
		ByteBuf {
			chunks: [ChunkId(0); CH],
			n: 0,
			len: 0,
			chunk: c as u32,
		}
	}

	/// Bytes held.
	pub const fn len(&self) -> u32 {
		self.len
	}

	/// True when empty.
	pub const fn is_empty(&self) -> bool {
		self.len == 0
	}

	/// Appends `data` and returns its offset. On error the buffer is unchanged.
	pub fn append<S: Store + ?Sized>(&mut self, s: &mut S, data: &[u8]) -> Result<u32> {
		let start = self.len;
		let (n0, len0) = (self.n, self.len);
		let mut done = 0usize;
		while done < data.len() {
			let at = self.len;
			let ci = (at / self.chunk) as usize;
			let off = at % self.chunk;
			if ci >= self.n {
				if ci >= CH {
					self.rollback(s, n0, len0);
					return Err(Error::NoMemory);
				}
				let Some(c) = s.alloc(self.chunk as u16) else {
					self.rollback(s, n0, len0);
					return Err(Error::NoMemory);
				};
				if let Some(x) = self.chunks.get_mut(ci) {
					*x = c;
				}
				self.n = ci + 1;
			}
			let room = (self.chunk - off) as usize;
			let take = room.min(data.len() - done);
			let Some(c) = self.chunks.get(ci).copied() else {
				self.rollback(s, n0, len0);
				return Err(Error::Range);
			};
			if let Some(part) = data.get(done..done + take) {
				s.write(c, off as u16, part);
			}
			self.len += take as u32;
			done += take;
		}
		Ok(start)
	}

	fn rollback<S: Store + ?Sized>(&mut self, s: &mut S, n0: usize, len0: u32) {
		for ci in n0..self.n {
			if let Some(c) = self.chunks.get(ci) {
				s.free(*c);
			}
		}
		self.n = n0;
		self.len = len0;
	}

	/// Copies `out.len()` bytes starting at `pos`. Bytes past the end read as zero.
	pub fn read<S: ReadStore + ?Sized>(&self, s: &S, pos: u32, out: &mut [u8]) {
		let mut done = 0usize;
		while done < out.len() {
			let at = pos + done as u32;
			let ci = (at / self.chunk) as usize;
			let off = at % self.chunk;
			let take = ((self.chunk - off) as usize).min(out.len() - done);
			if let (Some(c), Some(part)) = (self.chunks.get(ci), out.get_mut(done..done + take)) {
				if ci < self.n {
					s.read(*c, off as u16, part);
				} else {
					part.fill(0);
				}
			}
			done += take;
		}
	}

	/// Cuts the buffer back to `len` bytes (no-op when it is already shorter), freeing chunks past it.
	pub fn truncate<S: Store + ?Sized>(&mut self, s: &mut S, len: u32) {
		if len >= self.len {
			return;
		}
		let keep = len.div_ceil(self.chunk) as usize;
		self.rollback(s, keep, len);
	}

	/// Overwrites bytes already held, starting at `pos`; whatever falls past the end is ignored.
	pub fn patch<S: Store + ?Sized>(&mut self, s: &mut S, pos: u32, data: &[u8]) {
		for (i, b) in data.iter().enumerate() {
			let at = pos + i as u32;
			if at >= self.len {
				return;
			}
			if let Some(c) = self.chunks.get((at / self.chunk) as usize) {
				s.write(*c, (at % self.chunk) as u16, core::slice::from_ref(b));
			}
		}
	}

	/// Frees every chunk.
	pub fn clear<S: Store + ?Sized>(&mut self, s: &mut S) {
		self.rollback(s, 0, 0);
	}
}

const TAG_BEGIN: u8 = 1;
const TAG_PIECE: u8 = 2;
const TAG_SPLICE: u8 = 3;
const TAG_RUN: u8 = 4;
const TAG_FMT: u8 = 5;

/// One undo or redo log record. Records of a step sit between its `Begin` and the next one.
#[derive(Copy, Clone, Debug)]
struct Rec {
	tag: u8,
	x: u8,
	y: u16,
	a: u32,
	b: u32,
	c: u32,
}

impl Pod for Rec {
	const SIZE: usize = 16;
	fn write(&self, w: &mut Writer<'_>) {
		w.u8(self.tag);
		w.u8(self.x);
		w.u16(self.y);
		w.u32(self.a);
		w.u32(self.b);
		w.u32(self.c);
	}
	fn read(r: &mut Reader<'_>) -> Self {
		let tag = r.u8();
		let x = r.u8();
		let y = r.u16();
		let a = r.u32();
		let b = r.u32();
		let c = r.u32();
		Rec { tag, x, y, a, b, c }
	}
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum Log {
	Undo,
	Redo,
}

/// What has changed in a story since the last [`Text::take_damage`]: the text before `lo` is as it was, and the text from `hi` on is
/// what was at `hi - delta` before, where `delta` is the new length less the length then. Between them anything may have changed
/// (a deletion leaves `lo == hi`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Damage {
	/// The first position that may have changed.
	pub lo: u32,
	/// The position the changed part ends at, in the text as it is now.
	pub hi: u32,
}

const CLEAN: (u32, u32) = (u32::MAX, 0);

/// A story's text: the piece table, its buffers and its history.
pub struct Text<const PC: usize, const BC: usize, const UC: usize> {
	orig: ByteBuf<BC>,
	add: ByteBuf<BC>,
	pieces: FarVec<Piece, PC>,
	len: u32,
	undo: FarVec<Rec, UC>,
	redo: FarVec<Rec, UC>,
	group: Option<u8>,
	group_begun: bool,
	cursor: Cell<(u32, u32)>,
	rev: u32,
	changed: (u32, u32),
}

impl<const PC: usize, const BC: usize, const UC: usize> Default for Text<PC, BC, UC> {
	fn default() -> Self {
		Self::new()
	}
}

impl<const PC: usize, const BC: usize, const UC: usize> Text<PC, BC, UC> {
	/// An empty text with 32 KB chunks.
	pub const fn new() -> Self {
		Self::with_chunk_bytes(MAX_CHUNK)
	}

	/// An empty text whose chunks hold `bytes` bytes. Tests use small chunks.
	pub const fn with_chunk_bytes(bytes: usize) -> Self {
		Text {
			orig: ByteBuf::new(bytes),
			add: ByteBuf::new(bytes),
			pieces: FarVec::with_chunk_bytes(bytes),
			len: 0,
			undo: FarVec::with_chunk_bytes(bytes),
			redo: FarVec::with_chunk_bytes(bytes),
			group: None,
			group_begun: false,
			cursor: Cell::new((0, 0)),
			rev: 0,
			changed: CLEAN,
		}
	}

	/// The story's revision: a number that changes (wrapping, by one) whenever the text, a format run or the objects in it change,
	/// and never otherwise. Undo history, groups and reads do not touch it. Two equal revisions of one story mean equal content.
	pub const fn revision(&self) -> u32 {
		self.rev
	}

	const fn touch(&mut self) {
		self.rev = self.rev.wrapping_add(1);
	}

	/// Says that `removed` characters at `at` were replaced by `inserted` (a format change replaces a range by itself). Changes made
	/// one after another merge into one range that covers them all.
	fn note(&mut self, at: u32, removed: u32, inserted: u32) {
		let new_hi = at.saturating_add(inserted);
		let (lo, hi) = self.changed;
		if lo == u32::MAX {
			self.changed = (at, new_hi);
			return;
		}
		let shifted = if hi <= at {
			hi
		} else if hi >= at.saturating_add(removed) {
			hi - removed + inserted
		} else {
			new_hi
		};
		self.changed = (lo.min(at), shifted.max(new_hi));
	}

	/// What has changed since the last [`Text::take_damage`], or `None` when nothing has.
	pub const fn damage(&self) -> Option<Damage> {
		let (lo, hi) = self.changed;
		if lo == u32::MAX {
			None
		} else {
			Some(Damage { lo, hi })
		}
	}

	/// Returns what has changed since the last call and starts again from nothing.
	pub const fn take_damage(&mut self) -> Option<Damage> {
		let c = self.damage();
		self.changed = CLEAN;
		c
	}

	/// Characters in the story.
	pub const fn len(&self) -> u32 {
		self.len
	}

	/// True when there is no text.
	pub const fn is_empty(&self) -> bool {
		self.len == 0
	}

	/// Number of pieces (for tests and reports).
	pub const fn piece_count(&self) -> u32 {
		self.pieces.len()
	}

	/// Replaces the text with `bytes` as one original piece, in formats `cfmt` and `pfmt`, and forgets the history.
	pub fn load<S: Store + ?Sized>(
		&mut self,
		s: &mut S,
		bytes: &[u8],
		cfmt: u16,
		pfmt: u16,
	) -> Result<()> {
		if bytes.len() > MAX_TEXT as usize {
			return Err(Error::TooLarge);
		}
		self.clear(s);
		if bytes.is_empty() {
			return Ok(());
		}
		self.pieces.reserve(s, 1)?;
		self.note(0, 0, bytes.len() as u32);
		let start = self.orig.append(s, bytes)?;
		self.pieces.push(
			s,
			Piece {
				source: SRC_ORIGINAL,
				start,
				len: bytes.len() as u32,
				cfmt,
				pfmt,
			},
		)?;
		self.len = bytes.len() as u32;
		Ok(())
	}

	/// Appends `bytes` to the original buffer as the last piece, without recording history. Used while loading a file.
	pub fn append_original<S: Store + ?Sized>(
		&mut self,
		s: &mut S,
		bytes: &[u8],
		cfmt: u16,
		pfmt: u16,
	) -> Result<()> {
		if bytes.is_empty() {
			return Ok(());
		}
		if bytes.len() as u64 + u64::from(self.len) > u64::from(MAX_TEXT) {
			return Err(Error::TooLarge);
		}
		self.pieces.reserve(s, 1)?;
		self.touch();
		self.note(self.len, 0, bytes.len() as u32);
		let start = self.orig.append(s, bytes)?;
		let at = self.pieces.len();
		self.pieces.push(
			s,
			Piece {
				source: SRC_ORIGINAL,
				start,
				len: bytes.len() as u32,
				cfmt,
				pfmt,
			},
		)?;
		self.len += bytes.len() as u32;
		if at > 0 {
			self.try_merge(s, at - 1);
		}
		self.cursor.set((0, 0));
		Ok(())
	}

	/// Appends one inline object piece without recording history. Used while loading a file.
	pub fn append_object<S: Store + ?Sized>(
		&mut self,
		s: &mut S,
		index: u32,
		cfmt: u16,
		pfmt: u16,
	) -> Result<()> {
		if self.len >= MAX_TEXT {
			return Err(Error::TooLarge);
		}
		self.pieces.reserve(s, 1)?;
		self.touch();
		self.note(self.len, 0, 1);
		self.pieces.push(
			s,
			Piece {
				source: SRC_OBJECT,
				start: index,
				len: 1,
				cfmt,
				pfmt,
			},
		)?;
		self.len += 1;
		self.cursor.set((0, 0));
		Ok(())
	}

	/// Turns the character at `pos` (a `0x01` placeholder read from a file) into an inline object piece, keeping its formats.
	pub fn convert_to_object<S: Store + ?Sized>(
		&mut self,
		s: &mut S,
		pos: u32,
		index: u32,
	) -> Result<()> {
		self.pieces.reserve(s, 3)?;
		self.touch();
		self.note(pos, 1, 1);
		let ia = self.split_at(s, pos)?;
		self.split_at(s, pos + 1)?;
		let p = self.piece(s, ia).ok_or(Error::Range)?;
		self.pieces.set(
			s,
			ia,
			Piece {
				source: SRC_OBJECT,
				start: index,
				len: 1,
				..p
			},
		)?;
		self.cursor.set((0, 0));
		Ok(())
	}

	/// Forgets undo and redo history.
	pub fn forget_history<S: Store + ?Sized>(&mut self, s: &mut S) {
		self.undo.clear(s);
		self.redo.clear(s);
		self.group = None;
		self.group_begun = false;
	}

	/// Frees everything, including the history.
	pub fn clear<S: Store + ?Sized>(&mut self, s: &mut S) {
		self.touch();
		self.note(0, self.len, 0);
		self.pieces.clear(s);
		self.orig.clear(s);
		self.add.clear(s);
		self.undo.clear(s);
		self.redo.clear(s);
		self.len = 0;
		self.group = None;
		self.group_begun = false;
		self.cursor.set((0, 0));
	}

	fn piece(&self, s: &(impl ReadStore + ?Sized), i: u32) -> Option<Piece> {
		self.pieces.get(s, i)
	}

	/// The piece holding character `pos`: its index, the piece and the position where it starts.
	fn find<S: ReadStore + ?Sized>(&self, s: &S, pos: u32) -> Option<(u32, Piece, u32)> {
		if pos >= self.len {
			return None;
		}
		let (ci, cs) = self.cursor.get();
		let (mut i, mut at) = if cs <= pos { (ci, cs) } else { (0, 0) };
		loop {
			let p = self.piece(s, i)?;
			if pos < at + p.len {
				self.cursor.set((i, at));
				return Some((i, p, at));
			}
			at += p.len;
			i += 1;
		}
	}

	/// Copies characters from `pos` into `out` and returns how many there were. Inline objects read as `0x01`.
	pub fn read<S: ReadStore + ?Sized>(&self, s: &S, pos: u32, out: &mut [u8]) -> usize {
		let mut done = 0usize;
		let Some((mut i, mut p, mut at)) = self.find(s, pos) else {
			return 0;
		};
		while done < out.len() {
			let off = pos + done as u32 - at;
			let take = ((p.len - off) as usize).min(out.len() - done);
			if let Some(part) = out.get_mut(done..done + take) {
				match p.source {
					SRC_ORIGINAL => self.orig.read(s, p.start + off, part),
					SRC_ADD => self.add.read(s, p.start + off, part),
					_ => part.fill(1),
				}
			}
			done += take;
			if pos + done as u32 >= self.len {
				break;
			}
			if done < out.len() {
				at += p.len;
				i += 1;
				match self.piece(s, i) {
					Some(n) => p = n,
					None => break,
				}
			}
		}
		done
	}

	/// The `start` of the inline object at `pos` (its kind in the high half), when the character there is one.
	pub fn object_at<S: ReadStore + ?Sized>(&self, s: &S, pos: u32) -> Option<u32> {
		self.find(s, pos)
			.filter(|(_, p, _)| p.source == SRC_OBJECT)
			.map(|(_, p, _)| p.start)
	}

	/// The character at `pos`.
	pub fn read_byte<S: ReadStore + ?Sized>(&self, s: &S, pos: u32) -> Option<u8> {
		let mut b = [0u8; 1];
		(self.read(s, pos, &mut b) == 1)
			.then_some(b)
			.and_then(|x| x.first().copied())
	}

	/// Calls `f(position, piece)` for each stretch of one format inside `[pos, pos + len)`, the pieces trimmed to it.
	pub fn for_each_run<S: ReadStore + ?Sized>(
		&self,
		s: &S,
		pos: u32,
		len: u32,
		mut f: impl FnMut(u32, Piece),
	) {
		let end = pos.saturating_add(len).min(self.len);
		let mut at = pos;
		while at < end {
			let Some((_, p, ps)) = self.find(s, at) else {
				return;
			};
			let off = at - ps;
			let take = (p.len - off).min(end - at);
			f(
				at,
				Piece {
					source: p.source,
					start: p.start + off,
					len: take,
					..p
				},
			);
			at += take;
		}
	}

	/// Copies the characters of one trimmed piece (as given to `for_each_run`) into `out`, starting `skip` into it.
	pub fn read_piece<S: ReadStore + ?Sized>(&self, s: &S, p: &Piece, skip: u32, out: &mut [u8]) {
		match p.source {
			SRC_ORIGINAL => self.orig.read(s, p.start + skip, out),
			SRC_ADD => self.add.read(s, p.start + skip, out),
			_ => out.fill(1),
		}
	}

	/// Makes a piece boundary at `pos` and returns the index of the piece that starts there (the piece count at the end).
	fn split_at<S: Store + ?Sized>(&mut self, s: &mut S, pos: u32) -> Result<u32> {
		if pos >= self.len {
			return Ok(self.pieces.len());
		}
		let (i, p, at) = self.find(s, pos).ok_or(Error::Range)?;
		self.cursor.set((0, 0));
		if at == pos {
			return Ok(i);
		}
		let cut = pos - at;
		let (a, b) = if p.source == SRC_OBJECT {
			return Err(Error::Range);
		} else {
			(
				Piece { len: cut, ..p },
				Piece {
					start: p.start + cut,
					len: p.len - cut,
					..p
				},
			)
		};
		self.pieces.set(s, i, a)?;
		self.pieces.insert(s, i + 1, b)?;
		Ok(i + 1)
	}

	/// Number of pieces that overlap `[pos, pos + len)`.
	fn overlapping<S: ReadStore + ?Sized>(&self, s: &S, pos: u32, len: u32) -> u32 {
		let mut n = 0;
		self.for_each_run(s, pos, len, |_, _| n += 1);
		n
	}

	fn log_mut(&mut self, which: Log) -> &mut FarVec<Rec, UC> {
		match which {
			Log::Undo => &mut self.undo,
			Log::Redo => &mut self.redo,
		}
	}

	fn push_log<S: Store + ?Sized>(&mut self, s: &mut S, which: Log, rec: Rec) -> Result<()> {
		self.log_mut(which).push(s, rec)
	}

	fn reserve_log<S: Store + ?Sized>(&mut self, s: &mut S, which: Log, extra: u32) -> Result<()> {
		if self.log_mut(which).reserve(s, extra).is_ok() {
			return Ok(());
		}
		// The history is full: drop it (oldest first would be better) rather than refuse the edit.
		self.log_mut(which).clear(s);
		self.log_mut(which).reserve(s, extra)
	}

	/// Starts a step of the undo log (if none is open) and returns nothing; all records until the next `Begin` undo together.
	fn begin_step<S: Store + ?Sized>(&mut self, s: &mut S, label: u8) -> Result<()> {
		if self.group.is_some() {
			if self.group_begun {
				return Ok(());
			}
			self.group_begun = true;
			let l = self.group.unwrap_or(label);
			return self.push_log(
				s,
				Log::Undo,
				Rec {
					tag: TAG_BEGIN,
					x: l,
					y: 0,
					a: 0,
					b: 0,
					c: 0,
				},
			);
		}
		self.push_log(
			s,
			Log::Undo,
			Rec {
				tag: TAG_BEGIN,
				x: label,
				y: 0,
				a: 0,
				b: 0,
				c: 0,
			},
		)
	}

	/// Removes `[pos, pos + del)` from the table, writing the removed pieces and no header to `dst`. Returns the index
	/// where the range was and how many pieces were removed. The caller has reserved.
	fn remove_range<S: Store + ?Sized>(
		&mut self,
		s: &mut S,
		pos: u32,
		del: u32,
		dst: Log,
	) -> Result<(u32, u32)> {
		let ia = self.split_at(s, pos)?;
		let ib = self.split_at(s, pos + del)?;
		for i in ia..ib {
			let p = self.piece(s, i).ok_or(Error::Range)?;
			self.push_log(
				s,
				dst,
				Rec {
					tag: TAG_PIECE,
					x: p.source,
					y: p.cfmt,
					a: p.start,
					b: p.len,
					c: u32::from(p.pfmt),
				},
			)?;
		}
		for _ in ia..ib {
			self.pieces.remove(s, ia);
		}
		self.len -= del;
		self.cursor.set((0, 0));
		Ok((ia, ib - ia))
	}

	fn insert_piece<S: Store + ?Sized>(&mut self, s: &mut S, idx: u32, p: Piece) -> Result<()> {
		self.pieces.insert(s, idx, p)?;
		self.len += p.len;
		self.cursor.set((0, 0));
		Ok(())
	}

	/// Joins piece `i` with the next when they are neighbours in the same buffer with the same format.
	fn try_merge<S: Store + ?Sized>(&mut self, s: &mut S, i: u32) {
		let (Some(a), Some(b)) = (self.piece(s, i), self.piece(s, i + 1)) else {
			return;
		};
		if a.source == b.source
			&& a.source != SRC_OBJECT
			&& a.cfmt == b.cfmt
			&& a.pfmt == b.pfmt
			&& a.start + a.len == b.start
			&& self
				.pieces
				.set(
					s,
					i,
					Piece {
						len: a.len + b.len,
						..a
					},
				)
				.is_ok()
		{
			self.pieces.remove(s, i + 1);
			self.cursor.set((0, 0));
		}
	}

	/// Inserts `bytes` at `pos` in format `cfmt`. On error the text is as it was.
	pub fn insert<S: Store + ?Sized>(
		&mut self,
		s: &mut S,
		pos: u32,
		bytes: &[u8],
		cfmt: u16,
		pfmt: u16,
		label: u8,
	) -> Result<()> {
		if bytes.is_empty() {
			return Ok(());
		}
		if pos > self.len {
			return Err(Error::Range);
		}
		if bytes.len() as u64 + u64::from(self.len) > u64::from(MAX_TEXT) {
			return Err(Error::TooLarge);
		}
		self.pieces.reserve(s, 3)?;
		self.reserve_log(s, Log::Undo, 3)?;
		let start = self.add.append(s, bytes)?;
		self.touch();
		self.note(pos, 0, bytes.len() as u32);
		self.redo.clear(s);
		self.begin_step(s, label)?;
		let (ia, _) = self.remove_range(s, pos, 0, Log::Undo)?;
		let n = bytes.len() as u32;
		self.insert_piece(
			s,
			ia,
			Piece {
				source: SRC_ADD,
				start,
				len: n,
				cfmt,
				pfmt,
			},
		)?;
		self.push_log(
			s,
			Log::Undo,
			Rec {
				tag: TAG_SPLICE,
				x: 0,
				y: 0,
				a: pos,
				b: n,
				c: 0,
			},
		)?;
		self.try_merge(s, ia);
		if ia > 0 {
			self.try_merge(s, ia - 1);
		}
		Ok(())
	}

	/// Inserts one inline object (a picture or a field) at `pos`.
	pub fn insert_object<S: Store + ?Sized>(
		&mut self,
		s: &mut S,
		pos: u32,
		index: u32,
		cfmt: u16,
		pfmt: u16,
		label: u8,
	) -> Result<()> {
		if pos > self.len {
			return Err(Error::Range);
		}
		if self.len >= MAX_TEXT {
			return Err(Error::TooLarge);
		}
		self.pieces.reserve(s, 3)?;
		self.reserve_log(s, Log::Undo, 3)?;
		self.touch();
		self.note(pos, 0, 1);
		self.redo.clear(s);
		self.begin_step(s, label)?;
		let (ia, _) = self.remove_range(s, pos, 0, Log::Undo)?;
		self.insert_piece(
			s,
			ia,
			Piece {
				source: SRC_OBJECT,
				start: index,
				len: 1,
				cfmt,
				pfmt,
			},
		)?;
		self.push_log(
			s,
			Log::Undo,
			Rec {
				tag: TAG_SPLICE,
				x: 0,
				y: 0,
				a: pos,
				b: 1,
				c: 0,
			},
		)
	}

	/// Deletes `[pos, pos + len)`. On error the text is as it was.
	pub fn delete<S: Store + ?Sized>(
		&mut self,
		s: &mut S,
		pos: u32,
		len: u32,
		label: u8,
	) -> Result<()> {
		if len == 0 {
			return Ok(());
		}
		if pos.checked_add(len).is_none_or(|e| e > self.len) {
			return Err(Error::Range);
		}
		let k = self.overlapping(s, pos, len);
		self.pieces.reserve(s, 3)?;
		self.reserve_log(s, Log::Undo, k + 3)?;
		self.touch();
		self.note(pos, len, 0);
		self.redo.clear(s);
		self.begin_step(s, label)?;
		let (ia, n) = self.remove_range(s, pos, len, Log::Undo)?;
		self.push_log(
			s,
			Log::Undo,
			Rec {
				tag: TAG_SPLICE,
				x: 0,
				y: 0,
				a: pos,
				b: 0,
				c: n,
			},
		)?;
		if ia > 0 {
			self.try_merge(s, ia - 1);
		}
		Ok(())
	}

	/// Sets the character format of `[pos, pos + len)`.
	pub fn set_cfmt<S: Store + ?Sized>(
		&mut self,
		s: &mut S,
		pos: u32,
		len: u32,
		cfmt: u16,
		label: u8,
	) -> Result<()> {
		self.set_fmt(s, pos, len, Change::Char(cfmt), label)
	}

	/// Sets the paragraph format carried by the paragraph marks in `[pos, pos + len)`. Pass whole paragraphs: the
	/// format of a paragraph is the one on its mark's piece.
	pub fn set_pfmt<S: Store + ?Sized>(
		&mut self,
		s: &mut S,
		pos: u32,
		len: u32,
		pfmt: u16,
		label: u8,
	) -> Result<()> {
		self.set_fmt(s, pos, len, Change::Para(pfmt), label)
	}

	/// The paragraph format of the paragraph mark at `pos`.
	pub fn pfmt_at<S: ReadStore + ?Sized>(&self, s: &S, pos: u32) -> Option<u16> {
		self.find(s, pos).map(|(_, p, _)| p.pfmt)
	}

	/// The character format of the character at `pos`.
	pub fn cfmt_at<S: ReadStore + ?Sized>(&self, s: &S, pos: u32) -> Option<u16> {
		self.find(s, pos).map(|(_, p, _)| p.cfmt)
	}

	fn set_fmt<S: Store + ?Sized>(
		&mut self,
		s: &mut S,
		pos: u32,
		len: u32,
		change: Change,
		label: u8,
	) -> Result<()> {
		if len == 0 {
			return Ok(());
		}
		if pos.checked_add(len).is_none_or(|e| e > self.len) {
			return Err(Error::Range);
		}
		let k = self.overlapping(s, pos, len);
		self.pieces.reserve(s, 3)?;
		self.reserve_log(s, Log::Undo, k + 3)?;
		self.touch();
		self.note(pos, len, len);
		self.redo.clear(s);
		self.begin_step(s, label)?;
		self.fmt_range(s, pos, len, Log::Undo, change)
	}

	/// Sets formats over a range and logs the old runs (last run first, so a pop gives them in text order), then the header.
	fn fmt_range<S: Store + ?Sized>(
		&mut self,
		s: &mut S,
		pos: u32,
		len: u32,
		dst: Log,
		change: Change,
	) -> Result<()> {
		let ia = self.split_at(s, pos)?;
		let ib = self.split_at(s, pos + len)?;
		for i in (ia..ib).rev() {
			let p = self.piece(s, i).ok_or(Error::Range)?;
			self.push_log(
				s,
				dst,
				Rec {
					tag: TAG_RUN,
					x: 0,
					y: p.cfmt,
					a: p.len,
					b: u32::from(p.pfmt),
					c: 0,
				},
			)?;
		}
		for i in ia..ib {
			let p = self.piece(s, i).ok_or(Error::Range)?;
			let q = match change {
				Change::Char(c) => Piece { cfmt: c, ..p },
				Change::Para(f) => Piece { pfmt: f, ..p },
			};
			self.pieces.set(s, i, q)?;
		}
		self.push_log(
			s,
			dst,
			Rec {
				tag: TAG_FMT,
				x: 0,
				y: 0,
				a: pos,
				b: 0,
				c: ib - ia,
			},
		)?;
		for i in (ia.saturating_sub(1)..=ib).rev() {
			self.try_merge(s, i);
		}
		self.cursor.set((0, 0));
		Ok(())
	}

	/// Opens a group: every edit until `end_group` undoes as one step named `label`.
	pub fn begin_group(&mut self, label: u8) {
		self.group = Some(label);
		self.group_begun = false;
	}

	/// Closes the open group.
	pub fn end_group(&mut self) {
		self.group = None;
		self.group_begun = false;
	}

	/// True when there is a step to undo.
	pub fn can_undo(&self) -> bool {
		!self.undo.is_empty()
	}

	/// True when there is a step to redo.
	pub fn can_redo(&self) -> bool {
		!self.redo.is_empty()
	}

	/// The label of the step Undo would undo.
	pub fn undo_label<S: ReadStore + ?Sized>(&self, s: &S) -> Option<u8> {
		Self::top_label(&self.undo, s)
	}

	/// The label of the step Redo would redo.
	pub fn redo_label<S: ReadStore + ?Sized>(&self, s: &S) -> Option<u8> {
		Self::top_label(&self.redo, s)
	}

	fn top_label<S: ReadStore + ?Sized>(log: &FarVec<Rec, UC>, s: &S) -> Option<u8> {
		let mut i = log.len();
		while i > 0 {
			i -= 1;
			let r = log.get(s, i)?;
			if r.tag == TAG_BEGIN {
				return Some(r.x);
			}
		}
		None
	}

	/// Undoes the latest step. Returns false when there is nothing to undo.
	pub fn undo<S: Store + ?Sized>(&mut self, s: &mut S) -> Result<bool> {
		self.replay(s, Log::Undo, Log::Redo)
	}

	/// Redoes the latest undone step. Returns false when there is nothing to redo.
	pub fn redo<S: Store + ?Sized>(&mut self, s: &mut S) -> Result<bool> {
		self.replay(s, Log::Redo, Log::Undo)
	}

	/// Pops one step from `from`, applying each record's inverse and writing the inverses to `to`.
	fn replay<S: Store + ?Sized>(&mut self, s: &mut S, from: Log, to: Log) -> Result<bool> {
		self.end_group();
		let Some(label) = Self::top_label(
			if from == Log::Undo {
				&self.undo
			} else {
				&self.redo
			},
			s,
		) else {
			return Ok(false);
		};
		// Size the reservation from the step itself.
		let n = {
			let log = if from == Log::Undo {
				&self.undo
			} else {
				&self.redo
			};
			let mut i = log.len();
			let mut count = 0u32;
			while i > 0 {
				i -= 1;
				match log.get(s, i) {
					Some(r) if r.tag == TAG_BEGIN => break,
					Some(_) => count += 1,
					None => break,
				}
			}
			count
		};
		self.touch();
		self.pieces
			.reserve(s, n.saturating_mul(2).saturating_add(4))?;
		self.reserve_log(s, to, n.saturating_add(8))?;
		self.push_log(
			s,
			to,
			Rec {
				tag: TAG_BEGIN,
				x: label,
				y: 0,
				a: 0,
				b: 0,
				c: 0,
			},
		)?;
		while let Some(r) = self.log_mut(from).pop(s) {
			match r.tag {
				TAG_BEGIN => break,
				TAG_SPLICE => self.apply_splice(s, from, to, r)?,
				TAG_FMT => self.apply_fmt(s, from, to, r)?,
				_ => return Err(Error::Range),
			}
		}
		Ok(true)
	}

	fn apply_splice<S: Store + ?Sized>(
		&mut self,
		s: &mut S,
		from: Log,
		to: Log,
		r: Rec,
	) -> Result<()> {
		let (ia, removed) = self.remove_range(s, r.a, r.b, to)?;
		let mut tally = 0u32;
		for _ in 0..r.c {
			let p = self.log_mut(from).pop(s).ok_or(Error::Range)?;
			if p.tag != TAG_PIECE {
				return Err(Error::Range);
			}
			self.insert_piece(
				s,
				ia,
				Piece {
					source: p.x,
					start: p.a,
					len: p.b,
					cfmt: p.y,
					pfmt: p.c as u16,
				},
			)?;
			tally += p.b;
		}
		self.note(r.a, r.b, tally);
		self.push_log(
			s,
			to,
			Rec {
				tag: TAG_SPLICE,
				x: 0,
				y: 0,
				a: r.a,
				b: tally,
				c: removed,
			},
		)?;
		if ia > 0 {
			self.try_merge(s, ia - 1);
		}
		Ok(())
	}

	fn apply_fmt<S: Store + ?Sized>(
		&mut self,
		s: &mut S,
		from: Log,
		to: Log,
		r: Rec,
	) -> Result<()> {
		// Runs pop in text order from pos on. Find the total length first by reading them in place.
		let log_len = self.log_mut(from).len();
		let mut total = 0u32;
		for k in 0..r.c {
			let rec = self
				.log_mut(from)
				.get(s, log_len - 1 - k)
				.ok_or(Error::Range)?;
			total += rec.a;
		}
		self.note(r.a, total, total);
		let ia = self.split_at(s, r.a)?;
		let ib = self.split_at(s, r.a + total)?;
		for i in (ia..ib).rev() {
			let p = self.piece(s, i).ok_or(Error::Range)?;
			self.push_log(
				s,
				to,
				Rec {
					tag: TAG_RUN,
					x: 0,
					y: p.cfmt,
					a: p.len,
					b: u32::from(p.pfmt),
					c: 0,
				},
			)?;
		}
		let nruns = ib - ia;
		let mut at = r.a;
		for _ in 0..r.c {
			let run = self.log_mut(from).pop(s).ok_or(Error::Range)?;
			let i = self.split_at(s, at)?;
			let j = self.split_at(s, at + run.a)?;
			for k in i..j {
				let p = self.piece(s, k).ok_or(Error::Range)?;
				self.pieces.set(
					s,
					k,
					Piece {
						cfmt: run.y,
						pfmt: run.b as u16,
						..p
					},
				)?;
			}
			at += run.a;
		}
		self.push_log(
			s,
			to,
			Rec {
				tag: TAG_FMT,
				x: 0,
				y: 0,
				a: r.a,
				b: 0,
				c: nruns,
			},
		)?;
		let ia2 = self.split_at(s, r.a)?;
		let ib2 = self.split_at(s, r.a + total)?;
		let _ = ia2;
		for i in (ia2.saturating_sub(1)..=ib2).rev() {
			self.try_merge(s, i);
		}
		self.cursor.set((0, 0));
		Ok(())
	}
}

#[derive(Copy, Clone)]
enum Change {
	Char(u16),
	Para(u16),
}
