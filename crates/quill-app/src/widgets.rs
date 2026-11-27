// The pointer's hands on the menus and dialogs: open, light and close a menu, and press the widget of a dialog the shell drew.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A shell that draws real controls (the DOS graphics screen) finds the widget under the pointer from [`Overlay`]'s cells and the
//! menu's rows from its own geometry; these methods turn what it found into what the keyboard would have done, so a click and the
//! keys that mean the same thing end in the same state. Nothing here is a second implementation of a dialog: a press sets the focus
//! (or the value) the keys would have set and sends Enter or a space the way `TextCells` does.

use quill_core::layout::Measure;
use quill_core::store::{ReadStore, Store};

use crate::find::Focus;
use crate::form::{FONT_FAMILY, FONT_NAMES};
use crate::{paint, App, Files, Key, Mode, Overlay, Wk};

/// What the pointer does to the caret of the focused text field.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Place {
	/// A press: the caret goes there and the selection is dropped.
	Set,
	/// A drag, or a press with Shift: the caret goes there and the other end stays.
	Extend,
	/// A double click: the word there is selected.
	Word,
}

/// What a press of a widget did.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Pressed {
	/// The press changed nothing (a label, a field that has the focus already, or a widget the dialog does not have).
	Nothing,
	/// The widget took the focus or its value changed; the dialog stays open.
	Focused,
	/// The widget ran its action (a button, or Enter through it).
	Acted,
}

impl<M: Measure + Clone> App<M> {
	/// Opens menu `menu` with its first item lit, from the editing screen or from another open menu. False when it has no items or a
	/// dialog is open.
	pub fn menu_pull(&mut self, menu: u8) -> bool {
		if !matches!(self.mode, Mode::Edit | Mode::Menu { .. })
			|| paint::menu_items(menu).is_empty()
		{
			return false;
		}
		self.mode = Mode::Menu { menu, item: 0 };
		true
	}

	/// Lights place `item` of the open menu (the app's own numbering: its items and the recent files). A place that is not there is
	/// ignored.
	pub fn menu_light(&mut self, item: u8) {
		if let Mode::Menu { menu, .. } = self.mode {
			let recents = if menu == 0 {
				self.recent_files().count()
			} else {
				0
			};
			let n = paint::menu_items_for(menu, !self.native).len() + recents;
			if usize::from(item) < n {
				self.mode = Mode::Menu { menu, item };
			}
		}
	}

	/// Closes the open menu without running anything.
	pub fn menu_shut(&mut self) {
		if matches!(self.mode, Mode::Menu { .. }) {
			self.mode = Mode::Edit;
		}
	}

