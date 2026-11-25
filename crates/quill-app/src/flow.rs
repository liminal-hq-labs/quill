// The answering layer: what each dialog asks, what a shell tells the editor back, and the notices that follow.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// A shell shows one of two things: an `Ask` (a question with buttons and a default) or a `Notice` (a message box, or a line for
// the status bar). The editor produces the words and decides what each answer does, so a shell only chooses how to draw them
// and which button the person pressed. The Windows common dialogs, a text-cell box and the host's flow scripts all answer
// through the entry points here, and the wording, the order of events and the failure handling are the same for every build.
use quill_core::store::Store;

use crate::native::{Dialog, FindAction};
use crate::{App, Command, Files, NAME_MAX};

/// The most bytes of a question: a file name and its sentence.
pub const ASK_MAX: usize = NAME_MAX + 64;

/// The most bytes of a notice, as long as a status-line message.
pub const NOTICE_MAX: usize = 96;

/// The words of a question or a notice: ANSI bytes in a fixed buffer, so the core allocates nothing.
#[derive(Copy, Clone)]
pub struct Text<const N: usize> {
	buf: [u8; N],
	len: usize,
}

impl<const N: usize> Text<N> {
	fn new() -> Self {
		Text {
			buf: [0; N],
			len: 0,
		}
	}

	fn from_parts(parts: &[&[u8]]) -> Self {
		let mut t = Text::new();
		for p in parts {
			t.push(p);
		}
		t
	}

	fn push(&mut self, b: &[u8]) {
		for &c in b {
			if let Some(slot) = self.buf.get_mut(self.len) {
				*slot = c;
				self.len += 1;
			}
		}
	}

	/// The bytes.
	pub fn as_bytes(&self) -> &[u8] {
		self.buf.get(..self.len).unwrap_or(&[])
	}
}

impl<const N: usize> PartialEq for Text<N> {
	fn eq(&self, other: &Self) -> bool {
		self.as_bytes() == other.as_bytes()
	}
}

impl<const N: usize> Eq for Text<N> {}

impl<const N: usize> core::fmt::Debug for Text<N> {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		match core::str::from_utf8(self.as_bytes()) {
			Ok(s) => write!(f, "{s:?}"),
			Err(_) => write!(f, "{:?}", self.as_bytes()),
		}
	}
}

/// What a person can answer a question with.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Answer {
	/// Yes.
	Yes,
	/// No.
	No,
	/// Cancel (and Esc).
	Cancel,
}

/// The buttons of a question.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Buttons {
	/// Yes, No and Cancel.
	YesNoCancel,
	/// Yes and No.
	YesNo,
}

/// A question for a message box.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Ask {
	/// The words.
	pub text: Text<ASK_MAX>,
	/// The buttons.
	pub buttons: Buttons,
	/// The button Enter presses.
	pub default: Answer,
}

/// How a notice is shown.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum NoticeKind {
	/// A message box with the information icon.
	Info,
	/// A message box with the exclamation icon.
	Warn,
	/// A line for the status bar, not a box. The editor has already put it there.
	Status,
}

/// A message for the person.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Notice {
	/// The words.
	pub text: Text<NOTICE_MAX>,
	/// How to show it.
	pub kind: NoticeKind,
}

impl Notice {
	fn of(kind: NoticeKind, text: &str) -> Self {
		Notice {
			text: Text::from_parts(&[text.as_bytes()]),
			kind,
		}
	}
}

/// The words of every box and status line the flows produce.
pub mod words {
	/// Find found nothing.
	pub const NOT_FOUND: &str = "Quill cannot find that text.";
	/// The spelling check reached the end.
	pub const SPELLING_COMPLETE: &str = "The spelling check is complete.";
	/// The Tabs position is not a number of inches.
	pub const BAD_POSITION: &str = "Type a position in inches, such as 1.25.";
	/// Open failed.
	pub const CANNOT_OPEN: &str = "Quill cannot open that file.";
	/// Save failed.
	pub const CANNOT_SAVE: &str = "Quill cannot save; the disk may be full.";
	/// The new document is whole in a temporary file because the old one could not be replaced; the file name follows, then a full stop.
	pub const KEPT_NEW_COPY: &str = "Work kept in ";
	/// Insert Picture failed.
	pub const CANNOT_USE_PICTURE: &str = "Quill cannot use that picture.";
	/// Print failed.
	pub const CANNOT_PRINT: &str = "Quill could not print.";
}

