// Reassembling messages from packets: one open message per channel, in a buffer the caller owns.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::body::Body;
use crate::{Channel, ErrorCode};

/// A receiver's largest message when its `hello` names none: 8 MiB.
pub const DEFAULT_MAX_MESSAGE: usize = 8_388_608;
/// The smallest `max` a `hello` may name.
pub const MIN_MAX_MESSAGE: usize = 65_536;

/// What a packet did to one channel's message.
#[derive(Debug, PartialEq, Eq)]
pub enum Step<'m> {
	/// More packets are due, or the packet belonged to a dropped message.
	Pending,
	/// The message is whole and its body parsed.
	Message(Body<'m>),
	/// The message is dropped; the code is for a `ctl op=error` reply.
	Error(ErrorCode),
}

/// One channel's reassembly. A packet with a new id abandons the open message without a reply; a message past `max` (or the buffer) is
/// refused with `too-large` and the rest of its packets are dropped; a packet that failed its CRC poisons the message the same way.
pub struct Assembler<'a> {
	buf: &'a mut [u8],
	len: usize,
	max: usize,
	open: Option<u32>,
	poisoned: Option<u32>,
	dropped: u32,
}

impl<'a> Assembler<'a> {
	pub fn new(buf: &'a mut [u8], max: usize) -> Assembler<'a> {
		Assembler {
			buf,
			len: 0,
			max,
			open: None,
			poisoned: None,
			dropped: 0,
		}
	}

	/// The messages abandoned by a new id, for the diagnostics.
	pub fn dropped(&self) -> u32 {
		self.dropped
	}

	/// Forgets any open message (a new `hello`).
	pub fn reset(&mut self) {
		self.len = 0;
		self.open = None;
		self.poisoned = None;
	}

	/// A packet of message `id` failed its CRC: the open message, if it is that one, and the rest of that id are dropped.
	pub fn poison(&mut self, id: u32) {
		if self.open == Some(id) {
			self.open = None;
			self.len = 0;
		}
		self.poisoned = Some(id);
	}

	/// A packet whose CRC held.
	pub fn push(&mut self, id: u32, more: bool, payload: &[u8]) -> Step<'_> {
		if self.poisoned == Some(id) {
			if !more {
				self.poisoned = None;
			}
			return Step::Pending;
		}
		self.poisoned = None;
		if self.open != Some(id) {
			if self.open.is_some() {
				self.dropped = self.dropped.saturating_add(1);
			}
			self.open = Some(id);
			self.len = 0;
		}
		let cap = self.max.min(self.buf.len());
		let end = self.len.saturating_add(payload.len());
		if end > cap {
			self.open = None;
			self.len = 0;
			if more {
				self.poisoned = Some(id);
			}
			return Step::Error(ErrorCode::TooLarge);
		}
		if let Some(slot) = self.buf.get_mut(self.len..end) {
			slot.copy_from_slice(payload);
		}
		self.len = end;
		if more {
			return Step::Pending;
		}
		self.open = None;
		self.len = 0;
		match self.buf.get(..end).map(Body::parse) {
			Some(Ok(body)) => Step::Message(body),
			_ => Step::Error(ErrorCode::Malformed),
		}
	}
}

/// What the five channels' assemblers report for a packet.
#[derive(Debug, PartialEq, Eq)]
pub enum Output<'m> {
	None,
	Message {
		channel: Channel,
		id: u32,
		body: Body<'m>,
	},
	Error {
		code: ErrorCode,
		channel: Channel,
		id: u32,
	},
}

/// The five channels' assemblers, plus the once-per-message `unknown` reply for a channel the codec does not know.
pub struct Mux<'a> {
	lanes: [Assembler<'a>; 5],
	unknown_id: Option<u32>,
}

impl<'a> Mux<'a> {
	/// One buffer per known channel, in the order of `Channel::index`.
	pub fn new(buffers: [&'a mut [u8]; 5], max: usize) -> Mux<'a> {
		let [a, b, c, d, e] = buffers;
		Mux {
			lanes: [
				Assembler::new(a, max),
				Assembler::new(b, max),
				Assembler::new(c, max),
				Assembler::new(d, max),
				Assembler::new(e, max),
			],
			unknown_id: None,
		}
	}

	pub fn reset(&mut self) {
		for lane in self.lanes.iter_mut() {
			lane.reset();
		}
		self.unknown_id = None;
	}

	/// The messages abandoned by a new id, over every channel.
	pub fn dropped(&self) -> u32 {
		self.lanes
			.iter()
			.fold(0u32, |sum, lane| sum.saturating_add(lane.dropped()))
	}

	/// A packet whose CRC held.
	pub fn push(&mut self, channel: Channel, id: u32, more: bool, payload: &[u8]) -> Output<'_> {
		let Some(index) = channel.index() else {
			if self.unknown_id == Some(id) {
				if !more {
					self.unknown_id = None;
				}
				return Output::None;
			}
			self.unknown_id = if more { Some(id) } else { None };
			return Output::Error {
				code: ErrorCode::Unknown,
				channel,
				id,
			};
		};
		let Some(lane) = self.lanes.get_mut(index) else {
			return Output::None;
		};
		match lane.push(id, more, payload) {
			Step::Pending => Output::None,
			Step::Message(body) => Output::Message { channel, id, body },
			Step::Error(code) => Output::Error { code, channel, id },
		}
	}

	/// A packet failed its CRC: drops its message and returns the `crc` error to reply with.
	pub fn crc_failed(&mut self, channel: Channel, id: u32) -> Output<'_> {
		if let Some(lane) = channel.index().and_then(|index| self.lanes.get_mut(index)) {
			lane.poison(id);
		}
		Output::Error {
			code: ErrorCode::Crc,
			channel,
			id,
		}
	}
}
