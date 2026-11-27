// Editing behaviour the Windows build leans on: lists at their boundaries, hit-testing, selections by word and paragraph, fields in
// the body, the recent-file list and the menu tree's separators.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use proptest::prelude::*;
use quill_app::recent::{shorten_path, RecentList};
use quill_app::{App, Command, Grain, Key};
use quill_host::script::MemFiles;
use quill_host::HostStore;

fn setup() -> (App, HostStore, MemFiles) {
	let mut s = HostStore::new();
	let mut app = App::new();
	app.reset(&mut s).unwrap();
	(app, s, MemFiles::default())
}

fn type_text(app: &mut App, s: &mut HostStore, f: &mut MemFiles, text: &str) {
	for b in text.bytes() {
		match b {
			b'\n' => app.key(s, f, Key::Enter),
			_ => app.key(s, f, Key::Char(b)),
		}
	}
}

/// What the document holds, a paragraph at a time: its text and its list kind (0 none, 1 bullet, 2 numbered).
fn paragraphs(app: &App, s: &HostStore) -> Vec<(Vec<u8>, u8)> {
	let body = &app.doc().body;
	let mut bytes = vec![0u8; body.len() as usize];
	let n = body.read(s, 0, &mut bytes);
	bytes.truncate(n);
	let mut out = Vec::new();
	let mut cur = Vec::new();
	for (i, b) in bytes.iter().enumerate() {
		if *b == 0x0D {
			let flags = body
				.pfmt_at(s, i as u32)
				.and_then(|ix| app.doc().pfmts.get(s, ix))
				.map_or(0, |p| p.flags);
			let kind = if flags & 1 != 0 {
				1
			} else if flags & 4 != 0 {
				2
			} else {
				0
			};
			out.push((std::mem::take(&mut cur), kind));
		} else {
			cur.push(*b);
		}
	}
	out
}

fn indents(app: &App, s: &HostStore) -> (i32, i32) {
	let pf = app.para_state(s);
	(pf.left, pf.first)
}

#[test]
fn backspace_on_an_empty_bullet_removes_the_bullet_and_nothing_else() {
	let (mut app, mut s, mut f) = setup();
	type_text(&mut app, &mut s, &mut f, "intro\n");
	app.command(&mut s, &mut f, Command::Bullets);
	type_text(&mut app, &mut s, &mut f, "one\n");
	// The new bullet is empty and still a bullet.
	assert_eq!(
		paragraphs(&app, &s),
		[
			(b"intro".to_vec(), 0),
			(b"one".to_vec(), 1),
			(b"".to_vec(), 1)
		]
	);
	app.key(&mut s, &mut f, Key::Backspace);
	assert_eq!(
		paragraphs(&app, &s),
		[
			(b"intro".to_vec(), 0),
			(b"one".to_vec(), 1),
			(b"".to_vec(), 0)
		],
		"the paragraph stays and loses its bullet"
	);
	assert_eq!(indents(&app, &s), (0, 0), "and its hanging indent");
	// A second Backspace is the ordinary one: it joins the empty paragraph to the bullet above, which stays a bullet.
	app.key(&mut s, &mut f, Key::Backspace);
	assert_eq!(
		paragraphs(&app, &s),
		[(b"intro".to_vec(), 0), (b"one".to_vec(), 1)]
	);
	assert_eq!(app.caret(), 9);
}

#[test]
fn backspace_in_a_numbered_item_and_at_the_top_of_the_document() {
	let (mut app, mut s, mut f) = setup();
	app.command(&mut s, &mut f, Command::Numbering);
	type_text(&mut app, &mut s, &mut f, "first\n");
	assert_eq!(paragraphs(&app, &s)[1], (b"".to_vec(), 2));
	app.key(&mut s, &mut f, Key::Backspace);
	assert_eq!(paragraphs(&app, &s)[1], (b"".to_vec(), 0));
	// The first paragraph is numbered too; Backspace at its start takes the number off, though there is nothing before it.
	app.key(&mut s, &mut f, Key::CtrlHome);
	app.key(&mut s, &mut f, Key::Backspace);
	assert_eq!(paragraphs(&app, &s)[0], (b"first".to_vec(), 0));
	app.key(&mut s, &mut f, Key::Backspace);
	assert_eq!(
		paragraphs(&app, &s).len(),
		2,
		"nothing is joined above the first paragraph"
	);
}

