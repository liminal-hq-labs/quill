// Plain-text printing: the document as pages of text with form feeds, headers and footers, for the DOS build and for tests.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use quill_core::doc::{PFmt, StoryKind};
use quill_core::layout::{to_units, Measure};
use quill_core::store::{ReadStore, Store};
use quill_formats::io::Sink;
use quill_grid::ansi_to_cp437;

use crate::{App, Files};

const WIDTH: usize = 132;
const MAX_ROWS: usize = 70;

fn put(row: &mut [u8; WIDTH], x: usize, b: u8) {
	if let Some(slot) = row.get_mut(x) {
		*slot = b;
	}
}

/// Writes the page's rows up to `last`, then a form feed, and clears the grid for the next page.
fn flush_page<K: Sink>(k: &mut K, grid: &mut [[u8; WIDTH]; MAX_ROWS], last: &mut usize) -> bool {
	let mut ok = true;
	for row in grid.iter().take(*last) {
		ok &= emit(k, row);
	}
	ok &= k.write(b"\x0c");
	for row in grid.iter_mut() {
		row.fill(b' ');
	}
	*last = 0;
	ok
}

fn emit<K: Sink>(k: &mut K, row: &[u8; WIDTH]) -> bool {
	let end = row.iter().rposition(|b| *b != b' ').map_or(0, |i| i + 1);
	k.write(row.get(..end).unwrap_or(&[])) && k.write(b"\r\n")
}

impl<M: Measure + Clone> App<M> {
	/// Sets the date the Date field prints as; the platform knows the clock, the core does not.
	pub fn set_print_date(&mut self, text: &str) {
		self.date.set(text);
	}

	/// Sets the time the Time field prints as.
	pub fn set_print_time(&mut self, text: &str) {
		self.time.set(text);
	}

	/// The text a field object shows when printed: 1 the page number, 2 the number of pages, 3 the date, 4 the file name, 5 the time.
	/// Returns how
	/// many bytes of `out` it filled.
	pub fn field_text(&self, kind: u32, page: u32, pages: u32, out: &mut [u8; 32]) -> usize {
		let mut digits = [0u8; 10];
		let src: &[u8] = match kind {
			1 | 2 => {
				let mut v = if kind == 1 { page } else { pages };
				let mut i = digits.len();
				loop {
					i -= 1;
					if let Some(d) = digits.get_mut(i) {
						*d = b'0' + (v % 10) as u8;
					}
					v /= 10;
					if v == 0 {
						break;
					}
				}
				digits.get(i..).unwrap_or(&[])
			}
			3 => self.date.bytes(),
			5 => self.time.bytes(),
			4 if self.name_len == 0 => b"(Untitled)",
			4 => self.name(),
			_ => &[],
		};
		let n = src.len().min(out.len());
		if let (Some(d), Some(sv)) = (out.get_mut(..n), src.get(..n)) {
			d.copy_from_slice(sv);
		}
		n
	}

	/// Prints the body as text to `name` (a file, or `PRN` for the printer on DOS): each page is its lines at the SPEC's 10 pitch
	/// and 6 lines to the inch, with the header and footer in the margins, and a form feed after every page.
	#[inline(never)]
	pub fn print_text<S: Store + ?Sized, F: Files>(
		&mut self,
		s: &mut S,
		f: &mut F,
		name: &[u8],
	) -> bool {
		if self.view != 0 {
			self.set_view(s, 0);
		}
		let Some(mut k) = f.create(name) else {
			return false;
		};
		let ok = self.print_pages(s, &mut k);
		f.close(k) && ok
	}

