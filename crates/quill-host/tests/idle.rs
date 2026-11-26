// Idle time: the autosave timer, the budget every pass keeps, and the promise that no key is ever lost or held up for more than one poll.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use proptest::prelude::*;
use quill_app::idle::{Autosave, Idle, Probe};
use quill_app::{App, Key};
use quill_core::doc::Doc;
use quill_formats::io::SliceSource;
use quill_formats::qld;
use quill_host::docs::VecSink;
use quill_host::script::{dos_add, MemFiles};
use quill_host::HostStore;
use quill_job::{Inline, Sim};

const DICT: &[u8] = include_bytes!("../../../tests/docs/en_ca.dic");
const BIG: &str = "C:\\DOCS\\BIG.QLD";

struct Rig {
	app: App,
	s: HostStore,
	f: MemFiles,
	/// Seconds since the last key, as a shell's timer would count them.
	quiet: u32,
}

/// A document of about `bytes` bytes of text, as the bytes of its `.QLD`.
fn big_doc(bytes: usize) -> Vec<u8> {
	let mut s = HostStore::new();
	let mut d = Doc::new();
	d.init(&mut s).unwrap();
	let mut body = Vec::new();
	while body.len() < bytes {
		body.extend_from_slice(b"the quick brown fox jumps over the lazy dog and the teh\r");
	}
	d.body.load(&mut s, &body, 0, 0).unwrap();
	let mut out = VecSink(Vec::new());
	qld::write(&d, &s, &mut out).unwrap();
	out.0
}

impl Rig {
	fn new(live: bool) -> Rig {
		let mut f = MemFiles::default();
		f.files.insert("EN_CA.DIC".to_string(), DICT.to_vec());
		let mut s = HostStore::new();
		let mut app = App::new();
		app.reset(&mut s).unwrap();
		app.set_recovery(true);
		app.set_spell_live_default(live);
		// A minute, so the scripts here need not wait ten.
		app.set_autosave_minutes(1);
		Rig {
			app,
			s,
			f,
			quiet: 0,
		}
	}

	fn with_big(live: bool, bytes: usize) -> Rig {
		let mut r = Rig::new(live);
		r.f.files.insert(BIG.to_string(), big_doc(bytes));
		r.app.open(&mut r.s, &mut r.f, BIG.as_bytes()).unwrap();
		r
	}

	fn key(&mut self, k: Key) {
		self.quiet = 0;
		self.app.key(&mut self.s, &mut self.f, k);
	}

	fn type_(&mut self, t: &str) {
		for b in t.bytes() {
			self.key(Key::Char(b));
		}
	}

	fn wait(&mut self, secs: u32) {
		self.f.clock = dos_add(self.f.clock, secs);
	}

	fn quiet_for(&mut self, secs: u32) {
		self.wait(secs);
		self.quiet += secs;
	}

	fn idle(&mut self, units: u32) -> quill_app::idle::Idled {
		self.pass(units, &mut || false)
	}

	fn pass(&mut self, units: u32, input: &mut dyn FnMut() -> bool) -> quill_app::idle::Idled {
		let len = self.app.doc().body.len();
		self.app.spell_set_view(0, len);
		let ctx = Idle {
			quiet: self.quiet,
			units,
			busy: false,
		};
		self.app.idle(&mut self.s, &mut self.f, ctx, input)
	}

	fn body(&self) -> Vec<u8> {
		let n = self.app.doc().body.len() as usize;
		let mut b = vec![0u8; n];
		let got = self.app.doc().body.read(&self.s, 0, &mut b);
		b.truncate(got);
		b
	}

	/// The recovery copies on the disk and their bytes, by name.
	fn copies(&self) -> Vec<(String, Vec<u8>)> {
		let mut v: Vec<(String, Vec<u8>)> = self
			.f
			.files
			.iter()
			.filter(|(k, _)| k.ends_with(".RCV") || k.ends_with(".RC0") || k.ends_with(".$$$"))
			.map(|(k, b)| (k.clone(), b.clone()))
			.collect();
		v.sort();
		v
	}

