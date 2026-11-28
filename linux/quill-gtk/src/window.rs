// The window: the header bar, the menu bar, the toolbar, the formatting bar, the ruler, the page, the status bar and the dialogs.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Everything that decides something lives in `quill_gtk` (the view model, the document, the settings, the chrome's specs); this
// file only connects GTK's events to it and shows the result. Every menu item, button and shortcut is a `Gio` action named
// `win.NAME` (`quill_gtk::actions`) that runs one shared `Command` through `Ui::run_command`.

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use adw::prelude::*;
use gtk::{gdk, gio, glib};
use quill_app::mouse::{Ev, DOUBLE_TICKS};
use quill_app::session::{end_session_allowed, ANSWER_DISCARD, ANSWER_SAVE};
use quill_app::{Command, Dialog, CHROME_FORMAT, CHROME_RULER, CHROME_STATUS, CHROME_TOOLBAR};

use quill_gtk::actions::{action_name, is_stateful, is_text_key};
use quill_gtk::blink::Blink;
use quill_gtk::choose::{
	complete_path, default_save_filter, AlertAsker, Asker, Chooser, Failure, GtkChooser, Reply,
	ScriptChooser,
};
use quill_gtk::dialogs::{self, DialogView, Env};
use quill_gtk::doc::{Document, Overlay};
use quill_gtk::format_bar::{apply as apply_format, FormatBar, FormatEdit, FormatState};
use quill_gtk::keys::{map_key, Mapped};
use quill_gtk::menu::{self, MENU_BAR_ACTION};
use quill_gtk::ruler::{apply as apply_ruler, QuillRuler, RulerEdit, RulerGeometry, KINDS};
use quill_gtk::settings::WindowSettings;
use quill_gtk::status::{Status, StatusInfo};
use quill_gtk::toolbar;
use quill_gtk::view::{View, Zoom, LINE_STEP};

mod selftest;

use crate::page::QuillPage;

/// The application id, and the name of the icon in the icon theme.
pub const APP_ID: &str = "io.github.liminal_hq_labs.Quill";

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
	/// Open the Save As dialog at start.
	pub save_as_dialog: bool,
	/// Show this dialog (`find`, `replace`, `fonts`, `tabs`, `spelling`, `about`) at start.
	pub dialog: Option<String>,
	/// Test hook: check the pointer's shape over a sheet and the desk and the scroll of a drag past the page's edge, report on
	/// standard output and quit.
	pub self_test_input: bool,
	/// Show every dialog, press every button that does not close it, report each on standard output and quit.
	pub self_test_dialogs: bool,
	/// Answer the file dialogs and the unsaved-changes question from this script (`quill_gtk::choose::parse_script`) and print
	/// what happens, one line each.
	pub script_chooser: Option<PathBuf>,
	/// Force a dark or a light desk.
	pub dark: Option<bool>,
	/// Do not read or write the window settings.
	pub no_settings: bool,
	/// Test hook: commit this text through the input method's `commit` signal once the window is up.
	pub commit: Option<String>,
	/// Test hook: show this text as being composed, as an input method's preedit.
	pub preedit: Option<String>,
}

/// Makes the application icon known to GTK: the installed theme has it, and a build run from the source tree finds it in
/// `linux/data/icons`. Also loads the few rules the chrome needs, in named colours only.
pub fn install_icons() {
	let Some(display) = gdk::Display::default() else {
		return;
	};
	let theme = gtk::IconTheme::for_display(&display);
	let dev = concat!(env!("CARGO_MANIFEST_DIR"), "/../data/icons");
	if Path::new(dev).is_dir() {
		theme.add_search_path(dev);
	}
	gtk::Window::set_default_icon_name(APP_ID);
	let css = gtk::CssProvider::new();
	css.load_from_string(
		".quill-toolbar, .quill-format-bar { padding: 3px 6px; }\n\
		 .quill-status { padding: 2px 0; }\n\
		 .quill-swatch { min-width: 20px; min-height: 20px; padding: 0; border-radius: 4px; border: 1px solid @borders; }\n\
		 .quill-ruler-kind { min-width: 22px; min-height: 22px; padding: 0; border-radius: 0; font-size: smaller; }\n",
	);
	gtk::style_context_add_provider_for_display(
		&display,
		&css,
		gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
	);
}

/// The window and what it shows.
pub struct Ui {
	app: adw::Application,
	window: adw::ApplicationWindow,
	toasts: adw::ToastOverlay,
	title: adw::WindowTitle,
	page: QuillPage,
	doc: Rc<RefCell<Document>>,
	view: Rc<RefCell<View>>,
	vadj: gtk::Adjustment,
	hadj: gtk::Adjustment,
	vbar: gtk::Scrollbar,
	hbar: gtk::Scrollbar,
	menu_bar: gtk::PopoverMenuBar,
	toolbar: gtk::Box,
	format: FormatBar,
	ruler_row: gtk::Overlay,
	ruler: QuillRuler,
	kind_button: gtk::Button,
	status: Status,
	actions: RefCell<Vec<(Command, gio::SimpleAction)>>,
	last_dialog: RefCell<Option<Rc<DialogView>>>,
	chooser: RefCell<Rc<dyn Chooser>>,
	asker: RefCell<Rc<dyn Asker>>,
	im: gtk::IMMulticontext,
	overlay: RefCell<Overlay>,
	blink: Cell<Blink>,
	clock: std::time::Instant,
	dragging: Cell<bool>,
	pointer: Cell<(f64, f64)>,
	zoom_acc: Cell<f64>,
	syncing: Cell<bool>,
	closing: Cell<bool>,
	closes: Cell<u32>,
	settings: RefCell<WindowSettings>,
	opts: Opts,
}

/// What to do once the unsaved-changes question is answered so that going on is allowed.
type Continue = Rc<dyn Fn(&Rc<Ui>)>;

/// What to do once a save has been tried: told whether it worked.
type Saved = Box<dyn FnOnce(&Rc<Ui>, bool)>;

/// What an action does.
type Handler = Box<dyn Fn(&Rc<Ui>)>;

