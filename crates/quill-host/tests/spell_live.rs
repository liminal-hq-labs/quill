// Check Spelling as You Type: the idle scanner marks what the F7 check would stop at, keeps its marks true through edits, and holds back
// the word being typed.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use proptest::prelude::*;
use quill_app::{App, Command, Key};
use quill_host::script::MemFiles;
use quill_host::HostStore;

const DICT: &[u8] = include_bytes!("../../../tests/docs/en_ca.dic");

fn setup(live: bool) -> (App, HostStore, MemFiles) {
	let mut s = HostStore::new();
	let mut f = MemFiles::default();
	f.files.insert("EN_CA.DIC".to_string(), DICT.to_vec());
	let mut app = App::new();
	app.reset(&mut s).unwrap();
	app.set_spell_live_default(live);
	(app, s, f)
}

fn type_text(app: &mut App, s: &mut HostStore, f: &mut MemFiles, text: &str) {
	for b in text.bytes() {
		match b {
			b'\n' => app.key(s, f, Key::Enter),
			_ => app.key(s, f, Key::Char(b)),
		}
	}
}

fn body(app: &App, s: &HostStore) -> Vec<u8> {
	let n = app.doc().body.len() as usize;
	let mut b = vec![0u8; n];
	let got = app.doc().body.read(s, 0, &mut b);
	b.truncate(got);
	b
}

fn marks(app: &App, s: &HostStore) -> Vec<(u32, u32)> {
	app.spell_marks(s, 0, u32::MAX).collect()
}

fn words_of(app: &App, s: &HostStore) -> Vec<String> {
	let b = body(app, s);
	marks(app, s)
		.iter()
		.map(|(lo, hi)| String::from_utf8_lossy(&b[*lo as usize..*hi as usize]).into_owned())
		.collect()
}

/// Looks at the whole document and runs the scanner until it has nothing left.
fn settle(app: &mut App, s: &mut HostStore, f: &mut MemFiles) {
	let len = app.doc().body.len();
	app.spell_set_view(0, len);
	assert!(app.spell_live_ready(s, f));
	for _ in 0..10_000 {
		if !app.spell_idle(s, 4) {
			return;
		}
	}
	panic!("the scanner never finished");
}

/// What the F7 check stops at in the whole document, as a fresh run over the same text.
fn f7_words(app: &mut App, s: &mut HostStore, f: &mut MemFiles) -> Vec<(u32, u32)> {
	app.key(s, f, Key::CtrlHome);
	let mut out = Vec::new();
	if !app.spell_begin(s, f) {
		return out;
	}
	loop {
		let (lo, hi) = app.selection().unwrap();
		out.push((lo, hi));
		if !app.spell_ignore(s, false) {
			break;
		}
	}
	out
}

#[test]
fn a_misspelt_word_is_marked_when_the_keyboard_is_quiet() {
	let (mut app, mut s, mut f) = setup(true);
	type_text(&mut app, &mut s, &mut f, "The quikc fox jumped ");
	assert!(
		app.spell_marks(&s, 0, 100).next().is_none(),
		"nothing is checked while typing"
	);
	settle(&mut app, &mut s, &mut f);
	assert_eq!(marks(&app, &s), vec![(4, 9)]);
	assert_eq!(words_of(&app, &s), ["quikc"]);
	assert_eq!(app.spell_take_changed(), Some((4, 9)));
	assert_eq!(app.spell_take_changed(), None);
}

#[test]
fn the_word_being_typed_waits_for_the_caret_to_leave_it() {
	let (mut app, mut s, mut f) = setup(true);
	type_text(&mut app, &mut s, &mut f, "The ");
	settle(&mut app, &mut s, &mut f);
	type_text(&mut app, &mut s, &mut f, "quikc");
	settle(&mut app, &mut s, &mut f);
	assert!(marks(&app, &s).is_empty(), "the caret is on the word");
	// A boundary is typed: the word is checked.
	type_text(&mut app, &mut s, &mut f, " ");
	settle(&mut app, &mut s, &mut f);
	assert_eq!(words_of(&app, &s), ["quikc"]);
	// Typing a letter into a marked word drops its mark at once.
	app.key(&mut s, &mut f, Key::Left);
	app.key(&mut s, &mut f, Key::Left);
	app.key(&mut s, &mut f, Key::Char(b'x'));
	assert!(marks(&app, &s).is_empty());
	// Moving away from the word that is still misspelt marks it again.
	app.key(&mut s, &mut f, Key::CtrlEnd);
	settle(&mut app, &mut s, &mut f);
	assert_eq!(words_of(&app, &s), ["quikxc"]);
}

