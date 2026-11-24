// The glyph source for the pixel surface: outlines and advances from the faces an `OutlineMeasure` resolves to.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use quill_core::layout::FontRef;
use quill_paint::pixel::{FaceInfo, GlyphSource, PathSink, FUNIT_SHIFT};
use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::raw::FileRef;
use skrifa::{GlyphId, MetadataProvider};

use crate::cp1252;
use crate::face::Face;
use crate::measure::{OutlineMeasure, SYNTHETIC_BOLD_DIVISOR};

/// The glyphs of the faces a measure draws with. Layout measured through the same `OutlineMeasure` (at 96 to the inch), so what is
/// drawn is the font the widths came from.
pub struct MeasureGlyphs<'a> {
	measure: &'a OutlineMeasure,
	last: Option<(FontRef, &'a Face, bool, bool)>,
}

impl<'a> MeasureGlyphs<'a> {
	/// A glyph source over `measure`'s faces.
	pub fn new(measure: &'a OutlineMeasure) -> MeasureGlyphs<'a> {
		MeasureGlyphs {
			measure,
			last: None,
		}
	}

	fn resolve(&mut self, f: &FontRef) -> Option<(&'a Face, bool, bool)> {
		if let Some((req, face, b, i)) = self.last {
			if req == *f {
				return Some((face, b, i));
			}
		}
		let r = self.measure.resolve(f)?;
		self.last = Some((*f, r.face, r.synthetic_bold, r.synthetic_italic));
		Some((r.face, r.synthetic_bold, r.synthetic_italic))
	}
}

/// Passes skrifa's drawing commands on in 1/64 font unit.
struct Pen<'s> {
	sink: &'s mut dyn PathSink,
}

fn q(v: f32) -> i32 {
	(v * (1u32 << FUNIT_SHIFT) as f32).round() as i32
}

impl OutlinePen for Pen<'_> {
	fn move_to(&mut self, x: f32, y: f32) {
		self.sink.move_to(q(x), q(y));
	}
	fn line_to(&mut self, x: f32, y: f32) {
		self.sink.line_to(q(x), q(y));
	}
	fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
		self.sink.quad_to(q(cx), q(cy), q(x), q(y));
	}
	fn curve_to(&mut self, c1x: f32, c1y: f32, c2x: f32, c2y: f32, x: f32, y: f32) {
		self.sink
			.curve_to(q(c1x), q(c1y), q(c2x), q(c2y), q(x), q(y));
	}
	fn close(&mut self) {
		self.sink.close();
	}
}

impl GlyphSource for MeasureGlyphs<'_> {
	fn face(&mut self, f: &FontRef) -> Option<FaceInfo> {
		let (face, bold, italic) = self.resolve(f)?;
		Some(FaceInfo {
			units_per_em: face.units_per_em(),
			synthetic_bold: bold,
			synthetic_italic: italic,
		})
	}

	fn advance_units(&mut self, f: &FontRef, ch: u8) -> u32 {
		let Some((face, bold, _)) = self.resolve(f) else {
			return 0;
		};
		let extra = if bold {
			u32::from(face.units_per_em()).div_ceil(SYNTHETIC_BOLD_DIVISOR)
		} else {
			0
		};
		u32::from(face.advance(ch)) + extra
	}

	fn outline(&mut self, f: &FontRef, ch: u8, sink: &mut dyn PathSink) -> bool {
		let Some((face, _, _)) = self.resolve(f) else {
			return false;
		};
		let font = match FileRef::new(face.bytes()) {
			Ok(FileRef::Font(font)) => font,
			Ok(FileRef::Collection(c)) => match c.get(face.index()) {
				Ok(font) => font,
				Err(_) => return false,
			},
			Err(_) => return false,
		};
		let Some(gid) = font.charmap().map(cp1252::to_char(ch)) else {
			return false;
		};
		if gid == GlyphId::NOTDEF {
			return false;
		}
		let Some(glyph) = font.outline_glyphs().get(gid) else {
			return false;
		};
		let mut pen = Pen { sink };
		glyph
			.draw(
				DrawSettings::unhinted(Size::unscaled(), LocationRef::default()),
				&mut pen,
			)
			.is_ok()
	}
}
