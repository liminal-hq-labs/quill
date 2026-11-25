// BMP tests: round trips at every depth, refusals, damage, and a hashed dither.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use proptest::prelude::*;
use quill_formats::io::SliceSource;
use quill_image::bmp::{decode_rows, read_info};
use quill_image::dither::{dither_row, shrink_row};
use quill_image::ImageError;

/// Writes a bitmap: \`px\` is rows of (r, g, b) from the top; below 24 bits the colours must fit a palette of 2^bpp entries.
fn encode(px: &[Vec<[u8; 3]>], bpp: u16, top_down: bool, compression: u32) -> Vec<u8> {
	let (w, h) = (px[0].len(), px.len());
	let mut pal: Vec<[u8; 3]> = Vec::new();
	let mut idx = |c: [u8; 3]| -> usize {
		pal.iter().position(|p| *p == c).unwrap_or_else(|| {
			pal.push(c);
			pal.len() - 1
		})
	};
	let row_bytes = (usize::from(bpp) * w).div_ceil(32) * 4;
	let mut rows: Vec<Vec<u8>> = Vec::new();
	for r in px {
		let mut b = vec![0u8; row_bytes];
		for (x, c) in r.iter().enumerate() {
			match bpp {
				24 => b[x * 3..x * 3 + 3].copy_from_slice(&[c[2], c[1], c[0]]),
				8 => b[x] = idx(*c) as u8,
				4 => b[x / 2] |= (idx(*c) as u8) << if x % 2 == 0 { 4 } else { 0 },
				_ => b[x / 8] |= (idx(*c) as u8) << (7 - x % 8),
			}
		}
		rows.push(b);
	}
	let ncol = if bpp <= 8 { 1usize << bpp } else { 0 };
	let data_at = 14 + 40 + ncol * 4;
	let mut out = b"BM".to_vec();
	out.extend_from_slice(&((data_at + row_bytes * h) as u32).to_le_bytes());
	out.extend_from_slice(&[0; 4]);
	out.extend_from_slice(&(data_at as u32).to_le_bytes());
	out.extend_from_slice(&40u32.to_le_bytes());
	out.extend_from_slice(&(w as i32).to_le_bytes());
	out.extend_from_slice(&(if top_down { -(h as i32) } else { h as i32 }).to_le_bytes());
	out.extend_from_slice(&1u16.to_le_bytes());
	out.extend_from_slice(&bpp.to_le_bytes());
	out.extend_from_slice(&compression.to_le_bytes());
	out.extend_from_slice(&[0; 20]);
	for i in 0..ncol {
		let c = pal.get(i).copied().unwrap_or([0; 3]);
		out.extend_from_slice(&[c[2], c[1], c[0], 0]);
	}
	let order: Vec<&Vec<u8>> = if top_down {
		rows.iter().collect()
	} else {
		rows.iter().rev().collect()
	};
	for r in order {
		out.extend_from_slice(r);
	}
	out
}

fn decode(bytes: &[u8]) -> Result<(u32, u32, Vec<Vec<u8>>), ImageError> {
	let mut s = SliceSource::new(bytes);
	let info = read_info(&mut s)?;
	let mut rows = vec![Vec::new(); info.height as usize];
	decode_rows(&mut s, &info, |y, r| rows[y as usize] = r.to_vec())?;
	Ok((info.width, info.height, rows))
}

fn image(w: usize, h: usize, colours: &[[u8; 3]], seed: usize) -> Vec<Vec<[u8; 3]>> {
	(0..h)
		.map(|y| {
			(0..w)
				.map(|x| colours[(x * 7 + y * 3 + seed) % colours.len()])
				.collect()
		})
		.collect()
}

#[test]
fn every_depth_and_direction_round_trips() {
	let two = [[0, 0, 0], [255, 255, 255]];
	let sixteen: Vec<[u8; 3]> = (0..16).map(|i| [i * 16, 255 - i * 16, i * 8]).collect();
	let many: Vec<[u8; 3]> = (0..200)
		.map(|i| [i as u8, (i * 3) as u8, (i * 7) as u8])
		.collect();
	for (bpp, colours) in [
		(1u16, &two[..]),
		(4, &sixteen[..]),
		(8, &many[..]),
		(24, &many[..]),
	] {
		for top_down in [false, true] {
			for (w, h) in [(1, 1), (7, 5), (33, 4), (64, 3)] {
				let img = image(w, h, colours, 3);
				let (gw, gh, rows) = decode(&encode(&img, bpp, top_down, 0)).unwrap();
				assert_eq!((gw as usize, gh as usize), (w, h));
				for (y, row) in img.iter().enumerate() {
					let flat: Vec<u8> = row.iter().flatten().copied().collect();
					assert_eq!(
						rows[y], flat,
						"{bpp} bpp, top_down {top_down}, {w} x {h}, row {y}"
					);
				}
			}
		}
	}
}

