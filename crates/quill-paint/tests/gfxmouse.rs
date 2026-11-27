// The pointer on the graphics screen, beyond the page: menus, dialogs, the toolbars, the drop-down lists, the ruler and the scroll
// bars, each played through `Gfx` against the same geometry the painter draws with.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod common;

use common::*;
use proptest::prelude::*;
use quill_app::mouse::{Ctx, Ev};
use quill_app::{
	menu_items_for, Command, Key, Overlay, Wk, CHROME_RULER, CHROME_STATUS, MENU_COUNT,
};
use quill_chrome::view::{
	ruler_marks, tab_x, text_extent, Act, RulerState, COLOURS, SIZES, TOOLBAR,
};
use quill_core::doc::cflag;
use quill_core::layout::Measure;
use quill_host::script::MemFiles;
use quill_host::HostStore;
use quill_paint::drop::{self, Pop};
use quill_paint::layout::Layout;
use quill_paint::menu::{geom_of, title_rects};
use quill_paint::pointer::{Gfx, REPEAT_DELAY, REPEAT_STEP};
use quill_paint::shell::{self, hscroll};
use quill_paint::tip::{self, Target, TIP_TICKS};
use quill_paint::toolbar::{box_rect, button_rect, Box3};
use quill_paint::ui::ui_font;
use quill_paint::{dialog, scroll, Rect};
use quill_strike::StrikeMeasure;

struct Rig {
	app: TApp,
	s: HostStore,
	f: MemFiles,
	g: Gfx,
	tick: u32,
}

impl Rig {
	fn of(app: TApp, s: HostStore, f: MemFiles) -> Rig {
		let mut app = app;
		shell::sync_view(&mut app, &s);
		Rig {
			app,
			s,
			f,
			g: Gfx::new(&mut StrikeMeasure::new(strikes())),
			tick: 0,
		}
	}

	fn doc(name: &str) -> Rig {
		let (app, s, f) = open_doc(name);
		Rig::of(app, s, f)
	}

	fn ev(&mut self, ev: Ev) -> bool {
		let ctx = Ctx {
			shift: false,
			tick: self.tick,
		};
		self.g
			.handle(&mut self.app, &mut self.s, &mut self.f, ev, ctx)
	}

	fn mv(&mut self, x: i32, y: i32) -> bool {
		self.tick += 1;
		self.ev(Ev::Move { x, y })
	}

	fn down(&mut self, x: i32, y: i32) {
		self.mv(x, y);
		self.ev(Ev::Down { x, y });
	}

	fn up(&mut self, x: i32, y: i32) {
		self.mv(x, y);
		self.ev(Ev::Up { x, y });
	}

	/// A click, with time to spare after it so it is never half of a double click.
	fn click(&mut self, p: (i32, i32)) {
		self.down(p.0, p.1);
		self.up(p.0, p.1);
		self.tick += 100;
	}

	fn drag(&mut self, a: (i32, i32), b: (i32, i32)) {
		self.down(a.0, a.1);
		self.mv((a.0 + b.0) / 2, (a.1 + b.1) / 2);
		self.mv(b.0, b.1);
		self.up(b.0, b.1);
		self.tick += 100;
	}

	fn wait(&mut self, n: u32) -> bool {
		self.tick += n;
		self.ev(Ev::Tick)
	}

	fn key(&mut self, k: Key) {
		if !self.g.key(&mut self.app, &mut self.s, k) {
			self.app.key(&mut self.s, &mut self.f, k);
		}
		shell::sync_view(&mut self.app, &self.s);
		shell::follow_caret(&mut self.app, &self.s);
	}

	fn lay(&self) -> Layout {
		Layout::of(&self.app)
	}

	fn overlay(&self) -> Overlay {
		let mut o = Overlay::default();
		assert!(self.app.overlay(&self.s, &mut o), "a dialog is open");
		o
	}

	/// The centre of the widget whose text is `text` (a button or a check box, say).
	fn widget(&self, text: &[u8]) -> (usize, (i32, i32)) {
		let o = self.overlay();
		let (i, w) = o
			.widgets()
			.iter()
			.enumerate()
			.find(|(_, w)| w.kind != Wk::Label && w.bytes() == text)
			.unwrap_or_else(|| panic!("no widget {:?}", String::from_utf8_lossy(text)));
		let r = dialog::widget_rect(&o, w);
		(i, centre(r))
	}

	fn is_on(&self, text: &[u8]) -> bool {
		let o = self.overlay();
		o.widgets()
			.iter()
			.find(|w| w.kind != Wk::Label && w.bytes() == text)
			.is_some_and(|w| w.on)
	}
}

fn centre(r: Rect) -> (i32, i32) {
	((r.left + r.right) / 2, (r.top + r.bottom) / 2)
}

fn widths() -> [u16; 224] {
	let mut w = [0u16; 224];
	StrikeMeasure::new(strikes()).widths(&ui_font(), &mut w);
	w
}

fn adv(w: &[u16; 224]) -> impl Fn(u8) -> i32 + '_ {
	move |c| {
		c.checked_sub(0x20)
			.and_then(|i| w.get(usize::from(i)))
			.map_or(0, |v| i32::from(*v))
	}
}

fn title_centre(m: u8) -> (i32, i32) {
	let w = widths();
	centre(title_rects(adv(&w))[usize::from(m)])
}

fn row_centre(r: &Rig, menu: u8, item: usize) -> (i32, i32) {
	let w = widths();
	let g = geom_of(&r.app, menu, adv(&w)).unwrap();
	centre(g.item_rect(item).unwrap())
}

fn menu_index(menu: u8, c: Command) -> usize {
	menu_items_for(menu, true)
		.iter()
		.position(|i| i.command == c)
		.unwrap_or_else(|| panic!("{c:?} is not in menu {menu}"))
}

const VIEW: u8 = 7;
const EDIT: u8 = 1;

// ---- menus ------------------------------------------------------------------------------------------------------------------------

#[test]
fn a_click_on_a_title_opens_its_menu_and_a_click_on_it_again_closes_it() {
	let mut r = Rig::doc("letter.rtf");
	for m in 0..MENU_COUNT {
		r.click(title_centre(m));
		assert_eq!(r.app.menu_open(), Some((m, 0)), "menu {m} opened");
		r.click(title_centre(m));
		assert_eq!(r.app.menu_open(), None, "menu {m} closed");
	}
}

#[test]
fn hovering_lights_the_item_and_another_title_switches_menus() {
	let mut r = Rig::doc("letter.rtf");
	r.click(title_centre(1));
	let at = row_centre(&r, 1, 3);
	assert!(r.mv(at.0, at.1), "a lit item needs a repaint");
	assert_eq!(r.app.menu_open(), Some((1, 3)));
	r.mv(title_centre(4).0, title_centre(4).1);
	assert_eq!(
		r.app.menu_open(),
		Some((4, 0)),
		"hovering a title opens that menu"
	);
	let at = row_centre(&r, 4, 2);
	r.mv(at.0, at.1);
	assert_eq!(r.app.menu_open(), Some((4, 2)));
	// The same item again changes nothing, so it needs no repaint.
	assert!(!r.mv(at.0 + 1, at.1));
}

#[test]
fn a_click_on_an_item_runs_it() {
	let mut r = Rig::doc("letter.rtf");
	let ruler = menu_index(VIEW, Command::Chrome(CHROME_RULER));
	assert!(r.app.show_ruler());
	r.click(title_centre(VIEW));
	r.click(row_centre(&r, VIEW, ruler));
	assert_eq!(r.app.menu_open(), None);
	assert!(!r.app.show_ruler(), "the View menu's item ran");
	// Hiding the ruler gave its rows to the page: the app was told the page's new height.
	assert_eq!(r.lay().ruler, None);
}

#[test]
fn a_click_outside_closes_the_menu_and_goes_no_further() {
	let mut r = Rig::doc("letter.rtf");
	let caret = r.app.caret();
	r.click(title_centre(EDIT));
	let page = r.lay().page;
	r.click((page.right - 40, page.top + 200));
	assert_eq!(r.app.menu_open(), None);
	assert_eq!(
		r.app.caret(),
		caret,
		"the click that closed it did not move the caret"
	);
	// A click on the frame of an open menu does nothing and leaves it open.
	r.click(title_centre(EDIT));
	let w = widths();
	let g = geom_of(&r.app, EDIT, adv(&w)).unwrap();
	r.click((g.rect.left, g.rect.top + 8));
	assert_eq!(r.app.menu_open().map(|m| m.0), Some(EDIT));
}

#[test]
fn a_greyed_item_does_nothing_and_the_menu_stays_open() {
	let mut r = Rig::doc("letter.rtf");
	let cut = menu_index(EDIT, Command::Cut);
	assert!(r.app.selection().is_none());
	r.click(title_centre(EDIT));
	let before = r.app.doc().body.len();
	r.click(row_centre(&r, EDIT, cut));
	assert_eq!(r.app.menu_open().map(|m| m.0), Some(EDIT), "still open");
	assert_eq!(r.app.doc().body.len(), before);
	// With a selection the same item runs and closes the menu.
	r.app.command(&mut r.s, &mut r.f, Command::SelectAll);
	r.click(row_centre(&r, EDIT, cut));
	assert_eq!(r.app.menu_open(), None);
	assert!(r.app.doc().body.len() < before, "Cut took the text");
}

