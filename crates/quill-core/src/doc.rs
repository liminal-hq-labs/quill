// The document: formats, tables, page setup and the stories.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::bytes::{Pod, Reader, Writer};
use crate::farvec::FarVec;
use crate::store::{ReadStore, Store};
use crate::text::{ByteBuf, Text};
use crate::{Error, Result};

/// Fonts in one document.
pub const MAX_FONTS: u32 = 64;
/// Colours in one document.
pub const MAX_COLOURS: u32 = 256;
/// Distinct character formats.
pub const MAX_CFMTS: u32 = 4096;
/// Distinct paragraph formats.
pub const MAX_PFMTS: u32 = 1024;
/// Distinct tab sets.
pub const MAX_TABSETS: u32 = 256;
/// List definitions.
pub const MAX_LISTS: u32 = 64;
/// Tab stops in one set.
pub const MAX_STOPS: usize = 16;
/// The tab set index that means no tab set.
pub const NO_TABS: u16 = 0xFFFF;

/// The character format flag bits.
pub mod cflag {
	/// Bold.
	pub const BOLD: u16 = 1;
	/// Italic.
	pub const ITALIC: u16 = 2;
	/// Underline.
	pub const UNDERLINE: u16 = 4;
	/// Strikethrough.
	pub const STRIKE: u16 = 8;
}

/// A font in the document's font table.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Font {
	/// 0 don't care, 1 roman, 2 swiss, 3 modern, 4 script, 5 decorative.
	pub family: u8,
	/// Pitch.
	pub pitch: u8,
	/// Character set.
	pub charset: u8,
	/// Length of the name.
	pub name_len: u8,
	/// The face name, ANSI, at most 31 bytes.
	pub name: [u8; 31],
}

impl Font {
	/// A font from a name (cut at 31 bytes).
	pub fn new(name: &[u8], family: u8) -> Self {
		let mut n = [0u8; 31];
		let len = name.len().min(31);
		for (d, s) in n.iter_mut().zip(name.iter()) {
			*d = *s;
		}
		Font {
			family,
			pitch: 0,
			charset: 0,
			name_len: len as u8,
			name: n,
		}
	}

	/// The name bytes.
	pub fn name(&self) -> &[u8] {
		self.name
			.get(..usize::from(self.name_len).min(31))
			.unwrap_or(&[])
	}
}

impl Pod for Font {
	const SIZE: usize = 36;
	fn write(&self, w: &mut Writer<'_>) {
		w.u8(self.family);
		w.u8(self.pitch);
		w.u8(self.charset);
		w.u8(self.name_len);
		w.bytes(&self.name);
	}
	fn read(r: &mut Reader<'_>) -> Self {
		let family = r.u8();
		let pitch = r.u8();
		let charset = r.u8();
		let name_len = r.u8().min(31);
		let mut name = [0u8; 31];
		r.bytes(&mut name);
		Font {
			family,
			pitch,
			charset,
			name_len,
			name,
		}
	}
}

/// An RGB colour.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Colour {
	/// Red.
	pub r: u8,
	/// Green.
	pub g: u8,
	/// Blue.
	pub b: u8,
}

impl Pod for Colour {
	const SIZE: usize = 3;
	fn write(&self, w: &mut Writer<'_>) {
		w.u8(self.r);
		w.u8(self.g);
		w.u8(self.b);
	}
	fn read(r: &mut Reader<'_>) -> Self {
		Colour {
			r: r.u8(),
			g: r.u8(),
			b: r.u8(),
		}
	}
}

/// A character format.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct CFmt {
	/// Index into the font table.
	pub font: u16,
	/// Size in half points.
	pub size: u16,
	/// Index into the colour table (0 is Automatic).
	pub colour: u8,
	/// 0 normal, 1 superscript, 2 subscript.
	pub vpos: u8,
	/// `cflag` bits.
	pub flags: u16,
}

impl Pod for CFmt {
	const SIZE: usize = 8;
	fn write(&self, w: &mut Writer<'_>) {
		w.u16(self.font);
		w.u16(self.size);
		w.u8(self.colour);
		w.u8(self.vpos);
		w.u16(self.flags);
	}
	fn read(r: &mut Reader<'_>) -> Self {
		CFmt {
			font: r.u16(),
			size: r.u16(),
			colour: r.u8(),
			vpos: r.u8(),
			flags: r.u16(),
		}
	}
}

