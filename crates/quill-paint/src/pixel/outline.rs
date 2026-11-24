// The outline rasteriser: a glyph's curves in font units to 8-bit coverage, in 16.16 fixed point and without a float.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;

/// Outline coordinates are font units times this: six fractional bits, y up.
pub const FUNIT_SHIFT: u32 = 6;

/// A glyph outline, one drawing command at a time. Coordinates are in 1/64 of a font unit with y pointing up.
pub trait PathSink {
	/// Starts a contour.
	fn move_to(&mut self, x: i32, y: i32);
	/// A straight line to a point.
	fn line_to(&mut self, x: i32, y: i32);
	/// A quadratic curve (TrueType) with its control point and end.
	fn quad_to(&mut self, cx: i32, cy: i32, x: i32, y: i32);
	/// A cubic curve (CFF) with its two control points and end.
	fn curve_to(&mut self, c1x: i32, c1y: i32, c2x: i32, c2y: i32, x: i32, y: i32);
	/// Closes the contour with a line back to where it started.
	fn close(&mut self);
}

/// One pixel in 16.16 fixed point.
const ONE: i64 = 1 << 16;
/// The most curve pieces one curve is cut into.
const MAX_PIECES: i64 = 32;
/// The widest or tallest glyph bitmap that is drawn, in pixels; anything larger is not text.
const MAX_SIDE: i64 = 2048;

/// Square root of an integer, rounded down.
fn isqrt(v: u64) -> u64 {
	if v < 2 {
		return v;
	}
	let mut x = v;
	let mut y = x.div_ceil(2).max(1);
	while y < x {
		x = y;
		y = (x + v / x) / 2;
	}
	x
}

/// A line in 16.16 device pixels, y down.
#[derive(Copy, Clone)]
struct Seg {
	x0: i64,
	y0: i64,
	x1: i64,
	y1: i64,
}

/// The coverage a glyph leaves: its box in pixels from the pen position, and one byte a pixel.
pub struct Coverage<'a> {
	/// Column of the first pixel, from the pen's whole-pixel position.
	pub left: i32,
	/// Row of the first pixel, from the baseline (negative above it).
	pub top: i32,
	/// Width in pixels.
	pub width: usize,
	/// Height in pixels.
	pub height: usize,
	/// `width * height` bytes, 0 for none and 255 for all.
	pub alpha: &'a [u8],
}

/// Turns outlines into coverage. It keeps its buffers between glyphs, so a run of text allocates only when a glyph is larger than
/// any before it.
#[derive(Default)]
pub struct Rasteriser {
	segs: Vec<Seg>,
	area: Vec<i32>,
	alpha: Vec<u8>,
	/// Font units times 64 to 16.16 pixels: numerator (the em in 16.16 pixels) and denominator (units to the em times 64).
	em16: i64,
	den: i64,
	/// Shear: x grows by y over this, 0 for none.
	slant_div: i64,
	/// Shift of the whole outline, 16.16 pixels.
	dx: i64,
	cur: (i64, i64),
	start: (i64, i64),
}

impl Rasteriser {
	/// An empty rasteriser.
	pub fn new() -> Rasteriser {
		Rasteriser::default()
	}

	/// Begins a glyph at a pen position `frac_x` (16.16 pixels, 0 to 65535 past a whole pixel). `em16` is the em in 16.16 pixels
	/// and `upem` the font's units to the em. A `slant_div` of 4 leans the glyph by a quarter (a synthetic italic); 0 is upright.
	pub fn begin(&mut self, em16: i64, upem: u16, frac_x: i64, slant_div: i64) {
		self.segs.clear();
		self.em16 = em16;
		self.den = i64::from(upem.max(1)) << FUNIT_SHIFT;
		self.slant_div = slant_div;
		self.dx = frac_x;
		self.cur = (0, 0);
		self.start = (0, 0);
	}

	/// Moves the outline right by `dx` 16.16 pixels, for the second pass of a synthetic bold.
	pub fn shift(&mut self, dx: i64) {
		self.dx = dx;
	}

	fn map(&self, x: i32, y: i32) -> (i64, i64) {
		let fx = i64::from(x) * self.em16 / self.den;
		let fy = i64::from(y) * self.em16 / self.den;
		let lean = if self.slant_div > 0 {
			fy / self.slant_div
		} else {
			0
		};
		(fx + lean + self.dx, -fy)
	}

