// The frames: a push parser for `LHQ-PKT` and `LHQ-FILE` that finds them in noise and resyncs after an error, and the encoders.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::crc::Crc32;
use crate::Channel;

/// The longest packet payload.
pub const MAX_PAYLOAD: usize = 4096;
/// The longest `LHQ-PKT` header line, its CRLF included.
pub const PKT_HEADER_MAX: usize = 64;
/// The longest `LHQ-FILE` header line, its CRLF included.
pub const FILE_HEADER_MAX: usize = 160;
/// The longest `LHQ-END` line, its CRLF included (`LHQ-END `, 64 hex digits, CRLF is 74).
pub const TRAILER_MAX: usize = 80;
/// The longest file name, in bytes.
pub const FILE_NAME_MAX: usize = 128;
/// The largest `LHQ-FILE` body, in bytes (64 MiB).
pub const FILE_MAX_SIZE: u32 = 67_108_864;
const CHANNEL_MAX: usize = 16;

/// What the parser found. Slices borrow the parser's or the caller's buffers and are valid for the call only.
#[derive(Debug, PartialEq, Eq)]
pub enum Event<'a> {
	/// A packet whose CRC held.
	Packet {
		channel: Channel,
		id: u32,
		more: bool,
		payload: &'a [u8],
	},
	/// An `LHQ-FILE` header.
	FileBegin { size: u32, name: &'a [u8] },
	/// Some of the file's bytes, as they arrive (any split).
	FileData(&'a [u8]),
	/// A good `LHQ-END`: the 64 hex digits of the SHA-256 if it carried one.
	FileEnd { sha256: Option<&'a [u8]> },
	/// Something was skipped.
	Error(ParseError),
}

/// What was skipped. `offset` counts bytes since the parser was made (or last reset) and names the first byte of the bad frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParseError {
	/// A line that starts `LHQ-` and is not a frame header.
	Header { offset: u64 },
	/// A packet whose CRC is wrong; the payload was read and dropped. The sender gets `ctl op=error code=crc id=<id>`.
	Crc {
		offset: u64,
		channel: Channel,
		id: u32,
	},
	/// The bytes after a file were not `LHQ-END`.
	Trailer { offset: u64 },
}

#[derive(Clone, Copy)]
enum State {
	/// Looking for the start of a frame: `LHQ-` at the start of a line.
	Scan,
	/// Reading a header line.
	Header,
	/// Dropping a bad line up to its newline.
	SkipLine,
	Payload {
		channel: Channel,
		id: u32,
		more: bool,
		want: usize,
		got: usize,
		crc: Crc32,
		expect: u32,
	},
	FileData {
		remaining: u32,
	},
	Trailer {
		start: u64,
	},
}

/// The push parser. It owns a 160-byte line and borrows a payload buffer of at least `MAX_PAYLOAD` bytes.
pub struct Parser<'a> {
	payload: &'a mut [u8],
	line: [u8; FILE_HEADER_MAX],
	line_len: usize,
	state: State,
	offset: u64,
	frame_start: u64,
	at_line_start: bool,
}

fn is_digit_text(text: &[u8]) -> bool {
	!text.is_empty()
		&& text.iter().all(u8::is_ascii_digit)
		&& (text.len() == 1 || text.first() != Some(&b'0'))
}

fn parse_u32(text: &[u8]) -> Option<u32> {
	if !is_digit_text(text) || text.len() > 10 {
		return None;
	}
	let mut n: u64 = 0;
	for &b in text {
		n = n * 10 + u64::from(b - b'0');
	}
	u32::try_from(n).ok()
}

fn parse_hex8(text: &[u8]) -> Option<u32> {
	if text.len() != 8 {
		return None;
	}
	let mut n: u32 = 0;
	for &b in text {
		let digit = match b {
			b'0'..=b'9' => b - b'0',
			b'a'..=b'f' => b - b'a' + 10,
			_ => return None,
		};
		n = (n << 4) | u32::from(digit);
	}
	Some(n)
}

