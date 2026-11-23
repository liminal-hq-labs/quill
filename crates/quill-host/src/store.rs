// HostStore: a Store over heap blocks that keeps the 32 KB chunk cap, so chunk-edge bugs show up on the host.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use quill_core::store::{ChunkId, Store, MAX_CHUNK};

/// Chunks on the host heap, with an optional limit on live chunks to test out-of-memory paths.
#[derive(Default)]
pub struct HostStore {
	chunks: Vec<Option<Vec<u8>>>,
	free: Vec<u16>,
	live: usize,
	limit: Option<usize>,
}

impl HostStore {
	/// A store with no limit.
	pub fn new() -> Self {
		Self::default()
	}

	/// A store that refuses to hold more than `max_chunks` live chunks.
	pub fn with_limit(max_chunks: usize) -> Self {
		HostStore {
			limit: Some(max_chunks),
			..Self::default()
		}
	}

	/// Live chunks, for leak checks.
	pub fn live_chunks(&self) -> usize {
		self.live
	}

	/// Changes the live-chunk limit.
	pub fn set_limit(&mut self, limit: Option<usize>) {
		self.limit = limit;
	}
}

impl Store for HostStore {
	fn alloc(&mut self, len: u16) -> Option<ChunkId> {
		if usize::from(len) > MAX_CHUNK {
			return None;
		}
		if self.limit.is_some_and(|l| self.live >= l) {
			return None;
		}
		let block = vec![0u8; usize::from(len)];
		self.live += 1;
		if let Some(i) = self.free.pop() {
			self.chunks[usize::from(i)] = Some(block);
			return Some(ChunkId(i));
		}
		let i = u16::try_from(self.chunks.len()).ok()?;
		self.chunks.push(Some(block));
		Some(ChunkId(i))
	}

	fn free(&mut self, id: ChunkId) {
		if let Some(slot) = self.chunks.get_mut(usize::from(id.0)) {
			if slot.take().is_some() {
				self.live -= 1;
				self.free.push(id.0);
			}
		}
	}

	fn read(&self, id: ChunkId, offset: u16, out: &mut [u8]) {
		out.fill(0);
		if let Some(Some(b)) = self.chunks.get(usize::from(id.0)) {
			let start = usize::from(offset);
			if let Some(src) = b.get(start..) {
				let n = src.len().min(out.len());
				out[..n].copy_from_slice(&src[..n]);
			}
		}
	}

	fn write(&mut self, id: ChunkId, offset: u16, data: &[u8]) {
		if let Some(Some(b)) = self.chunks.get_mut(usize::from(id.0)) {
			let start = usize::from(offset);
			if let Some(dst) = b.get_mut(start..) {
				let n = dst.len().min(data.len());
				dst[..n].copy_from_slice(&data[..n]);
			}
		}
	}

	fn copy(&mut self, from: (ChunkId, u16), to: (ChunkId, u16), len: u16) {
		let mut tmp = vec![0u8; usize::from(len)];
		self.read(from.0, from.1, &mut tmp);
		self.write(to.0, to.1, &tmp);
	}
}
