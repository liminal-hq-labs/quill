// The drop-down lists of the formatting bar: the style, face and size boxes and the colour button's list.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A list drops from under the box (or the colour button) that opened it and is as wide as the box (the colour list has its own
//! width). [`list_rect`], [`item_rect`] and [`item_at`] are pure and use only the [`Layout`], so a pointer hits what the painter
//! drew; the lists' contents come from `quill_app`'s tables, the same ones the other shells use.

use quill_app::widgets::{face_choice, face_choices};
use quill_app::{App, STYLE_NAMES};
use quill_chrome::view::{Act, GLYPH_COLOUR};
use quill_chrome::view::{COLOURS, SIZES, TOOLBAR};
use quill_chrome::zoom_field::{zoom_command, zoom_text, ZOOM_LEVELS, ZOOM_TEXT_MAX};
use quill_core::layout::Measure;
use quill_core::store::{ReadStore, Store};

use crate::layout::Layout;
use crate::toolbar::{box_rect, button_rect, Box3};
use crate::ui::{bevel, outline, Ui, FACE, INK, LIGHT, SELECT};
use crate::zoombox;
use crate::{Rect, Rgb, Surface};

/// The height of one entry.
pub const ROW: i32 = 18;
/// The width of the colour list.
pub const COLOUR_W: i32 = 112;
/// The space the colour list gives its swatches.
const SWATCH_W: i32 = 22;

/// Which list is open.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Pop {
	/// The paragraph styles.
	Style,
	/// The faces.
	Face,
	/// The sizes.
	Size,
	/// Automatic and the sixteen colours.
	Colour,
	/// The levels of the zoom, which drops up from the status line's box.
	Zoom,
}

impl Pop {
	/// The list a formatting bar box opens.
	pub fn of_box(b: Box3) -> Pop {
		match b {
			Box3::Style => Pop::Style,
			Box3::Face => Pop::Face,
			Box3::Size => Pop::Size,
		}
	}

	/// The numbers `combo_tip` and `combo_hint` use for the boxes (the colour button has its own tip).
	pub fn tip_index(self) -> u8 {
		match self {
			Pop::Style => 0,
			Pop::Face => 1,
			_ => 2,
		}
	}

	/// How many entries the list has.
	pub fn count(self) -> usize {
		match self {
			Pop::Style => STYLE_NAMES.len(),
			Pop::Face => face_choices(),
			Pop::Size => SIZES.len(),
			Pop::Colour => COLOURS.len() + 1,
			Pop::Zoom => ZOOM_LEVELS.len(),
		}
	}
}

/// The rectangle of the box or button that opens `pop`, or `None` when its bar is hidden.
pub fn anchor(lay: &Layout, pop: Pop) -> Option<Rect> {
	match pop {
		Pop::Style => box_rect(lay, Box3::Style),
		Pop::Face => box_rect(lay, Box3::Face),
		Pop::Size => box_rect(lay, Box3::Size),
		Pop::Colour => {
			let i = TOOLBAR
				.buttons
				.iter()
				.position(|b| b.glyph == GLYPH_COLOUR && b.act == Act::Colour)?;
			button_rect(lay, i)
		}
		Pop::Zoom => zoombox::field_rect(lay),
	}
}

/// How far the zoom list reaches left of its box: room for `Page Width` and the key beside it.
const ZOOM_LIST_EXTRA: i32 = 52;

/// The key that sets Page Width on this screen (DOS-105), shown at the right of the zoom list's Page Width row.
const ZOOM_KEY: &[u8] = b"Alt+0";

/// The list's rectangle, border included, under its anchor.
pub fn list_rect(lay: &Layout, pop: Pop) -> Option<Rect> {
	let a = anchor(lay, pop)?;
	if pop == Pop::Zoom {
		// The status line is at the foot of the screen: the list rises from the box, wider than it to the left, so `Page Width`
		// has room beside its lit bar and its key (`ZOOM_KEY`) at the right.
		let top = lay.status?.top - pop.count() as i32 * ROW - 4;
		return Some(Rect::new(
			a.left - ZOOM_LIST_EXTRA,
			top,
			a.right,
			lay.status?.top,
		));
	}
	let w = if pop == Pop::Colour {
		COLOUR_W
	} else {
		a.width()
	};
	Some(Rect::new(
		a.left,
		a.bottom,
		a.left + w,
		a.bottom + pop.count() as i32 * ROW + 4,
	))
}

/// The rectangle of entry `i` of a list whose rectangle is `list`.
pub fn item_rect(list: Rect, i: usize) -> Rect {
	let top = list.top + 2 + i as i32 * ROW;
	Rect::new(list.left + 2, top, list.right - 2, top + ROW)
}

/// The entry at (`x`, `y`) of a list with `count` entries.
pub fn item_at(list: Rect, count: usize, x: i32, y: i32) -> Option<usize> {
	if x < list.left + 2 || x >= list.right - 2 || y < list.top + 2 {
		return None;
	}
	let i = ((y - list.top - 2) / ROW) as usize;
	(i < count && y < list.bottom - 2).then_some(i)
}

