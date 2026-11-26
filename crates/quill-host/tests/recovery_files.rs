// Recovery files: their names, the list in QUILL.INI, the two-copy rule, the lock note, and what a moved or damaged copy does.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use proptest::prelude::*;
use quill_app::flow::{Answer, Buttons};
use quill_app::recovery::{beside, central, hash, kind_of, Kind, Path, Style, Why};
use quill_app::{App, Command, Drive, Key};
use quill_host::script::{dos_add, MemFiles};
use quill_host::HostStore;

const LETTER: &str = "C:\\DOCS\\LETTER.QLD";

fn text(p: Option<Path>) -> String {
	String::from_utf8(p.expect("a name").bytes().to_vec()).unwrap()
}

fn beside_s(style: Style, kind: Kind, orig: &str, probe: u8) -> String {
	text(beside(style, kind, orig.as_bytes(), probe))
}

// ---- names -------------------------------------------------------------------------------------------------------------------

#[test]
fn the_8_3_names_are_a_tilde_four_characters_three_hex_digits_and_an_extension() {
	let n = beside_s(Style::Dos, Kind::Current, "C:\\LETTERS\\LETTER1.QLD", 0);
	assert!(
		n.starts_with("C:\\LETTERS\\~LETT") && n.ends_with(".RCV"),
		"{n}"
	);
	let file = &n["C:\\LETTERS\\".len()..];
	assert_eq!(file.len(), 12, "a legal 8.3 name: {file}");
	assert!(file[5..8]
		.bytes()
		.all(|b| b.is_ascii_hexdigit() && !b.is_ascii_lowercase()));
	for (kind, ext) in [
		(Kind::Aside, ".RC0"),
		(Kind::Temp, ".$$$"),
		(Kind::Lock, ".LCK"),
		(Kind::Emergency, ".RCE"),
		(Kind::Bad, ".BAD"),
	] {
		let k = beside_s(Style::Dos, kind, "C:\\LETTERS\\LETTER1.QLD", 0);
		assert_eq!(
			k[..k.len() - 4],
			n[..n.len() - 4],
			"one stem for every kind"
		);
		assert!(k.ends_with(ext), "{k}");
	}
}

#[test]
fn letter1_and_letter2_have_different_slots_and_so_do_two_extensions() {
	let a = beside_s(Style::Dos, Kind::Current, "LETTER1.QLD", 0);
	let b = beside_s(Style::Dos, Kind::Current, "LETTER2.QLD", 0);
	let c = beside_s(Style::Dos, Kind::Current, "LETTER.QLD", 0);
	let d = beside_s(Style::Dos, Kind::Current, "LETTER.RTF", 0);
	assert_ne!(a, b);
	assert_ne!(c, d);
	assert!(a.starts_with("~LETT") && b.starts_with("~LETT"));
}

#[test]
fn a_stem_under_four_characters_gives_a_shorter_name() {
	let n = beside_s(Style::Dos, Kind::Current, "A.QLD", 0);
	assert!(n.starts_with("~A") && n.ends_with(".RCV"), "{n}");
	assert_eq!(n.len(), 2 + 3 + 4);
	let n = beside_s(Style::Dos, Kind::Current, "NOEXT", 0);
	assert!(n.starts_with("~NOEX"), "{n}");
}

#[test]
fn a_character_an_8_3_name_cannot_hold_becomes_an_underscore_and_case_does_not_matter() {
	let n = beside_s(Style::Dos, Kind::Current, "a b+c.qld", 0);
	assert!(n.starts_with("~A_B_"), "{n}");
	assert_eq!(
		beside_s(Style::Dos, Kind::Current, "letter.qld", 0),
		beside_s(Style::Dos, Kind::Current, "LETTER.QLD", 0)
	);
}

