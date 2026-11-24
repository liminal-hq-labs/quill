// The quill command: golden output for the test documents, and convert against quill-convert.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn root() -> PathBuf {
	PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn docs() -> PathBuf {
	root().join("tests/docs")
}

fn quill(args: &[&str]) -> Output {
	Command::new(env!("CARGO_BIN_EXE_quill"))
		.args(args)
		.current_dir(docs())
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

/// Compares with `tests/golden/cli/NAME`, or rewrites it under `QUILL_UPDATE_GOLDEN`. Escape characters are kept as `<ESC>`.
fn golden(name: &str, actual: &str) {
	let file = root().join("tests/golden/cli").join(name);
	let shown = actual.replace('\x1b', "<ESC>");
	if std::env::var_os("QUILL_UPDATE_GOLDEN").is_some() {
		std::fs::create_dir_all(file.parent().unwrap()).unwrap();
		std::fs::write(&file, shown).unwrap();
		return;
	}
	let want = std::fs::read_to_string(&file)
		.unwrap_or_else(|_| panic!("missing golden {name}; run with QUILL_UPDATE_GOLDEN=1"));
	assert_eq!(
		want, shown,
		"{name} differs; QUILL_UPDATE_GOLDEN=1 rewrites it"
	);
}

const DOCS: [&str; 6] = [
	"letter.rtf",
	"lists.rtf",
	"headers.rtf",
	"pictures.rtf",
	"plain.qld",
	"letter.qld",
];

#[test]
fn cat_writes_the_text_of_each_test_document() {
	for d in DOCS {
		golden(&format!("{d}.cat.txt"), &stdout(&["cat", d]));
	}
}

#[test]
fn cat_ansi_shows_bold_italic_underline_and_strikethrough() {
	let text = stdout(&["cat", "--ansi", "letter.rtf"]);
	for code in ["\x1b[0;1m", "\x1b[0;3m", "\x1b[0;4m", "\x1b[0;9m"] {
		assert!(text.contains(code), "{code:?}");
	}
	golden("letter.rtf.cat-ansi.txt", &text);
	// Without the option the text is the same with nothing but the escapes taken out.
	let plain = stdout(&["cat", "letter.rtf"]);
	let mut stripped = String::new();
	let mut chars = text.chars();
	while let Some(c) = chars.next() {
		if c == '\x1b' {
			for e in chars.by_ref() {
				if e == 'm' {
					break;
				}
			}
		} else {
			stripped.push(c);
		}
	}
	assert_eq!(stripped, plain);
}

#[test]
fn info_describes_each_test_document() {
	let mut all = String::new();
	for d in DOCS {
		all.push_str(&stdout(&["info", d]));
		all.push('\n');
	}
	golden("info.txt", &all);
	// Several files in one call are separated by a blank line.
	assert_eq!(
		stdout(&["info", "letter.rtf", "lists.rtf"])
			.matches("\n\n")
			.count(),
		1
	);
}

#[test]
fn a_file_that_is_not_a_document_is_refused_with_the_editors_words() {
	let t = tempfile_path("bad.qld");
	std::fs::write(&t, b"RIFF\x04\x00\x00\x00QLD1").unwrap();
	let o = quill(&["cat", t.to_str().unwrap()]);
	assert_eq!(o.status.code(), Some(1));
	let err = String::from_utf8_lossy(&o.stderr).into_owned();
	assert!(err.contains("is not a document Quill can read"), "{err}");
	assert_eq!(quill(&["cat"]).status.code(), Some(2));
	assert_eq!(quill(&["frobnicate"]).status.code(), Some(2));
	assert_eq!(quill(&["cat", "nothing-here.rtf"]).status.code(), Some(1));
	let _ = std::fs::remove_file(&t);
}

fn tempfile_path(name: &str) -> PathBuf {
	std::env::temp_dir().join(format!("quill-cli-test-{}-{name}", std::process::id()))
}

/// The `quill-convert` binary of quill-host, built beside this test's own.
fn quill_convert() -> PathBuf {
	let exe = std::env::current_exe().unwrap();
	let target = exe.parent().and_then(Path::parent).unwrap().to_path_buf();
	let bin = target.join("quill-convert");
	let status = Command::new(env!("CARGO"))
		.args(["build", "-q", "-p", "quill-host", "--bin", "quill-convert"])
		.current_dir(root())
		.status()
		.unwrap();
	assert!(status.success());
	bin
}

#[test]
fn convert_writes_the_bytes_quill_convert_writes() {
	let old = quill_convert();
	let mut checked = 0;
	for d in DOCS {
		for ext in ["qld", "rtf", "txt"] {
			let (a, b) = (
				tempfile_path(&format!("new.{ext}")),
				tempfile_path(&format!("old.{ext}")),
			);
			let new = quill(&["convert", d, a.to_str().unwrap()]);
			assert!(new.status.success(), "{d} to {ext}");
			let was = Command::new(&old)
				.args([d, b.to_str().unwrap()])
				.current_dir(docs())
				.output()
				.unwrap();
			assert!(was.status.success());
			assert_eq!(
				std::fs::read(&a).unwrap(),
				std::fs::read(&b).unwrap(),
				"{d} to .{ext}"
			);
			let _ = std::fs::remove_file(&a);
			let _ = std::fs::remove_file(&b);
			checked += 1;
		}
	}
	assert_eq!(checked, 18);
}

#[test]
fn convert_to_standard_output_and_back_is_stable() {
	let rtf = quill(&["convert", "--to", "rtf", "letter.qld", "-"]);
	assert!(rtf.status.success());
	assert!(rtf.stdout.starts_with(b"{\\rtf"));
	// Reading that RTF and writing RTF again changes nothing: the writer's output is its own fixed point.
	let t = tempfile_path("again.rtf");
	std::fs::write(&t, &rtf.stdout).unwrap();
	let again = quill(&["convert", "--to", "rtf", t.to_str().unwrap(), "-"]);
	assert_eq!(again.stdout, rtf.stdout);
	let _ = std::fs::remove_file(&t);
	// The text a converted file gives is the text of the original.
	let via = tempfile_path("via.qld");
	assert!(quill(&["convert", "letter.rtf", via.to_str().unwrap()])
		.status
		.success());
	assert_eq!(
		stdout(&["cat", via.to_str().unwrap()]),
		stdout(&["cat", "letter.rtf"])
	);
	let _ = std::fs::remove_file(&via);
	assert_eq!(
		quill(&["convert", "--to", "pdf", "letter.rtf", "-"])
			.status
			.code(),
		Some(2)
	);
}
