// Selecting and sizing a picture: the geometry, the keys, the pointer on the graphics screen, one undo step for a drag, and the
// frame the painter draws.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod common;

use common::*;
use proptest::prelude::*;
use quill_app::mouse::{Ctx, Ev, Pointer};
use quill_app::picture_geom::{self as geom, Frame, Handle, Limits, HANDLE, MIN_SHOWN, SLOP};
use quill_app::{Command, Key};
use quill_chrome::view::{page_width, page_x0, PageGeom, PAGE_TOP};
use quill_host::script::MemFiles;
use quill_host::HostStore;
use quill_paint::layout::Layout;
use quill_paint::shell::{self, hscroll, Pixels};

struct Rig {
	app: TApp,
	s: HostStore,
	f: MemFiles,
	p: Pointer,
}

impl Rig {
	fn new() -> Rig {
		let (mut app, s, f) = open_doc("pictures.rtf");
		shell::sync_view(&mut app, &s);
		Rig {
			app,
			s,
			f,
			p: Pointer::default(),
		}
	}

	fn stride(&self) -> i32 {
		PageGeom::of(&self.app, &self.s).page_h + quill_chrome::view::PAGE_GAP
	}

	/// Where the first picture is, in the page's units.
	fn first(&self) -> geom::PicHit {
		(0..self.app.doc().body.len())
			.find_map(|p| self.app.picture_at_pos(&self.s, p, self.stride()))
			.expect("a picture")
	}

	/// The screen pixel of a point of the page.
	fn pixel(&self, x: i32, y: i32) -> (i32, i32) {
		let area = Layout::of(&self.app).page;
		let g = PageGeom::of(&self.app, &self.s);
		(
			x + area.left
				+ page_x0(
					area.width(),
					page_width(&self.app),
					hscroll(&self.app, area.width()),
				),
			y + area.top + PAGE_TOP - g.top,
		)
	}

	fn ev(&mut self, ev: Ev, shift: bool) {
		let space = Pixels::of(&self.app);
		let ctx = Ctx { shift, tick: 0 };
		self.p
			.handle(&mut self.app, &mut self.s, &mut self.f, &space, ev, ctx);
	}

	fn click(&mut self, x: i32, y: i32) {
		let (x, y) = self.pixel(x, y);
		self.ev(Ev::Down { x, y }, false);
		self.ev(Ev::Up { x, y }, false);
	}

	fn drag(&mut self, from: (i32, i32), to: (i32, i32), shift: bool) {
		let (x, y) = self.pixel(from.0, from.1);
		self.ev(Ev::Down { x, y }, false);
		let (x2, y2) = self.pixel(to.0, to.1);
		self.ev(Ev::Move { x: x2, y: y2 }, shift);
		self.ev(Ev::Up { x: x2, y: y2 }, shift);
	}

	fn shown(&self) -> (i32, i32) {
		let v = self.app.picture_size_values(&self.s).unwrap();
		(v.w, v.h)
	}

	fn key(&mut self, k: Key) {
		self.app.key(&mut self.s, &mut self.f, k);
	}
}

fn centre(r: &Frame) -> (i32, i32) {
	((r.left + r.right) / 2, (r.top + r.bottom) / 2)
}

#[test]
fn a_click_inside_a_picture_selects_it_and_outside_does_not() {
	let mut r = Rig::new();
	let hit = r.first();
	let (cx, cy) = centre(&hit.rect);
	r.click(cx, cy);
	assert_eq!(r.app.selection(), Some((hit.pos, hit.pos + 1)));
	assert_eq!(r.app.selected_picture(&r.s), Some((hit.pos, hit.idx)));
	// A click in the text below puts a caret and drops the selection.
	r.click(hit.rect.left + 4, hit.rect.bottom + 8);
	assert_eq!(r.app.selected_picture(&r.s), None);
	// Just outside the picture's edge is not a hit.
	assert!(r
		.app
		.picture_at(&r.s, hit.rect.right + 1, cy, r.stride())
		.is_none());
	assert!(r
		.app
		.picture_at(&r.s, hit.rect.right - 1, cy, r.stride())
		.is_some());
}

#[test]
fn the_keys_select_collapse_delete_and_undo() {
	let mut r = Rig::new();
	let hit = r.first();
	r.app.place_caret(&r.s, hit.pos, false);
	r.key(Key::Select(quill_app::Dir::Right));
	assert_eq!(r.app.selected_picture(&r.s), Some((hit.pos, hit.idx)));
	// The status line says what is selected.
	let mut out = [0u8; 80];
	let n = quill_app::status_text(&r.app, &r.s, &mut out);
	let text = String::from_utf8_lossy(&out[..n]).into_owned();
	assert!(
		text.contains("Picture 64 x 48 pixels, 0.67 x 0.50 in (100%)"),
		"{text}"
	);
	r.key(Key::Esc);
	assert_eq!(r.app.selection(), None);
	assert_eq!(r.app.selected_picture(&r.s), None);
	// Delete removes it in one step, and undo brings it back.
	r.app.place_caret(&r.s, hit.pos, false);
	r.key(Key::Select(quill_app::Dir::Right));
	let before = r.app.doc().body.len();
	r.key(Key::Delete);
	assert_eq!(r.app.doc().body.len(), before - 1);
	r.app.command(&mut r.s, &mut r.f, Command::Undo);
	assert_eq!(r.app.doc().body.len(), before);
	assert!(r.app.doc().body.object_at(&r.s, hit.pos).is_some());
}

