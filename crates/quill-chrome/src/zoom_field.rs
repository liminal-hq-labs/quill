// The zoom box at the right end of a graphical shell's status bar: its text, the levels it offers and the command each sets.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A shell with a status bar shows the view's zoom near its right end: a minus button, a box with the zoom in it (`100%`, or
//! `Page Width`), a plus button, a Page Width button and a small margin before the bar's edge. Clicking the box lists [`ZOOM_LEVELS`] with the current one
//! ticked; the buttons step to the next level down or up ([`zoom_step`]). Every choice runs the command [`zoom_command`] names, the
//! same one View > Zoom runs, so the control adds no state: the zoom is `App::zoom`. [`ZoomLayout`] places the three parts and
//! answers where a click landed, in whatever unit the shell measures in (pixels), so every shell shares the geometry.

use quill_app::Command;

pub use quill_app::scroll::{zoom_level_index, zoom_text, ZOOM_LEVELS, ZOOM_TEXT_MAX};

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
	/// The width of the Page Width button right of the plus button, or 0 for a control without one.
	pub reset: i32,
	/// The gap after the last button.
	pub margin: i32,
}

impl ZoomMetrics {
	/// The width of the whole control: minus, box, plus, the Page Width button and the margin.
	pub const fn width(&self) -> i32 {
		self.button * 2 + self.field + self.reset + self.margin
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
	/// The button right of the plus button that sets Page Width.
	Reset,
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
	/// The Page Width button's `left..right`, right of the plus button; empty (`left == right`) when the control has none.
	pub reset: (i32, i32),
	/// Where the margin after the last button ends: the bar's right edge.
	pub right: i32,
}

impl ZoomLayout {
	/// The layout in a status bar `bar_w` wide.
	pub const fn new(bar_w: i32, metrics: ZoomMetrics) -> Self {
		let reset_right = bar_w - metrics.margin;
		let plus_right = reset_right - metrics.reset;
		let plus_left = plus_right - metrics.button;
		let field_left = plus_left - metrics.field;
		ZoomLayout {
			minus: (field_left - metrics.button, field_left),
			field: (field_left, plus_left),
			plus: (plus_left, plus_right),
			reset: (plus_right, reset_right),
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
		} else if inside(self.reset) {
			Some(ZoomPart::Reset)
		} else {
			None
		}
	}
}

/// Whether the Page Width button has anything to do: false when the zoom setting is already Page Width (0), which is when a shell greys it.
pub const fn reset_live(zoom: u16) -> bool {
	zoom != 0
}

/// The tooltip of a part of the control, in Title Case with the key that does the same: the Page Width button's names Ctrl+0, which
/// is what every build but the DOS graphics screen answers to ([`RESET_TIP_DOS`]).
pub const fn zoom_tip(part: ZoomPart) -> &'static str {
	match part {
		ZoomPart::Reset => "Page Width (Ctrl+0)",
		ZoomPart::Minus => "Zoom Out",
		ZoomPart::Field => "Zoom (click for the list)",
		ZoomPart::Plus => "Zoom In",
	}
}

/// The Page Width button's tooltip on the DOS graphics screen, where the BIOS gives no Ctrl with a digit and Alt+0 does the work.
pub const RESET_TIP_DOS: &str = "Page Width (Alt+0)";

/// The sentence a status bar shows in its first field while the pointer rests on a part of the control.
pub const fn zoom_hint(part: ZoomPart) -> &'static str {
	match part {
		ZoomPart::Reset => "Fit the page to the width of the window",
		ZoomPart::Minus => "Show the page smaller",
		ZoomPart::Field => "Choose how large the page is shown",
		ZoomPart::Plus => "Show the page larger",
	}
}

/// The Page Width glyph's width and height: two bars with a double-headed arrow between them.
pub const RESET_GLYPH: (i32, i32) = (11, 9);

/// Calls `rect(x, y, w, h)` for each filled rectangle of the Page Width glyph, relative to its top left corner, so every shell paints
/// the same mark in its own colour: a bar at each end, a line between them and a three-pixel arrow head at each end of the line.
pub fn reset_glyph(mut rect: impl FnMut(i32, i32, i32, i32)) {
	rect(0, 0, 1, RESET_GLYPH.1);
	rect(RESET_GLYPH.0 - 1, 0, 1, RESET_GLYPH.1);
	rect(2, 4, 7, 1);
	rect(3, 3, 1, 3);
	rect(7, 3, 1, 3);
}

