// The file dialog of the text screens: Open, Save As and Insert Picture as `Mode::File`, in place of the one-line name prompt.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The logic is `quill_dlg::filedlg::FileDlg`, the geometry is `quill_dlg::filelayout`, and the painter is `filepaint`. This module
// owns what is the editor's: when the dialog opens, how a key reaches it, the message boxes over it (replace this file, a drive
// that is not ready, a name that is not valid) and how an accepted name goes back through the same entry points the Windows
// dialogs use (`open_chosen`, `save_as_chosen`, `picture_chosen`). The dialog opens only when the shell asks for it
// (`App::set_file_dialog`) and its `Files` has a file system to list (`Files::fs`); otherwise the name prompt stays.

use quill_core::layout::Measure;
use quill_core::store::Store;
use quill_dlg::filedlg::{DlgError, FileDlg, Outcome, PlaceSpec, Question, Start};
use quill_dlg::filelayout::{Layout, Screen};
use quill_dlg::key::DlgKey;
use quill_dlg::name83::NameError;
use quill_dlg::types::Kind;

use crate::{App, Files, Key, Mode, Prompt, NAME_MAX};

/// How many bytes of a box's line.
pub(crate) const LINE_MAX: usize = 46;

/// One line of a message box.
#[derive(Copy, Clone, PartialEq, Eq)]
pub(crate) struct Line {
	buf: [u8; LINE_MAX],
	len: usize,
}

impl Line {
	pub(crate) const fn new() -> Line {
		Line {
			buf: [0; LINE_MAX],
			len: 0,
		}
	}

	fn of(parts: &[&[u8]]) -> Line {
		let mut l = Line::new();
		for p in parts {
			l.push(p);
		}
		l
	}

	fn push(&mut self, b: &[u8]) {
		for &c in b {
			if let Some(slot) = self.buf.get_mut(self.len) {
				*slot = c;
				self.len += 1;
			}
		}
	}

	pub(crate) fn bytes(&self) -> &[u8] {
		self.buf.get(..self.len).unwrap_or(&[])
	}
}

/// A message box over the dialog.
#[derive(Copy, Clone, PartialEq, Eq)]
pub(crate) enum FileBox {
	None,
	/// A question with Yes and No: the focused button (0 Yes, 1 No) and two lines
	Ask(u8, Line, Line),
	/// A notice with OK, or with Retry and Cancel when a drive can be tried again; the focused button
	Say(Option<u8>, u8, Line, Line),
}

/// A folder the shell names among the places.
#[derive(Copy, Clone)]
struct Named {
	label: [u8; 12],
	label_len: u8,
	path: [u8; NAME_MAX],
	path_len: u8,
}

impl Named {
	const NONE: Named = Named {
		label: [0; 12],
		label_len: 0,
		path: [0; NAME_MAX],
		path_len: 0,
	};

	fn label(&self) -> &[u8] {
		self.label.get(..usize::from(self.label_len)).unwrap_or(&[])
	}

	fn path(&self) -> &[u8] {
		self.path.get(..usize::from(self.path_len)).unwrap_or(&[])
	}
}

/// Everything the file dialog adds to an app.
#[derive(Clone)]
pub(crate) struct State {
	/// The shell uses this dialog in place of the name prompt.
	pub on: bool,
	pub dlg: Option<FileDlg>,
	pub kind: Kind,
	pub boxed: FileBox,
	shell_places: [Named; 2],
	doc: Named,
	/// The date pattern for the list and the preview (`MM/dd/yyyy`), and whether the clock shows 24 hours
	pub date: [u8; 10],
	pub date_len: u8,
	pub h24: bool,
}

impl State {
	pub(crate) const fn new() -> State {
		State {
			on: false,
			dlg: None,
			kind: Kind::Open,
			boxed: FileBox::None,
			shell_places: [Named::NONE; 2],
			doc: Named::NONE,
			date: *b"MM/dd/yyyy",
			date_len: 10,
			h24: false,
		}
	}
}

