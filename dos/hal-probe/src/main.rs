// HALPROBE.EXE: draws a picture in VGA mode 12h and in VBE 640 x 480 x 8, reads it back and checks its checksum.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `HALPROBE.EXE [12] [vbe] [mem]` runs the named stages, or all of them. Exit status: 0 every check held, 1 a checksum or a copy
//! differed, 2 VBE was not available (the screen is back in text mode first), 3 a panic (the runtime's).
//!
//! - `12`  mode 12h by INT 10h, the identity attribute palette, a picture drawn through the planes, read back through the read map
//!   select and compared with the same picture computed in memory
//! - `vbe` the VBE information, a 640 x 480 x 8 mode with a linear frame buffer mapped with DPMI 0800h, the same comparison
//! - `mem` the string-move `memcpy` and `memset` against byte loops, at every length and alignment that matters
#![no_std]
#![no_main]

extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;
use core::hint::black_box;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use desk_hal::x86::RawPorts;
use desk_hal::{Bios, BiosRegs, HalError, MmioRegion};
use desk_vga::vbe::Vbe;
use desk_vga::{checksum, regs, Adler32, Canvas, Linear8, Planar16};
use rt::bios::DosBios;
use rt::{cleanup, dpmi, env, println};

rt::entry!(main);

const W: u32 = 640;
const H: u32 = 480;
/// Colour of the one-pixel frame, drawn over the blocks.
const FRAME: u8 = 15;

static FAILED: AtomicBool = AtomicBool::new(false);
/// The linear address of the VBE frame buffer mapping, or 0.
static MAPPED: AtomicU32 = AtomicU32::new(0);

fn check(what: &str, ok: bool) {
	println!("halprobe: {what}: {ok}");
	if !ok {
		FAILED.store(true, Ordering::Relaxed);
	}
}

fn int10(ax: u32) -> Result<(), HalError> {
	let mut regs = BiosRegs {
		eax: ax,
		..BiosRegs::default()
	};
	DosBios.int(0x10, &mut regs, None)
}

fn video_mode() -> u8 {
	let mut regs = BiosRegs {
		eax: 0x0F00,
		..BiosRegs::default()
	};
	let _ = DosBios.int(0x10, &mut regs, None);
	regs.eax as u8
}

/// Back to 80x25 text and the frame buffer mapping released. Safe to call twice.
fn leave() {
	if video_mode() != 3 {
		let _ = int10(0x0003);
	}
	let linear = MAPPED.swap(0, Ordering::Relaxed);
	if linear != 0 {
		let _ = dpmi::unmap_physical(linear);
	}
	cleanup::forget(leave);
}

/// The picture's colour at (x, y): 16 by 16 blocks that step through the palette, and a one-pixel frame.
fn colour(x: u32, y: u32, mask: u8) -> u8 {
	let on_frame = (8..=W - 9).contains(&x) && (8..=H - 9).contains(&y) && (x == 8 || x == W - 9 || y == 8 || y == H - 9);
	if on_frame {
		FRAME & mask
	} else {
		((x / 16) * 7 + (y / 16) * 3) as u8 & mask
	}
}

fn draw<C: Canvas>(c: &mut C, mask: u8) {
	c.clear(0);
	for by in 0..H / 16 {
		for bx in 0..W / 16 {
			c.fill_rect((bx * 16) as i32, (by * 16) as i32, 16, 16, ((bx * 7 + by * 3) as u8) & mask);
		}
	}
	c.rect(8, 8, W - 16, H - 16, FRAME & mask);
}

fn expected(mask: u8) -> u32 {
	let mut a = Adler32::new();
	let mut row = vec![0u8; W as usize];
	for y in 0..H {
		for x in 0..W {
			row[x as usize] = colour(x, y, mask);
		}
		a.update(&row);
	}
	a.finish()
}

fn report<C: Canvas>(what: &str, c: &mut C, mask: u8) {
	draw(c, mask);
	let want = expected(mask);
	match checksum(c) {
		Ok(got) => {
			println!("halprobe: {what}: checksum {got:#010x}, picture in memory {want:#010x}");
			check(&alloc::format!("{what}: read-back equals the picture"), got == want);
		}
		Err(e) => check(&alloc::format!("{what}: read-back ({e})"), false),
	}
	// A pixel changed after drawing must change the checksum, or the read-back proves nothing.
	c.pixel(100, 100, colour(100, 100, mask) ^ 1);
	check(
		&alloc::format!("{what}: a changed pixel changes the checksum"),
		checksum(c).map(|v| v != want).unwrap_or(false),
	);
}

fn stage_12() {
	println!("halprobe: mode 12h by INT 10h, from video mode {:#04x}", video_mode());
	// SAFETY: a DPMI client under CWSDPMI may use the VGA ports (Desk's evidence); a host that traps them emulates them.
	let ports = unsafe { RawPorts::new() };
	cleanup::register(leave);
	if int10(0x0012).is_err() || video_mode() != 0x12 {
		check("12h: the BIOS reports mode 12h", false);
		leave();
		return;
	}
	regs::attribute_palette_identity(&ports);
	// SAFETY: A0000h to AFFFFh is in the first megabyte, mapped at linear 0 and reached through the 4 GiB data segment; it is
	// video memory for as long as the mode is set and nothing else holds a reference to it.
	let mut region = unsafe { MmioRegion::new(dpmi::linear_to_ptr::<u8>(0xA0000), 0x10000) };
	match Planar16::new(&ports, &mut region, W, H, 80) {
		Ok(mut c) => report("12h", &mut c, 0x0F),
		Err(e) => check(&alloc::format!("12h: surface ({e})"), false),
	}
	leave();
	check("12h: text mode is back", video_mode() == 3);
}

