// INT 21h: console and file I/O, the exit call.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// CWSDPMI does not translate pointers for a protected-mode INT 21h: the call is reflected to real mode with the registers as they
// are, so DS:EDX names whatever real-mode DS and the low 16 bits of EDX happen to address (in the tests: CWSDPMI's own data).
// Every DOS call that takes a pointer therefore goes through a transfer buffer in conventional memory and the DPMI real-mode
// call (0300h), as DJGPP's C library does: copy the data into the buffer, call DOS with a real-mode segment:offset, copy the
// result out. The buffer is 32 KiB, so larger reads and writes loop. Calls that take no pointer (AH=4Ch, AH=3Eh) are made
// directly.
use crate::dpmi::{self, RealRegs};
use alloc::vec::Vec;
use core::arch::asm;
use core::sync::atomic::{AtomicU32, Ordering};

/// A DOS error code (the AX returned with the carry flag set).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DosError(pub u16);

/// The size of the transfer buffer in paragraphs (32 KiB).
const TB_PARAGRAPHS: u16 = 0x800;
pub const TB_SIZE: usize = TB_PARAGRAPHS as usize * 16;

/// Real-mode segment of the transfer buffer, 0 until first use.
static TB_SEGMENT: AtomicU32 = AtomicU32::new(0);

fn tb_segment() -> u16 {
	let seg = TB_SEGMENT.load(Ordering::Relaxed);
	if seg != 0 {
		return seg as u16;
	}
	match dpmi::alloc_dos_memory(TB_PARAGRAPHS) {
		Ok((segment, _selector)) => {
			TB_SEGMENT.store(segment as u32, Ordering::Relaxed);
			segment
		}
		Err(_) => die(b"dos: no conventional memory for the transfer buffer\r\n"),
	}
}

/// The transfer buffer as a pointer in the flat data segment (conventional memory is mapped at linear 0).
fn tb_ptr() -> *mut u8 {
	dpmi::linear_to_ptr::<u8>((tb_segment() as u32) << 4)
}

/// Runs INT 21h in real mode with `regs`, and turns the carry flag into an error.
fn rm21(regs: &mut RealRegs) -> Result<(), DosError> {
	dpmi::simulate_int(0x21, regs).map_err(|e| DosError(e.0))?;
	if regs.flags & 1 != 0 {
		Err(DosError(regs.eax as u16))
	} else {
		Ok(())
	}
}

/// Ends the program with `code` after running the clean-up list (restores hooked interrupts and the video mode).
pub fn exit(code: i32) -> ! {
	crate::cleanup::run_all();
	exit_now(code)
}

/// Ends the program at once: AH=4Ch with the status in AL. The host releases the program's memory and descriptors.
pub fn exit_now(code: i32) -> ! {
	loop {
		// SAFETY: AH=4Ch takes no pointer and does not return; the loop is for the impossible case that it does.
		unsafe { asm!("int 0x21", in("eax") 0x4C00 | (code as u32 & 0xFF), options(nostack)) };
	}
}

/// Reports a failure before the runtime is up, and exits with status 255. No allocation, no formatting.
pub fn die(msg: &[u8]) -> ! {
	let _ = write(1, msg);
	exit_now(255)
}

/// AH=09h: print a `$`-terminated string to the console through the transfer buffer. AH=40h is the one to use for anything binary.
pub fn print_dollar(s: &[u8]) {
	let n = s.len().min(TB_SIZE - 1);
	// SAFETY: the buffer is TB_SIZE bytes and n + 1 <= TB_SIZE.
	unsafe {
		core::ptr::copy_nonoverlapping(s.as_ptr(), tb_ptr(), n);
		tb_ptr().add(n).write(b'$');
	}
	let mut r = RealRegs {
		eax: 0x0900,
		ds: tb_segment(),
		..RealRegs::default()
	};
	let _ = rm21(&mut r);
}

