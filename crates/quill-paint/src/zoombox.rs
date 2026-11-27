// The zoom control at the right end of the status line: a minus button, the box with the zoom in it, a plus button and a margin.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The geometry is `quill_chrome::zoom_field`'s ([`ZoomLayout`], [`zoom_step`], CORE-67), laid out from the screen's right edge; this
//! module draws it and answers where a click landed. The box opens the list of levels ([`crate::drop::Pop::Zoom`]) and a button steps
//! to the next level down or up, greyed when there is none.

use quill_app::App;
use quill_chrome::zoom_field::{
	reset_glyph, reset_live, zoom_step, ZoomLayout, ZoomMetrics, ZoomPart, RESET_GLYPH,
	ZOOM_MARGIN, ZOOM_TEXT_MAX,
};
use quill_core::layout::Measure;

use crate::layout::Layout;
use crate::shell::{pct, SCREEN_W};
use crate::ui::{bevel, Ui, FACE, INK, LIGHT, SHADOW};
use crate::{Rect, Surface};

/// What each part of the control takes: three buttons 20 pixels wide (minus, plus and Page Width), a box wide enough for `Page
/// Width`, and the margin every build leaves at the bar's edge.
pub const METRICS: ZoomMetrics = ZoomMetrics {
	button: 20,
	field: 76,
	reset: 20,
	margin: ZOOM_MARGIN,
};

/// Where the parts sit on the status line.
pub fn layout() -> ZoomLayout {
	ZoomLayout::new(SCREEN_W, METRICS)
}

/// The part of the control at (`x`, `y`), or `None` when the status line is hidden or the point is off the control.
pub fn part_at(lay: &Layout, x: i32, y: i32) -> Option<ZoomPart> {
	let r = lay.status?;
	if y < r.top || y >= r.bottom {
		return None;
	}
	layout().hit(x)
}

/// The box's rectangle, where the list is anchored.
pub fn field_rect(lay: &Layout) -> Option<Rect> {
	let r = lay.status?;
	let l = layout();
	Some(Rect::new(l.field.0, r.top + 3, l.field.1, r.bottom - 2))
}

/// The x where the status text must end: a little before the control's first button.
pub fn text_right() -> i32 {
	layout().left() - 3
}

/// The rectangle of `part`, the same one a click on it lands in.
pub fn part_rect(lay: &Layout, part: ZoomPart) -> Option<Rect> {
	let r = lay.status?;
	let l = layout();
	let (left, right) = match part {
		ZoomPart::Minus => l.minus,
		ZoomPart::Field => l.field,
		ZoomPart::Plus => l.plus,
		ZoomPart::Reset => l.reset,
	};
	Some(Rect::new(left, r.top + 2, right, r.bottom - 1))
}

/// What a button shows: a minus, a plus or the Page Width mark.
#[derive(Copy, Clone, PartialEq, Eq)]
enum Sign {
	Minus,
	Plus,
	Page,
}

fn button<Sf: Surface + ?Sized>(
	sf: &mut Sf,
	range: (i32, i32),
	r: Rect,
	kind: Sign,
	live: bool,
	held: bool,
) {
	let face = Rect::new(range.0, r.top + 2, range.1, r.bottom - 1);
	sf.fill_rect(face, FACE);
	bevel(sf, face, !held);
	let (cx, cy) = (
		(face.left + face.right) / 2 + i32::from(held),
		(face.top + face.bottom) / 2 + i32::from(held),
	);
	let sign = |sf: &mut Sf, dx: i32, c| {
		if kind == Sign::Page {
			let (left, top) = (cx - RESET_GLYPH.0 / 2 + dx, cy - RESET_GLYPH.1 / 2 + dx);
			reset_glyph(|x, y, w, h| {
				sf.fill_rect(Rect::new(left + x, top + y, left + x + w, top + y + h), c)
			});
			return;
		}
		sf.hline(cx - 3 + dx, cx + 4 + dx, cy + dx, c);
		if kind == Sign::Plus {
			sf.vline(cx + dx, cy - 3 + dx, cy + 4 + dx, c);
		}
	};
	if live {
		sign(sf, 0, INK);
	} else {
		// Embossed: the shadow's sign over the highlight's, a pixel apart.
		sign(sf, 1, LIGHT);
		sign(sf, 0, SHADOW);
	}
}

/// Draws the control on the status line `r`: `held` is the button being pressed, shown sunk.
pub fn draw<Sf: Surface + ?Sized, M: Measure + Clone>(
	sf: &mut Sf,
	ui: &Ui<Sf::Font>,
	app: &App<M>,
	r: Rect,
	held: Option<ZoomPart>,
) {
	let l = layout();
	let shown = pct(app);
	button(
		sf,
		l.minus,
		r,
		Sign::Minus,
		zoom_step(shown, false).is_some(),
		held == Some(ZoomPart::Minus),
	);
	button(
		sf,
		l.plus,
		r,
		Sign::Plus,
		zoom_step(shown, true).is_some(),
		held == Some(ZoomPart::Plus),
	);
	button(
		sf,
		l.reset,
		r,
		Sign::Page,
		reset_live(app.zoom()),
		held == Some(ZoomPart::Reset),
	);
	let field = Rect::new(l.field.0, r.top + 3, l.field.1, r.bottom - 2);
	bevel(sf, field, false);
	let mut text = [0u8; ZOOM_TEXT_MAX];
	let n = app.zoom_field_text(&mut text);
	let t = text.get(..n).unwrap_or(&[]);
	let x = field.left + (field.width() - ui.width(sf, t)) / 2;
	ui.text(sf, x, field.top + (field.height() - ui.height) / 2, t, INK);
}