/// The system's blink: `gtk-cursor-blink` and its time, with `QUILL_NO_BLINK` for tests.
fn system_blink() -> Blink {
	let (on, time) = gtk::Settings::default()
		.map(|s| (s.is_gtk_cursor_blink(), s.gtk_cursor_blink_time()))
		.unwrap_or((true, 1200));
	Blink::from_settings(on, time, std::env::var("QUILL_NO_BLINK").ok().as_deref())
}

/// The primary menu: what the buttons do not show.
fn primary_menu() -> gio::Menu {
	let menu = gio::Menu::new();
	let file = gio::Menu::new();
	file.append(Some("_New"), Some("win.new"));
	file.append(Some("_Open\u{2026}"), Some("win.open"));
	file.append(Some("Save _As\u{2026}"), Some("win.save-as"));
	menu.append_section(None, &file);
	let zoom = gio::Menu::new();
	for (label, z) in [
		("Page Width", 0),
		("150%", 150),
		("100%", 100),
		("75%", 75),
		("50%", 50),
	] {
		zoom.append(
			Some(label),
			Some(&format!("win.{}", action_name(Command::Zoom(z)))),
		);
	}
	let view = gio::Menu::new();
	view.append_submenu(Some("_Zoom"), &zoom);
	view.append(Some("_Menu Bar"), Some(&format!("win.{MENU_BAR_ACTION}")));
	view.append(Some("_Full Screen"), Some("win.fullscreen"));
	menu.append_section(None, &view);
	let about = gio::Menu::new();
	about.append(Some("_About Quill"), Some("win.about"));
	menu.append_section(None, &about);
	menu
}

impl Ui {
	/// Builds the window for `app`.
	pub fn new(app: &adw::Application, doc: Document, opts: Opts) -> Rc<Ui> {
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
		let window = adw::ApplicationWindow::builder()
			.application(app)
			.title("Quill")
			.icon_name(APP_ID)
			.default_width(w)
			.default_height(h)
			.maximized(settings.maximized && opts.size.is_none())
			.build();

		let page = QuillPage::new(doc.clone(), view.clone());
		let vadj = gtk::Adjustment::new(0.0, 0.0, 1.0, f64::from(LINE_STEP), 100.0, 1.0);
		let hadj = gtk::Adjustment::new(0.0, 0.0, 1.0, f64::from(LINE_STEP), 100.0, 1.0);
		let vbar = gtk::Scrollbar::new(gtk::Orientation::Vertical, Some(&vadj));
		let hbar = gtk::Scrollbar::new(gtk::Orientation::Horizontal, Some(&hadj));
		// The bars float over the page's edges and thin to an indicator until the pointer is near them, as a scrolled window's do.
		for (bar, halign, valign) in [
			(&vbar, gtk::Align::End, gtk::Align::Fill),
			(&hbar, gtk::Align::Fill, gtk::Align::End),
		] {
			bar.set_halign(halign);
			bar.set_valign(valign);
			bar.add_css_class("overlay-indicator");
			let motion = gtk::EventControllerMotion::new();
			let b = bar.clone();
			motion.connect_enter(move |_, _, _| b.add_css_class("hovering"));
			let b = bar.clone();
			motion.connect_leave(move |_| b.remove_css_class("hovering"));
			bar.add_controller(motion);
		}
		let content = gtk::Overlay::new();
		content.set_child(Some(&page));
		content.add_overlay(&vbar);
		content.add_overlay(&hbar);

		// The header bar.
		let title = adw::WindowTitle::new("Quill", "");
		let header = adw::HeaderBar::new();
		header.set_title_widget(Some(&title));
		let open_button = gtk::Button::from_icon_name("document-open-symbolic");
		open_button.set_action_name(Some("win.open"));
		open_button.set_tooltip_text(Some("Open a document (Ctrl+O)"));
		let save_button = gtk::Button::from_icon_name("document-save-symbolic");
		save_button.set_action_name(Some("win.save"));
		save_button.set_tooltip_text(Some("Save the document (Ctrl+S)"));
		header.pack_start(&open_button);
		header.pack_start(&save_button);
		header.pack_end(
			&gtk::MenuButton::builder()
				.icon_name("open-menu-symbolic")
				.tooltip_text("Main menu")
				.primary(true)
				.menu_model(&primary_menu())
				.build(),
		);

		// The bars under it, in the Windows build's order: menu bar, toolbar, formatting bar, ruler.
		let menu_bar = gtk::PopoverMenuBar::from_model(Some(&menu::model(&menu::tree(&[]))));
		let toolbar_box = toolbar::build();
		let families = doc.borrow().fonts().families();
		let on_format: Rc<dyn Fn(FormatEdit)> = Rc::new({
			let doc = doc.clone();
			let page = page.clone();
			move |e| {
				apply_format(&mut doc.borrow_mut(), &e);
				page.queue_draw();
				page.grab_focus();
			}
		});
		let format = FormatBar::build(families, on_format);
		let ruler = QuillRuler::new();
		let kind_button = gtk::Button::with_label("L");
		kind_button.add_css_class("flat");
		kind_button.add_css_class("quill-ruler-kind");
		kind_button.set_halign(gtk::Align::Start);
		kind_button.set_valign(gtk::Align::Fill);
		kind_button.set_tooltip_text(Some("Tab stop kind: Left (click to change)"));
		let ruler_row = gtk::Overlay::new();
		ruler_row.set_child(Some(&ruler));
		ruler_row.add_overlay(&kind_button);

		let status = Status::build();
		let view_stack = adw::ToolbarView::new();
		view_stack.set_top_bar_style(adw::ToolbarStyle::Flat);
		view_stack.set_bottom_bar_style(adw::ToolbarStyle::Flat);
		view_stack.add_top_bar(&header);
		view_stack.add_top_bar(&menu_bar);
		view_stack.add_top_bar(&toolbar_box);
		view_stack.add_top_bar(&format.widget);
		view_stack.add_top_bar(&ruler_row);
		view_stack.set_content(Some(&content));
		view_stack.add_bottom_bar(&status.widget);
		let toasts = adw::ToastOverlay::new();
		toasts.set_child(Some(&view_stack));
		window.set_content(Some(&toasts));

		let chooser: Rc<dyn Chooser> = Rc::new(GtkChooser::new(&window));
		let asker: Rc<dyn Asker> = Rc::new(AlertAsker::new(&window));
		let ui = Rc::new(Ui {
			app: app.clone(),
			window,
			toasts,
			title,
			page,
			doc,
			view,
			vadj,
			hadj,
			vbar,
			hbar,
			menu_bar,
			toolbar: toolbar_box,
			format,
			ruler_row,
			ruler,
			kind_button,
			status,
			actions: RefCell::new(Vec::new()),
			last_dialog: RefCell::new(None),
			chooser: RefCell::new(chooser),
			asker: RefCell::new(asker),
			im: gtk::IMMulticontext::new(),
			overlay: RefCell::new(Overlay {
				active: true,
				caret: true,
				..Overlay::default()
			}),
			blink: Cell::new(system_blink()),
			clock: std::time::Instant::now(),
			dragging: Cell::new(false),
			pointer: Cell::new((0.0, 0.0)),
			zoom_acc: Cell::new(0.0),
			syncing: Cell::new(false),
			closing: Cell::new(false),
			closes: Cell::new(0),
			settings: RefCell::new(settings),
			opts,
		});
		dialogs::set_trace(ui.tracing());
		if let Some(file) = ui.opts.script_chooser.clone() {
			match std::fs::read_to_string(&file)
				.map_err(|e| e.to_string())
				.and_then(|t| quill_gtk::choose::parse_script(&t))
			{
				Ok(steps) => ui.use_script(ScriptChooser::new(steps, true)),
				Err(e) => eprintln!("quill: {}: {e}", file.display()),
			}
		}
		ui.wire(app);
		ui
	}

