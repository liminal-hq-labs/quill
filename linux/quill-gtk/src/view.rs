// The view model: zoom, scroll, the page geometry and the mapping from the document to the device, with no toolkit in it.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The document is laid out at 96 pixels to the inch (`quill_app::view`), and everything here is in those *document pixels* until
// the last step. A page is `Geometry::page_w` by `page_h` of them, with `PAGE_TOP` above the first, `PAGE_GAP` between sheets and
// `PAGE_PAD` beside the page when it is wider than the window. The [`Mapping`] turns a document pixel into a device pixel: the
// zoom, then the display's scale in 1/120 (what Wayland sends), in one fixed-point number so each edge is rounded once.

use quill_app::view::{page_x0, PAGE_GAP, PAGE_PAD, PAGE_TOP};
use quill_paint::pixel::Scale;

/// The smallest zoom, in percent.
pub const ZOOM_MIN: u32 = 25;
/// The largest zoom, in percent (the editor's own limit).
pub const ZOOM_MAX: u32 = 200;
/// The zooms Ctrl+plus and Ctrl+minus step through.
pub const ZOOM_STEPS: [u32; 10] = [25, 50, 75, 90, 100, 110, 125, 150, 175, 200];
/// How far an arrow key or one notch of the wheel scrolls, in document pixels.
pub const LINE_STEP: i32 = 48;

/// The pixel mapping: the view's zoom and the display's scale.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Mapping {
	/// The zoom in percent, 25 to 200.
	pub zoom_pct: u32,
	/// The display scale in 1/120: 120 is 1.0, 180 is 1.5, what `wp_fractional_scale_v1` reports.
	pub scale_120: u32,
}

impl Mapping {
	/// A mapping with the zoom held to its limits and the scale to 0.5 to 8.0.
	pub fn new(zoom_pct: u32, scale_120: u32) -> Mapping {
		Mapping {
			zoom_pct: zoom_pct.clamp(ZOOM_MIN, ZOOM_MAX),
			scale_120: scale_120.clamp(60, 960),
		}
	}

	/// The scale in 1/120 of a display scale such as `gdk::Surface::scale()`.
	pub fn scale_120_of(scale: f64) -> u32 {
		(scale * 120.0).round().clamp(60.0, 960.0) as u32
	}

	/// The scale from a document pixel to a device pixel, as the pixel surface takes it.
	pub fn scale(self) -> Scale {
		Scale::from_ratio(self.zoom_pct * self.scale_120, 12_000)
	}

	/// A device length for `px` document pixels, to the nearest.
	pub fn dev(self, px: i32) -> i32 {
		self.scale().dev(px)
	}

	/// How many document pixels `dev` device pixels cover, rounded up so the whole device length is drawn.
	pub fn doc_ceil(self, dev: u32) -> i32 {
		let den = u64::from(self.scale_120) * u64::from(self.zoom_pct);
		(((u64::from(dev) * 12_000) + den - 1) / den.max(1)).min(i32::MAX as u64) as i32
	}

	/// The device size of a widget `w` by `h` in the toolkit's logical pixels: what the texture must be, whatever the zoom.
	pub fn device_size(self, w: i32, h: i32) -> (u32, u32) {
		let d = |v: i32| ((i64::from(v.max(0)) * i64::from(self.scale_120) + 60) / 120) as u32;
		(d(w), d(h))
	}

	/// The document pixel (rounded down) under a device pixel of the view: the inverse of [`Mapping::dev`] for a point.
	pub fn to_doc(self, dev: i32) -> i32 {
		let num = i64::from(dev) * 12_000;
		let den = i64::from(self.scale_120) * i64::from(self.zoom_pct);
		num.div_euclid(den.max(1)) as i32
	}

	/// The display scale as a number.
	pub fn display_scale(self) -> f64 {
		f64::from(self.scale_120) / 120.0
	}
}

/// How big the pages are.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Geometry {
	/// A sheet's width in document pixels.
	pub page_w: i32,
	/// A sheet's height in document pixels.
	pub page_h: i32,
	/// How many sheets the body takes (at least 1).
	pub pages: u32,
}

impl Default for Geometry {
	fn default() -> Self {
		// Letter paper: 8.5 by 11 inches.
		Geometry {
			page_w: 816,
			page_h: 1056,
			pages: 1,
		}
	}
}

