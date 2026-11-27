// Fuzz and property tests: framing errors, truncated frames, bad checksums, credit exhaustion and the size limits.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![allow(
	clippy::unwrap_used,
	clippy::expect_used,
	clippy::panic,
	clippy::indexing_slicing
)]

use proptest::prelude::*;
use quill_hostlink::*;
use std::fs;
use std::path::PathBuf;

struct Rng(u64);

impl Rng {
	fn next(&mut self) -> u64 {
		self.0 ^= self.0 << 13;
		self.0 ^= self.0 >> 7;
		self.0 ^= self.0 << 17;
		self.0
	}

	fn below(&mut self, n: usize) -> usize {
		(self.next() % n as u64) as usize
	}
}

fn vectors() -> Vec<Vec<u8>> {
	let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/host-link/vectors");
	let mut paths: Vec<_> = fs::read_dir(dir)
		.unwrap()
		.map(|e| e.unwrap().path())
		.filter(|p| p.extension().is_some_and(|e| e == "bin"))
		.collect();
	paths.sort();
	paths.iter().map(|p| fs::read(p).unwrap()).collect()
}

/// A flat record of what a parser said, to compare runs.
fn record(bytes: &[u8], chunks: &[usize]) -> Vec<String> {
	let mut payload = vec![0u8; MAX_PAYLOAD];
	let mut parser = Parser::new(&mut payload);
	let mut out: Vec<String> = Vec::new();
	let mut file_bytes: Vec<u8> = Vec::new();
	let mut at = 0;
	let mut sizes = chunks.iter().copied().cycle();
	while at < bytes.len() {
		let end = if chunks.is_empty() {
			bytes.len()
		} else {
			(at + sizes.next().unwrap().max(1)).min(bytes.len())
		};
		parser.feed(&bytes[at..end], |event| {
			if let Event::FileData(d) = &event {
				file_bytes.extend_from_slice(d);
				return;
			}
			if !file_bytes.is_empty() {
				out.push(format!("d {file_bytes:?}"));
				file_bytes.clear();
			}
			let line = match event {
				Event::Packet {
					channel,
					id,
					more,
					payload,
				} => {
					assert!(payload.len() <= MAX_PAYLOAD);
					format!("p {} {id} {more} {payload:?}", channel.name())
				}
				Event::FileBegin { size, name } => format!("b {size} {name:?}"),
				Event::FileData(_) => return,
				Event::FileEnd { sha256 } => format!("e {sha256:?}"),
				Event::Error(e) => format!("x {e:?}"),
			};
			out.push(line);
		});
		at = end;
	}
	if !file_bytes.is_empty() {
		out.push(format!("d {file_bytes:?}"));
	}
	out
}

#[test]
fn ten_thousand_seeded_inputs_never_panic_and_split_the_same_way() {
	let corpus = vectors();
	let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
	for round in 0..10_000 {
		let mut input: Vec<u8> = match round % 4 {
			// Pure noise.
			0 => (0..rng.below(600)).map(|_| rng.next() as u8).collect(),
			// A vector with some bytes changed.
			1 => {
				let mut v = corpus[rng.below(corpus.len())].clone();
				for _ in 0..=rng.below(6) {
					if !v.is_empty() {
						let at = rng.below(v.len());
						v[at] = rng.next() as u8;
					}
				}
				v
			}
			// Several vectors cut at random places and joined.
			2 => {
				let mut v = Vec::new();
				for _ in 0..=rng.below(4) {
					let part = &corpus[rng.below(corpus.len())];
					let cut = rng.below(part.len() + 1);
					v.extend_from_slice(&part[..cut]);
				}
				v
			}
			// Text that looks like frames.
			_ => {
				let pieces: [&[u8]; 8] = [
					b"LHQ-PKT 1 ctl 1 E 0 ",
					b"LHQ-FILE 1 ",
					b"\r\n",
					b"\n",
					b"LHQ-END",
					b" 4096 ",
					b"clip",
					b"00000000",
				];
				let mut v = Vec::new();
				for _ in 0..rng.below(40) {
					v.extend_from_slice(pieces[rng.below(pieces.len())]);
				}
				v
			}
		};
		input.truncate(20_000);
		let whole = record(&input, &[]);
		let chunks: Vec<usize> = (0..16).map(|_| rng.below(50) + 1).collect();
		assert_eq!(
			whole,
			record(&input, &chunks),
			"round {round}: a split changed what the parser said"
		);
	}
}

