// The mouse cursor: an arrow composited over the screen at present time, never written into the shadow.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The shadow is the picture the shell drew; the cursor is not part of it. A presenter that shows the cursor writes the bytes that
//! cover the arrow to video memory, each one the shadow's own byte with the arrow's pixels set over it; one that hides it writes
//! the shadow's bytes back over the same place. Video memory is never read. Both are a handful of bytes, so a pointer that only moves
//! costs a few writes and no repaint.
//!
//! The functions take what to write as a callback, so the same code serves the card (a pointer into video memory) and the host test
//! (a second buffer to compare with the shadow).

use crate::damage::{LINEAR_ROW, PLANAR_ROW, ROWS};
use crate::palette::index256;
use crate::raster::PLANE_BYTES;
use crate::Rgb;

/// The arrow's width in pixels.
pub const ARROW_W: i32 = 12;
/// The arrow's height in pixels.
pub const ARROW_H: i32 = 16;

/// The arrow: `X` is black outline, `.` is white fill, a space leaves the screen as it is. The tip, at the top left, is the hot spot.
const ARROW: [&[u8; 12]; 16] = [
	b"X           ",
	b"XX          ",
	b"X.X         ",
	b"X..X        ",
	b"X...X       ",
	b"X....X      ",
	b"X.....X     ",
	b"X......X    ",
	b"X.......X   ",
	b"X........X  ",
	b"X.....XXXXX ",
	b"X..X..X     ",
	b"X.X X..X    ",
	b"XX  X..X    ",
	b"X    X..X   ",
	b"     XXXX   ",
];

/// The arrow's rows as masks over its 12 pixels, bit 15 the leftmost: the pixels each covers, and those of them that are black.
const MASKS: [(u16, u16); 16] = masks();

#[allow(clippy::indexing_slicing)] // evaluated at compile time over fixed bounds, so a mistake does not build
const fn masks() -> [(u16, u16); 16] {
	let mut out = [(0u16, 0u16); 16];
	let mut r = 0;
	while r < 16 {
		let row = ARROW[r];
		let (mut cover, mut black) = (0u16, 0u16);
		let mut i = 0;
		while i < 12 {
			let bit = 0x8000u16 >> i;
			if row[i] != b' ' {
				cover |= bit;
			}
			if row[i] == b'X' {
				black |= bit;
			}
			i += 1;
		}
		out[r] = (cover, black);
		r += 1;
	}
	out
}

fn row_masks(r: usize) -> (u16, u16) {
	MASKS.get(r).copied().unwrap_or((0, 0))
}

/// The columns and rows of the screen the arrow with its tip at (`x`, `y`) touches, inside the 640 by 480 screen: `(left, top, right,
/// bottom)`, right and bottom one past the last. Empty when the arrow is wholly off the screen.
pub fn bounds(x: i32, y: i32) -> (i32, i32, i32, i32) {
	(
		x.max(0),
		y.max(0),
		(x + ARROW_W).min(LINEAR_ROW as i32),
		(y + ARROW_H).min(ROWS as i32),
	)
}

/// The arrow's pixels in screen row `row` for an arrow with its tip at (`x`, `y`), as masks over the 16 pixels starting at pixel `from`
/// (bit 15 the pixel `from`): covered, and black.
fn row_at(x: i32, y: i32, row: i32, from: i32) -> (u16, u16) {
	let r = row - y;
	if !(0..ARROW_H).contains(&r) {
		return (0, 0);
	}
	let (c, b) = row_masks(r as usize);
	// The arrow's column 0 is at screen column `x`; the window's first pixel is `from`, so the arrow moves right by `x - from`.
	let sh = x - from;
	let shift = |m: u16| -> u16 {
		if sh >= 16 || sh <= -16 {
			0
		} else if sh >= 0 {
			m >> sh
		} else {
			m << -sh
		}
	};
	(shift(c), shift(b))
}

/// Shows or hides the arrow on a mode 12h screen: calls `put(plane, offset, byte)` for each byte of each plane the arrow touches,
/// `offset` counted from the start of the plane. `shadow` is the four planes one after the other. With `show` false the bytes are the
/// shadow's own, which puts back what the arrow covered.
pub fn planar(shadow: &[u8], x: i32, y: i32, show: bool, mut put: impl FnMut(usize, usize, u8)) {
	let (l, t, r, b) = bounds(x, y);
	if l >= r || t >= b {
		return;
	}
	let (first, last) = ((l >> 3) as usize, ((r - 1) >> 3) as usize);
	for row in t..b {
		for bx in first..=last {
			let from = (bx * 8) as i32;
			// The arrow's pixels in these eight, as a byte (the window is 16 wide; the byte is its top half).
			let (cover, black) = row_at(x, y, row, from);
			let (cover, black) = ((cover >> 8) as u8, (black >> 8) as u8);
			let at = row as usize * PLANAR_ROW + bx;
			for plane in 0..4 {
				let old = shadow.get(plane * PLANE_BYTES + at).copied().unwrap_or(0);
				// White is every plane set, black none.
				let white = cover & !black;
				let byte = if show { (old & !cover) | white } else { old };
				put(plane, at, byte);
			}
		}
	}
}

