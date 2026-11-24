// Spike G0: one window showing the same pixels through each GTK4 drawing path, to compare them at a fractional scale.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod page;
mod pattern;
mod probe;

use gtk::glib;
use gtk::prelude::*;
use page::{PageTexture, QuillPage, Strategy};
use pattern::{PANEL_H, PANEL_W};
use std::path::PathBuf;

const BG: &str = "#123456";

struct Opts {
	out: PathBuf,
	quit_after: u32,
	ime: bool,
	file_dialog: bool,
	offsets: bool,
}

fn opts() -> Opts {
	let mut o = Opts {
		out: PathBuf::from("."),
		quit_after: 0,
		ime: false,
		file_dialog: false,
		offsets: false,
	};
	let mut args = std::env::args().skip(1);
	while let Some(a) = args.next() {
		match a.as_str() {
			"--out" => o.out = PathBuf::from(args.next().unwrap_or_default()),
			"--quit-after" => o.quit_after = args.next().and_then(|s| s.parse().ok()).unwrap_or(0),
			"--ime" => o.ime = true,
			"--file-dialog" => o.file_dialog = true,
			"--offsets" => o.offsets = true,
			_ => {}
		}
	}
	o
}

/// Panels whose paths need a widget other than `QuillPage`.
fn cairo_image(aligned: bool) -> gtk::Widget {
	let area = gtk::DrawingArea::new();
	area.set_content_width(PANEL_W);
	area.set_content_height(PANEL_H);
	area.set_halign(gtk::Align::Start);
	let logged = std::cell::Cell::new(false);
	area.set_draw_func(move |a, cr, w, h| {
		let scale = a.native().and_then(|n| n.surface()).map(|s| s.scale()).unwrap_or(1.0);
		let target = cr.target().device_scale();
		if !logged.replace(true) {
			println!("cairo-image: draw {w}x{h} logical, surface scale {scale}, cairo target device scale {target:?}");
		}
		let pm = pattern::render(scale);
		let (pw, ph) = (pm.width() as i32, pm.height() as i32);
		let mut img = gtk::cairo::ImageSurface::create(gtk::cairo::Format::ARgb32, pw, ph).unwrap();
		{
			let stride = img.stride() as usize;
			let mut data = img.data().unwrap();
			for (i, p) in pm.pixels().iter().enumerate() {
				let (x, y) = (i % pw as usize, i / pw as usize);
				let o = y * stride + x * 4;
				data[o..o + 4].copy_from_slice(&[p.blue(), p.green(), p.red(), p.alpha()]);
			}
		}
		img.set_device_scale(scale, scale);
		if aligned {
			let (dx, dy) = page::snap_offset(a.upcast_ref(), scale);
			cr.translate(dx, dy);
		}
		let _ = cr.set_source_surface(&img, 0.0, 0.0);
		let _ = cr.paint();
	});
	area.upcast()
}

/// Draws each source pixel as a cairo rectangle of one device pixel in logical units, with antialiasing off.
fn cairo_direct() -> gtk::Widget {
	let area = gtk::DrawingArea::new();
	area.set_content_width(PANEL_W);
	area.set_content_height(PANEL_H);
	area.set_halign(gtk::Align::Start);
	area.set_draw_func(|a, cr, _w, _h| {
		let scale = a
			.native()
			.and_then(|n| n.surface())
			.map(|s| s.scale())
			.unwrap_or(1.0);
		let pm = pattern::render(scale);
		let pw = pm.width() as usize;
		cr.set_antialias(gtk::cairo::Antialias::None);
		let s = 1.0 / scale;
		for (i, p) in pm.pixels().iter().enumerate() {
			let (x, y) = ((i % pw) as f64, (i / pw) as f64);
			cr.set_source_rgb(
				f64::from(p.red()) / 255.0,
				f64::from(p.green()) / 255.0,
				f64::from(p.blue()) / 255.0,
			);
			cr.rectangle(x * s, y * s, s, s);
			let _ = cr.fill();
		}
	});
	area.upcast()
}

fn picture() -> gtk::Widget {
	let scale = scale_guess();
	let pic =
		gtk::Picture::for_paintable(&PageTexture::new(pattern::texture(&pattern::render(scale))));
	pic.set_content_fit(gtk::ContentFit::Fill);
	pic.set_halign(gtk::Align::Start);
	pic.set_valign(gtk::Align::Start);
	pic.upcast()
}

/// The monitor's scale before a surface exists; `Picture` and `Image` hold a fixed paintable.
fn scale_guess() -> f64 {
	let display = gtk::gdk::Display::default().unwrap();
	display
		.monitors()
		.item(0)
		.and_downcast::<gtk::gdk::Monitor>()
		.map(|m| m.scale())
		.unwrap_or(1.0)
}

/// Renders the grid with the window's own renderer at the surface scale and saves it, bypassing the compositor.
fn self_render(
	window: &gtk::ApplicationWindow,
	grid: &gtk::Grid,
	scale: f64,
	out: &std::path::Path,
) {
	let paintable = gtk::WidgetPaintable::new(Some(grid));
	let (w, h) = (f64::from(grid.width()), f64::from(grid.height()));
	let snapshot = gtk::Snapshot::new();
	snapshot.scale(scale as f32, scale as f32);
	paintable.snapshot(&snapshot, w, h);
	let (Some(node), Some(renderer)) = (snapshot.to_node(), window.renderer()) else {
		return;
	};
	let viewport = gtk::graphene::Rect::new(
		0.0,
		0.0,
		(w * scale).round() as f32,
		(h * scale).round() as f32,
	);
	let texture = renderer.render_texture(&node, Some(&viewport));
	let _ = texture.save_to_png(out.join("self.png"));
}

