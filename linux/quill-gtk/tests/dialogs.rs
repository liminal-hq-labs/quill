// The dialogs answered through the real window: every `Dialog` variant but Print, the unsaved-changes question for every command
// that asks, and the keyboard (Alt mnemonics, Enter and Esc).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Each test starts its own Xvfb (`-displayfd`, so the number is free) and runs the `quill` binary on it with no session bus, no
// portal and no Wayland display; it stops only those two processes. A test that cannot start Xvfb or find `xdotool` says so and
// passes, so a machine with no X server still runs the rest of the suite (CI's job for these is `make gtk` on a runner that has
// them). `quill --self-test-dialogs` shows each dialog in the window, presses its buttons and answers the file dialogs and the
// question from a `ScriptChooser`, printing one line each; the tests read those lines and the files the run left, and compare the
// saved bytes with what a headless `Document` writes. The keyboard is driven with xdotool and checked from the lines the window
// prints when `--script-chooser` is given.

use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use quill_fonts::FontSet;
use quill_gtk::doc::Document;
use quill_gtk::files::GuiFiles;
use quill_tty::files::Dirs;

/// A private X server, stopped when dropped.
struct Xvfb {
	child: Child,
	display: String,
}

impl Xvfb {
	fn start() -> Option<Xvfb> {
		let mut child = Command::new("Xvfb")
			.args([
				"-displayfd",
				"1",
				"-screen",
				"0",
				"1280x900x24",
				"-nolisten",
				"tcp",
			])
			.stdin(Stdio::null())
			.stdout(Stdio::piped())
			.stderr(Stdio::null())
			.spawn()
			.ok()?;
		let mut line = String::new();
		BufReader::new(child.stdout.take()?)
			.read_line(&mut line)
			.ok()?;
		let n: u32 = line.trim().parse().ok()?;
		Some(Xvfb {
			child,
			display: format!(":{n}"),
		})
	}
}

impl Drop for Xvfb {
	fn drop(&mut self) {
		let _ = self.child.kill();
		let _ = self.child.wait();
	}
}

fn have_xdotool() -> bool {
	Command::new("xdotool")
		.arg("version")
		.stdout(Stdio::null())
		.stderr(Stdio::null())
		.status()
		.is_ok_and(|s| s.success())
}

fn docs() -> PathBuf {
	Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/docs")
}

/// `quill` with the environment a private display needs, run from `tests/docs` (the dictionary is there).
fn quill(x: &Xvfb, cfg: &Path, args: &[&str]) -> Command {
	let mut c = Command::new(env!("CARGO_BIN_EXE_quill"));
	c.current_dir(docs())
		.env_remove("WAYLAND_DISPLAY")
		.env_remove("DBUS_SESSION_BUS_ADDRESS")
		.env("DISPLAY", &x.display)
		.env("GDK_BACKEND", "x11")
		.env("GSK_RENDERER", "cairo")
		.env("GTK_USE_PORTAL", "0")
		.env("QUILL_NO_BLINK", "1")
		.env("XDG_CONFIG_HOME", cfg)
		.env("XDG_DATA_HOME", cfg)
		.args([
			"--new-instance",
			"--vendored-fonts",
			"--no-settings",
			"--size",
			"1000x700",
		])
		.args(args)
		.stdin(Stdio::null())
		.stderr(Stdio::null());
	c
}

/// Runs `c` to the end (killing it after `limit`) and returns what it printed.
fn output_of(mut c: Command, limit: Duration) -> String {
	let mut child = c.stdout(Stdio::piped()).spawn().unwrap();
	let mut out = child.stdout.take().unwrap();
	let reader = std::thread::spawn(move || {
		let mut s = String::new();
		let _ = out.read_to_string(&mut s);
		s
	});
	let end = Instant::now() + limit;
	while child.try_wait().unwrap().is_none() {
		if Instant::now() > end {
			let _ = child.kill();
			break;
		}
		std::thread::sleep(Duration::from_millis(50));
	}
	let _ = child.wait();
	reader.join().unwrap()
}

/// A headless document that has had `text` typed into it and been saved as `path`.
fn headless_save(dir: &Path, path: &Path, text: &str) -> Vec<u8> {
	let files = GuiFiles::new(Dirs {
		base: ".".into(),
		config: dir.join("cfg"),
		data: Vec::new(),
	});
	let mut doc = Document::new(FontSet::vendored(), files).unwrap();
	doc.type_text(text);
	doc.save_as(path).unwrap();
	std::fs::read(path).unwrap()
}