/// A paragraph format.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct PFmt {
	/// 0 left, 1 centre, 2 right, 3 justify.
	pub align: u8,
	/// 0 single, 1 one and a half, 2 double.
	pub spacing: u8,
	/// Bit 0 bullet, 1 page break before, 2 numbered, 3 restart numbering.
	pub flags: u16,
	/// List definition index.
	pub list: u16,
	/// List level, 0 to 8.
	pub level: u8,
	/// Tab set index, or `NO_TABS`.
	pub tabset: u16,
	/// Left indent in twips.
	pub left: i32,
	/// Right indent in twips.
	pub right: i32,
	/// First-line indent in twips.
	pub first: i32,
	/// Space before in twips.
	pub before: i32,
	/// Space after in twips.
	pub after: i32,
}

impl PFmt {
	/// Left aligned, single spaced, no indents.
	pub const DEFAULT: PFmt = PFmt {
		align: 0,
		spacing: 0,
		flags: 0,
		list: 0,
		level: 0,
		tabset: NO_TABS,
		left: 0,
		right: 0,
		first: 0,
		before: 0,
		after: 0,
	};
}

impl Pod for PFmt {
	const SIZE: usize = 32;
	fn write(&self, w: &mut Writer<'_>) {
		w.u8(self.align);
		w.u8(self.spacing);
		w.u16(self.flags);
		w.u16(self.list);
		w.u8(self.level);
		w.u8(0);
		w.u16(self.tabset);
		w.u16(0);
		w.i32(self.left);
		w.i32(self.right);
		w.i32(self.first);
		w.i32(self.before);
		w.i32(self.after);
	}
	fn read(r: &mut Reader<'_>) -> Self {
		let align = r.u8();
		let spacing = r.u8();
		let flags = r.u16();
		let list = r.u16();
		let level = r.u8();
		r.u8();
		let tabset = r.u16();
		r.u16();
		PFmt {
			align,
			spacing,
			flags,
			list,
			level,
			tabset,
			left: r.i32(),
			right: r.i32(),
			first: r.i32(),
			before: r.i32(),
			after: r.i32(),
		}
	}
}

/// One tab stop.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct TabStop {
	/// Position in twips.
	pub pos: i32,
	/// 0 left, 1 centre, 2 right, 3 decimal.
	pub kind: u8,
	/// 0 none, 1 dots, 2 hyphens, 3 underline.
	pub leader: u8,
}

/// A set of up to 16 tab stops.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct TabSet {
	/// Stops in use.
	pub count: u8,
	/// The stops, in position order.
	pub stops: [TabStop; MAX_STOPS],
}

impl Pod for TabSet {
	const SIZE: usize = 1 + MAX_STOPS * 6;
	fn write(&self, w: &mut Writer<'_>) {
		w.u8(self.count);
		for s in &self.stops {
			w.i32(s.pos);
			w.u8(s.kind);
			w.u8(s.leader);
		}
	}
	fn read(r: &mut Reader<'_>) -> Self {
		let count = r.u8().min(MAX_STOPS as u8);
		let mut stops = [TabStop::default(); MAX_STOPS];
		for s in &mut stops {
			*s = TabStop {
				pos: r.i32(),
				kind: r.u8(),
				leader: r.u8(),
			};
		}
		TabSet { count, stops }
	}
}

/// One level of a list definition.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct ListLevel {
	/// Number style.
	pub style: u8,
	/// First number.
	pub start: u16,
	/// Indent in twips.
	pub indent: i32,
}

/// A list definition: nine levels.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct ListDef {
	/// The levels.
	pub levels: [ListLevel; 9],
}

impl Pod for ListDef {
	const SIZE: usize = 9 * 7;
	fn write(&self, w: &mut Writer<'_>) {
		for l in &self.levels {
			w.u8(l.style);
			w.u16(l.start);
			w.i32(l.indent);
		}
	}
	fn read(r: &mut Reader<'_>) -> Self {
		let mut levels = [ListLevel::default(); 9];
		for l in &mut levels {
			*l = ListLevel {
				style: r.u8(),
				start: r.u16(),
				indent: r.i32(),
			};
		}
		ListDef { levels }
	}
}

/// A table in which equal values share one index.
pub struct Intern<T: Pod + PartialEq, const CH: usize> {
	v: FarVec<T, CH>,
	limit: u32,
}

