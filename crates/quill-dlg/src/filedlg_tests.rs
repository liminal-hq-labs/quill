// Tests of the file dialog against the fake file system: scenarios for each rule in the plan, and property tests over random
// folder trees (including ones past the 512-file cap) and random key sequences, with a leak check on the store.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
#![allow(
	clippy::unwrap_used,
	clippy::expect_used,
	clippy::panic,
	clippy::indexing_slicing
)]

use crate::fake::FakeFs;
use crate::filedlg::*;
use crate::fs::{FileSystem, FsError, ATTR_DIR, ATTR_HIDDEN, ATTR_READONLY};
use crate::key::tests::any_key;
use crate::key::DlgKey;
use crate::name83::{matches_any, Name83, NameError, PathBuf, PATH_MAX};
use crate::types::{Format, Kind};
use proptest::prelude::*;
use quill_heap::HeapStore;
use std::string::String;
use std::vec::Vec;

fn tree() -> FakeFs {
	let mut fs = FakeFs::dosbox();
	fs.add_dir("C:\\DOCS\\LETTERS\\OLD");
	fs.add_dir("C:\\DOCS\\2025");
	fs.add_dir("C:\\GAMES");
	for f in [
		"APRIL.QLD",
		"BUDGET.QLD",
		"LETTER.QLD",
		"MEMO.RTF",
		"NOTES.TXT",
		"README",
	] {
		fs.add_file(&std::format!("C:\\DOCS\\{f}"), 0);
	}
	fs.add_file("C:\\DOCS\\SECRET.QLD", ATTR_HIDDEN);
	fs.add_file("C:\\DOCS\\LOCKED.QLD", ATTR_READONLY);
	fs.add_file("C:\\DOCS\\PIC.BMP", 0);
	fs.add_file("C:\\DOCS\\PIC.PNG", 0);
	fs.add_file("C:\\QUILLDOS.EXE", 0);
	fs.add_file("D:\\WORK\\TODO.TXT", 0);
	fs.set_cwd(b'C', "C:\\DOCS");
	fs.set_cwd(b'D', "D:\\WORK");
	fs
}

struct Rig {
	fs: FakeFs,
	store: HeapStore,
	dlg: FileDlg,
}

impl Rig {
	fn new(kind: Kind, folder: &str, name: &str, ty: usize) -> Rig {
		Rig::with(tree(), kind, folder, name, ty)
	}

	fn with(mut fs: FakeFs, kind: Kind, folder: &str, name: &str, ty: usize) -> Rig {
		let mut store = HeapStore::new();
		let dlg = FileDlg::new(
			kind,
			Start {
				folder: folder.as_bytes(),
				name: name.as_bytes(),
				type_index: ty,
				rows: 8,
			},
			&mut fs,
			&mut store,
		)
		.unwrap();
		Rig { fs, store, dlg }
	}

	fn key(&mut self, k: DlgKey) -> Outcome {
		self.dlg.key(k, &mut self.fs, &mut self.store)
	}

	fn type_text(&mut self, s: &str) {
		for c in s.bytes() {
			self.key(DlgKey::Char(c));
		}
	}

	/// Selects the whole field and types `s`, then presses Enter.
	fn enter_name(&mut self, s: &str) -> Outcome {
		self.dlg.focus_name_for_test();
		self.dlg.select_name_for_test();
		self.type_text(s);
		self.key(DlgKey::Enter)
	}

	fn list(&self, dirs: bool) -> Vec<String> {
		let n = if dirs {
			self.dlg.dirs().count()
		} else {
			self.dlg.files().count()
		};
		(0..n)
			.map(|i| {
				String::from_utf8(
					self.dlg
						.entry(&self.store, dirs, i)
						.unwrap()
						.name()
						.to_vec(),
				)
				.unwrap()
			})
			.collect()
	}

	fn chosen(&self) -> String {
		String::from_utf8(self.dlg.chosen().to_vec()).unwrap()
	}

	fn folder(&self) -> String {
		String::from_utf8(self.dlg.folder().to_vec()).unwrap()
	}
}

#[test]
fn the_lists_are_filtered_sorted_and_free_of_dots_and_hidden_files() {
	for (reversed, lower) in [(false, false), (true, false), (false, true), (true, true)] {
		let mut fs = tree();
		fs.reversed = reversed;
		fs.lower = lower;
		let r = Rig::with(fs, Kind::Open, "C:\\DOCS", "", 0);
		assert_eq!(
			r.list(false),
			["APRIL.QLD", "BUDGET.QLD", "LETTER.QLD", "LOCKED.QLD"]
		);
		assert_eq!(r.list(true), ["..", "2025", "LETTERS", "-C-", "-D-", "-Z-"]);
		assert!(!r.more_files_or_dirs());
		assert_eq!(r.folder(), "C:\\DOCS");
		assert_eq!(
			r.dlg.drive_letter(r.dlg.drive_box().selected().unwrap()),
			Some(b'C')
		);
	}
}

impl Rig {
	fn more_files_or_dirs(&self) -> bool {
		self.dlg.more_files() || self.dlg.more_dirs()
	}
}