	/// The body of the current copy, read as an ordinary document.
	fn copy_body(&self) -> Option<Vec<u8>> {
		let (_, bytes) = self
			.copies()
			.into_iter()
			.find(|(k, _)| k.ends_with(".RCV"))?;
		let mut s = HostStore::new();
		let mut d = Doc::new();
		qld::read(&mut d, &mut s, &mut SliceSource::new(&bytes)).ok()?;
		let n = d.body.len() as usize;
		let mut b = vec![0u8; n];
		let got = d.body.read(&s, 0, &mut b);
		b.truncate(got);
		Some(b)
	}
}

// ---- when an autosave is due -------------------------------------------------------------------------------------------------------

#[test]
fn nothing_is_written_until_the_interval_and_the_pause_have_both_passed() {
	let mut r = Rig::new(false);
	r.type_("some words");
	r.quiet_for(4);
	assert!(!r.idle(8).wrote, "the interval has not passed");
	r.wait(60);
	r.quiet = 0;
	assert!(!r.idle(8).wrote, "a key was just typed");
	r.quiet_for(1);
	assert!(!r.idle(8).wrote, "the idle wait is two seconds");
	r.quiet_for(2);
	let r1 = r.idle(8);
	assert!(r1.wrote && r.copy_body().is_some());
	// Nothing changed since: nothing more is written, and nothing waits.
	let again = r.idle(8);
	assert!(!again.wrote && !again.waiting);
}

#[test]
fn a_key_never_steps_a_job() {
	let mut r = Rig::new(false);
	r.type_("a");
	r.wait(3600);
	r.quiet = 100;
	for _ in 0..50 {
		r.key(Key::Char(b'x'));
	}
	assert!(r.copies().is_empty(), "only `idle` writes a copy");
	r.quiet_for(2);
	assert!(r.idle(8).wrote);
}

#[test]
fn a_dialog_or_a_busy_shell_holds_the_autosave_off() {
	let mut r = Rig::new(false);
	r.type_("some words");
	r.wait(3600);
	r.quiet = 10;
	let busy = Idle {
		quiet: 10,
		units: 8,
		busy: true,
	};
	let out = r.app.idle(&mut r.s, &mut r.f, busy, &mut || false);
	assert!(!out.wrote && out.waiting);
	r.key(Key::Esc);
	r.quiet = 10;
	assert!(r.idle(8).wrote);
}

#[test]
fn the_debug_hook_sets_the_interval_in_seconds_and_a_one_second_wait() {
	let mut r = Rig::new(false);
	r.app.apply_setting(b"Debug", b"AutosaveSeconds", b"3");
	r.type_("x");
	r.quiet_for(2);
	assert!(!r.idle(8).wrote);
	r.quiet_for(2);
	assert!(r.idle(8).wrote);
}

#[test]
fn a_failed_write_backs_off_one_interval_and_says_so_once() {
	let mut r = Rig::new(false);
	r.f.denied.push("RECOVER".to_string());
	r.type_("x");
	r.wait(60);
	r.quiet_for(2);
	assert!(!r.idle(8).wrote);
	assert_eq!(r.app.message(), b"Quill cannot autosave: the disk is full.");
	r.app.set_message("");
	// One interval later it tries again; before that, not even a try.
	r.f.denied.clear();
	r.quiet_for(30);
	assert!(!r.idle(8).wrote, "half the interval");
	r.quiet_for(30);
	assert!(r.idle(8).wrote);
}

#[test]
fn a_save_or_a_new_document_forgets_the_interval() {
	let mut r = Rig::with_big(false, 300);
	r.type_("x");
	r.wait(3600);
	r.quiet_for(2);
	assert!(r.idle(8).wrote);
	r.type_("y");
	r.app.command(&mut r.s, &mut r.f, quill_app::Command::Save);
	assert!(r.copies().is_empty(), "a Save removes the copies");
	let out = r.idle(8);
	assert!(
		!out.wrote && !out.waiting,
		"a clean document has nothing to protect"
	);
	r.type_("z");
	r.wait(30);
	r.quiet_for(2);
	assert!(
		!r.idle(8).wrote,
		"the interval starts again at the next change"
	);
}

// ---- the write that input cuts short -----------------------------------------------------------------------------------------------

