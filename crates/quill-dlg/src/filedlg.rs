// The file dialog: Open, Save As and Insert Picture as a state machine over keys, a file system and a store.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The dialog draws nothing. A shell feeds it `DlgKey`s (or, for a pointer, the operations a key also reaches) and acts on the
// `Outcome`: nothing, a question to ask (Yes or No, answered with `answer`), an error to show, the chosen path, or a cancel. A
// painter reads the state through the accessors.
//
// The folder is one list: `..` first, then the folders, then the files that match the type, as 24-byte records in `Store` chunks
// of 1,024 (two pages, so 2,000 entries at most; past that the listing stops and a flag says so). The records hold the name, the
// attributes, the size and the DOS date and time, so a click on a header sorts them again without reading the disk: a heapsort
// in place, folders before files and ties by name, so the order is the same whatever order the file system returns them. The
// places (drives, the folders the shell names and the recent files) are the other list. The dialog never changes the current
// directory: it keeps its own folder and returns full paths. The chunks are freed as soon as the dialog ends, by `Accept`,
// `Cancel` or `close`.

use crate::filelayout::{default_button, text, type_label};
use crate::focus::{Control, Focus, Nav};
use crate::fs::{
	DriveKind, FileSystem, Found, FsError, ATTR_DIR, ATTR_HIDDEN, ATTR_READONLY, ATTR_SYSTEM,
	ATTR_VOLUME,
};
use crate::key::DlgKey;
use crate::name83::{
	is_wild, last_part, matches_any, resolve, Name83, NameError, PathBuf, PATH_MAX,
};
use crate::savename::{ext_of, swap_extension, with_default_ext, Rule};
use crate::types::{type_for_ext, types, write_label, FileType, Format, Kind};
use crate::widgets::{Combo, ComboEvent, Field, FieldKind, List};
use core::cmp::Ordering;
use quill_core::store::{ChunkId, ReadStore, Store};

/// The most entries listed: `..`, folders and files together.
pub const ENTRIES_MAX: usize = 2000;
/// The longest pattern typed into the name field, such as `*.TXT`.
pub const PATTERN_MAX: usize = 24;
/// The longest prefix type-ahead keeps.
pub const AHEAD_MAX: usize = 12;
/// Ticks of the shell's poll (about 55 ms each) after which type-ahead starts a new prefix.
pub const AHEAD_TICKS: u8 = 9;
/// The most named folders and recent files in the places, each.
pub const PLACE_SPECS_MAX: usize = 4;

const REC: usize = 24;
const PAGE_ENTRIES: usize = 1024;
const PAGE_BYTES: u16 = (PAGE_ENTRIES * REC) as u16;
const SPEC_REC: usize = 96;
const SPECS_BYTES: u16 = (2 * PLACE_SPECS_MAX * SPEC_REC) as u16;

/// The controls, in Tab order.
pub const NAME: u8 = 0;
pub const TYPES: u8 = 1;
pub const OK: u8 = 2;
pub const CANCEL: u8 = 3;
pub const LOOK_IN: u8 = 4;
pub const UP: u8 = 5;
pub const NEW_FOLDER: u8 = 6;
pub const PLACES: u8 = 7;
pub const LIST: u8 = 8;

/// What an entry of the list is.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum EntryKind {
	/// The parent folder, shown `..`
	Up,
	Folder,
	File,
}

impl EntryKind {
	fn code(self) -> u8 {
		match self {
			EntryKind::Up => 0,
			EntryKind::Folder => 1,
			EntryKind::File => 2,
		}
	}

	fn from_code(c: u8) -> EntryKind {
		match c {
			0 => EntryKind::Up,
			1 => EntryKind::Folder,
			_ => EntryKind::File,
		}
	}
}

/// One entry of the list: a 24-byte record.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Entry {
	bytes: [u8; REC],
}

impl Entry {
	fn new(f: &Found<'_>, kind: EntryKind) -> Option<Entry> {
		if f.name.is_empty() || f.name.len() > 12 {
			return None;
		}
		let mut bytes = [0u8; REC];
		for (dst, src) in bytes.iter_mut().zip(f.name) {
			*dst = src.to_ascii_uppercase();
		}
		*bytes.get_mut(12)? = f.attr;
		*bytes.get_mut(13)? = kind.code();
		bytes
			.get_mut(14..18)?
			.copy_from_slice(&f.size.to_le_bytes());
		bytes
			.get_mut(18..20)?
			.copy_from_slice(&f.date.to_le_bytes());
		bytes
			.get_mut(20..22)?
			.copy_from_slice(&f.time.to_le_bytes());
		Some(Entry { bytes })
	}

	/// The name, upper case: `LETTER.QLD`, a folder's name, `..` for the parent.
	pub fn name(&self) -> &[u8] {
		let n = self.bytes.get(..12).unwrap_or(&[]);
		let len = n.iter().position(|b| *b == 0).unwrap_or(12);
		n.get(..len).unwrap_or(&[])
	}

	pub fn attr(&self) -> u8 {
		self.bytes.get(12).copied().unwrap_or(0)
	}

	pub fn kind(&self) -> EntryKind {
		EntryKind::from_code(self.bytes.get(13).copied().unwrap_or(2))
	}

	/// The size in bytes; 0 for a folder.
	pub fn size(&self) -> u32 {
		self.bytes
			.get(14..18)
			.and_then(|b| b.try_into().ok())
			.map_or(0, u32::from_le_bytes)
	}

	/// The DOS date: year since 1980 in bits 9 to 15, month in 5 to 8, day in 0 to 4.
	pub fn date(&self) -> u16 {
		self.bytes
			.get(18..20)
			.and_then(|b| b.try_into().ok())
			.map_or(0, u16::from_le_bytes)
	}

	/// The DOS time: hour in bits 11 to 15, minute in 5 to 10, seconds in twos in 0 to 4.
	pub fn time(&self) -> u16 {
		self.bytes
			.get(20..22)
			.and_then(|b| b.try_into().ok())
			.map_or(0, u16::from_le_bytes)
	}

	fn rank(&self) -> u8 {
		self.kind().code()
	}
}

/// A column the list sorts by.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SortBy {
	Name,
	Size,
	Date,
}

/// How the list is sorted: the column and the direction. `..` stays first and folders stay before files either way.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Sort {
	pub by: SortBy,
	pub descending: bool,
}

