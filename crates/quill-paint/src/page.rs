// The page painter: the laid-out document drawn through a Surface, each glyph at the x walk_chars gives.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A port of the Windows 95 painter's `draw_line`, `paint_page` and `paper_sheet` to [`Surface`]. It draws at 100 %, 96 dots to the
//! inch, with the vertical scroll the app keeps (by line), and reads only the app's public API.

use quill_app::picture::{self, WINDOW};
use quill_app::picture_geom::{self as geom, Frame};
use quill_app::view::{page_x0, PageGeom};
use quill_app::{line_label, App, ListCounters};
use quill_core::doc::{cflag, CFmt, Doc, StoryKind};
use quill_core::layout::{
	font_ref, layout_story, picture_size, to_units, walk_line, Line, Measure,
};
use quill_core::store::Store;
use quill_image::scale::MAX_OUT;

use crate::{Rect, Rgb, Surface};

/// The colour of the desk the sheets lie on.
pub const DESK: Rgb = Rgb(128, 128, 128);
/// The colour of the sheet's shadow.
pub const SHADOW: Rgb = Rgb(64, 64, 64);

/// Where and how the page is shown.
#[derive(Copy, Clone, Debug)]
pub struct PageView {
	/// The part of the surface the page area takes.
	pub area: Rect,
	/// Pixels the page is scrolled sideways.
	pub hscroll: i32,
}

/// What painting worked out.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct PageInfo {
	/// How many lines of the height of the last one drawn fit the page area.
	pub screen_lines: usize,
	/// How many sheets of paper the body takes.
	pub pages: u32,
}

/// Draws one sheet of paper: a black edge, a shadow below and to the right, and the white page.
pub fn paper_sheet<Sf: Surface>(sf: &mut Sf, left: i32, top: i32, w: i32, h: i32) {
	sf.fill_rect(
		Rect::new(left + 3, top + 3, left + w + 3, top + h + 3),
		SHADOW,
	);
	sf.fill_rect(
		Rect::new(left - 1, top - 1, left + w + 1, top + h + 1),
		Rgb::BLACK,
	);
	sf.fill_rect(Rect::new(left, top, left + w, top + h), Rgb::WHITE);
}

fn colour_of<St: Store + ?Sized>(doc: &Doc, s: &St, cf: &CFmt) -> Rgb {
	if cf.colour == 0 {
		return Rgb::BLACK;
	}
	doc.colours
		.get(s, u16::from(cf.colour))
		.map_or(Rgb::BLACK, |c| Rgb(c.r, c.g, c.b))
}

/// Draws one laid-out line: call [`LinePainter::begin`], then [`LinePainter::char`] for each character a walk gives, then
/// [`LinePainter::finish`].
struct LinePainter<'a, M: Measure + Clone, St: Store + ?Sized> {
	doc: &'a Doc,
	s: &'a St,
	app: &'a App<M>,
	kind: StoryKind,
	sel: Option<(u32, u32)>,
	page: u32,
	fields: Option<u32>,
	area: Rect,
	tx: i32,
	y_top: i32,
	ascent: i32,
	descent: i32,
	window: &'a mut [u8; WINDOW],
	run: [(u8, i32); 64],
	n: usize,
	run_cf: u16,
	run_sel: bool,
	run_end: i32,
	/// The misspelt words of this line (Check Spelling as You Type), four at most: first and last position, and the x of the first
	/// and last character as the walk placed them.
	marks: [(u32, u32, i32, i32); 4],
	marked: usize,
}

/// What a line needs to know about where it is drawn and what is around it.
struct LineCtx<'a, M: Measure + Clone, St: Store + ?Sized> {
	doc: &'a Doc,
	s: &'a St,
	app: &'a App<M>,
	kind: StoryKind,
	sel: Option<(u32, u32)>,
	page: u32,
	fields: Option<u32>,
	area: Rect,
	/// Draw the misspelt words' wave: the editing views of the body, not a print preview or a header.
	wave: bool,
}

impl<M: Measure + Clone, St: Store + ?Sized> Clone for LineCtx<'_, M, St> {
	fn clone(&self) -> Self {
		*self
	}
}

