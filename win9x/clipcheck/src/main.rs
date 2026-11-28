// CLIPCHECK.EXE: puts files on the Windows clipboard and writes what is on it to files, so a script can check what QUILL32.EXE copied
// and feed it what another program would. Not shipped.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `CLIPCHECK put FORMAT FILE [FORMAT FILE ...]` empties the clipboard and puts each file on it as `rtf` (Rich Text Format), `text`
//! (CF_TEXT), `utext` (CF_UNICODETEXT, a UTF-16LE file) or `dib` (CF_DIB). A NUL ends `rtf`, `text` and `utext`.
//!
//! `CLIPCHECK hold MS` opens the clipboard, waits and closes it: another program that has it busy.
//!
//! `CLIPCHECK get PREFIX` writes PREFIX.RTF, PREFIX.TXT, PREFIX.UTX and PREFIX.DIB for each of those formats that the clipboard offers,
//! and PREFIX.LST: one line per format on it, `ID SIZE NAME`.
#![no_std]
#![no_main]

extern crate alloc;

#[path = "../../quill-win9x/src/rt.rs"]
mod rt;
mod sys {
	pub use super::win::{ExitProcess, GetProcessHeap, HeapAlloc, HeapFree};
}

mod win {
	use core::ffi::c_void;
	pub type Handle = *mut c_void;
	#[link(name = "kernel32", kind = "raw-dylib", import_name_type = "undecorated")]
	extern "system" {
		pub fn ExitProcess(code: u32) -> !;
		pub fn GetProcessHeap() -> Handle;
		pub fn HeapAlloc(heap: Handle, flags: u32, bytes: usize) -> *mut u8;
		pub fn HeapFree(heap: Handle, flags: u32, p: *mut u8) -> i32;
		pub fn GetCommandLineA() -> *const u8;
		pub fn CreateFileA(
			name: *const u8,
			access: u32,
			share: u32,
			security: *const c_void,
			disposition: u32,
			flags: u32,
			template: Handle,
		) -> Handle;
		pub fn ReadFile(file: Handle, buf: *mut u8, len: u32, read: *mut u32, overlapped: *mut c_void) -> i32;
		pub fn WriteFile(file: Handle, buf: *const u8, len: u32, written: *mut u32, overlapped: *mut c_void) -> i32;
		pub fn CloseHandle(h: Handle) -> i32;
		pub fn GetFileSize(file: Handle, high: *mut u32) -> u32;
		pub fn GlobalAlloc(flags: u32, bytes: usize) -> Handle;
		pub fn GlobalLock(mem: Handle) -> *mut u8;
		pub fn GlobalUnlock(mem: Handle) -> i32;
		pub fn GlobalSize(mem: Handle) -> usize;
		pub fn Sleep(ms: u32);
	}
	#[link(name = "user32", kind = "raw-dylib", import_name_type = "undecorated")]
	extern "system" {
		pub fn OpenClipboard(hwnd: Handle) -> i32;
		pub fn CloseClipboard() -> i32;
		pub fn EmptyClipboard() -> i32;
		pub fn SetClipboardData(format: u32, mem: Handle) -> Handle;
		pub fn GetClipboardData(format: u32) -> Handle;
		pub fn EnumClipboardFormats(format: u32) -> u32;
		pub fn GetClipboardFormatNameA(format: u32, out: *mut u8, max: i32) -> i32;
		pub fn RegisterClipboardFormatA(name: *const u8) -> u32;
	}
}

use alloc::vec::Vec;
use core::ptr::{null, null_mut};
use win::*;

const CF_TEXT: u32 = 1;
const CF_DIB: u32 = 8;
const CF_UNICODETEXT: u32 = 13;

fn rtf() -> u32 {
	// SAFETY: NUL-terminated name.
	unsafe { RegisterClipboardFormatA(b"Rich Text Format\0".as_ptr()) }
}

fn cstr(s: &[u8]) -> Vec<u8> {
	let mut v = s.to_vec();
	v.push(0);
	v
}

fn read_file(name: &[u8]) -> Option<Vec<u8>> {
	let c = cstr(name);
	// SAFETY: plain file calls on a handle this function opens and closes.
	unsafe {
		let h = CreateFileA(c.as_ptr(), 0x8000_0000, 1, null(), 3, 0, null_mut());
		if h as isize == -1 {
			return None;
		}
		let n = GetFileSize(h, null_mut()) as usize;
		let mut v = alloc::vec![0u8; n];
		let mut got = 0u32;
		let ok = ReadFile(h, v.as_mut_ptr(), n as u32, &mut got, null_mut());
		CloseHandle(h);
		(ok != 0 && got as usize == n).then_some(v)
	}
}

fn write_file(name: &[u8], data: &[u8]) -> bool {
	let c = cstr(name);
	// SAFETY: as above.
	unsafe {
		let h = CreateFileA(c.as_ptr(), 0x4000_0000, 0, null(), 2, 0, null_mut());
		if h as isize == -1 {
			return false;
		}
		let mut put = 0u32;
		let ok = WriteFile(h, data.as_ptr(), data.len() as u32, &mut put, null_mut());
		CloseHandle(h);
		ok != 0
	}
}