impl Geometry {
	/// The document pixel at which sheet `index` (from 0) begins.
	pub fn page_top(&self, index: u32) -> i32 {
		PAGE_TOP + index as i32 * (self.page_h + PAGE_GAP)
	}

	/// The whole document's height: the sheets, the gaps and the pad under the last.
	pub fn content_h(&self) -> i32 {
		self.page_top(self.pages.saturating_sub(1)) + self.page_h + PAGE_PAD
	}

	/// The whole document's width: the sheet and the pad on both sides.
	pub fn content_w(&self) -> i32 {
		self.page_w + 2 * PAGE_PAD
	}
}

/// The zoom the person asked for.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Zoom {
	/// A fixed percentage.
	Percent(u32),
	/// The page's width fills the window, never larger than 100 percent.
	Width,
}

/// A sheet that is on screen: its number from 1 and where its top left corner is in document pixels from the view's corner.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Slot {
	/// The sheet's number, from 1.
	pub page: u32,
	/// Its left edge, in document pixels from the left of the view.
	pub left: i32,
	/// Its top edge, in document pixels from the top of the view (negative when it starts above).
	pub top: i32,
}

/// What a scroll bar needs for one axis, in document pixels.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Range {
	/// The scroll position.
	pub value: i32,
	/// The most it can be: the content less the view, 0 when the content fits.
	pub max: i32,
	/// The length of the content.
	pub upper: i32,
	/// The length of the view.
	pub page: i32,
}

/// The view of one document.
#[derive(Clone, Debug)]
pub struct View {
	zoom: Zoom,
	scale_120: u32,
	/// The size of the view in device pixels.
	device: (u32, u32),
	geom: Geometry,
	x: i32,
	y: i32,
}

impl Default for View {
	fn default() -> Self {
		View::new(Zoom::Percent(100))
	}
}

impl View {
	/// A view at a zoom, at scale 1.0 and with no size yet.
	pub fn new(zoom: Zoom) -> View {
		View {
			zoom,
			scale_120: 120,
			device: (0, 0),
			geom: Geometry::default(),
			x: 0,
			y: 0,
		}
	}

	/// The zoom asked for.
	pub fn zoom(&self) -> Zoom {
		self.zoom
	}

	/// The page geometry.
	pub fn geometry(&self) -> Geometry {
		self.geom
	}

	/// The size in device pixels.
	pub fn device(&self) -> (u32, u32) {
		self.device
	}

	/// The display's scale in 1/120.
	pub fn scale_120(&self) -> u32 {
		self.scale_120
	}

	/// The zoom in percent as drawn: the percentage, or what fits the page's width in the window.
	pub fn effective_zoom(&self) -> u32 {
		match self.zoom {
			Zoom::Percent(p) => p.clamp(ZOOM_MIN, ZOOM_MAX),
			Zoom::Width => {
				let view_w = u64::from(self.device.0) * 120 / u64::from(self.scale_120.max(1));
				let fit = view_w * 100 / self.geom.content_w().max(1) as u64;
				(fit as u32).clamp(ZOOM_MIN, 100)
			}
		}
	}

	/// The pixel mapping now.
	pub fn mapping(&self) -> Mapping {
		Mapping::new(self.effective_zoom(), self.scale_120)
	}

	/// The size of the view in document pixels (the part of a pixel that is cut off counts).
	pub fn visible(&self) -> (i32, i32) {
		let m = self.mapping();
		(m.doc_ceil(self.device.0), m.doc_ceil(self.device.1))
	}

	/// Tells the view how big it is and at what display scale; the scroll is held inside the document.
	pub fn set_viewport(&mut self, device_w: u32, device_h: u32, scale_120: u32) {
		self.device = (device_w, device_h);
		self.scale_120 = Mapping::new(100, scale_120).scale_120;
		self.clamp();
	}

	/// Tells the view the sheets' size (after a file opened); scrolls to the top.
	pub fn set_geometry(&mut self, geom: Geometry) {
		self.geom = Geometry {
			pages: geom.pages.max(1),
			..geom
		};
		self.x = 0;
		self.y = 0;
		self.clamp();
	}

