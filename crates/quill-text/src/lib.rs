// quill-text: the one home of Quill's character tables: Windows-1252, CP437 and the UTF-8 reading rule.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Documents hold Windows-1252 bytes (D5), the DOS screen shows CP437, and a text file may be either that
//! ANSI or UTF-8. This crate holds every table those conversions need, in one place, with no allocation,
//! no floating point and no panics.
//!
//! - [`codepage`]: the Windows code pages 1250, 1251, 1253, 1254 and 1257 (and 1252) to Unicode, for RTF's `\ansicpg` and `\fcharset`.
//! - [`cp1252`]: a Windows-1252 byte to a Unicode character and back.
//! - [`cp437`]: a CP437 byte to a Unicode character, and between CP437 and Windows-1252 bytes.
//! - [`utf8`]: reading text as UTF-8 when it is valid and as Windows-1252 otherwise, and writing UTF-8.
//!
//! The Unicode tables (WP-UNI-1), generated from the Unicode Character Database 16.0.0 by `scripts/gen-unicode.py`.
//! Nothing links them yet, so no build pays for them; each is a sorted range table with a binary search, and
//! [`unitab`] reads them all from one blob (`assets/unicode/UNITAB.BIN`) for a build that serves them from a resource.
//!
//! - [`cluster`]: which characters join the one before in a grapheme cluster.
//! - [`breaks`]: where a line may break, a UAX #14 subset.
//! - [`word`]: letter, punctuation, space or ideograph, for words beyond ASCII.
//! - [`fold`]: simple case folding for the alphabetic scripts.
//! - [`compose`]: the Windows-1252 letters as a base and a combining mark.
//! - [`width`]: East Asian Wide and Fullwidth.
//! - [`script`]: the scripts drawn as placeholders in v1, and the invisible format characters.
//! - [`blocks`]: the Unicode block of a character and the short names of invisible ones.

#![no_std]
#![forbid(unsafe_code)]
#![deny(
	clippy::unwrap_used,
	clippy::expect_used,
	clippy::panic,
	clippy::indexing_slicing,
	clippy::unreachable,
	clippy::todo
)]

#[cfg(test)]
extern crate std;

pub mod blocks;
pub mod breaks;
pub mod cluster;
pub mod codepage;
pub mod compose;
pub mod cp1252;
pub mod cp437;
pub mod fold;
pub mod script;
pub mod unitab;
pub mod utf8;
pub mod width;
pub mod word;

mod cpdata;
