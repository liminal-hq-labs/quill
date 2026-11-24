// The graphics screen: VGA mode 12h or a VBE 640 by 480 mode with 256 colours, a shadow in the device's own format and a presenter.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The shell draws the whole screen into a RAM shadow (`quill-paint`'s surfaces) and presents the rows whose hash changed:
//!
//! - **Mode 12h** (INT 10h AX=0012h, the identity attribute palette, the Windows 16 colours in the DAC): the shadow is the four planes
//!   one after the other, so a changed run of rows is one string move per plane with the map mask set to that plane. Video memory is
//!   written with whole bytes and never read, except by the dump.
//! - **VBE** (4F00h to 4F02h through DPMI 0300h, the frame buffer mapped with 0800h, the 6 x 6 x 6 cube and eight Windows colours in
//!   the DAC): the shadow is one byte a pixel, and a run of rows is one string move (or one a row when the pitch is wider).
//!
//! Entering registers `leave` with the runtime's clean-up list first, so a fatal error or a panic after the mode is set still
//! restores the text mode and unmaps the frame buffer.

use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use desk_hal::x86::RawPorts;
use desk_hal::{Bios, BiosRegs, PortIo, Rgb6};
use desk_vga::vbe::Vbe;
use desk_vga::{dac, regs};
use quill_paint::crc32_update;
use quill_paint::damage::{self, RowSet, LINEAR_ROW, PLANAR_ROW, ROWS};
use quill_paint::palette::{rgb256, WINDOWS16};
use quill_paint::raster::{PLANAR_BYTES, PLANE_BYTES};
use quill_paint::shell::Graphics;
use rt::bios::DosBios;
use rt::{cleanup, dpmi};

static ACTIVE: AtomicBool = AtomicBool::new(false);
static SAVED_MODE: AtomicU32 = AtomicU32::new(3);
/// The linear address of the VBE frame buffer mapping, or 0.
static MAPPED: AtomicU32 = AtomicU32::new(0);
static TIMER: AtomicBool = AtomicBool::new(false);

/// Which mode the screen is in.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Kind {
	/// VGA mode 12h: 640 by 480, 16 colours, four planes.
	Planar16,
	/// A VBE mode: 640 by 480, 256 colours, a linear frame buffer.
	Linear256,
}

/// The screen in graphics mode: its shadow and how to present it.
pub struct Screen {
	/// The mode.
	pub kind: Kind,
	/// The shadow, in the device's own format: 153,600 bytes of planes, or 640 by 480 bytes.
	pub shadow: Vec<u8>,
	/// The frame last presented, to find the rows that changed; `fresh` until the first present.
	prev: Vec<u8>,
	fresh: bool,
	video: *mut u8,
	pitch: usize,
	ports: RawPorts,
}

fn int10(ax: u32, bx: u32) -> BiosRegs {
	let mut r = BiosRegs {
		eax: ax,
		ebx: bx,
		..BiosRegs::default()
	};
	let _ = DosBios.int(0x10, &mut r, None);
	r
}

fn ports() -> RawPorts {
	// SAFETY: a DPMI client under CWSDPMI may use the VGA and timer ports, as Desk's DOSBox runs showed; a host that traps them emulates
	// them. `RawPorts` holds no state.
	unsafe { RawPorts::new() }
}

/// Sets the mode `want` asks for: a VBE 640 by 480 mode with 256 colours first for `Best`, then mode 12h. `Err` says in a few words
/// why not, and the screen is then as it was.
pub fn enter(want: Graphics) -> Result<Screen, &'static str> {
	if want == Graphics::Text {
		return Err("graphics was not asked for");
	}
	// INT 10h AX=1A00h returns AL=1Ah on a VGA.
	if int10(0x1A00, 0).eax as u8 != 0x1A {
		return Err("there is no VGA");
	}
	SAVED_MODE.store(u32::from(int10(0x0F00, 0).eax as u8 & 0x7F), Ordering::Relaxed);
	ACTIVE.store(true, Ordering::Relaxed);
	cleanup::register(leave);
	if want == Graphics::Best {
		if let Some(s) = try_vbe() {
			return Ok(s);
		}
	}
	try_12h()
}

