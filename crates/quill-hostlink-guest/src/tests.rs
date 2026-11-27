// The helper's state machine against a fake port and a fake clipboard: the bytes it writes are read back with the codec's own parser.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![allow(
	clippy::unwrap_used,
	clippy::expect_used,
	clippy::indexing_slicing,
	clippy::panic
)]

use super::*;
use quill_hostlink::{Output as MuxOut, ParseError};
use std::string::{String, ToString};
use std::vec;
use std::vec::Vec;

/// What the parser saw of the packets: channel, id, more, payload length.
type Packets = Vec<(Channel, u32, bool, usize)>;

const CLIP_LANE: usize = 4 * 1024 * 1024 + 4096;

#[derive(Debug, Clone, PartialEq)]
struct Msg {
	channel: Channel,
	id: u32,
	op: String,
	fields: Vec<(String, String)>,
	data: Vec<u8>,
}

impl Msg {
	fn get(&self, key: &str) -> Option<&str> {
		self.fields
			.iter()
			.find(|(k, _)| k == key)
			.map(|(_, v)| v.as_str())
	}
}

#[derive(Default)]
struct FakePort {
	bytes: Vec<u8>,
	writes: Vec<usize>,
	fail: bool,
}

impl Port for FakePort {
	fn write(&mut self, bytes: &[u8]) -> bool {
		if self.fail {
			return false;
		}
		self.writes.push(bytes.len());
		self.bytes.extend_from_slice(bytes);
		true
	}
}

impl FakePort {
	/// Everything written since the last call, as messages, plus how many packets and the longest packet's payload.
	fn take(&mut self) -> (Vec<Msg>, Packets) {
		let bytes = std::mem::take(&mut self.bytes);
		decode(&bytes)
	}

	fn messages(&mut self) -> Vec<Msg> {
		self.take().0
	}
}

fn decode(bytes: &[u8]) -> (Vec<Msg>, Packets) {
	let mut payload = vec![0u8; MAX_PAYLOAD];
	let mut parser = Parser::new(&mut payload);
	let mut lanes: Vec<Vec<u8>> = (0..5).map(|_| vec![0u8; CLIP_LANE]).collect();
	let [a, b, c, d, e] = &mut lanes[..] else {
		unreachable!()
	};
	let mut mux = Mux::new([a, b, c, d, e], CLIP_LANE);
	let mut messages = Vec::new();
	let mut packets: Packets = Vec::new();
	parser.feed(bytes, |event| match event {
		Event::Packet {
			channel,
			id,
			more,
			payload,
		} => {
			packets.push((channel, id, more, payload.len()));
			match mux.push(channel, id, more, payload) {
				MuxOut::Message { channel, id, body } => messages.push(Msg {
					channel,
					id,
					op: String::from_utf8_lossy(body.op()).to_string(),
					fields: body
						.fields()
						.map(|(k, v)| {
							(
								String::from_utf8_lossy(k).to_string(),
								String::from_utf8_lossy(v).to_string(),
							)
						})
						.collect(),
					data: body.data().to_vec(),
				}),
				MuxOut::Error { code, .. } => panic!("the helper wrote a bad message: {code:?}"),
				MuxOut::None => {}
			}
		}
		Event::Error(error) => panic!("the helper wrote a bad frame: {error:?}"),
		_ => {}
	});
	(messages, packets)
}

#[derive(Default)]
struct FakeClip {
	seq: u32,
	text: Option<Vec<u8>>,
	rtf: Option<Vec<u8>>,
	dib: Option<Vec<u8>>,
	staged: Vec<(Format, Vec<u8>)>,
	stage_ok: bool,
	commit_ok: bool,
	commits: u32,
	discards: u32,
	reads: u32,
}

impl FakeClip {
	fn new() -> FakeClip {
		FakeClip {
			seq: 7,
			stage_ok: true,
			commit_ok: true,
			..FakeClip::default()
		}
	}

	/// An application copies: one change of the sequence, however many messages the viewer gets.
	fn copy(&mut self, text: Option<&str>, rtf: Option<&str>, dib: Option<Vec<u8>>) {
		self.seq += 1;
		self.text = text.map(|t| t.as_bytes().to_vec());
		self.rtf = rtf.map(|t| t.as_bytes().to_vec());
		self.dib = dib;
	}

