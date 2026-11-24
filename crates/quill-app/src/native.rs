// What a build with native dialogs asks of its shell: the editor says which dialog it wants and applies what the shell collected.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The Windows builds never draw a dialog in text cells (D86). With `set_native_dialogs(true)` a command that the text build answers with
// a text-cell box (a prompt, a form, the Find box, About, a confirmation) leaves the mode alone and leaves a `Dialog` request instead,
// which the shell takes with `take_dialog`, shows as a real dialog or a common dialog, and answers through the methods here. The
// state and the commands behind each dialog are the same ones the text build drives.
use quill_core::doc::{cflag, CFmt, Font};
use quill_core::layout::Measure;
use quill_core::store::Store;

use crate::find::{FindState, FIELD};
use crate::{App, Command, Files};

/// The version Quill reports in its About box.
pub const VERSION: &str = "0.1 (experimental)";

/// A dialog the shell is asked to show.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Dialog {
	/// Help > About Quill.
	About,
	/// File > Open: a file name from the common dialog, then `open`.
	Open,
	/// File > Save As, or Save of a document with no name: then `save_as`.
	SaveAs,
	/// Insert > Picture: a file name, then `insert_picture`.
	Picture,
	/// File > Print.
	Print,
	/// Character > Fonts.
	Fonts,
	/// Paragraph > Tabs.
	Tabs,
	/// Find (false) or Replace (true).
	Find(bool),
	/// Find > Spelling: the shell calls `spell_begin` and shows each word.
	Spelling,
	/// The document has unsaved changes and the command would lose them: Save, Don't Save or Cancel, then the command again.
	ConfirmSave(Command),
}

/// What the Find dialog's buttons do.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FindAction {
	/// Find Next.
	Next,
	/// Replace this one and find the next.
	Replace,
	/// Replace every one.
	ReplaceAll,
}

impl<M: Measure + Clone> App<M> {
	/// Turns native dialogs on or off. On, no command ever puts the editor in a text-cell dialog or menu mode.
	pub fn set_native_dialogs(&mut self, on: bool) {
		self.native = on;
	}

	/// True when dialogs are the shell's.
	pub fn native_dialogs(&self) -> bool {
		self.native
	}

	/// The dialog the last command asked for, if any, taken so it is asked once.
	pub fn take_dialog(&mut self) -> Option<Dialog> {
		self.dialog.take()
	}

	/// The request for command `c`, when native dialogs are on and `c` would open a text-cell box.
	pub(crate) fn native_request(&self, c: Command) -> Option<Dialog> {
		if !self.native {
			return None;
		}
		Some(match c {
			Command::New | Command::Open | Command::Exit | Command::Recent(_)
				if self.modified && self.pending != Some(c) =>
			{
				Dialog::ConfirmSave(c)
			}
			Command::Open => Dialog::Open,
			Command::Save if self.name_len == 0 => Dialog::SaveAs,
			Command::SaveAs => Dialog::SaveAs,
			Command::Fonts => Dialog::Fonts,
			Command::Tabs => Dialog::Tabs,
			Command::About => Dialog::About,
			Command::Picture => Dialog::Picture,
			Command::Print => Dialog::Print,
			Command::Find => Dialog::Find(false),
			// Find Next before any search has no text to look for: it asks for the Find dialog, as Find does.
			Command::FindNext if self.last_find.is_none() => Dialog::Find(false),
			Command::Replace => Dialog::Find(true),
			Command::Spelling => Dialog::Spelling,
			_ => return None,
		})
	}

	/// Says the document has no unsaved changes (the user chose not to save them). When `run_confirmed` follows, the changes
	/// count as unsaved until that command completes, so cancelling its dialog loses nothing.
	pub fn discard_changes(&mut self) {
		self.discarded = self.modified;
		self.modified = false;
	}

