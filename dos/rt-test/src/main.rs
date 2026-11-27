// RTTEST.EXE: exercises the runtime's directory, drive, rename and critical-error calls.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `RTTEST.EXE list DIR` prints the tree under DIR, depth first, each folder's entries sorted by name, one `PATH <DIR>` or
//! `PATH SIZE` line each. `RTTEST.EXE self` checks drives, rename, the critical-error stub and the invalid-drive answer, printing
//! `ok NAME` or `FAIL NAME` and exiting 1 on any failure. `RTTEST.EXE time DIR` times directory reads with the BIOS tick
//! counter. `RTTEST.EXE probe DIR` prints what DOS says about the odd cases (attributes, dot entries, drives) as facts.
//! `RTTEST.EXE mod DIR BUILD` and its relatives check the module loader (`modtest.rs`).
#![no_std]
#![no_main]

extern crate alloc;

mod modtest;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use rt::dos::{self, DirEntry, DosError, File, Mode};
use rt::{dpmi, env, println};

rt::entry!(main);

/// Reads the folder `dir` (no trailing backslash) with attribute mask 10h and returns its entries without `.` and `..`, sorted.
fn read_dir(dir: &str, attr: u8) -> Result<Vec<DirEntry>, DosError> {
	let mut v = Vec::new();
	dos::find_each(&format!("{dir}\\*.*"), attr, |e| {
		if e.name_bytes() != b"." && e.name_bytes() != b".." {
			v.push(*e);
		}
	})?;
	v.sort_by(|a, b| a.name_bytes().cmp(b.name_bytes()));
	Ok(v)
}

fn name(e: &DirEntry) -> String {
	String::from_utf8_lossy(e.name_bytes()).into_owned()
}

fn walk(dir: &str, out: &mut Vec<String>) -> Result<(), DosError> {
	for e in read_dir(dir, dos::ATTR_DIRECTORY)? {
		let path = format!("{dir}\\{}", name(&e));
		if e.is_dir() {
			out.push(format!("{path} <DIR>"));
			walk(&path, out)?;
		} else {
			out.push(format!("{path} {}", e.size));
		}
	}
	Ok(())
}

fn ticks() -> u32 {
	// SAFETY: the BIOS tick counter is at 0040:006C, in the first megabyte the host maps at linear 0.
	unsafe { dpmi::linear_to_ptr::<u32>(0x46C).read_volatile() }
}

/// Runs `f` until at least 40 ticks (about 2.2 s of emulated time) have passed, and prints the mean per run (a tick is 54.925 ms).
fn timed(label: &str, mut f: impl FnMut() -> usize) {
	let mut entries = 0;
	// Start on a tick edge so the first interval is not short.
	let t0 = ticks();
	while ticks() == t0 {}
	let start = ticks();
	let mut reps = 0u32;
	while ticks().wrapping_sub(start) < 40 {
		entries = f();
		reps += 1;
	}
	let t = ticks().wrapping_sub(start);
	let us = t * 54_925 / reps;
	println!(
		"time {label}: {entries} entries, {reps} reads in {t} ticks, {} us per read ({} us per entry)",
		us,
		us / entries.max(1) as u32
	);
}

fn cmd_time(root: &str) -> i32 {
	println!("CPU can toggle AC (486 or later): {}", has_ac_flag());
	let big = format!("{root}\\BIG\\*.*");
	let f512 = format!("{root}\\F512\\*.*");
	let mut deep = String::from(root);
	for i in 1..=12 {
		deep.push_str(&format!("\\L{i}"));
	}
	let deep_pat = format!("{deep}\\*.*");
	timed("find 600 files", || {
		dos::find_each(&big, dos::ATTR_DIRECTORY, |_| {}).unwrap_or(0)
	});
	timed("find 512 files", || {
		dos::find_each(&f512, dos::ATTR_DIRECTORY, |_| {}).unwrap_or(0)
	});
	timed("find 3 entries 12 folders deep", || {
		dos::find_each(&deep_pat, dos::ATTR_DIRECTORY, |_| {}).unwrap_or(0)
	});
	timed("walk the whole tree", || {
		let mut v = Vec::new();
		let _ = walk(root, &mut v);
		v.len()
	});
	timed("drive_mask and current_dir (26 drive checks)", || {
		let _ = dos::current_dir(2);
		dos::drive_mask();
		1
	});
	0
}

