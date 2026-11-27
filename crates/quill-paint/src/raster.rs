// The surfaces: Planar4 (mode 12h) and Linear8 (a linear frame buffer) over caller-owned bytes.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use quill_core::layout::FontRef;
use quill_strike::{wanted_px, StrikeId, StrikeSource, Strikes, MAX_GLYPH_BYTES, MAX_GLYPH_SIDE};

use crate::palette::{dither16, dither256, index256, nearest16, rgb256};
use crate::{Rect, Rgb, Surface};

/// The pixels of a surface in its device's own format.
pub trait Device {
	/// Width in pixels.
	fn width(&self) -> i32;
	/// Height in pixels.
	fn height(&self) -> i32;
	/// The device colour for `c`.
	fn index(&self, c: Rgb) -> u8;
	/// The device colour for a picture pixel at (`x`, `y`), dithered.
	fn dither(&self, rgb: [u8; 3], x: i32, y: i32) -> u8;
	/// Sets a pixel known to be inside the surface.
	fn put(&mut self, x: i32, y: i32, i: u8);
	/// Sets a run of pixels of one row, known to be inside the surface.
	fn span(&mut self, y: i32, x0: i32, x1: i32, i: u8);
	/// Inverts a pixel known to be inside the surface.
	fn invert(&mut self, x: i32, y: i32);
}

/// Mode 12h: 640 × 480, four planes of 38,400 bytes one after the other, 80 bytes a row, the leftmost pixel in the top bit, bit `p`
/// of the colour in plane `p`.
pub struct PlanarDev<'a> {
	buf: &'a mut [u8],
}

/// Bytes in one plane of mode 12h.
pub const PLANE_BYTES: usize = 80 * 480;
/// Bytes of the four planes.
pub const PLANAR_BYTES: usize = 4 * PLANE_BYTES;

impl Device for PlanarDev<'_> {
	fn width(&self) -> i32 {
		640
	}
	fn height(&self) -> i32 {
		480
	}
	fn index(&self, c: Rgb) -> u8 {
		nearest16(c)
	}
	fn dither(&self, rgb: [u8; 3], x: i32, y: i32) -> u8 {
		dither16(rgb, x, y)
	}
	fn put(&mut self, x: i32, y: i32, i: u8) {
		self.span(y, x, x + 1, i);
	}
	fn span(&mut self, y: i32, x0: i32, x1: i32, i: u8) {
		let row = y as usize * 80;
		let (b0, b1) = ((x0 >> 3) as usize, ((x1 - 1) >> 3) as usize);
		let first = 0xFFu8 >> (x0 & 7);
		let last = 0xFFu8 << (7 - ((x1 - 1) & 7));
		for p in 0..4 {
			let ink = i >> p & 1 != 0;
			let plane = p * PLANE_BYTES + row;
			let set = |v: &mut u8, mask: u8| *v = if ink { *v | mask } else { *v & !mask };
			if b0 == b1 {
				if let Some(v) = self.buf.get_mut(plane + b0) {
					set(v, first & last);
				}
				continue;
			}
			if let Some(v) = self.buf.get_mut(plane + b0) {
				set(v, first);
			}
			// Whole bytes in between are one fill, which the runtime does with string moves.
			if let Some(mid) = self.buf.get_mut(plane + b0 + 1..plane + b1) {
				mid.fill(if ink { 0xFF } else { 0x00 });
			}
			if let Some(v) = self.buf.get_mut(plane + b1) {
				set(v, last);
			}
		}
	}
	fn invert(&mut self, x: i32, y: i32) {
		let at = y as usize * 80 + (x >> 3) as usize;
		for p in 0..4 {
			if let Some(v) = self.buf.get_mut(p * PLANE_BYTES + at) {
				*v ^= 0x80 >> (x & 7);
			}
		}
	}
}

/// A linear frame buffer: one byte a pixel, `pitch` bytes a row, colours from the 256-colour palette.
pub struct LinearDev<'a> {
	buf: &'a mut [u8],
	w: i32,
	h: i32,
	pitch: usize,
}

