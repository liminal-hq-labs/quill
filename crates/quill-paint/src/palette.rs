// The palettes: the sixteen Windows colours in VGA order, and the 256-colour cube.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Mode 12h index `i` is the VGA-native order with the Windows values, so a screen whose DAC was not loaded is still right to
//! within the brightness of the colours. The 256-colour palette is the 6 × 6 × 6 cube in 0 to 215 (`36 r + 6 g + b`), then the eight
//! Windows colours the cube lacks in 216 to 223; the rest is free.

use crate::Rgb;

/// The sixteen Windows colours by mode 12h index: black, navy, green, teal, maroon, purple, olive, silver, grey, blue, lime, aqua, red,
/// fuchsia, yellow, white.
pub const WINDOWS16: [Rgb; 16] = [
	Rgb(0, 0, 0),
	Rgb(0, 0, 128),
	Rgb(0, 128, 0),
	Rgb(0, 128, 128),
	Rgb(128, 0, 0),
	Rgb(128, 0, 128),
	Rgb(128, 128, 0),
	Rgb(192, 192, 192),
	Rgb(128, 128, 128),
	Rgb(0, 0, 255),
	Rgb(0, 255, 0),
	Rgb(0, 255, 255),
	Rgb(255, 0, 0),
	Rgb(255, 0, 255),
	Rgb(255, 255, 0),
	Rgb(255, 255, 255),
];

/// The Windows colours that are not corners of the cube, as entries 216 to 223 of the 256-colour palette.
pub const EXTRAS: [Rgb; 8] = [
	Rgb(0, 0, 128),
	Rgb(0, 128, 0),
	Rgb(0, 128, 128),
	Rgb(128, 0, 0),
	Rgb(128, 0, 128),
	Rgb(128, 128, 0),
	Rgb(192, 192, 192),
	Rgb(128, 128, 128),
];

/// The first extra entry of the 256-colour palette.
pub const EXTRA_BASE: u8 = 216;

/// The levels of one channel of the cube.
pub const CUBE_LEVELS: [u8; 6] = [0, 51, 102, 153, 204, 255];

fn dist(a: Rgb, b: Rgb) -> i32 {
	let d = |x: u8, y: u8| {
		let v = i32::from(x) - i32::from(y);
		v * v
	};
	d(a.0, b.0) + d(a.1, b.1) + d(a.2, b.2)
}

/// The Windows colour nearest `c`, as a mode 12h index.
pub fn nearest16(c: Rgb) -> u8 {
	let mut best = (i32::MAX, 0u8);
	for (i, w) in WINDOWS16.iter().enumerate() {
		let d = dist(c, *w);
		if d < best.0 {
			best = (d, i as u8);
		}
	}
	best.1
}

/// The colour of entry `i` of the 256-colour palette; the free entries are black.
pub fn rgb256(i: u8) -> Rgb {
	if i < EXTRA_BASE {
		let (r, g, b) = (i / 36, (i / 6) % 6, i % 6);
		let l = |n: u8| CUBE_LEVELS.get(usize::from(n)).copied().unwrap_or(0);
		Rgb(l(r), l(g), l(b))
	} else {
		EXTRAS
			.get(usize::from(i - EXTRA_BASE))
			.copied()
			.unwrap_or(Rgb::BLACK)
	}
}

fn cube_level(v: u8) -> u8 {
	((u16::from(v) * 5 + 127) / 255) as u8
}

/// The entry of the 256-colour palette for `c`: an exact Windows colour when it is one, else the nearest cube colour.
pub fn index256(c: Rgb) -> u8 {
	if let Some(i) = EXTRAS.iter().position(|e| *e == c) {
		return EXTRA_BASE + i as u8;
	}
	36 * cube_level(c.0) + 6 * cube_level(c.1) + cube_level(c.2)
}

/// The 4 × 4 ordered-dither threshold at a pixel, 0 to 15.
pub fn bayer(x: i32, y: i32) -> i32 {
	const M: [[i32; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];
	M.get((y & 3) as usize)
		.and_then(|r| r.get((x & 3) as usize))
		.copied()
		.unwrap_or(0)
}

/// A picture pixel reduced to the sixteen colours at (`x`, `y`).
pub fn dither16(rgb: [u8; 3], x: i32, y: i32) -> u8 {
	let bias = (bayer(x, y) - 8) * 4;
	let ch = |v: u8| (i32::from(v) + bias).clamp(0, 255) as u8;
	nearest16(Rgb(ch(rgb[0]), ch(rgb[1]), ch(rgb[2])))
}

/// A picture pixel reduced to the cube at (`x`, `y`).
pub fn dither256(rgb: [u8; 3], x: i32, y: i32) -> u8 {
	let t = bayer(x, y);
	let lv = |v: u8| ((i32::from(v) * 80 + t * 255) / 4080).clamp(0, 5) as u8;
	36 * lv(rgb[0]) + 6 * lv(rgb[1]) + lv(rgb[2])
}
