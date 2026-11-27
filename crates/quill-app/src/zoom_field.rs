// The zoom box at the right end of a graphical shell's status bar: its text, the levels it offers and the command each sets.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A shell with a status bar shows the view's zoom near its right end: a minus button, a box with the zoom in it (`100%`, or
//! `Page Width`), a plus button, and a small margin before the bar's edge. Clicking the box lists [`ZOOM_LEVELS`] with the current one
//! ticked; the buttons step to the next level down or up ([`zoom_step`]). Every choice runs the command [`zoom_command`] names, the
//! same one View > Zoom runs, so the control adds no state: the zoom is `App::zoom`. [`ZoomLayout`] places the three parts and
//! answers where a click landed, in whatever unit the shell measures in (pixels), so every shell shares the geometry.

use crate::{App, Command};
use quill_core::layout::Measure;

/// The levels the box offers, top to bottom: Page Width (0), then percentages from largest to smallest. Every one is a value
/// `App::set_zoom` keeps as it is.
pub const ZOOM_LEVELS: [u16; 7] = [0, 200, 150, 125, 100, 75, 50];

/// The longest text the box or one of its entries shows (`Page Width`, `200%`), so a shell can keep a small buffer.
pub const ZOOM_TEXT_MAX: usize = 10;

/// The text for a zoom: `Page Width` for 0, otherwise the percentage and its sign. Writes into `out` and returns the length.
pub fn zoom_text(zoom: u16, out: &mut [u8; ZOOM_TEXT_MAX]) -> usize {
	if zoom == 0 {
		let words = b"Page Width";
		out.copy_from_slice(words);
		return words.len();
	}
	let mut digits = [0u8; 5];
	let mut count = 0;
	let mut rest = zoom;
	while rest > 0 {
		if let Some(slot) = digits.get_mut(count) {
			*slot = b'0' + (rest % 10) as u8;
		}
		count += 1;
		rest /= 10;
	}
	let mut len = 0;
	for k in (0..count.min(digits.len())).rev() {
		if let (Some(d), Some(o)) = (digits.get(k), out.get_mut(len)) {
			*o = *d;
			len += 1;
		}
	}
	if let Some(o) = out.get_mut(len) {
		*o = b'%';
		len += 1;
	}
	len
}

/// The position of `zoom` in [`ZOOM_LEVELS`], or `None` for a zoom the list does not hold (a `QUILL.INI` value such as 90).
pub fn zoom_level_index(zoom: u16) -> Option<usize> {
	ZOOM_LEVELS.iter().position(|&l| l == zoom)
}

/// The command that chooses level `index` of [`ZOOM_LEVELS`], or `None` past the end.
pub fn zoom_command(index: usize) -> Option<Command> {
	ZOOM_LEVELS.get(index).map(|&l| Command::Zoom(l))
}

/// The gap, in pixels, between the plus button and the right edge of the status bar: about one character of the system font.
pub const ZOOM_MARGIN: i32 = 8;

/// How wide a shell makes each part of the control, in its own pixels. `margin` is at least [`ZOOM_MARGIN`]; a shell whose bar draws
/// a size grip in that corner makes it as wide as the grip.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ZoomMetrics {
	/// The width of the minus button and of the plus button.
	pub button: i32,
	/// The width of the box between them.
	pub field: i32,
	/// The gap after the plus button.
	pub margin: i32,
}

impl ZoomMetrics {
	/// The width of the whole control: minus, box, plus and the margin.
	pub const fn width(&self) -> i32 {
		self.button * 2 + self.field + self.margin
	}
}

/// A part of the control.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ZoomPart {
	/// The button that steps to the next level down.
	Minus,
	/// The box with the zoom in it, which opens the list.
	Field,
	/// The button that steps to the next level up.
	Plus,
}

/// Where the parts sit along a status bar: each is a half-open range `left..right` of x positions, laid out from the bar's right
/// edge so the margin is the same whatever the bar's width.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ZoomLayout {
	/// The minus button's `left..right`.
	pub minus: (i32, i32),
	/// The box's `left..right`.
	pub field: (i32, i32),
	/// The plus button's `left..right`.
	pub plus: (i32, i32),
	/// Where the margin after the plus button ends: the bar's right edge.
	pub right: i32,
}

impl ZoomLayout {
	/// The layout in a status bar `bar_w` wide.
	pub const fn new(bar_w: i32, metrics: ZoomMetrics) -> Self {
		let plus_right = bar_w - metrics.margin;
		let plus_left = plus_right - metrics.button;
		let field_left = plus_left - metrics.field;
		ZoomLayout {
			minus: (field_left - metrics.button, field_left),
			field: (field_left, plus_left),
			plus: (plus_left, plus_right),
			right: bar_w,
		}
	}

	/// Where the control starts: the minus button's left edge, which the other status panes end at.
	pub const fn left(&self) -> i32 {
		self.minus.0
	}

	/// Which part, if any, holds x position `x`. The margin and everything left of the minus button hold none.
	pub fn hit(&self, x: i32) -> Option<ZoomPart> {
		let inside = |range: (i32, i32)| x >= range.0 && x < range.1;
		if inside(self.minus) {
			Some(ZoomPart::Minus)
		} else if inside(self.field) {
			Some(ZoomPart::Field)
		} else if inside(self.plus) {
			Some(ZoomPart::Plus)
		} else {
			None
		}
	}
}