	fn line(&mut self, to: (i64, i64)) {
		let from = self.cur;
		if from != to {
			self.segs.push(Seg {
				x0: from.0,
				y0: from.1,
				x1: to.0,
				y1: to.1,
			});
		}
		self.cur = to;
	}

	/// The point at `i / n` of a quadratic.
	fn quad_at(p: [(i64, i64); 3], i: i64, n: i64) -> (i64, i64) {
		let (a, b, c) = ((n - i) * (n - i), 2 * i * (n - i), i * i);
		let d = n * n;
		(
			(a * p[0].0 + b * p[1].0 + c * p[2].0) / d,
			(a * p[0].1 + b * p[1].1 + c * p[2].1) / d,
		)
	}

	/// The point at `i / n` of a cubic.
	fn cubic_at(p: [(i64, i64); 4], i: i64, n: i64) -> (i64, i64) {
		let m = n - i;
		let (a, b, c, e) = (m * m * m, 3 * m * m * i, 3 * m * i * i, i * i * i);
		let d = n * n * n;
		(
			(a * p[0].0 + b * p[1].0 + c * p[2].0 + e * p[3].0) / d,
			(a * p[0].1 + b * p[1].1 + c * p[2].1 + e * p[3].1) / d,
		)
	}

	/// Pieces for a curve whose second difference is `d` (16.16 pixels): the error of a piece stays under an eighth of a pixel.
	fn pieces(d: i64, k: u64) -> i64 {
		let n = isqrt(d.unsigned_abs() * k / (4 * (ONE as u64 / 8))) + 1;
		(n as i64).clamp(1, MAX_PIECES)
	}

	/// Fills the coverage of everything pushed since `begin`, with the non-zero rule, or `None` when nothing is drawn.
	pub fn finish(&mut self) -> Option<Coverage<'_>> {
		let first = *self.segs.first()?;
		let (mut minx, mut maxx, mut miny, mut maxy) = (first.x0, first.x0, first.y0, first.y0);
		for s in &self.segs {
			for (x, y) in [(s.x0, s.y0), (s.x1, s.y1)] {
				minx = minx.min(x);
				maxx = maxx.max(x);
				miny = miny.min(y);
				maxy = maxy.max(y);
			}
		}
		let left = minx.div_euclid(ONE);
		let top = miny.div_euclid(ONE);
		let width = (maxx + ONE - 1).div_euclid(ONE) - left + 2;
		let height = (maxy + ONE - 1).div_euclid(ONE) - top + 1;
		if width <= 0 || height <= 0 || width > MAX_SIDE || height > MAX_SIDE {
			return None;
		}
		let (w, h) = (width as usize, height as usize);
		self.area.clear();
		self.area.resize(w * h + 2, 0);
		let (ox, oy) = (left * ONE, top * ONE);
		for i in 0..self.segs.len() {
			if let Some(s) = self.segs.get(i).copied() {
				accumulate(
					&mut self.area,
					w,
					h,
					(s.x0 - ox, s.y0 - oy),
					(s.x1 - ox, s.y1 - oy),
				);
			}
		}
		self.alpha.clear();
		self.alpha.reserve(w * h);
		for row in self.area.chunks_exact(w).take(h) {
			let mut acc = 0i64;
			for v in row {
				acc += i64::from(*v);
				let a = acc.abs().min(ONE);
				self.alpha.push(((a * 255 + ONE / 2) >> 16) as u8);
			}
		}
		Some(Coverage {
			left: left as i32,
			top: top as i32,
			width: w,
			height: h,
			alpha: &self.alpha,
		})
	}
}

impl PathSink for Rasteriser {
	fn move_to(&mut self, x: i32, y: i32) {
		let p = self.map(x, y);
		// The previous contour is closed first, so a missing close cannot leave the fill open.
		if self.cur != self.start {
			self.line(self.start);
		}
		self.cur = p;
		self.start = p;
	}

	fn line_to(&mut self, x: i32, y: i32) {
		let p = self.map(x, y);
		self.line(p);
	}