	fn slot(&mut self, format: Format) -> &mut Option<Vec<u8>> {
		match format {
			Format::Text => &mut self.text,
			Format::Rtf => &mut self.rtf,
			Format::Dib => &mut self.dib,
		}
	}
}

impl Clipboard for FakeClip {
	fn sequence(&mut self) -> u32 {
		self.seq
	}
	fn size(&mut self, format: Format) -> Option<usize> {
		self.slot(format).as_ref().map(Vec::len)
	}
	fn read(&mut self, format: Format, out: &mut [u8]) -> Option<usize> {
		self.reads += 1;
		let data = self.slot(format).clone()?;
		out.get_mut(..data.len())?.copy_from_slice(&data);
		Some(data.len())
	}
	fn stage(&mut self, format: Format, data: &[u8]) -> bool {
		if self.stage_ok {
			self.staged.push((format, data.to_vec()));
		}
		self.stage_ok
	}
	fn commit(&mut self) -> Option<u32> {
		if !self.commit_ok {
			return None;
		}
		self.commits += 1;
		let staged = std::mem::take(&mut self.staged);
		self.text = None;
		self.rtf = None;
		self.dib = None;
		for (format, data) in staged {
			*self.slot(format) = Some(data);
		}
		// Emptying and setting four formats moves the number by more than one, as ReactOS does.
		self.seq += 5;
		Some(self.seq)
	}
	fn discard(&mut self) {
		self.discards += 1;
		self.staged.clear();
	}
}

struct Rig {
	payload: Vec<u8>,
	lanes: Vec<Vec<u8>>,
	tx: Vec<u8>,
}

impl Rig {
	fn new() -> Rig {
		Rig {
			payload: vec![0; MAX_PAYLOAD],
			lanes: vec![
				vec![0; 1024],
				vec![0; CLIP_LANE],
				vec![0; 256],
				vec![0; 256],
				vec![0; 256],
			],
			tx: vec![0; CLIP_LANE],
		}
	}

	fn helper(&mut self) -> Helper<'_> {
		let [ctl, clip, file, time, display] = &mut self.lanes[..] else {
			unreachable!()
		};
		Helper::new(
			Buffers {
				payload: &mut self.payload,
				lanes: [ctl, clip, file, time, display],
				tx: &mut self.tx,
			},
			CLIP_LANE as u32,
		)
	}
}

/// What the page sends: messages as packets, with ids counted per direction.
struct Page {
	next: u32,
}

impl Page {
	fn message(
		&mut self,
		channel: Channel,
		op: &str,
		fields: &[(&str, &str)],
		data: &[u8],
	) -> Vec<u8> {
		let mut head = vec![0u8; 1024];
		let mut w = BodyWriter::new(&mut head, op.as_bytes()).unwrap();
		for (k, v) in fields {
			w.put(k.as_bytes(), v.as_bytes()).unwrap();
		}
		let n = w.finish(&[]).unwrap();
		head.truncate(n);
		head.extend_from_slice(data);
		let mut out = vec![0u8; head.len() + (head.len() / MAX_PAYLOAD + 2) * 80];
		let n = encode_message(&mut out, channel, self.next, &head).unwrap();
		self.next += 1;
		out.truncate(n);
		out
	}

	fn hello(&mut self) -> Vec<u8> {
		self.message(
			Channel::Ctl,
			"hello",
			&[
				("proto", "1"),
				("role", "page"),
				("app", "host-link/0.1.0"),
				("caps", "clip,file,time,display"),
			],
			&[],
		)
	}
}

