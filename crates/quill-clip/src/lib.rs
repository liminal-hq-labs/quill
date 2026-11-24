// Quill's clipboard: a selection to and from the formats a clipboard holds.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The conversions live here and the transport lives in each shell. [`copy`] turns a range of a document into the flavours a
//! [`Clipboard`] is asked for; [`paste_fragment`] reads the best flavour on offer into a fragment document. Neither touches the
//! editor: `App` keeps selection replacement and undo grouping, and asks for a copy or a paste through a request the shell serves.
//!
//! [`StoreClipboard`] holds the flavours in `Store` chunks. DOS, the terminal build and the tests use it; a Windows or GTK shell
//! implements [`Clipboard`] over the system clipboard instead.

#![no_std]
#![deny(
	clippy::unwrap_used,
	clippy::expect_used,
	clippy::panic,
	clippy::indexing_slicing
)]

use quill_formats::io::{Sink, Source};
use quill_formats::FormatError;

mod convert;
#[cfg(feature = "dib")]
mod dib;
mod store;

pub use convert::{copy, paste_fragment};
pub use store::{StoreClipboard, StoreReader, StoreWriter, MAX_DIB, MAX_RTF, MAX_TEXT};

/// One representation of the clipboard's contents.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Flavour {
	/// `Rich Text Format`: characters, paragraphs, tabs and pictures.
	Rtf,
	/// `CF_TEXT`: Windows-1252 text, a paragraph ending in CR LF.
	Text1252,
	/// `CF_DIB`: a lone bitmap picture, without its file header. Needs the `dib` feature.
	Dib,
}

impl Flavour {
	/// Every flavour, richest first.
	pub const ALL: [Flavour; 3] = [Flavour::Rtf, Flavour::Dib, Flavour::Text1252];

	pub(crate) const fn bit(self) -> u8 {
		match self {
			Flavour::Rtf => 1,
			Flavour::Text1252 => 2,
			Flavour::Dib => 4,
		}
	}
}

/// A set of flavours.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Flavours(pub(crate) u8);

impl Flavours {
	/// No flavour.
	pub const NONE: Flavours = Flavours(0);
	/// Every flavour.
	pub const ALL: Flavours = Flavours(7);
	/// What any word processor reads: RTF and text.
	pub const TEXT_AND_RTF: Flavours = Flavours(3);

	/// This set with `f` added.
	pub const fn with(self, f: Flavour) -> Flavours {
		Flavours(self.0 | f.bit())
	}

	/// True when `f` is in the set.
	pub const fn has(self, f: Flavour) -> bool {
		self.0 & f.bit() != 0
	}

	/// True when the set is empty.
	pub const fn is_empty(self) -> bool {
		self.0 == 0
	}
}

/// Why a copy or a paste did not happen.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ClipError {
	/// The range is empty or past the end of the document, or the clipboard holds nothing Quill can read.
	Empty,
	/// Out of memory, in the store or on the clipboard.
	NoMemory,
	/// The selection passes what the clipboard flavour can hold.
	TooLarge,
	/// The clipboard refused the data or could not be opened.
	Refused,
	/// What the clipboard held was damaged or not of the kind it claimed.
	BadData,
}

impl From<FormatError> for ClipError {
	fn from(e: FormatError) -> Self {
		match e {
			FormatError::BadFile | FormatError::Newer => ClipError::BadData,
			FormatError::TooLarge => ClipError::TooLarge,
			FormatError::NoMemory => ClipError::NoMemory,
			FormatError::Write => ClipError::Refused,
		}
	}
}

impl From<quill_core::Error> for ClipError {
	fn from(e: quill_core::Error) -> Self {
		match e {
			quill_core::Error::NoMemory => ClipError::NoMemory,
			quill_core::Error::TooLarge => ClipError::TooLarge,
			quill_core::Error::Range => ClipError::Empty,
		}
	}
}

/// The bytes of one flavour being put on the clipboard. Dropping a writer without calling [`ClipWrite::commit`] abandons the data.
pub trait ClipWrite: Sink {
	/// Makes the flavour available to readers. False when the system refused it.
	fn commit(self) -> bool;
}

/// A transport: where [`copy`] puts flavours and [`paste_fragment`] finds them.
///
/// A Windows shell opens the clipboard in `clear` and closes it when the writers and readers are gone; GTK's clipboard is
/// asynchronous, so its shell prefetches into a [`quill_formats::io::SliceSource`] before the call.
pub trait Clipboard {
	/// Where one flavour's bytes go.
	type W<'a>: ClipWrite
	where
		Self: 'a;
	/// Where one flavour's bytes come from.
	type R<'a>: Source
	where
		Self: 'a;

	/// Empties the clipboard and takes ownership of it (`EmptyClipboard`). False when it could not be opened.
	fn clear(&mut self) -> bool;
	/// The flavours on the clipboard now.
	fn offers(&mut self) -> Flavours;
	/// Starts writing `f`. `None` when the clipboard cannot take it.
	fn put(&mut self, f: Flavour) -> Option<Self::W<'_>>;
	/// Starts reading `f`. `None` when it is not on the clipboard.
	fn get(&mut self, f: Flavour) -> Option<Self::R<'_>>;
}
