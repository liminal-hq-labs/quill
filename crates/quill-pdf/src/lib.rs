// quill-pdf: a PDF 1.4 writer for Quill's Print to PDF. Pages hold selectable text in the standard 14 fonts, placed where the
// layout put every character, filled rectangles for rules and underlines, and pictures as Flate-compressed RGB images.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The writer knows nothing of documents or windows: a shell (or `quill-pdfdoc`, which walks a laid-out document) says "this text
// at this baseline with these advances" in the device units of one resolution, and the writer turns that into content streams.
// Text is shown with the standard font nearest the face and each character is nudged (`TJ`) to the layout's own position, so
// the page looks as it was laid out and a viewer still finds the words, in order, to select, search and copy. Output is
// byte-for-byte repeatable: no clock is read, and the dates are whatever the caller passes.

#![no_std]
#![forbid(unsafe_code)]
#![deny(
	clippy::unwrap_used,
	clippy::expect_used,
	clippy::panic,
	clippy::indexing_slicing
)]

extern crate alloc;
#[cfg(test)]
extern crate std;

pub mod deflate;
pub mod font;
mod widths;

use alloc::vec::Vec;

pub use font::{Family, Font, FONT_COUNT};

/// A colour: red, green and blue, 0 to 255.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
	/// Black, which is also where every page starts.
	pub const BLACK: Rgb = Rgb(0, 0, 0);
}

/// What a document says about itself.
#[derive(Copy, Clone, Debug, Default)]
pub struct Info<'a> {
	/// The title, in Windows-1252 (empty for none).
	pub title: &'a [u8],
	/// When it was made, as `YYYYMMDDHHMMSS` (UTC is assumed), or `None` to leave the date out so the bytes do not change.
	pub created: Option<&'a [u8]>,
}

/// One compressed picture of a page.
struct Image {
	width: u32,
	height: u32,
	data: Vec<u8>,
}

/// A run of text to show: Windows-1252 bytes in one font, with the layout's advance of each, in the page's device units.
#[derive(Copy, Clone, Debug)]
pub struct Run<'a> {
	/// The font.
	pub font: Font,
	/// The size in half points.
	pub size_half: u16,
	/// The colour.
	pub colour: Rgb,
	/// The left edge of the first character.
	pub x: i32,
	/// The baseline.
	pub baseline: i32,
	/// The characters.
	pub text: &'a [u8],
	/// The layout's width of each character (a missing one is 0).
	pub advances: &'a [i32],
}

/// A picture to place: `px_w` by `px_h` pixels of `rgb` (three bytes a pixel, top row first) in the rectangle with its top left at
/// (`x`, `y`) and size `w` by `h`, in the page's device units.
#[derive(Copy, Clone, Debug)]
pub struct Picture<'a> {
	/// Left edge.
	pub x: i32,
	/// Top edge.
	pub y: i32,
	/// Width.
	pub w: i32,
	/// Height.
	pub h: i32,
	/// Pixels across.
	pub px_w: u32,
	/// Pixels down.
	pub px_h: u32,
	/// The pixels.
	pub rgb: &'a [u8],
}

/// A page being drawn. Positions and sizes are in device units of `dpi` units to the inch, from the top left of the page.
pub struct Page {
	dpi: u32,
	width: i32,
	height: i32,
	content: Vec<u8>,
	fonts: u16,
	images: Vec<Image>,
	fill: Rgb,
}

/// Millipoints (thousandths of a point) of `v` device units at `dpi`, rounded half away from zero.
fn milli(v: i32, dpi: u32) -> i64 {
	let n = i64::from(v) * 72_000;
	let d = i64::from(dpi.max(1));
	if n >= 0 {
		(n + d / 2) / d
	} else {
		-((-n + d / 2) / d)
	}
}