#[test]
fn tab_and_shift_tab_move_between_pictures() {
	let mut r = Rig::new();
	let mut at = Vec::new();
	let mut pos = r.app.neighbour_picture(&r.s, true);
	r.app.place_caret(&r.s, 0, false);
	while let Some(p) = pos {
		at.push(p);
		r.app.select_picture(&r.s, p);
		pos = r.app.neighbour_picture(&r.s, true);
	}
	assert!(!at.is_empty());
	r.app.select_picture(&r.s, at[0]);
	for want in at.iter().skip(1) {
		r.key(Key::Tab);
		assert_eq!(r.app.selection(), Some((*want, *want + 1)));
	}
	for want in at.iter().rev().skip(1) {
		r.key(Key::BackTab);
		assert_eq!(r.app.selection(), Some((*want, *want + 1)));
	}
	// With nothing past it, Tab leaves the picture selected and writes no tab.
	let len = r.app.doc().body.len();
	r.app.select_picture(&r.s, *at.last().unwrap());
	r.key(Key::Tab);
	assert_eq!(r.app.doc().body.len(), len);
}

#[test]
fn a_corner_drag_keeps_the_proportions_and_is_one_undo_step() {
	let mut r = Rig::new();
	let hit = r.first();
	let (w0, h0) = (960, 720);
	let (cx, cy) = centre(&hit.rect);
	r.click(cx, cy);
	let sw = geom::handle_box(&hit.rect, Handle::SouthEast, HANDLE);
	let from = centre(&sw);
	r.drag(from, (from.0 + 48, from.1 + 3), false);
	let (w, h) = r.shown();
	assert_eq!(w, w0 + 48 * 15);
	assert_eq!(h, w * h0 / w0);
	assert_eq!(r.app.selected_picture(&r.s).map(|p| p.0), Some(hit.pos));
	// Shift frees the proportions.
	let hit = r.first();
	let se = centre(&geom::handle_box(&hit.rect, Handle::SouthEast, HANDLE));
	let (w1, h1) = r.shown();
	r.drag(se, (se.0 + 30, se.1 + 60), true);
	assert_eq!(r.shown(), (w1 + 30 * 15, h1 + 60 * 15));
	// Undo twice gives the original size back, one step a drag; redo goes forward again.
	let pos = hit.pos;
	r.app.command(&mut r.s, &mut r.f, Command::Undo);
	r.app.select_picture(&r.s, pos);
	assert_eq!(r.shown(), (w1, h1));
	r.app.command(&mut r.s, &mut r.f, Command::Undo);
	r.app.select_picture(&r.s, pos);
	assert_eq!(r.shown(), (w0, h0));
	r.app.command(&mut r.s, &mut r.f, Command::Redo);
	r.app.select_picture(&r.s, pos);
	assert_eq!(r.shown(), (w1, h1));
	assert_eq!(r.app.doc().pictures.len(), 4);
}

#[test]
fn size_picture_sets_values_and_limits_them() {
	let mut r = Rig::new();
	let hit = r.first();
	r.app.select_picture(&r.s, hit.pos);
	assert!(r.app.size_picture(&mut r.s, 1920, 1440));
	assert_eq!(r.shown(), (1920, 1440));
	let v = r.app.picture_size_values(&r.s).unwrap();
	assert_eq!((v.pct_w, v.pct_h, v.px_w, v.px_h), (200, 200, 64, 48));
	// The same size again is no change; a tiny one stops at a tenth of an inch; a huge one at the text area.
	assert!(!r.app.size_picture(&mut r.s, 1920, 1440));
	assert!(r.app.size_picture(&mut r.s, 1, 1));
	assert_eq!(r.shown(), (MIN_SHOWN, MIN_SHOWN));
	assert!(r.app.size_picture(&mut r.s, 1_000_000, 1_000_000));
	let lim = r.app.picture_limits();
	assert_eq!(r.shown(), (lim.max_w, lim.max_h));
	// Sizing back to a size the picture had reuses its record.
	let n = r.app.doc().pictures.len();
	assert!(r.app.size_picture(&mut r.s, 1920, 1440));
	assert_eq!(r.app.doc().pictures.len(), n);
	r.app.command(&mut r.s, &mut r.f, Command::Undo);
	r.app.select_picture(&r.s, hit.pos);
	assert_eq!(r.shown(), (lim.max_w, lim.max_h));
}

fn count(f: &Frame, g: &Frame) -> bool {
	f.left < g.right && g.left < f.right && f.top < g.bottom && g.top < f.bottom
}

