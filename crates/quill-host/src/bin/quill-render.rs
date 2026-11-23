// quill-render: reads a .QLD file and prints its layout dump.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::process::ExitCode;

use quill_core::doc::{Doc, StoryKind};
use quill_formats::io::SliceSource;
use quill_host::render::text_dump;
use quill_host::{GridMeasure, HostStore};

fn main() -> ExitCode {
	let args: Vec<String> = std::env::args().collect();
	let Some(path) = args.get(1) else {
		eprintln!("usage: quill-render FILE.QLD");
		return ExitCode::from(2);
	};
	let bytes = match std::fs::read(path) {
		Ok(b) => b,
		Err(e) => {
			eprintln!("quill-render: {path}: {e}");
			return ExitCode::from(1);
		}
	};
	let mut s = HostStore::new();
	let mut doc = Doc::new();
	if let Err(e) = quill_formats::qld::read(&mut doc, &mut s, &mut SliceSource::new(&bytes)) {
		eprintln!("quill-render: {path}: {e:?}");
		return ExitCode::from(1);
	}
	print!(
		"{}",
		text_dump(&doc, &s, &mut GridMeasure, "grid", StoryKind::Body)
	);
	ExitCode::SUCCESS
}