fn is_lower_hex64(text: &[u8]) -> bool {
	text.len() == 64
		&& text
			.iter()
			.all(|&b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// A parsed `LHQ-PKT` line (without its CRLF).
struct PktHeader {
	channel: Channel,
	id: u32,
	more: bool,
	length: usize,
	expect: u32,
	/// How many bytes of the line the CRC covers (everything before the space ahead of the CRC).
	covered: usize,
}

fn parse_pkt(line: &[u8]) -> Option<PktHeader> {
	let mut parts = line.split(|&b| b == b' ');
	if parts.next()? != b"LHQ-PKT" || parts.next()? != b"1" {
		return None;
	}
	let name = parts.next()?;
	if name.is_empty() || name.len() > CHANNEL_MAX || !name.iter().all(u8::is_ascii_lowercase) {
		return None;
	}
	let id = parse_u32(parts.next()?)?;
	let more = match parts.next()? {
		b"M" => true,
		b"E" => false,
		_ => return None,
	};
	let length = parse_u32(parts.next()?)? as usize;
	if length > MAX_PAYLOAD {
		return None;
	}
	let crc_text = parts.next()?;
	if parts.next().is_some() {
		return None;
	}
	let expect = parse_hex8(crc_text)?;
	let covered = line.len().checked_sub(crc_text.len() + 1)?;
	Some(PktHeader {
		channel: Channel::from_name(name),
		id,
		more,
		length,
		expect,
		covered,
	})
}

/// A parsed `LHQ-FILE` line: the size, and where the name starts.
fn parse_file(line: &[u8]) -> Option<(u32, usize)> {
	let mut parts = line.splitn(4, |&b| b == b' ');
	if parts.next()? != b"LHQ-FILE" || parts.next()? != b"1" {
		return None;
	}
	let size = parse_u32(parts.next()?)?;
	let name = parts.next()?;
	if size > FILE_MAX_SIZE
		|| name.is_empty()
		|| name.len() > FILE_NAME_MAX
		|| name.iter().any(|&b| b < 0x20 || b == 0x7f)
	{
		return None;
	}
	Some((size, line.len() - name.len()))
}

impl<'a> Parser<'a> {
	/// `payload` is the buffer a packet's payload is gathered in; it must hold at least `MAX_PAYLOAD` bytes (a shorter one makes every longer packet a bad header).
	pub fn new(payload: &'a mut [u8]) -> Parser<'a> {
		Parser {
			payload,
			line: [0; FILE_HEADER_MAX],
			line_len: 0,
			state: State::Scan,
			offset: 0,
			frame_start: 0,
			at_line_start: true,
		}
	}

	/// Forgets any frame in progress (a new `hello`); the offset restarts at zero.
	pub fn reset(&mut self) {
		self.line_len = 0;
		self.state = State::Scan;
		self.offset = 0;
		self.frame_start = 0;
		self.at_line_start = true;
	}

	/// Bytes consumed since the parser was made or reset.
	pub fn offset(&self) -> u64 {
		self.offset
	}

	/// Takes the next bytes, in any split, and reports what they complete.
	pub fn feed<F: FnMut(Event<'_>)>(&mut self, bytes: &[u8], mut sink: F) {
		let mut i = 0usize;
		while i < bytes.len() {
			match self.state {
				State::Payload {
					channel,
					id,
					more,
					want,
					got,
					mut crc,
					expect,
				} => {
					let rest = bytes.get(i..).unwrap_or(&[]);
					let take = (want - got).min(rest.len());
					let chunk = rest.get(..take).unwrap_or(&[]);
					if let Some(slot) = self.payload.get_mut(got..got + take) {
						slot.copy_from_slice(chunk);
					}
					crc.update(chunk);
					i += take;
					self.offset += take as u64;
					if got + take == want {
						self.finish_packet(channel, id, more, want, crc, expect, &mut sink);
					} else {
						self.state = State::Payload {
							channel,
							id,
							more,
							want,
							got: got + take,
							crc,
							expect,
						};
					}
				}
				State::FileData { remaining } => {
					let rest = bytes.get(i..).unwrap_or(&[]);
					let take = (remaining as usize).min(rest.len());
					let chunk = rest.get(..take).unwrap_or(&[]);
					sink(Event::FileData(chunk));
					i += take;
					self.offset += take as u64;
					let remaining = remaining - take as u32;
					if remaining == 0 {
						self.line_len = 0;
						self.state = State::Trailer { start: self.offset };
					} else {
						self.state = State::FileData { remaining };
					}
				}
				_ => {
					let b = bytes.get(i).copied().unwrap_or(0);
					i += 1;
					self.offset += 1;
					self.byte(b, &mut sink);
				}
			}
		}
	}

	#[allow(clippy::too_many_arguments)]
	fn finish_packet<F: FnMut(Event<'_>)>(
		&mut self,
		channel: Channel,
		id: u32,
		more: bool,
		want: usize,
		crc: Crc32,
		expect: u32,
		sink: &mut F,
	) {
		self.state = State::Scan;
		self.at_line_start = true;
		if crc.finish() == expect {
			let payload = self.payload.get(..want).unwrap_or(&[]);
			sink(Event::Packet {
				channel,
				id,
				more,
				payload,
			});
		} else {
			sink(Event::Error(ParseError::Crc {
				offset: self.frame_start,
				channel,
				id,
			}));
		}
	}

	fn fail_header<F: FnMut(Event<'_>)>(&mut self, b: u8, sink: &mut F) {
		sink(Event::Error(ParseError::Header {
			offset: self.frame_start,
		}));
		self.line_len = 0;
		if b == b'\n' {
			self.state = State::Scan;
			self.at_line_start = true;
		} else {
			self.state = State::SkipLine;
		}
	}

	fn byte<F: FnMut(Event<'_>)>(&mut self, b: u8, sink: &mut F) {
		match self.state {
			State::Scan => {
				if self.at_line_start && b == b'L' {
					self.frame_start = self.offset - 1;
					self.line[0] = b;
					self.line_len = 1;
					self.state = State::Header;
				}
				self.at_line_start = b == b'\n';
			}
			State::SkipLine => {
				if b == b'\n' {
					self.state = State::Scan;
					self.at_line_start = true;
				}
			}
			State::Header => self.header_byte(b, sink),
			State::Trailer { start } => self.trailer_byte(b, start, sink),
			State::Payload { .. } | State::FileData { .. } => {}
		}
	}

	fn header_byte<F: FnMut(Event<'_>)>(&mut self, b: u8, sink: &mut F) {
		let len = self.line_len;
		// The first four bytes must spell `LHQ-`; anything else was noise that began with an L.
		if len < 4 {
			let expected = b"LHQ-".get(len).copied().unwrap_or(0);
			if b != expected {
				self.line_len = 0;
				self.state = State::Scan;
				self.at_line_start = b == b'\n';
				return;
			}
		}
		if b == b'\n' {
			self.header_line(sink);
			return;
		}
		let pkt = self
			.line
			.get(..8)
			.is_some_and(|p| len >= 8 && p == b"LHQ-PKT ");
		let limit = if len >= 8 && !pkt {
			FILE_HEADER_MAX
		} else if pkt {
			PKT_HEADER_MAX
		} else {
			FILE_HEADER_MAX
		};
		if len + 1 >= limit {
			self.fail_header(b, sink);
			return;
		}
		if let Some(slot) = self.line.get_mut(len) {
			*slot = b;
		}
		self.line_len = len + 1;
		if self.line_len == 8 {
			let head = self.line.get(..8).unwrap_or(&[]);
			if head != b"LHQ-PKT " && head != b"LHQ-FILE" {
				self.fail_header(b, sink);
			}
		}
	}

	fn header_line<F: FnMut(Event<'_>)>(&mut self, sink: &mut F) {
		let len = self.line_len;
		let line = self.line.get(..len).unwrap_or(&[]);
		let Some(line) = line.strip_suffix(b"\r") else {
			self.fail_header(b'\n', sink);
			return;
		};
		if line.starts_with(b"LHQ-PKT ") {
			if len + 1 > PKT_HEADER_MAX {
				self.fail_header(b'\n', sink);
				return;
			}
			let Some(h) = parse_pkt(line) else {
				self.fail_header(b'\n', sink);
				return;
			};
			let mut crc = Crc32::new();
			crc.update(line.get(..h.covered).unwrap_or(&[]));
			self.line_len = 0;
			if h.length > self.payload.len() {
				self.fail_header(b'\n', sink);
				return;
			}
			if h.length == 0 {
				self.finish_packet(h.channel, h.id, h.more, 0, crc, h.expect, sink);
			} else {
				self.state = State::Payload {
					channel: h.channel,
					id: h.id,
					more: h.more,
					want: h.length,
					got: 0,
					crc,
					expect: h.expect,
				};
			}
		} else if line.starts_with(b"LHQ-FILE ") {
			let Some((size, name_at)) = parse_file(line) else {
				self.fail_header(b'\n', sink);
				return;
			};
			let name = line.get(name_at..).unwrap_or(&[]);
			sink(Event::FileBegin { size, name });
			self.line_len = 0;
			if size == 0 {
				self.state = State::Trailer { start: self.offset };
			} else {
				self.state = State::FileData { remaining: size };
			}
		} else {
			self.fail_header(b'\n', sink);
		}
	}

	fn trailer_byte<F: FnMut(Event<'_>)>(&mut self, b: u8, start: u64, sink: &mut F) {
		let len = self.line_len;
		if b != b'\n' {
			if len + 1 >= TRAILER_MAX {
				sink(Event::Error(ParseError::Trailer { offset: start }));
				self.line_len = 0;
				self.state = State::SkipLine;
				return;
			}
			if let Some(slot) = self.line.get_mut(len) {
				*slot = b;
			}
			self.line_len = len + 1;
			return;
		}
		let line = self.line.get(..len).unwrap_or(&[]);
		let sha = match line.strip_suffix(b"\r") {
			Some(b"LHQ-END") => Some(None),
			Some(rest) => match rest.strip_prefix(b"LHQ-END ") {
				Some(hex) if is_lower_hex64(hex) => Some(Some(hex)),
				_ => None,
			},
			None => None,
		};
		match sha {
			Some(sha256) => sink(Event::FileEnd { sha256 }),
			None => sink(Event::Error(ParseError::Trailer { offset: start })),
		}
		self.line_len = 0;
		self.state = State::Scan;
		self.at_line_start = true;
	}
}

/// Why a frame could not be written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EncodeError {
	/// The output buffer is too small.
	NoRoom,
	/// A payload over `MAX_PAYLOAD`, a channel the codec does not know, or a file name or size outside the limits.
	BadArgument,
}

struct Out<'a> {
	buf: &'a mut [u8],
	len: usize,
}

impl Out<'_> {
	fn bytes(&mut self, bytes: &[u8]) -> Result<(), EncodeError> {
		let end = self.len + bytes.len();
		self.buf
			.get_mut(self.len..end)
			.ok_or(EncodeError::NoRoom)?
			.copy_from_slice(bytes);
		self.len = end;
		Ok(())
	}

	fn decimal(&mut self, n: u32) -> Result<(), EncodeError> {
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
		self.bytes(digits.get(start..).unwrap_or(&[]))
	}
}

