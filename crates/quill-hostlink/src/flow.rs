// Credits: how many frame bytes the page may have in flight to the guest, and how the guest reports the bytes it has taken.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// The window a `hello` names when it names none: 16 KiB, within the 64 KiB input buffer the helper asks for.
pub const DEFAULT_WINDOW: u32 = 16_384;
/// The smallest window a sender accepts: one longest packet (4,096 bytes of payload and a header line) with room to spare.
pub const MIN_WINDOW: u32 = 4_352;
/// The receiver sends a `credit` once it has taken at least this many bytes off the port.
pub const CREDIT_STEP: u32 = 4_096;

/// The sender's account. `outstanding` counts frame bytes sent and not yet credited back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Credit {
	window: u32,
	outstanding: u32,
}

impl Credit {
	/// A window below `MIN_WINDOW` is raised to it, so a sender can always send a whole packet when idle.
	pub fn new(window: u32) -> Credit {
		Credit {
			window: window.max(MIN_WINDOW),
			outstanding: 0,
		}
	}

	pub fn window(&self) -> u32 {
		self.window
	}

	pub fn outstanding(&self) -> u32 {
		self.outstanding
	}

	/// Bytes that may be sent now.
	pub fn available(&self) -> u32 {
		self.window.saturating_sub(self.outstanding)
	}

	/// Spends `bytes` if they fit; otherwise changes nothing and returns false.
	pub fn try_spend(&mut self, bytes: u32) -> bool {
		if bytes > self.available() {
			return false;
		}
		self.outstanding += bytes;
		true
	}

	/// Takes a `credit` message. More than is outstanding is clamped, so a repeated credit cannot raise the window.
	pub fn grant(&mut self, bytes: u32) {
		self.outstanding = self.outstanding.saturating_sub(bytes);
	}

	/// A new `hello`: a new window and nothing outstanding (the bytes in flight went with the old state).
	pub fn reset(&mut self, window: u32) {
		*self = Credit::new(window);
	}
}

/// The receiver's side: counts bytes taken off the port and says when a `credit` is due.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CreditReporter {
	taken: u32,
}

impl CreditReporter {
	pub const fn new() -> CreditReporter {
		CreditReporter { taken: 0 }
	}

	/// Records `bytes` taken; returns the `n` for a `credit` message once `CREDIT_STEP` or more are owed, and starts counting again.
	pub fn took(&mut self, bytes: u32) -> Option<u32> {
		self.taken = self.taken.saturating_add(bytes);
		if self.taken >= CREDIT_STEP {
			let n = self.taken;
			self.taken = 0;
			Some(n)
		} else {
			None
		}
	}

	pub fn reset(&mut self) {
		self.taken = 0;
	}
}