#[test]
fn enter_on_an_empty_bullet_ends_the_list_and_keeps_the_paragraph() {
	let (mut app, mut s, mut f) = setup();
	app.command(&mut s, &mut f, Command::Bullets);
	type_text(&mut app, &mut s, &mut f, "a\n");
	app.key(&mut s, &mut f, Key::Enter);
	assert_eq!(
		paragraphs(&app, &s),
		[(b"a".to_vec(), 1), (b"".to_vec(), 0)]
	);
	assert_eq!(indents(&app, &s), (0, 0));
}

#[test]
fn joining_keeps_the_first_paragraphs_format_unless_it_is_empty() {
	let (mut app, mut s, mut f) = setup();
	app.command(&mut s, &mut f, Command::Bullets);
	type_text(&mut app, &mut s, &mut f, "item\n");
	app.key(&mut s, &mut f, Key::Backspace); // plain paragraph below the item
	type_text(&mut app, &mut s, &mut f, "plain");
	app.key(&mut s, &mut f, Key::Home);
	// Backspace at the start of the plain paragraph joins it to the item: one bullet, not a bullet swallowed by plain text.
	app.key(&mut s, &mut f, Key::Backspace);
	assert_eq!(paragraphs(&app, &s), [(b"itemplain".to_vec(), 1)]);
	// It is one undo step, and undoing it brings back both formats.
	app.command(&mut s, &mut f, Command::Undo);
	assert_eq!(
		paragraphs(&app, &s),
		[(b"item".to_vec(), 1), (b"plain".to_vec(), 0)]
	);

	// Delete at the end of a paragraph does the same from the other side.
	app.key(&mut s, &mut f, Key::CtrlHome);
	app.key(&mut s, &mut f, Key::End);
	app.key(&mut s, &mut f, Key::Delete);
	assert_eq!(paragraphs(&app, &s), [(b"itemplain".to_vec(), 1)]);

	// An empty paragraph above a bullet just disappears; the bullet keeps its own format.
	let (mut app, mut s, mut f) = setup();
	app.key(&mut s, &mut f, Key::Enter);
	app.command(&mut s, &mut f, Command::Bullets);
	type_text(&mut app, &mut s, &mut f, "x");
	app.key(&mut s, &mut f, Key::Home);
	app.key(&mut s, &mut f, Key::Left);
	app.key(&mut s, &mut f, Key::Delete);
	assert_eq!(paragraphs(&app, &s), [(b"x".to_vec(), 1)]);
}

/// A paragraph-and-list model of the editor, for the property test.
#[derive(Clone, Debug)]
struct Model {
	paras: Vec<(Vec<u8>, u8)>,
	at: (usize, usize),
}

impl Model {
	fn new() -> Self {
		Model {
			paras: vec![(Vec::new(), 0)],
			at: (0, 0),
		}
	}

	fn apply(&mut self, op: &Op) {
		let (p, o) = self.at;
		match op {
			Op::Char(c) => {
				self.paras[p].0.insert(o, *c);
				self.at.1 += 1;
			}
			Op::Enter => {
				let (text, kind) = &self.paras[p];
				if *kind != 0 && text.is_empty() {
					self.paras[p].1 = 0;
				} else {
					let rest = self.paras[p].0.split_off(o);
					let kind = self.paras[p].1;
					self.paras.insert(p + 1, (rest, kind));
					self.at = (p + 1, 0);
				}
			}
			Op::Backspace => {
				if o > 0 {
					self.paras[p].0.remove(o - 1);
					self.at.1 -= 1;
				} else if self.paras[p].1 != 0 {
					self.paras[p].1 = 0;
				} else if p > 0 {
					let (cur, ckind) = self.paras.remove(p);
					let (prev, pkind) = &mut self.paras[p - 1];
					let at = prev.len();
					let kind = if prev.is_empty() { ckind } else { *pkind };
					prev.extend(cur);
					*pkind = kind;
					self.at = (p - 1, at);
				}
			}
			Op::Delete => {
				if o < self.paras[p].0.len() {
					self.paras[p].0.remove(o);
				} else if p + 1 < self.paras.len() {
					let (next, nkind) = self.paras.remove(p + 1);
					let (cur, ckind) = &mut self.paras[p];
					let kind = if cur.is_empty() { nkind } else { *ckind };
					cur.extend(next);
					*ckind = kind;
				}
			}
			Op::Left => {
				if o > 0 {
					self.at.1 -= 1;
				} else if p > 0 {
					self.at = (p - 1, self.paras[p - 1].0.len());
				}
			}
			Op::Right => {
				if o < self.paras[p].0.len() {
					self.at.1 += 1;
				} else if p + 1 < self.paras.len() {
					self.at = (p + 1, 0);
				}
			}
			Op::Home => self.at.1 = 0,
			Op::End => self.at.1 = self.paras[p].0.len(),
			Op::Bullets => self.paras[p].1 = if self.paras[p].1 == 1 { 0 } else { 1 },
			Op::Numbering => self.paras[p].1 = if self.paras[p].1 == 2 { 0 } else { 2 },
		}
	}
}

