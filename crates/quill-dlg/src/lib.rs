// quill-dlg: the logic of Quill's file dialogs, with no drawing and no platform.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Everything a file dialog decides, kept apart from how it is drawn so the text screens, the host's goldens and the Windows
//! builds share it:
//!
//! - [`name83`]: DOS 8.3 names, paths, wildcards and the reserved device names. Nothing is ever truncated: a name that is not
//!   valid is refused.
//! - [`types`]: the one table of document and picture types, and the `\0`-separated filter strings the Windows common dialogs take.
//! - [`savename`]: the name and type Save As starts with, the extension swap when the type changes, and the default extension.
//! - [`key`], [`widgets`] and [`focus`]: the keys a dialog understands, the state of its widgets (text field, list, drop-down, check
//!   box, radio group) and the focus ring with mnemonics and the default button. None of them draws.
//!
//! The crate is `no_std`, allocates nothing, uses no floating point and cannot panic on any input.

#![no_std]
#![deny(
	clippy::unwrap_used,
	clippy::expect_used,
	clippy::panic,
	clippy::indexing_slicing
)]

pub mod focus;
pub mod key;
pub mod name83;
pub mod savename;
pub mod types;
pub mod widgets;

#[cfg(test)]
#[macro_use]
extern crate std;