impl<M: Measure + Clone, St: Store + ?Sized> Copy for LineCtx<'_, M, St> {}

impl<'a, M: Measure + Clone, St: Store + ?Sized> LinePainter<'a, M, St> {
	fn new(
		cx: &LineCtx<'a, M, St>,
		l: &Line,
		tx: i32,
		y_top: i32,
		window: &'a mut [u8; WINDOW],
	) -> Self {
		let mut lp = LinePainter {
			doc: cx.doc,
			s: cx.s,
			app: cx.app,
			kind: cx.kind,
			sel: cx.sel,
			page: cx.page,
			fields: cx.fields,
			area: cx.area,
			tx,
			y_top,
			ascent: l.ascent,
			descent: l.descent,
			window,
			run: [(0, 0); 64],
			n: 0,
			run_cf: 0,
			run_sel: false,
			run_end: 0,
			marks: [(0, 0, i32::MAX, i32::MIN); 4],
			marked: 0,
		};
		if cx.wave && cx.kind == StoryKind::Body {
			for (st, en) in cx
				.app
				.spell_marks(cx.s, l.start, l.start + l.len)
				.take(lp.marks.len())
			{
				if let Some(m) = lp.marks.get_mut(lp.marked) {
					*m = (st, en, i32::MAX, i32::MIN);
					lp.marked += 1;
				}
			}
		}
		lp
	}

	fn base(&self) -> i32 {
		self.y_top.saturating_add(self.ascent)
	}

	/// Draws `chars` from `x` on the baseline, each at the advance of the one before.
	fn text<Sf: Surface>(&mut self, sf: &mut Sf, chars: &[u8], x: i32, cf: u16, selected: bool) {
		let (fr, _, cfmt) = font_ref(self.doc, self.s, cf);
		let font = sf.font(&fr);
		let mut glyphs = [(0u8, 0i32); 32];
		let mut at = x;
		let mut n = 0;
		for (slot, c) in glyphs.iter_mut().zip(chars) {
			*slot = (*c, at);
			at += sf.advance(font, *c);
			n += 1;
		}
		let fg = if selected {
			sf.fill_rect(
				Rect::new(x, self.y_top, at, self.y_top + self.ascent + self.descent),
				Rgb::NAVY,
			);
			Rgb::WHITE
		} else {
			colour_of(self.doc, self.s, &cfmt)
		};
		if let Some(g) = glyphs.get(..n) {
			sf.glyph_run(font, self.base(), g, fg, !selected);
		}
	}

	fn begin<Sf: Surface>(
		&mut self,
		sf: &mut Sf,
		l: &Line,
		label: Option<([u8; 16], usize, usize)>,
	) {
		if let Some((mut lab, n, first)) = label {
			if lab.first() == Some(&0x07) {
				lab[0] = 0x95;
			}
			let cf = self
				.doc
				.story(self.kind)
				.cfmt_at(self.s, l.start)
				.unwrap_or(0);
			let at = self.tx.saturating_add(first as i32);
			if let Some(text) = lab.get(..n.min(16)) {
				let text = text.get(..text.len().min(32)).unwrap_or(&[]);
				self.text(sf, text, at, cf, false);
			}
		}
	}

	fn flush<Sf: Surface>(&mut self, sf: &mut Sf) {
		if self.n == 0 {
			return;
		}
		let (fr, vpos, cfmt) = font_ref(self.doc, self.s, self.run_cf);
		let font = sf.font(&fr);
		let shift = match vpos {
			1 => -(self.ascent / 3),
			2 => self.ascent / 4,
			_ => 0,
		};
		let base = self.base() + shift;
		let first = self.run.first().map_or(0, |g| g.1);
		let end = self.run_end;
		let fg = if self.run_sel {
			sf.fill_rect(
				Rect::new(
					first,
					self.y_top,
					end,
					self.y_top + self.ascent + self.descent,
				),
				Rgb::NAVY,
			);
			Rgb::WHITE
		} else {
			colour_of(self.doc, self.s, &cfmt)
		};
		if let Some(g) = self.run.get(..self.n) {
			sf.glyph_run(font, base, g, fg, !self.run_sel);
		}
		if cfmt.flags & cflag::UNDERLINE != 0 {
			sf.hline(first, end, base + 1, fg);
		}
		if cfmt.flags & cflag::STRIKE != 0 {
			sf.hline(first, end, base - (self.ascent * 3 + 4) / 8, fg);
		}
		self.n = 0;
	}