#[test]
fn marks_move_with_the_text_before_them() {
	let (mut app, mut s, mut f) = setup(true);
	type_text(&mut app, &mut s, &mut f, "the quikc fox ");
	settle(&mut app, &mut s, &mut f);
	assert_eq!(marks(&app, &s), vec![(4, 9)]);
	app.key(&mut s, &mut f, Key::CtrlHome);
	type_text(&mut app, &mut s, &mut f, "the ");
	assert_eq!(marks(&app, &s), vec![(8, 13)]);
	for _ in 0..4 {
		app.key(&mut s, &mut f, Key::Backspace);
	}
	assert_eq!(marks(&app, &s), vec![(4, 9)]);
}

#[test]
fn the_f7_rules_hold_capitals_digits_the_user_dictionary_and_ignore_all() {
	let (mut app, mut s, mut f) = setup(true);
	type_text(&mut app, &mut s, &mut f, "NASA abc123 zorb quikc a x ");
	f.files
		.insert("QUILL.UDC".to_string(), b"zorb\r\n".to_vec());
	settle(&mut app, &mut s, &mut f);
	assert_eq!(words_of(&app, &s), ["quikc"]);
	// Ignore All removes the mark; so does Add to Dictionary.
	let at = marks(&app, &s)[0].0;
	assert!(app.spell_ignore_at(&mut s, at));
	settle(&mut app, &mut s, &mut f);
	assert!(marks(&app, &s).is_empty());
	app.key(&mut s, &mut f, Key::Char(b'w'));
	app.key(&mut s, &mut f, Key::Char(b' '));
	type_text(&mut app, &mut s, &mut f, "blorb ");
	settle(&mut app, &mut s, &mut f);
	assert_eq!(words_of(&app, &s), ["blorb"]);
	let at = marks(&app, &s)[0].0;
	assert!(app.spell_add_at(&mut s, &mut f, at));
	assert_eq!(f.files["QUILL.UDC"], b"zorb\r\nblorb\r\n");
	settle(&mut app, &mut s, &mut f);
	assert!(marks(&app, &s).is_empty());
}

#[test]
fn suggestions_and_replacement_through_the_mark() {
	let (mut app, mut s, mut f) = setup(true);
	type_text(&mut app, &mut s, &mut f, "The quikc fox ");
	settle(&mut app, &mut s, &mut f);
	let sg = app.spell_suggestions_at(&s, 6);
	assert!(!sg.is_empty());
	let first = sg.get(0).unwrap();
	assert_eq!(&first.text[..usize::from(first.len)], b"quick");
	assert!(app.spell_replace_at(&mut s, 6, b"quick"));
	assert_eq!(body(&app, &s), b"The quick fox \r");
	assert!(marks(&app, &s).is_empty());
	assert_eq!(app.spell_take_changed(), Some((4, 9)));
	// One Undo puts the misspelling back, and the scanner finds it again.
	app.command(&mut s, &mut f, Command::Undo);
	assert_eq!(body(&app, &s), b"The quikc fox \r");
	settle(&mut app, &mut s, &mut f);
	assert_eq!(marks(&app, &s), vec![(4, 9)]);
	assert!(!app.spell_replace_at(&mut s, 1, b"x"), "no mark there");
}