	/// Tells the view the sheets' size after an edit: the scroll stays where it is, held inside the document.
	pub fn update_geometry(&mut self, geom: Geometry) {
		self.geom = Geometry {
			pages: geom.pages.max(1),
			..geom
		};
		self.clamp();
	}

	fn clamp(&mut self) {
		let (mx, my) = self.max_scroll();
		self.x = self.x.clamp(0, mx);
		self.y = self.y.clamp(0, my);
	}

	/// The furthest the view can scroll across and down.
	pub fn max_scroll(&self) -> (i32, i32) {
		let (vw, vh) = self.visible();
		(
			(self.geom.content_w() - vw).max(0),
			(self.geom.content_h() - vh).max(0),
		)
	}

	/// The scroll position in document pixels.
	pub fn scroll(&self) -> (i32, i32) {
		(self.x, self.y)
	}

	/// The scroll bar values for one axis: `horizontal` or vertical.
	pub fn range(&self, horizontal: bool) -> Range {
		let (vw, vh) = self.visible();
		let (value, upper, page, max) = if horizontal {
			(self.x, self.geom.content_w(), vw, self.max_scroll().0)
		} else {
			(self.y, self.geom.content_h(), vh, self.max_scroll().1)
		};
		Range {
			value,
			max,
			upper,
			page,
		}
	}

	/// Scrolls by an amount, held inside the document. Returns true when the position changed.
	pub fn scroll_by(&mut self, dx: i32, dy: i32) -> bool {
		self.scroll_to(self.x.saturating_add(dx), self.y.saturating_add(dy))
	}

	/// Scrolls to a position, held inside the document. Returns true when the position changed.
	pub fn scroll_to(&mut self, x: i32, y: i32) -> bool {
		let before = (self.x, self.y);
		self.x = x;
		self.y = y;
		self.clamp();
		(self.x, self.y) != before
	}

	/// Page Down (`+1`) or Page Up (`-1`): a view less a few lines, so the last lines stay in sight.
	pub fn page_by(&mut self, dir: i32) -> bool {
		let (_, vh) = self.visible();
		let step = (vh - LINE_STEP).max(LINE_STEP);
		self.scroll_by(0, step * dir.signum())
	}

	/// Home or End: the top or the bottom of the document.
	pub fn to_edge(&mut self, end: bool) -> bool {
		self.scroll_to(self.x, if end { i32::MAX } else { 0 })
	}

	/// The sheet's left edge in document pixels from the view's left (the page is centred when narrower than the view).
	pub fn page_left(&self) -> i32 {
		page_x0(self.visible().0, self.geom.page_w, self.x)
	}

	/// Scrolls the least that brings a caret into view. `view_x` is the caret's left edge in document pixels from the view's left,
	/// `content_y` its top in document pixels from the top of the content, and `h` its height. A margin of a line is kept
	/// around it, so a caret that was just off the edge is not left against it. Returns true when the view moved.
	pub fn reveal(&mut self, view_x: i32, content_y: i32, h: i32) -> bool {
		const MARGIN: i32 = 24;
		const TOP_SNAP: i32 = 120;
		let (vw, vh) = self.visible();
		let mut y = self.y;
		if content_y < y {
			// Near the top of the first sheet the view goes all the way up, so the paper's top edge shows.
			y = if content_y - MARGIN < TOP_SNAP {
				0
			} else {
				content_y - MARGIN
			};
		} else if content_y + h > y + vh {
			y = content_y + h - vh + MARGIN;
		}
		let mut x = self.x;
		if view_x < 8 {
			x += view_x - MARGIN;
		} else if view_x > vw - 8 {
			x += view_x - vw + MARGIN;
		}
		self.scroll_to(x, y)
	}

	/// The sheets that are on screen. Each sheet is placed by `quill_app::view::page_x0`, so a page narrower than the window is
	/// centred and a wider one scrolls from its padding.
	pub fn slots(&self) -> Vec<Slot> {
		let (vw, vh) = self.visible();
		let step = self.geom.page_h + PAGE_GAP;
		let first = ((self.y - PAGE_TOP).max(0) / step.max(1)) as u32;
		let left = page_x0(vw, self.geom.page_w, self.x);
		let mut out = Vec::new();
		for i in first..self.geom.pages {
			let top = self.geom.page_top(i) - self.y;
			if top >= vh {
				break;
			}
			if top + self.geom.page_h > 0 {
				out.push(Slot {
					page: i + 1,
					left,
					top,
				});
			}
		}
		out
	}

