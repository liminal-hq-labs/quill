// Idle time: the one place the editor's background work runs, in a fixed order, a budget at a time.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// A shell calls `App::idle` when its input has been quiet: from the DOS wait loop, a Windows timer or a terminal's poll deadline. The
// call does at most `Idle::units` units of work and says whether more is waiting, so a driver is a few lines and never knows what the
// jobs are (CORE-17). The order is fixed and never depends on timing, so a run is the same on every machine and under every schedule
// (CORE-20): reflow tail, spelling, visible layout, background layout, word count, dictionary, pictures, autosave. Autosave is last so
// it never delays a repaint. Only spelling and autosave exist so far; the rest keep their numbers in `job`.
//
// The rules every job keeps (CORE-16): a job never edits the document, so a keystroke cannot be undone by idle time; its budget is in
// units it defines (a word checked, a block written); and a result is applied only for the revision it was made for.
//
// Autosave is a `quill_job::Task` held by the inline executor. Its `step` reads a `Probe` (the facts the due check needs) and finishes
// when an autosave is due and one unit is left; the owner then writes the recovery copy, because writing changes the app's own state
// and a task only reads. The write is the one piece of work that cannot be cut into units, so it polls for input every
// `qld::PROGRESS_STEP` bytes and gives up the moment a key waits: a key is held up by at most one poll, and what is on the disk is
// untouched. After `ABANDONS` abandoned writes the next one is announced on the status line and runs to the end.
use quill_core::layout::Measure;
use quill_core::store::Store;
use quill_job::{Budget, Inline, Poll, Task};

use crate::recovery::{base_name, Why, Wrote, STR_AUTOSAVING};
use crate::{App, Files, Mode};

/// The kinds of job, in the order `App::idle` runs them. A `quill_job::Ticket` carries one.
pub mod job {
	/// The tail of the checkpoint list after an edit.
	pub const REFLOW_TAIL: u8 = 0;
	/// Check Spelling as You Type.
	pub const SPELL: u8 = 1;
	/// Layout of the lines on the screen.
	pub const LAYOUT: u8 = 2;
	/// Layout of the rest of the document.
	pub const BG_LAYOUT: u8 = 3;
	/// The word count.
	pub const WORD_COUNT: u8 = 4;
	/// Loading a dictionary.
	pub const DICTIONARY: u8 = 5;
	/// Decoding pictures.
	pub const PICTURES: u8 = 6;
	/// The recovery copy.
	pub const AUTOSAVE: u8 = 7;
}

/// Units an autosave costs to start: it needs the budget of one pause. The write itself is bounded by the poll for input, not by units.
pub const AUTOSAVE_START: u32 = 1;
/// Writes that input may abandon before the next one runs to the end.
pub const ABANDONS: u8 = 2;
/// The most idle passes `drain` makes.
const DRAIN_PASSES: u32 = 256;
/// No revision: what a clean document holds as the last autosaved one.
const NO_REV: u32 = u32::MAX;

/// The announcement of a forced autosave has been made and the next pass writes.
const ANNOUNCED: u8 = 1;

/// What a driver tells `App::idle`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Idle {
	/// Whole seconds since the last key, click or command.
	pub quiet: u32,
	/// How many units of work the pass may do: a word checked is one.
	pub units: u32,
	/// A native dialog, a print or preview, a drag or an IME composition is under way: nothing is written.
	pub busy: bool,
}

/// What a pass did.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Idled {
	/// Units spent.
	pub spent: u32,
	/// Work is ready now: call again soon, with a repaint first if the damage is not empty.
	pub more: bool,
	/// An autosave is waiting for its time: call again at the shell's slow tick.
	pub waiting: bool,
	/// A recovery copy was written.
	pub wrote: bool,
}