#[test]
fn the_long_names_carry_the_whole_file_name() {
	let o = "C:\\LETTERS\\LETTER.QLD";
	assert_eq!(
		beside_s(Style::Long, Kind::Current, o, 0),
		"C:\\LETTERS\\~Quill-LETTER.QLD.rcv"
	);
	assert_eq!(
		beside_s(Style::Long, Kind::Aside, o, 0),
		"C:\\LETTERS\\~Quill-LETTER.QLD.rcv.old"
	);
	assert_eq!(
		beside_s(Style::Long, Kind::Temp, o, 0),
		"C:\\LETTERS\\~Quill-LETTER.QLD.rcv.$$$"
	);
	assert_eq!(
		beside_s(Style::Long, Kind::Lock, o, 0),
		"C:\\LETTERS\\~Quill-LETTER.QLD.lck"
	);
	assert_eq!(
		beside_s(Style::Dot, Kind::Current, "/home/s/letter.qld", 0),
		"/home/s/.~Quill-letter.qld.rcv"
	);
	assert_eq!(
		beside_s(Style::Dot, Kind::Aside, "/home/s/letter.qld", 0),
		"/home/s/.~Quill-letter.qld.rcv.old"
	);
}

#[test]
fn another_kind_of_the_same_slot_comes_from_any_of_its_names() {
	for style in [Style::Dos, Style::Long, Style::Dot] {
		let cur = beside(style, Kind::Current, b"C:\\D\\LETTER.QLD", 0).unwrap();
		for from in [
			Kind::Aside,
			Kind::Temp,
			Kind::Lock,
			Kind::Emergency,
			Kind::Bad,
		] {
			let other = beside(style, from, b"C:\\D\\LETTER.QLD", 0).unwrap();
			assert_eq!(kind_of(&other, style, Kind::Current).bytes(), cur.bytes());
		}
	}
}

#[test]
fn the_central_names_are_the_session_for_an_untitled_document_and_the_whole_path_for_an_original() {
	let u = text(central(
		Style::Dos,
		Kind::Current,
		b"RECOVER",
		None,
		0x1A2B3C,
		0,
	));
	assert_eq!(u, "RECOVER\\~Q1A2B3C.RCV");
	let u = text(central(
		Style::Long,
		Kind::Current,
		b"C:\\Q\\",
		None,
		0x1A2B3C,
		0,
	));
	assert_eq!(u, "C:\\Q\\~Quill-Untitled-1A2B3C.rcv");
	// The same file name in two folders gets two names in the one central folder.
	let a = text(central(
		Style::Dos,
		Kind::Current,
		b"R",
		Some(b"C:\\A\\LETTER.QLD"),
		1,
		0,
	));
	let b = text(central(
		Style::Dos,
		Kind::Current,
		b"R",
		Some(b"C:\\B\\LETTER.QLD"),
		1,
		0,
	));
	assert!(a.starts_with("R\\~LETT") && b.starts_with("R\\~LETT"));
	assert_ne!(a, b);
	let l = text(central(
		Style::Long,
		Kind::Current,
		b"R",
		Some(b"C:\\A\\LETTER.QLD"),
		1,
		0,
	));
	assert!(
		l.starts_with("R\\~Quill-LETTER.QLD-") && l.ends_with(".rcv"),
		"{l}"
	);
	assert_eq!(l.len(), "R\\~Quill-LETTER.QLD-".len() + 6 + 4);
	assert_eq!(hash(b"a.qld"), hash(b"A.QLD"));
}

#[test]
fn a_name_too_long_for_the_buffer_is_refused_not_cut() {
	let long = format!("C:\\{}\\LETTER.QLD", "D".repeat(400));
	assert!(beside(Style::Long, Kind::Current, long.as_bytes(), 0).is_none());
}

// ---- a rig -------------------------------------------------------------------------------------------------------------------

struct Rig {
	app: App,
	s: HostStore,
	f: MemFiles,
}

impl Rig {
	fn new() -> Rig {
		let mut f = MemFiles::default();
		for (n, t) in [(LETTER, "dear sir"), ("C:\\DOCS\\OTHER.QLD", "other words")] {
			f.files.insert(n.to_string(), t.as_bytes().to_vec());
		}
		let mut r = Rig {
			app: App::new(),
			s: HostStore::new(),
			f,
		};
		r.start();
		r
	}

	/// The program starts: settings read, launch check run.
	fn start(&mut self) -> bool {
		self.s = HostStore::new();
		self.app = App::new();
		self.app.reset(&mut self.s).unwrap();
		self.app.set_native_dialogs(true);
		self.app.set_recovery(true);
		self.app.load_settings_with(&mut self.s, &mut self.f);
		self.app.recovery_check_launch(&mut self.s, &mut self.f)
	}

	fn ask(&mut self) -> Option<(String, Buttons)> {
		let a = self.app.take_recovery_question(&mut self.s, &mut self.f)?;
		Some((
			String::from_utf8_lossy(a.text.as_bytes()).into_owned(),
			a.buttons,
		))
	}

