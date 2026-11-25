// The window: the page, two scroll bars, a status line, the open dialog, the accelerators and drag and drop.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Everything that decides something lives in `quill_gtk` (the view model, the document, the settings); this file only connects
// GTK's events to it and shows the result.

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, gio, glib};

use quill_gtk::doc::Document;
use quill_gtk::settings::WindowSettings;
use quill_gtk::view::{page_label, zoom_label, View, Zoom, LINE_STEP};

use crate::page::QuillPage;

/// What the command line asks for beyond the files to open.
#[derive(Clone, Debug, Default)]
pub struct Opts {
	/// Draw at this scale in 1/120 whatever the display says (off-screen renders).
	pub scale_120: Option<u32>,
	/// Start at this zoom in percent.
	pub zoom: Option<u32>,
	/// Start scrolled to this document pixel.
	pub scroll: Option<i32>,
	/// The window's size in logical pixels.
	pub size: Option<(i32, i32)>,
	/// Render the window to this PNG once it has drawn, then quit.
	pub screenshot: Option<PathBuf>,
	/// Once it has drawn, write the page's texture to this PNG and print where it sits in the window's device pixels.
	pub dump: Option<PathBuf>,
	/// Quit after this many milliseconds.
	pub quit_after: Option<u32>,
	/// Use only the vendored fonts, so a render is the same on every machine.
	pub vendored_fonts: bool,
	/// Open the Open dialog at start.
	pub open_dialog: bool,
	/// Force a dark or a light desk.
	pub dark: Option<bool>,
	/// Do not read or write the window settings.
	pub no_settings: bool,
}

/// What an action does.
type Handler = Box<dyn Fn(&Rc<Ui>)>;

/// The window and what it shows.
pub struct Ui {
	window: gtk::ApplicationWindow,
	page: QuillPage,
	doc: Rc<RefCell<Document>>,
	view: Rc<RefCell<View>>,
	vadj: gtk::Adjustment,
	hadj: gtk::Adjustment,
	vbar: gtk::Scrollbar,
	hbar: gtk::Scrollbar,
	page_label: gtk::Label,
	zoom_label: gtk::Label,
	recent_menu: gio::Menu,
	recent_button: gtk::MenuButton,
	pointer: Cell<(f64, f64)>,
	zoom_acc: Cell<f64>,
	syncing: Cell<bool>,
	opts: Opts,
}

/// True when the system asks for a dark theme: GTK's colour scheme, else the older dark-theme setting (deprecated in GTK 4.20, but
/// still what older systems and `GTK_THEME` set).
#[allow(deprecated)]
pub fn system_dark() -> bool {
	let Some(s) = gtk::Settings::default() else {
		return false;
	};
	if s.find_property("gtk-interface-color-scheme").is_some() {
		let v = s.property_value("gtk-interface-color-scheme");
		if let Some((_, ev)) = glib::EnumValue::from_value(&v) {
			match ev.nick() {
				"dark" => return true,
				"light" => return false,
				_ => {}
			}
		}
	}
	s.is_gtk_application_prefer_dark_theme()
		|| s.gtk_theme_name()
			.is_some_and(|n| n.to_lowercase().ends_with("-dark"))
}

fn filters() -> (gio::ListStore, gtk::FileFilter) {
	let make = |name: &str, suffixes: &[&str]| {
		let f = gtk::FileFilter::new();
		f.set_name(Some(name));
		for s in suffixes {
			f.add_suffix(s);
		}
		f
	};
	let all_docs = make("All documents", &["qld", "rtf", "txt"]);
	let store = gio::ListStore::new::<gtk::FileFilter>();
	store.append(&all_docs);
	store.append(&make("Quill documents", &["qld"]));
	store.append(&make("Rich text", &["rtf"]));
	store.append(&make("Text files", &["txt"]));
	let everything = gtk::FileFilter::new();
	everything.set_name(Some("All files"));
	everything.add_pattern("*");
	store.append(&everything);
	(store, all_docs)
}

