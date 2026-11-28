// The shell's screen: frames of a keystroke scenario on both surfaces, the row hashes, and the settings.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod common;

use std::fmt::Write as _;

use common::*;
use quill_app::recovery::AutosaveMode;
use quill_host::png::encode_indexed;
use quill_host::script::{parse_script, stage, MemFiles};
use quill_paint::damage::{self, RowSet, PLANAR_ROW};
use quill_paint::layout::Layout;
use quill_paint::pointer::{Gfx, View};
use quill_paint::raster::{Planar4, PLANAR_BYTES, PLANE_BYTES};
use quill_paint::shell::{self, paint_screen, paint_screen_view, Graphics};
use quill_paint::{crc32, Surface};
use quill_picture::WINDOW;
use quill_strike::{SliceSource, StrikeMeasure};

fn update() -> bool {
	std::env::var_os("QUILL_UPDATE_GOLDEN").is_some()
}

fn compare_bytes(path: &str, actual: &[u8]) {
	let file = root().join(path);
	if update() {
		std::fs::create_dir_all(file.parent().unwrap()).unwrap();
		std::fs::write(&file, actual).unwrap();
		return;
	}
	let want = std::fs::read(&file)
		.unwrap_or_else(|_| panic!("missing golden {path}; run with QUILL_UPDATE_GOLDEN=1"));
	assert!(
		want == actual,
		"{path} differs; QUILL_UPDATE_GOLDEN=1 rewrites it"
	);
}

struct ScreenJob<'a> {
	app: &'a TApp,
	s: &'a quill_host::HostStore,
	window: Box<[u8; WINDOW]>,
	view: View,
}

impl Job for ScreenJob<'_> {
	fn run<Sf: Surface>(&mut self, sf: &mut Sf) {
		let mut m = StrikeMeasure::new(strikes());
		paint_screen_view(sf, self.app, &mut m, self.s, &mut self.window, &self.view);
	}
}

/// Plays the scenario as the DOS shell does: a frame at the start and after every key or pointer action. Returns the frames of one mode
/// and the files.
fn play(mode: Mode) -> (Vec<Frame>, MemFiles) {
	play_file(mode, "g6-gfx")
}

fn play_file(mode: Mode, name: &str) -> (Vec<Frame>, MemFiles) {
	let text =
		std::fs::read_to_string(root().join(format!("tests/scenarios/{name}.keys"))).unwrap();
	let sc = parse_script(&text);
	let mut files = MemFiles::default();
	stage(&mut files, &sc, &root().join("tests/docs"));
	let (mut app, mut s, _) = new_app();
	app.set_spell_live_default(sc.live);
	if sc.recovery {
		app.configure_recovery(5, 1, false);
		app.configure_autosave(AutosaveMode::Minutes, 5, 3);
		app.set_recovery(true);
		app.load_settings_with(&mut s, &mut files);
	} else {
		app.load_settings(&mut files);
	}
	app.set_about("Host test run");
	// The shell shows the file dialog as the DOS graphics shell does, unless the scenario brings the name prompt back (`@prompt`).
	app.set_file_dialog(sc.dialog);
	app.set_file_dialog_graphics(true);
	let places: Vec<(&[u8], &[u8])> = sc
		.places
		.iter()
		.map(|(l, p)| (l.as_bytes(), p.as_bytes()))
		.collect();
	app.set_file_places(&places);
	shell::sync_view(&mut app, &s);
	if sc.recovery {
		app.recovery_check_launch(&mut s, &mut files);
		app.recovery_prompt(&mut s, &mut files);
	}
	if let Some(open) = sc.open.as_deref() {
		app.open(&mut s, &mut files, open.as_bytes()).unwrap();
	}
	let mut frames = vec![frame(
		mode,
		&mut ScreenJob {
			app: &app,
			s: &s,
			window: Box::new([0; WINDOW]),
			view: View::default(),
		},
	)];
	let mut gfx = Gfx::new(&mut StrikeMeasure::new(strikes()));
	let mut tick = 0;
	// Check Spelling as You Type (`@spell`): the keyboard goes quiet after each frame and the whole scan runs, as the DOS shell does.
	let settle = |app: &mut TApp, s: &mut quill_host::HostStore, files: &mut MemFiles| {
		if app.spell_live() {
			shell::spell_view(app, s);
			app.drain(s, files, 0);
		}
	};
	settle(&mut app, &mut s, &mut files);
	for step in &sc.steps {
		match step {
			quill_app::script::Step::Key(k) => {
				if !gfx.key(&mut app, &mut s, *k) {
					app.key(&mut s, &mut files, *k)
				}
				shell::sync_view(&mut app, &s);
				shell::follow_caret(&mut app, &s);
			}
			other => other.play(&mut tick, |ev, ctx| {
				gfx.handle(&mut app, &mut s, &mut files, ev, ctx);
			}),
		}
		frames.push(frame(
			mode,
			&mut ScreenJob {
				app: &app,
				s: &s,
				window: Box::new([0; WINDOW]),
				view: gfx.view(),
			},
		));
		settle(&mut app, &mut s, &mut files);
		if app.quit() {
			break;
		}
	}
	(frames, files)
}

