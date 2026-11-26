// Key scripts: the same typed keys the DOS end-to-end run sends, played through the app on the host.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::HashMap;
use std::fmt::Write as _;

use quill_app::mouse::{Pointer, TextCells};
use quill_app::script::{parse_pointer, Step};
use quill_app::{App, Files, Key};
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
	/// Files only the file dialog sees, with their sizes: a way to fill a big folder without storing bytes.
	pub dummies: Vec<(String, u32)>,
	/// The file system the file dialog lists: `C:`, `D:` and `Z:`, with `files` and `dirs` on `C:`.
	tree: FakeFs,
}

impl Default for MemFiles {
	fn default() -> Self {
		MemFiles {
			files: HashMap::new(),
			dirs: Vec::new(),
			stamps: HashMap::new(),
			dummies: Vec::new(),
			readonly: Vec::new(),
			tree: FakeFs::dosbox(),
		}
	}
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
		for (k, v) in &self.files {
			let path = tree_path(k);
			let (date, time) = self.stamps.get(k).copied().unwrap_or((0x5A21, 0));
			let attr = u8::from(self.readonly.contains(k));
			self.tree
				.add_file_info(&path, attr, v.len() as u32, date, time);
		}
		for d in &self.dirs {
			self.tree.add_dir(&tree_path(d));
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
		Some(NamedSink(key_name(name), Vec::new()))
	}

	fn close(&mut self, w: Self::W) -> bool {
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
	/// Open, Save As and Insert Picture use the file dialog (`@dialog`).
	pub dialog: bool,
	/// Folders for the dialog's places (`@place Samples C:\\SAMPLES`), at most two.
	pub places: Vec<(String, String)>,
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
				"@dialog" => sc.dialog = true,
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
					2024 + (i % 3) as u16,
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
	app.load_settings(files);
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
