// Files through DOS (INT 21h), as the app's `Files`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use quill_app::{Drive, Files, SaveMode, Stamp};
use quill_dlg::fs::{DriveKind, FileSystem, Found, FsError, ATTR_DIR};
use quill_dlg::name83::PathBuf;
use quill_formats::io::{Sink, Source};
use quill_strike::{index_len, StrikeSource, Strikes, HEADER_LEN};
use rt::dos::{DosError, File, Mode};

/// Reads a DOS file.
pub struct DosSource(File);

impl Source for DosSource {
	fn read(&mut self, out: &mut [u8]) -> usize {
		// A read of 32 KB or more would not fit the real-mode transfer buffer; the runtime splits it.
		self.0.read(out).unwrap_or(0)
	}
}

/// Writes a DOS file.
pub struct DosSink(File);

impl Sink for DosSink {
	fn write(&mut self, data: &[u8]) -> bool {
		self.0.write_all(data).is_ok()
	}
}

/// The DOS file system.
pub struct DosFiles;

impl Files for DosFiles {
	type R = DosSource;
	type W = DosSink;

	fn open(&mut self, name: &[u8]) -> Option<DosSource> {
		File::open(core::str::from_utf8(name).ok()?, Mode::Read).ok().map(DosSource)
	}

	fn create(&mut self, name: &[u8]) -> Option<DosSink> {
		File::open(core::str::from_utf8(name).ok()?, Mode::Create).ok().map(DosSink)
	}

	fn close(&mut self, w: DosSink) -> bool {
		w.0.finish().is_ok()
	}

	fn save_mode(&self) -> SaveMode {
		SaveMode::TempRename
	}

	fn fs(&mut self) -> Option<&mut dyn FileSystem> {
		Some(self)
	}

	fn remove(&mut self, name: &[u8]) -> bool {
		core::str::from_utf8(name).is_ok_and(|n| rt::dos::remove(n).is_ok())
	}

	// DOS refuses a name that is taken, so a replace falls to `remove` and then `rename` (the provided `replace`).
	fn rename(&mut self, from: &[u8], to: &[u8]) -> bool {
		FileSystem::rename(self, from, to).is_ok()
	}

	fn len(&mut self, name: &[u8]) -> Option<u32> {
		File::open(core::str::from_utf8(name).ok()?, Mode::Read).ok()?.len().ok()
	}

	fn now(&mut self) -> u32 {
		crate::clock::packed()
	}

	fn stamp(&mut self, name: &[u8]) -> Option<Stamp> {
		let e = rt::dos::find_first(core::str::from_utf8(name).ok()?, 0).ok()?;
		Some(Stamp {
			size: e.size,
			time: (u32::from(e.date) << 16) | u32::from(e.time),
		})
	}

	/// Where the file is: a floppy takes no recovery copy beside it, and a CD-ROM none at all (the copy goes to the central folder), and
	/// a drive letter nothing answers to is not there. Nothing here reads the media.
	fn drive(&mut self, name: &[u8]) -> Drive {
		let letter = match name {
			[l, b':', ..] => l.to_ascii_uppercase().wrapping_sub(b'A'),
			_ => rt::dos::current_drive(),
		};
		if letter < 26 && !rt::dos::drive_valid(letter) {
			return Drive::Missing;
		}
		match self.drive_kind(b'A' + letter) {
			DriveKind::Removable => Drive::Removable,
			DriveKind::CdRom => Drive::ReadOnly,
			_ => Drive::Fixed,
		}
	}

	fn make_dir(&mut self, dir: &[u8]) -> bool {
		let Ok(text) = core::str::from_utf8(dir) else {
			return false;
		};
		// A folder is named without its closing backslash, except a root.
		let bare = match text.strip_suffix('\\') {
			Some(b) if b.len() > 2 => b,
			_ => text,
		};
		if rt::dos::attributes(bare).is_ok_and(|a| a & u16::from(ATTR_DIR) != 0) {
			return true;
		}
		rt::dos::mkdir(bare).is_ok()
	}

	fn find_nth(&mut self, pattern: &[u8], n: usize, out: &mut [u8]) -> usize {
		let Ok(text) = core::str::from_utf8(pattern) else {
			return 0;
		};
		let mut seen = 0;
		let mut len = 0;
		let _ = rt::dos::find_each(text, 0, |e| {
			if seen == n {
				let name = e.name_bytes();
				len = name.len().min(out.len());
				if let (Some(d), Some(s)) = (out.get_mut(..len), name.get(..len)) {
					d.copy_from_slice(s);
				}
			}
			seen += 1;
		});
		len
	}

