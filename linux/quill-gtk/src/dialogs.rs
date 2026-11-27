// The dialogs: Quill's own dialog descriptions (`assets/dialogs/dialogs.json`) as `adw::Dialog`s, and what answering each one does.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The Windows builds, the text cells and the tests all read the dialogs from `quill-dialogs`; so does this client. `spec_of` turns
// a `quill_dialogs::Dialog` into rows (a label with its field, a check box, a titled group of options, text, buttons) with no
// display, and is tested. `DialogView` makes the widgets from a spec and keeps each control by its control ID, so the controllers
// below (Find and Replace, Fonts, Tabs, Spelling) read and set them the way the Windows dialog procedures do, and so a test hook
// can press a button by ID.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};

use adw::prelude::*;
use gtk::{gdk, glib};
use quill_app::FindAction;
use quill_chrome::view::family_of;
use quill_core::doc::cflag;
use quill_dialogs::{style, Class, Dialog, Item};

use crate::actions::label_of;
use crate::doc::Document;
use crate::format_bar::{size_items, size_text};
use crate::names::lossy_ansi;

static TRACE: AtomicBool = AtomicBool::new(false);

/// Turns on one line of standard output for each dialog that is shown, focused, toggled, pressed or closed (the test hooks).
pub fn set_trace(on: bool) {
	TRACE.store(on, Ordering::Relaxed);
}

fn tracing() -> bool {
	TRACE.load(Ordering::Relaxed)
}

/// What a control is, as far as a row of a dialog needs to know.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ControlKind {
	/// A single line of text.
	Entry,
	/// A choice from a list (a combo box).
	Choice,
	/// A list with a selection.
	List,
}

/// One row of a dialog.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Row {
	/// A line of text; `boxed` when the description frames it.
	Text {
		/// The control ID.
		id: u16,
		/// The words, with `&` before a mnemonic.
		text: &'static str,
		/// True for a framed text.
		boxed: bool,
	},
	/// A control under its label.
	Field {
		/// The label's words.
		label: &'static str,
		/// The control's ID.
		id: u16,
		/// What the control is.
		kind: ControlKind,
	},
	/// A check box.
	Check {
		/// The control ID.
		id: u16,
		/// The words.
		text: &'static str,
	},
	/// A titled group of options: radio buttons when `radio`, else check boxes.
	Group {
		/// The group's title.
		title: &'static str,
		/// True for radio buttons.
		radio: bool,
		/// The options: ID and words.
		items: Vec<(u16, &'static str)>,
	},
}

/// A push button of the dialog.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ButtonSpec {
	/// The control ID: 1 is OK and 2 Cancel or Close, as Windows has them.
	pub id: u16,
	/// The words.
	pub text: &'static str,
	/// True for the button Enter presses.
	pub default: bool,
}

/// A dialog as the GTK client lays it out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spec {
	/// The dialog's name.
	pub name: &'static str,
	/// Its caption.
	pub caption: &'static str,
	/// Its width in dialog units, for sizing.
	pub width: i16,
	/// The rows, top to bottom.
	pub rows: Vec<Row>,
	/// The buttons, in the description's order.
	pub buttons: Vec<ButtonSpec>,
}

fn kind_of(i: &Item) -> Option<ControlKind> {
	match i.class {
		Class::Edit => Some(ControlKind::Entry),
		Class::ComboBox => Some(ControlKind::Choice),
		Class::ListBox => Some(ControlKind::List),
		_ => None,
	}
}

