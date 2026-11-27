// Autosave modes: Off, As you work and Every N minutes, their timing, and the `QUILL.INI` keys that carry them.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use quill_app::idle::Idle;
use quill_app::recovery::AutosaveMode;
use quill_app::{App, Key};
use quill_core::doc::Doc;
use quill_formats::qld;
use quill_host::docs::VecSink;
use quill_host::script::{dos_add, MemFiles};
use quill_host::HostStore;

const BIG: &str = "C:\\DOCS\\BIG.QLD";

/// A document of about `bytes` bytes of text, as the bytes of its `.QLD`.
fn big_doc(bytes: usize) -> Vec<u8> {
	let mut s = HostStore::new();
	let mut d = Doc::new();
	d.init(&mut s).unwrap();
	let mut body = Vec::new();
	while body.len() < bytes {
		body.extend_from_slice(b"the quick brown fox jumps over the lazy dog\r");
	}
	d.body.load(&mut s, &body, 0, 0).unwrap();
	let mut out = VecSink(Vec::new());
	qld::write(&d, &s, &mut out).unwrap();
	out.0
}

struct Rig {
	app: App,
	s: HostStore,
	f: MemFiles,
	/// Seconds since the last key, as a shell's timer would count them.
	quiet: u32,
	/// Seconds the test has waited since the start. The clock counts in the two seconds of a DOS time, so a key is typed at an even
	/// second.
	total: u32,
	start: u32,
}

impl Rig {
	/// An app set up as a shell does: the 8.3 builds' recovery, with `mode` and 5 minutes of backstop or interval.
	fn new(mode: AutosaveMode) -> Rig {
		let mut s = HostStore::new();
		let mut app = App::new();
		app.reset(&mut s).unwrap();
		app.configure_recovery(5, 0, false);
		app.configure_autosave(mode, 5, 3);
		app.set_recovery(true);
		let files = MemFiles::default();
		Rig {
			app,
			s,
			start: files.clock,
			f: files,
			quiet: 0,
			total: 0,
		}
	}

	/// Opens a document of about `bytes` bytes, whose write the poll for input can cut short.
	fn open_big(&mut self, bytes: usize) {
		self.f.files.insert(BIG.to_string(), big_doc(bytes));
		self.app
			.open(&mut self.s, &mut self.f, BIG.as_bytes())
			.unwrap();
	}

	fn type_(&mut self, text: &str) {
		self.quiet = 0;
		for b in text.bytes() {
			self.app.key(&mut self.s, &mut self.f, Key::Char(b));
		}
		// A shell's idle loop looks at least twice a second, so the interval starts at the first key.
		self.pass(&mut || false);
	}

	fn wait(&mut self, secs: u32) {
		self.total += secs;
		self.f.clock = dos_add(self.start, self.total);
	}

	fn quiet_for(&mut self, secs: u32) {
		self.wait(secs);
		self.quiet += secs;
	}

	fn pass(&mut self, input: &mut dyn FnMut() -> bool) -> quill_app::idle::Idled {
		let ctx = Idle {
			quiet: self.quiet,
			units: 8,
			busy: false,
		};
		self.app.idle(&mut self.s, &mut self.f, ctx, input)
	}

	fn wrote(&mut self) -> bool {
		self.pass(&mut || false).wrote
	}
}

// ---- As you work -------------------------------------------------------------------------------------------------------------------

#[test]
fn as_you_work_waits_for_the_pause_and_for_the_floor() {
	let mut r = Rig::new(AutosaveMode::Work);
	r.type_("some words");
	r.quiet_for(2);
	assert!(!r.wrote(), "two seconds of quiet is under the pause");
	r.quiet_for(1);
	assert!(
		!r.wrote(),
		"three seconds in, the floor of ten has not passed"
	);
	r.quiet_for(6);
	assert!(
		!r.wrote(),
		"nine seconds after the first change is under the floor"
	);
	r.quiet_for(1);
	assert!(
		r.wrote(),
		"a pause of ten seconds after the first change writes a copy"
	);
	assert!(
		!r.wrote(),
		"nothing changed since, so nothing more is written"
	);
	// More typing starts the floor again, and the next pause writes another copy.
	r.type_(" and more");
	r.quiet_for(3);
	assert!(
		!r.wrote(),
		"the floor runs from the first change after a copy"
	);
	r.quiet_for(7);
	assert!(r.wrote());
}

#[test]
fn a_key_during_the_floor_keeps_as_you_work_from_writing() {
	let mut r = Rig::new(AutosaveMode::Work);
	r.type_("a");
	r.wait(20);
	r.type_("b");
	r.quiet_for(2);
	assert!(!r.wrote(), "a key a moment ago is not a pause");
	r.quiet_for(1);
	assert!(r.wrote());
}

