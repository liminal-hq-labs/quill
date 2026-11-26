// Key scripts: the same typed keys the DOS end-to-end run sends, played through the app on the host.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::HashMap;
use std::fmt::Write as _;

use quill_app::mouse::{Pointer, TextCells};
use quill_app::script::{parse_pointer, Step};
use quill_app::{App, Drive, Files, Key, LockMade, Owner, Stamp};
use quill_grid::Grid;

use crate::HostStore;

/// In-memory files, upper-case names like DOS, with a clock that a test moves by hand.
pub struct MemFiles {
	/// The files by name.
	pub files: HashMap<String, Vec<u8>>,
	/// The time now, as a DOS packed date and time.
	pub clock: u32,
	/// When each file was last written.
	pub times: HashMap<String, u32>,
	/// Lock notes whose owner is provably alive.
	pub alive: Vec<String>,
	/// What a place is, by the start of its path (upper case); anything else is a fixed disk.
	pub drives: Vec<(String, Drive)>,
	/// The central folders the shell offers, in order.
	pub dirs: Vec<String>,
	/// Names that start with one of these cannot be created.
	pub denied: Vec<String>,
}

impl Default for MemFiles {
	fn default() -> Self {
		MemFiles {
			files: HashMap::new(),
			clock: dos_time(2026, 11, 25, 14, 32, 0),
			times: HashMap::new(),
			alive: Vec::new(),
			drives: Vec::new(),
			dirs: vec!["RECOVER\\".to_string()],
			denied: Vec::new(),
		}
	}
}

/// A date and time as DOS packs them: the date in the high word, the time in the low one.
pub fn dos_time(y: u32, mo: u32, d: u32, h: u32, mi: u32, s: u32) -> u32 {
	((((y - 1980) << 9) | (mo << 5) | d) << 16) | ((h << 11) | (mi << 5) | (s / 2))
}

/// The DOS time `secs` seconds after `t`.
pub fn dos_add(t: u32, secs: u32) -> u32 {
	let (date, time) = (t >> 16, t & 0xFFFF);
	let (mut y, mut mo, mut d) = (1980 + (date >> 9), (date >> 5) & 15, date & 31);
	let mut total = (time >> 11) * 3600 + ((time >> 5) & 63) * 60 + (time & 31) * 2 + secs;
	let mut days = total / 86400;
	total %= 86400;
	while days > 0 {
		let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
		let dim = [
			31,
			if leap { 29 } else { 28 },
			31,
			30,
			31,
			30,
			31,
			31,
			30,
			31,
			30,
			31,
		][(mo - 1) as usize];
		d += 1;
		if d > dim {
			d = 1;
			mo += 1;
			if mo > 12 {
				mo = 1;
				y += 1;
			}
		}
		days -= 1;
	}
	dos_time(y, mo, d, total / 3600, total / 60 % 60, total % 60)
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
		let n = key_name(name);
		if self.denied.iter().any(|d| n.starts_with(d.as_str())) {
			return None;
		}
		Some(NamedSink(n, Vec::new()))
	}

	fn close(&mut self, w: Self::W) -> bool {
		self.times.insert(w.0.clone(), self.clock);
		self.files.insert(w.0, w.1);
		true
	}

	fn save_mode(&self) -> quill_app::SaveMode {
		quill_app::SaveMode::TempRename
	}

	fn remove(&mut self, name: &[u8]) -> bool {
		self.times.remove(&key_name(name));
		self.files.remove(&key_name(name)).is_some()
	}

	/// Renames as DOS does: a name that is taken is an error.
	fn rename(&mut self, from: &[u8], to: &[u8]) -> bool {
		if self.files.contains_key(&key_name(to)) {
			return false;
		}
		match self.files.remove(&key_name(from)) {
			Some(b) => {
				if let Some(t) = self.times.remove(&key_name(from)) {
					self.times.insert(key_name(to), t);
				}
				self.files.insert(key_name(to), b);
				true
			}
			None => false,
		}
	}

	fn len(&mut self, name: &[u8]) -> Option<u32> {
		self.files.get(&key_name(name)).map(|b| b.len() as u32)
	}

	fn now(&mut self) -> u32 {
		self.clock
	}

	fn stamp(&mut self, name: &[u8]) -> Option<Stamp> {
		let n = key_name(name);
		let size = self.files.get(&n)?.len() as u32;
		Some(Stamp {
			size,
			time: self.times.get(&n).copied().unwrap_or(0),
		})
	}

	fn drive(&mut self, name: &[u8]) -> Drive {
		let n = key_name(name);
		self.drives
			.iter()
			.find(|(p, _)| n.starts_with(p.as_str()))
			.map_or(Drive::Fixed, |(_, d)| *d)
	}

	fn make_dir(&mut self, _dir: &[u8]) -> bool {
		true
	}

	fn find_nth(&mut self, pattern: &[u8], n: usize, out: &mut [u8]) -> usize {
		let pat = key_name(pattern);
		let cut = pat.rfind(['\\', '/', ':']).map_or(0, |i| i + 1);
		let (dir, file) = pat.split_at(cut);
		let Some((pre, post)) = file.split_once('*') else {
			return 0;
		};
		let mut names: Vec<&String> = self
			.files
			.keys()
			.filter(|k| {
				let c = k.rfind(['\\', '/', ':']).map_or(0, |i| i + 1);
				k[..c] == *dir
					&& k.len() >= c + pre.len() + post.len()
					&& k[c..].starts_with(pre)
					&& k.ends_with(post)
			})
			.collect();
		names.sort();
		let Some(name) = names.get(n) else { return 0 };
		let base = &name[dir.len()..];
		let len = base.len().min(out.len());
		out[..len].copy_from_slice(&base.as_bytes()[..len]);
		len
	}

	fn recovery_dir(&mut self, n: u8, out: &mut [u8]) -> usize {
		let Some(d) = self.dirs.get(usize::from(n)) else {
			return 0;
		};
		out[..d.len()].copy_from_slice(d.as_bytes());
		d.len()
	}

	fn instance(&mut self) -> u32 {
		7
	}

	fn lock_owner(&mut self, name: &[u8]) -> Owner {
		if self.alive.contains(&key_name(name)) {
			Owner::Alive
		} else {
			Owner::Unknown
		}
	}

	fn lock_make(&mut self, name: &[u8], stamp: &[u8; 32]) -> LockMade {
		if self.files.contains_key(&key_name(name)) {
			return LockMade::Exists;
		}
		self.files.insert(key_name(name), stamp.to_vec());
		LockMade::Made
	}
}

