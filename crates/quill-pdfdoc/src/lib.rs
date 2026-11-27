// quill-pdfdoc: writes a laid-out Quill document as a PDF with a text layer, from the layout a shell's own measure gives.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The page is drawn the way a shell draws one for a printer (`draw_page` in the Win32 shell): the header and footer of the page,
// then each body line of it, with the same positions, list labels, fields, tab leaders, vertical offsets and underlines. The
// difference is where the characters go: into `quill-pdf` runs, in the standard font nearest the face, each at the layout's own
// position. Any `Measure` lays the document out (the Win32 shell passes GDI's at a printer-like resolution, the host tests pass
// `Std14Measure`), and the page follows what it says, so the PDF looks as the document was laid out for that measure.

#![no_std]
#![forbid(unsafe_code)]
#![deny(
	clippy::unwrap_used,
	clippy::expect_used,
	clippy::panic,
	clippy::indexing_slicing
)]

extern crate alloc;

use alloc::vec::Vec;

use quill_app::{line_label, App, ListCounters};
use quill_core::doc::{cflag, Doc, StoryKind};
use quill_core::layout::{
	font_ref, picture_size, to_units, walk_line, FontMetrics, FontRef, Line, Measure, GLYPHS,
};
use quill_core::store::ReadStore;
use quill_pdf::{Font, Info, Page, Pdf, Picture, Rgb, Run};

mod measure;

pub use measure::Std14Measure;

/// What to write.
#[derive(Copy, Clone, Debug)]
pub struct Options<'a> {
	/// The document's title, in Windows-1252.
	pub title: &'a [u8],
	/// The creation date as `YYYYMMDDHHMMSS`, or none to leave it out (so the bytes are repeatable).
	pub created: Option<&'a [u8]>,
	/// The first page to write (1-based) and the last; any range is clipped to the document.
	pub from: u32,
	/// The last page to write.
	pub to: u32,
	/// The resolution pictures are kept at, in pixels to the inch (a picture is never enlarged beyond the layout's resolution).
	pub image_dpi: u16,
}

impl Default for Options<'_> {
	fn default() -> Self {
		Options {
			title: b"",
			created: None,
			from: 1,
			to: u32::MAX,
			image_dpi: 150,
		}
	}
}

/// One body line's list label: its bytes (0x07 is the bullet), how many there are, and where it starts across.
type Label = Option<([u8; 16], usize, usize)>;

/// What drawing a line needs to know.
struct Ctx<'a, S: ReadStore + ?Sized, M: Measure + Clone> {
	doc: &'a Doc,
	s: &'a S,
	app: &'a App<M>,
	kind: StoryKind,
	dpi: u16,
	page: u32,
	pages: u32,
	image_dpi: u16,
}

fn colour_of<S: ReadStore + ?Sized>(doc: &Doc, s: &S, colour: u8) -> Rgb {
	if colour == 0 {
		return Rgb::BLACK;
	}
	doc.colours
		.get(s, u16::from(colour))
		.map_or(Rgb::BLACK, |c| Rgb(c.r, c.g, c.b))
}

fn pdf_font(fr: &FontRef) -> Font {
	let name = fr.face.get(..usize::from(fr.face_len)).unwrap_or(&[]);
	Font::pick(
		name,
		fr.family,
		fr.flags & cflag::BOLD != 0,
		fr.flags & cflag::ITALIC != 0,
	)
}

/// The layout's advance of each byte of `text` in `fr`.
fn advances_of<N: Measure>(measure: &mut N, fr: &FontRef, text: &[u8]) -> Vec<i32> {
	let mut widths = [0u16; GLYPHS];
	measure.widths(fr, &mut widths);
	text.iter()
		.map(|b| {
			b.checked_sub(32)
				.and_then(|i| widths.get(usize::from(i)))
				.copied()
				.map_or(0, i32::from)
		})
		.collect()
}

/// One run: text at a baseline, with its underline and strikeout.
#[allow(clippy::too_many_arguments)]
fn show<N: Measure>(
	page: &mut Page,
	measure: &mut N,
	fr: &FontRef,
	flags: u16,
	colour: Rgb,
	x: i32,
	baseline: i32,
	text: &[u8],
	advances: &[i32],
) {
	if text.is_empty() {
		return;
	}
	page.text(&Run {
		font: pdf_font(fr),
		size_half: fr.size,
		colour,
		x,
		baseline,
		text,
		advances,
	});
	if flags & (cflag::UNDERLINE | cflag::STRIKE) == 0 {
		return;
	}
	// An em in device units, from the size in half points.
	let (dpi, _) = measure.resolution();
	let em = (i32::from(fr.size) * i32::from(dpi) + 72) / 144;
	let thick = (em / 20).max(1);
	let width: i32 = advances.iter().sum();
	if flags & cflag::UNDERLINE != 0 {
		page.rect(colour, x, baseline + em / 10, width, thick);
	}
	if flags & cflag::STRIKE != 0 {
		page.rect(colour, x, baseline - em * 3 / 10, width, thick);
	}
}

