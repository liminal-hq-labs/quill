// What the shells ask of the editor when the operating system ends the session: whether there is unsaved work, whether the
// shutdown may go on, and a copy of the document written aside when it does.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Windows sends `WM_QUERYENDSESSION` before it logs off or shuts down, and `WM_ENDSESSION` after. Both Windows shells answer the first
// with the same Save, Don't Save, Cancel question as File > Exit, and the verdict is `end_session_allowed`, here, so the two agree
// (CORE-7). `unsaved_work` is the one dirty signal: the shells' shutdown block follows it, and they act only when it changes.
use quill_core::layout::Measure;
use quill_core::store::Store;
use quill_formats::qld;

use crate::{App, Files};

/// The answer a Save / Don't Save / Cancel question returns: the Windows `IDYES` and `IDNO`. Anything else is a cancel.
pub const ANSWER_SAVE: u32 = 6;
/// See `ANSWER_SAVE`.
pub const ANSWER_DISCARD: u32 = 7;

/// True when the session may end. A document with no unsaved work always lets it; otherwise Save lets it only when the save worked
/// (`saved`), Don't Save lets it, and a cancel or any other answer blocks it.
pub fn end_session_allowed(unsaved: bool, answer: u32, saved: bool) -> bool {
	if !unsaved {
		return true;
	}
	match answer {
		ANSWER_SAVE => saved,
		ANSWER_DISCARD => true,
		_ => false,
	}
}

impl<M: Measure + Clone> App<M> {
	/// True when the document holds changes that are not on disk: what a shell blocks a shutdown for.
	pub fn unsaved_work(&self) -> bool {
		self.modified
	}

	/// Writes the document, in the native format, to `name` without renaming it, clearing its modified flag or touching the recent
	/// list: the copy a shell leaves when the session is ending and the person cannot be asked. False when nothing was written.
	pub fn save_recovery<S: Store + ?Sized, F: Files>(
		&self,
		s: &mut S,
		f: &mut F,
		name: &[u8],
	) -> bool {
		if !self.modified {
			return false;
		}
		let Some(mut w) = f.create(name) else {
			return false;
		};
		let ok = qld::write(&self.doc, s, &mut w).is_ok();
		f.close(w) && ok
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn a_clean_document_never_blocks() {
		for a in [0, 2, ANSWER_SAVE, ANSWER_DISCARD] {
			assert!(end_session_allowed(false, a, false));
		}
	}

	#[test]
	fn a_modified_document_blocks_on_cancel_and_on_a_failed_save() {
		assert!(end_session_allowed(true, ANSWER_SAVE, true));
		assert!(!end_session_allowed(true, ANSWER_SAVE, false));
		assert!(end_session_allowed(true, ANSWER_DISCARD, false));
		assert!(!end_session_allowed(true, 2, true));
		assert!(!end_session_allowed(true, 0, true));
	}
}
