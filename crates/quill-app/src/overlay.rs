// What a menu or dialog of the app shows, as widgets in text cells, for a shell that draws them with real controls.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The text screen draws the app's own dialogs into a `Grid`; the DOS graphics shell draws the same dialogs with bevelled buttons,
//! check boxes, radio buttons, edit fields and list boxes. [`App::overlay`] describes the open dialog once, from the state the keys
//! already drive, so the keyboard behaves exactly as it does in text mode and a shell only decides how a widget looks. Positions are
//! in cells from the dialog's top left corner (row 0 is the title bar); a shell maps a cell to pixels as it likes.
//!
//! [`Wid::id`] is the widget's place in the dialog's own focus order (Tab), so a pointer that hits a widget knows which one to focus.
//! [`App::menu_mark`] gives the check mark and the greyed state of a menu item.

use quill_core::doc::cflag;
use quill_core::layout::Measure;
use quill_core::store::ReadStore;

use quill_dlg::widgets::Caret;

use crate::find::Focus;
use crate::{form, Command, Mode, Prompt};

/// The longest text a widget carries.
pub const WID_TEXT: usize = 72;
/// The most widgets a dialog has.
pub const WIDS: usize = 28;

/// What a widget is.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Wk {
	/// Plain text.
	#[default]
	Label,
	/// A text field; `caret` is the caret's offset in `text`, or 255 when it is not the focus.
	Edit,
	/// A check box; `on` says whether it is checked.
	Check,
	/// One button of a radio group; `on` says whether it is the chosen one.
	Radio,
	/// A push button; `default` draws the heavy border.
	Button,
	/// One entry of a list box; `on` is the selection bar.
	ListItem,
}

/// One control of a dialog.
#[derive(Copy, Clone, Debug)]
pub struct Wid {
	/// What it is.
	pub kind: Wk,
	/// Its place in the dialog's focus order (Tab goes 0, 1, 2 ...), or 255 when the keyboard never rests on it.
	pub id: u8,
	/// Column in cells from the dialog's left edge.
	pub col: u8,
	/// Row in cells from the dialog's top edge.
	pub row: u8,
	/// Width in cells, for the controls that have a frame (edit, button, list entry).
	pub w: u8,
	/// How many bytes of `text` are used.
	pub len: u8,
	/// The text: a label, a field's contents, a button's name.
	pub text: [u8; WID_TEXT],
	/// Checked, chosen or selected.
	pub on: bool,
	/// The keyboard focus is here.
	pub focus: bool,
	/// A default button.
	pub default: bool,
	/// The caret's offset in `text` for an edit field that has the focus, else 255.
	pub caret: u8,
	/// The other end of the selection in an edit field that has the focus: the same as `caret` when nothing is selected.
	pub anchor: u8,
}

impl Wid {
	/// The text.
	pub fn bytes(&self) -> &[u8] {
		self.text.get(..usize::from(self.len)).unwrap_or(&[])
	}
}

const BLANK: Wid = Wid {
	kind: Wk::Label,
	id: 255,
	col: 0,
	row: 0,
	w: 0,
	len: 0,
	text: [0; WID_TEXT],
	on: false,
	focus: false,
	default: false,
	caret: 255,
	anchor: 255,
};

/// A dialog: its title, its size in cells and its widgets.
#[derive(Clone)]
pub struct Overlay {
	/// The title bar's text.
	pub title: [u8; 24],
	/// How many bytes of `title`.
	pub title_len: u8,
	/// Width in cells.
	pub w: u8,
	/// Height in rows, the title bar included.
	pub h: u8,
	/// How many widgets are in use.
	pub n: u8,
	/// The widgets.
	pub wid: [Wid; WIDS],
}

impl Default for Overlay {
	fn default() -> Self {
		Overlay {
			title: [0; 24],
			title_len: 0,
			w: 0,
			h: 0,
			n: 0,
			wid: [BLANK; WIDS],
		}
	}
}

impl Overlay {
	/// The title.
	pub fn title(&self) -> &[u8] {
		self.title.get(..usize::from(self.title_len)).unwrap_or(&[])
	}

	/// The widgets in use.
	pub fn widgets(&self) -> &[Wid] {
		self.wid.get(..usize::from(self.n)).unwrap_or(&[])
	}

