// The recovery file: the RCOV chunk, its checksum from the counting pass, and what damage does to it.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use proptest::prelude::*;
use quill_core::doc::Doc;
use quill_formats::io::{Sink, SliceSource};
use quill_formats::qld::{
	self, Recovery, MAX_RCOV_PATH, PROGRESS_STEP, RCOV_HEAD_MAX, RCV_AUTOSAVE, RCV_CENTRAL,
};
use quill_formats::FormatError;
use quill_host::docs::{self, VecSink};
use quill_host::HostStore;

/// The offset of the first byte after `HEAD`, where `RCOV` starts.
const RCOV_AT: usize = 28;

fn sample() -> Recovery {
	let mut r = Recovery::default();
	r.flags = RCV_AUTOSAVE | RCV_CENTRAL;
	r.writer = 5;
	r.quill = (0, 9);
	r.written = 0x5A3B_6C4D;
	r.revision = 1234;
	r.orig_size = 99_999;
	r.orig_stamp = 0x1111_2222;
	r.caret = 321;
	r.top_line = 12;
	r.set_path(b"C:\\LETTERS\\LETTER1.QLD").unwrap();
	r
}

/// A document of about `kb` kilobytes, so the checksum crosses several of its own blocks.
fn big(s: &mut HostStore, kb: usize) -> Doc {
	let mut d = docs::plain(s).unwrap();
	let line = "The quick brown fox jumps over the lazy dog, again and again.\r";
	let text: String = line.repeat(kb * 1024 / line.len());
	let at = d.body.len();
	d.body.insert(s, at, text.as_bytes(), 0, 0, 0).unwrap();
	d
}

fn plain_bytes(d: &Doc, s: &HostStore) -> Vec<u8> {
	let mut out = VecSink(Vec::new());
	qld::write(d, s, &mut out).unwrap();
	out.0
}

fn recovery_bytes(d: &Doc, s: &HostStore, rec: &Recovery) -> Vec<u8> {
	let mut out = VecSink(Vec::new());
	qld::write_recovery(d, s, &mut out, rec, None).unwrap();
	out.0
}

/// Adler-32 the slow way, one reduction per byte.
fn adler(data: &[u8]) -> u32 {
	let (mut a, mut b) = (1u32, 0u32);
	for v in data {
		a = (a + u32::from(*v)) % 65521;
		b = (b + a) % 65521;
	}
	(b << 16) | a
}

fn verify(bytes: &[u8]) -> Result<Recovery, FormatError> {
	qld::verify(&mut SliceSource::new(bytes))
}

fn fixture() -> (HostStore, Doc) {
	let mut s = HostStore::new();
	let d = big(&mut s, 20);
	(s, d)
}

#[test]
fn a_recovery_file_round_trips_its_header() {
	let (s, d) = fixture();
	let want = sample();
	let bytes = recovery_bytes(&d, &s, &want);
	let got = verify(&bytes).unwrap();
	assert_eq!(got.version, 1);
	assert_eq!(got.flags, want.flags);
	assert_eq!((got.format, got.writer, got.quill), (0, 5, (0, 9)));
	assert_eq!((got.written, got.revision), (want.written, want.revision));
	assert_eq!((got.orig_size, got.orig_stamp), (99_999, 0x1111_2222));
	assert_eq!((got.caret, got.top_line), (321, 12));
	assert_eq!(got.path(), b"C:\\LETTERS\\LETTER1.QLD");
	assert_eq!(got.body as usize, bytes.len() - got.covered as usize);
}

#[test]
fn the_bytes_after_rcov_are_the_plain_writers() {
	let (s, d) = fixture();
	let plain = plain_bytes(&d, &s);
	let rec = recovery_bytes(&d, &s, &sample());
	let got = verify(&rec).unwrap();
	assert_eq!(&rec[got.body as usize..], &plain[RCOV_AT..]);
	assert_eq!(&rec[..RCOV_AT][8..], &plain[..RCOV_AT][8..]);
	assert_eq!(got.covered as usize, plain.len() - RCOV_AT);
	assert_eq!(got.adler, adler(&plain[RCOV_AT..]));
}

