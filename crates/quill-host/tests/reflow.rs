// Property test: the layout cache equals a layout from the start after every kind of edit, on every test document and generated
// ones, on the text screen and at 96 dpi, and the queries it answers equal the ones a full pass answers.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use quill_app::{App, Command, Dir, Key};
use quill_core::doc::Doc;
use quill_core::layout::{Line, Measure};
use quill_host::corpus::{files, generated, Rng, Var96};
use quill_host::docs::VecSink;
use quill_host::script::MemFiles;
use quill_host::{GridMeasure, HostStore};

/// `line_for` as it was before the cache, over every line.
fn oracle(lines: &[Line], pos: u32) -> Option<(u32, Line)> {
	let mut found = None;
	let mut prev: Option<(u32, Line)> = None;
	for (i, l) in lines.iter().enumerate() {
		if found.is_none() {
			if let Some((pi, p)) = prev {
				let soft = l.start == p.start + p.len;
				if pos >= p.start && pos <= p.start + p.len && !(soft && pos == p.start + p.len) {
					found = Some((pi, p));
				}
			}
		}
		prev = Some((i as u32, *l));
	}
	if found.is_none() {
		if let Some((pi, p)) = prev {
			if pos >= p.start {
				found = Some((pi, p));
			}
		}
	}
	found
}

fn random_op<M: Measure + Clone>(
	app: &mut App<M>,
	s: &mut HostStore,
	f: &mut MemFiles,
	r: &mut Rng,
) {
	let key = |app: &mut App<M>, s: &mut HostStore, f: &mut MemFiles, k: Key| app.key(s, f, k);
	match r.below(30) {
		0..=9 => {
			let c = *b"ae -xQ.o".get(r.below(8) as usize).unwrap_or(&b'a');
			key(app, s, f, Key::Char(c));
		}
		10 => key(app, s, f, Key::Enter),
		11 | 12 => key(app, s, f, Key::Backspace),
		13 => key(app, s, f, Key::Delete),
		14 => key(app, s, f, Key::Tab),
		15 => key(app, s, f, Key::Left),
		16 => key(app, s, f, Key::Right),
		17 => key(app, s, f, Key::Up),
		18 => key(app, s, f, Key::Down),
		19 => key(app, s, f, Key::PageDown),
		20 => key(app, s, f, Key::Select(Dir::Right)),
		21 => {
			for _ in 0..r.below(40) {
				key(app, s, f, Key::Select(Dir::Right));
			}
		}
		22 => app.command(s, f, Command::Undo),
		23 => app.command(s, f, Command::Redo),
		24 => app.command(s, f, Command::Bold),
		25 => app.set_size(s, [16, 24, 40][r.below(3) as usize]),
		26 => app.command(s, f, Command::Cut),
		27 => app.command(s, f, Command::Paste),
		28 => app.command(s, f, Command::Bullets),
		_ => {
			let at = r.below(app.doc().body.len() + 1);
			app.place_caret(s, at, false);
		}
	}
}

/// Edits to each document; `QUILL_REFLOW_EDITS` makes it longer (10,000 takes minutes in a release build).
fn edits() -> u32 {
	std::env::var("QUILL_REFLOW_EDITS")
		.ok()
		.and_then(|v| v.parse().ok())
		.unwrap_or(300)
}

