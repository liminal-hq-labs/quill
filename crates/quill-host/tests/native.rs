// Native dialogs: with them on, the editor asks its shell for each dialog and never draws one in text cells or opens a text menu.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use quill_app::flow::{
	words, Answer, Buttons, FindFields, Notice, NoticeKind, SpellButton, SpellStep, TabButton,
};
use quill_app::{App, Command, Dialog, Files, FindAction, Key};
use quill_grid::Grid;
use quill_host::script::{MemFiles, MemSource, NamedSink};
use quill_host::HostStore;

fn setup(text: &str) -> (App, HostStore, MemFiles) {
	let mut s = HostStore::new();
	let mut f = MemFiles::default();
	let mut app = App::new();
	app.reset(&mut s).unwrap();
	app.set_native_dialogs(true);
	for b in text.bytes() {
		app.key(&mut s, &mut f, Key::Char(b));
	}
	(app, s, f)
}

/// True when the screen holds a box-drawing corner: what the text-cell dialogs and menus are made of.
fn has_box(app: &App, s: &HostStore) -> bool {
	let mut g = Grid::<25>::new(0);
	quill_app::draw(app, s, &mut g);
	(0..25).any(|y| {
		(0..80).any(|x| {
			matches!(
				g.get(x, y).map(|c| c.ch),
				Some(0xDA | 0xC9 | 0xBF | 0xBB | 0xC0 | 0xC8)
			)
		})
	})
}

#[test]
fn every_dialog_command_becomes_a_request_and_the_page_stays_clean() {
	let (mut app, mut s, mut f) = setup("hello");
	app.discard_changes();
	for (c, want) in [
		(Command::Open, Dialog::Open),
		(Command::SaveAs, Dialog::SaveAs),
		(Command::Save, Dialog::SaveAs),
		(Command::Picture, Dialog::Picture),
		(Command::Print, Dialog::Print),
		(Command::Fonts, Dialog::Fonts),
		(Command::Tabs, Dialog::Tabs),
		(Command::About, Dialog::About),
		(Command::Find, Dialog::Find(false)),
		(Command::Replace, Dialog::Find(true)),
	] {
		app.command(&mut s, &mut f, c);
		assert_eq!(app.take_dialog(), Some(want), "{c:?}");
		assert_eq!(app.take_dialog(), None, "a request is taken once");
		assert!(!has_box(&app, &s), "{c:?} drew a text-cell box");
	}
	// The shortcuts and F10 reach the same place or nowhere.
	app.key(&mut s, &mut f, Key::Ctrl(b'o'));
	assert_eq!(app.take_dialog(), Some(Dialog::Open));
	app.key(&mut s, &mut f, Key::F(10));
	assert!(!has_box(&app, &s), "F10 opened a text menu");
	assert_eq!(app.take_dialog(), None);
}

#[test]
fn unsaved_changes_ask_before_new_open_and_exit_and_then_go_on() {
	let (mut app, mut s, mut f) = setup("some text");
	assert!(app.modified());
	app.command(&mut s, &mut f, Command::New);
	assert_eq!(app.take_dialog(), Some(Dialog::ConfirmSave(Command::New)));
	assert!(!has_box(&app, &s));
	// Don't save: the shell discards and runs the command.
	app.discard_changes();
	app.run_confirmed(&mut s, &mut f, Command::New);
	assert_eq!(app.take_dialog(), None);
	let mut b = [0u8; 8];
	assert_eq!(
		app.doc().body.read(&s, 0, &mut b),
		1,
		"the new document is empty"
	);
	app.key(&mut s, &mut f, Key::Char(b'x'));
	app.command(&mut s, &mut f, Command::Exit);
	assert_eq!(app.take_dialog(), Some(Dialog::ConfirmSave(Command::Exit)));
	assert!(!app.quit());
	app.run_confirmed(&mut s, &mut f, Command::Exit);
	assert!(app.quit());
}

