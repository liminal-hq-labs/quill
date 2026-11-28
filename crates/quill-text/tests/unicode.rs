// Tests for the Unicode tables: every table against the goldens `scripts/gen-unicode.py` computes from the UCD
// and Python's unicodedata, the grapheme and line-break rules against the UCD's own test files, the blob against
// the built-in tables for every code point, and lookups on every `u32` that never panic.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::BTreeMap;

use proptest::prelude::*;
use quill_text::blocks::{self, Block};
use quill_text::breaks::{self, Brk};
use quill_text::cluster::{self, Class as Cluster};
use quill_text::compose;
use quill_text::cp437;
use quill_text::fold;
use quill_text::script;
use quill_text::unitab::{self, Tables};
use quill_text::width;
use quill_text::word::{self, Class as Word};

const BLOB: &[u8] = include_bytes!("../../../assets/unicode/UNITAB.BIN");
const GOLDEN: &str = include_str!("data/unicode.tsv");
const GRAPHEME_TEST: &str = include_str!("ucd/GraphemeBreakTest.txt");
const LINE_BREAK_TEST: &str = include_str!("ucd/LineBreakTest.txt");
const LAST: u32 = 0x10_FFFF;

fn blob() -> Tables<'static> {
	unitab::tables(BLOB).expect("UNITAB.BIN is a valid blob")
}

/// The golden's runs of one range table: first, last and the value, which is `None` for a code point the table may answer anything for.
fn golden_runs(section: &str) -> Vec<(u32, u32, Option<u8>)> {
	let mut out = Vec::new();
	let mut inside = false;
	for line in GOLDEN.lines() {
		if line.starts_with('[') {
			inside = line == format!("[{section}]");
		} else if inside && !line.starts_with('#') {
			let mut fields = line.split('\t');
			let first = u32::from_str_radix(fields.next().unwrap(), 16).unwrap();
			let last = u32::from_str_radix(fields.next().unwrap(), 16).unwrap();
			let value = fields.next().unwrap();
			out.push((
				first,
				last,
				if value == "x" {
					None
				} else {
					Some(value.parse().unwrap())
				},
			));
		}
	}
	out
}

fn golden_rows(section: &str) -> Vec<Vec<u32>> {
	let mut out = Vec::new();
	let mut inside = false;
	for line in GOLDEN.lines() {
		if line.starts_with('[') {
			inside = line == format!("[{section}]");
		} else if inside && !line.starts_with('#') {
			out.push(
				line.split('\t')
					.map(|f| u32::from_str_radix(f, 16).unwrap())
					.collect(),
			);
		}
	}
	out
}

/// Checks a range table, built in (`through_char`, for every scalar value) and read from the blob (`through_blob`, for every `u32` up to U+10FFFF), against its golden.
fn check_ranges(
	section: &str,
	through_char: impl Fn(char) -> u8,
	through_blob: impl Fn(&Tables, u32) -> u8,
) {
	let runs = golden_runs(section);
	assert_eq!(
		runs.first().map(|r| r.0),
		Some(0),
		"{section}: the golden starts at U+0000"
	);
	assert_eq!(
		runs.last().map(|r| r.1),
		Some(LAST),
		"{section}: the golden ends at U+10FFFF"
	);
	let tables = blob();
	let mut seen = 0u32;
	for (first, last, value) in runs {
		assert_eq!(
			first, seen,
			"{section}: the golden has a gap before U+{first:04X}"
		);
		seen = last + 1;
		let Some(value) = value else { continue };
		for cp in first..=last {
			assert_eq!(
				through_blob(&tables, cp),
				value,
				"{section}: the blob at U+{cp:04X}"
			);
			if let Some(c) = char::from_u32(cp) {
				assert_eq!(
					through_char(c),
					value,
					"{section}: the built-in table at U+{cp:04X}"
				);
			}
		}
	}
}

