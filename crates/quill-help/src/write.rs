// Writing a WinHelp 3.1 file: the internal files, the B+ trees and the topic records.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The layout was read from real 3.1 files (the ones shipped with Windows 3.11) and then checked by opening what this writes in the real
// WINHELP.EXE. Topic text is stored without phrase compression, and each topic block is "compressed" with literal-only LZ77, which every
// reader accepts and which keeps the compressor trivial.

use crate::{hash, HelpError};

/// A piece of a paragraph.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Run {
	/// Plain text.
	Text(String),
	/// Bold text.
	Bold(String),
	/// A jump: the words shown and the context name they open.
	Jump(String, String),
	/// A pop-up: the words shown and the context name whose topic pops up.
	Popup(String, String),
}

/// How a paragraph looks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
	Body,
	Heading,
	/// A bulleted item (hanging indent).
	Item,
}

/// One paragraph.
#[derive(Clone, Debug)]
pub struct Para {
	pub style: Style,
	pub runs: Vec<Run>,
}

/// One topic.
#[derive(Clone, Debug)]
pub struct Topic {
	/// The context name jumps and `HELP_CONTEXT` calls use.
	pub context: String,
	pub title: String,
	/// The number an application passes to `WinHelp(HELP_CONTEXT)` (the `[MAP]` section of a help project).
	pub map: Option<u32>,
	pub paras: Vec<Para>,
}

/// A whole help file.
#[derive(Clone, Debug, Default)]
pub struct Help {
	pub title: String,
	pub copyright: String,
	pub topics: Vec<Topic>,
}

// Font numbers in |FONT.
const F_BODY: u16 = 0;
const F_BOLD: u16 = 1;
const F_HEAD: u16 = 2;
const F_JUMP: u16 = 3;
const F_POPUP: u16 = 4;

fn put16(v: &mut Vec<u8>, x: u16) {
	v.extend_from_slice(&x.to_le_bytes());
}

fn put32(v: &mut Vec<u8>, x: u32) {
	v.extend_from_slice(&x.to_le_bytes());
}

/// A compressed signed long (two bytes, or four with the low bit set).
fn long(v: &mut Vec<u8>, x: i32) {
	if (-0x4000..0x4000).contains(&x) {
		put16(v, (x * 2 + 0x8000) as u16);
	} else {
		put32(v, (i64::from(x) * 2 + 0x8000_0001) as u32);
	}
}

fn ushort(v: &mut Vec<u8>, x: u32) {
	if x < 0x80 {
		v.push((x * 2) as u8);
	} else {
		put16(v, (x * 2 + 1) as u16);
	}
}

fn short(v: &mut Vec<u8>, x: i32) {
	if (-0x40..0x40).contains(&x) {
		v.push((x * 2 + 0x80) as u8);
	} else {
		put16(v, (x * 2 + 0x8001) as u16);
	}
}

/// One topic's display text as pieces and commands, ready for a record.
struct Built {
	text: Vec<u8>,
	format: Vec<u8>,
	pieces: u32,
}

