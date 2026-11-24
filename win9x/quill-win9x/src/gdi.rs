// The GDI page: the Measure that asks Windows for font metrics, and the painter that draws the laid-out document with real fonts.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::mem::MaybeUninit;
use core::ptr::null_mut;

use quill_app::{line_label, App, ListCounters};
use quill_core::doc::{cflag, CFmt, Doc, StoryKind};
use quill_core::layout::{font_ref, to_units, FontMetrics, FontRef, Measure, GLYPHS};
use quill_heap::HeapStore;

use crate::sys::*;
use alloc::vec::Vec;

/// The measure: the screen at 96 dots per inch, or (for the golden-frame tests) the DOS text grid.
#[derive(Copy, Clone)]
pub struct GdiMeasure {
	/// A screen device context to ask.
	pub dc: Handle,
	/// Measure like the DOS text screen instead (one column per character, 10 by 6 units per inch).
	pub grid: bool,
	/// Dots per inch of the device (96 for the screen, the printer's for printing).
	pub dpi: u16,
}

static mut FONTS: [(Option<(FontRef, u8, u16)>, Handle); 12] = [(None, null_mut()); 12];
static mut NEXT_FONT: usize = 0;

/// The GDI font for `fr` with the extra `cflag` bits for underline and strikeout, cached in a small ring.
pub fn font_handle(fr: &FontRef, extra: u8) -> Handle {
	font_handle_at(fr, extra, 96)
}

/// The same at `dpi` dots per inch.
pub fn font_handle_at(fr: &FontRef, extra: u8, dpi: u16) -> Handle {
	// SAFETY: single-threaded; the cache is only touched here.
	unsafe {
		let cache = &mut *core::ptr::addr_of_mut!(FONTS);
		for (k, h) in cache.iter() {
			if let Some((f, e, d)) = k {
				if f == fr && *e == extra && *d == dpi {
					return *h;
				}
			}
		}
		let mut face = [0u8; 32];
		for (d, s) in face.iter_mut().zip(fr.face.iter().take(usize::from(fr.face_len))) {
			*d = *s;
		}
		let height = -((i32::from(fr.size) * i32::from(dpi) + 72) / 144);
		let weight = if fr.flags & cflag::BOLD != 0 { FW_BOLD } else { FW_NORMAL };
		let italic = u32::from(fr.flags & cflag::ITALIC != 0);
		let pitch_family = match fr.family {
			1 => 0x10 | 2,
			2 => 0x20 | 2,
			3 => 0x30 | 1,
			_ => 0,
		};
		let h = CreateFontA(
			height,
			0,
			0,
			0,
			weight,
			italic,
			u32::from(extra & 1),
			u32::from(extra & 2 != 0),
			0,
			0,
			0,
			0,
			pitch_family,
			face.as_ptr(),
		);
		let slot = NEXT_FONT % cache.len();
		NEXT_FONT += 1;
		if let Some((old, handle)) = cache.get_mut(slot) {
			if old.is_some() && !handle.is_null() {
				DeleteObject(*handle);
			}
			*old = Some((*fr, extra, dpi));
			*handle = h;
		}
		h
	}
}

impl Measure for GdiMeasure {
	fn resolution(&self) -> (u16, u16) {
		if self.grid {
			(10, 6)
		} else {
			(self.dpi, self.dpi)
		}
	}

	fn metrics(&mut self, font: &FontRef) -> FontMetrics {
		if self.grid {
			return FontMetrics {
				ascent: 1,
				descent: 0,
				leading: 0,
			};
		}
		let mut tm = MaybeUninit::<TextMetricA>::zeroed();
		// SAFETY: the DC is the screen's; the font handle is ours; the structure is the size GDI expects.
		unsafe {
			let old = SelectObject(self.dc, font_handle_at(font, 0, self.dpi));
			GetTextMetricsA(self.dc, tm.as_mut_ptr());
			SelectObject(self.dc, old);
			let tm = tm.assume_init();
			FontMetrics {
				ascent: tm.ascent,
				descent: tm.descent,
				leading: tm.external_leading,
			}
		}
	}