	/// Where a recovery copy goes when it has no place beside its document: `RECOVER\` in the program's folder, then the program's own
	/// folder.
	fn recovery_dir(&mut self, n: u8, out: &mut [u8]) -> usize {
		let Some(exe) = rt::env::program_path() else {
			return 0;
		};
		let Some(cut) = exe.as_bytes().iter().rposition(|b| *b == b'\\') else {
			return 0;
		};
		let dir = exe.as_bytes().get(..=cut).unwrap_or(&[]);
		let extra: &[u8] = match n {
			0 => b"RECOVER\\",
			1 => b"",
			_ => return 0,
		};
		if dir.len() + extra.len() > out.len() {
			return 0;
		}
		let (head, tail) = out.split_at_mut(dir.len());
		head.copy_from_slice(dir);
		if let Some(e) = tail.get_mut(..extra.len()) {
			e.copy_from_slice(extra);
		}
		dir.len() + extra.len()
	}
}

/// The failure DOS reported, as the dialog says it. Where the critical-error stub answered Fail, DOS says only that the call failed
/// (15h or 53h), so the extended error is asked for the cause; it is the next DOS call after the failure.
fn fail(e: DosError) -> FsError {
	let code = if matches!(e.0, 0x15 | 0x53) {
		match rt::dos::extended_error() {
			0 | 0x53 => e.0,
			cause => cause,
		}
	} else {
		e.0
	};
	FsError::from_dos(code)
}

fn text(path: &[u8]) -> Result<&str, FsError> {
	core::str::from_utf8(path).map_err(|_| FsError::NoPath)
}

/// The file dialog's view of the disk: INT 21h searches and the drive calls that do not read the media.
impl FileSystem for DosFiles {
	fn list(&mut self, dir: &[u8], each: &mut dyn FnMut(Found<'_>)) -> Result<(), FsError> {
		let mut pattern = Vec::with_capacity(dir.len() + 4);
		pattern.extend_from_slice(dir);
		if pattern.last() != Some(&b'\\') {
			pattern.push(b'\\');
		}
		pattern.extend_from_slice(b"*.*");
		// 10h asks for folders besides files, and so leaves out hidden, system and volume entries.
		rt::dos::find_each(text(&pattern)?, 0x10, |e| {
			each(Found {
				name: e.name_bytes(),
				attr: e.attr,
				// A folder has no size; DOSBox gives one a few bytes.
				size: if e.is_dir() { 0 } else { e.size },
				date: e.date,
				time: e.time,
			})
		})
		.map(|_| ())
		.map_err(fail)
	}

	fn drives(&mut self) -> u32 {
		rt::dos::drive_mask()
	}

	fn drive_kind(&mut self, letter: u8) -> DriveKind {
		let d = letter.to_ascii_uppercase().wrapping_sub(b'A');
		if d >= 26 {
			return DriveKind::Fixed;
		}
		if rt::dos::drive_attributes(d).is_ok_and(|w| w & 0x1000 != 0) {
			DriveKind::Network
		} else if rt::dos::drive_is_cdrom(d) {
			DriveKind::CdRom
		} else if rt::dos::drive_removable(d) == Ok(true) {
			DriveKind::Removable
		} else {
			DriveKind::Fixed
		}
	}

	fn current_dir(&mut self, drive: u8, out: &mut PathBuf) -> Result<(), FsError> {
		let index = if drive == 0 {
			rt::dos::current_drive()
		} else {
			drive.to_ascii_uppercase().wrapping_sub(b'A')
		};
		if index >= 26 {
			return Err(FsError::NoPath);
		}
		let rest = rt::dos::current_dir(index).map_err(fail)?;
		out.clear();
		let ok = out.push_bytes(&[b'A' + index, b':', b'\\']) && out.push_bytes(rest.as_bytes());
		if ok {
			Ok(())
		} else {
			Err(FsError::NoPath)
		}
	}

	fn exists(&mut self, path: &[u8]) -> Option<u8> {
		// A root has no attributes to ask for; it exists when its drive does.
		if path.len() == 3 && path.get(1) == Some(&b':') && path.get(2) == Some(&b'\\') {
			let d = path.first()?.to_ascii_uppercase().wrapping_sub(b'A');
			return (d < 26 && rt::dos::drive_valid(d)).then_some(ATTR_DIR);
		}
		rt::dos::attributes(text(path).ok()?).ok().map(|a| a as u8)
	}

	fn mkdir(&mut self, path: &[u8]) -> Result<(), FsError> {
		rt::dos::mkdir(text(path)?).map_err(fail)
	}

	fn rename(&mut self, from: &[u8], to: &[u8]) -> Result<(), FsError> {
		rt::dos::rename(text(from)?, text(to)?).map_err(fail)
	}

	fn remove(&mut self, path: &[u8]) -> Result<(), FsError> {
		rt::dos::remove(text(path)?).map_err(fail)
	}
}

/// The folders the dialog offers besides the drives: Quill's own and the samples beside it, when there are samples.
pub fn places() -> Vec<(String, String)> {
	let mut v = Vec::new();
	let Some(exe) = rt::env::program_path() else {
		return v;
	};
	let Some(cut) = exe.rfind('\\') else {
		return v;
	};
	// A root keeps its backslash (`C:\`); any other folder has none.
	let own = if cut <= 2 { &exe[..=cut] } else { &exe[..cut] };
	v.push((String::from("Quill"), String::from(own)));
	let mut samples = String::from(own);
	if !samples.ends_with('\\') {
		samples.push('\\');
	}
	samples.push_str("SAMPLES");
	if rt::dos::attributes(&samples).is_ok_and(|a| a & u16::from(ATTR_DIR) != 0) {
		v.push((String::from("Samples"), samples));
	}
	v
}

/// Bytes of `QUILL.FNT` read at a time and kept.
const PAGE: usize = 4096;
/// Pages kept at most (640 KB), so a document that uses every strike still fits; the usual page needs a few dozen.
const PAGES: usize = 160;

/// `QUILL.FNT`, read a page at a time with a seek and kept, as the painter's `StrikeSource`: a glyph is a box and a few bytes, and a
/// DOS call for each would cost more than drawing it.
pub struct FntFile {
	file: File,
	len: u32,
	pages: Vec<(u32, Box<[u8; PAGE]>)>,
	next: usize,
	/// Pages read from the file so far.
	pub loaded: u32,
}

impl FntFile {
	/// Opens the file and reads its index. The index is leaked, because the strikes and the measure refer to it for as long as the
	/// program runs. `Err` says in a few words why graphics is not possible.
	pub fn open(path: &str) -> Result<(FntFile, &'static Strikes<'static>), &'static str> {
		let mut file = File::open(path, Mode::Read).map_err(|_| "QUILL.FNT was not found")?;
		let len = file.len().map_err(|_| "QUILL.FNT could not be read")?;
		let mut head = [0u8; HEADER_LEN];
		if file.read(&mut head).map_err(|_| "QUILL.FNT could not be read")? != HEADER_LEN {
			return Err("QUILL.FNT is too short");
		}
		let n = index_len(&head).map_err(|_| "QUILL.FNT is not a font file")? as usize;
		let mut index = vec![0u8; n];
		file.seek_start(0).map_err(|_| "QUILL.FNT could not be read")?;
		if file.read(&mut index).map_err(|_| "QUILL.FNT could not be read")? != n {
			return Err("QUILL.FNT is too short");
		}
		let index: &'static [u8] = Box::leak(index.into_boxed_slice());
		let strikes = Strikes::parse(index, len).map_err(|_| "QUILL.FNT failed its checks")?;
		Ok((
			FntFile {
				file,
				len,
				pages: Vec::new(),
				next: 0,
				loaded: 0,
			},
			Box::leak(Box::new(strikes)),
		))
	}

