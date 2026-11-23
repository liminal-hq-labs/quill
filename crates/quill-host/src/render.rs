// quill-render: lays a document out with a measure and writes the text dump the goldens compare against.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::fmt::Write;

use quill_core::doc::{Doc, StoryKind};
use quill_core::layout::{layout_story, Measure};
use quill_core::store::Store;

/// The text dump of a story's layout: one line per line box, with its text after a bar.
pub fn text_dump<S: Store, M: Measure>(
	doc: &Doc,
	s: &S,
	m: &mut M,
	name: &str,
	kind: StoryKind,
) -> String {
	let (rx, ry) = m.resolution();
	let mut out = String::new();
	let _ = writeln!(out, "# quill layout dump 1");
	let _ = writeln!(out, "# measure {name}, {rx} x {ry} units per inch");
	let text = doc.story(kind);
	let mut lines = Vec::new();
	let pages = layout_story(doc, s, m, kind, |l| lines.push(*l));
	let _ = writeln!(
		out,
		"# {} characters, {} lines, {} pages",
		text.len(),
		lines.len(),
		pages
	);
	for l in &lines {
		let mut buf = vec![0u8; l.len as usize];
		text.read(s, l.start, &mut buf);
		let shown: String = buf
			.iter()
			.map(|b| match b {
				0x09 => '→',
				0x0b => '↵',
				0x20..=0x7e => char::from(*b),
				_ => '.',
			})
			.collect();
		let _ = writeln!(
			out,
			"page {} y {} para {} line {} start {} len {} x {} width {} ascent {} descent {} | {}",
			l.page, l.y, l.para, l.line, l.start, l.len, l.x, l.width, l.ascent, l.descent, shown
		);
	}
	out
}

/// A page map: the page's lines drawn as blocks, one block per character cell (`cw` by `ch` pixels), with the text
/// area outlined. Deterministic, small, and enough to see where lines break. Returns a PNG.
pub fn page_map_png<S: Store, M: Measure>(
	doc: &Doc,
	s: &S,
	m: &mut M,
	kind: StoryKind,
	page: u32,
	cw: u32,
	ch: u32,
) -> Vec<u8> {
	let (rx, ry) = m.resolution();
	let units =
		|tw: i32, res: u16| ((i64::from(tw) * i64::from(res) * 2 + 1440).div_euclid(2880)) as u32;
	let (pw, ph) = (units(doc.page.paper_w, rx), units(doc.page.paper_h, ry));
	let (ml, mt) = (units(doc.page.margin_l, rx), units(doc.page.margin_t, ry));
	let (tw, th) = (
		units(doc.page.paper_w - doc.page.margin_l - doc.page.margin_r, rx),
		units(doc.page.paper_h - doc.page.margin_t - doc.page.margin_b, ry),
	);
	let (w, h) = (pw * cw, ph * ch);
	let stride = w.div_ceil(8) as usize;
	let mut img = vec![vec![0xFFu8; stride]; h as usize];
	let mut put = |x: u32, y: u32, black: bool| {
		if x < w && y < h {
			let byte = &mut img[y as usize][(x / 8) as usize];
			let bit = 0x80u8 >> (x % 8);
			if black {
				*byte &= !bit;
			} else {
				*byte |= bit;
			}
		}
	};
	// The text area's corners.
	for (cx, cy) in [(ml, mt), (ml + tw, mt), (ml, mt + th), (ml + tw, mt + th)] {
		for d in 0..4 {
			put(cx * cw + d, cy * ch, true);
			put(cx * cw, cy * ch + d, true);
			put((cx * cw).saturating_sub(d), cy * ch, true);
			put(cx * cw, (cy * ch).saturating_sub(d), true);
		}
	}
	let text = doc.story(kind);
	let mut lines = Vec::new();
	layout_story(doc, s, m, kind, |l| lines.push(*l));
	for l in lines.iter().filter(|l| l.page == page) {
		let mut buf = vec![0u8; l.len as usize];
		text.read(s, l.start, &mut buf);
		let mut col = l.x;
		for b in buf {
			let wcell = if b == 0x09 { 5 - (col % 5) } else { 1 };
			if b > 0x20 && b != 0x0b {
				let (px, py) = ((ml as i32 + col) as u32 * cw, (mt + l.y as u32) * ch);
				for dy in 0..ch.saturating_sub(2) {
					for dx in 0..cw.saturating_sub(1) {
						put(px + dx, py + dy + 1, true);
					}
				}
			}
			col += wcell;
		}
	}
	crate::png::encode_1bit(w, h, &img)
}