	fn answer(&mut self, a: Answer) {
		// The question is shown before it is answered.
		let _ = self.ask();
		self.app.recovery_answered(&mut self.s, &mut self.f, a);
	}

	fn open(&mut self, name: &str) {
		self.app
			.open(&mut self.s, &mut self.f, name.as_bytes())
			.unwrap();
	}

	fn type_(&mut self, t: &str) {
		for b in t.bytes() {
			self.app.key(&mut self.s, &mut self.f, Key::Char(b));
		}
	}

	fn autosave(&mut self) -> bool {
		self.app
			.save_recovery(&mut self.s, &mut self.f, Why::Autosave)
	}

	fn wait(&mut self, secs: u32) {
		self.f.clock = dos_add(self.f.clock, secs);
	}

	/// The files that are recovery copies, current and set aside, sorted.
	fn copies(&self) -> Vec<String> {
		let mut v: Vec<String> = self
			.f
			.files
			.keys()
			.filter(|k| k.ends_with(".RCV") || k.ends_with(".RC0"))
			.cloned()
			.collect();
		v.sort();
		v
	}

	fn has(&self, name: &str) -> bool {
		self.f.files.contains_key(name)
	}

	fn body(&self) -> String {
		let mut out = Vec::new();
		let mut at = 0;
		let mut buf = [0u8; 256];
		loop {
			let n = self.app.doc().body.read(&self.s, at, &mut buf);
			if n == 0 {
				break;
			}
			out.extend_from_slice(&buf[..n]);
			at += n as u32;
		}
		out.pop();
		String::from_utf8_lossy(&out).into_owned()
	}

	/// File > New, answering Don't Save.
	fn dont_save(&mut self) {
		self.app.command(&mut self.s, &mut self.f, Command::New);
		assert!(matches!(
			self.app.take_dialog(),
			Some(quill_app::Dialog::ConfirmSave(Command::New))
		));
		self.app
			.confirm_answered(&mut self.s, &mut self.f, Command::New, Answer::No);
	}
}

fn cur(orig: &str) -> String {
	beside_s(Style::Dos, Kind::Current, orig, 0)
}

fn aside(orig: &str) -> String {
	beside_s(Style::Dos, Kind::Aside, orig, 0)
}

// ---- the places --------------------------------------------------------------------------------------------------------------

#[test]
fn a_document_on_a_fixed_disk_has_its_copy_beside_it() {
	let mut r = Rig::new();
	r.open(LETTER);
	r.type_("x");
	assert!(r.autosave());
	assert_eq!(r.copies(), [cur(LETTER)]);
}

#[test]
fn a_floppy_a_read_only_place_and_a_refused_create_send_the_copy_to_the_central_folder() {
	for case in 0..3 {
		let mut r = Rig::new();
		match case {
			0 => r.f.drives.push(("C:".into(), Drive::Removable)),
			1 => r.f.drives.push(("C:".into(), Drive::ReadOnly)),
			_ => r.f.denied.push("C:\\DOCS\\~".into()),
		}
		r.open(LETTER);
		r.type_("x");
		assert!(r.autosave(), "case {case}");
		let copies = r.copies();
		assert_eq!(copies.len(), 1, "case {case}: {copies:?}");
		assert!(
			copies[0].starts_with("RECOVER\\~LETT"),
			"case {case}: {copies:?}"
		);
	}
}

#[test]
fn nowhere_to_write_means_no_copy_and_a_word_once() {
	let mut r = Rig::new();
	r.f.drives.push(("C:".into(), Drive::ReadOnly));
	r.f.denied.push("RECOVER".into());
	r.open(LETTER);
	r.type_("x");
	assert!(!r.autosave());
	assert!(r.copies().is_empty());
	assert_eq!(r.app.message(), b"Quill cannot autosave: the disk is full.");
	r.app.set_message("");
	assert!(!r.autosave());
	assert_eq!(r.app.message(), b"", "said once a session");
}

#[test]
fn an_untitled_document_has_its_copy_in_the_central_folder_named_for_the_session() {
	let mut r = Rig::new();
	r.type_("words");
	assert!(r.autosave());
	let copies = r.copies();
	assert_eq!(copies.len(), 1);
	assert!(
		copies[0].starts_with("RECOVER\\~Q") && copies[0].len() == "RECOVER\\~Q123456.RCV".len()
	);
}

