// Scroll bars: the arrow buttons, the track and the thumb, where they are and how they are drawn.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! [`parts`] is pure: from the bar's rectangle and the scroll position it gives the rectangles a pointer hits ([`Part`], [`hit`]) and
//! [`pos_at`] turns a thumb's pixel position back into a scroll position for dragging. The scroll state is the app's.

use crate::ui::{bevel, outline, FACE, INK, LIGHT, SHADOW};
use crate::{Rect, Rgb, Surface};

/// What a point on a scroll bar is on.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Part {
	/// The arrow at the top or left.
	LineUp,
	/// The arrow at the bottom or right.
	LineDown,
	/// The track between the first arrow and the thumb.
	PageUp,
	/// The track between the thumb and the second arrow.
	PageDown,
	/// The thumb.
	Thumb,
}

/// The rectangles of a scroll bar.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Parts {
	/// Whether the bar runs top to bottom.
	pub vertical: bool,
	/// The arrow at the top or left.
	pub up: Rect,
	/// The arrow at the bottom or right.
	pub down: Rect,
	/// The track between the arrows.
	pub track: Rect,
	/// The thumb (the whole track when there is nothing to scroll).
	pub thumb: Rect,
	/// How far the thumb can travel, in pixels.
	pub travel: i32,
}

/// The smallest a thumb gets.
const MIN_THUMB: i32 = 16;

/// Lays out the bar `bar`: the thumb stands for `view` of `total` units scrolled `pos` units from the start (`total` is at least
/// `view`; the most `pos` can be is `total - view`).
pub fn parts(bar: Rect, vertical: bool, pos: i32, total: i32, view: i32) -> Parts {
	let side = if vertical { bar.width() } else { bar.height() };
	let len = if vertical { bar.height() } else { bar.width() };
	let at = |a: i32, b: i32| {
		if vertical {
			Rect::new(bar.left, bar.top + a, bar.right, bar.top + b)
		} else {
			Rect::new(bar.left + a, bar.top, bar.left + b, bar.bottom)
		}
	};
	let arrow = side.min(len / 2);
	let track_len = (len - 2 * arrow).max(0);
	let range = (total - view).max(0);
	let (t_len, t_at) = if range == 0 || total <= 0 {
		(track_len, 0)
	} else {
		let t = (track_len * view.max(0) / total).clamp(MIN_THUMB.min(track_len), track_len);
		let travel = track_len - t;
		(t, travel * pos.clamp(0, range) / range)
	};
	Parts {
		vertical,
		up: at(0, arrow),
		down: at(len - arrow, len),
		track: at(arrow, arrow + track_len),
		thumb: at(arrow + t_at, arrow + t_at + t_len),
		travel: track_len - t_len,
	}
}

/// What is at (`x`, `y`) on the bar.
pub fn hit(p: &Parts, x: i32, y: i32) -> Option<Part> {
	let inside = |r: Rect| x >= r.left && x < r.right && y >= r.top && y < r.bottom;
	if inside(p.up) {
		Some(Part::LineUp)
	} else if inside(p.down) {
		Some(Part::LineDown)
	} else if inside(p.thumb) {
		Some(Part::Thumb)
	} else if inside(p.track) {
		let before = if p.vertical {
			y < p.thumb.top
		} else {
			x < p.thumb.left
		};
		Some(if before { Part::PageUp } else { Part::PageDown })
	} else {
		None
	}
}

/// The scroll position (0 to `range`) that puts the thumb's start at `coord` pixels along the bar's axis (the thumb's top or left
/// edge, so a drag subtracts where in the thumb it was grabbed).
pub fn pos_at(p: &Parts, coord: i32, range: i32) -> i32 {
	if p.travel <= 0 || range <= 0 {
		return 0;
	}
	let start = if p.vertical {
		p.track.top
	} else {
		p.track.left
	};
	((coord - start).clamp(0, p.travel) * range + p.travel / 2) / p.travel
}

/// An arrow button in `r` pointing up, down, left or right (`dir` 0 to 3), greyed when not `enabled`, sunk with its arrow a pixel
/// down and right when `down` (held by the pointer, as the zoom buttons are).
pub(crate) fn arrow<Sf: Surface + ?Sized>(
	sf: &mut Sf,
	r: Rect,
	dir: u8,
	(enabled, down): (bool, bool),
) {
	let ink = if enabled { INK } else { SHADOW };
	let off = i32::from(down);
	sf.fill_rect(r, FACE);
	outline(sf, r, INK);
	bevel(
		sf,
		Rect::new(r.left + 1, r.top + 1, r.right - 1, r.bottom - 1),
		!down,
	);
	triangle(
		sf,
		((r.left + r.right) / 2 + off, (r.top + r.bottom) / 2 + off),
		dir,
		ink,
	);
}

/// A solid triangle four rows deep centred on `(cx, cy)`, pointing up, down, left or right (`dir` 0 to 3).
pub(crate) fn triangle<Sf: Surface + ?Sized>(sf: &mut Sf, (cx, cy): (i32, i32), dir: u8, ink: Rgb) {
	for i in 0..4 {
		match dir {
			0 => sf.hline(cx - i, cx + i + 1, cy - 2 + i, ink),
			1 => sf.hline(cx - 3 + i, cx + 4 - i, cy - 1 + i, ink),
			2 => sf.vline(cx - 2 + i, cy - i, cy + i + 1, ink),
			_ => sf.vline(cx - 1 + i, cy - 3 + i, cy + 4 - i, ink),
		}
	}
}

/// Draws a bar: a striped track (white and silver in a checker), the thumb raised, the arrows on their buttons. `held` is the part the
/// pointer has pressed: an arrow is drawn sunk and the thumb grabbed (sunk, on the track's silver), as the zoom buttons are.
pub fn draw<Sf: Surface + ?Sized>(sf: &mut Sf, p: &Parts, enabled: bool, held: Option<Part>) {
	sf.clip(None);
	let t = p.track;
	// The track is white and silver in lines one pixel wide along its length, which reads as the grey of Windows 3.1's dither but takes
	// a line a pixel across instead of a call for every pixel of a bar 624 pixels long.
	sf.fill_rect(t, FACE);
	if p.vertical {
		for x in (t.left..t.right).step_by(2) {
			sf.vline(x, t.top, t.bottom, Rgb::WHITE);
		}
	} else {
		for y in (t.top..t.bottom).step_by(2) {
			sf.hline(t.left, t.right, y, Rgb::WHITE);
		}
	}
	if p.vertical {
		sf.vline(t.left, t.top, t.bottom, SHADOW);
	} else {
		sf.hline(t.left, t.right, t.top, SHADOW);
	}
	let up = (enabled, held == Some(Part::LineUp));
	let down = (enabled, held == Some(Part::LineDown));
	arrow(sf, p.up, if p.vertical { 0 } else { 2 }, up);
	arrow(sf, p.down, if p.vertical { 1 } else { 3 }, down);
	if enabled && !p.thumb.is_empty() {
		let r = p.thumb;
		sf.fill_rect(r, FACE);
		outline(sf, r, INK);
		bevel(
			sf,
			Rect::new(r.left + 1, r.top + 1, r.right - 1, r.bottom - 1),
			held != Some(Part::Thumb),
		);
	}
	let _ = LIGHT;
}
