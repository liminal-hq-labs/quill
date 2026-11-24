// Check Spelling as You Type: the idle-time scanner, the table of marks it fills, and the commands behind the right-click menu.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The shell tells the app which part of the body is on the screen (`spell_set_view`) and calls `spell_idle` when the keyboard has been
// quiet, a few words at a time. Each unknown word becomes a mark: where it starts, how long it is and a hash of its letters. The marks
// live in one small `Store` chunk (64 records of 8 bytes), sorted by position, so the data segment of the Windows 3.x build pays for a
// handful of integers and nothing more.
//
// Edits keep the marks true in two ways. The typing paths (`insert`, `delete`, joining paragraphs, a replacement) say what changed and
// where (`spell_note_edit`): marks before the edit stay, marks after it move with the text, and a mark the edit touches is dropped. Any
// other change (Undo, Redo, a file opened, another story) shows as a different length or story and drops every mark, and the hash of
// each mark is checked again at the start of each idle call, so a mark never stays on text that is not the word it was made for.
//
// The word being typed is not looked at until the caret leaves it: a word an edit has just touched, with the caret on it, is held back
// and checked when the caret has moved away, as Word does.
use quill_core::layout::Measure;
use quill_core::store::{ChunkId, Store};
use quill_spell::{is_letter, lower, Suggestions, MAX_WORD};

use crate::{App, Files, Mode};

/// Marks the table holds: the unknown words of a screenful, with room to spare.
const CAP: usize = 64;
/// Bytes of a mark: start (`u32`), length (`u16`) and a hash of the word (`u16`).
const REC: usize = 8;
/// The nodes the suggestion search may visit for the pop-up menu.
const BUDGET: u32 = 40_000;
/// The longest run a word is widened by when an edit touches it.
const WIDEN: u32 = 64;

/// A range of positions, empty when its start is not before its end.
type Range = (u32, u32);

const NONE: Range = (0, 0);
const NOWHERE: Range = (u32::MAX, u32::MAX);

fn is_empty(r: Range) -> bool {
	r.0 >= r.1
}

fn union(a: Range, b: Range) -> Range {
	if is_empty(a) {
		b
	} else if is_empty(b) {
		a
	} else {
		(a.0.min(b.0), a.1.max(b.1))
	}
}

/// Moves a position across an edit that replaced `removed` bytes at `pos` with `ins` bytes.
fn shift(p: u32, pos: u32, removed: u32, ins: u32) -> u32 {
	if p <= pos {
		p
	} else if p >= pos.saturating_add(removed) {
		p + ins - removed.min(p)
	} else {
		pos
	}
}

/// A 16-bit hash of a word's letters, ignoring case.
fn hash(word: &[u8]) -> u16 {
	let mut h: u16 = 0x811C;
	for b in word {
		h = (h ^ u16::from(lower(*b))).wrapping_mul(0x0193);
	}
	h
}

/// One unknown word.
#[derive(Copy, Clone)]
struct Mark {
	start: u32,
	len: u16,
	hash: u16,
}

impl Mark {
	fn end(self) -> u32 {
		self.start + u32::from(self.len)
	}
}

/// What the scanner keeps between calls: a few integers, and the id of the chunk that holds the marks.
#[derive(Copy, Clone)]
pub(crate) struct Live {
	/// Check Spelling as You Type is on.
	pub on: bool,
	/// The dictionary could not be opened; nothing more is tried until the language changes.
	failed: bool,
	chunk: Option<ChunkId>,
	n: u8,
	/// The story and the length the marks were made for.
	view: u8,
	synced: u32,
	/// The positions on the screen, and the part of them that is checked.
	vis: Range,
	done: Range,
	/// The span being scanned, how far the scan has got, and whether it widens `done` (or is a span an edit made).
	span: Range,
	cur: u32,
	ext: bool,
	/// What edits touched since it was checked, and the word held back because the caret is in it.
	dirty: Range,
	hold: Range,
	/// Where the user last typed (`NOWHERE` when no edit is being followed): a word there with the caret on it is the one being typed.
	typed: Range,
	/// What the marks changed in since the shell last asked.
	changed: Range,
}

impl Live {
	pub(crate) const fn new() -> Self {
		Live {
			on: false,
			failed: false,
			chunk: None,
			n: 0,
			view: 0,
			synced: 0,
			vis: NONE,
			done: NONE,
			span: NONE,
			cur: 0,
			ext: false,
			dirty: NONE,
			hold: NONE,
			typed: NOWHERE,
			changed: NONE,
		}
	}
}

