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

/// What the screen's device context said about a font, so that asking is once per font and not once per layout: the widths of
/// characters 32 to 255 and the metrics. Layout builds its own font cache for every query, and each of its misses is two calls into GDI
/// (`GetCharWidthA` of 224 characters and `GetTextMetricsA`), which on a 486 is most of what a key costs. The screen's fonts do not
/// change while Quill runs, so the screen's measure (96 dots per inch, never the printer's) keeps them.
#[derive(Copy, Clone)]
struct Measured {
	font: FontRef,
	widths: [u16; GLYPHS],
	metrics: FontMetrics,
}

static mut MEASURED: [Option<Measured>; 16] = [None; 16];
static mut NEXT_MEASURED: usize = 0;

/// The cached entry for `font` on the screen, made by `ask` when there is none.
fn screen_font(font: &FontRef, ask: impl FnOnce() -> Measured) -> Measured {
	// SAFETY: single-threaded; the cache is only touched here, and an entry is replaced only when the ring comes round to it.
	unsafe {
		let cache = &mut *core::ptr::addr_of_mut!(MEASURED);
		if let Some(m) = cache.iter().flatten().find(|m| m.font == *font) {
			return *m;
		}
		let made = ask();
		let at = NEXT_MEASURED % cache.len();
		NEXT_MEASURED += 1;
		if let Some(slot) = cache.get_mut(at) {
			*slot = Some(made);
		}
		made
	}
}

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
		if self.dpi == 96 {
			return screen_font(font, || self.measure_font(font)).metrics;
		}
		self.ask_metrics(font)
	}

	fn widths(&mut self, font: &FontRef, out: &mut [u16; GLYPHS]) {
		if self.grid {
			out.fill(1);
			return;
		}
		if self.dpi == 96 {
			*out = screen_font(font, || self.measure_font(font)).widths;
			return;
		}
		self.ask_widths(font, out);
	}
}

impl GdiMeasure {
	fn measure_font(&self, font: &FontRef) -> Measured {
		let mut widths = [0u16; GLYPHS];
		self.ask_widths(font, &mut widths);
		Measured {
			font: *font,
			widths,
			metrics: self.ask_metrics(font),
		}
	}

