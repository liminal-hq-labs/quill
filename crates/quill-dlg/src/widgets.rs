// The dialog widgets' state: text field, list, drop-down, check box and radio group. No drawing: a painter reads the state.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Every widget keeps its invariants whatever keys arrive and whatever sizes it is given, so a painter can index with what it reads
// without checking.

use crate::key::DlgKey;

/// What a [`Field`] takes.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FieldKind {
	/// A file name: upper-cased, and spaces, control characters and anything above `~` are dropped.
	Name,
	/// Free text: any character from space to `~`, and the CP437 range above 0x7F, in the case typed.
	Text,
}

/// A caret and the other end of the selection, for a text held somewhere else (a `[u8; N]` and a length): the editing rules of
/// [`Field`] on any buffer, so the app's own fields (Find, the forms, the file name prompts) edit the same way. Both ends are offsets
/// from the start of the text and never past its length; every method takes the length it is working with and keeps them there.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Caret {
	at: u16,
	anchor: u16,
}

/// True for the characters a word is made of.
fn word_char(c: u8) -> bool {
	c.is_ascii_alphanumeric() || c == b'_' || c >= 0x80
}

impl Caret {
	/// A caret at the start with nothing selected.
	pub const fn new() -> Self {
		Caret { at: 0, anchor: 0 }
	}

	/// A caret at the end of `len` bytes with nothing selected: where a field starts when it is filled in for the user.
	pub const fn at_end(len: usize) -> Self {
		let e = if len > 65535 { 65535 } else { len as u16 };
		Caret { at: e, anchor: e }
	}

	/// The caret, from 0 to the length.
	pub fn at(&self) -> usize {
		usize::from(self.at)
	}

	/// The selected range, start before end; empty when nothing is selected.
	pub fn selection(&self) -> (usize, usize) {
		let (a, c) = (usize::from(self.anchor), usize::from(self.at));
		(a.min(c), a.max(c))
	}

	pub fn has_selection(&self) -> bool {
		self.anchor != self.at
	}

	/// Brings both ends back inside a text of `len` bytes.
	pub fn clamp(&mut self, len: usize) {
		let m = len.min(65535) as u16;
		self.at = self.at.min(m);
		self.anchor = self.anchor.min(m);
	}

	/// Selects all of `len` bytes, caret at the end.
	pub fn select_all(&mut self, len: usize) {
		self.anchor = 0;
		self.at = len.min(65535) as u16;
	}

	/// Puts the caret at the end of `len` bytes with nothing selected.
	pub fn end(&mut self, len: usize) {
		*self = Caret::at_end(len);
	}

	/// A press or a drag at character offset `pos`: the caret goes there, and the selection is dropped unless `extend` keeps its
	/// other end (a drag, or a click with Shift).
	pub fn place(&mut self, pos: usize, len: usize, extend: bool) {
		let p = pos.min(len).min(65535) as u16;
		self.at = p;
		if !extend {
			self.anchor = p;
		}
	}

	/// A double click at `pos`: selects the word there, or the run of spaces or punctuation if it is not on a word. At the end of
	/// the text it takes the last run.
	pub fn select_word(&mut self, text: &[u8], pos: usize) {
		let len = text.len();
		if len == 0 {
			*self = Caret::new();
			return;
		}
		let i = pos.min(len - 1);
		let class = |k: usize| -> u8 {
			match text.get(k) {
				Some(c) if word_char(*c) => 0,
				Some(b' ') => 1,
				_ => 2,
			}
		};
		let c = class(i);
		let mut s = i;
		while s > 0 && class(s - 1) == c {
			s -= 1;
		}
		let mut e = i + 1;
		while e < len && class(e) == c {
			e += 1;
		}
		self.anchor = s.min(65535) as u16;
		self.at = e.min(65535) as u16;
	}

	fn remove(&mut self, buf: &mut [u8], len: &mut usize, start: usize, end: usize) {
		let l = (*len).min(buf.len());
		let end = end.min(l);
		let start = start.min(end);
		buf.copy_within(end..l, start);
		*len = l - (end - start);
		self.at = start.min(65535) as u16;
		self.anchor = self.at;
	}

	fn delete_selection(&mut self, buf: &mut [u8], len: &mut usize) -> bool {
		let (s, e) = self.selection();
		if s == e {
			return false;
		}
		self.remove(buf, len, s, e);
		true
	}

	fn insert(&mut self, buf: &mut [u8], len: &mut usize, c: u8, overtype: bool) {
		self.delete_selection(buf, len);
		let at = self.at().min(*len);
		let l = *len;
		if overtype && at < l {
			if let Some(slot) = buf.get_mut(at) {
				*slot = c;
			}
		} else {
			if l >= buf.len() {
				return;
			}
			buf.copy_within(at..l, at + 1);
			if let Some(slot) = buf.get_mut(at) {
				*slot = c;
			}
			*len = l + 1;
		}
		self.at = (at + 1).min(65535) as u16;
		self.anchor = self.at;
	}

