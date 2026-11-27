// The module loader: finds a module beside the program, loads it, checks it, starts it and frees it (W9X-131, W9X-135).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Loads Quill's DLL modules.
//!
//! The steps, in order: build the full path from `GetModuleFileNameA(NULL)` (never a bare name, so a stale copy in the current
//! folder, the system folder or `PATH` cannot win); `LoadLibraryA` under `SetErrorMode` so that a missing, damaged or 16-bit file never
//! raises a system box; sort a null handle by `GetLastError`; find the two exports by name; `QuillModuleInfo` then
//! [`quill_modabi::check_info`]; `QuillModuleStart` with a `ModStart` the module checks from its side. A refused module is freed at
//! once. Nothing here allocates.

use core::ffi::c_void;
use core::fmt::Write;

use quill_modabi::{
	check_info, name_of, status_name, Accepted, ModInfo, ModStart, Status, ABI_MAJOR, ABI_MINOR, QS_CORRUPT, QS_MISSING,
	QS_NOMEMORY, QS_OK, QS_UNSUPPORTED, QS_VERSION, WIN32_EXTENSION, WIN32_INFO_EXPORT, WIN32_START_EXPORT,
};

use crate::policy_for;
use crate::sys::{FreeLibrary, GetLastError, GetModuleFileNameA, GetProcAddress, LoadLibraryA, SetErrorMode};

/// `SEM_FAILCRITICALERRORS | SEM_NOOPENFILEERRORBOX`: no system box for a bad file, a missing file or an empty drive.
pub const ERROR_MODE: u32 = 0x0001 | 0x8000;
/// The longest path the loader builds, with its terminating zero.
pub const MAX_PATH: usize = 260;

/// A module that loaded, passed `QuillModuleInfo` and answered `QuillModuleStart`.
#[derive(Clone, Copy, Debug)]
pub struct Loaded {
	/// The module's handle, which is its load address. Pass it to [`free`].
	pub handle: usize,
	/// The flat address of the kind's operation table, as the module left it in `ModStart::table`.
	pub table: u32,
	/// The table's length in bytes.
	pub table_len: u32,
	/// What the module said about itself.
	pub info: ModInfo,
	/// What the check made of it: another build (accepted in a development build) and a newer minor.
	pub accepted: Accepted,
}

/// Why a module did not load. The module is already freed.
#[derive(Clone, Copy, Debug)]
pub struct Failure {
	/// The status to show and remember (`QS_MISSING`, `QS_NOMEMORY`, `QS_VERSION`, `QS_CORRUPT`, `QS_UNSUPPORTED`).
	pub status: Status,
	/// The Windows error behind a null handle, 0 when the file loaded and a check refused it.
	pub os_error: u32,
	/// What the module said about itself, when it got that far.
	pub info: Option<ModInfo>,
}

impl Failure {
	const fn status(status: Status) -> Failure {
		Failure {
			status,
			os_error: 0,
			info: None,
		}
	}
}

/// Sorts the error `LoadLibraryA` left: file, path or module not found (2, 3, 126) is `QS_MISSING`; not enough memory (8, 14) is
/// `QS_NOMEMORY`; a bad image (193, such as a Windows 3.x module in the same folder) and anything else is `QS_VERSION`. Windows NT
/// answers 126 for an absent file named by a full path, Windows 9x answers 2.
pub const fn status_for_error(error: u32) -> Status {
	match error {
		2 | 3 | 126 => QS_MISSING,
		8 | 14 => QS_NOMEMORY,
		_ => QS_VERSION,
	}
}

/// Writes the program's folder and `name` (no zero in it) into `out`, zero-terminated, and returns the length without the zero.
/// `None` when the program's own path cannot be read or the result does not fit.
pub fn beside(name: &[u8], out: &mut [u8; MAX_PATH]) -> Option<usize> {
	// SAFETY: `out` holds MAX_PATH bytes, which is the size passed.
	let got = unsafe { GetModuleFileNameA(core::ptr::null_mut(), out.as_mut_ptr(), MAX_PATH as u32) } as usize;
	// A zero is a failure, and a length that fills the buffer was cut short.
	if got == 0 || got >= MAX_PATH {
		return None;
	}
	let cut = out[..got].iter().rposition(|b| *b == b'\\').map_or(0, |i| i + 1);
	if cut + name.len() + 1 > MAX_PATH || name.contains(&0) {
		return None;
	}
	out[cut..cut + name.len()].copy_from_slice(name);
	out[cut + name.len()] = 0;
	Some(cut + name.len())
}

