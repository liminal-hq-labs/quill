// The Windows clipboard as a `quill_clip::Clipboard`: Copy and Cut put CF_TEXT, Rich Text Format and (for a lone picture) CF_DIB on it,
// Paste reads them back in that order of preference (WP-ED-6, W9X-140).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;

use quill_app::ClipRequest;
use quill_clip::{ClipWrite, Clipboard, Flavour, Flavours, MAX_DIB, MAX_RTF, MAX_TEXT};
use quill_formats::io::{Sink, Source};

use crate::sys::Handle;
use crate::Shell;

const CF_TEXT: u32 = 1;
const CF_DIB: u32 = 8;
const GMEM_MOVEABLE: u32 = 2;
const MF_GRAYED: u32 = 1;

/// How many times a busy clipboard is tried, and how long to wait between tries, in milliseconds: about a fifth of a second in all, so
/// another program holding the clipboard open delays Copy and Paste a little and never hangs them.
const TRIES: u32 = 10;
const WAIT_MS: u32 = 20;

#[link(name = "user32", kind = "raw-dylib", import_name_type = "undecorated")]
extern "system" {
	fn OpenClipboard(hwnd: Handle) -> i32;
	fn CloseClipboard() -> i32;
	fn EmptyClipboard() -> i32;
	fn SetClipboardData(format: u32, mem: Handle) -> Handle;
	fn GetClipboardData(format: u32) -> Handle;
	fn IsClipboardFormatAvailable(format: u32) -> i32;
	fn RegisterClipboardFormatA(name: *const u8) -> u32;
	fn EnableMenuItem(menu: Handle, id: u32, flags: u32) -> i32;
}

#[link(name = "kernel32", kind = "raw-dylib", import_name_type = "undecorated")]
extern "system" {
	fn GlobalAlloc(flags: u32, bytes: usize) -> Handle;
	fn GlobalFree(mem: Handle) -> Handle;
	fn GlobalLock(mem: Handle) -> *mut u8;
	fn GlobalUnlock(mem: Handle) -> i32;
	fn GlobalSize(mem: Handle) -> usize;
	fn Sleep(ms: u32);
}

/// The registered format that word processors share for formatted text.
fn rtf_format() -> u32 {
	// SAFETY: the name is NUL-terminated; registering the same name again returns the same number.
	unsafe { RegisterClipboardFormatA(b"Rich Text Format\0".as_ptr()) }
}

fn format_of(f: Flavour) -> u32 {
	match f {
		Flavour::Rtf => rtf_format(),
		Flavour::Text1252 => CF_TEXT,
		Flavour::Dib => CF_DIB,
	}
}

/// The most bytes of one flavour Quill takes or gives.
const fn limit(f: Flavour) -> usize {
	(match f {
		Flavour::Rtf => MAX_RTF,
		Flavour::Text1252 => MAX_TEXT,
		Flavour::Dib => MAX_DIB,
	}) as usize
}

/// What the clipboard offers Quill now. Needs no open clipboard, so a menu can ask. Windows makes CF_TEXT from a Unicode-only
/// clipboard (Windows NT and later), which is how text from a program that only offers Unicode text reaches Quill.
pub(crate) fn available() -> Flavours {
	let mut set = Flavours::NONE;
	for f in Flavour::ALL {
		// SAFETY: a plain query of the clipboard's format list.
		if unsafe { IsClipboardFormatAvailable(format_of(f)) } != 0 {
			set = set.with(f);
		}
	}
	set
}

/// The system clipboard, open for one Copy, Cut or Paste. Dropping it closes the clipboard.
pub(crate) struct Win32Clip {
	hwnd: Handle,
	open: bool,
	/// The RTF reached the clipboard, which text and a picture may follow: a selection whose RTF did not fit is refused whole, so a
	/// Cut never deletes formatting that the clipboard did not keep.
	rtf_ok: bool,
}

impl Win32Clip {
	pub(crate) fn new(hwnd: Handle) -> Self {
		Win32Clip {
			hwnd,
			open: false,
			rtf_ok: false,
		}
	}

	/// Opens the clipboard, trying again a few times when another program has it. False when it stays busy.
	fn ensure_open(&mut self) -> bool {
		for n in 0..TRIES {
			if self.open {
				return true;
			}
			// SAFETY: `hwnd` is the shell's window.
			if unsafe { OpenClipboard(self.hwnd) } != 0 {
				self.open = true;
			} else if n + 1 < TRIES {
				// SAFETY: a plain wait.
				unsafe { Sleep(WAIT_MS) };
			}
		}
		self.open
	}
}

impl Drop for Win32Clip {
	fn drop(&mut self) {
		if self.open {
			// SAFETY: this shell opened the clipboard.
			unsafe { CloseClipboard() };
		}
	}
}

/// One flavour on its way to the clipboard: the bytes are collected, and the commit hands Windows a moveable block of them.
pub(crate) struct Win32Writer<'a> {
	flavour: Flavour,
	data: Vec<u8>,
	/// Set by a successful commit of the RTF.
	rtf_ok: &'a mut bool,
}

