// Property tests: FarVec and the piece table against plain Vec and String models.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use proptest::prelude::*;
use quill_core::farvec::FarVec;
use quill_core::text::Text;
use quill_host::HostStore;

#[derive(Clone, Debug)]
enum VecOp {
	Insert(u16, u16),
	Remove(u16),
	Set(u16, u16),
	Pop,
}

fn vec_op() -> impl Strategy<Value = VecOp> {
	prop_oneof![
		(any::<u16>(), any::<u16>()).prop_map(|(i, v)| VecOp::Insert(i, v)),
		any::<u16>().prop_map(VecOp::Remove),
		(any::<u16>(), any::<u16>()).prop_map(|(i, v)| VecOp::Set(i, v)),
		Just(VecOp::Pop),
	]
}

fn same_vec(v: &FarVec<u16, 64>, s: &HostStore, m: &[u16]) {
	assert_eq!(v.len() as usize, m.len());
	for (i, x) in m.iter().enumerate() {
		assert_eq!(v.get(s, i as u32), Some(*x), "element {i}");
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(200))]

	// Chunks of 8 bytes hold four elements, so every few operations cross a chunk edge.
	#[test]
	fn farvec_matches_vec(ops in proptest::collection::vec(vec_op(), 0..120)) {
		let mut s = HostStore::new();
		let mut v: FarVec<u16, 64> = FarVec::with_chunk_bytes(8);
		let mut m: Vec<u16> = Vec::new();
		for op in ops {
			match op {
				VecOp::Insert(i, x) => {
					let i = usize::from(i) % (m.len() + 1);
					if v.insert(&mut s, i as u32, x).is_ok() {
						m.insert(i, x);
					}
				}
				VecOp::Remove(i) if !m.is_empty() => {
					let i = usize::from(i) % m.len();
					prop_assert_eq!(v.remove(&mut s, i as u32), Some(m.remove(i)));
				}
				VecOp::Set(i, x) if !m.is_empty() => {
					let i = usize::from(i) % m.len();
					v.set(&mut s, i as u32, x).unwrap();
					m[i] = x;
				}
				VecOp::Pop => prop_assert_eq!(v.pop(&mut s), m.pop()),
				_ => {}
			}
			same_vec(&v, &s, &m);
		}
		v.clear(&mut s);
		prop_assert_eq!(s.live_chunks(), 0);
	}

	#[test]
	fn farvec_full_chunks_keep_their_content_when_memory_runs_out(n in 1usize..40, limit in 1usize..8) {
		let mut s = HostStore::with_limit(limit);
		let mut v: FarVec<u16, 64> = FarVec::with_chunk_bytes(8);
		let mut m: Vec<u16> = Vec::new();
		for i in 0..n {
			if v.insert(&mut s, 0, i as u16).is_ok() {
				m.insert(0, i as u16);
			}
			same_vec(&v, &s, &m);
		}
	}
}

type T = Text<128, 128, 512>;

#[derive(Clone, Debug)]
enum TextOp {
	Insert(u16, Vec<u8>, u8),
	Delete(u16, u16),
	Format(u16, u16, u8),
	ParaFormat(u16, u16, u8),
}

fn text_op() -> impl Strategy<Value = TextOp> {
	prop_oneof![
		3 => (any::<u16>(), proptest::collection::vec(0x20u8..0x7f, 1..12), 0u8..4).prop_map(|(p, b, f)| TextOp::Insert(p, b, f)),
		2 => (any::<u16>(), any::<u16>()).prop_map(|(p, l)| TextOp::Delete(p, l)),
		2 => (any::<u16>(), any::<u16>(), 0u8..4).prop_map(|(p, l, f)| TextOp::Format(p, l, f)),
		1 => (any::<u16>(), any::<u16>(), 0u8..4).prop_map(|(p, l, f)| TextOp::ParaFormat(p, l, f)),
	]
}

/// The model: one (byte, character format, paragraph format) triple per character.
type Model = Vec<(u8, u16, u16)>;

fn snapshot(t: &T, s: &HostStore) -> Model {
	let mut out = vec![0u8; t.len() as usize];
	assert_eq!(t.read(s, 0, &mut out), out.len());
	let mut fmts = Vec::new();
	t.for_each_run(s, 0, t.len(), |_, p| {
		fmts.extend(std::iter::repeat_n((p.cfmt, p.pfmt), p.len as usize))
	});
	assert_eq!(fmts.len(), out.len());
	out.into_iter()
		.zip(fmts)
		.map(|(b, (c, p))| (b, c, p))
		.collect()
}

/// Applies an edit and says whether it did anything (and so made an undo step).
fn apply(t: &mut T, s: &mut HostStore, m: &mut Model, op: &TextOp) -> bool {
	match op {
		TextOp::Insert(p, b, f) => {
			let pos = usize::from(*p) % (m.len() + 1);
			t.insert(s, pos as u32, b, u16::from(*f), 0, 1).unwrap();
			let ins: Vec<_> = b.iter().map(|c| (*c, u16::from(*f), 0)).collect();
			m.splice(pos..pos, ins);
			true
		}
		TextOp::Delete(p, l) if !m.is_empty() => {
			let pos = usize::from(*p) % m.len();
			let len = 1 + usize::from(*l) % (m.len() - pos);
			t.delete(s, pos as u32, len as u32, 2).unwrap();
			m.drain(pos..pos + len);
			true
		}
		TextOp::Format(p, l, f) if !m.is_empty() => {
			let pos = usize::from(*p) % m.len();
			let len = 1 + usize::from(*l) % (m.len() - pos);
			t.set_cfmt(s, pos as u32, len as u32, u16::from(*f), 3)
				.unwrap();
			for c in &mut m[pos..pos + len] {
				c.1 = u16::from(*f);
			}
			true
		}
		TextOp::ParaFormat(p, l, f) if !m.is_empty() => {
			let pos = usize::from(*p) % m.len();
			let len = 1 + usize::from(*l) % (m.len() - pos);
			t.set_pfmt(s, pos as u32, len as u32, u16::from(*f), 4)
				.unwrap();
			for c in &mut m[pos..pos + len] {
				c.2 = u16::from(*f);
			}
			true
		}
		_ => false,
	}
}