#[derive(Clone, Debug)]
enum Op {
	Char(u8),
	Enter,
	Backspace,
	Delete,
	Left,
	Right,
	Home,
	End,
	Bullets,
	Numbering,
}

fn op() -> impl Strategy<Value = Op> {
	prop_oneof![
		4 => prop::sample::select(vec![b'a', b'b', b'c', b' ']).prop_map(Op::Char),
		3 => Just(Op::Enter),
		4 => Just(Op::Backspace),
		3 => Just(Op::Delete),
		2 => Just(Op::Left),
		2 => Just(Op::Right),
		1 => Just(Op::Home),
		1 => Just(Op::End),
		2 => Just(Op::Bullets),
		1 => Just(Op::Numbering),
	]
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(300))]

	// After any run of keys the document is what a plain model of paragraphs says: Backspace and Delete never lose or invent text
	// at a list boundary, a bullet's own Backspace only drops the bullet, and undoing everything brings back the empty document.
	#[test]
	fn lists_and_paragraph_joins_match_a_model(ops in prop::collection::vec(op(), 0..80)) {
		let (mut app, mut s, mut f) = setup();
		let mut model = Model::new();
		for o in &ops {
			match o {
				Op::Char(c) => app.key(&mut s, &mut f, Key::Char(*c)),
				Op::Enter => app.key(&mut s, &mut f, Key::Enter),
				Op::Backspace => app.key(&mut s, &mut f, Key::Backspace),
				Op::Delete => app.key(&mut s, &mut f, Key::Delete),
				Op::Left => app.key(&mut s, &mut f, Key::Left),
				Op::Right => app.key(&mut s, &mut f, Key::Right),
				Op::Home => app.key(&mut s, &mut f, Key::Home),
				Op::End => app.key(&mut s, &mut f, Key::End),
				Op::Bullets => app.command(&mut s, &mut f, Command::Bullets),
				Op::Numbering => app.command(&mut s, &mut f, Command::Numbering),
			}
			model.apply(o);
			prop_assert_eq!(paragraphs(&app, &s), model.paras.clone(), "after {:?}", o);
			let before: usize = model.paras[..model.at.0].iter().map(|(t, _)| t.len() + 1).sum();
			prop_assert_eq!(app.caret() as usize, before + model.at.1, "caret after {:?}", o);
		}
		for _ in 0..400 {
			app.command(&mut s, &mut f, Command::Undo);
		}
		prop_assert_eq!(paragraphs(&app, &s), vec![(Vec::new(), 0)]);
	}
}

// ---- hit-testing -----------------------------------------------------------------------------------------------------------------

fn prose(words: &[(u8, bool)]) -> String {
	let mut out = String::new();
	for (n, para) in words {
		for _ in 0..=(*n % 9) {
			out.push('w');
		}
		out.push(' ');
		if *para && *n % 5 == 0 {
			out.push('\n');
		}
	}
	out
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(200))]

	// Every position, taken to a point and back, is the position again; any point at all gives a position inside the document; and
	// moving the point down never moves the position up.
	#[test]
	fn the_point_of_a_position_hits_that_position(words in prop::collection::vec((any::<u8>(), any::<bool>()), 1..80)) {
		let (mut app, mut s, mut f) = setup();
		type_text(&mut app, &mut s, &mut f, &prose(&words));
		let len = app.doc().body.len();
		let stride = 300;
		for pos in 0..len {
			let (x, top, h) = app.point_of(&s, pos, stride).unwrap();
			let back = app.pos_at_point(&s, x, top + h / 2, stride);
			prop_assert_eq!(back, pos, "x {} top {}", x, top);
		}
		for (x, y) in [(-50, -50), (0, 0), (9999, 9999), (40, 3), (i32::MAX / 4, i32::MIN / 4), (10, 150), (500, 290)] {
			prop_assert!(app.pos_at_point(&s, x, y, stride) < len);
		}
		let mut last = 0;
		for y in (-20..2000).step_by(3) {
			let p = app.pos_at_point(&s, 25, y, stride);
			prop_assert!(p >= last, "y {} went from {} to {}", y, last, p);
			last = p;
		}
	}

	// A word, a line and a paragraph contain the position they were asked about, never the final paragraph mark's successor, and a
	// drag by words only ever grows the range it began with.
	#[test]
	fn selections_by_grain_contain_their_point(words in prop::collection::vec((any::<u8>(), any::<bool>()), 1..40), pick in any::<u16>()) {
		let (mut app, mut s, mut f) = setup();
		type_text(&mut app, &mut s, &mut f, &prose(&words));
		let len = app.doc().body.len();
		let pos = u32::from(pick) % len;
		for grain in [Grain::Word, Grain::Line, Grain::Paragraph] {
			let (lo, hi) = app.grain_range(&s, pos, grain);
			prop_assert!(lo <= pos && pos <= hi.max(lo), "{:?} {} {} {}", grain, lo, pos, hi);
			prop_assert!(hi <= len);
		}
		let start = app.grain_range(&s, pos, Grain::Word);
		app.select_range(&s, start.0, start.1);
		let other = u32::from(pick).wrapping_mul(31) % len;
		app.drag_select(&s, start, other, Grain::Word);
		if let Some((lo, hi)) = app.selection() {
			prop_assert!(lo <= start.0.max(other) && hi >= start.1.min(other).min(len - 1));
		}
	}
}