fn args() -> Vec<Vec<u8>> {
	// SAFETY: the command line is a NUL-terminated string that lives as long as the process.
	let mut p = unsafe { GetCommandLineA() };
	let mut line = Vec::new();
	// SAFETY: reads up to the terminator.
	unsafe {
		while *p != 0 {
			line.push(*p);
			p = p.add(1);
		}
	}
	let mut out: Vec<Vec<u8>> = Vec::new();
	let mut quoted = false;
	let mut cur: Vec<u8> = Vec::new();
	let mut any = false;
	for &b in &line {
		match b {
			b'"' => {
				quoted = !quoted;
				any = true;
			}
			b' ' if !quoted => {
				if any {
					out.push(core::mem::take(&mut cur));
					any = false;
				}
			}
			_ => {
				cur.push(b);
				any = true;
			}
		}
	}
	if any {
		out.push(cur);
	}
	out
}

unsafe fn put(format: u32, data: &[u8], nul: usize) -> bool {
	let mem = GlobalAlloc(2, data.len() + nul);
	if mem.is_null() {
		return false;
	}
	let p = GlobalLock(mem);
	if p.is_null() {
		return false;
	}
	core::ptr::copy_nonoverlapping(data.as_ptr(), p, data.len());
	for i in 0..nul {
		*p.add(data.len() + i) = 0;
	}
	GlobalUnlock(mem);
	!SetClipboardData(format, mem).is_null()
}

fn run() -> u32 {
	let a = args();
	let Some(cmd) = a.get(1) else {
		return 2;
	};
	// SAFETY: plain clipboard calls; the clipboard is opened and closed here.
	unsafe {
		if OpenClipboard(null_mut()) == 0 {
			return 3;
		}
		let mut code = 0;
		if cmd == b"put" {
			EmptyClipboard();
			let mut i = 2;
			while i + 1 < a.len() {
				let (Some(kind), Some(data)) = (a.get(i), read_file(&a[i + 1])) else {
					code = 4;
					break;
				};
				let ok = match kind.as_slice() {
					b"rtf" => put(rtf(), &data, 1),
					b"text" => put(CF_TEXT, &data, 1),
					b"utext" => put(CF_UNICODETEXT, &data, 2),
					b"dib" => put(CF_DIB, &data, 0),
					_ => false,
				};
				if !ok {
					code = 5;
				}
				i += 2;
			}
		} else if cmd == b"hold" {
			let ms = a
				.get(2)
				.map_or(0, |n| n.iter().fold(0u32, |v, d| v * 10 + u32::from(d - b'0')));
			Sleep(ms);
		} else if cmd == b"get" {
			let Some(prefix) = a.get(2) else {
				CloseClipboard();
				return 2;
			};
			let named = |ext: &[u8]| {
				let mut n = prefix.clone();
				n.extend_from_slice(ext);
				n
			};
			let mut list = Vec::new();
			let mut f = 0;
			loop {
				f = EnumClipboardFormats(f);
				if f == 0 {
					break;
				}
				let h = GetClipboardData(f);
				let size = if h.is_null() { 0 } else { GlobalSize(h) };
				let mut name = [0u8; 64];
				let n = GetClipboardFormatNameA(f, name.as_mut_ptr(), 64).max(0) as usize;
				let mut line = Vec::new();
				push_num(&mut line, f as usize);
				line.push(b' ');
				push_num(&mut line, size);
				line.push(b' ');
				line.extend_from_slice(&name[..n]);
				line.push(b'\n');
				list.extend_from_slice(&line);
			}
			write_file(&named(b".LST"), &list);
			for (format, ext, text) in [
				(rtf(), &b".RTF"[..], 1usize),
				(CF_TEXT, b".TXT", 1),
				(CF_UNICODETEXT, b".UTX", 2),
				(CF_DIB, b".DIB", 0),
			] {
				let h = GetClipboardData(format);
				if h.is_null() {
					continue;
				}
				let size = GlobalSize(h);
				let p = GlobalLock(h);
				if p.is_null() {
					continue;
				}
				let mut data = core::slice::from_raw_parts(p, size);
				if text == 1 {
					if let Some(n) = data.iter().position(|&b| b == 0) {
						data = &data[..n];
					}
				}
				if text == 2 {
					let mut n = 0;
					while n + 1 < data.len() && (data[n] != 0 || data[n + 1] != 0) {
						n += 2;
					}
					data = &data[..n];
				}
				write_file(&named(ext), data);
				GlobalUnlock(h);
			}
		} else {
			code = 2;
		}
		CloseClipboard();
		code
	}
}

fn push_num(out: &mut Vec<u8>, mut n: usize) {
	let mut digits = [0u8; 12];
	let mut i = digits.len();
	loop {
		i -= 1;
		digits[i] = b'0' + (n % 10) as u8;
		n /= 10;
		if n == 0 {
			break;
		}
	}
	out.extend_from_slice(&digits[i..]);
}

#[no_mangle]
pub extern "C" fn start() -> ! {
	let code = run();
	// SAFETY: ends the process.
	unsafe { ExitProcess(code) }
}