#[test]
fn cluster_table_matches_the_golden() {
	check_ranges(
		"cluster",
		|c| cluster::class(c) as u8,
		|t, cp| t.cluster_class(cp) as u8,
	);
}

#[test]
fn break_classes_match_the_golden() {
	check_ranges("breaks", |c| breaks::brk(c) as u8, |t, cp| t.brk(cp) as u8);
}

#[test]
fn word_classes_match_the_golden() {
	check_ranges(
		"word",
		|c| word::class(c) as u8,
		|t, cp| t.word_class(cp) as u8,
	);
}

#[test]
fn width_matches_the_golden() {
	check_ranges(
		"width",
		|c| u8::from(width::is_wide(c)),
		|t, cp| u8::from(t.is_wide(cp)),
	);
}

#[test]
fn script_flags_match_the_golden() {
	check_ranges(
		"script",
		|c| u8::from(script::needs_shaping_or_bidi(c)) | (u8::from(script::is_invisible(c)) << 1),
		|t, cp| u8::from(t.needs_shaping_or_bidi(cp)) | (u8::from(t.is_invisible(cp)) << 1),
	);
}

#[test]
fn blocks_match_the_golden() {
	check_ranges("blocks", |c| blocks::block(c).0, |t, cp| t.block(cp).0);
}

#[test]
fn fold_matches_python_casefold_for_every_code_point() {
	let golden: BTreeMap<u32, u32> = golden_rows("fold")
		.iter()
		.map(|row| (row[0], row[1]))
		.collect();
	assert!(
		golden.len() > 1000,
		"the golden holds the foldings of the scripts in scope"
	);
	let tables = blob();
	for cp in 0..=LAST {
		let expected = golden.get(&cp).copied().unwrap_or(cp);
		assert_eq!(tables.fold(cp), expected, "the blob at U+{cp:04X}");
		if let Some(c) = char::from_u32(cp) {
			assert_eq!(
				u32::from(fold::fold(c)),
				expected,
				"the built-in table at U+{cp:04X}"
			);
		}
	}
}

#[test]
fn folding_is_idempotent() {
	for cp in 0..=LAST {
		if let Some(c) = char::from_u32(cp) {
			assert_eq!(fold::fold(fold::fold(c)), fold::fold(c), "U+{cp:04X}");
		}
	}
}

#[test]
fn compose_matches_python_nfc() {
	let golden = golden_rows("compose");
	assert_eq!(
		golden.len(),
		58,
		"the Windows-1252 letters that are a base and one mark"
	);
	let tables = blob();
	let mut expected = BTreeMap::new();
	for row in &golden {
		expected.insert((row[0], row[1]), row[2]);
	}
	for base in 0..0x200u32 {
		for mark in 0x2FF..0x370u32 {
			let want = expected.get(&(base, mark)).copied();
			assert_eq!(
				tables.compose(base, mark).map(u32::from),
				want,
				"the blob at {base:04X} {mark:04X}"
			);
			if let (Some(b), Some(m)) = (char::from_u32(base), char::from_u32(mark)) {
				assert_eq!(
					compose::compose(b, m).map(u32::from),
					want,
					"the built-in table at {base:04X} {mark:04X}"
				);
			}
		}
	}
	// Every letter of the table decomposes back, and nothing else does.
	for row in &golden {
		let letter = char::from_u32(row[2]).unwrap();
		let (base, mark) = compose::decompose(letter).unwrap();
		assert_eq!((u32::from(base), u32::from(mark)), (row[0], row[1]));
		assert_eq!(tables.decompose(row[2]), Some((base, mark)));
		assert!(cp1252_has(letter), "{letter} is a Windows-1252 letter");
	}
	let decomposable = (0..=0x2FFu32)
		.filter_map(char::from_u32)
		.filter(|c| compose::decompose(*c).is_some())
		.count();
	assert_eq!(decomposable, 58);
}

fn cp1252_has(c: char) -> bool {
	(0..=255u8).any(|b| quill_text::cp1252::to_char(b) == c)
}

