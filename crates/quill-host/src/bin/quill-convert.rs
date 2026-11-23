// quill-convert: reads a document by content (QLD, RTF or text) and writes it as QLD, RTF or text by the output's extension.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::process::ExitCode;

use quill_core::doc::Doc;
use quill_formats::io::SliceSource;
use quill_host::docs::VecSink;
use quill_host::HostStore;

fn main() -> ExitCode {
	let args: Vec<String> = std::env::args().collect();
	let (Some(input), Some(output)) = (args.get(1), args.get(2)) else {
		eprintln!("usage: quill-convert IN OUT.(qld|rtf|txt)");
		return ExitCode::from(2);
	};
	let bytes = match std::fs::read(input) {
		Ok(b) => b,
		Err(e) => {
			eprintln!("quill-convert: {input}: {e}");
			return ExitCode::from(1);
		}
	};
	let mut s = HostStore::new();
	let mut d = Doc::new();
	let mut src = SliceSource::new(&bytes);
	let r = if bytes.starts_with(b"RIFF") {
		quill_formats::qld::read(&mut d, &mut s, &mut src)
	} else if bytes.starts_with(b"{\\rtf") {
		quill_formats::rtf::read(&mut d, &mut s, &mut src)
	} else {
		quill_formats::text::read(&mut d, &mut s, &mut src)
	};
	if let Err(e) = r {
		eprintln!("quill-convert: {input}: {e:?}");
		return ExitCode::from(1);
	}
	let mut out = VecSink(Vec::new());
	let ext = output.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
	let w = match ext.as_str() {
		"rtf" => quill_formats::rtf::write(&d, &s, &mut out),
		"txt" => quill_formats::text::write(&d, &s, &mut out),
		_ => quill_formats::qld::write(&d, &s, &mut out),
	};
	if let Err(e) = w.and_then(|()| {
		std::fs::write(output, &out.0).map_err(|_| quill_formats::FormatError::Write)
	}) {
		eprintln!("quill-convert: {output}: {e:?}");
		return ExitCode::from(1);
	}
	ExitCode::SUCCESS
}
