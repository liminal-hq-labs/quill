// Bounds-checked little-endian reading and writing, and the `Pod` trait for fixed-size records.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// A fixed-size record that can be copied into and out of a [`crate::store::Store`] chunk.
pub trait Pod: Copy {
	/// Encoded size in bytes (at most [`MAX_POD`]).
	const SIZE: usize;
	/// Writes the record.
	fn write(&self, w: &mut Writer<'_>);
	/// Reads the record. Missing bytes read as zero.
	fn read(r: &mut Reader<'_>) -> Self;
}

/// The largest `Pod` the core handles.
pub const MAX_POD: usize = 128;

/// Writes little-endian values into a slice; writes past the end are dropped.
pub struct Writer<'a> {
	buf: &'a mut [u8],
	at: usize,
}

impl<'a> Writer<'a> {
	/// Starts at the front of `buf`.
	pub fn new(buf: &'a mut [u8]) -> Self {
		Writer { buf, at: 0 }
	}
	/// Writes one byte.
	pub fn u8(&mut self, v: u8) {
		if let Some(b) = self.buf.get_mut(self.at) {
			*b = v;
		}
		self.at = self.at.saturating_add(1);
	}
	/// Writes a `u16`.
	pub fn u16(&mut self, v: u16) {
		for b in v.to_le_bytes() {
			self.u8(b);
		}
	}
	/// Writes a `u32`.
	pub fn u32(&mut self, v: u32) {
		for b in v.to_le_bytes() {
			self.u8(b);
		}
	}
	/// Writes an `i32`.
	pub fn i32(&mut self, v: i32) {
		for b in v.to_le_bytes() {
			self.u8(b);
		}
	}
	/// Writes a run of bytes.
	pub fn bytes(&mut self, v: &[u8]) {
		for b in v {
			self.u8(*b);
		}
	}
	/// Skips `n` bytes, leaving them as they were.
	pub fn skip(&mut self, n: usize) {
		self.at = self.at.saturating_add(n);
	}
}

/// Reads little-endian values from a slice; reads past the end give zero.
pub struct Reader<'a> {
	buf: &'a [u8],
	at: usize,
}

impl<'a> Reader<'a> {
	/// Starts at the front of `buf`.
	pub fn new(buf: &'a [u8]) -> Self {
		Reader { buf, at: 0 }
	}
	/// Reads one byte.
	pub fn u8(&mut self) -> u8 {
		let v = self.buf.get(self.at).copied().unwrap_or(0);
		self.at = self.at.saturating_add(1);
		v
	}
	/// Reads a `u16`.
	pub fn u16(&mut self) -> u16 {
		let a = self.u8();
		let b = self.u8();
		u16::from_le_bytes([a, b])
	}
	/// Reads a `u32`.
	pub fn u32(&mut self) -> u32 {
		let a = self.u16();
		let b = self.u16();
		(u32::from(b) << 16) | u32::from(a)
	}
	/// Reads an `i32`.
	pub fn i32(&mut self) -> i32 {
		self.u32() as i32
	}
	/// Fills `out` from the input.
	pub fn bytes(&mut self, out: &mut [u8]) {
		for o in out.iter_mut() {
			*o = self.u8();
		}
	}
	/// Skips `n` bytes.
	pub fn skip(&mut self, n: usize) {
		self.at = self.at.saturating_add(n);
	}
}

impl Pod for u16 {
	const SIZE: usize = 2;
	fn write(&self, w: &mut Writer<'_>) {
		w.u16(*self);
	}
	fn read(r: &mut Reader<'_>) -> Self {
		r.u16()
	}
}

impl Pod for u8 {
	const SIZE: usize = 1;
	fn write(&self, w: &mut Writer<'_>) {
		w.u8(*self);
	}
	fn read(r: &mut Reader<'_>) -> Self {
		r.u8()
	}
}