/// Draws one laid-out line whose top is at `y_top` and whose left margin is at `tx`.
fn draw_line<S: ReadStore + ?Sized, M: Measure + Clone, N: Measure>(
	page: &mut Page,
	cx: &Ctx<'_, S, M>,
	measure: &mut N,
	l: &Line,
	label: Label,
	tx: i32,
	y_top: i32,
) {
	let (doc, s, dpi) = (cx.doc, cx.s, cx.dpi);
	let text = doc.story(cx.kind);
	let base = y_top + l.ascent;
	if let Some((mut lab, n, first)) = label {
		if lab.first() == Some(&0x07) {
			lab[0] = 0x95;
		}
		let cf = text.cfmt_at(s, l.start).unwrap_or(0);
		let (fr, _, cfmt) = font_ref(doc, s, cf);
		let bytes = lab.get(..n).unwrap_or(&[]);
		let adv = advances_of(measure, &fr, bytes);
		show(
			page,
			measure,
			&fr,
			0,
			colour_of(doc, s, cfmt.colour),
			tx + first as i32,
			base,
			bytes,
			&adv,
		);
	}
	// The pieces of the line, gathered first so the measure is free to be asked about widths while they are drawn.
	enum Piece {
		Run {
			cf: u16,
			x: i32,
			text: Vec<u8>,
			adv: Vec<i32>,
		},
		Picture {
			idx: u16,
			x: i32,
			w: i32,
			h: i32,
		},
		Field {
			kind: u32,
			cf: u16,
			x: i32,
		},
		Tab {
			cf: u16,
			x: i32,
			w: i32,
			leader: u8,
		},
	}
	let mut pieces: Vec<Piece> = Vec::new();
	walk_line(doc, s, measure, cx.kind, l, |p, c, x, w, leader, cf| {
		if c == 0x01 {
			if let Some((pw, ph)) = picture_size(doc, s, text, p, dpi, dpi) {
				let idx = text.object_at(s, p).map_or(0, |v| v & 0xFFFF) as u16;
				pieces.push(Piece::Picture {
					idx,
					x,
					w: pw,
					h: ph,
				});
				return;
			}
			if let Some(kind) = text
				.object_at(s, p)
				.map(|v| v >> 16)
				.filter(|k| (1..=5).contains(k))
			{
				pieces.push(Piece::Field { kind, cf, x });
				return;
			}
		}
		if c == 0x09 || c == 0x01 {
			if c == 0x09 && w > 0 {
				pieces.push(Piece::Tab { cf, x, w, leader });
			}
			return;
		}
		if let Some(Piece::Run {
			cf: last,
			text,
			adv,
			..
		}) = pieces.last_mut()
		{
			if *last == cf {
				text.push(c);
				adv.push(w);
				return;
			}
		}
		pieces.push(Piece::Run {
			cf,
			x,
			text: alloc::vec![c],
			adv: alloc::vec![w],
		});
	});
	for piece in pieces {
		match piece {
			Piece::Run {
				cf,
				x,
				text: bytes,
				adv,
			} => {
				let (fr, vpos, cfmt) = font_ref(doc, s, cf);
				let shift = match vpos {
					1 => -(l.ascent / 3),
					2 => l.ascent / 4,
					_ => 0,
				};
				show(
					page,
					measure,
					&fr,
					cfmt.flags,
					colour_of(doc, s, cfmt.colour),
					tx + x,
					base + shift,
					&bytes,
					&adv,
				);
			}
			Piece::Picture { idx, x, w, h } => {
				draw_picture(page, cx, idx, tx + x, base - h, w, h);
			}
			Piece::Field { kind, cf, x } => {
				let mut buf = [0u8; 32];
				let n = cx.app.field_text(kind, cx.page, cx.pages, &mut buf);
				let bytes = buf.get(..n).unwrap_or(&[]);
				let (fr, _, cfmt) = font_ref(doc, s, cf);
				let adv = advances_of(measure, &fr, bytes);
				show(
					page,
					measure,
					&fr,
					0,
					colour_of(doc, s, cfmt.colour),
					tx + x,
					base,
					bytes,
					&adv,
				);
			}
			Piece::Tab { cf, x, w, leader } => {
				let ch = match leader {
					1 => b'.',
					2 => b'-',
					3 => b'_',
					_ => continue,
				};
				let (fr, _, cfmt) = font_ref(doc, s, cf);
				let step = advances_of(measure, &fr, &[ch])
					.first()
					.copied()
					.unwrap_or(0)
					.max(1);
				let count = (w / step).max(0) as usize;
				let bytes = alloc::vec![ch; count];
				let adv = alloc::vec![step; count];
				show(
					page,
					measure,
					&fr,
					0,
					colour_of(doc, s, cfmt.colour),
					tx + x,
					base,
					&bytes,
					&adv,
				);
			}
		}
	}
}