proptest! {
	/// A point taken from a handle's box (and its slop) is that handle; the boxes do not overlap each other or the picture.
	#[test]
	fn handles_hit_inside_their_slop(
		w in 12i32..400, h in 12i32..400, x0 in -50i32..500, y0 in -50i32..500,
		which in 0usize..8, fx in 0i32..100, fy in 0i32..100
	) {
		let rect = Frame { left: x0, top: y0, right: x0 + w, bottom: y0 + h };
		let boxes = geom::handles(&rect, HANDLE);
		for (i, a) in boxes.iter().enumerate() {
			prop_assert!(!count(a, &rect));
			for b in boxes.iter().skip(i + 1) {
				prop_assert!(!count(a, b), "{a:?} {b:?}");
			}
		}
		let b = boxes[which];
		let x = b.left + fx % HANDLE;
		let y = b.top + fy % HANDLE;
		prop_assert_eq!(geom::handle_at(&rect, x, y, HANDLE, 0), Some(Handle::ALL[which]));
		let near = geom::handle_at(&rect, x, y, HANDLE, SLOP);
		prop_assert!(near.is_some());
		// Whatever a point hits is within the slop of that handle's box.
		if let Some(hd) = geom::handle_at(&rect, x0 + fx - 20, y0 + fy - 20, HANDLE, SLOP) {
			let bx = geom::handle_box(&rect, hd, HANDLE).grow(SLOP);
			prop_assert!(bx.contains(x0 + fx - 20, y0 + fy - 20));
		}
		// Inside the picture is never a handle.
		prop_assert_eq!(geom::handle_at(&rect, x0 + w / 2, y0 + h / 2, HANDLE, SLOP), None);
	}

	/// A resize stays inside the limits (or the size it started at, when that was already past them), and a corner keeps the
	/// proportions to within a twip unless Shift frees them.
	#[test]
	fn a_resize_stays_in_its_limits(
		w0 in 144i32..9000, h0 in 144i32..12000, dx in -20000i32..20000, dy in -20000i32..20000,
		which in 0usize..8, free in any::<bool>()
	) {
		let lim = Limits { max_w: 8640, max_h: 12960 };
		let hd = Handle::ALL[which];
		let (w, h) = geom::resize(hd, w0, h0, dx, dy, free, lim);
		prop_assert!(w >= MIN_SHOWN && h >= MIN_SHOWN);
		prop_assert!(w <= lim.max_w.max(w0) && h <= lim.max_h.max(h0));
		let (sx, sy) = hd.direction();
		if sx == 0 { prop_assert_eq!(w, w0); }
		if sy == 0 { prop_assert_eq!(h, h0); }
		if hd.is_corner() && !free {
			let (a, b) = (i64::from(w) * i64::from(h0), i64::from(h) * i64::from(w0));
			prop_assert!((a - b).abs() <= i64::from(w0.max(h0)), "{w}x{h} from {w0}x{h0}");
		}
		// No movement is no change (for a picture inside the limits).
		if w0 <= lim.max_w && h0 <= lim.max_h {
			prop_assert_eq!(geom::resize(hd, w0, h0, 0, 0, free, lim), (w0, h0));
		}
	}
}

/// The frame is drawn where the geometry says: black outside the picture's pixels, eight handles, nothing on the picture.
#[test]
fn the_painter_draws_the_frame_where_the_geometry_says() {
	let mut r = Rig::new();
	let hit = r.first();
	r.app.select_picture(&r.s, hit.pos);
	let job = |app: &TApp, s: &HostStore| frame(Mode::C16, &mut PageJob::new(app, s));
	let fr = job(&r.app, &r.s);
	// The painter's frames use `AREA` and no sideways scroll.
	let g = PageGeom::of(&r.app, &r.s);
	let to_px = |x: i32, y: i32| {
		(
			x + AREA.left + page_x0(AREA.width(), page_width(&r.app), 0),
			y + AREA.top + PAGE_TOP - g.top,
		)
	};
	let (x, y) = to_px(hit.rect.left, hit.rect.top);
	let black = 0u8;
	let at = |x: i32, y: i32| fr.pixels[(y * 640 + x) as usize];
	// The top edge of the frame is the row above the picture; a handle's corner pixel is black; the picture's own corner is not.
	assert_eq!(at(x + 10, y - 1), black);
	assert_eq!(at(x - 1, y + 10), black);
	let hb = geom::handle_box(&hit.rect, Handle::NorthWest, HANDLE);
	let (hx, hy) = to_px(hb.left, hb.top);
	assert_eq!(at(hx, hy), black);
	assert_eq!(at(hx + 6, hy + 6), black);
	assert_ne!(at(x + 10, y + 10), black);
	// Unselected: none of it.
	r.key(Key::Esc);
	let plain = job(&r.app, &r.s);
	assert_ne!(plain.pixels[((y - 1) * 640 + x + 10) as usize], black);
}
