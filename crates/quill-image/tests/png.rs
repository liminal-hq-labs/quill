// PNG tests: every colour type and depth, every filter, split IDAT chunks, refusals and damage.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use proptest::prelude::*;
use quill_formats::io::SliceSource;
use quill_image::png::decode_rows;
use quill_image::ImageError;
use quill_inflate::WINDOW;

fn crc(data: &[u8]) -> u32 {
	let mut c = 0xFFFF_FFFFu32;
	for b in data {
		c ^= u32::from(*b);
		for _ in 0..8 {
			c = if c & 1 != 0 {
				0xEDB8_8320 ^ (c >> 1)
			} else {
				c >> 1
			};
		}
	}
	!c
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
	out.extend_from_slice(&(data.len() as u32).to_be_bytes());
	let mut body = kind.to_vec();
	body.extend_from_slice(data);
	out.extend_from_slice(&body);
	out.extend_from_slice(&crc(&body).to_be_bytes());
}

fn paeth(a: u8, b: u8, c: u8) -> u8 {
	let (ia, ib, ic) = (i16::from(a), i16::from(b), i16::from(c));
	let p = ia + ib - ic;
	let (pa, pb, pc) = ((p - ia).abs(), (p - ib).abs(), (p - ic).abs());
	if pa <= pb && pa <= pc {
		a
	} else if pb <= pc {
		b
	} else {
		c
	}
}

/// Encodes raw rows (already packed for the depth), filtering row y with filter y % 5, in stored deflate blocks.
#[allow(clippy::too_many_arguments)] // a test helper that spells out every header field
fn encode(
	w: u32,
	h: u32,
	depth: u8,
	colour: u8,
	palette: &[[u8; 3]],
	rows: &[Vec<u8>],
	bpp: usize,
	split_idat: bool,
	interlace: u8,
) -> Vec<u8> {
	let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
	let mut ihdr = Vec::new();
	ihdr.extend_from_slice(&w.to_be_bytes());
	ihdr.extend_from_slice(&h.to_be_bytes());
	ihdr.extend_from_slice(&[depth, colour, 0, 0, interlace]);
	chunk(&mut out, b"IHDR", &ihdr);
	if !palette.is_empty() {
		chunk(
			&mut out,
			b"PLTE",
			&palette.iter().flatten().copied().collect::<Vec<u8>>(),
		);
	}
	let mut raw = Vec::new();
	let zeros = vec![0u8; rows[0].len()];
	for (y, row) in rows.iter().enumerate() {
		let f = (y % 5) as u8;
		raw.push(f);
		let up = if y > 0 { &rows[y - 1] } else { &zeros };
		for i in 0..row.len() {
			let left = if i >= bpp { row[i - bpp] } else { 0 };
			let ul = if i >= bpp { up[i - bpp] } else { 0 };
			let p = match f {
				0 => 0,
				1 => left,
				2 => up[i],
				3 => ((u16::from(left) + u16::from(up[i])) / 2) as u8,
				_ => paeth(left, up[i], ul),
			};
			raw.push(row[i].wrapping_sub(p));
		}
	}
	let mut z = vec![0x78, 0x01];
	let blocks: Vec<&[u8]> = raw.chunks(60000).collect();
	for (i, b) in blocks.iter().enumerate() {
		z.push(u8::from(i + 1 == blocks.len()));
		z.extend_from_slice(&(b.len() as u16).to_le_bytes());
		z.extend_from_slice(&(!(b.len() as u16)).to_le_bytes());
		z.extend_from_slice(b);
	}
	let (mut a, mut b2) = (1u32, 0u32);
	for v in &raw {
		a = (a + u32::from(*v)) % 65521;
		b2 = (b2 + a) % 65521;
	}
	z.extend_from_slice(&((b2 << 16) | a).to_be_bytes());
	if split_idat {
		for part in z.chunks(7) {
			chunk(&mut out, b"IDAT", part);
		}
	} else {
		chunk(&mut out, b"IDAT", &z);
	}
	chunk(&mut out, b"IEND", &[]);
	out
}