fn golden(mode: Mode) {
	golden_of(mode, "g6-gfx");
}

fn golden_of(mode: Mode, name: &str) -> Vec<Frame> {
	let (frames, files) = play_file(mode, name);
	let mut sums = String::new();
	for (n, fr) in frames.iter().enumerate() {
		writeln!(sums, "frame {} crc32 {:08x}", n + 1, crc32(&fr.native)).unwrap();
	}
	compare_bytes(
		&format!("tests/golden/gfx/{name}.{}.sums", mode.name()),
		sums.as_bytes(),
	);
	if name == "g9-gfx-save-as" {
		// The same document saved as RTF into the same folder as the text screen's `m16-save-as`: the same bytes.
		let want = std::fs::read(root().join("tests/golden/files/m16-new.rtf")).unwrap();
		assert!(
			files.files["C:\\DOCS\\NEW.RTF"] == want,
			"the saved NEW.RTF differs from the text screen's"
		);
	}
	if name == "g9-gfx-save-as-mouse" {
		// Yes on the replace question saves LETTER.RTF over the one that was there.
		compare_bytes(
			"tests/golden/files/g9-gfx-save-as-mouse.rtf",
			&files.files["C:\\DOCS\\LETTER.RTF"],
		);
	}
	// The file dialog's scenarios keep the frames that show its parts as pictures, by frame number.
	let shots: &[(&str, usize)] = match name {
		"g9-gfx-open" => &[("dialog", 4), ("types", 6), ("sorted", 14)],
		"g9-gfx-open-mouse" => &[("picked", 5), ("scrolled", 20), ("types", 31)],
		"g9-gfx-save-as-mouse" => &[("new-folder", 7), ("read-only", 14), ("replace", 19)],
		"g13-gfx-last-used" => &[("save-as", 6), ("last-used", 12)],
		"g17-gfx-recover" => &[("question", 1), ("focus", 2), ("recovered", 3)],
		"g9-gfx-save-as" => &[
			("dialog", 6),
			("new-folder", 7),
			("read-only", 19),
			("replace", 42),
		],
		_ => &[],
	};
	for (label, n) in shots {
		let png = encode_indexed(
			640,
			480,
			mode.depth(),
			&mode.palette(),
			&frames[n - 1].pixels,
		);
		compare_bytes(
			&format!("tests/golden/gfx/{name}.{}.{label}.png", mode.name()),
			&png,
		);
	}
	if name == "g10-gfx-spell" {
		// The frame after the words are checked, as a picture: a wave under each misspelt word.
		let png = encode_indexed(
			640,
			480,
			mode.depth(),
			&mode.palette(),
			&frames[frames.len() - 1].pixels,
		);
		compare_bytes(
			&format!("tests/golden/gfx/{name}.{}.marked.png", mode.name()),
			&png,
		);
	}
	if !name.starts_with("g7-")
		&& !name.starts_with("g9-")
		&& !name.starts_with("g13-")
		&& !name.starts_with("g17-")
		&& name != "g10-gfx-spell"
	{
		let text =
			std::fs::read_to_string(root().join(format!("tests/scenarios/{name}.keys"))).unwrap();
		let opened = parse_script(&text).open.unwrap().to_ascii_uppercase();
		compare_bytes(
			&format!("tests/golden/files/{name}.rtf"),
			&files.files[&opened],
		);
		for (label, n) in [("start", 0), ("end", frames.len() - 2)] {
			let fr = &frames[n];
			let png = encode_indexed(640, 480, mode.depth(), &mode.palette(), &fr.pixels);
			compare_bytes(
				&format!("tests/golden/gfx/{name}.{}.{label}.png", mode.name()),
				&png,
			);
		}
	}
	// QUILL_FRAMES_DIR=dir writes every frame as a PNG there, for looking at.
	if let Some(dir) = std::env::var_os("QUILL_FRAMES_DIR") {
		let dir = std::path::PathBuf::from(dir);
		std::fs::create_dir_all(&dir).unwrap();
		for (n, fr) in frames.iter().enumerate() {
			let png = encode_indexed(640, 480, mode.depth(), &mode.palette(), &fr.pixels);
			std::fs::write(
				dir.join(format!("{name}.{}.{:03}.png", mode.name(), n + 1)),
				png,
			)
			.unwrap();
		}
	}
	frames
}

