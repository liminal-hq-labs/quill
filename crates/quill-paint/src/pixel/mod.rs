// The RGBA pixel surface: the page painter's Surface over a 32-bit buffer, at a fractional device scale, with outline text.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! One engine for every client that draws into pixels it owns: the GTK4 texture, the WinUI 3 bitmap and a browser canvas's
//! `ImageData` are all a width, a height and a byte buffer.
//!
//! **Format.** Four bytes a pixel, in memory order red, green, blue, alpha ([`ByteOrder::Rgba`], what canvas `ImageData` and
//! `gdk::MemoryFormat::R8g8b8a8Premultiplied` take) or blue, green, red, alpha ([`ByteOrder::Bgra`], what Direct2D, WriteableBitmap
//! and Cairo's `ARgb32` on a little-endian machine take). The pixels are **premultiplied**: each colour byte has been multiplied by
//! alpha. Everything the painter draws is opaque, so on the page the premultiplied and straight forms are the same bytes; the
//! choice matters only for the blend of antialiased glyph edges and for a buffer left transparent around the page, and
//! premultiplied is what every compositor those clients hand the bitmap to expects.
//!
//! **Scale.** The painter lays the page out and addresses it in logical pixels, 96 to the inch (the `OutlineMeasure` resolution is
//! set to 96). [`Scale`] maps them to device pixels. It is a fixed-point number in 1/256, so 1.25, 1.5 and 1.75 are exact and no
//! float is involved. Edges are rounded as `round(v * scale)`, which keeps rectangles that share an edge sharing it, and a one-pixel
//! line is `max(1, round(scale))` device pixels thick, the same for every line. Glyphs are drawn from their outlines at the device
//! size, with the pen at the fractional device position the logical `x` gives, so text keeps its layout at every scale and is never
//! stretched. A picture's rows, which the painter decodes at logical size, are repeated to the device size (nearest neighbour).
//!
//! **Text.** Outlines come from a [`GlyphSource`]; `quill-fonts` provides one over `OutlineMeasure`'s faces, so the widths layout
//! used and the glyphs drawn are the same font. Coverage is exact area, not sampled, and is blended onto what is already there.
//!
//! The design is in `docs/pixel-surface.md`.

mod outline;

use alloc::vec;
use alloc::vec::Vec;

use quill_core::layout::FontRef;

pub use outline::{Coverage, PathSink, Rasteriser, FUNIT_SHIFT};

use crate::{Rect, Rgb, Surface};

/// The byte order of a pixel in memory.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ByteOrder {
	/// Red, green, blue, alpha.
	Rgba,
	/// Blue, green, red, alpha.
	Bgra,
}

/// A device scale in 1/256: 256 is 1.0, 448 is 1.75. The page is drawn at a scale from 1.0 to 3.0; the limits of what is accepted
/// are wider so a browser at an odd zoom still draws.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Scale(u32);

impl Scale {
	/// 1.0.
	pub const ONE: Scale = Scale(256);
	const MIN: u32 = 64;
	const MAX: u32 = 2048;

	/// From 1/256 units, held between 0.25 and 8.0.
	pub fn from_raw(raw: u32) -> Scale {
		Scale(raw.clamp(Self::MIN, Self::MAX))
	}

	/// From percent: 100 is 1.0, 175 is 1.75. Rounded to the nearest 1/256.
	pub fn from_percent(p: u32) -> Scale {
		Scale::from_raw((p.saturating_mul(256) + 50) / 100)
	}

	/// From a ratio such as a device pixel ratio expressed as `num / den`.
	pub fn from_ratio(num: u32, den: u32) -> Scale {
		let den = u64::from(den.max(1));
		Scale::from_raw(((u64::from(num) * 256 + den / 2) / den).min(u64::from(u32::MAX)) as u32)
	}

	/// The scale in 1/256.
	pub fn raw(self) -> u32 {
		self.0
	}

	/// A logical length or position in device pixels, to the nearest.
	pub fn dev(self, v: i32) -> i32 {
		((i64::from(v) * i64::from(self.0) + 128) >> 8)
			.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
	}

	/// A device length in logical pixels, rounded down.
	pub fn logical(self, v: i32) -> i32 {
		((i64::from(v) << 8) / i64::from(self.0)).clamp(i64::from(i32::MIN), i64::from(i32::MAX))
			as i32
	}

	/// The width of a one-pixel line in device pixels.
	pub fn line(self) -> i32 {
		self.dev(1).max(1)
	}
}

/// What a face is, as the surface needs it to draw.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct FaceInfo {
	/// Font units to the em.
	pub units_per_em: u16,
	/// Bold was asked for and the face is not bold: it is widened by `units_per_em / 64`, as the measure widens its advances.
	pub synthetic_bold: bool,
	/// Italic was asked for and the face is not italic: it is leaned a quarter.
	pub synthetic_italic: bool,
}

