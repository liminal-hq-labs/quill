// The zoom box at the right end of a graphical shell's status bar: its text, the levels it offers and the command each sets.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A shell with a status bar shows the view's zoom in a box at its right end (`100%`, or `Page Width`), and clicking the box lists
//! [`ZOOM_LEVELS`] with the current one ticked. Choosing a level runs the command [`zoom_command`] names, the same one View > Zoom
//! runs, so the box adds no state: the zoom is `App::zoom`. Nothing here knows about windows or pixels.

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

	#[test]
	fn a_level_the_list_lacks_has_no_tick_but_still_has_text() {
		assert_eq!(zoom_level_index(90), None);
		assert_eq!(shown(90), "90%");
		assert_eq!(zoom_level_index(100), Some(4));
	}
}
