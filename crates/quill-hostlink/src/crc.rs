// CRC-32 (IEEE 802.3, the one zlib and PNG use), table built at compile time.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

const fn make_table() -> [u32; 256] {
	let mut table = [0u32; 256];
	let mut n = 0usize;
	while n < 256 {
		let mut c = n as u32;
		let mut k = 0;
		while k < 8 {
			c = if c & 1 != 0 {
				0xEDB8_8320 ^ (c >> 1)
			} else {
				c >> 1
			};
			k += 1;
		}
		// A constant evaluation: `n` is below 256, so the store cannot miss.
		#[allow(clippy::indexing_slicing)]
		{
			table[n] = c;
		}
		n += 1;
	}
	table
}

static TABLE: [u32; 256] = make_table();

/// A running CRC-32: `update` as bytes arrive, `finish` for the value.
#[derive(Clone, Copy, Debug)]
pub struct Crc32(u32);

impl Default for Crc32 {
	fn default() -> Self {
		Self::new()
	}
}

impl Crc32 {
	pub const fn new() -> Self {
		Crc32(0xFFFF_FFFF)
	}

	pub fn update(&mut self, bytes: &[u8]) {
		let mut c = self.0;
		for &b in bytes {
			let index = ((c ^ u32::from(b)) & 0xFF) as usize;
			c = TABLE.get(index).copied().unwrap_or(0) ^ (c >> 8);
		}
		self.0 = c;
	}

	pub fn finish(self) -> u32 {
		self.0 ^ 0xFFFF_FFFF
	}
}

/// The CRC-32 of one slice.
pub fn crc32(bytes: &[u8]) -> u32 {
	let mut crc = Crc32::new();
	crc.update(bytes);
	crc.finish()
}
