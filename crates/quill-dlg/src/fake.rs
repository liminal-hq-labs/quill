// An in-memory file system tree for tests and for the shells' host builds: drives, folders and files, with the quirks of a DOS
// listing that the dialog must cope with.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// This module uses `std`, so it exists only in tests and behind the `fake` feature; the dialog itself never allocates.

use crate::fs::{FileSystem, FsError, ATTR_DIR};
use crate::name83::PathBuf;
use std::string::String;
use std::vec::Vec;

/// A tree of folders and files on up to 26 drives.
#[derive(Clone, Debug, Default)]
pub struct FakeFs {
	/// (full upper-case path, attributes); drive roots are not listed
	nodes: Vec<(String, u8)>,
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

	/// Adds a folder (and any missing parents).
	pub fn add_dir(&mut self, path: &str) {
		let path = path.to_ascii_uppercase();
		if let Some((parent, _)) = path.rsplit_once('\\') {
			if parent.len() > 2 && !self.has(parent) {
				self.add_dir(parent);
			}
		}
		if !self.has(&path) {
			self.nodes.push((path, ATTR_DIR));
		}
	}

	/// Adds a file with attributes `attr` (and any missing parents).
	pub fn add_file(&mut self, path: &str, attr: u8) {
		let path = path.to_ascii_uppercase();
		if let Some((parent, _)) = path.rsplit_once('\\') {
			if parent.len() > 2 {
				self.add_dir(parent);
			}
		}
		if let Some(n) = self.nodes.iter_mut().find(|n| n.0 == path) {
			n.1 = attr;
		} else {
			self.nodes.push((path, attr));
		}
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

	/// The entries directly in `dir`, as (name, attributes).
	pub fn children(&self, dir: &str) -> Vec<(String, u8)> {
		let mut v: Vec<(String, u8)> = self
			.nodes
			.iter()
			.filter_map(|(p, a)| {
				let (parent, name) = p.rsplit_once('\\')?;
				let parent = if parent.len() == 2 {
					std::format!("{parent}\\")
				} else {
					parent.into()
				};
				(parent == dir).then(|| (name.into(), *a))
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
	fn list(&mut self, dir: &[u8], each: &mut dyn FnMut(&[u8], u8)) -> Result<(), FsError> {
		self.lists += 1;
		self.drive_ok(dir)?;
		let dir = text(dir);
		let is_root = dir.len() == 3;
		if !is_root && !self.nodes.iter().any(|n| n.0 == dir && n.1 & ATTR_DIR != 0) {
			return Err(FsError::NoPath);
		}
		if self.dots && !is_root {
			each(b".", ATTR_DIR);
			each(b"..", ATTR_DIR);
		}
		for (name, attr) in self.children(&dir) {
			let name = if self.lower {
				name.to_ascii_lowercase()
			} else {
				name
			};
			each(name.as_bytes(), attr);
		}
		Ok(())
	}

	fn drives(&mut self) -> u32 {
		self.drive_mask
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
