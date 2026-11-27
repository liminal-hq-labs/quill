// Property tests over random documents: the painter puts every glyph where the layout did, the caret and the clicks agree with the
// drawing, nothing is drawn outside the clip, both surfaces draw the same picture, and damaged documents never panic.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod common;

use common::*;
use proptest::prelude::*;
use quill_app::picture::WINDOW;
use quill_app::{Command, Key};
use quill_chrome::view::{page_x0, PAGE_GAP};
use quill_core::layout::{font_ref, to_units, FontRef, Line};
use quill_host::HostStore;
use quill_paint::page::{paint_page, paint_sheet, PageView};
use quill_paint::palette::{rgb256, WINDOWS16};
use quill_paint::raster::{Linear8, Planar4, StrikeFont, PLANAR_BYTES};
use quill_paint::{Rect, Rgb, Surface};
use quill_strike::{SliceSource, StrikeMeasure, Strikes};

const CASES: u32 = 10_000;

// ---- a surface that records what it is asked to draw ----

struct Rec<'a> {
	strikes: &'a Strikes<'a>,
	/// Every glyph drawn: character, x, baseline.
	glyphs: Vec<(u8, i32, i32)>,
	rects: Vec<(Rect, Rgb)>,
}

impl<'a> Rec<'a> {
	fn new() -> Rec<'a> {
		Rec {
			strikes: strikes(),
			glyphs: Vec::new(),
			rects: Vec::new(),
		}
	}
}

impl Surface for Rec<'_> {
	type Font = StrikeFont;
	fn size(&self) -> (i32, i32) {
		(i32::MAX, i32::MAX)
	}
	fn clip(&mut self, _: Option<Rect>) {}
	fn font(&mut self, f: &FontRef) -> StrikeFont {
		let (id, scale) = self.strikes.strike_for(f);
		StrikeFont { id, scale, want: 0 }
	}
	fn advance(&mut self, f: StrikeFont, ch: u8) -> i32 {
		i32::from(self.strikes.advance(f.id, ch)) * i32::from(f.scale)
	}
	fn fill_rect(&mut self, r: Rect, c: Rgb) {
		self.rects.push((r, c));
	}
	fn hline(&mut self, x0: i32, x1: i32, y: i32, c: Rgb) {
		self.rects.push((Rect::new(x0, y, x1, y + 1), c));
	}
	fn vline(&mut self, x: i32, y0: i32, y1: i32, c: Rgb) {
		self.rects.push((Rect::new(x, y0, x + 1, y1), c));
	}
	fn glyph_run(&mut self, _: StrikeFont, base_y: i32, glyphs: &[(u8, i32)], _: Rgb, _: bool) {
		for (c, x) in glyphs {
			self.glyphs.push((*c, *x, base_y));
		}
	}
	fn bitmap4(&mut self, _: i32, _: i32, _: &[u8], _: i32, _: i32, _: &[Rgb; 16]) {}
	fn rgb_row(&mut self, _: i32, _: i32, _: &[u8]) {}
	fn invert_rect(&mut self, _: Rect) {}
}

fn paint_big(app: &TApp, s: &HostStore) -> (Rec<'static>, i32) {
	let mut rec = Rec::new();
	let area = Rect::new(0, 0, 1000, 200_000);
	let mut window = Box::new([0u8; WINDOW]);
	paint_page(
		&mut rec,
		app,
		s,
		&PageView {
			area,
			hscroll: 0,
			pct: 100,
		},
		&mut window,
	);
	let page_w = to_units(app.doc().page.paper_w, 96);
	(rec, page_x0(area.width(), page_w, 0))
}

// ---- random documents ----

#[derive(Debug, Clone)]
struct RunSpec {
	face: u8,
	size: u8,
	bold: bool,
	italic: bool,
	underline: bool,
	strike: bool,
	vpos: u8,
	colour: u8,
	tabs: u8,
	text: String,
}

