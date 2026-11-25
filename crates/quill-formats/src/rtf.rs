// The RTF subset: a canonical writer and a forgiving reader for characters, paragraphs, tabs and the page setup.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Lists, headers and footers, fields and pictures are not read or written yet (the SPEC lists them for later milestones); a
//! reader skips what it does not know, as the SPEC requires, so such files open with their text and formats.

use quill_core::doc::{
	cflag, CFmt, Colour, Doc, Font, PFmt, StoryKind, TabSet, TabStop, MAX_COLOURS, MAX_FONTS,
	MAX_STOPS, NO_TABS, PAGE_LIMIT,
};
use quill_core::store::{ReadStore, Store};
use quill_core::text::MAX_TEXT;
use quill_text::codepage::CodePage;
use quill_text::utf8::Refused;

use crate::io::{Sink, Source};
use crate::{FormatError, Result};

const MAX_DEPTH: usize = 64;
const MAX_WORD: usize = 32;

// ---------------------------------------------------------------------------------------------------------------- writer

struct Out<'a, K: Sink> {
	k: &'a mut K,
	ok: bool,
	col: usize,
}

impl<K: Sink> Out<'_, K> {
	fn put(&mut self, d: &[u8]) {
		if self.ok && !self.k.write(d) {
			self.ok = false;
		}
		self.col += d.len();
	}
	fn s(&mut self, t: &str) {
		self.put(t.as_bytes());
	}
	fn num(&mut self, v: i64) {
		let mut buf = [0u8; 24];
		let mut i = buf.len();
		let neg = v < 0;
		let mut u = v.unsigned_abs();
		loop {
			i -= 1;
			if let Some(b) = buf.get_mut(i) {
				*b = b'0' + (u % 10) as u8;
			}
			u /= 10;
			if u == 0 {
				break;
			}
		}
		if neg {
			self.put(b"-");
		}
		self.put(buf.get(i..).unwrap_or(&[]));
	}
	fn word(&mut self, w: &str, v: i64) {
		self.s("\\");
		self.s(w);
		self.num(v);
	}
	fn nl(&mut self) {
		self.put(b"\r\n");
		self.col = 0;
	}
}

fn family_word(f: u8) -> &'static str {
	match f {
		1 => "\\froman",
		2 => "\\fswiss",
		3 => "\\fmodern",
		4 => "\\fscript",
		5 => "\\fdecor",
		_ => "\\fnil",
	}
}

/// Writes `doc` as RTF. The same document always gives the same bytes.
pub fn write<S: ReadStore + ?Sized, K: Sink>(doc: &Doc, s: &S, k: &mut K) -> Result<()> {
	let mut o = Out {
		k,
		ok: true,
		col: 0,
	};
	o.s("{\\rtf1\\ansi\\ansicpg1252\\deff0");
	o.nl();
	o.s("{\\fonttbl");
	for i in 0..doc.fonts.len() {
		if let Some(f) = doc.fonts.get(s, i as u16) {
			o.s("{\\f");
			o.num(i64::from(i));
			o.s(family_word(f.family));
			o.s(" ");
			o.put(f.name());
			o.s(";}");
		}
	}
	o.s("}");
	o.nl();
	o.s("{\\colortbl;");
	for i in 1..doc.colours.len() {
		if let Some(c) = doc.colours.get(s, i as u16) {
			o.word("red", i64::from(c.r));
			o.word("green", i64::from(c.g));
			o.word("blue", i64::from(c.b));
			o.s(";");
		}
	}
	o.s("}");
	o.nl();
	let p = &doc.page;
	o.word("paperw", i64::from(p.paper_w));
	o.word("paperh", i64::from(p.paper_h));
	o.word("margl", i64::from(p.margin_l));
	o.word("margr", i64::from(p.margin_r));
	o.word("margt", i64::from(p.margin_t));
	o.word("margb", i64::from(p.margin_b));
	o.word("headery", i64::from(p.header_dist));
	o.word("footery", i64::from(p.footer_dist));
	if p.flags & 1 != 0 {
		o.s("\\titlepg");
	}
	if p.first_page != 1 {
		o.word("pgnstart", i64::from(p.first_page));
	}
	o.nl();

	for kind in [
		StoryKind::Header,
		StoryKind::Footer,
		StoryKind::FirstHeader,
		StoryKind::FirstFooter,
	] {
		if doc.story(kind).is_empty() {
			continue;
		}
		o.s(match kind {
			StoryKind::Header => "{\\header ",
			StoryKind::Footer => "{\\footer ",
			StoryKind::FirstHeader => "{\\headerf ",
			_ => "{\\footerf ",
		});
		write_story(&mut o, doc, s, kind);
		o.s("}");
		o.nl();
	}
	write_story(&mut o, doc, s, StoryKind::Body);
	o.s("}");
	o.nl();
	if o.ok {
		Ok(())
	} else {
		Err(FormatError::Write)
	}
}

fn write_story<S: ReadStore + ?Sized, K: Sink>(
	o: &mut Out<'_, K>,
	doc: &Doc,
	s: &S,
	kind: StoryKind,
) {
	let t = doc.story(kind);
	let total = t.len();
	let mut para_start = 0u32;
	let mut at = 0u32;
	let mut buf = [0u8; 128];
	let mut para_open = false;
	let mut cur_cfmt: Option<u16> = None;
	while at < total {
		let n = t.read(s, at, &mut buf);
		if n == 0 {
			break;
		}
		for (i, &b) in buf.iter().take(n).enumerate() {
			let pos = at + i as u32;
			if !para_open {
				let mark = para_end(doc, s, kind, pos);
				let pf = mark
					.and_then(|m| t.pfmt_at(s, m))
					.and_then(|x| doc.pfmts.get(s, x))
					.unwrap_or(PFmt::DEFAULT);
				para_props(o, doc, s, &pf);
				para_start = pos;
				para_open = true;
				cur_cfmt = None;
			}
			if b == 0x0D {
				o.s("\\par");
				o.nl();
				para_open = false;
				continue;
			}
			let cf = t.cfmt_at(s, pos).unwrap_or(0);
			if cur_cfmt != Some(cf) {
				if cur_cfmt.is_some() || pos != para_start {
					// A new run inside a paragraph starts from plain text.
				}
				char_props(o, doc, s, cf);
				cur_cfmt = Some(cf);
			}
			match b {
				b'\\' | b'{' | b'}' => {
					o.put(&[b'\\', b]);
				}
				0x09 => o.s("\\tab "),
				0x0B => o.s("\\line "),
				0x01 => match t.object_at(s, pos) {
					Some(v) if v >> 16 == 0 => picture(o, doc, s, v & 0xFFFF),
					v => field(o, v),
				},
				0x20..=0x7E => o.put(&[b]),
				_ => {
					o.s("\\'");
					o.put(&[hex(b >> 4), hex(b & 15)]);
				}
			}
			if o.col > 100 && b == b' ' {
				o.nl();
			}
		}
		at += n as u32;
	}
}

