// Selecting and sizing a picture: where it is, which handle is under a point, the frame to draw, and the sizes a drag or a dialog gives.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A picture is one `0x01` object character, and it is *selected* when the selection is exactly that character. Nothing else in the
//! app holds the state, so Shift+Arrow, a click, Select All and a drag all agree. The geometry is pure and in the layout's device
//! units, measured as [`App::pos_at_point`] measures: from the first paper's top left corner with the pages `stride` units apart.
//! A shell turns pixels into these units and back, and gives the handle size and the slop in them too, so zoom changes nothing here.
//!
//! Sizing changes the record the object character names (a copy-on-write record, so undo has the old one), as one undo step.

use quill_core::layout::{picture_size, to_units, Measure};
use quill_core::store::{ReadStore, Store};

use crate::{label, App};

/// The size of a handle, in device pixels at 96 to the inch.
pub const HANDLE: i32 = 7;
/// How far outside a handle a press still takes it, in device pixels at 96 to the inch.
pub const SLOP: i32 = 2;
/// The smallest a picture is made, in twips: a tenth of an inch.
pub const MIN_SHOWN: i32 = 144;

/// A rectangle in layout units: `left` and `top` are inside it, `right` and `bottom` are not.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Frame {
	pub left: i32,
	pub top: i32,
	pub right: i32,
	pub bottom: i32,
}

impl Frame {
	/// True when the point is inside.
	pub fn contains(&self, x: i32, y: i32) -> bool {
		x >= self.left && x < self.right && y >= self.top && y < self.bottom
	}

	/// The frame grown by `n` on every side.
	pub fn grow(&self, n: i32) -> Frame {
		Frame {
			left: self.left - n,
			top: self.top - n,
			right: self.right + n,
			bottom: self.bottom + n,
		}
	}

	/// Width.
	pub fn width(&self) -> i32 {
		self.right - self.left
	}

	/// Height.
	pub fn height(&self) -> i32 {
		self.bottom - self.top
	}
}

/// One of the eight handles of a selected picture, going round from the top left.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Handle {
	NorthWest,
	North,
	NorthEast,
	East,
	SouthEast,
	South,
	SouthWest,
	West,
}

impl Handle {
	/// All eight, in the order the frame draws them.
	pub const ALL: [Handle; 8] = [
		Handle::NorthWest,
		Handle::North,
		Handle::NorthEast,
		Handle::East,
		Handle::SouthEast,
		Handle::South,
		Handle::SouthWest,
		Handle::West,
	];

	/// Which way the handle moves the width (-1 left, 1 right, 0 not at all) and the height (-1 up, 1 down, 0 not at all).
	pub fn direction(self) -> (i32, i32) {
		match self {
			Handle::NorthWest => (-1, -1),
			Handle::North => (0, -1),
			Handle::NorthEast => (1, -1),
			Handle::East => (1, 0),
			Handle::SouthEast => (1, 1),
			Handle::South => (0, 1),
			Handle::SouthWest => (-1, 1),
			Handle::West => (-1, 0),
		}
	}

	/// True for a corner, which keeps the picture's proportions unless Shift is held.
	pub fn is_corner(self) -> bool {
		let (x, y) = self.direction();
		x != 0 && y != 0
	}
}

/// The one-pixel frame around a picture's rectangle: the rectangle grown by one, drawn as four lines outside the picture's pixels.
pub fn outline(rect: &Frame) -> Frame {
	rect.grow(1)
}

/// The box of one handle of a picture whose rectangle is `rect`, `size` on a side: outside the frame, centred on a side's middle or
/// touching the frame's corner.
pub fn handle_box(rect: &Frame, h: Handle, size: i32) -> Frame {
	let o = outline(rect);
	let (dx, dy) = h.direction();
	let (left, right) = match dx {
		-1 => (o.left - size, o.left),
		1 => (o.right, o.right + size),
		_ => {
			let l = (o.left + o.right - size) / 2;
			(l, l + size)
		}
	};
	let (top, bottom) = match dy {
		-1 => (o.top - size, o.top),
		1 => (o.bottom, o.bottom + size),
		_ => {
			let t = (o.top + o.bottom - size) / 2;
			(t, t + size)
		}
	};
	Frame {
		left,
		top,
		right,
		bottom,
	}
}

/// The eight handle boxes, in [`Handle::ALL`]'s order.
pub fn handles(rect: &Frame, size: i32) -> [Frame; 8] {
	let mut out = [Frame::default(); 8];
	for (o, h) in out.iter_mut().zip(Handle::ALL) {
		*o = handle_box(rect, h, size);
	}
	out
}

