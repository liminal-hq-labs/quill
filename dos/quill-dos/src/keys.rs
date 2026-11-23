// The keyboard through the BIOS (INT 16h).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// INT 16h AH=00h gives an ASCII code in AL and a scan code in AH; AL is 0 (or 0xE0) for the extended keys. Control keys come as
// the codes 01h to 1Ah, so the scan code tells Enter, Backspace and Tab from Ctrl+M, Ctrl+H and Ctrl+I.

use core::arch::asm;

use quill_app::Key;

/// Waits for the next key and decodes it; keys Quill has no use for come back as `None`.
pub fn wait() -> Option<Key> {
	let ax: u32;
	// SAFETY: INT 16h is a BIOS service with register arguments only.
	unsafe { asm!("int 0x16", inout("eax") 0u32 => ax) };
	decode(ax as u8, (ax >> 8) as u8)
}

/// Turns a BIOS key (ASCII code, scan code) into a `Key`.
pub fn decode(ascii: u8, scan: u8) -> Option<Key> {
	Some(match (ascii, scan) {
		(0 | 0xE0, 0x48) => Key::Up,
		(0 | 0xE0, 0x50) => Key::Down,
		(0 | 0xE0, 0x4B) => Key::Left,
		(0 | 0xE0, 0x4D) => Key::Right,
		(0 | 0xE0, 0x47) => Key::Home,
		(0 | 0xE0, 0x4F) => Key::End,
		(0 | 0xE0, 0x49) => Key::PageUp,
		(0 | 0xE0, 0x51) => Key::PageDown,
		(0 | 0xE0, 0x53) => Key::Delete,
		(0, 0x77) => Key::CtrlHome,
		(0, 0x75) => Key::CtrlEnd,
		(0, 0x3B..=0x44) => Key::F(scan - 0x3B + 1),
		(0, 0x85) => Key::F(11),
		(0, 0x86) => Key::F(12),
		(0x0D, _) => Key::Enter,
		(0x08, _) => Key::Backspace,
		(0x09, _) => Key::Tab,
		(0x1B, _) => Key::Esc,
		(0x01..=0x1A, _) => Key::Ctrl(ascii + 0x60),
		(0x20..=0xFF, _) => Key::Char(ascii),
		_ => return None,
	})
}
