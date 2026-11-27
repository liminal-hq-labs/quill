// The toolbar's and ruler's commands: face, size, colour, indents and tab stops, each undoable, and the chrome toggles that persist.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use quill_app::{App, Command, Dir, Key, CHROME_RULER};
use quill_host::script::MemFiles;
use quill_host::HostStore;

fn app_with(text: &str) -> (App, HostStore, MemFiles) {
	let mut s = HostStore::new();
	let mut f = MemFiles::default();
	let mut app = App::new();
	app.reset(&mut s).unwrap();
	for b in text.bytes() {
		app.key(&mut s, &mut f, Key::Char(b));
	}
	(app, s, f)
}

#[test]
fn face_size_and_colour_apply_to_the_selection_and_show_at_the_caret() {
	let (mut app, mut s, mut f) = app_with("hello");
	app.key(&mut s, &mut f, Key::Home);
	for _ in 0..5 {
		app.key(&mut s, &mut f, Key::Select(Dir::Right));
	}
	app.set_face(&mut s, b"Arial", 2);
	app.set_size(&mut s, 40);
	app.set_colour(&mut s, Some((255, 0, 0)));
	let cs = app.char_state(&s);
	assert_eq!(&cs.face[..usize::from(cs.face_len)], b"Arial");
	assert_eq!((cs.size, cs.colour), (40, Some((255, 0, 0))));
	app.set_colour(&mut s, None);
	assert_eq!(app.char_state(&s).colour, None);
	app.command(&mut s, &mut f, Command::Undo);
	assert_eq!(app.char_state(&s).colour, Some((255, 0, 0)));
}

#[test]
fn indents_and_tab_stops_set_from_the_ruler() {
	let (mut app, mut s, _f) = app_with("a line");
	app.set_indents(&mut s, 720, -360, 360);
	let pf = app.para_state(&s);
	assert_eq!((pf.left, pf.first, pf.right), (720, -360, 360));
	app.add_tab(&mut s, 1440, 1);
	app.add_tab(&mut s, 2880, 3);
	let mut tabs = [(0, 0); 16];
	assert_eq!(app.tab_stops(&s, &mut tabs), 2);
	assert_eq!(&tabs[..2], &[(1440, 1), (2880, 3)]);
	app.remove_tab(&mut s, 1440);
	assert_eq!(app.tab_stops(&s, &mut tabs), 1);
	assert_eq!(tabs[0], (2880, 3));
}

#[test]
fn hiding_the_ruler_is_remembered_in_quill_ini() {
	let (mut app, mut s, mut f) = app_with("");
	assert!(app.show_toolbar() && app.show_ruler() && app.show_status());
	app.command(&mut s, &mut f, Command::Chrome(CHROME_RULER));
	assert!(!app.show_ruler());
	app.save_settings(&mut f);
	let mut again = App::new();
	again.load_settings(&mut f);
	assert!(again.show_toolbar() && !again.show_ruler() && again.show_status());
}

#[test]
fn a_style_preset_is_one_undo_step_and_the_box_reads_it_back() {
	let (mut app, mut s, mut f) = app_with("Chapter one");
	app.key(&mut s, &mut f, Key::Enter);
	for b in b"Some text." {
		app.key(&mut s, &mut f, Key::Char(*b));
	}
	app.key(&mut s, &mut f, Key::CtrlHome);
	assert_eq!(app.style_of(&s), Some(0), "plain text is Normal");
	app.apply_style(&mut s, 1);
	assert_eq!(app.style_of(&s), Some(1));
	let cs = app.char_state(&s);
	assert_eq!(cs.size, 32);
	assert_ne!(cs.flags & 1, 0, "bold");
	// The second paragraph is untouched.
	app.key(&mut s, &mut f, Key::Down);
	assert_eq!(app.style_of(&s), Some(0));
	// A direct change makes the box blank.
	app.key(&mut s, &mut f, Key::CtrlHome);
	app.set_indents(&mut s, 100, 0, 0);
	assert_eq!(app.style_of(&s), None);
	// One Undo takes back the indent, one more takes back the whole preset.
	app.command(&mut s, &mut f, Command::Undo);
	assert_eq!(app.style_of(&s), Some(1));
	app.command(&mut s, &mut f, Command::Undo);
	assert_eq!(app.style_of(&s), Some(0));
	assert_eq!(app.char_state(&s).size, 24);
	// Every preset reads back as itself.
	for n in 0..quill_app::STYLE_NAMES.len() {
		app.apply_style(&mut s, n);
		assert_eq!(app.style_of(&s), Some(n), "{}", quill_app::STYLE_NAMES[n]);
	}
}

