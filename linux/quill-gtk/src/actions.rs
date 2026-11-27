// Actions: the names GTK knows the editor's commands by, their accelerators and labels, and the trait the widgets call through.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Every menu item, toolbar button and shortcut is a `Gio` action named `win.NAME` that runs one shared `Command`. The names, the
// accelerators (read from the Windows menu tree's own text, such as `Ctrl+Shift+=`) and the labels (`&Open...` becomes `_Open…`)
// are worked out here, with no display, so the tree can be tested against them.

use quill_app::{Command, Dialog, Mark};

/// What the chrome asks of the document: run a command, and say how its menu item looks. The window implements the routing around
/// it (zoom, bars, files); a test implements it with a recorder.
pub trait Commands {
	/// Runs the command and returns the dialog the editor asked for, if any.
	fn run(&mut self, c: Command) -> Option<Dialog>;
	/// The check mark and the greyed state of the item that runs `c`.
	fn mark(&self, c: Command) -> Mark;
}

impl Commands for crate::doc::Document {
	fn run(&mut self, c: Command) -> Option<Dialog> {
		self.command(c)
	}

	fn mark(&self, c: Command) -> Mark {
		crate::doc::Document::mark(self, c)
	}
}

/// The action name (without the `win.` prefix) of a command.
pub fn action_name(c: Command) -> String {
	match c {
		Command::New => "new".into(),
		Command::Open => "open".into(),
		Command::Save => "save".into(),
		Command::SaveAs => "save-as".into(),
		Command::Exit => "exit".into(),
		Command::Undo => "undo".into(),
		Command::Redo => "redo".into(),
		Command::Bullets => "bullets".into(),
		Command::Numbering => "numbering".into(),
		Command::LevelUp => "level-up".into(),
		Command::LevelDown => "level-down".into(),
		Command::View(n) => format!("view-{n}"),
		Command::Field(n) => format!("field-{n}"),
		Command::FirstPage => "first-page".into(),
		Command::Cut => "cut".into(),
		Command::Copy => "copy".into(),
		Command::Paste => "paste".into(),
		Command::SelectAll => "select-all".into(),
		Command::Fonts => "fonts".into(),
		Command::Tabs => "tabs".into(),
		Command::About => "about".into(),
		Command::Help => "help".into(),
		Command::Picture => "picture".into(),
		Command::Print => "print".into(),
		Command::Chrome(bit) => format!("chrome-{bit}"),
		Command::Spelling => "spelling".into(),
		Command::SpellAsYouType => "spell-live".into(),
		Command::Preview => "preview".into(),
		Command::PrintSetup => "page-setup".into(),
		Command::Recent(n) => format!("recent-{n}"),
		Command::Find => "find".into(),
		Command::FindNext => "find-next".into(),
		Command::Replace => "replace".into(),
		Command::Regular => "regular".into(),
		Command::Bold => "bold".into(),
		Command::Italic => "italic".into(),
		Command::Underline => "underline".into(),
		Command::Strike => "strike".into(),
		Command::Super => "superscript".into(),
		Command::Sub => "subscript".into(),
		Command::Align(n) => format!("align-{n}"),
		Command::Spacing(n) => format!("spacing-{n}"),
		Command::Zoom(z) => format!("zoom-{z}"),
		// The actions the window already has for Ctrl+numpad plus and Ctrl+minus (WP-BAR-1K).
		Command::ZoomIn => "zoom-in".into(),
		Command::ZoomOut => "zoom-out".into(),
	}
}

/// The command an action name stands for: the inverse of [`action_name`].
pub fn parse_action(name: &str) -> Option<Command> {
	let n = |p: &str| name.strip_prefix(p).and_then(|v| v.parse::<u16>().ok());
	Some(match name {
		"new" => Command::New,
		"open" => Command::Open,
		"save" => Command::Save,
		"save-as" => Command::SaveAs,
		"exit" => Command::Exit,
		"undo" => Command::Undo,
		"redo" => Command::Redo,
		"bullets" => Command::Bullets,
		"numbering" => Command::Numbering,
		"level-up" => Command::LevelUp,
		"level-down" => Command::LevelDown,
		"first-page" => Command::FirstPage,
		"cut" => Command::Cut,
		"copy" => Command::Copy,
		"paste" => Command::Paste,
		"select-all" => Command::SelectAll,
		"fonts" => Command::Fonts,
		"tabs" => Command::Tabs,
		"about" => Command::About,
		"help" => Command::Help,
		"picture" => Command::Picture,
		"print" => Command::Print,
		"spelling" => Command::Spelling,
		"spell-live" => Command::SpellAsYouType,
		"preview" => Command::Preview,
		"page-setup" => Command::PrintSetup,
		"find" => Command::Find,
		"find-next" => Command::FindNext,
		"replace" => Command::Replace,
		"regular" => Command::Regular,
		"bold" => Command::Bold,
		"italic" => Command::Italic,
		"underline" => Command::Underline,
		"strike" => Command::Strike,
		"superscript" => Command::Super,
		"subscript" => Command::Sub,
		"zoom-in" => Command::ZoomIn,
		"zoom-out" => Command::ZoomOut,
		_ => {
			if let Some(v) = n("view-") {
				Command::View(v as u8)
			} else if let Some(v) = n("field-") {
				Command::Field(v as u8)
			} else if let Some(v) = n("chrome-") {
				Command::Chrome(v as u8)
			} else if let Some(v) = n("recent-") {
				Command::Recent(v as u8)
			} else if let Some(v) = n("align-") {
				Command::Align(v as u8)
			} else if let Some(v) = n("spacing-") {
				Command::Spacing(v as u8)
			} else {
				Command::Zoom(n("zoom-")?)
			}
		}
	})
}

