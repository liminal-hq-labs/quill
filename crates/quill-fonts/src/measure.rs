// The measure: widths and metrics from outline fonts, in whatever resolution layout runs at.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use quill_core::layout::{FontMetrics, FontRef, Measure, GLYPHS};

use crate::cp1252;
use crate::set::{FontSet, Resolved};

/// A face without a bold of its own is widened by this fraction of an em (one over the divisor), in the advance, so the glyphs
/// drawn emboldened by the same amount still fit their widths.
pub const SYNTHETIC_BOLD_DIVISOR: u32 = 64;

/// A `Measure` over outline fonts.
///
/// At the default resolution of 1440 units to the inch a unit is a twip, so a layout in it has the same line breaks on the
/// screen, on paper and in a PDF. Another resolution (96 for a Windows-style comparison) scales the same font units.
#[derive(Clone, Debug)]
pub struct OutlineMeasure {
	fonts: FontSet,
	resolution: u16,
}

impl OutlineMeasure {
	/// A measure over `fonts` at 1440 units to the inch.
	pub fn new(fonts: FontSet) -> OutlineMeasure {
		OutlineMeasure {
			fonts,
			resolution: 1440,
		}
	}

	/// The same measure at `units` to the inch.
	pub fn with_resolution(mut self, units: u16) -> OutlineMeasure {
		self.resolution = units.max(1);
		self
	}

	/// The face a request resolves to.
	pub fn resolve(&self, font: &FontRef) -> Option<Resolved<'_>> {
		let name: String = font
			.face
			.get(..usize::from(font.face_len))
			.unwrap_or(&[])
			.iter()
			.map(|b| cp1252::to_char(*b))
			.collect();
		self.fonts
			.resolve(&name, font.family, font.flags & 1 != 0, font.flags & 2 != 0)
	}

	/// `value` font units at `size` half points, in device units, to the nearest.
	fn scale(&self, value: u32, size: u16, upem: u16) -> u32 {
		// units = value / upem em, an em is size / 2 points, a point is res / 72 units.
		let num = u64::from(value) * u64::from(size) * u64::from(self.resolution);
		let den = 144 * u64::from(upem);
		u32::try_from((num + den / 2) / den).unwrap_or(u32::MAX)
	}
}

impl Measure for OutlineMeasure {
	fn resolution(&self) -> (u16, u16) {
		(self.resolution, self.resolution)
	}

	fn metrics(&mut self, font: &FontRef) -> FontMetrics {
		let Some(r) = self.resolve(font) else {
			return FontMetrics::default();
		};
		let f = r.face;
		let (asc, desc) = f.win_extent();
		let to =
			|v: u32| i32::try_from(self.scale(v, font.size, f.units_per_em())).unwrap_or(i32::MAX);
		FontMetrics {
			ascent: to(u32::from(asc)),
			descent: to(u32::from(desc)),
			leading: to(f.external_leading()),
		}
	}

	fn widths(&mut self, font: &FontRef, out: &mut [u16; GLYPHS]) {
		let Some(r) = self.resolve(font) else {
			out.fill(0);
			return;
		};
		let f = r.face;
		let extra = if r.synthetic_bold {
			u32::from(f.units_per_em()).div_ceil(SYNTHETIC_BOLD_DIVISOR)
		} else {
			0
		};
		for (i, w) in out.iter_mut().enumerate() {
			let adv = u32::from(f.advance(0x20 + i as u8)) + extra;
			*w = u16::try_from(self.scale(adv, font.size, f.units_per_em())).unwrap_or(u16::MAX);
		}
	}
}