#[derive(Debug, Clone)]
struct ParaSpec {
	align: u8,
	left: i32,
	first: i32,
	right: i32,
	tabs: Vec<(i32, u8, u8)>,
	runs: Vec<RunSpec>,
}

fn run_strategy() -> impl Strategy<Value = RunSpec> {
	(
		(0u8..3, 8u8..=40, any::<bool>(), any::<bool>()),
		(any::<bool>(), any::<bool>(), 0u8..3, 0u8..4),
		0u8..3,
		"[a-zA-Z0-9 ,.!?-]{0,60}",
	)
		.prop_map(
			|((face, size, bold, italic), (underline, strike, vpos, colour), tabs, text)| RunSpec {
				face,
				size,
				bold,
				italic,
				underline,
				strike,
				vpos,
				colour,
				tabs,
				text,
			},
		)
}

fn para_strategy() -> impl Strategy<Value = ParaSpec> {
	(
		0u8..4,
		0i32..2000,
		-600i32..600,
		0i32..1500,
		proptest::collection::vec((200i32..6000, 0u8..4, 0u8..4), 0..3),
		proptest::collection::vec(run_strategy(), 0..4),
	)
		.prop_map(|(align, left, first, right, tabs, runs)| ParaSpec {
			align,
			left,
			first,
			right,
			tabs,
			runs,
		})
}

fn doc_strategy() -> impl Strategy<Value = Vec<ParaSpec>> {
	proptest::collection::vec(para_strategy(), 1..8)
}

fn rtf_of(paras: &[ParaSpec]) -> Vec<u8> {
	let mut r = String::from("{\\rtf1\\ansi\\deff0{\\fonttbl{\\f0\\froman Times New Roman;}{\\f1\\fswiss Arial;}{\\f2\\fmodern Courier New;}}{\\colortbl;\\red255\\green0\\blue0;\\red0\\green128\\blue0;\\red0\\green0\\blue255;}\\paperw12240\\paperh15840\\margl1800\\margr1800\\margt1440\\margb1440\n");
	for p in paras {
		r.push_str(&format!(
			"\\pard\\{}\\li{}\\fi{}\\ri{}",
			["ql", "qc", "qr", "qj"][usize::from(p.align)],
			p.left,
			p.first,
			p.right
		));
		for (pos, kind, leader) in &p.tabs {
			r.push_str(match leader {
				1 => "\\tldot",
				2 => "\\tlhyph",
				3 => "\\tlul",
				_ => "",
			});
			r.push_str(match kind {
				1 => "\\tqc",
				2 => "\\tqr",
				3 => "\\tqdec",
				_ => "",
			});
			r.push_str(&format!("\\tx{pos}"));
		}
		r.push(' ');
		for run in &p.runs {
			r.push_str(&format!("{{\\f{}\\fs{}", run.face, u16::from(run.size) * 2));
			for (on, w) in [
				(run.bold, "\\b"),
				(run.italic, "\\i"),
				(run.underline, "\\ul"),
				(run.strike, "\\strike"),
			] {
				if on {
					r.push_str(w);
				}
			}
			r.push_str(["", "\\super", "\\sub"][usize::from(run.vpos)]);
			if run.colour > 0 {
				r.push_str(&format!("\\cf{}", run.colour));
			}
			r.push(' ');
			for _ in 0..run.tabs {
				r.push_str("\\tab ");
			}
			r.push_str(&run.text);
			r.push('}');
		}
		r.push_str("\\par\n");
	}
	r.push_str("}\n");
	r.into_bytes()
}

fn app_of(paras: &[ParaSpec]) -> (TApp, HostStore) {
	let (app, s, _) = app_with("gen.rtf", &rtf_of(paras)).expect("the generated RTF opens");
	(app, s)
}

