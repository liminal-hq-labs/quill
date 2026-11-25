// The toolbar and the formatting bar: the buttons from the shared strip, the style, face and size boxes, and where each is.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The glyphs are `assets/toolbar/toolbar.bmp`, the strip the Windows builds use: 22 pictures of 16 by 15 pixels in 4 bits, already
//! in the 16 Windows colours, so the surfaces draw them as they are. Palette index 0 (silver) is the background, which is left
//! alone so a button's face shows through; a switched-on toggle has the checkered face of Windows 3.1 and the same bevel sunk. The
//! geometry is `quill_app::view::TOOLBAR`: [`button_rect`], [`button_at`] and [`box_rect`] are pure and use only it and the
//! [`Layout`], so a pointer hits what the painter drew.

use quill_app::view::{is_on, Act, Button, BTN_H, BTN_W, TB_H, TOOLBAR};
use quill_app::{App, STYLE_NAMES};
use quill_core::layout::Measure;
use quill_core::store::ReadStore;

use crate::layout::Layout;
use crate::ui::{bevel, outline, Ui, FACE, INK, LIGHT, SHADOW};
use crate::{Rect, Rgb, Surface};

static STRIP: &[u8] = include_bytes!("../../../assets/toolbar/toolbar.bmp");

/// The three boxes of the formatting bar.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Box3 {
	/// The paragraph style.
	Style,
	/// The font face.
	Face,
	/// The size in points.
	Size,
}

fn le32(at: usize) -> usize {
	let b = |i: usize| usize::from(STRIP.get(at + i).copied().unwrap_or(0));
	b(0) | b(1) << 8 | b(2) << 16 | b(3) << 24
}

/// The colour of palette entry `n` of the strip.
fn palette(n: usize) -> Rgb {
	let at = 14 + le32(14) + n * 4;
	let b = |i: usize| STRIP.get(at + i).copied().unwrap_or(0);
	Rgb(b(2), b(1), b(0))
}

/// Draws glyph `k` (16 by 15) with its top left corner at (`x`, `y`), leaving palette entry 0 undrawn.
fn glyph<Sf: Surface + ?Sized>(sf: &mut Sf, k: i32, x: i32, y: i32, transparent: bool) {
	let off = le32(10);
	let (w, h) = (le32(18), le32(22));
	let stride = (w * 4).div_ceil(32) * 4;
	if !transparent {
		// On a plain face the silver of the strip's background is the face, so the whole glyph is one bitmap, which maps the 16 colours
		// once instead of once for every run of pixels.
		let mut rows = [0u8; 15 * 8];
		for (row, dst) in rows.chunks_mut(8).enumerate() {
			let at = off + h.saturating_sub(1 + row) * stride + k as usize * 8;
			if let Some(src) = STRIP.get(at..at + 8) {
				dst.copy_from_slice(src);
			}
		}
		let pal: [Rgb; 16] = core::array::from_fn(palette);
		sf.bitmap4(x, y, &rows, 16, 15, &pal);
		return;
	}
	for row in 0..15usize {
		// The strip is stored bottom row first.
		let line = h.saturating_sub(1 + row);
		let base = off + line * stride + k as usize * 8;
		let mut col = 0usize;
		while col < 16 {
			let nib = |c: usize| {
				let b = STRIP.get(base + c / 2).copied().unwrap_or(0);
				usize::from(if c & 1 == 0 { b >> 4 } else { b & 15 })
			};
			let v = nib(col);
			let mut end = col + 1;
			while end < 16 && nib(end) == v {
				end += 1;
			}
			if v != 0 {
				sf.fill_rect(
					Rect::new(
						x + col as i32,
						y + row as i32,
						x + end as i32,
						y + row as i32 + 1,
					),
					palette(v),
				);
			}
			col = end;
		}
	}
}

/// The rectangle of toolbar button `i` (an index into `TOOLBAR.buttons`), or `None` when its row is hidden.
pub fn button_rect(lay: &Layout, i: usize) -> Option<Rect> {
	let b = TOOLBAR.buttons.get(i)?;
	let row = if b.row == 0 { lay.tool? } else { lay.format? };
	Some(Rect::new(
		b.x,
		row.top + 2,
		b.x + BTN_W,
		row.top + 2 + BTN_H,
	))
}

/// The button at (`x`, `y`), as an index into `TOOLBAR.buttons`.
pub fn button_at(lay: &Layout, x: i32, y: i32) -> Option<usize> {
	(0..TOOLBAR.buttons.len()).find(|i| {
		button_rect(lay, *i)
			.is_some_and(|r| x >= r.left && x < r.right && y >= r.top && y < r.bottom)
	})
}

/// The rectangle of one of the formatting bar's boxes (the arrow at its right end included), or `None` when the bar is hidden.
pub fn box_rect(lay: &Layout, which: Box3) -> Option<Rect> {
	let row = lay.format?;
	let (x, w) = match which {
		Box3::Style => TOOLBAR.style,
		Box3::Face => TOOLBAR.face,
		Box3::Size => TOOLBAR.size,
	};
	Some(Rect::new(x, row.top + 3, x + w, row.top + 3 + TB_H - 6))
}

/// The box at (`x`, `y`).
pub fn box_at(lay: &Layout, x: i32, y: i32) -> Option<Box3> {
	[Box3::Style, Box3::Face, Box3::Size].into_iter().find(|b| {
		box_rect(lay, *b).is_some_and(|r| x >= r.left && x < r.right && y >= r.top && y < r.bottom)
	})
}