fn activate(app: &gtk::Application, o: &Opts) {
	let window = gtk::ApplicationWindow::builder()
		.application(app)
		.title("Quill G0 spike")
		.build();
	window.set_resizable(false);
	let grid = gtk::Grid::builder()
		.row_spacing(8)
		.column_spacing(8)
		.margin_start(8)
		.margin_end(8)
		.margin_top(8)
		.margin_bottom(8)
		.build();
	grid.add_css_class("spike-grid");
	// (name, widget, extra margin left, extra margin top) in logical pixels.
	let panels: Vec<(String, gtk::Widget, i32, i32)> = if o.offsets {
		[
			(0, 0),
			(1, 0),
			(0, 1),
			(1, 1),
			(2, 0),
			(0, 2),
			(2, 2),
			(3, 3),
		]
		.into_iter()
		.map(|(x, y)| {
			(
				format!("texture-off{x}-{y}"),
				QuillPage::new(Strategy::Texture).upcast(),
				x,
				y,
			)
		})
		.collect()
	} else {
		let t = |n: &str, w: gtk::Widget, x: i32| (n.to_string(), w, x, 0);
		vec![
			t("texture", QuillPage::new(Strategy::Texture).upcast(), 0),
			t("aligned", QuillPage::new(Strategy::Aligned).upcast(), 0),
			t("nearest", QuillPage::new(Strategy::Nearest).upcast(), 0),
			t(
				"nearest-aligned",
				QuillPage::new(Strategy::NearestAligned).upcast(),
				0,
			),
			t("descale", QuillPage::new(Strategy::Descale).upcast(), 0),
			t("logical", QuillPage::new(Strategy::Logical).upcast(), 0),
			t("cairo-image", cairo_image(false), 0),
			t("cairo-aligned", cairo_image(true), 0),
			t("cairo-direct", cairo_direct(), 0),
			t("picture", picture(), 0),
			t("texture-odd", QuillPage::new(Strategy::Texture).upcast(), 3),
			t("aligned-odd", QuillPage::new(Strategy::Aligned).upcast(), 3),
		]
	};
	let mut named = Vec::new();
	for (i, (name, widget, mx, my)) in panels.into_iter().enumerate() {
		widget.set_margin_start(mx);
		widget.set_margin_top(my);
		// A panel with an odd margin spans both columns, so it cannot widen the first column.
		if mx % 2 == 1 {
			grid.attach(&widget, 0, 6 + (i as i32 - 10), 2, 1);
		} else {
			grid.attach(&widget, (i % 2) as i32, (i / 2) as i32, 1, 1);
		}
		named.push((name, widget));
	}
	if o.ime {
		let ime = probe::ime_probe();
		grid.attach(&ime, 0, 9, 2, 1);
		glib::timeout_add_local_once(std::time::Duration::from_millis(2000), move || {
			ime.grab_focus();
		});
	}
	let provider = gtk::CssProvider::new();
	provider.load_from_string(&format!(".spike-grid {{ background: {BG}; }}"));
	gtk::style_context_add_provider_for_display(
		&gtk::gdk::Display::default().unwrap(),
		&provider,
		gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
	);
	window.set_child(Some(&grid));
	probe::log_dark_mode();
	let out = o.out.clone();
	let (file_dialog, quit_after) = (o.file_dialog, o.quit_after);
	let w2 = window.clone();
	window.connect_map(move |_| {
		let w = w2.clone();
		let named = named.clone();
		let grid = grid.clone();
		let out = out.clone();
		glib::timeout_add_local_once(std::time::Duration::from_millis(1500), move || {
			probe::log_environment(&w);
			let scale = w.surface().map(|s| s.scale()).unwrap_or(1.0);
			println!(
				"layout scale {scale} grid {}x{}",
				grid.width(),
				grid.height()
			);
			for (name, widget) in &named {
				let b = widget.compute_bounds(&grid).unwrap();
				println!(
					"panel {name} {} {} {} {}",
					b.x(),
					b.y(),
					b.width(),
					b.height()
				);
				if let Some(o) = widget.compute_bounds(&w) {
					println!(
						"origin {name} {} {} (device {} {})",
						o.x(),
						o.y(),
						f64::from(o.x()) * scale,
						f64::from(o.y()) * scale
					);
				}
			}
			self_render(&w, &grid, scale, &out);
			let _ = pattern::render(scale).save_png(out.join("source.png"));
			let _ = pattern::render(1.0).save_png(out.join("source-logical.png"));
			if file_dialog {
				probe::file_dialog(&w, 4000);
			}
		});
	});
	window.connect_is_active_notify(|w| println!("window: is-active {}", w.is_active()));
	window.present();
	if quit_after > 0 {
		let app = app.clone();
		glib::timeout_add_local_once(
			std::time::Duration::from_millis(u64::from(quit_after)),
			move || app.quit(),
		);
	}
}

fn main() -> glib::ExitCode {
	let o = opts();
	adw::init().expect("libadwaita");
	let app = gtk::Application::builder()
		.application_id("io.github.liminal_hq_labs.QuillSpike")
		.flags(gtk::gio::ApplicationFlags::NON_UNIQUE)
		.build();
	app.connect_activate(move |app| activate(app, &o));
	app.run_with_args::<&str>(&[])
}