#[test]
fn a_truncated_vector_says_a_prefix_of_what_the_whole_says() {
	for bytes in vectors() {
		let whole = record(&bytes, &[]);
		let stride = bytes.len() / 200 + 1;
		let mut cut = 0;
		while cut < bytes.len() {
			let part = record(&bytes[..cut], &[]);
			// The last event of a prefix may be a file's data cut short; every other event is one the whole run also has.
			let settled = part.len().saturating_sub(1);
			assert!(
				whole.starts_with(&part[..settled]),
				"a prefix of {cut} bytes said something the whole did not"
			);
			cut += stride;
		}
	}
}

fn one_packet(id: u32, payload: &[u8]) -> Vec<u8> {
	let mut out = vec![0u8; 4200];
	let n = encode_packet(&mut out, Channel::Clip, id, false, payload).unwrap();
	out.truncate(n);
	out
}

#[test]
fn any_changed_byte_in_a_packet_is_a_header_error_or_a_crc_error_never_a_changed_packet() {
	let payload = b"op=ok\nseq=7\n\n";
	let good = one_packet(7, payload);
	for at in 0..good.len() {
		for delta in [1u8, 0x20, 0x80, 0xFF] {
			let mut bad = good.clone();
			bad[at] ^= delta;
			for event in record(&bad, &[]) {
				assert!(
					!event.starts_with("p ") || bad == good,
					"byte {at} xor {delta:#x} still gave a packet: {event}"
				);
			}
		}
	}
	assert_eq!(record(&good, &[]).len(), 1);
}

#[test]
fn a_wrong_crc_is_reported_with_the_channel_and_id_and_the_next_frame_still_parses() {
	let mut bytes = one_packet(5, b"abc");
	let last = bytes.len() - 1;
	bytes[last] ^= 1;
	bytes.extend_from_slice(&one_packet(6, b"def"));
	let said = record(&bytes, &[]);
	assert_eq!(said.len(), 2);
	assert!(
		said[0].contains("Crc") && said[0].contains("Clip") && said[0].contains("id: 5"),
		"{}",
		said[0]
	);
	assert!(said[1].starts_with("p clip 6 false"));
}

#[test]
fn size_limits_are_enforced_by_the_encoder_and_the_parser() {
	let mut out = vec![0u8; 5000];
	assert_eq!(
		encode_packet(&mut out, Channel::Clip, 1, false, &[0u8; MAX_PAYLOAD + 1]),
		Err(EncodeError::BadArgument)
	);
	assert!(encode_packet(&mut out, Channel::Clip, 1, false, &[0u8; MAX_PAYLOAD]).is_ok());
	assert_eq!(
		encode_packet(&mut out, Channel::Unknown, 1, false, b""),
		Err(EncodeError::BadArgument)
	);
	assert_eq!(
		encode_packet(&mut out[..10], Channel::Clip, 1, false, b""),
		Err(EncodeError::NoRoom)
	);
	assert_eq!(
		encode_file_header(&mut out, FILE_MAX_SIZE + 1, b"x"),
		Err(EncodeError::BadArgument)
	);
	assert_eq!(
		encode_file_header(&mut out, 1, &[b'a'; FILE_NAME_MAX + 1]),
		Err(EncodeError::BadArgument)
	);
	let long_name = [b'a'; FILE_NAME_MAX];
	let n = encode_file_header(&mut out, FILE_MAX_SIZE, &long_name).unwrap();
	assert!(n <= FILE_HEADER_MAX);
	assert_eq!(record(&out[..n], &[]).len(), 1);
	// The longest packet header is within its limit.
	let longest = {
		let mut v = vec![0u8; 5000];
		let n = encode_packet(&mut v, Channel::Display, u32::MAX, true, &[]).unwrap();
		v.truncate(n);
		v
	};
	assert!(longest.len() <= PKT_HEADER_MAX);
}

