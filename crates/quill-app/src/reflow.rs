// The paragraph checkpoint cache: the state of the body's layout at paragraph boundaries, kept in one Store chunk, so a typed key
// re-lays out the paragraph it changed (and the few after it that moved) and every query starts at the paragraph it needs.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A checkpoint is a [`LayoutCursor`] (20 bytes). Record 0 is the start of the story and `end` is the state after the last paragraph,
//! so `count + 1` cursors in all. When the document has more paragraphs than a chunk holds, only every `stride`th is kept.
//!
//! The cache is repaired where the editor can write to the store (`sync_layout`, at the end of each command and key), from what the
//! story's text says changed (`Text::take_damage`): layout restarts at the paragraph holding the first changed byte and stops at the
//! first boundary whose position (shifted by the edit) and `y` match an old checkpoint, because layout reads neither the page, the
//! paragraph nor the line number. Readers only read it, and check that it describes the document as it is now (the revisions);
//! when it does not, they lay out from the start as they always did.

use core::cell::Cell;

use quill_core::doc::StoryKind;
use quill_core::layout::{layout_from, LayoutCursor, Line, Measure};
use quill_core::store::{ChunkId, ReadStore, Store};
use quill_core::text::Damage;

use crate::App;

/// Bytes of one checkpoint.
const REC: usize = 20;
/// Checkpoints staged between layout and the store.
const BATCH: usize = 16;
/// The records a first chunk holds to begin with, the most any chunk holds (32,000 bytes), how many chunks there are and so the most
/// records: 6,400, which is a paragraph each for the 1 MB document of the J0 spike (4,700 paragraphs).
const MIN_CAP: u16 = 64;
const PER: u16 = 1600;
const CHUNKS: usize = 4;
const MAX_CAP: u16 = PER * CHUNKS as u16;

/// A line found by a query, with the document it was found in.
#[derive(Copy, Clone)]
pub(crate) struct Memo {
	/// The body's revision and the rest of the document's when it was found, or `None` before the first.
	stamp: Option<(u32, u32)>,
	/// What was asked: a position for the caret's line, a line index for the top line.
	key: u32,
	idx: u32,
	line: Line,
}

impl Memo {
	const EMPTY: Memo = Memo {
		stamp: None,
		key: 0,
		idx: 0,
		line: Line {
			para: 0,
			line: 0,
			start: 0,
			len: 0,
			x: 0,
			indent: 0,
			width: 0,
			ascent: 0,
			descent: 0,
			page: 0,
			y: 0,
		},
	};
}

/// The cache: where the checkpoints are and which document they describe.
pub(crate) struct Reflow {
	/// The chunks that hold the records: the first grows to `PER` records, the rest are `PER` each.
	chunks: [ChunkId; CHUNKS],
	nchunks: u8,
	/// Records the chunks hold; 0 before the first build.
	cap: u16,
	/// Records stored, the start of the story first.
	count: u16,
	/// Only every `stride`th paragraph has a record.
	stride: u16,
	/// The state after the last paragraph.
	end: LayoutCursor,
	/// The text's length when the cache was last made right.
	len: u32,
	/// The body's revision and the rest of the document's it describes.
	stamp: (u32, u32),
	/// False before the first build and after anything that the cache cannot follow.
	valid: bool,
	/// Some paragraph format of the document is a list, so labels count from the start of the story.
	listed: bool,
	/// How many paragraph formats `listed` has looked at.
	seen: u16,
	/// The caret's line, and the top line, from the last time they were asked for.
	caret: Cell<Memo>,
	top: Cell<Memo>,
	/// Paragraphs laid out for queries since the app began (for the bench and the tests).
	visits: Cell<u32>,
}

fn encode(c: &LayoutCursor) -> [u8; REC] {
	let mut b = [0u8; REC];
	for (i, v) in [c.pos, c.para, c.page, c.y as u32, c.line]
		.into_iter()
		.enumerate()
	{
		if let Some(d) = b.get_mut(i * 4..i * 4 + 4) {
			d.copy_from_slice(&v.to_le_bytes());
		}
	}
	b
}