/// An 8-bit bitmap as other writers make it: a palette of `ncol` entries (`biClrUsed` set to `used`), then the rows `idx`, bottom-up,
/// each padded to four bytes.
fn eight_bit(w: usize, idx: &[Vec<u8>], ncol: usize, used: u32) -> Vec<u8> {
	let row_bytes = w.div_ceil(4) * 4;
	let data_at = 14 + 40 + ncol * 4;
	let mut out = b"BM".to_vec();
	out.extend_from_slice(&((data_at + row_bytes * idx.len()) as u32).to_le_bytes());
	out.extend_from_slice(&[0; 4]);
	out.extend_from_slice(&(data_at as u32).to_le_bytes());
	out.extend_from_slice(&40u32.to_le_bytes());
	out.extend_from_slice(&(w as i32).to_le_bytes());
	out.extend_from_slice(&(idx.len() as i32).to_le_bytes());
	out.extend_from_slice(&1u16.to_le_bytes());
	out.extend_from_slice(&8u16.to_le_bytes());
	out.extend_from_slice(&[0; 16]);
	out.extend_from_slice(&used.to_le_bytes());
	out.extend_from_slice(&[0; 4]);
	for i in 0..ncol {
		out.extend_from_slice(&[i as u8, 0, 255 - i as u8, 0]);
	}
	for r in idx.iter().rev() {
		let mut b = r.clone();
		b.resize(row_bytes, 0);
		out.extend_from_slice(&b);
	}
	out
}

#[test]
fn eight_bit_pictures_with_a_short_or_overrun_palette_still_decode() {
	// `biClrUsed` 16, indices into it and one past it: the stray index is black, and the picture is not refused.
	let idx = vec![vec![0u8, 1, 15, 16, 200], vec![15, 15, 0, 0, 255]];
	let (w, h, rows) = decode(&eight_bit(5, &idx, 16, 16)).unwrap();
	assert_eq!((w, h), (5, 2));
	let px = |i: u8| -> [u8; 3] {
		if i < 16 {
			[255 - i, 0, i]
		} else {
			[0; 3]
		}
	};
	for (y, row) in idx.iter().enumerate() {
		let want: Vec<u8> = row.iter().flat_map(|i| px(*i)).collect();
		assert_eq!(rows[y], want, "row {y}");
	}
	// The same table with `biClrUsed` 0: the file header's pixel offset says the table has 16 entries, not 256.
	let (_, _, rows) = decode(&eight_bit(5, &idx, 16, 0)).unwrap();
	let want: Vec<u8> = idx[0].iter().flat_map(|i| px(*i)).collect();
	assert_eq!(rows[0], want);
	// A full table of 256 entries, as an 8-bit picture normally has, is read whole.
	let full = vec![vec![0u8, 128, 255]];
	let (_, _, rows) = decode(&eight_bit(3, &full, 256, 0)).unwrap();
	assert_eq!(rows[0], vec![255, 0, 0, 127, 0, 128, 0, 0, 255]);
}

#[test]
fn rle_and_other_depths_are_refused_and_big_pictures_too() {
	let img = image(4, 4, &[[1, 2, 3], [4, 5, 6]], 0);
	assert_eq!(
		decode(&encode(&img, 8, false, 1)).err(),
		Some(ImageError::Unsupported)
	);
	let mut b = encode(&img, 24, false, 0);
	b[28] = 32;
	assert_eq!(decode(&b).err(), Some(ImageError::Unsupported));
	let mut b = encode(&img, 24, false, 0);
	b[18..22].copy_from_slice(&5000i32.to_le_bytes());
	assert_eq!(decode(&b).err(), Some(ImageError::TooLarge));
	assert_eq!(decode(b"not a bitmap").err(), Some(ImageError::BadFile));
}

#[test]
fn the_dithered_gradient_has_the_recorded_hash() {
	// A 64 by 16 ramp from black to white with a red and a blue wash, dithered to 16 colours, rows hashed with FNV-1a.
	let mut h = 0xcbf29ce484222325u64;
	for y in 0..16u32 {
		let rgb: Vec<u8> = (0..64u32)
			.flat_map(|x| {
				[
					(x * 4) as u8,
					(x * 4 + y * 8) as u8 / 2,
					(255 - x * 4) as u8,
				]
			})
			.collect();
		let mut out = vec![0u8; 64];
		dither_row(&rgb, y, &mut out);
		for b in out {
			h = (h ^ u64::from(b)).wrapping_mul(0x100000001b3);
		}
	}
	assert_eq!(
		format!("{h:016x}"),
		std::fs::read_to_string(concat!(
			env!("CARGO_MANIFEST_DIR"),
			"/../../tests/golden/images/gradient.dither16.hash"
		))
		.unwrap()
		.trim()
	);
}

#[test]
fn shrinking_averages() {
	let rgb = [0u8, 0, 0, 100, 100, 100, 200, 200, 200, 255, 255, 255];
	let mut out = [0u8; 6];
	shrink_row(&rgb, 2, &mut out);
	assert_eq!(out, [50, 50, 50, 227, 227, 227]);
}

proptest! {
	#[test]
	fn damaged_bitmaps_never_panic(flips in proptest::collection::vec((any::<u16>(), any::<u8>()), 1..8), cut in any::<u16>()) {
		let img = image(20, 6, &[[9, 8, 7], [200, 100, 50], [0, 0, 0]], 1);
		let mut b = encode(&img, 8, false, 0);
		for (i, v) in flips {
			let n = b.len();
			b[usize::from(i) % n] = v;
		}
		b.truncate(usize::from(cut) % (b.len() + 1));
		let _ = decode(&b);
	}
}
