// Find and replace: the dialog state, the search and the replacements.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use quill_core::store::Store;

use quill_core::layout::Measure;

use crate::{label, App, Mode};

/// Longest text in a find or replace field.
pub(crate) const FIELD: usize = 32;

/// What the dialog's focus is on, in Tab order.
#[derive(Copy, Clone, PartialEq, Eq)]
pub(crate) enum Focus {
	Find,
	Replace,
	Case,
	Word,
	FindNext,
	ReplaceOne,
	ReplaceAll,
	Close,
}

/// The Find or Replace dialog.
#[derive(Copy, Clone, PartialEq, Eq)]
pub(crate) struct FindState {
	pub replace: bool,
	pub find: [u8; FIELD],
	pub flen: usize,
	pub with: [u8; FIELD],
	pub wlen: usize,
	pub case: bool,
	pub word: bool,
	pub focus: Focus,
}

impl FindState {
	pub(crate) fn new(replace: bool, find: &[u8]) -> Self {
		let mut f = [0u8; FIELD];
		let n = find.len().min(FIELD);
		for (d, s) in f.iter_mut().zip(find.iter()) {
			*d = *s;
		}
		FindState {
			replace,
			find: f,
			flen: n,
			with: [0; FIELD],
			wlen: 0,
			case: false,
			word: false,
			focus: Focus::Find,
		}
	}

	fn order(&self) -> &'static [Focus] {
		if self.replace {
			&[
				Focus::Find,
				Focus::Replace,
				Focus::Case,
				Focus::Word,
				Focus::FindNext,
				Focus::ReplaceOne,
				Focus::ReplaceAll,
				Focus::Close,
			]
		} else {
			&[
				Focus::Find,
				Focus::Case,
				Focus::Word,
				Focus::FindNext,
				Focus::Close,
			]
		}
	}

	fn step(&mut self) {
		let o = self.order();
		let i = o.iter().position(|f| *f == self.focus).unwrap_or(0);
		self.focus = o.get((i + 1) % o.len()).copied().unwrap_or(Focus::Find);
	}

	fn field(&mut self) -> Option<(&mut [u8; FIELD], &mut usize)> {
		match self.focus {
			Focus::Find => Some((&mut self.find, &mut self.flen)),
			Focus::Replace => Some((&mut self.with, &mut self.wlen)),
			_ => None,
		}
	}

	pub(crate) fn needle(&self) -> &[u8] {
		self.find.get(..self.flen).unwrap_or(&[])
	}
}

fn same(a: u8, b: u8, case: bool) -> bool {
	if case {
		a == b
	} else {
		a.eq_ignore_ascii_case(&b)
	}
}

impl<M: Measure + Clone> App<M> {
	/// The position of the next match at or after `from` and before `limit` (a match must end by `limit`).
	fn search<S: Store + ?Sized>(
		&self,
		s: &S,
		from: u32,
		limit: u32,
		st: &FindState,
	) -> Option<u32> {
		let needle = st.needle();
		let m = needle.len() as u32;
		if m == 0 {
			return None;
		}
		let t = &self.doc.body;
		let mut p = from;
		let mut buf = [0u8; 320];
		while p + m <= limit {
			let want = ((limit - p) as usize).min(buf.len());
			let n = buf.get_mut(..want).map_or(0, |b| t.read(s, p, b));
			if (n as u32) < m {
				return None;
			}
			for i in 0..=(n as u32 - m) {
				let window = buf.get(i as usize..(i + m) as usize)?;
				if window
					.iter()
					.zip(needle)
					.all(|(a, b)| same(*a, *b, st.case))
					&& self.word_ok(s, p + i, m, st)
				{
					return Some(p + i);
				}
			}
			p += n as u32 - m + 1;
		}
		None
	}

	fn word_ok<S: Store + ?Sized>(&self, s: &S, at: u32, m: u32, st: &FindState) -> bool {
		if !st.word {
			return true;
		}
		let t = &self.doc.body;
		let before = at.checked_sub(1).and_then(|p| t.read_byte(s, p));
		let after = t.read_byte(s, at + m);
		!before.is_some_and(|b| b.is_ascii_alphanumeric())
			&& !after.is_some_and(|b| b.is_ascii_alphanumeric())
	}

	/// Finds the next match from the caret (or the end of the selection), wrapping once, and selects it.
	pub(crate) fn find_next<S: Store + ?Sized>(&mut self, s: &S, st: &FindState) -> bool {
		let len = self.doc.body.len();
		let m = st.flen as u32;
		let start = self.selection().map_or(self.caret, |(_, hi)| hi);
		let hit = self.search(s, start, len, st).or_else(|| {
			let r = self.search(s, 0, (start + m).min(len), st);
			if r.is_some() {
				return r;
			}
			None
		});
		match hit {
			Some(p) => {
				self.anchor = Some(p);
				self.caret = p + m;
				true
			}
			None => {
				self.msg.set_quoted("Cannot find ", st.needle(), ".");
				false
			}
		}
	}

