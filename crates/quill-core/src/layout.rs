// Layout: the Measure trait, line breaking and pagination, all in integer device units.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::doc::{cflag, CFmt, Doc, Font, PFmt, StoryKind, MAX_STOPS, NO_TABS};
use crate::store::Store;
use crate::text::Piece;

/// Number of characters a width table covers: ANSI 0x20 to 0xFF.
pub const GLYPHS: usize = 224;

/// A font as layout asks for it.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct FontRef {
	/// Face name.
	pub face: [u8; 31],
	/// Length of the face name.
	pub face_len: u8,
	/// Family, as in the font table.
	pub family: u8,
	/// Size in half points (already reduced for superscript and subscript).
	pub size: u16,
	/// `cflag` bold and italic bits.
	pub flags: u16,
}

/// Vertical metrics of a font, in device units.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct FontMetrics {
	/// Above the baseline.
	pub ascent: i32,
	/// Below the baseline.
	pub descent: i32,
	/// External leading.
	pub leading: i32,
}

/// What layout needs from a device: its resolution and its fonts.
pub trait Measure {
	/// Device units per inch across and down: 300 × 300 for a laser printer, 96 × 96 for a VGA screen, 10 × 6 for a text screen.
	fn resolution(&self) -> (u16, u16);
	/// Ascent, descent and external leading of a font, in device units.
	fn metrics(&mut self, font: &FontRef) -> FontMetrics;
	/// Advance widths of ANSI characters 0x20 to 0xFF, in device units.
	fn widths(&mut self, font: &FontRef, out: &mut [u16; GLYPHS]);
}

#[derive(Copy, Clone)]
struct Entry {
	key: FontRef,
	widths: [u16; GLYPHS],
	metrics: FontMetrics,
	used: u32,
}

/// The widths and metrics of the eight fonts used most recently.
pub struct FontCache {
	ents: [Option<Entry>; 8],
	tick: u32,
}

impl Default for FontCache {
	fn default() -> Self {
		Self::new()
	}
}

impl FontCache {
	/// An empty cache.
	pub const fn new() -> Self {
		FontCache {
			ents: [None; 8],
			tick: 0,
		}
	}

	/// The slot holding `key`, asking the measure when it is not cached.
	fn slot<M: Measure + ?Sized>(&mut self, m: &mut M, key: &FontRef) -> usize {
		self.tick += 1;
		let mut oldest = (0usize, u32::MAX);
		for (i, e) in self.ents.iter_mut().enumerate() {
			match e {
				Some(e) if e.key == *key => {
					e.used = self.tick;
					return i;
				}
				Some(e) => {
					if e.used < oldest.1 {
						oldest = (i, e.used);
					}
				}
				None => {
					oldest = (i, 0);
				}
			}
		}
		let mut widths = [0u16; GLYPHS];
		m.widths(key, &mut widths);
		let metrics = m.metrics(key);
		if let Some(slot) = self.ents.get_mut(oldest.0) {
			*slot = Some(Entry {
				key: *key,
				widths,
				metrics,
				used: self.tick,
			});
		}
		oldest.0
	}

	fn width(&self, slot: usize, ch: u8) -> i32 {
		let i = usize::from(ch.saturating_sub(0x20));
		self.ents
			.get(slot)
			.and_then(|e| e.as_ref())
			.and_then(|e| e.widths.get(i))
			.map_or(0, |w| i32::from(*w))
	}

	fn metrics(&self, slot: usize) -> FontMetrics {
		self.ents
			.get(slot)
			.and_then(|e| e.as_ref())
			.map_or(FontMetrics::default(), |e| e.metrics)
	}
}

/// One tab stop in device units, measured from the left margin.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Stop {
	/// Position.
	pub x: i32,
	/// 0 left, 1 centre, 2 right, 3 decimal.
	pub kind: u8,
	/// 0 none, 1 dots, 2 hyphens, 3 underline.
	pub leader: u8,
}

/// A paragraph's tab stops and the default step that applies past the last one.
#[derive(Copy, Clone, Debug)]
pub struct Stops {
	/// Stops set.
	pub n: usize,
	/// The stops, in position order.
	pub set: [Stop; MAX_STOPS],
	/// The default step, in device units (every half inch).
	pub step: i32,
}

impl Stops {
	/// The stops of paragraph format `pf`, converted at `rx` units per inch.
	pub fn for_para<S: Store + ?Sized>(doc: &Doc, s: &S, pf: &PFmt, rx: u16) -> Stops {
		let mut st = Stops {
			n: 0,
			set: [Stop::default(); MAX_STOPS],
			step: to_units(720, rx).max(1),
		};
		if pf.tabset != NO_TABS {
			if let Some(ts) = doc.tabsets.get(s, pf.tabset) {
				for (i, t) in ts.stops.iter().take(usize::from(ts.count)).enumerate() {
					if let Some(slot) = st.set.get_mut(i) {
						*slot = Stop {
							x: to_units(t.pos, rx),
							kind: t.kind,
							leader: t.leader,
						};
						st.n = i + 1;
					}
				}
			}
		}
		st
	}

	/// The first stop after `cur` (units from the margin): a set stop, or a default left stop.
	pub fn next(&self, cur: i32) -> Stop {
		for st in self.set.iter().take(self.n) {
			if st.x > cur {
				return *st;
			}
		}
		let last = self.set.get(self.n.wrapping_sub(1)).map_or(0, |s| s.x);
		let from = cur.max(last);
		Stop {
			x: (from / self.step + 1) * self.step,
			kind: 0,
			leader: 0,
		}
	}
}