#[test]
fn two_documents_that_share_a_slot_take_the_next_hash_value() {
	// Find two names that give the same 8.3 slot.
	let mut seen = std::collections::HashMap::new();
	let mut pair = None;
	for i in 0..9999 {
		let name = format!("LETT{i}.QLD");
		let n = beside_s(Style::Dos, Kind::Current, &name, 0);
		if let Some(prev) = seen.insert(n, name.clone()) {
			pair = Some((prev, name));
			break;
		}
	}
	let (a, b) = pair.expect("a collision among ten thousand names");
	let (a, b) = (format!("C:\\DOCS\\{a}"), format!("C:\\DOCS\\{b}"));
	assert_eq!(cur(&a), cur(&b));
	let mut r = Rig::new();
	for n in [&a, &b] {
		r.f.files.insert(n.clone(), b"words".to_vec());
	}
	r.open(&a);
	r.type_("x");
	assert!(r.autosave());
	r.kill();
	r.open(&b);
	r.type_("y");
	assert!(r.autosave());
	let copies = r.copies();
	assert_eq!(copies.len(), 2, "{copies:?}");
	assert_eq!(copies[0], cur(&a));
	assert_eq!(copies[1], beside_s(Style::Dos, Kind::Current, &b, 1));
	// Each is found for its own document.
	assert!(r.start());
	let (q, _) = r.ask().unwrap();
	assert!(q.contains("LETT"), "{q}");
}

impl Rig {
	/// The program is killed: nothing is cleaned up, and a new one starts without its launch check.
	fn kill(&mut self) {
		self.s = HostStore::new();
		self.app = App::new();
		self.app.reset(&mut self.s).unwrap();
		self.app.set_native_dialogs(true);
		self.app.set_recovery(true);
		self.app.load_settings_with(&mut self.s, &mut self.f);
	}
}

// ---- the list ----------------------------------------------------------------------------------------------------------------

fn list(r: &Rig) -> Vec<String> {
	(0..r.app.recovery_list_len(&r.s))
		.map(|i| text(r.app.recovery_list_get(&r.s, i)))
		.collect()
}

#[test]
fn the_list_keeps_eight_originals_newest_first_and_a_ninth_drops_the_oldest() {
	let mut r = Rig::new();
	for i in 1..=9 {
		assert!(r
			.app
			.recovery_list_add(&mut r.s, format!("C:\\D\\F{i}.QLD").as_bytes()));
	}
	let l = list(&r);
	assert_eq!(l.len(), 8);
	assert_eq!(l[0], "C:\\D\\F9.QLD");
	assert_eq!(l[7], "C:\\D\\F2.QLD");
	// Adding one that is there moves it to the front, whatever its case; adding the first again changes nothing.
	assert!(r.app.recovery_list_add(&mut r.s, b"c:\\d\\f4.qld"));
	assert_eq!(list(&r)[0], "c:\\d\\f4.qld");
	assert_eq!(list(&r).len(), 8);
	assert!(!r.app.recovery_list_add(&mut r.s, b"C:\\D\\F4.QLD"));
	assert!(r.app.recovery_list_remove(&mut r.s, b"C:\\D\\F9.QLD"));
	assert!(!r.app.recovery_list_remove(&mut r.s, b"C:\\D\\F9.QLD"));
	assert_eq!(list(&r).len(), 7);
}

#[test]
fn the_list_is_written_when_the_first_copy_is_made_and_when_the_last_one_goes() {
	let mut r = Rig::new();
	r.open(LETTER);
	r.type_("x");
	assert!(
		!r.has("QUILL.INI"),
		"nothing is written before a copy exists"
	);
	assert!(r.autosave());
	let ini = String::from_utf8(r.f.files["QUILL.INI"].clone()).unwrap();
	assert!(
		ini.contains("[Recovery]\r\nFile1=C:\\DOCS\\LETTER.QLD\r\n"),
		"{ini}"
	);
	// A second copy of the same document writes nothing new.
	let before = r.f.files["QUILL.INI"].clone();
	r.wait(60);
	assert!(r.autosave());
	assert_eq!(r.f.files["QUILL.INI"], before);
	r.app.command(&mut r.s, &mut r.f, Command::Save);
	let ini = String::from_utf8(r.f.files["QUILL.INI"].clone()).unwrap();
	assert!(!ini.contains("Recovery"), "{ini}");
	assert!(r.copies().is_empty());
}

