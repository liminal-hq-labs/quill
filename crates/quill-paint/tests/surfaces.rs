// The surfaces and palettes: exact pixels for each drawing call, on the planar and the linear surface.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod common;

use common::planar_pixels;
use quill_core::layout::{FontRef, GLYPHS};
use quill_paint::palette::{
	dither16, dither256, index256, nearest16, rgb256, EXTRAS, EXTRA_BASE, WINDOWS16,
};
use quill_paint::raster::{Linear8, Planar4, PLANAR_BYTES};
use quill_paint::{crc32, Rect, Rgb, Surface};
use quill_strike::builder::{build, GlyphSpec, StrikeSpec};
use quill_strike::{SliceSource, Strikes};

/// A strike of 8 pixels with `A` (3 by 4, two bits: 0 1 2 / 3 3 3 / 1 0 1 / 2 2 0) drawn from (-1, 4).
fn tiny_font() -> Vec<u8> {
	let mut advances = [5u8; GLYPHS];
	advances[usize::from(b'A' - 0x20)] = 6;
	build(&[StrikeSpec {
		face: 0,
		style: 0,
		size: 12,
		pixels: 8,
		ascent: 6,
		descent: 2,
		leading: 0,
		bpp: 2,
		advances,
		glyphs: vec![(
			b'A',
			GlyphSpec {
				left: -1,
				top: 4,
				width: 3,
				height: 4,
				bits: vec![0b0001_1000, 0b1111_1100, 0b0100_0100, 0b1010_0000],
			},
		)],
	}])
}

fn font(size: u16) -> FontRef {
	FontRef {
		face: [0; 31],
		face_len: 0,
		family: 1,
		size,
		flags: 0,
	}
}

fn planar(bytes: &[u8], draw: impl FnOnce(&mut Planar4<'_, SliceSource<'_>>)) -> Vec<Rgb> {
	let st = Strikes::parse(bytes, bytes.len() as u32).unwrap();
	let mut src = SliceSource(bytes);
	let mut buf = vec![0xFFu8; PLANAR_BYTES];
	{
		let mut sf = Planar4::new(&mut buf, st, &mut src).unwrap();
		sf.fill_rect(Rect::new(0, 0, 640, 480), Rgb::WHITE);
		draw(&mut sf);
	}
	planar_pixels(&buf)
		.iter()
		.map(|i| WINDOWS16[usize::from(*i)])
		.collect()
}

fn linear(bytes: &[u8], draw: impl FnOnce(&mut Linear8<'_, SliceSource<'_>>)) -> Vec<Rgb> {
	let st = Strikes::parse(bytes, bytes.len() as u32).unwrap();
	let mut src = SliceSource(bytes);
	let mut buf = vec![0u8; 640 * 480];
	{
		let mut sf = Linear8::new(&mut buf, 640, 480, 640, st, &mut src).unwrap();
		sf.fill_rect(Rect::new(0, 0, 640, 480), Rgb::WHITE);
		draw(&mut sf);
	}
	buf.iter().map(|i| rgb256(*i)).collect()
}

fn at(img: &[Rgb], x: usize, y: usize) -> Rgb {
	img[y * 640 + x]
}

/// Draws with `f` on both surfaces and checks they agree, returning the picture.
macro_rules! on_both {
	($bytes:expr, |$sf:ident| $body:block) => {{
		let a = planar($bytes, |$sf| $body);
		let b = linear($bytes, |$sf| $body);
		assert!(a == b, "the surfaces drew different pictures");
		a
	}};
}

#[test]
fn rectangles_lines_and_clipping_are_exact() {
	let bytes = tiny_font();
	let img = on_both!(&bytes, |sf| {
		sf.fill_rect(Rect::new(3, 5, 12, 8), Rgb(255, 0, 0));
		sf.hline(20, 30, 40, Rgb::BLACK);
		sf.vline(50, 60, 70, Rgb(0, 0, 255));
		sf.clip(Some(Rect::new(100, 100, 110, 110)));
		sf.fill_rect(Rect::new(90, 90, 120, 120), Rgb(0, 128, 0));
		sf.clip(None);
		sf.fill_rect(Rect::new(-10, -10, 2, 2), Rgb(255, 255, 0));
		sf.fill_rect(Rect::new(638, 478, 700, 700), Rgb(255, 0, 255));
		sf.fill_rect(Rect::new(5, 15, 5, 19), Rgb::BLACK);
	});
	let red = Rgb(255, 0, 0);
	assert_eq!(
		[
			at(&img, 3, 5),
			at(&img, 11, 7),
			at(&img, 12, 7),
			at(&img, 3, 8)
		],
		[red, red, Rgb::WHITE, Rgb::WHITE]
	);
	assert_eq!(
		[
			at(&img, 19, 40),
			at(&img, 20, 40),
			at(&img, 29, 40),
			at(&img, 30, 40)
		],
		[Rgb::WHITE, Rgb::BLACK, Rgb::BLACK, Rgb::WHITE]
	);
	assert_eq!(
		[
			at(&img, 50, 59),
			at(&img, 50, 60),
			at(&img, 50, 69),
			at(&img, 50, 70)
		],
		[Rgb::WHITE, Rgb(0, 0, 255), Rgb(0, 0, 255), Rgb::WHITE]
	);
	let g = Rgb(0, 128, 0);
	assert_eq!(
		[
			at(&img, 99, 105),
			at(&img, 100, 105),
			at(&img, 109, 105),
			at(&img, 110, 105),
			at(&img, 105, 110)
		],
		[Rgb::WHITE, g, g, Rgb::WHITE, Rgb::WHITE]
	);
	assert_eq!(
		[at(&img, 0, 0), at(&img, 1, 1), at(&img, 2, 2)],
		[Rgb(255, 255, 0), Rgb(255, 255, 0), Rgb::WHITE]
	);
	assert_eq!(
		[at(&img, 639, 479), at(&img, 637, 479)],
		[Rgb(255, 0, 255), Rgb::WHITE]
	);
	assert_eq!(
		at(&img, 5, 16),
		Rgb::WHITE,
		"an empty rectangle draws nothing"
	);
}

