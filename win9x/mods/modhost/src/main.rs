// MODHOST.EXE: loads QUILLTST.DLL and its variants the way the shell loads a module, and writes one line per check to MODHOST.LOG.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The loader's test program. It expects the folder it runs from to hold `QUILLTST.DLL` and four variants of it (build.sh makes them):
//! `QUILLTSTR.DLL` linked at the program's own base, `QUILLTSTB.DLL` from another build, `QUILLTSTD.DLL` from another development
//! build, `QUILLTSTM.DLL` reporting another interface major. It exits 0 only if every check passes.

#![no_std]
#![no_main]

use core::ffi::c_void;
use core::fmt::Write;
use core::sync::atomic::{AtomicIsize, AtomicU32, Ordering::Relaxed};

use quill_modabi::*;
use quill_w32rt::module::{self, Failure, Line, Loaded};

quill_w32rt::runtime!();

#[link(name = "kernel32", kind = "raw-dylib", import_name_type = "undecorated")]
extern "system" {
	fn CreateFileA(
		name: *const u8,
		access: u32,
		share: u32,
		security: *mut c_void,
		how: u32,
		flags: u32,
		template: *mut c_void,
	) -> isize;
	fn WriteFile(file: isize, bytes: *const u8, count: u32, written: *mut u32, overlapped: *mut c_void) -> i32;
	fn CloseHandle(handle: isize) -> i32;
	fn DeleteFileA(name: *const u8) -> i32;
	fn GetModuleHandleA(name: *const u8) -> *mut c_void;
}

/// The program's build id, clean: the release policy applies to it. The test modules carry this id, or a neighbour of it.
const OWN: u32 = quill_w32rt::build_id!() & !BUILD_DIRTY;
/// The same build with the dirty bit set: the development policy.
const OWN_DEV: u32 = OWN | BUILD_DIRTY;

static LOG: AtomicIsize = AtomicIsize::new(-1);
static CHECKS: AtomicU32 = AtomicU32::new(0);
static FAILED: AtomicU32 = AtomicU32::new(0);
static mut ROWS: [u8; 64 * 64 * 3] = [0; 64 * 64 * 3];

fn say(line: &[u8]) {
	let log = LOG.load(Relaxed);
	let mut written = 0u32;
	// SAFETY: `log` is a handle this program opened (or -1, which WriteFile refuses); the buffers are as long as they say.
	unsafe {
		WriteFile(log, line.as_ptr(), line.len() as u32, &mut written, core::ptr::null_mut());
		WriteFile(log, b"\r\n".as_ptr(), 2, &mut written, core::ptr::null_mut());
	}
}

fn check(what: &str, ok: bool) {
	let mut line = Line::new();
	CHECKS.fetch_add(1, Relaxed);
	if !ok {
		FAILED.fetch_add(1, Relaxed);
	}
	let _ = write!(line, "{} {what}", if ok { "ok  " } else { "FAIL" });
	say(line.as_bytes());
}

fn note(args: core::fmt::Arguments) {
	let mut line = Line::new();
	let _ = line.write_fmt(args);
	say(line.as_bytes());
}

fn path_of(name: &[u8]) -> [u8; module::MAX_PATH] {
	let mut path = [0u8; module::MAX_PATH];
	module::beside(name, &mut path).expect("the program's own folder");
	path
}

fn write_file(name: &[u8], bytes: &[u8]) {
	let path = path_of(name);
	// SAFETY: `path` is zero-terminated; the handle is closed here.
	unsafe {
		let file = CreateFileA(
			path.as_ptr(),
			0x4000_0000,
			0,
			core::ptr::null_mut(),
			2,
			0,
			core::ptr::null_mut(),
		);
		if file != -1 {
			let mut written = 0u32;
			WriteFile(file, bytes.as_ptr(), bytes.len() as u32, &mut written, core::ptr::null_mut());
			CloseHandle(file);
		}
	}
}