/// True when the command's menu item can carry a check mark, so its action keeps a boolean state.
pub fn is_stateful(c: Command) -> bool {
	matches!(
		c,
		Command::Bold
			| Command::Italic
			| Command::Underline
			| Command::Strike
			| Command::Super
			| Command::Sub
			| Command::Regular
			| Command::Align(_)
			| Command::Spacing(_)
			| Command::Bullets
			| Command::Numbering
			| Command::Chrome(_)
			| Command::SpellAsYouType
			| Command::FirstPage
			| Command::View(_)
			| Command::Zoom(_)
	)
}

/// The commands that act on the text and take Ctrl+C, Ctrl+V and the other editing keys: while a text field of a dialog has the
/// focus these accelerators are put back in the field's hands.
pub fn is_text_key(c: Command) -> bool {
	matches!(
		c,
		Command::Cut
			| Command::Copy
			| Command::Paste
			| Command::SelectAll
			| Command::Undo
			| Command::Redo
	)
}

/// A GTK accelerator for the text a menu item shows at its right (`Ctrl+Shift+=`, `F3`), or `None` for no accelerator.
pub fn accel_of(text: &str) -> Option<String> {
	if text.is_empty() {
		return None;
	}
	let mut out = String::new();
	let parts: Vec<&str> = text.split('+').collect();
	let (key, mods) = match parts.split_last() {
		// "Ctrl+=" splits into ["Ctrl", "="]; "Ctrl++" would split into an empty last part, which no item uses.
		Some((k, m)) if !k.is_empty() => (*k, m),
		_ => return None,
	};
	for m in mods {
		out.push_str(match *m {
			"Ctrl" => "<Ctrl>",
			"Shift" => "<Shift>",
			"Alt" => "<Alt>",
			_ => return None,
		});
	}
	let key = match key {
		"=" => "equal",
		"-" => "minus",
		k => k,
	};
	if key.len() == 1 {
		out.push_str(&key.to_ascii_lowercase());
	} else {
		out.push_str(key);
	}
	Some(out)
}

/// A GTK label from a Windows menu label: `&` marks the mnemonic (`_` in GTK), `&&` is an ampersand, and three dots are an
/// ellipsis.
pub fn label_of(text: &str) -> String {
	let mut out = String::with_capacity(text.len() + 1);
	let mut it = text.chars().peekable();
	while let Some(c) = it.next() {
		match c {
			'&' if it.peek() == Some(&'&') => {
				it.next();
				out.push('&');
			}
			'&' => out.push('_'),
			'_' => out.push_str("__"),
			c => out.push(c),
		}
	}
	out.replace("...", "\u{2026}")
}

/// The label without its mnemonic mark, for a tooltip or a title.
pub fn plain_label(text: &str) -> String {
	label_of(text)
		.replace("__", "\u{1}")
		.replace('_', "")
		.replace('\u{1}', "_")
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn every_action_name_reads_back_as_its_command() {
		let mut all = vec![
			Command::New,
			Command::Open,
			Command::Save,
			Command::SaveAs,
			Command::Exit,
			Command::Undo,
			Command::Redo,
			Command::Bullets,
			Command::Numbering,
			Command::LevelUp,
			Command::LevelDown,
			Command::FirstPage,
			Command::Cut,
			Command::Copy,
			Command::Paste,
			Command::SelectAll,
			Command::Fonts,
			Command::Tabs,
			Command::About,
			Command::Help,
			Command::Picture,
			Command::Print,
			Command::Spelling,
			Command::SpellAsYouType,
			Command::Preview,
			Command::PrintSetup,
			Command::Find,
			Command::FindNext,
			Command::Replace,
			Command::Regular,
			Command::Bold,
			Command::Italic,
			Command::Underline,
			Command::Strike,
			Command::Super,
			Command::Sub,
		];
		all.extend((0..=4).map(Command::View));
		all.extend((1..=5).map(Command::Field));
		all.extend([1, 2, 4, 8].map(Command::Chrome));
		all.extend((0..4).map(Command::Recent));
		all.extend((0..4).map(Command::Align));
		all.extend((0..3).map(Command::Spacing));
		all.extend([0, 50, 75, 100, 150].map(Command::Zoom));
		let mut names = std::collections::HashSet::new();
		for c in all {
			let name = action_name(c);
			assert_eq!(parse_action(&name), Some(c), "{name}");
			assert!(names.insert(name.clone()), "{name} names two commands");
		}
		assert_eq!(parse_action("no-such-action"), None);
	}

	#[test]
	fn the_windows_accelerators_become_gtk_ones() {
		assert_eq!(accel_of("Ctrl+N").as_deref(), Some("<Ctrl>n"));
		assert_eq!(accel_of("F3").as_deref(), Some("F3"));
		assert_eq!(
			accel_of("Ctrl+Shift+=").as_deref(),
			Some("<Ctrl><Shift>equal")
		);
		assert_eq!(accel_of("Ctrl+=").as_deref(), Some("<Ctrl>equal"));
		assert_eq!(accel_of("Ctrl+1").as_deref(), Some("<Ctrl>1"));
		assert_eq!(accel_of(""), None);
		assert_eq!(accel_of("Hyper+X"), None);
	}

	#[test]
	fn labels_carry_their_mnemonics_over() {
		assert_eq!(label_of("&Open..."), "_Open\u{2026}");
		assert_eq!(label_of("Save &As..."), "Save _As\u{2026}");
		assert_eq!(label_of("Tom && Jerry"), "Tom & Jerry");
		assert_eq!(plain_label("&Open..."), "Open\u{2026}");
		assert_eq!(plain_label("1\u{bd} &Space"), "1\u{bd} Space");
	}
}