	/// The sheet the status line names: the one under the middle of the view, or the nearest when the middle is in a gap.
	pub fn current_page(&self) -> u32 {
		let (_, vh) = self.visible();
		let mid = self.y + vh / 2;
		let step = (self.geom.page_h + PAGE_GAP).max(1);
		let i = ((mid - PAGE_TOP).max(0) / step) as u32;
		i.min(self.geom.pages.saturating_sub(1)) + 1
	}

	/// Sets the zoom. The document point under the device position `anchor` (or the middle of the view) stays under it.
	pub fn set_zoom(&mut self, zoom: Zoom, anchor: Option<(f64, f64)>) {
		let (dw, dh) = self.device;
		let (ax, ay) = anchor.unwrap_or((f64::from(dw) / 2.0, f64::from(dh) / 2.0));
		let before = self.mapping();
		let (sx, sy) = (
			f64::from(before.scale().raw()) / 256.0,
			f64::from(before.scale().raw()) / 256.0,
		);
		let (px, py) = (f64::from(self.x) + ax / sx, f64::from(self.y) + ay / sy);
		self.zoom = match zoom {
			Zoom::Percent(p) => Zoom::Percent(p.clamp(ZOOM_MIN, ZOOM_MAX)),
			Zoom::Width => Zoom::Width,
		};
		let after = f64::from(self.mapping().scale().raw()) / 256.0;
		self.x = (px - ax / after).round() as i32;
		self.y = (py - ay / after).round() as i32;
		self.clamp();
	}

	/// Ctrl+plus: the next zoom above the one drawn.
	pub fn zoom_in(&mut self, anchor: Option<(f64, f64)>) {
		let now = self.effective_zoom();
		let next = ZOOM_STEPS
			.iter()
			.copied()
			.find(|z| *z > now)
			.unwrap_or(ZOOM_MAX);
		self.set_zoom(Zoom::Percent(next), anchor);
	}

	/// Ctrl+minus: the next zoom below the one drawn.
	pub fn zoom_out(&mut self, anchor: Option<(f64, f64)>) {
		let now = self.effective_zoom();
		let next = ZOOM_STEPS
			.iter()
			.rev()
			.copied()
			.find(|z| *z < now)
			.unwrap_or(ZOOM_MIN);
		self.set_zoom(Zoom::Percent(next), anchor);
	}

	/// Ctrl+0: back to 100 %.
	pub fn zoom_reset(&mut self) {
		self.set_zoom(Zoom::Percent(100), None);
	}
}

/// The status line's page text: `Page 2 of 5`.
pub fn page_label(current: u32, total: u32) -> String {
	format!("Page {current} of {}", total.max(1))
}

/// The status line's zoom text: `150%`.
pub fn zoom_label(percent: u32) -> String {
	format!("{percent}%")
}

#[cfg(test)]
mod tests {
	use super::*;

	fn view(w: u32, h: u32, scale_120: u32, pages: u32) -> View {
		let mut v = View::new(Zoom::Percent(100));
		v.set_viewport(w, h, scale_120);
		v.set_geometry(Geometry {
			pages,
			..Geometry::default()
		});
		v
	}

	#[test]
	fn a_device_pixel_maps_back_to_the_document_pixel_under_it() {
		for (zoom, scale) in [(100, 120), (100, 180), (125, 150), (50, 240), (200, 120)] {
			let m = Mapping::new(zoom, scale);
			for px in [0, 1, 7, 100, 333, 1000] {
				let dev = m.dev(px);
				assert!((m.to_doc(dev) - px).abs() <= 1, "{zoom} {scale} {px}");
			}
		}
	}

	#[test]
	fn revealing_a_caret_scrolls_the_least_and_keeps_a_margin() {
		let mut v = View::new(Zoom::Percent(100));
		v.set_viewport(800, 400, 120);
		v.set_geometry(Geometry {
			page_w: 816,
			page_h: 1056,
			pages: 2,
		});
		assert!(!v.reveal(100, 200, 20), "in view: nothing moves");
		assert_eq!(v.scroll().1, 0);
		assert!(v.reveal(100, 900, 20));
		assert_eq!(v.scroll().1, 900 + 20 - 400 + 24);
		assert!(v.reveal(100, 300, 20));
		assert_eq!(v.scroll().1, 276);
		assert!(v.reveal(100, 50, 20));
		assert_eq!(v.scroll().1, 0, "near the top it goes all the way");
	}

