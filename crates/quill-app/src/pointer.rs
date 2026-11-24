// Pointing at the page: from a point to a character position and back, and the selections a click, a double click and a drag make.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A shell that draws the page with a mouse works in the layout's own device units. The page's pages are stacked top to bottom
//! with `stride` units from one paper's top to the next, so the point `(x, y)` is measured from the top left corner of the first
//! page's paper. Nothing here knows about windows or zoom: the shell turns pixels into these units and back.

use quill_core::layout::{to_units, Line, Measure};
use quill_core::store::Store;

use crate::{App, Dir};

/// How a drag grows the selection from where the button went down.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Grain {
	/// By character: a click.
	Char,
	/// By word: a double click.
	Word,
	/// By line: a click in the left margin.
	Line,
	/// By paragraph: a triple click, or a double click in the left margin.
	Paragraph,
}

impl<M: Measure + Clone> App<M> {
	/// The top of `line` in the stacked pages, in layout units.
	fn stacked_top(&self, l: &Line, stride: i32) -> i32 {
		let (_, ry) = self.measure.resolution();
		(l.page as i32 - 1) * stride + to_units(self.doc.page.margin_t, ry) + l.y
	}

	/// The character position nearest the point (`x`, `y`), measured from the first page's paper corner with the pages `stride`
	/// units apart. A point above, below or beside the text, or in the gap between pages, gives the nearest position. Never fails: an
	/// empty document gives 0.
	pub fn pos_at_point<S: Store + ?Sized>(&self, s: &S, x: i32, y: i32, stride: i32) -> u32 {
		let (rx, _) = self.measure.resolution();
		let want = x - to_units(self.doc.page.margin_l, rx);
		// The line whose rows are nearest `y`; the first of equals, so a point on the border of two lines belongs to the lower one.
		let mut best: Option<(Line, i32)> = None;
		let mut soft = false;
		let mut prev_is_best = false;
		self.lines(s, |_, l| {
			if prev_is_best {
				if let Some((p, _)) = best {
					soft = l.start == p.start + p.len;
				}
				prev_is_best = false;
			}
			let top = self.stacked_top(l, stride);
			let bottom = top + (l.ascent + l.descent).max(1);
			let d = if y < top {
				top - y
			} else if y >= bottom {
				y - bottom + 1
			} else {
				0
			};
			if best.is_none_or(|(_, bd)| d < bd) {
				best = Some((*l, d));
				prev_is_best = true;
				soft = false;
			}
		});
		let Some((line, _)) = best else {
			return 0;
		};
		let end = self.doc.body.len().saturating_sub(1);
		let pos = self.pos_at_x(s, &line, want);
		// The end of a wrapped line is the start of the next, which a click at the right of the line did not mean: stop short of it.
		let limit = if soft && line.len > 0 {
			line.start + line.len - 1
		} else {
			line.start + line.len
		};
		pos.min(limit).min(end)
	}

	/// Where position `pos` is: the stacked top of its line, the line's height, and the x of the position from the paper's left edge.
	pub fn point_of<S: Store + ?Sized>(
		&self,
		s: &S,
		pos: u32,
		stride: i32,
	) -> Option<(i32, i32, i32)> {
		let (rx, _) = self.measure.resolution();
		let (_, l) = self.line_for(s, pos)?;
		let x = self.x_of(s, &l, pos) + to_units(self.doc.page.margin_l, rx);
		Some((
			x,
			self.stacked_top(&l, stride),
			(l.ascent + l.descent).max(1),
		))
	}

	/// Puts the caret at `pos`. With `extend` the selection grows from where it started (the caret's old place when there was none);
	/// without, any selection is dropped.
	pub fn place_caret<S: Store + ?Sized>(&mut self, s: &S, pos: u32, extend: bool) {
		self.end_typing();
		self.typing_fmt = None;
		let pos = pos.min(self.doc.body.len().saturating_sub(1));
		if extend {
			if self.anchor.is_none() {
				self.anchor = Some(self.caret);
			}
		} else {
			self.anchor = None;
		}
		self.caret = pos;
		self.want_x = -1;
		self.scroll(s);
	}

	/// Selects `lo` to `hi`, with the caret at `hi`; `lo` may be above `hi`.
	pub fn select_range<S: Store + ?Sized>(&mut self, s: &S, lo: u32, hi: u32) {
		self.end_typing();
		self.typing_fmt = None;
		let end = self.doc.body.len().saturating_sub(1);
		self.anchor = Some(lo.min(end));
		self.caret = hi.min(end);
		self.want_x = -1;
		self.scroll(s);
	}

