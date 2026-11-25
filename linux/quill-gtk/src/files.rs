// The files of the viewer: the terminal client's `LinuxFiles`, and a way to open a path the editor's names cannot spell.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The editor names a file by Windows-1252 bytes, and a build with long names keeps up to 260 of them. A path that fits goes in
// whole, as an absolute path, so the recent list opens it again in the next session. A path with a character Windows-1252 lacks,
// or one too long, is given a short stand-in name for this session and `open` resolves it back; it cannot be read again from the
// recent list in a later session.

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use quill_app::Files;
use quill_formats::io::Source;
use quill_tty::cp1252;
use quill_tty::files::{Dirs, FileSink, FileSource, LinuxFiles};

use crate::names::lossy_ansi;

/// The longest name the editor keeps with long names on.
pub const NAME_MAX: usize = 260;

/// A file being read.
pub enum GuiSource {
	/// Found by the terminal client's rules.
	Linux(FileSource),
	/// Opened from a path that has a stand-in name.
	Raw(File),
}

impl Source for GuiSource {
	fn read(&mut self, out: &mut [u8]) -> usize {
		match self {
			GuiSource::Linux(s) => s.read(out),
			GuiSource::Raw(f) => {
				let mut n = 0;
				while n < out.len() {
					match f.read(&mut out[n..]) {
						Ok(0) | Err(_) => break,
						Ok(k) => n += k,
					}
				}
				n
			}
		}
	}
}

/// The editor's files for the GTK client.
pub struct GuiFiles {
	inner: LinuxFiles,
	aliases: Vec<(Vec<u8>, PathBuf)>,
}

impl GuiFiles {
	/// Files over the given folders.
	pub fn new(dirs: Dirs) -> GuiFiles {
		GuiFiles {
			inner: LinuxFiles::new(dirs),
			aliases: Vec::new(),
		}
	}

	/// Files in this user's folders.
	pub fn from_env() -> GuiFiles {
		GuiFiles::new(Dirs::from_env())
	}

	/// The folders in use.
	pub fn dirs(&self) -> &Dirs {
		self.inner.dirs()
	}

	/// The name to give the editor for `path`.
	pub fn name_for(&mut self, path: &Path) -> Vec<u8> {
		let abs = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
		if let Some(name) = abs.to_str().and_then(cp1252::name_to_ansi) {
			if name.len() <= NAME_MAX && !name.contains(&0) {
				return name;
			}
		}
		if let Some((k, _)) = self.aliases.iter().find(|(_, p)| *p == abs) {
			return k.clone();
		}
		let stem = abs
			.file_name()
			.map(|n| n.to_string_lossy().into_owned())
			.unwrap_or_default();
		let base = lossy_ansi(&stem, 48);
		let mut key = base.clone();
		let mut n = 1;
		while self.aliases.iter().any(|(k, _)| *k == key) {
			n += 1;
			key = base.clone();
			key.extend_from_slice(format!("~{n}").as_bytes());
		}
		self.aliases.push((key.clone(), abs));
		key
	}
}

impl Files for GuiFiles {
	type R = GuiSource;
	type W = FileSink;

	fn open(&mut self, name: &[u8]) -> Option<GuiSource> {
		if let Some((_, p)) = self.aliases.iter().find(|(k, _)| k == name) {
			return File::open(p).ok().map(GuiSource::Raw);
		}
		self.inner.open(name).map(GuiSource::Linux)
	}

	fn create(&mut self, name: &[u8]) -> Option<FileSink> {
		if self.aliases.iter().any(|(k, _)| k == name) {
			return None;
		}
		self.inner.create(name)
	}

	fn close(&mut self, w: FileSink) -> bool {
		self.inner.close(w)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn files(dir: &Path) -> GuiFiles {
		GuiFiles::new(Dirs {
			base: dir.to_path_buf(),
			config: dir.join("config"),
			data: Vec::new(),
		})
	}

	#[test]
	fn a_path_windows_1252_can_spell_goes_in_whole() {
		let d = tempfile::tempdir().unwrap();
		let mut f = files(d.path());
		let p = d.path().join("Caf\u{e9} notes.txt");
		std::fs::write(&p, b"hello").unwrap();
		let name = f.name_for(&p);
		assert!(name.starts_with(b"/") && name.ends_with(b"Caf\xE9 notes.txt"));
		let mut src = f.open(&name).unwrap();
		let mut buf = [0u8; 8];
		assert_eq!(src.read(&mut buf), 5);
		assert_eq!(&buf[..5], b"hello");
	}

	#[test]
	fn a_path_it_cannot_spell_gets_a_stand_in_that_opens_it_this_session() {
		let d = tempfile::tempdir().unwrap();
		let mut f = files(d.path());
		let p = d.path().join("\u{4e2d}\u{6587}.txt");
		std::fs::write(&p, b"hi").unwrap();
		let q = d.path().join("\u{65e5}\u{672c}.txt");
		std::fs::write(&q, b"yo").unwrap();
		let a = f.name_for(&p);
		let b = f.name_for(&q);
		assert_eq!(a, b"??.txt");
		assert_eq!(b, b"??.txt~2");
		assert_eq!(f.name_for(&p), a);
		let mut buf = [0u8; 4];
		assert_eq!(f.open(&b).unwrap().read(&mut buf), 2);
		assert_eq!(&buf[..2], b"yo");
		assert!(f.create(&a).is_none());
	}

	#[test]
	fn a_missing_file_is_not_found() {
		let d = tempfile::tempdir().unwrap();
		let mut f = files(d.path());
		let name = f.name_for(&d.path().join("none.rtf"));
		assert!(f.open(&name).is_none());
	}
}