#[test]
fn the_root_has_no_parent_entry() {
	let r = Rig::new(Kind::Open, "C:\\", "", 3);
	assert_eq!(r.list(true), ["DOCS", "GAMES", "-C-", "-D-", "-Z-"]);
	assert_eq!(r.list(false), ["QUILLDOS.EXE"]);
}

#[test]
fn pictures_list_both_kinds_and_a_type_change_filters_again() {
	let mut r = Rig::new(Kind::Picture, "C:\\DOCS", "", 0);
	assert_eq!(r.list(false), ["PIC.BMP", "PIC.PNG"]);
	r.dlg.focus_for_test(crate::filedlg::TYPES);
	r.key(DlgKey::Down);
	assert_eq!(r.list(false), ["PIC.BMP"]);
	r.key(DlgKey::Down);
	assert_eq!(r.list(false), ["PIC.PNG"]);
	r.key(DlgKey::Down);
	assert_eq!(r.list(false).len(), 9);
}

#[test]
fn enter_in_the_files_list_takes_the_file() {
	let mut r = Rig::new(Kind::Open, "C:\\DOCS", "", 0);
	r.dlg.focus_for_test(FILES);
	r.key(DlgKey::Down);
	assert_eq!(r.dlg.name().as_bytes(), b"BUDGET.QLD");
	assert_eq!(r.key(DlgKey::Enter), Outcome::Accept);
	assert_eq!(r.chosen(), "C:\\DOCS\\BUDGET.QLD");
	assert!(r.dlg.is_done());
	assert_eq!(r.store.live_chunks(), 0);
}

#[test]
fn a_letter_in_the_files_list_jumps_and_copies_the_name() {
	let mut r = Rig::new(Kind::Open, "C:\\DOCS", "", 0);
	r.dlg.focus_for_test(FILES);
	r.key(DlgKey::Char(b'l'));
	assert_eq!(r.dlg.name().as_bytes(), b"LETTER.QLD");
	r.key(DlgKey::Char(b'l'));
	assert_eq!(r.dlg.name().as_bytes(), b"LOCKED.QLD");
	r.key(DlgKey::Char(b'l'));
	assert_eq!(r.dlg.name().as_bytes(), b"LETTER.QLD");
}

#[test]
fn open_finds_files_by_name_and_refuses_the_missing() {
	let mut r = Rig::new(Kind::Open, "C:\\DOCS", "", 0);
	assert_eq!(r.enter_name("memo.rtf"), Outcome::Accept);
	assert_eq!(r.chosen(), "C:\\DOCS\\MEMO.RTF");

	let mut r = Rig::new(Kind::Open, "C:\\DOCS", "", 0);
	assert_eq!(r.enter_name("x.qld"), Outcome::Error(DlgError::NotFound));
	assert_eq!(r.chosen(), "C:\\DOCS\\X.QLD");
	assert!(!r.dlg.is_done());
	assert_eq!(r.store.live_chunks(), 2);

	// With no dot, Open tries the type's extension, as a default extension does.
	let mut r = Rig::new(Kind::Open, "C:\\DOCS", "", 0);
	assert_eq!(r.enter_name("budget"), Outcome::Accept);
	assert_eq!(r.chosen(), "C:\\DOCS\\BUDGET.QLD");
	// The name as typed wins when it exists.
	let mut r = Rig::new(Kind::Open, "C:\\DOCS", "", 0);
	assert_eq!(r.enter_name("readme"), Outcome::Accept);
	assert_eq!(r.chosen(), "C:\\DOCS\\README");
	// All Files has no default extension.
	let mut r = Rig::new(Kind::Open, "C:\\DOCS", "", 3);
	assert_eq!(r.enter_name("budget"), Outcome::Error(DlgError::NotFound));
}

#[test]
fn open_by_full_path_relative_path_and_drive() {
	let mut r = Rig::new(Kind::Open, "C:\\DOCS", "", 0);
	assert_eq!(r.enter_name("d:\\work\\todo.txt"), Outcome::Accept);
	assert_eq!(r.chosen(), "D:\\WORK\\TODO.TXT");
	let mut r = Rig::new(Kind::Open, "C:\\DOCS\\LETTERS", "", 0);
	assert_eq!(r.enter_name("..\\april.qld"), Outcome::Accept);
	assert_eq!(r.chosen(), "C:\\DOCS\\APRIL.QLD");
	let mut r = Rig::new(Kind::Open, "C:\\DOCS", "", 0);
	assert_eq!(r.enter_name("d:todo.txt"), Outcome::Accept);
	assert_eq!(r.chosen(), "D:\\WORK\\TODO.TXT");
}

#[test]
fn bad_names_are_refused_with_the_reason_and_nothing_is_cut_short() {
	let cases = [
		("LONGFILENAME.QLD", NameError::BaseTooLong),
		("A.LONG", NameError::ExtTooLong),
		("CON.TXT", NameError::Reserved),
		("lpt1", NameError::Reserved),
		("A.B.C", NameError::TwoDots),
		("X|Y", NameError::BadChar(b'|')),
		("..\\..\\..\\X", NameError::AboveRoot),
	];
	for kind in [Kind::Open, Kind::SaveAs] {
		for (name, why) in cases {
			let mut r = Rig::new(kind, "C:\\DOCS\\LETTERS", "", 0);
			assert_eq!(
				r.enter_name(name),
				Outcome::Error(DlgError::BadName(why)),
				"{kind:?} {name}"
			);
			assert!(!r.dlg.is_done());
		}
	}
}

