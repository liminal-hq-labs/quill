// Reading a WinHelp 3.1 file: the internal files, the B+ trees, |SYSTEM, |FONT, |CONTEXT and the topics.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::HelpError;

fn u16at(b: &[u8], at: usize) -> u16 {
	u16::from_le_bytes([b.get(at).copied().unwrap_or(0), b.get(at + 1).copied().unwrap_or(0)])
}

fn u32at(b: &[u8], at: usize) -> u32 {
	u32::from(u16at(b, at)) | u32::from(u16at(b, at + 2)) << 16
}

/// An opened help file.
pub struct Hlp<'a> {
	data: &'a [u8],
}

/// A topic: where it is, its title and what it says.
pub struct Topic {
	/// The topic's position, as `|CONTEXT` and jumps name it.
	pub offset: u32,
	/// The title.
	pub title: String,
	/// The paragraphs: their text with `[J:target]` and `[P:target]` markers around links and `{fN}` font changes.
	pub paragraphs: Vec<String>,
}

/// The `|SYSTEM` records.
pub struct System {
	pub minor: u16,
	pub flags: u16,
	pub title: String,
	pub copyright: String,
	pub contents: Option<u32>,
}

impl<'a> Hlp<'a> {
	/// Opens `data` as a help file.
	pub fn new(data: &'a [u8]) -> Result<Hlp<'a>, HelpError> {
		if data.len() < 16 || u32at(data, 0) != 0x0003_5F3F {
			return Err(HelpError::Bad("no WinHelp signature"));
		}
		if u32at(data, 12) as usize != data.len() {
			return Err(HelpError::Bad("the length in the header is not the file's"));
		}
		Ok(Hlp { data })
	}

	fn sub_at(&self, off: usize) -> Result<&'a [u8], HelpError> {
		let reserved = u32at(self.data, off) as usize;
		let used = u32at(self.data, off + 4) as usize;
		if used + 9 > reserved || off + reserved > self.data.len() {
			return Err(HelpError::Bad("an internal file does not fit"));
		}
		Ok(&self.data[off + 9..off + 9 + used])
	}

	/// The directory: (name, offset of the internal file's header).
	pub fn directory(&self) -> Result<Vec<(String, u32)>, HelpError> {
		let tree = self.sub_at(u32at(self.data, 4) as usize)?;
		let mut out = Vec::new();
		leaves(tree, |p| {
			let n = p.iter().position(|b| *b == 0).unwrap_or(0);
			out.push((String::from_utf8_lossy(&p[..n]).into_owned(), u32at(p, n + 1)));
			n + 5
		})?;
		Ok(out)
	}

