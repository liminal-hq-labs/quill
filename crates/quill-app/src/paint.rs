// The text-mode painter: the menu bar, the page, the status line, the menus and the prompts, into a Grid.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use quill_core::layout::Measure;
use quill_core::store::ReadStore;
use quill_grid::{ansi_to_cp437, attr, Grid, COLS};

use quill_core::doc::cflag;

use crate::{App, Command, Mode, Prompt, Scheme};

/// Rows of text between the menu bar and the status line on an 80 by 25 screen.
pub const TEXT_ROWS: usize = 23;
/// Menus on the bar.
pub const MENU_COUNT: u8 = 9;

#[cfg(feature = "text-file-dialog")]
#[path = "filepaint.rs"]
mod filepaint;

/// The column each menu's title starts at on the DOS menu bar.
pub(crate) const MENU_COLS: [usize; 9] = [1, 7, 13, 21, 27, 38, 49, 59, 65];
/// The attribute of a mnemonic letter on an unselected menu entry, and on the selected one.
const HOT: u8 = 0x74;
const HOT_SELECTED: u8 = 0x0E;

/// The mnemonic of a label written with an `&` before the letter, as the lower-case letter or digit.
pub fn mnemonic(label: &str) -> Option<u8> {
	let mut it = label.bytes();
	while let Some(b) = it.next() {
		if b == b'&' {
			return it.next().map(|c| c.to_ascii_lowercase());
		}
	}
	None
}

/// The menu whose title has the mnemonic `c` (a lower-case letter).
pub fn menu_for_mnemonic(c: u8) -> Option<u8> {
	(0..MENU_COUNT).find(|m| {
		mnemonic(core::str::from_utf8(menu_title(*m)).unwrap_or("")) == Some(c)
			&& !menu_items(*m).is_empty()
	})
}

/// The item of menu `menu` whose mnemonic is `c`, counting the recent files of File (1 to 4) after its own items.
pub fn item_for_mnemonic(menu: u8, c: u8, recents: usize, text: bool) -> Option<usize> {
	let items = menu_items_for(menu, text);
	if let Some(i) = items.iter().position(|it| mnemonic(it.label) == Some(c)) {
		return Some(i);
	}
	let n = usize::from(c.wrapping_sub(b'1'));
	(menu == 0 && n < recents).then(|| items.len().saturating_sub(1) + n)
}

/// The title of menu `menu`, with `&` before its mnemonic.
pub fn menu_title(menu: u8) -> &'static [u8] {
	MENU_TITLES.get(usize::from(menu)).copied().unwrap_or(b"")
}

const MENU_TITLES: [&[u8]; 9] = [
	b"&File",
	b"&Edit",
	b"&Insert",
	b"Fi&nd",
	b"&Character",
	b"&Paragraph",
	b"&Document",
	b"&View",
	b"&Help",
];
const SELECTED: u8 = 0x07;

/// A pull-down item.
pub struct Item {
	/// The text, with its ellipsis.
	pub label: &'static str,
	/// The accelerator shown at the right.
	pub accel: &'static str,
	/// The command it runs; for an item with a submenu, the command its first entry runs, so a menu that cannot open submenus (the
	/// DOS text menu) still has something sensible to do.
	pub command: Command,
	/// True when a separator rule is drawn above this item, which starts a new group.
	pub rule: bool,
	/// The entries of a submenu that opens from this item; empty for an ordinary item.
	pub sub: &'static [Item],
}

const fn item(label: &'static str, accel: &'static str, command: Command) -> Item {
	Item {
		label,
		accel,
		command,
		rule: false,
		sub: &[],
	}
}

/// The same item, with a separator rule above it.
const fn rule(i: Item) -> Item {
	Item { rule: true, ..i }
}

/// An item that opens a submenu.
const fn submenu(label: &'static str, command: Command, sub: &'static [Item]) -> Item {
	Item {
		label,
		accel: "",
		command,
		rule: false,
		sub,
	}
}