#[test]
fn the_list_comes_back_from_quill_ini_in_order_with_the_central_folder() {
	let mut r = Rig::new();
	r.f.files.insert(
		"QUILL.INI".into(),
		b"[Quill]\r\nRecoveryDir=D:\\SAFE\r\n[Recovery]\r\nFile1=C:\\A.QLD\r\nFile2=C:\\B.QLD\r\nFile3=C:\\C.QLD\r\n".to_vec(),
	);
	r.kill();
	assert_eq!(list(&r), ["C:\\A.QLD", "C:\\B.QLD", "C:\\C.QLD"]);
	r.app.save_settings_with(&r.s, &mut r.f);
	let ini = String::from_utf8(r.f.files["QUILL.INI"].clone()).unwrap();
	assert!(ini.contains("RecoveryDir=D:\\SAFE\r\n"), "{ini}");
	assert!(
		ini.ends_with("[Recovery]\r\nFile1=C:\\A.QLD\r\nFile2=C:\\B.QLD\r\nFile3=C:\\C.QLD\r\n"),
		"{ini}"
	);
	// The recovery directory the person chose is where an untitled document goes.
	r.type_("words");
	assert!(r.autosave());
	assert!(
		r.copies()[0].starts_with("D:\\SAFE\\~Q"),
		"{:?}",
		r.copies()
	);
}

#[test]
fn an_entry_on_a_missing_drive_is_kept_and_one_whose_copy_is_gone_is_dropped() {
	let mut r = Rig::new();
	r.f.files.insert(
		"QUILL.INI".into(),
		b"[Recovery]\r\nFile1=Z:\\NET.QLD\r\nFile2=C:\\GONE.QLD\r\n".to_vec(),
	);
	r.f.drives.push(("Z:".into(), Drive::Missing));
	assert!(!r.start(), "nothing to ask: no copy anywhere");
	assert_eq!(
		list(&r),
		["Z:\\NET.QLD"],
		"the missing drive stays, the other leaves"
	);
	let ini = String::from_utf8(r.f.files["QUILL.INI"].clone()).unwrap();
	assert!(
		ini.contains("Z:\\NET.QLD") && !ini.contains("GONE"),
		"{ini}"
	);
}

#[test]
fn the_old_quill_rcv_is_offered_as_an_untitled_document_and_goes_with_the_save() {
	let mut r = Rig::new();
	// What #90 left: a plain .QLD with no RCOV chunk.
	let mut other = Rig::new();
	other.type_("saved at shutdown");
	let mut sink = quill_host::docs::VecSink(Vec::new());
	quill_formats::qld::write(other.app.doc(), &other.s, &mut sink).unwrap();
	r.f.files.insert("RECOVER\\QUILL.RCV".into(), sink.0);
	r.f.times.insert("RECOVER\\QUILL.RCV".into(), r.f.clock);
	assert!(r.start());
	let (q, b) = r.ask().unwrap();
	assert_eq!(
		q,
		"Quill found an untitled document that was not saved, from 25 Nov 2026 at 2:32 PM."
	);
	assert_eq!(b, Buttons::RecoverDiscardLater);
	r.answer(Answer::Yes);
	assert_eq!(r.body(), "saved at shutdown");
	assert!(r.app.modified());
	assert!(r.has("RECOVER\\QUILL.RCV"), "kept until the save");
	r.app.save_as(&mut r.s, &mut r.f, b"C:\\DOCS\\NEW.QLD");
	assert!(!r.has("RECOVER\\QUILL.RCV"));
}

#[test]
fn discarding_the_old_quill_rcv_deletes_it() {
	let mut r = Rig::new();
	r.f.files
		.insert("RECOVER\\QUILL.RCV".into(), b"not a document".to_vec());
	assert!(r.start());
	assert!(r.ask().is_some());
	r.answer(Answer::No);
	assert!(!r.has("RECOVER\\QUILL.RCV"));
}

// ---- the two-copy rule -------------------------------------------------------------------------------------------------------