/// Writes a number of thousandths as a decimal with the trailing zeros left off.
fn put_milli(out: &mut Vec<u8>, v: i64) {
	if v < 0 {
		out.push(b'-');
	}
	let v = v.unsigned_abs();
	put_uint(out, v / 1000);
	let mut frac = v % 1000;
	if frac != 0 {
		out.push(b'.');
		let mut scale = 100;
		while frac != 0 {
			out.push(b'0' + (frac / scale) as u8);
			frac %= scale;
			scale /= 10;
		}
	}
}

fn put_uint(out: &mut Vec<u8>, v: u64) {
	let mut buf = [0u8; 20];
	let mut at = buf.len();
	let mut rest = v;
	loop {
		at -= 1;
		if let Some(slot) = buf.get_mut(at) {
			*slot = b'0' + (rest % 10) as u8;
		}
		rest /= 10;
		if rest == 0 {
			break;
		}
	}
	out.extend_from_slice(buf.get(at..).unwrap_or(&[]));
}

fn put_int(out: &mut Vec<u8>, v: i64) {
	if v < 0 {
		out.push(b'-');
	}
	put_uint(out, v.unsigned_abs());
}

/// A colour component, 0 to 255, as the PDF's 0 to 1.
fn put_component(out: &mut Vec<u8>, c: u8) {
	put_milli(out, (i64::from(c) * 1000 + 127) / 255);
}

impl Page {
	/// A blank page `width` by `height` device units, at `dpi` units to the inch.
	pub fn new(width: i32, height: i32, dpi: u16) -> Page {
		Page {
			dpi: u32::from(dpi.max(1)),
			width: width.max(1),
			height: height.max(1),
			content: Vec::new(),
			fonts: 0,
			images: Vec::new(),
			fill: Rgb::BLACK,
		}
	}

	fn set_fill(&mut self, c: Rgb) {
		if c == self.fill {
			return;
		}
		for part in [c.0, c.1, c.2] {
			put_component(&mut self.content, part);
			self.content.push(b' ');
		}
		self.content.extend_from_slice(b"rg\n");
		self.fill = c;
	}

	/// Shows a run of text. Every character is placed at the sum of the layout's advances before it, whatever the font's own widths
	/// are, and bytes below 32 draw nothing but still take their advance.
	pub fn text(&mut self, run: &Run<'_>) {
		let Run {
			font,
			size_half,
			colour,
			x,
			baseline,
			text,
			advances,
		} = *run;
		if text.iter().all(|b| *b < 32) || size_half == 0 {
			return;
		}
		self.set_fill(colour);
		self.fonts |= 1 << font.index();
		let c = &mut self.content;
		c.extend_from_slice(b"BT /F");
		put_uint(c, font.index() as u64);
		c.push(b' ');
		put_milli(c, i64::from(size_half) * 500);
		c.extend_from_slice(b" Tf ");
		put_milli(c, milli(x, self.dpi));
		c.push(b' ');
		put_milli(c, milli(self.height.saturating_sub(baseline), self.dpi));
		c.extend_from_slice(b" Td [");
		// Thousandths of an em that one device unit is worth at this size.
		let per_unit_num = 144_000i64;
		let per_unit_den = i64::from(self.dpi) * i64::from(size_half);
		let (mut cumulative, mut nominal, mut shifted) = (0i64, 0i64, 0i64);
		let mut open = false;
		for (i, &byte) in text.iter().enumerate() {
			let target =
				(cumulative * per_unit_num * 2 + per_unit_den).div_euclid(per_unit_den * 2);
			cumulative += i64::from(advances.get(i).copied().unwrap_or(0));
			if byte < 32 {
				continue;
			}
			let want_shift = nominal - target;
			if want_shift != shifted {
				if open {
					c.extend_from_slice(b")");
					open = false;
				}
				put_int(c, want_shift - shifted);
				shifted = want_shift;
			}
			if !open {
				c.push(b'(');
				open = true;
			}
			match byte {
				b'(' | b')' | b'\\' => {
					c.push(b'\\');
					c.push(byte);
				}
				0x20..=0x7E => c.push(byte),
				_ => {
					c.push(b'\\');
					c.push(b'0' + (byte >> 6));
					c.push(b'0' + ((byte >> 3) & 7));
					c.push(b'0' + (byte & 7));
				}
			}
			nominal += i64::from(font.width(byte));
		}
		if open {
			c.push(b')');
		}
		c.extend_from_slice(b"] TJ ET\n");
	}

