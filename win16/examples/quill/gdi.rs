// The Windows 3.1 page: the Measure that asks GDI for font metrics, and the painter that draws the laid-out document with real fonts.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The same drawing as the Win32 build's `gdi.rs`, with the Win16 calls: every handle is a 16-bit word, rectangles and points are
// 16-bit, and fonts come from `CreateFontIndirect` with a 50-byte LOGFONT. Bitmaps are drawn by `picture.rs`; a picture it does not
// draw (PNG) shows as a grey box the size it prints.
use quill_app::view::{self, PageGeom};
use quill_app::{App, ListCounters, line_label};
use quill_core::doc::{CFmt, StoryKind, cflag};
use quill_core::layout::{FontMetrics, FontRef, GLYPHS, Line, Measure, font_ref};
use w16::*;

use crate::store::FarStore;

/// The measure: the screen at 96 dots per inch, or (for the golden-frame runs, `/PLAY`) the DOS text grid.
#[derive(Copy, Clone)]
pub struct Gdi16 {
	/// A memory device context compatible with the screen, kept for the life of the program, to ask for metrics.
	pub dc: u32,
	/// Measure like the DOS text screen instead (one column per character, 10 by 6 units per inch).
	pub grid: bool,
}

pub type QApp = App<Gdi16>;

/// A rectangle as Windows 3.1 lays it out.
#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct Rect16 {
	pub left: i16,
	pub top: i16,
	pub right: i16,
	pub bottom: i16,
}

impl Rect16 {
	pub fn new(left: i32, top: i32, right: i32, bottom: i32) -> Self {
		Rect16 {
			left: clamp16(left),
			top: clamp16(top),
			right: clamp16(right),
			bottom: clamp16(bottom),
		}
	}
}

/// A coordinate as a 16-bit value, kept inside what GDI takes.
pub fn clamp16(v: i32) -> i16 {
	v.clamp(-16000, 16000) as i16
}

/// A near pointer as the 32-bit value the import stubs take.
pub fn np<T>(p: *const T) -> u32 {
	p as u32
}

/// A near pointer as a far pointer into the data segment, for calls that take an `l` (and SendMessage's lParam).
pub fn fp<T>(p: *const T) -> u32 {
	far(data_selector(), p as u32)
}

/// Reads a WIN.INI string. `section`, `key` and `default` are `cs!` literals; `key` of `None` lists every key in the section, each
/// followed by a NUL. GetProfileString's key is an `l` slot (a far pointer, so that 0 can mean NULL), which is why the key is made
/// far here and the rest stay near. Returns the number of bytes written, not counting the final NUL.
#[inline(never)]
pub fn profile_string(section: u32, key: Option<u32>, default: u32, buf: &mut [u8]) -> usize {
	let key = key.map_or(0, |k| far(data_selector(), k));
	let room = buf.len().saturating_sub(1) as u32;
	unsafe { GetProfileString(section, key, default, np(buf.as_mut_ptr()), room) as usize }
}

pub const TRANSPARENT: u32 = 1;
pub const OPAQUE: u32 = 2;
pub const TA_BASELINE: u32 = 24;
pub const SRCCOPY: u32 = 0x00CC_0020;
pub const FACE: u32 = 0x00C0_C0C0;
pub const WHITE: u32 = 0x00FF_FFFF;
pub const SHADOW: u32 = 0x0080_8080;
pub const BLACK: u32 = 0;
const SELECTED_BG: u32 = 0x0080_0000;

/// The brushes of the five colours the chrome and the page are drawn in (black, white, the desk's grey, the face grey and the
/// selection), made on first use and kept for the life of the program: a fill is one `FillRect` where it was a brush made, used and
/// deleted. Any other colour (a document's colour, the toolbar's colour swatch) still makes its own.
static mut BRUSHES: [u16; 5] = [0; 5];

fn brush_slot(colour: u32) -> Option<usize> {
	match colour {
		BLACK => Some(0),
		WHITE => Some(1),
		SHADOW => Some(2),
		FACE => Some(3),
		SELECTED_BG => Some(4),
		_ => None,
	}
}

