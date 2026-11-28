// HLCLIP.EXE: a test-only program that puts a rich-text copy on the guest's clipboard, the way an editor would (`Rich Text Format` with `CF_TEXT`
// beside it), so the end-to-end check can see the helper offer RTF. Not on the product disk.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `HLCLIP.EXE rtf` sets the clipboard and ends. The copy is `plain bold italic` in a small RTF document.
#![no_std]
#![no_main]
#![allow(dead_code)]

mod rt;
mod sys;

use core::ptr::null_mut;

use sys::*;

const TEXT: &[u8] = b"plain bold italic\0";
const RTF: &[u8] = b"{\\rtf1\\ansi{\\fonttbl{\\f0 Arial;}}\\f0\\fs28 plain {\\b bold} {\\i italic}\\par}\0";

/// Puts one block on the open clipboard.
fn put(format: u32, bytes: &[u8]) -> bool {
	// SAFETY: allocates a movable block, fills it and hands it to the clipboard, which then owns it.
	unsafe {
		let handle = GlobalAlloc(GMEM_MOVEABLE | GMEM_DDESHARE, bytes.len());
		if handle.is_null() {
			return false;
		}
		let pointer = GlobalLock(handle);
		if pointer.is_null() {
			return false;
		}
		core::ptr::copy_nonoverlapping(bytes.as_ptr(), pointer, bytes.len());
		GlobalUnlock(handle);
		!SetClipboardData(format, handle).is_null()
	}
}

#[no_mangle]
pub extern "C" fn start() -> ! {
	// SAFETY: plain calls on this thread; the clipboard is opened with no owner window, as a command-line tool would.
	let code = unsafe {
		let rtf = RegisterClipboardFormatA(b"Rich Text Format\0".as_ptr());
		let mut opened = false;
		for _ in 0..20 {
			if OpenClipboard(null_mut()) != 0 {
				opened = true;
				break;
			}
			Sleep(5);
		}
		if opened {
			EmptyClipboard();
			let both = put(CF_TEXT, TEXT) && put(rtf, RTF);
			CloseClipboard();
			if both {
				0
			} else {
				2
			}
		} else {
			1
		}
	};
	// SAFETY: ends the process.
	unsafe { ExitProcess(code) }
}
