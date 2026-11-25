// The native .QLD format: a RIFF file of form type QLD1.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use quill_core::doc::{
	CFmt, Colour, Doc, Font, ListDef, ListLevel, PFmt, PageSetup, StoryKind, TabSet, TabStop,
	MAX_CFMTS, MAX_COLOURS, MAX_FONTS, MAX_LISTS, MAX_PFMTS, MAX_PICTURES, MAX_PICTURE_BYTES,
	MAX_STOPS, MAX_TABSETS, PAGE_LIMIT,
};
use quill_core::store::{ReadStore, Store};
use quill_core::text::{MAX_TEXT, SRC_OBJECT};

use crate::io::{Sink, Source};
use crate::{FormatError, Result};

/// The format version this code writes.
pub const VERSION: (u16, u16) = (1, 0);

const MAX_HF_TEXT: u32 = 4096;
const MAX_CHUNKS: usize = 64;

/// The bytes between progress calls while a file is written.
pub const PROGRESS_STEP: u32 = 4096;

/// The progress hook of a write: called with the bytes done so far and the file's size in bytes (0 during the counting pass). Returning
/// false stops the write, which then reports `FormatError::Write` like a full disk.
pub type Progress<'a> = &'a mut dyn FnMut(u32, u32) -> bool;

struct Emit<'a, K: Sink> {
	k: &'a mut K,
	pass2: bool,
	sizes: [u32; MAX_CHUNKS],
	idx: usize,
	stack: [(usize, u32); 4],
	depth: usize,
	count: u32,
	ok: bool,
	rcov: Option<&'a Recovery>,
	/// Set once the `RCOV` chunk is behind: the checksum covers what follows.
	sum: Option<Sum>,
	adler: u32,
	covered: u32,
	total: u32,
	since: u32,
	hook: Option<Progress<'a>>,
}

/// The checksum of the counting pass. That pass writes every chunk's size as 0, so each size is added afterwards: a byte `v` at offset
/// `i` of `n` covered bytes adds `v` to `a` and `v * (n - i)` to `b`.
struct Sum {
	start: u32,
	adler: Adler,
	/// The sum of the size bytes, and of each times its offset, both modulo 65521.
	size_sum: u32,
	size_weighted: u32,
}

impl<'a, K: Sink> Emit<'a, K> {
	fn new(k: &'a mut K) -> Self {
		Emit {
			k,
			pass2: false,
			sizes: [0; MAX_CHUNKS],
			idx: 0,
			stack: [(0, 0); 4],
			depth: 0,
			count: 0,
			ok: true,
			rcov: None,
			sum: None,
			adler: 0,
			covered: 0,
			total: 0,
			since: 0,
			hook: None,
		}
	}

	/// Ends the counting pass: the checksum and the length it gives are what the second pass writes into `RCOV`.
	fn restart(&mut self) {
		self.total = self.count;
		if let Some(m) = self.sum.take() {
			let n = self.count - m.start;
			let mut a = m.adler;
			a.a = (a.a + m.size_sum) % ADLER_MOD;
			let grown = (n % ADLER_MOD) * m.size_sum % ADLER_MOD;
			a.b = (a.b + grown + ADLER_MOD - m.size_weighted) % ADLER_MOD;
			self.adler = a.finish();
			self.covered = n;
		}
		self.pass2 = true;
		self.idx = 0;
		self.depth = 0;
		self.count = 0;
		self.since = 0;
	}

	fn put(&mut self, d: &[u8]) {
		self.count += d.len() as u32;
		if !self.ok {
			return;
		}
		if self.pass2 {
			if !self.k.write(d) {
				self.ok = false;
			}
		} else if let Some(m) = self.sum.as_mut() {
			m.adler.update(d);
		}
		self.since += d.len() as u32;
		if self.since >= PROGRESS_STEP {
			self.since = 0;
			let total = if self.pass2 { self.total } else { 0 };
			let count = self.count;
			if let Some(h) = self.hook.as_mut() {
				if !h(count, total) {
					self.ok = false;
				}
			}
		}
	}

	fn u8(&mut self, v: u8) {
		self.put(&[v]);
	}
	fn u16(&mut self, v: u16) {
		self.put(&v.to_le_bytes());
	}
	fn u32(&mut self, v: u32) {
		self.put(&v.to_le_bytes());
	}
	fn i32(&mut self, v: i32) {
		self.put(&v.to_le_bytes());
	}

	fn begin(&mut self, id: &[u8; 4], form: Option<&[u8; 4]>) {
		let idx = self.idx;
		self.idx += 1;
		let size = self
			.sizes
			.get(idx)
			.copied()
			.filter(|_| self.pass2)
			.unwrap_or(0);
		self.put(id);
		self.u32(size);
		if let Some(slot) = self.stack.get_mut(self.depth) {
			*slot = (idx, self.count);
		}
		self.depth += 1;
		if let Some(f) = form {
			self.put(f);
		}
	}