fn decode(b: &[u8; REC]) -> LayoutCursor {
	let w = |i: usize| {
		let mut a = [0u8; 4];
		if let Some(s) = b.get(i * 4..i * 4 + 4) {
			a.copy_from_slice(s);
		}
		u32::from_le_bytes(a)
	};
	LayoutCursor {
		pos: w(0),
		para: w(1),
		page: w(2),
		y: w(3) as i32,
		line: w(4),
	}
}

impl Reflow {
	pub(crate) const fn new() -> Reflow {
		Reflow {
			chunks: [ChunkId(0); CHUNKS],
			nchunks: 0,
			cap: 0,
			count: 0,
			stride: 1,
			end: LayoutCursor::START,
			len: 0,
			stamp: (0, 0),
			valid: false,
			listed: false,
			seen: 0,
			caret: Cell::new(Memo::EMPTY),
			top: Cell::new(Memo::EMPTY),
			visits: Cell::new(0),
		}
	}

	/// Paragraphs laid out for queries since the app began.
	pub(crate) fn visits(&self) -> u32 {
		self.visits.get()
	}

	/// The record `i`; the end cursor stands in as record `count`.
	fn rec<S: ReadStore + ?Sized>(&self, s: &S, i: u16) -> LayoutCursor {
		if i >= self.count {
			return self.end;
		}
		self.raw(s, i)
	}

	/// The chunk and byte offset of slot `i`.
	fn at(&self, i: u16) -> (ChunkId, u16) {
		let chunk = self
			.chunks
			.get(usize::from(i / PER))
			.copied()
			.unwrap_or(ChunkId(0));
		(chunk, (usize::from(i % PER) * REC) as u16)
	}

	/// Whatever the chunks hold at slot `i`, the count aside.
	fn raw<S: ReadStore + ?Sized>(&self, s: &S, i: u16) -> LayoutCursor {
		let mut b = [0u8; REC];
		let (id, off) = self.at(i);
		s.read(id, off, &mut b);
		decode(&b)
	}

	fn set_rec<S: Store + ?Sized>(&self, s: &mut S, i: u16, c: &LayoutCursor) {
		let (id, off) = self.at(i);
		s.write(id, off, &encode(c));
	}

	/// Reads `k` (at most 8) records from slot `i` on into `buf`: one call when they lie in one chunk, one each when they do not.
	fn read_recs<S: ReadStore + ?Sized>(&self, s: &S, i: u16, k: u16, buf: &mut [u8; REC * 8]) {
		if i / PER == (i + k - 1) / PER {
			let (id, off) = self.at(i);
			if let Some(b) = buf.get_mut(..usize::from(k) * REC) {
				s.read(id, off, b);
			}
			return;
		}
		for j in 0..k {
			let (id, off) = self.at(i + j);
			if let Some(b) = buf.get_mut(usize::from(j) * REC..usize::from(j + 1) * REC) {
				s.read(id, off, b);
			}
		}
	}

	fn write_recs<S: Store + ?Sized>(&self, s: &mut S, i: u16, k: u16, buf: &[u8; REC * 8]) {
		if i / PER == (i + k - 1) / PER {
			let (id, off) = self.at(i);
			if let Some(b) = buf.get(..usize::from(k) * REC) {
				s.write(id, off, b);
			}
			return;
		}
		for j in 0..k {
			let (id, off) = self.at(i + j);
			if let Some(b) = buf.get(usize::from(j) * REC..usize::from(j + 1) * REC) {
				s.write(id, off, b);
			}
		}
	}

	/// Moves `n` records from slot `from` to slot `to` (either may be the higher), as `memmove` does, eight at a time.
	fn move_recs<S: Store + ?Sized>(&self, s: &mut S, from: u16, to: u16, n: u16) {
		if from == to {
			return;
		}
		let mut done = 0u16;
		while done < n {
			let k = (n - done).min(8);
			let (src, dst) = if to < from {
				(from + done, to + done)
			} else {
				(from + n - done - k, to + n - done - k)
			};
			let mut buf = [0u8; REC * 8];
			self.read_recs(&*s, src, k, &mut buf);
			self.write_recs(s, dst, k, &buf);
			done += k;
		}
	}

