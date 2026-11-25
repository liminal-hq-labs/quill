// Fields: every kind goes in at the caret from the Insert menu and shows its value, and the date follows the user's short date format.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use quill_app::datefmt::{dos_pattern, format, format_time};
use quill_app::{App, Command, Key};
use quill_core::doc::StoryKind;
use quill_core::layout::{walk_line, FontMetrics, FontRef, Measure, GLYPHS};
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

#[test]
fn the_dos_about_text_names_the_system_and_the_memory() {
	let mut out = [0u8; 96];
	let n = quill_app::platform::dos_about(&mut out, (6, 22), (0, 9), 65536, Some(90264));
	assert_eq!(
		std::str::from_utf8(&out[..n]).unwrap(),
		"MS-DOS, DPMI 0.9 on DOS 6.22\nFree memory: 90264 KB, largest block 65536 KB"
	);
	let n = quill_app::platform::dos_about(&mut out, (5, 0), (1, 0), 1024, None);
	assert_eq!(
		std::str::from_utf8(&out[..n]).unwrap(),
		"MS-DOS, DPMI 1.0 on DOS 5.00\nFree memory: 1024 KB free"
	);
	// A short buffer is cut, never overrun.
	let mut tiny = [0u8; 8];
	assert_eq!(
		quill_app::platform::dos_about(&mut tiny, (6, 22), (0, 9), 1, None),
		8
	);
}

fn time(h: u32, m: u32, h24: bool) -> String {
	let mut out = [0u8; 24];
	let n = format_time(h, m, h24, b"", b"", b"", &mut out);
	String::from_utf8(out[..n].to_vec()).unwrap()
}

#[test]
fn the_time_follows_the_twelve_or_twenty_four_hour_clock() {
	assert_eq!(time(0, 5, false), "12:05 AM");
	assert_eq!(time(9, 30, false), "9:30 AM");
	assert_eq!(time(12, 0, false), "12:00 PM");
	assert_eq!(time(18, 45, false), "6:45 PM");
	assert_eq!(time(9, 3, true), "09:03");
	assert_eq!(time(18, 45, true), "18:45");
	let mut out = [0u8; 24];
	let n = format_time(18, 45, false, b".", b"a.m.", b"p.m.", &mut out);
	assert_eq!(&out[..n], b"6.45 p.m.");
}

#[test]
fn a_dos_country_gives_a_date_pattern() {
	let mut p = [0u8; 10];
	let mut d = [0u8; 24];
	for (order, sep, want) in [
		(0u16, b'/', "11/24/2026"),
		(1, b'.', "24.11.2026"),
		(2, b'-', "2026-11-24"),
	] {
		let k = dos_pattern(order, sep, &mut p);
		let n = format(&p[..k], 2026, 11, 24, &mut d);
		assert_eq!(std::str::from_utf8(&d[..n]).unwrap(), want);
	}
}

/// A screen at 96 dots per inch where every character is 8 pixels wide.
#[derive(Clone)]
struct Fixed;

impl Measure for Fixed {
	fn resolution(&self) -> (u16, u16) {
		(96, 96)
	}
	fn metrics(&mut self, _: &FontRef) -> FontMetrics {
		FontMetrics {
			ascent: 12,
			descent: 4,
			leading: 0,
		}
	}
	fn widths(&mut self, _: &FontRef, out: &mut [u16; GLYPHS]) {
		out.fill(8);
	}
}

#[test]
fn a_time_field_shows_its_text_and_the_caret_lands_after_the_room_made_for_it() {
	let mut s = HostStore::new();
	let mut f = MemFiles::default();
	let mut app = App::with_measure(Fixed);
	app.reset(&mut s).unwrap();
	app.key(&mut s, &mut f, Key::Char(b'a'));
	app.set_print_date("11/24/2026");
	app.set_print_time("6:45 PM");
	let mut field_width = [0i32; 6];
	for k in [3u8, 5, 4] {
		app.command(&mut s, &mut f, Command::Field(k));
		let mut out = [0u8; 32];
		let n = app.field_text(u32::from(k), 1, 1, &mut out);
		assert!(n > 0, "field {k} has text once the shell sets it");
		app.key(&mut s, &mut f, Key::Char(b'b'));
		let mut seen = None;
		app.for_each_line(&s, |_, line| {
			walk_line(
				app.doc(),
				&s,
				&mut Fixed,
				StoryKind::Body,
				line,
				|p, c, x, w, _, _| {
					if c == 0x01
						&& app.doc().body.object_at(&s, p).map(|v| v >> 16) == Some(u32::from(k))
					{
						seen = Some((x, w));
					}
				},
			);
		});
		let (_, w) = seen.expect("the field is on a line");
		field_width[k as usize] = w;
		// The room the layout makes is at least the width the painter needs for the text it draws.
		assert!(w >= 8 * n as i32, "field {k}: room {w} for {n} characters");
	}
	// The Time field reserves eight digits, and "6:45 PM" is seven characters.
	assert_eq!(field_width[5], 64);
}

#[test]
fn a_time_field_without_the_clock_has_no_text() {
	let (mut app, mut s, mut f) = (App::new(), HostStore::new(), MemFiles::default());
	app.reset(&mut s).unwrap();
	app.command(&mut s, &mut f, Command::Field(5));
	let mut out = [0u8; 32];
	assert_eq!(app.field_text(5, 1, 1, &mut out), 0);
}
