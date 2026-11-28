// Shortcut keys from virtual-key codes: the Ctrl combinations that a character message cannot tell from Tab, Backspace and Enter.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A Windows program that takes its shortcuts from the character messages gets the wrong key for four of them: with Ctrl held, the
//! system turns Ctrl+H into the code for Backspace, Ctrl+I into Tab, Ctrl+J into Line Feed and Ctrl+M into Enter, so Italic inserts a
//! tab and Replace deletes a character. A shell that reads the key-down message, where the virtual-key code and the shift state
//! are separate, asks this table instead and drops the character messages that Ctrl produced. The virtual-key codes are the same on
//! Windows 3.1 and on 32-bit Windows.

use crate::{Command, Key};

/// What a shortcut does: a key the editor's key handler takes, or a command.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Accel {
	/// Pass this key to the editor.
	Key(Key),
	/// Run this command.
	Command(Command),
}

/// The virtual-key codes the table reads.
mod vk {
	pub const BACK: u16 = 0x08;
	pub const INSERT: u16 = 0x2D;
	pub const DELETE: u16 = 0x2E;
	pub const A: u16 = 0x41;
	pub const Z: u16 = 0x5A;
	pub const OEM_PLUS: u16 = 0xBB;
	pub const NUMPAD0: u16 = 0x60;
	pub const ADD: u16 = 0x6B;
	pub const SUBTRACT: u16 = 0x6D;
	pub const OEM_MINUS: u16 = 0xBD;
}

/// The shortcut for virtual key `code` with the given shift state, if it is one. A key that is not a shortcut (a letter typed with
/// no Ctrl, an arrow, a function key) is `None`, and the shell takes it the way it always did.
pub fn from_vk(code: u16, ctrl: bool, shift: bool, alt: bool) -> Option<Accel> {
	if alt && !ctrl {
		return match code {
			vk::BACK => Some(Accel::Command(Command::Undo)),
			_ => None,
		};
	}
	if alt {
		// Ctrl and Alt together is how some keyboards type AltGr characters: they are text, not shortcuts.
		return None;
	}
	if !ctrl {
		return match (code, shift) {
			(vk::DELETE, true) => Some(Accel::Command(Command::Cut)),
			(vk::INSERT, true) => Some(Accel::Command(Command::Paste)),
			_ => None,
		};
	}
	match code {
		vk::A..=vk::Z => Some(Accel::Key(Key::Ctrl(b'a' + (code - vk::A) as u8))),
		// Ctrl+0 and Ctrl+numpad 0 (Num Lock on) are Page Width. With Num Lock off the numpad's 0 is VK_INSERT, which stays Copy.
		0x30 | vk::NUMPAD0 => Some(Accel::Command(Command::Zoom(0))),
		// Ctrl+numpad plus is Zoom In; Ctrl+numpad minus and Ctrl+minus are Zoom Out. Ctrl+= is Subscript, so there is no Ctrl+plus.
		vk::ADD => Some(Accel::Command(Command::ZoomIn)),
		vk::SUBTRACT | vk::OEM_MINUS => Some(Accel::Command(Command::ZoomOut)),
		0x31 => Some(Accel::Command(Command::Spacing(0))),
		0x32 => Some(Accel::Command(Command::Spacing(2))),
		0x35 => Some(Accel::Command(Command::Spacing(1))),
		vk::OEM_PLUS if shift => Some(Accel::Command(Command::Super)),
		vk::OEM_PLUS => Some(Accel::Command(Command::Sub)),
		vk::INSERT => Some(Accel::Command(Command::Copy)),
		_ => None,
	}
}

/// True for the control characters (1 to 26, Escape and 1Fh, which Ctrl+minus makes) that the system produces for a letter typed with
/// Ctrl, and for the plus sign, minus sign and zero that a keypad key typed with Ctrl can still produce: the character message of a
/// shortcut, which a shell that handled the key-down must not insert as text (the zoom keys).
pub fn is_ctrl_character(c: u8) -> bool {
	(1..=26).contains(&c) || c == 0x1B || c == 0x1F || c == 0x7F || matches!(c, b'+' | b'-' | b'0')
}