#[test]
fn clicks_select_what_a_word_processor_selects() {
	let (mut app, mut s, mut f) = setup();
	type_text(
		&mut app,
		&mut s,
		&mut f,
		"alpha beta  gamma\nsecond paragraph",
	);
	// A word and the spaces after it; a run of spaces by itself; the paragraph with its mark.
	assert_eq!(app.word_range(&s, 1), (0, 6));
	assert_eq!(app.word_range(&s, 11), (10, 12));
	assert_eq!(app.paragraph_range(&s, 3), (0, 18));
	assert_eq!(app.paragraph_range(&s, 20), (18, app.doc().body.len()));
	app.place_caret(&s, 8, false);
	assert_eq!((app.caret(), app.selection()), (8, None));
	app.place_caret(&s, 14, true);
	assert_eq!(app.selection(), Some((8, 14)));
	app.place_caret(&s, 2, true);
	assert_eq!(
		app.selection(),
		Some((2, 8)),
		"Shift+click grows from the old caret, on either side"
	);
	app.place_caret(&s, 5, false);
	assert_eq!(app.selection(), None);
}

// ---- fields, zoom, settings ------------------------------------------------------------------------------------------------------

#[test]
fn page_number_and_page_count_go_into_the_body_and_save_with_it() {
	let (mut app, mut s, mut f) = setup();
	type_text(&mut app, &mut s, &mut f, "Page  of ");
	app.key(&mut s, &mut f, Key::Home);
	for _ in 0..5 {
		app.key(&mut s, &mut f, Key::Right);
	}
	app.command(&mut s, &mut f, Command::Field(1));
	app.key(&mut s, &mut f, Key::End);
	app.command(&mut s, &mut f, Command::Field(2));
	let body = &app.doc().body;
	assert_eq!(
		body.read_byte(&s, 5),
		Some(1),
		"an object character where the page number goes"
	);
	assert_eq!(body.object_at(&s, 5).map(|v| v >> 16), Some(1));
	let end = body.len() - 2;
	assert_eq!(body.object_at(&s, end).map(|v| v >> 16), Some(2));
	let mut out = [0u8; 32];
	let n = app.field_text(1, 3, 12, &mut out);
	assert_eq!(&out[..n], b"3");
	let n = app.field_text(2, 3, 12, &mut out);
	assert_eq!(&out[..n], b"12");
	// One undo step each.
	app.command(&mut s, &mut f, Command::Undo);
	assert_eq!(app.doc().body.object_at(&s, end), None);
}

#[test]
fn zoom_is_remembered_in_the_settings_and_clamped() {
	let (mut app, mut s, mut f) = setup();
	assert_eq!(app.zoom(), 0, "the page's width is the default");
	app.command(&mut s, &mut f, Command::Zoom(75));
	assert_eq!(app.zoom(), 75);
	app.command(&mut s, &mut f, Command::Zoom(900));
	assert_eq!(app.zoom(), 200);
	app.command(&mut s, &mut f, Command::Zoom(75));
	app.save_settings(&mut f);
	let (mut other, _, _) = setup();
	other.load_settings(&mut f);
	// MemFiles keeps what `create` wrote only when closed, which `save_settings` does.
	assert_eq!(other.zoom(), 75);
	app.command(&mut s, &mut f, Command::Zoom(0));
	app.save_settings(&mut f);
	other.load_settings(&mut f);
	assert_eq!(other.zoom(), 0);
}

