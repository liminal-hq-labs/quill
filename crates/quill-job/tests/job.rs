// The job crate: budgets, tickets, cancellation, the inline executor and the simulator.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use quill_job::{Budget, Cancel, Event, Inline, Poll, Sim, Task, Ticket, TRACE_LEN};

/// Counts to `target` one unit at a time and gives back the target.
struct Count {
	at: u32,
	target: u32,
	survives: bool,
}

impl Count {
	fn to(target: u32) -> Self {
		Count {
			at: 0,
			target,
			survives: false,
		}
	}
}

impl Task<u32> for Count {
	type Out = u32;

	fn step(&mut self, view: &u32, b: &mut Budget) -> Poll<u32> {
		while self.at < self.target {
			if !b.take(1) {
				return Poll::Pending;
			}
			self.at += 1;
		}
		Poll::Ready(self.target + *view)
	}

	fn survives_edits(&self) -> bool {
		self.survives
	}
}

/// Waits for something that never comes, spending nothing.
struct Wait;

impl Task<u32> for Wait {
	type Out = u32;

	fn step(&mut self, _: &u32, _: &mut Budget) -> Poll<u32> {
		Poll::Pending
	}
}

enum Job {
	Count(Count),
	Wait(Wait),
}

impl Task<u32> for Job {
	type Out = u32;

	fn step(&mut self, view: &u32, b: &mut Budget) -> Poll<u32> {
		match self {
			Job::Count(c) => c.step(view, b),
			Job::Wait(w) => w.step(view, b),
		}
	}

	fn survives_edits(&self) -> bool {
		match self {
			Job::Count(c) => c.survives_edits(),
			Job::Wait(w) => w.survives_edits(),
		}
	}
}

#[test]
fn a_budget_spends_down_and_refuses_what_it_cannot_pay() {
	let mut b = Budget::new(10);
	assert!(b.take(4));
	assert_eq!(b.left(), 6);
	assert!(!b.spent());
	assert!(b.take(6));
	assert!(b.spent());
	assert!(!b.take(1));
	let mut b = Budget::new(3);
	assert!(!b.take(5), "too much is refused");
	assert!(b.spent(), "and spends what was left");
	assert!(Budget::none().spent());
}

#[test]
fn poll_maps_a_result_and_leaves_pending() {
	assert_eq!(Poll::Ready(2).map(|n| n + 1), Poll::Ready(3));
	assert_eq!(Poll::<u32>::Pending.map(|n| n + 1), Poll::Pending);
	assert!(Poll::Ready(()).is_ready());
	assert!(!Poll::<()>::Pending.is_ready());
}

#[test]
fn a_ticket_is_current_at_its_revision_only() {
	let t = Ticket {
		kind: 1,
		slot: 0,
		rev: 7,
	};
	assert!(t.is_current(7));
	assert!(!t.is_current(8));
}

#[test]
fn cancelling_outdates_every_earlier_token_and_no_later_one() {
	let c = Cancel::new();
	let old = c.token();
	assert!(!c.is_cancelled(old));
	let now = c.cancel();
	assert!(c.is_cancelled(old));
	assert_eq!(c.token(), now);
	assert!(!c.is_cancelled(now));
}

#[test]
fn the_executor_finishes_a_task_over_several_passes() {
	let mut e: Inline<Job, 2> = Inline::new();
	let t = e.spawn(1, 0, Job::Count(Count::to(10))).ok().unwrap();
	let mut got = Vec::new();
	for _ in 0..3 {
		let mut b = Budget::new(4);
		e.run(&100, 0, &mut b, |t, v| got.push((t, v)));
	}
	assert_eq!(got, vec![(t, 110)]);
	assert!(e.is_empty());
}

#[test]
fn slots_run_in_order_and_a_full_executor_gives_the_task_back() {
	let mut e: Inline<Job, 2> = Inline::new();
	e.spawn(1, 0, Job::Count(Count::to(3))).ok().unwrap();
	e.spawn(2, 0, Job::Count(Count::to(3))).ok().unwrap();
	assert!(e.spawn(3, 0, Job::Count(Count::to(1))).is_err());
	let mut order = Vec::new();
	let mut b = Budget::new(100);
	assert_eq!(e.run(&0, 0, &mut b, |t, _| order.push(t.kind)), 2);
	assert_eq!(order, vec![1, 2]);
	assert_eq!(b.left(), 94);
}

#[test]
fn a_task_that_waits_does_not_hold_up_the_one_after_it() {
	let mut e: Inline<Job, 3> = Inline::new();
	e.spawn(1, 0, Job::Wait(Wait)).ok().unwrap();
	e.spawn(2, 0, Job::Count(Count::to(5))).ok().unwrap();
	let mut b = Budget::new(50);
	let mut done = Vec::new();
	assert_eq!(e.run(&0, 0, &mut b, |t, _| done.push(t.kind)), 1);
	assert_eq!(done, vec![2]);
	assert_eq!(e.len(), 1);
}

#[test]
fn an_edit_drops_the_tasks_that_do_not_survive_it() {
	let mut e: Inline<Job, 3> = Inline::new();
	e.spawn(1, 4, Job::Count(Count::to(2))).ok().unwrap();
	let mut keep = Count::to(2);
	keep.survives = true;
	e.spawn(2, 4, Job::Count(keep)).ok().unwrap();
	let mut done = Vec::new();
	let mut b = Budget::new(10);
	// The document is at revision 5 now.
	e.run(&0, 5, &mut b, |t, _| done.push(t.kind));
	assert_eq!(done, vec![2]);
	assert!(e.is_empty());
}