fn face_of<Sf: Surface + ?Sized>(sf: &mut Sf, r: Rect, down: bool, on: bool) {
	if down && on {
		// The checkered face of a switched-on toggle: white and silver, a pixel each.
		sf.fill_rect(r, Rgb::WHITE);
		let mut y = r.top;
		while y < r.bottom {
			let mut x = r.left + (y & 1);
			while x < r.right {
				sf.fill_rect(Rect::new(x, y, x + 1, y + 1), FACE);
				x += 2;
			}
			y += 1;
		}
	} else {
		sf.fill_rect(r, FACE);
	}
	outline(sf, r, INK);
	bevel(
		sf,
		Rect::new(r.left + 1, r.top + 1, r.right - 1, r.bottom - 1),
		!down,
	);
}

fn button<Sf: Surface + ?Sized, M: Measure + Clone, S: ReadStore + ?Sized>(
	sf: &mut Sf,
	app: &App<M>,
	s: &S,
	b: &Button,
	r: Rect,
	held: bool,
) {
	let on = is_on(app, s, b.toggle);
	// A button the pointer holds down is sunk, with a silver face unless it is a toggle that is on (its face stays checkered).
	let down = on || held;
	face_of(sf, r, down, on);
	let off = i32::from(down);
	glyph(sf, b.glyph, r.left + 4 + off, r.top + 4 + off, on);
	if b.act == Act::Colour {
		if let Some((cr, cg, cb)) = app.char_state(s).colour {
			sf.fill_rect(
				Rect::new(
					r.left + 6 + off,
					r.top + 15 + off,
					r.left + 18 + off,
					r.top + 18 + off,
				),
				Rgb(cr, cg, cb),
			);
		}
	}
}

fn combo<Sf: Surface + ?Sized>(sf: &mut Sf, ui: &Ui<Sf::Font>, r: Rect, text: &[u8]) {
	sf.fill_rect(r, Rgb::WHITE);
	bevel(sf, r, false);
	sf.hline(r.left + 1, r.right - 1, r.top + 1, INK);
	sf.vline(r.left + 1, r.top + 1, r.bottom - 1, INK);
	// The drop-down button at the right end, with its arrow.
	let a = Rect::new(r.right - 17, r.top + 2, r.right - 2, r.bottom - 2);
	sf.fill_rect(a, FACE);
	outline(sf, a, INK);
	bevel(
		sf,
		Rect::new(a.left + 1, a.top + 1, a.right - 1, a.bottom - 1),
		true,
	);
	let (cx, cy) = ((a.left + a.right) / 2, (a.top + a.bottom) / 2);
	for i in 0..4 {
		sf.hline(cx - 3 + i, cx + 4 - i, cy - 1 + i, INK);
	}
	let inner = Rect::new(r.left + 3, r.top + 2, a.left - 1, r.bottom - 2);
	sf.clip(Some(inner));
	let y = r.top + (r.height() - ui.height) / 2;
	ui.text(sf, r.left + 4, y, text, INK);
	sf.clip(None);
}

/// Draws the rows that show: the standard toolbar and the formatting bar. `held` is the button the pointer has pressed (an index into
/// `TOOLBAR.buttons`), which shows sunk.
pub fn draw<Sf: Surface + ?Sized, M: Measure + Clone, S: ReadStore + ?Sized>(
	sf: &mut Sf,
	ui: &Ui<Sf::Font>,
	app: &App<M>,
	s: &S,
	lay: &Layout,
	held: Option<usize>,
) {
	for (row, strip) in [(0u8, lay.tool), (1u8, lay.format)] {
		let Some(strip) = strip else { continue };
		sf.clip(Some(strip));
		sf.fill_rect(strip, FACE);
		sf.hline(strip.left, strip.right, strip.top, LIGHT);
		sf.hline(strip.left, strip.right, strip.bottom - 1, SHADOW);
		let mut prev_end: Option<i32> = None;
		for (i, b) in TOOLBAR
			.buttons
			.iter()
			.enumerate()
			.filter(|(_, b)| b.row == row)
		{
			// An etched line fills the gap between groups of buttons.
			if let Some(e) = prev_end {
				if b.x - e > 4 {
					let x = e + (b.x - e) / 2 - 1;
					sf.vline(x, strip.top + 5, strip.bottom - 5, SHADOW);
					sf.vline(x + 1, strip.top + 5, strip.bottom - 5, LIGHT);
				}
			}
			prev_end = Some(b.x + BTN_W);
			if let Some(r) = button_rect(lay, i) {
				button(sf, app, s, b, r, held == Some(i));
			}
		}
		sf.clip(None);
	}
	if lay.format.is_some() {
		let cs = app.char_state(s);
		let mut size = [0u8; 8];
		let n = decimal(u32::from(cs.size) / 2, &mut size);
		let style = app
			.style_of(s)
			.and_then(|i| STYLE_NAMES.get(i))
			.map_or(&b""[..], |n| n.as_bytes());
		let texts: [(Box3, &[u8]); 3] = [
			(Box3::Style, style),
			(
				Box3::Face,
				cs.face.get(..usize::from(cs.face_len)).unwrap_or(&[]),
			),
			(Box3::Size, size.get(..n).unwrap_or(&[])),
		];
		for (b, t) in texts {
			if let Some(r) = box_rect(lay, b) {
				combo(sf, ui, r, t);
			}
		}
	}
}

fn decimal(mut v: u32, out: &mut [u8; 8]) -> usize {
	let mut tmp = [0u8; 8];
	let mut i = tmp.len();
	loop {
		i -= 1;
		if let Some(t) = tmp.get_mut(i) {
			*t = b'0' + (v % 10) as u8;
		}
		v /= 10;
		if v == 0 || i == 0 {
			break;
		}
	}
	let n = tmp.len() - i;
	for (d, s) in out.iter_mut().zip(tmp.get(i..).unwrap_or(&[])) {
		*d = *s;
	}
	n
}
