// The Unicode test document set (WP-UNI-2, CORE-225): every document in `tests/docs/unicode/` read through today's readers.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Each document has an `.expect` (its code points as the format defines them) and a `.today` (what today's readers give: Windows-1252
//! with `?` and a count). A document whose two files agree must read, write and read again unchanged. A document whose files differ
//! carries `flips = "WP-UNI-N"` in `manifest.toml`: the test that it still differs is the marker that the package has not landed, and
//! when it lands the package makes `.today` equal `.expect`, which this file then holds it to. `beyond_windows_1252_reads_as_expect`
//! is the expected failure: run it with `--ignored` to see the documents that wait.

use std::collections::BTreeMap;
use std::path::PathBuf;

use quill_core::doc::Doc;
use quill_formats::io::{Sink, SliceSource};
use quill_formats::{qld, rtf, text};
use quill_host::HostStore;
use quill_text::{cp1252, cp437};

fn dir() -> PathBuf {
	PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/docs/unicode")
}

type Entry = BTreeMap<String, String>;

/// The entries of `manifest.toml`. The file is generated and flat: `[[doc]]` tables of `key = "string"` and `key = number` lines.
fn manifest() -> Vec<Entry> {
	let src = std::fs::read_to_string(dir().join("manifest.toml")).unwrap();
	let mut out: Vec<Entry> = Vec::new();
	for line in src.lines() {
		let line = line.trim();
		if line == "[[doc]]" {
			out.push(Entry::new());
		} else if line.is_empty() || line.starts_with('#') {
			continue;
		} else {
			let (k, v) = line.split_once(" = ").unwrap();
			let v = match v.strip_prefix('"') {
				Some(s) => s
					.strip_suffix('"')
					.unwrap()
					.replace("\\\\", "\u{0}")
					.replace("\\\"", "\"")
					.replace('\u{0}', "\\"),
				None => v.to_string(),
			};
			out.last_mut().unwrap().insert(k.to_string(), v);
		}
	}
	out
}

fn number(e: &Entry, key: &str) -> usize {
	e[key].parse().unwrap()
}

