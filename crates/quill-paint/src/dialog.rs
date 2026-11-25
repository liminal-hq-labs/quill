// The app's dialogs drawn as Windows 3.1 windows: a title bar, bevelled buttons, check boxes, radio buttons, edit fields and lists.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! [`quill_app::Overlay`] describes a dialog in text cells; this module turns a cell into pixels ([`CELL_W`] by [`ROW_H`]), puts the
//! window in the middle of the screen and draws each widget. [`widget_rect`] and [`hit`] are pure and use only the cell geometry, so
//! a pointer finds the control under it with the numbers the painter used. The keyboard is the app's: the widget with `focus` set
//! is the only one that shows the dotted focus rectangle or the caret, and a button with `default` set has the heavy border.

use quill_app::{Overlay, Wid, Wk};

use crate::shell::{SCREEN_H, SCREEN_W};
use crate::ui::{bevel, dotted, outline, Ui, FACE, INK, LIGHT, SELECT, SHADOW};
use crate::{Rect, Rgb, Surface};

/// The width of a cell in pixels.
pub const CELL_W: i32 = 8;
/// The pitch of a row in pixels.
pub const ROW_H: i32 = 22;
/// The height of the title bar.
pub const TITLE_H: i32 = 18;
/// The border around the window.
pub const BORDER: i32 = 4;
/// The height of a control.
pub const CTL_H: i32 = 20;
/// The side of a check box and the diameter of a radio button.
pub const BOX: i32 = 13;

/// The window, centred on the screen.
pub fn window_rect(o: &Overlay) -> Rect {
	let w = i32::from(o.w) * CELL_W + 2 * BORDER;
	let h = BORDER + TITLE_H + 8 + (i32::from(o.h) - 1).max(0) * ROW_H + 8 + BORDER;
	let left = ((SCREEN_W - w) / 2).max(0);
	let top = ((SCREEN_H - h) / 2).max(0);
	Rect::new(left, top, left + w, top + h)
}

/// The title bar inside the border.
pub fn title_rect(o: &Overlay) -> Rect {
	let r = window_rect(o);
	Rect::new(
		r.left + BORDER,
		r.top + BORDER,
		r.right - BORDER,
		r.top + BORDER + TITLE_H,
	)
}

/// The rectangle a widget answers clicks in. A check box or a radio button's includes its label (a label of `w` cells).
pub fn widget_rect(o: &Overlay, w: &Wid) -> Rect {
	let win = window_rect(o);
	let x = win.left + BORDER + i32::from(w.col) * CELL_W;
	let y = win.top + BORDER + TITLE_H + 8 + (i32::from(w.row) - 1) * ROW_H;
	match w.kind {
		Wk::Label => Rect::new(x, y, x + i32::from(w.len) * CELL_W, y + CTL_H),
		Wk::Check | Wk::Radio => Rect::new(x, y, x + BOX + 6 + i32::from(w.w) * 7, y + CTL_H),
		_ => Rect::new(x, y, x + i32::from(w.w) * CELL_W, y + CTL_H),
	}
}

/// The widget at (`x`, `y`) that the keyboard can rest on or press (a button, a check box, a radio button, an edit field) or a list
/// entry, as an index into `o.widgets()`; `None` over a label or empty space. Which one to focus is `widgets()[i].id`.
pub fn hit(o: &Overlay, x: i32, y: i32) -> Option<usize> {
	o.widgets().iter().position(|w| {
		let r = widget_rect(o, w);
		w.kind != Wk::Label && x >= r.left && x < r.right && y >= r.top && y < r.bottom
	})
}

