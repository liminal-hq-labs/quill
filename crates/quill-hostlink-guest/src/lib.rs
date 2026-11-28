// The ReactOS demo's link helper without its Win32 calls: the clipboard bridge between the guest and the page, as a state machine over the host link.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// `win9x/hostlink` (HOSTLINK.EXE) owns the port, the window and the clipboard; this crate owns everything that decides what to say: the
// handshake, the credit after each packet, the clipboard's offers (once per change, never for the helper's own writes), the parts of a `set`
// gathered before the clipboard is touched, and the packets of a big message sent one at a time between reads. The two seams, `Port` and
// `Clipboard`, are what the shell implements and the tests fake. Nothing here allocates, and every buffer is the caller's.

#![no_std]
#![forbid(unsafe_code)]
#![cfg_attr(
	not(test),
	deny(
		clippy::unwrap_used,
		clippy::expect_used,
		clippy::panic,
		clippy::indexing_slicing
	)
)]

#[cfg(test)]
extern crate std;

pub mod text;

use quill_hostlink::{
	encode_message, encode_packet, Body, BodyWriter, Channel, ErrorCode, Event, Mux, Output,
	Parser, MAX_PAYLOAD, PROTO,
};

/// The name `hello` gives the helper.
pub const APP: &str = "HOSTLINK/0.1.0";
/// The page may have this many frame bytes in flight: one longest packet (header and payload) with a little room, so a second full packet
/// waits for the credit. Specified in docs/host-link.md; v86's UART stops taking bytes when it holds more (WP-HOST-1).
pub const WINDOW: u32 = 4_352;
/// Text and rich text of at most this many bytes follow an offer at once; anything else waits for the page's `get`.
pub const TEXT_AUTO_MAX: usize = 256 * 1024;
pub const RTF_AUTO_MAX: usize = 1024 * 1024;
/// The per-format limits (docs/host-link.md): the helper refuses more.
pub const TEXT_MAX: usize = 1024 * 1024;
pub const RTF_MAX: usize = 4 * 1024 * 1024;
pub const DIB_MAX: usize = 4 * 1024 * 1024;
/// Room for a body's lines in front of the data.
pub const BODY_HEAD: usize = 512;
/// What the clipboard's changes settle for before one offer is made (an application's several `SetClipboardData` calls make one).
pub const DEBOUNCE_MS: u32 = 30;
/// A `set` whose parts stop coming is dropped after this long.
pub const SET_TIMEOUT_MS: u32 = 15_000;
/// A half-read frame with nothing more coming is given up after this long (the page restarted mid-packet).
pub const PARTIAL_MS: u32 = 1_500;
/// The room for small messages (hello, credit, offer, ok, errors) waiting to be written ahead of any data.
const URGENT: usize = 1_024;
const JOBS: usize = 8;
/// A packet with its header: the most `encode_packet` writes.
const PACKET_ROOM: usize = MAX_PAYLOAD + 80;

/// A clipboard format the link carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
	/// Text as UTF-8 on the link (`CF_UNICODETEXT` in the guest).
	Text,
	/// `Rich Text Format` bytes.
	Rtf,
	/// A packed DIB, as `CF_DIB` holds it.
	Dib,
}

impl Format {
	pub fn name(self) -> &'static str {
		match self {
			Format::Text => "text",
			Format::Rtf => "rtf",
			Format::Dib => "dib",
		}
	}

	pub fn from_name(name: &[u8]) -> Option<Format> {
		match name {
			b"text" => Some(Format::Text),
			b"rtf" => Some(Format::Rtf),
			b"dib" => Some(Format::Dib),
			_ => None,
		}
	}

	pub fn limit(self) -> usize {
		match self {
			Format::Text => TEXT_MAX,
			Format::Rtf => RTF_MAX,
			Format::Dib => DIB_MAX,
		}
	}

	fn bit(self) -> u8 {
		match self {
			Format::Text => 1,
			Format::Rtf => 2,
			Format::Dib => 4,
		}
	}
}

/// Where the helper's bytes go (COM1).
pub trait Port {
	/// Writes every byte; `false` when the port failed.
	fn write(&mut self, bytes: &[u8]) -> bool;
}