/// Fills a rectangle with one solid colour.
pub fn fill(dc: u32, x: i32, y: i32, w: i32, h: i32, colour: u32) {
	if w <= 0 || h <= 0 {
		return;
	}
	let r = Rect16::new(x, y, x + w, y + h);
	#[cfg(feature = "bench")]
	crate::bench::filled(w, h);
	unsafe {
		if let Some(i) = brush_slot(colour) {
			let kept = &mut (*core::ptr::addr_of_mut!(BRUSHES))[i];
			if *kept == 0 {
				*kept = CreateSolidBrush(colour) as u16;
			}
			FillRect(dc, np(&r), u32::from(*kept));
			return;
		}
		let b = CreateSolidBrush(colour);
		FillRect(dc, np(&r), b);
		DeleteObject(b);
	}
}

/// Fills the part of a rectangle that is inside `clip` (nothing when none is).
fn fill_in(dc: u32, clip: &Rect16, x: i32, y: i32, w: i32, h: i32, colour: u32) {
	let (x0, y0) = (x.max(i32::from(clip.left)), y.max(i32::from(clip.top)));
	let (x1, y1) = (
		(x + w).min(i32::from(clip.right)),
		(y + h).min(i32::from(clip.bottom)),
	);
	fill(dc, x0, y0, x1 - x0, y1 - y0, colour);
}

/// The 50-byte LOGFONT of Windows 3.1.
#[repr(C)]
struct LogFont {
	height: i16,
	width: i16,
	escapement: i16,
	orientation: i16,
	weight: i16,
	italic: u8,
	underline: u8,
	strike: u8,
	charset: u8,
	out_precision: u8,
	clip_precision: u8,
	quality: u8,
	pitch_family: u8,
	face: [u8; 32],
}

/// Makes a font: `height` in pixels (negative for the character height), the face, bold, italic, underline and strikeout, and the
/// pitch and family byte.
pub fn make_font(
	height: i32,
	face: &[u8],
	bold: bool,
	italic: bool,
	extra: u8,
	pitch_family: u8,
) -> u32 {
	let mut lf = LogFont {
		height: clamp16(height),
		width: 0,
		escapement: 0,
		orientation: 0,
		weight: if bold { 700 } else { 400 },
		italic: u8::from(italic),
		underline: extra & 1,
		strike: u8::from(extra & 2 != 0),
		charset: 0,
		out_precision: 0,
		clip_precision: 0,
		quality: 0,
		pitch_family,
		face: [0; 32],
	};
	for (d, s) in lf.face.iter_mut().zip(face.iter().take(31)) {
		*d = *s;
	}
	unsafe { CreateFontIndirect(np(&lf)) }
}

/// The fonts made lately, keyed by the font, the underline and strikeout bits: a small ring, so a page of a few faces makes each once.
static mut FONTS: [(Option<(FontRef, u8)>, u32); 10] = [(None, 0); 10];
static mut NEXT_FONT: usize = 0;

/// The font handle `font_handle` already holds for `fr` and `extra`, if any.
fn font_cached(fr: &FontRef, extra: u8) -> Option<u32> {
	unsafe {
		(*core::ptr::addr_of!(FONTS))
			.iter()
			.find_map(|(k, h)| k.filter(|(f, e)| f == fr && *e == extra).map(|_| *h))
	}
}

/// The GDI font for `fr` at 96 dots per inch with the extra underline (1) and strikeout (2) bits.
pub fn font_handle(fr: &FontRef, extra: u8) -> u32 {
	if let Some(h) = font_cached(fr, extra) {
		return h;
	}
	unsafe {
		let cache = &mut *core::ptr::addr_of_mut!(FONTS);
		let height = -((i32::from(fr.size) * 96 + 72) / 144);
		let pitch_family = match fr.family {
			1 => 0x10 | 2,
			2 => 0x20 | 2,
			3 => 0x30 | 1,
			_ => 0,
		};
		let face = fr.face.get(..usize::from(fr.face_len)).unwrap_or(&[]);
		let h = make_font(
			height,
			face,
			fr.flags & cflag::BOLD != 0,
			fr.flags & cflag::ITALIC != 0,
			extra,
			pitch_family,
		);
		let slot = NEXT_FONT % cache.len();
		NEXT_FONT += 1;
		if let Some((old, handle)) = cache.get_mut(slot) {
			if old.is_some() && *handle != 0 {
				DeleteObject(*handle);
			}
			*old = Some((*fr, extra));
			*handle = h;
		}
		h
	}
}