/// The frame `dos/tools/mouse-x11.sh` compares with: the last one of the scenario a real pointer is made to play, as `crc32 XXXXXXXX`.
fn real_driver_final(mode: Mode) {
	let (frames, _) = play_file(mode, "g8-real");
	let last = frames.last().unwrap();
	compare_bytes(
		&format!("tests/golden/gfx/g8-real.{}.final", mode.name()),
		format!("crc32 {:08x}\n", crc32(&last.native)).as_bytes(),
	);
}

/// The frame `dos/tools/keys-x11.sh` compares with: the last one of the scenario whose zoom keys a real keyboard is made to type.
fn real_keys_final(mode: Mode, name: &str) {
	let (frames, _) = play_file(mode, name);
	let last = frames.last().unwrap();
	compare_bytes(
		&format!("tests/golden/gfx/{name}.{}.final", mode.name()),
		format!("crc32 {:08x}\n", crc32(&last.native)).as_bytes(),
	);
}

#[test]
fn the_real_keyboard_zoom_scenario_ends_on_the_same_frame_at_16_colours() {
	real_keys_final(Mode::C16, "g11-real-keys");
}

#[test]
fn the_real_keyboard_scroll_scenario_ends_on_the_same_frame_at_16_colours() {
	real_keys_final(Mode::C16, "g12-real-keys");
}

#[test]
fn the_real_keyboard_zoom_scenario_ends_on_the_same_frame_at_256_colours() {
	real_keys_final(Mode::C256, "g11-real-keys");
}

#[test]
fn the_real_keyboard_scroll_scenario_ends_on_the_same_frame_at_256_colours() {
	real_keys_final(Mode::C256, "g12-real-keys");
}

#[test]
fn the_real_driver_scenario_ends_on_the_same_frame_at_16_colours() {
	real_driver_final(Mode::C16);
}

#[test]
fn the_real_driver_scenario_ends_on_the_same_frame_at_256_colours() {
	real_driver_final(Mode::C256);
}

/// The frame `dos/tools/held-keys-x11.sh` compares with: the last one of the scenario whose keys are held down for real.
fn held_keys_final(mode: Mode) {
	let (frames, _) = play_file(mode, "g18-held-keys");
	let last = frames.last().unwrap();
	compare_bytes(
		&format!("tests/golden/gfx/g18-held-keys.{}.final", mode.name()),
		format!("crc32 {:08x}\n", crc32(&last.native)).as_bytes(),
	);
}

#[test]
fn the_held_keys_scenario_ends_on_the_same_frame_at_16_colours() {
	held_keys_final(Mode::C16);
}

#[test]
fn the_held_keys_scenario_ends_on_the_same_frame_at_256_colours() {
	held_keys_final(Mode::C256);
}

#[test]
fn spelling_as_you_type_in_graphics_at_16_colours() {
	golden_of(Mode::C16, "g10-gfx-spell");
}

#[test]
fn spelling_as_you_type_in_graphics_at_256_colours() {
	golden_of(Mode::C256, "g10-gfx-spell");
}

#[test]
fn the_pointer_scenario_at_16_colours() {
	golden_of(Mode::C16, "g8-mouse-gfx");
}

#[test]
fn the_pointer_scenario_at_256_colours() {
	golden_of(Mode::C256, "g8-mouse-gfx");
}

#[test]
fn the_scenario_at_16_colours() {
	golden(Mode::C16);
}

#[test]
fn the_scenario_at_256_colours() {
	golden(Mode::C256);
}