/// Writes picture `idx` as a `\pict` group: PNG as `\pngblip`, a BMP as a `\dibitmap0` without its file header.
fn picture<S: ReadStore + ?Sized, K: Sink>(o: &mut Out<'_, K>, doc: &Doc, s: &S, idx: u32) {
	let Some(p) = doc.pictures.get(s, idx) else {
		return;
	};
	let mut skip = 0u32;
	o.s("{\\pict");
	if p.kind == 1 {
		o.s("\\pngblip");
	} else {
		let mut h = [0u8; 30];
		doc.pic_data.read(s, p.offset, &mut h);
		let bpp = i64::from(h.get(28).copied().unwrap_or(24))
			| (i64::from(h.get(29).copied().unwrap_or(0)) << 8);
		o.s("\\dibitmap0");
		o.word("wbmbitspixel", bpp);
		o.word("wbmplanes", 1);
		o.word("wbmwidthbytes", (i64::from(p.w) * bpp + 31) / 32 * 4);
		skip = 14;
	}
	o.word("picw", i64::from(p.w));
	o.word("pich", i64::from(p.h));
	o.word("picwgoal", i64::from(p.shown_w));
	o.word("pichgoal", i64::from(p.shown_h));
	o.nl();
	let mut at = skip;
	let mut buf = [0u8; 48];
	while at < p.len {
		let n = doc.picture_bytes(s, idx as u16, at, &mut buf);
		if n == 0 {
			break;
		}
		for &b in buf.iter().take(n) {
			o.put(&[hex(b >> 4), hex(b & 15)]);
		}
		o.nl();
		at += n as u32;
	}
	o.s("}");
}

fn field<K: Sink>(o: &mut Out<'_, K>, start: Option<u32>) {
	let (name, shown) = match start.map(|v| v >> 16) {
		Some(1) => ("PAGE", "1"),
		Some(2) => ("NUMPAGES", "1"),
		Some(3) => ("DATE", "1/1/1990"),
		Some(4) => ("FILENAME", "UNTITLED"),
		_ => return,
	};
	o.s("{\\field{\\*\\fldinst ");
	o.s(name);
	o.s("}{\\fldrslt ");
	o.s(shown);
	o.s("}}");
}

fn hex(n: u8) -> u8 {
	if n < 10 {
		b'0' + n
	} else {
		b'a' + n - 10
	}
}

fn para_end<S: ReadStore + ?Sized>(doc: &Doc, s: &S, kind: StoryKind, from: u32) -> Option<u32> {
	let t = doc.story(kind);
	let mut at = from;
	let mut buf = [0u8; 128];
	while at < t.len() {
		let n = t.read(s, at, &mut buf);
		if n == 0 {
			break;
		}
		for (i, b) in buf.iter().take(n).enumerate() {
			if *b == 0x0D {
				return Some(at + i as u32);
			}
		}
		at += n as u32;
	}
	None
}

fn para_props<S: ReadStore + ?Sized, K: Sink>(o: &mut Out<'_, K>, doc: &Doc, s: &S, pf: &PFmt) {
	let mut group = false;
	o.s("\\pard");
	o.s(match pf.align {
		1 => "\\qc",
		2 => "\\qr",
		3 => "\\qj",
		_ => "\\ql",
	});
	if pf.left != 0 {
		o.word("li", i64::from(pf.left));
	}
	if pf.right != 0 {
		o.word("ri", i64::from(pf.right));
	}
	if pf.first != 0 {
		o.word("fi", i64::from(pf.first));
	}
	if pf.before != 0 {
		o.word("sb", i64::from(pf.before));
	}
	if pf.after != 0 {
		o.word("sa", i64::from(pf.after));
	}
	if pf.spacing != 0 {
		o.word("sl", if pf.spacing == 1 { 360 } else { 480 });
		o.s("\\slmult1");
	}
	if pf.flags & 2 != 0 {
		o.s("\\pagebb");
	}
	if pf.flags & 5 != 0 {
		// The Word 6 form, which every reader since has understood.
		group = true;
		o.s("{\\*\\pn");
		if pf.flags & 1 != 0 {
			o.s("\\pnlvlblt\\pnindent360{\\pntxtb\\'95}}");
		} else {
			let l = doc.lists.get(s, pf.list).unwrap_or_default();
			let lv = l
				.levels
				.get(usize::from(pf.level))
				.copied()
				.unwrap_or_default();
			o.s("\\pnlvlbody");
			o.s(match lv.style {
				2 | 3 => "\\pnlcltr",
				4 => "\\pnlcrm",
				5 => "\\pnucltr",
				6 => "\\pnucrm",
				_ => "\\pndec",
			});
			o.word("pnstart", i64::from(lv.start.max(1)));
			o.s("\\pnindent360{\\pntxta ");
			o.s(if lv.style == 1 || lv.style == 3 {
				")"
			} else {
				"."
			});
			o.s("}}");
		}
		if pf.level > 0 {
			o.word("ilvl", i64::from(pf.level));
		}
	}
	if pf.tabset != NO_TABS {
		if let Some(ts) = doc.tabsets.get(s, pf.tabset) {
			for st in ts.stops.iter().take(usize::from(ts.count)) {
				o.s(match st.kind {
					1 => "\\tqc",
					2 => "\\tqr",
					3 => "\\tqdec",
					_ => "",
				});
				o.s(match st.leader {
					1 => "\\tldot",
					2 => "\\tlhyph",
					3 => "\\tlul",
					_ => "",
				});
				o.word("tx", i64::from(st.pos));
			}
		}
	}
	// A group is not followed by a delimiter space: the next control word starts the run.
	if !group {
		o.s(" ");
	}
}

fn char_props<S: ReadStore + ?Sized, K: Sink>(o: &mut Out<'_, K>, doc: &Doc, s: &S, cf: u16) {
	let c = doc.cfmts.get(s, cf).unwrap_or(CFmt {
		font: 0,
		size: 24,
		colour: 0,
		vpos: 0,
		flags: 0,
	});
	o.s("\\plain");
	o.word("f", i64::from(c.font));
	o.word("fs", i64::from(c.size));
	if c.flags & cflag::BOLD != 0 {
		o.s("\\b");
	}
	if c.flags & cflag::ITALIC != 0 {
		o.s("\\i");
	}
	if c.flags & cflag::UNDERLINE != 0 {
		o.s("\\ul");
	}
	if c.flags & cflag::STRIKE != 0 {
		o.s("\\strike");
	}
	match c.vpos {
		1 => o.s("\\super"),
		2 => o.s("\\sub"),
		_ => {}
	}
	if c.colour != 0 {
		o.word("cf", i64::from(c.colour));
	}
	o.s(" ");
}

