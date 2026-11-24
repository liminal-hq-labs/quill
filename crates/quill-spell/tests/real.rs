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

/// Prints what one dictionary lookup costs on this machine, for the notes on spelling as you type. Run it with
/// `cargo test -p quill-spell --release --test real lookup_cost -- --ignored --nocapture`.
#[test]
#[ignore]
fn lookup_cost() {
	use std::time::Instant;
	let (words, dic) = build();
	let mut s = HeapStore::new();
	let d = Dict::load(&mut s, &mut SliceSource::new(&dic)).unwrap();
	let text: Vec<&Vec<u8>> = words.iter().filter(|w| w.len() <= 20).take(100_000).collect();
	let mut sink = 0usize;
	let t = Instant::now();
	let rounds = 5;
	for _ in 0..rounds {
		for w in &text {
			sink += usize::from(d.known(&s, w));
		}
	}
	let per = t.elapsed().as_nanos() / (rounds * text.len() as u128);
	let avg: usize = text.iter().map(|w| w.len()).sum::<usize>() / text.len();
	println!("lookup_cost: {per} ns a word over {} words of {avg} letters on average ({sink} found)", text.len());
	// A misspelt word walks the same edges and stops sooner.
	let t = Instant::now();
	for w in text.iter().take(20_000) {
		let mut bad = w.to_vec();
		bad.push(b'q');
		bad.push(b'x');
		sink += usize::from(d.known(&s, &bad));
	}
	println!("lookup_cost: {} ns a misspelt word ({sink})", t.elapsed().as_nanos() / 20_000);
}