/// The guest's clipboard, as the shell can offer it. Each call opens and closes the clipboard itself; none keeps it open.
pub trait Clipboard {
	/// `GetClipboardSequenceNumber`.
	fn sequence(&mut self) -> u32;
	/// The size in bytes the format would have on the link (text as UTF-8), or `None` when the clipboard has none.
	fn size(&mut self, format: Format) -> Option<usize>;
	/// Reads the format into `out` and returns its length, or `None` when it is absent or does not fit.
	fn read(&mut self, format: Format, out: &mut [u8]) -> Option<usize>;
	/// Holds one part of a `set` (converted as the guest needs it) until `commit`. `false` when it cannot.
	fn stage(&mut self, format: Format, data: &[u8]) -> bool;
	/// Puts everything staged on the clipboard (emptying it first) and returns the clipboard's sequence number afterwards, or `None`.
	fn commit(&mut self) -> Option<u32>;
	/// Drops what was staged.
	fn discard(&mut self);
}

/// What the helper has done, for its log and the tests.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counters {
	pub offers: u32,
	pub data_messages: u32,
	pub sets: u32,
	pub refusals: u32,
	pub credits: u32,
	pub hellos: u32,
	pub errors_sent: u32,
	pub errors_received: u32,
	/// Small messages that did not fit in the queue and were dropped.
	pub lost: u32,
}

#[derive(Clone, Copy)]
enum Job {
	/// Send the format's `data` message for this clipboard sequence, if the clipboard still has it.
	Data { seq: u32, format: Format },
	/// The page's `get`: the same, with a `refused` when it cannot be done.
	Get { seq: u32, format: Format },
}

struct Sending {
	id: u32,
	len: usize,
	at: usize,
}

struct SetState {
	seq: u32,
	want: u8,
	have: u8,
	since: u32,
}

/// Everything but the parser, the assembler and the big buffer, so an event can use these while it borrows the assembler's message.
struct State {
	urgent: [u8; URGENT],
	urgent_len: usize,
	packet: [u8; PACKET_ROOM],
	jobs: [Option<Job>; JOBS],
	sending: Option<Sending>,
	next_id: u32,
	max: u32,
	/// Bytes taken off the port since the last credit.
	taken: u32,
	/// A packet (not a hello) completed since the last credit.
	packet_done: bool,
	/// The page has said hello: it is listening.
	session: bool,
	clip_paused: bool,
	last_seq: u32,
	/// The clipboard's sequence after the helper's own `set`: a change to it is the helper's own and goes nowhere.
	own_seq: Option<u32>,
	debounce: Option<u32>,
	set: Option<SetState>,
	last_rx: u32,
	failed: bool,
	reset_mux: bool,
	counters: Counters,
}

/// The helper's state over caller-owned buffers: the parser's 4,096-byte payload buffer, a buffer for each channel's reassembly and one for
/// the message being sent.
pub struct Helper<'a> {
	parser: Parser<'a>,
	mux: Mux<'a>,
	tx: &'a mut [u8],
	st: State,
}

/// The buffers a `Helper` borrows. `lanes` are in the order of `Channel::index` (ctl, clip, file, time, display): only the clip lane needs to
/// be large, and a message too big for its lane is refused with `too-large`.
pub struct Buffers<'a> {
	pub payload: &'a mut [u8],
	pub lanes: [&'a mut [u8]; 5],
	pub tx: &'a mut [u8],
}