	fn end(&mut self) {
		self.depth = self.depth.saturating_sub(1);
		let (idx, start) = self.stack.get(self.depth).copied().unwrap_or((0, 0));
		let size = self.count - start;
		if !self.pass2 {
			if let Some(s) = self.sizes.get_mut(idx) {
				*s = size;
			}
			if let Some(m) = self.sum.as_mut() {
				// The size field sits just before `start` and is covered when it follows the `RCOV` chunk.
				if let Some(at) = (start - 4).checked_sub(m.start) {
					for (j, v) in size.to_le_bytes().into_iter().enumerate() {
						let v = u32::from(v);
						m.size_sum = (m.size_sum + v) % ADLER_MOD;
						let pos = (at + j as u32) % ADLER_MOD;
						m.size_weighted = (m.size_weighted + pos * v) % ADLER_MOD;
					}
				}
			}
		}
		if size % 2 == 1 {
			self.put(&[0]);
		}
	}
}

fn runs<S: ReadStore + ?Sized>(doc: &Doc, s: &S, kind: StoryKind, mut f: impl FnMut(u32, u16)) {
	let t = doc.story(kind);
	let mut cur: Option<(u32, u16)> = None;
	t.for_each_run(s, 0, t.len(), |_, p| match cur {
		Some((l, c)) if c == p.cfmt => cur = Some((l + p.len, c)),
		Some((l, c)) => {
			f(l, c);
			cur = Some((p.len, p.cfmt));
		}
		None => cur = Some((p.len, p.cfmt)),
	});
	if let Some((l, c)) = cur {
		f(l, c);
	}
}

fn marks<S: ReadStore + ?Sized>(doc: &Doc, s: &S, kind: StoryKind, mut f: impl FnMut(u32)) {
	let t = doc.story(kind);
	let mut at = 0u32;
	let mut buf = [0u8; 256];
	while at < t.len() {
		let n = t.read(s, at, &mut buf);
		if n == 0 {
			break;
		}
		for (i, b) in buf.iter().take(n).enumerate() {
			if *b == 0x0D {
				f(at + i as u32);
			}
		}
		at += n as u32;
	}
}

