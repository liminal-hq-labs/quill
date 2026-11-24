// Pictures under memory pressure: a picture that does not fit is refused with a message and the document is left as it was.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use quill_app::{App, Command};
use quill_core::store::{ChunkId, ReadStore, Store};
use quill_heap::HeapStore;
use quill_host::script::MemFiles;

/// A store that stops handing out chunks once a budget of bytes is used, as a 4 MB machine does.
struct Limited {
	inner: HeapStore,
	budget: usize,
	used: usize,
	sizes: std::collections::HashMap<u16, usize>,
}

impl ReadStore for Limited {
	fn read(&self, id: ChunkId, offset: u16, out: &mut [u8]) {
		self.inner.read(id, offset, out);
	}
}

impl Store for Limited {
	fn alloc(&mut self, len: u16) -> Option<ChunkId> {
		if self.used + usize::from(len) > self.budget {
			return None;
		}
		let id = self.inner.alloc(len)?;
		self.used += usize::from(len);
		self.sizes.insert(id.0, usize::from(len));
		Some(id)
	}
	fn free(&mut self, id: ChunkId) {
		self.used -= self.sizes.remove(&id.0).unwrap_or(0);
		self.inner.free(id);
	}
	fn write(&mut self, id: ChunkId, offset: u16, data: &[u8]) {
		self.inner.write(id, offset, data);
	}
	fn copy(&mut self, from: (ChunkId, u16), to: (ChunkId, u16), len: u16) {
		self.inner.copy(from, to, len);
	}
}

/// A 24-bit BMP of `w` by `h` pixels in a gradient.
fn bmp(w: u32, h: u32) -> Vec<u8> {
	let stride = (w * 3).div_ceil(4) * 4;
	let mut b = b"BM".to_vec();
	b.extend_from_slice(&(54 + stride * h).to_le_bytes());
	b.extend_from_slice(&[0; 4]);
	b.extend_from_slice(&54u32.to_le_bytes());
	b.extend_from_slice(&40u32.to_le_bytes());
	b.extend_from_slice(&w.to_le_bytes());
	b.extend_from_slice(&h.to_le_bytes());
	b.extend_from_slice(&[1, 0, 24, 0]);
	b.extend_from_slice(&[0; 24]);
	for y in 0..h {
		for x in 0..stride {
			b.push((x + y) as u8);
		}
	}
	b
}

fn save(app: &mut App, s: &mut Limited, files: &mut MemFiles, name: &[u8]) -> Vec<u8> {
	assert!(app.save_as(s, files, name));
	files.files[std::str::from_utf8(name).unwrap()].clone()
}

#[test]
fn an_oversized_picture_is_refused_and_the_document_saves_unchanged() {
	let mut s = Limited {
		inner: HeapStore::new(),
		budget: 1_500_000,
		used: 0,
		sizes: Default::default(),
	};
	let mut files = MemFiles::default();
	let mut app = App::new();
	app.reset(&mut s).unwrap();
	files.files.insert("SMALL.BMP".into(), bmp(100, 100));
	files.files.insert("BIG.BMP".into(), bmp(1000, 1000)); // 3 MB
	files.files.insert("HUGE.BMP".into(), bmp(1500, 1000)); // over the 4 MB limit for one picture
	let before = save(&mut app, &mut s, &mut files, b"A.QLD");
	app.insert_picture(&mut s, &mut files, b"SMALL.BMP")
		.unwrap();
	let with_one = save(&mut app, &mut s, &mut files, b"B.QLD");
	assert!(
		with_one.len() > before.len() + 30_000,
		"the picture's bytes are in the file"
	);
	for name in [&b"BIG.BMP"[..], b"HUGE.BMP"] {
		let used = s.used;
		assert!(app.insert_picture(&mut s, &mut files, name).is_err());
		assert_eq!(s.used, used, "the pool gives back what it took");
		assert_eq!(
			save(&mut app, &mut s, &mut files, b"C.QLD"),
			with_one,
			"{name:?}"
		);
	}
	// Through the dialog the user sees a message, and the document is still saved unchanged.
	let _ = Command::Picture;
	// What fits still goes in afterwards.
	app.insert_picture(&mut s, &mut files, b"SMALL.BMP")
		.unwrap();
	assert!(save(&mut app, &mut s, &mut files, b"D.QLD").len() > with_one.len());
}
