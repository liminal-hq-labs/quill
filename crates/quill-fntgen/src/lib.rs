// quill-fntgen: renders outline faces into the bitmap strikes of QUILL.FNT, on the host.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A host-only tool (it uses `std` and floats; the DOS runtime never links it). It draws each face of a [`faceset::FaceSet`] at
//! 8, 10, 12, 14, 18 and 24 points at 96 dpi with skrifa's autohinter and tiny-skia, in 1 or 2 bits per pixel, and writes the
//! result in the layout of `docs/dos-fnt-format.md`. The output depends only on the font files and the options, so building it
//! twice gives the same bytes.

pub mod faceset;
pub mod fnt;
pub mod render;
pub mod report;
pub mod sheet;
pub mod sizes;

use faceset::FaceSet;
use render::{render_strike, Opts, Strike, POINT_SIZES};

/// The strikes of a set, ordered by face, then style, then size.
pub fn strikes(set: &FaceSet, opts: Opts) -> Vec<Strike> {
	strikes_where(set, |_, _, _| Some(opts))
}

/// The strikes `pick(face, style, points)` chooses a mode for, in the same order; `None` leaves a strike out.
pub fn strikes_where(set: &FaceSet, pick: impl Fn(u8, u8, u16) -> Option<Opts>) -> Vec<Strike> {
	let mut out = Vec::new();
	for face in 0..3u8 {
		for style in 0..4u8 {
			for pt in POINT_SIZES {
				if let Some(opts) = pick(face, style, pt) {
					out.push(render_strike(
						set.fonts[usize::from(face)][usize::from(style)],
						face,
						style,
						pt * 2,
						opts,
					));
				}
			}
		}
	}
	out
}

/// The `QUILL.FNT` bytes of a set.
pub fn build(set: &FaceSet, opts: Opts) -> Vec<u8> {
	fnt::write(&strikes(set, opts))
}