#[test]
fn a_key_cuts_a_write_short_and_leaves_the_disk_as_it_was() {
	let mut r = Rig::with_big(false, 60_000);
	r.type_("x");
	r.wait(3600);
	r.quiet_for(2);
	assert!(r.idle(8).wrote);
	let before = r.copies();
	r.type_("y");
	r.wait(3600);
	r.quiet_for(2);
	// The first poll finds a key waiting.
	let mut polls = 0;
	let out = r.pass(8, &mut || {
		polls += 1;
		polls > 1
	});
	assert!(!out.wrote);
	assert_eq!(polls, 2, "the write stops at the poll that saw the key");
	assert_eq!(
		r.copies(),
		before,
		"the copy and its temporary file are as they were"
	);
	assert!(
		out.waiting,
		"the autosave is still due, for when the keyboard is quiet"
	);
	assert!(r.idle(8).wrote, "the next pause writes it");
	assert!(r.copy_body().unwrap() == r.body());
}

#[test]
fn after_two_cut_writes_the_third_is_announced_and_runs_to_the_end() {
	let mut r = Rig::with_big(false, 60_000);
	r.type_("x");
	r.wait(3600);
	r.quiet_for(2);
	for _ in 0..2 {
		let mut polls = 0;
		let out = r.pass(8, &mut || {
			polls += 1;
			polls > 1
		});
		assert!(!out.wrote && !out.more);
	}
	// The pass that would be the third says what it is about to do, and the shell paints it; the next one writes.
	let mut asked = 0;
	let announce = r.pass(8, &mut || {
		asked += 1;
		true
	});
	assert!(announce.more && !announce.wrote);
	assert_eq!(r.app.message(), b"Autosaving BIG.QLD...");
	let done = r.pass(8, &mut || {
		asked += 1;
		true
	});
	assert!(done.wrote, "a forced write does not ask for input");
	assert_eq!(asked, 0);
	assert_eq!(r.app.message(), b"");
	assert!(r.copy_body().unwrap() == r.body());
}

// ---- the budget and the interleaving -----------------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
enum Op {
	Type(Vec<u8>),
	Back(u8),
	Enter,
	Wait(u8),
	Quiet(u8),
	Idle(u8),
	Cut(u8),
}

fn op() -> impl Strategy<Value = Op> {
	prop_oneof![
		4 => prop::collection::vec(prop_oneof![b'a'..=b'z', Just(b' ')], 1..8).prop_map(Op::Type),
		1 => (1u8..4).prop_map(Op::Back),
		1 => Just(Op::Enter),
		2 => (1u8..=60).prop_map(Op::Wait),
		3 => (1u8..=30).prop_map(Op::Quiet),
		4 => (0u8..12).prop_map(Op::Idle),
		2 => (0u8..4).prop_map(Op::Cut),
	]
}

/// Plays `ops` and returns the rig. `sliced` runs idle passes between the operations, as a schedule would; the edits and the time are
/// the same either way.
fn play(ops: &[Op], sliced: Option<u32>, check: bool) -> Rig {
	let mut r = Rig::new(true);
	let mut sim = sliced.map(Sim::new);
	let mut model = Rig::new(false);
	model.app.set_recovery(false);
	for o in ops {
		match o {
			Op::Type(t) => {
				for b in t {
					r.key(Key::Char(*b));
					model.key(Key::Char(*b));
				}
			}
			Op::Back(n) => {
				for _ in 0..*n {
					r.key(Key::Backspace);
					model.key(Key::Backspace);
				}
			}
			Op::Enter => {
				r.key(Key::Enter);
				model.key(Key::Enter);
			}
			Op::Wait(n) => r.wait(u32::from(*n) * 2),
			Op::Quiet(n) => r.quiet_for(u32::from(*n) * 2),
			Op::Idle(u) => {
				let before = r.copies();
				let out = r.idle(u32::from(*u));
				assert!(
					out.spent <= u32::from(*u),
					"spent {} of a budget of {u}",
					out.spent
				);
				if check && !out.wrote {
					assert_eq!(
						r.copies(),
						before,
						"a pass that wrote nothing changed the disk"
					);
				}
			}
			Op::Cut(after) => {
				let before = r.copies();
				let mut polls = 0u32;
				let mut calls_after_cut = 0u32;
				let out = r.pass(4, &mut || {
					polls += 1;
					if polls > u32::from(*after) {
						calls_after_cut += 1;
					}
					polls > u32::from(*after)
				});
				assert!(out.spent <= 4);
				// Once the poll says a key waits the write stops: it is never asked again.
				assert!(
					calls_after_cut <= 1,
					"{calls_after_cut} polls after a key waited"
				);
				if check && !out.wrote {
					assert_eq!(r.copies(), before, "a cut write changed the disk");
				}
				if check && out.wrote {
					assert!(r.copy_body().unwrap() == r.body());
				}
			}
		}
		if let Some(sim) = sim.as_mut() {
			for _ in 0..sim.below(4) {
				let units = sim.budget(8).left();
				let before = r.copies();
				let out = r.idle(units);
				assert!(out.spent <= units);
				if check && !out.wrote {
					assert_eq!(r.copies(), before);
				}
			}
		}
		// Idle work never reads or edits the document: what was typed is what is there.
		if check {
			assert!(r.body() == model.body(), "idle time changed the document");
		}
	}
	r
}