	fn widths(&mut self, font: &FontRef, out: &mut [u16; GLYPHS]) {
		if self.grid {
			out.fill(1);
			return;
		}
		let mut w = [0i32; GLYPHS];
		// SAFETY: `w` holds the 224 widths for characters 32 to 255.
		unsafe {
			let old = SelectObject(self.dc, font_handle_at(font, 0, self.dpi));
			GetCharWidthA(self.dc, 32, 255, w.as_mut_ptr());
			SelectObject(self.dc, old);
		}
		for (o, v) in out.iter_mut().zip(w.iter()) {
			*o = (*v).clamp(0, 4000) as u16;
		}
	}
}

const PAGE_GAP: i32 = 10;
const PAD: i32 = 8;

fn colour_of(app: &App<GdiMeasure>, s: &HeapStore, cf: &CFmt) -> u32 {
	if cf.colour == 0 {
		return 0;
	}
	app.doc()
		.colours
		.get(s, u16::from(cf.colour))
		.map_or(0, |c| u32::from(c.r) | (u32::from(c.g) << 8) | (u32::from(c.b) << 16))
}

/// One picture rendered at the size it is shown: 24-bit rows, bottom row first, each padded to four bytes.
struct Rendered {
	idx: u16,
	fp: u32,
	w: i32,
	h: i32,
	bits: Vec<u8>,
}

/// The pictures drawn lately, so scrolling does not decode them again. Holds at most three.
pub struct PicCache {
	items: Vec<Rendered>,
	window: Option<alloc::boxed::Box<[u8; quill_app::picture::WINDOW]>>,
}

impl PicCache {
	pub const fn new() -> Self {
		PicCache {
			items: Vec::new(),
			window: None,
		}
	}

	fn get(&mut self, app: &App<GdiMeasure>, s: &HeapStore, idx: u16, w: i32, h: i32) -> &Rendered {
		let fp = quill_app::picture::fingerprint(app.doc(), s, idx);
		if let Some(i) = self
			.items
			.iter()
			.position(|r| r.idx == idx && r.fp == fp && r.w == w && r.h == h)
		{
			return &self.items[i];
		}
		if self.items.len() >= 3 {
			self.items.remove(0);
		}
		let stride = ((w as usize * 3) + 3) & !3;
		// Short of memory, or a picture that does not decode: no bits, and the page shows a grey box in its place.
		let mut bits: Vec<u8> = Vec::new();
		if bits.try_reserve_exact(stride * h as usize).is_ok() {
			bits.resize(stride * h as usize, 0xC0);
			let window = self
				.window
				.get_or_insert_with(|| alloc::boxed::Box::new([0u8; quill_app::picture::WINDOW]));
			let done = quill_app::picture::render(app.doc(), s, idx, w as usize, h as u32, window, |row, rgb| {
				let at = row as usize * stride;
				if let Some(dst) = bits.get_mut(at..at + w as usize * 3) {
					for (d, p) in dst.chunks_exact_mut(3).zip(rgb.chunks_exact(3)) {
						d[0] = p[2];
						d[1] = p[1];
						d[2] = p[0];
					}
				}
			});
			if done.is_err() {
				bits.clear();
			}
		}
		self.items.push(Rendered { idx, fp, w, h, bits });
		&self.items[self.items.len() - 1]
	}
}

