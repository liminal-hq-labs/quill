// Compatibility: the frozen `.QLD` 1.0 reader opens every `.QLD` file in the corpus, including 1.1 files with a `UNIC` chunk (CORE-225).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `qld_1_0.rs` is a frozen copy of the 1.0 reader and is never edited (`scripts/check-frozen-reader.sh` holds its hash). These tests
//! read every `tests/docs/**/*.qld` through it and through today's reader. For a 1.1 file they check what the format promises an
//! old reader: the story's length, runs, paragraph marks and objects are right, and each character of the `UNIC` chunk shows as `?`.

#[rustfmt::skip]
#[allow(dead_code)]
mod qld_1_0;

use std::path::{Path, PathBuf};

use quill_core::doc::Doc;
use quill_formats::io::SliceSource;
use quill_formats::FormatError;
use quill_host::HostStore;

fn docs() -> PathBuf {
	PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/docs")
}

fn qld_files(dir: &Path, out: &mut Vec<PathBuf>) {
	let mut entries: Vec<_> = std::fs::read_dir(dir)
		.unwrap()
		.map(|e| e.unwrap().path())
		.collect();
	entries.sort();
	for p in entries {
		if p.is_dir() {
			qld_files(&p, out);
		} else if p.extension().and_then(|x| x.to_str()) == Some("qld") {
			out.push(p);
		}
	}
}

fn corpus() -> Vec<PathBuf> {
	let mut out = Vec::new();
	qld_files(&docs(), &mut out);
	out
}

fn body(s: &HostStore, d: &Doc) -> Vec<u8> {
	let mut b = vec![0u8; d.body.len() as usize];
	d.body.read(s, 0, &mut b);
	b
}

fn read_frozen(bytes: &[u8]) -> Result<(HostStore, Doc), FormatError> {
	let mut s = HostStore::new();
	let mut d = Doc::new();
	qld_1_0::read(&mut d, &mut s, &mut SliceSource::new(bytes))?;
	Ok((s, d))
}

fn read_today(bytes: &[u8]) -> (HostStore, Doc) {
	let mut s = HostStore::new();
	let mut d = Doc::new();
	quill_formats::qld::read(&mut d, &mut s, &mut SliceSource::new(bytes)).unwrap();
	(s, d)
}

fn u16_at(b: &[u8], at: usize) -> usize {
	usize::from(u16::from_le_bytes([b[at], b[at + 1]]))
}

fn u32_at(b: &[u8], at: usize) -> usize {
	u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]]) as usize
}

/// The chunks of the form starting at `at` (just past its form type), as `(id, payload)`.
fn chunks(b: &[u8], mut at: usize, end: usize) -> Vec<([u8; 4], &[u8])> {
	let mut out = Vec::new();
	while at + 8 <= end {
		let id = [b[at], b[at + 1], b[at + 2], b[at + 3]];
		let size = u32_at(b, at + 4);
		out.push((id, &b[at + 8..at + 8 + size]));
		at += 8 + size + size % 2;
	}
	out
}

/// The `(version, positions of the characters the first story's `UNIC` chunk holds, and their UTF-8)` of a file, found by walking its
/// chunks here so the test does not depend on a reader.
fn unic(bytes: &[u8]) -> ((usize, usize), Vec<(usize, String)>) {
	assert_eq!(&bytes[..4], b"RIFF");
	assert_eq!(&bytes[8..12], b"QLD1");
	let top = chunks(bytes, 12, bytes.len());
	let head = top.iter().find(|c| &c.0 == b"HEAD").unwrap().1;
	let version = (u16_at(head, 0), u16_at(head, 2));
	let mut runs = Vec::new();
	for (id, payload) in &top {
		if id != b"LIST" || payload.len() < 4 || &payload[..4] != b"STRY" {
			continue;
		}
		for (sub, data) in chunks(payload, 4, payload.len()) {
			if &sub != b"UNIC" {
				continue;
			}
			let mut at = 0;
			while at < data.len() {
				let position = u32_at(data, at);
				let chars = u16_at(data, at + 4);
				let len = u16_at(data, at + 6);
				let text = std::str::from_utf8(&data[at + 8..at + 8 + len])
					.unwrap()
					.to_string();
				assert_eq!(text.chars().count(), chars, "a run's character count");
				runs.push((position, text));
				at += 8 + len;
			}
		}
	}
	(version, runs)
}

#[test]
fn the_frozen_reader_opens_every_qld_file_in_the_corpus_and_reads_what_todays_reader_reads() {
	let files = corpus();
	assert!(files.len() >= 4, "found {} .qld files", files.len());
	for path in files {
		let bytes = std::fs::read(&path).unwrap();
		let (s, d) = read_frozen(&bytes)
			.unwrap_or_else(|e| panic!("{}: the 1.0 reader refused it: {e:?}", path.display()));
		let (s2, d2) = read_today(&bytes);
		assert_eq!(
			body(&s, &d),
			body(&s2, &d2),
			"{}: the story",
			path.display()
		);
	}
}

#[test]
fn a_one_point_one_file_shows_question_marks_in_exactly_the_unic_positions_to_a_one_point_oh_reader(
) {
	let mut with_unic = 0;
	for path in corpus() {
		let bytes = std::fs::read(&path).unwrap();
		let ((major, minor), runs) = unic(&bytes);
		assert_eq!(major, 1, "{}", path.display());
		if minor == 0 {
			assert!(
				runs.is_empty(),
				"{}: a 1.0 file has no UNIC chunk",
				path.display()
			);
			continue;
		}
		with_unic += 1;
		assert!(
			!runs.is_empty(),
			"{}: a 1.1 file in this corpus carries a UNIC chunk",
			path.display()
		);
		let (s, d) = read_frozen(&bytes).unwrap();
		let text = body(&s, &d);
		let mut in_run = vec![false; text.len()];
		let mut last_end = 0;
		for (position, chars) in &runs {
			assert!(
				*position >= last_end,
				"{}: runs are in order",
				path.display()
			);
			let count = chars.chars().count();
			for i in *position..*position + count {
				assert_eq!(
					text[i],
					b'?',
					"{}: byte {i} is a UNIC position",
					path.display()
				);
				in_run[i] = true;
			}
			last_end = position + count;
		}
		// The corpus' 1.1 files have no `?` of their own, so every other byte is a Windows-1252 character, not a `?`.
		for (i, b) in text.iter().enumerate() {
			assert!(
				in_run[i] || *b != b'?',
				"{}: byte {i} is a `?` outside the UNIC runs",
				path.display()
			);
		}
		assert_eq!(
			text.last(),
			Some(&0x0D),
			"{}: the story still ends with a paragraph mark",
			path.display()
		);
	}
	assert!(with_unic >= 1, "the corpus holds no 1.1 file");
}

#[test]
fn the_frozen_reader_is_a_one_point_oh_reader_and_refuses_a_newer_major_version() {
	assert_eq!(qld_1_0::VERSION, (1, 0));
	let mut bytes = std::fs::read(docs().join("plain.qld")).unwrap();
	// HEAD is the first chunk: RIFF, size, QLD1, then `HEAD`, its size, then the major version.
	assert_eq!(&bytes[12..16], b"HEAD");
	bytes[20] = 2;
	assert_eq!(read_frozen(&bytes).err(), Some(FormatError::Newer));
	// A later minor version is read.
	bytes[20] = 1;
	bytes[22] = 9;
	assert!(read_frozen(&bytes).is_ok());
}