fn run<M: Measure + Clone>(m: &M, label: &str, seed: u64, edits: u32) {
	let mut rng = Rng(seed);
	let mut docs: Vec<(String, Doc, HostStore)> = files()
		.into_iter()
		.map(|(n, d, s)| (n.to_string(), d, s))
		.collect();
	for n in [3u32, 40, 250] {
		let mut s = HostStore::new();
		let d = generated(&mut s, &mut rng, n);
		docs.push((format!("generated {n}"), d, s));
	}
	for (name, d, s) in docs {
		let mut bytes = VecSink(Vec::new());
		quill_formats::qld::write(&d, &s, &mut bytes).unwrap();
		let mut s = HostStore::new();
		let mut f = MemFiles::default();
		f.files.insert("DOC.QLD".into(), bytes.0);
		let mut app = App::with_measure(m.clone());
		app.reset(&mut s).unwrap();
		app.open(&mut s, &mut f, b"DOC.QLD").unwrap();
		let at = format!("{name} ({label}, seed {seed:#x})");
		app.check_layout_cache(&s)
			.unwrap_or_else(|e| panic!("{at}: after open: {e}"));
		for n in 0..edits {
			random_op(&mut app, &mut s, &mut f, &mut rng);
			app.check_layout_cache(&s)
				.unwrap_or_else(|e| panic!("{at}: after edit {n}: {e}"));
			if n % 5 == 0 {
				let mut lines = Vec::new();
				app.for_each_line(&s, |_, l| lines.push(*l));
				assert_eq!(
					app.line_total(&s) as usize,
					lines.len(),
					"{at}: line total, edit {n}"
				);
				for pos in [
					app.caret(),
					0,
					app.doc().body.len(),
					rng.below(app.doc().body.len() + 1),
				] {
					app.place_caret(&s, pos, false);
					let got = app.caret_xy(&s).map(|(i, l, _)| (i, l));
					assert_eq!(got, oracle(&lines, pos), "{at}: line for {pos}, edit {n}");
				}
				if let Some(k) = (!lines.is_empty()).then(|| rng.below(lines.len() as u32)) {
					let mut seen = None;
					app.for_each_line(&s, |i, l| {
						if i == k {
							seen = Some(*l);
						}
					});
					assert_eq!(
						seen,
						lines.get(k as usize).copied(),
						"{at}: line {k}, edit {n}"
					);
				}
			}
		}
	}
}

#[test]
fn the_cache_equals_a_full_layout_on_the_text_screen() {
	run(&GridMeasure, "grid", 0x514c_494c_4c20_4a34, edits());
}

#[test]
fn the_cache_equals_a_full_layout_at_96_dpi() {
	run(&Var96, "96 dpi", 0x514c_494c_4c20_4a35, edits());
}

/// A story longer than one chunk of records (1,600), and one longer than the cache holds (6,400, which keeps every other paragraph).
fn long_story(paras: u32, edits: u32, every: u32) {
	let mut rng = Rng(0x514c_494c_4c20_4a37 ^ u64::from(paras));
	let mut s = HostStore::new();
	let d = generated(&mut s, &mut rng, paras);
	let mut bytes = VecSink(Vec::new());
	quill_formats::qld::write(&d, &s, &mut bytes).unwrap();
	let mut s = HostStore::new();
	let mut f = MemFiles::default();
	f.files.insert("LONG.QLD".into(), bytes.0);
	let mut app = App::with_measure(Var96);
	app.reset(&mut s).unwrap();
	app.open(&mut s, &mut f, b"LONG.QLD").unwrap();
	app.check_layout_cache(&s).unwrap();
	for n in 0..edits {
		// Edits near the start, the middle and the end, and at random: the tail that moves is long, and the records cross chunks.
		let at = match n % 4 {
			0 => 10,
			1 => app.doc().body.len() / 2,
			2 => app.doc().body.len().saturating_sub(10),
			_ => rng.below(app.doc().body.len() + 1),
		};
		app.place_caret(&s, at, false);
		for _ in 0..1 + rng.below(3) {
			random_op(&mut app, &mut s, &mut f, &mut rng);
		}
		if n % every == 0 || n + 1 == edits {
			app.check_layout_cache(&s)
				.unwrap_or_else(|e| panic!("{paras} paragraphs, after edit {n}: {e}"));
		}
	}
}

#[test]
fn the_cache_spans_chunks() {
	long_story(1700, 12, 3);
}

#[test]
#[ignore = "minutes in a debug build; run with --release"]
fn the_cache_thins_out_past_its_capacity() {
	long_story(3300, 60, 4);
	long_story(6600, 60, 5);
}
