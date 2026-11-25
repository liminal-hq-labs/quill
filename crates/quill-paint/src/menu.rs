// The menu bar's titles and the pull-down menus: where they are, and how they are drawn.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The geometry is pure: it takes a function for the width of a character and returns rectangles, so a pointer can hit-test the bar
//! and an open menu with the very numbers the painter draws with ([`title_rects`], [`MenuGeom::item_at`]). The state (which menu is
//! open, which item is lit) is the app's, and the keys drive it; nothing here changes it.

use quill_app::{menu_items_for, menu_title, slot, App, Slot, MENU_COUNT};
use quill_core::layout::Measure;
use quill_core::store::ReadStore;

use crate::shell::{MENU_H, SCREEN_W};
use crate::ui::Ui;
use crate::ui::{bevel, FACE, INK, LIGHT, SELECT, SHADOW};
use crate::{Rect, Rgb, Surface};

/// The space either side of a title.
const TITLE_PAD: i32 = 8;
/// The height of one item.
pub const ITEM_H: i32 = 20;
/// The height a separator rule takes.
pub const RULE_H: i32 = 8;
/// The space left of an item's label for its check mark.
const CHECK_W: i32 = 22;
/// The most places an open menu has: its items, the recent files and nothing else.
const MAX_ROWS: usize = 20;

/// The rectangle of each title on the menu bar, given the width of a character in the interface font. A title's rectangle is
/// the bar's full height less its bottom rule, so a click anywhere in it opens the menu.
pub fn title_rects(mut adv: impl FnMut(u8) -> i32) -> [Rect; MENU_COUNT as usize] {
	let mut out = [Rect::default(); MENU_COUNT as usize];
	let mut x = 2;
	for (m, r) in out.iter_mut().enumerate() {
		let w: i32 = menu_title(m as u8)
			.iter()
			.filter(|c| **c != b'&')
			.map(|c| adv(*c))
			.sum();
		*r = Rect::new(x, 0, x + w + 2 * TITLE_PAD, MENU_H - 1);
		x += w + 2 * TITLE_PAD;
	}
	out
}

/// One place in an open menu.
#[derive(Copy, Clone, Debug)]
pub struct Row {
	/// The rectangle that lights when the place is chosen and takes a click.
	pub rect: Rect,
	/// A rule is drawn above it.
	pub rule: bool,
	/// What it holds.
	pub slot: Slot,
}

/// Where an open menu and each of its places are.
pub struct MenuGeom {
	/// The menu, border included.
	pub rect: Rect,
	rows: [Row; MAX_ROWS],
	n: usize,
}

impl MenuGeom {
	/// The places, in the order the app counts them (`menu_open`'s item).
	pub fn rows(&self) -> &[Row] {
		self.rows.get(..self.n).unwrap_or(&[])
	}

	/// The rectangle of place `i`.
	pub fn item_rect(&self, i: usize) -> Option<Rect> {
		self.rows().get(i).map(|r| r.rect)
	}

	/// The place at (`x`, `y`), if the point is on one.
	pub fn item_at(&self, x: i32, y: i32) -> Option<usize> {
		self.rows().iter().position(|r| {
			x >= r.rect.left && x < r.rect.right && y >= r.rect.top && y < r.rect.bottom
		})
	}

	/// True when the point is inside the menu, border and rules included.
	pub fn contains(&self, x: i32, y: i32) -> bool {
		x >= self.rect.left && x < self.rect.right && y >= self.rect.top && y < self.rect.bottom
	}
}

/// Lays out menu `menu` under its title (`title` is the title's rectangle): `recents` are the recent files' names (File only),
/// `text` says the menu is the DOS one, and `adv` is the width of a character.
pub fn menu_geom(
	menu: u8,
	text: bool,
	title: Rect,
	recents: &[&[u8]],
	mut adv: impl FnMut(u8) -> i32,
) -> MenuGeom {
	let items = menu_items_for(menu, text);
	let mut width = 0;
	let mut w_of = |t: &[u8]| -> i32 { t.iter().filter(|c| **c != b'&').map(|c| adv(*c)).sum() };
	for it in items {
		let mut w = w_of(it.label.as_bytes()) + 16;
		if !it.accel.is_empty() {
			w += 24 + w_of(it.accel.as_bytes());
		} else if !it.sub.is_empty() {
			w += 16;
		}
		width = width.max(w);
	}
	for (k, n) in recents.iter().enumerate() {
		let _ = k;
		width = width.max(w_of(n) + 16 + w_of(b"1  "));
	}
	let width = width + CHECK_W + 4;
	let left = title.left.min(SCREEN_W - width - 1).max(0);
	let mut g = MenuGeom {
		rect: Rect::default(),
		rows: [Row {
			rect: Rect::default(),
			rule: false,
			slot: Slot::Item(0),
		}; MAX_ROWS],
		n: 0,
	};
	let mut y = title.bottom + 1 + 2;
	for i in 0..items.len() + recents.len() {
		let at = slot(items.len(), recents.len(), i);
		let rule = match at {
			Slot::Item(k) => items.get(k).is_some_and(|it| it.rule),
			Slot::Recent(k) => k == 0,
		};
		if rule {
			y += RULE_H;
		}
		if let Some(r) = g.rows.get_mut(i) {
			*r = Row {
				rect: Rect::new(left + 2, y, left + width - 2, y + ITEM_H),
				rule,
				slot: at,
			};
			g.n = i + 1;
		}
		y += ITEM_H;
	}
	g.rect = Rect::new(left, title.bottom + 1, left + width, y + 2);
	g
}