	#[allow(clippy::too_many_arguments)]
	fn char<Sf: Surface>(
		&mut self,
		sf: &mut Sf,
		p: u32,
		c: u8,
		x: i32,
		w: i32,
		leader: u8,
		cf: u16,
	) {
		let selected = self.sel.is_some_and(|(lo, hi)| p >= lo && p < hi);
		let ax = self.tx.saturating_add(x);
		for m in self.marks.iter_mut().take(self.marked) {
			if p >= m.0 && p < m.1 {
				m.2 = m.2.min(ax);
				m.3 = m.3.max(ax.saturating_add(w));
			}
		}
		let text = self.doc.story(self.kind);
		if c == 0x01 {
			self.flush(sf);
			if self.picture(sf, p, ax) {
				return;
			}
			if let Some(kind) = text
				.object_at(self.s, p)
				.map(|v| v >> 16)
				.filter(|k| (1..=5).contains(k))
			{
				let mut buf = [0u8; 32];
				let n = match self.fields {
					Some(total) => self.app.field_text(kind, self.page, total, &mut buf),
					None => {
						let name: &[u8] = match kind {
							1 => b"[Page]",
							2 => b"[Pages]",
							3 => b"[Date]",
							5 => b"[Time]",
							_ => b"[File]",
						};
						let n = name.len().min(buf.len());
						if let (Some(d), Some(s)) = (buf.get_mut(..n), name.get(..n)) {
							d.copy_from_slice(s);
						}
						n
					}
				};
				if let Some(t) = buf.get(..n.min(32)) {
					let t: [u8; 32] = {
						let mut a = [0u8; 32];
						a.get_mut(..t.len()).unwrap_or(&mut []).copy_from_slice(t);
						a
					};
					let len = n.min(32);
					self.text(sf, t.get(..len).unwrap_or(&[]), ax, cf, selected);
				}
			}
			return;
		}
		if c == 0x09 {
			self.flush(sf);
			if w > 0 {
				let (fr, _, cfmt) = font_ref(self.doc, self.s, cf);
				let font = sf.font(&fr);
				let fg = if selected {
					sf.fill_rect(
						Rect::new(
							ax,
							self.y_top,
							ax + w,
							self.y_top + self.ascent + self.descent,
						),
						Rgb::NAVY,
					);
					colour_of(self.doc, self.s, &cfmt)
				} else {
					colour_of(self.doc, self.s, &cfmt)
				};
				let ch = match leader {
					1 => b'.',
					2 => b'-',
					3 => b'_',
					_ => 0,
				};
				if ch != 0 {
					let step = sf.advance(font, ch).max(1);
					let mut dx = 0;
					while dx + step <= w {
						sf.glyph_run(font, self.base(), &[(ch, ax + dx)], fg, false);
						dx += step;
					}
				}
			}
			return;
		}
		if self.n > 0 && (cf != self.run_cf || selected != self.run_sel || self.n == self.run.len())
		{
			self.flush(sf);
		}
		if self.n == 0 {
			self.run_cf = cf;
			self.run_sel = selected;
		}
		if let Some(slot) = self.run.get_mut(self.n) {
			*slot = (c, ax);
			self.n += 1;
		}
		self.run_end = ax.saturating_add(w);
	}

	/// Draws the picture at `p` if there is one: a silver box first, then its rows.
	fn picture<Sf: Surface>(&mut self, sf: &mut Sf, p: u32, ax: i32) -> bool {
		let text = self.doc.story(self.kind);
		let Some((pw, ph)) = picture_size(self.doc, self.s, text, p, 96, 96) else {
			return false;
		};
		let idx = text.object_at(self.s, p).map_or(0, |v| v & 0xFFFF) as u16;
		let pw = pw.clamp(1, MAX_OUT as i32);
		let ph = ph.max(1);
		let top = self.base().saturating_sub(ph);
		let r = Rect::new(ax, top, ax.saturating_add(pw), top.saturating_add(ph));
		if r.intersect(self.area).is_empty() {
			return true;
		}
		sf.fill_rect(r, Rgb::SILVER);
		let done = picture::render(
			self.doc,
			self.s,
			idx,
			pw as usize,
			ph as u32,
			self.window,
			|row, rgb| sf.rgb_row(ax, top.saturating_add(ph - 1 - row as i32), rgb),
		);
		if done.is_err() {
			sf.fill_rect(r, Rgb::SILVER);
		}
		self.picture_marks(sf, p, r);
		true
	}

