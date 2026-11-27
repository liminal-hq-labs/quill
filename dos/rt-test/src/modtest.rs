// The module loader's checks (`RTTEST.EXE mod ...`): handshake, failure paths, load addresses, 200 load and free cycles.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `RTTEST.EXE mod DIR BUILD` runs every check that needs only the files `dos/tools/rt-test.sh` puts in DIR (the test modules at
//! three link bases, and damaged copies of one) and prints `ok NAME` or `FAIL NAME`, exiting 1 on any failure; BUILD is the build id
//! the modules were built with. The other commands each do one thing that ends or endangers the program, so each runs on its own:
//! `mod-find` loads from the program's own folder, `mod-nomem` runs the heap out of memory, `mod-nowiden` calls a module with CS as
//! the stub left it, `mod-panic` makes the module panic.
use alloc::format;
use alloc::vec::Vec;
use quill_modabi::*;
use rt::module::{self, Failure, Module, Options, Place, Reason};
use rt::{dos, dpmi, println};

fn ticks() -> u32 {
	// SAFETY: the BIOS tick counter is at 0040:006C, in the first megabyte the host maps at linear 0.
	unsafe { dpmi::linear_to_ptr::<u32>(0x46C).read_volatile() }
}

struct Checks {
	fails: u32,
}

impl Checks {
	fn check(&mut self, name: &str, ok: bool) {
		if ok {
			println!("ok {name}");
		} else {
			println!("FAIL {name}");
			self.fails += 1;
		}
	}
}

/// FNV-1a over `bytes`: the checksum the pictures are compared by.
fn fnv(bytes: &[u8]) -> u32 {
	bytes
		.iter()
		.fold(0x811C_9DC5u32, |h, &b| (h ^ b as u32).wrapping_mul(0x0100_0193))
}

const WIDTH: usize = 64;
const HEIGHT: usize = 32;
const STRIDE: usize = 80;

/// The picture QUILLTST pretends to decode, worked out here a second way: the byte at a row and a column.
fn expected(row: usize, column: usize) -> u8 {
	(((row * 31) ^ (column * 7)) ^ 0x5A) as u8
}

/// Opens the picture through the module's table, takes it in slices of `slice` rows, closes it, and returns the checksum of the rows if
/// the module wrote exactly the pattern; `None` if any step answered otherwise or any byte differs.
fn decode(module: &Module, slice: u16) -> Option<u32> {
	let table = module.pic()?;
	let mut buffer = alloc::vec![0xEEu8; STRIDE * HEIGHT];
	let mut open = PicOpen::default();
	// SAFETY: the table's entries were checked to lie inside the module; each call is given blocks that live through it.
	unsafe {
		if (table.open?)(1, 0, 0, &mut open as *mut PicOpen as usize as u32) != QS_OK {
			return None;
		}
		if (open.width as usize, open.height as usize, open.depth) != (WIDTH, HEIGHT, 8) {
			return None;
		}
		let mut row = 0usize;
		loop {
			let mut call = PicRows {
				dst: buffer.as_mut_ptr().add(row * STRIDE) as usize as u32,
				stride: STRIDE as u32,
				shown_width: WIDTH as u16,
				rows: slice,
				..PicRows::default()
			};
			let status = (table.rows?)(&mut call as *mut PicRows as usize as u32);
			row += call.rows as usize;
			match status {
				QS_MORE if call.done == 0 && call.rows != 0 => continue,
				QS_OK if call.done != 0 => break,
				_ => return None,
			}
		}
		if row != HEIGHT || (table.close?)() != QS_OK {
			return None;
		}
	}
	let mut pixels = Vec::new();
	for row in 0..HEIGHT {
		for column in 0..WIDTH {
			if buffer[row * STRIDE + column] != expected(row, column) {
				return None;
			}
			pixels.push(buffer[row * STRIDE + column]);
		}
		// The bytes between the picture and the end of the row are not the module's to touch.
		if buffer[row * STRIDE + WIDTH..(row + 1) * STRIDE].iter().any(|&b| b != 0xEE) {
			return None;
		}
	}
	Some(fnv(&pixels))
}

use core::sync::atomic::{AtomicU32, Ordering};

static LINES: AtomicU32 = AtomicU32::new(0);
static FRESH: AtomicU32 = AtomicU32::new(0);