	fn ask_metrics(&self, font: &FontRef) -> FontMetrics {
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

	fn ask_widths(&self, font: &FontRef, out: &mut [u16; GLYPHS]) {
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
		let b = CreateSolidBrush(crate::scheme::face());
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
	// Stretched, so a zoomed view (a mapping mode that scales page units to pixels) draws the picture at the size it is shown.
	StretchDIBits(dc, x, y, w, h, 0, 0, w, h, r.bits.as_ptr(), &info, 0, SRCCOPY);
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
	/// Draw each character at its own layout position (a zoomed screen), so no rounding of the font's widths or of the layout's
	/// advances builds up along the line.
	per_char: bool,
	/// The zoom in percent, so the selected picture's frame and handles are a pixel and seven pixels whatever the page units are.
	pct: i32,
	/// The half-period of the wavy underline of misspelt words in page units, or 0 when the page shows none (the printer, a header
	/// or footer, and the screen with Check Spelling as You Type off).
	wave: i32,
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
	span!(PLine);
	#[cfg(feature = "bench")]
	crate::bench::line();
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
	// The layout's advance of each character of the run, which the drawing follows: a zoomed font is hinted to whole pixels and its own
	// widths drift from the layout's, and each advance given to the system is rounded alone, so a run drawn in one call ends away from
	// where the caret and the clicks think.
	let mut adv = [0i32; 200];
	let mut n = 0usize;
	let mut run_x = 0;
	let mut run_cf = 0u16;
	let mut run_sel = false;
	let flush = |n: &mut usize, run: &[u8; 200], adv: &[i32; 200], x: i32, cf: u16, selected: bool| {
		if *n == 0 {
			return;
		}
		span!(PText);
		let (fr, vpos, cfmt) = font_ref(doc, s, cf);
		let extra = u8::from(cfmt.flags & cflag::UNDERLINE != 0) | (u8::from(cfmt.flags & cflag::STRIKE != 0) << 1);
		let old = SelectObject(dc, fhandle(&fr, extra));
		let shift = match vpos {
			1 => -(l.ascent / 3),
			2 => l.ascent / 4,
			_ => 0,
		};
		if cx.per_char {
			let total: i32 = adv.iter().take(*n).sum();
			if selected {
				let r = Rect {
					left: tx + x,
					top: y_top,
					right: tx + x + total,
					bottom: y_top + l.ascent + l.descent,
				};
				let b = CreateSolidBrush(crate::scheme::select());
				FillRect(dc, &r, b);
				DeleteObject(b);
				SetTextColor(dc, crate::scheme::select_text());
			} else {
				SetTextColor(dc, colour_of(app, s, &cfmt));
			}
			SetBkMode(dc, TRANSPARENT);
			let mut at = 0;
			for (ch, a) in run.iter().zip(adv.iter()).take(*n) {
				TextOutA(dc, tx + x + at, base + shift, ch, 1);
				at += *a;
			}
			SelectObject(dc, old);
			*n = 0;
			return;
		}
		if selected {
			SetBkMode(dc, OPAQUE);
			SetBkColor(dc, crate::scheme::select());
			SetTextColor(dc, crate::scheme::select_text());
		} else {
			SetBkMode(dc, TRANSPARENT);
			SetTextColor(dc, colour_of(app, s, &cfmt));
		}
		ExtTextOutA(
			dc,
			tx + x,
			base + shift,
			0,
			core::ptr::null(),
			run.as_ptr(),
			*n as u32,
			adv.as_ptr(),
		);
		SelectObject(dc, old);
		*n = 0;
	};
	let mut m = GdiMeasure {
		dc: cx.mdc,
		grid: false,
		dpi,
	};
	// The misspelt words in this line, and where the layout put the first and last of their characters.
	let mut marks = [(0u32, 0u32, i32::MAX, i32::MIN); 8];
	let mut nmarks = 0;
	if cx.wave > 0 {
		for (st, en) in app.spell_marks(s, l.start, l.start + l.len).take(marks.len()) {
			if let Some(m) = marks.get_mut(nmarks) {
				*m = (st, en, i32::MAX, i32::MIN);
				nmarks += 1;
			}
		}
	}
	quill_core::layout::walk_line(doc, s, &mut m, cx.kind, l, |p, c, x, w, leader, cf| {
		for m in marks.iter_mut().take(nmarks) {
			if p >= m.0 && p < m.1 {
				m.2 = m.2.min(x);
				m.3 = m.3.max(x + w);
			}
		}
		let selected = sel.is_some_and(|(lo, hi)| p >= lo && p < hi);
		if c == 0x01 {
			flush(&mut n, &run, &adv, run_x, run_cf, run_sel);
			if let Some((pw, ph)) = quill_core::layout::picture_size(doc, s, text, p, dpi, dpi) {
				let idx = text.object_at(s, p).map_or(0, |v| v & 0xFFFF) as u16;
				match pics {
					Pics::Cache(cache) => draw_picture(dc, cache, app, s, idx, tx + x, base - ph, pw, ph),
					Pics::Banded(cache) => draw_picture_banded(dc, cache, doc, s, idx, tx + x, base - ph, pw, ph),
				}
				if matches!(pics, Pics::Cache(_)) && selected {
					let r = quill_app::picture_geom::Frame {
						left: tx + x,
						top: base - ph,
						right: tx + x + pw,
						bottom: base,
					};
					picture_marks(dc, &r, cx.pct, sel.is_some_and(|(lo, hi)| hi > lo + 1));
				}
				return;
			}
			if let Some(kind) = text.object_at(s, p).map(|v| v >> 16).filter(|k| (1..=5).contains(k)) {
				// A field shows its value where it will print (the body), and its name where it is edited (a header or footer).
				let mut buf = [0u8; 32];
				let n = match cx.fields {
					// The screen reads the clock when it draws the field; a printed page has the date and time it was started with.
					Some(_) if matches!(pics, Pics::Cache(_)) && (kind == 3 || kind == 5) => {
						crate::ui::clock_text(kind, &mut buf)
					}
					Some(total) => app.field_text(kind, cx.page, total, &mut buf),
					None => {
						let name: &[u8] = match kind {
							1 => b"[Page]",
							2 => b"[Pages]",
							3 => b"[Date]",
							5 => b"[Time]",
							_ => b"[File]",
						};
						buf.get_mut(..name.len()).unwrap_or(&mut []).copy_from_slice(name);
						name.len()
					}
				};
				if n > 0 {
					let (fr, _, cfmt) = font_ref(doc, s, cf);
					let old = SelectObject(dc, fhandle(&fr, 0));
					if selected {
						SetBkMode(dc, OPAQUE);
						SetBkColor(dc, crate::scheme::select());
						SetTextColor(dc, crate::scheme::select_text());
					} else {
						SetBkMode(dc, TRANSPARENT);
						SetTextColor(dc, colour_of(app, s, &cfmt));
					}
					TextOutA(dc, tx + x, base, buf.as_ptr(), n as i32);
					SelectObject(dc, old);
				}
				return;
			}
		}
		if c == 0x09 || c == 0x01 {
			flush(&mut n, &run, &adv, run_x, run_cf, run_sel);
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
					SetBkColor(dc, crate::scheme::select());
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
					let b = CreateSolidBrush(crate::scheme::select());
					FillRect(dc, &r, b);
					DeleteObject(b);
				}
				SelectObject(dc, old);
			}
			return;
		}
		if n > 0 && (cf != run_cf || selected != run_sel || n == run.len()) {
			flush(&mut n, &run, &adv, run_x, run_cf, run_sel);
		}
		if n == 0 {
			run_x = x;
			run_cf = cf;
			run_sel = selected;
		}
		if let Some(slot) = run.get_mut(n) {
			*slot = c;
			if let Some(a) = adv.get_mut(n) {
				*a = w;
			}
			n += 1;
		}
	});
	flush(&mut n, &run, &adv, run_x, run_cf, run_sel);
	// The wave goes in after the text, from the layout's own positions, so it is under the glyphs the caret and the clicks agree with.
	span!(PWave);
	for m in marks.iter().take(nmarks) {
		if m.3 > m.2 {
			wavy_underline(dc, tx + m.2, tx + m.3, base + 1, cx.wave, (l.descent - 1).max(1));
		}
	}
}