// ---------------------------------------------------------------------------------------------------------------- reader

#[derive(Copy, Clone)]
struct CharState {
	font: u16,
	size: u16,
	colour: u8,
	vpos: u8,
	flags: u16,
}

#[derive(Copy, Clone)]
struct ParaState {
	align: u8,
	spacing: u8,
	flags: u16,
	left: i32,
	right: i32,
	first: i32,
	before: i32,
	after: i32,
	tabs: TabSet,
	next_kind: u8,
	next_leader: u8,
	/// 0 none, 1 bullet, 2 numbered.
	list: u8,
	style: u8,
	start: u16,
	level: u8,
}

const PARA0: ParaState = ParaState {
	align: 0,
	spacing: 0,
	flags: 0,
	left: 0,
	right: 0,
	first: 0,
	before: 0,
	after: 0,
	tabs: TabSet {
		count: 0,
		stops: [TabStop {
			pos: 0,
			kind: 0,
			leader: 0,
		}; MAX_STOPS],
	},
	next_kind: 0,
	next_leader: 0,
	list: 0,
	style: 0,
	start: 1,
	level: 0,
};

#[derive(Copy, Clone, PartialEq, Eq)]
enum Dest {
	Body,
	FontTable,
	ColourTable,
	Skip,
	Pn,
	Field,
	FldInst,
	Pict,
}

/// How the reader turns the bytes of `\'hh` (and of 8-bit text) into characters.
#[derive(Copy, Clone, PartialEq, Eq)]
enum Enc {
	/// One byte a character, in this code page.
	Single(CodePage),
	/// A double-byte code page (Japanese, Chinese, Korean): Quill cannot show it, so each character is a `?`.
	Double,
	/// The font names no code page, so the document's applies.
	Unset,
}

impl Enc {
	/// What a font's `\fcharsetN` means.
	fn from_charset(n: i64) -> Enc {
		match n {
			// Shift-JIS, Hangul, Johab, GB 2312 and Big 5.
			128 | 129 | 130 | 134 | 136 => Enc::Double,
			_ => CodePage::from_charset(n).map_or(Enc::Unset, Enc::Single),
		}
	}

	/// What `\ansicpgN` means, or `None` for a page the reader leaves at the default.
	fn from_ansicpg(n: i64) -> Option<Enc> {
		match n {
			932 | 936 | 949 | 950 | 1361 => Some(Enc::Double),
			_ => CodePage::from_number(n).map(Enc::Single),
		}
	}
}

#[derive(Copy, Clone)]
struct Group {
	chars: CharState,
	dest: Dest,
	uc_skip: u32,
	/// The story the text goes to: 0 body, 1 header, 2 footer, 3 first header, 4 first footer.
	story: u8,
}

struct Reader<'a, R: Source> {
	r: &'a mut R,
	peeked: Option<u8>,
	buf: [u8; 256],
	at: usize,
	len: usize,
}

impl<R: Source> Reader<'_, R> {
	fn next(&mut self) -> Option<u8> {
		if let Some(b) = self.peeked.take() {
			return Some(b);
		}
		if self.at >= self.len {
			self.len = self.r.read(&mut self.buf);
			self.at = 0;
			if self.len == 0 {
				return None;
			}
		}
		let b = self.buf.get(self.at).copied();
		self.at += 1;
		b
	}
	fn unread(&mut self, b: u8) {
		self.peeked = Some(b);
	}
}

struct State<'a, S: Store + ?Sized> {
	doc: &'a mut Doc,
	s: &'a mut S,
	chars: CharState,
	para: ParaState,
	font_map: [u16; MAX_FONTS as usize],
	font_count: usize,
	colour_map: [u8; MAX_COLOURS as usize],
	colour_count: usize,
	deff: u16,
	font_cur: Option<(usize, u8, u8, [u8; 31])>,
	/// The document's code page (`\ansicpg`), and each font's own (`\fcharset`), by RTF font number.
	doc_enc: Enc,
	font_enc: [Enc; MAX_FONTS as usize],
	/// A high surrogate from `\uN` waiting for its low half (0 when none).
	hi: u16,
	/// The second byte of a double-byte character is still to come, and is dropped.
	trail: bool,
	/// Characters the document cannot hold, each replaced by `?`.
	refused: usize,
	colour_cur: (u8, u8, u8, bool),
	pending: [u8; 128],
	pending_len: usize,
	pending_cf: u16,
	pagebb_next: bool,
	text_total: u32,
	last_mark: bool,
	target: u8,
	pending_story: u8,
	fld: [u8; 12],
	fld_len: usize,
	pic: PictState,
	shp_ok: bool,
}

/// A `\pict` group being read.
#[derive(Copy, Clone, Default)]
struct PictState {
	/// 0 none yet, 1 PNG, 2 DIB, 3 a kind Quill cannot show.
	kind: u8,
	picw: i32,
	pich: i32,
	goalw: i32,
	goalh: i32,
	hi: Option<u8>,
	start: u32,
	len: u32,
	failed: bool,
}