/// Draws a picture with its top left at (`x`, `y`).
///
/// # Safety
/// `dc` must be a valid device context.
unsafe fn draw_picture(
	dc: Handle,
	cache: &mut PicCache,
	app: &App<GdiMeasure>,
	s: &HeapStore,
	idx: u16,
	x: i32,
	y: i32,
	w: i32,
	h: i32,
) {
	let w = w.clamp(1, quill_image::scale::MAX_OUT as i32);
	let r = cache.get(app, s, idx, w, h);
	if r.bits.is_empty() {
		let area = Rect {
			left: x,
			top: y,
			right: x + w,
			bottom: y + h,
		};
		let b = CreateSolidBrush(0x00C0_C0C0);
		FillRect(dc, &area, b);
		DeleteObject(b);
		return;
	}
	let info = BitmapInfoHeader {
		size: 40,
		width: w,
		height: h,
		planes: 1,
		bit_count: 24,
		compression: 0,
		size_image: 0,
		x_pels: 0,
		y_pels: 0,
		clr_used: 0,
		clr_important: 0,
	};
	SetDIBitsToDevice(dc, x, y, w as u32, h as u32, 0, 0, 0, h as u32, r.bits.as_ptr(), &info, 0);
}

/// Draws a picture on the printer: decoded again at up to 1,024 pixels across in bands of 48 rows, each stretched to its place,
/// so the whole picture is never in memory at once.
///
/// # Safety
/// `dc` must be a valid device context.
#[allow(clippy::too_many_arguments)]
unsafe fn draw_picture_banded(
	dc: Handle,
	cache: &mut PicCache,
	doc: &Doc,
	s: &HeapStore,
	idx: u16,
	x: i32,
	y: i32,
	w: i32,
	h: i32,
) {
	const BAND: i32 = 48;
	let rw = w.clamp(1, quill_image::scale::MAX_OUT as i32);
	let rh = ((i64::from(h) * i64::from(rw)) / i64::from(w.max(1))).max(1) as i32;
	let stride = ((rw as usize * 3) + 3) & !3;
	let mut band: Vec<u8> = Vec::new();
	if band.try_reserve_exact(stride * BAND as usize).is_err() {
		return;
	}
	band.resize(stride * BAND as usize, 0xFF);
	let window = cache
		.window
		.get_or_insert_with(|| alloc::boxed::Box::new([0u8; quill_app::picture::WINDOW]));
	let mut t0 = 0;
	while t0 < rh {
		let t1 = (t0 + BAND).min(rh);
		let rows = (t1 - t0) as u32;
		band.fill(0xFF);
		// DIB row 0 is the bottom of the picture, so this band holds DIB rows rh - t1 up to rh - t0.
		let low = (rh - t1) as u32;
		let _ = quill_app::picture::render(doc, s, idx, rw as usize, rh as u32, window, |row, rgb| {
			if row >= low && row < low + rows {
				let at = (row - low) as usize * stride;
				if let Some(dst) = band.get_mut(at..at + rw as usize * 3) {
					for (d, p) in dst.chunks_exact_mut(3).zip(rgb.chunks_exact(3)) {
						d[0] = p[2];
						d[1] = p[1];
						d[2] = p[0];
					}
				}
			}
		});
		let info = BitmapInfoHeader {
			size: 40,
			width: rw,
			height: rows as i32,
			planes: 1,
			bit_count: 24,
			compression: 0,
			size_image: 0,
			x_pels: 0,
			y_pels: 0,
			clr_used: 0,
			clr_important: 0,
		};
		let (dy0, dy1) = (
			y + (i64::from(t0) * i64::from(h) / i64::from(rh)) as i32,
			y + (i64::from(t1) * i64::from(h) / i64::from(rh)) as i32,
		);
		StretchDIBits(
			dc,
			x,
			dy0,
			w,
			(dy1 - dy0).max(1),
			0,
			0,
			rw,
			rows as i32,
			band.as_ptr(),
			&info,
			0,
			0x00CC_0020,
		);
		t0 = t1;
	}
}

/// What drawing a line needs to know about the document and the device.
struct LineCtx<'a> {
	doc: &'a Doc,
	s: &'a HeapStore,
	app: &'a App<GdiMeasure>,
	kind: StoryKind,
	/// Dots per inch of the device the line is drawn on.
	dpi: u16,
	/// The selection to highlight, if any.
	sel: Option<(u32, u32)>,
	/// A device context to ask for character widths (the screen's, or the printer's).
	mdc: Handle,
	/// The page the line is on, and the number of pages when fields should show their values.
	page: u32,
	fields: Option<u32>,
}

