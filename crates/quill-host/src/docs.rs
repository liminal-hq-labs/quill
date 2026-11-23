// The test documents, built in code so the committed .qld files can be checked against them.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use quill_core::doc::{Doc, PFmt};
use quill_core::Result;

use crate::HostStore;

/// The body of `tests/docs/plain.qld`: plain text in the default formats, with a long paragraph that wraps, a tab, a
/// hard line break, a word wider than nothing and an empty paragraph.
pub const PLAIN: &str = "Quill: a plain document\rQuill is a small word processor for DOS, Windows 3.1 and Windows 95, written in Rust. This paragraph is long enough to wrap onto several lines in a sixty column text area, and its lines break after spaces and hyphens, never in the middle of a word unless the word is wider than the line.\r\rName\tQuantity\tPrice\rQuills\t12\t3.50\rInk bottles\t3\t8.25\r\rA line break\x0bkeeps the paragraph together, and a supercalifragilisticexpialidocious-looking-word-that-has-no-spaces-and-is-far-longer-than-a-line is cut where it must be.\rThe end.\r";

/// Builds the `plain` document.
pub fn plain(s: &mut HostStore) -> Result<Doc> {
	let mut d = Doc::new();
	d.init(s)?;
	d.body.load(s, PLAIN.as_bytes(), 0, 0)?;
	// The title gets its own paragraph format: centred, with space after.
	let title_len = PLAIN.find('\r').map_or(0, |i| i as u32 + 1);
	let pf = d.pfmts.intern(
		s,
		PFmt {
			align: 1,
			after: 240,
			..PFmt::DEFAULT
		},
	)?;
	d.body.set_pfmt(s, 0, title_len, pf, 0)?;
	d.body.forget_history(s);
	Ok(d)
}

/// The bytes of `plain.qld`.
pub fn plain_bytes(s: &mut HostStore) -> Result<Vec<u8>> {
	let d = plain(s)?;
	let mut out = VecSink(Vec::new());
	quill_formats::qld::write(&d, s, &mut out).map_err(|_| quill_core::Error::NoMemory)?;
	Ok(out.0)
}

/// A sink that collects bytes in memory.
pub struct VecSink(pub Vec<u8>);

impl quill_formats::io::Sink for VecSink {
	fn write(&mut self, data: &[u8]) -> bool {
		self.0.extend_from_slice(data);
		true
	}
}

/// The bytes of `letter.qld`: the same document the RTF corpus holds, built in code.
pub fn letter_bytes(s: &mut HostStore) -> Result<Vec<u8>> {
	let mut d = Doc::new();
	let rtf = std::fs::read(concat!(
		env!("CARGO_MANIFEST_DIR"),
		"/../../tests/docs/letter.rtf"
	))
	.map_err(|_| quill_core::Error::Range)?;
	quill_formats::rtf::read(&mut d, s, &mut quill_formats::io::SliceSource::new(&rtf))
		.map_err(|_| quill_core::Error::Range)?;
	let mut out = VecSink(Vec::new());
	quill_formats::qld::write(&d, s, &mut out).map_err(|_| quill_core::Error::NoMemory)?;
	Ok(out.0)
}