/// Where glyph shapes and widths come from: the faces behind a layout's `Measure`.
pub trait GlyphSource {
	/// The face a font request resolves to, or `None` when there is none (nothing is drawn and the advance is 0).
	fn face(&mut self, f: &FontRef) -> Option<FaceInfo>;
	/// The advance of Windows-1252 character `ch` in font units, with the extra of a synthetic bold already added.
	fn advance_units(&mut self, f: &FontRef, ch: u8) -> u32;
	/// Sends the outline of `ch` to `sink` in 1/64 font units, y up. False when the face has no glyph for it.
	fn outline(&mut self, f: &FontRef, ch: u8, sink: &mut dyn PathSink) -> bool;
}

/// A font as the pixel surface holds it: the request and what it resolved to.
#[derive(Copy, Clone, Debug)]
pub struct PixelFont {
	req: FontRef,
	info: Option<FaceInfo>,
}

/// An owned buffer of pixels. Use [`PixelBuffer::surface`] to draw on it and [`PixelBuffer::bytes`] to hand it on.
#[derive(Clone, Debug)]
pub struct PixelBuffer {
	data: Vec<u8>,
	width: u32,
	height: u32,
}

impl PixelBuffer {
	/// A transparent buffer of `width` by `height` device pixels.
	pub fn new(width: u32, height: u32) -> PixelBuffer {
		let n = (width as usize)
			.saturating_mul(height as usize)
			.saturating_mul(4);
		PixelBuffer {
			data: vec![0; n],
			width,
			height,
		}
	}

	/// Width in device pixels.
	pub fn width(&self) -> u32 {
		self.width
	}

	/// Height in device pixels.
	pub fn height(&self) -> u32 {
		self.height
	}

	/// Bytes from one row to the next.
	pub fn stride(&self) -> usize {
		self.width as usize * 4
	}

	/// The pixels, row by row from the top.
	pub fn bytes(&self) -> &[u8] {
		&self.data
	}

	/// The pixels, to hand to a client that writes them.
	pub fn bytes_mut(&mut self) -> &mut [u8] {
		&mut self.data
	}

	/// A surface that draws on this buffer.
	pub fn surface<G: GlyphSource>(
		&mut self,
		order: ByteOrder,
		scale: Scale,
		source: G,
	) -> PixelSurface<'_, G> {
		let (w, h) = (self.width as i32, self.height as i32);
		let stride = self.stride();
		PixelSurface {
			buf: &mut self.data,
			w,
			h,
			stride,
			order,
			scale,
			clip: Rect::new(0, 0, w, h),
			source,
			raster: Rasteriser::new(),
		}
	}
}

/// The Surface over a borrowed buffer.
pub struct PixelSurface<'a, G: GlyphSource> {
	buf: &'a mut [u8],
	w: i32,
	h: i32,
	stride: usize,
	order: ByteOrder,
	scale: Scale,
	/// The clip in device pixels.
	clip: Rect,
	source: G,
	raster: Rasteriser,
}

impl<'a, G: GlyphSource> PixelSurface<'a, G> {
	/// A surface over `buf`, `width` by `height` device pixels with `stride` bytes between rows (at least `width * 4`). `None` when
	/// the buffer is too small for that.
	pub fn new(
		buf: &'a mut [u8],
		width: u32,
		height: u32,
		stride: usize,
		order: ByteOrder,
		scale: Scale,
		source: G,
	) -> Option<Self> {
		let row = (width as usize).checked_mul(4)?;
		if stride < row {
			return None;
		}
		let need = if height == 0 {
			0
		} else {
			stride.checked_mul(height as usize - 1)?.checked_add(row)?
		};
		if buf.len() < need || width > i32::MAX as u32 || height > i32::MAX as u32 {
			return None;
		}
		let (w, h) = (width as i32, height as i32);
		Some(PixelSurface {
			buf,
			w,
			h,
			stride,
			order,
			scale,
			clip: Rect::new(0, 0, w, h),
			source,
			raster: Rasteriser::new(),
		})
	}

	/// The device scale.
	pub fn scale(&self) -> Scale {
		self.scale
	}

	/// The glyph source, to share it with other work.
	pub fn source_mut(&mut self) -> &mut G {
		&mut self.source
	}

	fn pack(&self, c: Rgb, a: u8) -> [u8; 4] {
		let m = |v: u8| mul255(v, a);
		match self.order {
			ByteOrder::Rgba => [m(c.0), m(c.1), m(c.2), a],
			ByteOrder::Bgra => [m(c.2), m(c.1), m(c.0), a],
		}
	}

	/// A logical rectangle in device pixels, clipped to the clip.
	fn device(&self, r: Rect) -> Rect {
		let s = self.scale;
		Rect::new(s.dev(r.left), s.dev(r.top), s.dev(r.right), s.dev(r.bottom)).intersect(self.clip)
	}

