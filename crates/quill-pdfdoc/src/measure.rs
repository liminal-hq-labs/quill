// Std14Measure: lays a document out with the widths of the standard 14 fonts, so a PDF needs no font files and no shell.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use quill_core::layout::{FontMetrics, FontRef, Measure, GLYPHS};

use crate::{pdf_font, std14_metrics};

/// A measure whose fonts are Helvetica, Times and Courier: the advances the PDF's own fonts have, so a page laid out with it needs
/// no adjustment when it is written. Arial, Times New Roman and Courier New have these same advances.
#[derive(Copy, Clone, Debug)]
pub struct Std14Measure {
	/// Device units to the inch.
	pub dpi: u16,
}

impl Measure for Std14Measure {
	fn resolution(&self) -> (u16, u16) {
		(self.dpi, self.dpi)
	}

	fn metrics(&mut self, font: &FontRef) -> FontMetrics {
		std14_metrics(font.size, self.dpi)
	}

	fn widths(&mut self, font: &FontRef, out: &mut [u16; GLYPHS]) {
		let face = pdf_font(font);
		for (i, w) in out.iter_mut().enumerate() {
			let thousandths = u64::from(face.width(32 + i as u8));
			let units = thousandths * u64::from(font.size) * u64::from(self.dpi);
			*w = ((units + 72_000) / 144_000) as u16;
		}
	}
}
