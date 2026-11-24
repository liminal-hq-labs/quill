// The test frame the spike presents: a one-pixel checkerboard, a pixel ruler and a page of real text.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Every frame is built in device pixels from the panel size, so a screenshot of the window can be compared with the source
//! byte for byte. The text is a minimal skrifa outline fed to tiny-skia: unhinted, with advances in twips as the layout plan
//! specifies. The surround is `#123456`, which the analysis uses to find the frame in a screenshot.

use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::{FontRef, MetadataProvider};
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Rect, Transform};

/// Where the checkerboard starts, in device pixels, on both axes.
pub const MARGIN: u32 = 16;
/// The checkerboard's size in device pixels.
pub const CHECK_W: u32 = 256;
pub const CHECK_H: u32 = 128;

pub const PAGE_LINES: [&str; 6] = [
	"Dear Reader,",
	"The quick brown fox jumps over the lazy dog \u{2014} Quill 12 pt.",
	"Every glyph sits at the position the layout chose, rounded once",
	"from twips to a whole device pixel, so a caret and a letter agree",
	"at 100% and at 150%.",
	"Yours, Quill",
];

const FONT_PATHS: [&str; 4] = [
	"C:\\Windows\\Fonts\\times.ttf",
	"/usr/share/fonts/liberation/LiberationSerif-Regular.ttf",
	"/usr/share/fonts/TTF/LiberationSerif-Regular.ttf",
	"/usr/share/fonts/truetype/liberation/LiberationSerif-Regular.ttf",
];

fn px(rgb: (u8, u8, u8)) -> tiny_skia::PremultipliedColorU8 {
	tiny_skia::ColorU8::from_rgba(rgb.0, rgb.1, rgb.2, 255).premultiply()
}

/// The frame for a panel of `w` by `h` device pixels at `scale` device pixels per DIP.
pub fn render(w: u32, h: u32, scale: f64) -> Pixmap {
	let mut pm = Pixmap::new(w.max(1), h.max(1)).expect("pixmap");
	pm.fill(tiny_skia::Color::from_rgba8(0x12, 0x34, 0x56, 255));
	let stride = pm.width() as usize;
	let (fw, fh) = (pm.width(), pm.height());
	{
		let data = pm.pixels_mut();
		let mut set = |x: u32, y: u32, rgb: (u8, u8, u8)| {
			if x < fw && y < fh {
				data[y as usize * stride + x as usize] = px(rgb);
			}
		};
		// A one-device-pixel checkerboard.
		for y in 0..CHECK_H {
			for x in 0..CHECK_W {
				let v = if (x + y) % 2 == 0 { 0 } else { 255 };
				set(MARGIN + x, MARGIN + y, (v, v, v));
			}
		}
		// One-pixel vertical lines every four pixels beside it, then single red and blue lines under them.
		let lx = MARGIN + CHECK_W + 8;
		for y in 0..CHECK_H {
			for x in 0..64 {
				let v = if x % 4 == 0 { 0 } else { 255 };
				set(lx + x, MARGIN + y, (v, v, v));
			}
		}
		for x in 0..64 {
			set(lx + x, MARGIN + CHECK_H + 2, (200, 0, 0));
			set(lx + x, MARGIN + CHECK_H + 5, (0, 0, 200));
		}
		// A ruler under the checkerboard: a tick every 10 device pixels, taller every 50.
		for x in 0..CHECK_W {
			if x % 10 == 0 {
				let len = if x % 50 == 0 { 12 } else { 6 };
				for y in 0..len {
					set(MARGIN + x, MARGIN + CHECK_H + 2 + y, (255, 255, 255));
				}
			}
		}
	}
	page(&mut pm, scale);
	pm
}

/// A white sheet under the checkerboard with the sample lines on it.
fn page(pm: &mut Pixmap, scale: f64) {
	let top = MARGIN + CHECK_H + 24;
	let left = MARGIN;
	let sheet_w = ((6.0 * 96.0 * scale).round() as u32).min(pm.width().saturating_sub(left * 2));
	let sheet_h = pm.height().saturating_sub(top + MARGIN);
	if sheet_w == 0 || sheet_h == 0 {
		return;
	}
	let mut paint = Paint::default();
	paint.set_color_rgba8(255, 255, 255, 255);
	if let Some(r) = Rect::from_xywh(left as f32, top as f32, sheet_w as f32, sheet_h as f32) {
		pm.fill_rect(r, &paint, Transform::identity(), None);
	}
	let Some(data) = std::env::var("QUILL_FONT")
		.ok()
		.into_iter()
		.chain(FONT_PATHS.iter().map(|s| s.to_string()))
		.find_map(|p| std::fs::read(p).ok())
	else {
		eprintln!("spike: no serif font found (set QUILL_FONT); the page text is left out");
		return;
	};
	let Ok(font) = FontRef::new(&data) else {
		return;
	};
	let upem = f64::from(font.metrics(Size::unscaled(), LocationRef::default()).units_per_em);
	// 12 pt at 100% is 16 px at 96 dpi.
	let px_size = (12.0 * 96.0 / 72.0 * scale) as f32;
	let charmap = font.charmap();
	let metrics = font.glyph_metrics(Size::unscaled(), LocationRef::default());
	let outlines = font.outline_glyphs();
	paint.set_color_rgba8(20, 20, 20, 255);
	paint.anti_alias = true;
	let line_h = (px_size * 1.25).round();
	for (n, line) in PAGE_LINES.iter().enumerate() {
		let baseline = top as f32 + (24.0 * scale as f32).round() + line_h * n as f32;
		if baseline > (top + sheet_h) as f32 {
			break;
		}
		let mut twips: i64 = 0;
		for ch in line.chars() {
			let Some(gid) = charmap.map(ch) else { continue };
			let adv = f64::from(metrics.advance_width(gid).unwrap_or(0.0));
			let x = (twips as f64 * 96.0 * scale / 1440.0).round() as f32 + left as f32 + (24.0 * scale as f32).round();
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
			twips += (adv * 10.0 * 24.0 / upem).round() as i64;
		}
	}
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

/// The pixmap as BGRA bytes, the order a `B8G8R8A8_UNORM` swap chain expects. The frame is opaque, so premultiplying is a no-op.
pub fn bgra(pm: &Pixmap) -> Vec<u8> {
	let mut out = pm.data().to_vec();
	for p in out.as_chunks_mut::<4>().0 {
		p.swap(0, 2);
	}
	out
}
