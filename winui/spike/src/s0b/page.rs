// The page both hostings draw: a white sheet of Times New Roman lines, the caret and an inline composition string.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Drawn by skrifa outlines and tiny-skia as in spike S0a: unhinted, advances rounded once per glyph from twips. The layout
//! it returns (line boxes and per-character x offsets in device pixels) is what places the caret, the input method's windows
//! and the accessibility tree's character positions, so all four agree with the pixels.

use std::sync::OnceLock;

use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::{FontRef, MetadataProvider};
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Rect, Transform};

pub const MARGIN: u32 = 16;
/// The colour around the sheet, which the pixel analysis uses to find the frame in a screenshot.
pub const SURROUND: (u8, u8, u8) = (0x12, 0x34, 0x56);

const FONT_PATHS: [&str; 4] = [
	"C:\\Windows\\Fonts\\times.ttf",
	"/usr/share/fonts/liberation/LiberationSerif-Regular.ttf",
	"/usr/share/fonts/TTF/LiberationSerif-Regular.ttf",
	"/usr/share/fonts/truetype/liberation/LiberationSerif-Regular.ttf",
];

fn font_data() -> Option<&'static [u8]> {
	static DATA: OnceLock<Option<Vec<u8>>> = OnceLock::new();
	DATA.get_or_init(|| {
		std::env::var("QUILL_FONT")
			.ok()
			.into_iter()
			.chain(FONT_PATHS.iter().map(|s| s.to_string()))
			.find_map(|p| std::fs::read(p).ok())
	})
	.as_deref()
}

/// What to draw and where the caret and composition are.
pub struct Scene<'a> {
	pub lines: &'a [String],
	pub caret: (usize, usize),
	pub composition: Option<&'a str>,
	/// Draw the page's own caret (a hosting with a system caret leaves this off).
	pub focused: bool,
	pub scale: f64,
	/// The cursor inside the composition string, in characters.
	pub composition_cursor: usize,
}

/// Where one line sits, in device pixels of the frame.
#[derive(Clone, Debug)]
pub struct LineGeom {
	pub left: f32,
	pub top: f32,
	pub bottom: f32,
	/// Left edge of each character of the document line, relative to `left`, with one more entry for the end.
	pub xs: Vec<f32>,
}

pub struct Frame {
	pub pixmap: Pixmap,
	pub lines: Vec<LineGeom>,
	/// Caret x, top and bottom in device pixels, composition included.
	pub caret: (f32, f32, f32),
	pub line_h: f32,
}

/// Prefix x offsets in device pixels of `text`: advances in twips, each rounded once to a whole pixel.
pub fn char_xs(font: &FontRef, text: &str, scale: f64) -> Vec<f32> {
	let upem = f64::from(font.metrics(Size::unscaled(), LocationRef::default()).units_per_em);
	let charmap = font.charmap();
	let metrics = font.glyph_metrics(Size::unscaled(), LocationRef::default());
	let mut twips: i64 = 0;
	let mut xs = vec![0.0];
	for ch in text.chars() {
		let adv = charmap
			.map(ch)
			.and_then(|g| metrics.advance_width(g))
			.map(f64::from)
			.unwrap_or(upem * 0.6);
		twips += (adv * 10.0 * 24.0 / upem).round() as i64;
		xs.push((twips as f64 * 96.0 * scale / 1440.0).round() as f32);
	}
	xs
}