/// The handle under (`x`, `y`), counting `slop` units around each box as part of it; the first of two that overlap wins.
pub fn handle_at(rect: &Frame, x: i32, y: i32, size: i32, slop: i32) -> Option<Handle> {
	Handle::ALL
		.into_iter()
		.find(|h| handle_box(rect, *h, size).grow(slop).contains(x, y))
}

/// What a resize may do, in twips: no smaller than [`MIN_SHOWN`] and no larger than the text area, or than the picture already is.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Limits {
	pub max_w: i32,
	pub max_h: i32,
}

/// The size a picture of `w0` by `h0` twips becomes when `handle` is moved `dx` right and `dy` down (twips). A corner keeps the
/// proportions unless `free` (Shift); a side changes one dimension. The result is within the limits (a picture that is already
/// larger than the text area may be made smaller but not larger), and is never smaller than [`MIN_SHOWN`].
pub fn resize(
	handle: Handle,
	w0: i32,
	h0: i32,
	dx: i32,
	dy: i32,
	free: bool,
	lim: Limits,
) -> (i32, i32) {
	let (sx, sy) = handle.direction();
	let (w0, h0) = (w0.max(1), h0.max(1));
	let max_w = lim.max_w.max(w0).max(MIN_SHOWN);
	let max_h = lim.max_h.max(h0).max(MIN_SHOWN);
	let w1 = i64::from(w0) + i64::from(sx) * i64::from(dx);
	let h1 = i64::from(h0) + i64::from(sy) * i64::from(dy);
	let clamp = |v: i64, lo: i32, hi: i32| v.clamp(i64::from(lo), i64::from(hi.max(lo))) as i32;
	if free || !handle.is_corner() {
		let w = if sx == 0 {
			w0
		} else {
			clamp(w1, MIN_SHOWN, max_w)
		};
		let h = if sy == 0 {
			h0
		} else {
			clamp(h1, MIN_SHOWN, max_h)
		};
		return (w, h);
	}
	// Proportional: the dimension that moved more leads, then the width stays inside the range both dimensions allow.
	let (w0l, h0l) = (i64::from(w0), i64::from(h0));
	let lead_w = (w1 - w0l).abs() * h0l >= (h1 - h0l).abs() * w0l;
	let want_w = if lead_w { w1 } else { h1 * w0l / h0l };
	let lo = i64::from(MIN_SHOWN).max((i64::from(MIN_SHOWN) * w0l + h0l - 1) / h0l);
	let hi = i64::from(max_w).min(i64::from(max_h) * w0l / h0l);
	let w = want_w.clamp(lo, hi.max(lo));
	let h = (w * h0l + w0l / 2) / w0l;
	(w as i32, (h as i32).max(MIN_SHOWN))
}

/// A handle held down: where it went down in the page's units, the size the picture had, and the record the drag changes (made at the
/// first change, so a press that never moves changes nothing).
#[derive(Copy, Clone, Debug)]
pub struct HandleDrag {
	handle: Handle,
	x: i32,
	y: i32,
	w: i32,
	h: i32,
	rec: Option<u16>,
}

/// What a press on the page did to pictures.
#[derive(Copy, Clone, Debug)]
pub enum PicPress {
	/// The press was not on a picture or a handle; the text takes it.
	Miss,
	/// A picture became the selection.
	Selected,
	/// A handle of the selected picture was taken.
	Handle(HandleDrag),
}

impl HandleDrag {
	/// The pointer is now at (`x`, `y`) in the page's units, with Shift held or not: the picture takes the size the handle's new place
	/// gives. The first change starts the drag's one undo step.
	pub fn moved<M: Measure + Clone, S: Store + ?Sized>(
		&mut self,
		app: &mut App<M>,
		s: &mut S,
		x: i32,
		y: i32,
		shift: bool,
	) {
		let (dx, dy) = app.units_to_twips(x - self.x, y - self.y);
		let (w, h) = resize(
			self.handle,
			self.w,
			self.h,
			dx,
			dy,
			shift,
			app.picture_limits(),
		);
		if self.rec.is_none() {
			if (w, h) == (self.w, self.h) {
				return;
			}
			self.rec = app.begin_picture_drag(s);
		}
		if let Some(rec) = self.rec {
			app.drag_picture(s, rec, w, h);
		}
	}

	/// The button is up: whatever the drag changed is one undo step.
	pub fn release<M: Measure + Clone>(self, app: &mut App<M>) {
		if self.rec.is_some() {
			app.end_picture_drag();
		}
	}
}

