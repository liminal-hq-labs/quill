// The twelve source faces of a set: serif, sans and mono in four styles.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// Face numbers in `QUILL.FNT`; they match the document font table's family codes minus one (roman, swiss, modern).
pub const FACE_NAMES: [&str; 3] = ["Serif", "Sans", "Mono"];

/// Style numbers: bit 0 is bold, bit 1 is italic, as in the `cflag` bits layout reads.
pub const STYLE_NAMES: [&str; 4] = ["Regular", "Bold", "Italic", "Bold Italic"];

/// The faces to render: three families of four styles each.
#[derive(Copy, Clone)]
pub struct FaceSet {
	/// Short name used in file names and tables.
	pub id: &'static str,
	/// What the set is called in prose.
	pub title: &'static str,
	/// Font files, indexed `[face][style]`.
	pub fonts: [[&'static [u8]; 4]; 3],
}

macro_rules! desk {
	($n:literal) => {
		include_bytes!(concat!("../../../assets/fonts/desk/desk-", $n, ".ttf")) as &[u8]
	};
}

macro_rules! test {
	($n:literal) => {
		include_bytes!(concat!("../../quill-fonts/fonts/QuillTest", $n, ".ttf")) as &[u8]
	};
}

/// Desk Serif, Sans and Mono (SIL OFL 1.1, no Reserved Font Name).
pub const DESK: FaceSet = FaceSet {
	id: "desk",
	title: "Desk Serif, Sans and Mono",
	fonts: [
		[
			desk!("serif"),
			desk!("serif-bold"),
			desk!("serif-italic"),
			desk!("serif-bold-italic"),
		],
		[
			desk!("sans"),
			desk!("sans-bold"),
			desk!("sans-italic"),
			desk!("sans-bold-italic"),
		],
		[
			desk!("mono"),
			desk!("mono-bold"),
			desk!("mono-italic"),
			desk!("mono-bold-italic"),
		],
	],
};

/// The Liberation-derived Quill Test Serif, Sans and Mono (metric compatible with Times New Roman, Arial and Courier New).
pub const QUILL_TEST: FaceSet = FaceSet {
	id: "test",
	title: "Quill Test Serif, Sans and Mono (Liberation-derived)",
	fonts: [
		[
			test!("Serif-Regular"),
			test!("Serif-Bold"),
			test!("Serif-Italic"),
			test!("Serif-BoldItalic"),
		],
		[
			test!("Sans-Regular"),
			test!("Sans-Bold"),
			test!("Sans-Italic"),
			test!("Sans-BoldItalic"),
		],
		[
			test!("Mono-Regular"),
			test!("Mono-Bold"),
			test!("Mono-Italic"),
			test!("Mono-BoldItalic"),
		],
	],
};

/// Every set, in report order.
pub const SETS: [FaceSet; 2] = [DESK, QUILL_TEST];

/// The set named `id`.
pub fn by_id(id: &str) -> Option<FaceSet> {
	SETS.iter().copied().find(|s| s.id == id)
}

/// The Unicode character of a Windows-1252 byte.
pub fn cp1252(b: u8) -> char {
	const HIGH: [char; 32] = [
		'€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8d}', 'Ž',
		'\u{8f}', '\u{90}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9d}',
		'ž', 'Ÿ',
	];
	match b {
		0x80..=0x9F => HIGH[usize::from(b - 0x80)],
		_ => char::from(b),
	}
}