/// The widths and metrics of the screen's fonts, in a block of far memory of eight entries (the plan's 4 KB): one `GetCharWidth` and one
/// `GetTextMetrics` per font for the life of the program, where every layout walk and every drawn line used to ask again. The only near
/// data is the block's far pointer (0 until the first use, `u32::MAX` when Windows would not give the block) and the entry to replace
/// next. Only the screen measure uses it: the printer's DC measures a printer's fonts and keeps asking.
static mut MEASURES: u32 = 0;
static mut MEASURE_NEXT: u8 = 0;
const MEASURE_SLOTS: u32 = 8;
const MEASURE_STRIDE: u32 = 512;
/// An entry: the font's 37 key bytes, a flags byte (1 widths are set, 2 metrics are set), the three metrics as words, the 224 widths.
const KEY_LEN: usize = 37;
const FLAGS_AT: u32 = 37;
const METRICS_AT: u32 = 38;
const WIDTHS_AT: u32 = 64;

fn key_bytes(f: &FontRef) -> [u8; KEY_LEN] {
	let mut k = [0u8; KEY_LEN];
	k[..31].copy_from_slice(&f.face);
	k[31] = f.face_len;
	k[32] = f.family;
	k[33..35].copy_from_slice(&f.size.to_le_bytes());
	k[35..37].copy_from_slice(&f.flags.to_le_bytes());
	k
}

/// The far block, made on first use; `None` when Windows would not give it.
fn measures() -> Option<u32> {
	unsafe {
		if MEASURES == 0 {
			MEASURES = u32::MAX;
			if let Some(b) = crate::far::FarBlock::alloc(MEASURE_SLOTS * MEASURE_STRIDE, true) {
				MEASURES = b.ptr_at(0);
				// Held for the life of the program, so the block is never freed.
				core::mem::forget(b);
			}
		}
		(MEASURES != u32::MAX).then_some(MEASURES)
	}
}

/// The far address of the entry for `font`, made (and given the oldest slot) when it is not there yet.
fn measure_entry(base: u32, font: &FontRef) -> u32 {
	let key = key_bytes(font);
	let at = |i: u32| base + i * MEASURE_STRIDE;
	let mut got = [0u8; KEY_LEN + 1];
	for i in 0..MEASURE_SLOTS {
		unsafe { far_read(at(i), got.as_mut_ptr(), KEY_LEN + 1) };
		if got[KEY_LEN] != 0 && got[..KEY_LEN] == key {
			return at(i);
		}
	}
	let e = unsafe {
		let n = MEASURE_NEXT;
		MEASURE_NEXT = (n + 1) % MEASURE_SLOTS as u8;
		at(u32::from(n))
	};
	got[..KEY_LEN].copy_from_slice(&key);
	got[KEY_LEN] = 0;
	unsafe { far_write(e, got.as_ptr(), KEY_LEN + 1) };
	e
}

/// Sets bit `bit` of the entry's flags.
fn measure_flag(e: u32, bit: u8) {
	let mut f = 0u8;
	unsafe {
		far_read(e + FLAGS_AT, &mut f, 1);
		f |= bit;
		far_write(e + FLAGS_AT, &f, 1);
	}
}

/// The advance of the leader character `ch` in `fr`: from the screen widths kept in far memory when they are there, else asked of
/// `dc` (the same device, so the same widths). At least 1.
fn leader_width(fr: &FontRef, dc: u32, ch: u8) -> i32 {
	if let Some(base) = measures() {
		let e = measure_entry(base, fr);
		let mut f = 0u8;
		unsafe { far_read(e + FLAGS_AT, &mut f, 1) };
		if f & 1 != 0 {
			let mut w = [0u8; 2];
			unsafe { far_read(e + WIDTHS_AT + u32::from(ch - 32) * 2, w.as_mut_ptr(), 2) };
			return i32::from(u16::from_le_bytes(w)).max(1);
		}
	}
	let mut cw = [0i16; 1];
	unsafe { GetCharWidth(dc, u32::from(ch), u32::from(ch), np(cw.as_mut_ptr())) };
	i32::from(cw[0]).max(1)
}