	/// The data of the internal file `name` (`|SYSTEM`, `|TOPIC`, ...).
	pub fn file(&self, name: &str) -> Option<&'a [u8]> {
		let off = self.directory().ok()?.into_iter().find(|(n, _)| n == name)?.1;
		self.sub_at(off as usize).ok()
	}

	/// Reads `|SYSTEM`.
	pub fn system(&self) -> Result<System, HelpError> {
		let b = self.file("|SYSTEM").ok_or(HelpError::Bad("no |SYSTEM"))?;
		if u16at(b, 0) != 0x036C || u16at(b, 4) != 1 {
			return Err(HelpError::Bad("|SYSTEM has the wrong signature"));
		}
		let mut s = System {
			minor: u16at(b, 2),
			flags: u16at(b, 10),
			title: String::new(),
			copyright: String::new(),
			contents: None,
		};
		let mut at = 12;
		if s.minor <= 16 {
			s.title = zstr(&b[at..]);
			return Ok(s);
		}
		while at + 4 <= b.len() {
			let (kind, len) = (u16at(b, at), usize::from(u16at(b, at + 2)));
			let body = b.get(at + 4..at + 4 + len).ok_or(HelpError::Bad("a |SYSTEM record runs past its end"))?;
			match kind {
				1 => s.title = zstr(body),
				2 => s.copyright = zstr(body),
				3 => s.contents = Some(u32at(body, 0)),
				_ => {}
			}
			at += 4 + len;
		}
		Ok(s)
	}

	/// `|CONTEXT`: (hash, topic offset).
	pub fn contexts(&self) -> Result<Vec<(i32, u32)>, HelpError> {
		let tree = self.file("|CONTEXT").ok_or(HelpError::Bad("no |CONTEXT"))?;
		let mut out = Vec::new();
		leaves(tree, |p| {
			out.push((u32at(p, 0) as i32, u32at(p, 4)));
			8
		})?;
		Ok(out)
	}

	/// The topics, in file order.
	pub fn topics(&self) -> Result<Vec<Topic>, HelpError> {
		let sys = self.system()?;
		let raw = self.file("|TOPIC").ok_or(HelpError::Bad("no |TOPIC"))?;
		let compressed = sys.minor > 16 && sys.flags != 0 && sys.flags != 0 && (sys.flags == 4 || sys.flags & 4 != 0 || sys.flags != 0);
		let tb = if sys.minor <= 16 { 0x800 } else if sys.flags == 0 || sys.flags == 4 { 0x1000 } else { 0x800 };
		let dsize = if compressed { 0x4000 } else { tb - 0x0C };
		let mut blocks: Vec<Vec<u8>> = Vec::new();
		for chunk in raw.chunks(tb) {
			if chunk.len() < 0x0C {
				break;
			}
			if compressed {
				blocks.push(lz77(&chunk[0x0C..]));
			} else {
				blocks.push(chunk[0x0C..].to_vec());
			}
		}
		let mut topics: Vec<Topic> = Vec::new();
		let mut reference = 0x0Cu32;
		let mut offs = 0u32;
		let mut old = usize::MAX;
		loop {
			let index = ((reference - 0x0C) >> 14) as usize;
			let offset = ((reference - 0x0C) & 0x3FFF) as usize;
			let Some(block) = blocks.get(index) else {
				break;
			};
			if index != old {
				offs = 0;
				old = index;
			}
			let Some(rec) = block.get(offset..) else {
				break;
			};
			if rec.len() < 0x15 {
				break;
			}
			let size = u32at(rec, 0) as usize;
			let kind = rec[0x14];
			let rec = &rec[..size.min(rec.len())];
			match kind {
				0x02 => {
					let datalen = u32at(rec, 0x10) as usize;
					let tsize = u32at(rec, 4) as usize;
					let title = rec.get(datalen..).map(|t| zstr(&t[..t.len().min(tsize)])).unwrap_or_default();
					topics.push(Topic {
						offset: index as u32 * 0x8000 + offs,
						title,
						paragraphs: Vec::new(),
					});
				}
				0x01 | 0x20 | 0x23 => {
					let mut p = 0x15;
					fetch_long(rec, &mut p);
					let len = fetch_ushort(rec, &mut p);
					offs += len;
					if let Some(t) = topics.last_mut() {
						t.paragraphs.push(paragraph(rec));
					}
				}
				_ => {}
			}
			let next = u32at(rec, 0x0C);
			if next == 0xFFFF_FFFF || next == 0 {
				break;
			}
			reference = next;
		}
		Ok(topics)
	}
}