impl Sort {
	fn order(&self, a: &Entry, b: &Entry) -> Ordering {
		let ranks = a.rank().cmp(&b.rank());
		if ranks != Ordering::Equal || a.kind() == EntryKind::Up {
			return ranks;
		}
		let by_name = a.name().cmp(b.name());
		let key = match self.by {
			SortBy::Name => by_name,
			SortBy::Size => a.size().cmp(&b.size()),
			SortBy::Date => (a.date(), a.time()).cmp(&(b.date(), b.time())),
		};
		let key = if self.descending { key.reverse() } else { key };
		key.then(by_name)
	}
}

/// The pages of records.
#[derive(Copy, Clone, Debug)]
struct Pages([Option<ChunkId>; 2]);

impl Pages {
	fn get<S: ReadStore + ?Sized>(&self, store: &S, i: usize) -> Entry {
		let mut e = Entry { bytes: [0; REC] };
		if let Some(Some(id)) = self.0.get(i / PAGE_ENTRIES) {
			store.read(*id, ((i % PAGE_ENTRIES) * REC) as u16, &mut e.bytes);
		}
		e
	}

	fn put<S: Store + ?Sized>(&self, store: &mut S, i: usize, e: &Entry) {
		if let Some(Some(id)) = self.0.get(i / PAGE_ENTRIES) {
			store.write(*id, ((i % PAGE_ENTRIES) * REC) as u16, &e.bytes);
		}
	}

	fn free<S: Store + ?Sized>(&mut self, store: &mut S) {
		for p in &mut self.0 {
			if let Some(id) = p.take() {
				store.free(id);
			}
		}
	}

	/// Heapsort of the first `n` records, in place: no allocation, O(n log n), the same result for the same set.
	fn sort<S: Store + ?Sized>(&self, store: &mut S, n: usize, order: &Sort) {
		let less = |a: &Entry, b: &Entry| order.order(a, b) == Ordering::Less;
		let sift = |store: &mut S, mut root: usize, end: usize| loop {
			let mut child = 2 * root + 1;
			if child >= end {
				return;
			}
			if child + 1 < end && less(&self.get(&*store, child), &self.get(&*store, child + 1)) {
				child += 1;
			}
			let (r, c) = (self.get(&*store, root), self.get(&*store, child));
			if less(&r, &c) {
				self.put(store, root, &c);
				self.put(store, child, &r);
				root = child;
			} else {
				return;
			}
		};
		for start in (0..n / 2).rev() {
			sift(store, start, n);
		}
		for end in (1..n).rev() {
			let (a, b) = (self.get(&*store, 0), self.get(&*store, end));
			self.put(store, 0, &b);
			self.put(store, end, &a);
			sift(store, 0, end);
		}
	}
}

/// What a place is.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PlaceKind {
	/// A drive, by its letter
	Drive(u8, DriveKind),
	/// A folder the shell named: Documents, Samples, the program's folder, the last one used
	Folder,
	/// A recent file
	Recent,
}

/// One entry of the places list.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Place {
	pub kind: PlaceKind,
	label: [u8; 12],
	label_len: u8,
	/// A drive's root, a folder, or a recent file's full path
	pub path: PathBuf,
}

impl Place {
	/// The text shown: `C:`, `Documents`, `LETTER.QLD`.
	pub fn label(&self) -> &[u8] {
		self.label.get(..usize::from(self.label_len)).unwrap_or(&[])
	}
}

/// A folder the shell wants among the places.
#[derive(Copy, Clone, Debug)]
pub struct PlaceSpec<'a> {
	/// At most 12 bytes
	pub label: &'a [u8],
	pub path: &'a [u8],
}

/// One choice of the Look in drop-down: a drive, or a folder on the way down to the current one.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct LookItem {
	/// The drive's letter, or for a folder the letter of the drive it is on
	pub letter: u8,
	/// 0 for a drive, 1 for the folders below the root, and so on, for a painter's indent
	pub indent: u8,
	/// The folder's own name, empty for a drive
	label: [u8; 12],
	label_len: u8,
	/// Where choosing it goes: a root for a drive that is not the current one is that drive's own folder, see `FileDlg::look_path`
	pub path: PathBuf,
}

impl LookItem {
	/// A folder's name, or empty for a drive.
	pub fn label(&self) -> &[u8] {
		self.label.get(..usize::from(self.label_len)).unwrap_or(&[])
	}

	pub fn is_drive(&self) -> bool {
		self.indent == 0
	}
}

/// A Yes or No question the shell asks, then answers with [`FileDlg::answer`].
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Question {
	/// `C:\DOCS\LETTER.QLD already exists. Do you want to replace it?` The default button is No.
	Overwrite,
	/// Saving in text format removes the formatting, pictures and layout (SPEC). Asked only when [`Start::warn_text`] is set.
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

/// What a pointer press did to the dialog, as the part it landed on says: each is an operation a key also reaches, so a click can
/// never leave the dialog in a state the keyboard cannot.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Point {
	/// Entry `i` of the list: selected
	Item(usize),
	/// Entry `i` of the list, a second press in a double click: opened or accepted
	Open(usize),
	/// Place `i`
	Place(usize),
	/// A column header: sorts by it
	Header(SortBy),
	/// A button (`UP`, `NEW_FOLDER`, `OK` or `CANCEL`), let go over: pressed
	Button(u8),
	/// The field of a drop-down (`LOOK_IN` or `TYPES`) or its arrow: the list opens, or shuts if it was open
	Drop(u8),
	/// Row `i` of the list a drop-down shows: chosen
	Pick(usize),
	/// Row `i` of the open drop-down list under the pointer: lit
	Light(usize),
	/// The file name field, a press at character offset `n`: the caret goes there
	Name(usize),
	/// The list's scroll bar, which the shell tracks itself
	Bar,
	/// Anywhere else: an open drop-down shuts
	Away,
}

/// What a key did.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
	/// Nothing the shell must act on; redraw
	None,
	/// Ask this, then call [`FileDlg::answer`]
	Ask(Question),
	/// Done: use [`FileDlg::chosen`], [`FileDlg::chosen_type`] and [`FileDlg::chosen_format`]. The lists are already freed.
	Accept,
	/// The dialog was cancelled. The lists are already freed.
	Cancel,
	/// Show this error; the dialog stays open
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
	/// How many rows the list and the places show
	pub rows: u8,
	/// The named folders after the drives in the places, such as Samples and the last folder used; the first four are kept
	pub folders: &'a [PlaceSpec<'a>],
	/// The recent files, full paths, after the folders; the first four are kept
	pub recent: &'a [&'a [u8]],
	/// Ask the text-format warning (SPEC) when saving as text. Off by default, and on no build until the core asks everywhere.
	pub warn_text: bool,
}

