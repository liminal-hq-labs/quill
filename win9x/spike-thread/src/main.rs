// THREAD95.EXE: the smallest test of the threading model J0 proposes for Windows 95. The window thread makes a hidden window and
// starts a worker with `CreateThread`; the worker computes a checksum, posts it to the window with `PostMessageA(WM_APP)` three
// times, and ends; the window thread checks the checksum in its window procedure, checks that the message arrived on its own
// thread and not the worker's, joins the worker with `WaitForSingleObject` and writes THREAD.LOG next to the program. The exit
// code is 0 when every check held.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![no_std]
#![no_main]

use core::ffi::c_void;
use core::ptr::{null, null_mut};
use core::sync::atomic::{AtomicU32, Ordering};

type Handle = *mut c_void;

const WM_TIMER: u32 = 0x0113;
const WM_APP: u32 = 0x8000;
const ROUNDS: u32 = 3;

#[repr(C)]
struct WndClassA {
	style: u32,
	wnd_proc: unsafe extern "system" fn(Handle, u32, usize, isize) -> isize,
	cls_extra: i32,
	wnd_extra: i32,
	instance: Handle,
	icon: Handle,
	cursor: Handle,
	background: Handle,
	menu_name: *const u8,
	class_name: *const u8,
}

#[repr(C)]
struct Point {
	x: i32,
	y: i32,
}

#[repr(C)]
struct Msg {
	hwnd: Handle,
	message: u32,
	wparam: usize,
	lparam: isize,
	time: u32,
	pt: Point,
}

#[link(name = "kernel32", kind = "raw-dylib", import_name_type = "undecorated")]
extern "system" {
	fn ExitProcess(code: u32) -> !;
	fn GetModuleHandleA(name: *const u8) -> Handle;
	fn GetCurrentThreadId() -> u32;
	fn CreateThread(
		attrs: *const c_void,
		stack: usize,
		start: unsafe extern "system" fn(*mut c_void) -> u32,
		param: *mut c_void,
		flags: u32,
		id: *mut u32,
	) -> Handle;
	fn WaitForSingleObject(h: Handle, ms: u32) -> u32;
	fn Sleep(ms: u32);
	fn CloseHandle(h: Handle) -> i32;
	fn CreateFileA(
		name: *const u8,
		access: u32,
		share: u32,
		security: *const c_void,
		disposition: u32,
		flags: u32,
		template: Handle,
	) -> Handle;
	fn WriteFile(file: Handle, buf: *const u8, len: u32, written: *mut u32, overlapped: *mut c_void) -> i32;
}

#[link(name = "user32", kind = "raw-dylib", import_name_type = "undecorated")]
extern "system" {
	fn RegisterClassA(class: *const WndClassA) -> u16;
	fn CreateWindowExA(
		ex_style: u32,
		class_name: *const u8,
		title: *const u8,
		style: u32,
		x: i32,
		y: i32,
		width: i32,
		height: i32,
		parent: Handle,
		menu: Handle,
		instance: Handle,
		param: *const c_void,
	) -> Handle;
	fn DefWindowProcA(hwnd: Handle, msg: u32, wparam: usize, lparam: isize) -> isize;
	fn GetMessageA(msg: *mut Msg, hwnd: Handle, min: u32, max: u32) -> i32;
	fn DispatchMessageA(msg: *const Msg) -> isize;
	fn PostMessageA(hwnd: Handle, msg: u32, wparam: usize, lparam: isize) -> i32;
	fn PostQuitMessage(code: i32);
	fn SetTimer(hwnd: Handle, id: usize, ms: u32, proc_: *const c_void) -> usize;
}

static MAIN_TID: AtomicU32 = AtomicU32::new(0);
static WORKER_TID: AtomicU32 = AtomicU32::new(0);
static HWND: AtomicU32 = AtomicU32::new(0);
static ARRIVED: AtomicU32 = AtomicU32::new(0);
static SUMS_OK: AtomicU32 = AtomicU32::new(0);
static THREAD_OK: AtomicU32 = AtomicU32::new(0);
static TIMED_OUT: AtomicU32 = AtomicU32::new(0);

/// A checksum both threads can compute: the window thread recomputes it to check the worker's.
fn checksum(round: u32) -> u32 {
	let mut h: u32 = 0x811c_9dc5 ^ round;
	let mut i = 0u32;
	while i < 200_000 {
		h = (h ^ (i & 0xFF)).wrapping_mul(0x0100_0193);
		i += 1;
	}
	h
}

unsafe extern "system" fn worker(_: *mut c_void) -> u32 {
	WORKER_TID.store(GetCurrentThreadId(), Ordering::SeqCst);
	let hwnd = HWND.load(Ordering::SeqCst) as usize as Handle;
	let mut round = 1;
	while round <= ROUNDS {
		Sleep(20);
		PostMessageA(hwnd, WM_APP, round as usize, checksum(round) as isize);
		round += 1;
	}
	0
}

