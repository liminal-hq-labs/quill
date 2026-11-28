// Text the helper moves between the clipboard and the link: UTF-16 (CF_UNICODETEXT) and Windows-1252 (CF_TEXT) to UTF-8, and UTF-8 back.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Every function works on bytes, so the caller can hand over a locked block of memory as a slice without any alignment promise, and counts
// first when the output buffer is not yet known (`None` for the buffer). A bad unit becomes U+FFFD, never an error: the clipboard is not ours.

/// Windows-1252's 0x80 to 0x9F as Unicode (the five undefined bytes map to themselves, as Windows does).
const CP1252_HIGH: [u16; 32] = [
	0x20AC, 0x0081, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, 0x02C6, 0x2030, 0x0160, 0x2039,
	0x0152, 0x008D, 0x017D, 0x008F, 0x0090, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013, 0x2014,
	0x02DC, 0x2122, 0x0161, 0x203A, 0x0153, 0x009D, 0x017E, 0x0178,
];

/// Where encoded bytes go: counted only, or written into a buffer (which can run out).
struct Sink<'a> {
	out: Option<&'a mut [u8]>,
	len: usize,
	full: bool,
}

impl Sink<'_> {
	fn push(&mut self, byte: u8) {
		if let Some(out) = self.out.as_deref_mut() {
			match out.get_mut(self.len) {
				Some(slot) => *slot = byte,
				None => self.full = true,
			}
		}
		self.len += 1;
	}

	fn char(&mut self, c: u32) {
		if c < 0x80 {
			self.push(c as u8);
		} else if c < 0x800 {
			self.push(0xC0 | (c >> 6) as u8);
			self.push(0x80 | (c & 0x3F) as u8);
		} else if c < 0x1_0000 {
			self.push(0xE0 | (c >> 12) as u8);
			self.push(0x80 | ((c >> 6) & 0x3F) as u8);
			self.push(0x80 | (c & 0x3F) as u8);
		} else {
			self.push(0xF0 | (c >> 18) as u8);
			self.push(0x80 | ((c >> 12) & 0x3F) as u8);
			self.push(0x80 | ((c >> 6) & 0x3F) as u8);
			self.push(0x80 | (c & 0x3F) as u8);
		}
	}

	fn finish(self) -> Option<usize> {
		if self.full {
			None
		} else {
			Some(self.len)
		}
	}
}

fn unit(bytes: &[u8], at: usize) -> Option<u32> {
	let pair = bytes.get(at..at + 2)?;
	Some(u32::from(u16::from_le_bytes([
		*pair.first()?,
		*pair.get(1)?,
	])))
}

/// UTF-16LE (as bytes, up to the first NUL unit or the end) to UTF-8. With `out` of `None` it only counts. `None` when `out` is too small.
pub fn utf16le_to_utf8(src: &[u8], out: Option<&mut [u8]>) -> Option<usize> {
	let mut sink = Sink {
		out,
		len: 0,
		full: false,
	};
	let mut at = 0usize;
	while let Some(u) = unit(src, at) {
		at += 2;
		if u == 0 {
			break;
		}
		if (0xD800..0xDC00).contains(&u) {
			match unit(src, at) {
				Some(low) if (0xDC00..0xE000).contains(&low) => {
					at += 2;
					sink.char(0x1_0000 + ((u - 0xD800) << 10) + (low - 0xDC00));
				}
				_ => sink.char(0xFFFD),
			}
		} else if (0xDC00..0xE000).contains(&u) {
			sink.char(0xFFFD);
		} else {
			sink.char(u);
		}
	}
	sink.finish()
}

/// Windows-1252 bytes (up to the first NUL or the end) to UTF-8.
pub fn cp1252_to_utf8(src: &[u8], out: Option<&mut [u8]>) -> Option<usize> {
	let mut sink = Sink {
		out,
		len: 0,
		full: false,
	};
	for &b in src {
		if b == 0 {
			break;
		}
		let c = match b {
			0x80..=0x9F => u32::from(
				CP1252_HIGH
					.get(usize::from(b - 0x80))
					.copied()
					.unwrap_or(0xFFFD),
			),
			_ => u32::from(b),
		};
		sink.char(c);
	}
	sink.finish()
}

