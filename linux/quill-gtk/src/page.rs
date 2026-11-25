// QuillPage: the widget that shows the sheets, as a texture of exactly device size put on the pixel grid.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The widget draws nothing itself. On each snapshot it asks the shared view for what is on screen, has the document paint that into
// an RGBA buffer of (widget size × display scale) device pixels, and appends it as a `gdk::MemoryTexture` after a shift that puts
// the widget's corner on a whole device pixel of the surface (see `docs/linux-spike.md`). The texture is kept until the document,
// the scroll, the zoom, the size, the scale or the theme changes, so a repaint with nothing new costs no layout and no drawing.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::gdk;
use gtk::glib;
use gtk::graphene;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

use quill_gtk::doc::{Document, Overlay, Theme};
use quill_gtk::view::{Mapping, View};

/// What decides whether the texture on hand is still right.
#[derive(Clone, PartialEq, Eq)]
pub struct Key {
	revision: u64,
	scroll: (i32, i32),
	zoom: u32,
	scale_120: u32,
	device: (u32, u32),
	dark: bool,
	overlay: Overlay,
}

/// Where the texture sits, as `QuillPage::dump_texture` reports it.
#[derive(Clone, Copy, Debug)]
pub struct Dump {
	/// The texture's left edge in device pixels from the surface (or the reference widget).
	pub x: f64,
	/// The top edge.
	pub y: f64,
	/// The texture's size in device pixels.
	pub width: i32,
	pub height: i32,
	/// How much of it the widget shows, in whole device pixels.
	pub visible: (i32, i32),
	/// The display scale.
	pub scale: f64,
}

/// How far to shift drawing, in logical pixels, so the widget's top left falls on a whole device pixel.
///
/// A widget sits at a whole number of logical pixels, which is a whole device pixel only at a whole scale or an even position,
/// and the surface itself can sit at a fractional device offset inside its window (a client-side shadow counts in logical pixels
/// too). So the offset is measured from the surface, through `Native::surface_transform`. With `reference` the position is
/// measured from that widget's own origin instead, which is where a `WidgetPaintable` of it puts the page when it is rendered
/// off screen.
pub fn snap_offset(
	widget: &gtk::Widget,
	scale: f64,
	reference: Option<&gtk::Widget>,
) -> (f64, f64) {
	let (target, (sx, sy)) = match reference {
		Some(r) => (r.clone(), (0.0, 0.0)),
		None => {
			let Some(native) = widget.native() else {
				return (0.0, 0.0);
			};
			let t = native.surface_transform();
			(native.upcast::<gtk::Widget>(), t)
		}
	};
	let Some(p) = widget.compute_point(&target, &graphene::Point::zero()) else {
		return (0.0, 0.0);
	};
	let snap = |logical: f64| {
		let device = logical * scale;
		(device.round() - device) / scale
	};
	(snap(f64::from(p.x()) + sx), snap(f64::from(p.y()) + sy))
}

mod imp {
	use super::*;

	#[derive(Default)]
	pub struct QuillPage {
		pub doc: RefCell<Option<Rc<RefCell<Document>>>>,
		pub view: RefCell<Option<Rc<RefCell<View>>>>,
		pub cache: RefCell<Option<(Key, gdk::MemoryTexture)>>,
		pub dark: Cell<bool>,
		pub overlay: RefCell<Overlay>,
		/// A scale to draw at whatever the surface says (for off-screen renders and tests).
		pub scale_override: Cell<Option<u32>>,
		/// The widget an off-screen render starts from, in place of the surface.
		pub reference: RefCell<Option<gtk::Widget>>,
		pub on_resize: RefCell<Option<Rc<dyn Fn()>>>,
		pub draws: Cell<u64>,
	}

	#[glib::object_subclass]
	impl ObjectSubclass for QuillPage {
		const NAME: &'static str = "QuillPage";
		type Type = super::QuillPage;
		type ParentType = gtk::Widget;
	}