impl<'a> Start<'a> {
	/// A start in `folder` with `name` selected in the field and the first type, with no extra places.
	pub fn new(folder: &'a [u8], name: &'a [u8], rows: u8) -> Start<'a> {
		Start {
			folder,
			name,
			type_index: 0,
			rows,
			folders: &[],
			recent: &[],
			warn_text: false,
		}
	}
}

/// The file dialog. About 450 bytes; the lists are in the `Store`.
#[derive(Clone, Debug)]
pub struct FileDlg {
	kind: Kind,
	folder: PathBuf,
	name: Field<PATH_MAX>,
	list: List,
	places: List,
	type_box: Combo,
	look: Combo,
	focus: Focus<9>,
	pages: Pages,
	specs_id: Option<ChunkId>,
	count: u16,
	folders: u16,
	more: bool,
	sort: Sort,
	pattern: [u8; PATTERN_MAX],
	pattern_len: u8,
	drive_mask: u32,
	drive_kinds: u64,
	nspecs: u8,
	chosen: PathBuf,
	chosen_type: u8,
	pending: Option<Question>,
	done: bool,
	warn_text: bool,
	ahead: [u8; AHEAD_MAX],
	ahead_len: u8,
	ahead_age: u8,
	new_folder: Option<Field<12>>,
}

impl FileDlg {
	/// Opens the dialog: reads the starting folder into the list. `Err` when there is no memory for the lists or not even the
	/// current folder can be read.
	pub fn new<S: Store + ?Sized, F: FileSystem + ?Sized>(
		kind: Kind,
		start: Start<'_>,
		fs: &mut F,
		store: &mut S,
	) -> Result<FileDlg, DlgError> {
		let first = store.alloc(PAGE_BYTES).ok_or(DlgError::NoMemory)?;
		let Some(specs_id) = store.alloc(SPECS_BYTES) else {
			store.free(first);
			return Err(DlgError::NoMemory);
		};
		let mut focus = Focus::<9>::new();
		for (c, label) in [
			(Control::Field, text::NAME),
			(Control::Combo, type_label(kind)),
			(Control::Button, default_button(kind)),
			(Control::Button, text::CANCEL),
			(Control::Combo, text::LOOK_IN),
			(Control::Button, text::UP),
			(Control::Button, text::NEW_FOLDER),
			(Control::List, text::PLACES),
			(Control::List, text::LIST),
		] {
			focus.add(c, label);
		}
		focus.set_default(Some(OK));
		focus.set_enabled(NEW_FOLDER, kind == Kind::SaveAs);
		let mut name = Field::<PATH_MAX>::new(FieldKind::Name);
		name.set(start.name);
		name.select_all();
		let rows = start.rows.max(1);
		let mut type_box = Combo::new();
		type_box.set_count(types(kind).len());
		type_box.set(start.type_index);
		let mut d = FileDlg {
			kind,
			folder: PathBuf::new(),
			name,
			list: List::new(rows),
			places: List::new(rows),
			type_box,
			look: Combo::new(),
			focus,
			pages: Pages([Some(first), None]),
			specs_id: Some(specs_id),
			count: 0,
			folders: 0,
			more: false,
			sort: Sort {
				by: SortBy::Name,
				descending: false,
			},
			pattern: [0; PATTERN_MAX],
			pattern_len: 0,
			drive_mask: 0,
			drive_kinds: 0,
			nspecs: 0,
			chosen: PathBuf::new(),
			chosen_type: 0,
			pending: None,
			done: false,
			warn_text: start.warn_text,
			ahead: [0; AHEAD_MAX],
			ahead_len: 0,
			ahead_age: 0,
			new_folder: None,
		};
		d.read_drives(fs);
		d.write_specs(&start, store);
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
		d.sync_look();
		d.select_name_in_list(store);
		Ok(d)
	}

	// ----- what a painter reads -----

	pub fn kind(&self) -> Kind {
		self.kind
	}

	/// The folder shown in the Look in field, such as `C:\DOCS\LETTERS`.
	pub fn folder(&self) -> &[u8] {
		self.folder.as_bytes()
	}

	/// The File name field.
	pub fn name(&self) -> &Field<PATH_MAX> {
		&self.name
	}

	/// The list's selection and scrolling.
	pub fn list(&self) -> &List {
		&self.list
	}

	/// The places list's selection and scrolling.
	pub fn places(&self) -> &List {
		&self.places
	}

	/// The type drop-down.
	pub fn type_box(&self) -> &Combo {
		&self.type_box
	}

	/// The Look in drop-down; its choices are [`FileDlg::look_item`].
	pub fn look_box(&self) -> &Combo {
		&self.look
	}

	/// The focus ring: which control has the keyboard, and the default button.
	pub fn focus(&self) -> &Focus<9> {
		&self.focus
	}

