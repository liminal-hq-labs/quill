// The DOS clock for the Date and Time fields: INT 21h AH=2Ah and 2Ch, and the country's formats from AX=3800h.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::measure::ScreenMeasure;
use core::arch::asm;
use quill_app::datefmt;
use quill_app::App;

/// Today's date (AH=2Ah): year, month and day.
fn date() -> (u32, u32, u32) {
	let (cx, dx): (u32, u32);
	// SAFETY: INT 21h AH=2Ah takes no pointer and only sets CX and DX (and AL).
	unsafe { asm!("int 0x21", inout("eax") 0x2A00u32 => _, out("ecx") cx, out("edx") dx, options(nostack)) };
	(cx & 0xFFFF, (dx >> 8) & 0xFF, dx & 0xFF)
}

/// The time now (AH=2Ch): hour and minute.
fn time() -> (u32, u32) {
	let cx: u32;
	// SAFETY: INT 21h AH=2Ch takes no pointer and only sets CX and DX.
	unsafe { asm!("int 0x21", inout("eax") 0x2C00u32 => _, out("ecx") cx, out("edx") _, options(nostack)) };
	((cx >> 8) & 0xFF, cx & 0xFF)
}

/// The country information (AX=3800h): the date order (0 month first, 1 day first, 2 year first), the date and time separators and
/// whether the clock has 24 hours. A DOS that does not answer gives the United States' settings.
fn country() -> (u16, u8, u8, bool) {
	// The DPMI host copies the answer back; a buffer of exactly 34 bytes has been overrun here, so it has room to spare.
	let mut buf = [0u8; 64];
	let (ax, cf): (u32, u32);
	// SAFETY: AX=3800h fills the buffer DS:EDX names (34 bytes at most) (a DPMI host copies it back); the carry flag says whether it worked.
	unsafe {
		asm!(
			"int 0x21",
			"sbb ecx, ecx",
			inout("eax") 0x3800u32 => ax,
			inout("edx") buf.as_mut_ptr() as u32 => _,
			out("ecx") cf,
			options(nostack)
		);
	}
	if cf != 0 || ax > 0xFFFF {
		return (0, b'/', b':', false);
	}
	(u16::from(buf[0]) | u16::from(buf[1]) << 8, buf[11], buf[13], buf[17] & 1 != 0)
}

/// Gives the editor the date and the time as the country writes them, for the fields a page prints and the graphics screen draws.
pub fn set(app: &mut App<ScreenMeasure>) {
	let (order, date_sep, time_sep, h24) = country();
	let mut pattern = [0u8; 10];
	let p = datefmt::dos_pattern(order, date_sep, &mut pattern);
	let (y, m, d) = date();
	let mut out = [0u8; 32];
	let n = datefmt::format(pattern.get(..p).unwrap_or(&[]), y, m, d, &mut out);
	if let Ok(text) = core::str::from_utf8(out.get(..n).unwrap_or(&[])) {
		app.set_print_date(text);
	}
	let (h, min) = time();
	// A DOS that leaves the time separator out (DOSBox's does) gets the colon.
	let sep = [if time_sep == 0 { b':' } else { time_sep }];
	let n = datefmt::format_time(h, min, h24, &sep, b"", b"", &mut out);
	if let Ok(text) = core::str::from_utf8(out.get(..n).unwrap_or(&[])) {
		app.set_print_time(text);
	}
}

/// Gives the file dialog the country's date order and separator and whether its clock has 24 hours, for the Modified column and the
/// preview.
pub fn set_file(app: &mut App<ScreenMeasure>) {
	let (order, date_sep, _, h24) = country();
	let mut pattern = [0u8; 10];
	let p = datefmt::dos_pattern(order, date_sep, &mut pattern);
	app.set_file_clock(pattern.get(..p).unwrap_or(&[]), h24);
}