/// AH=40h: write to a handle through the transfer buffer. Returns the bytes DOS reports written (all, unless the disk is full).
pub fn write(handle: u16, buf: &[u8]) -> Result<usize, DosError> {
	let mut done = 0;
	while done < buf.len() {
		let n = (buf.len() - done).min(TB_SIZE);
		// SAFETY: the buffer is TB_SIZE bytes of conventional memory and n <= TB_SIZE.
		unsafe { core::ptr::copy_nonoverlapping(buf.as_ptr().add(done), tb_ptr(), n) };
		let mut r = RealRegs {
			eax: 0x4000,
			ebx: handle as u32,
			ecx: n as u32,
			ds: tb_segment(),
			..RealRegs::default()
		};
		rm21(&mut r)?;
		let w = r.eax as usize & 0xFFFF;
		done += w;
		if w < n {
			break;
		}
	}
	Ok(done)
}

/// Writes everything, or fails.
pub fn write_all(handle: u16, buf: &[u8]) -> Result<(), DosError> {
	if write(handle, buf)? == buf.len() {
		Ok(())
	} else {
		Err(DosError(0x1D))
	}
}

/// AH=3Fh: read from a handle through the transfer buffer, looping until `buf` is full or the file ends. Returns 0 at end of file.
pub fn read(handle: u16, buf: &mut [u8]) -> Result<usize, DosError> {
	let mut done = 0;
	while done < buf.len() {
		let n = (buf.len() - done).min(TB_SIZE);
		let mut r = RealRegs {
			eax: 0x3F00,
			ebx: handle as u32,
			ecx: n as u32,
			ds: tb_segment(),
			..RealRegs::default()
		};
		rm21(&mut r)?;
		let got = r.eax as usize & 0xFFFF;
		// SAFETY: `got <= n <= TB_SIZE` bytes were read into the buffer, and `buf[done..]` has room for n.
		unsafe { core::ptr::copy_nonoverlapping(tb_ptr(), buf.as_mut_ptr().add(done), got) };
		done += got;
		if got < n {
			break;
		}
	}
	Ok(done)
}

/// Copies a file name into the buffer as an ASCIIZ string and returns its length including the NUL.
fn put_path(path: &str) -> Result<(), DosError> {
	if path.len() + 1 > TB_SIZE {
		return Err(DosError(3));
	}
	// SAFETY: bounds checked above.
	unsafe {
		core::ptr::copy_nonoverlapping(path.as_ptr(), tb_ptr(), path.len());
		tb_ptr().add(path.len()).write(0);
	}
	Ok(())
}

/// How to open a file.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
	Read,
	/// Create or truncate, for writing
	Create,
}

/// An open file. Closed on drop.
pub struct File {
	handle: u16,
}

impl File {
	/// AH=3Dh (open for reading) or AH=3Ch (create).
	pub fn open(path: &str, mode: Mode) -> Result<File, DosError> {
		put_path(path)?;
		let ax = match mode {
			Mode::Read => 0x3D00,
			Mode::Create => 0x3C00,
		};
		let mut r = RealRegs {
			eax: ax,
			ecx: 0,
			ds: tb_segment(),
			..RealRegs::default()
		};
		rm21(&mut r)?;
		Ok(File { handle: r.eax as u16 })
	}

	pub fn read(&mut self, buf: &mut [u8]) -> Result<usize, DosError> {
		read(self.handle, buf)
	}

	pub fn write_all(&mut self, buf: &[u8]) -> Result<(), DosError> {
		write_all(self.handle, buf)
	}

	/// AH=42h: seek from the start; returns the new position.
	pub fn seek_start(&mut self, pos: u32) -> Result<u32, DosError> {
		self.seek(0, pos)
	}

	/// The length (seek to the end and back).
	pub fn len(&mut self) -> Result<u32, DosError> {
		let here = self.seek(1, 0)?;
		let end = self.seek(2, 0)?;
		self.seek(0, here)?;
		Ok(end)
	}

	pub fn is_empty(&mut self) -> Result<bool, DosError> {
		Ok(self.len()? == 0)
	}

