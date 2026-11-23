// Drawing a document's picture at the size it is shown: decoded by rows from the picture pool, scaled, handed to the caller.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use quill_core::doc::Doc;
use quill_core::store::Store;
use quill_formats::io::Source;
use quill_image::scale::{Scaler, MAX_OUT};
use quill_image::{bmp, png, ImageError};

pub use quill_inflate::WINDOW;

/// A picture's file bytes in the document's pool, read from the start.
pub struct PictureSource<'a, S: Store + ?Sized> {
	doc: &'a Doc,
	s: &'a S,
	idx: u16,
	pos: u32,
}

impl<'a, S: Store + ?Sized> PictureSource<'a, S> {
	/// The bytes of picture `idx`.
	pub fn new(doc: &'a Doc, s: &'a S, idx: u16) -> Self {
		PictureSource {
			doc,
			s,
			idx,
			pos: 0,
		}
	}
}

impl<S: Store + ?Sized> Source for PictureSource<'_, S> {
	fn read(&mut self, out: &mut [u8]) -> usize {
		let n = self.doc.picture_bytes(self.s, self.idx, self.pos, out);
		self.pos += n as u32;
		n
	}
}

/// A fingerprint of a picture (its size, place and first bytes), to tell one picture from another that took its index.
pub fn fingerprint<S: Store + ?Sized>(doc: &Doc, s: &S, idx: u16) -> u32 {
	let mut h = 0x811C_9DC5u32;
	if let Some(p) = doc.pictures.get(s, u32::from(idx)) {
		let mut b = [0u8; 128];
		let n = doc.picture_bytes(s, idx, 0, &mut b);
		for v in b
			.iter()
			.take(n)
			.copied()
			.chain(p.len.to_le_bytes())
			.chain(p.offset.to_le_bytes())
		{
			h = (h ^ u32::from(v)).wrapping_mul(0x0100_0193);
		}
	}
	h
}

/// Decodes picture `idx` and calls `emit(row, rgb)` for each of its `out_h` rows of `out_w` pixels, where `row` 0 is the *bottom* row
/// (the order a Windows DIB wants) and `rgb` holds red, green, blue for each pixel. `window` is the inflater's working memory.
/// A picture that stops early leaves its remaining rows unwritten and returns an error.
pub fn render<S: Store + ?Sized>(
	doc: &Doc,
	s: &S,
	idx: u16,
	out_w: usize,
	out_h: u32,
	window: &mut [u8; WINDOW],
	mut emit: impl FnMut(u32, &[u8]),
) -> Result<(), ImageError> {
	let pic = doc
		.pictures
		.get(s, u32::from(idx))
		.ok_or(ImageError::BadFile)?;
	if out_w == 0 || out_w > MAX_OUT || out_h == 0 {
		return Err(ImageError::TooLarge);
	}
	let mut scaler = Scaler::new(u32::from(pic.h), out_w, out_h);
	let mut src = PictureSource::new(doc, s, idx);
	if pic.kind == 0 {
		let info = bmp::read_info(&mut src)?;
		let flip = info.top_down;
		scaler = Scaler::new(info.height, out_w, out_h);
		bmp::decode_rows(&mut src, &info, |_, rgb| {
			scaler.push(rgb, |k, row| {
				emit(if flip { out_h - 1 - k } else { k }, row)
			});
		})
	} else {
		png::decode_rows(&mut src, window, |_, rgb| {
			scaler.push(rgb, |k, row| emit(out_h - 1 - k, row));
		})
		.map(|_| ())
	}
}