#[test]
fn cp437_from_unicode_inverts_to_char() {
	let mut found = 0;
	for byte in 0..=255u8 {
		let c = cp437::to_char(byte);
		// The codes below 0x20 (but the bullet) and 0x7F show as spaces; a space is 0x20.
		let expected = if (0x20..=0x7E).contains(&byte) || byte >= 0x80 || byte == 0x07 {
			Some(byte)
		} else {
			None
		};
		if let Some(code) = expected {
			assert_eq!(
				cp437::from_unicode(c),
				Some(code),
				"byte {byte:#04x} shows as {c}"
			);
			found += 1;
		}
	}
	assert_eq!(found, 95 + 128 + 1);
	for c in ['\u{0}', '\u{7F}', 'Ж', '😀', '\u{3000}'] {
		assert_eq!(cp437::from_unicode(c), None, "{c:?}");
	}
	// And only those characters have a code.
	let with_code = (0..=0x2FFFu32)
		.filter_map(char::from_u32)
		.filter(|c| cp437::from_unicode(*c).is_some())
		.count();
	assert_eq!(with_code, found);
}

#[test]
fn blob_and_built_in_tables_agree_on_names() {
	let tables = blob();
	for id in 0..=255u8 {
		assert_eq!(
			tables.block_name(Block(id)),
			blocks::name(Block(id)),
			"block {id}"
		);
	}
	assert_eq!(blocks::name(Block::OTHER), "Other");
	let mut short = 0;
	for cp in 0..=0x1_0000u32 {
		let from_blob = tables.short_name(cp).map(|s| s.as_str().to_string());
		let built_in = char::from_u32(cp)
			.and_then(blocks::short_name)
			.map(|s| s.as_str().to_string());
		assert_eq!(from_blob, built_in, "U+{cp:04X}");
		short += usize::from(built_in.is_some());
	}
	assert_eq!(short, 12);
	for (c, name) in [
		('\u{200D}', "ZWJ"),
		('\u{200B}', "ZWSP"),
		('\u{2060}', "WJ"),
		('\u{200E}', "LRM"),
		('\u{200F}', "RLM"),
		('\u{FE0F}', "VS16"),
		('\u{FEFF}', "BOM"),
	] {
		assert_eq!(
			blocks::short_name(c).map(|s| s.as_str().to_string()),
			Some(name.to_string())
		);
	}
	assert!(blocks::short_name('a').is_none());
}

// ----- the rules, against the UCD's own test files -----

/// The lines of a trimmed test file: its code points and the breaks between them, and the count in its header.
/// A test line: its characters and whether a break is allowed between each pair.
type TestLine = (Vec<char>, Vec<bool>);

fn test_lines(text: &str) -> (Vec<TestLine>, usize, Vec<String>) {
	let mut lines = Vec::new();
	let mut total = 0;
	let mut excluded = Vec::new();
	for line in text.lines() {
		if let Some(rest) = line.strip_prefix("# Trimmed by scripts/gen-unicode.py to the ") {
			let mut words = rest.split_whitespace();
			assert_eq!(
				words.next().map(str::parse::<usize>),
				Some(Ok(text.lines().filter(|l| !l.starts_with('#')).count())),
				"the header counts the lines"
			);
			total = words.nth(1).unwrap().parse().unwrap();
		} else if line.starts_with("# excluded ") {
			excluded.push(line.trim_start_matches("# excluded ").to_string());
		} else if !line.starts_with('#') && !line.is_empty() {
			let mut chars = Vec::new();
			let mut breaks = Vec::new();
			for field in line.split_whitespace() {
				match field {
					"\u{d7}" => breaks.push(false),
					"\u{f7}" => breaks.push(true),
					hex => {
						chars.push(char::from_u32(u32::from_str_radix(hex, 16).unwrap()).unwrap())
					}
				}
			}
			// The break before the first character and after the last are always the line's own ends.
			breaks.remove(0);
			breaks.pop();
			lines.push((chars, breaks));
		}
	}
	(lines, total, excluded)
}

