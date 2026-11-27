// Golden tests: the committed documents, their layout dumps and the .QLD round trip.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::PathBuf;

use proptest::prelude::*;
use quill_core::doc::{Doc, StoryKind};
use quill_core::layout::layout_story;
use quill_formats::io::SliceSource;
use quill_host::docs::{self, VecSink};
use quill_host::render::text_dump;
use quill_host::{GridMeasure, HostStore};

fn root() -> PathBuf {
	PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn compare(path: &str, actual: &str) {
	let file = root().join(path);
	if std::env::var_os("QUILL_UPDATE_GOLDEN").is_some() {
		std::fs::write(&file, actual).unwrap();
		return;
	}
	let want = std::fs::read_to_string(&file)
		.unwrap_or_else(|_| panic!("missing golden {path}; run with QUILL_UPDATE_GOLDEN=1"));
	assert_eq!(
		want, actual,
		"{path} differs; QUILL_UPDATE_GOLDEN=1 rewrites it and the diff is reviewed like code"
	);
}

fn read_doc(s: &mut HostStore, bytes: &[u8]) -> Doc {
	let mut d = Doc::new();
	quill_formats::qld::read(&mut d, s, &mut SliceSource::new(bytes)).unwrap();
	d
}

#[test]
fn plain_qld_is_what_the_builder_writes() {
	let mut s = HostStore::new();
	let built = docs::plain_bytes(&mut s).unwrap();
	let committed = std::fs::read(root().join("tests/docs/plain.qld")).unwrap();
	assert_eq!(
		built, committed,
		"tests/docs/plain.qld is stale; run: cargo run -p quill-host --bin make-docs -- tests/docs"
	);
}

#[test]
fn the_test_dictionary_is_what_the_builder_writes() {
	let words = std::fs::read(root().join("dict/test-words.txt")).unwrap();
	let committed = std::fs::read(root().join("tests/docs/en_ca.dic")).unwrap();
	assert_eq!(
		docs::test_dictionary(&words),
		committed,
		"tests/docs/en_ca.dic is stale; run: cargo run -p quill-host --bin make-docs -- tests/docs"
	);
}

#[test]
fn plain_layout_matches_the_grid_golden() {
	let mut s = HostStore::new();
	let bytes = std::fs::read(root().join("tests/docs/plain.qld")).unwrap();
	let d = read_doc(&mut s, &bytes);
	let dump = text_dump(&d, &s, &mut GridMeasure, "grid", StoryKind::Body);
	compare("tests/golden/layout/plain.grid.txt", &dump);
}

#[test]
fn plain_page_map_matches_the_png_golden() {
	let mut s = HostStore::new();
	let bytes = std::fs::read(root().join("tests/docs/plain.qld")).unwrap();
	let d = read_doc(&mut s, &bytes);
	let png = quill_host::render::page_map_png(&d, &s, &mut GridMeasure, StoryKind::Body, 1, 3, 6);
	let file = root().join("tests/golden/pages/plain.grid.png");
	if std::env::var_os("QUILL_UPDATE_GOLDEN").is_some() {
		std::fs::write(&file, &png).unwrap();
		return;
	}
	assert_eq!(
		std::fs::read(&file).unwrap(),
		png,
		"tests/golden/pages/plain.grid.png differs; QUILL_UPDATE_GOLDEN=1 rewrites it"
	);
}

#[test]
fn hello_screen_matches_its_golden() {
	let mut text = String::new();
	quill_grid::hello::<25>().dump(1, &mut text).unwrap();
	compare("tests/golden/screens/hello.80x25.txt", &text);
}

#[test]
fn hello_screen_80x50_matches_its_golden() {
	let mut text = String::new();
	quill_grid::hello::<50>().dump(1, &mut text).unwrap();
	compare("tests/golden/screens/hello.80x50.txt", &text);
}

#[test]
fn qld_round_trip_is_byte_stable() {
	let mut s = HostStore::new();
	let bytes = std::fs::read(root().join("tests/docs/plain.qld")).unwrap();
	let d = read_doc(&mut s, &bytes);
	let mut out = VecSink(Vec::new());
	quill_formats::qld::write(&d, &s, &mut out).unwrap();
	assert_eq!(out.0, bytes);
}

#[test]
fn reading_is_memory_neutral() {
	let mut s = HostStore::new();
	let bytes = std::fs::read(root().join("tests/docs/plain.qld")).unwrap();
	let mut d = read_doc(&mut s, &bytes);
	d.reset(&mut s);
	assert_eq!(s.live_chunks(), 0, "reset frees every chunk");
}

#[test]
fn a_newer_major_version_is_refused() {
	let mut s = HostStore::new();
	let mut bytes = std::fs::read(root().join("tests/docs/plain.qld")).unwrap();
	// RIFF header 12 bytes, HEAD id and size 8 more, then the major version.
	bytes[20] = 2;
	let mut d = Doc::new();
	let r = quill_formats::qld::read(&mut d, &mut s, &mut SliceSource::new(&bytes));
	assert_eq!(r, Err(quill_formats::FormatError::Newer));
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(300))]

	// Every byte of damage either reads or is refused; nothing panics and no memory leaks.
	#[test]
	fn damaged_files_never_panic(flips in proptest::collection::vec((any::<u16>(), any::<u8>()), 1..8), cut in any::<u16>()) {
		let mut s = HostStore::new();
		let mut bytes = std::fs::read(root().join("tests/docs/plain.qld")).unwrap();
		for (i, v) in flips {
			let n = bytes.len();
			bytes[usize::from(i) % n] = v;
		}
		let keep = usize::from(cut) % (bytes.len() + 1);
		bytes.truncate(keep.max(bytes.len() - 8));
		let mut d = Doc::new();
		let _ = quill_formats::qld::read(&mut d, &mut s, &mut SliceSource::new(&bytes));
		d.reset(&mut s);
		prop_assert_eq!(s.live_chunks(), 0);
	}

	#[test]
	fn random_bytes_never_panic(bytes in proptest::collection::vec(any::<u8>(), 0..200)) {
		let mut s = HostStore::new();
		let mut d = Doc::new();
		let _ = quill_formats::qld::read(&mut d, &mut s, &mut SliceSource::new(&bytes));
		d.reset(&mut s);
		prop_assert_eq!(s.live_chunks(), 0);
	}

	// Lines cover the text: they never overlap, and with one-column characters none is wider than the page.
	#[test]
	fn layout_covers_the_text(text in "[a-z \\-]{0,300}") {
		let mut s = HostStore::new();
		let mut d = Doc::new();
		d.init(&mut s).unwrap();
		let mut body = text.into_bytes();
		body.push(b'\r');
		d.body.load(&mut s, &body, 0, 0).unwrap();
		let mut prev_end = 0u32;
		let mut ok = true;
		layout_story(&d, &s, &mut GridMeasure, StoryKind::Body, |l| {
			ok &= l.start == prev_end && l.width <= 60;
			prev_end = l.start + l.len;
		});
		prop_assert!(ok);
		prop_assert!(prev_end + 1 >= d.body.len(), "the last line reaches the paragraph mark");
	}
}