	/// Fills the rectangle with its top left at (`x`, `y`) and size `w` by `h`.
	pub fn rect(&mut self, colour: Rgb, x: i32, y: i32, w: i32, h: i32) {
		if w <= 0 || h <= 0 {
			return;
		}
		self.set_fill(colour);
		let c = &mut self.content;
		put_milli(c, milli(x, self.dpi));
		c.push(b' ');
		put_milli(
			c,
			milli(self.height.saturating_sub(y.saturating_add(h)), self.dpi),
		);
		c.push(b' ');
		put_milli(c, milli(w, self.dpi));
		c.push(b' ');
		put_milli(c, milli(h, self.dpi));
		c.extend_from_slice(b" re f\n");
	}

	/// Places a picture in its rectangle. Returns false, and draws nothing, when its bytes are not the size it says.
	pub fn image(&mut self, pic: &Picture<'_>) -> bool {
		let Picture {
			x,
			y,
			w,
			h,
			px_w,
			px_h,
			rgb,
		} = *pic;
		let Some(len) = (px_w as usize)
			.checked_mul(px_h as usize)
			.and_then(|n| n.checked_mul(3))
		else {
			return false;
		};
		if w <= 0 || h <= 0 || px_w == 0 || px_h == 0 || rgb.len() != len {
			return false;
		}
		let mut data = Vec::new();
		deflate::zlib(rgb, &mut data);
		let n = self.images.len();
		self.images.push(Image {
			width: px_w,
			height: px_h,
			data,
		});
		let c = &mut self.content;
		c.extend_from_slice(b"q ");
		put_milli(c, milli(w, self.dpi));
		c.extend_from_slice(b" 0 0 ");
		put_milli(c, milli(h, self.dpi));
		c.push(b' ');
		put_milli(c, milli(x, self.dpi));
		c.push(b' ');
		put_milli(
			c,
			milli(self.height.saturating_sub(y.saturating_add(h)), self.dpi),
		);
		c.extend_from_slice(b" cm /Im");
		put_uint(c, n as u64);
		c.extend_from_slice(b" Do Q\n");
		true
	}

	/// The size of the page in device units.
	pub fn size(&self) -> (i32, i32) {
		(self.width, self.height)
	}

	/// The content stream so far, for tests.
	#[doc(hidden)]
	pub fn content_for_tests(&self) -> &[u8] {
		&self.content
	}
}

/// A finished page: its content stream compressed, ready to write.
struct Done {
	width: i64,
	height: i64,
	content: Vec<u8>,
	fonts: u16,
	images: Vec<Image>,
}

/// A document: pages added in order, written out in one piece by `finish`.
pub struct Pdf {
	title: Vec<u8>,
	created: Option<Vec<u8>>,
	pages: Vec<Done>,
}

/// The Unicode values of Windows-1252 bytes 0x80 to 0x9F.
const HIGH: [u16; 32] = [
	0x20AC, 0x2022, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, 0x02C6, 0x2030, 0x0160, 0x2039,
	0x0152, 0x2022, 0x017D, 0x2022, 0x2022, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013, 0x2014,
	0x02DC, 0x2122, 0x0161, 0x203A, 0x0153, 0x2022, 0x017E, 0x0178,
];

fn put_hex(out: &mut Vec<u8>, v: u16) {
	for shift in [12, 8, 4, 0] {
		let nibble = ((v >> shift) & 15) as u8;
		out.push(if nibble < 10 {
			b'0' + nibble
		} else {
			b'A' + nibble - 10
		});
	}
}