#[test]
fn the_text_column_fits_beside_the_edges() {
	let (app, s, _) = open_doc("letter.rtf");
	let lay = Layout::of(&app);
	let hs = shell::hscroll(&app, lay.page.width());
	let (left, w) = quill_chrome::view::text_extent(&app, lay.page.width(), hs);
	assert!(
		left >= 0 && left + w <= lay.page.width(),
		"text {left}..{}",
		left + w
	);
	assert_eq!(lay.page.top, 98);
	assert_eq!(lay.page.bottom, 444);
	let _ = s;
}

#[test]
fn the_chrome_is_drawn_over_its_strips_only() {
	let (app, s, _) = open_doc("letter.rtf");
	let fr = frame(
		Mode::C256,
		&mut ScreenJob {
			app: &app,
			s: &s,
			window: Box::new([0; WINDOW]),
			view: View::default(),
		},
	);
	// The status line is silver at its left edge, and the page area is not all silver.
	assert_eq!(
		fr.pixels[479 * 640],
		0xD8 + 6,
		"silver is the extra entry 222"
	);
	let lay = Layout::of(&app);
	let area = &fr.pixels[lay.page.top as usize * 640..lay.page.bottom as usize * 640];
	let mut seen = [false; 256];
	for p in area {
		seen[*p as usize] = true;
	}
	assert!(
		seen.iter().filter(|v| **v).count() >= 3,
		"desk, paper and ink"
	);
}

fn random_frame(seed: u32) -> Vec<u8> {
	let mut x = seed.wrapping_mul(2_654_435_761).wrapping_add(1);
	let mut v = vec![0u8; PLANAR_BYTES];
	for b in v.iter_mut() {
		x ^= x << 13;
		x ^= x >> 17;
		x ^= x << 5;
		*b = (x >> 24) as u8 & 0x0F;
	}
	v
}

fn naive(prev: &[u8], cur: &[u8]) -> Vec<bool> {
	(0..480)
		.map(|y| {
			(0..4).any(|p| {
				let at = p * PLANE_BYTES + y * PLANAR_ROW;
				prev[at..at + PLANAR_ROW] != cur[at..at + PLANAR_ROW]
			})
		})
		.collect()
}

#[test]
fn the_comparison_finds_exactly_the_rows_that_changed() {
	let a = random_frame(1);
	assert_eq!(damage::planar(&[], &a).count(), 480);
	assert_eq!(damage::planar(&a, &a).count(), 0);
	let mut b = a.clone();
	let rows = [0usize, 7, 8, 63, 64, 65, 300, 479];
	for (i, y) in rows.into_iter().enumerate() {
		b[(i % 4) * PLANE_BYTES + y * PLANAR_ROW + (i * 11) % PLANAR_ROW] ^= 0x40;
	}
	let set = damage::planar(&a, &b);
	assert_eq!(set.count(), rows.len());
	for y in 0..480 {
		assert_eq!(set.has(y), rows.contains(&y), "row {y}");
	}
	assert_eq!(RowSet::full().count(), 480);
}

#[test]
fn four_equal_changes_in_the_four_planes_are_still_a_change() {
	// What a sum over the row's words cannot see: the same bits change in every plane, so the changes add up to nothing.
	let a = vec![0x3F; PLANAR_BYTES];
	let mut b = a.clone();
	for p in 0..4 {
		b[p * PLANE_BYTES + 50 * PLANAR_ROW + 35] = 0xFF;
	}
	let set = damage::planar(&a, &b);
	assert!(set.has(50) && set.count() == 1);
}

#[test]
fn the_comparison_agrees_with_bytes_over_many_frames() {
	let mut prev = random_frame(9);
	for seed in 10..60u32 {
		let mut next = prev.clone();
		let fresh = random_frame(seed);
		for k in 0..5u32 {
			let y = ((seed * 37 + k * 101) % 480) as usize;
			for p in 0..4 {
				let at = p * PLANE_BYTES + y * PLANAR_ROW;
				next[at..at + PLANAR_ROW].copy_from_slice(&fresh[at..at + PLANAR_ROW]);
			}
		}
		let set = damage::planar(&prev, &next);
		let want = naive(&prev, &next);
		for (y, w) in want.iter().enumerate() {
			assert_eq!(set.has(y), *w, "seed {seed} row {y}");
		}
		prev = next;
	}
}