impl<S: Store + ?Sized> State<'_, S> {
	/// One hexadecimal digit of picture data.
	fn pict_hex(&mut self, b: u8) {
		let v = match b {
			b'0'..=b'9' => b - b'0',
			b'a'..=b'f' => b - b'a' + 10,
			b'A'..=b'F' => b - b'A' + 10,
			_ => return,
		};
		let Some(hi) = self.pic.hi.take() else {
			self.pic.hi = Some(v);
			return;
		};
		if self.pic.kind == 0 || self.pic.kind == 3 || self.pic.failed {
			return;
		}
		if self.pic.len >= quill_core::doc::MAX_PICTURE_BYTES
			|| self.doc.pic_data.append(self.s, &[(hi << 4) | v]).is_err()
		{
			self.pic.failed = true;
			return;
		}
		self.pic.len += 1;
	}

	/// The end of a `\pict` group: checks the bytes and records the picture, or drops them.
	fn finish_pict(&mut self, story: u8) -> Result<()> {
		let p = self.pic;
		self.pic = PictState::default();
		let ok = p.len > 0 && !p.failed && (p.kind == 1 || p.kind == 2);
		let mut dims = None;
		if ok {
			let mut h = [0u8; 54];
			self.doc.pic_data.read(self.s, p.start, &mut h);
			let at = |i: usize| -> [u8; 4] {
				let mut x = [0u8; 4];
				if let Some(src) = h.get(i..i + 4) {
					x.copy_from_slice(src);
				}
				x
			};
			// Not `u32::from_be_bytes`: it becomes `bswap`, which a 386 does not have.
			let be = |i: usize| {
				at(i)
					.iter()
					.fold(0u32, |a, &x| (a << 8) | u32::from(core::hint::black_box(x)))
			};
			let le = |i: usize| u32::from_le_bytes(at(i));
			if p.kind == 1 {
				if h.get(..8) == Some(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A][..])
					&& p.len >= 24
				{
					dims = Some((be(16), be(20)));
				}
			} else if p.len >= 54 && le(14) >= 40 {
				// The DIB's header, then its palette, then the pixels: fill in the file header.
				let bpp = le(26) >> 16;
				let used = le(46);
				let colours = if bpp <= 8 && used == 0 {
					1u32 << bpp
				} else {
					used
				};
				let data_at = 14 + le(14) + colours.min(256) * 4;
				let mut fh = [0u8; 14];
				let (len_b, at_b) = (p.len.to_le_bytes(), data_at.to_le_bytes());
				for (i, v) in [
					b'B', b'M', len_b[0], len_b[1], len_b[2], len_b[3], 0, 0, 0, 0, at_b[0],
					at_b[1], at_b[2], at_b[3],
				]
				.into_iter()
				.enumerate()
				{
					if let Some(d) = fh.get_mut(i) {
						*d = v;
					}
				}
				self.doc.pic_data.patch(self.s, p.start, &fh);
				dims = Some((le(18), (le(22) as i32).unsigned_abs()));
			}
		}
		let valid = dims.filter(|&(w, h)| w > 0 && h > 0 && w <= 8192 && h <= 8192);
		let (Some((w, h)), true) = (
			valid,
			self.doc.pictures.len() < quill_core::doc::MAX_PICTURES,
		) else {
			self.doc.pic_data.truncate(self.s, p.start);
			return Ok(());
		};
		let text_w =
			(self.doc.page.paper_w - self.doc.page.margin_l - self.doc.page.margin_r).max(1440);
		let (mut sw, mut sh) = if p.goalw > 0 && p.goalh > 0 {
			(p.goalw, p.goalh)
		} else {
			(w as i32 * 15, h as i32 * 15)
		};
		if sw > text_w {
			sh = (i64::from(sh) * i64::from(text_w) / i64::from(sw)) as i32;
			sw = text_w;
		}
		self.doc.pictures.reserve(self.s, 1)?;
		let idx = self.doc.pictures.len();
		self.doc.pictures.push(
			self.s,
			quill_core::doc::Picture {
				kind: if p.kind == 1 { 1 } else { 0 },
				w: w as u16,
				h: h as u16,
				shown_w: sw.max(144),
				shown_h: sh.max(144),
				offset: p.start,
				len: p.len,
			},
		)?;
		self.shp_ok = true;
		self.target = story;
		self.flush()?;
		let cf = self.cfmt()?;
		let k = StoryKind::from_u8(story).unwrap_or(StoryKind::Body);
		self.doc
			.story_mut(k)
			.append_object(self.s, idx & 0xFFFF, cf, 0)?;
		Ok(())
	}
	fn cfmt(&mut self) -> Result<u16> {
		let f = *self
			.font_map
			.get(usize::from(self.chars.font))
			.unwrap_or(&0);
		let c = *self
			.colour_map
			.get(usize::from(self.chars.colour))
			.unwrap_or(&0);
		let cf = CFmt {
			font: f,
			size: self.chars.size,
			colour: c,
			vpos: self.chars.vpos,
			flags: self.chars.flags,
		};
		Ok(self.doc.cfmts.intern(self.s, cf)?)
	}

	fn flush(&mut self) -> Result<()> {
		self.flush_hi()?;
		if self.pending_len > 0 {
			let n = self.pending_len;
			self.pending_len = 0;
			let cf = self.pending_cf;
			let kind = StoryKind::from_u8(self.pending_story).unwrap_or(StoryKind::Body);
			self.doc.story_mut(kind).append_original(
				self.s,
				self.pending.get(..n).unwrap_or(&[]),
				cf,
				0,
			)?;
		}
		Ok(())
	}

	/// A high surrogate with no low half after it is a character that cannot be read.
	fn flush_hi(&mut self) -> Result<()> {
		if self.hi != 0 {
			self.hi = 0;
			self.character('\u{FFFD}')?;
		}
		Ok(())
	}

	/// One character of text, as the document holds it today: a Windows-1252 byte, or `?` (counted) when there is none.
	/// This is the one step CORE-63 replaces when the document holds the character itself.
	fn character(&mut self, c: char) -> Result<()> {
		match narrow(self.s, c) {
			Some(b) => self.text(b),
			None => {
				self.refused = self.refused.saturating_add(1);
				self.text(b'?')
			}
		}
	}

	/// A byte of the file's text (`\'hh`, or a byte of 8 bits written as it is), read in the font's code page.
	fn byte(&mut self, b: u8) -> Result<()> {
		let enc = match self.font_enc.get(usize::from(self.chars.font)) {
			Some(&e) if e != Enc::Unset => e,
			_ => self.doc_enc,
		};
		if enc == Enc::Double && self.trail {
			self.trail = false;
			return Ok(());
		}
		if b < 0x80 {
			return self.text(b);
		}
		match enc {
			Enc::Single(cp) => {
				let c = self.s.to_unicode(cp, b);
				self.character(c)
			}
			_ => {
				self.refused = self.refused.saturating_add(1);
				self.text(b'?')?;
				self.trail = true;
				Ok(())
			}
		}
	}

	/// `\uN`: the character with that signed 16-bit value; a surrogate pair is one character from two escapes.
	fn unicode(&mut self, v: i32) -> Result<()> {
		let u = if (-0x8000..0x1_0000).contains(&v) {
			(v & 0xFFFF) as u32
		} else {
			0xFFFD
		};
		match u {
			0xD800..=0xDBFF => {
				self.flush_hi()?;
				self.hi = u as u16;
				Ok(())
			}
			0xDC00..=0xDFFF if self.hi != 0 => {
				let high = u32::from(self.hi) - 0xD800;
				self.hi = 0;
				let c = char::from_u32(0x1_0000 + (high << 10) + (u - 0xDC00));
				self.character(c.unwrap_or('\u{FFFD}'))
			}
			_ => {
				self.flush_hi()?;
				self.character(char::from_u32(u).unwrap_or('\u{FFFD}'))
			}
		}
	}

	fn text(&mut self, b: u8) -> Result<()> {
		self.trail = false;
		self.flush_hi()?;
		self.text_total += 1;
		if self.text_total > MAX_TEXT {
			return Err(FormatError::TooLarge);
		}
		let cf = self.cfmt()?;
		if self.pending_len > 0
			&& (cf != self.pending_cf
				|| self.pending_len == self.pending.len()
				|| self.pending_story != self.target)
		{
			self.flush()?;
		}
		self.pending_cf = cf;
		self.pending_story = self.target;
		if let Some(slot) = self.pending.get_mut(self.pending_len) {
			*slot = b;
			self.pending_len += 1;
		}
		if self.target == 0 {
			self.last_mark = b == 0x0D;
		}
		Ok(())
	}

	fn par(&mut self) -> Result<()> {
		self.flush()?;
		let p = self.para;
		let tabset = if p.tabs.count > 0 {
			self.doc.tabsets.intern(self.s, p.tabs)?
		} else {
			NO_TABS
		};
		let mut flags = p.flags;
		if self.pagebb_next {
			flags |= 2;
			self.pagebb_next = false;
		}
		let mut list = 0;
		if p.list != 0 {
			flags |= if p.list == 1 { 1 } else { 4 };
			let mut def = quill_core::doc::ListDef::default();
			for (i, lv) in def.levels.iter_mut().enumerate() {
				*lv = quill_core::doc::ListLevel {
					style: p.style,
					start: p.start,
					indent: 360 * (i as i32 + 1),
				};
			}
			list = self.doc.lists.intern(self.s, def)?;
		}
		let pf = PFmt {
			align: p.align,
			spacing: p.spacing,
			flags,
			list,
			level: p.level.min(8),
			tabset,
			left: p.left,
			right: p.right,
			first: p.first,
			before: p.before,
			after: p.after,
		};
		let pf = self.doc.pfmts.intern(self.s, pf)?;
		self.text_total += 1;
		if self.text_total > MAX_TEXT {
			return Err(FormatError::TooLarge);
		}
		let cf = self.cfmt()?;
		let kind = StoryKind::from_u8(self.target).unwrap_or(StoryKind::Body);
		self.doc
			.story_mut(kind)
			.append_original(self.s, b"\r", cf, pf)?;
		if self.target == 0 {
			self.last_mark = true;
		}
		Ok(())
	}
}

