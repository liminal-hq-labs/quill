// quill-spell: word lookup, the case rules and suggestions over a `.DIC` automaton that lives in `Store` chunks.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The format is described in `quill-dict`, which builds it. The whole file is read into chunks of at most 32 KB (about 8 for a
// 250 KB dictionary) when it is opened, so a lookup is a walk over edges that the `Store` hands out by chunk and offset; the paging of
// the architecture (four resident pages) can replace the loader without changing a lookup (D88).
#![no_std]
#![deny(
	clippy::unwrap_used,
	clippy::expect_used,
	clippy::panic,
	clippy::indexing_slicing
)]

use quill_core::store::{ChunkId, Store, MAX_CHUNK};
use quill_formats::io::Source;

const HEADER: usize = 32;
const END: u32 = 1 << 8;
const LAST: u32 = 1 << 9;
const CHILD_SHIFT: u32 = 10;
/// Chunks a dictionary may occupy (512 KB).
const MAX_PAGES: usize = 16;
/// The longest word the suggestion search works with.
pub const MAX_WORD: usize = 32;
/// Suggestions kept.
pub const MAX_SUGGESTIONS: usize = 8;
/// Edits a suggestion may be from the word.
const MAX_EDITS: u8 = 2;

/// Why a dictionary could not be opened.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SpellError {
	/// Not a `.DIC`, or damaged.
	BadFile,
	/// More than 512 KB, or the store has no room.
	TooLarge,
}

/// An open dictionary.
pub struct Dict {
	chunks: [ChunkId; MAX_PAGES],
	pages: usize,
	edges: u32,
	words: u32,
	lic_off: u32,
	lic_len: u32,
	language: u16,
}

/// A word and how far it is from the one that was looked up.
#[derive(Copy, Clone)]
pub struct Candidate {
	/// The word, in the dictionary's case.
	pub text: [u8; MAX_WORD],
	/// Its length.
	pub len: u8,
	/// Edits from the word (0 to 2).
	pub cost: u8,
}

const EMPTY: Candidate = Candidate {
	text: [0; MAX_WORD],
	len: 0,
	cost: 0,
};

impl Candidate {
	/// The word's bytes.
	pub fn bytes(&self) -> &[u8] {
		self.text.get(..usize::from(self.len)).unwrap_or(&[])
	}
}

/// The best suggestions, closest first.
pub struct Suggestions {
	list: [Candidate; MAX_SUGGESTIONS],
	n: usize,
	/// True when the search ran out of its budget before it had seen everything.
	pub partial: bool,
}

impl Suggestions {
	/// An empty list.
	pub const fn new() -> Self {
		Suggestions {
			list: [EMPTY; MAX_SUGGESTIONS],
			n: 0,
			partial: false,
		}
	}

	/// How many there are.
	pub fn len(&self) -> usize {
		self.n
	}

	/// True when there are none.
	pub fn is_empty(&self) -> bool {
		self.n == 0
	}

	/// Suggestion `i`.
	pub fn get(&self, i: usize) -> Option<&Candidate> {
		self.list.get(..self.n)?.get(i)
	}

	/// How many letters differ between the two words as bags of letters (a swap or a doubled letter is cheap, a different letter dear).
	fn bag(c: &[u8], w: &[u8]) -> usize {
		let mut n = [0i16; 26];
		for b in c {
			if let Some(x) = n.get_mut(usize::from(lower(*b).wrapping_sub(b'a'))) {
				*x += 1;
			}
		}
		for b in w {
			if let Some(x) = n.get_mut(usize::from(lower(*b).wrapping_sub(b'a'))) {
				*x -= 1;
			}
		}
		n.iter().map(|x| x.unsigned_abs() as usize).sum()
	}

	fn better(a: &Candidate, b: &Candidate, word: &[u8]) -> bool {
		let key = |c: &Candidate| {
			(
				c.cost,
				Self::bag(c.bytes(), word),
				usize::from(c.len).abs_diff(word.len()),
			)
		};
		match key(a).cmp(&key(b)) {
			core::cmp::Ordering::Equal => fold(a.bytes()) < fold(b.bytes()),
			o => o == core::cmp::Ordering::Less,
		}
	}

