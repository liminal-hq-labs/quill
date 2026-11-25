// The window's chrome colours: the system colour scheme, read at start and again when the user changes it.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The toolbar, the format bar, the ruler, the desk around the page and the selection take their colours from `GetSysColor`, so the
//! chrome matches the menu bar and the status bar that the system draws itself, whatever theme is active. Until the first
//! [`refresh`] the values are those of the Windows 95 default scheme (silver face, grey shadow, navy selection). The paper and its
//! text stay white and black on every scheme: they are the document, not the chrome.

use core::sync::atomic::{AtomicU32, Ordering};

use crate::sys::GetSysColor;

const COLOR_WINDOW: i32 = 5;
const COLOR_WINDOWTEXT: i32 = 8;
const COLOR_HIGHLIGHT: i32 = 13;
const COLOR_HIGHLIGHTTEXT: i32 = 14;
const COLOR_BTNFACE: i32 = 15;
const COLOR_BTNSHADOW: i32 = 16;
const COLOR_BTNTEXT: i32 = 18;
const COLOR_BTNHIGHLIGHT: i32 = 20;
const COLOR_APPWORKSPACE: i32 = 12;

static FACE: AtomicU32 = AtomicU32::new(0x00C0_C0C0);
static SHADOW: AtomicU32 = AtomicU32::new(0x0080_8080);
static HILIGHT: AtomicU32 = AtomicU32::new(0x00FF_FFFF);
static TEXT: AtomicU32 = AtomicU32::new(0);
static WINDOW: AtomicU32 = AtomicU32::new(0x00FF_FFFF);
static WINDOW_TEXT: AtomicU32 = AtomicU32::new(0);
static DESK: AtomicU32 = AtomicU32::new(0x0080_8080);
static SELECT: AtomicU32 = AtomicU32::new(0x0080_0000);
static SELECT_TEXT: AtomicU32 = AtomicU32::new(0x00FF_FFFF);

/// Reads the system colours again.
///
/// # Safety
/// Calls into USER32.
pub unsafe fn refresh() {
	let get = |i: i32, slot: &AtomicU32| slot.store(GetSysColor(i) & 0x00FF_FFFF, Ordering::Relaxed);
	get(COLOR_BTNFACE, &FACE);
	get(COLOR_BTNSHADOW, &SHADOW);
	get(COLOR_BTNHIGHLIGHT, &HILIGHT);
	get(COLOR_BTNTEXT, &TEXT);
	get(COLOR_WINDOW, &WINDOW);
	get(COLOR_WINDOWTEXT, &WINDOW_TEXT);
	get(COLOR_APPWORKSPACE, &DESK);
	get(COLOR_HIGHLIGHT, &SELECT);
	get(COLOR_HIGHLIGHTTEXT, &SELECT_TEXT);
}

/// Button face: the toolbar, format bar and ruler band.
pub fn face() -> u32 {
	FACE.load(Ordering::Relaxed)
}

/// The dark edge of a bevel and an etched line.
pub fn shadow() -> u32 {
	SHADOW.load(Ordering::Relaxed)
}

/// The bright edge of a bevel and an etched line.
pub fn hilight() -> u32 {
	HILIGHT.load(Ordering::Relaxed)
}

/// Text and marks on the face.
pub fn text() -> u32 {
	TEXT.load(Ordering::Relaxed)
}

/// A field: the ruler's scale.
pub fn window() -> u32 {
	WINDOW.load(Ordering::Relaxed)
}

/// Text and ticks on a field.
pub fn window_text() -> u32 {
	WINDOW_TEXT.load(Ordering::Relaxed)
}

/// The desk the page lies on.
pub fn desk() -> u32 {
	DESK.load(Ordering::Relaxed)
}

/// The selection's background.
pub fn select() -> u32 {
	SELECT.load(Ordering::Relaxed)
}

/// The selection's text.
pub fn select_text() -> u32 {
	SELECT_TEXT.load(Ordering::Relaxed)
}

/// The halfway colour between two: the inner ring of a bevel, between the face and the bright or the dark edge.
pub fn mix(a: u32, b: u32) -> u32 {
	let ch = |s: u32| (((a >> s) & 0xFF) + ((b >> s) & 0xFF)) / 2;
	ch(0) | (ch(8) << 8) | (ch(16) << 16)
}