/// A log sink that counts lines and the ones saying QUILLTST started with fresh `.data` and `.bss`, so a loop of 200 loads is not 400 lines.
fn count_line(text: &[u8]) {
	LINES.fetch_add(1, Ordering::Relaxed);
	if text.windows(5).any(|w| w == b"fresh") {
		FRESH.fetch_add(1, Ordering::Relaxed);
	}
}

/// Counts lines without clearing the counters.
fn quiet_keep() {
	module::set_log_sink(count_line);
}

fn quiet() {
	LINES.store(0, Ordering::Relaxed);
	FRESH.store(0, Ordering::Relaxed);
	module::set_log_sink(count_line);
}

fn read(path: &str) -> Option<Vec<u8>> {
	dos::File::open(path, dos::Mode::Read).ok()?.read_to_vec(0).ok()
}

fn options(build: u32, policy: Policy) -> Options {
	Options::new(KIND_PIC, build, policy)
}

fn status_of(result: &Result<Module, Failure>) -> Option<Status> {
	result.as_ref().err().map(|f| f.status)
}

/// The linear address a module sits at, for the log.
fn linear(module: &Module) -> u32 {
	module.base().wrapping_add(dpmi::image_base())
}

pub fn cmd_mod(dir: &str, build: u32) -> i32 {
	let mut c = Checks { fails: 0 };
	let path = |name: &str| format!("{dir}\\{name}");

	// ---- the first load, in the build's own default place -------------------------------------------------------------------
	let first = module::load_file(&path("QUILLTST.DMD"), &options(build, Policy::Release));
	let before = rt::heap::HEAP.total_bytes();
	println!("default place: arena build {}", cfg!(feature = "module-arena"));
	c.check("QUILLTST.DMD loads under the release policy", first.is_ok());
	#[cfg(feature = "module-arena")]
	c.check("an arena module needs no wider CS", !module::code_segment_widened());
	#[cfg(not(feature = "module-arena"))]
	c.check("a heap module widened CS", module::code_segment_widened());
	let checksum = match &first {
		Ok(m) => {
			println!(
				"QUILLTST at linear {:#x}, {} bytes of memory, build {:08X}, kind {}, minor {}",
				linear(m),
				m.mem_size(),
				m.info().build_id,
				m.info().kind,
				m.info().abi_minor
			);
			c.check(
				"the module said KIND_PIC, its build, and no other build",
				m.info().kind == KIND_PIC && m.info().build_id == build && !m.accepted().other_build && !m.accepted().newer_minor,
			);
			decode(m, HEIGHT as u16)
		}
		Err(f) => {
			println!("refused: {}", f.describe());
			None
		}
	};
	c.check("rows decode to the pattern in one call", checksum.is_some());
	let checksum = checksum.unwrap_or(0);
	println!("checksum {checksum:08X}");

	// ---- slices of 1, 7 and all rows, at three link bases, in two places, two instances at once ----------------------------
	if let Ok(m) = &first {
		c.check("slices of 1 row give the same checksum", decode(m, 1) == Some(checksum));
		c.check("slices of 7 rows give the same checksum", decode(m, 7) == Some(checksum));
		c.check(
			"a second picture on the same load gives the same checksum",
			decode(m, 5) == Some(checksum),
		);
	}
	for name in ["QUILLTS8.DMD", "QUILLTSC.DMD"] {
		let other = module::load_file(&path(name), &options(build, Policy::Release));
		match &other {
			Ok(m) => println!("{name} at linear {:#x}", linear(m)),
			Err(f) => println!("{name} refused: {}", f.describe()),
		}
		c.check(
			&format!("{name}, linked at another base, gives the same checksum"),
			other.as_ref().ok().and_then(|m| decode(m, 7)) == Some(checksum),
		);
		if let (Ok(a), Ok(b)) = (&first, &other) {
			c.check(
				&format!("{name} lives at another address than QUILLTST.DMD"),
				a.base() != b.base(),
			);
		}
	}
	let extended = Options {
		place: Place::Extended,
		..options(build, Policy::Release)
	};
	let conventional = Options {
		place: Place::Conventional,
		..options(build, Policy::Release)
	};
	let above = module::load_file(&path("QUILLTST.DMD"), &extended);
	let below = module::load_file(&path("QUILLTST.DMD"), &conventional);
	match (&above, &below) {
		(Ok(a), Ok(b)) => println!(
			"extended memory at linear {:#x}, conventional memory at linear {:#x}",
			linear(a),
			linear(b)
		),
		_ => println!("above {:?}, below {:?}", status_of(&above), status_of(&below)),
	}
	c.check(
		"a module above the 1 MB mark runs and gives the same checksum",
		above.as_ref().ok().and_then(|m| decode(m, 7)) == Some(checksum),
	);
	c.check(
		"a module below the 1 MB mark runs and gives the same checksum",
		below.as_ref().ok().and_then(|m| decode(m, 7)) == Some(checksum),
	);
	c.check(
		"extended memory is above 1 MB",
		above.as_ref().is_ok_and(|m| linear(m) >= 0x10_0000),
	);
	c.check(
		"conventional memory is below 1 MB",
		below.as_ref().is_ok_and(|m| linear(m) < 0x10_0000),
	);
	drop(below);
	drop(above);
	// Two instances of one module alive together: separate memory, separate statics, the same output.
	let twin_a = module::load_file(&path("QUILLTST.DMD"), &extended);
	let twin_b = module::load_file(&path("QUILLTST.DMD"), &extended);
	c.check(
		"two instances at two addresses give the same checksum",
		matches!((&twin_a, &twin_b), (Ok(a), Ok(b)) if a.base() != b.base() && decode(a, 7) == Some(checksum) && decode(b, 3) == Some(checksum)),
	);
	drop(twin_a);
	drop(twin_b);

	// ---- the handshake ------------------------------------------------------------------------------------------------------
	let release_wrong = module::load_file(&path("QUILLTST.DMD"), &options(build ^ 1, Policy::Release));
	c.check(
		"another build is refused under the release policy with VERSION",
		matches!(
			&release_wrong,
			Err(Failure {
				status: QS_VERSION,
				reason: Reason::Info
			})
		),
	);
	let development = module::load_file(&path("QUILLTST.DMD"), &options(build ^ 1, Policy::Development));
	c.check(
		"another build is accepted under the development policy, flagged",
		development
			.as_ref()
			.is_ok_and(|m| m.accepted().other_build && m.info().build_id == build),
	);
	c.check(
		"a development-policy module still decodes",
		development.as_ref().ok().and_then(|m| decode(m, 7)) == Some(checksum),
	);
	drop(development);
	let major = module::load_file(
		&path("QUILLTST.DMD"),
		&Options {
			abi_major: ABI_MAJOR + 1,
			..options(build, Policy::Release)
		},
	);
	c.check(
		"a host of another major is refused by the module with VERSION",
		matches!(
			&major,
			Err(Failure {
				status: QS_VERSION,
				reason: Reason::Start(_)
			})
		),
	);
	let major_dev = module::load_file(
		&path("QUILLTST.DMD"),
		&Options {
			abi_major: ABI_MAJOR + 1,
			..options(build, Policy::Development)
		},
	);
	c.check(
		"another major is refused under the development policy too",
		status_of(&major_dev) == Some(QS_VERSION),
	);
	let minor = module::load_file(
		&path("QUILLTST.DMD"),
		&Options {
			abi_minor: ABI_MINOR + 7,
			..options(build, Policy::Release)
		},
	);
	c.check("a host of a newer minor is accepted", minor.is_ok());
	drop(minor);
	let kind = module::load_file(
		&path("QUILLTST.DMD"),
		&Options {
			kind: KIND_DLG,
			..options(build, Policy::Release)
		},
	);
	c.check(
		"a picture module asked for as a dialog module is refused with UNSUPPORTED",
		status_of(&kind) == Some(QS_UNSUPPORTED),
	);

	// ---- the failure paths: every one a status, none a panic or a hang ------------------------------------------------------
	let missing = module::load_file(&path("NOSUCH.DMD"), &options(build, Policy::Release));
	c.check(
		"a missing file is MISSING",
		matches!(
			&missing,
			Err(Failure {
				status: QS_MISSING,
				reason: Reason::Missing
			})
		),
	);
	let nodir = module::load_file(&format!("{dir}\\NOSUCHDR\\X.DMD"), &options(build, Policy::Release));
	c.check("a missing folder is MISSING", status_of(&nodir) == Some(QS_MISSING));
	for (name, why) in [
		("T_EMPTY.DMD", "an empty file"),
		("T_HDR.DMD", "a file cut inside its header"),
		("T_HALF.DMD", "a file cut in half"),
		("T_LAST.DMD", "a file missing its last byte"),
		("T_FLIP.DMD", "a file with one bit flipped in its image"),
		("T_CRC.DMD", "a file with one bit flipped in its checksum"),
		("T_MAGIC.DMD", "a file with a bad magic"),
		("T_JUNK.DMD", "a file of noise"),
		("T_PAD.DMD", "a file with bytes added at the end"),
	] {
		let result = module::load_file(&path(name), &options(build, Policy::Release));
		if let Err(f) = &result {
			println!("{name}: {}", f.describe());
		}
		c.check(
			&format!("{why} is CORRUPT"),
			matches!(
				&result,
				Err(Failure {
					status: QS_CORRUPT,
					reason: Reason::Format(_)
				})
			),
		);
	}
	let version = module::load_file(&path("T_VER.DMD"), &options(build, Policy::Release));
	if let Err(f) = &version {
		println!("T_VER.DMD: {}", f.describe());
	}
	c.check(
		"a file of a newer format version is VERSION",
		status_of(&version) == Some(QS_VERSION),
	);
	let wrong_kind = module::load_file(&path("T_KIND.DMD"), &options(build, Policy::Release));
	if let Err(f) = &wrong_kind {
		println!("T_KIND.DMD: {}", f.describe());
	}
	c.check("a file whose kind is an application is refused", wrong_kind.is_err());
	let import = module::load_file(&path("T_IMPORT.DMD"), &options(build, Policy::Release));
	if let Err(f) = &import {
		println!("T_IMPORT.DMD: {}", f.describe());
	}
	c.check(
		"a module importing an unknown service is refused with its name",
		matches!(&import, Err(Failure { status: QS_VERSION, reason: Reason::Import(name) }) if name == "desk_lox"),
	);
	let big = module::load_file(&path("T_BIGF.DMD"), &options(build, Policy::Release));
	c.check(
		"a file over 256 KB is TOO_LARGE without being read",
		matches!(
			&big,
			Err(Failure {
				status: QS_TOO_LARGE,
				reason: Reason::TooBig(_)
			})
		),
	);

	// A bit flipped anywhere in the file is caught before the module runs: every header bit, then one bit in every other byte.
	if let Some(bytes) = read(&path("QUILLTST.DMD")) {
		let heap = rt::heap::HEAP.total_bytes();
		quiet();
		let mut tried = 0u32;
		let mut accepted = 0u32;
		let mut copy = bytes.clone();
		for at in 0..bytes.len() {
			let bits: &[u8] = if at < 96 {
				&[0, 1, 2, 3, 4, 5, 6, 7]
			} else {
				&[(at % 8) as u8]
			};
			for &bit in bits {
				copy[at] ^= 1 << bit;
				tried += 1;
				if module::load_bytes("flip", &copy, &options(build, Policy::Release)).is_ok() {
					accepted += 1;
				}
				copy[at] ^= 1 << bit;
			}
		}
		println!(
			"{tried} single-bit flips tried, {accepted} accepted, {} refusals logged",
			LINES.load(Ordering::Relaxed)
		);
		c.check("every single-bit flip is refused", tried > 0 && accepted == 0);
		c.check(
			"refusals leave the heap's total as it was",
			rt::heap::HEAP.total_bytes() == heap,
		);
		let mut accepted_prefix = 0u32;
		for len in (0..bytes.len()).step_by(3) {
			if module::load_bytes("cut", &bytes[..len], &options(build, Policy::Release)).is_ok() {
				accepted_prefix += 1;
			}
		}
		module::reset_log_sink();
		c.check("every cut of the file is refused", accepted_prefix == 0);
		c.check(
			"the original still loads",
			module::load_bytes("whole", &bytes, &options(build, Policy::Release)).is_ok(),
		);
	} else {
		c.check("QUILLTST.DMD can be read", false);
	}

	// ---- 200 loads and frees with the heap unchanged -----------------------------------------------------------------------
	drop(first);
	let mut wrong = 0u32;
	let mut heap_after_first = 0usize;
	quiet();
	let started = ticks();
	for cycle in 0..200u32 {
		match module::load_file(&path("QUILLTST.DMD"), &extended) {
			Ok(m) => {
				if decode(&m, 7) != Some(checksum) {
					wrong += 1;
				}
			}
			Err(_) => wrong += 1,
		}
		if cycle == 0 {
			heap_after_first = rt::heap::HEAP.total_bytes();
		}
	}
	let elapsed = ticks().wrapping_sub(started);
	println!(
		"200 load, decode and free cycles in {elapsed} ticks ({} ms each)",
		elapsed * 54_925 / 1000 / 200
	);
	c.check("200 load and free cycles all give the same checksum", wrong == 0);
	c.check(
		"every one of the 200 loads started with fresh data and bss",
		FRESH.load(Ordering::Relaxed) == 200,
	);
	c.check(
		"the heap's total is unchanged after the first cycle",
		rt::heap::HEAP.total_bytes() == heap_after_first && heap_after_first >= before,
	);
	let bytes = read(&path("QUILLTST.DMD")).unwrap_or_default();
	let low_before = dpmi::free_memory().map(|f| f.free_pages).unwrap_or(0);
	quiet_keep();
	let mut wrong_low = 0u32;
	for _ in 0..200u32 {
		match module::load_bytes("low", &bytes, &conventional) {
			Ok(m) => {
				if decode(&m, 7) != Some(checksum) {
					wrong_low += 1;
				}
			}
			Err(_) => wrong_low += 1,
		}
	}
	c.check("200 loads into conventional memory give the same checksum", wrong_low == 0);
	c.check(
		"every one of those 200 loads started fresh too",
		FRESH.load(Ordering::Relaxed) == 400,
	);
	module::reset_log_sink();
	c.check(
		"conventional memory is all given back",
		dpmi::free_memory().map(|f| f.free_pages).unwrap_or(1) == low_before,
	);
	println!("{} checks failed", c.fails);
	i32::from(c.fails != 0)
}

