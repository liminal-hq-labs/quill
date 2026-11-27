// Every shared vector in tests/host-link/vectors, fed whole, a byte at a time, split at every point and split at random.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![allow(
	clippy::unwrap_used,
	clippy::expect_used,
	clippy::panic,
	clippy::indexing_slicing
)]

use quill_hostlink::{
	Channel, Event, Mux, Output, ParseError, Parser, DEFAULT_MAX_MESSAGE, MAX_PAYLOAD,
};
use std::fs;
use std::path::PathBuf;

// A just-enough JSON reader: the vectors use objects, arrays, strings, integers, booleans and null.
#[derive(Clone, Debug, PartialEq)]
enum Json {
	Null,
	Bool(bool),
	Num(i64),
	Str(String),
	Arr(Vec<Json>),
	Obj(Vec<(String, Json)>),
}

impl Json {
	fn get(&self, key: &str) -> Option<&Json> {
		match self {
			Json::Obj(fields) => fields.iter().find(|(k, _)| k == key).map(|(_, v)| v),
			_ => None,
		}
	}

	/// A form with the keys sorted, so two values compare as text whatever order the keys came in.
	fn canon(&self) -> String {
		match self {
			Json::Null => "null".into(),
			Json::Bool(b) => b.to_string(),
			Json::Num(n) => n.to_string(),
			Json::Str(s) => format!("{s:?}"),
			Json::Arr(items) => format!(
				"[{}]",
				items.iter().map(Json::canon).collect::<Vec<_>>().join(",")
			),
			Json::Obj(fields) => {
				let mut parts: Vec<String> = fields
					.iter()
					.map(|(k, v)| format!("{k:?}:{}", v.canon()))
					.collect();
				parts.sort();
				format!("{{{}}}", parts.join(","))
			}
		}
	}
}

struct Reader<'a> {
	text: &'a [u8],
	at: usize,
}

impl Reader<'_> {
	fn skip(&mut self) {
		while self
			.text
			.get(self.at)
			.is_some_and(|b| b.is_ascii_whitespace())
		{
			self.at += 1;
		}
	}

	fn eat(&mut self, b: u8) {
		self.skip();
		assert_eq!(
			self.text[self.at], b,
			"JSON: expected {:?} at {}",
			b as char, self.at
		);
		self.at += 1;
	}

	fn value(&mut self) -> Json {
		self.skip();
		match self.text[self.at] {
			b'{' => {
				self.at += 1;
				let mut fields = Vec::new();
				loop {
					self.skip();
					if self.text[self.at] == b'}' {
						self.at += 1;
						return Json::Obj(fields);
					}
					if !fields.is_empty() {
						self.eat(b',');
					}
					let Json::Str(key) = self.value() else {
						panic!("JSON: key")
					};
					self.eat(b':');
					fields.push((key, self.value()));
				}
			}
			b'[' => {
				self.at += 1;
				let mut items = Vec::new();
				loop {
					self.skip();
					if self.text[self.at] == b']' {
						self.at += 1;
						return Json::Arr(items);
					}
					if !items.is_empty() {
						self.eat(b',');
					}
					items.push(self.value());
				}
			}
			b'"' => {
				self.at += 1;
				let mut out = Vec::new();
				loop {
					let b = self.text[self.at];
					self.at += 1;
					match b {
						b'"' => return Json::Str(String::from_utf8(out).unwrap()),
						b'\\' => {
							let e = self.text[self.at];
							self.at += 1;
							match e {
								b'n' => out.push(b'\n'),
								b'r' => out.push(b'\r'),
								b't' => out.push(b'\t'),
								b'"' | b'\\' | b'/' => out.push(e),
								b'u' => {
									let code = u32::from_str_radix(
										std::str::from_utf8(&self.text[self.at..self.at + 4])
											.unwrap(),
										16,
									)
									.unwrap();
									self.at += 4;
									let mut buf = [0u8; 4];
									out.extend_from_slice(
										char::from_u32(code)
											.unwrap()
											.encode_utf8(&mut buf)
											.as_bytes(),
									);
								}
								_ => panic!("JSON: escape"),
							}
						}
						_ => out.push(b),
					}
				}
			}
			b't' => {
				self.at += 4;
				Json::Bool(true)
			}
			b'f' => {
				self.at += 5;
				Json::Bool(false)
			}
			b'n' => {
				self.at += 4;
				Json::Null
			}
			_ => {
				let start = self.at;
				while self
					.text
					.get(self.at)
					.is_some_and(|b| b.is_ascii_digit() || *b == b'-')
				{
					self.at += 1;
				}
				Json::Num(
					std::str::from_utf8(&self.text[start..self.at])
						.unwrap()
						.parse()
						.unwrap(),
				)
			}
		}
	}
}

