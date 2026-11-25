// The formatting bar: the paragraph style, font and size boxes, the character and paragraph buttons and the colour list.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// What the bar shows is `FormatState`, read from the shared editor state at the caret (`char_state`, `para_state`, `style_of`);
// what a box or the colour list does is a `FormatEdit`, applied by `apply` through the same `App` calls the Windows toolbar's
// boxes make. Both are plain data and are tested without a display. The buttons are `ToggleButton`s on boolean actions, so they
// follow the same state the menu's check marks do.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::prelude::*;
use quill_app::view::{family_of, COLOURS, SIZES};
use quill_app::{combo_tip, Command, COLOUR_TIP, STYLE_NAMES};

use crate::actions::action_name;
use crate::doc::Document;
use crate::names::lossy_ansi;
use crate::toolbar::{accel_text, icon_of, tooltip};

/// What the bar shows at the caret.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormatState {
	/// The paragraph preset the paragraph matches, as an index into `STYLE_NAMES`.
	pub style: Option<usize>,
	/// The face name.
	pub face: String,
	/// The size in half points.
	pub half_points: u16,
	/// The text colour, or `None` for Automatic.
	pub colour: Option<(u8, u8, u8)>,
}

impl FormatState {
	/// The state at the document's caret.
	pub fn of(doc: &Document) -> FormatState {
		doc.read(|app, s| {
			let cs = app.char_state(s);
			let face = cs.face.get(..usize::from(cs.face_len)).unwrap_or(&[]);
			FormatState {
				style: app.style_of(s),
				face: quill_tty::cp1252::ansi_to_string(face),
				half_points: cs.size,
				colour: cs.colour,
			}
		})
	}

	/// The size as the size box writes it: `12`, or `10.5` for a half point.
	pub fn size_text(&self) -> String {
		size_text(self.half_points)
	}
}

/// A size in half points as the size box writes it.
pub fn size_text(half_points: u16) -> String {
	if half_points.is_multiple_of(2) {
		(half_points / 2).to_string()
	} else {
		format!("{}.5", half_points / 2)
	}
}

/// What the boxes and the colour list change.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FormatEdit {
	/// A paragraph preset, by index into `STYLE_NAMES`.
	Style(usize),
	/// A face by name.
	Face(String),
	/// A size in half points.
	Size(u16),
	/// A text colour, or `None` for Automatic.
	Colour(Option<(u8, u8, u8)>),
}

/// Applies an edit through the shared editor calls.
pub fn apply(doc: &mut Document, e: &FormatEdit) {
	doc.with(|app, s, _| match e {
		FormatEdit::Style(n) => app.apply_style(s, *n),
		FormatEdit::Face(name) => {
			let ansi = lossy_ansi(name, 31);
			app.set_face(s, &ansi, family_of(&ansi));
		}
		FormatEdit::Size(h) => app.set_size(s, *h),
		FormatEdit::Colour(c) => app.set_colour(s, *c),
	});
}

/// The sizes the size box lists, as text.
pub fn size_items() -> Vec<String> {
	SIZES.iter().map(|p| p.to_string()).collect()
}

/// What the window hands the bar to call when the person changes something.
pub type OnEdit = Rc<dyn Fn(FormatEdit)>;

/// The bar and what it needs to follow the caret.
pub struct FormatBar {
	/// The widget to put in the window.
	pub widget: gtk::Box,
	style: gtk::DropDown,
	face: gtk::DropDown,
	size: gtk::DropDown,
	faces: Rc<RefCell<Vec<String>>>,
	face_store: gtk::StringList,
	syncing: Rc<Cell<bool>>,
}

fn string_drop(items: &[String], tip: &str, width: i32) -> gtk::DropDown {
	let refs: Vec<&str> = items.iter().map(String::as_str).collect();
	let d = gtk::DropDown::from_strings(&refs);
	d.set_tooltip_text(Some(tip));
	d.set_enable_search(items.len() > 12);
	if items.len() > 12 {
		let expr = gtk::PropertyExpression::new(
			gtk::StringObject::static_type(),
			None::<&gtk::Expression>,
			"string",
		);
		d.set_expression(Some(&expr));
	}
	d.set_size_request(width, -1);
	d
}

fn toggle(c: Command) -> gtk::ToggleButton {
	let b = gtk::ToggleButton::new();
	if let Some(icon) = icon_of(c) {
		b.set_icon_name(icon);
	}
	b.add_css_class("flat");
	b.set_action_name(Some(&format!("win.{}", action_name(c))));
	b.set_tooltip_text(Some(&tooltip(c, accel_text(c))));
	b
}