	fn start(&mut self, title: &[u8], w: u8, h: u8) {
		*self = Overlay::default();
		let n = title.len().min(self.title.len());
		if let (Some(d), Some(s)) = (self.title.get_mut(..n), title.get(..n)) {
			d.copy_from_slice(s);
		}
		self.title_len = n as u8;
		self.w = w;
		self.h = h;
	}

	fn add(&mut self, kind: Wk, col: u8, row: u8, w: u8, text: &[u8]) -> Option<&mut Wid> {
		let at = usize::from(self.n);
		let wid = self.wid.get_mut(at)?;
		self.n += 1;
		*wid = Wid {
			kind,
			col,
			row,
			w,
			..BLANK
		};
		let n = text.len().min(WID_TEXT);
		if let (Some(d), Some(s)) = (wid.text.get_mut(..n), text.get(..n)) {
			d.copy_from_slice(s);
		}
		wid.len = n as u8;
		Some(wid)
	}

	fn label(&mut self, col: u8, row: u8, text: &[u8]) {
		self.add(Wk::Label, col, row, 0, text);
	}

	fn edit(&mut self, id: u8, (col, row, w): (u8, u8, u8), text: &[u8], focus: bool, c: &Caret) {
		if let Some(e) = self.add(Wk::Edit, col, row, w, text) {
			e.id = id;
			e.focus = focus;
			let at = |v: usize| v.min(text.len()).min(WID_TEXT) as u8;
			(e.caret, e.anchor) = if focus {
				(at(c.at()), at(c.selection().0 + c.selection().1 - c.at()))
			} else {
				(255, 255)
			};
		}
	}

	fn check(&mut self, id: u8, col: u8, row: u8, text: &[u8], on: bool, focus: bool) {
		let w = text.len() as u8;
		if let Some(c) = self.add(Wk::Check, col, row, w, text) {
			c.id = id;
			c.on = on;
			c.focus = focus;
		}
	}

	fn radio(&mut self, id: u8, col: u8, row: u8, text: &[u8], on: bool, focus: bool) {
		let w = text.len() as u8;
		if let Some(c) = self.add(Wk::Radio, col, row, w, text) {
			c.id = id;
			c.on = on;
			c.focus = focus;
		}
	}

	fn button(&mut self, id: u8, col: u8, row: u8, text: &[u8], focus: bool, default: bool) -> u8 {
		let w = text.len() as u8 + 4;
		if let Some(b) = self.add(Wk::Button, col, row, w, text) {
			b.id = id;
			b.focus = focus;
			b.default = default;
		}
		w
	}
}

/// Strips the `< ` and ` >` a text-mode button's label has.
fn plain(label: &str) -> &[u8] {
	let b = label.as_bytes();
	let b = b.strip_prefix(b"< ").unwrap_or(b);
	b.strip_suffix(b" >").unwrap_or(b)
}

/// What a menu item shows besides its words.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Mark {
	/// A check mark before the label.
	pub checked: bool,
	/// Greyed out: the command would do nothing now.
	pub disabled: bool,
}

impl<M: Measure + Clone> crate::App<M> {
	/// The check mark and the greyed state of the menu item that runs `c`.
	pub fn menu_mark<S: ReadStore + ?Sized>(&self, s: &S, c: Command) -> Mark {
		let cs = || self.char_state(s);
		let vpos = || {
			let at = self
				.selection()
				.map_or(self.caret.saturating_sub(1), |(lo, _)| lo);
			self.doc
				.body
				.cfmt_at(s, at)
				.and_then(|i| self.doc.cfmts.get(s, i))
				.map_or(0, |c| c.vpos)
		};
		let checked = match c {
			Command::Bold => cs().flags & cflag::BOLD != 0,
			Command::Italic => cs().flags & cflag::ITALIC != 0,
			Command::Underline => cs().flags & cflag::UNDERLINE != 0,
			Command::Strike => cs().flags & cflag::STRIKE != 0,
			Command::Super => vpos() == 1,
			Command::Sub => vpos() == 2,
			Command::Regular => cs().flags & 0xF == 0 && vpos() == 0,
			Command::Align(a) => self.para_state(s).align == a,
			Command::Spacing(n) => self.para_state(s).spacing == n,
			Command::Bullets => self.para_state(s).flags & 1 != 0,
			Command::Numbering => self.para_state(s).flags & 4 != 0,
			Command::Chrome(bit) => self.chrome_on(bit),
			Command::SpellAsYouType => self.spell.live.on,
			Command::FirstPage => self.doc.page.flags & 1 != 0,
			Command::View(n) => self.view == n,
			Command::Zoom(z) => self.zoom == z,
			_ => false,
		};
		let disabled = match c {
			Command::Cut | Command::Copy => self.selection().is_none(),
			Command::Paste => self.clip_len == 0,
			_ => false,
		};
		Mark { checked, disabled }
	}

