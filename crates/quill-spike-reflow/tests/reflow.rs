// Spike J0 (b): the checkpoint cache equals a full layout, line for line, after 10,000 random edits, on every file in tests/docs.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use quill_core::doc::StoryKind;
use quill_core::layout::reflow_spike::{layout_from, Cp};
use quill_grid::GridMeasure;
use quill_host::HostStore;
use quill_spike_reflow::corpus;
use quill_spike_reflow::counting::Prop96;
use quill_spike_reflow::prove::{compare, full_lines, run};
use quill_spike_reflow::rng::seed;

const EDITS: u32 = 10_000;

#[test]
fn resuming_from_the_start_is_layout_story_for_every_story_of_every_file() {
	for name in corpus::FILES {
		let mut s = HostStore::new();
		let d = corpus::load(&mut s, name);
		for kind in [
			StoryKind::Body,
			StoryKind::Header,
			StoryKind::Footer,
			StoryKind::FirstHeader,
			StoryKind::FirstFooter,
		] {
			let mut m = Prop96;
			let (want, pages) = full_lines(&d, &s, &mut m, kind);
			let mut got = Vec::new();
			let end = layout_from(&d, &s, &mut m, kind, Cp::START, |l| got.push(*l), |_| true);
			assert_eq!(got, want, "{name} {kind:?}");
			assert_eq!(end, pages, "{name} {kind:?}");
		}
	}
}

fn prove_file(name: &str) {
	let seed = seed();
	println!("seed {seed:#x} ({name})");
	for (label, grid) in [("grid", true), ("prop96", false)] {
		let mut s = HostStore::new();
		let mut d = corpus::load(&mut s, name);
		let res = if grid {
			run(name, seed, &mut d, &mut s, &mut GridMeasure, EDITS, 5, true)
		} else {
			run(name, seed, &mut d, &mut s, &mut Prop96, EDITS, 5, true)
		};
		match res {
			Ok(st) => println!(
				"{name} {label}: {} edits, {} paragraphs at the end, {:.2} visits per edit, max {}, converged {}",
				st.edits,
				st.paras,
				st.visits as f64 / f64::from(st.edits),
				st.max_visits,
				st.converged
			),
			Err(e) => panic!("{e}"),
		}
	}
}

#[test]
fn letter_qld() {
	prove_file("letter.qld");
}
#[test]
fn plain_qld() {
	prove_file("plain.qld");
}
#[test]
fn headers_rtf() {
	prove_file("headers.rtf");
}
#[test]
fn letter_rtf() {
	prove_file("letter.rtf");
}
#[test]
fn lists_rtf() {
	prove_file("lists.rtf");
}
#[test]
fn pictures_rtf() {
	prove_file("pictures.rtf");
}
#[test]
fn print3_rtf() {
	prove_file("print3.rtf");
}

#[test]
fn the_generated_megabyte_document() {
	let seed = seed();
	let mut s = HostStore::new();
	let mut d = corpus::big_doc(&mut s);
	let res = run("big", seed, &mut d, &mut s, &mut Prop96, 300, 25, true);
	let st = res.unwrap_or_else(|e| panic!("{e}"));
	println!(
		"big: {} paragraphs, {:.2} visits per edit, max {}",
		st.paras,
		st.visits as f64 / f64::from(st.edits),
		st.max_visits
	);
	let mut m = Prop96;
	let r = quill_core::layout::reflow_spike::Reflow::build(&d, &s, &mut m, StoryKind::Body);
	assert!(compare(&d, &s, &mut m, StoryKind::Body, &r).is_none());
}
