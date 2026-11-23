// The text-mode painter: the menu bar, the page, the status line, the menus and the prompts, into a Grid.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use quill_core::store::Store;
use quill_grid::{ansi_to_cp437, attr, Grid, COLS};

use quill_core::doc::cflag;

use crate::{App, Command, Mode, Prompt, Scheme};

/// Rows of text between the menu bar and the status line on an 80 by 25 screen.
pub const TEXT_ROWS: usize = 23;
/// Menus on the bar.
pub const MENU_COUNT: u8 = 8;

const MENU_BAR: &[u8] = b" File  Edit  Insert  Find  Character  Paragraph  Document  Help";
const MENU_COLS: [usize; 8] = [1, 7, 13, 21, 27, 38, 49, 59];
const MENU_TITLES: [&[u8]; 8] = [
	b"File",
	b"Edit",
	b"Insert",
	b"Find",
	b"Character",
	b"Paragraph",
	b"Document",
	b"Help",
];
const SELECTED: u8 = 0x07;

/// A pull-down item.
pub struct Item {
	/// The text, with its ellipsis.
	pub label: &'static str,
	/// The accelerator shown at the right.
	pub accel: &'static str,
	/// The command it runs.
	pub command: Command,
}

const fn item(label: &'static str, accel: &'static str, command: Command) -> Item {
	Item {
		label,
		accel,
		command,
	}
}

const FIND: [Item; 3] = [
	item("Find...", "Ctrl+F", Command::Find),
	item("Find Next", "F3", Command::FindNext),
	item("Replace...", "Ctrl+H", Command::Replace),
];
const INSERT: [Item; 4] = [
	item("Page Number", "", Command::Field(1)),
	item("Page Count", "", Command::Field(2)),
	item("Date", "", Command::Field(3)),
	item("File Name", "", Command::Field(4)),
];
const DOCUMENT: [Item; 6] = [
	item("Header", "", Command::View(1)),
	item("Footer", "", Command::View(2)),
	item("First Page Header", "", Command::View(3)),
	item("First Page Footer", "", Command::View(4)),
	item("Different First Page", "", Command::FirstPage),
	item("Return to Document", "", Command::View(0)),
];
const FILE: [Item; 5] = [
	item("New", "Ctrl+N", Command::New),
	item("Open...", "Ctrl+O", Command::Open),
	item("Save", "Ctrl+S", Command::Save),
	item("Save As...", "", Command::SaveAs),
	item("Exit", "Ctrl+Q", Command::Exit),
];
const EDIT: [Item; 2] = [
	item("Undo", "Ctrl+Z", Command::Undo),
	item("Redo", "Ctrl+Y", Command::Redo),
];
const CHARACTER: [Item; 7] = [
	item("Regular", "F5", Command::Regular),
	item("Bold", "Ctrl+B", Command::Bold),
	item("Italic", "Ctrl+I", Command::Italic),
	item("Underline", "Ctrl+U", Command::Underline),
	item("Strikethrough", "", Command::Strike),
	item("Superscript", "", Command::Super),
	item("Subscript", "", Command::Sub),
];
const PARAGRAPH: [Item; 11] = [
	item("Left", "Ctrl+L", Command::Align(0)),
	item("Centred", "Ctrl+E", Command::Align(1)),
	item("Right", "Ctrl+R", Command::Align(2)),
	item("Justified", "Ctrl+J", Command::Align(3)),
	item("Bullets", "", Command::Bullets),
	item("Numbering", "", Command::Numbering),
	item("Increase Level", "", Command::LevelUp),
	item("Decrease Level", "", Command::LevelDown),
	item("Single Space", "", Command::Spacing(0)),
	item("1\u{bd} Space", "", Command::Spacing(1)),
	item("Double Space", "", Command::Spacing(2)),
];

pub(crate) fn menu_items(menu: u8) -> &'static [Item] {
	match menu {
		0 => &FILE,
		1 => &EDIT,
		2 => &INSERT,
		3 => &FIND,
		4 => &CHARACTER,
		5 => &PARAGRAPH,
		6 => &DOCUMENT,
		_ => &[],
	}
}

