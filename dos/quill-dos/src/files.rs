// Files through DOS (INT 21h), as the app's `Files`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;
use quill_app::Files;
use quill_formats::io::{Sink, Source};
use quill_strike::{index_len, StrikeSource, Strikes, HEADER_LEN};
use rt::dos::{File, Mode};

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
		drop(w);
		true
	}
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
