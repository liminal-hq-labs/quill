// The comparison with Windows ME: strike widths and metrics against the captured table, and the lines of the letter that break
// differently. A report, not a pass mark.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::fmt::Write as _;
use std::path::PathBuf;

use quill_core::doc::{Doc, StoryKind};
use quill_core::layout::{dump_story, FontMetrics, FontRef, Measure, GLYPHS};
use quill_formats::io::SliceSource;
use quill_host::HostStore;

use crate::faceset::{cp1252, FaceSet};
use crate::fnt::Font;

/// The repository root.
pub fn root() -> PathBuf {
	PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The strike that serves a request, and the whole-number scale it is drawn at: the nearest pixel size of the face and style
/// among each strike times 1, 2 and 3, smaller scales and sizes first on a tie.
pub fn strike_for(font: &Font, face: u8, style: u8, half_points: u16) -> Option<(usize, u32)> {
	let want = u32::from(half_points) * 2;
	let mut best: Option<(u32, usize, u32)> = None;
	for (i, e) in font.entries.iter().enumerate() {
		if e.face != face || e.style != style {
			continue;
		}
		for scale in 1..=3u32 {
			let have = u32::from(e.px) * scale * 3;
			let d = have.abs_diff(want);
			if best.is_none_or(|(bd, _, bs)| d < bd || (d == bd && scale < bs)) {
				best = Some((d, i, scale));
			}
		}
	}
	best.map(|(_, i, s)| (i, s))
}

/// A `Measure` over strikes, for the report: a stand-in for the DOS runtime's own, with the same nearest-strike rule.
pub struct StrikeMeasure<'a> {
	font: &'a Font<'a>,
}

impl<'a> StrikeMeasure<'a> {
	/// A measure over a parsed file.
	pub fn new(font: &'a Font<'a>) -> Self {
		StrikeMeasure { font }
	}

	fn pick(&self, f: &FontRef) -> Option<(usize, u32)> {
		let name: String = f.face[..usize::from(f.face_len)]
			.iter()
			.map(|b| cp1252(*b))
			.collect::<String>()
			.to_ascii_lowercase();
		let face = match name.as_str() {
			"arial" | "helvetica" | "ms sans serif" => 1,
			"courier new" | "courier" | "lucida console" | "fixedsys" => 2,
			"times new roman" | "times" | "ms serif" => 0,
			_ => match f.family {
				2 => 1,
				3 => 2,
				_ => 0,
			},
		};
		strike_for(self.font, face, (f.flags & 3) as u8, f.size)
	}
}

impl Measure for StrikeMeasure<'_> {
	fn resolution(&self) -> (u16, u16) {
		(96, 96)
	}

	fn metrics(&mut self, f: &FontRef) -> FontMetrics {
		let Some((s, k)) = self.pick(f) else {
			return FontMetrics::default();
		};
		let e = &self.font.entries[s];
		FontMetrics {
			ascent: i32::from(e.ascent) * k as i32,
			descent: i32::from(e.descent) * k as i32,
			leading: i32::from(e.leading) * k as i32,
		}
	}

	fn widths(&mut self, f: &FontRef, out: &mut [u16; GLYPHS]) {
		let Some((s, k)) = self.pick(f) else {
			out.fill(0);
			return;
		};
		for (i, w) in out.iter_mut().enumerate() {
			*w = u16::from(self.font.advance(s, i)) * k as u16;
		}
	}
}