/// Draws picture `idx` into its box, at the resolution the options keep pictures at.
fn draw_picture<S: ReadStore + ?Sized, M: Measure + Clone>(
	page: &mut Page,
	cx: &Ctx<'_, S, M>,
	idx: u16,
	x: i32,
	y: i32,
	w: i32,
	h: i32,
) {
	let keep = i64::from(cx.image_dpi.max(1)).min(i64::from(cx.dpi));
	let px_w = ((i64::from(w) * keep / i64::from(cx.dpi)).max(1) as usize)
		.min(quill_image::scale::MAX_OUT);
	let px_h = ((i64::from(h) * px_w as i64 / i64::from(w.max(1))).max(1)) as u32;
	let mut rgb: Vec<u8> = Vec::new();
	if rgb.try_reserve_exact(px_w * px_h as usize * 3).is_err() {
		return;
	}
	// Grey where the picture does not decode, as the screen shows it.
	rgb.resize(px_w * px_h as usize * 3, 0xC0);
	let mut window = alloc::boxed::Box::new([0u8; quill_picture::WINDOW]);
	let _ = quill_picture::render(cx.doc, cx.s, idx, px_w, px_h, &mut window, |row, line| {
		// Row 0 is the bottom of the picture; a PDF image's first row is the top.
		let at = (px_h - 1 - row.min(px_h - 1)) as usize * px_w * 3;
		if let (Some(dst), Some(src)) = (rgb.get_mut(at..at + px_w * 3), line.get(..px_w * 3)) {
			dst.copy_from_slice(src);
		}
	});
	page.image(&Picture {
		x,
		y,
		w,
		h,
		px_w: px_w as u32,
		px_h,
		rgb: &rgb,
	});
}

/// Writes the document as a PDF. `measure` lays it out (its resolution is the page's unit) and `app` supplies what the layout does
/// not: list labels, the date and the file name for fields.
pub fn export<S: ReadStore + ?Sized, M: Measure + Clone, N: Measure>(
	app: &App<M>,
	s: &S,
	measure: &mut N,
	opts: &Options<'_>,
) -> Vec<u8> {
	let doc = app.doc();
	let (dpi, _) = measure.resolution();
	let mut lines: Vec<Line> = Vec::new();
	quill_core::layout::layout_story(doc, s, measure, StoryKind::Body, |l| lines.push(*l));
	let pages = lines.iter().map(|l| l.page).max().unwrap_or(1);
	// List labels count through the whole document, so they are worked out once, in order.
	let mut counters: ListCounters = [[0; 9]; 4];
	let labels: Vec<Label> = lines
		.iter()
		.map(|l| line_label(app, s, l, &mut counters, dpi))
		.collect();
	let setup = doc.page;
	let u = |tw: i32| to_units(tw, dpi);
	let (paper_w, paper_h) = (u(setup.paper_w), u(setup.paper_h));
	let mut pdf = Pdf::new(&Info {
		title: opts.title,
		created: opts.created,
	});
	for p in opts.from.max(1)..=opts.to.min(pages) {
		let mut page = Page::new(paper_w, paper_h, dpi);
		let first = p == 1 && setup.flags & 1 != 0;
		let tx = u(setup.margin_l);
		for (kind, from_bottom, dist) in [
			(
				if first {
					StoryKind::FirstHeader
				} else {
					StoryKind::Header
				},
				false,
				setup.header_dist,
			),
			(
				if first {
					StoryKind::FirstFooter
				} else {
					StoryKind::Footer
				},
				true,
				setup.footer_dist,
			),
		] {
			if doc.story(kind).is_empty() {
				continue;
			}
			let mut hl: Vec<Line> = Vec::new();
			quill_core::layout::layout_story(doc, s, measure, kind, |l| {
				if l.page == 1 {
					hl.push(*l);
				}
			});
			let height = hl.last().map_or(0, |l| l.y + l.ascent + l.descent);
			let top = if from_bottom {
				paper_h - u(dist) - height
			} else {
				u(dist)
			};
			let mut counters: ListCounters = [[0; 9]; 4];
			let cx = Ctx {
				doc,
				s,
				app,
				kind,
				dpi,
				page: p,
				pages,
				image_dpi: opts.image_dpi,
			};
			for l in &hl {
				let label = line_label(app, s, l, &mut counters, dpi);
				draw_line(&mut page, &cx, measure, l, label, tx, top + l.y);
			}
		}
		let cx = Ctx {
			doc,
			s,
			app,
			kind: StoryKind::Body,
			dpi,
			page: p,
			pages,
			image_dpi: opts.image_dpi,
		};
		for (l, label) in lines.iter().zip(labels.iter()) {
			if l.page == p {
				draw_line(
					&mut page,
					&cx,
					measure,
					l,
					*label,
					tx,
					u(setup.margin_t) + l.y,
				);
			}
		}
		pdf.add_page(page);
	}
	pdf.finish()
}

/// The vertical metrics a standard font has at a size, for measures that have no font files to ask.
pub fn std14_metrics(size_half: u16, dpi: u16) -> FontMetrics {
	let em = (i32::from(size_half) * i32::from(dpi) + 72) / 144;
	FontMetrics {
		ascent: (em * 9 + 5) / 10,
		descent: (em * 22 + 50) / 100,
		leading: 0,
	}
}