/// Turns a dialog's description into rows. A static control with a field after it labels the field; a group box takes the check
/// boxes or radio buttons that follow it; push buttons gather at the bottom.
pub fn spec_of(d: &Dialog) -> Spec {
	let mut rows = Vec::new();
	let mut buttons = Vec::new();
	let items = d.items;
	let mut i = 0;
	while let Some(it) = items.get(i) {
		i += 1;
		match it.class {
			Class::Static if it.has(style::ICON) => {}
			Class::Static => match items.get(i).and_then(|n| kind_of(n).map(|k| (n, k))) {
				Some((n, kind)) => {
					rows.push(Row::Field {
						label: it.text,
						id: n.id,
						kind,
					});
					i += 1;
				}
				None => rows.push(Row::Text {
					id: it.id,
					text: it.text,
					boxed: it.has(style::BORDER),
				}),
			},
			Class::Button if it.has(style::GROUPBOX) => {
				let mut group = Vec::new();
				let mut radio = false;
				while let Some(n) = items.get(i) {
					let is_radio = n.has(style::AUTORADIOBUTTON);
					if n.class != Class::Button
						|| n.has(style::GROUPBOX)
						|| !(is_radio || n.has(style::AUTOCHECKBOX))
					{
						break;
					}
					radio |= is_radio;
					group.push((n.id, n.text));
					i += 1;
				}
				rows.push(Row::Group {
					title: it.text,
					radio,
					items: group,
				});
			}
			Class::Button if it.has(style::AUTOCHECKBOX) => rows.push(Row::Check {
				id: it.id,
				text: it.text,
			}),
			Class::Button => buttons.push(ButtonSpec {
				id: it.id,
				text: it.text,
				default: it.has(style::DEFPUSHBUTTON),
			}),
			c => {
				if let Some(kind) = kind_of(it) {
					rows.push(Row::Field {
						label: "",
						id: it.id,
						kind,
					});
				}
				let _ = c;
			}
		}
	}
	Spec {
		name: d.name,
		caption: d.caption,
		width: d.cx,
		rows,
		buttons,
	}
}

/// The widgets of one dialog, by control ID.
pub struct DialogView {
	/// The dialog to present.
	pub dialog: adw::Dialog,
	/// Its description.
	pub spec: Spec,
	entries: HashMap<u16, gtk::Entry>,
	choices: HashMap<u16, (gtk::DropDown, gtk::StringList)>,
	lists: HashMap<u16, (gtk::ListBox, RefCell<Vec<String>>)>,
	checks: HashMap<u16, gtk::CheckButton>,
	texts: HashMap<u16, gtk::Label>,
	buttons: HashMap<u16, gtk::Button>,
	cancelled: Cell<bool>,
}