#[test]
fn the_zoom_box_follows_the_zoom_and_choosing_a_level_sets_it() {
	use quill_chrome::zoom_field::{zoom_command, ZOOM_LEVELS, ZOOM_TEXT_MAX};
	let (mut app, mut s, mut f) = setup();
	let mut text = [0u8; ZOOM_TEXT_MAX];
	let n = app.zoom_field_text(&mut text);
	assert_eq!(&text[..n], b"Page Width");
	assert_eq!(app.zoom_field_current(), Some(0));
	for (index, level) in ZOOM_LEVELS.iter().enumerate() {
		let command = zoom_command(index).unwrap();
		app.command(&mut s, &mut f, command);
		assert_eq!(app.zoom(), *level);
		assert_eq!(app.zoom_field_current(), Some(index));
		let n = app.zoom_field_text(&mut text);
		let want = if *level == 0 {
			"Page Width".to_string()
		} else {
			format!("{level}%")
		};
		assert_eq!(std::str::from_utf8(&text[..n]).unwrap(), want);
	}
	// A zoom from `QUILL.INI` that the list lacks shows its own percentage and ticks nothing.
	app.set_zoom(90);
	let n = app.zoom_field_text(&mut text);
	assert_eq!(&text[..n], b"90%");
	assert_eq!(app.zoom_field_current(), None);
}

#[test]
fn a_steady_caret_setting_is_read_and_kept() {
	let (mut app, _, mut f) = setup();
	assert!(
		!app.blink_steady(),
		"the caret blinks unless the settings say otherwise"
	);
	app.apply_setting(b"Quill", b"Blink", b"1");
	assert!(!app.blink_steady());
	app.apply_setting(b"Quill", b"Blink", b"0");
	assert!(app.blink_steady());
	app.save_settings(&mut f);
	let (mut other, _, _) = setup();
	other.load_settings(&mut f);
	assert!(other.blink_steady(), "Blink=0 survives the save at exit");
	// A blinking caret writes no key at all.
	other.apply_setting(b"Quill", b"Blink", b"1");
	other.save_settings(&mut f);
	let (mut third, _, _) = setup();
	third.apply_setting(b"Quill", b"Blink", b"0");
	third.load_settings(&mut f);
	assert!(
		third.blink_steady(),
		"an absent key leaves the earlier value alone"
	);
}

// ---- the recent-file list --------------------------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
enum RecentOp {
	Push(u8),
	Remove(u8),
}

fn recent_name(n: u8) -> Vec<u8> {
	format!("D:\\DOCS\\FILE{}.QLD", n % 7).into_bytes()
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(300))]

	// Most recent first, each file once (whatever its case), at most four, and a removal closes the gap.
	#[test]
	fn the_recent_list_keeps_order_without_duplicates_up_to_its_limit(
		ops in prop::collection::vec(prop_oneof![any::<u8>().prop_map(RecentOp::Push), any::<u8>().prop_map(RecentOp::Remove)], 0..60)
	) {
		let mut list = RecentList::new();
		let mut model: Vec<Vec<u8>> = Vec::new();
		for o in ops {
			match o {
				RecentOp::Push(n) => {
					let mut name = recent_name(n);
					if n % 2 == 0 {
						name.make_ascii_lowercase();
					}
					list.push(&name);
					model.retain(|m| !m.eq_ignore_ascii_case(&name));
					model.insert(0, name);
					model.truncate(4);
				}
				RecentOp::Remove(n) => {
					let name = recent_name(n);
					list.remove(&name);
					model.retain(|m| !m.eq_ignore_ascii_case(&name));
				}
			}
			let got: Vec<Vec<u8>> = list.iter().map(<[u8]>::to_vec).collect();
			prop_assert_eq!(got, model.clone());
			prop_assert!(list.len() <= 4);
		}
	}

	// A shortened path fits, keeps the drive, keeps the end and says where it cut; a path that fits is untouched.
	#[test]
	fn shortened_paths_fit_and_keep_both_ends(
		dirs in prop::collection::vec("[A-Z0-9]{1,12}", 0..8), file in "[A-Z0-9]{1,8}\\.[A-Z]{3}", max in 8usize..70
	) {
		let mut path = String::from("D:\\");
		for d in &dirs {
			path.push_str(d);
			path.push('\\');
		}
		path.push_str(&file);
		let mut out = [0u8; 80];
		let n = shorten_path(path.as_bytes(), max, &mut out);
		let short = &out[..n];
		if path.len() <= max {
			prop_assert_eq!(short, path.as_bytes());
		} else {
			prop_assert!(n <= max, "{} is {} long for {}", String::from_utf8_lossy(short), n, max);
			let tail = 1;
			prop_assert!(short.ends_with(&path.as_bytes()[path.len() - tail..]));
			if max >= file.len() + 8 {
				prop_assert!(short.starts_with(b"D:\\...\\"), "{}", String::from_utf8_lossy(short));
				prop_assert!(short.ends_with(file.as_bytes()), "{}", String::from_utf8_lossy(short));
			}
		}
	}
}