pub fn render(w: u32, h: u32, scene: &Scene) -> Frame {
	let mut pm = Pixmap::new(w.max(1), h.max(1)).expect("pixmap");
	pm.fill(tiny_skia::Color::from_rgba8(SURROUND.0, SURROUND.1, SURROUND.2, 255));
	let scale = scene.scale;
	let left = MARGIN as f32;
	let top = MARGIN as f32;
	let sheet_w = pm.width().saturating_sub(MARGIN * 2) as f32;
	let sheet_h = pm.height().saturating_sub(MARGIN * 2) as f32;
	let px_size = (12.0 * 96.0 / 72.0 * scale) as f32;
	let line_h = (px_size * 1.25).round();
	let text_left = left + (24.0 * scale as f32).round();
	let first_base = top + (24.0 * scale as f32).round();
	let mut frame = Frame {
		pixmap: Pixmap::new(1, 1).expect("pixmap"),
		lines: Vec::new(),
		caret: (text_left, first_base - line_h * 0.8, first_base + line_h * 0.2),
		line_h,
	};
	let mut paint = Paint::default();
	paint.set_color_rgba8(255, 255, 255, 255);
	if let Some(r) = Rect::from_xywh(left, top, sheet_w.max(1.0), sheet_h.max(1.0)) {
		pm.fill_rect(r, &paint, Transform::identity(), None);
	}
	let font = font_data().and_then(|d| FontRef::new(d).ok());
	let Some(font) = font else {
		frame.pixmap = pm;
		return frame;
	};
	let charmap = font.charmap();
	let outlines = font.outline_glyphs();
	for (n, line) in scene.lines.iter().enumerate() {
		let baseline = first_base + line_h * n as f32;
		let line_top = baseline - line_h * 0.8;
		let doc_xs = char_xs(&font, line, scale);
		// The text actually drawn: the document line with the composition string spliced in at the caret.
		let (display, comp_range) = match (scene.composition, n == scene.caret.0) {
			(Some(c), true) => {
				let at = line.char_indices().nth(scene.caret.1).map(|(i, _)| i).unwrap_or(line.len());
				let mut s = String::new();
				s.push_str(&line[..at]);
				s.push_str(c);
				s.push_str(&line[at..]);
				(s, Some((scene.caret.1, scene.caret.1 + c.chars().count())))
			}
			_ => (line.clone(), None),
		};
		let xs = char_xs(&font, &display, scale);
		for (i, ch) in display.chars().enumerate() {
			let x = text_left + xs[i];
			match charmap.map(ch) {
				Some(gid) => {
					paint.set_color_rgba8(20, 20, 20, 255);
					paint.anti_alias = true;
					if let Some(glyph) = outlines.get(gid) {
						let mut pen = Pen {
							pb: PathBuilder::new(),
							x,
							y: baseline,
						};
						let _ = glyph.draw(DrawSettings::unhinted(Size::new(px_size), LocationRef::default()), &mut pen);
						if let Some(path) = pen.pb.finish() {
							pm.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
						}
					}
				}
				None => {
					// No glyph in this font (a CJK composition string, say): an outlined box stands in for it.
					paint.set_color_rgba8(20, 20, 20, 255);
					paint.anti_alias = false;
					let bw = (xs[i + 1] - xs[i] - 2.0).max(2.0);
					let bh = px_size * 0.7;
					for (rx, ry, rw, rh) in [
						(x + 1.0, baseline - bh, bw, 1.0),
						(x + 1.0, baseline, bw, 1.0),
						(x + 1.0, baseline - bh, 1.0, bh),
						(x + bw, baseline - bh, 1.0, bh),
					] {
						if let Some(r) = Rect::from_xywh(rx, ry, rw, rh) {
							pm.fill_rect(r, &paint, Transform::identity(), None);
						}
					}
				}
			}
		}
		if let Some((a, b)) = comp_range {
			paint.set_color_rgba8(0, 90, 200, 255);
			paint.anti_alias = false;
			let x0 = text_left + xs[a];
			let x1 = text_left + xs[b];
			if let Some(r) = Rect::from_xywh(x0, baseline + 2.0, (x1 - x0).max(1.0), 2.0) {
				pm.fill_rect(r, &paint, Transform::identity(), None);
			}
		}
		if n == scene.caret.0 {
			let col = match comp_range {
				Some((a, _)) => {
					a + scene
						.composition_cursor
						.min(scene.composition.map(|c| c.chars().count()).unwrap_or(0))
				}
				None => scene.caret.1,
			};
			let col = col.min(xs.len() - 1);
			frame.caret = (text_left + xs[col], line_top, line_top + line_h);
		}
		frame.lines.push(LineGeom {
			left: text_left,
			top: line_top,
			bottom: line_top + line_h,
			xs: doc_xs,
		});
	}
	if scene.focused {
		paint.set_color_rgba8(0, 0, 0, 255);
		paint.anti_alias = false;
		let cw = scale.round().max(1.0) as f32;
		if let Some(r) = Rect::from_xywh(frame.caret.0, frame.caret.1, cw, frame.caret.2 - frame.caret.1) {
			pm.fill_rect(r, &paint, Transform::identity(), None);
		}
	}
	frame.pixmap = pm;
	frame
}

struct Pen {
	pb: PathBuilder,
	x: f32,
	y: f32,
}

impl OutlinePen for Pen {
	fn move_to(&mut self, x: f32, y: f32) {
		self.pb.move_to(self.x + x, self.y - y);
	}
	fn line_to(&mut self, x: f32, y: f32) {
		self.pb.line_to(self.x + x, self.y - y);
	}
	fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
		self.pb.quad_to(self.x + cx0, self.y - cy0, self.x + x, self.y - y);
	}
	fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
		self.pb
			.cubic_to(self.x + cx0, self.y - cy0, self.x + cx1, self.y - cy1, self.x + x, self.y - y);
	}
	fn close(&mut self) {
		self.pb.close();
	}
}

/// The pixmap as BGRA bytes, the order a `B8G8R8A8_UNORM` swap chain and a 32-bit DIB expect (the frame is opaque).
pub fn bgra(pm: &Pixmap) -> Vec<u8> {
	let mut out = pm.data().to_vec();
	for p in out.as_chunks_mut::<4>().0 {
		p.swap(0, 2);
	}
	out
}