#[test]
fn a_cut_short_write_is_retried_at_the_next_pause_and_forced_only_at_the_backstop() {
	let mut r = Rig::new(AutosaveMode::Work);
	r.open_big(60_000);
	r.type_("x");
	r.quiet_for(10);
	let cut = |r: &mut Rig| {
		let mut polls = 0;
		r.pass(&mut || {
			polls += 1;
			polls > 1
		})
	};
	for _ in 0..4 {
		let out = cut(&mut r);
		assert!(
			!out.wrote && !out.more,
			"an abandoned write only waits for the next pause"
		);
		assert_eq!(r.app.message(), b"", "no announcement before the backstop");
		r.quiet_for(1);
	}
	// The backstop's five minutes have passed since the first change: the next write is announced, and runs to the end.
	r.quiet_for(300);
	let announce = r.pass(&mut || true);
	assert!(announce.more && !announce.wrote);
	assert!(
		r.pass(&mut || true).wrote,
		"a forced write does not ask for input"
	);
}

#[test]
fn every_n_minutes_forces_the_third_write_at_once() {
	let mut r = Rig::new(AutosaveMode::Minutes);
	r.open_big(60_000);
	r.type_("x");
	r.wait(300);
	r.quiet_for(2);
	for _ in 0..2 {
		let mut polls = 0;
		let out = r.pass(&mut || {
			polls += 1;
			polls > 1
		});
		assert!(!out.wrote && !out.more);
	}
	assert!(r.pass(&mut || true).more, "the third is announced");
}

// ---- Every N minutes and Off -------------------------------------------------------------------------------------------------------

#[test]
fn every_n_minutes_writes_at_five_minutes_and_a_two_second_pause() {
	let mut r = Rig::new(AutosaveMode::Minutes);
	r.type_("words");
	r.wait(297);
	r.quiet_for(2);
	assert!(!r.wrote(), "four minutes and 59 seconds");
	r.quiet_for(1);
	assert!(r.wrote(), "five minutes");

	let mut r = Rig::new(AutosaveMode::Minutes);
	r.type_("words");
	r.wait(298);
	r.quiet_for(1);
	assert!(!r.wrote(), "five minutes but one second of quiet");
	r.quiet_for(1);
	assert!(r.wrote(), "five minutes and two seconds of quiet");
}

#[test]
fn off_writes_nothing() {
	let mut r = Rig::new(AutosaveMode::Off);
	r.type_("words");
	r.quiet_for(3600);
	assert!(!r.wrote());
	assert!(!r.pass(&mut || false).waiting);
}

#[test]
fn the_debug_seconds_win_over_the_mode() {
	let mut r = Rig::new(AutosaveMode::Work);
	r.app.apply_setting(b"Quill", b"AutosaveSeconds", b"4");
	r.type_("words");
	r.quiet_for(2);
	assert!(!r.wrote(), "the debug interval is four seconds");
	r.quiet_for(2);
	assert!(r.wrote());
}

// ---- the settings file -------------------------------------------------------------------------------------------------------------

/// The family suffix of this build's keys: the tests below write the 8.3 builds' names, and a build with long names adds this to them.
const FAMILY: &str = if cfg!(feature = "long-names") {
	"32"
} else {
	""
};

/// The file the app writes, with this build's family suffix taken off the autosave keys.
fn ini_of(app: &App) -> String {
	ini_raw(app)
		.replace(&format!("AutosaveMode{FAMILY}"), "AutosaveMode")
		.replace(&format!("AutosavePause{FAMILY}"), "AutosavePause")
}

fn ini_raw(app: &App) -> String {
	let mut f = MemFiles::default();
	app.save_settings(&mut f);
	String::from_utf8(f.files.get("QUILL.INI").cloned().unwrap_or_default()).unwrap()
}

/// An app that reads `ini` written with the 8.3 builds' key names, which are given this build's family suffix.
fn app_reading(mode: AutosaveMode, ini: &str) -> App {
	app_reading_raw(
		mode,
		&ini.replace("AutosaveMode", &format!("AutosaveMode{FAMILY}"))
			.replace("AutosavePause", &format!("AutosavePause{FAMILY}")),
	)
}

fn app_reading_raw(mode: AutosaveMode, ini: &str) -> App {
	let mut f = MemFiles::default();
	f.files
		.insert("QUILL.INI".to_string(), ini.as_bytes().to_vec());
	let mut app = App::new();
	app.configure_recovery(5, 0, false);
	app.configure_autosave(mode, 5, 3);
	app.load_settings(&mut f);
	app
}

#[test]
fn the_defaults_write_no_autosave_lines() {
	for mode in [AutosaveMode::Minutes, AutosaveMode::Work] {
		let ini = ini_of(&app_reading(mode, ""));
		assert!(!ini.contains("Autosave"), "{ini}");
	}
}

#[test]
fn a_hand_set_interval_is_kept_and_written_back() {
	let app = app_reading(AutosaveMode::Minutes, "[Quill]\r\nAutosave=10\r\n");
	assert_eq!(app.autosave_minutes(), 10);
	assert_eq!(app.autosave_mode(), AutosaveMode::Minutes);
	let ini = ini_of(&app);
	assert!(
		ini.contains("Autosave=10\r\n") && !ini.contains("AutosaveMode"),
		"{ini}"
	);
}