fn wordish(b: u8) -> bool {
	is_letter(b) || b.is_ascii_digit()
}

impl<M: Measure + Clone> App<M> {
	/// True when Check Spelling as You Type is on.
	pub fn spell_live(&self) -> bool {
		self.spell.live.on
	}

	/// Sets what Check Spelling as You Type is before `QUILL.INI` is read: each build says what its default is.
	pub fn set_spell_live_default(&mut self, on: bool) {
		self.spell.live.on = on;
	}

	/// Turns Check Spelling as You Type on or off. Off drops every mark.
	pub fn set_spell_live(&mut self, on: bool) {
		if on == self.spell.live.on {
			return;
		}
		self.spell.live.on = on;
		if !on {
			self.live_forget();
		}
	}

	/// Opens the dictionary and the user dictionary for the idle checker, saying nothing when there is none (the status line is for what
	/// the user did). False when checking cannot go on.
	pub fn spell_live_ready<S: Store + ?Sized, F: Files>(&mut self, s: &mut S, f: &mut F) -> bool {
		if !self.spell.live.on {
			return false;
		}
		if self.spell.dict.is_some() {
			return true;
		}
		if self.spell.live.failed {
			return false;
		}
		if !self.spell_load(s, f) {
			self.spell.live.failed = true;
			return false;
		}
		true
	}

	/// Tells the scanner which part of the body is on the screen: the position of the first character of the first visible line and the
	/// end of the last.
	pub fn spell_set_view(&mut self, lo: u32, hi: u32) {
		let l = &mut self.spell.live;
		if (lo, hi) == l.vis {
			return;
		}
		l.vis = (lo, hi);
		// A scan of ground that is no longer on the screen stops.
		if l.ext && l.cur < l.span.1 && (l.span.1 <= lo || l.span.0 >= hi) {
			l.cur = l.span.1;
		}
	}

	/// Forgets every mark and what was checked, and says the screen needs painting again.
	fn live_forget(&mut self) {
		let body_len = self.doc.body.len();
		let view = self.view;
		let l = &mut self.spell.live;
		l.changed = union(l.changed, l.vis);
		l.n = 0;
		l.done = NONE;
		l.span = NONE;
		l.cur = 0;
		l.dirty = NONE;
		l.hold = NONE;
		l.typed = NOWHERE;
		l.synced = body_len;
		l.view = view;
	}

	/// Drops every mark: the document changed in a way the app does not follow (Undo, Redo, a file opened, a replace everywhere).
	pub(crate) fn spell_forget(&mut self) {
		if self.spell.live.n > 0 || !is_empty(self.spell.live.done) {
			self.live_forget();
		}
	}

	/// The language changed or a word was added or ignored: the marks are made again.
	pub(crate) fn spell_recheck(&mut self) {
		self.spell.live.failed = false;
		self.live_forget();
	}

	/// Notes that `removed` bytes at `pos` were replaced with `ins` bytes by an edit the app made, so marks after it move with the text
	/// and the words it touched are checked again.
	pub(crate) fn spell_note_edit<S: Store + ?Sized>(
		&mut self,
		s: &mut S,
		pos: u32,
		removed: u32,
		ins: u32,
	) {
		if !self.spell.live.on {
			return;
		}
		let len = self.doc.body.len();
		let before = (len + removed).saturating_sub(ins);
		if self.spell.live.synced != before || self.spell.live.view != self.view {
			self.live_forget();
			return;
		}
		let n = usize::from(self.spell.live.n);
		let mut w = 0;
		let mut gone = NONE;
		for r in 0..n {
			let Some(mut m) = self.mark_get(s, r) else {
				continue;
			};
			if m.end() < pos {
				// Before the edit.
			} else if m.start > pos.saturating_add(removed) {
				m.start = m.start + ins - removed;
			} else {
				gone = union(gone, (m.start, m.end()));
				continue;
			}
			self.mark_put(s, w, m);
			w += 1;
		}
		let l = &mut self.spell.live;
		l.n = w as u8;
		l.changed = union(l.changed, gone);
		l.vis = (
			shift(l.vis.0, pos, removed, ins),
			shift(l.vis.1, pos, removed, ins),
		);
		l.done = (
			shift(l.done.0, pos, removed, ins),
			shift(l.done.1, pos, removed, ins),
		);
		l.hold = (
			shift(l.hold.0, pos, removed, ins),
			shift(l.hold.1, pos, removed, ins),
		);
		l.dirty = (
			shift(l.dirty.0, pos, removed, ins),
			shift(l.dirty.1, pos, removed, ins),
		);
		l.dirty = union(l.dirty, (pos, pos + ins.max(1)));
		// The word being typed is the one the last edits ran together in.
		let edit = (pos, pos + ins);
		l.typed = if l.typed == NOWHERE {
			edit
		} else {
			let t = (
				shift(l.typed.0, pos, removed, ins),
				shift(l.typed.1, pos, removed, ins),
			);
			if edit.0 <= t.1 && edit.1 >= t.0 {
				(t.0.min(edit.0), t.1.max(edit.1))
			} else {
				edit
			}
		};
		// A scan in progress cannot follow the edit. What was left of a span an earlier edit made is still to be checked; the ground
		// of a scan of the screen is looked at again from its edges.
		if !l.ext && l.cur < l.span.1 {
			let rest = (
				shift(l.cur, pos, removed, ins),
				shift(l.span.1, pos, removed, ins),
			);
			l.dirty = union(l.dirty, rest);
		}
		l.span = NONE;
		l.cur = 0;
		l.synced = len;
	}

