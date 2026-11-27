// QUILLTST.DLL: a picture-kind module whose `rows` writes a gradient. It exists to test the loader, not to decode anything.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A Win32 module of kind `KIND_PIC` that fills a `PicTableV1` with four entries. `open` reports a 64 by 64 picture; `rows` writes a
//! gradient of the width the caller asks for. It allocates nothing and holds one session in its own `.bss`.

#![no_std]

use quill_modabi::*;
use quill_w32rt::dll::{plain_at, struct_at};

quill_w32rt::runtime!(dll);
quill_w32rt::dll_entry!();

/// This build's id: the one the build gives it in `QUILL_BUILD_ID`.
const BUILD: u32 = quill_w32rt::build_id!();
/// The interface major this module reports.
const MAJOR: u16 = if cfg!(feature = "wrong-major") {
	ABI_MAJOR + 1
} else {
	ABI_MAJOR
};
/// The picture's side in pixels.
const SIDE: usize = 64;

static mut OPEN: bool = false;
static mut TABLE: PicTableV1 = PicTableV1 {
	size: 0,
	info: None,
	open: None,
	rows: None,
	close: None,
};

unsafe extern "C" fn info(at: u32) -> Status {
	// The caller sets `size` before the call, so a short struct is refused before anything is written to it.
	let out = match struct_at::<ModInfo>(at) {
		Ok(out) => out,
		Err(status) => return status,
	};
	*out = ModInfo {
		size: core::mem::size_of::<ModInfo>() as u16,
		abi_major: MAJOR,
		abi_minor: ABI_MINOR,
		kind: KIND_PIC,
		build_id: BUILD,
		arena: 0,
		limit: SIDE as u32,
	};
	QS_OK
}

unsafe extern "C" fn open(_owner: u32, _kind: u16, _chunks: u32, answer: u32) -> Status {
	let out = match plain_at::<PicOpen>(answer) {
		Ok(out) => out,
		Err(status) => return status,
	};
	if OPEN {
		return QS_BUSY;
	}
	out.width = SIDE as u32;
	out.height = SIDE as u32;
	out.orientation = 1;
	out.depth = 24;
	OPEN = true;
	QS_OK
}

unsafe extern "C" fn rows(at: u32) -> Status {
	let call = match plain_at::<PicRows>(at) {
		Ok(call) => call,
		Err(status) => return status,
	};
	let width = call.shown_width as usize;
	if !OPEN || call.dst == 0 || width == 0 || (call.stride as usize) < width * 3 {
		return QS_CORRUPT;
	}
	let count = (call.rows as usize).min(SIDE);
	for row in 0..count {
		let line = (call.dst as usize + row * call.stride as usize) as *mut u8;
		let down = (row * 255 / (SIDE - 1)) as u8;
		for x in 0..width {
			// Blue across, green down, red the sum, so a wrong width or stride changes the picture.
			let across = if width > 1 { (x * 255 / (width - 1)) as u8 } else { 0 };
			let pixel = line.add(x * 3);
			*pixel = across;
			*pixel.add(1) = down;
			*pixel.add(2) = across.wrapping_add(down);
		}
	}
	call.rows = count as u16;
	call.done = 1;
	QS_OK
}

unsafe extern "C" fn close() -> Status {
	OPEN = false;
	QS_OK
}

/// `QuillModuleInfo(*ModInfo) -> Status`.
///
/// # Safety
/// `at` is null, or the flat address of a `ModInfo` the caller owns, whose leading `size` says how long it is.
#[no_mangle]
pub unsafe extern "C" fn QuillModuleInfo(at: u32) -> Status {
	info(at)
}

/// `QuillModuleStart(*ModStart) -> Status`: checks the host from this side, then fills the picture table.
///
/// # Safety
/// `at` is null, or the flat address of a `ModStart` the caller owns, whose leading `size` says how long it is.
#[no_mangle]
pub unsafe extern "C" fn QuillModuleStart(at: u32) -> Status {
	let start = match struct_at::<ModStart>(at) {
		Ok(start) => start,
		Err(status) => return status,
	};
	if let Err(status) = check_start(start, KIND_PIC, BUILD, quill_w32rt::policy_for(BUILD)) {
		return status;
	}
	let table = &mut *core::ptr::addr_of_mut!(TABLE);
	*table = PicTableV1 {
		size: core::mem::size_of::<PicTableV1>() as u32,
		info: Some(info),
		open: Some(open),
		rows: Some(rows),
		close: Some(close),
	};
	start.table = table as *mut PicTableV1 as usize as u32;
	start.table_len = table.size;
	QS_OK
}