#[test]
fn the_mode_and_the_pause_are_written_when_set_and_read_back() {
	let app = app_reading(
		AutosaveMode::Minutes,
		"[Quill]\r\nAutosaveMode=WORK\r\nAutosavePause=5\r\n",
	);
	assert_eq!(app.autosave_mode(), AutosaveMode::Work);
	assert_eq!(app.autosave_pause(), 5);
	let ini = ini_of(&app);
	assert!(
		ini.contains("AutosaveMode=work\r\n") && ini.contains("AutosavePause=5\r\n"),
		"{ini}"
	);
	let again = app_reading(AutosaveMode::Minutes, &ini);
	assert_eq!(again.autosave_mode(), AutosaveMode::Work);
	assert_eq!(again.autosave_pause(), 5);
	// A value equal to the default is not written.
	let same = app_reading(
		AutosaveMode::Work,
		"[Quill]\r\nAutosaveMode=work\r\nAutosavePause=3\r\n",
	);
	assert!(!ini_of(&same).contains("Autosave"));
}

#[test]
fn a_bad_mode_or_pause_is_the_default() {
	let app = app_reading(
		AutosaveMode::Minutes,
		"[Quill]\r\nAutosaveMode=sometimes\r\nAutosavePause=0\r\n",
	);
	assert_eq!(app.autosave_mode(), AutosaveMode::Minutes);
	assert_eq!(app.autosave_pause(), 3);
	let app = app_reading(AutosaveMode::Minutes, "[Quill]\r\nAutosavePause=99\r\n");
	assert_eq!(app.autosave_pause(), 30);
}

#[test]
fn an_old_autosave_of_zero_is_off_and_stays_off() {
	let app = app_reading(AutosaveMode::Minutes, "[Quill]\r\nAutosave=0\r\n");
	assert_eq!(app.autosave_mode(), AutosaveMode::Off);
	let ini = ini_of(&app);
	assert!(ini.contains("AutosaveMode=off\r\n"), "{ini}");
	assert_eq!(
		app_reading(AutosaveMode::Work, &ini).autosave_mode(),
		AutosaveMode::Off
	);
	// On a build whose default is As you work too.
	assert_eq!(
		app_reading(AutosaveMode::Work, "[Quill]\r\nAutosave=0\r\n").autosave_mode(),
		AutosaveMode::Off
	);
}

#[test]
fn a_file_that_names_the_mode_ignores_an_autosave_of_zero_in_either_order() {
	for ini in [
		"[Quill]\r\nAutosave=0\r\nAutosaveMode=work\r\n",
		"[Quill]\r\nAutosaveMode=work\r\nAutosave=0\r\n",
	] {
		let app = app_reading(AutosaveMode::Minutes, ini);
		assert_eq!(app.autosave_mode(), AutosaveMode::Work, "{ini}");
		assert_eq!(app.autosave_minutes(), 5, "{ini}");
	}
}

#[test]
fn off_is_written_as_a_mode_and_leaves_the_minutes_alone() {
	let mut app = app_reading(AutosaveMode::Minutes, "");
	app.set_autosave_mode(AutosaveMode::Off);
	let ini = ini_of(&app);
	assert!(
		ini.contains("AutosaveMode=off\r\n") && !ini.contains("Autosave="),
		"{ini}"
	);
}

#[cfg(not(feature = "long-names"))]
#[test]
fn an_8_3_build_ignores_the_win32_keys() {
	let app = app_reading_raw(
		AutosaveMode::Minutes,
		"[Quill]\r\nAutosaveMode32=off\r\nAutosavePause32=9\r\n",
	);
	assert_eq!(app.autosave_mode(), AutosaveMode::Minutes);
	assert_eq!(app.autosave_pause(), 3);
}

/// Run with `cargo nextest run -p quill-host --features long-names --test autosave_modes`: the Win32 build turns the same feature on.
#[cfg(feature = "long-names")]
#[test]
fn a_win32_style_build_reads_its_own_keys_and_ignores_the_8_3_ones() {
	let app = app_reading_raw(
		AutosaveMode::Work,
		"[Quill]\r\nAutosaveMode=off\r\nAutosavePause=9\r\nAutosaveMode32=minutes\r\nAutosavePause32=7\r\n",
	);
	assert_eq!(app.autosave_mode(), AutosaveMode::Minutes);
	assert_eq!(app.autosave_pause(), 7);
	let ini = ini_raw(&app);
	assert!(
		ini.contains("AutosaveMode32=minutes\r\n") && ini.contains("AutosavePause32=7\r\n"),
		"{ini}"
	);
	assert!(
		!ini.contains("AutosaveMode=") && !ini.contains("AutosavePause="),
		"{ini}"
	);
}
