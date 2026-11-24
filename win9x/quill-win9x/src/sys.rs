// The Win32 calls Quill makes: ANSI entry points only, imported by name from the DLLs (raw-dylib), nothing newer than Windows 95.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::ffi::c_void;

/// A Win32 handle.
pub type Handle = *mut c_void;

pub const WM_DESTROY: u32 = 0x0002;
pub const WM_PAINT: u32 = 0x000F;
pub const WM_KEYDOWN: u32 = 0x0100;
pub const WM_CHAR: u32 = 0x0102;
pub const WM_TIMER: u32 = 0x0113;
pub const WS_CAPTION: u32 = 0x00C0_0000;
pub const WS_SYSMENU: u32 = 0x0008_0000;
pub const WS_MINIMIZEBOX: u32 = 0x0002_0000;
pub const WS_VISIBLE: u32 = 0x1000_0000;
pub const CS_OWNDC: u32 = 0x0020;
pub const IDC_ARROW: usize = 32512;
pub const OEM_FIXED_FONT: i32 = 10;
pub const ETO_OPAQUE: u32 = 2;
pub const TA_BASELINE: u32 = 24;
pub const TRANSPARENT: i32 = 1;
pub const OPAQUE: i32 = 2;
pub const FW_NORMAL: i32 = 400;
pub const FW_BOLD: i32 = 700;
pub const WM_COMMAND: u32 = 0x0111;
pub const WM_ERASEBKGND: u32 = 0x0014;
pub const WS_OVERLAPPEDWINDOW: u32 = 0x00CF_0000;
pub const MF_STRING: u32 = 0;
pub const MF_POPUP: u32 = 0x10;
pub const VK_SHIFT: i32 = 0x10;
pub const VK_CONTROL: i32 = 0x11;
pub const GENERIC_READ: u32 = 0x8000_0000;
pub const GENERIC_WRITE: u32 = 0x4000_0000;
pub const FILE_SHARE_READ: u32 = 1;
pub const CREATE_ALWAYS: u32 = 2;
pub const OPEN_EXISTING: u32 = 3;
pub const INVALID_HANDLE_VALUE: Handle = -1isize as Handle;

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

#[repr(C)]
pub struct TextMetricA {
	pub height: i32,
	pub ascent: i32,
	pub descent: i32,
	pub internal_leading: i32,
	pub external_leading: i32,
	pub ave_char_width: i32,
	pub max_char_width: i32,
	pub weight: i32,
	pub overhang: i32,
	pub digitized_aspect_x: i32,
	pub digitized_aspect_y: i32,
	pub first_char: u8,
	pub last_char: u8,
	pub default_char: u8,
	pub break_char: u8,
	pub italic: u8,
	pub underlined: u8,
	pub struck_out: u8,
	pub pitch_and_family: u8,
	pub char_set: u8,
}

#[link(name = "kernel32", kind = "raw-dylib", import_name_type = "undecorated")]
extern "system" {
	pub fn ExitProcess(code: u32) -> !;
	pub fn GetModuleHandleA(name: *const u8) -> Handle;
	pub fn GetModuleFileNameA(module: Handle, name: *mut u8, size: u32) -> u32;
	pub fn GetCommandLineA() -> *const u8;
	pub fn GetProcessHeap() -> Handle;
	pub fn GlobalMemoryStatus(status: *mut MemoryStatus);
	pub fn GetVersion() -> u32;
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
}

