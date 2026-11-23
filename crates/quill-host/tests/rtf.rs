// RTF tests: the corpus reads, writing is stable, and damaged input never panics.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::PathBuf;

use proptest::prelude::*;
use quill_core::doc::Doc;
use quill_formats::io::SliceSource;
use quill_host::docs::VecSink;
use quill_host::HostStore;

fn root() -> PathBuf {
	PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(s: &mut HostStore, bytes: &[u8]) -> Result<Doc, quill_formats::FormatError> {
	let mut d = Doc::new();
	quill_formats::rtf::read(&mut d, s, &mut SliceSource::new(bytes))?;
	Ok(d)
}

fn write(s: &HostStore, d: &Doc) -> Vec<u8> {
	let mut out = VecSink(Vec::new());
	quill_formats::rtf::write(d, s, &mut out).unwrap();
	out.0
}

fn text_of(s: &HostStore, d: &Doc) -> String {
	let mut b = vec![0u8; d.body.len() as usize];
	d.body.read(s, 0, &mut b);
	b.iter().map(|c| char::from(*c)).collect()
}

#[test]
fn the_letter_reads_with_its_text_and_formats() {
	let mut s = HostStore::new();
	let d = read(
		&mut s,
		&std::fs::read(root().join("tests/docs/letter.rtf")).unwrap(),
	)
	.unwrap();
	let t = text_of(&s, &d);
	assert!(t.starts_with("Quill Letter\r"), "{t:?}");
	assert!(t.contains("Dear reader,\r"));
	assert!(t.contains("Item\tQty\tPrice\r"));
	assert!(t.contains("\u{93}curly quotes\u{94} and an em dash\u{97}here."));
	assert!(t.ends_with("Yours truly,\u{b}The Quill team\r"));
	assert_eq!(d.fonts.len(), 3);
	// Bold appears: some character format has the bold flag.
	let bold = (0..d.cfmts.len())
		.filter_map(|i| d.cfmts.get(&s, i as u16))
		.any(|c| c.flags & 1 != 0);
	assert!(bold);
	// The title paragraph is centred and the last is right aligned.
	let first_mark = t.find('\r').unwrap() as u32;
	let pf = d
		.pfmts
		.get(&s, d.body.pfmt_at(&s, first_mark).unwrap())
		.unwrap();
	assert_eq!(pf.align, 1);
	let last = d
		.pfmts
		.get(&s, d.body.pfmt_at(&s, d.body.len() - 1).unwrap())
		.unwrap();
	assert_eq!(last.align, 2);
}

#[test]
fn writing_then_reading_then_writing_is_stable() {
	let mut s = HostStore::new();
	let d = read(
		&mut s,
		&std::fs::read(root().join("tests/docs/letter.rtf")).unwrap(),
	)
	.unwrap();
	let first = write(&s, &d);
	let d2 = read(&mut s, &first).unwrap();
	assert_eq!(text_of(&s, &d), text_of(&s, &d2));
	let second = write(&s, &d2);
	assert_eq!(
		String::from_utf8_lossy(&first),
		String::from_utf8_lossy(&second)
	);
}

#[test]
fn tolerates_unknown_destinations_and_words() {
	let mut s = HostStore::new();
	let rtf = br"{\rtf1\ansi{\fonttbl{\f0 Arial;}}{\*\unknown \foo123 hidden}{\info{\title T}}\pard Visible \xyzzy5 text\par}";
	let d = read(&mut s, rtf).unwrap();
	assert_eq!(text_of(&s, &d), "Visible text\r");
}

#[test]
fn refuses_what_is_not_rtf_and_overlong_words() {
	let mut s = HostStore::new();
	assert!(read(&mut s, b"hello").is_err());
	let mut long = b"{\\rtf1\\".to_vec();
	long.extend(std::iter::repeat_n(b'a', 40));
	long.extend_from_slice(b" x}");
	assert!(read(&mut s, &long).is_err());
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(300))]

	#[test]
	fn damaged_rtf_never_panics_or_leaks(flips in proptest::collection::vec((any::<u16>(), any::<u8>()), 1..12), cut in any::<u16>()) {
		let mut s = HostStore::new();
		let mut bytes = std::fs::read(root().join("tests/docs/letter.rtf")).unwrap();
		for (i, v) in flips {
			let n = bytes.len();
			bytes[usize::from(i) % n] = v;
		}
		let keep = usize::from(cut) % (bytes.len() + 1);
		bytes.truncate(keep.max(8));
		let mut d = Doc::new();
		let _ = quill_formats::rtf::read(&mut d, &mut s, &mut SliceSource::new(&bytes));
		d.reset(&mut s);
		prop_assert_eq!(s.live_chunks(), 0);
	}

	#[test]
	fn random_text_survives_the_round_trip(text in "[ -~\u{a0}-\u{ff}]{0,200}") {
		let mut s = HostStore::new();
		let mut d = Doc::new();
		d.init(&mut s).unwrap();
		let mut body: Vec<u8> = text.chars().map(|c| c as u32 as u8).collect();
		body.push(b'\r');
		d.body.load(&mut s, &body, 0, 0).unwrap();
		let rtf = write(&s, &d);
		let d2 = read(&mut s, &rtf).unwrap();
		prop_assert_eq!(text_of(&s, &d), text_of(&s, &d2));
	}
}

#[test]
fn lists_round_trip_in_the_word_6_form() {
	let mut s = HostStore::new();
	let d = read(
		&mut s,
		&std::fs::read(root().join("tests/docs/lists.rtf")).unwrap(),
	)
	.unwrap();
	let first = write(&s, &d);
	let text = String::from_utf8_lossy(&first).to_string();
	assert!(text.contains("\\pnlvlblt"), "{text}");
	assert!(text.contains("\\pnlvlbody\\pndec\\pnstart1"));
	let d2 = read(&mut s, &first).unwrap();
	assert_eq!(String::from_utf8_lossy(&write(&s, &d2)), text);
	assert_eq!(text_of(&s, &d2), text_of(&s, &d));
}

#[test]
fn headers_footers_and_fields_round_trip() {
	use quill_core::doc::StoryKind;
	let mut s = HostStore::new();
	let d = read(
		&mut s,
		&std::fs::read(root().join("tests/docs/headers.rtf")).unwrap(),
	)
	.unwrap();
	let story = |d: &Doc, s: &HostStore, k: StoryKind| {
		let t = d.story(k);
		let mut b = vec![0u8; t.len() as usize];
		t.read(s, 0, &mut b);
		b
	};
	assert_eq!(story(&d, &s, StoryKind::Header), b"Page \x01\r");
	assert_eq!(story(&d, &s, StoryKind::Footer), b"Quill test document\r");
	assert_eq!(d.header.object_at(&s, 5), Some(1 << 16));
	assert_eq!(d.page.flags & 1, 1);
	let first = write(&s, &d);
	let d2 = read(&mut s, &first).unwrap();
	assert_eq!(story(&d2, &s, StoryKind::Header), b"Page \x01\r");
	assert_eq!(
		String::from_utf8_lossy(&write(&s, &d2)),
		String::from_utf8_lossy(&first)
	);
	// The native format keeps them too.
	let mut out = VecSink(Vec::new());
	quill_formats::qld::write(&d, &s, &mut out).unwrap();
	let mut d3 = Doc::new();
	quill_formats::qld::read(&mut d3, &mut s, &mut SliceSource::new(&out.0)).unwrap();
	assert_eq!(story(&d3, &s, StoryKind::Footer), b"Quill test document\r");
	assert_eq!(d3.header.object_at(&s, 5), Some(1 << 16));
}