fn body<S: ReadStore + ?Sized, K: Sink>(doc: &Doc, s: &S, e: &mut Emit<'_, K>, stories: u16) {
	e.begin(b"RIFF", Some(b"QLD1"));
	e.begin(b"HEAD", None);
	e.u16(VERSION.0);
	e.u16(VERSION.1);
	e.u16(stories);
	e.u16(0);
	e.end();

	if let Some(r) = e.rcov {
		e.begin(b"RCOV", None);
		e.u16(RCOV_VERSION);
		e.u16(r.flags);
		e.u8(r.format);
		e.u8(r.writer);
		e.u8(r.quill.0);
		e.u8(r.quill.1);
		e.u32(r.written);
		e.u32(r.revision);
		e.u32(r.orig_size);
		e.u32(r.orig_stamp);
		e.u32(r.caret);
		e.u32(r.top_line);
		let (adler, covered) = (e.adler, e.covered);
		e.u32(adler);
		e.u32(covered);
		e.u16(r.path_len);
		e.put(r.path());
		e.end();
		if !e.pass2 {
			e.sum = Some(Sum {
				start: e.count,
				adler: Adler::new(),
				size_sum: 0,
				size_weighted: 0,
			});
		}
	}

	let p = &doc.page;
	e.begin(b"DOCP", None);
	for v in [
		p.paper_w,
		p.paper_h,
		p.margin_l,
		p.margin_r,
		p.margin_t,
		p.margin_b,
		p.header_dist,
		p.footer_dist,
	] {
		e.i32(v);
	}
	e.u16(p.first_page);
	e.u16(p.flags);
	e.u8(p.units);
	e.u8(p.lang);
	e.end();

	e.begin(b"FONT", None);
	e.u16(doc.fonts.len() as u16);
	for i in 0..doc.fonts.len() {
		if let Some(f) = doc.fonts.get(s, i as u16) {
			e.u8(f.family);
			e.u8(f.pitch);
			e.u8(f.charset);
			e.u8(f.name_len);
			e.put(f.name());
		}
	}
	e.end();

	e.begin(b"COLR", None);
	e.u16(doc.colours.len() as u16);
	for i in 0..doc.colours.len() {
		if let Some(c) = doc.colours.get(s, i as u16) {
			e.put(&[c.r, c.g, c.b]);
		}
	}
	e.end();

	e.begin(b"CFMT", None);
	e.u16(doc.cfmts.len() as u16);
	for i in 0..doc.cfmts.len() {
		if let Some(c) = doc.cfmts.get(s, i as u16) {
			e.u16(c.font);
			e.u16(c.size);
			e.u8(c.colour);
			e.u8(c.vpos);
			e.u16(c.flags);
		}
	}
	e.end();

	e.begin(b"TABS", None);
	e.u16(doc.tabsets.len() as u16);
	for i in 0..doc.tabsets.len() {
		if let Some(t) = doc.tabsets.get(s, i as u16) {
			e.u8(t.count);
			for st in t.stops.iter().take(usize::from(t.count)) {
				e.i32(st.pos);
				e.u8(st.kind);
				e.u8(st.leader);
			}
		}
	}
	e.end();

	e.begin(b"LIST", None);
	e.u16(doc.lists.len() as u16);
	for i in 0..doc.lists.len() {
		if let Some(l) = doc.lists.get(s, i as u16) {
			for lv in &l.levels {
				e.u8(lv.style);
				e.u16(lv.start);
				e.i32(lv.indent);
			}
		}
	}
	e.end();

	e.begin(b"PFMT", None);
	e.u16(doc.pfmts.len() as u16);
	for i in 0..doc.pfmts.len() {
		if let Some(f) = doc.pfmts.get(s, i as u16) {
			e.u8(f.align);
			e.u8(f.spacing);
			e.u16(f.flags);
			e.u16(f.list);
			e.u8(f.level);
			e.u16(f.tabset);
			for v in [f.left, f.right, f.first, f.before, f.after] {
				e.i32(v);
			}
		}
	}
	e.end();

	for i in 0..doc.pictures.len() {
		let Some(p) = doc.pictures.get(s, i) else {
			continue;
		};
		e.begin(b"PICT", None);
		e.u8(p.kind);
		e.u16(p.w);
		e.u16(p.h);
		e.i32(p.shown_w);
		e.i32(p.shown_h);
		let mut at = 0u32;
		let mut buf = [0u8; 512];
		while at < p.len {
			let n = doc.picture_bytes(s, i as u16, at, &mut buf);
			if n == 0 {
				break;
			}
			e.put(buf.get(..n).unwrap_or(&[]));
			at += n as u32;
		}
		e.end();
	}

	for kind in StoryKind::ALL {
		let t = doc.story(kind);
		if kind != StoryKind::Body && t.is_empty() {
			continue;
		}
		e.begin(b"LIST", Some(b"STRY"));
		e.begin(b"STHD", None);
		e.u8(kind as u8);
		e.u32(t.len());
		e.end();

		e.begin(b"TEXT", None);
		let mut at = 0u32;
		let mut buf = [0u8; 512];
		while at < t.len() {
			let n = t.read(s, at, &mut buf);
			if n == 0 {
				break;
			}
			e.put(buf.get(..n).unwrap_or(&[]));
			at += n as u32;
		}
		e.end();

		e.begin(b"RUNS", None);
		let mut n = 0u32;
		runs(doc, s, kind, |_, _| n += 1);
		e.u32(n);
		runs(doc, s, kind, |l, c| {
			e.u32(l);
			e.u16(c);
		});
		e.end();

		e.begin(b"PARA", None);
		marks(doc, s, kind, |p| e.u16(t.pfmt_at(s, p).unwrap_or(0)));
		e.end();

		e.begin(b"OBJS", None);
		t.for_each_run(s, 0, t.len(), |_, p| {
			if p.source == SRC_OBJECT {
				e.u8((p.start >> 16) as u8);
				e.u16((p.start & 0xFFFF) as u16);
			}
		});
		e.end();
		e.end();
	}
	e.end();
}

/// Writes `doc` as a `.QLD` file. The same document always gives the same bytes.
pub fn write<S: ReadStore + ?Sized, K: Sink>(doc: &Doc, s: &S, k: &mut K) -> Result<()> {
	write_inner(doc, s, k, None, None)
}

/// Writes `doc` as `write` does, calling `progress(done, total)` about every `PROGRESS_STEP` bytes (`total` is 0 while the first pass counts).
/// When `progress` returns false the write stops, nothing more reaches `k` and the result is `FormatError::Write`.
pub fn write_with_progress<'a, S: ReadStore + ?Sized, K: Sink>(
	doc: &Doc,
	s: &S,
	k: &'a mut K,
	progress: Progress<'a>,
) -> Result<()> {
	write_inner(doc, s, k, None, Some(progress))
}

/// Writes `doc` as a recovery file: the same bytes as `write` with an `RCOV` chunk right after `HEAD`, which holds `rec` and the Adler-32 and
/// length of everything after it. The checksum comes from the counting pass, so the file is not read back or written twice.
pub fn write_recovery<'a, S: ReadStore + ?Sized, K: Sink>(
	doc: &Doc,
	s: &S,
	k: &'a mut K,
	rec: &'a Recovery,
	progress: Option<Progress<'a>>,
) -> Result<()> {
	if usize::from(rec.path_len) > MAX_RCOV_PATH {
		return Err(FormatError::TooLarge);
	}
	write_inner(doc, s, k, Some(rec), progress)
}

