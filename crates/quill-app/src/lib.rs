// quill-app: the editor controller shared by every build: keys in, a text-mode screen out.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The app owns the document, the caret and the menus, takes keys, and paints into a `quill_grid::Grid`. Files come in through
//! the [`Files`] trait, so the same code runs on DOS and in the host's tests.

#![no_std]
#![deny(
	clippy::unwrap_used,
	clippy::expect_used,
	clippy::panic,
	clippy::indexing_slicing
)]

pub mod accel;
pub mod bios;
mod chrome;
pub mod cmdline;
pub mod datefmt;
mod find;
pub mod flow;
mod form;
pub mod mouse;
mod native;
mod overlay;
mod paint;
pub mod picture;
pub mod platform;
mod pointer;
mod print;
pub mod recent;
mod reflow;
pub mod safe;
pub mod savename;
pub mod script;
pub mod session;
mod spell_live;
mod spelling;
pub mod view;
pub mod widgets;

use quill_core::doc::{cflag, CFmt, Doc, ListDef, ListLevel, PFmt, StoryKind};
use quill_core::layout::{Line, Measure};
use quill_core::store::{ReadStore, Store};
use quill_core::Error;
use quill_formats::io::{Sink, Source};
use quill_formats::{qld, rtf, text, FormatError};
use quill_grid::GridMeasure;

pub use chrome::{
	tab_stop, CharState, CHROME_FORMAT, CHROME_RULER, CHROME_STATUS, CHROME_TOOLBAR, STYLE_NAMES,
};
pub use native::{Dialog, FindAction, VERSION};
pub use overlay::{Mark, Overlay, Wid, Wk, WIDS, WID_TEXT};
pub use paint::{
	combo_hint, combo_tip, command_hint, command_tip, draw, item_for_mnemonic, line_label,
	menu_box, menu_for_mnemonic, menu_items, menu_items_for, menu_title, mnemonic, slot,
	status_text, Cursor, Item, ListCounters, Slot, COLOUR_HINT, COLOUR_TIP, MENU_COUNT,
};
pub use pointer::Grain;
pub use quill_spell::{Candidate, Suggestions};

/// A key, as the platform decodes it.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Key {
	/// A printable character (ANSI).
	Char(u8),
	/// Enter.
	Enter,
	/// Backspace.
	Backspace,
	/// Delete.
	Delete,
	/// Escape.
	Esc,
	/// Tab.
	Tab,
	/// Cursor keys.
	Left,
	/// Right arrow.
	Right,
	/// Up arrow.
	Up,
	/// Down arrow.
	Down,
	/// Home.
	Home,
	/// End.
	End,
	/// Page Up.
	PageUp,
	/// Page Down.
	PageDown,
	/// Ctrl+Home.
	CtrlHome,
	/// Ctrl+End.
	CtrlEnd,
	/// Shift with an arrow, Home, End or a page key: the same movement, extending the selection.
	Select(Dir),
	/// The Insert key.
	Insert,
	/// A function key, 1 to 12.
	F(u8),
	/// Ctrl with a letter, as the lower-case letter.
	Ctrl(u8),
	/// Alt with a letter or digit (the lower-case letter), or with Backspace (8). A shell without native menus passes it so Alt+letter
	/// opens the menu with that mnemonic.
	Alt(u8),
}

/// A direction for the movement keys.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Dir {
	/// Left arrow.
	Left,
	/// Right arrow.
	Right,
	/// Up arrow.
	Up,
	/// Down arrow.
	Down,
	/// Home.
	Home,
	/// End.
	End,
	/// Page Up.
	PageUp,
	/// Page Down.
	PageDown,
}

/// The attribute scheme of text mode.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Scheme {
	/// Colours: white on blue, bold bright, italic cyan.
	Colour,
	/// Monochrome: normal, bright, reverse and underline.
	Mono,
}

/// How a shell's `create` and `close` treat a file that already exists, which decides how a save reaches the disk.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SaveMode {
	/// `create` writes over the file: the shell has no way to rename or delete. A failed save can truncate the original.
	InPlace,
	/// `create` writes to a temporary file of its own and `close` replaces the original with it, or takes it away (the Linux shells).
	Atomic,
	/// The save writes a temporary file beside the target and replaces the target with `replace` once the new file is whole.
	TempRename,
}

/// What `Files::replace` did.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Replaced {
	/// The target is now the new file.
	Done,
	/// Nothing changed: the target is as it was and the temporary file is still there.
	Refused,
	/// The target was removed and the new file could not take its place: the temporary file holds the whole new document.
	Lost,
}

/// Where files come from and go to.
pub trait Files {
	/// What reads a file.
	type R: Source;
	/// What writes a file.
	type W: Sink;
	/// Opens a file for reading.
	fn open(&mut self, name: &[u8]) -> Option<Self::R>;
	/// Creates or replaces a file for writing.
	fn create(&mut self, name: &[u8]) -> Option<Self::W>;
	/// Finishes a file started with `create`; false when the disk refused it (a full disk shows up here on a floppy).
	fn close(&mut self, w: Self::W) -> bool;
	/// How a save reaches the disk. A shell that can remove and rename says `TempRename`.
	fn save_mode(&self) -> SaveMode {
		SaveMode::InPlace
	}
	/// Deletes a file; false when it could not be (missing, read-only, in use).
	fn remove(&mut self, _name: &[u8]) -> bool {
		false
	}
	/// Renames `from` to `to`, replacing `to` where the system allows that; false when it could not be done.
	fn rename(&mut self, _from: &[u8], _to: &[u8]) -> bool {
		false
	}
	/// Whether a file can be opened.
	fn exists(&mut self, name: &[u8]) -> bool {
		self.open(name).is_some()
	}
	/// The size of a file, or `None` when the shell cannot tell (the save then goes without its size check).
	fn len(&mut self, _name: &[u8]) -> Option<u32> {
		None
	}
	/// Puts the finished file `temp` in the place of `target`: a rename that replaces where the system can, otherwise the target is
	/// removed and then the new file renamed, which is done only once the new file is whole.
	fn replace(&mut self, temp: &[u8], target: &[u8]) -> Replaced {
		if self.rename(temp, target) {
			Replaced::Done
		} else if !self.exists(target) || !self.remove(target) {
			Replaced::Refused
		} else if self.rename(temp, target) {
			Replaced::Done
		} else {
			Replaced::Lost
		}
	}
}

/// Why a save did not finish.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SaveError {
	/// The disk refused the write, the file is locked or read-only, or the folder is gone. The file that was there is as it was.
	Failed,
	/// The old file was removed and the new one could not take its place. The new document is whole in a temporary file beside it.
	Kept,
}

/// A command from a menu or a key.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Command {
	/// File > New.
	New,
	/// File > Open.
	Open,
	/// File > Save.
	Save,
	/// File > Save As.
	SaveAs,
	/// File > Exit.
	Exit,
	/// Edit > Undo.
	Undo,
	/// Edit > Redo.
	Redo,
	/// Paragraph > Bullets.
	Bullets,
	/// Paragraph > Numbering.
	Numbering,
	/// Paragraph > Increase Level.
	LevelUp,
	/// Paragraph > Decrease Level.
	LevelDown,
	/// Document > Header, Footer, First Page Header or First Page Footer (1 to 4), or back to the body (0).
	View(u8),
	/// Insert > Page Number, Page Count, Date or File Name (1 to 4).
	Field(u8),
	/// Document > Different First Page.
	FirstPage,
	/// Edit > Cut.
	Cut,
	/// Edit > Copy.
	Copy,
	/// Edit > Paste.
	Paste,
	/// Edit > Select All.
	SelectAll,
	/// Character > Fonts.
	Fonts,
	/// Paragraph > Tabs.
	Tabs,
	/// Help > About Quill.
	About,
	/// Help > Contents (F1): the Windows builds open QUILL.HLP in WinHelp.
	Help,
	/// Insert > Picture.
	Picture,
	/// File > Print.
	Print,
	/// View > Toolbar (1), Ruler (2), Status Bar (4) or Formatting Bar (8).
	Chrome(u8),
	/// Find > Spelling (F7).
	Spelling,
	/// Find > Check Spelling as You Type.
	SpellAsYouType,
	/// File > Print Preview (Windows builds only).
	Preview,
	/// File > Print Setup (Windows builds only).
	PrintSetup,
	/// File > one of the recent files (0 to 3).
	Recent(u8),
	/// Find > Find.
	Find,
	/// Find > Find Next.
	FindNext,
	/// Find > Replace.
	Replace,
	/// Character > Regular.
	Regular,
	/// Character > Bold.
	Bold,
	/// Character > Italic.
	Italic,
	/// Character > Underline.
	Underline,
	/// Character > Strikethrough.
	Strike,
	/// Character > Superscript.
	Super,
	/// Character > Subscript.
	Sub,
	/// Paragraph > Left, Centred, Right or Justified (0 to 3).
	Align(u8),
	/// Paragraph > Single, 1½ or Double Space (0 to 2).
	Spacing(u8),
	/// View > Zoom: a percentage from 25 to 200, or 0 for the page's width. It changes only the view, so a build with no zoom ignores it.
	Zoom(u16),
}