impl Measure for Gdi16 {
	fn resolution(&self) -> (u16, u16) {
		if self.grid {
			(10, 6)
		} else {
			(view::DPI, view::DPI)
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
		#[cfg(feature = "bench")]
		crate::bench::add(crate::bench::METRICS, 1);
		let cached = self.dc == app_measure_dc();
		let entry = if cached { measures() } else { None }.map(|base| measure_entry(base, font));
		let mut m = [0i16; 3];
		if let Some(e) = entry {
			let mut f = 0u8;
			unsafe { far_read(e + FLAGS_AT, &mut f, 1) };
			if f & 2 != 0 {
				unsafe { far_read(e + METRICS_AT, m.as_mut_ptr().cast::<u8>(), 6) };
				return FontMetrics {
					ascent: i32::from(m[0]),
					descent: i32::from(m[1]),
					leading: i32::from(m[2]),
				};
			}
		}
		let mut tm = [0u8; 32];
		unsafe {
			let old = SelectObject(self.dc, font_handle(font, 0));
			GetTextMetrics(self.dc, np(tm.as_mut_ptr()));
			SelectObject(self.dc, old);
		}
		let word = |at: usize| i16::from_le_bytes([tm[at], tm[at + 1]]);
		m = [word(2), word(4), word(8)];
		if let Some(e) = entry {
			unsafe { far_write(e + METRICS_AT, m.as_ptr().cast::<u8>(), 6) };
			measure_flag(e, 2);
		}
		FontMetrics {
			ascent: i32::from(m[0]),
			descent: i32::from(m[1]),
			leading: i32::from(m[2]),
		}
	}

	fn widths(&mut self, font: &FontRef, out: &mut [u16; GLYPHS]) {
		if self.grid {
			out.fill(1);
			return;
		}
		#[cfg(feature = "bench")]
		crate::bench::add(crate::bench::CACHES, 1);
		let cached = self.dc == app_measure_dc();
		let entry = if cached { measures() } else { None }.map(|base| measure_entry(base, font));
		if let Some(e) = entry {
			let mut f = 0u8;
			unsafe { far_read(e + FLAGS_AT, &mut f, 1) };
			if f & 1 != 0 {
				unsafe { far_read(e + WIDTHS_AT, out.as_mut_ptr().cast::<u8>(), GLYPHS * 2) };
				return;
			}
		}
		let mut w = [0i16; GLYPHS];
		unsafe {
			let old = SelectObject(self.dc, font_handle(font, 0));
			GetCharWidth(self.dc, 32, 255, np(w.as_mut_ptr()));
			SelectObject(self.dc, old);
		}
		for (o, v) in out.iter_mut().zip(w.iter()) {
			*o = (*v).clamp(0, 4000) as u16;
		}
		if let Some(e) = entry {
			unsafe { far_write(e + WIDTHS_AT, out.as_ptr().cast::<u8>(), GLYPHS * 2) };
			measure_flag(e, 1);
		}
	}
}

/// What a page paint has told the device context about text, so a run changes only what differs from the one before it (a run used
/// to select its font, set the background mode and the colour, draw, and select the old font back: five calls). The font that was in
/// the context first is selected back once, by `done`, and before a font is made that could take the ring's place of one still
/// selected.
pub struct TextState {
	old: u32,
	font: u32,
	mode: u32,
	colour: u32,
	back: u32,
}

/// A value no font handle, mode or colour has.
const UNSET: u32 = u32::MAX;

impl TextState {
	pub const fn new() -> Self {
		TextState {
			old: 0,
			font: UNSET,
			mode: UNSET,
			colour: UNSET,
			back: UNSET,
		}
	}

	/// Selects the font for `fr` and `extra`.
	fn font(&mut self, dc: u32, fr: &FontRef, extra: u8) {
		let h = match font_cached(fr, extra) {
			Some(h) => h,
			None => {
				// A new font may take the place of the one selected, which must not be deleted while it is.
				self.done(dc);
				font_handle(fr, extra)
			}
		};
		if h != self.font {
			let old = unsafe { SelectObject(dc, h) };
			if self.old == 0 {
				self.old = old;
			}
			self.font = h;
		}
	}