#[test]
fn a_drag_from_a_title_through_to_an_item_runs_it_on_release() {
	let mut r = Rig::doc("letter.rtf");
	let ruler = menu_index(VIEW, Command::Chrome(CHROME_RULER));
	let to = row_centre(&r, VIEW, ruler);
	r.down(title_centre(VIEW).0, title_centre(VIEW).1);
	assert_eq!(
		r.app.menu_open().map(|m| m.0),
		Some(VIEW),
		"open while held"
	);
	r.mv(to.0, to.1);
	assert_eq!(
		r.app.menu_open(),
		Some((VIEW, ruler as u8)),
		"lit under the held button"
	);
	assert!(r.app.show_ruler(), "not run before the button is up");
	r.up(to.0, to.1);
	assert!(!r.app.show_ruler());
	assert_eq!(r.app.menu_open(), None);
}

#[test]
fn letting_go_over_the_title_keeps_the_menu_and_letting_go_elsewhere_closes_it() {
	let mut r = Rig::doc("letter.rtf");
	let t = title_centre(EDIT);
	r.down(t.0, t.1);
	r.up(t.0, t.1);
	assert_eq!(r.app.menu_open().map(|m| m.0), Some(EDIT));
	r.key(Key::Esc);
	let page = r.lay().page;
	r.down(t.0, t.1);
	r.up(page.left + 50, page.top + 50);
	assert_eq!(r.app.menu_open(), None);
}

#[test]
fn the_keys_work_on_a_menu_the_pointer_opened() {
	let mut r = Rig::doc("letter.rtf");
	r.click(title_centre(1));
	r.key(Key::Down);
	assert_eq!(r.app.menu_open(), Some((1, 1)));
	r.key(Key::Right);
	assert_eq!(r.app.menu_open(), Some((2, 0)));
	r.key(Key::Esc);
	assert_eq!(r.app.menu_open(), None);
}

// ---- dialogs ----------------------------------------------------------------------------------------------------------------------

#[test]
fn a_dialog_button_acts_on_release_over_it_and_shows_pressed_while_held() {
	let mut r = Rig::doc("letter.rtf");
	r.app.command(&mut r.s, &mut r.f, Command::Find);
	let (idx, at) = r.widget(b"Close");
	r.down(at.0, at.1);
	assert_eq!(r.g.view().widget, Some(idx));
	assert!(!r.app.editing(), "not closed by the press");
	// Out of the button it is raised again, and a release there cancels.
	r.mv(2, 2);
	assert_eq!(r.g.view().widget, None);
	r.up(2, 2);
	assert!(!r.app.editing());
	// Back over it and let go: it acts.
	r.down(at.0, at.1);
	r.mv(2, 2);
	r.mv(at.0, at.1);
	assert_eq!(r.g.view().widget, Some(idx));
	r.up(at.0, at.1);
	assert!(r.app.editing());
	assert_eq!(r.g.view().widget, None);
}

#[test]
fn the_title_bar_and_the_space_round_the_controls_do_nothing() {
	let mut r = Rig::doc("letter.rtf");
	r.app.command(&mut r.s, &mut r.f, Command::Replace);
	let o = r.overlay();
	let before: Vec<_> = o.widgets().iter().map(|w| (w.on, w.focus, w.len)).collect();
	let t = centre(dialog::title_rect(&o));
	r.drag(t, (t.0 + 80, t.1 + 40));
	r.click((2, 2));
	let o = r.overlay();
	let after: Vec<_> = o.widgets().iter().map(|w| (w.on, w.focus, w.len)).collect();
	assert_eq!(before, after);
	assert_eq!(dialog::window_rect(&o), dialog::window_rect(&r.overlay()));
}

#[test]
fn clicking_a_field_gives_it_the_focus_and_the_keys_then_type_there() {
	let mut r = Rig::doc("letter.rtf");
	r.app.command(&mut r.s, &mut r.f, Command::Replace);
	let o = r.overlay();
	let edits: Vec<usize> = (0..o.widgets().len())
		.filter(|i| o.widgets()[*i].kind == Wk::Edit)
		.collect();
	assert_eq!(edits.len(), 2);
	let second = centre(dialog::widget_rect(&o, &o.widgets()[edits[1]]));
	r.click(second);
	let o = r.overlay();
	assert!(o.widgets()[edits[1]].focus && !o.widgets()[edits[0]].focus);
	r.key(Key::Char(b'z'));
	let o = r.overlay();
	assert_eq!(o.widgets()[edits[1]].bytes(), b"z");
	// The caret stays at the end of what the field holds, wherever it was clicked.
	assert_eq!(usize::from(o.widgets()[edits[1]].caret), 1);
	r.key(Key::Esc);
	assert!(r.app.editing(), "Esc is the keyboard's, as before");
}

#[test]
fn check_boxes_toggle_on_release_and_buttons_find_the_text() {
	let mut r = Rig::doc("letter.rtf");
	r.app.command(&mut r.s, &mut r.f, Command::Find);
	assert!(!r.is_on(b"Match case"));
	r.click(r.widget(b"Match case").1);
	assert!(r.is_on(b"Match case"));
	r.click(r.widget(b"Match case").1);
	assert!(!r.is_on(b"Match case"));
	// Type a word and press Find Next: the match is selected.
	r.click(
		r.overlay()
			.widgets()
			.iter()
			.find(|w| w.kind == Wk::Edit)
			.map(|w| centre(dialog::widget_rect(&r.overlay(), w)))
			.unwrap(),
	);
	for c in b"Dear" {
		r.key(Key::Char(*c));
	}
	r.click(r.widget(b"Find Next").1);
	assert!(r.app.selection().is_some(), "Find Next ran");
	assert!(!r.app.editing(), "the dialog stays open");
}

#[test]
fn radio_buttons_and_check_boxes_in_the_fonts_dialog() {
	let mut r = Rig::doc("letter.rtf");
	r.app.command(&mut r.s, &mut r.f, Command::Fonts);
	r.click(r.widget(b"Times New Roman").1);
	assert!(r.is_on(b"Times New Roman") && !r.is_on(b"Arial"));
	r.click(r.widget(b"Arial").1);
	assert!(
		r.is_on(b"Arial") && !r.is_on(b"Times New Roman"),
		"one of the group is chosen"
	);
	r.click(r.widget(b"Courier New").1);
	assert!(r.is_on(b"Courier New") && !r.is_on(b"Arial"));
	let bold = r.is_on(b"Bold");
	r.click(r.widget(b"Bold").1);
	assert_eq!(r.is_on(b"Bold"), !bold);
	r.click(r.widget(b"Bold").1);
	assert_eq!(r.is_on(b"Bold"), bold);
	r.click(r.widget(b"Bold").1);
	r.click(r.widget(b"OK").1);
	assert!(r.app.editing());
	let cs = r.app.char_state(&r.s);
	assert_eq!(&cs.face[..usize::from(cs.face_len)], b"Courier New");
	assert_eq!(cs.flags & cflag::BOLD != 0, !bold);
}

#[test]
fn the_save_changes_question_answers_to_its_buttons() {
	let mut r = Rig::doc("plain.qld");
	r.key(Key::Char(b'x'));
	r.app.command(&mut r.s, &mut r.f, Command::Exit);
	r.click(r.widget(b"Cancel").1);
	assert!(
		r.app.editing() && !r.app.quit(),
		"Cancel goes back to the document"
	);
	r.app.command(&mut r.s, &mut r.f, Command::Exit);
	r.click(r.widget(b"No").1);
	assert!(r.app.quit(), "No leaves without saving");
}

#[test]
fn the_about_box_and_a_prompt_close_from_their_buttons() {
	let mut r = Rig::doc("letter.rtf");
	r.app.command(&mut r.s, &mut r.f, Command::About);
	r.click(r.widget(b"OK").1);
	assert!(r.app.editing());
	r.app.command(&mut r.s, &mut r.f, Command::Open);
	r.click(r.widget(b"Cancel").1);
	assert!(r.app.editing());
}

#[test]
fn a_suggestion_is_selected_by_a_click_and_taken_by_a_double_click() {
	let dic = std::fs::read(root().join("tests/docs/en_ca.dic")).unwrap();
	// A word the dictionary has several suggestions for.
	let mut found = None;
	for word in [
		"teh", "wich", "thier", "qick", "fxo", "recieve", "adn", "hte", "seperate",
	] {
		let (mut app, mut s, mut f) = new_app();
		for c in format!("The {word} fox").bytes() {
			app.key(&mut s, &mut f, Key::Char(c));
		}
		f.files.insert("EN_CA.DIC".to_string(), dic.clone());
		app.key(&mut s, &mut f, Key::CtrlHome);
		let mut r = Rig::of(app, s, f);
		r.app.command(&mut r.s, &mut r.f, Command::Spelling);
		let mut o = Overlay::default();
		if r.app.overlay(&r.s, &mut o)
			&& o.widgets()
				.iter()
				.filter(|w| w.kind == Wk::ListItem)
				.count() >= 2
		{
			found = Some((r, word));
			break;
		}
	}
	let (mut r, word) = found.expect("some misspelling has two suggestions");
	let o = r.overlay();
	let items: Vec<usize> = (0..o.widgets().len())
		.filter(|i| o.widgets()[*i].kind == Wk::ListItem)
		.collect();
	let second = centre(dialog::widget_rect(&o, &o.widgets()[items[1]]));
	r.click(second);
	let o = r.overlay();
	assert!(o.widgets()[items[1]].on && !o.widgets()[items[0]].on);
	// The wheel moves the selection.
	r.ev(Ev::Wheel(-1));
	assert!(r.overlay().widgets()[items[0]].on);
	// Two presses within a double click's time take the entry: the word is changed to it.
	let chosen = r.overlay().widgets()[items[0]].bytes().to_vec();
	let first = centre(dialog::widget_rect(&o, &o.widgets()[items[0]]));
	r.tick += 100;
	r.down(first.0, first.1);
	r.up(first.0, first.1);
	assert!(!r.app.editing(), "one press only selects");
	r.tick += 2;
	r.down(first.0, first.1);
	r.up(first.0, first.1);
	let mut text = Vec::new();
	for i in 0..r.app.doc().body.len() {
		if let Some(b) = r.app.doc().body.read_byte(&r.s, i) {
			text.push(b);
		}
	}
	let text = String::from_utf8_lossy(&text).to_string();
	assert!(
		!text.contains(word) && text.contains(&*String::from_utf8_lossy(&chosen)),
		"{word} became {}: {text}",
		String::from_utf8_lossy(&chosen)
	);
}