/// Returns false (with the screen already in text mode) when VBE is missing or refuses.
fn stage_vbe() -> bool {
	let vbe = Vbe::new(&DosBios);
	let info = match vbe.info() {
		Ok(i) => i,
		Err(e) => {
			println!("halprobe: vbe: no VBE information ({e}); not available");
			return false;
		}
	};
	println!(
		"halprobe: vbe: version {:x}.{:x}, {} KiB video memory",
		info.version >> 8,
		info.version & 0xFF,
		u32::from(info.memory_64k) * 64
	);
	let mut ids = [0u16; desk_vga::vbe::MAX_MODES];
	let count = vbe.mode_list(&info, &mut ids).unwrap_or(0);
	println!("halprobe: vbe: {count} modes listed");
	let Some(mode) = vbe.find(&ids[..count], W as u16, H as u16, 8, true) else {
		println!("halprobe: vbe: no 640x480x8 mode with a linear frame buffer");
		return false;
	};
	println!(
		"halprobe: vbe: using {:#05x}: pitch {}, frame buffer at physical {:#010x}, {} bytes",
		mode.id,
		mode.pitch,
		mode.phys_base,
		mode.frame_bytes()
	);
	let len = (mode.frame_bytes() + 0xFFF) & !0xFFF;
	let linear = match dpmi::map_physical(mode.phys_base, len) {
		Ok(l) => l,
		Err(e) => {
			println!("halprobe: vbe: mapping the frame buffer failed ({})", e.0);
			return false;
		}
	};
	// Registered before the mode is set, so that a failure or a panic after it leaves text mode.
	MAPPED.store(linear, Ordering::Relaxed);
	cleanup::register(leave);
	if let Err(e) = vbe.set_mode(mode.id, true) {
		println!("halprobe: vbe: setting the mode failed ({e})");
		leave();
		return false;
	}
	check(
		"vbe: the BIOS reports the mode with the linear bit",
		vbe.current_mode() == Ok(mode.id | 0x4000),
	);
	// SAFETY: `linear` is the host's mapping of the frame buffer, `len` bytes, valid until `leave` unmaps it, and reached through the
	// 4 GiB data segment.
	let mut region = unsafe { MmioRegion::new(dpmi::linear_to_ptr::<u8>(linear), len as usize) };
	match Linear8::new(&mut region, W, H, u32::from(mode.pitch)) {
		Ok(mut c) => report("vbe 640x480x8", &mut c, 0xFF),
		Err(e) => check(&alloc::format!("vbe: surface ({e})"), false),
	}
	leave();
	check("vbe: text mode is back", video_mode() == 3);
	true
}

/// The string-move `memcpy` and `memset` against plain loops: every length from 0 to 40 at the four source and destination
/// alignments, with guard bytes either side that must not change.
fn stage_mem() {
	let mut ok = true;
	let src: Vec<u8> = (0..64u32).map(|i| (i * 37 + 11) as u8).collect();
	for len in 0..=40usize {
		for so in 0..4usize {
			for d_o in 0..4usize {
				let mut dst = vec![0xA5u8; 64];
				let mut want = vec![0xA5u8; 64];
				for i in 0..len {
					want[4 + d_o + i] = src[4 + so + i];
				}
				let s = black_box(&src[4 + so..]).as_ptr();
				let d = black_box(&mut dst[4 + d_o..]).as_mut_ptr();
				// SAFETY: both ranges hold `len` bytes inside their vectors and do not overlap.
				unsafe { core::ptr::copy_nonoverlapping(s, d, black_box(len)) };
				ok &= dst == want;
				let mut dst = vec![0xA5u8; 64];
				let mut want = vec![0xA5u8; 64];
				for b in want.iter_mut().skip(4 + d_o).take(len) {
					*b = 0x5C;
				}
				let d = black_box(&mut dst[4 + d_o..]).as_mut_ptr();
				// SAFETY: `len` bytes inside the vector.
				unsafe { core::ptr::write_bytes(d, black_box(0x5C), black_box(len)) };
				ok &= dst == want;
			}
		}
	}
	check(
		"mem: memcpy and memset match byte loops at lengths 0 to 40 and four alignments",
		ok,
	);
	let big = vec![7u8; 150_000];
	let mut copy = vec![0u8; big.len()];
	copy.copy_from_slice(black_box(&big));
	check("mem: a 150,000-byte copy", copy == big);
}

fn main() -> i32 {
	let args = env::args();
	let wanted = |name: &str| args.is_empty() || args.iter().any(|a| a.as_str().eq_ignore_ascii_case(name));
	let mut vbe_missing = false;
	if wanted("mem") {
		stage_mem();
	}
	if wanted("12") {
		stage_12();
	}
	if wanted("vbe") {
		vbe_missing = !stage_vbe();
	}
	if FAILED.load(Ordering::Relaxed) {
		1
	} else if vbe_missing {
		2
	} else {
		0
	}
}
