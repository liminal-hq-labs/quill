// quill-picture: a picture as it is drawn, and the cache that keeps the pictures drawn lately.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A document's picture drawn at the size it is shown: [`render`] decodes it by rows from the picture pool and scales it, [`fingerprint`]
//! tells one picture from another that took its index, and [`Cache`] keeps the pictures drawn lately so scrolling does not decode them
//! again. The cache holds only the keys and the replacement rule: the caller lends the slots (a [`Slots`] implementation), so Windows 95
//! keeps its bitmaps' bits and a later GTK or WinUI cache can keep its textures.

#![no_std]
#![deny(
	clippy::unwrap_used,
	clippy::expect_used,
	clippy::panic,
	clippy::indexing_slicing
)]

mod cache;
mod render;

pub use cache::{Cache, Key, Slots};
pub use render::{fingerprint, render, PictureSource, WINDOW};