#[test]
fn cancel_by_ticket_and_by_kind() {
	let mut e: Inline<Job, 4> = Inline::new();
	let a = e.spawn(1, 0, Job::Wait(Wait)).ok().unwrap();
	e.spawn(2, 0, Job::Wait(Wait)).ok().unwrap();
	e.spawn(2, 0, Job::Wait(Wait)).ok().unwrap();
	assert!(e.cancel(a));
	assert!(!e.cancel(a), "gone");
	assert_eq!(e.cancel_kind(2), 2);
	assert!(e.is_empty());
	// A slot that was reused is not cancelled by the old ticket.
	let b = e.spawn(1, 0, Job::Wait(Wait)).ok().unwrap();
	assert_eq!(b.slot, a.slot);
	assert!(!e.cancel(Ticket { rev: 9, ..a }));
	assert!(e.cancel(b));
}

#[test]
fn spawn_at_keeps_slots_for_kinds_in_a_fixed_order() {
	let mut e: Inline<Job, 4> = Inline::new();
	let t = e.spawn_at(2, 9, 0, Job::Wait(Wait)).ok().unwrap();
	assert_eq!(t.slot, 2);
	let u = e.spawn_at(2, 9, 0, Job::Wait(Wait)).ok().unwrap();
	assert_eq!(u.slot, 3);
	assert!(e.spawn_at(2, 9, 0, Job::Wait(Wait)).is_err());
}

/// Runs a seeded simulation and returns its hash, its count, its ring and what finished.
fn sim_run(seed: u32) -> (u32, u32, Vec<Event>, Vec<(Ticket, u32)>, u32) {
	let mut sim = Sim::new(seed);
	let mut e: Inline<Job, 4> = Inline::new();
	let mut rev = 0u32;
	let mut finished = Vec::new();
	for round in 0..40 {
		for k in 0..2u8 {
			let target = 1 + sim.below(30);
			let mut c = Count::to(target);
			c.survives = k == 1;
			let _ = e.spawn(k, rev, Job::Count(c));
		}
		sim.drive(&mut e, &round, &mut rev, 3, 12, |t, v| {
			finished.push((t, v));
		});
	}
	(
		sim.trace_hash(),
		sim.count(),
		sim.recent().collect(),
		finished,
		rev,
	)
}

#[test]
fn the_same_seed_gives_the_same_trace() {
	for seed in [0, 1, 2, 3, 42, 0xDEAD_BEEF] {
		let a = sim_run(seed);
		let b = sim_run(seed);
		assert_eq!(a, b, "seed {seed}");
		assert!(a.1 > 0 && !a.3.is_empty());
		assert!(a.2.len() <= TRACE_LEN);
	}
}

#[test]
fn different_seeds_give_different_traces() {
	let hashes: Vec<u32> = (1..=20).map(|s| sim_run(s).0).collect();
	let mut sorted = hashes.clone();
	sorted.sort_unstable();
	sorted.dedup();
	assert!(sorted.len() >= 19, "{hashes:?}");
}

#[test]
fn the_sequence_is_fixed() {
	// A change to the generator would change every recorded seed, so its first values are pinned.
	let mut s = Sim::new(1);
	let v: Vec<u32> = (0..4).map(|_| s.next_u32()).collect();
	assert_eq!(v, vec![270_369, 67_634_689, 2_647_435_461, 307_599_695]);
	assert_eq!(Sim::new(0).next_u32(), Sim::new(0).next_u32());
	assert_ne!(Sim::new(0).next_u32(), 0);
}

#[test]
fn every_task_is_finished_dropped_cancelled_or_waiting() {
	for seed in 1..=60 {
		let mut sim = Sim::new(seed);
		let mut e: Inline<Job, 4> = Inline::new();
		let (mut rev, mut spawned, mut finished) = (0u32, 0usize, 0usize);
		for _ in 0..30 {
			let mut c = Count::to(1 + sim.below(20));
			c.survives = sim.chance(1, 2);
			if e.spawn(0, rev, Job::Count(c)).is_ok() {
				spawned += 1;
			}
			finished += sim.drive(&mut e, &0, &mut rev, 2, 8, |t, v| {
				assert!(v > 0 && t.slot < 4);
			});
		}
		// What is neither finished nor waiting was dropped by an edit or cancelled; nothing is lost or counted twice.
		let waiting = e.len();
		assert!(finished + waiting <= spawned, "seed {seed}");
	}
}

#[test]
fn the_ring_keeps_the_latest_events_oldest_first() {
	let mut s = Sim::new(1);
	for i in 0..(TRACE_LEN as u32 + 10) {
		s.record(Event::Edit(i));
	}
	let ring: Vec<Event> = s.recent().collect();
	assert_eq!(ring.len(), TRACE_LEN);
	assert_eq!(ring[0], Event::Edit(10));
	assert_eq!(ring[TRACE_LEN - 1], Event::Edit(TRACE_LEN as u32 + 9));
	assert_eq!(s.count(), TRACE_LEN as u32 + 10);
}

#[test]
fn the_core_never_names_alloc() {
	let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/src");
	for f in std::fs::read_dir(dir).unwrap() {
		let p = f.unwrap().path();
		let text = std::fs::read_to_string(&p).unwrap();
		assert!(
			!text.contains("extern crate alloc") && !text.contains("alloc::"),
			"{}",
			p.display()
		);
	}
	let toml = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml")).unwrap();
	assert!(!toml.contains("default ="), "no feature is on by default");
}
