// Menu keyboard access: mnemonics in the tree, Alt and F10 on the text menu, and the BIOS key decoding behind the DOS build's Ctrl keys.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use quill_app::{bios, App, Key};
use quill_host::script::MemFiles;
use quill_host::HostStore;

fn setup() -> (App, HostStore, MemFiles) {
	let mut s = HostStore::new();
	let mut app = App::new();
	app.reset(&mut s).unwrap();
	(app, s, MemFiles::default())
}

fn italic(app: &App, s: &HostStore) -> bool {
	app.char_state(s).flags & quill_core::doc::cflag::ITALIC != 0
}

fn title(m: u8) -> String {
	String::from_utf8(quill_app::menu_title(m).to_vec()).unwrap()
}

#[test]
fn every_menu_and_item_has_one_mnemonic_and_none_clash() {
	let mut bar = std::collections::HashSet::new();
	for m in 0..quill_app::MENU_COUNT {
		let t = title(m);
		assert_eq!(t.matches('&').count(), 1, "{t}");
		assert!(bar.insert(quill_app::mnemonic(&t).unwrap()), "{t} clashes");
		let mut seen = std::collections::HashSet::new();
		for it in quill_app::menu_items(m) {
			assert_eq!(it.label.matches('&').count(), 1, "{}", it.label);
			assert!(
				seen.insert(quill_app::mnemonic(it.label).unwrap()),
				"{} clashes in {t}",
				it.label
			);
			let mut sub = std::collections::HashSet::new();
			for x in it.sub {
				assert_eq!(x.label.matches('&').count(), 1, "{}", x.label);
				assert!(
					sub.insert(quill_app::mnemonic(x.label).unwrap()),
					"{}",
					x.label
				);
			}
		}
	}
	assert_eq!(quill_app::MENU_COUNT, 9);
	let titles: Vec<String> = (0..9).map(|m| title(m).replace('&', "")).collect();
	assert_eq!(
		titles,
		[
			"File",
			"Edit",
			"Insert",
			"Find",
			"Character",
			"Paragraph",
			"Document",
			"View",
			"Help"
		]
	);
}

#[test]
fn alt_and_a_letter_opens_that_menu_and_a_letter_runs_its_item() {
	let (mut app, mut s, mut f) = setup();
	for m in 0..quill_app::MENU_COUNT {
		let c = quill_app::mnemonic(&title(m)).unwrap();
		app.key(&mut s, &mut f, Key::Alt(c));
		assert_eq!(app.menu_open(), Some((m, 0)));
		app.key(&mut s, &mut f, Key::Esc);
	}
	// Alt+h, then the letter of an item: Character > Italic, as Alt+C then I.
	app.key(&mut s, &mut f, Key::Alt(b'c'));
	app.key(&mut s, &mut f, Key::Char(b'I'));
	assert_eq!(app.menu_open(), None);
	assert!(italic(&app, &s), "Character > Italic took effect");
}

#[test]
fn f10_and_escape_enter_and_leave_the_bar_and_arrows_walk_it() {
	let (mut app, mut s, mut f) = setup();
	app.key(&mut s, &mut f, Key::F(10));
	assert_eq!(app.menu_open(), Some((0, 0)));
	app.key(&mut s, &mut f, Key::Right);
	app.key(&mut s, &mut f, Key::Down);
	assert_eq!(app.menu_open(), Some((1, 1)));
	app.key(&mut s, &mut f, Key::Esc);
	assert_eq!(app.menu_open(), None);
	// A shell with native menus never gets the text menu, so F10 and Alt+letter leave the editor alone.
	app.set_native_dialogs(true);
	app.key(&mut s, &mut f, Key::F(10));
	app.key(&mut s, &mut f, Key::Alt(b'f'));
	assert_eq!(app.menu_open(), None);
}

