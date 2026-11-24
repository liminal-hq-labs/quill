// Compiles help sources and reads the files back with the crate's own reader.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
use quill_help::{hash, qhm, read::Hlp, write};

const SRC: &str = "title: Test\ncopyright: (c) Test\n\n= one | First | 1\n# Heading\nSome **bold** words and a [jump](two) and a [pop-up](!two).\n- an item\n\n= two | Second | 2\nThe end.\n";

#[test]
fn a_compiled_file_reads_back() {
	let help = qhm::parse(SRC).expect("parse");
	let bytes = write::build(&help).expect("build");
	let h = Hlp::new(&bytes).expect("open");
	let names: Vec<String> = h
		.directory()
		.expect("directory")
		.into_iter()
		.map(|(n, _)| n)
		.collect();
	for want in ["|CONTEXT", "|CTXOMAP", "|FONT", "|SYSTEM", "|TOPIC"] {
		assert!(
			names.iter().any(|n| n == want),
			"{want} missing from {names:?}"
		);
	}
	let sys = h.system().expect("system");
	assert_eq!(
		(sys.title.as_str(), sys.copyright.as_str()),
		("Test", "(c) Test")
	);
	let topics = h.topics().expect("topics");
	assert_eq!(
		topics.iter().map(|t| t.title.as_str()).collect::<Vec<_>>(),
		["First", "Second"]
	);
	let ctx = h.contexts().expect("contexts");
	for t in &topics {
		let name = if t.title == "First" { "one" } else { "two" };
		assert!(
			ctx.contains(&(hash(name), t.offset)),
			"{name} not at {:#x} in {ctx:?}",
			t.offset
		);
	}
	let text = topics[0].paragraphs.join("|");
	assert!(text.contains("{f1}bold{f0}"), "{text}");
	assert!(text.contains(&format!("[J:{:08X} ", hash("two"))), "{text}");
	assert!(text.contains(&format!("[P:{:08X} ", hash("two"))), "{text}");
}

#[test]
fn many_topics_span_blocks_and_index_pages() {
	let mut src = String::from("title: Big\n");
	for i in 0..120 {
		src.push_str(&format!("\n= t{i} | Topic {i}\n{}\n", "word ".repeat(150)));
	}
	let help = qhm::parse(&src).expect("parse");
	let bytes = write::build(&help).expect("build");
	let h = Hlp::new(&bytes).expect("open");
	let topics = h.topics().expect("topics");
	assert_eq!(topics.len(), 120);
	let ctx = h.contexts().expect("contexts");
	assert_eq!(ctx.len(), 120);
	for (i, t) in topics.iter().enumerate() {
		assert!(ctx.contains(&(hash(&format!("t{i}")), t.offset)), "t{i}");
	}
}

#[test]
fn bad_sources_name_the_line() {
	assert!(qhm::parse("text before a topic\n").is_err());
	let help = qhm::parse("= a | A\nsee [x](missing)\n").expect("parse");
	assert!(write::build(&help).is_err());
	assert!(qhm::parse("= a | A\n**open\n").is_err());
}
