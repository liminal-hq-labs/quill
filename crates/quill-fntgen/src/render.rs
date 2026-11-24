// Rendering one strike: a face at a pixel size, hinted by skrifa and drawn by tiny-skia into 1- or 2-bit glyph bitmaps.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{DrawSettings, Engine, HintingInstance, HintingOptions, SmoothMode, Target};
use skrifa::raw::TableProvider;
use skrifa::{FontRef, GlyphId, MetadataProvider};
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Transform};

use crate::faceset::cp1252;

/// Characters in a strike: Windows-1252 0x20 to 0xFF.
pub const GLYPHS: usize = 224;

/// The point sizes `QUILL.FNT` carries; larger ones are whole multiples of these at run time.
pub const POINT_SIZES: [u16; 6] = [8, 10, 12, 14, 18, 24];

/// The pixel size of a point size at 96 dpi, to the nearest.
pub fn pixels_for(points: u16) -> u8 {
	((u32::from(points) * 4 + 1) / 3) as u8
}

/// How a strike is drawn.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Opts {
	/// Bits per pixel: 1, or 2 for the four grey levels.
	pub bpp: u8,
	/// Run skrifa's autohinter. Off draws the plain outline at the same pixel size.
	pub hinted: bool,
}

/// A glyph's bitmap and where it sits.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Glyph {
	/// Pixels from the pen position to the left edge of the bitmap.
	pub x: i8,
	/// Rows from the baseline up to the top row of the bitmap.
	pub y: i8,
	/// Width in pixels.
	pub w: u8,
	/// Height in rows.
	pub h: u8,
	/// Rows padded to bytes, most significant bit first; `h * ceil(w * bpp / 8)` bytes.
	pub bits: Vec<u8>,
}

/// One rendered strike.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Strike {
	/// 0 serif, 1 sans, 2 mono.
	pub face: u8,
	/// Bit 0 bold, bit 1 italic.
	pub style: u8,
	/// Size in half points.
	pub half_points: u16,
	/// Em size in pixels.
	pub px: u8,
	/// Ascent in pixels.
	pub ascent: u8,
	/// Descent in pixels.
	pub descent: u8,
	/// External leading in pixels.
	pub leading: u8,
	/// Bits per pixel of the bitmaps.
	pub bpp: u8,
	/// Advance of each character in whole pixels.
	pub advances: [u8; GLYPHS],
	/// Bitmaps of 0x20 to 0xFF.
	pub glyphs: Vec<Glyph>,
}

/// The four levels of a 2-bit pixel as ink fractions in 256ths: white, silver, grey and black of the Windows palette.
pub const INK: [u32; 4] = [0, 64, 128, 256];

/// Quantises coverage (0 to 255) to a level: the nearest of the ink fractions 0, 1/4, 1/2 and 1.
fn level(alpha: u8) -> u8 {
	match u32::from(alpha) * 256 / 255 {
		0..=31 => 0,
		32..=95 => 1,
		96..=191 => 2,
		_ => 3,
	}
}

struct Outline(PathBuilder);

impl skrifa::outline::OutlinePen for Outline {
	fn move_to(&mut self, x: f32, y: f32) {
		self.0.move_to(x, -y);
	}
	fn line_to(&mut self, x: f32, y: f32) {
		self.0.line_to(x, -y);
	}
	fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
		self.0.quad_to(cx0, -cy0, x, -y);
	}
	fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
		self.0.cubic_to(cx0, -cy0, cx1, -cy1, x, -y);
	}
	fn close(&mut self) {
		self.0.close();
	}
}

fn scaled(value: u32, px: u32, upem: u32) -> u8 {
	u8::try_from((value * px + upem / 2) / upem).unwrap_or(u8::MAX)
}