	#[test]
	fn the_mapping_joins_zoom_and_display_scale_in_one_number() {
		assert_eq!(Mapping::new(100, 120).scale().raw(), 256);
		assert_eq!(Mapping::new(100, 180).scale().raw(), 384);
		assert_eq!(Mapping::new(150, 180).scale().raw(), 576);
		assert_eq!(Mapping::new(25, 120).scale().raw(), 64);
		assert_eq!(Mapping::new(999, 120).zoom_pct, ZOOM_MAX);
		assert_eq!(Mapping::scale_120_of(1.5), 180);
		assert_eq!(Mapping::scale_120_of(1.25), 150);
		assert_eq!(Mapping::scale_120_of(1.0 / 3.0 + 1.0), 160);
	}

	#[test]
	fn device_sizes_round_the_widget_size_by_the_display_scale_only() {
		let m = Mapping::new(100, 180);
		assert_eq!(m.device_size(100, 50), (150, 75));
		assert_eq!(m.device_size(101, 51), (152, 77));
		assert_eq!(Mapping::new(200, 180).device_size(101, 51), (152, 77));
		assert_eq!(m.device_size(-5, 0), (0, 0));
	}

	#[test]
	fn a_document_length_covers_the_whole_device_length() {
		for (z, s) in [(100, 120), (100, 180), (125, 150), (75, 180), (200, 360)] {
			let m = Mapping::new(z, s);
			for dev in [1u32, 2, 3, 149, 150, 151, 999, 1920] {
				let doc = m.doc_ceil(dev);
				assert!(m.dev(doc) >= dev as i32 - 1, "{z} {s} {dev} -> {doc}");
				assert!(m.dev(doc - 1) <= dev as i32, "{z} {s} {dev} -> {doc}");
			}
		}
	}

	#[test]
	fn geometry_stacks_the_sheets_with_gaps() {
		let g = Geometry {
			pages: 3,
			..Geometry::default()
		};
		assert_eq!(g.page_top(0), PAGE_TOP);
		assert_eq!(g.page_top(1), PAGE_TOP + 1056 + PAGE_GAP);
		assert_eq!(g.content_h(), PAGE_TOP + 3 * 1056 + 2 * PAGE_GAP + PAGE_PAD);
		assert_eq!(g.content_w(), 816 + 2 * PAGE_PAD);
	}

	#[test]
	fn scrolling_stops_at_both_ends_and_reports_a_change() {
		let mut v = view(1000, 800, 120, 2);
		assert_eq!(v.scroll(), (0, 0));
		assert!(!v.scroll_by(0, -10));
		assert!(v.scroll_by(0, 100));
		assert_eq!(v.scroll(), (0, 100));
		assert!(v.to_edge(true));
		let (_, max) = v.max_scroll();
		assert_eq!(v.scroll().1, max);
		assert!(!v.scroll_by(0, 5));
		assert!(v.to_edge(false));
		assert_eq!(v.scroll().1, 0);
	}

	#[test]
	fn a_page_narrower_than_the_view_is_centred_and_does_not_scroll_across() {
		let v = view(1200, 800, 120, 1);
		assert_eq!(v.max_scroll().0, 0);
		let s = v.slots();
		assert_eq!(s.len(), 1);
		assert_eq!(s[0].left, (1200 - 816) / 2);
		assert_eq!(s[0].top, PAGE_TOP);
	}

	#[test]
	fn a_page_wider_than_the_view_scrolls_across() {
		let mut v = view(500, 800, 120, 1);
		assert_eq!(v.max_scroll().0, 816 + 2 * PAGE_PAD - 500);
		assert_eq!(v.slots()[0].left, PAGE_PAD);
		v.scroll_by(40, 0);
		assert_eq!(v.slots()[0].left, PAGE_PAD - 40);
	}

