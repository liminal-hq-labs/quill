// An in-memory file system tree for tests and for the shells' host builds: drives, folders and files, with the quirks of a DOS
// listing that the dialog must cope with.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// This module uses `std`, so it exists only in tests and behind the `fake` feature; the dialog itself never allocates.

use crate::fs::{DriveKind, FileSystem, Found, FsError, ATTR_DIR};
use crate::name83::PathBuf;
use std::string::String;
use std::vec::Vec;

/// A tree of folders and files on up to 26 drives.
#[derive(Clone, Debug, Default)]
pub struct FakeFs {
	/// (full upper-case path, attributes, size, date, time); drive roots are not listed
	nodes: Vec<(String, u8, u32, u16, u16)>,
	/// The kind of each drive that is not a fixed disk
	kinds: Vec<(u8, DriveKind)>,
	drive_mask: u32,
	cwd: Vec<(u8, String)>,
	/// Drives that answer `NotReady`, as a bit mask
	pub not_ready: u32,
	/// Return `.` and `..` in every folder but a root, as DOS does
	pub dots: bool,
	/// Return the entries last-first, and in lower case
	pub reversed: bool,
	pub lower: bool,
	/// How many listings were asked for
	pub lists: usize,
	/// The date and time a folder made by `add_dir` has (a real disk's folders all have one)
	pub dir_stamp: (u16, u16),
	/// The date and time a folder made by `mkdir` has
	pub now: (u16, u16),
}

impl FakeFs {
	/// A file system with the given drives (letters).
	pub fn new(drives: &[u8]) -> FakeFs {
		let mut f = FakeFs::default();
		for d in drives {
			f.drive_mask |= 1 << (d.to_ascii_uppercase() - b'A');
		}
		f
	}

	/// The disk Quill's DOS tests mount: `C:`, `D:` and `Z:`.
	pub fn dosbox() -> FakeFs {
		let mut f = FakeFs::new(b"CDZ");
		f.dots = true;
		f
	}

	/// Adds a drive (a letter) of a kind; it answers `NotReady` when `not_ready` is true.
	pub fn add_drive(&mut self, letter: u8, kind: DriveKind, not_ready: bool) {
		let bit = 1u32 << (letter.to_ascii_uppercase() - b'A');
		self.drive_mask |= bit;
		self.set_kind(letter, kind);
		if not_ready {
			self.not_ready |= bit;
		} else {
			self.not_ready &= !bit;
		}
	}

	/// Adds a folder (and any missing parents).
	pub fn add_dir(&mut self, path: &str) {
		let path = path.to_ascii_uppercase();
		if let Some((parent, _)) = path.rsplit_once('\\') {
			if parent.len() > 2 && !self.has(parent) {
				self.add_dir(parent);
			}
		}
		if !self.has(&path) {
			let (date, time) = self.dir_stamp;
			self.nodes.push((path, ATTR_DIR, 0, date, time));
		}
	}

	/// Adds a file with attributes `attr` (and any missing parents).
	pub fn add_file(&mut self, path: &str, attr: u8) {
		self.add_file_info(path, attr, 0, 0, 0);
	}

	/// Adds a file with attributes, size and DOS date and time (and any missing parents).
	pub fn add_file_info(&mut self, path: &str, attr: u8, size: u32, date: u16, time: u16) {
		let path = path.to_ascii_uppercase();
		if let Some((parent, _)) = path.rsplit_once('\\') {
			if parent.len() > 2 {
				self.add_dir(parent);
			}
		}
		if let Some(n) = self.nodes.iter_mut().find(|n| n.0 == path) {
			(n.1, n.2, n.3, n.4) = (attr, size, date, time);
		} else {
			self.nodes.push((path, attr, size, date, time));
		}
	}

	/// Says what kind `drive` is (the default is a fixed disk).
	pub fn set_kind(&mut self, drive: u8, kind: DriveKind) {
		let d = drive.to_ascii_uppercase();
		self.kinds.retain(|k| k.0 != d);
		self.kinds.push((d, kind));
	}

	fn has(&self, path: &str) -> bool {
		self.nodes.iter().any(|n| n.0 == path)
	}

	/// Sets the current folder of a drive.
	pub fn set_cwd(&mut self, drive: u8, path: &str) {
		let d = drive.to_ascii_uppercase();
		self.cwd.retain(|c| c.0 != d);
		self.cwd.push((d, path.to_ascii_uppercase()));
	}

	/// How many files and folders there are in all.
	pub fn len(&self) -> usize {
		self.nodes.len()
	}

	pub fn is_empty(&self) -> bool {
		self.nodes.is_empty()
	}