/// Shows or hides the arrow on a one byte a pixel screen (a VBE 256-colour mode): calls `put(offset, byte)` for each pixel it touches,
/// `offset` counted from the start of the shadow's rows (`row * 640 + column`).
pub fn linear(shadow: &[u8], x: i32, y: i32, show: bool, mut put: impl FnMut(usize, u8)) {
	let (l, t, r, b) = bounds(x, y);
	let (black, white) = (index256(Rgb::BLACK), index256(Rgb::WHITE));
	for row in t..b {
		let (cover, blk) = row_at(x, y, row, x);
		for col in l..r {
			let at = row as usize * LINEAR_ROW + col as usize;
			let old = shadow.get(at).copied().unwrap_or(0);
			let bit = 0x8000u16 >> (col - x);
			let byte = match (show && cover & bit != 0, blk & bit != 0) {
				(true, true) => black,
				(true, false) => white,
				_ => old,
			};
			put(at, byte);
		}
	}
}

#[cfg(test)]
#[allow(clippy::indexing_slicing)]
mod tests {
	use super::*;
	use crate::raster::PLANAR_BYTES;
	extern crate std;
	use std::{vec, vec::Vec};

	fn pattern(n: usize) -> Vec<u8> {
		(0..n)
			.map(|i| (i.wrapping_mul(2654435761) >> 7) as u8)
			.collect()
	}

	/// The pixel of a planar buffer.
	fn pixel(planes: &[u8], x: usize, y: usize) -> u8 {
		(0..4).fold(0, |c, p| {
			c | (((planes[p * PLANE_BYTES + y * PLANAR_ROW + x / 8] >> (7 - x % 8)) & 1) << p)
		})
	}

	#[test]
	fn the_arrow_masks_are_what_is_drawn() {
		assert_eq!(row_masks(0), (0x8000, 0x8000));
		assert_eq!(row_masks(2), (0xE000, 0xA000));
		assert!(ARROW.iter().all(|r| r.len() == 12));
	}

	#[test]
	fn showing_the_arrow_on_planes_changes_only_its_pixels() {
		let shadow = pattern(PLANAR_BYTES);
		for (x, y) in [
			(0, 0),
			(5, 7),
			(631, 100),
			(639, 479),
			(13, 470),
			(300, 300),
		] {
			let mut video = shadow.clone();
			planar(&shadow, x, y, true, |p, o, v| {
				video[p * PLANE_BYTES + o] = v
			});
			for py in 0..ROWS {
				for px in 0..LINEAR_ROW {
					let (dx, dy) = (px as i32 - x, py as i32 - y);
					let arrow = (0..ARROW_W).contains(&dx)
						&& (0..ARROW_H).contains(&dy)
						&& ARROW[dy as usize][dx as usize] != b' ';
					let want = if arrow {
						if ARROW[dy as usize][dx as usize] == b'X' {
							0
						} else {
							15
						}
					} else {
						pixel(&shadow, px, py)
					};
					assert_eq!(
						pixel(&video, px, py),
						want,
						"tip ({x},{y}) pixel ({px},{py})"
					);
				}
			}
		}
	}

	#[test]
	fn hiding_the_arrow_gives_back_the_shadow_and_never_touches_it() {
		let shadow = pattern(PLANAR_BYTES);
		let copy = shadow.clone();
		let mut video = shadow.clone();
		planar(&shadow, 40, 40, true, |p, o, v| {
			video[p * PLANE_BYTES + o] = v
		});
		assert_ne!(video, shadow);
		planar(&shadow, 40, 40, false, |p, o, v| {
			video[p * PLANE_BYTES + o] = v
		});
		assert_eq!(video, shadow);
		assert_eq!(shadow, copy);
	}

	#[test]
	fn the_linear_arrow_sets_its_pixels_and_hiding_restores_them() {
		let shadow = pattern(LINEAR_ROW * ROWS);
		for (x, y) in [(0, 0), (630, 470), (639, 479), (100, 200)] {
			let mut video = shadow.clone();
			linear(&shadow, x, y, true, |o, v| video[o] = v);
			let changed = video.iter().zip(&shadow).filter(|(a, b)| a != b).count();
			assert!(changed > 0 && changed <= (ARROW_W * ARROW_H) as usize);
			assert_eq!(
				video[(y as usize) * LINEAR_ROW + x as usize],
				index256(Rgb::BLACK),
				"the tip is black"
			);
			linear(&shadow, x, y, false, |o, v| video[o] = v);
			assert_eq!(video, shadow);
		}
	}

	#[test]
	fn the_two_formats_draw_the_same_arrow() {
		let planes = vec![0u8; PLANAR_BYTES];
		let mut p = planes.clone();
		planar(&planes, 77, 123, true, |pl, o, v| {
			p[pl * PLANE_BYTES + o] = v
		});
		let lin = vec![0u8; LINEAR_ROW * ROWS];
		let mut l = lin.clone();
		linear(&lin, 77, 123, true, |o, v| l[o] = v);
		for y in 0..ROWS {
			for x in 0..LINEAR_ROW {
				let white16 = pixel(&p, x, y) == 15;
				let white256 = l[y * LINEAR_ROW + x] == index256(Rgb::WHITE);
				assert_eq!(white16, white256, "({x},{y})");
			}
		}
	}
}