/// What a painter should draw for each line, the way the layout walk gives it.
fn expected_glyphs(app: &TApp, s: &HostStore, tx: i32) -> Vec<(u8, i32)> {
	let mut out = Vec::new();
	let mut rec = Rec::new();
	app.for_each_line(s, |_, l| {
		app.walk_chars(s, l, |_, c, x, w, leader, cf| {
			if c == 0x01 {
				return;
			}
			if c == 0x09 {
				let ch = match leader {
					1 => b'.',
					2 => b'-',
					3 => b'_',
					_ => 0,
				};
				if ch != 0 && w > 0 {
					let (fr, _, _) = font_ref(app.doc(), s, cf);
					let f = rec.font(&fr);
					let step = rec.advance(f, ch).max(1);
					let mut dx = 0;
					while dx + step <= w {
						out.push((ch, tx + x + dx));
						dx += step;
					}
				}
				return;
			}
			out.push((c, tx + x));
		});
	});
	out
}

fn stride(app: &TApp) -> i32 {
	to_units(app.doc().page.paper_h, 96) + PAGE_GAP
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(CASES))]

	/// Every glyph is drawn at the x the layout walk gives it (tab leaders at their steps), in the order of the walk.
	#[test]
	fn glyph_x_is_the_walk_x(paras in doc_strategy()) {
		let (app, s) = app_of(&paras);
		let (rec, x0) = paint_big(&app, &s);
		let tx = x0 + to_units(app.doc().page.margin_l, 96);
		let want = expected_glyphs(&app, &s, tx);
		let got: Vec<(u8, i32)> = rec.glyphs.iter().map(|g| (g.0, g.1)).collect();
		prop_assert_eq!(got, want);
	}

	/// The caret is drawn where `caret_xy` puts it, and `pos_at_point` at that point gives the position back. (The end of a wrapped
	/// line is also the start of the next; the caret is drawn on the next, and that is the line the point is on.)
	#[test]
	fn the_caret_and_pos_at_point_agree(paras in doc_strategy(), frac in 0u32..1000) {
		let (mut app, s) = app_of(&paras);
		let len = app.doc().body.len().saturating_sub(1);
		let pos = if len == 0 { 0 } else { (u64::from(frac) * u64::from(len) / 1000) as u32 };
		app.place_caret(&s, pos, false);
		let st = stride(&app);
		let page = &app.doc().page;
		let (_, l, x) = app.caret_xy(&s).expect("the caret is on a line");
		let px = to_units(page.margin_l, 96) + x;
		let top = (l.page as i32 - 1) * st + to_units(page.margin_t, 96) + l.y;
		let y = top + (l.ascent + l.descent) / 2;
		// At the end of a wrapped line the caret sits after the last character, but a click there stops short of the start of the next
		// line, because the end of one line is the start of the other (`pos_at_point` says so); every other position round trips.
		let wrapped_end = pos == l.start + l.len && !matches!(app.doc().body.read_byte(&s, pos), Some(0x0D | 0x0B) | None);
		// Where the character before has no width, two positions share the edge and a click belongs to the first.
		let mut zero_before = false;
		app.walk_chars(&s, &l, |p, _, _, w, _, _| zero_before |= p + 1 == pos && w == 0);
		if !wrapped_end && !zero_before {
			prop_assert_eq!(app.pos_at_point(&s, px, y, st), pos, "pos_at_point of the caret's point; line {:?} x {} byte {:?}", l, x, app.doc().body.read_byte(&s, pos));
		}
		prop_assert_eq!(app.point_of(&s, pos, st).map(|p| p.0).is_some(), true);
		let (rec, x0) = paint_big(&app, &s);
		let caret = rec.rects.iter().find(|(r, c)| *c == Rgb::BLACK && r.width() == 2 && r.left == x0 + px);
		prop_assert!(caret.is_some(), "no caret drawn at x {} for position {}", x0 + px, pos);
	}

	/// A click left of a glyph's half-advance puts the caret before it, and right of it after it.
	#[test]
	fn clicks_either_side_of_the_half_advance(paras in doc_strategy()) {
		let (app, s) = app_of(&paras);
		let st = stride(&app);
		let margin = to_units(app.doc().page.margin_l, 96);
		let mut lines: Vec<Line> = Vec::new();
		app.for_each_line(&s, |_, l| lines.push(*l));
		// Each click lays the whole document out again, so a few characters of a few lines are tried, spread over the document.
		let step = lines.len().div_ceil(4).max(1);
		for l in lines.into_iter().step_by(step) {
			let mut chars: Vec<(u32, u8, i32, i32)> = Vec::new();
			app.walk_chars(&s, &l, |p, c, x, w, _, _| chars.push((p, c, x, w)));
			let page = &app.doc().page;
			let y = (l.page as i32 - 1) * st + to_units(page.margin_t, 96) + l.y + (l.ascent + l.descent) / 2;
			let mut prev_w = 1;
			let marked: Vec<(u32, u8, i32, i32, bool)> = chars
				.into_iter()
				.map(|(p, c, x, w)| (p, c, x, w, std::mem::replace(&mut prev_w, w) == 0))
				.collect();
			let stepc = marked.len().div_ceil(4).max(1);
			for (p, c, x, w, shared) in marked.into_iter().step_by(stepc) {
				// A character after a zero-width one shares its edge, and a click there belongs to the first of them.
				if c == 0x09 || c == 0x01 || w < 2 || shared {
					continue;
				}
				let left = app.pos_at_point(&s, margin + x + w / 2 - 1, y, st);
				prop_assert_eq!(left, p, "left of the middle of {:?} at {} wide {}", c as char, x, w);
				if p + 1 < l.start + l.len {
					let right = app.pos_at_point(&s, margin + x + w / 2 + 1, y, st);
					prop_assert_eq!(right, p + 1, "right of the middle of {:?} at {} wide {}", c as char, x, w);
				}
			}
		}
	}

	/// The page painted on both surfaces, in a page area that is not the whole surface, leaves everything outside it untouched.
	#[test]
	fn the_page_stays_inside_its_area(
		paras in doc_strategy(),
		(l, t, w, h) in (0i32..400, 0i32..300, 1i32..640, 1i32..480),
		hscroll in 0i32..300,
		seed in any::<u8>(),
		sel in any::<bool>(),
	) {
		let (mut app, mut s) = app_of(&paras);
		if sel {
			let mut f = quill_host::script::MemFiles::default();
			app.command(&mut s, &mut f, Command::SelectAll);
		}
		let area = Rect::new(l, t, (l + w).min(640), (t + h).min(480));
		let before = vec![seed; PLANAR_BYTES];
		let mut buf = before.clone();
		let mut src = SliceSource(font_bytes());
		let mut window = Box::new([0u8; WINDOW]);
		{
			let mut sf = Planar4::new(&mut buf, *strikes(), &mut src).unwrap();
			paint_page(&mut sf, &app, &s, &PageView { area, hscroll, pct: 100 }, &mut window);
		}
		prop_assert!(outside_planar_unchanged(&before, &buf, area));
		let mut lin = vec![seed; 640 * 480];
		{
			let mut sf = Linear8::new(&mut lin, 640, 480, 640, *strikes(), &mut src).unwrap();
			paint_page(&mut sf, &app, &s, &PageView { area, hscroll, pct: 100 }, &mut window);
		}
		for y in 0..480usize {
			let row = &lin[y * 640..y * 640 + 640];
			let inside_row = (y as i32) >= area.top && (y as i32) < area.bottom;
			let ok = if inside_row {
				row[..area.left.max(0) as usize].iter().chain(&row[area.right.max(0) as usize..]).all(|v| *v == seed)
			} else {
				row.iter().all(|v| *v == seed)
			};
			prop_assert!(ok, "row {} outside {:?}", y, area);
		}
	}
}