/// The longest file name the editor keeps: a DOS path, or a Windows path on the build that asks for long names.
pub(crate) const NAME_MAX: usize = if cfg!(feature = "long-names") {
	260
} else {
	64
};

/// The zoom a `QUILL.INI` value names: a percentage, or `Width` (anything that is not a number) for the page's width.
fn parse_zoom(v: &[u8]) -> u16 {
	let mut n = 0u16;
	let mut any = false;
	for b in v.iter().take_while(|b| b.is_ascii_digit()) {
		n = n.saturating_mul(10).saturating_add(u16::from(b - b'0'));
		any = true;
	}
	if any {
		n.clamp(25, 200)
	} else {
		0
	}
}

/// The one-line message shown in the status line until the next key.
#[derive(Copy, Clone)]
pub(crate) struct Msg {
	buf: [u8; 96],
	len: usize,
}

impl Msg {
	const fn new() -> Self {
		Msg {
			buf: [0; 96],
			len: 0,
		}
	}
	fn clear(&mut self) {
		self.len = 0;
	}
	fn push(&mut self, b: &[u8]) {
		for &c in b {
			if let Some(slot) = self.buf.get_mut(self.len) {
				*slot = c;
				self.len += 1;
			}
		}
	}
	pub(crate) fn set(&mut self, t: &str) {
		self.clear();
		self.push(t.as_bytes());
	}
	/// A message with a quoted text in it: `a` + `"text"` + `c`.
	pub(crate) fn set_quoted(&mut self, a: &str, text: &[u8], c: &str) {
		self.clear();
		self.push(a.as_bytes());
		self.push(b"\"");
		self.push(text);
		self.push(b"\"");
		self.push(c.as_bytes());
	}
	pub(crate) fn is_empty(&self) -> bool {
		self.len == 0
	}
	pub(crate) fn bytes(&self) -> &[u8] {
		self.buf.get(..self.len).unwrap_or(&[])
	}
}

/// Undo step labels.
pub mod label {
	/// Typing.
	pub const TYPING: u8 = 1;
	/// Deleting.
	pub const DELETE: u8 = 2;
	/// Character formatting.
	pub const CHAR_FORMAT: u8 = 3;
	/// Paragraph formatting.
	pub const PARA_FORMAT: u8 = 4;
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub(crate) enum Prompt {
	Open,
	SaveAs,
	Print,
	Picture,
}

#[derive(Copy, Clone, PartialEq, Eq)]
#[allow(clippy::large_enum_variant)] // one value lives in the app; boxing would need an allocator
pub(crate) enum Mode {
	Edit,
	Menu {
		menu: u8,
		item: u8,
	},
	Prompt {
		kind: Prompt,
		buf: [u8; NAME_MAX],
		len: usize,
	},
	/// The Save changes question: the command waiting, and the focused button (0 Yes, 1 No, 2 Cancel).
	Confirm(Command, u8),
	About,
	Find(find::FindState),
	Form(form::Form),
	Spell(spelling::SpellDlg),
}

/// The editor.
pub struct App<M: Measure + Clone = GridMeasure> {
	/// The device the document is laid out for.
	pub(crate) measure: M,
	/// Lines the view shows at once (23 on an 80 by 25 screen); the shell sets it.
	pub(crate) rows: usize,
	/// The height in pixels of a window's page area, which scrolling keeps the caret inside; 0 on the text screen.
	pub(crate) view_h: i32,
	pub(crate) doc: Doc,
	pub(crate) caret: u32,
	pub(crate) top: u32,
	want_x: i32,
	pub(crate) name: [u8; NAME_MAX],
	pub(crate) name_len: usize,
	pub(crate) modified: bool,
	pub(crate) mode: Mode,
	typing: bool,
	quit: bool,
	pub(crate) msg: Msg,
	/// Help > Contents was chosen and the shell has not taken the request.
	pub(crate) help: bool,
	pending: Option<Command>,
	/// `discard_changes` cleared the modified flag and the command it was for has not run yet.
	discarded: bool,
	/// The other end of the selection, when there is one.
	pub(crate) anchor: Option<u32>,
	pub(crate) extend: bool,
	pub(crate) overtype: bool,
	pub(crate) scheme: Scheme,
	/// The format the next typed character takes, chosen with no selection.
	typing_fmt: Option<u16>,
	/// Which parts of the window chrome show (`CHROME_*`).
	pub(crate) chrome: u8,
	/// True when dialogs belong to the shell (D86).
	pub(crate) native: bool,
	pub(crate) spell: spelling::SpellData,
	/// The dialog the last command asked the shell for.
	pub(crate) dialog: Option<native::Dialog>,
	/// The folder of the last file dialog.
	pub(crate) folder: [u8; NAME_MAX],
	pub(crate) folder_len: usize,
	/// The story being edited: 0 body, 1 header, 2 footer, 3 first header, 4 first footer. The active story lives in `doc.body`.
	pub(crate) view: u8,
	saved_view: (u32, u32),
	/// The clipboard: text only, until Quill exits.
	pub(crate) clip: [u8; 2048],
	pub(crate) clip_len: usize,
	/// The last search, for Find Next.
	pub(crate) last_find: Option<find::FindState>,
	/// What the platform adds to the About box (the mode and the free memory).
	pub(crate) about: Msg,
	/// The date the Date field prints as (the platform fills it in).
	pub(crate) date: Msg,
	/// The time the Time field prints as (the platform fills it in).
	pub(crate) time: Msg,
	/// The four most recent files, most recent first.
	pub(crate) recent: recent::RecentList,
	/// The settings section the recent files live in: `Recent` (DOS, Win16) or `Recent32`.
	pub(crate) recent_section: &'static str,
	/// The layout checkpoints of the body (see `reflow`).
	pub(crate) reflow: reflow::Reflow,
	/// The format the document was opened in: 0 native, 1 RTF, 2 text.
	fmt: u8,
	/// The view's zoom: a percentage, or 0 for the page's width.
	pub(crate) zoom: u16,
}

impl Default for App {
	fn default() -> Self {
		Self::new()
	}
}

impl App<GridMeasure> {
	/// An app laid out for the DOS text screen, with no document yet; call `reset`.
	pub const fn new() -> Self {
		Self::with_measure(GridMeasure)
	}
}

impl<M: Measure + Clone> App<M> {
	/// An app laid out for `measure`, with no document yet; call `reset`.
	pub const fn with_measure(measure: M) -> Self {
		App {
			measure,
			rows: paint::TEXT_ROWS,
			view_h: 0,
			doc: Doc::new(),
			caret: 0,
			top: 0,
			want_x: 0,
			name: [0; NAME_MAX],
			name_len: 0,
			modified: false,
			mode: Mode::Edit,
			typing: false,
			quit: false,
			msg: Msg::new(),
			help: false,
			pending: None,
			discarded: false,
			anchor: None,
			extend: false,
			overtype: false,
			scheme: Scheme::Colour,
			typing_fmt: None,
			chrome: 15,
			native: false,
			spell: spelling::SpellData::new(),
			dialog: None,
			folder: [0; NAME_MAX],
			folder_len: 0,
			reflow: reflow::Reflow::new(),
			fmt: 0,
			last_find: None,
			about: Msg::new(),
			date: Msg::new(),
			time: Msg::new(),
			recent: recent::RecentList::new(),
			recent_section: "Recent",
			clip: [0; 2048],
			clip_len: 0,
			view: 0,
			saved_view: (0, 0),
			zoom: 0,
		}
	}

	/// Starts an empty, untitled document.
	pub fn reset<S: Store + ?Sized>(&mut self, s: &mut S) -> Result<(), Error> {
		self.doc.init(s)?;
		self.reflow.invalidate();
		self.spell_forget();
		self.caret = 0;
		self.top = 0;
		self.name_len = 0;
		self.modified = false;
		self.mode = Mode::Edit;
		self.typing = false;
		self.sync_layout(s);
		Ok(())
	}

	/// Sets how many lines the view shows at once, for paging and scrolling.
	pub fn set_rows(&mut self, rows: usize) {
		self.rows = rows.max(1);
	}

	/// The view's zoom: a percentage from 25 to 200, or 0 for the page's width.
	pub fn zoom(&self) -> u16 {
		self.zoom
	}

	/// Sets the view's zoom (0 for the page's width); other values are held between 25 and 200 percent.
	pub fn set_zoom(&mut self, percent: u16) {
		self.zoom = if percent == 0 {
			0
		} else {
			percent.clamp(25, 200)
		};
	}

	/// The caret position in the active story.
	pub fn caret(&self) -> u32 {
		self.caret
	}

	/// The index of the first line shown.
	pub fn top_line(&self) -> u32 {
		self.top
	}

	/// Which story the view edits: 0 the body, 1 the header, 2 the footer, 3 the first page's header, 4 its footer.
	pub fn story_view(&self) -> u8 {
		self.view
	}

