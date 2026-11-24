// The file types the dialogs offer, from one table, and the filter strings the Windows common dialogs take.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// Which dialog the types are for.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Kind {
	Open,
	SaveAs,
	Picture,
}

/// A document format Quill reads and writes. The number is what `quill-app` keeps as the format of the open file.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Format {
	Qld,
	Rtf,
	Text,
}

impl Format {
	/// 0 for `.QLD`, 1 for `.RTF`, 2 for text.
	pub fn index(self) -> usize {
		match self {
			Format::Qld => 0,
			Format::Rtf => 1,
			Format::Text => 2,
		}
	}

	/// The format with that number; anything else is `.QLD`.
	pub fn from_index(i: usize) -> Format {
		match i {
			1 => Format::Rtf,
			2 => Format::Text,
			_ => Format::Qld,
		}
	}
}

/// One entry of a type list.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct FileType {
	/// The name shown, as on the Windows builds (`Quill Documents`); the patterns are added in brackets.
	pub label: &'static str,
	/// The patterns, lower case as the Windows filters have them, separated by `;`.
	pub patterns: &'static str,
	/// The extension added to a name typed without one, upper case and without the dot; empty for none.
	pub default_ext: &'static str,
	/// The document format this type writes, for the Save As types.
	pub format: Option<Format>,
}

const QLD: FileType = FileType {
	label: "Quill Documents",
	patterns: "*.qld",
	default_ext: "QLD",
	format: Some(Format::Qld),
};
const RTF: FileType = FileType {
	label: "Rich Text Format",
	patterns: "*.rtf",
	default_ext: "RTF",
	format: Some(Format::Rtf),
};
const TXT: FileType = FileType {
	label: "Text Files",
	patterns: "*.txt",
	default_ext: "TXT",
	format: Some(Format::Text),
};
const ALL: FileType = FileType {
	label: "All Files",
	patterns: "*.*",
	default_ext: "",
	format: None,
};

/// Open: Quill documents, RTF, text, all files.
pub const OPEN: [FileType; 4] = [QLD, RTF, TXT, ALL];
/// Save As: the three formats Quill writes. `Text Files (ANSI)` waits for CP437 text writing.
pub const SAVE_AS: [FileType; 3] = [QLD, RTF, TXT];
/// Insert Picture: both, bitmaps, PNG, all files.
pub const PICTURE: [FileType; 4] = [
	FileType {
		label: "Pictures",
		patterns: "*.bmp;*.png",
		default_ext: "",
		format: None,
	},
	FileType {
		label: "Bitmaps",
		patterns: "*.bmp",
		default_ext: "",
		format: None,
	},
	FileType {
		label: "PNG Pictures",
		patterns: "*.png",
		default_ext: "",
		format: None,
	},
	ALL,
];

/// The types of a dialog, in the order the list shows them.
pub fn types(kind: Kind) -> &'static [FileType] {
	match kind {
		Kind::Open => &OPEN,
		Kind::SaveAs => &SAVE_AS,
		Kind::Picture => &PICTURE,
	}
}

/// The type whose only pattern is `*.EXT`, for an extension without its dot (any case). A wildcard never names a type, so the
/// all-files type never matches.
pub fn type_for_ext(kind: Kind, ext: &[u8]) -> Option<usize> {
	if ext.is_empty() || crate::name83::is_wild(ext) {
		return None;
	}
	types(kind).iter().position(|t| {
		t.patterns
			.as_bytes()
			.strip_prefix(b"*.")
			.is_some_and(|e| e.eq_ignore_ascii_case(ext))
	})
}

/// Writes the Windows filter string for a dialog into `out`: `label (patterns)\0patterns\0` for each type and a closing `\0`.
/// Returns the length written, or `None` when `out` is too small. These are the strings `OPENFILENAME.lpstrFilter` takes.
pub fn write_filter(kind: Kind, out: &mut [u8]) -> Option<usize> {
	let mut w = Writer { out, at: 0 };
	for t in types(kind) {
		w.put(t.label.as_bytes())?;
		w.put(b" (")?;
		w.put(t.patterns.as_bytes())?;
		w.put(b")\0")?;
		w.put(t.patterns.as_bytes())?;
		w.put(b"\0")?;
	}
	w.put(b"\0")?;
	Some(w.at)
}

