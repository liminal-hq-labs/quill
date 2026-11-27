// The recovery question in the text screens' own box: its screens against the goldens in `tests/golden/screens`, and what each answer
// leaves on the disk.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::PathBuf;

use quill_host::script::{parse_script, play, stage, MemFiles};

fn root() -> PathBuf {
	PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Plays `tests/scenarios/NAME.keys` and returns the files and the frames.
fn scenario(name: &str) -> (MemFiles, String) {
	let text =
		std::fs::read_to_string(root().join(format!("tests/scenarios/{name}.keys"))).unwrap();
	let sc = parse_script(&text);
	let mut files = MemFiles::default();
	stage(&mut files, &sc, &root().join("tests/docs"));
	let mut frames = String::new();
	play(&mut files, &sc, &mut frames);
	(files, frames)
}

fn compare(name: &str, frames: &str) {
	let path = root().join(format!("tests/golden/screens/{name}.frames.txt"));
	if std::env::var_os("QUILL_UPDATE_GOLDEN").is_some() {
		std::fs::write(&path, frames).unwrap();
		return;
	}
	let want = std::fs::read_to_string(&path)
		.unwrap_or_else(|_| panic!("missing golden for {name}; run with QUILL_UPDATE_GOLDEN=1"));
	assert_eq!(want, frames, "{name}: the screens differ");
}

macro_rules! screens {
	($($test:ident => $name:literal,)*) => {
		$(#[test] fn $test() { let (_, frames) = scenario($name); compare($name, &frames); })*
	};
}

screens! {
	m17_recover_screens => "m17-recover",
	m17_recover_50_screens => "m17-recover-50",
	m17_discard_screens => "m17-discard",
	m17_later_screens => "m17-later",
}

const COPY: &str = "C:\\RECOVER\\QUILL.RCV";

#[test]
fn recover_keeps_the_copy_until_the_document_is_saved() {
	let (files, frames) = scenario("m17-recover");
	assert!(frames.contains("Quill found"), "no question was shown");
	assert!(files.files.contains_key(COPY));
}

#[test]
fn discard_removes_the_copy() {
	let (files, _) = scenario("m17-discard");
	assert!(!files.files.contains_key(COPY));
}

#[test]
fn later_leaves_the_copy_alone() {
	let (files, _) = scenario("m17-later");
	assert!(files.files.contains_key(COPY));
}