#[test]
fn a_path_over_79_bytes_is_refused() {
	let mut fs = tree();
	let deep = "C:\\AAAAAAAA\\BBBBBBBB\\CCCCCCCC\\DDDDDDDD\\EEEEEEEE\\FFFFFFFF\\GGGGGGGG\\HHHHHHHH";
	fs.add_dir(deep);
	let mut r = Rig::with(fs, Kind::SaveAs, deep, "", 0);
	assert_eq!(
		r.enter_name("ABCDEFGH.QLD"),
		Outcome::Error(DlgError::BadName(NameError::PathTooLong))
	);
}

#[test]
fn save_as_starts_with_the_document_name_selected() {
	let mut r = Rig::new(Kind::SaveAs, "C:\\DOCS", "LETTER.QLD", 0);
	assert_eq!(r.dlg.name().as_bytes(), b"LETTER.QLD");
	assert_eq!(r.dlg.name().selection(), (0, 10));
	r.type_text("memo");
	assert_eq!(r.dlg.name().as_bytes(), b"MEMO");
	assert_eq!(r.dlg.focus().focused(), NAME);
}

#[test]
fn changing_the_type_keeps_the_name_and_swaps_the_extension() {
	let mut r = Rig::new(Kind::SaveAs, "C:\\DOCS", "LETTER.QLD", 0);
	r.dlg.focus_for_test(TYPES);
	r.key(DlgKey::Down);
	assert_eq!(r.dlg.name().as_bytes(), b"LETTER.RTF");
	assert_eq!(r.list(false), ["MEMO.RTF"]);
	r.key(DlgKey::Down);
	assert_eq!(r.dlg.name().as_bytes(), b"LETTER.TXT");
	assert_eq!(r.list(false), ["NOTES.TXT"]);
	r.key(DlgKey::Up);
	r.key(DlgKey::Up);
	assert_eq!(r.dlg.name().as_bytes(), b"LETTER.QLD");
	// An empty name and a wildcard are left alone.
	let mut r = Rig::new(Kind::SaveAs, "C:\\DOCS", "", 0);
	r.dlg.focus_for_test(TYPES);
	r.key(DlgKey::Down);
	assert_eq!(r.dlg.name().as_bytes(), b"");
	let mut r = Rig::new(Kind::SaveAs, "C:\\DOCS", "*.QLD", 0);
	r.dlg.focus_for_test(TYPES);
	r.key(DlgKey::Down);
	assert_eq!(r.dlg.name().as_bytes(), b"*.QLD");
}

#[test]
fn save_as_adds_the_default_extension_and_a_trailing_dot_means_none() {
	let mut r = Rig::new(Kind::SaveAs, "C:\\DOCS", "", 0);
	assert_eq!(r.enter_name("fresh"), Outcome::Accept);
	assert_eq!(r.chosen(), "C:\\DOCS\\FRESH.QLD");
	assert_eq!(r.dlg.chosen_type(), 0);
	let mut r = Rig::new(Kind::SaveAs, "C:\\DOCS", "", 1);
	assert_eq!(r.enter_name("fresh"), Outcome::Accept);
	assert_eq!(r.chosen(), "C:\\DOCS\\FRESH.RTF");
	assert_eq!(r.dlg.chosen_format(), Some(Format::Rtf));
	let mut r = Rig::new(Kind::SaveAs, "C:\\DOCS", "", 0);
	assert_eq!(r.enter_name("fresh."), Outcome::Accept);
	assert_eq!(r.chosen(), "C:\\DOCS\\FRESH");
}

#[test]
fn the_format_follows_the_type_and_not_the_extension() {
	let mut r = Rig::new(Kind::SaveAs, "C:\\DOCS", "", 1);
	assert_eq!(r.enter_name("odd.txt"), Outcome::Accept);
	assert_eq!(r.chosen(), "C:\\DOCS\\ODD.TXT");
	assert_eq!(r.dlg.chosen_format(), Some(Format::Rtf));
}

#[test]
fn an_existing_file_asks_before_it_is_replaced() {
	let mut r = Rig::new(Kind::SaveAs, "C:\\DOCS", "", 0);
	assert_eq!(r.enter_name("letter"), Outcome::Ask(Question::Overwrite));
	assert_eq!(r.chosen(), "C:\\DOCS\\LETTER.QLD");
	assert_eq!(r.dlg.asking(), Some(Question::Overwrite));
	// Keys are ignored while the question is up.
	assert_eq!(r.key(DlgKey::Esc), Outcome::None);
	assert!(!r.dlg.is_done());
	// No goes back to the dialog with the name selected.
	assert_eq!(r.dlg.answer(false, &mut r.store), Outcome::None);
	assert_eq!(r.dlg.asking(), None);
	assert_eq!(r.dlg.name().selection(), (0, 6));
	// Yes accepts.
	assert_eq!(r.key(DlgKey::Enter), Outcome::Ask(Question::Overwrite));
	assert_eq!(r.dlg.answer(true, &mut r.store), Outcome::Accept);
	assert_eq!(r.store.live_chunks(), 0);
	assert_eq!(r.dlg.answer(true, &mut r.store), Outcome::None);
}

