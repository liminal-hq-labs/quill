// The optional-module interface: the one definition of what crosses between Quill's programs and their modules.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The contract between a Quill program and an optional module (CORE-90).
//!
//! A module is a cold feature (picture decoding, the file dialog) built from a shared crate behind this interface and packaged per
//! platform: a `.DMD` file on DOS (`dos/module`), an NE DLL on Windows 3.x, nothing at all on the platforms that link the feature
//! in. Both packagings and the program's loaders read their structs, status codes and ordinals from here, so a change to the layout
//! fails a test in one place instead of drifting between two loaders.
//!
//! **What crosses.** Only `#[repr(C)]` structs of `u16` and `u32`, status codes and ids. No Rust types, slices, traits or `Option`
//! of data cross a call. An address is a `u32` "module address": a near offset on DOS (the flat 4 GiB data segment), a packed
//! selector and offset on Win16. Only a platform's glue turns one into a pointer; this crate never does.
//!
//! **Who owns memory.** The caller, always. A module reads and writes only the blocks a call names, only for the length of that
//! call, and keeps none of them. A module that needs working memory says how much in [`ModInfo::arena`], and the caller gives it.
//!
//! **Versions (CORE-94).** [`ABI_MAJOR`] changes when a layout or an operation changes incompatibly; a caller refuses another
//! major. [`ABI_MINOR`] grows when something is added at the end of a struct or a table; a caller accepts a higher minor, which is
//! why every struct that can grow starts with its own `size`. A *release* build also requires the module's build id to equal its
//! own, so the program and its modules always come from one build; a development build accepts another build id and says so.
//! File names carry no version digit.
//!
//! The crate is `no_std`, allocates nothing, uses no floats and never panics on any input: every check returns a [`Status`].

#![no_std]
#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]

use core::mem::size_of;

/// The interface's major version. A caller refuses a module with another major.
pub const ABI_MAJOR: u16 = 1;
/// The interface's minor version. A caller accepts a module with a higher minor and uses only what it knows.
pub const ABI_MINOR: u16 = 0;

// ---------------------------------------------------------------------------------------------------------------------------
// Kinds and names
// ---------------------------------------------------------------------------------------------------------------------------

/// Picture decoding: bytes in from a chunk list, rows out to the caller's DIB rows.
pub const KIND_PIC: u16 = 1;
/// Dialogs (the DOS file dialog first). The operation table arrives with the dialog host's SDK version (SDK 2); until then the kind
/// is only a number a caller can refuse.
pub const KIND_DLG: u16 = 2;
/// Reserved for the help viewer.
pub const KIND_HLP: u16 = 3;
/// Reserved for PDF output.
pub const KIND_PDF: u16 = 4;

/// The file name stem of the picture module (`QUILLPIC.DMD`, `QUILLPIC.DLL`). Stems never carry a version digit (CORE-94).
pub const NAME_PIC: &str = "QUILLPIC";
/// The file name stem of the dialog module.
pub const NAME_DLG: &str = "QUILLDLG";
/// The file name stem of the help module (reserved).
pub const NAME_HLP: &str = "QUILLHLP";
/// The file name stem of the PDF module (reserved).
pub const NAME_PDF: &str = "QUILLPDF";

/// The file name stem of a kind, or `None` for a kind this interface does not know.
pub const fn name_of(kind: u16) -> Option<&'static str> {
	match kind {
		KIND_PIC => Some(NAME_PIC),
		KIND_DLG => Some(NAME_DLG),
		KIND_HLP => Some(NAME_HLP),
		KIND_PDF => Some(NAME_PDF),
		_ => None,
	}
}

/// The extension of a DOS module file, with the dot. The format's own magic (`DMD1`) and SDK version carry the format's version.
pub const DOS_EXTENSION: &str = ".DMD";
/// The extension of a Windows 3.x module file, with the dot.
pub const WIN16_EXTENSION: &str = ".DLL";
/// The extension of a Windows 95 and later module file, with the dot. The stems are the same as on Windows 3.x; the two builds keep
/// their files in folders of their own where they share one (INS-27).
pub const WIN32_EXTENSION: &str = ".DLL";
/// The name a Win32 module exports `info(*ModInfo) -> Status` under (cdecl, undecorated, found with `GetProcAddress`; no ordinals).
pub const WIN32_INFO_EXPORT: &str = "QuillModuleInfo";
/// The name a Win32 module exports `start(*ModStart) -> Status` under: it fills the kind's table, as a `.DMD`'s `app_start` does.
pub const WIN32_START_EXPORT: &str = "QuillModuleStart";