fn named(label: &[u8], path: &[u8]) -> Named {
	let mut n = Named::NONE;
	let ll = label.len().min(12);
	let pl = path.len().min(NAME_MAX).min(usize::from(u8::MAX));
	if let (Some(d), Some(s)) = (n.label.get_mut(..ll), label.get(..ll)) {
		d.copy_from_slice(s);
	}
	if let (Some(d), Some(s)) = (n.path.get_mut(..pl), path.get(..pl)) {
		d.copy_from_slice(s);
	}
	n.label_len = ll as u8;
	n.path_len = pl as u8;
	n
}

/// The key a dialog sees for an editor key; `None` for one it has no use for.
fn dlg_key(k: Key) -> Option<DlgKey> {
	use crate::Dir;
	Some(match k {
		Key::Char(c) => DlgKey::Char(c),
		Key::Alt(c) => DlgKey::Alt(c),
		Key::Enter => DlgKey::Enter,
		Key::Esc => DlgKey::Esc,
		Key::Tab => DlgKey::Tab,
		Key::BackTab => DlgKey::BackTab,
		Key::Backspace => DlgKey::Backspace,
		Key::Delete => DlgKey::Delete,
		Key::Insert => DlgKey::Insert,
		Key::Left => DlgKey::Left,
		Key::Right => DlgKey::Right,
		Key::Up => DlgKey::Up,
		Key::Down => DlgKey::Down,
		Key::Home => DlgKey::Home,
		Key::End => DlgKey::End,
		Key::PageUp => DlgKey::PgUp,
		Key::PageDown => DlgKey::PgDn,
		Key::F(4) => DlgKey::F4,
		Key::F(5) => DlgKey::F5,
		Key::Select(Dir::Left) => DlgKey::ShiftLeft,
		Key::Select(Dir::Right) => DlgKey::ShiftRight,
		Key::Select(Dir::Home) => DlgKey::ShiftHome,
		Key::Select(Dir::End) => DlgKey::ShiftEnd,
		_ => return None,
	})
}

/// The words of the boxes.
pub mod words {
	pub const TOO_LONG: &[u8] = b"That path is too long for Quill.";
	pub const NOT_VALID: &[u8] = b"That is not a valid DOS file name.";
	pub const DEVICE: &[u8] = b" is a name DOS keeps for a device.";
	pub const NOT_FOUND: &[u8] = b" was not found. Check the name.";
	pub const NO_PATH: &[u8] = b"That folder does not exist.";
	pub const READ_ONLY: &[u8] = b" is read-only. Save it under another name.";
	pub const ACCESS: &[u8] = b"Quill cannot use that; access was refused.";
	pub const FULL: &[u8] = b"The disk is full.";
	pub const UNSUPPORTED: &[u8] = b"That is not available here.";
	pub const MEMORY: &[u8] = b"There is not enough memory to complete this.";
	pub const EXISTS: &[u8] = b" already exists.";
	pub const REPLACE: &[u8] = b"Do you want to replace it?";
	pub const TEXT_WARNING: &[u8] = b"Saving in text format will remove all";
	pub const TEXT_WARNING_2: &[u8] = b"formatting and pictures. Save anyway?";
}

fn file_name(path: &[u8]) -> &[u8] {
	quill_dlg::name83::last_part(path)
}

impl<M: Measure + Clone> App<M> {
	/// Says whether Open, Save As and Insert Picture use the file dialog (when the `Files` has a file system) instead of the name
	/// prompt. The host's text screen and the terminal client turn it on; a shell turns it on once its `Files::fs` works.
	pub fn set_file_dialog(&mut self, on: bool) {
		self.fd.on = on;
	}

	/// The folders the file dialog shows among its places after the drives: `label` at most 12 bytes. The shell gives the program's
	/// own folder and the samples folder when there is one; at most two are kept.
	pub fn set_file_places(&mut self, places: &[(&[u8], &[u8])]) {
		self.fd.shell_places = [Named::NONE; 2];
		for (slot, (label, path)) in self.fd.shell_places.iter_mut().zip(places) {
			*slot = named(label, path);
		}
	}