fn button<Sf: Surface + ?Sized>(sf: &mut Sf, ui: &Ui<Sf::Font>, r: Rect, w: &Wid, down: bool) {
	sf.fill_rect(r, FACE);
	if down {
		// Held by the pointer: one outline, the bevel sunk, and the label a pixel down and right.
		outline(sf, r, INK);
		bevel(
			sf,
			Rect::new(r.left + 1, r.top + 1, r.right - 1, r.bottom - 1),
			false,
		);
	} else if w.default || w.focus {
		outline(sf, r, INK);
		let inner = Rect::new(r.left + 1, r.top + 1, r.right - 1, r.bottom - 1);
		outline(sf, inner, INK);
		bevel(
			sf,
			Rect::new(
				inner.left + 1,
				inner.top + 1,
				inner.right - 1,
				inner.bottom - 1,
			),
			true,
		);
	} else {
		outline(sf, r, INK);
		bevel(
			sf,
			Rect::new(r.left + 1, r.top + 1, r.right - 1, r.bottom - 1),
			true,
		);
	}
	let tw = ui.label_width(sf, w.bytes());
	let off = i32::from(down);
	let x = r.left + (r.width() - tw) / 2 + off;
	let y = r.top + (r.height() - ui.height) / 2 + off;
	ui.label(sf, x, y, w.bytes(), INK);
	if w.focus {
		dotted(
			sf,
			Rect::new(r.left + 4, r.top + 4, r.right - 4, r.bottom - 4),
			INK,
		);
	}
}

fn edit<Sf: Surface + ?Sized>(sf: &mut Sf, ui: &Ui<Sf::Font>, r: Rect, w: &Wid) {
	sf.fill_rect(r, LIGHT);
	bevel(sf, r, false);
	sf.hline(r.left + 1, r.right - 1, r.top + 1, INK);
	sf.vline(r.left + 1, r.top + 1, r.bottom - 1, INK);
	let inner = Rect::new(r.left + 2, r.top + 2, r.right - 2, r.bottom - 2);
	let text = w.bytes();
	// A field narrower than its text shows the end of it, where the caret is.
	let room = inner.width() - 6;
	let mut skip = 0;
	while skip < text.len() && ui.width(sf, text.get(skip..).unwrap_or(&[])) > room {
		skip += 1;
	}
	let shown = text.get(skip..).unwrap_or(&[]);
	let y = r.top + (r.height() - ui.height) / 2;
	sf.clip(Some(inner));
	let end = ui.text(sf, inner.left + 3, y, shown, INK);
	if w.focus {
		let at = usize::from(w.caret).min(text.len()).saturating_sub(skip);
		let x = inner.left + 3 + ui.width(sf, shown.get(..at).unwrap_or(&[]));
		let _ = end;
		sf.vline(x, y, y + ui.height, INK);
	}
	sf.clip(None);
}

fn check<Sf: Surface + ?Sized>(sf: &mut Sf, ui: &Ui<Sf::Font>, r: Rect, w: &Wid) {
	let by = r.top + (r.height() - BOX) / 2;
	let b = Rect::new(r.left, by, r.left + BOX, by + BOX);
	sf.fill_rect(b, LIGHT);
	bevel(sf, b, false);
	sf.hline(b.left + 1, b.right - 1, b.top + 1, INK);
	sf.vline(b.left + 1, b.top + 1, b.bottom - 1, INK);
	if w.on {
		for i in 0..3 {
			sf.hline(b.left + 3 + i, b.left + 5 + i, b.top + 5 + i, INK);
		}
		for i in 0..5 {
			sf.hline(b.left + 5 + i, b.left + 7 + i, b.top + 7 - i, INK);
		}
	}
	label_of(sf, ui, r, w, r.left + BOX + 6);
}

/// The rows of a 13-pixel circle: the half width at each row.
const CIRCLE: [i32; 13] = [2, 4, 5, 5, 6, 6, 6, 6, 6, 5, 5, 4, 2];