	/// Closes the window for good: the editor said Exit, or the question allowed the close. Under `--self-test-dialogs` the window
	/// stays, so the next case can run, and the close is counted.
	fn close_window(&self) {
		self.closes.set(self.closes.get() + 1);
		if self.opts.self_test_dialogs {
			self.trace("window: closing");
			return;
		}
		self.closing.set(true);
		self.window.close();
	}

	/// Answers the file dialogs and the unsaved-changes question from `script` from now on.
	fn use_script(&self, script: Rc<ScriptChooser>) {
		*self.chooser.borrow_mut() = script.clone();
		*self.asker.borrow_mut() = script;
	}

	/// True when the test hooks ask for a line on standard output for each dialog.
	fn tracing(&self) -> bool {
		self.opts.script_chooser.is_some() || self.opts.self_test_dialogs
	}

	/// Prints a line of the trace.
	fn trace(&self, line: &str) {
		if self.tracing() {
			println!("{line}");
		}
	}

	/// The Gio action for a command, made once: stateful (a check mark) when the item can have one.
	fn add_command_action(self: &Rc<Self>, c: Command) {
		let name = action_name(c);
		let weak = Rc::downgrade(self);
		let action = if is_stateful(c) {
			let a = gio::SimpleAction::new_stateful(&name, None, &false.to_variant());
			// The state is the editor's to say (`sync_actions`), not the toggle's.
			a.connect_change_state(move |_, _| {
				if let Some(u) = weak.upgrade() {
					u.run_command(c);
				}
			});
			a
		} else {
			let a = gio::SimpleAction::new(&name, None);
			a.connect_activate(move |_, _| {
				if let Some(u) = weak.upgrade() {
					u.run_command(c);
				}
			});
			a
		};
		self.window.add_action(&action);
		self.actions.borrow_mut().push((c, action));
	}