impl Device for LinearDev<'_> {
	fn width(&self) -> i32 {
		self.w
	}
	fn height(&self) -> i32 {
		self.h
	}
	fn index(&self, c: Rgb) -> u8 {
		index256(c)
	}
	fn dither(&self, rgb: [u8; 3], x: i32, y: i32) -> u8 {
		dither256(rgb, x, y)
	}
	fn put(&mut self, x: i32, y: i32, i: u8) {
		if let Some(v) = self.buf.get_mut(y as usize * self.pitch + x as usize) {
			*v = i;
		}
	}
	fn span(&mut self, y: i32, x0: i32, x1: i32, i: u8) {
		let at = y as usize * self.pitch;
		if let Some(s) = self.buf.get_mut(at + x0 as usize..at + x1 as usize) {
			s.fill(i);
		}
	}
	fn invert(&mut self, x: i32, y: i32) {
		let at = y as usize * self.pitch + x as usize;
		if let Some(v) = self.buf.get_mut(at) {
			let c = rgb256(*v);
			*v = index256(Rgb(255 - c.0, 255 - c.1, 255 - c.2));
		}
	}
}

/// Either device, chosen when the surface is made: one `Raster<AnyDev, _>` is the program's only surface, so the painter's generic code
/// (the page, the chrome, the dialogs) is compiled once and not once for each device (DOS-43). Each call is one branch on the variant.
pub enum AnyDev<'a> {
	/// Mode 12h.
	Planar(PlanarDev<'a>),
	/// A linear frame buffer.
	Linear(LinearDev<'a>),
}

impl Device for AnyDev<'_> {
	fn width(&self) -> i32 {
		match self {
			AnyDev::Planar(d) => d.width(),
			AnyDev::Linear(d) => d.width(),
		}
	}
	fn height(&self) -> i32 {
		match self {
			AnyDev::Planar(d) => d.height(),
			AnyDev::Linear(d) => d.height(),
		}
	}
	fn index(&self, c: Rgb) -> u8 {
		match self {
			AnyDev::Planar(d) => d.index(c),
			AnyDev::Linear(d) => d.index(c),
		}
	}
	fn dither(&self, rgb: [u8; 3], x: i32, y: i32) -> u8 {
		match self {
			AnyDev::Planar(d) => d.dither(rgb, x, y),
			AnyDev::Linear(d) => d.dither(rgb, x, y),
		}
	}
	fn put(&mut self, x: i32, y: i32, i: u8) {
		match self {
			AnyDev::Planar(d) => d.put(x, y, i),
			AnyDev::Linear(d) => d.put(x, y, i),
		}
	}
	fn span(&mut self, y: i32, x0: i32, x1: i32, i: u8) {
		match self {
			AnyDev::Planar(d) => d.span(y, x0, x1, i),
			AnyDev::Linear(d) => d.span(y, x0, x1, i),
		}
	}
	fn invert(&mut self, x: i32, y: i32) {
		match self {
			AnyDev::Planar(d) => d.invert(x, y),
			AnyDev::Linear(d) => d.invert(x, y),
		}
	}
}

/// A font as the surfaces draw it: a strike and the scale it is drawn at.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct StrikeFont {
	/// The strike.
	pub id: StrikeId,
	/// 1 to 3.
	pub scale: u8,
	/// The em height in pixels a zoomed page wants when no strike is near enough, so each glyph is resampled to it (see
	/// [`resample_row`]); 0 for the strike as it is.
	pub want: u8,
}

/// A surface over a [`Device`], drawing text from strikes read through a [`StrikeSource`].
pub struct Raster<'a, D: Device, S: StrikeSource> {
	dev: D,
	strikes: Strikes<'a>,
	src: &'a mut S,
	clip: Rect,
	/// The last font asked for and the strike it came to: a line asks for the same one for every character, and choosing a strike
	/// looks at every strike in the file.
	last_font: Option<(FontRef, i32, StrikeFont)>,
}

/// A 640 × 480 mode 12h surface.
pub type Planar4<'a, S> = Raster<'a, PlanarDev<'a>, S>;
/// A one-byte-a-pixel surface.
pub type Linear8<'a, S> = Raster<'a, LinearDev<'a>, S>;

/// A surface over either device.
pub type AnyRaster<'a, S> = Raster<'a, AnyDev<'a>, S>;