impl<T: Pod + PartialEq, const CH: usize> Intern<T, CH> {
	/// An empty table that holds at most `limit` distinct values.
	pub const fn new(limit: u32) -> Self {
		Intern {
			v: FarVec::new(),
			limit,
		}
	}

	/// Number of values.
	pub const fn len(&self) -> u32 {
		self.v.len()
	}

	/// True when empty.
	pub const fn is_empty(&self) -> bool {
		self.v.is_empty()
	}

	/// The value at `i`.
	pub fn get<S: ReadStore + ?Sized>(&self, s: &S, i: u16) -> Option<T> {
		self.v.get(s, u32::from(i))
	}

	/// The index of `x`, adding it when new.
	pub fn intern<S: Store + ?Sized>(&mut self, s: &mut S, x: T) -> Result<u16> {
		for i in 0..self.v.len() {
			if self.v.get(s, i) == Some(x) {
				return Ok(i as u16);
			}
		}
		if self.v.len() >= self.limit {
			return Err(Error::TooLarge);
		}
		self.v.reserve(s, 1)?;
		let i = self.v.len();
		self.v.push(s, x)?;
		Ok(i as u16)
	}

	/// Empties the table.
	pub fn clear<S: Store + ?Sized>(&mut self, s: &mut S) {
		self.v.clear(s);
	}
}

/// The largest paper size or margin a reader accepts, in twips (about 11,650 inches): three of them still add up in an `i32`.
pub const PAGE_LIMIT: i32 = 1 << 24;

/// Paper and margins, in twips.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct PageSetup {
	/// Paper width.
	pub paper_w: i32,
	/// Paper height.
	pub paper_h: i32,
	/// Left margin.
	pub margin_l: i32,
	/// Right margin.
	pub margin_r: i32,
	/// Top margin.
	pub margin_t: i32,
	/// Bottom margin.
	pub margin_b: i32,
	/// Header distance from the top edge.
	pub header_dist: i32,
	/// Footer distance from the bottom edge.
	pub footer_dist: i32,
	/// Number of the first page.
	pub first_page: u16,
	/// Bit 0: different first page.
	pub flags: u16,
	/// 0 inches, 1 centimetres.
	pub units: u8,
	/// 0 en-CA, 1 en-US, 2 en-GB.
	pub lang: u8,
}

impl PageSetup {
	/// The width between the margins, in twips; a damaged file's numbers saturate instead of overflowing.
	pub fn text_w(&self) -> i32 {
		self.paper_w
			.saturating_sub(self.margin_l)
			.saturating_sub(self.margin_r)
	}

	/// The height between the margins, in twips, saturating as `text_w` does.
	pub fn text_h(&self) -> i32 {
		self.paper_h
			.saturating_sub(self.margin_t)
			.saturating_sub(self.margin_b)
	}

	/// Letter paper, 1.25 inch side margins, 1 inch top and bottom, as the SPEC's defaults.
	pub const LETTER: PageSetup = PageSetup {
		paper_w: 12240,
		paper_h: 15840,
		margin_l: 1800,
		margin_r: 1800,
		margin_t: 1440,
		margin_b: 1440,
		header_dist: 720,
		footer_dist: 720,
		first_page: 1,
		flags: 0,
		units: 0,
		lang: 0,
	};
}

/// Which story of the document.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum StoryKind {
	/// The body.
	Body = 0,
	/// The header.
	Header = 1,
	/// The footer.
	Footer = 2,
	/// The first page's header.
	FirstHeader = 3,
	/// The first page's footer.
	FirstFooter = 4,
}

impl StoryKind {
	/// All five, in file order.
	pub const ALL: [StoryKind; 5] = [
		StoryKind::Body,
		StoryKind::Header,
		StoryKind::Footer,
		StoryKind::FirstHeader,
		StoryKind::FirstFooter,
	];

	/// From the file's story number.
	pub fn from_u8(n: u8) -> Option<Self> {
		Self::ALL.get(usize::from(n)).copied()
	}
}

/// The piece table with the capacities the document uses: up to about 175,000 pieces, a million characters in each of
/// the two buffers, and a history of about 32,000 records.
pub type StoryText = Text<64, 32, 16>;

/// Pictures in one document.
pub const MAX_PICTURES: u32 = 256;
/// The most file bytes one picture may hold.
pub const MAX_PICTURE_BYTES: u32 = 4 * 1024 * 1024;

