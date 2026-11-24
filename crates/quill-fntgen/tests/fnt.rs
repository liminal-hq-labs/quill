// QUILL.FNT: reproducible bytes, a writer and reader that agree, a reader that refuses damage, the report and the proof sheets.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use quill_fntgen::faceset::{cp1252, DESK, QUILL_TEST, SETS};
use quill_fntgen::fnt::{self, FntError, Font, HEADER_LEN, MAX_DATA_LEN};
use quill_fntgen::render::{Opts, GLYPHS};
use quill_fntgen::{build, report, sheet, sizes, strikes};
use skrifa::{FontRef, MetadataProvider};

const HINTED_2: Opts = Opts {
	bpp: 2,
	hinted: true,
};
const HINTED_1: Opts = Opts {
	bpp: 1,
	hinted: true,
};

#[test]
fn building_twice_gives_the_same_bytes() {
	for set in SETS {
		for opts in [HINTED_1, HINTED_2] {
			assert_eq!(build(&set, opts), build(&set, opts), "{} {opts:?}", set.id);
		}
	}
}

#[test]
fn hinting_and_depth_change_the_bytes() {
	let a = build(&DESK, HINTED_2);
	assert_ne!(
		a,
		build(
			&DESK,
			Opts {
				hinted: false,
				..HINTED_2
			}
		)
	);
	assert_ne!(a, build(&DESK, HINTED_1));
}

#[test]
fn what_is_written_is_what_is_read() {
	for set in SETS {
		let s = strikes(&set, HINTED_2);
		let bytes = fnt::write(&s);
		let font = Font::parse(&bytes).expect("parses");
		assert_eq!(font.entries.len(), 72);
		for (i, strike) in s.iter().enumerate() {
			let e = &font.entries[i];
			assert_eq!(
				(e.face, e.style, e.half_points, e.px, e.bpp),
				(strike.face, strike.style, strike.half_points, strike.px, 2)
			);
			assert_eq!(
				(e.ascent, e.descent, e.leading),
				(strike.ascent, strike.descent, strike.leading)
			);
			assert!(e.data_len as usize <= MAX_DATA_LEN);
			for c in 0..GLYPHS {
				assert_eq!(font.advance(i, c), strike.advances[c]);
				assert_eq!(
					font.glyph(i, c),
					strike.glyphs[c],
					"{} strike {i} char {c}",
					set.id
				);
			}
		}
	}
}

#[test]
fn every_defined_character_has_a_glyph_in_every_face() {
	// 0x81, 0x8D, 0x8F, 0x90 and 0x9D are undefined in Windows-1252 and draw .notdef; 0x7F is a control.
	let undefined = [0x81u8, 0x8D, 0x8F, 0x90, 0x9D];
	for set in [DESK, QUILL_TEST] {
		for face in set.fonts {
			for file in face {
				let f = FontRef::new(file).expect("font");
				let map = f.charmap();
				for b in 0x20..=0xFFu8 {
					if b != 0x7F && !undefined.contains(&b) {
						assert!(map.map(cp1252(b)).is_some(), "{} {b:#x}", set.id);
					}
				}
			}
		}
	}
}

#[test]
fn a_hinted_strike_has_ink_where_it_should() {
	let font_bytes = build(&DESK, HINTED_2);
	let font = Font::parse(&font_bytes).unwrap();
	for s in 0..font.entries.len() {
		// 'H' is 0x48, space 0x20.
		assert_eq!(font.glyph(s, 0).w, 0, "a space has no bitmap");
		let h = font.glyph(s, 0x48 - 0x20);
		assert!(h.w > 0 && h.h > 0 && h.y > 0, "strike {s}");
		assert!(font.advance(s, 0x48 - 0x20) > 0);
	}
}