fn decode(png: &[u8]) -> Result<Vec<Vec<u8>>, ImageError> {
	let mut win = Box::new([0u8; WINDOW]);
	let mut rows: Vec<Vec<u8>> = Vec::new();
	let (_, h) = decode_rows(&mut SliceSource::new(png), &mut win, |y, r| {
		if rows.len() <= y as usize {
			rows.resize(y as usize + 1, Vec::new());
		}
		rows[y as usize] = r.to_vec();
	})?;
	assert_eq!(rows.len(), h as usize);
	Ok(rows)
}

#[test]
fn grey_at_every_depth() {
	for depth in [1u8, 2, 4, 8, 16] {
		let (w, h) = (13u32, 6u32);
		let max = (1u32 << depth) - 1;
		let samples: Vec<Vec<u32>> = (0..h)
			.map(|y| (0..w).map(|x| (x * 5 + y * 3) % (max + 1)).collect())
			.collect();
		let rows: Vec<Vec<u8>> = samples
			.iter()
			.map(|r| {
				let mut bits: Vec<u8> = Vec::new();
				let mut acc = 0u32;
				let mut n = 0;
				for &s in r {
					if depth == 16 {
						bits.extend_from_slice(&(s as u16).to_be_bytes());
						continue;
					}
					acc = (acc << depth) | s;
					n += u32::from(depth);
					if n == 8 {
						bits.push(acc as u8);
						acc = 0;
						n = 0;
					}
				}
				if n > 0 {
					bits.push((acc << (8 - n)) as u8);
				}
				bits
			})
			.collect();
		let got = decode(&encode(
			w,
			h,
			depth,
			0,
			&[],
			&rows,
			usize::from(depth.max(8) / 8),
			false,
			0,
		))
		.unwrap();
		for (y, r) in samples.iter().enumerate() {
			let want: Vec<u8> = r
				.iter()
				.flat_map(|s| {
					let g = if depth == 16 {
						(s >> 8) as u8
					} else if depth == 8 {
						*s as u8
					} else {
						(s * 255 / ((1 << depth) - 1)) as u8
					};
					[g, g, g]
				})
				.collect();
			assert_eq!(got[y], want, "grey {depth} bit, row {y}");
		}
	}
}

#[test]
fn rgb_rgba_palette_and_grey_alpha() {
	let (w, h) = (9u32, 7u32);
	// RGB 8 and RGBA 8 (blended over white), split into many IDAT chunks.
	let rgb: Vec<Vec<u8>> = (0..h)
		.map(|y| {
			(0..w)
				.flat_map(|x| [(x * 20) as u8, (y * 30) as u8, (x * y) as u8])
				.collect()
		})
		.collect();
	let got = decode(&encode(w, h, 8, 2, &[], &rgb, 3, true, 0)).unwrap();
	assert_eq!(got, rgb);
	let rgba: Vec<Vec<u8>> = (0..h)
		.map(|y| {
			(0..w)
				.flat_map(|x| [200u8, 100, 50, (x * 28 + y) as u8])
				.collect()
		})
		.collect();
	let got = decode(&encode(w, h, 8, 6, &[], &rgba, 4, false, 0)).unwrap();
	let blend =
		|c: u8, a: u8| ((u16::from(c) * u16::from(a) + 255 * (255 - u16::from(a))) / 255) as u8;
	for (y, r) in rgba.iter().enumerate() {
		let want: Vec<u8> = r
			.chunks(4)
			.flat_map(|p| [blend(p[0], p[3]), blend(p[1], p[3]), blend(p[2], p[3])])
			.collect();
		assert_eq!(got[y], want);
	}
	// 16-bit RGB takes the high bytes.
	let rgb16: Vec<Vec<u8>> = (0..h)
		.map(|_| {
			(0..w)
				.flat_map(|x| [(x * 9) as u8, 7, (x * 3) as u8, 9, 200, 1])
				.collect()
		})
		.collect();
	let got = decode(&encode(w, h, 16, 2, &[], &rgb16, 6, false, 0)).unwrap();
	assert_eq!(&got[0][..6], &[0, 0, 200, 9, 3, 200]);
	// Palette at 4 bits.
	let pal: Vec<[u8; 3]> = (0..16).map(|i| [i * 16, 255 - i * 16, i]).collect();
	let idx: Vec<Vec<u8>> = (0..h)
		.map(|y| {
			(0..(w as usize).div_ceil(2))
				.map(|i| (((i + y as usize) % 16) as u8) << 4 | ((i * 3 + 1) % 16) as u8)
				.collect()
		})
		.collect();
	let got = decode(&encode(w, h, 4, 3, &pal, &idx, 1, false, 0)).unwrap();
	for (y, r) in idx.iter().enumerate() {
		for x in 0..w as usize {
			let i = if x % 2 == 0 {
				r[x / 2] >> 4
			} else {
				r[x / 2] & 15
			} as usize;
			assert_eq!(&got[y][x * 3..x * 3 + 3], &pal[i]);
		}
	}
	// Grey with alpha.
	let ga: Vec<Vec<u8>> = (0..h)
		.map(|_| (0..w).flat_map(|x| [(x * 25) as u8, 128]).collect())
		.collect();
	let got = decode(&encode(w, h, 8, 4, &[], &ga, 2, false, 0)).unwrap();
	assert_eq!(got[0][0], blend(0, 128));
}