	/// How the file dialog writes dates (a pattern for `datefmt::format`, such as `MM/dd/yyyy`) and whether its clock shows 24 hours.
	pub fn set_file_clock(&mut self, date_pattern: &[u8], h24: bool) {
		let n = date_pattern.len().min(self.fd.date.len());
		self.fd.date = [0; 10];
		if let (Some(d), Some(s)) = (self.fd.date.get_mut(..n), date_pattern.get(..n)) {
			d.copy_from_slice(s);
		}
		self.fd.date_len = n as u8;
		self.fd.h24 = h24;
	}

	/// The `[Quill] DocFolder` setting: where the person keeps documents, shown among the places when it is set.
	pub(crate) fn set_doc_folder(&mut self, path: &[u8]) {
		self.fd.doc = if path.is_empty() {
			Named::NONE
		} else {
			named(b"Documents", path)
		};
	}

	/// True while the file dialog is open.
	pub fn file_dialog_open(&self) -> bool {
		matches!(self.mode, Mode::File)
	}

	/// A tick of the shell's poll (about 55 ms): a type-ahead prefix in the file dialog ages.
	pub fn file_tick(&mut self) {
		if let Some(d) = &mut self.fd.dlg {
			d.tick();
		}
	}

	/// Opens the file dialog for `kind` of prompt when the shell wants it and can list files. False leaves everything as it was.
	pub(crate) fn try_file_dialog<S: Store + ?Sized, F: Files>(
		&mut self,
		s: &mut S,
		f: &mut F,
		kind: Prompt,
	) -> bool {
		let kind = match kind {
			Prompt::Open => Kind::Open,
			Prompt::SaveAs => Kind::SaveAs,
			Prompt::Picture => Kind::Picture,
			Prompt::Print => return false,
		};
		if !self.fd.on {
			return false;
		}
		// What the Save As dialog starts with, and the places, are the editor's to say.
		let start_as = self.save_as_start();
		let (name, type_index) = if kind == Kind::SaveAs {
			(start_as.name, start_as.type_index)
		} else {
			(&[][..], 0)
		};
		let mut folders: [PlaceSpec<'_>; 4] = [PlaceSpec {
			label: b"",
			path: b"",
		}; 4];
		let mut n = 0;
		// The folder is remembered with its closing backslash; the dialog is given it without, as a path it lists.
		let last = self.last_folder();
		let last = match last.split_last() {
			Some((b'\\', rest)) if last.len() > 3 => rest,
			_ => last,
		};
		fn put<'a>(folders: &mut [PlaceSpec<'a>; 4], spec: PlaceSpec<'a>, n: &mut usize) {
			if let Some(slot) = folders.get_mut(*n) {
				*slot = spec;
				*n += 1;
			}
		}
		let doc = self.fd.doc;
		if doc.label_len > 0 {
			put(
				&mut folders,
				PlaceSpec {
					label: doc.label(),
					path: doc.path(),
				},
				&mut n,
			);
		}
		for p in &self.fd.shell_places {
			if p.label_len > 0 {
				put(
					&mut folders,
					PlaceSpec {
						label: p.label(),
						path: p.path(),
					},
					&mut n,
				);
			}
		}
		if !last.is_empty() {
			put(
				&mut folders,
				PlaceSpec {
					label: b"Last used",
					path: last,
				},
				&mut n,
			);
		}
		let mut recent: [&[u8]; 4] = [&[]; 4];
		let mut nr = 0;
		if kind != Kind::Picture {
			for (slot, r) in recent.iter_mut().zip(self.recent.iter()) {
				*slot = r;
				nr += 1;
			}
		}
		let rows = Layout::new(Screen::Text(self.rows as u16 + 2), kind)
			.map_or(1, |l| l.list_rows)
			.min(255) as u8;
		let mut start = Start::new(last, name, rows);
		start.type_index = type_index;
		start.folders = folders.get(..n).unwrap_or(&[]);
		start.recent = recent.get(..nr).unwrap_or(&[]);
		let made = match f.fs() {
			Some(fs) => FileDlg::new(kind, start, fs, s),
			None => return false,
		};
		match made {
			Ok(d) => {
				self.fd.dlg = Some(d);
				self.fd.kind = kind;
				self.fd.boxed = FileBox::None;
				self.mode = Mode::File;
				true
			}
			// A start folder that cannot be read leaves the name prompt, which at least takes a path.
			Err(_) => false,
		}
	}

	/// A key while the file dialog is open.
	#[inline(never)] // keeps the dialog's locals out of `key`'s frame
	pub(crate) fn key_file<S: Store + ?Sized, F: Files>(&mut self, s: &mut S, f: &mut F, k: Key) {
		let Some(mut dlg) = self.fd.dlg.take() else {
			self.mode = Mode::Edit;
			return;
		};
		let out = match self.fd.boxed {
			FileBox::None => match dlg_key(k) {
				Some(dk) => match f.fs() {
					Some(fs) => dlg.key(dk, fs, s),
					None => Outcome::Cancel,
				},
				None => Outcome::None,
			},
			FileBox::Ask(focus, l1, l2) => match self.ask_key(k, focus, l1, l2) {
				Some(yes) => {
					self.fd.boxed = FileBox::None;
					dlg.answer(yes, s)
				}
				None => Outcome::None,
			},
			FileBox::Say(retry, focus, l1, l2) => match self.say_key(k, retry, focus, l1, l2) {
				Some(true) => {
					self.fd.boxed = FileBox::None;
					match (retry, f.fs()) {
						(Some(letter), Some(fs)) => dlg.go_drive(letter, fs, s),
						_ => Outcome::None,
					}
				}
				Some(false) => {
					self.fd.boxed = FileBox::None;
					Outcome::None
				}
				None => Outcome::None,
			},
		};
		self.file_outcome(s, f, dlg, out);
	}

	/// Keys of a Yes or No box: `Some(true)` for Yes, `Some(false)` for No, `None` to stay.
	fn ask_key(&mut self, k: Key, focus: u8, l1: Line, l2: Line) -> Option<bool> {
		match k {
			Key::Char(b'y' | b'Y') => Some(true),
			Key::Char(b'n' | b'N') | Key::Esc => Some(false),
			Key::Enter => Some(focus == 0),
			Key::Tab | Key::BackTab | Key::Left | Key::Right => {
				self.fd.boxed = FileBox::Ask(1 - focus.min(1), l1, l2);
				None
			}
			_ => None,
		}
	}

	/// Keys of a notice: `Some(true)` to try the drive again, `Some(false)` to close it, `None` to stay.
	fn say_key(
		&mut self,
		k: Key,
		retry: Option<u8>,
		focus: u8,
		l1: Line,
		l2: Line,
	) -> Option<bool> {
		match (k, retry) {
			(Key::Esc, _) => Some(false),
			(Key::Char(b'r' | b'R'), Some(_)) => Some(true),
			(Key::Char(b'c' | b'C'), Some(_)) => Some(false),
			(Key::Enter | Key::Char(b' '), Some(_)) => Some(focus == 0),
			(Key::Enter | Key::Char(b' '), None) => Some(false),
			(Key::Tab | Key::BackTab | Key::Left | Key::Right, Some(_)) => {
				self.fd.boxed = FileBox::Say(retry, 1 - focus.min(1), l1, l2);
				None
			}
			_ => None,
		}
	}

	/// Acts on what the dialog said: keeps it, asks, shows an error, or ends and hands the name to the flow.
	fn file_outcome<S: Store + ?Sized, F: Files>(
		&mut self,
		s: &mut S,
		f: &mut F,
		dlg: FileDlg,
		out: Outcome,
	) {
		match out {
			Outcome::None => {}
			Outcome::Ask(q) => {
				let path = dlg.chosen();
				let (l1, l2) = match q {
					Question::Overwrite => {
						let mut shown = [0u8; LINE_MAX];
						let n = crate::recent::shorten_path(
							path,
							LINE_MAX - words::EXISTS.len(),
							&mut shown,
						);
						(
							Line::of(&[shown.get(..n).unwrap_or(&[]), words::EXISTS]),
							Line::of(&[words::REPLACE]),
						)
					}
					Question::TextWarning => (
						Line::of(&[words::TEXT_WARNING]),
						Line::of(&[words::TEXT_WARNING_2]),
					),
				};
				// Replacing a file defaults to No; the text warning to Yes.
				let focus = if q == Question::Overwrite { 1 } else { 0 };
				self.fd.boxed = FileBox::Ask(focus, l1, l2);
			}
			Outcome::Error(e) => {
				let (l1, l2, retry) = error_words(&dlg, e);
				self.fd.boxed = FileBox::Say(retry, 0, l1, l2);
			}
			Outcome::Cancel => {
				self.mode = Mode::Edit;
				self.pending = None;
				self.fd.boxed = FileBox::None;
				return;
			}
			Outcome::Accept => {
				let mut path = [0u8; NAME_MAX];
				let chosen = dlg.chosen();
				let n = chosen.len().min(NAME_MAX);
				if let (Some(d), Some(sv)) = (path.get_mut(..n), chosen.get(..n)) {
					d.copy_from_slice(sv);
				}
				let format = dlg.chosen_format();
				let kind = self.fd.kind;
				self.mode = Mode::Edit;
				self.fd.boxed = FileBox::None;
				let path = path.get(..n).unwrap_or(&[]);
				self.file_chosen(s, f, kind, path, format.map(|f| f.index() as u8));
				return;
			}
		}
		if !dlg.is_done() {
			self.fd.dlg = Some(dlg);
		}
	}

	/// The dialog returned `path`: the same entry points the Windows dialogs answer through.
	fn file_chosen<S: Store + ?Sized, F: Files>(
		&mut self,
		s: &mut S,
		f: &mut F,
		kind: Kind,
		path: &[u8],
		format: Option<u8>,
	) {
		let result = match kind {
			Kind::Open => self.open_chosen(s, f, path),
			Kind::Picture => self.picture_chosen(s, f, path),
			Kind::SaveAs => {
				// The format the dialog worked out (a known extension, else the type) is the one the save writes.
				let old = self.fmt;
				if let Some(fm) = format {
					self.fmt = fm;
				}
				let r = self.save_as_chosen(s, f, path);
				if r.is_err() {
					self.fmt = old;
				}
				r
			}
		};
		// A failure has already put its words on the status line.
		let _ = result;
	}
}