#[test]
fn the_reader_refuses_damaged_files() {
	let good = build(&QUILL_TEST, HINTED_1);
	assert!(Font::parse(&good).is_ok());
	assert_eq!(Font::parse(&[]).err(), Some(FntError::Truncated));
	assert_eq!(
		Font::parse(&good[..HEADER_LEN - 1]).err(),
		Some(FntError::Truncated)
	);
	for cut in [HEADER_LEN, 100, 5000, good.len() - 1] {
		assert!(Font::parse(&good[..cut]).is_err(), "cut at {cut}");
	}
	let mut bad = good.clone();
	bad[0] = b'X';
	assert_eq!(Font::parse(&bad).err(), Some(FntError::Magic));
	let mut bad = good.clone();
	bad[4] = 9;
	assert_eq!(Font::parse(&bad).err(), Some(FntError::Version));
	let mut bad = good.clone();
	let last = bad.len() - 1;
	bad[last] ^= 1;
	assert_eq!(Font::parse(&bad).err(), Some(FntError::Checksum));

	// Damage with a valid checksum, so the range checks are what refuse it, and nothing may panic afterwards.
	let mut state = 0x2545_F491u32;
	let mut next = move || {
		state ^= state << 13;
		state ^= state >> 17;
		state ^= state << 5;
		state
	};
	for _ in 0..300 {
		let mut bad = good.clone();
		let region =
			(HEADER_LEN + 72 * fnt::ENTRY_LEN + 72 * GLYPHS + 72 * fnt::BOXES_LEN).min(bad.len());
		for _ in 0..1 + next() % 3 {
			let at = HEADER_LEN + (next() as usize) % (region - HEADER_LEN);
			bad[at] = next() as u8;
		}
		let crc = fnt::crc32(&bad[HEADER_LEN..]);
		bad[20..24].copy_from_slice(&crc.to_le_bytes());
		if let Ok(font) = Font::parse(&bad) {
			for s in 0..font.entries.len() {
				for c in [0usize, 1, 40, 223] {
					let _ = font.advance(s, c);
					let _ = font.glyph(s, c);
				}
			}
		}
	}
}

#[test]
fn no_strike_is_over_one_store_chunk() {
	for set in SETS {
		for opts in [
			HINTED_1,
			HINTED_2,
			Opts {
				hinted: false,
				..HINTED_2
			},
		] {
			let bytes = build(&set, opts);
			let font = Font::parse(&bytes).unwrap();
			assert!(font
				.entries
				.iter()
				.all(|e| e.data_len as usize <= MAX_DATA_LEN));
		}
	}
}

#[test]
fn the_sheets_are_pngs() {
	let bytes = build(&DESK, HINTED_2);
	let font = Font::parse(&bytes).unwrap();
	for png in [
		sheet::size_sheet(&font, "t"),
		sheet::style_sheet(&font, "t"),
	] {
		assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
		assert!(png.len() > 10_000);
	}
}

#[test]
fn the_size_table_reports_the_budgets() {
	let bytes = build(&DESK, HINTED_2);
	let text = sizes::describe("x", &bytes);
	assert!(text.contains("strikes 72"));
	assert!(text.contains("budget 450 KB raw"));
	assert_eq!(sizes::sha256(b"abc").len(), 64);
}

fn compare(path: &str, actual: &str) {
	let file = report::root().join(path);
	if std::env::var_os("QUILL_UPDATE_GOLDEN").is_some() {
		std::fs::write(&file, actual).unwrap();
		return;
	}
	let want = std::fs::read_to_string(&file)
		.unwrap_or_else(|_| panic!("missing {path}; run with QUILL_UPDATE_GOLDEN=1"));
	assert_eq!(
		want, actual,
		"{path} differs; QUILL_UPDATE_GOLDEN=1 rewrites it and the diff is reviewed like code"
	);
}

#[test]
fn the_windows_me_comparison_report_is_current() {
	// A report, not a pass mark: a change to the strikes' advances shows as a change in the file.
	let mut text = String::new();
	for set in SETS {
		text.push_str(&report::windows_me_report(
			&set,
			&build(&set, HINTED_2),
			set.id,
		));
		text.push('\n');
	}
	compare("docs/media/dos-fnt-windows-me-report.txt", &text);
}