	/// The frame and eight handles of the selected picture, black and outside its pixels; a picture inside a larger selection gets a
	/// navy border inside its edge instead, as selected text has navy behind it.
	fn picture_marks<Sf: Surface>(&self, sf: &mut Sf, p: u32, r: Rect) {
		let Some((lo, hi)) = self.sel.filter(|(lo, hi)| p >= *lo && p < *hi) else {
			return;
		};
		if hi > lo + 1 {
			for k in 0..2 {
				let q = Rect::new(r.left + k, r.top + k, r.right - k, r.bottom - k);
				sf.hline(q.left, q.right, q.top, Rgb::NAVY);
				sf.hline(q.left, q.right, q.bottom - 1, Rgb::NAVY);
				sf.fill_rect(Rect::new(q.left, q.top, q.left + 1, q.bottom), Rgb::NAVY);
				sf.fill_rect(Rect::new(q.right - 1, q.top, q.right, q.bottom), Rgb::NAVY);
			}
			return;
		}
		let rect = Frame {
			left: r.left,
			top: r.top,
			right: r.right,
			bottom: r.bottom,
		};
		let o = geom::outline(&rect);
		let ink = Rgb::BLACK;
		sf.fill_rect(Rect::new(o.left, o.top, o.right, o.top + 1), ink);
		sf.fill_rect(Rect::new(o.left, o.bottom - 1, o.right, o.bottom), ink);
		sf.fill_rect(Rect::new(o.left, o.top, o.left + 1, o.bottom), ink);
		sf.fill_rect(Rect::new(o.right - 1, o.top, o.right, o.bottom), ink);
		for b in geom::handles(&rect, geom::HANDLE) {
			sf.fill_rect(Rect::new(b.left, b.top, b.right, b.bottom), ink);
		}
	}

	fn finish<Sf: Surface>(&mut self, sf: &mut Sf) {
		self.flush(sf);
		// The wave goes in after the text, under the first to the last character of each misspelt word: a zig-zag of two-pixel
		// strokes, a pixel lower on every other one, in the pixels the descent leaves.
		let y = self.base() + 1;
		let drop = i32::from(self.descent > 2);
		for m in self.marks.iter().take(self.marked) {
			let mut x = m.2;
			let mut low = false;
			while x < m.3 {
				let end = (x + 2).min(m.3);
				sf.hline(x, end, y + i32::from(low) * drop, Rgb::RED);
				x = end;
				low = !low;
			}
		}
	}
}

