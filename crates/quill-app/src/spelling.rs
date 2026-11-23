// Spelling: the word scanner, the dialog's state and the commands behind Find > Spelling (F7), for the text dialog and the native one.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The dictionary (`EN_CA.DIC` and its siblings) is read into `Store` chunks the first time it is needed. The user dictionary is a
// plain text file, `QUILL.UDC`, one word a line, shared by every language. Words in all capitals, words with digits, words in the
// user dictionary and words ignored for this session are skipped.
use quill_core::layout::Measure;
use quill_core::store::Store;
use quill_spell::{is_letter, lower, upper, Dict, Suggestions, MAX_WORD};

use crate::find::FindState;
use crate::{App, Files, Key, Mode};
use quill_formats::io::{Sink, Source};

/// Words ignored for the session.
const IGNORE: usize = 24;
/// Bytes of user dictionary kept in memory.
const USER: usize = 2048;
/// The nodes the suggestion search may visit.
const BUDGET: u32 = 40_000;

/// What the spelling check keeps between words.
pub(crate) struct SpellData {
	pub dict: Option<Dict>,
	/// 0 Canadian, 1 American, 2 British.
	pub lang: u8,
	ignore: [[u8; MAX_WORD]; IGNORE],
	ignore_len: [u8; IGNORE],
	ignore_n: usize,
	user: [u8; USER],
	user_len: usize,
	/// The word under the dialog: where it starts, how long it is and its text.
	pub word: [u8; MAX_WORD],
	pub wlen: usize,
	/// Where the check began, and whether it has wrapped round the end of the document.
	origin: u32,
	wrapped: bool,
}

impl SpellData {
	pub(crate) const fn new() -> Self {
		SpellData {
			dict: None,
			lang: 0,
			ignore: [[0; MAX_WORD]; IGNORE],
			ignore_len: [0; IGNORE],
			ignore_n: 0,
			user: [0; USER],
			user_len: 0,
			word: [0; MAX_WORD],
			wlen: 0,
			origin: 0,
			wrapped: false,
		}
	}
}

/// The dialog of the text build.
#[derive(Copy, Clone, PartialEq, Eq)]
pub(crate) struct SpellDlg {
	/// The chosen suggestion, 0 to 7, or 255 when there are none.
	pub sel: u8,
	/// The button with the focus: 0 Ignore, 1 Ignore All, 2 Change, 3 Change All, 4 Add, 5 Cancel.
	pub focus: u8,
	/// How many suggestions there are, and the words, each at most 31 bytes after a length byte.
	pub n: u8,
	pub words: [[u8; MAX_WORD]; 8],
	pub lens: [u8; 8],
}

pub(crate) const BUTTONS: [&str; 6] = [
	"Ignore",
	"Ignore All",
	"Change",
	"Change All",
	"Add",
	"Cancel",
];

fn dic_name(lang: u8) -> &'static [u8] {
	match lang {
		1 => b"EN_US.DIC",
		2 => b"EN_GB.DIC",
		_ => b"EN_CA.DIC",
	}
}

fn same_word(a: &[u8], b: &[u8]) -> bool {
	a.len() == b.len() && a.iter().zip(b).all(|(x, y)| lower(*x) == lower(*y))
}

impl<M: Measure + Clone> App<M> {
	/// Chooses the language of the dictionary (0 Canadian, 1 American, 2 British); the next check opens its file.
	pub fn set_spelling_language<S: Store + ?Sized>(&mut self, s: &mut S, lang: u8) {
		let lang = lang.min(2);
		if lang != self.spell.lang {
			if let Some(mut d) = self.spell.dict.take() {
				d.free(s);
			}
			self.spell.lang = lang;
		}
	}

	/// The language chosen (0 Canadian, 1 American, 2 British).
	pub fn spelling_language(&self) -> u8 {
		self.spell.lang
	}

	/// Opens the dictionary and the user dictionary if they are not open. False (with a message) when there is no dictionary.
	pub fn spell_ready<S: Store + ?Sized, F: Files>(&mut self, s: &mut S, f: &mut F) -> bool {
		if self.spell.dict.is_none() {
			let loaded = f
				.open(dic_name(self.spell.lang))
				.and_then(|mut r| Dict::load(s, &mut r).ok());
			match loaded {
				Some(d) => self.spell.dict = Some(d),
				None => {
					self.msg.set("The spelling dictionary was not found.");
					return false;
				}
			}
			if self.spell.user_len == 0 {
				if let Some(mut r) = f.open(b"QUILL.UDC") {
					let n = r.read(&mut self.spell.user);
					self.spell.user_len = n;
				}
			}
		}
		true
	}

	fn user_has(&self, w: &[u8]) -> bool {
		self.spell
			.user
			.get(..self.spell.user_len)
			.unwrap_or(&[])
			.split(|b| *b == b'\n')
			.any(|l| same_word(l.strip_suffix(b"\r").unwrap_or(l), w))
	}