/// Draws a red zig-zag from `x0` to `x1` with its top at `y`: points `unit` apart across and at most `unit` down (and `room` at the
/// most), joined with `LineTo`.
///
/// # Safety
/// `dc` must be a valid device context.
unsafe fn wavy_underline(dc: Handle, x0: i32, x1: i32, y: i32, unit: i32, room: i32) {
	let pen = CreatePen(0, 1, 0x0000_00FF);
	let old = SelectObject(dc, pen);
	let depth = unit.min(room).max(1);
	let mut x = x0;
	let mut low = false;
	MoveToEx(dc, x, y, null_mut());
	while x < x1 {
		x = (x + unit).min(x1);
		low = !low;
		LineTo(dc, x, y + if low { depth } else { 0 });
	}
	SelectObject(dc, old);
	DeleteObject(pen);
}

/// The number of sheets of paper the body takes on the screen.
pub fn body_pages(app: &App<GdiMeasure>, s: &HeapStore) -> u32 {
	app.page_total(s).max(1)
}

/// The selection's navy, as a COLORREF (0x00BBGGRR).
const NAVY: u32 = 0x0080_0000;

/// The selected picture's frame and eight handles in black, or, for a picture inside a larger selection, a navy border two pixels wide
/// inside its edge. `r` is the picture's rectangle in page units, drawn at `pct` percent, so a pixel is `100 / pct` of a unit.
unsafe fn picture_marks(dc: Handle, r: &quill_app::picture_geom::Frame, pct: i32, within: bool) {
	let px = crate::view::log(1, pct).max(1);
	if within {
		for k in 0..2 {
			let q = r.grow(-k * px);
			fill(dc, q.left, q.top, q.right, q.top + px, NAVY);
			fill(dc, q.left, q.bottom - px, q.right, q.bottom, NAVY);
			fill(dc, q.left, q.top, q.left + px, q.bottom, NAVY);
			fill(dc, q.right - px, q.top, q.right, q.bottom, NAVY);
		}
		return;
	}
	let o = r.grow(px);
	fill(dc, o.left, o.top, o.right, o.top + px, 0);
	fill(dc, o.left, o.bottom - px, o.right, o.bottom, 0);
	fill(dc, o.left, o.top, o.left + px, o.bottom, 0);
	fill(dc, o.right - px, o.top, o.right, o.bottom, 0);
	let size = crate::view::log(quill_app::picture_geom::HANDLE, pct).max(1);
	for b in quill_app::picture_geom::handles(&r.grow(px - 1), size) {
		fill(dc, b.left, b.top, b.right, b.bottom, 0);
	}
}

/// Fills a device rectangle with a colour.
unsafe fn fill(dc: Handle, left: i32, top: i32, right: i32, bottom: i32, colour: u32) {
	let r = Rect {
		left,
		top,
		right,
		bottom,
	};
	let b = CreateSolidBrush(colour);
	FillRect(dc, &r, b);
	DeleteObject(b);
}

/// Draws one sheet of paper in device pixels: a black edge, a shadow below and to the right, and the white page.
unsafe fn paper_sheet(dc: Handle, left: i32, top: i32, w: i32, h: i32) {
	fill(dc, left + 3, top + 3, left + w + 3, top + h + 3, 0x0040_4040);
	fill(dc, left - 1, top - 1, left + w + 1, top + h + 1, 0);
	fill(dc, left, top, left + w, top + h, 0x00FF_FFFF);
}

