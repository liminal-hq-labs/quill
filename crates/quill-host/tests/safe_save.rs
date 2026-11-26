// Safe save: a save never leaves a half-written file. A disk that fails at every step, or loses power after any number of
// operations, leaves the original or the whole new document, never anything in between.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use proptest::prelude::*;
use quill_app::recovery::{Kind, Style, Why};
use quill_app::safe::TempName;
use quill_app::{App, Files, Key, Replaced, SaveError, SaveMode};
use quill_formats::io::{Sink, Source};
use quill_formats::{qld, rtf, text};
use quill_host::HostStore;

/// The calls a save makes on the disk, as a failure can name them.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Op {
	Create,
	Write,
	Close,
	Remove,
	Rename,
}

#[derive(Default)]
struct Disk {
	files: BTreeMap<String, Vec<u8>>,
	/// Every call so far, in order.
	log: Vec<Op>,
	/// The machine stops before this call number: it and every later call does nothing.
	cut_at: Option<usize>,
	/// The call number (counting every kind) that fails once, without effect.
	fail_at: Option<usize>,
	/// Room on the disk in bytes; a write that does not fit puts in what does and fails.
	capacity: Option<usize>,
	/// Names that cannot be written, removed or replaced.
	read_only: Vec<String>,
	/// Rename replaces a name that is taken (Windows NT and Linux) instead of refusing (DOS, Windows 3.x, Windows 95).
	rename_replaces: bool,
	/// How the shell says a save goes to the disk.
	mode: Option<SaveMode>,
	dead: bool,
	/// The calls to `Sink::write`.
	writes: usize,
}

impl Disk {
	/// Counts a call; false when the call does nothing (the power is cut, or this is the call that fails).
	fn step(&mut self, op: Op) -> bool {
		let n = self.log.len();
		self.log.push(op);
		if self.cut_at == Some(n) {
			self.dead = true;
		}
		!self.dead && self.fail_at != Some(n)
	}

	fn used(&self) -> usize {
		self.files.values().map(Vec::len).sum()
	}
}

fn key(name: &[u8]) -> String {
	String::from_utf8_lossy(name).to_ascii_uppercase()
}

#[derive(Clone, Default)]
struct Fake(Rc<RefCell<Disk>>);

struct FakeSource(Vec<u8>, usize);

impl Source for FakeSource {
	fn read(&mut self, out: &mut [u8]) -> usize {
		let n = out.len().min(self.0.len() - self.1);
		out[..n].copy_from_slice(&self.0[self.1..self.1 + n]);
		self.1 += n;
		n
	}
}

struct FakeSink(Rc<RefCell<Disk>>, String);

impl Sink for FakeSink {
	fn write(&mut self, data: &[u8]) -> bool {
		let mut d = self.0.borrow_mut();
		d.writes += 1;
		if !d.step(Op::Write) {
			// A write cut off by the power or a bad sector gets half of its bytes down.
			if d.dead {
				let half = data.len() / 2;
				d.files
					.entry(self.1.clone())
					.or_default()
					.extend_from_slice(&data[..half]);
			}
			return false;
		}
		let room = d.capacity.map(|c| c.saturating_sub(d.used()));
		let n = room.map_or(data.len(), |r| r.min(data.len()));
		d.files
			.entry(self.1.clone())
			.or_default()
			.extend_from_slice(&data[..n]);
		n == data.len()
	}
}

impl Files for Fake {
	type R = FakeSource;
	type W = FakeSink;

	fn open(&mut self, name: &[u8]) -> Option<FakeSource> {
		let d = self.0.borrow();
		d.files.get(&key(name)).map(|b| FakeSource(b.clone(), 0))
	}

	fn create(&mut self, name: &[u8]) -> Option<FakeSink> {
		let mut d = self.0.borrow_mut();
		let k = key(name);
		if !d.step(Op::Create) || d.read_only.contains(&k) {
			return None;
		}
		d.files.insert(k.clone(), Vec::new());
		Some(FakeSink(self.0.clone(), k))
	}

	fn close(&mut self, _w: FakeSink) -> bool {
		self.0.borrow_mut().step(Op::Close)
	}

	fn save_mode(&self) -> SaveMode {
		self.0.borrow().mode.unwrap_or(SaveMode::TempRename)
	}

