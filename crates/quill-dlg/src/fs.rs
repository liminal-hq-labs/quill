// What the file dialog needs from a file system, and nothing it does not.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::name83::PathBuf;

/// A file or folder that cannot be seen by a normal listing: read-only is the only one of these a dialog shows.
pub const ATTR_READONLY: u8 = 0x01;
pub const ATTR_HIDDEN: u8 = 0x02;
pub const ATTR_SYSTEM: u8 = 0x04;
pub const ATTR_VOLUME: u8 = 0x08;
/// A folder.
pub const ATTR_DIR: u8 = 0x10;

/// Why a file system call failed.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FsError {
	/// The drive has no disk, or is not ready (DOS: the critical-error handler answered Fail)
	NotReady,
	/// No such file or folder
	NotFound,
	/// A folder on the path does not exist
	NoPath,
	/// Access denied, or a read-only file
	Denied,
	/// The disk is full
	Full,
	/// This file system cannot do that
	Unsupported,
}

/// What a drive is, for the icon and for whether the dialog may read it at once.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DriveKind {
	/// A hard disk
	Fixed,
	/// A floppy or other removable disk: listed, never read until chosen
	Removable,
	/// A CD-ROM
	CdRom,
	/// A network drive
	Network,
}

/// One entry of a listing, as DOS gives it: the 8.3 name, the attributes and the DTA's size, date and time.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Found<'a> {
	pub name: &'a [u8],
	pub attr: u8,
	/// Bytes; 0 for a folder
	pub size: u32,
	/// DOS date: bits 9 to 15 the year since 1980, 5 to 8 the month, 0 to 4 the day
	pub date: u16,
	/// DOS time: bits 11 to 15 the hour, 5 to 10 the minute, 0 to 4 the seconds in twos
	pub time: u16,
}

/// The file system under a dialog. The shell implements it over INT 21h on DOS, and a fake implements it for tests. Paths are
/// upper-case 8.3 paths with a drive, such as `C:\DOCS\A.QLD`; a folder has no trailing backslash except a root.
pub trait FileSystem {
	/// Calls `each` for every file and folder in `dir`. A real DOS listing asks for attribute `10h` and so returns no hidden, system
	/// or volume entries, and a dialog drops any it is given. Names may include `.` and `..`; the dialog skips them.
	fn list(&mut self, dir: &[u8], each: &mut dyn FnMut(Found<'_>)) -> Result<(), FsError>;

	/// The drives that exist, as a bit mask with bit 0 for `A:`. It must not touch the media: a floppy drive is listed whether or
	/// not it holds a disk.
	fn drives(&mut self) -> u32;

	/// What kind of drive `letter` is. It must not read the disk. Anything the file system cannot tell is a fixed disk.
	fn drive_kind(&mut self, _letter: u8) -> DriveKind {
		DriveKind::Fixed
	}

	/// The current folder of `drive` (a letter, or 0 for the current drive), as a full path. `Err(NotReady)` for an empty drive.
	fn current_dir(&mut self, drive: u8, out: &mut PathBuf) -> Result<(), FsError>;

	/// The attributes of a file or folder, or `None` when there is none. A root folder always exists.
	fn exists(&mut self, path: &[u8]) -> Option<u8>;

	/// Makes a folder.
	fn mkdir(&mut self, _path: &[u8]) -> Result<(), FsError> {
		Err(FsError::Unsupported)
	}

	/// Renames a file, as the safe save does to put the finished temporary file in place.
	fn rename(&mut self, from: &[u8], to: &[u8]) -> Result<(), FsError>;

	/// Deletes a file.
	fn remove(&mut self, path: &[u8]) -> Result<(), FsError>;
}