fn ansi_for(word: &str) -> Option<u8> {
	Some(match word {
		"emdash" => 0x97,
		"endash" => 0x96,
		"lquote" => 0x91,
		"rquote" => 0x92,
		"ldblquote" => 0x93,
		"rdblquote" => 0x94,
		"bullet" => 0x95,
		_ => return None,
	})
}

/// The Windows-1252 byte the document holds for `c`, when there is one. Control characters have none.
///
/// The 27 characters 1252 has in 0x80 to 0x9F are asked of the store (so Win16 serves them from its resource and not its data
/// segment), after a test that keeps the rest of Unicode from asking.
fn narrow<S: ReadStore + ?Sized>(s: &S, c: char) -> Option<u8> {
	let u = u32::from(c);
	match u {
		0x20..=0x7E | 0xA0..=0xFF => u8::try_from(u).ok(),
		0x80..=0x9F | 0x152..=0x192 | 0x2C6 | 0x2DC | 0x2013..=0x2122 => {
			(0x80..=0x9Fu8).find(|&b| s.to_unicode(CodePage::Cp1252, b) == c)
		}
		_ => None,
	}
}

/// Reads RTF into `doc` (replacing it). Unknown destinations and control words are skipped.
///
/// `\'hh` and 8-bit text are read in the code page of the font (`\fcharset`) or else of the document (`\ansicpg`), then kept as
/// Windows-1252. What 1252 lacks is `?`; use [`read_counted`] to learn how many.
pub fn read<S: Store + ?Sized, R: Source>(doc: &mut Doc, s: &mut S, r: &mut R) -> Result<()> {
	read_counted(doc, s, r).map(|_| ())
}