	fn mark_get<S: Store + ?Sized>(&self, s: &S, i: usize) -> Option<Mark> {
		read_mark(s, self.spell.live.chunk?, i)
	}

	fn mark_put<S: Store + ?Sized>(&mut self, s: &mut S, i: usize, m: Mark) {
		if let Some(c) = self.spell.live.chunk {
			let mut b = [0u8; REC];
			if let Some(d) = b.get_mut(..4) {
				d.copy_from_slice(&m.start.to_le_bytes());
			}
			if let Some(d) = b.get_mut(4..6) {
				d.copy_from_slice(&m.len.to_le_bytes());
			}
			if let Some(d) = b.get_mut(6..8) {
				d.copy_from_slice(&m.hash.to_le_bytes());
			}
			s.write(c, (i * REC) as u16, &b);
		}
	}

	/// Removes the marks that overlap `r`.
	fn mark_clear_range<S: Store + ?Sized>(&mut self, s: &mut S, r: Range) {
		let n = usize::from(self.spell.live.n);
		let mut w = 0;
		let mut gone = NONE;
		for i in 0..n {
			let Some(m) = self.mark_get(s, i) else {
				continue;
			};
			if m.start < r.1 && m.end() > r.0 {
				gone = union(gone, (m.start, m.end()));
				continue;
			}
			if w != i {
				self.mark_put(s, w, m);
			}
			w += 1;
		}
		let l = &mut self.spell.live;
		l.n = w as u8;
		l.changed = union(l.changed, gone);
	}

	/// Adds a mark, keeping the table sorted. False when the table is full or there is no memory for it.
	fn mark_add<S: Store + ?Sized>(
		&mut self,
		s: &mut S,
		start: u32,
		len: u32,
		word: &[u8],
	) -> bool {
		if self.spell.live.chunk.is_none() {
			self.spell.live.chunk = s.alloc((CAP * REC) as u16);
		}
		self.mark_clear_range(s, (start, start + len));
		let n = usize::from(self.spell.live.n);
		if self.spell.live.chunk.is_none() || n >= CAP {
			return false;
		}
		let mut at = n;
		for i in 0..n {
			if self.mark_get(s, i).is_some_and(|m| m.start > start) {
				at = i;
				break;
			}
		}
		let mut i = n;
		while i > at {
			if let Some(m) = self.mark_get(s, i - 1) {
				self.mark_put(s, i, m);
			}
			i -= 1;
		}
		self.mark_put(
			s,
			at,
			Mark {
				start,
				len: len as u16,
				hash: hash(word),
			},
		);
		let l = &mut self.spell.live;
		l.n += 1;
		l.changed = union(l.changed, (start, start + len));
		true
	}

	/// Makes the marks agree with the text: another length or story, or a mark whose word has changed, drops what cannot be trusted.
	fn live_sync<S: Store + ?Sized>(&mut self, s: &mut S) {
		if self.spell.live.synced != self.doc.body.len() || self.spell.live.view != self.view {
			self.live_forget();
			return;
		}
		let n = usize::from(self.spell.live.n);
		let mut w = 0;
		let mut gone = NONE;
		for i in 0..n {
			let Some(m) = self.mark_get(s, i) else {
				continue;
			};
			let mut buf = [0u8; MAX_WORD];
			let len = usize::from(m.len).min(MAX_WORD);
			let ok = (0..len).all(|k| match self.doc.body.read_byte(s, m.start + k as u32) {
				Some(b) => {
					if let Some(d) = buf.get_mut(k) {
						*d = if b == 0x92 { b'\'' } else { b };
					}
					true
				}
				None => false,
			});
			if ok && usize::from(m.len) <= MAX_WORD && hash(buf.get(..len).unwrap_or(&[])) == m.hash
			{
				if w != i {
					self.mark_put(s, w, m);
				}
				w += 1;
			} else {
				gone = union(gone, (m.start, m.end()));
			}
		}
		let l = &mut self.spell.live;
		l.n = w as u8;
		l.changed = union(l.changed, gone);
	}