/// Paints the page view into `view.area`: the desk, the sheets, every visible line of the body and the caret. The vertical scroll is
/// the app's own (its first line shown). `window` is the picture decoder's working memory.
pub fn paint_page<Sf: Surface, M: Measure + Clone, St: Store + ?Sized>(
	sf: &mut Sf,
	app: &App<M>,
	s: &St,
	view: &PageView,
	window: &mut [u8; WINDOW],
) -> PageInfo {
	let area = view.area;
	sf.clip(Some(area));
	sf.fill_rect(area, DESK);
	let doc = app.doc();
	let page = &doc.page;
	let page_w = to_units(page.paper_w, 96);
	let page_h = to_units(page.paper_h, 96);
	let margin_l = to_units(page.margin_l, 96);
	let x0 = area.left + page_x0(area.width(), page_w, view.hscroll);
	let g = PageGeom::of(app, s);
	let pages = app.page_total(s).max(1);
	for p in 0..pages as i32 {
		let top = g.page_y(p, area.top);
		if top > area.bottom + 4 {
			break;
		}
		if top.saturating_add(page_h) + 4 < area.top {
			continue;
		}
		paper_sheet(sf, x0, top, page_w, page_h);
	}
	let cx = LineCtx {
		doc,
		s,
		app,
		kind: StoryKind::Body,
		sel: app.selection(),
		page: 1,
		fields: (app.story_view() == 0).then_some(pages),
		area,
		wave: true,
	};
	let tx = x0 + margin_l;
	let mut counters: ListCounters = [[0; 9]; 4];
	let mut line_h = 16;
	app.for_each_line_from(s, app.top_line(), |_, l| {
		let label = line_label(app, s, l, &mut counters, 96);
		let y_top = g.line_y(l, area.top);
		let h = (l.ascent + l.descent).max(1);
		if y_top > area.bottom + 4 {
			// Below the window: nothing later is shown either.
			return false;
		}
		if y_top + h < area.top - 4 {
			return true;
		}
		line_h = h.max(8);
		let cx = LineCtx { page: l.page, ..cx };
		let mut lp = LinePainter::new(&cx, l, tx, y_top, window);
		lp.begin(sf, l, label);
		app.walk_chars(s, l, |p, c, x, w, leader, cf| {
			lp.char(sf, p, c, x, w, leader, cf)
		});
		lp.finish(sf);
		true
	});
	if app.editing() {
		if let Some((_, l, x)) = app.caret_xy(s) {
			let y = g.line_y(&l, area.top);
			sf.fill_rect(
				Rect::new(tx + x, y, tx + x + 2, y + l.ascent + l.descent),
				Rgb::BLACK,
			);
		}
	}
	sf.clip(None);
	PageInfo {
		screen_lines: (area.height() / line_h.max(1)).max(1) as usize,
		pages,
	}
}

/// How many header or footer lines a sheet draws.
const STORY_LINES: usize = 16;

/// What an editing window adds to a sheet: the selection, the caret and the text an input method is composing.
#[derive(Copy, Clone, Debug, Default)]
pub struct Editing<'a> {
	/// Draw the selection and the caret of the body. Off, a sheet is the print preview's.
	pub active: bool,
	/// The caret's on phase: false while it blinks off.
	pub caret: bool,
	/// Text being composed, in Windows-1252, drawn under the caret with an underline.
	pub preedit: &'a [u8],
	/// Where the composing text's own cursor is: a byte offset into `preedit`.
	pub preedit_cursor: usize,
}

/// Paints one whole sheet as the print preview shows it: the paper with its top left corner at (`left`, `top`), the header and
/// footer, and the lines of the body that fall on page `page` of `pages`. `m` is the measure the app was built with (layout of a
/// header needs it; the body reuses the app's own).
#[allow(clippy::too_many_arguments)]
pub fn paint_sheet<Sf: Surface, M: Measure + Clone, St: Store + ?Sized>(
	sf: &mut Sf,
	app: &App<M>,
	m: &mut M,
	s: &St,
	left: i32,
	top: i32,
	page: u32,
	pages: u32,
	area: Rect,
	window: &mut [u8; WINDOW],
) {
	paint_sheet_editing(
		sf,
		app,
		m,
		s,
		left,
		top,
		page,
		pages,
		area,
		window,
		&Editing::default(),
	);
}