impl DialogView {
	/// Builds the widgets of `spec`. `choices` are the lists to fill a choice with, by control ID.
	pub fn build(spec: Spec, choices: &[(u16, Vec<String>)]) -> Rc<DialogView> {
		let mut entries = HashMap::new();
		let mut drops = HashMap::new();
		let mut lists = HashMap::new();
		let mut checks = HashMap::new();
		let mut texts = HashMap::new();
		let mut buttons = HashMap::new();
		let body = gtk::Box::new(gtk::Orientation::Vertical, 12);
		// The first control the person can type in or choose from: where the focus starts.
		let mut first_focus: Option<gtk::Widget> = None;
		for row in &spec.rows {
			match row {
				Row::Text { id, text, boxed } => {
					let l = gtk::Label::builder()
						.label(label_of(text).replace('_', ""))
						.xalign(0.0)
						.wrap(true)
						.build();
					if *boxed {
						l.add_css_class("card");
						l.set_margin_top(2);
						l.set_margin_bottom(2);
					}
					body.append(&l);
					texts.insert(*id, l);
				}
				Row::Field { label, id, kind } => {
					let col = gtk::Box::new(gtk::Orientation::Vertical, 6);
					let mut focus_target: Option<gtk::Widget> = None;
					let widget: gtk::Widget = match kind {
						ControlKind::Entry => {
							let e = gtk::Entry::builder().activates_default(true).build();
							entries.insert(*id, e.clone());
							e.upcast()
						}
						ControlKind::Choice => {
							let items = choices
								.iter()
								.find(|(i, _)| i == id)
								.map(|(_, v)| v.clone())
								.unwrap_or_default();
							let refs: Vec<&str> = items.iter().map(String::as_str).collect();
							let store = gtk::StringList::new(&refs);
							let d = gtk::DropDown::new(Some(store.clone()), gtk::Expression::NONE);
							d.set_enable_search(items.len() > 12);
							if items.len() > 12 {
								d.set_expression(Some(&gtk::PropertyExpression::new(
									gtk::StringObject::static_type(),
									None::<&gtk::Expression>,
									"string",
								)));
							}
							drops.insert(*id, (d.clone(), store));
							d.upcast()
						}
						ControlKind::List => {
							let lb = gtk::ListBox::new();
							lb.add_css_class("boxed-list");
							lb.set_selection_mode(gtk::SelectionMode::Single);
							let sw = gtk::ScrolledWindow::builder()
								.child(&lb)
								.min_content_height(110)
								.build();
							sw.add_css_class("card");
							focus_target = Some(lb.clone().upcast());
							lists.insert(*id, (lb, RefCell::new(Vec::new())));
							sw.upcast()
						}
					};
					if !label.is_empty() {
						let l = gtk::Label::with_mnemonic(&label_of(label));
						l.set_xalign(0.0);
						l.add_css_class("heading");
						// A list's mnemonic goes to the list inside its frame.
						l.set_mnemonic_widget(Some(focus_target.as_ref().unwrap_or(&widget)));
						col.append(&l);
					}
					if first_focus.is_none() {
						first_focus = Some(focus_target.unwrap_or_else(|| widget.clone()));
					}
					col.append(&widget);
					body.append(&col);
				}
				Row::Check { id, text } => {
					let c = gtk::CheckButton::with_mnemonic(&label_of(text));
					body.append(&c);
					checks.insert(*id, c);
				}
				Row::Group {
					title,
					radio,
					items,
				} => {
					let col = gtk::Box::new(gtk::Orientation::Vertical, 6);
					let l = gtk::Label::builder()
						.label(*title)
						.xalign(0.0)
						.css_classes(["heading"])
						.build();
					col.append(&l);
					let mut first: Option<gtk::CheckButton> = None;
					for (id, text) in items {
						let c = gtk::CheckButton::with_mnemonic(&label_of(text));
						if *radio {
							match &first {
								Some(f) => c.set_group(Some(f)),
								None => first = Some(c.clone()),
							}
						}
						col.append(&c);
						checks.insert(*id, c);
					}
					body.append(&col);
				}
			}
		}
		let bar = gtk::Box::new(gtk::Orientation::Horizontal, 6);
		bar.set_halign(gtk::Align::End);
		bar.set_margin_top(6);
		let mut default = None;
		for b in &spec.buttons {
			let button = gtk::Button::with_mnemonic(&label_of(b.text));
			if b.default {
				button.add_css_class("suggested-action");
				default = Some(button.clone());
			}
			bar.append(&button);
			buttons.insert(b.id, button);
		}
		body.append(&bar);
		body.set_margin_top(18);
		body.set_margin_bottom(18);
		body.set_margin_start(18);
		body.set_margin_end(18);
		let dialog = adw::Dialog::builder()
			.title(spec.caption)
			.child(&body)
			.content_width((i32::from(spec.width) * 2).clamp(320, 560))
			.build();
		for (id, c) in &checks {
			let (id, name) = (*id, spec.name);
			c.connect_toggled(move |c| {
				if tracing() {
					println!(
						"dialog {name} control {id}: {}",
						if c.is_active() { "on" } else { "off" }
					);
				}
			});
		}
		// Enter runs the default button from a choice or a list too, which would otherwise take it for themselves.
		if let Some(default) = &default {
			for (drop, _) in drops.values() {
				let key = gtk::EventControllerKey::new();
				key.set_propagation_phase(gtk::PropagationPhase::Capture);
				let default = default.clone();
				key.connect_key_pressed(move |_, k, _, state| {
					let plain = state.difference(gdk::ModifierType::LOCK_MASK).is_empty();
					if plain && matches!(k, gdk::Key::Return | gdk::Key::KP_Enter) {
						default.emit_clicked();
						glib::Propagation::Stop
					} else {
						glib::Propagation::Proceed
					}
				});
				drop.add_controller(key);
			}
			for (list, _) in lists.values() {
				let default = default.clone();
				list.connect_row_activated(move |_, _| default.emit_clicked());
			}
		}
		dialog.set_default_widget(default.as_ref());
		dialog.set_focus(first_focus.as_ref());
		Rc::new(DialogView {
			dialog,
			spec,
			entries,
			choices: drops,
			lists,
			checks,
			texts,
			buttons,
			cancelled: Cell::new(false),
		})
	}

