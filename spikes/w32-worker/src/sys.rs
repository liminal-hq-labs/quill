// The Win32 calls the spike's main thread makes: ANSI entry points only, imported by name (raw-dylib), nothing newer than Windows 95.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::ffi::c_void;

pub type Handle = *mut c_void;

pub const WM_DESTROY: u32 = 0x0002;
pub const WM_PAINT: u32 = 0x000F;
pub const WM_ERASEBKGND: u32 = 0x0014;
pub const WM_TIMER: u32 = 0x0113;
pub const WM_APP: u32 = 0x8000;
pub const WM_QUIT: u32 = 0x0012;
pub const PM_REMOVE: u32 = 1;
pub const WS_POPUP: u32 = 0x8000_0000;
pub const WS_VISIBLE: u32 = 0x1000_0000;
pub const GENERIC_READ: u32 = 0x8000_0000;
pub const OPEN_EXISTING: u32 = 3;
pub const WAIT_OBJECT_0: u32 = 0;
pub const THREAD_PRIORITY_LOWEST: i32 = -2;
pub const THREAD_PRIORITY_IDLE: i32 = -15;
pub const THREAD_PRIORITY_TIME_CRITICAL: i32 = 15;
pub const TRANSPARENT: i32 = 1;

#[repr(C)]
pub struct WndClassA {
	pub style: u32,
	pub wnd_proc: unsafe extern "system" fn(Handle, u32, usize, isize) -> isize,
	pub cls_extra: i32,
	pub wnd_extra: i32,
	pub instance: Handle,
	pub icon: Handle,
	pub cursor: Handle,
	pub background: Handle,
	pub menu_name: *const u8,
	pub class_name: *const u8,
}

#[repr(C)]
pub struct Point {
	pub x: i32,
	pub y: i32,
}

#[repr(C)]
pub struct Msg {
	pub hwnd: Handle,
	pub message: u32,
	pub wparam: usize,
	pub lparam: isize,
	pub time: u32,
	pub pt: Point,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct Rect {
	pub left: i32,
	pub top: i32,
	pub right: i32,
	pub bottom: i32,
}

#[repr(C)]
pub struct PaintStruct {
	pub hdc: Handle,
	pub erase: i32,
	pub paint: Rect,
	pub restore: i32,
	pub inc_update: i32,
	pub reserved: [u8; 32],
}

#[link(name = "kernel32", kind = "raw-dylib", import_name_type = "undecorated")]
extern "system" {
	pub fn ExitProcess(code: u32) -> !;
	pub fn GetModuleHandleA(name: *const u8) -> Handle;
	pub fn GetModuleFileNameA(module: Handle, name: *mut u8, size: u32) -> u32;
	pub fn GetProcessHeap() -> Handle;
	pub fn HeapAlloc(heap: Handle, flags: u32, bytes: usize) -> *mut u8;
	pub fn HeapFree(heap: Handle, flags: u32, p: *mut u8) -> i32;
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
	pub fn GetTickCount() -> u32;
	pub fn QueryPerformanceCounter(count: *mut u64) -> i32;
	pub fn QueryPerformanceFrequency(freq: *mut u64) -> i32;
	pub fn CreateThread(
		security: *const c_void,
		stack: u32,
		start: unsafe extern "system" fn(*mut c_void) -> u32,
		param: *mut c_void,
		flags: u32,
		id: *mut u32,
	) -> Handle;
	pub fn CreateEventA(security: *const c_void, manual: i32, initial: i32, name: *const u8) -> Handle;
	pub fn WaitForSingleObject(h: Handle, ms: u32) -> u32;
	pub fn SetThreadPriority(thread: Handle, priority: i32) -> i32;
	pub fn Sleep(ms: u32);
}

#[link(name = "user32", kind = "raw-dylib", import_name_type = "undecorated")]
extern "system" {
	pub fn RegisterClassA(class: *const WndClassA) -> u16;
	pub fn CreateWindowExA(
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
	pub fn DefWindowProcA(hwnd: Handle, msg: u32, wparam: usize, lparam: isize) -> isize;
	pub fn GetMessageA(msg: *mut Msg, hwnd: Handle, min: u32, max: u32) -> i32;
	pub fn PeekMessageA(msg: *mut Msg, hwnd: Handle, min: u32, max: u32, remove: u32) -> i32;
	pub fn DispatchMessageA(msg: *const Msg) -> isize;
	pub fn PostMessageA(hwnd: Handle, msg: u32, wparam: usize, lparam: isize) -> i32;
	pub fn PostQuitMessage(code: i32);
	pub fn DestroyWindow(hwnd: Handle) -> i32;
	pub fn BeginPaint(hwnd: Handle, ps: *mut PaintStruct) -> Handle;
	pub fn EndPaint(hwnd: Handle, ps: *const PaintStruct) -> i32;
	pub fn InvalidateRect(hwnd: Handle, rect: *const Rect, erase: i32) -> i32;
	pub fn FillRect(dc: Handle, rect: *const Rect, brush: Handle) -> i32;
	pub fn SetTimer(hwnd: Handle, id: usize, ms: u32, proc_: *const c_void) -> usize;
}

#[link(name = "gdi32", kind = "raw-dylib", import_name_type = "undecorated")]
extern "system" {
	pub fn TextOutA(dc: Handle, x: i32, y: i32, text: *const u8, len: i32) -> i32;
	pub fn CreateSolidBrush(color: u32) -> Handle;
	pub fn SetBkMode(dc: Handle, mode: i32) -> i32;
}