/// [`paint_sheet`] with the selection highlighted and the caret drawn on the body, as an editing window shows a sheet. The body is
/// the story being edited only while the app is on the body (not a header or footer), so nothing is drawn otherwise.
#[allow(clippy::too_many_arguments)]
pub fn paint_sheet_editing<Sf: Surface, M: Measure + Clone, St: Store + ?Sized>(
	sf: &mut Sf,
	app: &App<M>,
	m: &mut M,
	s: &St,
	left: i32,
	top: i32,
	page: u32,
	pages: u32,
	area: Rect,
	window: &mut [u8; WINDOW],
	edit: &Editing,
) {
	let on_body = edit.active && app.story_view() == 0;
	sf.clip(Some(area));
	let doc = app.doc();
	let setup = doc.page;
	let u = |tw: i32| to_units(tw, 96);
	paper_sheet(sf, left, top, u(setup.paper_w), u(setup.paper_h));
	let tx = left + u(setup.margin_l);
	let first = page == 1 && setup.flags & 1 != 0;
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
		let mut lines: [Option<Line>; STORY_LINES] = [None; STORY_LINES];
		let mut count = 0;
		layout_story(doc, s, m, kind, |l| {
			if l.page == 1 {
				if let Some(slot) = lines.get_mut(count) {
					*slot = Some(*l);
					count += 1;
				}
			}
		});
		let height = lines
			.get(count.wrapping_sub(1))
			.copied()
			.flatten()
			.map_or(0, |l| l.y + l.ascent + l.descent);
		let y0 = if from_bottom {
			top + u(setup.paper_h) - u(dist) - height
		} else {
			top + u(dist)
		};
		let cx = LineCtx {
			doc,
			s,
			app,
			kind,
			sel: None,
			page,
			fields: Some(pages),
			area,
			wave: false,
		};
		for l in lines.iter().flatten() {
			let mut lp = LinePainter::new(&cx, l, tx, y0 + l.y, window);
			lp.begin(sf, l, None);
			walk_line(doc, s, m, kind, l, |p, c, x, w, leader, cf| {
				lp.char(sf, p, c, x, w, leader, cf)
			});
			lp.finish(sf);
		}
	}
	let cx = LineCtx {
		doc,
		s,
		app,
		kind: StoryKind::Body,
		sel: if on_body { app.selection() } else { None },
		page,
		fields: Some(pages),
		area,
		wave: on_body,
	};
	let mut counters: ListCounters = [[0; 9]; 4];
	app.for_each_line_from_page(s, page, |_, l| {
		let label = line_label(app, s, l, &mut counters, 96);
		if l.page > page {
			return false;
		}
		if l.page != page {
			return true;
		}
		let mut lp = LinePainter::new(&cx, l, tx, top + u(setup.margin_t) + l.y, window);
		lp.begin(sf, l, label);
		app.walk_chars(s, l, |p, c, x, w, leader, cf| {
			lp.char(sf, p, c, x, w, leader, cf)
		});
		lp.finish(sf);
		true
	});
	if on_body && app.editing() {
		draw_caret(
			sf,
			app,
			s,
			left + u(setup.margin_l),
			top + u(setup.margin_t),
			page,
			edit,
		);
	}
	sf.clip(None);
}

/// Draws the caret as two document pixels of black, with the composing text before it: over the text that follows, on white, with
/// an underline.
fn draw_caret<Sf: Surface, M: Measure + Clone, St: Store + ?Sized>(
	sf: &mut Sf,
	app: &App<M>,
	s: &St,
	tx: i32,
	ty: i32,
	page: u32,
	edit: &Editing,
) {
	let Some((_, l, x)) = app.caret_xy(s) else {
		return;
	};
	if l.page != page {
		return;
	}
	let y = ty + l.y;
	let h = l.ascent + l.descent;
	let mut cx = tx + x;
	if !edit.preedit.is_empty() {
		let cf = app
			.doc()
			.body
			.cfmt_at(s, app.caret().saturating_sub(1))
			.unwrap_or(0);
		let (fr, _, cfmt) = font_ref(app.doc(), s, cf);
		let font = sf.font(&fr);
		let mut at = cx;
		let mut glyphs: [(u8, i32); 64] = [(0, 0); 64];
		let mut n = 0;
		let mut to_cursor = cx;
		for (i, c) in edit.preedit.iter().enumerate() {
			if i == edit.preedit_cursor {
				to_cursor = at;
			}
			if let Some(g) = glyphs.get_mut(n) {
				*g = (*c, at);
				n += 1;
			}
			at += sf.advance(font, *c);
		}
		if edit.preedit_cursor >= edit.preedit.len() {
			to_cursor = at;
		}
		sf.fill_rect(Rect::new(cx, y, at, y + h), Rgb::WHITE);
		let base = y + l.ascent;
		let fg = colour_of(app.doc(), s, &cfmt);
		if let Some(g) = glyphs.get(..n) {
			sf.glyph_run(font, base, g, fg, true);
		}
		sf.hline(cx, at, base + 1, Rgb::BLACK);
		cx = to_cursor;
	}
	if edit.caret {
		sf.fill_rect(Rect::new(cx, y, cx + 2, y + h), Rgb::BLACK);
	}
}