#[test]
fn text_format_warns_and_replacing_a_text_file_asks_both() {
	let mut r = Rig::new(Kind::SaveAs, "C:\\DOCS", "", 2);
	assert_eq!(r.enter_name("fresh"), Outcome::Ask(Question::TextWarning));
	assert_eq!(r.dlg.answer(false, &mut r.store), Outcome::None);
	assert_eq!(r.key(DlgKey::Enter), Outcome::Ask(Question::TextWarning));
	assert_eq!(r.dlg.answer(true, &mut r.store), Outcome::Accept);

	let mut r = Rig::new(Kind::SaveAs, "C:\\DOCS", "", 2);
	assert_eq!(r.enter_name("notes"), Outcome::Ask(Question::Overwrite));
	assert_eq!(
		r.dlg.answer(true, &mut r.store),
		Outcome::Ask(Question::TextWarning)
	);
	assert_eq!(r.dlg.answer(true, &mut r.store), Outcome::Accept);
	assert_eq!(r.chosen(), "C:\\DOCS\\NOTES.TXT");
}

#[test]
fn a_read_only_file_is_not_replaced() {
	let mut r = Rig::new(Kind::SaveAs, "C:\\DOCS", "", 0);
	assert_eq!(r.enter_name("locked"), Outcome::Error(DlgError::ReadOnly));
	assert_eq!(r.chosen(), "C:\\DOCS\\LOCKED.QLD");
	assert!(!r.dlg.is_done());
}

#[test]
fn a_new_file_needs_its_folder() {
	let mut r = Rig::new(Kind::SaveAs, "C:\\DOCS", "", 0);
	assert_eq!(
		r.enter_name("nodir\\x.qld"),
		Outcome::Error(DlgError::NoPath)
	);
	assert_eq!(r.enter_name("\\games\\x.qld"), Outcome::Accept);
	assert_eq!(r.chosen(), "C:\\GAMES\\X.QLD");
}

#[test]
fn folders_drives_and_dotdot_change_the_folder() {
	let mut r = Rig::new(Kind::Open, "C:\\DOCS\\LETTERS", "", 3);
	assert_eq!(r.enter_name(".."), Outcome::None);
	assert_eq!(r.folder(), "C:\\DOCS");
	assert_eq!(r.dlg.name().as_bytes(), b"");
	assert_eq!(r.enter_name("letters\\"), Outcome::None);
	assert_eq!(r.folder(), "C:\\DOCS\\LETTERS");
	assert_eq!(r.enter_name("\\"), Outcome::None);
	assert_eq!(r.folder(), "C:\\");
	assert_eq!(r.enter_name("d:"), Outcome::None);
	assert_eq!(r.folder(), "D:\\WORK");
	assert_eq!(
		r.dlg.drive_letter(r.dlg.drive_box().selected().unwrap()),
		Some(b'D')
	);
	assert_eq!(r.list(false), ["TODO.TXT"]);
	assert_eq!(r.enter_name("c:\\docs"), Outcome::None);
	assert_eq!(r.folder(), "C:\\DOCS");
	assert_eq!(r.enter_name("nowhere\\"), Outcome::Error(DlgError::NoPath));
	assert_eq!(r.folder(), "C:\\DOCS");
	assert_eq!(
		r.enter_name("q:"),
		Outcome::Error(DlgError::Fs(FsError::NoPath))
	);
	assert_eq!(r.folder(), "C:\\DOCS");
	assert_eq!(r.list(false).len(), 9);
	assert_eq!(r.store.live_chunks(), 2);
}

#[test]
fn the_directories_list_opens_folders_goes_up_and_changes_drive() {
	let mut r = Rig::new(Kind::Open, "C:\\DOCS", "", 0);
	r.dlg.focus_for_test(DIRS);
	assert_eq!(r.list(true)[0], "..");
	r.key(DlgKey::Enter);
	assert_eq!(r.folder(), "C:\\");
	assert_eq!(r.list(true), ["DOCS", "GAMES", "-C-", "-D-", "-Z-"]);
	r.key(DlgKey::Down);
	r.key(DlgKey::Enter);
	assert_eq!(r.folder(), "C:\\GAMES");
	r.key(DlgKey::Home);
	r.key(DlgKey::Enter);
	r.key(DlgKey::Char(b'-'));
	r.key(DlgKey::End);
	r.key(DlgKey::Up);
	assert_eq!(
		r.dlg
			.entry(&r.store, true, r.dlg.dirs().selected().unwrap())
			.unwrap()
			.name(),
		b"-D-"
	);
	r.key(DlgKey::Enter);
	assert_eq!(r.folder(), "D:\\WORK");
}

#[test]
fn a_drive_that_is_not_ready_leaves_the_dialog_where_it_was() {
	let mut fs = tree();
	fs.not_ready = 1 << 3;
	let mut r = Rig::with(fs, Kind::Open, "C:\\DOCS", "", 0);
	r.dlg.focus_for_test(DRIVES);
	assert_eq!(
		r.key(DlgKey::Down),
		Outcome::Error(DlgError::NotReady(b'D'))
	);
	assert_eq!(r.folder(), "C:\\DOCS");
	assert_eq!(
		r.dlg.drive_letter(r.dlg.drive_box().selected().unwrap()),
		Some(b'C')
	);
	assert_eq!(r.list(false).len(), 4);
	// Typing its name gets an error too, and the dialog stays where it was.
	assert!(matches!(r.enter_name("d:\\x"), Outcome::Error(_)));
	assert_eq!(r.folder(), "C:\\DOCS");
	assert_eq!(r.store.live_chunks(), 2);
}