/// The attribute of a character with these flags in a scheme (the colour scheme of the SPEC's DOS table, or the mono one).
fn text_attr(scheme: Scheme, flags: u16, vpos: u8, selected: bool) -> u8 {
	if selected {
		return 0x70;
	}
	let b = flags & cflag::BOLD != 0;
	let i = flags & cflag::ITALIC != 0;
	let u = flags & cflag::UNDERLINE != 0;
	let st = flags & cflag::STRIKE != 0;
	match scheme {
		Scheme::Colour => {
			if vpos != 0 {
				return 0x16;
			}
			match (b, i, u) {
				(false, false, false) => {
					if st {
						0x18
					} else {
						attr::TEXT
					}
				}
				(true, false, false) => attr::BOLD,
				(false, true, false) => attr::ITALIC,
				(false, false, true) => 0x1D,
				(true, true, false) => 0x1E,
				(true, false, true) => 0x1C,
				(false, true, true) => 0x1A,
				(true, true, true) => 0x1E,
			}
		}
		Scheme::Mono => match (b, i, u) {
			_ if st => 0x08,
			(false, false, false) => 0x07,
			(true, false, false) => 0x0F,
			(false, true, false) => 0x70,
			(false, false, true) => 0x01,
			(true, _, true) => 0x09,
			(true, true, false) => 0x7F,
			(false, true, true) => 0x71,
		},
	}
}

fn put_str<const ROWS: usize>(g: &mut Grid<ROWS>, x: usize, y: usize, t: &str, attr: u8) {
	for (i, c) in t.chars().enumerate() {
		let b = if (c as u32) < 0x80 {
			c as u32 as u8
		} else {
			ansi_to_cp437(c as u32 as u8)
		};
		g.put(x + i, y, b, attr);
	}
}

/// Where the hardware cursor goes: column and row.
pub type Cursor = (usize, usize);

struct Row {
	buf: [u8; COLS],
	n: usize,
}

impl Row {
	fn new() -> Self {
		Row {
			buf: [b' '; COLS],
			n: 0,
		}
	}
	fn s(&mut self, t: &str) -> &mut Self {
		for b in t.bytes() {
			self.byte(b);
		}
		self
	}
	fn bytes(&mut self, t: &[u8]) -> &mut Self {
		for b in t {
			self.byte(ansi_to_cp437(*b));
		}
		self
	}
	fn byte(&mut self, b: u8) {
		if let Some(slot) = self.buf.get_mut(self.n) {
			*slot = b;
			self.n += 1;
		}
	}
	fn dec(&mut self, mut v: u32) -> &mut Self {
		let mut tmp = [0u8; 10];
		let mut i = 0;
		loop {
			if let Some(t) = tmp.get_mut(i) {
				*t = b'0' + (v % 10) as u8;
			}
			i += 1;
			v /= 10;
			if v == 0 {
				break;
			}
		}
		while i > 0 {
			i -= 1;
			self.byte(tmp.get(i).copied().unwrap_or(b'0'));
		}
		self
	}
}