/// True when every pixel outside `area` is the same in the two planar buffers.
fn outside_planar_unchanged(before: &[u8], after: &[u8], area: Rect) -> bool {
	for y in 0..480i32 {
		let inside_row = y >= area.top && y < area.bottom;
		for p in 0..4usize {
			let row = p * 38400 + y as usize * 80;
			let (a, b) = (&before[row..row + 80], &after[row..row + 80]);
			if !inside_row {
				if a != b {
					return false;
				}
				continue;
			}
			for byte in 0..80usize {
				let mut outside = 0u8;
				for bit in 0..8 {
					let x = (byte * 8 + bit) as i32;
					if x < area.left || x >= area.right {
						outside |= 0x80 >> bit;
					}
				}
				if (a[byte] ^ b[byte]) & outside != 0 {
					return false;
				}
			}
		}
	}
	true
}

// ---- random drawing calls ----

#[derive(Debug, Clone)]
enum Op {
	Fill(Rect, u8),
	HLine(i32, i32, i32, u8),
	VLine(i32, i32, i32, u8),
	Glyphs(u16, u8, bool, i32, Vec<(u8, i32)>, u8),
	Bitmap(i32, i32, i32, i32, Vec<u8>),
	Row(i32, i32, Vec<u8>),
	Invert(Rect),
}