/// Writes one packet into `out` and returns its length. The CRC covers the header up to the space before it, then the payload.
pub fn encode_packet(
	out: &mut [u8],
	channel: Channel,
	id: u32,
	more: bool,
	payload: &[u8],
) -> Result<usize, EncodeError> {
	if payload.len() > MAX_PAYLOAD || channel == Channel::Unknown {
		return Err(EncodeError::BadArgument);
	}
	let mut w = Out { buf: out, len: 0 };
	w.bytes(b"LHQ-PKT 1 ")?;
	w.bytes(channel.name().as_bytes())?;
	w.bytes(b" ")?;
	w.decimal(id)?;
	w.bytes(if more { b" M " } else { b" E " })?;
	w.decimal(payload.len() as u32)?;
	let mut crc = Crc32::new();
	crc.update(w.buf.get(..w.len).unwrap_or(&[]));
	crc.update(payload);
	let value = crc.finish();
	w.bytes(b" ")?;
	let mut hex = [0u8; 8];
	for (n, slot) in hex.iter_mut().enumerate() {
		let digit = ((value >> (28 - 4 * n as u32)) & 0xF) as u8;
		*slot = if digit < 10 {
			b'0' + digit
		} else {
			b'a' + digit - 10
		};
	}
	w.bytes(&hex)?;
	w.bytes(b"\r\n")?;
	w.bytes(payload)?;
	Ok(w.len)
}

