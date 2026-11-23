// The text-mode painter: the menu bar, the page, the status line, the menus and the prompts, into a Grid.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use quill_core::store::Store;
use quill_grid::{ansi_to_cp437, attr, Grid, COLS};

use crate::{App, Command, Mode, Prompt};

/// Rows of text between the menu bar and the status line on an 80 by 25 screen.
pub const TEXT_ROWS: usize = 23;
/// Menus that open (the rest of the bar is drawn for later milestones).
pub const ACTIVE_MENUS: u8 = 2;

const MENU_BAR: &[u8] = b" File  Edit  Insert  Find  Character  Paragraph  Document  Help";
const MENU_COLS: [usize; 2] = [1, 7];
const MENU_TITLES: [&[u8]; 2] = [b"File", b"Edit"];
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

const FILE: [Item; 5] = [
	Item {
		label: "New",
		accel: "Ctrl+N",
		command: Command::New,
	},
	Item {
		label: "Open...",
		accel: "Ctrl+O",
		command: Command::Open,
	},
	Item {
		label: "Save",
		accel: "Ctrl+S",
		command: Command::Save,
	},
	Item {
		label: "Save As...",
		accel: "",
		command: Command::SaveAs,
	},
	Item {
		label: "Exit",
		accel: "Ctrl+Q",
		command: Command::Exit,
	},
];
const EDIT: [Item; 2] = [
	Item {
		label: "Undo",
		accel: "Ctrl+Z",
		command: Command::Undo,
	},
	Item {
		label: "Redo",
		accel: "Ctrl+Y",
		command: Command::Redo,
	},
];

pub(crate) fn menu_items(menu: u8) -> &'static [Item] {
	if menu == 0 {
		&FILE
	} else {
		&EDIT
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
	*g = Grid::new(attr::TEXT);
	let text_w = ((app.doc.page.paper_w - app.doc.page.margin_l - app.doc.page.margin_r) * 10
		/ 1440) as usize;
	let margin = COLS.saturating_sub(text_w) / 2;

	// The page.
	let caret = app.caret_line(s);
	let caret_page = caret.map_or(1, |(_, l)| l.page);
	let mut ln_in_page = 0u32;
	let mut cursor = None;
	app.lines(s, |i, l| {
		if let Some((ci, _)) = caret {
			if l.page == caret_page && i <= ci {
				ln_in_page += 1;
			}
		}
		let top = app.top;
		if i < top || (i - top) as usize >= text_rows {
			return;
		}
		let row = 1 + (i - top) as usize;
		app.walk(s, l, |_, c, x, w| {
			let col = margin + x as usize;
			if c == 0x09 {
				for k in 0..w as usize {
					g.put(col + k, row, b' ', attr::TEXT);
				}
			} else if c == 0x01 {
				g.put(col, row, 0xB0, attr::TEXT);
			} else {
				g.put(col, row, ansi_to_cp437(c), attr::TEXT);
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
		if app.modified {
			st.s("   Mod");
		}
	} else {
		st.s(" ").s(app.msg);
	}
	g.text(0, last, st.buf.get(..COLS).unwrap_or(&[]), attr::CHROME);

	// Menus and prompts on top.
	match app.mode {
		Mode::Menu { menu, item } => {
			let items = menu_items(menu);
			let col = MENU_COLS.get(usize::from(menu)).copied().unwrap_or(1) - 1;
			let w = 24;
			g.fill(col, 1, w, items.len() + 2, b' ', attr::CHROME);
			g.frame(col, 1, w, items.len() + 2, attr::CHROME);
			for (i, it) in items.iter().enumerate() {
				let a = if i == usize::from(item) {
					SELECTED
				} else {
					attr::CHROME
				};
				g.fill(col + 1, 2 + i, w - 2, 1, b' ', a);
				g.text(col + 2, 2 + i, it.label.as_bytes(), a);
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
