// Pixel goldens: four documents drawn on the 16-colour planar surface and the 256-colour linear one, a few frames each.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod common;

use std::fmt::Write as _;

use common::*;
use quill_app::{Command, Key};
use quill_host::png::encode_indexed;
use quill_paint::page::paint_sheet;
use quill_paint::{crc32, Rect, Surface};
use quill_picture::WINDOW;
use quill_strike::StrikeMeasure;

const DOCS: [&str; 5] = [
	"letter.rtf",
	"lists.rtf",
	"headers.rtf",
	"pictures.rtf",
	"long.rtf",
];

fn update() -> bool {
	std::env::var_os("QUILL_UPDATE_GOLDEN").is_some()
}

fn compare_bytes(path: &str, actual: &[u8]) {
	let file = root().join(path);
	if update() {
		std::fs::create_dir_all(file.parent().unwrap()).unwrap();
		std::fs::write(&file, actual).unwrap();
		return;
	}
	let want = std::fs::read(&file)
		.unwrap_or_else(|_| panic!("missing golden {path}; run with QUILL_UPDATE_GOLDEN=1"));
	assert!(
		want == actual,
		"{path} differs; QUILL_UPDATE_GOLDEN=1 rewrites it and the diff is reviewed like code"
	);
}

/// One preview sheet of the app.
struct SheetJob<'a> {
	app: &'a TApp,
	s: &'a quill_host::HostStore,
	page: u32,
	at: (i32, i32),
}

impl Job for SheetJob<'_> {
	fn run<Sf: Surface>(&mut self, sf: &mut Sf) {
		let mut m = StrikeMeasure::new(strikes());
		let mut pages = 1;
		self.app
			.for_each_line(self.s, |_, l| pages = pages.max(l.page));
		let mut window = Box::new([0u8; WINDOW]);
		paint_sheet(
			sf,
			self.app,
			&mut m,
			self.s,
			self.at.0,
			self.at.1,
			self.page,
			pages,
			Rect::new(0, 0, 640, 480),
			&mut window,
		);
	}
}

fn open_named(name: &str) -> (TApp, quill_host::HostStore, quill_host::script::MemFiles) {
	if name == "long.rtf" {
		app_with(name, &long_rtf()).unwrap()
	} else {
		open_doc(name)
	}
}

fn frames_of(name: &str, mode: Mode) -> Vec<(&'static str, Frame)> {
	let (mut app, mut s, mut f) = open_named(name);
	app.set_view_height(AREA.height());
	let mut out = Vec::new();
	out.push(("top", frame(mode, &mut PageJob::new(&app, &s))));
	for _ in 0..4 {
		app.key(&mut s, &mut f, Key::PageDown);
	}
	out.push(("scrolled", frame(mode, &mut PageJob::new(&app, &s))));
	app.key(&mut s, &mut f, Key::CtrlEnd);
	out.push(("end", frame(mode, &mut PageJob::new(&app, &s))));
	app.command(&mut s, &mut f, Command::SelectAll);
	out.push(("selected", frame(mode, &mut PageJob::new(&app, &s))));
	let (sheet_app, sheet_s, _) = open_named(name);
	for (label, page, at) in [("sheet", 1, (8, 8)), ("sheet2", 2, (-190, -590))] {
		if page == 2 && name != "long.rtf" {
			continue;
		}
		out.push((
			label,
			frame(
				mode,
				&mut SheetJob {
					app: &sheet_app,
					s: &sheet_s,
					page,
					at,
				},
			),
		));
	}
	if name == "headers.rtf" {
		app.command(&mut s, &mut f, Command::View(1));
		out.push(("header", frame(mode, &mut PageJob::new(&app, &s))));
	}
	out
}

fn golden(name: &str, mode: Mode) {
	let stem = name.trim_end_matches(".rtf");
	let mut sums = String::new();
	for (n, (label, fr)) in frames_of(name, mode).into_iter().enumerate() {
		writeln!(sums, "frame {n} crc32 {:08x} {label}", crc32(&fr.native)).unwrap();
		let png = encode_indexed(640, 480, mode.depth(), &mode.palette(), &fr.pixels);
		compare_bytes(
			&format!("tests/golden/gfx/{stem}.{}.{n}.png", mode.name()),
			&png,
		);
	}
	compare_bytes(
		&format!("tests/golden/gfx/{stem}.{}.sums", mode.name()),
		sums.as_bytes(),
	);
}

#[test]
fn letter_at_16_colours() {
	golden(DOCS[0], Mode::C16);
}
#[test]
fn letter_at_256_colours() {
	golden(DOCS[0], Mode::C256);
}
#[test]
fn lists_at_16_colours() {
	golden(DOCS[1], Mode::C16);
}
#[test]
fn lists_at_256_colours() {
	golden(DOCS[1], Mode::C256);
}
#[test]
fn headers_at_16_colours() {
	golden(DOCS[2], Mode::C16);
}
#[test]
fn headers_at_256_colours() {
	golden(DOCS[2], Mode::C256);
}
#[test]
fn pictures_at_16_colours() {
	golden(DOCS[3], Mode::C16);
}
#[test]
fn pictures_at_256_colours() {
	golden(DOCS[3], Mode::C256);
}

#[test]
fn long_at_16_colours() {
	golden(DOCS[4], Mode::C16);
}
#[test]
fn long_at_256_colours() {
	golden(DOCS[4], Mode::C256);
}

#[test]
fn a_frame_is_the_same_every_time() {
	let (app, s, _) = open_doc("letter.rtf");
	let a = frame(Mode::C16, &mut PageJob::new(&app, &s));
	let b = frame(Mode::C16, &mut PageJob::new(&app, &s));
	assert!(a.native == b.native);
}

/// The first picture selected (its frame and eight handles), and the same picture inside a larger selection (a navy border).
fn selected_picture_golden(mode: Mode) {
	let (mut app, s, _) = open_doc("pictures.rtf");
	app.set_view_height(AREA.height());
	let pos = (0..app.doc().body.len())
		.find(|p| app.doc().body.object_at(&s, *p).is_some())
		.unwrap();
	app.select_picture(&s, pos);
	let alone = frame(mode, &mut PageJob::new(&app, &s));
	app.select_range(&s, 0, pos + 12);
	let inside = frame(mode, &mut PageJob::new(&app, &s));
	let mut sums = String::new();
	for (n, (label, fr)) in [("alone", alone), ("inside", inside)]
		.into_iter()
		.enumerate()
	{
		writeln!(sums, "frame {n} crc32 {:08x} {label}", crc32(&fr.native)).unwrap();
		let png = encode_indexed(640, 480, mode.depth(), &mode.palette(), &fr.pixels);
		compare_bytes(
			&format!("tests/golden/gfx/pictures-selected.{}.{n}.png", mode.name()),
			&png,
		);
	}
	compare_bytes(
		&format!("tests/golden/gfx/pictures-selected.{}.sums", mode.name()),
		sums.as_bytes(),
	);
}

#[test]
fn a_selected_picture_at_16_colours() {
	selected_picture_golden(Mode::C16);
}
#[test]
fn a_selected_picture_at_256_colours() {
	selected_picture_golden(Mode::C256);
}
