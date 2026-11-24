// The documents the spike runs on: every file in tests/docs, and a generated document of about 1 MB.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::PathBuf;

use quill_core::doc::{Doc, PFmt};
use quill_formats::io::SliceSource;
use quill_host::docs::VecSink;
use quill_host::HostStore;

use crate::rng::Rng;

/// The repository root.
pub fn root() -> PathBuf {
	PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The document files in tests/docs (the pictures and the dictionary are not documents).
pub const FILES: [&str; 7] = [
	"letter.qld",
	"plain.qld",
	"headers.rtf",
	"letter.rtf",
	"lists.rtf",
	"pictures.rtf",
	"print3.rtf",
];

/// Reads a tests/docs file into a new document in `s`.
pub fn load(s: &mut HostStore, name: &str) -> Doc {
	let bytes = std::fs::read(root().join("tests/docs").join(name)).expect("test document");
	let mut d = Doc::new();
	let mut src = SliceSource::new(&bytes);
	if name.ends_with(".qld") {
		quill_formats::qld::read(&mut d, s, &mut src).expect("qld");
	} else {
		quill_formats::rtf::read(&mut d, s, &mut src).expect("rtf");
	}
	d
}

const WORDS: [&str; 24] = [
	"the",
	"quill",
	"writes",
	"a",
	"letter",
	"to",
	"every",
	"reader",
	"who",
	"asks",
	"for",
	"small",
	"tools",
	"that",
	"run",
	"on",
	"old",
	"machines",
	"and",
	"keep",
	"their",
	"promises",
	"well-made",
	"pages",
];

/// The text of a generated document: `target` bytes of paragraphs of 20 to 400 characters with a tab or a hard break now and
/// then. Returns the text and the (start, end) of the paragraphs that get a format of their own.
pub fn big_text(target: usize, seed: u64) -> (Vec<u8>, Vec<(u32, u32)>) {
	let mut rng = Rng(seed);
	let mut out = Vec::with_capacity(target + 512);
	let mut special = Vec::new();
	let mut n = 0u32;
	while out.len() < target - 450 {
		let start = out.len() as u32;
		let want = 20 + rng.below(380) as usize;
		let mut para = Vec::new();
		while para.len() < want {
			let w = WORDS[rng.below(WORDS.len() as u64) as usize];
			para.extend_from_slice(w.as_bytes());
			match rng.below(40) {
				0 => para.push(b'\t'),
				1 => para.push(0x0B),
				_ => para.push(b' '),
			}
		}
		out.extend_from_slice(&para);
		out.push(0x0D);
		n += 1;
		if n.is_multiple_of(25) || n.is_multiple_of(61) {
			special.push((start, out.len() as u32));
		}
	}
	(out, special)
}

/// A document of about 1 MB (the most Quill holds is 1,048,575 bytes) in `s`.
pub fn big_doc(s: &mut HostStore) -> Doc {
	let (text, special) = big_text(1_000_000, 7);
	let mut d = Doc::new();
	d.init(s).expect("init");
	d.body.load(s, &text, 0, 0).expect("load");
	let centred = d
		.pfmts
		.intern(
			s,
			PFmt {
				align: 1,
				after: 240,
				..PFmt::DEFAULT
			},
		)
		.expect("pfmt");
	let page_break = d
		.pfmts
		.intern(
			s,
			PFmt {
				flags: 2,
				before: 120,
				..PFmt::DEFAULT
			},
		)
		.expect("pfmt");
	for (i, (a, b)) in special.iter().enumerate() {
		let pf = if i % 7 == 0 { page_break } else { centred };
		d.body.set_pfmt(s, *a, b - a, pf, 0).expect("set_pfmt");
	}
	d.body.forget_history(s);
	d
}

/// The `.QLD` bytes of a document.
pub fn qld_bytes(d: &Doc, s: &HostStore) -> Vec<u8> {
	let mut out = VecSink(Vec::new());
	quill_formats::qld::write(d, s, &mut out).expect("write");
	out.0
}

/// How many paragraphs the body has (marks, plus one when the text ends without one).
pub fn paragraphs(d: &Doc, s: &HostStore) -> u32 {
	let t = &d.body;
	let mut buf = vec![0u8; t.len() as usize];
	let n = t.read(s, 0, &mut buf);
	let marks = buf[..n].iter().filter(|b| **b == 0x0D).count() as u32;
	marks + u32::from(n > 0 && buf[n - 1] != 0x0D)
}