unsafe extern "system" fn wnd_proc(hwnd: Handle, msg: u32, wparam: usize, lparam: isize) -> isize {
	match msg {
		WM_APP => {
			let round = wparam as u32;
			if lparam as u32 == checksum(round) {
				SUMS_OK.fetch_add(1, Ordering::SeqCst);
			}
			// The window procedure runs on the window thread, never on the worker.
			if GetCurrentThreadId() == MAIN_TID.load(Ordering::SeqCst) {
				THREAD_OK.fetch_add(1, Ordering::SeqCst);
			}
			if ARRIVED.fetch_add(1, Ordering::SeqCst) + 1 == ROUNDS {
				PostQuitMessage(0);
			}
			0
		}
		WM_TIMER => {
			TIMED_OUT.store(1, Ordering::SeqCst);
			PostQuitMessage(1);
			0
		}
		_ => DefWindowProcA(hwnd, msg, wparam, lparam),
	}
}

fn hex(out: &mut [u8], at: usize, v: u32) {
	let mut i = 0;
	while i < 8 {
		let nib = ((v >> (28 - 4 * i)) & 15) as u8;
		out[at + i] = if nib < 10 { b'0' + nib } else { b'a' + nib - 10 };
		i += 1;
	}
}

fn put(out: &mut [u8], at: usize, s: &[u8]) -> usize {
	let mut i = 0;
	while i < s.len() {
		out[at + i] = s[i];
		i += 1;
	}
	at + s.len()
}

#[no_mangle]
pub extern "C" fn start() -> ! {
	// SAFETY: plain Win32 calls from the one window thread; the worker touches only atomics and `PostMessageA`.
	unsafe {
		let inst = GetModuleHandleA(null());
		let class = b"Thread95\0";
		let wc = WndClassA {
			style: 0,
			wnd_proc,
			cls_extra: 0,
			wnd_extra: 0,
			instance: inst,
			icon: null_mut(),
			cursor: null_mut(),
			background: null_mut(),
			menu_name: null(),
			class_name: class.as_ptr(),
		};
		RegisterClassA(&wc);
		MAIN_TID.store(GetCurrentThreadId(), Ordering::SeqCst);
		let hwnd = CreateWindowExA(
			0,
			class.as_ptr(),
			b"J0\0".as_ptr(),
			0,
			0,
			0,
			10,
			10,
			null_mut(),
			null_mut(),
			inst,
			null(),
		);
		HWND.store(hwnd as usize as u32, Ordering::SeqCst);
		SetTimer(hwnd, 1, 8000, null());
		let mut tid = 0u32;
		let th = CreateThread(null(), 0x10000, worker, null_mut(), 0, &mut tid);
		let mut msg = Msg {
			hwnd: null_mut(),
			message: 0,
			wparam: 0,
			lparam: 0,
			time: 0,
			pt: Point { x: 0, y: 0 },
		};
		while GetMessageA(&mut msg, null_mut(), 0, 0) > 0 {
			DispatchMessageA(&msg);
		}
		let join = if th.is_null() {
			0xFFFF_FFFF
		} else {
			WaitForSingleObject(th, 5000)
		};
		if !th.is_null() {
			CloseHandle(th);
		}
		let worker_tid = WORKER_TID.load(Ordering::SeqCst);
		let main_tid = MAIN_TID.load(Ordering::SeqCst);
		let ok = !th.is_null()
			&& join == 0
			&& ARRIVED.load(Ordering::SeqCst) == ROUNDS
			&& SUMS_OK.load(Ordering::SeqCst) == ROUNDS
			&& THREAD_OK.load(Ordering::SeqCst) == ROUNDS
			&& TIMED_OUT.load(Ordering::SeqCst) == 0
			&& worker_tid != 0
			&& worker_tid != main_tid
			&& worker_tid == tid;
		let mut line = [b' '; 128];
		let mut n = put(&mut line, 0, b"main=");
		hex(&mut line, n, main_tid);
		n = put(&mut line, n + 8, b" worker=");
		hex(&mut line, n, worker_tid);
		n = put(&mut line, n + 8, b" arrived=");
		hex(&mut line, n, ARRIVED.load(Ordering::SeqCst));
		n = put(&mut line, n + 8, b" sums=");
		hex(&mut line, n, SUMS_OK.load(Ordering::SeqCst));
		n = put(&mut line, n + 8, b" on_window_thread=");
		hex(&mut line, n, THREAD_OK.load(Ordering::SeqCst));
		n = put(&mut line, n + 8, b" join=");
		hex(&mut line, n, join);
		n = put(&mut line, n + 8, if ok { b" result=PASS\r\n" } else { b" result=FAIL\r\n" });
		let f = CreateFileA(b"THREAD.LOG\0".as_ptr(), 0x4000_0000, 0, null(), 2, 0x80, null_mut());
		if f as usize != usize::MAX {
			let mut w = 0u32;
			WriteFile(f, line.as_ptr(), n as u32, &mut w, null_mut());
			CloseHandle(f);
		}
		ExitProcess(if ok { 0 } else { 1 })
	}
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
	// SAFETY: ends the process.
	unsafe { ExitProcess(3) }
}
