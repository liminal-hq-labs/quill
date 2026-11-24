// What Save As starts with, and how the name follows the type: the extension swap and the default extension.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Save As starts with the document's own name and the type its extension names (or the format it was opened in). Changing the
// type keeps the name and swaps only the extension; a name typed with no extension gets the type's. The Windows 95 builds swap
// only the extensions of Quill's own types ([`Rule::KnownOnly`]); on DOS the name has room for one extension, so any is replaced
// ([`Rule::Any`]).

use crate::name83::{is_wild, last_part, PathBuf};
use crate::types::{type_for_ext, Format, Kind};

/// Which extensions a type change replaces.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Rule {
	/// Any extension is replaced (DOS: 8.3 has only one).
	Any,
	/// Only `QLD`, `RTF` and `TXT` are replaced; any other extension is kept and the new one added after it (Win32).
	KnownOnly,
}

/// True for the extensions of the Save As types, any case, without the dot.
pub fn is_known_ext(ext: &[u8]) -> bool {
	type_for_ext(Kind::SaveAs, ext).is_some()
}

/// The extension of the last part of `name`, without the dot: `Some("")` for `LETTER.`, `None` when there is no dot.
pub fn ext_of(name: &[u8]) -> Option<&[u8]> {
	let part = last_part(name);
	let dot = part.iter().rposition(|b| *b == b'.')?;
	part.get(dot + 1..)
}

/// The name with `ext` (no dot) as its extension, for a change of type. The name is returned as it was when it is empty, holds a
/// wildcard (it is going to be the list's pattern) or `ext` is empty (the all-files type). `None` when the result would not fit.
pub fn swap_extension(name: &[u8], ext: &[u8], rule: Rule) -> Option<PathBuf> {
	if name.is_empty() || ext.is_empty() || is_wild(name) {
		return PathBuf::from_bytes(name);
	}
	let cut = match ext_of(name) {
		Some(old) if rule == Rule::Any || is_known_ext(old) => name.len() - old.len() - 1,
		_ => name.len(),
	};
	let mut out = PathBuf::from_bytes(name.get(..cut)?)?;
	if !out.push_byte(b'.') || !out.push_bytes(ext) {
		return None;
	}
	Some(out)
}

/// The name with the type's default extension added when the last part has no dot. A trailing dot means no extension, so it is
/// left alone; so are an empty name, a wildcard and a type with no default.
pub fn with_default_ext(name: &[u8], default_ext: &[u8]) -> Option<PathBuf> {
	if name.is_empty() || default_ext.is_empty() || is_wild(name) || ext_of(name).is_some() {
		return PathBuf::from_bytes(name);
	}
	let mut out = PathBuf::from_bytes(name)?;
	if !out.push_byte(b'.') || !out.push_bytes(default_ext) {
		return None;
	}
	Some(out)
}

/// What the Save As dialog opens with.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Initial {
	/// The text of the File name field: the document's own name, empty for an untitled document.
	pub name: PathBuf,
	/// The document's folder, empty when it has none (the caller falls back to `Folder=` and then the current folder).
	pub folder: PathBuf,
	/// The index in the Save As types.
	pub type_index: usize,
}

/// The starting name, folder and type for a document at `doc_path` (empty when untitled) that was opened as `opened`. The type
/// is the one the extension names; with any other extension it is the format the file was opened in.
pub fn initial(doc_path: &[u8], opened: Format) -> Initial {
	let name = last_part(doc_path);
	let folder = doc_path.get(..doc_path.len() - name.len());
	let mut folder = folder.and_then(PathBuf::from_bytes).unwrap_or_default();
	// `C:\DOCS\` is the folder `C:\DOCS`, and `C:\` stays the root.
	if !folder.is_root() && folder.as_bytes().last() == Some(&b'\\') {
		let n = folder.len() - 1;
		folder.truncate(n);
	}
	let type_index = if name.is_empty() {
		0
	} else {
		ext_of(name)
			.and_then(|e| type_for_ext(Kind::SaveAs, e))
			.unwrap_or_else(|| opened.index())
	};
	Initial {
		name: PathBuf::from_bytes(name).unwrap_or_default(),
		folder,
		type_index,
	}
}

#[cfg(test)]
#[allow(
	clippy::unwrap_used,
	clippy::expect_used,
	clippy::panic,
	clippy::indexing_slicing
)]
mod tests {
	use super::*;
	use crate::types::SAVE_AS;
	use proptest::prelude::*;
	use std::vec::Vec;

	fn swap(name: &str, ext: &str, rule: Rule) -> std::string::String {
		let p = swap_extension(name.as_bytes(), ext.as_bytes(), rule).unwrap();
		std::string::String::from_utf8(p.as_bytes().to_vec()).unwrap()
	}

	fn dflt(name: &str, ext: &str) -> std::string::String {
		let p = with_default_ext(name.as_bytes(), ext.as_bytes()).unwrap();
		std::string::String::from_utf8(p.as_bytes().to_vec()).unwrap()
	}

	#[test]
	fn dos_replaces_any_extension() {
		assert_eq!(swap("LETTER.QLD", "RTF", Rule::Any), "LETTER.RTF");
		assert_eq!(swap("LETTER.DOC", "RTF", Rule::Any), "LETTER.RTF");
		assert_eq!(swap("LETTER", "TXT", Rule::Any), "LETTER.TXT");
		assert_eq!(swap("LETTER.", "TXT", Rule::Any), "LETTER.TXT");
		assert_eq!(
			swap("C:\\A.B\\LETTER", "TXT", Rule::Any),
			"C:\\A.B\\LETTER.TXT"
		);
		assert_eq!(
			swap("C:\\A.B\\LETTER.QLD", "TXT", Rule::Any),
			"C:\\A.B\\LETTER.TXT"
		);
	}