#[link(name = "user32", kind = "raw-dylib", import_name_type = "undecorated")]
extern "system" {
	pub fn DialogBoxParamA(
		instance: Handle,
		template: *const u8,
		owner: Handle,
		proc_: unsafe extern "system" fn(Handle, u32, usize, isize) -> isize,
		lparam: isize,
	) -> isize;
	pub fn EndDialog(dlg: Handle, result: isize) -> i32;
	pub fn GetDlgItem(dlg: Handle, id: i32) -> Handle;
	pub fn SetDlgItemTextA(dlg: Handle, id: i32, text: *const u8) -> i32;
	pub fn GetDlgItemTextA(dlg: Handle, id: i32, text: *mut u8, max: i32) -> u32;
	pub fn SendDlgItemMessageA(dlg: Handle, id: i32, msg: u32, wparam: usize, lparam: isize) -> isize;
	pub fn CheckDlgButton(dlg: Handle, id: i32, state: u32) -> i32;
	pub fn IsDlgButtonChecked(dlg: Handle, id: i32) -> u32;
	pub fn CheckRadioButton(dlg: Handle, first: i32, last: i32, check: i32) -> i32;
	pub fn GetWindowRect(hwnd: Handle, r: *mut Rect) -> i32;
	pub fn SetWindowPos(hwnd: Handle, after: Handle, x: i32, y: i32, cx: i32, cy: i32, flags: u32) -> i32;
	pub fn MessageBoxA(hwnd: Handle, text: *const u8, caption: *const u8, flags: u32) -> i32;
	pub fn WinHelpA(hwnd: Handle, file: *const u8, command: u32, data: usize) -> i32;
	pub fn SendMessageA(hwnd: Handle, msg: u32, wparam: usize, lparam: isize) -> isize;
	pub fn MoveWindow(hwnd: Handle, x: i32, y: i32, w: i32, h: i32, repaint: i32) -> i32;
	pub fn ShowWindow(hwnd: Handle, cmd: i32) -> i32;
	pub fn SetFocus(hwnd: Handle) -> Handle;
	pub fn TrackPopupMenu(menu: Handle, flags: u32, x: i32, y: i32, reserved: i32, hwnd: Handle, rect: *const Rect) -> i32;
	pub fn ClientToScreen(hwnd: Handle, p: *mut Point) -> i32;
	pub fn DestroyMenu(menu: Handle) -> i32;
	pub fn CheckMenuItem(menu: Handle, id: u32, flags: u32) -> i32;
	pub fn GetMenu(hwnd: Handle) -> Handle;
	pub fn SetCapture(hwnd: Handle) -> Handle;
	pub fn ReleaseCapture() -> i32;
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
	pub fn TranslateMessage(msg: *const Msg) -> i32;
	pub fn DispatchMessageA(msg: *const Msg) -> isize;
	pub fn PostQuitMessage(code: i32);
	pub fn LoadCursorA(instance: Handle, name: *const u8) -> Handle;
	pub fn BeginPaint(hwnd: Handle, ps: *mut PaintStruct) -> Handle;
	pub fn EndPaint(hwnd: Handle, ps: *const PaintStruct) -> i32;
	pub fn InvalidateRect(hwnd: Handle, rect: *const Rect, erase: i32) -> i32;
	pub fn GetKeyState(vk: i32) -> i16;
	pub fn AdjustWindowRect(rect: *mut Rect, style: u32, menu: i32) -> i32;
	pub fn DestroyWindow(hwnd: Handle) -> i32;
	pub fn SetTimer(hwnd: Handle, id: usize, ms: u32, proc_: *const c_void) -> usize;
	pub fn GetDC(hwnd: Handle) -> Handle;
	pub fn LoadMenuA(instance: Handle, name: *const u8) -> Handle;
	pub fn LoadIconA(instance: Handle, name: *const u8) -> Handle;
	pub fn GetClientRect(hwnd: Handle, rect: *mut Rect) -> i32;
	pub fn FillRect(dc: Handle, rect: *const Rect, brush: Handle) -> i32;
	pub fn CreateMenu() -> Handle;
	pub fn CreatePopupMenu() -> Handle;
	pub fn AppendMenuA(menu: Handle, flags: u32, id: usize, item: *const u8) -> i32;
	pub fn SetWindowTextA(hwnd: Handle, text: *const u8) -> i32;
	pub fn ReleaseDC(hwnd: Handle, dc: Handle) -> i32;
}

/// What `GlobalMemoryStatus` fills in.
#[repr(C)]
pub struct MemoryStatus {
	pub length: u32,
	pub load: u32,
	pub total_phys: u32,
	pub avail_phys: u32,
	pub total_page: u32,
	pub avail_page: u32,
	pub total_virtual: u32,
	pub avail_virtual: u32,
}

/// What `StartDoc` is told about a print job.
#[repr(C)]
pub struct DocInfoA {
	pub size: i32,
	pub doc_name: *const u8,
	pub output: *const u8,
	pub datatype: *const u8,
	pub kind: u32,
}

