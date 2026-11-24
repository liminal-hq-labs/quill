// Property test: every call that changes a document moves `Doc::revision`, and no call that only reads does.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use proptest::prelude::*;
use quill_core::doc::{Doc, PageSetup, StoryKind};
use quill_host::HostStore;

const STORIES: [StoryKind; 5] = [
	StoryKind::Body,
	StoryKind::Header,
	StoryKind::Footer,
	StoryKind::FirstHeader,
	StoryKind::FirstFooter,
];

struct Rng(u64);

impl Rng {
	fn next(&mut self) -> u32 {
		self.0 ^= self.0 << 13;
		self.0 ^= self.0 >> 7;
		self.0 ^= self.0 << 17;
		(self.0 >> 16) as u32
	}
	fn below(&mut self, n: u32) -> u32 {
		self.next() % n.max(1)
	}
}

/// Everything layout can see of the document: each story's bytes, runs and objects, the page setup and the pictures.
fn content(d: &Doc, s: &HostStore) -> Vec<u8> {
	let mut out = Vec::new();
	for k in STORIES {
		let t = d.story(k);
		out.extend_from_slice(&t.len().to_le_bytes());
		let mut b = vec![0u8; t.len() as usize];
		t.read(s, 0, &mut b);
		out.extend_from_slice(&b);
		t.for_each_run(s, 0, t.len(), |_, p| {
			out.extend_from_slice(&[p.source]);
			out.extend_from_slice(&p.start.to_le_bytes());
			out.extend_from_slice(&p.len.to_le_bytes());
			out.extend_from_slice(&p.cfmt.to_le_bytes());
			out.extend_from_slice(&p.pfmt.to_le_bytes());
		});
	}
	out.extend_from_slice(format!("{:?}", d.page).as_bytes());
	out.extend_from_slice(&d.pictures.len().to_le_bytes());
	out.extend_from_slice(&d.pic_data.len().to_le_bytes());
	out
}

/// Calls every reader at random; none may move the revision.
fn read_everything(d: &Doc, s: &HostStore, r: &mut Rng) {
	for k in STORIES {
		let t = d.story(k);
		let at = r.below(t.len() + 2);
		let mut b = [0u8; 16];
		t.read(s, at, &mut b);
		let _ = (
			t.read_byte(s, at),
			t.object_at(s, at),
			t.pfmt_at(s, at),
			t.cfmt_at(s, at),
			t.can_undo(),
			t.can_redo(),
			t.undo_label(s),
			t.redo_label(s),
			t.len(),
			t.piece_count(),
			t.revision(),
		);
		t.for_each_run(s, at, 8, |_, _| {});
	}
	let mut b = [0u8; 8];
	d.picture_bytes(s, 0, 0, &mut b);
	let _ = (d.fonts.get(s, 0), d.cfmts.get(s, 0), d.pfmts.get(s, 0));
}

#[derive(Debug)]
enum Op {
	Insert,
	InsertObject,
	Delete,
	SetCfmt,
	SetPfmt,
	Undo,
	Redo,
	Group,
	ForgetHistory,
	AddPicture,
	SetPage,
	Reset,
	Init,
}

const OPS: [Op; 13] = [
	Op::Insert,
	Op::InsertObject,
	Op::Delete,
	Op::SetCfmt,
	Op::SetPfmt,
	Op::Undo,
	Op::Redo,
	Op::Group,
	Op::ForgetHistory,
	Op::AddPicture,
	Op::SetPage,
	Op::Reset,
	Op::Init,
];

fn run(seed: u64, steps: usize) {
	let mut s = HostStore::new();
	let mut d = Box::new(Doc::new());
	d.init(&mut s).unwrap();
	let mut r = Rng(seed | 1);
	for step in 0..steps {
		// Readers never move the revision, whatever the document holds.
		let before = d.revision();
		read_everything(&d, &s, &mut r);
		assert_eq!(
			d.revision(),
			before,
			"a reader moved the revision (seed {seed:#x}, step {step})"
		);

		let op = &OPS[r.below(OPS.len() as u32) as usize];
		let k = STORIES[r.below(5) as usize];
		let c0 = content(&d, &s);
		let rev0 = d.revision();
		let len = d.story(k).len();
		let pos = r.below(len + 1);
		let span = r.below(len.saturating_sub(pos) + 1).min(6);
		// Whether the call is supposed to have changed something, from its own result.
		let mut changed = false;
		match op {
			Op::Insert => {
				let n = 1 + r.below(5);
				let bytes: Vec<u8> = (0..n).map(|_| b'a' + r.below(26) as u8).collect();
				changed = d.story_mut(k).insert(&mut s, pos, &bytes, 0, 0, 1).is_ok();
			}
			Op::InsertObject => {
				changed = d
					.story_mut(k)
					.insert_object(&mut s, pos, 0, 0, 0, 1)
					.is_ok();
			}
			Op::Delete => {
				changed = span > 0 && d.story_mut(k).delete(&mut s, pos, span, 2).is_ok();
			}
			Op::SetCfmt => {
				let f = r.below(2) as u16;
				changed = span > 0 && d.story_mut(k).set_cfmt(&mut s, pos, span, f, 3).is_ok();
			}
			Op::SetPfmt => {
				let f = r.below(2) as u16;
				changed = span > 0 && d.story_mut(k).set_pfmt(&mut s, pos, span, f, 3).is_ok();
			}
			Op::Undo => changed = d.story_mut(k).undo(&mut s).unwrap_or(false),
			Op::Redo => changed = d.story_mut(k).redo(&mut s).unwrap_or(false),
			Op::Group => {
				// Opening and closing a group changes no content.
				d.story_mut(k).begin_group(4);
				d.story_mut(k).end_group();
				assert_eq!(d.revision(), rev0);
			}
			Op::ForgetHistory => {
				d.story_mut(k).forget_history(&mut s);
				assert_eq!(d.revision(), rev0);
			}
			Op::AddPicture => {
				let mut n = 0;
				changed = d
					.add_picture(&mut s, 0, 4, 4, 5000, |b| {
						n += 1;
						if n == 1 {
							b[..3].copy_from_slice(b"BMP");
							3
						} else {
							0
						}
					})
					.is_ok();
			}
			Op::SetPage => {
				let mut p = PageSetup::LETTER;
				p.margin_l += r.below(500) as i32 + 1;
				d.set_page(p);
				changed = true;
			}
			Op::Reset => {
				d.reset(&mut s);
				changed = true;
			}
			Op::Init => {
				changed = d.init(&mut s).is_ok();
			}
		}
		let c1 = content(&d, &s);
		if changed {
			assert_ne!(
				d.revision(),
				rev0,
				"{op:?} did not move the revision (seed {seed:#x}, step {step})"
			);
		}
		// The converse is what a cache relies on: new content never has the old revision.
		if c1 != c0 {
			assert_ne!(
				d.revision(),
				rev0,
				"{op:?} changed the content but not the revision (seed {seed:#x}, step {step})"
			);
		}
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(64))]

	#[test]
	fn mutators_bump_and_readers_do_not(seed in any::<u64>()) {
		run(seed, 150);
	}
}

#[test]
fn the_revision_wraps() {
	let mut s = HostStore::new();
	let mut d = Box::new(Doc::new());
	d.init(&mut s).unwrap();
	let r0 = d.revision();
	for _ in 0..3 {
		d.set_page(PageSetup::LETTER);
	}
	assert_eq!(d.revision(), r0.wrapping_add(3));
}
