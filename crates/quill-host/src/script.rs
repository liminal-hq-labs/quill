// Key scripts: the same typed keys the DOS end-to-end run sends, played through the app on the host.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::HashMap;
use std::fmt::Write as _;

use quill_app::idle::Idle;
use quill_app::mouse::{Pointer, TextCells};
use quill_app::script::{parse_pointer, Step};
use quill_app::{App, Drive, Files, Key, LockMade, Owner, Stamp};
use quill_dlg::fake::FakeFs;
use quill_grid::Grid;

use crate::HostStore;

/// In-memory files, upper-case names like DOS.
pub struct MemFiles {
	/// The files by name.
	pub files: HashMap<String, Vec<u8>>,
	/// Folders that hold nothing yet, as `C:\DOCS` paths, for the file dialog.
	pub dirs: Vec<String>,
	/// The DOS date and time of a file, by its key, for the file dialog's Modified column.
	pub stamps: HashMap<String, (u16, u16)>,
	/// Files that are read-only, by key.
	pub readonly: Vec<String>,
	/// The time now, as a DOS packed date and time.
	pub clock: u32,
	/// When each file was last written.
	pub times: HashMap<String, u32>,
	/// Lock notes whose owner is provably alive.
	pub alive: Vec<String>,
	/// What a place is, by the start of its path (upper case); anything else is a fixed disk.
	pub drives: Vec<(String, Drive)>,
	/// The central recovery folders the shell offers, in order.
	pub recover_dirs: Vec<String>,
	/// Names that start with one of these cannot be created.
	pub denied: Vec<String>,
	/// Files only the file dialog sees, with their sizes: a way to fill a big folder without storing bytes.
	pub dummies: Vec<(String, u32)>,
	/// The file system the file dialog lists: `C:`, `D:` and `Z:`, with `files` and `dirs` on `C:`.
	tree: FakeFs,
}

