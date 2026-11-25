// The toolbar: the Windows build's first row of buttons, as flat buttons with symbolic icons.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The buttons, their order and their groups are read from `quill_app::view::TOOLBAR` (row 0), and the tips from `command_tip`, so
// they are the Windows toolbar's. Each button's icon is one Adwaita ships; none is bundled.

use gtk::prelude::*;
use quill_app::view::{Act, BTN_W, TOOLBAR};
use quill_app::{command_tip, Command};

use crate::actions::action_name;

/// One button of the toolbar.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Tool {
	/// The command it runs.
	pub command: Command,
	/// The symbolic icon's name in the Adwaita icon theme.
	pub icon: &'static str,
	/// The tooltip's words, from the shared tip table.
	pub tip: &'static str,
}

/// The Adwaita icon for a command, when the toolbar or the formatting bar shows one.
pub fn icon_of(c: Command) -> Option<&'static str> {
	Some(match c {
		Command::New => "document-new-symbolic",
		Command::Open => "document-open-symbolic",
		Command::Save => "document-save-symbolic",
		Command::Print => "document-print-symbolic",
		Command::Preview => "document-print-preview-symbolic",
		Command::Cut => "edit-cut-symbolic",
		Command::Copy => "edit-copy-symbolic",
		Command::Paste => "edit-paste-symbolic",
		Command::Undo => "edit-undo-symbolic",
		Command::Redo => "edit-redo-symbolic",
		Command::Spelling => "tools-check-spelling-symbolic",
		Command::Bold => "format-text-bold-symbolic",
		Command::Italic => "format-text-italic-symbolic",
		Command::Underline => "format-text-underline-symbolic",
		Command::Align(0) => "format-justify-left-symbolic",
		Command::Align(1) => "format-justify-center-symbolic",
		Command::Align(2) => "format-justify-right-symbolic",
		Command::Align(3) => "format-justify-fill-symbolic",
		Command::Bullets => "view-list-bullet-symbolic",
		Command::Numbering => "view-list-ordered-symbolic",
		_ => return None,
	})
}

/// The toolbar's groups, in order: the buttons of row 0 of the shared table, split where it leaves a gap.
pub fn groups() -> Vec<Vec<Tool>> {
	let mut out: Vec<Vec<Tool>> = Vec::new();
	let mut prev_x = i32::MIN;
	for b in TOOLBAR.buttons.iter().filter(|b| b.row == 0) {
		let Act::Cmd(c) = b.act else {
			continue;
		};
		let Some(icon) = icon_of(c) else {
			continue;
		};
		if b.x != prev_x + BTN_W || out.is_empty() {
			out.push(Vec::new());
		}
		prev_x = b.x;
		if let Some(g) = out.last_mut() {
			g.push(Tool {
				command: c,
				icon,
				tip: command_tip(c),
			});
		}
	}
	out
}

/// The tooltip of a command's button: its words and, in brackets, its accelerator as a person reads it.
pub fn tooltip(c: Command, accel_text: &str) -> String {
	let tip = command_tip(c);
	if accel_text.is_empty() {
		tip.to_string()
	} else {
		format!("{tip} ({accel_text})")
	}
}

/// The accelerator text of the menu item that runs `c`, for its tooltip.
pub fn accel_text(c: Command) -> &'static str {
	(0..quill_app::MENU_COUNT)
		.flat_map(|m| quill_app::menu_items(m).iter())
		.find(|i| i.command == c && i.sub.is_empty())
		.map_or("", |i| i.accel)
}

/// Builds the toolbar: a horizontal bar of flat icon buttons, a separator between groups.
pub fn build() -> gtk::Box {
	let bar = gtk::Box::new(gtk::Orientation::Horizontal, 2);
	bar.add_css_class("toolbar");
	bar.add_css_class("quill-toolbar");
	bar.set_accessible_role(gtk::AccessibleRole::Toolbar);
	for (i, g) in groups().iter().enumerate() {
		if i > 0 {
			let sep = gtk::Separator::new(gtk::Orientation::Vertical);
			sep.set_margin_top(6);
			sep.set_margin_bottom(6);
			bar.append(&sep);
		}
		for t in g {
			let b = gtk::Button::from_icon_name(t.icon);
			b.add_css_class("flat");
			b.set_action_name(Some(&format!("win.{}", action_name(t.command))));
			b.set_tooltip_text(Some(&tooltip(t.command, accel_text(t.command))));
			b.update_property(&[gtk::accessible::Property::Label(t.tip)]);
			bar.append(&b);
		}
	}
	bar
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn the_toolbar_is_the_windows_one() {
		let g = groups();
		let cmds: Vec<Vec<Command>> = g
			.iter()
			.map(|g| g.iter().map(|t| t.command).collect())
			.collect();
		assert_eq!(
			cmds,
			vec![
				vec![
					Command::New,
					Command::Open,
					Command::Save,
					Command::Print,
					Command::Preview
				],
				vec![Command::Cut, Command::Copy, Command::Paste],
				vec![Command::Undo, Command::Redo],
				vec![Command::Spelling],
			]
		);
	}

	#[test]
	fn every_button_has_an_icon_and_a_tip_from_the_shared_table() {
		for t in groups().iter().flatten() {
			assert!(t.icon.ends_with("-symbolic"));
			assert!(!t.tip.is_empty());
		}
		assert_eq!(tooltip(Command::Save, "Ctrl+S"), "Save (Ctrl+S)");
		assert_eq!(tooltip(Command::Preview, ""), "Print Preview");
		assert_eq!(accel_text(Command::Undo), "Ctrl+Z");
	}
}
