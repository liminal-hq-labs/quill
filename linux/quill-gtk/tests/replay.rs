// The host's key scripts, replayed through the GTK command layer, must save the host's bytes.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Each key of a script in `tests/scenarios` is turned into the GTK key press that gives it (`keys::events_for`), mapped back by
// `keys::map_key` (what the window's key controller does), and handed to a `Document` (what the window does with it): a
// character as the input method's commit (`type_text`), any other key as the editor's own. The files the scripts save are compared
// with `tests/golden/files`, byte for byte. A script that drives the editor's text menus and prompts (File > Open, Save As) runs
// with the shell's own dialogs off, as the Windows builds' golden runs do; the window turns them on.

use std::fs;
use std::path::{Path, PathBuf};

use quill_app::App;
use quill_fonts::FontSet;
use quill_gtk::command;
use quill_gtk::doc::Document;
use quill_gtk::files::GuiFiles;
use quill_gtk::keys::{events_for, map_key, Mapped};
use quill_host::script::parse_script;
use quill_host::HostStore;
use quill_tty::files::Dirs;

fn root() -> PathBuf {
	PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn find_any_case(dir: &Path, name: &str) -> Option<PathBuf> {
	fs::read_dir(dir)
		.ok()?
		.filter_map(Result::ok)
		.map(|e| e.path())
		.find(|p| {
			p.file_name()
				.and_then(|n| n.to_str())
				.is_some_and(|n| n.eq_ignore_ascii_case(name))
		})
}

/// A fresh folder holding the documents a script copies, and the files over it.
fn folder(sc: &quill_host::script::Script) -> (tempfile::TempDir, GuiFiles) {
	let t = tempfile::tempdir().unwrap();
	let work = t.path().join("work");
	fs::create_dir(&work).unwrap();
	for f in &sc.copy {
		let lower = f.to_ascii_lowercase();
		fs::copy(root().join("tests/docs").join(&lower), work.join(&lower)).unwrap();
	}
	let files = GuiFiles::new(Dirs {
		base: work,
		config: t.path().join("cfg/quill"),
		data: Vec::new(),
	});
	(t, files)
}

/// Sends a key through the GTK key press that gives it and the mapping the window's key controller uses.
fn mapped(k: quill_app::Key) -> Mapped {
	let (key, state) = events_for(k).unwrap_or_else(|| panic!("{k:?} has no key press"));
	let m = map_key(key, state);
	match m {
		Mapped::Edit(got) => assert_eq!(got, k, "{k:?} changed in the mapping"),
		Mapped::Text(_) => {}
		other => panic!("{k:?} mapped to {other:?}"),
	}
	m
}

/// Plays the script `name` through the command layer over the text grid the host's goldens were made on, and returns the folder.
fn replay_on_the_grid(name: &str) -> tempfile::TempDir {
	let text = fs::read_to_string(root().join(format!("tests/scenarios/{name}.keys"))).unwrap();
	let sc = parse_script(&text);
	let (t, mut files) = folder(&sc);
	let mut store = HostStore::new();
	let mut app = Box::new(App::new());
	app.reset(&mut store).unwrap();
	app.set_spell_live_default(sc.live);
	app.set_about("Host test run");
	app.set_rows(if sc.rows == 50 { 48 } else { 23 });
	if sc.mono {
		app.set_scheme(quill_app::Scheme::Mono);
	}
	if let Some(open) = &sc.open {
		app.open(&mut store, &mut files, open.as_bytes()).unwrap();
	}
	for k in &sc.keys {
		command::apply(&mut app, &mut store, &mut files, mapped(*k));
		if app.quit() {
			break;
		}
	}
	t
}

/// Plays the script through a `Document`: the window's own layout (outline fonts, 96 pixels to the inch).
fn replay_in_a_document(name: &str) -> tempfile::TempDir {
	let text = fs::read_to_string(root().join(format!("tests/scenarios/{name}.keys"))).unwrap();
	let sc = parse_script(&text);
	let (t, files) = folder(&sc);
	let mut doc = Document::new(FontSet::vendored(), files).unwrap();
	doc.set_native_dialogs(false);
	if let Some(open) = &sc.open {
		let name = open.as_bytes().to_vec();
		doc.with(|app, s, f| app.open(s, f, &name)).unwrap();
	}
	for k in &sc.keys {
		let out = doc.apply(mapped(*k));
		assert!(out.typed.refused.is_empty());
		if doc.quit_requested() {
			break;
		}
	}
	t
}

/// Each script and the files it leaves behind: (name in the folder, golden).
const RUNS: &[(&str, &[(&str, &str)])] = &[
	(
		"m1-edit",
		&[("plain.qld", "tests/golden/files/m1-edit.qld")],
	),
	(
		"m2-letter",
		&[("out.rtf", "tests/golden/files/m2-letter.rtf")],
	),
	(
		"m7-picture",
		&[("out.qld", "tests/golden/files/m7-picture.qld")],
	),
	("m2-lists", &[]),
	("m2-find", &[]),
	("m12-confirm", &[]),
];

fn check(t: &tempfile::TempDir, name: &str, outputs: &[(&str, &str)]) {
	for (file, golden) in outputs {
		let path = find_any_case(&t.path().join("work"), file)
			.unwrap_or_else(|| panic!("{name}: {file} was not saved"));
		let want = fs::read(root().join(golden)).unwrap();
		assert!(
			fs::read(&path).unwrap() == want,
			"{name}: {file} differs from {golden}"
		);
	}
}

#[test]
fn every_editing_script_saves_the_hosts_bytes_over_the_text_grid() {
	for (name, outputs) in RUNS {
		check(&replay_on_the_grid(name), name, outputs);
	}
}

/// The scripts whose saved bytes do not depend on where Down and Page Down go, so they hold under the window's own layout too.
const LAYOUT_FREE: &[&str] = &[
	"m1-edit",
	"m7-picture",
	"m2-lists",
	"m2-find",
	"m12-confirm",
];

#[test]
fn the_layout_free_scripts_save_the_same_bytes_in_a_document() {
	for (name, outputs) in RUNS.iter().filter(|(n, _)| LAYOUT_FREE.contains(n)) {
		check(&replay_in_a_document(name), name, outputs);
	}
}
