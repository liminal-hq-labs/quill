// The spelling lists in the document's store: the user dictionary (`QUILL.UDC`) and the words ignored for the session are kept in a
// `Store` chunk, so the editor holds a handle and the Windows 3.x data segment holds neither. These tests drive them through the F7
// check and the files, with the lists full, a word at the limits and a damaged dictionary file.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use quill_app::{App, Key};
use quill_host::script::MemFiles;
use quill_host::HostStore;

const DICT: &[u8] = include_bytes!("../../../tests/docs/en_ca.dic");
const UDC: &str = "QUILL.UDC";
/// The room in the user dictionary, in bytes.
const ROOM: usize = 2048;

fn setup(udc: Option<&[u8]>) -> (App, HostStore, MemFiles) {
	let mut s = HostStore::new();
	let mut f = MemFiles::default();
	f.files.insert("EN_CA.DIC".to_string(), DICT.to_vec());
	if let Some(u) = udc {
		f.files.insert(UDC.to_string(), u.to_vec());
	}
	let mut app = App::new();
	app.reset(&mut s).unwrap();
	(app, s, f)
}

fn type_text(app: &mut App, s: &mut HostStore, f: &mut MemFiles, text: &str) {
	for b in text.bytes() {
		app.key(s, f, Key::Char(b));
	}
}

/// Starts a check at the top and returns the first word it stops at.
fn first_stop(app: &mut App, s: &mut HostStore, f: &mut MemFiles) -> Option<String> {
	app.key(s, f, Key::CtrlHome);
	app.spell_begin(s, f)
		.then(|| String::from_utf8_lossy(app.spell_word()).into_owned())
}

/// A word of `n` letters that no dictionary has, different for each `i`.
fn made_up(i: usize, n: usize) -> String {
	let mut w = String::from("zq");
	let mut k = i;
	while w.len() < n {
		w.push((b'a' + (k % 26) as u8) as char);
		k /= 26;
	}
	w.truncate(n);
	w
}

#[test]
fn an_added_word_is_known_and_reaches_the_file() {
	let (mut app, mut s, mut f) = setup(None);
	type_text(&mut app, &mut s, &mut f, "zorb ");
	assert_eq!(
		first_stop(&mut app, &mut s, &mut f).as_deref(),
		Some("zorb")
	);
	assert!(!app.spell_add(&mut s, &mut f));
	assert_eq!(f.files[UDC], b"zorb\r\n");
	assert_eq!(first_stop(&mut app, &mut s, &mut f), None);
}

#[test]
fn the_dictionary_comes_back_from_the_file_in_a_new_session() {
	let (mut app, mut s, mut f) = setup(None);
	type_text(&mut app, &mut s, &mut f, "zorb blorb ");
	assert_eq!(
		first_stop(&mut app, &mut s, &mut f).as_deref(),
		Some("zorb")
	);
	assert!(app.spell_add(&mut s, &mut f));
	app.spell_add(&mut s, &mut f);
	assert_eq!(f.files[UDC], b"zorb\r\nblorb\r\n");
	// A new editor and a new store read the same file; case does not matter.
	let (mut app2, mut s2, _) = setup(None);
	type_text(&mut app2, &mut s2, &mut f, "ZORB Blorb quikc ");
	assert_eq!(
		first_stop(&mut app2, &mut s2, &mut f).as_deref(),
		Some("quikc")
	);
	// Adding to the reloaded list appends to what was there.
	assert!(!app2.spell_add(&mut s2, &mut f));
	assert_eq!(f.files[UDC], b"zorb\r\nblorb\r\nquikc\r\n");
}

#[test]
fn ignore_all_skips_the_word_for_the_session_only() {
	let (mut app, mut s, mut f) = setup(None);
	type_text(&mut app, &mut s, &mut f, "blorb a blorb ");
	assert_eq!(
		first_stop(&mut app, &mut s, &mut f).as_deref(),
		Some("blorb")
	);
	assert!(!app.spell_ignore(&mut s, true));
	assert!(!f.files.contains_key(UDC));
	assert_eq!(first_stop(&mut app, &mut s, &mut f), None);
	// A new session has forgotten it.
	let (mut app2, mut s2, _) = setup(None);
	type_text(&mut app2, &mut s2, &mut f, "blorb ");
	assert_eq!(
		first_stop(&mut app2, &mut s2, &mut f).as_deref(),
		Some("blorb")
	);
}