	/// True when the caret is in or against the word `r`.
	fn caret_touches(&self, r: Range) -> bool {
		self.caret >= r.0 && self.caret <= r.1
	}

	/// The start of the word `p` is in or at the end of.
	fn word_start<S: Store + ?Sized>(&self, s: &S, mut p: u32) -> u32 {
		for _ in 0..WIDEN {
			let Some(q) = p.checked_sub(1) else {
				break;
			};
			let b = self.doc.body.read_byte(s, q);
			let inner = matches!(b, Some(b'\'' | 0x92))
				&& q.checked_sub(1)
					.and_then(|r| self.doc.body.read_byte(s, r))
					.is_some_and(wordish);
			if b.is_some_and(wordish) || inner {
				p = q;
			} else {
				break;
			}
		}
		p
	}

	/// The end of the word `p` is in or at the start of.
	fn word_end<S: Store + ?Sized>(&self, s: &S, mut p: u32, total: u32) -> u32 {
		for _ in 0..WIDEN {
			if p >= total {
				break;
			}
			let b = self.doc.body.read_byte(s, p);
			let inner = matches!(b, Some(b'\'' | 0x92))
				&& self.doc.body.read_byte(s, p + 1).is_some_and(wordish);
			if b.is_some_and(wordish) || inner {
				p += 1;
			} else {
				break;
			}
		}
		p
	}

	/// Chooses the next span to scan when none is in progress: the part of the text an edit touched, then the screen's unchecked ground.
	/// False when there is nothing to do.
	fn live_pick<S: Store + ?Sized>(&mut self, s: &mut S) -> bool {
		let l = self.spell.live;
		if l.cur < l.span.1 {
			return true;
		}
		let total = self.doc.body.len().saturating_sub(1);
		let (span, ext) = if !is_empty(l.dirty) {
			// What an edit touched is checked where it can be seen; ground that is off the screen is checked when it is scrolled to.
			let (dl, dh) = (l.dirty.0.max(l.vis.0), l.dirty.1.min(l.vis.1));
			self.spell.live.dirty = NONE;
			// Ground an edit touched and that is not on the screen is no longer known to be checked.
			if dl > dh || is_empty(l.vis) || dl > l.dirty.0 || dh < l.dirty.1 {
				self.spell.live.done = NONE;
			}
			if dl > dh || is_empty(l.vis) {
				return false;
			}
			let lo = self.word_start(s, dl.min(total));
			let dh = dh.min(total);
			let in_word = dh
				.checked_sub(1)
				.and_then(|q| self.doc.body.read_byte(s, q))
				.is_some_and(wordish);
			// The edit's own end takes in the word that starts there; the edge of the screen only the word it is in the middle of.
			let hi = if in_word || dh == l.dirty.1.min(total) {
				self.word_end(s, dh, total)
			} else {
				dh
			};
			((lo, hi.max(lo + 1).min(total.max(lo))), false)
		} else {
			let (vl, vh) = (l.vis.0, l.vis.1.min(total));
			if vl >= vh {
				return false;
			}
			let (dl, dh) = l.done;
			let span = if is_empty(l.done) || vh <= dl || vl >= dh {
				self.spell.live.done = NONE;
				(vl, vh)
			} else if vl < dl {
				(vl, dl)
			} else if vh > dh {
				(dh, vh)
			} else {
				return false;
			};
			((self.word_start(s, span.0), span.1), true)
		};
		self.mark_clear_range(s, span);
		let l = &mut self.spell.live;
		l.span = span;
		l.cur = span.0;
		l.ext = ext;
		!is_empty(span)
	}