	fn remove(&mut self, name: &[u8]) -> bool {
		let mut d = self.0.borrow_mut();
		let k = key(name);
		if !d.step(Op::Remove) || d.read_only.contains(&k) {
			return false;
		}
		d.files.remove(&k).is_some()
	}

	fn rename(&mut self, from: &[u8], to: &[u8]) -> bool {
		let mut d = self.0.borrow_mut();
		let (a, b) = (key(from), key(to));
		if !d.step(Op::Rename) || d.read_only.contains(&b) {
			return false;
		}
		if d.files.contains_key(&b) && !d.rename_replaces {
			return false;
		}
		match d.files.remove(&a) {
			Some(bytes) => {
				d.files.insert(b, bytes);
				true
			}
			None => false,
		}
	}

	fn exists(&mut self, name: &[u8]) -> bool {
		self.0.borrow().files.contains_key(&key(name))
	}

	fn len(&mut self, name: &[u8]) -> Option<u32> {
		self.0
			.borrow()
			.files
			.get(&key(name))
			.map(|b| b.len() as u32)
	}
}

impl Fake {
	fn get(&self, name: &str) -> Option<Vec<u8>> {
		self.0
			.borrow()
			.files
			.get(&name.to_ascii_uppercase())
			.cloned()
	}

	fn put(&self, name: &str, bytes: &[u8]) {
		self.0
			.borrow_mut()
			.files
			.insert(name.to_ascii_uppercase(), bytes.to_vec());
	}

	fn calls(&self) -> usize {
		self.0.borrow().log.len()
	}

	fn clear_log(&self) {
		let mut d = self.0.borrow_mut();
		d.log.clear();
		d.dead = false;
		d.writes = 0;
	}

	fn names(&self) -> Vec<String> {
		self.0.borrow().files.keys().cloned().collect()
	}
}

/// A document saved once as `name` on the disk, then edited: unsaved changes ready for a second save. Returns the app, its
/// store, the bytes of the first save and the bytes a good second save would write.
fn edited(
	f: &mut Fake,
	name: &[u8],
	first: &str,
	more: &str,
) -> (App, HostStore, Vec<u8>, Vec<u8>) {
	let mut s = HostStore::new();
	let mut app = App::new();
	app.reset(&mut s).unwrap();
	for b in first.bytes() {
		app.key(&mut s, f, Key::Char(b));
	}
	assert!(app.save_as(&mut s, f, name));
	let old = f.get(std::str::from_utf8(name).unwrap()).unwrap();
	for b in more.bytes() {
		app.key(&mut s, f, Key::Char(if b == b'\n' { b'\r' } else { b }));
	}
	let want = written(&app, &mut s, name);
	(app, s, old, want)
}

/// The bytes the writer for `name`'s extension puts out, straight into a vector: what a save has always written.
fn written(app: &App, s: &mut HostStore, name: &[u8]) -> Vec<u8> {
	struct Vec8(Vec<u8>);
	impl Sink for Vec8 {
		fn write(&mut self, data: &[u8]) -> bool {
			self.0.extend_from_slice(data);
			true
		}
	}
	let mut out = Vec8(Vec::new());
	let lower = name.to_ascii_lowercase();
	if lower.ends_with(b".rtf") {
		rtf::write(app.doc(), s, &mut out).unwrap();
	} else if lower.ends_with(b".txt") {
		text::write(app.doc(), s, &mut out).unwrap();
	} else {
		qld::write(app.doc(), s, &mut out).unwrap();
	}
	out.0
}

fn temp_of(name: &str) -> String {
	String::from_utf8(TempName::of(name.as_bytes()).bytes().to_vec()).unwrap()
}

#[test]
fn a_save_writes_the_bytes_the_writer_always_wrote_and_leaves_no_temporary_file() {
	for name in [
		"LETTER.QLD",
		"LETTER.RTF",
		"LETTER.TXT",
		"C:\\DOCS\\LETTER.QLD",
	] {
		let mut f = Fake::default();
		let (mut app, mut s, old, want) = edited(&mut f, name.as_bytes(), "one", " two\nthree");
		assert_ne!(old, want);
		app.save(&mut s, &mut f).unwrap();
		assert_eq!(f.get(name).unwrap(), want, "{name}");
		assert!(!app.unsaved_work());
		assert_eq!(
			f.names(),
			[name.to_ascii_uppercase()],
			"no temporary file is left"
		);
	}
}

