// One font face, read once: its names, style and the numbers layout needs.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::Arc;

use quill_core::layout::GLYPHS;
use skrifa::raw::{FileRef, TableProvider};
use skrifa::string::StringId;
use skrifa::{GlyphId, MetadataProvider};

use crate::cp1252;

/// Why a font file could not be used.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LoadError {
	/// Not an OpenType or TrueType font, or damaged.
	BadFont,
	/// A table layout needs is missing (`head`, `hhea`, `hmtx`, `OS/2` or `cmap`).
	MissingTable,
}

/// A font face: where its bytes are, what it is called and the measurements layout takes from it. Nothing here depends on a size.
#[derive(Clone, Debug)]
pub struct Face {
	data: Arc<[u8]>,
	index: u32,
	family: String,
	bold: bool,
	italic: bool,
	units_per_em: u16,
	advances: [u16; GLYPHS],
	win_ascent: u16,
	win_descent: u16,
	hhea_ascent: i16,
	hhea_descent: i16,
	line_gap: i16,
}

impl Face {
	/// Reads the face at `index` of a font file's bytes (0 for a single font; a collection has several).
	pub fn parse(data: Arc<[u8]>, index: u32) -> Result<Face, LoadError> {
		let font = match FileRef::new(&data[..]).map_err(|_| LoadError::BadFont)? {
			FileRef::Font(f) if index == 0 => f,
			FileRef::Font(_) => return Err(LoadError::BadFont),
			FileRef::Collection(c) => c.get(index).map_err(|_| LoadError::BadFont)?,
		};
		let t = |_| LoadError::MissingTable;
		let units_per_em = font.head().map_err(t)?.units_per_em();
		if units_per_em == 0 {
			return Err(LoadError::BadFont);
		}
		let os2 = font.os2().map_err(t)?;
		let hhea = font.hhea().map_err(t)?;
		let hmtx = font.hmtx().map_err(t)?;
		let sel = os2.fs_selection();
		let bold = os2.us_weight_class() >= 600;
		let italic = sel.contains(skrifa::raw::tables::os2::SelectionFlags::ITALIC)
			|| sel.contains(skrifa::raw::tables::os2::SelectionFlags::OBLIQUE);
		let family = font
			.localized_strings(StringId::FAMILY_NAME)
			.english_or_first()
			.map(|s| s.to_string())
			.unwrap_or_default();
		let charmap = font.charmap();
		let notdef = hmtx.advance(GlyphId::NOTDEF).unwrap_or(0);
		let mut advances = [notdef; GLYPHS];
		for (i, a) in advances.iter_mut().enumerate() {
			let ch = cp1252::to_char(0x20 + i as u8);
			if let Some(gid) = charmap.map(ch) {
				*a = hmtx.advance(gid).unwrap_or(notdef);
			}
		}
		Ok(Face {
			index,
			family,
			bold,
			italic,
			units_per_em,
			advances,
			win_ascent: os2.us_win_ascent(),
			win_descent: os2.us_win_descent(),
			hhea_ascent: hhea.ascender().to_i16(),
			hhea_descent: hhea.descender().to_i16(),
			line_gap: hhea.line_gap().to_i16(),
			data,
		})
	}

	/// The faces of a font file's bytes: one, or every face of a collection. A damaged one is skipped.
	pub fn parse_all(data: &Arc<[u8]>) -> Vec<Face> {
		let n = match FileRef::new(&data[..]) {
			Ok(FileRef::Font(_)) => 1,
			Ok(FileRef::Collection(c)) => c.len(),
			Err(_) => 0,
		};
		(0..n)
			.filter_map(|i| Face::parse(data.clone(), i).ok())
			.collect()
	}

	/// The font file's bytes.
	pub fn bytes(&self) -> &[u8] {
		&self.data
	}

	/// The face's index in its file.
	pub fn index(&self) -> u32 {
		self.index
	}

	/// The family name (name ID 1), such as `Liberation Serif`.
	pub fn family(&self) -> &str {
		&self.family
	}

	/// True for a bold face (weight 600 or more).
	pub fn is_bold(&self) -> bool {
		self.bold
	}

	/// True for an italic or oblique face.
	pub fn is_italic(&self) -> bool {
		self.italic
	}

	/// Font units to the em.
	pub fn units_per_em(&self) -> u16 {
		self.units_per_em
	}

	/// The advance of Windows-1252 character `ch` (0x20 to 0xFF) in font units; `.notdef`'s when the font lacks it.
	pub fn advance(&self, ch: u8) -> u16 {
		self.advances
			.get(usize::from(ch.saturating_sub(0x20)))
			.copied()
			.unwrap_or(0)
	}

	/// `usWinAscent` and `usWinDescent`, the extent GDI takes for a font's ascent and descent.
	pub fn win_extent(&self) -> (u16, u16) {
		(self.win_ascent, self.win_descent)
	}

	/// External leading in font units: what the `hhea` line (ascender, descender and gap) holds beyond the Windows extent.
	pub fn external_leading(&self) -> u32 {
		let hhea =
			i32::from(self.hhea_ascent) - i32::from(self.hhea_descent) + i32::from(self.line_gap);
		let win = i32::from(self.win_ascent) + i32::from(self.win_descent);
		u32::try_from((hhea - win).max(0)).unwrap_or(0)
	}
}

#[cfg(test)]
impl Face {
	/// A copy under another family name and style, for tests of the fallback order.
	pub(crate) fn renamed(&self, family: &str, bold: bool, italic: bool) -> Face {
		Face {
			family: family.to_string(),
			bold,
			italic,
			..self.clone()
		}
	}
}