fn read_file(name: &str) -> Vec<u8> {
	std::fs::read(dir().join(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

/// The code points of an `.expect` or `.today` file, `U+XXXX` a line. `.today` files start with `refused=N`.
fn code_points(name: &str) -> (Vec<u32>, Option<usize>) {
	let text = String::from_utf8(read_file(name)).unwrap();
	let mut refused = None;
	let mut out = Vec::new();
	for line in text.lines() {
		if let Some(n) = line.strip_prefix("refused=") {
			refused = Some(n.parse().unwrap());
		} else {
			out.push(u32::from_str_radix(line.strip_prefix("U+").unwrap(), 16).unwrap());
		}
	}
	(out, refused)
}

struct VecSink(Vec<u8>);

impl Sink for VecSink {
	fn write(&mut self, data: &[u8]) -> bool {
		self.0.extend_from_slice(data);
		true
	}
}

/// The body of a document as code points: each byte is its Windows-1252 character and a paragraph mark stays U+000D.
fn body(s: &HostStore, d: &Doc) -> Vec<u32> {
	let mut bytes = vec![0u8; d.body.len() as usize];
	d.body.read(s, 0, &mut bytes);
	bytes
		.iter()
		.map(|&b| {
			if b < 0x20 {
				u32::from(b)
			} else {
				u32::from(cp1252::to_char(b))
			}
		})
		.collect()
}

/// What today's reader for `kind` makes of `bytes`, and how many characters it refused (RTF only).
fn read_today(kind: &str, bytes: &[u8]) -> (HostStore, Doc, usize) {
	let mut s = HostStore::new();
	let mut d = Doc::new();
	let mut refused = 0;
	let mut src = SliceSource::new(bytes);
	match kind {
		"rtf" => refused = rtf::read_counted(&mut d, &mut s, &mut src).unwrap().count,
		"text" => text::read(&mut d, &mut s, &mut src).unwrap(),
		"qld" => qld::read(&mut d, &mut s, &mut src).unwrap(),
		other => panic!("unknown kind {other}"),
	}
	(s, d, refused)
}

fn write_back(kind: &str, s: &HostStore, d: &Doc) -> Vec<u8> {
	let mut out = VecSink(Vec::new());
	match kind {
		"rtf" => rtf::write(d, s, &mut out).unwrap(),
		"text" => text::write(d, s, &mut out).unwrap(),
		"qld" => qld::write(d, s, &mut out).unwrap(),
		other => panic!("unknown kind {other}"),
	}
	out.0
}

fn files_listed(entries: &[Entry]) -> Vec<String> {
	entries.iter().map(|e| e["file"].clone()).collect()
}

#[test]
fn the_manifest_lists_every_document_and_every_document_has_its_files() {
	let entries = manifest();
	let listed = files_listed(&entries);
	assert!(listed.len() >= 40, "found {} documents", listed.len());
	let mut on_disk: Vec<String> = std::fs::read_dir(dir())
		.unwrap()
		.map(|e| e.unwrap().file_name().into_string().unwrap())
		.filter(|n| n != "manifest.toml" && n != "README.md")
		.filter(|n| {
			!["expect", "today", "clusters", "breaks"]
				.iter()
				.any(|x| n.ends_with(&format!(".{x}")))
		})
		.collect();
	on_disk.sort();
	let mut want = listed.clone();
	want.sort();
	assert_eq!(
		on_disk, want,
		"a document is not in manifest.toml, or one in it is missing"
	);
	for f in &listed {
		for x in ["expect", "today", "clusters", "breaks"] {
			assert!(
				dir().join(format!("{f}.{x}")).exists(),
				"{f}.{x} is missing"
			);
		}
	}
}

#[test]
fn the_reference_files_agree_with_the_manifest_and_with_each_other() {
	for e in manifest() {
		let f = &e["file"];
		let (expect, _) = code_points(&format!("{f}.expect"));
		assert_eq!(expect.len(), number(&e, "scalars"), "{f}: scalars");
		assert_eq!(
			expect.iter().filter(|&&c| c == 0x0D).count(),
			number(&e, "paragraphs"),
			"{f}: paragraphs"
		);
		assert_eq!(
			expect.last(),
			Some(&0x0D),
			"{f}: a document ends with a paragraph mark"
		);
		assert!(
			expect.iter().all(|&c| char::from_u32(c).is_some()),
			"{f}: every code point is a scalar value"
		);
		// Clusters: a line of lengths a paragraph, which add up to the paragraph's length.
		let clusters = String::from_utf8(read_file(&format!("{f}.clusters"))).unwrap();
		let paras: Vec<&[u32]> = expect.split_inclusive(|&c| c == 0x0D).collect();
		assert_eq!(
			clusters.lines().count(),
			paras.len(),
			"{f}: clusters per paragraph"
		);
		let mut total = 0;
		for (line, para) in clusters.lines().zip(&paras) {
			let lens: Vec<usize> = line.split(' ').map(|n| n.parse().unwrap()).collect();
			assert_eq!(
				lens.iter().sum::<usize>(),
				para.len(),
				"{f}: cluster lengths of a paragraph"
			);
			assert_eq!(
				lens.last(),
				Some(&1),
				"{f}: the paragraph mark is a cluster of its own"
			);
			total += lens.len();
		}
		assert_eq!(total, number(&e, "clusters"), "{f}: clusters");
		// Breaks: increasing offsets inside the paragraph, none before its mark.
		let breaks = String::from_utf8(read_file(&format!("{f}.breaks"))).unwrap();
		assert_eq!(
			breaks.lines().count(),
			paras.len(),
			"{f}: breaks per paragraph"
		);
		for (line, para) in breaks.lines().zip(&paras) {
			let at: Vec<usize> = line
				.split(' ')
				.filter(|s| !s.is_empty())
				.map(|n| n.parse().unwrap())
				.collect();
			assert!(
				at.windows(2).all(|w| w[0] < w[1]),
				"{f}: break offsets increase"
			);
			assert!(
				at.iter().all(|&o| o > 0 && o < para.len()),
				"{f}: a break is inside the paragraph, before its mark"
			);
		}
		// What each build shows: the counts cover every cluster that is not a paragraph mark or a tab.
		let structural = expect.iter().filter(|&&c| c == 0x0D || c == 0x09).count();
		for build in ["dos_text", "dos_gfx", "win16", "win32", "tty", "gtk"] {
			let sum: usize = e[build]
				.split(' ')
				.map(|kv| kv.split_once('=').unwrap().1.parse::<usize>().unwrap())
				.sum();
			assert_eq!(sum, number(&e, "clusters") - structural, "{f}: {build}");
		}
		// The two expectations a build's counts cannot disagree on: the retro builds show the same.
		assert_eq!(e["win16"], e["win32"], "{f}");
		assert_eq!(e["win16"], e["dos_gfx"], "{f}");
	}
}

/// The text a text file holds, decoded by the standard library and the rules of CORE-221, as a check on the generated `.expect`.
fn decode_text(bytes: &[u8], encoding: &str) -> Vec<u32> {
	let chars: Vec<char> = match encoding {
		"utf-8-bom" => String::from_utf8(bytes[3..].to_vec())
			.unwrap()
			.chars()
			.collect(),
		"utf-8" => String::from_utf8(bytes.to_vec()).unwrap().chars().collect(),
		"utf-16le-bom" | "utf-16be-bom" => {
			let le = encoding == "utf-16le-bom";
			let units: Vec<u16> = bytes[2..]
				.chunks(2)
				.map(|p| {
					if le {
						u16::from_le_bytes([p[0], p[1]])
					} else {
						u16::from_be_bytes([p[0], p[1]])
					}
				})
				.collect();
			char::decode_utf16(units).map(|r| r.unwrap()).collect()
		}
		"windows-1252" | "invalid-utf8" => {
			assert!(
				String::from_utf8(bytes.to_vec()).is_err(),
				"the C4 rule reads this file as the code page only when it is not UTF-8"
			);
			bytes.iter().map(|&b| cp1252::to_char(b)).collect()
		}
		"cp437" => bytes
			.iter()
			.map(|&b| {
				if b < 0x20 {
					char::from(b)
				} else {
					cp437::to_char(b)
				}
			})
			.collect(),
		other => panic!("unknown text encoding {other}"),
	};
	let mut out = Vec::new();
	let mut prev_cr = false;
	for c in chars {
		let cp = u32::from(c);
		if cp == 0x0A && prev_cr {
			prev_cr = false;
			continue;
		}
		prev_cr = cp == 0x0D;
		match cp {
			0x0A | 0x0C | 0x0D => out.push(0x0D),
			0x09 | 0x20.. => out.push(cp),
			_ => {}
		}
	}
	if out.last() != Some(&0x0D) {
		out.push(0x0D);
	}
	out
}

#[test]
fn a_text_file_decodes_to_its_expect_by_the_standard_library() {
	let mut seen = 0;
	for e in manifest().iter().filter(|e| e["kind"] == "text") {
		let f = &e["file"];
		let (expect, _) = code_points(&format!("{f}.expect"));
		assert_eq!(decode_text(&read_file(f), &e["encoding"]), expect, "{f}");
		seen += 1;
	}
	assert!(seen >= 25, "found {seen} text files");
}

#[test]
fn every_document_reads_today_as_its_today_file() {
	for e in manifest() {
		let f = &e["file"];
		let (s, d, refused) = read_today(&e["kind"], &read_file(f));
		let (today, want_refused) = code_points(&format!("{f}.today"));
		assert_eq!(body(&s, &d), today, "{f}: today's reader");
		assert_eq!(refused, want_refused.unwrap(), "{f}: characters refused");
		assert_eq!(
			refused,
			number(&e, "refused_today"),
			"{f}: refused_today in the manifest"
		);
	}
}

fn pending(e: &Entry) -> bool {
	e.contains_key("flips")
}

#[test]
fn a_document_with_nothing_beyond_windows_1252_reads_as_its_expect_and_survives_a_save() {
	let mut checked = 0;
	for e in manifest().iter().filter(|e| !pending(e)) {
		let f = &e["file"];
		let (expect, _) = code_points(&format!("{f}.expect"));
		let (today, _) = code_points(&format!("{f}.today"));
		assert_eq!(
			today, expect,
			"{f}: no `flips` in the manifest, so today's reader must give the expect"
		);
		assert_eq!(number(e, "refused_today"), 0, "{f}");
		let bytes = read_file(f);
		let (s, d, _) = read_today(&e["kind"], &bytes);
		// Written and read again, the document is the same.
		let saved = write_back(&e["kind"], &s, &d);
		let (s2, d2, refused) = read_today(&e["kind"], &saved);
		assert_eq!(body(&s2, &d2), expect, "{f}: after a save");
		assert_eq!(refused, 0, "{f}: after a save");
		checked += 1;
	}
	assert!(
		checked >= 5,
		"found {checked} documents that need no new door"
	);
}

#[test]
fn a_one_point_oh_file_stays_one_point_oh_on_save() {
	let bytes = read_file("latin-1-0.qld");
	let (s, d, _) = read_today("qld", &bytes);
	assert_eq!(
		write_back("qld", &s, &d),
		bytes,
		"latin-1-0.qld is what the writer makes of it"
	);
}

#[test]
fn a_document_beyond_windows_1252_still_differs_from_its_expect() {
	// The marker: this fails the moment a package opens a door, and says to flip the files and the manifest.
	let mut waiting = Vec::new();
	for e in manifest().iter().filter(|e| pending(e)) {
		let f = &e["file"];
		let (expect, _) = code_points(&format!("{f}.expect"));
		let (today, refused) = code_points(&format!("{f}.today"));
		assert_ne!(
			(&today, refused),
			(&expect, Some(0)),
			"{f}: today's reader now gives the expect; make `.today` equal `.expect` and remove `flips` from its manifest entry"
		);
		waiting.push(format!("{} ({})", f, e["flips"]));
	}
	assert!(
		waiting.len() >= 30,
		"found {} documents waiting",
		waiting.len()
	);
	eprintln!("PENDING {} documents wait for a package that lets a character beyond Windows-1252 in:\n  {}", waiting.len(), waiting.join("\n  "));
}

#[test]
#[ignore = "PENDING WP-UNI-4: files do not yet keep a character beyond Windows-1252; `cargo test -p quill-host --test unicode_docs -- --ignored` lists what waits"]
fn beyond_windows_1252_reads_as_expect() {
	let mut failing = Vec::new();
	for e in manifest().iter().filter(|e| pending(e)) {
		let f = &e["file"];
		let (expect, _) = code_points(&format!("{f}.expect"));
		let (s, d, refused) = read_today(&e["kind"], &read_file(f));
		if body(&s, &d) != expect || refused != 0 {
			failing.push(format!("{f} ({})", e["flips"]));
		}
	}
	assert!(
		failing.is_empty(),
		"{} documents do not read as their expect yet:\n  {}",
		failing.len(),
		failing.join("\n  ")
	);
}