/// The file name of a kind's module, `QUILLPIC.DLL`, written into `out`; its length, or `None` for a kind with no name.
pub fn file_name(kind: u16, out: &mut [u8; 16]) -> Option<usize> {
	let stem = name_of(kind)?.as_bytes();
	let ext = WIN32_EXTENSION.as_bytes();
	out[..stem.len()].copy_from_slice(stem);
	out[stem.len()..stem.len() + ext.len()].copy_from_slice(ext);
	Some(stem.len() + ext.len())
}

type InfoFn = unsafe extern "C" fn(info: u32) -> Status;
type StartFn = unsafe extern "C" fn(start: u32) -> Status;

/// Loads the module of `kind` from the program's folder (`QUILLPIC.DLL` for pictures) for a program built as `own_build`; see
/// [`load_file`]. `QS_MISSING` for a kind with no file name.
pub fn load(kind: u16, own_build: u32) -> Result<Loaded, Failure> {
	let mut name = [0u8; 16];
	let Some(len) = file_name(kind, &mut name) else {
		return Err(Failure::status(QS_UNSUPPORTED));
	};
	load_file(&name[..len], kind, own_build)
}

/// Loads the file `name` (an upper-case 8.3 name with no folder) from the program's folder as a module of `kind`, for a program built
/// as `own_build`. The policy follows the build id (W9X-135): release when it is clean, development when its dirty bit is set.
pub fn load_file(name: &[u8], kind: u16, own_build: u32) -> Result<Loaded, Failure> {
	let mut path = [0u8; MAX_PATH];
	if beside(name, &mut path).is_none() {
		return Err(Failure::status(QS_MISSING));
	}
	// SAFETY: `path` is zero-terminated; the old error mode is put back before anything else can fail.
	let (handle, os_error) = unsafe {
		let old = SetErrorMode(ERROR_MODE);
		let handle = LoadLibraryA(path.as_ptr());
		let os_error = if handle.is_null() { GetLastError() } else { 0 };
		SetErrorMode(old);
		(handle, os_error)
	};
	if handle.is_null() {
		return Err(Failure {
			status: status_for_error(os_error),
			os_error,
			info: None,
		});
	}
	match start_module(handle, kind, own_build) {
		Ok(loaded) => Ok(loaded),
		Err(failure) => {
			// SAFETY: `handle` came from LoadLibraryA above and is not used again.
			unsafe { FreeLibrary(handle) };
			Err(failure)
		}
	}
}

fn start_module(handle: *mut c_void, kind: u16, own_build: u32) -> Result<Loaded, Failure> {
	// SAFETY: the names are zero-terminated; a null answer is handled; the pointers are only called with the signatures the contract
	// gives them (cdecl, one address in, a status out).
	let (info_fn, start_fn): (InfoFn, StartFn) = unsafe {
		let info_fn = symbol(handle as usize, WIN32_INFO_EXPORT);
		let start_fn = symbol(handle as usize, WIN32_START_EXPORT);
		match (info_fn, start_fn) {
			(Some(i), Some(s)) => (
				core::mem::transmute::<usize, InfoFn>(i),
				core::mem::transmute::<usize, StartFn>(s),
			),
			_ => return Err(Failure::status(QS_VERSION)),
		}
	};
	let mut info = ModInfo {
		size: core::mem::size_of::<ModInfo>() as u16,
		..ModInfo::default()
	};
	// SAFETY: `info` is a ModInfo of the size it declares.
	let status = unsafe { info_fn(&mut info as *mut ModInfo as usize as u32) };
	if status != QS_OK {
		return Err(Failure {
			status,
			os_error: 0,
			info: None,
		});
	}
	let accepted = match check_info(&info, kind, own_build, policy_for(own_build)) {
		Ok(accepted) => accepted,
		Err(status) => {
			return Err(Failure {
				status,
				os_error: 0,
				info: Some(info),
			})
		}
	};
	let mut start = ModStart {
		size: core::mem::size_of::<ModStart>() as u16,
		abi_major: ABI_MAJOR,
		abi_minor: ABI_MINOR,
		kind,
		build_id: own_build,
		table: 0,
		table_len: 0,
	};
	// SAFETY: `start` is a ModStart of the size it declares.
	let status = unsafe { start_fn(&mut start as *mut ModStart as usize as u32) };
	if status != QS_OK {
		return Err(Failure {
			status,
			os_error: 0,
			info: Some(info),
		});
	}
	// A table the module did not leave, or one too short to hold its own size field, is a broken module.
	if start.table == 0 || (start.table_len as usize) < core::mem::size_of::<u32>() {
		return Err(Failure {
			status: QS_CORRUPT,
			os_error: 0,
			info: Some(info),
		});
	}
	Ok(Loaded {
		handle: handle as usize,
		table: start.table,
		table_len: start.table_len,
		info,
		accepted,
	})
}