	fn offer(&mut self, c: Candidate, word: &[u8]) {
		if self
			.list
			.get(..self.n)
			.is_some_and(|l| l.iter().any(|x| fold(x.bytes()) == fold(c.bytes())))
		{
			return;
		}
		let mut at = self.n;
		for i in 0..self.n {
			if self.list.get(i).is_some_and(|x| Self::better(&c, x, word)) {
				at = i;
				break;
			}
		}
		if at >= MAX_SUGGESTIONS {
			return;
		}
		let last = self.n.min(MAX_SUGGESTIONS - 1);
		for j in (at..last).rev() {
			let moved = self.list.get(j).copied().unwrap_or(EMPTY);
			if let Some(slot) = self.list.get_mut(j + 1) {
				*slot = moved;
			}
		}
		if let Some(slot) = self.list.get_mut(at) {
			*slot = c;
		}
		self.n = (self.n + 1).min(MAX_SUGGESTIONS);
	}
}

impl Default for Suggestions {
	fn default() -> Self {
		Self::new()
	}
}

/// Lower-case form of a Windows-1252 letter.
pub fn lower(b: u8) -> u8 {
	match b {
		b'A'..=b'Z' | 0xC0..=0xD6 | 0xD8..=0xDE => b + 32,
		_ => b,
	}
}

/// Upper-case form of a Windows-1252 letter.
pub fn upper(b: u8) -> u8 {
	match b {
		b'a'..=b'z' | 0xE0..=0xF6 | 0xF8..=0xFE => b - 32,
		_ => b,
	}
}

/// A comparison key: the word with its letters in lower case, as a small iterator-free fingerprint.
fn fold(w: &[u8]) -> [u8; MAX_WORD] {
	let mut o = [0u8; MAX_WORD];
	for (d, s) in o.iter_mut().zip(w.iter()) {
		*d = lower(*s);
	}
	o
}

/// True for the letters words are made of: ASCII and Latin-1 letters.
pub fn is_letter(b: u8) -> bool {
	matches!(b, b'a'..=b'z' | b'A'..=b'Z' | 0xC0..=0xD6 | 0xD8..=0xF6 | 0xF8..=0xFF)
}

fn u32le(b: &[u8], at: usize) -> u32 {
	let g = |i: usize| u32::from(b.get(at + i).copied().unwrap_or(0));
	g(0) | g(1) << 8 | g(2) << 16 | g(3) << 24
}

impl Dict {
	/// Reads a `.DIC` from `src` into `Store` chunks.
	pub fn load<S: Store + ?Sized, R: Source>(s: &mut S, src: &mut R) -> Result<Dict, SpellError> {
		let mut d = Dict {
			chunks: [ChunkId(0); MAX_PAGES],
			pages: 0,
			edges: 0,
			words: 0,
			lic_off: 0,
			lic_len: 0,
			language: 0,
		};
		let mut buf = [0u8; 512];
		let mut total = 0usize;
		let mut head = [0u8; HEADER];
		loop {
			let n = src.read(&mut buf);
			if n == 0 {
				break;
			}
			let page = total / MAX_CHUNK;
			if page >= MAX_PAGES {
				d.free(s);
				return Err(SpellError::TooLarge);
			}
			if page == d.pages {
				let Some(c) = s.alloc(MAX_CHUNK as u16) else {
					d.free(s);
					return Err(SpellError::TooLarge);
				};
				if let Some(slot) = d.chunks.get_mut(page) {
					*slot = c;
				}
				d.pages += 1;
			}
			let at = total % MAX_CHUNK;
			let room = (MAX_CHUNK - at).min(n);
			if let (Some(c), Some(part)) = (d.chunks.get(page), buf.get(..room)) {
				s.write(*c, at as u16, part);
			}
			if let (Some(h), Some(part)) = (
				head.get_mut(total.min(HEADER)..(total + room).min(HEADER)),
				buf.get(..room.min(HEADER.saturating_sub(total))),
			) {
				h.copy_from_slice(part);
			}
			// A read never straddles a chunk edge: 512 divides 32768.
			total += room;
		}
		if total < HEADER || head.get(..4) != Some(b"QDIC") {
			d.free(s);
			return Err(SpellError::BadFile);
		}
		d.language = (u32le(&head, 4) >> 16) as u16;
		d.edges = u32le(&head, 8);
		d.words = u32le(&head, 16);
		d.lic_off = u32le(&head, 20);
		d.lic_len = u32le(&head, 24);
		if (HEADER as u64 + u64::from(d.edges) * 4) > total as u64 {
			d.free(s);
			return Err(SpellError::BadFile);
		}
		Ok(d)
	}