fn try_12h() -> Result<Screen, &'static str> {
	int10(0x0012, 0);
	if int10(0x0F00, 0).eax as u8 & 0x7F != 0x12 {
		leave();
		return Err("the card has no 640 by 480 16-colour mode");
	}
	let p = ports();
	regs::attribute_palette_identity(&p);
	let mut colours = [Rgb6::BLACK; 16];
	for (d, s) in colours.iter_mut().zip(WINDOWS16.iter()) {
		*d = Rgb6::from_rgb8(s.0, s.1, s.2);
	}
	let _ = dac::set_palette(&p, 0, &colours);
	// Write mode 0, no rotate, no set/reset, every bit: a byte written is the byte, and the map mask picks the plane.
	regs::gc_write(&p, regs::gc::MODE, 0x00);
	regs::gc_write(&p, regs::gc::DATA_ROTATE, 0x00);
	regs::gc_write(&p, regs::gc::ENABLE_SET_RESET, 0x00);
	regs::gc_write(&p, regs::gc::BIT_MASK, 0xFF);
	Ok(Screen {
		kind: Kind::Planar16,
		shadow: vec![0; PLANAR_BYTES],
		prev: vec![0; PLANAR_BYTES],
		fresh: true,
		video: dpmi::linear_to_ptr::<u8>(0xA0000),
		pitch: PLANAR_ROW,
		ports: p,
	})
}

fn try_vbe() -> Option<Screen> {
	let vbe = Vbe::new(&DosBios);
	let info = vbe.info().ok()?;
	let mut ids = [0u16; desk_vga::vbe::MAX_MODES];
	let count = vbe.mode_list(&info, &mut ids).unwrap_or(0);
	let mode = vbe.find(ids.get(..count)?, 640, 480, 8, true)?;
	let len = (mode.frame_bytes() + 0xFFF) & !0xFFF;
	let linear = dpmi::map_physical(mode.phys_base, len).ok()?;
	MAPPED.store(linear, Ordering::Relaxed);
	if vbe.set_mode(mode.id, true).is_err() {
		let _ = dpmi::unmap_physical(MAPPED.swap(0, Ordering::Relaxed));
		return None;
	}
	let p = ports();
	let mut colours = [Rgb6::BLACK; 256];
	for (i, d) in colours.iter_mut().enumerate() {
		let c = rgb256(i as u8);
		*d = Rgb6::from_rgb8(c.0, c.1, c.2);
	}
	let _ = dac::set_palette(&p, 0, &colours);
	Some(Screen {
		kind: Kind::Linear256,
		shadow: vec![0; LINEAR_ROW * ROWS],
		prev: vec![0; LINEAR_ROW * ROWS],
		fresh: true,
		video: dpmi::linear_to_ptr::<u8>(linear),
		pitch: usize::from(mode.pitch),
		ports: p,
	})
}

/// Gives the screen back: the mode it was in, with the frame buffer unmapped and the timer as it was. Safe to call twice.
pub fn leave() {
	if !ACTIVE.swap(false, Ordering::Relaxed) {
		return;
	}
	let linear = MAPPED.swap(0, Ordering::Relaxed);
	if linear != 0 {
		let _ = dpmi::unmap_physical(linear);
	}
	int10(SAVED_MODE.load(Ordering::Relaxed), 0);
	timer_stop();
	cleanup::forget(leave);
}

/// Calls `f(first row, rows)` for each run of consecutive marked rows.
fn runs(set: &RowSet, mut f: impl FnMut(usize, usize)) {
	let mut y = 0;
	while y < ROWS {
		if set.has(y) {
			let start = y;
			while y < ROWS && set.has(y) {
				y += 1;
			}
			f(start, y - start);
		} else {
			y += 1;
		}
	}
}