fn font_ref(face: &str, family: u8, size: u16, flags: u16) -> FontRef {
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

/// Windows ME's captured table: (face, half points, flags, ascent/descent/leading, widths).
type Row = (String, u16, u16, [i32; 3], Vec<u16>);

fn captured() -> Vec<Row> {
	let text = std::fs::read_to_string(root().join("tests/metrics/screen96-windows-me.txt"))
		.expect("the captured table");
	text.lines()
		.filter(|l| !l.starts_with('#') && !l.trim().is_empty())
		.map(|l| {
			let p: Vec<&str> = l.split('|').collect();
			let n = |i: usize| p[i].parse::<i32>().expect("a number");
			(
				p[0].to_string(),
				p[1].parse().expect("size"),
				p[2].parse().expect("flags"),
				[n(3), n(4), n(5)],
				p[6].split(',')
					.map(|v| v.trim().parse().expect("width"))
					.collect(),
			)
		})
		.collect()
}

fn line_starts(dump: &str) -> Vec<(u32, u32, String)> {
	dump.lines()
		.filter(|l| l.starts_with("page"))
		.map(|l| {
			let f: Vec<&str> = l
				.split('|')
				.next()
				.unwrap_or("")
				.split_whitespace()
				.collect();
			let at = |k: &str| {
				f.iter()
					.position(|x| *x == k)
					.and_then(|i| f.get(i + 1))
					.and_then(|v| v.parse::<u32>().ok())
					.unwrap_or(0)
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

fn load(name: &str) -> (Doc, HostStore) {
	let mut s = HostStore::new();
	let mut d = Doc::new();
	let bytes = std::fs::read(root().join("tests/docs").join(name)).expect("a test document");
	let mut src = SliceSource::new(&bytes);
	quill_formats::rtf::read(&mut d, &mut s, &mut src).expect("readable");
	(d, s)
}

/// The report for one face set, from its `QUILL.FNT` bytes.
pub fn windows_me_report(set: &FaceSet, fnt: &[u8], what: &str) -> String {
	let font = Font::parse(fnt).expect("a file this crate wrote");
	let mut m = StrikeMeasure::new(&font);
	let mut out = String::new();
	let _ = writeln!(out, "# {what}: strikes against Windows ME at 96 dpi");
	let _ = writeln!(out, "# fonts: {}", set.title);
	let _ = writeln!(
		out,
		"# widths in whole pixels at 96 dpi; the strike nearest the size is used (each of 8, 10, 12, 14, 18 and 24 pt at 1, 2 or 3 times), as the measure on DOS will"
	);
	let _ = writeln!(
		out,
		"# the strike's advances are skrifa's autohinted advances, rounded"
	);
	let _ = writeln!(out);
	let _ = writeln!(
		out,
		"face | half points | flags | strike (pt x scale) | glyphs wider or narrower | largest difference | ascent windows/strike | descent | leading"
	);
	let (mut glyphs, mut differing) = (0usize, 0usize);
	let (mut exact_g, mut exact_d) = (0usize, 0usize);
	for (face, size, flags, vm, widths) in captured() {
		let code = match face.as_str() {
			"Arial" => 2,
			"Courier New" => 3,
			_ => 1,
		};
		let f = font_ref(&face, code, size, flags);
		let mut w = [0u16; GLYPHS];
		m.widths(&f, &mut w);
		let diffs: Vec<i32> = w
			.iter()
			.zip(&widths)
			.map(|(a, b)| i32::from(*a) - i32::from(*b))
			.collect();
		let n = diffs.iter().filter(|d| **d != 0).count();
		let worst = diffs.iter().map(|d| d.abs()).max().unwrap_or(0);
		let vo = m.metrics(&f);
		let picked = m.pick(&f).map_or(String::from("none"), |(s, k)| {
			format!("{} x {k}", font.entries[s].half_points / 2)
		});
		glyphs += diffs.len();
		differing += n;
		if picked.ends_with("x 1")
			&& picked.split(' ').next().and_then(|p| p.parse::<u16>().ok()) == Some(size / 2)
		{
			exact_g += diffs.len();
			exact_d += n;
		}
		let _ = writeln!(
			out,
			"{face} | {size} | {flags} | {picked} | {n} of {} | {worst} | {}/{} | {}/{} | {}/{}",
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
	let _ = writeln!(
		out,
		"at the sizes the strikes have exactly (8, 10, 12, 14, 18, 24 pt): {exact_d} of {exact_g}"
	);
	let _ = writeln!(out);
	let me = std::fs::read_to_string(root().join("tests/golden/layout/letter.windows-me.txt"))
		.expect("the Windows ME layout dump")
		.replace("\r\n", "\n");
	let mine = {
		let (d, s) = load("letter.rtf");
		let mut text = String::new();
		let mut m = StrikeMeasure::new(&font);
		dump_story(&d, &s, &mut m, "strikes-96", StoryKind::Body, &mut text).expect("a dump");
		text
	};
	let (a, b) = (line_starts(&me), line_starts(&mine));
	let _ = writeln!(
		out,
		"letter.rtf: {} lines on Windows ME, {} with the strikes",
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
			"{} lines start in a different place (paragraph, character position, text):",
			only_me.len() + only_mine.len()
		);
		for (tag, v) in [("windows me only", &only_me), ("strikes only", &only_mine)] {
			for (para, start, text) in v.iter() {
				let _ = writeln!(out, "  {tag}: paragraph {para}, start {start}: {text}");
			}
		}
	}
	out
}
