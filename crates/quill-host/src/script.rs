// Key scripts: the same typed keys the DOS end-to-end run sends, played through the app on the host.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::HashMap;
use std::fmt::Write as _;

use quill_app::{App, Files, Key};
use quill_grid::Grid;

use crate::HostStore;

/// In-memory files, upper-case names like DOS.
#[derive(Default)]
pub struct MemFiles {
	/// The files by name.
	pub files: HashMap<String, Vec<u8>>,
}

/// A source over a copy of a file.
pub struct MemSource(Vec<u8>, usize);

impl quill_formats::io::Source for MemSource {
	fn read(&mut self, out: &mut [u8]) -> usize {
		let n = out.len().min(self.0.len() - self.1);
		out[..n].copy_from_slice(&self.0[self.1..self.1 + n]);
		self.1 += n;
		n
	}
}

/// A file being written.
pub struct NamedSink(String, Vec<u8>);

impl NamedSink {
	/// The (upper-case) name being written.
	pub fn name(&self) -> &str {
		&self.0
	}
}

impl quill_formats::io::Sink for NamedSink {
	fn write(&mut self, data: &[u8]) -> bool {
		self.1.extend_from_slice(data);
		true
	}
}

fn key_name(name: &[u8]) -> String {
	String::from_utf8_lossy(name).to_ascii_uppercase()
}

impl Files for MemFiles {
	type R = MemSource;
	type W = NamedSink;

	fn open(&mut self, name: &[u8]) -> Option<MemSource> {
		self.files
			.get(&key_name(name))
			.map(|b| MemSource(b.clone(), 0))
	}

	fn create(&mut self, name: &[u8]) -> Option<Self::W> {
		Some(NamedSink(key_name(name), Vec::new()))
	}

	fn close(&mut self, w: Self::W) -> bool {
		self.files.insert(w.0, w.1);
		true
	}
}

/// Turns one AUTOTYPE-style token into a key (see `quill_app::script`).
pub fn parse_token(t: &str) -> Option<Key> {
	quill_app::script::parse_token(t)
}

/// A key script: directives (`@mono`, `@rows N`, `@open NAME`, `@copy NAME`) and key tokens, with `#` starting a comment.
#[derive(Default)]
pub struct Script {
	/// The keys.
	pub keys: Vec<Key>,
	/// Start in the mono attribute scheme.
	pub mono: bool,
	/// The screen's rows: 25 (the default, 0 here) or 50.
	pub rows: usize,
	/// A file named on the command line.
	pub open: Option<String>,
	/// Test documents to copy to the disk before the run.
	pub copy: Vec<String>,
}

/// Reads a script: tokens separated by white space, `#` starting a comment to the end of the line.
pub fn parse_script(text: &str) -> Script {
	let mut sc = Script::default();
	for line in text.lines().map(|l| l.split('#').next().unwrap_or("")) {
		let mut it = line.split_whitespace().peekable();
		while let Some(t) = it.next() {
			match t {
				"@mono" => sc.mono = true,
				"@rows" => {
					sc.rows = match it.next() {
						Some("25") => 25,
						Some("50") => 50,
						other => panic!("@rows takes 25 or 50, not {other:?}"),
					}
				}
				"@open" => sc.open = it.next().map(str::to_string),
				"@copy" => sc.copy.extend(it.next().map(str::to_string)),
				_ => sc
					.keys
					.push(parse_token(t).unwrap_or_else(|| panic!("unknown key token {t:?}"))),
			}
		}
	}
	sc
}

/// Plays `keys` through a new app, writing a frame (`=== screen N ===`) for the start and after each key, and returns the files.
pub fn play(files: &mut MemFiles, script: &Script, frames: &mut String) {
	if script.rows == 50 {
		play_rows::<50>(files, script, frames);
	} else {
		play_rows::<25>(files, script, frames);
	}
}

fn play_rows<const ROWS: usize>(files: &mut MemFiles, script: &Script, frames: &mut String) {
	let mut s = HostStore::new();
	let mut app = App::new();
	app.reset(&mut s).unwrap();
	app.load_settings(files);
	app.set_about("Host test run");
	app.set_rows(ROWS - 2);
	if script.mono {
		app.set_scheme(quill_app::Scheme::Mono);
	}
	if let Some(name) = &script.open {
		app.open(&mut s, files, name.as_bytes()).unwrap();
	}
	let mut g = Grid::<ROWS>::new(0);
	let mut n = 1;
	quill_app::draw(&app, &s, &mut g);
	g.dump(n, frames).unwrap();
	for k in &script.keys {
		app.key(&mut s, files, *k);
		quill_app::draw(&app, &s, &mut g);
		n += 1;
		g.dump(n, frames).unwrap();
		if app.quit() {
			break;
		}
	}
	app.save_settings(files);
	let _ = writeln!(frames, "=== end ===");
}