	/// The bytes kept in memory.
	pub fn resident(&self) -> usize {
		self.pages.len() * PAGE
	}

	fn page(&mut self, no: u32) -> Option<&[u8; PAGE]> {
		if let Some(i) = self.pages.iter().position(|p| p.0 == no) {
			return self.pages.get(i).map(|p| &*p.1);
		}
		let mut data = Box::new([0u8; PAGE]);
		self.file.seek_start(no.checked_mul(PAGE as u32)?).ok()?;
		let want = (self.len - no * PAGE as u32).min(PAGE as u32) as usize;
		if self.file.read(data.get_mut(..want)?).ok()? != want {
			return None;
		}
		self.loaded += 1;
		let slot = if self.pages.len() < PAGES {
			self.pages.push((no, data));
			self.pages.len() - 1
		} else {
			let i = self.next;
			self.next = (self.next + 1) % PAGES;
			*self.pages.get_mut(i)? = (no, data);
			i
		};
		self.pages.get(slot).map(|p| &*p.1)
	}
}

impl StrikeSource for FntFile {
	fn read_at(&mut self, offset: u32, out: &mut [u8]) -> usize {
		let mut done = 0;
		while done < out.len() {
			let at = offset.saturating_add(done as u32);
			if at >= self.len {
				break;
			}
			let (no, inside) = (at / PAGE as u32, (at % PAGE as u32) as usize);
			let valid = (self.len - no * PAGE as u32).min(PAGE as u32) as usize;
			let Some(page) = self.page(no) else {
				break;
			};
			let n = (out.len() - done).min(valid.saturating_sub(inside));
			if n == 0 {
				break;
			}
			if let (Some(d), Some(s)) = (out.get_mut(done..done + n), page.get(inside..inside + n)) {
				d.copy_from_slice(s);
			}
			done += n;
		}
		done
	}
}