	/// Runs `c` as though the confirmation had been answered, after the shell has saved or discarded. A discard takes effect
	/// only when `c` completes (a new document, a file opened, Exit): until then the document stays modified.
	pub fn run_confirmed<S: Store + ?Sized, F: Files>(&mut self, s: &mut S, f: &mut F, c: Command) {
		if core::mem::take(&mut self.discarded) {
			self.modified = true;
		}
		self.pending = Some(c);
		self.command(s, f, c);
		self.pending = None;
	}

	/// Sets the face, size and the bold, italic, underline and strikeout bits of the selection (or of what is typed next) together,
	/// as the Fonts dialog's OK does.
	pub fn set_font<S: Store + ?Sized>(
		&mut self,
		s: &mut S,
		face: &[u8],
		family: u8,
		half_points: u16,
		flags: u16,
	) {
		let size = half_points.clamp(8, 288);
		let mask = cflag::BOLD | cflag::ITALIC | cflag::UNDERLINE | cflag::STRIKE;
		if let Ok(fi) = self.doc.fonts.intern(s, Font::new(face, family)) {
			self.char_command(
				s,
				move |c| CFmt {
					font: fi,
					size,
					flags: (c.flags & !mask) | (flags & mask),
					..c
				},
				None,
			);
		}
	}

	/// Adds or changes a tab stop with a leader (0 none, 1 dots, 2 hyphens, 3 underline).
	pub fn set_tab<S: Store + ?Sized>(&mut self, s: &mut S, pos: i32, kind: u8, leader: u8) {
		self.tab_edit(s, "< Set >", Some(pos), kind, leader);
	}

	/// Removes every tab stop of the selected paragraphs.
	pub fn clear_tabs<S: Store + ?Sized>(&mut self, s: &mut S) {
		self.tab_edit(s, "< Clear All >", None, 0, 0);
	}

	/// The text, replacement and options the Find dialog last used.
	pub fn last_find(&self) -> (&[u8], &[u8], bool, bool) {
		match &self.last_find {
			Some(st) => (
				st.find.get(..st.flen).unwrap_or(&[]),
				st.with.get(..st.wlen).unwrap_or(&[]),
				st.case,
				st.word,
			),
			None => (&[], &[], false, false),
		}
	}

	/// Does what a Find dialog button does. Returns false when the text was not found (or `find` is empty).
	pub fn find_text<S: Store + ?Sized>(
		&mut self,
		s: &mut S,
		find: &[u8],
		with: &[u8],
		case: bool,
		word: bool,
		action: FindAction,
	) -> bool {
		if find.is_empty() {
			return false;
		}
		let mut st = FindState::new(action != FindAction::Next, find);
		let n = with.len().min(FIELD);
		for (d, b) in st.with.iter_mut().zip(with.iter()) {
			*d = *b;
		}
		st.wlen = n;
		st.case = case;
		st.word = word;
		self.end_typing();
		let found = match action {
			FindAction::Next => self.find_next(s, &st),
			FindAction::Replace => {
				if self.selection_matches(s, &st) {
					self.replace_here(s, &st);
				}
				self.find_next(s, &st)
			}
			FindAction::ReplaceAll => {
				self.replace_all(s, &st);
				true
			}
		};
		self.last_find = Some(st);
		self.scroll(s);
		found
	}

	/// Where the last file dialog was, remembered in `QUILL.INI`.
	pub fn last_folder(&self) -> &[u8] {
		self.folder.get(..self.folder_len).unwrap_or(&[])
	}

	/// Remembers the folder of `path` (everything before the last backslash or colon) for the next file dialog.
	pub fn remember_folder(&mut self, path: &[u8]) {
		let cut = path
			.iter()
			.rposition(|b| *b == b'\\' || *b == b':')
			.map_or(0, |i| i + 1);
		let dir = path.get(..cut).unwrap_or(&[]);
		let n = dir.len().min(self.folder.len());
		for (d, b) in self.folder.iter_mut().zip(dir.iter()) {
			*d = *b;
		}
		self.folder_len = n;
	}
}
