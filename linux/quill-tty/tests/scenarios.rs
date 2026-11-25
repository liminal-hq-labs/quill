// The host's key scripts, played through the terminal client's key mapper, must give the host's screens and files.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Each key of a script is turned into the event a terminal delivers for it, mapped back by `map_key`, and handed to a `Session`.
// The frames are compared with `tests/golden/screens` and the files with `tests/golden/files`, byte for byte, over the host's
// in-memory files and over real files with the safe save.

use std::fs;
use std::path::{Path, PathBuf};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use quill_app::{Dir, Files, Key};
use quill_host::script::{parse_script, MemFiles, Script};
use quill_tty::{cp1252, map_key, Dirs, LinuxFiles, Options, Session};

fn root() -> PathBuf {
	PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The event a terminal delivers when the user presses `k`.
fn typed(k: Key) -> KeyEvent {
	let n = KeyModifiers::NONE;
	let sel = |code| KeyEvent::new(code, KeyModifiers::SHIFT);
	match k {
		Key::Char(b) => KeyEvent::new(KeyCode::Char(cp1252::decode(b)), n),
		Key::Enter => KeyEvent::new(KeyCode::Enter, n),
		Key::Backspace => KeyEvent::new(KeyCode::Backspace, n),
		Key::Delete => KeyEvent::new(KeyCode::Delete, n),
		Key::Esc => KeyEvent::new(KeyCode::Esc, n),
		Key::Tab => KeyEvent::new(KeyCode::Tab, n),
		Key::BackTab => KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT),
		Key::Left => KeyEvent::new(KeyCode::Left, n),
		Key::Right => KeyEvent::new(KeyCode::Right, n),
		Key::Up => KeyEvent::new(KeyCode::Up, n),
		Key::Down => KeyEvent::new(KeyCode::Down, n),
		Key::Home => KeyEvent::new(KeyCode::Home, n),
		Key::End => KeyEvent::new(KeyCode::End, n),
		Key::PageUp => KeyEvent::new(KeyCode::PageUp, n),
		Key::PageDown => KeyEvent::new(KeyCode::PageDown, n),
		Key::CtrlHome => KeyEvent::new(KeyCode::Home, KeyModifiers::CONTROL),
		Key::CtrlEnd => KeyEvent::new(KeyCode::End, KeyModifiers::CONTROL),
		Key::Select(d) => sel(match d {
			Dir::Left => KeyCode::Left,
			Dir::Right => KeyCode::Right,
			Dir::Up => KeyCode::Up,
			Dir::Down => KeyCode::Down,
			Dir::Home => KeyCode::Home,
			Dir::End => KeyCode::End,
			Dir::PageUp => KeyCode::PageUp,
			Dir::PageDown => KeyCode::PageDown,
		}),
		Key::Insert => KeyEvent::new(KeyCode::Insert, n),
		Key::F(i) => KeyEvent::new(KeyCode::F(i), n),
		Key::Ctrl(c) => KeyEvent::new(KeyCode::Char(char::from(c)), KeyModifiers::CONTROL),
		Key::Alt(8) => KeyEvent::new(KeyCode::Backspace, KeyModifiers::ALT),
		Key::Alt(c) => KeyEvent::new(KeyCode::Char(char::from(c)), KeyModifiers::ALT),
	}
}

#[test]
fn every_key_the_editor_knows_survives_the_terminal() {
	let mut keys = vec![
		Key::Enter,
		Key::Backspace,
		Key::Delete,
		Key::Esc,
		Key::Tab,
		Key::Left,
		Key::Right,
		Key::Up,
		Key::Down,
		Key::Home,
		Key::End,
		Key::PageUp,
		Key::PageDown,
		Key::CtrlHome,
		Key::CtrlEnd,
		Key::Insert,
	];
	keys.extend(
		(0x20..=0xFFu8)
			.filter(|b| *b != 0x7F && !matches!(b, 0x81 | 0x8D | 0x8F | 0x90 | 0x9D))
			.map(Key::Char),
	);
	keys.extend((1..=12).map(Key::F));
	keys.extend((b'a'..=b'z').map(Key::Ctrl));
	for d in [
		Dir::Left,
		Dir::Right,
		Dir::Up,
		Dir::Down,
		Dir::Home,
		Dir::End,
		Dir::PageUp,
		Dir::PageDown,
	] {
		keys.push(Key::Select(d));
	}
	for k in keys {
		assert_eq!(map_key(&typed(k)), Some(k), "{k:?}");
	}
}