fn rect_strategy() -> impl Strategy<Value = Rect> {
	(-50i32..700, -50i32..520, -20i32..300, -20i32..300)
		.prop_map(|(l, t, w, h)| Rect::new(l, t, l + w, t + h))
}

fn op_strategy(with_invert_and_rows: bool) -> BoxedStrategy<Op> {
	let c = 0u8..16;
	let mut v: Vec<BoxedStrategy<Op>> = vec![
		(rect_strategy(), c.clone())
			.prop_map(|(r, c)| Op::Fill(r, c))
			.boxed(),
		(-50i32..700, -50i32..700, -10i32..500, c.clone())
			.prop_map(|(a, b, y, c)| Op::HLine(a, b, y, c))
			.boxed(),
		(-10i32..700, -50i32..520, -50i32..520, c.clone())
			.prop_map(|(x, a, b, c)| Op::VLine(x, a, b, c))
			.boxed(),
		(
			(1u16..100, 0u8..4, 1u8..4, any::<bool>()),
			-40i32..700,
			proptest::collection::vec((0x20u8..=0xFF, -40i32..700), 0..30),
			c.clone(),
		)
			.prop_map(|((size, family, flags, smooth), base, g, c)| {
				Op::Glyphs(
					size * 2,
					family,
					smooth,
					base.min(520),
					g,
					c + 16 * (flags & 3),
				)
			})
			.boxed(),
		(
			-30i32..660,
			-30i32..500,
			0i32..40,
			0i32..40,
			proptest::collection::vec(any::<u8>(), 0..800),
		)
			.prop_map(|(x, y, w, h, b)| Op::Bitmap(x, y, w, h, b))
			.boxed(),
	];
	if with_invert_and_rows {
		v.push(rect_strategy().prop_map(Op::Invert).boxed());
		v.push(
			(
				-300i32..700,
				-10i32..500,
				proptest::collection::vec(any::<u8>(), 0..900),
			)
				.prop_map(|(x, y, b)| Op::Row(x, y, b))
				.boxed(),
		);
	}
	proptest::strategy::Union::new(v).boxed()
}

fn win(i: u8) -> Rgb {
	WINDOWS16[usize::from(i & 15)]
}