#[test]
fn long_paths_shorten_like_windows_menus() {
	let mut out = [0u8; 64];
	let n = shorten_path(
		b"D:\\MY DOCUMENTS\\LETTERS\\TO THE BANK\\JUNE\\LETTER.QLD",
		32,
		&mut out,
	);
	assert_eq!(&out[..n], b"D:\\...\\JUNE\\LETTER.QLD");
	let n = shorten_path(b"D:\\LETTER.QLD", 32, &mut out);
	assert_eq!(&out[..n], b"D:\\LETTER.QLD");
}

#[test]
fn opening_a_recent_file_that_has_gone_removes_it_and_says_so() {
	let (mut app, mut s, mut f) = setup();
	type_text(&mut app, &mut s, &mut f, "hello");
	f.files.insert("A.QLD".into(), Vec::new());
	let _ = app.save_as(&mut s, &mut f, b"GONE.QLD");
	assert_eq!(app.recent_files().count(), 1);
	f.files.remove("GONE.QLD");
	app.discard_changes();
	app.command(&mut s, &mut f, Command::Recent(0));
	assert_eq!(app.recent_files().count(), 0);
	assert_eq!(app.message(), b"Quill cannot find GONE.QLD.");
}

#[test]
fn opening_a_recent_file_asks_about_unsaved_changes_first() {
	let (mut app, mut s, mut f) = setup();
	app.set_native_dialogs(true);
	type_text(&mut app, &mut s, &mut f, "unsaved");
	let _ = app.save_as(&mut s, &mut f, b"B.QLD");
	type_text(&mut app, &mut s, &mut f, " more");
	assert!(app.modified());
	app.command(&mut s, &mut f, Command::Recent(0));
	assert_eq!(
		app.take_dialog(),
		Some(quill_app::Dialog::ConfirmSave(Command::Recent(0)))
	);
}

// ---- the menu tree ----------------------------------------------------------------------------------------------------------------

#[test]
fn menus_group_their_items_with_rules_and_never_start_or_end_with_one() {
	for m in 0..quill_app::MENU_COUNT {
		let items = quill_app::menu_items(m);
		if items.is_empty() {
			continue;
		}
		assert!(!items[0].rule, "menu {m} starts with a rule");
		let mut accels = std::collections::HashSet::new();
		for it in items {
			assert!(
				it.accel.is_empty() || accels.insert(it.accel),
				"{} repeats {}",
				it.label,
				it.accel
			);
			if !it.sub.is_empty() {
				assert!(
					it.sub.iter().all(|x| x.sub.is_empty() && !x.rule),
					"submenus are one level of plain items"
				);
			}
		}
		// A rule splits a menu into groups of at least one item: no two rules in a row is by construction (a rule is a flag).
	}
	let file = quill_app::menu_items(0);
	let rules: Vec<&str> = file.iter().filter(|i| i.rule).map(|i| i.label).collect();
	assert_eq!(rules, ["&Save", "&Print...", "E&xit"]);
	let view = quill_app::menu_items(7);
	let zoom = view
		.iter()
		.find(|i| i.label == "&Zoom")
		.expect("a Zoom item in View");
	let labels: Vec<&str> = zoom.sub.iter().map(|i| i.label).collect();
	assert_eq!(labels, ["&Page Width", "&150%", "1&00%", "&75%", "&50%"]);
}

#[test]
fn every_command_in_the_menus_has_a_hint() {
	for m in 0..quill_app::MENU_COUNT {
		for it in quill_app::menu_items(m) {
			assert!(
				!quill_app::command_hint(it.command).is_empty(),
				"{} has no hint",
				it.label
			);
			for sub in it.sub {
				assert!(
					!quill_app::command_hint(sub.command).is_empty(),
					"{} has no hint",
					sub.label
				);
			}
		}
	}
}

// ---- shortcuts ---------------------------------------------------------------------------------------------------------------------

/// The virtual-key code, and the shift state, an accelerator label such as `Ctrl+Shift+=` names.
fn parse_accel(label: &str) -> Option<(u16, bool, bool)> {
	let mut ctrl = false;
	let mut shift = false;
	let mut key = None;
	for part in label.split('+').filter(|p| !p.is_empty()) {
		match part {
			"Ctrl" => ctrl = true,
			"Shift" => shift = true,
			"Alt" => return None,
			p if p.len() == 1 => {
				let c = p.as_bytes()[0];
				key = Some(match c {
					b'A'..=b'Z' => u16::from(c),
					b'0'..=b'9' => u16::from(c),
					b'=' => 0xBB,
					_ => return None,
				});
			}
			_ => return None,
		}
	}
	let _ = shift;
	key.filter(|_| ctrl).map(|k| (k, ctrl, shift))
}

