// The open document: the shared `App`, its store and files, and a drawing of the sheets that are on screen.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Layout is the app's own, at 96 pixels to the inch, laid out once into checkpoints (`App::open` syncs the layout), so a paint
// walks only the lines of the sheets on screen. A paint draws into an RGBA buffer of exactly the widget's device size through the
// pixel surface, the way every client of `quill-paint` does, and the widget hands that buffer to GTK as a texture.

use std::path::{Path, PathBuf};

use quill_app::picture::WINDOW;
use quill_app::view::page_width;
use quill_app::{App, Command, Dialog, Mark};
use quill_core::doc::StoryKind;
use quill_core::layout::{dump_story, to_units};
use quill_fonts::{FontSet, MeasureGlyphs, OutlineMeasure};
use quill_host::HostStore;
use quill_paint::page::{paint_sheet, DESK};
use quill_paint::pixel::{ByteOrder, PixelBuffer};
use quill_paint::{Rect, Rgb, Surface};

use crate::files::GuiFiles;
use crate::names::{display_editor_name, display_name, heading, open_failure, subtitle, title};
use crate::view::{Geometry, View, Zoom};

/// The section of `QUILL.INI` that holds the recent files of a build with long names.
const RECENT_SECTION: &str = "Recent32";

/// The colours of what is behind the sheets. The paper is white in every theme.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Theme {
	/// The desk the sheets lie on.
	pub desk: Rgb,
}

impl Theme {
	/// The grey of the Windows builds' page view.
	pub const LIGHT: Theme = Theme { desk: DESK };
	/// A dark desk, for a dark system theme.
	pub const DARK: Theme = Theme {
		desk: Rgb(46, 46, 50),
	};

	/// The theme for a system that prefers dark or not.
	pub fn of(dark: bool) -> Theme {
		if dark {
			Theme::DARK
		} else {
			Theme::LIGHT
		}
	}
}

/// A recent file for a menu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recent {
	/// The file name to show.
	pub label: String,
	/// The path the editor stored, to open again.
	pub path: PathBuf,
}

/// One document, open or empty.
pub struct Document {
	app: Box<App<OutlineMeasure>>,
	store: HostStore,
	files: GuiFiles,
	fonts: FontSet,
	/// The measure used to paint headers and footers.
	layout: OutlineMeasure,
	/// The measure the glyphs come from.
	glyphs: OutlineMeasure,
	window: Box<[u8; WINDOW]>,
	path: Option<PathBuf>,
	name: Option<String>,
	revision: u64,
	geometry: Geometry,
}

impl Document {
	/// An empty document over the given fonts and files; the settings (recent files and zoom) are read.
	pub fn new(fonts: FontSet, mut files: GuiFiles) -> Result<Document, String> {
		let measure = || OutlineMeasure::new(fonts.clone()).with_resolution(96);
		let mut store = HostStore::new();
		let mut app = Box::new(App::with_measure(measure()));
		app.reset(&mut store)
			.map_err(|_| "There is not enough memory to start Quill.".to_string())?;
		app.set_recent_section(RECENT_SECTION);
		// Dialogs are the window's, never text cells (D86): a command that wants one leaves a `Dialog` request.
		app.set_native_dialogs(true);
		app.load_settings(&mut files);
		let mut doc = Document {
			layout: measure(),
			glyphs: measure(),
			app,
			store,
			files,
			fonts,
			window: Box::new([0; WINDOW]),
			path: None,
			name: None,
			revision: 0,
			geometry: Geometry::default(),
		};
		doc.geometry = doc.measure_geometry();
		Ok(doc)
	}

	fn measure_geometry(&self) -> Geometry {
		let page = &self.app.doc().page;
		Geometry {
			page_w: page_width(&self.app),
			page_h: to_units(page.paper_h, 96),
			pages: self.app.page_total(&self.store).max(1),
		}
	}

	/// Opens a file in place of the document, and keeps the one that is open when it cannot be read.
	pub fn open(&mut self, path: &Path) -> Result<(), String> {
		let shown = display_name(path);
		if !path.is_file() {
			return Err(format!("{shown} could not be found."));
		}
		let name = self.files.name_for(path);
		self.app
			.open(&mut self.store, &mut self.files, &name)
			.map_err(|e| open_failure(&shown, e))?;
		self.path = Some(path.to_path_buf());
		self.name = Some(shown);
		self.revision += 1;
		self.geometry = self.measure_geometry();
		self.app.save_settings(&mut self.files);
		Ok(())
	}

