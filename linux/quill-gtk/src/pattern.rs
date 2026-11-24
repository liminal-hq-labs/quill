// The test pattern the spike draws: a one-pixel checkerboard, a pixel ruler and a line of real text.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Every approach in the spike shows the same pixels, so a screenshot can be compared with the source byte for byte. The text is
//! a minimal skrifa outline fed to tiny-skia: unhinted, with advances in twips as the layout plan specifies.

use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::{FontRef, MetadataProvider};
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Transform};

/// The panel's logical size. Both are even so a scale of 1.5 gives whole device pixels.
pub const PANEL_W: i32 = 192;
pub const PANEL_H: i32 = 96;

pub const SAMPLE: &str = "The quick brown fox \u{2014} Quill 12 pt";

const FONT_PATHS: [&str; 3] = [
	"/usr/share/fonts/liberation/LiberationSerif-Regular.ttf",
	"/usr/share/fonts/TTF/LiberationSerif-Regular.ttf",
	"/usr/share/fonts/truetype/liberation/LiberationSerif-Regular.ttf",
];

/// Device pixels for a logical length at a scale, rounded the way the page will round.
pub fn device(logical: i32, scale: f64) -> i32 {
	(f64::from(logical) * scale).round() as i32
}

/// The source image: `(PANEL_W, PANEL_H)` logical, at `scale` device pixels per logical pixel. Premultiplied RGBA, opaque.
pub fn render(scale: f64) -> Pixmap {
	let (w, h) = (device(PANEL_W, scale) as u32, device(PANEL_H, scale) as u32);
	let mut pm = Pixmap::new(w, h).expect("pixmap");
	pm.fill(tiny_skia::Color::from_rgba8(250, 246, 232, 255));
	let stride = w as usize;
	let px = pm.pixels_mut();
	let set = |px: &mut [tiny_skia::PremultipliedColorU8], x: u32, y: u32, rgb: (u8, u8, u8)| {
		if x < w && y < h {
			px[y as usize * stride + x as usize] =
				tiny_skia::ColorU8::from_rgba(rgb.0, rgb.1, rgb.2, 255).premultiply();
		}
	};
	// A one-device-pixel checkerboard in the top left, half the panel wide.
	let (cw, ch) = (w / 2, h / 2 - 8);
	for y in 0..ch {
		for x in 0..cw {
			let v = if (x + y) % 2 == 0 { 0 } else { 255 };
			set(px, x, y, (v, v, v));
		}
	}
	// One-pixel vertical lines every four pixels, top right, then single red and blue lines under them.
	for y in 0..ch {
		for x in (cw + 8)..w.saturating_sub(8) {
			let v = if (x - cw - 8) % 4 == 0 { 0 } else { 255 };
			set(px, x, y, (v, v, v));
		}
	}
	for x in (cw + 8)..w.saturating_sub(8) {
		set(px, x, ch + 2, (200, 0, 0));
		set(px, x, ch + 5, (0, 0, 200));
	}
	// A ruler under the checkerboard: a tick every 10 device pixels, taller every 50.
	for x in 0..cw {
		if x % 10 == 0 {
			let len = if x % 50 == 0 { 12 } else { 6 };
			for y in 0..len {
				set(px, x, ch + 2 + y, (0, 0, 0));
			}
		}
	}
	text(&mut pm, scale);
	pm
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
		self.pb
			.quad_to(self.x + cx0, self.y - cy0, self.x + x, self.y - y);
	}
	fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
		self.pb.cubic_to(
			self.x + cx0,
			self.y - cy0,
			self.x + cx1,
			self.y - cy1,
			self.x + x,
			self.y - y,
		);
	}
	fn close(&mut self) {
		self.pb.close();
	}
}

/// 12 pt at 100 % is 16 px at 96 dpi, so `16 x scale` device pixels. Advances are font units to twips at 24 half-points,
/// rounded once per glyph; the pen position is twips to device pixels, rounded to a whole pixel.
fn text(pm: &mut Pixmap, scale: f64) {
	let Some(data) = std::env::var("QUILL_FONT")
		.ok()
		.into_iter()
		.chain(FONT_PATHS.iter().map(|s| s.to_string()))
		.find_map(|p| std::fs::read(p).ok())
	else {
		eprintln!("spike: no Liberation Serif found (set QUILL_FONT); the text line is left out");
		return;
	};
	let Ok(font) = FontRef::new(&data) else {
		return;
	};
	let upem = f64::from(
		font.metrics(Size::unscaled(), LocationRef::default())
			.units_per_em,
	);
	let px_size = (12.0 * 96.0 / 72.0 * scale) as f32;
	let charmap = font.charmap();
	let metrics = font.glyph_metrics(Size::unscaled(), LocationRef::default());
	let outlines = font.outline_glyphs();
	let baseline = pm.height() as f32 - (6.0 * scale as f32).round();
	let mut paint = Paint::default();
	paint.set_color_rgba8(20, 20, 20, 255);
	paint.anti_alias = true;
	let mut twips: i64 = 0;
	for ch in SAMPLE.chars() {
		let Some(gid) = charmap.map(ch) else { continue };
		let adv = f64::from(metrics.advance_width(gid).unwrap_or(0.0));
		let x =
			(twips as f64 * 96.0 * scale / 1440.0).round() as f32 + (4.0 * scale as f32).round();
		if let Some(glyph) = outlines.get(gid) {
			let mut pen = Pen {
				pb: PathBuilder::new(),
				x,
				y: baseline,
			};
			let _ = glyph.draw(
				DrawSettings::unhinted(Size::new(px_size), LocationRef::default()),
				&mut pen,
			);
			if let Some(path) = pen.pb.finish() {
				pm.fill_path(
					&path,
					&paint,
					FillRule::Winding,
					Transform::identity(),
					None,
				);
			}
		}
		twips += (adv * 10.0 * 24.0 / upem).round() as i64;
	}
}

/// The pixmap as a `gdk::MemoryTexture`.
pub fn texture(pm: &Pixmap) -> gtk::gdk::MemoryTexture {
	let bytes = gtk::glib::Bytes::from(pm.data());
	gtk::gdk::MemoryTexture::new(
		pm.width() as i32,
		pm.height() as i32,
		gtk::gdk::MemoryFormat::R8g8b8a8Premultiplied,
		&bytes,
		pm.width() as usize * 4,
	)
}