// ---------------------------------------------------------------------------------------------------------------------------
// Status
// ---------------------------------------------------------------------------------------------------------------------------

/// What a module call, or a loader, answers: one of the `QS_*` values below. A `u16` so it crosses a 16-bit call unchanged.
pub type Status = u16;

/// The call finished.
pub const QS_OK: Status = 0;
/// The call made progress and wants to be called again (rows: more rows remain).
pub const QS_MORE: Status = 1;
/// A session is already open; close it first.
pub const QS_BUSY: Status = 2;
/// The input is well-formed but uses something this module does not do (an interlaced or 16-bit picture it does not decode).
pub const QS_UNSUPPORTED: Status = 3;
/// The input is damaged, or a struct handed to the module is malformed.
pub const QS_CORRUPT: Status = 4;
/// The input is larger than the module's [`ModInfo::limit`].
pub const QS_TOO_LARGE: Status = 5;
/// The loader could not make the module's addresses reachable (DOS: CS and DS do not share a base, or the host refused to widen CS).
pub const QS_NOALIAS: Status = 6;
/// The module failed inside (a panic on a platform that contains it, a broken invariant). The module is unusable until reloaded.
pub const QS_INTERNAL: Status = 7;
/// The module file is absent or could not be read. Never raised by a module; a loader's answer.
pub const QS_MISSING: Status = 8;
/// The module is from another interface major, or, in a release build, from another build.
pub const QS_VERSION: Status = 9;
/// The loader could not get the memory to load the module. Never raised by a module; a loader's answer (Windows 95 and later), so an
/// addition within major 1.
pub const QS_NOMEMORY: Status = 10;

/// A short name for a status, for the log line a refused module writes (`QUILL.LOG`, `PICS.LOG`). Unknown values read `"?"`.
pub const fn status_name(status: Status) -> &'static str {
	match status {
		QS_OK => "OK",
		QS_MORE => "MORE",
		QS_BUSY => "BUSY",
		QS_UNSUPPORTED => "UNSUPPORTED",
		QS_CORRUPT => "CORRUPT",
		QS_TOO_LARGE => "TOO_LARGE",
		QS_NOALIAS => "NOALIAS",
		QS_INTERNAL => "INTERNAL",
		QS_MISSING => "MISSING",
		QS_VERSION => "VERSION",
		QS_NOMEMORY => "NOMEMORY",
		_ => "?",
	}
}

// ---------------------------------------------------------------------------------------------------------------------------
// Structs
// ---------------------------------------------------------------------------------------------------------------------------

/// What a module says about itself: filled by the module's `info` entry (Win16 ordinal [`PIC_INFO`], DOS table slot `info`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ModInfo {
	/// `size_of::<ModInfo>()` of the module's own version, so a newer module may add fields at the end. The caller reads only what it
	/// knows and refuses a size below its own.
	pub size: u16,
	/// The module's [`ABI_MAJOR`]. The caller refuses another major.
	pub abi_major: u16,
	/// The module's [`ABI_MINOR`]. The caller accepts a higher one.
	pub abi_minor: u16,
	/// The module's kind ([`KIND_PIC`] and so on).
	pub kind: u16,
	/// The build both files were made from, see [`build_id`]. A release build requires equality.
	pub build_id: u32,
	/// Bytes of working memory the module holds (Win16), 0 on DOS.
	pub arena: u32,
	/// Kind-specific: for pictures, the largest side in pixels.
	pub limit: u32,
}

/// What the host gives a DOS module's `app_start`, and what the module gives back: the host's version and build in, the kind's
/// operation table out. The host sets every field but `table` and `table_len`; the module sets those two before it returns zero.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ModStart {
	/// `size_of::<ModStart>()` of the host's version.
	pub size: u16,
	/// The host's [`ABI_MAJOR`]. A module that does not know this major refuses to start.
	pub abi_major: u16,
	/// The host's [`ABI_MINOR`].
	pub abi_minor: u16,
	/// The kind the host loaded this file for. A module of another kind refuses to start.
	pub kind: u16,
	/// The host's build id.
	pub build_id: u32,
	/// Out: the module address of the kind's table (for pictures a [`PicTableV1`]), in the module's own image.
	pub table: u32,
	/// Out: the byte length of that table.
	pub table_len: u32,
}