	/// True when no menu, dialog or prompt of the app's own is open.
	pub fn editing(&self) -> bool {
		self.mode == Mode::Edit
	}

	/// Runs a command, as a menu does.
	pub fn command<S: Store + ?Sized, F: Files>(&mut self, s: &mut S, f: &mut F, c: Command) {
		self.discarded = false;
		self.msg.clear();
		self.run(s, f, c);
		self.sync_layout(s);
		self.scroll(s);
	}

	/// Saves under a new name, as Save As does.
	pub fn save_as<S: Store + ?Sized, F: Files>(
		&mut self,
		s: &mut S,
		f: &mut F,
		name: &[u8],
	) -> bool {
		// The document takes the new name only when the save works.
		let (old, old_len, old_fmt) = (self.name, self.name_len, self.fmt);
		self.set_name(name);
		let ok = self.do_save(s, f);
		if !ok {
			self.name = old;
			self.name_len = old_len;
			self.fmt = old_fmt;
		}
		ok
	}

	/// True once when Help > Contents (or F1) was chosen: the shell opens its help and the message the editor left is cleared.
	pub fn take_help(&mut self) -> bool {
		let h = core::mem::take(&mut self.help);
		if h {
			self.msg.clear();
		}
		h
	}

	/// Puts a message on the status line.
	pub fn set_message(&mut self, text: &str) {
		self.msg.set(text);
	}

	/// The message the last command left for the status line.
	pub fn message(&self) -> &[u8] {
		self.msg.bytes()
	}

	/// Calls `f(index, line)` for each line of the body (or the story being edited), laid out for the app's device.
	pub fn for_each_line<S: ReadStore + ?Sized>(&self, s: &S, f: impl FnMut(u32, &Line)) {
		self.lines(s, f);
	}

	/// Calls `f(position, character, x, width, leader, character format)` for each character of a line.
	pub fn walk_chars<S: ReadStore + ?Sized>(
		&self,
		s: &S,
		line: &Line,
		f: impl FnMut(u32, u8, i32, i32, u8, u16),
	) {
		quill_core::layout::walk_line(
			&self.doc,
			s,
			&mut self.measure.clone(),
			quill_core::doc::StoryKind::Body,
			line,
			f,
		);
	}

	/// The index of the line holding the caret, the line, and the caret's x within it.
	pub fn caret_xy<S: ReadStore + ?Sized>(&self, s: &S) -> Option<(u32, Line, i32)> {
		let (i, l) = self.caret_line(s)?;
		Some((i, l, self.x_of(s, &l, self.caret)))
	}

	/// Chooses the attribute scheme.
	pub fn set_scheme(&mut self, scheme: Scheme) {
		self.scheme = scheme;
	}

	/// True in overtype mode.
	pub fn overtype(&self) -> bool {
		self.overtype
	}

	/// True once the person has chosen Exit.
	pub fn quit(&self) -> bool {
		self.quit
	}

	/// The menu that is open on the text menu bar and its highlighted item, or `None` when no menu is open.
	pub fn menu_open(&self) -> Option<(u8, u8)> {
		match self.mode {
			Mode::Menu { menu, item } => Some((menu, item)),
			_ => None,
		}
	}

	/// The document, for tests.
	pub fn doc(&self) -> &Doc {
		&self.doc
	}

	/// The file name, upper case, empty when untitled.
	pub fn name(&self) -> &[u8] {
		self.name.get(..self.name_len).unwrap_or(&[])
	}

	/// True when there are unsaved changes.
	pub fn modified(&self) -> bool {
		self.modified
	}

	fn set_name(&mut self, n: &[u8]) {
		self.name_len = n.len().min(NAME_MAX);
		for (d, s) in self.name.iter_mut().zip(n.iter()) {
			// DOS and Windows 3.1 keep names in capitals; a build with long names keeps what the file system returned, case and all.
			*d = if cfg!(feature = "long-names") {
				*s
			} else {
				s.to_ascii_uppercase()
			};
		}
	}

	/// Inserts the BMP or PNG file `name` at the caret as one inline picture.
	pub fn insert_picture<S: Store + ?Sized, F: Files>(
		&mut self,
		s: &mut S,
		f: &mut F,
		name: &[u8],
	) -> Result<(), quill_core::Error> {
		let bad = Error::Range;
		let mut head = [0u8; 8];
		f.open(name).ok_or(bad)?.read(&mut head);
		let (kind, w, h) = if head.starts_with(b"BM") {
			let info =
				quill_image::bmp::read_info(&mut f.open(name).ok_or(bad)?).map_err(|_| bad)?;
			(0u8, info.width, info.height)
		} else {
			let (w, h) =
				quill_image::png::read_size(&mut f.open(name).ok_or(bad)?).map_err(|_| bad)?;
			(1u8, w, h)
		};
		let text_w = self.doc.page.text_w();
		let mut src = f.open(name).ok_or(bad)?;
		let idx = self
			.doc
			.add_picture(s, kind, w as u16, h as u16, text_w, |b| src.read(b))?;
		let cf = self.typing_cfmt_at(s, self.caret);
		let pfi = self
			.para_mark(s, self.caret)
			.and_then(|m| self.doc.body.pfmt_at(s, m))
			.unwrap_or(0);
		self.end_typing();
		self.doc
			.body
			.insert_object(s, self.caret, u32::from(idx), cf, pfi, label::TYPING)?;
		self.spell_note_edit(s, self.caret, 0, 1);
		self.caret += 1;
		self.modified = true;
		Ok(())
	}

	/// Puts the current file at the front of the recent list.
	fn push_recent(&mut self) {
		let name = self.name;
		self.recent.push(name.get(..self.name_len).unwrap_or(&[]));
	}

	/// Names the settings section that holds the recent files: `Recent` for DOS and Win16, `Recent32` for the Win32 build, whose
	/// names may be long ones the Win16 build cannot open.
	pub fn set_recent_section(&mut self, section: &'static str) {
		self.recent_section = section;
	}

	/// Calls `f(section, key, value)` for each setting the editor keeps, in the order they are written.
	pub fn each_setting(&self, mut f: impl FnMut(&[u8], &[u8], &[u8])) {
		let sec = self.recent_section.as_bytes();
		for (i, name) in self.recent.iter().enumerate() {
			f(sec, &[b'F', b'i', b'l', b'e', b'1' + i as u8], name);
		}
		if self.spell.lang != 0 {
			f(b"Quill", b"Language", &[b'0' + self.spell.lang]);
		}
		f(
			b"Quill",
			b"AsYouType",
			if self.spell.live.on { b"1" } else { b"0" },
		);
		if self.folder_len > 0 {
			f(
				b"Quill",
				b"Folder",
				self.folder.get(..self.folder_len).unwrap_or(&[]),
			);
		}
		let mut digits = [0u8; 4];
		let mut v = self.zoom;
		let mut i = digits.len();
		while v > 0 && i > 0 {
			i -= 1;
			if let Some(x) = digits.get_mut(i) {
				*x = b'0' + (v % 10) as u8;
			}
			v /= 10;
		}
		f(
			b"Quill",
			b"Zoom",
			if self.zoom == 0 {
				b"Width"
			} else {
				digits.get(i..).unwrap_or(&[])
			},
		);
		for (name, bit) in [
			(&b"Toolbar"[..], 1u8),
			(b"Ruler", 2),
			(b"StatusBar", 4),
			(b"FormattingBar", 8),
		] {
			f(
				b"Quill",
				name,
				if self.chrome & bit != 0 { b"1" } else { b"0" },
			);
		}
	}

	/// Takes in one setting read from a settings file; keys it does not know are ignored.
	pub fn apply_setting(&mut self, section: &[u8], key: &[u8], value: &[u8]) {
		if section.eq_ignore_ascii_case(self.recent_section.as_bytes()) {
			if let [b'F' | b'f', b'i' | b'I', b'l' | b'L', b'e' | b'E', b'1'..=b'4'] = key {
				self.recent.append(value);
			}
			return;
		}
		let is = |k: &[u8]| key.eq_ignore_ascii_case(k);
		if is(b"Language") {
			self.spell.lang = value.first().map_or(0, |b| b.saturating_sub(b'0').min(2));
		} else if is(b"AsYouType") {
			self.set_spell_live(value.first() != Some(&b'0'));
		} else if is(b"Zoom") {
			self.zoom = parse_zoom(value);
		} else if is(b"Folder") {
			let n = value.len().min(NAME_MAX);
			if let (Some(d), Some(sv)) = (self.folder.get_mut(..n), value.get(..n)) {
				d.copy_from_slice(sv);
			}
			self.folder_len = n;
		} else {
			for (name, bit) in [
				(&b"Toolbar"[..], 1u8),
				(b"Ruler", 2),
				(b"StatusBar", 4),
				(b"FormattingBar", 8),
			] {
				if is(name) {
					if value.first() == Some(&b'0') {
						self.chrome &= !bit;
					} else {
						self.chrome |= bit;
					}
				}
			}
		}
	}

