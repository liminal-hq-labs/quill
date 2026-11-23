// QUILLDOS.EXE: Quill for DOS.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! T4: draws the `quill-grid` hello screen in text mode. Set `QUILL_DUMP=D:\DUMP.TXT` to write the screen as a golden
//! frame, which is how the headless tests compare it with the host's.
#![no_std]
#![no_main]

extern crate alloc;

mod video;

use alloc::string::String;
use rt::dos::{File, Mode};
use rt::env;

rt::entry!(main);

fn main() -> i32 {
	let grid = quill_grid::hello::<25>();
	video::present(&grid);
	if let Some(path) = env::var("QUILL_DUMP") {
		let mut text = String::new();
		let _ = video::read_back::<25>().dump(1, &mut text);
		match File::open(&path, Mode::Create) {
			Ok(mut f) => {
				if f.write_all(text.as_bytes()).is_err() {
					return 2;
				}
			}
			Err(_) => return 2,
		}
	}
	if env::var("QUILL_HOLD").is_some() {
		// Leave the screen up for a screenshot until a key is pressed.
		video::wait_key();
	}
	video::leave();
	rt::println!("QUILLDOS T4 hello ok");
	0
}
