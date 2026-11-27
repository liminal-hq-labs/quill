// The status bar: where the caret is, the font at it, whether the document has changed, and the zoom.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// `StatusInfo` is read from the shared editor state (`status_position`, `char_state`) and the view, and is tested without a
// display. The bar is an Adwaita-styled strip of captions with the zoom control at its right end: a minus button, the zoom as a menu
// button that lists the shared levels, and a plus button, then a margin (`quill_app::zoom_field`, CORE-67).

use gtk::gio;
use gtk::prelude::*;
use quill_app::zoom_field::{
	zoom_command, zoom_step, zoom_text, ZOOM_LEVELS, ZOOM_MARGIN, ZOOM_TEXT_MAX,
};
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
	/// Whether the minus button has a level to step down to.
	pub can_zoom_out: bool,
	/// Whether the plus button has a level to step up to.
	pub can_zoom_in: bool,
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
			can_zoom_out: zoom_step(view.effective_zoom() as i32, false).is_some(),
			can_zoom_in: zoom_step(view.effective_zoom() as i32, true).is_some(),
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
	zoom_out: gtk::Button,
	zoom_in: gtk::Button,
}

fn caption(text: &str) -> gtk::Label {
	gtk::Label::builder()
		.label(text)
		.css_classes(["caption", "dim-label"])
		.build()
}

/// The menu the zoom button opens: the levels of the shared list, Page Width first, each the action of its command.
fn zoom_menu() -> gio::Menu {
	let menu = gio::Menu::new();
	for index in 0..ZOOM_LEVELS.len() {
		let Some(Command::Zoom(level)) = zoom_command(index) else {
			continue;
		};
		let mut text = [0u8; ZOOM_TEXT_MAX];
		let len = zoom_text(level, &mut text);
		let label = String::from_utf8_lossy(text.get(..len).unwrap_or(&[])).into_owned();
		menu.append(
			Some(&label),
			Some(&format!("win.{}", action_name(Command::Zoom(level)))),
		);
	}
	menu
}

/// A flat zoom step button with a symbolic icon.
fn step_button(icon: &str, tooltip: &str, action: &str) -> gtk::Button {
	let button = gtk::Button::builder()
		.icon_name(icon)
		.tooltip_text(tooltip)
		.action_name(action)
		.build();
	button.add_css_class("flat");
	button.add_css_class("caption");
	button
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
		let zoom_out = step_button("list-remove-symbolic", "Zoom out", "win.zoom-step-down");
		let zoom_in = step_button("list-add-symbolic", "Zoom in", "win.zoom-step-up");
		// The control is the minus button, the box and the plus button, close together.
		let control = gtk::Box::new(gtk::Orientation::Horizontal, 2);
		control.append(&zoom_out);
		control.append(&zoom);
		control.append(&zoom_in);
		let widget = gtk::Box::new(gtk::Orientation::Horizontal, 16);
		widget.add_css_class("quill-status");
		widget.set_margin_start(12);
		// The gap at the bar's right edge, the same on every build.
		widget.set_margin_end(ZOOM_MARGIN);
		for w in [
			message.upcast_ref::<gtk::Widget>(),
			page.upcast_ref(),
			position.upcast_ref(),
			font.upcast_ref(),
			modified.upcast_ref(),
			control.upcast_ref(),
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
			zoom_out,
			zoom_in,
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
		self.zoom_out.set_sensitive(info.can_zoom_out);
		self.zoom_in.set_sensitive(info.can_zoom_in);
	}
}