	fn seek(&mut self, whence: u32, offset: u32) -> Result<u32, DosError> {
		let mut r = RealRegs {
			eax: 0x4200 | whence,
			ebx: self.handle as u32,
			ecx: offset >> 16,
			edx: offset & 0xFFFF,
			..RealRegs::default()
		};
		rm21(&mut r)?;
		// DX:AX is the new position.
		Ok(((r.edx & 0xFFFF) << 16) | (r.eax & 0xFFFF))
	}

	/// Reads the whole file into a vector, calling `read` with at most `chunk` bytes at a time (0 = everything in one call).
	pub fn read_to_vec(&mut self, chunk: usize) -> Result<Vec<u8>, DosError> {
		let len = self.len()? as usize;
		let mut v = alloc::vec![0u8; len];
		let mut done = 0;
		while done < len {
			let want = if chunk == 0 { len - done } else { chunk.min(len - done) };
			let n = self.read(&mut v[done..done + want])?;
			if n == 0 {
				break;
			}
			done += n;
		}
		v.truncate(done);
		Ok(v)
	}

	/// The DOS handle.
	pub fn handle(&self) -> u16 {
		self.handle
	}

	/// Finishes a file that was written: AH=68h commits the buffers (DOS 3.3 and later; older DOS answers with an error that does not
	/// matter), then AH=3Eh closes it and reports what the close says. A full disk shows itself here when DOS held the last sector back.
	pub fn finish(self) -> Result<(), DosError> {
		let handle = self.handle;
		core::mem::forget(self);
		let mut r = RealRegs {
			eax: 0x6800,
			ebx: handle as u32,
			..RealRegs::default()
		};
		let _ = rm21(&mut r);
		let mut r = RealRegs {
			eax: 0x3E00,
			ebx: handle as u32,
			..RealRegs::default()
		};
		rm21(&mut r)
	}
}

impl Drop for File {
	fn drop(&mut self) {
		let mut r = RealRegs {
			eax: 0x3E00,
			ebx: self.handle as u32,
			..RealRegs::default()
		};
		let _ = rm21(&mut r);
	}
}

/// AH=41h: delete a file.
pub fn remove(path: &str) -> Result<(), DosError> {
	put_path(path)?;
	let mut r = RealRegs {
		eax: 0x4100,
		ds: tb_segment(),
		..RealRegs::default()
	};
	rm21(&mut r)
}

/// What a protected-mode INT 21h does with a pointer: AH=40h with DS:EDX naming `buf`, no transfer buffer. Returns the count DOS
/// reports. The data DOS actually wrote is whatever sat at the real-mode DS:DX, not `buf`; this exists to demonstrate that.
pub fn write_direct_unsafe(handle: u16, buf: &[u8]) -> Result<usize, DosError> {
	let ax: u32;
	let carry: u32;
	// SAFETY: the call is made with a valid handle and length; DOS reads the bytes it is pointed at, which are in memory.
	unsafe {
		asm!(
			"push ebx",
			"mov ebx, {h}",
			"int 0x21",
			"sbb {c}, {c}",
			"pop ebx",
			h = in(reg) handle as u32,
			c = out(reg) carry,
			inout("eax") 0x4000u32 => ax,
			in("ecx") buf.len() as u32,
			in("edx") buf.as_ptr() as usize as u32,
		);
	}
	if carry != 0 {
		Err(DosError(ax as u16))
	} else {
		Ok(ax as usize & 0xFFFF)
	}
}

// What Liminal Installer added to the copy (see PROVENANCE.md): directories, attributes and free space.

/// AH=39h: create a directory.
pub fn mkdir(path: &str) -> Result<(), DosError> {
	put_path(path)?;
	let mut r = RealRegs {
		eax: 0x3900,
		ds: tb_segment(),
		..RealRegs::default()
	};
	rm21(&mut r)
}

/// AH=3Ah: remove an empty directory.
pub fn rmdir(path: &str) -> Result<(), DosError> {
	put_path(path)?;
	let mut r = RealRegs {
		eax: 0x3A00,
		ds: tb_segment(),
		..RealRegs::default()
	};
	rm21(&mut r)
}