#[test]
fn the_comparison_works_on_a_linear_shadow_with_a_pitch() {
	let pitch = 704;
	let a = vec![3u8; pitch * 480];
	let mut b = a.clone();
	b[100 * pitch + 639] = 9;
	b[101 * pitch + 640] = 9; // padding past the width is not part of the row
	let set = damage::linear(&a, &b, pitch);
	assert!(set.has(100) && !set.has(101) && set.count() == 1);
}

#[test]
fn the_comparison_misses_no_changed_row_across_the_scenario() {
	for mode in [Mode::C16, Mode::C256] {
		let (frames, _) = play(mode);
		for (n, pair) in frames.windows(2).enumerate() {
			let set = if mode == Mode::C16 {
				damage::planar(&pair[0].native, &pair[1].native)
			} else {
				damage::linear(&pair[0].native, &pair[1].native, 640)
			};
			for y in 0..480 {
				let differs = if mode == Mode::C16 {
					naive(&pair[0].native, &pair[1].native)[y]
				} else {
					pair[0].native[y * 640..(y + 1) * 640] != pair[1].native[y * 640..(y + 1) * 640]
				};
				assert_eq!(set.has(y), differs, "frame {} row {y}", n + 2);
			}
		}
	}
}

#[test]
fn a_real_screen_changes_few_rows_for_one_typed_letter() {
	let (mut app, mut s, mut f) = open_named_for_typing();
	let draw = |app: &TApp, s: &quill_host::HostStore| {
		let mut src = SliceSource(font_bytes());
		let mut buf = vec![0u8; PLANAR_BYTES];
		let mut sf = Planar4::new(&mut buf, *strikes(), &mut src).unwrap();
		let mut m = StrikeMeasure::new(strikes());
		let mut w = Box::new([0u8; WINDOW]);
		paint_screen(&mut sf, app, &mut m, s, &mut w);
		buf
	};
	let before = draw(&app, &s);
	app.key(&mut s, &mut f, quill_app::Key::Char(b'x'));
	let n = damage::planar(&before, &draw(&app, &s)).count();
	assert!(n > 0 && n < 120, "{n} rows changed for one letter");
}

fn open_named_for_typing() -> (TApp, quill_host::HostStore, MemFiles) {
	let (mut app, s, f) = open_doc("letter.rtf");
	shell::sync_view(&mut app, &s);
	(app, s, f)
}

#[test]
fn the_graphics_setting_reads_the_settings_text_and_the_command_line() {
	assert_eq!(
		Graphics::from_ini(b"[Quill]\r\nGraphics=1\r\n"),
		Graphics::Best
	);
	assert_eq!(Graphics::from_ini(b"Graphics=16\n"), Graphics::Planar16);
	assert_eq!(Graphics::from_ini(b"Graphics=0\n"), Graphics::Text);
	assert_eq!(Graphics::from_ini(b"Graphics=banana\n"), Graphics::Text);
	assert_eq!(Graphics::from_ini(b"Rows=50\n"), Graphics::Text);
	assert_eq!(Graphics::from_ini(b""), Graphics::Text);
	assert_eq!(Graphics::from_arg("/G"), Some(Graphics::Best));
	assert_eq!(Graphics::from_arg("/gfx"), Some(Graphics::Best));
	assert_eq!(Graphics::from_arg("-G16"), Some(Graphics::Planar16));
	assert_eq!(Graphics::from_arg("/t"), Some(Graphics::Text));
	assert_eq!(Graphics::from_arg("/mono"), None);
	assert_eq!(Graphics::from_arg("LETTER.RTF"), None);
}

#[test]
fn the_menus_at_16_colours() {
	golden_of(Mode::C16, "g7-menus");
}

#[test]
fn the_menus_at_256_colours() {
	golden_of(Mode::C256, "g7-menus");
}

#[test]
fn the_dialogs_at_16_colours() {
	golden_of(Mode::C16, "g7-dialogs");
}

#[test]
fn the_dialogs_at_256_colours() {
	golden_of(Mode::C256, "g7-dialogs");
}

#[test]
fn the_toolbar_and_the_scroll_bars_at_16_colours() {
	golden_of(Mode::C16, "g6-toolbar");
}

#[test]
fn the_toolbar_and_the_scroll_bars_at_256_colours() {
	golden_of(Mode::C256, "g6-toolbar");
}

