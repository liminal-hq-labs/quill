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
//! - [`actions`], [`menu`], [`toolbar`], [`format_bar`], [`ruler`], [`status`] and [`dialogs`] are the chrome: the Windows build's
//!   menus, toolbar, formatting bar, ruler, status bar and dialogs as GTK4 and libadwaita widgets over Gio actions that run the
//!   shared `Command`s. Each has a plain-data part (the tree, the specs, the layout) that is tested without a display.
//!
//! The window and the page widget are in the `quill` binary (`window`, `page`).

pub mod actions;
pub mod dialogs;
pub mod doc;
pub mod files;
pub mod format_bar;
pub mod menu;
pub mod names;
pub mod ruler;
pub mod settings;
pub mod status;
pub mod toolbar;
pub mod view;