#[test]
fn the_toggle_and_the_setting() {
	let (mut app, mut s, mut f) = setup(false);
	assert!(!app.spell_live());
	app.apply_setting(b"Quill", b"AsYouType", b"1");
	assert!(app.spell_live());
	type_text(&mut app, &mut s, &mut f, "quikc fox ");
	settle(&mut app, &mut s, &mut f);
	assert_eq!(marks(&app, &s).len(), 1);
	app.command(&mut s, &mut f, Command::SpellAsYouType);
	assert!(!app.spell_live());
	assert!(marks(&app, &s).is_empty());
	assert!(!app.spell_idle(&mut s, 8));
	let mut seen = Vec::new();
	app.each_setting(|sec, key, v| {
		if key == b"AsYouType" {
			seen.push((sec.to_vec(), v.to_vec()));
		}
	});
	assert_eq!(seen, vec![(b"Quill".to_vec(), b"0".to_vec())]);
	app.command(&mut s, &mut f, Command::SpellAsYouType);
	settle(&mut app, &mut s, &mut f);
	assert_eq!(marks(&app, &s).len(), 1);
}

#[test]
fn only_the_visible_words_are_checked_and_scrolling_checks_more() {
	let (mut app, mut s, mut f) = setup(true);
	type_text(&mut app, &mut s, &mut f, "quikc fox. teh dog. wrod fox. ");
	app.key(&mut s, &mut f, Key::CtrlEnd);
	assert!(app.spell_live_ready(&mut s, &mut f));
	app.spell_set_view(0, 11);
	while app.spell_idle(&mut s, 3) {}
	assert_eq!(words_of(&app, &s), ["quikc"]);
	app.spell_set_view(0, 31);
	while app.spell_idle(&mut s, 3) {}
	assert_eq!(words_of(&app, &s), ["quikc", "teh", "wrod"]);
	assert_eq!(app.spell_marks(&s, 12, 20).count(), 1);
	assert_eq!(app.spell_mark_at(&s, 13), Some((11, 14)));
}

#[test]
fn a_dictionary_that_is_missing_stops_the_scanner_quietly() {
	let (mut app, mut s, mut f) = setup(true);
	f.files.clear();
	type_text(&mut app, &mut s, &mut f, "quikc ");
	assert!(!app.spell_live_ready(&mut s, &mut f));
	assert!(!app.spell_idle(&mut s, 8));
	assert_eq!(app.message(), b"");
}

#[derive(Clone, Debug)]
enum Op {
	Type(&'static str),
	Back(u8),
	Left(u8),
	Right(u8),
	Home,
	End,
	Enter,
	Undo,
	Redo,
	Idle(u8),
	View(u16, u16),
	Replace,
}

fn op() -> impl Strategy<Value = Op> {
	let tokens = prop::sample::select(vec![
		"the", " ", "quick", "quikc", "teh", "fox", " ", "jumpd", ", ", ". ", "NASA", "abc1", "x",
		"spelling", "sentense", "dog", "dgo", "'", "don't",
	]);
	prop_oneof![
		4 => tokens.prop_map(Op::Type),
		2 => (1u8..4).prop_map(Op::Back),
		2 => (1u8..6).prop_map(Op::Left),
		1 => (1u8..6).prop_map(Op::Right),
		1 => Just(Op::Home),
		1 => Just(Op::End),
		1 => Just(Op::Enter),
		1 => Just(Op::Undo),
		1 => Just(Op::Redo),
		4 => (1u8..8).prop_map(Op::Idle),
		1 => (0u16..120, 0u16..120).prop_map(|(a, b)| Op::View(a.min(b), a.max(b))),
		1 => Just(Op::Replace),
	]
}

fn wordish(b: u8) -> bool {
	b.is_ascii_alphanumeric()
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(200))]

	// Whatever is typed, moved over, undone and scanned: the marks are in order and apart, inside the document, each on a whole word, and
	// when the scanner has finished the marks are the words F7 stops at.
	#[test]
	fn marks_stay_true_and_agree_with_f7(ops in prop::collection::vec(op(), 1..60)) {
		let (mut app, mut s, mut f) = setup(true);
		assert!(app.spell_live_ready(&mut s, &mut f));
		for o in ops {
			match o {
				Op::Type(t) => type_text(&mut app, &mut s, &mut f, t),
				Op::Back(n) => (0..n).for_each(|_| app.key(&mut s, &mut f, Key::Backspace)),
				Op::Left(n) => (0..n).for_each(|_| app.key(&mut s, &mut f, Key::Left)),
				Op::Right(n) => (0..n).for_each(|_| app.key(&mut s, &mut f, Key::Right)),
				Op::Home => app.key(&mut s, &mut f, Key::CtrlHome),
				Op::End => app.key(&mut s, &mut f, Key::CtrlEnd),
				Op::Enter => app.key(&mut s, &mut f, Key::Enter),
				Op::Undo => app.command(&mut s, &mut f, Command::Undo),
				Op::Redo => app.command(&mut s, &mut f, Command::Redo),
				Op::Idle(n) => {
					app.spell_idle(&mut s, u32::from(n));
				}
				Op::View(a, b) => app.spell_set_view(u32::from(a), u32::from(b)),
				Op::Replace => {
					if let Some((lo, _)) = marks(&app, &s).first().copied() {
						app.spell_replace_at(&mut s, lo, b"the");
					}
				}
			}
			let text = body(&app, &s);
			let ms = marks(&app, &s);
			let mut last = 0;
			for (lo, hi) in &ms {
				prop_assert!(*lo < *hi, "empty mark");
				prop_assert!(*lo >= last, "marks overlap or are out of order: {ms:?}");
				prop_assert!(*hi < text.len() as u32, "mark outside the document: {ms:?} in {}", text.len());
				let (l, h) = (*lo as usize, *hi as usize);
				prop_assert!(l == 0 || !wordish(text[l - 1]), "mark starts inside a word");
				prop_assert!(h >= text.len() || !wordish(text[h]), "mark ends inside a word");
				prop_assert!(text[l..h].iter().all(|b| b.is_ascii_alphabetic() || *b == b'\''), "mark on something that is not a word");
				last = *hi;
			}
		}
		// The caret goes to the end after a boundary so no word is being typed, and the scanner finishes.
		app.key(&mut s, &mut f, Key::CtrlEnd);
		app.key(&mut s, &mut f, Key::Char(b' '));
		settle(&mut app, &mut s, &mut f);
		let live = marks(&app, &s);
		let want = f7_words(&mut app, &mut s, &mut f);
		prop_assert_eq!(live, want);
	}
}

