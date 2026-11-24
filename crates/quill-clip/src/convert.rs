// Copy and paste: a selection to the flavours a clipboard is asked for, and the best flavour on offer back into a fragment.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::ops::Range;

use quill_core::doc::{Doc, Picture};
use quill_core::store::Store;
use quill_core::text::SRC_OBJECT;
use quill_formats::io::Source;
use quill_formats::{rtf, text};

use crate::{ClipError, ClipWrite, Clipboard, Flavour, Flavours};

/// How many characters one piece of the selection is copied in. Runs longer than this are copied as several pieces of one format,
/// which the text merges again.
const STEP: u32 = 256;

/// Copies `range` of the body of `doc` to `cb` in each flavour of `want` that suits it. The clipboard is emptied first.
///
/// The selection is copied into a scratch document in the same store, with the format tables of `doc`, and that document is written
/// with the RTF and text writers, so the clipboard gets exactly what a file save would write for those characters. A selection that
/// ends inside a paragraph is written without the paragraph's end, so a paste puts it back inline. The scratch document is freed
/// before this returns.
///
/// Succeeds when at least one flavour was committed; otherwise returns the first failure.
pub fn copy<S: Store + ?Sized, C: Clipboard>(
	doc: &Doc,
	s: &mut S,
	range: Range<u32>,
	want: Flavours,
	cb: &mut C,
) -> Result<(), ClipError> {
	if range.start >= range.end || range.end > doc.body.len() {
		return Err(ClipError::Empty);
	}
	if !cb.clear() {
		return Err(ClipError::Refused);
	}
	let mut scratch = Doc::new();
	let built = build(doc, s, &range, &mut scratch);
	let result = match built {
		Ok(chars) => put_all(doc, s, &range, &scratch, chars, want, cb),
		Err(e) => Err(e),
	};
	scratch.reset(s);
	result
}

fn put_all<S: Store + ?Sized, C: Clipboard>(
	doc: &Doc,
	s: &S,
	range: &Range<u32>,
	scratch: &Doc,
	chars: u32,
	want: Flavours,
	cb: &mut C,
) -> Result<(), ClipError> {
	let mut first_err = None;
	let mut done = 0u32;
	let mut note = |r: Result<(), ClipError>| match r {
		Ok(()) => done += 1,
		Err(e) => {
			first_err.get_or_insert(e);
		}
	};
	if want.has(Flavour::Rtf) {
		note(write_with(cb.put(Flavour::Rtf), |k| {
			rtf::write(scratch, s, k).map_err(ClipError::from)
		}));
	}
	if want.has(Flavour::Text1252) && chars > 0 {
		note(write_with(cb.put(Flavour::Text1252), |k| {
			text::write(scratch, s, k).map_err(ClipError::from)
		}));
	}
	#[cfg(feature = "dib")]
	if want.has(Flavour::Dib) {
		if let Some(idx) = crate::dib::lone_bmp(doc, s, range) {
			note(write_with(cb.put(Flavour::Dib), |k| {
				crate::dib::write(doc, s, idx, k)
			}));
		}
	}
	#[cfg(not(feature = "dib"))]
	let _ = (doc, range);
	match (done, first_err) {
		(0, Some(e)) => Err(e),
		(0, None) => Err(ClipError::Empty),
		_ => Ok(()),
	}
}

/// Runs `fill` on a writer and commits it, or abandons the writer when anything fails.
fn write_with<W: ClipWrite>(
	w: Option<W>,
	fill: impl FnOnce(&mut W) -> Result<(), ClipError>,
) -> Result<(), ClipError> {
	let mut w = w.ok_or(ClipError::Refused)?;
	fill(&mut w)?;
	if w.commit() {
		Ok(())
	} else {
		Err(ClipError::Refused)
	}
}

/// Fills `scratch` with the selection and returns how many characters (not pictures) it holds.
fn build<S: Store + ?Sized>(
	doc: &Doc,
	s: &mut S,
	range: &Range<u32>,
	scratch: &mut Doc,
) -> Result<u32, ClipError> {
	scratch.page = doc.page;
	macro_rules! tables {
		($($t:ident),*) => {$(
			for i in 0..doc.$t.len() {
				if let Some(v) = doc.$t.get(s, i as u16) {
					scratch.$t.intern(s, v)?;
				}
			}
		)*};
	}
	tables!(fonts, colours, cfmts, pfmts, tabsets, lists);
	let mut buf = [0u8; STEP as usize];
	let mut chars = 0u32;
	let mut at = range.start;
	while at < range.end {
		let mut run = None;
		doc.body
			.for_each_run(s, at, (range.end - at).min(STEP), |p, piece| {
				if run.is_none() {
					run = Some((p, piece));
				}
			});
		let Some((pos, piece)) = run else {
			break;
		};
		if piece.source == SRC_OBJECT {
			let v = piece.start;
			let index = if v >> 16 == 0 {
				copy_picture(doc, s, scratch, v & 0xFFFF)?
			} else {
				v
			};
			scratch
				.body
				.append_object(s, index, piece.cfmt, piece.pfmt)?;
		} else if let Some(part) = buf.get_mut(..piece.len as usize) {
			doc.body.read_piece(s, &piece, 0, part);
			scratch
				.body
				.append_original(s, part, piece.cfmt, piece.pfmt)?;
			chars += piece.len;
		}
		at = pos + piece.len.max(1);
	}
	Ok(chars)
}