impl Default for MemFiles {
	fn default() -> Self {
		let clock = dos_time(2026, 11, 25, 14, 32, 0);
		let mut stamps = HashMap::new();
		// The DPMI host QUILLDOS.EXE carries makes its swap file in the root of `C:` when it starts.
		stamps.insert(
			"C:\\CWSDPMI.SWP".to_string(),
			((clock >> 16) as u16, clock as u16),
		);
		MemFiles {
			files: HashMap::new(),
			dirs: Vec::new(),
			stamps,
			dummies: vec![("C:\\CWSDPMI.SWP".to_string(), 0)],
			clock,
			times: HashMap::new(),
			alive: Vec::new(),
			drives: Vec::new(),
			recover_dirs: vec!["RECOVER\\".to_string()],
			denied: Vec::new(),
			readonly: Vec::new(),
			tree: {
				// A real disk's folders all have a date; the clock is what a folder made now has.
				let mut t = FakeFs::dosbox();
				// DOSBox Staging 0.83 also gives the machine a `Y:` of its own.
				t.add_drive(b'Y', quill_dlg::fs::DriveKind::Fixed, false);
				t.dir_stamp = (0x5A21, 0);
				t.now = (0x5A21, 0);
				t
			},
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

/// A DOS date and time (the DTA's words) for `year-month-day hour:minute`.
pub fn dos_stamp(year: u16, month: u16, day: u16, hour: u16, minute: u16) -> (u16, u16) {
	(
		(year.saturating_sub(1980) << 9) | (month << 5) | day,
		(hour << 11) | (minute << 5),
	)
}

/// The path of a key as the file dialog's tree has it: names without a drive are on `C:`, at its root.
fn tree_path(key: &str) -> String {
	let key = key.to_ascii_uppercase();
	if key.as_bytes().get(1) == Some(&b':') {
		key
	} else {
		format!("C:\\{key}")
	}
}

impl MemFiles {
	/// Makes the tree the file dialog lists agree with the files: what was put in `files` since the last call shows up.
	fn sync(&mut self) {
		self.tree.now = ((self.clock >> 16) as u16, self.clock as u16);
		for (k, v) in &self.files {
			let path = tree_path(k);
			// A file written during the run has the time it was written; the others have their stamp.
			let (date, time) = match self.times.get(k) {
				Some(t) => ((t >> 16) as u16, *t as u16),
				None => self.stamps.get(k).copied().unwrap_or((0x5A21, 0)),
			};
			let attr = u8::from(self.readonly.contains(k));
			self.tree
				.add_file_info(&path, attr, v.len() as u32, date, time);
		}
		for d in &self.dirs {
			let path = tree_path(d);
			self.tree.add_dir(&path);
			if let Some((date, time)) = self.stamps.get(d) {
				self.tree
					.add_file_info(&path, quill_dlg::fs::ATTR_DIR, 0, *date, *time);
			}
		}
		for (k, size) in &self.dummies {
			let (date, time) = self.stamps.get(k).copied().unwrap_or((0x5A21, 0));
			self.tree.add_file_info(&tree_path(k), 0, *size, date, time);
		}
	}

	/// Sets the folder `C:` is in when the dialog opens with no last folder.
	pub fn set_cwd(&mut self, path: &str) {
		self.tree.set_cwd(b'C', path);
	}

	/// Gives the file dialog's tree a way to see folders and files made through the dialog itself.
	pub fn tree(&mut self) -> &mut FakeFs {
		self.sync();
		&mut self.tree
	}

	/// The key of a file by the name it is asked for: the name as typed, then without or with `C:\`.
	fn find(&self, name: &[u8]) -> Option<String> {
		let raw = key_name(name);
		if self.files.contains_key(&raw) {
			return Some(raw);
		}
		let other = match raw.strip_prefix("C:\\") {
			Some(rest) => rest.to_string(),
			None => format!("C:\\{raw}"),
		};
		self.files.contains_key(&other).then_some(other)
	}
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
		let key = self.find(name)?;
		self.files.get(&key).map(|b| MemSource(b.clone(), 0))
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

	fn fs(&mut self) -> Option<&mut dyn quill_dlg::fs::FileSystem> {
		Some(self.tree())
	}

	fn save_mode(&self) -> quill_app::SaveMode {
		quill_app::SaveMode::TempRename
	}

	fn remove(&mut self, name: &[u8]) -> bool {
		self.times.remove(&key_name(name));
		let Some(key) = self.find(name) else {
			return false;
		};
		use quill_dlg::fs::FileSystem as _;
		let _ = self.tree.remove(tree_path(&key).as_bytes());
		self.files.remove(&key).is_some()
	}

	/// Renames as DOS does: a name that is taken is an error.
	fn rename(&mut self, from: &[u8], to: &[u8]) -> bool {
		if self.find(to).is_some() {
			return false;
		}
		let Some(from_key) = self.find(from) else {
			return false;
		};
		use quill_dlg::fs::FileSystem as _;
		let _ = self.tree.remove(tree_path(&from_key).as_bytes());
		match self.files.remove(&from_key) {
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
		let key = self.find(name)?;
		self.files.get(&key).map(|b| b.len() as u32)
	}

	fn now(&mut self) -> u32 {
		self.clock
	}

	fn stamp(&mut self, name: &[u8]) -> Option<Stamp> {
		let n = self.find(name)?;
		let size = self.files.get(&n)?.len() as u32;
		let time = self.times.get(&n).copied().or_else(|| {
			self.stamps
				.get(&n)
				.map(|(date, time)| u32::from(*date) << 16 | u32::from(*time))
		});
		Some(Stamp {
			size,
			time: time.unwrap_or(0),
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
		let Some(d) = self.recover_dirs.get(usize::from(n)) else {
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

/// A key script: directives (`@mono`, `@rows N`, `@open NAME`, `@copy NAME`, `@live`, `@spell`), key tokens and pointer directives
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
	/// Test documents to copy into a folder: (name, folder with a closing backslash, such as `DOCS\\LETTERS\\`).
	pub copy_to: Vec<(String, String)>,
	/// Files that are read-only (`@readonly DOCS\\LETTER.QLD`).
	pub readonly: Vec<String>,
	/// Folders to make (`@dir DOCS\\LETTERS`).
	pub dirs: Vec<String>,
	/// Dates and times of files (`@stamp DOCS\\LETTER.QLD 2026-11-24 18:45`).
	pub stamps: Vec<(String, (u16, u16))>,
	/// Files only the file dialog lists (`@many BIG 2100 QLD`): a folder, how many and their extension.
	pub many: Vec<(String, u32, String)>,
	/// The folder the dialog opens in when there is no last one (`@cwd C:\\DOCS`).
	pub cwd: Option<String>,
	/// Drives to add (`@drive A removable notready`): the letter and the words after it.
	pub drives: Vec<(u8, String)>,
	/// Open, Save As and Insert Picture use the file dialog, as they do in the DOS build; `@prompt` brings the one-line name box back.
	pub dialog: bool,
	/// Folders for the dialog's places (`@place Samples C:\\SAMPLES`), at most two: Quill's own folder, which a script moves with
	/// `@place Quill`, and the samples.
	pub places: Vec<(String, String)>,
	/// Check Spelling as You Type is on, and the keyboard goes quiet after each key: the marks follow each frame.
	pub live: bool,
	/// `@spell`: the same, without the `marks:` lines under the frames, so a DOS run can match the golden screens (the marks are in
	/// the screens' attributes).
	pub marks_hidden: bool,
	/// The seed of a random idle schedule (`@sched seed=N`): idle passes of random sizes run after each step, and the frames and files
	/// must not change.
	pub sched: Option<u32>,
	/// `@recovery`: the recovery protocol is on, as the DOS screens run it, and the launch check has run before the first frame.
	pub recovery: bool,
}

/// Reads a script: tokens separated by white space, `#` starting a comment to the end of the line.
pub fn parse_script(text: &str) -> Script {
	let mut sc = Script {
		dialog: true,
		// The program is in the root of `C:`, as it is when it is run from a disk's root.
		places: vec![("Quill".to_string(), "C:\\".to_string())],
		..Script::default()
	};
	for line in text.lines().map(|l| l.split('#').next().unwrap_or("")) {
		let mut it = line.split_whitespace().peekable();
		while let Some(t) = it.next() {
			match t {
				"@mono" => sc.mono = true,
				"@live" => sc.live = true,
				"@spell" => {
					sc.live = true;
					sc.marks_hidden = true;
				}
				"@sched" => {
					sc.sched = it
						.next()
						.and_then(|t| t.strip_prefix("seed="))
						.and_then(|n| n.parse().ok());
					assert!(sc.sched.is_some(), "@sched takes seed=N");
				}
				"@rows" => {
					sc.rows = match it.next() {
						Some("25") => 25,
						Some("50") => 50,
						other => panic!("@rows takes 25 or 50, not {other:?}"),
					}
				}
				"@open" => sc.open = it.next().map(str::to_string),
				"@recovery" => sc.recovery = true,
				"@dialog" => sc.dialog = true,
				"@prompt" => sc.dialog = false,
				"@copy" => {
					let name = it.next().map(str::to_string);
					match it.next_if(|t| t.ends_with('\\')) {
						Some(dir) => sc.copy_to.extend(name.map(|n| (n, dir.to_string()))),
						None => sc.copy.extend(name),
					}
				}
				"@drive" => {
					let letter = it.next().and_then(|l| l.bytes().next());
					let mut words = String::new();
					while let Some(w) =
						it.next_if(|w| matches!(*w, "removable" | "cdrom" | "network" | "notready"))
					{
						words.push_str(w);
						words.push(' ');
					}
					sc.drives
						.extend(letter.map(|l| (l.to_ascii_uppercase(), words)));
				}
				"@dir" => sc.dirs.extend(it.next().map(str::to_string)),
				"@readonly" => sc.readonly.extend(it.next().map(str::to_string)),
				"@cwd" => sc.cwd = it.next().map(str::to_string),
				"@place" => {
					if let (Some(l), Some(p)) = (it.next(), it.next()) {
						sc.places.retain(|(k, _)| k != l);
						sc.places.push((l.to_string(), p.to_string()));
					}
				}
				"@stamp" => {
					let (path, day, time) = (it.next(), it.next(), it.next());
					let num = |t: &str, i: usize, sep: char| -> u16 {
						t.split(sep)
							.nth(i)
							.and_then(|v| v.parse().ok())
							.unwrap_or(0)
					};
					if let (Some(path), Some(day), Some(time)) = (path, day, time) {
						let stamp = dos_stamp(
							num(day, 0, '-'),
							num(day, 1, '-'),
							num(day, 2, '-'),
							num(time, 0, ':'),
							num(time, 1, ':'),
						);
						sc.stamps.push((path.to_string(), stamp));
					}
				}
				"@many" => {
					if let (Some(dir), Some(n), Some(ext)) = (it.next(), it.next(), it.next()) {
						sc.many
							.push((dir.to_string(), n.parse().unwrap_or(0), ext.to_string()));
					}
				}
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

/// Puts everything a script's directives name on the disk: its documents (from `docs`, the folder of test documents), its folders,
/// dates and the files only the file dialog sees.
pub fn stage(files: &mut MemFiles, script: &Script, docs: &std::path::Path) {
	let read = |name: &str| {
		std::fs::read(docs.join(name.to_ascii_lowercase()))
			.unwrap_or_else(|e| panic!("test document {name}: {e}"))
	};
	for f in &script.copy {
		files.files.insert(f.to_ascii_uppercase(), read(f));
	}
	for (f, dir) in &script.copy_to {
		let key = format!("C:\\{}{}", dir.to_ascii_uppercase(), f.to_ascii_uppercase());
		files.files.insert(key, read(f));
	}
	for r in &script.readonly {
		files
			.readonly
			.push(format!("C:\\{}", r.to_ascii_uppercase()));
	}
	for d in &script.dirs {
		files.dirs.push(format!("C:\\{}", d.to_ascii_uppercase()));
	}
	for (path, stamp) in &script.stamps {
		files
			.stamps
			.insert(format!("C:\\{}", path.to_ascii_uppercase()), *stamp);
	}
	for (dir, n, ext) in &script.many {
		for i in 0..*n {
			// Sizes and dates that vary without a pattern a sort could hide behind.
			let size = (i * 7919) % 90_000 + 1;
			let key = format!(
				"C:\\{}\\F{i:04}.{}",
				dir.to_ascii_uppercase(),
				ext.to_ascii_uppercase()
			);
			let (m, d) = ((i * 5) % 12 + 1, (i * 3) % 28 + 1);
			files.stamps.insert(
				key.clone(),
				dos_stamp(
					2021 + (i % 3) as u16,
					m as u16,
					d as u16,
					(i % 24) as u16,
					(i % 60) as u16,
				),
			);
			files.dummies.push((key, size));
		}
	}
	for (letter, words) in &script.drives {
		use quill_dlg::fs::DriveKind;
		let kind = if words.contains("cdrom") {
			DriveKind::CdRom
		} else if words.contains("network") {
			DriveKind::Network
		} else if words.contains("removable") {
			DriveKind::Removable
		} else {
			DriveKind::Fixed
		};
		files
			.tree()
			.add_drive(*letter, kind, words.contains("notready"));
	}
	if let Some(c) = &script.cwd {
		files.set_cwd(&c.to_ascii_uppercase());
	}
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
	if script.recovery {
		app.configure_recovery(10, 0, false);
		app.set_recovery(true);
		app.load_settings_with(&mut s, files);
	} else {
		app.load_settings(files);
	}
	app.set_about("Host test run");
	app.set_file_dialog(script.dialog);
	let places: Vec<(&[u8], &[u8])> = script
		.places
		.iter()
		.map(|(l, p)| (l.as_bytes(), p.as_bytes()))
		.collect();
	app.set_file_places(&places);
	app.set_rows(ROWS - 2);
	if script.mono {
		app.set_scheme(quill_app::Scheme::Mono);
	}
	if script.recovery {
		app.recovery_check_launch(&mut s, files);
		app.recovery_prompt(&mut s, files);
	}
	if let Some(name) = &script.open {
		app.open(&mut s, files, name.as_bytes()).unwrap();
	}
	let mut g = Grid::<ROWS>::new(0);
	let mut n = 1;
	quill_app::draw(&app, &s, &mut g);
	g.dump(n, frames).unwrap();
	let mut pointer = Pointer::default();
	let mut sim = script.sched.map(quill_job::Sim::new);
	// Seconds since the last input: `@quiet` adds to it, and everything the person does clears it.
	let mut quiet = 0u32;
	let target = TextCells {
		rows: (ROWS - 2) as i32,
	};
	let mut tick = 0;
	for step in &script.steps {
		match step {
			Step::Key(k) => {
				quiet = 0;
				app.key(&mut s, files, *k)
			}
			Step::Clock(secs) => files.clock = dos_add(files.clock, (*secs).max(0) as u32),
			Step::Quiet(secs) => {
				files.clock = dos_add(files.clock, (*secs).max(0) as u32);
				quiet = quiet.saturating_add((*secs).max(0) as u32);
			}
			other => {
				quiet = 0;
				other.play(&mut tick, |ev, ctx| {
					pointer.handle(&mut app, &mut s, files, &target, ev, ctx)
				})
			}
		}
		quill_app::draw(&app, &s, &mut g);
		n += 1;
		g.dump(n, frames).unwrap();
		if script.live {
			app.spell_view_rows(&s, ROWS - 2);
		}
		if let Some(sim) = sim.as_mut() {
			// A schedule slices idle time between the steps; the drain below reaches the same state.
			for _ in 0..sim.below(4) {
				let ctx = Idle {
					quiet,
					units: sim.budget(8).left(),
					busy: false,
				};
				app.idle(&mut s, files, ctx, &mut || false);
			}
		}
		if script.live {
			// The keyboard goes quiet: the whole document is on the screen and the scanner runs out of work.
			app.drain(&mut s, files, quiet);
			if !script.marks_hidden {
				let marks: Vec<String> = app
					.spell_marks(&s, 0, u32::MAX)
					.map(|(a, b)| format!("{a}-{b}"))
					.collect();
				let _ = writeln!(frames, "marks: {}", marks.join(" "));
			}
		}
		if app.quit() {
			break;
		}
	}
	if script.recovery {
		app.save_settings_with(&s, files);
	} else {
		app.save_settings(files);
	}
	let _ = writeln!(frames, "=== end ===");
}