/// [`read`], and how many characters the document could not hold and replaced by `?` (CORE-61): those of a code page or `\uN`
/// that Windows-1252 has no form for, and each character that could not be read at all (a lone surrogate, a double-byte pair).
pub fn read_counted<S: Store + ?Sized, R: Source>(
	doc: &mut Doc,
	s: &mut S,
	r: &mut R,
) -> Result<Refused> {
	// The tables come from the file; only Automatic, colour 0, is ours.
	doc.reset(s);
	doc.colours.intern(s, Colour { r: 0, g: 0, b: 0 })?;
	let defaults = CharState {
		font: 0,
		size: 24,
		colour: 0,
		vpos: 0,
		flags: 0,
	};
	let mut st = State {
		doc,
		s,
		chars: defaults,
		para: PARA0,
		font_map: [0; MAX_FONTS as usize],
		font_count: 0,
		colour_map: [0; MAX_COLOURS as usize],
		colour_count: 0,
		deff: 0,
		font_cur: None,
		doc_enc: Enc::Single(CodePage::Cp1252),
		font_enc: [Enc::Unset; MAX_FONTS as usize],
		hi: 0,
		trail: false,
		refused: 0,
		colour_cur: (0, 0, 0, false),
		pending: [0; 128],
		pending_len: 0,
		pending_cf: 0,
		pagebb_next: false,
		text_total: 0,
		last_mark: true,
		target: 0,
		pending_story: 0,
		fld: [0; 12],
		fld_len: 0,
		pic: PictState::default(),
		shp_ok: false,
	};
	let mut rd = Reader {
		r,
		peeked: None,
		buf: [0; 256],
		at: 0,
		len: 0,
	};
	let mut stack = [Group {
		chars: defaults,
		dest: Dest::Body,
		uc_skip: 1,
		story: 0,
	}; MAX_DEPTH];
	let mut depth = 0usize;
	let mut cur = Group {
		chars: defaults,
		dest: Dest::Body,
		uc_skip: 1,
		story: 0,
	};
	let mut skip_chars = 0u32;
	let mut seen_rtf = false;
	let mut deeper = 0u32; // groups past the nesting limit, skipped

	while let Some(b) = rd.next() {
		match b {
			b'{' => {
				if depth + 1 >= MAX_DEPTH {
					deeper += 1;
					continue;
				}
				if let Some(slot) = stack.get_mut(depth) {
					*slot = cur;
				}
				depth += 1;
			}
			b'}' => {
				if deeper > 0 {
					deeper -= 1;
					continue;
				}
				if depth == 0 {
					break;
				}
				if cur.dest == Dest::FontTable || cur.dest == Dest::ColourTable {
					// A table entry ends with its `;`, handled below; the group's end closes the table.
				}
				depth -= 1;
				let saved = stack.get(depth).copied().unwrap_or(cur);
				if cur.dest == Dest::Pict && saved.dest != Dest::Pict {
					st.finish_pict(cur.story)?;
				}
				if cur.dest == Dest::Field && st.fld_len > 0 {
					let name = st.fld.get(..st.fld_len).unwrap_or(&[]);
					let kind: u32 = if name.starts_with(b"PAGE") {
						1
					} else if name.starts_with(b"NUMPAGES") {
						2
					} else if name.starts_with(b"DATE") {
						3
					} else if name.starts_with(b"FILENAME") {
						4
					} else {
						0
					};
					if kind != 0 {
						st.target = cur.story;
						st.flush()?;
						let cf = st.cfmt()?;
						let k = StoryKind::from_u8(cur.story).unwrap_or(StoryKind::Body);
						st.doc.story_mut(k).append_object(st.s, kind << 16, cf, 0)?;
					}
				}
				if cur.story != saved.story {
					st.flush()?;
				}
				// Entries inside the font table are groups of their own: finish the font when its group ends.
				if cur.dest == Dest::FontTable && saved.dest == Dest::FontTable {
					finish_font(&mut st)?;
				}
				cur = saved;
				st.chars = cur.chars;
				st.target = cur.story;
				if depth == 0 {
					break;
				}
			}
			b'\\' => {
				let Some(c) = rd.next() else {
					break;
				};
				match c {
					b'\\' | b'{' | b'}' => {
						if cur.dest == Dest::Body {
							if skip_chars > 0 {
								skip_chars -= 1;
							} else if depth > 0 {
								st.chars = cur.chars;
								st.target = cur.story;
								st.text(c)?;
							}
						}
					}
					b'\'' => {
						// Two hexadecimal digits; a damaged escape ends where the digits do and costs nothing else.
						let Some(h) = hex_digit(&mut rd) else {
							continue;
						};
						let Some(l) = hex_digit(&mut rd) else {
							continue;
						};
						let v = (h << 4) | l;
						if cur.dest == Dest::Body {
							if skip_chars > 0 {
								skip_chars -= 1;
							} else {
								st.chars = cur.chars;
								st.target = cur.story;
								st.byte(v)?;
							}
						} else if cur.dest == Dest::FontTable {
							font_char(&mut st, v);
						}
					}
					b'*' => {
						if cur.dest == Dest::Body {
							cur.dest = Dest::Skip;
						}
					}
					b'~' => {
						if cur.dest == Dest::Body {
							st.chars = cur.chars;
							st.target = cur.story;
							st.text(0xA0)?;
						}
					}
					b'_' => {
						if cur.dest == Dest::Body {
							st.chars = cur.chars;
							st.target = cur.story;
							st.text(b'-')?;
						}
					}
					b'-' | b'|' | b':' => {}
					b'\r' | b'\n' => {
						if cur.dest == Dest::Body {
							st.chars = cur.chars;
							st.target = cur.story;
							st.text(0x0D)?;
							st.flush()?;
						}
					}
					c if c.is_ascii_alphabetic() => {
						let mut word = [0u8; MAX_WORD];
						let mut n = 0;
						word[0] = c;
						n += 1;
						let mut next = rd.next();
						while let Some(ch) = next {
							if !ch.is_ascii_alphabetic() {
								break;
							}
							if n >= MAX_WORD {
								return Err(FormatError::BadFile);
							}
							if let Some(w) = word.get_mut(n) {
								*w = ch;
							}
							n += 1;
							next = rd.next();
						}
						let mut neg = false;
						let mut num: Option<i64> = None;
						if next == Some(b'-') {
							neg = true;
							next = rd.next();
						}
						while let Some(ch) = next {
							if !ch.is_ascii_digit() {
								break;
							}
							let v = num.unwrap_or(0) * 10 + i64::from(ch - b'0');
							if v > i64::from(i32::MAX) {
								return Err(FormatError::BadFile);
							}
							num = Some(v);
							next = rd.next();
						}
						match next {
							Some(b' ') | None => {}
							Some(other) => rd.unread(other),
						}
						let w = core::str::from_utf8(word.get(..n).unwrap_or(&[])).unwrap_or("");
						let val = num.map(|v| if neg { -v } else { v });
						if w == "rtf" {
							seen_rtf = true;
						}
						if !seen_rtf {
							return Err(FormatError::BadFile);
						}
						control(&mut st, &mut cur, &mut skip_chars, w, val, depth)?;
					}
					_ => {}
				}
			}
			b'\r' | b'\n' => {}
			_ => {
				if !seen_rtf {
					return Err(FormatError::BadFile);
				}
				match cur.dest {
					Dest::Body => {
						if depth == 0 {
							continue;
						}
						if skip_chars > 0 {
							skip_chars -= 1;
							continue;
						}
						st.chars = cur.chars;
						st.target = cur.story;
						st.byte(if b < 0x20 && b != 0x09 { b' ' } else { b })?;
					}
					Dest::FontTable => font_char(&mut st, b),
					Dest::ColourTable => {
						if b == b';' {
							finish_colour(&mut st)?;
						}
					}
					Dest::FldInst => {
						if b.is_ascii_alphabetic() && st.fld_len < st.fld.len() {
							if let Some(slot) = st.fld.get_mut(st.fld_len) {
								*slot = b.to_ascii_uppercase();
								st.fld_len += 1;
							}
						}
					}
					Dest::Pict => st.pict_hex(b),
					Dest::Skip | Dest::Pn | Dest::Field => {}
				}
			}
		}
	}
	if !seen_rtf {
		return Err(FormatError::BadFile);
	}
	st.flush()?;
	let refused = Refused { count: st.refused };
	if st.doc.fonts.is_empty() {
		st.doc
			.fonts
			.intern(st.s, Font::new(b"Times New Roman", 1))?;
	}
	if !st.last_mark || st.doc.body.is_empty() {
		st.chars = defaults;
		st.par()?;
	}
	for k in [
		StoryKind::Header,
		StoryKind::Footer,
		StoryKind::FirstHeader,
		StoryKind::FirstFooter,
	] {
		let n = st.doc.story(k).len();
		if n > 0 && st.doc.story(k).read_byte(st.s, n - 1) != Some(0x0D) {
			st.target = k as u8;
			st.par()?;
		}
	}
	for k in StoryKind::ALL {
		st.doc.story_mut(k).forget_history(st.s);
	}
	Ok(refused)
}

/// The next byte as a hexadecimal digit. Anything else is not consumed, so a damaged `\'` cannot eat a brace.
fn hex_digit<R: Source>(rd: &mut Reader<'_, R>) -> Option<u8> {
	let b = rd.next()?;
	let v = match b {
		b'0'..=b'9' => b - b'0',
		b'a'..=b'f' => b - b'a' + 10,
		b'A'..=b'F' => b - b'A' + 10,
		_ => {
			rd.unread(b);
			return None;
		}
	};
	Some(v)
}