#[test]
fn the_bios_tells_ctrl_letters_from_tab_backspace_and_enter() {
	// Ctrl+I, Ctrl+H, Ctrl+M and Ctrl+J carry the control codes of Tab, Backspace, Enter and Line Feed; the scan code differs.
	assert_eq!(bios::decode(0x09, 0x17), Some(Key::Ctrl(b'i')));
	assert_eq!(bios::decode(0x09, 0x0F), Some(Key::Tab));
	assert_eq!(bios::decode(0x08, 0x23), Some(Key::Ctrl(b'h')));
	assert_eq!(bios::decode(0x08, 0x0E), Some(Key::Backspace));
	assert_eq!(bios::decode(0x0D, 0x32), Some(Key::Ctrl(b'm')));
	assert_eq!(bios::decode(0x0D, 0x1C), Some(Key::Enter));
	assert_eq!(bios::decode(0x0D, 0xE0), Some(Key::Enter));
	assert_eq!(bios::decode(0x0A, 0x24), Some(Key::Ctrl(b'j')));
	// Ctrl+Enter, Ctrl+Backspace and Ctrl+[ are not Ctrl+J, a character or Escape.
	assert_eq!(bios::decode(0x0A, 0x1C), None);
	assert_eq!(bios::decode(0x7F, 0x0E), None);
	assert_eq!(bios::decode(0x1B, 0x1A), None);
	assert_eq!(bios::decode(0x1B, 0x01), Some(Key::Esc));
	// Every other letter, with Ctrl, is its shortcut.
	for (i, c) in (b'a'..=b'z').enumerate() {
		if matches!(c, b'h' | b'i' | b'j' | b'm') {
			continue;
		}
		assert_eq!(bios::decode(i as u8 + 1, 0x10), Some(Key::Ctrl(c)));
	}
	// Alt with a letter, a digit or Backspace.
	assert_eq!(bios::decode(0, 0x21), Some(Key::Alt(b'f')));
	assert_eq!(bios::decode(0, 0x12), Some(Key::Alt(b'e')));
	assert_eq!(bios::decode(0, 0x2C), Some(Key::Alt(b'z')));
	assert_eq!(bios::decode(0, 0x78), Some(Key::Alt(b'1')));
	assert_eq!(bios::decode(0, 0x81), Some(Key::Alt(b'0')));
	assert_eq!(bios::decode(0, 0x0E), Some(Key::Alt(8)));
	assert_eq!(bios::decode(0, 0x3B), Some(Key::F(1)));
	assert_eq!(bios::decode(0, 0x44), Some(Key::F(10)));
}

#[test]
fn ctrl_i_toggles_italic_and_tab_still_tabs_through_the_bios_decoder() {
	let (mut app, mut s, mut f) = setup();
	let tab = bios::decode(0x09, 0x0F).unwrap();
	app.key(&mut s, &mut f, tab);
	let ital = bios::decode(0x09, 0x17).unwrap();
	app.key(&mut s, &mut f, ital);
	app.key(&mut s, &mut f, Key::Char(b'x'));
	assert_eq!(
		app.doc().body.len(),
		3,
		"one tab, one x and the paragraph mark: Ctrl+I typed nothing"
	);
	assert!(italic(&app, &s));
}

#[test]
fn the_windows_key_table_covers_the_shortcuts_the_menus_list() {
	use quill_app::accel::{from_vk, Accel};
	use quill_app::Command;
	// Ctrl+I, Ctrl+H and Ctrl+M are the letters, not Tab, Backspace and Enter; Ctrl+Backspace is nothing.
	assert_eq!(
		from_vk(0x49, true, false, false),
		Some(Accel::Key(Key::Ctrl(b'i')))
	);
	assert_eq!(
		from_vk(0x48, true, false, false),
		Some(Accel::Key(Key::Ctrl(b'h')))
	);
	assert_eq!(
		from_vk(0x4D, true, false, false),
		Some(Accel::Key(Key::Ctrl(b'm')))
	);
	assert_eq!(from_vk(0x08, true, false, false), None);
	assert_eq!(
		from_vk(0x08, false, false, true),
		Some(Accel::Command(Command::Undo))
	);
	// Ctrl+0 is Page Width, the key the View > Zoom item shows (CORE-206).
	assert_eq!(
		from_vk(0x30, true, false, false),
		Some(Accel::Command(Command::Zoom(0)))
	);
	assert_eq!(from_vk(0x30, false, false, false), None);
	assert_eq!(
		from_vk(0x31, true, false, false),
		Some(Accel::Command(Command::Spacing(0)))
	);
	assert_eq!(
		from_vk(0x32, true, false, false),
		Some(Accel::Command(Command::Spacing(2)))
	);
	assert_eq!(
		from_vk(0x35, true, false, false),
		Some(Accel::Command(Command::Spacing(1)))
	);
	assert_eq!(
		from_vk(0xBB, true, false, false),
		Some(Accel::Command(Command::Sub))
	);
	assert_eq!(
		from_vk(0xBB, true, true, false),
		Some(Accel::Command(Command::Super))
	);
	assert_eq!(
		from_vk(0x2E, false, true, false),
		Some(Accel::Command(Command::Cut))
	);
	assert_eq!(
		from_vk(0x2D, false, true, false),
		Some(Accel::Command(Command::Paste))
	);
	assert_eq!(
		from_vk(0x2D, true, false, false),
		Some(Accel::Command(Command::Copy))
	);
	// A plain letter is text.
	assert_eq!(from_vk(0x49, false, false, false), None);
}