/// Turns one AUTOTYPE-style token into a key (see `quill_app::script`).
pub fn parse_token(t: &str) -> Option<Key> {
	quill_app::script::parse_token(t)
}

/// A key script: directives (`@mono`, `@rows N`, `@open NAME`, `@copy NAME`, `@live`), key tokens and pointer directives
/// (`@click x y`, `@shiftclick x y`, `@dclick x y`, `@tclick x y`, `@drag x1 y1 x2 y2`, `@wheel n`, in text cells), with `#` starting
/// a comment.
#[derive(Default)]
pub struct Script {
	/// The keys and pointer actions, in order.
	pub steps: Vec<Step>,
	/// The keys, for scripts that have no pointer.
	pub keys: Vec<Key>,
	/// Start in the mono attribute scheme.
	pub mono: bool,
	/// The screen's rows: 25 (the default, 0 here) or 50.
	pub rows: usize,
	/// A file named on the command line.
	pub open: Option<String>,
	/// Test documents to copy to the disk before the run.
	pub copy: Vec<String>,
	/// Check Spelling as You Type is on, and the keyboard goes quiet after each key: the marks follow each frame.
	pub live: bool,
}

/// Reads a script: tokens separated by white space, `#` starting a comment to the end of the line.
pub fn parse_script(text: &str) -> Script {
	let mut sc = Script::default();
	for line in text.lines().map(|l| l.split('#').next().unwrap_or("")) {
		let mut it = line.split_whitespace().peekable();
		while let Some(t) = it.next() {
			match t {
				"@mono" => sc.mono = true,
				"@live" => sc.live = true,
				"@rows" => {
					sc.rows = match it.next() {
						Some("25") => 25,
						Some("50") => 50,
						other => panic!("@rows takes 25 or 50, not {other:?}"),
					}
				}
				"@open" => sc.open = it.next().map(str::to_string),
				"@copy" => sc.copy.extend(it.next().map(str::to_string)),
				_ if t.starts_with('@') => {
					let step = parse_pointer(t, &mut it)
						.unwrap_or_else(|| panic!("unknown directive {t:?}"))
						.unwrap_or_else(|e| panic!("{t}: {e}"));
					sc.steps.push(step);
				}
				_ => {
					let k = parse_token(t).unwrap_or_else(|| panic!("unknown key token {t:?}"));
					sc.keys.push(k);
					sc.steps.push(Step::Key(k));
				}
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
	app.set_spell_live_default(script.live);
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
	let mut pointer = Pointer::default();
	let target = TextCells {
		rows: (ROWS - 2) as i32,
	};
	let mut tick = 0;
	for step in &script.steps {
		match step {
			Step::Key(k) => app.key(&mut s, files, *k),
			Step::Clock(secs) | Step::Quiet(secs) => {
				files.clock = dos_add(files.clock, (*secs).max(0) as u32);
			}
			other => other.play(&mut tick, |ev, ctx| {
				pointer.handle(&mut app, &mut s, files, &target, ev, ctx)
			}),
		}
		quill_app::draw(&app, &s, &mut g);
		n += 1;
		g.dump(n, frames).unwrap();
		if script.live {
			// The keyboard goes quiet: the whole document is on the screen and the scanner runs out of work.
			app.spell_set_view(0, app.doc().body.len());
			if app.spell_live_ready(&mut s, files) {
				while app.spell_idle(&mut s, 8) {}
			}
			let marks: Vec<String> = app
				.spell_marks(&s, 0, u32::MAX)
				.map(|(a, b)| format!("{a}-{b}"))
				.collect();
			let _ = writeln!(frames, "marks: {}", marks.join(" "));
		}
		if app.quit() {
			break;
		}
	}
	app.save_settings(files);
	let _ = writeln!(frames, "=== end ===");
}