/// Copies picture `idx` of `doc` into `scratch` and returns its new index.
fn copy_picture<S: Store + ?Sized>(
	doc: &Doc,
	s: &mut S,
	scratch: &mut Doc,
	idx: u32,
) -> Result<u32, ClipError> {
	let p = doc.pictures.get(s, idx).ok_or(ClipError::BadData)?;
	scratch.pictures.reserve(s, 1)?;
	let offset = scratch.pic_data.len();
	let mut buf = [0u8; 512];
	let mut pos = 0u32;
	while pos < p.len {
		let n = doc.picture_bytes(s, idx as u16, pos, &mut buf);
		if n == 0 {
			break;
		}
		if let Err(e) = scratch.pic_data.append(s, buf.get(..n).unwrap_or(&[])) {
			scratch.pic_data.truncate(s, offset);
			return Err(e.into());
		}
		pos += n as u32;
	}
	let at = scratch.pictures.len();
	scratch.pictures.push(
		s,
		Picture {
			offset,
			len: pos,
			..p
		},
	)?;
	Ok(at)
}

/// Reads the first of `prefer` that the clipboard offers and can be read into the fragment `out` (replacing it), and returns which
/// one it was. RTF and text fragments carry the paragraph end of their last paragraph only when the source did, so a selection
/// copied from inside a paragraph pastes inline. A picture fragment is one picture and no text.
///
/// The fragment is a document of its own: its format tables are the clipboard's, and the caller merges them into the target
/// document when it inserts the fragment. On an error `out` is empty.
pub fn paste_fragment<S: Store + ?Sized, C: Clipboard>(
	cb: &mut C,
	prefer: &[Flavour],
	s: &mut S,
	out: &mut Doc,
) -> Result<Flavour, ClipError> {
	let offers = cb.offers();
	let mut err = ClipError::Empty;
	for &f in prefer {
		if !offers.has(f) {
			continue;
		}
		let Some(mut r) = cb.get(f) else {
			continue;
		};
		let got = match f {
			Flavour::Rtf => read_rtf(&mut r, s, out),
			Flavour::Text1252 => read_text(&mut r, s, out),
			#[cfg(feature = "dib")]
			Flavour::Dib => crate::dib::read(&mut r, s, out),
			#[cfg(not(feature = "dib"))]
			Flavour::Dib => continue,
		};
		match got {
			Ok(()) => return Ok(f),
			Err(e) => {
				out.reset(s);
				err = e;
			}
		}
	}
	Err(err)
}

/// A source that remembers the end of what it passed on.
struct Tail<'a, R: Source> {
	r: &'a mut R,
	tail: [u8; 32],
}

impl<R: Source> Source for Tail<'_, R> {
	fn read(&mut self, out: &mut [u8]) -> usize {
		let n = self.r.read(out);
		let keep = n.min(self.tail.len());
		self.tail.rotate_left(keep);
		let at = self.tail.len() - keep;
		if let (Some(dst), Some(src)) = (self.tail.get_mut(at..), out.get(n - keep..n)) {
			dst.copy_from_slice(src);
		}
		n
	}
}

fn read_rtf<S: Store + ?Sized, R: Source>(
	r: &mut R,
	s: &mut S,
	out: &mut Doc,
) -> Result<(), ClipError> {
	let mut t = Tail { r, tail: [0; 32] };
	rtf::read(out, s, &mut t)?;
	let trimmed = t
		.tail
		.iter()
		.rposition(|b| !matches!(b, b'}' | b'\r' | b'\n' | b' ' | b'\t' | 0))
		.map_or(&[][..], |i| t.tail.get(..=i).unwrap_or(&[]));
	finish(s, out, trimmed.ends_with(b"\\par"))
}

fn read_text<S: Store + ?Sized, R: Source>(
	r: &mut R,
	s: &mut S,
	out: &mut Doc,
) -> Result<(), ClipError> {
	let mut t = Tail { r, tail: [0; 32] };
	text::read(out, s, &mut t)?;
	let ended = matches!(t.tail.last(), Some(0x0D | 0x0A | 0x0C));
	finish(s, out, ended)
}

/// The readers add a final paragraph mark when the source had none. A fragment copied from inside a paragraph must not have one.
fn finish<S: Store + ?Sized>(s: &mut S, out: &mut Doc, ended: bool) -> Result<(), ClipError> {
	let len = out.body.len();
	if !ended && len > 0 && out.body.read_byte(s, len - 1) == Some(0x0D) {
		out.body.delete(s, len - 1, 1, 0)?;
		out.body.forget_history(s);
	}
	if out.body.is_empty() {
		out.reset(s);
		return Err(ClipError::Empty);
	}
	Ok(())
}