const ZOOM: [Item; 5] = [
	item("&Page Width", "Ctrl+0", Command::Zoom(0)),
	item("&150%", "", Command::Zoom(150)),
	item("1&00%", "", Command::Zoom(100)),
	item("&75%", "", Command::Zoom(75)),
	item("&50%", "", Command::Zoom(50)),
];
const FIND: [Item; 5] = [
	item("&Find...", "Ctrl+F", Command::Find),
	item("Find &Next", "F3", Command::FindNext),
	item("&Replace...", "Ctrl+H", Command::Replace),
	rule(item("&Spelling...", "F7", Command::Spelling)),
	item("&Check Spelling as You Type", "", Command::SpellAsYouType),
];
const INSERT: [Item; 6] = [
	item("&Picture...", "", Command::Picture),
	rule(item("Page &Number", "", Command::Field(1))),
	item("Page &Count", "", Command::Field(2)),
	item("&Date", "", Command::Field(3)),
	item("&File Name", "", Command::Field(4)),
	item("&Time", "", Command::Field(5)),
];
const DOCUMENT: [Item; 6] = [
	item("&Header", "", Command::View(1)),
	item("&Footer", "", Command::View(2)),
	item("Fir&st Page Header", "", Command::View(3)),
	item("First &Page Footer", "", Command::View(4)),
	item("&Different First Page", "", Command::FirstPage),
	rule(item("&Return to Document", "", Command::View(0))),
];
const VIEW: [Item; 5] = [
	item("&Toolbar", "", Command::Chrome(1)),
	item("&Formatting Bar", "", Command::Chrome(8)),
	item("&Ruler", "", Command::Chrome(2)),
	item("&Status Bar", "", Command::Chrome(4)),
	rule(submenu("&Zoom", Command::Zoom(0), &ZOOM)),
];
const HELP: [Item; 2] = [
	item("&Contents", "F1", Command::Help),
	item("&About Quill...", "", Command::About),
];
const FILE: [Item; 8] = [
	item("&New", "Ctrl+N", Command::New),
	item("&Open...", "Ctrl+O", Command::Open),
	rule(item("&Save", "Ctrl+S", Command::Save)),
	item("Save &As...", "", Command::SaveAs),
	rule(item("&Print...", "Ctrl+P", Command::Print)),
	item("Print Pre&view", "", Command::Preview),
	item("Print Set&up...", "", Command::PrintSetup),
	rule(item("E&xit", "Ctrl+Q", Command::Exit)),
];
const EDIT: [Item; 6] = [
	item("&Undo", "Ctrl+Z", Command::Undo),
	item("&Redo", "Ctrl+Y", Command::Redo),
	rule(item("Cu&t", "Ctrl+X", Command::Cut)),
	item("&Copy", "Ctrl+C", Command::Copy),
	item("&Paste", "Ctrl+V", Command::Paste),
	rule(item("Select A&ll", "Ctrl+A", Command::SelectAll)),
];
const CHARACTER: [Item; 8] = [
	item("&Regular", "F5", Command::Regular),
	rule(item("&Bold", "Ctrl+B", Command::Bold)),
	item("&Italic", "Ctrl+I", Command::Italic),
	item("&Underline", "Ctrl+U", Command::Underline),
	item("Stri&kethrough", "", Command::Strike),
	rule(item("Su&perscript", "Ctrl+Shift+=", Command::Super)),
	item("Subs&cript", "Ctrl+=", Command::Sub),
	rule(item("&Fonts...", "", Command::Fonts)),
];
const PARAGRAPH: [Item; 12] = [
	item("&Left", "Ctrl+L", Command::Align(0)),
	item("&Centred", "Ctrl+E", Command::Align(1)),
	item("&Right", "Ctrl+R", Command::Align(2)),
	item("&Justified", "Ctrl+J", Command::Align(3)),
	rule(item("&Single Space", "Ctrl+1", Command::Spacing(0))),
	item("&1\u{bd} Space", "Ctrl+5", Command::Spacing(1)),
	item("&Double Space", "Ctrl+2", Command::Spacing(2)),
	rule(item("&Bullets", "", Command::Bullets)),
	item("&Numbering", "", Command::Numbering),
	item("&Increase Level", "", Command::LevelUp),
	item("Decrease Le&vel", "", Command::LevelDown),
	rule(item("&Tabs...", "", Command::Tabs)),
];

/// File as the DOS text menu has it: the SPEC leaves out Print Preview and Print Setup there.
const FILE_TEXT: [Item; 6] = [
	item("&New", "Ctrl+N", Command::New),
	item("&Open...", "Ctrl+O", Command::Open),
	rule(item("&Save", "Ctrl+S", Command::Save)),
	item("Save &As...", "", Command::SaveAs),
	rule(item("&Print...", "Ctrl+P", Command::Print)),
	rule(item("E&xit", "Ctrl+Q", Command::Exit)),
];

/// What a place in an open menu holds: one of its own items, or one of the recent files.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Slot {
	/// The item at this index of the menu's items.
	Item(usize),
	/// The recent file at this index.
	Recent(usize),
}

/// What place `at` of the File menu holds when `recents` recent files are listed: the recent files go between the last group and the
/// last item (Exit), as on Windows. Other menus have no recent files; pass 0.
pub fn slot(items: usize, recents: usize, at: usize) -> Slot {
	let before = items.saturating_sub(1);
	if recents == 0 || at < before {
		Slot::Item(at)
	} else if at < before + recents {
		Slot::Recent(at - before)
	} else {
		Slot::Item(at - recents)
	}
}

/// The items menu `menu` shows in a shell: `text` is true for the text menu bar (DOS), which has fewer items in File than the
/// Windows menus have; everywhere else it is [`menu_items`].
pub fn menu_items_for(menu: u8, text: bool) -> &'static [Item] {
	if text && menu == 0 {
		&FILE_TEXT
	} else {
		menu_items(menu)
	}
}