/// How pictures reach the device: from a cache of renderings (the screen) or in bands (the printer).
enum Pics<'a> {
	Cache(&'a mut PicCache),
	Banded(&'a mut PicCache),
}

/// Draws one laid-out line whose top is at `y_top` and whose left margin is at `tx`.
///
/// # Safety
/// `dc` must be a valid device context.
unsafe fn draw_line(
	dc: Handle,
	cx: &LineCtx<'_>,
	pics: &mut Pics<'_>,
	l: &quill_core::layout::Line,
	label: Option<([u8; 16], usize, usize)>,
	tx: i32,
	y_top: i32,
) {
	let (doc, s, app, sel, dpi) = (cx.doc, cx.s, cx.app, cx.sel, cx.dpi);
	let text = doc.story(cx.kind);
	let fhandle = |fr: &FontRef, extra: u8| font_handle_at(fr, extra, dpi);
	let base = y_top + l.ascent;
	if let Some((mut lab, n, first)) = label {
		if lab.first() == Some(&0x07) {
			lab[0] = 0x95;
		}
		let cf = text.cfmt_at(s, l.start).unwrap_or(0);
		let (fr, _, cfmt) = font_ref(doc, s, cf);
		let old = SelectObject(dc, fhandle(&fr, 0));
		SetBkMode(dc, TRANSPARENT);
		SetTextColor(dc, colour_of(app, s, &cfmt));
		TextOutA(dc, tx + first as i32, base, lab.as_ptr(), n as i32);
		SelectObject(dc, old);
	}
	// Runs of one format and selection state, split at tabs and objects.
	let mut run = [0u8; 200];
	let mut n = 0usize;
	let mut run_x = 0;
	let mut run_cf = 0u16;
	let mut run_sel = false;
	let flush = |n: &mut usize, run: &[u8; 200], x: i32, cf: u16, selected: bool| {
		if *n == 0 {
			return;
		}
		let (fr, vpos, cfmt) = font_ref(doc, s, cf);
		let extra = u8::from(cfmt.flags & cflag::UNDERLINE != 0) | (u8::from(cfmt.flags & cflag::STRIKE != 0) << 1);
		let old = SelectObject(dc, fhandle(&fr, extra));
		let shift = match vpos {
			1 => -(l.ascent / 3),
			2 => l.ascent / 4,
			_ => 0,
		};
		if selected {
			SetBkMode(dc, OPAQUE);
			SetBkColor(dc, 0x0080_0000);
			SetTextColor(dc, 0x00FF_FFFF);
		} else {
			SetBkMode(dc, TRANSPARENT);
			SetTextColor(dc, colour_of(app, s, &cfmt));
		}
		TextOutA(dc, tx + x, base + shift, run.as_ptr(), *n as i32);
		SelectObject(dc, old);
		*n = 0;
	};
	let mut m = GdiMeasure {
		dc: cx.mdc,
		grid: false,
		dpi,
	};
	quill_core::layout::walk_line(doc, s, &mut m, cx.kind, l, |p, c, x, w, leader, cf| {
		let selected = sel.is_some_and(|(lo, hi)| p >= lo && p < hi);
		if c == 0x01 {
			flush(&mut n, &run, run_x, run_cf, run_sel);
			if let Some((pw, ph)) = quill_core::layout::picture_size(doc, s, text, p, dpi, dpi) {
				let idx = text.object_at(s, p).map_or(0, |v| v & 0xFFFF) as u16;
				match pics {
					Pics::Cache(cache) => draw_picture(dc, cache, app, s, idx, tx + x, base - ph, pw, ph),
					Pics::Banded(cache) => draw_picture_banded(dc, cache, doc, s, idx, tx + x, base - ph, pw, ph),
				}
				return;
			}
			if let (Some(fv), Some(v)) = (cx.fields, text.object_at(s, p)) {
				let mut buf = [0u8; 32];
				let n = app.field_text(v >> 16, cx.page, fv, &mut buf);
				if n > 0 {
					let (fr, _, cfmt) = font_ref(doc, s, cf);
					let old = SelectObject(dc, fhandle(&fr, 0));
					SetBkMode(dc, TRANSPARENT);
					SetTextColor(dc, colour_of(app, s, &cfmt));
					TextOutA(dc, tx + x, base, buf.as_ptr(), n as i32);
					SelectObject(dc, old);
				}
				return;
			}
		}
		if c == 0x09 || c == 0x01 {
			flush(&mut n, &run, run_x, run_cf, run_sel);
			if c == 0x09 && w > 0 {
				let (fr, _, cfmt) = font_ref(doc, s, cf);
				let old = SelectObject(dc, fhandle(&fr, 0));
				let ch = [match leader {
					1 => b'.',
					2 => b'-',
					3 => b'_',
					_ => b' ',
				}];
				if selected {
					SetBkMode(dc, OPAQUE);
					SetBkColor(dc, 0x0080_0000);
				} else {
					SetBkMode(dc, TRANSPARENT);
				}
				SetTextColor(dc, colour_of(app, s, &cfmt));
				if leader != 0 {
					let mut dx = 0;
					let mut cw = [0i32; 1];
					GetCharWidthA(dc, u32::from(ch[0]), u32::from(ch[0]), cw.as_mut_ptr());
					let step = cw[0].max(1);
					while dx + step <= w {
						TextOutA(dc, tx + x + dx, base, ch.as_ptr(), 1);
						dx += step;
					}
				} else if selected {
					let r = Rect {
						left: tx + x,
						top: y_top,
						right: tx + x + w,
						bottom: y_top + l.ascent + l.descent,
					};
					let b = CreateSolidBrush(0x0080_0000);
					FillRect(dc, &r, b);
					DeleteObject(b);
				}
				SelectObject(dc, old);
			}
			return;
		}
		if n > 0 && (cf != run_cf || selected != run_sel || n == run.len()) {
			flush(&mut n, &run, run_x, run_cf, run_sel);
		}
		if n == 0 {
			run_x = x;
			run_cf = cf;
			run_sel = selected;
		}
		if let Some(slot) = run.get_mut(n) {
			*slot = c;
			n += 1;
		}
	});
	flush(&mut n, &run, run_x, run_cf, run_sel);
}

/// Paints the page view into the client rectangle `area` and returns how many lines fit.
///
/// # Safety
/// `dc` must be a valid device context.
pub unsafe fn paint_page(
	dc: Handle,
	area: Rect,
	hscroll: i32,
	app: &App<GdiMeasure>,
	s: &HeapStore,
	cache: &mut PicCache,
) -> usize {
	// Nothing of the page may spill over the toolbar and ruler above it.
	let saved = SaveDC(dc);
	IntersectClipRect(dc, area.left, area.top, area.right, area.bottom);
	let page = &app.doc().page;
	let page_w = to_units(page.paper_w, 96);
	let page_h = to_units(page.paper_h, 96);
	let margin_l = to_units(page.margin_l, 96);
	let margin_t = to_units(page.margin_t, 96);
	let client_w = area.right - area.left;
	let x0 = area.left + crate::chrome::page_x0(client_w, page_w, hscroll);
	let desk = CreateSolidBrush(0x0080_8080);
	FillRect(dc, &area, desk);
	DeleteObject(desk);
	let white = CreateSolidBrush(0x00FF_FFFF);

	let cum = |l: &quill_core::layout::Line| (l.page as i32 - 1) * (page_h + PAGE_GAP) + margin_t + l.y;
	// Where the first shown line sits decides the scroll offset.
	let mut top_cum = 0;
	let mut line_h = 16;
	app.for_each_line(s, |i, l| {
		if i == app.top_line() {
			top_cum = cum(l);
			line_h = (l.ascent + l.descent).max(8);
		}
	});
	let off = top_cum - PAD;
	// The pages that show.
	let mut p = 0;
	loop {
		let top = p * (page_h + PAGE_GAP) - off + area.top;
		if top > area.bottom || p > 200 {
			break;
		}
		if top + page_h > area.top {
			let r = Rect {
				left: x0,
				top,
				right: x0 + page_w,
				bottom: top + page_h,
			};
			FillRect(dc, &r, white);
		}
		p += 1;
	}
	DeleteObject(white);

	let tx = x0 + margin_l;
	SetTextAlign(dc, TA_BASELINE);
	let sel = app.selection();
	let mut counters: ListCounters = [[0; 9]; 4];
	let mut fitted = 0usize;
	let doc = app.doc();
	app.for_each_line(s, |i, l| {
		let label = line_label(app, s, l, &mut counters, 96);
		if i < app.top_line() {
			return;
		}
		let y_top = cum(l) - off + area.top;
		if y_top > area.bottom {
			return;
		}
		fitted += 1;
		draw_line(
			dc,
			&LineCtx {
				doc,
				s,
				app,
				kind: StoryKind::Body,
				dpi: 96,
				sel,
				mdc: dc,
				page: l.page,
				fields: None,
			},
			&mut Pics::Cache(cache),
			l,
			label,
			tx,
			y_top,
		);
	});

	// The caret.
	if app.editing() {
		if let Some((idx, l, x)) = app.caret_xy(s) {
			if idx >= app.top_line() {
				let y = cum(&l) - off + area.top;
				let pen = CreatePen(0, 2, 0);
				let old = SelectObject(dc, pen);
				MoveToEx(dc, tx + x, y, null_mut());
				LineTo(dc, tx + x, y + l.ascent + l.descent);
				SelectObject(dc, old);
				DeleteObject(pen);
			}
		}
	}
	RestoreDC(dc, saved);
	(((area.bottom - area.top) / line_h.max(1)) as usize)
		.max(fitted.min(1))
		.max(1)
}

/// The laid-out body of a document for one device, and where on that device the page's top left corner is.
struct PageLayout<'a> {
	lines: &'a [quill_core::layout::Line],
	labels: &'a [Option<([u8; 16], usize, usize)>],
	pages: u32,
	dpi: u16,
	/// The unprintable margin to subtract (the printer's physical offset).
	offx: i32,
	offy: i32,
	/// A device context to ask for widths.
	mdc: Handle,
}