impl Screen {
	/// Shows the rows of the shadow that changed since the last call; returns how many.
	pub fn present(&mut self) -> usize {
		match self.kind {
			Kind::Planar16 => {
				let set = if self.fresh {
					RowSet::full()
				} else {
					damage::planar(&self.prev, &self.shadow)
				};
				let n = set.count();
				if n == 0 {
					return 0;
				}
				let (shadow, prev) = (self.shadow.as_ptr(), self.prev.as_mut_ptr());
				for plane in 0..4usize {
					regs::sequencer_write(&self.ports, regs::seq::MAP_MASK, 1 << plane);
					runs(&set, |y, count| {
						let at = y * PLANAR_ROW;
						// SAFETY: the shadow holds four planes of 38,400 bytes and the rows lie inside one; video memory at A0000h is
						// 38,400 bytes of the plane the map mask selects, in the first megabyte the host maps at linear 0.
						unsafe {
							core::ptr::copy_nonoverlapping(
								shadow.add(plane * PLANE_BYTES + at),
								self.video.add(at),
								count * PLANAR_ROW,
							);
							core::ptr::copy_nonoverlapping(
								shadow.add(plane * PLANE_BYTES + at),
								prev.add(plane * PLANE_BYTES + at),
								count * PLANAR_ROW,
							);
						}
					});
				}
				regs::sequencer_write(&self.ports, regs::seq::MAP_MASK, 0x0F);
				self.fresh = false;
				n
			}
			Kind::Linear256 => {
				let set = if self.fresh {
					RowSet::full()
				} else {
					damage::linear(&self.prev, &self.shadow, LINEAR_ROW)
				};
				let n = set.count();
				let pitch = self.pitch;
				let (shadow, prev) = (self.shadow.as_ptr(), self.prev.as_mut_ptr());
				runs(&set, |y, count| {
					// SAFETY: the shadow is 640 by 480 bytes; the mapping covers `pitch * 480` bytes of frame buffer.
					unsafe {
						core::ptr::copy_nonoverlapping(shadow.add(y * LINEAR_ROW), prev.add(y * LINEAR_ROW), count * LINEAR_ROW);
						if pitch == LINEAR_ROW {
							core::ptr::copy_nonoverlapping(
								shadow.add(y * LINEAR_ROW),
								self.video.add(y * pitch),
								count * LINEAR_ROW,
							);
						} else {
							for r in y..y + count {
								core::ptr::copy_nonoverlapping(shadow.add(r * LINEAR_ROW), self.video.add(r * pitch), LINEAR_ROW);
							}
						}
					}
				});
				self.fresh = false;
				n
			}
		}
	}

	/// The CRC-32 of the screen as the card holds it, read back from video memory in the shadow's own layout: what a dump compares with
	/// the host's.
	pub fn read_back_crc(&self) -> u32 {
		let mut crc = 0;
		let mut row = [0u8; LINEAR_ROW];
		match self.kind {
			Kind::Planar16 => {
				for plane in 0..4u8 {
					regs::gc_write(&self.ports, regs::gc::READ_MAP_SELECT, plane);
					for y in 0..ROWS {
						// SAFETY: 80 bytes of plane memory at A0000h plus the row.
						unsafe { core::ptr::copy_nonoverlapping(self.video.add(y * PLANAR_ROW), row.as_mut_ptr(), PLANAR_ROW) };
						crc = crc32_update(crc, row.get(..PLANAR_ROW).unwrap_or(&[]));
					}
				}
				regs::gc_write(&self.ports, regs::gc::READ_MAP_SELECT, 0);
			}
			Kind::Linear256 => {
				for y in 0..ROWS {
					// SAFETY: 640 bytes of the frame buffer's row `y`.
					unsafe { core::ptr::copy_nonoverlapping(self.video.add(y * self.pitch), row.as_mut_ptr(), LINEAR_ROW) };
					crc = crc32_update(crc, &row);
				}
			}
		}
		crc
	}
}

/// The BIOS tick count at 0040:006C, which counts at 18.2 Hz.
fn bios_ticks() -> u32 {
	// SAFETY: the BIOS data area is in the first megabyte, which the host maps at linear 0.
	unsafe { dpmi::linear_to_ptr::<u32>(0x46C).read_volatile() }
}

/// Puts timer channel 0 in rate-generator mode with a count of 65,536 (the BIOS tick's rate, 18.2 Hz), so the counter runs down once a
/// tick, one count to a clock of 1.193182 MHz. Only a run that measures does this; `leave` puts the BIOS's square wave back.
pub fn timer_start() {
	let p = ports();
	p.outb(0x43, 0x34);
	p.outb(0x40, 0);
	p.outb(0x40, 0);
	TIMER.store(true, Ordering::Relaxed);
}

fn timer_stop() {
	if TIMER.swap(false, Ordering::Relaxed) {
		let p = ports();
		p.outb(0x43, 0x36);
		p.outb(0x40, 0);
		p.outb(0x40, 0);
	}
}

/// Clocks (1.193182 MHz) since the BIOS tick count was zero, with the count and the counter read together.
pub fn clocks() -> u32 {
	let p = ports();
	loop {
		let t1 = bios_ticks();
		p.outb(0x43, 0x00);
		let lo = u32::from(p.inb(0x40));
		let hi = u32::from(p.inb(0x40));
		if bios_ticks() == t1 {
			let counter = (hi << 8) | lo;
			return t1.wrapping_mul(65_536).wrapping_add(65_536 - counter);
		}
	}
}

/// Microseconds in a span of clocks.
pub fn micros(clocks: u32) -> u32 {
	(u64::from(clocks) * 1_000_000 / 1_193_182) as u32
}