/// Writes a whole message (`body` as `Body` bytes) as packets of at most `MAX_PAYLOAD` bytes, `M` on all but the last, and returns the length.
/// An empty body is one empty packet.
pub fn encode_message(
	out: &mut [u8],
	channel: Channel,
	id: u32,
	body: &[u8],
) -> Result<usize, EncodeError> {
	let mut written = 0usize;
	let mut rest = body;
	loop {
		let take = rest.len().min(MAX_PAYLOAD);
		let (part, after) = rest.split_at(take);
		let more = !after.is_empty();
		let slot = out.get_mut(written..).ok_or(EncodeError::NoRoom)?;
		written += encode_packet(slot, channel, id, more, part)?;
		rest = after;
		if !more {
			return Ok(written);
		}
	}
}

/// Writes an `LHQ-FILE 1 <size> <name>` header line.
pub fn encode_file_header(out: &mut [u8], size: u32, name: &[u8]) -> Result<usize, EncodeError> {
	if size > FILE_MAX_SIZE
		|| name.is_empty()
		|| name.len() > FILE_NAME_MAX
		|| name.iter().any(|&b| b < 0x20 || b == 0x7f)
	{
		return Err(EncodeError::BadArgument);
	}
	let mut w = Out { buf: out, len: 0 };
	w.bytes(b"LHQ-FILE 1 ")?;
	w.decimal(size)?;
	w.bytes(b" ")?;
	w.bytes(name)?;
	w.bytes(b"\r\n")?;
	Ok(w.len)
}

/// Writes the `LHQ-END` line, with the 64 lowercase hex digits of a SHA-256 if given.
pub fn encode_file_trailer(out: &mut [u8], sha256: Option<&[u8]>) -> Result<usize, EncodeError> {
	let mut w = Out { buf: out, len: 0 };
	w.bytes(b"LHQ-END")?;
	if let Some(hex) = sha256 {
		if !is_lower_hex64(hex) {
			return Err(EncodeError::BadArgument);
		}
		w.bytes(b" ")?;
		w.bytes(hex)?;
	}
	w.bytes(b"\r\n")?;
	Ok(w.len)
}