/// AX=4300h: the attributes of a file or directory (bit 4 is a directory).
pub fn attributes(path: &str) -> Result<u16, DosError> {
	put_path(path)?;
	let mut r = RealRegs {
		eax: 0x4300,
		ds: tb_segment(),
		..RealRegs::default()
	};
	rm21(&mut r)?;
	Ok(r.ecx as u16)
}

/// AH=36h: the free space on a drive (0 for A:, like every drive number in this module), in bytes. DOS reports at most 2 GiB. Error
/// 0Fh means no such drive.
pub fn free_space(drive: u8) -> Result<u64, DosError> {
	let mut r = RealRegs {
		eax: 0x3600,
		edx: u32::from(drive) + 1,
		..RealRegs::default()
	};
	rm21(&mut r)?;
	if r.eax as u16 == 0xFFFF {
		return Err(DosError(0x0F));
	}
	Ok(u64::from(r.eax as u16) * u64::from(r.ebx as u16) * u64::from(r.ecx as u16))
}

// What Quill added to the copy (see PROVENANCE.md): searching a directory, drives, rename, version, and the critical-error stub.

/// Longest path the calls below take (DOS paths are at most 80 bytes; the transfer buffer holds a second path at offset 256).
const PATH_MAX: usize = 255;
/// Where the second path of a two-path call (rename) sits in the transfer buffer.
const SECOND_PATH: usize = 256;

/// AX=4301h: sets the attributes of a file or directory (the test program hides a file with it).
pub fn set_attributes(path: &str, attr: u16) -> Result<(), DosError> {
	put_path(path)?;
	let mut r = RealRegs {
		eax: 0x4301,
		ecx: u32::from(attr),
		ds: tb_segment(),
		..RealRegs::default()
	};
	rm21(&mut r)
}

/// AH=56h: renames or moves a file (both paths on the same drive). An existing target is an error (5), as DOS reports it.
pub fn rename(from: &str, to: &str) -> Result<(), DosError> {
	if from.len() > PATH_MAX || to.len() > PATH_MAX {
		return Err(DosError(3));
	}
	put_path(from)?;
	// SAFETY: SECOND_PATH + PATH_MAX + 1 is well inside the buffer.
	unsafe {
		core::ptr::copy_nonoverlapping(to.as_ptr(), tb_ptr().add(SECOND_PATH), to.len());
		tb_ptr().add(SECOND_PATH + to.len()).write(0);
	}
	let mut r = RealRegs {
		eax: 0x5600,
		ds: tb_segment(),
		es: tb_segment(),
		edx: 0,
		edi: SECOND_PATH as u32,
		..RealRegs::default()
	};
	rm21(&mut r)
}

/// AH=30h: the DOS version as (major, minor).
pub fn dos_version() -> (u8, u8) {
	let mut r = RealRegs {
		eax: 0x3000,
		ebx: 0,
		..RealRegs::default()
	};
	let _ = rm21(&mut r);
	(r.eax as u8, (r.eax >> 8) as u8)
}

/// A directory entry from a search, read from the DTA.
#[derive(Clone, Copy, Debug)]
pub struct DirEntry {
	/// Attribute byte: 01 read-only, 02 hidden, 04 system, 08 volume label, 10 directory, 20 archive
	pub attr: u8,
	pub time: u16,
	pub date: u16,
	pub size: u32,
	name: [u8; 13],
}

pub const ATTR_READ_ONLY: u8 = 0x01;
pub const ATTR_HIDDEN: u8 = 0x02;
pub const ATTR_SYSTEM: u8 = 0x04;
pub const ATTR_VOLUME: u8 = 0x08;
pub const ATTR_DIRECTORY: u8 = 0x10;

impl DirEntry {
	/// The 8.3 name as DOS returns it (no path, no trailing NUL).
	pub fn name_bytes(&self) -> &[u8] {
		let n = self.name.iter().position(|&b| b == 0).unwrap_or(self.name.len());
		&self.name[..n]
	}

	pub fn is_dir(&self) -> bool {
		self.attr & ATTR_DIRECTORY != 0
	}
}