/// A picture found on the page.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct PicHit {
	/// Where its object character is.
	pub pos: u32,
	/// Its record.
	pub idx: u16,
	/// Where it is drawn.
	pub rect: Frame,
}

/// The values the Size Picture dialog shows.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct SizeValues {
	/// Shown width and height in twips.
	pub w: i32,
	pub h: i32,
	/// The picture's own size in pixels.
	pub px_w: u16,
	pub px_h: u16,
	/// The shown size as a percentage of the original, at 96 pixels to the inch (15 twips a pixel).
	pub pct_w: i32,
	pub pct_h: i32,
}

/// Twips from layout units on a device of `res` units to the inch.
fn twips(units: i32, res: u16) -> i32 {
	((i64::from(units) * 1440 + i64::from(res) / 2) / i64::from(res.max(1))) as i32
}

impl<M: Measure + Clone> App<M> {
	/// The picture whose object character is at `pos`, with the rectangle it is drawn in: from the line's baseline up, at the x the
	/// line puts the character. `stride` is the pages' spacing, as for [`App::pos_at_point`].
	pub fn picture_at_pos<S: ReadStore + ?Sized>(
		&self,
		s: &S,
		pos: u32,
		stride: i32,
	) -> Option<PicHit> {
		let (rx, ry) = self.measure.resolution();
		let (w, h) = picture_size(&self.doc, s, &self.doc.body, pos, rx, ry)?;
		let idx = (self.doc.body.object_at(s, pos)? & 0xFFFF) as u16;
		let (_, l) = self.line_for(s, pos)?;
		if pos < l.start || pos >= l.start + l.len.max(1) {
			return None;
		}
		let x = self.x_of(s, &l, pos) + to_units(self.doc.page.margin_l, rx);
		let top = self.stacked_top(&l, stride) + l.ascent - h;
		Some(PicHit {
			pos,
			idx,
			rect: Frame {
				left: x,
				top,
				right: x + w,
				bottom: top + h,
			},
		})
	}

	/// The picture under the point (`x`, `y`), measured as [`App::pos_at_point`] measures.
	pub fn picture_at<S: ReadStore + ?Sized>(
		&self,
		s: &S,
		x: i32,
		y: i32,
		stride: i32,
	) -> Option<PicHit> {
		if self.story_view() != 0 {
			return None;
		}
		// The nearest character boundary is the picture's start or its end, whichever half the point is in.
		let near = self.pos_at_point(s, x, y, stride);
		[near, near.wrapping_sub(1)]
			.into_iter()
			.filter_map(|p| self.picture_at_pos(s, p, stride))
			.find(|h| h.rect.contains(x, y))
	}

	/// The position and record of the picture that is selected: the selection is exactly one picture's object character.
	pub fn selected_picture<S: ReadStore + ?Sized>(&self, s: &S) -> Option<(u32, u16)> {
		let (lo, hi) = self.selection()?;
		if hi != lo + 1 || self.story_view() != 0 {
			return None;
		}
		let v = self.doc.body.object_at(s, lo)?;
		(v >> 16 == 0).then_some((lo, (v & 0xFFFF) as u16))
	}

	/// The selected picture and where it is drawn.
	pub fn selected_picture_hit<S: ReadStore + ?Sized>(
		&self,
		s: &S,
		stride: i32,
	) -> Option<PicHit> {
		let (pos, _) = self.selected_picture(s)?;
		self.picture_at_pos(s, pos, stride)
	}

	/// True when the picture whose object character is at `pos` is part of the selection, whether alone or among other text.
	pub fn picture_in_selection(&self, pos: u32) -> bool {
		self.selection()
			.is_some_and(|(lo, hi)| pos >= lo && pos < hi)
	}

	/// The handle of the selected picture under (`x`, `y`), with the picture; `size` and `slop` are in layout units.
	pub fn picture_handle_at<S: ReadStore + ?Sized>(
		&self,
		s: &S,
		x: i32,
		y: i32,
		stride: i32,
		size: i32,
		slop: i32,
	) -> Option<(Handle, PicHit)> {
		let hit = self.selected_picture_hit(s, stride)?;
		handle_at(&hit.rect, x, y, size, slop).map(|h| (h, hit))
	}

