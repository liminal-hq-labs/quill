// The Win32 calls the probe makes: ANSI entry points only, imported by name (raw-dylib), nothing newer than Windows 95.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::ffi::c_void;

pub type Handle = *mut c_void;

pub const WM_DESTROY: u32 = 0x0002;
pub const WM_DRAWCLIPBOARD: u32 = 0x0308;
pub const WM_CHANGECBCHAIN: u32 = 0x030D;
pub const WM_TIMER: u32 = 0x0113;
pub const WS_POPUP: u32 = 0x8000_0000;
pub const GENERIC_READ: u32 = 0x8000_0000;
pub const GENERIC_WRITE: u32 = 0x4000_0000;
pub const OPEN_EXISTING: u32 = 3;
pub const OPEN_ALWAYS: u32 = 4;
pub const FILE_END: u32 = 2;
pub const FILE_FLAG_OVERLAPPED: u32 = 0x4000_0000;
pub const INVALID_HANDLE_VALUE: Handle = -1isize as Handle;
pub const GMEM_MOVEABLE: u32 = 2;
pub const GMEM_DDESHARE: u32 = 0x2000;
pub const CF_TEXT: u32 = 1;
pub const CF_BITMAP: u32 = 2;
pub const CF_OEMTEXT: u32 = 7;
pub const CF_DIB: u32 = 8;
pub const CF_UNICODETEXT: u32 = 13;
pub const EV_RXCHAR: u32 = 1;
pub const ERROR_IO_PENDING: u32 = 997;
pub const WAIT_OBJECT_0: u32 = 0;
pub const MAXDWORD: u32 = 0xFFFF_FFFF;
pub const PURGE_ALL: u32 = 0x0F;

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

#[repr(C)]
pub struct ComStat {
	pub flags: u32,
	pub in_queue: u32,
	pub out_queue: u32,
}

#[repr(C)]
pub struct SystemTime {
	pub year: u16,
	pub month: u16,
	pub day_of_week: u16,
	pub day: u16,
	pub hour: u16,
	pub minute: u16,
	pub second: u16,
	pub millis: u16,
}

/// A `DEVMODEA`, as bytes: 156 of them, `dmSize` at 36, `dmFields` at 40, `dmBitsPerPel` at 104, `dmPelsWidth` at 108, `dmPelsHeight` at 112.
#[repr(C, align(4))]
pub struct DevMode(pub [u8; 156]);

#[repr(C)]
pub struct BitmapInfoHeader {
	pub size: u32,
	pub width: i32,
	pub height: i32,
	pub planes: u16,
	pub bit_count: u16,
	pub compression: u32,
	pub size_image: u32,
	pub x_pels: i32,
	pub y_pels: i32,
	pub clr_used: u32,
	pub clr_important: u32,
}