fn font_char<S: Store + ?Sized>(st: &mut State<'_, S>, b: u8) {
	if b == b';' {
		return;
	}
	if let Some((_, _, _, name)) = st.font_cur.as_mut() {
		let len = name.iter().position(|c| *c == 0).unwrap_or(31);
		if len < 31 && b >= 0x20 {
			if let Some(slot) = name.get_mut(len) {
				*slot = b;
			}
		}
	}
}

fn finish_font<S: Store + ?Sized>(st: &mut State<'_, S>) -> Result<()> {
	if let Some((num, family, _, name)) = st.font_cur.take() {
		let len = name.iter().position(|c| *c == 0).unwrap_or(31);
		let f = Font {
			family,
			pitch: 0,
			charset: 0,
			name_len: len as u8,
			name,
		};
		let idx = st.doc.fonts.intern(st.s, f)?;
		if let Some(slot) = st.font_map.get_mut(num) {
			*slot = idx;
		}
		st.font_count += 1;
	}
	Ok(())
}

fn finish_colour<S: Store + ?Sized>(st: &mut State<'_, S>) -> Result<()> {
	let (r, g, b, set) = st.colour_cur;
	st.colour_cur = (0, 0, 0, false);
	let n = st.colour_count;
	st.colour_count += 1;
	if n == 0 && !set {
		// The first empty entry is Automatic.
		return Ok(());
	}
	if st.colour_count > MAX_COLOURS as usize {
		return Err(FormatError::BadFile);
	}
	let idx = st.doc.colours.intern(st.s, Colour { r, g, b })?;
	if let Some(slot) = st.colour_map.get_mut(n) {
		*slot = idx as u8;
	}
	Ok(())
}

