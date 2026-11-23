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

mod find;
mod form;
mod paint;
pub mod script;

use quill_core::doc::{cflag, CFmt, Doc, ListDef, ListLevel, PFmt, StoryKind};
use quill_core::layout::{layout_story, Line, Measure};
use quill_core::store::Store;
use quill_core::Error;
use quill_formats::io::{Sink, Source};
use quill_formats::{qld, rtf, text, FormatError};
use quill_grid::GridMeasure;

pub use paint::{draw, line_label, menu_items, menu_title, Cursor, Item, ListCounters, MENU_COUNT};

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
	/// Finishes a file started with `create`; false when the disk refused it.
	fn close(&mut self, w: Self::W) -> bool;
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
}

const NAME_MAX: usize = 64;

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
	Confirm(Command),
	Find(find::FindState),
	Form(form::Form),
}

/// The editor.
pub struct App<M: Measure + Clone = GridMeasure> {
	/// The device the document is laid out for.
	pub(crate) measure: M,
	/// Lines the view shows at once (23 on an 80 by 25 screen); the shell sets it.
	pub(crate) rows: usize,
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
	pending: Option<Command>,
	/// The other end of the selection, when there is one.
	pub(crate) anchor: Option<u32>,
	pub(crate) extend: bool,
	pub(crate) overtype: bool,
	pub(crate) scheme: Scheme,
	/// The format the next typed character takes, chosen with no selection.
	typing_fmt: Option<u16>,
	/// The story being edited: 0 body, 1 header, 2 footer, 3 first header, 4 first footer. The active story lives in `doc.body`.
	pub(crate) view: u8,
	saved_view: (u32, u32),
	/// The clipboard: text only, until Quill exits.
	pub(crate) clip: [u8; 2048],
	pub(crate) clip_len: usize,
	/// The last search, for Find Next.
	pub(crate) last_find: Option<find::FindState>,
	/// The format the document was opened in: 0 native, 1 RTF, 2 text.
	fmt: u8,
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
			pending: None,
			anchor: None,
			extend: false,
			overtype: false,
			scheme: Scheme::Colour,
			typing_fmt: None,
			fmt: 0,
			last_find: None,
			clip: [0; 2048],
			clip_len: 0,
			view: 0,
			saved_view: (0, 0),
		}
	}

	/// Starts an empty, untitled document.
	pub fn reset<S: Store + ?Sized>(&mut self, s: &mut S) -> Result<(), Error> {
		self.doc.init(s)?;
		self.caret = 0;
		self.top = 0;
		self.name_len = 0;
		self.modified = false;
		self.mode = Mode::Edit;
		self.typing = false;
		Ok(())
	}

	/// Sets how many lines the view shows at once, for paging and scrolling.
	pub fn set_rows(&mut self, rows: usize) {
		self.rows = rows.max(1);
	}

	/// The caret position in the active story.
	pub fn caret(&self) -> u32 {
		self.caret
	}

	/// The index of the first line shown.
	pub fn top_line(&self) -> u32 {
		self.top
	}

	/// True when no menu, dialog or prompt of the app's own is open.
	pub fn editing(&self) -> bool {
		self.mode == Mode::Edit
	}

	/// Runs a command, as a menu does.
	pub fn command<S: Store + ?Sized, F: Files>(&mut self, s: &mut S, f: &mut F, c: Command) {
		self.msg.clear();
		self.run(s, f, c);
		self.scroll(s);
	}

	/// Saves under a new name, as Save As does.
	pub fn save_as<S: Store + ?Sized, F: Files>(
		&mut self,
		s: &mut S,
		f: &mut F,
		name: &[u8],
	) -> bool {
		self.set_name(name);
		self.do_save(s, f)
	}

	/// The message the last command left for the status line.
	pub fn message(&self) -> &[u8] {
		self.msg.bytes()
	}

	/// Calls `f(index, line)` for each line of the body (or the story being edited), laid out for the app's device.
	pub fn for_each_line<S: Store + ?Sized>(&self, s: &S, f: impl FnMut(u32, &Line)) {
		self.lines(s, f);
	}

	/// Calls `f(position, character, x, width, leader, character format)` for each character of a line.
	pub fn walk_chars<S: Store + ?Sized>(
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
	pub fn caret_xy<S: Store + ?Sized>(&self, s: &S) -> Option<(u32, Line, i32)> {
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
			*d = s.to_ascii_uppercase();
		}
	}

	/// Opens a file by content (a RIFF `QLD1` header is native, anything else is plain text) into a fresh document, and
	/// keeps the current one when it cannot be read.
	pub fn open<S: Store + ?Sized, F: Files>(
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
		self.caret = 0;
		self.top = 0;
		self.modified = false;
		self.typing = false;
		self.set_name(name);
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
	) -> Result<(), FormatError> {
		let name = self.name;
		let name = name.get(..self.name_len).unwrap_or(&[]);
		let mut w = f.create(name).ok_or(FormatError::Write)?;
		let ext = name.len().checked_sub(4).and_then(|i| name.get(i..));
		let kind = match ext {
			Some(b".QLD") => 0,
			Some(b".RTF") => 1,
			Some(b".TXT") => 2,
			_ => self.fmt,
		};
		match kind {
			1 => rtf::write(&self.doc, s, &mut w)?,
			2 => text::write(&self.doc, s, &mut w)?,
			_ => qld::write(&self.doc, s, &mut w)?,
		}
		self.fmt = kind;
		if !f.close(w) {
			return Err(FormatError::Write);
		}
		self.modified = false;
		Ok(())
	}

	pub(crate) fn lines<S: Store + ?Sized>(&self, s: &S, mut f: impl FnMut(u32, &Line)) {
		let mut i = 0;
		layout_story(
			&self.doc,
			s,
			&mut self.measure.clone(),
			quill_core::doc::StoryKind::Body,
			|l| {
				f(i, l);
				i += 1;
			},
		);
	}

	/// The index and line holding the caret.
	pub(crate) fn caret_line<S: Store + ?Sized>(&self, s: &S) -> Option<(u32, Line)> {
		let caret = self.caret;
		let mut found: Option<(u32, Line)> = None;
		let mut prev: Option<(u32, Line)> = None;
		self.lines(s, |i, l| {
			if found.is_none() {
				if let Some((pi, p)) = prev {
					let soft = l.start == p.start + p.len;
					if caret >= p.start
						&& caret <= p.start + p.len
						&& !(soft && caret == p.start + p.len)
					{
						found = Some((pi, p));
					}
				}
			}
			prev = Some((i, *l));
		});
		if found.is_none() {
			if let Some((pi, p)) = prev {
				if caret >= p.start {
					found = Some((pi, p));
				}
			}
		}
		found
	}

	/// The x of each character of a line, calling `f(position, character, x, width, leader)`. The leader is 0 except for a tab
	/// that has one (1 dots, 2 hyphens, 3 underline).
	pub(crate) fn walk<S: Store + ?Sized>(
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
	pub(crate) fn x_of<S: Store + ?Sized>(&self, s: &S, line: &Line, pos: u32) -> i32 {
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

	fn pos_at_x<S: Store + ?Sized>(&self, s: &S, line: &Line, want: i32) -> u32 {
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

	fn line_at<S: Store + ?Sized>(&self, s: &S, idx: u32) -> Option<Line> {
		let mut out = None;
		self.lines(s, |i, l| {
			if i == idx {
				out = Some(*l);
			}
		});
		out
	}

	fn line_count<S: Store + ?Sized>(&self, s: &S) -> u32 {
		let mut n = 0;
		self.lines(s, |_, _| n += 1);
		n
	}

	fn end_typing(&mut self) {
		if self.typing {
			self.doc.body.end_group();
			self.typing = false;
		}
	}

	/// The position of the paragraph mark that ends the paragraph holding `pos`.
	fn para_mark<S: Store + ?Sized>(&self, s: &S, pos: u32) -> Option<u32> {
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
			self.caret = lo;
			self.modified = true;
			true
		} else {
			false
		}
	}

	fn insert<S: Store + ?Sized>(&mut self, s: &mut S, c: u8) {
		if self.selection().is_some() {
			self.end_typing();
			self.doc.body.begin_group(label::TYPING);
			self.typing = true;
			self.delete_selection(s);
		}
		self.anchor = None;
		let t = &self.doc.body;
		let prev = if self.caret > 0 {
			t.cfmt_at(s, self.caret - 1)
		} else {
			t.cfmt_at(s, 0)
		}
		.unwrap_or(0);
		let cfmt = self.typing_fmt.unwrap_or(prev);
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
		{
			let _ = self.doc.body.delete(s, self.caret, 1, label::TYPING);
		}
		match self
			.doc
			.body
			.insert(s, self.caret, &[c], cfmt, pfmt, label::TYPING)
		{
			Ok(()) => {
				self.caret += 1;
				self.modified = true;
			}
			Err(Error::NoMemory) => self
				.msg
				.set("There is not enough memory to complete this operation."),
			Err(_) => self.msg.set("The document is too large for Quill."),
		}
		if c == 0x0D {
			self.end_typing();
			self.typing_fmt = None;
		}
	}

	fn delete<S: Store + ?Sized>(&mut self, s: &mut S, pos: u32) {
		self.end_typing();
		self.want_x = -1;
		if self.selection().is_some() {
			self.delete_selection(s);
			return;
		}
		if self.doc.body.delete(s, pos, 1, label::DELETE).is_ok() {
			self.caret = pos;
			self.modified = true;
		}
	}

	fn move_vertical<S: Store + ?Sized>(&mut self, s: &S, delta: i32, keep_x: bool) {
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
		let _ = s;
	}

	fn set_view<S: Store + ?Sized>(&mut self, s: &mut S, to: u8) {
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
		if self.view != 0
			&& matches!(
				c,
				Command::New | Command::Open | Command::Save | Command::SaveAs | Command::Exit
			) {
			self.set_view(s, 0);
		}
		match c {
			Command::New | Command::Open | Command::Exit
				if self.modified && self.pending != Some(c) =>
			{
				self.mode = Mode::Confirm(c);
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
				self.pending = None;
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
			Command::Field(k) if self.view != 0 => {
				let t = &self.doc.body;
				let cf = if self.caret > 0 {
					t.cfmt_at(s, self.caret - 1)
				} else {
					t.cfmt_at(s, 0)
				}
				.unwrap_or(0);
				let pf = self.para_pfmt(s, self.caret);
				let _ = pf;
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
					self.caret += 1;
					self.modified = true;
				}
			}
			Command::Field(_) => {}
			Command::Cut | Command::Copy | Command::Paste | Command::SelectAll => {
				self.clip_command(s, c)
			}
			Command::Fonts => self.open_fonts(s),
			Command::Tabs => self.open_tabs(),
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
					self.modified = true;
					self.caret = self.caret.min(self.doc.body.len().saturating_sub(1));
				}
			}
			Command::Redo => {
				if self.doc.body.redo(s).unwrap_or(false) {
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
			let t = &self.doc.body;
			let at = if self.caret > 0 { self.caret - 1 } else { 0 };
			let cur = self.typing_fmt.or_else(|| t.cfmt_at(s, at)).unwrap_or(0);
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
	pub(crate) fn para_pfmt<S: Store + ?Sized>(&self, s: &S, pos: u32) -> PFmt {
		self.para_mark(s, pos)
			.and_then(|m| self.doc.body.pfmt_at(s, m))
			.and_then(|i| self.doc.pfmts.get(s, i))
			.unwrap_or(PFmt::DEFAULT)
	}

	/// The position where the paragraph holding `pos` starts.
	fn para_start<S: Store + ?Sized>(&self, s: &S, pos: u32) -> u32 {
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
			Err(_) => {
				self.msg.set("Quill cannot save the file.");
				self.pending = None;
				false
			}
		}
	}

	/// Handles one key.
	pub fn key<S: Store + ?Sized, F: Files>(&mut self, s: &mut S, f: &mut F, k: Key) {
		self.msg.set("");
		match self.mode {
			Mode::Edit => self.key_edit(s, f, k),
			Mode::Menu { menu, item } => self.key_menu(s, f, k, menu, item),
			Mode::Prompt { kind, buf, len } => self.key_prompt(s, f, k, kind, buf, len),
			Mode::Find(st) => self.key_find(s, k, st),
			Mode::Form(fm) => self.key_form(s, k, fm),
			Mode::Confirm(c) => match k {
				Key::Char(b'y' | b'Y') => {
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
				Key::Char(b'n' | b'N') => {
					self.mode = Mode::Edit;
					self.modified = false;
					self.run(s, f, c);
				}
				Key::Esc => self.mode = Mode::Edit,
				_ => {}
			},
		}
		self.scroll(s);
	}

	fn move_caret<S: Store + ?Sized>(&mut self, s: &S, d: Dir) {
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
				let empty = self.doc.body.read_byte(s, self.caret) == Some(0x0D)
					&& (self.caret == 0
						|| self.doc.body.read_byte(s, self.caret - 1) == Some(0x0D));
				if pf.flags & 5 != 0 && empty && self.selection().is_none() {
					self.para_command(s, |p| PFmt {
						flags: p.flags & !5,
						left: 0,
						first: 0,
						level: 0,
						..p
					});
				} else {
					self.insert(s, 0x0D);
				}
			}
			Key::Backspace if self.selection().is_some() => self.delete(s, self.caret),
			Key::Backspace if self.caret > 0 => self.delete(s, self.caret - 1),
			Key::Delete if self.selection().is_some() => self.delete(s, self.caret),
			Key::Delete if self.caret + 1 < len => self.delete(s, self.caret),
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
			Key::Ctrl(b'f') => self.run(s, f, Command::Find),
			Key::Ctrl(b'h') => self.run(s, f, Command::Replace),
			Key::F(10) => {
				self.end_typing();
				self.mode = Mode::Menu { menu: 0, item: 0 };
			}
			Key::Ctrl(b's') => self.run(s, f, Command::Save),
			Key::Ctrl(b'o') => self.run(s, f, Command::Open),
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

	fn key_menu<S: Store + ?Sized, F: Files>(
		&mut self,
		s: &mut S,
		f: &mut F,
		k: Key,
		menu: u8,
		item: u8,
	) {
		let items = paint::menu_items(menu);
		let n = items.len() as u8;
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
				if let Some(it) = items.get(usize::from(item)) {
					self.run(s, f, it.command);
				}
			}
			_ => {}
		}
	}

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
						self.set_name(name);
						self.do_save(s, f);
					}
				}
				return;
			}
			_ => {}
		}
		self.mode = Mode::Prompt { kind, buf, len };
	}

	/// Keeps the caret on the screen.
	fn scroll<S: Store + ?Sized>(&mut self, s: &S) {
		let Some((idx, _)) = self.caret_line(s) else {
			return;
		};
		let rows = self.rows as u32;
		if idx < self.top {
			self.top = idx;
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
