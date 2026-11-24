// Property test: a story laid out in several calls of `layout_from`, each resuming from the cursor the one before returned, gives the
// lines of one `layout_story`, and the cursors are the state a whole layout passes through.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use quill_core::doc::{Doc, StoryKind};
use quill_core::layout::{layout_from, layout_story, LayoutCursor, Line, Measure};
use quill_host::corpus::{files, generated, Rng, Var96};
use quill_host::{GridMeasure, HostStore};

const KINDS: [StoryKind; 5] = [
	StoryKind::Body,
	StoryKind::Header,
	StoryKind::Footer,
	StoryKind::FirstHeader,
	StoryKind::FirstFooter,
];

/// Lays `kind` out whole, then again in pieces of random numbers of paragraphs, and compares everything.
fn check<M: Measure + Clone>(
	d: &Doc,
	s: &HostStore,
	m: &M,
	kind: StoryKind,
	rng: &mut Rng,
	name: &str,
) {
	let mut whole = Vec::new();
	let mut cursors = vec![LayoutCursor::START];
	let pages = layout_story(d, s, &mut m.clone(), kind, |l| whole.push(*l));
	// The cursor after every paragraph of one pass.
	let last = layout_from(
		d,
		s,
		&mut m.clone(),
		kind,
		LayoutCursor::START,
		|_| {},
		|c| {
			cursors.push(*c);
			true
		},
	);
	assert_eq!(last.page, pages, "{name} {kind:?}: pages");
	assert_eq!(
		last.line as usize,
		whole.len(),
		"{name} {kind:?}: line count"
	);
	assert_eq!(
		last.pos,
		d.story(kind).len(),
		"{name} {kind:?}: ends at the end"
	);

	for round in 0..4 {
		let mut parts: Vec<Line> = Vec::new();
		let mut at = LayoutCursor::START;
		let mut k = 0usize;
		let mut guard = 0;
		loop {
			let n = 1 + rng.below(if round == 0 { 1 } else { 5 });
			let mut left = n;
			let next = layout_from(
				d,
				s,
				&mut m.clone(),
				kind,
				at,
				|l| parts.push(*l),
				|_| {
					left -= 1;
					left > 0
				},
			);
			k += (n - left) as usize;
			// Resuming from a cursor reaches the state the whole pass had at that paragraph.
			assert_eq!(
				Some(&next),
				cursors.get(k),
				"{name} {kind:?}: cursor after paragraph {k}"
			);
			if next.pos >= d.story(kind).len() {
				break;
			}
			at = next;
			guard += 1;
			assert!(guard < 100_000);
		}
		assert_eq!(parts, whole, "{name} {kind:?}: split lines (round {round})");
	}
}

fn run<M: Measure + Clone>(m: &M, label: &str) {
	let mut rng = Rng(0x514c_4c41_594f_5554);
	let all = files();
	assert_eq!(all.len(), 7, "every test document loads");
	for (name, d, s) in all {
		for kind in KINDS {
			check(&d, &s, m, kind, &mut rng, &format!("{name} {label}"));
		}
	}
	for n in [0u32, 1, 2, 7, 60, 300] {
		let mut s = HostStore::new();
		let d = generated(&mut s, &mut rng, n);
		check(
			&d,
			&s,
			m,
			StoryKind::Body,
			&mut rng,
			&format!("generated {n} {label}"),
		);
	}
}

#[test]
fn splitting_a_layout_gives_the_same_lines_on_the_text_screen() {
	run(&GridMeasure, "grid");
}

#[test]
fn splitting_a_layout_gives_the_same_lines_at_96_dpi() {
	run(&Var96, "96 dpi");
}

#[test]
fn an_empty_story_has_the_start_cursor_and_one_page() {
	let mut s = HostStore::new();
	let d = generated(&mut s, &mut Rng(1), 0);
	let c = layout_from(
		&d,
		&s,
		&mut GridMeasure,
		StoryKind::Header,
		LayoutCursor::START,
		|_| panic!("no lines"),
		|_| true,
	);
	assert_eq!(c.pos, 0);
	assert_eq!(c.line, 0);
	assert_eq!(c.page, 1);
}
