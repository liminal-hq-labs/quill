// Throwaway prototype for spike J0: a resumable layout (`layout_from`) and a paragraph checkpoint cache. Not part of the product build:
// it exists only under the `reflow-spike` feature, and `docs/reflow-spike.md` reports what it measured. J3 and J4 write the real ones.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![allow(
	clippy::indexing_slicing,
	clippy::unwrap_used,
	clippy::expect_used,
	clippy::panic,
	clippy::too_many_arguments
)]

extern crate alloc;
use alloc::vec::Vec;

use super::{
	field_digits, font_ref, para_end, picture_size, segment, tab_advance, to_units, End, FontCache,
	FontRef, Line, LineBreak, Measure, Stops,
};
use crate::doc::{Doc, PFmt, StoryKind};
use crate::store::Store;
use crate::text::Piece;

/// A checkpoint: the layout state at a paragraph boundary. 16 bytes.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Cp {
	/// Position of the paragraph's first character.
	pub pos: u32,
	/// Paragraph number, from 0.
	pub para: u32,
	/// Page the paragraph starts on (or follows on from), from 1.
	pub page: u32,
	/// The y after the previous paragraph, from the top of the text area.
	pub y: i32,
}

impl Cp {
	/// The state at the start of a story.
	pub const START: Cp = Cp {
		pos: 0,
		para: 0,
		page: 1,
		y: 0,
	};
}

/// Lays out one story from checkpoint `start`, calling `f` for each line and `after_para` with the state after each paragraph;
/// `after_para` returns false to stop. Returns the page the layout ended on. From `Cp::START` with `after_para` always true this
/// is `layout_story` (the spike proves it is, line for line).
pub fn layout_from<S, M, F, G>(
	doc: &Doc,
	s: &S,
	m: &mut M,
	kind: StoryKind,
	start: Cp,
	mut f: F,
	mut after_para: G,
) -> u32
where
	S: Store + ?Sized,
	M: Measure + ?Sized,
	F: FnMut(&Line),
	G: FnMut(&Cp) -> bool,
{
	let (rx, ry) = m.resolution();
	let text = doc.story(kind);
	let total = text.len();
	let text_w = to_units(doc.page.paper_w - doc.page.margin_l - doc.page.margin_r, rx);
	let text_h = to_units(doc.page.paper_h - doc.page.margin_t - doc.page.margin_b, ry);
	let mut cache = FontCache::new();
	let mut page = start.page;
	let mut y = start.y;
	let mut pos = start.pos;
	let mut para = start.para;
	while pos < total {
		let end = para_end(text, s, pos, total);
		let mark = end
			.checked_sub(1)
			.filter(|p| text.read_byte(s, *p) == Some(0x0D));
		let pf = mark
			.and_then(|p| text.pfmt_at(s, p))
			.and_then(|i| doc.pfmts.get(s, i))
			.unwrap_or(PFmt::DEFAULT);
		let stops = Stops::for_para(doc, s, &pf, rx);
		let left = to_units(pf.left, rx);
		let right = to_units(pf.right, rx);
		let first = to_units(pf.first, rx);
		let before = to_units(pf.before, ry);
		let after = to_units(pf.after, ry);
		let mult = i32::from(pf.spacing.min(2)) + 2;
		if pf.flags & 2 != 0 && y > 0 {
			page += 1;
			y = 0;
		}
		let mut line_start = pos;
		let mut line_no = 0u16;
		loop {
			// A list label sits in the hanging indent, so the text of the first line starts at the left indent.
			let listed = pf.flags & 5 != 0;
			let x0 = if line_no == 0 && !listed {
				(left + first).max(0)
			} else {
				left.max(0)
			};
			let avail = (text_w - right - x0).max(1);
			let mut width = 0i32;
			let mut cw = 0i32;
			let mut asc = 0i32;
			let mut desc = 0i32;
			let mut bp: Option<LineBreak> = None;
			let mut result: Option<(End, u32, i32, i32, i32)> = None; // end, line end position, width, ascent, descent
			let span = end - line_start;
			text.for_each_run(s, line_start, span, |at, piece: Piece| {
				if result.is_some() {
					return;
				}
				let (fr, vpos, _) = font_ref(doc, s, piece.cfmt);
				let slot = cache.slot(m, &fr);
				let met = cache.metrics(slot);
				let (mut a, mut d) = (met.ascent, met.descent);
				if vpos != 0 {
					let (full, _, _) = font_ref(doc, s, 0);
					let fslot = cache.slot(
						m,
						&FontRef {
							size: doc.cfmts.get(s, piece.cfmt).map_or(full.size, |c| c.size),
							..fr
						},
					);
					let shift = cache.metrics(fslot).ascent / 3;
					if vpos == 1 {
						a += shift;
					} else {
						d += shift;
					}
				}
				let mut buf = [0u8; 64];
				let mut done = 0u32;
				while done < piece.len && result.is_none() {
					let take = (piece.len - done).min(64) as usize;
					let Some(b) = buf.get_mut(..take) else {
						return;
					};
					text.read_piece(s, &piece, done, b);
					for (i, &c) in b.iter().enumerate() {
						let p = at + done + i as u32;
						if c == 0x0D {
							if p == line_start {
								// An empty paragraph takes its height from the mark's font.
								asc = asc.max(a);
								desc = desc.max(d);
							}
							result = Some((End::Mark, p, cw, asc, desc));
							return;
						}
						if c == 0x0B {
							asc = asc.max(a);
							desc = desc.max(d);
							result = Some((End::Next(p + 1), p, cw, asc, desc));
							return;
						}
						let mut pic_h = 0;
						let w = if c == 0x09 {
							let cur = x0 + width;
							let stop = stops.next(cur);
							let (seg, dec) = if stop.kind == 0 {
								(0, 0)
							} else {
								segment(doc, s, &mut cache, m, p + 1, end)
							};
							tab_advance(&stop, cur, seg, dec)
						} else if c == 0x01 {
							match picture_size(doc, s, text, p, rx, ry) {
								Some((pw, ph)) => {
									pic_h = ph;
									pw
								}
								// A field is measured as two digits, so the layout does not move from page to page.
								None => {
									cache.width(slot, b'8') * field_digits(text.object_at(s, p), rx)
								}
							}
						} else {
							cache.width(slot, c)
						};
						if width + w > avail && p > line_start && c != b' ' {
							match bp.take() {
								Some(b) => {
									result = Some((End::Next(b.pos), b.pos, b.cw, b.asc, b.desc))
								}
								None => result = Some((End::Next(p), p, cw, asc, desc)),
							}
							return;
						}
						width += w;
						asc = asc.max(a).max(pic_h);
						desc = desc.max(d);
						if c != b' ' {
							cw = width;
						}
						if c == b' ' || c == b'-' {
							bp = Some(LineBreak {
								pos: p + 1,
								cw,
								asc,
								desc,
							});
						}
					}
					done += take as u32;
				}
			});
			let (end_kind, line_end, lw, la, ld) =
				result.unwrap_or((End::Text, end, cw, asc, desc));
			let last = !matches!(end_kind, End::Next(_));
			let shift = match pf.align {
				1 => ((avail - lw) / 2).max(0),
				2 => (avail - lw).max(0),
				_ => 0,
			};
			let h = (la + ld) * mult / 2;
			let extra_before = if line_no == 0 { before } else { 0 };
			let extra_after = if last { after } else { 0 };
			if y > 0 && y + extra_before + h > text_h {
				page += 1;
				y = 0;
			}
			let top = y + if y == 0 { 0 } else { extra_before };
			f(&Line {
				para,
				line: line_no,
				start: line_start,
				len: line_end - line_start,
				x: x0 + shift,
				indent: x0,
				width: lw,
				ascent: la,
				descent: ld,
				page,
				y: top,
			});
			y = top + h + extra_after;
			match end_kind {
				End::Next(n) => {
					line_start = n;
					line_no = line_no.saturating_add(1);
				}
				End::Mark | End::Text => break,
			}
		}
		pos = end;
		para += 1;
		if !after_para(&Cp { pos, para, page, y }) {
			break;
		}
	}
	page
}

