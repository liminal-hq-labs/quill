// CapturedMeasure: lays out with the font widths a real Windows screen reported, read from a table the Win32 build captured.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use quill_core::layout::{FontMetrics, FontRef, Measure, GLYPHS};

struct Entry {
	face: String,
	size: u16,
	flags: u16,
	metrics: FontMetrics,
	widths: [u16; GLYPHS],
}

/// The widths and vertical metrics of Times New Roman, Arial and Courier New at the sizes in the table, from `QUILL32.EXE /CAPTURE`.
/// A size that is not in the table uses the nearest one; a face that is not uses Times New Roman.
pub struct CapturedMeasure {
	entries: Vec<Entry>,
}

impl CapturedMeasure {
	/// Parses a captured table (`tests/metrics/*.txt`).
	pub fn parse(text: &str) -> Option<Self> {
		let mut entries = Vec::new();
		for line in text
			.lines()
			.filter(|l| !l.starts_with('#') && !l.trim().is_empty())
		{
			let mut p = line.split('|');
			let face = p.next()?.to_string();
			let size = p.next()?.parse().ok()?;
			let flags = p.next()?.parse().ok()?;
			let (asc, desc, lead) = (
				p.next()?.parse().ok()?,
				p.next()?.parse().ok()?,
				p.next()?.parse().ok()?,
			);
			let mut widths = [0u16; GLYPHS];
			for (w, v) in widths.iter_mut().zip(p.next()?.split(',')) {
				*w = v.trim().parse().ok()?;
			}
			entries.push(Entry {
				face,
				size,
				flags,
				metrics: FontMetrics {
					ascent: asc,
					descent: desc,
					leading: lead,
				},
				widths,
			});
		}
		(!entries.is_empty()).then_some(CapturedMeasure { entries })
	}

	fn pick(&self, f: &FontRef) -> Option<&Entry> {
		let face = String::from_utf8_lossy(f.face.get(..usize::from(f.face_len)).unwrap_or(&[]))
			.to_string();
		let of = |name: &str| {
			self.entries
				.iter()
				.filter(|e| e.face.eq_ignore_ascii_case(name) && e.flags == f.flags)
				.min_by_key(|e| e.size.abs_diff(f.size))
		};
		of(&face).or_else(|| of("Times New Roman"))
	}
}

impl Measure for CapturedMeasure {
	fn resolution(&self) -> (u16, u16) {
		(96, 96)
	}

	fn metrics(&mut self, font: &FontRef) -> FontMetrics {
		self.pick(font)
			.map_or(FontMetrics::default(), |e| e.metrics)
	}

	fn widths(&mut self, font: &FontRef, out: &mut [u16; GLYPHS]) {
		match self.pick(font) {
			Some(e) => *out = e.widths,
			None => out.fill(0),
		}
	}
}