/// The facts the autosave due check reads.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Probe {
	/// Autosave is on and a place to write is usable.
	pub on: bool,
	/// The document has changes the last autosave does not have.
	pub dirty: bool,
	/// Seconds since the document first became dirty.
	pub waited: u32,
	/// Seconds between autosaves, 0 for never.
	pub interval: u32,
	/// Seconds since the last input.
	pub quiet: u32,
	/// Seconds of quiet an autosave needs.
	pub wait: u32,
	/// The editor is in plain editing: no menu, dialog, prompt, print, drag or composition.
	pub plain: bool,
	/// This window does not defer to another one that has the document.
	pub free: bool,
}

impl Probe {
	/// True when every condition of section 4 holds.
	pub const fn due(&self) -> bool {
		self.on
			&& self.interval > 0
			&& self.dirty
			&& self.plain
			&& self.free
			&& self.waited >= self.interval
			&& self.quiet >= self.wait
	}
}

/// The autosave job: it waits, costing nothing, until an autosave is due.
#[derive(Copy, Clone, Debug, Default)]
pub struct Autosave;

impl Task<Probe> for Autosave {
	type Out = ();

	fn step(&mut self, view: &Probe, budget: &mut Budget) -> Poll<()> {
		if view.due() && budget.take(AUTOSAVE_START) {
			Poll::Ready(())
		} else {
			Poll::Pending
		}
	}

	// It reads the document as it is when it runs, so an edit does not outdate it.
	fn survives_edits(&self) -> bool {
		true
	}
}

/// What the app keeps between passes: a few integers and the executor's one slot.
pub(crate) struct State {
	jobs: Inline<Autosave, 1>,
	/// When the document first became dirty after its last save or autosave, as a DOS packed time; 0 for not dirty.
	since: u32,
	/// The revision the last autosave wrote.
	saved: u32,
	/// Writes input has cut short since the last one that finished.
	cut: u8,
	flags: u8,
}

impl State {
	pub(crate) const fn new() -> State {
		State {
			jobs: Inline::new(),
			since: 0,
			saved: NO_REV,
			cut: 0,
			flags: 0,
		}
	}

	/// The document went away or was saved: what was true of it is not true of the next.
	pub(crate) fn reset(&mut self) {
		self.jobs.cancel_kind(job::AUTOSAVE);
		self.since = 0;
		self.saved = NO_REV;
		self.cut = 0;
		self.flags = 0;
	}
}

/// Seconds since 1 January 1980 for a DOS packed date and time. A clock that goes back gives smaller numbers, which callers treat as no
/// time having passed.
pub fn dos_seconds(t: u32) -> u32 {
	const BEFORE: [u16; 12] = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
	let (date, time) = (t >> 16, t & 0xFFFF);
	let year = (date >> 9) & 127;
	let month = ((date >> 5) & 15).clamp(1, 12);
	let day = date & 31;
	let leap = year % 4 == 0;
	let mut days = year * 365 + year.div_ceil(4);
	days += u32::from(BEFORE.get(month as usize - 1).copied().unwrap_or(0));
	days += u32::from(leap && month > 2);
	days += day.saturating_sub(1);
	days * 86_400 + (time >> 11) * 3600 + ((time >> 5) & 63) * 60 + (time & 31) * 2
}

impl<M: Measure + Clone> App<M> {
	/// Does up to `ctx.units` units of background work, in the fixed order, and says what is left. `input` is true when a key or
	/// click waits: a recovery write polls it and gives up. Idle work never changes the document and never reads the keyboard, so a
	/// typed key is never lost and never waits for more than one poll.
	pub fn idle<S: Store + ?Sized, F: Files>(
		&mut self,
		s: &mut S,
		f: &mut F,
		ctx: Idle,
		input: &mut dyn FnMut() -> bool,
	) -> Idled {
		let mut budget = Budget::new(ctx.units);
		let mut out = Idled::default();
		// Spelling. The shell says which part of the body is on the screen (`spell_set_view`).
		if self.spell_live_ready(s, f) {
			let (more, spent) = self.spell_step(s, budget.left());
			budget.take(spent);
			out.more = more;
		}
		self.autosave_pass(s, f, ctx, input, &mut budget, &mut out);
		out.spent = ctx.units - budget.left();
		out
	}

