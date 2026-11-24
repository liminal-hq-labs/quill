// The file dialog: Open, Save As and Insert Picture as a state machine over keys, a file system and a store.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The dialog draws nothing. A shell feeds it `DlgKey`s and acts on the `Outcome`: nothing, a question to ask (Yes or No, answered
// with `answer`), an error to show, the chosen path, or a cancel. A painter reads the state through the accessors.
//
// The file and folder lists live in `Store` chunks of 16-byte records and are freed as soon as the dialog ends, by `Accept`,
// `Cancel` or `close`. Files are capped at 512 and folders at 256 (drives and `[..]` included); past the cap the list ends and a
// flag says so. Entries are sorted in place with a heapsort, so the order is the same whatever order the file system returns them.
// The dialog never changes the current directory: it keeps its own folder and returns full paths.

use crate::focus::{Control, Focus, Nav};
use crate::fs::{
	FileSystem, FsError, ATTR_DIR, ATTR_HIDDEN, ATTR_READONLY, ATTR_SYSTEM, ATTR_VOLUME,
};
use crate::key::DlgKey;
use crate::name83::{is_wild, last_part, matches_any, resolve, NameError, PathBuf, PATH_MAX};
use crate::savename::{swap_extension, with_default_ext, Rule};
use crate::types::{types, write_label, FileType, Format, Kind};
use crate::widgets::{Combo, ComboEvent, Field, FieldKind, List};
use quill_core::store::{ChunkId, ReadStore, Store};

/// The most files listed.
pub const FILES_MAX: usize = 512;
/// The most entries in the Directories list: `[..]`, folders and drives.
pub const DIRS_MAX: usize = 256;
/// The longest pattern typed into the name field, such as `*.TXT`.
pub const PATTERN_MAX: usize = 24;

const REC: usize = 16;
const FILES_BYTES: u16 = (FILES_MAX * REC) as u16;
const DIRS_BYTES: u16 = (DIRS_MAX * REC) as u16;

/// The controls, in Tab order.
pub const NAME: u8 = 0;
pub const FILES: u8 = 1;
pub const DIRS: u8 = 2;
pub const TYPES: u8 = 3;
pub const DRIVES: u8 = 4;
pub const OK: u8 = 5;
pub const CANCEL: u8 = 6;

/// What an entry of a list is.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum EntryKind {
	File,
	Folder,
	/// A drive, shown `[-C-]`; the name is `-C-`
	Drive,
	/// The parent folder, shown `[..]`
	Up,
}

impl EntryKind {
	fn code(self) -> u8 {
		match self {
			EntryKind::File => 0,
			EntryKind::Folder => 1,
			EntryKind::Drive => 2,
			EntryKind::Up => 3,
		}
	}

	fn from_code(c: u8) -> EntryKind {
		match c {
			1 => EntryKind::Folder,
			2 => EntryKind::Drive,
			3 => EntryKind::Up,
			_ => EntryKind::File,
		}
	}

	/// Where the kind sorts: `[..]` first, then folders, then drives.
	fn rank(self) -> u8 {
		match self {
			EntryKind::File | EntryKind::Up => 0,
			EntryKind::Folder => 1,
			EntryKind::Drive => 2,
		}
	}
}

/// One entry of the Files or Directories list.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Entry {
	bytes: [u8; REC],
}

impl Entry {
	fn new(name: &[u8], attr: u8, kind: EntryKind) -> Option<Entry> {
		if name.is_empty() || name.len() > 12 {
			return None;
		}
		let mut bytes = [0u8; REC];
		for (dst, src) in bytes.iter_mut().zip(name) {
			*dst = src.to_ascii_uppercase();
		}
		*bytes.get_mut(12)? = attr;
		*bytes.get_mut(13)? = kind.code();
		Some(Entry { bytes })
	}

	/// The name, upper case: `LETTER.QLD`, a folder's name, `-C-` for a drive, `..` for the parent.
	pub fn name(&self) -> &[u8] {
		let n = self.bytes.get(..12).unwrap_or(&[]);
		let len = n.iter().position(|b| *b == 0).unwrap_or(12);
		n.get(..len).unwrap_or(&[])
	}

	pub fn attr(&self) -> u8 {
		self.bytes.get(12).copied().unwrap_or(0)
	}

	pub fn kind(&self) -> EntryKind {
		EntryKind::from_code(self.bytes.get(13).copied().unwrap_or(0))
	}

