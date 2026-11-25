// The status bar: where the caret is, the font at it, whether the document has changed, and the zoom.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// `StatusInfo` is read from the shared editor state (`status_position`, `char_state`) and the view, and is tested without a
// display. The bar is an Adwaita-styled strip of captions with the zoom as a menu button.

use gtk::gio;
use gtk::prelude::*;
use quill_app::Command;

use crate::actions::action_name;
use crate::doc::Document;
use crate::format_bar::size_text;
use crate::view::{page_label, View};

/// What the bar says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatusInfo {
	/// The sentence the editor left (a failure, a refusal), empty for none.
	pub message: String,
	/// `Page X of N`.
	pub page: String,
	/// `Ln L, Col C`.
	pub position: String,
	/// The face and size at the caret: `Liberation Serif 12`.
	pub font: String,
	/// True when there are unsaved changes.
	pub modified: bool,
	/// The zoom as shown: `100%`.
	pub zoom: String,
}

impl StatusInfo {
	/// The state of a document in a view.
	pub fn of(doc: &Document, view: &View) -> StatusInfo {
		let (page, ln, col, font, message) = doc.read(|app, s| {
			let (page, ln, col) = app.status_position(s);
			let cs = app.char_state(s);
			let face = cs.face.get(..usize::from(cs.face_len)).unwrap_or(&[]);
			(
				page,
				ln,
				col,
				format!(
					"{} {}",
					quill_tty::cp1252::ansi_to_string(face),
					size_text(cs.size)
				),
				quill_tty::cp1252::ansi_to_string(app.message()),
			)
		});
		let message = doc.notice().map_or(message, str::to_string);
		StatusInfo {
			message,
			page: page_label(page, view.geometry().pages),
			position: format!("Ln {ln}, Col {col}"),
			font,
			modified: doc.modified(),
			zoom: crate::view::zoom_label(view.effective_zoom()),
		}
	}
}

/// The bar's widgets.
pub struct Status {
	/// The widget to put in the window.
	pub widget: gtk::Box,
	message: gtk::Label,
	page: gtk::Label,
	position: gtk::Label,
	font: gtk::Label,
	modified: gtk::Label,
	zoom: gtk::MenuButton,
}

fn caption(text: &str) -> gtk::Label {
	gtk::Label::builder()
		.label(text)
		.css_classes(["caption", "dim-label"])
		.build()
}

/// The menu the zoom button opens: the zoom items of the View menu and the steps.
fn zoom_menu() -> gio::Menu {
	let menu = gio::Menu::new();
	let presets = gio::Menu::new();
	for (label, z) in [
		("Page Width", 0),
		("150%", 150),
		("100%", 100),
		("75%", 75),
		("50%", 50),
	] {
		presets.append(
			Some(label),
			Some(&format!("win.{}", action_name(Command::Zoom(z)))),
		);
	}
	menu.append_section(None, &presets);
	let steps = gio::Menu::new();
	steps.append(Some("Zoom In"), Some("win.zoom-in"));
	steps.append(Some("Zoom Out"), Some("win.zoom-out"));
	menu.append_section(None, &steps);
	menu
}

impl Status {
	/// Builds the bar.
	pub fn build() -> Status {
		let message = gtk::Label::builder()
			.xalign(0.0)
			.hexpand(true)
			.ellipsize(gtk::pango::EllipsizeMode::End)
			.css_classes(["caption"])
			.build();
		let page = caption("");
		let position = caption("");
		let font = caption("");
		let modified = gtk::Label::builder()
			.label("Mod")
			.css_classes(["caption-heading", "accent"])
			.tooltip_text("The document has unsaved changes")
			.build();
		let zoom = gtk::MenuButton::builder()
			.menu_model(&zoom_menu())
			.tooltip_text("Zoom")
			.build();
		zoom.add_css_class("flat");
		zoom.add_css_class("caption");
		let widget = gtk::Box::new(gtk::Orientation::Horizontal, 16);
		widget.add_css_class("quill-status");
		widget.set_margin_start(12);
		widget.set_margin_end(4);
		for w in [
			message.upcast_ref::<gtk::Widget>(),
			page.upcast_ref(),
			position.upcast_ref(),
			font.upcast_ref(),
			modified.upcast_ref(),
			zoom.upcast_ref(),
		] {
			widget.append(w);
		}
		Status {
			widget,
			message,
			page,
			position,
			font,
			modified,
			zoom,
		}
	}

	/// Shows `info`.
	pub fn show(&self, info: &StatusInfo) {
		self.message.set_text(&info.message);
		self.page.set_text(&info.page);
		self.position.set_text(&info.position);
		self.font.set_text(&info.font);
		self.modified.set_visible(info.modified);
		self.zoom.set_label(&info.zoom);
	}
}
