// The text screen: mode 3 through the BIOS, cells written straight to B8000h (from the installer's DOS runtime).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The first megabyte is mapped at linear 0 by the DPMI host, so the video memory is B8000h minus the image base through the 4 GiB
// data segment, as in `rt`'s other near pointers. INT 10h calls take only registers, which the host reflects to the BIOS.

use core::arch::asm;

use quill_grid::Grid;
use rt::{cleanup, dpmi};

fn int10(ax: u32, bx: u32, cx: u32, dx: u32) -> (u32, u32, u32) {
	let (a, c, d): (u32, u32, u32);
	// SAFETY: INT 10h is a BIOS service with register arguments only; EBX is saved by hand because the compiler keeps it.
	unsafe {
		asm!(
			"push ebx",
			"mov ebx, {b}",
			"int 0x10",
			"pop ebx",
			b = in(reg) bx,
			inout("eax") ax => a,
			inout("ecx") cx => c,
			inout("edx") dx => d,
		);
	}
	(a, c, d)
}

static mut SAVED_MODE: u8 = 3;
static mut SAVED_CURSOR: u16 = 0x0607;
static mut ACTIVE: bool = false;

fn screen() -> *mut u16 {
	dpmi::linear_to_ptr::<u16>(0xB8000)
}

/// Takes over the screen: remembers the mode and the cursor, sets 80 by 25 colour text with bright backgrounds instead of
/// blinking, hides the cursor and registers the way back with the clean-up list.
pub fn enter() {
	// SAFETY: single-threaded; the statics are touched only here and in `leave`, which `cleanup` may run from the panic path.
	unsafe {
		if ACTIVE {
			return;
		}
		let (mode, _, _) = int10(0x0F00, 0, 0, 0);
		SAVED_MODE = (mode as u8) & 0x7F;
		let (_, cursor, _) = int10(0x0300, 0, 0, 0);
		SAVED_CURSOR = cursor as u16;
		ACTIVE = true;
	}
	int10(0x0003, 0, 0, 0);
	// Background intensity instead of blink, so attributes 80h to FFh are bright backgrounds.
	int10(0x1003, 0, 0, 0);
	hide_cursor();
	cleanup::register(leave);
}

fn hide_cursor() {
	int10(0x0100, 0, 0x2000, 0);
}

/// Gives the screen back: the saved mode (which clears it) and the saved cursor. Safe to call twice.
pub fn leave() {
	// SAFETY: see `enter`.
	unsafe {
		if !ACTIVE {
			return;
		}
		ACTIVE = false;
		int10(u32::from(SAVED_MODE), 0, 0, 0);
		int10(0x0100, 0, u32::from(SAVED_CURSOR), 0);
	}
	cleanup::forget(leave);
}

/// Shows the grid.
pub fn present<const ROWS: usize>(grid: &Grid<ROWS>) {
	enter();
	let vram = screen();
	for (i, word) in grid.words().enumerate() {
		// SAFETY: B8000h onward is the text page: 4000 bytes at 80 by 25 and 8000 at 80 by 50.
		unsafe { vram.add(i).write_volatile(word) };
	}
}

#[allow(dead_code)]
/// Puts the hardware cursor at (column, row), or hides it.
pub fn cursor(at: Option<(u8, u8)>) {
	match at {
		Some((x, y)) => {
			int10(0x0200, 0, 0, (u32::from(y) << 8) | u32::from(x));
			int10(0x0100, 0, 0x0607, 0);
		}
		None => hide_cursor(),
	}
}

/// Reads the screen back from video memory, so a dump shows what is on the screen and not what the program meant to draw.
pub fn read_back<const ROWS: usize>() -> Grid<ROWS> {
	let vram = screen();
	let mut g = Grid::<ROWS>::new(0);
	// SAFETY: as `present`.
	g.load_words(|i| unsafe { vram.add(i).read_volatile() });
	g
}
