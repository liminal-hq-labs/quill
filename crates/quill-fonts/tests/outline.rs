// OutlineMeasure: layout dumps at 1440 units to the inch, the unhinted-width check and the Windows ME comparison report.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::fmt::Write as _;
use std::path::PathBuf;

use quill_core::doc::{Doc, StoryKind};
use quill_core::layout::{dump_story, FontRef, Measure, GLYPHS};
use quill_fonts::{FontSet, OutlineMeasure};
use quill_formats::io::SliceSource;
use quill_host::HostStore;
use skrifa::instance::{LocationRef, Size};
use skrifa::raw::TableProvider;
use skrifa::{FontRef as SkFont, GlyphId, MetadataProvider};

fn root() -> PathBuf {
	PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn compare(path: &str, actual: &str) {
	let file = root().join(path);
	if std::env::var_os("QUILL_UPDATE_GOLDEN").is_some() {
		std::fs::write(&file, actual).unwrap();
		return;
	}
	let want = std::fs::read_to_string(&file)
		.unwrap_or_else(|_| panic!("missing golden {path}; run with QUILL_UPDATE_GOLDEN=1"));
	assert_eq!(
		want, actual,
		"{path} differs; QUILL_UPDATE_GOLDEN=1 rewrites it and the diff is reviewed like code"
	);
}

fn load(name: &str) -> (Doc, HostStore) {
	let mut s = HostStore::new();
	let mut d = Doc::new();
	let bytes = std::fs::read(root().join("tests/docs").join(name)).unwrap();
	let mut src = SliceSource::new(&bytes);
	if name.ends_with(".qld") {
		quill_formats::qld::read(&mut d, &mut s, &mut src).unwrap();
	} else {
		quill_formats::rtf::read(&mut d, &mut s, &mut src).unwrap();
	}
	(d, s)
}

fn font(face: &str, family: u8, size: u16, flags: u16) -> FontRef {
	let mut f = [0u8; 31];
	f[..face.len()].copy_from_slice(face.as_bytes());
	FontRef {
		face: f,
		face_len: face.len() as u8,
		family,
		size,
		flags,
	}
}

fn dump(doc: &str, name: &str, res: u16) -> String {
	let (d, s) = load(doc);
	let mut m = OutlineMeasure::new(FontSet::vendored()).with_resolution(res);
	let mut out = String::new();
	dump_story(&d, &s, &mut m, name, StoryKind::Body, &mut out).unwrap();
	out
}

#[test]
fn layout_dumps_at_1440_match_their_goldens() {
	for doc in [
		"letter.rtf",
		"lists.rtf",
		"headers.rtf",
		"pictures.rtf",
		"plain.qld",
	] {
		let stem = doc.split('.').next().unwrap();
		compare(
			&format!("tests/golden/layout/{stem}.outline-1440.txt"),
			&dump(doc, "outline", 1440),
		);
	}
}

#[test]
fn a_layout_at_twips_has_the_line_breaks_of_the_same_layout_at_other_resolutions_only_where_rounding_decides(
) {
	// The same font units at 1440 and 2880 units to the inch break every line of the letter in the same place: widths are the
	// font's, not the device's.
	let starts = |text: &str| -> Vec<String> {
		text.lines()
			.filter(|l| l.starts_with("page"))
			.map(|l| {
				l.split_whitespace()
					.skip(8)
					.take(4)
					.collect::<Vec<_>>()
					.join(" ")
			})
			.collect()
	};
	let a = starts(&dump("letter.rtf", "a", 1440));
	let b = starts(&dump("letter.rtf", "b", 2880));
	assert!(!a.is_empty());
	assert_eq!(a, b);
}

#[test]
fn widths_are_the_fonts_advances_and_never_depend_on_a_pixel_size() {
	let set = FontSet::vendored();
	for face in set.faces() {
		let sk = SkFont::from_index(face.bytes(), face.index()).unwrap();
		let upem = f32::from(face.units_per_em());
		let hmtx = sk.hmtx().unwrap();
		// skrifa's unhinted metrics at any size are the hmtx advances scaled; there is no pixel grid in them.
		for ppem in [
			6.0f32, 8.0, 9.0, 10.5, 11.0, 12.0, 13.0, 16.0, 24.0, 48.0, 96.5,
		] {
			let gm = sk.glyph_metrics(Size::new(ppem), LocationRef::default());
			for ch in 0x20u8..=0x7E {
				let gid = sk.charmap().map(char::from(ch)).unwrap();
				let scaled = gm.advance_width(gid).unwrap();
				let want = f32::from(hmtx.advance(gid).unwrap()) * ppem / upem;
				assert!(
					(scaled - want).abs() < 0.01,
					"{} {:?} at {ppem} ppem: {scaled} against {want}",
					face.family(),
					char::from(ch)
				);
			}
		}
		let _ = GlyphId::NOTDEF;
	}
	// Whatever size a measure is asked at, widths are the font units scaled: 24 half points is exactly twice 12, up to rounding.
	let mut m = OutlineMeasure::new(FontSet::vendored());
	for (face, code) in [("Times New Roman", 1), ("Arial", 2), ("Courier New", 3)] {
		let (mut w12, mut w24, mut w25) = ([0u16; GLYPHS], [0u16; GLYPHS], [0u16; GLYPHS]);
		m.widths(&font(face, code, 24, 0), &mut w12);
		m.widths(&font(face, code, 48, 0), &mut w24);
		m.widths(&font(face, code, 49, 0), &mut w25);
		for i in 0..GLYPHS {
			assert!(w24[i].abs_diff(2 * w12[i]) <= 1, "{face} {i}");
			assert!(w25[i] >= w24[i]);
		}
		// A space of Times at 12 pt is 1/4 em: 3 pt, 60 twips.
		if face == "Times New Roman" {
			assert_eq!(w12[0], 60);
		}
	}
}

#[test]
fn metrics_come_from_the_windows_extent_and_the_hhea_line() {
	let mut m = OutlineMeasure::new(FontSet::vendored());
	let f = m.metrics(&font("Times New Roman", 1, 24, 0));
	// Liberation Serif: usWinAscent 1825, usWinDescent 443, hhea line 1825 + 443 + 87 over 2048 units, at 12 pt = 240 twips an em.
	assert_eq!(f.ascent, 214);
	assert_eq!(f.descent, 52);
	assert_eq!(f.leading, 10);
	let mono = m.metrics(&font("Courier New", 3, 24, 0));
	assert!(mono.ascent > 0 && mono.descent > 0);
}

#[test]
fn a_bold_without_a_bold_face_is_wider_and_flagged() {
	let v = FontSet::vendored();
	let mut only = FontSet::empty();
	for f in v
		.faces()
		.iter()
		.filter(|f| f.family() == "Quill Test Serif" && !f.is_bold() && !f.is_italic())
	{
		only.add(f.bytes().to_vec().into());
	}
	let mut m = OutlineMeasure::new(only);
	let (mut plain, mut bold) = ([0u16; GLYPHS], [0u16; GLYPHS]);
	let name = "Quill Test Serif";
	m.widths(&font(name, 1, 24, 0), &mut plain);
	m.widths(&font(name, 1, 24, 1), &mut bold);
	assert!(m.resolve(&font(name, 1, 24, 1)).unwrap().synthetic_bold);
	// An em of 240 twips over 64 is 3.75 twips on every glyph, which rounding makes three or four.
	for i in 0..GLYPHS {
		assert!(
			(3..=4).contains(&(i32::from(bold[i]) - i32::from(plain[i]))),
			"{i}"
		);
	}
}

/// Windows ME's captured table, as (face, half points, flags, ascent, descent, leading, widths).
type Row = (String, u16, u16, [i32; 3], Vec<u16>);

fn captured() -> Vec<Row> {
	let text =
		std::fs::read_to_string(root().join("tests/metrics/screen96-windows-me.txt")).unwrap();
	text.lines()
		.filter(|l| !l.starts_with('#') && !l.trim().is_empty())
		.map(|l| {
			let p: Vec<&str> = l.split('|').collect();
			let n = |i: usize| p[i].parse::<i32>().unwrap();
			(
				p[0].to_string(),
				p[1].parse().unwrap(),
				p[2].parse().unwrap(),
				[n(3), n(4), n(5)],
				p[6].split(',').map(|v| v.trim().parse().unwrap()).collect(),
			)
		})
		.collect()
}

fn line_starts(dump: &str) -> Vec<(u32, u32, String)> {
	dump.lines()
		.filter(|l| l.starts_with("page"))
		.map(|l| {
			let f: Vec<&str> = l.split('|').next().unwrap().split_whitespace().collect();
			let at = |k: &str| {
				f.iter()
					.position(|x| *x == k)
					.map(|i| f[i + 1].parse::<u32>().unwrap())
					.unwrap()
			};
			(
				at("para"),
				at("start"),
				l.split_once("| ")
					.map_or(String::new(), |(_, t)| t.chars().take(40).collect()),
			)
		})
		.collect()
}

/// The report of how the Quill test fonts, which stand for Times New Roman, Arial and Courier New, compare with the widths and
/// vertical metrics Windows ME reported at 96 dpi, and which lines of the letter break differently. It describes; it never fails.
fn windows_me_report(fonts: FontSet) -> String {
	let mut out = String::new();
	let mut m = OutlineMeasure::new(fonts.clone()).with_resolution(96);
	let _ = writeln!(out, "# outline measure against Windows ME at 96 dpi");
	let _ = writeln!(out, "# fonts: the Quill test fonts (Liberation 2.1.5, renamed), for Times New Roman, Arial and Courier New");
	let _ = writeln!(out, "# widths in pixels at 96 dpi; GDI hints them to whole pixels, the outline measure rounds the unhinted advance");
	let _ = writeln!(out);
	let _ = writeln!(out, "face | half points | flags | glyphs wider or narrower | largest difference | ascent windows/outline | descent | leading");
	let rows = captured();
	let (mut glyphs, mut differing) = (0usize, 0usize);
	for (face, size, flags, vm, widths) in &rows {
		let code = match face.as_str() {
			"Arial" => 2,
			"Courier New" => 3,
			_ => 1,
		};
		let f = font(face, code, *size, *flags);
		let mut w = [0u16; GLYPHS];
		m.widths(&f, &mut w);
		let diffs: Vec<i32> = w
			.iter()
			.zip(widths)
			.map(|(a, b)| i32::from(*a) - i32::from(*b))
			.collect();
		let n = diffs.iter().filter(|d| **d != 0).count();
		let worst = diffs.iter().map(|d| d.abs()).max().unwrap_or(0);
		let vo = m.metrics(&f);
		glyphs += diffs.len();
		differing += n;
		let _ = writeln!(
			out,
			"{face} | {size} | {flags} | {n} of {} | {worst} | {}/{} | {}/{} | {}/{}",
			diffs.len(),
			vm[0],
			vo.ascent,
			vm[1],
			vo.descent,
			vm[2],
			vo.leading
		);
	}
	let _ = writeln!(out);
	let _ = writeln!(
		out,
		"total: {differing} of {glyphs} glyph widths differ from Windows ME by at least one pixel"
	);
	let _ = writeln!(out);
	let me = std::fs::read_to_string(root().join("tests/golden/layout/letter.windows-me.txt"))
		.unwrap()
		.replace("\r\n", "\n");
	let mine = {
		let (d, s) = load("letter.rtf");
		let mut text = String::new();
		let mut m = OutlineMeasure::new(fonts).with_resolution(96);
		dump_story(&d, &s, &mut m, "outline-96", StoryKind::Body, &mut text).unwrap();
		text
	};
	let (a, b) = (line_starts(&me), line_starts(&mine));
	let _ = writeln!(
		out,
		"letter.rtf: {} lines on Windows ME, {} with the outline measure",
		a.len(),
		b.len()
	);
	let only_me: Vec<_> = a
		.iter()
		.filter(|l| !b.iter().any(|x| x.0 == l.0 && x.1 == l.1))
		.collect();
	let only_mine: Vec<_> = b
		.iter()
		.filter(|l| !a.iter().any(|x| x.0 == l.0 && x.1 == l.1))
		.collect();
	if only_me.is_empty() && only_mine.is_empty() {
		let _ = writeln!(
			out,
			"every line of the letter starts in the same place as on Windows ME"
		);
	} else {
		let _ = writeln!(
			out,
			"lines that start in a different place (paragraph, character position, text):"
		);
		for (tag, v) in [("windows me only", &only_me), ("outline only", &only_mine)] {
			for (para, start, text) in v.iter() {
				let _ = writeln!(out, "  {tag}: paragraph {para}, start {start}: {text}");
			}
		}
	}
	out
}

#[test]
fn the_windows_me_comparison_report_is_current() {
	// A report, not a pass mark: the file shows how far the outline measure is from Windows ME, and a change to the measure shows
	// as a change in the report.
	compare(
		"tests/golden/layout/letter.outline96-vs-windows-me.report.txt",
		&windows_me_report(FontSet::vendored()),
	);
}

/// The same report over the fonts installed on this machine (Times New Roman and the rest, where they are): local only, since
/// those fonts are not ours to commit. Run with `cargo nextest run -p quill-fonts --run-ignored only`; the report goes to stdout.
#[test]
#[ignore = "reads the machine's installed fonts"]
fn the_windows_me_comparison_over_the_installed_fonts() {
	let set = FontSet::system();
	for name in ["Times New Roman", "Arial", "Courier New"] {
		let r = set.resolve(name, 1, false, false).unwrap();
		println!(
			"{name} resolves to {} ({} bytes)",
			r.face.family(),
			r.face.bytes().len()
		);
	}
	println!("{}", windows_me_report(set));
}