/// Everything idle time can do, at a time that makes all of it due.
fn settle(r: &mut Rig) {
	r.wait(7200);
	r.quiet_for(10);
	let len = r.app.doc().body.len();
	r.app.spell_set_view(0, len);
	r.app.drain(&mut r.s, &mut r.f, r.quiet);
}

fn marks(r: &Rig) -> Vec<(u32, u32)> {
	r.app.spell_marks(&r.s, 0, u32::MAX).collect()
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(120))]

	// Idle work stays inside its budget, a pass that wrote nothing leaves the disk alone, a cut write is never asked again, and no key is
	// ever lost: the document is what the keys made.
	#[test]
	fn idle_never_exceeds_its_budget_and_never_loses_input(ops in prop::collection::vec(op(), 1..50)) {
		let mut r = play(&ops, None, true);
		settle(&mut r);
		// What idle time ends with: a document with changes has its copy, and the copy is the document.
		if r.app.unsaved_work() {
			prop_assert!(r.copy_body().as_ref() == Some(&r.body()));
		}
	}

	// Slicing idle time any way at all reaches the state eager idle time reaches: the same document, the same marks and a copy of the
	// document as it is.
	#[test]
	fn any_schedule_reaches_what_eager_idle_time_reaches(
		ops in prop::collection::vec(op(), 1..50),
		seed in 1u32..u32::MAX,
	) {
		let mut eager = play(&ops, None, false);
		let mut sliced = play(&ops, Some(seed), true);
		settle(&mut eager);
		settle(&mut sliced);
		prop_assert!(eager.body() == sliced.body());
		prop_assert_eq!(marks(&eager), marks(&sliced));
		let (a, b) = (eager.copy_body(), sliced.copy_body());
		prop_assert!(a == b, "the recovery copies differ");
		if sliced.app.unsaved_work() {
			prop_assert!(b.as_ref() == Some(&sliced.body()), "the copy is the document as it is");
		}
	}
}

// ---- the job on the simulator --------------------------------------------------------------------------------------------------

#[test]
fn the_same_seed_gives_the_same_autosave_trace() {
	let run = |seed: u32| {
		let mut exec: Inline<Autosave, 1> = Inline::new();
		let mut sim = Sim::new(seed);
		let mut rev = 0u32;
		let mut done = 0;
		for round in 0..40u32 {
			if exec.is_empty() {
				exec.spawn(7, rev, Autosave).ok();
			}
			let probe = Probe {
				on: true,
				dirty: true,
				waited: round * 30,
				interval: 600,
				quiet: sim.below(4),
				wait: 2,
				plain: true,
				free: true,
			};
			done += sim.drive(&mut exec, &probe, &mut rev, 1, 4, |_, ()| {});
		}
		(sim.trace_hash(), sim.count(), done)
	};
	assert_eq!(run(5), run(5));
	assert_ne!(run(5).0, run(6).0);
	assert!(run(5).2 > 0, "the job finishes when it is due");
}