/// Paints the whole screen and returns where the hardware cursor belongs (none while a menu is open).
pub fn draw<S: Store + ?Sized, const ROWS: usize>(
	app: &App,
	s: &S,
	g: &mut Grid<ROWS>,
) -> Option<Cursor> {
	let text_rows = ROWS.saturating_sub(2);
	let base = if app.scheme == Scheme::Colour {
		attr::TEXT
	} else {
		0x07
	};
	*g = Grid::new(base);
	let sel = app.selection();
	let text_w = ((app.doc.page.paper_w - app.doc.page.margin_l - app.doc.page.margin_r) * 10
		/ 1440) as usize;
	let margin = COLS.saturating_sub(text_w) / 2;

	// The page.
	let caret = app.caret_line(s);
	let caret_page = caret.map_or(1, |(_, l)| l.page);
	let mut ln_in_page = 0u32;
	let mut counters = [[0u32; 9]; 4];
	let mut cursor = None;
	app.lines(s, |i, l| {
		if let Some((ci, _)) = caret {
			if l.page == caret_page && i <= ci {
				ln_in_page += 1;
			}
		}
		if l.line == 0 {
			let pf = app.para_pfmt(s, l.start);
			if pf.flags & 5 != 0 {
				let lvl = usize::from(pf.level.min(8));
				let list = usize::from(pf.list).min(3);
				let def = app.doc.lists.get(s, pf.list).unwrap_or_default();
				let lv = def.levels.get(lvl).copied().unwrap_or_default();
				let mut label = [0u8; 16];
				let mut n = 0;
				if pf.flags & 1 != 0 {
					label[0] = 0x07;
					n = 1;
				} else {
					if let Some(c) = counters.get_mut(list) {
						let cur = c.get(lvl).copied().unwrap_or(0);
						let next = if cur == 0 || pf.flags & 8 != 0 {
							u32::from(lv.start.max(1))
						} else {
							cur + 1
						};
						if let Some(slot) = c.get_mut(lvl) {
							*slot = next;
						}
						for deeper in c.iter_mut().skip(lvl + 1) {
							*deeper = 0;
						}
						n = number_label(lv.style, next, &mut label);
					}
				}
				let first = quill_core::layout::to_units(pf.left + pf.first, 10).max(0) as usize;
				let top = app.top;
				if l_visible(i, top, text_rows) {
					let row = 1 + (i - top) as usize;
					for (k, b) in label.iter().take(n).enumerate() {
						g.put(margin + first + k, row, *b, attr::TEXT);
					}
				}
			}
		}
		let top = app.top;
		if i < top || (i - top) as usize >= text_rows {
			return;
		}
		let row = 1 + (i - top) as usize;
		app.walk(s, l, |p, c, x, w, leader| {
			let col = margin + x as usize;
			let cf = app
				.doc
				.body
				.cfmt_at(s, p)
				.and_then(|i| app.doc.cfmts.get(s, i));
			let (flags, vpos) = cf.map_or((0, 0), |c| (c.flags, c.vpos));
			let selected = sel.is_some_and(|(lo, hi)| p >= lo && p < hi);
			let a = text_attr(app.scheme, flags, vpos, selected);
			if c == 0x09 {
				let fill = match leader {
					1 => b'.',
					2 => b'-',
					3 => b'_',
					_ => b' ',
				};
				for k in 0..w as usize {
					g.put(col + k, row, fill, a);
				}
			} else if c == 0x01 {
				let letter = match app.doc.body.object_at(s, p).map(|v| v >> 16) {
					Some(1) => b'P',
					Some(2) => b'N',
					Some(3) => b'D',
					Some(4) => b'F',
					_ => 0,
				};
				if letter == 0 {
					g.put(col, row, 0xB0, a);
					g.put(col + 1, row, 0xB0, a);
				} else {
					g.put(col, row, b'{', a);
					g.put(col + 1, row, letter, a);
				}
			} else {
				g.put(col, row, ansi_to_cp437(c), a);
			}
		});
		if let Some((ci, _)) = caret {
			if ci == i {
				cursor = Some((margin + app.x_of(s, l, app.caret) as usize, row));
			}
		}
	});

	// The menu bar.
	g.fill(0, 0, COLS, 1, b' ', attr::CHROME);
	g.text(0, 0, MENU_BAR, attr::CHROME);
	if let Mode::Menu { menu, .. } = app.mode {
		let title = MENU_TITLES.get(usize::from(menu)).copied().unwrap_or(b"");
		let col = MENU_COLS.get(usize::from(menu)).copied().unwrap_or(1);
		g.text(col, 0, title, SELECTED);
	}

	// The status line.
	let last = ROWS - 1;
	g.fill(0, last, COLS, 1, b' ', attr::CHROME);
	let mut st = Row::new();
	if app.msg.is_empty() {
		st.s(" ");
		match app.view {
			1 => {
				st.s("[Header] ");
			}
			2 => {
				st.s("[Footer] ");
			}
			3 => {
				st.s("[First header] ");
			}
			4 => {
				st.s("[First footer] ");
			}
			_ => {}
		}
		if app.name_len == 0 {
			st.s("(Untitled)");
		} else {
			st.bytes(app.name());
		}
		st.s("   Pg ")
			.dec(caret_page)
			.s(" Ln ")
			.dec(ln_in_page)
			.s(" Col ");
		let col = caret.map_or(1, |(_, l)| app.x_of(s, &l, app.caret) + 1);
		st.dec(col as u32).s("   ");
		let t = &app.doc.body;
		let cf = if app.caret > 0 {
			t.cfmt_at(s, app.caret - 1)
		} else {
			t.cfmt_at(s, 0)
		}
		.unwrap_or(0);
		if let Some(c) = app.doc.cfmts.get(s, cf) {
			if let Some(f) = app.doc.fonts.get(s, c.font) {
				st.bytes(f.name()).s(" ").dec(u32::from(c.size / 2));
				if c.size % 2 == 1 {
					st.s(".5");
				}
			}
		}
		if app.overtype {
			st.s("   OVR");
		}
		if app.extend {
			st.s("   EXT");
		}
		if app.modified {
			st.s("   Mod");
		}
	} else {
		st.s(" ").bytes(app.msg.bytes());
	}
	g.text(0, last, st.buf.get(..COLS).unwrap_or(&[]), attr::CHROME);

	// Menus and prompts on top.
	match app.mode {
		Mode::Menu { menu, item } => {
			let items = menu_items(menu);
			let col = MENU_COLS.get(usize::from(menu)).copied().unwrap_or(1) - 1;
			let w = 26;
			g.fill(col, 1, w, items.len() + 2, b' ', attr::CHROME);
			g.frame(col, 1, w, items.len() + 2, attr::CHROME);
			for (i, it) in items.iter().enumerate() {
				let a = if i == usize::from(item) {
					SELECTED
				} else {
					attr::CHROME
				};
				g.fill(col + 1, 2 + i, w - 2, 1, b' ', a);
				put_str(g, col + 2, 2 + i, it.label, a);
				g.text(col + w - 2 - it.accel.len(), 2 + i, it.accel.as_bytes(), a);
			}
			shadow(g, col, 1, w, items.len() + 2);
			cursor = None;
		}
		Mode::Prompt { kind, buf, len } => {
			let title: &[u8] = if kind == Prompt::Open {
				b" Open "
			} else {
				b" Save As "
			};
			dialog(g, 8, 62, 7, title);
			g.text(10, 10, b"File name:", attr::CHROME);
			g.fill(21, 10, 47, 1, b' ', 0x07);
			g.text(21, 10, buf.get(..len).unwrap_or(&[]), 0x07);
			g.text(24, 12, b"< OK >", attr::CHROME);
			g.text(40, 12, b"< Cancel >", attr::CHROME);
			cursor = Some((21 + len, 10));
		}
		Mode::Find(st) => {
			let h = if st.replace { 11 } else { 9 };
			dialog(
				g,
				8,
				62,
				h,
				if st.replace { b" Replace " } else { b" Find " },
			);
			let bx = |on: bool| if on { b"[x]" } else { b"[ ]" };
			g.text(10, 10, b"Find what:", attr::CHROME);
			g.fill(24, 10, 34, 1, b' ', 0x07);
			g.text(24, 10, st.find.get(..st.flen).unwrap_or(&[]), 0x07);
			let mut y = 11;
			if st.replace {
				g.text(10, 11, b"Replace with:", attr::CHROME);
				g.fill(24, 11, 34, 1, b' ', 0x07);
				g.text(24, 11, st.with.get(..st.wlen).unwrap_or(&[]), 0x07);
				y = 12;
			}
			g.text(10, y, bx(st.case), attr::CHROME);
			g.text(14, y, b"Match case", attr::CHROME);
			g.text(30, y, bx(st.word), attr::CHROME);
			g.text(34, y, b"Match whole word only", attr::CHROME);
			let by = y + 2;
			let mut x = 10;
			let buttons: &[(&[u8], crate::find::Focus)] = if st.replace {
				&[
					(b"< Find Next >", crate::find::Focus::FindNext),
					(b"< Replace >", crate::find::Focus::ReplaceOne),
					(b"< Replace All >", crate::find::Focus::ReplaceAll),
					(b"< Close >", crate::find::Focus::Close),
				]
			} else {
				&[
					(b"< Find Next >", crate::find::Focus::FindNext),
					(b"< Close >", crate::find::Focus::Close),
				]
			};
			for (t, f) in buttons {
				let a = if st.focus == *f {
					SELECTED
				} else {
					attr::CHROME
				};
				g.text(x, by, t, a);
				x += t.len() + 2;
			}
			cursor = match st.focus {
				crate::find::Focus::Find => Some((24 + st.flen, 10)),
				crate::find::Focus::Replace => Some((24 + st.wlen, 11)),
				_ => None,
			};
		}
		Mode::Confirm(_) => {
			dialog(g, 8, 62, 7, b" Quill ");
			let mut r = Row::new();
			r.s("Save changes to ");
			if app.name_len == 0 {
				r.s("(Untitled)");
			} else {
				r.bytes(app.name());
			}
			r.s("?");
			g.text(10, 10, r.buf.get(..r.n).unwrap_or(&[]), attr::CHROME);
			g.text(16, 12, b"< Yes >", attr::CHROME);
			g.text(30, 12, b"< No >", attr::CHROME);
			g.text(43, 12, b"< Cancel >", attr::CHROME);
			cursor = None;
		}
		Mode::Edit => {}
	}
	cursor
}

