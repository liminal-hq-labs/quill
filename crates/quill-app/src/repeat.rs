// Held keys: which keys keep acting while they are held down, and which act once per press.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A held key is sent again by the keyboard (typematic) or by the system (the repeat flag of `WM_KEYDOWN`, a repeated GTK key event).
//! Text and movement are meant to repeat; a toggle (Bold, Italic, Underline, Insert, the View items) and a command that opens
//! something (Save, Find, a menu) are not: held for a second, Ctrl+I would turn italic on and off again and again. One rule decides
//! it for every shell. A shell that can tell a repeat from a press asks `key_repeats` or `Command::repeats` before it passes the
//! key on, and drops the repeat when the answer is no; a shell that cannot tell passes every key and gets the old behaviour.
//!
//! What repeats: characters, Tab and Enter (typing), Backspace and Delete, the cursor, Home and End, the page keys and the selecting
//! forms of them, and Undo and Redo (Word repeats them too). The zoom steps also repeat. In a menu only the arrows repeat, and in a
//! dialog only the text and the movement of its fields; a key that answers a question, such as Enter or `Y`, never does.

use crate::{App, Command, Key, Measure, Mode};

impl Command {
	/// True when the command may run again while its key is held: the ones that step through a history or a size.
	pub fn repeats(self) -> bool {
		matches!(self, Command::Undo | Command::Redo | Command::Zoom(_))
	}
}

/// True for the keys that move the caret or delete around it.
fn motion(k: Key) -> bool {
	matches!(
		k,
		Key::Left
			| Key::Right
			| Key::Up | Key::Down
			| Key::Home
			| Key::End
			| Key::PageUp
			| Key::PageDown
			| Key::CtrlHome
			| Key::CtrlEnd
			| Key::Select(_)
			| Key::Backspace
			| Key::Delete
	)
}

impl<M: Measure + Clone> App<M> {
	/// True when key `k` may act again while it is held, in the mode the editor is in. A shell that knows a key is a repeat passes it on
	/// only when this says yes.
	pub fn key_repeats(&self, k: Key) -> bool {
		match self.mode {
			Mode::Edit => {
				motion(k)
					|| matches!(
						k,
						Key::Char(_) | Key::Tab | Key::Enter | Key::Ctrl(b'z' | b'y')
					)
			}
			Mode::Menu { .. } => matches!(k, Key::Left | Key::Right | Key::Up | Key::Down),
			// A space may flip a check box, and Tab and Enter move on or answer.
			Mode::Prompt { .. } | Mode::Find(_) | Mode::Form(_) => {
				motion(k) || matches!(k, Key::Char(c) if c != b' ')
			}
			#[cfg(feature = "text-file-dialog")]
			Mode::File => motion(k) || matches!(k, Key::Char(c) if c != b' '),
			_ => false,
		}
	}

	/// Handles a key that is a repeat of one still held: it acts only when `key_repeats` allows it.
	pub fn key_held<S: crate::Store + ?Sized, F: crate::Files>(
		&mut self,
		s: &mut S,
		f: &mut F,
		k: Key,
	) {
		if self.key_repeats(k) {
			self.key(s, f, k);
		}
	}
}