#[test]
fn clusters_agree_with_grapheme_break_test() {
	let (lines, total, excluded) = test_lines(GRAPHEME_TEST);
	eprintln!(
		"GraphemeBreakTest.txt: {} of {total} lines checked; excluded:",
		lines.len()
	);
	for reason in &excluded {
		eprintln!("  {reason}");
	}
	assert!(lines.len() > 150, "enough lines stay in scope");
	for (chars, expected) in lines {
		let mut regional = 0;
		let mut actual = Vec::new();
		for pair in chars.windows(2) {
			regional = if cluster::is_regional(pair[0]) {
				regional + 1
			} else {
				0
			};
			actual.push(!cluster::joins(pair[0], pair[1], regional % 2 == 1));
		}
		assert_eq!(actual, expected, "{chars:?}");
	}
}

#[test]
fn breaks_agree_with_line_break_test() {
	let (lines, total, excluded) = test_lines(LINE_BREAK_TEST);
	eprintln!(
		"LineBreakTest.txt: {} of {total} lines checked; excluded:",
		lines.len()
	);
	for reason in &excluded {
		eprintln!("  {reason}");
	}
	assert!(lines.len() > 300, "enough lines stay in scope");
	for (chars, expected) in lines {
		let actual: Vec<bool> = chars
			.windows(2)
			.map(|pair| breaks::can_break(pair[0], pair[1]))
			.collect();
		assert_eq!(actual, expected, "{chars:?}");
	}
}

// ----- the cases the plan names -----

#[test]
fn clusters_follow_the_plan() {
	let joins = |a: char, b: char| cluster::joins(a, b, false);
	assert!(joins('e', '\u{301}'), "a combining mark joins its base");
	assert!(
		joins('e', '\u{301}') && joins('\u{301}', '\u{302}'),
		"marks stack"
	);
	assert!(
		!joins('\r', '\u{301}') && !joins('a', '\n'),
		"controls are never joined"
	);
	assert!(
		joins('\u{1F468}', '\u{200D}') && joins('\u{200D}', '\u{1F469}'),
		"a ZWJ sequence"
	);
	assert!(joins('\u{1F44B}', '\u{1F3FD}'), "a skin tone");
	assert!(joins('\u{2764}', '\u{FE0F}'), "VS16");
	assert!(
		joins('1', '\u{FE0F}') && joins('\u{FE0F}', '\u{20E3}'),
		"a keycap"
	);
	assert!(joins('\u{1F3F4}', '\u{E0067}'), "a tag");
	assert!(
		cluster::joins('\u{1F1E8}', '\u{1F1E6}', true),
		"the second regional indicator joins"
	);
	assert!(
		!cluster::joins('\u{1F1E6}', '\u{1F1E8}', false),
		"the third starts a new flag"
	);
	assert!(!joins('a', 'b'));
	assert!(
		cluster::is_extend('\u{200C}')
			&& cluster::is_pictographic('\u{1F600}')
			&& cluster::is_regional('\u{1F1FF}')
	);
}

