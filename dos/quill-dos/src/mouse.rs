// The mouse: INT 33h through the driver DOS has (or DOSBox's own), polled from the main loop, and the text screen's pointer cell.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `quill_mouse::Mouse` does the driver's protocol against the `Int33` trait; this is the trait over the real interrupt. A program
//! with no driver (the vector empty, or pointing at an `IRET`) gets `None` from `open` and runs keyboard-only. The graphics arrow is
//! composited by `gfx::Screen`; on the text screen the pointer is the cell under it with its colours swapped.

use core::arch::asm;

use quill_mouse::{Ev, Int33, Mouse, Regs};
use rt::dpmi;

use crate::ps2::Ps2Mouse;

/// INT 33h as a software interrupt from protected mode: the host reflects it to the real-mode driver, and these functions take only
/// registers.
pub struct DosInt33;

impl DosInt33 {
	/// True when something answers INT 33h: the vector is not empty and does not point straight at an `IRET`.
	fn present() -> bool {
		let (offset, segment) = {
			let p = dpmi::linear_to_ptr::<u16>(0x33 * 4);
			// SAFETY: the interrupt vector table is the first kilobyte of the first megabyte, which the host maps at linear 0.
			unsafe { (p.read_volatile(), p.add(1).read_volatile()) }
		};
		if segment == 0 && offset == 0 {
			return false;
		}
		let linear = (u32::from(segment) << 4) + u32::from(offset);
		// SAFETY: as above; the vector points into the first megabyte.
		linear < 0x10_0000 && unsafe { dpmi::linear_to_ptr::<u8>(linear).read_volatile() } != 0xCF
	}
}

impl Int33 for DosInt33 {
	fn call(&mut self, r: &mut Regs) -> bool {
		let (bx, cx, dx, ax): (u32, u32, u32, u32);
		// SAFETY: INT 33h takes and gives registers only; EBX is swapped through a scratch register because the compiler keeps it.
		unsafe {
			asm!(
				"xchg {b:e}, ebx",
				"int 0x33",
				"xchg {b:e}, ebx",
				b = inout(reg) u32::from(r.bx) => bx,
				inout("eax") u32::from(r.ax) => ax,
				inout("ecx") u32::from(r.cx) => cx,
				inout("edx") u32::from(r.dx) => dx,
			);
		}
		*r = Regs {
			ax: ax as u16,
			bx: bx as u16,
			cx: cx as u16,
			dx: dx as u16,
		};
		true
	}
}

/// The mouse the program has: a driver's, or the PS/2 mouse read directly when there is no driver.
pub enum Mice {
	Int33(Mouse<DosInt33>),
	Ps2(Ps2Mouse),
}

impl Mice {
	/// Turns what the mouse did since the last call into events.
	pub fn poll(&mut self, emit: impl FnMut(Ev)) {
		match self {
			Mice::Int33(m) => m.poll(emit),
			Mice::Ps2(m) => m.poll(emit),
		}
	}

	/// Where the pointer is, in shell units.
	pub fn at(&self) -> (i32, i32) {
		match self {
			Mice::Int33(m) => m.at(),
			Mice::Ps2(m) => m.at(),
		}
	}

	/// Gives the mouse back to whatever runs next.
	pub fn close(self) {
		match self {
			Mice::Int33(m) => m.close(),
			Mice::Ps2(m) => m.close(),
		}
	}
}

/// Finds the driver and limits it to `w` by `h` units, `div` driver units to each (8 by 8 in text, where a cell is eight of the driver's
/// virtual pixels; 1 by 1 in graphics); with no driver, the PS/2 mouse itself. `None` when there is no mouse.
pub fn open(w: i32, h: i32, div: (i32, i32)) -> Option<Mice> {
	if DosInt33::present() {
		if let Some(m) = Mouse::open(DosInt33, w, h, div) {
			return Some(Mice::Int33(m));
		}
	}
	Ps2Mouse::open(w, h, div).map(Mice::Ps2)
}

/// The BIOS tick count at 0040:006C, which counts at 18.2 Hz: the clock for double clicks.
pub fn ticks() -> u32 {
	// SAFETY: the BIOS data area is in the first megabyte, which the host maps at linear 0.
	unsafe { dpmi::linear_to_ptr::<u32>(0x46C).read_volatile() }
}