fn new_text(s: &mut HostStore, original: &[u8]) -> (T, Model) {
	let mut t = T::with_chunk_bytes(96);
	t.load(s, original, 0, 0).unwrap();
	(t, original.iter().map(|c| (*c, 0, 0)).collect())
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(200))]

	// After any sequence the text and formats match the model, undoing everything gives the original and redoing
	// everything gives the end state.
	#[test]
	fn piece_table_matches_model_and_undoes_everything(
		original in proptest::collection::vec(0x20u8..0x7f, 0..60),
		ops in proptest::collection::vec(text_op(), 0..40),
	) {
		let mut s = HostStore::new();
		let (mut t, mut m) = new_text(&mut s, &original);
		let start = m.clone();
		let mut states = vec![m.clone()];
		for op in &ops {
			if apply(&mut t, &mut s, &mut m, op) {
				prop_assert_eq!(snapshot(&t, &s), m.clone());
				states.push(m.clone());
			}
		}
		let end = m.clone();
		// Undo each step and compare with the state before it.
		let mut k = states.len() - 1;
		while t.can_undo() {
			k -= 1;
			prop_assert!(t.undo(&mut s).unwrap());
			prop_assert_eq!(snapshot(&t, &s), states[k].clone());
		}
		prop_assert_eq!(snapshot(&t, &s), start);
		while t.can_redo() {
			prop_assert!(t.redo(&mut s).unwrap());
			k += 1;
			prop_assert_eq!(snapshot(&t, &s), states[k].clone());
		}
		prop_assert_eq!(snapshot(&t, &s), end);
	}

	// Undo then a new edit discards the redo steps; the text stays a model of what was done.
	#[test]
	fn a_new_edit_after_undo_drops_redo(
		ops in proptest::collection::vec(text_op(), 2..20),
		extra in text_op(),
	) {
		let mut s = HostStore::new();
		let (mut t, mut m) = new_text(&mut s, b"The quick brown fox");
		let mut states = vec![m.clone()];
		for op in &ops {
			if apply(&mut t, &mut s, &mut m, op) {
				states.push(m.clone());
			}
		}
		if states.len() < 2 {
			return Ok(());
		}
		t.undo(&mut s).unwrap();
		m = states[states.len() - 2].clone();
		prop_assert_eq!(snapshot(&t, &s), m.clone());
		if apply(&mut t, &mut s, &mut m, &extra) {
			prop_assert!(!t.can_redo(), "a new edit drops the redo steps");
		}
		prop_assert_eq!(snapshot(&t, &s), m);
	}

	// An edit that runs out of memory leaves the text exactly as it was.
	#[test]
	fn out_of_memory_leaves_the_text_unchanged(
		ops in proptest::collection::vec(text_op(), 1..30),
		limit in 6usize..40,
	) {
		let mut s = HostStore::new();
		let (mut t, mut m) = new_text(&mut s, b"Some original text for the table.");
		s.set_limit(Some(s.live_chunks() + limit - 6));
		for op in &ops {
			let before = snapshot(&t, &s);
			let r = match op {
				TextOp::Insert(p, b, f) => t.insert(&mut s, u32::from(*p) % (before.len() as u32 + 1), b, u16::from(*f), 0, 1),
				TextOp::Delete(p, l) if !before.is_empty() => {
					let pos = u32::from(*p) % before.len() as u32;
					t.delete(&mut s, pos, 1 + u32::from(*l) % (before.len() as u32 - pos), 2)
				}
				TextOp::Format(p, l, f) | TextOp::ParaFormat(p, l, f) if !before.is_empty() => {
					let pos = u32::from(*p) % before.len() as u32;
					t.set_cfmt(&mut s, pos, 1 + u32::from(*l) % (before.len() as u32 - pos), u16::from(*f), 3)
				}
				_ => Ok(()),
			};
			if r.is_err() {
				prop_assert_eq!(snapshot(&t, &s), before);
			}
			m = snapshot(&t, &s);
		}
		let _ = m;
	}
}

#[test]
fn typing_extends_one_piece_and_undo_groups() {
	let mut s = HostStore::new();
	let mut t = T::new();
	t.begin_group(7);
	for (i, c) in b"hello".iter().enumerate() {
		t.insert(&mut s, i as u32, &[*c], 0, 0, 7).unwrap();
	}
	t.end_group();
	assert_eq!(t.piece_count(), 1, "typing coalesces into one piece");
	assert_eq!(t.undo_label(&s), Some(7));
	assert!(t.undo(&mut s).unwrap());
	assert_eq!(t.len(), 0, "the whole group undoes as one step");
	assert!(t.redo(&mut s).unwrap());
	assert_eq!(t.len(), 5);
}

#[test]
fn memory_is_released() {
	let mut s = HostStore::new();
	let mut t = T::with_chunk_bytes(96);
	t.load(&mut s, &[b'x'; 500], 0, 0).unwrap();
	t.insert(&mut s, 250, b"abc", 1, 0, 1).unwrap();
	t.delete(&mut s, 10, 400, 2).unwrap();
	t.clear(&mut s);
	assert_eq!(s.live_chunks(), 0);
}