	/// Gives the chunks back.
	pub fn free<S: Store + ?Sized>(&mut self, s: &mut S) {
		for c in self.chunks.iter().take(self.pages) {
			s.free(*c);
		}
		self.pages = 0;
	}

	/// How many words the dictionary holds.
	pub fn words(&self) -> u32 {
		self.words
	}

	/// The language code (1 en_CA, 2 en_US, 3 en_GB).
	pub fn language(&self) -> u16 {
		self.language
	}

	/// Copies part of the licence text (SCOWL's, carried in every file) into `out`, from `from`.
	pub fn licence<S: Store + ?Sized>(&self, s: &S, from: u32, out: &mut [u8]) -> usize {
		let n = (self.lic_len.saturating_sub(from) as usize).min(out.len());
		let mut done = 0;
		while done < n {
			let off = self.lic_off as usize + from as usize + done;
			let (page, at) = (off / MAX_CHUNK, off % MAX_CHUNK);
			let take = (MAX_CHUNK - at).min(n - done);
			if let (Some(c), Some(part)) = (self.chunks.get(page), out.get_mut(done..done + take)) {
				s.read(*c, at as u16, part);
			}
			done += take;
		}
		n
	}

	fn edge<S: Store + ?Sized>(&self, s: &S, i: u32) -> u32 {
		if i >= self.edges {
			return LAST;
		}
		let off = HEADER + i as usize * 4;
		let mut b = [0u8; 4];
		if let Some(c) = self.chunks.get(off / MAX_CHUNK) {
			s.read(*c, (off % MAX_CHUNK) as u16, &mut b);
		}
		u32le(&b, 0)
	}

	/// True when exactly `word` is in the dictionary.
	pub fn contains_exact<S: Store + ?Sized>(&self, s: &S, word: &[u8]) -> bool {
		if word.is_empty() {
			return false;
		}
		let mut node = 0u32;
		for (i, &b) in word.iter().enumerate() {
			let mut k = node;
			let next = loop {
				let e = self.edge(s, k);
				let letter = (e & 0xFF) as u8;
				if letter == b {
					break Some(e);
				}
				if letter > b || e & LAST != 0 {
					break None;
				}
				k += 1;
			};
			let Some(e) = next else {
				return false;
			};
			if i + 1 == word.len() {
				return e & END != 0;
			}
			let child = e >> CHILD_SHIFT;
			if child == 0 {
				return false;
			}
			node = child;
		}
		false
	}

	/// True when `word` is known under the case rules: it is in the dictionary as written, or it is all lower case, capitalised or all
	/// capitals and its lower-case form is.
	pub fn known<S: Store + ?Sized>(&self, s: &S, word: &[u8]) -> bool {
		if self.contains_exact(s, word) {
			return true;
		}
		if word.len() > MAX_WORD {
			return false;
		}
		let letters = |f: fn(u8) -> bool| word.iter().all(|b| f(*b) || !is_letter(*b));
		let is_lower = letters(|b| lower(b) == b);
		let is_upper = letters(|b| upper(b) == b);
		let is_cap = word.first().is_some_and(|b| upper(*b) == *b)
			&& word
				.get(1..)
				.is_some_and(|r| r.iter().all(|b| lower(*b) == *b));
		if !(is_lower || is_upper || is_cap) {
			return false;
		}
		let mut l = [0u8; MAX_WORD];
		for (d, b) in l.iter_mut().zip(word.iter()) {
			*d = lower(*b);
		}
		if self.contains_exact(s, l.get(..word.len()).unwrap_or(&[])) {
			return true;
		}
		// An all-capitals word may be a proper name or an abbreviation that the dictionary holds capitalised.
		if is_upper && word.len() > 1 {
			let mut c = l;
			if let Some(f) = c.first_mut() {
				*f = upper(*f);
			}
			return self.contains_exact(s, c.get(..word.len()).unwrap_or(&[]));
		}
		false
	}

