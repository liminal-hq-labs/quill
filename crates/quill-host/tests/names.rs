// File names on a build with long names: the case the file system returned is the case the title, the status line, the recent-file list
// and the saved file keep.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Run with `cargo nextest run -p quill-host --features long-names --test names`: the Win32 build turns the same feature on.
#![cfg(feature = "long-names")]

use quill_app::{App, Key};
use quill_host::script::MemFiles;
use quill_host::HostStore;

#[test]
fn a_saved_name_keeps_its_case_everywhere() {
	let mut s = HostStore::new();
	let mut f = MemFiles::default();
	let mut app = App::new();
	app.reset(&mut s).unwrap();
	app.key(&mut s, &mut f, Key::Char(b'x'));
	assert!(app.save_as(&mut s, &mut f, b"D:\\Scott Sample.qld"));
	assert_eq!(
		app.name(),
		b"D:\\Scott Sample.qld",
		"the title and status line show the name as typed"
	);
	let recent: Vec<&[u8]> = app.recent_files().collect();
	assert_eq!(recent, [&b"D:\\Scott Sample.qld"[..]]);
	// MemFiles upper-cases names for lookup, as DOS does; what matters is that the editor asked for this exact name and wrote QLD.
	assert!(f.files.contains_key("D:\\SCOTT SAMPLE.QLD"));
	assert_eq!(
		&f.files["D:\\SCOTT SAMPLE.QLD"][..4],
		b"RIFF",
		"a lower-case extension still saves the native format"
	);
	// The same file reached again by a different case is one recent file.
	assert!(app.save_as(&mut s, &mut f, b"D:\\SCOTT SAMPLE.QLD"));
	assert_eq!(app.recent_files().count(), 1);
}