	impl ObjectImpl for QuillPage {
		fn constructed(&self) {
			self.parent_constructed();
			let obj = self.obj();
			obj.set_hexpand(true);
			obj.set_vexpand(true);
			obj.set_focusable(true);
			obj.set_overflow(gtk::Overflow::Hidden);
			obj.add_css_class("quill-page");
		}
	}

	impl WidgetImpl for QuillPage {
		fn measure(&self, _o: gtk::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
			(160, 160, -1, -1)
		}

		fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
			self.parent_size_allocate(width, height, baseline);
			self.obj().sync_viewport();
		}

		fn realize(&self) {
			self.parent_realize();
			let obj = self.obj();
			if let Some(surface) = obj.native().and_then(|n| n.surface()) {
				let weak = obj.downgrade();
				surface.connect_notify_local(Some("scale"), move |_, _| {
					if let Some(page) = weak.upgrade() {
						page.sync_viewport();
						page.queue_draw();
					}
				});
			}
		}

		fn snapshot(&self, snapshot: &gtk::Snapshot) {
			let obj = self.obj();
			let (Some(doc), Some(view)) = (self.doc.borrow().clone(), self.view.borrow().clone())
			else {
				return;
			};
			obj.sync_viewport();
			let scale_120 = obj.scale_120();
			let (key, device) = {
				let v = view.borrow();
				let key = Key {
					revision: doc.borrow().revision(),
					scroll: v.scroll(),
					zoom: v.effective_zoom(),
					scale_120: v.scale_120(),
					device: v.device(),
					dark: self.dark.get(),
					overlay: self.overlay.borrow().clone(),
				};
				(key, v.device())
			};
			if device.0 == 0 || device.1 == 0 {
				return;
			}
			let mut cache = self.cache.borrow_mut();
			if cache.as_ref().map(|(k, _)| k) != Some(&key) {
				let buf = doc.borrow_mut().render(
					&view.borrow(),
					Theme::of(self.dark.get()),
					&key.overlay,
				);
				let bytes = glib::Bytes::from(buf.bytes());
				let tex = gdk::MemoryTexture::new(
					buf.width() as i32,
					buf.height() as i32,
					gdk::MemoryFormat::R8g8b8a8Premultiplied,
					&bytes,
					buf.stride(),
				);
				self.draws.set(self.draws.get() + 1);
				*cache = Some((key, tex));
			}
			let Some((_, tex)) = cache.as_ref() else {
				return;
			};
			// The texture is device-sized; its bounds are the same length in logical pixels, so it lands one to one.
			let scale = f64::from(scale_120) / 120.0;
			let (lw, lh) = (f64::from(device.0) / scale, f64::from(device.1) / scale);
			let (dx, dy) = snap_offset(obj.upcast_ref(), scale, self.reference.borrow().as_ref());
			snapshot.push_clip(&graphene::Rect::new(
				0.0,
				0.0,
				obj.width() as f32,
				obj.height() as f32,
			));
			snapshot.save();
			snapshot.translate(&graphene::Point::new(dx as f32, dy as f32));
			snapshot.append_texture(tex, &graphene::Rect::new(0.0, 0.0, lw as f32, lh as f32));
			snapshot.restore();
			snapshot.pop();
		}
	}
}