/// The lower-case names the host's scripts copy from `tests/docs`.
fn copy_docs(sc: &Script) -> Vec<(String, Vec<u8>)> {
	sc.copy
		.iter()
		.map(|f| {
			let name = f.to_ascii_lowercase();
			let bytes = fs::read(root().join(format!("tests/docs/{name}"))).unwrap();
			(name, bytes)
		})
		.collect()
}

/// Plays `sc` and returns the frames, as `quill_host::script::play` writes them.
fn play<F: Files>(files: F, sc: &Script) -> (String, F) {
	let opts = Options {
		about: "Host test run".into(),
		mono: sc.mono,
		open: sc.open.as_ref().map(|n| n.as_bytes().to_vec()),
		..Options::default()
	};
	let mut s = Session::new(files, &opts).unwrap();
	let mut frames = String::new();
	let mut n = 1;
	s.screen().dump(n, &mut frames);
	for k in &sc.keys {
		let k = map_key(&typed(*k)).unwrap_or_else(|| panic!("{k:?} did not map"));
		s.key(k);
		n += 1;
		s.screen().dump(n, &mut frames);
		if s.quit() {
			break;
		}
	}
	s.finish();
	frames.push_str("=== end ===\n");
	(frames, into_files(s))
}

fn into_files<F: Files>(s: Session<F>) -> F {
	s.into_files()
}

fn script(name: &str) -> Script {
	parse_script(&fs::read_to_string(root().join(format!("tests/scenarios/{name}.keys"))).unwrap())
}

fn golden(path: &str) -> Vec<u8> {
	fs::read(root().join(path)).unwrap_or_else(|_| panic!("missing golden {path}"))
}

/// Each script, and the files it leaves behind: (name in the folder or in the settings folder, golden).
const RUNS: &[(&str, &[(&str, &str)])] = &[
	(
		"m1-edit",
		&[("plain.qld", "tests/golden/files/m1-edit.qld")],
	),
	(
		"m2-letter",
		&[("out.rtf", "tests/golden/files/m2-letter.rtf")],
	),
	("m2-find", &[]),
	("m2-lists", &[]),
	("m2-headers", &[]),
	("m2-dialogs", &[]),
	("m2-mono", &[]),
	("m12-confirm", &[]),
	("m12-confirm-mono", &[]),
	(
		"m4-about",
		&[("QUILL.INI", "tests/golden/files/m4-quill.ini")],
	),
	(
		"m4-print",
		&[("out.prn", "tests/golden/files/letter.print.txt")],
	),
	(
		"m4-print-headers",
		&[("out.prn", "tests/golden/files/headers.print.txt")],
	),
	(
		"m7-picture",
		&[("out.qld", "tests/golden/files/m7-picture.qld")],
	),
	(
		"m8-spelling",
		&[("QUILL.UDC", "tests/golden/files/m8-quill.udc")],
	),
];

fn frames_golden(name: &str) -> String {
	String::from_utf8(golden(&format!("tests/golden/screens/{name}.frames.txt"))).unwrap()
}

