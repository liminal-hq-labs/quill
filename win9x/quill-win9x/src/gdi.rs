// The GDI page: the Measure that asks Windows for font metrics, and the painter that draws the laid-out document with real fonts.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::mem::MaybeUninit;
use core::ptr::null_mut;

use quill_app::{line_label, App, ListCounters};
use quill_core::doc::{cflag, CFmt};
use quill_core::layout::{font_ref, to_units, FontMetrics, FontRef, Measure, GLYPHS};
use quill_heap::HeapStore;

use alloc::vec::Vec;
use crate::sys::*;

/// The measure: the screen at 96 dots per inch, or (for the golden-frame tests) the DOS text grid.
#[derive(Copy, Clone)]
pub struct GdiMeasure {
	/// A screen device context to ask.
	pub dc: Handle,
	/// Measure like the DOS text screen instead (one column per character, 10 by 6 units per inch).
	pub grid: bool,
}

static mut FONTS: [(Option<(FontRef, u8)>, Handle); 12] = [(None, null_mut()); 12];
static mut NEXT_FONT: usize = 0;

/// The GDI font for `fr` with the extra `cflag` bits for underline and strikeout, cached in a small ring.
pub fn font_handle(fr: &FontRef, extra: u8) -> Handle {
	// SAFETY: single-threaded; the cache is only touched here.
	unsafe {
		let cache = &mut *core::ptr::addr_of_mut!(FONTS);
		for (k, h) in cache.iter() {
			if let Some((f, e)) = k {
				if f == fr && *e == extra {
					return *h;
				}
			}
		}
		let mut face = [0u8; 32];
		for (d, s) in face.iter_mut().zip(fr.face.iter().take(usize::from(fr.face_len))) {
			*d = *s;
		}
		let height = -((i32::from(fr.size) * 96 + 72) / 144);
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
			*old = Some((*fr, extra));
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
			(96, 96)
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
			let old = SelectObject(self.dc, font_handle(font, 0));
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
			let old = SelectObject(self.dc, font_handle(font, 0));
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
		if let Some(i) = self.items.iter().position(|r| r.idx == idx && r.fp == fp && r.w == w && r.h == h) {
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
			let window = self.window.get_or_insert_with(|| alloc::boxed::Box::new([0u8; quill_app::picture::WINDOW]));
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
unsafe fn draw_picture(dc: Handle, cache: &mut PicCache, app: &App<GdiMeasure>, s: &HeapStore, idx: u16, x: i32, y: i32, w: i32, h: i32) {
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

/// Paints the page view into the client rectangle `area` and returns how many lines fit.
///
/// # Safety
/// `dc` must be a valid device context.
pub unsafe fn paint_page(dc: Handle, area: Rect, app: &App<GdiMeasure>, s: &HeapStore, cache: &mut PicCache) -> usize {
	let page = &app.doc().page;
	let page_w = to_units(page.paper_w, 96);
	let page_h = to_units(page.paper_h, 96);
	let margin_l = to_units(page.margin_l, 96);
	let margin_t = to_units(page.margin_t, 96);
	let client_w = area.right - area.left;
	let x0 = area.left + ((client_w - page_w) / 2).max(PAD);
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
		let base = y_top + l.ascent;
		if let Some((mut lab, n, first)) = label {
			if lab.first() == Some(&0x07) {
				lab[0] = 0x95;
			}
			let cf = doc.body.cfmt_at(s, l.start).unwrap_or(0);
			let (fr, _, cfmt) = font_ref(doc, s, cf);
			let old = SelectObject(dc, font_handle(&fr, 0));
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
			let old = SelectObject(dc, font_handle(&fr, extra));
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
		app.walk_chars(s, l, |p, c, x, w, leader, cf| {
			let selected = sel.is_some_and(|(lo, hi)| p >= lo && p < hi);
			if c == 0x01 {
				flush(&mut n, &run, run_x, run_cf, run_sel);
				if let Some((pw, ph)) = quill_core::layout::picture_size(doc, s, &doc.body, p, 96, 96) {
					let idx = doc.body.object_at(s, p).map_or(0, |v| v & 0xFFFF) as u16;
					draw_picture(dc, cache, app, s, idx, tx + x, base - ph, pw, ph);
					return;
				}
			}
			if c == 0x09 || c == 0x01 {
				flush(&mut n, &run, run_x, run_cf, run_sel);
				if c == 0x09 && w > 0 {
					let (fr, _, cfmt) = font_ref(doc, s, cf);
					let old = SelectObject(dc, font_handle(&fr, 0));
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
	(((area.bottom - area.top) / line_h.max(1)) as usize)
		.max(fitted.min(1))
		.max(1)
}
