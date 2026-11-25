// Names: how a path shows in the title, the status line and the recent list, and the text of a failed open.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::Path;

use quill_formats::FormatError;
use quill_tty::cp1252;

/// The file name of a path as the person sees it: UTF-8, long, in the case the file system has.
pub fn display_name(path: &Path) -> String {
	path.file_name()
		.map(|n| n.to_string_lossy().into_owned())
		.unwrap_or_default()
}

/// The text of a name the editor stores (an absolute path in Windows-1252 on this client), shown as its file name.
pub fn display_editor_name(name: &[u8]) -> String {
	let text = cp1252::ansi_to_string(name);
	text.rsplit('/').next().unwrap_or(&text).to_string()
}

/// A name the editor can hold, from a long Linux name: Windows-1252 only, with `?` for a character that does not fit, cut to
/// `max` bytes.
pub fn lossy_ansi(name: &str, max: usize) -> Vec<u8> {
	name.chars()
		.map(|c| cp1252::encode(c).unwrap_or(b'?'))
		.take(max)
		.collect()
}

/// The window title: `*Name – Quill` with a `*` before the name when there are unsaved changes, and `Quill` when no document has
/// a name. (Windows puts the mark after the name; Linux puts it before, where a narrow tab or task switcher still shows it.)
pub fn title(name: Option<&str>, modified: bool) -> String {
	match name {
		Some(n) if modified => format!("*{n} \u{2013} Quill"),
		Some(n) => format!("{n} \u{2013} Quill"),
		None if modified => "*Quill".to_string(),
		None => "Quill".to_string(),
	}
}

/// What to say when a file would not open.
pub fn open_failure(name: &str, e: FormatError) -> String {
	match e {
		FormatError::BadFile => format!("{name} is not a document Quill can read."),
		FormatError::Newer => format!("{name} was written by a newer version of Quill."),
		FormatError::TooLarge => format!("{name} is too large for Quill."),
		FormatError::NoMemory => format!("There is not enough memory to open {name}."),
		FormatError::Write => format!("{name} could not be read."),
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn the_title_marks_a_changed_document_before_its_name() {
		assert_eq!(
			title(Some("Letter.rtf"), false),
			"Letter.rtf \u{2013} Quill"
		);
		assert_eq!(
			title(Some("Letter.rtf"), true),
			"*Letter.rtf \u{2013} Quill"
		);
		assert_eq!(title(None, false), "Quill");
		assert_eq!(title(None, true), "*Quill");
	}

	#[test]
	fn a_long_name_shows_whole_and_an_editor_name_shows_its_last_part() {
		assert_eq!(
			display_name(Path::new("/home/a/My long résumé.rtf")),
			"My long résumé.rtf"
		);
		assert_eq!(
			display_editor_name(b"/home/a/Caf\xE9 notes.rtf"),
			"Caf\u{e9} notes.rtf"
		);
	}

	#[test]
	fn characters_windows_1252_lacks_become_question_marks() {
		assert_eq!(lossy_ansi("a\u{4e2d}b\u{e9}", 8), b"a?b\xE9");
		assert_eq!(lossy_ansi("abcdef", 3), b"abc");
	}
}