#[test]
fn the_temporary_name_is_beside_the_target_and_not_a_recovery_name() {
	let t = temp_of("C:\\DOCS\\LETTER.QLD");
	assert!(
		t.starts_with("C:\\DOCS\\~QTMP") && t.ends_with(".TMP"),
		"{t}"
	);
	assert!(!t.ends_with(".RCV"));
	let file = TempName::of(b"C:\\DOCS\\LETTER.QLD").file().to_vec();
	if !cfg!(feature = "long-names") {
		assert_eq!(file.len(), 12, "an 8.3 name");
	}
}

#[test]
fn a_failure_at_every_step_leaves_the_original_and_says_it_is_unsaved() {
	// The number of calls a good save makes tells how many steps there are to fail.
	let mut probe = Fake::default();
	let (mut app, mut s, ..) = edited(&mut probe, b"A.QLD", "one", " two");
	probe.clear_log();
	app.save(&mut s, &mut probe).unwrap();
	let total = probe.calls();
	assert!(
		total >= 5,
		"create, writes, close, rename and more: {total}"
	);

	for rename_replaces in [false, true] {
		for step in 0..total {
			let mut f = Fake::default();
			f.0.borrow_mut().rename_replaces = rename_replaces;
			let (mut app, mut s, old, want) = edited(&mut f, b"A.QLD", "one", " two");
			f.clear_log();
			f.0.borrow_mut().fail_at = Some(step);
			let r = app.save(&mut s, &mut f);
			let op = f.0.borrow().log.get(step).copied();
			let got = f.get("A.QLD");
			match r {
				Ok(()) => {
					// A failure the save can work around (a rename refused where a second try works).
					assert_eq!(got.as_deref(), Some(&want[..]), "step {step} {op:?}");
					assert!(!app.unsaved_work());
				}
				Err(SaveError::Failed) => {
					assert_eq!(
						got.as_deref(),
						Some(&old[..]),
						"step {step} {op:?}: the original"
					);
					assert!(app.unsaved_work(), "step {step}: still unsaved");
					assert_eq!(
						f.names(),
						["A.QLD"],
						"step {step} {op:?}: no temporary file"
					);
				}
				Err(SaveError::Kept) => {
					// Only after the old file was removed: the new document waits whole in the temporary file.
					assert_eq!(op, Some(Op::Rename), "step {step}");
					assert!(got.is_none());
					assert_eq!(f.get(&temp_of("A.QLD")).as_deref(), Some(&want[..]));
					assert!(app.unsaved_work());
				}
			}
		}
	}
}

#[test]
fn a_rename_that_fails_after_the_delete_names_the_file_that_holds_the_new_document() {
	let mut f = Fake::default();
	let (mut app, mut s, _, want) = edited(&mut f, b"C:\\DOCS\\A.QLD", "one", " two");
	f.clear_log();
	// Calls: create, writes, close, then the first rename (refused: the name is taken), remove, and the second rename.
	let n = {
		let mut probe = Fake::default();
		let (mut a, mut st, ..) = edited(&mut probe, b"C:\\DOCS\\A.QLD", "one", " two");
		probe.clear_log();
		a.save(&mut st, &mut probe).unwrap();
		probe.calls()
	};
	f.0.borrow_mut().fail_at = Some(n - 1);
	app.command(&mut s, &mut f, quill_app::Command::Save);
	let m = String::from_utf8_lossy(app.message()).into_owned();
	assert!(
		m.contains("Work kept in \"~QTMP") && m.ends_with(".TMP\"."),
		"{m}"
	);
	assert_eq!(f.get(&temp_of("C:\\DOCS\\A.QLD")).unwrap(), want);
	assert!(app.unsaved_work());
}

