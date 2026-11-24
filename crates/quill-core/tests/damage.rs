// Property test: what `Text::take_damage` reports covers every change made since the last call, whatever mix of inserts, deletes,
// formats, objects, undo and redo made it.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use proptest::prelude::*;
use quill_core::text::Text;
use quill_host::HostStore;

type T = Text<256, 256, 1024>;

/// One entry per character: its byte, character format, paragraph format and whether it is an object.
type Model = Vec<(u8, u16, u16, bool)>;

fn model(t: &T, s: &HostStore) -> Model {
	let mut bytes = vec![0u8; t.len() as usize];
	t.read(s, 0, &mut bytes);
	let mut fmts = Vec::new();
	t.for_each_run(s, 0, t.len(), |_, p| {
		fmts.extend(std::iter::repeat_n(
			(p.cfmt, p.pfmt, p.source == 2),
			p.len as usize,
		))
	});
	bytes
		.into_iter()
		.zip(fmts)
		.map(|(b, (c, p, o))| (b, c, p, o))
		.collect()
}

struct Rng(u64);

impl Rng {
	fn below(&mut self, n: u32) -> u32 {
		self.0 ^= self.0 << 13;
		self.0 ^= self.0 >> 7;
		self.0 ^= self.0 << 17;
		if n == 0 {
			0
		} else {
			((self.0 >> 16) as u32) % n
		}
	}
}

fn run(seed: u64) {
	let mut s = HostStore::with_limit(usize::MAX);
	let mut t = T::new();
	t.load(&mut s, &[b'a'; 40], 0, 0).unwrap();
	t.take_damage();
	let mut r = Rng(seed | 1);
	for round in 0..120 {
		let before = model(&t, &s);
		let old_len = t.len();
		let steps = 1 + r.below(4);
		for _ in 0..steps {
			let len = t.len();
			let pos = r.below(len + 1);
			let span = r.below(len.saturating_sub(pos) + 1).min(7);
			match r.below(8) {
				0..=1 => {
					let n = 1 + r.below(6);
					let b: Vec<u8> = (0..n).map(|_| b'a' + r.below(26) as u8).collect();
					let _ = t.insert(&mut s, pos, &b, r.below(3) as u16, 0, 1);
				}
				2 => {
					let _ = t.insert_object(&mut s, pos, 0, 0, 0, 1);
				}
				3 => {
					let _ = t.delete(&mut s, pos, span, 2);
				}
				4 => {
					let _ = t.set_cfmt(&mut s, pos, span, r.below(3) as u16, 3);
				}
				5 => {
					let _ = t.set_pfmt(&mut s, pos, span, r.below(3) as u16, 3);
				}
				6 => {
					let _ = t.undo(&mut s);
				}
				_ => {
					let _ = t.redo(&mut s);
				}
			}
		}
		let after = model(&t, &s);
		match t.take_damage() {
			None => assert_eq!(
				before, after,
				"no damage but the text changed (seed {seed:#x}, round {round})"
			),
			Some(d) => {
				let (lo, hi) = (d.lo as usize, d.hi as usize);
				let old_hi = (i64::from(d.hi) - (i64::from(t.len()) - i64::from(old_len))) as usize;
				assert!(
					lo <= hi && hi <= after.len(),
					"bad range {lo}..{hi} (seed {seed:#x}, round {round})"
				);
				assert!(
					lo <= old_hi && old_hi <= before.len(),
					"bad old range (seed {seed:#x}, round {round})"
				);
				assert_eq!(
					before[..lo],
					after[..lo],
					"text before the damage moved (seed {seed:#x}, round {round})"
				);
				assert_eq!(
					before[old_hi..],
					after[hi..],
					"text after the damage moved (seed {seed:#x}, round {round})"
				);
			}
		}
		assert_eq!(t.take_damage(), None);
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(120))]

	#[test]
	fn the_damage_covers_every_change(seed in any::<u64>()) {
		run(seed);
	}
}