	/// Handles an editing or movement key on the text `buf[..*len]` (`accepts` says which characters it takes, and may change them).
	/// Returns true when the key was one of the field's, even if nothing changed, so the caller does not treat it as something else.
	pub fn key(
		&mut self,
		k: DlgKey,
		buf: &mut [u8],
		len: &mut usize,
		accepts: &dyn Fn(u8) -> Option<u8>,
		overtype: &mut bool,
	) -> bool {
		*len = (*len).min(buf.len());
		self.clamp(*len);
		let l = *len;
		match k {
			DlgKey::Char(c) => {
				if let Some(c) = accepts(c) {
					self.insert(buf, len, c, *overtype);
				}
				true
			}
			DlgKey::Backspace => {
				if !self.delete_selection(buf, len) && self.at > 0 {
					let at = self.at();
					self.remove(buf, len, at - 1, at);
				}
				true
			}
			DlgKey::Delete => {
				if !self.delete_selection(buf, len) && self.at() < l {
					let at = self.at();
					self.remove(buf, len, at, at + 1);
				}
				true
			}
			DlgKey::Left => {
				let (s, _) = self.selection();
				self.at = if self.has_selection() {
					s as u16
				} else {
					self.at.saturating_sub(1)
				};
				self.anchor = self.at;
				true
			}
			DlgKey::Right => {
				let (_, e) = self.selection();
				self.at = if self.has_selection() {
					e as u16
				} else {
					(self.at + 1).min(l as u16)
				};
				self.anchor = self.at;
				true
			}
			DlgKey::Home => {
				*self = Caret::new();
				true
			}
			DlgKey::End => {
				self.end(l);
				true
			}
			DlgKey::ShiftLeft => {
				self.at = self.at.saturating_sub(1);
				true
			}
			DlgKey::ShiftRight => {
				self.at = (self.at + 1).min(l as u16);
				true
			}
			DlgKey::ShiftHome => {
				self.at = 0;
				true
			}
			DlgKey::ShiftEnd => {
				self.at = l as u16;
				true
			}
			DlgKey::Insert => {
				*overtype = !*overtype;
				true
			}
			_ => false,
		}
	}
}

/// A one-line text field of at most `N` bytes (`N` up to 255), with a caret, a selection and overtype.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Field<const N: usize> {
	bytes: [u8; N],
	len: u8,
	caret: Caret,
	kind: FieldKind,
	overtype: bool,
}

impl<const N: usize> Field<N> {
	/// An empty field.
	pub const fn new(kind: FieldKind) -> Self {
		const { assert!(N <= 255) };
		Field {
			bytes: [0; N],
			len: 0,
			caret: Caret::new(),
			kind,
			overtype: false,
		}
	}

	/// The text.
	pub fn as_bytes(&self) -> &[u8] {
		self.bytes.get(..usize::from(self.len)).unwrap_or(&[])
	}

	pub fn len(&self) -> usize {
		usize::from(self.len)
	}

	pub fn is_empty(&self) -> bool {
		self.len == 0
	}

	/// The caret, from 0 to the length.
	pub fn cursor(&self) -> usize {
		self.caret.at()
	}

	/// The selected range, start before end; empty when nothing is selected.
	pub fn selection(&self) -> (usize, usize) {
		self.caret.selection()
	}

	pub fn has_selection(&self) -> bool {
		self.caret.has_selection()
	}

	pub fn overtype(&self) -> bool {
		self.overtype
	}

	fn accepts_of(kind: FieldKind, c: u8) -> Option<u8> {
		match kind {
			FieldKind::Name if (0x21..=0x7E).contains(&c) => Some(c.to_ascii_uppercase()),
			FieldKind::Text if (0x20..=0x7E).contains(&c) || c >= 0x80 => Some(c),
			_ => None,
		}
	}

	/// Replaces the text with `text` (what does not fit or is not accepted is dropped) and selects all of it, so typing replaces it.
	pub fn set(&mut self, text: &[u8]) {
		self.len = 0;
		for c in text {
			if let Some(c) = Self::accepts_of(self.kind, *c) {
				if usize::from(self.len) < N {
					if let Some(slot) = self.bytes.get_mut(usize::from(self.len)) {
						*slot = c;
						self.len += 1;
					}
				}
			}
		}
		self.caret.select_all(self.len());
	}

	/// Selects all the text, caret at the end.
	pub fn select_all(&mut self) {
		self.caret.select_all(self.len());
	}