/// A picture: its original file bytes live in the document's picture pool; this is the record of them.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Picture {
	/// 0 BMP, 1 PNG.
	pub kind: u8,
	/// Width in pixels.
	pub w: u16,
	/// Height in pixels.
	pub h: u16,
	/// Shown width in twips.
	pub shown_w: i32,
	/// Shown height in twips.
	pub shown_h: i32,
	/// Where the file bytes start in the pool.
	pub offset: u32,
	/// How many file bytes.
	pub len: u32,
}

impl Pod for Picture {
	const SIZE: usize = 24;
	fn write(&self, w: &mut Writer<'_>) {
		w.u8(self.kind);
		w.u8(0);
		w.u16(self.w);
		w.u16(self.h);
		w.u16(0);
		w.i32(self.shown_w);
		w.i32(self.shown_h);
		w.u32(self.offset);
		w.u32(self.len);
	}
	fn read(r: &mut Reader<'_>) -> Self {
		let kind = r.u8();
		r.u8();
		let (w, h) = (r.u16(), r.u16());
		r.u16();
		Picture {
			kind,
			w,
			h,
			shown_w: r.i32(),
			shown_h: r.i32(),
			offset: r.u32(),
			len: r.u32(),
		}
	}
}

/// A document.
pub struct Doc {
	/// The pictures' records, in the order the \`PICT\` chunks hold them.
	pub pictures: FarVec<Picture, 1>,
	/// The pictures' file bytes, one after another.
	pub pic_data: ByteBuf<128>,
	/// Paper and margins.
	pub page: PageSetup,
	/// Fonts.
	pub fonts: Intern<Font, 1>,
	/// Colours.
	pub colours: Intern<Colour, 1>,
	/// Character formats.
	pub cfmts: Intern<CFmt, 1>,
	/// Paragraph formats.
	pub pfmts: Intern<PFmt, 1>,
	/// Tab sets.
	pub tabsets: Intern<TabSet, 1>,
	/// List definitions.
	pub lists: Intern<ListDef, 1>,
	/// The body story.
	pub body: StoryText,
	/// The header story.
	pub header: StoryText,
	/// The footer story.
	pub footer: StoryText,
	/// The first page's header story.
	pub first_header: StoryText,
	/// The first page's footer story.
	pub first_footer: StoryText,
	/// What the document's own changes (the page setup, the pictures and a reset) add to [`Doc::revision`].
	rev: u32,
}

impl Default for Doc {
	fn default() -> Self {
		Self::new()
	}
}

impl Doc {
	/// A document with no tables yet. Use [`Doc::init`] for the defaults.
	pub const fn new() -> Self {
		Doc {
			pictures: FarVec::new(),
			pic_data: ByteBuf::new(crate::store::MAX_CHUNK),
			page: PageSetup::LETTER,
			fonts: Intern::new(MAX_FONTS),
			colours: Intern::new(MAX_COLOURS),
			cfmts: Intern::new(MAX_CFMTS),
			pfmts: Intern::new(MAX_PFMTS),
			tabsets: Intern::new(MAX_TABSETS),
			lists: Intern::new(MAX_LISTS),
			body: Text::new(),
			header: Text::new(),
			footer: Text::new(),
			first_header: Text::new(),
			first_footer: Text::new(),
			rev: 0,
		}
	}

	/// The document's revision: a number that changes whenever anything layout reads changes (the text and format runs of any story,
	/// the page setup, the pictures, a reset, a load, undo and redo) and never for a read. It is the wrapping sum of a counter in each
	/// story and one for the document itself, so it moves by one for each change and equal values mean equal content. The interned
	/// tables (fonts, colours, formats) are append-only and change nothing until a story's run names a new entry, which moves the
	/// story's counter, so they are not counted. Anything that changes `page` must go through [`Doc::set_page`].
	pub fn revision(&self) -> u32 {
		self.rev
			.wrapping_add(self.body.revision())
			.wrapping_add(self.header.revision())
			.wrapping_add(self.footer.revision())
			.wrapping_add(self.first_header.revision())
			.wrapping_add(self.first_footer.revision())
	}

	fn touch(&mut self) {
		self.rev = self.rev.wrapping_add(1);
	}

	/// Replaces the page setup and counts it in [`Doc::revision`].
	pub fn set_page(&mut self, page: PageSetup) {
		self.touch();
		self.page = page;
	}

