// The Canadian dictionary built from the SCOWL lists in dict/scowl: every source word is found, near-misses are not, and the common
// misspellings get their word back among the first suggestions without running out of budget.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use quill_formats::io::SliceSource;
use quill_heap::HeapStore;
use quill_spell::Dict;

fn build() -> (Vec<Vec<u8>>, Vec<u8>) {
	let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dict/scowl");
	let mut words = Vec::new();
	for set in ["english", "canadian", "american"] {
		for kind in [
			"words",
			"upper",
			"proper-names",
			"contractions",
			"abbreviations",
		] {
			for size in [10, 20, 35, 40, 50] {
				if let Ok(t) = std::fs::read(dir.join(format!("{set}-{kind}.{size}"))) {
					words.extend(quill_dict::read_list(&t));
				}
			}
		}
	}
	let dic = quill_dict::build(&words, 1, "SCOWL");
	(words, dic)
}

#[test]
fn every_source_word_is_found_and_the_file_is_under_the_budget() {
	let (words, dic) = build();
	assert!(dic.len() < 300 * 1024, "{} bytes", dic.len());
	let mut s = HeapStore::new();
	let d = Dict::load(&mut s, &mut SliceSource::new(&dic)).unwrap();
	for w in &words {
		assert!(d.contains_exact(&s, w), "{}", String::from_utf8_lossy(w));
	}
	for bad in [
		"recieve",
		"definately",
		"seperate",
		"occured",
		"teh",
		"colourr",
		"wierd",
		"adress",
		"untill",
	] {
		assert!(!d.known(&s, bad.as_bytes()), "{bad}");
	}
	for good in [
		"receive",
		"definitely",
		"colour",
		"color",
		"centre",
		"center",
		"don't",
		"Canada",
		"Toronto",
		"café",
	] {
		let b: Vec<u8> = good.chars().map(|c| c as u32 as u8).collect();
		assert!(d.known(&s, &b), "{good}");
	}
}

#[test]
fn common_misspellings_get_their_word_in_the_first_eight_within_the_budget() {
	let (_, dic) = build();
	let mut s = HeapStore::new();
	let d = Dict::load(&mut s, &mut SliceSource::new(&dic)).unwrap();
	for (bad, good) in [
		("recieve", "receive"),
		("definately", "definitely"),
		("seperate", "separate"),
		("occured", "occurred"),
		("teh", "the"),
		("wierd", "weird"),
		("adress", "address"),
		("untill", "until"),
		("accomodate", "accommodate"),
		("neccessary", "necessary"),
	] {
		let sg = d.suggest(&s, bad.as_bytes(), 60_000);
		let found = (0..sg.len()).any(|i| sg.get(i).unwrap().bytes() == good.as_bytes());
		assert!(
			found,
			"{bad} -> {good}: {:?}",
			(0..sg.len())
				.map(|i| String::from_utf8_lossy(sg.get(i).unwrap().bytes()).into_owned())
				.collect::<Vec<_>>()
		);
		assert!(!sg.partial, "{bad} ran out of budget");
	}
}
