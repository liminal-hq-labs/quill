// The recovery question as a box in the app's own cells, for the builds that draw their dialogs themselves (the DOS screens).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The Windows builds show the question in a message box. A build with no message box turns on `text-recovery`, and the question that
// `take_recovery_question` hands over becomes `Mode::Recover`: the words wrapped in a box, with the question's buttons under them.
// The keys, the pointer and the graphics screen's widgets all answer through `recovery_answered`, so the files are handled in one place
// (CORE-13). Escape is the question's Cancel, which at launch is Later and at Open is Open original, so it never deletes anything.
use quill_core::layout::Measure;
use quill_core::store::Store;

use crate::flow::{Answer, Ask, Buttons};
use crate::{App, Files, Key, Mode};

/// Columns the words wrap to.
pub(crate) const WRAP: usize = 58;
/// Lines the words may take.
pub(crate) const LINES: usize = 6;
/// Columns of the box, and the one its text starts in.
pub(crate) const BOX_W: usize = 62;
pub(crate) const BOX_X: usize = 9;
/// The row the box's top edge is on.
pub(crate) const BOX_Y: usize = 8;

/// The buttons of a question, left to right, with what each answers.
pub(crate) fn buttons(b: Buttons) -> &'static [(&'static [u8], Answer)] {
	match b {
		Buttons::RecoverDiscardLater => &[
			(b"Recover", Answer::Yes),
			(b"Discard", Answer::No),
			(b"Later", Answer::Cancel),
		],
		Buttons::RecoverOriginalDiscard => &[
			(b"Recover", Answer::Yes),
			(b"Open original", Answer::Cancel),
			(b"Discard", Answer::No),
		],
		Buttons::YesNo => &[(b"Yes", Answer::Yes), (b"No", Answer::No)],
		Buttons::YesNoCancel => &[
			(b"Yes", Answer::Yes),
			(b"No", Answer::No),
			(b"Cancel", Answer::Cancel),
		],
	}
}

/// The words cut into lines at spaces: how many lines, and each one's start and end. Words that do not fit in `LINES` are dropped.
pub(crate) fn wrap(text: &[u8]) -> (usize, [(usize, usize); LINES]) {
	let mut lines = [(0, 0); LINES];
	let (mut n, mut start) = (0, 0);
	while start < text.len() && n < LINES {
		let mut end = (start + WRAP).min(text.len());
		if end < text.len() {
			if let Some(sp) = text
				.get(start..end)
				.and_then(|t| t.iter().rposition(|b| *b == b' '))
			{
				end = start + sp.max(1);
			}
		}
		if let Some(l) = lines.get_mut(n) {
			*l = (start, end);
		}
		n += 1;
		start = end;
		while text.get(start) == Some(&b' ') {
			start += 1;
		}
	}
	(n, lines)
}

/// The box's height in rows: the title, a blank, the lines, a blank, the buttons and the bottom edge.
pub(crate) fn box_h(lines: usize) -> usize {
	lines + 5
}

/// The column and width of button `i` as the text screen draws it (`< Recover >`), counted from the box's left edge.
pub(crate) fn button_at(list: &[(&[u8], Answer)], i: usize) -> (usize, usize) {
	let mut x = 2;
	for (k, (label, _)) in list.iter().enumerate() {
		let w = label.len() + 4;
		if k == i {
			return (x, w);
		}
		x += w + 2;
	}
	(x, 0)
}

impl<M: Measure + Clone> App<M> {
	/// Shows the question that is waiting, if there is one and nothing else is open: the launch check and an Open that found a copy
	/// raise it, and a shell calls this after the launch check. Does nothing when dialogs are native.
	pub fn recovery_prompt<S: Store + ?Sized, F: Files>(&mut self, s: &mut S, f: &mut F) {
		if self.native || self.mode != Mode::Edit || !self.recovery_pending() {
			return;
		}
		if let Some(ask) = self.take_recovery_question(s, f) {
			self.mode = Mode::Recover(ask, 0);
		}
	}

	pub(crate) fn key_recover<S: Store + ?Sized, F: Files>(
		&mut self,
		s: &mut S,
		f: &mut F,
		k: Key,
		ask: Ask,
		focus: u8,
	) {
		let list = buttons(ask.buttons);
		let n = list.len() as u8;
		let chosen = match k {
			Key::Enter => list.get(usize::from(focus)).map(|b| b.1),
			Key::Esc => Some(Answer::Cancel),
			Key::Tab | Key::Right => {
				self.mode = Mode::Recover(ask, (focus + 1) % n);
				None
			}
			Key::Left => {
				self.mode = Mode::Recover(ask, (focus + n - 1) % n);
				None
			}
			Key::Char(c) => list
				.iter()
				.find(|b| b.0.first().is_some_and(|l| l.eq_ignore_ascii_case(&c)))
				.map(|b| b.1),
			_ => None,
		};
		if let Some(a) = chosen {
			self.mode = Mode::Edit;
			self.recovery_answered(s, f, a);
		}
	}
}