#[test]
fn interlaced_oversize_and_damaged_files_are_refused() {
	let rows = vec![vec![0u8; 3]; 2];
	assert_eq!(
		decode(&encode(1, 2, 8, 2, &[], &rows, 3, false, 1)).err(),
		Some(ImageError::Unsupported)
	);
	let mut big = encode(1, 2, 8, 2, &[], &rows, 3, false, 0);
	big[16..20].copy_from_slice(&5000u32.to_be_bytes());
	assert_eq!(decode(&big).err(), Some(ImageError::TooLarge));
	assert_eq!(decode(b"not a png").err(), Some(ImageError::BadFile));
}

#[test]
fn a_real_zlib_file_passes_its_checksums() {
	let bytes = std::fs::read(concat!(
		env!("CARGO_MANIFEST_DIR"),
		"/../../tests/docs/pic.png"
	))
	.unwrap();
	let rows = decode(&bytes).unwrap();
	assert_eq!((rows.len(), rows[0].len()), (48, 64 * 3));
}

#[test]
fn checksums_are_checked() {
	let rows: Vec<Vec<u8>> = (0..6u8)
		.map(|y| (0..30u8).map(|x| x.wrapping_mul(y)).collect())
		.collect();
	let good = encode(10, 6, 8, 2, &[], &rows, 3, true, 0);
	assert!(decode(&good).is_ok());
	// Each byte of the IHDR CRC, the IDAT CRC and the zlib Adler-32 matters; so does a byte of pixel data.
	let idat_crc = good.len() - 12 - 4; // the last chunk is IEND (12 bytes); the IDAT CRC ends before it
	for at in [29, 30, idat_crc, idat_crc + 3, 41, 60] {
		let mut b = good.clone();
		b[at] ^= 0x01;
		assert_eq!(decode(&b).err(), Some(ImageError::BadFile), "byte {at}");
	}
	// Extra data after the stream is refused too.
	let mut tail = encode(10, 6, 8, 2, &[], &rows, 3, false, 0);
	let n = tail.len();
	tail.splice(
		n - 12..n - 12,
		[0, 0, 0, 1, b'I', b'D', b'A', b'T', 0, 0, 0, 0, 0],
	);
	assert!(decode(&tail).is_err());
}

proptest! {
	#[test]
	fn damaged_pngs_never_panic(flips in proptest::collection::vec((any::<u16>(), any::<u8>()), 1..8), cut in any::<u16>()) {
		let rows: Vec<Vec<u8>> = (0..8u8).map(|y| (0..30u8).map(|x| x.wrapping_mul(y)).collect()).collect();
		let mut b = encode(10, 8, 8, 2, &[], &rows, 3, true, 0);
		for (i, v) in flips {
			let n = b.len();
			b[usize::from(i) % n] = v;
		}
		b.truncate(usize::from(cut) % (b.len() + 1));
		let _ = decode(&b);
	}
}