#[test]
fn a_save_and_a_discard_remove_both_copies() {
	let mut r = Rig::new();
	r.open(LETTER);
	r.type_("x");
	assert!(r.autosave());
	r.kill();
	r.start();
	assert!(r.ask().is_some());
	r.answer(Answer::Cancel);
	r.open(LETTER);
	assert!(r.ask().is_some());
	r.answer(Answer::Cancel);
	r.type_("y");
	r.wait(600);
	assert!(r.autosave());
	assert_eq!(r.copies(), [aside(LETTER), cur(LETTER)]);
	r.app.command(&mut r.s, &mut r.f, Command::Save);
	assert!(r.copies().is_empty());
}

#[derive(Clone, Copy, Debug)]
enum Op {
	Edit,
	Autosave,
	Save,
}

#[derive(Clone, Copy, Debug)]
enum End {
	DontSave,
	CleanClose,
	Crash,
}

#[derive(Clone, Copy, Debug)]
enum Open {
	Recover,
	Original,
	Discard,
}

fn op() -> impl Strategy<Value = Op> {
	prop_oneof![
		4 => Just(Op::Edit),
		4 => Just(Op::Autosave),
		1 => Just(Op::Save),
	]
}

/// What the table in the plan says is on the disk, by file: the current copy and the set-aside copy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Model {
	cur: bool,
	aside: bool,
	left: bool,
	wrote: bool,
	modified: bool,
}

fn check(r: &Rig, m: &Model, what: &str) {
	let copies = r.copies();
	assert!(copies.len() <= 2, "{what}: {copies:?}");
	assert_eq!(
		r.has(&cur(LETTER)),
		m.cur,
		"{what}: current copy, {copies:?}"
	);
	assert_eq!(
		r.has(&aside(LETTER)),
		m.aside,
		"{what}: set-aside copy, {copies:?}"
	);
}

/// Leaves what a session that was killed leaves behind: `cur` and `aside` copies of the letter.
fn seed(r: &mut Rig, cur_: bool, aside_: bool) {
	if !(cur_ || aside_) {
		return;
	}
	r.open(LETTER);
	r.type_("seed");
	assert!(r.autosave());
	if aside_ {
		r.kill();
		r.start();
		r.answer(Answer::Cancel);
		r.open(LETTER);
		r.answer(Answer::Cancel);
		r.type_("seed again");
		r.wait(600);
		assert!(r.autosave());
		if !cur_ {
			r.f.files.remove(&cur(LETTER));
		}
	}
	r.kill();
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(160))]

	/// Random sequences of Open original, Recover, Discard, edit, autosave, Save, Don't Save, clean close and crash never leave more than
	/// two copies, and each event leaves what the plan's table says.
	#[test]
	fn the_cap_and_the_table_hold(
		cur0 in any::<bool>(),
		aside0 in any::<bool>(),
		answer in prop_oneof![Just(Open::Recover), Just(Open::Original), Just(Open::Discard)],
		ops in proptest::collection::vec(op(), 0..10),
		end in prop_oneof![Just(End::DontSave), Just(End::CleanClose), Just(End::Crash)],
	) {
		let mut r = Rig::new();
		seed(&mut r, cur0, aside0);
		let mut m = Model { cur: cur0, aside: aside0, left: false, wrote: false, modified: false };
		check(&r, &m, "seeded");
		r.start();
		// The launch question comes first when there is a copy; Later leaves everything.
		if cur0 || aside0 {
			prop_assert!(r.ask().is_some());
			r.answer(Answer::Cancel);
		}
		check(&r, &m, "later");
		r.open(LETTER);
		if cur0 || aside0 {
			prop_assert!(r.ask().is_some());
			match answer {
				Open::Recover => {
					r.answer(Answer::Yes);
					m.modified = true;
					m.wrote = true;
				}
				Open::Original => {
					r.answer(Answer::Cancel);
					m.left = m.cur;
				}
				Open::Discard => {
					r.answer(Answer::No);
					m.cur = false;
					m.aside = false;
				}
			}
		} else {
			prop_assert!(r.ask().is_none());
		}
		check(&r, &m, "open");
		let mut ended = false;
		for o in ops {
			match o {
				Op::Edit => {
					r.type_("e");
					m.modified = true;
				}
				Op::Autosave => {
					r.wait(600);
					let wrote = r.autosave();
					prop_assert_eq!(wrote, m.modified);
					if m.modified {
						if m.left && m.cur {
							m.aside = true;
						}
						m.left = false;
						m.cur = true;
						m.wrote = true;
					}
				}
				Op::Save => {
					r.app.command(&mut r.s, &mut r.f, Command::Save);
					prop_assert!(!r.app.modified());
					// A copy exists to remove only when this session touched one.
					if m.wrote || m.left || m.aside {
						m.cur = false;
						m.aside = false;
					}
					m = Model { modified: false, wrote: false, left: false, ..m };
				}
			}
			check(&r, &m, &format!("{o:?}"));
		}
		match end {
			End::DontSave if m.modified => {
				r.dont_save();
				if m.wrote {
					m.cur = m.aside;
					m.aside = false;
				}
			}
			End::Crash => ended = true,
			_ => {}
		}
		let _ = ended;
		check(&r, &m, &format!("{end:?}"));
	}
}

