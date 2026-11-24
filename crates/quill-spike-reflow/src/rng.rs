// A fixed-seed generator (splitmix64), so a failing property run can be repeated from the seed it prints.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// The seed the tests use unless `QUILL_SPIKE_SEED` names another.
pub const DEFAULT_SEED: u64 = 0x51_4C_49_4C_4C_20_4A_30;

/// The seed for this run.
pub fn seed() -> u64 {
	std::env::var("QUILL_SPIKE_SEED")
		.ok()
		.and_then(|v| v.parse().ok())
		.unwrap_or(DEFAULT_SEED)
}

/// A splitmix64 generator.
#[derive(Clone)]
pub struct Rng(pub u64);

impl Rng {
	/// The next 64 bits.
	pub fn next_u64(&mut self) -> u64 {
		self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
		let mut z = self.0;
		z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
		z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
		z ^ (z >> 31)
	}

	/// A number below `n` (`n` above 0).
	pub fn below(&mut self, n: u64) -> u64 {
		self.next_u64() % n
	}

	/// A number below `n` as a `u32`.
	pub fn below32(&mut self, n: u32) -> u32 {
		self.below(u64::from(n.max(1))) as u32
	}
}