	/// Replaces slots `start..start + n` by `f` of each, eight at a time.
	fn rewrite<S: Store + ?Sized>(
		&self,
		s: &mut S,
		start: u16,
		n: u16,
		f: impl Fn(&LayoutCursor) -> LayoutCursor,
	) {
		let mut done = 0u16;
		while done < n {
			let k = (n - done).min(8);
			let mut buf = [0u8; REC * 8];
			self.read_recs(&*s, start + done, k, &mut buf);
			for j in 0..usize::from(k) {
				let mut one = [0u8; REC];
				if let Some(b) = buf.get(j * REC..(j + 1) * REC) {
					one.copy_from_slice(b);
				}
				let c = encode(&f(&decode(&one)));
				if let Some(b) = buf.get_mut(j * REC..(j + 1) * REC) {
					b.copy_from_slice(&c);
				}
			}
			self.write_recs(s, start + done, k, &buf);
			done += k;
		}
	}

	/// The last record, counting the end as one, for which `before` holds (the first when none does).
	fn last_where<S: ReadStore + ?Sized>(
		&self,
		s: &S,
		before: impl Fn(&LayoutCursor) -> bool,
	) -> u16 {
		let (mut lo, mut hi) = (0u16, self.count.saturating_add(1));
		// Invariant: record `lo` satisfies `before` (the first always does), none at `hi` or later is wanted.
		while hi - lo > 1 {
			let mid = lo + (hi - lo) / 2;
			if before(&self.rec(s, mid)) {
				lo = mid;
			} else {
				hi = mid;
			}
		}
		lo
	}

	/// Makes room for `want` records, growing the first chunk and then adding whole ones. False when memory is short or the cache is as
	/// big as it gets.
	fn grow<S: Store + ?Sized>(&mut self, s: &mut S, want: u32) -> bool {
		if want <= u32::from(self.cap) {
			return true;
		}
		if want > u32::from(MAX_CAP) {
			return false;
		}
		if self.nchunks <= 1 && want <= u32::from(PER) {
			let cap = (want.max(u32::from(self.cap) * 4) as u16).clamp(MIN_CAP, PER);
			let Some(id) = s.alloc((usize::from(cap) * REC) as u16) else {
				return false;
			};
			if self.cap > 0 {
				let (old, _) = self.at(0);
				s.copy((old, 0), (id, 0), (usize::from(self.count) * REC) as u16);
				s.free(old);
			}
			if let Some(first) = self.chunks.first_mut() {
				*first = id;
			}
			self.nchunks = 1;
			self.cap = cap;
			return true;
		}
		if self.cap < PER && !self.grow(s, u32::from(PER)) {
			return false;
		}
		while u32::from(self.cap) < want {
			let Some(id) = s.alloc((usize::from(PER) * REC) as u16) else {
				return false;
			};
			let Some(slot) = self.chunks.get_mut(usize::from(self.nchunks)) else {
				s.free(id);
				return false;
			};
			*slot = id;
			self.nchunks += 1;
			self.cap += PER;
		}
		true
	}

	fn free<S: Store + ?Sized>(&mut self, s: &mut S) {
		for id in self.chunks.iter().take(usize::from(self.nchunks)) {
			s.free(*id);
		}
		self.nchunks = 0;
		self.cap = 0;
		self.count = 0;
		self.valid = false;
	}

	/// Keeps every other record, doubling the stride, when a chunk of the biggest size is full.
	fn thin<S: Store + ?Sized>(&mut self, s: &mut S) {
		let mut to = 0u16;
		let mut from = 0u16;
		while from < self.count {
			let c = self.rec(s, from);
			self.set_rec(s, to, &c);
			to += 1;
			from += 2;
		}
		self.count = to;
		self.stride = self.stride.saturating_mul(2);
	}

	/// Adds a record at the end while building. False when memory is short.
	fn push<S: Store + ?Sized>(&mut self, s: &mut S, c: &LayoutCursor) -> bool {
		if !c.para.is_multiple_of(u32::from(self.stride)) && self.count > 0 {
			return true;
		}
		if self.count == self.cap && !self.grow(s, u32::from(self.count) + 1) {
			if self.cap < MAX_CAP {
				return false;
			}
			self.thin(s);
			if !c.para.is_multiple_of(u32::from(self.stride)) {
				return true;
			}
		}
		self.set_rec(s, self.count, c);
		self.count += 1;
		true
	}

	/// Whether the cache describes the document as it is now.
	pub(crate) fn fresh(&self, stamp: (u32, u32)) -> bool {
		self.valid && self.stamp == stamp
	}

