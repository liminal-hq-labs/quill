// The file the display-driven smoke test must save: what typing the same keys into a `Document` and saving gives.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// `tools/smoke-edit.sh` opens a copy of `plain.qld` in the real window under Xvfb, types `LINES` at its end, saves with Ctrl+S
// and compares the file with the one this writes for the same keys. (A bare Xvfb has no window manager or portal, so the Save As
// dialog is not shown there: the test saves under the document's own name.)

use quill_fonts::FontSet;
use quill_gtk::doc::Document;
use quill_gtk::files::GuiFiles;
use quill_tty::files::Dirs;

/// What the script types, line by line (Enter between lines): Windows-1252 text with an accent.
pub const LINES: [&str; 2] = ["Hello, Quill.", "Caf\u{e9} au lait"];

/// A copy of `tests/docs/plain.qld` in `dir`, opened, with the script's text typed at its end.
fn typed(dir: &std::path::Path) -> (Document, std::path::PathBuf) {
	let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/docs/plain.qld");
	let path = dir.join("note.qld");
	std::fs::copy(src, &path).unwrap();
	let files = GuiFiles::new(Dirs {
		base: ".".into(),
		config: dir.join("cfg"),
		data: Vec::new(),
	});
	let mut doc = Document::new(FontSet::vendored(), files).unwrap();
	doc.open(&path).unwrap();
	doc.apply(quill_gtk::keys::Mapped::Move {
		to: quill_gtk::keys::Move::DocEnd,
		extend: false,
	});
	doc.type_text(&format!("\n{}", LINES.join("\n")));
	(doc, path)
}

#[test]
fn typing_then_saving_writes_a_quill_document_that_opens_again() {
	let dir = tempfile::tempdir().unwrap();
	let (mut doc, path) = typed(dir.path());
	let before = doc.read(|app, _| app.doc().body.len());
	doc.save().unwrap();
	assert!(!doc.unsaved_work());
	let mut again = Document::new(
		FontSet::vendored(),
		GuiFiles::new(Dirs {
			base: ".".into(),
			config: dir.path().join("cfg2"),
			data: Vec::new(),
		}),
	)
	.unwrap();
	again.open(&path).unwrap();
	assert_eq!(again.read(|app, _| app.doc().body.len()), before);
}

#[test]
#[ignore = "run by tools/smoke-edit.sh, which names the folder"]
fn writes_the_file_the_smoke_test_expects() {
	let dir = std::path::PathBuf::from(
		std::env::var_os("QUILL_SMOKE_DIR").expect("QUILL_SMOKE_DIR names a folder"),
	);
	let (mut doc, _) = typed(&dir);
	doc.save().unwrap();
}
