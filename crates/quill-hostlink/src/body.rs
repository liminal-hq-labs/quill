// A message body: `key=value` lines, an empty line, then binary data. The first key is always `op`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// The longest key.
pub const KEY_MAX: usize = 32;
/// The longest value.
pub const VALUE_MAX: usize = 256;
/// The most fields in one body, `op` included.
pub const FIELDS_MAX: usize = 16;

/// Why bytes are not a body.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BodyError {
	/// A line with no end, or no empty line after the fields.
	Unterminated,
	/// A line with no `=`, or a key or value outside the limits.
	BadField,
	/// The first field is not `op`.
	NoOp,
	/// More than `FIELDS_MAX` fields.
	TooManyFields,
	/// The output buffer is too small (writer only).
	NoRoom,
}

fn valid_key(key: &[u8]) -> bool {
	!key.is_empty()
		&& key.len() <= KEY_MAX
		&& key
			.iter()
			.all(|&b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
}

fn valid_value(value: &[u8]) -> bool {
	value.len() <= VALUE_MAX && !value.contains(&b'\n')
}

fn split_once(bytes: &[u8], at: u8) -> Option<(&[u8], &[u8])> {
	let i = bytes.iter().position(|&b| b == at)?;
	Some((bytes.get(..i)?, bytes.get(i + 1..)?))
}

/// A parsed body borrowing the bytes it came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Body<'a> {
	head: &'a [u8],
	data: &'a [u8],
}

impl<'a> Body<'a> {
	/// Checks every field and finds where the data starts.
	pub fn parse(bytes: &'a [u8]) -> Result<Body<'a>, BodyError> {
		let mut rest = bytes;
		let mut count = 0usize;
		let mut head_len = 0usize;
		loop {
			let (line, after) = split_once(rest, b'\n').ok_or(BodyError::Unterminated)?;
			if line.is_empty() {
				let head = bytes.get(..head_len).ok_or(BodyError::Unterminated)?;
				return Ok(Body { head, data: after });
			}
			let (key, value) = split_once(line, b'=').ok_or(BodyError::BadField)?;
			if !valid_key(key) || !valid_value(value) {
				return Err(BodyError::BadField);
			}
			if count == 0 && (key != b"op" || value.is_empty()) {
				return Err(BodyError::NoOp);
			}
			count += 1;
			if count > FIELDS_MAX {
				return Err(BodyError::TooManyFields);
			}
			head_len += line.len() + 1;
			rest = after;
		}
	}

	/// The `op` value.
	pub fn op(&self) -> &'a [u8] {
		self.get(b"op").unwrap_or(&[])
	}

	/// The first field named `key`.
	pub fn get(&self, key: &[u8]) -> Option<&'a [u8]> {
		self.fields().find(|(k, _)| *k == key).map(|(_, v)| v)
	}

	/// A decimal field as a number.
	pub fn get_u32(&self, key: &[u8]) -> Option<u32> {
		let text = self.get(key)?;
		if text.is_empty() || text.len() > 10 {
			return None;
		}
		let mut n: u64 = 0;
		for &b in text {
			if !b.is_ascii_digit() {
				return None;
			}
			n = n * 10 + u64::from(b - b'0');
		}
		u32::try_from(n).ok()
	}

	/// The fields in order, `op` first.
	pub fn fields(&self) -> Fields<'a> {
		Fields { rest: self.head }
	}

	/// The bytes after the empty line.
	pub fn data(&self) -> &'a [u8] {
		self.data
	}
}

/// The fields of a body, as (key, value).
#[derive(Clone, Copy, Debug)]
pub struct Fields<'a> {
	rest: &'a [u8],
}

impl<'a> Iterator for Fields<'a> {
	type Item = (&'a [u8], &'a [u8]);

	fn next(&mut self) -> Option<Self::Item> {
		let (line, after) = split_once(self.rest, b'\n')?;
		self.rest = after;
		split_once(line, b'=')
	}
}

/// Writes a body into the caller's buffer.
pub struct BodyWriter<'a> {
	out: &'a mut [u8],
	len: usize,
	count: usize,
}

impl<'a> BodyWriter<'a> {
	/// Starts a body with its `op`.
	pub fn new(out: &'a mut [u8], op: &[u8]) -> Result<BodyWriter<'a>, BodyError> {
		let mut writer = BodyWriter {
			out,
			len: 0,
			count: 0,
		};
		if op.is_empty() {
			return Err(BodyError::NoOp);
		}
		writer.put(b"op", op)?;
		Ok(writer)
	}

	fn raw(&mut self, bytes: &[u8]) -> Result<(), BodyError> {
		let end = self.len + bytes.len();
		self.out
			.get_mut(self.len..end)
			.ok_or(BodyError::NoRoom)?
			.copy_from_slice(bytes);
		self.len = end;
		Ok(())
	}

	/// Adds a field.
	pub fn put(&mut self, key: &[u8], value: &[u8]) -> Result<(), BodyError> {
		if !valid_key(key) || !valid_value(value) {
			return Err(BodyError::BadField);
		}
		if self.count >= FIELDS_MAX {
			return Err(BodyError::TooManyFields);
		}
		self.raw(key)?;
		self.raw(b"=")?;
		self.raw(value)?;
		self.raw(b"\n")?;
		self.count += 1;
		Ok(())
	}

	/// Adds a decimal field.
	pub fn put_u32(&mut self, key: &[u8], n: u32) -> Result<(), BodyError> {
		let mut digits = [0u8; 10];
		let mut start = digits.len();
		let mut rest = n;
		loop {
			start -= 1;
			if let Some(slot) = digits.get_mut(start) {
				*slot = b'0' + (rest % 10) as u8;
			}
			rest /= 10;
			if rest == 0 || start == 0 {
				break;
			}
		}
		let text = digits.get(start..).ok_or(BodyError::BadField)?;
		self.put(key, text)
	}

	/// Ends the fields with the empty line, appends `data` and returns the body's length.
	pub fn finish(mut self, data: &[u8]) -> Result<usize, BodyError> {
		self.raw(b"\n")?;
		self.raw(data)?;
		Ok(self.len)
	}
}