/// The address of the export `name` (with its terminating zero) of the module `handle`, or `None`.
///
/// # Safety
/// `handle` must be a module handle from `LoadLibraryA` that has not been freed.
pub unsafe fn symbol(handle: usize, name: &str) -> Option<usize> {
	let mut zeroed = [0u8; 32];
	let bytes = name.as_bytes();
	if bytes.len() >= zeroed.len() {
		return None;
	}
	zeroed[..bytes.len()].copy_from_slice(bytes);
	let address = GetProcAddress(handle as *mut c_void, zeroed.as_ptr()) as usize;
	if address == 0 {
		None
	} else {
		Some(address)
	}
}

/// Frees a module loaded by [`load`] or [`load_file`]. Call it before `ExitProcess`; Windows would free it at exit anyway.
pub fn free(loaded: Loaded) {
	// SAFETY: `handle` came from LoadLibraryA and the caller drops the table with it.
	unsafe { FreeLibrary(loaded.handle as *mut c_void) };
}

/// A fixed-size line buffer for [`log_line`] and for any line a caller formats with `write!`.
pub struct Line {
	bytes: [u8; 200],
	len: usize,
}

impl Line {
	/// An empty line.
	pub const fn new() -> Line {
		Line { bytes: [0; 200], len: 0 }
	}

	/// The line, with no line ending.
	pub fn as_bytes(&self) -> &[u8] {
		&self.bytes[..self.len]
	}
}

impl Default for Line {
	fn default() -> Line {
		Line::new()
	}
}

impl Write for Line {
	fn write_str(&mut self, text: &str) -> core::fmt::Result {
		let room = self.bytes.len() - self.len;
		let take = text.len().min(room);
		self.bytes[self.len..self.len + take].copy_from_slice(&text.as_bytes()[..take]);
		self.len += take;
		Ok(())
	}
}

/// The line `QUILL.LOG` gets for a load (with `[Debug] Trace=1`): the file, the load address, the status and both build ids, so a
/// fault address can be read against the module's `.MAP`. For a refusal it adds the Windows error and `from another build` or
/// `not a Win32 module` where they apply.
pub fn log_line(name: &str, result: &Result<Loaded, Failure>, own_build: u32) -> Line {
	let mut line = Line::new();
	let _ = match result {
		Ok(loaded) => {
			let _ = write!(
				line,
				"{name} at {:08x} OK ours {own_build:08x} theirs {:08x}",
				loaded.handle, loaded.info.build_id
			);
			if loaded.accepted.other_build {
				write!(line, " (other build, accepted in a development build)")
			} else {
				Ok(())
			}
		}
		Err(failure) => {
			let _ = write!(line, "{name} {} ours {own_build:08x}", status_name(failure.status));
			if let Some(info) = failure.info {
				let _ = write!(line, " theirs {:08x}", info.build_id);
				if info.build_id != own_build && failure.status == QS_VERSION {
					let _ = write!(line, " (from another build)");
				}
			}
			if failure.os_error != 0 {
				let _ = write!(line, " error {}", failure.os_error);
				if failure.os_error == 193 {
					let _ = write!(line, " (not a Win32 module)");
				}
			}
			Ok(())
		}
	};
	line
}