/// Plays a scenario file from `tests/scenarios` on the host and returns the files and the frames.
fn scenario(name: &str) -> (quill_host::script::MemFiles, String) {
	let sc = quill_host::script::parse_script(
		&std::fs::read_to_string(root().join(format!("tests/scenarios/{name}.keys"))).unwrap(),
	);
	let mut files = quill_host::script::MemFiles::default();
	quill_host::script::stage(&mut files, &sc, &root().join("tests/docs"));
	let mut frames = String::new();
	quill_host::script::play(&mut files, &sc, &mut frames);
	(files, frames)
}

/// A file the run saved: the file dialog hands the editor a full path, so the name is looked for in the root of `C:` too.
fn saved<'a>(files: &'a quill_host::script::MemFiles, name: &str) -> &'a Vec<u8> {
	files
		.files
		.get(name)
		.or_else(|| files.files.get(&format!("C:\\{name}")))
		.unwrap_or_else(|| panic!("{name} was not saved"))
}

fn compare_bytes(path: &str, actual: &[u8]) {
	let file = root().join(path);
	if std::env::var_os("QUILL_UPDATE_GOLDEN").is_some() {
		std::fs::write(&file, actual).unwrap();
		return;
	}
	assert_eq!(std::fs::read(&file).unwrap(), actual, "{path} differs");
}

#[test]
fn m8_spelling_script_matches_its_golden_screens_and_user_dictionary() {
	let (files, frames) = scenario("m8-spelling");
	compare("tests/golden/screens/m8-spelling.frames.txt", &frames);
	compare_bytes("tests/golden/files/m8-quill.udc", &files.files["QUILL.UDC"]);
}

#[test]
fn m10_spell_live_script_matches_its_golden_screens_and_marks() {
	let (_, frames) = scenario("m10-spell-live");
	compare("tests/golden/screens/m10-spell-live.frames.txt", &frames);
}

#[test]
fn m10_spell_text_script_paints_the_marks_in_the_screens_attributes() {
	let (_, frames) = scenario("m10-spell-text");
	compare("tests/golden/screens/m10-spell-text.frames.txt", &frames);
	// A misspelt word has a red background in the colour scheme; the selection shows over it.
	assert!(frames.contains("474747"), "no misspelt word was painted");
}