	fn ignored(&self, w: &[u8]) -> bool {
		(0..self.spell.ignore_n).any(|i| {
			let n = usize::from(self.spell.ignore_len.get(i).copied().unwrap_or(0));
			self.spell
				.ignore
				.get(i)
				.and_then(|b| b.get(..n))
				.is_some_and(|b| same_word(b, w))
		})
	}

	/// The next word from `from` before `limit`: its start and length, with the letters, apostrophes and digits it is made of.
	fn scan_word<S: Store + ?Sized>(&self, s: &S, mut p: u32, limit: u32) -> Option<(u32, u32)> {
		let t = &self.doc.body;
		let wordish = |b: u8| is_letter(b) || b.is_ascii_digit();
		while p < limit {
			let b = t.read_byte(s, p)?;
			if !wordish(b) {
				p += 1;
				continue;
			}
			let start = p;
			let mut end = p;
			while end < limit {
				let c = t.read_byte(s, end)?;
				let inner = (c == b'\'' || c == 0x92)
					&& end + 1 < limit
					&& t.read_byte(s, end + 1).is_some_and(wordish);
				if wordish(c) || inner {
					end += 1;
				} else {
					break;
				}
			}
			return Some((start, end - start));
		}
		None
	}

	/// True when the word at `start` of `len` bytes is one to look at: it has no digits, is not in capitals, and is long enough.
	fn checkable<S: Store + ?Sized>(
		&self,
		s: &S,
		start: u32,
		len: u32,
		out: &mut [u8; MAX_WORD],
	) -> Option<usize> {
		if len as usize > MAX_WORD - 1 {
			return None;
		}
		let t = &self.doc.body;
		let mut letters = 0;
		let mut capitals = 0;
		for i in 0..len as usize {
			let b = t.read_byte(s, start + i as u32)?;
			if b.is_ascii_digit() {
				return None;
			}
			if is_letter(b) {
				letters += 1;
				if upper(b) == b {
					capitals += 1;
				}
			}
			*out.get_mut(i)? = if b == 0x92 { b'\'' } else { b };
		}
		if letters < 2 || capitals == letters {
			return None;
		}
		Some(len as usize)
	}

	/// Finds the next misspelled word from the caret (or from the end of the word just dealt with), selects it and returns true. When
	/// the whole document has been checked it says so and returns false.
	pub fn spell_next<S: Store + ?Sized>(&mut self, s: &mut S) -> bool {
		let total = self.doc.body.len().saturating_sub(1);
		let mut from = self.selection().map_or(self.caret, |(_, hi)| hi);
		loop {
			while let Some((st, len)) = self.scan_word(s, from, total) {
				// After the wrap the check stops at the word it began in, whole.
				if self.spell.wrapped && st >= self.spell.origin {
					break;
				}
				from = st + len;
				let mut w = [0u8; MAX_WORD];
				let Some(n) = self.checkable(s, st, len, &mut w) else {
					continue;
				};
				let word = w.get(..n).unwrap_or(&[]);
				let known = self.spell.dict.as_ref().is_some_and(|d| d.known(s, word));
				if known || self.user_has(word) || self.ignored(word) {
					continue;
				}
				self.spell.word = w;
				self.spell.wlen = n;
				self.anchor = Some(st);
				self.caret = st + len;
				self.scroll(s);
				return true;
			}
			if self.spell.wrapped || self.spell.origin == 0 {
				break;
			}
			self.spell.wrapped = true;
			from = 0;
		}
		self.anchor = None;
		self.msg.set("The spelling check is complete.");
		false
	}

	/// Begins a check at the caret.
	pub fn spell_begin<S: Store + ?Sized, F: Files>(&mut self, s: &mut S, f: &mut F) -> bool {
		if !self.spell_ready(s, f) {
			return false;
		}
		self.anchor = None;
		self.spell.origin = self.caret;
		self.spell.wrapped = false;
		self.spell_next(s)
	}

	/// Ends the check: nothing stays selected.
	pub fn spell_cancel(&mut self) {
		self.anchor = None;
	}

	/// The word the dialog shows.
	pub fn spell_word(&self) -> &[u8] {
		self.spell.word.get(..self.spell.wlen).unwrap_or(&[])
	}

	/// The sentence around the word, up to 70 bytes, for the dialog.
	pub fn spell_context<S: Store + ?Sized>(&self, s: &S, out: &mut [u8; 72]) -> usize {
		let Some((lo, hi)) = self.selection() else {
			return 0;
		};
		let t = &self.doc.body;
		let from = lo.saturating_sub(30);
		let to = (hi + 30).min(t.len().saturating_sub(1));
		let mut n = 0;
		for p in from..to {
			let b = t.read_byte(s, p).unwrap_or(b' ');
			if let Some(d) = out.get_mut(n) {
				*d = if b < 0x20 { b' ' } else { b };
				n += 1;
			}
		}
		n
	}

	/// The suggestions for the word the dialog shows.
	pub fn spell_suggestions<S: Store + ?Sized>(&self, s: &S) -> Suggestions {
		match &self.spell.dict {
			Some(d) => d.suggest(s, self.spell_word(), BUDGET),
			None => Suggestions::new(),
		}
	}