#[test]
fn every_ctrl_shortcut_in_the_menus_comes_from_its_key_down_and_none_from_a_character() {
	use quill_app::accel::{from_vk, is_ctrl_character, Accel};
	let mut seen = 0;
	for m in 0..quill_app::MENU_COUNT {
		for it in quill_app::menu_items(m) {
			if !it.accel.starts_with("Ctrl+") {
				continue;
			}
			let (code, ctrl, _) =
				parse_accel(it.accel).unwrap_or_else(|| panic!("{} {}", it.label, it.accel));
			let shift = it.accel.contains("Shift");
			let got = from_vk(code, ctrl, shift, false)
				.unwrap_or_else(|| panic!("{} ({}) has no shortcut", it.label, it.accel));
			match got {
				Accel::Command(c) => assert_eq!(c, it.command, "{}", it.label),
				Accel::Key(k) => {
					// A Ctrl+letter goes to the key handler, which must turn it into the item's command or its own equivalent.
					let (mut app, mut s, mut f) = setup();
					app.set_native_dialogs(true);
					type_text(&mut app, &mut s, &mut f, "x");
					app.key(&mut s, &mut f, k);
					let direct = app.take_dialog();
					let (mut other, mut s2, mut f2) = setup();
					other.set_native_dialogs(true);
					type_text(&mut other, &mut s2, &mut f2, "x");
					other.command(&mut s2, &mut f2, it.command);
					assert_eq!(direct, other.take_dialog(), "{} ({})", it.label, it.accel);
					assert_eq!(
						paragraphs(&app, &s),
						paragraphs(&other, &s2),
						"{} ({})",
						it.label,
						it.accel
					);
				}
			}
			seen += 1;
		}
	}
	assert!(seen >= 18, "only {seen} Ctrl shortcuts found");
	// The four letters whose character is another key's character are shortcuts here, and their characters are not text.
	for (letter, code) in [(b'h', 0x48u16), (b'i', 0x49), (b'j', 0x4A), (b'm', 0x4D)] {
		assert_eq!(
			from_vk(code, true, false, false),
			Some(Accel::Key(Key::Ctrl(letter)))
		);
	}
	for c in [8u8, 9, 10, 13, 27] {
		assert!(is_ctrl_character(c));
	}
	// Without Ctrl nothing is a shortcut: Tab and Backspace stay the keys they are, and AltGr text is left alone.
	assert_eq!(from_vk(0x49, false, false, false), None);
	assert_eq!(from_vk(0x49, true, false, true), None);
	assert_eq!(
		from_vk(0x2E, false, true, false),
		Some(Accel::Command(Command::Cut))
	);
}

#[test]
fn every_field_kind_inserts_in_the_body_and_shows_its_text() {
	let (mut app, mut s, mut f) = setup();
	app.set_print_date("2026-11-24");
	app.set_print_time("6:45 PM");
	for (kind, want) in [
		(1u8, &b"3"[..]),
		(2, b"12"),
		(3, b"2026-11-24"),
		(4, b"(Untitled)"),
		(5, b"6:45 PM"),
	] {
		app.command(&mut s, &mut f, Command::Field(kind));
		let at = app.caret() - 1;
		assert_eq!(
			app.doc().body.object_at(&s, at).map(|v| v >> 16),
			Some(u32::from(kind)),
			"field {kind} inserted"
		);
		let mut out = [0u8; 32];
		let n = app.field_text(u32::from(kind), 3, 12, &mut out);
		assert_eq!(&out[..n], want, "field {kind}");
	}
	// Several fields in a row, the same kind twice included, each its own object.
	app.command(&mut s, &mut f, Command::Field(1));
	app.command(&mut s, &mut f, Command::Field(1));
	let len = app.doc().body.len();
	assert_eq!(
		app.doc().body.object_at(&s, len - 3).map(|v| v >> 16),
		Some(1)
	);
	assert_eq!(
		app.doc().body.object_at(&s, len - 2).map(|v| v >> 16),
		Some(1)
	);
	// The Insert menu offers all five, each with a hint and a tooltip.
	let insert = quill_app::menu_items(2);
	for kind in 1..=5u8 {
		let it = insert
			.iter()
			.find(|i| i.command == Command::Field(kind))
			.expect("a menu item");
		assert!(
			!quill_app::command_hint(it.command).is_empty()
				&& !quill_app::command_tip(it.command).is_empty()
		);
	}
}

