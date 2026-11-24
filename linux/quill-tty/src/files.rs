// The editor's files over `std::fs`: names, the settings folder and a save that cannot leave half a file.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The editor names files by Windows-1252 bytes and writes `QUILL.INI` and `QUILL.UDC` by bare name. Here a name is turned into
// UTF-8 and resolved like this:
//
// - `QUILL.INI` and `QUILL.UDC` live in `$XDG_CONFIG_HOME/quill` (`~/.config/quill`).
// - A bare dictionary name (`EN_CA.DIC`) is searched for in the current folder, beside the program, in
//   `$XDG_DATA_HOME/quill` and in the folders of `$XDG_DATA_DIRS`, each with `/quill` added.
// - Any other name is a path as typed, relative to the current folder. A name that does not exist is retried without regard to case, so `LETTER.RTF` finds
//   `letter.rtf`.
//
// `create` writes `.name.~tmp` beside the target, and `close` flushes, syncs and renames it over the target, so a failed save
// leaves the old file whole.

use std::fs::{self, File};
use std::io::{Read, Write};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use quill_app::Files;
use quill_formats::io::{Sink, Source};

use crate::cp1252;

/// Where the files are.
#[derive(Clone, Debug)]
pub struct Dirs {
	/// The folder relative names are taken from (the current folder).
	pub base: PathBuf,
	/// The settings folder (`quill` under `$XDG_CONFIG_HOME`).
	pub config: PathBuf,
	/// Folders searched for a bare dictionary name, after the current folder.
	pub data: Vec<PathBuf>,
}

impl Dirs {
	/// The folders for this user, from the environment.
	pub fn from_env() -> Dirs {
		let home = std::env::var_os("HOME").map(PathBuf::from);
		let dir = |var: &str, fallback: &[&str]| -> Option<PathBuf> {
			std::env::var_os(var)
				.map(PathBuf::from)
				.filter(|p| p.is_absolute())
				.or_else(|| {
					home.as_ref()
						.map(|h| fallback.iter().fold(h.clone(), |p, s| p.join(s)))
				})
		};
		let mut data = Vec::new();
		if let Some(exe) = std::env::current_exe()
			.ok()
			.and_then(|e| e.parent().map(Path::to_path_buf))
		{
			data.push(exe);
		}
		data.extend(dir("XDG_DATA_HOME", &[".local", "share"]).map(|p| p.join("quill")));
		let dirs =
			std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
		data.extend(
			dirs.split(':')
				.filter(|d| d.starts_with('/'))
				.map(|d| Path::new(d).join("quill")),
		);
		Dirs {
			base: PathBuf::from("."),
			config: dir("XDG_CONFIG_HOME", &[".config"])
				.unwrap_or_else(|| PathBuf::from("."))
				.join("quill"),
			data,
		}
	}
}

/// The files of the terminal client.
pub struct LinuxFiles {
	dirs: Dirs,
}

impl LinuxFiles {
	/// Files with the folders given.
	pub fn new(dirs: Dirs) -> LinuxFiles {
		LinuxFiles { dirs }
	}

	/// The folders in use.
	pub fn dirs(&self) -> &Dirs {
		&self.dirs
	}

	/// The path to read for `name`, if there is such a file.
	fn find(&self, name: &[u8]) -> Option<PathBuf> {
		let text = cp1252::ansi_to_string(name);
		if is_config(&text) {
			return exists_any_case(&self.dirs.config.join(&text));
		}
		if is_bare_dictionary(&text) {
			return std::iter::once(&self.dirs.base)
				.chain(self.dirs.data.iter())
				.find_map(|d| exists_any_case(&d.join(&text)));
		}
		exists_any_case(&self.dirs.base.join(&text))
	}
}

fn is_config(name: &str) -> bool {
	name.eq_ignore_ascii_case("QUILL.INI") || name.eq_ignore_ascii_case("QUILL.UDC")
}