	/// Prints the pages in one pass over the layout: lines come in order, so a page is finished when the next one's first line arrives,
	/// and list labels (which count across pages) are worked out as the lines go by.
	#[inline(never)]
	fn print_pages<S: ReadStore + ?Sized, K: Sink>(&self, s: &S, k: &mut K) -> bool {
		let page = self.doc.page;
		let top = to_units(page.margin_t, 6) as usize;
		let left = to_units(page.margin_l, 10) as usize;
		let rows = (to_units(page.paper_h, 6) as usize).min(MAX_ROWS);
		let head_row = to_units(page.header_dist, 6) as usize;
		let foot_row = rows.saturating_sub(to_units(page.footer_dist, 6) as usize + 1);
		let text_w = to_units(page.paper_w - page.margin_l - page.margin_r, 10) as usize;
		let mut counters: crate::ListCounters = [[0; 9]; 4];
		let mut grid = [[b' '; WIDTH]; MAX_ROWS];
		let mut last = 0usize;
		let mut cur = 0u32;
		let mut ok = true;
		let begin = |p: u32, grid: &mut [[u8; WIDTH]; MAX_ROWS], last: &mut usize| {
			let first = p == 1 && page.flags & 1 != 0;
			let (hk, fk) = if first {
				(StoryKind::FirstHeader, StoryKind::FirstFooter)
			} else {
				(StoryKind::Header, StoryKind::Footer)
			};
			if let (Some(r), Some(row)) = (
				self.margin_row(s, hk, p, text_w, left),
				grid.get_mut(head_row),
			) {
				*row = r;
				*last = (*last).max(head_row + 1);
			}
			if let (Some(r), Some(row)) = (
				self.margin_row(s, fk, p, text_w, left),
				grid.get_mut(foot_row),
			) {
				*row = r;
				*last = (*last).max(foot_row + 1);
			}
		};
		self.lines(s, |_, l| {
			if l.page != cur {
				if cur != 0 {
					ok &= flush_page(k, &mut grid, &mut last);
				}
				cur = l.page;
				begin(cur, &mut grid, &mut last);
			}
			// The label comes first whether or not the line fits the sheet: the counters follow every line.
			let label = crate::line_label(self, s, l, &mut counters, 10);
			let y = top + l.y as usize;
			let Some(row) = grid.get_mut(y) else {
				return;
			};
			last = last.max(y + 1);
			if let Some((lab, n, x)) = label {
				for (i, b) in lab.iter().take(n).enumerate() {
					put(
						row,
						left + x + i,
						ansi_to_cp437(if *b == 0x07 { 0x95 } else { *b }),
					);
				}
			}
			self.walk(s, l, |_, c, x, w, leader| {
				let xx = left + x as usize;
				if c == 0x09 {
					let fill = match leader {
						1 => b'.',
						2 => b'-',
						3 => b'_',
						_ => b' ',
					};
					for i in 0..w as usize {
						put(row, xx + i, fill);
					}
				} else if c != 0x01 {
					put(row, xx, ansi_to_cp437(c));
				}
			});
		});
		if cur == 0 {
			// No lines at all: one page that holds the header and footer.
			begin(1, &mut grid, &mut last);
		}
		ok & flush_page(k, &mut grid, &mut last)
	}

	/// One header or footer row: its first paragraph, fields filled in, aligned inside the text width.
	#[inline(never)]
	fn margin_row<S: ReadStore + ?Sized>(
		&self,
		s: &S,
		kind: StoryKind,
		page: u32,
		text_w: usize,
		left: usize,
	) -> Option<[u8; WIDTH]> {
		let t = self.doc.story(kind);
		if t.is_empty() {
			return None;
		}
		let mut row = [b' '; WIDTH];
		let mut out = [0u8; 100];
		let mut buf = [0u8; 100];
		let len = t.read(s, 0, &mut buf);
		let mut n;
		// Fields are objects in the text: rebuild with their values.
		n = 0;
		let mut at = 0u32;
		while at < len as u32 && n < out.len() {
			let c = t.read_byte(s, at)?;
			if c == 0x0D || c == 0x0B {
				break;
			}
			if c == 0x01 {
				let v: &[u8] = match t.object_at(s, at).map(|v| v >> 16) {
					Some(2) => b"1",
					Some(3) => self.date.bytes(),
					Some(5) => self.time.bytes(),
					Some(4) => {
						if self.name_len == 0 {
							b"(Untitled)"
						} else {
							self.name()
						}
					}
					_ => b"",
				};
				if t.object_at(s, at).map(|v| v >> 16) == Some(1) {
					let mut digits = [0u8; 10];
					let mut i = digits.len();
					let mut v = page;
					loop {
						i -= 1;
						if let Some(d) = digits.get_mut(i) {
							*d = b'0' + (v % 10) as u8;
						}
						v /= 10;
						if v == 0 {
							break;
						}
					}
					for b in digits.get(i..).unwrap_or(&[]) {
						if let Some(slot) = out.get_mut(n) {
							*slot = *b;
							n += 1;
						}
					}
				} else {
					for b in v {
						if let Some(slot) = out.get_mut(n) {
							*slot = *b;
							n += 1;
						}
					}
				}
			} else if let Some(slot) = out.get_mut(n) {
				*slot = ansi_to_cp437(c);
				n += 1;
			}
			at += 1;
		}
		let mark = (0..t.len()).find(|p| t.read_byte(s, *p) == Some(0x0D));
		let pf = mark
			.and_then(|m| t.pfmt_at(s, m))
			.and_then(|i| self.doc.pfmts.get(s, i))
			.unwrap_or(PFmt::DEFAULT);
		let shift = match pf.align {
			1 => text_w.saturating_sub(n) / 2,
			2 => text_w.saturating_sub(n),
			_ => 0,
		};
		for (i, b) in out.iter().take(n).enumerate() {
			put(&mut row, left + shift + i, *b);
		}
		Some(row)
	}
}
