// The viewer without a window: every test document opens, its layout equals the host's goldens, its pages draw at fractional
// scales to the same bytes every time, and the settings come back.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![allow(clippy::chunks_exact_to_as_chunks)]

use std::path::{Path, PathBuf};

use quill_fonts::FontSet;
use quill_gtk::doc::{Document, Overlay, Theme};
use quill_gtk::files::GuiFiles;
use quill_gtk::view::{Geometry, View, Zoom};
use quill_host::png::encode_rgb;
use quill_tty::files::Dirs;

fn root() -> PathBuf {
	PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn docs(name: &str) -> PathBuf {
	root().join("tests/docs").join(name)
}

fn document(config: &Path) -> Document {
	let files = GuiFiles::new(Dirs {
		base: ".".into(),
		config: config.to_path_buf(),
		data: Vec::new(),
	});
	Document::new(FontSet::vendored(), files).unwrap()
}

fn compare_bytes(path: &str, actual: &[u8]) {
	let file = root().join(path);
	if std::env::var_os("QUILL_UPDATE_GOLDEN").is_some() {
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

fn view_of(doc: &Document, w: i32, h: i32, scale_120: u32, zoom: u32) -> View {
	let mut v = View::new(Zoom::Percent(zoom));
	let m = quill_gtk::view::Mapping::new(zoom, scale_120);
	let (dw, dh) = m.device_size(w, h);
	v.set_viewport(dw, dh, scale_120);
	v.set_geometry(doc.geometry());
	v
}

fn rgb(buf: &quill_paint::pixel::PixelBuffer) -> Vec<u8> {
	buf.bytes()
		.chunks_exact(4)
		.flat_map(|p| [p[0], p[1], p[2]])
		.collect()
}

/// The documents of `tests/docs` the editor opens: the native and rich-text files.
const OPENABLE: [&str; 7] = [
	"letter.qld",
	"plain.qld",
	"letter.rtf",
	"lists.rtf",
	"headers.rtf",
	"pictures.rtf",
	"print3.rtf",
];

#[test]
fn every_document_in_tests_docs_opens_and_draws() {
	let cfg = tempfile::tempdir().unwrap();
	let mut doc = document(cfg.path());
	let mut names: Vec<String> = std::fs::read_dir(root().join("tests/docs"))
		.unwrap()
		.map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
		.filter(|n| n.ends_with(".rtf") || n.ends_with(".qld"))
		.collect();
	names.sort();
	assert!(names.len() >= OPENABLE.len(), "{names:?}");
	for name in &names {
		doc.open(&docs(name))
			.unwrap_or_else(|e| panic!("{name}: {e}"));
		assert_eq!(doc.title(), format!("{name} \u{2013} Quill"));
		assert!(doc.geometry().pages >= 1);
		for scale_120 in [120, 150, 180] {
			let v = view_of(&doc, 640, 420, scale_120, 100);
			let buf = doc.render(&v, Theme::LIGHT, &Overlay::default());
			let (dw, dh) = v.device();
			assert_eq!(
				(buf.width(), buf.height()),
				(dw, dh),
				"{name} at {scale_120}"
			);
			// The sheet is white and there is ink on it.
			let px = buf.bytes();
			assert!(
				px.chunks_exact(4).any(|p| p == [255, 255, 255, 255]),
				"{name}"
			);
			assert!(
				px.chunks_exact(4).any(|p| p[0] < 100 && p[3] == 255),
				"{name}"
			);
		}
	}
}

#[test]
fn a_file_that_is_not_there_or_not_a_document_leaves_the_open_one_alone() {
	let cfg = tempfile::tempdir().unwrap();
	let mut doc = document(cfg.path());
	doc.open(&docs("letter.rtf")).unwrap();
	let rev = doc.revision();
	let err = doc.open(&docs("missing.rtf")).unwrap_err();
	assert_eq!(err, "missing.rtf could not be found.");
	assert_eq!(doc.name(), Some("letter.rtf"));
	assert_eq!(doc.revision(), rev);
}

#[test]
fn the_layout_equals_the_hosts_dumps_at_1440_to_the_inch() {
	let cfg = tempfile::tempdir().unwrap();
	let mut doc = document(cfg.path());
	for name in [
		"letter.rtf",
		"lists.rtf",
		"headers.rtf",
		"pictures.rtf",
		"plain.qld",
	] {
		doc.open(&docs(name)).unwrap();
		let stem = name.split('.').next().unwrap();
		let want = std::fs::read_to_string(
			root().join(format!("tests/golden/layout/{stem}.outline-1440.txt")),
		)
		.unwrap();
		assert_eq!(doc.dump_layout(1440, "outline"), want, "{name}");
	}
}

#[test]
fn a_page_draws_to_the_same_bytes_at_a_scale_of_one_and_of_one_and_a_half() {
	let cfg = tempfile::tempdir().unwrap();
	let mut doc = document(cfg.path());
	doc.open(&docs("letter.rtf")).unwrap();
	for (scale_120, file) in [(120, "1.00"), (180, "1.50")] {
		let v = view_of(&doc, 640, 420, scale_120, 100);
		let first = doc.render(&v, Theme::LIGHT, &Overlay::default());
		let again = doc.render(&v, Theme::LIGHT, &Overlay::default());
		assert!(first.bytes() == again.bytes());
		let (w, h) = (first.width(), first.height());
		compare_bytes(
			&format!("tests/golden/gtk/letter.{file}.png"),
			&encode_rgb(w, h, &rgb(&first)),
		);
	}
}

#[test]
fn a_zoomed_page_on_a_dark_desk_keeps_white_paper() {
	let cfg = tempfile::tempdir().unwrap();
	let mut doc = document(cfg.path());
	doc.open(&docs("lists.rtf")).unwrap();
	let v = view_of(&doc, 640, 420, 150, 125);
	let buf = doc.render(&v, Theme::DARK, &Overlay::default());
	let px = buf.bytes();
	assert_eq!(&px[..4], &[46, 46, 50, 255], "the corner is the dark desk");
	let mid = ((buf.height() / 2) * buf.width() + buf.width() / 2) as usize * 4;
	assert_eq!(
		&px[mid..mid + 4],
		&[255, 255, 255, 255],
		"the sheet is white"
	);
	let (w, h) = (buf.width(), buf.height());
	compare_bytes(
		"tests/golden/gtk/lists.1.25.zoom125.dark.png",
		&encode_rgb(w, h, &rgb(&buf)),
	);
}

#[test]
fn scrolling_to_the_next_sheet_draws_it_and_changes_the_pixels() {
	let cfg = tempfile::tempdir().unwrap();
	let mut doc = document(cfg.path());
	doc.open(&docs("print3.rtf")).unwrap();
	let g: Geometry = doc.geometry();
	assert!(g.pages >= 2, "print3.rtf has {} sheets", g.pages);
	let mut v = view_of(&doc, 640, 420, 120, 100);
	let top = doc
		.render(&v, Theme::LIGHT, &Overlay::default())
		.bytes()
		.to_vec();
	v.scroll_to(0, g.page_top(1) - 300);
	assert_eq!(v.current_page(), 1);
	let second = doc
		.render(&v, Theme::LIGHT, &Overlay::default())
		.bytes()
		.to_vec();
	assert_ne!(top, second);
	v.to_edge(true);
	assert_eq!(v.current_page(), g.pages);
}

#[test]
fn the_recent_files_and_the_zoom_come_back_in_the_next_session() {
	let cfg = tempfile::tempdir().unwrap();
	let mut doc = document(cfg.path());
	doc.open(&docs("letter.rtf")).unwrap();
	doc.open(&docs("lists.rtf")).unwrap();
	doc.remember_zoom(Zoom::Percent(150));
	doc.save_settings();
	let next = document(cfg.path());
	let recent = next.recent();
	assert_eq!(
		recent.iter().map(|r| r.label.as_str()).collect::<Vec<_>>(),
		["lists.rtf", "letter.rtf"]
	);
	assert!(recent[0].path.is_absolute() && recent[0].path.is_file());
	assert_eq!(next.saved_zoom(), Zoom::Percent(150));
	let ini = std::fs::read_to_string(cfg.path().join("QUILL.INI")).unwrap();
	assert!(
		ini.contains("[Recent32]") && ini.contains("Zoom=150"),
		"{ini}"
	);
}

#[test]
fn a_document_with_a_name_windows_1252_lacks_opens_by_its_long_name() {
	let cfg = tempfile::tempdir().unwrap();
	let mut doc = document(cfg.path());
	let dir = tempfile::tempdir().unwrap();
	let path = dir.path().join("\u{4e2d}\u{6587} letter.rtf");
	std::fs::copy(docs("letter.rtf"), &path).unwrap();
	doc.open(&path).unwrap();
	assert_eq!(doc.title(), "\u{4e2d}\u{6587} letter.rtf \u{2013} Quill");
}
