// The chrome without a window: the commands the menus and bars run, and the state they show, over a real document.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::{Path, PathBuf};

use quill_app::view::RulerState;
use quill_app::{Command, Dialog, Key, CHROME_RULER, CHROME_TOOLBAR};
use quill_fonts::FontSet;
use quill_gtk::doc::Document;
use quill_gtk::files::GuiFiles;
use quill_gtk::format_bar::{apply as apply_format, FormatEdit, FormatState};
use quill_gtk::ruler::{apply as apply_ruler, RulerEdit};
use quill_gtk::status::StatusInfo;
use quill_gtk::view::{View, Zoom};
use quill_tty::files::Dirs;

fn root() -> PathBuf {
	PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn document(config: &Path) -> Document {
	let files = GuiFiles::new(Dirs {
		base: ".".into(),
		config: config.to_path_buf(),
		data: Vec::new(),
	});
	Document::new(FontSet::vendored(), files).unwrap()
}

fn letter(doc: &mut Document) {
	doc.open(&root().join("tests/docs/letter.rtf")).unwrap();
}

fn type_text(doc: &mut Document, text: &str) {
	doc.with(|app, s, f| {
		for b in text.bytes() {
			app.key(s, f, Key::Char(b));
		}
	});
}

#[test]
fn bold_is_checked_where_the_caret_is_bold_and_a_selection_can_be_made_bold() {
	let cfg = tempfile::tempdir().unwrap();
	let mut doc = document(cfg.path());
	letter(&mut doc);
	// The caret starts in the bold title.
	assert!(doc.mark(Command::Bold).checked);
	assert!(!doc.mark(Command::Italic).checked);
	assert!(doc.mark(Command::Align(1)).checked, "the title is centred");
	assert!(doc.command(Command::SelectAll).is_none());
	assert!(doc.command(Command::Italic).is_none());
	assert!(doc.mark(Command::Italic).checked);
	assert!(doc.modified());
	assert!(doc.title().starts_with('*'));
}

#[test]
fn cut_and_copy_are_greyed_without_a_selection_and_paste_without_a_clip() {
	let cfg = tempfile::tempdir().unwrap();
	let mut doc = document(cfg.path());
	letter(&mut doc);
	assert!(doc.mark(Command::Cut).disabled);
	assert!(doc.mark(Command::Copy).disabled);
	assert!(doc.mark(Command::Paste).disabled);
	doc.command(Command::SelectAll);
	assert!(!doc.mark(Command::Copy).disabled);
	doc.command(Command::Copy);
	assert!(!doc.mark(Command::Paste).disabled);
}

#[test]
fn the_bars_toggle_and_stay_toggled_in_the_next_session() {
	let cfg = tempfile::tempdir().unwrap();
	{
		let mut doc = document(cfg.path());
		assert!(doc.mark(Command::Chrome(CHROME_RULER)).checked);
		doc.command(Command::Chrome(CHROME_RULER));
		doc.command(Command::Chrome(CHROME_TOOLBAR));
		assert!(!doc.mark(Command::Chrome(CHROME_RULER)).checked);
		doc.save_settings();
	}
	let doc = document(cfg.path());
	assert!(!doc.mark(Command::Chrome(CHROME_RULER)).checked);
	assert!(!doc.mark(Command::Chrome(CHROME_TOOLBAR)).checked);
	assert!(doc.mark(Command::Chrome(quill_app::CHROME_STATUS)).checked);
}

#[test]
fn the_dialogs_the_menus_ask_for_are_the_editors_requests() {
	let cfg = tempfile::tempdir().unwrap();
	let mut doc = document(cfg.path());
	assert_eq!(doc.command(Command::Open), Some(Dialog::Open));
	assert_eq!(doc.command(Command::Fonts), Some(Dialog::Fonts));
	assert_eq!(doc.command(Command::Tabs), Some(Dialog::Tabs));
	assert_eq!(doc.command(Command::Find), Some(Dialog::Find(false)));
	assert_eq!(doc.command(Command::Replace), Some(Dialog::Find(true)));
	assert_eq!(doc.command(Command::Spelling), Some(Dialog::Spelling));
	assert_eq!(doc.command(Command::About), Some(Dialog::About));
	assert_eq!(doc.command(Command::Picture), Some(Dialog::Picture));
	// An untitled document has no name to save under.
	assert_eq!(doc.command(Command::Save), Some(Dialog::SaveAs));
	// A changed document asks before it is replaced.
	type_text(&mut doc, "x");
	assert_eq!(
		doc.command(Command::New),
		Some(Dialog::ConfirmSave(Command::New))
	);
}

#[test]
fn the_formatting_bar_shows_the_caret_and_its_boxes_change_the_selection() {
	let cfg = tempfile::tempdir().unwrap();
	let mut doc = document(cfg.path());
	letter(&mut doc);
	let st = FormatState::of(&doc);
	assert_eq!(st.face, "Arial");
	assert_eq!(st.half_points, 32);
	assert_eq!(st.size_text(), "16");
	doc.command(Command::SelectAll);
	apply_format(&mut doc, &FormatEdit::Size(24));
	apply_format(&mut doc, &FormatEdit::Face("Courier New".into()));
	let st = FormatState::of(&doc);
	assert_eq!((st.face.as_str(), st.half_points), ("Courier New", 24));
	apply_format(&mut doc, &FormatEdit::Colour(Some((255, 0, 0))));
	assert_eq!(FormatState::of(&doc).colour, Some((255, 0, 0)));
	apply_format(&mut doc, &FormatEdit::Style(1));
	assert_eq!(FormatState::of(&doc).style, Some(1), "Heading 1");
}

#[test]
fn the_ruler_sets_indents_and_tab_stops_through_the_editor() {
	let cfg = tempfile::tempdir().unwrap();
	let mut doc = document(cfg.path());
	letter(&mut doc);
	let (margin, text, st) = doc.ruler_info();
	assert_eq!(
		(margin, text),
		(120, 576),
		"1.25 inch margins on 8.5 inch paper"
	);
	assert_eq!(st.ntabs, 0);
	apply_ruler(&mut doc, RulerEdit::AddTab(1440, 2));
	apply_ruler(&mut doc, RulerEdit::AddTab(720, 0));
	let st: RulerState = doc.ruler_info().2;
	assert_eq!(st.stops(), [(720, 0), (1440, 2)]);
	apply_ruler(&mut doc, RulerEdit::RemoveTab(720));
	assert_eq!(doc.ruler_info().2.stops(), [(1440, 2)]);
	apply_ruler(
		&mut doc,
		RulerEdit::Indents {
			left: 720,
			first: 360,
			right: 0,
		},
	);
	let st = doc.ruler_info().2;
	assert_eq!((st.left, st.first, st.right), (720, 360, 0));
}

#[test]
fn the_status_bar_reports_the_page_the_caret_the_font_and_the_changed_mark() {
	let cfg = tempfile::tempdir().unwrap();
	let mut doc = document(cfg.path());
	letter(&mut doc);
	let mut view = View::new(Zoom::Percent(100));
	view.set_geometry(doc.geometry());
	let st = StatusInfo::of(&doc, &view);
	assert_eq!(st.page, "Page 1 of 1");
	assert_eq!(st.position, "Ln 1, Col 1");
	assert_eq!(st.font, "Arial 16");
	assert_eq!(st.zoom, "100%");
	// At 100 percent there is a level either way, and none past the ends.
	assert!(st.can_zoom_in && st.can_zoom_out);
	assert!(!st.modified);
	type_text(&mut doc, "ab");
	let st = StatusInfo::of(&doc, &view);
	assert_eq!(st.position, "Ln 1, Col 3");
	assert!(st.modified);
}

#[test]
fn the_zoom_buttons_grey_at_the_ends_of_the_shared_list() {
	let cfg = tempfile::tempdir().unwrap();
	let mut doc = document(cfg.path());
	letter(&mut doc);
	let mut view = View::new(Zoom::Percent(200));
	view.set_geometry(doc.geometry());
	let st = StatusInfo::of(&doc, &view);
	assert_eq!(st.zoom, "200%");
	assert!(!st.can_zoom_in && st.can_zoom_out);
	view.set_zoom(Zoom::Percent(50), None);
	let st = StatusInfo::of(&doc, &view);
	assert!(st.can_zoom_in && !st.can_zoom_out);
	view.set_zoom(Zoom::Percent(90), None);
	let st = StatusInfo::of(&doc, &view);
	assert!(st.can_zoom_in && st.can_zoom_out);
}

#[test]
fn save_as_adds_the_default_extension_and_clears_the_changed_mark() {
	let cfg = tempfile::tempdir().unwrap();
	let dir = tempfile::tempdir().unwrap();
	let mut doc = document(cfg.path());
	type_text(&mut doc, "Hello");
	assert_eq!(doc.title(), "*Quill");
	doc.save_as(&dir.path().join("note")).unwrap();
	assert!(dir.path().join("note.qld").is_file());
	assert_eq!(doc.name(), Some("note.qld"));
	assert_eq!(doc.title(), "note.qld \u{2013} Quill");
	assert!(!doc.unsaved_work());
	type_text(&mut doc, "!");
	assert!(doc.unsaved_work());
	doc.save().unwrap();
	assert!(!doc.unsaved_work());
	// The saved file opens again with the same text.
	let mut other = document(cfg.path());
	other.open(&dir.path().join("note.qld")).unwrap();
	assert_eq!(other.read(|app, _| app.doc().body.len()), 7);
}

#[test]
fn new_document_forgets_the_name_and_the_changes() {
	let cfg = tempfile::tempdir().unwrap();
	let mut doc = document(cfg.path());
	letter(&mut doc);
	type_text(&mut doc, "x");
	doc.new_document();
	assert_eq!(doc.name(), None);
	assert!(doc.path().is_none());
	assert!(!doc.unsaved_work());
	assert_eq!(doc.read(|app, _| app.doc().body.len()), 1);
}

#[test]
fn a_recent_file_opened_by_the_editor_gives_the_window_its_name_and_folder() {
	let cfg = tempfile::tempdir().unwrap();
	let mut doc = document(cfg.path());
	letter(&mut doc);
	doc.new_document();
	assert_eq!(doc.recent().len(), 1);
	assert!(doc.command(Command::Recent(0)).is_none());
	assert_eq!(doc.name(), Some("letter.rtf"));
	assert!(doc.subtitle().ends_with("tests/docs"));
}