impl Ui {
	/// Builds the window for `app`.
	pub fn new(app: &gtk::Application, doc: Document, opts: Opts) -> Rc<Ui> {
		let doc = Rc::new(RefCell::new(doc));
		let zoom = opts
			.zoom
			.map(Zoom::Percent)
			.unwrap_or_else(|| doc.borrow().saved_zoom());
		let view = Rc::new(RefCell::new(View::new(zoom)));
		let settings = if opts.no_settings {
			WindowSettings::default()
		} else {
			WindowSettings::load(doc.borrow().config_dir())
		};
		let (w, h) = opts.size.unwrap_or((settings.width, settings.height));
		let window = gtk::ApplicationWindow::builder()
			.application(app)
			.title("Quill")
			.default_width(w)
			.default_height(h)
			.maximized(settings.maximized && opts.size.is_none())
			.build();

		let page = QuillPage::new(doc.clone(), view.clone());
		let vadj = gtk::Adjustment::new(0.0, 0.0, 1.0, f64::from(LINE_STEP), 100.0, 1.0);
		let hadj = gtk::Adjustment::new(0.0, 0.0, 1.0, f64::from(LINE_STEP), 100.0, 1.0);
		let vbar = gtk::Scrollbar::new(gtk::Orientation::Vertical, Some(&vadj));
		let hbar = gtk::Scrollbar::new(gtk::Orientation::Horizontal, Some(&hadj));
		let grid = gtk::Grid::new();
		grid.attach(&page, 0, 0, 1, 1);
		grid.attach(&vbar, 1, 0, 1, 1);
		grid.attach(&hbar, 0, 1, 1, 1);
		let page_label = gtk::Label::builder()
			.xalign(0.0)
			.hexpand(true)
			.margin_start(8)
			.build();
		let zoom_label = gtk::Label::builder().xalign(1.0).margin_end(8).build();
		let status = gtk::Box::new(gtk::Orientation::Horizontal, 8);
		status.set_margin_top(3);
		status.set_margin_bottom(3);
		status.add_css_class("quill-status");
		status.append(&page_label);
		status.append(&zoom_label);
		let column = gtk::Box::new(gtk::Orientation::Vertical, 0);
		column.append(&grid);
		column.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
		column.append(&status);
		window.set_child(Some(&column));

		let header = gtk::HeaderBar::new();
		let open_button = gtk::Button::with_label("Open");
		open_button.set_action_name(Some("win.open"));
		open_button.set_tooltip_text(Some("Open a document (Ctrl+O)"));
		let recent_menu = gio::Menu::new();
		let recent_button = gtk::MenuButton::builder()
			.label("Recent")
			.menu_model(&recent_menu)
			.build();
		header.pack_start(&open_button);
		header.pack_start(&recent_button);
		window.set_titlebar(Some(&header));

		let ui = Rc::new(Ui {
			window,
			page,
			doc,
			view,
			vadj,
			hadj,
			vbar,
			hbar,
			page_label,
			zoom_label,
			recent_menu,
			recent_button,
			pointer: Cell::new((0.0, 0.0)),
			zoom_acc: Cell::new(0.0),
			syncing: Cell::new(false),
			opts,
		});
		ui.wire(app);
		ui
	}