	/// Presses widget `idx` of the open dialog (an index into [`Overlay::widgets`]): a field or a list entry takes the focus or the
	/// selection, a check box or a radio button changes, a button acts. With `accept` a list entry also takes its Change button, which
	/// is what a double click on a suggestion does. A text field keeps the caret at the end of what it holds, so pressing one only
	/// gives it the focus.
	pub fn press_widget<S: Store + ?Sized, F: Files>(
		&mut self,
		s: &mut S,
		f: &mut F,
		idx: usize,
		accept: bool,
	) -> Pressed {
		let mut o = Overlay::default();
		if !self.overlay(s, &mut o) {
			return Pressed::Nothing;
		}
		let Some(w) = o.widgets().get(idx).copied() else {
			return Pressed::Nothing;
		};
		let id = usize::from(w.id);
		match self.mode {
			Mode::Prompt { .. } => match w.kind {
				Wk::Button => {
					let ok = w.bytes() == b"OK";
					self.key(s, f, if ok { Key::Enter } else { Key::Esc });
					Pressed::Acted
				}
				_ => Pressed::Nothing,
			},
			#[cfg(feature = "text-recovery")]
			Mode::Recover(ask, _) => {
				if w.kind != Wk::Button {
					return Pressed::Nothing;
				}
				self.mode = Mode::Recover(ask, w.id);
				self.key(s, f, Key::Enter);
				Pressed::Acted
			}
			Mode::Confirm(c, _) => {
				if w.kind != Wk::Button {
					return Pressed::Nothing;
				}
				self.mode = Mode::Confirm(c, w.id);
				self.key(s, f, Key::Enter);
				Pressed::Acted
			}
			#[cfg(feature = "text-file-dialog")]
			Mode::File => {
				if w.kind != Wk::Button {
					return Pressed::Nothing;
				}
				self.press_file_box(s, f, w.id, w.bytes() == b"OK");
				Pressed::Acted
			}
			Mode::About => {
				if w.kind != Wk::Button {
					return Pressed::Nothing;
				}
				self.key(s, f, Key::Enter);
				Pressed::Acted
			}
			Mode::Find(mut st) => {
				let order = st.order();
				// Ids count the edit fields, the two check boxes and the buttons in the order the dialog draws them.
				let skip = if st.replace { 2 } else { 1 };
				let focus = match id {
					0 => Focus::Find,
					1 if st.replace => Focus::Replace,
					n if n < skip + 2 => {
						if n == skip {
							Focus::Case
						} else {
							Focus::Word
						}
					}
					n => {
						let at = n - skip - 2;
						let buttons: &[Focus] = if st.replace {
							&[
								Focus::FindNext,
								Focus::ReplaceOne,
								Focus::ReplaceAll,
								Focus::Close,
							]
						} else {
							&[Focus::FindNext, Focus::Close]
						};
						buttons.get(at).copied().unwrap_or(Focus::Find)
					}
				};
				if !order.contains(&focus) {
					return Pressed::Nothing;
				}
				st.focus = focus;
				self.mode = Mode::Find(st);
				match w.kind {
					Wk::Edit => Pressed::Focused,
					_ => {
						self.key(s, f, Key::Enter);
						Pressed::Acted
					}
				}
			}
			Mode::Form(mut fm) => {
				if id >= fm.n {
					return Pressed::Nothing;
				}
				fm.focus = id;
				match w.kind {
					Wk::Radio => {
						let k = o.widgets().get(..idx).map_or(0, |before| {
							before
								.iter()
								.filter(|b| b.kind == Wk::Radio && b.id == w.id)
								.count()
						});
						if let Some(r) = fm.rows.get_mut(id) {
							r.val = k as u8;
						}
						self.mode = Mode::Form(fm);
						Pressed::Focused
					}
					Wk::Check => {
						if let Some(r) = fm.rows.get_mut(id) {
							r.val = u8::from(r.val == 0);
						}
						self.mode = Mode::Form(fm);
						Pressed::Focused
					}
					Wk::Button => {
						self.mode = Mode::Form(fm);
						self.key(s, f, Key::Enter);
						Pressed::Acted
					}
					_ => {
						self.mode = Mode::Form(fm);
						Pressed::Focused
					}
				}
			}
			Mode::Spell(mut d) => match w.kind {
				Wk::ListItem => {
					let k = o.widgets().get(..idx).map_or(0, |before| {
						before.iter().filter(|b| b.kind == Wk::ListItem).count()
					});
					d.sel = k as u8;
					if accept {
						// Change is the third button.
						d.focus = 2;
						self.mode = Mode::Spell(d);
						self.key(s, f, Key::Enter);
						Pressed::Acted
					} else {
						self.mode = Mode::Spell(d);
						Pressed::Focused
					}
				}
				Wk::Button => {
					d.focus = w.id;
					self.mode = Mode::Spell(d);
					self.key(s, f, Key::Enter);
					Pressed::Acted
				}
				_ => Pressed::Nothing,
			},
			Mode::Edit | Mode::Menu { .. } => Pressed::Nothing,
		}
	}

	/// Moves the caret of the field that has the focus to character offset `pos` (what a press, a drag or a double click on it does).
	/// False when no text field has the focus.
	pub fn field_place(&mut self, pos: usize, how: Place) -> bool {
		let place = |text: &[u8], c: &mut quill_dlg::widgets::Caret| match how {
			Place::Set => c.place(pos, text.len(), false),
			Place::Extend => c.place(pos, text.len(), true),
			Place::Word => c.select_word(text, pos),
		};
		match &mut self.mode {
			Mode::Prompt {
				buf, len, caret, ..
			} => buf.get(..*len).map(|t| place(t, caret)).is_some(),
			Mode::Find(st) => st
				.field()
				.and_then(|(b, l, c)| b.get(..*l).map(|t| place(t, c)))
				.is_some(),
			Mode::Form(fm) => fm
				.rows
				.get_mut(fm.focus)
				.filter(|r| r.kind == 0)
				.and_then(|r| r.text.get(..r.len).map(|t| place(t, &mut r.caret)))
				.is_some(),
			_ => false,
		}
	}

	/// The index in the face list of the face at the caret, if it is one of them.
	pub fn face_index<S: ReadStore + ?Sized>(&self, s: &S) -> Option<usize> {
		let cs = self.char_state(s);
		let face = cs.face.get(..usize::from(cs.face_len))?;
		FONT_NAMES.iter().position(|n| n.as_bytes() == face)
	}
}

/// The face the face list offers at `i`, and its family code.
pub fn face_choice(i: usize) -> Option<(&'static str, u8)> {
	Some((*FONT_NAMES.get(i)?, *FONT_FAMILY.get(i)?))
}

/// How many faces the face list offers.
pub fn face_choices() -> usize {
	FONT_NAMES.len()
}
