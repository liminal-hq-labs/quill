// Which rows of a shadow screen changed since the last present, found by comparing it with the copy that was presented.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The shell draws the whole page into a shadow in the device's own format and presents only the rows whose bytes differ from the
//! frame it presented last, which it keeps. A first design kept a hash of each row instead of a copy; a sum over the row's words
//! missed a caret that changed the same bits in all four planes (four equal changes add up to nothing in 32 bits), so the comparison
//! is exact. It walks whole words, so it costs about what the hash did.

use crate::raster::{PLANAR_BYTES, PLANE_BYTES};

/// Rows of the screen the shell presents.
pub const ROWS: usize = 480;
/// Bytes of one row of one plane in mode 12h.
pub const PLANAR_ROW: usize = 80;
/// Bytes of one row of the 256-colour screen.
pub const LINEAR_ROW: usize = 640;

/// A bit for each row: set means the row must be presented.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct RowSet([u64; ROWS.div_ceil(64)]);

impl RowSet {
	/// No row.
	pub const fn empty() -> Self {
		RowSet([0; ROWS.div_ceil(64)])
	}

	/// Marks a row.
	pub fn set(&mut self, y: usize) {
		if let Some(w) = self.0.get_mut(y / 64) {
			*w |= 1 << (y % 64);
		}
	}

	/// True when the row is marked.
	pub fn has(&self, y: usize) -> bool {
		self.0.get(y / 64).is_some_and(|w| w & (1 << (y % 64)) != 0)
	}

	/// How many rows are marked.
	pub fn count(&self) -> usize {
		self.0.iter().map(|w| w.count_ones() as usize).sum()
	}
}

impl RowSet {
	/// Every row.
	pub fn full() -> Self {
		let mut set = RowSet::empty();
		for y in 0..ROWS {
			set.set(y);
		}
		set
	}
}

/// True when two equal-length rows differ, by 32-bit words (a byte-wise `==` on slices is a call to `memcmp`, which the DOS runtime
/// leaves to the compiler's byte loop).
fn differs(a: &[u8], b: &[u8]) -> bool {
	let (wa, ra) = a.as_chunks::<4>();
	let (wb, rb) = b.as_chunks::<4>();
	wa.iter()
		.zip(wb)
		.any(|(x, y)| u32::from_ne_bytes(*x) != u32::from_ne_bytes(*y))
		|| ra.iter().zip(rb).any(|(x, y)| x != y)
}

/// The rows of a mode 12h shadow (four planes of 38,400 bytes) that differ between `prev` and `cur`. A short buffer marks every row.
pub fn planar(prev: &[u8], cur: &[u8]) -> RowSet {
	if prev.len() < PLANAR_BYTES || cur.len() < PLANAR_BYTES {
		return RowSet::full();
	}
	let mut set = RowSet::empty();
	for y in 0..ROWS {
		let changed = (0..4).any(|p| {
			let at = p * PLANE_BYTES + y * PLANAR_ROW;
			match (prev.get(at..at + PLANAR_ROW), cur.get(at..at + PLANAR_ROW)) {
				(Some(a), Some(b)) => differs(a, b),
				_ => true,
			}
		});
		if changed {
			set.set(y);
		}
	}
	set
}

/// The rows of a one-byte-a-pixel shadow, `pitch` bytes a row (the first 640 of each are the pixels), that differ.
pub fn linear(prev: &[u8], cur: &[u8], pitch: usize) -> RowSet {
	let mut set = RowSet::empty();
	for y in 0..ROWS {
		let at = y * pitch;
		match (prev.get(at..at + LINEAR_ROW), cur.get(at..at + LINEAR_ROW)) {
			(Some(a), Some(b)) if !differs(a, b) => {}
			_ => set.set(y),
		}
	}
	set
}
