// The pointer: clicks, drags, double clicks and the wheel turned into the app's own calls, on the text screen's cells or on pixels.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A shell hands [`Pointer::handle`] the [`Ev`]s its pointer makes, in its own units. Where the units are the text screen's cells the
//! shell passes [`TextCells`]; a graphics shell passes its own [`PointerTarget`], which says what is under a point. The page is
//! pointed at the way the Windows shells do: a click places the caret, Shift extends, a drag selects, a double click takes a word,
//! a triple click a paragraph, and a click in the white space left of the text takes a line.
//!
//! A menu or a dialog that is open takes the click instead of the page. On the text screen that is [`TextCells`]: the menu bar's
//! titles, the items of the open menu, and the buttons, check boxes, choices and suggestions of the dialogs, each found from the
//! cell the way `draw` put it there. The text fields keep the caret at the end of what is typed, so a click in one only gives it the
//! focus.

use quill_core::layout::{to_units, Measure};
use quill_core::store::{ReadStore, Store};
use quill_grid::COLS;
pub use quill_mouse::Ev;

use crate::paint::{self, MENU_COUNT};
use crate::view;
use crate::{App, Files, Grain, Key, Mode};

/// What the shell knows when an event comes in.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Ctx {
	/// A Shift key is held.
	pub shift: bool,
	/// The clock, in ticks (the BIOS tick on DOS, 18.2 a second); a second press within `DOUBLE_TICKS` of the first is a double click.
	pub tick: u32,
}

/// Ticks between the presses of a double click: half a second at 18.2 ticks a second, Windows' default.
pub const DOUBLE_TICKS: u32 = 9;

/// A point in the page: the position of the character nearest it, and whether it is in the white space left of the text.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct PageHit {
	pub pos: u32,
	pub margin: bool,
}

/// What is under a point of a shell's screen. A shell implements this for its own units; the page and the wheel are the same on all.
pub trait PointerTarget<M: Measure + Clone> {
	/// The page position under (`x`, `y`), or `None` when the point is not in the page area (the menu bar, the ruler, the status line).
	fn page_hit<S: ReadStore + ?Sized>(
		&self,
		app: &App<M>,
		s: &S,
		x: i32,
		y: i32,
	) -> Option<PageHit>;

	/// How far a press may stray from the last to count as the same place for a double click.
	fn slop(&self) -> i32 {
		0
	}

	/// How far above (-1) or below (+1) the page area `y` is, or 0 inside it: a drag past an edge scrolls.
	fn edge(&self, _y: i32) -> i32 {
		0
	}

	/// `y` brought inside the page area, for a drag that went past its edge.
	fn clamp_y(&self, y: i32) -> i32 {
		y
	}

	/// A press outside the page while the app is editing (the menu bar). True when it was used.
	fn press_chrome<S: Store + ?Sized, F: Files>(
		&self,
		_app: &mut App<M>,
		_s: &mut S,
		_f: &mut F,
		_x: i32,
		_y: i32,
	) -> bool {
		false
	}

	/// A press while a menu or dialog is open.
	fn press_overlay<S: Store + ?Sized, F: Files>(
		&self,
		_app: &mut App<M>,
		_s: &mut S,
		_f: &mut F,
		_x: i32,
		_y: i32,
	) {
	}

	/// The pointer moved with no button held and a menu is open.
	fn hover_overlay(&self, _app: &mut App<M>, _x: i32, _y: i32) {}
}

#[derive(Copy, Clone, Debug)]
struct Click {
	tick: u32,
	x: i32,
	y: i32,
	count: u8,
}

#[derive(Copy, Clone, Debug)]
struct Sel {
	start: (u32, u32),
	grain: Grain,
}

/// The pointer's state: the last press, for double and triple clicks, and the selection a drag is growing.
#[derive(Copy, Clone, Debug, Default)]
pub struct Pointer {
	click: Option<Click>,
	sel: Option<Sel>,
	x: i32,
	y: i32,
}

/// Lines a notch of the wheel scrolls.
pub const WHEEL_LINES: i32 = 3;

impl Pointer {
	/// Where the pointer was last, in the shell's units.
	pub fn at(&self) -> (i32, i32) {
		(self.x, self.y)
	}

	/// True while a drag is selecting.
	pub fn dragging(&self) -> bool {
		self.sel.is_some()
	}

