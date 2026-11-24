// A corpus for the layout tests and the reflow bench: the committed test documents, generated ones, a measure whose widths vary
// and a small random number generator.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use quill_core::doc::{CFmt, Doc, PFmt};
use quill_core::layout::{FontMetrics, FontRef, Measure, GLYPHS};
use quill_formats::io::SliceSource;

use crate::HostStore;

/// A deterministic xorshift generator, so a failure is reproduced from its seed.
pub struct Rng(pub u64);

impl Rng {
	/// The next 32 bits.
	pub fn bits(&mut self) -> u32 {
		self.0 ^= self.0 << 13;
		self.0 ^= self.0 >> 7;
		self.0 ^= self.0 << 17;
		(self.0 >> 16) as u32
	}

	/// A number below `n` (0 when `n` is 0).
	pub fn below(&mut self, n: u32) -> u32 {
		if n == 0 {
			0
		} else {
			self.bits() % n
		}
	}
}

/// A screen at 96 dots per inch whose character widths depend on the character and the size, so lines break in many places.
#[derive(Clone, Copy, Debug)]
pub struct Var96;

impl Measure for Var96 {
	fn resolution(&self) -> (u16, u16) {
		(96, 96)
	}

	fn metrics(&mut self, font: &FontRef) -> FontMetrics {
		let size = i32::from(font.size);
		FontMetrics {
			ascent: size * 2 / 3,
			descent: size / 6,
			leading: 0,
		}
	}

	fn widths(&mut self, font: &FontRef, out: &mut [u16; GLYPHS]) {
		let base = font.size / 4 + u16::from(font.flags != 0);
		for (i, w) in out.iter_mut().enumerate() {
			*w = base + (i as u16 % 5);
		}
	}
}

/// The committed test documents (`tests/docs`), by name.
pub fn files() -> Vec<(&'static str, Doc, HostStore)> {
	let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/docs/");
	let mut out = Vec::new();
	for name in [
		"letter.rtf",
		"headers.rtf",
		"lists.rtf",
		"pictures.rtf",
		"print3.rtf",
		"letter.qld",
		"plain.qld",
	] {
		let bytes = std::fs::read(format!("{root}{name}")).unwrap_or_default();
		let mut s = HostStore::new();
		let mut d = Doc::new();
		let mut src = SliceSource::new(&bytes);
		let ok = if name.ends_with(".rtf") {
			quill_formats::rtf::read(&mut d, &mut s, &mut src).is_ok()
		} else {
			quill_formats::qld::read(&mut d, &mut s, &mut src).is_ok()
		};
		if ok {
			out.push((name, d, s));
		}
	}
	out
}

/// A generated document of `paras` paragraphs of 20 to 400 characters, now and then with a tab or a hard line break, in a few
/// paragraph and character formats, some centred, some with a page break before, some at 20 or 12 point.
pub fn generated(s: &mut HostStore, rng: &mut Rng, paras: u32) -> Doc {
	let mut d = Doc::new();
	let _ = d.init(s);
	let mut text = Vec::new();
	let mut spans = Vec::new();
	for _ in 0..paras {
		let start = text.len() as u32;
		let n = 20 + rng.below(380);
		for i in 0..n {
			let c = match rng.below(40) {
				0 if i > 0 => b'\t',
				1 if i > 0 => 0x0B,
				2..=8 => b' ',
				9 => b'-',
				_ => b'a' + rng.below(26) as u8,
			};
			text.push(c);
		}
		text.push(b'\r');
		spans.push((start, text.len() as u32 - start));
	}
	let _ = d.body.load(s, &text, 0, 0);
	let pfs: Vec<u16> = [
		PFmt {
			align: 1,
			after: 240,
			..PFmt::DEFAULT
		},
		PFmt {
			flags: 2,
			..PFmt::DEFAULT
		},
		PFmt {
			left: 720,
			first: -360,
			before: 120,
			..PFmt::DEFAULT
		},
		PFmt {
			align: 2,
			spacing: 1,
			right: 360,
			..PFmt::DEFAULT
		},
	]
	.into_iter()
	.filter_map(|p| d.pfmts.intern(s, p).ok())
	.collect();
	let cfs: Vec<u16> = [(40u16, 0u16), (24, 1), (16, 0)]
		.into_iter()
		.filter_map(|(size, flags)| {
			d.cfmts
				.intern(
					s,
					CFmt {
						font: 0,
						size,
						colour: 0,
						vpos: 0,
						flags,
					},
				)
				.ok()
		})
		.collect();
	for (start, len) in spans {
		match rng.below(8) {
			0 => {
				let _ = d.body.set_pfmt(s, start, len, pfs[0], 0);
			}
			1 if rng.below(7) == 0 => {
				let _ = d.body.set_pfmt(s, start, len, pfs[1], 0);
			}
			2 => {
				let _ = d.body.set_pfmt(s, start, len, pfs[2], 0);
			}
			3 => {
				let _ = d.body.set_pfmt(s, start, len, pfs[3], 0);
			}
			_ => {}
		}
		if rng.below(6) == 0 {
			let at = start + rng.below(len / 2);
			let _ = d
				.body
				.set_cfmt(s, at, rng.below(len / 2) + 1, cfs[rng.below(3) as usize], 0);
		}
	}
	d.body.forget_history(s);
	d.body.take_damage();
	d
}