impl<'a, S: StrikeSource> AnyRaster<'a, S> {
	/// The same as [`Planar4::new`], as an [`AnyRaster`].
	pub fn planar(buf: &'a mut [u8], strikes: Strikes<'a>, src: &'a mut S) -> Option<Self> {
		if buf.len() < PLANAR_BYTES {
			return None;
		}
		Some(Raster::over(
			AnyDev::Planar(PlanarDev { buf }),
			strikes,
			src,
		))
	}

	/// The same as [`Linear8::new`], as an [`AnyRaster`].
	pub fn linear(
		buf: &'a mut [u8],
		w: i32,
		h: i32,
		pitch: usize,
		strikes: Strikes<'a>,
		src: &'a mut S,
	) -> Option<Self> {
		let need = pitch.checked_mul(usize::try_from(h).ok()?)?;
		if w <= 0 || h <= 0 || pitch < w as usize || buf.len() < need {
			return None;
		}
		Some(Raster::over(
			AnyDev::Linear(LinearDev { buf, w, h, pitch }),
			strikes,
			src,
		))
	}
}

impl<'a, S: StrikeSource> Planar4<'a, S> {
	/// A surface over the four planes in `buf` (153,600 bytes), or `None` when `buf` is shorter.
	pub fn new(buf: &'a mut [u8], strikes: Strikes<'a>, src: &'a mut S) -> Option<Self> {
		if buf.len() < PLANAR_BYTES {
			return None;
		}
		Some(Raster::over(PlanarDev { buf }, strikes, src))
	}
}

impl<'a, S: StrikeSource> Linear8<'a, S> {
	/// A surface of `w` by `h` pixels over `buf`, `pitch` bytes a row; `None` when the sizes do not fit the bytes.
	pub fn new(
		buf: &'a mut [u8],
		w: i32,
		h: i32,
		pitch: usize,
		strikes: Strikes<'a>,
		src: &'a mut S,
	) -> Option<Self> {
		let need = pitch.checked_mul(usize::try_from(h).ok()?)?;
		if w <= 0 || h <= 0 || pitch < w as usize || buf.len() < need {
			return None;
		}
		Some(Raster::over(LinearDev { buf, w, h, pitch }, strikes, src))
	}
}

impl<'a, D: Device, S: StrikeSource> Raster<'a, D, S> {
	fn over(dev: D, strikes: Strikes<'a>, src: &'a mut S) -> Self {
		let clip = Rect::new(0, 0, dev.width(), dev.height());
		Raster {
			dev,
			strikes,
			src,
			clip,
			last_font: None,
		}
	}

	fn bounds(&self) -> Rect {
		Rect::new(0, 0, self.dev.width(), self.dev.height())
	}

	fn plot(&mut self, x: i32, y: i32, i: u8) {
		let c = self.clip;
		if x >= c.left && x < c.right && y >= c.top && y < c.bottom {
			self.dev.put(x, y, i);
		}
	}

	fn fill_i(&mut self, r: Rect, i: u8) {
		let r = r.intersect(self.clip);
		if r.is_empty() {
			return;
		}
		for y in r.top..r.bottom {
			self.dev.span(y, r.left, r.right, i);
		}
	}
}

/// How the pixels of a glyph's levels are inked.
struct Inks {
	ink: u8,
	greys: [u8; 4],
	grey: bool,
}

/// Unpacks one row of a glyph's bitmap (1 or 2 bits a pixel) into levels 0 to 3. False when the row is blank.
fn unpack(packed: &[u8], bpp: u8, width: usize, lv: &mut [u8; MAX_GLYPH_SIDE]) -> bool {
	if packed.iter().all(|b| *b == 0) {
		return false;
	}
	if bpp == 1 {
		for (col, l) in lv.iter_mut().enumerate().take(width) {
			let b = packed.get(col >> 3).copied().unwrap_or(0);
			*l = if (b >> (7 - (col & 7))) & 1 != 0 {
				3
			} else {
				0
			};
		}
	} else {
		for (col, l) in lv.iter_mut().enumerate().take(width) {
			let b = packed.get(col >> 2).copied().unwrap_or(0);
			*l = (b >> (6 - 2 * (col & 3))) & 3;
		}
	}
	true
}