fn dialog<const ROWS: usize>(g: &mut Grid<ROWS>, x: usize, w: usize, h: usize, title: &[u8]) {
	let y = 8;
	g.fill(x, y, w, h, b' ', attr::CHROME);
	g.frame(x, y, w, h, attr::CHROME);
	g.text(x + (w - title.len()) / 2, y, title, attr::CHROME);
	shadow(g, x, y, w, h);
}

/// A dark shadow one cell right and below a box.
fn shadow<const ROWS: usize>(g: &mut Grid<ROWS>, x: usize, y: usize, w: usize, h: usize) {
	for yy in y + 1..=y + h {
		for xx in [x + w, x + w + 1] {
			if let Some(c) = g.get(xx, yy) {
				g.put(xx, yy, c.ch, 0x08);
			}
		}
	}
	for xx in x + 2..x + w + 2 {
		if let Some(c) = g.get(xx, y + h) {
			g.put(xx, y + h, c.ch, 0x08);
		}
	}
}

fn l_visible(i: u32, top: u32, rows: usize) -> bool {
	i >= top && ((i - top) as usize) < rows
}

/// Writes the label of number `n` in `style` (0 `1.`, 1 `1)`, 2 `a.`, 3 `a)`, 4 `i.`, 5 `A.`, 6 `I.`, 7 `1.`) and returns its length.
fn number_label(style: u8, n: u32, out: &mut [u8; 16]) -> usize {
	let mut len = 0;
	let mut put = |b: u8| {
		if let Some(slot) = out.get_mut(len) {
			*slot = b;
			len += 1;
		}
	};
	match style {
		2 | 3 | 5 => {
			let base = if style == 5 { b'A' } else { b'a' };
			put(base + ((n.max(1) - 1) % 26) as u8);
		}
		4 | 6 => {
			let mut v = n.clamp(1, 3999);
			for (val, sym) in [
				(1000, "m"),
				(900, "cm"),
				(500, "d"),
				(400, "cd"),
				(100, "c"),
				(90, "xc"),
				(50, "l"),
				(40, "xl"),
				(10, "x"),
				(9, "ix"),
				(5, "v"),
				(4, "iv"),
				(1, "i"),
			] {
				while v >= val {
					for b in sym.bytes() {
						put(if style == 6 {
							b.to_ascii_uppercase()
						} else {
							b
						});
					}
					v -= val;
				}
			}
		}
		_ => {
			let mut digits = [0u8; 10];
			let mut i = digits.len();
			let mut v = n;
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
				put(*b);
			}
		}
	}
	put(if style == 1 || style == 3 { b')' } else { b'.' });
	len
}
