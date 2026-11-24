// quill-text: the one home of Quill's character tables: Windows-1252, CP437 and the UTF-8 reading rule.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Documents hold Windows-1252 bytes (D5), the DOS screen shows CP437, and a text file may be either that
//! ANSI or UTF-8. This crate holds every table those conversions need, in one place, with no allocation,
//! no floating point and no panics.
//!
//! - [`cp1252`]: a Windows-1252 byte to a Unicode character and back.
//! - [`cp437`]: a CP437 byte to a Unicode character, and between CP437 and Windows-1252 bytes.
//! - [`utf8`]: reading text as UTF-8 when it is valid and as Windows-1252 otherwise, and writing UTF-8.

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

pub mod cp1252;
pub mod cp437;
pub mod utf8;
