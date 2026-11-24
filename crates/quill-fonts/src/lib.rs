// quill-fonts: Quill's measure over outline fonts.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `OutlineMeasure` answers layout's questions (`Measure`) from font files with no hinting: an advance is the font's own advance
//! in font units scaled to the size, so it never depends on a pixel size, a zoom or a screen's scale. A `FontSet` holds the faces
//! and finds one for a face name and style, falling back from the name asked for to the metric-compatible fonts and finally to the
//! Quill test fonts the crate carries.

mod cp1252;
mod face;
mod glyphs;
mod measure;
mod set;

pub use face::{Face, LoadError};
pub use glyphs::MeasureGlyphs;
pub use measure::{OutlineMeasure, SYNTHETIC_BOLD_DIVISOR};
pub use set::{FontSet, Resolved};