/// How far a tab at `cur` advances to reach `stop`, given the width of the text that follows it up to the next tab (`seg`) and
/// the part of it before a decimal point (`dec`). A left stop advances to the stop; the others hold the segment back so that it
/// ends, centres or lines up on the stop, and advance nothing when it is wider than the room.
pub fn tab_advance(stop: &Stop, cur: i32, seg: i32, dec: i32) -> i32 {
	match stop.kind {
		1 => (stop.x - seg / 2 - cur).max(0),
		2 => (stop.x - seg - cur).max(0),
		3 => (stop.x - dec - cur).max(0),
		_ => (stop.x - cur).max(1),
	}
}

/// One laid-out line, in device units.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Line {
	/// Paragraph number, from 0.
	pub para: u32,
	/// Line number in the paragraph, from 0.
	pub line: u16,
	/// Position of the first character.
	pub start: u32,
	/// Characters in the line, without the paragraph mark.
	pub len: u32,
	/// Where the line starts across, from the left margin (indent and alignment included).
	pub x: i32,
	/// The indent alone: tab stops count from here, whatever the alignment shift.
	pub indent: i32,
	/// Width of the content, trailing spaces excluded.
	pub width: i32,
	/// Tallest ascent.
	pub ascent: i32,
	/// Deepest descent.
	pub descent: i32,
	/// Page number from 1.
	pub page: u32,
	/// Top of the line from the top of the text area of its page.
	pub y: i32,
}

/// Twips to device units, rounding half up.
pub fn to_units(twips: i32, res: u16) -> i32 {
	let v = i64::from(twips) * i64::from(res) * 2 + 1440;
	v.div_euclid(2880) as i32
}

fn font_ref<S: Store + ?Sized>(doc: &Doc, s: &S, cfmt: u16) -> (FontRef, u8, CFmt) {
	let cf = doc
		.cfmts
		.get(s, cfmt)
		.or_else(|| doc.cfmts.get(s, 0))
		.unwrap_or(CFmt {
			font: 0,
			size: 24,
			colour: 0,
			vpos: 0,
			flags: 0,
		});
	let font = doc
		.fonts
		.get(s, cf.font)
		.unwrap_or_else(|| Font::new(b"Times New Roman", 1));
	let size = if cf.vpos == 0 {
		cf.size
	} else {
		(cf.size * 2).div_ceil(3)
	};
	let fr = FontRef {
		face: font.name,
		face_len: font.name_len,
		family: font.family,
		size,
		flags: cf.flags & (cflag::BOLD | cflag::ITALIC),
	};
	(fr, cf.vpos, cf)
}

struct LineBreak {
	pos: u32,
	cw: i32,
	asc: i32,
	desc: i32,
}

enum End {
	/// The paragraph mark: the paragraph is done.
	Mark,
	/// A forced line break (0x0B) or a wrapped line; the next line starts at the given position.
	Next(u32),
	/// The text ended without a mark.
	Text,
}

/// Lays out one story with `m`, calling `f` for each line, and returns the number of pages.
pub fn layout_story<S, M, F>(doc: &Doc, s: &S, m: &mut M, kind: StoryKind, mut f: F) -> u32
where
	S: Store + ?Sized,
	M: Measure + ?Sized,
	F: FnMut(&Line),
{
	let (rx, ry) = m.resolution();
	let text = doc.story(kind);
	let total = text.len();
	let text_w = to_units(doc.page.paper_w - doc.page.margin_l - doc.page.margin_r, rx);
	let text_h = to_units(doc.page.paper_h - doc.page.margin_t - doc.page.margin_b, ry);
	let mut cache = FontCache::new();
	let mut page = 1u32;
	let mut y = 0i32;
	let mut pos = 0u32;
	let mut para = 0u32;
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
							0
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
						asc = asc.max(a);
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
	}
	page
}

/// The position after the paragraph mark that ends the paragraph starting at `pos` (or the end of the text).
fn para_end<S: Store + ?Sized>(text: &crate::doc::StoryText, s: &S, pos: u32, total: u32) -> u32 {
	let mut at = pos;
	let mut buf = [0u8; 128];
	while at < total {
		let n = text.read(s, at, &mut buf);
		if n == 0 {
			break;
		}
		for (i, b) in buf.iter().take(n).enumerate() {
			if *b == 0x0D {
				return at + i as u32 + 1;
			}
		}
		at += n as u32;
	}
	total
}

/// The width of the text from `from` to the next tab, line break or paragraph mark (at most 200 characters), and the part of it
/// before the first decimal point (all of it when there is none).
fn segment<S, M>(
	doc: &Doc,
	s: &S,
	cache: &mut FontCache,
	m: &mut M,
	from: u32,
	end: u32,
) -> (i32, i32)
where
	S: Store + ?Sized,
	M: Measure + ?Sized,
{
	let text = &doc.body;
	let (mut total, mut dec) = (0i32, None::<i32>);
	let mut done = false;
	let span = (end.saturating_sub(from)).min(200);
	text.for_each_run(s, from, span, |_, piece| {
		if done {
			return;
		}
		let (fr, _, _) = font_ref(doc, s, piece.cfmt);
		let slot = cache.slot(m, &fr);
		let mut buf = [0u8; 64];
		let mut at = 0u32;
		while at < piece.len && !done {
			let take = (piece.len - at).min(64) as usize;
			let Some(b) = buf.get_mut(..take) else {
				return;
			};
			text.read_piece(s, &piece, at, b);
			for &c in b.iter() {
				if c == 0x09 || c == 0x0D || c == 0x0B {
					done = true;
					break;
				}
				if c == b'.' && dec.is_none() {
					dec = Some(total);
				}
				total += cache.width(slot, c);
			}
			at += take as u32;
		}
	});
	(total, dec.unwrap_or(total))
}
