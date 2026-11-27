// HOSTLINK.EXE: the ReactOS demo's guest helper. It joins the clipboard viewer chain with a hidden window, owns COM1, and speaks the host link
// (docs/host-link.md) with the page: the guest's copies go out, the page's copies come in. All the deciding is in `quill-hostlink-guest`
// (tested on the host); this file is the Win32 around it. One thread, one loop: messages, a read of the port, the helper's timers, one packet out.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `HOSTLINK.EXE` starts the helper (from the Run key); `HOSTLINK.EXE /quit` ends the one that is running; `/send FILE` is a stub that exits
//! with 3 until files arrive (WP-HOST-5). The helper makes no window anyone can see and no sound; when COM1 is absent or taken it ends quietly.
#![no_std]
#![no_main]
#![allow(static_mut_refs)]

mod clip;
mod port;
mod rt;
mod sys;

use core::ptr::{null, null_mut};
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use quill_hostlink_guest::{Buffers, Clipboard, Helper};

use clip::Win32Clip;
use port::Com;
use sys::*;

/// The window class, which `/quit` finds the running helper by.
const CLASS: &[u8] = b"LHQHostLink\0";
/// The largest message the helper takes: 4 MiB (a picture's limit) and a header.
const CLIP_LANE: usize = 4 * 1024 * 1024 + 4096;

/// The next window in the viewer chain, kept for the window procedure (which must pass every message on at once).
static NEXT_VIEWER: AtomicUsize = AtomicUsize::new(0);
/// A clipboard change arrived; the loop takes it and tells the helper.
static CHANGED: AtomicBool = AtomicBool::new(false);
/// Set while `SetClipboardViewer` runs: the message it sends the new viewer straight away is not a copy.
static JOINING: AtomicBool = AtomicBool::new(false);

static mut PAYLOAD: [u8; 4096] = [0; 4096];
static mut LANE_CTL: [u8; 1024] = [0; 1024];
static mut LANE_FILE: [u8; 256] = [0; 256];
static mut LANE_TIME: [u8; 256] = [0; 256];
static mut LANE_DISPLAY: [u8; 256] = [0; 256];
static mut READ: [u8; 8192] = [0; 8192];

unsafe extern "system" fn wnd_proc(hwnd: Handle, msg: u32, wparam: usize, lparam: isize) -> isize {
	match msg {
		WM_DRAWCLIPBOARD => {
			if !JOINING.load(Ordering::Relaxed) {
				clip::DRAWS.fetch_add(1, Ordering::Relaxed);
				CHANGED.store(true, Ordering::Relaxed);
			}
			// The chain only works if every viewer passes the message on.
			let next = NEXT_VIEWER.load(Ordering::Relaxed);
			if next != 0 {
				SendMessageA(next as Handle, msg, wparam, lparam);
			}
			0
		}
		WM_CHANGECBCHAIN => {
			// wparam is the window that left, lparam the one after it.
			let next = NEXT_VIEWER.load(Ordering::Relaxed);
			if wparam == next {
				NEXT_VIEWER.store(lparam as usize, Ordering::Relaxed);
			} else if next != 0 {
				SendMessageA(next as Handle, msg, wparam, lparam);
			}
			0
		}
		WM_CLOSE => {
			DestroyWindow(hwnd);
			0
		}
		WM_DESTROY => {
			ChangeClipboardChain(hwnd, NEXT_VIEWER.load(Ordering::Relaxed) as Handle);
			PostQuitMessage(0);
			0
		}
		WM_QUERYENDSESSION => 1,
		WM_ENDSESSION => {
			if wparam != 0 {
				ChangeClipboardChain(hwnd, NEXT_VIEWER.load(Ordering::Relaxed) as Handle);
			}
			0
		}
		_ => DefWindowProcA(hwnd, msg, wparam, lparam),
	}
}

/// What follows the program name on the command line.
fn arguments() -> &'static [u8] {
	// SAFETY: the command line is a NUL-terminated string that lives as long as the process.
	unsafe {
		let p = GetCommandLineA();
		let mut n = 0;
		while *p.add(n) != 0 {
			n += 1;
		}
		let all = core::slice::from_raw_parts(p, n);
		let mut i = 0;
		if all.first() == Some(&b'"') {
			i = 1;
			while all.get(i).is_some_and(|&c| c != b'"') {
				i += 1;
			}
			i += 1;
		} else {
			while all.get(i).is_some_and(|&c| c != b' ') {
				i += 1;
			}
		}
		all.get(i.min(n)..).unwrap_or(&[])
	}
}

fn has_switch(args: &[u8], name: &[u8]) -> bool {
	args.split(|&c| c == b' ').any(|word| {
		word.len() == name.len() + 1 && word.first() == Some(&b'/') && word.get(1..).is_some_and(|w| w.eq_ignore_ascii_case(name))
	})
}

