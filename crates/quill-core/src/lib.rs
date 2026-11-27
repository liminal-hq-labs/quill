// Quill's portable core: storage, the piece table, undo, formats and layout.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The core never names a platform. Document-sized data lives in a [`Store`] of chunks of at most
//! 32 KB, and nothing here allocates, uses floating point or panics.

#![no_std]
#![deny(
	clippy::unwrap_used,
	clippy::expect_used,
	clippy::panic,
	clippy::indexing_slicing,
	clippy::unreachable,
	clippy::todo
)]

pub mod bytes;
pub mod doc;
pub mod farvec;
pub mod fit;
pub mod layout;
pub mod store;
pub mod text;

/// Every failure the core can report. None of them leaves a document half changed.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Error {
	/// The store could not give a chunk, or a fixed table is full.
	NoMemory,
	/// A position or index was outside what exists.
	Range,
	/// The edit would pass a document limit.
	TooLarge,
}

/// The result type used across the core.
pub type Result<T> = core::result::Result<T, Error>;