#[test]
fn the_dialogs_answers_apply_through_the_same_state_layer() {
	let (mut app, mut s, mut f) = setup("one two one three one");
	// Find and Replace.
	app.key(&mut s, &mut f, Key::CtrlHome);
	assert!(app.find_text(&mut s, b"two", b"", false, false, FindAction::Next));
	assert!(!app.find_text(&mut s, b"absent", b"", false, false, FindAction::Next));
	app.find_text(&mut s, b"one", b"1", false, false, FindAction::ReplaceAll);
	let mut b = [0u8; 32];
	let n = app.doc().body.read(&s, 0, &mut b);
	assert_eq!(&b[..n - 1], b"1 two 1 three 1");
	assert_eq!(app.last_find().0, b"one");
	// Fonts: face, size and styles together.
	app.key(&mut s, &mut f, Key::CtrlHome);
	for _ in 0..3 {
		app.key(&mut s, &mut f, Key::Select(quill_app::Dir::Right));
	}
	app.set_font(&mut s, b"Arial", 2, 28, 1 | 4);
	let cs = app.char_state(&s);
	assert_eq!((cs.size, cs.flags & 0xF), (28, 1 | 4));
	assert_eq!(&cs.face[..usize::from(cs.face_len)], b"Arial");
	// Tabs with a leader, then all cleared.
	app.set_tab(&mut s, 1440, 2, 1);
	let mut tabs = [(0, 0); 16];
	assert_eq!(app.tab_stops(&s, &mut tabs), 1);
	app.clear_tabs(&mut s);
	assert_eq!(app.tab_stops(&s, &mut tabs), 0);
	// The folder of the last file dialog is remembered in the settings.
	app.remember_folder(b"C:\\DOCS\\LETTERS\\A.QLD");
	assert_eq!(app.last_folder(), b"C:\\DOCS\\LETTERS\\");
	app.save_settings(&mut f);
	let mut again = App::new();
	again.load_settings(&mut f);
	assert_eq!(again.last_folder(), b"C:\\DOCS\\LETTERS\\");
}

#[test]
fn the_text_build_still_draws_its_own_dialogs() {
	let mut s = HostStore::new();
	let mut f = MemFiles::default();
	let mut app = App::new();
	app.reset(&mut s).unwrap();
	app.command(&mut s, &mut f, Command::About);
	assert_eq!(app.take_dialog(), None);
	assert!(has_box(&app, &s), "text mode shows About in a box");
}

// ---- the flow layer: questions, answers and notices ----------------------------------------------------------------------------

/// Files that refuse to create a file, or to finish writing one.
#[derive(Default)]
struct Failing {
	inner: MemFiles,
	no_create: bool,
	no_close: bool,
}

impl Files for Failing {
	type R = MemSource;
	type W = NamedSink;

	fn open(&mut self, name: &[u8]) -> Option<MemSource> {
		self.inner.open(name)
	}

	fn create(&mut self, name: &[u8]) -> Option<NamedSink> {
		if self.no_create {
			None
		} else {
			self.inner.create(name)
		}
	}

	fn close(&mut self, w: NamedSink) -> bool {
		!self.no_close && self.inner.close(w)
	}
}

fn body(app: &App, s: &HostStore) -> String {
	let mut b = [0u8; 256];
	let n = app.doc().body.read(s, 0, &mut b);
	String::from_utf8_lossy(&b[..n.saturating_sub(1)]).into_owned()
}

fn text(n: &Notice) -> &str {
	std::str::from_utf8(n.text.as_bytes()).unwrap()
}

#[test]
fn every_question_has_the_windows_words_and_yes_as_the_default() {
	let (mut app, mut s, mut f) = setup("x");
	let ask = |app: &App, c| {
		let a = app.confirm_question(c);
		assert_eq!(a.buttons, Buttons::YesNoCancel);
		assert_eq!(a.default, Answer::Yes, "{c:?}");
		String::from_utf8(a.text.as_bytes().to_vec()).unwrap()
	};
	assert_eq!(
		ask(&app, Command::New),
		"Save the changes to (Untitled) before a new document starts?"
	);
	assert_eq!(
		ask(&app, Command::Open),
		"Save the changes to (Untitled) before another document opens?"
	);
	assert_eq!(
		ask(&app, Command::Exit),
		"Save the changes to (Untitled) before Quill closes?"
	);
	// A recent file replaces the document, as Open does.
	assert_eq!(
		ask(&app, Command::Recent(0)),
		"Save the changes to (Untitled) before another document opens?"
	);
	assert!(app.save_as(&mut s, &mut f, b"letter.qld"));
	app.key(&mut s, &mut f, Key::Char(b'y'));
	assert_eq!(
		ask(&app, Command::New),
		"Save the changes to LETTER.QLD before a new document starts?"
	);
}

