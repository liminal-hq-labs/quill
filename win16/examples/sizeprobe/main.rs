// A size probe: links the editor core for the Win16 target to measure its code, with a tiny arena for a Store. It is not run.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
#![no_std]
#![no_main]

use quill_app::{App, Files, Key};
use quill_core::store::{ChunkId, Store};
use quill_formats::io::{Sink, Source};
use quill_grid::Grid;
use w16 as _;

static mut ARENA: [u8; 6000] = [0; 6000];

struct Arena {
	used: [bool; 12],
}

impl Store for Arena {
	fn alloc(&mut self, len: u16) -> Option<ChunkId> {
		let i = self.used.iter().position(|u| !*u)?;
		if usize::from(len) > 500 {
			return None;
		}
		self.used[i] = true;
		Some(ChunkId(i as u16))
	}
	fn free(&mut self, id: ChunkId) {
		self.used[usize::from(id.0) % 12] = false;
	}
	fn read(&self, id: ChunkId, offset: u16, out: &mut [u8]) {
		let base = usize::from(id.0) * 500 + usize::from(offset);
		let a = unsafe { &*core::ptr::addr_of!(ARENA) };
		if let Some(s) = a.get(base..base + out.len()) {
			out.copy_from_slice(s);
		}
	}
	fn write(&mut self, id: ChunkId, offset: u16, data: &[u8]) {
		let base = usize::from(id.0) * 500 + usize::from(offset);
		let a = unsafe { &mut *core::ptr::addr_of_mut!(ARENA) };
		if let Some(s) = a.get_mut(base..base + data.len()) {
			s.copy_from_slice(data);
		}
	}
	fn copy(&mut self, from: (ChunkId, u16), to: (ChunkId, u16), len: u16) {
		let mut b = [0u8; 64];
		let mut done = 0usize;
		while done < usize::from(len) {
			let n = (usize::from(len) - done).min(64);
			self.read(from.0, from.1 + done as u16, &mut b[..n]);
			self.write(to.0, to.1 + done as u16, &b[..n]);
			done += n;
		}
	}
}

struct NoSource;
impl Source for NoSource {
	fn read(&mut self, _: &mut [u8]) -> usize {
		0
	}
}
struct NoSink;
impl Sink for NoSink {
	fn write(&mut self, _: &[u8]) -> bool {
		false
	}
}
struct NoFiles;
impl Files for NoFiles {
	type R = NoSource;
	type W = NoSink;
	fn open(&mut self, _: &[u8]) -> Option<NoSource> {
		None
	}
	fn create(&mut self, _: &[u8]) -> Option<NoSink> {
		None
	}
	fn close(&mut self, _: NoSink) -> bool {
		true
	}
}

#[unsafe(no_mangle)]
pub extern "C" fn w16_main(_hinst: u32, _hprev: u32, show: u32) -> u32 {
	let mut s = Arena { used: [false; 12] };
	let mut f = NoFiles;
	let mut app = App::new();
	let _ = app.reset(&mut s);
	app.key(&mut s, &mut f, Key::Char(show as u8));
	let mut g = Grid::<25>::new(0);
	quill_app::draw(&app, &s, &mut g);
	g.get(1, 1).map_or(0, |c| u32::from(c.ch))
}
