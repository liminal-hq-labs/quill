// The few KERNEL32 calls the runtime makes, imported by name (raw-dylib), all on Windows 95.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#[cfg(any(feature = "heap", feature = "loader"))]
use core::ffi::c_void;

#[link(name = "kernel32", kind = "raw-dylib", import_name_type = "undecorated")]
extern "system" {
	pub fn ExitProcess(code: u32) -> !;
}

#[cfg(feature = "heap")]
#[link(name = "kernel32", kind = "raw-dylib", import_name_type = "undecorated")]
extern "system" {
	pub fn GetProcessHeap() -> *mut c_void;
	pub fn HeapAlloc(heap: *mut c_void, flags: u32, bytes: usize) -> *mut u8;
	pub fn HeapFree(heap: *mut c_void, flags: u32, p: *mut u8) -> i32;
}

#[cfg(feature = "loader")]
#[link(name = "kernel32", kind = "raw-dylib", import_name_type = "undecorated")]
extern "system" {
	pub fn LoadLibraryA(name: *const u8) -> *mut c_void;
	pub fn FreeLibrary(module: *mut c_void) -> i32;
	pub fn GetProcAddress(module: *mut c_void, name: *const u8) -> *const c_void;
	pub fn GetModuleFileNameA(module: *mut c_void, name: *mut u8, size: u32) -> u32;
	pub fn SetErrorMode(mode: u32) -> u32;
	pub fn GetLastError() -> u32;
}
