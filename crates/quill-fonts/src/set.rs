// The faces available, and the choice of one for a font layout asks for.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::face::Face;

/// The kinds of face the fallback tables know.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Class {
	Serif,
	Sans,
	Mono,
}

/// Face names and what they stand for. The right-hand names are tried in order after the face asked for.
const ALIASES: [(&str, Class); 16] = [
	("times new roman", Class::Serif),
	("times", Class::Serif),
	("ms serif", Class::Serif),
	("liberation serif", Class::Serif),
	("tinos", Class::Serif),
	("arial", Class::Sans),
	("helvetica", Class::Sans),
	("ms sans serif", Class::Sans),
	("liberation sans", Class::Sans),
	("arimo", Class::Sans),
	("courier new", Class::Mono),
	("courier", Class::Mono),
	("liberation mono", Class::Mono),
	("cousine", Class::Mono),
	("lucida console", Class::Mono),
	("fixedsys", Class::Mono),
];

impl Class {
	fn chain(self) -> [&'static str; 3] {
		match self {
			Class::Serif => ["Times New Roman", "Liberation Serif", "Quill Test Serif"],
			Class::Sans => ["Arial", "Liberation Sans", "Quill Test Sans"],
			Class::Mono => ["Courier New", "Liberation Mono", "Quill Test Mono"],
		}
	}

	/// From the font table's family code: 1 roman, 2 swiss, 3 modern; anything else is serif.
	fn from_family(code: u8) -> Class {
		match code {
			2 => Class::Sans,
			3 => Class::Mono,
			_ => Class::Serif,
		}
	}
}

/// The face chosen for a request, and what the face lacks of it.
#[derive(Copy, Clone, Debug)]
pub struct Resolved<'a> {
	/// The face.
	pub face: &'a Face,
	/// Bold was asked for and the face is not bold: the measure widens it.
	pub synthetic_bold: bool,
	/// Italic was asked for and the face is not italic: it is drawn slanted, with the same advances.
	pub synthetic_italic: bool,
}

/// Font faces, searched by family name.
#[derive(Clone, Debug, Default)]
pub struct FontSet {
	faces: Vec<Face>,
}

macro_rules! test_font {
	($name:literal) => {
		include_bytes!(concat!("../fonts/", $name, ".ttf"))
	};
}

impl FontSet {
	/// A set with no faces; `resolve` finds nothing in it.
	pub fn empty() -> FontSet {
		FontSet::default()
	}

	/// The twelve Quill test fonts: serif, sans and mono in four styles. They are always the end of the fallback chain, so a
	/// set that holds them gives the same widths wherever it runs.
	pub fn vendored() -> FontSet {
		let files: [&'static [u8]; 12] = [
			test_font!("QuillTestSerif-Regular"),
			test_font!("QuillTestSerif-Bold"),
			test_font!("QuillTestSerif-Italic"),
			test_font!("QuillTestSerif-BoldItalic"),
			test_font!("QuillTestSans-Regular"),
			test_font!("QuillTestSans-Bold"),
			test_font!("QuillTestSans-Italic"),
			test_font!("QuillTestSans-BoldItalic"),
			test_font!("QuillTestMono-Regular"),
			test_font!("QuillTestMono-Bold"),
			test_font!("QuillTestMono-Italic"),
			test_font!("QuillTestMono-BoldItalic"),
		];
		let mut set = FontSet::empty();
		for f in files {
			set.add(Arc::from(f));
		}
		set
	}

	/// The vendored fonts plus every font file in the usual Linux font folders (`/usr/share/fonts`, `~/.fonts`,
	/// `$XDG_DATA_HOME/fonts`, `$XDG_DATA_DIRS/fonts`). No fontconfig: the folders are read directly.
	pub fn system() -> FontSet {
		let mut set = FontSet::vendored();
		for dir in system_dirs() {
			set.add_dir(&dir);
		}
		set
	}

	/// The family names of the faces, sorted and without repeats: what a font box lists.
	pub fn families(&self) -> Vec<String> {
		let mut names: Vec<String> = self.faces.iter().map(|f| f.family().to_string()).collect();
		names.sort_by_key(|n| n.to_lowercase());
		names.dedup();
		names
	}

	/// Adds the faces of a font file's bytes; returns how many it held.
	pub fn add(&mut self, data: Arc<[u8]>) -> usize {
		let faces = Face::parse_all(&data);
		let n = faces.len();
		self.faces.extend(faces);
		n
	}

	/// Adds every `.ttf`, `.otf`, `.ttc` and `.otc` file under `dir`, folders below it included. Unreadable files and folders
	/// are skipped. Returns the number of faces added.
	pub fn add_dir(&mut self, dir: &Path) -> usize {
		let mut added = 0;
		let Ok(entries) = std::fs::read_dir(dir) else {
			return 0;
		};
		let mut paths: Vec<PathBuf> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
		paths.sort();
		for p in paths {
			if p.is_dir() {
				added += self.add_dir(&p);
			} else if p.extension().and_then(|e| e.to_str()).is_some_and(|e| {
				["ttf", "otf", "ttc", "otc"]
					.iter()
					.any(|x| e.eq_ignore_ascii_case(x))
			}) {
				if let Ok(bytes) = std::fs::read(&p) {
					added += self.add(Arc::from(bytes));
				}
			}
		}
		added
	}

	/// The faces, in the order they were added.
	pub fn faces(&self) -> &[Face] {
		&self.faces
	}