	pub(crate) fn invalidate(&mut self) {
		self.valid = false;
		self.caret.set(Memo::EMPTY);
		self.top.set(Memo::EMPTY);
	}
}

impl<M: Measure + Clone> App<M> {
	/// What the body's layout depends on besides its own text: the revision of the rest of the document.
	pub(crate) fn layout_stamp(&self) -> (u32, u32) {
		(
			self.doc.body.revision(),
			self.doc.revision().wrapping_sub(self.doc.body.revision()),
		)
	}

	fn reflow_fresh(&self) -> bool {
		self.reflow.fresh(self.layout_stamp())
	}

	/// Forgets the cache: the next sync builds it again. For anything that changes layout without changing the document.
	pub fn invalidate_layout(&mut self) {
		self.reflow.invalidate();
	}

	/// Paragraphs the layout cache has laid out for queries (a count for the bench and the tests).
	pub fn layout_visits(&self) -> u32 {
		self.reflow.visits()
	}

	/// Makes the cache describe the document as it is now: repairs it from the story's damage, or builds it again.
	#[inline(never)]
	pub(crate) fn sync_layout<S: Store + ?Sized>(&mut self, s: &mut S) {
		let stamp = self.layout_stamp();
		if self.reflow.fresh(stamp) {
			return;
		}
		let damage = self.doc.body.take_damage();
		let was = self.reflow.valid && self.reflow.stamp.1 == stamp.1;
		let done = was && damage.is_some_and(|d| self.repair(s, d));
		if !done && !self.build(s) {
			self.reflow.free(s);
			self.reflow.invalidate();
			return;
		}
		self.reflow.stamp = stamp;
		self.reflow.valid = true;
		self.reflow.len = self.doc.body.len();
		self.reflow.caret.set(Memo::EMPTY);
		self.reflow.top.set(Memo::EMPTY);
		self.note_list_formats(s);
	}

	/// Looks at the paragraph formats added since last time for a list.
	fn note_list_formats<S: ReadStore + ?Sized>(&mut self, s: &S) {
		let n = self.doc.pfmts.len().min(u32::from(u16::MAX)) as u16;
		for i in self.reflow.seen..n {
			if self.doc.pfmts.get(s, i).is_some_and(|p| p.flags & 5 != 0) {
				self.reflow.listed = true;
			}
		}
		self.reflow.seen = n;
	}

	/// Lays the whole body out and keeps a checkpoint for each paragraph (every few, for a very long story).
	#[inline(never)]
	fn build<S: Store + ?Sized>(&mut self, s: &mut S) -> bool {
		self.reflow.free(s);
		self.reflow.stride = 1;
		self.reflow.listed = false;
		self.reflow.seen = 0;
		if !self.reflow.grow(s, 1) {
			return false;
		}
		let total = self.doc.body.len();
		let mut at = LayoutCursor::START;
		self.reflow.count = 0;
		if !self.reflow.push(s, &at) {
			return false;
		}
		loop {
			let mut staged = [LayoutCursor::START; BATCH];
			let mut m = 0usize;
			let visits = &self.reflow.visits;
			let fin = layout_from(
				&self.doc,
				&*s,
				&mut self.measure.clone(),
				StoryKind::Body,
				at,
				|_| {},
				|c| {
					visits.set(visits.get().wrapping_add(1));
					if c.pos < total {
						if let Some(slot) = staged.get_mut(m) {
							*slot = *c;
							m += 1;
						}
					}
					m < BATCH
				},
			);
			for c in staged.iter().take(m) {
				if !self.reflow.push(s, c) {
					return false;
				}
			}
			if fin.pos >= total {
				self.reflow.end = fin;
				return true;
			}
			at = fin;
		}
	}