	/// Runs `idle` with all the budget it can use until nothing is ready, as a test does before it looks at the screen or the files.
	/// Autosave runs if it is due, at `quiet` seconds of quiet. The result adds the passes up.
	pub fn drain<S: Store + ?Sized, F: Files>(
		&mut self,
		s: &mut S,
		f: &mut F,
		quiet: u32,
	) -> Idled {
		let mut total = Idled::default();
		for _ in 0..DRAIN_PASSES {
			let ctx = Idle {
				quiet,
				units: u32::MAX / 2,
				busy: false,
			};
			let r = self.idle(s, f, ctx, &mut || false);
			total.spent = total.spent.saturating_add(r.spent);
			total.wrote |= r.wrote;
			total.waiting = r.waiting;
			total.more = r.more;
			if !r.more {
				break;
			}
		}
		total
	}

	/// The range of the body that idle work changed since the last call, for the shell to paint again; none when nothing changed.
	pub fn take_damage(&mut self) -> Option<(u32, u32)> {
		self.spell_take_changed()
	}

	/// A key or command was handled: a document that has just become dirty starts its interval now.
	pub(crate) fn idle_dirty<F: Files>(&mut self, f: &mut F) {
		if self.modified && self.idle.since == 0 && self.doc.revision() != self.idle.saved {
			self.idle.since = f.now();
		}
	}

	/// The document went away or was saved.
	pub(crate) fn idle_reset(&mut self) {
		self.idle.reset();
	}

	fn autosave_pass<S: Store + ?Sized, F: Files>(
		&mut self,
		s: &mut S,
		f: &mut F,
		ctx: Idle,
		input: &mut dyn FnMut() -> bool,
		budget: &mut Budget,
		out: &mut Idled,
	) {
		let interval = self.autosave_interval();
		let rev = self.doc.revision();
		if !self.modified || !self.recovery_on() || interval == 0 {
			if !self.modified {
				self.idle.reset();
			} else {
				self.idle.jobs.cancel_kind(job::AUTOSAVE);
			}
			return;
		}
		let now = f.now();
		if now == 0 || rev == self.idle.saved {
			// No clock to measure by, or nothing the last autosave does not have.
			self.idle.jobs.cancel_kind(job::AUTOSAVE);
			self.idle.since = 0;
			return;
		}
		if self.idle.since == 0 {
			self.idle.since = now;
		}
		if self.idle.jobs.is_empty() {
			let _ = self.idle.jobs.spawn(job::AUTOSAVE, rev, Autosave);
		}
		out.waiting = true;
		let probe = Probe {
			on: true,
			dirty: true,
			waited: dos_seconds(now).saturating_sub(dos_seconds(self.idle.since)),
			interval,
			quiet: ctx.quiet,
			wait: self.autosave_wait(),
			plain: self.mode == Mode::Edit && self.pending.is_none() && !ctx.busy,
			free: !self.second_window(),
		};
		let mut go = false;
		self.idle.jobs.run(&probe, rev, budget, |_, ()| go = true);
		if go {
			self.autosave_go(s, f, input, rev, out);
		}
	}

	/// An autosave is due and the pause has the budget for it: write the copy, or announce a forced write and write it on the next pass.
	fn autosave_go<S: Store + ?Sized, F: Files>(
		&mut self,
		s: &mut S,
		f: &mut F,
		input: &mut dyn FnMut() -> bool,
		rev: u32,
		out: &mut Idled,
	) {
		let forced = self.idle.cut >= ABANDONS;
		if forced && self.idle.flags & ANNOUNCED == 0 {
			// The person sees what the pause is for before it starts; the next pass writes.
			self.idle.flags |= ANNOUNCED;
			self.say_autosaving(f);
			out.more = true;
			return;
		}
		if !forced && input() {
			return;
		}
		if forced {
			self.msg.clear();
		}
		// One writer serves both, so the program holds one copy of it: a forced write polls a check that never says stop.
		let mut never = || false;
		let poll: &mut dyn FnMut() -> bool = if forced { &mut never } else { input };
		let how = self.save_recovery_with(s, f, Why::Autosave, poll);
		match how {
			Wrote::Done => {
				self.idle.saved = rev;
				self.idle.since = 0;
				self.idle.cut = 0;
				self.idle.flags = 0;
				out.wrote = true;
			}
			Wrote::Abandoned => self.idle.cut = self.idle.cut.saturating_add(1),
			Wrote::Failed => {
				// Back off one interval.
				self.idle.since = f.now();
				self.idle.cut = 0;
				self.idle.flags = 0;
			}
		}
	}