	/// Puts the caret at the end with nothing selected.
	pub fn end(&mut self) {
		self.caret.end(self.len());
	}

	/// A press or a drag at character offset `pos` (see [`Caret::place`]).
	pub fn place(&mut self, pos: usize, extend: bool) {
		self.caret.place(pos, self.len(), extend);
	}

	/// A double click at `pos`: selects the word there.
	pub fn select_word(&mut self, pos: usize) {
		let text = self.bytes.get(..usize::from(self.len)).unwrap_or(&[]);
		self.caret.select_word(text, pos);
	}

	/// Handles an editing or movement key. Returns true when the key was one of the field's (even if nothing changed), so the
	/// caller does not treat it as something else.
	pub fn key(&mut self, k: DlgKey) -> bool {
		let kind = self.kind;
		let mut len = usize::from(self.len);
		let r = self.caret.key(
			k,
			&mut self.bytes,
			&mut len,
			&|c| Self::accepts_of(kind, c),
			&mut self.overtype,
		);
		self.len = len as u8;
		r
	}
}

/// The state of a scrolling list of `count` entries, `rows` of which show at once. The entries themselves live elsewhere; this
/// holds the selection and the top row.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct List {
	count: u16,
	rows: u8,
	sel: u16,
	top: u16,
}

impl List {
	/// An empty list showing `rows` rows (at least 1).
	pub const fn new(rows: u8) -> Self {
		List {
			count: 0,
			rows: if rows == 0 { 1 } else { rows },
			sel: 0,
			top: 0,
		}
	}

	pub fn count(&self) -> usize {
		usize::from(self.count)
	}

	pub fn rows(&self) -> usize {
		usize::from(self.rows)
	}

	/// The selected entry, or `None` when the list is empty.
	pub fn selected(&self) -> Option<usize> {
		(self.count > 0).then_some(usize::from(self.sel))
	}

	/// The first entry shown.
	pub fn top(&self) -> usize {
		usize::from(self.top)
	}

	/// True when the entries do not all fit, so a scroll bar shows.
	pub fn overflows(&self) -> bool {
		self.count > u16::from(self.rows)
	}

	/// Sets the number of entries (the selection and top row are kept in range).
	pub fn set_count(&mut self, count: usize) {
		self.count = count.min(usize::from(u16::MAX)) as u16;
		self.fix();
	}

	/// Sets the number of rows shown (at least 1).
	pub fn set_rows(&mut self, rows: u8) {
		self.rows = rows.max(1);
		self.fix();
	}

	/// Selects entry `i` (the last one if past the end) and scrolls it into view.
	pub fn select(&mut self, i: usize) {
		self.sel = i.min(usize::from(u16::MAX)) as u16;
		self.fix();
	}

	/// Sets the top row, as a scroll bar does. The selection moves only if it would fall off the list.
	pub fn scroll_to(&mut self, top: usize) {
		let max_top = self.count.saturating_sub(u16::from(self.rows));
		self.top = (top.min(usize::from(u16::MAX)) as u16).min(max_top);
		let last_shown = self
			.top
			.saturating_add(u16::from(self.rows))
			.saturating_sub(1);
		self.sel = self.sel.clamp(self.top, last_shown.max(self.top));
		self.fix();
	}

	fn fix(&mut self) {
		let rows = u16::from(self.rows);
		self.sel = self.sel.min(self.count.saturating_sub(1));
		let max_top = self.count.saturating_sub(rows);
		self.top = self.top.min(max_top);
		if self.sel < self.top {
			self.top = self.sel;
		}
		if self.sel >= self.top.saturating_add(rows) {
			self.top = self.sel + 1 - rows;
		}
	}

	/// Handles Up, Down, PgUp, PgDn, Home and End. Returns true when the key was one of the list's.
	pub fn key(&mut self, k: DlgKey) -> bool {
		let rows = u16::from(self.rows);
		let page = rows.saturating_sub(1).max(1);
		match k {
			DlgKey::Up => self.sel = self.sel.saturating_sub(1),
			DlgKey::Down => self.sel = self.sel.saturating_add(1),
			DlgKey::Home => self.sel = 0,
			DlgKey::End => self.sel = self.count.saturating_sub(1),
			DlgKey::PgUp => {
				self.sel = if self.sel > self.top {
					self.top
				} else {
					self.sel.saturating_sub(page)
				};
			}
			DlgKey::PgDn => {
				let bottom = self.top.saturating_add(rows).saturating_sub(1);
				self.sel = if self.sel < bottom {
					bottom
				} else {
					self.sel.saturating_add(page)
				};
			}
			_ => return false,
		}
		self.fix();
		true
	}