impl<'a> Helper<'a> {
	/// `max` is the largest message the helper accepts (named in `hello`; it should be the clip lane's size).
	pub fn new(buffers: Buffers<'a>, max: u32) -> Helper<'a> {
		Helper {
			parser: Parser::new(buffers.payload),
			mux: Mux::new(buffers.lanes, max as usize),
			tx: buffers.tx,
			st: State {
				urgent: [0; URGENT],
				urgent_len: 0,
				packet: [0; PACKET_ROOM],
				jobs: [None; JOBS],
				sending: None,
				next_id: 0,
				max,
				taken: 0,
				packet_done: false,
				session: false,
				clip_paused: false,
				last_seq: 0,
				own_seq: None,
				debounce: None,
				set: None,
				last_rx: 0,
				failed: false,
				reset_mux: false,
				counters: Counters {
					offers: 0,
					data_messages: 0,
					sets: 0,
					refusals: 0,
					credits: 0,
					hellos: 0,
					errors_sent: 0,
					errors_received: 0,
					lost: 0,
				},
			},
		}
	}

	/// Starts a session: remembers the clipboard as it is (what was there before is not offered) and queues the helper's `hello`.
	pub fn start<C: Clipboard>(&mut self, clip: &mut C) {
		self.st.last_seq = clip.sequence();
		self.st.queue_hello();
	}

	pub fn counters(&self) -> Counters {
		self.st.counters
	}

	/// Whether the page has said hello (and not said bye).
	pub fn session(&self) -> bool {
		self.st.session
	}

	/// Whether something is waiting to be written: the caller should `pump` before it waits again.
	pub fn busy(&self) -> bool {
		self.st.urgent_len > 0
			|| self.st.sending.is_some()
			|| self.st.jobs.iter().any(Option::is_some)
	}

	/// Whether the port failed on a write; the caller should close it.
	pub fn failed(&self) -> bool {
		self.st.failed
	}

	/// Milliseconds until the helper next has work that is not input (the clipboard settling, a `set` going stale, a partial frame being
	/// given up), or `None`.
	pub fn next_due(&self, now: u32) -> Option<u32> {
		let mut due: Option<u32> = None;
		let mut consider = |at: u32| {
			let wait = at.wrapping_sub(now) as i32;
			let wait = if wait < 0 { 0 } else { wait as u32 };
			due = Some(due.map_or(wait, |d| d.min(wait)));
		};
		if let Some(at) = self.st.debounce {
			consider(at);
		}
		if let Some(set) = &self.st.set {
			consider(set.since.wrapping_add(SET_TIMEOUT_MS));
		}
		if self.st.taken > 0 {
			consider(self.st.last_rx.wrapping_add(PARTIAL_MS));
		}
		due
	}

	/// The clipboard changed (`WM_DRAWCLIPBOARD`, any number of times for one copy): the offer is made once it has been quiet for a moment.
	pub fn on_clipboard_change(&mut self, now: u32) {
		if self.st.debounce.is_none() {
			self.st.debounce = Some(now.wrapping_add(DEBOUNCE_MS));
		}
	}

	/// Bytes from the port, in any split.
	pub fn on_bytes<C: Clipboard>(&mut self, bytes: &[u8], clip: &mut C, now: u32) {
		let Helper {
			parser, mux, st, ..
		} = self;
		st.last_rx = now;
		st.taken = st.taken.saturating_add(bytes.len() as u32);
		parser.feed(bytes, |event| st.event(mux, clip, event, now));
		if st.reset_mux {
			// A new session: half-built messages from the page that went away are not finished by the next one.
			st.reset_mux = false;
			mux.reset();
		}
		if st.packet_done {
			st.packet_done = false;
			let n = st.taken;
			st.taken = 0;
			st.queue_credit(n);
		}
	}

	/// Timed work: a settled clipboard change becomes an offer, a stale `set` is dropped, a half-read frame is given up.
	pub fn tick<C: Clipboard>(&mut self, clip: &mut C, now: u32) {
		let Helper { parser, st, .. } = self;
		if let Some(at) = st.debounce {
			if (now.wrapping_sub(at) as i32) >= 0 {
				st.debounce = None;
				st.settle(clip);
			}
		}
		if let Some(set) = &st.set {
			if now.wrapping_sub(set.since) >= SET_TIMEOUT_MS {
				st.set = None;
				clip.discard();
			}
		}
		if st.taken > 0 && now.wrapping_sub(st.last_rx) >= PARTIAL_MS {
			st.taken = 0;
			parser.reset();
		}
	}

	/// Writes the next piece of what is queued: the small messages first, then one packet of the message being sent. Returns whether more is
	/// waiting. Call it between reads, so the port keeps being read while a big message goes out.
	pub fn pump<P: Port, C: Clipboard>(&mut self, port: &mut P, clip: &mut C) -> bool {
		let Helper { tx, st, .. } = self;
		if st.failed {
			return false;
		}
		if st.urgent_len > 0 {
			let n = st.urgent_len;
			st.urgent_len = 0;
			let ok = st.urgent.get(..n).is_some_and(|bytes| port.write(bytes));
			if !ok {
				st.fail();
				return false;
			}
			return true;
		}
		if st.sending.is_none() && !st.start_next(tx, clip) {
			return st.urgent_len > 0 || st.jobs.iter().any(Option::is_some);
		}
		let Some(sending) = st.sending.as_mut() else {
			return true;
		};
		let take = (sending.len - sending.at).min(MAX_PAYLOAD);
		let more = sending.at + take < sending.len;
		let id = sending.id;
		let from = sending.at;
		let written = match (tx.get(from..from + take), st.packet.as_mut_slice()) {
			(Some(payload), room) => encode_packet(room, Channel::Clip, id, more, payload).ok(),
			_ => None,
		};
		let ok = written.is_some_and(|n| st.packet.get(..n).is_some_and(|bytes| port.write(bytes)));
		if !ok {
			st.fail();
			return false;
		}
		if more {
			if let Some(sending) = st.sending.as_mut() {
				sending.at += take;
			}
			return true;
		}
		st.sending = None;
		st.counters.data_messages += 1;
		st.urgent_len > 0 || st.jobs.iter().any(Option::is_some)
	}
}

impl State {
	fn fail(&mut self) {
		self.failed = true;
		self.urgent_len = 0;
		self.sending = None;
		self.jobs = [None; JOBS];
	}