// ---- the toolbars -----------------------------------------------------------------------------------------------------------------

fn button_index(c: Command) -> usize {
	TOOLBAR
		.buttons
		.iter()
		.position(|b| b.act == Act::Cmd(c))
		.unwrap()
}

#[test]
fn a_toolbar_button_runs_on_release_and_a_toggle_follows_the_state() {
	let mut r = Rig::doc("plain.qld");
	r.app.command(&mut r.s, &mut r.f, Command::SelectAll);
	let i = button_index(Command::Bold);
	let at = centre(button_rect(&r.lay(), i).unwrap());
	let bold = |r: &Rig| r.app.char_state(&r.s).flags & cflag::BOLD != 0;
	let was = bold(&r);
	r.down(at.0, at.1);
	assert_eq!(r.g.view().button, Some(i), "pressed and shown sunk");
	assert_eq!(bold(&r), was, "nothing yet");
	r.up(at.0, at.1);
	assert_eq!(r.g.view().button, None);
	assert_ne!(bold(&r), was, "the release ran it");
	assert_eq!(
		quill_chrome::view::is_on(&r.app, &r.s, quill_chrome::view::Toggle::Bold),
		bold(&r),
		"the toggle follows the state"
	);
	// Press, slide off and let go: cancelled; slide off and back: still held.
	let now = bold(&r);
	r.down(at.0, at.1);
	r.mv(2, 2);
	assert_eq!(
		r.g.view().button,
		None,
		"raised while the pointer is off it"
	);
	r.mv(at.0, at.1);
	assert_eq!(r.g.view().button, Some(i));
	r.mv(300, 300);
	r.up(300, 300);
	assert_eq!(bold(&r), now, "released outside: cancelled");
}

#[test]
fn every_button_that_is_a_command_runs_it() {
	// New, Open, Save and the rest each do what their menu item does: the dialog or the change they leave behind is the same.
	for (b, want) in [
		(Command::Open, true),
		(Command::Find, false),
		(Command::Spelling, false),
		(Command::Bullets, false),
	] {
		let mut r = Rig::doc("letter.rtf");
		let Some(i) = TOOLBAR.buttons.iter().position(|x| x.act == Act::Cmd(b)) else {
			continue;
		};
		let at = centre(button_rect(&r.lay(), i).unwrap());
		r.click(at);
		let (mut a2, mut s2, mut f2) = open_doc("letter.rtf");
		a2.command(&mut s2, &mut f2, b);
		let mut o1 = Overlay::default();
		let mut o2 = Overlay::default();
		assert_eq!(
			r.app.overlay(&r.s, &mut o1),
			a2.overlay(&s2, &mut o2),
			"{b:?} leaves the same dialog open"
		);
		let _ = want;
		assert_eq!(r.app.doc().body.len(), a2.doc().body.len());
	}
}

#[test]
fn a_tip_shows_after_a_rest_and_goes_when_the_pointer_moves_or_a_key_is_typed() {
	let mut r = Rig::doc("letter.rtf");
	let i = button_index(Command::Bold);
	let at = centre(button_rect(&r.lay(), i).unwrap());
	r.mv(at.0, at.1);
	assert_eq!(r.g.view().tip, None);
	assert!(!r.wait(TIP_TICKS - 2));
	assert!(r.wait(2), "a repaint when the tip comes");
	assert_eq!(r.g.view().tip, Some(Target::Button(i)));
	let mut buf = [0u8; tip::TIP_MAX];
	let n = tip::text(Target::Button(i), &mut buf);
	assert_eq!(&buf[..n], b"Bold (Ctrl+B)");
	r.mv(at.0 + 1, at.1 + 1);
	assert_eq!(
		r.g.view().tip,
		Some(Target::Button(i)),
		"still on the same button"
	);
	let page = r.lay().page;
	r.mv(page.left + 20, page.top + 20);
	assert_eq!(r.g.view().tip, None, "gone when the pointer leaves");
	// A key takes it away and it stays away until the pointer rests on something else.
	r.mv(at.0, at.1);
	r.wait(TIP_TICKS);
	assert!(r.g.view().tip.is_some());
	r.key(Key::Left);
	assert_eq!(r.g.view().tip, None);
	r.wait(TIP_TICKS * 3);
	assert_eq!(r.g.view().tip, None);
	// A click hides it too.
	r.mv(page.left + 20, page.top + 20);
	r.mv(at.0, at.1);
	r.wait(TIP_TICKS);
	assert!(r.g.view().tip.is_some());
	r.down(at.0, at.1);
	assert_eq!(r.g.view().tip, None);
	r.up(at.0, at.1);
}

#[test]
fn each_part_of_the_zoom_control_has_a_tip_above_it() {
	use quill_chrome::zoom_field::ZoomPart;
	let mut r = Rig::doc("letter.rtf");
	let want = [
		(ZoomPart::Minus, &b"Zoom Out"[..]),
		(ZoomPart::Field, &b"Zoom (click for the list)"[..]),
		(ZoomPart::Plus, &b"Zoom In"[..]),
		(ZoomPart::Reset, &b"Page Width (Alt+0)"[..]),
	];
	for (part, text) in want {
		let a = quill_paint::zoombox::part_rect(&r.lay(), part).unwrap();
		let at = centre(a);
		r.mv(at.0, at.1);
		r.wait(TIP_TICKS);
		assert_eq!(r.g.view().tip, Some(Target::Zoom(part)));
		let mut buf = [0u8; tip::TIP_MAX];
		let n = tip::text(Target::Zoom(part), &mut buf);
		assert_eq!(&buf[..n], text);
		let t = tip::rect(a, 90, 18);
		assert!(t.bottom <= a.top, "the tip clears the control");
		assert!(t.left >= 0 && t.right <= 640);
		let page = r.lay().page;
		r.mv(page.left + 20, page.top + 20);
		assert_eq!(r.g.view().tip, None);
	}
}

#[test]
fn every_button_and_box_has_a_tip_and_the_shortcuts_come_from_the_menus() {
	for (i, b) in TOOLBAR.buttons.iter().enumerate() {
		let mut buf = [0u8; tip::TIP_MAX];
		let n = tip::text(Target::Button(i), &mut buf);
		assert!(n > 0, "button {i} has a tip");
		if let Act::Cmd(c) = b.act {
			let key = tip::shortcut(c);
			if !key.is_empty() {
				assert!(buf[..n].ends_with(format!("({key})").as_bytes()));
			}
		}
	}
	for b in [Box3::Style, Box3::Face, Box3::Size] {
		let mut buf = [0u8; tip::TIP_MAX];
		assert!(tip::text(Target::Box(b), &mut buf) > 0);
	}
	assert_eq!(tip::shortcut(Command::Undo), "Ctrl+Z");
	assert_eq!(tip::shortcut(Command::Align(2)), "Ctrl+R");
	assert_eq!(tip::shortcut(Command::Recent(0)), "");
	// The box sits under its target and stays on the screen.
	for a in [Rect::new(0, 20, 24, 44), Rect::new(600, 440, 624, 464)] {
		let t = tip::rect(a, 90, 18);
		assert!(t.left >= 0 && t.right <= 640 && t.top >= 0 && t.bottom <= 480);
	}
}

#[test]
fn the_drop_down_lists_choose_the_style_the_face_the_size_and_the_colour() {
	let mut r = Rig::doc("letter.rtf");
	r.app.command(&mut r.s, &mut r.f, Command::SelectAll);
	// The size list: open from the box, hover, choose.
	let b = centre(box_rect(&r.lay(), Box3::Size).unwrap());
	r.click(b);
	let (pop, _) = r.g.view().drop.expect("the list opened");
	assert_eq!(pop, Pop::Size);
	let list = drop::list_rect(&r.lay(), Pop::Size).unwrap();
	let want = SIZES.iter().position(|p| *p == 20).unwrap();
	let at = centre(drop::item_rect(list, want));
	r.mv(at.0, at.1);
	assert_eq!(r.g.view().drop, Some((Pop::Size, want)));
	r.click(at);
	assert_eq!(r.g.view().drop, None);
	assert_eq!(r.app.char_state(&r.s).size, 40);
	// The face list.
	r.click(centre(box_rect(&r.lay(), Box3::Face).unwrap()));
	let list = drop::list_rect(&r.lay(), Pop::Face).unwrap();
	r.click(centre(drop::item_rect(list, 2)));
	let cs = r.app.char_state(&r.s);
	assert_eq!(&cs.face[..usize::from(cs.face_len)], b"Courier New");
	// The style list.
	r.click(centre(box_rect(&r.lay(), Box3::Style).unwrap()));
	let list = drop::list_rect(&r.lay(), Pop::Style).unwrap();
	r.click(centre(drop::item_rect(list, 1)));
	assert_eq!(r.app.style_of(&r.s), Some(1));
	// The colour button's list, opened when the button is let go.
	let ci = TOOLBAR
		.buttons
		.iter()
		.position(|b| b.act == Act::Colour)
		.unwrap();
	r.click(centre(button_rect(&r.lay(), ci).unwrap()));
	assert_eq!(r.g.view().drop.map(|d| d.0), Some(Pop::Colour));
	let list = drop::list_rect(&r.lay(), Pop::Colour).unwrap();
	r.click(centre(drop::item_rect(list, 10)));
	assert_eq!(r.app.char_state(&r.s).colour, Some(COLOURS[9].1));
	// Automatic puts it back.
	r.click(centre(button_rect(&r.lay(), ci).unwrap()));
	let list = drop::list_rect(&r.lay(), Pop::Colour).unwrap();
	r.click(centre(drop::item_rect(list, 0)));
	assert_eq!(r.app.char_state(&r.s).colour, None);
}

