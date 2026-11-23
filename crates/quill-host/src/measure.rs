// The measures: how the host lays text out like each platform does.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use quill_core::layout::{FontMetrics, FontRef, Measure, GLYPHS};

/// DOS text mode: every character 1/10 inch wide and every line 1/6 inch tall (a 12-point typewriter), so a unit across
/// is one column and a unit down is one row.
#[derive(Copy, Clone, Debug, Default)]
pub struct GridMeasure;

impl Measure for GridMeasure {
	fn resolution(&self) -> (u16, u16) {
		(10, 6)
	}

	fn metrics(&mut self, _font: &FontRef) -> FontMetrics {
		FontMetrics {
			ascent: 1,
			descent: 0,
			leading: 0,
		}
	}

	fn widths(&mut self, _font: &FontRef, out: &mut [u16; GLYPHS]) {
		out.fill(1);
	}
}