	/// The bytes of device row `y`, `x0` to `x1` (already inside the clip).
	fn span(&mut self, y: i32, x0: i32, x1: i32) -> Option<&mut [u8]> {
		let start = (y as usize)
			.checked_mul(self.stride)?
			.checked_add(x0 as usize * 4)?;
		self.buf.get_mut(start..start + (x1 - x0) as usize * 4)
	}

	/// Fills a device rectangle (inside the clip) with an opaque colour.
	fn fill_device(&mut self, r: Rect, c: Rgb) {
		if r.is_empty() {
			return;
		}
		let px = self.pack(c, 255);
		for y in r.top..r.bottom {
			if let Some(row) = self.span(y, r.left, r.right) {
				for p in row.as_chunks_mut::<4>().0 {
					*p = px;
				}
			}
		}
	}

	/// Blends colour `c` over device pixel (`x`, `y`) with coverage `a`; the pixel is inside the clip.
	fn blend(&mut self, x: i32, y: i32, c: Rgb, a: u8) {
		if a == 0 {
			return;
		}
		let src = self.pack(c, a);
		if let Some(p) = self.span(y, x, x + 1) {
			if a == 255 {
				p.copy_from_slice(&src);
			} else {
				let inv = 255 - a;
				for (d, s) in p.iter_mut().zip(src) {
					*d = s.saturating_add(mul255(*d, inv));
				}
			}
		}
	}

	/// Draws the coverage the rasteriser holds at device (`ox`, `oy`) in colour `c`.
	fn draw_coverage(&mut self, ox: i32, oy: i32, c: Rgb) {
		// The rasteriser is taken out so the coverage can be read while pixels are written, then put back for its buffers.
		let mut raster = core::mem::take(&mut self.raster);
		if let Some(cv) = raster.finish() {
			self.blend_coverage(&cv, ox, oy, c);
		}
		self.raster = raster;
	}

	fn blend_coverage(&mut self, cv: &Coverage<'_>, ox: i32, oy: i32, c: Rgb) {
		let (left, top, width, height) = (cv.left, cv.top, cv.width, cv.height);
		let alpha = cv.alpha;
		let clip = self.clip;
		for row in 0..height {
			let y = oy + top + row as i32;
			if y < clip.top || y >= clip.bottom {
				continue;
			}
			let Some(line) = alpha.get(row * width..(row + 1) * width) else {
				continue;
			};
			for (col, a) in line.iter().enumerate() {
				let x = ox + left + col as i32;
				if x >= clip.left && x < clip.right {
					self.blend(x, y, c, *a);
				}
			}
		}
	}

	/// An underline for text drawn with its baseline at `base_y` from `x0` to `x1`, all logical: one line below the baseline.
	pub fn underline(&mut self, x0: i32, x1: i32, base_y: i32, c: Rgb) {
		self.hline(x0, x1, base_y + 1, c);
	}

	/// A strikethrough for text of the given `ascent` (logical pixels), three eighths of the ascent above the baseline.
	pub fn strikethrough(&mut self, x0: i32, x1: i32, base_y: i32, ascent: i32, c: Rgb) {
		self.hline(x0, x1, base_y - (ascent * 3 + 4) / 8, c);
	}
}

/// `a * b / 255`, rounded.
fn mul255(a: u8, b: u8) -> u8 {
	let t = u32::from(a) * u32::from(b) + 128;
	((t + (t >> 8)) >> 8) as u8
}

/// A picture pixel for a device offset into a run of `n` source pixels that starts at the device edge `origin`.
fn source_index(scale: Scale, dev: i32, origin: i32, n: i32) -> i32 {
	scale.logical(dev - origin).clamp(0, (n - 1).max(0))
}