#[test]
fn line_breaks_follow_the_plan() {
	let space = Brk::Space;
	assert!(breaks::break_between(space, Brk::Other), "after a space");
	assert!(!breaks::break_between(Brk::Other, space), "not before one");
	assert!(
		breaks::can_break('-', 'a')
			&& breaks::can_break('\u{2014}', 'a')
			&& breaks::can_break('\u{200B}', 'a'),
		"after a hyphen, an em dash and ZWSP"
	);
	assert!(!breaks::can_break('a', '-'), "not before a hyphen");
	assert!(
		!breaks::can_break('a', '\u{A0}')
			&& !breaks::can_break('\u{A0}', 'a')
			&& !breaks::can_break('\u{202F}', 'a')
			&& !breaks::can_break('\u{2007}', 'a')
	);
	assert!(
		!breaks::can_break('\u{2060}', 'a') && !breaks::can_break('\u{FEFF}', 'a'),
		"not after a word joiner or BOM"
	);
	assert!(
		breaks::can_break('中', '文')
			&& breaks::can_break('a', '中')
			&& breaks::can_break('中', 'a'),
		"around ideographs"
	);
	assert!(
		breaks::can_break('あ', 'い') && breaks::can_break('한', '글'),
		"kana and Hangul syllables"
	);
	for closing in [
		'、', '。', '，', '．', '・', '：', '；', '？', '！', '）', '」', '』', '】', '〕', '〉',
		'》',
	] {
		assert!(
			!breaks::can_break('中', closing),
			"no break before {closing}"
		);
	}
	for opening in ['（', '「', '『', '【', '〔', '〈', '《'] {
		assert!(
			!breaks::can_break(opening, '中'),
			"no break after {opening}"
		);
	}
	assert!(!breaks::can_break('a', 'b'), "no break inside a word");
	for before in [
		Brk::Other,
		Brk::Space,
		Brk::After,
		Brk::Ideo,
		Brk::Close,
		Brk::Open,
		Brk::Glue,
	] {
		assert!(!breaks::break_between(before, Brk::Close));
	}
}

#[test]
fn words_follow_the_plan() {
	for c in [
		'a', 'Z', '9', 'é', 'ж', 'ß', '\u{301}', 'א', 'ก', '\u{200D}',
	] {
		assert_eq!(word::class(c), Word::Letter, "{c:?}");
	}
	for c in [',', '.', '-', '$', '+', '«', '\u{2014}', '\u{0}', '\u{7F}'] {
		assert_eq!(word::class(c), Word::Punct, "{c:?}");
	}
	for c in [' ', '\t', '\n', '\u{A0}', '\u{3000}', '\u{2028}'] {
		assert_eq!(word::class(c), Word::Space, "{c:?}");
	}
	for c in ['中', 'あ', 'ア', '한', 'ー', '\u{20000}'] {
		assert_eq!(word::class(c), Word::Ideo, "{c:?}");
	}
}

#[test]
fn folding_follows_the_plan() {
	for (upper, lower) in [
		('A', 'a'),
		('Z', 'z'),
		('À', 'à'),
		('Ÿ', 'ÿ'),
		('Σ', 'σ'),
		('ς', 'σ'),
		('Ж', 'ж'),
		('Ա', 'ա'),
		('Ⴀ', 'ⴀ'),
		('Ა', 'ა'),
		('Ａ', 'ａ'),
		('K', 'k'),
		('ẞ', 'ß'),
		('ǅ', 'ǆ'),
		('µ', 'μ'),
	] {
		assert_eq!(fold::fold(upper), lower, "{upper}");
	}
	assert_eq!(
		fold::fold('ꭰ'),
		'Ꭰ',
		"Cherokee folds to the capital letters"
	);
	for same in ['a', 'ß', 'İ', 'ı', '1', '中', 'ж'] {
		assert_eq!(fold::fold(same), same, "{same}");
	}
}

#[test]
fn composition_follows_the_plan() {
	assert_eq!(compose::compose('e', '\u{301}'), Some('é'));
	assert_eq!(compose::compose('E', '\u{308}'), Some('Ë'));
	assert_eq!(compose::compose('c', '\u{327}'), Some('ç'));
	assert_eq!(compose::compose('a', '\u{30A}'), Some('å'));
	assert_eq!(compose::compose('Z', '\u{30C}'), Some('Ž'));
	assert_eq!(compose::compose('Y', '\u{308}'), Some('Ÿ'));
	assert_eq!(compose::compose('n', '\u{303}'), Some('ñ'));
	assert_eq!(compose::compose('x', '\u{301}'), None);
	assert_eq!(
		compose::compose('a', '\u{328}'),
		None,
		"ą is not in Windows-1252"
	);
	assert_eq!(compose::decompose('é'), Some(('e', '\u{301}')));
	assert_eq!(compose::decompose('a'), None);
}

