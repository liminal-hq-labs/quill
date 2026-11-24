// Scaling a picture to the size it is shown at, a row at a time.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::dither::shrink_row;

/// The widest picture, in pixels, the scaler produces.
pub const MAX_OUT: usize = 1024;

/// Takes the rows of a picture in the order a decoder produces them and gives back rows of the shown size in the same order:
/// a box filter across and down when shrinking, repeated pixels and rows when growing. Working memory is about 16 KB.
pub struct Scaler {
	out_w: usize,
	out_h: u32,
	src_h: u32,
	seen: u32,
	n: u32,
	acc: [u32; MAX_OUT * 3],
	tmp: [u8; MAX_OUT * 3],
}

impl Scaler {
	/// A scaler for a picture `src_h` rows tall, shown `out_w` by `out_h` pixels (`out_w` at most [`MAX_OUT`]).
	pub fn new(src_h: u32, out_w: usize, out_h: u32) -> Self {
		Scaler {
			out_w: out_w.clamp(1, MAX_OUT),
			out_h: out_h.max(1),
			src_h: src_h.max(1),
			seen: 0,
			n: 0,
			acc: [0; MAX_OUT * 3],
			tmp: [0; MAX_OUT * 3],
		}
	}

	/// Takes the next source row (RGB, three bytes a pixel) and calls `emit(k, row)` for every output row it completes, `k` counting
	/// from 0 in arrival order.
	pub fn push(&mut self, rgb: &[u8], mut emit: impl FnMut(u32, &[u8])) {
		let k = self.seen;
		if k >= self.src_h {
			return;
		}
		self.seen += 1;
		let (oh, sh) = (u64::from(self.out_h), u64::from(self.src_h));
		let cols = self.out_w * 3;
		if let Some(t) = self.tmp.get_mut(..cols) {
			shrink_row(rgb, self.out_w, t);
		}
		let row = self.tmp.get(..cols).unwrap_or(&[]);
		if self.out_h >= self.src_h {
			let first = (u64::from(k) * oh).div_ceil(sh);
			let end = (u64::from(k + 1) * oh).div_ceil(sh);
			for oy in first..end {
				emit(oy as u32, row);
			}
			return;
		}
		let oy = (u64::from(k) * oh / sh) as u32;
		for (a, v) in self.acc.iter_mut().zip(row) {
			*a += u32::from(*v);
		}
		self.n += 1;
		if u64::from(k + 1) * oh / sh != u64::from(oy) || k + 1 == self.src_h {
			let n = self.n.max(1);
			for (v, a) in self.tmp.iter_mut().zip(self.acc.iter()).take(cols) {
				*v = (*a / n) as u8;
			}
			emit(oy, self.tmp.get(..cols).unwrap_or(&[]));
			self.acc.fill(0);
			self.n = 0;
		}
	}
}

#[cfg(test)]
#[allow(clippy::indexing_slicing)]
mod tests {
	extern crate std;
	use super::*;
	use std::vec;
	use std::vec::Vec;

	fn run(
		src_h: u32,
		src_w: usize,
		out_w: usize,
		out_h: u32,
		value: impl Fn(u32) -> u8,
	) -> Vec<(u32, Vec<u8>)> {
		let mut s = Scaler::new(src_h, out_w, out_h);
		let mut got = Vec::new();
		for y in 0..src_h {
			let row = vec![value(y); src_w * 3];
			s.push(&row, |k, r| got.push((k, r.to_vec())));
		}
		got
	}

	#[test]
	fn shrinking_averages_rows_and_gives_every_output_row_once() {
		let got = run(8, 8, 4, 4, |y| (y * 10) as u8);
		let ks: Vec<u32> = got.iter().map(|g| g.0).collect();
		assert_eq!(ks, [0, 1, 2, 3]);
		assert_eq!(got[0].1, vec![5; 12]);
		assert_eq!(got[3].1, vec![65; 12]);
	}

	#[test]
	fn growing_repeats_rows_and_pixels() {
		let got = run(2, 2, 6, 6, |y| (y * 100) as u8);
		let ks: Vec<u32> = got.iter().map(|g| g.0).collect();
		assert_eq!(ks, [0, 1, 2, 3, 4, 5]);
		assert_eq!(got[2].1, vec![0; 18]);
		assert_eq!(got[3].1, vec![100; 18]);
	}

	#[test]
	fn odd_sizes_still_cover_every_row() {
		for (sh, oh) in [(7, 3), (3, 7), (100, 99), (1, 5), (5, 1)] {
			let got = run(sh, 3, 3, oh, |_| 9);
			let ks: Vec<u32> = got.iter().map(|g| g.0).collect();
			assert_eq!(ks, (0..oh).collect::<Vec<_>>(), "{sh} to {oh}");
		}
	}
}