	/// Jumps to the next entry after the selection (wrapping) whose first character is `c`, as a list box does. `first(i)` gives
	/// the first character of entry `i`. Returns true when the selection moved.
	pub fn type_ahead(&mut self, c: u8, first: impl Fn(usize) -> u8) -> bool {
		let n = self.count();
		for step in 1..=n {
			let i = (usize::from(self.sel) + step) % n.max(1);
			if first(i).eq_ignore_ascii_case(&c) {
				let moved = i != usize::from(self.sel);
				self.select(i);
				return moved;
			}
		}
		false
	}
}

/// Where the scroll bar's thumb sits in a track of `track` cells for a list of `count` entries, `rows` showing, from `top`.
/// Always inside the track; 0 when everything fits.
pub fn thumb(count: usize, rows: usize, top: usize, track: usize) -> usize {
	let span = count.saturating_sub(rows);
	if span == 0 || track <= 1 {
		return 0;
	}
	(top.min(span) * (track - 1) + span / 2) / span
}

/// What a key did to a [`Combo`].
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ComboEvent {
	/// Not a key of the combo, or no effect
	None,
	/// The choice changed (apply it)
	Changed,
	/// The list opened
	Opened,
	/// The list closed with the choice unchanged
	Closed,
}

/// A drop-down list: shows one choice, and opens to a list of at most [`Combo::DROP_ROWS`] rows.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Combo {
	count: u8,
	sel: u8,
	before: u8,
	open: bool,
	list: List,
}

impl Combo {
	/// The most rows the drop-down shows.
	pub const DROP_ROWS: u8 = 6;

	pub const fn new() -> Self {
		Combo {
			count: 0,
			sel: 0,
			before: 0,
			open: false,
			list: List::new(Self::DROP_ROWS),
		}
	}

	/// Sets the number of choices (up to 255). Closes the list.
	pub fn set_count(&mut self, count: usize) {
		self.count = count.min(255) as u8;
		self.sel = self.sel.min(self.count.saturating_sub(1));
		self.open = false;
		self.list.set_count(usize::from(self.count));
		self.list.set_rows(Self::DROP_ROWS.min(self.count).max(1));
	}

	/// Chooses `i` (the last choice if past the end).
	pub fn set(&mut self, i: usize) {
		self.sel = i.min(usize::from(self.count.saturating_sub(1))) as u8;
		self.list.select(usize::from(self.sel));
	}

	/// The choice, or `None` when there are no choices.
	pub fn selected(&self) -> Option<usize> {
		(self.count > 0).then_some(usize::from(self.sel))
	}

	pub fn count(&self) -> usize {
		usize::from(self.count)
	}

	pub fn is_open(&self) -> bool {
		self.open
	}

	/// The drop-down list, for a painter: its top row and highlight.
	pub fn list(&self) -> &List {
		&self.list
	}

	fn open_list(&mut self) -> ComboEvent {
		if self.count == 0 {
			return ComboEvent::None;
		}
		self.open = true;
		self.before = self.sel;
		self.list.select(usize::from(self.sel));
		ComboEvent::Opened
	}

	fn commit(&mut self) -> ComboEvent {
		self.open = false;
		let chosen = self.list.selected().unwrap_or(0).min(255) as u8;
		if chosen != self.before {
			self.sel = chosen;
			ComboEvent::Changed
		} else {
			ComboEvent::Closed
		}
	}

	/// Lights row `i` of the open list, as the pointer passing over it does.
	pub fn light(&mut self, i: usize) {
		if self.open {
			self.list.select(i);
		}
	}

	/// Chooses row `i` of the open list and closes it, as a click on it does.
	pub fn pick(&mut self, i: usize) -> ComboEvent {
		self.light(i);
		if self.open {
			self.commit()
		} else {
			ComboEvent::None
		}
	}

	/// Handles a key. Closed, Up, Down, Home and End change the choice in place, and F4 or Alt+Down opens the list. Open, those keys
	/// move the highlight, Enter (or F4, Alt+Down) chooses it and Esc closes the list without changing the choice. Anything else
	/// gives `None` and is the caller's.
	pub fn key(&mut self, k: DlgKey) -> ComboEvent {
		if self.open {
			return match k {
				DlgKey::Up
				| DlgKey::Down
				| DlgKey::Home
				| DlgKey::End
				| DlgKey::PgUp
				| DlgKey::PgDn => {
					self.list.key(k);
					ComboEvent::None
				}
				DlgKey::Enter | DlgKey::F4 | DlgKey::AltDown => self.commit(),
				DlgKey::Esc => {
					self.open = false;
					self.sel = self.before;
					self.list.select(usize::from(self.sel));
					ComboEvent::Closed
				}
				_ => ComboEvent::None,
			};
		}
		let last = self.count.saturating_sub(1);
		let next = match k {
			DlgKey::F4 | DlgKey::AltDown => return self.open_list(),
			DlgKey::Up => self.sel.saturating_sub(1),
			DlgKey::Down => self.sel.saturating_add(1).min(last),
			DlgKey::Home => 0,
			DlgKey::End => last,
			_ => return ComboEvent::None,
		};
		if self.count > 0 && next != self.sel {
			self.sel = next;
			self.list.select(usize::from(next));
			ComboEvent::Changed
		} else {
			ComboEvent::None
		}
	}
}