	/// Makes the cache right after the body changed within `d` (and nothing else changed). False when it cannot, and `build` must.
	#[inline(never)]
	fn repair<S: Store + ?Sized>(&mut self, s: &mut S, d: Damage) -> bool {
		let r = &self.reflow;
		let total = self.doc.body.len();
		let delta = i64::from(total) - i64::from(r.len);
		let (count, stride) = (r.count, r.stride);
		let mut k = r.last_where(&*s, |c| c.pos <= d.lo);
		// A boundary that is not after a paragraph mark is only the end of a text that had none.
		let at_edge = r.rec(&*s, k).pos == d.lo;
		if at_edge && d.lo > 0 && self.doc.body.read_byte(&*s, d.lo - 1) != Some(0x0D) {
			k = k.saturating_sub(1);
		}
		let start = r.rec(&*s, k);
		// Layout from the first changed paragraph to the first boundary that is as it was.
		let mut staged = [LayoutCursor::START; BATCH];
		let mut m = 0usize;
		// The end of the story is not a record; when layout starts from it, it becomes one.
		if k >= count {
			if let Some(slot) = staged.first_mut() {
				*slot = start;
				m = 1;
			}
		}
		let mut j = k.saturating_add(1);
		let mut joined: Option<(u16, LayoutCursor, LayoutCursor)> = None;
		let mut overflow = false;
		let fin = layout_from(
			&self.doc,
			&*s,
			&mut self.measure.clone(),
			StoryKind::Body,
			start,
			|_| {},
			|c| {
				r.visits.set(r.visits.get().wrapping_add(1));
				while j <= count && i64::from(r.rec(&*s, j).pos) + delta < i64::from(c.pos) {
					j += 1;
				}
				if c.pos >= d.hi && j <= count {
					let old = r.rec(&*s, j);
					if i64::from(old.pos) + delta == i64::from(c.pos) && old.y == c.y {
						joined = Some((j, old, *c));
						return false;
					}
				}
				if c.pos < total && (stride == 1 || c.para.is_multiple_of(u32::from(stride))) {
					match staged.get_mut(m) {
						Some(slot) => {
							*slot = *c;
							m += 1;
						}
						None => {
							overflow = true;
							return false;
						}
					}
				}
				true
			},
		);
		if overflow {
			return false;
		}
		// The records from k + 1 up to the joining one (or all, when layout ran to the end) are replaced by the staged ones.
		let first_gone = if k >= count { count } else { k + 1 };
		let (keep_from, new_end) = match joined {
			Some((j, old, c)) => (Some((j, old, c)), c),
			None => (None, fin),
		};
		let tail_from = keep_from.map_or(count, |(j, _, _)| j.min(count));
		let tail_n = count.saturating_sub(tail_from);
		let new_count = u32::from(first_gone) + m as u32 + u32::from(tail_n);
		if new_count > u32::from(MAX_CAP) || !self.reflow.grow(s, new_count) {
			return false;
		}
		let r = &mut self.reflow;
		let at_new = first_gone + m as u16;
		if tail_n > 0 {
			r.move_recs(s, tail_from, at_new, tail_n);
		}
		for (i, c) in staged.iter().take(m).enumerate() {
			r.set_rec(s, first_gone + i as u16, c);
		}
		if let Some((j, old, c)) = keep_from {
			let shift = |x: &LayoutCursor| LayoutCursor {
				pos: (i64::from(x.pos) + delta) as u32,
				para: (i64::from(x.para) + i64::from(c.para) - i64::from(old.para)) as u32,
				page: (i64::from(x.page) + i64::from(c.page) - i64::from(old.page)) as u32,
				y: x.y,
				line: (i64::from(x.line) + i64::from(c.line) - i64::from(old.line)) as u32,
			};
			if j < count {
				// The joining record itself becomes the cursor layout reached; the ones after it are shifted by the same amounts.
				r.set_rec(s, at_new, &c);
				r.rewrite(s, at_new + 1, tail_n - 1, shift);
				r.end = shift(&r.end);
			} else {
				r.end = c;
			}
		} else {
			r.end = new_end;
		}
		r.count = at_new + tail_n;
		true
	}

	/// The checkpoint to start a query at, or the start of the story when the cache cannot be used.
	fn reflow_start<S: ReadStore + ?Sized>(
		&self,
		s: &S,
		want: impl Fn(&LayoutCursor) -> bool,
	) -> LayoutCursor {
		if self.reflow_fresh() && !self.reflow.listed {
			self.reflow.rec(s, self.reflow.last_where(s, want))
		} else {
			LayoutCursor::START
		}
	}