#[test]
fn a_list_closes_on_a_click_elsewhere_the_keys_and_the_wheel_move_in_it() {
	let mut r = Rig::doc("letter.rtf");
	let b = centre(box_rect(&r.lay(), Box3::Size).unwrap());
	let size = r.app.char_state(&r.s).size;
	r.click(b);
	assert!(r.g.view().drop.is_some());
	let caret = r.app.caret();
	let page = r.lay().page;
	r.click((page.left + 80, page.top + 80));
	assert_eq!(r.g.view().drop, None, "a click elsewhere shuts it");
	assert_eq!(r.app.caret(), caret, "and goes no further");
	r.click(b);
	r.click(b);
	assert_eq!(r.g.view().drop, None, "the box again shuts it");
	r.click(b);
	let (_, hot) = r.g.view().drop.unwrap();
	r.ev(Ev::Wheel(2));
	assert_eq!(r.g.view().drop.unwrap().1, (hot + 2).min(SIZES.len() - 1));
	r.key(Key::Up);
	r.key(Key::Esc);
	assert_eq!(r.g.view().drop, None);
	assert_eq!(r.app.char_state(&r.s).size, size, "Esc chose nothing");
	// Enter takes the lit entry.
	r.click(b);
	r.key(Key::Down);
	r.key(Key::Enter);
	assert_eq!(r.g.view().drop, None);
	assert_ne!(r.app.char_state(&r.s).size, size);
}

#[test]
fn a_drag_from_the_box_through_the_list_chooses_on_release() {
	let mut r = Rig::doc("letter.rtf");
	r.app.command(&mut r.s, &mut r.f, Command::SelectAll);
	let b = centre(box_rect(&r.lay(), Box3::Size).unwrap());
	let list = drop::list_rect(&r.lay(), Pop::Size).unwrap();
	let to = centre(drop::item_rect(list, 3));
	r.drag(b, to);
	assert_eq!(r.g.view().drop, None);
	assert_eq!(r.app.char_state(&r.s).size, SIZES[3] * 2);
}

// ---- the ruler --------------------------------------------------------------------------------------------------------------------

fn ruler_geometry(r: &Rig) -> (Rect, i32, i32) {
	let lay = r.lay();
	let ruler = lay.ruler.unwrap();
	let w = lay.page.width();
	let (left, tw) = text_extent(&r.app, w, hscroll(&r.app, w));
	(ruler, left, tw)
}

#[test]
fn a_click_on_the_scale_sets_a_tab_and_a_drag_of_the_stop_moves_or_clears_it() {
	let mut r = Rig::doc("plain.qld");
	let (ruler, left, _) = ruler_geometry(&r);
	let stops = |r: &Rig| RulerState::of(&r.app, &r.s).stops().to_vec();
	assert!(stops(&r).is_empty());
	r.click((left + 100, ruler.top + 12));
	assert_eq!(stops(&r).len(), 1);
	let pos = stops(&r)[0].0;
	assert_eq!(pos % 90, 0, "snapped to a sixteenth of an inch");
	assert!((pos / 15 - 100).abs() <= 3);
	// Drag it along the ruler: it moves.
	let x = tab_x(pos, left);
	r.drag((x, ruler.top + 12), (x + 60, ruler.top + 12));
	let moved = stops(&r);
	assert_eq!(moved.len(), 1);
	assert!(
		moved[0].0 > pos + 600,
		"the stop followed the pointer: {moved:?}"
	);
	// Drag it off the ruler: it is cleared.
	let x = tab_x(moved[0].0, left);
	r.drag((x, ruler.top + 12), (x, ruler.top + 80));
	assert!(stops(&r).is_empty());
}

#[test]
fn the_ruler_shows_a_drag_before_the_app_has_it_and_the_markers_set_the_indents() {
	let mut r = Rig::doc("plain.qld");
	let (ruler, left, tw) = ruler_geometry(&r);
	let st = RulerState::of(&r.app, &r.s);
	let (fx, lx, _) = ruler_marks(&st, left, tw);
	r.down(fx, ruler.top + 3);
	r.mv(fx + 48, ruler.top + 3);
	assert_eq!(r.g.view().ruler.map(|s| s.first), Some(720), "the preview");
	assert_eq!(
		r.app.para_state(&r.s).first,
		0,
		"the app is not told until the button is up"
	);
	r.up(fx + 48, ruler.top + 3);
	assert_eq!(r.app.para_state(&r.s).first, 720);
	assert!(r.g.view().ruler.is_none());
	// The left indent marker, grabbed even at the left edge where a tab-kind button would be on other shells.
	let st = RulerState::of(&r.app, &r.s);
	let (_, lx2, _) = ruler_marks(&st, left, tw);
	assert_eq!(lx, lx2);
	r.drag((lx, ruler.top + 15), (lx + 30, ruler.top + 15));
	assert_eq!(r.app.para_state(&r.s).left, 450);
}

// ---- the horizontal bar, auto-repeat, the fields and the ruler's kind button ----------------------------------------------------

/// A document of one line of words, on a Letter sheet with margins of `margin` twips: a text column of 8.5 inches less both of them.
fn margined(margin: i32, body: &str) -> Rig {
	let rtf = format!(
		"{{\\rtf1\\ansi\\paperw12240\\paperh15840\\margl{margin}\\margr{margin}\\margt1440\\margb1440 {body}\\par}}"
	);
	let (app, s, f) = app_with("wide.rtf", rtf.as_bytes()).unwrap();
	Rig::of(app, s, f)
}

fn words(n: usize) -> String {
	"word ".repeat(n)
}

fn hparts(r: &Rig) -> scroll::Parts {
	let w = r.lay().page.width();
	let range = shell::hrange(&r.app, &r.s, w);
	scroll::parts(
		r.lay().hbar.unwrap(),
		false,
		r.app.hscroll_pos(),
		range + w,
		w,
	)
}

#[test]
fn a_page_whose_text_fits_has_a_disabled_horizontal_bar_that_does_nothing() {
	let mut r = Rig::doc("letter.rtf");
	let w = r.lay().page.width();
	assert_eq!(shell::hrange(&r.app, &r.s, w), 0);
	let base = hscroll(&r.app, w);
	let p = hparts(&r);
	for at in [
		centre(p.up),
		centre(p.down),
		(p.track.left + 3, p.track.top + 3),
	] {
		r.click(at);
		r.down(at.0, at.1);
		r.wait(60);
		r.up(at.0, at.1);
	}
	r.ev(Ev::Wheel(0));
	assert_eq!(r.app.hscroll_pos(), 0);
	assert_eq!(
		hscroll(&r.app, w),
		base,
		"the text column stays at the left edge"
	);
	assert!(!r.g.holding());
}

#[test]
fn narrow_margins_make_the_text_wider_than_the_view_and_the_bar_scrolls_it() {
	// A half-inch margin gives a column 7.5 inches (720 pixels) wide in a view 624 wide.
	let mut r = margined(720, &words(200));
	let w = r.lay().page.width();
	assert_eq!(
		shell::hrange(&r.app, &r.s, w),
		720 - w + (to_px(720) + 8 - hbase(&r))
	);
	let range = shell::hrange(&r.app, &r.s, w);
	assert!(range > 0 && range < 140, "{range}");
	let p = hparts(&r);
	assert!(
		p.thumb.width() < p.track.width(),
		"a thumb that is a part of the track"
	);
	assert_eq!(p.thumb.left, p.track.left);
	let (left0, _) = text_extent(&r.app, w, hscroll(&r.app, w));
	r.click(centre(p.down));
	assert_eq!(r.app.hscroll_pos(), 16, "the right arrow is sixteen pixels");
	let (left1, _) = text_extent(&r.app, w, hscroll(&r.app, w));
	assert_eq!(left1, left0 - 16, "the ruler and the page move with it");
	r.click(centre(hparts(&r).up));
	assert_eq!(r.app.hscroll_pos(), 0);
	r.click(centre(hparts(&r).up));
	assert_eq!(
		r.app.hscroll_pos(),
		0,
		"never past the text column's left edge"
	);
	// The track right of the thumb is a page.
	let p = hparts(&r);
	r.click((p.thumb.right + 4, p.thumb.top + 4));
	assert_eq!(r.app.hscroll_pos(), range.min(w - 32));
	// The thumb dragged to the end, and back.
	let p = hparts(&r);
	let grab = (p.thumb.left + 3, p.thumb.top + 3);
	r.drag(grab, (p.track.right + 50, grab.1));
	assert_eq!(r.app.hscroll_pos(), range);
	let p = hparts(&r);
	assert_eq!(
		p.thumb.right, p.track.right,
		"the thumb is at the end of its track"
	);
	r.drag((p.thumb.left + 3, p.thumb.top + 3), (0, p.thumb.top + 3));
	assert_eq!(r.app.hscroll_pos(), 0);
}

fn to_px(twips: i32) -> i32 {
	quill_core::layout::to_units(twips, 96)
}

fn hbase(r: &Rig) -> i32 {
	hscroll(&r.app, r.lay().page.width()) - r.app.hscroll_pos()
}

