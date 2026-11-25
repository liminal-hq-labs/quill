// The name of a job and the document revision it was made for.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// Identifies one job and the document revision it works on. A result carries its ticket back, and the owner applies it only when the
/// revision is still the document's (or the job survives edits).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Ticket {
	/// What the job is, as the owner numbers jobs (spelling, layout, autosave).
	pub kind: u8,
	/// Which slot of the executor holds it.
	pub slot: u8,
	/// `Doc::revision()` when the job was made.
	pub rev: u32,
}

impl Ticket {
	/// True when the document is still at the revision the job was made for.
	pub const fn is_current(&self, rev: u32) -> bool {
		self.rev == rev
	}
}