impl FormatBar {
	/// Builds the bar over these font families; `on_edit` is called for a box or a colour chosen.
	pub fn build(families: Vec<String>, on_edit: OnEdit) -> FormatBar {
		let syncing = Rc::new(Cell::new(false));
		let styles: Vec<String> = STYLE_NAMES.iter().map(|s| s.to_string()).collect();
		let style = string_drop(&styles, combo_tip(0), 140);
		let face = string_drop(&families, combo_tip(1), 190);
		let size = string_drop(&size_items(), combo_tip(2), 72);

		let sync = syncing.clone();
		let cb = on_edit.clone();
		style.connect_selected_notify(move |d| {
			if !sync.get() && (d.selected() as usize) < STYLE_NAMES.len() {
				cb(FormatEdit::Style(d.selected() as usize));
			}
		});
		let sync = syncing.clone();
		let cb = on_edit.clone();
		let names = Rc::new(RefCell::new(families));
		let seen = names.clone();
		face.connect_selected_notify(move |d| {
			if let (false, Some(n)) = (sync.get(), seen.borrow().get(d.selected() as usize)) {
				cb(FormatEdit::Face(n.clone()));
			}
		});
		let face_store = face
			.model()
			.and_downcast::<gtk::StringList>()
			.unwrap_or_default();
		let sync = syncing.clone();
		let cb = on_edit.clone();
		size.connect_selected_notify(move |d| {
			if let (false, Some(p)) = (sync.get(), SIZES.get(d.selected() as usize)) {
				cb(FormatEdit::Size(p * 2));
			}
		});

		let bar = gtk::Box::new(gtk::Orientation::Horizontal, 6);
		bar.add_css_class("toolbar");
		bar.add_css_class("quill-format-bar");
		bar.append(&style);
		bar.append(&face);
		bar.append(&size);
		let chars = gtk::Box::new(gtk::Orientation::Horizontal, 0);
		chars.add_css_class("linked");
		for c in [Command::Bold, Command::Italic, Command::Underline] {
			chars.append(&toggle(c));
		}
		bar.append(&chars);
		bar.append(&colour_button(on_edit.clone()));
		let aligns = gtk::Box::new(gtk::Orientation::Horizontal, 0);
		aligns.add_css_class("linked");
		for n in 0..4 {
			aligns.append(&toggle(Command::Align(n)));
		}
		bar.append(&aligns);
		let lists = gtk::Box::new(gtk::Orientation::Horizontal, 0);
		lists.add_css_class("linked");
		for c in [Command::Bullets, Command::Numbering] {
			lists.append(&toggle(c));
		}
		bar.append(&lists);
		FormatBar {
			widget: bar,
			style,
			face,
			size,
			faces: names,
			face_store,
			syncing,
		}
	}

	/// Shows the state at the caret in the boxes.
	pub fn show(&self, st: &FormatState) {
		self.syncing.set(true);
		self.style
			.set_selected(st.style.map_or(gtk::INVALID_LIST_POSITION, |i| i as u32));
		// A document may name a face this machine lacks; the box still shows it, as the Windows box does.
		let found = self
			.faces
			.borrow()
			.iter()
			.position(|f| f.eq_ignore_ascii_case(&st.face));
		let at = match found {
			Some(i) => Some(i),
			None if !st.face.is_empty() => {
				self.faces.borrow_mut().push(st.face.clone());
				self.face_store.append(&st.face);
				Some(self.faces.borrow().len() - 1)
			}
			None => None,
		};
		self.face
			.set_selected(at.map_or(gtk::INVALID_LIST_POSITION, |i| i as u32));
		let size = SIZES.iter().position(|p| p * 2 == st.half_points);
		self.size
			.set_selected(size.map_or(gtk::INVALID_LIST_POSITION, |i| i as u32));
		self.syncing.set(false);
	}
}

/// The text colour button: a menu button whose popover lists Automatic and the sixteen colours.
fn colour_button(on_edit: OnEdit) -> gtk::MenuButton {
	let pop = gtk::Popover::new();
	let grid = gtk::Grid::builder()
		.row_spacing(4)
		.column_spacing(4)
		.margin_top(8)
		.margin_bottom(8)
		.margin_start(8)
		.margin_end(8)
		.build();
	let provider = gtk::CssProvider::new();
	let mut css = String::new();
	for (i, (_, (r, g, b))) in COLOURS.iter().enumerate() {
		css.push_str(&format!(
			".quill-swatch-{i} {{ background-color: rgb({r},{g},{b}); }}\n"
		));
	}
	provider.load_from_string(&css);
	if let Some(display) = gtk::gdk::Display::default() {
		gtk::style_context_add_provider_for_display(
			&display,
			&provider,
			gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
		);
	}
	for (i, (name, rgb)) in COLOURS.iter().enumerate() {
		let swatch = gtk::Button::new();
		swatch.set_size_request(24, 24);
		swatch.add_css_class("quill-swatch");
		swatch.add_css_class(&format!("quill-swatch-{i}"));
		swatch.set_tooltip_text(Some(name));
		let cb = on_edit.clone();
		let p = pop.clone();
		let rgb = *rgb;
		swatch.connect_clicked(move |_| {
			cb(FormatEdit::Colour(Some(rgb)));
			p.popdown();
		});
		grid.attach(&swatch, (i % 4) as i32, (i / 4) as i32, 1, 1);
	}
	let auto = gtk::Button::with_label("Automatic");
	auto.add_css_class("flat");
	let cb = on_edit;
	let p = pop.clone();
	auto.connect_clicked(move |_| {
		cb(FormatEdit::Colour(None));
		p.popdown();
	});
	grid.attach(&auto, 0, 4, 4, 1);
	pop.set_child(Some(&grid));
	let b = gtk::MenuButton::builder()
		.icon_name("color-select-symbolic")
		.tooltip_text(COLOUR_TIP)
		.popover(&pop)
		.build();
	b.add_css_class("flat");
	b
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn sizes_read_as_the_size_box_writes_them() {
		assert_eq!(size_text(24), "12");
		assert_eq!(size_text(21), "10.5");
		assert_eq!(size_items().first().map(String::as_str), Some("8"));
		assert_eq!(size_items().len(), 15);
	}
}