	/// How the list is sorted.
	pub fn sort(&self) -> Sort {
		self.sort
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

	/// An entry of the list; `None` past the end or after the dialog has ended.
	pub fn entry<S: ReadStore + ?Sized>(&self, store: &S, i: usize) -> Option<Entry> {
		(i < usize::from(self.count) && self.pages.0.first().is_some_and(Option::is_some))
			.then(|| self.pages.get(store, i))
	}

	/// How many entries the list holds.
	pub fn len(&self) -> usize {
		usize::from(self.count)
	}

	pub fn is_empty(&self) -> bool {
		self.count == 0
	}

	/// How many of the entries are folders (`..` not counted).
	pub fn folder_count(&self) -> usize {
		usize::from(self.folders)
	}

	/// True when the listing stopped at [`ENTRIES_MAX`]; the shell shows `(more files: type a pattern such as R*.QLD)`.
	pub fn more(&self) -> bool {
		self.more
	}

	/// How many places there are: the drives, then the named folders, then the recent files.
	pub fn place_count(&self) -> usize {
		self.drive_mask.count_ones() as usize + usize::from(self.nspecs)
	}

	/// The `i`th place.
	pub fn place<S: ReadStore + ?Sized>(&self, store: &S, i: usize) -> Option<Place> {
		let ndrives = self.drive_mask.count_ones() as usize;
		if i < ndrives {
			let letter = nth_drive(self.drive_mask, i)?;
			let mut label = [0u8; 12];
			*label.get_mut(0)? = letter;
			*label.get_mut(1)? = b':';
			return Some(Place {
				kind: PlaceKind::Drive(letter, self.drive_kind_of(letter)),
				label,
				label_len: 2,
				path: PathBuf::root(letter),
			});
		}
		let id = self.specs_id?;
		let k = i - ndrives;
		if k >= usize::from(self.nspecs) {
			return None;
		}
		let mut rec = [0u8; SPEC_REC];
		store.read(id, (k * SPEC_REC) as u16, &mut rec);
		let kind = match rec.first() {
			Some(1) => PlaceKind::Recent,
			_ => PlaceKind::Folder,
		};
		let label_len = rec.get(1).copied().unwrap_or(0).min(12);
		let mut label = [0u8; 12];
		label.copy_from_slice(rec.get(2..14)?);
		let path_len = usize::from(rec.get(14).copied().unwrap_or(0)).min(PATH_MAX);
		Some(Place {
			kind,
			label,
			label_len,
			path: PathBuf::from_bytes(rec.get(15..15 + path_len)?)?,
		})
	}

	fn drive_kind_of(&self, letter: u8) -> DriveKind {
		let shift = u32::from(letter.saturating_sub(b'A')) * 2;
		match self.drive_kinds.checked_shr(shift).map_or(0, |v| v & 3) {
			1 => DriveKind::Removable,
			2 => DriveKind::CdRom,
			3 => DriveKind::Network,
			_ => DriveKind::Fixed,
		}
	}

	/// How many choices the Look in drop-down has: every drive, and the folders from the current drive's root down to the folder.
	pub fn look_len(&self) -> usize {
		self.look.count()
	}

	/// The `i`th choice of the Look in drop-down.
	pub fn look_item(&self, i: usize) -> Option<LookItem> {
		let ndrives = self.drive_mask.count_ones() as usize;
		let depth = depth_of(self.folder.as_bytes());
		let here = self
			.folder
			.drive()
			.and_then(|d| (0..ndrives).find(|k| nth_drive(self.drive_mask, *k) == Some(d)));
		let drive_item = |k: usize| {
			let letter = nth_drive(self.drive_mask, k)?;
			Some(LookItem {
				letter,
				indent: 0,
				label: [0; 12],
				label_len: 0,
				path: PathBuf::root(letter),
			})
		};
		let Some(here) = here else {
			return drive_item(i);
		};
		if i <= here {
			return drive_item(i);
		}
		if i <= here + depth {
			let k = i - here;
			let end = component_end(self.folder.as_bytes(), k)?;
			let start = if k == 1 {
				3
			} else {
				component_end(self.folder.as_bytes(), k - 1)? + 1
			};
			let mut label = [0u8; 12];
			let part = self.folder.as_bytes().get(start..end)?;
			let n = part.len().min(12);
			label.get_mut(..n)?.copy_from_slice(part.get(..n)?);
			return Some(LookItem {
				letter: self.folder.drive()?,
				indent: k.min(255) as u8,
				label,
				label_len: n as u8,
				path: PathBuf::from_bytes(self.folder.as_bytes().get(..end)?)?,
			});
		}
		drive_item(i - depth)
	}

	/// The prefix type-ahead has built, for a painter that shows it.
	pub fn type_ahead_prefix(&self) -> &[u8] {
		self.ahead.get(..usize::from(self.ahead_len)).unwrap_or(&[])
	}

	/// The box asking for a new folder's name, when it is open.
	pub fn new_folder(&self) -> Option<&Field<12>> {
		self.new_folder.as_ref()
	}

	/// The full path the last `Accept`, `Ask` or `Error` was about, such as `C:\DOCS\LETTER.QLD`.
	pub fn chosen(&self) -> &[u8] {
		self.chosen.as_bytes()
	}

	/// The type chosen when the dialog ended.
	pub fn chosen_type(&self) -> usize {
		usize::from(self.chosen_type)
	}

	/// The document format to write, for Save As: the one a known extension names (`LETTER.RTF` is RTF whatever the type says), and
	/// otherwise the chosen type's.
	pub fn chosen_format(&self) -> Option<Format> {
		let by_ext = ext_of(self.chosen.as_bytes())
			.and_then(|e| type_for_ext(Kind::SaveAs, e))
			.and_then(|i| types(Kind::SaveAs).get(i))
			.and_then(|t| t.format);
		by_ext.or_else(|| self.types().get(self.chosen_type()).and_then(|t| t.format))
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
		self.pages.free(store);
		if let Some(id) = self.specs_id.take() {
			store.free(id);
		}
		self.count = 0;
		self.list.set_count(0);
		self.places.set_count(0);
		self.nspecs = 0;
		self.new_folder = None;
		self.done = true;
	}

	fn finish<S: Store + ?Sized>(&mut self, store: &mut S, out: Outcome) -> Outcome {
		self.pending = None;
		self.chosen_type = self.type_index() as u8;
		self.close(store);
		out
	}

	// ----- reading -----

	fn read_drives<F: FileSystem + ?Sized>(&mut self, fs: &mut F) {
		self.drive_mask = fs.drives();
		self.drive_kinds = 0;
		for b in 0..26u8 {
			if self.drive_mask & (1 << b) != 0 {
				let code: u64 = match fs.drive_kind(b'A' + b) {
					DriveKind::Fixed => 0,
					DriveKind::Removable => 1,
					DriveKind::CdRom => 2,
					DriveKind::Network => 3,
				};
				self.drive_kinds |= code << (u32::from(b) * 2);
			}
		}
	}

	fn write_specs<S: Store + ?Sized>(&mut self, start: &Start<'_>, store: &mut S) {
		let Some(id) = self.specs_id else {
			return;
		};
		let mut n = 0usize;
		let mut put = |kind: u8, label: &[u8], path: &[u8], n: &mut usize| {
			let mut rec = [0u8; SPEC_REC];
			let label = label.get(..label.len().min(12)).unwrap_or(&[]);
			let Some(path) = PathBuf::from_bytes(path) else {
				return;
			};
			if let Some(r) = rec.get_mut(0) {
				*r = kind;
			}
			if let Some(r) = rec.get_mut(1) {
				*r = label.len() as u8;
			}
			if let Some(r) = rec.get_mut(2..2 + label.len()) {
				r.copy_from_slice(label);
			}
			if let Some(r) = rec.get_mut(14) {
				*r = path.len() as u8;
			}
			if let Some(r) = rec.get_mut(15..15 + path.len()) {
				r.copy_from_slice(path.as_bytes());
			}
			store.write(id, (*n * SPEC_REC) as u16, &rec);
			*n += 1;
		};
		for f in start.folders.iter().take(PLACE_SPECS_MAX) {
			put(0, f.label, f.path, &mut n);
		}
		for r in start.recent.iter().take(PLACE_SPECS_MAX) {
			put(1, last_part(r), r, &mut n);
		}
		self.nspecs = n as u8;
		self.places.set_count(self.place_count());
	}

	fn patterns_copy(&self) -> ([u8; PATTERN_MAX], usize) {
		let mut buf = [0u8; PATTERN_MAX];
		let p = self.pattern();
		let n = p.len().min(buf.len());
		if let (Some(dst), Some(src)) = (buf.get_mut(..n), p.get(..n)) {
			dst.copy_from_slice(src);
		}
		(buf, n)
	}

	/// Reads `dir` into the list. On error the list is left empty; the caller puts the old folder back.
	fn read_folder<S: Store + ?Sized, F: FileSystem + ?Sized>(
		&mut self,
		dir: &PathBuf,
		fs: &mut F,
		store: &mut S,
	) -> Result<(), FsError> {
		if self.pages.0.first().is_none_or(Option::is_none) {
			return Err(FsError::Unsupported);
		}
		// The second page is held only while a folder needs it.
		if let Some(id) = self.pages.0.get_mut(1).and_then(Option::take) {
			store.free(id);
		}
		let (pat, pat_len) = self.patterns_copy();
		let patterns = pat.get(..pat_len).unwrap_or(&[]);
		let cap = if dir.is_root() {
			ENTRIES_MAX
		} else {
			ENTRIES_MAX - 1
		};
		let mut pages = self.pages;
		let (mut n, mut nfolders, mut more) = (0usize, 0usize, false);
		let result = fs.list(dir.as_bytes(), &mut |f: Found<'_>| {
			if f.name == b"."
				|| f.name == b".."
				|| f.attr & (ATTR_HIDDEN | ATTR_SYSTEM | ATTR_VOLUME) != 0
			{
				return;
			}
			let is_dir = f.attr & ATTR_DIR != 0;
			let kind = if is_dir {
				EntryKind::Folder
			} else {
				EntryKind::File
			};
			let Some(e) = Entry::new(&f, kind) else {
				return;
			};
			if !is_dir && (is_recovery_file(e.name()) || !matches_any(patterns, e.name())) {
				return;
			}
			if n >= cap {
				more = true;
				return;
			}
			if n == PAGE_ENTRIES && pages.0.get(1).is_some_and(Option::is_none) {
				match store.alloc(PAGE_BYTES) {
					Some(id) => {
						if let Some(p) = pages.0.get_mut(1) {
							*p = Some(id);
						}
					}
					None => {
						more = true;
						n = cap;
						return;
					}
				}
			}
			pages.put(store, n, &e);
			n += 1;
			if is_dir {
				nfolders += 1;
			}
		});
		self.pages = pages;
		if let Err(e) = result {
			// What was read is a mixture of two folders: show nothing rather than that.
			self.count = 0;
			self.folders = 0;
			self.list.set_count(0);
			return Err(e);
		}
		if !dir.is_root() {
			let up = Found {
				name: b"..",
				attr: ATTR_DIR,
				size: 0,
				date: 0,
				time: 0,
			};
			if let Some(e) = Entry::new(&up, EntryKind::Up) {
				// The cap left room for it, but the second page may not exist yet.
				if n == PAGE_ENTRIES && self.pages.0.get(1).is_some_and(Option::is_none) {
					if let Some(id) = store.alloc(PAGE_BYTES) {
						if let Some(p) = self.pages.0.get_mut(1) {
							*p = Some(id);
						}
					}
				}
				self.pages.put(store, n, &e);
				n += 1;
			}
		}
		self.pages.sort(store, n, &self.sort);
		self.count = n as u16;
		self.folders = nfolders as u16;
		self.more = more;
		self.list.set_count(n);
		self.list.select(0);
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
		self.ahead_len = 0;
		match self.read_folder(&path, fs, store) {
			Ok(()) => {
				self.folder = path;
				self.sync_look();
				Ok(())
			}
			Err(e) => {
				let old = self.folder;
				let _ = self.read_folder(&old, fs, store);
				self.sync_look();
				Err(Self::error_for(&path, e))
			}
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

	/// Reads the folder again (and the drives), keeping the selected entry when it is still there. The shell calls it for F5.
	pub fn refresh<S: Store + ?Sized, F: FileSystem + ?Sized>(
		&mut self,
		fs: &mut F,
		store: &mut S,
	) -> Outcome {
		if self.done {
			return Outcome::None;
		}
		let kept = self.selected_entry(store);
		self.read_drives(fs);
		self.places.set_count(self.place_count());
		let here = self.folder;
		let out = self.go_outcome(here, fs, store);
		if let Some(e) = kept {
			self.select_named(store, e.name());
		}
		out
	}

	/// Points the Look in drop-down at the folder.
	fn sync_look(&mut self) {
		let ndrives = self.drive_mask.count_ones() as usize;
		let depth = depth_of(self.folder.as_bytes());
		self.look.set_count(ndrives + depth);
		let here = self
			.folder
			.drive()
			.and_then(|d| (0..ndrives).find(|k| nth_drive(self.drive_mask, *k) == Some(d)))
			.unwrap_or(0);
		self.look.set(here + depth);
	}

	/// After a read: the entry named like the File name field is selected, as Save As and a reopened Open expect.
	fn select_name_in_list<S: ReadStore + ?Sized>(&mut self, store: &S) {
		let typed = self.name.as_bytes();
		if typed.is_empty() || typed.len() > 12 {
			return;
		}
		let mut buf = [0u8; 12];
		for (d, s) in buf.iter_mut().zip(typed) {
			*d = s.to_ascii_uppercase();
		}
		self.select_named(store, buf.get(..typed.len()).unwrap_or(&[]));
	}

	fn select_named<S: ReadStore + ?Sized>(&mut self, store: &S, name: &[u8]) {
		for i in 0..self.len() {
			if self.entry(store, i).is_some_and(|e| e.name() == name) {
				self.list.select(i);
				return;
			}
		}
	}

	fn selected_entry<S: ReadStore + ?Sized>(&self, store: &S) -> Option<Entry> {
		self.list.selected().and_then(|i| self.entry(store, i))
	}

	// ----- operations a pointer reaches as well as a key -----

	/// Selects entry `i` and gives the list the focus; a file's name goes into the field.
	pub fn select<S: ReadStore + ?Sized>(&mut self, i: usize, store: &S) {
		if self.done || self.new_folder.is_some() || self.pending.is_some() {
			return;
		}
		self.focus.set_focus(LIST);
		self.list.select(i);
		self.ahead_len = 0;
		self.copy_selected_name(store);
	}

	/// Scrolls the list so entry `top` is the first shown, as a scroll bar does.
	pub fn scroll_to(&mut self, top: usize) {
		self.list.scroll_to(top);
	}

	/// Opens entry `i` (a folder, or the parent) or accepts it (a file): a double click, or Enter.
	pub fn activate<S: Store + ?Sized, F: FileSystem + ?Sized>(
		&mut self,
		i: usize,
		fs: &mut F,
		store: &mut S,
	) -> Outcome {
		if self.done || self.new_folder.is_some() || self.pending.is_some() {
			return Outcome::None;
		}
		self.select(i, &*store);
		self.open_selected(fs, store)
	}

	/// Sorts by `by`; the same column again reverses it. The selected entry stays selected.
	pub fn sort_by<S: Store + ?Sized>(&mut self, by: SortBy, store: &mut S) {
		if self.done {
			return;
		}
		self.sort = if self.sort.by == by {
			Sort {
				by,
				descending: !self.sort.descending,
			}
		} else {
			Sort {
				by,
				descending: false,
			}
		};
		let kept = self.selected_entry(&*store);
		self.pages.sort(store, self.len(), &self.sort);
		if let Some(e) = kept {
			self.select_named(&*store, e.name());
		}
	}

	/// Goes to the `i`th place: a drive or folder is entered, a recent file is opened (Open) or its name and folder are filled in
	/// (Save As).
	pub fn go_place<S: Store + ?Sized, F: FileSystem + ?Sized>(
		&mut self,
		i: usize,
		fs: &mut F,
		store: &mut S,
	) -> Outcome {
		if self.done || self.new_folder.is_some() || self.pending.is_some() {
			return Outcome::None;
		}
		self.focus.set_focus(PLACES);
		self.places.select(i);
		let Some(p) = self.place(&*store, i) else {
			return Outcome::None;
		};
		match p.kind {
			PlaceKind::Drive(letter, _) => self.change_drive(letter, fs, store),
			PlaceKind::Folder => self.go_outcome(p.path, fs, store),
			PlaceKind::Recent => {
				// A recent file kept as `SAMPLES\DEMO.QLD` is relative to the folder the program runs in: made a full path first.
				let mut here = self.folder;
				if let Some(d) = self.folder.drive() {
					let _ = fs.current_dir(d, &mut here);
				}
				let Ok(full) = resolve(&here, p.path.as_bytes(), |_| None) else {
					return Outcome::Error(DlgError::NotFound);
				};
				let p = Place {
					path: full.path,
					..p
				};
				if self.kind == Kind::SaveAs {
					let Some(folder) = p.path.parent() else {
						return Outcome::None;
					};
					let out = self.go_outcome(folder, fs, store);
					if out == Outcome::None {
						self.name.set(p.path.file_name());
						self.select_name_in_list(&*store);
					}
					out
				} else {
					self.chosen = p.path;
					if fs
						.exists(p.path.as_bytes())
						.is_some_and(|a| a & ATTR_DIR == 0)
					{
						self.finish(store, Outcome::Accept)
					} else {
						Outcome::Error(DlgError::NotFound)
					}
				}
			}
		}
	}

	/// Goes up one folder, selecting the one it came from.
	pub fn up<S: Store + ?Sized, F: FileSystem + ?Sized>(
		&mut self,
		fs: &mut F,
		store: &mut S,
	) -> Outcome {
		let Some(parent) = self.folder.parent() else {
			return Outcome::None;
		};
		let came_from = self.folder;
		let out = self.go_outcome(parent, fs, store);
		if out == Outcome::None {
			self.select_named(&*store, came_from.file_name());
		}
		out
	}

	/// Chooses the `i`th place of the Look in drop-down.
	pub fn go_look<S: Store + ?Sized, F: FileSystem + ?Sized>(
		&mut self,
		i: usize,
		fs: &mut F,
		store: &mut S,
	) -> Outcome {
		let Some(item) = self.look_item(i) else {
			return Outcome::None;
		};
		if item.is_drive() && self.folder.drive() != Some(item.letter) {
			self.change_drive(item.letter, fs, store)
		} else {
			self.go_outcome(item.path, fs, store)
		}
	}

	/// Opens the New folder box.
	pub fn start_new_folder(&mut self) {
		if self.kind == Kind::SaveAs && !self.done && self.new_folder.is_none() {
			self.new_folder = Some(Field::<12>::new(FieldKind::Name));
		}
	}

	/// Makes the folder named in the New folder box, and selects it.
	fn make_folder<S: Store + ?Sized, F: FileSystem + ?Sized>(
		&mut self,
		fs: &mut F,
		store: &mut S,
	) -> Outcome {
		let Some(f) = self.new_folder else {
			return Outcome::None;
		};
		let name = match Name83::parse(f.as_bytes()) {
			Ok(n) => n,
			Err(e) => return Outcome::Error(DlgError::BadName(e)),
		};
		let path = match self.folder.join(name.as_bytes()) {
			Ok(p) => p,
			Err(e) => return Outcome::Error(DlgError::BadName(e)),
		};
		self.chosen = path;
		if let Err(e) = fs.mkdir(path.as_bytes()) {
			return Outcome::Error(Self::error_for(&path, e));
		}
		self.new_folder = None;
		let here = self.folder;
		let out = self.go_outcome(here, fs, store);
		self.select_named(&*store, name.as_bytes());
		out
	}

	/// Ticks of the shell's poll (about 55 ms): a type-ahead prefix older than [`AHEAD_TICKS`] starts again.
	pub fn tick(&mut self) {
		self.ahead_age = self.ahead_age.saturating_add(1);
		if self.ahead_age >= AHEAD_TICKS {
			self.ahead_len = 0;
		}
	}

	/// Changes how many rows the list and the places show, for a screen of another height.
	pub fn set_rows(&mut self, rows: u8) {
		self.list.set_rows(rows);
		self.places.set_rows(rows);
	}

	/// Handles a pointer press on `p`. While a drop-down is open only its own list is live: a choice picks, and anything else
	/// shuts it. A box over the dialog, or a question, takes no pointer.
	pub fn point<S: Store + ?Sized, F: FileSystem + ?Sized>(
		&mut self,
		p: Point,
		fs: &mut F,
		store: &mut S,
	) -> Outcome {
		if self.done || self.new_folder.is_some() || self.pending.is_some() {
			return Outcome::None;
		}
		let open = if self.look.is_open() { LOOK_IN } else { TYPES };
		if self.look.is_open() || self.type_box.is_open() {
			let combo = if open == TYPES {
				&mut self.type_box
			} else {
				&mut self.look
			};
			match p {
				Point::Light(i) => combo.light(i),
				Point::Pick(i) if combo.pick(i) == ComboEvent::Changed => {
					return self.combo_changed(open, fs, store)
				}
				Point::Pick(_) => {}
				_ => {
					combo.key(DlgKey::Esc);
				}
			}
			return Outcome::None;
		}
		match p {
			Point::Item(i) => self.select(i, &*store),
			Point::Open(i) => return self.activate(i, fs, store),
			Point::Place(i) => return self.go_place(i, fs, store),
			Point::Header(by) => self.sort_by(by, store),
			Point::Button(id) if self.focus.set_focus(id) => {
				return self.key(DlgKey::Enter, fs, store)
			}
			Point::Drop(id) => {
				self.focus.set_focus(id);
				return self.key(DlgKey::F4, fs, store);
			}
			Point::Name(at) => {
				self.focus.set_focus(NAME);
				self.name.place(at, false);
			}
			_ => {}
		}
		Outcome::None
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
		if self.new_folder.is_some() {
			return self.new_folder_key(k, fs, store);
		}
		if self.type_box.is_open() {
			return self.combo_key(TYPES, k, fs, store);
		}
		if self.look.is_open() {
			return self.combo_key(LOOK_IN, k, fs, store);
		}
		match k {
			DlgKey::F5 => return self.refresh(fs, store),
			DlgKey::Alt(b'1') => {
				self.sort_by(SortBy::Name, store);
				return Outcome::None;
			}
			DlgKey::Alt(b'2') => {
				self.sort_by(SortBy::Size, store);
				return Outcome::None;
			}
			DlgKey::Alt(b'3') => {
				self.sort_by(SortBy::Date, store);
				return Outcome::None;
			}
			_ => {}
		}
		let focused = self.focus.focused();
		if k == DlgKey::Enter {
			match focused {
				LIST => return self.open_selected(fs, store),
				PLACES => {
					return match self.places.selected() {
						Some(i) => self.go_place(i, fs, store),
						None => Outcome::None,
					}
				}
				_ => {}
			}
		}
		match self.focus.key(k) {
			Nav::Cancel => return Outcome::Cancel,
			Nav::Moved(_) => {
				self.ahead_len = 0;
				return Outcome::None;
			}
			Nav::Press(CANCEL) => return Outcome::Cancel,
			Nav::Press(UP) => return self.up(fs, store),
			Nav::Press(NEW_FOLDER) => {
				self.start_new_folder();
				return Outcome::None;
			}
			Nav::Press(OK) | Nav::Enter(_) => return self.accept(fs, store),
			Nav::Press(_) => return Outcome::None,
			Nav::Unhandled => {}
		}
		match focused {
			NAME => {
				self.name.key(k);
				Outcome::None
			}
			LIST => self.list_key(k, fs, store),
			PLACES => {
				match k {
					DlgKey::Backspace => return self.up(fs, store),
					_ => {
						self.places.key(k);
					}
				}
				Outcome::None
			}
			TYPES | LOOK_IN => self.combo_key(focused, k, fs, store),
			_ => Outcome::None,
		}
	}

	fn list_key<S: Store + ?Sized, F: FileSystem + ?Sized>(
		&mut self,
		k: DlgKey,
		fs: &mut F,
		store: &mut S,
	) -> Outcome {
		if k == DlgKey::Backspace {
			return self.up(fs, store);
		}
		let before = self.list.selected();
		if self.list.key(k) {
			self.ahead_len = 0;
		} else if let DlgKey::Char(c) = k {
			if c.is_ascii_alphanumeric() || c == b'_' || c == b'$' || c == b'~' {
				self.type_ahead(c, &*store);
			}
		}
		if self.list.selected() != before {
			self.copy_selected_name(&*store);
		}
		Outcome::None
	}

	/// Types `c` into the prefix and selects the first entry that starts with it; the same letter again moves to the next entry
	/// that starts with that letter, as a Windows list does.
	fn type_ahead<S: ReadStore + ?Sized>(&mut self, c: u8, store: &S) {
		let c = c.to_ascii_uppercase();
		if self.ahead_age >= AHEAD_TICKS {
			self.ahead_len = 0;
		}
		self.ahead_age = 0;
		let n = self.len();
		let len = usize::from(self.ahead_len);
		let repeating = len > 0
			&& self
				.ahead
				.get(..len)
				.is_some_and(|p| p.iter().all(|b| *b == c));
		if len == 0 || repeating {
			// A single letter, typed again and again: the entries that start with it, one after another.
			let from = if repeating {
				self.list.selected().map_or(0, |s| s + 1)
			} else {
				0
			};
			self.ahead = [0; AHEAD_MAX];
			if let Some(f) = self.ahead.first_mut() {
				*f = c;
			}
			self.ahead_len = 1;
			for step in 0..n {
				let i = (from + step) % n;
				if self
					.entry(store, i)
					.is_some_and(|e| e.name().first() == Some(&c))
				{
					self.list.select(i);
					return;
				}
			}
			return;
		}
		let mut next = self.ahead;
		let Some(slot) = next.get_mut(len) else {
			return;
		};
		*slot = c;
		let prefix = next.get(..len + 1).unwrap_or(&[]);
		for i in 0..n {
			if self
				.entry(store, i)
				.is_some_and(|e| e.name().starts_with(prefix))
			{
				self.ahead = next;
				self.ahead_len = (len + 1) as u8;
				self.list.select(i);
				return;
			}
		}
	}

	/// Moving in the list onto a file copies its name into the field.
	fn copy_selected_name<S: ReadStore + ?Sized>(&mut self, store: &S) {
		if let Some(e) = self.selected_entry(store) {
			if e.kind() == EntryKind::File {
				self.name.set(e.name());
			}
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
			self.look.key(k)
		};
		if ev != ComboEvent::Changed {
			return Outcome::None;
		}
		self.combo_changed(which, fs, store)
	}

	/// The choice of drop-down `which` changed: apply it.
	fn combo_changed<S: Store + ?Sized, F: FileSystem + ?Sized>(
		&mut self,
		which: u8,
		fs: &mut F,
		store: &mut S,
	) -> Outcome {
		if which == TYPES {
			self.type_changed(fs, store)
		} else {
			let chosen = self.look.selected();
			let out = match chosen {
				Some(i) => self.go_look(i, fs, store),
				None => Outcome::None,
			};
			self.sync_look();
			out
		}
	}

	/// Chooses type `i` from the drop-down (a click on its list), as Enter on it would.
	pub fn choose_type<S: Store + ?Sized, F: FileSystem + ?Sized>(
		&mut self,
		i: usize,
		fs: &mut F,
		store: &mut S,
	) -> Outcome {
		if self.done || self.pending.is_some() || i >= self.types().len() || i == self.type_index()
		{
			return Outcome::None;
		}
		self.type_box.set(i);
		self.type_changed(fs, store)
	}

	fn type_changed<S: Store + ?Sized, F: FileSystem + ?Sized>(
		&mut self,
		fs: &mut F,
		store: &mut S,
	) -> Outcome {
		self.pattern_len = 0;
		if self.kind == Kind::SaveAs {
			let ext = self.default_ext();
			if let Some(n) = swap_extension(self.name.as_bytes(), ext, Rule::Any) {
				if n.as_bytes() != self.name.as_bytes() {
					self.name.set(n.as_bytes());
				}
			}
		}
		let here = self.folder;
		let kept = self.name.as_bytes().len();
		let out = self.go_outcome(here, fs, store);
		if kept > 0 {
			self.select_name_in_list(&*store);
		}
		out
	}

	/// Goes to drive `letter`'s own current folder, as choosing the drive does; a shell calls it to try a drive again after it said
	/// the drive was not ready.
	pub fn go_drive<S: Store + ?Sized, F: FileSystem + ?Sized>(
		&mut self,
		letter: u8,
		fs: &mut F,
		store: &mut S,
	) -> Outcome {
		if self.done || self.new_folder.is_some() || self.pending.is_some() {
			return Outcome::None;
		}
		self.change_drive(letter, fs, store)
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
				self.sync_look();
				return Outcome::Error(DlgError::NotReady(letter));
			}
			Err(_) => path = PathBuf::root(letter),
		}
		self.go_outcome(path, fs, store)
	}