#[test]
fn a_list_that_failed_to_read_is_empty_and_the_next_change_reads_it_again() {
	let mut r = Rig::new(Kind::Open, "C:\\DOCS", "", 0);
	r.fs.not_ready = 1 << 2;
	r.dlg.focus_for_test(TYPES);
	assert_eq!(
		r.key(DlgKey::Down),
		Outcome::Error(DlgError::NotReady(b'C'))
	);
	assert!(r.list(false).is_empty());
	assert!(r.list(true).is_empty());
	assert_eq!(r.store.live_chunks(), 2);
	r.fs.not_ready = 0;
	assert_eq!(r.key(DlgKey::Down), Outcome::None);
	assert_eq!(r.list(false), ["NOTES.TXT"]);
}

#[test]
fn a_typed_wildcard_becomes_the_pattern_until_the_type_changes() {
	let mut r = Rig::new(Kind::Open, "C:\\DOCS", "", 0);
	assert_eq!(r.enter_name("*.txt"), Outcome::None);
	assert_eq!(r.list(false), ["NOTES.TXT"]);
	assert_eq!(r.dlg.pattern(), b"*.TXT");
	assert!(!r.dlg.is_done());
	assert_eq!(r.enter_name("*.*"), Outcome::None);
	assert_eq!(r.list(false).len(), 9);
	assert_eq!(r.enter_name("\\games\\*.exe"), Outcome::None);
	assert_eq!(r.folder(), "C:\\GAMES");
	assert_eq!(r.list(false), Vec::<String>::new());
	r.dlg.focus_for_test(TYPES);
	r.key(DlgKey::Down);
	assert_eq!(r.dlg.pattern(), b"*.rtf");
	assert_eq!(
		r.enter_name("*.*.*.*.*.*.*.*.*.*.*.*.*.*.*"),
		Outcome::Error(DlgError::BadName(NameError::BadChar(b'*')))
	);
}

#[test]
fn the_file_cap_is_512_and_a_flag_says_so() {
	let mut fs = FakeFs::dosbox();
	for i in 0..600 {
		fs.add_file(&std::format!("C:\\BIG\\F{i:03}.QLD"), 0);
	}
	for i in 0..300 {
		fs.add_dir(&std::format!("C:\\BIG\\D{i:03}"));
	}
	let r = Rig::with(fs, Kind::Open, "C:\\BIG", "", 0);
	assert_eq!(r.dlg.files().count(), FILES_MAX);
	assert!(r.dlg.more_files());
	assert!(r.dlg.more_dirs());
	assert!(r.dlg.dirs().count() <= DIRS_MAX);
	let names = r.list(false);
	assert!(names.windows(2).all(|w| w[0] < w[1]));
	// Drives and `[..]` are kept even when the folders overflow.
	let dirs = r.list(true);
	assert_eq!(dirs[0], "..");
	assert_eq!(&dirs[dirs.len() - 3..], ["-C-", "-D-", "-Z-"]);
	assert_eq!(dirs.len(), DIRS_MAX);
	assert!(r.dlg.files().overflows());
}

#[test]
fn exactly_512_files_do_not_set_the_flag() {
	let mut fs = FakeFs::dosbox();
	for i in 0..512 {
		fs.add_file(&std::format!("C:\\BIG\\F{i:03}.QLD"), 0);
	}
	let r = Rig::with(fs, Kind::Open, "C:\\BIG", "", 0);
	assert_eq!(r.dlg.files().count(), 512);
	assert!(!r.dlg.more_files());
}

#[test]
fn the_store_is_clean_after_accept_cancel_and_close() {
	let mut r = Rig::new(Kind::Open, "C:\\DOCS", "", 0);
	assert_eq!(r.store.live_chunks(), 2);
	assert_eq!(r.key(DlgKey::Esc), Outcome::Cancel);
	assert_eq!(r.store.live_chunks(), 0);
	assert!(r.dlg.entry(&r.store, false, 0).is_none());
	assert_eq!(r.key(DlgKey::Enter), Outcome::None);

	let mut r = Rig::new(Kind::Open, "C:\\DOCS", "", 0);
	r.dlg.focus_for_test(CANCEL);
	assert_eq!(r.key(DlgKey::Enter), Outcome::Cancel);
	assert_eq!(r.store.live_chunks(), 0);

	let mut r = Rig::new(Kind::Open, "C:\\DOCS", "", 0);
	r.dlg.close(&mut r.store);
	r.dlg.close(&mut r.store);
	assert_eq!(r.store.live_chunks(), 0);
}

#[test]
fn no_memory_for_the_lists_is_an_error_and_leaks_nothing() {
	for limit in 0..2 {
		let mut store = HeapStore::with_limit(limit);
		let mut fs = tree();
		let r = FileDlg::new(
			Kind::Open,
			Start {
				folder: b"C:\\DOCS",
				name: b"",
				type_index: 0,
				rows: 8,
			},
			&mut fs,
			&mut store,
		);
		assert_eq!(r.err(), Some(DlgError::NoMemory));
		assert_eq!(store.live_chunks(), 0);
	}
}