fn apply<Sf: Surface>(sf: &mut Sf, op: &Op) {
	match op {
		Op::Fill(r, c) => sf.fill_rect(*r, win(*c)),
		Op::HLine(a, b, y, c) => sf.hline(*a, *b, *y, win(*c)),
		Op::VLine(x, a, b, c) => sf.vline(*x, *a, *b, win(*c)),
		Op::Glyphs(size, family, smooth, base, g, c) => {
			let fr = font(*family, *size, u16::from(*c >> 4));
			let f = sf.font(&fr);
			sf.glyph_run(f, *base, g, win(*c), *smooth);
		}
		Op::Bitmap(x, y, w, h, bytes) => {
			let pal: [Rgb; 16] = core::array::from_fn(|i| WINDOWS16[(i * 7 + 3) % 16]);
			sf.bitmap4(*x, *y, bytes, *w, *h, &pal);
		}
		Op::Row(x, y, b) => sf.rgb_row(*x, *y, b),
		Op::Invert(r) => sf.invert_rect(*r),
	}
}

fn font(family: u8, size: u16, flags: u16) -> FontRef {
	FontRef {
		face: [0; 31],
		face_len: 0,
		family,
		size,
		flags,
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(CASES))]

	/// Whatever is drawn with whatever clip, no pixel outside the clip changes, on the planar surface.
	#[test]
	fn nothing_is_drawn_outside_the_clip_planar(
		clip in rect_strategy(),
		ops in proptest::collection::vec(op_strategy(true), 1..8),
		seed in any::<u8>(),
	) {
		let before: Vec<u8> = (0..PLANAR_BYTES).map(|i| seed.rotate_left((i / 38400) as u32 * 2) ^ (i as u8 & 0x0F)).collect();
		let mut buf = before.clone();
		let mut src = SliceSource(font_bytes());
		{
			let mut sf = Planar4::new(&mut buf, *strikes(), &mut src).unwrap();
			sf.clip(Some(clip));
			for op in &ops {
				apply(&mut sf, op);
			}
		}
		prop_assert!(outside_planar_unchanged(&before, &buf, clip.intersect(Rect::new(0, 0, 640, 480))));
	}

	/// The same on the linear surface, whatever its size and pitch.
	#[test]
	fn nothing_is_drawn_outside_the_clip_linear(
		clip in rect_strategy(),
		ops in proptest::collection::vec(op_strategy(true), 1..8),
		(w, h, extra) in (1i32..300, 1i32..200, 0usize..7),
		seed in any::<u8>(),
	) {
		let pitch = w as usize + extra;
		let before: Vec<u8> = (0..pitch * h as usize).map(|i| seed ^ (i as u8)).collect();
		let mut buf = before.clone();
		let mut src = SliceSource(font_bytes());
		{
			let mut sf = Linear8::new(&mut buf, w, h, pitch, *strikes(), &mut src).unwrap();
			sf.clip(Some(clip));
			for op in &ops {
				apply(&mut sf, op);
			}
		}
		let c = clip.intersect(Rect::new(0, 0, w, h));
		for y in 0..h {
			for x in 0..pitch as i32 {
				let inside = x < w && x >= c.left && x < c.right && y >= c.top && y < c.bottom;
				if !inside {
					prop_assert_eq!(buf[y as usize * pitch + x as usize], before[y as usize * pitch + x as usize], "({}, {})", x, y);
				}
			}
		}
	}

	/// With only the Windows colours, the two surfaces draw the same picture, pixel for pixel.
	#[test]
	fn both_surfaces_draw_the_same_picture(
		clip in rect_strategy(),
		ops in proptest::collection::vec(op_strategy(false), 1..8),
	) {
		let mut src = SliceSource(font_bytes());
		let mut planes = vec![0u8; PLANAR_BYTES];
		let mut lin = vec![0u8; 640 * 480];
		{
			let mut a = Planar4::new(&mut planes, *strikes(), &mut src).unwrap();
			a.clip(Some(clip));
			for op in &ops {
				apply(&mut a, op);
			}
		}
		let mut src2 = SliceSource(font_bytes());
		{
			let mut b = Linear8::new(&mut lin, 640, 480, 640, *strikes(), &mut src2).unwrap();
			b.clip(Some(clip));
			for op in &ops {
				apply(&mut b, op);
			}
		}
		let pa = planar_pixels(&planes);
		let same = pa.iter().zip(&lin).all(|(a, b)| WINDOWS16[usize::from(*a)] == rgb256(*b));
		if !same {
			let i = pa.iter().zip(&lin).position(|(a, b)| WINDOWS16[usize::from(*a)] != rgb256(*b)).unwrap_or(0);
			prop_assert!(false, "pixel ({}, {}): planar {:?}, linear {:?}", i % 640, i / 640, WINDOWS16[usize::from(pa[i])], rgb256(lin[i]));
		}
	}
}