impl Sink for Win32Writer<'_> {
	fn write(&mut self, bytes: &[u8]) -> bool {
		if self.data.len() + bytes.len() > limit(self.flavour) {
			return false;
		}
		if self.data.try_reserve(bytes.len()).is_err() {
			return false;
		}
		self.data.extend_from_slice(bytes);
		true
	}
}

impl ClipWrite for Win32Writer<'_> {
	fn commit(mut self) -> bool {
		// Text and RTF end in a NUL, as every program that reads them expects.
		let text = self.flavour != Flavour::Dib;
		if text && self.data.try_reserve(1).is_err() {
			return false;
		}
		if text {
			self.data.push(0);
		}
		// SAFETY: the block is allocated, filled while locked and given to the clipboard, which owns it from then on; when the
		// clipboard refuses it, it is freed here.
		let ok = unsafe {
			let mem = GlobalAlloc(GMEM_MOVEABLE, self.data.len());
			if mem.is_null() {
				return false;
			}
			let p = GlobalLock(mem);
			if p.is_null() {
				GlobalFree(mem);
				return false;
			}
			core::slice::from_raw_parts_mut(p, self.data.len()).copy_from_slice(&self.data);
			GlobalUnlock(mem);
			if SetClipboardData(format_of(self.flavour), mem).is_null() {
				GlobalFree(mem);
				false
			} else {
				true
			}
		};
		if ok && self.flavour == Flavour::Rtf {
			*self.rtf_ok = true;
		}
		ok
	}
}

/// One flavour read from the clipboard's block, which stays locked while the reader lives.
pub(crate) struct Win32Reader {
	mem: Handle,
	at: *const u8,
	len: usize,
	pos: usize,
}

impl Source for Win32Reader {
	fn read(&mut self, out: &mut [u8]) -> usize {
		let n = out.len().min(self.len - self.pos);
		// SAFETY: `at` points to `len` bytes of the locked block, and `pos + n` is within them.
		let from = unsafe { core::slice::from_raw_parts(self.at.add(self.pos), n) };
		out[..n].copy_from_slice(from);
		self.pos += n;
		n
	}
}

impl Drop for Win32Reader {
	fn drop(&mut self) {
		// SAFETY: the block was locked when the reader was made.
		unsafe { GlobalUnlock(self.mem) };
	}
}

impl Clipboard for Win32Clip {
	type W<'a> = Win32Writer<'a>;
	type R<'a> = Win32Reader;

	fn clear(&mut self) -> bool {
		// SAFETY: the clipboard is open when it is emptied; the shell's window becomes its owner.
		self.ensure_open() && unsafe { EmptyClipboard() } != 0
	}

	fn offers(&mut self) -> Flavours {
		available()
	}

	fn put(&mut self, f: Flavour) -> Option<Win32Writer<'_>> {
		if !self.open || (f != Flavour::Rtf && !self.rtf_ok) {
			return None;
		}
		Some(Win32Writer {
			flavour: f,
			data: Vec::new(),
			rtf_ok: &mut self.rtf_ok,
		})
	}

	fn get(&mut self, f: Flavour) -> Option<Win32Reader> {
		if !self.ensure_open() {
			return None;
		}
		// SAFETY: the clipboard is open; the block stays the clipboard's, locked only while the reader lives.
		unsafe {
			let mem = GetClipboardData(format_of(f));
			if mem.is_null() {
				return None;
			}
			let size = GlobalSize(mem);
			let p = GlobalLock(mem);
			if p.is_null() {
				return None;
			}
			let mut len = size.min(limit(f) + 1);
			if f != Flavour::Dib {
				// A text block is as long as its first NUL, whatever size Windows rounded the block to.
				let bytes = core::slice::from_raw_parts(p, len);
				len = bytes.iter().position(|&b| b == 0).unwrap_or(len);
			}
			if len == 0 || len > limit(f) {
				GlobalUnlock(mem);
				return None;
			}
			Some(Win32Reader { mem, at: p, len, pos: 0 })
		}
	}
}

/// Answers the Copy, Cut or Paste the last key or command asked for, if there was one.
pub(crate) fn serve(hwnd: Handle, sh: &mut Shell) {
	let Some(req) = sh.app.take_clip_request() else {
		return;
	};
	let mut cb = Win32Clip::new(hwnd);
	match req {
		ClipRequest::Copy { cut } => {
			sh.app.copy_to(&mut sh.store, &mut cb, cut);
		}
		ClipRequest::Paste => {
			sh.app.paste_from(&mut sh.store, &mut cb);
		}
	}
}

/// A menu is about to open: tells the app what the clipboard offers, and greys Paste in the menu when it is nothing Quill reads.
pub(crate) fn menu_opening(menu: Handle, sh: &mut Shell) {
	let offers = available();
	sh.app.set_clip_offers(offers);
	if let Some(i) = sh.cmds.iter().position(|c| *c == quill_app::Command::Paste) {
		let flags = if offers.is_empty() { MF_GRAYED } else { 0 };
		// SAFETY: a menu handle from the system; an item the menu does not hold is ignored.
		unsafe { EnableMenuItem(menu, 100 + i as u32, flags) };
	}
}