	fn take_id(&mut self) -> u32 {
		let id = self.next_id;
		self.next_id = self.next_id.wrapping_add(1);
		id
	}

	/// Queues a small message (one that fits a packet) behind whatever small messages wait, ahead of any data.
	fn queue_small(&mut self, channel: Channel, body: &[u8]) {
		let id = self.take_id();
		let used = self.urgent_len;
		let encoded = self
			.urgent
			.get_mut(used..)
			.map(|room| encode_message(room, channel, id, body));
		match encoded {
			Some(Ok(n)) => self.urgent_len = used + n,
			_ => self.counters.lost += 1,
		}
	}

	fn queue_hello(&mut self) {
		let mut scratch = [0u8; 256];
		let max = self.max;
		let built = (|| {
			let mut w = BodyWriter::new(&mut scratch, b"hello").ok()?;
			w.put_u32(b"proto", PROTO).ok()?;
			w.put(b"role", b"guest").ok()?;
			w.put(b"app", APP.as_bytes()).ok()?;
			w.put(b"caps", b"clip").ok()?;
			w.put_u32(b"window", WINDOW).ok()?;
			w.put_u32(b"max", max).ok()?;
			w.finish(&[]).ok()
		})();
		if let Some(len) = built {
			if let Some(body) = scratch.get(..len) {
				self.queue_small(Channel::Ctl, body);
				self.counters.hellos += 1;
			}
		}
	}

	fn queue_credit(&mut self, n: u32) {
		let mut scratch = [0u8; 64];
		let built = (|| {
			let mut w = BodyWriter::new(&mut scratch, b"credit").ok()?;
			w.put_u32(b"n", n).ok()?;
			w.finish(&[]).ok()
		})();
		if let Some(len) = built {
			if let Some(body) = scratch.get(..len) {
				self.queue_small(Channel::Ctl, body);
				self.counters.credits += 1;
			}
		}
	}

	fn queue_error(&mut self, code: ErrorCode, id: u32) {
		let mut scratch = [0u8; 96];
		let built = (|| {
			let mut w = BodyWriter::new(&mut scratch, b"error").ok()?;
			w.put(b"code", code.as_str().as_bytes()).ok()?;
			w.put_u32(b"id", id).ok()?;
			w.finish(&[]).ok()
		})();
		if let Some(len) = built {
			if let Some(body) = scratch.get(..len) {
				self.queue_small(Channel::Ctl, body);
				self.counters.errors_sent += 1;
			}
		}
	}

	fn queue_clip(&mut self, op: &[u8], seq: u32, reason: Option<&[u8]>) {
		let mut scratch = [0u8; 128];
		let built = (|| {
			let mut w = BodyWriter::new(&mut scratch, op).ok()?;
			w.put_u32(b"seq", seq).ok()?;
			if let Some(reason) = reason {
				w.put(b"reason", reason).ok()?;
			}
			w.finish(&[]).ok()
		})();
		if let Some(len) = built {
			if let Some(body) = scratch.get(..len) {
				self.queue_small(Channel::Clip, body);
			}
		}
		if op == b"refused" {
			self.counters.refusals += 1;
		}
	}

	fn push_job(&mut self, job: Job) {
		if let Some(slot) = self.jobs.iter_mut().find(|slot| slot.is_none()) {
			*slot = Some(job);
		} else {
			self.counters.lost += 1;
		}
	}

	fn pop_job(&mut self) -> Option<Job> {
		let first = self.jobs.first().copied().flatten()?;
		let mut at = 0usize;
		while at + 1 < JOBS {
			let next = self.jobs.get(at + 1).copied().flatten();
			if let Some(slot) = self.jobs.get_mut(at) {
				*slot = next;
			}
			at += 1;
		}
		if let Some(last) = self.jobs.get_mut(JOBS - 1) {
			*last = None;
		}
		Some(first)
	}