/// The first line of `out` that holds all of `parts`, or a failure that prints the whole output.
fn line<'a>(out: &'a str, parts: &[&str]) -> &'a str {
	out.lines()
		.find(|l| parts.iter().all(|p| l.contains(p)))
		.unwrap_or_else(|| panic!("no line with {parts:?} in:\n{out}"))
}

/// The self-test's output and the folder it kept its files in, or `None` when there is no X server.
fn self_test() -> Option<(String, tempfile::TempDir)> {
	let x = Xvfb::start()?;
	let keep = tempfile::tempdir().unwrap();
	let cfg = tempfile::tempdir().unwrap();
	let mut c = quill(&x, cfg.path(), &["--self-test-dialogs"]);
	c.env("QUILL_SELFTEST_DIR", keep.path());
	Some((output_of(c, Duration::from_secs(150)), keep))
}

/// The self-test is slow, so every check of its output is in one test.
#[test]
fn every_dialog_is_answered_through_the_window_and_reaches_the_document() {
	let Some((out, keep)) = self_test() else {
		eprintln!("dialogs: no Xvfb, so the window was not run");
		return;
	};
	let dir = keep.path();

	// About, Find, Replace, Fonts, Tabs and Spelling are shown and the answers change the document.
	line(&out, &["dialog about: shown"]);
	line(&out, &["dialog find: shown", "focus in control Some(10)"]);
	line(&out, &["dialog find button 100", "selection Some((0, 5))"]);
	line(
		&out,
		&["dialog find button 101", "text \"omega beta alpha\""],
	);
	line(&out, &["dialog find button 100", "not found"]);
	line(
		&out,
		&["dialog find button 100", "match case, nothing found"],
	);
	line(
		&out,
		&["dialog find button 100", "whole word, nothing found"],
	);
	line(
		&out,
		&["dialog find button 102", "text \"omega beta omega\""],
	);
	line(&out, &["dialog find button 2", "text \"omega beta omega\""]);
	line(&out, &["dialog replace: focus on the replacement is true"]);
	line(
		&out,
		&[
			"dialog fonts button 2",
			"cancelled",
			"size 24 half-points, bold false",
		],
	);
	line(
		&out,
		&[
			"dialog fonts button 1",
			"size 28 half-points, bold true, italic true, underline true",
		],
	);
	line(&out, &["dialog tabs button 20", "tab stops [(1440, 1, 1)]"]);
	line(
		&out,
		&["dialog tabs button 20", "[(1440, 1, 1), (3600, 1, 1)]"],
	);
	line(&out, &["dialog tabs button 22", "tab stops []"]);
	line(
		&out,
		&["dialog spelling button 102", "text \"the brown fox\""],
	);

	// Open: the types, the answer, Cancel and a file that is not there.
	line(
		&out,
		&[
			"dialog open: Open types [\"Quill documents (*.qld)\", \"Rich Text (*.rtf)\", \"Text (*.txt)\", \"All files\"]",
			"name Some(\"alpha.qld\"), text \"alpha text\"",
		],
	);
	line(
		&out,
		&[
			"dialog open button cancel",
			"name Some(\"alpha.qld\"), text \"alpha text\"",
		],
	);
	line(
		&out,
		&["dialog open missing file", "name Some(\"alpha.qld\")"],
	);

	// Save As: each type, a typed extension and no type, and what lands on disk is what a headless document writes.
	line(&out, &["dialog save-as type 1", "bare1.qld exists true"]);
	line(&out, &["dialog save-as type 2", "bare2.rtf exists true"]);
	line(&out, &["dialog save-as type 3", "bare3.txt exists true"]);
	line(
		&out,
		&[
			"dialog save-as typed extension kept",
			"typed.rtf exists true",
		],
	);
	line(
		&out,
		&["dialog save-as no type reported", "plain.qld exists true"],
	);
	line(
		&out,
		&["Save As types [\"Quill documents (*.qld)\", \"Rich Text (*.rtf)\", \"Text (*.txt)\"]"],
	);
	line(
		&out,
		&["dialog save-as button cancel", "name None, modified true"],
	);
	line(
		&out,
		&[
			"dialog save-as from Save",
			"named-by-save.qld exists true, modified false",
		],
	);
	let expected = tempfile::tempdir().unwrap();
	for leaf in [
		"bare1.qld",
		"bare2.rtf",
		"bare3.txt",
		"typed.rtf",
		"plain.qld",
	] {
		let want = headless_save(
			expected.path(),
			&expected.path().join(leaf),
			"Saved by the dialog.",
		);
		let got = std::fs::read(dir.join(leaf)).unwrap();
		assert_eq!(
			got, want,
			"{leaf}: the window's bytes differ from a headless document's"
		);
	}

	// Insert Picture: the types, one picture in, a file that is not one refused, Cancel.
	line(
		&out,
		&[
			"dialog picture: Insert Picture types [\"Pictures (*.bmp, *.png)\", \"Bitmaps (*.bmp)\", \"PNG pictures (*.png)\"]",
			"pictures 0 -> 1",
		],
	);
	line(&out, &["dialog picture not a picture", "pictures 1 -> 1"]);
	line(&out, &["dialog picture button cancel", "pictures 1"]);

	// The unsaved-changes question: each asking command with each answer.
	for label in ["New", "Open", "Recent", "drop", "hand-off"] {
		let stays = line(&out, &[&format!("question {label} Cancel:")]);
		assert!(
			stays.contains("text \"subject text changed\", modified true"),
			"{stays}"
		);
		assert!(
			stays.contains("file is 280 bytes"),
			"Cancel must leave the file alone: {stays}"
		);
		let lost = line(&out, &[&format!("question {label} Discard:")]);
		assert!(lost.contains("modified false"), "{lost}");
		assert!(
			lost.contains("file is 280 bytes"),
			"Don't Save must leave the file alone: {lost}"
		);
		let saved = line(&out, &[&format!("question {label} Save:")]);
		assert!(saved.contains("modified false"), "{saved}");
		assert!(
			saved.contains("file is 288 bytes"),
			"Save must write the changes: {saved}"
		);
	}
	line(&out, &["question New Discard:", "name None, text \"\""]);
	line(
		&out,
		&[
			"question Open Discard:",
			"name Some(\"target.qld\"), text \"target text\"",
		],
	);
	line(
		&out,
		&[
			"question Open Save:",
			"name Some(\"target.qld\"), text \"target text\"",
		],
	);
	line(&out, &["question drop Save:", "name Some(\"target.qld\")"]);
	line(
		&out,
		&["question hand-off Discard:", "name Some(\"target.qld\")"],
	);
	line(
		&out,
		&[
			"question Open Discard then Cancel in Open:",
			"name Some(\"subject.qld\")",
			"modified true",
		],
	);
	line(
		&out,
		&[
			"question New Save with no name:",
			"from-question.qld exists true",
		],
	);
	line(
		&out,
		&[
			"question New Save then Cancel in Save As:",
			"text \"Unnamed work\", modified true",
		],
	);
	assert!(dir.join("from-question.qld").is_file());
	for (what, answer, closed) in [
		("close window", "Cancel", "closed false"),
		("close window", "Discard", "closed true"),
		("close window", "Save", "closed true"),
		("Exit", "Cancel", "closed false"),
		("Exit", "Save", "closed true"),
		("Exit", "Discard", "closed true"),
	] {
		line(&out, &[&format!("question {what} {answer}:"), closed]);
	}
	// Save keeps the changes on disk; Cancel and Don't Save close or keep without writing.
	assert!(std::fs::read(dir.join("closing.qld")).unwrap().len() > 280);
}

