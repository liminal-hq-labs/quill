// The ruler: the text's width in inches, the indent markers and the tab stops, over the page and in its scale.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// `layout` turns the paragraph's `RulerState` and where the sheet sits in the window into the positions a drawing needs, in
// logical pixels; it is plain data, shared with the Windows ruler through `quill_app::view` (ticks, marks, hit tests, drags), and
// tested without a display. `QuillRuler` draws that with `GtkSnapshot` in the widget's own foreground colour and the system
// accent, so it follows the dark and the light scheme. The tab-kind button is a separate `gtk::Button` at its left.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::gdk;
use gtk::glib;
use gtk::graphene;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use quill_app::view::{
	drag_to, ruler_hit_with, ruler_marks, ruler_ticks, snap, Drag, Hit, RulerState, RULER_H,
	TWIPS_PX,
};

/// How the ruler lines up with the page.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct RulerGeometry {
	/// The sheet's left edge in logical pixels from the ruler's left.
	pub page_left: f64,
	/// The zoom as a ratio (1.0 is 100 %).
	pub zoom: f64,
	/// The left margin in document pixels (96 to the inch).
	pub margin_px: i32,
	/// The text's width in document pixels.
	pub text_px: i32,
}

/// What a tick on the scale is.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Tick {
	/// Its x in logical pixels.
	pub x: f64,
	/// Its height in pixels: 0 for a whole inch, which carries a number.
	pub kind: i32,
	/// The inch it numbers, for a whole inch.
	pub inch: i32,
}

/// Where everything on the ruler is, in logical pixels from its left edge.
#[derive(Clone, Debug, PartialEq)]
pub struct RulerDraw {
	/// The text area: left and right.
	pub text: (f64, f64),
	/// The scale's ticks.
	pub ticks: Vec<Tick>,
	/// The first-line, left and right indent markers.
	pub marks: (f64, f64, f64),
	/// The tab stops: x and kind.
	pub tabs: Vec<(f64, u8)>,
}

/// Works out the ruler's drawing for a paragraph.
pub fn layout(g: &RulerGeometry, st: &RulerState) -> RulerDraw {
	let x = |doc: i32| g.page_left + f64::from(g.margin_px + doc) * g.zoom;
	let (f, l, r) = ruler_marks(st, 0, g.text_px);
	RulerDraw {
		text: (x(0), x(g.text_px)),
		ticks: ruler_ticks(g.text_px)
			.map(|(dx, kind, inch)| Tick {
				x: x(dx),
				kind,
				inch,
			})
			.collect(),
		marks: (x(f), x(l), x(r)),
		tabs: st
			.stops()
			.iter()
			.map(|(p, k)| (x(p / TWIPS_PX), *k))
			.collect(),
	}
}

/// What a person's work on the ruler changes.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RulerEdit {
	/// New indents in twips: left, first-line (absolute) and right.
	Indents { left: i32, first: i32, right: i32 },
	/// A tab stop of this kind at this position in twips.
	AddTab(i32, u8),
	/// The tab stop at this position goes.
	RemoveTab(i32),
}

/// The kinds of tab stop the kind button cycles through, with their names.
pub const KINDS: [&str; 4] = ["Left", "Centre", "Right", "Decimal"];

/// What the ruler calls when the person edits the paragraph.
type EditCallback = Rc<dyn Fn(RulerEdit)>;

mod imp {
	use super::*;

	#[derive(Default)]
	pub struct QuillRuler {
		pub geometry: Cell<Option<RulerGeometry>>,
		pub state: Cell<Option<RulerState>>,
		pub drag: RefCell<Option<Drag>>,
		pub kind: Cell<u8>,
		pub on_edit: RefCell<Option<EditCallback>>,
	}

	#[glib::object_subclass]
	impl ObjectSubclass for QuillRuler {
		const NAME: &'static str = "QuillRuler";
		type Type = super::QuillRuler;
		type ParentType = gtk::Widget;
	}

	impl ObjectImpl for QuillRuler {
		fn constructed(&self) {
			self.parent_constructed();
			let obj = self.obj();
			obj.set_hexpand(true);
			obj.set_overflow(gtk::Overflow::Hidden);
			obj.add_css_class("quill-ruler");
			obj.set_tooltip_text(Some(
				"Ruler: drag the markers to set indents; click the scale to set a tab stop",
			));
			let drag = gtk::GestureDrag::new();
			let weak = obj.downgrade();
			drag.connect_drag_begin(move |_, x, y| {
				if let Some(r) = weak.upgrade() {
					r.drag_begin(x, y);
				}
			});
			let weak = obj.downgrade();
			drag.connect_drag_update(move |g, dx, _| {
				if let (Some(r), Some((sx, _))) = (weak.upgrade(), g.start_point()) {
					r.drag_update(sx + dx);
				}
			});
			let weak = obj.downgrade();
			drag.connect_drag_end(move |g, dx, _| {
				if let (Some(r), Some((sx, _))) = (weak.upgrade(), g.start_point()) {
					r.drag_end(sx + dx);
				}
			});
			obj.add_controller(drag);
		}
	}

