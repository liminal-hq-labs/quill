// Property tests for the character tables: they equal quill-grid's, and Windows-1252 and UTF-8 round-trip.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use proptest::prelude::*;
use quill_text::utf8::{decode_1252, encode_1252, Decoded, Refused, BOM};
use quill_text::{cp1252, cp437};

fn decode(src: &[u8]) -> (Vec<u8>, Decoded) {
	let mut out = Vec::new();
	let d = decode_1252(src, |b| out.push(b));
	(out, d)
}

fn encode(src: &[u8]) -> Vec<u8> {
	let mut out = Vec::new();
	encode_1252(src, |b| out.push(b));
	out
}

#[test]
fn cp437_to_char_equals_the_grids_for_every_byte() {
	for b in 0..=255u8 {
		assert_eq!(cp437::to_char(b), quill_grid::cp437(b), "byte {b:#04x}");
	}
}

#[test]
fn cp437_from_ansi_equals_the_grids_for_every_byte() {
	for b in 0..=255u8 {
		assert_eq!(
			cp437::from_ansi(b),
			quill_grid::ansi_to_cp437(b),
			"byte {b:#04x}"
		);
	}
}

#[test]
fn cp437_to_ansi_inverts_from_ansi_wherever_cp437_has_a_letter() {
	for a in 0x80..=0xFFu8 {
		let c = cp437::from_ansi(a);
		if c >= 0x80 && c != 0xB1 {
			assert_eq!(cp437::to_ansi(c), a, "ANSI {a:#04x} is CP437 {c:#04x}");
		}
	}
}

#[test]
fn cp437_to_ansi_keeps_ascii_and_marks_the_rest() {
	for b in 0..0x80u8 {
		assert_eq!(cp437::to_ansi(b), b);
	}
	// Box drawing, Greek and the shaded block have no Windows-1252 form.
	for b in [0xB0, 0xB1, 0xB2, 0xC4, 0xDB, 0xE0, 0xE3, 0xEC, 0xFE] {
		assert_eq!(cp437::to_ansi(b), b'?', "CP437 {b:#04x}");
	}
	assert_eq!(cp437::to_ansi(0x82), 0xE9); // é
	assert_eq!(cp437::to_ansi(0xE1), 0xDF); // ß
}

#[test]
fn cp1252_known_characters() {
	assert_eq!(cp1252::to_char(0x80), '€');
	assert_eq!(cp1252::to_char(0x93), '“');
	assert_eq!(cp1252::to_char(0x94), '”');
	assert_eq!(cp1252::to_char(0x95), '•');
	assert_eq!(cp1252::to_char(0x9F), 'Ÿ');
	assert_eq!(cp1252::to_char(0xA0), '\u{a0}');
	assert_eq!(cp1252::to_char(0xE9), 'é');
	assert_eq!(cp1252::from_char('€'), Some(0x80));
	assert_eq!(cp1252::from_char('é'), Some(0xE9));
	assert_eq!(cp1252::from_char('\u{80}'), None);
	assert_eq!(cp1252::from_char('\u{100}'), None);
	assert_eq!(cp1252::from_char('☺'), None);
	assert_eq!(cp1252::from_char('\u{1F600}'), None);
}

#[test]
fn cp1252_every_byte_round_trips() {
	for b in 0..=255u8 {
		assert_eq!(
			cp1252::from_char(cp1252::to_char(b)),
			Some(b),
			"byte {b:#04x}"
		);
	}
}

#[test]
fn cp1252_has_no_two_bytes_for_one_character() {
	for a in 0..=255u8 {
		for b in 0..a {
			assert_ne!(
				cp1252::to_char(a),
				cp1252::to_char(b),
				"{a:#04x} and {b:#04x}"
			);
		}
	}
}

#[test]
fn a_byte_order_mark_is_dropped_and_counts_as_utf8() {
	let mut src = BOM.to_vec();
	src.extend_from_slice("café".as_bytes());
	let (out, d) = decode(&src);
	assert_eq!(out, b"caf\xE9");
	assert!(d.utf8);
	assert_eq!(d.refused, Refused { count: 0 });
	assert_eq!(
		decode(&BOM),
		(
			Vec::new(),
			Decoded {
				utf8: true,
				refused: Refused { count: 0 }
			}
		)
	);
}

#[test]
fn invalid_utf8_is_windows_1252_and_unchanged() {
	let src = b"caf\xE9 \x93quoted\x94";
	let (out, d) = decode(src);
	assert_eq!(out, src);
	assert_eq!(d, Decoded::default());
}

#[test]
fn ascii_is_not_reported_as_utf8() {
	let (out, d) = decode(b"plain text\r\n");
	assert_eq!(out, b"plain text\r\n");
	assert!(!d.utf8);
}

#[test]
fn characters_without_a_form_are_refused_and_counted() {
	let (out, d) = decode("a☺b€c\u{1F600}".as_bytes());
	assert_eq!(out, b"a?b\x80c?");
	assert!(d.utf8);
	assert_eq!(d.refused.count, 2);
}

proptest! {
	#[test]
	fn cp1252_char_to_byte_to_char(c in any::<char>()) {
		if let Some(b) = cp1252::from_char(c) {
			prop_assert_eq!(cp1252::to_char(b), c);
		}
	}

	#[test]
	fn windows_1252_bytes_survive_utf8_and_back(bytes in proptest::collection::vec(any::<u8>(), 0..300)) {
		let utf8 = encode(&bytes);
		prop_assert!(core::str::from_utf8(&utf8).is_ok());
		let (back, d) = decode(&utf8);
		prop_assert_eq!(back, bytes.clone());
		prop_assert_eq!(d.refused.count, 0);
		prop_assert_eq!(d.utf8, bytes.iter().any(|&b| b >= 0x80));
	}

	#[test]
	fn any_text_decodes_to_one_byte_per_character(s in ".{0,100}") {
		let (out, d) = decode(s.as_bytes());
		// Only a leading byte order mark is dropped; a later U+FEFF has no Windows-1252 form.
		let body = s.strip_prefix('\u{FEFF}').unwrap_or(&s);
		let refused = body.chars().filter(|&c| cp1252::from_char(c).is_none()).count();
		prop_assert_eq!(out.len(), body.chars().count());
		prop_assert_eq!(d.refused.count, refused);
		prop_assert!(out.iter().filter(|&&b| b == b'?').count() >= refused);
	}

	#[test]
	fn any_bytes_decode_without_growing(bytes in proptest::collection::vec(any::<u8>(), 0..300)) {
		let (out, d) = decode(&bytes);
		prop_assert!(out.len() <= bytes.len());
		if core::str::from_utf8(&bytes).is_err() {
			prop_assert_eq!(out, bytes);
			prop_assert_eq!(d, Decoded::default());
		}
	}
}