/// A window with a script of no answers, and a way to type at it.
struct Window {
	app: Child,
	lines: std::sync::mpsc::Receiver<String>,
	seen: Vec<String>,
	display: String,
}

impl Window {
	fn start(x: &Xvfb, cfg: &Path, script: &Path, file: &str) -> Window {
		let mut c = quill(
			x,
			cfg,
			&[
				"--quit-after",
				"60000",
				"--script-chooser",
				script.to_str().unwrap(),
				file,
			],
		);
		let mut app = c.stdout(Stdio::piped()).spawn().unwrap();
		let out = app.stdout.take().unwrap();
		let (tx, rx) = std::sync::mpsc::channel();
		std::thread::spawn(move || {
			for l in BufReader::new(out).lines().map_while(Result::ok) {
				if tx.send(l).is_err() {
					break;
				}
			}
		});
		let mut w = Window {
			app,
			lines: rx,
			seen: Vec::new(),
			display: x.display.clone(),
		};
		// The window prints that it opened the file once it is up.
		w.wait_for("opened");
		std::thread::sleep(Duration::from_millis(800));
		w
	}

	/// Waits for a line containing `text` and returns it.
	fn wait_for(&mut self, text: &str) -> String {
		let end = Instant::now() + Duration::from_secs(15);
		loop {
			if let Some(l) = self.seen.iter().find(|l| l.contains(text)) {
				return l.clone();
			}
			let left = end.saturating_duration_since(Instant::now());
			match self.lines.recv_timeout(left) {
				Ok(l) => self.seen.push(l),
				Err(_) => panic!("no line with {text:?} in:\n{}", self.seen.join("\n")),
			}
		}
	}

