// Inserting a pasted fragment into a document: its formats merged into the target's tables.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use quill_core::doc::{CFmt, Doc, PFmt, MAX_PICTURES, NO_TABS};
use quill_core::store::Store;
use quill_core::text::SRC_OBJECT;

use crate::convert::copy_picture;
use crate::ClipError;

/// How many characters one insert takes at most, and so the stack a paste needs.
const STEP: u32 = 256;

/// Which formats the inserted characters take.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PasteStyle {
	/// The fragment's own character formats and paragraph formats, merged into the target's tables. `first_pfmt` is the paragraph
	/// format (already in the target's table) of the paragraph the paste starts in when it starts inside that paragraph: the first
	/// paragraph mark of the fragment ends that paragraph, so it keeps its look. `None` when the paste starts a paragraph.
	Rich {
		/// The target's paragraph format for the first mark, when the paste starts inside a paragraph.
		first_pfmt: Option<u16>,
	},
	/// Every character takes `cfmt` and every paragraph mark takes `pfmt`: what typing there would give (plain text).
	Plain {
		/// Character format of the target.
		cfmt: u16,
		/// Paragraph format of the target.
		pfmt: u16,
	},
}

/// Index in `doc` of what `src_cfmt` of `frag` names, adding the font, the colour and the format when `doc` lacks them. A table that
/// is full gives the default entry (index 0) in its place.
fn map_cfmt<S: Store + ?Sized>(doc: &mut Doc, s: &mut S, frag: &Doc, src: u16) -> u16 {
	let Some(f) = frag.cfmts.get(s, src) else {
		return 0;
	};
	let font = frag
		.fonts
		.get(s, f.font)
		.and_then(|v| doc.fonts.intern(s, v).ok())
		.unwrap_or(0);
	let colour = if f.colour == 0 {
		0
	} else {
		frag.colours
			.get(s, u16::from(f.colour))
			.and_then(|v| doc.colours.intern(s, v).ok())
			.and_then(|i| u8::try_from(i).ok())
			.unwrap_or(0)
	};
	doc.cfmts.intern(s, CFmt { font, colour, ..f }).unwrap_or(0)
}

/// As [`map_cfmt`] for a paragraph format and its tab set and list.
fn map_pfmt<S: Store + ?Sized>(doc: &mut Doc, s: &mut S, frag: &Doc, src: u16) -> u16 {
	let Some(p) = frag.pfmts.get(s, src) else {
		return 0;
	};
	let tabset = if p.tabset == NO_TABS {
		NO_TABS
	} else {
		frag.tabsets
			.get(s, p.tabset)
			.and_then(|v| doc.tabsets.intern(s, v).ok())
			.unwrap_or(NO_TABS)
	};
	let list = frag
		.lists
		.get(s, p.list)
		.and_then(|v| doc.lists.intern(s, v).ok())
		.unwrap_or(0);
	doc.pfmts.intern(s, PFmt { tabset, list, ..p }).unwrap_or(0)
}

/// Inserts the body of `frag` into the body of `doc` at `at`, in the formats `style` says, and counts what went in. Each insert
/// records its undo in the group the caller has open, so the paste is one step. Fonts, colours, formats, tab sets, lists and
/// pictures the fragment names are copied into `doc`.
///
/// `done` is the number of characters (a picture counts one) inserted so far, and is right when an error comes back: the characters
/// before the failure stay in the document.
#[inline(never)]
pub fn insert_fragment<S: Store + ?Sized>(
	doc: &mut Doc,
	s: &mut S,
	frag: &Doc,
	at: u32,
	style: PasteStyle,
	label: u8,
	done: &mut u32,
) -> Result<(), ClipError> {
	*done = 0;
	let end = frag.body.len();
	let mut src = 0u32;
	let mut pos = at;
	let mut first = matches!(
		style,
		PasteStyle::Rich {
			first_pfmt: Some(_)
		}
	);
	let first_pfmt = match style {
		PasteStyle::Rich { first_pfmt } => first_pfmt,
		PasteStyle::Plain { .. } => None,
	};
	// The last character format and paragraph format mapped, so a long run of one look is mapped once.
	let mut last_c: Option<(u16, u16)> = None;
	let mut last_p: Option<(u16, u16)> = None;
	let mut buf = [0u8; STEP as usize];
	while src < end {
		let mut run = None;
		frag.body
			.for_each_run(s, src, (end - src).min(STEP), |p, piece| {
				if run.is_none() {
					run = Some((p, piece));
				}
			});
		let Some((p, piece)) = run else {
			break;
		};
		let cfmt = match style {
			PasteStyle::Plain { cfmt, .. } => cfmt,
			PasteStyle::Rich { .. } => match last_c {
				Some((from, to)) if from == piece.cfmt => to,
				_ => {
					let to = map_cfmt(doc, s, frag, piece.cfmt);
					last_c = Some((piece.cfmt, to));
					to
				}
			},
		};
		let mut pfmt_for = |doc: &mut Doc, s: &mut S, first: bool| match style {
			PasteStyle::Plain { pfmt, .. } => pfmt,
			PasteStyle::Rich { .. } => match (first, first_pfmt) {
				(true, Some(f)) => f,
				_ => match last_p {
					Some((from, to)) if from == piece.pfmt => to,
					_ => {
						let to = map_pfmt(doc, s, frag, piece.pfmt);
						last_p = Some((piece.pfmt, to));
						to
					}
				},
			},
		};
		if piece.source == SRC_OBJECT {
			let v = piece.start;
			let index = if v >> 16 == 0 {
				if doc.pictures.len() >= MAX_PICTURES {
					return Err(ClipError::TooLarge);
				}
				copy_picture(frag, s, doc, v & 0xFFFF)?
			} else {
				v
			};
			let pfmt = pfmt_for(doc, s, first);
			doc.body.insert_object(s, pos, index, cfmt, pfmt, label)?;
			pos += 1;
			*done += 1;
		} else if let Some(part) = buf.get_mut(..piece.len as usize) {
			frag.body.read_piece(s, &piece, 0, part);
			let mut from = 0usize;
			while from < part.len() {
				let rest = part.get(from..).unwrap_or(&[]);
				let take = rest
					.iter()
					.position(|&b| b == 0x0D)
					.map_or(rest.len(), |i| i + 1);
				let seg = rest.get(..take).unwrap_or(&[]);
				let pfmt = pfmt_for(doc, s, first);
				doc.body.insert(s, pos, seg, cfmt, pfmt, label)?;
				if seg.last() == Some(&0x0D) {
					first = false;
				}
				pos += take as u32;
				*done += take as u32;
				from += take;
			}
		}
		src = p + piece.len.max(1);
	}
	Ok(())
}
