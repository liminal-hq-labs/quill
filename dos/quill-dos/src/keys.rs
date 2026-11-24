// The keyboard through the BIOS (INT 16h).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// INT 16h AH=00h gives an ASCII code in AL and a scan code in AH; `quill_app::bios::decode` turns the pair into a key, using the scan
// code to tell Enter, Backspace and Tab from Ctrl+M, Ctrl+H and Ctrl+I.

use core::arch::asm;
use core::sync::atomic::{AtomicBool, Ordering};

use quill_app::{Dir, Key};

/// Set when a key was typed while Alt was held, so that letting Alt go afterwards is the end of a chord and not a tap.
static ALT_USED: AtomicBool = AtomicBool::new(false);

/// Waits for the next key and decodes it; keys Quill has no use for come back as `None`. Alt pressed and let go with no key between
/// comes back as F10, which is how Alt activates the menu bar: the BIOS reports only the state of Alt, so this polls it while it waits.
pub fn wait() -> Option<Key> {
	let mut alt_down = false;
	while !key_ready() {
		if alt_flag() {
			alt_down = true;
		} else if alt_down {
			// Alt went up; a key that Alt let through (a keypad code) may have arrived in the same instant.
			if ALT_USED.swap(false, Ordering::Relaxed) {
				alt_down = false;
			} else if !key_ready() {
				return Some(Key::F(10));
			} else {
				break;
			}
		} else {
			ALT_USED.store(false, Ordering::Relaxed);
		}
		idle();
	}
	if alt_flag() {
		ALT_USED.store(true, Ordering::Relaxed);
	}
	let ax: u32;
	// SAFETY: INT 16h is a BIOS service with register arguments only.
	unsafe { asm!("int 0x16", inout("eax") 0u32 => ax) };
	let key = quill_app::bios::decode(ax as u8, (ax >> 8) as u8)?;
	if shift_down() {
		let dir = match key {
			Key::Left => Dir::Left,
			Key::Right => Dir::Right,
			Key::Up => Dir::Up,
			Key::Down => Dir::Down,
			Key::Home => Dir::Home,
			Key::End => Dir::End,
			Key::PageUp => Dir::PageUp,
			Key::PageDown => Dir::PageDown,
			other => return Some(other),
		};
		return Some(Key::Select(dir));
	}
	Some(key)
}

/// True while a Shift key is held (INT 16h AH=02h, bits 0 and 1 of the shift flags).
fn shift_down() -> bool {
	let ax: u32;
	// SAFETY: INT 16h is a BIOS service with register arguments only.
	unsafe { asm!("int 0x16", inout("eax") 0x0200u32 => ax) };
	ax & 3 != 0
}

/// True when a key is waiting in the BIOS buffer (INT 16h AH=01h clears ZF when there is one).
fn key_ready() -> bool {
	let ready: u32;
	// SAFETY: INT 16h is a BIOS service with register arguments only; the flags come back in the byte after `lahf`.
	unsafe {
		asm!("int 0x16", "lahf", "movzx {r:e}, ah", r = out(reg) ready, inout("eax") 0x0100u32 => _);
	}
	ready & 0x40 == 0
}

/// True while an Alt key is held (bit 3 of the shift flags, INT 16h AH=02h).
fn alt_flag() -> bool {
	let ax: u32;
	// SAFETY: INT 16h is a BIOS service with register arguments only.
	unsafe { asm!("int 0x16", inout("eax") 0x0200u32 => ax) };
	ax & 8 != 0
}

/// Gives the rest of the time slice back (INT 2Fh AX=1680h), so waiting for a key does not spin an emulator's or a multitasker's CPU.
fn idle() {
	// SAFETY: INT 2Fh is a DOS service with register arguments only; a host that has no idle call leaves AL at 80h and does nothing.
	unsafe { asm!("int 0x2f", inout("eax") 0x1680u32 => _) };
}