fn is_bare_dictionary(name: &str) -> bool {
	!name.contains('/') && name.len() > 4 && name[name.len() - 4..].eq_ignore_ascii_case(".DIC")
}

/// `path` if it exists, else a file of the same name in the same folder that differs only in case.
fn exists_any_case(path: &Path) -> Option<PathBuf> {
	if path.is_file() {
		return Some(path.to_path_buf());
	}
	let want = path.file_name()?;
	let dir = match path.parent() {
		Some(p) if !p.as_os_str().is_empty() => p,
		_ => Path::new("."),
	};
	let mut found: Vec<PathBuf> = fs::read_dir(dir)
		.ok()?
		.filter_map(Result::ok)
		.filter(|e| {
			e.file_name()
				.as_bytes()
				.eq_ignore_ascii_case(want.as_bytes())
		})
		.map(|e| e.path())
		.filter(|p| p.is_file())
		.collect();
	found.sort();
	found.into_iter().next()
}

/// A file being read.
pub struct FileSource(File);

impl Source for FileSource {
	fn read(&mut self, out: &mut [u8]) -> usize {
		let mut n = 0;
		while n < out.len() {
			match self.0.read(&mut out[n..]) {
				Ok(0) | Err(_) => break,
				Ok(k) => n += k,
			}
		}
		n
	}
}

/// A file being written to its temporary name.
pub struct FileSink {
	file: Option<File>,
	tmp: PathBuf,
	dest: PathBuf,
	failed: bool,
}

impl Sink for FileSink {
	fn write(&mut self, data: &[u8]) -> bool {
		if self.failed {
			return false;
		}
		let ok = self
			.file
			.as_mut()
			.is_some_and(|f| f.write_all(data).is_ok());
		self.failed |= !ok;
		ok
	}
}

impl Drop for FileSink {
	fn drop(&mut self) {
		// Never finished: take the half-written file away.
		if self.file.take().is_some() {
			let _ = fs::remove_file(&self.tmp);
		}
	}
}

impl Files for LinuxFiles {
	type R = FileSource;
	type W = FileSink;

	fn open(&mut self, name: &[u8]) -> Option<FileSource> {
		File::open(self.find(name)?).ok().map(FileSource)
	}

	fn create(&mut self, name: &[u8]) -> Option<FileSink> {
		let text = cp1252::ansi_to_string(name);
		let mut dest = if is_config(&text) {
			fs::create_dir_all(&self.dirs.config).ok()?;
			self.dirs.config.join(&text)
		} else {
			self.dirs.base.join(&text)
		};
		// Save through a link to the file it points at, and keep the name of a file that is already there in another case.
		if let Some(existing) = exists_any_case(&dest) {
			dest = fs::canonicalize(existing).ok()?;
		}
		let dir = match dest.parent() {
			Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
			_ => PathBuf::from("."),
		};
		let mut tmp_name = std::ffi::OsString::from(".");
		tmp_name.push(dest.file_name()?);
		tmp_name.push(".~tmp");
		let tmp = dir.join(tmp_name);
		let file = File::create(&tmp).ok()?;
		Some(FileSink {
			file: Some(file),
			tmp,
			dest,
			failed: false,
		})
	}