struct Writer {
	out: Vec<u8>,
	offsets: Vec<usize>,
}

impl Writer {
	fn begin(&mut self) -> u32 {
		self.offsets.push(self.out.len());
		let n = self.offsets.len() as u32;
		put_uint(&mut self.out, u64::from(n));
		self.out.extend_from_slice(b" 0 obj\n");
		n
	}

	fn end(&mut self) {
		self.out.extend_from_slice(b"endobj\n");
	}

	fn stream(&mut self, dict: &[u8], data: &[u8]) {
		self.begin();
		self.out.extend_from_slice(b"<< ");
		self.out.extend_from_slice(dict);
		self.out
			.extend_from_slice(b" /Filter /FlateDecode /Length ");
		put_uint(&mut self.out, data.len() as u64);
		self.out.extend_from_slice(b" >>\nstream\n");
		self.out.extend_from_slice(data);
		self.out.extend_from_slice(b"\nendstream\n");
		self.end();
	}
}

fn put_ref(out: &mut Vec<u8>, n: u32) {
	put_uint(out, u64::from(n));
	out.extend_from_slice(b" 0 R");
}

impl Pdf {
	/// A document with no pages.
	pub fn new(info: &Info<'_>) -> Pdf {
		Pdf {
			title: info.title.to_vec(),
			created: info.created.map(<[u8]>::to_vec),
			pages: Vec::new(),
		}
	}

	/// The number of pages added so far.
	pub fn page_count(&self) -> usize {
		self.pages.len()
	}

	/// Adds a finished page.
	pub fn add_page(&mut self, page: Page) {
		let mut content = Vec::new();
		deflate::zlib(&page.content, &mut content);
		self.pages.push(Done {
			width: milli(page.width, page.dpi),
			height: milli(page.height, page.dpi),
			content,
			fonts: page.fonts,
			images: page.images,
		});
	}

