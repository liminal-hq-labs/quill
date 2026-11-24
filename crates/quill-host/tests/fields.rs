// Fields: every kind goes in at the caret from the Insert menu and shows its value, and the date follows the user's short date format.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use quill_app::datefmt::format;
use quill_app::{App, Command, Key};
use quill_host::script::MemFiles;
use quill_host::HostStore;

fn date(fmt: &str, y: u32, m: u32, d: u32) -> String {
	let mut out = [0u8; 32];
	let n = format(fmt.as_bytes(), y, m, d, &mut out);
	String::from_utf8(out[..n].to_vec()).unwrap()
}

#[test]
fn short_dates_follow_the_pattern() {
	assert_eq!(date("M/d/yy", 1999, 3, 7), "3/7/99");
	assert_eq!(date("MM/dd/yyyy", 2026, 3, 7), "03/07/2026");
	assert_eq!(date("dd-MM-yy", 2026, 11, 24), "24-11-26");
	assert_eq!(date("yyyy-MM-dd", 2026, 11, 24), "2026-11-24");
	assert_eq!(date("d MMM yyyy", 2026, 11, 4), "4 Nov 2026");
	assert_eq!(date("", 2026, 1, 2), "1/2/26");
	assert_eq!(date("d.M.yy", 2005, 12, 31), "31.12.05");
	// The longest a pattern gives, which the Windows 3.x editor makes room for, is ten characters.
	assert_eq!(date("MM/dd/yyyy", 2026, 12, 31).len(), 10);
}

#[test]
fn each_field_kind_goes_in_and_shows_its_value() {
	let mut s = HostStore::new();
	let mut f = MemFiles::default();
	let mut app = App::new();
	app.reset(&mut s).unwrap();
	app.key(&mut s, &mut f, Key::Char(b'x'));
	for k in 1..=4u8 {
		app.command(&mut s, &mut f, Command::Field(k));
	}
	let body = &app.doc().body;
	let kinds: Vec<u32> = (1..=4)
		.map(|p| body.object_at(&s, p).unwrap() >> 16)
		.collect();
	assert_eq!(kinds, [1, 2, 3, 4]);
	app.set_print_date("24/11/2026");
	let mut out = [0u8; 32];
	let mut text = |k: u32, page: u32, pages: u32| {
		let n = app.field_text(k, page, pages, &mut out);
		String::from_utf8(out[..n].to_vec()).unwrap()
	};
	assert_eq!(text(1, 3, 12), "3");
	assert_eq!(text(2, 3, 12), "12");
	assert_eq!(text(3, 3, 12), "24/11/2026");
	assert_eq!(text(4, 3, 12), "(Untitled)");
}

#[test]
fn the_file_name_field_shows_the_name_without_its_path() {
	let mut s = HostStore::new();
	let mut f = MemFiles::default();
	let mut app = App::new();
	app.reset(&mut s).unwrap();
	let _ = app.save_as(&mut s, &mut f, b"LETTER.QLD");
	let mut out = [0u8; 32];
	let n = app.field_text(4, 1, 1, &mut out);
	assert_eq!(&out[..n], b"LETTER.QLD");
}