/// The command a click on the minus (`up` false) or plus (`up` true) button runs, or `None` when there is no level that way, which is
/// when a shell greys the button. `shown` is the percentage the page is drawn at now, which is not the setting while it is Page
/// Width. Minus goes to the largest percentage below it and plus to the smallest above it, so Page Width, which has no step of its
/// own, is left by either button and reached only from the list.
pub fn zoom_step(shown: i32, up: bool) -> Option<Command> {
	let mut found: Option<u16> = None;
	for &level in ZOOM_LEVELS.iter().filter(|&&level| level != 0) {
		let level_pct = i32::from(level);
		let better = if up {
			level_pct > shown && found.is_none_or(|best| level < best)
		} else {
			level_pct < shown && found.is_none_or(|best| level > best)
		};
		if better {
			found = Some(level);
		}
	}
	found.map(Command::Zoom)
}

impl<M: Measure + Clone> App<M> {
	/// The box's text now: the view's zoom as `Page Width` or a percentage. Returns the length written to `out`.
	pub fn zoom_field_text(&self, out: &mut [u8; ZOOM_TEXT_MAX]) -> usize {
		zoom_text(self.zoom(), out)
	}

	/// Which entry of [`ZOOM_LEVELS`] is the view's zoom, for the tick in the list.
	pub fn zoom_field_current(&self) -> Option<usize> {
		zoom_level_index(self.zoom())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	extern crate std;
	use std::string::String;

	fn shown(zoom: u16) -> String {
		let mut out = [0u8; ZOOM_TEXT_MAX];
		let n = zoom_text(zoom, &mut out);
		String::from_utf8_lossy(out.get(..n).unwrap_or(&[])).into_owned()
	}

	#[test]
	fn the_text_is_the_percentage_or_page_width() {
		assert_eq!(shown(0), "Page Width");
		assert_eq!(shown(100), "100%");
		assert_eq!(shown(75), "75%");
		assert_eq!(shown(25), "25%");
		assert_eq!(shown(200), "200%");
	}

	#[test]
	fn every_level_fits_the_buffer_and_has_a_command() {
		for (index, &level) in ZOOM_LEVELS.iter().enumerate() {
			assert!(shown(level).len() <= ZOOM_TEXT_MAX);
			assert_eq!(zoom_level_index(level), Some(index));
			assert_eq!(zoom_command(index), Some(Command::Zoom(level)));
		}
		assert!(zoom_command(ZOOM_LEVELS.len()).is_none());
	}

	#[test]
	fn the_levels_run_from_page_width_down() {
		assert_eq!(ZOOM_LEVELS.first(), Some(&0));
		assert!(ZOOM_LEVELS
			.iter()
			.skip(1)
			.zip(ZOOM_LEVELS.iter().skip(2))
			.all(|(a, b)| a > b));
	}

	const METRICS: ZoomMetrics = ZoomMetrics {
		button: 18,
		field: 62,
		margin: ZOOM_MARGIN,
	};

	#[test]
	fn the_layout_keeps_the_margin_at_the_right_edge() {
		let layout = ZoomLayout::new(640, METRICS);
		assert_eq!(layout.plus, (614, 632));
		assert_eq!(layout.field, (552, 614));
		assert_eq!(layout.minus, (534, 552));
		assert_eq!(layout.left(), 640 - METRICS.width());
		assert_eq!(layout.right - layout.plus.1, ZOOM_MARGIN);
		// The parts follow the right edge, whatever the width.
		let wide = ZoomLayout::new(1024, METRICS);
		assert_eq!(wide.left(), 1024 - METRICS.width());
	}

	#[test]
	fn a_click_lands_on_one_part_or_none() {
		let layout = ZoomLayout::new(640, METRICS);
		assert_eq!(layout.hit(533), None);
		assert_eq!(layout.hit(534), Some(ZoomPart::Minus));
		assert_eq!(layout.hit(551), Some(ZoomPart::Minus));
		assert_eq!(layout.hit(552), Some(ZoomPart::Field));
		assert_eq!(layout.hit(613), Some(ZoomPart::Field));
		assert_eq!(layout.hit(614), Some(ZoomPart::Plus));
		assert_eq!(layout.hit(631), Some(ZoomPart::Plus));
		assert_eq!(layout.hit(632), None);
		assert_eq!(layout.hit(639), None);
	}

	#[test]
	fn the_buttons_step_through_the_levels() {
		assert_eq!(zoom_step(100, true), Some(Command::Zoom(125)));
		assert_eq!(zoom_step(100, false), Some(Command::Zoom(75)));
		assert_eq!(zoom_step(75, false), Some(Command::Zoom(50)));
		assert_eq!(zoom_step(125, true), Some(Command::Zoom(150)));
		assert_eq!(zoom_step(150, true), Some(Command::Zoom(200)));
		// An unlisted zoom, or Page Width's percentage, steps to its neighbours.
		assert_eq!(zoom_step(90, true), Some(Command::Zoom(100)));
		assert_eq!(zoom_step(90, false), Some(Command::Zoom(75)));
		assert_eq!(zoom_step(68, true), Some(Command::Zoom(75)));
		assert_eq!(zoom_step(68, false), Some(Command::Zoom(50)));
	}

	#[test]
	fn the_buttons_have_nothing_past_the_ends() {
		assert_eq!(zoom_step(200, true), None);
		assert_eq!(zoom_step(50, false), None);
		assert_eq!(zoom_step(25, false), None);
		assert_eq!(zoom_step(200, false), Some(Command::Zoom(150)));
		assert_eq!(zoom_step(50, true), Some(Command::Zoom(75)));
	}

	#[test]
	fn a_level_the_list_lacks_has_no_tick_but_still_has_text() {
		assert_eq!(zoom_level_index(90), None);
		assert_eq!(shown(90), "90%");
		assert_eq!(zoom_level_index(100), Some(4));
	}
}