fn write_inner<'a, S: ReadStore + ?Sized, K: Sink>(
	doc: &Doc,
	s: &S,
	k: &'a mut K,
	rec: Option<&'a Recovery>,
	hook: Option<Progress<'a>>,
) -> Result<()> {
	let stories = StoryKind::ALL
		.iter()
		.filter(|k| **k == StoryKind::Body || !doc.story(**k).is_empty())
		.count() as u16;
	let mut e = Emit::new(k);
	e.rcov = rec;
	e.hook = hook;
	body(doc, s, &mut e, stories);
	if !e.ok {
		return Err(FormatError::Write);
	}
	e.restart();
	body(doc, s, &mut e, stories);
	if e.ok {
		Ok(())
	} else {
		Err(FormatError::Write)
	}
}

const ADLER_MOD: u32 = 65521;

/// Adler-32 (RFC 1950): two sums, no table.
#[derive(Copy, Clone)]
pub struct Adler {
	a: u32,
	b: u32,
}

impl Default for Adler {
	fn default() -> Self {
		Self::new()
	}
}

impl Adler {
	/// The empty checksum, 1.
	pub const fn new() -> Self {
		Adler { a: 1, b: 0 }
	}

	/// Adds `data`.
	pub fn update(&mut self, data: &[u8]) {
		// 5552 bytes is the most that cannot overflow 32 bits before the sums are reduced.
		for block in data.chunks(5552) {
			for v in block {
				self.a += u32::from(*v);
				self.b += self.a;
			}
			self.a %= ADLER_MOD;
			self.b %= ADLER_MOD;
		}
	}

	/// The checksum of everything added.
	pub const fn finish(self) -> u32 {
		(self.b << 16) | self.a
	}
}

/// The version of the `RCOV` chunk this code writes and reads.
pub const RCOV_VERSION: u16 = 1;
/// The longest original path a recovery file records, in bytes.
pub const MAX_RCOV_PATH: usize = 259;
/// The most bytes `read_header` reads: the `RIFF`, `HEAD` and `RCOV` headers, the fixed fields and the longest padded path.
pub const RCOV_HEAD_MAX: usize = 28 + 8 + 42 + MAX_RCOV_PATH + 1;
/// The bytes `verify` reads at a time.
pub const VERIFY_BLOCK: usize = 1024;

/// `Recovery::flags`: the document has no name yet.
pub const RCV_UNTITLED: u16 = 1;
/// `Recovery::flags`: written after a failure.
pub const RCV_EMERGENCY: u16 = 2;
/// `Recovery::flags`: written as a session ended.
pub const RCV_END_OF_SESSION: u16 = 4;
/// `Recovery::flags`: written by the autosave timer.
pub const RCV_AUTOSAVE: u16 = 8;
/// `Recovery::flags`: the path is UTF-8, not the system code page.
pub const RCV_PATH_UTF8: u16 = 16;
/// `Recovery::flags`: the file is in the central recovery folder, not beside the original.
pub const RCV_CENTRAL: u16 = 32;

/// The fields of an `RCOV` chunk: what a recovery file says about the document it holds. A reader fills `version`, `adler`, `covered` and
/// `body`; a writer sets the rest and the writer computes the checksum.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recovery {
	/// The chunk's version (read only).
	pub version: u16,
	/// `RCV_*` bits.
	pub flags: u16,
	/// The document's own format: 0 `.QLD`, 1 RTF, 2 text.
	pub format: u8,
	/// Which build wrote the file: 0 DOS text, 1 DOS graphics, 2 Win16, 3 Win32, 4 terminal, 5 GTK, 6 WinUI, 7 web.
	pub writer: u8,
	/// The Quill version that wrote it, major and minor.
	pub quill: (u8, u8),
	/// When it was written, as a DOS packed date and time.
	pub written: u32,
	/// `Doc::revision` at the write.
	pub revision: u32,
	/// The original's size when the session opened or last saved it (`u32::MAX`: none).
	pub orig_size: u32,
	/// The original's DOS date and time then.
	pub orig_stamp: u32,
	/// The caret position.
	pub caret: u32,
	/// The top line of the view.
	pub top_line: u32,
	/// The Adler-32 of the covered bytes (read only).
	pub adler: u32,
	/// How many bytes follow the chunk to the end of the file (read only).
	pub covered: u32,
	/// How many bytes of `path` are used (0 for an untitled document).
	pub path_len: u16,
	path: [u8; MAX_RCOV_PATH],
	/// Where the covered bytes start in the file (read only).
	pub body: u32,
}

impl Default for Recovery {
	fn default() -> Self {
		Recovery {
			version: RCOV_VERSION,
			flags: 0,
			format: 0,
			writer: 0,
			quill: (0, 0),
			written: 0,
			revision: 0,
			orig_size: u32::MAX,
			orig_stamp: 0,
			caret: 0,
			top_line: 0,
			adler: 0,
			covered: 0,
			path_len: 0,
			path: [0; MAX_RCOV_PATH],
			body: 0,
		}
	}
}