/// Draws page `p`: the header, the footer and the body lines of the page, with the paper's top left at (`ox`, `oy`) in the units of `dc`.
///
/// # Safety
/// `dc` must be a valid device context.
#[allow(clippy::too_many_arguments)]
unsafe fn draw_page(
	dc: Handle,
	app: &App<GdiMeasure>,
	s: &HeapStore,
	pl: &mut PageLayout<'_>,
	p: u32,
	ox: i32,
	oy: i32,
	cache: &mut PicCache,
) {
	let doc = app.doc();
	let (dpi, pages) = (pl.dpi, pl.pages);
	let setup = doc.page;
	let u = |tw: i32| to_units(tw, dpi);
	let paper_h = u(setup.paper_h);
	let tx = ox + u(setup.margin_l) - pl.offx;
	let first = p == 1 && setup.flags & 1 != 0;
	let mut m = GdiMeasure {
		dc: pl.mdc,
		grid: false,
		dpi,
	};
	for (kind, from_bottom, dist) in [
		(
			if first { StoryKind::FirstHeader } else { StoryKind::Header },
			false,
			setup.header_dist,
		),
		(
			if first { StoryKind::FirstFooter } else { StoryKind::Footer },
			true,
			setup.footer_dist,
		),
	] {
		if doc.story(kind).is_empty() {
			continue;
		}
		let mut hl: Vec<quill_core::layout::Line> = Vec::new();
		quill_core::layout::layout_story(doc, s, &mut m, kind, |l| {
			if l.page == 1 {
				hl.push(*l);
			}
		});
		let height = hl.last().map_or(0, |l| l.y + l.ascent + l.descent);
		let top = if from_bottom { paper_h - u(dist) - height } else { u(dist) };
		let mut c: ListCounters = [[0; 9]; 4];
		for l in &hl {
			let cx = LineCtx {
				doc,
				s,
				app,
				kind,
				dpi,
				sel: None,
				mdc: pl.mdc,
				page: p,
				fields: Some(pages),
			};
			let label = line_label(app, s, l, &mut c, dpi);
			draw_line(dc, &cx, &mut Pics::Banded(cache), l, label, tx, oy + top + l.y - pl.offy);
		}
	}
	for (l, label) in pl.lines.iter().zip(pl.labels.iter()) {
		if l.page != p {
			continue;
		}
		let cx = LineCtx {
			doc,
			s,
			app,
			kind: StoryKind::Body,
			dpi,
			sel: None,
			mdc: pl.mdc,
			page: p,
			fields: Some(pages),
		};
		draw_line(
			dc,
			&cx,
			&mut Pics::Banded(cache),
			l,
			*label,
			tx,
			oy + u(setup.margin_t) + l.y - pl.offy,
		);
	}
}

