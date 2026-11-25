// The sample documents shipped to users (`samples/`): they open, they hold what the pitch says, and the QLD is what the RTF converts to.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::PathBuf;
use std::process::{Command, Output};

fn samples() -> PathBuf {
	PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../samples")
}

fn quill(args: &[&str]) -> Output {
	Command::new(env!("CARGO_BIN_EXE_quill"))
		.args(args)
		.current_dir(samples())
		.output()
		.unwrap()
}

fn stdout(args: &[&str]) -> String {
	let o = quill(args);
	assert!(
		o.status.success(),
		"quill {args:?}: {}",
		String::from_utf8_lossy(&o.stderr)
	);
	String::from_utf8(o.stdout).unwrap()
}

const SETS: [&str; 3] = ["demo", "demo-reactos", "demo-reactos16"];

fn scratch(name: &str) -> PathBuf {
	std::env::temp_dir().join(format!("quill-samples-{}-{name}", std::process::id()))
}

#[test]
fn every_sample_document_opens() {
	for set in SETS {
		for ext in ["rtf", "qld"] {
			let f = format!("{set}.{ext}");
			let info = stdout(&["info", &f]);
			assert!(info.contains("Pictures    2"), "{f}: {info}");
			let text = stdout(&["cat", &f]);
			assert!(text.contains("Hi, this is Quill"), "{f}");
			assert!(text.contains("MS-DOS, Windows 3.1, Windows 95/98/Me and Linux"));
		}
	}
}

#[test]
fn the_qld_is_what_the_rtf_converts_to() {
	for set in SETS {
		let out = scratch(&format!("{set}.qld"));
		let rtf = format!("{set}.rtf");
		assert!(quill(&["convert", &rtf, out.to_str().unwrap()])
			.status
			.success());
		let want = std::fs::read(samples().join(format!("{set}.qld"))).unwrap();
		assert_eq!(
			std::fs::read(&out).unwrap(),
			want,
			"{set}.qld is out of date: run samples/make-demo.sh"
		);
		let _ = std::fs::remove_file(&out);
	}
}

#[test]
fn rtf_to_qld_and_back_loses_nothing() {
	for set in SETS {
		let qld = format!("{set}.qld");
		let back = scratch(&format!("{set}-back.rtf"));
		assert!(quill(&["convert", &qld, back.to_str().unwrap()])
			.status
			.success());
		let back_s = back.to_str().unwrap();
		// The same text, the same pictures and the same page.
		assert_eq!(
			stdout(&["cat", back_s]),
			stdout(&["cat", &format!("{set}.rtf")])
		);
		let strip = |s: String| {
			s.lines()
				.filter(|l| !l.starts_with("File "))
				.filter(|l| !l.starts_with("Size "))
				.collect::<Vec<_>>()
				.join("\n")
		};
		assert_eq!(
			strip(stdout(&["info", back_s])),
			strip(stdout(&["info", &format!("{set}.rtf")]))
		);
		// And the way round again is a fixed point: QLD to RTF to QLD to RTF writes the same RTF, pictures included.
		let again = scratch(&format!("{set}-again.qld"));
		assert!(quill(&["convert", back_s, again.to_str().unwrap()])
			.status
			.success());
		let twice = quill(&["convert", "--to", "rtf", again.to_str().unwrap(), "-"]);
		assert_eq!(
			twice.stdout,
			std::fs::read(&back).unwrap(),
			"{qld} loses something on the way round"
		);
		let _ = std::fs::remove_file(&again);
		let _ = std::fs::remove_file(&back);
	}
}