	/// Opens one of the recent files.
	pub fn open_recent(&mut self, recent: &Recent) -> Result<(), String> {
		self.open(&recent.path)
	}

	/// The recent files, most recent first.
	pub fn recent(&self) -> Vec<Recent> {
		self.app
			.recent_files()
			.map(|n| Recent {
				label: display_editor_name(n),
				path: PathBuf::from(quill_tty::cp1252::ansi_to_string(n)),
			})
			.collect()
	}

	/// Writes the document as the editor does when told the person chose not to save: clears its changed mark so a command that
	/// replaces it can go on.
	pub fn discard_changes(&mut self) {
		self.with(|app, _, _| app.discard_changes());
	}

	/// True when there are changes that are not on disk: what the question on close is asked for.
	pub fn unsaved_work(&self) -> bool {
		self.app.unsaved_work()
	}

	/// True once the editor was asked to exit and has nothing left to ask.
	pub fn quit_requested(&self) -> bool {
		self.app.quit()
	}

	/// The window title.
	pub fn title(&self) -> String {
		title(self.name.as_deref(), self.app.modified())
	}

	/// Runs `f` over the app, its store and its files, then brings what a paint depends on up to date: a command from a menu, a
	/// button or a dialog goes through here.
	pub fn with<R>(
		&mut self,
		f: impl FnOnce(&mut App<OutlineMeasure>, &mut HostStore, &mut GuiFiles) -> R,
	) -> R {
		let r = f(&mut self.app, &mut self.store, &mut self.files);
		self.sync_name();
		self.revision += 1;
		self.geometry = self.measure_geometry();
		r
	}

	/// Reads the app and its store: the state a menu, a bar or the ruler shows.
	pub fn read<R>(&self, f: impl FnOnce(&App<OutlineMeasure>, &HostStore) -> R) -> R {
		f(&self.app, &self.store)
	}

	/// Takes the editor's idea of the file name back into the path and the name shown: a new document has none, and a file the
	/// editor opened itself (a recent file) has the path it kept.
	fn sync_name(&mut self) {
		let name = self.app.name();
		if name.is_empty() {
			self.path = None;
			self.name = None;
			return;
		}
		let shown = self.files.path_of(name);
		if self.path.as_deref() != Some(shown.as_path()) {
			self.name = Some(display_name(&shown));
			self.path = Some(shown);
		}
	}

	/// Starts an empty document. The changes are gone: ask first (`unsaved_work`).
	pub fn new_document(&mut self) {
		self.with(|app, s, f| {
			app.discard_changes();
			app.run_confirmed(s, f, Command::New);
		});
	}

	/// Saves under the current name. `Err` says why not, for a message.
	pub fn save(&mut self) -> Result<(), String> {
		let shown = self.name.clone().unwrap_or_default();
		self.with(|app, s, f| {
			app.command(s, f, Command::Save);
			if app.modified() {
				Err(format!("{shown} could not be saved."))
			} else {
				Ok(())
			}
		})
	}

	/// Saves under a new name, adding the default extension when the name has none. `Err` says why not.
	pub fn save_as(&mut self, path: &Path) -> Result<(), String> {
		let name = self.files.name_for(path);
		let mut buf = [0u8; crate::files::NAME_MAX + 8];
		let n = name.len().min(crate::files::NAME_MAX);
		buf[..n].copy_from_slice(&name[..n]);
		let n = quill_app::savename::complete_name(&mut buf, n, 1);
		let name = buf[..n].to_vec();
		let shown = display_name(path);
		let ok = self.with(|app, s, f| app.save_as(s, f, &name));
		if ok {
			Ok(())
		} else {
			Err(format!("{shown} could not be saved."))
		}
	}

	/// Inserts a picture at the caret.
	pub fn insert_picture(&mut self, path: &Path) -> Result<(), String> {
		let name = self.files.name_for(path);
		let shown = display_name(path);
		self.with(|app, s, f| app.insert_picture(s, f, &name))
			.map_err(|_| format!("{shown} is not a picture Quill can use."))
	}

