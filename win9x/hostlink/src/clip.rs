// The guest's clipboard for the helper: `CF_UNICODETEXT` (or `CF_TEXT` when that is all there is), `Rich Text Format` and `CF_DIB`, read and
// written the way the system's own programs do. ReactOS makes the other text and picture formats itself, both ways (WP-HOST-1), so the helper
// sets one text format and reads one.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::ffi::c_void;
use core::mem::transmute;
use core::ptr::null_mut;
use core::sync::atomic::{AtomicU32, Ordering};

use quill_hostlink_guest::text::{cp1252_to_utf8, utf16le_to_utf8, utf8_to_utf16le};
use quill_hostlink_guest::{Clipboard, Format};

use crate::sys::*;

/// Counts the viewer's messages: the sequence number on a system that has no `GetClipboardSequenceNumber` (Windows 95).
pub static DRAWS: AtomicU32 = AtomicU32::new(0);

pub struct Win32Clip {
	pub hwnd: Handle,
	/// The registered number of `Rich Text Format`, looked up at start (never assumed).
	pub rtf: u32,
	sequence_fn: Option<unsafe extern "system" fn() -> u32>,
	/// Memory waiting for `commit`: text, rich text and picture.
	staged: [Handle; 3],
}

impl Win32Clip {
	pub fn new(hwnd: Handle) -> Win32Clip {
		// SAFETY: looks a function up in a module that is already loaded; the transmute is to the function's documented type.
		unsafe {
			let rtf = RegisterClipboardFormatA(b"Rich Text Format\0".as_ptr());
			let user32 = GetModuleHandleA(b"user32.dll\0".as_ptr());
			let found = GetProcAddress(user32, b"GetClipboardSequenceNumber\0".as_ptr());
			let sequence_fn = if found.is_null() {
				None
			} else {
				Some(transmute::<*const c_void, unsafe extern "system" fn() -> u32>(found))
			};
			Win32Clip {
				hwnd,
				rtf,
				sequence_fn,
				staged: [null_mut(); 3],
			}
		}
	}

	/// The clipboard may be held for a moment by another program: try for about 100 ms.
	fn open(&self) -> bool {
		for _ in 0..20 {
			// SAFETY: plain calls.
			unsafe {
				if OpenClipboard(self.hwnd) != 0 {
					return true;
				}
				Sleep(5);
			}
		}
		false
	}

	fn number(&self, format: Format) -> u32 {
		match format {
			Format::Text => CF_UNICODETEXT,
			Format::Rtf => self.rtf,
			Format::Dib => CF_DIB,
		}
	}

	/// Runs `f` on the bytes of the format's block with the clipboard open, then closes it. `None` when the format is not there.
	fn with_block<T>(&self, format: Format, f: impl FnOnce(&[u8], bool) -> Option<T>) -> Option<T> {
		if !self.open() {
			return None;
		}
		// SAFETY: the clipboard is open, so the handle stays valid until CloseClipboard; the block is locked while the slice is used.
		let result = unsafe {
			let available = |number: u32| number != 0 && IsClipboardFormatAvailable(number) != 0;
			let (handle, wide) = match format {
				Format::Text if available(CF_UNICODETEXT) => (GetClipboardData(CF_UNICODETEXT), true),
				Format::Text if available(CF_TEXT) => (GetClipboardData(CF_TEXT), false),
				Format::Text => (null_mut(), true),
				Format::Rtf | Format::Dib => {
					let number = self.number(format);
					(
						if available(number) {
							GetClipboardData(number)
						} else {
							null_mut()
						},
						true,
					)
				}
			};
			if handle.is_null() {
				None
			} else {
				let size = GlobalSize(handle);
				let pointer = GlobalLock(handle);
				let out = if pointer.is_null() {
					None
				} else {
					f(core::slice::from_raw_parts(pointer, size), wide)
				};
				GlobalUnlock(handle);
				out
			}
		};
		// SAFETY: closes what `open` opened.
		unsafe { CloseClipboard() };
		result
	}

	fn drop_staged(&mut self) {
		for slot in self.staged.iter_mut() {
			if !slot.is_null() {
				// SAFETY: a block this struct allocated and never gave to the clipboard.
				unsafe { GlobalFree(*slot) };
				*slot = null_mut();
			}
		}
	}

