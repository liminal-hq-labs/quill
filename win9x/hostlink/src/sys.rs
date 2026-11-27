// The Win32 calls the helper makes: ANSI entry points only, imported by name, nothing newer than Windows 95 (the one later call is looked up
// with GetProcAddress). The allowlist is win9x/tools/hostlink-imports.allow.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::ffi::c_void;

pub type Handle = *mut c_void;

pub const WM_DESTROY: u32 = 0x0002;
pub const WM_CLOSE: u32 = 0x0010;
pub const WM_QUERYENDSESSION: u32 = 0x0011;
pub const WM_ENDSESSION: u32 = 0x0016;
pub const WM_DRAWCLIPBOARD: u32 = 0x0308;
pub const WM_CHANGECBCHAIN: u32 = 0x030D;
pub const WM_QUIT: u32 = 0x0012;
pub const WS_POPUP: u32 = 0x8000_0000;
pub const GENERIC_READ: u32 = 0x8000_0000;
pub const GENERIC_WRITE: u32 = 0x4000_0000;
pub const OPEN_EXISTING: u32 = 3;
pub const FILE_FLAG_OVERLAPPED: u32 = 0x4000_0000;
pub const INVALID_HANDLE_VALUE: Handle = -1isize as Handle;
pub const GMEM_MOVEABLE: u32 = 2;
pub const GMEM_DDESHARE: u32 = 0x2000;
pub const CF_TEXT: u32 = 1;
pub const CF_DIB: u32 = 8;
pub const CF_UNICODETEXT: u32 = 13;
pub const ERROR_IO_PENDING: u32 = 997;
pub const ERROR_ALREADY_EXISTS: u32 = 183;
pub const WAIT_OBJECT_0: u32 = 0;
pub const MAXDWORD: u32 = 0xFFFF_FFFF;
pub const PURGE_ALL: u32 = 0x0F;
/// `PURGE_TXABORT | PURGE_TXCLEAR`.
pub const PURGE_TX: u32 = 0x05;
pub const PM_REMOVE: u32 = 1;
pub const QS_ALLINPUT: u32 = 0x04FF;
pub const MEM_COMMIT: u32 = 0x1000;
pub const MEM_RESERVE: u32 = 0x2000;
pub const PAGE_READWRITE: u32 = 4;

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
pub struct Overlapped {
	pub internal: usize,
	pub internal_high: usize,
	pub offset: u32,
	pub offset_high: u32,
	pub event: Handle,
}

#[repr(C)]
pub struct Dcb {
	pub length: u32,
	pub baud: u32,
	pub flags: u32,
	pub reserved: u16,
	pub xon_lim: u16,
	pub xoff_lim: u16,
	pub byte_size: u8,
	pub parity: u8,
	pub stop_bits: u8,
	pub chars: [u8; 5],
	pub reserved1: u16,
}

#[repr(C)]
pub struct CommTimeouts {
	pub read_interval: u32,
	pub read_multiplier: u32,
	pub read_constant: u32,
	pub write_multiplier: u32,
	pub write_constant: u32,
}

#[link(name = "kernel32", kind = "raw-dylib", import_name_type = "undecorated")]
extern "system" {
	pub fn ExitProcess(code: u32) -> !;
	pub fn GetModuleHandleA(name: *const u8) -> Handle;
	pub fn GetProcAddress(module: Handle, name: *const u8) -> *const c_void;
	pub fn GetCommandLineA() -> *const u8;
	pub fn GetLastError() -> u32;
	pub fn GetTickCount() -> u32;
	pub fn Sleep(ms: u32);
	pub fn CreateFileA(
		name: *const u8,
		access: u32,
		share: u32,
		security: *const c_void,
		disposition: u32,
		flags: u32,
		template: Handle,
	) -> Handle;
	pub fn ReadFile(file: Handle, buf: *mut u8, len: u32, done: *mut u32, overlapped: *mut Overlapped) -> i32;
	pub fn WriteFile(file: Handle, buf: *const u8, len: u32, done: *mut u32, overlapped: *mut Overlapped) -> i32;
	pub fn CloseHandle(h: Handle) -> i32;
	pub fn CreateEventA(security: *const c_void, manual: i32, initial: i32, name: *const u8) -> Handle;
	pub fn CreateMutexA(security: *const c_void, owner: i32, name: *const u8) -> Handle;
	pub fn WaitForSingleObject(h: Handle, ms: u32) -> u32;
	pub fn GetOverlappedResult(file: Handle, overlapped: *mut Overlapped, done: *mut u32, wait: i32) -> i32;
	pub fn GlobalAlloc(flags: u32, bytes: usize) -> Handle;
	pub fn GlobalFree(h: Handle) -> Handle;
	pub fn GlobalLock(h: Handle) -> *mut u8;
	pub fn GlobalUnlock(h: Handle) -> i32;
	pub fn GlobalSize(h: Handle) -> usize;
	pub fn VirtualAlloc(address: *const c_void, bytes: usize, kind: u32, protect: u32) -> *mut u8;
	pub fn GetCommState(file: Handle, dcb: *mut Dcb) -> i32;
	pub fn SetCommState(file: Handle, dcb: *const Dcb) -> i32;
	pub fn SetCommTimeouts(file: Handle, t: *const CommTimeouts) -> i32;
	pub fn SetupComm(file: Handle, in_queue: u32, out_queue: u32) -> i32;
	pub fn PurgeComm(file: Handle, flags: u32) -> i32;
}

#[link(name = "user32", kind = "raw-dylib", import_name_type = "undecorated")]
extern "system" {
	pub fn RegisterClassA(class: *const WndClassA) -> u16;
	pub fn CreateWindowExA(
		ex: u32,
		class: *const u8,
		name: *const u8,
		style: u32,
		x: i32,
		y: i32,
		w: i32,
		h: i32,
		parent: Handle,
		menu: Handle,
		instance: Handle,
		param: *const c_void,
	) -> Handle;
	pub fn DefWindowProcA(hwnd: Handle, msg: u32, wparam: usize, lparam: isize) -> isize;
	pub fn DestroyWindow(hwnd: Handle) -> i32;
	pub fn FindWindowA(class: *const u8, title: *const u8) -> Handle;
	pub fn PeekMessageA(msg: *mut Msg, hwnd: Handle, min: u32, max: u32, remove: u32) -> i32;
	pub fn DispatchMessageA(msg: *const Msg) -> isize;
	pub fn PostQuitMessage(code: i32);
	pub fn PostMessageA(hwnd: Handle, msg: u32, wparam: usize, lparam: isize) -> i32;
	pub fn SendMessageA(hwnd: Handle, msg: u32, wparam: usize, lparam: isize) -> isize;
	pub fn MsgWaitForMultipleObjects(count: u32, handles: *const Handle, all: i32, ms: u32, mask: u32) -> u32;
	pub fn SetClipboardViewer(hwnd: Handle) -> Handle;
	pub fn ChangeClipboardChain(hwnd: Handle, next: Handle) -> i32;
	pub fn OpenClipboard(hwnd: Handle) -> i32;
	pub fn CloseClipboard() -> i32;
	pub fn EmptyClipboard() -> i32;
	pub fn SetClipboardData(format: u32, h: Handle) -> Handle;
	pub fn GetClipboardData(format: u32) -> Handle;
	pub fn IsClipboardFormatAvailable(format: u32) -> i32;
	pub fn RegisterClipboardFormatA(name: *const u8) -> u32;
}