	impl WidgetImpl for QuillRuler {
		fn measure(&self, o: gtk::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
			match o {
				gtk::Orientation::Vertical => (RULER_H, RULER_H, -1, -1),
				_ => (60, 60, -1, -1),
			}
		}

		fn snapshot(&self, snapshot: &gtk::Snapshot) {
			let obj = self.obj();
			let (Some(g), Some(st)) = (self.geometry.get(), self.drawn_state()) else {
				return;
			};
			let d = layout(&g, &st);
			let (w, h) = (obj.width() as f32, obj.height() as f32);
			let fg = obj.color();
			let with_alpha = |a: f32| gdk::RGBA::new(fg.red(), fg.green(), fg.blue(), a);
			let rect = |x: f64, y: f32, rw: f32, rh: f32| graphene::Rect::new(x as f32, y, rw, rh);
			// The text area is a little lighter than the margins either side.
			snapshot.append_color(
				&with_alpha(0.07),
				&rect(d.text.0, 0.0, (d.text.1 - d.text.0) as f32, h - 1.0),
			);
			snapshot.append_color(&with_alpha(0.2), &rect(0.0, h - 1.0, w, 1.0));
			let layout_of = |text: &str| {
				let l = obj.create_pango_layout(Some(text));
				let attrs = gtk::pango::AttrList::new();
				attrs.insert(gtk::pango::AttrSize::new(8 * gtk::pango::SCALE));
				l.set_attributes(Some(&attrs));
				l
			};
			for t in &d.ticks {
				let tall = match t.kind {
					0 => 8.0,
					3 => 5.0,
					_ => 3.0,
				};
				snapshot.append_color(&with_alpha(0.55), &rect(t.x.round(), 9.0, 1.0, tall));
				if t.kind == 0 && t.inch > 0 {
					let l = layout_of(&t.inch.to_string());
					let (lw, _) = l.pixel_size();
					snapshot.save();
					snapshot.translate(&graphene::Point::new(t.x as f32 - lw as f32 / 2.0, 0.0));
					snapshot.append_layout(&l, &with_alpha(0.75));
					snapshot.restore();
				}
			}
			let accent = adw::StyleManager::default().accent_color_rgba();
			let tri = |x: f64, top: bool| {
				let b = gsk_triangle(x as f32, if top { 1.0 } else { h - 2.0 }, top);
				snapshot.append_fill(&b, gtk::gsk::FillRule::Winding, &accent);
			};
			tri(d.marks.0, true);
			tri(d.marks.1, false);
			tri(d.marks.2, false);
			// A box under the left marker moves both indents, as on the Windows ruler.
			snapshot.append_color(&accent, &rect(d.marks.1 - 3.0, h - 5.0, 7.0, 3.0));
			for (x, k) in &d.tabs {
				let x = x.round();
				snapshot.append_color(&accent, &rect(x, 9.0, 1.0, 9.0));
				match k {
					0 => snapshot.append_color(&accent, &rect(x, 17.0, 5.0, 1.0)),
					2 => snapshot.append_color(&accent, &rect(x - 4.0, 17.0, 5.0, 1.0)),
					_ => snapshot.append_color(&accent, &rect(x - 3.0, 17.0, 7.0, 1.0)),
				}
				if *k == 3 {
					snapshot.append_color(&accent, &rect(x + 2.0, 13.0, 2.0, 2.0));
				}
			}
		}
	}

	impl QuillRuler {
		/// The state being dragged, else the one set.
		pub fn drawn_state(&self) -> Option<RulerState> {
			self.drag
				.borrow()
				.as_ref()
				.map(|d| d.state)
				.or(self.state.get())
		}
	}
}

fn gsk_triangle(x: f32, y: f32, down: bool) -> gtk::gsk::Path {
	let b = gtk::gsk::PathBuilder::new();
	if down {
		b.move_to(x - 4.0, y);
		b.line_to(x + 4.0, y);
		b.line_to(x, y + 6.0);
	} else {
		b.move_to(x - 4.0, y);
		b.line_to(x + 4.0, y);
		b.line_to(x, y - 6.0);
	}
	b.close();
	b.to_path()
}