/// What one lookup costs, in words a second, over a page of text with a misspelling in a tenth of it. Run it with
/// `cargo test -p quill-host --release --test spell_live idle_cost -- --ignored --nocapture`.
#[test]
#[ignore]
fn idle_cost() {
	use std::time::Instant;
	let (mut app, mut s, mut f) = setup(true);
	// The real Canadian dictionary, built from the SCOWL lists as `quill-dict` does.
	let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dict/scowl");
	let mut words = Vec::new();
	for set in ["english", "canadian", "american"] {
		for kind in [
			"words",
			"upper",
			"proper-names",
			"contractions",
			"abbreviations",
		] {
			for size in [10, 20, 35, 40, 50] {
				if let Ok(t) = std::fs::read(dir.join(format!("{set}-{kind}.{size}"))) {
					words.extend(quill_dict::read_list(&t));
				}
			}
		}
	}
	f.files.insert(
		"EN_CA.DIC".to_string(),
		quill_dict::build(&words, 1, "SCOWL"),
	);
	let line = "the quick brown fox jumped over the lazy dog with some fine words, and a sentense is teh error. ";
	for _ in 0..20 {
		type_text(&mut app, &mut s, &mut f, line);
	}
	app.key(&mut s, &mut f, Key::CtrlHome);
	assert!(app.spell_live_ready(&mut s, &mut f));
	let len = app.doc().body.len();
	let words = body(&app, &s)
		.split(|b| !b.is_ascii_alphabetic())
		.filter(|w| w.len() > 1)
		.count();
	let mut total = std::time::Duration::ZERO;
	let rounds = 200;
	for _ in 0..rounds {
		app.spell_set_view(0, 0);
		app.apply_setting(b"Quill", b"AsYouType", b"0");
		app.apply_setting(b"Quill", b"AsYouType", b"1");
		app.spell_set_view(0, len);
		let t = Instant::now();
		while app.spell_idle(&mut s, 8) {}
		total += t.elapsed();
	}
	println!(
		"idle_cost: {} words a page, {} marks, {} ns a word",
		words,
		marks(&app, &s).len(),
		total.as_nanos() / (rounds as u128 * words as u128)
	);
}
