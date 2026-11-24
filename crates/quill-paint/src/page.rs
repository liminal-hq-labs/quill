// The page painter: the laid-out document drawn through a Surface, each glyph at the x walk_chars gives.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A port of the Windows 95 painter's `draw_line`, `paint_page` and `paper_sheet` to [`Surface`]. It draws at 100 %, 96 dots to the
//! inch, with the vertical scroll the app keeps (by line), and reads only the app's public API.

use quill_app::picture::{self, WINDOW};
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
		LinePainter {
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
		}
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
		true
	}

	fn finish<Sf: Surface>(&mut self, sf: &mut Sf) {
		self.flush(sf);
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
		sel: None,
		page,
		fields: Some(pages),
		area,
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
	sf.clip(None);
}