fn check(name: &str, ok: bool, fails: &mut u32) {
	if ok {
		println!("ok {name}");
	} else {
		println!("FAIL {name}");
		*fails += 1;
	}
}

fn cmd_self() -> i32 {
	let mut fails = 0;
	let mask = dos::drive_mask();
	println!("drive mask {mask:#010x}");
	check(
		"drives C, D and Z are valid",
		mask & (1 << 2) != 0 && mask & (1 << 3) != 0 && mask & (1 << 25) != 0,
		&mut fails,
	);
	check(
		"E: is not a valid drive",
		!dos::drive_valid(4) && mask & (1 << 4) == 0,
		&mut fails,
	);
	check(
		"invalid drive gives error 0Fh",
		dos::drive_attributes(4) == Err(DosError(0x0F)),
		&mut fails,
	);
	check(
		"free_space takes A: as 0: C: answers and E: is refused with 0Fh",
		dos::free_space(2).is_ok() && dos::free_space(4) == Err(DosError(0x0F)),
		&mut fails,
	);
	check(
		"find on an invalid drive fails without hanging",
		dos::find_first("E:\\*.*", 0x10).is_err(),
		&mut fails,
	);
	check(
		"open on an invalid drive fails without hanging",
		File::open("E:\\X.TXT", Mode::Read).is_err(),
		&mut fails,
	);
	check("the current drive is C:", dos::current_drive() == 2, &mut fails);
	check(
		"the current directory of C: is the root",
		dos::current_dir(2).as_deref() == Ok(""),
		&mut fails,
	);
	let (major, _) = dos::dos_version();
	check("DOS version is at least 5", major >= 5, &mut fails);
	check(
		"DPMI reports free memory",
		dpmi::free_memory().map(|m| m.largest_block > 0).unwrap_or(false),
		&mut fails,
	);

	// Rename on the scratch drive.
	let _ = dos::remove("D:\\R1.TXT");
	let _ = dos::remove("D:\\R2.TXT");
	let made = File::open("D:\\R1.TXT", Mode::Create)
		.and_then(|mut f| f.write_all(b"hello"))
		.is_ok();
	check("create D:\\R1.TXT", made, &mut fails);
	check("rename R1 to R2", dos::rename("D:\\R1.TXT", "D:\\R2.TXT").is_ok(), &mut fails);
	check(
		"R1 is gone (error 2)",
		File::open("D:\\R1.TXT", Mode::Read).err() == Some(DosError(2)),
		&mut fails,
	);
	let mut buf = [0u8; 8];
	let n = File::open("D:\\R2.TXT", Mode::Read)
		.and_then(|mut f| f.read(&mut buf))
		.unwrap_or(0);
	check("R2 holds the bytes", &buf[..n] == b"hello", &mut fails);
	let _ = File::open("D:\\R3.TXT", Mode::Create).map(drop);
	check(
		"rename onto an existing name fails",
		dos::rename("D:\\R2.TXT", "D:\\R3.TXT").is_err(),
		&mut fails,
	);
	check(
		"rename of a missing file gives error 2",
		dos::rename("D:\\NOPE.TXT", "D:\\NOPE2.TXT") == Err(DosError(2)),
		&mut fails,
	);
	let _ = dos::mkdir("D:\\SUB");
	check(
		"rename moves a file into a folder",
		dos::rename("D:\\R2.TXT", "D:\\SUB\\R2.TXT").is_ok(),
		&mut fails,
	);
	check(
		"the moved file is found by name",
		dos::find_first("D:\\SUB\\R2.TXT", 0).map(|e| e.size == 5).unwrap_or(false),
		&mut fails,
	);
	let _ = dos::remove("D:\\SUB\\R2.TXT");
	let _ = dos::remove("D:\\R3.TXT");
	let _ = dos::rmdir("D:\\SUB");

	// INT 24h stub.
	let before = dpmi::get_rm_vector(0x24).unwrap_or((0, 0));
	check("the stub installs", dos::install_critical_handler().is_ok(), &mut fails);
	let seg = dos::critical_handler_segment().unwrap_or(0);
	check(
		"INT 24h points at the stub",
		dpmi::get_rm_vector(0x24) == Ok((seg, 0)) && seg != 0,
		&mut fails,
	);
	let bytes = (0..3)
		.map(|i| unsafe { dpmi::linear_to_ptr::<u8>(u32::from(seg) << 4).add(i).read() })
		.collect::<Vec<u8>>();
	check("the stub is mov al,3; iret", bytes == dos::CRITICAL_STUB, &mut fails);
	check(
		"installing twice is harmless",
		dos::install_critical_handler().is_ok() && dos::critical_handler_segment() == Some(seg),
		&mut fails,
	);
	dos::remove_critical_handler();
	check(
		"INT 24h is back as it was",
		dpmi::get_rm_vector(0x24) == Ok(before) && dos::critical_handler_segment().is_none(),
		&mut fails,
	);
	// What `exit` and the panic handler do: run the clean-up list, which must take the stub out.
	let _ = dos::install_critical_handler();
	rt::cleanup::run_all();
	check(
		"the exit list restores INT 24h",
		dpmi::get_rm_vector(0x24) == Ok(before) && dos::critical_handler_segment().is_none(),
		&mut fails,
	);
	if fails == 0 {
		println!("self: all passed");
		0
	} else {
		1
	}
}