	/// The margin and width of the text in document pixels, and the paragraph at the caret, for the ruler.
	pub fn ruler_info(&self) -> (i32, i32, quill_app::view::RulerState) {
		self.read(|app, s| {
			let page = &app.doc().page;
			(
				to_units(page.margin_l, 96),
				to_units(page.text_w(), 96),
				quill_app::view::RulerState::of(app, s),
			)
		})
	}

	/// Runs a command and returns the dialog it asked for, if any.
	pub fn command(&mut self, c: Command) -> Option<Dialog> {
		self.with(|app, s, f| {
			app.command(s, f, c);
			app.take_dialog()
		})
	}

	/// The check mark and the greyed state of the menu item that runs `c`.
	pub fn mark(&self, c: Command) -> Mark {
		self.read(|app, s| app.menu_mark(s, c))
	}

	/// The font a document uses at the caret and its set of fonts the page can draw, for the formatting bar.
	pub fn fonts(&self) -> &FontSet {
		&self.fonts
	}

	/// The header bar's heading.
	pub fn heading(&self) -> String {
		heading(self.name.as_deref(), self.app.modified())
	}

	/// The header bar's subtitle.
	pub fn subtitle(&self) -> String {
		let home = std::env::var_os("HOME").map(PathBuf::from);
		subtitle(self.path.as_deref(), home.as_deref())
	}

	/// The path of the open file.
	pub fn path(&self) -> Option<&Path> {
		self.path.as_deref()
	}

	/// The file name of the open document, as the person sees it.
	pub fn name(&self) -> Option<&str> {
		self.name.as_deref()
	}

	/// True when there are unsaved changes (never, in a viewer).
	pub fn modified(&self) -> bool {
		self.app.modified()
	}

	/// A number that changes whenever what a paint draws could, so a texture can be kept.
	pub fn revision(&self) -> u64 {
		self.revision
	}

	/// The size of the sheets and how many there are.
	pub fn geometry(&self) -> Geometry {
		self.geometry
	}

	/// The zoom kept in the settings.
	pub fn saved_zoom(&self) -> Zoom {
		match self.app.zoom() {
			0 => Zoom::Width,
			p => Zoom::Percent(u32::from(p)),
		}
	}

	/// Remembers the zoom for the next session.
	pub fn remember_zoom(&mut self, zoom: Zoom) {
		self.app.set_zoom(match zoom {
			Zoom::Width => 0,
			Zoom::Percent(p) => p as u16,
		});
	}

	/// The folder the settings are in.
	pub fn config_dir(&self) -> &Path {
		&self.files.dirs().config
	}

	/// Writes the editor's settings (the recent files and the zoom).
	pub fn save_settings(&mut self) {
		self.app.save_settings(&mut self.files);
	}

	/// The body's layout as the host's layout dumps write it, at `units` to the inch with these fonts; a golden compares it.
	pub fn dump_layout(&self, units: u16, name: &str) -> String {
		let mut m = OutlineMeasure::new(self.fonts.clone()).with_resolution(units);
		let mut out = String::new();
		let _ = dump_story(
			self.app.doc(),
			&self.store,
			&mut m,
			name,
			StoryKind::Body,
			&mut out,
		);
		out
	}

	/// Draws the view: the desk, then every sheet that is on screen, into a buffer of exactly the view's device size.
	pub fn render(&mut self, view: &View, theme: Theme) -> PixelBuffer {
		let (dw, dh) = view.device();
		let mut buf = PixelBuffer::new(dw.max(1), dh.max(1));
		let (lw, lh) = view.visible();
		let area = Rect::new(0, 0, lw + 2, lh + 2);
		let pages = self.geometry.pages;
		let mut sf = buf.surface(
			ByteOrder::Rgba,
			view.mapping().scale(),
			MeasureGlyphs::new(&self.glyphs),
		);
		sf.fill_rect(area, theme.desk);
		for s in view.slots() {
			paint_sheet(
				&mut sf,
				&self.app,
				&mut self.layout,
				&self.store,
				s.left,
				s.top,
				s.page,
				pages,
				area,
				&mut self.window,
			);
		}
		drop(sf);
		buf
	}
}