/// The fields of the Find and Replace dialog.
#[derive(Copy, Clone, Debug)]
pub struct FindFields<'a> {
	/// The text to find.
	pub find: &'a [u8],
	/// The text to replace it with.
	pub with: &'a [u8],
	/// Match case.
	pub case: bool,
	/// Whole words only.
	pub word: bool,
}

/// What the Save As dialog starts with.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct SaveAsStart<'a> {
	/// The folder it opens in (empty when none is remembered: the shell uses its own default).
	pub folder: &'a [u8],
	/// The file name to fill in: the document's own, without its folder (empty when untitled).
	pub name: &'a [u8],
	/// The type that matches the name's extension: 0 Quill, 1 Rich Text Format, 2 Text, anything else 0.
	pub type_index: usize,
}

/// The buttons of the spelling dialog.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SpellButton {
	/// Ignore this one.
	Ignore,
	/// Ignore every one.
	IgnoreAll,
	/// Change this one.
	Change,
	/// Change every one.
	ChangeAll,
	/// Add the word to the user dictionary.
	Add,
	/// Cancel.
	Cancel,
}

/// What the spelling dialog does next.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SpellStep {
	/// Show the next unknown word (the dialog is open, or opens).
	Next,
	/// Nothing happened: keep the dialog as it is.
	Stay,
	/// The person cancelled; close the dialog with no message.
	Cancelled,
	/// The check is over: close the dialog and show the notice.
	Done(Notice),
}

/// The buttons of the Tabs dialog that change a stop.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TabButton {
	/// Set a stop at the position.
	Set,
	/// Clear the stop at the position.
	Clear,
	/// Clear every stop.
	ClearAll,
}

/// Parses a position in inches with up to two decimals ("1", "1.5", ".25", " 0.25 ") into twips. Anything else, and nothing at
/// all, is `None`.
pub fn parse_inches(t: &[u8]) -> Option<i32> {
	let t = t.trim_ascii();
	let (whole, frac) = match t.iter().position(|b| *b == b'.') {
		Some(i) => (t.get(..i)?, t.get(i + 1..)?),
		None => (t, &[][..]),
	};
	if whole.is_empty() && frac.is_empty() {
		return None;
	}
	let mut w = 0i32;
	for b in whole {
		if !b.is_ascii_digit() {
			return None;
		}
		w = w.checked_mul(10)?.checked_add(i32::from(b - b'0'))?;
	}
	let mut hundredths = 0i32;
	let mut scale = 10;
	for (i, b) in frac.iter().enumerate() {
		if !b.is_ascii_digit() {
			return None;
		}
		if i < 2 {
			hundredths += i32::from(b - b'0') * scale;
			scale /= 10;
		}
	}
	w.checked_mul(1440)?.checked_add(hundredths * 1440 / 100)
}

impl<M: quill_core::layout::Measure + Clone> App<M> {
	/// The question for command `c`, which would lose the unsaved changes: Save, Don't Save or Cancel, Yes the default.
	pub fn confirm_question(&self, c: Command) -> Ask {
		let name = self.name();
		let shown: &[u8] = if name.is_empty() { b"(Untitled)" } else { name };
		let what: &[u8] = match c {
			Command::Exit => b" before Quill closes?",
			Command::New => b" before a new document starts?",
			// Open and a recent file both replace the document with another.
			_ => b" before another document opens?",
		};
		Ask {
			text: Text::from_parts(&[b"Save the changes to ", shown, what]),
			buttons: Buttons::YesNoCancel,
			default: Answer::Yes,
		}
	}