	/// Calls `f(index, line)` for the lines from the one a checkpoint at or before `first` begins with, until `f` says false. The first
	/// calls may be for lines before `first`; a story with lists is always walked from its start, because labels count from there.
	pub(crate) fn lines_from<S: ReadStore + ?Sized>(
		&self,
		s: &S,
		first: u32,
		mut f: impl FnMut(u32, &Line) -> bool,
	) {
		let start = self.reflow_start(s, |c| c.line <= first);
		self.walk_lines_from(s, start, &mut f);
	}

	/// The same from the checkpoint before page `first_page`.
	pub(crate) fn lines_from_page<S: ReadStore + ?Sized>(
		&self,
		s: &S,
		first_page: u32,
		mut f: impl FnMut(u32, &Line) -> bool,
	) {
		let start = self.reflow_start(s, |c| c.page < first_page);
		self.walk_lines_from(s, start, &mut f);
	}

	fn walk_lines_from<S: ReadStore + ?Sized>(
		&self,
		s: &S,
		start: LayoutCursor,
		f: &mut dyn FnMut(u32, &Line) -> bool,
	) {
		let mut i = start.line;
		let go = Cell::new(true);
		let visits = &self.reflow.visits;
		layout_from(
			&self.doc,
			s,
			&mut self.measure.clone(),
			StoryKind::Body,
			start,
			|l| {
				if go.get() {
					go.set(f(i, l));
				}
				i += 1;
			},
			|_| {
				visits.set(visits.get().wrapping_add(1));
				go.get()
			},
		);
	}

	/// Calls `f(index, line)` for the lines from the one a checkpoint at or before line `first` begins with, until `f` says false. It may
	/// be called for lines before `first`. Lay out only what is needed: a painter stops at the bottom of its window.
	pub fn for_each_line_from<S: ReadStore + ?Sized>(
		&self,
		s: &S,
		first: u32,
		f: impl FnMut(u32, &Line) -> bool,
	) {
		self.lines_from(s, first, f);
	}

	/// The same from the lines just before page `first_page` (from 1) begins; `f` is called for lines of earlier pages too.
	pub fn for_each_line_from_page<S: ReadStore + ?Sized>(
		&self,
		s: &S,
		first_page: u32,
		f: impl FnMut(u32, &Line) -> bool,
	) {
		self.lines_from_page(s, first_page, f);
	}

	/// The same from the paragraph holding position `pos`; `f` may be called for lines a little before it.
	pub fn for_each_line_from_pos<S: ReadStore + ?Sized>(
		&self,
		s: &S,
		pos: u32,
		f: impl FnMut(u32, &Line) -> bool,
	) {
		let start = self.reflow_start(s, |c| c.pos <= pos);
		self.walk_lines_from(s, start, &mut { f });
	}

	/// How many pages the body takes: the page of its last line, and 0 when it has no lines.
	pub fn page_total<S: ReadStore + ?Sized>(&self, s: &S) -> u32 {
		self.pages_total(s)
	}

	/// How many lines the body has.
	pub(crate) fn lines_total<S: ReadStore + ?Sized>(&self, s: &S) -> u32 {
		if self.reflow_fresh() {
			return self.reflow.end.line;
		}
		let mut n = 0;
		self.lines_from(s, 0, |_, _| {
			n += 1;
			true
		});
		n
	}

	/// How many pages the body takes (0 when it has no lines).
	pub(crate) fn pages_total<S: ReadStore + ?Sized>(&self, s: &S) -> u32 {
		if self.reflow_fresh() {
			return if self.reflow.end.line == 0 {
				0
			} else {
				self.reflow.end.page
			};
		}
		let mut pages = 0;
		self.lines_from(s, 0, |_, l| {
			pages = pages.max(l.page);
			true
		});
		pages
	}

	/// How many lines of page `page` there are up to and including line `index` (which is on it): the lines of a page are one run.
	pub(crate) fn line_in_page<S: ReadStore + ?Sized>(&self, s: &S, index: u32, page: u32) -> u32 {
		let mut first = None;
		self.lines_from_page(s, page, |i, l| {
			if l.page == page {
				first = Some(i);
			}
			first.is_none() && i < index
		});
		first.map_or(0, |f| index.saturating_sub(f) + 1)
	}