fn build_para(
	p: &Para,
	ctx_hash: &dyn Fn(&str) -> Result<u32, HelpError>,
) -> Result<Built, HelpError> {
	let base = match p.style {
		Style::Heading => F_HEAD,
		_ => F_BODY,
	};
	let mut b = Built {
		text: Vec::new(),
		format: Vec::new(),
		pieces: 0,
	};
	let mut cur: Vec<u8> = Vec::new();
	// Every paragraph starts with a font command, as in the files the real compiler writes.
	let mut font = u16::MAX;
	macro_rules! cmd {
		($($byte:expr),+) => {{
			b.text.extend_from_slice(&cur);
			b.text.push(0);
			cur.clear();
			b.format.extend_from_slice(&[$($byte),+]);
			b.pieces += 1;
		}};
	}
	fn set_font(b: &mut Built, cur: &mut Vec<u8>, font: &mut u16, to: u16) {
		if *font != to {
			b.text.extend_from_slice(cur);
			b.text.push(0);
			cur.clear();
			b.format.push(0x80);
			b.format.extend_from_slice(&to.to_le_bytes());
			b.pieces += 1;
			*font = to;
		}
	}
	set_font(&mut b, &mut cur, &mut font, base);
	if p.style == Style::Item {
		// A bullet and a tab to the hanging indent.
		cur.extend_from_slice(&[0x95]);
		cmd!(0x83);
	}
	for r in &p.runs {
		match r {
			Run::Text(t) => {
				set_font(&mut b, &mut cur, &mut font, base);
				cur.extend(t.bytes());
			}
			Run::Bold(t) => {
				set_font(
					&mut b,
					&mut cur,
					&mut font,
					if base == F_HEAD { F_HEAD } else { F_BOLD },
				);
				cur.extend(t.bytes());
			}
			Run::Jump(t, c) | Run::Popup(t, c) => {
				let jump = matches!(r, Run::Jump(..));
				let h = ctx_hash(c)?;
				let code = if jump { 0xE3 } else { 0xE2 };
				// The hotspot: the link command, the hotspot font, the words and the end of the link.
				cmd!(
					code,
					h as u8,
					(h >> 8) as u8,
					(h >> 16) as u8,
					(h >> 24) as u8
				);
				set_font(
					&mut b,
					&mut cur,
					&mut font,
					if jump { F_JUMP } else { F_POPUP },
				);
				cur.extend(t.bytes());
				cmd!(0x89);
			}
		}
	}
	// The end of the paragraph, then the end of the record (each command takes the text before it).
	cmd!(0x82);
	b.text.extend_from_slice(&cur);
	b.text.push(0);
	b.pieces += 1;
	b.format.push(0xFF);
	Ok(b)
}

fn record(kind: u8, prev: u32, next: u32, data1: &[u8], data2: &[u8], text_len: u32) -> Vec<u8> {
	let mut r = Vec::new();
	put32(&mut r, (0x15 + data1.len() + data2.len()) as u32);
	put32(&mut r, text_len);
	put32(&mut r, prev);
	put32(&mut r, next);
	put32(&mut r, (0x15 + data1.len()) as u32);
	r.push(kind);
	r.extend_from_slice(data1);
	r.extend_from_slice(data2);
	r
}

/// The literal-only form of LZ77: a zero flag byte before every eight literals.
fn store_lz77(src: &[u8]) -> Vec<u8> {
	let mut out = Vec::with_capacity(src.len() + src.len() / 8 + 1);
	for chunk in src.chunks(8) {
		out.push(0);
		out.extend_from_slice(chunk);
	}
	out
}

const BLOCK: usize = 0x1000;
const BLOCK_DATA: usize = 3400;