	fn quad_to(&mut self, cx: i32, cy: i32, x: i32, y: i32) {
		let p = [self.cur, self.map(cx, cy), self.map(x, y)];
		let d = (p[0].0 - 2 * p[1].0 + p[2].0).abs() + (p[0].1 - 2 * p[1].1 + p[2].1).abs();
		let n = Self::pieces(d, 1);
		for i in 1..=n {
			let q = Self::quad_at(p, i, n);
			self.line(q);
		}
	}

	fn curve_to(&mut self, c1x: i32, c1y: i32, c2x: i32, c2y: i32, x: i32, y: i32) {
		let p = [
			self.cur,
			self.map(c1x, c1y),
			self.map(c2x, c2y),
			self.map(x, y),
		];
		let d0 = (p[0].0 - 2 * p[1].0 + p[2].0).abs() + (p[0].1 - 2 * p[1].1 + p[2].1).abs();
		let d1 = (p[1].0 - 2 * p[2].0 + p[3].0).abs() + (p[1].1 - 2 * p[2].1 + p[3].1).abs();
		let n = Self::pieces(d0.max(d1), 3);
		for i in 1..=n {
			let q = Self::cubic_at(p, i, n);
			self.line(q);
		}
	}

	fn close(&mut self) {
		let s = self.start;
		self.line(s);
	}
}

/// Adds one line's signed area to the accumulation buffer, a row at a time. Summing a row left to right then gives the winding
/// coverage of each pixel. Coordinates are 16.16 pixels from the buffer's corner.
fn accumulate(a: &mut [i32], w: usize, h: usize, p0: (i64, i64), p1: (i64, i64)) {
	if p0.1 == p1.1 {
		return;
	}
	let (dir, p0, p1) = if p0.1 < p1.1 {
		(1i64, p0, p1)
	} else {
		(-1i64, p1, p0)
	};
	let dxdy = ((p1.0 - p0.0) << 16) / (p1.1 - p0.1);
	let y_first = (p0.1 >> 16).max(0);
	let y_last = ((p1.1 + ONE - 1) >> 16).min(h as i64);
	let mut add = |idx: i64, v: i64| {
		if idx >= 0 {
			if let Some(c) = a.get_mut(idx as usize) {
				*c = c.saturating_add(v as i32);
			}
		}
	};
	let mut y = y_first;
	while y < y_last {
		let ytop = (y << 16).max(p0.1);
		let ybot = ((y + 1) << 16).min(p1.1);
		let dy = ybot - ytop;
		if dy <= 0 {
			y += 1;
			continue;
		}
		let xs = p0.0 + ((dxdy * (ytop - p0.1)) >> 16);
		let xe = p0.0 + ((dxdy * (ybot - p0.1)) >> 16);
		let d = dy * dir;
		let (x0, x1) = if xs < xe { (xs, xe) } else { (xe, xs) };
		let row = y * w as i64;
		let x0i = x0 >> 16;
		let x1i = (x1 + ONE - 1) >> 16;
		if x1i <= x0i + 1 {
			let xmf = ((xs + xe) / 2) - (x0i << 16);
			add(row + x0i, d - ((d * xmf) >> 16));
			add(row + x0i + 1, (d * xmf) >> 16);
		} else {
			let s = (1i64 << 32) / (x1 - x0).max(1);
			let x0f = x0 - (x0i << 16);
			let u = ONE - x0f;
			let a0 = ((((u * u) >> 16) * s) >> 16) >> 1;
			let x1f = x1 - ((x1i - 1) << 16);
			let am = ((((x1f * x1f) >> 16) * s) >> 16) >> 1;
			add(row + x0i, (d * a0) >> 16);
			if x1i == x0i + 2 {
				add(row + x0i + 1, (d * (ONE - a0 - am)) >> 16);
			} else {
				let a1 = (s * (98304 - x0f)) >> 16;
				add(row + x0i + 1, (d * (a1 - a0)) >> 16);
				for xi in (x0i + 2)..(x1i - 1) {
					add(row + xi, (d * s) >> 16);
				}
				let a2 = a1 + (x1i - x0i - 3) * s;
				add(row + x1i - 1, (d * (ONE - a2 - am)) >> 16);
			}
			add(row + x1i, (d * am) >> 16);
		}
		y += 1;
	}
}