#[test]
fn a_glyph_is_drawn_from_its_box_with_greys_for_black_text() {
	let bytes = tiny_font();
	let smooth = on_both!(&bytes, |sf| {
		let f = sf.font(&font(12));
		assert_eq!(sf.advance(f, b'A'), 6);
		sf.glyph_run(f, 30, &[(b'A', 100)], Rgb::BLACK, true);
	});
	// Left -1, top 4: the bitmap's top left is (99, 26). Levels: 0 1 2 / 3 3 3 / 1 0 1 / 2 2 0 as white, silver, grey, black.
	let (w, s, g, b) = (Rgb::WHITE, Rgb::SILVER, Rgb::GREY, Rgb::BLACK);
	let want = [[w, s, g], [b, b, b], [s, w, s], [g, g, w]];
	for (y, row) in want.iter().enumerate() {
		for (x, c) in row.iter().enumerate() {
			assert_eq!(at(&smooth, 99 + x, 26 + y), *c, "({x}, {y})");
		}
	}
	assert_eq!(at(&smooth, 98, 26), Rgb::WHITE);
	assert_eq!(at(&smooth, 99, 30), Rgb::WHITE);
	// Not smooth, or not black: a level of 2 or more is ink in the text colour.
	let red = Rgb(255, 0, 0);
	let hard = on_both!(&bytes, |sf| {
		let f = sf.font(&font(12));
		sf.glyph_run(f, 30, &[(b'A', 100)], red, true);
	});
	let want = [[w, w, red], [red, red, red], [w, w, w], [red, red, w]];
	for (y, row) in want.iter().enumerate() {
		for (x, c) in row.iter().enumerate() {
			assert_eq!(at(&hard, 99 + x, 26 + y), *c, "({x}, {y})");
		}
	}
	let black_hard = on_both!(&bytes, |sf| {
		let f = sf.font(&font(12));
		sf.glyph_run(f, 30, &[(b'A', 100)], Rgb::BLACK, false);
	});
	assert_eq!(at(&black_hard, 99, 27), Rgb::BLACK);
	assert_eq!(at(&black_hard, 99, 26), Rgb::WHITE, "level 0 is paper");
	assert_eq!(
		at(&black_hard, 100, 26),
		Rgb::WHITE,
		"level 1 is below the threshold when not smoothing"
	);
}

#[test]
fn a_larger_size_draws_every_pixel_as_a_block() {
	let bytes = tiny_font();
	// 12 point is 16 pixels, twice the strike's 8: scale 2. Left -1 and top 4 become -2 and 8.
	let img = on_both!(&bytes, |sf| {
		let f = sf.font(&font(24));
		assert_eq!(sf.advance(f, b'A'), 12);
		sf.glyph_run(f, 50, &[(b'A', 100)], Rgb::BLACK, true);
	});
	assert_eq!(at(&img, 98, 42), Rgb::WHITE);
	assert_eq!(
		at(&img, 100, 42),
		Rgb::SILVER,
		"level 1 of the first row, second column, covers 100..102"
	);
	assert_eq!(at(&img, 101, 43), Rgb::SILVER);
	assert_eq!(at(&img, 102, 42), Rgb::GREY);
	assert_eq!(at(&img, 98, 44), Rgb::BLACK);
	assert_eq!(at(&img, 103, 45), Rgb::BLACK);
	assert_eq!(at(&img, 104, 44), Rgb::WHITE);
}

#[test]
fn a_four_bit_bitmap_goes_through_its_palette() {
	let bytes = tiny_font();
	let pal: [Rgb; 16] = core::array::from_fn(|i| WINDOWS16[15 - i]);
	// Two rows of three pixels: 0 1 2 / 15 14 13, rows of two bytes.
	let rows = [0x01, 0x20, 0xFE, 0xD0];
	let img = on_both!(&bytes, |sf| {
		sf.bitmap4(10, 10, &rows, 3, 2, &pal);
	});
	assert_eq!(
		[at(&img, 10, 10), at(&img, 11, 10), at(&img, 12, 10)],
		[pal[0], pal[1], pal[2]]
	);
	assert_eq!(
		[at(&img, 10, 11), at(&img, 11, 11), at(&img, 12, 11)],
		[pal[15], pal[14], pal[13]]
	);
	assert_eq!(at(&img, 13, 10), Rgb::WHITE);
	assert_eq!(at(&img, 10, 12), Rgb::WHITE);
}