impl Recovery {
	/// The original's full path as recorded, empty for an untitled document.
	pub fn path(&self) -> &[u8] {
		self.path.get(..usize::from(self.path_len)).unwrap_or(&[])
	}

	/// Records the original's path; a path over `MAX_RCOV_PATH` bytes is refused.
	pub fn set_path(&mut self, path: &[u8]) -> Result<()> {
		let dst = self
			.path
			.get_mut(..path.len())
			.ok_or(FormatError::TooLarge)?;
		dst.copy_from_slice(path);
		self.path_len = path.len() as u16;
		Ok(())
	}
}

fn le16(b: &[u8], at: usize) -> u16 {
	u16::from_le_bytes([
		b.get(at).copied().unwrap_or(0),
		b.get(at + 1).copied().unwrap_or(0),
	])
}

fn le32(b: &[u8], at: usize) -> u32 {
	u32::from(le16(b, at)) | (u32::from(le16(b, at + 2)) << 16)
}

/// Reads the `RCOV` chunk at the front of a recovery file, from at most `RCOV_HEAD_MAX` bytes, and checks it against the file's own
/// sizes. A file that is not a recovery file, or whose header is damaged, is `BadFile`; a newer major version is `Newer`. The fields are not
/// covered by the checksum, so a caller that goes on to use the document calls `verify` as well.
pub fn read_header<R: Source>(r: &mut R) -> Result<Recovery> {
	let mut h = [0u8; 36];
	if r.read(&mut h) != h.len() {
		return Err(FormatError::BadFile);
	}
	let riff = le32(&h, 4);
	if h.get(..4) != Some(b"RIFF") || h.get(8..12) != Some(b"QLD1") {
		return Err(FormatError::BadFile);
	}
	if h.get(12..16) != Some(b"HEAD") || le32(&h, 16) != 8 {
		return Err(FormatError::BadFile);
	}
	if le16(&h, 20) > VERSION.0 {
		return Err(FormatError::Newer);
	}
	if h.get(28..32) != Some(b"RCOV") {
		return Err(FormatError::BadFile);
	}
	let size = le32(&h, 32);
	let fixed = 42u32;
	if size < fixed || size > fixed + MAX_RCOV_PATH as u32 {
		return Err(FormatError::BadFile);
	}
	// The chunk's data, then the pad byte RIFF puts after an odd size.
	let mut f = [0u8; 42 + MAX_RCOV_PATH + 1];
	let take = (size + size % 2) as usize;
	if r.read(f.get_mut(..take).ok_or(FormatError::BadFile)?) != take {
		return Err(FormatError::BadFile);
	}
	let data = f.get(..size as usize).ok_or(FormatError::BadFile)?;
	let mut rec = Recovery {
		version: le16(data, 0),
		flags: le16(data, 2),
		format: data.get(4).copied().unwrap_or(0),
		writer: data.get(5).copied().unwrap_or(0),
		quill: (
			data.get(6).copied().unwrap_or(0),
			data.get(7).copied().unwrap_or(0),
		),
		written: le32(data, 8),
		revision: le32(data, 12),
		orig_size: le32(data, 16),
		orig_stamp: le32(data, 20),
		caret: le32(data, 24),
		top_line: le32(data, 28),
		adler: le32(data, 32),
		covered: le32(data, 36),
		path_len: le16(data, 40),
		path: [0; MAX_RCOV_PATH],
		body: 36 + size + size % 2,
	};
	if rec.version == 0 || rec.format > 2 {
		return Err(FormatError::BadFile);
	}
	if rec.version > RCOV_VERSION {
		return Err(FormatError::Newer);
	}
	let plen = usize::from(rec.path_len);
	if plen > MAX_RCOV_PATH || size as usize != 42 + plen {
		return Err(FormatError::BadFile);
	}
	rec.set_path(data.get(42..42 + plen).ok_or(FormatError::BadFile)?)?;
	// The RIFF chunk holds everything after its own header, so the covered bytes run from the end of `RCOV` to its end.
	let end = riff.checked_add(8).ok_or(FormatError::BadFile)?;
	if end.checked_sub(rec.body) != Some(rec.covered) {
		return Err(FormatError::BadFile);
	}
	Ok(rec)
}

/// Checks a whole recovery file: the header as `read_header` does, then the Adler-32 of every byte after it, read `VERIFY_BLOCK` bytes at
/// a time, and that the file ends where the `RIFF` chunk does. Returns the header of a file that passes. This reads no document, so it
/// allocates nothing and cannot fail on memory.
pub fn verify<R: Source>(r: &mut R) -> Result<Recovery> {
	let rec = read_header(r)?;
	let mut left = rec.covered;
	let mut sum = Adler::new();
	let mut buf = [0u8; VERIFY_BLOCK];
	while left > 0 {
		let n = (left as usize).min(VERIFY_BLOCK);
		let part = buf.get_mut(..n).ok_or(FormatError::BadFile)?;
		if r.read(part) != n {
			return Err(FormatError::BadFile);
		}
		sum.update(part);
		left -= n as u32;
	}
	let mut extra = [0u8; 1];
	if r.read(&mut extra) != 0 || sum.finish() != rec.adler {
		return Err(FormatError::BadFile);
	}
	Ok(rec)
}