	/// For a drive entry, its letter.
	pub fn drive_letter(&self) -> Option<u8> {
		match (self.kind(), self.bytes.get(1)) {
			(EntryKind::Drive, Some(l)) => Some(*l),
			_ => None,
		}
	}

	fn key(&self) -> (u8, &[u8]) {
		(self.kind().rank(), self.bytes.get(..12).unwrap_or(&[]))
	}
}

fn get<S: ReadStore + ?Sized>(store: &S, id: ChunkId, i: usize) -> Entry {
	let mut e = Entry { bytes: [0; REC] };
	store.read(id, (i * REC) as u16, &mut e.bytes);
	e
}

fn put<S: Store + ?Sized>(store: &mut S, id: ChunkId, i: usize, e: &Entry) {
	store.write(id, (i * REC) as u16, &e.bytes);
}

fn less(a: &Entry, b: &Entry) -> bool {
	a.key() < b.key()
}

/// Heapsort of the first `n` records of a chunk, in place: no allocation, O(n log n), the same result for the same set.
fn sort<S: Store + ?Sized>(store: &mut S, id: ChunkId, n: usize) {
	fn sift<S: Store + ?Sized>(store: &mut S, id: ChunkId, mut root: usize, end: usize) {
		loop {
			let mut child = 2 * root + 1;
			if child >= end {
				return;
			}
			if child + 1 < end && less(&get(store, id, child), &get(store, id, child + 1)) {
				child += 1;
			}
			let (r, c) = (get(store, id, root), get(store, id, child));
			if less(&r, &c) {
				put(store, id, root, &c);
				put(store, id, child, &r);
				root = child;
			} else {
				return;
			}
		}
	}
	for start in (0..n / 2).rev() {
		sift(store, id, start, n);
	}
	for end in (1..n).rev() {
		let (a, b) = (get(store, id, 0), get(store, id, end));
		put(store, id, 0, &b);
		put(store, id, end, &a);
		sift(store, id, 0, end);
	}
}

/// A Yes or No question the shell asks, then answers with [`FileDlg::answer`].
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Question {
	/// `C:\DOCS\LETTER.QLD already exists. Replace it?` The default button is No.
	Overwrite,
	/// Saving in text format removes the formatting, pictures and layout (SPEC). The default button is Yes.
	TextWarning,
}

/// Why the dialog refused, for the shell to put in a message naming [`FileDlg::chosen`].
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DlgError {
	/// The name or path is not a valid DOS name
	BadName(NameError),
	/// Open: there is no such file
	NotFound,
	/// Save As: the folder does not exist
	NoPath,
	/// Save As: the file is read-only
	ReadOnly,
	/// The drive (the letter) has no disk or is not ready
	NotReady(u8),
	/// The file system refused
	Fs(FsError),
	/// No `Store` memory for the lists
	NoMemory,
}

/// What a key did.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
	/// Nothing the shell must act on; redraw
	None,
	/// Ask this, then call [`FileDlg::answer`]
	Ask(Question),
	/// Done: use [`FileDlg::chosen`] and [`FileDlg::chosen_type`]. The lists are already freed.
	Accept,
	/// The dialog was cancelled. The lists are already freed.
	Cancel,
	/// Show this error; the dialog stays open and unchanged
	Error(DlgError),
}

/// Where the dialog starts.
#[derive(Copy, Clone, Debug)]
pub struct Start<'a> {
	/// The folder to open, such as the document's own; empty or missing means the current folder of the current drive
	pub folder: &'a [u8],
	/// The File name field's text, selected so typing replaces it
	pub name: &'a [u8],
	/// The index in the dialog's types
	pub type_index: usize,
	/// How many rows the lists show
	pub rows: u8,
}

/// The file dialog. About 400 bytes; the lists are in the `Store`.
#[derive(Clone, Debug)]
pub struct FileDlg {
	kind: Kind,
	folder: PathBuf,
	name: Field<PATH_MAX>,
	files: List,
	dirs: List,
	type_box: Combo,
	drive_box: Combo,
	focus: Focus<7>,
	files_id: Option<ChunkId>,
	dirs_id: Option<ChunkId>,
	nfiles: u16,
	ndirs: u16,
	more_files: bool,
	more_dirs: bool,
	pattern: [u8; PATTERN_MAX],
	pattern_len: u8,
	drive_mask: u32,
	chosen: PathBuf,
	chosen_type: u8,
	pending: Option<Question>,
	done: bool,
}

