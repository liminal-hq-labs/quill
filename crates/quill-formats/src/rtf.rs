// The RTF subset: a canonical writer and a forgiving reader for characters, paragraphs, tabs and the page setup.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Lists, headers and footers, fields and pictures are not read or written yet (the SPEC lists them for later milestones); a
//! reader skips what it does not know, as the SPEC requires, so such files open with their text and formats.

use quill_core::doc::{
	cflag, CFmt, Colour, Doc, Font, PFmt, StoryKind, TabSet, TabStop, MAX_COLOURS, MAX_FONTS,
	MAX_STOPS, NO_TABS,
};
use quill_core::store::Store;
use quill_core::text::MAX_TEXT;

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
pub fn write<S: Store + ?Sized, K: Sink>(doc: &Doc, s: &S, k: &mut K) -> Result<()> {
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

	let t = &doc.body;
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
				let mark = para_end(doc, s, pos);
				let pf = mark
					.and_then(|m| t.pfmt_at(s, m))
					.and_then(|x| doc.pfmts.get(s, x))
					.unwrap_or(PFmt::DEFAULT);
				para_props(&mut o, doc, s, &pf);
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
				char_props(&mut o, doc, s, cf);
				cur_cfmt = Some(cf);
			}
			match b {
				b'\\' | b'{' | b'}' => {
					o.put(&[b'\\', b]);
				}
				0x09 => o.s("\\tab "),
				0x0B => o.s("\\line "),
				0x01 => {}
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
	o.s("}");
	o.nl();
	if o.ok {
		Ok(())
	} else {
		Err(FormatError::Write)
	}
}

fn hex(n: u8) -> u8 {
	if n < 10 {
		b'0' + n
	} else {
		b'a' + n - 10
	}
}

fn para_end<S: Store + ?Sized>(doc: &Doc, s: &S, from: u32) -> Option<u32> {
	let t = &doc.body;
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

fn para_props<S: Store + ?Sized, K: Sink>(o: &mut Out<'_, K>, doc: &Doc, s: &S, pf: &PFmt) {
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
	o.s(" ");
}

fn char_props<S: Store + ?Sized, K: Sink>(o: &mut Out<'_, K>, doc: &Doc, s: &S, cf: u16) {
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
};

#[derive(Copy, Clone, PartialEq, Eq)]
enum Dest {
	Body,
	FontTable,
	ColourTable,
	Skip,
}

#[derive(Copy, Clone)]
struct Group {
	chars: CharState,
	dest: Dest,
	uc_skip: u32,
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
	colour_cur: (u8, u8, u8, bool),
	pending: [u8; 128],
	pending_len: usize,
	pending_cf: u16,
	pagebb_next: bool,
	text_total: u32,
	last_mark: bool,
}

impl<S: Store + ?Sized> State<'_, S> {
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
		if self.pending_len > 0 {
			let n = self.pending_len;
			self.pending_len = 0;
			let cf = self.pending_cf;
			self.doc
				.body
				.append_original(self.s, self.pending.get(..n).unwrap_or(&[]), cf, 0)?;
		}
		Ok(())
	}

	fn text(&mut self, b: u8) -> Result<()> {
		self.text_total += 1;
		if self.text_total > MAX_TEXT {
			return Err(FormatError::TooLarge);
		}
		let cf = self.cfmt()?;
		if self.pending_len > 0 && (cf != self.pending_cf || self.pending_len == self.pending.len())
		{
			self.flush()?;
		}
		self.pending_cf = cf;
		if let Some(slot) = self.pending.get_mut(self.pending_len) {
			*slot = b;
			self.pending_len += 1;
		}
		self.last_mark = b == 0x0D;
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
		let pf = PFmt {
			align: p.align,
			spacing: p.spacing,
			flags,
			list: 0,
			level: 0,
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
		self.doc.body.append_original(self.s, b"\r", cf, pf)?;
		self.last_mark = true;
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

fn unicode_to_ansi(u: i32) -> u8 {
	let c = if u < 0 { u + 65536 } else { u };
	match c {
		0x20..=0x7E | 0xA0..=0xFF => c as u8,
		0x2013 => 0x96,
		0x2014 => 0x97,
		0x2018 => 0x91,
		0x2019 => 0x92,
		0x201C => 0x93,
		0x201D => 0x94,
		0x2022 => 0x95,
		0x2026 => 0x85,
		0x20AC => 0x80,
		_ => b'?',
	}
}

/// Reads RTF into `doc` (replacing it). Unknown destinations and control words are skipped.
pub fn read<S: Store + ?Sized, R: Source>(doc: &mut Doc, s: &mut S, r: &mut R) -> Result<()> {
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
		colour_cur: (0, 0, 0, false),
		pending: [0; 128],
		pending_len: 0,
		pending_cf: 0,
		pagebb_next: false,
		text_total: 0,
		last_mark: true,
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
	}; MAX_DEPTH];
	let mut depth = 0usize;
	let mut cur = Group {
		chars: defaults,
		dest: Dest::Body,
		uc_skip: 1,
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
				// Entries inside the font table are groups of their own: finish the font when its group ends.
				if cur.dest == Dest::FontTable && saved.dest == Dest::FontTable {
					finish_font(&mut st)?;
				}
				cur = saved;
				st.chars = cur.chars;
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
								st.text(c)?;
							}
						}
					}
					b'\'' => {
						let h = rd.next().unwrap_or(b'0');
						let l = rd.next().unwrap_or(b'0');
						let v = (unhex(h) << 4) | unhex(l);
						if cur.dest == Dest::Body {
							if skip_chars > 0 {
								skip_chars -= 1;
							} else {
								st.chars = cur.chars;
								st.text(v)?;
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
							st.text(0xA0)?;
						}
					}
					b'_' => {
						if cur.dest == Dest::Body {
							st.chars = cur.chars;
							st.text(b'-')?;
						}
					}
					b'-' | b'|' | b':' => {}
					b'\r' | b'\n' => {
						if cur.dest == Dest::Body {
							st.chars = cur.chars;
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
						st.text(if b < 0x20 && b != 0x09 { b' ' } else { b })?;
					}
					Dest::FontTable => font_char(&mut st, b),
					Dest::ColourTable => {
						if b == b';' {
							finish_colour(&mut st)?;
						}
					}
					Dest::Skip => {}
				}
			}
		}
	}
	if !seen_rtf {
		return Err(FormatError::BadFile);
	}
	st.flush()?;
	if st.doc.fonts.is_empty() {
		st.doc
			.fonts
			.intern(st.s, Font::new(b"Times New Roman", 1))?;
	}
	if !st.last_mark || st.doc.body.is_empty() {
		st.chars = defaults;
		st.par()?;
	}
	st.doc.body.forget_history(st.s);
	let _ = StoryKind::Body;
	Ok(())
}

