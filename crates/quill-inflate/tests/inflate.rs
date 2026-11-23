// Inflate tests against hand-made streams, a stored block, and damage.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use proptest::prelude::*;
use quill_inflate::{inflate, InflateError, WINDOW};

fn run(data: &[u8]) -> Result<Vec<u8>, InflateError> {
	let mut win = Box::new([0u8; WINDOW]);
	let mut i = 0;
	let mut out = Vec::new();
	inflate(
		|| {
			let b = data.get(i).copied();
			i += 1;
			b
		},
		|b| {
			out.push(b);
			true
		},
		&mut win,
	)?;
	Ok(out)
}

#[test]
fn a_stored_block_comes_out_as_it_went_in() {
	let mut s = vec![0x01, 5, 0, 0xFA, 0xFF];
	s.extend_from_slice(b"hello");
	assert_eq!(run(&s).unwrap(), b"hello");
}

#[test]
fn fixed_huffman_with_a_back_reference() {
	// "abcabcabc" compressed by zlib at level 9, raw deflate (computed with Python's zlib, wbits=-15).
	let s = [75, 76, 74, 78, 4, 35, 0];
	assert_eq!(run(&s).unwrap(), b"abcabcabc");
}

#[test]
fn bad_input_is_refused_not_trusted() {
	assert_eq!(run(&[0x07]).err(), Some(InflateError::BadBlock));
	assert_eq!(run(&[]).err(), Some(InflateError::Truncated));
	// A length-3 match at distance 1 with nothing before it.
	assert_eq!(run(&[3, 2, 0]).err(), Some(InflateError::BadDistance));
}

#[test]
fn the_sink_can_stop_the_stream() {
	let mut win = Box::new([0u8; WINDOW]);
	let s = [75, 76, 74, 78, 4, 35, 0];
	let mut i = 0;
	let mut n = 0;
	let r = inflate(
		|| {
			i += 1;
			s.get(i - 1).copied()
		},
		|_| {
			n += 1;
			n < 4
		},
		&mut win,
	);
	assert_eq!(r.err(), Some(InflateError::Stopped));
}

proptest! {
	#[test]
	fn random_bytes_never_panic(data in proptest::collection::vec(any::<u8>(), 0..200)) {
		let _ = run(&data);
	}
}

#[test]
fn real_zlib_streams_round_trip() {
	// 40,000 bytes of the SPEC compressed raw by zlib at levels 1 (fixed and short matches) and 9 (dynamic tables, long distances).
	let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/");
	let want = std::fs::read(format!("{dir}sample.txt")).unwrap();
	for lvl in [1, 9] {
		let packed = std::fs::read(format!("{dir}sample.l{lvl}.deflate")).unwrap();
		assert_eq!(run(&packed).unwrap(), want, "level {lvl}");
	}
}