	/// Takes one event.
	pub fn handle<M: Measure + Clone, S: Store + ?Sized, F: Files, T: PointerTarget<M>>(
		&mut self,
		app: &mut App<M>,
		s: &mut S,
		f: &mut F,
		t: &T,
		ev: Ev,
		ctx: Ctx,
	) {
		match ev {
			Ev::Down { x, y } => {
				self.x = x;
				self.y = y;
				self.down(app, s, f, t, ctx);
			}
			Ev::Move { x, y } => {
				self.x = x;
				self.y = y;
				if self.sel.is_some() {
					self.drag(app, s, t);
				} else if !app.editing() {
					t.hover_overlay(app, x, y);
				}
			}
			Ev::Up { x, y } => {
				self.x = x;
				self.y = y;
				self.sel = None;
			}
			Ev::Wheel(n) => {
				if app.editing() && app.pixel_view() {
					app.vscroll_by(s, n.saturating_mul(WHEEL_LINES * view::SCROLL_LINE));
				} else if app.editing() {
					let top = i64::from(app.top_line()) + i64::from(n) * i64::from(WHEEL_LINES);
					app.scroll_to(s, top.clamp(0, i64::from(u32::MAX)) as u32);
				}
			}
			Ev::Tick => {}
		}
	}

	fn down<M: Measure + Clone, S: Store + ?Sized, F: Files, T: PointerTarget<M>>(
		&mut self,
		app: &mut App<M>,
		s: &mut S,
		f: &mut F,
		t: &T,
		ctx: Ctx,
	) {
		let (x, y) = (self.x, self.y);
		self.sel = None;
		if !app.editing() {
			self.click = None;
			t.press_overlay(app, s, f, x, y);
			return;
		}
		let Some(hit) = t.page_hit(app, s, x, y) else {
			self.click = None;
			t.press_chrome(app, s, f, x, y);
			return;
		};
		let slop = t.slop();
		let count = match self.click {
			Some(c)
				if ctx.tick.wrapping_sub(c.tick) <= DOUBLE_TICKS
					&& (c.x - x).abs() <= slop
					&& (c.y - y).abs() <= slop =>
			{
				c.count.min(2) + 1
			}
			_ => 1,
		};
		self.click = Some(Click {
			tick: ctx.tick,
			x,
			y,
			count,
		});
		let grain = match (hit.margin, count) {
			(true, 1) => Grain::Line,
			(true, _) => Grain::Paragraph,
			(false, 1) => Grain::Char,
			(false, 2) => Grain::Word,
			(false, _) => Grain::Paragraph,
		};
		let start = if grain == Grain::Char {
			(hit.pos, hit.pos)
		} else {
			app.grain_range(s, hit.pos, grain)
		};
		if grain == Grain::Char {
			app.place_caret(s, hit.pos, ctx.shift);
		} else {
			app.select_range(s, start.0, start.1);
		}
		self.sel = Some(Sel {
			start: if grain == Grain::Char && ctx.shift {
				app.selection().unwrap_or(start)
			} else {
				start
			},
			grain,
		});
	}

	fn drag<M: Measure + Clone, S: Store + ?Sized, T: PointerTarget<M>>(
		&mut self,
		app: &mut App<M>,
		s: &mut S,
		t: &T,
	) {
		let Some(sel) = self.sel else {
			return;
		};
		// Past the top or bottom of the page area the page scrolls a line, and the selection grows to the line at the edge.
		let edge = t.edge(self.y);
		if edge != 0 {
			let top = i64::from(app.top_line()) + i64::from(edge);
			app.scroll_to(s, top.clamp(0, i64::from(u32::MAX)) as u32);
		}
		let y = t.clamp_y(self.y);
		if let Some(hit) = t.page_hit(app, s, self.x, y) {
			app.drag_select(s, sel.start, hit.pos, sel.grain);
		}
	}
}

// ---- the text screen ----------------------------------------------------------------------------------------------------------------

/// The text screen's cells as a pointer space: column and row of an 80 column screen.
#[derive(Copy, Clone, Debug)]
pub struct TextCells {
	/// The rows of page between the menu bar and the status line.
	pub rows: i32,
}

