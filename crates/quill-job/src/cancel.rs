// Cancellation shared between the owner of a job and the thread that runs it.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::sync::atomic::{AtomicU32, Ordering};

/// A generation number. A job copies the number when it starts (`token`) and checks it between steps (`is_cancelled`); the owner bumps
/// it (`cancel`) to cancel every job started before, which needs no list of them and cannot race with a job that starts afterwards.
/// It is one 32-bit atomic, which every target Quill builds for has.
#[derive(Debug, Default)]
pub struct Cancel(AtomicU32);

impl Cancel {
	/// Generation 0.
	pub const fn new() -> Self {
		Cancel(AtomicU32::new(0))
	}

	/// The generation a job started now belongs to.
	pub fn token(&self) -> u32 {
		self.0.load(Ordering::Acquire)
	}

	/// Cancels every job whose token is older than the one this returns.
	pub fn cancel(&self) -> u32 {
		self.0.fetch_add(1, Ordering::AcqRel).wrapping_add(1)
	}

	/// True when `token` has been cancelled since it was taken.
	pub fn is_cancelled(&self, token: u32) -> bool {
		self.0.load(Ordering::Acquire) != token
	}
}
