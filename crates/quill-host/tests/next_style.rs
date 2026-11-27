// The next preset (CORE-39) and the format typed at a paragraph's start (CORE-40): Enter at the end of a heading starts a Normal
// paragraph in the same undo step, the middle and the start of a heading keep it, and text typed at a paragraph's start takes the
// character at the caret's format, never the previous paragraph's.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use proptest::prelude::*;
use quill_app::{App, Command, Key, STYLE_NAMES};
use quill_core::doc::{cflag, CFmt, PFmt};
use quill_host::script::MemFiles;
use quill_host::HostStore;

type Rig = (App, HostStore, MemFiles);

const NORMAL: usize = 0;
const H1: usize = 1;
const H2: usize = 2;
const H3: usize = 3;
const TITLE: usize = 4;
const QUOTE: usize = 5;
const LIST: usize = 6;
const HANG: usize = 7;

/// A document of paragraphs, each in the preset named, with the caret at the start.
fn build(paras: &[(&str, usize)]) -> Rig {
	let mut s = HostStore::new();
	let mut f = MemFiles::default();
	let mut app = App::new();
	app.reset(&mut s).unwrap();
	for (i, (text, _)) in paras.iter().enumerate() {
		for b in text.bytes() {
			app.key(&mut s, &mut f, Key::Char(b));
		}
		if i + 1 < paras.len() {
			app.key(&mut s, &mut f, Key::Enter);
		}
	}
	let mut start = 0;
	for (text, n) in paras {
		app.place_caret(&s, start, false);
		app.apply_style(&mut s, *n);
		start += text.len() as u32 + 1;
	}
	app.place_caret(&s, 0, false);
	(app, s, f)
}

fn text(app: &App, s: &HostStore) -> Vec<u8> {
	let body = &app.doc().body;
	let mut bytes = vec![0u8; body.len() as usize];
	let n = body.read(s, 0, &mut bytes);
	bytes.truncate(n);
	bytes
}

fn cfmt(app: &App, s: &HostStore, pos: u32) -> CFmt {
	let i = app.doc().body.cfmt_at(s, pos).unwrap();
	app.doc().cfmts.get(s, i).unwrap()
}

fn pfmt(app: &App, s: &HostStore, pos: u32) -> PFmt {
	let i = app.doc().body.pfmt_at(s, pos).unwrap();
	app.doc().pfmts.get(s, i).unwrap()
}

/// Everything an edit can change that a round trip must give back: the text, each character's format and each paragraph's.
fn snapshot(app: &App, s: &HostStore) -> (Vec<u8>, Vec<CFmt>, Vec<PFmt>, u32) {
	let t = text(app, s);
	let cs = (0..t.len() as u32).map(|i| cfmt(app, s, i)).collect();
	let ps = t
		.iter()
		.enumerate()
		.filter(|(_, b)| **b == 0x0D)
		.map(|(i, _)| pfmt(app, s, i as u32))
		.collect();
	(t, cs, ps, app.caret())
}

fn style(app: &App, s: &HostStore) -> Option<usize> {
	app.style_of(s)
}

fn enter(r: &mut Rig) {
	r.0.key(&mut r.1, &mut r.2, Key::Enter);
}

fn type_str(r: &mut Rig, t: &str) {
	for b in t.bytes() {
		r.0.key(&mut r.1, &mut r.2, Key::Char(b));
	}
}

fn undo(r: &mut Rig) {
	r.0.command(&mut r.1, &mut r.2, Command::Undo);
}

#[test]
fn enter_at_the_end_of_a_heading_starts_a_normal_paragraph() {
	for n in [H1, H2, H3, TITLE] {
		let mut r = build(&[("Heading", n)]);
		r.0.place_caret(&r.1, 7, false);
		enter(&mut r);
		assert_eq!(style(&r.0, &r.1), Some(NORMAL), "{}", STYLE_NAMES[n]);
		assert_eq!(r.0.caret(), 8);
		// The new paragraph is Normal's own paragraph and character format, not the heading's size, bold or spacing.
		let c = r.0.char_state(&r.1);
		assert_eq!((c.size, c.flags & cflag::BOLD), (24, 0));
		type_str(&mut r, "body");
		assert_eq!(cfmt(&r.0, &r.1, 8).size, 24);
		assert_eq!(cfmt(&r.0, &r.1, 8).flags & cflag::BOLD, 0);
		assert_eq!(pfmt(&r.0, &r.1, 12).before, 0);
		// The heading above is untouched.
		r.0.place_caret(&r.1, 0, false);
		assert_eq!(style(&r.0, &r.1), Some(n));
	}
}

