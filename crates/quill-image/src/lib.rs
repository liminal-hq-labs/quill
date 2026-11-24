// quill-image: pictures decoded by rows into a caller's sink, then reduced to what the screen shows.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Nothing here holds a whole picture: a decoder reads one row at a time (a BMP bottom-up, as it is stored) and hands it to a sink,
//! so a 640 by 480 picture costs a few kilobytes of working memory on a 4 MB machine.

#![no_std]
#![deny(
	clippy::unwrap_used,
	clippy::expect_used,
	clippy::panic,
	clippy::indexing_slicing
)]

pub mod bmp;
pub mod dither;
pub mod png;
pub mod scale;

/// The most pixels along a side of a picture Quill reads.
pub const MAX_SIDE: u32 = 4096;
/// The widest row a decoder holds, in RGB bytes.
pub const MAX_ROW: usize = MAX_SIDE as usize * 3;

/// Why a picture could not be read.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ImageError {
	/// Not a picture of this kind, or damaged.
	BadFile,
	/// A kind of picture Quill does not decode (RLE bitmaps, 16 or 32 bits a pixel).
	Unsupported,
	/// A side over 4,096 pixels, or no pixels at all.
	TooLarge,
}

/// A big-endian `u32` from four bytes, built a byte at a time. `u32::from_be_bytes` becomes `bswap`, which the 386 does not have (it
/// came in with the 486) and which LLVM emits for an i386 target all the same; the Windows 3.x build must run on a 386. Each byte goes
/// through `black_box` so the shifts are not turned back into a `bswap`.
pub fn be32_bytes(b: [u8; 4]) -> u32 {
	b.iter()
		.fold(0u32, |a, &x| (a << 8) | u32::from(core::hint::black_box(x)))
}