/// Writes how the list shows a type: `Quill Documents (*.QLD)`, with the patterns in upper case as DOS lists names. Returns the
/// length written, or `None` when `out` is too small.
pub fn write_label(t: &FileType, out: &mut [u8]) -> Option<usize> {
	let mut w = Writer { out, at: 0 };
	w.put(t.label.as_bytes())?;
	w.put(b" (")?;
	for b in t.patterns.bytes() {
		w.put(&[b.to_ascii_uppercase()])?;
	}
	w.put(b")")?;
	Some(w.at)
}

struct Writer<'a> {
	out: &'a mut [u8],
	at: usize,
}

impl Writer<'_> {
	fn put(&mut self, b: &[u8]) -> Option<()> {
		let end = self.at.checked_add(b.len())?;
		self.out.get_mut(self.at..end)?.copy_from_slice(b);
		self.at = end;
		Some(())
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
	use crate::name83::matches_any;

	#[test]
	fn the_filter_strings_are_the_ones_the_windows_builds_use() {
		let mut b = [0u8; 256];
		let n = write_filter(Kind::Open, &mut b).unwrap();
		assert_eq!(
			&b[..n],
			b"Quill Documents (*.qld)\0*.qld\0Rich Text Format (*.rtf)\0*.rtf\0Text Files (*.txt)\0*.txt\0All Files (*.*)\0*.*\0\0"
		);
		let n = write_filter(Kind::Picture, &mut b).unwrap();
		assert_eq!(
			&b[..n],
			b"Pictures (*.bmp;*.png)\0*.bmp;*.png\0Bitmaps (*.bmp)\0*.bmp\0PNG Pictures (*.png)\0*.png\0All Files (*.*)\0*.*\0\0"
		);
		let n = write_filter(Kind::SaveAs, &mut b).unwrap();
		assert_eq!(
			&b[..n],
			b"Quill Documents (*.qld)\0*.qld\0Rich Text Format (*.rtf)\0*.rtf\0Text Files (*.txt)\0*.txt\0\0"
		);
	}

	#[test]
	fn a_small_buffer_is_refused_not_overrun() {
		for kind in [Kind::Open, Kind::SaveAs, Kind::Picture] {
			let mut big = [0u8; 256];
			let need = write_filter(kind, &mut big).unwrap();
			for len in 0..need {
				assert!(write_filter(kind, &mut big[..len]).is_none());
			}
			assert_eq!(write_filter(kind, &mut big[..need]), Some(need));
		}
	}

	#[test]
	fn labels_show_the_patterns_in_upper_case() {
		let mut b = [0u8; 64];
		let n = write_label(&PICTURE[0], &mut b).unwrap();
		assert_eq!(&b[..n], b"Pictures (*.BMP;*.PNG)");
		assert!(write_label(&PICTURE[0], &mut b[..5]).is_none());
	}

	#[test]
	fn types_match_their_extensions() {
		assert_eq!(type_for_ext(Kind::SaveAs, b"rtf"), Some(1));
		assert_eq!(type_for_ext(Kind::SaveAs, b"TXT"), Some(2));
		assert_eq!(type_for_ext(Kind::SaveAs, b"DOC"), None);
		assert_eq!(type_for_ext(Kind::SaveAs, b""), None);
		assert_eq!(type_for_ext(Kind::Open, b"*"), None);
		assert_eq!(type_for_ext(Kind::Picture, b"png"), Some(2));
		for (i, t) in SAVE_AS.iter().enumerate() {
			assert_eq!(t.format.map(Format::index), Some(i));
			assert_eq!(Format::from_index(i).index(), i);
		}
		assert_eq!(Format::from_index(9), Format::Qld);
	}

	#[test]
	fn the_patterns_select_what_they_say() {
		assert!(matches_any(PICTURE[0].patterns.as_bytes(), b"A.PNG"));
		assert!(matches_any(PICTURE[0].patterns.as_bytes(), b"a.bmp"));
		assert!(!matches_any(PICTURE[1].patterns.as_bytes(), b"A.PNG"));
		assert!(matches_any(OPEN[3].patterns.as_bytes(), b"README"));
		assert!(!matches_any(OPEN[0].patterns.as_bytes(), b"A.RTF"));
	}

	#[test]
	fn every_default_extension_is_a_pattern_of_its_type() {
		for kind in [Kind::Open, Kind::SaveAs, Kind::Picture] {
			for t in types(kind) {
				if !t.default_ext.is_empty() {
					let mut name = [0u8; 12];
					name[..2].copy_from_slice(b"A.");
					name[2..2 + t.default_ext.len()].copy_from_slice(t.default_ext.as_bytes());
					assert!(matches_any(
						t.patterns.as_bytes(),
						&name[..2 + t.default_ext.len()]
					));
				}
			}
		}
	}
}