/// Drives a rig to a quiet state: the helper has said hello and the page has answered it.
fn joined(rig: &mut Rig, f: impl FnOnce(&mut Helper<'_>, &mut FakeClip, &mut FakePort, &mut Page)) {
	let mut helper = rig.helper();
	let mut clip = FakeClip::new();
	let mut port = FakePort::default();
	let mut page = Page { next: 0 };
	helper.start(&mut clip);
	while helper.pump(&mut port, &mut clip) {}
	let first = port.messages();
	assert_eq!(first.len(), 1);
	helper.on_bytes(&page.hello(), &mut clip, 1000);
	while helper.pump(&mut port, &mut clip) {}
	port.messages();
	assert!(helper.session());
	f(&mut helper, &mut clip, &mut port, &mut page);
}

fn settle(helper: &mut Helper<'_>, clip: &mut FakeClip, now: u32) {
	helper.on_clipboard_change(now);
	helper.tick(clip, now + DEBOUNCE_MS);
}

fn drain(helper: &mut Helper<'_>, clip: &mut FakeClip, port: &mut FakePort) {
	let mut guard = 0;
	while helper.pump(port, clip) {
		guard += 1;
		assert!(guard < 10_000);
	}
}

#[test]
fn the_helper_says_hello_with_a_one_packet_window() {
	let mut rig = Rig::new();
	let mut helper = rig.helper();
	let mut clip = FakeClip::new();
	let mut port = FakePort::default();
	helper.start(&mut clip);
	assert!(helper.busy());
	while helper.pump(&mut port, &mut clip) {}
	let messages = port.messages();
	let hello = &messages[0];
	assert_eq!((hello.channel, hello.op.as_str()), (Channel::Ctl, "hello"));
	assert_eq!(hello.get("proto"), Some("1"));
	assert_eq!(hello.get("role"), Some("guest"));
	assert_eq!(hello.get("window"), Some("4352"));
	assert_eq!(hello.get("caps"), Some("clip"));
	assert_eq!(hello.get("max"), Some(CLIP_LANE.to_string().as_str()));
}

#[test]
fn a_page_hello_is_answered_and_opens_the_session_without_a_credit() {
	let mut rig = Rig::new();
	joined(&mut rig, |helper, clip, port, page| {
		// The hello of a page that restarted: answered again, still no credit for it.
		helper.on_bytes(&page.hello(), clip, 2000);
		drain(helper, clip, port);
		let messages = port.messages();
		assert_eq!(messages.len(), 1);
		assert_eq!(messages[0].op, "hello");
		assert_eq!(helper.counters().credits, 0);
	});
}

#[test]
fn every_packet_gets_a_credit_of_the_bytes_it_took() {
	let mut rig = Rig::new();
	joined(&mut rig, |helper, clip, port, page| {
		let frame = page.message(Channel::Clip, "get", &[("seq", "1"), ("fmt", "text")], &[]);
		helper.on_bytes(&frame, clip, 1500);
		drain(helper, clip, port);
		let messages = port.messages();
		let credit = messages.iter().find(|m| m.op == "credit").unwrap();
		assert_eq!(credit.get("n"), Some(frame.len().to_string().as_str()));
		// Two packets in two reads: a credit for each.
		let a = page.message(Channel::Clip, "get", &[("seq", "1"), ("fmt", "text")], &[]);
		helper.on_bytes(&a, clip, 1600);
		helper.on_bytes(&a, clip, 1601);
		drain(helper, clip, port);
		let credits: Vec<_> = port
			.messages()
			.into_iter()
			.filter(|m| m.op == "credit")
			.collect();
		assert_eq!(credits.len(), 2);
	});
}

#[test]
fn a_frame_split_across_reads_is_credited_once_it_is_whole() {
	let mut rig = Rig::new();
	joined(&mut rig, |helper, clip, port, page| {
		let frame = page.message(Channel::Clip, "get", &[("seq", "1"), ("fmt", "text")], &[]);
		let (a, b) = frame.split_at(20);
		helper.on_bytes(a, clip, 1500);
		drain(helper, clip, port);
		assert!(port.messages().iter().all(|m| m.op != "credit"));
		helper.on_bytes(b, clip, 1501);
		drain(helper, clip, port);
		let credits: Vec<_> = port
			.messages()
			.into_iter()
			.filter(|m| m.op == "credit")
			.collect();
		assert_eq!(credits.len(), 1);
		assert_eq!(credits[0].get("n"), Some(frame.len().to_string().as_str()));
	});
}

#[test]
fn a_copy_makes_one_offer_and_sends_its_text_and_rtf() {
	let mut rig = Rig::new();
	joined(&mut rig, |helper, clip, port, _| {
		clip.copy(Some("Zoë — café\r\nline two"), Some("{\\rtf1 hi}"), None);
		settle(helper, clip, 2000);
		drain(helper, clip, port);
		let messages = port.messages();
		let ops: Vec<_> = messages
			.iter()
			.map(|m| (m.channel, m.op.as_str()))
			.collect();
		assert_eq!(
			ops,
			vec![
				(Channel::Clip, "offer"),
				(Channel::Clip, "data"),
				(Channel::Clip, "data")
			]
		);
		let seq = clip.seq.to_string();
		assert_eq!(messages[0].get("seq"), Some(seq.as_str()));
		assert_eq!(messages[0].get("text"), Some("24"));
		assert_eq!(messages[0].get("rtf"), Some("10"));
		assert_eq!(messages[0].get("dib"), None);
		assert_eq!(messages[0].get("source"), Some("user"));
		assert_eq!(
			(messages[1].get("fmt"), messages[1].data.as_slice()),
			(Some("text"), "Zoë — café\r\nline two".as_bytes())
		);
		assert_eq!(
			(messages[2].get("fmt"), messages[2].data.as_slice()),
			(Some("rtf"), b"{\\rtf1 hi}".as_slice())
		);
	});
}

#[test]
fn the_doubled_text_message_makes_no_second_offer() {
	let mut rig = Rig::new();
	joined(&mut rig, |helper, clip, port, _| {
		clip.copy(Some("once"), None, None);
		// Two WM_DRAWCLIPBOARD, 15 ms apart, one sequence number: one offer.
		helper.on_clipboard_change(2000);
		helper.on_clipboard_change(2015);
		helper.tick(clip, 2030);
		// And a straggler after the offer, as the log showed.
		helper.on_clipboard_change(2031);
		helper.tick(clip, 2070);
		drain(helper, clip, port);
		let offers = port
			.messages()
			.into_iter()
			.filter(|m| m.op == "offer")
			.count();
		assert_eq!(offers, 1);
		assert_eq!(helper.counters().offers, 1);
	});
}

#[test]
fn what_was_on_the_clipboard_before_the_helper_started_is_not_offered() {
	let mut rig = Rig::new();
	let mut helper = rig.helper();
	let mut clip = FakeClip::new();
	clip.copy(Some("old"), None, None);
	let mut port = FakePort::default();
	let mut page = Page { next: 0 };
	helper.start(&mut clip);
	helper.on_bytes(&page.hello(), &mut clip, 10);
	// The message SetClipboardViewer sends on joining.
	settle(&mut helper, &mut clip, 20);
	drain(&mut helper, &mut clip, &mut port);
	assert!(port.messages().iter().all(|m| m.op != "offer"));
}

#[test]
fn nothing_is_offered_before_the_page_has_said_hello_or_while_clipboard_sharing_is_paused() {
	let mut rig = Rig::new();
	let mut helper = rig.helper();
	let mut clip = FakeClip::new();
	let mut port = FakePort::default();
	let mut page = Page { next: 0 };
	helper.start(&mut clip);
	clip.copy(Some("nobody listening"), None, None);
	settle(&mut helper, &mut clip, 100);
	while helper.pump(&mut port, &mut clip) {}
	assert!(port.messages().iter().all(|m| m.op != "offer"));

	helper.on_bytes(&page.hello(), &mut clip, 200);
	let pause = page.message(Channel::Ctl, "pause", &[("channel", "clip")], &[]);
	helper.on_bytes(&pause, &mut clip, 201);
	clip.copy(Some("sharing is off"), None, None);
	settle(&mut helper, &mut clip, 300);
	drain(&mut helper, &mut clip, &mut port);
	assert!(port.messages().iter().all(|m| m.op != "offer"));

	let resume = page.message(Channel::Ctl, "resume", &[("channel", "clip")], &[]);
	helper.on_bytes(&resume, &mut clip, 400);
	clip.copy(Some("sharing is on"), None, None);
	settle(&mut helper, &mut clip, 500);
	drain(&mut helper, &mut clip, &mut port);
	assert_eq!(
		port.messages().iter().filter(|m| m.op == "offer").count(),
		1
	);
}

#[test]
fn a_big_copy_is_offered_and_waits_for_a_get() {
	let mut rig = Rig::new();
	joined(&mut rig, |helper, clip, port, page| {
		let big = "x".repeat(TEXT_AUTO_MAX + 1);
		clip.copy(Some(&big), None, Some(vec![9; 3000]));
		settle(helper, clip, 2000);
		drain(helper, clip, port);
		let messages = port.messages();
		assert_eq!(messages.len(), 1, "an offer and nothing else");
		assert_eq!(
			messages[0].get("text"),
			Some((TEXT_AUTO_MAX + 1).to_string().as_str())
		);
		assert_eq!(messages[0].get("dib"), Some("3000"));
		let seq = clip.seq.to_string();
		let get = page.message(Channel::Clip, "get", &[("seq", &seq), ("fmt", "dib")], &[]);
		helper.on_bytes(&get, clip, 2100);
		drain(helper, clip, port);
		let messages = port.messages();
		let data = messages.iter().find(|m| m.op == "data").unwrap();
		assert_eq!((data.get("fmt"), data.data.len()), (Some("dib"), 3000));
	});
}

#[test]
fn a_one_mebibyte_message_goes_out_one_packet_at_a_time() {
	let mut rig = Rig::new();
	joined(&mut rig, |helper, clip, port, page| {
		let big: String = (0..(1 << 20) / 8)
			.map(|n| std::format!("{n:06}\r\n"))
			.collect();
		assert_eq!(big.len(), 1 << 20);
		clip.copy(Some(&big), None, None);
		settle(helper, clip, 2000);
		drain(helper, clip, port);
		port.messages();
		let seq = clip.seq.to_string();
		let get = page.message(Channel::Clip, "get", &[("seq", &seq), ("fmt", "text")], &[]);
		helper.on_bytes(&get, clip, 2100);
		let mut calls = 0;
		while helper.pump(port, clip) {
			calls += 1;
		}
		let (messages, packets) = port.take();
		let data = messages.iter().find(|m| m.op == "data").unwrap();
		assert_eq!(data.data, big.as_bytes());
		let data_packets: Vec<_> = packets
			.iter()
			.filter(|p| p.0 == Channel::Clip && p.3 > 0)
			.collect();
		assert!(data_packets.len() >= 256);
		assert!(data_packets.iter().all(|p| p.3 <= MAX_PAYLOAD));
		assert!(
			data_packets[..data_packets.len() - 1].iter().all(|p| p.2),
			"M on all but the last"
		);
		assert!(!data_packets[data_packets.len() - 1].2);
		assert!(
			calls >= 256,
			"one packet per call, so the port is read in between"
		);
	});
}

#[test]
fn a_get_for_a_clipboard_that_changed_or_has_no_such_format_is_refused() {
	let mut rig = Rig::new();
	joined(&mut rig, |helper, clip, port, page| {
		clip.copy(Some("a"), None, None);
		settle(helper, clip, 2000);
		drain(helper, clip, port);
		port.messages();
		let seq = clip.seq;
		let stale = page.message(
			Channel::Clip,
			"get",
			&[("seq", &(seq - 1).to_string()), ("fmt", "text")],
			&[],
		);
		let absent = page.message(
			Channel::Clip,
			"get",
			&[("seq", &seq.to_string()), ("fmt", "rtf")],
			&[],
		);
		helper.on_bytes(&stale, clip, 2100);
		helper.on_bytes(&absent, clip, 2101);
		drain(helper, clip, port);
		let reasons: Vec<_> = port
			.messages()
			.iter()
			.filter(|m| m.op == "refused")
			.map(|m| m.get("reason").unwrap().to_string())
			.collect();
		assert_eq!(reasons, vec!["changed", "absent"]);
	});
}

#[test]
fn a_set_gathers_its_parts_then_writes_the_clipboard_once_and_says_ok() {
	let mut rig = Rig::new();
	joined(&mut rig, |helper, clip, port, page| {
		let set = page.message(
			Channel::Clip,
			"set",
			&[("seq", "41"), ("formats", "text,rtf,dib")],
			&[],
		);
		helper.on_bytes(&set, clip, 3000);
		for (fmt, data) in [
			("text", &b"Zo\xC3\xAB"[..]),
			("rtf", b"{\\rtf1}"),
			("dib", &[1, 2, 3, 4]),
		] {
			assert_eq!(clip.commits, 0, "nothing is written before the last part");
			let part = page.message(Channel::Clip, "part", &[("seq", "41"), ("fmt", fmt)], data);
			helper.on_bytes(&part, clip, 3001);
		}
		assert_eq!(clip.commits, 1);
		assert_eq!(clip.text.as_deref(), Some("Zoë".as_bytes()));
		assert_eq!(clip.rtf.as_deref(), Some(&b"{\\rtf1}"[..]));
		assert_eq!(clip.dib.as_deref(), Some(&[1u8, 2, 3, 4][..]));
		drain(helper, clip, port);
		let ok: Vec<_> = port
			.messages()
			.into_iter()
			.filter(|m| m.op == "ok")
			.collect();
		assert_eq!(ok.len(), 1);
		assert_eq!(ok[0].get("seq"), Some("41"));
		assert_eq!(helper.counters().sets, 1);
	});
}

#[test]
fn the_helpers_own_clipboard_write_is_not_offered_back() {
	let mut rig = Rig::new();
	joined(&mut rig, |helper, clip, port, page| {
		let set = page.message(
			Channel::Clip,
			"set",
			&[("seq", "1"), ("formats", "text")],
			&[],
		);
		let part = page.message(
			Channel::Clip,
			"part",
			&[("seq", "1"), ("fmt", "text")],
			b"from the page",
		);
		helper.on_bytes(&set, clip, 3000);
		helper.on_bytes(&part, clip, 3001);
		// The viewer messages for our own write, several of them, arrive after the commit.
		helper.on_clipboard_change(3002);
		helper.on_clipboard_change(3010);
		helper.tick(clip, 3040);
		drain(helper, clip, port);
		let ops: Vec<_> = port.messages().into_iter().map(|m| m.op).collect();
		assert!(ops.contains(&"ok".to_string()));
		assert!(!ops.contains(&"offer".to_string()), "{ops:?}");
		// The next copy by anyone else is offered.
		clip.copy(Some("next"), None, None);
		settle(helper, clip, 4000);
		drain(helper, clip, port);
		assert_eq!(
			port.messages().iter().filter(|m| m.op == "offer").count(),
			1
		);
	});
}

#[test]
fn a_set_that_cannot_be_done_is_refused_and_leaves_nothing_behind() {
	let mut rig = Rig::new();
	joined(&mut rig, |helper, clip, port, page| {
		clip.stage_ok = false;
		let set = page.message(
			Channel::Clip,
			"set",
			&[("seq", "5"), ("formats", "text")],
			&[],
		);
		let part = page.message(
			Channel::Clip,
			"part",
			&[("seq", "5"), ("fmt", "text")],
			b"x",
		);
		helper.on_bytes(&set, clip, 3000);
		helper.on_bytes(&part, clip, 3001);
		clip.stage_ok = true;
		clip.commit_ok = false;
		let set = page.message(
			Channel::Clip,
			"set",
			&[("seq", "6"), ("formats", "text")],
			&[],
		);
		let part = page.message(
			Channel::Clip,
			"part",
			&[("seq", "6"), ("fmt", "text")],
			b"x",
		);
		helper.on_bytes(&set, clip, 3100);
		helper.on_bytes(&part, clip, 3101);
		// A part of a set that was never announced, and a set with no known format.
		let stray = page.message(
			Channel::Clip,
			"part",
			&[("seq", "9"), ("fmt", "text")],
			b"x",
		);
		let empty = page.message(
			Channel::Clip,
			"set",
			&[("seq", "10"), ("formats", "bitmap")],
			&[],
		);
		helper.on_bytes(&stray, clip, 3200);
		helper.on_bytes(&empty, clip, 3201);
		drain(helper, clip, port);
		let refused: Vec<_> = port
			.messages()
			.iter()
			.filter(|m| m.op == "refused")
			.map(|m| {
				(
					m.get("seq").unwrap().to_string(),
					m.get("reason").unwrap().to_string(),
				)
			})
			.collect();
		let want = [
			("5", "failed"),
			("6", "busy"),
			("9", "no-set"),
			("10", "no-formats"),
		];
		assert_eq!(
			refused,
			want.iter()
				.map(|(a, b)| (a.to_string(), b.to_string()))
				.collect::<Vec<_>>()
		);
		assert!(clip.text.is_none());
		assert_eq!(helper.counters().refusals, 4);
	});
}

#[test]
fn a_part_over_its_limit_is_refused_before_it_is_staged() {
	let mut rig = Rig::new();
	joined(&mut rig, |helper, clip, port, page| {
		let set = page.message(
			Channel::Clip,
			"set",
			&[("seq", "2"), ("formats", "text")],
			&[],
		);
		let part = page.message(
			Channel::Clip,
			"part",
			&[("seq", "2"), ("fmt", "text")],
			&vec![b'a'; TEXT_MAX + 1],
		);
		helper.on_bytes(&set, clip, 3000);
		helper.on_bytes(&part, clip, 3001);
		drain(helper, clip, port);
		let messages = port.messages();
		let refused = messages.iter().find(|m| m.op == "refused").unwrap();
		assert_eq!(refused.get("reason"), Some("too-large"));
		assert!(clip.staged.is_empty());
	});
}

#[test]
fn a_one_mebibyte_set_arrives_whole() {
	let mut rig = Rig::new();
	joined(&mut rig, |helper, clip, port, page| {
		let big: Vec<u8> = (0..1usize << 20).map(|n| b'a' + (n % 26) as u8).collect();
		let set = page.message(
			Channel::Clip,
			"set",
			&[("seq", "3"), ("formats", "text")],
			&[],
		);
		helper.on_bytes(&set, clip, 3000);
		let part = page.message(
			Channel::Clip,
			"part",
			&[("seq", "3"), ("fmt", "text")],
			&big,
		);
		// One packet at a time, as the page sends them, each credited.
		let mut taken_credits = 0;
		for chunk in part.chunks(4142) {
			helper.on_bytes(chunk, clip, 3001);
			drain(helper, clip, port);
			taken_credits += port.messages().iter().filter(|m| m.op == "credit").count();
		}
		assert!(taken_credits >= 256);
		assert_eq!(clip.text.as_deref(), Some(&big[..]));
	});
}

#[test]
fn a_new_hello_mid_transfer_drops_the_old_work_and_the_helper_carries_on() {
	let mut rig = Rig::new();
	joined(&mut rig, |helper, clip, port, page| {
		// A set with one part in and one to come, and a big message half sent.
		let set = page.message(
			Channel::Clip,
			"set",
			&[("seq", "8"), ("formats", "text,rtf")],
			&[],
		);
		let part = page.message(
			Channel::Clip,
			"part",
			&[("seq", "8"), ("fmt", "text")],
			b"half",
		);
		helper.on_bytes(&set, clip, 3000);
		helper.on_bytes(&part, clip, 3001);
		let big = "y".repeat(TEXT_AUTO_MAX);
		clip.copy(Some(&big), None, None);
		settle(helper, clip, 3100);
		helper.pump(port, clip);
		helper.pump(port, clip);
		helper.pump(port, clip);
		assert!(helper.busy());
		let discards = clip.discards;

		// The page is reloaded: a new hello.
		helper.on_bytes(&page.hello(), clip, 3200);
		assert!(clip.discards > discards, "the staged part is dropped");
		drain(helper, clip, port);
		let (messages, _) = port.take();
		assert_eq!(
			messages.last().map(|m| m.op.as_str()),
			Some("hello"),
			"answered, and the rest of the old message is not sent"
		);
		assert_eq!(clip.commits, 0);

		// The old set's other part arriving now is refused, a new copy is offered.
		let late = page.message(
			Channel::Clip,
			"part",
			&[("seq", "8"), ("fmt", "rtf")],
			b"{}",
		);
		helper.on_bytes(&late, clip, 3300);
		clip.copy(Some("after"), None, None);
		settle(helper, clip, 3400);
		drain(helper, clip, port);
		let ops: Vec<_> = port.messages().into_iter().map(|m| m.op).collect();
		assert!(ops.contains(&"refused".to_string()));
		assert!(ops.contains(&"offer".to_string()));
	});
}

#[test]
fn a_set_whose_parts_stop_coming_is_dropped() {
	let mut rig = Rig::new();
	joined(&mut rig, |helper, clip, _, page| {
		let set = page.message(
			Channel::Clip,
			"set",
			&[("seq", "8"), ("formats", "text,rtf")],
			&[],
		);
		let part = page.message(
			Channel::Clip,
			"part",
			&[("seq", "8"), ("fmt", "text")],
			b"half",
		);
		helper.on_bytes(&set, clip, 3000);
		helper.on_bytes(&part, clip, 3001);
		assert_eq!(helper.next_due(3002), Some(SET_TIMEOUT_MS - 1));
		helper.tick(clip, 3001 + SET_TIMEOUT_MS - 1);
		assert_eq!(clip.staged.len(), 1);
		helper.tick(clip, 3001 + SET_TIMEOUT_MS);
		assert!(clip.staged.is_empty());
		assert_eq!(helper.next_due(20_000), None);
	});
}

#[test]
fn a_half_frame_with_nothing_more_coming_is_given_up_so_the_next_hello_parses() {
	let mut rig = Rig::new();
	joined(&mut rig, |helper, clip, port, page| {
		let frame = page.message(
			Channel::Clip,
			"part",
			&[("seq", "1"), ("fmt", "text")],
			&vec![b'z'; 3000],
		);
		helper.on_bytes(&frame[..1000], clip, 5000);
		helper.tick(clip, 5000 + PARTIAL_MS - 1);
		helper.tick(clip, 5000 + PARTIAL_MS);
		helper.on_bytes(&page.hello(), clip, 7000);
		drain(helper, clip, port);
		assert_eq!(
			port.messages().last().map(|m| m.op.clone()),
			Some("hello".to_string())
		);
	});
}

#[test]
fn a_bad_crc_is_answered_with_an_error_and_noise_is_ignored() {
	let mut rig = Rig::new();
	joined(&mut rig, |helper, clip, port, page| {
		let mut frame = page.message(Channel::Clip, "get", &[("seq", "1"), ("fmt", "text")], &[]);
		let last = frame.len() - 1;
		frame[last] ^= 1;
		helper.on_bytes(b"C:\\> dir\r\n", clip, 6000);
		helper.on_bytes(&frame, clip, 6001);
		drain(helper, clip, port);
		let messages = port.messages();
		let error = messages.iter().find(|m| m.op == "error").unwrap();
		assert_eq!(error.get("code"), Some("crc"));
		assert!(
			messages.iter().any(|m| m.op == "credit"),
			"the bytes were still taken off the port"
		);
		assert_eq!(helper.counters().errors_sent, 1);
	});
}

#[test]
fn unknown_channels_and_ops_are_answered_once_and_ignored() {
	let mut rig = Rig::new();
	joined(&mut rig, |helper, clip, port, page| {
		let file = page.message(Channel::File, "begin", &[("id", "1")], &[]);
		let weird = page.message(Channel::Clip, "frobnicate", &[], &[]);
		helper.on_bytes(&file, clip, 6000);
		helper.on_bytes(&weird, clip, 6001);
		drain(helper, clip, port);
		let codes: Vec<_> = port
			.messages()
			.iter()
			.filter(|m| m.op == "error")
			.map(|m| m.get("code").unwrap().to_string())
			.collect();
		assert_eq!(codes, vec!["unknown", "unknown"]);
	});
}

#[test]
fn a_newer_protocol_is_answered_with_version_and_the_helper_stays_quiet() {
	let mut rig = Rig::new();
	let mut helper = rig.helper();
	let mut clip = FakeClip::new();
	let mut port = FakePort::default();
	let mut page = Page { next: 0 };
	helper.start(&mut clip);
	while helper.pump(&mut port, &mut clip) {}
	port.messages();
	let hello = page.message(
		Channel::Ctl,
		"hello",
		&[("proto", "2"), ("role", "page")],
		&[],
	);
	helper.on_bytes(&hello, &mut clip, 10);
	while helper.pump(&mut port, &mut clip) {}
	let messages = port.messages();
	assert_eq!(messages.len(), 1);
	assert_eq!(messages[0].get("code"), Some("version"));
	assert!(!helper.session());
}

#[test]
fn a_failed_port_stops_the_helper_without_a_panic() {
	let mut rig = Rig::new();
	joined(&mut rig, |helper, clip, port, _| {
		clip.copy(Some("x"), None, None);
		settle(helper, clip, 2000);
		port.fail = true;
		let mut guard = 0;
		while helper.pump(port, clip) {
			guard += 1;
			assert!(guard < 100);
		}
		assert!(helper.failed());
		assert!(!helper.busy());
	});
}

#[test]
fn the_bye_ends_the_session_and_garbage_never_panics() {
	let mut rig = Rig::new();
	joined(&mut rig, |helper, clip, port, page| {
		let mut noise = Vec::new();
		let mut x = 0x1234_5678u32;
		for _ in 0..20_000 {
			x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
			noise.push((x >> 24) as u8);
		}
		for chunk in noise.chunks(333) {
			helper.on_bytes(chunk, clip, 9000);
			drain(helper, clip, port);
			port.bytes.clear();
		}
		let _ = ParseError::Trailer { offset: 0 };
		// A frame starts at the start of a line, so the noise's last line is ended first.
		helper.on_bytes(b"\r\n", clip, 9050);
		let bye = page.message(Channel::Ctl, "bye", &[], &[]);
		helper.on_bytes(&bye, clip, 9100);
		assert!(!helper.session());
	});
}