	fn say_autosaving<F: Files>(&mut self, f: &mut F) {
		let mut buf = [0u8; 48];
		let n = f.string(STR_AUTOSAVING, &mut buf);
		self.msg.clear();
		self.msg.push(buf.get(..n).unwrap_or(&[]));
		let name = self.name.get(..self.name_len).unwrap_or(&[]);
		if !name.is_empty() {
			self.msg.push(b" ");
			self.msg.push(base_name(name));
		}
		self.msg.push(b"...");
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn probe() -> Probe {
		Probe {
			on: true,
			dirty: true,
			waited: 600,
			interval: 600,
			quiet: 2,
			wait: 2,
			plain: true,
			free: true,
		}
	}

	#[test]
	fn an_autosave_is_due_only_when_every_condition_holds() {
		assert!(probe().due());
		assert!(!Probe {
			on: false,
			..probe()
		}
		.due());
		assert!(!Probe {
			interval: 0,
			..probe()
		}
		.due());
		assert!(!Probe {
			dirty: false,
			..probe()
		}
		.due());
		assert!(!Probe {
			waited: 599,
			..probe()
		}
		.due());
		assert!(!Probe {
			quiet: 1,
			..probe()
		}
		.due());
		assert!(!Probe {
			plain: false,
			..probe()
		}
		.due());
		assert!(!Probe {
			free: false,
			..probe()
		}
		.due());
	}

	#[test]
	fn the_job_waits_without_spending_and_finishes_when_due() {
		let mut j = Autosave;
		let mut b = Budget::new(5);
		assert_eq!(
			j.step(
				&Probe {
					quiet: 0,
					..probe()
				},
				&mut b
			),
			Poll::Pending
		);
		assert_eq!(b.left(), 5);
		assert_eq!(j.step(&probe(), &mut b), Poll::Ready(()));
		assert_eq!(b.left(), 5 - AUTOSAVE_START);
		let mut none = Budget::none();
		assert_eq!(j.step(&probe(), &mut none), Poll::Pending);
	}

	#[test]
	fn dos_seconds_counts_from_1980() {
		let t = |y: u32, mo: u32, d: u32, h: u32, mi: u32, s: u32| {
			((((y - 1980) << 9) | (mo << 5) | d) << 16) | ((h << 11) | (mi << 5) | (s / 2))
		};
		assert_eq!(dos_seconds(t(1980, 1, 1, 0, 0, 0)), 0);
		assert_eq!(dos_seconds(t(1980, 1, 1, 0, 1, 2)), 62);
		// 1980 is a leap year: 1 March is day 60.
		assert_eq!(dos_seconds(t(1980, 3, 1, 0, 0, 0)), 60 * 86_400);
		assert_eq!(dos_seconds(t(1981, 1, 1, 0, 0, 0)), 366 * 86_400);
		let a = t(2026, 11, 25, 23, 59, 58);
		let b = t(2026, 11, 26, 0, 0, 0);
		assert_eq!(dos_seconds(b) - dos_seconds(a), 2);
		let c = t(2028, 2, 28, 0, 0, 0);
		let d = t(2028, 3, 1, 0, 0, 0);
		assert_eq!(dos_seconds(d) - dos_seconds(c), 2 * 86_400);
		let e = t(2026, 12, 31, 0, 0, 0);
		let g = t(2027, 1, 1, 0, 0, 0);
		assert_eq!(dos_seconds(g) - dos_seconds(e), 86_400);
	}
}
