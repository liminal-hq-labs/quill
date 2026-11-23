// Quill's file formats over byte-stream traits.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Only this crate reads or writes documents, and every build saves the same bytes for the same document.

#![no_std]
#![deny(
	clippy::unwrap_used,
	clippy::expect_used,
	clippy::panic,
	clippy::indexing_slicing,
	clippy::unreachable,
	clippy::todo
)]

pub mod io;
pub mod qld;

/// Why a file could not be read or written.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FormatError {
	/// Not a document of this format, or damaged: `... is not a document Quill can read.`
	BadFile,
	/// Written by a newer version: `... was written by a newer version of Quill.`
	Newer,
	/// Passes a document limit: `... is too large for Quill.`
	TooLarge,
	/// Out of memory.
	NoMemory,
	/// The sink refused bytes (disk full).
	Write,
}

impl From<quill_core::Error> for FormatError {
	fn from(e: quill_core::Error) -> Self {
		match e {
			quill_core::Error::NoMemory => FormatError::NoMemory,
			quill_core::Error::TooLarge => FormatError::TooLarge,
			quill_core::Error::Range => FormatError::BadFile,
		}
	}
}

/// The result type of the readers and writers.
pub type Result<T> = core::result::Result<T, FormatError>;