/// The error find-next returns when the list is used up.
pub const NO_MORE_FILES: u16 = 0x12;

/// Real-mode segment of the 128-byte DTA, 0 until first use. One block of conventional memory for the life of the program.
static DTA_SEGMENT: AtomicU32 = AtomicU32::new(0);

fn dta_segment() -> u16 {
	let seg = DTA_SEGMENT.load(Ordering::Relaxed);
	if seg != 0 {
		return seg as u16;
	}
	match dpmi::alloc_dos_memory(8) {
		Ok((segment, _)) => {
			let mut r = RealRegs {
				eax: 0x1A00,
				ds: segment,
				edx: 0,
				..RealRegs::default()
			};
			let _ = rm21(&mut r);
			DTA_SEGMENT.store(u32::from(segment), Ordering::Relaxed);
			segment
		}
		Err(_) => die(b"dos: no conventional memory for the DTA\r\n"),
	}
}

fn read_dta() -> DirEntry {
	let base = dpmi::linear_to_ptr::<u8>(u32::from(dta_segment()) << 4);
	let mut e = DirEntry {
		attr: 0,
		time: 0,
		date: 0,
		size: 0,
		name: [0; 13],
	};
	// SAFETY: the DTA block is 128 bytes and the fields read lie in its first 43.
	unsafe {
		e.attr = base.add(0x15).read();
		e.time = u16::from_le_bytes([base.add(0x16).read(), base.add(0x17).read()]);
		e.date = u16::from_le_bytes([base.add(0x18).read(), base.add(0x19).read()]);
		e.size = u32::from_le_bytes([
			base.add(0x1A).read(),
			base.add(0x1B).read(),
			base.add(0x1C).read(),
			base.add(0x1D).read(),
		]);
		for (i, b) in e.name.iter_mut().enumerate() {
			*b = base.add(0x1E + i).read();
		}
	}
	e.name[12] = 0;
	e
}

/// AH=4Eh: starts a search. `attr` is the mask of attributes to include besides normal files (10h adds directories; hidden,
/// system and volume entries come only when their bits are set). The search state lives in the DTA, so only one search runs at a
/// time: copy what you need before starting another. Errors: 2 or 3 for no such path, 12h for no match.
pub fn find_first(pattern: &str, attr: u8) -> Result<DirEntry, DosError> {
	if pattern.len() > PATH_MAX {
		return Err(DosError(3));
	}
	let _ = dta_segment();
	put_path(pattern)?;
	let mut r = RealRegs {
		eax: 0x4E00,
		ecx: u32::from(attr),
		ds: tb_segment(),
		..RealRegs::default()
	};
	rm21(&mut r)?;
	Ok(read_dta())
}

/// AH=4Fh: the next entry of the search `find_first` started. `Err(DosError(NO_MORE_FILES))` at the end.
pub fn find_next() -> Result<DirEntry, DosError> {
	let _ = dta_segment();
	let mut r = RealRegs {
		eax: 0x4F00,
		..RealRegs::default()
	};
	rm21(&mut r)?;
	Ok(read_dta())
}

/// Calls `each` for every entry a search returns, in the order DOS gives them, and returns how many. A search that finds nothing
/// is `Ok(0)` (DOS says 2 or 12h for it, depending on the version); a missing folder (3) is an error.
pub fn find_each(pattern: &str, attr: u8, mut each: impl FnMut(&DirEntry)) -> Result<usize, DosError> {
	let mut n = 0;
	let mut next = match find_first(pattern, attr) {
		Ok(e) => Some(e),
		Err(DosError(NO_MORE_FILES)) => None,
		Err(DosError(2)) if pattern.contains('*') || pattern.contains('?') => None,
		Err(e) => return Err(e),
	};
	while let Some(e) = next {
		each(&e);
		n += 1;
		next = match find_next() {
			Ok(e) => Some(e),
			Err(DosError(NO_MORE_FILES)) => None,
			Err(e) => return Err(e),
		};
	}
	Ok(n)
}