/// The number of pages and the laid-out body at 96 dpi, for the preview.
pub fn preview_pages(
	app: &App<GdiMeasure>,
	s: &HeapStore,
	dc: Handle,
) -> (Vec<quill_core::layout::Line>, Vec<Option<([u8; 16], usize, usize)>>, u32) {
	let mut m = GdiMeasure {
		dc,
		grid: false,
		dpi: 96,
	};
	let mut lines: Vec<quill_core::layout::Line> = Vec::new();
	quill_core::layout::layout_story(app.doc(), s, &mut m, StoryKind::Body, |l| lines.push(*l));
	let mut counters: ListCounters = [[0; 9]; 4];
	let labels = lines.iter().map(|l| line_label(app, s, l, &mut counters, 96)).collect();
	let pages = lines.iter().map(|l| l.page).max().unwrap_or(1);
	(lines, labels, pages)
}

/// Paints the print preview: one page, or two side by side, scaled to fit the area, from page `first`.
///
/// # Safety
/// `dc` must be a valid device context.
pub unsafe fn paint_preview(
	dc: Handle,
	area: Rect,
	app: &App<GdiMeasure>,
	s: &HeapStore,
	mdc: Handle,
	first: u32,
	two: bool,
	cache: &mut PicCache,
) -> u32 {
	let desk = CreateSolidBrush(0x0080_8080);
	FillRect(dc, &area, desk);
	DeleteObject(desk);
	let (lines, labels, pages) = preview_pages(app, s, mdc);
	let setup = app.doc().page;
	let (pw, ph) = (to_units(setup.paper_w, 96), to_units(setup.paper_h, 96));
	let n = if two { 2 } else { 1 };
	let (aw, ah) = (area.right - area.left - 16 * (n + 1), area.bottom - area.top - 32);
	// The scale, in percent, that fits n pages across and one page down.
	let pct = (aw * 100 / (pw * n)).min(ah * 100 / ph).clamp(10, 200);
	let first = first.clamp(1, pages);
	let used = pw * pct / 100 * n + 16 * (n - 1);
	let left = area.left + ((area.right - area.left - used) / 2).max(8);
	let saved = SaveDC(dc);
	SetMapMode(dc, 8);
	SetWindowExtEx(dc, 100, 100, null_mut());
	SetViewportExtEx(dc, pct, pct, null_mut());
	let white = CreateSolidBrush(0x00FF_FFFF);
	let mut pl = PageLayout {
		lines: &lines,
		labels: &labels,
		pages,
		dpi: 96,
		offx: 0,
		offy: 0,
		mdc,
	};
	for k in 0..n {
		let p = first + k as u32;
		if p > pages {
			break;
		}
		// Device pixels to the logical units of the page: the paper's corner.
		let ox = (left - area.left + k * (pw * pct / 100 + 16)) * 100 / pct;
		let oy = 16 * 100 / pct;
		let paper = Rect {
			left: ox,
			top: oy,
			right: ox + pw,
			bottom: oy + ph,
		};
		FillRect(dc, &paper, white);
		SetTextAlign(dc, TA_BASELINE);
		draw_page(dc, app, s, &mut pl, p, ox, oy, cache);
	}
	DeleteObject(white);
	RestoreDC(dc, saved);
	pages
}