/// Sets the mapping from page units to pixels: `pct` percent, with page unit (0, 0) at device pixel (`x`, `y`).
unsafe fn map_pages(dc: Handle, pct: i32, x: i32, y: i32) {
	if pct == 100 {
		SetMapMode(dc, 1);
	} else {
		SetMapMode(dc, 8);
		SetWindowExtEx(dc, 100, 100, null_mut());
		SetViewportExtEx(dc, pct, pct, null_mut());
	}
	SetViewportOrgEx(dc, x, y, null_mut());
}

/// Paints the page view into the client rectangle `area`, scrolled `hscroll` pixels sideways and `vpos` page units down, at `pct`
/// percent. Only the lines that reach into `damage` (in the device's pixels) are drawn; the rest of the page is already in the picture. `mdc` is a device context with no mapping set, which character widths are asked of: under the zoom's mapping a font's widths
/// come back in scaled, hinted units and no longer agree with the layout. Returns how many lines a screenful holds and how many sheets the document takes.
///
/// # Safety
/// `dc` must be a valid device context.
pub unsafe fn paint_page(
	dc: Handle,
	area: Rect,
	hscroll: i32,
	vpos: i32,
	pct: i32,
	damage: Rect,
	app: &App<GdiMeasure>,
	s: &HeapStore,
	mdc: Handle,
	cache: &mut PicCache,
) -> (usize, u32) {
	use crate::view::{dev, log, page_x0, paper, rel, stride};
	span!(PPage);
	// Nothing of the page may spill over the toolbar and ruler above it.
	let saved = SaveDC(dc);
	IntersectClipRect(dc, area.left, area.top, area.right, area.bottom);
	let page = &app.doc().page;
	let (page_w, page_h) = paper(app);
	let stride = stride(app);
	let margin_l = to_units(page.margin_l, 96);
	let margin_t = to_units(page.margin_t, 96);
	let client_w = area.right - area.left;
	let x0 = area.left + page_x0(client_w, dev(page_w, pct), hscroll);
	#[cfg(feature = "bench")]
	let fills = crate::bench::span(crate::bench::P::PFills);
	fill(dc, area.left, area.top, area.right, area.bottom, crate::scheme::desk());
	let pages = body_pages(app, s);
	let view_h = log(area.bottom - area.top, pct);
	for p in 0..pages as i32 {
		let top = rel(p * stride, vpos);
		if top > view_h + 4 {
			break;
		}
		if top + page_h + 4 < 0 {
			continue;
		}
		let (l, t) = (x0, area.top + dev(top, pct));
		paper_sheet(dc, l, t, dev(page_w, pct), dev(top + page_h, pct) - dev(top, pct));
	}
	#[cfg(feature = "bench")]
	drop(fills);

	// Text and the caret are drawn in page units, relative to the first sheet's corner, under a mapping that scales them.
	map_pages(dc, pct, x0, area.top);
	let tx = margin_l;
	SetTextAlign(dc, TA_BASELINE);
	let sel = app.selection();
	let mut counters: ListCounters = [[0; 9]; 4];
	let doc = app.doc();
	let fields = (app.story_view() == 0).then_some(pages);
	let mut line_h = 16;
	let cum = |l: &quill_core::layout::Line| (l.page as i32 - 1) * stride + margin_t + l.y;
	// Only the lines from the first page on the screen to the bottom of the window are laid out.
	let first_page = (vpos.max(0) / stride.max(1)) as u32 + 1;
	app.for_each_line_from_page(s, first_page, |_, l| {
		let label = line_label(app, s, l, &mut counters, 96);
		let y_top = rel(cum(l), vpos);
		let h = (l.ascent + l.descent).max(1);
		if y_top > view_h + 4 {
			return false;
		}
		if y_top + h < -4 {
			return true;
		}
		line_h = h.max(8);
		// A line wholly outside the damaged rectangle is left as it is: its text, with its wave, is already in the picture, and drawing
		// it again would cost a character at a time for nothing.
		let (top_px, bottom_px) = (area.top + dev(y_top, pct), area.top + dev(y_top + h, pct));
		if bottom_px + 4 < damage.top || top_px - 4 > damage.bottom {
			return true;
		}
		draw_line(
			dc,
			&LineCtx {
				doc,
				s,
				app,
				kind: StoryKind::Body,
				dpi: 96,
				sel,
				mdc,
				page: l.page,
				fields,
				per_char: pct != 100,
				pct,
				wave: if app.spell_live() { (200 / pct.max(1)).clamp(2, 8) } else { 0 },
			},
			&mut Pics::Cache(cache),
			l,
			label,
			tx,
			y_top,
		);
		true
	});

	// The caret.
	span!(PCaret);
	if app.editing() {
		if let Some((_, l, x)) = app.caret_xy(s) {
			let y = rel(cum(&l), vpos);
			let pen = CreatePen(0, 2, 0);
			let old = SelectObject(dc, pen);
			MoveToEx(dc, tx + x, y, null_mut());
			LineTo(dc, tx + x, y + l.ascent + l.descent);
			SelectObject(dc, old);
			DeleteObject(pen);
		}
	}
	RestoreDC(dc, saved);
	((view_h / line_h.max(1)).max(1) as usize, pages)
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
	/// Whether the sheet is drawn scaled, so characters are placed one at a time.
	per_char: bool,
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
				per_char: pl.per_char,
				pct: 100,
				wave: 0,
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
			per_char: pl.per_char,
			pct: 100,
			wave: 0,
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

/// How the print preview is showing: the first page, one or two across, fitted to the window or at actual size, and the scroll
/// positions that matter at actual size.
#[derive(Copy, Clone)]
pub struct PreviewView {
	pub first: u32,
	pub two: bool,
	pub actual: bool,
	pub hscroll: i32,
	pub vscroll: i32,
}

/// What painting the preview worked out: the page count, and the size of everything it shows in pixels (for the scroll bars).
#[derive(Copy, Clone)]
pub struct PreviewSize {
	pub pages: u32,
	pub w: i32,
	pub h: i32,
}

/// The percentage the preview draws at for an area `aw` by `ah` pixels, and the size in pixels of the sheets at that scale.
pub fn preview_scale(app: &App<GdiMeasure>, aw: i32, ah: i32, two: bool, actual: bool) -> (i32, i32, i32) {
	let setup = app.doc().page;
	let (pw, ph) = (to_units(setup.paper_w, 96), to_units(setup.paper_h, 96));
	let n = if two { 2 } else { 1 };
	let pct = if actual {
		100
	} else {
		((aw - 16 * (n + 1)) * 100 / (pw * n))
			.min((ah - 32) * 100 / ph)
			.clamp(10, 200)
	};
	(pct, pw * pct / 100, ph * pct / 100)
}

/// Paints the print preview into `area`: one sheet, or two side by side, from `v.first`.
///
/// # Safety
/// `dc` must be a valid device context.
pub unsafe fn paint_preview(
	dc: Handle,
	area: Rect,
	app: &App<GdiMeasure>,
	s: &HeapStore,
	mdc: Handle,
	v: &PreviewView,
	cache: &mut PicCache,
) -> PreviewSize {
	let saved = SaveDC(dc);
	IntersectClipRect(dc, area.left, area.top, area.right, area.bottom);
	fill(dc, area.left, area.top, area.right, area.bottom, crate::scheme::desk());
	let (lines, labels, pages) = preview_pages(app, s, mdc);
	let n = if v.two { 2 } else { 1 };
	let (aw, ah) = (area.right - area.left, area.bottom - area.top);
	let (pct, pwd, phd) = preview_scale(app, aw, ah, v.two, v.actual);
	let content_w = 16 + n * pwd + (n - 1) * 16 + 16;
	let content_h = phd + 32;
	let first = v.first.clamp(1, pages);
	let left = if content_w <= aw {
		area.left + (aw - content_w) / 2 + 16
	} else {
		area.left + 16 - v.hscroll
	};
	let top = if content_h <= ah {
		area.top + (ah - phd) / 2
	} else {
		area.top + 16 - v.vscroll
	};
	let mut pl = PageLayout {
		lines: &lines,
		labels: &labels,
		pages,
		dpi: 96,
		offx: 0,
		offy: 0,
		mdc,
		per_char: pct != 100,
	};
	for k in 0..n {
		let p = first + k as u32;
		if p > pages {
			break;
		}
		let l = left + k * (pwd + 16);
		paper_sheet(dc, l, top, pwd, phd);
		// The sheet's text, in page units, with the paper's corner at the origin.
		map_pages(dc, pct, l, top);
		SetTextAlign(dc, TA_BASELINE);
		draw_page(dc, app, s, &mut pl, p, 0, 0, cache);
		SetMapMode(dc, 1);
		SetViewportOrgEx(dc, 0, 0, null_mut());
	}
	RestoreDC(dc, saved);
	PreviewSize {
		pages,
		w: content_w,
		h: content_h,
	}
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
		per_char: false,
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
