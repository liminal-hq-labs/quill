// The RGBA pixel surface: one fixed page drawn at six device scales, byte-compared with PNG goldens, and checks of the surface's parts.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![allow(clippy::chunks_exact_to_as_chunks)]

use std::fmt::Write as _;
use std::path::PathBuf;

use quill_app::picture::WINDOW;
use quill_app::App;
use quill_core::layout::FontRef;
use quill_fonts::{FontSet, MeasureGlyphs, OutlineMeasure};
use quill_host::png::encode_rgb;
use quill_host::script::MemFiles;
use quill_host::HostStore;
use quill_paint::page::paint_sheet;
use quill_paint::pixel::{ByteOrder, PixelBuffer, PixelSurface, Scale};
use quill_paint::{crc32, Rect, Rgb, Surface};

/// Logical size of the test sheet's window: a 3.33 by 2.5 inch page and an 8 pixel desk around it.
const LOGICAL: (i32, i32) = (336, 256);

/// Scales in percent, and the file name of each.
const SCALES: [(u32, &str); 6] = [
	(100, "1.00"),
	(125, "1.25"),
	(150, "1.50"),
	(175, "1.75"),
	(200, "2.00"),
	(300, "3.00"),
];

fn root() -> PathBuf {
	PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn compare_bytes(path: &str, actual: &[u8]) {
	let file = root().join(path);
	if std::env::var_os("QUILL_UPDATE_GOLDEN").is_some() {
		std::fs::create_dir_all(file.parent().unwrap()).unwrap();
		std::fs::write(&file, actual).unwrap();
		return;
	}
	let want = std::fs::read(&file)
		.unwrap_or_else(|_| panic!("missing golden {path}; run with QUILL_UPDATE_GOLDEN=1"));
	assert!(
		want == actual,
		"{path} differs; QUILL_UPDATE_GOLDEN=1 rewrites it and the diff is reviewed like code"
	);
}

fn measure() -> OutlineMeasure {
	OutlineMeasure::new(FontSet::vendored()).with_resolution(96)
}

/// The page: formatting of every kind the surface draws, and the picture of `pictures.rtf`.
fn page_rtf() -> Vec<u8> {
	let pictures = std::fs::read_to_string(root().join("tests/docs/pictures.rtf")).unwrap();
	let at = pictures.find("{\\pict").unwrap();
	let mut depth = 0;
	let mut end = at;
	for (i, c) in pictures[at..].char_indices() {
		match c {
			'{' => depth += 1,
			'}' => {
				depth -= 1;
				if depth == 0 {
					end = at + i + 1;
					break;
				}
			}
			_ => {}
		}
	}
	let pict = &pictures[at..end];
	format!(
		"{{\\rtf1\\ansi\\deff0{{\\fonttbl{{\\f0\\froman Times New Roman;}}{{\\f1\\fswiss Arial;}}{{\\f2\\fmodern Courier New;}}}}\
		{{\\colortbl ;\\red200\\green0\\blue0;\\red0\\green0\\blue200;}}\
		\\paperw4800\\paperh3600\\margl360\\margr360\\margt360\\margb360\n\
		\\f1\\fs28\\b Quill pixels\\b0\\par\n\
		\\f0\\fs20 Plain, \\b bold\\b0 , \\i italic\\i0 , \\ul underlined\\ulnone  and \\strike struck\\strike0  text, \
		a \\cf1 red\\cf0  and a \\cf2 blue\\cf0  word, x{{\\super 2}} and x{{\\sub i}}.\\par\n\
		\\f2\\fs18 Courier: 0123456789 gjpqy\\par\n\
		\\f1\\fs16 Arial at eight points, the quick brown fox jumps over the lazy dog.\\par\n\
		\\f0\\fs20 {pict}\\par\n}}"
	)
	.into_bytes()
}

type TApp = App<OutlineMeasure>;

fn open_page() -> (TApp, HostStore) {
	let mut s = HostStore::new();
	let mut app = App::with_measure(measure());
	app.reset(&mut s).unwrap();
	let mut f = MemFiles::default();
	f.files.insert("PAGE.RTF".into(), page_rtf());
	app.open(&mut s, &mut f, b"PAGE.RTF").unwrap();
	(app, s)
}

fn device(scale: Scale) -> (u32, u32) {
	(scale.dev(LOGICAL.0) as u32, scale.dev(LOGICAL.1) as u32)
}

fn draw_page(percent: u32) -> PixelBuffer {
	let scale = Scale::from_percent(percent);
	let (w, h) = device(scale);
	let (app, s) = open_page();
	let mut m = measure();
	let m2 = measure();
	let mut buf = PixelBuffer::new(w, h);
	let mut sf = buf.surface(ByteOrder::Rgba, scale, MeasureGlyphs::new(&m2));
	sf.fill_rect(Rect::new(0, 0, LOGICAL.0, LOGICAL.1), Rgb::SILVER);
	let mut window = Box::new([0u8; WINDOW]);
	let mut pages = 1;
	app.for_each_line(&s, |_, l| pages = pages.max(l.page));
	paint_sheet(
		&mut sf,
		&app,
		&mut m,
		&s,
		8,
		8,
		1,
		pages,
		Rect::new(0, 0, LOGICAL.0, LOGICAL.1),
		&mut window,
	);
	drop(sf);
	buf
}

fn to_rgb(buf: &PixelBuffer) -> Vec<u8> {
	buf.bytes()
		.chunks_exact(4)
		.flat_map(|p| [p[0], p[1], p[2]])
		.collect()
}

#[test]
fn page_goldens_at_every_scale() {
	let mut sums = String::new();
	for (percent, name) in SCALES {
		let buf = draw_page(percent);
		let rgb = to_rgb(&buf);
		writeln!(
			sums,
			"scale {name} {}x{} crc32 {:08x}",
			buf.width(),
			buf.height(),
			crc32(buf.bytes())
		)
		.unwrap();
		compare_bytes(
			&format!("tests/golden/pixel/page.{name}.png"),
			&encode_rgb(buf.width(), buf.height(), &rgb),
		);
	}
	compare_bytes("tests/golden/pixel/page.sums", sums.as_bytes());
}

#[test]
fn page_is_opaque_where_it_is_painted_and_has_ink() {
	for (percent, _) in SCALES {
		let buf = draw_page(percent);
		let px: Vec<&[u8]> = buf.bytes().chunks_exact(4).collect();
		assert!(
			px.iter().all(|p| p[3] == 255),
			"scale {percent}: a pixel is not opaque"
		);
		let dark = px
			.iter()
			.filter(|p| p[0] < 100 && p[1] < 100 && p[2] < 100)
			.count();
		let (w, h) = (buf.width() as usize, buf.height() as usize);
		assert!(
			dark > w * h / 200,
			"scale {percent}: too little ink ({dark})"
		);
	}
}

/// Text is drawn at the same logical place at every scale, so its ink bounds scale with the device scale.
#[test]
fn ink_bounds_follow_the_scale() {
	let bounds = |percent: u32| {
		let buf = draw_page(percent);
		let w = buf.width() as usize;
		let (mut x0, mut x1, mut y0, mut y1) = (usize::MAX, 0, usize::MAX, 0);
		for (i, p) in buf.bytes().chunks_exact(4).enumerate() {
			if p[0] < 90 && p[1] < 90 && p[2] < 90 {
				let (x, y) = (i % w, i / w);
				x0 = x0.min(x);
				x1 = x1.max(x);
				y0 = y0.min(y);
				y1 = y1.max(y);
			}
		}
		(x0 as f64, x1 as f64, y0 as f64, y1 as f64)
	};
	let one = bounds(100);
	for (percent, _) in SCALES {
		let b = bounds(percent);
		let k = percent as f64 / 100.0;
		for (got, want) in [b.0, b.1, b.2, b.3]
			.into_iter()
			.zip([one.0, one.1, one.2, one.3])
		{
			assert!(
				(got - want * k).abs() <= 3.0 * k + 2.0,
				"scale {percent}: edge {got} against {}",
				want * k
			);
		}
	}
}

fn serif() -> FontRef {
	let mut face = [0u8; 31];
	face[..15].copy_from_slice(b"Times New Roman");
	FontRef {
		face,
		face_len: 15,
		family: 1,
		size: 48,
		flags: 0,
	}
}

fn px(buf: &PixelBuffer, x: u32, y: u32) -> [u8; 4] {
	let i = (y * buf.width() + x) as usize * 4;
	buf.bytes()[i..i + 4].try_into().unwrap()
}

#[test]
fn fills_are_clipped_and_tile_at_fractional_scales() {
	let m = measure();
	for percent in [100, 125, 150, 175, 300] {
		let scale = Scale::from_percent(percent);
		let mut buf = PixelBuffer::new(60, 60);
		let mut sf = buf.surface(ByteOrder::Rgba, scale, MeasureGlyphs::new(&m));
		sf.clip(Some(Rect::new(2, 2, 10, 10)));
		sf.fill_rect(Rect::new(-5, -5, 100, 5), Rgb(10, 20, 30));
		sf.fill_rect(Rect::new(5, 5, 8, 8), Rgb(200, 100, 50));
		drop(sf);
		// Nothing outside the clip, and shared edges leave no gap or overlap in the filled area.
		let (l, t, r, b) = (scale.dev(2), scale.dev(2), scale.dev(10), scale.dev(10));
		for y in 0..60u32 {
			for x in 0..60u32 {
				let inside = (x as i32) >= l && (x as i32) < r && (y as i32) >= t && (y as i32) < b;
				let p = px(&buf, x, y);
				if !inside {
					assert_eq!(
						p,
						[0, 0, 0, 0],
						"scale {percent}: ({x},{y}) is outside the clip"
					);
				}
			}
		}
		assert_eq!(px(&buf, l as u32, t as u32), [10, 20, 30, 255]);
		assert_eq!(
			px(&buf, scale.dev(6) as u32, scale.dev(6) as u32),
			[200, 100, 50, 255]
		);
	}
}

#[test]
fn bgra_swaps_red_and_blue_and_stride_is_respected() {
	let m = measure();
	let mut data = vec![0u8; 20 * 8 * 4];
	let mut sf = PixelSurface::new(
		&mut data,
		16,
		8,
		20 * 4,
		ByteOrder::Bgra,
		Scale::ONE,
		MeasureGlyphs::new(&m),
	)
	.unwrap();
	sf.fill_rect(Rect::new(0, 0, 100, 100), Rgb(1, 2, 3));
	drop(sf);
	assert_eq!(&data[..4], &[3, 2, 1, 255]);
	// The four padding pixels at the end of each row stay untouched.
	assert_eq!(&data[16 * 4..20 * 4], &[0; 16]);
	let mut small = vec![0u8; 10];
	assert!(PixelSurface::new(
		&mut small,
		4,
		4,
		16,
		ByteOrder::Rgba,
		Scale::ONE,
		MeasureGlyphs::new(&m)
	)
	.is_none());
}

#[test]
fn lines_are_one_logical_pixel_thick() {
	let m = measure();
	for (percent, thick) in [(100, 1), (125, 1), (150, 2), (175, 2), (200, 2), (300, 3)] {
		let scale = Scale::from_percent(percent);
		let mut buf = PixelBuffer::new(80, 80);
		let mut sf = buf.surface(ByteOrder::Rgba, scale, MeasureGlyphs::new(&m));
		sf.hline(2, 12, 5, Rgb::BLACK);
		sf.vline(3, 0, 6, Rgb::BLACK);
		sf.underline(14, 20, 4, Rgb::BLACK);
		sf.strikethrough(14, 20, 12, 8, Rgb::BLACK);
		drop(sf);
		let rows = (0..80u32)
			.filter(|y| px(&buf, scale.dev(5) as u32 + 2, *y)[3] == 255)
			.count();
		assert_eq!(rows, thick, "scale {percent}: horizontal line thickness");
		let cols = (0..80u32)
			.filter(|x| {
				px(&buf, *x, scale.dev(5) as u32 - 1)[3] == 255 && *x < scale.dev(2) as u32 + 8
			})
			.count();
		assert!(cols >= thick);
		assert_eq!(
			px(&buf, scale.dev(15) as u32, scale.dev(5) as u32)[3],
			255,
			"underline"
		);
		assert_eq!(
			px(&buf, scale.dev(15) as u32, scale.dev(9) as u32)[3],
			255,
			"strikethrough"
		);
	}
}

#[test]
fn glyph_edges_are_antialiased_and_advances_match_the_measure() {
	use quill_core::layout::Measure;
	let mut m = measure();
	let f = serif();
	let mut widths = [0u16; quill_core::layout::GLYPHS];
	m.widths(&f, &mut widths);
	let gm = measure();
	let mut buf = PixelBuffer::new(120, 60);
	let mut sf = buf.surface(
		ByteOrder::Rgba,
		Scale::from_percent(175),
		MeasureGlyphs::new(&gm),
	);
	let font = sf.font(&f);
	for ch in *b"AgW " {
		assert_eq!(
			sf.advance(font, ch),
			i32::from(widths[usize::from(ch - 0x20)])
		);
	}
	sf.fill_rect(Rect::new(0, 0, 70, 40), Rgb::WHITE);
	sf.glyph_run(font, 30, &[(b'A', 4), (b'g', 20)], Rgb::BLACK, true);
	drop(sf);
	let grey = buf
		.bytes()
		.chunks_exact(4)
		.filter(|p| p[0] > 20 && p[0] < 235)
		.count();
	let black = buf
		.bytes()
		.chunks_exact(4)
		.filter(|p| p[0] < 20 && p[3] == 255)
		.count();
	assert!(grey > 40, "no antialiasing: {grey} grey pixels");
	assert!(black > 40, "no solid ink: {black} black pixels");
}

#[test]
fn pictures_scale_by_repeating_rows_and_columns() {
	let m = measure();
	let mut buf = PixelBuffer::new(40, 40);
	let mut sf = buf.surface(
		ByteOrder::Rgba,
		Scale::from_percent(150),
		MeasureGlyphs::new(&m),
	);
	sf.rgb_row(2, 2, &[255, 0, 0, 0, 255, 0, 0, 0, 255]);
	drop(sf);
	// Three logical pixels at 1.5 are 4 or 5 device pixels, 3 from x = 3.
	assert_eq!(px(&buf, 3, 3), [255, 0, 0, 255]);
	assert_eq!(px(&buf, 3, 4), [255, 0, 0, 255]);
	assert_eq!(px(&buf, 5, 3), [0, 255, 0, 255]);
	assert_eq!(px(&buf, 7, 3), [0, 0, 255, 255]);
	assert_eq!(px(&buf, 8, 3), [0, 0, 0, 0]);
}

#[test]
fn invert_flips_colour_and_keeps_alpha() {
	let m = measure();
	let mut buf = PixelBuffer::new(8, 8);
	let mut sf = buf.surface(ByteOrder::Rgba, Scale::ONE, MeasureGlyphs::new(&m));
	sf.fill_rect(Rect::new(0, 0, 8, 8), Rgb(10, 20, 30));
	sf.invert_rect(Rect::new(0, 0, 4, 4));
	drop(sf);
	assert_eq!(px(&buf, 1, 1), [245, 235, 225, 255]);
	assert_eq!(px(&buf, 5, 5), [10, 20, 30, 255]);
}

#[test]
fn scale_conversions() {
	assert_eq!(Scale::from_percent(175).raw(), 448);
	assert_eq!(Scale::from_ratio(7, 4).raw(), 448);
	assert_eq!(Scale::from_percent(150).dev(11), 17);
	assert_eq!(Scale::from_percent(125).logical(15), 12);
	assert_eq!(Scale::from_raw(1).raw(), 64);
	assert_eq!(Scale::from_percent(100000).raw(), 2048);
}