/// The letter of the drive the dialog is on, for the words about its disk.
fn drive_of(dlg: &FileDlg) -> [u8; 1] {
	[dlg.folder().first().map_or(b'A', u8::to_ascii_uppercase)]
}

/// The words of a refusal: two lines, and a drive that can be tried again.
fn error_words(dlg: &FileDlg, e: DlgError) -> (Line, Line, Option<u8>) {
	let name = file_name(dlg.chosen());
	let one = |t: &[u8]| (Line::of(&[t]), Line::new(), None);
	match e {
		DlgError::BadName(NameError::PathTooLong) => one(words::TOO_LONG),
		DlgError::BadName(NameError::Reserved) => {
			(Line::of(&[name, words::DEVICE]), Line::new(), None)
		}
		DlgError::BadName(_) => one(words::NOT_VALID),
		DlgError::NotFound => (Line::of(&[name, words::NOT_FOUND]), Line::new(), None),
		DlgError::NoPath => one(words::NO_PATH),
		DlgError::ReadOnly => (Line::of(&[name, words::READ_ONLY]), Line::new(), None),
		DlgError::NotReady(d) => (
			Line::of(&[b"Drive ", &[d], b": is not ready."]),
			Line::of(&[b"Insert a disk and choose Retry."]),
			Some(d),
		),
		DlgError::Fs(quill_dlg::fs::FsError::Full) => one(words::FULL),
		DlgError::Fs(quill_dlg::fs::FsError::WriteProtected) => (
			Line::of(&[
				b"The disk in drive ",
				&drive_of(dlg),
				b": is write-protected.",
			]),
			Line::new(),
			None,
		),
		DlgError::Fs(quill_dlg::fs::FsError::Unreadable) => (
			Line::of(&[b"The disk in drive ", &drive_of(dlg), b": cannot be read."]),
			Line::of(&[b"It may not be formatted."]),
			None,
		),
		DlgError::Fs(quill_dlg::fs::FsError::Unsupported) => one(words::UNSUPPORTED),
		DlgError::Fs(_) => one(words::ACCESS),
		DlgError::NoMemory => one(words::MEMORY),
	}
}