/// One block of caller memory.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Chunk {
	/// The module address of the first byte: a near offset on DOS, `selector:offset` on Win16.
	pub addr: u32,
	/// Its length in bytes.
	pub len: u32,
}

/// A list of [`Chunk`]s read in order as one stream (a picture arrives in file-sized pieces on Win16).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChunkList {
	/// The number of chunks.
	pub count: u16,
	/// Zero.
	pub pad: u16,
	/// The module address of the first [`Chunk`].
	pub chunks: u32,
}

/// What `open` is given for a picture, and what it answers once the header is read.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PicOpen {
	/// In: the picture's container as the caller sniffed it (`quill-picture`'s kind id).
	pub kind: u16,
	/// In: flags, zero in version 1.
	pub flags: u16,
	/// Out: the picture's width in pixels.
	pub width: u32,
	/// Out: the picture's height in pixels.
	pub height: u32,
	/// Out: the EXIF-style orientation, 1 when the container has none.
	pub orientation: u16,
	/// Out: bits per pixel of the rows the module will write (1, 4, 8 or 24).
	pub depth: u16,
}

/// What `rows` is given to decode into, and what it answers.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PicRows {
	/// In: the module address of the first row to write.
	pub dst: u32,
	/// In: the byte distance between two rows.
	pub stride: u32,
	/// In: the width in pixels the rows are decoded at (the shown size).
	pub shown_width: u16,
	/// In: the most rows to decode in this call. Out: the number written.
	pub rows: u16,
	/// Out: non-zero once the last row is written (the call also answers [`QS_OK`] instead of [`QS_MORE`]).
	pub done: u16,
	/// Zero.
	pub pad: u16,
	/// In: the module address of a 256-entry palette to fill (`B, G, R, 0` each), or 0 when the depth needs none.
	pub palette: u32,
}

// ---------------------------------------------------------------------------------------------------------------------------
// The picture kind, version 1
// ---------------------------------------------------------------------------------------------------------------------------

/// An exported function: its name in the DOS table, and its ordinal in a Win16 DLL.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Export {
	/// The operation's name.
	pub name: &'static str,
	/// The ordinal a Windows 3.x DLL exports it as. `WEP` is ordinal 1.
	pub ordinal: u16,
}

/// Win16: the DLL's exit procedure, required by Windows.
pub const PIC_WEP: u16 = 1;
/// Win16 ordinal of `info(*ModInfo) -> Status`.
pub const PIC_INFO: u16 = 2;
/// Win16 ordinal of `open(owner, kind, *ChunkList, *PicOpen) -> Status`.
pub const PIC_OPEN: u16 = 3;
/// Win16 ordinal of `rows(*PicRows) -> Status`.
pub const PIC_ROWS: u16 = 4;
/// Win16 ordinal of `close() -> Status`.
pub const PIC_CLOSE: u16 = 5;

/// The picture kind's exports, in ordinal order. On DOS the same four operations are the entries of [`PicTableV1`].
pub const PIC_EXPORTS: [Export; 5] = [
	Export {
		name: "WEP",
		ordinal: PIC_WEP,
	},
	Export {
		name: "info",
		ordinal: PIC_INFO,
	},
	Export {
		name: "open",
		ordinal: PIC_OPEN,
	},
	Export {
		name: "rows",
		ordinal: PIC_ROWS,
	},
	Export {
		name: "close",
		ordinal: PIC_CLOSE,
	},
];

/// `info(info) -> Status`: fills a [`ModInfo`] at the module address `info`.
pub type PicInfoFn = unsafe extern "C" fn(info: u32) -> Status;
/// `open(owner, kind, chunks, open) -> Status`: starts a session for the picture `owner` (the caller's token for it) from the
/// [`ChunkList`] at `chunks`, reads the header and fills the answers of the [`PicOpen`] at `open`. [`QS_BUSY`] while a session is open.
pub type PicOpenFn = unsafe extern "C" fn(owner: u32, kind: u16, chunks: u32, open: u32) -> Status;
/// `rows(rows) -> Status`: decodes up to `rows` rows into the [`PicRows`] at `rows`; [`QS_MORE`] until the last.
pub type PicRowsFn = unsafe extern "C" fn(rows: u32) -> Status;
/// `close() -> Status`: ends the session and forgets everything about it. Safe to call with no session open.
pub type PicCloseFn = unsafe extern "C" fn() -> Status;