impl FileDlg {
	/// Opens the dialog: reads the starting folder into the lists. `Err` when there is no memory for the lists or not even the
	/// current folder can be read.
	pub fn new<S: Store + ?Sized, F: FileSystem + ?Sized>(
		kind: Kind,
		start: Start<'_>,
		fs: &mut F,
		store: &mut S,
	) -> Result<FileDlg, DlgError> {
		let files_id = store.alloc(FILES_BYTES).ok_or(DlgError::NoMemory)?;
		let Some(dirs_id) = store.alloc(DIRS_BYTES) else {
			store.free(files_id);
			return Err(DlgError::NoMemory);
		};
		let mut focus = Focus::<7>::new();
		for (c, label) in [
			(Control::Field, &b"File &name:"[..]),
			(Control::List, b"Fi&les:"),
			(Control::List, b"&Directories:"),
			(
				Control::Combo,
				if kind == Kind::SaveAs {
					b"Save file as &type:"
				} else {
					b"List files of &type:"
				},
			),
			(Control::Combo, b"Dri&ves:"),
			(Control::Button, b"OK"),
			(Control::Button, b"Cancel"),
		] {
			focus.add(c, label);
		}
		focus.set_default(Some(OK));
		let mut name = Field::<PATH_MAX>::new(FieldKind::Name);
		name.set(start.name);
		let rows = start.rows.max(1);
		let mut type_box = Combo::new();
		type_box.set_count(types(kind).len());
		type_box.set(start.type_index);
		let mut d = FileDlg {
			kind,
			folder: PathBuf::new(),
			name,
			files: List::new(rows),
			dirs: List::new(rows),
			type_box,
			drive_box: Combo::new(),
			focus,
			files_id: Some(files_id),
			dirs_id: Some(dirs_id),
			nfiles: 0,
			ndirs: 0,
			more_files: false,
			more_dirs: false,
			pattern: [0; PATTERN_MAX],
			pattern_len: 0,
			drive_mask: fs.drives(),
			chosen: PathBuf::new(),
			chosen_type: 0,
			pending: None,
			done: false,
		};
		d.drive_box.set_count(d.drive_mask.count_ones() as usize);
		let mut first = PathBuf::from_bytes(start.folder).unwrap_or_default();
		let mut opened = !first.is_empty() && d.read_folder(&first, fs, store).is_ok();
		if !opened {
			first = PathBuf::new();
			opened =
				fs.current_dir(0, &mut first).is_ok() && d.read_folder(&first, fs, store).is_ok();
		}
		if !opened {
			d.close(store);
			return Err(DlgError::Fs(FsError::NotReady));
		}
		d.folder = first;
		d.sync_drive();
		Ok(d)
	}

	// ----- what a painter reads -----

	pub fn kind(&self) -> Kind {
		self.kind
	}

	/// The folder shown above the Directories list, such as `C:\DOCS\LETTERS`.
	pub fn folder(&self) -> &[u8] {
		self.folder.as_bytes()
	}

	/// The File name field.
	pub fn name(&self) -> &Field<PATH_MAX> {
		&self.name
	}

	/// The Files list's selection and scrolling.
	pub fn files(&self) -> &List {
		&self.files
	}

	/// The Directories list's selection and scrolling.
	pub fn dirs(&self) -> &List {
		&self.dirs
	}

	/// The type drop-down.
	pub fn type_box(&self) -> &Combo {
		&self.type_box
	}

	/// The Drives drop-down; its choices are [`FileDlg::drive_letter`].
	pub fn drive_box(&self) -> &Combo {
		&self.drive_box
	}

	/// The focus ring: which control has the keyboard, and the default button.
	pub fn focus(&self) -> &Focus<7> {
		&self.focus
	}

	/// The letter of the `i`th drive in the Drives drop-down.
	pub fn drive_letter(&self, i: usize) -> Option<u8> {
		nth_drive(self.drive_mask, i)
	}