fn delete_file(name: &[u8]) {
	let path = path_of(name);
	// SAFETY: `path` is zero-terminated.
	unsafe { DeleteFileA(path.as_ptr()) };
}

/// Whether the module file `name` (with its zero) is mapped in this process.
fn is_mapped(name: &[u8]) -> bool {
	// SAFETY: `name` is zero-terminated.
	unsafe { !GetModuleHandleA(name.as_ptr()).is_null() }
}

/// The base the module's image asks for: `ImageBase` in its own header, read from the mapped image.
fn preferred_base(handle: usize) -> u32 {
	// SAFETY: a mapped PE image starts with its DOS header, whose `e_lfanew` (at 0x3C) leads to the NT header; `ImageBase` is 0x34
	// bytes into it.
	unsafe {
		let image = handle as *const u8;
		let nt = core::ptr::read_unaligned(image.add(0x3C) as *const u32) as usize;
		core::ptr::read_unaligned(image.add(nt + 0x34) as *const u32)
	}
}

/// Opens, reads `width`-wide rows of and closes the picture of a loaded test module; the checksum of the rows, or `None` if any call
/// answered anything but `QS_OK`.
fn draw(loaded: &Loaded, width: u16) -> Option<u32> {
	if (loaded.table_len as usize) < core::mem::size_of::<PicTableV1>() {
		return None;
	}
	// SAFETY: the module said the table at this flat address is `table_len` bytes of a PicTableV1, which it keeps until it is freed.
	unsafe {
		let table = &*(loaded.table as usize as *const PicTableV1);
		if table.size as usize != core::mem::size_of::<PicTableV1>() {
			return None;
		}
		let (open, rows, close) = (table.open?, table.rows?, table.close?);
		let mut answer = PicOpen::default();
		if open(1, 1, 0, &mut answer as *mut PicOpen as usize as u32) != QS_OK
			|| (answer.width, answer.height, answer.depth) != (64, 64, 24)
		{
			return None;
		}
		let buffer = &mut *core::ptr::addr_of_mut!(ROWS);
		buffer.fill(0);
		let mut call = PicRows {
			dst: buffer.as_mut_ptr() as usize as u32,
			stride: 64 * 3,
			shown_width: width,
			rows: 64,
			done: 0,
			pad: 0,
			palette: 0,
		};
		let status = rows(&mut call as *mut PicRows as usize as u32);
		close();
		if status != QS_OK || call.done != 1 || call.rows != 64 {
			return None;
		}
		Some(
			buffer
				.iter()
				.fold(0u32, |sum, byte| sum.wrapping_mul(31).wrapping_add(u32::from(*byte))),
		)
	}
}

fn load(name: &[u8], own: u32) -> Result<Loaded, Failure> {
	let result = module::load_file(name, KIND_PIC, own);
	let shown = core::str::from_utf8(name).unwrap_or("?");
	say(module::log_line(shown, &result, own).as_bytes());
	result
}

fn expect_refused(name: &[u8], own: u32, status: Status, what: &str) -> Option<Failure> {
	match load(name, own) {
		Err(failure) => {
			check(what, failure.status == status);
			Some(failure)
		}
		Ok(loaded) => {
			module::free(loaded);
			check(what, false);
			None
		}
	}
}

