// Tests of the file dialog against the fake file system: scenarios for each rule in the plan, and property tests over random
// folder trees (including ones past the 2,000-entry cap) and random key sequences, with a leak check on the store.
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
use crate::fs::{DriveKind, FileSystem, FsError, ATTR_DIR, ATTR_HIDDEN, ATTR_READONLY};
use crate::key::tests::any_key;
use crate::key::DlgKey;
use crate::name83::{matches_any, Name83, NameError, PathBuf, PATH_MAX};
use crate::types::{Format, Kind};
use proptest::prelude::*;
use quill_heap::HeapStore;
use std::string::{String, ToString};
use std::vec::Vec;

/// A DOS date and time, as the DTA gives them.
fn stamp(y: u16, m: u16, d: u16, hh: u16, mm: u16) -> (u16, u16) {
	(((y - 1980) << 9) | (m << 5) | d, (hh << 11) | (mm << 5))
}

fn tree() -> FakeFs {
	let mut fs = FakeFs::dosbox();
	fs.add_dir("C:\\DOCS\\LETTERS\\OLD");
	fs.add_dir("C:\\DOCS\\2025");
	fs.add_dir("C:\\GAMES");
	// Sizes and dates that put the three orders in three different sequences.
	let files: [(&str, u32, (u16, u16)); 6] = [
		("APRIL.QLD", 12_300, stamp(2026, 4, 30, 9, 5)),
		("BUDGET.QLD", 8_100, stamp(2026, 5, 2, 14, 30)),
		("LETTER.QLD", 21_000, stamp(2026, 11, 24, 18, 45)),
		("MEMO.RTF", 3_000, stamp(2025, 10, 1, 8, 0)),
		("NOTES.TXT", 900, stamp(2026, 6, 11, 12, 0)),
		("README", 100, stamp(2024, 1, 1, 0, 0)),
	];
	for (f, size, (date, time)) in files {
		fs.add_file_info(&std::format!("C:\\DOCS\\{f}"), 0, size, date, time);
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

	fn with(fs: FakeFs, kind: Kind, folder: &str, name: &str, ty: usize) -> Rig {
		let mut start = Start::new(folder.as_bytes(), name.as_bytes(), 8);
		start.type_index = ty;
		Rig::start(fs, kind, start)
	}

	fn start(mut fs: FakeFs, kind: Kind, start: Start<'_>) -> Rig {
		let mut store = HeapStore::new();
		let dlg = FileDlg::new(kind, start, &mut fs, &mut store).unwrap();
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
		self.dlg.focus_for_test(NAME);
		self.dlg.select_name_for_test();
		self.type_text(s);
		self.key(DlgKey::Enter)
	}

	fn names(&self) -> Vec<String> {
		(0..self.dlg.len())
			.map(|i| {
				String::from_utf8(self.dlg.entry(&self.store, i).unwrap().name().to_vec()).unwrap()
			})
			.collect()
	}

	fn files(&self) -> Vec<String> {
		(0..self.dlg.len())
			.map(|i| self.dlg.entry(&self.store, i).unwrap())
			.filter(|e| e.kind() == EntryKind::File)
			.map(|e| String::from_utf8(e.name().to_vec()).unwrap())
			.collect()
	}

	fn selected(&self) -> String {
		let e = self
			.dlg
			.entry(&self.store, self.dlg.list().selected().unwrap())
			.unwrap();
		String::from_utf8(e.name().to_vec()).unwrap()
	}

	fn chosen(&self) -> String {
		String::from_utf8(self.dlg.chosen().to_vec()).unwrap()
	}

	fn folder(&self) -> String {
		String::from_utf8(self.dlg.folder().to_vec()).unwrap()
	}

	fn place_labels(&self) -> Vec<String> {
		(0..self.dlg.place_count())
			.map(|i| {
				let p = self.dlg.place(&self.store, i).unwrap();
				String::from_utf8(p.label().to_vec()).unwrap()
			})
			.collect()
	}

	fn look_labels(&self) -> Vec<String> {
		(0..self.dlg.look_len())
			.map(|i| {
				let it = self.dlg.look_item(i).unwrap();
				if it.is_drive() {
					std::format!("{}:", it.letter as char)
				} else {
					std::format!(
						"{}{}",
						" ".repeat(usize::from(it.indent)),
						String::from_utf8_lossy(it.label())
					)
				}
			})
			.collect()
	}
}

/// The chunks a small folder holds: the first page and the places.
const SMALL: usize = 2;

#[test]
fn the_list_is_one_list_with_up_first_then_folders_then_matching_files() {
	for (reversed, lower) in [(false, false), (true, false), (false, true), (true, true)] {
		let mut fs = tree();
		fs.reversed = reversed;
		fs.lower = lower;
		let r = Rig::with(fs, Kind::Open, "C:\\DOCS", "", 0);
		assert_eq!(
			r.names(),
			[
				"..",
				"2025",
				"LETTERS",
				"APRIL.QLD",
				"BUDGET.QLD",
				"LETTER.QLD",
				"LOCKED.QLD"
			]
		);
		assert_eq!(r.dlg.folder_count(), 2);
		assert!(!r.dlg.more());
		assert_eq!(r.folder(), "C:\\DOCS");
		assert_eq!(r.dlg.entry(&r.store, 0).unwrap().kind(), EntryKind::Up);
		assert_eq!(r.dlg.entry(&r.store, 1).unwrap().kind(), EntryKind::Folder);
	}
}

#[test]
fn the_root_has_no_parent_entry_and_a_dot_entry_is_never_shown() {
	let r = Rig::new(Kind::Open, "C:\\", "", 3);
	assert_eq!(r.names(), ["DOCS", "GAMES", "QUILLDOS.EXE"]);
}

#[test]
fn entries_carry_their_size_and_date() {
	let r = Rig::new(Kind::Open, "C:\\DOCS", "", 0);
	let e = r.dlg.entry(&r.store, 5).unwrap();
	assert_eq!(e.name(), b"LETTER.QLD");
	assert_eq!(e.size(), 21_000);
	let (date, time) = stamp(2026, 11, 24, 18, 45);
	assert_eq!((e.date(), e.time()), (date, time));
}

#[test]
fn hidden_system_and_recovery_files_are_never_listed() {
	let mut fs = tree();
	for f in [
		"~LETT3A7.RCV",
		"~LETT3A7.RC0",
		"~LETT3A7.RCE",
		"~LETT3A7.$$$",
		"~LETT3A7.LCK",
	] {
		fs.add_file(&std::format!("C:\\DOCS\\{f}"), 0);
	}
	// A name that only looks like one of them is a real file.
	fs.add_file("C:\\DOCS\\~NOTES.TXT", 0);
	fs.add_file("C:\\DOCS\\TEMP.RCV", 0);
	let r = Rig::with(fs, Kind::Open, "C:\\DOCS", "", 3);
	let files = r.files();
	assert!(files.contains(&"~NOTES.TXT".to_string()));
	assert!(files.contains(&"TEMP.RCV".to_string()));
	assert!(!files.iter().any(|f| f.starts_with("~LETT")));
	assert!(!files.contains(&"SECRET.QLD".to_string()));
}

#[test]
fn pictures_list_both_kinds_and_a_type_change_filters_again() {
	let mut r = Rig::new(Kind::Picture, "C:\\DOCS", "", 0);
	assert_eq!(r.files(), ["PIC.BMP", "PIC.PNG"]);
	r.dlg.focus_for_test(TYPES);
	r.key(DlgKey::Down);
	assert_eq!(r.files(), ["PIC.BMP"]);
	r.key(DlgKey::Down);
	assert_eq!(r.files(), ["PIC.PNG"]);
	r.key(DlgKey::Down);
	assert_eq!(r.files().len(), 9);
}

#[test]
fn moving_onto_a_file_copies_its_name_and_a_folder_does_not() {
	let mut r = Rig::new(Kind::Open, "C:\\DOCS", "", 0);
	r.dlg.focus_for_test(LIST);
	assert_eq!(r.selected(), "..");
	r.key(DlgKey::Down);
	r.key(DlgKey::Down);
	assert_eq!(r.selected(), "LETTERS");
	assert_eq!(r.dlg.name().as_bytes(), b"");
	r.key(DlgKey::Down);
	r.key(DlgKey::Down);
	assert_eq!(r.dlg.name().as_bytes(), b"BUDGET.QLD");
	assert_eq!(r.key(DlgKey::Enter), Outcome::Accept);
	assert_eq!(r.chosen(), "C:\\DOCS\\BUDGET.QLD");
	assert!(r.dlg.is_done());
	assert_eq!(r.store.live_chunks(), 0);
}

#[test]
fn enter_on_a_folder_opens_it_and_up_selects_the_folder_it_came_from() {
	let mut r = Rig::new(Kind::Open, "C:\\DOCS", "", 0);
	r.dlg.focus_for_test(LIST);
	r.key(DlgKey::Down);
	r.key(DlgKey::Down);
	assert_eq!(r.key(DlgKey::Enter), Outcome::None);
	assert_eq!(r.folder(), "C:\\DOCS\\LETTERS");
	assert_eq!(r.names(), ["..", "OLD"]);
	assert_eq!(r.key(DlgKey::Backspace), Outcome::None);
	assert_eq!(r.folder(), "C:\\DOCS");
	assert_eq!(r.selected(), "LETTERS");
	// `..` opens the parent too, and so does the Up button.
	r.key(DlgKey::Home);
	r.key(DlgKey::Enter);
	assert_eq!(r.folder(), "C:\\");
	assert_eq!(r.selected(), "DOCS");
	r.dlg.focus_for_test(UP);
	assert_eq!(r.key(DlgKey::Enter), Outcome::None);
	assert_eq!(r.folder(), "C:\\");
}

#[test]
fn type_ahead_builds_a_prefix_cycles_one_letter_and_resets_on_ticks() {
	let mut fs = tree();
	for f in ["LAB.TXT", "LABEL.TXT", "LOG.TXT", "ZED.TXT"] {
		fs.add_file(&std::format!("C:\\DOCS\\{f}"), 0);
	}
	let mut r = Rig::with(fs, Kind::Open, "C:\\DOCS", "", 3);
	r.dlg.focus_for_test(LIST);
	// Folders count too: LETTERS comes before every file.
	r.key(DlgKey::Char(b'l'));
	assert_eq!(r.selected(), "LETTERS");
	assert_eq!(r.dlg.name().as_bytes(), b"");
	r.key(DlgKey::Char(b'a'));
	assert_eq!(r.selected(), "LAB.TXT");
	assert_eq!(r.dlg.name().as_bytes(), b"LAB.TXT");
	r.key(DlgKey::Char(b'b'));
	r.key(DlgKey::Char(b'e'));
	assert_eq!(r.selected(), "LABEL.TXT");
	assert_eq!(r.dlg.type_ahead_prefix(), b"LABE");
	// A miss leaves the prefix and the selection as they were.
	r.key(DlgKey::Char(b'q'));
	assert_eq!(r.selected(), "LABEL.TXT");
	assert_eq!(r.dlg.type_ahead_prefix(), b"LABE");
	// Nine ticks later a letter starts again.
	for _ in 0..8 {
		r.dlg.tick();
	}
	assert_eq!(r.dlg.type_ahead_prefix(), b"LABE");
	r.dlg.tick();
	assert_eq!(r.dlg.type_ahead_prefix(), b"");
	// The same letter again and again steps through the entries that start with it, wrapping.
	r.key(DlgKey::Char(b'l'));
	assert_eq!(r.selected(), "LETTERS");
	for want in [
		"LAB.TXT",
		"LABEL.TXT",
		"LETTER.QLD",
		"LOCKED.QLD",
		"LOG.TXT",
		"LETTERS",
	] {
		r.key(DlgKey::Char(b'l'));
		assert_eq!(r.selected(), want);
	}
	// Any other key ends the prefix.
	r.key(DlgKey::Down);
	assert_eq!(r.dlg.type_ahead_prefix(), b"");
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
	assert_eq!(r.store.live_chunks(), SMALL);

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
fn save_as_starts_with_the_document_name_selected_in_the_field_and_the_list() {
	let mut r = Rig::new(Kind::SaveAs, "C:\\DOCS", "LETTER.QLD", 0);
	assert_eq!(r.dlg.name().as_bytes(), b"LETTER.QLD");
	assert_eq!(r.dlg.name().selection(), (0, 10));
	assert_eq!(r.selected(), "LETTER.QLD");
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
	assert_eq!(r.files(), ["MEMO.RTF"]);
	r.key(DlgKey::Down);
	assert_eq!(r.dlg.name().as_bytes(), b"LETTER.TXT");
	assert_eq!(r.files(), ["NOTES.TXT"]);
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
fn the_format_follows_a_known_extension_and_otherwise_the_type() {
	// A known extension decides, whatever the type says.
	let mut r = Rig::new(Kind::SaveAs, "C:\\DOCS", "", 1);
	assert_eq!(r.enter_name("odd.txt"), Outcome::Accept);
	assert_eq!(r.chosen(), "C:\\DOCS\\ODD.TXT");
	assert_eq!(r.dlg.chosen_format(), Some(Format::Text));
	let mut r = Rig::new(Kind::SaveAs, "C:\\DOCS", "", 0);
	assert_eq!(r.enter_name("memo.rtf"), Ask(Question::Overwrite));
	assert_eq!(r.dlg.answer(true, &mut r.store), Outcome::Accept);
	assert_eq!(r.dlg.chosen_format(), Some(Format::Rtf));
	// An extension that is not Quill's falls back on the type.
	let mut r = Rig::new(Kind::SaveAs, "C:\\DOCS", "", 1);
	assert_eq!(r.enter_name("odd.doc"), Outcome::Accept);
	assert_eq!(r.dlg.chosen_format(), Some(Format::Rtf));
	let mut r = Rig::new(Kind::SaveAs, "C:\\DOCS", "", 2);
	assert_eq!(r.enter_name("plain."), Outcome::Accept);
	assert_eq!(r.dlg.chosen_format(), Some(Format::Text));
}

use Outcome::Ask;

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
fn saving_as_text_does_not_warn_unless_the_shell_asks_for_it() {
	let mut r = Rig::new(Kind::SaveAs, "C:\\DOCS", "", 2);
	assert_eq!(r.enter_name("fresh"), Outcome::Accept);
	let mut r = Rig::new(Kind::SaveAs, "C:\\DOCS", "", 2);
	assert_eq!(r.enter_name("notes"), Outcome::Ask(Question::Overwrite));
	assert_eq!(r.dlg.answer(true, &mut r.store), Outcome::Accept);

	let mut start = Start::new(b"C:\\DOCS", b"", 8);
	start.type_index = 2;
	start.warn_text = true;
	let mut r = Rig::start(tree(), Kind::SaveAs, start);
	assert_eq!(r.enter_name("fresh"), Outcome::Ask(Question::TextWarning));
	assert_eq!(r.dlg.answer(false, &mut r.store), Outcome::None);
	assert_eq!(r.key(DlgKey::Enter), Outcome::Ask(Question::TextWarning));
	assert_eq!(r.dlg.answer(true, &mut r.store), Outcome::Accept);
	let mut r = Rig::start(tree(), Kind::SaveAs, start);
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
fn folders_drives_and_dotdot_typed_into_the_field_change_the_folder() {
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
	assert_eq!(r.files(), ["TODO.TXT"]);
	assert_eq!(r.enter_name("c:\\docs"), Outcome::None);
	assert_eq!(r.folder(), "C:\\DOCS");
	assert_eq!(r.enter_name("nowhere\\"), Outcome::Error(DlgError::NoPath));
	assert_eq!(r.folder(), "C:\\DOCS");
	assert_eq!(
		r.enter_name("q:"),
		Outcome::Error(DlgError::Fs(FsError::NoPath))
	);
	assert_eq!(r.folder(), "C:\\DOCS");
	assert_eq!(r.files().len(), 9);
	assert_eq!(r.store.live_chunks(), SMALL);
}

#[test]
fn look_in_lists_the_drives_with_the_folders_of_the_current_one_below_it() {
	let mut r = Rig::new(Kind::Open, "C:\\DOCS\\LETTERS", "", 0);
	assert_eq!(r.look_labels(), ["C:", " DOCS", "  LETTERS", "D:", "Z:"]);
	assert_eq!(r.dlg.look_box().selected(), Some(2));
	// Choosing an ancestor goes there in one step.
	r.dlg.focus_for_test(LOOK_IN);
	r.key(DlgKey::F4);
	assert!(r.dlg.look_box().is_open());
	r.key(DlgKey::Up);
	assert_eq!(r.key(DlgKey::Enter), Outcome::None);
	assert_eq!(r.folder(), "C:\\DOCS");
	assert_eq!(r.look_labels(), ["C:", " DOCS", "D:", "Z:"]);
	// Another drive is its current folder.
	r.key(DlgKey::F4);
	r.key(DlgKey::Down);
	r.key(DlgKey::Enter);
	assert_eq!(r.folder(), "D:\\WORK");
	assert_eq!(r.look_labels(), ["C:", "D:", " WORK", "Z:"]);
	// The root of the current drive is its own item.
	r.key(DlgKey::F4);
	r.key(DlgKey::Up);
	r.key(DlgKey::Enter);
	assert_eq!(r.folder(), "D:\\");
	assert_eq!(r.dlg.look_box().selected(), Some(1));
}

#[test]
fn a_drive_that_is_not_ready_leaves_the_dialog_where_it_was() {
	let mut fs = tree();
	fs.not_ready = 1 << 3;
	let mut r = Rig::with(fs, Kind::Open, "C:\\DOCS", "", 0);
	assert_eq!(
		r.go_place_of(b'D'),
		Outcome::Error(DlgError::NotReady(b'D'))
	);
	assert_eq!(r.folder(), "C:\\DOCS");
	assert_eq!(r.dlg.look_box().selected(), Some(1));
	assert_eq!(r.files().len(), 4);
	// Typing its name gets an error too, and the dialog stays where it was.
	assert!(matches!(r.enter_name("d:\\x"), Outcome::Error(_)));
	assert_eq!(r.folder(), "C:\\DOCS");
	assert_eq!(r.store.live_chunks(), SMALL);
}

impl Rig {
	fn go_place_of(&mut self, letter: u8) -> Outcome {
		let i = (0..self.dlg.place_count())
			.find(|i| self.dlg.place(&self.store, *i).unwrap().label() == [letter, b':'])
			.unwrap();
		self.dlg.go_place(i, &mut self.fs, &mut self.store)
	}
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
	assert!(r.names().is_empty());
	assert_eq!(r.store.live_chunks(), SMALL);
	r.fs.not_ready = 0;
	assert_eq!(r.key(DlgKey::Down), Outcome::None);
	assert_eq!(r.files(), ["NOTES.TXT"]);
}

#[test]
fn a_typed_wildcard_becomes_the_pattern_until_the_type_changes() {
	let mut r = Rig::new(Kind::Open, "C:\\DOCS", "", 0);
	assert_eq!(r.enter_name("*.txt"), Outcome::None);
	assert_eq!(r.files(), ["NOTES.TXT"]);
	assert_eq!(r.dlg.pattern(), b"*.TXT");
	assert!(!r.dlg.is_done());
	assert_eq!(r.enter_name("*.*"), Outcome::None);
	assert_eq!(r.files().len(), 9);
	assert_eq!(r.enter_name("\\games\\*.exe"), Outcome::None);
	assert_eq!(r.folder(), "C:\\GAMES");
	assert_eq!(r.files(), Vec::<String>::new());
	r.dlg.focus_for_test(TYPES);
	r.key(DlgKey::Down);
	assert_eq!(r.dlg.pattern(), b"*.rtf");
	assert_eq!(
		r.enter_name("*.*.*.*.*.*.*.*.*.*.*.*.*.*.*"),
		Outcome::Error(DlgError::BadName(NameError::BadChar(b'*')))
	);
}

#[test]
fn the_cap_is_2000_entries_over_two_pages_and_a_flag_says_so() {
	let mut fs = FakeFs::dosbox();
	for i in 0..300 {
		fs.add_dir(&std::format!("C:\\BIG\\D{i:03}"));
	}
	for i in 0..1900 {
		fs.add_file(&std::format!("C:\\BIG\\F{i:04}.QLD"), 0);
	}
	let r = Rig::with(fs, Kind::Open, "C:\\BIG", "", 0);
	assert_eq!(r.dlg.len(), ENTRIES_MAX);
	assert!(r.dlg.more());
	assert_eq!(r.store.live_chunks(), 3);
	let names = r.names();
	assert_eq!(names[0], "..");
	// `..`, the 300 folders, then the files, each in order, across the page boundary at 1,024.
	assert_eq!(r.dlg.folder_count(), 300);
	assert!(names[1..301].windows(2).all(|w| w[0] < w[1]));
	assert!(names[301..].windows(2).all(|w| w[0] < w[1]));
	assert_eq!(names[1], "D000");
	assert_eq!(names[301], "F0000.QLD");
	assert_eq!(
		r.dlg.entry(&r.store, 1023).unwrap().name(),
		names[1023].as_bytes()
	);
	assert_eq!(
		r.dlg.entry(&r.store, 1024).unwrap().name(),
		names[1024].as_bytes()
	);
}

#[test]
fn exactly_2000_entries_do_not_set_the_flag_and_leaving_frees_the_second_page() {
	let mut fs = FakeFs::dosbox();
	for i in 0..1999 {
		fs.add_file(&std::format!("C:\\BIG\\F{i:04}.QLD"), 0);
	}
	fs.add_dir("C:\\SMALL");
	let mut r = Rig::with(fs, Kind::Open, "C:\\BIG", "", 0);
	// 1,999 files and `..` make exactly 2,000.
	assert_eq!(r.dlg.len(), 2000);
	assert!(!r.dlg.more());
	assert_eq!(r.store.live_chunks(), 3);
	r.enter_name("\\SMALL");
	assert_eq!(r.store.live_chunks(), SMALL);
	assert_eq!(r.dlg.len(), 1);
}

#[test]
fn sorting_by_a_column_again_reverses_it_and_keeps_the_selection_without_reading_the_disk() {
	let mut r = Rig::new(Kind::Open, "C:\\DOCS", "", 3);
	let files = |r: &Rig| r.files();
	assert_eq!(
		files(&r),
		[
			"APRIL.QLD",
			"BUDGET.QLD",
			"LETTER.QLD",
			"LOCKED.QLD",
			"MEMO.RTF",
			"NOTES.TXT",
			"PIC.BMP",
			"PIC.PNG",
			"README"
		]
	);
	let at = r.names().iter().position(|n| n == "LETTER.QLD").unwrap();
	r.dlg.select(at, &r.store);
	assert_eq!(r.selected(), "LETTER.QLD");
	let lists = r.fs.lists;
	r.key(DlgKey::Alt(b'2'));
	assert_eq!(
		r.dlg.sort(),
		Sort {
			by: SortBy::Size,
			descending: false
		}
	);
	// Folders and `..` stay first; files by size, ties by name (the unsized ones come first).
	assert_eq!(&r.names()[..3], ["..", "2025", "LETTERS"]);
	assert_eq!(
		&r.files()[..5],
		["LOCKED.QLD", "PIC.BMP", "PIC.PNG", "README", "NOTES.TXT"]
	);
	assert_eq!(r.files().last().unwrap(), "LETTER.QLD");
	assert_eq!(r.selected(), "LETTER.QLD");
	r.key(DlgKey::Alt(b'2'));
	assert!(r.dlg.sort().descending);
	assert_eq!(r.files()[0], "LETTER.QLD");
	assert_eq!(&r.names()[..3], ["..", "2025", "LETTERS"]);
	assert_eq!(r.selected(), "LETTER.QLD");
	r.key(DlgKey::Alt(b'3'));
	assert_eq!(&r.files()[..3], ["LOCKED.QLD", "PIC.BMP", "PIC.PNG"]);
	// Date: README (2024) is the oldest dated file; the undated ones sort before it.
	let dated: Vec<String> = r
		.files()
		.into_iter()
		.filter(|f| !["LOCKED.QLD", "PIC.BMP", "PIC.PNG"].contains(&f.as_str()))
		.collect();
	assert_eq!(
		dated,
		[
			"README",
			"MEMO.RTF",
			"APRIL.QLD",
			"BUDGET.QLD",
			"NOTES.TXT",
			"LETTER.QLD"
		]
	);
	r.key(DlgKey::Alt(b'1'));
	assert_eq!(r.files()[0], "APRIL.QLD");
	r.key(DlgKey::Alt(b'1'));
	assert_eq!(r.files()[0], "README");
	assert_eq!(r.fs.lists, lists, "sorting read the disk");
	// A new folder keeps the order the person chose.
	assert_eq!(
		r.dlg.sort(),
		Sort {
			by: SortBy::Name,
			descending: true
		}
	);
}

#[test]
fn places_are_the_drives_then_the_named_folders_then_the_recent_files() {
	let mut fs = tree();
	fs.set_kind(b'D', DriveKind::Removable);
	fs.set_kind(b'Z', DriveKind::CdRom);
	let folders = [
		PlaceSpec {
			label: b"Samples",
			path: b"C:\\GAMES",
		},
		PlaceSpec {
			label: b"Last used",
			path: b"C:\\DOCS\\LETTERS",
		},
	];
	let recent: [&[u8]; 2] = [b"C:\\DOCS\\LETTER.QLD", b"D:\\WORK\\TODO.TXT"];
	let mut start = Start::new(b"C:\\DOCS", b"", 8);
	start.folders = &folders;
	start.recent = &recent;
	let mut r = Rig::start(fs, Kind::Open, start);
	assert_eq!(
		r.place_labels(),
		[
			"C:",
			"D:",
			"Z:",
			"Samples",
			"Last used",
			"LETTER.QLD",
			"TODO.TXT"
		]
	);
	let kinds: Vec<PlaceKind> = (0..7)
		.map(|i| r.dlg.place(&r.store, i).unwrap().kind)
		.collect();
	assert_eq!(kinds[0], PlaceKind::Drive(b'C', DriveKind::Fixed));
	assert_eq!(kinds[1], PlaceKind::Drive(b'D', DriveKind::Removable));
	assert_eq!(kinds[2], PlaceKind::Drive(b'Z', DriveKind::CdRom));
	assert_eq!(kinds[3], PlaceKind::Folder);
	assert_eq!(kinds[6], PlaceKind::Recent);
	// A folder is one click away.
	assert_eq!(r.dlg.go_place(3, &mut r.fs, &mut r.store), Outcome::None);
	assert_eq!(r.folder(), "C:\\GAMES");
	assert_eq!(r.dlg.focus().focused(), PLACES);
	// A drive goes to the drive's own current folder.
	assert_eq!(r.dlg.go_place(1, &mut r.fs, &mut r.store), Outcome::None);
	assert_eq!(r.folder(), "D:\\WORK");
	// Enter on the focused place does the same.
	r.dlg.focus_for_test(PLACES);
	r.key(DlgKey::Home);
	assert_eq!(r.key(DlgKey::Enter), Outcome::None);
	assert_eq!(r.folder(), "C:\\DOCS");
	// A recent file is opened at once.
	assert_eq!(r.dlg.go_place(5, &mut r.fs, &mut r.store), Outcome::Accept);
	assert_eq!(r.chosen(), "C:\\DOCS\\LETTER.QLD");
	assert_eq!(r.store.live_chunks(), 0);
}

#[test]
fn a_recent_file_in_save_as_fills_the_name_and_goes_to_its_folder() {
	let recent: [&[u8]; 1] = [b"D:\\WORK\\TODO.TXT"];
	let mut start = Start::new(b"C:\\DOCS", b"", 8);
	start.recent = &recent;
	let mut r = Rig::start(tree(), Kind::SaveAs, start);
	assert_eq!(r.dlg.go_place(3, &mut r.fs, &mut r.store), Outcome::None);
	assert_eq!(r.folder(), "D:\\WORK");
	assert_eq!(r.dlg.name().as_bytes(), b"TODO.TXT");
	assert!(!r.dlg.is_done());
	// A recent file that is gone is said so and the dialog stays.
	let recent: [&[u8]; 1] = [b"C:\\DOCS\\GONE.QLD"];
	let mut start = Start::new(b"C:\\DOCS", b"", 8);
	start.recent = &recent;
	let mut r = Rig::start(tree(), Kind::Open, start);
	assert_eq!(
		r.dlg.go_place(3, &mut r.fs, &mut r.store),
		Outcome::Error(DlgError::NotFound)
	);
	assert!(!r.dlg.is_done());
}

#[test]
fn a_relative_recent_file_is_resolved_before_save_as_uses_it() {
	let mut fs = tree();
	fs.add_file("C:\\DOCS\\LETTERS\\DEMO.QLD", 0);
	let recent: [&[u8]; 1] = [b"LETTERS\\DEMO.QLD"];
	let mut start = Start::new(b"C:\\GAMES", b"", 8);
	start.recent = &recent;
	let mut r = Rig::start(fs, Kind::SaveAs, start);
	// The relative path is the program's folder (C:\DOCS here), not the dialog's.
	assert_eq!(r.dlg.go_place(3, &mut r.fs, &mut r.store), Outcome::None);
	assert_eq!(r.folder(), "C:\\DOCS\\LETTERS");
	assert_eq!(r.dlg.name().as_bytes(), b"DEMO.QLD");
	r.dlg.focus_for_test(NAME);
	assert!(matches!(
		r.key(DlgKey::Enter),
		Outcome::Ask(_) | Outcome::Accept
	));
	assert_eq!(r.dlg.chosen(), b"C:\\DOCS\\LETTERS\\DEMO.QLD");
}

#[test]
fn a_relative_recent_file_in_open_is_chosen_as_a_full_path() {
	let mut fs = tree();
	fs.add_file("C:\\DOCS\\LETTERS\\DEMO.QLD", 0);
	let recent: [&[u8]; 1] = [b"LETTERS\\DEMO.QLD"];
	let mut start = Start::new(b"C:\\GAMES", b"", 8);
	start.recent = &recent;
	let mut r = Rig::start(fs, Kind::Open, start);
	assert_eq!(r.dlg.go_place(3, &mut r.fs, &mut r.store), Outcome::Accept);
	assert_eq!(r.dlg.chosen(), b"C:\\DOCS\\LETTERS\\DEMO.QLD");
}

#[test]
fn a_recent_file_whose_folder_is_gone_leaves_save_as_as_it_was() {
	let recent: [&[u8]; 1] = [b"C:\\GONE\\DEMO.QLD"];
	let mut start = Start::new(b"C:\\DOCS", b"KEEP.QLD", 8);
	start.recent = &recent;
	let mut r = Rig::start(tree(), Kind::SaveAs, start);
	assert!(matches!(
		r.dlg.go_place(3, &mut r.fs, &mut r.store),
		Outcome::Error(_)
	));
	assert_eq!(r.folder(), "C:\\DOCS");
	assert_eq!(r.dlg.name().as_bytes(), b"KEEP.QLD");
}

#[test]
fn more_than_four_of_each_kind_of_place_are_dropped() {
	let folders: Vec<PlaceSpec<'_>> = (0..6)
		.map(|_| PlaceSpec {
			label: b"Docs",
			path: b"C:\\DOCS",
		})
		.collect();
	let recent: Vec<&[u8]> = (0..6).map(|_| &b"C:\\DOCS\\APRIL.QLD"[..]).collect();
	let mut start = Start::new(b"C:\\DOCS", b"", 8);
	start.folders = &folders;
	start.recent = &recent;
	let r = Rig::start(tree(), Kind::Open, start);
	assert_eq!(r.dlg.place_count(), 3 + 4 + 4);
	assert!(r.dlg.place(&r.store, 11).is_none());
}

#[test]
fn new_folder_makes_the_folder_and_selects_it() {
	let mut r = Rig::new(Kind::SaveAs, "C:\\DOCS", "", 0);
	r.dlg.focus_for_test(NEW_FOLDER);
	assert_eq!(r.key(DlgKey::Enter), Outcome::None);
	assert!(r.dlg.new_folder().is_some());
	// The box has the keys, and a bad name stays in it.
	r.type_text("two.long.name");
	assert_eq!(
		r.key(DlgKey::Enter),
		Outcome::Error(DlgError::BadName(NameError::TwoDots))
	);
	assert!(r.dlg.new_folder().is_some());
	for _ in 0..13 {
		r.key(DlgKey::Backspace);
	}
	r.type_text("drafts");
	assert_eq!(r.key(DlgKey::Enter), Outcome::None);
	assert!(r.dlg.new_folder().is_none());
	assert!(r.fs.exists(b"C:\\DOCS\\DRAFTS").is_some());
	assert_eq!(r.selected(), "DRAFTS");
	assert_eq!(r.dlg.folder_count(), 3);
	// Esc closes the box and not the dialog; a second New folder with the same name is refused.
	r.dlg.focus_for_test(NEW_FOLDER);
	r.key(DlgKey::Enter);
	r.type_text("drafts");
	assert_eq!(
		r.key(DlgKey::Enter),
		Outcome::Error(DlgError::Fs(FsError::Denied))
	);
	assert_eq!(r.key(DlgKey::Esc), Outcome::None);
	assert!(r.dlg.new_folder().is_none());
	assert!(!r.dlg.is_done());
	// Open has no New folder button.
	let mut r = Rig::new(Kind::Open, "C:\\DOCS", "", 0);
	assert!(!r.dlg.focus().is_enabled(NEW_FOLDER));
	r.dlg.start_new_folder();
	assert!(r.dlg.new_folder().is_none());
}

#[test]
fn a_file_system_that_cannot_make_folders_says_so() {
	struct NoMkdir(FakeFs);
	impl FileSystem for NoMkdir {
		fn list(
			&mut self,
			d: &[u8],
			e: &mut dyn FnMut(crate::fs::Found<'_>),
		) -> Result<(), FsError> {
			self.0.list(d, e)
		}
		fn drives(&mut self) -> u32 {
			self.0.drives()
		}
		fn current_dir(&mut self, d: u8, o: &mut PathBuf) -> Result<(), FsError> {
			self.0.current_dir(d, o)
		}
		fn exists(&mut self, p: &[u8]) -> Option<u8> {
			self.0.exists(p)
		}
		fn rename(&mut self, a: &[u8], b: &[u8]) -> Result<(), FsError> {
			self.0.rename(a, b)
		}
		fn remove(&mut self, p: &[u8]) -> Result<(), FsError> {
			self.0.remove(p)
		}
	}
	let mut fs = NoMkdir(tree());
	let mut store = HeapStore::new();
	let mut d = FileDlg::new(
		Kind::SaveAs,
		Start::new(b"C:\\DOCS", b"", 8),
		&mut fs,
		&mut store,
	)
	.unwrap();
	d.start_new_folder();
	for c in *b"X" {
		d.key(DlgKey::Char(c), &mut fs, &mut store);
	}
	assert_eq!(
		d.key(DlgKey::Enter, &mut fs, &mut store),
		Outcome::Error(DlgError::Fs(FsError::Unsupported))
	);
	// The default drive kind is a fixed disk.
	assert_eq!(fs.drive_kind(b'C'), DriveKind::Fixed);
}

#[test]
fn f5_reads_the_folder_again_and_keeps_the_selection() {
	let mut r = Rig::new(Kind::Open, "C:\\DOCS", "", 0);
	r.dlg.focus_for_test(LIST);
	r.key(DlgKey::Char(b'b'));
	assert_eq!(r.selected(), "BUDGET.QLD");
	let before = r.fs.lists;
	r.fs.add_file("C:\\DOCS\\ABOUT.QLD", 0);
	assert_eq!(r.key(DlgKey::F5), Outcome::None);
	assert_eq!(r.fs.lists, before + 1);
	assert!(r.files().contains(&"ABOUT.QLD".to_string()));
	assert_eq!(r.selected(), "BUDGET.QLD");
}

#[test]
fn the_store_is_clean_after_accept_cancel_and_close() {
	let mut r = Rig::new(Kind::Open, "C:\\DOCS", "", 0);
	assert_eq!(r.store.live_chunks(), SMALL);
	assert_eq!(r.key(DlgKey::Esc), Outcome::Cancel);
	assert_eq!(r.store.live_chunks(), 0);
	assert!(r.dlg.entry(&r.store, 0).is_none());
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
			Start::new(b"C:\\DOCS", b"", 8),
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
		Start::new(b"C:\\DOCS", b"", 8),
		&mut fs,
		&mut store,
	);
	assert!(r.is_err());
	assert_eq!(store.live_chunks(), 0);
}

#[test]
fn tab_walks_the_controls_in_the_order_of_the_plan_and_mnemonics_jump() {
	let mut r = Rig::new(Kind::SaveAs, "C:\\DOCS", "", 0);
	assert_eq!(r.dlg.focus().focused(), NAME);
	let mut seen = Vec::new();
	for _ in 0..9 {
		r.key(DlgKey::Tab);
		seen.push(r.dlg.focus().focused());
	}
	assert_eq!(
		seen,
		[TYPES, OK, CANCEL, LOOK_IN, UP, NEW_FOLDER, PLACES, LIST, NAME]
	);
	r.key(DlgKey::BackTab);
	assert_eq!(r.dlg.focus().focused(), LIST);
	for (c, id) in [
		(b'n', NAME),
		(b't', TYPES),
		(b'i', LOOK_IN),
		(b'p', PLACES),
		(b'l', LIST),
	] {
		r.key(DlgKey::Alt(c));
		assert_eq!(r.dlg.focus().focused(), id, "Alt+{}", c as char);
	}
	assert_eq!(r.key(DlgKey::Alt(b'f')), Outcome::None);
	assert!(r.dlg.new_folder().is_some());
	// Open skips the New folder button.
	let mut r = Rig::new(Kind::Open, "C:\\DOCS", "", 0);
	r.dlg.focus_for_test(UP);
	r.key(DlgKey::Tab);
	assert_eq!(r.dlg.focus().focused(), PLACES);
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
	assert_eq!(r.files().len(), 9);
}

#[test]
fn the_pointer_reaches_the_same_operations_as_the_keys() {
	let mut r = Rig::new(Kind::Open, "C:\\DOCS", "", 0);
	// A click selects and focuses; a file's name goes in the field.
	r.dlg.select(4, &r.store);
	assert_eq!(r.selected(), "BUDGET.QLD");
	assert_eq!(r.dlg.name().as_bytes(), b"BUDGET.QLD");
	assert_eq!(r.dlg.focus().focused(), LIST);
	// A double click on a folder opens it; on a file accepts it.
	assert_eq!(r.dlg.activate(2, &mut r.fs, &mut r.store), Outcome::None);
	assert_eq!(r.folder(), "C:\\DOCS\\LETTERS");
	assert_eq!(r.dlg.activate(0, &mut r.fs, &mut r.store), Outcome::None);
	assert_eq!(r.folder(), "C:\\DOCS");
	assert_eq!(r.dlg.activate(3, &mut r.fs, &mut r.store), Outcome::Accept);
	assert_eq!(r.chosen(), "C:\\DOCS\\APRIL.QLD");
	// Nothing reaches a dialog that has ended.
	assert_eq!(r.dlg.activate(0, &mut r.fs, &mut r.store), Outcome::None);
}

#[test]
fn scrolling_moves_the_top_and_keeps_the_selection_on_the_screen() {
	let mut fs = FakeFs::dosbox();
	for i in 0..100 {
		fs.add_file(&std::format!("C:\\BIG\\F{i:03}.QLD"), 0);
	}
	let mut r = Rig::with(fs, Kind::Open, "C:\\BIG", "", 0);
	r.dlg.scroll_to(40);
	assert_eq!(r.dlg.list().top(), 40);
	let s = r.dlg.list().selected().unwrap();
	assert!((40..48).contains(&s));
	r.dlg.scroll_to(10_000);
	assert_eq!(r.dlg.list().top(), 101 - 8);
	r.dlg.set_rows(20);
	assert_eq!(r.dlg.list().rows(), 20);
	assert_eq!(r.dlg.places().rows(), 20);
}

#[test]
fn the_dialog_stays_small() {
	assert!(
		std::mem::size_of::<FileDlg>() <= 640,
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
	Click(usize),
	Sort(u8),
}

fn act() -> impl Strategy<Value = Act> {
	prop_oneof![
		6 => any_key().prop_map(Act::Key),
		3 => "[A-Ca-c0-2.\\\\:*?-]{0,10}".prop_map(Act::Text),
		1 => any::<bool>().prop_map(Act::Answer),
		1 => (0u8..4).prop_map(Act::NotReady),
		2 => (0u8..9).prop_map(Act::Focus),
		2 => (0usize..40).prop_map(Act::Click),
		1 => (0u8..3).prop_map(Act::Sort),
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
	let entries: Vec<Entry> = (0..d.len()).map(|i| d.entry(store, i).unwrap()).collect();
	assert!(entries.len() <= ENTRIES_MAX);
	assert!(d.list().selected().is_none_or(|s| s < entries.len()));
	// `..` is first exactly when the folder is not a root, then folders, then files.
	let root = d.folder().len() == 3;
	assert_eq!(
		entries.first().is_some_and(|e| e.kind() == EntryKind::Up),
		!root
	);
	assert_eq!(
		entries.iter().filter(|e| e.kind() == EntryKind::Up).count(),
		usize::from(!root)
	);
	let rank = |e: &Entry| match e.kind() {
		EntryKind::Up => 0,
		EntryKind::Folder => 1,
		EntryKind::File => 2,
	};
	assert!(
		entries.windows(2).all(|w| rank(&w[0]) <= rank(&w[1])),
		"kinds out of order"
	);
	// Within a kind the order is the column's, ties by name, so it never depends on the listing order.
	let sort = d.sort();
	for kind in [EntryKind::Folder, EntryKind::File] {
		let of: Vec<&Entry> = entries.iter().filter(|e| e.kind() == kind).collect();
		for w in of.windows(2) {
			let key = |e: &Entry| match sort.by {
				SortBy::Name => (0u32, 0u16, 0u16),
				SortBy::Size => (e.size(), 0, 0),
				SortBy::Date => (0, e.date(), e.time()),
			};
			let (ka, kb) = (key(w[0]), key(w[1]));
			if sort.by == SortBy::Name {
				if sort.descending {
					assert!(w[0].name() > w[1].name());
				} else {
					assert!(w[0].name() < w[1].name());
				}
			} else if ka == kb {
				assert!(w[0].name() < w[1].name(), "ties by name");
			} else if sort.descending {
				assert!(ka > kb);
			} else {
				assert!(ka < kb);
			}
		}
	}
	let pattern = d.pattern();
	for e in entries.iter().filter(|e| e.kind() == EntryKind::File) {
		assert!(
			matches_any(pattern, e.name()),
			"{:?} does not match {:?}",
			String::from_utf8_lossy(e.name()),
			String::from_utf8_lossy(pattern)
		);
		assert!(!e.name().starts_with(b"~") || !e.name().ends_with(b".RCV"));
	}
	// Against a model of the folder.
	let folder = String::from_utf8(d.folder().to_vec()).unwrap();
	let mut model: Vec<(bool, Vec<u8>)> = Vec::new();
	for (name, attr, ..) in fs.children(&folder) {
		let up = name.to_ascii_uppercase().into_bytes();
		if attr & 0x0E != 0 || up.len() > 12 {
			continue;
		}
		if attr & ATTR_DIR != 0 {
			model.push((true, up));
		} else if matches_any(pattern, &up) {
			model.push((false, up));
		}
	}
	let cap = if root { ENTRIES_MAX } else { ENTRIES_MAX - 1 };
	let listed: Vec<(bool, Vec<u8>)> = entries
		.iter()
		.filter(|e| e.kind() != EntryKind::Up)
		.map(|e| (e.kind() == EntryKind::Folder, e.name().to_vec()))
		.collect();
	if model.len() <= cap {
		let mut want = model.clone();
		let mut got = listed.clone();
		want.sort();
		got.sort();
		assert_eq!(got, want);
		assert!(!d.more());
	} else {
		assert_eq!(listed.len(), cap);
		assert!(d.more());
		assert!(listed.iter().all(|e| model.contains(e)));
	}
	assert_eq!(d.folder_count(), listed.iter().filter(|e| e.0).count());
	assert!(d.type_box().selected().is_some_and(|t| t < d.types().len()));
	assert!(d.name().len() <= PATH_MAX && d.name().cursor() <= d.name().len());
	// Look in names the folder: the drives, and the folders down to this one.
	let sel = d.look_box().selected().unwrap();
	assert_eq!(d.look_item(sel).unwrap().path.as_bytes(), d.folder());
	for i in 0..d.look_len() {
		check_path(d.look_item(i).unwrap().path.as_bytes());
	}
	for i in 0..d.place_count() {
		check_path(d.place(store, i).unwrap().path.as_bytes());
	}
}

fn run(fs: FakeFs, kind: Kind, start_name: String, ty: usize, acts: Vec<Act>) {
	let mut fs = fs;
	let mut store = HeapStore::new();
	let recent: [&[u8]; 2] = [b"C:\\A\\F001.QLD", b"D:\\NOPE\\X.QLD"];
	let folders = [PlaceSpec {
		label: b"Samples",
		path: b"C:\\A",
	}];
	let mut start = Start::new(b"C:\\A", start_name.as_bytes(), 8);
	start.type_index = ty;
	start.recent = &recent;
	start.folders = &folders;
	let Ok(mut d) = FileDlg::new(kind, start, &mut fs, &mut store) else {
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
				for _ in 0..9 {
					if d.focus().focused() == id {
						break;
					}
					d.key(DlgKey::Tab, &mut fs, &mut store);
				}
			}
			Act::Click(i) => {
				if i % 2 == 0 {
					d.select(i, &store);
				} else {
					outcomes.push(d.activate(i, &mut fs, &mut store));
				}
			}
			Act::Sort(c) => d.sort_by(
				[SortBy::Name, SortBy::Size, SortBy::Date][usize::from(c) % 3],
				&mut store,
			),
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
							if !degraded {
								let attr = fs.exists(&chosen).expect("an accepted file exists");
								assert_eq!(attr & ATTR_DIR, 0);
							}
						}
						Kind::SaveAs => {
							let part = chosen.rsplit(|b| *b == b'\\').next().unwrap();
							assert!(Name83::parse(part).is_ok());
							let p = PathBuf::from_bytes(&chosen).unwrap();
							let parent = p.parent().unwrap();
							// A drive that went not ready after the answer was asked for cannot be checked.
							assert!(
								degraded
									|| parent.is_root() || fs
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
							assert!(d.chosen_format().is_some());
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
			assert!((SMALL..=3).contains(&store.live_chunks()));
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
		big in 0usize..2400,
		folders in 0usize..400,
		kind in prop_oneof![Just(Kind::Open), Just(Kind::SaveAs)],
		acts in proptest::collection::vec(act(), 0..30),
	) {
		let mut fs = build(&[], big, folders);
		fs.add_dir("C:\\A");
		run(fs, kind, String::new(), 0, acts);
	}

	#[test]
	fn the_listing_order_does_not_change_the_dialog(entries in entries(), reversed in any::<bool>(), lower in any::<bool>(), sort in 0u8..3) {
		let mut a = build(&entries, 0, 0);
		a.add_dir("C:\\A");
		let mut b = a.clone();
		b.reversed = reversed;
		b.lower = lower;
		let mut store = HeapStore::new();
		let start = Start::new(b"C:\\A", b"", 8);
		let mut start = start;
		start.type_index = 3;
		let mut da = FileDlg::new(Kind::Open, start, &mut a, &mut store).unwrap();
		let mut db = FileDlg::new(Kind::Open, start, &mut b, &mut store).unwrap();
		let by = [SortBy::Name, SortBy::Size, SortBy::Date][usize::from(sort)];
		da.sort_by(by, &mut store);
		db.sort_by(by, &mut store);
		let list = |d: &FileDlg| -> Vec<Vec<u8>> {
			(0..d.len()).map(|i| d.entry(&store, i).unwrap().name().to_vec()).collect()
		};
		prop_assert_eq!(list(&da), list(&db));
	}
}

#[test]
fn a_dialog_given_up_part_way_is_freed_by_close() {
	let mut r = Rig::new(Kind::Open, "C:\\DOCS", "", 0);
	r.dlg.focus_for_test(LIST);
	r.key(DlgKey::Down);
	r.key(DlgKey::Down);
	r.key(DlgKey::Enter);
	r.key(DlgKey::F4);
	assert_eq!(r.store.live_chunks(), SMALL);
	r.dlg.close(&mut r.store);
	assert_eq!(r.store.live_chunks(), 0);
}

impl Rig {
	fn point(&mut self, p: Point) -> Outcome {
		self.dlg.point(p, &mut self.fs, &mut self.store)
	}
}

#[test]
fn a_press_on_an_entry_selects_it_and_a_second_press_opens_it() {
	let mut r = Rig::new(Kind::Open, "C:\\DOCS", "", 0);
	// `..`, 2025, LETTERS, then the files.
	assert_eq!(r.point(Point::Item(3)), Outcome::None);
	assert_eq!(r.selected(), r.names()[3]);
	assert_eq!(r.dlg.focus().focused(), LIST);
	assert_eq!(r.dlg.name().as_bytes(), r.selected().as_bytes());
	assert_eq!(r.point(Point::Open(2)), Outcome::None);
	assert_eq!(r.folder(), "C:\\DOCS\\LETTERS");
	assert_eq!(r.point(Point::Open(0)), Outcome::None);
	assert_eq!(r.folder(), "C:\\DOCS");
	let file = r.names().iter().position(|n| n == "LETTER.QLD").unwrap();
	assert_eq!(r.point(Point::Open(file)), Outcome::Accept);
	assert_eq!(r.chosen(), "C:\\DOCS\\LETTER.QLD");
	assert_eq!(r.store.live_chunks(), 0);
}

#[test]
fn a_press_on_a_place_goes_there() {
	let mut r = Rig::new(Kind::Open, "C:\\DOCS", "", 0);
	let d = (0..r.dlg.place_count())
		.position(|i| r.dlg.place(&r.store, i).unwrap().label() == b"D:")
		.unwrap();
	assert_eq!(r.point(Point::Place(d)), Outcome::None);
	assert_eq!(r.dlg.focus().focused(), PLACES);
	assert!(r.folder().starts_with("D:"));
}

#[test]
fn a_press_on_a_header_sorts_and_again_reverses() {
	let mut r = Rig::new(Kind::Open, "C:\\DOCS", "", 0);
	r.point(Point::Header(SortBy::Size));
	assert_eq!(r.files()[0], "LOCKED.QLD");
	r.point(Point::Header(SortBy::Size));
	assert_eq!(r.files()[0], "LETTER.QLD");
}

#[test]
fn the_buttons_act_as_their_keys_do() {
	let mut r = Rig::new(Kind::SaveAs, "C:\\DOCS\\LETTERS", "A.QLD", 0);
	assert_eq!(r.point(Point::Button(UP)), Outcome::None);
	assert_eq!(r.folder(), "C:\\DOCS");
	r.point(Point::Button(NEW_FOLDER));
	assert!(r.dlg.new_folder().is_some());
	// A box is open: the dialog under it takes no pointer.
	assert_eq!(r.point(Point::Button(CANCEL)), Outcome::None);
	assert!(r.dlg.new_folder().is_some());
	r.key(DlgKey::Esc);
	// The name is a file that is not there, so OK accepts it.
	assert_eq!(r.point(Point::Button(OK)), Outcome::Accept);
}

#[test]
fn cancel_ends_the_dialog_and_frees_its_pages() {
	let mut r = Rig::new(Kind::Open, "C:\\DOCS", "", 0);
	assert_eq!(r.point(Point::Button(CANCEL)), Outcome::Cancel);
	assert_eq!(r.store.live_chunks(), 0);
}

#[test]
fn a_press_on_the_name_field_focuses_it_and_places_the_caret() {
	let mut r = Rig::new(Kind::Open, "C:\\DOCS", "ABCDEF", 0);
	r.point(Point::Name(2));
	assert_eq!(r.dlg.focus().focused(), NAME);
	assert_eq!(r.dlg.name().cursor(), 2);
	r.key(DlgKey::Char(b'x'));
	assert_eq!(r.dlg.name().as_bytes(), b"ABXCDEF");
}

#[test]
fn a_drop_down_opens_on_a_press_picks_a_row_and_shuts_on_a_press_away() {
	let mut r = Rig::new(Kind::Open, "C:\\DOCS", "", 0);
	let before = r.dlg.type_index();
	r.point(Point::Drop(TYPES));
	assert!(r.dlg.type_box().is_open());
	// Anything but a pick or a light shuts it, and does nothing else.
	r.point(Point::Item(3));
	assert!(!r.dlg.type_box().is_open());
	assert_eq!(r.dlg.type_index(), before);
	r.point(Point::Drop(TYPES));
	r.point(Point::Light(1));
	assert_eq!(r.dlg.type_box().list().selected(), Some(1));
	assert_eq!(r.dlg.type_index(), before);
	assert_eq!(r.point(Point::Pick(1)), Outcome::None);
	assert!(!r.dlg.type_box().is_open());
	assert_eq!(r.dlg.type_index(), 1);
	// The Look in list picks a folder above the current one.
	r.point(Point::Drop(LOOK_IN));
	assert!(r.dlg.look_box().is_open());
	r.point(Point::Pick(0));
	assert!(r.folder().ends_with(":\\"));
}

#[test]
fn a_press_while_a_question_is_pending_does_nothing() {
	let mut r = Rig::new(Kind::SaveAs, "C:\\DOCS", "LETTER.QLD", 0);
	assert_eq!(r.key(DlgKey::Enter), Outcome::Ask(Question::Overwrite));
	assert_eq!(r.point(Point::Button(CANCEL)), Outcome::None);
	assert_eq!(r.point(Point::Item(1)), Outcome::None);
	assert_eq!(r.dlg.asking(), Some(Question::Overwrite));
}
