// Work budgets and the result of a step.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// How much work a step may still do, in units the job defines (characters scanned, lines laid out, bytes written). A shell turns
/// milliseconds into units; the job never reads a clock, so a run is the same on every machine.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Budget {
	left: u32,
}

impl Budget {
	/// A budget of `units`.
	pub const fn new(units: u32) -> Self {
		Budget { left: units }
	}

	/// No budget at all: every `take` fails. A step given this one does nothing.
	pub const fn none() -> Self {
		Budget { left: 0 }
	}

	/// Spends `n` units and returns true, or, when fewer than `n` are left, spends what is left and returns false. A job calls it before a
	/// piece of work of `n` units and stops when it says false.
	pub fn take(&mut self, n: u32) -> bool {
		if n <= self.left {
			self.left -= n;
			true
		} else {
			self.left = 0;
			false
		}
	}

	/// True when nothing is left.
	pub const fn spent(&self) -> bool {
		self.left == 0
	}

	/// The units left.
	pub const fn left(&self) -> u32 {
		self.left
	}
}

/// What a step reports: the job is finished with a result, or it has more to do.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Poll<T> {
	/// Finished.
	Ready(T),
	/// Not finished: step again.
	Pending,
}

impl<T> Poll<T> {
	/// True for `Ready`.
	pub const fn is_ready(&self) -> bool {
		matches!(self, Poll::Ready(_))
	}

	/// Changes a result and leaves `Pending` as it is.
	pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Poll<U> {
		match self {
			Poll::Ready(v) => Poll::Ready(f(v)),
			Poll::Pending => Poll::Pending,
		}
	}
}
