// The pieces every piece of chrome shares: the Windows 3.1 colours, the interface font and a bevel.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use quill_core::layout::{FontRef, Measure};

use crate::{Rect, Rgb, Surface};

/// The face of buttons, bars and menus.
pub const FACE: Rgb = Rgb::SILVER;
/// The dark edge of a bevel.
pub const SHADOW: Rgb = Rgb::GREY;
/// The light edge of a bevel.
pub const LIGHT: Rgb = Rgb::WHITE;
/// Text and outlines.
pub const INK: Rgb = Rgb::BLACK;
/// The selection bar.
pub const SELECT: Rgb = Rgb::NAVY;

/// The interface font: Arial at 10 point.
pub fn ui_font() -> FontRef {
	let mut face = [0u8; 31];
	let name = b"Arial";
	for (d, s) in face.iter_mut().zip(name) {
		*d = *s;
	}
	FontRef {
		face,
		face_len: name.len() as u8,
		family: 2,
		size: 20,
		flags: 0,
	}
}

/// The user-interface font as the surface draws it, with its baseline offset.
pub struct Ui<F: Copy> {
	/// The surface's font.
	pub font: F,
	/// From the top of a line to the baseline.
	pub ascent: i32,
	/// A line's height.
	pub height: i32,
}

impl<F: Copy> Ui<F> {
	/// The interface font on `sf`, measured with `m`.
	pub fn new<Sf: Surface<Font = F> + ?Sized, Me: Measure>(sf: &mut Sf, m: &mut Me) -> Self {
		let fr = ui_font();
		let met = m.metrics(&fr);
		Ui {
			font: sf.font(&fr),
			ascent: met.ascent,
			height: met.ascent + met.descent,
		}
	}

	/// Draws `text` with its top at `y`; returns the x after it.
	pub fn text<Sf: Surface<Font = F> + ?Sized>(
		&self,
		sf: &mut Sf,
		x: i32,
		y: i32,
		text: &[u8],
		fg: Rgb,
	) -> i32 {
		let mut x = x;
		for chunk in text.chunks(32) {
			let mut run = [(0u8, 0i32); 32];
			let mut n = 0;
			for (slot, &ch) in run.iter_mut().zip(chunk) {
				*slot = (ch, x);
				n += 1;
				x += sf.advance(self.font, ch);
			}
			if let Some(r) = run.get(..n) {
				sf.glyph_run(self.font, y + self.ascent, r, fg, false);
			}
		}
		x
	}

	/// The width of `text`.
	pub fn width<Sf: Surface<Font = F> + ?Sized>(&self, sf: &mut Sf, text: &[u8]) -> i32 {
		text.iter().map(|c| sf.advance(self.font, *c)).sum()
	}

	/// The width of a label written with an `&` before its mnemonic, the `&` not counted.
	pub fn label_width<Sf: Surface<Font = F> + ?Sized>(&self, sf: &mut Sf, label: &[u8]) -> i32 {
		label
			.iter()
			.filter(|c| **c != b'&')
			.map(|c| sf.advance(self.font, *c))
			.sum()
	}

	/// Draws a label with its mnemonic underlined (the letter after the `&`); returns the x after it.
	pub fn label<Sf: Surface<Font = F> + ?Sized>(
		&self,
		sf: &mut Sf,
		x: i32,
		y: i32,
		label: &[u8],
		fg: Rgb,
	) -> i32 {
		let mut x = x;
		for (i, part) in label.split(|c| *c == b'&').enumerate() {
			let (mark, rest) = match part.split_first() {
				Some((c, rest)) if i > 0 => (Some(*c), rest),
				_ => (None, part),
			};
			if let Some(c) = mark {
				let w = sf.advance(self.font, c);
				self.text(sf, x, y, &[c], fg);
				sf.hline(x, x + w, y + self.height, fg);
				x += w;
			}
			x = self.text(sf, x, y, rest, fg);
		}
		x
	}
}

/// A one-pixel bevel inside `r`: light on the top and left and dark on the bottom and right when `raised`, the other way round when not.
pub fn bevel<Sf: Surface + ?Sized>(sf: &mut Sf, r: Rect, raised: bool) {
	let (tl, br) = if raised {
		(LIGHT, SHADOW)
	} else {
		(SHADOW, LIGHT)
	};
	sf.hline(r.left, r.right, r.top, tl);
	sf.vline(r.left, r.top, r.bottom, tl);
	sf.hline(r.left, r.right, r.bottom - 1, br);
	sf.vline(r.right - 1, r.top, r.bottom, br);
}

/// A one-pixel outline of `r` in `c`.
pub fn outline<Sf: Surface + ?Sized>(sf: &mut Sf, r: Rect, c: Rgb) {
	sf.hline(r.left, r.right, r.top, c);
	sf.hline(r.left, r.right, r.bottom - 1, c);
	sf.vline(r.left, r.top, r.bottom, c);
	sf.vline(r.right - 1, r.top, r.bottom, c);
}

/// A dotted outline of `r` (every other pixel), which marks the control that has the keyboard focus.
pub fn dotted<Sf: Surface + ?Sized>(sf: &mut Sf, r: Rect, c: Rgb) {
	let mut x = r.left;
	while x < r.right {
		sf.fill_rect(Rect::new(x, r.top, x + 1, r.top + 1), c);
		if (r.bottom - 1 - r.top) % 2 == 0 {
			sf.fill_rect(Rect::new(x, r.bottom - 1, x + 1, r.bottom), c);
		} else if x + 1 < r.right {
			sf.fill_rect(Rect::new(x + 1, r.bottom - 1, x + 2, r.bottom), c);
		}
		x += 2;
	}
	let mut y = r.top + 2;
	while y < r.bottom - 1 {
		sf.fill_rect(Rect::new(r.left, y, r.left + 1, y + 1), c);
		if (r.right - 1 - r.left) % 2 == 0 {
			sf.fill_rect(Rect::new(r.right - 1, y, r.right, y + 1), c);
		} else {
			sf.fill_rect(Rect::new(r.right - 1, y + 1, r.right, y + 2), c);
		}
		y += 2;
	}
}