	/// The type list's entries for this dialog.
	pub fn types(&self) -> &'static [FileType] {
		types(self.kind)
	}

	/// The index of the chosen type.
	pub fn type_index(&self) -> usize {
		self.type_box.selected().unwrap_or(0)
	}

	/// Writes how the list shows the chosen type, such as `Quill Documents (*.QLD)`.
	pub fn type_label(&self, i: usize, out: &mut [u8]) -> Option<usize> {
		write_label(self.types().get(i)?, out)
	}

	/// The pattern in force: what was typed into the name field, or the chosen type's.
	pub fn pattern(&self) -> &[u8] {
		if self.pattern_len > 0 {
			self.pattern
				.get(..usize::from(self.pattern_len))
				.unwrap_or(&[])
		} else {
			self.types()
				.get(self.type_index())
				.map_or(&b"*.*"[..], |t| t.patterns.as_bytes())
		}
	}

	/// An entry of the Files or Directories list; `None` past the end or after the dialog has ended.
	pub fn entry<S: ReadStore + ?Sized>(&self, store: &S, dirs: bool, i: usize) -> Option<Entry> {
		let (id, n) = if dirs {
			(self.dirs_id?, self.ndirs)
		} else {
			(self.files_id?, self.nfiles)
		};
		(i < usize::from(n)).then(|| get(store, id, i))
	}

	/// True when the listing stopped at 512 files; the shell shows `(more files)` and asks for a pattern.
	pub fn more_files(&self) -> bool {
		self.more_files
	}

	/// True when folders were left out of the Directories list.
	pub fn more_dirs(&self) -> bool {
		self.more_dirs
	}

	/// The full path the last `Accept`, `Ask` or `Error` was about, such as `C:\DOCS\LETTER.QLD`.
	pub fn chosen(&self) -> &[u8] {
		self.chosen.as_bytes()
	}

	/// The type chosen when the dialog ended. For Save As this decides the format written, not the extension.
	pub fn chosen_type(&self) -> usize {
		usize::from(self.chosen_type)
	}

	/// The document format of the chosen type, for Save As.
	pub fn chosen_format(&self) -> Option<Format> {
		self.types().get(self.chosen_type()).and_then(|t| t.format)
	}

	/// True when a question is waiting for [`FileDlg::answer`].
	pub fn asking(&self) -> Option<Question> {
		self.pending
	}

	/// True once the dialog has accepted or cancelled.
	pub fn is_done(&self) -> bool {
		self.done
	}

	// ----- ending -----

	/// Frees the lists. Called by `Accept` and `Cancel`; call it yourself if the dialog is abandoned some other way. Safe to call twice.
	pub fn close<S: Store + ?Sized>(&mut self, store: &mut S) {
		if let Some(id) = self.files_id.take() {
			store.free(id);
		}
		if let Some(id) = self.dirs_id.take() {
			store.free(id);
		}
		self.nfiles = 0;
		self.ndirs = 0;
		self.files.set_count(0);
		self.dirs.set_count(0);
		self.done = true;
	}

	fn finish<S: Store + ?Sized>(&mut self, store: &mut S, out: Outcome) -> Outcome {
		self.pending = None;
		self.chosen_type = self.type_index() as u8;
		self.close(store);
		out
	}

	// ----- reading a folder -----

	fn patterns_copy(&self) -> ([u8; PATTERN_MAX], usize) {
		let mut buf = [0u8; PATTERN_MAX];
		let p = self.pattern();
		let n = p.len().min(buf.len());
		if let (Some(dst), Some(src)) = (buf.get_mut(..n), p.get(..n)) {
			dst.copy_from_slice(src);
		}
		(buf, n)
	}

	/// Reads `dir` into the lists. On error the lists are left empty; the caller puts the old folder back.
	fn read_folder<S: Store + ?Sized, F: FileSystem + ?Sized>(
		&mut self,
		dir: &PathBuf,
		fs: &mut F,
		store: &mut S,
	) -> Result<(), FsError> {
		let (Some(fid), Some(did)) = (self.files_id, self.dirs_id) else {
			return Err(FsError::Unsupported);
		};
		let (pat, pat_len) = self.patterns_copy();
		let patterns = pat.get(..pat_len).unwrap_or(&[]);
		let ndrives = self.drive_mask.count_ones() as usize;
		// Room for `[..]` and every drive after the folders.
		let folder_cap = DIRS_MAX.saturating_sub(1 + ndrives);
		let (mut nf, mut nd) = (0usize, 0usize);
		let (mut more_f, mut more_d) = (false, false);
		let result = fs.list(dir.as_bytes(), &mut |name: &[u8], attr: u8| {
			if name == b"."
				|| name == b".."
				|| attr & (ATTR_HIDDEN | ATTR_SYSTEM | ATTR_VOLUME) != 0
			{
				return;
			}
			if attr & ATTR_DIR != 0 {
				if let Some(e) = Entry::new(name, attr, EntryKind::Folder) {
					if nd < folder_cap {
						put(store, did, nd, &e);
						nd += 1;
					} else {
						more_d = true;
					}
				}
			} else if let Some(e) = Entry::new(name, attr, EntryKind::File) {
				if matches_any(patterns, e.name()) {
					if nf < FILES_MAX {
						put(store, fid, nf, &e);
						nf += 1;
					} else {
						more_f = true;
					}
				}
			}
		});
		if let Err(e) = result {
			// What was read is a mixture of two folders: show nothing rather than that.
			self.nfiles = 0;
			self.ndirs = 0;
			self.files.set_count(0);
			self.dirs.set_count(0);
			return Err(e);
		}
		if !dir.is_root() {
			if let Some(e) = Entry::new(b"..", ATTR_DIR, EntryKind::Up) {
				put(store, did, nd, &e);
				nd += 1;
			}
		}
		for i in 0..ndrives {
			if let Some(l) = nth_drive(self.drive_mask, i) {
				if let Some(e) = Entry::new(&[b'-', l, b'-'], 0, EntryKind::Drive) {
					put(store, did, nd, &e);
					nd += 1;
				}
			}
		}
		sort(store, fid, nf);
		sort(store, did, nd);
		self.nfiles = nf as u16;
		self.ndirs = nd as u16;
		self.more_files = more_f;
		self.more_dirs = more_d;
		self.files.set_count(nf);
		self.files.select(0);
		self.dirs.set_count(nd);
		self.dirs.select(0);
		Ok(())
	}

	fn error_for(path: &PathBuf, e: FsError) -> DlgError {
		match e {
			FsError::NotReady => DlgError::NotReady(path.drive().unwrap_or(b'A')),
			other => DlgError::Fs(other),
		}
	}

	/// Moves to `path`, or stays where it is and says why not.
	fn go<S: Store + ?Sized, F: FileSystem + ?Sized>(
		&mut self,
		path: PathBuf,
		fs: &mut F,
		store: &mut S,
	) -> Result<(), DlgError> {
		match self.read_folder(&path, fs, store) {
			Ok(()) => {
				self.folder = path;
				self.sync_drive();
				Ok(())
			}
			Err(e) => {
				let old = self.folder;
				let _ = self.read_folder(&old, fs, store);
				self.sync_drive();
				Err(Self::error_for(&path, e))
			}
		}
	}

	fn relist<S: Store + ?Sized, F: FileSystem + ?Sized>(
		&mut self,
		fs: &mut F,
		store: &mut S,
	) -> Outcome {
		let here = self.folder;
		match self.go(here, fs, store) {
			Ok(()) => Outcome::None,
			Err(e) => Outcome::Error(e),
		}
	}

	/// Points the Drives drop-down at the folder's drive.
	fn sync_drive(&mut self) {
		if let Some(d) = self.folder.drive() {
			let idx = (0..d.saturating_sub(b'A'))
				.filter(|i| self.drive_mask & (1 << i) != 0)
				.count();
			self.drive_box.set(idx);
		}
	}

	// ----- keys -----

	/// Handles a key. After `Accept` or `Cancel` the dialog is over and every call returns `None`.
	pub fn key<S: Store + ?Sized, F: FileSystem + ?Sized>(
		&mut self,
		k: DlgKey,
		fs: &mut F,
		store: &mut S,
	) -> Outcome {
		if self.done || self.pending.is_some() {
			return Outcome::None;
		}
		match self.key_inner(k, fs, store) {
			Outcome::Cancel => self.finish(store, Outcome::Cancel),
			out => out,
		}
	}

	fn key_inner<S: Store + ?Sized, F: FileSystem + ?Sized>(
		&mut self,
		k: DlgKey,
		fs: &mut F,
		store: &mut S,
	) -> Outcome {
		if self.type_box.is_open() {
			return self.combo_key(TYPES, k, fs, store);
		}
		if self.drive_box.is_open() {
			return self.combo_key(DRIVES, k, fs, store);
		}
		let focused = self.focus.focused();
		if k == DlgKey::Enter {
			match focused {
				FILES => return self.accept_selected_file(fs, store),
				DIRS => return self.open_selected_entry(fs, store),
				_ => {}
			}
		}
		match self.focus.key(k) {
			Nav::Cancel => return Outcome::Cancel,
			Nav::Moved(_) => return Outcome::None,
			Nav::Press(CANCEL) => return Outcome::Cancel,
			Nav::Press(OK) | Nav::Enter(_) => return self.accept(fs, store),
			Nav::Press(_) => return Outcome::None,
			Nav::Unhandled => {}
		}
		match focused {
			NAME => {
				self.name.key(k);
				Outcome::None
			}
			FILES => self.files_key(k, store),
			DIRS => self.dirs_key(k, store),
			TYPES | DRIVES => self.combo_key(focused, k, fs, store),
			_ => Outcome::None,
		}
	}

	/// The first character of each entry of a list, for type-ahead.
	fn first_of<'a, S: ReadStore + ?Sized>(
		store: &'a S,
		id: Option<ChunkId>,
	) -> impl Fn(usize) -> u8 + 'a {
		move |i| {
			let Some(id) = id else {
				return 0;
			};
			let mut one = [0u8; 1];
			store.read(id, (i * REC) as u16, &mut one);
			one[0]
		}
	}

	fn files_key<S: Store + ?Sized>(&mut self, k: DlgKey, store: &mut S) -> Outcome {
		let before = self.files.selected();
		if !self.files.key(k) {
			if let DlgKey::Char(c) = k {
				if c.is_ascii_alphanumeric() {
					let first = Self::first_of(&*store, self.files_id);
					self.files.type_ahead(c, first);
				}
			}
		}
		if self.files.selected() != before {
			self.copy_selected_name(store);
		}
		Outcome::None
	}

	fn dirs_key<S: Store + ?Sized>(&mut self, k: DlgKey, store: &mut S) -> Outcome {
		if !self.dirs.key(k) {
			if let DlgKey::Char(c) = k {
				if c.is_ascii_alphanumeric() {
					let first = Self::first_of(&*store, self.dirs_id);
					self.dirs.type_ahead(c, first);
				}
			}
		}
		Outcome::None
	}

	/// Moving in Files copies the name into the field.
	fn copy_selected_name<S: ReadStore + ?Sized>(&mut self, store: &S) {
		if let Some(e) = self
			.files
			.selected()
			.and_then(|i| self.entry(store, false, i))
		{
			self.name.set(e.name());
		}
	}

	fn combo_key<S: Store + ?Sized, F: FileSystem + ?Sized>(
		&mut self,
		which: u8,
		k: DlgKey,
		fs: &mut F,
		store: &mut S,
	) -> Outcome {
		let ev = if which == TYPES {
			self.type_box.key(k)
		} else {
			self.drive_box.key(k)
		};
		if ev != ComboEvent::Changed {
			return Outcome::None;
		}
		if which == TYPES {
			self.type_changed(fs, store)
		} else {
			match self.drive_box.selected().and_then(|i| self.drive_letter(i)) {
				Some(l) => self.change_drive(l, fs, store),
				None => Outcome::None,
			}
		}
	}

	fn type_changed<S: Store + ?Sized, F: FileSystem + ?Sized>(
		&mut self,
		fs: &mut F,
		store: &mut S,
	) -> Outcome {
		self.pattern_len = 0;
		if self.kind == Kind::SaveAs {
			let ext = self
				.types()
				.get(self.type_index())
				.map_or(&b""[..], |t| t.default_ext.as_bytes());
			if let Some(n) = swap_extension(self.name.as_bytes(), ext, Rule::Any) {
				if n.as_bytes() != self.name.as_bytes() {
					self.name.set(n.as_bytes());
				}
			}
		}
		self.relist(fs, store)
	}

	fn change_drive<S: Store + ?Sized, F: FileSystem + ?Sized>(
		&mut self,
		letter: u8,
		fs: &mut F,
		store: &mut S,
	) -> Outcome {
		let mut path = PathBuf::new();
		match fs.current_dir(letter, &mut path) {
			Ok(()) => {}
			Err(FsError::NotReady) => {
				self.sync_drive();
				return Outcome::Error(DlgError::NotReady(letter));
			}
			Err(_) => path = PathBuf::root(letter),
		}
		match self.go(path, fs, store) {
			Ok(()) => Outcome::None,
			Err(e) => Outcome::Error(e),
		}
	}

	/// Enter in the Directories list: open the folder, go up, or change drive.
	fn open_selected_entry<S: Store + ?Sized, F: FileSystem + ?Sized>(
		&mut self,
		fs: &mut F,
		store: &mut S,
	) -> Outcome {
		let Some(e) = self
			.dirs
			.selected()
			.and_then(|i| self.entry(store, true, i))
		else {
			return Outcome::None;
		};
		match e.kind() {
			EntryKind::Up => match self.folder.parent() {
				Some(p) => self.go_outcome(p, fs, store),
				None => Outcome::None,
			},
			EntryKind::Folder => match self.folder.join(e.name()) {
				Ok(p) => self.go_outcome(p, fs, store),
				Err(err) => Outcome::Error(DlgError::BadName(err)),
			},
			EntryKind::Drive => match e.drive_letter() {
				Some(l) => self.change_drive(l, fs, store),
				None => Outcome::None,
			},
			EntryKind::File => Outcome::None,
		}
	}

	fn go_outcome<S: Store + ?Sized, F: FileSystem + ?Sized>(
		&mut self,
		p: PathBuf,
		fs: &mut F,
		store: &mut S,
	) -> Outcome {
		match self.go(p, fs, store) {
			Ok(()) => Outcome::None,
			Err(e) => Outcome::Error(e),
		}
	}

	/// Enter in the Files list: take the file.
	fn accept_selected_file<S: Store + ?Sized, F: FileSystem + ?Sized>(
		&mut self,
		fs: &mut F,
		store: &mut S,
	) -> Outcome {
		self.copy_selected_name(store);
		self.accept(fs, store)
	}

	// ----- accepting -----

	fn default_ext(&self) -> &'static [u8] {
		self.types()
			.get(self.type_index())
			.map_or(&b""[..], |t| t.default_ext.as_bytes())
	}

	/// OK, or Enter in the name field: reads what was typed.
	fn accept<S: Store + ?Sized, F: FileSystem + ?Sized>(
		&mut self,
		fs: &mut F,
		store: &mut S,
	) -> Outcome {
		let typed = match PathBuf::from_bytes(self.name.as_bytes()) {
			Some(t) if !t.is_empty() => t,
			_ => return Outcome::None,
		};
		// 1. A wildcard becomes the list's pattern; a folder part before it is entered first.
		if is_wild(typed.as_bytes()) {
			return self.take_pattern(&typed, fs, store);
		}
		// 2. A save adds the type's extension to a name with none. An open tries it if the name as typed is not there.
		let with_ext = if self.kind == Kind::SaveAs {
			with_default_ext(typed.as_bytes(), self.default_ext())
		} else {
			Some(typed)
		};
		let Some(text) = with_ext else {
			return Outcome::Error(DlgError::BadName(NameError::PathTooLong));
		};
		let folder = self.folder;
		let resolved = match resolve(&folder, text.as_bytes(), |d| {
			let mut p = PathBuf::new();
			fs.current_dir(d, &mut p).ok().map(|()| p)
		}) {
			Ok(r) => r,
			Err(e) => {
				self.chosen = typed;
				return Outcome::Error(DlgError::BadName(e));
			}
		};
		self.chosen = resolved.path;
		let attr = if resolved.path.is_root() {
			Some(ATTR_DIR)
		} else {
			fs.exists(resolved.path.as_bytes())
		};
		// 3. A folder: go there.
		if resolved.dir_hint || attr.is_some_and(|a| a & ATTR_DIR != 0) {
			if attr.is_none() && !resolved.path.is_root() {
				return Outcome::Error(DlgError::NoPath);
			}
			return match self.go(resolved.path, fs, store) {
				Ok(()) => {
					self.name.set(b"");
					Outcome::None
				}
				Err(e) => Outcome::Error(e),
			};
		}
		match self.kind {
			Kind::Open | Kind::Picture => {
				self.accept_existing(typed, attr, resolved.path, fs, store)
			}
			Kind::SaveAs => self.accept_save(attr, resolved.path, fs, store),
		}
	}

	fn accept_existing<S: Store + ?Sized, F: FileSystem + ?Sized>(
		&mut self,
		typed: PathBuf,
		attr: Option<u8>,
		path: PathBuf,
		fs: &mut F,
		store: &mut S,
	) -> Outcome {
		if attr.is_some() {
			return self.finish(store, Outcome::Accept);
		}
		// Not there as typed: with no dot, try the type's extension, as a default extension does.
		let ext = self.default_ext();
		if !ext.is_empty() && !last_part(typed.as_bytes()).contains(&b'.') {
			if let Some(named) = with_default_ext(typed.as_bytes(), ext) {
				let folder = self.folder;
				if let Ok(r) = resolve(&folder, named.as_bytes(), |_| None) {
					if fs
						.exists(r.path.as_bytes())
						.is_some_and(|a| a & ATTR_DIR == 0)
					{
						self.chosen = r.path;
						return self.finish(store, Outcome::Accept);
					}
				}
			}
		}
		self.chosen = path;
		Outcome::Error(DlgError::NotFound)
	}

	fn accept_save<S: Store + ?Sized, F: FileSystem + ?Sized>(
		&mut self,
		attr: Option<u8>,
		path: PathBuf,
		fs: &mut F,
		store: &mut S,
	) -> Outcome {
		match attr {
			Some(a) if a & ATTR_READONLY != 0 => return Outcome::Error(DlgError::ReadOnly),
			Some(_) => {
				self.pending = Some(Question::Overwrite);
				return Outcome::Ask(Question::Overwrite);
			}
			None => {}
		}
		// A new file needs its folder to exist.
		if let Some(parent) = path.parent() {
			if !parent.is_root()
				&& !fs
					.exists(parent.as_bytes())
					.is_some_and(|a| a & ATTR_DIR != 0)
			{
				return Outcome::Error(DlgError::NoPath);
			}
		}
		if self.type_is_text() {
			self.pending = Some(Question::TextWarning);
			return Outcome::Ask(Question::TextWarning);
		}
		self.finish(store, Outcome::Accept)
	}

	fn type_is_text(&self) -> bool {
		self.types().get(self.type_index()).and_then(|t| t.format) == Some(Format::Text)
	}

	/// A typed pattern such as `*.TXT` or `\DOCS\*.QLD`.
	fn take_pattern<S: Store + ?Sized, F: FileSystem + ?Sized>(
		&mut self,
		typed: &PathBuf,
		fs: &mut F,
		store: &mut S,
	) -> Outcome {
		let bytes = typed.as_bytes();
		let pat = last_part(bytes);
		let dir = bytes.get(..bytes.len() - pat.len()).unwrap_or(&[]);
		if pat.len() > PATTERN_MAX || is_wild(dir) {
			return Outcome::Error(DlgError::BadName(NameError::BadChar(b'*')));
		}
		if !dir.is_empty() {
			let folder = self.folder;
			let r = match resolve(&folder, dir, |d| {
				let mut p = PathBuf::new();
				fs.current_dir(d, &mut p).ok().map(|()| p)
			}) {
				Ok(r) => r,
				Err(e) => return Outcome::Error(DlgError::BadName(e)),
			};
			if let Err(e) = self.go(r.path, fs, store) {
				return Outcome::Error(e);
			}
		}
		self.pattern = [0; PATTERN_MAX];
		for (dst, src) in self.pattern.iter_mut().zip(pat) {
			*dst = src.to_ascii_uppercase();
		}
		self.pattern_len = pat.len() as u8;
		self.name.set(pat);
		self.relist(fs, store)
	}

	/// Answers the question [`Outcome::Ask`] asked. Yes to a replace goes on to the text warning when the type is text, otherwise
	/// accepts; No goes back to the dialog with the name selected.
	pub fn answer<S: Store + ?Sized>(&mut self, yes: bool, store: &mut S) -> Outcome {
		let Some(q) = self.pending.take() else {
			return Outcome::None;
		};
		if !yes {
			self.name.select_all();
			self.focus.set_focus(NAME);
			return Outcome::None;
		}
		match q {
			Question::Overwrite if self.type_is_text() => {
				self.pending = Some(Question::TextWarning);
				Outcome::Ask(Question::TextWarning)
			}
			_ => self.finish(store, Outcome::Accept),
		}
	}

	#[cfg(test)]
	pub(crate) fn focus_for_test(&mut self, id: u8) {
		self.focus.set_focus(id);
	}

	#[cfg(test)]
	pub(crate) fn focus_name_for_test(&mut self) {
		self.focus.set_focus(NAME);
	}

	#[cfg(test)]
	pub(crate) fn select_name_for_test(&mut self) {
		self.name.select_all();
	}

	/// Changes how many rows the lists show, for a screen of another height.
	pub fn set_rows(&mut self, rows: u8) {
		self.files.set_rows(rows);
		self.dirs.set_rows(rows);
	}
}

/// The letter of the `i`th set bit of a drive mask.
fn nth_drive(mask: u32, i: usize) -> Option<u8> {
	(0..26u8)
		.filter(|b| mask & (1 << b) != 0)
		.nth(i)
		.map(|b| b'A' + b)
}
