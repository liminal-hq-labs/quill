// The QUILL.FNT reader: a hand-built strike, the test font, strike choice and scaling, and files damaged in every way.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use proptest::prelude::*;
use quill_core::doc::cflag;
use quill_core::layout::{FontRef, Measure, GLYPHS};
use quill_strike::builder::{build, GlyphSpec, StrikeSpec};
use quill_strike::testfont::test_font as build_test_font;
use quill_strike::{
	crc32, index_len, Error, SliceSource, StrikeId, StrikeMeasure, Strikes, BOXES_LEN,
	MAX_GLYPH_BYTES, MAX_SCALE,
};

fn test_font() -> Vec<u8> {
	static BYTES: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();
	BYTES.get_or_init(build_test_font).clone()
}

fn font(face: &str, family: u8, size: u16, flags: u16) -> FontRef {
	let mut f = [0u8; 31];
	f[..face.len()].copy_from_slice(face.as_bytes());
	FontRef {
		face: f,
		face_len: face.len() as u8,
		family,
		size,
		flags,
	}
}

/// One face, one 8-pixel strike with `A` (a 3 by 4 block, two bits) and `B` (a 2 by 2 block, one bit).
fn tiny_parts() -> Vec<StrikeSpec> {
	let mut advances = [5u8; GLYPHS];
	advances[usize::from(b'A' - 0x20)] = 6;
	let a = GlyphSpec {
		left: -1,
		top: 4,
		width: 3,
		height: 4,
		// Rows of three pixels at 2 bits: 0 1 2 | 3 3 3 | 1 0 1 | 2 2 0.
		bits: vec![0b0001_1000, 0b1111_1100, 0b0100_0100, 0b1010_0000],
	};
	let b = GlyphSpec {
		left: 0,
		top: 2,
		width: 2,
		height: 2,
		bits: vec![0b1000_0000, 0b0100_0000],
	};
	vec![StrikeSpec {
		face: 0,
		style: 0,
		size: 12,
		pixels: 8,
		ascent: 6,
		descent: 2,
		leading: 1,
		bpp: 2,
		advances,
		glyphs: vec![(b'A', a), (b'B', b)],
	}]
}

fn tiny() -> Vec<u8> {
	build(&tiny_parts())
}

fn parsed(bytes: &[u8]) -> Result<Strikes<'_>, Error> {
	Strikes::parse(bytes, bytes.len() as u32)
}

#[test]
fn a_hand_built_strike_reads_back_exactly() {
	let bytes = tiny();
	let st = parsed(&bytes).unwrap();
	assert_eq!(st.strike_count(), 1);
	let id = StrikeId::default_first(&st);
	let info = st.strike(id);
	assert_eq!(
		(
			info.size,
			info.pixels,
			info.ascent,
			info.descent,
			info.leading,
			info.bpp
		),
		(12, 8, 6, 2, 1, 2)
	);
	assert_eq!(
		(
			st.advance(id, b'A'),
			st.advance(id, b'B'),
			st.advance(id, b' ')
		),
		(6, 5, 5)
	);
	assert_eq!((st.advance(id, 0x1F), st.advance(id, 0xFF)), (0, 5));

	let mut src = SliceSource(&bytes);
	let mut bits = [0u8; MAX_GLYPH_BYTES];
	let g = st.glyph(&mut src, id, b'A', &mut bits).unwrap();
	assert_eq!(
		(g.left, g.top, g.width, g.height, g.stride, g.bpp),
		(-1, 4, 3, 4, 1, 2)
	);
	let rows: Vec<Vec<u8>> = (0..4)
		.map(|y| (0..3).map(|x| g.level(&bits, x, y)).collect())
		.collect();
	assert_eq!(
		rows,
		vec![vec![0, 1, 2], vec![3, 3, 3], vec![1, 0, 1], vec![2, 2, 0]]
	);
	assert_eq!(g.level(&bits, 3, 0), 0, "past the width is paper");
	assert_eq!(g.level(&bits, 0, 4), 0, "past the height is paper");
}