/// Walks the leaves of a B+ tree in order, handing each entry's bytes to `entry`, which returns the entry's length.
pub fn leaves(tree: &[u8], mut entry: impl FnMut(&[u8]) -> usize) -> Result<(), HelpError> {
	if u16at(tree, 0) != 0x293B {
		return Err(HelpError::Bad("a B+ tree has the wrong signature"));
	}
	let page = usize::from(u16at(tree, 4));
	let levels = usize::from(u16at(tree, 32));
	let total = usize::from(u16at(tree, 30));
	let mut cur = usize::from(u16at(tree, 26));
	let pages = tree.get(38..).ok_or(HelpError::Bad("a B+ tree has no pages"))?;
	if levels == 0 || page == 0 {
		return Ok(());
	}
	for _ in 1..levels {
		cur = usize::from(u16at(pages, cur * page + 4));
	}
	let mut seen = 0;
	while cur != 0xFFFF && seen <= total {
		seen += 1;
		let base = cur * page;
		let n = usize::from(u16at(pages, base + 2));
		let mut at = base + 8;
		for _ in 0..n {
			let len = entry(pages.get(at..).ok_or(HelpError::Bad("a B+ tree page is short"))?);
			at += len;
		}
		cur = usize::from(u16at(pages, base + 6));
	}
	Ok(())
}

fn zstr(b: &[u8]) -> String {
	let n = b.iter().position(|c| *c == 0).unwrap_or(b.len());
	String::from_utf8_lossy(&b[..n]).into_owned()
}

/// The LZ77 WinHelp uses for topic blocks: a flag byte, then eight items, each a literal or a 16-bit (length - 3) and (offset - 1).
pub fn lz77(src: &[u8]) -> Vec<u8> {
	let mut out: Vec<u8> = Vec::new();
	let mut i = 0;
	while i < src.len() {
		let mask = src[i];
		i += 1;
		for bit in 0..8 {
			if i >= src.len() {
				break;
			}
			if mask >> bit & 1 == 1 {
				let code = usize::from(u16at(src, i));
				i += 2;
				let (len, off) = (3 + (code >> 12), (code & 0xFFF) + 1);
				for _ in 0..len {
					let b = out.len().checked_sub(off).and_then(|p| out.get(p)).copied().unwrap_or(0);
					out.push(b);
				}
			} else {
				out.push(src[i]);
				i += 1;
			}
		}
	}
	out
}

/// A compressed signed long: two bytes when the low bit is clear, four when it is set.
fn fetch_long(b: &[u8], p: &mut usize) -> i32 {
	if b.get(*p).is_some_and(|x| x & 1 == 1) {
		let v = u32at(b, *p);
		*p += 4;
		((i64::from(v) - 0x8000_0000) / 2) as i32
	} else {
		let v = u16at(b, *p);
		*p += 2;
		(i32::from(v) - 0x8000) / 2
	}
}

fn fetch_ushort(b: &[u8], p: &mut usize) -> u32 {
	if b.get(*p).is_some_and(|x| x & 1 == 1) {
		let v = u16at(b, *p);
		*p += 2;
		u32::from(v) / 2
	} else {
		let v = b.get(*p).copied().unwrap_or(0);
		*p += 1;
		u32::from(v) / 2
	}
}

fn fetch_short(b: &[u8], p: &mut usize) -> i32 {
	if b.get(*p).is_some_and(|x| x & 1 == 1) {
		let v = u16at(b, *p);
		*p += 2;
		(i32::from(v) - 0x8000) / 2
	} else {
		let v = b.get(*p).copied().unwrap_or(0);
		*p += 1;
		(i32::from(v) - 0x80) / 2
	}
}