/// Loads from the program's own folder (CORE-95): the file beside RTTEST.EXE, not the one in the current folder.
pub fn cmd_find(build: u32) -> i32 {
	let mut c = Checks { fails: 0 };
	println!("program folder {}", module::program_folder().unwrap_or_default());
	let found = module::load(&options(build, Policy::Release));
	if let Err(f) = &found {
		println!("refused: {}", f.describe());
	}
	c.check(
		"QUILLPIC.DMD beside the program is found and loads",
		found.as_ref().is_ok_and(|m| decode(m, 7).is_some()),
	);
	let absent = module::load(&Options::new(KIND_DLG, build, Policy::Release));
	c.check("QUILLDLG.DMD, absent, is MISSING", status_of(&absent) == Some(QS_MISSING));
	println!("{}", module::missing_message(KIND_PIC));
	println!("{}", module::missing_message(KIND_DLG));
	c.check(
		"the picture message names the file",
		module::missing_message(KIND_PIC).contains("QUILLPIC.DMD"),
	);
	c.check("the dialog message names the file and Setup", {
		let text = module::missing_message(KIND_DLG);
		text.contains("QUILLDLG.DMD") && text.contains("Run Setup again")
	});
	i32::from(c.fails != 0)
}

/// Runs the heap out of memory and loads a module that does not fit: a status, not a panic or a hang; then gives the memory back.
pub fn cmd_nomem(dir: &str, build: u32) -> i32 {
	let mut c = Checks { fails: 0 };
	let big = read(&format!("{dir}\\QUILLBIG.DMD")).unwrap_or_default();
	let small = read(&format!("{dir}\\QUILLTST.DMD")).unwrap_or_default();
	c.check(
		"both files are read while memory is plentiful",
		!big.is_empty() && !small.is_empty(),
	);
	#[cfg_attr(feature = "module-arena", allow(unused_mut))]
	let mut blocks: Vec<dpmi::Block> = Vec::new();
	#[cfg(not(feature = "module-arena"))]
	while let Ok(free) = dpmi::free_memory() {
		let size = free.largest_block & !0xFFF;
		if size == 0 {
			break;
		}
		match dpmi::alloc_block(size) {
			Ok(block) => blocks.push(block),
			Err(_) => break,
		}
	}
	println!(
		"took {} blocks; {} bytes were the largest left",
		blocks.len(),
		dpmi::free_memory().map(|f| f.largest_block).unwrap_or(0)
	);
	let refused = module::load_bytes("big", &big, &options(build, Policy::Release));
	if let Err(f) = &refused {
		println!("QUILLBIG.DMD: {}", f.describe());
	}
	c.check(
		"a module that does not fit is TOO_LARGE with the size",
		matches!(&refused, Err(Failure { status: QS_TOO_LARGE, reason: Reason::NoMemory(n) }) if *n >= 1 << 20),
	);
	for block in blocks {
		let _ = dpmi::free_block(block.handle);
	}
	let again = module::load_bytes("small", &small, &options(build, Policy::Release));
	c.check("the small module loads again once the memory is back", again.is_ok());
	drop(again);
	let normal = module::load_bytes("big", &big, &options(build, Policy::Release));
	#[cfg(not(feature = "module-arena"))]
	c.check("QUILLBIG.DMD, a megabyte, loads once the memory is back", normal.is_ok());
	#[cfg(feature = "module-arena")]
	c.check(
		"QUILLBIG.DMD, a megabyte, still does not fit the 64 KB arena",
		status_of(&normal) == Some(QS_TOO_LARGE),
	);
	i32::from(c.fails != 0)
}