/// The topic file and the topic offsets (for |CONTEXT) of each topic.
fn topic_file(
	h: &Help,
	ctx: &dyn Fn(&str) -> Result<u32, HelpError>,
) -> Result<(Vec<u8>, Vec<u32>), HelpError> {
	// Records are numbered by position: 0x0C plus the block number times 0x4000 plus the offset in the block.
	let mut blocks: Vec<Vec<u8>> = vec![Vec::new()];
	let mut firsts: Vec<u32> = vec![0xFFFF_FFFF];
	let mut lasts: Vec<u32> = vec![0xFFFF_FFFF];
	let mut last_hdr: Vec<u32> = vec![0];
	let mut offsets = Vec::new();
	let mut offs: u32 = 0;
	let mut prev_pos: u32 = 0xFFFF_FFFF;
	let mut prev_hdr_block_start = 0u32;
	let _ = &mut prev_hdr_block_start;
	let pos_of = |block: usize, at: usize| 0x0C + block as u32 * 0x4000 + at as u32;

	// First, the records' sizes decide the positions, so each is built twice: once to measure, once with real links.
	struct Rec {
		kind: u8,
		data1: Vec<u8>,
		data2: Vec<u8>,
		text_len: u32,
		topic: usize,
		para_len: u32,
	}
	let mut recs: Vec<Rec> = Vec::new();
	for (ti, t) in h.topics.iter().enumerate() {
		let mut title = t.title.clone().into_bytes();
		title.push(0);
		recs.push(Rec {
			kind: 0x02,
			data1: Vec::new(),
			data2: title.clone(),
			text_len: title.len() as u32,
			topic: ti,
			para_len: 0,
		});
		for p in &t.paras {
			let b = build_para(p, ctx)?;
			let tl = b.text.len() as u32;
			// What follows the first two numbers: four flag bytes, the bit field and its values, then the character format.
			let mut rest = vec![0x00, 0x80, 0x00, 0x00];
			let mut bits: u16 = 0x0004;
			match p.style {
				Style::Item => bits |= 0x0010 | 0x0040 | 0x0200,
				Style::Heading => bits |= 0x0002,
				Style::Body => {}
			}
			put16(&mut rest, bits);
			if bits & 0x0002 != 0 {
				short(&mut rest, 8);
			}
			if bits & 0x0004 != 0 {
				short(&mut rest, 8);
			}
			if bits & 0x0010 != 0 {
				short(&mut rest, 24);
			}
			if bits & 0x0040 != 0 {
				short(&mut rest, -18);
			}
			if bits & 0x0200 != 0 {
				short(&mut rest, 1);
				ushort(&mut rest, 24);
			}
			rest.extend_from_slice(&b.format);
			let mut d1 = Vec::new();
			// The first number is the length of everything after the second (as the real files have it).
			let mut second = Vec::new();
			ushort(&mut second, tl);
			long(&mut d1, rest.len() as i32);
			d1.extend_from_slice(&second);
			d1.extend_from_slice(&rest);
			recs.push(Rec {
				kind: 0x20,
				data1: d1,
				data2: b.text,
				text_len: tl,
				topic: ti,
				para_len: tl,
			});
		}
	}
	// Lay the records out in blocks and note each one's position.
	let mut place: Vec<(usize, usize, usize)> = Vec::new();
	let mut sizes: Vec<usize> = Vec::new();
	for r in &mut recs {
		let hdr = if r.kind == 0x02 { 28 } else { 0 };
		sizes.push(0x15 + r.data1.len() + hdr + r.data2.len());
	}
	let mut used = 0usize;
	let mut bi = 0usize;
	for sz in &sizes {
		if *sz > BLOCK_DATA {
			return Err(HelpError::Source(
				0,
				"a paragraph is too long for one topic block".into(),
			));
		}
		if used + sz > BLOCK_DATA {
			bi += 1;
			used = 0;
		}
		place.push((bi, used, *sz));
		used += sz;
	}
	let nblocks = bi + 1;
	blocks.resize(nblocks, Vec::new());
	firsts.resize(nblocks, 0xFFFF_FFFF);
	lasts.resize(nblocks, 0xFFFF_FFFF);
	last_hdr.resize(nblocks, 0);
	let mut topic_pos: Vec<u32> = vec![0; h.topics.len()];
	let mut topic_first_para: Vec<u32> = vec![0xFFFF_FFFF; h.topics.len()];
	let mut next_hdr_pos: Vec<u32> = vec![0xFFFF_FFFF; h.topics.len()];
	for (i, r) in recs.iter().enumerate() {
		let (b, at, _) = place[i];
		let p = pos_of(b, at);
		if r.kind == 0x02 {
			topic_pos[r.topic] = p;
		} else if topic_first_para[r.topic] == 0xFFFF_FFFF {
			topic_first_para[r.topic] = p;
		}
	}
	for (t, next) in topic_pos.iter().skip(1).enumerate() {
		next_hdr_pos[t] = *next;
	}
	let mut topic_no = 0u32;
	let mut cur_block = usize::MAX;
	let mut blk_topic_bytes: Vec<u32> = vec![0; h.topics.len()];
	for (i, r) in recs.iter().enumerate() {
		blk_topic_bytes[r.topic] += sizes[i] as u32;
	}
	for (i, r) in recs.iter().enumerate() {
		let (b, at, sz) = place[i];
		let pos = pos_of(b, at);
		if b != cur_block {
			cur_block = b;
			offs = 0;
		}
		let next = if i + 1 < recs.len() {
			pos_of(place[i + 1].0, place[i + 1].1)
		} else {
			0xFFFF_FFFF
		};
		let rec = if r.kind == 0x02 {
			let mut d1 = Vec::new();
			put32(&mut d1, blk_topic_bytes[r.topic]);
			put32(&mut d1, 0xFFFF_FFFF);
			put32(&mut d1, 0xFFFF_FFFF);
			topic_no += 1;
			put32(&mut d1, topic_no);
			put32(&mut d1, 0xFFFF_FFFF);
			put32(&mut d1, topic_first_para[r.topic]);
			put32(&mut d1, next_hdr_pos[r.topic]);
			offsets.push(b as u32 * 0x8000 + offs);
			record(0x02, prev_pos, next, &d1, &r.data2, r.text_len)
		} else {
			offs += r.para_len;
			record(0x20, prev_pos, next, &r.data1, &r.data2, r.text_len)
		};
		debug_assert_eq!(rec.len(), sz);
		if firsts[b] == 0xFFFF_FFFF {
			firsts[b] = pos;
		}
		lasts[b] = pos;
		if r.kind == 0x02 {
			last_hdr[b] = pos;
		}
		blocks[b].extend_from_slice(&rec);
		prev_pos = pos;
	}
	let mut file = Vec::new();
	let mut prev_last = 0xFFFF_FFFFu32;
	let mut hdr_so_far = 0u32;
	for (b, data) in blocks.iter().enumerate() {
		let mut head = Vec::new();
		put32(&mut head, prev_last);
		put32(&mut head, firsts[b]);
		if last_hdr[b] != 0 {
			hdr_so_far = last_hdr[b];
		}
		put32(&mut head, hdr_so_far);
		let mut chunk = head;
		chunk.extend_from_slice(&store_lz77(data));
		if chunk.len() > BLOCK {
			return Err(HelpError::Source(0, "a topic block overflowed".into()));
		}
		chunk.resize(BLOCK, 0);
		file.extend_from_slice(&chunk);
		prev_last = lasts[b];
	}
	// The last block need not be padded.
	if let Some(end) = file.len().checked_sub(1) {
		let _ = end;
	}
	Ok((file, offsets))
}