struct Ck<'a, R: Source> {
	r: &'a mut R,
	left: u32,
}

impl<R: Source> Ck<'_, R> {
	fn bytes(&mut self, out: &mut [u8]) -> Result<()> {
		if out.len() as u64 > u64::from(self.left) {
			return Err(FormatError::BadFile);
		}
		if self.r.read(out) != out.len() {
			return Err(FormatError::BadFile);
		}
		self.left -= out.len() as u32;
		Ok(())
	}
	fn u8(&mut self) -> Result<u8> {
		let mut b = [0u8; 1];
		self.bytes(&mut b)?;
		Ok(u16::from(b[0]) as u8)
	}
	fn u16(&mut self) -> Result<u16> {
		let mut b = [0u8; 2];
		self.bytes(&mut b)?;
		Ok(u16::from_le_bytes(b))
	}
	fn u32(&mut self) -> Result<u32> {
		let mut b = [0u8; 4];
		self.bytes(&mut b)?;
		Ok(u32::from_le_bytes(b))
	}
	fn i32(&mut self) -> Result<i32> {
		Ok(self.u32()? as i32)
	}
	fn skip_rest(&mut self) -> Result<()> {
		let mut b = [0u8; 64];
		while self.left > 0 {
			let n = (self.left as usize).min(64);
			let part = b.get_mut(..n).ok_or(FormatError::BadFile)?;
			self.bytes(part)?;
		}
		Ok(())
	}
}

fn chunk_header<R: Source>(r: &mut R) -> Result<Option<([u8; 4], u32)>> {
	let mut h = [0u8; 8];
	let n = r.read(&mut h);
	if n == 0 {
		return Ok(None);
	}
	if n != 8 {
		return Err(FormatError::BadFile);
	}
	let id = [h[0], h[1], h[2], h[3]];
	let size = u32::from_le_bytes([h[4], h[5], h[6], h[7]]);
	Ok(Some((id, size)))
}

fn pad<R: Source>(r: &mut R, size: u32) -> Result<()> {
	if size % 2 == 1 {
		let mut b = [0u8; 1];
		if r.read(&mut b) != 1 {
			return Err(FormatError::BadFile);
		}
	}
	Ok(())
}

