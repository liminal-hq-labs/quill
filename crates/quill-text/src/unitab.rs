// The Unicode tables as one byte blob, and the search every table shares.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Every table of this crate (`cluster`, `breaks`, `word`, `fold`, `compose`, `width`, `script`, `blocks`) is a
//! sequence of 32-bit words. Built in, they are `const` arrays; as `assets/unicode/UNITAB.BIN`, the same words are
//! one blob that a build can serve from a resource (Windows 3.x keeps `const` data in its one data segment), and
//! [`tables`] reads it through the same lookups. The generator, `scripts/gen-unicode.py`, writes both from one source.
//!
//! A blob is a header and ten sections, all little-endian:
//!
//! - `"LIUT"`, the version (`1`, a `u16`) and the number of sections (`10`, a `u16`);
//! - for each section, in the order [`Tables`] lists them, its offset from the start of the blob and its length in
//!   bytes, both `u16`;
//! - the sections, each padded to a multiple of four bytes.
//!
//! A *range table* is a run of words `start << 8 | value`, sorted by start, the first starting at U+0000; a code point
//! has the value of the last entry that starts at or before it. Fold runs are two words each and the letters of
//! [`compose`](crate::compose) one; the block labels are bytes. [`tables`] checks all of this once, so the lookups
//! after it cannot be handed a table that would make them wander.

use crate::{blocks, breaks, cluster, compose, fold, script, width, word};

/// The last Unicode code point.
pub(crate) const LAST: u32 = 0x10_FFFF;

/// One table's words: built in, or read from a blob's bytes.
#[derive(Clone, Copy)]
pub(crate) enum Seq<'a> {
	/// A `const` table.
	Words(&'a [u32]),
	/// Four little-endian bytes a word.
	Bytes(&'a [u8]),
}

impl Seq<'_> {
	/// The number of words.
	pub(crate) fn len(self) -> usize {
		match self {
			Seq::Words(w) => w.len(),
			Seq::Bytes(b) => b.len() / 4,
		}
	}

	/// The word at `i`, or 0 past the end.
	pub(crate) fn get(self, i: usize) -> u32 {
		match self {
			Seq::Words(w) => w.get(i).copied().unwrap_or(0),
			Seq::Bytes(b) => {
				let at = i.saturating_mul(4);
				b.get(at..at.saturating_add(4))
					.and_then(|four| <[u8; 4]>::try_from(four).ok())
					.map_or(0, u32::from_le_bytes)
			}
		}
	}
}

/// A range table's word for a run starting at `start` with `value`.
pub(crate) const fn pack(start: u32, value: u8) -> u32 {
	(start << 8) | value as u32
}

/// The value of `cp` in a range table; 0 beyond U+10FFFF.
pub(crate) fn lookup(table: Seq<'_>, cp: u32) -> u8 {
	if cp > LAST {
		return 0;
	}
	// The first entry whose start is above `cp`; the one before it holds the value.
	let (mut low, mut high) = (0, table.len());
	while low < high {
		let middle = low + (high - low) / 2;
		if table.get(middle) >> 8 <= cp {
			low = middle + 1;
		} else {
			high = middle;
		}
	}
	match low.checked_sub(1) {
		Some(i) => (table.get(i) & 0xFF) as u8,
		None => 0,
	}
}

/// Every table, read from a blob. See [`tables`].
#[derive(Clone, Copy)]
pub struct Tables<'a> {
	pub(crate) cluster: Seq<'a>,
	pub(crate) breaks: Seq<'a>,
	pub(crate) word: Seq<'a>,
	pub(crate) fold: Seq<'a>,
	pub(crate) compose: Seq<'a>,
	pub(crate) width: Seq<'a>,
	pub(crate) script: Seq<'a>,
	pub(crate) blocks: Seq<'a>,
	pub(crate) names: &'a [u8],
	pub(crate) short: Seq<'a>,
}

/// The section count, and so the header's size is `8 + 4 * SECTIONS`.
const SECTIONS: usize = 10;

/// Reads `blob`, a copy of `assets/unicode/UNITAB.BIN`, or `None` when it is not one: the wrong header, a section
/// outside the blob, or a table that is not sorted or does not start at U+0000. The blob is checked here once; the
/// lookups after it trust what was checked.
pub fn tables(blob: &[u8]) -> Option<Tables<'_>> {
	if blob.get(..4)? != b"LIUT" {
		return None;
	}
	let u16_at = |at: usize| -> Option<usize> {
		let two = <[u8; 2]>::try_from(blob.get(at..at + 2)?).ok()?;
		Some(usize::from(u16::from_le_bytes(two)))
	};
	if u16_at(4)? != 1 || u16_at(6)? != SECTIONS {
		return None;
	}
	let section = |n: usize| -> Option<&[u8]> {
		let at = 8 + 4 * n;
		let (offset, length) = (u16_at(at)?, u16_at(at + 2)?);
		blob.get(offset..offset.checked_add(length)?)
	};
	let tables = Tables {
		cluster: Seq::Bytes(section(0)?),
		breaks: Seq::Bytes(section(1)?),
		word: Seq::Bytes(section(2)?),
		fold: Seq::Bytes(section(3)?),
		compose: Seq::Bytes(section(4)?),
		width: Seq::Bytes(section(5)?),
		script: Seq::Bytes(section(6)?),
		blocks: Seq::Bytes(section(7)?),
		names: section(8)?,
		short: Seq::Bytes(section(9)?),
	};
	for ranges in [
		tables.cluster,
		tables.breaks,
		tables.word,
		tables.width,
		tables.script,
		tables.blocks,
	] {
		if !valid_ranges(ranges) {
			return None;
		}
	}
	if !fold::valid(tables.fold)
		|| !compose::valid(tables.compose)
		|| !blocks::valid(tables.names, tables.short)
	{
		return None;
	}
	Some(tables)
}