#[test]
fn the_status_position_counts_page_line_and_column_from_one() {
	let (mut app, mut s, mut f) = setup();
	assert_eq!(app.status_position(&s), (1, 1, 1));
	type_text(&mut app, &mut s, &mut f, "abc\nde");
	assert_eq!(app.status_position(&s), (1, 2, 3));
	app.key(&mut s, &mut f, Key::Home);
	assert_eq!(app.status_position(&s), (1, 2, 1));
	app.key(&mut s, &mut f, Key::CtrlHome);
	assert_eq!(app.status_position(&s), (1, 1, 1));
}

// Enter and the flags that belong to where a paragraph starts (CORE-41): page break before and restart numbering (restart shares the page break's mask in `clear_start_flags`; no key or file reader sets it, so only the break is exercised).

const BREAK: u16 = 2;

/// A document of "First." and then "Chapter two." with a page break before it, the caret at `caret` in the second paragraph.
fn chapter_with_break(caret: u32) -> (App, HostStore, MemFiles) {
	let (mut app, mut s, mut f) = setup();
	let rtf = b"{\\rtf1\\ansi{\\fonttbl{\\f0 Arial;}}\\pard\\f0 First.\\par\\pard\\pagebb\\fs24 Chapter two.\\par}";
	f.files.insert("D.RTF".into(), rtf.to_vec());
	app.open(&mut s, &mut f, b"D.RTF").unwrap();
	app.place_caret(&s, 7 + caret, false);
	(app, s, f)
}

/// The flags of each paragraph, in order.
fn flags(app: &App, s: &HostStore) -> Vec<u16> {
	let body = &app.doc().body;
	let mut bytes = vec![0u8; body.len() as usize];
	let n = body.read(s, 0, &mut bytes);
	bytes[..n]
		.iter()
		.enumerate()
		.filter(|(_, b)| **b == 0x0D)
		.map(|(i, _)| {
			body.pfmt_at(s, i as u32)
				.and_then(|ix| app.doc().pfmts.get(s, ix))
				.map_or(0, |p| p.flags)
		})
		.collect()
}

#[test]
fn enter_at_the_end_of_a_page_break_paragraph_makes_a_paragraph_with_no_break() {
	let (mut app, mut s, mut f) = chapter_with_break(12);
	assert_eq!(flags(&app, &s), vec![0, BREAK]);
	app.key(&mut s, &mut f, Key::Enter);
	assert_eq!(flags(&app, &s), vec![0, BREAK, 0]);
	assert_eq!(app.status_position(&s).0, 2);
	// Every further Enter stays on the same page.
	app.key(&mut s, &mut f, Key::Enter);
	assert_eq!(flags(&app, &s), vec![0, BREAK, 0, 0]);
	assert_eq!(app.status_position(&s).0, 2);
}

#[test]
fn enter_in_the_middle_keeps_the_break_on_the_first_half_only() {
	let (mut app, mut s, mut f) = chapter_with_break(7);
	app.key(&mut s, &mut f, Key::Enter);
	assert_eq!(flags(&app, &s), vec![0, BREAK, 0]);
	assert_eq!(app.status_position(&s).0, 2);
}

#[test]
fn enter_at_the_start_leaves_one_break_above_the_text() {
	let (mut app, mut s, mut f) = chapter_with_break(0);
	app.key(&mut s, &mut f, Key::Enter);
	// The empty paragraph above carries nothing; the text keeps the break, so there is still one.
	assert_eq!(flags(&app, &s), vec![0, 0, BREAK]);
	assert_eq!(
		flags(&app, &s).iter().filter(|f| **f & BREAK != 0).count(),
		1
	);
}

proptest! {
	#[test]
	fn enter_then_backspace_gives_the_flags_back(at in 0u32..13) {
		let (mut app, mut s, mut f) = chapter_with_break(at);
		let before = (flags(&app, &s), paragraphs(&app, &s));
		app.key(&mut s, &mut f, Key::Enter);
		app.key(&mut s, &mut f, Key::Backspace);
		prop_assert_eq!((flags(&app, &s), paragraphs(&app, &s)), before);
	}

	#[test]
	fn enter_never_adds_a_break(at in 0u32..13, presses in 1usize..4) {
		let (mut app, mut s, mut f) = chapter_with_break(at);
		for _ in 0..presses {
			app.key(&mut s, &mut f, Key::Enter);
		}
		prop_assert_eq!(flags(&app, &s).iter().filter(|f| **f & BREAK != 0).count(), 1);
	}
}