fn control<S: Store + ?Sized>(
	st: &mut State<'_, S>,
	cur: &mut Group,
	skip: &mut u32,
	w: &str,
	val: Option<i64>,
	depth: usize,
) -> Result<()> {
	let v = val.unwrap_or(0) as i32;
	// Destinations.
	match w {
		"pn" => {
			cur.dest = Dest::Pn;
			return Ok(());
		}
		"fonttbl" => {
			cur.dest = Dest::FontTable;
			return Ok(());
		}
		"colortbl" => {
			cur.dest = Dest::ColourTable;
			st.colour_count = 0;
			return Ok(());
		}
		"header" | "headerr" | "footer" | "footerr" | "headerf" | "footerf" => {
			st.flush()?;
			cur.story = match w {
				"header" | "headerr" => 1,
				"footer" | "footerr" => 2,
				"headerf" => 3,
				_ => 4,
			};
			return Ok(());
		}
		_ if cur.dest == Dest::Pict => {
			match w {
				"picw" => st.pic.picw = v,
				"pich" => st.pic.pich = v,
				"picwgoal" => st.pic.goalw = v,
				"pichgoal" => st.pic.goalh = v,
				"pngblip" => st.pic.kind = 1,
				"dibitmap" => {
					st.pic.kind = 2;
					// A DIB has no file header; leave room for one and fill it in at the end.
					st.pic.start = st.doc.pic_data.len();
					if st.doc.pic_data.append(st.s, &[0u8; 14]).is_err() {
						st.pic.failed = true;
					}
					st.pic.len = 14;
				}
				"jpegblip" | "emfblip" | "wmetafile" | "macpict" | "pmmetafile" | "wbitmap" => {
					st.pic.kind = 3
				}
				_ => {}
			}
			return Ok(());
		}
		"pict" => {
			if cur.dest == Dest::Body {
				cur.dest = Dest::Pict;
				st.pic = PictState {
					start: st.doc.pic_data.len(),
					..PictState::default()
				};
			} else if cur.dest != Dest::FontTable && cur.dest != Dest::ColourTable {
				cur.dest = Dest::Skip;
			}
			return Ok(());
		}
		"shppict" => {
			st.shp_ok = false;
			if cur.dest == Dest::Skip {
				cur.dest = Dest::Body;
			}
			return Ok(());
		}
		"nonshppict" => {
			// The copy after a picture that was read is not needed.
			if st.shp_ok {
				cur.dest = Dest::Skip;
			} else if cur.dest == Dest::Skip {
				cur.dest = Dest::Body;
			}
			return Ok(());
		}
		"field" => {
			cur.dest = Dest::Field;
			st.fld_len = 0;
			return Ok(());
		}
		"fldinst" if cur.dest == Dest::Field => {
			cur.dest = Dest::FldInst;
			return Ok(());
		}
		"stylesheet" | "info" | "headerl" | "footerl" | "object" | "footnote" | "annotation"
		| "listtable" | "listoverridetable" | "pntext" | "listtext" | "revtbl" | "rsidtbl"
		| "generator" | "xmlnstbl" | "themedata" | "colorschememapping" | "latentstyles"
		| "datastore" | "fldinst" | "fldrslt" => {
			if cur.dest != Dest::FontTable && cur.dest != Dest::ColourTable {
				cur.dest = Dest::Skip;
			}
			return Ok(());
		}
		_ => {}
	}
	match cur.dest {
		Dest::FontTable => {
			match w {
				"f" => {
					let num = v.max(0) as usize % MAX_FONTS as usize;
					st.font_cur = Some((num, 0, 0, [0; 31]));
					if let Some(e) = st.font_enc.get_mut(num) {
						*e = Enc::Unset;
					}
				}
				"fcharset" => {
					if let Some((num, _, _, _)) = st.font_cur {
						if let Some(e) = st.font_enc.get_mut(num) {
							*e = Enc::from_charset(i64::from(v));
						}
					}
				}
				"froman" | "fswiss" | "fmodern" | "fscript" | "fdecor" | "fnil" | "ftech"
				| "fbidi" => {
					if let Some((_, fam, _, _)) = st.font_cur.as_mut() {
						*fam = match w {
							"froman" => 1,
							"fswiss" => 2,
							"fmodern" => 3,
							"fscript" => 4,
							"fdecor" => 5,
							_ => 0,
						};
					}
				}
				_ => {}
			}
			return Ok(());
		}
		Dest::ColourTable => {
			match w {
				"red" => st.colour_cur = (v as u8, st.colour_cur.1, st.colour_cur.2, true),
				"green" => st.colour_cur = (st.colour_cur.0, v as u8, st.colour_cur.2, true),
				"blue" => st.colour_cur = (st.colour_cur.0, st.colour_cur.1, v as u8, true),
				_ => {}
			}
			return Ok(());
		}
		Dest::Pn => {
			match w {
				"pnlvlblt" => st.para.list = 1,
				"pnlvlbody" => st.para.list = 2,
				"pnlvl" => st.para.level = v.clamp(1, 9) as u8 - 1,
				"pndec" => st.para.style = 0,
				"pnlcltr" => st.para.style = 2,
				"pnlcrm" => st.para.style = 4,
				"pnucltr" => st.para.style = 5,
				"pnucrm" => st.para.style = 6,
				"pnstart" => st.para.start = v.clamp(0, 65535) as u16,
				"pntxta" | "pntxtb" => cur.dest = Dest::Skip,
				_ => {}
			}
			return Ok(());
		}
		Dest::Skip | Dest::Pict | Dest::Field | Dest::FldInst => return Ok(()),
		Dest::Body => {}
	}
	if depth == 0 {
		// Document-level properties before the first text.
		match w {
			"deff" => st.deff = v.max(0) as u16,
			"paperw" => st.doc.page.paper_w = v.clamp(1, PAGE_LIMIT),
			"paperh" => st.doc.page.paper_h = v.clamp(1, PAGE_LIMIT),
			"margl" => st.doc.page.margin_l = v.clamp(0, PAGE_LIMIT),
			"margr" => st.doc.page.margin_r = v.clamp(0, PAGE_LIMIT),
			"margt" => st.doc.page.margin_t = v.clamp(0, PAGE_LIMIT),
			"margb" => st.doc.page.margin_b = v.clamp(0, PAGE_LIMIT),
			"headery" => st.doc.page.header_dist = v.clamp(0, PAGE_LIMIT),
			"footery" => st.doc.page.footer_dist = v.clamp(0, PAGE_LIMIT),
			"titlepg" => st.doc.page.flags |= 1,
			"pgnstart" => st.doc.page.first_page = v.clamp(1, 65535) as u16,
			_ => {}
		}
	}
	match w {
		"deff" => st.deff = v.max(0) as u16,
		"ansicpg" => {
			if let Some(e) = Enc::from_ansicpg(i64::from(v)) {
				st.doc_enc = e;
			}
		}
		"paperw" => st.doc.page.paper_w = v.clamp(1, PAGE_LIMIT),
		"paperh" => st.doc.page.paper_h = v.clamp(1, PAGE_LIMIT),
		"margl" => st.doc.page.margin_l = v.clamp(0, PAGE_LIMIT),
		"margr" => st.doc.page.margin_r = v.clamp(0, PAGE_LIMIT),
		"margt" => st.doc.page.margin_t = v.clamp(0, PAGE_LIMIT),
		"margb" => st.doc.page.margin_b = v.clamp(0, PAGE_LIMIT),
		"headery" => st.doc.page.header_dist = v.clamp(0, PAGE_LIMIT),
		"footery" => st.doc.page.footer_dist = v.clamp(0, PAGE_LIMIT),
		"titlepg" => st.doc.page.flags |= 1,
		"pgnstart" => st.doc.page.first_page = v.clamp(1, 65535) as u16,
		"plain" => {
			cur.chars = CharState {
				font: st.deff,
				size: 24,
				colour: 0,
				vpos: 0,
				flags: 0,
			};
		}
		"f" => cur.chars.font = v.clamp(0, MAX_FONTS as i32 - 1) as u16,
		"fs" => cur.chars.size = v.clamp(2, 288) as u16,
		"b" => set_flag(&mut cur.chars.flags, cflag::BOLD, val != Some(0)),
		"i" => set_flag(&mut cur.chars.flags, cflag::ITALIC, val != Some(0)),
		"ul" | "uld" | "uldb" | "ulw" | "ulth" | "uldash" | "ulwave" => {
			set_flag(&mut cur.chars.flags, cflag::UNDERLINE, val != Some(0))
		}
		"ulnone" => set_flag(&mut cur.chars.flags, cflag::UNDERLINE, false),
		"strike" | "striked" => set_flag(&mut cur.chars.flags, cflag::STRIKE, val != Some(0)),
		"super" | "up" => cur.chars.vpos = if val == Some(0) { 0 } else { 1 },
		"sub" | "dn" => cur.chars.vpos = if val == Some(0) { 0 } else { 2 },
		"nosupersub" => cur.chars.vpos = 0,
		"cf" => cur.chars.colour = v.clamp(0, 255) as u8,
		"pard" => st.para = PARA0,
		"ilvl" => st.para.level = v.clamp(0, 8) as u8,
		"ql" => st.para.align = 0,
		"qc" => st.para.align = 1,
		"qr" => st.para.align = 2,
		"qj" => st.para.align = 3,
		"li" => st.para.left = v,
		"ri" => st.para.right = v,
		"fi" => st.para.first = v,
		"sb" => st.para.before = v.max(0),
		"sa" => st.para.after = v.max(0),
		"sl" => {
			st.para.spacing = match v {
				0 | 240 => 0,
				x if x > 0 && x < 420 => 1,
				x if x >= 420 => 2,
				_ => 0,
			}
		}
		"pagebb" => st.para.flags |= 2,
		"tqc" => st.para.next_kind = 1,
		"tqr" => st.para.next_kind = 2,
		"tqdec" => st.para.next_kind = 3,
		"tldot" | "tlth" => st.para.next_leader = if w == "tldot" { 1 } else { 3 },
		"tlhyph" => st.para.next_leader = 2,
		"tlul" => st.para.next_leader = 3,
		"tx" => {
			let n = usize::from(st.para.tabs.count);
			if n < MAX_STOPS {
				if let Some(slot) = st.para.tabs.stops.get_mut(n) {
					*slot = TabStop {
						pos: v,
						kind: st.para.next_kind,
						leader: st.para.next_leader,
					};
					st.para.tabs.count += 1;
				}
			}
			st.para.next_kind = 0;
			st.para.next_leader = 0;
		}
		"par" | "sect" => st.par()?,
		"page" => {
			st.par()?;
			st.pagebb_next = true;
		}
		"line" => {
			st.chars = cur.chars;
			st.target = cur.story;
			st.text(0x0B)?;
		}
		"tab" => {
			st.chars = cur.chars;
			st.target = cur.story;
			st.text(0x09)?;
		}
		"u" => {
			st.chars = cur.chars;
			st.target = cur.story;
			st.unicode(v)?;
			*skip = cur.uc_skip;
		}
		"uc" => cur.uc_skip = v.max(0) as u32,
		_ => {
			if let Some(a) = ansi_for(w) {
				st.chars = cur.chars;
				st.target = cur.story;
				st.text(a)?;
			}
		}
	}
	st.chars = cur.chars;
	st.target = cur.story;
	Ok(())
}

fn set_flag(flags: &mut u16, bit: u16, on: bool) {
	if on {
		*flags |= bit;
	} else {
		*flags &= !bit;
	}
}