/// A B+ tree with `entries` (sorted key bytes, payload bytes), page size `ps` and the structure string `kind`.
fn btree(
	kind: &str,
	ps: usize,
	entries: &[(Vec<u8>, Vec<u8>)],
	key_is_string: bool,
) -> Result<Vec<u8>, HelpError> {
	// Leaves.
	let mut pages: Vec<Vec<u8>> = Vec::new();
	let mut leaf_first_key: Vec<Vec<u8>> = Vec::new();
	let mut cur: Vec<u8> = Vec::new();
	let mut n = 0u16;
	let flush = |pages: &mut Vec<Vec<u8>>, cur: &mut Vec<u8>, n: &mut u16| {
		if *n > 0 {
			pages.push(std::mem::take(cur));
			*n = 0;
		}
	};
	for (k, v) in entries {
		let mut e = k.clone();
		if key_is_string {
			e.push(0);
		}
		e.extend_from_slice(v);
		if 8 + cur.len() + e.len() > ps {
			flush(&mut pages, &mut cur, &mut n);
		}
		if n == 0 {
			leaf_first_key.push(k.clone());
		}
		cur.extend_from_slice(&e);
		n += 1;
	}
	flush(&mut pages, &mut cur, &mut n);
	if pages.is_empty() {
		pages.push(Vec::new());
		leaf_first_key.push(Vec::new());
	}
	let count_of = |entries: &[(Vec<u8>, Vec<u8>)], pages: &[Vec<u8>]| -> Vec<u16> {
		let mut left = entries.len();
		let _ = &mut left;
		let mut out = Vec::new();
		let mut idx = 0usize;
		for p in pages {
			// Count entries by re-walking.
			let mut at = 0;
			let mut c = 0u16;
			while at < p.len() {
				let (k, v) = &entries[idx];
				at += k.len() + usize::from(key_is_string) + v.len();
				idx += 1;
				c += 1;
			}
			out.push(c);
		}
		out
	};
	let counts = count_of(entries, &pages);
	let nleaf = pages.len();
	let mut out_pages: Vec<Vec<u8>> = Vec::new();
	for (i, p) in pages.iter().enumerate() {
		let mut pg = Vec::new();
		put16(&mut pg, (ps - 8 - p.len()) as u16);
		put16(&mut pg, counts[i]);
		put16(&mut pg, if i == 0 { 0xFFFF } else { (i - 1) as u16 });
		put16(
			&mut pg,
			if i + 1 == nleaf {
				0xFFFF
			} else {
				(i + 1) as u16
			},
		);
		pg.extend_from_slice(p);
		pg.resize(ps, 0);
		out_pages.push(pg);
	}
	let mut levels = 1u16;
	let mut root = 0u16;
	if nleaf > 1 {
		// One index page over the leaves (enough for the help files this makes).
		let mut pg = Vec::new();
		let mut body = Vec::new();
		for (i, k) in leaf_first_key.iter().enumerate().skip(1) {
			body.extend_from_slice(k);
			if key_is_string {
				body.push(0);
			}
			put16(&mut body, i as u16);
		}
		if 6 + body.len() > ps {
			return Err(HelpError::Source(
				0,
				"too many entries for a two-level tree".into(),
			));
		}
		put16(&mut pg, (ps - 6 - body.len()) as u16);
		put16(&mut pg, (nleaf - 1) as u16);
		put16(&mut pg, 0);
		pg.extend_from_slice(&body);
		pg.resize(ps, 0);
		root = out_pages.len() as u16;
		out_pages.push(pg);
		levels = 2;
	}
	let mut t = Vec::new();
	put16(&mut t, 0x293B);
	put16(&mut t, 0x0002);
	put16(&mut t, ps as u16);
	let mut s = [0u8; 16];
	s[..kind.len()].copy_from_slice(kind.as_bytes());
	t.extend_from_slice(&s);
	put16(&mut t, 0);
	put16(&mut t, 0);
	put16(&mut t, root);
	put16(&mut t, 0xFFFF);
	put16(&mut t, out_pages.len() as u16);
	put16(&mut t, levels);
	put32(&mut t, entries.len() as u32);
	for p in out_pages {
		t.extend_from_slice(&p);
	}
	Ok(t)
}