fn hex(bytes: &[u8]) -> String {
	bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn obj(fields: Vec<(&str, Json)>) -> Json {
	Json::Obj(
		fields
			.into_iter()
			.map(|(k, v)| (k.to_string(), v))
			.collect(),
	)
}

fn text(s: &str) -> Json {
	Json::Str(s.to_string())
}

fn lossy(bytes: &[u8]) -> Json {
	Json::Str(String::from_utf8_lossy(bytes).into_owned())
}

/// What a codec reports for `bytes` fed in `chunks`: the parser's events and the message layer's results, in the vectors' JSON shape.
fn run(bytes: &[u8], chunks: &[usize], max: usize) -> (Vec<Json>, Vec<Json>) {
	let mut payload = vec![0u8; MAX_PAYLOAD];
	let mut parser = Parser::new(&mut payload);
	let mut buffers: Vec<Vec<u8>> = (0..5).map(|_| vec![0u8; 16384]).collect();
	let mut iter = buffers.iter_mut();
	let mut mux = Mux::new(
		[
			iter.next().unwrap().as_mut_slice(),
			iter.next().unwrap().as_mut_slice(),
			iter.next().unwrap().as_mut_slice(),
			iter.next().unwrap().as_mut_slice(),
			iter.next().unwrap().as_mut_slice(),
		],
		max,
	);
	let mut events: Vec<Json> = Vec::new();
	let mut messages: Vec<Json> = Vec::new();
	let mut at = 0usize;
	let mut sizes = chunks.iter().copied().cycle();
	while at < bytes.len() {
		let size = if chunks.is_empty() {
			bytes.len()
		} else {
			sizes.next().unwrap().max(1)
		};
		let end = (at + size).min(bytes.len());
		parser.feed(&bytes[at..end], |event| match event {
			Event::Packet {
				channel,
				id,
				more,
				payload,
			} => {
				events.push(obj(vec![
					("kind", text("packet")),
					("channel", text(channel.name())),
					("id", Json::Num(i64::from(id))),
					("more", Json::Bool(more)),
					("payload_hex", Json::Str(hex(payload))),
				]));
				match mux.push(channel, id, more, payload) {
					Output::None => {}
					Output::Message { channel, id, body } => {
						let fields = body
							.fields()
							.map(|(k, v)| Json::Arr(vec![lossy(k), lossy(v)]))
							.collect();
						messages.push(obj(vec![
							("kind", text("message")),
							("channel", text(channel.name())),
							("id", Json::Num(i64::from(id))),
							("op", lossy(body.op())),
							("fields", Json::Arr(fields)),
							("data_hex", Json::Str(hex(body.data()))),
						]));
					}
					Output::Error { code, channel, id } => {
						messages.push(error_message(code.as_str(), channel, id))
					}
				}
			}
			Event::FileBegin { size, name } => events.push(obj(vec![
				("kind", text("file-begin")),
				("size", Json::Num(i64::from(size))),
				("name", lossy(name)),
			])),
			Event::FileData(data) => {
				if let Some(Json::Obj(fields)) = events.last_mut() {
					if fields[0].1 == text("file-data") {
						if let Json::Str(old) = &mut fields[1].1 {
							old.push_str(&hex(data));
						}
						return;
					}
				}
				events.push(obj(vec![
					("kind", text("file-data")),
					("data_hex", Json::Str(hex(data))),
				]));
			}
			Event::FileEnd { sha256 } => events.push(obj(vec![
				("kind", text("file-end")),
				("sha256", sha256.map_or(Json::Null, lossy)),
			])),
			Event::Error(ParseError::Header { offset }) => events.push(obj(vec![
				("kind", text("error")),
				("error", text("header")),
				("offset", Json::Num(offset as i64)),
			])),
			Event::Error(ParseError::Trailer { offset }) => events.push(obj(vec![
				("kind", text("error")),
				("error", text("trailer")),
				("offset", Json::Num(offset as i64)),
			])),
			Event::Error(ParseError::Crc {
				offset,
				channel,
				id,
			}) => {
				events.push(obj(vec![
					("kind", text("error")),
					("error", text("crc")),
					("offset", Json::Num(offset as i64)),
					("channel", text(channel.name())),
					("id", Json::Num(i64::from(id))),
				]));
				if let Output::Error { code, channel, id } = mux.crc_failed(channel, id) {
					messages.push(error_message(code.as_str(), channel, id));
				}
			}
		});
		at = end;
	}
	(events, messages)
}

fn error_message(code: &str, channel: Channel, id: u32) -> Json {
	obj(vec![
		("kind", text("error")),
		("code", text(code)),
		("channel", text(channel.name())),
		("id", Json::Num(i64::from(id))),
	])
}

struct Vector {
	name: String,
	bytes: Vec<u8>,
	max: usize,
	events: Vec<Json>,
	messages: Vec<Json>,
}

fn load() -> Vec<Vector> {
	let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/host-link/vectors");
	let mut names: Vec<String> = fs::read_dir(&dir)
		.unwrap()
		.map(|e| e.unwrap().file_name().into_string().unwrap())
		.filter_map(|n| n.strip_suffix(".json").map(String::from))
		.collect();
	names.sort();
	names
		.into_iter()
		.map(|name| {
			let bytes = fs::read(dir.join(format!("{name}.bin"))).unwrap();
			let json = fs::read(dir.join(format!("{name}.json"))).unwrap();
			let doc = Reader { text: &json, at: 0 }.value();
			let max = match doc.get("max") {
				Some(Json::Num(n)) => *n as usize,
				_ => DEFAULT_MAX_MESSAGE,
			};
			let list = |key: &str| match doc.get(key) {
				Some(Json::Arr(items)) => items.clone(),
				_ => panic!("{name}: no {key}"),
			};
			Vector {
				events: list("events"),
				messages: list("messages"),
				name,
				bytes,
				max,
			}
		})
		.collect()
}

fn same(v: &Vector, how: &str, chunks: &[usize]) {
	let (events, messages) = run(&v.bytes, chunks, v.max);
	let canon = |items: &[Json]| items.iter().map(Json::canon).collect::<Vec<_>>();
	assert_eq!(
		canon(&events),
		canon(&v.events),
		"{}: events ({how})",
		v.name
	);
	assert_eq!(
		canon(&messages),
		canon(&v.messages),
		"{}: messages ({how})",
		v.name
	);
}

#[test]
fn there_are_at_least_thirty_vectors() {
	assert!(load().len() >= 30);
}

#[test]
fn every_vector_whole() {
	for v in load() {
		same(&v, "whole", &[]);
	}
}

#[test]
fn every_vector_a_byte_at_a_time() {
	for v in load() {
		same(&v, "byte at a time", &[1]);
	}
}

#[test]
fn every_vector_split_in_two_at_every_point() {
	for v in load() {
		let stride = v.bytes.len() / 600 + 1;
		let mut cut = 1;
		while cut < v.bytes.len() {
			same(&v, &format!("split at {cut}"), &[cut, v.bytes.len()]);
			cut += stride;
		}
	}
}

#[test]
fn every_vector_at_random_splits() {
	let mut seed: u64 = 0x2545_F491_4F6C_DD1D;
	let mut next = move || {
		seed ^= seed << 13;
		seed ^= seed >> 7;
		seed ^= seed << 17;
		seed
	};
	for v in load() {
		for round in 0..20 {
			let chunks: Vec<usize> = (0..64).map(|_| (next() % 97) as usize + 1).collect();
			same(&v, &format!("random round {round}"), &chunks);
		}
	}
}