	/// Reads `QUILL.INI` through `f`; a missing file is fine.
	pub fn load_settings<F: Files>(&mut self, f: &mut F) {
		let Some(mut r) = f.open(b"QUILL.INI") else {
			return;
		};
		let mut buf = [0u8; 1536];
		let n = r.read(&mut buf);
		let text = buf.get(..n).unwrap_or(&[]);
		let mut section: &[u8] = b"";
		for line in text.split(|b| *b == b'\n') {
			let line = line.strip_suffix(b"\r").unwrap_or(line);
			if let Some(name) = line.strip_prefix(b"[").and_then(|l| l.strip_suffix(b"]")) {
				section = name;
			} else if let Some(eq) = line.iter().position(|b| *b == b'=') {
				if let (Some(k), Some(v)) = (line.get(..eq), line.get(eq + 1..)) {
					self.apply_setting(section, k, v);
				}
			}
		}
	}

	/// Writes `QUILL.INI` whole: the recent files, then the other settings, as the SPEC's settings file lists them.
	pub fn save_settings<F: Files>(&self, f: &mut F) {
		let Some(mut w) = f.create(b"QUILL.INI") else {
			return;
		};
		let mut section: [u8; 16] = [0; 16];
		let mut slen = usize::MAX;
		self.each_setting(|sec, key, value| {
			if section.get(..slen) != Some(sec) {
				let _ = w.write(b"[");
				let _ = w.write(sec);
				let _ = w.write(b"]\r\n");
				slen = sec.len().min(section.len());
				if let Some(d) = section.get_mut(..slen) {
					d.copy_from_slice(sec.get(..slen).unwrap_or(&[]));
				}
			}
			let _ = w.write(key);
			let _ = w.write(b"=");
			let _ = w.write(value);
			let _ = w.write(b"\r\n");
		});
		f.close(w);
	}

	/// Sets the platform's line of the About box (the mode and the free memory).
	pub fn set_about(&mut self, text: &str) {
		self.about.set(text);
	}

	/// The platform line set by the shell, as shown in About.
	pub fn about_platform(&self) -> &str {
		core::str::from_utf8(self.about.bytes()).unwrap_or("")
	}

	/// The recent files, for a native menu.
	pub fn recent_files(&self) -> impl Iterator<Item = &[u8]> {
		self.recent.iter()
	}

	/// Opens a file by content (a RIFF `QLD1` header is native, anything else is plain text) into a fresh document, and
	/// keeps the current one when it cannot be read.
	pub fn open<S: Store + ?Sized, F: Files>(
		&mut self,
		s: &mut S,
		f: &mut F,
		name: &[u8],
	) -> Result<(), FormatError> {
		let r = self.open_file(s, f, name);
		self.sync_layout(s);
		r
	}

	/// Reads the file into a new document and swaps it in. A separate function, so the new document's 5 KB is off the stack when the
	/// layout cache is made.
	#[inline(never)]
	fn open_file<S: Store + ?Sized, F: Files>(
		&mut self,
		s: &mut S,
		f: &mut F,
		name: &[u8],
	) -> Result<(), FormatError> {
		let mut head = [0u8; 12];
		let n = f.open(name).ok_or(FormatError::BadFile)?.read(&mut head);
		let native = n == 12 && head.starts_with(b"RIFF") && head.get(8..12) == Some(b"QLD1");
		let rich = n >= 5 && head.starts_with(b"{\\rtf");
		let mut r = f.open(name).ok_or(FormatError::BadFile)?;
		let mut fresh = Doc::new();
		let res = if native {
			qld::read(&mut fresh, s, &mut r)
		} else if rich {
			rtf::read(&mut fresh, s, &mut r)
		} else {
			text::read(&mut fresh, s, &mut r)
		};
		if let Err(e) = res {
			fresh.reset(s);
			return Err(e);
		}
		self.doc.reset(s);
		core::mem::swap(&mut self.doc, &mut fresh);
		safe::clean_leftover(f, name);
		self.reflow.invalidate();
		self.spell_forget();
		self.caret = 0;
		self.top = 0;
		self.modified = false;
		self.typing = false;
		self.set_name(name);
		self.push_recent();
		self.fmt = if native {
			0
		} else if rich {
			1
		} else {
			2
		};
		Ok(())
	}

	/// Saves under the current name: text for `.TXT`, native otherwise.
	pub fn save<S: Store + ?Sized, F: Files>(
		&mut self,
		s: &mut S,
		f: &mut F,
	) -> Result<(), SaveError> {
		let name = self.name;
		let name = name.get(..self.name_len).unwrap_or(&[]);
		let ext = name.len().checked_sub(4).and_then(|i| name.get(i..));
		let is = |e: &[u8]| ext.is_some_and(|x| x.eq_ignore_ascii_case(e));
		let kind = if is(b".QLD") {
			0
		} else if is(b".RTF") {
			1
		} else if is(b".TXT") {
			2
		} else {
			self.fmt
		};
		let doc = &self.doc;
		match safe::write_replace(f, name, |w| match kind {
			1 => rtf::write(doc, s, w),
			2 => text::write(doc, s, w),
			_ => qld::write(doc, s, w),
		}) {
			safe::Saved::Done => {}
			safe::Saved::Failed => return Err(SaveError::Failed),
			safe::Saved::Kept => return Err(SaveError::Kept),
		}
		self.fmt = kind;
		self.push_recent();
		self.modified = false;
		Ok(())
	}

	/// Every line of the body, from the first, with its index.
	pub(crate) fn lines<S: ReadStore + ?Sized>(&self, s: &S, mut f: impl FnMut(u32, &Line)) {
		self.lines_from(s, 0, |i, l| {
			f(i, l);
			true
		});
	}

	/// The index and line holding the caret.
	pub(crate) fn caret_line<S: ReadStore + ?Sized>(&self, s: &S) -> Option<(u32, Line)> {
		self.line_for(s, self.caret)
	}

	/// The x of each character of a line, calling `f(position, character, x, width, leader)`. The leader is 0 except for a tab
	/// that has one (1 dots, 2 hyphens, 3 underline).
	pub(crate) fn walk<S: ReadStore + ?Sized>(
		&self,
		s: &S,
		line: &Line,
		mut f: impl FnMut(u32, u8, i32, i32, u8),
	) {
		quill_core::layout::walk_line(
			&self.doc,
			s,
			&mut self.measure.clone(),
			quill_core::doc::StoryKind::Body,
			line,
			|p, c, x, w, l, _| f(p, c, x, w, l),
		);
	}

	/// The x of position `pos` in `line`.
	pub(crate) fn x_of<S: ReadStore + ?Sized>(&self, s: &S, line: &Line, pos: u32) -> i32 {
		let mut x = line.x;
		let mut end = line.x;
		self.walk(s, line, |p, _, cx, w, _| {
			if p == pos {
				x = cx;
			}
			end = cx + w;
		});
		if pos >= line.start + line.len {
			end
		} else {
			x
		}
	}

	fn pos_at_x<S: ReadStore + ?Sized>(&self, s: &S, line: &Line, want: i32) -> u32 {
		let mut best = (line.start, i32::MAX);
		self.walk(s, line, |p, _, x, _, _| {
			let d = (x - want).abs();
			if d < best.1 {
				best = (p, d);
			}
		});
		let end = self.x_of(s, line, line.start + line.len);
		if (end - want).abs() < best.1 {
			best = (line.start + line.len, 0);
		}
		best.0
	}

	fn line_count<S: ReadStore + ?Sized>(&self, s: &S) -> u32 {
		self.lines_total(s)
	}

	fn end_typing(&mut self) {
		if self.typing {
			self.doc.body.end_group();
			self.typing = false;
		}
	}

	/// The position of the paragraph mark that ends the paragraph holding `pos`.
	fn para_mark<S: ReadStore + ?Sized>(&self, s: &S, pos: u32) -> Option<u32> {
		let t = &self.doc.body;
		let mut at = pos;
		let mut buf = [0u8; 64];
		while at < t.len() {
			let n = t.read(s, at, &mut buf);
			if n == 0 {
				break;
			}
			for (i, b) in buf.iter().take(n).enumerate() {
				if *b == 0x0D {
					return Some(at + i as u32);
				}
			}
			at += n as u32;
		}
		None
	}

	/// The selection as an ordered range, when it holds anything.
	pub fn selection(&self) -> Option<(u32, u32)> {
		let a = self.anchor?;
		let (lo, hi) = if a < self.caret {
			(a, self.caret)
		} else {
			(self.caret, a)
		};
		(lo != hi).then_some((lo, hi.min(self.doc.body.len().saturating_sub(1))))
	}

	/// Removes the selection (never the final paragraph mark) and puts the caret where it was. Returns true when it did.
	fn delete_selection<S: Store + ?Sized>(&mut self, s: &mut S) -> bool {
		let Some((lo, hi)) = self.selection() else {
			self.anchor = None;
			return false;
		};
		self.anchor = None;
		if hi > lo && self.doc.body.delete(s, lo, hi - lo, label::DELETE).is_ok() {
			self.spell_note_edit(s, lo, hi - lo, 0);
			self.caret = lo;
			self.modified = true;
			true
		} else {
			false
		}
	}

