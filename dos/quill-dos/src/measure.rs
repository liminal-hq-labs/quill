// The one measure the DOS build's app is laid out with: the text grid's, or the strikes' in graphics.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `App<M>` is generic over its measure and clones it for every line it walks, so the executable carries one `App<ScreenMeasure>`
//! and one copy of the core, not two. Both variants are `Copy`, and each call goes straight to the variant's own measure, so text
//! mode lays out exactly as `App<GridMeasure>` did.

use quill_core::layout::{FontMetrics, FontRef, Measure, GLYPHS};
use quill_grid::GridMeasure;
use quill_strike::StrikeMeasure;

/// What the document is laid out for.
#[derive(Copy, Clone)]
pub enum ScreenMeasure {
	/// The 80 by 25 text screen.
	Grid(GridMeasure),
	/// The bitmap strikes of `QUILL.FNT`, at 96 dots to the inch.
	Strikes(StrikeMeasure<'static>),
}

impl Measure for ScreenMeasure {
	fn resolution(&self) -> (u16, u16) {
		match self {
			ScreenMeasure::Grid(m) => m.resolution(),
			ScreenMeasure::Strikes(m) => m.resolution(),
		}
	}

	fn metrics(&mut self, font: &FontRef) -> FontMetrics {
		match self {
			ScreenMeasure::Grid(m) => m.metrics(font),
			ScreenMeasure::Strikes(m) => m.metrics(font),
		}
	}

	fn widths(&mut self, font: &FontRef, out: &mut [u16; GLYPHS]) {
		match self {
			ScreenMeasure::Grid(m) => m.widths(font, out),
			ScreenMeasure::Strikes(m) => m.widths(font, out),
		}
	}
}
