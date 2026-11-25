// The inline executor: a fixed array of task slots stepped in order from idle time.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::budget::{Budget, Poll};
use crate::ticket::Ticket;

/// A job over a view `V` of the document. It owns whatever else it needs and does at most a budget of work per step.
pub trait Task<V: ?Sized> {
	/// What the job produces.
	type Out;

	/// Does up to `budget` units of work and says whether the job is finished.
	fn step(&mut self, view: &V, budget: &mut Budget) -> Poll<Self::Out>;

	/// True for a job whose work stays right when the document changes (a picture decode, which is keyed by what it decodes). The
	/// executor drops every other job when the revision has moved.
	fn survives_edits(&self) -> bool {
		false
	}
}

struct Slot<T> {
	task: T,
	ticket: Ticket,
}

/// `N` task slots. A shell uses its own enum for `T`, because there is no `Box<dyn Task>` without `alloc`. Slots are stepped in index order,
/// so a lower slot is a higher priority, and the order never depends on timing.
pub struct Inline<T, const N: usize> {
	slots: [Option<Slot<T>>; N],
}

impl<T, const N: usize> Default for Inline<T, N> {
	fn default() -> Self {
		Self::new()
	}
}

impl<T, const N: usize> Inline<T, N> {
	/// No tasks.
	pub const fn new() -> Self {
		Inline {
			slots: [const { None }; N],
		}
	}

	/// How many tasks are waiting.
	pub fn len(&self) -> usize {
		self.slots.iter().flatten().count()
	}

	/// True when no task is waiting.
	pub fn is_empty(&self) -> bool {
		self.len() == 0
	}

	/// Puts `task` in the first free slot at or after `from` and returns its ticket, or gives the task back when every such slot is taken.
	/// `from` lets a shell keep slots for kinds of job in a fixed order.
	pub fn spawn_at(&mut self, from: usize, kind: u8, rev: u32, task: T) -> Result<Ticket, T> {
		let found = self
			.slots
			.iter()
			.enumerate()
			.skip(from)
			.find(|(_, s)| s.is_none())
			.map(|(i, _)| i);
		match found {
			Some(i) => {
				let ticket = Ticket {
					kind,
					slot: i as u8,
					rev,
				};
				if let Some(s) = self.slots.get_mut(i) {
					*s = Some(Slot { task, ticket });
				}
				Ok(ticket)
			}
			None => Err(task),
		}
	}

	/// `spawn_at` from the first slot.
	pub fn spawn(&mut self, kind: u8, rev: u32, task: T) -> Result<Ticket, T> {
		self.spawn_at(0, kind, rev, task)
	}

	/// The tickets of the tasks waiting, in step order.
	pub fn tickets(&self) -> impl Iterator<Item = Ticket> + '_ {
		self.slots.iter().flatten().map(|s| s.ticket)
	}

	/// Drops the task `ticket` names, if it is still there, and says whether it was.
	pub fn cancel(&mut self, ticket: Ticket) -> bool {
		match self.slots.get_mut(usize::from(ticket.slot)) {
			Some(s) if s.as_ref().is_some_and(|x| x.ticket == ticket) => {
				*s = None;
				true
			}
			_ => false,
		}
	}

	/// Drops every task of `kind` and returns how many there were.
	pub fn cancel_kind(&mut self, kind: u8) -> usize {
		let mut n = 0;
		for s in &mut self.slots {
			if s.as_ref().is_some_and(|x| x.ticket.kind == kind) {
				*s = None;
				n += 1;
			}
		}
		n
	}

	/// Drops every task made for a revision other than `rev` unless it survives edits, and returns how many were dropped. `run` does
	/// this first, so a shell calls it only to clear tasks early.
	pub fn drop_stale<V: ?Sized>(&mut self, rev: u32) -> usize
	where
		T: Task<V>,
	{
		let mut n = 0;
		for s in &mut self.slots {
			if s.as_ref()
				.is_some_and(|x| !x.ticket.is_current(rev) && !x.task.survives_edits())
			{
				*s = None;
				n += 1;
			}
		}
		n
	}

	/// Steps the waiting tasks in order until `budget` is spent: first the tasks the revision has outdated are dropped, then the first
	/// task is stepped until it finishes, spends the budget or returns `Pending` without spending any (it is waiting, so the next task
	/// gets the turn). `done` is called with each finished task's ticket and result. Returns how many tasks finished.
	pub fn run<V: ?Sized>(
		&mut self,
		view: &V,
		rev: u32,
		budget: &mut Budget,
		mut done: impl FnMut(Ticket, T::Out),
	) -> usize
	where
		T: Task<V>,
	{
		self.drop_stale::<V>(rev);
		let mut finished = 0;
		for s in &mut self.slots {
			while !budget.spent() {
				let Some(slot) = s.as_mut() else {
					break;
				};
				let before = budget.left();
				match slot.task.step(view, budget) {
					Poll::Ready(out) => {
						done(slot.ticket, out);
						*s = None;
						finished += 1;
					}
					Poll::Pending if budget.left() == before => break,
					Poll::Pending => {}
				}
			}
			if budget.spent() {
				break;
			}
		}
		finished
	}
}