/// The width of the box for text `text_w` wide: 6 pixels a side, so the widest text (`Page Width`) fits every shell that measures.
pub const fn field_for(text_w: i32) -> i32 {
	text_w + 12
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
		reset: 0,
		margin: ZOOM_MARGIN,
	};

	const WITH_RESET: ZoomMetrics = ZoomMetrics {
		reset: 22,
		..METRICS
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
	fn a_layout_without_a_reset_part_has_an_empty_one() {
		let layout = ZoomLayout::new(640, METRICS);
		assert_eq!(layout.reset, (632, 632));
		assert_eq!(layout.hit(632), None);
	}

	#[test]
	fn the_reset_button_sits_right_of_the_plus_before_the_margin() {
		let layout = ZoomLayout::new(640, WITH_RESET);
		assert_eq!(layout.reset, (610, 632));
		assert_eq!(layout.plus, (592, 610));
		assert_eq!(layout.field, (530, 592));
		assert_eq!(layout.minus, (512, 530));
		assert_eq!(layout.left(), 640 - WITH_RESET.width());
		assert_eq!(layout.right - layout.reset.1, ZOOM_MARGIN);
		// Every part starts where the one before it ends.
		assert_eq!(layout.minus.1, layout.field.0);
		assert_eq!(layout.field.1, layout.plus.0);
		assert_eq!(layout.plus.1, layout.reset.0);
		let wide = ZoomLayout::new(1024, WITH_RESET);
		assert_eq!(wide.right - wide.reset.1, ZOOM_MARGIN);
	}

	#[test]
	fn a_click_lands_on_the_reset_button_by_its_edges() {
		let layout = ZoomLayout::new(640, WITH_RESET);
		assert_eq!(layout.hit(511), None);
		assert_eq!(layout.hit(512), Some(ZoomPart::Minus));
		assert_eq!(layout.hit(529), Some(ZoomPart::Minus));
		assert_eq!(layout.hit(530), Some(ZoomPart::Field));
		assert_eq!(layout.hit(591), Some(ZoomPart::Field));
		assert_eq!(layout.hit(592), Some(ZoomPart::Plus));
		assert_eq!(layout.hit(609), Some(ZoomPart::Plus));
		assert_eq!(layout.hit(610), Some(ZoomPart::Reset));
		assert_eq!(layout.hit(631), Some(ZoomPart::Reset));
		assert_eq!(layout.hit(632), None);
		assert_eq!(layout.hit(639), None);
	}

	#[test]
	fn the_reset_button_is_live_unless_the_zoom_is_page_width() {
		assert!(!reset_live(0));
		assert!(reset_live(100));
		assert!(reset_live(25));
		assert!(reset_live(200));
	}

	const PARTS: [ZoomPart; 4] = [
		ZoomPart::Minus,
		ZoomPart::Field,
		ZoomPart::Plus,
		ZoomPart::Reset,
	];

	/// Title Case for a tooltip: every word before the parenthesised note starts with a capital.
	fn title_case(text: &str) -> bool {
		let name = text.split(" (").next().unwrap_or("");
		name.split(' ')
			.all(|word| word.starts_with(|c: char| c.is_ascii_uppercase()))
	}

	#[test]
	fn every_tooltip_and_hint_is_there_and_the_tooltips_are_title_case() {
		for part in PARTS {
			let tip = zoom_tip(part);
			assert!(!tip.is_empty() && title_case(tip), "{tip}");
			let hint = zoom_hint(part);
			assert!(hint.len() > 10 && hint.starts_with(|c: char| c.is_ascii_uppercase()));
			assert!(!hint.ends_with('.'));
		}
		assert!(title_case(RESET_TIP_DOS));
	}

	#[test]
	fn the_tooltips_are_the_owners_words() {
		assert_eq!(zoom_tip(ZoomPart::Reset), "Page Width (Ctrl+0)");
		assert_eq!(RESET_TIP_DOS, "Page Width (Alt+0)");
		assert_eq!(zoom_tip(ZoomPart::Minus), "Zoom Out");
		assert_eq!(zoom_tip(ZoomPart::Field), "Zoom (click for the list)");
		assert_eq!(zoom_tip(ZoomPart::Plus), "Zoom In");
	}

	#[test]
	fn the_glyph_stays_inside_its_box_and_is_symmetric() {
		extern crate std;
		let mut on = std::collections::BTreeSet::new();
		reset_glyph(|x, y, w, h| {
			for dy in 0..h {
				for dx in 0..w {
					on.insert((x + dx, y + dy));
				}
			}
		});
		let (width, height) = RESET_GLYPH;
		assert!(on.iter().all(|&(x, y)| x < width && y < height));
		assert!(on.iter().all(|&(x, y)| on.contains(&(width - 1 - x, y))));
		// The bars run the full height, and the line joins them.
		assert!((0..height).all(|y| on.contains(&(0, y)) && on.contains(&(width - 1, y))));
		assert!((2..=8).all(|x| on.contains(&(x, 4))));
	}

	#[test]
	fn the_box_is_sized_from_its_widest_text() {
		assert_eq!(field_for(60), 72);
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