#[test]
fn a_power_cut_after_any_call_leaves_the_original_or_the_whole_new_file() {
	let mut probe = Fake::default();
	let (mut app, mut s, ..) = edited(&mut probe, b"A.QLD", "one", " two three four");
	probe.clear_log();
	app.save(&mut s, &mut probe).unwrap();
	let total = probe.calls();
	for rename_replaces in [false, true] {
		for cut in 0..=total {
			let mut f = Fake::default();
			f.0.borrow_mut().rename_replaces = rename_replaces;
			let (mut app, mut s, old, want) = edited(&mut f, b"A.QLD", "one", " two three four");
			f.clear_log();
			f.0.borrow_mut().cut_at = Some(cut);
			let r = app.save(&mut s, &mut f);
			let target = f.get("A.QLD");
			let temp = f.get(&temp_of("A.QLD"));
			let ok = match &target {
				Some(t) => *t == old || *t == want,
				// Between the delete and the rename of the old file the whole new document is in the temporary file.
				None => temp.as_deref() == Some(&want[..]),
			};
			assert!(
				ok,
				"cut {cut} (replace {rename_replaces}): target {:?} bytes of {}",
				target.map(|t| t.len()),
				want.len()
			);
			if r.is_ok() {
				assert_eq!(f.get("A.QLD").as_deref(), Some(&want[..]));
			} else {
				assert!(
					app.unsaved_work(),
					"cut {cut}: a save that did not finish is not Saved"
				);
			}
		}
	}
}

#[test]
fn a_full_disk_leaves_the_original_and_removes_the_partial_temporary_file() {
	let mut f = Fake::default();
	let (mut app, mut s, old, want) = edited(&mut f, b"A.QLD", "one", &" more words".repeat(300));
	// Room for the old file and half the new one.
	f.0.borrow_mut().capacity = Some(old.len() + want.len() / 2);
	app.command(&mut s, &mut f, quill_app::Command::Save);
	assert_eq!(f.get("A.QLD").unwrap(), old);
	assert_eq!(f.names(), ["A.QLD"]);
	assert!(app.unsaved_work());
	let m = String::from_utf8_lossy(app.message()).into_owned();
	assert!(m.contains("disk may be full"), "{m}");
	// With room, the same document saves.
	f.0.borrow_mut().capacity = None;
	app.save(&mut s, &mut f).unwrap();
	assert_eq!(f.get("A.QLD").unwrap(), want);
}

#[test]
fn a_read_only_target_stays_as_it_was() {
	for rename_replaces in [false, true] {
		let mut f = Fake::default();
		f.0.borrow_mut().rename_replaces = rename_replaces;
		let (mut app, mut s, old, _) = edited(&mut f, b"A.QLD", "one", " two");
		f.0.borrow_mut().read_only.push("A.QLD".into());
		assert_eq!(app.save(&mut s, &mut f), Err(SaveError::Failed));
		assert_eq!(f.get("A.QLD").unwrap(), old);
		assert_eq!(f.names(), ["A.QLD"]);
		assert!(app.unsaved_work());
	}
}

#[test]
fn a_leftover_temporary_file_goes_on_the_next_save_and_on_the_next_open() {
	let mut f = Fake::default();
	let (mut app, mut s, _, want) = edited(&mut f, b"A.QLD", "one", " two");
	let temp = temp_of("A.QLD");
	f.put(&temp, b"half a docum");
	app.save(&mut s, &mut f).unwrap();
	assert_eq!(f.names(), ["A.QLD"]);
	assert_eq!(f.get("A.QLD").unwrap(), want);

	// Opening the document takes one away too.
	f.put(&temp, b"half a docum");
	let mut other = App::new();
	other.reset(&mut s).unwrap();
	other.open(&mut s, &mut f, b"A.QLD").unwrap();
	assert_eq!(f.names(), ["A.QLD"]);
}

#[test]
fn a_temporary_file_beside_a_missing_original_is_kept() {
	// It may hold the only copy of a document (the old file was removed and the rename did not happen).
	let mut f = Fake::default();
	let mut s = HostStore::new();
	let mut app = App::new();
	app.reset(&mut s).unwrap();
	f.put(&temp_of("GONE.QLD"), b"the only copy");
	assert!(app.open(&mut s, &mut f, b"GONE.QLD").is_err());
	assert!(f.get(&temp_of("GONE.QLD")).is_some());
}

