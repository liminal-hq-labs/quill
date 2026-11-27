// QUILLTST: a picture-kind module that decodes nothing. It answers the interface of `quill-modabi` so the DOS loader's tests have a real
// module to load: a build id, a table, rows of a pattern the test can check, counters in `.data` and `.bss` that show each load starts fresh.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
#![no_std]
#![no_main]

use core::mem::size_of;
use core::sync::atomic::{AtomicU32, Ordering};

use desk_guest::log;
use quill_modabi::*;

/// The build the module says it is from: `QUILL_BUILD_ID` (decimal) when `build-mods.sh` sets it, a fixed number otherwise.
const BUILD: u32 = parse(option_env!("QUILL_BUILD_ID"));

const fn parse(text: Option<&str>) -> u32 {
	match text {
		None => 0x1A2B_3C4D,
		Some(text) => {
			let bytes = text.as_bytes();
			let mut value: u32 = 0;
			let mut i = 0;
			while i < bytes.len() {
				value = value.wrapping_mul(10).wrapping_add((bytes[i] - b'0') as u32);
				i += 1;
			}
			value
		}
	}
}

/// In `.data`: set again by every load, so a second load of the module sees 0x5A again.
static SEED: AtomicU32 = AtomicU32::new(0x5A);
/// In `.bss`: zeroed by every load, so the first `open` of a fresh load answers 1.
static OPENS: AtomicU32 = AtomicU32::new(0);
/// The size of the picture the module pretends to hold, and how far `rows` has got.
const PICTURE_WIDTH: u32 = 64;
const PICTURE_HEIGHT: u32 = 32;
static HEIGHT: AtomicU32 = AtomicU32::new(0);
static NEXT_ROW: AtomicU32 = AtomicU32::new(0);

#[cfg(feature = "big")]
#[used]
static mut BIG: [u8; 1 << 20] = [0; 1 << 20];

/// The byte of the pattern at a row and a column.
fn pattern(row: u32, column: u32, seed: u32) -> u8 {
	((row * 31) ^ (column * 7) ^ seed) as u8
}

unsafe extern "C" fn info(out: u32) -> Status {
	if out == 0 {
		return QS_CORRUPT;
	}
	#[cfg(feature = "big")]
	// SAFETY: a single-threaded write to a static the module owns; it keeps the megabyte in the image's memory size.
	unsafe {
		(&raw mut BIG).cast::<u8>().write_volatile(1)
	};
	// SAFETY: the host gives a `ModInfo`-sized block at `out` for the length of the call.
	unsafe {
		(out as usize as *mut ModInfo).write(ModInfo {
			size: size_of::<ModInfo>() as u16,
			abi_major: ABI_MAJOR,
			abi_minor: ABI_MINOR,
			kind: KIND_PIC,
			build_id: BUILD,
			arena: 0,
			limit: 4096,
		})
	};
	QS_OK
}

unsafe extern "C" fn open(owner: u32, _kind: u16, _chunks: u32, open: u32) -> Status {
	if owner == 0xDEAD_0001 {
		panic!("quilltst: asked to panic");
	}
	if open == 0 {
		return QS_CORRUPT;
	}
	// The module "decodes" a 64 by 32 picture of one byte a pixel whatever the chunks hold, and answers its header.
	// SAFETY: the host gives a `PicOpen` at `open` for the length of the call.
	unsafe {
		let picture = &mut *(open as usize as *mut PicOpen);
		picture.width = PICTURE_WIDTH;
		picture.height = PICTURE_HEIGHT;
		picture.orientation = 1;
		picture.depth = 8;
	}
	HEIGHT.store(PICTURE_HEIGHT, Ordering::Relaxed);
	NEXT_ROW.store(0, Ordering::Relaxed);
	OPENS.store(OPENS.load(Ordering::Relaxed) + 1, Ordering::Relaxed);
	QS_OK
}

unsafe extern "C" fn rows(block: u32) -> Status {
	if block == 0 {
		return QS_CORRUPT;
	}
	// SAFETY: the host gives a `PicRows` at `block`, and `rows` rows of `stride` bytes at `dst`, for the length of the call.
	unsafe {
		let call = &mut *(block as usize as *mut PicRows);
		let height = HEIGHT.load(Ordering::Relaxed);
		let first = NEXT_ROW.load(Ordering::Relaxed);
		let count = (call.rows as u32).min(height - first.min(height));
		let seed = SEED.load(Ordering::Relaxed);
		for r in 0..count {
			let row = (call.dst as usize + (r * call.stride) as usize) as *mut u8;
			for column in 0..call.shown_width as u32 {
				row.add(column as usize).write(pattern(first + r, column, seed));
			}
		}
		NEXT_ROW.store(first + count, Ordering::Relaxed);
		call.rows = count as u16;
		call.done = (first + count >= height) as u16;
		if call.done != 0 {
			QS_OK
		} else {
			QS_MORE
		}
	}
}

unsafe extern "C" fn close() -> Status {
	NEXT_ROW.store(0, Ordering::Relaxed);
	QS_OK
}

static TABLE: PicTableV1 = PicTableV1 {
	size: size_of::<PicTableV1>() as u32,
	info: Some(info),
	open: Some(open),
	rows: Some(rows),
	close: Some(close),
};

/// The host's block in, the table out. A host of another major or kind is refused; a host of another build is accepted and logged, because
/// a program in a release build checks the build itself, on the `info` this module answers.
///
/// # Safety
///
/// `start` is a writable `ModStart` of `size` bytes, as the loader passes it.
#[no_mangle]
pub unsafe extern "C" fn app_start(_app: u32, start: *mut ModStart, size: u32) -> i32 {
	if start.is_null() || (size as usize) < size_of::<ModStart>() {
		return QS_CORRUPT as i32;
	}
	// SAFETY: as documented above.
	let start = unsafe { &mut *start };
	match check_start(start, KIND_PIC, BUILD, Policy::Development) {
		Err(status) => return status as i32,
		Ok(accepted) if accepted.other_build => log("quilltst: the program is from another build"),
		Ok(_) => {}
	}
	if SEED.load(Ordering::Relaxed) == 0x5A && OPENS.load(Ordering::Relaxed) == 0 {
		log("quilltst: started fresh (data 5A, bss 0)");
	} else {
		log("quilltst: started with stale state");
	}
	start.table = &TABLE as *const PicTableV1 as usize as u32;
	start.table_len = size_of::<PicTableV1>() as u32;
	0
}

#[no_mangle]
pub extern "C" fn app_stop(_app: u32) {
	log("quilltst: stop");
}