#[test]
fn a_missing_start_folder_falls_back_to_the_current_folder() {
	let r = Rig::new(Kind::Open, "C:\\GONE", "", 0);
	assert_eq!(r.folder(), "C:\\DOCS");
	let r = Rig::new(Kind::Open, "", "", 0);
	assert_eq!(r.folder(), "C:\\DOCS");
	let r = Rig::new(Kind::Open, "Q:\\X", "", 0);
	assert_eq!(r.folder(), "C:\\DOCS");
}

#[test]
fn an_unreadable_start_is_an_error_and_leaks_nothing() {
	let mut fs = tree();
	fs.not_ready = 1 << 2;
	let mut store = HeapStore::new();
	let r = FileDlg::new(
		Kind::Open,
		Start {
			folder: b"C:\\DOCS",
			name: b"",
			type_index: 0,
			rows: 8,
		},
		&mut fs,
		&mut store,
	);
	assert!(r.is_err());
	assert_eq!(store.live_chunks(), 0);
}

#[test]
fn tab_and_mnemonics_move_between_the_controls() {
	let mut r = Rig::new(Kind::Open, "C:\\DOCS", "", 0);
	r.key(DlgKey::Tab);
	assert_eq!(r.dlg.focus().focused(), FILES);
	r.key(DlgKey::Alt(b'D'));
	assert_eq!(r.dlg.focus().focused(), DIRS);
	r.key(DlgKey::BackTab);
	assert_eq!(r.dlg.focus().focused(), FILES);
	r.key(DlgKey::Alt(b'v'));
	assert_eq!(r.dlg.focus().focused(), DRIVES);
	assert_eq!(r.key(DlgKey::Alt(b'c')), Outcome::None);
}

#[test]
fn an_open_drop_down_takes_the_keys_and_esc_closes_it_first() {
	let mut r = Rig::new(Kind::Open, "C:\\DOCS", "", 0);
	r.dlg.focus_for_test(TYPES);
	r.key(DlgKey::F4);
	assert!(r.dlg.type_box().is_open());
	r.key(DlgKey::Tab);
	assert!(r.dlg.type_box().is_open());
	assert_eq!(r.key(DlgKey::Esc), Outcome::None);
	assert!(!r.dlg.type_box().is_open());
	assert!(!r.dlg.is_done());
	r.key(DlgKey::AltDown);
	r.key(DlgKey::End);
	r.key(DlgKey::Enter);
	assert_eq!(r.dlg.type_index(), 3);
	assert_eq!(r.list(false).len(), 9);
}

#[test]
fn the_dialog_stays_small() {
	assert!(
		std::mem::size_of::<FileDlg>() <= 512,
		"FileDlg is {} bytes",
		std::mem::size_of::<FileDlg>()
	);
}

#[test]
fn every_type_pattern_fits_the_pattern_buffer() {
	for k in [Kind::Open, Kind::SaveAs, Kind::Picture] {
		for t in crate::types::types(k) {
			assert!(t.patterns.len() <= PATTERN_MAX);
		}
	}
}

// ----- property tests -----

#[derive(Clone, Debug)]
enum Act {
	Key(DlgKey),
	Text(String),
	Answer(bool),
	NotReady(u8),
	Focus(u8),
}

fn act() -> impl Strategy<Value = Act> {
	prop_oneof![
		6 => any_key().prop_map(Act::Key),
		3 => "[A-Ca-c0-2.\\\\:*?-]{0,10}".prop_map(Act::Text),
		1 => any::<bool>().prop_map(Act::Answer),
		1 => (0u8..4).prop_map(Act::NotReady),
		2 => (0u8..7).prop_map(Act::Focus),
	]
}

fn name_strategy() -> impl Strategy<Value = String> {
	("[A-C]{1,3}", proptest::option::of("[A-C]{1,3}")).prop_map(|(b, e)| match e {
		Some(e) => std::format!("{b}.{e}"),
		None => b,
	})
}

/// A random tree: a handful of folders nested up to three deep, each with random files and sub-folders.
fn build(entries: &[(u8, Vec<String>, bool, u8)], big: usize, folders: usize) -> FakeFs {
	let mut fs = FakeFs::dosbox();
	let mut kinds: std::collections::HashMap<String, bool> = std::collections::HashMap::new();
	for (drive, parts, is_dir, attr) in entries {
		let d = (*b"CDZ")[usize::from(*drive) % 3] as char;
		let mut path = std::format!("{d}:");
		let mut ok = true;
		for (i, p) in parts.iter().enumerate() {
			path.push('\\');
			path.push_str(p);
			let last = i + 1 == parts.len();
			let want_dir = !last || *is_dir;
			match kinds.get(&path) {
				Some(k) if *k != want_dir => ok = false,
				_ => {
					kinds.insert(path.clone(), want_dir);
				}
			}
		}
		if ok && !parts.is_empty() {
			if *is_dir {
				fs.add_dir(&path);
			} else {
				fs.add_file(&path, *attr & 0x07);
			}
		}
	}
	for i in 0..big {
		let p = std::format!("C:\\A\\F{i:03}.QLD");
		if kinds.get("C:\\A") != Some(&false) && !kinds.contains_key(&p) {
			fs.add_file(&p, 0);
		}
	}
	for i in 0..folders {
		let p = std::format!("C:\\A\\D{i:03}");
		if kinds.get("C:\\A") != Some(&false) && !kinds.contains_key(&p) {
			fs.add_dir(&p);
		}
	}
	fs.dots = true;
	fs
}

