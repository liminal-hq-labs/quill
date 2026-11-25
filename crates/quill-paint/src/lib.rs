// quill-paint: the Surface trait, the 16 and 256 colour surfaces and the page painter of the DOS graphics mode.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The painter draws the laid-out page through a [`Surface`]; the surfaces here keep their pixels in caller-owned slices in the
//! device's own format, so the host runs exactly the code DOS runs and a golden is byte-identical to a frame read back from video
//! memory. Every glyph is drawn at the `x` that `walk_chars` gives, so the drawing, the caret and the hit tests are one set of
//! integers. The design is in `docs/dos-graphics.md`. With the `pixel` feature, [`pixel`] adds a truecolour RGBA surface over the same
//! trait for the GTK, WinUI and browser clients (`docs/pixel-surface.md`).

#![no_std]
#![deny(
	clippy::unwrap_used,
	clippy::expect_used,
	clippy::panic,
	clippy::indexing_slicing,
	clippy::unreachable,
	clippy::todo
)]

#[cfg(feature = "pixel")]
extern crate alloc;

pub mod damage;
pub mod dialog;
pub mod menu;
pub mod page;
pub mod palette;
#[cfg(feature = "pixel")]
pub mod pixel;
pub mod raster;
pub mod shell;
pub mod ui;

use quill_core::layout::FontRef;

pub use quill_strike::{crc32, crc32_update};

/// A rectangle in pixels: `left` and `top` are inside it, `right` and `bottom` are not.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Rect {
	/// First column.
	pub left: i32,
	/// First row.
	pub top: i32,
	/// One past the last column.
	pub right: i32,
	/// One past the last row.
	pub bottom: i32,
}

impl Rect {
	/// A rectangle from its edges.
	pub const fn new(left: i32, top: i32, right: i32, bottom: i32) -> Self {
		Rect {
			left,
			top,
			right,
			bottom,
		}
	}

	/// The part both rectangles cover (empty when they do not meet).
	pub fn intersect(self, o: Rect) -> Rect {
		Rect {
			left: self.left.max(o.left),
			top: self.top.max(o.top),
			right: self.right.min(o.right),
			bottom: self.bottom.min(o.bottom),
		}
	}

	/// True when it covers no pixel.
	pub fn is_empty(self) -> bool {
		self.right <= self.left || self.bottom <= self.top
	}

	/// Width in pixels, 0 when empty.
	pub fn width(self) -> i32 {
		(self.right.saturating_sub(self.left)).max(0)
	}

	/// Height in pixels, 0 when empty.
	pub fn height(self) -> i32 {
		(self.bottom.saturating_sub(self.top)).max(0)
	}
}

/// A colour: red, green, blue.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
	/// Black.
	pub const BLACK: Rgb = Rgb(0, 0, 0);
	/// White.
	pub const WHITE: Rgb = Rgb(255, 255, 255);
	/// Silver, the Windows light grey.
	pub const SILVER: Rgb = Rgb(192, 192, 192);
	/// Grey, the Windows dark grey.
	pub const GREY: Rgb = Rgb(128, 128, 128);
	/// Navy, the selection.
	pub const NAVY: Rgb = Rgb(0, 0, 128);
}

/// What the painter draws through. It is integer and pixel-addressed. Every call is clipped to the surface and to the clip set
/// with [`Surface::clip`].
pub trait Surface {
	/// What [`Surface::font`] gives back: a handle the surface understands.
	type Font: Copy;

	/// Width and height in pixels.
	fn size(&self) -> (i32, i32);
	/// Limits drawing to a rectangle, or to the whole surface with `None`.
	fn clip(&mut self, r: Option<Rect>);
	/// The font the surface will draw `f` with.
	fn font(&mut self, f: &FontRef) -> Self::Font;
	/// The advance in pixels of character `ch` in a font; the integers layout used.
	fn advance(&mut self, f: Self::Font, ch: u8) -> i32;
	/// Fills a rectangle.
	fn fill_rect(&mut self, r: Rect, c: Rgb);
	/// Draws a horizontal line from `x0` up to but not including `x1`, one pixel thick, at row `y`.
	fn hline(&mut self, x0: i32, x1: i32, y: i32, c: Rgb);
	/// Draws a vertical line at column `x` from `y0` up to but not including `y1`, one pixel thick.
	fn vline(&mut self, x: i32, y0: i32, y1: i32, c: Rgb);
	/// Draws characters, each with its pen at the `x` given and the baseline at `base_y`. With `smooth` and a black `fg` the grey
	/// levels of a 2-bit font come out as grey, which assumes white paper under the text; otherwise each pixel is ink or not.
	fn glyph_run(
		&mut self,
		f: Self::Font,
		base_y: i32,
		glyphs: &[(u8, i32)],
		fg: Rgb,
		smooth: bool,
	);
	/// Draws a 4-bit bitmap, top row first, high nibble first, `(w + 1) / 2` bytes a row, nibbles mapped by `pal`.
	fn bitmap4(&mut self, x: i32, y: i32, rows: &[u8], w: i32, h: i32, pal: &[Rgb; 16]);
	/// Draws one row of a picture, red, green, blue bytes, reduced to the surface's colours with an ordered dither keyed on the
	/// pixel's position.
	fn rgb_row(&mut self, x: i32, y: i32, rgb: &[u8]);
	/// Inverts the colours of a rectangle.
	fn invert_rect(&mut self, r: Rect);
}