#[test]
fn the_pointer_on_the_menus_at_16_colours() {
	golden_of(Mode::C16, "g8-gfx-menus");
}

#[test]
fn the_pointer_on_the_menus_at_256_colours() {
	golden_of(Mode::C256, "g8-gfx-menus");
}

#[test]
fn the_pointer_in_the_dialogs_at_16_colours() {
	golden_of(Mode::C16, "g8-gfx-dialogs");
}

#[test]
fn the_pointer_in_the_dialogs_at_256_colours() {
	golden_of(Mode::C256, "g8-gfx-dialogs");
}

#[test]
fn the_pointer_on_the_toolbars_at_16_colours() {
	golden_of(Mode::C16, "g8-gfx-toolbar");
}

#[test]
fn the_pointer_on_the_toolbars_at_256_colours() {
	golden_of(Mode::C256, "g8-gfx-toolbar");
}

#[test]
fn the_pointer_on_the_scroll_bar_at_16_colours() {
	golden_of(Mode::C16, "g8-gfx-scroll");
}

#[test]
fn the_pointer_on_the_scroll_bar_at_256_colours() {
	golden_of(Mode::C256, "g8-gfx-scroll");
}

#[test]
fn the_scroll_keys_at_16_colours() {
	golden_of(Mode::C16, "g12-gfx-scroll-keys");
}

#[test]
fn the_scroll_keys_at_256_colours() {
	golden_of(Mode::C256, "g12-gfx-scroll-keys");
}

#[test]
fn a_last_used_file_in_save_as_at_16_colours() {
	golden_of(Mode::C16, "g13-gfx-last-used");
}

#[test]
fn a_last_used_file_in_save_as_at_256_colours() {
	golden_of(Mode::C256, "g13-gfx-last-used");
}

#[test]
fn the_zoom_at_16_colours() {
	golden_of(Mode::C16, "g11-gfx-zoom");
}

#[test]
fn the_zoom_at_256_colours() {
	golden_of(Mode::C256, "g11-gfx-zoom");
}

#[test]
fn the_horizontal_scroll_bar_at_16_colours() {
	golden_of(Mode::C16, "g8-gfx-hscroll");
}

#[test]
fn the_horizontal_scroll_bar_at_256_colours() {
	golden_of(Mode::C256, "g8-gfx-hscroll");
}

#[test]
fn the_caret_in_a_dialogs_text_field_at_16_colours() {
	golden_of(Mode::C16, "g8-gfx-fields");
}

#[test]
fn the_caret_in_a_dialogs_text_field_at_256_colours() {
	golden_of(Mode::C256, "g8-gfx-fields");
}

#[test]
fn the_rulers_tab_kind_button_at_16_colours() {
	golden_of(Mode::C16, "g8-gfx-ruler");
}

#[test]
fn the_rulers_tab_kind_button_at_256_colours() {
	golden_of(Mode::C256, "g8-gfx-ruler");
}

#[test]
fn the_recovery_question_at_16_colours() {
	golden_of(Mode::C16, "g17-gfx-recover");
}

#[test]
fn the_recovery_question_at_256_colours() {
	golden_of(Mode::C256, "g17-gfx-recover");
}

#[test]
fn the_file_dialog_at_16_colours() {
	golden_of(Mode::C16, "g9-gfx-open");
}

#[test]
fn the_file_dialog_at_256_colours() {
	golden_of(Mode::C256, "g9-gfx-open");
}

#[test]
fn the_file_dialog_save_as_at_16_colours() {
	golden_of(Mode::C16, "g9-gfx-save-as");
}

#[test]
fn the_file_dialog_save_as_at_256_colours() {
	golden_of(Mode::C256, "g9-gfx-save-as");
}

#[test]
fn the_pointer_in_the_file_dialog_at_16_colours() {
	golden_of(Mode::C16, "g9-gfx-open-mouse");
}

#[test]
fn the_pointer_in_the_file_dialog_at_256_colours() {
	golden_of(Mode::C256, "g9-gfx-open-mouse");
}

#[test]
fn the_pointer_in_save_as_at_16_colours() {
	golden_of(Mode::C16, "g9-gfx-save-as-mouse");
}

#[test]
fn the_pointer_in_save_as_at_256_colours() {
	golden_of(Mode::C256, "g9-gfx-save-as-mouse");
}