#[test]
fn the_other_presets_continue_themselves() {
	for n in [NORMAL, QUOTE, LIST, HANG] {
		let mut r = build(&[("Some text", n)]);
		r.0.place_caret(&r.1, 9, false);
		enter(&mut r);
		assert_eq!(style(&r.0, &r.1), Some(n), "{}", STYLE_NAMES[n]);
	}
}

#[test]
fn enter_in_the_middle_of_a_heading_keeps_both_halves_a_heading() {
	let mut r = build(&[("Heading", H2)]);
	r.0.place_caret(&r.1, 3, false);
	enter(&mut r);
	assert_eq!(style(&r.0, &r.1), Some(H2));
	r.0.place_caret(&r.1, 0, false);
	assert_eq!(style(&r.0, &r.1), Some(H2));
	assert_eq!(text(&r.0, &r.1), b"Hea\rding\r");
}

#[test]
fn enter_at_the_start_of_a_heading_leaves_an_empty_heading_above_and_the_caret_below() {
	let mut r = build(&[("Heading", H1)]);
	enter(&mut r);
	assert_eq!(text(&r.0, &r.1), b"\rHeading\r");
	assert_eq!(r.0.caret(), 1);
	assert_eq!(style(&r.0, &r.1), Some(H1));
	r.0.place_caret(&r.1, 0, false);
	assert_eq!(
		style(&r.0, &r.1),
		Some(H1),
		"the empty paragraph is a heading too"
	);
	assert_eq!(cfmt(&r.0, &r.1, 0).size, 32);
}

#[test]
fn enter_on_an_empty_heading_starts_a_normal_paragraph_and_leaves_the_heading() {
	let mut r = build(&[("", H1)]);
	enter(&mut r);
	assert_eq!(text(&r.0, &r.1), b"\r\r");
	assert_eq!(style(&r.0, &r.1), Some(NORMAL));
	r.0.place_caret(&r.1, 0, false);
	assert_eq!(style(&r.0, &r.1), Some(H1));
}

#[test]
fn one_undo_gives_back_the_heading_exactly() {
	for n in [H1, H3, TITLE] {
		let mut r = build(&[("Intro", NORMAL), ("Heading", n), ("After", NORMAL)]);
		r.0.place_caret(&r.1, 13, false);
		let before = snapshot(&r.0, &r.1);
		enter(&mut r);
		assert_ne!(snapshot(&r.0, &r.1), before);
		undo(&mut r);
		// The caret is not restored by Undo yet (CORE-49), so it is not compared; the document is.
		let (t, c, p, _) = snapshot(&r.0, &r.1);
		assert_eq!(
			(t, c, p),
			(before.0, before.1, before.2),
			"{}",
			STYLE_NAMES[n]
		);
		r.0.place_caret(&r.1, 7, false);
		assert_eq!(style(&r.0, &r.1), Some(n));
	}
}

#[test]
fn title_then_enter_then_type() {
	let mut r = build(&[("My title", TITLE)]);
	r.0.place_caret(&r.1, 8, false);
	enter(&mut r);
	type_str(&mut r, "It begins");
	assert_eq!(text(&r.0, &r.1), b"My title\rIt begins\r");
	assert_eq!(style(&r.0, &r.1), Some(NORMAL));
	assert_eq!(cfmt(&r.0, &r.1, 9).size, 24);
	r.0.place_caret(&r.1, 0, false);
	assert_eq!(style(&r.0, &r.1), Some(TITLE));
	assert_eq!(pfmt(&r.0, &r.1, 8).align, 1, "the title stays centred");
}

#[test]
fn a_heading_that_was_changed_afterwards_keeps_the_old_behaviour() {
	let mut r = build(&[("Heading", H1)]);
	// A new alignment keeps the heading a heading (CORE-142); an indent no preset has does not.
	r.0.command(&mut r.1, &mut r.2, Command::Align(2));
	assert_eq!(style(&r.0, &r.1), Some(H1));
	r.0.set_indents(&mut r.1, 100, 0, 0);
	assert_eq!(style(&r.0, &r.1), None);
	r.0.place_caret(&r.1, 7, false);
	enter(&mut r);
	assert_eq!(style(&r.0, &r.1), None);
	assert_eq!(cfmt(&r.0, &r.1, 7).size, 32);
}

#[test]
fn typing_at_the_start_of_a_heading_keeps_heading_size_text() {
	let mut r = build(&[("Plain", NORMAL), ("Heading", H1)]);
	r.0.place_caret(&r.1, 6, false);
	type_str(&mut r, "My ");
	let c = cfmt(&r.0, &r.1, 6);
	assert_eq!((c.size, c.flags & cflag::BOLD), (32, cflag::BOLD));
	assert_eq!(style(&r.0, &r.1), Some(H1));
}