	/// Calls `f` with the control ID of a button when it is pressed. The Cancel and Close buttons (ID 2) also close the dialog,
	/// and Esc or any other way of closing the dialog counts as pressing Cancel: `f` hears ID 2 once, whichever way it was.
	pub fn on_button(self: &Rc<Self>, f: impl Fn(&Rc<DialogView>, u16) + 'static) {
		let f = Rc::new(f);
		for (id, b) in &self.buttons {
			let id = *id;
			let f = f.clone();
			let me = Rc::downgrade(self);
			b.connect_clicked(move |_| {
				if let Some(me) = me.upgrade() {
					if tracing() {
						println!("dialog {} button {id}: pressed", me.spec.name);
					}
					if id == 2 {
						me.cancelled.set(true);
					}
					f(&me, id);
					if id == 2 {
						me.dialog.close();
					}
				}
			});
		}
		let me = Rc::downgrade(self);
		self.dialog.connect_closed(move |_| {
			if let Some(me) = me.upgrade() {
				if tracing() {
					println!("dialog {}: closed", me.spec.name);
				}
				if !me.cancelled.replace(true) {
					f(&me, 2);
				}
			}
		});
	}

	/// The control ID of the entry, choice or list that has the keyboard focus, when one has.
	pub fn focused_id(&self) -> Option<u16> {
		let focus = self.dialog.focus()?;
		let is = |w: &gtk::Widget| *w == focus || focus.is_ancestor(w);
		self.entries
			.iter()
			.find(|(_, e)| is(e.upcast_ref()))
			.map(|(id, _)| *id)
			.or_else(|| {
				self.choices
					.iter()
					.find(|(_, (d, _))| is(d.upcast_ref()))
					.map(|(id, _)| *id)
			})
			.or_else(|| {
				self.lists
					.iter()
					.find(|(_, (l, _))| is(l.upcast_ref()))
					.map(|(id, _)| *id)
			})
	}

	/// The test hook: presses the button with this control ID. False when there is none.
	pub fn press(&self, id: u16) -> bool {
		match self.buttons.get(&id) {
			Some(b) => {
				b.emit_clicked();
				true
			}
			None => false,
		}
	}

	/// The control IDs of the buttons.
	pub fn button_ids(&self) -> Vec<u16> {
		self.spec.buttons.iter().map(|b| b.id).collect()
	}

	/// The text of an entry, the chosen item of a choice, the selected row of a list or a text.
	pub fn text(&self, id: u16) -> String {
		if let Some(e) = self.entries.get(&id) {
			return e.text().to_string();
		}
		if let Some((d, store)) = self.choices.get(&id) {
			return store
				.string(d.selected())
				.map(|s| s.to_string())
				.unwrap_or_default();
		}
		if let Some((lb, rows)) = self.lists.get(&id) {
			return lb
				.selected_row()
				.and_then(|r| rows.borrow().get(r.index() as usize).cloned())
				.unwrap_or_default();
		}
		self.texts
			.get(&id)
			.map(|l| l.text().to_string())
			.unwrap_or_default()
	}

	/// Sets the text of an entry or a static, or chooses the item of a choice with this text.
	pub fn set_text(&self, id: u16, text: &str) {
		if let Some(e) = self.entries.get(&id) {
			e.set_text(text);
		} else if let Some((d, store)) = self.choices.get(&id) {
			if let Some(i) = (0..store.n_items()).find(|i| {
				store
					.string(*i)
					.is_some_and(|s| s.eq_ignore_ascii_case(text))
			}) {
				d.set_selected(i);
			}
		} else if let Some(l) = self.texts.get(&id) {
			l.set_text(text);
		}
	}

	/// The index of the selected row of a list.
	pub fn selected(&self, id: u16) -> Option<usize> {
		self.lists
			.get(&id)
			.and_then(|(lb, _)| lb.selected_row())
			.map(|r| r.index() as usize)
	}