impl<M: Measure + Clone> App<M> {
	/// The character position nearest the cell (`col`, `row`) of the text screen, as `draw` lays the page out: row 1 is the first row
	/// under the menu bar. Never fails.
	pub fn pos_at_cell<S: ReadStore + ?Sized>(&self, s: &S, col: i32, row: i32) -> u32 {
		let (rx, ry) = self.measure.resolution();
		let margin = self.cell_margin();
		let page_rows = to_units(self.doc.page.text_h(), 6);
		let cum = |l: &quill_core::layout::Line| (l.page as i32 - 1) * page_rows + l.y;
		let top_cum = self.line_at(s, self.top).map_or(0, |l| cum(&l));
		let y = top_cum + (row - 1) + to_units(self.doc.page.margin_t, ry);
		let x = col - margin + to_units(self.doc.page.margin_l, rx);
		self.pos_at_point(s, x, y, page_rows)
	}

	/// The column the text starts at on the text screen.
	fn cell_margin(&self) -> i32 {
		let text_w = self.doc.page.text_w().saturating_mul(10) / 1440;
		(COLS as i32 - text_w).max(0) / 2
	}

	/// Opens or closes the menus at the menu bar's title under `col`. True when there was one.
	fn bar_press(&mut self, col: i32) -> bool {
		for m in 0..MENU_COUNT {
			let title = paint::menu_title(m);
			let len = title.iter().filter(|c| **c != b'&').count() as i32;
			let at = paint::MENU_COLS.get(usize::from(m)).copied().unwrap_or(1) as i32;
			if col >= at - 1 && col <= at + len && !paint::menu_items(m).is_empty() {
				self.mode = match self.mode {
					Mode::Menu { menu, .. } if menu == m => Mode::Edit,
					_ => Mode::Menu { menu: m, item: 0 },
				};
				return true;
			}
		}
		false
	}

	/// The slot of the open menu at `row`, counting the rules that take a row of their own; `None` for the frame, a rule or outside.
	fn menu_slot(&self, menu: u8, col: i32, row: i32) -> Option<u8> {
		let items = paint::menu_items_for(menu, !self.native);
		let recents = if menu == 0 {
			self.recent_files().count()
		} else {
			0
		};
		let (bx, w, rows) = paint::menu_box(menu, !self.native, recents);
		let (bx, w, rows) = (bx as i32, w as i32, rows as i32);
		if col <= bx || col >= bx + w - 1 || row < 2 || row >= 1 + rows - 1 {
			return None;
		}
		let mut y = 2;
		for i in 0..items.len() + recents {
			match paint::slot(items.len(), recents, i) {
				paint::Slot::Item(k) => {
					if items.get(k).is_some_and(|it| it.rule) {
						y += 1;
					}
				}
				paint::Slot::Recent(k) => {
					if k == 0 {
						y += 1;
					}
				}
			}
			if y == row {
				return Some(i as u8);
			}
			y += 1;
		}
		None
	}