	fn insert<S: Store + ?Sized>(&mut self, s: &mut S, c: u8) {
		self.insert_next(s, c, None);
	}

	/// Inserts `c` at the caret. After a paragraph mark, `next` is the preset the new paragraph (the one the caret is in) takes, in the
	/// same undo step as the mark.
	fn insert_next<S: Store + ?Sized>(&mut self, s: &mut S, c: u8, next: Option<usize>) {
		if self.selection().is_some() {
			self.end_typing();
			self.doc.body.begin_group(label::TYPING);
			self.typing = true;
			self.delete_selection(s);
		}
		self.anchor = None;
		let t = &self.doc.body;
		let cfmt = self
			.typing_fmt
			.unwrap_or_else(|| self.typing_cfmt_at(s, self.caret));
		let pfmt = self
			.para_mark(s, self.caret)
			.and_then(|m| t.pfmt_at(s, m))
			.unwrap_or(0);
		if !self.typing {
			self.doc.body.begin_group(label::TYPING);
			self.typing = true;
		}
		self.want_x = -1;
		// Overtype replaces the character after the caret, except a paragraph mark.
		if self.overtype
			&& c != 0x0D
			&& self.caret + 1 < self.doc.body.len()
			&& self.doc.body.read_byte(s, self.caret) != Some(0x0D)
			&& self
				.doc
				.body
				.delete(s, self.caret, 1, label::TYPING)
				.is_ok()
		{
			self.spell_note_edit(s, self.caret, 1, 0);
		}
		match self
			.doc
			.body
			.insert(s, self.caret, &[c], cfmt, pfmt, label::TYPING)
		{
			Ok(()) => {
				self.spell_note_edit(s, self.caret, 0, 1);
				self.caret += 1;
				self.modified = true;
			}
			Err(Error::NoMemory) => self
				.msg
				.set("There is not enough memory to complete this operation."),
			Err(_) => self.msg.set("The document is too large for Quill."),
		}
		if c == 0x0D {
			if let Some(n) = next {
				let at = self.caret;
				self.apply_preset(s, n, at, at);
			}
			self.end_typing();
			self.typing_fmt = None;
		}
	}

	/// The character format text typed at `pos` takes (CORE-40): the format of the character before it, except at a paragraph's start,
	/// where it is the format of the character at the caret (the paragraph's own mark when it is empty), never the previous paragraph's.
	pub(crate) fn typing_cfmt_at<S: ReadStore + ?Sized>(&self, s: &S, pos: u32) -> u16 {
		let t = &self.doc.body;
		let at_start = pos == 0 || t.read_byte(s, pos - 1) == Some(0x0D);
		let from = if at_start { pos } else { pos - 1 };
		t.cfmt_at(s, from)
			.or_else(|| t.cfmt_at(s, pos.saturating_sub(1)))
			.unwrap_or(0)
	}

	fn delete<S: Store + ?Sized>(&mut self, s: &mut S, pos: u32) {
		self.end_typing();
		self.want_x = -1;
		if self.selection().is_some() {
			self.delete_selection(s);
			return;
		}
		if self.doc.body.delete(s, pos, 1, label::DELETE).is_ok() {
			self.spell_note_edit(s, pos, 1, 0);
			self.caret = pos;
			self.modified = true;
		}
	}

	/// Takes the paragraph out of its list: no bullet or number, no hanging indent.
	fn end_list<S: Store + ?Sized>(&mut self, s: &mut S) {
		self.para_command(s, |p| PFmt {
			flags: p.flags & !5,
			left: 0,
			first: 0,
			level: 0,
			..p
		});
	}

	/// Backspace. At the start of a bullet or numbered paragraph it takes the paragraph out of the list and nothing more, as in Word
	/// and Write; at the start of any other paragraph it joins it to the one before; elsewhere it removes the character before the caret.
	fn backspace<S: Store + ?Sized>(&mut self, s: &mut S) {
		if self.caret != self.para_start(s, self.caret) {
			self.delete(s, self.caret - 1);
			return;
		}
		self.end_typing();
		self.want_x = -1;
		if self.para_pfmt(s, self.caret).flags & 5 != 0 {
			self.end_list(s);
		} else if self.caret > 0 {
			self.join_paragraphs(s, self.caret - 1);
		}
	}

	/// Delete: at the end of a paragraph it joins the next one to it; elsewhere it removes the character after the caret.
	fn delete_forward<S: Store + ?Sized>(&mut self, s: &mut S) {
		if self.doc.body.read_byte(s, self.caret) == Some(0x0D) {
			self.join_paragraphs(s, self.caret);
		} else {
			self.delete(s, self.caret);
		}
	}

	/// Removes the paragraph mark at `mark`, joining the paragraph that ends there to the next one. The joined paragraph keeps the
	/// first one's format (so a bullet's text is never swallowed by the paragraph below it, and a paragraph never takes a bullet from
	/// the one it is joined to); when the first is empty the joined paragraph is the second, unchanged. One undo step.
	fn join_paragraphs<S: Store + ?Sized>(&mut self, s: &mut S, mark: u32) {
		self.end_typing();
		self.want_x = -1;
		let first = self.para_start(s, mark);
		let keep = (first < mark)
			.then(|| self.doc.body.pfmt_at(s, mark))
			.flatten();
		// An empty paragraph joined to the one above leaves the upper paragraph's mark as it was, so Enter then Backspace is a round trip.
		let mark_cf = (first < mark && self.doc.body.read_byte(s, mark + 1) == Some(0x0D))
			.then(|| self.doc.body.cfmt_at(s, mark))
			.flatten();
		self.doc.body.begin_group(label::DELETE);
		if self.doc.body.delete(s, mark, 1, label::DELETE).is_ok() {
			self.spell_note_edit(s, mark, 1, 0);
			self.caret = mark;
			self.modified = true;
			if let Some(cf) = mark_cf {
				let _ = self.doc.body.set_cfmt(s, mark, 1, cf, label::PARA_FORMAT);
			}
			if let (Some(pf), Some(end)) = (keep, self.para_mark(s, first)) {
				let _ = self
					.doc
					.body
					.set_pfmt(s, first, end + 1 - first, pf, label::PARA_FORMAT);
			}
		}
		self.doc.body.end_group();
	}

	fn move_vertical<S: ReadStore + ?Sized>(&mut self, s: &S, delta: i32, keep_x: bool) {
		let Some((idx, line)) = self.caret_line(s) else {
			return;
		};
		if !keep_x {
			self.want_x = self.x_of(s, &line, self.caret);
		}
		let last = self.line_count(s).saturating_sub(1);
		let target = (idx as i32 + delta).clamp(0, last as i32) as u32;
		if let Some(l) = self.line_at(s, target) {
			self.caret = self.pos_at_x(s, &l, self.want_x);
		}
	}

	/// Swaps the story being edited into `doc.body` (or back), so every editing command works on it unchanged.
	fn swap_view<S: Store + ?Sized>(&mut self, s: &mut S, kind: StoryKind) {
		match kind {
			StoryKind::Body => {}
			StoryKind::Header => core::mem::swap(&mut self.doc.body, &mut self.doc.header),
			StoryKind::Footer => core::mem::swap(&mut self.doc.body, &mut self.doc.footer),
			StoryKind::FirstHeader => {
				core::mem::swap(&mut self.doc.body, &mut self.doc.first_header)
			}
			StoryKind::FirstFooter => {
				core::mem::swap(&mut self.doc.body, &mut self.doc.first_footer)
			}
		}
		self.reflow.invalidate();
		let _ = s;
	}

	pub(crate) fn set_view<S: Store + ?Sized>(&mut self, s: &mut S, to: u8) {
		let cur = StoryKind::from_u8(self.view).unwrap_or(StoryKind::Body);
		if self.view != 0 {
			// Back to the body: a story holding only its paragraph mark is empty.
			if self.doc.body.len() == 1 && self.doc.body.read_byte(s, 0) == Some(0x0D) {
				self.doc.body.clear(s);
			}
			self.swap_view(s, cur);
			let (c, t) = self.saved_view;
			self.caret = c;
			self.top = t;
		}
		self.anchor = None;
		self.view = 0;
		if to != 0 {
			self.saved_view = (self.caret, self.top);
			let k = StoryKind::from_u8(to).unwrap_or(StoryKind::Header);
			self.swap_view(s, k);
			if self.doc.body.is_empty() {
				let _ = self.doc.body.load(s, b"\r", 0, 0);
			}
			self.view = to;
			self.caret = 0;
			self.top = 0;
		}
	}