	fn mode(&mut self, dc: u32, mode: u32) {
		if mode != self.mode {
			self.mode = mode;
			unsafe { SetBkMode(dc, mode) };
		}
	}

	fn colour(&mut self, dc: u32, colour: u32) {
		if colour != self.colour {
			self.colour = colour;
			unsafe { SetTextColor(dc, colour) };
		}
	}

	fn back(&mut self, dc: u32, colour: u32) {
		if colour != self.back {
			self.back = colour;
			unsafe { SetBkColor(dc, colour) };
		}
	}

	/// Puts the font the context had back.
	pub fn done(&mut self, dc: u32) {
		if self.old != 0 {
			unsafe { SelectObject(dc, self.old) };
			self.old = 0;
		}
		self.font = UNSET;
	}
}

fn colour_of(app: &QApp, s: &FarStore, cf: &CFmt) -> u32 {
	if cf.colour == 0 {
		return 0;
	}
	app.doc()
		.colours
		.get(s, u16::from(cf.colour))
		.map_or(0, |c| {
			u32::from(c.r) | (u32::from(c.g) << 8) | (u32::from(c.b) << 16)
		})
}

/// Draws one laid-out line whose top is at `y_top` and whose left margin is at `tx`.
fn draw_line(
	dc: u32,
	app: &QApp,
	s: &FarStore,
	l: &Line,
	label: Option<([u8; 16], usize, usize)>,
	tx: i32,
	y_top: i32,
	pages: u32,
	ts: &mut TextState,
) {
	let doc = app.doc();
	let text = doc.story(StoryKind::Body);
	let sel = app.selection();
	let base = y_top + l.ascent;
	#[cfg(feature = "bench")]
	crate::bench::add(crate::bench::LINES, 1);
	unsafe {
		if let Some((mut lab, n, first)) = label {
			if lab.first() == Some(&0x07) {
				lab[0] = 0x95;
			}
			let cf = text.cfmt_at(s, l.start).unwrap_or(0);
			let (fr, _, cfmt) = font_ref(doc, s, cf);
			ts.font(dc, &fr, 0);
			ts.mode(dc, TRANSPARENT);
			ts.colour(dc, colour_of(app, s, &cfmt));
			TextOut(
				dc,
				(tx + first as i32) as u32,
				base as u32,
				np(lab.as_ptr()),
				n as u32,
			);
		}
	}
	// Runs of one format and selection state, split at tabs and objects.
	let mut run = [0u8; 160];
	let mut n = 0usize;
	let mut run_x = 0;
	let mut run_cf = 0u16;
	let mut run_sel = false;
	let flush =
		|n: &mut usize, run: &[u8; 160], x: i32, cf: u16, selected: bool, ts: &mut TextState| {
			if *n == 0 {
				return;
			}
			let (fr, vpos, cfmt) = font_ref(doc, s, cf);
			let extra = u8::from(cfmt.flags & cflag::UNDERLINE != 0)
				| (u8::from(cfmt.flags & cflag::STRIKE != 0) << 1);
			let shift = match vpos {
				1 => -(l.ascent / 3),
				2 => l.ascent / 4,
				_ => 0,
			};
			crate::store::sample();
			#[cfg(feature = "bench")]
			crate::bench::add(crate::bench::RUNS, 1);
			#[cfg(feature = "bench")]
			crate::bench::add(crate::bench::CHARS, *n as u32);
			unsafe {
				ts.font(dc, &fr, extra);
				if selected {
					ts.mode(dc, OPAQUE);
					ts.back(dc, SELECTED_BG);
					ts.colour(dc, WHITE);
				} else {
					ts.mode(dc, TRANSPARENT);
					ts.colour(dc, colour_of(app, s, &cfmt));
				}
				TextOut(
					dc,
					(tx + x) as u32,
					(base + shift) as u32,
					np(run.as_ptr()),
					*n as u32,
				);
			}
			*n = 0;
		};
	let mut m = Gdi16 {
		dc: app_measure_dc(),
		grid: false,
	};
	quill_core::layout::walk_line(
		doc,
		s,
		&mut m,
		StoryKind::Body,
		l,
		|p, c, x, w, leader, cf| {
			let selected = sel.is_some_and(|(lo, hi)| p >= lo && p < hi);
			if c == 0x01 {
				flush(&mut n, &run, run_x, run_cf, run_sel, ts);
				if let Some((pw, ph)) = quill_core::layout::picture_size(doc, s, text, p, 96, 96) {
					let idx = text.object_at(s, p).map_or(0, |v| v & 0xFFFF) as u16;
					// The picture, or a grey box with a dark edge the size it prints when it is not drawn (see `picture.rs`).
					if !crate::picture::draw(dc, doc, s, idx, tx + x, base - ph, pw, ph) {
						fill(dc, tx + x, base - ph, pw, ph, SHADOW);
						fill(dc, tx + x + 1, base - ph + 1, pw - 2, ph - 2, FACE);
					}
				} else if let Some(v) = text.object_at(s, p) {
					// A field shows its value: the page it is on, the page count, the date or the file name.
					let mut buf = [0u8; 32];
					// The date and the time are read as they are now, for this paint and no other.
					let n = match v >> 16 {
						k @ (3 | 5) => crate::clock_text(k, &mut buf),
						k => app.field_text(k, l.page, pages, &mut buf),
					};
					let (fr, _, cfmt) = font_ref(doc, s, cf);
					ts.font(dc, &fr, 0);
					ts.mode(dc, TRANSPARENT);
					ts.colour(dc, colour_of(app, s, &cfmt));
					unsafe {
						TextOut(dc, (tx + x) as u32, base as u32, np(buf.as_ptr()), n as u32)
					};
				}
				return;
			}
			if c == 0x09 {
				flush(&mut n, &run, run_x, run_cf, run_sel, ts);
				if w > 0 {
					let (fr, _, cfmt) = font_ref(doc, s, cf);
					ts.font(dc, &fr, 0);
					unsafe {
						if selected {
							fill(dc, tx + x, y_top, w, l.ascent + l.descent, SELECTED_BG);
						}
						if leader != 0 {
							let ch = [match leader {
								1 => b'.',
								2 => b'-',
								_ => b'_',
							}];
							let step = leader_width(&fr, dc, ch[0]);
							ts.mode(dc, TRANSPARENT);
							ts.colour(
								dc,
								if selected {
									WHITE
								} else {
									colour_of(app, s, &cfmt)
								},
							);
							// The leader's characters go out in strings of up to 64, each as far from the last as the characters
							// advance, where each one used to be a `TextOut` of its own.
							let mut dots = [ch[0]; 64];
							let mut dx = 0;
							while dx + step <= w {
								let k = (((w - dx) / step) as usize).min(dots.len());
								TextOut(
									dc,
									(tx + x + dx) as u32,
									base as u32,
									np(dots.as_mut_ptr()),
									k as u32,
								);
								dx += step * k as i32;
							}
						}
					}
				}
				return;
			}
			if n > 0 && (cf != run_cf || selected != run_sel || n == run.len()) {
				flush(&mut n, &run, run_x, run_cf, run_sel, ts);
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
		},
	);
	flush(&mut n, &run, run_x, run_cf, run_sel, ts);
}

/// The memory DC the editor measures with.
fn app_measure_dc() -> u32 {
	unsafe { crate::MEASURE_DC }
}

/// How far past its box a line's drawing may reach (an italic overhang, a picture's edge).
const LINE_MARGIN: i32 = 4;

/// The most lines one paint draws: what a 768-pixel-high screen of 10-point text holds.
const MAX_SHOWN: usize = 48;

const NO_LINE: Line = Line {
	para: 0,
	line: 0,
	start: 0,
	len: 0,
	x: 0,
	indent: 0,
	width: 0,
	ascent: 0,
	descent: 0,
	page: 0,
	y: 0,
};

/// A line that shows, with its list label, collected before any is drawn so the layout's font cache is gone from the stack by then.
#[derive(Copy, Clone)]
struct Shown {
	line: Line,
	label: Option<([u8; 16], usize, usize)>,
}

/// Paints the page view into `area` (the client area under the toolbars and ruler, above the status bar), scrolled `hscroll` pixels
/// sideways, and returns how many lines fit. Only `damage` is drawn: a fill is cut to it and a line it does not reach is not drawn
/// (the clip region would discard the pixels anyway, but every call would still be made).
pub fn paint_page(
	dc: u32,
	area: Rect16,
	damage: Rect16,
	hscroll: i32,
	app: &QApp,
	s: &FarStore,
) -> usize {
	let (left, top, right, bottom) = (
		i32::from(area.left),
		i32::from(area.top),
		i32::from(area.right),
		i32::from(area.bottom),
	);
	let page = &app.doc().page;
	let page_w = view::page_width(app);
	let margin_l = quill_core::layout::to_units(page.margin_l, view::DPI);
	let x0 = left + view::page_x0(right - left, page_w, hscroll);
	let g = PageGeom::of(app, s);
	unsafe {
		let saved = SaveDC(dc);
		IntersectClipRect(dc, left as u32, top as u32, right as u32, bottom as u32);
		let d = &damage;
		fill_in(dc, d, left, top, right - left, bottom - top, SHADOW);
		let mut p = 0;
		loop {
			let y = g.page_y(p, top);
			if y > bottom || p > 400 {
				break;
			}
			if y + g.page_h > top {
				fill_in(dc, d, x0, y, page_w, g.page_h, WHITE);
				// A thin shadow on the right and below, as a sheet on the desk.
				fill_in(dc, d, x0 + page_w, y + 3, 2, g.page_h, BLACK);
				fill_in(dc, d, x0 + 3, y + g.page_h, page_w, 2, BLACK);
			}
			p += 1;
		}
		// The lines that show, with their labels (list numbers count through the whole document, so every line is seen).
		let mut shown = [Shown {
			line: NO_LINE,
			label: None,
		}; MAX_SHOWN];
		let mut count = 0usize;
		let mut fitted = 0usize;
		let mut line_h = 16;
		let first = app.top_line();
		let mut counters: ListCounters = [[0; 9]; 4];
		let pages = app.page_total(s).max(1);
		app.for_each_line_from(s, first, |i, l| {
			let label = line_label(app, s, l, &mut counters, view::DPI);
			if i < first {
				return true;
			}
			let y = g.line_y(l, top);
			if i == first {
				line_h = (l.ascent + l.descent).max(8);
			}
			if y > bottom {
				// Below the window: no later line is shown either.
				return false;
			}
			fitted += 1;
			if let Some(slot) = shown.get_mut(count) {
				*slot = Shown { line: *l, label };
				count += 1;
			}
			true
		});
		let tx = x0 + margin_l;
		SetTextAlign(dc, TA_BASELINE);
		let mut ts = TextState::new();
		for sh in shown.iter().take(count) {
			let y = g.line_y(&sh.line, top);
			// A line is drawn when its box, with a margin for what hangs outside it, reaches the damage.
			if y + sh.line.ascent + sh.line.descent + LINE_MARGIN <= i32::from(damage.top)
				|| y - LINE_MARGIN >= i32::from(damage.bottom)
			{
				continue;
			}
			draw_line(dc, app, s, &sh.line, sh.label, tx, y, pages, &mut ts);
		}
		// The text alignment goes back with the `RestoreDC` below.
		ts.done(dc);
		// The caret: a two-pixel bar the height of its line.
		if app.editing() {
			if let Some((idx, l, x)) = app.caret_xy(s) {
				if idx >= first {
					let y = g.line_y(&l, top);
					fill_in(dc, d, tx + x, y, 2, l.ascent + l.descent, BLACK);
				}
			}
		}
		RestoreDC(dc, saved);
		(((bottom - top) / line_h.max(1)) as usize)
			.max(fitted.min(1))
			.max(1)
	}
}

/// The page's left margin in the window and the line under (`x`, `y`), for a click in a page area whose top is `area_top`.
pub fn hit(
	app: &QApp,
	s: &FarStore,
	area: Rect16,
	hscroll: i32,
	x: i32,
	y: i32,
) -> Option<(u32, i32)> {
	let (left, _) = view::text_extent(app, i32::from(area.right - area.left), hscroll);
	let line = app.line_at_y(s, i32::from(area.top), y)?;
	Some((line, x - i32::from(area.left) - left))
}