/// The header of a device-independent bitmap.
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

#[link(name = "gdi32", kind = "raw-dylib", import_name_type = "undecorated")]
extern "system" {
	pub fn IntersectClipRect(dc: Handle, left: i32, top: i32, right: i32, bottom: i32) -> i32;
	pub fn Polygon(dc: Handle, points: *const Point, count: i32) -> i32;
	pub fn EnumFontFamiliesA(
		dc: Handle,
		face: *const u8,
		proc_: unsafe extern "system" fn(*const u8, *const u8, u32, isize) -> i32,
		lparam: isize,
	) -> i32;
	pub fn SaveDC(dc: Handle) -> i32;
	pub fn RestoreDC(dc: Handle, saved: i32) -> i32;
	pub fn SetMapMode(dc: Handle, mode: i32) -> i32;
	pub fn SetWindowExtEx(dc: Handle, x: i32, y: i32, old: *mut Point) -> i32;
	pub fn SetViewportExtEx(dc: Handle, x: i32, y: i32, old: *mut Point) -> i32;
	pub fn GetDeviceCaps(dc: Handle, index: i32) -> i32;
	pub fn DeleteDC(dc: Handle) -> i32;
	pub fn StartDocA(dc: Handle, info: *const DocInfoA) -> i32;
	pub fn EndDoc(dc: Handle) -> i32;
	pub fn AbortDoc(dc: Handle) -> i32;
	pub fn StartPage(dc: Handle) -> i32;
	pub fn EndPage(dc: Handle) -> i32;
	#[allow(clippy::too_many_arguments)]
	pub fn StretchDIBits(
		dc: Handle,
		x: i32,
		y: i32,
		w: i32,
		h: i32,
		src_x: i32,
		src_y: i32,
		src_w: i32,
		src_h: i32,
		bits: *const u8,
		info: *const BitmapInfoHeader,
		usage: u32,
		rop: u32,
	) -> i32;
	#[allow(clippy::too_many_arguments)]
	pub fn SetDIBitsToDevice(
		dc: Handle,
		x: i32,
		y: i32,
		w: u32,
		h: u32,
		src_x: i32,
		src_y: i32,
		start_scan: u32,
		lines: u32,
		bits: *const u8,
		info: *const BitmapInfoHeader,
		usage: u32,
	) -> i32;
	pub fn CreateFontA(
		height: i32,
		width: i32,
		escapement: i32,
		orientation: i32,
		weight: i32,
		italic: u32,
		underline: u32,
		strike: u32,
		charset: u32,
		out_precision: u32,
		clip_precision: u32,
		quality: u32,
		pitch_family: u32,
		face: *const u8,
	) -> Handle;
	pub fn DeleteObject(obj: Handle) -> i32;
	pub fn GetCharWidthA(dc: Handle, first: u32, last: u32, out: *mut i32) -> i32;
	pub fn SetTextAlign(dc: Handle, mode: u32) -> u32;
	pub fn SetBkMode(dc: Handle, mode: i32) -> i32;
	pub fn CreateSolidBrush(colour: u32) -> Handle;
	pub fn TextOutA(dc: Handle, x: i32, y: i32, text: *const u8, len: i32) -> i32;
	pub fn MoveToEx(dc: Handle, x: i32, y: i32, old: *mut Point) -> i32;
	pub fn LineTo(dc: Handle, x: i32, y: i32) -> i32;
	pub fn CreatePen(style: i32, width: i32, colour: u32) -> Handle;
	pub fn GetStockObject(index: i32) -> Handle;
	pub fn SelectObject(dc: Handle, obj: Handle) -> Handle;
	pub fn SetTextColor(dc: Handle, colour: u32) -> u32;
	pub fn SetBkColor(dc: Handle, colour: u32) -> u32;
	pub fn ExtTextOutA(
		dc: Handle,
		x: i32,
		y: i32,
		options: u32,
		rect: *const Rect,
		text: *const u8,
		len: u32,
		dx: *const i32,
	) -> i32;
	pub fn GetTextMetricsA(dc: Handle, tm: *mut TextMetricA) -> i32;
}