	#[test]
	fn win32_replaces_only_known_extensions() {
		assert_eq!(swap("report.qld", "RTF", Rule::KnownOnly), "report.RTF");
		assert_eq!(swap("report.TXT", "QLD", Rule::KnownOnly), "report.QLD");
		assert_eq!(swap("report.v2", "RTF", Rule::KnownOnly), "report.v2.RTF");
		assert_eq!(swap("report", "RTF", Rule::KnownOnly), "report.RTF");
	}

	#[test]
	fn names_that_are_left_alone() {
		for rule in [Rule::Any, Rule::KnownOnly] {
			assert_eq!(swap("", "RTF", rule), "");
			assert_eq!(swap("*.QLD", "RTF", rule), "*.QLD");
			assert_eq!(swap("LET?ER", "RTF", rule), "LET?ER");
			assert_eq!(swap("LETTER.QLD", "", rule), "LETTER.QLD");
		}
	}

	#[test]
	fn the_default_extension() {
		assert_eq!(dflt("LETTER", "QLD"), "LETTER.QLD");
		assert_eq!(dflt("LETTER.RTF", "QLD"), "LETTER.RTF");
		assert_eq!(dflt("LETTER.", "QLD"), "LETTER.");
		assert_eq!(dflt("C:\\A.B\\LETTER", "QLD"), "C:\\A.B\\LETTER.QLD");
		assert_eq!(dflt("*", "QLD"), "*");
		assert_eq!(dflt("", "QLD"), "");
		assert_eq!(dflt("LETTER", ""), "LETTER");
	}

	#[test]
	fn a_result_that_does_not_fit_is_none() {
		let long = [b'A'; 78];
		assert!(swap_extension(&long, b"RTF", Rule::Any).is_none());
		assert!(with_default_ext(&long, b"QLD").is_none());
		assert!(PathBuf::from_bytes(&[b'A'; 80]).is_none());
		assert!(swap_extension(&[b'A'; 80], b"RTF", Rule::Any).is_none());
	}

	#[test]
	fn the_start_follows_the_document() {
		let i = initial(b"C:\\DOCS\\LETTER.RTF", Format::Qld);
		assert_eq!(i.name.as_bytes(), b"LETTER.RTF");
		assert_eq!(i.folder.as_bytes(), b"C:\\DOCS");
		assert_eq!(i.type_index, 1);
		let i = initial(b"C:\\LETTER.QLD", Format::Rtf);
		assert_eq!(i.folder.as_bytes(), b"C:\\");
		assert_eq!(i.type_index, 0);
		let i = initial(b"NOTES.DOC", Format::Text);
		assert_eq!(i.folder.as_bytes(), b"");
		assert_eq!(i.type_index, 2);
		let i = initial(b"NOTES", Format::Rtf);
		assert_eq!(i.type_index, 1);
		let i = initial(b"", Format::Rtf);
		assert_eq!(i.name.as_bytes(), b"");
		assert_eq!(i.type_index, 0);
		let i = initial(b"D:NOTES.TXT", Format::Qld);
		assert_eq!(i.folder.as_bytes(), b"D:");
		assert_eq!(i.type_index, 2);
	}

	fn name() -> impl Strategy<Value = Vec<u8>> {
		proptest::collection::vec(
			prop_oneof![
				Just(b'.'),
				Just(b'\\'),
				Just(b'*'),
				Just(b'?'),
				0x41u8..0x4B,
				0x61u8..0x64
			],
			0..30,
		)
	}

	proptest! {
		#[test]
		fn the_swap_is_idempotent_and_keeps_the_base(n in name(), t in 0usize..3, any_rule in any::<bool>()) {
			let rule = if any_rule { Rule::Any } else { Rule::KnownOnly };
			let ext = SAVE_AS[t].default_ext.as_bytes();
			let Some(once) = swap_extension(&n, ext, rule) else { return Ok(()); };
			let twice = swap_extension(once.as_bytes(), ext, rule).unwrap();
			prop_assert_eq!(once, twice);
			if !n.is_empty() && !is_wild(&n) {
				// Everything before the old extension is still there, and the new one follows it.
				let keep = match ext_of(&n) {
					Some(old) if rule == Rule::Any || is_known_ext(old) => n.len() - old.len() - 1,
					_ => n.len(),
				};
				prop_assert_eq!(&once.as_bytes()[..keep], &n[..keep]);
				prop_assert_eq!(&once.as_bytes()[keep + 1..], ext);
				prop_assert_eq!(once.as_bytes()[keep], b'.');
			}
		}

		#[test]
		fn the_default_extension_is_idempotent(n in name(), t in 0usize..3) {
			let ext = SAVE_AS[t].default_ext.as_bytes();
			let Some(once) = with_default_ext(&n, ext) else { return Ok(()); };
			prop_assert_eq!(with_default_ext(once.as_bytes(), ext).unwrap(), once);
		}

		#[test]
		fn nothing_panics(doc in name(), f in 0usize..3) {
			let i = initial(&doc, Format::from_index(f));
			prop_assert!(i.type_index < SAVE_AS.len());
			prop_assert!(i.folder.len() + i.name.len() <= doc.len());
		}
	}
}