/// One display paragraph as text: font changes, line breaks and links shown as markers.
fn paragraph(rec: &[u8]) -> String {
	let datalen = u32at(rec, 0x10) as usize;
	let size = u32at(rec, 4) as usize;
	let text = rec.get(datalen..).unwrap_or(&[]);
	let text = &text[..text.len().min(size.max(text.len().min(size)))];
	let mut f = 0x15;
	fetch_long(rec, &mut f);
	let _len = fetch_ushort(rec, &mut f);
	let _ = _len;
	// The paragraph's format: after the record header, `fetch_long`, then the two-byte start of the text format.
	let mut p = 0x15;
	fetch_long(rec, &mut p);
	fetch_ushort(rec, &mut p);
	// A display paragraph then has: the size of the topic part (a long), the bit field and the optional values.
	let bits = u16at(rec, p);
	p += 2;
	if bits & 0x0001 != 0 {
		fetch_long(rec, &mut p);
	}
	for bit in [0x0002, 0x0004, 0x0008, 0x0010, 0x0020, 0x0040] {
		if bits & bit != 0 {
			fetch_short(rec, &mut p);
		}
	}
	if bits & 0x0100 != 0 {
		p += 3;
	}
	if bits & 0x0200 != 0 {
		let n = fetch_short(rec, &mut p);
		for _ in 0..n {
			let tab = fetch_ushort_raw(rec, &mut p);
			if tab & 0x4000 != 0 {
				fetch_ushort_raw(rec, &mut p);
			}
		}
	}
	let format = rec.get(p..datalen.min(rec.len())).unwrap_or(&[]);
	let mut out = String::new();
	let mut t = 0;
	let mut fi = 0;
	while t < text.len() && fi < format.len() {
		let n = text[t..].iter().position(|c| *c == 0).unwrap_or(text.len() - t);
		out.push_str(&String::from_utf8_lossy(&text[t..t + n]));
		t += n + 1;
		match format[fi] {
			0xFF => break,
			0x20 => fi += 5,
			0x21 => fi += 3,
			0x80 => {
				out.push_str(&format!("{{f{}}}", u16at(format, fi + 1)));
				fi += 3;
			}
			0x81 => {
				out.push('\n');
				fi += 1;
			}
			0x82 => {
				out.push('\n');
				fi += 1;
			}
			0x83 => {
				out.push('\t');
				fi += 1;
			}
			0x89 => {
				out.push_str("]");
				fi += 1;
			}
			0xE0 | 0xE1 | 0xE2 | 0xE3 | 0xE6 | 0xE7 => {
				let kind = if format[fi] & 1 == 1 { 'J' } else { 'P' };
				out.push_str(&format!("[{kind}:{:08X} ", u32at(format, fi + 1)));
				fi += 5;
			}
			0xC8 | 0xCC => {
				let l = usize::from(u16at(format, fi + 1));
				out.push_str("[M:");
				out.push_str(&String::from_utf8_lossy(format.get(fi + 3..fi + 3 + l).unwrap_or(&[])));
				out.push(' ');
				fi += 3 + l;
			}
			0xEA | 0xEB | 0xEE | 0xEF => fi += 3 + usize::from(u16at(format, fi + 1)),
			0x86 | 0x87 | 0x88 => {
				out.push_str("[picture]");
				fi += 2;
				let mut q = fi;
				let size = fetch_long(format, &mut q) as usize;
				fi = q + size;
			}
			_ => fi += 1,
		}
	}
	out
}

fn fetch_ushort_raw(b: &[u8], p: &mut usize) -> u32 {
	if b.get(*p).is_some_and(|x| x & 1 == 1) {
		let v = u16at(b, *p);
		*p += 2;
		u32::from(v) / 2
	} else {
		let v = b.get(*p).copied().unwrap_or(0);
		*p += 1;
		u32::from(v) / 2
	}
}

impl Hlp<'_> {
	/// The decompressed topic blocks (each without its 12-byte header), for inspecting records.
	pub fn topic_blocks(&self) -> Result<Vec<Vec<u8>>, HelpError> {
		let sys = self.system()?;
		let raw = self.file("|TOPIC").ok_or(HelpError::Bad("no |TOPIC"))?;
		let tb = if sys.minor <= 16 { 0x800 } else { 0x1000 };
		let mut out = Vec::new();
		for chunk in raw.chunks(tb) {
			if chunk.len() >= 0x0C {
				out.push(if sys.minor > 16 && sys.flags & 4 != 0 { lz77(&chunk[0x0C..]) } else { chunk[0x0C..].to_vec() });
			}
		}
		Ok(out)
	}
}