	/// Applies the answer to `confirm_question(c)`.
	///
	/// Yes saves and then goes on with `c`. A document with no name asks for one first: the editor leaves a `Dialog::SaveAs`
	/// request with `c` waiting, and `save_as_chosen` goes on with it (`dialog_cancelled` drops it). A failed save leaves its
	/// message on the status line and drops `c`. No goes on without saving; the changes are dropped only when `c` completes, so cancelling the dialog that follows (Open's) leaves the document modified. Cancel does nothing.
	pub fn confirm_answered<S: Store + ?Sized, F: Files>(
		&mut self,
		s: &mut S,
		f: &mut F,
		c: Command,
		a: Answer,
	) {
		self.msg.clear();
		match a {
			Answer::Yes => {
				self.pending = Some(c);
				if self.name_len == 0 {
					self.dialog = Some(Dialog::SaveAs);
				} else {
					self.do_save(s, f);
				}
			}
			Answer::No => {
				// The changes go only when `c` completes: a cancelled Open leaves them unsaved.
				self.run_confirmed(s, f, c);
			}
			Answer::Cancel => self.pending = None,
		}
		self.sync_layout(s);
		self.scroll(s);
	}

	/// The person closed a file or confirmation dialog without choosing: whatever waited on it is dropped.
	pub fn dialog_cancelled(&mut self, _d: Dialog) {
		self.pending = None;
	}