fn run() {
	// The status a Windows error becomes.
	check(
		"errors 2, 3 and 126 are a missing file",
		[2, 3, 126].iter().all(|e| module::status_for_error(*e) == QS_MISSING),
	);
	check(
		"errors 8 and 14 are not enough memory",
		[8, 14].iter().all(|e| module::status_for_error(*e) == QS_NOMEMORY),
	);
	check(
		"error 193 and any other are a version refusal",
		[193, 5, 127, 1114].iter().all(|e| module::status_for_error(*e) == QS_VERSION),
	);
	check("a policy follows the dirty bit", {
		quill_w32rt::policy_for(OWN) == Policy::Release && quill_w32rt::policy_for(OWN_DEV) == Policy::Development
	});

	// The kind-named path: QUILLPIC.DLL is not in this folder, so the file is missing and no box appears.
	match module::load(KIND_PIC, OWN) {
		Err(failure) => check(
			"load(KIND_PIC) finds no QUILLPIC.DLL here: QS_MISSING",
			failure.status == QS_MISSING && failure.os_error != 0,
		),
		Ok(loaded) => {
			module::free(loaded);
			check("load(KIND_PIC) finds no QUILLPIC.DLL here: QS_MISSING", false);
		}
	}
	match module::load(0, OWN) {
		Err(failure) => check(
			"load of a kind with no file name: QS_UNSUPPORTED",
			failure.status == QS_UNSUPPORTED,
		),
		Ok(loaded) => {
			module::free(loaded);
			check("load of a kind with no file name: QS_UNSUPPORTED", false);
		}
	}
	expect_refused(b"NOPE.DLL", OWN, QS_MISSING, "a missing file is QS_MISSING");

	// The version handshake: accepted, and the picture it draws.
	let mut reference = 0u32;
	match load(b"QUILLTST.DLL", OWN) {
		Ok(loaded) => {
			let table_ok = loaded.table != 0 && loaded.table_len as usize == core::mem::size_of::<PicTableV1>();
			check("QUILLTST.DLL is accepted in a release build", true);
			check("its table is a PicTableV1 of the size the module said", table_ok);
			check("it reports the picture kind, the program's build and no newer minor", {
				loaded.info.kind == KIND_PIC
					&& loaded.info.build_id == OWN
					&& !loaded.accepted.other_build
					&& !loaded.accepted.newer_minor
			});
			if let Some(sum) = draw(&loaded, 64) {
				reference = sum;
			}
			check("it draws 64 rows", reference != 0);
			let narrow = draw(&loaded, 30);
			check(
				"a narrower width draws another picture",
				narrow.is_some() && narrow != Some(reference),
			);
			module::free(loaded);
			check("FreeLibrary unmaps it", !is_mapped(b"QUILLTST.DLL\0"));
		}
		Err(_) => check("QUILLTST.DLL is accepted in a release build", false),
	}

	// 200 load, draw and free cycles with one checksum.
	let mut cycles = 0;
	let mut same = true;
	let mut addresses_differ = false;
	let mut first_address = 0usize;
	for _ in 0..200 {
		let Ok(loaded) = module::load_file(b"QUILLTST.DLL", KIND_PIC, OWN) else {
			break;
		};
		if cycles == 0 {
			first_address = loaded.handle;
		} else if loaded.handle != first_address {
			addresses_differ = true;
		}
		same &= draw(&loaded, 64) == Some(reference);
		module::free(loaded);
		cycles += 1;
	}
	check("200 load, draw and free cycles", cycles == 200);
	check("every cycle draws the same checksum", same && reference != 0);
	note(format_args!(
		"note: the load address was {} across the cycles ({first_address:08x})",
		if addresses_differ { "not the same" } else { "the same" }
	));
	check("the module is gone after the cycles", !is_mapped(b"QUILLTST.DLL\0"));

	// A relocated load: the copy linked at the program's own base must land elsewhere, and draw the same.
	match load(b"QUILLTSTR.DLL", OWN) {
		Ok(loaded) => {
			let preferred = preferred_base(loaded.handle);
			note(format_args!(
				"note: QUILLTSTR.DLL asks for {preferred:08x} and loaded at {:08x}",
				loaded.handle
			));
			check("the relocated copy asks for the program's own base", preferred == 0x0040_0000);
			check(
				"it loaded elsewhere, so its relocations were applied",
				loaded.handle != preferred as usize,
			);
			check("and it draws the same picture", draw(&loaded, 64) == Some(reference));
			module::free(loaded);
		}
		Err(_) => check("the relocated copy is accepted", false),
	}

	// Another build: refused in a release build and freed; in a development build accepted with a log line.
	if let Some(failure) = expect_refused(
		b"QUILLTSTB.DLL",
		OWN,
		QS_VERSION,
		"another build is QS_VERSION in a release build",
	) {
		check(
			"the refusal names the module's build id",
			failure.info.map(|i| i.build_id) == Some(OWN ^ 1),
		);
	}
	check("a refused module is freed at once", !is_mapped(b"QUILLTSTB.DLL\0"));
	match load(b"QUILLTSTD.DLL", OWN_DEV) {
		Ok(loaded) => {
			check(
				"another development build is accepted in a development build",
				loaded.accepted.other_build,
			);
			let line = module::log_line("QUILLTSTD.DLL", &Ok(loaded), OWN_DEV);
			check(
				"the log line says it is another build",
				line.as_bytes().windows(11).any(|w| w == b"other build"),
			);
			check("and it draws", draw(&loaded, 64) == Some(reference));
			module::free(loaded);
		}
		Err(_) => check("another development build is accepted in a development build", false),
	}
	// A clean module refuses a development host: the module's own check, from its side.
	expect_refused(
		b"QUILLTST.DLL",
		OWN_DEV,
		QS_VERSION,
		"a release module refuses a development host's build id",
	);
	check("and is freed", !is_mapped(b"QUILLTST.DLL\0"));

	// Another interface major.
	if let Some(failure) = expect_refused(b"QUILLTSTM.DLL", OWN, QS_VERSION, "another interface major is QS_VERSION") {
		check(
			"the refusal carries the module's major",
			failure.info.map(|i| i.abi_major) == Some(ABI_MAJOR + 1),
		);
	}
	check("and the module is freed", !is_mapped(b"QUILLTSTM.DLL\0"));

	// Wrong files: no system box, a status, the Windows error kept.
	write_file(
		b"NOTPE.DLL",
		b"this is not a program, only some text that a person left in the folder\r\n",
	);
	let failure = expect_refused(b"NOTPE.DLL", OWN, QS_VERSION, "a file that is not a PE image is QS_VERSION");
	check(
		"with a Windows error to log",
		failure.map(|f| f.os_error != 0).unwrap_or(false),
	);
	delete_file(b"NOTPE.DLL");
	// An MZ header whose `e_lfanew` leads to `NE`: a Windows 3.x module.
	let mut ne = [0u8; 128];
	ne[0] = b'M';
	ne[1] = b'Z';
	ne[0x3C] = 0x40;
	ne[0x40] = b'N';
	ne[0x41] = b'E';
	write_file(b"WIN16.DLL", &ne);
	let failure = expect_refused(b"WIN16.DLL", OWN, QS_VERSION, "a Windows 3.x (NE) module is QS_VERSION");
	check(
		"with a Windows error to log",
		failure.map(|f| f.os_error != 0).unwrap_or(false),
	);
	delete_file(b"WIN16.DLL");

	// The module's own checks of what a caller hands it.
	exports();
}