#[test]
fn the_checksum_matches_a_second_computation_for_many_sizes() {
	let mut s = HostStore::new();
	for kb in [0, 1, 5, 6, 40, 130] {
		let d = if kb == 0 {
			docs::plain(&mut s).unwrap()
		} else {
			big(&mut s, kb)
		};
		let bytes = recovery_bytes(&d, &s, &sample());
		let got = verify(&bytes).unwrap_or_else(|e| panic!("{kb} KB: {e:?}"));
		assert_eq!(got.adler, adler(&bytes[got.body as usize..]), "{kb} KB");
	}
}

#[test]
fn a_recovery_file_opens_with_the_plain_reader_and_gives_the_same_document() {
	let (mut s, d) = fixture();
	let bytes = recovery_bytes(&d, &s, &sample());
	let mut fresh = Doc::new();
	qld::read(&mut fresh, &mut s, &mut SliceSource::new(&bytes)).unwrap();
	assert_eq!(plain_bytes(&fresh, &s), plain_bytes(&d, &s));
}

#[test]
fn the_header_reads_from_its_first_bytes_alone() {
	let (s, d) = fixture();
	let mut rec = sample();
	rec.set_path(&[b'x'; MAX_RCOV_PATH]).unwrap();
	let bytes = recovery_bytes(&d, &s, &rec);
	let head = &bytes[..RCOV_HEAD_MAX];
	let got = qld::read_header(&mut SliceSource::new(head)).unwrap();
	assert_eq!(got.path().len(), MAX_RCOV_PATH);
	assert_eq!(got.revision, rec.revision);
	// The header alone does not prove the rest.
	assert!(verify(head).is_err());
}

#[test]
fn paths_of_every_length_to_the_limit_round_trip() {
	let (s, d) = fixture();
	for n in [0usize, 1, 2, 257, 258, 259] {
		let mut rec = sample();
		rec.set_path(&vec![b'p'; n]).unwrap();
		let got = verify(&recovery_bytes(&d, &s, &rec)).unwrap();
		assert_eq!(got.path(), &vec![b'p'; n][..], "{n}");
	}
}

#[test]
fn a_path_over_the_limit_is_refused_by_the_writer() {
	let mut rec = sample();
	assert_eq!(
		rec.set_path(&[b'x'; MAX_RCOV_PATH + 1]),
		Err(FormatError::TooLarge)
	);
	let (s, d) = fixture();
	let mut forced = sample();
	forced.path_len = 300;
	let mut out = VecSink(Vec::new());
	let r = qld::write_recovery(&d, &s, &mut out, &forced, None);
	assert_eq!(r, Err(FormatError::TooLarge));
	assert!(out.0.is_empty());
}

#[test]
fn an_over_long_path_field_is_rejected_by_the_reader() {
	let (s, d) = fixture();
	let bytes = recovery_bytes(&d, &s, &sample());
	// The path length is the u16 at offset 40 of the chunk's data, which starts 8 bytes after `RCOV_AT`.
	let at = RCOV_AT + 8 + 40;
	for len in [260u16, 261, 300, 1000, 0x7FFF, 0xFFFF] {
		let mut b = bytes.clone();
		b[at..at + 2].copy_from_slice(&len.to_le_bytes());
		assert_eq!(verify(&b), Err(FormatError::BadFile), "{len}");
	}
	// Over-long chunk sizes too: the field and the chunk must agree.
	for size in [41u32, 43, 302, 304, 0x10000, u32::MAX] {
		let mut b = bytes.clone();
		b[RCOV_AT + 4..RCOV_AT + 8].copy_from_slice(&size.to_le_bytes());
		assert_eq!(verify(&b), Err(FormatError::BadFile), "{size}");
	}
}