#[test]
fn the_unset_and_blank_characters_have_no_ink() {
	let bytes = tiny();
	let st = parsed(&bytes).unwrap();
	let id = StrikeId::default_first(&st);
	let mut src = SliceSource(&bytes);
	let mut bits = [0u8; MAX_GLYPH_BYTES];
	let g = st.glyph(&mut src, id, b'z', &mut bits).unwrap();
	assert_eq!((g.width, g.height), (0, 0));
	assert!(st.glyph(&mut src, id, 0x1F, &mut bits).is_none());
	assert!(st
		.glyph(&mut src, StrikeId::out_of_range(), b'A', &mut bits)
		.is_none());
}

#[test]
fn a_one_bit_bitmap_reads_as_full_ink() {
	let mut f = tiny_parts();
	f[0].bpp = 1;
	f[0].glyphs = vec![(
		b'B',
		GlyphSpec {
			left: 0,
			top: 2,
			width: 9,
			height: 1,
			bits: vec![0b1000_0000, 0b1000_0000],
		},
	)];
	let bytes = build(&f);
	let st = parsed(&bytes).unwrap();
	let mut bits = [0u8; MAX_GLYPH_BYTES];
	let id = StrikeId::default_first(&st);
	let g = st
		.glyph(&mut SliceSource(&bytes), id, b'B', &mut bits)
		.unwrap();
	assert_eq!(
		(
			g.stride,
			g.level(&bits, 0, 0),
			g.level(&bits, 1, 0),
			g.level(&bits, 8, 0)
		),
		(2, 3, 0, 3)
	);
}

#[test]
fn the_measure_reports_96_dots_and_the_strikes_own_integers() {
	let bytes = tiny();
	let st = parsed(&bytes).unwrap();
	let mut m = StrikeMeasure::new(&st);
	assert_eq!(m.resolution(), (96, 96));
	// 6 point is 12 half points: 8 pixels at 96 dpi, the strike's own size.
	let f = font("Tiny", 2, 12, 0);
	let mut w = [0u16; GLYPHS];
	m.widths(&f, &mut w);
	assert_eq!((w[usize::from(b'A' - 0x20)], w[0]), (6, 5));
	let me = m.metrics(&f);
	assert_eq!((me.ascent, me.descent, me.leading), (6, 2, 1));
	// Twice the size: the same strike drawn twice as large, every integer doubled.
	let f2 = font("Tiny", 2, 24, 0);
	m.widths(&f2, &mut w);
	assert_eq!((w[usize::from(b'A' - 0x20)], w[0]), (12, 10));
	let me = m.metrics(&f2);
	assert_eq!((me.ascent, me.descent, me.leading), (12, 4, 2));
	assert_eq!(st.strike_for(&f2), (StrikeId::default_first(&st), 2));
}

#[test]
fn the_test_font_parses_and_has_every_face_in_four_styles_and_two_sizes() {
	let bytes = test_font();
	let st = parsed(&bytes).unwrap();
	assert_eq!(st.strike_count(), 24);
	assert_eq!(
		index_len(&bytes[..32]).unwrap() as usize,
		32 + 24 * (24 + 224)
	);
	assert!(st.verify(&mut SliceSource(&bytes)), "the checksum holds");
	let mut damaged = bytes.clone();
	let last = damaged.len() - 1;
	damaged[last] ^= 1;
	assert!(!st.verify(&mut SliceSource(&damaged)));
	assert!(
		!st.verify(&mut SliceSource(&bytes[..bytes.len() - 1])),
		"a short file fails"
	);
	assert_eq!(build_test_font(), bytes, "the generator is deterministic");
}

#[test]
fn the_face_is_chosen_by_family_and_falls_back_to_a_face_that_has_strikes() {
	let bytes = test_font();
	let st = parsed(&bytes).unwrap();
	let face = |f: &FontRef| st.strike(st.strike_for(f).0).face;
	assert_eq!(face(&font("Times New Roman", 1, 24, 0)), 0);
	assert_eq!(face(&font("Arial", 2, 24, 0)), 1);
	assert_eq!(face(&font("Courier New", 3, 24, 0)), 2);
	assert_eq!(
		face(&font("Wingdings", 0, 24, 0)),
		0,
		"an unknown family is the first face"
	);
	assert_eq!(face(&font("Whatever", 9, 24, 0)), 0);
	// A file with only the modern face serves every request with it.
	let mut s = tiny_parts();
	s[0].face = 2;
	let bytes = build(&s);
	let st = parsed(&bytes).unwrap();
	assert_eq!(st.strike(st.strike_for(&font("Arial", 2, 24, 0)).0).face, 2);
}