#[test]
fn shift_and_the_wheel_scroll_sideways_and_the_plain_wheel_does_not() {
	let mut r = margined(720, &words(300));
	let range = shell::hrange(&r.app, &r.s, r.lay().page.width());
	let ctx = |shift| Ctx { shift, tick: 0 };
	r.g.handle(&mut r.app, &mut r.s, &mut r.f, Ev::Wheel(1), ctx(true));
	assert_eq!(r.app.hscroll_pos(), 48, "three arrow steps a notch");
	r.g.handle(&mut r.app, &mut r.s, &mut r.f, Ev::Wheel(1), ctx(true));
	assert_eq!(r.app.hscroll_pos(), range, "stops at the end");
	r.g.handle(&mut r.app, &mut r.s, &mut r.f, Ev::Wheel(-100), ctx(true));
	assert_eq!(r.app.hscroll_pos(), 0);
	let top = r.app.top_line();
	r.g.handle(&mut r.app, &mut r.s, &mut r.f, Ev::Wheel(2), ctx(false));
	assert_eq!(r.app.hscroll_pos(), 0, "the wheel alone is the page's");
	assert!(r.app.top_line() >= top);
}

#[test]
fn the_caret_scrolls_into_view_sideways_and_a_bar_that_moved_away_stays_away() {
	let mut r = margined(720, &words(120));
	r.key(Key::Home);
	assert_eq!(r.app.hscroll_pos(), 0);
	r.key(Key::End);
	r.key(Key::Left);
	let w = r.lay().page.width();
	let range = shell::hrange(&r.app, &r.s, w);
	assert!(
		r.app.hscroll_pos() > 0,
		"the end of a wide line is off the right edge"
	);
	let (_, _, x) = r.app.caret_xy(&r.s).unwrap();
	let at = 8 + to_px(720) + x - hscroll(&r.app, w);
	assert!((0..=w).contains(&at), "the caret is in the area at {at}");
	r.key(Key::Home);
	assert_eq!(r.app.hscroll_pos(), 0, "and Home brings it back");
	// The bar scrolls the view away from the caret, and a click elsewhere does not bring it back.
	r.click(centre(hparts(&r).down));
	let moved = r.app.hscroll_pos();
	assert!(moved > 0 && moved <= range);
	r.click((300, 470));
	assert_eq!(r.app.hscroll_pos(), moved);
}

#[test]
fn a_tab_stop_or_a_picture_wider_than_the_column_widens_the_content() {
	// A tab stop at 7.5 inches on a column of 6.5: the line runs to the stop and past the column.
	let r = margined(1440, "a\\tab b");
	let w = r.lay().page.width();
	assert_eq!(shell::hrange(&r.app, &r.s, w), 0, "no tab stop yet");
	let rtf = "{\\rtf1\\ansi\\paperw12240\\paperh15840\\margl1440\\margr1440\\margt1440\\margb1440 \\tx10800 a\\tab b\\par}";
	let (app, s, f) = app_with("tab.rtf", rtf.as_bytes()).unwrap();
	let r = Rig::of(app, s, f);
	assert!(
		shell::hrange(&r.app, &r.s, w) > 0,
		"the line reaches past the column"
	);
	// The sideways scroll is what the lines shown need, so the shell never leaves it past the range.
	let mut r = r;
	r.app.set_hscroll_pos(5000);
	shell::sync_view(&mut r.app, &r.s);
	assert_eq!(r.app.hscroll_pos(), shell::hrange(&r.app, &r.s, w));
}

/// How far the page is scrolled, in pixels.
fn px(r: &Rig) -> i32 {
	r.app.vscroll_pos(&r.s)
}

/// What an arrow of the vertical bar moves, in pixels.
const LINE: i32 = quill_chrome::view::SCROLL_LINE;

fn hold_at(r: &mut Rig, at: (i32, i32)) -> u32 {
	r.down(at.0, at.1);
	r.tick
}

#[test]
fn a_held_arrow_repeats_after_a_delay_and_stops_on_release() {
	let mut r = long_doc();
	let p = bar(&r);
	let at = centre(p.down);
	let t0 = hold_at(&mut r, at);
	assert_eq!(px(&r), LINE, "the press is one line at once");
	r.wait(REPEAT_DELAY - 1);
	assert_eq!(px(&r), LINE, "nothing before the delay");
	r.wait(1);
	assert_eq!(px(&r), 2 * LINE, "the first repeat comes at the delay");
	r.wait(5);
	assert_eq!(
		px(&r),
		(2 + 5 / REPEAT_STEP) as i32 * LINE,
		"then one a step"
	);
	r.up(at.0, at.1);
	let after = px(&r);
	r.wait(200);
	assert_eq!(px(&r), after, "released: no more");
	assert!(r.tick > t0);
	assert!(!r.g.holding());
}

#[test]
fn a_held_arrow_pauses_off_the_arrow_and_goes_on_when_the_pointer_returns() {
	let mut r = long_doc();
	let at = centre(bar(&r).down);
	hold_at(&mut r, at);
	r.wait(REPEAT_DELAY);
	let on = px(&r);
	r.mv(300, 300);
	r.wait(20);
	assert_eq!(px(&r), on, "the pointer left the arrow");
	r.mv(at.0, at.1);
	r.wait(3);
	assert!(px(&r) > on, "back on it, it goes on");
	r.up(at.0, at.1);
}

#[test]
fn a_held_track_repeats_pages_and_stops_where_the_thumb_reaches_the_pointer() {
	let mut r = long_doc();
	let p = bar(&r);
	let at = (p.thumb.left + 4, p.thumb.bottom + 20);
	hold_at(&mut r, at);
	let first = px(&r);
	assert!(first > LINE, "a page");
	r.wait(REPEAT_DELAY + 400);
	let top = px(&r);
	let p = bar(&r);
	assert!(
		p.thumb.top <= at.1 && at.1 < p.thumb.bottom + 1 + 20,
		"the thumb has come to the pointer"
	);
	r.wait(100);
	assert_eq!(px(&r), top, "and the track stops there");
	r.up(at.0, at.1);
	assert!(top < r.app.vscroll_max(&r.s), "short of the end");
}

#[test]
fn the_horizontal_bar_repeats_too_and_never_runs_past_its_end() {
	let mut r = margined(720, &words(200));
	let range = shell::hrange(&r.app, &r.s, r.lay().page.width());
	let at = centre(hparts(&r).down);
	hold_at(&mut r, at);
	assert_eq!(r.app.hscroll_pos(), 16);
	r.wait(REPEAT_DELAY);
	assert_eq!(r.app.hscroll_pos(), 32);
	r.wait(500);
	assert_eq!(r.app.hscroll_pos(), range);
	r.up(at.0, at.1);
}

#[test]
fn a_clock_that_jumps_runs_a_bounded_number_of_repeats() {
	let mut r = long_doc();
	let at = centre(bar(&r).down);
	hold_at(&mut r, at);
	r.wait(1_000_000);
	assert!(px(&r) <= (1 + 64 + 1) * LINE, "{}", px(&r));
	r.up(at.0, at.1);
}

#[test]
fn the_ruler_button_cycles_the_kind_and_the_scale_sets_a_tab_of_that_kind() {
	let mut r = Rig::doc("plain.qld");
	let (ruler, left, _) = ruler_geometry(&r);
	let kind = r.lay().ruler.unwrap();
	assert_eq!(r.g.view().kind, 0);
	let button = (4, ruler.top + 10);
	for want in [1, 2, 3, 0, 1] {
		r.click(button);
		assert_eq!(r.g.view().kind, want);
	}
	// A click on the scale makes a tab of the kind the button shows (1, centre).
	r.click((left + 200, kind.top + 12));
	let st = RulerState::of(&r.app, &r.s);
	assert_eq!(st.stops().iter().map(|t| t.1).collect::<Vec<_>>(), vec![1]);
	r.click(button);
	r.click(button);
	assert_eq!(r.g.view().kind, 3);
	r.click((left + 300, kind.top + 12));
	let st = RulerState::of(&r.app, &r.s);
	let kinds: Vec<u8> = st.stops().iter().map(|t| t.1).collect();
	assert!(kinds.contains(&1) && kinds.contains(&3), "{kinds:?}");
	// The markers and stops come first where they overlap the button: the first-line marker at the left edge is still grabbed.
	let st = RulerState::of(&r.app, &r.s);
	let (fx, _, _) = ruler_marks(&st, left, 624);
	r.down(fx, ruler.top + 3);
	assert!(r.g.holding());
	r.up(fx, ruler.top + 3);
	assert_eq!(
		r.g.view().kind,
		3,
		"that press was the marker's, not the button's"
	);
}

fn edit_of(r: &Rig, which: usize) -> (usize, Rect) {
	let o = r.overlay();
	let i = (0..o.widgets().len())
		.filter(|i| o.widgets()[*i].kind == Wk::Edit)
		.nth(which)
		.unwrap();
	(i, dialog::widget_rect(&o, &o.widgets()[i]))
}

/// The x of the gap before character `n` of the text of edit field `which`, as the painter lays the field out.
fn gap_x(r: &Rig, which: usize, n: usize) -> i32 {
	let (i, rect) = edit_of(r, which);
	let o = r.overlay();
	let w = &o.widgets()[i];
	let wd = widths();
	let a = adv(&wd);
	let (x0, _) = dialog::field_text_box(rect);
	x0 + w.bytes()[..n].iter().map(|c| a(*c)).sum::<i32>()
}

fn caret_of(r: &Rig, which: usize) -> (usize, usize) {
	let (i, _) = edit_of(r, which);
	let w = r.overlay().widgets()[i];
	(usize::from(w.caret), usize::from(w.anchor))
}