// ---- the lock note -----------------------------------------------------------------------------------------------------------

fn lock_of(orig: &str) -> String {
	beside_s(Style::Dos, Kind::Lock, orig, 0)
}

#[test]
fn the_first_modification_takes_the_lock_note_and_a_save_gives_it_up() {
	let mut r = Rig::new();
	r.open(LETTER);
	assert!(!r.has(&lock_of(LETTER)), "opening alone makes none");
	r.type_("x");
	assert!(r.has(&lock_of(LETTER)));
	assert_eq!(r.f.files[&lock_of(LETTER)].len(), 32);
	r.app.command(&mut r.s, &mut r.f, Command::Save);
	assert!(!r.has(&lock_of(LETTER)));
}

#[test]
fn a_note_left_by_a_crash_is_taken_over() {
	let mut r = Rig::new();
	r.f.files.insert(lock_of(LETTER), vec![9; 32]);
	r.open(LETTER);
	assert!(
		r.ask().is_none(),
		"an owner nothing proves alive does not stop the open"
	);
	r.type_("x");
	assert_eq!(r.f.files[&lock_of(LETTER)].len(), 32);
	assert_ne!(r.f.files[&lock_of(LETTER)], vec![9; 32], "a new stamp");
	r.wait(60);
	assert!(r.autosave(), "this window still autosaves");
}

#[test]
fn a_second_window_asks_and_then_writes_no_copy() {
	let mut r = Rig::new();
	r.f.files.insert(lock_of(LETTER), vec![1; 32]);
	r.f.alive.push(lock_of(LETTER));
	r.open(LETTER);
	let (q, b) = r.ask().unwrap();
	assert_eq!(
		q,
		"LETTER.QLD is open in another Quill window. Open it here as well? Changes made here are not autosaved."
	);
	assert_eq!(b, Buttons::YesNo);
	r.answer(Answer::No);
	assert_eq!(r.app.name(), b"");
	r.open(LETTER);
	r.ask().unwrap();
	r.answer(Answer::Yes);
	assert_eq!(r.app.name(), LETTER.as_bytes());
	r.type_("x");
	assert!(!r.autosave());
	assert!(r.copies().is_empty());
	assert_eq!(
		r.f.files[&lock_of(LETTER)],
		vec![1; 32],
		"the other window's note is untouched"
	);
}

#[test]
fn a_window_that_finds_a_live_note_when_it_first_modifies_stops_autosaving() {
	let mut r = Rig::new();
	r.open(LETTER);
	r.f.files.insert(lock_of(LETTER), vec![1; 32]);
	r.f.alive.push(lock_of(LETTER));
	r.type_("x");
	assert!(r.app.message().starts_with(b"Another Quill window"));
	assert!(!r.autosave());
}

// ---- moved, deleted and damaged ----------------------------------------------------------------------------------------------

#[test]
fn a_folder_moved_with_its_files_is_the_same_document_in_its_new_place() {
	let mut r = Rig::new();
	r.open(LETTER);
	r.type_("x");
	assert!(r.autosave());
	r.kill();
	let moved = "D:\\ARCHIVE\\LETTER.QLD";
	for (from, to) in [
		(LETTER.to_string(), moved.to_string()),
		(cur(LETTER), cur(moved)),
	] {
		assert!(quill_app::Files::rename(
			&mut r.f,
			from.as_bytes(),
			to.as_bytes()
		));
	}
	r.start();
	// The launch question is for the old place, which has no copy now; the entry leaves the list.
	assert!(r.ask().is_none());
	r.open(moved);
	let (q, _) = r.ask().unwrap();
	assert_eq!(
		q,
		"LETTER.QLD has changes that were not saved, from 25 Nov 2026 at 2:32 PM."
	);
	r.answer(Answer::Yes);
	assert_eq!(r.app.name(), moved.as_bytes());
	assert!(r.app.modified());
	assert_eq!(list(&r), [moved]);
}

