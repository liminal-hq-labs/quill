// Files through DOS (INT 21h), as the app's `Files`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use quill_app::Files;
use quill_formats::io::{Sink, Source};
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