/// Reads a `.QLD` file into `doc`, replacing its content. On error `doc` is left in an unspecified but memory-safe
/// state: read into a fresh `Doc` and swap it in on success.
pub fn read<S: Store + ?Sized, R: Source>(doc: &mut Doc, s: &mut S, r: &mut R) -> Result<()> {
	doc.reset(s);
	let (id, total) = chunk_header(r)?.ok_or(FormatError::BadFile)?;
	if &id != b"RIFF" || total < 4 {
		return Err(FormatError::BadFile);
	}
	let mut form = [0u8; 4];
	if r.read(&mut form) != 4 || &form != b"QLD1" {
		return Err(FormatError::BadFile);
	}
	let mut left = total - 4;
	let mut seen_head = false;
	let mut seen_body = false;
	while left >= 8 {
		let (id, size) = chunk_header(r)?.ok_or(FormatError::BadFile)?;
		left -= 8;
		let padded = size + (size % 2);
		if padded > left {
			return Err(FormatError::BadFile);
		}
		left -= padded;
		let mut c = Ck { r, left: size };
		match &id {
			b"HEAD" => {
				let major = c.u16()?;
				c.u16()?;
				c.u16()?;
				c.u16()?;
				if major > VERSION.0 {
					return Err(FormatError::Newer);
				}
				seen_head = true;
			}
			b"DOCP" => {
				let mut p = PageSetup::LETTER;
				p.paper_w = c.i32()?;
				p.paper_h = c.i32()?;
				p.margin_l = c.i32()?;
				p.margin_r = c.i32()?;
				p.margin_t = c.i32()?;
				p.margin_b = c.i32()?;
				p.header_dist = c.i32()?;
				p.footer_dist = c.i32()?;
				p.first_page = c.u16()?;
				p.flags = c.u16()?;
				p.units = c.u8()?;
				p.lang = c.u8()?;
				let sane = |v: i32| (0..=PAGE_LIMIT).contains(&v);
				if p.paper_w <= 0
					|| p.paper_h <= 0
					|| ![
						p.paper_w, p.paper_h, p.margin_l, p.margin_r, p.margin_t, p.margin_b,
					]
					.into_iter()
					.all(sane) || p.margin_l.saturating_add(p.margin_r) >= p.paper_w
					|| p.margin_t.saturating_add(p.margin_b) >= p.paper_h
				{
					return Err(FormatError::BadFile);
				}
				doc.set_page(p);
			}
			b"FONT" => {
				let n = u32::from(c.u16()?);
				if n > MAX_FONTS {
					return Err(FormatError::BadFile);
				}
				for i in 0..n {
					let family = c.u8()?;
					let pitch = c.u8()?;
					let charset = c.u8()?;
					let len = usize::from(c.u8()?);
					if len > 31 {
						return Err(FormatError::BadFile);
					}
					let mut name = [0u8; 31];
					c.bytes(name.get_mut(..len).ok_or(FormatError::BadFile)?)?;
					let f = Font {
						family,
						pitch,
						charset,
						name_len: len as u8,
						name,
					};
					if u32::from(doc.fonts.intern(s, f)?) != i {
						return Err(FormatError::BadFile);
					}
				}
			}
			b"COLR" => {
				let n = u32::from(c.u16()?);
				if n > MAX_COLOURS {
					return Err(FormatError::BadFile);
				}
				for i in 0..n {
					let col = Colour {
						r: c.u8()?,
						g: c.u8()?,
						b: c.u8()?,
					};
					if u32::from(doc.colours.intern(s, col)?) != i {
						return Err(FormatError::BadFile);
					}
				}
			}
			b"CFMT" => {
				let n = u32::from(c.u16()?);
				if n > MAX_CFMTS {
					return Err(FormatError::BadFile);
				}
				for i in 0..n {
					let f = CFmt {
						font: c.u16()?,
						size: c.u16()?,
						colour: c.u8()?,
						vpos: c.u8()?,
						flags: c.u16()?,
					};
					if u32::from(f.font) >= doc.fonts.len().max(1)
						|| u32::from(f.colour) >= doc.colours.len().max(1)
					{
						return Err(FormatError::BadFile);
					}
					if u32::from(doc.cfmts.intern(s, f)?) != i {
						return Err(FormatError::BadFile);
					}
				}
			}
			b"TABS" => {
				let n = u32::from(c.u16()?);
				if n > MAX_TABSETS {
					return Err(FormatError::BadFile);
				}
				for i in 0..n {
					let count = c.u8()?;
					if usize::from(count) > MAX_STOPS {
						return Err(FormatError::BadFile);
					}
					let mut t = TabSet {
						count,
						..TabSet::default()
					};
					for st in t.stops.iter_mut().take(usize::from(count)) {
						*st = TabStop {
							pos: c.i32()?,
							kind: c.u8()?,
							leader: c.u8()?,
						};
					}
					if u32::from(doc.tabsets.intern(s, t)?) != i {
						return Err(FormatError::BadFile);
					}
				}
			}
			b"PICT" => {
				if doc.pictures.len() >= MAX_PICTURES {
					return Err(FormatError::BadFile);
				}
				let kind = c.u8()?;
				let (w, h) = (c.u16()?, c.u16()?);
				let (sw, sh) = (c.i32()?, c.i32()?);
				let len = c.left;
				if kind > 1 || len > MAX_PICTURE_BYTES || sw <= 0 || sh <= 0 {
					return Err(FormatError::BadFile);
				}
				let mut buf = [0u8; 512];
				let mut offset = None;
				while c.left > 0 {
					let n = (c.left as usize).min(512);
					let part = buf.get_mut(..n).ok_or(FormatError::BadFile)?;
					c.bytes(part)?;
					let at = doc.pic_data.append(s, part)?;
					offset.get_or_insert(at);
				}
				doc.pictures.reserve(s, 1)?;
				let offset = offset.unwrap_or(doc.pic_data.len());
				doc.pictures.push(
					s,
					quill_core::doc::Picture {
						kind,
						w,
						h,
						shown_w: sw,
						shown_h: sh,
						offset,
						len,
					},
				)?;
			}
			b"PFMT" => {
				let n = u32::from(c.u16()?);
				if n > MAX_PFMTS {
					return Err(FormatError::BadFile);
				}
				for i in 0..n {
					let f = PFmt {
						align: c.u8()?,
						spacing: c.u8()?,
						flags: c.u16()?,
						list: c.u16()?,
						level: c.u8()?,
						tabset: c.u16()?,
						left: c.i32()?,
						right: c.i32()?,
						first: c.i32()?,
						before: c.i32()?,
						after: c.i32()?,
					};
					if u32::from(doc.pfmts.intern(s, f)?) != i {
						return Err(FormatError::BadFile);
					}
				}
			}
			b"LIST" => {
				// A story container starts with its form type, `STRY`; a list-definition chunk starts with a count
				// (at most 64), so the two cannot be confused.
				let first = c.u16()?;
				if first == u16::from_le_bytes(*b"ST") {
					if c.u16()? != u16::from_le_bytes(*b"RY") || !seen_head {
						return Err(FormatError::BadFile);
					}
					read_story(doc, s, &mut c)?;
					seen_body = true;
				} else {
					let n = u32::from(first);
					if n > MAX_LISTS {
						return Err(FormatError::BadFile);
					}
					for i in 0..n {
						let mut l = ListDef::default();
						for lv in l.levels.iter_mut() {
							*lv = ListLevel {
								style: c.u8()?,
								start: c.u16()?,
								indent: c.i32()?,
							};
						}
						if u32::from(doc.lists.intern(s, l)?) != i {
							return Err(FormatError::BadFile);
						}
					}
				}
			}
			_ => {}
		}
		c.skip_rest()?;
		pad(r, size)?;
	}
	if !seen_head || !seen_body {
		return Err(FormatError::BadFile);
	}
	let n = doc.body.len();
	if n == 0 || doc.body.read_byte(s, n - 1) != Some(0x0D) {
		return Err(FormatError::BadFile);
	}
	for k in StoryKind::ALL {
		doc.story_mut(k).forget_history(s);
	}
	Ok(())
}