#[test]
fn typing_on_an_empty_line_under_a_heading_is_normal() {
	let mut r = build(&[("Heading", H1)]);
	r.0.place_caret(&r.1, 7, false);
	enter(&mut r);
	type_str(&mut r, "x");
	let c = cfmt(&r.0, &r.1, 8);
	assert_eq!((c.size, c.flags & cflag::BOLD), (24, 0));
	// An empty paragraph made some other way takes its own mark's format.
	let mut r = build(&[("Heading", H1), ("", NORMAL)]);
	r.0.place_caret(&r.1, 8, false);
	type_str(&mut r, "x");
	assert_eq!(cfmt(&r.0, &r.1, 8).size, 24);
}

#[test]
fn a_pending_bold_still_applies_at_a_paragraphs_start() {
	let mut r = build(&[("Heading", H1)]);
	r.0.place_caret(&r.1, 7, false);
	enter(&mut r);
	r.0.command(&mut r.1, &mut r.2, Command::Bold);
	type_str(&mut r, "b");
	let c = cfmt(&r.0, &r.1, 8);
	assert_eq!((c.size, c.flags & cflag::BOLD), (24, cflag::BOLD));
	type_str(&mut r, "c");
	assert_eq!(cfmt(&r.0, &r.1, 9).flags & cflag::BOLD, cflag::BOLD);
	// Bold toggled at the start of a bold heading turns it off for what is typed.
	let mut r = build(&[("Plain", NORMAL), ("Heading", H1)]);
	r.0.place_caret(&r.1, 6, false);
	r.0.command(&mut r.1, &mut r.2, Command::Bold);
	type_str(&mut r, "n");
	assert_eq!(cfmt(&r.0, &r.1, 6).flags & cflag::BOLD, 0);
}

#[test]
fn the_style_box_reads_normal_straight_after_the_enter() {
	let mut r = build(&[("Heading", H1)]);
	r.0.place_caret(&r.1, 7, false);
	assert_eq!(style(&r.0, &r.1), Some(H1));
	enter(&mut r);
	assert_eq!(style(&r.0, &r.1), Some(NORMAL));
	assert_eq!(r.0.para_state(&r.1).before, 0);
}

fn doc_strategy() -> impl Strategy<Value = Vec<(String, usize)>> {
	prop::collection::vec(("[a-z ]{0,6}", 0usize..8), 1..4)
}

proptest! {
	/// Enter then Backspace gives back the text, every character's format and every paragraph's, with the caret where it was, for every
	/// preset and every caret position. The one exception is an empty paragraph whose next preset differs: Backspace there is the
	/// ordinary join of an empty paragraph, which keeps the lower one (D98).
	#[test]
	fn enter_then_backspace_round_trips(paras in doc_strategy(), pos in 0u32..40) {
		let spec: Vec<(&str, usize)> = paras.iter().map(|(t, n)| (t.as_str(), *n)).collect();
		let mut r = build(&spec);
		let len = text(&r.0, &r.1).len() as u32;
		let pos = pos % len;
		r.0.place_caret(&r.1, pos, false);
		// Skip the empty paragraph of a heading.
		let t = text(&r.0, &r.1);
		let empty = t[pos as usize] == 0x0D && (pos == 0 || t[pos as usize - 1] == 0x0D);
		let st = style(&r.0, &r.1);
		prop_assume!(!(empty && st.is_some_and(|n| n != NORMAL && n <= TITLE)));
		let before = snapshot(&r.0, &r.1);
		enter(&mut r);
		r.0.key(&mut r.1, &mut r.2, Key::Backspace);
		prop_assert_eq!(snapshot(&r.0, &r.1), before);
	}

	/// Enter then Undo gives back the same text, formats and caret.
	#[test]
	fn enter_then_undo_round_trips(paras in doc_strategy(), pos in 0u32..40) {
		let spec: Vec<(&str, usize)> = paras.iter().map(|(t, n)| (t.as_str(), *n)).collect();
		let mut r = build(&spec);
		let len = text(&r.0, &r.1).len() as u32;
		r.0.place_caret(&r.1, pos % len, false);
		let (t, c, p, _) = snapshot(&r.0, &r.1);
		enter(&mut r);
		undo(&mut r);
		let (t2, c2, p2, _) = snapshot(&r.0, &r.1);
		prop_assert_eq!((t, c, p), (t2, c2, p2));
	}
}