	/// Forgets what has been printed, so the next wait sees only what follows.
	fn forget(&mut self) {
		while let Ok(l) = self.lines.try_recv() {
			self.seen.push(l);
		}
		self.seen.clear();
	}

	fn key(&self, keys: &[&str]) {
		let ok = Command::new("xdotool")
			.env("DISPLAY", &self.display)
			.arg("key")
			.args(["--delay", "120"])
			.args(keys)
			.status()
			.unwrap();
		assert!(ok.success());
		std::thread::sleep(Duration::from_millis(500));
	}
}

impl Drop for Window {
	fn drop(&mut self) {
		let _ = self.app.kill();
		let _ = self.app.wait();
	}
}

/// Starts a window on a private display, or says why not.
fn window(script: &str, file: &str) -> Option<(Xvfb, tempfile::TempDir, Window)> {
	if !have_xdotool() {
		eprintln!("dialogs: no xdotool, so the keys were not driven");
		return None;
	}
	let x = Xvfb::start().or_else(|| {
		eprintln!("dialogs: no Xvfb, so the window was not run");
		None
	})?;
	let dir = tempfile::tempdir().unwrap();
	let script_file = dir.path().join("script.txt");
	std::fs::write(&script_file, script).unwrap();
	let w = Window::start(&x, dir.path(), &script_file, file);
	Some((x, dir, w))
}

#[test]
fn alt_f_opens_the_file_menu_and_o_runs_open() {
	let target = docs().join("letter.qld");
	let Some((_x, _dir, mut w)) = window(&format!("open {}\n", target.display()), "plain.qld")
	else {
		return;
	};
	w.forget();
	w.key(&["alt+f"]);
	w.key(&["o"]);
	w.wait_for("dialog Open: asked");
	w.wait_for("chooser open: answered");
	w.wait_for("opened");
}

#[test]
fn alt_letters_enter_and_esc_work_in_the_find_dialog() {
	let Some((_x, _dir, mut w)) = window("", "letter.rtf") else {
		return;
	};
	w.forget();
	w.key(&["ctrl+f"]);
	w.wait_for("dialog Find(false): asked");
	w.wait_for("dialog find focus: control 10");
	// Alt+C is Match case, Alt+W Whole word (the labels' own letters).
	w.key(&["alt+c"]);
	assert!(w.wait_for("dialog find control 12").ends_with("on"));
	w.key(&["alt+c"]);
	w.wait_for("dialog find control 12: off");
	w.key(&["alt+w"]);
	w.wait_for("dialog find control 13: on");
	// Alt+P puts the focus in Replace with, Alt+N back in Find what.
	w.key(&["alt+p"]);
	w.wait_for("dialog find focus: control 11");
	w.key(&["alt+n"]);
	w.wait_for("dialog find focus: control 10");
	// Tab goes on to the next control in the description's order: Find what, Replace with.
	w.key(&["Tab"]);
	w.wait_for("dialog find focus: control 11");
	w.key(&["alt+n"]);
	w.wait_for("dialog find focus: control 10");
	// Enter presses the default button, Find Next; Esc is Cancel.
	w.key(&["Return"]);
	w.wait_for("dialog find button 100: pressed");
	w.key(&["Escape"]);
	w.wait_for("dialog find: closed");
}

#[test]
fn alt_letters_enter_and_esc_work_in_the_fonts_dialog() {
	let Some((_x, _dir, mut w)) = window("", "letter.rtf") else {
		return;
	};
	w.forget();
	w.key(&["ctrl+a"]);
	w.key(&["alt+c"]);
	w.key(&["f"]);
	w.wait_for("dialog Fonts: asked");
	w.key(&["alt+b"]);
	w.wait_for("dialog fonts control 15: on");
	w.key(&["alt+i"]);
	w.wait_for("dialog fonts control 16: on");
	// Enter is OK; the dialog closes.
	w.key(&["Return"]);
	w.wait_for("dialog fonts button 1: pressed");
	w.wait_for("dialog fonts: closed");
	w.forget();
	w.key(&["alt+c"]);
	w.key(&["f"]);
	w.wait_for("dialog Fonts: asked");
	w.key(&["Escape"]);
	w.wait_for("dialog fonts: closed");
}