#[test]
fn m10_spell_text_mono_script_paints_the_marks_in_reverse() {
	let (_, frames) = scenario("m10-spell-text-mono");
	compare(
		"tests/golden/screens/m10-spell-text-mono.frames.txt",
		&frames,
	);
}

#[test]
fn m1_edit_script_matches_its_golden_screens_and_file() {
	let (files, frames) = scenario("m1-edit");
	compare("tests/golden/screens/m1-edit.frames.txt", &frames);
	compare_bytes("tests/golden/files/m1-edit.qld", saved(&files, "PLAIN.QLD"));
}

#[test]
fn m1_edit_script_on_50_rows_matches_its_golden_screens_and_the_same_file() {
	let (files, frames) = scenario("m1-edit-50");
	compare("tests/golden/screens/m1-edit-50.frames.txt", &frames);
	compare_bytes("tests/golden/files/m1-edit.qld", saved(&files, "PLAIN.QLD"));
}

#[test]
fn m16_save_as_writes_the_same_files_every_run() {
	let (files, _) = scenario("m16-save-as");
	compare_bytes(
		"tests/golden/files/m16-new.rtf",
		saved(&files, "C:\\DOCS\\NEW.RTF"),
	);
	compare_bytes(
		"tests/golden/files/m16-letter.rtf",
		saved(&files, "C:\\DOCS\\LETTER.RTF"),
	);
}

#[test]
fn m2_letter_script_matches_its_golden_screens_and_file() {
	let (files, frames) = scenario("m2-letter");
	compare("tests/golden/screens/m2-letter.frames.txt", &frames);
	compare_bytes("tests/golden/files/m2-letter.rtf", saved(&files, "OUT.RTF"));
}

#[test]
fn m2_find_script_matches_its_golden_screens() {
	let (_, frames) = scenario("m2-find");
	compare("tests/golden/screens/m2-find.frames.txt", &frames);
}

#[test]
fn m10_menus_script_matches_its_golden_screens() {
	let (_, frames) = scenario("m10-menus");
	compare("tests/golden/screens/m10-menus.frames.txt", &frames);
}

#[test]
fn m11_recent_script_matches_its_golden_screens() {
	let (_, frames) = scenario("m11-recent");
	compare("tests/golden/screens/m11-recent.frames.txt", &frames);
}

#[test]
fn m12_confirm_scripts_match_their_golden_screens() {
	for name in ["m12-confirm", "m12-confirm-mono", "m12-confirm-50"] {
		let (_, frames) = scenario(name);
		compare(&format!("tests/golden/screens/{name}.frames.txt"), &frames);
	}
}

#[test]
fn m2_lists_script_matches_its_golden_screens() {
	let (_, frames) = scenario("m2-lists");
	compare("tests/golden/screens/m2-lists.frames.txt", &frames);
}

#[test]
fn m15_picture_select_script_matches_its_golden_screens() {
	let (_, frames) = scenario("m15-picture-select");
	compare(
		"tests/golden/screens/m15-picture-select.frames.txt",
		&frames,
	);
}

#[test]
fn m14_heading_enter_script_matches_its_golden_screens() {
	let (_, frames) = scenario("m14-heading-enter");
	compare("tests/golden/screens/m14-heading-enter.frames.txt", &frames);
}

#[test]
fn m14_pagebreak_enter_script_matches_its_golden_screens() {
	let (_, frames) = scenario("m14-pagebreak-enter");
	compare(
		"tests/golden/screens/m14-pagebreak-enter.frames.txt",
		&frames,
	);
}

#[test]
fn m14_paste_undo_script_matches_its_golden_screens() {
	let (_, frames) = scenario("m14-paste-undo");
	compare("tests/golden/screens/m14-paste-undo.frames.txt", &frames);
}

#[test]
fn m14_cut_guard_script_matches_its_golden_screens() {
	let (_, frames) = scenario("m14-cut-guard");
	compare("tests/golden/screens/m14-cut-guard.frames.txt", &frames);
}

#[test]
fn m2_headers_script_matches_its_golden_screens() {
	let (_, frames) = scenario("m2-headers");
	compare("tests/golden/screens/m2-headers.frames.txt", &frames);
}

#[test]
fn m2_dialogs_script_matches_its_golden_screens() {
	let (_, frames) = scenario("m2-dialogs");
	compare("tests/golden/screens/m2-dialogs.frames.txt", &frames);
}

#[test]
fn m4_about_script_matches_its_golden_screens_and_settings() {
	let (files, frames) = scenario("m4-about");
	compare("tests/golden/screens/m4-about.frames.txt", &frames);
	compare_bytes("tests/golden/files/m4-quill.ini", &files.files["QUILL.INI"]);
}

