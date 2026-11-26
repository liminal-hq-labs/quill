// Quill's job mechanism: work in budgeted steps that can be stopped, with a seeded simulator for tests.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A job is a `Task`: it reads a view of the document and owns everything else it needs, and each `step` does at most a `Budget` of
//! work units and says whether it is done. Nothing here knows a document, a thread or a clock. The `Inline` executor steps tasks in a
//! fixed order from a shell's idle time; a thread executor, built elsewhere, steps the same tasks from owned snapshots.
//!
//! The core is `no_std` and never names `alloc`: slots are a fixed array and the shell's own enum stands in for a boxed task. The
//! `alloc` and `std` features are reserved for executors that need a heap, and add nothing yet.
//!
//! Policy (which jobs, how many units, inline or worker) lives above this crate. This is mechanism only.

#![no_std]
#![deny(
	clippy::unwrap_used,
	clippy::expect_used,
	clippy::panic,
	clippy::indexing_slicing,
	clippy::unreachable,
	clippy::todo
)]

mod budget;
mod cancel;
mod inline;
mod sim;
mod ticket;

pub use budget::{Budget, Poll};
pub use cancel::Cancel;
pub use inline::{Inline, Task};
pub use sim::{Event, Sim, TRACE_LEN};
pub use ticket::Ticket;