	fn run<S: Store + ?Sized, F: Files>(&mut self, s: &mut S, f: &mut F, c: Command) {
		self.end_typing();
		if let Some(d) = self.native_request(c) {
			self.dialog = Some(d);
			return;
		}
		if self.view != 0
			&& matches!(
				c,
				Command::New | Command::Open | Command::Save | Command::SaveAs | Command::Exit
			) {
			self.set_view(s, 0);
		}
		match c {
			Command::New | Command::Open | Command::Exit | Command::Recent(_)
				if self.modified && self.pending != Some(c) =>
			{
				self.mode = Mode::Confirm(c, 0);
			}
			Command::New => {
				self.pending = None;
				self.doc.reset(s);
				if self.reset(s).is_err() {
					self.msg
						.set("There is not enough memory to complete this operation.");
				}
			}
			Command::Open => {
				self.pending = None;
				self.mode = Mode::Prompt {
					kind: Prompt::Open,
					buf: [0; NAME_MAX],
					len: 0,
				};
			}
			Command::Save => {
				if self.name_len == 0 {
					self.mode = Mode::Prompt {
						kind: Prompt::SaveAs,
						buf: [0; NAME_MAX],
						len: 0,
					};
				} else {
					self.do_save(s, f);
				}
			}
			Command::SaveAs => {
				self.mode = Mode::Prompt {
					kind: Prompt::SaveAs,
					buf: [0; NAME_MAX],
					len: 0,
				};
			}
			Command::Exit => {
				// Exiting after a No completes the discard.
				if self.pending.take().is_some() {
					self.modified = false;
				}
				self.quit = true;
			}
			Command::Regular => self.char_command(
				s,
				|c| CFmt {
					flags: 0,
					vpos: 0,
					..c
				},
				None,
			),
			Command::Bold => self.char_command(
				s,
				|c| CFmt {
					flags: c.flags ^ cflag::BOLD,
					..c
				},
				Some(cflag::BOLD),
			),
			Command::Italic => self.char_command(
				s,
				|c| CFmt {
					flags: c.flags ^ cflag::ITALIC,
					..c
				},
				Some(cflag::ITALIC),
			),
			Command::Underline => self.char_command(
				s,
				|c| CFmt {
					flags: c.flags ^ cflag::UNDERLINE,
					..c
				},
				Some(cflag::UNDERLINE),
			),
			Command::Strike => self.char_command(
				s,
				|c| CFmt {
					flags: c.flags ^ cflag::STRIKE,
					..c
				},
				Some(cflag::STRIKE),
			),
			Command::Super => self.char_command(
				s,
				|c| CFmt {
					vpos: if c.vpos == 1 { 0 } else { 1 },
					..c
				},
				None,
			),
			Command::Sub => self.char_command(
				s,
				|c| CFmt {
					vpos: if c.vpos == 2 { 0 } else { 2 },
					..c
				},
				None,
			),
			Command::Align(a) => self.para_command(s, |p| PFmt { align: a, ..p }),
			Command::Spacing(n) => self.para_command(s, |p| PFmt { spacing: n, ..p }),
			Command::Bullets | Command::Numbering => {
				let bullet = c == Command::Bullets;
				let mut def = ListDef::default();
				for (i, lv) in def.levels.iter_mut().enumerate() {
					*lv = ListLevel {
						style: 0,
						start: 1,
						indent: 360 * (i as i32 + 1),
					};
				}
				let li = self.doc.lists.intern(s, def).unwrap_or(0);
				self.para_command(s, move |p| {
					let bit = if bullet { 1 } else { 4 };
					if p.flags & bit != 0 {
						PFmt {
							flags: p.flags & !5,
							left: 0,
							first: 0,
							level: 0,
							..p
						}
					} else {
						let fresh = p.left == 0;
						PFmt {
							flags: (p.flags & !5) | bit,
							list: li,
							left: if fresh { 360 } else { p.left },
							first: if fresh { -360 } else { p.first },
							..p
						}
					}
				});
			}
			Command::LevelUp | Command::LevelDown => {
				let up = c == Command::LevelUp;
				self.para_command(s, move |p| {
					if p.flags & 5 == 0 {
						return p;
					}
					let level = if up {
						(p.level + 1).min(8)
					} else {
						p.level.saturating_sub(1)
					};
					let left = if up {
						p.left + 360
					} else {
						(p.left - 360).max(360)
					};
					PFmt { level, left, ..p }
				});
			}
			Command::View(n) => self.set_view(s, n),
			Command::FirstPage => {
				self.doc.page.flags ^= 1;
				self.modified = true;
			}
			Command::Field(k) => {
				// A field goes in at the caret, in the body or in a header or footer, replacing a selection.
				if self.selection().is_some() {
					self.doc.body.begin_group(label::TYPING);
					self.delete_selection(s);
					self.doc.body.end_group();
				}
				self.anchor = None;
				let cf = self.typing_cfmt_at(s, self.caret);
				let pfi = self
					.para_mark(s, self.caret)
					.and_then(|m| self.doc.body.pfmt_at(s, m))
					.unwrap_or(0);
				if self
					.doc
					.body
					.insert_object(s, self.caret, u32::from(k) << 16, cf, pfi, label::TYPING)
					.is_ok()
				{
					self.spell_note_edit(s, self.caret, 0, 1);
					self.caret += 1;
					self.modified = true;
				}
			}
			Command::Cut | Command::Copy | Command::Paste | Command::SelectAll => {
				self.clip_command(s, c)
			}
			Command::Spelling => {
				if self.spell_begin(s, f) {
					self.open_spell_dialog(s);
				}
			}
			Command::Fonts => self.open_fonts(s),
			Command::Tabs => self.open_tabs(),
			Command::About => self.mode = Mode::About,
			Command::Help => {
				self.help = true;
				// Shells that show help take the request and clear this.
				self.set_message("Help is in the Windows builds for now");
			}
			Command::Picture => {
				self.mode = Mode::Prompt {
					kind: Prompt::Picture,
					buf: [0; NAME_MAX],
					len: 0,
				}
			}
			Command::Chrome(bit) => self.chrome ^= bit,
			Command::SpellAsYouType => {
				self.set_spell_live(!self.spell.live.on);
				self.msg.set(if self.spell.live.on {
					"Spelling is checked as you type."
				} else {
					"Spelling is not checked as you type."
				});
			}
			Command::Zoom(z) => self.set_zoom(z),
			Command::Preview | Command::PrintSetup => {
				self.msg.set("That is not available in this version.")
			}
			Command::Print => {
				self.mode = Mode::Prompt {
					kind: Prompt::Print,
					buf: [0; NAME_MAX],
					len: 0,
				}
			}
			Command::Recent(n) => {
				self.pending = None;
				let mut name = [0u8; NAME_MAX];
				let len = self.recent.get(usize::from(n)).map_or(0, <[u8]>::len);
				if let Some(src) = self.recent.get(usize::from(n)) {
					name.get_mut(..len).unwrap_or(&mut []).copy_from_slice(src);
				}
				let name = name.get(..len).unwrap_or(&[]);
				if len == 0 {
				} else if f.open(name).is_none() {
					// A file that is gone leaves the list, and says so.
					self.recent.remove(name);
					let mut short = [0u8; 56];
					let n = recent::shorten_path(name, short.len(), &mut short);
					self.msg.clear();
					self.msg.push(b"Quill cannot find ");
					self.msg.push(short.get(..n).unwrap_or(&[]));
					self.msg.push(b".");
				} else if self.open(s, f, name).is_err() {
					self.msg.set("Quill cannot open that file.");
				}
			}
			Command::Find => self.open_find(false),
			Command::Replace => self.open_find(true),
			Command::FindNext => {
				if let Some(st) = self.last_find {
					self.find_next(s, &st);
				} else {
					self.open_find(false);
				}
			}
			Command::Undo => {
				if self.doc.body.undo(s).unwrap_or(false) {
					self.spell_forget();
					self.modified = true;
					self.caret = self.caret.min(self.doc.body.len().saturating_sub(1));
				}
			}
			Command::Redo => {
				if self.doc.body.redo(s).unwrap_or(false) {
					self.spell_forget();
					self.modified = true;
					self.caret = self.caret.min(self.doc.body.len().saturating_sub(1));
				}
			}
		}
	}

	/// Applies a character-format change to the selection, or sets the format typing takes next when nothing is selected.
	/// `flag` makes the change a toggle: when every character already has it, it is switched off for all.
	fn char_command<S: Store + ?Sized>(
		&mut self,
		s: &mut S,
		f: impl Fn(CFmt) -> CFmt,
		flag: Option<u16>,
	) {
		let Some((lo, hi)) = self.selection() else {
			let cur = self
				.typing_fmt
				.unwrap_or_else(|| self.typing_cfmt_at(s, self.caret));
			if let Some(c) = self.doc.cfmts.get(s, cur) {
				self.typing_fmt = self.doc.cfmts.intern(s, f(c)).ok();
			}
			return;
		};
		// A toggle that every character already has switches off; otherwise it switches on for all.
		let mut all = true;
		if let Some(bit) = flag {
			let mut cfs = [0u16; 32];
			let mut n = 0;
			self.doc.body.for_each_run(s, lo, hi - lo, |_, p| {
				if let Some(slot) = cfs.get_mut(n) {
					*slot = p.cfmt;
					n += 1;
				}
			});
			for cf in cfs.iter().take(n) {
				if self
					.doc
					.cfmts
					.get(s, *cf)
					.is_some_and(|c| c.flags & bit == 0)
				{
					all = false;
				}
			}
		}
		self.doc.body.begin_group(label::CHAR_FORMAT);
		let mut at = lo;
		while at < hi {
			let mut runs = [(0u32, 0u32, 0u16); 32];
			let mut n = 0;
			self.doc.body.for_each_run(s, at, hi - at, |pos, p| {
				if let Some(slot) = runs.get_mut(n) {
					*slot = (pos, p.len, p.cfmt);
					n += 1;
				}
			});
			if n == 0 {
				break;
			}
			for &(pos, len, cf) in runs.iter().take(n) {
				let Some(c) = self.doc.cfmts.get(s, cf) else {
					continue;
				};
				let mut new = f(c);
				if let Some(bit) = flag {
					// Make the toggle uniform: on for everything unless everything had it.
					new.flags = if all { c.flags & !bit } else { c.flags | bit };
				}
				if let Ok(idx) = self.doc.cfmts.intern(s, new) {
					let _ = self.doc.body.set_cfmt(s, pos, len, idx, label::CHAR_FORMAT);
				}
				at = pos + len;
			}
		}
		self.doc.body.end_group();
		self.modified = true;
	}