	fn wire(self: &Rc<Self>, app: &gtk::Application) {
		let ui = self;
		// The page's size and scale reach the view in the widget; the scroll bars follow.
		let weak = Rc::downgrade(ui);
		ui.page.connect_viewport_changed(move || {
			if let Some(u) = weak.upgrade() {
				u.refresh();
			}
		});
		ui.page.set_scale_override(ui.opts.scale_120);
		ui.set_theme();
		if let Some(settings) = gtk::Settings::default() {
			let weak = Rc::downgrade(ui);
			for prop in [
				"gtk-application-prefer-dark-theme",
				"gtk-theme-name",
				"gtk-interface-color-scheme",
			] {
				if settings.find_property(prop).is_none() {
					continue;
				}
				let weak = weak.clone();
				settings.connect_notify_local(Some(prop), move |_, _| {
					if let Some(u) = weak.upgrade() {
						u.set_theme();
					}
				});
			}
		}

		// Scroll bars drive the view.
		for adj in [&ui.vadj, &ui.hadj] {
			let weak = Rc::downgrade(ui);
			adj.connect_value_changed(move |_| {
				if let Some(u) = weak.upgrade() {
					if !u.syncing.get() {
						let (x, y) = (u.hadj.value() as i32, u.vadj.value() as i32);
						u.view.borrow_mut().scroll_to(x, y);
						u.changed();
					}
				}
			});
		}

		// Wheel, Ctrl+wheel and the pointer's place for zooming around it.
		let scroll = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::BOTH_AXES);
		let weak = Rc::downgrade(ui);
		scroll.connect_scroll(move |c, dx, dy| {
			let Some(u) = weak.upgrade() else {
				return glib::Propagation::Proceed;
			};
			let state = c.current_event_state();
			let wheel = c.unit() == gdk::ScrollUnit::Wheel;
			if state.contains(gdk::ModifierType::CONTROL_MASK) {
				u.wheel_zoom(dy, wheel);
			} else {
				let zoom = f64::from(u.view.borrow().effective_zoom());
				let unit = if wheel {
					f64::from(LINE_STEP)
				} else {
					100.0 / zoom
				};
				let (mut dx, mut dy) = (dx * unit, dy * unit);
				if state.contains(gdk::ModifierType::SHIFT_MASK) && dx == 0.0 {
					std::mem::swap(&mut dx, &mut dy);
				}
				u.view
					.borrow_mut()
					.scroll_by(dx.round() as i32, dy.round() as i32);
				u.changed();
			}
			glib::Propagation::Stop
		});
		ui.page.add_controller(scroll);
		let motion = gtk::EventControllerMotion::new();
		let weak = Rc::downgrade(ui);
		motion.connect_motion(move |_, x, y| {
			if let Some(u) = weak.upgrade() {
				u.pointer.set((x, y));
			}
		});
		ui.page.add_controller(motion);
		let click = gtk::GestureClick::new();
		let weak = Rc::downgrade(ui);
		click.connect_pressed(move |_, _, _, _| {
			if let Some(u) = weak.upgrade() {
				u.page.grab_focus();
			}
		});
		ui.page.add_controller(click);

		// Keys: scrolling. Zoom, open, quit and full screen are accelerators of actions below.
		let keys = gtk::EventControllerKey::new();
		let weak = Rc::downgrade(ui);
		keys.connect_key_pressed(move |_, key, _, state| {
			let Some(u) = weak.upgrade() else {
				return glib::Propagation::Proceed;
			};
			if state.intersects(gdk::ModifierType::ALT_MASK) {
				return glib::Propagation::Proceed;
			}
			let shift = state.contains(gdk::ModifierType::SHIFT_MASK);
			let step = LINE_STEP;
			let mut v = u.view.borrow_mut();
			let moved = match key {
				gdk::Key::Up => v.scroll_by(0, -step),
				gdk::Key::Down => v.scroll_by(0, step),
				gdk::Key::Left => v.scroll_by(-step, 0),
				gdk::Key::Right => v.scroll_by(step, 0),
				gdk::Key::Page_Up => v.page_by(-1),
				gdk::Key::Page_Down => v.page_by(1),
				gdk::Key::space if shift => v.page_by(-1),
				gdk::Key::space => v.page_by(1),
				gdk::Key::Home => v.to_edge(false),
				gdk::Key::End => v.to_edge(true),
				_ => return glib::Propagation::Proceed,
			};
			drop(v);
			if moved {
				u.changed();
			}
			glib::Propagation::Stop
		});
		ui.window.add_controller(keys);