fn font_file() -> Vec<u8> {
	// Face names are 20 bytes each; the descriptors are 11: attributes, half-points, family, face, foreground, background.
	let faces = ["Helv", "Times New Roman"];
	let flen = 20usize;
	let mut v = Vec::new();
	put16(&mut v, faces.len() as u16);
	put16(&mut v, 5);
	put16(&mut v, 8);
	put16(&mut v, (8 + faces.len() * flen) as u16);
	for f in faces {
		let mut b = vec![0u8; flen];
		b[..f.len()].copy_from_slice(f.as_bytes());
		v.extend_from_slice(&b);
	}
	let d = |attr: u8, half: u8, face: u16, fg: [u8; 3]| {
		let mut x = vec![attr, half, 0x03];
		x.extend_from_slice(&face.to_le_bytes());
		x.extend_from_slice(&fg);
		x.extend_from_slice(&[0xFF, 0xFF, 0xFF]);
		x
	};
	let _ = (F_BODY, F_BOLD, F_HEAD, F_JUMP, F_POPUP);
	v.extend(d(0, 20, 0, [0, 0, 0]));
	v.extend(d(1, 20, 0, [0, 0, 0]));
	v.extend(d(1, 28, 0, [0, 0, 128]));
	v.extend(d(4, 20, 0, [0, 128, 0]));
	v.extend(d(0x10, 20, 0, [0, 128, 0]));
	v
}