/// Reserves and commits memory for a buffer; `None` when the system has none to give.
fn buffer(bytes: usize) -> Option<&'static mut [u8]> {
	// SAFETY: a fresh allocation nothing else points to, kept for the life of the process.
	unsafe {
		let p = VirtualAlloc(null(), bytes, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE);
		if p.is_null() {
			None
		} else {
			Some(core::slice::from_raw_parts_mut(p, bytes))
		}
	}
}

fn run() -> u32 {
	// SAFETY: one thread owns every static buffer and the window; the pointers to them are taken once.
	unsafe {
		// One helper at a time: a second one ends at once, quietly.
		let mutex = CreateMutexA(null(), 0, CLASS.as_ptr());
		if mutex.is_null() || GetLastError() == ERROR_ALREADY_EXISTS {
			return 0;
		}
		let Some(mut com) = Com::open() else { return 0 };
		let (Some(rx), Some(tx)) = (buffer(CLIP_LANE), buffer(CLIP_LANE)) else {
			com.close();
			return 0;
		};
		let instance = GetModuleHandleA(null());
		let class = WndClassA {
			style: 0,
			wnd_proc,
			cls_extra: 0,
			wnd_extra: 0,
			instance,
			icon: null_mut(),
			cursor: null_mut(),
			background: null_mut(),
			menu_name: null(),
			class_name: CLASS.as_ptr(),
		};
		RegisterClassA(&class);
		// Never shown: a top-level window the viewer chain and `/quit` can reach.
		let hwnd = CreateWindowExA(
			0,
			CLASS.as_ptr(),
			CLASS.as_ptr(),
			WS_POPUP,
			0,
			0,
			0,
			0,
			null_mut(),
			null_mut(),
			instance,
			null(),
		);
		if hwnd.is_null() {
			com.close();
			return 0;
		}
		let mut clipboard = Win32Clip::new(hwnd);
		JOINING.store(true, Ordering::Relaxed);
		let next = SetClipboardViewer(hwnd);
		NEXT_VIEWER.store(next as usize, Ordering::Relaxed);
		JOINING.store(false, Ordering::Relaxed);

		let mut helper = Helper::new(
			Buffers {
				payload: &mut PAYLOAD,
				lanes: [&mut LANE_CTL, rx, &mut LANE_FILE, &mut LANE_TIME, &mut LANE_DISPLAY],
				tx,
			},
			CLIP_LANE as u32,
		);
		helper.start(&mut clipboard);

		let mut msg: Msg = core::mem::zeroed();
		let mut quit = false;
		while !quit {
			while PeekMessageA(&mut msg, null_mut(), 0, 0, PM_REMOVE) != 0 {
				if msg.message == WM_QUIT {
					quit = true;
					break;
				}
				DispatchMessageA(&msg);
			}
			if quit {
				break;
			}
			let now = GetTickCount();
			// The port: take a finished read, then start the next.
			let mut failed = false;
			let mut got = match com.collect_read() {
				Ok(n) => n,
				Err(()) => {
					failed = true;
					None
				}
			};
			let mut spin = false;
			loop {
				if let Some(n) = got.take() {
					if let Some(bytes) = READ.get(..n as usize) {
						helper.on_bytes(bytes, &mut clipboard, now);
					}
				}
				match com.start_read(&mut READ) {
					Ok(Some(n)) if n > 0 => got = Some(n),
					Ok(Some(_)) => {
						// Finished with nothing: do not ask again until the loop has waited.
						spin = true;
						break;
					}
					Ok(None) => break,
					Err(()) => {
						failed = true;
						break;
					}
				}
			}
			if CHANGED.swap(false, Ordering::Relaxed) {
				helper.on_clipboard_change(now);
			}
			helper.tick(&mut clipboard, now);
			helper.pump(&mut com, &mut clipboard);
			if failed || helper.failed() {
				break;
			}
			// Wait for the port, a message or the helper's own next deadline; never longer than a second.
			let wait = if helper.busy() {
				0
			} else {
				let due = helper.next_due(GetTickCount()).unwrap_or(1_000).min(1_000);
				if spin {
					due.min(20)
				} else {
					due
				}
			};
			let events = [com.read_event()];
			MsgWaitForMultipleObjects(1, events.as_ptr(), 0, wait, QS_ALLINPUT);
		}
		clipboard.discard();
		com.close();
		DestroyWindow(hwnd);
		0
	}
}

#[no_mangle]
pub extern "C" fn start() -> ! {
	let args = arguments();
	let code = if has_switch(args, b"quit") {
		// SAFETY: finds the running helper's window and asks it to close.
		unsafe {
			let window = FindWindowA(CLASS.as_ptr(), null());
			if !window.is_null() {
				PostMessageA(window, WM_CLOSE, 0, 0);
			}
		}
		0
	} else if has_switch(args, b"send") {
		// Files come with WP-HOST-5: until then there is never a helper to hand one to.
		3
	} else {
		run()
	};
	// SAFETY: ends the process.
	unsafe { ExitProcess(code) }
}