/// Whether the CPU can set the AC flag (EFLAGS bit 18): a 486 or later can, a 386 cannot.
fn has_ac_flag() -> bool {
	let (before, after): (u32, u32);
	// SAFETY: reads EFLAGS, tries to set AC and reads it back, then restores the original; no memory is touched.
	unsafe {
		core::arch::asm!(
			"pushfd",
			"pop {b}",
			"mov {a}, {b}",
			"xor {a}, 0x40000",
			"push {a}",
			"popfd",
			"pushfd",
			"pop {a}",
			"push {b}",
			"popfd",
			b = out(reg) before,
			a = out(reg) after,
		);
	}
	(before ^ after) & 0x40000 != 0
}

fn cmd_probe(root: &str) -> i32 {
	println!("CPU can toggle AC (486 or later): {}", has_ac_flag());
	println!("dos version {:?}", dos::dos_version());
	println!("floppy count {}", dos::floppy_count());
	println!(
		"current drive {} dir {:?}",
		dos::current_drive(),
		dos::current_dir(dos::current_drive())
	);
	println!("set_current_drive(2) returns {}", dos::set_current_drive(2));
	for d in 0..26u8 {
		let letter = (b'A' + d) as char;
		println!(
			"drive {letter}: 4409h {:?}  440Eh {:?}  free {:?}  cwd {:?}",
			dos::drive_attributes(d),
			dos::logical_drive_map(d),
			dos::free_space(d).map(|b| b / 1024),
			dos::current_dir(d)
		);
	}
	println!("drive mask {:#010x}", dos::drive_mask());
	println!("free memory {:?}", dpmi::free_memory());
	// What a search returns, in the order DOS gives it, with different masks.
	for (label, attr) in [("mask 00", 0u8), ("mask 10", 0x10), ("mask 16", 0x16), ("mask 1E", 0x1E)] {
		let mut names = Vec::new();
		let r = dos::find_each(&format!("{root}\\*.*"), attr, |e| {
			names.push(format!("{}[{:02X}]", name(e), e.attr))
		});
		println!("probe {label} {r:?}: {}", names.join(" "));
	}
	for dir in ["C:\\TREE\\MIXED", "C:\\TREE\\BIG", "C:\\TREE\\F512", "C:"] {
		let mut raw = Vec::new();
		let _ = dos::find_each(&format!("{dir}\\*.*"), 0x10, |e| raw.push(name(e)));
		let mut sorted = raw.clone();
		sorted.sort();
		let head = raw.iter().take(14).cloned().collect::<Vec<_>>().join(" ");
		println!(
			"raw order of {dir}: {} entries, sorted by bytes: {}; first: {head}",
			raw.len(),
			raw == sorted
		);
	}
	println!(
		"probe a pattern: {:?}",
		dos::find_first(&format!("{root}\\*.TXT"), 0).map(|e| name(&e))
	);
	println!(
		"probe a missing folder: {:?}",
		dos::find_first(&format!("{root}\\NOPE\\*.*"), 0x10).map(|e| name(&e))
	);
	println!(
		"probe a missing file: {:?}",
		dos::find_first(&format!("{root}\\NOPE.TXT"), 0x10).map(|e| name(&e))
	);
	println!(
		"probe a file used as a folder: {:?}",
		dos::find_first(&format!("{root}\\LOWER.TXT\\*.*"), 0x10).map(|e| name(&e))
	);
	// A file made hidden is returned only with the hidden bit in the mask.
	let _ = dos::remove("D:\\HID.TXT");
	let _ = File::open("D:\\HID.TXT", Mode::Create).map(drop);
	println!("set hidden: {:?}", dos::set_attributes("D:\\HID.TXT", 0x02));
	println!("attributes now: {:?}", dos::attributes("D:\\HID.TXT"));
	for mask in [0u8, 0x02, 0x10] {
		println!(
			"find HID.TXT mask {mask:02X}: {:?}",
			dos::find_first("D:\\HID.TXT", mask).map(|e| (name(&e), e.attr))
		);
	}
	let _ = dos::set_attributes("D:\\HID.TXT", 0x20);
	let _ = dos::remove("D:\\HID.TXT");
	0
}