#[test]
fn a_shell_that_cannot_rename_saves_in_place_as_before() {
	let mut f = Fake::default();
	f.0.borrow_mut().mode = Some(SaveMode::InPlace);
	let (mut app, mut s, _, want) = edited(&mut f, b"A.QLD", "one", " two");
	app.save(&mut s, &mut f).unwrap();
	assert_eq!(f.get("A.QLD").unwrap(), want);
	assert_eq!(f.names(), ["A.QLD"]);
}

#[test]
fn the_default_replace_takes_the_old_file_away_only_when_the_name_is_taken() {
	let mut f = Fake::default();
	f.put("T.TMP", b"new");
	f.put("A.QLD", b"old");
	assert_eq!(f.replace(b"T.TMP", b"A.QLD"), Replaced::Done);
	assert_eq!(f.get("A.QLD").unwrap(), b"new");
	assert!(f.get("T.TMP").is_none());
}

#[test]
fn a_recovery_copy_is_written_the_same_way() {
	let mut f = Fake::default();
	let (mut app, mut s, ..) = edited(&mut f, b"A.QLD", "one", " two");
	let rcv = quill_app::recovery::beside(Style::Dos, Kind::Current, b"A.QLD", 0).unwrap();
	let rcv = std::str::from_utf8(rcv.bytes()).unwrap().to_string();
	f.put(&rcv, b"an older copy");
	f.0.borrow_mut().fail_at = Some(2);
	f.clear_log();
	f.0.borrow_mut().fail_at = Some(2);
	assert!(!app.save_recovery(&mut s, &mut f, Why::Autosave));
	assert_eq!(
		f.get(&rcv).unwrap(),
		b"an older copy",
		"a failed copy leaves the older one"
	);
	f.0.borrow_mut().fail_at = None;
	assert!(app.save_recovery(&mut s, &mut f, Why::Autosave));
	assert_ne!(f.get(&rcv).unwrap(), b"an older copy");
}

/// The disk calls a save makes for a document of about `n` bytes of text (and the original's bytes).
fn save_writes(n: usize) -> (usize, usize) {
	let mut f = Fake::default();
	let (mut app, mut s, _, want) = edited(&mut f, b"A.QLD", "x", &"word ".repeat(n / 5));
	f.clear_log();
	app.save(&mut s, &mut f).unwrap();
	let after = f.0.borrow().writes;
	// What the writer did before: one call to the sink per field.
	struct Count(usize);
	impl Sink for Count {
		fn write(&mut self, _: &[u8]) -> bool {
			self.0 += 1;
			true
		}
	}
	let mut c = Count(0);
	qld::write(app.doc(), &s, &mut c).unwrap();
	assert_eq!(f.get("A.QLD").unwrap(), want);
	(c.0, after)
}

#[test]
fn small_writes_are_gathered_into_kilobyte_blocks() {
	for n in [200, 5_000, 20_000] {
		let (before, after) = save_writes(n);
		println!("document of {n} bytes: {before} calls to the disk before, {after} after");
		assert!(
			after * 2 <= before || before <= 4,
			"{n}: {before} -> {after}"
		);
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(64))]

	#[test]
	fn any_failure_or_cut_at_any_step_of_any_document_is_safe(
		words in 0usize..1200,
		step in 0usize..400,
		cut in any::<bool>(),
		replaces in any::<bool>(),
	) {
		let mut f = Fake::default();
		f.0.borrow_mut().rename_replaces = replaces;
		let (mut app, mut s, old, want) = edited(&mut f, b"D.QLD", "start", &" lorem".repeat(words));
		f.clear_log();
		{
			let mut d = f.0.borrow_mut();
			if cut { d.cut_at = Some(step) } else { d.fail_at = Some(step) }
		}
		let r = app.save(&mut s, &mut f);
		let target = f.get("D.QLD");
		let temp = f.get(&temp_of("D.QLD"));
		match &target {
			Some(t) => prop_assert!(*t == old || *t == want, "a truncated or mixed file"),
			None => prop_assert_eq!(temp.as_deref(), Some(&want[..])),
		}
		prop_assert_eq!(r.is_ok(), !app.unsaved_work());
		if r.is_ok() {
			prop_assert_eq!(target.as_deref(), Some(&want[..]));
		}
	}
}
