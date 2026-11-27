// The host link's codec for the guest end: the frames on COM1, the messages in them and the credits that pace the page. Specified in docs/host-link.md.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Nothing here allocates, reads a clock or touches a port: the caller feeds bytes in and takes events out, and every buffer is the caller's. The
// parser holds at most one header line and one packet payload (4,096 bytes, which the caller supplies), so the guest helper's memory is known
// before it starts.

#![no_std]
#![forbid(unsafe_code)]
#![deny(
	clippy::unwrap_used,
	clippy::expect_used,
	clippy::panic,
	clippy::indexing_slicing
)]

mod body;
mod crc;
mod flow;
mod frame;
mod message;

pub use body::{Body, BodyError, BodyWriter, Fields, FIELDS_MAX, KEY_MAX, VALUE_MAX};
pub use crc::{crc32, Crc32};
pub use flow::{Credit, CreditReporter, CREDIT_STEP, DEFAULT_WINDOW, MIN_WINDOW};
pub use frame::{
	encode_file_header, encode_file_trailer, encode_message, encode_packet, EncodeError, Event,
	ParseError, Parser, FILE_HEADER_MAX, FILE_MAX_SIZE, FILE_NAME_MAX, MAX_PAYLOAD, PKT_HEADER_MAX,
	TRAILER_MAX,
};
pub use message::{Assembler, Mux, Output, Step, DEFAULT_MAX_MESSAGE, MIN_MAX_MESSAGE};

/// The protocol version `hello` announces.
pub const PROTO: u32 = 1;

/// A channel of the link. A name the codec does not know is `Unknown`: its packets are reported once and otherwise ignored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
	Ctl,
	Clip,
	File,
	Time,
	Display,
	Unknown,
}

impl Channel {
	/// The five known channels in the order a sender serves them: control first, files last.
	pub const PRIORITY: [Channel; 5] = [
		Channel::Ctl,
		Channel::Clip,
		Channel::Time,
		Channel::Display,
		Channel::File,
	];

	pub fn from_name(name: &[u8]) -> Channel {
		match name {
			b"ctl" => Channel::Ctl,
			b"clip" => Channel::Clip,
			b"file" => Channel::File,
			b"time" => Channel::Time,
			b"display" => Channel::Display,
			_ => Channel::Unknown,
		}
	}

	/// The wire name; `unknown` for a channel the codec does not know (which cannot be sent).
	pub fn name(self) -> &'static str {
		match self {
			Channel::Ctl => "ctl",
			Channel::Clip => "clip",
			Channel::File => "file",
			Channel::Time => "time",
			Channel::Display => "display",
			Channel::Unknown => "unknown",
		}
	}

	/// A slot 0 to 4 for a known channel, in the order of the enum.
	pub fn index(self) -> Option<usize> {
		match self {
			Channel::Ctl => Some(0),
			Channel::Clip => Some(1),
			Channel::File => Some(2),
			Channel::Time => Some(3),
			Channel::Display => Some(4),
			Channel::Unknown => None,
		}
	}
}

/// The codes of `ctl op=error`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorCode {
	Crc,
	TooLarge,
	Unknown,
	Version,
	Malformed,
}

impl ErrorCode {
	pub fn as_str(self) -> &'static str {
		match self {
			ErrorCode::Crc => "crc",
			ErrorCode::TooLarge => "too-large",
			ErrorCode::Unknown => "unknown",
			ErrorCode::Version => "version",
			ErrorCode::Malformed => "malformed",
		}
	}
}