glib::wrapper! {
	/// The page view: the sheets of one document, one to one with the device's pixels.
	pub struct QuillPage(ObjectSubclass<imp::QuillPage>) @extends gtk::Widget, @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl QuillPage {
	/// A page view of a document and its view model.
	pub fn new(doc: Rc<RefCell<Document>>, view: Rc<RefCell<View>>) -> QuillPage {
		let page: QuillPage = glib::Object::new();
		page.imp().doc.replace(Some(doc));
		page.imp().view.replace(Some(view));
		page
	}

	/// Sets what is drawn over the sheets besides the text: the selection and caret, and any text being composed.
	pub fn set_overlay(&self, overlay: Overlay) {
		if *self.imp().overlay.borrow() != overlay {
			self.imp().overlay.replace(overlay);
			self.queue_draw();
		}
	}

	/// Draws on a dark or a light desk.
	pub fn set_dark(&self, dark: bool) {
		if self.imp().dark.replace(dark) != dark {
			self.queue_draw();
		}
	}

	/// Draws at this scale in 1/120 whatever the surface says: for rendering a window off screen at a scale the compositor does
	/// not offer.
	pub fn set_scale_override(&self, scale_120: Option<u32>) {
		self.imp().scale_override.set(scale_120);
		self.sync_viewport();
		self.queue_draw();
	}

	/// Measures the pixel shift from `widget`'s origin instead of the surface's: for a render of that widget through a
	/// `WidgetPaintable`.
	pub fn set_reference(&self, widget: Option<&gtk::Widget>) {
		self.imp().reference.replace(widget.cloned());
		self.queue_draw();
	}

	/// Called after the page's size or scale changed the view, so the scroll bars can follow.
	pub fn connect_viewport_changed(&self, f: impl Fn() + 'static) {
		self.imp().on_resize.replace(Some(Rc::new(f)));
	}

	/// How many times the texture has been drawn anew (a repaint that reuses it does not count).
	pub fn draw_count(&self) -> u64 {
		self.imp().draws.get()
	}

	/// Writes the texture last drawn to a PNG. Returns where it sits in device pixels from the surface's top left (the surface's own
	/// offset, such as a shadow, and the pixel shift included), its size and the scale.
	pub fn dump_texture(&self, out: &std::path::Path) -> Option<Dump> {
		let cache = self.imp().cache.borrow();
		let (_, tex) = cache.as_ref()?;
		tex.save_to_png(out).ok()?;
		let scale = f64::from(self.scale_120()) / 120.0;
		let native = self.native()?;
		let reference = self.imp().reference.borrow().clone();
		let target: gtk::Widget = reference.clone().unwrap_or_else(|| native.clone().upcast());
		let p = self.compute_point(&target, &graphene::Point::zero())?;
		let (sx, sy) = if reference.is_some() {
			(0.0, 0.0)
		} else {
			native.surface_transform()
		};
		let (dx, dy) = snap_offset(self.upcast_ref(), scale, reference.as_ref());
		let origin = |v: f32, s: f64, d: f64| (f64::from(v) + s + d) * scale;
		Some(Dump {
			x: origin(p.x(), sx, dx),
			y: origin(p.y(), sy, dy),
			width: tex.width(),
			height: tex.height(),
			visible: (
				(f64::from(self.width()) * scale).floor() as i32,
				(f64::from(self.height()) * scale).floor() as i32,
			),
			scale,
		})
	}

	/// The display scale in 1/120: the surface's, or the override.
	pub fn scale_120(&self) -> u32 {
		self.imp().scale_override.get().unwrap_or_else(|| {
			Mapping::scale_120_of(
				self.native()
					.and_then(|n| n.surface())
					.map_or(1.0, |s| s.scale()),
			)
		})
	}

	/// Gives the view the widget's device size and the scale now in force.
	pub fn sync_viewport(&self) {
		let Some(view) = self.imp().view.borrow().clone() else {
			return;
		};
		let scale_120 = self.scale_120();
		let (w, h) = (self.width(), self.height());
		let dev = Mapping::new(100, scale_120).device_size(w, h);
		let changed = {
			let v = view.borrow();
			v.device() != dev || v.scale_120() != scale_120
		};
		if changed {
			view.borrow_mut().set_viewport(dev.0, dev.1, scale_120);
			let f = self.imp().on_resize.borrow().clone();
			if let Some(f) = f {
				// Not from inside a snapshot or an allocation: the scroll bars change their own size.
				glib::idle_add_local_once(move || f());
			}
		}
	}
}