#[test]
fn yes_on_an_untitled_document_asks_for_a_name_and_cancelling_drops_the_command() {
	let (mut app, mut s, mut f) = setup("keep me");
	app.command(&mut s, &mut f, Command::New);
	assert_eq!(app.take_dialog(), Some(Dialog::ConfirmSave(Command::New)));
	app.confirm_answered(&mut s, &mut f, Command::New, Answer::Yes);
	assert_eq!(app.take_dialog(), Some(Dialog::SaveAs));
	app.dialog_cancelled(Dialog::SaveAs);
	assert_eq!(body(&app, &s), "keep me", "nothing ran");
	assert!(app.modified());
	// Had the command stayed pending, this save would have started a new document.
	app.command(&mut s, &mut f, Command::Save);
	assert_eq!(app.take_dialog(), Some(Dialog::SaveAs));
	app.save_as_chosen(&mut s, &mut f, b"C:\\DOCS\\KEEP.QLD")
		.unwrap();
	assert_eq!(body(&app, &s), "keep me");
	assert_eq!(app.name(), b"C:\\DOCS\\KEEP.QLD");
	assert_eq!(app.last_folder(), b"C:\\DOCS\\");
}

#[test]
fn yes_saves_and_goes_on_and_no_discards_and_goes_on() {
	let (mut app, mut s, mut f) = setup("first");
	app.command(&mut s, &mut f, Command::New);
	app.take_dialog();
	app.confirm_answered(&mut s, &mut f, Command::New, Answer::Yes);
	assert_eq!(app.take_dialog(), Some(Dialog::SaveAs));
	app.save_as_chosen(&mut s, &mut f, b"A.QLD").unwrap();
	assert!(f.files.contains_key("A.QLD"), "saved first");
	assert_eq!(body(&app, &s), "", "then the new document");
	assert_eq!(app.name(), b"");
	// A named document saves without asking for a name.
	app.key(&mut s, &mut f, Key::Char(b'b'));
	app.save_as(&mut s, &mut f, b"B.QLD");
	app.key(&mut s, &mut f, Key::Char(b'c'));
	app.command(&mut s, &mut f, Command::Exit);
	assert_eq!(app.take_dialog(), Some(Dialog::ConfirmSave(Command::Exit)));
	app.confirm_answered(&mut s, &mut f, Command::Exit, Answer::Cancel);
	assert!(!app.quit() && app.modified());
	app.confirm_answered(&mut s, &mut f, Command::Exit, Answer::Yes);
	assert_eq!(app.take_dialog(), None);
	assert!(app.quit() && !app.modified());
	// No discards.
	let (mut app, mut s, mut f) = setup("gone");
	app.confirm_answered(&mut s, &mut f, Command::New, Answer::No);
	assert_eq!(body(&app, &s), "");
	assert!(!app.modified() && f.files.is_empty());
}

#[test]
fn a_failed_save_as_keeps_the_old_name() {
	let mut f = Failing::default();
	let mut s = HostStore::new();
	let mut app = App::new();
	app.reset(&mut s).unwrap();
	app.set_native_dialogs(true);
	app.key(&mut s, &mut f, Key::Char(b'a'));
	assert!(app.save_as(&mut s, &mut f, b"OLD.QLD"));
	app.key(&mut s, &mut f, Key::Char(b'b'));
	f.no_close = true;
	let r = app.save_as_chosen(&mut s, &mut f, b"NEW.RTF");
	assert_eq!(
		r.map_err(|n| (text(&n).to_string(), n.kind)),
		Err((words::CANNOT_SAVE.to_string(), NoticeKind::Status))
	);
	assert_eq!(app.name(), b"OLD.QLD", "the document keeps its name");
	assert!(app.modified());
	assert_eq!(app.message(), words::CANNOT_SAVE.as_bytes());
	assert!(
		app.recent_files().all(|r| r != b"NEW.RTF"),
		"a failed save is not a recent file"
	);
	// The format did not change either: Save still writes what OLD.QLD is.
	f.no_close = false;
	app.command(&mut s, &mut f, Command::Save);
	assert!(f.inner.files["OLD.QLD"].starts_with(b"RIFF"));
	// The text-cell prompt has the same rule.
	let mut app = App::new();
	app.reset(&mut s).unwrap();
	app.save_as(&mut s, &mut f, b"KEEP.QLD");
	f.no_close = true;
	app.command(&mut s, &mut f, Command::SaveAs);
	for b in b"OTHER.QLD" {
		app.key(&mut s, &mut f, Key::Char(*b));
	}
	app.key(&mut s, &mut f, Key::Enter);
	assert_eq!(app.name(), b"KEEP.QLD");
}

#[test]
fn save_as_starts_with_the_base_name_and_the_matching_type() {
	let (mut app, mut s, mut f) = setup("x");
	let st = app.save_as_start();
	assert_eq!((st.name, st.type_index), (&b""[..], 0));
	app.save_as_chosen(&mut s, &mut f, b"C:\\DOCS\\notes.txt")
		.unwrap();
	let st = app.save_as_start();
	assert_eq!(st.folder, b"C:\\DOCS\\");
	assert_eq!(st.name, b"NOTES.TXT");
	assert_eq!(st.type_index, 2);
	assert!(
		f.files.contains_key("C:\\DOCS\\NOTES.TXT"),
		"format by extension"
	);
}