	/// The debounce is over: offer the clipboard if it really changed and the change is not the helper's own.
	fn settle<C: Clipboard>(&mut self, clip: &mut C) {
		let seq = clip.sequence();
		if seq == self.last_seq {
			// A text copy reaches the viewer twice with one sequence number.
			return;
		}
		self.last_seq = seq;
		if self.own_seq == Some(seq) || !self.session || self.clip_paused {
			return;
		}
		let text = clip.size(Format::Text);
		let rtf = clip.size(Format::Rtf);
		let dib = clip.size(Format::Dib);
		if text.is_none() && rtf.is_none() && dib.is_none() {
			return;
		}
		let mut scratch = [0u8; 192];
		let built = (|| {
			let mut w = BodyWriter::new(&mut scratch, b"offer").ok()?;
			w.put_u32(b"seq", seq).ok()?;
			for (key, size) in [(&b"text"[..], text), (b"rtf", rtf), (b"dib", dib)] {
				if let Some(size) = size {
					w.put_u32(key, u32::try_from(size).unwrap_or(u32::MAX))
						.ok()?;
				}
			}
			w.put(b"source", b"user").ok()?;
			w.finish(&[]).ok()
		})();
		if let Some(len) = built {
			if let Some(body) = scratch.get(..len) {
				self.queue_small(Channel::Clip, body);
				self.counters.offers += 1;
			}
		}
		if text.is_some_and(|n| n <= TEXT_AUTO_MAX) {
			self.push_job(Job::Data {
				seq,
				format: Format::Text,
			});
		}
		if rtf.is_some_and(|n| n <= RTF_AUTO_MAX) {
			self.push_job(Job::Data {
				seq,
				format: Format::Rtf,
			});
		}
	}

	/// Begins the next queued message into `tx`. `false` when there is none (or it came to nothing).
	fn start_next<C: Clipboard>(&mut self, tx: &mut [u8], clip: &mut C) -> bool {
		while let Some(job) = self.pop_job() {
			let (seq, format, asked) = match job {
				Job::Data { seq, format } => (seq, format, false),
				Job::Get { seq, format } => (seq, format, true),
			};
			if self.clip_paused {
				if asked {
					self.queue_clip(b"refused", seq, Some(b"paused"));
				}
				continue;
			}
			if clip.sequence() != seq {
				if asked {
					self.queue_clip(b"refused", seq, Some(b"changed"));
				}
				continue;
			}
			let mut head = [0u8; BODY_HEAD];
			let head_len = (|| {
				let mut w = BodyWriter::new(&mut head, b"data").ok()?;
				w.put_u32(b"seq", seq).ok()?;
				w.put(b"fmt", format.name().as_bytes()).ok()?;
				w.finish(&[]).ok()
			})();
			let Some(head_len) = head_len else { continue };
			let read = tx
				.get_mut(head_len..)
				.and_then(|room| clip.read(format, room));
			let Some(read) = read else {
				if asked {
					self.queue_clip(b"refused", seq, Some(b"absent"));
				}
				continue;
			};
			if read > format.limit() {
				if asked {
					self.queue_clip(b"refused", seq, Some(b"too-large"));
				}
				continue;
			}
			if let (Some(front), Some(source)) = (tx.get_mut(..head_len), head.get(..head_len)) {
				front.copy_from_slice(source);
			}
			let id = self.take_id();
			self.sending = Some(Sending {
				id,
				len: head_len + read,
				at: 0,
			});
			return true;
		}
		false
	}

	fn event<C: Clipboard>(&mut self, mux: &mut Mux<'_>, clip: &mut C, event: Event<'_>, now: u32) {
		match event {
			Event::Packet {
				channel,
				id,
				more,
				payload,
			} => {
				self.packet_done = true;
				match mux.push(channel, id, more, payload) {
					Output::None => {}
					Output::Message { channel, id, body } => {
						let hello = channel == Channel::Ctl && body.op() == b"hello";
						self.message(channel, id, &body, clip, now);
						if hello {
							// A hello is outside the credit account.
							self.packet_done = false;
							self.taken = 0;
							self.reset_mux = true;
						}
					}
					Output::Error { code, id, .. } => self.queue_error(code, id),
				}
			}
			Event::Error(quill_hostlink::ParseError::Crc { channel, id, .. }) => {
				self.packet_done = true;
				if let Output::Error { code, id, .. } = mux.crc_failed(channel, id) {
					self.queue_error(code, id);
				}
			}
			// A line that is not a frame, or a trailer: skipped, as the specification says. The page never sends LHQ-FILE.
			Event::Error(_)
			| Event::FileBegin { .. }
			| Event::FileData(_)
			| Event::FileEnd { .. } => {}
		}
	}