	/// The word at `pos` and the spaces that follow it, as a double click selects them; on a run of spaces, that run. Punctuation
	/// stays with the word it touches, as in Write. The paragraph mark is never part of it.
	pub fn word_range<S: Store + ?Sized>(&self, s: &S, pos: u32) -> (u32, u32) {
		let t = &self.doc.body;
		let end = t.len().saturating_sub(1);
		let pos = pos.min(end);
		let kind = |p: u32| match t.read_byte(s, p) {
			None | Some(0x0D) => 2u8,
			Some(b' ' | 0x09) => 1,
			_ => 0,
		};
		if pos >= end {
			return (end, end);
		}
		let here = kind(pos);
		let mut lo = pos;
		while lo > 0 && kind(lo - 1) == here {
			lo -= 1;
		}
		let mut hi = pos;
		while hi < end && kind(hi) == here {
			hi += 1;
		}
		if here == 0 {
			while hi < end && kind(hi) == 1 {
				hi += 1;
			}
		}
		(lo, hi)
	}

	/// The paragraph at `pos` with its paragraph mark, as a triple click selects it.
	pub fn paragraph_range<S: Store + ?Sized>(&self, s: &S, pos: u32) -> (u32, u32) {
		let lo = self.para_start(s, pos);
		let hi = self
			.para_mark(s, pos)
			.map_or(self.doc.body.len(), |m| m + 1);
		(lo, hi)
	}

	/// The line at `pos`, as a click in the left margin selects it: the line's own characters, or the whole line and the break that
	/// follows it when the line ends the paragraph.
	pub fn line_range<S: Store + ?Sized>(&self, s: &S, pos: u32) -> (u32, u32) {
		match self.line_for(s, pos) {
			Some((_, l)) => {
				let hi = l.start + l.len;
				let at_mark = self.doc.body.read_byte(s, hi) == Some(0x0D);
				(l.start, if at_mark { hi + 1 } else { hi })
			}
			None => (pos, pos),
		}
	}

	/// The range a grain of selection covers at `pos`.
	pub fn grain_range<S: Store + ?Sized>(&self, s: &S, pos: u32, grain: Grain) -> (u32, u32) {
		match grain {
			Grain::Char => (pos, pos),
			Grain::Word => self.word_range(s, pos),
			Grain::Line => self.line_range(s, pos),
			Grain::Paragraph => self.paragraph_range(s, pos),
		}
	}

	/// Grows the selection that began as `start` (the range a click selected) to take in `pos`, a grain at a time. The caret goes to
	/// the end of the range on the side of the pointer.
	pub fn drag_select<S: Store + ?Sized>(
		&mut self,
		s: &S,
		start: (u32, u32),
		pos: u32,
		grain: Grain,
	) {
		let (lo, hi) = self.grain_range(s, pos, grain);
		if pos < start.0 {
			self.select_range(s, start.1, lo.min(pos));
		} else if pos >= start.1 && grain != Grain::Char {
			self.select_range(s, start.0, hi);
		} else if grain == Grain::Char {
			self.select_range(s, start.0, pos);
		} else {
			self.select_range(s, start.0, start.1);
		}
	}

	/// Where the caret is for a status bar: the page it is on, its line on that page and its column (a character count on a device
	/// with finer units than the text screen), all counted from 1.
	pub fn status_position<S: Store + ?Sized>(&self, s: &S) -> (u32, u32, u32) {
		let Some((index, line)) = self.caret_line(s) else {
			return (1, 1, 1);
		};
		let mut ln = 0;
		self.lines(s, |i, l| {
			if l.page == line.page && i <= index {
				ln += 1;
			}
		});
		let col = if self.measure.resolution().0 > 10 {
			self.caret - line.start + 1
		} else {
			self.x_of(s, &line, self.caret) as u32 + 1
		};
		(line.page, ln, col)
	}

	/// How many lines a screenful holds, for the keys that page.
	pub fn page_rows(&self) -> usize {
		self.rows
	}

	/// Moves the caret as a page key does, for a shell that scrolls by pixels and wants to know how far the caret went.
	pub fn move_page<S: Store + ?Sized>(&mut self, s: &S, down: bool, extend: bool) {
		self.end_typing();
		self.typing_fmt = None;
		if extend {
			if self.anchor.is_none() {
				self.anchor = Some(self.caret);
			}
		} else {
			self.anchor = None;
		}
		self.move_caret(s, if down { Dir::PageDown } else { Dir::PageUp });
	}
}