#[test]
fn find_next_before_any_search_asks_for_the_find_dialog_and_stays_in_edit() {
	let (mut app, mut s, mut f) = setup("some text");
	app.command(&mut s, &mut f, Command::FindNext);
	assert_eq!(app.take_dialog(), Some(Dialog::Find(false)));
	assert!(
		app.editing(),
		"no invisible text-cell Find box took the keys"
	);
	assert!(!has_box(&app, &s));
	app.key(&mut s, &mut f, Key::F(3));
	assert_eq!(app.take_dialog(), Some(Dialog::Find(false)));
	// After a search it finds the next one with no box.
	app.find_text(&mut s, b"t", b"", false, false, FindAction::Next);
	app.command(&mut s, &mut f, Command::FindNext);
	assert_eq!(app.take_dialog(), None);
}

#[test]
fn replace_replaces_only_a_selection_that_matches() {
	let (mut app, mut s, mut f) = setup("one two one");
	app.key(&mut s, &mut f, Key::CtrlHome);
	assert!(app.find_text(&mut s, b"two", b"", false, false, FindAction::Next));
	// "two" is selected; Replace of "one" must not overwrite it.
	app.find_text(&mut s, b"one", b"1", false, false, FindAction::Replace);
	assert_eq!(body(&app, &s), "one two one");
	// Now "one" is selected: replaced, and the next match is selected.
	app.find_text(&mut s, b"one", b"1", false, false, FindAction::Replace);
	assert_eq!(body(&app, &s), "one two 1");
	// Case is part of matching.
	app.key(&mut s, &mut f, Key::CtrlHome);
	app.find_text(&mut s, b"ONE", b"", false, false, FindAction::Next);
	app.find_text(&mut s, b"ONE", b"x", true, false, FindAction::Replace);
	assert_eq!(body(&app, &s), "one two 1");
}

#[test]
fn find_and_replace_all_notices() {
	let (mut app, mut s, mut f) = setup("one two one");
	let fields = |find, with| FindFields {
		find,
		with,
		case: false,
		word: false,
	};
	app.key(&mut s, &mut f, Key::CtrlHome);
	let n = app
		.find_pressed(&mut s, fields(b"absent", b""), FindAction::Next)
		.unwrap();
	assert_eq!(
		(text(&n), n.kind),
		("Quill cannot find that text.", NoticeKind::Info)
	);
	assert_eq!(app.message(), b"", "the box is the message");
	assert!(app
		.find_pressed(&mut s, fields(b"two", b""), FindAction::Next)
		.is_none());
	let n = app
		.find_pressed(&mut s, fields(b"one", b"1"), FindAction::ReplaceAll)
		.unwrap();
	assert_eq!(text(&n), "Quill made 2 replacements.");
	assert_eq!(body(&app, &s), "1 two 1");
	let n = app
		.find_pressed(&mut s, fields(b"two", b"2"), FindAction::ReplaceAll)
		.unwrap();
	assert_eq!(text(&n), "Quill made 1 replacement.");
	assert!(app
		.find_pressed(&mut s, fields(b"", b""), FindAction::Next)
		.is_none());
}

#[test]
fn spelling_notices_and_steps() {
	let (mut app, mut s, mut f) = setup("The quikc fox");
	// No dictionary on the disk.
	match app.spell_started(&mut s, &mut f) {
		SpellStep::Done(n) => assert_eq!(text(&n), "The spelling dictionary was not found."),
		other => panic!("{other:?}"),
	}
	f.files.insert(
		"EN_CA.DIC".to_string(),
		std::fs::read(concat!(
			env!("CARGO_MANIFEST_DIR"),
			"/../../tests/docs/en_ca.dic"
		))
		.unwrap(),
	);
	app.key(&mut s, &mut f, Key::CtrlHome);
	assert_eq!(app.spell_started(&mut s, &mut f), SpellStep::Next);
	assert_eq!(app.spell_word(), b"quikc");
	assert_eq!(
		app.spell_pressed(&mut s, &mut f, SpellButton::Change, b""),
		SpellStep::Stay,
		"Change with an empty field does nothing"
	);
	assert_eq!(app.spell_word(), b"quikc");
	match app.spell_pressed(&mut s, &mut f, SpellButton::Change, b"quick") {
		SpellStep::Done(n) => assert_eq!(
			(text(&n), n.kind),
			(words::SPELLING_COMPLETE, NoticeKind::Info)
		),
		other => panic!("{other:?}"),
	}
	assert_eq!(body(&app, &s), "The quick fox");
	// Nothing to fix says the same.
	app.key(&mut s, &mut f, Key::CtrlHome);
	match app.spell_started(&mut s, &mut f) {
		SpellStep::Done(n) => assert_eq!(text(&n), "The spelling check is complete."),
		other => panic!("{other:?}"),
	}
	// Cancel closes with no box.
	app.key(&mut s, &mut f, Key::CtrlEnd);
	for c in b" zzq" {
		app.key(&mut s, &mut f, Key::Char(*c));
	}
	app.key(&mut s, &mut f, Key::CtrlHome);
	assert_eq!(app.spell_started(&mut s, &mut f), SpellStep::Next);
	assert_eq!(
		app.spell_pressed(&mut s, &mut f, SpellButton::Cancel, b""),
		SpellStep::Cancelled
	);
}