	/// Describes the dialog that is open, or returns false when none is (a menu is not a dialog: its state is `menu_open`).
	pub fn overlay<S: ReadStore + ?Sized>(&self, s: &S, o: &mut Overlay) -> bool {
		match &self.mode {
			Mode::Edit | Mode::Menu { .. } => false,
			Mode::Prompt {
				kind,
				buf,
				len,
				caret,
			} => {
				let title: &[u8] = match kind {
					Prompt::Open => b"Open",
					Prompt::SaveAs => b"Save As",
					Prompt::Print => b"Print to file or device",
					Prompt::Picture => b"Insert Picture",
				};
				o.start(title, 56, 4);
				o.label(2, 1, b"File name:");
				o.edit(0, (14, 1, 40), buf.get(..*len).unwrap_or(&[]), true, caret);
				let w = o.button(255, 14, 3, b"OK", false, true);
				o.button(255, 14 + w + 2, 3, b"Cancel", false, false);
				true
			}
			Mode::Confirm(_, focus) => {
				o.start(b"Quill", 56, 4);
				let mut t = [0u8; WID_TEXT];
				let mut n = 0;
				let mut put = |b: &[u8]| {
					for &c in b {
						if let Some(d) = t.get_mut(n) {
							*d = c;
							n += 1;
						}
					}
				};
				put(b"Save changes to ");
				if self.name_len == 0 {
					put(b"(Untitled)");
				} else {
					put(self.name());
				}
				put(b"?");
				o.label(2, 1, t.get(..n).unwrap_or(&[]));
				let mut x = 8;
				for (i, l) in [&b"Yes"[..], b"No", b"Cancel"].iter().enumerate() {
					let on = usize::from(*focus) == i;
					let w = o.button(i as u8, x, 3, l, on, on);
					x += w + 3;
				}
				true
			}
			Mode::About => {
				o.start(b"About Quill", 56, 9);
				o.label(2, 1, b"Quill");
				let mut v = [0u8; WID_TEXT];
				let mut n = 0;
				for &c in b"Version ".iter().chain(crate::VERSION.as_bytes()) {
					if let Some(d) = v.get_mut(n) {
						*d = c;
						n += 1;
					}
				}
				o.label(2, 2, v.get(..n).unwrap_or(&[]));
				o.label(2, 3, b"A Liminal HQ Labs experiment");
				o.label(2, 4, b"Apache-2.0 OR MIT");
				for (i, line) in self
					.about
					.bytes()
					.split(|b| *b == b'\n')
					.take(2)
					.enumerate()
				{
					o.label(2, 5 + i as u8, line);
				}
				o.button(0, 22, 8, b"OK", true, true);
				true
			}
			Mode::Find(st) => {
				let replace = st.replace;
				o.start(
					if replace { b"Replace" } else { b"Find" },
					62,
					if replace { 7 } else { 6 },
				);
				let mut row = 1;
				o.label(2, row, b"Find what:");
				o.edit(
					0,
					(16, row, 34),
					st.find.get(..st.flen).unwrap_or(&[]),
					st.focus == Focus::Find,
					&st.fcaret,
				);
				if replace {
					row += 1;
					o.label(2, row, b"Replace with:");
					o.edit(
						1,
						(16, row, 34),
						st.with.get(..st.wlen).unwrap_or(&[]),
						st.focus == Focus::Replace,
						&st.wcaret,
					);
				}
				row += 1;
				let base = if replace { 2 } else { 1 };
				o.check(
					base,
					2,
					row + 1,
					b"Match case",
					st.case,
					st.focus == Focus::Case,
				);
				o.check(
					base + 1,
					22,
					row + 1,
					b"Match whole word only",
					st.word,
					st.focus == Focus::Word,
				);
				let row = row + 3;
				let buttons: &[(&[u8], Focus)] = if replace {
					&[
						(b"Find Next", Focus::FindNext),
						(b"Replace", Focus::ReplaceOne),
						(b"Replace All", Focus::ReplaceAll),
						(b"Close", Focus::Close),
					]
				} else {
					&[(b"Find Next", Focus::FindNext), (b"Close", Focus::Close)]
				};
				let mut x = 2;
				for (i, (t, f)) in buttons.iter().enumerate() {
					let on = st.focus == *f;
					let default = on
						|| (st.focus != Focus::Close
							&& *f == Focus::FindNext
							&& !is_button(st.focus));
					let w = o.button(base + 2 + i as u8, x, row, t, on, default);
					x += w + 1;
				}
				true
			}
			Mode::Form(f) => {
				let fonts = f.kind == form::FormKind::Fonts;
				let mut row = 1u8;
				let (title, w) = if fonts {
					(&b"Fonts"[..], 62)
				} else {
					(&b"Tabs"[..], 62)
				};
				o.start(title, w, 0);
				let col = if fonts { 12 } else { 22 };
				let mut x = 2u8;
				let mut checks = 0;
				let mut first_button = true;
				for (i, r) in f.rows.iter().take(f.n).enumerate() {
					let id = i as u8;
					let on = i == f.focus;
					match r.kind {
						0 => {
							o.label(2, row, r.label.as_bytes());
							o.edit(
								id,
								(col, row, 14),
								r.text.get(..r.len).unwrap_or(&[]),
								on,
								&r.caret,
							);
							row += 1;
						}
						1 => {
							o.label(2, row, r.label.as_bytes());
							let mut cx = col;
							for (k, name) in r.names.iter().enumerate() {
								o.radio(
									id,
									cx,
									row,
									name.as_bytes(),
									usize::from(r.val) == k,
									on && usize::from(r.val) == k,
								);
								cx += name.len() as u8 + 4;
							}
							row += 1;
						}
						2 => {
							// Check boxes go two to a row.
							let cx = if checks % 2 == 0 { 2 } else { 24 };
							o.check(id, cx, row, r.label.as_bytes(), r.val == 1, on);
							checks += 1;
							if checks % 2 == 0 {
								row += 1;
							}
						}
						_ => {
							if first_button {
								first_button = false;
								if checks % 2 == 1 {
									row += 1;
								}
								row += 1;
							}
							let d = on
								|| (first_button_default(f) == i
									&& !f.rows.get(f.focus).is_some_and(|r| r.kind == 3));
							let bw = o.button(id, x, row, plain(r.label), on, d);
							x += bw + 1;
						}
					}
				}
				o.h = row + 1;
				true
			}
			Mode::Spell(d) => {
				o.start(b"Spelling", 76, 10);
				let mut t = [0u8; WID_TEXT];
				let mut n = 0;
				for &c in b"Not in dictionary: ".iter().chain(self.spell_word()) {
					if let Some(slot) = t.get_mut(n) {
						*slot = c;
						n += 1;
					}
				}
				o.label(2, 1, t.get(..n).unwrap_or(&[]));
				let mut ctx = [0u8; 72];
				let n = self.spell_context(s, &mut ctx);
				o.label(2, 2, ctx.get(..n.min(62)).unwrap_or(&[]));
				o.label(2, 4, b"Change to:");
				for i in 0..usize::from(d.n) {
					let l = usize::from(d.lens.get(i).copied().unwrap_or(0));
					let word = d.words.get(i).and_then(|w| w.get(..l)).unwrap_or(&[]);
					let col = 14 + (i / 4) as u8 * 22;
					if let Some(e) = o.add(Wk::ListItem, col, 4 + (i % 4) as u8, 20, word) {
						e.on = usize::from(d.sel) == i;
						e.id = 255;
					}
				}
				if d.n == 0 {
					o.label(14, 4, b"(no suggestions)");
				}
				let mut x = 2;
				for (i, b) in crate::spelling::BUTTONS.iter().enumerate() {
					let on = usize::from(d.focus) == i;
					let w = o.button(i as u8, x, 9, b.as_bytes(), on, on);
					x += w + 1;
				}
				true
			}
		}
	}
}

fn is_button(f: Focus) -> bool {
	matches!(
		f,
		Focus::FindNext | Focus::ReplaceOne | Focus::ReplaceAll | Focus::Close
	)
}

/// The button Enter presses when the focus is not on a button.
fn first_button_default(f: &form::Form) -> usize {
	f.rows
		.iter()
		.take(f.n)
		.position(|r| r.kind == 3)
		.unwrap_or(0)
}