#[test]
fn a_temporary_file_that_checks_out_is_promoted_and_any_other_goes() {
	let mut r = Rig::new();
	r.open(LETTER);
	r.type_("x");
	assert!(r.autosave());
	let temp = beside_s(Style::Dos, Kind::Temp, LETTER, 0);
	// The power failed between the delete and the rename.
	assert!(quill_app::Files::rename(
		&mut r.f,
		cur(LETTER).as_bytes(),
		temp.as_bytes()
	));
	r.kill();
	assert!(r.app.recovery_check_launch(&mut r.s, &mut r.f));
	assert!(r.has(&cur(LETTER)) && !r.has(&temp));
	// A temporary file older than the current copy, or one that does not check out, is removed.
	for junk in [false, true] {
		let mut r = Rig::new();
		r.open(LETTER);
		r.type_("x");
		assert!(r.autosave());
		let old = r.f.files[&cur(LETTER)].clone();
		r.wait(60);
		r.type_("y");
		assert!(r.autosave());
		r.f.files.insert(
			temp.clone(),
			if junk { b"half a write".to_vec() } else { old },
		);
		r.kill();
		assert!(r.app.recovery_check_launch(&mut r.s, &mut r.f));
		assert!(r.has(&cur(LETTER)) && !r.has(&temp), "junk={junk}");
	}
}

#[test]
fn a_damaged_copy_is_set_aside_and_a_second_one_replaces_the_first() {
	let mut r = Rig::new();
	for round in 0..2 {
		r.open(LETTER);
		r.type_(&format!("round {round}"));
		assert!(r.autosave());
		r.kill();
		let name = cur(LETTER);
		let last = r.f.files[&name].len() - 1;
		r.f.files.get_mut(&name).unwrap()[last] ^= 0x40;
		assert!(!r.start(), "nothing to offer");
		assert!(r
			.app
			.message()
			.starts_with(b"Quill found a damaged recovery file"));
		let bad = beside_s(Style::Dos, Kind::Bad, LETTER, 0);
		assert!(r.has(&bad) && !r.has(&name));
		assert_eq!(r.f.files.keys().filter(|k| k.ends_with(".BAD")).count(), 1);
	}
}

#[test]
fn an_emergency_copy_is_offered_when_it_is_newer_and_never_replaces_the_good_one() {
	let mut r = Rig::new();
	r.open(LETTER);
	r.type_("good");
	assert!(r.autosave());
	let good = r.f.files[&cur(LETTER)].clone();
	r.wait(120);
	r.type_("more");
	assert!(r.app.save_recovery(&mut r.s, &mut r.f, Why::Emergency));
	assert_eq!(r.f.files[&cur(LETTER)], good, "the good copy is as it was");
	let rce = beside_s(Style::Dos, Kind::Emergency, LETTER, 0);
	assert!(r.has(&rce));
	assert!(
		!list(&r).is_empty(),
		"the autosave listed it, the emergency copy did not add"
	);
	r.kill();
	assert!(r.app.recovery_check_launch(&mut r.s, &mut r.f));
	let (q, _) = r.ask().unwrap();
	assert!(
		q.starts_with("Quill closed after an internal error and kept a copy of LETTER.QLD, from 25 Nov 2026 at 2:34 PM. Parts of the last change may be missing."),
		"{q}"
	);
	// A save removes it with the rest.
	r.answer(Answer::Yes);
	r.app.command(&mut r.s, &mut r.f, Command::Save);
	assert!(!r.has(&rce) && r.copies().is_empty());
}

#[test]
fn the_scripted_clock_moves_by_seconds() {
	let t = dos_add(
		quill_host::script::dos_time(2026, 11, 25, 14, 32, 0),
		28 * 3600 + 90,
	);
	assert_eq!(t, quill_host::script::dos_time(2026, 11, 26, 18, 33, 30));
	let t = dos_add(quill_host::script::dos_time(2026, 12, 31, 23, 59, 58), 4);
	assert_eq!(t, quill_host::script::dos_time(2027, 1, 1, 0, 0, 2));
}