/// Calls a module with CS as the stub left it. Where the emulator enforces the limit of CS this faults (the host prints an exception
/// and DOS ends the program); where it does not, the module runs, which the line says.
pub fn cmd_nowiden(dir: &str, build: u32) -> i32 {
	let options = Options {
		widen: false,
		place: Place::Extended,
		..options(build, Policy::Release)
	};
	println!("calling a module in a heap block with CS not widened");
	match module::load_file(&format!("{dir}\\QUILLTST.DMD"), &options) {
		Ok(_) => println!("NO FAULT: this machine does not enforce the limit of CS on instruction fetch"),
		Err(f) => println!("refused: {}", f.describe()),
	}
	0
}

/// Makes the module panic through the host's path (the loader's `desk_panic`): the program must print the message and exit with 3.
pub fn cmd_panic(dir: &str, build: u32) -> i32 {
	let loaded = module::load_file(&format!("{dir}\\QUILLTST.DMD"), &options(build, Policy::Release));
	let Ok(m) = loaded else {
		println!("FAIL the module did not load");
		return 1;
	};
	println!("asking the module to panic");
	if let Some(table) = m.pic() {
		let mut open = PicOpen::default();
		// SAFETY: as `decode`; the owner 0xDEAD0001 is the test module's request to panic.
		let _ = unsafe { (table.open.unwrap_or(no_open))(0xDEAD_0001, 0, 0, &mut open as *mut PicOpen as usize as u32) };
	}
	println!("FAIL the module returned from a panic");
	1
}

