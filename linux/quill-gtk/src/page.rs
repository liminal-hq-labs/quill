// QuillPage: the custom page widget, with the texture strategies the spike compares.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::pattern::{self, PANEL_H, PANEL_W};
use gtk::glib;
use gtk::graphene;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use std::cell::{Cell, RefCell};

/// How the page hands its pixels to GTK.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Strategy {
	/// A texture of device size appended at logical bounds with `append_texture`.
	#[default]
	Texture,
	/// The same, through `append_scaled_texture` with `ScalingFilter::Nearest`.
	Nearest,
	/// The snapshot is scaled by 1/scale so the texture is appended in device units.
	Descale,
	/// `Texture`, after shifting the snapshot so the widget's top-left lands on a whole device pixel of the surface.
	Aligned,
	/// `Nearest` after the same shift as `Aligned`.
	NearestAligned,
	/// Control: a texture of logical size, which GTK must scale up by the display scale.
	Logical,
}

/// How far to shift drawing, in logical pixels, so the widget's top-left falls on a whole device pixel of the surface.
///
/// A widget's position is a whole number of logical pixels, which is a whole number of device pixels only when the scale is
/// whole or the position even. The surface itself can sit at a fractional device offset inside its window (client-side
/// shadows and borders count in logical pixels too), so the offset is measured from the surface, not the window.
pub fn snap_offset(widget: &gtk::Widget, scale: f64) -> (f64, f64) {
	let Some(native) = widget.native() else {
		return (0.0, 0.0);
	};
	let Some(p) = widget.compute_point(&native, &graphene::Point::zero()) else {
		return (0.0, 0.0);
	};
	let (sx, sy) = native.surface_transform();
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
		pub strategy: Cell<Strategy>,
		pub cache: RefCell<Option<(i64, gtk::gdk::MemoryTexture)>>,
		pub logged: Cell<bool>,
	}

	#[glib::object_subclass]
	impl ObjectSubclass for QuillPage {
		const NAME: &'static str = "QuillSpikePage";
		type Type = super::QuillPage;
		type ParentType = gtk::Widget;
	}

	impl ObjectImpl for QuillPage {}

	impl WidgetImpl for QuillPage {
		fn measure(&self, _orientation: gtk::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
			let size = if _orientation == gtk::Orientation::Horizontal {
				PANEL_W
			} else {
				PANEL_H
			};
			(size, size, -1, -1)
		}

		fn snapshot(&self, snapshot: &gtk::Snapshot) {
			let obj = self.obj();
			let (w, h) = (obj.width(), obj.height());
			let scale = obj
				.native()
				.and_then(|n| n.surface())
				.map(|s| s.scale())
				.unwrap_or(1.0);
			let strategy = self.strategy.get();
			// The control draws at scale 1 on purpose.
			let tex_scale = if strategy == Strategy::Logical {
				1.0
			} else {
				scale
			};
			let key = (tex_scale * 1000.0) as i64;
			let mut cache = self.cache.borrow_mut();
			if cache.as_ref().map(|(k, _)| *k) != Some(key) {
				*cache = Some((key, pattern::texture(&pattern::render(tex_scale))));
			}
			let tex = &cache.as_ref().unwrap().1;
			if !self.logged.replace(true) {
				println!(
					"page {:?}: surface_transform {:?}, widget {}x{} logical, surface scale {} (scale_factor {}), texture {}x{} (wanted {}x{})",
					strategy,
					obj.native().map(|n| n.surface_transform()),
					w,
					h,
					scale,
					obj.native().and_then(|n| n.surface()).map(|s| s.scale_factor()).unwrap_or(0),
					tex.width(),
					tex.height(),
					pattern::device(w, tex_scale),
					pattern::device(h, tex_scale)
				);
			}
			let logical = graphene::Rect::new(0.0, 0.0, w as f32, h as f32);
			match strategy {
				Strategy::Aligned => {
					let (dx, dy) = snap_offset(obj.upcast_ref(), scale);
					snapshot.save();
					snapshot.translate(&graphene::Point::new(dx as f32, dy as f32));
					snapshot.append_texture(tex, &logical);
					snapshot.restore();
				}
				Strategy::NearestAligned => {
					let (dx, dy) = snap_offset(obj.upcast_ref(), scale);
					snapshot.save();
					snapshot.translate(&graphene::Point::new(dx as f32, dy as f32));
					snapshot.append_scaled_texture(tex, gtk::gsk::ScalingFilter::Nearest, &logical);
					snapshot.restore();
				}
				Strategy::Texture | Strategy::Logical => snapshot.append_texture(tex, &logical),
				Strategy::Nearest => {
					snapshot.append_scaled_texture(tex, gtk::gsk::ScalingFilter::Nearest, &logical)
				}
				Strategy::Descale => {
					snapshot.save();
					snapshot.scale(1.0 / scale as f32, 1.0 / scale as f32);
					snapshot.append_texture(
						tex,
						&graphene::Rect::new(0.0, 0.0, tex.width() as f32, tex.height() as f32),
					);
					snapshot.restore();
				}
			}
		}
	}
}

glib::wrapper! {
	pub struct QuillPage(ObjectSubclass<imp::QuillPage>) @extends gtk::Widget, @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl QuillPage {
	pub fn new(strategy: Strategy) -> QuillPage {
		let page: QuillPage = glib::Object::new();
		page.imp().strategy.set(strategy);
		page.set_halign(gtk::Align::Start);
		page.set_valign(gtk::Align::Start);
		page
	}
}

mod paintable_imp {
	use super::*;
	use gtk::gdk;

	#[derive(Default)]
	pub struct PageTexture {
		pub texture: RefCell<Option<gdk::MemoryTexture>>,
	}

	#[glib::object_subclass]
	impl ObjectSubclass for PageTexture {
		const NAME: &'static str = "QuillSpikePageTexture";
		type Type = super::PageTexture;
		type Interfaces = (gdk::Paintable,);
	}

	impl ObjectImpl for PageTexture {}

	impl PaintableImpl for PageTexture {
		fn intrinsic_width(&self) -> i32 {
			PANEL_W
		}
		fn intrinsic_height(&self) -> i32 {
			PANEL_H
		}
		fn snapshot(&self, snapshot: &gdk::Snapshot, width: f64, height: f64) {
			let snapshot = snapshot.downcast_ref::<gtk::Snapshot>().unwrap();
			if let Some(tex) = self.texture.borrow().as_ref() {
				snapshot.append_texture(
					tex,
					&graphene::Rect::new(0.0, 0.0, width as f32, height as f32),
				);
			}
		}
	}
}

glib::wrapper! {
	/// A `gdk::Paintable` of logical size holding a device-size texture, for `Picture` and `Image`.
	pub struct PageTexture(ObjectSubclass<paintable_imp::PageTexture>) @implements gtk::gdk::Paintable;
}

impl PageTexture {
	pub fn new(texture: gtk::gdk::MemoryTexture) -> PageTexture {
		let p: PageTexture = glib::Object::new();
		p.imp().texture.replace(Some(texture));
		p
	}
}