#[test]
fn a_style_is_matched_by_the_fewest_wrong_bits() {
	let mut s = tiny_parts();
	let mut bold = s[0].clone();
	bold.style = 1;
	s.push(bold);
	let bytes = build(&s);
	let st = parsed(&bytes).unwrap();
	let style = |flags| {
		st.strike(st.strike_for(&font("Tiny", 2, 12, flags)).0)
			.style
	};
	assert_eq!(style(0), 0);
	assert_eq!(style(cflag::BOLD), 1);
	assert_eq!(
		style(cflag::ITALIC),
		0,
		"no italic: the nearest is the regular"
	);
	assert_eq!(
		style(cflag::BOLD | cflag::ITALIC),
		1,
		"bold italic with no such strike takes the bold"
	);
}

#[test]
fn sizes_pick_the_nearest_pixel_size_and_the_smaller_scale_on_a_tie() {
	let bytes = test_font();
	let st = parsed(&bytes).unwrap();
	let pick = |hp: u16| {
		let (id, k) = st.strike_for(&font("Sans", 2, hp, 0));
		(st.strike(id).pixels, k)
	};
	// Strikes are 12 and 24 pixels (9 and 18 point). Pixels wanted = half points * 96 / 144, rounded.
	assert_eq!(pick(18), (12, 1), "9 point is 12 pixels");
	assert_eq!(pick(36), (24, 1), "18 point is 24 pixels");
	assert_eq!(
		pick(24),
		(12, 1),
		"12 point is 16 pixels: 12 is the nearest"
	);
	assert_eq!(pick(30), (24, 1), "15 point is 20 pixels");
	assert_eq!(pick(72), (24, 2), "36 point is 48 pixels: 24 doubled");
	assert_eq!(
		pick(96),
		(24, 3),
		"48 point is 64 pixels: 72 is nearer than 48"
	);
	assert_eq!(
		pick(54),
		(12, 3),
		"27 point is 36 pixels: 12 tripled is exact"
	);
}

#[test]
fn big_sizes_use_two_and_three_times() {
	let bytes = test_font();
	let st = parsed(&bytes).unwrap();
	let k = |hp: u16| st.strike_for(&font("Serif", 1, hp, 0)).1;
	assert_eq!(k(72), 2, "36 point");
	assert_eq!(
		k(144),
		MAX_SCALE,
		"72 point is 96 pixels: 24 tripled is 72, the largest there is"
	);
	assert_eq!(k(18), 1);
}

#[test]
fn superscript_sizes_go_through_the_same_mapping() {
	let bytes = test_font();
	let st = parsed(&bytes).unwrap();
	// font_ref gives a raised character two thirds of its size, rounded up: 12 point is 24 half points, 16 of them.
	let full = st.strike_for(&font("Sans", 2, 24, 0));
	let raised = st.strike_for(&font("Sans", 2, (24 * 2u16).div_ceil(3), 0));
	assert_eq!(st.strike(raised.0).pixels, 12);
	assert_eq!(full.1, 1);
}

