// Editing a dialog's text field at a caret: the keys of the app's own fields, through the shared field logic of `quill-dlg`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use quill_dlg::key::DlgKey;
use quill_dlg::widgets::Caret;

use crate::{Dir, Key};

/// The dialog key a field takes for `k`: characters, Backspace, Delete, the arrows and Home and End, and those with Shift.
fn dlg_key(k: Key) -> Option<DlgKey> {
	Some(match k {
		Key::Char(c) => DlgKey::Char(c),
		Key::Backspace => DlgKey::Backspace,
		Key::Delete => DlgKey::Delete,
		Key::Left => DlgKey::Left,
		Key::Right => DlgKey::Right,
		Key::Home => DlgKey::Home,
		Key::End => DlgKey::End,
		Key::Select(Dir::Left) => DlgKey::ShiftLeft,
		Key::Select(Dir::Right) => DlgKey::ShiftRight,
		Key::Select(Dir::Home) => DlgKey::ShiftHome,
		Key::Select(Dir::End) => DlgKey::ShiftEnd,
		_ => return None,
	})
}

/// Gives key `k` to the field `buf[..*len]` with its caret. `accepts` says which characters it takes (and may change them). True
/// when the key was one of the field's: a character, whether taken or not, or an editing or movement key.
pub(crate) fn field_key(
	k: Key,
	buf: &mut [u8],
	len: &mut usize,
	caret: &mut Caret,
	accepts: &dyn Fn(u8) -> Option<u8>,
) -> bool {
	let Some(d) = dlg_key(k) else {
		return false;
	};
	caret.key(d, buf, len, accepts, &mut false)
}