/// UTF-8 to UTF-16LE bytes, with a terminating NUL unit when `terminate` is set. A malformed sequence becomes U+FFFD.
pub fn utf8_to_utf16le(src: &[u8], out: Option<&mut [u8]>, terminate: bool) -> Option<usize> {
	let mut out = out;
	let mut len = 0usize;
	let mut full = false;
	let mut put = |u: u16, out: &mut Option<&mut [u8]>| {
		if let Some(buf) = out.as_deref_mut() {
			let pair = u.to_le_bytes();
			match buf.get_mut(len..len + 2) {
				Some(slot) => {
					if let (Some(a), Some(b)) = (slot.first_mut(), pair.first()) {
						*a = *b;
					}
					if let (Some(a), Some(b)) = (slot.get_mut(1), pair.get(1)) {
						*a = *b;
					}
				}
				None => full = true,
			}
		}
		len += 2;
	};
	let mut at = 0usize;
	while let Some(&lead) = src.get(at) {
		let (need, mut c) = match lead {
			0x00..=0x7F => (0usize, u32::from(lead)),
			0xC2..=0xDF => (1, u32::from(lead & 0x1F)),
			0xE0..=0xEF => (2, u32::from(lead & 0x0F)),
			0xF0..=0xF4 => (3, u32::from(lead & 0x07)),
			_ => {
				put(0xFFFD, &mut out);
				at += 1;
				continue;
			}
		};
		let mut used = 1usize;
		let mut good = true;
		while used <= need {
			match src.get(at + used) {
				Some(&b) if b & 0xC0 == 0x80 => c = (c << 6) | u32::from(b & 0x3F),
				_ => {
					good = false;
					break;
				}
			}
			used += 1;
		}
		let min = [0u32, 0x80, 0x800, 0x1_0000]
			.get(need)
			.copied()
			.unwrap_or(0);
		if !good || c < min || c > 0x10_FFFF || (0xD800..0xE000).contains(&c) {
			put(0xFFFD, &mut out);
			at += if good { used } else { used.max(1) };
			continue;
		}
		at += used;
		if c >= 0x1_0000 {
			let v = c - 0x1_0000;
			put(0xD800 + (v >> 10) as u16, &mut out);
			put(0xDC00 + (v & 0x3FF) as u16, &mut out);
		} else {
			put(c as u16, &mut out);
		}
	}
	if terminate {
		put(0, &mut out);
	}
	if full {
		None
	} else {
		Some(len)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn utf16(text: &str) -> std::vec::Vec<u8> {
		text.encode_utf16().flat_map(u16::to_le_bytes).collect()
	}

	#[test]
	fn utf16_round_trips_accents_symbols_and_astral_characters() {
		for text in ["plain", "Zoë €", "☺ and 😀 and é", "a\r\nb\r\n", ""] {
			let wide = utf16(text);
			let n = utf16le_to_utf8(&wide, None).unwrap();
			let mut out = std::vec![0u8; n];
			assert_eq!(utf16le_to_utf8(&wide, Some(&mut out)), Some(n));
			assert_eq!(out, text.as_bytes());
			let m = utf8_to_utf16le(text.as_bytes(), None, true).unwrap();
			assert_eq!(m, wide.len() + 2);
			let mut back = std::vec![0u8; m];
			assert_eq!(
				utf8_to_utf16le(text.as_bytes(), Some(&mut back), true),
				Some(m)
			);
			assert_eq!(&back[..wide.len()], &wide[..]);
			assert_eq!(&back[wide.len()..], &[0, 0]);
		}
	}

	#[test]
	fn utf16_stops_at_nul_and_replaces_lone_surrogates() {
		let mut wide = utf16("ab");
		wide.extend_from_slice(&[0, 0, b'c', 0]);
		let mut out = [0u8; 8];
		assert_eq!(utf16le_to_utf8(&wide, Some(&mut out)), Some(2));
		let lone = [0x00, 0xD8, b'x', 0];
		let mut out = [0u8; 8];
		assert_eq!(utf16le_to_utf8(&lone, Some(&mut out)), Some(4));
		assert_eq!(&out[..4], "\u{FFFD}x".as_bytes());
	}

	#[test]
	fn a_small_buffer_is_reported_not_overrun() {
		let wide = utf16("hello");
		let mut out = [0u8; 4];
		assert_eq!(utf16le_to_utf8(&wide, Some(&mut out)), None);
		let mut out = [0u8; 9];
		assert_eq!(utf8_to_utf16le(b"hello", Some(&mut out), true), None);
	}

	#[test]
	fn windows_1252_maps_the_high_block() {
		let src = [b'a', 0xE9, 0x80, 0x93, 0x94, 0xFF, 0];
		let mut out = [0u8; 16];
		let n = cp1252_to_utf8(&src, Some(&mut out)).unwrap();
		assert_eq!(
			std::str::from_utf8(&out[..n]).unwrap(),
			"aé€\u{201C}\u{201D}ÿ"
		);
	}

	#[test]
	fn malformed_utf8_becomes_replacement_characters() {
		let mut out = [0u8; 16];
		let n = utf8_to_utf16le(&[b'a', 0xFF, b'b', 0xC3], Some(&mut out), false).unwrap();
		assert_eq!(&out[..n], &utf16("a\u{FFFD}b\u{FFFD}")[..]);
		let n = utf8_to_utf16le(&[0xED, 0xA0, 0x80], Some(&mut out), false).unwrap();
		assert_eq!(&out[..n], &utf16("\u{FFFD}")[..]);
	}
}