fn type_into(r: &mut Rig, text: &[u8]) {
	for c in text {
		r.key(Key::Char(*c));
	}
}

#[test]
fn a_click_in_a_field_puts_the_caret_between_the_characters_and_the_keys_edit_there() {
	let mut r = Rig::doc("letter.rtf");
	r.app.command(&mut r.s, &mut r.f, Command::Find);
	let (_, rect) = edit_of(&r, 0);
	r.click(centre(rect));
	type_into(&mut r, b"hello world");
	assert_eq!(caret_of(&r, 0), (11, 11));
	// A press before the 'w' (character 6).
	let y = centre(rect).1;
	r.click((gap_x(&r, 0, 6), y));
	assert_eq!(caret_of(&r, 0), (6, 6));
	// Between two characters it takes the nearer gap.
	r.click((gap_x(&r, 0, 3) + 1, y));
	assert_eq!(caret_of(&r, 0).0, 3);
	r.click((gap_x(&r, 0, 3) - 1, y));
	assert_eq!(caret_of(&r, 0).0, 3);
	// Past the end of the text, the end; before the start, the start.
	r.click((rect.right - 3, y));
	assert_eq!(caret_of(&r, 0).0, 11);
	r.click((rect.left + 1, y));
	assert_eq!(caret_of(&r, 0).0, 0);
	// The keys act at the caret.
	r.key(Key::Char(b'>'));
	r.key(Key::Right);
	r.key(Key::Right);
	r.key(Key::Delete);
	let o = r.overlay();
	assert_eq!(o.widgets()[edit_of(&r, 0).0].bytes(), b">helo world");
	assert_eq!(caret_of(&r, 0), (3, 3));
	r.key(Key::Backspace);
	assert_eq!(
		r.overlay().widgets()[edit_of(&r, 0).0].bytes(),
		b">hlo world"
	);
	r.key(Key::Home);
	assert_eq!(caret_of(&r, 0), (0, 0));
	r.key(Key::End);
	assert_eq!(caret_of(&r, 0), (10, 10));
}

#[test]
fn a_drag_selects_text_and_typing_replaces_it_and_a_double_click_selects_a_word() {
	let mut r = Rig::doc("letter.rtf");
	r.app.command(&mut r.s, &mut r.f, Command::Find);
	let (_, rect) = edit_of(&r, 0);
	let y = centre(rect).1;
	r.click(centre(rect));
	type_into(&mut r, b"alpha beta gamma");
	r.drag((gap_x(&r, 0, 2), y), (gap_x(&r, 0, 8), y));
	assert_eq!(
		caret_of(&r, 0),
		(8, 2),
		"caret where the drag ended, anchor where it began"
	);
	r.key(Key::Char(b'-'));
	assert_eq!(
		r.overlay().widgets()[edit_of(&r, 0).0].bytes(),
		b"al-ta gamma"
	);
	// A drag to the left of where it began selects backwards.
	r.drag((gap_x(&r, 0, 6), y), (gap_x(&r, 0, 1), y));
	assert_eq!(caret_of(&r, 0), (1, 6));
	// A double click on a word selects it: the 'gamma' (characters 6 to 11).
	let mid = (gap_x(&r, 0, 8) + gap_x(&r, 0, 9)) / 2;
	r.down(mid, y);
	r.up(mid, y);
	r.down(mid, y);
	r.up(mid, y);
	assert_eq!(caret_of(&r, 0), (11, 6));
	// Shift with the arrows extends it from the caret.
	r.key(Key::Home);
	r.key(Key::Select(quill_app::Dir::Right));
	r.key(Key::Select(quill_app::Dir::Right));
	assert_eq!(caret_of(&r, 0), (2, 0));
	r.key(Key::Backspace);
	assert_eq!(
		r.overlay().widgets()[edit_of(&r, 0).0].bytes(),
		b"-ta gamma"
	);
}

#[test]
fn a_long_text_scrolls_in_its_field_to_keep_the_caret_in_view_and_a_click_lands_on_what_shows() {
	let mut r = Rig::doc("letter.rtf");
	r.app.command(&mut r.s, &mut r.f, Command::Open);
	let (_, rect) = edit_of(&r, 0);
	let y = centre(rect).1;
	type_into(&mut r, &[b'W'; 60]);
	let wd = widths();
	let a = adv(&wd);
	let room = dialog::field_text_box(rect).1;
	assert!(60 * a(b'W') > room, "the text is wider than the field");
	// The caret at the end: the field has scrolled so the caret shows, and a click at its right edge lands at the end.
	r.click((rect.right - 4, y));
	assert_eq!(caret_of(&r, 0).0, 60);
	// Home: the field shows the start, and a click at the right edge lands inside the text, not at its end.
	r.key(Key::Home);
	r.click((rect.right - 4, y));
	let (at, _) = caret_of(&r, 0);
	assert!(at > 20 && at < 60, "{at}");
	// The caret is never out of the field: the skip puts it in the room whatever the text and the caret.
	for caret in 0..=60 {
		let text = [b'W'; 60];
		let skip = dialog::field_skip(&text, caret, room, &mut |t| t.iter().map(|c| a(*c)).sum());
		let shown: i32 = text[skip..caret].iter().map(|c| a(*c)).sum();
		assert!(shown <= room, "caret {caret} skip {skip}");
	}
}

#[test]
fn the_form_and_file_name_fields_take_the_caret_too() {
	let mut r = Rig::doc("letter.rtf");
	r.app.command(&mut r.s, &mut r.f, Command::Open);
	let (_, rect) = edit_of(&r, 0);
	let y = centre(rect).1;
	type_into(&mut r, b"letter.rtf");
	r.click((gap_x(&r, 0, 6), y));
	assert_eq!(caret_of(&r, 0), (6, 6));
	r.key(Key::Char(b'X'));
	assert_eq!(
		r.overlay().widgets()[edit_of(&r, 0).0].bytes(),
		b"LETTERX.RTF"
	);
	r.key(Key::Esc);
	r.app.command(&mut r.s, &mut r.f, Command::Fonts);
	let (_, rect) = edit_of(&r, 0);
	let y = centre(rect).1;
	r.click(centre(rect));
	r.key(Key::End);
	for _ in 0..4 {
		r.key(Key::Backspace);
	}
	type_into(&mut r, b"1234");
	r.click((gap_x(&r, 0, 2), y));
	r.key(Key::Char(b'.'));
	r.key(Key::Char(b'x'));
	assert_eq!(
		r.overlay().widgets()[edit_of(&r, 0).0].bytes(),
		b"12.34",
		"only digits and a point go in"
	);
}

// ---- the scroll bars --------------------------------------------------------------------------------------------------------------

fn long_doc() -> Rig {
	let (mut app, mut s, mut f) = new_app();
	for i in 0..220 {
		app.key(&mut s, &mut f, Key::Char(b'a' + (i % 26) as u8));
		app.key(&mut s, &mut f, Key::Enter);
	}
	app.key(&mut s, &mut f, Key::CtrlHome);
	Rig::of(app, s, f)
}

fn bar(r: &Rig) -> scroll::Parts {
	let view = r.app.vscroll_view();
	let range = r.app.vscroll_max(&r.s);
	scroll::parts(r.lay().vbar, true, px(r).min(range), range + view, view)
}

#[test]
fn the_vertical_bar_scrolls_by_arrows_pages_and_thumb() {
	let mut r = long_doc();
	assert_eq!(px(&r), 0);
	let view = r.app.vscroll_view();
	let range = r.app.vscroll_max(&r.s);
	assert!(range > 2 * view);
	let p = bar(&r);
	r.click(centre(p.down));
	assert_eq!(px(&r), LINE, "the arrow is a line");
	r.click(centre(p.up));
	assert_eq!(px(&r), 0);
	r.click(centre(p.up));
	assert_eq!(px(&r), 0, "never above the first line");
	let p = bar(&r);
	let below = (p.thumb.left + 4, p.thumb.bottom + 6);
	r.click(below);
	assert_eq!(
		px(&r),
		view - 2 * LINE,
		"the track below the thumb is a page"
	);
	let p = bar(&r);
	r.click((p.thumb.left + 4, p.thumb.top - 4));
	assert_eq!(px(&r), 0, "and above it a page back");
	// Drag the thumb to the bottom of the track: the foot of the last sheet.
	let p = bar(&r);
	let grab = (p.thumb.left + 4, p.thumb.top + 3);
	r.drag(grab, (grab.0, p.track.bottom + 40));
	assert_eq!(px(&r), range);
	// And half way, to the pixel the thumb stands for.
	let p = bar(&r);
	let grab = (p.thumb.left + 4, p.thumb.top + 3);
	let half = p.track.top + p.travel / 2 + 3;
	r.drag(grab, (grab.0, half));
	let slack = range / p.travel.max(1) + 2;
	assert!((px(&r) - range / 2).abs() <= slack, "{}", px(&r));
	// The thumb follows the position, as the painter will draw it.
	let p = bar(&r);
	assert!(p.thumb.top > p.track.top && p.thumb.bottom < p.track.bottom);
	r.ev(Ev::Wheel(-100));
	assert_eq!(px(&r), 0, "the wheel is the page's, as before");
}

#[test]
fn an_empty_page_has_a_bar_and_the_wheel_arrows_and_thumb_scroll_it() {
	let (app, s, f) = new_app();
	let mut r = Rig::of(app, s, f);
	let range = r.app.vscroll_max(&r.s);
	assert!(range > 0, "the sheet is taller than the window: {range}");
	assert!(
		bar(&r).thumb.height() < bar(&r).track.height(),
		"so the bar has a thumb"
	);
	r.ev(Ev::Wheel(1));
	assert_eq!(px(&r), 3 * LINE, "a notch is three lines");
	r.click(centre(bar(&r).down));
	assert_eq!(px(&r), 4 * LINE);
	let p = bar(&r);
	let grab = (p.thumb.left + 4, p.thumb.top + 3);
	r.drag(grab, (grab.0, p.track.bottom + 40));
	assert_eq!(px(&r), range, "the thumb drags to the foot of the sheet");
	r.ev(Ev::Wheel(-1_000));
	assert_eq!(px(&r), 0);
}