fn main() -> i32 {
	let args = env::args();
	let cmd = args.first().map(String::as_str).unwrap_or("");
	let dir = args.get(1).map(String::as_str).unwrap_or("C:\\TREE");
	match cmd {
		"list" => {
			let mut out = Vec::new();
			match walk(dir, &mut out) {
				Ok(()) => {
					for l in &out {
						println!("{l}");
					}
					0
				}
				Err(e) => {
					println!("error {:#x}", e.0);
					2
				}
			}
		}
		"time" => cmd_time(dir),
		"self" => cmd_self(),

		"probe" => cmd_probe(dir),
		"mod" | "mod-find" | "mod-nomem" | "mod-nowiden" | "mod-widen" | "mod-time" | "mod-panic" => {
			// The build id the modules were built with is the last word: `RTTEST mod C:\\MODS 3489044284`.
			let build = args.get(2).and_then(|text| text.parse::<u32>().ok()).unwrap_or(0);
			match cmd {
				"mod" => modtest::cmd_mod(dir, build),
				"mod-find" => modtest::cmd_find(args.get(1).and_then(|text| text.parse::<u32>().ok()).unwrap_or(0)),
				"mod-nomem" => modtest::cmd_nomem(dir, build),
				"mod-nowiden" => modtest::cmd_nowiden(dir, build),
				"mod-widen" => modtest::cmd_widen(dir, build),
				"mod-time" => modtest::cmd_time(dir, build),
				_ => modtest::cmd_panic(dir, build),
			}
		}
		_ => {
			println!("usage: RTTEST list|time|probe DIR | self | mod|mod-nomem|mod-nowiden|mod-panic DIR BUILD | mod-find BUILD");
			64
		}
	}
}
