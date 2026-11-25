// RTF code pages: `\ansicpg` and `\fcharset` decide what `\'hh` means, `\uN` reaches the whole Windows-1252 table, and damage never panics.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::PathBuf;

use proptest::prelude::*;
use quill_core::doc::Doc;
use quill_formats::io::SliceSource;
use quill_formats::rtf::{read, read_counted};
use quill_host::HostStore;
use quill_text::cp1252;
use quill_text::utf8::Refused;

fn root() -> PathBuf {
	PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn load(rtf: &[u8]) -> Result<(HostStore, Doc, Refused), quill_formats::FormatError> {
	let mut s = HostStore::new();
	let mut d = Doc::new();
	let n = read_counted(&mut d, &mut s, &mut SliceSource::new(rtf))?;
	Ok((s, d, n))
}

/// The body as the document holds it: Windows-1252 bytes, shown as the characters they are, paragraph marks as newlines.
fn body(s: &HostStore, d: &Doc) -> String {
	let mut b = vec![0u8; d.body.len() as usize];
	d.body.read(s, 0, &mut b);
	b.iter()
		.map(|&c| if c == 0x0D { '\n' } else { cp1252::to_char(c) })
		.collect()
}

#[test]
fn every_fixture_reads_to_its_expected_text_and_count() {
	let dir = root().join("tests/rtf");
	let mut seen = 0;
	for e in std::fs::read_dir(&dir).unwrap() {
		let path = e.unwrap().path();
		if path.extension().and_then(|x| x.to_str()) != Some("rtf") {
			continue;
		}
		let name = path.file_stem().unwrap().to_str().unwrap().to_string();
		let expect = std::fs::read_to_string(path.with_extension("expect")).unwrap();
		let (head, text) = expect.split_once('\n').unwrap();
		let count: usize = head.strip_prefix("refused=").unwrap().parse().unwrap();
		let (s, d, n) = load(&std::fs::read(&path).unwrap()).unwrap();
		assert_eq!(body(&s, &d), text, "{name}");
		assert_eq!(n.count, count, "{name}: refused");
		seen += 1;
	}
	assert!(seen >= 8, "found {seen} fixtures");
}

#[test]
fn every_windows_1252_character_round_trips_through_u() {
	for b in 0x20..=0xFFu8 {
		if b == 0x7F {
			continue;
		}
		let u = u32::from(cp1252::to_char(b)) as i32;
		let rtf = format!("{{\\rtf1\\ansi\\uc1\\u{u}?x\\par}}");
		let (s, d, n) = load(rtf.as_bytes()).unwrap();
		let mut got = vec![0u8; d.body.len() as usize];
		d.body.read(&s, 0, &mut got);
		assert_eq!(got, [b, b'x', 0x0D], "byte {b:#04x} (\\u{u})");
		assert_eq!(n.count, 0, "byte {b:#04x}");
	}
}

#[test]
fn the_27_characters_of_the_c1_block_were_only_nine_before() {
	let chars = "€‚ƒ„…†‡ˆ‰Š‹ŒŽ‘’“”•–—˜™š›œžŸ";
	assert_eq!(chars.chars().count(), 27);
	let mut rtf = String::from("{\\rtf1\\ansi ");
	for c in chars.chars() {
		rtf.push_str(&format!("\\u{}?", u32::from(c)));
	}
	rtf.push_str("\\par}");
	let (s, d, n) = load(rtf.as_bytes()).unwrap();
	assert_eq!(body(&s, &d), format!("{chars}\n"));
	assert_eq!(n.count, 0);
}

#[test]
fn a_code_page_character_1252_has_is_kept_and_one_it_lacks_is_counted() {
	// 0x8A is Š in 1250 and 1252 alike, and 0xE9 is é in both; 0xF8 is ř in 1250 and ø in 1252; 0xF1 is ń in 1250.
	let (s, d, n) = load(b"{\\rtf1\\ansi\\ansicpg1250\\deff0 \\'8a\\'e9\\'f8 \\'f1\\par}").unwrap();
	assert_eq!(body(&s, &d), "Šé? ?\n");
	assert_eq!(n.count, 2);
}

#[test]
fn a_font_with_no_charset_follows_the_document_and_one_with_a_charset_follows_itself() {
	let rtf = b"{\\rtf1\\ansi\\ansicpg1251\\deff0{\\fonttbl{\\f0 A;}{\\f1\\fcharset0 B;}{\\f2\\fcharset161 C;}}\
		\\f0 \\'c0\\f1 \\'c0\\f2 \\'c0\\par}";
	let (s, d, n) = load(rtf).unwrap();
	// 0xC0 is Cyrillic A in 1251, À in 1252 and a Greek iota with dialytika and tonos in 1253: only 1252 has a form.
	assert_eq!(body(&s, &d), "?À?\n");
	assert_eq!(n.count, 2);
}

#[test]
fn an_unnamed_charset_leaves_the_document_page_and_an_unknown_ansicpg_is_ignored() {
	let (s, d, n) =
		load(b"{\\rtf1\\ansi\\ansicpg1255\\deff0{\\fonttbl{\\f0\\fcharset177 A;}}\\'e9\\par}")
			.unwrap();
	assert_eq!(body(&s, &d), "é\n");
	assert_eq!(n.count, 0);
}

#[test]
fn undefined_bytes_keep_their_number_as_the_c1_control_does_in_1252() {
	let (s, d, n) = load(b"{\\rtf1\\ansi\\ansicpg1250\\deff0 \\'81\\'90\\par}").unwrap();
	let mut got = vec![0u8; d.body.len() as usize];
	d.body.read(&s, 0, &mut got);
	assert_eq!(got, [0x81, 0x90, 0x0D]);
	assert_eq!(n.count, 0);
}

#[test]
fn eight_bit_text_written_as_it_is_follows_the_code_page_too() {
	let mut rtf = b"{\\rtf1\\ansi\\ansicpg1251\\deff0 ".to_vec();
	rtf.extend([0xCF, 0xF0, 0xE8, 0xE2, 0xE5, 0xF2]);
	rtf.extend(b"\\par}");
	let (s, d, n) = load(&rtf).unwrap();
	assert_eq!(body(&s, &d), "??????\n");
	assert_eq!(n.count, 6);
}

#[test]
fn read_and_read_counted_make_the_same_document() {
	let rtf = std::fs::read(root().join("tests/rtf/cp1250.rtf")).unwrap();
	let (s, d, _) = load(&rtf).unwrap();
	let mut s2 = HostStore::new();
	let mut d2 = Doc::new();
	read(&mut d2, &mut s2, &mut SliceSource::new(&rtf)).unwrap();
	assert_eq!(body(&s, &d), body(&s2, &d2));
}

#[test]
fn damaged_escapes_cost_what_they_name_and_nothing_else() {
	// `\'4}` must not eat the brace: the text after the group is still read.
	let (s, d, _) = load(b"{\\rtf1\\ansi {a\\'4}b\\par}").unwrap();
	assert_eq!(body(&s, &d), "ab\n");
	// A lone high surrogate, a lone low one, a value out of range.
	let (s, d, n) = load(b"{\\rtf1\\ansi\\u-10179?a\\u-8704?b\\u70000?c\\par}").unwrap();
	assert_eq!(body(&s, &d), "?a?b?c\n");
	assert_eq!(n.count, 3);
}

fn token() -> impl Strategy<Value = Vec<u8>> {
	prop_oneof![
		Just(b"\\'4".to_vec()),
		Just(b"\\'".to_vec()),
		Just(b"\\'zz".to_vec()),
		Just(b"\\'e9".to_vec()),
		Just(b"\\'82\\'a0".to_vec()),
		Just(b"\\u".to_vec()),
		Just(b"\\u9".to_vec()),
		Just(b"\\u-".to_vec()),
		Just(b"\\u-32769?".to_vec()),
		Just(b"\\u65536?".to_vec()),
		Just(b"\\u2147483647?".to_vec()),
		Just(b"\\u-10179?".to_vec()),
		Just(b"\\u-8704?".to_vec()),
		Just(b"\\uc".to_vec()),
		Just(b"\\uc0".to_vec()),
		Just(b"\\uc2".to_vec()),
		Just(b"\\uc99999999999".to_vec()),
		Just(b"\\ansicpg".to_vec()),
		Just(b"\\ansicpg1251".to_vec()),
		Just(b"\\ansicpg932".to_vec()),
		Just(b"\\ansicpg-1".to_vec()),
		Just(b"\\f0".to_vec()),
		Just(b"\\f1".to_vec()),
		Just(b"\\f999".to_vec()),
		Just(b"\\fcharset204".to_vec()),
		Just(b"\\fcharset128".to_vec()),
		Just(b"\\fcharset".to_vec()),
		Just(b"{\\fonttbl{\\f0\\fcharset238 A;}{\\f1\\fcharset128 B;}}".to_vec()),
		Just(b"{".to_vec()),
		Just(b"}".to_vec()),
		Just(b"\\par".to_vec()),
		Just(b"\\*".to_vec()),
		Just(b"\\\\".to_vec()),
		Just(b" ".to_vec()),
		prop::collection::vec(any::<u8>(), 0..6),
	]
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(512))]

	#[test]
	fn damaged_escapes_never_panic(tokens in prop::collection::vec(token(), 0..40)) {
		let mut rtf = b"{\\rtf1\\ansi\\deff0 ".to_vec();
		for t in &tokens {
			rtf.extend(t);
		}
		if let Ok((s, d, n)) = load(&rtf) {
			// Every counted character is a `?` in the text, so the count cannot pass the length.
			prop_assert!(n.count <= d.body.len() as usize + 1);
			let mut b = vec![0u8; d.body.len() as usize];
			d.body.read(&s, 0, &mut b);
			prop_assert_eq!(b.last().copied(), Some(0x0D));
		}
	}

	#[test]
	fn any_bytes_after_the_header_never_panic(bytes in prop::collection::vec(any::<u8>(), 0..300)) {
		let mut rtf = b"{\\rtf1\\ansi\\ansicpg1251\\deff0{\\fonttbl{\\f0\\fcharset204 A;}} ".to_vec();
		rtf.extend(bytes);
		let _ = load(&rtf);
	}
}