	/// Replaces the rows of a list, selecting the first.
	pub fn set_list(&self, id: u16, items: &[String]) {
		if let Some((lb, rows)) = self.lists.get(&id) {
			lb.remove_all();
			for s in items {
				lb.append(
					&gtk::Label::builder()
						.label(s)
						.xalign(0.0)
						.margin_start(8)
						.margin_end(8)
						.margin_top(6)
						.margin_bottom(6)
						.build(),
				);
			}
			*rows.borrow_mut() = items.to_vec();
			if let Some(r) = lb.row_at_index(0) {
				lb.select_row(Some(&r));
			}
		}
	}

	/// Whether a check box or radio button is on.
	pub fn checked(&self, id: u16) -> bool {
		self.checks.get(&id).is_some_and(|c| c.is_active())
	}

	/// Turns a check box or radio button on or off.
	pub fn set_checked(&self, id: u16, on: bool) {
		if let Some(c) = self.checks.get(&id) {
			c.set_active(on);
		}
	}

	/// Shows the dialog over `parent`.
	pub fn present(self: &Rc<Self>, parent: &impl IsA<gtk::Widget>) {
		let me = Rc::downgrade(self);
		self.dialog.connect_focus_widget_notify(move |_| {
			if let Some(v) = me.upgrade().filter(|_| tracing()) {
				if let Some(id) = v.focused_id() {
					println!("dialog {} focus: control {id}", v.spec.name);
				}
			}
		});
		self.dialog.present(Some(parent));
	}
}

/// What the controllers need from the window.
#[derive(Clone)]
pub struct Env {
	/// The document.
	pub doc: Rc<RefCell<Document>>,
	/// The widget the dialogs open over.
	pub parent: gtk::Widget,
	/// Called after a dialog changed the document, so the window redraws and the bars follow.
	pub changed: Rc<dyn Fn()>,
	/// Shows a short message as a toast.
	pub toast: Rc<dyn Fn(&str)>,
}

/// The document's text, paragraphs separated by a line feed: what a test compares.
pub fn document_text(doc: &Document) -> String {
	doc.read(|app, s| {
		let end = app.doc().body.len().saturating_sub(1);
		(0..end)
			.map(|p| match app.doc().body.read_byte(s, p).unwrap_or(b' ') {
				b'\r' | b'\n' => '\n',
				b => quill_tty::cp1252::decode(b),
			})
			.collect()
	})
}

fn ansi(text: &str, max: usize) -> Vec<u8> {
	lossy_ansi(text, max)
}

fn view_of(name: &str, choices: &[(u16, Vec<String>)]) -> Option<Rc<DialogView>> {
	let d = quill_dialogs::ALL.iter().find(|d| d.name == name)?;
	Some(DialogView::build(spec_of(d), choices))
}

/// Find and Replace.
pub fn find(env: &Env, replace: bool) -> Option<Rc<DialogView>> {
	use quill_dialogs::find as f;
	let v = view_of("find", &[])?;
	let (what, with, case, word) = env.doc.borrow().read(|app, _| {
		let (a, b, c, d) = app.last_find();
		(
			quill_tty::cp1252::ansi_to_string(a),
			quill_tty::cp1252::ansi_to_string(b),
			c,
			d,
		)
	});
	v.set_text(f::WHAT, &what);
	v.set_text(f::WITH, &with);
	v.set_checked(f::MATCH_CASE, case);
	v.set_checked(f::WHOLE_WORD, word);
	v.present(&env.parent_widget());
	if replace {
		// The Replace dialog is the same box; the focus goes to the replacement.
		v.dialog.set_focus(v.entries.get(&f::WITH));
	}
	let env = env.clone();
	v.on_button(move |v, id| {
		let action = match id {
			f::FIND_NEXT => FindAction::Next,
			f::REPLACE => FindAction::Replace,
			f::REPLACE_ALL => FindAction::ReplaceAll,
			_ => return,
		};
		let (what, with) = (ansi(&v.text(f::WHAT), 40), ansi(&v.text(f::WITH), 40));
		let (case, word) = (v.checked(f::MATCH_CASE), v.checked(f::WHOLE_WORD));
		let found = env
			.doc
			.borrow_mut()
			.with(|app, s, _| app.find_text(s, &what, &with, case, word, action));
		if !found {
			(env.toast)("The text was not found.");
		}
		(env.changed)();
	});
	Some(v)
}

