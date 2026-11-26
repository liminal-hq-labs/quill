// The end-of-session hooks: the dirty signal and the recovery copy.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use quill_app::recovery::Why;
use quill_app::{App, Key};
use quill_host::script::MemFiles;
use quill_host::HostStore;

fn typed(text: &str) -> (App, HostStore, MemFiles) {
	let mut s = HostStore::new();
	let mut f = MemFiles::default();
	let mut app = App::new();
	app.reset(&mut s).unwrap();
	for b in text.bytes() {
		app.key(&mut s, &mut f, Key::Char(b));
	}
	(app, s, f)
}

#[test]
fn unsaved_work_follows_the_modified_flag() {
	let (mut app, mut s, mut f) = typed("");
	assert!(!app.unsaved_work());
	app.key(&mut s, &mut f, Key::Char(b'a'));
	assert!(app.unsaved_work());
	assert!(app.save_as(&mut s, &mut f, b"A.QLD"));
	assert!(!app.unsaved_work());
}

#[test]
fn a_recovery_copy_leaves_the_document_as_it_was() {
	let (mut app, mut s, mut f) = typed("unsaved words");
	assert!(app.unsaved_work());
	assert!(app.save_recovery(&mut s, &mut f, Why::EndOfSession));
	assert!(app.unsaved_work(), "still unsaved, still untitled");
	assert!(app.name().is_empty());
	// A document with no name has its copy in the central folder.
	let (name, bytes) = f
		.files
		.iter()
		.find(|(n, _)| n.starts_with("RECOVER\\~Q") && n.ends_with(".RCV"))
		.expect("the copy was written");
	assert_eq!(name.len(), "RECOVER\\~Q123456.RCV".len(), "{name}");
	assert!(bytes.windows(13).any(|w| w == b"unsaved words"));
}

#[test]
fn a_clean_document_writes_no_recovery_copy() {
	let (mut app, mut s, mut f) = typed("");
	assert!(!app.save_recovery(&mut s, &mut f, Why::EndOfSession));
	assert!(f.files.is_empty());
}