	/// Applies a paragraph-format change to every paragraph the selection or the caret touches.
	fn para_command<S: Store + ?Sized>(&mut self, s: &mut S, f: impl Fn(PFmt) -> PFmt) {
		let (lo, hi) = self.selection().unwrap_or((self.caret, self.caret));
		let t_len = self.doc.body.len();
		let mut start = self.para_start(s, lo);
		self.doc.body.begin_group(label::PARA_FORMAT);
		while let Some(mark) = self.para_mark(s, start) {
			let old = self
				.doc
				.body
				.pfmt_at(s, mark)
				.and_then(|i| self.doc.pfmts.get(s, i))
				.unwrap_or(PFmt::DEFAULT);
			if let Ok(idx) = self.doc.pfmts.intern(s, f(old)) {
				let _ = self
					.doc
					.body
					.set_pfmt(s, start, mark + 1 - start, idx, label::PARA_FORMAT);
			}
			start = mark + 1;
			if start > hi || start >= t_len {
				break;
			}
		}
		self.doc.body.end_group();
		self.modified = true;
	}

	/// The paragraph format of the paragraph holding `pos`.
	pub(crate) fn para_pfmt<S: ReadStore + ?Sized>(&self, s: &S, pos: u32) -> PFmt {
		self.para_mark(s, pos)
			.and_then(|m| self.doc.body.pfmt_at(s, m))
			.and_then(|i| self.doc.pfmts.get(s, i))
			.unwrap_or(PFmt::DEFAULT)
	}

	/// The position where the paragraph holding `pos` starts.
	pub(crate) fn para_start<S: ReadStore + ?Sized>(&self, s: &S, pos: u32) -> u32 {
		let t = &self.doc.body;
		let mut end = pos;
		let mut buf = [0u8; 64];
		while end > 0 {
			let from = end.saturating_sub(64);
			let n = t.read(
				s,
				from,
				buf.get_mut(..(end - from) as usize).unwrap_or(&mut []),
			);
			for i in (0..n).rev() {
				if buf.get(i) == Some(&0x0D) {
					return from + i as u32 + 1;
				}
			}
			end = from;
		}
		0
	}

	fn do_save<S: Store + ?Sized, F: Files>(&mut self, s: &mut S, f: &mut F) -> bool {
		match self.save(s, f) {
			Ok(()) => {
				if let Some(p) = self.pending.take() {
					self.run(s, f, p);
				}
				true
			}
			Err(e) => {
				if e == SaveError::Kept {
					let name = self.name;
					let temp = safe::TempName::of(name.get(..self.name_len).unwrap_or(&[]));
					self.msg
						.set_quoted(flow::words::KEPT_NEW_COPY, temp.file(), ".");
				} else {
					self.msg.set(flow::words::CANNOT_SAVE);
				}
				self.pending = None;
				false
			}
		}
	}

	/// Handles one key.
	pub fn key<S: Store + ?Sized, F: Files>(&mut self, s: &mut S, f: &mut F, k: Key) {
		self.discarded = false;
		self.msg.set("");
		match self.mode {
			Mode::Edit => self.key_edit(s, f, k),
			Mode::Menu { menu, item } => self.key_menu(s, f, k, menu, item),
			Mode::Prompt { kind, buf, len } => self.key_prompt(s, f, k, kind, buf, len),
			Mode::About => {
				if matches!(k, Key::Enter | Key::Esc | Key::Char(b' ')) {
					self.mode = Mode::Edit;
				}
			}
			Mode::Find(st) => self.key_find(s, k, st),
			Mode::Form(fm) => self.key_form(s, k, fm),
			Mode::Spell(d) => self.key_spell(s, f, k, d),
			Mode::Confirm(c, focus) => {
				let answer = match k {
					Key::Char(b'y' | b'Y') => Some(0),
					Key::Char(b'n' | b'N') => Some(1),
					Key::Esc => Some(2),
					Key::Enter => Some(focus),
					Key::Tab | Key::Right => {
						self.mode = Mode::Confirm(c, (focus + 1) % 3);
						None
					}
					Key::Left => {
						self.mode = Mode::Confirm(c, (focus + 2) % 3);
						None
					}
					_ => None,
				};
				match answer {
					Some(0) => {
						self.mode = Mode::Edit;
						self.pending = Some(c);
						if self.name_len == 0 {
							self.mode = Mode::Prompt {
								kind: Prompt::SaveAs,
								buf: [0; NAME_MAX],
								len: 0,
							};
						} else {
							self.do_save(s, f);
						}
					}
					Some(1) => {
						// The changes go only when `c` completes: a cancelled Open leaves them unsaved.
						self.mode = Mode::Edit;
						self.pending = Some(c);
						self.run(s, f, c);
						self.pending = None;
					}
					Some(_) => self.mode = Mode::Edit,
					None => {}
				}
			}
		}
		self.sync_layout(s);
		self.scroll(s);
	}

	fn move_caret<S: ReadStore + ?Sized>(&mut self, s: &S, d: Dir) {
		let len = self.doc.body.len();
		match d {
			Dir::Left => {
				self.caret = self.caret.saturating_sub(1);
				self.want_x = -1;
			}
			Dir::Right => {
				self.caret = (self.caret + 1).min(len.saturating_sub(1));
				self.want_x = -1;
			}
			Dir::Up | Dir::Down | Dir::PageUp | Dir::PageDown => {
				let page = self.rows as i32 - 1;
				let delta = match d {
					Dir::Up => -1,
					Dir::Down => 1,
					Dir::PageUp => -page,
					_ => page,
				};
				let keep = self.want_x >= 0;
				self.move_vertical(s, delta, keep);
			}
			Dir::Home | Dir::End => {
				if let Some((_, l)) = self.caret_line(s) {
					self.caret = if d == Dir::Home {
						l.start
					} else {
						l.start + l.len
					};
				}
				self.want_x = -1;
			}
		}
	}