	/// A press at (`x`, `y`) in the page's units, with the pages `stride` apart and handles `size` wide with `slop` around them: a handle
	/// of the selected picture is taken, or the picture under the point is selected. With Shift the press is left to the text, so a
	/// selection can grow across a picture.
	pub fn press_picture<S: ReadStore + ?Sized>(
		&mut self,
		s: &S,
		(x, y, stride): (i32, i32, i32),
		(size, slop): (i32, i32),
		shift: bool,
	) -> PicPress {
		if shift {
			return PicPress::Miss;
		}
		if let Some((handle, _)) = self.picture_handle_at(s, x, y, stride, size, slop) {
			let (w, h) = self.picture_size_values(s).map_or((0, 0), |v| (v.w, v.h));
			return PicPress::Handle(HandleDrag {
				handle,
				x,
				y,
				w,
				h,
				rec: None,
			});
		}
		if let Some(hit) = self.picture_at(s, x, y, stride) {
			self.select_picture(s, hit.pos);
			return PicPress::Selected;
		}
		PicPress::Miss
	}

	/// Selects the picture whose object character is at `pos`.
	pub fn select_picture<S: ReadStore + ?Sized>(&mut self, s: &S, pos: u32) {
		self.select_range(s, pos, pos + 1);
	}

	/// The picture after (`forward`) or before the selection, for Tab and Shift+Tab: the nearest object character of kind 0 past the
	/// selected one, or past the caret when none is selected.
	pub fn neighbour_picture<S: ReadStore + ?Sized>(&self, s: &S, forward: bool) -> Option<u32> {
		let t = &self.doc.body;
		let len = t.len();
		let is_pic = |p: u32| t.object_at(s, p).is_some_and(|v| v >> 16 == 0);
		if forward {
			let from = self.selection().map_or(self.caret, |(_, hi)| hi).min(len);
			let mut at = from;
			let mut buf = [0u8; 64];
			while at < len {
				let n = t.read(s, at, &mut buf);
				if n == 0 {
					break;
				}
				for (i, b) in buf.iter().take(n).enumerate() {
					if *b == 1 && is_pic(at + i as u32) {
						return Some(at + i as u32);
					}
				}
				at += n as u32;
			}
			None
		} else {
			let mut p = self.selection().map_or(self.caret, |(lo, _)| lo);
			while p > 0 {
				p -= 1;
				if t.read_byte(s, p) == Some(1) && is_pic(p) {
					return Some(p);
				}
			}
			None
		}
	}

	/// The limits for sizing a picture: the text area's width and height, in twips.
	pub fn picture_limits(&self) -> Limits {
		let p = &self.doc.page;
		Limits {
			max_w: p.text_w(),
			max_h: (p.paper_h - p.margin_t - p.margin_b).max(MIN_SHOWN),
		}
	}

	/// The values for the Size Picture dialog, when a picture is selected.
	pub fn picture_size_values<S: ReadStore + ?Sized>(&self, s: &S) -> Option<SizeValues> {
		let (_, idx) = self.selected_picture(s)?;
		let p = self.doc.pictures.get(s, u32::from(idx))?;
		let pct = |shown: i32, px: u16| {
			let own = i64::from(px.max(1)) * 15;
			((i64::from(shown) * 100 + own / 2) / own) as i32
		};
		Some(SizeValues {
			w: p.shown_w,
			h: p.shown_h,
			px_w: p.w,
			px_h: p.h,
			pct_w: pct(p.shown_w, p.w),
			pct_h: pct(p.shown_h, p.h),
		})
	}

	/// The words for the status line while a picture is selected, in `out`, and how many bytes: `Picture 320 x 200 pixels, 3.33 x 2.08 in
	/// (100%)`. `times` is the byte that stands for the multiplication sign in the shell's character set (`b'x'` where there is none).
	pub fn picture_status<S: ReadStore + ?Sized>(&self, s: &S, times: u8, out: &mut [u8]) -> usize {
		let Some(v) = self.picture_size_values(s) else {
			return 0;
		};
		let mut n = 0;
		let mut put = |bytes: &[u8]| {
			for b in bytes {
				if let Some(o) = out.get_mut(n) {
					*o = *b;
					n += 1;
				}
			}
		};
		let num = |mut v: u32, put: &mut dyn FnMut(&[u8])| {
			let mut tmp = [0u8; 10];
			let mut i = tmp.len();
			loop {
				i -= 1;
				if let Some(t) = tmp.get_mut(i) {
					*t = b'0' + (v % 10) as u8;
				}
				v /= 10;
				if v == 0 {
					break;
				}
			}
			put(tmp.get(i..).unwrap_or(&[]));
		};
		let inches = |tw: i32, put: &mut dyn FnMut(&[u8])| {
			let hundredths = ((i64::from(tw) * 100 + 720) / 1440).max(0) as u32;
			num(hundredths / 100, put);
			put(b".");
			let f = hundredths % 100;
			put(&[b'0' + (f / 10) as u8, b'0' + (f % 10) as u8]);
		};
		put(b"Picture ");
		num(u32::from(v.px_w), &mut put);
		put(&[b' ', times, b' ']);
		num(u32::from(v.px_h), &mut put);
		put(b" pixels, ");
		inches(v.w, &mut put);
		put(&[b' ', times, b' ']);
		inches(v.h, &mut put);
		put(b" in (");
		num(v.pct_w.max(0) as u32, &mut put);
		put(b"%)");
		n
	}

