// quill-dict: builds EN_CA.DIC, EN_US.DIC and EN_GB.DIC from the SCOWL lists in dict/scowl.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// usage: quill-dict [--size N] [--dir dict/scowl] OUT_DIR
//
// A language is the English lists up to the size (words, capitalised words, proper names, contractions and abbreviations) plus the
// lists of its variants: Canadian takes the Canadian and the American lists (a Canadian reader meets both -ize and -ise), American the
// American, British the British. The size defaults to 50; the budget is under 300 KB per language and the tool says when it is over.
use std::path::PathBuf;

const KINDS: [&str; 5] = [
	"words",
	"upper",
	"proper-names",
	"contractions",
	"abbreviations",
];
const SIZES: [u32; 5] = [10, 20, 35, 40, 50];

fn main() {
	let mut size = 50u32;
	let mut dir = PathBuf::from("dict/scowl");
	let mut out = None;
	let mut args = std::env::args().skip(1);
	while let Some(a) = args.next() {
		match a.as_str() {
			"--size" => size = args.next().and_then(|v| v.parse().ok()).unwrap_or(50),
			"--dir" => dir = PathBuf::from(args.next().unwrap_or_default()),
			_ => out = Some(PathBuf::from(a)),
		}
	}
	let Some(out) = out else {
		eprintln!("usage: quill-dict [--size N] [--dir dict/scowl] OUT_DIR");
		std::process::exit(2);
	};
	std::fs::create_dir_all(&out).expect("create the output folder");
	let licence = std::fs::read_to_string(dir.join("COPYRIGHT")).unwrap_or_default();
	let licence = format!(
		"Quill spelling dictionary built from SCOWL (Spell Checker Oriented Word Lists), size {size}.\r\n\r\n{licence}"
	);
	for (file, code, sets) in [
		("EN_CA.DIC", 1u16, &["english", "canadian", "american"][..]),
		("EN_US.DIC", 2, &["english", "american"][..]),
		("EN_GB.DIC", 3, &["english", "british"][..]),
	] {
		let mut words = Vec::new();
		for set in sets {
			for kind in KINDS {
				for s in SIZES.iter().filter(|s| **s <= size) {
					if let Ok(t) = std::fs::read(dir.join(format!("{set}-{kind}.{s}"))) {
						words.extend(quill_dict::read_list(&t));
					}
				}
			}
		}
		let dic = quill_dict::build(&words, code, &licence);
		let note = if dic.len() > 300 * 1024 {
			"  OVER THE 300 KB BUDGET"
		} else {
			""
		};
		println!("{file}: {} words, {} bytes{note}", words.len(), dic.len());
		std::fs::write(out.join(file), dic).expect("write the dictionary");
	}
}