	/// The press on the text screen's cell while a menu or dialog is open.
	fn cell_press<S: Store + ?Sized, F: Files>(
		&mut self,
		s: &mut S,
		f: &mut F,
		col: i32,
		row: i32,
	) {
		let at = |x: i32, y: i32, w: usize| row == y && col >= x && col < x + w as i32;
		match self.mode {
			Mode::Menu { menu, .. } => {
				if row == 0 {
					if !self.bar_press(col) {
						self.mode = Mode::Edit;
					}
				} else if let Some(i) = self.menu_slot(menu, col, row) {
					self.mode = Mode::Menu { menu, item: i };
					self.key(s, f, Key::Enter);
				} else {
					let recents = if menu == 0 {
						self.recent_files().count()
					} else {
						0
					};
					let (bx, w, rows) = paint::menu_box(menu, !self.native, recents);
					let inside = col >= bx as i32
						&& col < (bx + w) as i32
						&& row >= 1 && row < 1 + rows as i32;
					// A click on the frame or a rule does nothing; one outside the box closes the menu.
					if !inside {
						self.mode = Mode::Edit;
					}
				}
			}
			Mode::Prompt { .. } => {
				if at(24, 12, 6) {
					self.key(s, f, Key::Enter);
				} else if at(40, 12, 10) {
					self.key(s, f, Key::Esc);
				}
			}
			Mode::Confirm(c, _) => {
				for (i, (x, w)) in [(16, 7usize), (30, 6), (43, 10)].iter().enumerate() {
					if at(*x, 12, *w) {
						self.mode = Mode::Confirm(c, i as u8);
						self.key(s, f, Key::Enter);
						return;
					}
				}
			}
			Mode::About => {
				if at(34, 16, 6) {
					self.key(s, f, Key::Enter);
				}
			}
			Mode::Find(mut st) => {
				use crate::find::Focus;
				let (case_y, buttons): (i32, &[(usize, Focus)]) = if st.replace {
					(
						12,
						&[
							(13, Focus::FindNext),
							(11, Focus::ReplaceOne),
							(15, Focus::ReplaceAll),
							(9, Focus::Close),
						],
					)
				} else {
					(11, &[(13, Focus::FindNext), (9, Focus::Close)])
				};
				if at(24, 10, 34) {
					st.focus = Focus::Find;
					self.mode = Mode::Find(st);
				} else if st.replace && at(24, 11, 34) {
					st.focus = Focus::Replace;
					self.mode = Mode::Find(st);
				} else if at(10, case_y, 14) {
					st.focus = Focus::Case;
					self.mode = Mode::Find(st);
					self.key(s, f, Key::Enter);
				} else if at(30, case_y, 25) {
					st.focus = Focus::Word;
					self.mode = Mode::Find(st);
					self.key(s, f, Key::Enter);
				} else {
					let mut x = 10;
					for (len, focus) in buttons {
						let w = *len;
						if at(x, case_y + 2, w) {
							st.focus = *focus;
							self.mode = Mode::Find(st);
							self.key(s, f, Key::Enter);
							return;
						}
						x += w as i32 + 2;
					}
				}
			}
			Mode::Form(mut fm) => {
				for i in 0..fm.n {
					if !at(10, 10 + i as i32, 48) {
						continue;
					}
					let kind = fm.rows.get(i).map_or(3, |r| r.kind);
					fm.focus = i;
					self.mode = Mode::Form(fm);
					match kind {
						1 | 2 => self.key(s, f, Key::Char(b' ')),
						3 => self.key(s, f, Key::Enter),
						_ => {}
					}
					return;
				}
			}
			Mode::Spell(mut d) => {
				for i in 0..usize::from(d.n) {
					let len = usize::from(d.lens.get(i).copied().unwrap_or(0));
					if at(17 + (i as i32 / 4) * 20, 13 + i as i32 % 4, len) {
						d.sel = i as u8;
						self.mode = Mode::Spell(d);
						return;
					}
				}
				let mut x = 4;
				for (i, b) in crate::spelling::BUTTONS.iter().enumerate() {
					let w = b.len() + 4;
					if at(x, 18, w) {
						d.focus = i as u8;
						self.mode = Mode::Spell(d);
						self.key(s, f, Key::Enter);
						return;
					}
					x += w as i32 + 1;
				}
			}
			Mode::Edit => {}
		}
	}
}

impl<M: Measure + Clone> PointerTarget<M> for TextCells {
	fn page_hit<S: ReadStore + ?Sized>(
		&self,
		app: &App<M>,
		s: &S,
		x: i32,
		y: i32,
	) -> Option<PageHit> {
		if y < 1 || y > self.rows {
			return None;
		}
		Some(PageHit {
			pos: app.pos_at_cell(s, x, y),
			margin: x < app.cell_margin(),
		})
	}

	fn edge(&self, y: i32) -> i32 {
		if y < 1 {
			-1
		} else if y > self.rows {
			1
		} else {
			0
		}
	}

	fn clamp_y(&self, y: i32) -> i32 {
		y.clamp(1, self.rows.max(1))
	}

	fn press_chrome<S: Store + ?Sized, F: Files>(
		&self,
		app: &mut App<M>,
		_s: &mut S,
		_f: &mut F,
		x: i32,
		y: i32,
	) -> bool {
		y == 0 && app.bar_press(x)
	}

	fn press_overlay<S: Store + ?Sized, F: Files>(
		&self,
		app: &mut App<M>,
		s: &mut S,
		f: &mut F,
		x: i32,
		y: i32,
	) {
		app.cell_press(s, f, x, y);
	}

	fn hover_overlay(&self, app: &mut App<M>, x: i32, y: i32) {
		if let Mode::Menu { menu, .. } = app.mode {
			if let Some(i) = app.menu_slot(menu, x, y) {
				app.mode = Mode::Menu { menu, item: i };
			}
		}
	}
}
