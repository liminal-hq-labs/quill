// Reducing rows to the 16 VGA colours with an ordered dither, and shrinking rows with a box filter.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// The 16 VGA colours: black, blue, green, cyan, red, magenta, brown, light grey, dark grey, light blue, light green, light cyan,
/// light red, light magenta, yellow, white.
pub const VGA: [[u8; 3]; 16] = [
	[0, 0, 0],
	[0, 0, 170],
	[0, 170, 0],
	[0, 170, 170],
	[170, 0, 0],
	[170, 0, 170],
	[170, 85, 0],
	[170, 170, 170],
	[85, 85, 85],
	[85, 85, 255],
	[85, 255, 85],
	[85, 255, 255],
	[255, 85, 85],
	[255, 85, 255],
	[255, 255, 85],
	[255, 255, 255],
];

/// A 4 by 4 Bayer matrix, scaled to a signed offset of at most 32 on a 0 to 255 channel.
const BAYER: [[i16; 4]; 4] = [
	[-30, 2, -22, 10],
	[18, -14, 26, -6],
	[-18, 14, -26, 6],
	[30, -2, 22, -10],
];

fn nearest(r: i16, g: i16, b: i16) -> u8 {
	let mut best = (0u8, i32::MAX);
	for (i, c) in VGA.iter().enumerate() {
		let (dr, dg, db) = (
			i32::from(r) - i32::from(c[0]),
			i32::from(g) - i32::from(c[1]),
			i32::from(b) - i32::from(c[2]),
		);
		let d = dr * dr + dg * dg + db * db;
		if d < best.1 {
			best = (i as u8, d);
		}
	}
	best.0
}

/// Dithers one row of RGB pixels at picture row `y` into VGA colour indices (one byte per pixel).
pub fn dither_row(rgb: &[u8], y: u32, out: &mut [u8]) {
	for (x, (px, o)) in rgb
		.as_chunks::<3>()
		.0
		.iter()
		.zip(out.iter_mut())
		.enumerate()
	{
		let t = BAYER
			.get((y % 4) as usize)
			.and_then(|r| r.get(x % 4))
			.copied()
			.unwrap_or(0);
		let add = |c: u8| (i16::from(c) + t).clamp(0, 255);
		*o = nearest(
			add(px.first().copied().unwrap_or(0)),
			add(px.get(1).copied().unwrap_or(0)),
			add(px.get(2).copied().unwrap_or(0)),
		);
	}
}

/// Averages a source row into a row `out_w` pixels wide (a box filter along the row); the caller averages rows down.
pub fn shrink_row(rgb: &[u8], out_w: usize, out: &mut [u8]) {
	let src_w = rgb.len() / 3;
	if src_w == 0 || out_w == 0 {
		return;
	}
	for x in 0..out_w.min(out.len() / 3) {
		let (a, b) = (
			x * src_w / out_w,
			((x + 1) * src_w / out_w)
				.max(x * src_w / out_w + 1)
				.min(src_w),
		);
		let mut sum = [0u32; 3];
		for px in rgb.as_chunks::<3>().0.iter().skip(a).take(b - a) {
			for (s, c) in sum.iter_mut().zip(px) {
				*s += u32::from(*c);
			}
		}
		let n = (b - a) as u32;
		for (k, s) in sum.iter().enumerate() {
			if let Some(o) = out.get_mut(x * 3 + k) {
				*o = (*s / n) as u8;
			}
		}
	}
}