		// Actions and accelerators.
		let act = |name: &str, f: Handler| {
			let a = gio::SimpleAction::new(name, None);
			let weak = Rc::downgrade(ui);
			a.connect_activate(move |_, _| {
				if let Some(u) = weak.upgrade() {
					f(&u);
				}
			});
			ui.window.add_action(&a);
		};
		act("open", Box::new(|u| u.open_dialog()));
		act("zoom-in", Box::new(|u| u.zoom(|v, a| v.zoom_in(a))));
		act("zoom-out", Box::new(|u| u.zoom(|v, a| v.zoom_out(a))));
		act("zoom-reset", Box::new(|u| u.zoom(|v, _| v.zoom_reset())));
		act(
			"zoom-width",
			Box::new(|u| u.zoom(|v, _| v.set_zoom(Zoom::Width, None))),
		);
		act(
			"fullscreen",
			Box::new(|u| u.window.set_fullscreened(!u.window.is_fullscreen())),
		);
		let recent = gio::SimpleAction::new("open-recent", Some(glib::VariantTy::UINT32));
		let weak = Rc::downgrade(ui);
		recent.connect_activate(move |_, p| {
			if let (Some(u), Some(i)) = (weak.upgrade(), p.and_then(|p| p.get::<u32>())) {
				let entry = u.doc.borrow().recent().get(i as usize).cloned();
				if let Some(r) = entry {
					u.open_path(&r.path);
				}
			}
		});
		ui.window.add_action(&recent);
		let quit = gio::SimpleAction::new("quit", None);
		let a2 = app.clone();
		let w2 = ui.window.clone();
		quit.connect_activate(move |_, _| {
			w2.close();
			a2.quit();
		});
		app.add_action(&quit);
		app.set_accels_for_action("win.open", &["<Ctrl>o"]);
		app.set_accels_for_action("app.quit", &["<Ctrl>q"]);
		app.set_accels_for_action("win.fullscreen", &["F11"]);
		app.set_accels_for_action(
			"win.zoom-in",
			&["<Ctrl>plus", "<Ctrl>equal", "<Ctrl>KP_Add"],
		);
		app.set_accels_for_action("win.zoom-out", &["<Ctrl>minus", "<Ctrl>KP_Subtract"]);
		app.set_accels_for_action("win.zoom-reset", &["<Ctrl>0", "<Ctrl>KP_0"]);
		app.set_accels_for_action("win.zoom-width", &["<Ctrl>9"]);

		// A file dropped on the window opens.
		let drop = gtk::DropTarget::new(glib::Type::INVALID, gdk::DragAction::COPY);
		drop.set_types(&[gdk::FileList::static_type(), gio::File::static_type()]);
		let weak = Rc::downgrade(ui);
		drop.connect_drop(move |_, value, _, _| {
			let Some(u) = weak.upgrade() else {
				return false;
			};
			let first = value
				.get::<gdk::FileList>()
				.ok()
				.and_then(|l| l.files().into_iter().next())
				.or_else(|| value.get::<gio::File>().ok());
			match first.and_then(|f| f.path()) {
				Some(p) => {
					u.open_path(&p);
					true
				}
				None => false,
			}
		});
		ui.window.add_controller(drop);

		// Remember the window when it closes.
		let weak = Rc::downgrade(ui);
		ui.window.connect_close_request(move |w| {
			if let Some(u) = weak.upgrade() {
				if !u.opts.no_settings {
					let (width, height) = w.default_size();
					let s = WindowSettings {
						width: width.max(quill_gtk::settings::SIZE_MIN),
						height: height.max(quill_gtk::settings::SIZE_MIN),
						maximized: w.is_maximized(),
					};
					let mut doc = u.doc.borrow_mut();
					s.save(doc.config_dir());
					doc.remember_zoom(u.view.borrow().zoom());
					doc.save_settings();
				}
			}
			glib::Propagation::Proceed
		});