#[test]
fn ignore_once_does_not_remember_the_word() {
	let (mut app, mut s, mut f) = setup(None);
	type_text(&mut app, &mut s, &mut f, "blorb blorb ");
	assert_eq!(
		first_stop(&mut app, &mut s, &mut f).as_deref(),
		Some("blorb")
	);
	assert!(app.spell_ignore(&mut s, false));
	assert_eq!(app.spell_word(), b"blorb");
}

#[test]
fn the_ignore_table_holds_twenty_four_words_and_then_overwrites_the_first_place() {
	let (mut app, mut s, mut f) = setup(None);
	let words: Vec<String> = (0..25).map(|i| made_up(i, 6 + i % 7)).collect();
	let text: String = words.iter().map(|w| format!("{w} ")).collect();
	type_text(&mut app, &mut s, &mut f, &text);
	// Ignore the first 24 in turn: each stop is the next word still unknown.
	for w in &words[..24] {
		let at = first_stop(&mut app, &mut s, &mut f);
		assert_eq!(at.as_deref(), Some(w.as_str()));
		app.spell_ignore(&mut s, true);
	}
	assert_eq!(
		first_stop(&mut app, &mut s, &mut f).as_deref(),
		Some(words[24].as_str())
	);
	// A twenty-fifth word takes the place of the first, which is then flagged again, and the other twenty-three are still ignored.
	app.spell_ignore(&mut s, true);
	assert_eq!(
		first_stop(&mut app, &mut s, &mut f).as_deref(),
		Some(words[0].as_str())
	);
}

#[test]
fn a_word_at_the_longest_length_is_kept() {
	let (mut app, mut s, mut f) = setup(None);
	let longest = made_up(1, 31);
	let shortest = made_up(2, 2);
	type_text(&mut app, &mut s, &mut f, &format!("{longest} {shortest} "));
	assert_eq!(
		first_stop(&mut app, &mut s, &mut f).as_deref(),
		Some(longest.as_str())
	);
	app.spell_add(&mut s, &mut f);
	assert_eq!(
		first_stop(&mut app, &mut s, &mut f).as_deref(),
		Some(shortest.as_str())
	);
	app.spell_ignore(&mut s, true);
	assert_eq!(first_stop(&mut app, &mut s, &mut f), None);
	assert_eq!(f.files[UDC], format!("{longest}\r\n").as_bytes());
	// The longest word is ignored as well as added.
	let (mut app2, mut s2, mut f2) = setup(None);
	type_text(&mut app2, &mut s2, &mut f2, &format!("{longest} "));
	assert_eq!(
		first_stop(&mut app2, &mut s2, &mut f2).as_deref(),
		Some(longest.as_str())
	);
	app2.spell_ignore(&mut s2, true);
	assert_eq!(first_stop(&mut app2, &mut s2, &mut f2), None);
}

#[test]
fn the_user_dictionary_fills_to_the_last_byte_and_refuses_the_next_word() {
	// 128 words of 14 letters and a line end make exactly 2,048 bytes.
	let mut file = Vec::new();
	for i in 0..127 {
		file.extend_from_slice(format!("{}\r\n", made_up(i, 14)).as_bytes());
	}
	let (mut app, mut s, mut f) = setup(Some(&file));
	let last = made_up(127, 14);
	let extra = made_up(500, 5);
	type_text(&mut app, &mut s, &mut f, &format!("{last} {extra} "));
	assert_eq!(
		first_stop(&mut app, &mut s, &mut f).as_deref(),
		Some(last.as_str())
	);
	// The word that fills the last 16 bytes is accepted.
	app.spell_add(&mut s, &mut f);
	assert_eq!(f.files[UDC].len(), ROOM);
	assert_eq!(
		first_stop(&mut app, &mut s, &mut f).as_deref(),
		Some(extra.as_str())
	);
	// Nothing more fits: the file is the same and the word is still unknown.
	let before = f.files[UDC].clone();
	app.spell_add(&mut s, &mut f);
	assert_eq!(f.files[UDC], before);
	assert_eq!(
		first_stop(&mut app, &mut s, &mut f).as_deref(),
		Some(extra.as_str())
	);
	// Every word kept is still found, the first and the last.
	let (mut app2, mut s2, mut f2) = setup(Some(&before));
	type_text(
		&mut app2,
		&mut s2,
		&mut f2,
		&format!("{} {} ", made_up(0, 14), last),
	);
	assert_eq!(first_stop(&mut app2, &mut s2, &mut f2), None);
}