impl Env {
	fn parent_widget(&self) -> gtk::Widget {
		self.parent.clone()
	}
}

const STYLE_CHECKS: [(u16, u16); 4] = [
	(quill_dialogs::fonts::BOLD, cflag::BOLD),
	(quill_dialogs::fonts::ITALIC, cflag::ITALIC),
	(quill_dialogs::fonts::UNDERLINE, cflag::UNDERLINE),
	(quill_dialogs::fonts::STRIKEOUT, cflag::STRIKE),
];

/// Character > Fonts.
pub fn fonts(env: &Env) -> Option<Rc<DialogView>> {
	use quill_dialogs::fonts as f;
	let mut families = env.doc.borrow().fonts().families();
	let (face, half, flags) = env.doc.borrow().read(|app, s| {
		let cs = app.char_state(s);
		let face = cs.face.get(..usize::from(cs.face_len)).unwrap_or(&[]);
		(quill_tty::cp1252::ansi_to_string(face), cs.size, cs.flags)
	});
	// A document may name a face this machine lacks; the list still shows it, as the Windows box does.
	if !face.is_empty() && !families.iter().any(|f| f.eq_ignore_ascii_case(&face)) {
		families.push(face.clone());
	}
	let v = view_of("fonts", &[(f::FONT, families), (f::SIZE, size_items())])?;
	v.set_text(f::FONT, &face);
	v.set_text(f::SIZE, &size_text(half));
	for (id, bit) in STYLE_CHECKS {
		v.set_checked(id, flags & bit != 0);
	}
	v.present(&env.parent_widget());
	let env = env.clone();
	v.on_button(move |v, id| {
		if id != f::OK {
			return;
		}
		let face = v.text(f::FONT);
		let half = v
			.text(f::SIZE)
			.parse::<f32>()
			.map_or(24, |p| (p * 2.0) as u16);
		let flags = STYLE_CHECKS
			.iter()
			.filter(|(id, _)| v.checked(*id))
			.fold(0, |a, (_, bit)| a | bit);
		let bytes = ansi(&face, 31);
		env.doc
			.borrow_mut()
			.with(|app, s, _| app.set_font(s, &bytes, family_of(&bytes), half, flags));
		v.dialog.close();
		(env.changed)();
	});
	Some(v)
}

fn inches(twips: i32) -> String {
	format!("{:.2}\"", f64::from(twips) / 1440.0)
}

fn tab_rows(env: &Env) -> (Vec<String>, Vec<i32>) {
	env.doc.borrow().read(|app, s| {
		let mut out = [(0, 0, 0); 16];
		let n = app.tab_stops_full(s, &mut out);
		let kinds = ["left", "centre", "right", "decimal"];
		let leaders = ["", ", dots", ", hyphens", ", underline"];
		let mut text = Vec::new();
		let mut pos = Vec::new();
		for (p, k, l) in out.iter().take(n) {
			text.push(format!(
				"{} {}{}",
				inches(*p),
				kinds.get(usize::from(*k)).unwrap_or(&"left"),
				leaders.get(usize::from(*l)).unwrap_or(&"")
			));
			pos.push(*p);
		}
		(text, pos)
	})
}

