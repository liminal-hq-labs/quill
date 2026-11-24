// quill-tty: Quill in a terminal.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The DOS text screen drawn on a terminal with crossterm: the same `App` and `Grid`, so a screen here equals the DOS and host
//! screens cell for cell. The modules are the terminal-free parts (`keymap`, `files`, `session`) and the terminal layer (`term`).

pub mod cp1252;
pub mod files;
pub mod keymap;
pub mod session;
pub mod term;

pub use files::{Dirs, LinuxFiles};
pub use keymap::map_key;
pub use session::{Options, Screen, Session};