	/// The line with index `idx`.
	pub(crate) fn line_at<S: ReadStore + ?Sized>(&self, s: &S, idx: u32) -> Option<Line> {
		let stamp = self.layout_stamp();
		let m = self.reflow.top.get();
		if m.stamp == Some(stamp) && m.key == idx {
			return Some(m.line);
		}
		let mut out = None;
		self.lines_from(s, idx, |i, l| {
			if i == idx {
				out = Some(*l);
			}
			i < idx
		});
		if let (Some(l), true) = (out, self.reflow.fresh(stamp)) {
			self.reflow.top.set(Memo {
				stamp: Some(stamp),
				key: idx,
				idx,
				line: l,
			});
		}
		out
	}

	/// The index and line holding position `pos`: the line it is on, except that the end of a line that wraps belongs to the next one.
	pub(crate) fn line_for<S: ReadStore + ?Sized>(&self, s: &S, pos: u32) -> Option<(u32, Line)> {
		let stamp = self.layout_stamp();
		let m = self.reflow.caret.get();
		if m.stamp == Some(stamp) && m.key == pos {
			return Some((m.idx, m.line));
		}
		let start = self.reflow_start(s, |c| c.pos <= pos && c.pos < self.doc.body.len().max(1));
		let found: Cell<Option<(u32, Line)>> = Cell::new(None);
		let prev: Cell<Option<(u32, Line)>> = Cell::new(None);
		let mut i = start.line;
		let visits = &self.reflow.visits;
		layout_from(
			&self.doc,
			s,
			&mut self.measure.clone(),
			StoryKind::Body,
			start,
			|l| {
				if found.get().is_none() {
					if let Some((pi, p)) = prev.get() {
						let soft = l.start == p.start + p.len;
						if pos >= p.start
							&& pos <= p.start + p.len
							&& !(soft && pos == p.start + p.len)
						{
							found.set(Some((pi, p)));
						}
					}
				}
				prev.set(Some((i, *l)));
				i += 1;
			},
			|_| {
				visits.set(visits.get().wrapping_add(1));
				// The paragraph just laid out ended with its last line, which no later line can wrap into.
				if found.get().is_none() {
					if let Some((pi, p)) = prev.get() {
						if pos >= p.start && pos <= p.start + p.len {
							found.set(Some((pi, p)));
						}
					}
				}
				found.get().is_none()
			},
		);
		let mut found = found.get();
		if found.is_none() {
			if let Some((pi, p)) = prev.get() {
				if pos >= p.start {
					found = Some((pi, p));
				}
			}
		}
		if let (Some((idx, line)), true) = (found, self.reflow.fresh(stamp)) {
			self.reflow.caret.set(Memo {
				stamp: Some(stamp),
				key: pos,
				idx,
				line,
			});
		}
		found
	}
}

impl<M: Measure + Clone> App<M> {
	/// Checks the layout cache against a layout from the start, for the tests: every record is the cursor that layout reaches at that
	/// position (and, with a stride of 1, every paragraph has one), and the end and the totals agree.
	#[doc(hidden)]
	pub fn check_layout_cache<S: ReadStore + ?Sized>(&self, s: &S) -> Result<(), &'static str> {
		if !self.reflow_fresh() {
			return Err("the cache is not fresh");
		}
		let r = &self.reflow;
		let total = self.doc.body.len();
		let ptr = Cell::new(0u16);
		let err: Cell<Option<&'static str>> = Cell::new(None);
		let fin = layout_from(
			&self.doc,
			s,
			&mut self.measure.clone(),
			StoryKind::Body,
			LayoutCursor::START,
			|_| {},
			|c| {
				let mut at = ptr.get();
				if at == 0 {
					if r.rec(s, 0) != LayoutCursor::START {
						err.set(Some("record 0 is not the start"));
					}
					at = 1;
				}
				if c.pos < total {
					if at < r.count && r.rec(s, at).pos == c.pos {
						if r.rec(s, at) != *c {
							err.set(Some("a record differs from layout"));
						}
						at += 1;
					} else if r.stride == 1 {
						err.set(Some("a paragraph has no record"));
					}
				}
				ptr.set(at);
				err.get().is_none()
			},
		);
		if let Some(e) = err.get() {
			return Err(e);
		}
		if ptr.get().max(1) != r.count {
			return Err("a record is not at a paragraph boundary");
		}
		if r.end != fin {
			return Err("the end differs from layout");
		}
		Ok(())
	}
}