	/// Makes the selected picture `w` by `h` twips (limited as a drag is), as one undo step: the object character is replaced by one
	/// that names a record of the new size. Returns false when nothing is selected or the size is already that.
	pub fn size_picture<S: Store + ?Sized>(&mut self, s: &mut S, w: i32, h: i32) -> bool {
		let Some((pos, idx)) = self.selected_picture(s) else {
			return false;
		};
		let lim = self.picture_limits();
		let Some(p) = self.doc.pictures.get(s, u32::from(idx)) else {
			return false;
		};
		let w = w.clamp(MIN_SHOWN, lim.max_w.max(p.shown_w).max(MIN_SHOWN));
		let h = h.clamp(MIN_SHOWN, lim.max_h.max(p.shown_h).max(MIN_SHOWN));
		if (w, h) == (p.shown_w, p.shown_h) {
			return false;
		}
		let Ok(new) = self.doc.shown_record(s, idx, w, h, false) else {
			self.msg.set("Too many picture changes to size another.");
			return false;
		};
		self.retarget_picture(s, pos, new)
	}

	/// Replaces the object character at `pos` with one naming record `new`, in one undo step; the selection stays on it.
	fn retarget_picture<S: Store + ?Sized>(&mut self, s: &mut S, pos: u32, new: u16) -> bool {
		self.end_typing();
		let cf = self.typing_cfmt_at(s, pos);
		let pfi = self
			.para_mark(s, pos)
			.and_then(|m| self.doc.body.pfmt_at(s, m))
			.unwrap_or(0);
		self.doc.body.begin_group(label::SIZE_PICTURE);
		let done = self
			.doc
			.body
			.delete(s, pos, 1, label::SIZE_PICTURE)
			.and_then(|()| {
				self.doc
					.body
					.insert_object(s, pos, u32::from(new), cf, pfi, label::SIZE_PICTURE)
			})
			.is_ok();
		self.doc.body.end_group();
		if done {
			self.spell_note_edit(s, pos, 1, 1);
			self.modified = true;
		}
		done
	}

	/// Starts a handle drag on the selected picture: a record of its own to change as the pointer moves, and an undo group the whole
	/// drag shares. Returns the record, or `None` when no picture is selected or no record can be made. [`App::drag_picture`] then
	/// changes the record, and [`App::end_picture_drag`] closes the group.
	pub fn begin_picture_drag<S: Store + ?Sized>(&mut self, s: &mut S) -> Option<u16> {
		let (pos, idx) = self.selected_picture(s)?;
		let p = self.doc.pictures.get(s, u32::from(idx))?;
		let Ok(new) = self.doc.shown_record(s, idx, p.shown_w, p.shown_h, true) else {
			self.msg.set("Too many picture changes to size another.");
			return None;
		};
		self.end_typing();
		let cf = self.typing_cfmt_at(s, pos);
		let pfi = self
			.para_mark(s, pos)
			.and_then(|m| self.doc.body.pfmt_at(s, m))
			.unwrap_or(0);
		self.doc.body.begin_group(label::SIZE_PICTURE);
		let done = self
			.doc
			.body
			.delete(s, pos, 1, label::SIZE_PICTURE)
			.and_then(|()| {
				self.doc
					.body
					.insert_object(s, pos, u32::from(new), cf, pfi, label::SIZE_PICTURE)
			})
			.is_ok();
		if !done {
			self.doc.body.end_group();
			return None;
		}
		self.spell_note_edit(s, pos, 1, 1);
		self.modified = true;
		Some(new)
	}

	/// Sets the size of the record a drag made to `w` by `h` twips.
	pub fn drag_picture<S: Store + ?Sized>(&mut self, s: &mut S, rec: u16, w: i32, h: i32) {
		let _ = self.doc.set_shown(s, rec, w, h);
	}

	/// Ends a handle drag: the drag's changes are one undo step.
	pub fn end_picture_drag(&mut self) {
		self.doc.body.end_group();
	}

	/// Layout units to twips on this device, horizontally and vertically.
	pub fn units_to_twips(&self, dx: i32, dy: i32) -> (i32, i32) {
		let (rx, ry) = self.measure.resolution();
		(twips(dx, rx), twips(dy, ry))
	}
}