	fn close(&mut self, mut w: FileSink) -> bool {
		let Some(file) = w.file.take() else {
			return false;
		};
		let done = !w.failed && file.sync_all().is_ok() && {
			if let Ok(meta) = fs::metadata(&w.dest) {
				let _ = fs::set_permissions(&w.tmp, meta.permissions());
			}
			drop(file);
			fs::rename(&w.tmp, &w.dest).is_ok()
		};
		if done {
			if let Some(dir) = w
				.dest
				.parent()
				.filter(|p| !p.as_os_str().is_empty())
				.or(Some(Path::new(".")))
			{
				if let Ok(d) = File::open(dir) {
					let _ = d.sync_all();
				}
			}
		} else {
			let _ = fs::remove_file(&w.tmp);
		}
		done
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn files(root: &Path) -> LinuxFiles {
		LinuxFiles::new(Dirs {
			base: ".".into(),
			config: root.join("cfg/quill"),
			data: vec![root.join("data/quill")],
		})
	}

	fn read_all(f: &mut LinuxFiles, name: &[u8]) -> Option<Vec<u8>> {
		let mut r = f.open(name)?;
		let mut buf = vec![0u8; 1 << 16];
		let n = r.read(&mut buf);
		buf.truncate(n);
		Some(buf)
	}

	#[test]
	fn a_save_replaces_the_file_whole_and_leaves_no_temporary() {
		let t = tempfile::tempdir().unwrap();
		let path = t.path().join("a.qld");
		fs::write(&path, b"old").unwrap();
		let mut f = files(t.path());
		let name = path.to_str().unwrap().as_bytes();
		let mut w = f.create(name).unwrap();
		assert!(t.path().join(".a.qld.~tmp").exists());
		assert_eq!(
			fs::read(&path).unwrap(),
			b"old",
			"the old file stays until the rename"
		);
		assert!(w.write(b"new bytes"));
		assert!(f.close(w));
		assert_eq!(fs::read(&path).unwrap(), b"new bytes");
		assert!(!t.path().join(".a.qld.~tmp").exists());
	}

	#[test]
	fn a_save_that_is_dropped_keeps_the_old_file() {
		let t = tempfile::tempdir().unwrap();
		let path = t.path().join("a.qld");
		fs::write(&path, b"old").unwrap();
		let mut f = files(t.path());
		let mut w = f.create(path.to_str().unwrap().as_bytes()).unwrap();
		assert!(w.write(b"half"));
		drop(w);
		assert_eq!(fs::read(&path).unwrap(), b"old");
		assert!(!t.path().join(".a.qld.~tmp").exists());
	}

	#[test]
	fn names_are_found_without_regard_to_case_and_saved_over_the_same_file() {
		let t = tempfile::tempdir().unwrap();
		fs::write(t.path().join("letter.rtf"), b"x").unwrap();
		let mut f = files(t.path());
		let upper = t.path().join("LETTER.RTF");
		let name = upper.to_str().unwrap().as_bytes();
		assert_eq!(read_all(&mut f, name).as_deref(), Some(&b"x"[..]));
		let mut w = f.create(name).unwrap();
		w.write(b"y");
		assert!(f.close(w));
		assert_eq!(fs::read(t.path().join("letter.rtf")).unwrap(), b"y");
		assert!(!t.path().join("LETTER.RTF").exists());
	}

	#[test]
	fn settings_go_to_the_config_folder_and_dictionaries_come_from_the_data_folders() {
		let t = tempfile::tempdir().unwrap();
		let mut f = files(t.path());
		assert!(read_all(&mut f, b"QUILL.INI").is_none());
		let mut w = f.create(b"QUILL.INI").unwrap();
		w.write(b"[Quill]\n");
		assert!(f.close(w));
		assert_eq!(
			fs::read(t.path().join("cfg/quill/QUILL.INI")).unwrap(),
			b"[Quill]\n"
		);
		fs::create_dir_all(t.path().join("data/quill")).unwrap();
		fs::write(t.path().join("data/quill/en_ca.dic"), b"dic").unwrap();
		assert_eq!(read_all(&mut f, b"EN_CA.DIC").as_deref(), Some(&b"dic"[..]));
	}

	#[test]
	fn windows_1252_names_become_utf8_paths() {
		let t = tempfile::tempdir().unwrap();
		fs::write(t.path().join("café.rtf"), b"z").unwrap();
		let mut f = files(t.path());
		let mut name = t.path().to_str().unwrap().as_bytes().to_vec();
		name.extend_from_slice(b"/caf\xE9.rtf");
		assert_eq!(read_all(&mut f, &name).as_deref(), Some(&b"z"[..]));
	}
}