/// AH=19h: the current drive, 0 for A:.
pub fn current_drive() -> u8 {
	let mut r = RealRegs {
		eax: 0x1900,
		..RealRegs::default()
	};
	let _ = rm21(&mut r);
	r.eax as u8
}

/// AH=0Eh: makes `drive` (0 for A:) current and returns how many logical drives DOS has (the highest letter in use, which can
/// count a drive that is not valid).
pub fn set_current_drive(drive: u8) -> u8 {
	let mut r = RealRegs {
		eax: 0x0E00,
		edx: u32::from(drive),
		..RealRegs::default()
	};
	let _ = rm21(&mut r);
	r.eax as u8
}

/// AH=47h: the current directory of `drive` (0 for A:), without the drive letter or the leading backslash (empty at the root).
pub fn current_dir(drive: u8) -> Result<alloc::string::String, DosError> {
	let mut r = RealRegs {
		eax: 0x4700,
		edx: u32::from(drive) + 1,
		ds: tb_segment(),
		esi: 0,
		..RealRegs::default()
	};
	rm21(&mut r)?;
	let mut out = alloc::string::String::new();
	// DOS means the buffer to hold 64 bytes; a DOS that keeps a longer path (DOSBox) writes all of it, and the transfer buffer is big.
	for i in 0..128 {
		// SAFETY: the buffer is 32 KiB and DOS wrote a NUL-terminated path at its start.
		let b = unsafe { tb_ptr().add(i).read() };
		if b == 0 {
			break;
		}
		out.push(b as char);
	}
	Ok(out)
}

/// AX=4409h: whether `drive` (0 for A:) exists, without touching its media. Returns the device attribute word; error 0Fh means no
/// such drive.
pub fn drive_attributes(drive: u8) -> Result<u16, DosError> {
	let mut r = RealRegs {
		eax: 0x4409,
		ebx: u32::from(drive) + 1,
		..RealRegs::default()
	};
	rm21(&mut r)?;
	Ok(r.edx as u16)
}

/// Whether `drive` (0 for A:) is a valid drive according to IOCTL 4409h.
pub fn drive_valid(drive: u8) -> bool {
	drive_attributes(drive).is_ok()
}

/// AX=440Eh: the logical drive that shares `drive`'s device (0 for A:): `Ok(0)` when the device has one letter, otherwise the
/// letter number in 1-based form (1 is A:) that the device is currently known as. A single floppy answers for both A: and B:.
pub fn logical_drive_map(drive: u8) -> Result<u8, DosError> {
	let mut r = RealRegs {
		eax: 0x440E,
		ebx: u32::from(drive) + 1,
		..RealRegs::default()
	};
	rm21(&mut r)?;
	Ok(r.eax as u8)
}

/// AX=4408h: whether `drive` (0 for A:) has removable media, without touching the disk: `Ok(true)` for a floppy, `Ok(false)` for a
/// fixed disk. A network drive and an invalid one are errors (1 and 0Fh).
pub fn drive_removable(drive: u8) -> Result<bool, DosError> {
	let mut r = RealRegs {
		eax: 0x4408,
		ebx: u32::from(drive) + 1,
		..RealRegs::default()
	};
	rm21(&mut r)?;
	Ok(r.eax as u8 == 0)
}

/// INT 2Fh AX=150Bh (MSCDEX): whether `drive` (0 for A:) is a CD-ROM drive. False when MSCDEX is not there. Does not read the disc.
pub fn drive_is_cdrom(drive: u8) -> bool {
	let mut r = RealRegs {
		eax: 0x150B,
		ebx: 0,
		ecx: u32::from(drive),
		..RealRegs::default()
	};
	dpmi::simulate_int(0x2F, &mut r).is_ok() && r.ebx as u16 == 0xADAD && r.eax as u16 != 0
}

