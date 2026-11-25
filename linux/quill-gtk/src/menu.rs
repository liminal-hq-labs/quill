// The menu bar: the Windows build's menus, in its order, with its names, mnemonics and accelerators.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The tree is read from `quill_app::menu_items`, the one table the DOS, Win16 and Win32 menus are made from, so a change there
// reaches this bar. `tree` is plain data and is tested without a display; `model` turns it into a `gio::Menu` for a
// `gtk::PopoverMenuBar`. Check marks and greyed items come from the actions' state (`window::Ui::sync_actions` sets them from
// `App::menu_mark`), not from the model.

use gtk::gio;
use quill_app::{menu_items, menu_title, Command, Item, MENU_COUNT};

use crate::actions::{accel_of, action_name, label_of};

/// The action of the GTK-only View > Menu Bar item.
pub const MENU_BAR_ACTION: &str = "menu-bar";

/// One entry of a menu section.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Entry {
	/// An item that runs an action.
	Item {
		/// The GTK label, with `_` before the mnemonic.
		label: String,
		/// The action name, without `win.`.
		action: String,
		/// The accelerator text the Windows menu shows (`Ctrl+N`), empty for none.
		accel: String,
		/// The shared command, or `None` for an item only the GTK client has.
		command: Option<Command>,
	},
	/// An item that opens a submenu.
	Submenu {
		/// The GTK label.
		label: String,
		/// Its sections.
		sections: Vec<Vec<Entry>>,
	},
}

/// One menu of the bar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Menu {
	/// The title as GTK wants it (`_File`).
	pub title: String,
	/// The sections, which GTK draws with a rule between them.
	pub sections: Vec<Vec<Entry>>,
}

fn entry(it: &Item) -> Entry {
	if it.sub.is_empty() {
		Entry::Item {
			label: label_of(it.label),
			action: action_name(it.command),
			accel: it.accel.to_string(),
			command: Some(it.command),
		}
	} else {
		Entry::Submenu {
			label: label_of(it.label),
			sections: sections(it.sub),
		}
	}
}

/// Splits the items into sections where the table puts a rule.
fn sections(items: &[Item]) -> Vec<Vec<Entry>> {
	let mut out: Vec<Vec<Entry>> = Vec::new();
	for it in items {
		if it.rule || out.is_empty() {
			out.push(Vec::new());
		}
		if let Some(s) = out.last_mut() {
			s.push(entry(it));
		}
	}
	out
}

/// The menus of the bar. `recents` are the recent file names to list in File, between its last group and Exit as on Windows.
pub fn tree(recents: &[String]) -> Vec<Menu> {
	(0..MENU_COUNT)
		.filter(|m| !menu_items(*m).is_empty())
		.map(|m| {
			let title = String::from_utf8_lossy(menu_title(m)).into_owned();
			let mut secs = sections(menu_items(m));
			if m == 0 && !recents.is_empty() {
				let list: Vec<Entry> = recents
					.iter()
					.enumerate()
					.take(4)
					.map(|(i, name)| Entry::Item {
						label: format!("_{} {}", i + 1, name.replace('_', "__")),
						action: action_name(Command::Recent(i as u8)),
						accel: String::new(),
						command: Some(Command::Recent(i as u8)),
					})
					.collect();
				let at = secs.len().saturating_sub(1);
				secs.insert(at, list);
			}
			if m == 7 {
				// The classic bar is optional in this client (the Windows build's is always there).
				if let Some(first) = secs.first_mut() {
					first.insert(
						0,
						Entry::Item {
							label: "_Menu Bar".to_string(),
							action: MENU_BAR_ACTION.to_string(),
							accel: String::new(),
							command: None,
						},
					);
				}
			}
			Menu {
				title: label_of(&title),
				sections: secs,
			}
		})
		.collect()
}

fn section_model(entries: &[Entry]) -> gio::Menu {
	let menu = gio::Menu::new();
	for e in entries {
		match e {
			Entry::Item { label, action, .. } => {
				menu.append(Some(label), Some(&format!("win.{action}")));
			}
			Entry::Submenu { label, sections } => {
				menu.append_submenu(Some(label), &model_of(sections));
			}
		}
	}
	menu
}

fn model_of(sections: &[Vec<Entry>]) -> gio::Menu {
	let menu = gio::Menu::new();
	for s in sections {
		menu.append_section(None, &section_model(s));
	}
	menu
}

/// The model of the whole bar, for `gtk::PopoverMenuBar::from_model`.
pub fn model(menus: &[Menu]) -> gio::Menu {
	let bar = gio::Menu::new();
	for m in menus {
		bar.append_submenu(Some(&m.title), &model_of(&m.sections));
	}
	bar
}