		ui.rebuild_recent();
		ui.refresh();
	}

	/// Shows the window, opens `file` if one was named and applies the start-up options.
	pub fn present(self: &Rc<Self>, file: Option<&Path>) {
		self.window.present();
		self.page.grab_focus();
		if let Some(f) = file {
			self.open_path(f);
		}
		if let Some(y) = self.opts.scroll {
			self.view.borrow_mut().scroll_to(0, y);
			self.changed();
		}
		if self.opts.open_dialog {
			self.open_dialog();
		}
		if let Some(ms) = self.opts.quit_after {
			let w = self.window.clone();
			glib::timeout_add_local_once(
				std::time::Duration::from_millis(u64::from(ms)),
				move || {
					w.close();
				},
			);
		}
		if let Some(out) = self.opts.screenshot.clone() {
			self.page.set_reference(self.window.child().as_ref());
			let ui = self.clone();
			glib::timeout_add_local_once(std::time::Duration::from_millis(1500), move || {
				ui.dump(&out.with_extension("page.png"));
				ui.screenshot(&out);
				ui.window.close();
			});
		} else if let Some(out) = self.opts.dump.clone() {
			let ui = self.clone();
			glib::timeout_add_local_once(std::time::Duration::from_millis(1500), move || {
				ui.dump(&out);
			});
		}
	}

	/// Opens a document in this window.
	pub fn open_path(self: &Rc<Self>, path: &Path) {
		let result = self.doc.borrow_mut().open(path);
		match result {
			Ok(()) => {
				let geom = self.doc.borrow().geometry();
				self.view.borrow_mut().set_geometry(geom);
				self.rebuild_recent();
				self.changed();
			}
			Err(message) => {
				let dialog = gtk::AlertDialog::builder()
					.message("Quill cannot open the file")
					.detail(message)
					.buttons(["OK"])
					.build();
				dialog.show(Some(&self.window));
			}
		}
	}

	fn open_dialog(self: &Rc<Self>) {
		let (store, default) = filters();
		let dialog = gtk::FileDialog::builder()
			.title("Open")
			.modal(true)
			.filters(&store)
			.default_filter(&default)
			.build();
		let folder = self
			.doc
			.borrow()
			.path()
			.and_then(|p| p.parent().map(gio::File::for_path));
		if let Some(f) = folder {
			dialog.set_initial_folder(Some(&f));
		}
		let ui = self.clone();
		dialog.open(
			Some(&self.window),
			gio::Cancellable::NONE,
			move |res| match res {
				Ok(file) => {
					if let Some(path) = file.path() {
						ui.open_path(&path);
					}
				}
				// Closing the dialog without a choice is not a failure.
				Err(e)
					if e.matches(gtk::DialogError::Dismissed)
						|| e.matches(gtk::DialogError::Cancelled) => {}
				Err(e) => eprintln!("quill: the Open dialog failed: {e}"),
			},
		);
	}

	fn zoom(self: &Rc<Self>, f: impl Fn(&mut View, Option<(f64, f64)>)) {
		let (px, py) = self.pointer.get();
		let scale = f64::from(self.page.scale_120()) / 120.0;
		let inside = self.page.contains(px, py);
		f(
			&mut self.view.borrow_mut(),
			inside.then_some((px * scale, py * scale)),
		);
		self.changed();
	}

	/// Ctrl+wheel: one step a notch; a smooth wheel adds up to a step.
	fn wheel_zoom(self: &Rc<Self>, dy: f64, notch: bool) {
		let acc = if notch {
			dy.signum() * 8.0
		} else {
			self.zoom_acc.get() + dy
		};
		let steps = (acc / 8.0).trunc();
		self.zoom_acc
			.set(if notch { 0.0 } else { acc - steps * 8.0 });
		for _ in 0..steps.abs() as u32 {
			if steps < 0.0 {
				self.zoom(|v, a| v.zoom_in(a));
			} else {
				self.zoom(|v, a| v.zoom_out(a));
			}
		}
	}

	#[allow(deprecated)]
	fn set_theme(&self) {
		let dark = self.opts.dark.unwrap_or_else(system_dark);
		if self.opts.dark.is_some() {
			if let Some(s) = gtk::Settings::default() {
				s.set_gtk_application_prefer_dark_theme(dark);
			}
		}
		self.page.set_dark(dark);
	}

	fn rebuild_recent(&self) {
		self.recent_menu.remove_all();
		let recent = self.doc.borrow().recent();
		for (i, r) in recent.iter().enumerate() {
			let item = gio::MenuItem::new(Some(&r.label), None);
			item.set_action_and_target_value(
				Some("win.open-recent"),
				Some(&(i as u32).to_variant()),
			);
			self.recent_menu.append_item(&item);
		}
		self.recent_button.set_sensitive(!recent.is_empty());
	}

	/// The view moved or zoomed: redraw, and keep the zoom for the next session.
	fn changed(self: &Rc<Self>) {
		let zoom = self.view.borrow().zoom();
		self.doc.borrow_mut().remember_zoom(zoom);
		self.refresh();
		self.page.queue_draw();
	}

	/// Brings the scroll bars, the status line and the title up to date with the view.
	fn refresh(&self) {
		self.syncing.set(true);
		let v = self.view.borrow();
		for (adj, bar, horizontal) in [
			(&self.hadj, &self.hbar, true),
			(&self.vadj, &self.vbar, false),
		] {
			let r = v.range(horizontal);
			adj.configure(
				f64::from(r.value),
				0.0,
				f64::from(r.upper.max(r.page).max(1)),
				f64::from(LINE_STEP),
				f64::from((r.page - LINE_STEP).max(LINE_STEP)),
				f64::from(r.page.max(1)),
			);
			bar.set_visible(!horizontal || r.max > 0);
		}
		self.page_label
			.set_text(&page_label(v.current_page(), v.geometry().pages));
		self.zoom_label.set_text(&zoom_label(v.effective_zoom()));
		drop(v);
		self.syncing.set(false);
		self.window.set_title(Some(&self.doc.borrow().title()));
	}

	/// Writes the texture the page last drew and says where it sits, in device pixels from the top left of the surface.
	fn dump(&self, out: &Path) {
		match self.page.dump_texture(out) {
			Some(d) => println!(
				"page-texture {} origin {} {} size {} {} visible {} {} scale {}",
				out.display(),
				d.x,
				d.y,
				d.width,
				d.height,
				d.visible.0,
				d.visible.1,
				d.scale
			),
			None => eprintln!("quill: no texture to write"),
		}
	}

	/// Renders the window with GTK's own renderer at the page's scale and writes it as a PNG, bypassing any compositor.
	fn screenshot(&self, out: &Path) {
		let scale = f64::from(self.page.scale_120()) / 120.0;
		// The window's content, not the window: a render of the window is placed by its decoration, which the page cannot know.
		let Some(content) = self.window.child() else {
			return;
		};
		let paintable = gtk::WidgetPaintable::new(Some(&content));
		let (w, h) = (f64::from(content.width()), f64::from(content.height()));
		let snapshot = gtk::Snapshot::new();
		snapshot.scale(scale as f32, scale as f32);
		paintable.snapshot(&snapshot, w, h);
		let (Some(node), Some(renderer)) = (snapshot.to_node(), self.window.renderer()) else {
			eprintln!("quill: could not render the window");
			return;
		};
		let viewport = gtk::graphene::Rect::new(
			0.0,
			0.0,
			(w * scale).round() as f32,
			(h * scale).round() as f32,
		);
		let texture = renderer.render_texture(&node, Some(&viewport));
		match texture.save_to_png(out) {
			Ok(()) => println!(
				"screenshot {} {}x{} at scale {scale}",
				out.display(),
				texture.width(),
				texture.height()
			),
			Err(e) => eprintln!("quill: {}: {e}", out.display()),
		}
	}
}