/// The DOS operation table of the picture kind, version 1: what the module's `app_start` leaves at [`ModStart::table`]. Each entry is
/// a function pointer into the module's own image, so the table sits in writable memory the module fills at start.
///
/// Adding an entry at the end is a minor change; a caller reads `size` and calls only the entries inside it.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PicTableV1 {
	/// `size_of::<PicTableV1>()` of the module's version.
	pub size: u32,
	/// See [`PicInfoFn`].
	pub info: Option<PicInfoFn>,
	/// See [`PicOpenFn`].
	pub open: Option<PicOpenFn>,
	/// See [`PicRowsFn`].
	pub rows: Option<PicRowsFn>,
	/// See [`PicCloseFn`].
	pub close: Option<PicCloseFn>,
}

/// The names of [`PicTableV1`]'s entries in order, for the log.
pub const PIC_TABLE_V1_NAMES: [&str; 4] = ["info", "open", "rows", "close"];

// On DOS and every other 32-bit target the table is five 32-bit words, the layout the module and the loader both assume. The host
// tests run on 64-bit machines, where the pointers are wider, so this is checked where it applies: the DOS build of the crate.
#[cfg(target_pointer_width = "32")]
const _: () = assert!(size_of::<PicTableV1>() == 20 && size_of::<Option<PicInfoFn>>() == 4);

// ---------------------------------------------------------------------------------------------------------------------------
// Builds and the compatibility check
// ---------------------------------------------------------------------------------------------------------------------------

/// The bit set in a build id when the tree it came from had uncommitted changes.
pub const BUILD_DIRTY: u32 = 0x8000_0000;

/// A build id: the first eight hex digits of the commit (the 32 bits `commit`, with the top bit cleared) and the dirty flag in the
/// top bit. Two builds of one clean commit have one id; any dirty tree has an id no clean commit has.
pub const fn build_id(commit: u32, dirty: bool) -> u32 {
	(commit & !BUILD_DIRTY) | if dirty { BUILD_DIRTY } else { 0 }
}

/// Whether a build id says its tree was dirty.
pub const fn is_dirty(build: u32) -> bool {
	build & BUILD_DIRTY != 0
}

/// How strictly a program checks its modules.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Policy {
	/// A shipped build: only its own build id is accepted.
	Release,
	/// A build under development: another build id is accepted and reported, so one side can be rebuilt alone.
	Development,
}

/// What a module's [`ModInfo`] is accepted as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Accepted {
	/// The module is from another build than the program: a development build accepted it and the loader should log the two ids.
	pub other_build: bool,
	/// The module's minor version is higher than the program's: only what the program knows is used.
	pub newer_minor: bool,
}

/// Checks what a module says about itself against what the program wants.
///
/// Refusals, in the order they are checked: a `size` below [`ModInfo`]'s own is [`QS_CORRUPT`]; another `kind` than `want_kind` is
/// [`QS_UNSUPPORTED`]; another `abi_major` is [`QS_VERSION`]; and in a [`Policy::Release`] build another build id is [`QS_VERSION`].
/// Never panics, whatever the fields hold.
pub fn check_info(
	info: &ModInfo,
	want_kind: u16,
	host_build: u32,
	policy: Policy,
) -> Result<Accepted, Status> {
	if (info.size as usize) < size_of::<ModInfo>() {
		return Err(QS_CORRUPT);
	}
	if info.kind != want_kind {
		return Err(QS_UNSUPPORTED);
	}
	if info.abi_major != ABI_MAJOR {
		return Err(QS_VERSION);
	}
	let other_build = info.build_id != host_build;
	if other_build && policy == Policy::Release {
		return Err(QS_VERSION);
	}
	Ok(Accepted {
		other_build,
		newer_minor: info.abi_minor > ABI_MINOR,
	})
}

/// The same check from the module's side, for its `app_start`: a [`ModStart`] the host built, against the module's own `kind` and
/// build id. A module refuses a host of another major or another kind, and in a release build another build.
pub fn check_start(
	start: &ModStart,
	own_kind: u16,
	own_build: u32,
	policy: Policy,
) -> Result<Accepted, Status> {
	if (start.size as usize) < size_of::<ModStart>() {
		return Err(QS_CORRUPT);
	}
	if start.kind != own_kind {
		return Err(QS_UNSUPPORTED);
	}
	if start.abi_major != ABI_MAJOR {
		return Err(QS_VERSION);
	}
	let other_build = start.build_id != own_build;
	if other_build && policy == Policy::Release {
		return Err(QS_VERSION);
	}
	Ok(Accepted {
		other_build,
		newer_minor: start.abi_minor > ABI_MINOR,
	})
}