	/// Enter in the list: open the folder or go up, or take the file.
	fn open_selected<S: Store + ?Sized, F: FileSystem + ?Sized>(
		&mut self,
		fs: &mut F,
		store: &mut S,
	) -> Outcome {
		let Some(e) = self.selected_entry(&*store) else {
			return Outcome::None;
		};
		match e.kind() {
			EntryKind::Up => self.up(fs, store),
			EntryKind::Folder => match self.folder.join(e.name()) {
				Ok(p) => self.go_outcome(p, fs, store),
				Err(err) => Outcome::Error(DlgError::BadName(err)),
			},
			EntryKind::File => {
				self.name.set(e.name());
				self.accept(fs, store)
			}
		}
	}

	fn new_folder_key<S: Store + ?Sized, F: FileSystem + ?Sized>(
		&mut self,
		k: DlgKey,
		fs: &mut F,
		store: &mut S,
	) -> Outcome {
		match k {
			DlgKey::Esc => {
				self.new_folder = None;
				Outcome::None
			}
			DlgKey::Enter => self.make_folder(fs, store),
			_ => {
				if let Some(f) = &mut self.new_folder {
					f.key(k);
				}
				Outcome::None
			}
		}
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
		if self.warn_text && self.format_is_text() {
			self.pending = Some(Question::TextWarning);
			return Outcome::Ask(Question::TextWarning);
		}
		self.finish(store, Outcome::Accept)
	}