#[test]
fn tab_notices() {
	let (mut app, mut s, _f) = setup("x");
	let n = app
		.tab_pressed(&mut s, b"abc", 0, 0, TabButton::Set)
		.unwrap();
	assert_eq!(
		(text(&n), n.kind),
		("Type a position in inches, such as 1.25.", NoticeKind::Warn)
	);
	assert!(app
		.tab_pressed(&mut s, b"", 0, 0, TabButton::Clear)
		.is_some());
	let mut stops = [(0, 0); 16];
	assert!(app
		.tab_pressed(&mut s, b"1.25", 1, 0, TabButton::Set)
		.is_none());
	assert_eq!(app.tab_stops(&s, &mut stops), 1);
	assert_eq!(stops[0], (1800, 1));
	assert!(app
		.tab_pressed(&mut s, b" 1.25 ", 0, 0, TabButton::Clear)
		.is_none());
	assert_eq!(app.tab_stops(&s, &mut stops), 0);
	for i in 1..=16 {
		let pos = format!("{i}.5");
		assert!(app
			.tab_pressed(&mut s, pos.as_bytes(), 0, 0, TabButton::Set)
			.is_none());
	}
	let n = app
		.tab_pressed(&mut s, b"20", 0, 0, TabButton::Set)
		.unwrap();
	assert_eq!(
		(text(&n), n.kind),
		(
			"A paragraph holds at most 16 tab stops.",
			NoticeKind::Status
		)
	);
	assert!(app
		.tab_pressed(&mut s, b"", 0, 0, TabButton::ClearAll)
		.is_none());
	assert_eq!(app.tab_stops(&s, &mut stops), 0);
	assert_eq!(quill_app::flow::parse_inches(b".5"), Some(720));
	assert_eq!(quill_app::flow::parse_inches(b"1.255"), Some(1800));
	assert_eq!(quill_app::flow::parse_inches(b"1.2.3"), None);
	assert_eq!(quill_app::flow::parse_inches(b"."), None);
}

#[test]
fn file_notices_and_the_remembered_folder() {
	let (mut app, mut s, mut f) = setup("hello");
	let mut ff = Failing::default();
	let n = app
		.open_chosen(&mut s, &mut ff, b"D:\\NOPE\\GONE.QLD")
		.unwrap_err();
	assert_eq!(
		(text(&n), n.kind),
		("Quill cannot open that file.", NoticeKind::Status)
	);
	assert_eq!(
		app.last_folder(),
		b"D:\\NOPE\\",
		"the folder is remembered on OK"
	);
	assert_eq!(body(&app, &s), "hello", "the document is as it was");
	let n = app
		.picture_chosen(&mut s, &mut ff, b"E:\\PICS\\X.BMP")
		.unwrap_err();
	assert_eq!(text(&n), "Quill cannot use that picture.");
	assert_eq!(app.last_folder(), b"E:\\PICS\\");
	ff.no_create = true;
	let n = app.print_chosen(&mut s, &mut ff, b"LPT1").unwrap_err();
	assert_eq!(
		(text(&n), n.kind),
		("Quill could not print.", NoticeKind::Status)
	);
	ff.no_create = false;
	app.print_chosen(&mut s, &mut ff, b"OUT.TXT").unwrap();
	assert!(ff.inner.files.contains_key("OUT.TXT"));
	// Opening works and clears what waited.
	f.files.insert("IN.TXT".to_string(), b"from disk".to_vec());
	app.discard_changes();
	app.open_chosen(&mut s, &mut f, b"IN.TXT").unwrap();
	assert_eq!(body(&app, &s), "from disk");
}
