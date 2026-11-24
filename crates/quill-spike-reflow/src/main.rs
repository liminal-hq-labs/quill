// Spike J0's report: prints the measurements docs/reflow-spike.md quotes. Run it with
// `cargo run --release -p quill-spike-reflow` (it plays real `App` runs, so build in release for the timings).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::time::Instant;

use quill_core::doc::StoryKind;
use quill_core::layout::reflow_spike::{EditNote, Reflow};
use quill_core::layout::{layout_story, Measure};
use quill_grid::GridMeasure;
use quill_host::HostStore;
use quill_spike_reflow::counting::{Counting, Prop96, Screen96};
use quill_spike_reflow::rng::{seed, Rng};
use quill_spike_reflow::scenario::{self, Paint, Row};
use quill_spike_reflow::{corpus, counting};

fn us(v: f64) -> String {
	if v >= 1000.0 {
		format!("{:.2} ms", v / 1000.0)
	} else {
		format!("{v:.1} us")
	}
}

fn sites(t: &counting::Tally) {
	for (site, n) in &t.by_site {
		println!("    {n:4} x {site}");
	}
	println!(
		"    layout_story {} / walk_line {} / other {}",
		t.layout_story, t.walk_line, t.other
	);
}

fn rows_summary(label: &str, rows: &[Row]) {
	let (_, _, ku, pu, paras) = scenario::summarise(rows);
	println!(
		"| {label} | {} | {paras} | {} | {} |",
		rows.len(),
		us(ku),
		us(pu)
	);
}

/// Types `keys` characters at the middle of a document on the prototype and reports paragraph visits and time per key.
fn after<M: Measure>(
	label: &str,
	m: &mut M,
	s: &mut HostStore,
	d: &mut quill_core::doc::Doc,
	keys: u32,
	scatter: bool,
) {
	let kind = StoryKind::Body;
	let t0 = Instant::now();
	let mut r = Reflow::build(d, s, m, kind);
	let build = t0.elapsed();
	let mut rng = Rng(seed());
	let mut at = d.body.len() / 2;
	let (mut visits, mut max, mut caret_visits) = (0u64, 0u32, 0u64);
	let mut hist = [0u32; 6];
	let mut spent = 0.0;
	for i in 0..keys {
		if scatter {
			at = rng.below32(d.body.len());
		}
		let c = b'a' + (i % 26) as u8;
		let t = Instant::now();
		d.body.insert(s, at, &[c], 0, 0, 0).unwrap();
		let rep = r.repair(
			d,
			s,
			m,
			kind,
			EditNote {
				at,
				removed: 0,
				inserted: 1,
				dirty_end: at + 1,
			},
		);
		let (_, extra) = r.line_at(d, s, m, kind, at + 1);
		spent += t.elapsed().as_secs_f64() * 1e6;
		at += 1;
		let v = rep.visits + extra;
		visits += u64::from(v);
		caret_visits += u64::from(extra);
		max = max.max(v);
		hist[(v as usize).min(5)] += 1;
	}
	println!(
		"| {label} | {} | {:.0} KB build once ({:.1} ms) | {:.2} | {max} | {} / {} / {} / {} / {}+ | {} | {} |",
		r.cps.len() - 1,
		r.bytes() as f64 / 1024.0,
		build.as_secs_f64() * 1e3,
		visits as f64 / f64::from(keys),
		hist[1] + hist[0],
		hist[2],
		hist[3],
		hist[4],
		hist[5],
		us(spent / f64::from(keys)),
		caret_visits
	);
}