#[test]
fn a_few_tall_lines_still_have_a_vertical_bar_with_a_thumb_that_scrolls() {
	// Twelve lines of 36 point type: fewer than the screen has text rows, but taller than the page area, as the sample's heading and
	// picture are. A bar that counted rows would call this nothing to scroll.
	let mut body = String::new();
	for i in 1..=12 {
		body.push_str(&format!("\\pard\\f0\\fs72 Tall line {i}\\par "));
	}
	let rtf = format!("{{\\rtf1\\ansi{{\\fonttbl{{\\f0\\froman Times New Roman;}}}}\\paperw12240\\paperh15840\\margl1440\\margr1440\\margt1440\\margb1440 {body}}}");
	let (app, s, f) = app_with("tall.rtf", rtf.as_bytes()).unwrap();
	let mut r = Rig::of(app, s, f);
	let total = r.app.line_total(&r.s) as i32;
	assert!(
		total <= r.app.page_rows() as i32,
		"fewer lines than rows: {total}"
	);
	assert!(r.app.vscroll_max(&r.s) > 0, "but the page scrolls");
	let p = bar(&r);
	assert!(
		p.thumb.height() < p.track.height(),
		"so the bar has a thumb"
	);
	r.click(centre(p.down));
	assert_eq!(px(&r), LINE, "and the arrow scrolls a line");
	let p = bar(&r);
	r.click((p.thumb.left + 4, p.thumb.bottom + 6));
	assert!(px(&r) > LINE, "and the track a page");
	let p = bar(&r);
	let grab = (p.thumb.left + 4, p.thumb.top + 3);
	r.drag(grab, (grab.0, p.track.bottom + 30));
	assert_eq!(
		px(&r),
		r.app.vscroll_max(&r.s),
		"the thumb drags to the end"
	);
}

#[test]
fn the_thumb_drag_uses_the_shared_position_arithmetic() {
	let mut r = long_doc();
	let view = r.app.vscroll_view();
	let range = r.app.vscroll_max(&r.s);
	for want in [0, 1, range / 3, range / 2, range - 1, range] {
		r.app.set_vscroll_pos(&r.s, 0);
		let p = bar(&r);
		let grab = (p.thumb.left + 4, p.thumb.top + 2);
		let target = scroll::parts(r.lay().vbar, true, want, range + view, view);
		r.drag(grab, (grab.0, target.thumb.top + 2));
		// A thumb pixel stands for several of the page's, so the drag lands within one pixel of where that thumb sits.
		let off = (bar(&r).thumb.top - target.thumb.top).abs();
		assert!(off <= 1, "a drag to where the thumb for {want} sits: {off}");
	}
}

#[test]
fn the_status_line_takes_a_click_and_does_nothing_and_a_bar_with_nothing_to_scroll_does_too() {
	let mut r = Rig::doc("letter.rtf");
	let before = (r.app.caret(), r.app.top_line());
	if let Some(h) = r.lay().hbar {
		r.click(centre(h));
	}
	r.click((300, 470));
	assert!(r.app.chrome_on(CHROME_STATUS));
	assert_eq!((r.app.caret(), r.app.top_line()), before);
}

// ---- the page, still ------------------------------------------------------------------------------------------------------------

#[test]
fn the_page_keeps_its_clicks_drags_and_double_clicks_under_the_rectangle_the_layout_gives() {
	let mut r = Rig::doc("letter.rtf");
	assert_eq!(r.lay().page.top, 98);
	r.click((20, 166));
	let a = r.app.caret();
	r.drag((150, 210), (330, 210));
	assert!(r.app.selection().is_some());
	assert!(r.app.caret() >= a);
	// A double click takes a word: the two presses are a few ticks apart.
	r.tick += 100;
	r.down(45, 146);
	r.up(45, 146);
	r.tick += 2;
	r.down(45, 146);
	r.up(45, 146);
	let (lo, hi) = r.app.selection().unwrap();
	assert!(hi > lo);
	// With the toolbars hidden the page's rectangle moves up and a click at its new top lands in the text.
	let (mut app, mut s, mut f) = open_doc("letter.rtf");
	app.command(&mut s, &mut f, Command::Chrome(quill_app::CHROME_TOOLBAR));
	app.command(&mut s, &mut f, Command::Chrome(quill_app::CHROME_FORMAT));
	let mut r = Rig::of(app, s, f);
	let page = r.lay().page;
	assert!(page.top < 98);
	r.click((page.left + 100, page.top + 30));
	assert!(r.app.editing());
}

// ---- properties -------------------------------------------------------------------------------------------------------------------

#[test]
fn the_interface_font_table_gives_the_widths_the_surfaces_draw_with() {
	use quill_paint::raster::{Linear8, Planar4, PLANAR_BYTES};
	use quill_paint::Surface;
	use quill_strike::SliceSource;
	let w = widths();
	let mut src = SliceSource(font_bytes());
	let ui = quill_paint::ui::ui_font();
	let mut buf = vec![0u8; PLANAR_BYTES];
	let mut sf = Planar4::new(&mut buf, *strikes(), &mut src).unwrap();
	let font = sf.font(&ui);
	let mut buf2 = vec![0u8; 640 * 480];
	let mut src2 = SliceSource(font_bytes());
	let mut sf2 = Linear8::new(&mut buf2, 640, 480, 640, *strikes(), &mut src2).unwrap();
	let font2 = sf2.font(&ui);
	for c in 0..=255u8 {
		assert_eq!(adv(&w)(c), sf.advance(font, c), "mode 12h, character {c}");
		assert_eq!(adv(&w)(c), sf2.advance(font2, c), "VBE, character {c}");
	}
}

#[test]
fn every_menu_row_and_every_list_entry_is_hit_by_its_own_centre() {
	let r = Rig::doc("letter.rtf");
	let w = widths();
	for m in 0..MENU_COUNT {
		let g = geom_of(&r.app, m, adv(&w)).unwrap();
		for (i, row) in g.rows().iter().enumerate() {
			let c = centre(row.rect);
			assert_eq!(g.item_at(c.0, c.1), Some(i));
			assert!(g.contains(c.0, c.1));
		}
	}
	for pop in [Pop::Style, Pop::Face, Pop::Size, Pop::Colour] {
		let list = drop::list_rect(&r.lay(), pop).unwrap();
		assert!(
			list.right <= 640 && list.bottom <= 480,
			"{pop:?} fits the screen"
		);
		for i in 0..pop.count() {
			let c = centre(drop::item_rect(list, i));
			assert_eq!(drop::item_at(list, pop.count(), c.0, c.1), Some(i));
			let mut t = [0u8; 24];
			assert!(drop::item_text(pop, i, &mut t) > 0);
		}
		assert_eq!(
			drop::item_at(list, pop.count(), list.left, list.top),
			None,
			"the border"
		);
		assert_eq!(
			drop::item_at(list, pop.count(), list.left + 4, list.bottom + 1),
			None
		);
	}
}

fn arb_event() -> impl Strategy<Value = (u8, i32, i32)> {
	(0u8..8, 0i32..640, 0i32..480)
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(24))]

	#[test]
	fn no_run_of_events_breaks_the_app_or_leaves_the_pointer_stuck(
		evs in proptest::collection::vec(arb_event(), 1..60),
	) {
		let mut r = Rig::doc("letter.rtf");
		let w = widths();
		for (k, x, y) in evs {
			match k {
				0 | 1 => { r.mv(x, y); }
				2 | 3 => {
					r.ev(Ev::Down { x, y });
				}
				4 | 5 => {
					r.ev(Ev::Up { x, y });
				}
				6 => { r.ev(Ev::Wheel((x % 5) - 2)); }
				_ => { r.wait((y % 20) as u32); }
			}
			r.tick += 1;
			if let Some((m, item)) = r.app.menu_open() {
				let g = geom_of(&r.app, m, adv(&w)).unwrap();
				prop_assert!(usize::from(item) < g.rows().len());
			}
			if let Some((pop, hot)) = r.g.view().drop {
				prop_assert!(hot < pop.count());
			}
			if !r.app.editing() && r.app.menu_open().is_none() {
				let mut o = Overlay::default();
				prop_assert!(r.app.overlay(&r.s, &mut o));
			}
			let lay = r.lay();
			prop_assert!(lay.page.top < lay.page.bottom);
			if !r.app.chrome_on(CHROME_RULER) {
				prop_assert!(lay.ruler.is_none());
			}
		}
		// Let go wherever it is: nothing is held afterwards.
		r.ev(Ev::Up { x: 5, y: 300 });
		prop_assert!(!r.g.holding());
	}

	#[test]
	fn a_press_and_release_on_the_same_point_of_a_bar_never_moves_the_caret_off_the_page(
		x in 0i32..640, y in 0i32..98,
	) {
		let mut r = Rig::doc("letter.rtf");
		let len = r.app.doc().body.len();
		r.click((x, y));
		// Clicks on the bars and the menu bar never change the text.
		prop_assert!(r.app.doc().body.len() <= len + 1);
		prop_assert!(r.app.caret() < r.app.doc().body.len());
	}

	#[test]
	fn the_thumb_position_round_trips_for_any_document_height(
		total in 1i32..2000, view in 1i32..120, pos in 0i32..2000,
	) {
		let view = view.min(total);
		let range = total - view;
		let pos = pos.min(range);
		let bar = Rect::new(624, 98, 640, 444);
		let p = scroll::parts(bar, true, pos, total, view);
		if p.travel > 0 && range > 0 {
			let back = scroll::pos_at(&p, p.thumb.top, range);
			// The thumb is whole pixels, so the position comes back to within a pixel's worth of lines.
			let per_px = (range + p.travel - 1) / p.travel;
			prop_assert!((back - pos).abs() <= per_px, "{pos} came back as {back}");
		}
	}
}