/// What one repair did.
#[derive(Default, Debug, Clone)]
pub struct Repair {
	/// Paragraphs laid out.
	pub visits: u32,
	/// Whether layout converged on an old checkpoint (false: it ran to the end of the text).
	pub converged: bool,
}

/// One edit, as the edit log would report it, in the coordinates of the new text.
#[derive(Copy, Clone, Debug)]
pub struct EditNote {
	/// The first byte that changed.
	pub at: u32,
	/// Bytes removed (from the old text).
	pub removed: u32,
	/// Bytes inserted (in the new text).
	pub inserted: u32,
	/// Layout may not converge before this position: `at + inserted` for a text edit, `at + len` for a format change.
	pub dirty_end: u32,
}

/// A paragraph checkpoint cache: `cps[i]` is the state before paragraph `i`, and the last entry is the state at the end of the text.
#[derive(Default, Clone)]
pub struct Reflow {
	/// The checkpoints.
	pub cps: Vec<Cp>,
	/// Paragraphs laid out since creation.
	pub visits: u64,
	/// Lines of the paragraphs the last repair laid out, for the caret query.
	pub recent: Vec<Line>,
}

impl Reflow {
	/// Lays the whole story out once and keeps a checkpoint per paragraph.
	pub fn build<S, M>(doc: &Doc, s: &S, m: &mut M, kind: StoryKind) -> Reflow
	where
		S: Store + ?Sized,
		M: Measure + ?Sized,
	{
		let mut r = Reflow {
			cps: alloc::vec![Cp::START],
			..Reflow::default()
		};
		let mut visits = 0u64;
		let cps = &mut r.cps;
		layout_from(
			doc,
			s,
			m,
			kind,
			Cp::START,
			|_| {},
			|c| {
				cps.push(*c);
				visits += 1;
				true
			},
		);
		r.visits = visits;
		r
	}

