// Editing without a window: typing and refusing, the pointer through device pixels, word movement, the caret following the view,
// the input method's surrounding text, and the caret, selection and composing text in the drawn page.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![allow(clippy::chunks_exact_to_as_chunks)]

use std::path::PathBuf;

use quill_app::mouse::Ev;
use quill_chrome::view::{PAGE_GAP, PAGE_TOP};
use quill_fonts::FontSet;
use quill_gtk::doc::{refusal, Document, Overlay, Theme};
use quill_gtk::files::GuiFiles;
use quill_gtk::view::{Mapping, View, Zoom};
use quill_tty::files::Dirs;

fn root() -> PathBuf {
	PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn document(cfg: &std::path::Path) -> Document {
	let files = GuiFiles::new(Dirs {
		base: ".".into(),
		config: cfg.to_path_buf(),
		data: Vec::new(),
	});
	Document::new(FontSet::vendored(), files).unwrap()
}

fn letter(cfg: &std::path::Path) -> (Document, View) {
	let mut doc = document(cfg);
	doc.open(&root().join("tests/docs/letter.rtf")).unwrap();
	let mut v = View::new(Zoom::Percent(100));
	let m = Mapping::new(100, 120);
	let (dw, dh) = m.device_size(900, 600);
	v.set_viewport(dw, dh, 120);
	v.set_geometry(doc.geometry());
	(doc, v)
}

/// The device pixel a little right of the left edge of the character at `pos`, at the middle of its line.
fn pixel_at(doc: &Document, v: &View, pos: u32) -> (i32, i32) {
	let stride = doc.geometry().page_h + PAGE_GAP;
	let (x, top, h) = doc.read(|app, s| app.point_of(s, pos, stride)).unwrap();
	let m = v.mapping();
	(
		m.dev(v.page_left() + x + 2),
		m.dev(PAGE_TOP + top + h / 2 - v.scroll().1),
	)
}

fn click(doc: &mut Document, v: &View, at: (i32, i32), shift: bool, tick: u32) {
	let (x, y) = at;
	doc.pointer(v, Ev::Move { x, y }, shift, tick);
	doc.pointer(v, Ev::Down { x, y }, shift, tick);
	doc.pointer(v, Ev::Up { x, y }, shift, tick);
}

fn click_pos(doc: &mut Document, v: &View, pos: u32, shift: bool, tick: u32) {
	let at = pixel_at(doc, v, pos);
	click(doc, v, at, shift, tick);
}

#[test]
fn a_character_windows_1252_lacks_is_refused_with_a_notice_and_the_rest_goes_in() {
	let cfg = tempfile::tempdir().unwrap();
	let mut doc = document(cfg.path());
	let t = doc.type_text("caf\u{e9} \u{4e2d} ok");
	assert_eq!((t.inserted, t.refused.clone()), (8, vec!['\u{4e2d}']));
	assert_eq!(
		doc.notice(),
		Some("Quill keeps text in Windows-1252: \u{4e2d}")
	);
	assert_eq!(refusal('x'), "Quill keeps text in Windows-1252: x");
	// The next key clears the notice.
	doc.key(quill_app::Key::Left);
	assert_eq!(doc.notice(), None);
	// Line breaks and tabs are keys; other control characters are dropped.
	let t = doc.type_text("a\nb\tc\u{7}");
	assert_eq!(t.inserted, 5);
	assert!(doc.modified());
}

#[test]
fn a_click_places_the_caret_and_a_shift_click_extends_the_selection() {
	let cfg = tempfile::tempdir().unwrap();
	let (mut doc, v) = letter(cfg.path());
	click_pos(&mut doc, &v, 20, false, 0);
	assert_eq!((doc.caret(), doc.selection()), (20, None));
	click_pos(&mut doc, &v, 25, true, 100);
	assert_eq!(doc.selection(), Some((20, 25)));
	click_pos(&mut doc, &v, 3, false, 200);
	assert_eq!((doc.caret(), doc.selection()), (3, None));
}

#[test]
fn a_drag_selects_and_a_double_and_a_triple_click_take_a_word_and_a_paragraph() {
	let cfg = tempfile::tempdir().unwrap();
	let (mut doc, v) = letter(cfg.path());
	// Dear reader, ... : the second word of the salutation.
	let (a, b) = doc.read(|app, s| {
		let start = app.paragraph_range(s, 14).0;
		(start, app.word_range(s, start + 5))
	});
	assert!(b.1 > b.0);
	let from = pixel_at(&doc, &v, a + 1);
	let to = pixel_at(&doc, &v, a + 8);
	doc.pointer(
		&v,
		Ev::Move {
			x: from.0,
			y: from.1,
		},
		false,
		0,
	);
	doc.pointer(
		&v,
		Ev::Down {
			x: from.0,
			y: from.1,
		},
		false,
		0,
	);
	doc.pointer(&v, Ev::Move { x: to.0, y: to.1 }, false, 0);
	doc.pointer(&v, Ev::Up { x: to.0, y: to.1 }, false, 0);
	assert_eq!(doc.selection(), Some((a + 1, a + 8)));
	// Two clicks in the time of a double click take the word, three the paragraph.
	let at = pixel_at(&doc, &v, a + 5);
	click(&mut doc, &v, at, false, 1000);
	click(&mut doc, &v, at, false, 1002);
	assert_eq!(doc.selection(), Some(b));
	click(&mut doc, &v, at, false, 1004);
	let para = doc.read(|app, s| app.paragraph_range(s, a + 5));
	assert_eq!(doc.selection(), Some(para));
	// A click that comes later is a new click, not a double.
	click(&mut doc, &v, at, false, 2000);
	assert_eq!(doc.selection(), None);
}

#[test]
fn the_pointer_is_an_i_beam_over_a_sheet_and_not_over_the_desk() {
	let cfg = tempfile::tempdir().unwrap();
	let (doc, v) = letter(cfg.path());
	let on = pixel_at(&doc, &v, 20);
	assert!(doc.over_sheet(&v, on.0, on.1));
	assert!(!doc.over_sheet(&v, 1, on.1), "the desk left of the sheet");
	assert!(!doc.over_sheet(&v, on.0, 2), "the strip above the sheet");
}

#[test]
fn ctrl_arrows_move_by_words_and_the_ends_of_the_document_extend_a_selection() {
	let cfg = tempfile::tempdir().unwrap();
	let mut doc = document(cfg.path());
	doc.type_text("one two  three");
	doc.apply(quill_gtk::keys::Mapped::Move {
		to: quill_gtk::keys::Move::DocStart,
		extend: false,
	});
	assert_eq!(doc.caret(), 0);
	let word = |doc: &mut Document, left: bool, extend: bool| {
		doc.apply(quill_gtk::keys::Mapped::Move {
			to: if left {
				quill_gtk::keys::Move::WordLeft
			} else {
				quill_gtk::keys::Move::WordRight
			},
			extend,
		});
	};
	word(&mut doc, false, false);
	assert_eq!(doc.caret(), 4);
	word(&mut doc, false, false);
	assert_eq!(doc.caret(), 9, "past the two spaces");
	word(&mut doc, false, true);
	assert_eq!(doc.selection(), Some((9, 14)));
	word(&mut doc, true, false);
	assert_eq!(doc.caret(), 9);
	word(&mut doc, true, false);
	assert_eq!(doc.caret(), 4);
	doc.apply(quill_gtk::keys::Mapped::Move {
		to: quill_gtk::keys::Move::DocEnd,
		extend: true,
	});
	assert_eq!(doc.selection(), Some((4, 14)));
}

#[test]
fn the_view_follows_the_caret_down_the_page_and_back_up() {
	let cfg = tempfile::tempdir().unwrap();
	let mut doc = document(cfg.path());
	let mut v = View::new(Zoom::Percent(100));
	let (dw, dh) = Mapping::new(100, 120).device_size(800, 400);
	v.set_viewport(dw, dh, 120);
	for _ in 0..60 {
		doc.type_text("line\n");
	}
	v.set_geometry(doc.geometry());
	assert!(doc.reveal_caret(&mut v));
	let r = doc.caret_rect().unwrap();
	let (_, vh) = v.visible();
	let y = r.content_y - v.scroll().1;
	assert!(y >= 0 && y + r.h <= vh, "the caret is in view: {y} of {vh}");
	doc.apply(quill_gtk::keys::Mapped::Move {
		to: quill_gtk::keys::Move::DocStart,
		extend: false,
	});
	assert!(doc.reveal_caret(&mut v));
	assert_eq!(v.scroll().1, 0);
}

#[test]
fn the_input_method_sees_the_paragraph_around_the_caret_and_can_delete_around_it() {
	let cfg = tempfile::tempdir().unwrap();
	let mut doc = document(cfg.path());
	doc.type_text("first\ncaf\u{e9} au lait");
	doc.key(quill_app::Key::Left);
	doc.key(quill_app::Key::Left);
	let (text, cursor) = doc.surrounding();
	assert_eq!(text, "caf\u{e9} au lait");
	assert_eq!(&text[cursor..], "it", "the cursor is a byte index");
	assert!(doc.delete_surrounding(-2, 2));
	assert_eq!(doc.surrounding().0, "caf\u{e9} au it");
	assert!(doc.delete_surrounding(0, 1));
	assert!(
		!doc.delete_surrounding(-5, 1),
		"only a backspace or a delete"
	);
}

fn count(buf: &quill_paint::pixel::PixelBuffer, rgb: [u8; 3]) -> usize {
	buf.bytes()
		.chunks_exact(4)
		.filter(|p| p[..3] == rgb)
		.count()
}

#[test]
fn the_caret_the_selection_and_the_composing_text_are_in_the_page() {
	let cfg = tempfile::tempdir().unwrap();
	let (mut doc, v) = letter(cfg.path());
	click_pos(&mut doc, &v, 20, false, 0);
	let plain = doc.render(&v, Theme::LIGHT, &Overlay::default());
	let navy = [0, 0, 128];
	let on = Overlay {
		active: true,
		caret: true,
		..Overlay::default()
	};
	let with_caret = doc.render(&v, Theme::LIGHT, &on);
	assert_ne!(plain.bytes(), with_caret.bytes(), "the caret is drawn");
	let off = Overlay {
		caret: false,
		..on.clone()
	};
	assert_eq!(
		doc.render(&v, Theme::LIGHT, &off).bytes(),
		plain.bytes(),
		"the off phase draws nothing"
	);
	// A selection is navy behind white text.
	click_pos(&mut doc, &v, 30, true, 100);
	let sel = doc.render(&v, Theme::LIGHT, &on);
	assert!(count(&sel, navy) > 100, "{}", count(&sel, navy));
	assert_eq!(count(&plain, navy), 0);
	// Composing text is drawn over the line, with an underline.
	click_pos(&mut doc, &v, 20, false, 5000);
	let composing = Overlay {
		preedit: b"nihon".to_vec(),
		preedit_cursor: 5,
		..on
	};
	let c = doc.render(&v, Theme::LIGHT, &composing);
	assert_ne!(c.bytes(), with_caret.bytes());
}
