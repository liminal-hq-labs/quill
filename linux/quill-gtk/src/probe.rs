// Probes that log what GTK reports: scale, dark mode, the input method, the file dialog.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![allow(deprecated)]

use gtk::gio;
use gtk::glib;
use gtk::prelude::*;

pub fn log_environment(window: &gtk::ApplicationWindow) {
	let display = gtk::gdk::Display::default().unwrap();
	println!("display: {} backend", display.type_().name());
	let monitors = display.monitors();
	for i in 0..monitors.n_items() {
		let m = monitors
			.item(i)
			.and_downcast::<gtk::gdk::Monitor>()
			.unwrap();
		let g = m.geometry();
		println!(
			"monitor {i}: {}x{} logical, scale {} (scale_factor {}), refresh {} mHz",
			g.width(),
			g.height(),
			m.scale(),
			m.scale_factor(),
			m.refresh_rate()
		);
	}
	if let Some(surface) = window.surface() {
		println!(
			"surface: scale {} scale_factor {} ({})",
			surface.scale(),
			surface.scale_factor(),
			surface.type_().name()
		);
		let r = window
			.renderer()
			.map(|r| r.type_().name().to_string())
			.unwrap_or_default();
		println!("renderer: {r}");
	}
	println!(
		"gtk {}.{}.{}",
		gtk::major_version(),
		gtk::minor_version(),
		gtk::micro_version()
	);
}

pub fn log_dark_mode() {
	let settings = gtk::Settings::default().unwrap();
	println!(
		"dark: gtk-application-prefer-dark-theme = {}",
		settings.is_gtk_application_prefer_dark_theme()
	);
	println!(
		"dark: gtk-theme-name = {}",
		settings.gtk_theme_name().unwrap_or_default()
	);
	println!(
		"dark: gtk-interface-color-scheme = {:?}",
		settings.property_value("gtk-interface-color-scheme")
	);
	println!("dark: gtk-im-module = {:?}", settings.gtk_im_module());
	let sm = adw::StyleManager::default();
	println!(
		"dark: adw colour scheme {:?}, system supports scheme {}, is_dark {}",
		sm.color_scheme(),
		sm.system_supports_color_schemes(),
		sm.is_dark()
	);
	sm.connect_dark_notify(|sm| println!("dark: changed, is_dark {}", sm.is_dark()));
}

/// A focusable area with an input-method context attached: logs key events and every commit and preedit signal.
pub fn ime_probe() -> gtk::Widget {
	let area = gtk::DrawingArea::new();
	area.set_content_width(192);
	area.set_content_height(24);
	area.set_focusable(true);
	area.set_can_focus(true);
	area.set_halign(gtk::Align::Start);
	area.set_draw_func(|a, cr, w, h| {
		let focus = a.has_focus();
		cr.set_source_rgb(if focus { 0.85 } else { 0.7 }, 0.9, 0.7);
		cr.rectangle(0.0, 0.0, f64::from(w), f64::from(h));
		let _ = cr.fill();
	});
	let im = gtk::IMMulticontext::new();
	im.set_client_widget(Some(&area));
	println!("ime: IMMulticontext context_id = {:?}", im.context_id());
	im.set_use_preedit(true);
	im.connect_commit(|_, s| println!("ime: commit {s:?}"));
	im.connect_preedit_start(|_| println!("ime: preedit-start"));
	im.connect_preedit_changed(|c| {
		let (text, _attrs, cursor) = c.preedit_string();
		println!("ime: preedit-changed {text:?} cursor {cursor}");
	});
	im.connect_preedit_end(|_| println!("ime: preedit-end"));
	im.connect_retrieve_surrounding(|c| {
		c.set_surrounding_with_selection("", 0, 0);
		true
	});
	im.connect_delete_surrounding(|_, o, n| {
		println!("ime: delete-surrounding {o} {n}");
		true
	});
	let keys = gtk::EventControllerKey::new();
	keys.set_im_context(Some(&im));
	keys.connect_key_pressed(|_, keyval, code, state| {
		println!(
			"ime: key-pressed {:?} code {code} state {state:?}",
			keyval.name()
		);
		glib::Propagation::Proceed
	});
	keys.connect_key_released(|_, keyval, _, _| println!("ime: key-released {:?}", keyval.name()));
	area.add_controller(keys);
	let click = gtk::GestureClick::new();
	let a2 = area.clone();
	click.connect_pressed(move |_, _, _, _| {
		a2.grab_focus();
	});
	area.add_controller(click);
	let focus = gtk::EventControllerFocus::new();
	let im_in = im.clone();
	focus.connect_enter(move |_| {
		println!("ime: focus-in");
		im_in.focus_in();
		im_in.set_cursor_location(&gtk::gdk::Rectangle::new(4, 4, 1, 16));
	});
	let im_out = im.clone();
	focus.connect_leave(move |_| {
		println!("ime: focus-out");
		im_out.focus_out();
	});
	area.add_controller(focus);
	area.upcast()
}

/// Opens the async file dialog and cancels it after `ms`; logs what the call returns.
pub fn file_dialog(window: &gtk::ApplicationWindow, ms: u32) {
	let dialog = gtk::FileDialog::builder()
		.title("Open a Quill document")
		.modal(true)
		.build();
	let cancel = gio::Cancellable::new();
	let c2 = cancel.clone();
	glib::timeout_add_local_once(std::time::Duration::from_millis(u64::from(ms)), move || {
		println!("filedialog: cancelling after the wait");
		c2.cancel();
	});
	println!(
		"filedialog: opening (GTK_USE_PORTAL={:?})",
		std::env::var("GTK_USE_PORTAL").ok()
	);
	dialog.open(Some(window), Some(&cancel), |res| match res {
		Ok(file) => println!("filedialog: returned {:?}", file.path()),
		Err(e) => println!(
			"filedialog: error {e} ({:?} {})",
			e.kind::<gtk::DialogError>(),
			e.domain().as_str()
		),
	});
}