#[test]
fn inverting_twice_restores_the_planar_surface() {
	let bytes = tiny_font();
	let once = planar(&bytes, |sf| {
		sf.fill_rect(Rect::new(0, 0, 640, 480), Rgb(0, 128, 128));
		sf.invert_rect(Rect::new(3, 3, 77, 9));
	});
	assert_eq!(
		at(&once, 3, 3),
		WINDOWS16[15 - 3],
		"teal inverts to its complement index"
	);
	assert_eq!(at(&once, 2, 3), Rgb(0, 128, 128));
	let twice = planar(&bytes, |sf| {
		sf.fill_rect(Rect::new(0, 0, 640, 480), Rgb(0, 128, 128));
		sf.invert_rect(Rect::new(3, 3, 77, 9));
		sf.invert_rect(Rect::new(3, 3, 77, 9));
	});
	assert_eq!(at(&twice, 3, 3), Rgb(0, 128, 128));
	let lin = linear(&bytes, |sf| sf.invert_rect(Rect::new(0, 0, 4, 4)));
	assert_eq!(at(&lin, 0, 0), Rgb::BLACK, "white inverts to black");
}

#[test]
fn a_picture_row_is_dithered_by_position_whatever_the_clip() {
	let bytes = tiny_font();
	let row: Vec<u8> = (0..64u32)
		.flat_map(|i| [(i * 4) as u8, 200, 255 - (i * 4) as u8])
		.collect();
	for draw_with_clip in [false, true] {
		let a = planar(&bytes, |sf| sf.rgb_row(100, 50, &row));
		let b = planar(&bytes, |sf| {
			if draw_with_clip {
				sf.clip(Some(Rect::new(110, 0, 130, 100)));
			}
			sf.rgb_row(100, 50, &row);
		});
		for x in 110..130 {
			assert_eq!(at(&a, x, 50), at(&b, x, 50), "clip {draw_with_clip} x {x}");
		}
		if draw_with_clip {
			assert_eq!(at(&b, 109, 50), Rgb::WHITE);
			assert_eq!(at(&b, 130, 50), Rgb::WHITE);
		}
	}
	let c = linear(&bytes, |sf| sf.rgb_row(100, 50, &row));
	assert_ne!(
		at(&c, 100, 50),
		at(&c, 163, 50),
		"a gradient changes colour"
	);
}

#[test]
fn surfaces_refuse_buffers_that_are_too_short() {
	let bytes = tiny_font();
	let st = Strikes::parse(&bytes, bytes.len() as u32).unwrap();
	let mut src = SliceSource(&bytes);
	let mut short = vec![0u8; PLANAR_BYTES - 1];
	assert!(Planar4::new(&mut short, st, &mut src).is_none());
	let mut buf = vec![0u8; 100];
	assert!(Linear8::new(&mut buf, 10, 10, 10, st, &mut src).is_some());
	assert!(Linear8::new(&mut buf, 10, 11, 10, st, &mut src).is_none());
	assert!(Linear8::new(&mut buf, 10, 10, 9, st, &mut src).is_none());
	assert!(Linear8::new(&mut buf, 0, 10, 10, st, &mut src).is_none());
	assert!(Linear8::new(&mut buf, 10, -1, 10, st, &mut src).is_none());
	assert!(Linear8::new(&mut buf, 10, 10, usize::MAX, st, &mut src).is_none());
}

#[test]
fn the_palettes_hold_the_windows_colours() {
	for (i, c) in WINDOWS16.iter().enumerate() {
		assert_eq!(nearest16(*c), i as u8);
		assert_eq!(rgb256(index256(*c)), *c, "{c:?} is exact at 256 colours");
	}
	for (i, e) in EXTRAS.iter().enumerate() {
		assert_eq!(index256(*e), EXTRA_BASE + i as u8);
	}
	assert_eq!(index256(Rgb::BLACK), 0);
	assert_eq!(index256(Rgb::WHITE), 215);
	assert_eq!(rgb256(36 + 6 * 2 + 3), Rgb(51, 102, 153));
	assert_eq!(rgb256(255), Rgb::BLACK);
	assert_eq!(nearest16(Rgb(10, 10, 10)), 0);
	assert_eq!(nearest16(Rgb(250, 250, 250)), 15);
	for x in 0..8 {
		for y in 0..8 {
			assert!(dither16([100, 150, 200], x, y) < 16);
			assert!(
				index256(rgb256(dither256([100, 150, 200], x, y)))
					== dither256([100, 150, 200], x, y)
			);
		}
	}
}

#[test]
fn the_checksum_of_a_frame_is_the_zlib_crc32() {
	assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
}
