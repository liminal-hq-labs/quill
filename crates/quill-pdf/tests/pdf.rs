// The writer's tests: the encoder round-trips through the decoder, the file parses, and the page's words come back out of it.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::PathBuf;
use std::process::Command;

use quill_pdf::{deflate, Family, Font, Info, Page, Pdf, Picture, Rgb, Run};

fn inflate(z: &[u8]) -> Vec<u8> {
	assert_eq!(&z[..2], &[0x78, 0x01]);
	let body = &z[2..z.len() - 4];
	let mut at = 0;
	let mut out = Vec::new();
	let mut window = Box::new([0u8; quill_inflate::WINDOW]);
	quill_inflate::inflate(
		|| {
			let b = body.get(at).copied();
			at += 1;
			b
		},
		|b| {
			out.push(b);
			true
		},
		&mut window,
	)
	.unwrap();
	assert_eq!(&z[z.len() - 4..], &deflate::adler32(&out).to_be_bytes());
	out
}

fn round_trip(data: &[u8]) -> usize {
	let mut z = Vec::new();
	deflate::zlib(data, &mut z);
	assert_eq!(inflate(&z), data);
	z.len()
}

/// A repeatable stream of bytes.
fn noise(seed: u64, n: usize) -> Vec<u8> {
	let mut s = seed | 1;
	(0..n)
		.map(|_| {
			s ^= s << 13;
			s ^= s >> 7;
			s ^= s << 17;
			(s >> 24) as u8
		})
		.collect()
}

#[test]
fn adler32_matches_the_published_value() {
	// RFC 1950's example and the checksum of "Wikipedia" from its article.
	assert_eq!(deflate::adler32(b""), 1);
	assert_eq!(deflate::adler32(b"Wikipedia"), 0x11E6_0398);
}

#[test]
fn deflate_round_trips_edge_cases() {
	round_trip(b"");
	round_trip(b"a");
	round_trip(b"ab");
	round_trip(b"abc");
	round_trip(&[0u8; 100_000]);
	round_trip(&b"abcabcabcabc".repeat(5000));
	let mut far = noise(7, 40_000);
	let head = far[..300].to_vec();
	far.extend_from_slice(&head);
	round_trip(&far);
}

#[test]
fn deflate_round_trips_random_data_and_never_grows_it_much() {
	for (seed, n) in [
		(1u64, 1usize),
		(2, 257),
		(3, 4096),
		(4, 65_535),
		(5, 65_536),
		(6, 200_000),
	] {
		let data = noise(seed, n);
		let z = round_trip(&data);
		// A stored block costs five bytes per 65,535, the wrapper six.
		assert!(z <= n + 5 * n.div_ceil(65_535) + 6, "{n} bytes became {z}");
	}
}

#[test]
fn deflate_round_trips_structured_data_and_shrinks_it() {
	let rows: Vec<u8> = (0..300u32)
		.flat_map(|y| (0..400u32).flat_map(move |x| [(x / 4) as u8, (y / 3) as u8, 0xFF]))
		.collect();
	let z = round_trip(&rows);
	assert!(z * 8 < rows.len(), "{} became {z}", rows.len());
	let text = b"BT /F4 12 Tf 72 700 Td [(Dear )-12(Sir)] TJ ET\n".repeat(300);
	assert!(round_trip(&text) * 10 < text.len());
}