/// The text of entry `i`, copied into `out`; the length. A size is its number of points.
pub fn item_text(pop: Pop, i: usize, out: &mut [u8; 24]) -> usize {
	let put = |out: &mut [u8; 24], t: &[u8]| {
		let n = t.len().min(out.len());
		if let (Some(d), Some(s)) = (out.get_mut(..n), t.get(..n)) {
			d.copy_from_slice(s);
		}
		n
	};
	match pop {
		Pop::Style => put(out, STYLE_NAMES.get(i).map_or(&[][..], |n| n.as_bytes())),
		Pop::Face => put(out, face_choice(i).map_or(&[][..], |(n, _)| n.as_bytes())),
		Pop::Size => {
			let v = SIZES.get(i).copied().unwrap_or(0);
			let mut tmp = [0u8; 4];
			let mut n = 0;
			let mut d = 1000;
			while d > 0 {
				let digit = (v / d % 10) as u8;
				if n > 0 || digit > 0 || d == 1 {
					if let Some(t) = tmp.get_mut(n) {
						*t = b'0' + digit;
						n += 1;
					}
				}
				d /= 10;
			}
			put(out, tmp.get(..n).unwrap_or(&[]))
		}
		Pop::Colour => match i {
			0 => put(out, b"Automatic"),
			k => put(
				out,
				COLOURS.get(k - 1).map_or(&[][..], |(n, _)| n.as_bytes()),
			),
		},
		Pop::Zoom => {
			let mut text = [0u8; ZOOM_TEXT_MAX];
			let n = zoom_text(ZOOM_LEVELS.get(i).copied().unwrap_or(0), &mut text);
			put(out, text.get(..n).unwrap_or(&[]))
		}
	}
}

/// The swatch colour of colour entry `i` (none for Automatic).
pub fn swatch(i: usize) -> Option<Rgb> {
	let (_, (r, g, b)) = COLOURS.get(i.checked_sub(1)?)?;
	Some(Rgb(*r, *g, *b))
}

/// The entry that stands for what the caret has now, where the list opens lit.
pub fn current<M: Measure + Clone, S: ReadStore + ?Sized>(app: &App<M>, s: &S, pop: Pop) -> usize {
	match pop {
		Pop::Style => app.style_of(s).unwrap_or(0),
		Pop::Face => app.face_index(s).unwrap_or(0),
		Pop::Size => {
			let half = app.char_state(s).size;
			SIZES.iter().position(|p| *p * 2 == half).unwrap_or(0)
		}
		Pop::Colour => match app.char_state(s).colour {
			None => 0,
			Some((r, g, b)) => COLOURS
				.iter()
				.position(|(_, c)| *c == (r, g, b))
				.map_or(0, |k| k + 1),
		},
		// A zoom the list lacks lights nothing.
		Pop::Zoom => app.zoom_field_current().unwrap_or(Pop::Zoom.count()),
	}
}

/// Applies entry `i` of the list to the selection, as choosing it does.
pub fn choose<M: Measure + Clone, S: Store + ?Sized>(
	app: &mut App<M>,
	s: &mut S,
	pop: Pop,
	i: usize,
) {
	match pop {
		Pop::Style => app.apply_style(s, i),
		Pop::Face => {
			if let Some((name, family)) = face_choice(i) {
				app.set_face(s, name.as_bytes(), family);
			}
		}
		Pop::Size => {
			if let Some(p) = SIZES.get(i) {
				app.set_size(s, *p * 2);
			}
		}
		Pop::Colour => app.set_colour(
			s,
			i.checked_sub(1).and_then(|k| COLOURS.get(k)).map(|c| c.1),
		),
		Pop::Zoom => {
			if let Some(quill_app::Command::Zoom(z)) = zoom_command(i) {
				app.set_zoom(z);
			}
		}
	}
}

/// Draws an open list with entry `hot` lit.
pub fn draw<Sf: Surface + ?Sized>(
	sf: &mut Sf,
	ui: &Ui<Sf::Font>,
	lay: &Layout,
	pop: Pop,
	hot: usize,
) {
	let Some(r) = list_rect(lay, pop) else { return };
	sf.clip(None);
	let (fill, ink) = if pop == Pop::Colour {
		(FACE, INK)
	} else {
		(LIGHT, INK)
	};
	sf.fill_rect(r, fill);
	outline(sf, r, INK);
	bevel(
		sf,
		Rect::new(r.left + 1, r.top + 1, r.right - 1, r.bottom - 1),
		pop == Pop::Colour,
	);
	for i in 0..pop.count() {
		let row = item_rect(r, i);
		let lit = i == hot;
		if lit {
			sf.fill_rect(row, SELECT);
		}
		let mut text = [0u8; 24];
		let n = item_text(pop, i, &mut text);
		let y = row.top + (row.height() - ui.height) / 2;
		let mut x = row.left + 3;
		if pop == Pop::Colour {
			if let Some(c) = swatch(i) {
				let sw = Rect::new(x, row.top + 3, x + 14, row.bottom - 3);
				sf.fill_rect(sw, c);
				outline(sf, sw, INK);
			}
			x = row.left + SWATCH_W;
		}
		let colour = if lit { LIGHT } else { ink };
		ui.text(sf, x, y, text.get(..n).unwrap_or(&[]), colour);
		if pop == Pop::Zoom && i == 0 {
			let key_x = row.right - 4 - ui.width(sf, ZOOM_KEY);
			ui.text(sf, key_x, y, ZOOM_KEY, colour);
		}
	}
}
