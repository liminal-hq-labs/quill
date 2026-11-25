// quill-gtk: the parts of the GTK4 client that need no display.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The GTK4 client is thin: this library holds everything that is not a widget, so it is tested without a display.
//!
//! - [`doc`] opens a file through the shared `App` and draws the sheets that are on screen into an RGBA buffer with the pixel
//!   surface of `quill-paint`, at the exact device size.
//! - [`view`] is the view model: zoom, scroll, the page geometry and the pixel [`view::Mapping`] from the document to the device.
//! - [`files`] and [`names`] map between Linux paths and the editor's Windows-1252 names, over the terminal client's `LinuxFiles`.
//! - [`settings`] keeps the window's size and state in `$XDG_CONFIG_HOME/quill`.
//!
//! The widgets are in the `quill` binary (`window`, `page`).

pub mod doc;
pub mod files;
pub mod names;
pub mod settings;
pub mod view;