/// Draws the bar's titles; the one whose menu is open (`open`) is lit.
pub fn draw_bar<Sf: Surface + ?Sized>(sf: &mut Sf, ui: &Ui<Sf::Font>, open: Option<u8>) {
	let bar = Rect::new(0, 0, SCREEN_W, MENU_H);
	sf.clip(Some(bar));
	sf.fill_rect(bar, FACE);
	sf.hline(0, SCREEN_W, MENU_H - 1, INK);
	let rects = title_rects(|c| sf.advance(ui.font, c));
	let y = (MENU_H - 1 - ui.height) / 2;
	for (m, r) in rects.iter().enumerate() {
		let on = open == Some(m as u8);
		if on {
			sf.fill_rect(*r, SELECT);
		}
		ui.label(
			sf,
			r.left + TITLE_PAD,
			y,
			menu_title(m as u8),
			if on { LIGHT } else { INK },
		);
	}
	sf.clip(None);
}

/// A check mark, 7 by 7, with its left top corner at (`x`, `y`).
fn check_mark<Sf: Surface + ?Sized>(sf: &mut Sf, x: i32, y: i32, c: Rgb) {
	for i in 0..3 {
		sf.hline(x + i, x + i + 2, y + 3 + i, c);
	}
	for i in 0..5 {
		sf.hline(x + 3 + i, x + 5 + i, y + 5 - i, c);
	}
}

/// The geometry of menu `menu` as `app` shows it now (its recent files included), from the width of a character in the interface
/// font. This is what [`draw_menu`] draws with and what a pointer hits, so the two cannot disagree.
pub fn geom_of<M: Measure + Clone>(
	app: &App<M>,
	menu: u8,
	mut adv: impl FnMut(u8) -> i32,
) -> Option<MenuGeom> {
	let mut names: [&[u8]; 4] = [&[]; 4];
	let mut count = 0;
	if menu == 0 {
		for (slot, name) in names.iter_mut().zip(app.recent_files()) {
			*slot = name;
			count += 1;
		}
	}
	let recents = names.get(..count).unwrap_or(&[]);
	let titles = title_rects(&mut adv);
	let title = titles.get(usize::from(menu)).copied()?;
	Some(menu_geom(menu, true, title, recents, adv))
}

/// Draws an open menu: `item` is the lit place. Greyed items and check marks come from the app.
pub fn draw_menu<Sf: Surface + ?Sized, M: Measure + Clone, S: ReadStore + ?Sized>(
	sf: &mut Sf,
	ui: &Ui<Sf::Font>,
	app: &App<M>,
	s: &S,
	menu: u8,
	item: u8,
) {
	let text = true;
	let Some(g) = geom_of(app, menu, |c| sf.advance(ui.font, c)) else {
		return;
	};
	let mut names: [&[u8]; 4] = [&[]; 4];
	let mut count = 0;
	if menu == 0 {
		for (slot, name) in names.iter_mut().zip(app.recent_files()) {
			*slot = name;
			count += 1;
		}
	}
	let recents = names.get(..count).unwrap_or(&[]);
	sf.clip(None);
	sf.fill_rect(g.rect, FACE);
	crate::ui::outline(sf, g.rect, INK);
	bevel(
		sf,
		Rect::new(
			g.rect.left + 1,
			g.rect.top + 1,
			g.rect.right - 1,
			g.rect.bottom - 1,
		),
		true,
	);
	let items = menu_items_for(menu, text);
	for (i, row) in g.rows().iter().enumerate() {
		let lit = i == usize::from(item);
		if row.rule {
			let y = row.rect.top - RULE_H / 2;
			sf.hline(g.rect.left + 2, g.rect.right - 2, y - 1, SHADOW);
			sf.hline(g.rect.left + 2, g.rect.right - 2, y, LIGHT);
		}
		let r = row.rect;
		let ty = r.top + (r.height() - ui.height) / 2;
		match row.slot {
			Slot::Item(k) => {
				let Some(it) = items.get(k) else { continue };
				let mark = app.menu_mark(s, it.command);
				let mark = if it.sub.is_empty() {
					mark
				} else {
					Default::default()
				};
				let fg = if mark.disabled {
					if lit {
						FACE
					} else {
						SHADOW
					}
				} else if lit {
					LIGHT
				} else {
					INK
				};
				if lit {
					sf.fill_rect(r, SELECT);
				}
				if mark.checked {
					check_mark(sf, r.left + 6, r.top + (ITEM_H - 7) / 2, fg);
				}
				ui.label(sf, r.left + CHECK_W, ty, it.label.as_bytes(), fg);
				if it.sub.is_empty() {
					let w = ui.width(sf, it.accel.as_bytes());
					ui.text(sf, r.right - 10 - w, ty, it.accel.as_bytes(), fg);
				} else {
					// An arrow for a submenu.
					for d in 0..4 {
						sf.vline(r.right - 12 + d, r.top + 6 + d, r.bottom - 6 - d, fg);
					}
				}
			}
			Slot::Recent(k) => {
				let fg = if lit { LIGHT } else { INK };
				if lit {
					sf.fill_rect(r, SELECT);
				}
				let digit = [b'1' + k as u8];
				let x = ui.label(sf, r.left + CHECK_W, ty, &[b'&', digit[0], b' ', b' '], fg);
				let name = recents.get(k).copied().unwrap_or(&[]);
				ui.text(sf, x, ty, name, fg);
			}
		}
	}
}
