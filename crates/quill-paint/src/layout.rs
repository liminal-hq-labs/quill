// Where the screen's parts are: the bars, the ruler, the page and the scroll bars, from the View menu's choices.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Pure arithmetic over `quill_app::view`'s sizes. A pointer asks the same [`Layout`] the painter draws with, so a click lands on
//! what is on the screen.

use quill_app::view::{hscroll_max, rows, scroll_layout, RULER_H};
use quill_app::App;
use quill_core::layout::Measure;

use crate::shell::{MENU_H, SCREEN_H, SCREEN_W, STATUS_H};
use crate::Rect;

/// The width of the vertical scroll bar and the height of the horizontal one.
pub const BAR: i32 = 16;

/// The parts of the screen. A part that the View menu hides is `None`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Layout {
	/// The menu bar.
	pub menu: Rect,
	/// The standard toolbar row.
	pub tool: Option<Rect>,
	/// The formatting bar row.
	pub format: Option<Rect>,
	/// The ruler.
	pub ruler: Option<Rect>,
	/// The page area, where the sheet is drawn and clicks reach the text.
	pub page: Rect,
	/// The vertical scroll bar.
	pub vbar: Rect,
	/// The horizontal scroll bar, when the page is wider than the window.
	pub hbar: Option<Rect>,
	/// The square where the two scroll bars meet.
	pub corner: Option<Rect>,
	/// The status bar.
	pub status: Option<Rect>,
}

fn rect((l, t, w, h): (i32, i32, i32, i32)) -> Rect {
	Rect::new(l, t, l + w, t + h)
}

impl Layout {
	/// The layout for what the app shows.
	pub fn of<M: Measure + Clone>(app: &App<M>) -> Self {
		let (h0, h1) = rows(app);
		let mut y = MENU_H;
		let tool = (h0 > 0).then(|| Rect::new(0, y, SCREEN_W, y + h0));
		y += h0;
		let format = (h1 > 0).then(|| Rect::new(0, y, SCREEN_W, y + h1));
		y += h1;
		let ruler = app
			.show_ruler()
			.then(|| Rect::new(0, y, SCREEN_W, y + RULER_H));
		if ruler.is_some() {
			y += RULER_H;
		}
		let status_h = if app.show_status() { STATUS_H } else { 0 };
		let show_h = hscroll_max(app, SCREEN_W - BAR) > 0;
		let sl = scroll_layout(SCREEN_W, SCREEN_H, y, status_h, BAR, BAR, show_h);
		Layout {
			menu: Rect::new(0, 0, SCREEN_W, MENU_H),
			tool,
			format,
			ruler,
			page: Rect::new(0, y, sl.page_w, sl.page_bottom),
			vbar: rect(sl.vbar),
			hbar: sl.hbar.map(rect),
			corner: sl.corner.map(rect),
			status: (status_h > 0).then(|| Rect::new(0, SCREEN_H - status_h, SCREEN_W, SCREEN_H)),
		}
	}
}