#[test]
fn bad_files_are_refused_for_the_right_reason() {
	let good = tiny();
	let len = good.len() as u32;
	let patch = |at: usize, v: &[u8]| {
		let mut b = good.clone();
		b[at..at + v.len()].copy_from_slice(v);
		b
	};
	let dir = 32; // the header
	let cases: Vec<(&str, Vec<u8>, Error)> = vec![
		("magic", patch(0, b"QFNX"), Error::BadMagic),
		("version", patch(4, &[2, 0]), Error::BadMagic),
		("no strikes", patch(6, &[0, 0]), Error::BadHeader),
		("too many strikes", patch(6, &[0, 3]), Error::BadHeader),
		("glyph count", patch(8, &[223, 0]), Error::BadHeader),
		("first character", patch(10, &[0x21]), Error::BadHeader),
		("entry length", patch(11, &[32]), Error::BadHeader),
		("directory offset", patch(12, &[40]), Error::BadHeader),
		(
			"file length",
			patch(16, &(len + 1).to_le_bytes()),
			Error::BadHeader,
		),
		("reserved in the header", patch(24, &[1]), Error::BadHeader),
		("strike face", patch(dir, &[3]), Error::BadStrike),
		("style bits", patch(dir + 1, &[4]), Error::BadStrike),
		("zero size", patch(dir + 2, &[0, 0]), Error::BadStrike),
		("zero pixels", patch(dir + 4, &[0]), Error::BadStrike),
		("huge pixels", patch(dir + 4, &[65]), Error::BadStrike),
		("bits per pixel", patch(dir + 8, &[3]), Error::BadStrike),
		(
			"reserved in a strike",
			patch(dir + 9, &[1]),
			Error::BadStrike,
		),
		(
			"advances elsewhere",
			patch(dir + 12, &40u32.to_le_bytes()),
			Error::Range,
		),
		(
			"data too short",
			patch(dir + 20, &10u32.to_le_bytes()),
			Error::BadStrike,
		),
		(
			"data too long",
			patch(dir + 20, &32769u32.to_le_bytes()),
			Error::BadStrike,
		),
		(
			"data inside the index",
			patch(dir + 16, &40u32.to_le_bytes()),
			Error::Range,
		),
		(
			"data past the file",
			patch(dir + 16, &(len - 100).to_le_bytes()),
			Error::Range,
		),
		(
			"data offset overflow",
			patch(dir + 16, &u32::MAX.to_le_bytes()),
			Error::Range,
		),
	];
	for (what, bytes, want) in cases {
		assert_eq!(Strikes::parse(&bytes, len).err(), Some(want), "{what}");
	}
	assert!(Strikes::parse(&good, len).is_ok());
	assert_eq!(
		Strikes::parse(&good, len + 1).err(),
		Some(Error::BadHeader),
		"the file is not the length it says"
	);
}

#[test]
fn strikes_must_be_sorted_without_repeats() {
	let strikes = tiny_parts();
	let mut two = strikes.clone();
	two.push(two[0].clone());
	assert_eq!(
		parsed(&build(&two)).err(),
		Some(Error::BadStrike),
		"two strikes of one kind"
	);
	let mut out_of_order = strikes.clone();
	let mut first = out_of_order[0].clone();
	first.size = 20;
	out_of_order.insert(0, first);
	assert_eq!(
		parsed(&build(&out_of_order)).err(),
		Some(Error::BadStrike),
		"descending sizes"
	);
	let mut by_face = strikes.clone();
	let mut other = by_face[0].clone();
	other.face = 1;
	other.size = 6;
	by_face.push(other);
	assert!(parsed(&build(&by_face)).is_ok(), "order is face first");
}

#[test]
fn data_blocks_that_overlap_are_refused() {
	let strikes = tiny_parts();
	let mut two = strikes.clone();
	let mut second = two[0].clone();
	second.size = 13;
	two.push(second);
	let good = build(&two);
	assert!(parsed(&good).is_ok());
	// Point the second block at the start of the first.
	let first_off = u32::from_le_bytes(good[32 + 16..32 + 20].try_into().unwrap());
	let mut bad = good.clone();
	bad[32 + 24 + 16..32 + 24 + 20].copy_from_slice(&first_off.to_le_bytes());
	assert_eq!(parsed(&bad).err(), Some(Error::Range));
}

#[test]
fn a_truncated_index_is_refused_at_every_length() {
	let bytes = test_font();
	let ilen = index_len(&bytes).unwrap() as usize;
	for n in 0..ilen {
		let r = Strikes::parse(&bytes[..n], bytes.len() as u32);
		assert!(r.is_err(), "{n} bytes of {ilen}");
	}
	assert!(
		Strikes::parse(&bytes[..ilen], bytes.len() as u32).is_ok(),
		"the index alone is enough"
	);
	assert_eq!(index_len(&bytes[..31]), Err(Error::Truncated));
}