	/// The finished file. A document with no pages gets one blank letter page, since a file with none is not valid.
	pub fn finish(mut self) -> Vec<u8> {
		if self.pages.is_empty() {
			self.add_page(Page::new(612, 792, 72));
		}
		let used = self.pages.iter().fold(0u16, |m, p| m | p.fonts);
		let mut w = Writer {
			out: Vec::new(),
			offsets: Vec::new(),
		};
		w.out.extend_from_slice(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");
		// 1 the catalogue, 2 the page tree, 3 the information, then the fonts in use, then each page's objects. Page objects come
		// after all of them in numbering, so the tree can name them before they are written.
		let font_count = used.count_ones();
		let first_page_obj = 4 + font_count;
		// Each page takes a page object, a content stream and its images.
		let mut page_objs: Vec<u32> = Vec::new();
		let mut next = first_page_obj;
		for p in &self.pages {
			page_objs.push(next);
			next += 2 + p.images.len() as u32;
		}
		w.begin();
		w.out.extend_from_slice(
			b"<< /Type /Catalog /Pages 2 0 R /ViewerPreferences << /DisplayDocTitle true >> >>\n",
		);
		w.end();
		w.begin();
		w.out.extend_from_slice(b"<< /Type /Pages /Count ");
		put_uint(&mut w.out, self.pages.len() as u64);
		w.out.extend_from_slice(b" /Kids [");
		for (i, n) in page_objs.iter().enumerate() {
			if i > 0 {
				w.out.push(b' ');
			}
			put_ref(&mut w.out, *n);
		}
		w.out.extend_from_slice(b"] >>\n");
		w.end();
		w.begin();
		w.out.extend_from_slice(b"<< /Producer (Quill)");
		if !self.title.is_empty() {
			w.out.extend_from_slice(b" /Title <FEFF");
			for &b in &self.title {
				let unit = match b {
					0x80..=0x9F => HIGH.get(usize::from(b - 0x80)).copied().unwrap_or(0x2022),
					_ => u16::from(b),
				};
				put_hex(&mut w.out, unit);
			}
			w.out.push(b'>');
		}
		if let Some(created) = &self.created {
			for key in [&b" /CreationDate"[..], b" /ModDate"] {
				w.out.extend_from_slice(key);
				w.out.extend_from_slice(b" (D:");
				w.out.extend_from_slice(created);
				w.out.extend_from_slice(b"Z)");
			}
		}
		w.out.extend_from_slice(b" >>\n");
		w.end();
		let mut font_objs = [0u32; FONT_COUNT];
		for index in 0..FONT_COUNT {
			if used & (1 << index) == 0 {
				continue;
			}
			let n = w.begin();
			if let Some(slot) = font_objs.get_mut(index) {
				*slot = n;
			}
			w.out
				.extend_from_slice(b"<< /Type /Font /Subtype /Type1 /BaseFont /");
			w.out
				.extend_from_slice(Font::from_index(index).base_name().as_bytes());
			w.out.extend_from_slice(b" /Encoding /WinAnsiEncoding >>\n");
			w.end();
		}
		for (p, &page_obj) in self.pages.iter().zip(page_objs.iter()) {
			w.begin();
			w.out
				.extend_from_slice(b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 ");
			put_milli(&mut w.out, p.width);
			w.out.push(b' ');
			put_milli(&mut w.out, p.height);
			w.out.extend_from_slice(b"] /Contents ");
			put_ref(&mut w.out, page_obj + 1);
			w.out
				.extend_from_slice(b" /Resources << /ProcSet [/PDF /Text /ImageC]");
			if p.fonts != 0 {
				w.out.extend_from_slice(b" /Font <<");
				for (index, &n) in font_objs.iter().enumerate() {
					if p.fonts & (1 << index) != 0 {
						w.out.extend_from_slice(b" /F");
						put_uint(&mut w.out, index as u64);
						w.out.push(b' ');
						put_ref(&mut w.out, n);
					}
				}
				w.out.extend_from_slice(b" >>");
			}
			if !p.images.is_empty() {
				w.out.extend_from_slice(b" /XObject <<");
				for i in 0..p.images.len() as u32 {
					w.out.extend_from_slice(b" /Im");
					put_uint(&mut w.out, u64::from(i));
					w.out.push(b' ');
					put_ref(&mut w.out, page_obj + 2 + i);
				}
				w.out.extend_from_slice(b" >>");
			}
			w.out.extend_from_slice(b" >> >>\n");
			w.end();
			w.stream(b"", &p.content);
			for img in &p.images {
				let mut dict = Vec::new();
				dict.extend_from_slice(b"/Type /XObject /Subtype /Image /Width ");
				put_uint(&mut dict, u64::from(img.width));
				dict.extend_from_slice(b" /Height ");
				put_uint(&mut dict, u64::from(img.height));
				dict.extend_from_slice(b" /ColorSpace /DeviceRGB /BitsPerComponent 8");
				w.stream(&dict, &img.data);
			}
		}
		let xref = w.out.len();
		w.out.extend_from_slice(b"xref\n0 ");
		put_uint(&mut w.out, w.offsets.len() as u64 + 1);
		w.out.extend_from_slice(b"\n0000000000 65535 f \n");
		let offsets = core::mem::take(&mut w.offsets);
		for off in &offsets {
			let mut digits = [b'0'; 10];
			let mut rest = *off as u64;
			for slot in digits.iter_mut().rev() {
				*slot = b'0' + (rest % 10) as u8;
				rest /= 10;
			}
			w.out.extend_from_slice(&digits);
			w.out.extend_from_slice(b" 00000 n \n");
		}
		w.out.extend_from_slice(b"trailer\n<< /Size ");
		put_uint(&mut w.out, offsets.len() as u64 + 1);
		w.out
			.extend_from_slice(b" /Root 1 0 R /Info 3 0 R >>\nstartxref\n");
		put_uint(&mut w.out, xref as u64);
		w.out.extend_from_slice(b"\n%%EOF\n");
		w.out
	}
}