fn open_rtf(rtf: &str) -> (App, HostStore, MemFiles) {
	let mut s = HostStore::new();
	let mut f = MemFiles::default();
	let mut app = App::new();
	app.reset(&mut s).unwrap();
	f.files.insert("D.RTF".into(), rtf.as_bytes().to_vec());
	app.open(&mut s, &mut f, b"D.RTF").unwrap();
	app.key(&mut s, &mut f, Key::CtrlHome);
	(app, s, f)
}

/// The style box's reading of the one paragraph an RTF body holds.
fn box_of(body: &str) -> Option<usize> {
	let (app, s, _) = open_rtf(&format!("{{\\rtf1\\ansi {body}\\par}}"));
	app.style_of(&s)
}

#[test]
fn a_normal_paragraph_with_spacing_or_alignment_still_reads_normal() {
	assert_eq!(box_of("\\pard\\fs24 Body"), Some(0));
	assert_eq!(box_of("\\pard\\sa120\\fs24 Body"), Some(0));
	assert_eq!(box_of("\\pard\\sa120\\sl360\\slmult1\\fs24 Body"), Some(0));
	assert_eq!(box_of("\\pard\\qc\\fs24 Body"), Some(0));
	assert_eq!(box_of("\\pard\\qj\\sb200\\sa40\\fs24 Body"), Some(0));
	// A heading's own spacing is not what makes it one: its size and bold do.
	assert_eq!(box_of("\\pard\\sb240\\sa120\\fs32\\b Head"), Some(1));
	assert_eq!(box_of("\\pard\\sa0\\fs32\\b Head"), Some(1));
	assert_eq!(box_of("\\pard\\sa0\\fs28\\b Head"), Some(2));
}

#[test]
fn indents_size_and_character_bits_still_blank_the_box() {
	assert_eq!(box_of("\\pard\\li720\\fs24 Body"), Some(6));
	assert_eq!(box_of("\\pard\\li360\\fs24 Body"), None);
	assert_eq!(box_of("\\pard\\sa120\\fs26 Body"), None);
	assert_eq!(box_of("\\pard\\sa120\\fs24\\b Body"), None);
	assert_eq!(box_of("\\pard\\sa120\\fs24\\i Body"), None);
	assert_eq!(box_of("\\pard\\sa120\\fs24\\ul Body"), None);
}

#[test]
fn the_demo_documents_read_heading_1_and_normal() {
	for name in [
		"demo.rtf",
		"demo-reactos.rtf",
		"demo-reactos16.qld",
		"demo.qld",
	] {
		let path = format!("{}/../../samples/{name}", env!("CARGO_MANIFEST_DIR"));
		let mut s = HostStore::new();
		let mut f = MemFiles::default();
		f.files.insert("D".into(), std::fs::read(path).unwrap());
		let mut app = App::new();
		app.reset(&mut s).unwrap();
		app.open(&mut s, &mut f, b"D").unwrap();
		app.key(&mut s, &mut f, Key::CtrlHome);
		assert_eq!(app.style_of(&s), Some(1), "{name}: the opening heading");
		let mut normal = 0;
		for _ in 0..400 {
			match app.style_of(&s) {
				Some(0) => normal += 1,
				Some(1) => {}
				None => {}
				Some(n) => panic!("{name}: a paragraph reads {}", quill_app::STYLE_NAMES[n]),
			}
			let at = app.caret();
			app.key(&mut s, &mut f, Key::Down);
			if app.caret() == at {
				break;
			}
		}
		assert!(normal >= 3, "{name}: only {normal} Normal readings");
	}
}

#[test]
fn the_win16_string_resources_hold_the_zoom_tables_words() {
	use quill_chrome::zoom_field::{zoom_hint, zoom_tip, ZoomPart};
	// Windows 3.x loads the zoom parts' tooltips (31 to 34) and sentences (35 to 38) from its string table, in the order Page Width,
	// minus, box, plus, because its data segment has no room for a second copy; the table in `zoom_field` is the one source.
	let resources = include_str!("../../../win16/examples/quill/resources.json");
	let order = [
		ZoomPart::Reset,
		ZoomPart::Minus,
		ZoomPart::Field,
		ZoomPart::Plus,
	];
	for (place, part) in order.into_iter().enumerate() {
		let tip = format!("\"{}\": \"{}\"", 31 + place, zoom_tip(part));
		let hint = format!("\"{}\": \"{}\"", 35 + place, zoom_hint(part));
		assert!(resources.contains(&tip), "{tip}");
		assert!(resources.contains(&hint), "{hint}");
	}
}