	/// True when the selection is exactly a match for `st`, so Replace may replace it.
	pub(crate) fn selection_matches<S: Store + ?Sized>(&self, s: &S, st: &FindState) -> bool {
		self.selection().is_some_and(|(lo, hi)| {
			hi - lo == st.flen as u32 && self.search(s, lo, hi, st) == Some(lo)
		})
	}

	pub(crate) fn replace_here<S: Store + ?Sized>(&mut self, s: &mut S, st: &FindState) -> bool {
		let Some((lo, hi)) = self.selection() else {
			return false;
		};
		let t = &self.doc.body;
		let cfmt = t.cfmt_at(s, lo).unwrap_or(0);
		let pfmt = t.pfmt_at(s, lo).unwrap_or(0);
		let with = st.with.get(..st.wlen).unwrap_or(&[]);
		self.doc.body.begin_group(label::TYPING);
		let ok = self.doc.body.delete(s, lo, hi - lo, label::TYPING).is_ok()
			&& (with.is_empty()
				|| self
					.doc
					.body
					.insert(s, lo, with, cfmt, pfmt, label::TYPING)
					.is_ok());
		self.doc.body.end_group();
		self.anchor = None;
		self.caret = lo + with.len() as u32;
		self.modified = true;
		ok
	}

	pub(crate) fn replace_all<S: Store + ?Sized>(&mut self, s: &mut S, st: &FindState) {
		let mut n = 0u32;
		let mut from = 0u32;
		self.doc.body.begin_group(label::TYPING);
		loop {
			let len = self.doc.body.len();
			let Some(p) = self.search(s, from, len, st) else {
				break;
			};
			let t = &self.doc.body;
			let cfmt = t.cfmt_at(s, p).unwrap_or(0);
			let pfmt = t.pfmt_at(s, p).unwrap_or(0);
			let with = st.with.get(..st.wlen).unwrap_or(&[]);
			if self
				.doc
				.body
				.delete(s, p, st.flen as u32, label::TYPING)
				.is_err()
			{
				break;
			}
			if !with.is_empty()
				&& self
					.doc
					.body
					.insert(s, p, with, cfmt, pfmt, label::TYPING)
					.is_err()
			{
				break;
			}
			from = p + with.len() as u32;
			n += 1;
		}
		self.doc.body.end_group();
		self.anchor = None;
		if n > 0 {
			self.modified = true;
			self.caret = self.caret.min(self.doc.body.len().saturating_sub(1));
		}
		self.msg.set("Quill made ");
		let mut digits = [0u8; 10];
		let mut i = digits.len();
		let mut v = n;
		loop {
			i -= 1;
			if let Some(d) = digits.get_mut(i) {
				*d = b'0' + (v % 10) as u8;
			}
			v /= 10;
			if v == 0 {
				break;
			}
		}
		self.msg.push(digits.get(i..).unwrap_or(&[]));
		self.msg.push(if n == 1 {
			b" replacement."
		} else {
			b" replacements."
		});
	}

	pub(crate) fn open_find(&mut self, replace: bool) {
		let mut st = self
			.last_find
			.unwrap_or_else(|| FindState::new(replace, &[]));
		st.replace = replace;
		st.focus = Focus::Find;
		self.mode = Mode::Find(st);
	}

	#[inline(never)]
	pub(crate) fn key_find<S: Store + ?Sized>(
		&mut self,
		s: &mut S,
		k: crate::Key,
		mut st: FindState,
	) {
		use crate::Key;
		match k {
			Key::Esc => {
				self.mode = Mode::Edit;
				return;
			}
			Key::Tab => st.step(),
			Key::Char(c) if st.field().is_some() => {
				if let Some((buf, len)) = st.field() {
					if *len < FIELD {
						if let Some(slot) = buf.get_mut(*len) {
							*slot = c;
							*len += 1;
						}
					}
				}
			}
			Key::Char(b' ') => match st.focus {
				Focus::Case => st.case = !st.case,
				Focus::Word => st.word = !st.word,
				_ => {}
			},
			Key::Backspace => {
				if let Some((_, len)) = st.field() {
					*len = len.saturating_sub(1);
				}
			}
			Key::Enter => {
				let act = match st.focus {
					Focus::Case => {
						st.case = !st.case;
						None
					}
					Focus::Word => {
						st.word = !st.word;
						None
					}
					Focus::Close => {
						self.mode = Mode::Edit;
						return;
					}
					Focus::ReplaceOne => Some(1),
					Focus::ReplaceAll => Some(2),
					_ => Some(0),
				};
				if let Some(a) = act {
					if st.flen > 0 {
						self.mode = Mode::Edit;
						match a {
							0 => {
								self.find_next(s, &st);
							}
							1 => {
								if self.selection_matches(s, &st) {
									self.replace_here(s, &st);
								}
								self.find_next(s, &st);
							}
							_ => self.replace_all(s, &st),
						}
						self.last_find = Some(st);
						// The dialog stays open while editing continues, as the common dialogs do.
						self.mode = Mode::Find(st);
						return;
					}
				}
			}
			_ => {}
		}
		self.mode = Mode::Find(st);
	}
}