// ---- the file dialog -----------------------------------------------------------------------------------------------------------

use quill_dlg::filedlg::{Point, CANCEL, LOOK_IN, NEW_FOLDER, OK, TYPES, UP};
use quill_host::script::{parse_script, stage};
use quill_paint::filedlg;

impl Rig {
	/// A graphics screen with the file dialog open over a folder of 30 files, a folder and Up.
	fn file_dialog(save_as: bool) -> Rig {
		let sc =
			parse_script("@dialog\n@dir QUILL\n@cwd C:\\DOCS\n@dir DOCS\\SUB\n@many DOCS 30 QLD\n");
		let (mut app, s, mut f) = new_app();
		stage(&mut f, &sc, &root().join("tests/docs"));
		app.set_file_dialog(true);
		app.set_file_dialog_graphics(true);
		let mut r = Rig::of(app, s, f);
		let keys: &[Key] = if save_as {
			&[Key::F(10), Key::Down, Key::Down, Key::Down, Key::Enter]
		} else {
			&[Key::F(10), Key::Down, Key::Enter]
		};
		for k in keys {
			r.key(*k);
		}
		assert!(r.app.file_dialog_open());
		r
	}

	fn hit(&self, x: i32, y: i32) -> Point {
		let o = self.overlay();
		filedlg::hit(&o, self.app.file_view().unwrap().dlg, x, y)
	}

	/// The pixels of a part's centre, found by asking `hit` where each kind of press lands.
	fn find(&self, want: Point) -> (i32, i32) {
		for y in (0..480).step_by(2) {
			for x in (0..640).step_by(2) {
				if self.hit(x, y) == want {
					return (x, y);
				}
			}
		}
		panic!("nothing on the screen is {want:?}");
	}

	fn top(&self) -> usize {
		self.app.file_view().unwrap().dlg.list().top()
	}

	fn selected(&self) -> Option<usize> {
		self.app.file_view().unwrap().dlg.list().selected()
	}
}

#[test]
fn every_part_of_the_file_dialog_is_found_under_the_pixels_it_is_drawn_on() {
	for save_as in [false, true] {
		let r = Rig::file_dialog(save_as);
		let mut buttons = vec![UP, OK, CANCEL];
		if save_as {
			buttons.push(NEW_FOLDER);
		}
		for id in buttons {
			r.find(Point::Button(id));
		}
		assert_eq!(r.find(Point::Button(OK)).1, r.find(Point::Button(CANCEL)).1);
		r.find(Point::Drop(LOOK_IN));
		r.find(Point::Drop(TYPES));
		r.find(Point::Name(0));
		r.find(Point::Bar);
		// 15 rows of the list and of the places, each 16 pixels, from the first to the last.
		let rows = r.app.file_view().unwrap().dlg.list().rows();
		let (x, y) = r.find(Point::Item(0));
		assert_eq!(rows, 15);
		for i in 0..rows {
			assert_eq!(r.hit(x, y + 16 * i as i32), Point::Item(i));
		}
		let (x, y) = r.find(Point::Place(0));
		assert_eq!(r.hit(x, y + 16), Point::Place(1));
		// The title bar, the frame and the desk beyond it are nothing.
		assert_eq!(r.hit(0, 0), Point::Away);
		assert_eq!(r.hit(320, 56), Point::Away);
		assert_eq!(r.hit(639, 479), Point::Away);
	}
}

#[test]
fn a_press_selects_a_second_press_opens_and_the_pointer_follows_the_keys() {
	let mut r = Rig::file_dialog(false);
	let (x, y) = r.find(Point::Item(0));
	// `..`, SUB, then the files.
	r.click((x, y + 16 * 4));
	assert_eq!(r.selected(), Some(4));
	assert!(r.app.file_dialog_open());
	r.down(x, y + 16);
	r.up(x, y + 16);
	r.down(x, y + 16);
	r.up(x, y + 16);
	assert!(r.app.file_dialog_open());
	let folder = r.app.file_view().unwrap().dlg.folder().to_vec();
	assert!(
		folder.ends_with(b"SUB"),
		"{}",
		String::from_utf8_lossy(&folder)
	);
	// Up is a button: it acts on release over it, and not when let go elsewhere.
	let up = r.find(Point::Button(UP));
	r.down(up.0, up.1);
	assert_eq!(r.g.view().widget, Some(usize::from(UP)));
	r.mv(300, 300);
	assert_eq!(r.g.view().widget, None);
	r.up(300, 300);
	assert!(r.app.file_view().unwrap().dlg.folder().ends_with(b"SUB"));
	r.click(up);
	assert!(r.app.file_view().unwrap().dlg.folder().ends_with(b"DOCS"));
	// Cancel ends the dialog.
	let cancel = r.find(Point::Button(CANCEL));
	r.click(cancel);
	assert!(!r.app.file_dialog_open());
}

#[test]
fn the_list_scrolls_by_its_bar_its_thumb_and_the_wheel() {
	let mut r = Rig::file_dialog(false);
	let bar = quill_paint::filedlg::list_bar(&r.overlay(), r.app.file_view().unwrap().dlg).unwrap();
	let x = (bar.up.left + bar.up.right) / 2;
	r.click((x, (bar.down.top + bar.down.bottom) / 2));
	assert_eq!(r.top(), 1);
	r.click((x, (bar.up.top + bar.up.bottom) / 2));
	assert_eq!(r.top(), 0);
	// The track below the thumb: a page (15 rows, less one).
	r.click((x, bar.thumb.bottom + 4));
	assert_eq!(r.top(), 14);
	// The thumb dragged to the end of its track and back.
	let bar = quill_paint::filedlg::list_bar(&r.overlay(), r.app.file_view().unwrap().dlg).unwrap();
	let mid = (bar.thumb.top + bar.thumb.bottom) / 2;
	r.drag((x, mid), (x, bar.track.bottom + 40));
	let last = r.app.file_view().unwrap().dlg.list().count() - 15;
	assert_eq!(r.top(), last);
	r.drag((x, mid), (x, 0));
	assert_eq!(r.top(), 0);
	r.ev(Ev::Wheel(2));
	assert_eq!(r.top(), 6);
	r.ev(Ev::Wheel(-1));
	assert_eq!(r.top(), 3);
	r.ev(Ev::Wheel(-9));
	assert_eq!(r.top(), 0);
	// An arrow held repeats.
	let down = (x, (bar.down.top + bar.down.bottom) / 2);
	r.down(down.0, down.1);
	r.wait(REPEAT_DELAY);
	r.wait(5 * REPEAT_STEP);
	r.up(down.0, down.1);
	assert!(r.top() >= 5, "{}", r.top());
}

#[test]
fn a_press_on_a_drop_down_opens_it_and_a_press_away_shuts_it_and_does_nothing_else() {
	let mut r = Rig::file_dialog(true);
	let types = r.find(Point::Drop(TYPES));
	r.click(types);
	assert!(r.app.file_view().unwrap().dlg.type_box().is_open());
	let before = r.selected();
	let away = r.find(Point::Place(0));
	r.click(away);
	assert!(!r.app.file_view().unwrap().dlg.type_box().is_open());
	assert_eq!(r.selected(), before);
	assert!(r.app.file_dialog_open());
	r.click(types);
	let pick = r.find(Point::Pick(1));
	r.mv(pick.0, pick.1);
	assert_eq!(
		r.app.file_view().unwrap().dlg.type_box().list().selected(),
		Some(1)
	);
	r.click(pick);
	assert_eq!(r.app.file_view().unwrap().dlg.type_index(), 1);
	// The New folder box is the app's overlay: its buttons are widgets, and the dialog under it takes no press.
	let nf = r.find(Point::Button(NEW_FOLDER));
	let item = r.find(Point::Item(1));
	r.click(nf);
	assert!(r.app.file_view().unwrap().covered());
	r.click(item);
	assert!(r.app.file_view().unwrap().covered());
	let (_, cancel) = r.widget(b"Cancel");
	r.click(cancel);
	assert!(r.app.file_dialog_open() && !r.app.file_view().unwrap().covered());
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(24))]

	#[test]
	fn no_run_of_presses_breaks_the_file_dialog_or_leaves_the_pointer_stuck(
		save_as in any::<bool>(),
		evs in proptest::collection::vec(arb_event(), 1..60),
	) {
		let mut r = Rig::file_dialog(save_as);
		for (k, x, y) in evs {
			match k {
				0 | 1 => { r.mv(x, y); }
				2 | 3 => { r.ev(Ev::Down { x, y }); }
				4 | 5 => { r.ev(Ev::Up { x, y }); }
				6 => { r.ev(Ev::Wheel((x % 5) - 2)); }
				_ => { r.wait((y % 20) as u32); }
			}
			r.tick += 1;
			if let Some(v) = r.app.file_view() {
				let list = v.dlg.list();
				prop_assert!(list.top() <= list.count().saturating_sub(list.rows()));
				if let Some(i) = list.selected() {
					prop_assert!(i < list.count());
				}
			}
		}
		r.ev(Ev::Up { x: 5, y: 300 });
		prop_assert!(!r.g.holding());
	}
}