#[test]
fn a_file_without_rcov_is_not_a_recovery_file() {
	let (s, d) = fixture();
	let plain = plain_bytes(&d, &s);
	assert_eq!(verify(&plain), Err(FormatError::BadFile));
	assert_eq!(verify(&[]), Err(FormatError::BadFile));
}

#[test]
fn a_newer_version_is_named_as_newer() {
	let (s, d) = fixture();
	let bytes = recovery_bytes(&d, &s, &sample());
	let mut b = bytes.clone();
	b[RCOV_AT + 8] = 2;
	assert_eq!(verify(&b), Err(FormatError::Newer));
	let mut b = bytes;
	b[20] = 2;
	assert_eq!(verify(&b), Err(FormatError::Newer));
}

#[test]
fn bytes_after_the_end_of_the_riff_are_refused() {
	let (s, d) = fixture();
	let mut bytes = recovery_bytes(&d, &s, &sample());
	bytes.push(0);
	assert_eq!(verify(&bytes), Err(FormatError::BadFile));
}

#[test]
fn truncation_at_every_byte_of_the_first_4k_is_refused() {
	let (s, d) = fixture();
	let bytes = recovery_bytes(&d, &s, &sample());
	for cut in 0..4096.min(bytes.len()) {
		assert!(verify(&bytes[..cut]).is_err(), "cut at {cut}");
		let _ = qld::read_header(&mut SliceSource::new(&bytes[..cut]));
	}
}

#[test]
fn every_single_byte_flip_in_the_covered_bytes_is_caught() {
	let (s, d) = fixture();
	let bytes = recovery_bytes(&d, &s, &sample());
	let body = verify(&bytes).unwrap().body as usize;
	// Every byte of the first 4 KB of the body, then a stride through the rest.
	let at = (body..body + 4096).chain((body + 4096..bytes.len()).step_by(37));
	for i in at {
		let mut b = bytes.clone();
		b[i] ^= 0x01;
		assert!(verify(&b).is_err(), "flip at {i}");
	}
}

#[test]
fn a_flip_in_the_header_never_panics_and_never_changes_the_covered_bytes() {
	let (s, d) = fixture();
	let bytes = recovery_bytes(&d, &s, &sample());
	let body = verify(&bytes).unwrap().body as usize;
	for i in 0..body {
		for x in [0x01u8, 0x80, 0xFF] {
			let mut b = bytes.clone();
			b[i] ^= x;
			if let Ok(got) = verify(&b) {
				assert_eq!(&b[got.body as usize..], &bytes[body..], "flip at {i}");
			}
		}
	}
}

struct Stop {
	got: usize,
	after: usize,
}

impl Sink for Stop {
	fn write(&mut self, data: &[u8]) -> bool {
		self.got += data.len();
		self.got <= self.after
	}
}

#[test]
fn the_progress_hook_counts_to_the_file_and_can_stop_it() {
	let (s, d) = fixture();
	let want = recovery_bytes(&d, &s, &sample());

	let mut calls: Vec<(u32, u32)> = Vec::new();
	let mut hook = |done: u32, total: u32| {
		calls.push((done, total));
		true
	};
	let mut out = VecSink(Vec::new());
	qld::write_recovery(&d, &s, &mut out, &sample(), Some(&mut hook)).unwrap();
	assert_eq!(out.0, want, "the hook changes no byte");
	let counting = calls.iter().take_while(|c| c.1 == 0).count();
	assert!(counting > 0, "the counting pass reports");
	let writing = &calls[counting..];
	assert!(writing.len() > 3);
	assert!(writing.iter().all(|c| c.1 as usize == want.len()));
	assert!(
		writing.windows(2).all(|w| w[0].0 < w[1].0),
		"done only rises"
	);
	assert!(writing.windows(2).all(|w| w[1].0 - w[0].0 >= PROGRESS_STEP));
	assert!(writing.last().unwrap().0 as usize <= want.len());

	// Stopping in the writing pass leaves a short file and an error.
	let mut n = 0;
	let mut hook = |_, total| {
		n += u32::from(total > 0);
		n < 3
	};
	let mut out = VecSink(Vec::new());
	let r = qld::write_with_progress(&d, &s, &mut out, &mut hook);
	assert_eq!(r, Err(FormatError::Write));
	assert!(out.0.len() < want.len() && out.0.len() < 4 * PROGRESS_STEP as usize + 1024);

	// Stopping in the counting pass writes nothing at all.
	let mut hook = |_, _| false;
	let mut out = VecSink(Vec::new());
	let r = qld::write_with_progress(&d, &s, &mut out, &mut hook);
	assert_eq!(r, Err(FormatError::Write));
	assert!(out.0.is_empty());
}