#[test]
fn a_parser_with_a_short_payload_buffer_refuses_a_long_packet_without_panicking() {
	let mut small = vec![0u8; 100];
	let mut parser = Parser::new(&mut small);
	let mut said = 0;
	parser.feed(&one_packet(1, &[b'x'; 500]), |_| said += 1);
	assert_eq!(said, 1);
}

#[test]
fn messages_over_the_buffer_or_max_are_refused_before_the_buffer_overflows() {
	let mut buffer = vec![0u8; 100];
	let mut lane = Assembler::new(&mut buffer, 1_000_000);
	assert_eq!(lane.push(1, true, &[b'a'; 60]), Step::Pending);
	assert_eq!(
		lane.push(1, true, &[b'a'; 60]),
		Step::Error(ErrorCode::TooLarge)
	);
	// The rest of a refused message is dropped, and the next id starts clean.
	assert_eq!(lane.push(1, true, b"zz"), Step::Pending);
	assert_eq!(lane.push(1, false, b"zz"), Step::Pending);
	assert!(matches!(
		lane.push(2, false, b"op=bye\n\n"),
		Step::Message(_)
	));
}

#[test]
fn a_message_of_many_packets_reassembles_and_a_new_id_drops_the_open_one() {
	let mut body = vec![0u8; 20_000];
	let mut w = BodyWriter::new(&mut body, b"data").unwrap();
	w.put(b"fmt", b"rtf").unwrap();
	let len = w.finish(&vec![7u8; 15_000]).unwrap();
	let mut wire = vec![0u8; 30_000];
	let n = encode_message(&mut wire, Channel::Clip, 3, &body[..len]).unwrap();
	let mut payload = vec![0u8; MAX_PAYLOAD];
	let mut parser = Parser::new(&mut payload);
	let mut buffer = vec![0u8; 20_000];
	let mut lane = Assembler::new(&mut buffer, DEFAULT_MAX_MESSAGE);
	let mut got = 0;
	let mut packets = 0;
	parser.feed(&wire[..n], |event| {
		if let Event::Packet {
			id, more, payload, ..
		} = event
		{
			packets += 1;
			if let Step::Message(m) = lane.push(id, more, payload) {
				assert_eq!(m.op(), b"data");
				assert_eq!(m.get(b"fmt"), Some(&b"rtf"[..]));
				assert_eq!(m.data().len(), 15_000);
				got += 1;
			}
		}
	});
	assert_eq!((packets, got), (4, 1));
	assert_eq!(lane.push(10, true, b"op=a\n\n"), Step::Pending);
	assert!(matches!(
		lane.push(11, false, b"op=b\n\n"),
		Step::Message(_)
	));
	assert_eq!(lane.dropped(), 1);
}

#[test]
fn body_limits() {
	assert_eq!(
		Body::parse(b"op=x\nk=1\n\n").unwrap().get_u32(b"k"),
		Some(1)
	);
	assert_eq!(
		Body::parse(b"op=x\nk=4294967296\n\n")
			.unwrap()
			.get_u32(b"k"),
		None
	);
	assert_eq!(Body::parse(b"op=x\nk=-1\n\n").unwrap().get_u32(b"k"), None);
	assert_eq!(Body::parse(b"op=\n\n"), Err(BodyError::NoOp));
	assert_eq!(Body::parse(b"Op=x\n\n"), Err(BodyError::BadField));
	let mut fields = String::from("op=x\n");
	for n in 0..FIELDS_MAX {
		fields.push_str(&format!("k{n}=1\n"));
	}
	fields.push('\n');
	assert_eq!(
		Body::parse(fields.as_bytes()),
		Err(BodyError::TooManyFields)
	);
	let long_value = format!("op=x\nk={}\n\n", "v".repeat(VALUE_MAX + 1));
	assert_eq!(Body::parse(long_value.as_bytes()), Err(BodyError::BadField));
	let ok_value = format!("op=x\nk={}\n\n", "v".repeat(VALUE_MAX));
	assert!(Body::parse(ok_value.as_bytes()).is_ok());
	let long_key = format!("op=x\n{}=1\n\n", "k".repeat(KEY_MAX + 1));
	assert_eq!(Body::parse(long_key.as_bytes()), Err(BodyError::BadField));
	let mut small = [0u8; 8];
	let mut w = BodyWriter::new(&mut small, b"ok").unwrap();
	assert_eq!(w.put(b"seq", b"1234567"), Err(BodyError::NoRoom));
}