#[test]
fn m4_print_scripts_match_their_golden_pages() {
	for (name, golden) in [
		("m4-print", "letter.print.txt"),
		("m4-print-headers", "headers.print.txt"),
	] {
		let (files, frames) = scenario(name);
		compare(&format!("tests/golden/screens/{name}.frames.txt"), &frames);
		compare_bytes(
			&format!("tests/golden/files/{golden}"),
			&files.files["OUT.PRN"],
		);
	}
}

#[test]
fn m13_clock_script_matches_its_golden_screens_and_prints_the_fields() {
	let (files, frames) = scenario("m13-clock");
	compare("tests/golden/screens/m13-clock.frames.txt", &frames);
	// The host has no clock, so the header fields print as nothing; the DOS run prints the clock's date and time.
	let prn = String::from_utf8_lossy(&files.files["OUT.PRN"]).into_owned();
	assert!(prn.contains("one"), "{prn:?}");
}

#[test]
fn m7_picture_script_matches_its_golden_screens_and_file() {
	let (files, frames) = scenario("m7-picture");
	compare("tests/golden/screens/m7-picture.frames.txt", &frames);
	compare_bytes(
		"tests/golden/files/m7-picture.qld",
		saved(&files, "OUT.QLD"),
	);
	// The original file bytes travel inside the document unchanged.
	let out = saved(&files, "OUT.QLD");
	for pic in ["pic.bmp", "pic.png"] {
		let bytes = std::fs::read(root().join(format!("tests/docs/{pic}"))).unwrap();
		assert!(
			out.windows(bytes.len()).any(|w| w == &bytes[..]),
			"{pic} is in OUT.QLD"
		);
	}
}

#[test]
fn m2_letter_in_the_mono_scheme_matches_its_golden_screens() {
	let (_, frames) = scenario("m2-mono");
	compare("tests/golden/screens/m2-mono.frames.txt", &frames);
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(100))]

	// Any keys, in any order, leave a document that ends in a paragraph mark, a caret inside it, and a screen that paints.
	#[test]
	fn random_keys_never_break_the_document(codes in proptest::collection::vec(0u8..24, 0..120)) {
		use quill_app::Key;
		let mut s = HostStore::new();
		let mut files = quill_host::script::MemFiles::default();
		files.files.insert("PLAIN.QLD".into(), std::fs::read(root().join("tests/docs/plain.qld")).unwrap());
		let mut app = quill_app::App::new();
		app.reset(&mut s).unwrap();
		let mut g = quill_grid::Grid::<25>::new(0);
		for c in codes {
			let k = match c {
				0..=7 => Key::Char(b'a' + c),
				8 => Key::Enter,
				9 => Key::Backspace,
				10 => Key::Delete,
				11 => Key::Left,
				12 => Key::Right,
				13 => Key::Up,
				14 => Key::Down,
				15 => Key::Home,
				16 => Key::End,
				17 => Key::PageDown,
				18 => Key::PageUp,
				19 => Key::Ctrl(b'z'),
				20 => Key::Ctrl(b'y'),
				21 => Key::Tab,
				22 => Key::CtrlEnd,
				_ => Key::CtrlHome,
			};
			app.key(&mut s, &mut files, k);
			quill_app::draw(&app, &s, &mut g);
			let body = &app.doc().body;
			prop_assert!(!body.is_empty());
			prop_assert_eq!(body.read_byte(&s, body.len() - 1), Some(0x0D));
		}
	}
}

#[test]
fn captured_metrics_lay_the_letter_out_as_windows_me_did() {
	// tests/golden/layout/letter.windows-me.txt is what QUILL32.EXE /LINES wrote on the Windows ME VM, with the fonts
	// ME has; the table is what /CAPTURE read there. Same widths in, so the host must break the same lines.
	let table =
		std::fs::read_to_string(root().join("tests/metrics/screen96-windows-me.txt")).unwrap();
	let mut m = quill_host::CapturedMeasure::parse(&table).unwrap();
	let mut s = HostStore::new();
	let rtf = std::fs::read(root().join("tests/docs/letter.rtf")).unwrap();
	let mut d = Doc::new();
	quill_formats::rtf::read(&mut d, &mut s, &mut SliceSource::new(&rtf)).unwrap();
	let dump = text_dump(&d, &s, &mut m, "windows-96", StoryKind::Body);
	let want =
		std::fs::read_to_string(root().join("tests/golden/layout/letter.windows-me.txt")).unwrap();
	assert_eq!(want.replace("\r\n", "\n"), dump);
}

#[test]
fn g8_mouse_script_matches_its_golden_screens() {
	let (_, frames) = scenario("g8-mouse");
	compare("tests/golden/screens/g8-mouse.frames.txt", &frames);
}
