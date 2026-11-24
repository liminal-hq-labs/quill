// The shell's screen: frames of a keystroke scenario on both surfaces, the row hashes, and the settings.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod common;

use std::fmt::Write as _;

use common::*;
use quill_app::picture::WINDOW;
use quill_host::png::encode_indexed;
use quill_host::script::{parse_script, MemFiles};
use quill_paint::damage::{self, RowSet, PLANAR_ROW};
use quill_paint::raster::{Planar4, PLANAR_BYTES, PLANE_BYTES};
use quill_paint::shell::{self, paint_screen, Graphics, PAGE_AREA};
use quill_paint::{crc32, Surface};
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
}

impl Job for ScreenJob<'_> {
	fn run<Sf: Surface>(&mut self, sf: &mut Sf) {
		let mut m = StrikeMeasure::new(strikes());
		paint_screen(sf, self.app, &mut m, self.s, &mut self.window);
	}
}

/// Plays the scenario as the DOS shell does: a frame at the start and after every key. Returns the frames of one mode and the saved file.
fn play(mode: Mode) -> (Vec<Frame>, Vec<u8>) {
	let text = std::fs::read_to_string(root().join("tests/scenarios/g6-gfx.keys")).unwrap();
	let sc = parse_script(&text);
	let mut files = MemFiles::default();
	for name in &sc.copy {
		files.files.insert(
			name.to_ascii_uppercase(),
			std::fs::read(root().join("tests/docs").join(name)).unwrap(),
		);
	}
	let (mut app, mut s, _) = new_app();
	app.load_settings(&mut files);
	app.set_about("Host test run");
	app.set_view_height(shell::view_height());
	app.set_rows(shell::view_rows());
	app.open(&mut s, &mut files, sc.open.as_deref().unwrap().as_bytes())
		.unwrap();
	let mut frames = vec![frame(
		mode,
		&mut ScreenJob {
			app: &app,
			s: &s,
			window: Box::new([0; WINDOW]),
		},
	)];
	for k in &sc.keys {
		app.key(&mut s, &mut files, *k);
		frames.push(frame(
			mode,
			&mut ScreenJob {
				app: &app,
				s: &s,
				window: Box::new([0; WINDOW]),
			},
		));
		if app.quit() {
			break;
		}
	}
	(frames, files.files["LETTER.RTF"].clone())
}

fn golden(mode: Mode) {
	let (frames, saved) = play(mode);
	let mut sums = String::new();
	for (n, fr) in frames.iter().enumerate() {
		writeln!(sums, "frame {} crc32 {:08x}", n + 1, crc32(&fr.native)).unwrap();
	}
	compare_bytes(
		&format!("tests/golden/gfx/g6-gfx.{}.sums", mode.name()),
		sums.as_bytes(),
	);
	compare_bytes("tests/golden/files/g6-gfx.rtf", &saved);
	for (label, n) in [("start", 0), ("end", frames.len() - 1)] {
		let fr = &frames[n];
		let png = encode_indexed(640, 480, mode.depth(), &mode.palette(), &fr.pixels);
		compare_bytes(
			&format!("tests/golden/gfx/g6-gfx.{}.{label}.png", mode.name()),
			&png,
		);
	}
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
	let hs = shell::hscroll(&app);
	let (left, w) = quill_app::view::text_extent(&app, 640, hs);
	assert!(left >= 0 && left + w <= 640, "text {left}..{}", left + w);
	assert_eq!(PAGE_AREA.top, 42);
	assert_eq!(PAGE_AREA.bottom, 460);
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
		},
	);
	// The status line is silver at its left edge, and the page area is not all silver.
	assert_eq!(
		fr.pixels[479 * 640],
		0xD8 + 6,
		"silver is the extra entry 222"
	);
	let area = &fr.pixels[PAGE_AREA.top as usize * 640..PAGE_AREA.bottom as usize * 640];
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
	app.set_view_height(shell::view_height());
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