fn unhex(b: u8) -> u8 {
	match b {
		b'0'..=b'9' => b - b'0',
		b'a'..=b'f' => b - b'a' + 10,
		b'A'..=b'F' => b - b'A' + 10,
		_ => 0,
	}
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
		"fonttbl" => {
			cur.dest = Dest::FontTable;
			return Ok(());
		}
		"colortbl" => {
			cur.dest = Dest::ColourTable;
			st.colour_count = 0;
			return Ok(());
		}
		"stylesheet" | "info" | "header" | "footer" | "headerl" | "headerr" | "headerf"
		| "footerl" | "footerr" | "footerf" | "pict" | "object" | "footnote" | "annotation"
		| "listtable" | "listoverridetable" | "field" | "pn" | "pntext" | "listtext" | "revtbl"
		| "rsidtbl" | "generator" | "xmlnstbl" | "themedata" | "colorschememapping"
		| "latentstyles" | "datastore" | "fldinst" | "fldrslt" => {
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
					st.font_cur = Some((v.max(0) as usize % MAX_FONTS as usize, 0, 0, [0; 31]));
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
		Dest::Skip => return Ok(()),
		Dest::Body => {}
	}
	if depth == 0 {
		// Document-level properties before the first text.
		match w {
			"deff" => st.deff = v.max(0) as u16,
			"paperw" => st.doc.page.paper_w = v.max(1),
			"paperh" => st.doc.page.paper_h = v.max(1),
			"margl" => st.doc.page.margin_l = v.max(0),
			"margr" => st.doc.page.margin_r = v.max(0),
			"margt" => st.doc.page.margin_t = v.max(0),
			"margb" => st.doc.page.margin_b = v.max(0),
			"headery" => st.doc.page.header_dist = v.max(0),
			"footery" => st.doc.page.footer_dist = v.max(0),
			"titlepg" => st.doc.page.flags |= 1,
			"pgnstart" => st.doc.page.first_page = v.clamp(1, 65535) as u16,
			_ => {}
		}
	}
	match w {
		"deff" => st.deff = v.max(0) as u16,
		"paperw" => st.doc.page.paper_w = v.max(1),
		"paperh" => st.doc.page.paper_h = v.max(1),
		"margl" => st.doc.page.margin_l = v.max(0),
		"margr" => st.doc.page.margin_r = v.max(0),
		"margt" => st.doc.page.margin_t = v.max(0),
		"margb" => st.doc.page.margin_b = v.max(0),
		"headery" => st.doc.page.header_dist = v.max(0),
		"footery" => st.doc.page.footer_dist = v.max(0),
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
			st.text(0x0B)?;
		}
		"tab" => {
			st.chars = cur.chars;
			st.text(0x09)?;
		}
		"u" => {
			st.chars = cur.chars;
			st.text(unicode_to_ansi(v))?;
			*skip = cur.uc_skip;
		}
		"uc" => cur.uc_skip = v.max(0) as u32,
		_ => {
			if let Some(a) = ansi_for(w) {
				st.chars = cur.chars;
				st.text(a)?;
			}
		}
	}
	st.chars = cur.chars;
	Ok(())
}

fn set_flag(flags: &mut u16, bit: u16, on: bool) {
	if on {
		*flags |= bit;
	} else {
		*flags &= !bit;
	}
}