/// Renders the strike of `font` (a TrueType file) at `half_points`.
///
/// # Panics
///
/// When the file is not a font; the sources are vendored and tested, so that is a build error.
pub fn render_strike(font: &[u8], face: u8, style: u8, half_points: u16, opts: Opts) -> Strike {
	let px = pixels_for(half_points / 2);
	let f = FontRef::new(font).expect("a font file");
	let outlines = f.outline_glyphs();
	let size = Size::new(f32::from(px));
	let instance = opts.hinted.then(|| {
		let target = if opts.bpp == 1 {
			Target::Mono
		} else {
			Target::Smooth {
				mode: SmoothMode::Normal,
				symmetric_rendering: true,
				preserve_linear_metrics: false,
			}
		};
		HintingInstance::new(
			&outlines,
			size,
			LocationRef::default(),
			HintingOptions {
				engine: Engine::AutoFallback,
				target,
			},
		)
		.expect("a hinting instance")
	});
	let charmap = f.charmap();
	let metrics = f.glyph_metrics(size, LocationRef::default());
	let upem = u32::from(f.head().expect("head").units_per_em());
	let os2 = f.os2().expect("OS/2");
	let hhea = f.hhea().expect("hhea");
	let win = u32::from(os2.us_win_ascent()) + u32::from(os2.us_win_descent());
	let line = (i32::from(hhea.ascender().to_i16()) - i32::from(hhea.descender().to_i16())
		+ i32::from(hhea.line_gap().to_i16()))
	.max(0) as u32;
	let mut strike = Strike {
		face,
		style,
		half_points,
		px,
		ascent: scaled(u32::from(os2.us_win_ascent()), u32::from(px), upem),
		descent: scaled(u32::from(os2.us_win_descent()), u32::from(px), upem),
		leading: scaled(line.saturating_sub(win), u32::from(px), upem),
		bpp: opts.bpp,
		advances: [0; GLYPHS],
		glyphs: Vec::with_capacity(GLYPHS),
	};
	for i in 0..GLYPHS {
		let ch = cp1252(0x20 + i as u8);
		let gid = charmap.map(ch).unwrap_or(GlyphId::NOTDEF);
		let outline = outlines.get(gid);
		let mut pen = Outline(PathBuilder::new());
		let mut hinted_advance = None;
		if let Some(o) = &outline {
			let settings = match &instance {
				Some(h) => DrawSettings::hinted(h, false),
				None => DrawSettings::unhinted(size, LocationRef::default()),
			};
			if let Ok(m) = o.draw(settings, &mut pen) {
				hinted_advance = m.advance_width;
			}
		}
		let linear = metrics.advance_width(gid).unwrap_or(0.0);
		let adv = hinted_advance.filter(|_| opts.hinted).unwrap_or(linear);
		strike.advances[i] = adv.round().clamp(0.0, 255.0) as u8;
		strike.glyphs.push(match pen.0.finish() {
			Some(path) => bitmap(&path, opts.bpp),
			None => Glyph::default(),
		});
	}
	strike
}

fn bitmap(path: &tiny_skia::Path, bpp: u8) -> Glyph {
	let b = path.bounds();
	let x0 = b.left().floor() as i32 - 1;
	let y0 = b.top().floor() as i32 - 1;
	let w = (b.right().ceil() as i32 + 1 - x0).max(1) as u32;
	let h = (b.bottom().ceil() as i32 + 1 - y0).max(1) as u32;
	let Some(mut pix) = Pixmap::new(w, h) else {
		return Glyph::default();
	};
	let mut paint = Paint::default();
	paint.set_color_rgba8(0, 0, 0, 255);
	paint.anti_alias = bpp == 2;
	pix.fill_path(
		path,
		&paint,
		FillRule::Winding,
		Transform::from_translate(-x0 as f32, -y0 as f32),
		None,
	);
	let cov = |x: u32, y: u32| -> u8 {
		let a = pix.pixel(x, y).map_or(0, |p| p.alpha());
		if bpp == 1 {
			u8::from(a >= 128)
		} else {
			level(a)
		}
	};
	// Tight box of the pixels that carry ink.
	let (mut left, mut right, mut top, mut bottom) = (w, 0, h, 0);
	for y in 0..h {
		for x in 0..w {
			if cov(x, y) != 0 {
				left = left.min(x);
				right = right.max(x + 1);
				top = top.min(y);
				bottom = bottom.max(y + 1);
			}
		}
	}
	if right == 0 {
		return Glyph::default();
	}
	let (gw, gh) = (right - left, bottom - top);
	let stride = (gw * u32::from(bpp)).div_ceil(8);
	let mut bits = vec![0u8; (stride * gh) as usize];
	for y in 0..gh {
		for x in 0..gw {
			let v = cov(left + x, top + y);
			let bit = x * u32::from(bpp);
			let at = (y * stride + bit / 8) as usize;
			bits[at] |= v << (8 - u32::from(bpp) - (bit % 8));
		}
	}
	Glyph {
		x: i8::try_from(x0 + left as i32).unwrap_or(0),
		// Pixmap rows run down from the top; the baseline is row zero of the outline's own space.
		y: i8::try_from(-(y0 + top as i32)).unwrap_or(0),
		w: u8::try_from(gw).unwrap_or(u8::MAX),
		h: u8::try_from(gh).unwrap_or(u8::MAX),
		bits,
	}
}

/// The pixel value (0 to 3 for 2 bits, 0 or 1 for 1 bit) at column `x`, row `y` of a glyph.
pub fn pixel(g: &Glyph, bpp: u8, x: u32, y: u32) -> u8 {
	let stride = (u32::from(g.w) * u32::from(bpp)).div_ceil(8);
	let bit = x * u32::from(bpp);
	let byte = g.bits[(y * stride + bit / 8) as usize];
	(byte >> (8 - u32::from(bpp) - (bit % 8))) & ((1 << bpp) - 1)
}