#[test]
fn a_file_cut_short_after_parsing_gives_no_glyph_not_a_panic() {
	let bytes = test_font();
	let st = parsed(&bytes).unwrap();
	let ilen = index_len(&bytes).unwrap() as usize;
	let mut bits = [0u8; MAX_GLYPH_BYTES];
	for cut in [ilen, ilen + 100, bytes.len() / 2, bytes.len() - 1] {
		let mut src = SliceSource(&bytes[..cut]);
		for id in st.ids() {
			for ch in 0x20..=0xFFu8 {
				let _ = st.glyph(&mut src, id, ch, &mut bits);
			}
		}
	}
}

#[test]
fn a_damaged_box_gives_no_glyph() {
	let bytes = tiny();
	let st = parsed(&bytes).unwrap();
	let id = StrikeId::default_first(&st);
	let data = st.strike(id).data_off as usize;
	let a = data + usize::from(b'A' - 0x20) * 6;
	let mut bits = [0u8; MAX_GLYPH_BYTES];
	let mut try_box = |patch: &[(usize, u8)]| {
		let mut b = bytes.clone();
		for (o, v) in patch {
			b[a + o] = *v;
		}
		st.glyph(&mut SliceSource(&b), id, b'A', &mut bits)
	};
	assert!(try_box(&[]).is_some());
	assert!(try_box(&[(2, 65)]).is_none(), "wider than 64");
	assert!(try_box(&[(3, 65)]).is_none(), "taller than 64");
	assert!(
		try_box(&[(4, 0xFF), (5, 0xFF)]).is_none(),
		"a bitmap past the block"
	);
	assert!(
		try_box(&[(2, 64), (3, 64), (4, 0), (5, 0)]).is_none(),
		"a bitmap the size of the box does not fit this block"
	);
}

#[test]
fn a_block_of_the_shortest_length_holds_one_bitmap() {
	let mut f = tiny_parts();
	let big = GlyphSpec {
		left: 0,
		top: 1,
		width: 8,
		height: 1,
		bits: vec![0xFF],
	};
	f[0].glyphs = vec![(b'A', big)];
	f[0].bpp = 1;
	let bytes = build(&f);
	let st = parsed(&bytes).unwrap();
	let id = StrikeId::default_first(&st);
	assert_eq!(st.strike(id).data_len as usize, BOXES_LEN + 1);
	let mut bits = [0u8; MAX_GLYPH_BYTES];
	let g = st
		.glyph(&mut SliceSource(&bytes), id, b'A', &mut bits)
		.unwrap();
	assert_eq!((0..8).map(|x| g.level(&bits, x, 0)).sum::<u8>(), 24);
}