/// Prints pages `from` to `to` (1-based, inclusive) of the document on the printer `pdc`, laid out for that printer. `output` names a
/// file the job goes to instead of the printer's port. Returns false when the job could not start.
///
/// # Safety
/// `pdc` must be a printer device context.
pub unsafe fn print_document(
	pdc: Handle,
	app: &App<GdiMeasure>,
	s: &HeapStore,
	title: &[u8],
	output: Option<&[u8]>,
	from: u32,
	to: u32,
) -> bool {
	const LOGPIXELSX: i32 = 88;
	const LOGPIXELSY: i32 = 90;
	const PHYSICALOFFSETX: i32 = 112;
	const PHYSICALOFFSETY: i32 = 113;
	let doc = app.doc();
	let dpi = GetDeviceCaps(pdc, LOGPIXELSY)
		.max(GetDeviceCaps(pdc, LOGPIXELSX))
		.clamp(72, 1200) as u16;
	let (offx, offy) = (GetDeviceCaps(pdc, PHYSICALOFFSETX), GetDeviceCaps(pdc, PHYSICALOFFSETY));
	let mut m = GdiMeasure {
		dc: pdc,
		grid: false,
		dpi,
	};
	let mut lines: Vec<quill_core::layout::Line> = Vec::new();
	quill_core::layout::layout_story(doc, s, &mut m, StoryKind::Body, |l| lines.push(*l));
	let pages = lines.iter().map(|l| l.page).max().unwrap_or(1);
	// List labels count through the whole document, so they are worked out once, in order.
	let mut counters: ListCounters = [[0; 9]; 4];
	let labels: Vec<Option<([u8; 16], usize, usize)>> = lines.iter().map(|l| line_label(app, s, l, &mut counters, dpi)).collect();
	let mut name = title.to_vec();
	name.push(0);
	let mut out = output.map(<[u8]>::to_vec);
	if let Some(o) = out.as_mut() {
		o.push(0);
	}
	let info = DocInfoA {
		size: 20,
		doc_name: name.as_ptr(),
		output: out.as_ref().map_or(core::ptr::null(), |o| o.as_ptr()),
		datatype: core::ptr::null(),
		kind: 0,
	};
	if StartDocA(pdc, &info) <= 0 {
		return false;
	}
	let mut cache = PicCache::new();
	SetTextAlign(pdc, TA_BASELINE);
	let mut layout = PageLayout {
		lines: &lines,
		labels: &labels,
		pages,
		dpi,
		offx,
		offy,
		mdc: pdc,
	};
	for p in from.max(1)..=to.min(pages) {
		StartPage(pdc);
		draw_page(pdc, app, s, &mut layout, p, 0, 0, &mut cache);
		EndPage(pdc);
	}
	EndDoc(pdc);
	true
}

/// The number of pages of the body at the printer's resolution `dpi`.
pub fn page_count(app: &App<GdiMeasure>, s: &HeapStore, dc: Handle, dpi: u16) -> u32 {
	let mut m = GdiMeasure { dc, grid: false, dpi };
	quill_core::layout::layout_story(app.doc(), s, &mut m, StoryKind::Body, |_| {}).max(1)
}