// ---- damaged documents ----

fn originals() -> Vec<(&'static str, Vec<u8>)> {
	let docs = root().join("tests/docs");
	[
		"letter.rtf",
		"lists.rtf",
		"headers.rtf",
		"pictures.rtf",
		"letter.qld",
		"plain.qld",
		"print3.rtf",
	]
	.into_iter()
	.map(|n| (n, std::fs::read(docs.join(n)).unwrap()))
	.chain(std::iter::once(("long.rtf", long_rtf())))
	.collect()
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(CASES))]

	/// A document with bytes overwritten, inserted or cut off either does not open or opens and paints, on a page, a sheet and the
	/// header story, without a panic.
	#[test]
	fn damaged_documents_never_panic(
		which in 0usize..8,
		edits in proptest::collection::vec((any::<u32>(), any::<u8>(), 0u8..3), 0..10),
		cut in proptest::option::of(any::<u32>()),
		select in any::<bool>(),
		keys in proptest::collection::vec(0u8..6, 0..4),
	) {
		let all = originals_cached();
		let (name, orig) = &all[which % all.len()];
		let mut bytes = orig.clone();
		for (at, v, how) in edits {
			if bytes.is_empty() {
				break;
			}
			let i = at as usize % bytes.len();
			match how {
				0 => bytes[i] = v,
				1 => bytes.insert(i, v),
				_ => {
					bytes.remove(i);
				}
			}
		}
		if let Some(c) = cut {
			let n = bytes.len();
			bytes.truncate(c as usize % (n + 1));
		}
		let Some((mut app, mut s, mut f)) = app_with(name, &bytes) else {
			return Ok(());
		};
		app.set_view_height(382);
		if select {
			app.command(&mut s, &mut f, Command::SelectAll);
		}
		for k in keys {
			match k {
				0 => app.key(&mut s, &mut f, Key::PageDown),
				1 => app.key(&mut s, &mut f, Key::CtrlEnd),
				2 => app.command(&mut s, &mut f, Command::View(1)),
				3 => app.command(&mut s, &mut f, Command::View(2)),
				4 => app.command(&mut s, &mut f, Command::View(0)),
				_ => app.key(&mut s, &mut f, Key::Down),
			}
		}
		let mut window = Box::new([0u8; WINDOW]);
		let mut src = SliceSource(font_bytes());
		let mut lin = vec![0u8; 640 * 480];
		let mut sf = Linear8::new(&mut lin, 640, 480, 640, *strikes(), &mut src).unwrap();
		paint_page(&mut sf, &app, &s, &view(), &mut window);
		let mut m = StrikeMeasure::new(strikes());
		for page in 1..=2 {
			paint_sheet(&mut sf, &app, &mut m, &s, 8, 8, page, 2, Rect::new(0, 0, 640, 480), &mut window);
		}
		let mut rec = Rec::new();
		paint_page(&mut rec, &app, &s, &PageView { area: Rect::new(-100, -100, 5000, 5000), hscroll: 7, pct: 100 }, &mut window);
	}
}

fn originals_cached() -> &'static Vec<(&'static str, Vec<u8>)> {
	static C: std::sync::OnceLock<Vec<(&'static str, Vec<u8>)>> = std::sync::OnceLock::new();
	C.get_or_init(originals)
}