/// Paragraph > Tabs.
pub fn tabs(env: &Env) -> Option<Rc<DialogView>> {
	use quill_dialogs::tabs as t;
	let v = view_of("tabs", &[])?;
	v.set_checked(t::LEFT, true);
	v.set_checked(t::LEADER_NONE, true);
	v.set_text(t::POSITION, "0.50");
	let (rows, _) = tab_rows(env);
	v.set_list(t::STOPS, &rows);
	v.present(&env.parent_widget());
	let env = env.clone();
	v.on_button(move |v, id| {
		let kind = [t::LEFT, t::CENTRE, t::RIGHT, t::DECIMAL]
			.iter()
			.position(|k| v.checked(*k))
			.unwrap_or(0) as u8;
		let leader = [t::LEADER_NONE, t::DOTS, t::HYPHENS, t::LEADER_UNDERLINE]
			.iter()
			.position(|k| v.checked(*k))
			.unwrap_or(0) as u8;
		match id {
			t::SET => {
				let Ok(p) = v
					.text(t::POSITION)
					.trim()
					.trim_end_matches('"')
					.parse::<f64>()
				else {
					(env.toast)("Type the position in inches.");
					return;
				};
				let twips = (p * 1440.0).round() as i32;
				env.doc
					.borrow_mut()
					.with(|app, s, _| app.set_tab(s, twips, kind, leader));
			}
			t::CLEAR => {
				let (_, pos) = tab_rows(&env);
				if let Some(p) = v.selected(t::STOPS).and_then(|i| pos.get(i).copied()) {
					env.doc.borrow_mut().with(|app, s, _| app.remove_tab(s, p));
				}
			}
			t::CLEAR_ALL => env.doc.borrow_mut().with(|app, s, _| app.clear_tabs(s)),
			_ => return,
		}
		let (rows, _) = tab_rows(&env);
		v.set_list(t::STOPS, &rows);
		(env.changed)();
	});
	Some(v)
}

fn show_word(env: &Env, v: &DialogView) {
	use quill_dialogs::spelling as sp;
	let (word, context, suggestions) = env.doc.borrow().read(|app, s| {
		let mut ctx = [0u8; 72];
		let n = app.spell_context(s, &mut ctx);
		let sug = app.spell_suggestions(s);
		let list: Vec<String> = (0..sug.len())
			.filter_map(|i| sug.get(i))
			.map(|c| quill_tty::cp1252::ansi_to_string(c.bytes()))
			.collect();
		(
			quill_tty::cp1252::ansi_to_string(app.spell_word()),
			quill_tty::cp1252::ansi_to_string(ctx.get(..n).unwrap_or(&[])),
			list,
		)
	});
	v.set_text(sp::WORD, &word);
	v.set_text(sp::CONTEXT, &context);
	v.set_text(
		sp::CHANGE_TO,
		suggestions.first().map_or(word.as_str(), String::as_str),
	);
	v.set_list(sp::SUGGESTIONS, &suggestions);
}

/// Find > Spelling. `None` when there is nothing to check (the message says why).
pub fn spelling(env: &Env) -> Option<Rc<DialogView>> {
	use quill_dialogs::spelling as sp;
	let found = env.doc.borrow_mut().with(|app, s, f| app.spell_begin(s, f));
	if !found {
		let msg = env
			.doc
			.borrow()
			.read(|app, _| quill_tty::cp1252::ansi_to_string(app.message()));
		(env.toast)(if msg.is_empty() {
			"No misspelled words."
		} else {
			&msg
		});
		(env.changed)();
		return None;
	}
	let v = view_of("spelling", &[])?;
	show_word(env, &v);
	v.present(&env.parent_widget());
	let envc = env.clone();
	v.on_button(move |v, id| {
		let env = &envc;
		let with = ansi(&v.text(sp::CHANGE_TO), 40);
		let more = env.doc.borrow_mut().with(|app, s, f| match id {
			sp::IGNORE => Some(app.spell_ignore(s, false)),
			sp::IGNORE_ALL => Some(app.spell_ignore(s, true)),
			sp::CHANGE => Some(app.spell_change(s, &with, false)),
			sp::CHANGE_ALL => Some(app.spell_change(s, &with, true)),
			sp::ADD => Some(app.spell_add(s, f)),
			_ => None,
		});
		match more {
			Some(true) => show_word(env, v),
			Some(false) => {
				v.dialog.close();
				(env.toast)("The spelling check is complete.");
			}
			None => env.doc.borrow_mut().with(|app, _, _| app.spell_cancel()),
		}
		(env.changed)();
	});
	Some(v)
}