	#[test]
	fn the_visible_sheets_follow_the_scroll_and_the_status_names_the_one_in_the_middle() {
		let mut v = view(1000, 800, 120, 3);
		assert_eq!(v.slots().iter().map(|s| s.page).collect::<Vec<_>>(), [1]);
		assert_eq!(v.current_page(), 1);
		v.scroll_to(0, 1056 + PAGE_GAP + 100);
		assert_eq!(v.slots().iter().map(|s| s.page).collect::<Vec<_>>(), [2]);
		assert_eq!(v.current_page(), 2);
		v.scroll_to(0, 1056 - 300);
		assert_eq!(v.slots().iter().map(|s| s.page).collect::<Vec<_>>(), [1, 2]);
		v.to_edge(true);
		assert_eq!(v.current_page(), 3);
		assert_eq!(page_label(v.current_page(), 3), "Page 3 of 3");
	}

	#[test]
	fn page_keys_keep_a_few_lines_in_sight() {
		let mut v = view(1000, 800, 120, 3);
		v.page_by(1);
		assert_eq!(v.scroll().1, 800 - LINE_STEP);
		v.page_by(-1);
		assert_eq!(v.scroll().1, 0);
	}

	#[test]
	fn zoom_steps_walk_the_table_and_stop_at_the_limits() {
		let mut v = view(1000, 800, 120, 1);
		v.zoom_in(None);
		assert_eq!(v.effective_zoom(), 110);
		v.zoom_out(None);
		v.zoom_out(None);
		assert_eq!(v.effective_zoom(), 90);
		for _ in 0..20 {
			v.zoom_out(None);
		}
		assert_eq!(v.effective_zoom(), ZOOM_MIN);
		for _ in 0..20 {
			v.zoom_in(None);
		}
		assert_eq!(v.effective_zoom(), ZOOM_MAX);
		v.zoom_reset();
		assert_eq!(v.zoom(), Zoom::Percent(100));
		assert_eq!(zoom_label(v.effective_zoom()), "100%");
	}

	#[test]
	fn zooming_keeps_the_point_under_the_pointer_where_it_was() {
		let mut v = view(900, 700, 120, 4);
		v.scroll_to(0, 2000);
		let at = |v: &View, ax: f64, ay: f64| {
			let s = f64::from(v.mapping().scale().raw()) / 256.0;
			(
				f64::from(v.scroll().0) + ax / s,
				f64::from(v.scroll().1) + ay / s,
			)
		};
		let before = at(&v, 300.0, 200.0);
		v.set_zoom(Zoom::Percent(150), Some((300.0, 200.0)));
		let after = at(&v, 300.0, 200.0);
		assert!((before.1 - after.1).abs() < 1.0, "{before:?} {after:?}");
	}

	#[test]
	fn page_width_zoom_fits_the_sheet_and_its_padding() {
		let mut v = view(1664, 800, 120, 1);
		v.set_zoom(Zoom::Width, None);
		assert_eq!(v.effective_zoom(), 100);
		v.set_viewport(832, 800, 120);
		assert_eq!(v.effective_zoom(), 100);
		v.set_viewport(500, 800, 120);
		assert_eq!(
			v.effective_zoom(),
			(500 * 100 / (816 + 2 * PAGE_PAD)) as u32
		);
		v.set_viewport(100, 800, 120);
		assert_eq!(v.effective_zoom(), ZOOM_MIN);
	}

	#[test]
	fn the_display_scale_does_not_change_what_fits_a_page_to_the_width() {
		let mut a = view(900, 800, 120, 1);
		a.set_zoom(Zoom::Width, None);
		let mut b = view(1350, 800, 180, 1);
		b.set_zoom(Zoom::Width, None);
		assert_eq!(a.effective_zoom(), b.effective_zoom());
	}

	#[test]
	fn scroll_bar_ranges_describe_the_content_and_the_view() {
		let mut v = view(1600, 800, 180, 2);
		let r = v.range(false);
		assert_eq!(r.upper, v.geometry().content_h());
		assert_eq!(r.page, v.visible().1);
		assert_eq!(r.max, r.upper - r.page);
		v.scroll_to(0, 50);
		assert_eq!(v.range(false).value, 50);
		assert_eq!(v.range(true).max, 0);
	}

	#[test]
	fn a_new_document_scrolls_back_to_the_top() {
		let mut v = view(1000, 800, 120, 3);
		v.scroll_to(0, 900);
		v.set_geometry(Geometry::default());
		assert_eq!(v.scroll(), (0, 0));
	}
}