impl Default for Combo {
	fn default() -> Self {
		Combo::new()
	}
}

/// A check box.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Check {
	pub checked: bool,
}

impl Check {
	/// Space toggles. Returns true when the key was one of the box's.
	pub fn key(&mut self, k: DlgKey) -> bool {
		if k == DlgKey::Char(b' ') {
			self.checked = !self.checked;
			true
		} else {
			false
		}
	}
}

/// A group of radio buttons, one of which is chosen.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Radio {
	count: u8,
	sel: u8,
}

impl Radio {
	pub const fn new(count: u8) -> Self {
		Radio { count, sel: 0 }
	}

	pub fn selected(&self) -> usize {
		usize::from(self.sel)
	}

	pub fn set(&mut self, i: usize) {
		self.sel = i.min(usize::from(self.count.saturating_sub(1))) as u8;
	}

	/// Up and Left choose the previous button, Down and Right the next, wrapping round. Returns true when the key was one of the group's.
	pub fn key(&mut self, k: DlgKey) -> bool {
		if self.count == 0 {
			return false;
		}
		match k {
			DlgKey::Up | DlgKey::Left => {
				self.sel = if self.sel == 0 {
					self.count - 1
				} else {
					self.sel - 1
				}
			}
			DlgKey::Down | DlgKey::Right => {
				self.sel = if self.sel + 1 >= self.count {
					0
				} else {
					self.sel + 1
				}
			}
			_ => return false,
		}
		true
	}
}

#[cfg(test)]
#[allow(
	clippy::unwrap_used,
	clippy::expect_used,
	clippy::panic,
	clippy::indexing_slicing
)]
mod tests {
	use super::*;
	use crate::key::tests::any_key;
	use proptest::prelude::*;
	use std::vec::Vec;

	fn typed<const N: usize>(f: &mut Field<N>, s: &str) {
		for c in s.bytes() {
			f.key(DlgKey::Char(c));
		}
	}

	#[test]
	fn a_press_places_the_caret_and_a_drag_extends_from_where_it_began() {
		let mut f = Field::<16>::new(FieldKind::Text);
		typed(&mut f, "hello world");
		f.place(5, false);
		assert_eq!((f.cursor(), f.selection()), (5, (5, 5)));
		f.place(9, true);
		assert_eq!((f.cursor(), f.selection()), (9, (5, 9)));
		f.place(2, true);
		assert_eq!(
			(f.cursor(), f.selection()),
			(2, (2, 5)),
			"dragged back past where it began"
		);
		f.place(99, false);
		assert_eq!(f.cursor(), 11, "past the end is the end");
		f.place(3, false);
		f.key(DlgKey::Char(b'X'));
		assert_eq!(f.as_bytes(), b"helXlo world");
		f.key(DlgKey::Delete);
		f.key(DlgKey::Delete);
		assert_eq!(f.as_bytes(), b"helX world");
	}

	#[test]
	fn a_double_click_selects_the_word_the_spaces_or_the_punctuation_under_it() {
		let mut f = Field::<24>::new(FieldKind::Text);
		typed(&mut f, "one  two,three_4");
		let word = |f: &mut Field<24>, at: usize| {
			f.select_word(at);
			f.selection()
		};
		assert_eq!(word(&mut f, 1), (0, 3));
		assert_eq!(word(&mut f, 3), (3, 5), "the spaces");
		assert_eq!(word(&mut f, 6), (5, 8));
		assert_eq!(word(&mut f, 8), (8, 9), "a comma is a run of its own");
		assert_eq!(
			word(&mut f, 12),
			(9, 16),
			"an underscore and digits belong to a word"
		);
		assert_eq!(word(&mut f, 99), (9, 16), "past the end takes the last run");
		let mut e = Field::<4>::new(FieldKind::Text);
		e.select_word(3);
		assert_eq!(e.selection(), (0, 0));
	}