	/// Bytes the checkpoints take.
	pub fn bytes(&self) -> usize {
		self.cps.len() * core::mem::size_of::<Cp>()
	}

	/// The index of the paragraph holding position `pos`.
	pub fn para_of(&self, pos: u32) -> usize {
		self.cps.partition_point(|c| c.pos <= pos).saturating_sub(1)
	}

	/// Makes the cache right after the edit `e`: re-lays out from the paragraph holding the first changed byte until the state at a
	/// paragraph boundary equals an old checkpoint (shifted by the edit) past the dirty range, then shifts the rest.
	pub fn repair<S, M>(
		&mut self,
		doc: &Doc,
		s: &S,
		m: &mut M,
		kind: StoryKind,
		e: EditNote,
	) -> Repair
	where
		S: Store + ?Sized,
		M: Measure + ?Sized,
	{
		let delta = i64::from(e.inserted) - i64::from(e.removed);
		let mut k = self.para_of(e.at);
		// An edit at a boundary joins the paragraph before it when that one has no mark (the text ended without one).
		if k > 0 && self.cps[k].pos == e.at && doc.story(kind).read_byte(s, e.at - 1) != Some(0x0D)
		{
			k -= 1;
		}
		let start = self.cps[k];
		// The old boundaries that survive: after the removed range, in the old text's coordinates.
		let cut = e.at + e.removed;
		let tail: Vec<Cp> = self.cps[k + 1..]
			.iter()
			.copied()
			.filter(|c| c.pos >= cut)
			.collect();
		self.cps.truncate(k + 1);
		self.recent.clear();
		let mut out = Repair::default();
		let mut j = 0usize;
		let mut hit: Option<(usize, i64, i64)> = None;
		let cps = &mut self.cps;
		let recent = &mut self.recent;
		layout_from(
			doc,
			s,
			m,
			kind,
			start,
			|l| recent.push(*l),
			|c| {
				out.visits += 1;
				cps.push(*c);
				while j < tail.len() && i64::from(tail[j].pos) + delta < i64::from(c.pos) {
					j += 1;
				}
				if c.pos >= e.dirty_end
					&& j < tail.len()
					&& i64::from(tail[j].pos) + delta == i64::from(c.pos)
					&& tail[j].y == c.y
				{
					// Layout reads neither the page number nor the paragraph number, so a state that differs only in them
					// is the same state: the rest is the old rest, with those two shifted by a constant.
					hit = Some((
						j,
						i64::from(c.para) - i64::from(tail[j].para),
						i64::from(c.page) - i64::from(tail[j].page),
					));
					return false;
				}
				true
			},
		);
		if let Some((j, dpara, dpage)) = hit {
			out.converged = true;
			// The state at the matching boundary was just pushed; the old entries after it are the rest, shifted.
			for c in &tail[j + 1..] {
				self.cps.push(Cp {
					pos: (i64::from(c.pos) + delta) as u32,
					para: (i64::from(c.para) + dpara) as u32,
					page: (i64::from(c.page) + dpage) as u32,
					y: c.y,
				});
			}
		}
		self.visits += u64::from(out.visits);
		out
	}

	/// The line holding `pos` (the caret query): from the last repair's lines when they cover it, else one paragraph from its
	/// checkpoint. Returns the line and the paragraphs laid out for it.
	pub fn line_at<S, M>(
		&mut self,
		doc: &Doc,
		s: &S,
		m: &mut M,
		kind: StoryKind,
		pos: u32,
	) -> (Option<Line>, u32)
	where
		S: Store + ?Sized,
		M: Measure + ?Sized,
	{
		let pick = |lines: &[Line]| {
			lines
				.iter()
				.copied()
				.rev()
				.find(|l| l.start <= pos && pos <= l.start + l.len)
		};
		if let Some(l) = pick(&self.recent) {
			return (Some(l), 0);
		}
		let k = self.para_of(pos);
		let mut found = Vec::new();
		layout_from(doc, s, m, kind, self.cps[k], |l| found.push(*l), |_| false);
		self.visits += 1;
		(pick(&found), 1)
	}

	/// Lays out `paras` paragraphs from paragraph `from` (a screenful), returning the lines and the paragraphs laid out.
	pub fn window<S, M>(
		&mut self,
		doc: &Doc,
		s: &S,
		m: &mut M,
		kind: StoryKind,
		from: usize,
		paras: u32,
	) -> (Vec<Line>, u32)
	where
		S: Store + ?Sized,
		M: Measure + ?Sized,
	{
		let mut lines = Vec::new();
		let mut n = 0;
		let start = self.cps[from.min(self.cps.len() - 1)];
		layout_from(
			doc,
			s,
			m,
			kind,
			start,
			|l| lines.push(*l),
			|_| {
				n += 1;
				n < paras
			},
		);
		self.visits += u64::from(n);
		(lines, n)
	}
}
