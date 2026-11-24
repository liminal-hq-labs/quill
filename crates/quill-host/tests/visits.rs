// How many paragraphs layout visits for a typed key and for the repaint that follows it, with the layout cache.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::PathBuf;

use quill_app::{App, Key};
use quill_core::layout::Measure;
use quill_grid::Grid;
use quill_host::corpus::{generated, Rng, Var96};
use quill_host::script::{parse_script, MemFiles};
use quill_host::{GridMeasure, HostStore};

fn root() -> PathBuf {
	PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn median_and_p99(v: &mut [u32]) -> (u32, u32) {
	v.sort_unstable();
	let at = |q: usize| {
		v.get((v.len() * q / 100).min(v.len().saturating_sub(1)))
			.copied()
	};
	(at(50).unwrap_or(0), at(99).unwrap_or(0))
}

/// The keys of `m2-letter` on the text screen: paragraph visits by the key, and by the key and the repaint.
#[test]
fn a_key_of_the_letter_script_visits_few_paragraphs() {
	let sc = parse_script(
		&std::fs::read_to_string(root().join("tests/scenarios/m2-letter.keys")).unwrap(),
	);
	let mut files = MemFiles::default();
	for f in &sc.copy {
		files.files.insert(
			f.to_ascii_uppercase(),
			std::fs::read(root().join(format!("tests/docs/{}", f.to_ascii_lowercase()))).unwrap(),
		);
	}
	let mut s = HostStore::new();
	let mut app = App::new();
	app.reset(&mut s).unwrap();
	app.set_rows(23);
	app.open(&mut s, &mut files, sc.open.clone().unwrap().as_bytes())
		.unwrap();
	let mut g = Grid::<25>::new(0);
	quill_app::draw(&app, &s, &mut g);
	let (mut keys, mut both) = (Vec::new(), Vec::new());
	for k in &sc.keys {
		let v0 = app.layout_visits();
		app.key(&mut s, &mut files, *k);
		let v1 = app.layout_visits();
		quill_app::draw(&app, &s, &mut g);
		let v2 = app.layout_visits();
		keys.push(v1 - v0);
		both.push(v2 - v0);
		if app.quit() {
			break;
		}
	}
	let (km, kp) = median_and_p99(&mut keys);
	let (bm, bp) = median_and_p99(&mut both);
	eprintln!(
		"m2-letter, {} keys: key median {km} p99 {kp}; key and repaint median {bm} p99 {bp}",
		sc.keys.len()
	);
	assert!(km <= 3, "a key visits at most 3 paragraphs, median");
	assert!(
		bm <= 12,
		"the repaint of the nine paragraph letter is bounded by what is on screen"
	);
}

/// Typing 2,000 letters in a generated document of `paras` paragraphs: the key's visits.
fn typing<M: Measure + Clone>(m: M, paras: u32, at_random: bool) -> (u32, u32, u32) {
	let mut s = HostStore::new();
	let mut rng = Rng(0x514c_494c_4c20_4a36);
	let d = generated(&mut s, &mut rng, paras);
	let mut bytes = quill_host::docs::VecSink(Vec::new());
	quill_formats::qld::write(&d, &s, &mut bytes).unwrap();
	let mut files = MemFiles::default();
	files.files.insert("BIG.QLD".into(), bytes.0);
	let mut s = HostStore::new();
	let mut app = App::with_measure(m);
	app.reset(&mut s).unwrap();
	app.open(&mut s, &mut files, b"BIG.QLD").unwrap();
	app.place_caret(&s, app.doc().body.len() / 2, false);
	let mut v = Vec::new();
	for i in 0..2000u32 {
		if at_random && i % 20 == 0 {
			let at = rng.below(app.doc().body.len());
			app.place_caret(&s, at, false);
		}
		let v0 = app.layout_visits();
		app.key(&mut s, &mut files, Key::Char(b'a' + (i % 26) as u8));
		v.push(app.layout_visits() - v0);
	}
	let worst = v.iter().copied().max().unwrap_or(0);
	let (m, p) = median_and_p99(&mut v);
	(m, p, worst)
}

#[test]
fn typing_in_a_long_document_visits_a_paragraph_or_a_page_of_them() {
	for (name, paras, random) in [
		("letter-sized", 9, false),
		("300 paragraphs", 300, false),
		("300, random places", 300, true),
	] {
		let (m, p, worst) = typing(Var96, paras, random);
		eprintln!("{name} at 96 dpi: median {m}, p99 {p}, worst {worst}");
		assert!(m <= 3, "{name}: median {m}");
	}
	let (m, _, _) = typing(GridMeasure, 300, false);
	assert!(m <= 3);
}
/// The 1 MB document of the J0 spike (4,700 paragraphs): `cargo test --release -p quill-host --test visits -- --ignored --nocapture`.
#[test]
#[ignore = "slow in a debug build; prints the figures quoted in the checkpoint cache's pull request"]
fn typing_in_a_one_megabyte_document() {
	for (name, random) in [("one place", false), ("random places", true)] {
		let (m, p, worst) = typing(Var96, 4700, random);
		eprintln!("4,700 paragraphs, {name}, 96 dpi: median {m}, p99 {p}, worst {worst}");
		assert!(m <= 3);
	}
}

/// Printing lays the body out once: each paragraph is visited one time, however many pages come out.
#[test]
fn printing_makes_one_pass_over_the_document() {
	for (name, pages) in [("print3.rtf", 3), ("letter.rtf", 1), ("headers.rtf", 1)] {
		let mut s = HostStore::new();
		let mut f = MemFiles::default();
		f.files.insert(
			name.to_ascii_uppercase(),
			std::fs::read(root().join(format!("tests/docs/{name}"))).unwrap(),
		);
		let mut app = App::new();
		app.reset(&mut s).unwrap();
		app.open(&mut s, &mut f, name.to_ascii_uppercase().as_bytes())
			.unwrap();
		let mut paras = 0;
		app.for_each_line(&s, |_, l| paras = paras.max(l.para + 1));
		assert!(
			app.page_total(&s) >= pages,
			"{name} has {pages} pages or more"
		);
		let v0 = app.layout_visits();
		assert!(app.print_text(&mut s, &mut f, b"OUT.PRN"));
		assert_eq!(
			app.layout_visits() - v0,
			paras,
			"{name}: one visit to each paragraph"
		);
	}
}
