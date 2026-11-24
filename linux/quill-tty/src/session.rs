// The editor, its screen and its files: one terminal session, with no terminal in it.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// A session takes keys and gives back a `Grid`, exactly as the DOS build does, so the tests drive it with scripts and compare the
// screens; the terminal layer only turns events into keys and the grid into escape sequences.

use quill_app::{draw, App, Cursor, Files, Key, Scheme};
use quill_grid::{Cell, Grid, COLS};
use quill_host::HostStore;

/// The screen, 25 rows or 50 as the terminal allows.
#[derive(Clone)]
pub enum Screen {
	/// 80 × 25.
	Rows25(Box<Grid<25>>),
	/// 80 × 50.
	Rows50(Box<Grid<50>>),
}

impl Screen {
	/// The number of rows.
	pub fn rows(&self) -> usize {
		match self {
			Screen::Rows25(_) => 25,
			Screen::Rows50(_) => 50,
		}
	}

	/// The cell at (column, row).
	pub fn get(&self, x: usize, y: usize) -> Option<Cell> {
		match self {
			Screen::Rows25(g) => g.get(x, y),
			Screen::Rows50(g) => g.get(x, y),
		}
	}

	/// The screen as a golden frame, as `Grid::dump` writes it.
	pub fn dump(&self, n: u32, out: &mut String) {
		let _ = match self {
			Screen::Rows25(g) => g.dump(n, out),
			Screen::Rows50(g) => g.dump(n, out),
		};
	}
}

/// How a session starts.
#[derive(Clone, Debug, Default)]
pub struct Options {
	/// The platform line of the About box.
	pub about: String,
	/// Start in the monochrome attribute scheme.
	pub mono: bool,
	/// Use the 80 × 50 screen.
	pub tall: bool,
	/// A file to open, as the editor names it.
	pub open: Option<Vec<u8>>,
}

/// One editing session.
pub struct Session<F: Files> {
	app: Box<App>,
	store: HostStore,
	files: F,
	screen: Screen,
	cursor: Option<Cursor>,
}

impl<F: Files> Session<F> {
	/// Starts a session: a new document, the settings read, the file opened if one was named, and the screen drawn.
	pub fn new(mut files: F, opts: &Options) -> Result<Session<F>, String> {
		let mut store = HostStore::new();
		let mut app = Box::new(App::new());
		app.reset(&mut store)
			.map_err(|_| "not enough memory".to_string())?;
		app.load_settings(&mut files);
		app.set_about(&opts.about);
		if opts.mono {
			app.set_scheme(Scheme::Mono);
		}
		if let Some(name) = &opts.open {
			app.open(&mut store, &mut files, name)
				.map_err(|_| format!("cannot open {}", crate::cp1252::ansi_to_string(name)))?;
		}
		let screen = if opts.tall {
			Screen::Rows50(Box::new(Grid::new(0)))
		} else {
			Screen::Rows25(Box::new(Grid::new(0)))
		};
		let mut s = Session {
			app,
			store,
			files,
			screen,
			cursor: None,
		};
		s.set_tall(opts.tall);
		Ok(s)
	}

	/// Switches between the 25 and the 50 row screen and redraws.
	pub fn set_tall(&mut self, tall: bool) {
		self.screen = if tall {
			Screen::Rows50(Box::new(Grid::new(0)))
		} else {
			Screen::Rows25(Box::new(Grid::new(0)))
		};
		self.app.set_rows(self.screen.rows() - 2);
		self.redraw();
	}

	/// Handles one key and redraws.
	pub fn key(&mut self, k: Key) {
		self.app.key(&mut self.store, &mut self.files, k);
		self.redraw();
	}

	/// Paints the screen from the document.
	pub fn redraw(&mut self) {
		self.cursor = match &mut self.screen {
			Screen::Rows25(g) => draw(&self.app, &self.store, g.as_mut()),
			Screen::Rows50(g) => draw(&self.app, &self.store, g.as_mut()),
		};
	}

	/// The screen as last drawn.
	pub fn screen(&self) -> &Screen {
		&self.screen
	}

	/// Where the terminal cursor belongs; none while a menu is open.
	pub fn cursor(&self) -> Option<Cursor> {
		self.cursor
	}

	/// True when the editor was told to exit.
	pub fn quit(&self) -> bool {
		self.app.quit()
	}

	/// True in overtype mode.
	pub fn overtype(&self) -> bool {
		self.app.overtype()
	}

	/// Writes the settings; call once when the session ends.
	pub fn finish(&mut self) {
		self.app.save_settings(&mut self.files);
	}

	/// Ends the session and gives the files back, for a caller that wants to look at what it wrote.
	pub fn into_files(self) -> F {
		self.files
	}
}

/// The number of columns a screen has.
pub const WIDTH: usize = COLS;
