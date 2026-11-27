// quill-chrome: the window geometry the graphical shells share.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Where the toolbar's buttons, the ruler, the page and the scroll bars are in a window, what a click on each hits, and the zoom box at
//! the right end of the status bar. Everything is in pixels of a 96-dot-per-inch screen with the client area's top left at (0, 0): the
//! platform draws and passes the mouse in, so every graphical shell lays the window out the same way. The editor's scroll state and the
//! methods that use it stay in `quill_app::scroll`, which [`view`] and [`zoom_field`] re-export from.

#![no_std]
#![deny(
	clippy::unwrap_used,
	clippy::expect_used,
	clippy::panic,
	clippy::indexing_slicing
)]

pub mod view;
pub mod zoom_field;
