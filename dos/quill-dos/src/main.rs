// QUILLDOS.EXE: Quill for DOS.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The editor in 80 by 25 text mode. `QUILLDOS.EXE [FILE]` opens a file. Set `QUILL_DUMP=D:\DUMP.TXT` to write every screen
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

fn show(app: &App, s: &HeapStore, dump: &mut Option<File>, n: &mut u32) {
	let mut g = Grid::<25>::new(0);
	let cursor = quill_app::draw(app, s, &mut g);
	video::present(&g);
	video::cursor(cursor.map(|(x, y)| (x as u8, y as u8)));
	if cursor.is_some() {
		video::cursor_shape(app.overtype());
	}
	if let Some(f) = dump {
		let mut text = String::new();
		*n += 1;
		let _ = video::read_back::<25>().dump(*n, &mut text);
		let _ = f.write_all(text.as_bytes());
	}
}

fn main() -> i32 {
	let mut store = HeapStore::new();
	let mut app = Box::new(App::new());
	let mut files = files::DosFiles;
	if app.reset(&mut store).is_err() {
		rt::println!("Quill: there is not enough memory.");
		return 1;
	}
	app.load_settings(&mut files);
	// The test runs (`QUILL_DUMP`) keep one fixed line so the DOS screens match the host's; otherwise About says what it runs on.
	if env::var("QUILL_DUMP").is_some() {
		app.set_about("Host test run");
	} else {
		let (dos_major, dos_minor) = rt::dos::dos_version();
		let (dpmi_major, dpmi_minor, ..) = rt::dpmi::version();
		let mem = rt::dpmi::free_memory().unwrap_or_default();
		let free = (mem.free_pages != u32::MAX).then(|| mem.free_pages.saturating_mul(4));
		let mut text = [0u8; 96];
		let n = quill_app::platform::dos_about(
			&mut text,
			(dos_major, dos_minor),
			(dpmi_major, dpmi_minor),
			mem.largest_block / 1024,
			free,
		);
		app.set_about(core::str::from_utf8(text.get(..n).unwrap_or(&[])).unwrap_or(""));
	}
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
	video::enter();
	show(&app, &store, &mut dump, &mut n);
	while !app.quit() {
		if let Some(k) = keys::wait() {
			app.key(&mut store, &mut files, k);
			show(&app, &store, &mut dump, &mut n);
		}
	}
	app.save_settings(&mut files);
	if let Some(f) = &mut dump {
		let _ = f.write_all(b"=== end ===\n");
	}
	video::leave();
	0
}