fn main() {
	// `reflow-spike trace` (build in dev, so the backtraces keep their names) classifies who asks for each layout; plain runs time.
	let trace = std::env::args().nth(1).as_deref() == Some("trace");
	println!("seed {:#x}\n", seed());
	if trace {
		traced();
		return;
	}

	let (timed, _) = scenario::letter_script(false);
	let (_, _, ku, pu, _) = scenario::summarise(&timed);
	println!(
		"m2-letter, {} keys: host time per key {} and per paint {}\n",
		timed.len(),
		us(ku),
		us(pu)
	);

	// Typing: before.
	let letter = std::fs::read(corpus::root().join("tests/docs/letter.rtf")).unwrap();
	let mut hs = HostStore::new();
	let big = corpus::big_doc(&mut hs);
	let big_bytes = corpus::qld_bytes(&big, &hs);
	println!("## (a) typing 40 keys at the middle of the document: before (real App)");
	println!("| run | keys | paragraphs | host time per key | host time per paint |");
	println!("|---|---|---|---|---|");
	for (name, bytes, label) in [
		("LETTER.RTF", letter.clone(), "letter.rtf"),
		("BIG.QLD", big_bytes.clone(), "1 MB"),
	] {
		for paint in [Paint::Text, Paint::Page] {
			let timed = if paint == Paint::Text {
				let (m, t) = Counting::new(GridMeasure, false);
				scenario::typing(m, &t.clone(), name, bytes.clone(), 40, paint)
			} else {
				let (m, t) = Counting::new(Screen96, false);
				scenario::typing(m, &t.clone(), name, bytes.clone(), 40, paint)
			};
			rows_summary(&format!("{label}, {paint:?} paint"), &timed);
		}
	}

	// One full pass, timed.
	println!("\n## One full layout_story pass");
	let mut hs2 = HostStore::new();
	let mut b2 = corpus::big_doc(&mut hs2);
	for (label, grid) in [("GridMeasure", true), ("Prop96", false)] {
		let t = Instant::now();
		let n = 5;
		let mut lines = 0;
		for _ in 0..n {
			lines = 0;
			if grid {
				layout_story(&b2, &hs2, &mut GridMeasure, StoryKind::Body, |_| lines += 1);
			} else {
				layout_story(&b2, &hs2, &mut Prop96, StoryKind::Body, |_| lines += 1);
			}
		}
		println!(
			"  {label}: {:.2} ms per pass over {lines} lines",
			t.elapsed().as_secs_f64() * 1e3 / f64::from(n)
		);
	}

	// (b) After.
	println!("\n## (b) after: typed keys through the checkpoint cache");
	println!("| document | paragraphs | checkpoints | paragraph visits per key (edit repair plus caret line) | most in one key | keys with 1 / 2 / 3 / 4 / 5+ visits | host time per key | caret-line visits |");
	println!("|---|---|---|---|---|---|---|---|");
	let mut s = HostStore::new();
	let mut d = corpus::load(&mut s, "letter.rtf");
	after(
		"letter.rtf, text screen, typing in one place",
		&mut GridMeasure,
		&mut s,
		&mut d,
		200,
		false,
	);
	let mut s = HostStore::new();
	let mut d = corpus::load(&mut s, "letter.rtf");
	after(
		"letter.rtf, 96 dpi, typing in one place",
		&mut Prop96,
		&mut s,
		&mut d,
		200,
		false,
	);
	after(
		"1 MB, 96 dpi, typing in one place",
		&mut Prop96,
		&mut hs2,
		&mut b2,
		2000,
		false,
	);
	let mut hs3 = HostStore::new();
	let mut b3 = corpus::big_doc(&mut hs3);
	after(
		"1 MB, 96 dpi, a key at a random place each time",
		&mut Prop96,
		&mut hs3,
		&mut b3,
		2000,
		true,
	);
	let mut hs4 = HostStore::new();
	let mut b4 = corpus::big_doc(&mut hs4);
	after(
		"1 MB, text screen, typing in one place",
		&mut GridMeasure,
		&mut hs4,
		&mut b4,
		2000,
		false,
	);

	// Screenful for painting.
	let mut m = Prop96;
	let mut r = Reflow::build(&b2, &hs2, &mut m, StoryKind::Body);
	let from = r.para_of(b2.body.len() / 2);
	let t = Instant::now();
	let mut total = 0;
	let mut lines = 0;
	for _ in 0..200 {
		let (l, n) = r.window(&b2, &hs2, &mut m, StoryKind::Body, from, 4);
		total += n;
		lines = l.len();
	}
	println!(
		"\n  a window of 4 paragraphs from a checkpoint ({lines} lines, 4 paragraphs of about 200 characters at 96 dpi): {} paragraph visits and {} per window",
		total / 200,
		us(t.elapsed().as_secs_f64() * 1e6 / 200.0)
	);
}

/// The traced runs: which `App` method asks for each `layout_story`. Run in a dev build.
fn traced() {
	println!("## (a) m2-letter, text screen, traced (which App method asks for each layout_story)");
	let (rows, tally) = scenario::letter_script(true);
	sites(&tally);
	let keys = rows.len();
	let kp: u64 = rows.iter().map(|r| r.key_passes).sum();
	let pp: u64 = rows.iter().map(|r| r.paint_passes).sum();
	println!("  {keys} keys: {kp} passes in keys, {pp} in paints ({:.2} and {:.2} per key); paragraph range {}..{}\n", kp as f64 / keys as f64, pp as f64 / keys as f64, rows.iter().map(|r| r.paras).min().unwrap(), rows.iter().map(|r| r.paras).max().unwrap());

	let letter = std::fs::read(corpus::root().join("tests/docs/letter.rtf")).unwrap();
	let mut hs = HostStore::new();
	let big = corpus::big_doc(&mut hs);
	let big_bytes = corpus::qld_bytes(&big, &hs);
	for (name, bytes, label, n) in [
		("LETTER.RTF", letter, "letter.rtf", 40),
		("BIG.QLD", big_bytes, "1 MB", 6),
	] {
		for paint in [Paint::Text, Paint::Page] {
			let (rows, t) = if paint == Paint::Text {
				let (m, t) = Counting::new(GridMeasure, true);
				(
					scenario::typing(m, &t.clone(), name, bytes.clone(), n, paint),
					t,
				)
			} else {
				let (m, t) = Counting::new(Screen96, true);
				(
					scenario::typing(m, &t.clone(), name, bytes.clone(), n, paint),
					t,
				)
			};
			let (kp, pp, _, _, paras) = scenario::summarise(&rows);
			println!("typing {n} keys in {label}, {paint:?} paint: layout_story per key {kp:.2}, per paint {pp:.2}, {paras} paragraphs");
			sites(&t.borrow());
		}
	}
}