	fn wire(self: &Rc<Self>, app: &adw::Application) {
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
		let weak = Rc::downgrade(ui);
		adw::StyleManager::default().connect_dark_notify(move |_| {
			if let Some(u) = weak.upgrade() {
				u.set_theme();
			}
		});

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
		ui.wire_editing();

		// One action for every command of the Windows menu tree, the toolbar and the formatting bar.
		let tree = menu::tree(&[]);
		let mut commands = menu::commands(&tree);
		commands.extend((0..4).map(Command::Recent));
		// The status bar's list has every level of the shared list, which the View menu does not.
		commands.dedup();
		for level in quill_chrome::zoom_field::ZOOM_LEVELS {
			if !commands.contains(&Command::Zoom(level)) {
				commands.push(Command::Zoom(level));
			}
		}
		for c in commands {
			ui.add_command_action(c);
		}
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
		act("zoom-in", Box::new(|u| u.zoom(|v, a| v.zoom_in(a))));
		act("zoom-out", Box::new(|u| u.zoom(|v, a| v.zoom_out(a))));
		act("zoom-reset", Box::new(|u| u.zoom(|v, _| v.zoom_reset())));
		// The status bar's minus and plus buttons: the next level of the shared list, from the zoom drawn now.
		act("zoom-step-down", Box::new(|u| u.zoom_step(false)));
		act("zoom-step-up", Box::new(|u| u.zoom_step(true)));
		act(
			"zoom-width",
			Box::new(|u| u.zoom(|v, _| v.set_zoom(Zoom::Width, None))),
		);
		act(
			"fullscreen",
			Box::new(|u| u.window.set_fullscreened(!u.window.is_fullscreen())),
		);
		let bar = gio::SimpleAction::new_stateful(
			MENU_BAR_ACTION,
			None,
			&ui.settings.borrow().menu_bar.to_variant(),
		);
		let weak = Rc::downgrade(ui);
		bar.connect_change_state(move |a, _| {
			if let Some(u) = weak.upgrade() {
				let on = !u.settings.borrow().menu_bar;
				u.settings.borrow_mut().menu_bar = on;
				a.set_state(&on.to_variant());
				u.apply_chrome();
			}
		});
		ui.window.add_action(&bar);

		// Accelerators: the menu tree's own, then the few this client adds.
		for (action, accel) in menu::accelerators(&tree) {
			app.set_accels_for_action(&format!("win.{action}"), &[&accel]);
		}
		app.set_accels_for_action("win.fullscreen", &["F11"]);
		app.set_accels_for_action("win.zoom-in", &["<Ctrl>KP_Add"]);
		app.set_accels_for_action("win.zoom-out", &["<Ctrl>minus", "<Ctrl>KP_Subtract"]);
		app.set_accels_for_action("win.zoom-reset", &["<Ctrl>0", "<Ctrl>KP_0"]);
		app.set_accels_for_action("win.zoom-width", &["<Ctrl>9"]);
		app.set_accels_for_action("win.save-as", &["<Ctrl><Shift>s"]);
		app.set_accels_for_action(&format!("win.{MENU_BAR_ACTION}"), &["<Ctrl><Shift>m"]);
		// The editing keys belong to a text field while one has the focus.
		let weak = Rc::downgrade(ui);
		ui.window.connect_focus_widget_notify(move |_| {
			if let Some(u) = weak.upgrade() {
				u.set_text_accels();
			}
		});

		// The ruler, its tab-kind button, and the page's own events.
		let weak = Rc::downgrade(ui);
		ui.ruler.connect_edit(move |e: RulerEdit| {
			if let Some(u) = weak.upgrade() {
				apply_ruler(&mut u.doc.borrow_mut(), e);
				u.changed();
			}
		});
		let weak = Rc::downgrade(ui);
		ui.kind_button.connect_clicked(move |b| {
			if let Some(u) = weak.upgrade() {
				let k = (u.ruler.kind() + 1) % 4;
				u.ruler.set_kind(k);
				let glyph = ["L", "C", "R", "D"][usize::from(k)];
				b.set_label(glyph);
				b.set_tooltip_text(Some(&format!(
					"Tab stop kind: {} (click to change)",
					KINDS[usize::from(k)]
				)));
			}
		});

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

		// Closing asks about unsaved changes, then remembers the window.
		let weak = Rc::downgrade(ui);
		ui.window.connect_close_request(move |w| {
			let Some(u) = weak.upgrade() else {
				return glib::Propagation::Proceed;
			};
			if u.doc.borrow().unsaved_work() && !u.closing.get() {
				let cont: Continue = Rc::new(|u| u.close_window());
				u.confirm_unsaved(cont);
				return glib::Propagation::Stop;
			}
			if !u.opts.no_settings {
				let (width, height) = w.default_size();
				let menu_bar = u.settings.borrow().menu_bar;
				let s = WindowSettings {
					width: width.max(quill_gtk::settings::SIZE_MIN),
					height: height.max(quill_gtk::settings::SIZE_MIN),
					maximized: w.is_maximized(),
					menu_bar,
				};
				let mut doc = u.doc.borrow_mut();
				s.save(doc.config_dir());
				doc.remember_zoom(u.view.borrow().zoom());
				doc.save_settings();
			}
			glib::Propagation::Proceed
		});

		ui.rebuild_menus();
		ui.apply_chrome();
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
			self.open_dialog(false);
		}
		if self.opts.save_as_dialog {
			let ui = self.clone();
			self.save_as_dialog(Box::new(move |_, _| ui.changed()));
		}
		if let Some(name) = self.opts.dialog.clone() {
			self.show_dialog_named(&name);
		}
		if self.opts.self_test_input {
			let ui = self.clone();
			glib::timeout_add_local_once(std::time::Duration::from_millis(800), move || {
				ui.self_test_input();
			});
		}
		if self.opts.self_test_dialogs {
			let ui = self.clone();
			glib::timeout_add_local_once(std::time::Duration::from_millis(500), move || {
				ui.self_test_dialogs();
			});
		}
		if let Some(text) = self.opts.commit.clone() {
			let ui = self.clone();
			glib::timeout_add_local_once(std::time::Duration::from_millis(600), move || {
				ui.im.emit_by_name::<()>("commit", &[&text]);
			});
		}
		if let Some(text) = self.opts.preedit.clone() {
			let ui = self.clone();
			glib::timeout_add_local_once(std::time::Duration::from_millis(900), move || {
				let n = text.len();
				ui.set_preedit(&text, n);
			});
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
			self.page.set_reference(self.window.content().as_ref());
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

	/// The milliseconds since the window was made: the clock the blink and the double click run on.
	fn now_ms(&self) -> u64 {
		self.clock.elapsed().as_millis() as u64
	}

	/// The keyboard, the input method, the pointer and the caret's blink.
	fn wire_editing(self: &Rc<Self>) {
		let ui = self;
		let page = &ui.page;
		let im = &ui.im;
		im.set_client_widget(Some(page));
		im.set_use_preedit(true);

		// Keys go to the input method first (a printable key is committed as text), then to the editor.
		let keys = gtk::EventControllerKey::new();
		keys.set_im_context(Some(im));
		let weak = Rc::downgrade(ui);
		// GTK 4 gives a held key's auto-repeat as more key-pressed events with no flag, so a key is a repeat when its hardware code is
		// pressed again before it was released: a toggle (Ctrl+I) must act once for the press, not on and off for as long as it is held.
		let held = Rc::new(std::cell::Cell::new(None::<u32>));
		let released = held.clone();
		keys.connect_key_pressed(move |_, key, code, state| {
			let repeat = held.replace(Some(code)) == Some(code);
			match weak.upgrade() {
				Some(u) if u.on_key(key, state, repeat) => glib::Propagation::Stop,
				_ => glib::Propagation::Proceed,
			}
		});
		keys.connect_key_released(move |_, _, code, _| {
			if released.get() == Some(code) {
				released.set(None);
			}
		});
		page.add_controller(keys);

		let focus = gtk::EventControllerFocus::new();
		let i = im.clone();
		focus.connect_enter(move |_| i.focus_in());
		let i = im.clone();
		focus.connect_leave(move |_| i.focus_out());
		page.add_controller(focus);

		let weak = Rc::downgrade(ui);
		im.connect_commit(move |_, text| {
			if let Some(u) = weak.upgrade() {
				u.commit(text);
			}
		});
		let weak = Rc::downgrade(ui);
		im.connect_preedit_changed(move |im| {
			if let Some(u) = weak.upgrade() {
				let (text, _, cursor) = im.preedit_string();
				u.set_preedit(&text, cursor.max(0) as usize);
			}
		});
		let weak = Rc::downgrade(ui);
		im.connect_preedit_end(move |_| {
			if let Some(u) = weak.upgrade() {
				u.set_preedit("", 0);
			}
		});
		let weak = Rc::downgrade(ui);
		im.connect_retrieve_surrounding(move |im| {
			let Some(u) = weak.upgrade() else {
				return false;
			};
			let (text, cursor) = u.doc.borrow().surrounding();
			im.set_surrounding_with_selection(&text, cursor as i32, cursor as i32);
			true
		});
		let weak = Rc::downgrade(ui);
		im.connect_delete_surrounding(move |_, offset, n| {
			let Some(u) = weak.upgrade() else {
				return false;
			};
			let ok = u
				.doc
				.borrow_mut()
				.delete_surrounding(offset, n.max(0) as u32);
			u.after_edit();
			ok
		});

		// The pointer: a click places the caret, a drag selects, Shift extends, a double and a triple click take a word and a
		// paragraph. The editor's own pointer layer decides; this turns GTK's events into its `Ev`s in device pixels.
		let click = gtk::GestureClick::builder().button(1).build();
		let weak = Rc::downgrade(ui);
		click.connect_pressed(move |g, _, x, y| {
			if let Some(u) = weak.upgrade() {
				u.page.grab_focus();
				u.dragging.set(true);
				u.pointer_event(g.current_event_state(), g.current_event_time(), x, y, 0);
				u.start_autoscroll();
			}
		});
		let weak = Rc::downgrade(ui);
		click.connect_released(move |g, _, x, y| {
			if let Some(u) = weak.upgrade() {
				u.pointer_event(g.current_event_state(), g.current_event_time(), x, y, 2);
				u.dragging.set(false);
			}
		});
		page.add_controller(click);
		let motion = gtk::EventControllerMotion::new();
		let weak = Rc::downgrade(ui);
		motion.connect_motion(move |c, x, y| {
			let Some(u) = weak.upgrade() else {
				return;
			};
			u.pointer.set((x, y));
			if u.dragging.get() {
				u.pointer_event(c.current_event_state(), c.current_event_time(), x, y, 1);
			} else {
				u.update_cursor(x, y);
			}
		});
		page.add_controller(motion);

		// The caret blinks, solid while typing and while the window is inactive.
		let weak = Rc::downgrade(ui);
		ui.window.connect_is_active_notify(move |w| {
			if let Some(u) = weak.upgrade() {
				let now = u.now_ms();
				let mut b = u.blink.get();
				b.set_active(w.is_active(), now);
				u.blink.set(b);
				u.sync_overlay();
			}
		});
		let step = ui.blink.get().step_ms().max(100);
		let weak = Rc::downgrade(ui);
		glib::timeout_add_local(std::time::Duration::from_millis(step), move || {
			match weak.upgrade() {
				Some(u) => {
					u.sync_overlay();
					glib::ControlFlow::Continue
				}
				None => glib::ControlFlow::Break,
			}
		});
	}

	/// A key the page was given and the input method did not take. True when it was used.
	fn on_key(self: &Rc<Self>, key: gdk::Key, state: gdk::ModifierType, repeat: bool) -> bool {
		let mapped = map_key(key, state);
		if mapped == Mapped::Ignore {
			return false;
		}
		// A repeat of a held key acts only if the editor says that key repeats (text and movement do; a toggle does not).
		if let (true, Mapped::Edit(k)) = (repeat, mapped) {
			if !self.doc.borrow().key_repeats(k) {
				return true;
			}
		}
		let out = self.doc.borrow_mut().apply(mapped);
		if let Some(c) = out.typed.refused.last() {
			self.toast(&quill_gtk::doc::refusal(*c));
		}
		if let Some(d) = out.dialog {
			self.handle_dialog(d);
		}
		if self.doc.borrow_mut().with(|app, _, _| app.take_help()) {
			self.toast("Help is not in the Linux client yet.");
		}
		self.after_edit();
		true
	}

	/// Text the input method committed: typed into the document, refusing what Windows-1252 cannot hold.
	fn commit(self: &Rc<Self>, text: &str) {
		let typed = self.doc.borrow_mut().type_text(text);
		if let Some(c) = typed.refused.last() {
			self.toast(&quill_gtk::doc::refusal(*c));
		}
		self.set_preedit("", 0);
		self.after_edit();
	}

	/// Shows (or clears) the text an input method is composing, under the caret.
	fn set_preedit(&self, text: &str, cursor_byte: usize) {
		{
			let mut o = self.overlay.borrow_mut();
			o.preedit = quill_gtk::names::lossy_ansi(text, 63);
			let before = text.get(..cursor_byte.min(text.len())).unwrap_or(text);
			o.preedit_cursor = before.chars().count().min(o.preedit.len());
		}
		self.sync_overlay();
	}

	/// Puts the overlay (the caret's phase, the composing text) on the page.
	fn sync_overlay(&self) {
		let now = self.now_ms();
		{
			let mut o = self.overlay.borrow_mut();
			o.active = true;
			o.caret = self.blink.get().on(now);
		}
		self.page.set_overlay(self.overlay.borrow().clone());
	}

	/// After an edit or a caret move: the caret shows and stays solid, the view follows it, and the bars follow the format.
	fn after_edit(self: &Rc<Self>) {
		let now = self.now_ms();
		let mut b = self.blink.get();
		b.input(now);
		self.blink.set(b);
		self.doc.borrow().reveal_caret(&mut self.view.borrow_mut());
		self.changed();
	}

	/// Tells the input method where the caret is, in the page's own coordinates, so its candidate window sits beside it.
	fn update_im(&self) {
		let (doc, view) = (self.doc.borrow(), self.view.borrow());
		let Some(r) = doc.caret_rect() else {
			return;
		};
		let zoom = f64::from(view.effective_zoom()) / 100.0;
		let x = f64::from(view.page_left() + r.view_x) * zoom;
		let y = f64::from(r.content_y - view.scroll().1) * zoom;
		self.im.set_cursor_location(&gdk::Rectangle::new(
			x.round() as i32,
			y.round() as i32,
			2,
			(f64::from(r.h) * zoom).round().max(1.0) as i32,
		));
	}

	/// One pointer event from the page, in its logical pixels. `kind` is 0 for the button going down, 1 for a move and 2 for it
	/// going up. While the button is down a point outside the page is brought to its edge, and the page scrolls.
	fn pointer_event(
		self: &Rc<Self>,
		state: gdk::ModifierType,
		time_ms: u32,
		x: f64,
		y: f64,
		kind: u8,
	) {
		let scale = f64::from(self.page.scale_120()) / 120.0;
		let (dw, dh) = self.view.borrow().device();
		let (mut dx, mut dy) = ((x * scale).round() as i32, (y * scale).round() as i32);
		if kind != 0 {
			if dy < 0 {
				self.view.borrow_mut().scroll_by(0, -LINE_STEP / 2);
			} else if dy >= dh as i32 {
				self.view.borrow_mut().scroll_by(0, LINE_STEP / 2);
			}
			dx = dx.clamp(0, dw as i32 - 1);
			dy = dy.clamp(0, dh as i32 - 1);
		}
		let ev = match kind {
			0 => Ev::Down { x: dx, y: dy },
			1 => Ev::Move { x: dx, y: dy },
			_ => Ev::Up { x: dx, y: dy },
		};
		let shift = state.contains(gdk::ModifierType::SHIFT_MASK);
		let click_ms = gtk::Settings::default()
			.map_or(400, |s| s.gtk_double_click_time())
			.max(100) as u64;
		let tick = (u64::from(time_ms) * u64::from(DOUBLE_TICKS) / click_ms) as u32;
		self.doc
			.borrow_mut()
			.pointer(&self.view.borrow(), ev, shift, tick);
		self.after_edit();
	}

	/// While the button is down the page scrolls a little on a timer when the pointer is past its edge, so a drag can go on.
	fn start_autoscroll(self: &Rc<Self>) {
		let weak = Rc::downgrade(self);
		glib::timeout_add_local(std::time::Duration::from_millis(60), move || {
			let Some(u) = weak.upgrade() else {
				return glib::ControlFlow::Break;
			};
			if !u.dragging.get() {
				return glib::ControlFlow::Break;
			}
			let (x, y) = u.pointer.get();
			let (_, dh) = u.view.borrow().device();
			let scale = f64::from(u.page.scale_120()) / 120.0;
			if y < 0.0 || y * scale >= f64::from(dh) {
				u.pointer_event(gdk::ModifierType::empty(), 0, x, y, 1);
			}
			glib::ControlFlow::Continue
		});
	}

	/// The I-beam over a sheet and the arrow elsewhere.
	fn update_cursor(&self, x: f64, y: f64) {
		let scale = f64::from(self.page.scale_120()) / 120.0;
		let over = self.doc.borrow().over_sheet(
			&self.view.borrow(),
			(x * scale).round() as i32,
			(y * scale).round() as i32,
		);
		self.page
			.set_cursor_from_name(Some(if over { "text" } else { "default" }));
	}

	fn page_has_focus(&self) -> bool {
		match gtk::prelude::GtkWindowExt::focus(&self.window) {
			None => true,
			Some(w) => w == *self.page.upcast_ref::<gtk::Widget>(),
		}
	}

	/// Hands Ctrl+C, Ctrl+V and the other editing accelerators to a text field that has the focus, and takes them back after.
	fn set_text_accels(&self) {
		let on = self.page_has_focus();
		let tree = menu::tree(&[]);
		for (action, accel) in menu::accelerators(&tree) {
			let is_text = quill_gtk::actions::parse_action(&action).is_some_and(is_text_key);
			if is_text {
				let keys: &[&str] = if on { &[accel.as_str()] } else { &[] };
				self.app
					.set_accels_for_action(&format!("win.{action}"), keys);
			}
		}
	}

	/// Shows the toast `text`.
	fn toast(&self, text: &str) {
		self.toasts.add_toast(adw::Toast::new(text));
	}

	/// Remakes the menu bar (the recent files are in File) and the actions' state.
	fn rebuild_menus(&self) {
		let recents: Vec<String> = self
			.doc
			.borrow()
			.recent()
			.into_iter()
			.map(|r| r.label)
			.collect();
		self.menu_bar
			.set_menu_model(Some(&menu::model(&menu::tree(&recents))));
		for i in 0..4u8 {
			let c = Command::Recent(i);
			if let Some((_, a)) = self.actions.borrow().iter().find(|(k, _)| *k == c) {
				a.set_enabled((i as usize) < recents.len());
			}
		}
	}

	/// Shows or hides each bar as the editor's settings and the menu bar's own say.
	fn apply_chrome(&self) {
		let d = self.doc.borrow();
		let (tb, fb, rl, st) = d.read(|app, _| {
			(
				app.chrome_on(CHROME_TOOLBAR),
				app.chrome_on(CHROME_FORMAT),
				app.chrome_on(CHROME_RULER),
				app.chrome_on(CHROME_STATUS),
			)
		});
		drop(d);
		self.menu_bar.set_visible(self.settings.borrow().menu_bar);
		self.toolbar.set_visible(tb);
		self.format.widget.set_visible(fb);
		self.ruler_row.set_visible(rl);
		self.status.widget.set_visible(st);
	}

	/// Sets each action's check mark and greyed state from the editor, as the Windows menu does.
	fn sync_actions(&self) {
		let doc = self.doc.borrow();
		for (c, a) in self.actions.borrow().iter() {
			if matches!(c, Command::Recent(_)) {
				continue;
			}
			let m = doc.mark(*c);
			if is_stateful(*c) {
				a.set_state(&m.checked.to_variant());
			}
			a.set_enabled(!m.disabled);
		}
	}

	/// Runs a command from a menu, a button or a key.
	fn run_command(self: &Rc<Self>, c: Command) {
		match c {
			Command::Zoom(z) => {
				let zoom = if z == 0 {
					Zoom::Width
				} else {
					Zoom::Percent(u32::from(z))
				};
				self.zoom(move |v, _| v.set_zoom(zoom, None));
				return;
			}
			Command::Print | Command::Preview | Command::PrintSetup => {
				self.toast("Printing arrives in a later release of the Linux client.");
				return;
			}
			Command::Help => {
				self.toast("Help is not in the Linux client yet.");
				return;
			}
			Command::Save => {
				let ui = self.clone();
				self.save_current(Box::new(move |_, _| ui.changed()));
				return;
			}
			_ => {}
		}
		let dialog = self.doc.borrow_mut().command(c);
		if matches!(c, Command::Chrome(_)) {
			self.apply_chrome();
			let mut doc = self.doc.borrow_mut();
			doc.save_settings();
		}
		if matches!(c, Command::Open | Command::Recent(_) | Command::New) || dialog.is_none() {
			let g = self.doc.borrow().geometry();
			if matches!(c, Command::Open | Command::Recent(_) | Command::New) {
				self.view.borrow_mut().set_geometry(g);
			}
		}
		if let Some(d) = dialog {
			self.handle_dialog(d);
		}
		if self.doc.borrow().quit_requested() {
			self.close_window();
			return;
		}
		self.rebuild_menus();
		self.changed();
	}

	fn env(self: &Rc<Self>) -> Env {
		let weak = Rc::downgrade(self);
		let changed: Rc<dyn Fn()> = Rc::new(move || {
			if let Some(u) = weak.upgrade() {
				u.changed();
			}
		});
		let weak = Rc::downgrade(self);
		let toast: Rc<dyn Fn(&str)> = Rc::new(move |t| {
			if let Some(u) = weak.upgrade() {
				u.toast(t);
			}
		});
		Env {
			doc: self.doc.clone(),
			parent: self.window.clone().upcast(),
			changed,
			toast,
		}
	}

	/// Shows the dialog the editor asked for.
	fn handle_dialog(self: &Rc<Self>, d: Dialog) {
		self.trace(&format!("dialog {d:?}: asked"));
		let env = self.env();
		let shown = match d {
			Dialog::About => {
				dialogs::about(&self.window, APP_ID);
				None
			}
			Dialog::Open => {
				self.open_dialog(false);
				None
			}
			Dialog::SaveAs => {
				let ui = self.clone();
				self.save_as_dialog(Box::new(move |_, _| ui.changed()));
				None
			}
			Dialog::Picture => {
				self.picture_dialog();
				None
			}
			Dialog::Print => {
				self.toast("Printing arrives in a later release of the Linux client.");
				None
			}
			Dialog::Fonts => dialogs::fonts(&env),
			Dialog::Tabs => dialogs::tabs(&env),
			Dialog::Find(replace) => dialogs::find(&env, replace),
			Dialog::Spelling => dialogs::spelling(&env),
			Dialog::ConfirmSave(c) => {
				let cont: Continue = Rc::new(move |u| {
					let next = u.doc.borrow_mut().with(|app, s, f| {
						app.run_confirmed(s, f, c);
						app.take_dialog()
					});
					// Open, once the question is answered, goes on to ask for the file; the question is not asked again.
					if next == Some(Dialog::Open) {
						u.open_dialog(true);
					}
					if u.doc.borrow().quit_requested() {
						u.close_window();
					}
					u.rebuild_menus();
					let g = u.doc.borrow().geometry();
					u.view.borrow_mut().set_geometry(g);
					u.changed();
				});
				self.confirm_unsaved(cont);
				None
			}
		};
		if shown.is_some() {
			*self.last_dialog.borrow_mut() = shown;
		}
	}

	fn show_dialog_named(self: &Rc<Self>, name: &str) -> Option<Rc<DialogView>> {
		let env = self.env();
		let v = match name {
			"find" => dialogs::find(&env, false),
			"replace" => dialogs::find(&env, true),
			"fonts" => dialogs::fonts(&env),
			"tabs" => dialogs::tabs(&env),
			"spelling" => dialogs::spelling(&env),
			"about" => {
				dialogs::about(&self.window, APP_ID);
				None
			}
			other => {
				eprintln!("quill: no dialog named {other}");
				None
			}
		};
		if v.is_some() {
			*self.last_dialog.borrow_mut() = v.clone();
		}
		v
	}

	/// The test hook behind `--self-test-input`: the pointer's shape over a sheet and over the desk, and a drag that goes past the
	/// bottom of the page, which must scroll the view and grow the selection.
	fn self_test_input(self: &Rc<Self>) {
		let shape = |ui: &Rc<Ui>, x: f64, y: f64| {
			ui.update_cursor(x, y);
			ui.page
				.cursor()
				.and_then(|c| c.name())
				.map_or_else(|| "none".to_string(), |n| n.to_string())
		};
		let w = f64::from(self.page.width());
		println!("cursor over a sheet: {}", shape(self, w / 2.0, 150.0));
		println!("cursor over the desk: {}", shape(self, 2.0, 150.0));
		let h = f64::from(self.page.height());
		let none = gdk::ModifierType::empty();
		let before = self.view.borrow().scroll().1;
		self.pointer_event(none, 0, w / 2.0, 200.0, 0);
		self.dragging.set(true);
		for _ in 0..12 {
			self.pointer_event(none, 0, w / 2.0, h + 30.0, 1);
		}
		self.pointer_event(none, 0, w / 2.0, h + 30.0, 2);
		self.dragging.set(false);
		let after = self.view.borrow().scroll().1;
		let sel = self.doc.borrow().selection();
		println!("drag past the bottom: scroll {before} -> {after}, selection {sel:?}");
		self.closing.set(true);
		self.window.close();
	}

	/// The unsaved-changes question: Save, Don't Save or Cancel, with the shared verdict (`end_session_allowed`, as the Windows
	/// shells' question on logging off).
	fn confirm_unsaved(self: &Rc<Self>, cont: Continue) {
		let name = self
			.doc
			.borrow()
			.name()
			.map_or_else(|| "Untitled".to_string(), str::to_string);
		let ui = self.clone();
		let asker = self.asker.borrow().clone();
		asker.ask_save(
			&name,
			Box::new(move |reply| {
				let answer = match reply {
					Reply::Save => ANSWER_SAVE,
					Reply::Discard => ANSWER_DISCARD,
					Reply::Cancel => 0,
				};
				if answer == ANSWER_SAVE {
					let ui2 = ui.clone();
					ui.save_current(Box::new(move |_, saved| {
						if end_session_allowed(true, answer, saved) {
							cont(&ui2);
						}
					}));
				} else if end_session_allowed(true, answer, false) {
					ui.doc.borrow_mut().discard_changes();
					cont(&ui);
				}
			}),
		);
	}

	/// Saves under the document's name, asking for one first when it has none. `then` hears whether it was saved.
	fn save_current(self: &Rc<Self>, then: Saved) {
		if self.doc.borrow().path().is_none() {
			self.save_as_dialog(then);
			return;
		}
		let result = self.doc.borrow_mut().save();
		match &result {
			Ok(()) => self.toast("Saved"),
			Err(m) => self.toast(m),
		}
		self.rebuild_menus();
		then(self, result.is_ok());
	}

	fn save_as_dialog(self: &Rc<Self>, then: Saved) {
		let (folder, name) = {
			let d = self.doc.borrow();
			(
				d.path().and_then(|p| p.parent().map(Path::to_path_buf)),
				d.name().unwrap_or("Untitled.qld").to_string(),
			)
		};
		// GTK's dialog cannot say which type was chosen, so a bare name takes the type listed first: the document's own.
		let first = default_save_filter(Some(&name));
		let ui = self.clone();
		let chooser = self.chooser.borrow().clone();
		chooser.save_as(
			folder,
			name,
			first,
			Box::new(move |outcome| {
				let saved = match outcome {
					Ok(choice) => {
						let path = complete_path(&choice.path, choice.filter.unwrap_or(first));
						let r = ui.doc.borrow_mut().save_as(&path);
						match &r {
							Ok(()) => {
								ui.toast("Saved");
								ui.trace(&format!("saved {}", path.display()));
							}
							Err(m) => ui.toast(m),
						}
						ui.rebuild_menus();
						r.is_ok()
					}
					Err(Failure::Cancelled) => false,
					Err(Failure::Failed(m)) => {
						ui.toast(&format!("The Save As dialog failed: {m}"));
						false
					}
				};
				then(&ui, saved);
			}),
		);
	}

	fn picture_dialog(self: &Rc<Self>) {
		let folder = self
			.doc
			.borrow()
			.path()
			.and_then(|p| p.parent().map(Path::to_path_buf));
		let ui = self.clone();
		let chooser = self.chooser.borrow().clone();
		chooser.picture(
			folder,
			Box::new(move |outcome| match outcome {
				Ok(choice) => {
					let r = ui.doc.borrow_mut().insert_picture(&choice.path);
					match r {
						Ok(()) => ui.trace(&format!("picture {}", choice.path.display())),
						Err(m) => ui.toast(&m),
					}
					ui.changed();
				}
				Err(Failure::Cancelled) => {}
				Err(Failure::Failed(m)) => {
					ui.toast(&format!("The Insert Picture dialog failed: {m}"))
				}
			}),
		);
	}

	/// Opens a document in this window, asking about unsaved changes first.
	pub fn open_path(self: &Rc<Self>, path: &Path) {
		if self.doc.borrow().unsaved_work() {
			let p = path.to_path_buf();
			let cont: Continue = Rc::new(move |u| u.open_now(&p));
			self.confirm_unsaved(cont);
		} else {
			self.open_now(path);
		}
	}

	fn open_now(self: &Rc<Self>, path: &Path) {
		let result = self.doc.borrow_mut().open(path);
		match result {
			Ok(()) => {
				self.trace(&format!("opened {}", path.display()));
				let geom = self.doc.borrow().geometry();
				self.view.borrow_mut().set_geometry(geom);
				self.rebuild_menus();
				self.changed();
			}
			Err(message) => {
				let dialog =
					adw::AlertDialog::new(Some("Quill cannot open the file"), Some(&message));
				dialog.add_response("ok", "_OK");
				dialog.present(Some(&self.window));
			}
		}
	}

	/// Asks for a file to open. `confirmed` when the unsaved-changes question has been answered already.
	fn open_dialog(self: &Rc<Self>, confirmed: bool) {
		let folder = self
			.doc
			.borrow()
			.path()
			.and_then(|p| p.parent().map(Path::to_path_buf));
		let ui = self.clone();
		let chooser = self.chooser.borrow().clone();
		chooser.open(
			folder,
			Box::new(move |outcome| match outcome {
				Ok(choice) if confirmed => ui.open_now(&choice.path),
				Ok(choice) => ui.open_path(&choice.path),
				// Closing the dialog without a choice is not a failure.
				Err(Failure::Cancelled) => {}
				Err(Failure::Failed(m)) => eprintln!("quill: the Open dialog failed: {m}"),
			}),
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

	/// The status bar's minus (`up` false) or plus button: chooses the next level of `ZOOM_LEVELS` below or above the zoom drawn now.
	fn zoom_step(self: &Rc<Self>, up: bool) {
		let shown = self.view.borrow().effective_zoom() as i32;
		if let Some(command) = quill_chrome::zoom_field::zoom_step(shown, up) {
			self.run_command(command);
		}
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

	fn set_theme(&self) {
		let styles = adw::StyleManager::default();
		if let Some(dark) = self.opts.dark {
			styles.set_color_scheme(if dark {
				adw::ColorScheme::ForceDark
			} else {
				adw::ColorScheme::ForceLight
			});
		}
		self.page.set_dark(styles.is_dark());
	}

	/// The view moved or zoomed: redraw, and keep the zoom for the next session.
	fn changed(self: &Rc<Self>) {
		let zoom = self.view.borrow().zoom();
		{
			let mut d = self.doc.borrow_mut();
			d.remember_zoom(zoom);
			let g = d.geometry();
			self.view.borrow_mut().update_geometry(g);
		}
		self.refresh();
		self.update_im();
		self.sync_overlay();
		self.page.queue_draw();
	}

	/// Brings the scroll bars, the bars of the window, the title and the actions up to date with the view and the document.
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
		let doc = self.doc.borrow();
		self.status.show(&StatusInfo::of(&doc, &v));
		self.format.show(&FormatState::of(&doc));
		let (margin_px, text_px, state) = doc.ruler_info();
		self.ruler.show(
			RulerGeometry {
				page_left: f64::from(v.page_left()) * f64::from(v.effective_zoom()) / 100.0,
				zoom: f64::from(v.effective_zoom()) / 100.0,
				margin_px,
				text_px,
			},
			state,
		);
		self.window.set_title(Some(&doc.title()));
		self.title.set_title(&doc.heading());
		self.title.set_subtitle(&doc.subtitle());
		drop(doc);
		drop(v);
		self.sync_actions();
		self.syncing.set(false);
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
		let Some(content) = self.window.content() else {
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
