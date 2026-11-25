// The Windows code pages RTF names (1250, 1251, 1252, 1253, 1254 and 1257) as tables to Unicode.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A document holds Windows-1252 bytes (D11), but an RTF file says which code page its `\'hh` escapes are in (`\ansicpgN` for the
//! document, `\fcharsetN` for a font). Reading one is two steps: [`to_unicode`] turns a byte of that page into the character, and
//! the reader then keeps what the document can hold. Splitting them lets the second step change (UTF-8 storage, CORE-63) without
//! the tables moving.
//!
//! The tables are `scripts/gen-codepages.py`'s output from Python's codecs. A byte a page leaves undefined is the C1 control of the
//! same number below 0xA0 (as [`crate::cp1252`] does, so every byte round-trips) and U+FFFD above.
//!
//! The 16-bit Windows build must not keep them in its 64 KB data segment (D200), so it serves [`to_unicode`] from a resource of
//! 1,536 bytes (`assets/codepages/CPAGES.BIN`, laid out as [`resource_offset`] says) through `ReadStore::to_unicode`.

use crate::cpdata::HIGH;

/// A code page the RTF reader converts.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CodePage {
	/// Central European.
	Cp1250,
	/// Cyrillic.
	Cp1251,
	/// Western European, the document's own.
	Cp1252,
	/// Greek.
	Cp1253,
	/// Turkish.
	Cp1254,
	/// Baltic.
	Cp1257,
}

/// The size of the table resource: six pages of 128 two-byte entries.
pub const RESOURCE_LEN: usize = 6 * 128 * 2;

impl CodePage {
	/// Every page, in table order.
	pub const ALL: [CodePage; 6] = [
		CodePage::Cp1250,
		CodePage::Cp1251,
		CodePage::Cp1252,
		CodePage::Cp1253,
		CodePage::Cp1254,
		CodePage::Cp1257,
	];

	/// The page's place in the table and the resource.
	pub const fn index(self) -> usize {
		match self {
			CodePage::Cp1250 => 0,
			CodePage::Cp1251 => 1,
			CodePage::Cp1252 => 2,
			CodePage::Cp1253 => 3,
			CodePage::Cp1254 => 4,
			CodePage::Cp1257 => 5,
		}
	}

	/// The Windows number (`1251`).
	pub const fn number(self) -> u16 {
		match self {
			CodePage::Cp1250 => 1250,
			CodePage::Cp1251 => 1251,
			CodePage::Cp1252 => 1252,
			CodePage::Cp1253 => 1253,
			CodePage::Cp1254 => 1254,
			CodePage::Cp1257 => 1257,
		}
	}

	/// The page `\ansicpgN` names, when it is one of these.
	pub fn from_number(n: i64) -> Option<CodePage> {
		Some(match n {
			1250 => CodePage::Cp1250,
			1251 => CodePage::Cp1251,
			1252 => CodePage::Cp1252,
			1253 => CodePage::Cp1253,
			1254 => CodePage::Cp1254,
			1257 => CodePage::Cp1257,
			_ => return None,
		})
	}

	/// The page a font's `\fcharsetN` means: 0 ANSI, 238 Central European, 204 Cyrillic, 161 Greek, 162 Turkish and 186 Baltic.
	pub fn from_charset(n: i64) -> Option<CodePage> {
		Some(match n {
			0 => CodePage::Cp1252,
			238 => CodePage::Cp1250,
			204 => CodePage::Cp1251,
			161 => CodePage::Cp1253,
			162 => CodePage::Cp1254,
			186 => CodePage::Cp1257,
			_ => return None,
		})
	}
}

/// Where the entry for `byte` (0x80 or more) of `cp` is in the Win16 resource, in bytes: two little-endian bytes each.
pub const fn resource_offset(cp: CodePage, byte: u8) -> usize {
	(cp.index() * 128 + (byte as usize & 0x7F)) * 2
}

/// The Unicode character for a byte of `cp`. Bytes below 0x80 are ASCII in every page.
pub fn to_unicode(cp: CodePage, byte: u8) -> char {
	if byte < 0x80 {
		return char::from(byte);
	}
	HIGH.get(cp.index())
		.and_then(|row| row.get(usize::from(byte - 0x80)))
		.and_then(|&u| char::from_u32(u32::from(u)))
		.unwrap_or('\u{FFFD}')
}