#[test]
fn every_script_gives_the_host_screens_and_files_over_memory() {
	for (name, outputs) in RUNS {
		let sc = script(name);
		let mut files = MemFiles::default();
		for (n, b) in copy_docs(&sc) {
			files.files.insert(n.to_ascii_uppercase(), b);
		}
		let (frames, files) = play(files, &sc);
		assert!(
			frames == frames_golden(name),
			"{name}: the screens differ from the golden"
		);
		for (file, want) in *outputs {
			assert_eq!(
				files.files[&file.to_ascii_uppercase()],
				golden(want),
				"{name}: {file}"
			);
		}
	}
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

#[test]
fn every_script_gives_the_host_screens_and_files_over_real_files() {
	for (name, outputs) in RUNS {
		let sc = script(name);
		let t = tempfile::tempdir().unwrap();
		let work = t.path().join("work");
		fs::create_dir(&work).unwrap();
		for (n, b) in copy_docs(&sc) {
			fs::write(work.join(n), b).unwrap();
		}
		let cfg = t.path().join("cfg/quill");
		let files = LinuxFiles::new(Dirs {
			base: work.clone(),
			config: cfg.clone(),
			data: Vec::new(),
		});
		let (frames, _) = play(files, &sc);
		assert!(
			frames == frames_golden(name),
			"{name}: the screens differ from the golden"
		);
		for (file, want) in *outputs {
			let dir = if file.starts_with("QUILL.") {
				&cfg
			} else {
				&work
			};
			let path = find_any_case(dir, file)
				.unwrap_or_else(|| panic!("{name}: {file} was not written"));
			assert_eq!(fs::read(path).unwrap(), golden(want), "{name}: {file}");
		}
		let left: Vec<_> = fs::read_dir(&work)
			.unwrap()
			.filter_map(Result::ok)
			.filter(|e| e.file_name().to_string_lossy().ends_with(".~tmp"))
			.collect();
		assert!(left.is_empty(), "{name}: temporary files left behind");
	}
}

#[test]
fn opening_editing_and_saving_the_letter_gives_the_host_bytes() {
	// The command-line way in: `quill-tty letter.rtf`, then File > Save over the real file.
	let t = tempfile::tempdir().unwrap();
	let letter = golden("tests/docs/letter.rtf");
	fs::write(t.path().join("letter.rtf"), &letter).unwrap();
	let files = LinuxFiles::new(Dirs {
		base: t.path().into(),
		config: t.path().join("cfg"),
		data: Vec::new(),
	});
	let opts = Options {
		about: "Host test run".into(),
		open: Some(b"letter.rtf".to_vec()),
		..Options::default()
	};
	let mut s = Session::new(files, &opts).unwrap();
	// Nothing is edited, so the file is the writer's own rendering of the document, which the host gives too.
	for t in "f10 down down enter".split_whitespace() {
		s.key(map_key(&typed(quill_app::script::parse_token(t).unwrap())).unwrap());
	}
	s.finish();
	let mut mem = MemFiles::default();
	mem.files.insert("LETTER.RTF".into(), letter);
	let mut hs = quill_host::HostStore::new();
	let mut app = quill_app::App::new();
	app.reset(&mut hs).unwrap();
	app.open(&mut hs, &mut mem, b"LETTER.RTF").unwrap();
	for t in "f10 down down enter".split_whitespace() {
		app.key(
			&mut hs,
			&mut mem,
			quill_app::script::parse_token(t).unwrap(),
		);
	}
	assert_eq!(
		fs::read(t.path().join("letter.rtf")).unwrap(),
		mem.files["LETTER.RTF"]
	);
}

#[test]
fn a_fifty_row_screen_draws_the_same_menu_and_status_lines() {
	let t = tempfile::tempdir().unwrap();
	let files = LinuxFiles::new(Dirs {
		base: t.path().into(),
		config: t.path().join("cfg"),
		data: Vec::new(),
	});
	let opts = Options {
		about: "x".into(),
		tall: true,
		..Options::default()
	};
	let s = Session::new(files, &opts).unwrap();
	let screen = s.screen();
	assert_eq!(screen.rows(), 50);
	let row = |y: usize| {
		(0..80)
			.map(|x| quill_grid::cp437(screen.get(x, y).unwrap().ch))
			.collect::<String>()
	};
	assert!(row(0).starts_with(" File  Edit"));
	assert!(row(49).contains("Pg 1"), "{:?}", row(49));
}