#[test]
fn a_word_that_just_fits_and_a_word_that_just_does_not() {
	// 120 lines of 17 bytes use 2,040: a word of 6 letters (8 bytes) fits exactly and one of 7 does not.
	let mut file = Vec::new();
	for i in 0..120 {
		file.extend_from_slice(format!("{}\r\n", made_up(i, 15)).as_bytes());
	}
	assert_eq!(file.len(), ROOM - 8);
	let (mut app, mut s, mut f) = setup(Some(&file));
	type_text(&mut app, &mut s, &mut f, "zqseven zqsixx ");
	assert_eq!(
		first_stop(&mut app, &mut s, &mut f).as_deref(),
		Some("zqseven")
	);
	app.spell_add(&mut s, &mut f);
	assert_eq!(f.files[UDC].len(), ROOM - 8, "seven letters need 9 bytes");
	// The refusal moved on to the next word, which is the one that fits.
	assert_eq!(app.spell_word(), b"zqsixx");
	app.spell_add(&mut s, &mut f);
	assert_eq!(f.files[UDC].len(), ROOM);
	assert_eq!(&f.files[UDC][ROOM - 8..], b"zqsixx\r\n");
}

#[test]
fn a_damaged_file_reads_what_fits_and_never_panics() {
	// Longer than the room, with a long run and no line ends, lone carriage returns, zero and high bytes, and a word cut at the room's
	// edge: `zqcut` is the last thing inside it and `ting` is the first thing outside.
	let mut file = vec![b'x'; 700];
	file.extend_from_slice(b"\r\r\r\nzorb\n\x00\x00zork\r\n");
	file.extend((0..=255u8).cycle().take(600));
	file.extend_from_slice(b"\nblorb\n");
	file.resize(ROOM - 5, b'\n');
	file.extend_from_slice(b"zqcut");
	file.extend_from_slice(b"ting\r\nzzzz\r\n");
	file.extend((0..3000).map(|i| (i * 7 % 251) as u8));
	assert!(file.len() > ROOM);
	let (mut app, mut s, mut f) = setup(Some(&file));
	type_text(
		&mut app,
		&mut s,
		&mut f,
		"zorb zork blorb zqcut zqcutting zzzz ",
	);
	// What is inside the first 2,048 bytes is known, except the word with zero bytes in front of it; what is outside is lost.
	let mut stops = Vec::new();
	while let Some(w) = first_stop(&mut app, &mut s, &mut f) {
		stops.push(w);
		app.spell_ignore(&mut s, true);
	}
	assert_eq!(stops, ["zork", "zqcutting", "zzzz"]);
	// The list is full, so nothing can be added, and the file is left alone.
	type_text(&mut app, &mut s, &mut f, "qwxz ");
	assert_eq!(
		first_stop(&mut app, &mut s, &mut f).as_deref(),
		Some("qwxz")
	);
	let before = f.files[UDC].clone();
	app.spell_add(&mut s, &mut f);
	assert_eq!(f.files[UDC], before);
}

#[test]
fn an_empty_file_and_a_file_of_blank_lines_know_nothing() {
	for file in [&b""[..], b"\r\n\r\n\r\n", b"\n\n"] {
		let (mut app, mut s, mut f) = setup(Some(file));
		type_text(&mut app, &mut s, &mut f, "zorb ");
		assert_eq!(
			first_stop(&mut app, &mut s, &mut f).as_deref(),
			Some("zorb")
		);
		app.spell_add(&mut s, &mut f);
		assert_eq!(first_stop(&mut app, &mut s, &mut f), None);
		let mut want = file.to_vec();
		want.extend_from_slice(b"zorb\r\n");
		assert_eq!(f.files[UDC], want);
	}
}

#[test]
fn a_line_longer_than_any_word_matches_nothing() {
	let long = "q".repeat(300);
	let file = format!("{long}\r\nzorb\r\n");
	let (mut app, mut s, mut f) = setup(Some(file.as_bytes()));
	type_text(&mut app, &mut s, &mut f, "zorb qq qqq ");
	assert_eq!(first_stop(&mut app, &mut s, &mut f).as_deref(), Some("qq"));
}

#[test]
fn a_file_without_a_final_line_end_still_knows_its_last_word() {
	let (mut app, mut s, mut f) = setup(Some(b"zorb\r\nblorb"));
	type_text(&mut app, &mut s, &mut f, "zorb blorb ");
	assert_eq!(first_stop(&mut app, &mut s, &mut f), None);
}