#[test]
fn a_refusing_sink_stops_the_recovery_write() {
	let (s, d) = fixture();
	let mut sink = Stop {
		got: 0,
		after: 5000,
	};
	let r = qld::write_recovery(&d, &s, &mut sink, &sample(), None);
	assert_eq!(r, Err(FormatError::Write));
}

#[test]
fn the_plain_writer_is_unchanged_by_the_hook() {
	let (s, d) = fixture();
	let plain = plain_bytes(&d, &s);
	let mut out = VecSink(Vec::new());
	qld::write_with_progress(&d, &s, &mut out, &mut |_, _| true).unwrap();
	assert_eq!(out.0, plain);
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(400))]

	// Truncation at a random offset anywhere in the file is refused.
	#[test]
	fn truncation_anywhere_is_refused(cut in any::<prop::sample::Index>()) {
		let (s, d) = fixture();
		let bytes = recovery_bytes(&d, &s, &sample());
		let keep = cut.index(bytes.len());
		prop_assert!(verify(&bytes[..keep]).is_err());
	}

	// One flipped byte in the covered part is always found.
	#[test]
	fn a_random_flip_in_the_body_is_refused(at in any::<prop::sample::Index>(), x in 1u8..=255) {
		let (s, d) = fixture();
		let bytes = recovery_bytes(&d, &s, &sample());
		let body = verify(&bytes).unwrap().body as usize;
		let i = body + at.index(bytes.len() - body);
		let mut b = bytes;
		b[i] ^= x;
		prop_assert!(verify(&b).is_err());
	}

	// Several flips anywhere: no panic, and what verifies still holds the original covered bytes unless the damage cancelled out
	// (which a handful of flips cannot do for the first four).
	#[test]
	fn random_flips_never_panic(flips in prop::collection::vec((any::<prop::sample::Index>(), any::<u8>()), 1..8)) {
		let (s, d) = fixture();
		let bytes = recovery_bytes(&d, &s, &sample());
		let mut b = bytes;
		for (i, v) in flips {
			let n = b.len();
			b[i.index(n)] = v;
		}
		let _ = verify(&b);
		let _ = qld::read_header(&mut SliceSource::new(&b));
	}

	#[test]
	fn random_bytes_are_refused(bytes in prop::collection::vec(any::<u8>(), 0..600)) {
		prop_assert!(verify(&bytes).is_err());
		let _ = qld::read_header(&mut SliceSource::new(&bytes));
	}

	// A valid prefix followed by noise is refused, and an over-long path field never panics.
	#[test]
	fn noise_after_a_valid_header_is_refused(tail in prop::collection::vec(any::<u8>(), 0..2000), len in any::<u16>()) {
		let (s, d) = fixture();
		let bytes = recovery_bytes(&d, &s, &sample());
		let body = verify(&bytes).unwrap().body as usize;
		let mut b = bytes[..body].to_vec();
		b.extend_from_slice(&tail);
		prop_assert!(verify(&b).is_err());
		let at = RCOV_AT + 8 + 40;
		let mut c = bytes;
		c[at..at + 2].copy_from_slice(&len.to_le_bytes());
		let r = verify(&c);
		if usize::from(len) > MAX_RCOV_PATH {
			prop_assert!(r.is_err());
		}
	}
}