	/// True when something is left to scan.
	fn live_more(&self) -> bool {
		let l = &self.spell.live;
		if l.cur < l.span.1 || !is_empty(l.dirty) {
			return true;
		}
		if !is_empty(l.hold) && !self.caret_touches(l.hold) {
			return true;
		}
		let total = self.doc.body.len().saturating_sub(1);
		let (vl, vh) = (l.vis.0, l.vis.1.min(total));
		if vl >= vh {
			return false;
		}
		let (dl, dh) = l.done;
		is_empty(l.done) || vh <= dl || vl >= dh || vl < dl || vh > dh
	}

	/// Checks up to `budget` words of the visible text, then says whether more is left. Call it when the keyboard has been quiet: an
	/// empty call costs next to nothing, and each word costs one dictionary lookup (about a tenth of a millisecond on a 486).
	pub fn spell_idle<S: Store + ?Sized>(&mut self, s: &mut S, budget: u32) -> bool {
		if !self.spell.live.on || self.spell.dict.is_none() || self.mode != Mode::Edit {
			return false;
		}
		self.live_sync(s);
		// The word that was held back is checked once the caret has left it.
		let l = self.spell.live;
		if !is_empty(l.hold) && !self.caret_touches(l.hold) {
			self.spell.live.dirty = union(l.dirty, l.hold);
			self.spell.live.hold = NONE;
		}
		let total = self.doc.body.len().saturating_sub(1);
		let mut left = budget;
		while left > 0 && self.live_pick(s) {
			while left > 0 && self.spell.live.cur < self.spell.live.span.1 {
				let l = self.spell.live;
				let Some((st, len)) = self.scan_word(s, l.cur, total) else {
					self.spell.live.cur = l.span.1;
					break;
				};
				if st >= l.span.1 {
					self.spell.live.cur = l.span.1;
					break;
				}
				self.spell.live.cur = st + len;
				let mut w = [0u8; MAX_WORD];
				let Some(n) = self.checkable(s, st, len, &mut w) else {
					continue;
				};
				// The word being typed waits for the caret to leave it.
				let typed = self.spell.live.typed;
				if self.caret_touches((st, st + len)) && st <= typed.1 && st + len >= typed.0 {
					self.spell.live.hold = (st, st + len);
					continue;
				}
				left -= 1;
				let word = w.get(..n).unwrap_or(&[]);
				let known = self.spell.dict.as_ref().is_some_and(|d| d.known(s, word));
				if !(known || self.user_has(word) || self.ignored(word)) {
					self.mark_add(s, st, len, word);
				}
			}
			let l = &mut self.spell.live;
			if l.cur >= l.span.1 && l.ext && !is_empty(l.span) {
				let hi = l.span.1.max(l.cur);
				l.done = if is_empty(l.done) {
					(l.span.0, hi)
				} else {
					(l.done.0.min(l.span.0), l.done.1.max(hi))
				};
			}
		}
		self.live_more()
	}

	/// The range the marks changed in since the last call, for the shell to paint again; none when nothing changed.
	pub fn spell_take_changed(&mut self) -> Option<(u32, u32)> {
		let c = core::mem::replace(&mut self.spell.live.changed, NONE);
		(!is_empty(c)).then_some(c)
	}