#[test]
fn alt_0_from_the_bios_is_page_width_and_opens_no_menu() {
	// INT 16h gives AL 0, AH 81h for Alt+0 on the DOS screens (DOS-105).
	assert_eq!(bios::decode(0, 0x81), Some(Key::Alt(b'0')));
	let (mut app, mut s, mut f) = setup();
	app.set_zoom(150);
	app.key(&mut s, &mut f, bios::decode(0, 0x81).unwrap());
	assert_eq!(app.zoom(), 0);
	assert!(app.menu_open().is_none(), "no menu opened");
	assert!(app.file_view().is_none(), "no dialog opened");
	// The View > Zoom > Page Width item shows the key on the Windows builds.
	let key = (0..quill_app::MENU_COUNT)
		.flat_map(|m| quill_app::menu_items_for(m, true).iter())
		.flat_map(|it| it.sub.iter())
		.find(|it| it.command == quill_app::Command::Zoom(0))
		.map(|it| it.accel);
	assert_eq!(key, Some("Ctrl+0"));
}

#[test]
fn the_text_menu_leaves_out_print_preview_and_setup_but_windows_keeps_them() {
	let text: Vec<&str> = quill_app::menu_items_for(0, true)
		.iter()
		.map(|i| i.label)
		.collect();
	assert_eq!(
		text,
		[
			"&New",
			"&Open...",
			"&Save",
			"Save &As...",
			"&Print...",
			"E&xit"
		]
	);
	let win: Vec<&str> = quill_app::menu_items(0).iter().map(|i| i.label).collect();
	assert!(win.contains(&"Print Pre&view") && win.contains(&"Print Set&up..."));
	for it in quill_app::menu_items_for(0, true) {
		assert!(quill_app::mnemonic(it.label).is_some());
	}
}

#[test]
fn every_drop_down_and_its_shadow_lie_inside_the_screen() {
	for text in [true, false] {
		for m in 0..quill_app::MENU_COUNT {
			for recents in 0..=4 {
				if m != 0 && recents > 0 {
					continue;
				}
				let (col, w, rows) = quill_app::menu_box(m, text, recents);
				// The shadow is two columns to the right and one row below.
				assert!(col + w + 2 <= 80, "menu {m} reaches column {}", col + w + 2);
				assert!(1 + rows < 24, "menu {m} reaches row {}", 1 + rows + 1);
				// And it still starts under its title, or as near as the edge allows.
				assert!(col >= 1 || m == 0);
			}
		}
	}
	// Help, the last title, is the one that moved.
	let (col, w, _) = quill_app::menu_box(8, true, 0);
	assert_eq!(col + w + 2, 80);
}

fn screen(app: &App, s: &HostStore) -> Vec<String> {
	let mut g = quill_grid::Grid::<25>::new(0);
	quill_app::draw(app, s, &mut g);
	(0..25)
		.map(|y| {
			(0..80)
				.map(|x| char::from(g.get(x, y).unwrap().ch))
				.collect()
		})
		.collect()
}

#[test]
fn recent_files_are_listed_in_file_before_exit_and_in_no_other_menu() {
	let (mut app, mut s, mut f) = setup();
	let _ = app.save_as(&mut s, &mut f, b"LETTER.QLD");
	for m in 0..quill_app::MENU_COUNT {
		app.key(&mut s, &mut f, Key::Esc);
		app.key(&mut s, &mut f, Key::F(10));
		for _ in 0..m {
			app.key(&mut s, &mut f, Key::Right);
		}
		assert_eq!(app.menu_open().map(|x| x.0), Some(m));
		let rows = screen(&app, &s);
		let listed = rows.iter().position(|r| r.contains("1 LETTER.QLD"));
		if m == 0 {
			let at = listed.expect("File lists the recent file");
			let exit = rows.iter().position(|r| r.contains("Exit")).unwrap();
			let print = rows.iter().position(|r| r.contains("Print...")).unwrap();
			assert!(print < at && at < exit, "between the Print group and Exit");
			assert!(rows[at - 1].contains('\u{c3}') && rows[exit - 1].contains('\u{c3}'));
		} else {
			assert!(listed.is_none(), "menu {m} lists a recent file");
		}
		// The items of every menu are the tree's, recent files or not.
		let n = quill_app::menu_items_for(m, true).len();
		assert!(n > 0);
	}
	// Enter on the recent file's row opens it; the digit does too.
	assert_eq!(quill_app::Slot::Recent(0), quill_app::slot(6, 1, 5));
	assert_eq!(quill_app::Slot::Item(5), quill_app::slot(6, 1, 6));
	assert_eq!(quill_app::Slot::Item(4), quill_app::slot(6, 1, 4));
	assert_eq!(quill_app::item_for_mnemonic(0, b'1', 1, true), Some(5));
	assert_eq!(quill_app::item_for_mnemonic(1, b'1', 0, true), None);
}