/// Everything a client does with a parsed file, which must never panic whatever the bytes.
fn exercise(st: &Strikes<'_>, file: &[u8]) {
	let mut m = StrikeMeasure::new(st);
	let mut bits = [0u8; MAX_GLYPH_BYTES];
	for (face, family, flags, size) in [
		("Arial", 2u8, 0u16, 24u16),
		("x", 0, 3, 1),
		("Courier New", 3, 1, 144),
		("", 1, 2, 65535),
	] {
		let f = font(face, family, size, flags);
		let (id, k) = st.strike_for(&f);
		assert!((1..=MAX_SCALE).contains(&k));
		let mut w = [0u16; GLYPHS];
		m.widths(&f, &mut w);
		let _ = m.metrics(&f);
		for ch in [0x20u8, b'A', 0x95, 0xFF] {
			let _ = st.glyph(&mut SliceSource(file), id, ch, &mut bits);
		}
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(2000))]

	#[test]
	fn random_bytes_never_panic(bytes in proptest::collection::vec(any::<u8>(), 0..600)) {
		let _ = index_len(&bytes);
		if let Ok(st) = Strikes::parse(&bytes, bytes.len() as u32) {
			exercise(&st, &bytes);
		}
	}

	#[test]
	fn random_bytes_behind_a_good_magic_never_panic(tail in proptest::collection::vec(any::<u8>(), 0..2000)) {
		let mut bytes = b"QFNT\x01\x00\x02\x00\xE0\x00\x20\x18\x20\x00\x00\x00".to_vec();
		bytes.extend_from_slice(&tail);
		let _ = Strikes::parse(&bytes, bytes.len() as u32);
	}

	#[test]
	fn the_test_font_with_bytes_overwritten_never_panics(
		edits in proptest::collection::vec((0usize..400_000, any::<u8>()), 1..12),
		cut in proptest::option::of(0usize..400_000),
	) {
		let mut bytes = test_font();
		let full = bytes.len() as u32;
		for (at, v) in edits {
			let n = bytes.len();
			bytes[at % n] = v;
		}
		if let Some(c) = cut {
			bytes.truncate(c % (bytes.len() + 1));
		}
		for claimed in [bytes.len() as u32, full] {
			if let Ok(st) = Strikes::parse(&bytes, claimed) {
				exercise(&st, &bytes);
			}
		}
	}

	#[test]
	fn the_data_of_a_good_index_may_be_anything(noise in proptest::collection::vec(any::<u8>(), 0..4000), at in 0usize..60_000) {
		let mut bytes = test_font();
		let ilen = index_len(&bytes).unwrap() as usize;
		let total = bytes.len();
		for (i, v) in noise.iter().enumerate() {
			let p = ilen + (at + i) % (total - ilen);
			bytes[p] = *v;
		}
		let st = parsed(&bytes).unwrap();
		exercise(&st, &bytes);
		let mut src = SliceSource(&bytes);
		let mut bits = [0u8; MAX_GLYPH_BYTES];
		for id in st.ids() {
			for ch in 0..=255u8 {
				if let Some(g) = st.glyph(&mut src, id, ch, &mut bits) {
					for y in 0..g.height {
						for x in 0..g.width {
							assert!(g.level(&bits, x, y) <= 3);
						}
					}
				}
			}
		}
	}

	#[test]
	fn every_scale_and_size_is_in_range_and_the_widths_are_whole_multiples(size in 1u16..=u16::MAX, flags in 0u16..4, family in 0u8..5) {
		let bytes = test_font();
		let st = parsed(&bytes).unwrap();
		let f = font("Anything", family, size, flags);
		let (id, k) = st.strike_for(&f);
		prop_assert!((1..=MAX_SCALE).contains(&k));
		let mut m = StrikeMeasure::new(&st);
		let mut w = [0u16; GLYPHS];
		m.widths(&f, &mut w);
		for (i, v) in w.iter().enumerate() {
			prop_assert_eq!(*v, u16::from(st.advance(id, 0x20 + i as u8)) * u16::from(k));
		}
	}
}

#[test]
fn the_checksum_is_the_zlib_crc32() {
	assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
	assert_eq!(
		quill_strike::crc32_update(crc32(b"1234"), b"56789"),
		0xCBF4_3926
	);
	assert_eq!(crc32(b""), 0);
}

/// A file from `quill-fntgen` (set `QUILL_REAL_FNT` to its path): it parses, its checksum holds, and every glyph of every strike reads.
#[test]
fn a_file_written_by_quill_fntgen_parses() {
	let Some(path) = std::env::var_os("QUILL_REAL_FNT") else {
		return;
	};
	let bytes = std::fs::read(path).unwrap();
	let st = parsed(&bytes).unwrap();
	assert!(st.verify(&mut SliceSource(&bytes)));
	let mut bits = [0u8; MAX_GLYPH_BYTES];
	let mut src = SliceSource(&bytes);
	let mut n = 0;
	for id in st.ids() {
		for ch in 0x20..=0xFFu8 {
			assert!(
				st.glyph(&mut src, id, ch, &mut bits).is_some(),
				"strike {} char {ch:#x}",
				id.index()
			);
			n += 1;
		}
	}
	eprintln!("{} strikes, {n} glyphs", st.strike_count());
	let mut m = StrikeMeasure::new(&st);
	let mut w = [0u16; GLYPHS];
	m.widths(&font("Times", 1, 24, 0), &mut w);
	eprintln!(
		"12 pt roman, 'n' is {} px, 'W' {} px",
		w[usize::from(b'n' - 0x20)],
		w[usize::from(b'W' - 0x20)]
	);
}
