// Spelling: the automaton finds exactly the words it was built from, the case rules hold, suggestions are right and bounded.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use quill_formats::io::SliceSource;
use quill_heap::HeapStore;
use quill_spell::Dict;

fn words(list: &[&str]) -> Vec<Vec<u8>> {
	list.iter()
		.map(|w| w.chars().map(|c| c as u32 as u8).collect())
		.collect()
}

fn open(s: &mut HeapStore, list: &[&str]) -> Dict {
	let bytes = quill_dict::build(&words(list), 1, "licence text");
	Dict::load(s, &mut SliceSource::new(&bytes)).unwrap()
}

#[test]
fn it_finds_what_was_put_in_and_nothing_else() {
	let mut s = HeapStore::new();
	let list = [
		"a",
		"ab",
		"abet",
		"abide",
		"bet",
		"better",
		"bettor",
		"can't",
		"Monday",
		"NASA",
		"colour",
		"color",
		"caf\u{e9}",
	];
	let d = open(&mut s, &list);
	assert_eq!(d.words(), list.len() as u32);
	for w in &list {
		let b: Vec<u8> = w.chars().map(|c| c as u32 as u8).collect();
		assert!(d.contains_exact(&s, &b), "{w}");
	}
	for w in ["", "abe", "abid", "betters", "can", "cant", "Mond", "x"] {
		assert!(!d.contains_exact(&s, w.as_bytes()), "{w:?}");
	}
}

#[test]
fn case_rules() {
	let mut s = HeapStore::new();
	let d = open(&mut s, &["colour", "Monday", "NASA", "better"]);
	for w in ["colour", "Colour", "COLOUR", "Monday", "NASA", "BETTER"] {
		assert!(d.known(&s, w.as_bytes()), "{w}");
	}
	// Mixed case that is not in the dictionary is unknown, and so is a wrong word.
	for w in ["cOlour", "monday", "MONDAY", "Nasa", "colur"] {
		let known = d.known(&s, w.as_bytes());
		// SCOWL holds Monday capitalised, so the lower-case form is not a word, but all capitals may stand for the capitalised one.
		assert_eq!(known, w == "MONDAY", "{w}");
	}
}

#[test]
fn suggestions_are_within_two_edits_closest_first_and_keep_the_case() {
	let mut s = HeapStore::new();
	let d = open(
		&mut s,
		&[
			"spelling", "speaking", "spewing", "smelling", "spilling", "selling", "the", "then",
			"them", "they",
		],
	);
	let sg = d.suggest(&s, b"speling", 10_000);
	let got: Vec<String> = (0..sg.len())
		.map(|i| String::from_utf8_lossy(sg.get(i).unwrap().bytes()).into_owned())
		.collect();
	assert!(got.iter().take(3).any(|w| w == "spelling"), "{got:?}");
	assert!(
		got.contains(&"spewing".to_string()) || got.contains(&"selling".to_string()),
		"{got:?}"
	);
	assert_eq!(sg.get(0).unwrap().cost, 1);
	// A swap of neighbours is one edit.
	let sg = d.suggest(&s, b"teh", 10_000);
	assert_eq!(sg.get(0).unwrap().bytes(), b"the");
	assert_eq!(sg.get(0).unwrap().cost, 1);
	// Capitals carry over.
	let has = |sg: &quill_spell::Suggestions, w: &[u8]| {
		(0..sg.len()).any(|i| sg.get(i).unwrap().bytes() == w)
	};
	assert!(has(&d.suggest(&s, b"Speling", 10_000), b"Spelling"));
	assert!(has(&d.suggest(&s, b"SPELING", 10_000), b"SPELLING"));
	// A tiny budget stops early and says so.
	let sg = d.suggest(&s, b"speling", 3);
	assert!(sg.partial);
	// Nothing is within two edits of nonsense.
	assert!(d.suggest(&s, b"qqqqqqqq", 10_000).is_empty());
}

#[test]
fn a_damaged_file_is_refused_and_chunks_are_given_back() {
	let mut s = HeapStore::new();
	let bytes = quill_dict::build(&words(&["alpha", "beta"]), 1, "x");
	assert!(Dict::load(&mut s, &mut SliceSource::new(&bytes[..20])).is_err());
	let mut bad = bytes.clone();
	bad[0] = b'X';
	assert!(Dict::load(&mut s, &mut SliceSource::new(&bad)).is_err());
	let mut d = Dict::load(&mut s, &mut SliceSource::new(&bytes)).unwrap();
	d.free(&mut s);
}

#[test]
fn the_licence_travels_with_the_file() {
	let mut s = HeapStore::new();
	let d = open(&mut s, &["x"]);
	let mut b = [0u8; 32];
	let n = d.licence(&s, 0, &mut b);
	assert_eq!(&b[..n], b"licence text");
}