/// AH=59h: why the last DOS call failed. With the critical-error stub answering Fail, the call itself says only "failed" (15h or 53h);
/// this gives the cause in the extended error code (13h write-protected, 15h not ready, 1Fh general failure, and so on). It must be
/// the next DOS call after the failure, before anything else runs.
pub fn extended_error() -> u16 {
	let mut r = RealRegs {
		eax: 0x5900,
		ebx: 0,
		..RealRegs::default()
	};
	if dpmi::simulate_int(0x21, &mut r).is_err() {
		return 0;
	}
	r.eax as u16
}

/// INT 11h: how many floppy drives the equipment word reports (0 to 4).
pub fn floppy_count() -> u8 {
	let mut r = RealRegs::default();
	if dpmi::simulate_int(0x11, &mut r).is_err() {
		return 0;
	}
	if r.eax & 1 == 0 {
		0
	} else {
		(((r.eax >> 6) & 3) + 1) as u8
	}
}

/// The valid drives as a bit mask (bit 0 is A:). Each letter is asked with IOCTL 4409h, which does not read the disk, and A: and B:
/// are left out beyond the floppy count so a single drive's phantom B: does not appear.
pub fn drive_mask() -> u32 {
	let floppies = floppy_count();
	let mut mask = 0;
	for d in 0..26u8 {
		if d < 2 && d >= floppies {
			continue;
		}
		if drive_valid(d) {
			mask |= 1 << d;
		}
	}
	mask
}

// INT 24h, the critical-error handler. Without one, a drive that is not ready makes DOS print "Abort, Retry, Fail?" over the
// text screen and wait for a key. The stub is `mov al, 3` (fail the call) and `iret`, in a block of conventional memory.

static CRIT_SEGMENT: AtomicU32 = AtomicU32::new(0);
static CRIT_SELECTOR: AtomicU32 = AtomicU32::new(0);
static CRIT_OLD: AtomicU32 = AtomicU32::new(0);

/// The 3-byte handler.
pub const CRITICAL_STUB: [u8; 3] = [0xB0, 0x03, 0xCF];

/// Installs the stub as the real-mode INT 24h handler and puts the old one back at exit. Safe to call twice.
pub fn install_critical_handler() -> Result<(), DosError> {
	if CRIT_SEGMENT.load(Ordering::Relaxed) != 0 {
		return Ok(());
	}
	let (segment, selector) = dpmi::alloc_dos_memory(1).map_err(|e| DosError(e.0))?;
	let (old_seg, old_off) = dpmi::get_rm_vector(0x24).map_err(|e| DosError(e.0))?;
	// SAFETY: the block is one paragraph (16 bytes) of conventional memory, mapped at its linear address.
	unsafe { core::ptr::copy_nonoverlapping(CRITICAL_STUB.as_ptr(), dpmi::linear_to_ptr::<u8>(u32::from(segment) << 4), 3) };
	dpmi::set_rm_vector(0x24, segment, 0).map_err(|e| DosError(e.0))?;
	CRIT_OLD.store((u32::from(old_seg) << 16) | u32::from(old_off), Ordering::Relaxed);
	CRIT_SELECTOR.store(u32::from(selector), Ordering::Relaxed);
	CRIT_SEGMENT.store(u32::from(segment), Ordering::Relaxed);
	crate::cleanup::register(remove_critical_handler);
	Ok(())
}

/// Puts the previous INT 24h handler back and frees the stub. Safe to call twice; the exit list calls it.
pub fn remove_critical_handler() {
	let segment = CRIT_SEGMENT.swap(0, Ordering::Relaxed);
	if segment == 0 {
		return;
	}
	let old = CRIT_OLD.load(Ordering::Relaxed);
	let _ = dpmi::set_rm_vector(0x24, (old >> 16) as u16, old as u16);
	let _ = dpmi::free_dos_memory(CRIT_SELECTOR.load(Ordering::Relaxed) as u16);
	crate::cleanup::forget(remove_critical_handler);
}

/// The real-mode segment of the installed stub, if there is one (for a test that reads the vector back).
pub fn critical_handler_segment() -> Option<u16> {
	match CRIT_SEGMENT.load(Ordering::Relaxed) {
		0 => None,
		s => Some(s as u16),
	}
}