glib::wrapper! {
	/// The ruler across the top of the page area.
	pub struct QuillRuler(ObjectSubclass<imp::QuillRuler>) @extends gtk::Widget, @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for QuillRuler {
	fn default() -> Self {
		Self::new()
	}
}

impl QuillRuler {
	/// A ruler with nothing to show yet.
	pub fn new() -> QuillRuler {
		glib::Object::new()
	}

	/// Calls `f` when the person changes the paragraph on the ruler.
	pub fn connect_edit(&self, f: impl Fn(RulerEdit) + 'static) {
		self.imp().on_edit.replace(Some(Rc::new(f)));
	}

	/// Shows a paragraph on the sheet's present place.
	pub fn show(&self, g: RulerGeometry, st: RulerState) {
		self.imp().geometry.set(Some(g));
		self.imp().state.set(Some(st));
		self.queue_draw();
	}

	/// The kind of tab stop a click on the scale sets (0 left to 3 decimal).
	pub fn kind(&self) -> u8 {
		self.imp().kind.get()
	}

	/// Chooses the kind of tab stop.
	pub fn set_kind(&self, kind: u8) {
		self.imp().kind.set(kind % 4);
	}

	fn hit(&self, x: f64, y: f64) -> Option<(Hit, RulerGeometry, RulerState)> {
		let g = self.imp().geometry.get()?;
		let st = self.imp().drawn_state()?;
		let text_left = (g.page_left + f64::from(g.margin_px) * g.zoom).round() as i32;
		let text_w = (f64::from(g.text_px) * g.zoom).round() as i32;
		ruler_hit_with(x as i32, y as i32, 0, text_left, text_w, &st, 0).map(|h| (h, g, st))
	}

	fn edge(&self, g: &RulerGeometry) -> (i32, i32) {
		(
			(g.page_left + f64::from(g.margin_px) * g.zoom).round() as i32,
			(f64::from(g.text_px) * g.zoom).round() as i32,
		)
	}

	fn drag_begin(&self, x: f64, y: f64) {
		match self.hit(x, y) {
			Some((h @ (Hit::First | Hit::Left | Hit::Both | Hit::Right), _, st)) => {
				self.imp().drag.replace(Some(Drag {
					hit: h,
					state: st,
					start: st,
				}));
			}
			Some((Hit::Tab(pos), _, _)) => self.emit(RulerEdit::RemoveTab(pos)),
			Some((Hit::Scale(pos), _, _)) => {
				self.emit(RulerEdit::AddTab(snap(pos), self.kind()));
			}
			_ => {}
		}
	}

	fn drag_update(&self, x: f64) {
		let Some(g) = self.imp().geometry.get() else {
			return;
		};
		let (left, w) = self.edge(&g);
		if let Some(d) = self.imp().drag.borrow_mut().as_mut() {
			drag_to(d, x as i32, left, w);
		}
		self.queue_draw();
	}

	fn drag_end(&self, x: f64) {
		self.drag_update(x);
		let d = self.imp().drag.take();
		if let Some(d) = d {
			if d.state != d.start {
				self.emit(RulerEdit::Indents {
					left: d.state.left,
					first: d.state.first,
					right: d.state.right,
				});
			}
		}
		self.queue_draw();
	}

	fn emit(&self, e: RulerEdit) {
		let f = self.imp().on_edit.borrow().clone();
		if let Some(f) = f {
			f(e);
		}
	}
}

/// Applies a ruler edit through the shared editor calls.
pub fn apply(doc: &mut crate::doc::Document, e: RulerEdit) {
	doc.with(|app, s, _| match e {
		RulerEdit::Indents { left, first, right } => app.set_indents(s, left, first - left, right),
		RulerEdit::AddTab(pos, kind) => app.add_tab(s, pos, kind),
		RulerEdit::RemoveTab(pos) => app.remove_tab(s, pos),
	});
}

#[cfg(test)]
mod tests {
	use super::*;

	fn state() -> RulerState {
		let mut tabs = [(0, 0u8); 16];
		tabs[0] = (1440, 0);
		tabs[1] = (2880, 2);
		RulerState {
			left: 720,
			first: 360,
			right: 0,
			tabs,
			ntabs: 2,
		}
	}

	#[test]
	fn the_text_area_and_markers_follow_the_sheet_and_the_zoom() {
		let g = RulerGeometry {
			page_left: 40.0,
			zoom: 1.0,
			margin_px: 96,
			text_px: 624,
		};
		let d = layout(&g, &state());
		assert_eq!(d.text, (136.0, 760.0));
		// Twips are 15 to a pixel: the left indent at 720 twips is 48 px in, the first line at 360 is 24 px in.
		assert_eq!(d.marks, (136.0 + 24.0, 136.0 + 48.0, 760.0));
		assert_eq!(d.tabs, vec![(136.0 + 96.0, 0), (136.0 + 192.0, 2)]);
		let z = layout(
			&RulerGeometry {
				zoom: 2.0,
				page_left: 0.0,
				..g
			},
			&state(),
		);
		assert_eq!(z.text, (192.0, 192.0 + 1248.0));
		assert_eq!(z.marks.1, 192.0 + 96.0);
	}

	#[test]
	fn the_scale_has_a_tick_every_eighth_of_an_inch_and_numbers_each_inch() {
		let g = RulerGeometry {
			page_left: 0.0,
			zoom: 1.0,
			margin_px: 0,
			text_px: 96 * 6,
		};
		let d = layout(&g, &state());
		assert_eq!(d.ticks.len(), 6 * 8 + 1);
		let inches: Vec<i32> = d
			.ticks
			.iter()
			.filter(|t| t.kind == 0)
			.map(|t| t.inch)
			.collect();
		assert_eq!(inches, [0, 1, 2, 3, 4, 5, 6]);
		assert_eq!(d.ticks[1].x, 12.0);
	}
}