	fn best(&self, family: &str, bold: bool, italic: bool) -> Option<Resolved<'_>> {
		self.faces
			.iter()
			.filter(|f| f.family().eq_ignore_ascii_case(family))
			// An exact style first, then the same slant in the other weight, then the same weight in the other slant.
			.min_by_key(|f| {
				(
					u8::from(f.is_italic() != italic) * 2 + u8::from(f.is_bold() != bold),
					f.index(),
				)
			})
			.map(|f| Resolved {
				face: f,
				synthetic_bold: bold && !f.is_bold(),
				synthetic_italic: italic && !f.is_italic(),
			})
	}

	/// The face for a face name, a family code (as in the document's font table: 1 roman, 2 swiss, 3 modern) and a style.
	///
	/// The face asked for comes first. Then its metric-compatible stand-ins: a serif face tries Times New Roman, Liberation
	/// Serif and the Quill test serif; a sans face Arial, Liberation Sans and the test sans; a monospaced face Courier New,
	/// Liberation Mono and the test mono. A name the tables do not know uses the family code to pick which. `None` only when no
	/// face of the chain is in the set.
	pub fn resolve(
		&self,
		name: &str,
		family: u8,
		bold: bool,
		italic: bool,
	) -> Option<Resolved<'_>> {
		let lower = name.to_ascii_lowercase();
		let class = ALIASES
			.iter()
			.find(|(n, _)| *n == lower)
			.map_or_else(|| Class::from_family(family), |(_, c)| *c);
		std::iter::once(name)
			.chain(class.chain())
			.find_map(|n| self.best(n, bold, italic))
	}
}

fn system_dirs() -> Vec<PathBuf> {
	let mut dirs = Vec::new();
	let home = std::env::var_os("HOME").map(PathBuf::from);
	if let Some(h) = &home {
		dirs.push(h.join(".fonts"));
	}
	match std::env::var_os("XDG_DATA_HOME")
		.map(PathBuf::from)
		.filter(|p| p.is_absolute())
	{
		Some(d) => dirs.push(d.join("fonts")),
		None => dirs.extend(home.map(|h| h.join(".local/share/fonts"))),
	}
	let data =
		std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
	dirs.extend(
		data.split(':')
			.filter(|d| d.starts_with('/'))
			.map(|d| Path::new(d).join("fonts")),
	);
	dirs
}

#[cfg(test)]
mod tests {
	use super::*;

	fn family_of(set: &FontSet, name: &str, code: u8, bold: bool, italic: bool) -> String {
		set.resolve(name, code, bold, italic)
			.map(|r| r.face.family().to_string())
			.unwrap_or_default()
	}

	#[test]
	fn the_vendored_set_answers_every_name_with_a_test_font() {
		let set = FontSet::vendored();
		assert_eq!(set.faces().len(), 12);
		for (name, code, want) in [
			("Times New Roman", 1, "Quill Test Serif"),
			("Arial", 2, "Quill Test Sans"),
			("Courier New", 3, "Quill Test Mono"),
			("Courier New", 1, "Quill Test Mono"),
			("MS Sans Serif", 0, "Quill Test Sans"),
			("Wingdings", 3, "Quill Test Mono"),
			("Brush Script", 4, "Quill Test Serif"),
			("", 0, "Quill Test Serif"),
		] {
			assert_eq!(
				family_of(&set, name, code, false, false),
				want,
				"{name} {code}"
			);
		}
	}

	#[test]
	fn the_chain_prefers_the_name_then_the_real_font_then_the_stand_in() {
		let v = FontSet::vendored();
		let base = v
			.faces()
			.iter()
			.find(|f| f.family() == "Quill Test Serif" && !f.is_bold() && !f.is_italic())
			.unwrap();
		let mut set = v.clone();
		set.faces
			.push(base.renamed("Liberation Serif", false, false));
		assert_eq!(
			family_of(&set, "Times New Roman", 1, false, false),
			"Liberation Serif"
		);
		set.faces
			.push(base.renamed("Times New Roman", false, false));
		assert_eq!(
			family_of(&set, "Times New Roman", 1, false, false),
			"Times New Roman"
		);
		set.faces.push(base.renamed("Georgia", false, false));
		assert_eq!(
			family_of(&set, "georgia", 1, false, false),
			"Georgia",
			"names match without regard to case"
		);
		assert_eq!(family_of(&set, "Arial", 2, false, false), "Quill Test Sans");
	}

	#[test]
	fn a_missing_style_is_flagged_and_the_nearest_face_is_used() {
		let v = FontSet::vendored();
		let regular = v
			.faces()
			.iter()
			.find(|f| f.family() == "Quill Test Serif" && !f.is_bold() && !f.is_italic())
			.unwrap();
		let mut only = FontSet::empty();
		only.faces
			.push(regular.renamed("Times New Roman", false, false));
		let r = only.resolve("Times New Roman", 1, true, true).unwrap();
		assert!(r.synthetic_bold && r.synthetic_italic);
		let r = only.resolve("Times New Roman", 1, false, false).unwrap();
		assert!(!r.synthetic_bold && !r.synthetic_italic);
		assert!(FontSet::empty().resolve("Arial", 2, false, false).is_none());
	}

	#[test]
	fn font_files_in_a_folder_are_found_below_it() {
		let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fonts");
		let mut set = FontSet::empty();
		assert_eq!(set.add_dir(&dir), 12);
		assert_eq!(set.add_dir(&dir.join("missing")), 0);
	}
}