#[test]
fn width_script_and_blocks_follow_the_plan() {
	assert!(
		width::is_wide('中')
			&& width::is_wide('Ａ')
			&& width::is_wide('😀')
			&& width::is_wide('한')
	);
	assert!(!width::is_wide('a') && !width::is_wide('é') && !width::is_wide('ｱ'));
	for c in [
		'א', 'ا', 'ܐ', 'ހ', 'अ', 'ก', 'ກ', 'ཀ', 'က', 'ក', 'ᠠ', '\u{200E}', '\u{202B}', '\u{2067}',
		'\u{FB50}', '\u{FE70}',
	] {
		assert!(script::needs_shaping_or_bidi(c), "{c:?}");
	}
	for c in ['a', 'é', 'ж', '中', 'あ', '한', '😀', '\u{FB01}'] {
		assert!(!script::needs_shaping_or_bidi(c), "{c:?}");
	}
	for c in [
		'\u{200B}',
		'\u{200C}',
		'\u{200D}',
		'\u{200E}',
		'\u{2060}',
		'\u{FEFF}',
		'\u{FE0F}',
		'\u{00AD}',
		'\u{E0020}',
	] {
		assert!(script::is_invisible(c), "{c:?}");
	}
	assert!(
		!script::is_invisible('a')
			&& !script::is_invisible('\u{A0}')
			&& !script::is_invisible('\u{301}')
	);
	assert_eq!(blocks::name(blocks::block('ж')), "Cyrillic");
	assert_eq!(blocks::name(blocks::block('é')), "Latin-1 Supplement");
	assert_eq!(blocks::name(blocks::block('中')), "CJK Ideographs");
	assert_eq!(blocks::name(blocks::block('😀')), "Emoticons");
	assert_eq!(
		blocks::name(blocks::block('a')),
		"Other",
		"Basic Latin is not named"
	);
	assert_eq!(blocks::name(blocks::block('\u{10FFFF}')), "Other");
}

// ----- the boundaries, and no panic on any value -----

#[test]
fn the_blob_is_checked_once_and_refuses_what_is_not_one() {
	assert!(unitab::tables(BLOB).is_some());
	for length in 0..BLOB.len() {
		assert!(unitab::tables(&BLOB[..length]).is_none(), "{length} bytes");
	}
	let mut wrong = BLOB.to_vec();
	wrong[0] = b'X';
	assert!(unitab::tables(&wrong).is_none(), "magic");
	let mut wrong = BLOB.to_vec();
	wrong[4] = 2;
	assert!(unitab::tables(&wrong).is_none(), "version");
	// A table whose first entry does not start at U+0000, or whose starts go down.
	for table in 0..10 {
		let offset = usize::from(u16::from_le_bytes([
			BLOB[8 + 4 * table],
			BLOB[9 + 4 * table],
		]));
		let mut wrong = BLOB.to_vec();
		wrong[offset + 1] ^= 0x80;
		wrong[offset + 4] ^= 0xFF;
		wrong[offset + 5] ^= 0xFF;
		// Whatever the corruption, the check answers without panicking and the lookups stay inside the blob.
		if let Some(tables) = unitab::tables(&wrong) {
			for cp in [0, 0x41, 0x300, 0x4E00, 0x1F600, LAST, LAST + 1, u32::MAX] {
				let _ = (
					tables.cluster_class(cp),
					tables.brk(cp),
					tables.word_class(cp),
					tables.fold(cp),
					tables.is_wide(cp),
					tables.block(cp),
				);
				let _ = (
					tables.compose(cp, 0x301),
					tables.decompose(cp),
					tables.short_name(cp),
					tables.block_name(Block(cp as u8)),
				);
			}
		}
	}
}