/// A range table is whole words, starts at U+0000 and goes up in starts.
pub(crate) fn valid_ranges(table: Seq<'_>) -> bool {
	let whole = match table {
		Seq::Words(_) => true,
		Seq::Bytes(b) => b.len() % 4 == 0,
	};
	if !whole || table.len() == 0 || table.get(0) >> 8 != 0 {
		return false;
	}
	(1..table.len()).all(|i| table.get(i - 1) >> 8 < table.get(i) >> 8)
		&& table.get(table.len() - 1) >> 8 <= LAST
}

impl Tables<'_> {
	/// The cluster class of `cp`; [`cluster::Class::Other`] beyond U+10FFFF.
	pub fn cluster_class(&self, cp: u32) -> cluster::Class {
		cluster::class_in(self.cluster, cp)
	}

	/// The line-break class of `cp`.
	pub fn brk(&self, cp: u32) -> breaks::Brk {
		breaks::brk_in(self.breaks, cp)
	}

	/// The word class of `cp`.
	pub fn word_class(&self, cp: u32) -> word::Class {
		word::class_in(self.word, cp)
	}

	/// The simple case folding of `cp`, or `cp` itself.
	pub fn fold(&self, cp: u32) -> u32 {
		fold::fold_in(self.fold, cp)
	}

	/// The Windows-1252 letter that `base` and `mark` compose to.
	pub fn compose(&self, base: u32, mark: u32) -> Option<char> {
		compose::compose_in(self.compose, base, mark)
	}

	/// The base and the mark of a Windows-1252 letter.
	pub fn decompose(&self, letter: u32) -> Option<(char, char)> {
		compose::decompose_in(self.compose, letter)
	}

	/// Whether `cp` is East Asian Wide or Fullwidth.
	pub fn is_wide(&self, cp: u32) -> bool {
		width::is_wide_in(self.width, cp)
	}

	/// Whether `cp` is in a script drawn as a placeholder in v1.
	pub fn needs_shaping_or_bidi(&self, cp: u32) -> bool {
		script::needs_shaping_or_bidi_in(self.script, cp)
	}

	/// Whether `cp` is a format character that draws nothing.
	pub fn is_invisible(&self, cp: u32) -> bool {
		script::is_invisible_in(self.script, cp)
	}

	/// The block of `cp`.
	pub fn block(&self, cp: u32) -> blocks::Block {
		blocks::block_in(self.blocks, cp)
	}

	/// The label of a block, `"Other"` for [`blocks::Block::OTHER`].
	pub fn block_name(&self, block: blocks::Block) -> &str {
		blocks::name_in(self.names, block)
	}

	/// The short name of an invisible character, such as `ZWJ`.
	pub fn short_name(&self, cp: u32) -> Option<blocks::ShortName> {
		blocks::short_name_in(self.short, cp)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	/// The size of each table in bytes, and the most each may grow to: a larger table is a decision (CORE-218), not an accident.
	#[test]
	fn tables_stay_within_their_budgets() {
		let sizes = [
			("cluster", 4 * cluster::RANGES.len(), 3_500),
			("breaks", 4 * breaks::RANGES.len(), 3_400),
			("word", 4 * word::RANGES.len(), 2_500),
			("fold", 4 * fold::RUNS.len(), 1_600),
			("compose", 4 * compose::LETTERS.len(), 240),
			("width", 4 * width::RANGES.len(), 1_000),
			("script", 4 * script::RANGES.len(), 200),
			("blocks", 4 * blocks::RANGES.len(), 420),
			("block names", blocks::NAMES.len(), 1_050),
			("short names", 4 * blocks::SHORT.len(), 100),
		];
		let mut total = 0;
		for (name, size, budget) in sizes {
			std::eprintln!("{name:12} {size:5} bytes (budget {budget})");
			assert!(
				size <= budget,
				"{name} is {size} bytes, over its budget of {budget}"
			);
			total += size;
		}
		std::eprintln!("{:12} {total:5} bytes in all", "tables");
	}

	#[test]
	fn the_built_in_tables_are_valid_range_tables() {
		for table in [
			cluster::RANGES.as_slice(),
			breaks::RANGES.as_slice(),
			word::RANGES.as_slice(),
			width::RANGES.as_slice(),
			script::RANGES.as_slice(),
			blocks::RANGES.as_slice(),
		] {
			assert!(valid_ranges(Seq::Words(table)));
		}
		assert!(fold::valid(Seq::Words(&fold::RUNS)));
		assert!(compose::valid(Seq::Words(&compose::LETTERS)));
		assert!(blocks::valid(blocks::NAMES, Seq::Words(&blocks::SHORT)));
	}
}