/// An external tool, or `None` with a note on the test's output when the host does not have it.
fn tool(name: &'static str) -> Option<&'static str> {
	let found = Command::new(name).arg("-v").output().is_ok();
	if !found {
		eprintln!("skipped: {name} is not installed");
	}
	found.then_some(name)
}

fn scratch(name: &str) -> PathBuf {
	let dir = std::env::temp_dir().join(format!("quill-pdf-{}", std::process::id()));
	std::fs::create_dir_all(&dir).unwrap();
	dir.join(name)
}

fn run(tool: &str, args: &[&str]) -> (bool, String) {
	let out = Command::new(tool).args(args).output().unwrap();
	let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
	text.push_str(&String::from_utf8_lossy(&out.stderr));
	(out.status.success(), text)
}

/// What a layout at `dpi` gives each character of `text` in `font` at `size_half`: the font's own width, rounded to a device unit.
fn advances(font: Font, size_half: u16, dpi: u16, text: &[u8]) -> Vec<i32> {
	text.iter()
		.map(|b| {
			let w = i64::from(font.width(*b)) * i64::from(size_half) * i64::from(dpi);
			((w + 72_000) / 144_000) as i32
		})
		.collect()
}

const DPI: u16 = 600;
const LETTER: (i32, i32) = (5100, 6600);

fn put(page: &mut Page, font: Font, size_half: u16, x: i32, baseline: i32, text: &[u8]) {
	let adv = advances(font, size_half, DPI, text);
	page.text(&Run {
		font,
		size_half,
		colour: Rgb::BLACK,
		x,
		baseline,
		text,
		advances: &adv,
	});
}

fn sample() -> Vec<u8> {
	let serif = Font {
		family: Family::Serif,
		bold: false,
		italic: false,
	};
	let bold = Font {
		bold: true,
		..serif
	};
	let mono = Font {
		family: Family::Mono,
		bold: false,
		italic: false,
	};
	let sans_italic = Font {
		family: Family::Sans,
		bold: false,
		italic: true,
	};
	let mut pdf = Pdf::new(&Info {
		title: b"Sample \x97 two pages",
		created: Some(b"20260101000000"),
	});
	let mut p1 = Page::new(LETTER.0, LETTER.1, DPI);
	put(&mut p1, bold, 28, 600, 900, b"Dear Sir or Madam,");
	put(
		&mut p1,
		serif,
		24,
		600,
		1200,
		b"Caf\xE9 na\xEFve \x93quoted\x94 (parens) back\\slash \x96 dash.",
	);
	put(&mut p1, mono, 20, 600, 1500, b"fn main() { 1 + 1 }");
	p1.text(&Run {
		font: sans_italic,
		size_half: 24,
		colour: Rgb(200, 0, 0),
		x: 600,
		baseline: 1800,
		text: b"Red italic",
		advances: &advances(sans_italic, 24, DPI, b"Red italic"),
	});
	p1.rect(Rgb(0, 0, 255), 600, 1850, 1200, 12);
	let mut p2 = Page::new(LETTER.0, LETTER.1, DPI);
	put(&mut p2, serif, 24, 600, 900, b"Page two");
	pdf.add_page(p1);
	pdf.add_page(p2);
	pdf.finish()
}

#[test]
fn output_is_repeatable_and_matches_the_golden() {
	let bytes = sample();
	assert_eq!(bytes, sample());
	let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/sample.pdf");
	if std::env::var_os("QUILL_UPDATE_GOLDEN").is_some() {
		std::fs::write(path, &bytes).unwrap();
		return;
	}
	let golden = std::fs::read(path)
		.unwrap_or_else(|_| panic!("missing golden {path}; run with QUILL_UPDATE_GOLDEN=1"));
	assert!(
		golden == bytes,
		"{path} differs; QUILL_UPDATE_GOLDEN=1 rewrites it and the diff is reviewed like code"
	);
}

#[test]
fn the_file_has_the_structure_a_reader_wants() {
	let bytes = sample();
	assert!(bytes.starts_with(b"%PDF-1.4\n"));
	assert!(bytes.ends_with(b"%%EOF\n"));
	let text = String::from_utf8_lossy(&bytes);
	assert!(text.contains("/Count 2"));
	assert!(text.contains("/BaseFont /Times-Roman"));
	assert!(text.contains("/BaseFont /Times-Bold"));
	assert!(text.contains("/BaseFont /Courier "));
	assert!(text.contains("/BaseFont /Helvetica-Oblique"));
	assert!(text.contains("/CreationDate (D:20260101000000Z)"));
	// The cross-reference table's offsets point at their objects.
	let start = bytes
		.windows(10)
		.rposition(|w| w == b"startxref\n")
		.unwrap()
		+ 10;
	let xref: usize = String::from_utf8_lossy(&bytes[start..])
		.lines()
		.next()
		.unwrap()
		.parse()
		.unwrap();
	assert_eq!(&bytes[xref..xref + 4], b"xref");
	let table = String::from_utf8_lossy(&bytes[xref..]).into_owned();
	let entries: Vec<&str> = table
		.lines()
		.skip(3)
		.take_while(|l| l.len() == 19)
		.collect();
	assert!(!entries.is_empty());
	for (i, e) in entries.iter().enumerate() {
		let at: usize = e[..10].parse().unwrap();
		let want = format!("{} 0 obj", i + 1);
		assert_eq!(
			&bytes[at..at + want.len()],
			want.as_bytes(),
			"object {}",
			i + 1
		);
	}
}

#[test]
fn an_empty_document_is_still_a_valid_file() {
	let bytes = Pdf::new(&Info::default()).finish();
	if let Some(qpdf) = tool("qpdf") {
		let path = scratch("empty.pdf");
		std::fs::write(&path, &bytes).unwrap();
		let (ok, out) = run(qpdf, &["--check", path.to_str().unwrap()]);
		assert!(ok, "{out}");
	}
	assert!(String::from_utf8_lossy(&bytes).contains("/Count 1"));
}

#[test]
fn qpdf_and_pdfinfo_accept_the_sample() {
	let path = scratch("sample.pdf");
	std::fs::write(&path, sample()).unwrap();
	let file = path.to_str().unwrap();
	if let Some(qpdf) = tool("qpdf") {
		let (ok, out) = run(qpdf, &["--check", file]);
		assert!(ok, "{out}");
		assert!(
			out.contains("No syntax or stream encoding errors found"),
			"{out}"
		);
	}
	if let Some(info) = tool("pdfinfo") {
		let (ok, out) = run(info, &[file]);
		assert!(ok, "{out}");
		let field = |key: &str| {
			let line = out.lines().find(|l| l.starts_with(key)).unwrap_or_default();
			line[key.len()..].trim().to_string()
		};
		assert_eq!(field("Pages:"), "2", "{out}");
		assert_eq!(field("Page size:"), "612 x 792 pts (letter)", "{out}");
		assert_eq!(field("Title:"), "Sample \u{2014} two pages", "{out}");
		assert_eq!(field("Producer:"), "Quill", "{out}");
	}
}

#[test]
fn the_words_come_back_out_in_order() {
	let Some(pdftotext) = tool("pdftotext") else {
		return;
	};
	let path = scratch("words.pdf");
	std::fs::write(&path, sample()).unwrap();
	let (ok, out) = run(pdftotext, &["-layout", path.to_str().unwrap(), "-"]);
	assert!(ok, "{out}");
	let lines: Vec<&str> = out
		.lines()
		.map(str::trim)
		.filter(|l| !l.is_empty())
		.collect();
	assert_eq!(
		lines,
		[
			"Dear Sir or Madam,",
			"Caf\u{e9} na\u{ef}ve \u{201c}quoted\u{201d} (parens) back\\slash \u{2013} dash.",
			"fn main() { 1 + 1 }",
			"Red italic",
			"Page two",
		]
	);
}

#[test]
fn a_picture_comes_back_as_drawn() {
	let Some(pdftoppm) = tool("pdftoppm") else {
		return;
	};
	let (w, h) = (96u32, 64u32);
	let rgb: Vec<u8> = (0..h)
		.flat_map(|y| (0..w).flat_map(move |x| [(x * 2) as u8, (y * 3) as u8, (x + y) as u8]))
		.collect();
	let mut page = Page::new(w as i32, h as i32, 72);
	assert!(page.image(&Picture {
		x: 0,
		y: 0,
		w: w as i32,
		h: h as i32,
		px_w: w,
		px_h: h,
		rgb: &rgb,
	}));
	let mut pdf = Pdf::new(&Info::default());
	pdf.add_page(page);
	let path = scratch("picture.pdf");
	std::fs::write(&path, pdf.finish()).unwrap();
	let base = scratch("picture");
	let (ok, out) = run(
		pdftoppm,
		&["-r", "72", path.to_str().unwrap(), base.to_str().unwrap()],
	);
	assert!(ok, "{out}");
	let ppm = std::fs::read(scratch("picture-1.ppm")).unwrap();
	let header = format!("P6\n{w} {h}\n255\n");
	assert!(ppm.starts_with(header.as_bytes()));
	// The viewer resamples a picture a hair even at one pixel to the unit, so the comparison is within a few levels.
	let worst = ppm[header.len()..]
		.iter()
		.zip(&rgb)
		.map(|(a, b)| a.abs_diff(*b))
		.max()
		.unwrap();
	assert!(worst <= 4, "the page differs from the picture by {worst}");
}

#[test]
fn text_lands_where_the_layout_put_it() {
	// A word whose advances are far from the font's own widths still starts, and ends, where the layout says: the second word is
	// at x = 3000 units whatever the first one's width.
	let Some(pdftotext) = tool("pdftotext") else {
		return;
	};
	let serif = Font {
		family: Family::Serif,
		bold: false,
		italic: false,
	};
	let mut page = Page::new(LETTER.0, LETTER.1, DPI);
	page.text(&Run {
		font: serif,
		size_half: 24,
		colour: Rgb::BLACK,
		x: 600,
		baseline: 900,
		text: b"Wide",
		advances: &[400, 400, 400, 400],
	});
	put(&mut page, serif, 24, 3000, 900, b"Next");
	let mut pdf = Pdf::new(&Info::default());
	pdf.add_page(page);
	let path = scratch("place.pdf");
	std::fs::write(&path, pdf.finish()).unwrap();
	let (ok, out) = run(pdftotext, &["-bbox", path.to_str().unwrap(), "-"]);
	assert!(ok, "{out}");
	// pdftotext's box for each word, in points: 3000 units at 600 to the inch is 360.
	let boxes: Vec<(String, f64, f64)> = out
		.lines()
		.filter(|l| l.contains("<word"))
		.map(|l| {
			let attr = |k: &str| -> f64 {
				let i = l.find(&format!("{k}=\"")).unwrap() + k.len() + 2;
				l[i..].split('"').next().unwrap().parse().unwrap()
			};
			let word = l
				.split('>')
				.nth(1)
				.unwrap()
				.split('<')
				.next()
				.unwrap()
				.to_string();
			(word, attr("xMin"), attr("xMax"))
		})
		.collect();
	// "Wide" has 400 units (48 points) between its letters, which the viewer reads as four words; each starts where the layout says.
	let starts: Vec<f64> = boxes.iter().map(|b| b.1).collect();
	let letters: Vec<&str> = boxes.iter().map(|b| b.0.as_str()).collect();
	assert_eq!(letters, ["W", "i", "d", "e", "Next"], "{out}");
	for (got, want) in starts.iter().zip([72.0, 120.0, 168.0, 216.0, 360.0]) {
		assert!((got - want).abs() < 0.5, "{boxes:?}");
	}
}

#[test]
fn font_choice_follows_the_face_then_the_family() {
	let pick = |face: &[u8], family| Font::pick(face, family, false, false).family;
	assert_eq!(pick(b"Times New Roman", 1), Family::Serif);
	assert_eq!(pick(b"Arial", 2), Family::Sans);
	assert_eq!(pick(b"Courier New", 3), Family::Mono);
	assert_eq!(pick(b"MS Sans Serif", 2), Family::Sans);
	assert_eq!(pick(b"Lucida Console", 0), Family::Mono);
	assert_eq!(pick(b"Georgia", 0), Family::Serif);
	assert_eq!(pick(b"Some Odd Face", 1), Family::Serif);
	assert_eq!(pick(b"Some Odd Face", 3), Family::Mono);
	assert_eq!(pick(b"Some Odd Face", 0), Family::Sans);
}

#[test]
fn widths_are_adobes() {
	let h = Font {
		family: Family::Sans,
		bold: false,
		italic: false,
	};
	assert_eq!(
		[
			h.width(b' '),
			h.width(b'A'),
			h.width(b'a'),
			h.width(0xE9),
			h.width(0x95)
		],
		[278, 667, 556, 556, 350]
	);
	let t = Font {
		family: Family::Serif,
		bold: true,
		italic: true,
	};
	assert_eq!(
		[t.width(b' '), t.width(b'A'), t.width(b'W')],
		[250, 667, 889]
	);
	assert_eq!(
		Font {
			family: Family::Mono,
			bold: true,
			italic: false
		}
		.width(b'x'),
		600
	);
	assert_eq!(h.width(7), 0);
}

#[test]
fn numbers_print_short_and_exact() {
	let font = Font {
		family: Family::Sans,
		bold: false,
		italic: false,
	};
	let mut page = Page::new(720, 720, 72);
	page.text(&Run {
		font,
		size_half: 21,
		colour: Rgb(255, 128, 0),
		x: 36,
		baseline: 72,
		text: b"A",
		advances: &[10],
	});
	let text = String::from_utf8(page.content_for_tests().to_vec()).unwrap();
	assert!(
		text.starts_with("1 0.502 0 rg\nBT /F0 10.5 Tf 36 648 Td [(A"),
		"{text}"
	);
}
