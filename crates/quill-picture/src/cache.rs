// The display cache: which pictures, at which sizes, are held in the slots the caller lends.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// What a cached picture is known by: its index in the document's pool, a fingerprint that tells it from another picture that took
/// that index, and the size it was drawn at.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Key {
	pub idx: u16,
	pub fp: u32,
	pub w: i32,
	pub h: i32,
}

/// The memory behind the cache's slots, lent by the caller: `N` places, numbered from 0, each holding whatever the shell draws from.
pub trait Slots {
	/// Slot `slot` is about to be reused for another picture; give back what it holds.
	fn release(&mut self, slot: usize);
}

/// The keys of up to `N` pictures drawn lately. The oldest entry is the one replaced, and a hit does not renew an entry.
pub struct Cache<const N: usize> {
	keys: [Option<Key>; N],
	next: usize,
}

impl<const N: usize> Default for Cache<N> {
	fn default() -> Self {
		Self::new()
	}
}

impl<const N: usize> Cache<N> {
	pub const fn new() -> Self {
		Cache {
			keys: [None; N],
			next: 0,
		}
	}

	/// The slot holding the picture known by `key`, if it is cached.
	pub fn find(&self, key: &Key) -> Option<usize> {
		self.keys.iter().position(|k| k.as_ref() == Some(key))
	}

	/// The slot holding `key`'s picture, filled by `fill(slots, slot)` when it is not cached: the oldest slot is released first and
	/// then handed to `fill`, which leaves in it whatever draws the picture (or nothing, for a picture that cannot be drawn: it is
	/// remembered all the same, so it is not tried again at every paint).
	pub fn get<S: Slots>(
		&mut self,
		slots: &mut S,
		key: Key,
		fill: impl FnOnce(&mut S, usize),
	) -> usize {
		if let Some(slot) = self.find(&key) {
			return slot;
		}
		let slot = self.next;
		if let Some(entry) = self.keys.get_mut(slot) {
			if entry.is_some() {
				slots.release(slot);
			}
			*entry = Some(key);
		}
		self.next = (slot + 1) % N.max(1);
		fill(slots, slot);
		slot
	}

	/// Forgets every entry, releasing each slot that holds one.
	pub fn clear<S: Slots>(&mut self, slots: &mut S) {
		for (slot, entry) in self.keys.iter_mut().enumerate() {
			if entry.take().is_some() {
				slots.release(slot);
			}
		}
		self.next = 0;
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	struct Log {
		released: [u8; 8],
		n: usize,
	}

	impl Slots for Log {
		fn release(&mut self, slot: usize) {
			if let Some(r) = self.released.get_mut(self.n) {
				*r = slot as u8;
			}
			self.n += 1;
		}
	}

	fn key(idx: u16, w: i32) -> Key {
		Key {
			idx,
			fp: 7,
			w,
			h: 10,
		}
	}

	#[test]
	fn a_hit_does_not_fill_again() {
		let mut cache = Cache::<3>::new();
		let mut log = Log {
			released: [0; 8],
			n: 0,
		};
		let mut fills = 0;
		for _ in 0..3 {
			let slot = cache.get(&mut log, key(1, 100), |_, _| fills += 1);
			assert_eq!(slot, 0);
		}
		assert_eq!(fills, 1);
		assert_eq!(log.n, 0);
	}

	#[test]
	fn size_and_fingerprint_are_part_of_the_key() {
		let mut cache = Cache::<3>::new();
		let mut log = Log {
			released: [0; 8],
			n: 0,
		};
		assert_eq!(cache.get(&mut log, key(1, 100), |_, _| {}), 0);
		assert_eq!(cache.get(&mut log, key(1, 120), |_, _| {}), 1);
		let other = Key {
			fp: 8,
			..key(1, 100)
		};
		assert_eq!(cache.get(&mut log, other, |_, _| {}), 2);
		assert_eq!(cache.find(&key(1, 100)), Some(0));
	}

	#[test]
	fn the_oldest_entry_is_replaced_whether_or_not_it_was_hit() {
		let mut cache = Cache::<3>::new();
		let mut log = Log {
			released: [0; 8],
			n: 0,
		};
		for i in 0..3 {
			cache.get(&mut log, key(i, 100), |_, _| {});
		}
		// A hit on the oldest does not save it.
		assert_eq!(cache.get(&mut log, key(0, 100), |_, _| {}), 0);
		assert_eq!(cache.get(&mut log, key(3, 100), |_, _| {}), 0);
		assert_eq!(cache.get(&mut log, key(4, 100), |_, _| {}), 1);
		assert_eq!(log.n, 2);
		assert_eq!(log.released.get(..2), Some(&[0u8, 1][..]));
		assert_eq!(cache.find(&key(0, 100)), None);
		assert_eq!(cache.find(&key(2, 100)), Some(2));
	}

	#[test]
	fn clear_releases_what_is_held() {
		let mut cache = Cache::<3>::new();
		let mut log = Log {
			released: [0; 8],
			n: 0,
		};
		cache.get(&mut log, key(1, 100), |_, _| {});
		cache.get(&mut log, key(2, 100), |_, _| {});
		cache.clear(&mut log);
		assert_eq!(log.n, 2);
		assert_eq!(cache.find(&key(1, 100)), None);
		assert_eq!(cache.get(&mut log, key(5, 100), |_, _| {}), 0);
	}
}