	fn slot(format: Format) -> usize {
		match format {
			Format::Text => 0,
			Format::Rtf => 1,
			Format::Dib => 2,
		}
	}
}

/// The bytes of a text block up to its first NUL.
fn until_nul(bytes: &[u8]) -> &[u8] {
	let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
	bytes.get(..end).unwrap_or(&[])
}

impl Clipboard for Win32Clip {
	fn sequence(&mut self) -> u32 {
		match self.sequence_fn {
			// SAFETY: the system's own function, no arguments.
			Some(f) => unsafe { f() },
			None => DRAWS.load(Ordering::Relaxed),
		}
	}

	fn size(&mut self, format: Format) -> Option<usize> {
		self.with_block(format, |bytes, wide| match format {
			Format::Text if wide => utf16le_to_utf8(bytes, None),
			Format::Text => cp1252_to_utf8(bytes, None),
			Format::Rtf => Some(until_nul(bytes).len()),
			Format::Dib => Some(bytes.len()),
		})
	}

	fn read(&mut self, format: Format, out: &mut [u8]) -> Option<usize> {
		self.with_block(format, |bytes, wide| match format {
			Format::Text if wide => utf16le_to_utf8(bytes, Some(out)),
			Format::Text => cp1252_to_utf8(bytes, Some(out)),
			Format::Rtf => {
				let rtf = until_nul(bytes);
				out.get_mut(..rtf.len())?.copy_from_slice(rtf);
				Some(rtf.len())
			}
			Format::Dib => {
				out.get_mut(..bytes.len())?.copy_from_slice(bytes);
				Some(bytes.len())
			}
		})
	}

	fn stage(&mut self, format: Format, data: &[u8]) -> bool {
		let slot = Win32Clip::slot(format);
		let size = match format {
			Format::Text => utf8_to_utf16le(data, None, true),
			Format::Rtf => Some(data.len() + 1),
			Format::Dib => Some(data.len()),
		};
		let Some(size) = size else { return false };
		// SAFETY: a movable block of the size worked out above, locked while it is filled, owned by this struct until `commit` hands it over.
		unsafe {
			let handle = GlobalAlloc(GMEM_MOVEABLE | GMEM_DDESHARE, size.max(1));
			if handle.is_null() {
				return false;
			}
			let pointer = GlobalLock(handle);
			if pointer.is_null() {
				GlobalFree(handle);
				return false;
			}
			let block = core::slice::from_raw_parts_mut(pointer, size);
			let filled = match format {
				Format::Text => utf8_to_utf16le(data, Some(&mut *block), true).is_some(),
				Format::Rtf | Format::Dib => {
					if let Some(front) = block.get_mut(..data.len()) {
						front.copy_from_slice(data);
					}
					if format == Format::Rtf {
						if let Some(last) = block.get_mut(data.len()) {
							*last = 0;
						}
					}
					true
				}
			};
			GlobalUnlock(handle);
			if !filled {
				GlobalFree(handle);
				return false;
			}
			if let Some(old) = self.staged.get_mut(slot) {
				if !old.is_null() {
					GlobalFree(*old);
				}
				*old = handle;
			}
		}
		true
	}

	fn commit(&mut self) -> Option<u32> {
		if !self.open() {
			return None;
		}
		// SAFETY: the clipboard is open under our window; a block `SetClipboardData` accepts belongs to the system from then on.
		unsafe {
			EmptyClipboard();
			for format in [Format::Text, Format::Rtf, Format::Dib] {
				let slot = Win32Clip::slot(format);
				let handle = self.staged.get(slot).copied().unwrap_or(null_mut());
				if handle.is_null() {
					continue;
				}
				if SetClipboardData(self.number(format), handle).is_null() {
					GlobalFree(handle);
				}
				if let Some(held) = self.staged.get_mut(slot) {
					*held = null_mut();
				}
			}
			CloseClipboard();
		}
		self.drop_staged();
		Some(self.sequence())
	}

	fn discard(&mut self) {
		self.drop_staged();
	}
}