	fn message<C: Clipboard>(
		&mut self,
		channel: Channel,
		id: u32,
		body: &Body<'_>,
		clip: &mut C,
		now: u32,
	) {
		match channel {
			Channel::Ctl => self.control(id, body, clip),
			Channel::Clip => self.clipboard(id, body, clip, now),
			_ => self.queue_error(ErrorCode::Unknown, id),
		}
	}

	fn control<C: Clipboard>(&mut self, id: u32, body: &Body<'_>, clip: &mut C) {
		match body.op() {
			b"hello" => {
				let proto = body.get_u32(b"proto").unwrap_or(0);
				// Whatever was in flight died with the page it was for.
				self.urgent_len = 0;
				self.sending = None;
				self.jobs = [None; JOBS];
				if self.set.take().is_some() {
					clip.discard();
				}
				self.clip_paused = false;
				self.debounce = None;
				if proto > PROTO {
					self.session = false;
					self.queue_error(ErrorCode::Version, id);
					return;
				}
				self.session = true;
				self.queue_hello();
			}
			b"pause" | b"resume" => {
				if body.get(b"channel") == Some(b"clip") {
					self.clip_paused = body.op() == b"pause";
				}
			}
			b"error" => self.counters.errors_received += 1,
			b"bye" => {
				self.session = false;
				self.clip_paused = false;
			}
			// `credit` is the page's to receive, not ours; anything else is not understood.
			b"credit" => {}
			_ => self.queue_error(ErrorCode::Unknown, id),
		}
	}

	fn clipboard<C: Clipboard>(&mut self, id: u32, body: &Body<'_>, clip: &mut C, now: u32) {
		if !self.session {
			return;
		}
		match body.op() {
			b"get" => {
				let (Some(seq), Some(format)) = (
					body.get_u32(b"seq"),
					body.get(b"fmt").and_then(Format::from_name),
				) else {
					self.queue_error(ErrorCode::Malformed, id);
					return;
				};
				self.push_job(Job::Get { seq, format });
			}
			b"set" => {
				if self.set.take().is_some() {
					clip.discard();
				}
				let Some(seq) = body.get_u32(b"seq") else {
					self.queue_error(ErrorCode::Malformed, id);
					return;
				};
				let mut want = 0u8;
				for name in body.get(b"formats").unwrap_or(&[]).split(|&b| b == b',') {
					if let Some(format) = Format::from_name(name) {
						want |= format.bit();
					}
				}
				if want == 0 {
					self.queue_clip(b"refused", seq, Some(b"no-formats"));
					return;
				}
				self.set = Some(SetState {
					seq,
					want,
					have: 0,
					since: now,
				});
			}
			b"part" => self.part(id, body, clip, now),
			_ => self.queue_error(ErrorCode::Unknown, id),
		}
	}

	fn part<C: Clipboard>(&mut self, id: u32, body: &Body<'_>, clip: &mut C, now: u32) {
		let (Some(seq), Some(format)) = (
			body.get_u32(b"seq"),
			body.get(b"fmt").and_then(Format::from_name),
		) else {
			self.queue_error(ErrorCode::Malformed, id);
			return;
		};
		let Some(set) = self
			.set
			.as_mut()
			.filter(|set| set.seq == seq && set.want & format.bit() != 0)
		else {
			self.queue_clip(b"refused", seq, Some(b"no-set"));
			return;
		};
		set.since = now;
		let data = body.data();
		let ok = data.len() <= format.limit() && clip.stage(format, data);
		if !ok {
			let reason: &[u8] = if data.len() > format.limit() {
				b"too-large"
			} else {
				b"failed"
			};
			self.set = None;
			clip.discard();
			self.queue_clip(b"refused", seq, Some(reason));
			return;
		}
		set.have |= format.bit();
		if set.have != set.want {
			return;
		}
		self.set = None;
		self.counters.sets += 1;
		match clip.commit() {
			Some(after) => {
				// The helper's own change: when it settles it matches this and goes nowhere.
				self.own_seq = Some(after);
				self.queue_clip(b"ok", seq, None);
			}
			None => {
				clip.discard();
				self.queue_clip(b"refused", seq, Some(b"busy"));
			}
		}
	}
}

#[cfg(test)]
mod tests;