fn read_story<S: Store + ?Sized, R: Source>(
	doc: &mut Doc,
	s: &mut S,
	c: &mut Ck<'_, R>,
) -> Result<()> {
	let mut kind = None;
	let mut count = 0u32;
	let mut paras = 0u32;
	while c.left >= 8 {
		let (id, size) = chunk_header(c.r)?.ok_or(FormatError::BadFile)?;
		c.left -= 8;
		let padded = size + (size % 2);
		if padded > c.left {
			return Err(FormatError::BadFile);
		}
		c.left -= padded;
		let mut k = Ck { r: c.r, left: size };
		match &id {
			b"STHD" => {
				let kd = StoryKind::from_u8(k.u8()?).ok_or(FormatError::BadFile)?;
				count = k.u32()?;
				let limit = if kd == StoryKind::Body {
					MAX_TEXT
				} else {
					MAX_HF_TEXT
				};
				if count > limit {
					return Err(FormatError::TooLarge);
				}
				doc.story_mut(kd).clear(s);
				kind = Some(kd);
			}
			b"TEXT" => {
				let kd = kind.ok_or(FormatError::BadFile)?;
				if size != count {
					return Err(FormatError::BadFile);
				}
				let mut buf = [0u8; 512];
				while k.left > 0 {
					let n = (k.left as usize).min(512);
					let part = buf.get_mut(..n).ok_or(FormatError::BadFile)?;
					k.bytes(part)?;
					paras += part.iter().filter(|b| **b == 0x0D).count() as u32;
					doc.story_mut(kd).append_original(s, part, 0, 0)?;
				}
				if count > 0 && doc.story(kd).read_byte(s, count - 1) != Some(0x0D) {
					return Err(FormatError::BadFile);
				}
			}
			b"RUNS" => {
				let kd = kind.ok_or(FormatError::BadFile)?;
				let n = k.u32()?;
				let mut at = 0u32;
				for _ in 0..n {
					let len = k.u32()?;
					let cf = k.u16()?;
					if u32::from(cf) >= doc.cfmts.len() || len > count - at.min(count) {
						return Err(FormatError::BadFile);
					}
					doc.story_mut(kd).set_cfmt(s, at, len, cf, 0)?;
					at += len;
				}
				if at != count {
					return Err(FormatError::BadFile);
				}
			}
			b"PARA" => {
				let kd = kind.ok_or(FormatError::BadFile)?;
				if size != paras * 2 {
					return Err(FormatError::BadFile);
				}
				let mut start = 0u32;
				let mut at = 0u32;
				let mut buf = [0u8; 256];
				while at < count {
					let n = doc.story(kd).read(s, at, &mut buf);
					if n == 0 {
						return Err(FormatError::BadFile);
					}
					for i in 0..n {
						if buf.get(i) == Some(&0x0D) {
							let end = at + i as u32 + 1;
							let pf = k.u16()?;
							if u32::from(pf) >= doc.pfmts.len() {
								return Err(FormatError::BadFile);
							}
							doc.story_mut(kd).set_pfmt(s, start, end - start, pf, 0)?;
							start = end;
						}
					}
					at += n as u32;
				}
			}
			b"OBJS" => {
				let kd = kind.ok_or(FormatError::BadFile)?;
				let mut at = 0u32;
				let mut buf = [0u8; 256];
				let mut positions = 0u32;
				while at < count {
					let n = doc.story(kd).read(s, at, &mut buf);
					if n == 0 {
						return Err(FormatError::BadFile);
					}
					for i in 0..n {
						if buf.get(i) == Some(&1) {
							let kindb = k.u8()?;
							let idx = k.u16()?;
							doc.story_mut(kd).convert_to_object(
								s,
								at + i as u32,
								(u32::from(kindb) << 16) | u32::from(idx),
							)?;
							positions += 1;
						}
					}
					at += n as u32;
				}
				if positions > 0 && k.left != 0 {
					return Err(FormatError::BadFile);
				}
			}
			_ => {}
		}
		k.skip_rest()?;
		pad(c.r, size)?;
	}
	c.skip_rest()
}