fn radio<Sf: Surface + ?Sized>(sf: &mut Sf, ui: &Ui<Sf::Font>, r: Rect, w: &Wid) {
	let by = r.top + (r.height() - BOX) / 2;
	let cx = r.left + 6;
	for (i, half) in CIRCLE.iter().enumerate() {
		let y = by + i as i32;
		sf.hline(cx - half - 1, cx + half + 2, y, SHADOW);
		sf.hline(cx - half, cx + half + 1, y, LIGHT);
	}
	// The ring is dark above and left, light below and right.
	for (i, half) in CIRCLE.iter().enumerate() {
		let y = by + i as i32;
		sf.fill_rect(
			Rect::new(cx - half - 1, y, cx - half, y + 1),
			if i < 7 { SHADOW } else { LIGHT },
		);
		sf.fill_rect(
			Rect::new(cx + half + 1, y, cx + half + 2, y + 1),
			if i < 6 { SHADOW } else { LIGHT },
		);
	}
	if w.on {
		sf.fill_rect(Rect::new(cx - 2, by + 4, cx + 3, by + 9), INK);
		sf.fill_rect(Rect::new(cx - 3, by + 5, cx + 4, by + 8), INK);
	}
	label_of(sf, ui, r, w, r.left + BOX + 6);
}

/// A check or radio label, with the dotted focus rectangle round it when it has the focus.
fn label_of<Sf: Surface + ?Sized>(sf: &mut Sf, ui: &Ui<Sf::Font>, r: Rect, w: &Wid, x: i32) {
	let y = r.top + (r.height() - ui.height) / 2;
	let end = ui.label(sf, x, y, w.bytes(), INK);
	if w.focus {
		dotted(sf, Rect::new(x - 2, y - 2, end + 2, y + ui.height + 2), INK);
	}
}

/// Draws the dialog over whatever is on the screen.
pub fn draw_dialog<Sf: Surface + ?Sized>(sf: &mut Sf, ui: &Ui<Sf::Font>, o: &Overlay) {
	draw_dialog_held(sf, ui, o, None);
}

/// Draws the dialog with the button `held` (an index into `o.widgets()`) shown pressed.
pub fn draw_dialog_held<Sf: Surface + ?Sized>(
	sf: &mut Sf,
	ui: &Ui<Sf::Font>,
	o: &Overlay,
	held: Option<usize>,
) {
	sf.clip(None);
	let win = window_rect(o);
	sf.fill_rect(win, FACE);
	outline(sf, win, INK);
	bevel(
		sf,
		Rect::new(win.left + 1, win.top + 1, win.right - 1, win.bottom - 1),
		true,
	);
	let t = title_rect(o);
	sf.fill_rect(t, SELECT);
	let tw = ui.width(sf, o.title());
	let ty = t.top + (t.height() - ui.height) / 2;
	ui.text(sf, t.left + (t.width() - tw) / 2, ty, o.title(), Rgb::WHITE);
	// The entries of a list sit in one sunken white box.
	let mut list: Option<Rect> = None;
	for w in o.widgets().iter().filter(|w| w.kind == Wk::ListItem) {
		let r = widget_rect(o, w);
		list = Some(match list {
			Some(l) => Rect::new(
				l.left.min(r.left),
				l.top.min(r.top),
				l.right.max(r.right),
				l.bottom.max(r.bottom),
			),
			None => r,
		});
	}
	if let Some(l) = list {
		let b = Rect::new(l.left - 3, l.top - 2, l.right + 3, l.bottom + 1);
		sf.fill_rect(b, LIGHT);
		bevel(sf, b, false);
	}
	for (i, w) in o.widgets().iter().enumerate() {
		let r = widget_rect(o, w);
		match w.kind {
			Wk::Label => {
				let y = r.top + (r.height() - ui.height) / 2;
				ui.text(sf, r.left, y, w.bytes(), INK);
			}
			Wk::Edit => edit(sf, ui, r, w),
			Wk::Check => check(sf, ui, r, w),
			Wk::Radio => radio(sf, ui, r, w),
			Wk::Button => button(sf, ui, r, w, held == Some(i)),
			Wk::ListItem => {
				let r = Rect::new(r.left, r.top + 1, r.right, r.bottom - 1);
				let y = r.top + (r.height() - ui.height) / 2;
				if w.on {
					sf.fill_rect(r, SELECT);
				} else {
					sf.fill_rect(r, LIGHT);
				}
				ui.text(sf, r.left + 3, y, w.bytes(), if w.on { LIGHT } else { INK });
			}
		}
	}
}
