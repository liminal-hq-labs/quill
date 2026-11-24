// The equivalence proof: after every edit, the cached layout equals the full layout, line for line.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use quill_core::doc::{Doc, StoryKind};
use quill_core::layout::reflow_spike::{layout_from, Cp, EditNote, Reflow};
use quill_core::layout::{layout_story, Line, Measure};
use quill_core::store::Store;

use crate::edits::{random_edit, Mix};
use crate::rng::Rng;

/// The lines `layout_story` gives, and its page count.
pub fn full_lines<S: Store + ?Sized, M: Measure + ?Sized>(
	d: &Doc,
	s: &S,
	m: &mut M,
	kind: StoryKind,
) -> (Vec<Line>, u32) {
	let mut v = Vec::new();
	let pages = layout_story(d, s, m, kind, |l| v.push(*l));
	(v, pages)
}

/// The lines got by resuming at each checkpoint of `r` in turn and laying out one paragraph.
pub fn lines_via_checkpoints<S: Store + ?Sized, M: Measure + ?Sized>(
	d: &Doc,
	s: &S,
	m: &mut M,
	kind: StoryKind,
	r: &Reflow,
) -> Vec<Line> {
	let mut v = Vec::new();
	for c in r.cps.iter().take(r.cps.len().saturating_sub(1)) {
		layout_from(d, s, m, kind, *c, |l| v.push(*l), |_| false);
	}
	v
}

/// Checks `r` against the document: its checkpoints equal a fresh build's, and resuming at each one reproduces `layout_story`'s
/// lines, in order. Returns what differs, or `None`.
pub fn compare<S: Store + ?Sized, M: Measure + ?Sized>(
	d: &Doc,
	s: &S,
	m: &mut M,
	kind: StoryKind,
	r: &Reflow,
) -> Option<String> {
	let (full, pages) = full_lines(d, s, m, kind);
	let fresh = Reflow::build(d, s, m, kind);
	if fresh.cps != r.cps {
		let i = fresh
			.cps
			.iter()
			.zip(&r.cps)
			.position(|(a, b)| a != b)
			.unwrap_or(fresh.cps.len().min(r.cps.len()));
		return Some(format!(
			"checkpoints differ at {i} (cached {} entries, fresh {}): cached {:?}, fresh {:?}",
			r.cps.len(),
			fresh.cps.len(),
			r.cps.get(i),
			fresh.cps.get(i)
		));
	}
	let via = lines_via_checkpoints(d, s, m, kind, r);
	if via != full {
		let i = via
			.iter()
			.zip(&full)
			.position(|(a, b)| a != b)
			.unwrap_or(via.len().min(full.len()));
		return Some(format!(
			"lines differ at {i} ({} cached, {} full): cached {:?}, full {:?}",
			via.len(),
			full.len(),
			via.get(i),
			full.get(i)
		));
	}
	let end = r.cps.last().map_or(Cp::START.page, |c| c.page);
	if full.is_empty() || end == pages {
		None
	} else {
		Some(format!(
			"pages differ: checkpoints end on {end}, layout_story says {pages}"
		))
	}
}

/// What a run of edits did.
#[derive(Default, Debug, Clone)]
pub struct Stats {
	/// Edits made.
	pub edits: u32,
	/// Edits that were text edits (insert or delete).
	pub text_edits: u32,
	/// Paragraphs laid out by repairs, summed.
	pub visits: u64,
	/// The most paragraphs one repair laid out.
	pub max_visits: u32,
	/// Repairs that stopped at a converged checkpoint.
	pub converged: u32,
	/// Repairs that laid out one paragraph or two.
	pub small: u32,
	/// Paragraphs at the end.
	pub paras: usize,
	/// Visits of every edit, to take percentiles.
	pub per_edit: Vec<u32>,
}

/// Makes `edits` random edits to `d`, repairs `r` after each, and compares it with a full layout after every `verify_every`th
/// (and the last). Returns the stats, or a message that names the seed, the document and the edit.
#[allow(clippy::too_many_arguments)]
pub fn run<S: Store + ?Sized, M: Measure + ?Sized>(
	name: &str,
	seed: u64,
	d: &mut Doc,
	s: &mut S,
	m: &mut M,
	edits: u32,
	verify_every: u32,
	formats: bool,
) -> Result<Stats, String> {
	let kind = StoryKind::Body;
	let mut rng = Rng(seed);
	let target = d.body.len();
	let mut r = Reflow::build(d, s, m, kind);
	if let Some(e) = compare(d, s, m, kind, &r) {
		return Err(format!("seed {seed:#x} doc {name}: before any edit: {e}"));
	}
	let mut st = Stats::default();
	let mut done = 0;
	while done < edits {
		let before = d.body.len();
		let Some(note) = random_edit(&mut rng, d, s, Mix { formats, target }) else {
			continue;
		};
		done += 1;
		let rep = r.repair(d, s, m, kind, note);
		st.edits += 1;
		st.text_edits += u32::from(note.removed != 0 || note.inserted != 0);
		st.visits += u64::from(rep.visits);
		st.max_visits = st.max_visits.max(rep.visits);
		st.converged += u32::from(rep.converged);
		st.small += u32::from(rep.visits <= 2);
		st.per_edit.push(rep.visits);
		if done % verify_every == 0 || done == edits {
			if let Some(e) = compare(d, s, m, kind, &r) {
				return Err(format!(
					"seed {seed:#x} doc {name} edit {done} ({note:?}, length {before} to {}): {e}",
					d.body.len()
				));
			}
		}
	}
	st.paras = r.cps.len() - 1;
	Ok(st)
}

/// A note for the edit that inserts `n` bytes at `at`.
pub fn insert_note(at: u32, n: u32) -> EditNote {
	EditNote {
		at,
		removed: 0,
		inserted: n,
		dirty_end: at + n,
	}
}