unsafe extern "C" fn no_open(_owner: u32, _kind: u16, _chunks: u32, _open: u32) -> Status {
	QS_INTERNAL
}

/// Widens CS, then loads one module (the first thing `mod` does, alone, so a failure to widen shows by itself).
pub fn cmd_widen(dir: &str, build: u32) -> i32 {
	println!("widening CS");
	match module::widen_code_segment() {
		Ok(()) => println!("CS widened"),
		Err(status) => {
			println!("refused: {}", status_name(status));
			return 1;
		}
	}
	let options = Options {
		place: Place::Extended,
		..options(build, Policy::Release)
	};
	match module::load_file(&format!("{dir}\\QUILLTST.DMD"), &options) {
		Ok(m) => println!("loaded at {:#x}", linear(&m)),
		Err(f) => println!("refused: {}", f.describe()),
	}
	0
}

/// Times loading: the first (cold) load, then `LOADS` more, each from the file through the first use, at whatever speed the emulated CPU
/// has (the harness sets a 386 for this run); a tick of the BIOS counter is 54.925 ms, so the mean is over many loads.
pub fn cmd_time(dir: &str, build: u32) -> i32 {
	const LOADS: u32 = 100;
	let options = Options {
		place: Place::Extended,
		..options(build, Policy::Release)
	};
	let file = format!("{dir}\\QUILLTST.DMD");
	let t0 = ticks();
	while ticks() == t0 {}
	let start = ticks();
	let mut done = 0u32;
	for _ in 0..LOADS {
		quiet();
		if module::load_file(&file, &options).is_ok() {
			done += 1;
		}
	}
	module::reset_log_sink();
	let elapsed = ticks().wrapping_sub(start);
	println!(
		"time: {done} of {LOADS} loads (open, read, check, relocate, start, info, free) in {elapsed} ticks: {} us each",
		elapsed * 54_925 / LOADS
	);
	let bytes = read(&file).unwrap_or_default();
	let start = ticks();
	for _ in 0..LOADS {
		quiet();
		let _ = module::load_bytes("time", &bytes, &options);
	}
	module::reset_log_sink();
	let elapsed = ticks().wrapping_sub(start);
	println!(
		"time: {LOADS} loads from memory (no file) in {elapsed} ticks: {} us each",
		elapsed * 54_925 / LOADS
	);
	i32::from(done != LOADS)
}