fn check_path(p: &[u8]) {
	assert!(p.len() <= PATH_MAX, "{p:?}");
	assert!(
		p.len() >= 3 && p[0].is_ascii_uppercase() && p[1] == b':' && p[2] == b'\\',
		"{:?}",
		String::from_utf8_lossy(p)
	);
	for c in p[3..].split(|b| *b == b'\\').filter(|c| !c.is_empty()) {
		assert!(Name83::parse(c).is_ok(), "{:?}", String::from_utf8_lossy(p));
	}
	assert!(p.len() == 3 || p.last() != Some(&b'\\'));
}

/// Checks everything the dialog shows against the file system.
fn check_state(d: &FileDlg, fs: &mut FakeFs, store: &HeapStore) {
	check_path(d.folder());
	let files: Vec<Vec<u8>> = (0..d.files().count())
		.map(|i| d.entry(store, false, i).unwrap().name().to_vec())
		.collect();
	assert!(files.len() <= FILES_MAX);
	assert!(files.windows(2).all(|w| w[0] < w[1]), "files out of order");
	assert!(d.files().selected().is_none_or(|s| s < files.len()));
	let pattern = d.pattern();
	for f in &files {
		assert!(
			matches_any(pattern, f),
			"{:?} does not match {:?}",
			String::from_utf8_lossy(f),
			String::from_utf8_lossy(pattern)
		);
	}
	// Against a model of the folder.
	let folder = String::from_utf8(d.folder().to_vec()).unwrap();
	let mut model_files: Vec<Vec<u8>> = Vec::new();
	let mut model_dirs: Vec<Vec<u8>> = Vec::new();
	for (name, attr) in fs.children(&folder) {
		let up = name.to_ascii_uppercase().into_bytes();
		if attr & 0x0E != 0 || up.len() > 12 {
			continue;
		}
		if attr & ATTR_DIR != 0 {
			model_dirs.push(up);
		} else if matches_any(pattern, &up) {
			model_files.push(up);
		}
	}
	model_files.sort();
	model_dirs.sort();
	if model_files.len() <= FILES_MAX {
		assert_eq!(files, model_files);
		assert!(!d.more_files());
	} else {
		assert_eq!(files.len(), FILES_MAX);
		assert!(d.more_files());
		assert!(files.iter().all(|f| model_files.contains(f)));
	}
	let dirs: Vec<(EntryKind, Vec<u8>)> = (0..d.dirs().count())
		.map(|i| d.entry(store, true, i).unwrap())
		.map(|e| (e.kind(), e.name().to_vec()))
		.collect();
	assert!(dirs.len() <= DIRS_MAX);
	let root = d.folder().len() == 3;
	assert_eq!(dirs.first().is_some_and(|e| e.0 == EntryKind::Up), !root);
	let folders: Vec<Vec<u8>> = dirs
		.iter()
		.filter(|e| e.0 == EntryKind::Folder)
		.map(|e| e.1.clone())
		.collect();
	assert!(folders.windows(2).all(|w| w[0] < w[1]));
	if model_dirs.len() + 1 + 3 <= DIRS_MAX {
		assert_eq!(folders, model_dirs);
		assert!(!d.more_dirs());
	}
	let drives: Vec<u8> = dirs
		.iter()
		.filter(|e| e.0 == EntryKind::Drive)
		.map(|e| e.1[1])
		.collect();
	assert_eq!(drives, b"CDZ");
	// Up and folders come before drives.
	let last_non_drive = dirs
		.iter()
		.rposition(|e| e.0 != EntryKind::Drive)
		.map_or(0, |i| i + 1);
	assert!(dirs[last_non_drive..]
		.iter()
		.all(|e| e.0 == EntryKind::Drive));
	assert!(d.dirs().selected().is_none_or(|s| s < dirs.len()));
	assert!(d.type_box().selected().is_some_and(|t| t < d.types().len()));
	assert!(d.name().len() <= PATH_MAX && d.name().cursor() <= d.name().len());
	let dl = d.drive_box().selected().and_then(|i| d.drive_letter(i));
	assert_eq!(dl, d.folder().first().copied());
}

