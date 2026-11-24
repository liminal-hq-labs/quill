// QUILLDOS.EXE: Quill for DOS.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The editor in 80 by 25 text mode, or 80 by 50 with `/50` or `Rows=50` under `[Quill]` in `QUILL.INI`. `QUILLDOS.EXE [FILE]` opens a file. Set `QUILL_DUMP=D:\DUMP.TXT` to write every screen
//! shown, read back from video memory, as golden frames: how the headless tests compare it with the host's.
#![no_std]
#![no_main]

extern crate alloc;

mod files;
mod keys;
mod video;

use alloc::boxed::Box;
use alloc::string::String;
use quill_app::{App, Scheme};
use quill_grid::Grid;
use quill_heap::HeapStore;
use rt::dos::{File, Mode};
use rt::env;

rt::entry!(main);

fn show<const ROWS: usize>(app: &App, s: &HeapStore, dump: &mut Option<File>, n: &mut u32) {
	let mut g = Grid::<ROWS>::new(0);
	let cursor = quill_app::draw(app, s, &mut g);
	video::present(&g);
	video::cursor(cursor.map(|(x, y)| (x as u8, y as u8)));
	if cursor.is_some() {
		video::cursor_shape(app.overtype());
	}
	if let Some(f) = dump {
		let mut text = String::new();
		*n += 1;
		let _ = video::read_back::<ROWS>().dump(*n, &mut text);
		let _ = f.write_all(text.as_bytes());
	}
}

/// The rows `QUILL.INI` asks for (`Rows=` under `[Quill]`), or 25.
fn rows_setting(ini: &[u8]) -> usize {
	let mut rows = 25;
	for line in ini.split(|b| *b == b'\n') {
		let line = line.strip_suffix(b"\r").unwrap_or(line);
		if let Some(v) = line.strip_prefix(&b"Rows="[..]) {
			rows = if v.starts_with(b"50") { 50 } else { 25 };
		}
	}
	rows
}

fn main() -> i32 {
	let mut ini = [0u8; 512];
	let n = File::open("QUILL.INI", Mode::Read)
		.ok()
		.and_then(|mut f| f.read(&mut ini).ok())
		.unwrap_or(0);
	let mut rows = rows_setting(&ini[..n]);
	let from_ini = rows == 50;
	for a in &env::args() {
		if a.as_str() == "/50" || a.as_str() == "-50" {
			rows = 50;
		}
	}
	// The screen decides: a card with no 50-row mode keeps 25.
	let rows = video::enter(rows);
	if rows == 50 {
		run::<50>(from_ini)
	} else {
		run::<25>(from_ini)
	}
}

fn run<const ROWS: usize>(rows_in_ini: bool) -> i32 {
	let mut store = HeapStore::new();
	let mut app = Box::new(App::new());
	let mut files = files::DosFiles;
	if app.reset(&mut store).is_err() {
		video::leave();
		rt::println!("Quill: there is not enough memory.");
		return 1;
	}
	app.load_settings(&mut files);
	app.set_about("Host test run");
	app.set_rows(ROWS - 2);
	let args = env::args();
	for a in &args {
		if a.eq_ignore_ascii_case("/mono") || a.eq_ignore_ascii_case("-mono") {
			app.set_scheme(Scheme::Mono);
		} else if !a.starts_with('/') && !a.starts_with('-') {
			let _ = app.open(&mut store, &mut files, a.as_bytes());
		}
	}
	let mut dump = env::var("QUILL_DUMP").and_then(|p| File::open(&p, Mode::Create).ok());
	let mut n = 0;
	show::<ROWS>(&app, &store, &mut dump, &mut n);
	while !app.quit() {
		if let Some(k) = keys::wait() {
			app.key(&mut store, &mut files, k);
			show::<ROWS>(&app, &store, &mut dump, &mut n);
		}
	}
	app.save_settings(&mut files);
	if rows_in_ini {
		// `save_settings` writes only the keys the app knows, so the choice of 50 rows is put back at the end of `[Quill]`.
		let kept = File::open("QUILL.INI", Mode::Read)
			.ok()
			.and_then(|mut f| f.read_to_vec(0).ok());
		if let (Some(mut text), Ok(mut f)) = (kept, File::open("QUILL.INI", Mode::Create)) {
			text.extend_from_slice(b"Rows=50\r\n");
			let _ = f.write_all(&text);
		}
	}
	if let Some(f) = &mut dump {
		let _ = f.write_all(b"=== end ===\n");
	}
	video::leave();
	0
}