	/// The words within two edits of `word` (a letter added, removed or changed, or two neighbours swapped), closest first, found by a
	/// walk of the automaton that stops after `budget` nodes. The words are in the dictionary's case, capitalised or in capitals to
	/// match `word`.
	pub fn suggest<S: Store + ?Sized>(&self, s: &S, word: &[u8], budget: u32) -> Suggestions {
		let mut out = Suggestions::new();
		let m = word.len();
		if m == 0 || m > MAX_WORD - 4 {
			return out;
		}
		let mut w = [0u8; MAX_WORD];
		for (d, b) in w.iter_mut().zip(word.iter()) {
			*d = lower(*b);
		}
		let mut st = Walk {
			w,
			m,
			rows: [[0u8; MAX_WORD + 1]; MAX_WORD + 2],
			prefix: [0u8; MAX_WORD + 2],
			left: budget,
		};
		for j in 0..=m {
			if let Some(r) = st.rows.first_mut().and_then(|r| r.get_mut(j)) {
				*r = j as u8;
			}
		}
		self.walk(s, &mut st, 0, 1, &mut out);
		out.partial = st.left == 0;
		// Match the case the word was typed in.
		let first_upper = word
			.first()
			.is_some_and(|b| upper(*b) == *b && lower(*b) != *b);
		let all_upper = m > 1 && word.iter().all(|b| upper(*b) == *b);
		for c in out.list.iter_mut().take(out.n) {
			if all_upper {
				for b in c.text.iter_mut().take(usize::from(c.len)) {
					*b = upper(*b);
				}
			} else if first_upper {
				if let Some(b) = c.text.first_mut() {
					*b = upper(*b);
				}
			}
		}
		out
	}

	fn walk<S: Store + ?Sized>(
		&self,
		s: &S,
		st: &mut Walk,
		node: u32,
		depth: usize,
		out: &mut Suggestions,
	) {
		if depth > MAX_WORD {
			return;
		}
		let mut k = node;
		loop {
			if st.left == 0 {
				return;
			}
			st.left -= 1;
			let e = self.edge(s, k);
			let letter = (e & 0xFF) as u8;
			if let Some(p) = st.prefix.get_mut(depth - 1) {
				*p = letter;
			}
			// The next row of the edit-distance table for this letter (with swaps of neighbours counted as one edit).
			let c = lower(letter);
			let mut min = u8::MAX;
			for j in 0..=st.m {
				let up = st
					.rows
					.get(depth - 1)
					.and_then(|r| r.get(j))
					.copied()
					.unwrap_or(99);
				let mut v = up.saturating_add(1);
				if j > 0 {
					let diag = st
						.rows
						.get(depth - 1)
						.and_then(|r| r.get(j - 1))
						.copied()
						.unwrap_or(99);
					let same = st.w.get(j - 1).copied() == Some(c);
					v = v.min(diag.saturating_add(u8::from(!same)));
					let left = st
						.rows
						.get(depth)
						.and_then(|r| r.get(j - 1))
						.copied()
						.unwrap_or(99);
					v = v.min(left.saturating_add(1));
					if depth >= 2 && j >= 2 {
						let prev_letter = lower(st.prefix.get(depth - 2).copied().unwrap_or(0));
						if st.w.get(j - 1).copied() == Some(prev_letter)
							&& st.w.get(j - 2).copied() == Some(c)
						{
							let two = st
								.rows
								.get(depth - 2)
								.and_then(|r| r.get(j - 2))
								.copied()
								.unwrap_or(99);
							v = v.min(two.saturating_add(1));
						}
					}
				}
				if let Some(r) = st.rows.get_mut(depth).and_then(|r| r.get_mut(j)) {
					*r = v;
				}
				min = min.min(v);
			}
			if min <= MAX_EDITS {
				let cost = st
					.rows
					.get(depth)
					.and_then(|r| r.get(st.m))
					.copied()
					.unwrap_or(99);
				if e & END != 0 && cost <= MAX_EDITS && depth <= MAX_WORD {
					let mut cand = EMPTY;
					cand.len = depth as u8;
					cand.cost = cost;
					for (d, b) in cand.text.iter_mut().zip(st.prefix.iter().take(depth)) {
						*d = *b;
					}
					out.offer(cand, st.w.get(..st.m).unwrap_or(&[]));
				}
				let child = e >> CHILD_SHIFT;
				if child != 0 {
					self.walk(s, st, child, depth + 1, out);
				}
			}
			if e & LAST != 0 {
				return;
			}
			k += 1;
		}
	}
}

struct Walk {
	w: [u8; MAX_WORD],
	m: usize,
	rows: [[u8; MAX_WORD + 1]; MAX_WORD + 2],
	prefix: [u8; MAX_WORD + 2],
	left: u32,
}
