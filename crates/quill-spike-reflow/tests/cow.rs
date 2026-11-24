// Spike J0 (c): a copy-on-write snapshot of a document of about 1 MB, written on a std thread while 1,000 random edits run on the
// live document, equals a write of the document as it was before the edits.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::{Arc, Barrier};

use quill_core::doc::Doc;
use quill_core::store::Store;
use quill_host::docs::VecSink;
use quill_spike_reflow::corpus;
use quill_spike_reflow::cow::{header_bytes, header_copy, CowStore};
use quill_spike_reflow::edits::{random_edit, Mix};
use quill_spike_reflow::rng::{seed, Rng};

/// A sink that gives up the processor after every write, so the writer thread and the editing thread interleave.
struct Polite(Vec<u8>, u64);

impl quill_formats::io::Sink for Polite {
	fn write(&mut self, data: &[u8]) -> bool {
		self.0.extend_from_slice(data);
		self.1 += 1;
		std::thread::yield_now();
		true
	}
}

fn qld(d: &Doc, s: &impl Store) -> Vec<u8> {
	let mut out = VecSink(Vec::new());
	quill_formats::qld::write(d, s, &mut out).unwrap();
	out.0
}

/// Builds the megabyte document in a copy-on-write store by copying the host build chunk by chunk through `.QLD`.
fn big(s: &mut CowStore) -> Doc {
	let mut host = quill_host::HostStore::new();
	let hd = corpus::big_doc(&mut host);
	let bytes = corpus::qld_bytes(&hd, &host);
	let mut d = Doc::new();
	quill_formats::qld::read(&mut d, s, &mut quill_formats::io::SliceSource::new(&bytes)).unwrap();
	d
}

#[test]
fn a_snapshot_written_on_a_thread_equals_the_document_before_the_edits() {
	let seed = seed();
	println!("seed {seed:#x}");
	let mut live = CowStore::new();
	let mut d = big(&mut live);
	let before = qld(&d, &live);
	println!(
		"document: {} bytes of text, {} bytes as .QLD, {} live chunks, header {} bytes",
		d.body.len(),
		before.len(),
		live.live(),
		header_bytes()
	);
	assert!(d.body.len() > 900_000);

	let snap_doc = header_copy(&d);
	let snap = live.snapshot();
	assert_eq!(
		live.shared(),
		live.live(),
		"every chunk is shared at the start"
	);
	live.copied = 0;
	live.copies = 0;
	live.mark();

	let gate = Arc::new(Barrier::new(2));
	let g2 = gate.clone();
	let writer = std::thread::spawn(move || {
		g2.wait();
		let mut sink = Polite(Vec::new(), 0);
		quill_formats::qld::write(&snap_doc, &snap, &mut sink).unwrap();
		(sink.0, sink.1)
	});
	gate.wait();
	let mut rng = Rng(seed);
	let mut made = 0;
	while made < 1_000 {
		if random_edit(
			&mut rng,
			&mut d,
			&mut live,
			Mix {
				formats: true,
				target: 1_000_000,
			},
		)
		.is_some()
		{
			made += 1;
		}
	}
	let (written, sink_calls) = writer.join().unwrap();
	let after = qld(&d, &live);
	println!(
		"snapshot write: {} bytes in {} sink calls; 1,000 edits copied {} bytes in {} chunk copies ({} distinct chunks of {}); live document now {} bytes",
		written.len(),
		sink_calls,
		live.copied,
		live.copies,
		live.distinct_copies(),
		live.live(),
		after.len()
	);
	assert_eq!(
		written, before,
		"the snapshot's bytes are the pre-edit document's"
	);
	assert_ne!(after, before, "the live document changed");
	assert!(live.copies > 0);
}

#[test]
fn a_typed_key_copies_a_few_chunks_once_per_snapshot() {
	let seed = seed();
	let mut live = CowStore::new();
	let mut d = big(&mut live);
	let mut rng = Rng(seed ^ 0xC0);
	let mut worst = 0usize;
	let mut total = 0usize;
	let mut bytes = 0u64;
	let rounds = 40u32;
	for _ in 0..rounds {
		let _snap = live.snapshot();
		live.copied = 0;
		live.mark();
		// One typed key: a letter inserted at a random place.
		let at = rng.below32(d.body.len());
		d.body.insert(&mut live, at, b"x", 0, 0, 0).unwrap();
		let n = live.distinct_copies();
		worst = worst.max(n);
		total += n;
		bytes += live.copied;
		// A second key in the same paragraph, same snapshot: nothing more is copied.
		let again = live.copied;
		d.body.insert(&mut live, at + 1, b"y", 0, 0, 0).unwrap();
		assert_eq!(live.copied, again, "a chunk is copied once per snapshot");
	}
	println!(
		"first key after a snapshot: {:.2} chunks copied on average, {} at most, {} bytes on average",
		total as f64 / f64::from(rounds),
		worst,
		bytes / u64::from(rounds)
	);
	assert!(worst <= 4);
	// With no snapshot alive, an edit copies nothing.
	drop(live.snapshot());
	live.copied = 0;
	d.body.insert(&mut live, 5, b"z", 0, 0, 0).unwrap();
	assert_eq!(live.copied, 0);
}