/// Every item of the tree with its accelerator as GTK writes it: (action name, accelerators).
pub fn accelerators(menus: &[Menu]) -> Vec<(String, String)> {
	fn walk(sections: &[Vec<Entry>], out: &mut Vec<(String, String)>) {
		for e in sections.iter().flatten() {
			match e {
				Entry::Item { action, accel, .. } => {
					if let Some(a) = accel_of(accel) {
						out.push((action.clone(), a));
					}
				}
				Entry::Submenu { sections, .. } => walk(sections, out),
			}
		}
	}
	let mut out = Vec::new();
	for m in menus {
		walk(&m.sections, &mut out);
	}
	out
}

/// Every command in the tree, in order.
pub fn commands(menus: &[Menu]) -> Vec<Command> {
	fn walk(sections: &[Vec<Entry>], out: &mut Vec<Command>) {
		for e in sections.iter().flatten() {
			match e {
				Entry::Item { command, .. } => out.extend(*command),
				Entry::Submenu { sections, .. } => walk(sections, out),
			}
		}
	}
	let mut out = Vec::new();
	for m in menus {
		walk(&m.sections, &mut out);
	}
	out
}

#[cfg(test)]
mod tests {
	use super::*;

	fn labels(m: &Menu) -> Vec<String> {
		m.sections
			.iter()
			.flatten()
			.map(|e| match e {
				Entry::Item { label, .. } | Entry::Submenu { label, .. } => label.clone(),
			})
			.collect()
	}

	#[test]
	fn the_bar_has_the_windows_menus_in_order() {
		let t = tree(&[]);
		let titles: Vec<&str> = t.iter().map(|m| m.title.as_str()).collect();
		assert_eq!(
			titles,
			[
				"_File",
				"_Edit",
				"_Insert",
				"Fi_nd",
				"_Character",
				"_Paragraph",
				"_Document",
				"_View",
				"_Help"
			]
		);
	}

	#[test]
	fn file_and_edit_have_the_windows_items_and_groups() {
		let t = tree(&[]);
		assert_eq!(
			labels(&t[0]),
			[
				"_New",
				"_Open\u{2026}",
				"_Save",
				"Save _As\u{2026}",
				"_Print\u{2026}",
				"Print Pre_view",
				"Print Set_up\u{2026}",
				"E_xit"
			]
		);
		let group_sizes: Vec<usize> = t[0].sections.iter().map(Vec::len).collect();
		assert_eq!(group_sizes, [2, 2, 3, 1]);
		assert_eq!(
			labels(&t[1]),
			["_Undo", "_Redo", "Cu_t", "_Copy", "_Paste", "Select A_ll"]
		);
	}

	#[test]
	fn recent_files_sit_before_exit() {
		let t = tree(&["a.qld".to_string(), "b_c.rtf".to_string()]);
		let l = labels(&t[0]);
		assert_eq!(&l[l.len() - 3..], ["_1 a.qld", "_2 b__c.rtf", "E_xit"]);
		assert_eq!(t[0].sections.last().map(Vec::len), Some(1));
	}

	#[test]
	fn view_leads_with_the_menu_bar_toggle_then_the_four_bars_and_zoom() {
		let t = tree(&[]);
		let v = &t[7];
		assert_eq!(
			labels(v),
			[
				"_Menu Bar",
				"_Toolbar",
				"_Formatting Bar",
				"_Ruler",
				"_Status Bar",
				"_Zoom"
			]
		);
		let Some(Entry::Submenu { sections, .. }) = v.sections.last().and_then(|s| s.last()) else {
			panic!("Zoom is a submenu");
		};
		assert_eq!(sections.iter().flatten().count(), 5);
	}

	#[test]
	fn every_command_of_the_windows_tree_is_here() {
		let want: Vec<Command> = (0..MENU_COUNT)
			.flat_map(|m| menu_items(m).iter())
			.flat_map(|i| std::iter::once(i.command).chain(i.sub.iter().map(|s| s.command)))
			.collect();
		let got = commands(&tree(&[]));
		// A submenu item contributes its entries, not its own placeholder command.
		for c in want {
			if matches!(c, Command::Zoom(0)) {
				continue;
			}
			assert!(got.contains(&c), "{c:?} is missing");
		}
		assert!(got.contains(&Command::Zoom(0)));
	}

	#[test]
	fn accelerators_are_the_windows_ones_and_none_repeats() {
		let accels = accelerators(&tree(&[]));
		let get = |a: &str| accels.iter().find(|(n, _)| n == a).map(|(_, k)| k.as_str());
		assert_eq!(get("save"), Some("<Ctrl>s"));
		assert_eq!(get("bold"), Some("<Ctrl>b"));
		assert_eq!(get("find-next"), Some("F3"));
		assert_eq!(get("superscript"), Some("<Ctrl><Shift>equal"));
		assert_eq!(get("spacing-1"), Some("<Ctrl>5"));
		let mut seen = std::collections::HashSet::new();
		for (_, k) in &accels {
			assert!(seen.insert(k.clone()), "{k} is bound twice");
		}
	}
}