type Call = unsafe extern "C" fn(u32) -> Status;

fn exports() {
	let Ok(loaded) = module::load_file(b"QUILLTST.DLL", KIND_PIC, OWN) else {
		check("QUILLTST.DLL loads for the export checks", false);
		return;
	};
	// SAFETY: the exports have the contract's signatures; the structs handed in are as large as they say, or null.
	unsafe {
		let info = module::symbol(loaded.handle, "QuillModuleInfo").map(|a| core::mem::transmute::<usize, Call>(a));
		let start = module::symbol(loaded.handle, "QuillModuleStart").map(|a| core::mem::transmute::<usize, Call>(a));
		let (Some(info), Some(start)) = (info, start) else {
			check("both exports are found by name", false);
			return;
		};
		check("both exports are found by name", true);
		check("QuillModuleInfo(null) is QS_CORRUPT", info(0) == QS_CORRUPT);
		check("QuillModuleStart(null) is QS_CORRUPT", start(0) == QS_CORRUPT);
		// A four-byte struct that says it is four bytes: only its size is read.
		let mut short = [4u32];
		let at = short.as_mut_ptr() as usize as u32;
		check("QuillModuleInfo with a short struct is QS_CORRUPT", info(at) == QS_CORRUPT);
		check("QuillModuleStart with a short struct is QS_CORRUPT", start(at) == QS_CORRUPT);
		check("and the short struct was not written to", short[0] == 4);
		let host = ModStart {
			size: core::mem::size_of::<ModStart>() as u16,
			abi_major: ABI_MAJOR,
			abi_minor: ABI_MINOR,
			kind: KIND_PIC,
			build_id: OWN,
			table: 0,
			table_len: 0,
		};
		let try_start = |edit: &dyn Fn(&mut ModStart)| {
			let mut call = host;
			edit(&mut call);
			start(&mut call as *mut ModStart as usize as u32)
		};
		check(
			"a start for another kind is QS_UNSUPPORTED",
			try_start(&|s| s.kind = KIND_DLG) == QS_UNSUPPORTED,
		);
		check(
			"a start from another major is QS_VERSION",
			try_start(&|s| s.abi_major = ABI_MAJOR + 1) == QS_VERSION,
		);
		check(
			"a start from another build is QS_VERSION in a release module",
			try_start(&|s| s.build_id = OWN ^ 2) == QS_VERSION,
		);
		check(
			"a start with a size below ModStart is QS_CORRUPT",
			try_start(&|s| s.size = 19) == QS_CORRUPT,
		);
		check("a start that is right is QS_OK", try_start(&|_| {}) == QS_OK);
		// The table's own entries refuse null and out-of-order calls.
		let table = &*(loaded.table as usize as *const PicTableV1);
		if let (Some(open), Some(rows), Some(close)) = (table.open, table.rows, table.close) {
			check("rows before open is QS_CORRUPT", {
				let mut call = PicRows {
					dst: 0,
					stride: 0,
					shown_width: 0,
					rows: 0,
					done: 0,
					pad: 0,
					palette: 0,
				};
				rows(&mut call as *mut PicRows as usize as u32) == QS_CORRUPT
			});
			check("open(null answer) is QS_CORRUPT", open(1, 1, 0, 0) == QS_CORRUPT);
			let mut answer = PicOpen::default();
			let answer_at = &mut answer as *mut PicOpen as usize as u32;
			check("open is QS_OK", open(1, 1, 0, answer_at) == QS_OK);
			check("a second open is QS_BUSY", open(1, 1, 0, answer_at) == QS_BUSY);
			check("rows(null) is QS_CORRUPT", rows(0) == QS_CORRUPT);
			check("close is QS_OK", close() == QS_OK);
		} else {
			check("the table has its entries", false);
		}
	}
	module::free(loaded);
}

/// The program's entry point.
///
/// # Safety
/// Called once by the loader, as the image's entry.
#[no_mangle]
pub unsafe extern "C" fn start() -> ! {
	let path = path_of(b"MODHOST.LOG");
	LOG.store(
		CreateFileA(
			path.as_ptr(),
			0x4000_0000,
			1,
			core::ptr::null_mut(),
			2,
			0,
			core::ptr::null_mut(),
		),
		Relaxed,
	);
	say(b"MODHOST: the Win32 module loader");
	run();
	let mut line = Line::new();
	let (checks, failed) = (CHECKS.load(Relaxed), FAILED.load(Relaxed));
	let _ = write!(line, "{checks} checks, {failed} failed");
	say(line.as_bytes());
	CloseHandle(LOG.load(Relaxed));
	quill_w32rt::sys::ExitProcess(if failed == 0 && checks > 0 { 0 } else { 1 })
}