impl<G: GlyphSource> Surface for PixelSurface<'_, G> {
	type Font = PixelFont;

	fn size(&self) -> (i32, i32) {
		(self.scale.logical(self.w), self.scale.logical(self.h))
	}

	fn clip(&mut self, r: Option<Rect>) {
		let all = Rect::new(0, 0, self.w, self.h);
		self.clip = match r {
			None => all,
			Some(r) => {
				let s = self.scale;
				Rect::new(s.dev(r.left), s.dev(r.top), s.dev(r.right), s.dev(r.bottom))
					.intersect(all)
			}
		};
	}

	fn font(&mut self, f: &FontRef) -> PixelFont {
		PixelFont {
			req: *f,
			info: self.source.face(f),
		}
	}

	fn advance(&mut self, f: PixelFont, ch: u8) -> i32 {
		let Some(info) = f.info else {
			return 0;
		};
		let units = u64::from(self.source.advance_units(&f.req, ch));
		let den = 144 * u64::from(info.units_per_em.max(1));
		let num = units * u64::from(f.req.size) * u64::from(BASE_DPI);
		i32::try_from((num + den / 2) / den).unwrap_or(i32::MAX)
	}

	fn fill_rect(&mut self, r: Rect, c: Rgb) {
		let d = self.device(r);
		self.fill_device(d, c);
	}

	fn hline(&mut self, x0: i32, x1: i32, y: i32, c: Rgb) {
		let s = self.scale;
		let top = s.dev(y);
		let r = Rect::new(s.dev(x0), top, s.dev(x1), top + s.line()).intersect(self.clip);
		self.fill_device(r, c);
	}

	fn vline(&mut self, x: i32, y0: i32, y1: i32, c: Rgb) {
		let s = self.scale;
		let left = s.dev(x);
		let r = Rect::new(left, s.dev(y0), left + s.line(), s.dev(y1)).intersect(self.clip);
		self.fill_device(r, c);
	}

	fn glyph_run(
		&mut self,
		f: PixelFont,
		base_y: i32,
		glyphs: &[(u8, i32)],
		fg: Rgb,
		_smooth: bool,
	) {
		let Some(info) = f.info else {
			return;
		};
		// The em in 16.16 device pixels: half points to logical pixels at 96 to the inch, then the scale.
		let em16 =
			i64::from(f.req.size) * i64::from(BASE_DPI) * i64::from(self.scale.raw()) * 256 / 144;
		let base = self.scale.dev(base_y);
		let bold16 = if info.synthetic_bold { em16 / 64 } else { 0 };
		let slant = if info.synthetic_italic { 4 } else { 0 };
		for (ch, x) in glyphs {
			let pen = i64::from(*x) * i64::from(self.scale.raw()) * 256;
			let (whole, frac) = (pen >> 16, pen & 0xFFFF);
			self.raster.begin(em16, info.units_per_em, frac, slant);
			if !self.source.outline(&f.req, *ch, &mut self.raster) {
				continue;
			}
			self.raster.close();
			if bold16 > 0 {
				self.raster.shift(frac + bold16);
				self.source.outline(&f.req, *ch, &mut self.raster);
				self.raster.close();
			}
			let ox = whole.clamp(-(1 << 24), 1 << 24) as i32;
			self.draw_coverage(ox, base, fg);
		}
	}

	fn bitmap4(&mut self, x: i32, y: i32, rows: &[u8], w: i32, h: i32, pal: &[Rgb; 16]) {
		if w <= 0 || h <= 0 {
			return;
		}
		let s = self.scale;
		let (ox, oy) = (s.dev(x), s.dev(y));
		let r = Rect::new(ox, oy, s.dev(x + w), s.dev(y + h)).intersect(self.clip);
		let bytes = (w as usize).div_ceil(2);
		for dy in r.top..r.bottom {
			let sy = source_index(s, dy, oy, h) as usize;
			let Some(src) = rows.get(sy * bytes..(sy + 1) * bytes) else {
				continue;
			};
			for dx in r.left..r.right {
				let sx = source_index(s, dx, ox, w) as usize;
				let Some(b) = src.get(sx / 2) else {
					continue;
				};
				let n = if sx.is_multiple_of(2) { b >> 4 } else { b & 15 };
				if let Some(c) = pal.get(usize::from(n)) {
					self.fill_device(Rect::new(dx, dy, dx + 1, dy + 1), *c);
				}
			}
		}
	}

	fn rgb_row(&mut self, x: i32, y: i32, rgb: &[u8]) {
		let n = (rgb.len() / 3) as i32;
		if n == 0 {
			return;
		}
		let s = self.scale;
		let (ox, top) = (s.dev(x), s.dev(y));
		let r = Rect::new(ox, top, s.dev(x + n), s.dev(y + 1)).intersect(self.clip);
		for dx in r.left..r.right {
			let i = source_index(s, dx, ox, n) as usize * 3;
			if let Some([r8, g8, b8]) = rgb.get(i..i + 3).and_then(|p| <[u8; 3]>::try_from(p).ok())
			{
				self.fill_device(Rect::new(dx, r.top, dx + 1, r.bottom), Rgb(r8, g8, b8));
			}
		}
	}

	fn invert_rect(&mut self, r: Rect) {
		let d = self.device(r);
		if d.is_empty() {
			return;
		}
		for y in d.top..d.bottom {
			if let Some(row) = self.span(y, d.left, d.right) {
				for [c0, c1, c2, a] in row.as_chunks_mut::<4>().0 {
					{
						*c0 = a.saturating_sub(*c0);
						*c1 = a.saturating_sub(*c1);
						*c2 = a.saturating_sub(*c2);
					}
				}
			}
		}
	}
}

/// The resolution the painter's logical pixels are in, and that `OutlineMeasure` must be set to with `with_resolution(96)`.
pub const BASE_DPI: u32 = 96;