#[test]
fn credit_exhaustion_and_restoration() {
	let mut credit = Credit::new(DEFAULT_WINDOW);
	let frame = 4_142;
	let mut sent = 0;
	while credit.try_spend(frame) {
		sent += 1;
	}
	assert_eq!(sent, 3, "16,384 bytes of window carry three longest frames");
	assert!(credit.available() < frame);
	assert!(!credit.try_spend(frame), "no sending past the window");
	credit.grant(4_096);
	assert!(credit.try_spend(frame));
	// A credit larger than what is outstanding cannot raise the window.
	credit.grant(1_000_000);
	assert_eq!(credit.available(), DEFAULT_WINDOW);
	credit.try_spend(10);
	credit.reset(100);
	assert_eq!(
		credit.window(),
		MIN_WINDOW,
		"a tiny window is raised to one whole packet"
	);
	assert_eq!(credit.outstanding(), 0);
}

#[test]
fn the_reporter_owes_a_credit_every_four_kib() {
	let mut reporter = CreditReporter::new();
	assert_eq!(reporter.took(4_000), None);
	assert_eq!(reporter.took(200), Some(4_200));
	assert_eq!(reporter.took(10), None);
	assert_eq!(reporter.took(u32::MAX), Some(u32::MAX));
}

proptest! {
	#[test]
	fn credit_never_exceeds_its_window(ops in proptest::collection::vec((any::<bool>(), 0u32..20_000), 0..200)) {
		let mut credit = Credit::new(DEFAULT_WINDOW);
		for (spend, n) in ops {
			if spend {
				let before = credit.available();
				let ok = credit.try_spend(n);
				prop_assert_eq!(ok, n <= before);
			} else {
				credit.grant(n);
			}
			prop_assert!(credit.outstanding() <= credit.window());
		}
	}

	#[test]
	fn encoded_messages_decode_to_themselves(channel in 0usize..5, id in any::<u32>(), data in proptest::collection::vec(any::<u8>(), 0..10_000), seq in any::<u32>()) {
		let channel = Channel::PRIORITY[channel];
		let mut body = vec![0u8; 10_100];
		let mut w = BodyWriter::new(&mut body, b"data").unwrap();
		w.put_u32(b"seq", seq).unwrap();
		let len = w.finish(&data).unwrap();
		let mut wire = vec![0u8; 20_000];
		let n = encode_message(&mut wire, channel, id, &body[..len]).unwrap();
		let mut payload = vec![0u8; MAX_PAYLOAD];
		let mut parser = Parser::new(&mut payload);
		let mut buffer = vec![0u8; 16_384];
		let mut lane = Assembler::new(&mut buffer, DEFAULT_MAX_MESSAGE);
		let mut found = false;
		parser.feed(&wire[..n], |event| {
			if let Event::Packet { channel: c, id: i, more, payload } = event {
				assert_eq!((c, i), (channel, id));
				if let Step::Message(m) = lane.push(i, more, payload) {
					assert_eq!(m.get_u32(b"seq"), Some(seq));
					assert_eq!(m.data(), &data[..]);
					found = true;
				}
			}
		});
		prop_assert!(found);
	}

	#[test]
	fn random_bytes_never_panic_and_split_alike(bytes in proptest::collection::vec(any::<u8>(), 0..2_000), cut in 1usize..300) {
		prop_assert_eq!(record(&bytes, &[]), record(&bytes, &[cut]));
	}
}

#[test]
fn file_frames_round_trip_through_their_encoders() {
	let mut wire = vec![0u8; 400];
	let mut n = encode_file_header(&mut wire, 5, "Caf\u{e9}.txt".as_bytes()).unwrap();
	wire[n..n + 5].copy_from_slice(b"hello");
	n += 5;
	let sha = [b'a'; 64];
	n += encode_file_trailer(&mut wire[n..], Some(&sha)).unwrap();
	assert_eq!(
		encode_file_trailer(&mut wire[n..], Some(b"ABC")),
		Err(EncodeError::BadArgument)
	);
	let said = record(&wire[..n], &[]);
	assert_eq!(said.len(), 3);
	assert!(said[2].contains("Some"));
}
