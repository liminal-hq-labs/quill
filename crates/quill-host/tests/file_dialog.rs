// The file dialog of the text screen, played through key scripts: its screens against the goldens in `tests/golden/screens`, and what it
// hands back to the editor against what the same names do through the native dialogs.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::PathBuf;

use quill_host::script::{parse_script, play, stage, MemFiles};

fn root() -> PathBuf {
	PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Plays `tests/scenarios/NAME.keys` and returns the files and the frames.
fn scenario(name: &str) -> (MemFiles, String) {
	let text =
		std::fs::read_to_string(root().join(format!("tests/scenarios/{name}.keys"))).unwrap();
	let sc = parse_script(&text);
	let mut files = MemFiles::default();
	stage(&mut files, &sc, &root().join("tests/docs"));
	let mut frames = String::new();
	play(&mut files, &sc, &mut frames);
	(files, frames)
}

fn compare(name: &str, frames: &str) {
	let path = root().join(format!("tests/golden/screens/{name}.frames.txt"));
	if std::env::var_os("QUILL_UPDATE_GOLDEN").is_some() {
		std::fs::write(&path, frames).unwrap();
		return;
	}
	let want = std::fs::read_to_string(&path)
		.unwrap_or_else(|_| panic!("missing golden for {name}; run with QUILL_UPDATE_GOLDEN=1"));
	assert_eq!(want, frames, "{name}: the screens differ");
}

/// The screens of each scenario match their goldens.
macro_rules! screens {
	($($test:ident => $name:literal,)*) => {
		$(#[test] fn $test() { let (_, frames) = scenario($name); compare($name, &frames); })*
	};
}

screens! {
	m16_open_screens => "m16-open",
	m16_open_50_screens => "m16-open-50",
	m16_open_mono_screens => "m16-open-mono",
	m16_save_as_screens => "m16-save-as",
	m16_errors_screens => "m16-errors",
	m16_errors_drive_screens => "m16-errors-drive",
	m16_big_screens => "m16-big",
}

// ----- what the dialog hands back to the editor -----

use std::collections::BTreeMap;

use quill_app::{App, Command, Files, Key};
use quill_host::flow::Disk;
use quill_host::HostStore;

/// An editor with the file dialog on, over files that hold `docs` (name in `tests/docs`, folder with a closing backslash) and `dirs`.
struct Rig {
	app: App,
	store: HostStore,
	files: MemFiles,
}

impl Rig {
	fn new(docs: &[(&str, &str)], dirs: &[&str]) -> Rig {
		let mut store = HostStore::new();
		let mut app = App::new();
		app.reset(&mut store).unwrap();
		app.set_file_dialog(true);
		let mut files = MemFiles::default();
		for (name, dir) in docs {
			let key = format!("C:\\{dir}{}", name.to_ascii_uppercase());
			let bytes = std::fs::read(root().join(format!("tests/docs/{name}"))).unwrap();
			files.files.insert(key, bytes);
		}
		for d in dirs {
			files.dirs.push(format!("C:\\{d}"));
		}
		Rig { app, store, files }
	}

	fn key(&mut self, k: Key) {
		self.app.key(&mut self.store, &mut self.files, k);
	}

	fn command(&mut self, c: Command) {
		self.app.command(&mut self.store, &mut self.files, c);
	}

	fn type_text(&mut self, s: &str) {
		for b in s.bytes() {
			self.key(Key::Char(b));
		}
	}

	/// Types `name` into the dialog's File name field, over what was there, and presses Enter.
	fn enter_name(&mut self, name: &str) {
		self.key(Key::Alt(b'n'));
		for _ in 0..80 {
			self.key(Key::Backspace);
		}
		self.type_text(name);
		self.key(Key::Enter);
	}

	fn snapshot(&self) -> Snapshot {
		Snapshot {
			name: self.app.name().to_vec(),
			status: self.app.message().to_vec(),
			recent: self.app.recent_files().map(<[u8]>::to_vec).collect(),
			folder: self.app.last_folder().to_vec(),
			modified: self.app.modified(),
			files: self
				.files
				.files
				.iter()
				.map(|(k, v)| (k.clone(), v.clone()))
				.collect(),
		}
	}
}

#[derive(Debug, PartialEq, Eq)]
struct Snapshot {
	name: Vec<u8>,
	status: Vec<u8>,
	recent: Vec<Vec<u8>>,
	folder: Vec<u8>,
	modified: bool,
	files: BTreeMap<String, Vec<u8>>,
}

/// The same choice made through the native dialogs' entry points, for comparison.
fn native(
	docs: &[(&str, &str)],
	dirs: &[&str],
	act: impl Fn(&mut App, &mut HostStore, &mut MemFiles),
) -> Snapshot {
	let mut r = Rig::new(docs, dirs);
	r.app.set_file_dialog(false);
	act(&mut r.app, &mut r.store, &mut r.files);
	r.snapshot()
}

#[test]
fn open_through_the_dialog_is_what_the_native_dialog_does() {
	let docs = [("letter.qld", "DOCS\\")];
	let mut r = Rig::new(&docs, &["DOCS"]);
	r.command(Command::Open);
	assert!(r.app.file_dialog_open());
	r.enter_name("c:\\docs\\letter.qld");
	assert!(!r.app.file_dialog_open());
	let want = native(&docs, &["DOCS"], |a, s, f| {
		a.open_chosen(s, f, b"C:\\DOCS\\LETTER.QLD").unwrap();
	});
	assert_eq!(r.snapshot(), want);
	assert_eq!(r.app.name(), b"C:\\DOCS\\LETTER.QLD");
	assert_eq!(r.app.last_folder(), b"C:\\DOCS\\");
}

#[test]
fn save_as_through_the_dialog_is_what_the_native_dialog_does_for_each_type() {
	for (path, text) in [
		("C:\\A\\ONE.TXT", "one.txt"),
		("C:\\B\\TWO.QLD", "two.qld"),
		("C:\\B\\THREE.RTF", "three.rtf"),
	] {
		let mut r = Rig::new(&[], &["A", "B"]);
		r.type_text("words");
		r.command(Command::SaveAs);
		assert!(r.app.file_dialog_open());
		r.enter_name(&format!(
			"{}{}",
			&path[..path.rfind('\\').unwrap() + 1],
			text
		));
		assert!(!r.app.file_dialog_open(), "{path}");
		let want = native(&[], &["A", "B"], |a, s, f| {
			a.key(s, f, Key::Char(b'w'));
			for b in b"ords" {
				a.key(s, f, Key::Char(*b));
			}
			a.save_as_chosen(s, f, path.as_bytes()).unwrap();
		});
		assert_eq!(r.snapshot(), want, "{path}");
	}
}

#[test]
fn a_typed_extension_decides_the_format_and_a_bare_name_takes_the_types() {
	let mut r = Rig::new(&[], &["A"]);
	r.type_text("hi");
	r.command(Command::SaveAs);
	// The type box says Quill documents; the name says RTF.
	r.enter_name("C:\\A\\ODD.RTF");
	assert!(r.files.files["C:\\A\\ODD.RTF"].starts_with(b"{\\rtf"));
	// No extension: the type's (Save As starts with the type of the name the document has, RTF now).
	r.command(Command::SaveAs);
	r.enter_name("C:\\A\\BARE");
	assert!(r.files.files["C:\\A\\BARE.RTF"].starts_with(b"{\\rtf"));
	// A name whose extension is not Quill's falls back on the type: text here.
	r.command(Command::SaveAs);
	r.key(Key::Alt(b't'));
	r.key(Key::Down);
	r.enter_name("C:\\A\\NOTES.LOG");
	assert_eq!(r.files.files["C:\\A\\NOTES.LOG"], b"hi\r\n");
}

#[test]
fn picture_through_the_dialog_inserts_the_picture() {
	let docs = [("pic.bmp", "")];
	let mut r = Rig::new(&docs, &[]);
	r.command(Command::Picture);
	assert!(r.app.file_dialog_open());
	r.enter_name("pic.bmp");
	assert!(!r.app.file_dialog_open());
	let want = native(&docs, &[], |a, s, f| {
		a.picture_chosen(s, f, b"C:\\PIC.BMP").unwrap();
	});
	assert_eq!(r.snapshot(), want);
	assert!(r.app.modified());
}

#[test]
fn cancelling_leaves_the_editor_as_it_was_and_drops_what_waited_on_it() {
	let docs = [("letter.qld", "")];
	let mut r = Rig::new(&docs, &[]);
	r.type_text("x");
	r.command(Command::Open);
	// Unsaved changes: the question comes first, in the text screen's own box.
	r.key(Key::Char(b'n'));
	assert!(r.app.file_dialog_open());
	r.key(Key::Esc);
	assert!(!r.app.file_dialog_open());
	assert!(r.app.modified());
	assert_eq!(r.app.name(), b"");
	// Esc with a message box open closes the box and not the dialog.
	r.command(Command::Open);
	r.enter_name("gone.qld");
	assert!(r.app.file_dialog_open());
	r.key(Key::Esc);
	assert!(r.app.file_dialog_open());
	r.key(Key::Esc);
	assert!(!r.app.file_dialog_open());
}

#[test]
fn a_failed_save_keeps_the_name_and_the_format_and_says_so() {
	let mut r = Rig::new(&[], &["A"]);
	r.type_text("hi");
	r.command(Command::SaveAs);
	r.enter_name("C:\\A\\ONE.QLD");
	assert!(!r.app.modified());
	assert_eq!(r.app.name(), b"C:\\A\\ONE.QLD");
	// A file system that refuses the write: the dialog hands the name over and the editor reports the failure on the status line.
	let mut disk = Disk::default();
	disk.inner.dirs.push("C:\\A".into());
	disk.locked.push("C:\\A\\TWO.RTF".into());
	let mut store = HostStore::new();
	let mut app = App::new();
	app.reset(&mut store).unwrap();
	app.set_file_dialog(true);
	// `Disk` has no file system to list, so the name prompt stays.
	app.command(&mut store, &mut disk, Command::SaveAs);
	assert!(!app.file_dialog_open());
}

#[test]
fn the_dialog_is_off_until_the_shell_asks_and_needs_a_file_system() {
	let mut store = HostStore::new();
	let mut app = App::new();
	app.reset(&mut store).unwrap();
	let mut files = MemFiles::default();
	app.command(&mut store, &mut files, Command::Open);
	assert!(!app.file_dialog_open());
	app.key(&mut store, &mut files, Key::Esc);
	app.set_file_dialog(true);
	app.command(&mut store, &mut files, Command::Open);
	assert!(app.file_dialog_open());
	app.key(&mut store, &mut files, Key::Esc);
	assert!(files.fs().is_some());
}

#[test]
fn a_drive_that_is_not_ready_says_so_and_retry_asks_again() {
	let mut r = Rig::new(&[], &[]);
	r.files
		.tree()
		.add_drive(b'A', quill_dlg::fs::DriveKind::Removable, true);
	r.command(Command::Open);
	r.key(Key::Alt(b'p'));
	r.key(Key::Home);
	r.key(Key::Enter);
	assert!(r.app.file_dialog_open());
	// A box over the dialog; Retry asks the drive again and it still is not ready.
	r.key(Key::Char(b'r'));
	// The disk goes in: Retry now works and the dialog is on A:.
	r.files
		.tree()
		.add_drive(b'A', quill_dlg::fs::DriveKind::Removable, false);
	r.files.tree().add_dir("A:\\DISK");
	r.key(Key::Char(b'r'));
	r.key(Key::Esc);
	assert!(!r.app.file_dialog_open());
}
