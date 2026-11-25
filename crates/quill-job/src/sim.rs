// A seeded simulator: the same seed gives the same schedule, the same edits and cancels, and the same trace.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::budget::Budget;
use crate::inline::{Inline, Task};
use crate::ticket::Ticket;

/// How many of the latest events a `Sim` keeps.
pub const TRACE_LEN: usize = 64;

/// One thing a simulated run did.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Event {
	/// The document changed; this is its new revision.
	Edit(u32),
	/// A task was cancelled.
	Cancel(Ticket),
	/// Tasks the new revision outdated were dropped.
	Dropped(u32),
	/// A pass of the executor with this budget spent this many units.
	Pass { budget: u32, spent: u32 },
	/// A task finished.
	Done(Ticket),
}

impl Event {
	fn words(&self) -> [u32; 4] {
		let t = |t: &Ticket| (u32::from(t.kind) << 8) | u32::from(t.slot);
		match self {
			Event::Edit(r) => [1, *r, 0, 0],
			Event::Cancel(k) => [2, t(k), k.rev, 0],
			Event::Dropped(n) => [3, *n, 0, 0],
			Event::Pass { budget, spent } => [4, *budget, *spent, 0],
			Event::Done(k) => [5, t(k), k.rev, 0],
		}
	}
}

/// A deterministic source of choices and a record of what was done with them. It uses no clock, no thread and no allocation, so a test
/// that fails under a seed fails the same way every time and on every build.
#[derive(Clone, Debug)]
pub struct Sim {
	state: u32,
	ring: [Option<Event>; TRACE_LEN],
	next: usize,
	count: u32,
	hash: u32,
}

impl Sim {
	/// Starts from `seed`.
	pub const fn new(seed: u32) -> Self {
		Sim {
			// Xorshift needs a state other than 0.
			state: if seed == 0 { 0x9E37_79B9 } else { seed },
			ring: [None; TRACE_LEN],
			next: 0,
			count: 0,
			hash: 0x811C_9DC5,
		}
	}

	/// The next number of the sequence.
	pub fn next_u32(&mut self) -> u32 {
		let mut x = self.state;
		x ^= x << 13;
		x ^= x >> 17;
		x ^= x << 5;
		self.state = x;
		x
	}

	/// A number below `n` (0 when `n` is 0).
	pub fn below(&mut self, n: u32) -> u32 {
		if n == 0 {
			0
		} else {
			self.next_u32() % n
		}
	}

	/// True with probability `num` in `den`.
	pub fn chance(&mut self, num: u32, den: u32) -> bool {
		self.below(den) < num
	}

	/// A budget of 1 to `max` units.
	pub fn budget(&mut self, max: u32) -> Budget {
		Budget::new(1 + self.below(max.max(1)))
	}

	/// Adds `ev` to the trace.
	pub fn record(&mut self, ev: Event) {
		for w in ev.words() {
			for b in w.to_le_bytes() {
				self.hash = (self.hash ^ u32::from(b)).wrapping_mul(0x0100_0193);
			}
		}
		if let Some(slot) = self.ring.get_mut(self.next) {
			*slot = Some(ev);
		}
		self.next = (self.next + 1) % TRACE_LEN;
		self.count = self.count.wrapping_add(1);
	}

	/// How many events were recorded.
	pub const fn count(&self) -> u32 {
		self.count
	}

	/// A hash of every event recorded, in order: two runs did the same things when it is equal.
	pub const fn trace_hash(&self) -> u32 {
		self.hash
	}

	/// The latest events, oldest first, at most `TRACE_LEN`.
	pub fn recent(&self) -> impl Iterator<Item = Event> + '_ {
		let (late, early) = self.ring.split_at(self.next);
		early.iter().chain(late.iter()).flatten().copied()
	}

	/// Runs `rounds` passes of `exec` under a random schedule. Each pass may edit the document (the revision goes up, which outdates
	/// the tasks that do not survive edits), may cancel a task, and then steps the executor with a budget of 1 to `max_units`.
	/// Returns how many tasks finished. A job that is correct under this is correct under any interleaving of edits and idle time.
	pub fn drive<V: ?Sized, T: Task<V>, const N: usize>(
		&mut self,
		exec: &mut Inline<T, N>,
		view: &V,
		rev: &mut u32,
		rounds: u32,
		max_units: u32,
		mut done: impl FnMut(Ticket, T::Out),
	) -> usize {
		let mut finished = 0;
		for _ in 0..rounds {
			if self.chance(1, 8) {
				*rev = rev.wrapping_add(1);
				self.record(Event::Edit(*rev));
				let n = exec.drop_stale::<V>(*rev);
				if n > 0 {
					self.record(Event::Dropped(n as u32));
				}
			}
			if self.chance(1, 8) {
				let waiting = exec.len() as u32;
				let pick = exec.tickets().nth(self.below(waiting) as usize);
				if let Some(t) = pick {
					exec.cancel(t);
					self.record(Event::Cancel(t));
				}
			}
			let mut b = self.budget(max_units);
			let start = b.left();
			let mut ready = [None; N];
			let mut n = 0;
			finished += exec.run(view, *rev, &mut b, |t, out| {
				if let Some(s) = ready.get_mut(n) {
					*s = Some(t);
					n += 1;
				}
				done(t, out);
			});
			self.record(Event::Pass {
				budget: start,
				spent: start - b.left(),
			});
			for t in ready.into_iter().flatten() {
				self.record(Event::Done(t));
			}
		}
		finished
	}
}