	#[test]
	fn a_caret_on_a_buffer_the_field_does_not_own_edits_the_same_way() {
		let mut buf = [0u8; 8];
		let mut len = 0usize;
		let mut c = Caret::new();
		let mut over = false;
		let any = |b: u8| Some(b);
		for b in b"abcdef" {
			c.key(DlgKey::Char(*b), &mut buf, &mut len, &any, &mut over);
		}
		c.place(2, len, false);
		c.key(DlgKey::Backspace, &mut buf, &mut len, &any, &mut over);
		assert_eq!(&buf[..len], b"acdef");
		assert_eq!(c.at(), 1);
		c.select_all(len);
		c.key(DlgKey::Char(b'z'), &mut buf, &mut len, &any, &mut over);
		assert_eq!(&buf[..len], b"z");
		// A full buffer takes no more, and a stale length or caret is brought back inside.
		for _ in 0..20 {
			c.key(DlgKey::Char(b'q'), &mut buf, &mut len, &any, &mut over);
		}
		assert_eq!(len, 8);
		let mut stale = Caret::at_end(500);
		let mut big = 500usize;
		stale.key(DlgKey::Left, &mut buf, &mut big, &any, &mut over);
		assert_eq!((big, stale.at()), (8, 7));
	}

	proptest! {
		#[test]
		fn a_caret_keeps_inside_its_text_for_any_keys_and_presses(
			ops in proptest::collection::vec((any_key(), 0usize..40, 0u8..3), 0..80),
		) {
			let mut f = Field::<12>::new(FieldKind::Text);
			for (k, pos, how) in ops {
				match how {
					0 => { f.key(k); }
					1 => f.place(pos, pos % 2 == 0),
					_ => f.select_word(pos),
				}
				prop_assert!(f.cursor() <= f.len());
				let (s, e) = f.selection();
				prop_assert!(s <= e && e <= f.len());
			}
		}
	}

	#[test]
	fn a_name_field_upper_cases_and_drops_spaces() {
		let mut f = Field::<12>::new(FieldKind::Name);
		typed(&mut f, "let ter.qld\x01\u{e9}");
		assert_eq!(f.as_bytes(), b"LETTER.QLD");
		let mut t = Field::<12>::new(FieldKind::Text);
		typed(&mut t, "a b");
		assert_eq!(t.as_bytes(), b"a b");
	}

	#[test]
	fn typing_replaces_the_selection() {
		let mut f = Field::<12>::new(FieldKind::Name);
		f.set(b"letter.qld");
		assert_eq!(f.selection(), (0, 10));
		f.key(DlgKey::Char(b'm'));
		assert_eq!(f.as_bytes(), b"M");
		assert_eq!(f.cursor(), 1);
		assert!(!f.has_selection());
	}

	#[test]
	fn editing_keys() {
		let mut f = Field::<8>::new(FieldKind::Name);
		typed(&mut f, "abcdef");
		f.key(DlgKey::Left);
		f.key(DlgKey::Left);
		f.key(DlgKey::Delete);
		assert_eq!(f.as_bytes(), b"ABCDF");
		f.key(DlgKey::Backspace);
		assert_eq!(f.as_bytes(), b"ABCF");
		f.key(DlgKey::Home);
		f.key(DlgKey::Backspace);
		assert_eq!(f.as_bytes(), b"ABCF");
		f.key(DlgKey::ShiftEnd);
		assert_eq!(f.selection(), (0, 4));
		f.key(DlgKey::Left);
		assert_eq!((f.selection(), f.cursor()), ((0, 0), 0));
		f.key(DlgKey::End);
		f.key(DlgKey::ShiftLeft);
		f.key(DlgKey::ShiftLeft);
		assert_eq!(f.selection(), (2, 4));
		f.key(DlgKey::Delete);
		assert_eq!(f.as_bytes(), b"AB");
		f.key(DlgKey::Insert);
		f.key(DlgKey::Home);
		f.key(DlgKey::Char(b'x'));
		assert_eq!(f.as_bytes(), b"XB");
		f.key(DlgKey::Tab);
		assert!(!f.key(DlgKey::Tab));
	}

	#[test]
	fn a_full_field_refuses_more() {
		let mut f = Field::<3>::new(FieldKind::Name);
		typed(&mut f, "abcdef");
		assert_eq!(f.as_bytes(), b"ABC");
		f.set(b"longer");
		assert_eq!(f.as_bytes(), b"LON");
	}

	fn list(count: usize, rows: u8) -> List {
		let mut l = List::new(rows);
		l.set_count(count);
		l
	}

