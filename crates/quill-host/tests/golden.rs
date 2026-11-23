// Golden tests: the committed documents, their layout dumps and the .QLD round trip.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::PathBuf;

use proptest::prelude::*;
use quill_core::doc::{Doc, StoryKind};
use quill_core::layout::layout_story;
use quill_formats::io::SliceSource;
use quill_host::docs::{self, VecSink};
use quill_host::render::text_dump;
use quill_host::{GridMeasure, HostStore};

fn root() -> PathBuf {
	PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn compare(path: &str, actual: &str) {
	let file = root().join(path);
	if std::env::var_os("QUILL_UPDATE_GOLDEN").is_some() {
		std::fs::write(&file, actual).unwrap();
		return;
	}
	let want = std::fs::read_to_string(&file)
		.unwrap_or_else(|_| panic!("missing golden {path}; run with QUILL_UPDATE_GOLDEN=1"));
	assert_eq!(
		want, actual,
		"{path} differs; QUILL_UPDATE_GOLDEN=1 rewrites it and the diff is reviewed like code"
	);
}

fn read_doc(s: &mut HostStore, bytes: &[u8]) -> Doc {
	let mut d = Doc::new();
	quill_formats::qld::read(&mut d, s, &mut SliceSource::new(bytes)).unwrap();
	d
}

#[test]
fn plain_qld_is_what_the_builder_writes() {
	let mut s = HostStore::new();
	let built = docs::plain_bytes(&mut s).unwrap();
	let committed = std::fs::read(root().join("tests/docs/plain.qld")).unwrap();
	assert_eq!(
		built, committed,
		"tests/docs/plain.qld is stale; run: cargo run -p quill-host --bin make-docs -- tests/docs"
	);
}

#[test]
fn plain_layout_matches_the_grid_golden() {
	let mut s = HostStore::new();
	let bytes = std::fs::read(root().join("tests/docs/plain.qld")).unwrap();
	let d = read_doc(&mut s, &bytes);
	let dump = text_dump(&d, &s, &mut GridMeasure, "grid", StoryKind::Body);
	compare("tests/golden/layout/plain.grid.txt", &dump);
}

#[test]
fn plain_page_map_matches_the_png_golden() {
	let mut s = HostStore::new();
	let bytes = std::fs::read(root().join("tests/docs/plain.qld")).unwrap();
	let d = read_doc(&mut s, &bytes);
	let png = quill_host::render::page_map_png(&d, &s, &mut GridMeasure, StoryKind::Body, 1, 3, 6);
	let file = root().join("tests/golden/pages/plain.grid.png");
	if std::env::var_os("QUILL_UPDATE_GOLDEN").is_some() {
		std::fs::write(&file, &png).unwrap();
		return;
	}
	assert_eq!(
		std::fs::read(&file).unwrap(),
		png,
		"tests/golden/pages/plain.grid.png differs; QUILL_UPDATE_GOLDEN=1 rewrites it"
	);
}

#[test]
fn hello_screen_matches_its_golden() {
	let mut text = String::new();
	quill_grid::hello::<25>().dump(1, &mut text).unwrap();
	compare("tests/golden/screens/hello.80x25.txt", &text);
}

#[test]
fn qld_round_trip_is_byte_stable() {
	let mut s = HostStore::new();
	let bytes = std::fs::read(root().join("tests/docs/plain.qld")).unwrap();
	let d = read_doc(&mut s, &bytes);
	let mut out = VecSink(Vec::new());
	quill_formats::qld::write(&d, &s, &mut out).unwrap();
	assert_eq!(out.0, bytes);
}

#[test]
fn reading_is_memory_neutral() {
	let mut s = HostStore::new();
	let bytes = std::fs::read(root().join("tests/docs/plain.qld")).unwrap();
	let mut d = read_doc(&mut s, &bytes);
	d.reset(&mut s);
	assert_eq!(s.live_chunks(), 0, "reset frees every chunk");
}

#[test]
fn a_newer_major_version_is_refused() {
	let mut s = HostStore::new();
	let mut bytes = std::fs::read(root().join("tests/docs/plain.qld")).unwrap();
	// RIFF header 12 bytes, HEAD id and size 8 more, then the major version.
	bytes[20] = 2;
	let mut d = Doc::new();
	let r = quill_formats::qld::read(&mut d, &mut s, &mut SliceSource::new(&bytes));
	assert_eq!(r, Err(quill_formats::FormatError::Newer));
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(300))]

	// Every byte of damage either reads or is refused; nothing panics and no memory leaks.
	#[test]
	fn damaged_files_never_panic(flips in proptest::collection::vec((any::<u16>(), any::<u8>()), 1..8), cut in any::<u16>()) {
		let mut s = HostStore::new();
		let mut bytes = std::fs::read(root().join("tests/docs/plain.qld")).unwrap();
		for (i, v) in flips {
			let n = bytes.len();
			bytes[usize::from(i) % n] = v;
		}
		let keep = usize::from(cut) % (bytes.len() + 1);
		bytes.truncate(keep.max(bytes.len() - 8));
		let mut d = Doc::new();
		let _ = quill_formats::qld::read(&mut d, &mut s, &mut SliceSource::new(&bytes));
		d.reset(&mut s);
		prop_assert_eq!(s.live_chunks(), 0);
	}

	#[test]
	fn random_bytes_never_panic(bytes in proptest::collection::vec(any::<u8>(), 0..200)) {
		let mut s = HostStore::new();
		let mut d = Doc::new();
		let _ = quill_formats::qld::read(&mut d, &mut s, &mut SliceSource::new(&bytes));
		d.reset(&mut s);
		prop_assert_eq!(s.live_chunks(), 0);
	}

	// Lines cover the text: they never overlap, and with one-column characters none is wider than the page.
	#[test]
	fn layout_covers_the_text(text in "[a-z \\-]{0,300}") {
		let mut s = HostStore::new();
		let mut d = Doc::new();
		d.init(&mut s).unwrap();
		let mut body = text.into_bytes();
		body.push(b'\r');
		d.body.load(&mut s, &body, 0, 0).unwrap();
		let mut prev_end = 0u32;
		let mut ok = true;
		layout_story(&d, &s, &mut GridMeasure, StoryKind::Body, |l| {
			ok &= l.start == prev_end && l.width <= 60;
			prev_end = l.start + l.len;
		});
		prop_assert!(ok);
		prop_assert!(prev_end + 1 >= d.body.len(), "the last line reaches the paragraph mark");
	}
}
