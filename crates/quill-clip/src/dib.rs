// The DIB flavour: one bitmap picture, as CF_DIB holds it.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::ops::Range;

use quill_core::doc::Doc;
use quill_core::store::Store;
use quill_formats::io::{Sink, Source};
use quill_image::bmp;

use crate::ClipError;

/// The bytes of a bitmap file before its DIB.
const FILE_HEADER: u32 = 14;

/// The index of the BMP picture when `range` is exactly one inline picture and nothing else.
pub(crate) fn lone_bmp<S: Store + ?Sized>(doc: &Doc, s: &S, range: &Range<u32>) -> Option<u16> {
	if range.end.checked_sub(range.start) != Some(1) {
		return None;
	}
	let v = doc.body.object_at(s, range.start)?;
	if v >> 16 != 0 {
		return None;
	}
	let idx = (v & 0xFFFF) as u16;
	let p = doc.pictures.get(s, u32::from(idx))?;
	(p.kind == 0).then_some(idx)
}

/// A picture's file bytes as a source.
struct PictureSource<'a, S: Store + ?Sized> {
	doc: &'a Doc,
	s: &'a S,
	idx: u16,
	pos: u32,
}

impl<S: Store + ?Sized> Source for PictureSource<'_, S> {
	fn read(&mut self, out: &mut [u8]) -> usize {
		let n = self.doc.picture_bytes(self.s, self.idx, self.pos, out);
		self.pos += n as u32;
		n
	}
}

/// Writes the DIB of BMP picture `idx`: the file without its 14-byte header. A bitmap Quill cannot decode is not offered, since
/// a program that pastes it could not show it either.
pub(crate) fn write<S: Store + ?Sized, K: Sink>(
	doc: &Doc,
	s: &S,
	idx: u16,
	k: &mut K,
) -> Result<(), ClipError> {
	let mut check = PictureSource {
		doc,
		s,
		idx,
		pos: 0,
	};
	bmp::read_info(&mut check).map_err(|_| ClipError::BadData)?;
	let mut at = FILE_HEADER;
	let mut buf = [0u8; 256];
	loop {
		let n = doc.picture_bytes(s, idx, at, &mut buf);
		if n == 0 {
			return Ok(());
		}
		if !k.write(buf.get(..n).unwrap_or(&[])) {
			return Err(ClipError::Refused);
		}
		at += n as u32;
	}
}

/// Reads a DIB into `out` (replacing it) as a document of one picture.
pub(crate) fn read<S: Store + ?Sized, R: Source>(
	r: &mut R,
	s: &mut S,
	out: &mut Doc,
) -> Result<(), ClipError> {
	let mut head = [0u8; 54];
	if r.read(&mut head) != head.len() {
		return Err(ClipError::BadData);
	}
	let le = |i: usize| {
		let mut x = [0u8; 4];
		if let Some(src) = head.get(i..i + 4) {
			x.copy_from_slice(src);
		}
		u32::from_le_bytes(x)
	};
	let hsize = le(0);
	if hsize < 40 {
		return Err(ClipError::BadData);
	}
	let (w, h) = (le(4) as i32, (le(8) as i32).unsigned_abs());
	let bpp = le(12) >> 16;
	let used = le(32);
	let colours = if bpp <= 8 && used == 0 {
		1u32 << bpp
	} else {
		used
	};
	let data_at = FILE_HEADER + hsize + colours.min(256) * 4;
	if w <= 0 || w > 8192 || h == 0 || h > 8192 {
		return Err(ClipError::BadData);
	}
	out.init(s)?;
	let text_w = (out.page.paper_w - out.page.margin_l - out.page.margin_r).max(1440);
	// The file header goes first as zeros and is filled in once the length is known.
	let mut staged = [0u8; FILE_HEADER as usize + 54];
	if let Some(dst) = staged.get_mut(FILE_HEADER as usize..) {
		dst.copy_from_slice(&head);
	}
	let mut sent = 0usize;
	let added = out.add_picture(s, 0, w as u16, h as u16, text_w, |buf| {
		if sent < staged.len() {
			let part = staged.get(sent..).unwrap_or(&[]);
			let n = part.len().min(buf.len());
			if let (Some(d), Some(sr)) = (buf.get_mut(..n), part.get(..n)) {
				d.copy_from_slice(sr);
			}
			sent += n;
			n
		} else {
			r.read(buf)
		}
	});
	let idx = match added {
		Ok(i) => i,
		Err(e) => {
			out.reset(s);
			return Err(e.into());
		}
	};
	let Some(p) = out.pictures.get(s, u32::from(idx)) else {
		out.reset(s);
		return Err(ClipError::BadData);
	};
	let (len_b, at_b) = (p.len.to_le_bytes(), data_at.to_le_bytes());
	let fh = [
		b'B', b'M', len_b[0], len_b[1], len_b[2], len_b[3], 0, 0, 0, 0, at_b[0], at_b[1], at_b[2],
		at_b[3],
	];
	out.pic_data.patch(s, p.offset, &fh);
	let ok = {
		let mut check = PictureSource {
			doc: out,
			s: &*s,
			idx,
			pos: 0,
		};
		bmp::read_info(&mut check).is_ok()
	};
	if !ok {
		out.reset(s);
		return Err(ClipError::BadData);
	}
	out.body.clear(s);
	out.body.append_object(s, u32::from(idx), 0, 0)?;
	Ok(())
}
