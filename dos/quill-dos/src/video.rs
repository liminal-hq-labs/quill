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
static mut SAVED_ROWS: u8 = 25;
static mut BLOCK_CURSOR: u16 = 0x000F;
static mut SAVED_CURSOR: u16 = 0x0607;
static mut ACTIVE: bool = false;

fn screen() -> *mut u16 {
	dpmi::linear_to_ptr::<u16>(0xB8000)
}

/// The number of text rows the BIOS reports (the byte at 0040:0084 holds the last row).
fn bios_rows() -> u8 {
	// SAFETY: the BIOS data area is in the first megabyte, which the host maps at linear 0.
	unsafe { dpmi::linear_to_ptr::<u8>(0x484).read_volatile().saturating_add(1) }
}

/// Takes over the screen: remembers the mode and the cursor, sets 80 by 25 colour text (or 80 by 50 when `rows` is 50 and the
/// adapter's 8 by 8 font gives it) with bright backgrounds instead of blinking, hides the cursor and registers the way back with
/// the clean-up list. Returns the rows the screen has, 25 or 50, so a card with no 50-row text mode leaves the caller at 25.
pub fn enter(rows: usize) -> usize {
	// SAFETY: single-threaded; the statics are touched only here and in `leave`, which `cleanup` may run from the panic path.
	unsafe {
		if ACTIVE {
			return usize::from(bios_rows());
		}
		let (mode, _, _) = int10(0x0F00, 0, 0, 0);
		SAVED_MODE = (mode as u8) & 0x7F;
		SAVED_ROWS = bios_rows();
		let (_, cursor, _) = int10(0x0300, 0, 0, 0);
		SAVED_CURSOR = cursor as u16;
		ACTIVE = true;
	}
	int10(0x0003, 0, 0, 0);
	let mut got = 25;
	if rows == 50 {
		// AX=1112h: load the 8 by 8 ROM font and reprogram the controller for it, which gives 50 rows on a VGA.
		int10(0x1112, 0, 0, 0);
		if bios_rows() == 50 {
			got = 50;
		} else {
			int10(0x0003, 0, 0, 0);
		}
	}
	// SAFETY: as above.
	unsafe {
		// An 8-line cell: the underline (lines 6 and 7) still fits, and the block stops at line 7.
		BLOCK_CURSOR = if got == 50 { 0x0007 } else { 0x000F };
	}
	// Background intensity instead of blink, so attributes 80h to FFh are bright backgrounds.
	int10(0x1003, 0, 0, 0);
	hide_cursor();
	cleanup::register(leave);
	got
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
		if SAVED_ROWS == 50 {
			int10(0x1112, 0, 0, 0);
		}
		int10(0x0100, 0, u32::from(SAVED_CURSOR), 0);
	}
	cleanup::forget(leave);
}

/// Shows the grid.
pub fn present<const ROWS: usize>(grid: &Grid<ROWS>) {
	enter(ROWS);
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

/// Shapes the hardware cursor: a line under the character for insert, a block for overtype.
pub fn cursor_shape(overtype: bool) {
	// SAFETY: written only by `enter`, on this thread.
	let block = unsafe { BLOCK_CURSOR };
	int10(0x0100, 0, u32::from(if overtype { block } else { 0x0607 }), 0);
}

/// Shows the pointer on the text screen: the cell at (column, row) with its colours swapped. `present` has just written every cell, so
/// there is nothing to take back; the next `present` does that.
pub fn pointer(at: (i32, i32), rows: usize) {
	let (x, y) = (at.0.clamp(0, 79) as usize, at.1.clamp(0, rows as i32 - 1) as usize);
	let cell = screen().wrapping_add(y * 80 + x);
	// SAFETY: inside the text page, as `present`.
	unsafe {
		let w = cell.read_volatile();
		let a = (w >> 8) as u8;
		cell.write_volatile((w & 0xFF) | (u16::from(a.rotate_left(4)) << 8));
	}
}