	/// The marked words that overlap `lo..hi`, each as its first position and the position after its last, in order.
	pub fn spell_marks<'a, S: Store + ?Sized>(
		&self,
		s: &'a S,
		lo: u32,
		hi: u32,
	) -> impl Iterator<Item = (u32, u32)> + 'a {
		let l = self.spell.live;
		let ok = l.on && l.synced == self.doc.body.len() && l.view == self.view;
		let (chunk, n) = match l.chunk {
			Some(c) if ok => (c, usize::from(l.n)),
			_ => (ChunkId(0), 0),
		};
		(0..n)
			.filter_map(move |i| read_mark(s, chunk, i))
			.take_while(move |m| m.start < hi)
			.filter(move |m| m.end() > lo)
			.map(|m| (m.start, m.end()))
	}

	/// The marked word at `pos` (in it, or just after it), as its first position and the position after its last.
	pub fn spell_mark_at<S: Store + ?Sized>(&self, s: &S, pos: u32) -> Option<(u32, u32)> {
		self.spell_marks(s, pos, pos.saturating_add(1))
			.next()
			.or_else(|| {
				self.spell_marks(s, pos.saturating_sub(1), pos)
					.find(|m| m.1 == pos)
			})
	}

	/// The text of the marked word at `pos`, in `out`; its length.
	pub fn spell_word_at<S: Store + ?Sized>(
		&self,
		s: &S,
		pos: u32,
		out: &mut [u8; MAX_WORD],
	) -> usize {
		let Some((st, en)) = self.spell_mark_at(s, pos) else {
			return 0;
		};
		let n = (en - st) as usize;
		if n > MAX_WORD {
			return 0;
		}
		for (k, d) in out.iter_mut().take(n).enumerate() {
			*d = self.doc.body.read_byte(s, st + k as u32).unwrap_or(b' ');
		}
		n
	}

	/// Up to eight suggestions for the marked word at `pos`, closest first; none when no word is marked there.
	pub fn spell_suggestions_at<S: Store + ?Sized>(&self, s: &S, pos: u32) -> Suggestions {
		let mut w = [0u8; MAX_WORD];
		let n = self.spell_word_at(s, pos, &mut w);
		match (&self.spell.dict, w.get(..n)) {
			(Some(d), Some(word)) if n > 0 => {
				let mut tmp = [0u8; MAX_WORD];
				for (d, b) in tmp.iter_mut().zip(word) {
					*d = if *b == 0x92 { b'\'' } else { *b };
				}
				d.suggest(s, tmp.get(..n).unwrap_or(&[]), BUDGET)
			}
			_ => Suggestions::new(),
		}
	}

	/// Replaces the marked word at `pos` with `with`, as one undo step; the caret goes after it. False when no word is marked there.
	pub fn spell_replace_at<S: Store + ?Sized>(
		&mut self,
		s: &mut S,
		pos: u32,
		with: &[u8],
	) -> bool {
		let Some((lo, hi)) = self.spell_mark_at(s, pos) else {
			return false;
		};
		let t = &self.doc.body;
		let cfmt = t.cfmt_at(s, lo).unwrap_or(0);
		let pfmt = t.pfmt_at(s, lo).unwrap_or(0);
		self.end_typing();
		self.doc.body.begin_group(crate::label::TYPING);
		let ok = self
			.doc
			.body
			.delete(s, lo, hi - lo, crate::label::TYPING)
			.is_ok() && (with.is_empty()
			|| self
				.doc
				.body
				.insert(s, lo, with, cfmt, pfmt, crate::label::TYPING)
				.is_ok());
		self.doc.body.end_group();
		// The caret stays where it was in the text: after the word when it was after it, and at the end of the new word when it was in it.
		self.anchor = None;
		let new = with.len() as u32;
		self.caret = if self.caret >= hi {
			self.caret + new - (hi - lo)
		} else if self.caret > lo {
			lo + new
		} else {
			self.caret
		};
		self.modified = true;
		self.spell_note_edit(s, lo, hi - lo, new);
		ok
	}

	/// Leaves the marked word at `pos` as it is, here and everywhere else in this session (Ignore All).
	pub fn spell_ignore_at<S: Store + ?Sized>(&mut self, s: &mut S, pos: u32) -> bool {
		let mut w = [0u8; MAX_WORD];
		let n = self.spell_word_at(s, pos, &mut w);
		if n == 0 {
			return false;
		}
		self.ignore_word(w.get(..n).unwrap_or(&[]));
		self.spell_recheck();
		true
	}

	/// Adds the marked word at `pos` to the user dictionary (`QUILL.UDC`).
	pub fn spell_add_at<S: Store + ?Sized, F: Files>(
		&mut self,
		s: &mut S,
		f: &mut F,
		pos: u32,
	) -> bool {
		let mut w = [0u8; MAX_WORD];
		let n = self.spell_word_at(s, pos, &mut w);
		if n == 0 {
			return false;
		}
		let ok = self.user_add(f, w.get(..n).unwrap_or(&[]));
		self.spell_recheck();
		ok
	}
}

fn read_mark<S: Store + ?Sized>(s: &S, c: ChunkId, i: usize) -> Option<Mark> {
	if i >= CAP {
		return None;
	}
	let mut b = [0u8; REC];
	s.read(c, (i * REC) as u16, &mut b);
	let start = u32::from_le_bytes([*b.first()?, *b.get(1)?, *b.get(2)?, *b.get(3)?]);
	let len = u16::from_le_bytes([*b.get(4)?, *b.get(5)?]);
	let hash = u16::from_le_bytes([*b.get(6)?, *b.get(7)?]);
	Some(Mark { start, len, hash })
}

#[cfg(test)]
mod tests {
	use super::*;

	/// What the scanner costs the data segment of the Windows 3.x build: a few integers, nothing that grows.
	#[test]
	fn the_scanner_state_is_small() {
		let n = core::mem::size_of::<Live>();
		assert!(n <= 96, "{n} bytes");
	}
}