	/// What the Save As dialog starts with.
	pub fn save_as_start(&self) -> SaveAsStart<'_> {
		let own = self.name();
		let base = own
			.iter()
			.rposition(|b| *b == b'\\' || *b == b':')
			.map_or(own, |i| own.get(i + 1..).unwrap_or(&[]));
		let type_index = base
			.iter()
			.rposition(|b| *b == b'.')
			.and_then(|d| base.get(d + 1..))
			.and_then(|e| {
				[&b"qld"[..], b"rtf", b"txt"]
					.iter()
					.position(|k| e.eq_ignore_ascii_case(k))
			})
			.unwrap_or(0);
		SaveAsStart {
			folder: self.last_folder(),
			name: base,
			type_index,
		}
	}

	/// The Save As dialog returned `path`: remembers its folder, saves under the name (the format follows the extension), and
	/// goes on with whatever waited on the save. A failed save keeps the old name and says so on the status line.
	pub fn save_as_chosen<S: Store + ?Sized, F: Files>(
		&mut self,
		s: &mut S,
		f: &mut F,
		path: &[u8],
	) -> Result<(), Notice> {
		self.msg.clear();
		self.remember_folder(path);
		if self.save_as(s, f, path) {
			Ok(())
		} else {
			// The status line holds the words that say what happened to the file.
			Err(Notice {
				text: Text::from_parts(&[self.msg.bytes()]),
				kind: NoticeKind::Status,
			})
		}
	}

	/// The Open dialog returned `path`: remembers its folder and opens it. A file that cannot be read leaves the document as it was.
	pub fn open_chosen<S: Store + ?Sized, F: Files>(
		&mut self,
		s: &mut S,
		f: &mut F,
		path: &[u8],
	) -> Result<(), Notice> {
		self.msg.clear();
		self.remember_folder(path);
		self.pending = None;
		if self.open(s, f, path).is_ok() {
			self.sync_layout(s);
			self.scroll(s);
			Ok(())
		} else {
			self.msg.set(words::CANNOT_OPEN);
			Err(Notice::of(NoticeKind::Status, words::CANNOT_OPEN))
		}
	}

	/// The Insert Picture dialog returned `path`: remembers its folder and inserts the picture.
	pub fn picture_chosen<S: Store + ?Sized, F: Files>(
		&mut self,
		s: &mut S,
		f: &mut F,
		path: &[u8],
	) -> Result<(), Notice> {
		self.msg.clear();
		self.remember_folder(path);
		if self.insert_picture(s, f, path).is_ok() {
			self.sync_layout(s);
			self.scroll(s);
			Ok(())
		} else {
			self.msg.set(words::CANNOT_USE_PICTURE);
			Err(Notice::of(NoticeKind::Status, words::CANNOT_USE_PICTURE))
		}
	}

	/// The Print dialog chose `dest` (a file or a device): prints the text to it. Copies and a page range come with the dialog
	/// that asks for them.
	pub fn print_chosen<S: Store + ?Sized, F: Files>(
		&mut self,
		s: &mut S,
		f: &mut F,
		dest: &[u8],
	) -> Result<(), Notice> {
		self.msg.clear();
		if self.print_text(s, f, dest) {
			Ok(())
		} else {
			self.msg.set(words::CANNOT_PRINT);
			Err(Notice::of(NoticeKind::Status, words::CANNOT_PRINT))
		}
	}

	/// A button of the Find or Replace dialog. Find Next that finds nothing gives the not-found box; Replace All gives the count
	/// ("Quill made 3 replacements."). The dialog stays open either way.
	pub fn find_pressed<S: Store + ?Sized>(
		&mut self,
		s: &mut S,
		fields: FindFields<'_>,
		action: FindAction,
	) -> Option<Notice> {
		self.msg.clear();
		if fields.find.is_empty() {
			return None;
		}
		let found = self.find_text(
			s,
			fields.find,
			fields.with,
			fields.case,
			fields.word,
			action,
		);
		let notice = if action == FindAction::ReplaceAll {
			Some(self.take_notice(NoticeKind::Info))
		} else if !found {
			Some(Notice::of(NoticeKind::Info, words::NOT_FOUND))
		} else {
			None
		};
		// The box says it; the status line does not say it again.
		self.msg.clear();
		notice
	}

	/// The F7 command ran: starts the check from the caret. `Next` means the dialog opens on the first unknown word; `Done` is
	/// a box (nothing to fix, or no dictionary).
	pub fn spell_started<S: Store + ?Sized, F: Files>(
		&mut self,
		s: &mut S,
		f: &mut F,
	) -> SpellStep {
		self.msg.clear();
		if self.spell_begin(s, f) {
			SpellStep::Next
		} else {
			SpellStep::Done(self.take_notice(NoticeKind::Info))
		}
	}

	/// A button of the spelling dialog, with the text of its Change to field. Change with an empty field does nothing.
	pub fn spell_pressed<S: Store + ?Sized, F: Files>(
		&mut self,
		s: &mut S,
		f: &mut F,
		button: SpellButton,
		with: &[u8],
	) -> SpellStep {
		self.msg.clear();
		let more = match button {
			SpellButton::Ignore => self.spell_ignore(s, false),
			SpellButton::IgnoreAll => self.spell_ignore(s, true),
			SpellButton::Change | SpellButton::ChangeAll if with.is_empty() => {
				return SpellStep::Stay;
			}
			SpellButton::Change => self.spell_change(s, with, false),
			SpellButton::ChangeAll => self.spell_change(s, with, true),
			SpellButton::Add => self.spell_add(s, f),
			SpellButton::Cancel => {
				self.spell_cancel();
				return SpellStep::Cancelled;
			}
		};
		if more {
			SpellStep::Next
		} else {
			SpellStep::Done(self.take_notice(NoticeKind::Info))
		}
	}

	/// A button of the Tabs dialog, with the position text, the kind (0 left, 1 centre, 2 right, 3 decimal) and the leader (0 none,
	/// 1 dots, 2 hyphens, 3 underline) it shows. A position that is not inches gives a box; a seventeenth stop leaves a line on
	/// the status bar.
	pub fn tab_pressed<S: Store + ?Sized>(
		&mut self,
		s: &mut S,
		text: &[u8],
		kind: u8,
		leader: u8,
		button: TabButton,
	) -> Option<Notice> {
		self.msg.clear();
		if button == TabButton::ClearAll {
			self.clear_tabs(s);
			return None;
		}
		let Some(pos) = parse_inches(text) else {
			return Some(Notice::of(NoticeKind::Warn, words::BAD_POSITION));
		};
		if button == TabButton::Set {
			self.set_tab(s, pos, kind, leader);
		} else {
			self.remove_tab(s, pos);
		}
		if self.msg.is_empty() {
			None
		} else {
			Some(self.take_notice(NoticeKind::Status))
		}
	}

	/// The status-line message as a notice. A box takes the message off the status line; a status notice leaves it.
	fn take_notice(&mut self, kind: NoticeKind) -> Notice {
		let mut text = Text::new();
		text.push(self.msg.bytes());
		if text.len == 0 {
			text.push(words::SPELLING_COMPLETE.as_bytes());
		}
		if kind != NoticeKind::Status {
			self.msg.clear();
		}
		Notice { text, kind }
	}
}