	#[test]
	fn list_selection_scrolls_into_view() {
		let mut l = list(20, 5);
		for _ in 0..7 {
			l.key(DlgKey::Down);
		}
		assert_eq!((l.selected(), l.top()), (Some(7), 3));
		l.key(DlgKey::PgUp);
		assert_eq!(l.selected(), Some(3));
		l.key(DlgKey::PgUp);
		assert_eq!((l.selected(), l.top()), (Some(0), 0));
		l.key(DlgKey::PgDn);
		assert_eq!(l.selected(), Some(4));
		l.key(DlgKey::PgDn);
		assert_eq!(l.selected(), Some(8));
		l.key(DlgKey::End);
		assert_eq!((l.selected(), l.top()), (Some(19), 15));
		l.key(DlgKey::Down);
		assert_eq!(l.selected(), Some(19));
		l.key(DlgKey::Home);
		assert_eq!((l.selected(), l.top()), (Some(0), 0));
		assert!(!l.key(DlgKey::Tab));
	}

	#[test]
	fn an_empty_list_selects_nothing() {
		let mut l = list(0, 5);
		for k in [DlgKey::Down, DlgKey::End, DlgKey::PgDn, DlgKey::Up] {
			assert!(l.key(k));
		}
		assert_eq!((l.selected(), l.top()), (None, 0));
		assert!(!l.overflows());
		assert!(!l.type_ahead(b'a', |_| b'A'));
	}

	#[test]
	fn type_ahead_walks_the_entries_that_start_with_the_letter() {
		let names = *b"ABBCB";
		let mut l = list(5, 3);
		assert!(l.type_ahead(b'b', |i| names[i]));
		assert_eq!(l.selected(), Some(1));
		assert!(l.type_ahead(b'B', |i| names[i]));
		assert_eq!(l.selected(), Some(2));
		assert!(l.type_ahead(b'b', |i| names[i]));
		assert_eq!(l.selected(), Some(4));
		assert!(l.type_ahead(b'b', |i| names[i]));
		assert_eq!(l.selected(), Some(1));
		assert!(!l.type_ahead(b'z', |i| names[i]));
		assert_eq!(l.selected(), Some(1));
		assert!(l.type_ahead(b'c', |i| names[i]));
		assert_eq!(l.selected(), Some(3));
		// The only match is the selection itself: it stays, and nothing moved.
		assert!(!l.type_ahead(b'c', |i| names[i]));
	}

	#[test]
	fn the_thumb_stays_in_the_track() {
		assert_eq!(thumb(5, 8, 0, 6), 0);
		assert_eq!(thumb(20, 5, 0, 8), 0);
		assert_eq!(thumb(20, 5, 15, 8), 7);
		assert_eq!(thumb(20, 5, 99, 8), 7);
		assert_eq!(thumb(20, 5, 7, 1), 0);
		assert_eq!(thumb(20, 5, 7, 0), 0);
	}

	#[test]
	fn a_combo_changes_in_place_and_opens_on_f4() {
		let mut c = Combo::new();
		c.set_count(3);
		assert_eq!(c.key(DlgKey::Down), ComboEvent::Changed);
		assert_eq!(c.selected(), Some(1));
		assert_eq!(c.key(DlgKey::End), ComboEvent::Changed);
		assert_eq!(c.key(DlgKey::Down), ComboEvent::None);
		assert_eq!(c.key(DlgKey::Tab), ComboEvent::None);
		assert_eq!(c.key(DlgKey::F4), ComboEvent::Opened);
		assert!(c.is_open());
		assert_eq!(c.key(DlgKey::Up), ComboEvent::None);
		assert_eq!(c.key(DlgKey::Esc), ComboEvent::Closed);
		assert_eq!((c.is_open(), c.selected()), (false, Some(2)));
		assert_eq!(c.key(DlgKey::AltDown), ComboEvent::Opened);
		c.key(DlgKey::Home);
		assert_eq!(c.key(DlgKey::Enter), ComboEvent::Changed);
		assert_eq!((c.is_open(), c.selected()), (false, Some(0)));
		assert_eq!(c.key(DlgKey::F4), ComboEvent::Opened);
		assert_eq!(c.key(DlgKey::Enter), ComboEvent::Closed);
	}

	#[test]
	fn an_empty_combo_does_not_open() {
		let mut c = Combo::new();
		assert_eq!(c.key(DlgKey::F4), ComboEvent::None);
		assert_eq!(c.key(DlgKey::Down), ComboEvent::None);
		assert_eq!(c.selected(), None);
	}

	#[test]
	fn check_and_radio() {
		let mut c = Check::default();
		assert!(c.key(DlgKey::Char(b' ')));
		assert!(c.checked);
		assert!(!c.key(DlgKey::Char(b'x')));
		let mut r = Radio::new(3);
		r.key(DlgKey::Up);
		assert_eq!(r.selected(), 2);
		r.key(DlgKey::Right);
		assert_eq!(r.selected(), 0);
		r.key(DlgKey::Down);
		assert_eq!(r.selected(), 1);
		assert!(!r.key(DlgKey::Tab));
		assert!(!Radio::new(0).key(DlgKey::Down));
	}