/// Where the drop-down of `menu` goes on the text screen: its column, its width and its height with the frame, the menu's own items
/// and `recents` recent files below them. It starts under its title, moved left when it would run past the right edge, and the two
/// columns of its shadow still fit inside the 80.
pub fn menu_box(menu: u8, text: bool, recents: usize) -> (usize, usize, usize) {
	let items = menu_items_for(menu, text);
	// Wide enough for the longest label and its accelerator with a gap between them (the Character menu needs more).
	let w = items
		.iter()
		.map(|it| it.label.chars().filter(|c| *c != '&').count() + it.accel.len() + 6)
		.fold(26, usize::max);
	// A rule above an item takes a row of its own, and so does the one above the recent files.
	let rules = items.iter().filter(|it| it.rule).count() + usize::from(recents > 0);
	let rows = items.len() + recents + rules + 2;
	let col = MENU_COLS
		.get(usize::from(menu))
		.copied()
		.unwrap_or(1)
		.saturating_sub(1);
	(col.min(COLS.saturating_sub(w + 2)), w, rows)
}

/// The items of menu `menu` (0 File to 7 Help); empty for a menu that has none yet.
pub fn menu_items(menu: u8) -> &'static [Item] {
	match menu {
		0 => &FILE,
		1 => &EDIT,
		2 => &INSERT,
		3 => &FIND,
		4 => &CHARACTER,
		5 => &PARAGRAPH,
		6 => &DOCUMENT,
		7 => &VIEW,
		8 => &HELP,
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

/// The attribute of a misspelt word (SPEC, DOS text mode): a red background in colour, keeping the text's own colour unless that
/// would vanish into the red; reverse video in mono, keeping bright.
fn misspelt_attr(scheme: Scheme, a: u8) -> u8 {
	match scheme {
		Scheme::Colour => {
			let fore = a & 0x0F;
			let fore = if fore & 7 == 4 { 0x0F } else { fore };
			0x40 | fore
		}
		Scheme::Mono => 0x70 | (a & 0x08),
	}
}

/// A separator rule across a menu box that starts at column `col`, row `y`: the box's own edges joined to a line.
fn menu_rule<const ROWS: usize>(g: &mut Grid<ROWS>, col: usize, y: usize, w: usize) {
	g.put(col, y, 0xC3, attr::CHROME);
	for x in col + 1..col + w - 1 {
		g.put(x, y, 0xC4, attr::CHROME);
	}
	g.put(col + w - 1, y, 0xB4, attr::CHROME);
}

/// Writes a label, leaving out the `&` and drawing the letter after it in `hot`.
fn put_label<const ROWS: usize>(
	g: &mut Grid<ROWS>,
	x: usize,
	y: usize,
	t: &str,
	attr: u8,
	hot: u8,
) {
	let mut col = x;
	let mut next_hot = false;
	for c in t.chars() {
		if c == '&' && !next_hot {
			next_hot = true;
			continue;
		}
		let b = if (c as u32) < 0x80 {
			c as u32 as u8
		} else {
			ansi_to_cp437(c as u32 as u8)
		};
		g.put(col, y, b, if next_hot { hot } else { attr });
		next_hot = false;
		col += 1;
	}
}

/// Where the hardware cursor goes: column and row.
pub type Cursor = (usize, usize);

struct Row {
	buf: [u8; COLS],
	n: usize,
	/// Keep ANSI text as it is instead of converting it to code page 437.
	ansi: bool,
}

impl Row {
	fn new() -> Self {
		Row {
			buf: [b' '; COLS],
			n: 0,
			ansi: false,
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
			self.byte(if self.ansi { *b } else { ansi_to_cp437(*b) });
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

/// The status line's text into `st`: the message the last command left, or the file name, the caret's page, line and column, the font
/// at the caret and the modes.
fn status_into<M: Measure + Clone, S: ReadStore + ?Sized>(
	app: &App<M>,
	s: &S,
	caret: Option<(u32, quill_core::layout::Line)>,
	caret_page: u32,
	ln_in_page: u32,
	st: &mut Row,
) {
	let mut pic = [0u8; 80];
	let pn = if app.msg.is_empty() {
		app.picture_status(s, b'x', &mut pic)
	} else {
		0
	};
	if pn > 0 {
		st.s(" ").bytes(pic.get(..pn).unwrap_or(&[]));
	} else if app.msg.is_empty() {
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
		// Col counts columns on the text screen and characters on a device with finer units.
		let col = caret.map_or(1, |(_, l)| {
			if app.measure.resolution().0 > 10 {
				(app.caret - l.start) as i32 + 1
			} else {
				app.x_of(s, &l, app.caret) + 1
			}
		});
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
}

/// The status line as the Windows builds show it, in ANSI (not code page 437): what the text screen's bottom row says, at most 80
/// bytes of it. Returns the length written to `out`.
pub fn status_text<M: Measure + Clone, S: ReadStore + ?Sized>(
	app: &App<M>,
	s: &S,
	out: &mut [u8],
) -> usize {
	let caret = app.caret_line(s);
	let caret_page = caret.map_or(1, |(_, l)| l.page);
	let ln_in_page = caret.map_or(0, |(ci, _)| app.line_in_page(s, ci, caret_page));
	let mut st = Row::new();
	st.ansi = true;
	status_into(app, s, caret, caret_page, ln_in_page, &mut st);
	let n = st.n.min(COLS).min(out.len());
	for (d, b) in out.iter_mut().zip(st.buf.iter().take(n)) {
		*d = *b;
	}
	n
}

/// Paints the whole screen and returns where the hardware cursor belongs (none while a menu is open).
pub fn draw<M: Measure + Clone, S: ReadStore + ?Sized, const ROWS: usize>(
	app: &App<M>,
	s: &S,
	g: &mut Grid<ROWS>,
) -> Option<Cursor> {
	draw_rows(app, s, g, ROWS)
}

/// Paints a screen of `rows` rows into the top of `g`, which may have more: a shell with one grid for the 25 and the 50 row screens
/// keeps one copy of the painter. The rows below `rows` are left as the grid's background.
pub fn draw_rows<M: Measure + Clone, S: ReadStore + ?Sized, const ROWS: usize>(
	app: &App<M>,
	s: &S,
	g: &mut Grid<ROWS>,
	rows: usize,
) -> Option<Cursor> {
	let text_rows = rows.saturating_sub(2);
	let base = if app.scheme == Scheme::Colour {
		attr::TEXT
	} else {
		0x07
	};
	*g = Grid::new(base);
	let sel = app.selection();
	let text_w = (app.doc.page.text_w().saturating_mul(10) / 1440) as usize;
	let margin = COLS.saturating_sub(text_w) / 2;

	// The page.
	let caret = app.caret_line(s);
	let caret_page = caret.map_or(1, |(_, l)| l.page);
	let mut counters: ListCounters = [[0u32; 9]; 4];
	let mut cursor = None;
	let mut caret_row = None;
	// Rows are the layout's own: a line sits where its page and y put it, so spacing and tall lines show.
	let page_rows = quill_core::layout::to_units(app.doc.page.text_h(), 6);
	let cum = |l: &quill_core::layout::Line| (l.page as i32 - 1) * page_rows + l.y;
	let top_cum = app.line_at(s, app.top).map_or(0, |l| cum(&l));
	app.lines_from(s, app.top, |i, l| {
		let rel = cum(l) - top_cum;
		let height = (l.ascent + l.descent).max(1);
		let first_row = 1 + rel;
		let row_i = first_row + height - 1;
		if let Some((label, n, first)) = line_label(app, s, l, &mut counters, 10) {
			if i >= app.top && rel < text_rows as i32 && row_i >= 1 {
				let row = row_i as usize;
				for (k, b) in label.iter().take(n).enumerate() {
					g.put(margin + first + k, row, *b, attr::TEXT);
				}
			}
		}
		if i >= app.top && rel >= text_rows as i32 {
			// Below the screen: nothing later is shown either.
			return false;
		}
		if i < app.top || row_i < 1 {
			return true;
		}
		let row = row_i as usize;
		// The misspelt words of this line (as you type), a few at most: a word past the sixth shows plain until the next scan.
		let mut marks = [(0u32, 0u32); 6];
		let mut marked = 0;
		for mark in app
			.spell_marks(s, l.start, l.start + l.len)
			.take(marks.len())
		{
			if let Some(slot) = marks.get_mut(marked) {
				*slot = mark;
				marked += 1;
			}
		}
		app.walk(s, l, |p, c, x, w, leader| {
			let col = margin + x as usize;
			let cf = app
				.doc
				.body
				.cfmt_at(s, p)
				.and_then(|i| app.doc.cfmts.get(s, i));
			let (flags, vpos) = cf.map_or((0, 0), |c| (c.flags, c.vpos));
			let selected = sel.is_some_and(|(lo, hi)| p >= lo && p < hi);
			let mut a = text_attr(app.scheme, flags, vpos, selected);
			if !selected && marks.iter().take(marked).any(|m| p >= m.0 && p < m.1) {
				a = misspelt_attr(app.scheme, a);
			}
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
				if let Some((pw, ph)) =
					quill_core::layout::picture_size(&app.doc, s, &app.doc.body, p, 10, 6)
				{
					// A picture is a box of shading the height it would print, labelled with its size in pixels.
					let top_row = first_row.max(1) as usize;
					for yy in 0..ph as usize {
						for xx in 0..pw as usize {
							g.put(col + xx, top_row + yy, 0xB0, a);
						}
					}
					let idx = app.doc.body.object_at(s, p).map_or(0, |v| v & 0xFFFF);
					if let Some(pic) = app.doc.pictures.get(s, idx) {
						let mut r = Row::new();
						r.s("[Picture ")
							.dec(u32::from(pic.w))
							.s(" x ")
							.dec(u32::from(pic.h))
							.s("]");
						let label = r.buf.get(..r.n.min(pw as usize)).unwrap_or(&[]);
						g.text(col, top_row + (ph as usize) / 2, label, a);
					}
					return;
				}
				let letter = match app.doc.body.object_at(s, p).map(|v| v >> 16) {
					Some(1) => b'P',
					Some(2) => b'N',
					Some(3) => b'D',
					Some(4) => b'F',
					Some(5) => b'T',
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
		if caret.is_some_and(|(ci, _)| ci == i) {
			caret_row = Some((*l, row));
		}
		true
	});
	// The caret's x is worked out after the pass, not inside it: the pass and the walk of a line each hold a font cache on the stack,
	// and a third one under them is more than a Windows 3.x stack has to spare.
	if let Some((l, row)) = caret_row {
		cursor = Some((margin + app.x_of(s, &l, app.caret) as usize, row));
	}
	let ln_in_page = caret.map_or(0, |(ci, _)| app.line_in_page(s, ci, caret_page));

	// The menu bar.
	g.fill(0, 0, COLS, 1, b' ', attr::CHROME);
	for (m, title) in MENU_TITLES.iter().enumerate() {
		let col = MENU_COLS.get(m).copied().unwrap_or(1);
		let on = matches!(app.mode, Mode::Menu { menu, .. } if usize::from(menu) == m);
		let (a, h) = if on {
			(SELECTED, HOT_SELECTED)
		} else {
			(attr::CHROME, HOT)
		};
		put_label(g, col, 0, core::str::from_utf8(title).unwrap_or(""), a, h);
	}

	// The status line.
	let last = rows.saturating_sub(1).min(ROWS - 1);
	g.fill(0, last, COLS, 1, b' ', attr::CHROME);
	let mut st = Row::new();
	status_into(app, s, caret, caret_page, ln_in_page, &mut st);
	g.text(0, last, st.buf.get(..COLS).unwrap_or(&[]), attr::CHROME);

	// Menus and prompts on top.
	match app.mode {
		Mode::Menu { menu, item } => {
			let items = menu_items_for(menu, !app.native);
			let recents = if menu == 0 {
				app.recent_files().count()
			} else {
				0
			};
			let (col, w, rows) = menu_box(menu, !app.native, recents);
			g.fill(col, 1, w, rows, b' ', attr::CHROME);
			g.frame(col, 1, w, rows, attr::CHROME);
			let mut y = 2;
			let names: [Option<&[u8]>; 4] = {
				let mut n = [None; 4];
				if menu == 0 {
					for (slot, name) in n.iter_mut().zip(app.recent_files()) {
						*slot = Some(name);
					}
				}
				n
			};
			for i in 0..items.len() + recents {
				let sel = i == usize::from(item);
				let a = if sel { SELECTED } else { attr::CHROME };
				let h = if sel { HOT_SELECTED } else { HOT };
				match slot(items.len(), recents, i) {
					Slot::Item(k) => {
						let Some(it) = items.get(k) else { continue };
						if it.rule {
							menu_rule(g, col, y, w);
							y += 1;
						}
						g.fill(col + 1, y, w - 2, 1, b' ', a);
						put_label(g, col + 2, y, it.label, a, h);
						if it.sub.is_empty() {
							g.text(col + w - 2 - it.accel.len(), y, it.accel.as_bytes(), a);
						} else {
							g.put(col + w - 3, y, 0x10, a);
						}
					}
					Slot::Recent(k) => {
						if k == 0 {
							menu_rule(g, col, y, w);
							y += 1;
						}
						g.fill(col + 1, y, w - 2, 1, b' ', a);
						g.put(col + 2, y, b'1' + k as u8, h);
						let name = names.get(k).copied().flatten().unwrap_or(&[]);
						g.text(
							col + 4,
							y,
							name.get(..name.len().min(w - 6)).unwrap_or(&[]),
							a,
						);
					}
				}
				y += 1;
			}
			shadow(g, col, 1, w, rows);
			cursor = None;
		}
		Mode::Prompt {
			kind,
			buf,
			len,
			caret,
		} => {
			let title: &[u8] = match kind {
				Prompt::Open => b" Open ",
				Prompt::SaveAs => b" Save As ",
				Prompt::Print => b" Print to file or device ",
				Prompt::Picture => b" Insert Picture ",
			};
			dialog(g, 8, 62, 7, title);
			g.text(10, 10, b"File name:", attr::CHROME);
			g.fill(21, 10, 47, 1, b' ', 0x07);
			field_text(g, 21, 10, buf.get(..len).unwrap_or(&[]), &caret);
			g.text(24, 12, b"< OK >", attr::CHROME);
			g.text(40, 12, b"< Cancel >", attr::CHROME);
			cursor = Some((21 + caret.at().min(len), 10));
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
			field_text(g, 24, 10, st.find.get(..st.flen).unwrap_or(&[]), &st.fcaret);
			let mut y = 11;
			if st.replace {
				g.text(10, 11, b"Replace with:", attr::CHROME);
				g.fill(24, 11, 34, 1, b' ', 0x07);
				field_text(g, 24, 11, st.with.get(..st.wlen).unwrap_or(&[]), &st.wcaret);
				y = 12;
			}
			let box_attr = |f: crate::find::Focus| {
				if st.focus == f {
					SELECTED
				} else {
					attr::CHROME
				}
			};
			g.text(10, y, bx(st.case), box_attr(crate::find::Focus::Case));
			g.text(14, y, b"Match case", attr::CHROME);
			g.text(30, y, bx(st.word), box_attr(crate::find::Focus::Word));
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
				crate::find::Focus::Find => Some((24 + st.fcaret.at().min(st.flen), 10)),
				crate::find::Focus::Replace => Some((24 + st.wcaret.at().min(st.wlen), 11)),
				_ => None,
			};
		}
		Mode::Form(f) => {
			let h = f.n + 4;
			dialog(
				g,
				8,
				62,
				h,
				if f.kind == crate::form::FormKind::Fonts {
					b" Fonts "
				} else {
					b" Tabs "
				},
			);
			let mut cur = None;
			for (i, r) in f.rows.iter().take(f.n).enumerate() {
				let y = 10 + i;
				let a = if i == f.focus { SELECTED } else { attr::CHROME };
				match r.kind {
					0 => {
						g.text(10, y, r.label.as_bytes(), attr::CHROME);
						g.fill(30, y, 14, 1, b' ', 0x07);
						field_text(g, 30, y, r.text.get(..r.len).unwrap_or(&[]), &r.caret);
						if i == f.focus {
							cur = Some((30 + r.caret.at().min(r.len), y));
						}
					}
					1 => {
						g.text(10, y, r.label.as_bytes(), attr::CHROME);
						let v = r.names.get(usize::from(r.val)).copied().unwrap_or("");
						g.text(30, y, v.as_bytes(), a);
					}
					2 => {
						g.text(10, y, if r.val == 1 { b"[x]" } else { b"[ ]" }, a);
						g.text(14, y, r.label.as_bytes(), attr::CHROME);
					}
					_ => g.text(10, y, r.label.as_bytes(), a),
				}
			}
			cursor = cur;
		}
		Mode::About => {
			dialog(g, 8, 62, 10, b" About Quill ");
			g.text(10, 10, b"Quill", attr::CHROME);
			let mut v = Row::new();
			v.s("Version ").s(crate::VERSION);
			g.text(10, 11, v.buf.get(..v.n).unwrap_or(&[]), attr::CHROME);
			g.text(10, 12, b"A Liminal HQ Labs experiment", attr::CHROME);
			g.text(10, 13, b"Apache-2.0 OR MIT", attr::CHROME);
			// The platform text may run to two lines.
			for (i, line) in app.about.bytes().split(|b| *b == b'\n').take(2).enumerate() {
				g.text(10, 14 + i, line, attr::CHROME);
			}
			g.text(34, 16, b"< OK >", SELECTED);
			cursor = None;
		}
		Mode::Spell(d) => {
			dialog(g, 2, 76, 13, b" Spelling ");
			let mut r = Row::new();
			r.s("Not in dictionary: ").bytes(app.spell_word());
			g.text(5, 10, r.buf.get(..r.n).unwrap_or(&[]), attr::CHROME);
			let mut ctx = [0u8; 72];
			let n = app.spell_context(s, &mut ctx);
			let mut c = Row::new();
			c.bytes(ctx.get(..n).unwrap_or(&[]));
			g.text(5, 11, c.buf.get(..c.n.min(62)).unwrap_or(&[]), attr::CHROME);
			g.text(5, 13, b"Change to:", attr::CHROME);
			for i in 0..usize::from(d.n) {
				let l = usize::from(d.lens.get(i).copied().unwrap_or(0));
				let mut w = Row::new();
				w.bytes(d.words.get(i).and_then(|w| w.get(..l)).unwrap_or(&[]));
				let at = if usize::from(d.sel) == i {
					SELECTED
				} else {
					attr::CHROME
				};
				g.text(
					17 + (i / 4) * 20,
					13 + i % 4,
					w.buf.get(..l).unwrap_or(&[]),
					at,
				);
			}
			if d.n == 0 {
				g.text(17, 13, b"(no suggestions)", attr::CHROME);
			}
			let mut x = 4;
			for (i, b) in crate::spelling::BUTTONS.iter().enumerate() {
				let mut t = Row::new();
				t.s("< ").s(b).s(" >");
				let at = if usize::from(d.focus) == i {
					SELECTED
				} else {
					attr::CHROME
				};
				g.text(x, 18, t.buf.get(..t.n).unwrap_or(&[]), at);
				x += t.n + 1;
			}
			cursor = None;
		}
		#[cfg(feature = "text-recovery")]
		Mode::Recover(ask, focus) => {
			use crate::recover_box as rb;
			let (n, lines) = rb::wrap(ask.text.as_bytes());
			let list = rb::buttons(ask.buttons);
			dialog(g, rb::BOX_X - 1, rb::BOX_W, rb::box_h(n), b" Quill ");
			for (i, (a, b)) in lines.iter().take(n).enumerate() {
				g.text(
					rb::BOX_X + 1,
					rb::BOX_Y + 2 + i,
					ask.text.as_bytes().get(*a..*b).unwrap_or(&[]),
					attr::CHROME,
				);
			}
			for (i, (label, _)) in list.iter().enumerate() {
				let mut r = Row::new();
				r.s("< ").bytes(label).s(" >");
				let a = if usize::from(focus) == i {
					SELECTED
				} else {
					attr::CHROME
				};
				g.text(
					rb::BOX_X + rb::button_at(list, i).0,
					rb::BOX_Y + 3 + n,
					r.buf.get(..r.n).unwrap_or(&[]),
					a,
				);
			}
			cursor = None;
		}
		Mode::Confirm(_, focus) => {
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
			for (i, (x, t)) in [
				(16, &b"< Yes >"[..]),
				(30, &b"< No >"[..]),
				(43, &b"< Cancel >"[..]),
			]
			.iter()
			.enumerate()
			{
				let a = if usize::from(focus) == i {
					SELECTED
				} else {
					attr::CHROME
				};
				g.text(*x, 12, t, a);
			}
			cursor = None;
		}
		#[cfg(feature = "text-file-dialog")]
		Mode::File => cursor = filepaint::draw(app, s, g, rows),
		Mode::Edit => {}
	}
	cursor
}

/// Draws a field's text in normal video, with the selected part reversed.
fn field_text<const R: usize>(
	g: &mut Grid<R>,
	col: usize,
	row: usize,
	text: &[u8],
	caret: &quill_dlg::widgets::Caret,
) {
	g.text(col, row, text, 0x07);
	let (s, e) = caret.selection();
	if let Some(sel) = text.get(s..e.min(text.len())).filter(|t| !t.is_empty()) {
		g.text(col + s, row, sel, 0x70);
	}
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

/// Counters for numbered lists: nine levels for each of four lists, kept across the lines of one pass over the document.
pub type ListCounters = [[u32; 9]; 4];

/// The list label of a line, when it is the first line of a list paragraph: the label bytes (a bullet is 0x07 as in CP437), their
/// number, and the x the label starts at in device units at `rx` units per inch. Call it for every line in order, so the counters
/// stay right.
pub fn line_label<M: Measure + Clone, S: ReadStore + ?Sized>(
	app: &App<M>,
	s: &S,
	l: &quill_core::layout::Line,
	counters: &mut ListCounters,
	rx: u16,
) -> Option<([u8; 16], usize, usize)> {
	if l.line != 0 {
		return None;
	}
	let pf = app.para_pfmt(s, l.start);
	if pf.flags & 5 == 0 {
		return None;
	}
	let lvl = usize::from(pf.level.min(8));
	let list = usize::from(pf.list).min(3);
	let def = app.doc.lists.get(s, pf.list).unwrap_or_default();
	let lv = def.levels.get(lvl).copied().unwrap_or_default();
	let mut label = [0u8; 16];
	let mut n = 0;
	if pf.flags & 1 != 0 {
		label[0] = 0x07;
		n = 1;
	} else if let Some(c) = counters.get_mut(list) {
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
	let first = quill_core::layout::to_units(pf.left + pf.first, rx).max(0) as usize;
	Some((label, n, first))
}

/// The short name of a command, as a tooltip on its toolbar button shows it.
pub fn command_tip(c: Command) -> &'static str {
	match c {
		Command::New => "New",
		Command::Open => "Open",
		Command::Save => "Save",
		Command::SaveAs => "Save As",
		Command::Print => "Print",
		Command::Preview => "Print Preview",
		Command::PrintSetup => "Print Setup",
		Command::Exit => "Exit",
		Command::Cut => "Cut",
		Command::Copy => "Copy",
		Command::Paste => "Paste",
		Command::Undo => "Undo",
		Command::Redo => "Redo",
		Command::SelectAll => "Select All",
		Command::Spelling => "Spelling",
		Command::SpellAsYouType => "Check Spelling as You Type",
		Command::Find => "Find",
		Command::FindNext => "Find Next",
		Command::Replace => "Replace",
		Command::Bold => "Bold",
		Command::Italic => "Italic",
		Command::Underline => "Underline",
		Command::Strike => "Strikethrough",
		Command::Super => "Superscript",
		Command::Sub => "Subscript",
		Command::Regular => "Regular",
		Command::Fonts => "Fonts",
		Command::Align(0) => "Align Left",
		Command::Align(1) => "Centre",
		Command::Align(2) => "Align Right",
		Command::Align(_) => "Justify",
		Command::Spacing(0) => "Single Space",
		Command::Spacing(1) => "1\u{bd} Space",
		Command::Spacing(_) => "Double Space",
		Command::Bullets => "Bullets",
		Command::Numbering => "Numbering",
		Command::LevelUp => "Increase Level",
		Command::LevelDown => "Decrease Level",
		Command::Tabs => "Tabs",
		Command::Picture => "Picture",
		Command::Field(1) => "Page Number",
		Command::Field(2) => "Page Count",
		Command::Field(3) => "Date",
		Command::Field(4) => "File Name",
		Command::Field(_) => "Time",
		Command::Help => "Help Contents",
		Command::About => "About Quill",
		_ => "",
	}
}

/// The tooltip of the drop-down boxes on the formatting bar: 0 the style box, 1 the font box, anything else the size box.
pub fn combo_tip(i: u8) -> &'static str {
	match i {
		0 => "Style",
		1 => "Font",
		_ => "Font Size",
	}
}

/// The status-line sentence for the drop-down boxes, by the same numbers as [`combo_tip`].
pub fn combo_hint(i: u8) -> &'static str {
	match i {
		0 => "Choose a paragraph style",
		1 => "Choose a font",
		_ => "Choose a size for the text",
	}
}

/// The tooltip and the status-line sentence of the text colour button.
pub const COLOUR_TIP: &str = "Font Colour";
/// The status-line sentence of the text colour button.
pub const COLOUR_HINT: &str = "Choose a colour for the selected text";

/// The sentence the status line shows while a menu item or a toolbar button is under the pointer.
pub fn command_hint(c: Command) -> &'static str {
	match c {
		Command::New => "Start a new, empty document",
		Command::Open => "Open an existing document",
		Command::Save => "Save the document under its own name",
		Command::SaveAs => "Save the document under a new name",
		Command::Print => "Print the document",
		Command::Preview => "Show the pages as they will print",
		Command::PrintSetup => "Choose the printer and the paper",
		Command::Exit => "Quit Quill",
		Command::Recent(_) => "Open this document",
		Command::Cut => "Move the selection to the clipboard",
		Command::Copy => "Copy the selection to the clipboard",
		Command::Paste => "Insert the clipboard's contents at the caret",
		Command::Undo => "Undo the last change",
		Command::Redo => "Redo the change that was undone",
		Command::SelectAll => "Select the whole document",
		Command::Spelling => "Check the spelling of the document",
		Command::SpellAsYouType => "Underline misspelled words as you type",
		Command::Find => "Find text in the document",
		Command::FindNext => "Find the next match",
		Command::Replace => "Find text and replace it",
		Command::Bold => "Make the selection bold",
		Command::Italic => "Make the selection italic",
		Command::Underline => "Underline the selection",
		Command::Strike => "Strike through the selection",
		Command::Super => "Raise the selection above the line",
		Command::Sub => "Lower the selection below the line",
		Command::Regular => "Remove bold, italic, underline and the other effects",
		Command::Fonts => "Choose the font, style and size",
		Command::Align(0) => "Align the paragraph with the left margin",
		Command::Align(1) => "Centre the paragraph between the margins",
		Command::Align(2) => "Align the paragraph with the right margin",
		Command::Align(_) => "Spread the paragraph between both margins",
		Command::Spacing(0) => "Single-space the paragraph",
		Command::Spacing(1) => "Space the paragraph at one and a half lines",
		Command::Spacing(_) => "Double-space the paragraph",
		Command::Bullets => "Start or stop a bulleted list",
		Command::Numbering => "Start or stop a numbered list",
		Command::LevelUp => "Move the list item in a level",
		Command::LevelDown => "Move the list item out a level",
		Command::Tabs => "Set the tab stops",
		Command::Picture => "Insert a picture from a file",
		Command::Field(1) => "Insert the number of the page",
		Command::Field(2) => "Insert the number of pages",
		Command::Field(3) => "Insert today's date",
		Command::Field(4) => "Insert the name of the file",
		Command::Field(_) => "Insert the time",
		Command::View(0) => "Go back to the body of the document",
		Command::View(_) => "Edit the text that repeats on every page",
		Command::FirstPage => "Give the first page its own header and footer",
		Command::Chrome(1) => "Show or hide the toolbar",
		Command::Chrome(2) => "Show or hide the ruler",
		Command::Chrome(4) => "Show or hide the status bar",
		Command::Chrome(_) => "Show or hide the formatting bar",
		Command::Zoom(0) => "Fit the page to the width of the window",
		Command::Zoom(_) => "Show the page larger or smaller",
		Command::Help => "Open the help contents",
		Command::About => "Show the version and the licence",
	}
}