	/// True when what Save As would write, by the name chosen and the type, is text.
	fn format_is_text(&self) -> bool {
		let by_ext = ext_of(self.chosen.as_bytes()).and_then(|e| type_for_ext(Kind::SaveAs, e));
		let idx = by_ext.unwrap_or_else(|| self.type_index());
		self.types().get(idx).and_then(|t| t.format) == Some(Format::Text)
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
		let here = self.folder;
		self.go_outcome(here, fs, store)
	}

	/// Answers the question [`Outcome::Ask`] asked. Yes to a replace goes on to the text warning when it is asked, otherwise
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
			Question::Overwrite if self.warn_text && self.format_is_text() => {
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
	pub(crate) fn select_name_for_test(&mut self) {
		self.name.select_all();
	}
}

/// A recovery or lock file the autosave writes beside a document: `~` first and one of five extensions. They are never listed.
fn is_recovery_file(name: &[u8]) -> bool {
	if name.first() != Some(&b'~') {
		return false;
	}
	ext_of(name).is_some_and(|e| {
		[&b"RCV"[..], b"RC0", b"RCE", b"$$$", b"LCK"]
			.iter()
			.any(|x| e.eq_ignore_ascii_case(x))
	})
}

/// The letter of the `i`th set bit of a drive mask.
fn nth_drive(mask: u32, i: usize) -> Option<u8> {
	(0..26u8)
		.filter(|b| mask & (1 << b) != 0)
		.nth(i)
		.map(|b| b'A' + b)
}

/// How many folders lie between the root and the end of `path` (`C:\DOCS\LETTERS` is 2, a root 0).
fn depth_of(path: &[u8]) -> usize {
	path.get(3..).map_or(0, |rest| {
		rest.split(|b| *b == b'\\')
			.filter(|p| !p.is_empty())
			.count()
	})
}

/// Where the `k`th folder name (1 is the first below the root) ends in `path`.
fn component_end(path: &[u8], k: usize) -> Option<usize> {
	let mut start = 3;
	for n in 1..=k {
		let rest = path.get(start..)?;
		let len = rest.iter().position(|b| *b == b'\\').unwrap_or(rest.len());
		if n == k {
			return Some(start + len);
		}
		start += len + 1;
	}
	None
}
