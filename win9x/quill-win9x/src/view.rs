// Where the page is: the zoom, the stack of pages, and the scroll positions, all as plain arithmetic.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The page view works in two spaces. **Page units** are the layout's: a pixel at 96 dots per inch on the paper, with the pages
//! stacked top to bottom `stride` apart. **Device pixels** are what is on the screen, `zoom` percent of that. The vertical scroll
//! position is kept in page units (it survives a change of zoom and never needs more than 32 bits), the horizontal one in device
//! pixels. Windows 95's graphics layer works in 16-bit coordinates, so nothing here hands it a coordinate measured from the top of
//! the document: drawing is always relative to the first visible page unit.

use quill_app::App;
use quill_core::layout::to_units;

use crate::gdi::GdiMeasure;

/// Space between the paper and the edge of the window when the paper is wider than the window, in device pixels.
pub const PAGE_PAD: i32 = 8;
/// Space between one sheet of paper and the next, in page units.
pub const PAGE_GAP: i32 = 14;
/// Space above the first sheet and below the last, in page units.
pub const DESK_PAD: i32 = 8;
/// The smallest and largest zoom, in percent.
pub const ZOOM_MIN: i32 = 25;
pub const ZOOM_MAX: i32 = 200;
/// The pixels one click on a scroll bar's arrow moves the page.
pub const LINE_STEP: i32 = 16;

/// The paper's width and height in page units.
pub fn paper(app: &App<GdiMeasure>) -> (i32, i32) {
	let p = &app.doc().page;
	(to_units(p.paper_w, 96), to_units(p.paper_h, 96))
}

/// From the top of one sheet of paper to the top of the next, in page units.
pub fn stride(app: &App<GdiMeasure>) -> i32 {
	paper(app).1 + PAGE_GAP
}

/// The zoom in percent the window shows now: the chosen one, or for the page's width the largest that fits, never past 100.
pub fn zoom_pct(app: &App<GdiMeasure>, client_w: i32) -> i32 {
	match app.zoom() {
		0 => {
			let (pw, _) = paper(app);
			((client_w - 2 * PAGE_PAD) * 100 / pw.max(1)).clamp(ZOOM_MIN, 100)
		}
		z => i32::from(z).clamp(ZOOM_MIN, ZOOM_MAX),
	}
}

/// Page units to device pixels.
pub fn dev(v: i32, pct: i32) -> i32 {
	(i64::from(v) * i64::from(pct) / 100) as i32
}

/// Device pixels to page units.
pub fn log(v: i32, pct: i32) -> i32 {
	(i64::from(v) * 100 / i64::from(pct.max(1))) as i32
}

/// Where the paper's left edge is in a client area `client_w` wide: the paper is centred when it fits with its padding, and
/// otherwise sits at the padding, shifted left by the sideways scroll.
pub fn page_x0(client_w: i32, page_dev_w: i32, hscroll: i32) -> i32 {
	if client_w >= page_dev_w + 2 * PAGE_PAD {
		(client_w - page_dev_w) / 2
	} else {
		PAGE_PAD - hscroll
	}
}

/// How far the page can scroll sideways, in device pixels: zero when it fits.
pub fn hscroll_max(app: &App<GdiMeasure>, client_w: i32) -> i32 {
	let pct = zoom_pct(app, client_w);
	(dev(paper(app).0, pct) + 2 * PAGE_PAD - client_w).max(0)
}

/// The height of all the sheets stacked with their gaps and the desk above and below, in page units.
pub fn stack_height(app: &App<GdiMeasure>, pages: u32) -> i32 {
	pages as i32 * stride(app) - PAGE_GAP + 2 * DESK_PAD
}

/// The farthest the view can scroll down, in page units, for a view `view_h` page units high: far enough that the bottom of the last
/// sheet can reach the middle of the view, so the end of the document is never pinned to the window's bottom edge.
pub fn vscroll_max(app: &App<GdiMeasure>, pages: u32, view_h: i32) -> i32 {
	(stack_height(app, pages) - view_h / 2).max(0)
}

/// What a 32-bit position is divided by to fit a Windows 95 scroll bar, whose positions are 16 bits.
pub fn bar_scale(total: i32) -> i32 {
	total / 30_000 + 1
}

/// The offset of page unit `y_stack` (measured from the first sheet's top) in the view scrolled to `vpos`, in page units.
pub fn rel(y_stack: i32, vpos: i32) -> i32 {
	y_stack + DESK_PAD - vpos
}
