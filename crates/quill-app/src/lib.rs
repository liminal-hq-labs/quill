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

mod paint;

use quill_core::doc::Doc;
use quill_core::layout::{layout_story, Line};
use quill_core::store::Store;
use quill_core::Error;
use quill_formats::io::{Sink, Source};
use quill_formats::{qld, text, FormatError};
use quill_grid::GridMeasure;

pub use paint::{draw, Cursor};

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
	/// A function key, 1 to 12.
	F(u8),
	/// Ctrl with a letter, as the lower-case letter.
	Ctrl(u8),
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
}

const NAME_MAX: usize = 64;

/// Undo step labels.
pub mod label {
	/// Typing.
	pub const TYPING: u8 = 1;
	/// Deleting.
	pub const DELETE: u8 = 2;
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub(crate) enum Prompt {
	Open,
	SaveAs,
}

#[derive(Copy, Clone, PartialEq, Eq)]
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
}

/// The editor.
pub struct App {
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
	pub(crate) msg: &'static str,
	pending: Option<Command>,
}

impl Default for App {
	fn default() -> Self {
		Self::new()
	}
}

impl App {
	/// An app with no document yet; call `reset`.
	pub const fn new() -> Self {
		App {
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
			msg: "",
			pending: None,
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
		let mut r = f.open(name).ok_or(FormatError::BadFile)?;
		let mut fresh = Doc::new();
		let res = if native {
			qld::read(&mut fresh, s, &mut r)
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
		let is_text = name.len() >= 4 && name.get(name.len() - 4..) == Some(b".TXT");
		if is_text {
			text::write(&self.doc, s, &mut w)?;
		} else {
			qld::write(&self.doc, s, &mut w)?;
		}
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
			&mut GridMeasure,
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

	/// The x of each character of a line, calling `f(position, character, x, width)`.
	pub(crate) fn walk<S: Store + ?Sized>(
		&self,
		s: &S,
		line: &Line,
		mut f: impl FnMut(u32, u8, i32, i32),
	) {
		let mut x = line.x;
		let mut at = 0u32;
		let mut buf = [0u8; 64];
		while at < line.len {
			let want = ((line.len - at) as usize).min(64);
			let Some(b) = buf.get_mut(..want) else {
				return;
			};
			let n = self.doc.body.read(s, line.start + at, b);
			if n == 0 {
				return;
			}
			for (i, &c) in b.iter().take(n).enumerate() {
				let rel = line.indent + (x - line.x);
				// A tab runs to the next half inch; everything else, an inline object included, is one column.
				let w = if c == 0x09 {
					(rel / 5 + 1) * 5 - rel
				} else {
					1
				};
				f(line.start + at + i as u32, c, x, w);
				x += w;
			}
			at += n as u32;
		}
	}

	/// The x of position `pos` in `line`.
	pub(crate) fn x_of<S: Store + ?Sized>(&self, s: &S, line: &Line, pos: u32) -> i32 {
		let mut x = line.x;
		let mut end = line.x;
		self.walk(s, line, |p, _, cx, w| {
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
		self.walk(s, line, |p, _, x, _| {
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

	fn insert<S: Store + ?Sized>(&mut self, s: &mut S, c: u8) {
		let t = &self.doc.body;
		let cfmt = if self.caret > 0 {
			t.cfmt_at(s, self.caret - 1)
		} else {
			t.cfmt_at(s, 0)
		}
		.unwrap_or(0);
		let pfmt = self
			.para_mark(s, self.caret)
			.and_then(|m| t.pfmt_at(s, m))
			.unwrap_or(0);
		if !self.typing {
			self.doc.body.begin_group(label::TYPING);
			self.typing = true;
		}
		self.want_x = -1;
		match self
			.doc
			.body
			.insert(s, self.caret, &[c], cfmt, pfmt, label::TYPING)
		{
			Ok(()) => {
				self.caret += 1;
				self.modified = true;
			}
			Err(Error::NoMemory) => {
				self.msg = "There is not enough memory to complete this operation."
			}
			Err(_) => self.msg = "The document is too large for Quill.",
		}
		if c == 0x0D {
			self.end_typing();
		}
	}

	fn delete<S: Store + ?Sized>(&mut self, s: &mut S, pos: u32) {
		self.end_typing();
		self.want_x = -1;
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

	fn run<S: Store + ?Sized, F: Files>(&mut self, s: &mut S, f: &mut F, c: Command) {
		self.end_typing();
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
					self.msg = "There is not enough memory to complete this operation.";
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

	fn do_save<S: Store + ?Sized, F: Files>(&mut self, s: &mut S, f: &mut F) -> bool {
		match self.save(s, f) {
			Ok(()) => {
				if let Some(p) = self.pending.take() {
					self.run(s, f, p);
				}
				true
			}
			Err(_) => {
				self.msg = "Quill cannot save the file.";
				self.pending = None;
				false
			}
		}
	}

	/// Handles one key.
	pub fn key<S: Store + ?Sized, F: Files>(&mut self, s: &mut S, f: &mut F, k: Key) {
		self.msg = "";
		match self.mode {
			Mode::Edit => self.key_edit(s, f, k),
			Mode::Menu { menu, item } => self.key_menu(s, f, k, menu, item),
			Mode::Prompt { kind, buf, len } => self.key_prompt(s, f, k, kind, buf, len),
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

	fn key_edit<S: Store + ?Sized, F: Files>(&mut self, s: &mut S, f: &mut F, k: Key) {
		let len = self.doc.body.len();
		match k {
			Key::Char(c) => self.insert(s, c),
			Key::Tab => self.insert(s, 0x09),
			Key::Enter => self.insert(s, 0x0D),
			Key::Backspace if self.caret > 0 => self.delete(s, self.caret - 1),
			Key::Delete if self.caret + 1 < len => self.delete(s, self.caret),
			Key::Left => {
				self.end_typing();
				self.caret = self.caret.saturating_sub(1);
				self.want_x = -1;
			}
			Key::Right => {
				self.end_typing();
				self.caret = (self.caret + 1).min(len.saturating_sub(1));
				self.want_x = -1;
			}
			Key::Up | Key::Down | Key::PageUp | Key::PageDown => {
				self.end_typing();
				let delta = match k {
					Key::Up => -1,
					Key::Down => 1,
					Key::PageUp => -(paint::TEXT_ROWS as i32 - 1),
					_ => paint::TEXT_ROWS as i32 - 1,
				};
				let keep = self.want_x >= 0;
				self.move_vertical(s, delta, keep);
			}
			Key::Home | Key::End => {
				self.end_typing();
				if let Some((_, l)) = self.caret_line(s) {
					self.caret = if k == Key::Home {
						l.start
					} else {
						l.start + l.len
					};
				}
				self.want_x = -1;
			}
			Key::CtrlHome => {
				self.end_typing();
				self.caret = 0;
				self.want_x = -1;
			}
			Key::CtrlEnd => {
				self.end_typing();
				self.caret = len.saturating_sub(1);
				self.want_x = -1;
			}
			Key::F(10) => {
				self.end_typing();
				self.mode = Mode::Menu { menu: 0, item: 0 };
			}
			Key::Ctrl(b's') => self.run(s, f, Command::Save),
			Key::Ctrl(b'o') => self.run(s, f, Command::Open),
			Key::Ctrl(b'n') => self.run(s, f, Command::New),
			Key::Ctrl(b'q') => self.run(s, f, Command::Exit),
			Key::Ctrl(b'z') => self.run(s, f, Command::Undo),
			Key::Ctrl(b'y') => self.run(s, f, Command::Redo),
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
		match k {
			Key::Esc | Key::F(10) => self.mode = Mode::Edit,
			Key::Left => {
				self.mode = Mode::Menu {
					menu: (menu + paint::ACTIVE_MENUS - 1) % paint::ACTIVE_MENUS,
					item: 0,
				}
			}
			Key::Right => {
				self.mode = Mode::Menu {
					menu: (menu + 1) % paint::ACTIVE_MENUS,
					item: 0,
				}
			}
			Key::Up => {
				self.mode = Mode::Menu {
					menu,
					item: (item + n - 1) % n,
				}
			}
			Key::Down => {
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
							self.msg = "Quill cannot open that file.";
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
		let rows = paint::TEXT_ROWS as u32;
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