/// Help > About: the About box's texts come from the dialog description.
pub fn about(parent: &impl IsA<gtk::Widget>, icon: &str) {
	use quill_dialogs::about as a;
	let text = |id: u16| a::DIALOG.item(id).map_or("", |i| i.text);
	let dialog = adw::AboutDialog::builder()
		.application_name(text(a::TITLE))
		.application_icon(icon)
		.developer_name("Liminal HQ")
		.version(quill_app::VERSION)
		.copyright(text(a::COPYRIGHT).replace("(c) Copyright", "\u{a9}"))
		.license_type(gtk::License::Custom)
		.license(
			"Quill is licensed under the Apache License 2.0 or the MIT licence, at your option.",
		)
		.website("https://github.com/liminal-hq-labs/quill")
		.comments("A small word processor for Quill, Rich Text and plain text documents.")
		.build();
	dialog.present(Some(parent));
}

#[cfg(test)]
mod tests {
	use super::*;

	fn spec(name: &str) -> Spec {
		spec_of(quill_dialogs::ALL.iter().find(|d| d.name == name).unwrap())
	}

	#[test]
	fn every_dialog_has_a_spec_and_every_control_lands_in_it() {
		for d in quill_dialogs::ALL {
			let s = spec_of(d);
			let mut ids: Vec<u16> = s.buttons.iter().map(|b| b.id).collect();
			for r in &s.rows {
				match r {
					Row::Text { id, .. } | Row::Check { id, .. } | Row::Field { id, .. } => {
						ids.push(*id)
					}
					Row::Group { items, .. } => ids.extend(items.iter().map(|(i, _)| *i)),
				}
			}
			for it in d.items {
				let label_only = it.class == Class::Static && it.text.ends_with(':');
				let framed = it.has(style::GROUPBOX) || it.has(style::ICON);
				assert!(
					ids.contains(&it.id) || label_only || framed,
					"{}: control {} has no place",
					d.name,
					it.id
				);
			}
		}
	}

	#[test]
	fn find_is_two_fields_two_checks_and_four_buttons() {
		let s = spec("find");
		assert_eq!(s.caption, "Find and Replace");
		assert_eq!(s.rows.len(), 4);
		assert!(matches!(
			s.rows[0],
			Row::Field {
				kind: ControlKind::Entry,
				id: quill_dialogs::find::WHAT,
				..
			}
		));
		assert!(matches!(s.rows[2], Row::Check { .. }));
		let ids: Vec<u16> = s.buttons.iter().map(|b| b.id).collect();
		assert_eq!(ids, [100, 101, 102, 2]);
		assert_eq!(s.buttons.iter().filter(|b| b.default).count(), 1);
	}

	#[test]
	fn fonts_has_two_choices_and_a_style_group_of_check_boxes() {
		let s = spec("fonts");
		assert!(matches!(
			s.rows[0],
			Row::Field {
				kind: ControlKind::Choice,
				..
			}
		));
		assert!(matches!(
			s.rows[1],
			Row::Field {
				kind: ControlKind::Choice,
				..
			}
		));
		let Row::Group {
			title,
			radio,
			items,
		} = &s.rows[2]
		else {
			panic!("a Style group");
		};
		assert_eq!((*title, *radio, items.len()), ("Style", false, 4));
	}

	#[test]
	fn tabs_has_a_kind_and_a_leader_group_of_radio_buttons_and_a_list() {
		let s = spec("tabs");
		let groups: Vec<_> = s
			.rows
			.iter()
			.filter_map(|r| match r {
				Row::Group {
					title,
					radio,
					items,
				} => Some((*title, *radio, items.len())),
				_ => None,
			})
			.collect();
		assert_eq!(groups, [("Kind", true, 4), ("Leader", true, 4)]);
		assert!(s.rows.iter().any(|r| matches!(
			r,
			Row::Field {
				kind: ControlKind::List,
				..
			}
		)));
	}

	#[test]
	fn spelling_frames_the_context() {
		let s = spec("spelling");
		assert!(s
			.rows
			.iter()
			.any(|r| matches!(r, Row::Text { boxed: true, .. })));
		assert_eq!(s.buttons.len(), 6);
	}
}