fn run(fs: FakeFs, kind: Kind, start_name: String, ty: usize, acts: Vec<Act>) {
	let mut fs = fs;
	let mut store = HeapStore::new();
	let Ok(mut d) = FileDlg::new(
		kind,
		Start {
			folder: b"C:\\A",
			name: start_name.as_bytes(),
			type_index: ty,
			rows: 8,
		},
		&mut fs,
		&mut store,
	) else {
		assert_eq!(store.live_chunks(), 0);
		return;
	};
	// Once a drive has stopped answering, a list may be empty until the next folder or type change reads it again, so the lists are
	// compared with the tree only while nothing has gone wrong.
	let mut degraded = false;
	for a in acts {
		if d.is_done() {
			break;
		}
		let mut outcomes = Vec::new();
		match a {
			Act::Key(k) => outcomes.push(d.key(k, &mut fs, &mut store)),
			Act::Text(s) => {
				for c in s.bytes() {
					outcomes.push(d.key(DlgKey::Char(c), &mut fs, &mut store));
				}
			}
			Act::Answer(y) => outcomes.push(d.answer(y, &mut store)),
			Act::NotReady(bit) => {
				fs.not_ready ^= 1 << bit.min(3);
				degraded |= fs.not_ready != 0;
			}
			Act::Focus(id) => {
				for _ in 0..7 {
					if d.focus().focused() == id {
						break;
					}
					d.key(DlgKey::Tab, &mut fs, &mut store);
				}
			}
		}
		for out in outcomes {
			match out {
				Outcome::Accept => {
					assert!(d.is_done());
					assert_eq!(store.live_chunks(), 0, "an accepted dialog leaked");
					let chosen = d.chosen().to_vec();
					check_path(&chosen);
					match kind {
						Kind::Open | Kind::Picture => {
							let attr = fs.exists(&chosen).expect("an accepted file exists");
							assert_eq!(attr & ATTR_DIR, 0);
						}
						Kind::SaveAs => {
							let part = chosen.rsplit(|b| *b == b'\\').next().unwrap();
							assert!(Name83::parse(part).is_ok());
							let p = PathBuf::from_bytes(&chosen).unwrap();
							let parent = p.parent().unwrap();
							assert!(
								parent.is_root()
									|| fs
										.exists(parent.as_bytes())
										.is_some_and(|a| a & ATTR_DIR != 0)
							);
							if let Some(a) = fs.exists(&chosen) {
								assert_eq!(
									a & (ATTR_DIR | ATTR_READONLY),
									0,
									"a save over a folder or a read-only file"
								);
							}
							assert!(d.chosen_type() < d.types().len());
						}
					}
				}
				Outcome::Cancel => {
					assert!(d.is_done());
					assert_eq!(store.live_chunks(), 0, "a cancelled dialog leaked");
				}
				Outcome::Ask(q) => {
					assert_eq!(kind, Kind::SaveAs);
					assert_eq!(d.asking(), Some(q));
				}
				_ => {}
			}
		}
		if !d.is_done() {
			assert_eq!(store.live_chunks(), 2);
			if d.asking().is_none() && !degraded {
				check_state(&d, &mut fs, &store);
			}
		}
	}
	d.close(&mut store);
	assert_eq!(store.live_chunks(), 0, "closing leaked");
}

fn entries() -> impl Strategy<Value = Vec<(u8, Vec<String>, bool, u8)>> {
	proptest::collection::vec(
		(
			0u8..3,
			proptest::collection::vec(name_strategy(), 1..4),
			any::<bool>(),
			0u8..8,
		),
		0..60,
	)
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(96))]

	#[test]
	fn random_trees_and_keys_never_break_the_dialog(
		entries in entries(),
		kind in prop_oneof![Just(Kind::Open), Just(Kind::SaveAs), Just(Kind::Picture)],
		name in "[A-C.]{0,8}",
		ty in 0usize..5,
		acts in proptest::collection::vec(act(), 0..70),
	) {
		let mut fs = build(&entries, 0, 0);
		fs.add_dir("C:\\A");
		run(fs, kind, name, ty, acts);
	}

	#[test]
	fn folders_past_the_cap_keep_every_rule(
		big in 0usize..700,
		folders in 0usize..400,
		kind in prop_oneof![Just(Kind::Open), Just(Kind::SaveAs)],
		acts in proptest::collection::vec(act(), 0..40),
	) {
		let mut fs = build(&[], big, folders);
		fs.add_dir("C:\\A");
		run(fs, kind, String::new(), 0, acts);
	}

	#[test]
	fn the_listing_order_does_not_change_the_dialog(entries in entries(), reversed in any::<bool>(), lower in any::<bool>()) {
		let mut a = build(&entries, 0, 0);
		a.add_dir("C:\\A");
		let mut b = a.clone();
		b.reversed = reversed;
		b.lower = lower;
		let mut store = HeapStore::new();
		let start = Start { folder: b"C:\\A", name: b"", type_index: 3, rows: 8 };
		let da = FileDlg::new(Kind::Open, start, &mut a, &mut store).unwrap();
		let db = FileDlg::new(Kind::Open, start, &mut b, &mut store).unwrap();
		let list = |d: &FileDlg, dirs: bool| -> Vec<Vec<u8>> {
			(0..if dirs { d.dirs().count() } else { d.files().count() })
				.map(|i| d.entry(&store, dirs, i).unwrap().name().to_vec())
				.collect()
		};
		prop_assert_eq!(list(&da, false), list(&db, false));
		prop_assert_eq!(list(&da, true), list(&db, true));
	}
}

#[test]
fn a_dialog_given_up_part_way_is_freed_by_close() {
	let mut r = Rig::new(Kind::Open, "C:\\DOCS", "", 0);
	r.dlg.focus_for_test(DIRS);
	r.key(DlgKey::Enter);
	r.key(DlgKey::F4);
	assert_eq!(r.store.live_chunks(), 2);
	r.dlg.close(&mut r.store);
	assert_eq!(r.store.live_chunks(), 0);
}