fn system_file(h: &Help) -> Vec<u8> {
	let mut v = Vec::new();
	put16(&mut v, 0x036C);
	put16(&mut v, 21);
	put16(&mut v, 1);
	put32(&mut v, 0x2900_0000 | 0x00B2_0609);
	// Flags 4: 4 KB topic blocks, LZ77-compressed.
	put16(&mut v, 4);
	let mut rec = |kind: u16, body: &[u8]| {
		put16(&mut v, kind);
		put16(&mut v, body.len() as u16);
		v.extend_from_slice(body);
	};
	let mut t = h.title.clone().into_bytes();
	t.push(0);
	rec(1, &t);
	if !h.copyright.is_empty() {
		let mut c = h.copyright.clone().into_bytes();
		c.push(0);
		rec(2, &c);
	}
	rec(3, &0u32.to_le_bytes());
	v
}

/// Builds the bytes of a help file.
pub fn build(h: &Help) -> Result<Vec<u8>, HelpError> {
	if h.topics.is_empty() {
		return Err(HelpError::Source(0, "a help file needs a topic".into()));
	}
	let ctx = |name: &str| -> Result<u32, HelpError> {
		if h.topics.iter().any(|t| t.context == name) {
			Ok(hash(name) as u32)
		} else {
			Err(HelpError::Source(
				0,
				format!("a link names the context {name:?}, which no topic has"),
			))
		}
	};
	let (topic, offsets) = topic_file(h, &ctx)?;
	let mut cx: Vec<(Vec<u8>, Vec<u8>)> = h
		.topics
		.iter()
		.zip(&offsets)
		.map(|(t, o)| {
			(
				hash(&t.context).to_le_bytes().to_vec(),
				o.to_le_bytes().to_vec(),
			)
		})
		.collect();
	cx.sort_by_key(|(k, _)| i32::from_le_bytes([k[0], k[1], k[2], k[3]]));
	for w in cx.windows(2) {
		if w[0].0 == w[1].0 {
			return Err(HelpError::Source(0, "two contexts share a hash".into()));
		}
	}
	let context = btree("L4", 0x400, &cx, false)?;
	let mut files: Vec<(&str, Vec<u8>)> = vec![
		("|CONTEXT", context),
		("|FONT", font_file()),
		("|SYSTEM", system_file(h)),
		("|TOPIC", topic),
	];
	let maps: Vec<(u32, u32)> = h
		.topics
		.iter()
		.zip(&offsets)
		.filter_map(|(t, o)| t.map.map(|m| (m, *o)))
		.collect();
	if !maps.is_empty() {
		let mut m = Vec::new();
		put16(&mut m, maps.len() as u16);
		for (id, off) in &maps {
			put32(&mut m, *id);
			put32(&mut m, *off);
		}
		files.push(("|CTXOMAP", m));
	}
	// Lay out: the file header, the directory, then each internal file.
	let mut dir_entries: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
	let mut sized: Vec<(usize, &Vec<u8>)> = Vec::new();
	// The directory's size depends on its entries only, so build it with placeholder offsets first.
	let mut offs = Vec::new();
	let placeholder: Vec<(Vec<u8>, Vec<u8>)> = files
		.iter()
		.map(|(n, _)| (n.as_bytes().to_vec(), vec![0u8; 4]))
		.collect();
	let dir_len = 9 + btree("z4", 0x400, &placeholder, true)?.len();
	let mut at = 16 + dir_len;
	for (_, data) in &files {
		offs.push(at);
		at += 9 + data.len();
	}
	for ((n, _), o) in files.iter().zip(&offs) {
		dir_entries.push((n.as_bytes().to_vec(), (*o as u32).to_le_bytes().to_vec()));
	}
	dir_entries.sort();
	let dir = btree("z4", 0x400, &dir_entries, true)?;
	let mut out = Vec::new();
	put32(&mut out, 0x0003_5F3F);
	put32(&mut out, 16);
	put32(&mut out, 0xFFFF_FFFF);
	put32(&mut out, 0);
	let add = |out: &mut Vec<u8>, data: &[u8]| {
		put32(out, (data.len() + 9) as u32);
		put32(out, data.len() as u32);
		out.push(0);
		out.extend_from_slice(data);
	};
	add(&mut out, &dir);
	for (_, data) in &files {
		add(&mut out, data);
	}
	let _ = &mut sized;
	let total = out.len() as u32;
	out[12..16].copy_from_slice(&total.to_le_bytes());
	Ok(out)
}
