// The keys a dialog understands.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// One key press, as the dialogs see it. `Key` lives in `quill-app`, which depends on this crate, so the shell maps its own keys
/// to these; a key with no entry here is not passed to a dialog at all.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DlgKey {
	/// A printable character, as typed (any case)
	Char(u8),
	/// Alt with a letter or digit, for mnemonics
	Alt(u8),
	Enter,
	Esc,
	Tab,
	BackTab,
	Backspace,
	Delete,
	Insert,
	Up,
	Down,
	Left,
	Right,
	Home,
	End,
	PgUp,
	PgDn,
	/// F4, which opens a drop-down list
	F4,
	/// Alt+Down, which opens a drop-down list
	AltDown,
	ShiftLeft,
	ShiftRight,
	ShiftHome,
	ShiftEnd,
}

#[cfg(test)]
#[allow(
	clippy::unwrap_used,
	clippy::expect_used,
	clippy::panic,
	clippy::indexing_slicing
)]
pub(crate) mod tests {
	use super::DlgKey;
	use proptest::prelude::*;

	/// Any key, with printable characters weighted so fields fill up.
	pub fn any_key() -> impl Strategy<Value = DlgKey> {
		prop_oneof![
			4 => (0x20u8..0x7F).prop_map(DlgKey::Char),
			1 => any::<u8>().prop_map(DlgKey::Char),
			1 => (0x30u8..0x7B).prop_map(DlgKey::Alt),
			1 => Just(DlgKey::Enter),
			1 => Just(DlgKey::Esc),
			2 => Just(DlgKey::Tab),
			1 => Just(DlgKey::BackTab),
			2 => Just(DlgKey::Backspace),
			1 => Just(DlgKey::Delete),
			1 => Just(DlgKey::Insert),
			2 => Just(DlgKey::Up),
			2 => Just(DlgKey::Down),
			2 => Just(DlgKey::Left),
			2 => Just(DlgKey::Right),
			1 => Just(DlgKey::Home),
			1 => Just(DlgKey::End),
			1 => Just(DlgKey::PgUp),
			1 => Just(DlgKey::PgDn),
			1 => Just(DlgKey::F4),
			1 => Just(DlgKey::AltDown),
			1 => Just(DlgKey::ShiftLeft),
			1 => Just(DlgKey::ShiftRight),
			1 => Just(DlgKey::ShiftHome),
			1 => Just(DlgKey::ShiftEnd),
		]
	}
}
