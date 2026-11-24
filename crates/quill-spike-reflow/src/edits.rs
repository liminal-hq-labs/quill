// Random edits to a body, and the note each leaves for the checkpoint cache.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use quill_core::doc::Doc;
use quill_core::layout::reflow_spike::EditNote;
use quill_core::store::Store;

use crate::rng::Rng;

/// What typing and editing put in a document: letters, spaces, hyphens, tabs, hard breaks and paragraph marks.
const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz ABCDEFGHIJ  --\t\x0b\r\r.,";

/// The start and end of the paragraph holding `pos` (the end is after its mark).
pub fn para_range<S: Store + ?Sized>(d: &Doc, s: &S, pos: u32) -> (u32, u32) {
	let t = &d.body;
	let len = t.len();
	let mut a = pos.min(len);
	while a > 0 && t.read_byte(s, a - 1) != Some(0x0D) {
		a -= 1;
	}
	let mut b = pos.min(len);
	while b < len {
		b += 1;
		if t.read_byte(s, b - 1) == Some(0x0D) {
			break;
		}
	}
	(a, b)
}

/// The paragraph format of the paragraph holding `pos`.
fn pfmt_of<S: Store + ?Sized>(d: &Doc, s: &S, pos: u32) -> u16 {
	let (_, b) = para_range(d, s, pos);
	if b == 0 {
		return 0;
	}
	d.body.pfmt_at(s, b - 1).unwrap_or(0)
}

/// Which kinds of edit to make.
#[derive(Copy, Clone)]
pub struct Mix {
	/// Include paragraph and character format changes.
	pub formats: bool,
	/// The length the document is kept near: below it edits favour inserting, above it deleting.
	pub target: u32,
}

/// Makes one random edit and returns its note, or `None` when the document refused it (nothing changed then).
pub fn random_edit<S: Store + ?Sized>(
	rng: &mut Rng,
	d: &mut Doc,
	s: &mut S,
	mix: Mix,
) -> Option<EditNote> {
	let len = d.body.len();
	let roll = rng.below(100);
	// Keeps the length near the target, so a run neither empties the document nor swells it.
	let ins = if len < mix.target { 70 } else { 40 };
	if roll < ins || len < 8 {
		let at = rng.below32(len + 1);
		let n = 1 + rng.below32(8) as usize;
		let bytes: Vec<u8> = (0..n)
			.map(|_| ALPHABET[rng.below(ALPHABET.len() as u64) as usize])
			.collect();
		let cfmt = if at > 0 {
			d.body.cfmt_at(s, at - 1)
		} else {
			d.body.cfmt_at(s, 0)
		}
		.unwrap_or(0);
		let pfmt = pfmt_of(d, s, at);
		d.body.insert(s, at, &bytes, cfmt, pfmt, 0).ok()?;
		Some(EditNote {
			at,
			removed: 0,
			inserted: n as u32,
			dirty_end: at + n as u32,
		})
	} else if roll < 85 || !mix.formats {
		let n = 1 + rng.below32(8).min(len - 2);
		let at = rng.below32(len - n);
		d.body.delete(s, at, n, 0).ok()?;
		Some(EditNote {
			at,
			removed: n,
			inserted: 0,
			dirty_end: at,
		})
	} else if roll < 93 {
		// A paragraph format on one whole paragraph: alignment, spacing, a page break, indents.
		let (a, b) = para_range(d, s, rng.below32(len));
		let mut pf = d
			.body
			.pfmt_at(s, b.saturating_sub(1))
			.and_then(|i| d.pfmts.get(s, i))?;
		match rng.below(5) {
			0 => pf.align = (rng.below(3)) as u8,
			1 => pf.spacing = rng.below(3) as u8,
			2 => pf.flags ^= 2,
			3 => pf.left = (rng.below(4) * 360) as i32,
			_ => pf.after = (rng.below(3) * 120) as i32,
		}
		let ix = d.pfmts.intern(s, pf).ok()?;
		d.body.set_pfmt(s, a, b - a, ix, 0).ok()?;
		Some(EditNote {
			at: a,
			removed: 0,
			inserted: 0,
			dirty_end: b,
		})
	} else {
		// A character format on a range: a different size, which changes line heights and wraps.
		let at = rng.below32(len - 1);
		let n = 1 + rng.below32(60).min(len - at - 1);
		let base = d.body.cfmt_at(s, at).and_then(|i| d.cfmts.get(s, i))?;
		let mut cf = base;
		cf.size = [16, 20, 24, 28, 36][rng.below(5) as usize];
		let ix = d.cfmts.intern(s, cf).ok()?;
		d.body.set_cfmt(s, at, n, ix, 0).ok()?;
		Some(EditNote {
			at,
			removed: 0,
			inserted: 0,
			dirty_end: at + n,
		})
	}
}