impl<D: Device, S: StrikeSource> Raster<'_, D, S> {
	/// Draws one row of glyph levels from (`x`, `y`), each pixel `k` by `k`: a run of equal levels is one span.
	fn levels(
		&mut self,
		lv: &[u8; MAX_GLYPH_SIDE],
		width: usize,
		(x, y): (i32, i32),
		k: i32,
		inks: &Inks,
	) {
		let mut col = 0;
		while col < width {
			let level = lv.get(col).copied().unwrap_or(0);
			let paint = if inks.grey { level > 0 } else { level >= 2 };
			if !paint {
				col += 1;
				continue;
			}
			let colour = if inks.grey {
				inks.greys.get(usize::from(level)).copied().unwrap_or(0)
			} else {
				inks.ink
			};
			let mut end = col + 1;
			while end < width && lv.get(end).copied() == Some(level) {
				end += 1;
			}
			let x0 = x.saturating_add(col as i32 * k);
			let x1 = x.saturating_add(end as i32 * k);
			self.fill_i(Rect::new(x0, y, x1, y.saturating_add(k)), colour);
			col = end;
		}
	}
}

impl<D: Device, S: StrikeSource> Surface for Raster<'_, D, S> {
	type Font = StrikeFont;

	fn size(&self) -> (i32, i32) {
		(self.dev.width(), self.dev.height())
	}

	fn clip(&mut self, r: Option<Rect>) {
		let b = self.bounds();
		self.clip = r.map_or(b, |r| r.intersect(b));
	}

	fn font(&mut self, f: &FontRef) -> StrikeFont {
		self.font_at(f, 100)
	}

	fn font_at(&mut self, f: &FontRef, pct: i32) -> StrikeFont {
		if let Some((key, at, font)) = &self.last_font {
			if key == f && *at == pct {
				return *font;
			}
		}
		let font = if pct == 100 {
			let (id, scale) = self.strikes.strike_for(f);
			StrikeFont { id, scale, want: 0 }
		} else {
			let want_px = ((wanted_px(f) as i32 * pct + 50) / 100).clamp(1, 255) as u32;
			let (id, scale) = self.strikes.strike_for_px(f, want_px);
			let have = u32::from(self.strikes.strike(id).pixels) * u32::from(scale);
			// A strike within an eighth of the size wanted is drawn as it is; further off, its glyphs are resampled.
			let near = want_px.abs_diff(have) * 8 <= want_px;
			StrikeFont {
				id,
				scale,
				want: if near { 0 } else { want_px as u8 },
			}
		};
		self.last_font = Some((*f, pct, font));
		font
	}

	fn advance(&mut self, f: StrikeFont, ch: u8) -> i32 {
		let full = i32::from(self.strikes.advance(f.id, ch)) * i32::from(f.scale);
		if f.want == 0 {
			return full;
		}
		let have = i32::from(self.strikes.strike(f.id).pixels) * i32::from(f.scale);
		(full * i32::from(f.want) + have / 2) / have.max(1)
	}

	fn fill_rect(&mut self, r: Rect, c: Rgb) {
		let i = self.dev.index(c);
		self.fill_i(r, i);
	}

	fn hline(&mut self, x0: i32, x1: i32, y: i32, c: Rgb) {
		self.fill_rect(Rect::new(x0, y, x1, y.saturating_add(1)), c);
	}

	fn vline(&mut self, x: i32, y0: i32, y1: i32, c: Rgb) {
		self.fill_rect(Rect::new(x, y0, x.saturating_add(1), y1), c);
	}

	fn glyph_run(
		&mut self,
		f: StrikeFont,
		base_y: i32,
		glyphs: &[(u8, i32)],
		fg: Rgb,
		smooth: bool,
	) {
		let ink = Inks {
			ink: self.dev.index(fg),
			greys: [
				0,
				self.dev.index(Rgb::SILVER),
				self.dev.index(Rgb::GREY),
				self.dev.index(Rgb::BLACK),
			],
			grey: smooth && fg == Rgb::BLACK,
		};
		let k = i32::from(f.scale);
		let have = i32::from(self.strikes.strike(f.id).pixels) * k;
		let mut bits = [0u8; MAX_GLYPH_BYTES];
		for &(ch, x) in glyphs {
			let Some(g) = self.strikes.glyph(self.src, f.id, ch, &mut bits) else {
				continue;
			};
			let mut lv = [0u8; MAX_GLYPH_SIDE];
			if f.want == 0 {
				let gx = x.saturating_add(g.left * k);
				let gy = base_y.saturating_sub(g.top * k);
				for row in 0..g.height {
					let y0 = gy.saturating_add(row as i32 * k);
					let Some(packed) = bits.get(row * g.stride..(row + 1) * g.stride) else {
						continue;
					};
					if unpack(packed, g.bpp, g.width, &mut lv) {
						self.levels(&lv, g.width, (gx, y0), k, &ink);
					}
				}
				continue;
			}
			// Resampled: the glyph is mapped onto the grid of the size wanted, anchored at the pen and the baseline, so every glyph of
			// a line shares one baseline whatever its bitmap's height. A source pixel is `k * want` wide and a pixel wanted `have`, in
			// one unit, so the overlap of the two is an integer and the levels average by area.
			let ks = k * i32::from(f.want);
			let (left, top) = (g.left, g.top);
			let (width, height) = (g.width as i32, g.height as i32);
			let col0 = (left * ks).div_euclid(have);
			let col1 = ((left + width) * ks + have - 1).div_euclid(have);
			let row0 = (-top * ks).div_euclid(have);
			let row1 = ((height - top) * ks + have - 1).div_euclid(have);
			let cols = (col1 - col0).max(0) as usize;
			if cols > MAX_GLYPH_SIDE || cols == 0 {
				continue;
			}
			let area = have * have;
			for j in row0..row1 {
				let mut acc = [0i32; MAX_GLYPH_SIDE];
				let lo = (j * have).div_euclid(ks) + top;
				let hi = ((j + 1) * have + ks - 1).div_euclid(ks) + top;
				for sy in lo.max(0)..hi.min(height) {
					let (s0, s1) = ((sy - top) * ks, (sy - top + 1) * ks);
					let oy = s1.min((j + 1) * have) - s0.max(j * have);
					let Some(packed) =
						bits.get(sy as usize * g.stride..(sy as usize + 1) * g.stride)
					else {
						continue;
					};
					let mut src = [0u8; MAX_GLYPH_SIDE];
					if !unpack(packed, g.bpp, g.width, &mut src) {
						continue;
					}
					for (sx, level) in src.iter().enumerate().take(g.width) {
						if *level == 0 {
							continue;
						}
						let a0 = (left + sx as i32) * ks;
						let a1 = a0 + ks;
						for c in a0.div_euclid(have)..=(a1 - 1).div_euclid(have) {
							let ox = a1.min((c + 1) * have) - a0.max(c * have);
							if let Some(slot) = acc.get_mut((c - col0) as usize) {
								*slot += i32::from(*level) * ox * oy;
							}
						}
					}
				}
				for (out, sum) in lv.iter_mut().zip(acc.iter()).take(cols) {
					*out = ((sum + area / 2) / area).min(3) as u8;
				}
				self.levels(&lv, cols, (x + col0, base_y + j), 1, &ink);
			}
		}
	}

	fn bitmap4(&mut self, x: i32, y: i32, rows: &[u8], w: i32, h: i32, pal: &[Rgb; 16]) {
		let map: [u8; 16] = core::array::from_fn(|n| pal.get(n).map_or(0, |c| self.dev.index(*c)));
		let stride = ((w.max(0) + 1) / 2) as usize;
		for row in 0..h.max(0) {
			for col in 0..w.max(0) {
				let at = row as usize * stride + (col / 2) as usize;
				let Some(b) = rows.get(at) else {
					continue;
				};
				let n = if col & 1 == 0 { b >> 4 } else { b & 15 };
				let i = map.get(usize::from(n)).copied().unwrap_or(0);
				self.plot(x.saturating_add(col), y.saturating_add(row), i);
			}
		}
	}

	fn rgb_row(&mut self, x: i32, y: i32, rgb: &[u8]) {
		let c = self.clip;
		if y < c.top || y >= c.bottom {
			return;
		}
		let (pixels, _) = rgb.as_chunks::<3>();
		for (n, p) in pixels.iter().enumerate() {
			let px = x.saturating_add(n as i32);
			if px < c.left || px >= c.right {
				continue;
			}
			let i = self.dev.dither(*p, px, y);
			self.dev.put(px, y, i);
		}
	}

	fn invert_rect(&mut self, r: Rect) {
		let r = r.intersect(self.clip);
		for y in r.top..r.bottom {
			for x in r.left..r.right {
				self.dev.invert(x, y);
			}
		}
	}
}