	/// Leaves the word as it is for this one occurrence and goes on.
	pub fn spell_ignore<S: Store + ?Sized>(&mut self, s: &mut S, all: bool) -> bool {
		if all {
			let n = self.spell.wlen.min(MAX_WORD);
			let i = self.spell.ignore_n % IGNORE;
			let w = self.spell.word;
			if let (Some(d), Some(l)) = (
				self.spell.ignore.get_mut(i),
				self.spell.ignore_len.get_mut(i),
			) {
				*d = w;
				*l = n as u8;
			}
			self.spell.ignore_n = (self.spell.ignore_n + 1).min(IGNORE);
		}
		self.anchor = None;
		self.spell_next(s)
	}

	/// Replaces the word with `with` (every occurrence when `all`, as one undo step) and goes on.
	pub fn spell_change<S: Store + ?Sized>(&mut self, s: &mut S, with: &[u8], all: bool) -> bool {
		let mut st = FindState::new(false, self.spell_word());
		let n = with.len().min(st.with.len());
		if let (Some(d), Some(sv)) = (st.with.get_mut(..n), with.get(..n)) {
			d.copy_from_slice(sv);
		}
		st.wlen = n;
		st.case = true;
		st.word = true;
		if all {
			self.replace_all(s, &st);
			self.msg.set("");
		} else {
			let old = self.spell.wlen as u32;
			self.replace_here(s, &st);
			// The place the check began moves with the text before it.
			self.spell.origin = (self.spell.origin + n as u32).saturating_sub(old);
		}
		self.spell_next(s)
	}

	/// Adds the word to the user dictionary file and goes on.
	pub fn spell_add<S: Store + ?Sized, F: Files>(&mut self, s: &mut S, f: &mut F) -> bool {
		let w = self.spell_word();
		let n = w.len();
		if self.spell.user_len + n + 2 <= USER {
			let at = self.spell.user_len;
			let word = self.spell.word;
			if let Some(d) = self.spell.user.get_mut(at..at + n) {
				d.copy_from_slice(word.get(..n).unwrap_or(&[]));
			}
			if let Some(d) = self.spell.user.get_mut(at + n..at + n + 2) {
				d.copy_from_slice(b"\r\n");
			}
			self.spell.user_len = at + n + 2;
			if let Some(mut w) = f.create(b"QUILL.UDC") {
				let _ = w.write(self.spell.user.get(..self.spell.user_len).unwrap_or(&[]));
				f.close(w);
			}
		} else {
			self.msg.set("The user dictionary is full.");
		}
		self.anchor = None;
		self.spell_next(s)
	}

	/// Opens the text dialog on the word that was just found.
	pub(crate) fn open_spell_dialog<S: Store + ?Sized>(&mut self, s: &S) {
		let sg = self.spell_suggestions(s);
		let mut dlg = SpellDlg {
			sel: if sg.is_empty() { 255 } else { 0 },
			focus: 2,
			n: sg.len() as u8,
			words: [[0; MAX_WORD]; 8],
			lens: [0; 8],
		};
		for i in 0..sg.len() {
			if let (Some(c), Some(w), Some(l)) =
				(sg.get(i), dlg.words.get_mut(i), dlg.lens.get_mut(i))
			{
				*w = c.text;
				*l = c.len;
			}
		}
		self.mode = Mode::Spell(dlg);
	}

	/// Keys in the text dialog: Up and Down choose a suggestion, Tab moves along the buttons, Enter presses one, Escape cancels.
	#[inline(never)]
	pub(crate) fn key_spell<S: Store + ?Sized, F: Files>(
		&mut self,
		s: &mut S,
		f: &mut F,
		k: Key,
		mut d: SpellDlg,
	) {
		match k {
			Key::Esc => {
				self.mode = Mode::Edit;
				self.anchor = None;
				return;
			}
			Key::Up if d.n > 0 => d.sel = d.sel.checked_sub(1).unwrap_or(d.n - 1),
			Key::Down if d.n > 0 => d.sel = (d.sel + 1) % d.n,
			Key::Tab | Key::Right => d.focus = (d.focus + 1) % 6,
			Key::Left => d.focus = (d.focus + 5) % 6,
			Key::Enter => {
				let with: [u8; MAX_WORD] = d
					.words
					.get(usize::from(d.sel))
					.copied()
					.unwrap_or([0; MAX_WORD]);
				let wl = usize::from(d.lens.get(usize::from(d.sel)).copied().unwrap_or(0));
				let with = with.get(..wl).unwrap_or(&[]);
				let more = match d.focus {
					0 => self.spell_ignore(s, false),
					1 => self.spell_ignore(s, true),
					2 | 3 if wl == 0 => true,
					2 => self.spell_change(s, with, false),
					3 => self.spell_change(s, with, true),
					4 => self.spell_add(s, f),
					_ => {
						self.mode = Mode::Edit;
						self.anchor = None;
						return;
					}
				};
				if more {
					self.open_spell_dialog(s);
				} else {
					self.mode = Mode::Edit;
				}
				return;
			}
			_ => {}
		}
		self.mode = Mode::Spell(d);
	}
}