	/// The entries directly in `dir`, as (name, attributes, size, date, time).
	pub fn children(&self, dir: &str) -> Vec<(String, u8, u32, u16, u16)> {
		let mut v: Vec<(String, u8, u32, u16, u16)> = self
			.nodes
			.iter()
			.filter_map(|(p, a, size, date, time)| {
				let (parent, name) = p.rsplit_once('\\')?;
				let parent = if parent.len() == 2 {
					std::format!("{parent}\\")
				} else {
					parent.into()
				};
				(parent == dir).then(|| (name.into(), *a, *size, *date, *time))
			})
			.collect();
		if self.reversed {
			v.reverse();
		}
		v
	}

	fn drive_ok(&self, path: &[u8]) -> Result<u8, FsError> {
		let d = path
			.first()
			.map(|b| b.to_ascii_uppercase())
			.ok_or(FsError::NoPath)?;
		if !d.is_ascii_uppercase() || self.drive_mask & (1 << (d - b'A')) == 0 {
			return Err(FsError::NoPath);
		}
		if self.not_ready & (1 << (d - b'A')) != 0 {
			return Err(FsError::NotReady);
		}
		Ok(d)
	}
}

fn text(b: &[u8]) -> String {
	String::from_utf8_lossy(b).to_ascii_uppercase()
}

impl FileSystem for FakeFs {
	fn list(&mut self, dir: &[u8], each: &mut dyn FnMut(Found<'_>)) -> Result<(), FsError> {
		self.lists += 1;
		self.drive_ok(dir)?;
		let dir = text(dir);
		let is_root = dir.len() == 3;
		if !is_root && !self.nodes.iter().any(|n| n.0 == dir && n.1 & ATTR_DIR != 0) {
			return Err(FsError::NoPath);
		}
		if self.dots && !is_root {
			for dots in [&b"."[..], b".."] {
				each(Found {
					name: dots,
					attr: ATTR_DIR,
					size: 0,
					date: 0,
					time: 0,
				});
			}
		}
		for (name, attr, size, date, time) in self.children(&dir) {
			let name = if self.lower {
				name.to_ascii_lowercase()
			} else {
				name
			};
			each(Found {
				name: name.as_bytes(),
				attr,
				size,
				date,
				time,
			});
		}
		Ok(())
	}

	fn drives(&mut self) -> u32 {
		self.drive_mask
	}

	fn drive_kind(&mut self, letter: u8) -> DriveKind {
		let d = letter.to_ascii_uppercase();
		self.kinds
			.iter()
			.find(|k| k.0 == d)
			.map_or(DriveKind::Fixed, |k| k.1)
	}

	fn current_dir(&mut self, drive: u8, out: &mut PathBuf) -> Result<(), FsError> {
		let d = if drive == 0 {
			b'C'
		} else {
			drive.to_ascii_uppercase()
		};
		self.drive_ok(&[d])?;
		let path = self
			.cwd
			.iter()
			.find(|c| c.0 == d)
			.map(|c| c.1.clone())
			.unwrap_or_else(|| std::format!("{}:\\", d as char));
		out.clear();
		if out.push_bytes(path.as_bytes()) {
			Ok(())
		} else {
			Err(FsError::NoPath)
		}
	}

	fn exists(&mut self, path: &[u8]) -> Option<u8> {
		self.drive_ok(path).ok()?;
		let p = text(path);
		if p.len() == 3 && p.ends_with(":\\") {
			return Some(ATTR_DIR);
		}
		self.nodes.iter().find(|n| n.0 == p).map(|n| n.1)
	}

	fn mkdir(&mut self, path: &[u8]) -> Result<(), FsError> {
		self.drive_ok(path)?;
		let p = text(path);
		if self.has(&p) {
			return Err(FsError::Denied);
		}
		match p.rsplit_once('\\') {
			Some((parent, _)) if parent.len() == 2 || self.has(parent) => {
				let (date, time) = self.now;
				self.nodes.push((p, ATTR_DIR, 0, date, time));
				Ok(())
			}
			_ => Err(FsError::NoPath),
		}
	}

	fn rename(&mut self, from: &[u8], to: &[u8]) -> Result<(), FsError> {
		let (from, to) = (text(from), text(to));
		if self.has(&to) {
			return Err(FsError::Denied);
		}
		match self.nodes.iter_mut().find(|n| n.0 == from) {
			Some(n) => {
				n.0 = to;
				Ok(())
			}
			None => Err(FsError::NotFound),
		}
	}

	fn remove(&mut self, path: &[u8]) -> Result<(), FsError> {
		let p = text(path);
		let before = self.nodes.len();
		self.nodes.retain(|n| n.0 != p);
		if self.nodes.len() == before {
			Err(FsError::NotFound)
		} else {
			Ok(())
		}
	}
}