	/// Resets to an empty document: Times New Roman 12 point, the default paragraph format and one empty paragraph.
	pub fn init<S: Store + ?Sized>(&mut self, s: &mut S) -> Result<()> {
		self.reset(s);
		self.fonts.intern(s, Font::new(b"Times New Roman", 1))?;
		self.colours.intern(s, Colour { r: 0, g: 0, b: 0 })?;
		self.cfmts.intern(
			s,
			CFmt {
				font: 0,
				size: 24,
				colour: 0,
				vpos: 0,
				flags: 0,
			},
		)?;
		self.pfmts.intern(s, PFmt::DEFAULT)?;
		self.body.load(s, b"\r", 0, 0)
	}

	/// Adds a picture whose file bytes `read` supplies (kind 0 BMP, 1 PNG; `w` by `h` pixels), shown at 96 dots per inch or scaled
	/// down to `max_w` twips wide. `read` fills a buffer and returns how many bytes it put there, zero at the end. Returns the index.
	pub fn add_picture<S: Store + ?Sized>(
		&mut self,
		s: &mut S,
		kind: u8,
		w: u16,
		h: u16,
		max_w: i32,
		mut read: impl FnMut(&mut [u8]) -> usize,
	) -> Result<u16> {
		if self.pictures.len() >= MAX_PICTURES {
			return Err(Error::TooLarge);
		}
		self.pictures.reserve(s, 1)?;
		self.touch();
		let offset = self.pic_data.len();
		let mut buf = [0u8; 512];
		let mut len = 0u32;
		loop {
			let n = read(&mut buf);
			if n == 0 {
				break;
			}
			len += n as u32;
			let r = if len > MAX_PICTURE_BYTES {
				Err(Error::TooLarge)
			} else {
				self.pic_data
					.append(s, buf.get(..n).unwrap_or(&[]))
					.map(|_| ())
			};
			if let Err(e) = r {
				// The bytes taken so far go back, so a refused picture leaves the document as it was.
				self.pic_data.truncate(s, offset);
				return Err(e);
			}
		}
		let (mut sw, mut sh) = (i32::from(w) * 15, i32::from(h) * 15);
		if sw > max_w && sw > 0 {
			sh = (i64::from(sh) * i64::from(max_w) / i64::from(sw)) as i32;
			sw = max_w;
		}
		let idx = self.pictures.len();
		self.pictures.push(
			s,
			Picture {
				kind,
				w,
				h,
				shown_w: sw.max(144),
				shown_h: sh.max(144),
				offset,
				len,
			},
		)?;
		Ok(idx as u16)
	}

	/// Copies picture `idx`'s file bytes from `pos` into `out`.
	pub fn picture_bytes<S: ReadStore + ?Sized>(
		&self,
		s: &S,
		idx: u16,
		pos: u32,
		out: &mut [u8],
	) -> usize {
		let Some(p) = self.pictures.get(s, u32::from(idx)) else {
			return 0;
		};
		let n = (p.len.saturating_sub(pos) as usize).min(out.len());
		if let Some(o) = out.get_mut(..n) {
			self.pic_data.read(s, p.offset + pos, o);
		}
		n
	}

	/// Frees everything.
	pub fn reset<S: Store + ?Sized>(&mut self, s: &mut S) {
		self.touch();
		self.page = PageSetup::LETTER;
		self.fonts.clear(s);
		self.colours.clear(s);
		self.cfmts.clear(s);
		self.pfmts.clear(s);
		self.tabsets.clear(s);
		self.lists.clear(s);
		self.pictures.clear(s);
		self.pic_data.clear(s);
		self.body.clear(s);
		self.header.clear(s);
		self.footer.clear(s);
		self.first_header.clear(s);
		self.first_footer.clear(s);
	}

	/// A story by kind.
	pub fn story(&self, k: StoryKind) -> &StoryText {
		match k {
			StoryKind::Body => &self.body,
			StoryKind::Header => &self.header,
			StoryKind::Footer => &self.footer,
			StoryKind::FirstHeader => &self.first_header,
			StoryKind::FirstFooter => &self.first_footer,
		}
	}

	/// A story by kind, for editing.
	pub fn story_mut(&mut self, k: StoryKind) -> &mut StoryText {
		match k {
			StoryKind::Body => &mut self.body,
			StoryKind::Header => &mut self.header,
			StoryKind::Footer => &mut self.footer,
			StoryKind::FirstHeader => &mut self.first_header,
			StoryKind::FirstFooter => &mut self.first_footer,
		}
	}
}