	proptest! {
		#[test]
		fn a_field_keeps_its_invariants(keys in proptest::collection::vec(any_key(), 0..80), sel_first in any::<bool>()) {
			let mut f = Field::<12>::new(FieldKind::Name);
			if sel_first {
				f.set(b"seed.txt");
			}
			for k in keys {
				f.key(k);
				prop_assert!(f.len() <= 12);
				prop_assert!(f.cursor() <= f.len());
				let (s, e) = f.selection();
				prop_assert!(s <= e && e <= f.len());
				prop_assert!(f.as_bytes().iter().all(|c| (0x21..=0x7E).contains(c) && !c.is_ascii_lowercase()));
			}
		}

		#[test]
		fn a_text_field_keeps_its_invariants(keys in proptest::collection::vec(any_key(), 0..80)) {
			let mut f = Field::<5>::new(FieldKind::Text);
			for k in keys {
				f.key(k);
				prop_assert!(f.len() <= 5 && f.cursor() <= f.len() && f.selection().1 <= f.len());
			}
		}

		#[test]
		fn a_list_keeps_the_selection_on_screen(
			count in 0usize..70000, rows in 0u8..30, start in 0usize..70000,
			ops in proptest::collection::vec(prop_oneof![
				any_key().prop_map(Some),
				(0usize..80000).prop_map(|_| None),
			], 0..60),
			tops in proptest::collection::vec(0usize..80000, 0..60),
		) {
			let mut l = List::new(rows);
			l.set_count(count);
			l.select(start);
			let mut tops = tops.into_iter();
			for op in ops {
				match op {
					Some(k) => { l.key(k); }
					None => { if let Some(t) = tops.next() { l.scroll_to(t); } }
				}
				let rows = l.rows();
				prop_assert!(rows >= 1);
				match l.selected() {
					None => prop_assert_eq!(l.top(), 0),
					Some(s) => {
						prop_assert!(s < l.count());
						prop_assert!(l.top() <= s, "top {} after sel {}", l.top(), s);
						prop_assert!(s < l.top() + rows, "sel {} off the bottom, top {}", s, l.top());
						prop_assert!(l.top() <= l.count().saturating_sub(rows));
					}
				}
				prop_assert!(thumb(l.count(), rows, l.top(), 10) < 10);
			}
		}

		#[test]
		fn resizing_a_list_keeps_it_valid(count in 0usize..300, rows in 0u8..30, sel in 0usize..300, count2 in 0usize..300, rows2 in 0u8..30) {
			let mut l = List::new(rows);
			l.set_count(count);
			l.select(sel);
			l.set_count(count2);
			l.set_rows(rows2);
			if let Some(s) = l.selected() {
				prop_assert!(s < count2 && l.top() <= s && s < l.top() + l.rows());
			}
		}

		#[test]
		fn type_ahead_only_lands_on_a_match(names in proptest::collection::vec(0x41u8..0x45, 0..40), c in 0x41u8..0x47, start in 0usize..50) {
			let mut l = List::new(4);
			l.set_count(names.len());
			l.select(start);
			let before = l.selected();
			if l.type_ahead(c, |i| names[i]) {
				let s = l.selected().unwrap();
				prop_assert!(names[s].eq_ignore_ascii_case(&c));
				prop_assert!(Some(s) != before);
			} else {
				prop_assert_eq!(l.selected(), before);
			}
		}

		#[test]
		fn a_combo_keeps_its_choice_in_range(count in 0usize..300, keys in proptest::collection::vec(any_key(), 0..60)) {
			let mut c = Combo::new();
			c.set_count(count);
			let mut keys: Vec<DlgKey> = keys;
			keys.truncate(60);
			for k in keys {
				let was_open = c.is_open();
				let before = c.selected();
				let ev = c.key(k);
				match c.selected() {
					None => prop_assert_eq!(c.count(), 0),
					Some(s) => prop_assert!(s < c.count()),
				}
				if c.is_open() {
					prop_assert!(c.list().selected().is_some());
				}
				// Esc never changes the choice, and the list opening never does either.
				if (was_open && k == DlgKey::Esc) || ev == ComboEvent::Opened {
					prop_assert_eq!(c.selected(), before);
				}
				if ev == ComboEvent::Changed {
					prop_assert!(c.selected() != before);
				}
			}
		}

		#[test]
		fn a_radio_group_stays_in_range(count in 0u8..8, keys in proptest::collection::vec(any_key(), 0..40)) {
			let mut r = Radio::new(count);
			for k in keys {
				r.key(k);
				prop_assert!(r.selected() < usize::from(count.max(1)));
			}
		}
	}
}