	#[inline(never)] // keeps each mode's locals out of `key`'s frame (Win16 stacks are small)
	fn key_edit<S: Store + ?Sized, F: Files>(&mut self, s: &mut S, f: &mut F, k: Key) {
		let len = self.doc.body.len();
		let dir = match k {
			Key::Left => Some(Dir::Left),
			Key::Right => Some(Dir::Right),
			Key::Up => Some(Dir::Up),
			Key::Down => Some(Dir::Down),
			Key::Home => Some(Dir::Home),
			Key::End => Some(Dir::End),
			Key::PageUp => Some(Dir::PageUp),
			Key::PageDown => Some(Dir::PageDown),
			Key::Select(d) => Some(d),
			_ => None,
		};
		if let Some(d) = dir {
			self.end_typing();
			self.typing_fmt = None;
			if matches!(k, Key::Select(_)) || self.extend {
				if self.anchor.is_none() {
					self.anchor = Some(self.caret);
				}
			} else {
				self.anchor = None;
			}
			self.move_caret(s, d);
			return;
		}
		match k {
			Key::Char(c) => self.insert(s, c),
			Key::Tab => self.insert(s, 0x09),
			Key::Enter => {
				let pf = self.para_pfmt(s, self.caret);
				let at_end = self.doc.body.read_byte(s, self.caret) == Some(0x0D);
				let empty = at_end
					&& (self.caret == 0
						|| self.doc.body.read_byte(s, self.caret - 1) == Some(0x0D));
				if pf.flags & 5 != 0 && empty && self.selection().is_none() {
					self.end_list(s);
				} else {
					// At the end of a paragraph that is exactly a preset, the new paragraph takes that preset's next one.
					let next = if at_end && self.selection().is_none() {
						self.style_of(s)
							.map(|n| (n, chrome::next_preset(n)))
							.filter(|(n, next)| n != next)
							.map(|(_, next)| next)
					} else {
						None
					};
					self.insert_next(s, 0x0D, next);
				}
			}
			Key::Backspace if self.selection().is_some() => self.delete(s, self.caret),
			Key::Backspace => self.backspace(s),
			Key::Delete if self.selection().is_some() => self.delete(s, self.caret),
			Key::Delete if self.caret + 1 < len => self.delete_forward(s),
			Key::CtrlHome | Key::CtrlEnd => {
				self.end_typing();
				self.typing_fmt = None;
				if self.extend {
					if self.anchor.is_none() {
						self.anchor = Some(self.caret);
					}
				} else {
					self.anchor = None;
				}
				self.caret = if k == Key::CtrlHome {
					0
				} else {
					len.saturating_sub(1)
				};
				self.want_x = -1;
			}
			Key::Esc => {
				self.anchor = None;
				self.extend = false;
			}
			Key::F(8) => {
				self.extend = !self.extend;
				self.anchor = if self.extend { Some(self.caret) } else { None };
			}
			Key::Insert => self.overtype = !self.overtype,
			Key::F(5) => self.run(s, f, Command::Regular),
			Key::F(3) => self.run(s, f, Command::FindNext),
			Key::F(1) => self.run(s, f, Command::Help),
			Key::F(7) => self.run(s, f, Command::Spelling),
			Key::Ctrl(b'f') => self.run(s, f, Command::Find),
			Key::Ctrl(b'h') => self.run(s, f, Command::Replace),
			Key::F(10) if !self.native => {
				self.end_typing();
				self.mode = Mode::Menu { menu: 0, item: 0 };
			}
			Key::Alt(8) => self.run(s, f, Command::Undo),
			Key::Alt(c) if !self.native => {
				if let Some(menu) = paint::menu_for_mnemonic(c) {
					self.end_typing();
					self.mode = Mode::Menu { menu, item: 0 };
				}
			}
			Key::Ctrl(b's') => self.run(s, f, Command::Save),
			Key::Ctrl(b'o') => self.run(s, f, Command::Open),
			Key::Ctrl(b'p') => self.run(s, f, Command::Print),
			Key::Ctrl(b'n') => self.run(s, f, Command::New),
			Key::Ctrl(b'q') => self.run(s, f, Command::Exit),
			Key::Ctrl(b'x') => self.run(s, f, Command::Cut),
			Key::Ctrl(b'c') => self.run(s, f, Command::Copy),
			Key::Ctrl(b'v') => self.run(s, f, Command::Paste),
			Key::Ctrl(b'a') => self.run(s, f, Command::SelectAll),
			Key::Ctrl(b'z') => self.run(s, f, Command::Undo),
			Key::Ctrl(b'y') => self.run(s, f, Command::Redo),
			Key::Ctrl(b'b') => self.run(s, f, Command::Bold),
			Key::Ctrl(b'i') => self.run(s, f, Command::Italic),
			Key::Ctrl(b'u') => self.run(s, f, Command::Underline),
			Key::Ctrl(b'l') => self.run(s, f, Command::Align(0)),
			Key::Ctrl(b'e') => self.run(s, f, Command::Align(1)),
			Key::Ctrl(b'r') => self.run(s, f, Command::Align(2)),
			Key::Ctrl(b'j') => self.run(s, f, Command::Align(3)),
			_ => {}
		}
	}

	/// Runs what place `at` of the open menu holds: an item's command, or the recent file listed there.
	fn run_slot<S: Store + ?Sized, F: Files>(
		&mut self,
		s: &mut S,
		f: &mut F,
		items: &[paint::Item],
		recents: usize,
		at: usize,
	) {
		match paint::slot(items.len(), recents, at) {
			paint::Slot::Item(i) => {
				if let Some(it) = items.get(i) {
					self.run(s, f, it.command);
				}
			}
			paint::Slot::Recent(k) => self.run(s, f, Command::Recent(k as u8)),
		}
	}

	#[inline(never)] // keeps each mode's locals out of `key`'s frame (Win16 stacks are small)
	fn key_menu<S: Store + ?Sized, F: Files>(
		&mut self,
		s: &mut S,
		f: &mut F,
		k: Key,
		menu: u8,
		item: u8,
	) {
		let items = paint::menu_items_for(menu, !self.native);
		let recents = if menu == 0 {
			self.recent_files().count() as u8
		} else {
			0
		};
		let n = items.len() as u8 + recents;
		let total = paint::MENU_COUNT;
		// The next menu that has items, going left or right round the bar.
		let step = |from: u8, right: bool| {
			let mut m = from;
			for _ in 0..total {
				m = if right {
					(m + 1) % total
				} else {
					(m + total - 1) % total
				};
				if !paint::menu_items(m).is_empty() {
					return m;
				}
			}
			from
		};
		match k {
			Key::Esc | Key::F(10) => self.mode = Mode::Edit,
			Key::Alt(c) => {
				if let Some(m) = paint::menu_for_mnemonic(c) {
					self.mode = Mode::Menu { menu: m, item: 0 };
				}
			}
			Key::Char(c) => {
				let c = c.to_ascii_lowercase();
				if let Some(i) = paint::item_for_mnemonic(menu, c, recents as usize, !self.native) {
					self.mode = Mode::Edit;
					self.run_slot(s, f, items, recents as usize, i);
				}
			}
			Key::Home if n > 0 => self.mode = Mode::Menu { menu, item: 0 },
			Key::End if n > 0 => self.mode = Mode::Menu { menu, item: n - 1 },
			Key::Left => {
				self.mode = Mode::Menu {
					menu: step(menu, false),
					item: 0,
				}
			}
			Key::Right => {
				self.mode = Mode::Menu {
					menu: step(menu, true),
					item: 0,
				}
			}
			Key::Up if n > 0 => {
				self.mode = Mode::Menu {
					menu,
					item: (item + n - 1) % n,
				}
			}
			Key::Down if n > 0 => {
				self.mode = Mode::Menu {
					menu,
					item: (item + 1) % n,
				}
			}
			Key::Enter => {
				self.mode = Mode::Edit;
				self.run_slot(s, f, items, recents as usize, usize::from(item));
			}
			_ => {}
		}
	}

	#[inline(never)] // keeps each mode's locals out of `key`'s frame (Win16 stacks are small)
	fn key_prompt<S: Store + ?Sized, F: Files>(
		&mut self,
		s: &mut S,
		f: &mut F,
		k: Key,
		kind: Prompt,
		mut buf: [u8; NAME_MAX],
		mut len: usize,
	) {
		match k {
			Key::Esc => {
				self.mode = Mode::Edit;
				self.pending = None;
				return;
			}
			Key::Char(c) if len < NAME_MAX - 1 && c > 0x20 => {
				if let Some(slot) = buf.get_mut(len) {
					*slot = c.to_ascii_uppercase();
					len += 1;
				}
			}
			Key::Backspace => len = len.saturating_sub(1),
			Key::Enter if len > 0 => {
				self.mode = Mode::Edit;
				let name = buf.get(..len).unwrap_or(&[]);
				match kind {
					Prompt::Open => {
						let mut n = [0u8; NAME_MAX];
						n.get_mut(..len).unwrap_or(&mut []).copy_from_slice(name);
						if self.open(s, f, n.get(..len).unwrap_or(&[])).is_err() {
							self.msg.set("Quill cannot open that file.");
						}
					}
					Prompt::SaveAs => {
						self.save_as(s, f, name);
					}
					Prompt::Picture => {
						if self.insert_picture(s, f, name).is_err() {
							self.msg.set("Quill cannot use that picture.");
						}
					}
					Prompt::Print => {
						if !self.print_text(s, f, name) {
							self.msg.set("Quill cannot print to that file.");
						}
					}
				}
				return;
			}
			_ => {}
		}
		self.mode = Mode::Prompt { kind, buf, len };
	}

	/// Keeps the caret on the screen.
	fn scroll<S: ReadStore + ?Sized>(&mut self, s: &S) {
		let Some((idx, caret_line)) = self.caret_line(s) else {
			return;
		};
		let rows = self.rows as u32;
		if idx < self.top {
			self.top = idx;
		} else if self.view_h > 0 {
			// A window that draws pages one under another: the caret's line must fit in the page area's pixels, gaps and all.
			self.top = self.top_to_show(s, idx, &caret_line);
		} else if self.measure.resolution().1 <= 6 {
			// On the text screen a row is a unit of layout, so spacing and tall lines count: move down until the caret's line fits.
			let page_rows = quill_core::layout::to_units(self.doc.page.text_h(), 6);
			let cum = |l: &Line| (l.page as i32 - 1) * page_rows + l.y;
			let caret_end = cum(&caret_line) + (caret_line.ascent + caret_line.descent).max(1);
			let from = self.top;
			let mut top = idx;
			self.lines_from(s, from, |i, l| {
				if i < from {
					return true;
				}
				if i >= idx || caret_end - cum(l) <= rows as i32 {
					top = i.min(idx);
					return false;
				}
				true
			});
			self.top = top;
		} else if idx >= self.top + rows {
			self.top = idx + 1 - rows;
		}
		if self.want_x < 0 {
			if let Some((_, l)) = self.caret_line(s) {
				self.want_x = self.x_of(s, &l, self.caret);
			}
		}
	}
}