#[test]
fn every_u32_has_an_answer() {
	let tables = blob();
	let probe = |cp: u32| {
		let _ = (
			tables.cluster_class(cp),
			tables.brk(cp),
			tables.word_class(cp),
			tables.fold(cp),
			tables.is_wide(cp),
		);
		let _ = (
			tables.needs_shaping_or_bidi(cp),
			tables.is_invisible(cp),
			tables.block(cp),
			tables.compose(cp, cp),
			tables.decompose(cp),
		);
		let _ = tables.short_name(cp);
	};
	for cp in (0..=u32::MAX).step_by(0x1_0001) {
		probe(cp);
	}
	for cp in [
		LAST - 1,
		LAST,
		LAST + 1,
		0xD7FF,
		0xD800,
		0xDFFF,
		0xE000,
		0xFFFF,
		0x1_0000,
		u32::MAX - 1,
		u32::MAX,
	] {
		probe(cp);
	}
	// Beyond Unicode every table answers its default.
	for cp in [LAST + 1, 0x20_0000, 0x8000_0000, u32::MAX] {
		assert_eq!(tables.cluster_class(cp), Cluster::Other);
		assert_eq!(tables.brk(cp), Brk::Other);
		assert_eq!(tables.word_class(cp), Word::Punct);
		assert_eq!(tables.fold(cp), cp);
		assert!(
			!tables.is_wide(cp) && !tables.needs_shaping_or_bidi(cp) && !tables.is_invisible(cp)
		);
		assert_eq!(tables.block(cp), Block::OTHER);
	}
	// The built-in lookups take a `char`, which is every scalar value; surrogates are not one.
	for cp in 0..=LAST {
		if let Some(c) = char::from_u32(cp) {
			let _ = (
				cluster::class(c),
				breaks::brk(c),
				word::class(c),
				fold::fold(c),
				width::is_wide(c),
				script::is_invisible(c),
				blocks::block(c),
			);
		}
	}
}

proptest! {
	#[test]
	fn lookups_never_panic_on_any_u32(cp in any::<u32>(), other in any::<u32>()) {
		let tables = blob();
		let _ = (tables.cluster_class(cp), tables.brk(cp), tables.word_class(cp), tables.is_wide(cp), tables.block(cp));
		let _ = (tables.compose(cp, other), tables.decompose(cp), tables.short_name(cp), tables.needs_shaping_or_bidi(cp));
		if cp > LAST {
			prop_assert_eq!(tables.fold(cp), cp);
		}
	}

	#[test]
	fn garbage_is_not_a_blob(bytes in proptest::collection::vec(any::<u8>(), 0..200)) {
		let _ = unitab::tables(&bytes);
	}

	#[test]
	fn folding_a_char_gives_a_char_that_folds_to_itself(c in any::<char>()) {
		prop_assert_eq!(fold::fold(fold::fold(c)), fold::fold(c));
	}

	#[test]
	fn no_break_before_a_space_or_inside_a_run_of_letters(a in any::<char>(), b in any::<char>()) {
		prop_assert!(!breaks::can_break(a, ' '));
		if word::class(a) == Word::Letter && word::class(b) == Word::Letter && breaks::brk(a) == Brk::Other && breaks::brk(b) == Brk::Other {
			prop_assert!(!breaks::can_break(a, b));
		}
	}

	#[test]
	fn only_extenders_and_pictographs_join(a in any::<char>(), b in any::<char>(), odd in any::<bool>()) {
		if cluster::joins(a, b, odd) {
			prop_assert!(matches!(cluster::class(b), Cluster::Extend | Cluster::Zwj | Cluster::Pictographic | Cluster::Regional));
			prop_assert!(cluster::class(a) != Cluster::Control && cluster::class(b) != Cluster::Control);
		}
	}

	#[test]
	fn composing_then_decomposing_returns_the_pair(base in 0u8..0x80, mark in 0x300u32..0x370) {
		if let Some(letter) = compose::compose(char::from(base), char::from_u32(mark).unwrap()) {
			prop_assert_eq!(compose::decompose(letter), Some((char::from(base), char::from_u32(mark).unwrap())));
		}
	}
}