#[link(name = "kernel32", kind = "raw-dylib", import_name_type = "undecorated")]
extern "system" {
	pub fn ExitProcess(code: u32) -> !;
	pub fn GetModuleHandleA(name: *const u8) -> Handle;
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
	pub fn SetFilePointer(file: Handle, distance: i32, high: *mut i32, method: u32) -> u32;
	pub fn CloseHandle(h: Handle) -> i32;
	pub fn CreateEventA(security: *const c_void, manual: i32, initial: i32, name: *const u8) -> Handle;
	pub fn ResetEvent(h: Handle) -> i32;
	pub fn WaitForSingleObject(h: Handle, ms: u32) -> u32;
	pub fn GetOverlappedResult(file: Handle, overlapped: *mut Overlapped, done: *mut u32, wait: i32) -> i32;
	pub fn GlobalAlloc(flags: u32, bytes: usize) -> Handle;
	pub fn GlobalLock(h: Handle) -> *mut u8;
	pub fn GlobalUnlock(h: Handle) -> i32;
	pub fn GlobalSize(h: Handle) -> usize;
	pub fn GetCommState(file: Handle, dcb: *mut Dcb) -> i32;
	pub fn SetCommState(file: Handle, dcb: *const Dcb) -> i32;
	pub fn SetCommTimeouts(file: Handle, t: *const CommTimeouts) -> i32;
	pub fn SetCommMask(file: Handle, mask: u32) -> i32;
	pub fn WaitCommEvent(file: Handle, mask: *mut u32, overlapped: *mut Overlapped) -> i32;
	pub fn SetupComm(file: Handle, in_queue: u32, out_queue: u32) -> i32;
	pub fn PurgeComm(file: Handle, flags: u32) -> i32;
	pub fn ClearCommError(file: Handle, errors: *mut u32, stat: *mut ComStat) -> i32;
	pub fn GetLocalTime(t: *mut SystemTime);
	pub fn SetLocalTime(t: *const SystemTime) -> i32;
	pub fn SetSystemTime(t: *const SystemTime) -> i32;
	pub fn GetVersion() -> u32;
	pub fn GetCurrentProcess() -> Handle;
	pub fn GetTimeZoneInformation(info: *mut [u8; 172]) -> u32;
	pub fn SetTimeZoneInformation(info: *const [u8; 172]) -> i32;
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
	pub fn GetMessageA(msg: *mut Msg, hwnd: Handle, min: u32, max: u32) -> i32;
	pub fn TranslateMessage(msg: *const Msg) -> i32;
	pub fn DispatchMessageA(msg: *const Msg) -> isize;
	pub fn PostQuitMessage(code: i32);
	pub fn SendMessageA(hwnd: Handle, msg: u32, wparam: usize, lparam: isize) -> isize;
	pub fn SetTimer(hwnd: Handle, id: usize, ms: u32, proc_: *const c_void) -> usize;
	pub fn SetClipboardViewer(hwnd: Handle) -> Handle;
	pub fn ChangeClipboardChain(hwnd: Handle, next: Handle) -> i32;
	pub fn OpenClipboard(hwnd: Handle) -> i32;
	pub fn CloseClipboard() -> i32;
	pub fn EmptyClipboard() -> i32;
	pub fn SetClipboardData(format: u32, h: Handle) -> Handle;
	pub fn GetClipboardData(format: u32) -> Handle;
	pub fn EnumClipboardFormats(format: u32) -> u32;
	pub fn GetClipboardFormatNameA(format: u32, buf: *mut u8, max: i32) -> i32;
	pub fn RegisterClipboardFormatA(name: *const u8) -> u32;
	pub fn GetClipboardOwner() -> Handle;
	pub fn GetClipboardSequenceNumber() -> u32;
	pub fn IsClipboardFormatAvailable(format: u32) -> i32;
	pub fn EnumDisplaySettingsA(device: *const u8, mode: u32, dev: *mut DevMode) -> i32;
	pub fn ChangeDisplaySettingsA(dev: *const DevMode, flags: u32) -> i32;
	pub fn GetSystemMetrics(index: i32) -> i32;
	pub fn GetDC(hwnd: Handle) -> Handle;
	pub fn ReleaseDC(hwnd: Handle, dc: Handle) -> i32;
}

#[link(name = "gdi32", kind = "raw-dylib", import_name_type = "undecorated")]
extern "system" {
	pub fn CreateCompatibleDC(dc: Handle) -> Handle;
	pub fn CreateCompatibleBitmap(dc: Handle, w: i32, h: i32) -> Handle;
	pub fn SelectObject(dc: Handle, obj: Handle) -> Handle;
	pub fn PatBlt(dc: Handle, x: i32, y: i32, w: i32, h: i32, rop: u32) -> i32;
	pub fn DeleteDC(dc: Handle) -> i32;
}

#[repr(C)]
pub struct TokenPrivileges {
	pub count: u32,
	pub luid_low: u32,
	pub luid_high: i32,
	pub attributes: u32,
}

// Not on Windows 95 (the helper would look these up with GetProcAddress); the spike runs on ReactOS only.
#[link(name = "advapi32", kind = "raw-dylib", import_name_type = "undecorated")]
extern "system" {
	pub fn OpenProcessToken(process: Handle, access: u32, token: *mut Handle) -> i32;
	pub fn LookupPrivilegeValueA(system: *const u8, name: *const u8, luid: *mut [u32; 2]) -> i32;
	pub fn AdjustTokenPrivileges(
		token: Handle,
		disable_all: i32,
		new_state: *const TokenPrivileges,
		length: u32,
		previous: *mut TokenPrivileges,
		returned: *mut u32,
	) -> i32;
}
