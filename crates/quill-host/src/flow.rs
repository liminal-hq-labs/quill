// Flow scripts: commands and answers played through a driver, with the questions and notices it raised logged beside the state.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// A `.flow` script says what a person does and what the editor should ask back, in the terms every build shares: a key or a
// command goes in, a dialog comes out (`ask`, `file`, `dialog`, `spell`, `notice`), and an answer goes back. A `Driver` is what
// plays it: `NativeStub` answers the editor's `take_dialog()` through `quill_app::flow` as the Windows shells do. A text-cell
// shell implements the same trait by turning each reply into keys, and must produce the same transcript. The format is in
// `docs/testing.md`.
use std::collections::{BTreeMap, VecDeque};
use std::fmt::Write as _;

use quill_app::flow::{
	Answer, Ask, FindFields, Notice, NoticeKind, SpellButton, SpellStep, TabButton,
};
use quill_app::recovery::Why;
use quill_app::{App, Command, Dialog, FindAction, Key};

use quill_app::{Drive, Files, LockMade, Owner, Stamp};

use crate::script::{MemFiles, MemSource, NamedSink};
use crate::HostStore;

// ---- what a driver shows and is told -------------------------------------------------------------------------------------------

/// A dialog on screen.
#[derive(Clone, Debug)]
pub enum Shown {
	/// A question in a message box, and the command it is about.
	Ask(Ask, Command),
	/// A recovery question: at launch, at Open, or the second window's.
	Recover(Ask),
	/// A file dialog (`Dialog::Open`, `SaveAs` or `Picture`) with what it starts with.
	File {
		/// Which one.
		dialog: Dialog,
		/// The folder it opens in.
		folder: Vec<u8>,
		/// The file name filled in.
		name: Vec<u8>,
		/// The selected type.
		type_index: usize,
	},
	/// Find or Replace, with the fields it starts with.
	Find {
		/// True for Replace.
		replace: bool,
		/// The text to find.
		find: Vec<u8>,
		/// The replacement.
		with: Vec<u8>,
		/// Match case.
		case: bool,
		/// Whole words.
		word: bool,
	},
	/// The spelling dialog on an unknown word, with the first suggestion in its Change to field.
	Spell {
		/// The word.
		word: Vec<u8>,
		/// What the Change to field starts with.
		with: Vec<u8>,
	},
	/// Paragraph > Tabs.
	Tabs,
	/// Character > Fonts.
	Fonts,
	/// Help > About.
	About,
	/// File > Print.
	Print,
}

/// Something the editor raised: a dialog or a message.
#[derive(Clone, Debug)]
pub enum Event {
	/// A dialog to answer.
	Show(Shown),
	/// A message.
	Notice(Notice),
}

/// What a reply did: the events it raised, and whether the dialog it answered is gone.
#[derive(Clone, Debug, Default)]
pub struct Outcome {
	/// What the editor raised.
	pub events: Vec<Event>,
	/// True when the dialog closed (Find, Tabs and a spelling check that goes on stay open).
	pub closed: bool,
}

/// What the person does in a dialog.
#[derive(Clone, Debug)]
pub enum Reply {
	/// A button of a question.
	Answer(Answer),
	/// A file dialog's OK with this path.
	Chosen(Vec<u8>),
	/// The dialog was dismissed (Cancel or Esc).
	Cancel,
	/// A button of Find or Replace; the dialog stays open.
	Find {
		/// The button.
		action: FindAction,
		/// The fields.
		find: Vec<u8>,
		/// The replacement.
		with: Vec<u8>,
		/// Match case.
		case: bool,
		/// Whole words.
		word: bool,
	},
	/// A button of the spelling dialog.
	Spell {
		/// The button.
		button: SpellButton,
		/// The Change to field.
		with: Vec<u8>,
	},
	/// A button of Tabs; the dialog stays open.
	Tab {
		/// The button.
		button: TabButton,
		/// The position field.
		text: Vec<u8>,
		/// The kind radio.
		kind: u8,
		/// The leader radio.
		leader: u8,
	},
	/// Fonts OK.
	Fonts {
		/// The face.
		face: Vec<u8>,
		/// The size in half points.
		half_points: u16,
		/// Bold, italic, underline and strikeout bits.
		flags: u16,
	},
	/// Print OK with a destination.
	Print(Vec<u8>),
	/// Close (OK on About, Close on Find and Tabs).
	Close,
}

/// What a run compares: the editor's state after the script.
#[derive(Clone, Debug, Default)]
pub struct State {
	/// The body text, paragraph marks as `\n`.
	pub text: String,
	/// The caret.
	pub caret: u32,
	/// The selection.
	pub selection: Option<(u32, u32)>,
	/// The file name.
	pub name: Vec<u8>,
	/// Unsaved changes.
	pub modified: bool,
	/// The status line.
	pub status: Vec<u8>,
	/// The recent files, most recent first.
	pub recent: Vec<Vec<u8>>,
	/// The last folder.
	pub folder: Vec<u8>,
	/// The face, size in half points and style bits at the caret.
	pub font: (Vec<u8>, u16, u16),
	/// The tab stops of the paragraph at the caret: position, kind, leader.
	pub tabs: Vec<(i32, u8, u8)>,
	/// True once Exit went through.
	pub quit: bool,
	/// The files that are new or changed since the run began.
	pub files: BTreeMap<String, Vec<u8>>,
}

/// What plays a script: the editor behind some shell.
pub trait Driver {
	/// The start of a run: the settings are read and the launch check runs, so the first events may be a question.
	fn start(&mut self) -> Vec<Event> {
		Vec::new()
	}
	/// A key.
	fn key(&mut self, k: Key) -> Vec<Event>;
	/// A command, as a menu sends it.
	fn command(&mut self, c: Command) -> Vec<Event>;
	/// The person's reply to `shown`.
	fn reply(&mut self, shown: &Shown, r: Reply) -> Outcome;
	/// Removes a file from the disk, as another program might.
	fn remove(&mut self, name: &str);
	/// Writes a file as another program might, with the time now.
	fn put(&mut self, _name: &str, _text: &str) {}
	/// The clock moves on this many seconds.
	fn clock(&mut self, _secs: u32) {}
	/// The keyboard is quiet for this many seconds, and the clock moves on with it.
	fn quiet(&mut self, secs: u32) {
		self.clock(secs);
	}
	/// The autosave timer fires: a recovery copy is written. False when the editor had nothing to write.
	fn autosave(&mut self) -> Vec<Event> {
		Vec::new()
	}
	/// The program is killed and started again on the same disk; the launch check runs.
	fn restart(&mut self) -> Vec<Event> {
		Vec::new()
	}
	/// A byte of every recovery copy on the disk is flipped.
	fn damage(&mut self) {}
	/// The state to compare.
	fn state(&mut self) -> State;
}

// ---- the native stub ------------------------------------------------------------------------------------------------------------

/// The disk a script runs on: in-memory files, some of which refuse to be written.
#[derive(Default)]
pub struct Disk {
	/// The files by upper-case name.
	pub inner: MemFiles,
	/// Names that cannot be created (a read-only file or folder).
	pub locked: Vec<String>,
	/// Names whose write cannot be finished (a full disk).
	pub full: Vec<String>,
}

impl Files for Disk {
	type R = MemSource;
	type W = NamedSink;

	fn open(&mut self, name: &[u8]) -> Option<MemSource> {
		self.inner.open(name)
	}

	fn create(&mut self, name: &[u8]) -> Option<NamedSink> {
		let n = String::from_utf8_lossy(name).to_ascii_uppercase();
		if self.locked.contains(&n) || n.is_empty() {
			return None;
		}
		self.inner.create(name)
	}

	fn close(&mut self, w: NamedSink) -> bool {
		// A full disk refuses the file being written: the target itself, or the temporary file a save to it writes first. A name
		// that starts with `*` is a suffix: `*.RCV` and `*.$$$` stand for every recovery copy and its temporary file.
		let name = w.name().to_string();
		let full = self.full.iter().any(|t| {
			*t == name
				|| quill_app::safe::TempName::of(t.as_bytes()).bytes() == name.as_bytes()
				|| t.strip_prefix('*')
					.is_some_and(|suffix| name.ends_with(suffix))
		});
		if full {
			return false;
		}
		self.inner.close(w)
	}

	fn save_mode(&self) -> quill_app::SaveMode {
		self.inner.save_mode()
	}

	fn remove(&mut self, name: &[u8]) -> bool {
		let n = String::from_utf8_lossy(name).to_ascii_uppercase();
		!self.locked.contains(&n) && self.inner.remove(name)
	}

	fn rename(&mut self, from: &[u8], to: &[u8]) -> bool {
		// A read-only file cannot be replaced.
		let n = String::from_utf8_lossy(to).to_ascii_uppercase();
		!self.locked.contains(&n) && self.inner.rename(from, to)
	}

	fn len(&mut self, name: &[u8]) -> Option<u32> {
		self.inner.len(name)
	}

	fn now(&mut self) -> u32 {
		self.inner.now()
	}

	fn stamp(&mut self, name: &[u8]) -> Option<Stamp> {
		self.inner.stamp(name)
	}

	fn drive(&mut self, name: &[u8]) -> Drive {
		self.inner.drive(name)
	}

	fn make_dir(&mut self, dir: &[u8]) -> bool {
		self.inner.make_dir(dir)
	}

	fn find_nth(&mut self, pattern: &[u8], n: usize, out: &mut [u8]) -> usize {
		self.inner.find_nth(pattern, n, out)
	}

	fn recovery_dir(&mut self, n: u8, out: &mut [u8]) -> usize {
		self.inner.recovery_dir(n, out)
	}

	fn instance(&mut self) -> u32 {
		self.inner.instance()
	}

	fn lock_make(&mut self, name: &[u8], stamp: &[u8; 32]) -> LockMade {
		self.inner.lock_make(name, stamp)
	}

	fn lock_owner(&mut self, name: &[u8]) -> Owner {
		self.inner.lock_owner(name)
	}
}

/// The editor with native dialogs on, answered through `quill_app::flow` the way the Windows shells answer.
pub struct NativeStub {
	app: App,
	store: HostStore,
	files: Disk,
	initial: BTreeMap<String, Vec<u8>>,
	recovery: bool,
}

impl NativeStub {
	/// A new editor over `files`.
	pub fn new(files: Disk) -> Self {
		let mut store = HostStore::new();
		let mut app = App::new();
		app.reset(&mut store).expect("a new document");
		app.set_native_dialogs(true);
		let initial = files
			.inner
			.files
			.iter()
			.map(|(k, v)| (k.clone(), v.clone()))
			.collect();
		NativeStub {
			app,
			store,
			files,
			initial,
			recovery: false,
		}
	}

	/// A new editor over `files` with the recovery protocol on, as a shell that asks the questions runs it. The list and the central
	/// folder are read from `QUILL.INI`, and the launch check has run: its question, if any, is the first event of `start`.
	pub fn with_recovery(files: Disk) -> Self {
		let mut stub = NativeStub::new(files);
		stub.recovery = true;
		stub.app.set_recovery(true);
		stub
	}

	/// Takes every dialog the editor has asked for since the last call, as the Windows shells' `handle_dialogs` does.
	fn shown(&mut self) -> Vec<Event> {
		let mut out = Vec::new();
		while let Some(d) = self.app.take_dialog() {
			match d {
				Dialog::ConfirmSave(c) => {
					out.push(Event::Show(Shown::Ask(self.app.confirm_question(c), c)))
				}
				Dialog::Open | Dialog::Picture => out.push(Event::Show(Shown::File {
					dialog: d,
					folder: self.app.last_folder().to_vec(),
					name: Vec::new(),
					type_index: 0,
				})),
				Dialog::SaveAs => {
					let st = self.app.save_as_start();
					out.push(Event::Show(Shown::File {
						dialog: d,
						folder: st.folder.to_vec(),
						name: st.name.to_vec(),
						type_index: st.type_index,
					}));
				}
				Dialog::Find(replace) => {
					let (find, with, case, word) = self.app.last_find();
					out.push(Event::Show(Shown::Find {
						replace,
						find: find.to_vec(),
						with: with.to_vec(),
						case,
						word,
					}));
				}
				Dialog::Spelling => {
					let step = self.app.spell_started(&mut self.store, &mut self.files);
					self.spell_events(step, &mut out);
				}
				Dialog::Tabs => out.push(Event::Show(Shown::Tabs)),
				Dialog::Fonts => out.push(Event::Show(Shown::Fonts)),
				Dialog::About => out.push(Event::Show(Shown::About)),
				Dialog::Print => out.push(Event::Show(Shown::Print)),
			}
		}
		if let Some(a) = self
			.app
			.take_recovery_question(&mut self.store, &mut self.files)
		{
			out.push(Event::Show(Shown::Recover(a)));
		}
		out
	}

	fn spell_events(&mut self, step: SpellStep, out: &mut Vec<Event>) {
		match step {
			SpellStep::Next => {
				let sg = self.app.spell_suggestions(&self.store);
				let with = sg
					.get(0)
					.map_or(Vec::new(), |c| c.text[..usize::from(c.len)].to_vec());
				out.push(Event::Show(Shown::Spell {
					word: self.app.spell_word().to_vec(),
					with,
				}));
			}
			SpellStep::Done(n) => out.push(Event::Notice(n)),
			SpellStep::Stay | SpellStep::Cancelled => {}
		}
	}
}

fn failed(r: Result<(), Notice>, out: &mut Vec<Event>) {
	if let Err(n) = r {
		out.push(Event::Notice(n));
	}
}

impl Driver for NativeStub {
	fn start(&mut self) -> Vec<Event> {
		if self.recovery {
			self.app
				.load_settings_with(&mut self.store, &mut self.files);
			self.app
				.recovery_check_launch(&mut self.store, &mut self.files);
		}
		self.shown()
	}

	fn key(&mut self, k: Key) -> Vec<Event> {
		self.app.key(&mut self.store, &mut self.files, k);
		self.shown()
	}

	fn command(&mut self, c: Command) -> Vec<Event> {
		self.app.command(&mut self.store, &mut self.files, c);
		self.shown()
	}

	fn reply(&mut self, shown: &Shown, r: Reply) -> Outcome {
		let (s, f) = (&mut self.store, &mut self.files);
		let mut out = Vec::new();
		let mut closed = true;
		match (shown, r) {
			(Shown::Ask(_, c), Reply::Answer(a)) => self.app.confirm_answered(s, f, *c, a),
			(Shown::Ask(_, c), Reply::Cancel) => {
				self.app.confirm_answered(s, f, *c, Answer::Cancel)
			}
			(Shown::Recover(_), Reply::Answer(a)) => self.app.recovery_answered(s, f, a),
			(Shown::Recover(_), Reply::Cancel) => self.app.recovery_answered(s, f, Answer::Cancel),
			(Shown::File { dialog, .. }, Reply::Chosen(path)) => match dialog {
				Dialog::Open => failed(self.app.open_chosen(s, f, &path), &mut out),
				Dialog::SaveAs => failed(self.app.save_as_chosen(s, f, &path), &mut out),
				_ => failed(self.app.picture_chosen(s, f, &path), &mut out),
			},
			(Shown::File { dialog, .. }, Reply::Cancel) => self.app.dialog_cancelled(*dialog),
			(
				Shown::Find { .. },
				Reply::Find {
					action,
					find,
					with,
					case,
					word,
				},
			) => {
				let fields = FindFields {
					find: &find,
					with: &with,
					case,
					word,
				};
				out.extend(self.app.find_pressed(s, fields, action).map(Event::Notice));
				closed = false;
			}
			(Shown::Spell { .. }, Reply::Spell { button, with }) => {
				let step = self.app.spell_pressed(s, f, button, &with);
				closed = !matches!(step, SpellStep::Next | SpellStep::Stay);
				self.spell_events(step, &mut out);
			}
			(Shown::Spell { .. }, Reply::Cancel | Reply::Close) => self.app.spell_cancel(),
			(
				Shown::Tabs,
				Reply::Tab {
					button,
					text,
					kind,
					leader,
				},
			) => {
				out.extend(
					self.app
						.tab_pressed(s, &text, kind, leader, button)
						.map(Event::Notice),
				);
				closed = false;
			}
			(
				Shown::Fonts,
				Reply::Fonts {
					face,
					half_points,
					flags,
				},
			) => self.app.set_font(s, &face, 2, half_points, flags),
			(Shown::Print, Reply::Print(dest)) => {
				failed(self.app.print_chosen(s, f, &dest), &mut out)
			}
			_ => {}
		}
		if closed {
			out.extend(self.shown());
		}
		Outcome {
			events: out,
			closed,
		}
	}

	fn remove(&mut self, name: &str) {
		self.files.inner.files.remove(&name.to_ascii_uppercase());
	}

	fn put(&mut self, name: &str, text: &str) {
		let n = name.to_ascii_uppercase();
		self.files
			.inner
			.times
			.insert(n.clone(), self.files.inner.clock);
		self.files.inner.files.insert(n, text.as_bytes().to_vec());
	}

	fn clock(&mut self, secs: u32) {
		let c = &mut self.files.inner.clock;
		*c = crate::script::dos_add(*c, secs);
	}

	fn autosave(&mut self) -> Vec<Event> {
		let (s, f) = (&mut self.store, &mut self.files);
		self.app.set_message("");
		self.app.save_recovery(s, f, Why::Autosave);
		let mut out = Vec::new();
		if !self.app.message().is_empty() {
			out.push(Event::Notice(Notice::status(self.app.message())));
		}
		out
	}

	fn restart(&mut self) -> Vec<Event> {
		self.store = HostStore::new();
		self.app = App::new();
		self.app.reset(&mut self.store).expect("a new document");
		self.app.set_native_dialogs(true);
		self.app.set_recovery(self.recovery);
		self.start()
	}

	fn damage(&mut self) {
		for (k, v) in self.files.inner.files.iter_mut() {
			if (k.ends_with(".RCV") || k.ends_with(".RC0")) && v.len() > 100 {
				let last = v.len() - 1;
				v[last] ^= 0x55;
			}
		}
	}

	fn state(&mut self) -> State {
		let mut text = Vec::new();
		let mut at = 0;
		let mut buf = [0u8; 512];
		loop {
			let n = self.app.doc().body.read(&self.store, at, &mut buf);
			if n == 0 {
				break;
			}
			text.extend_from_slice(&buf[..n]);
			at += n as u32;
		}
		// The last mark ends the document; it is not text.
		text.pop();
		let cs = self.app.char_state(&self.store);
		let mut stops = [(0, 0, 0); 16];
		let n = self.app.tab_stops_full(&self.store, &mut stops);
		State {
			text: String::from_utf8_lossy(&text).replace('\r', "\n"),
			caret: self.app.caret(),
			selection: self.app.selection(),
			name: self.app.name().to_vec(),
			modified: self.app.modified(),
			status: self.app.message().to_vec(),
			recent: self.app.recent_files().map(<[u8]>::to_vec).collect(),
			folder: self.app.last_folder().to_vec(),
			font: (
				cs.face[..usize::from(cs.face_len)].to_vec(),
				cs.size,
				cs.flags,
			),
			tabs: stops[..n].to_vec(),
			quit: self.app.quit(),
			files: self
				.files
				.inner
				.files
				.iter()
				.filter(|(k, v)| self.initial.get(*k) != Some(*v))
				.map(|(k, v)| (k.clone(), v.clone()))
				.collect(),
		}
	}
}

// ---- the script format ----------------------------------------------------------------------------------------------------------

/// One word of a script line: bare or quoted, with a `key=` prefix if it had one.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Tok {
	key: Option<String>,
	val: String,
	quoted: bool,
}

/// Splits a line into words. A `"` starts a quoted part with `\"`, `\\`, `\n` and `\t` escapes; `#` outside quotes starts a comment.
fn tokens(line: &str) -> Result<Vec<Tok>, String> {
	let mut out = Vec::new();
	let mut ch = line.chars().peekable();
	while let Some(&c) = ch.peek() {
		if c.is_whitespace() {
			ch.next();
			continue;
		}
		if c == '#' {
			break;
		}
		let (mut word, mut key, mut quoted) = (String::new(), None, false);
		while let Some(&c) = ch.peek() {
			if c.is_whitespace() {
				break;
			}
			ch.next();
			match c {
				'=' if !quoted && key.is_none() && !word.is_empty() => {
					key = Some(std::mem::take(&mut word));
				}
				'"' => {
					quoted = true;
					loop {
						match ch.next() {
							None => return Err("an unclosed quote".to_string()),
							Some('"') => break,
							Some('\\') => match ch.next() {
								Some('n') => word.push('\n'),
								Some('t') => word.push('\t'),
								Some('"') => word.push('"'),
								Some('\\') => word.push('\\'),
								Some(o) => {
									word.push('\\');
									word.push(o);
								}
								None => return Err("an unclosed quote".to_string()),
							},
							Some(o) => word.push(o),
						}
					}
				}
				o => word.push(o),
			}
		}
		out.push(Tok {
			key,
			val: word,
			quoted,
		});
	}
	Ok(out)
}

/// A key token: the `.keys` names, plus `ctrl-a`, `ctrlhome`, `ctrlend` and `shift-left` and its kin.
fn parse_key(t: &str) -> Option<Key> {
	use quill_app::Dir;
	if let Some(k) = crate::script::parse_token(t) {
		return Some(k);
	}
	if let Some(l) = t.strip_prefix("ctrl-") {
		let b = l.as_bytes();
		return (b.len() == 1).then(|| Key::Ctrl(b[0].to_ascii_lowercase()));
	}
	if let Some(d) = t.strip_prefix("shift-") {
		return Some(Key::Select(match d {
			"left" => Dir::Left,
			"right" => Dir::Right,
			"up" => Dir::Up,
			"down" => Dir::Down,
			"home" => Dir::Home,
			"end" => Dir::End,
			"pageup" => Dir::PageUp,
			"pagedown" => Dir::PageDown,
			_ => return None,
		}));
	}
	match t {
		"ctrlhome" => Some(Key::CtrlHome),
		"ctrlend" => Some(Key::CtrlEnd),
		_ => None,
	}
}

fn parse_command(name: &str, arg: Option<&str>) -> Option<Command> {
	Some(match name.to_ascii_lowercase().as_str() {
		"new" => Command::New,
		"open" => Command::Open,
		"save" => Command::Save,
		"saveas" => Command::SaveAs,
		"exit" => Command::Exit,
		"undo" => Command::Undo,
		"redo" => Command::Redo,
		"selectall" => Command::SelectAll,
		"fonts" => Command::Fonts,
		"tabs" => Command::Tabs,
		"about" => Command::About,
		"picture" => Command::Picture,
		"print" => Command::Print,
		"spelling" => Command::Spelling,
		"find" => Command::Find,
		"findnext" => Command::FindNext,
		"replace" => Command::Replace,
		"bold" => Command::Bold,
		"italic" => Command::Italic,
		"recent" => Command::Recent(arg?.parse().ok()?),
		_ => return None,
	})
}

fn parse_answer(t: &str) -> Option<Answer> {
	Some(match t.to_ascii_lowercase().as_str() {
		"yes" => Answer::Yes,
		"no" => Answer::No,
		"cancel" => Answer::Cancel,
		_ => return None,
	})
}

/// The buttons of the recovery questions by their names: Recover is Yes, Discard is No, and Later and Open original (which Esc
/// presses) are Cancel.
fn parse_recover_answer(t: &str) -> Option<Answer> {
	Some(match t.to_ascii_lowercase().as_str() {
		"recover" => Answer::Yes,
		"discard" => Answer::No,
		"later" | "original" => Answer::Cancel,
		other => return parse_answer(other),
	})
}

/// Quotes text for the transcript: `"`, `\` and line ends escaped.
fn quote(s: &str) -> String {
	let mut o = String::from("\"");
	for c in s.chars() {
		match c {
			'"' => o.push_str("\\\""),
			'\\' => o.push_str("\\\\"),
			'\n' => o.push_str("\\n"),
			'\t' => o.push_str("\\t"),
			c => o.push(c),
		}
	}
	o.push('"');
	o
}

fn bytes_quote(b: &[u8]) -> String {
	quote(&String::from_utf8_lossy(b))
}

/// An event as the words it is logged and matched by: a kind, bare words, an optional text and `key=value` fields.
struct Words {
	kind: &'static str,
	args: Vec<String>,
	text: Option<String>,
	fields: Vec<(&'static str, String)>,
}

fn dialog_name(d: Dialog) -> &'static str {
	match d {
		Dialog::Open => "Open",
		Dialog::SaveAs => "SaveAs",
		_ => "Picture",
	}
}

fn words_of(e: &Event) -> Words {
	let w = |kind, args: &[&str], text: Option<String>, fields| Words {
		kind,
		args: args.iter().map(|s| (*s).to_string()).collect(),
		text,
		fields,
	};
	match e {
		Event::Show(Shown::Ask(a, _)) => w(
			"ask",
			&[],
			Some(String::from_utf8_lossy(a.text.as_bytes()).into_owned()),
			vec![
				("buttons", format!("{:?}", a.buttons)),
				("default", format!("{:?}", a.default)),
			],
		),
		Event::Show(Shown::Recover(a)) => w(
			"recover",
			&[],
			Some(String::from_utf8_lossy(a.text.as_bytes()).into_owned()),
			vec![
				("buttons", format!("{:?}", a.buttons)),
				("default", format!("{:?}", a.default)),
			],
		),
		Event::Show(Shown::File {
			dialog,
			folder,
			name,
			type_index,
		}) => w(
			"file",
			&[dialog_name(*dialog)],
			None,
			vec![
				("folder", String::from_utf8_lossy(folder).into_owned()),
				("name", String::from_utf8_lossy(name).into_owned()),
				("type", type_index.to_string()),
			],
		),
		Event::Show(Shown::Find {
			replace,
			find,
			with,
			case,
			word,
		}) => w(
			"dialog",
			&[if *replace { "Replace" } else { "Find" }],
			None,
			vec![
				("find", String::from_utf8_lossy(find).into_owned()),
				("with", String::from_utf8_lossy(with).into_owned()),
				("case", case.to_string()),
				("word", word.to_string()),
			],
		),
		Event::Show(Shown::Spell { word, with }) => w(
			"spell",
			&[],
			Some(String::from_utf8_lossy(word).into_owned()),
			vec![("with", String::from_utf8_lossy(with).into_owned())],
		),
		Event::Show(Shown::Tabs) => w("dialog", &["Tabs"], None, vec![]),
		Event::Show(Shown::Fonts) => w("dialog", &["Fonts"], None, vec![]),
		Event::Show(Shown::About) => w("dialog", &["About"], None, vec![]),
		Event::Show(Shown::Print) => w("dialog", &["Print"], None, vec![]),
		Event::Notice(n) => w(
			"notice",
			&[match n.kind {
				NoticeKind::Info => "Info",
				NoticeKind::Warn => "Warn",
				NoticeKind::Status => "Status",
			}],
			Some(String::from_utf8_lossy(n.text.as_bytes()).into_owned()),
			vec![],
		),
	}
}

fn log_line(e: &Event) -> String {
	let w = words_of(e);
	let mut l = format!("< {}", w.kind);
	for a in &w.args {
		let _ = write!(l, " {a}");
	}
	if let Some(t) = &w.text {
		let _ = write!(l, " {}", quote(t));
	}
	for (k, v) in &w.fields {
		let _ = write!(l, " {k}={}", quote(v));
	}
	l
}

/// True when every part of the expectation holds for the event.
fn matches(expect: &[Tok], e: &Event) -> Result<(), String> {
	let w = words_of(e);
	let mut it = expect.iter();
	let kind = it.next().ok_or("expect what?")?;
	if kind.val != w.kind {
		return Err(format!("wanted a {}, got `{}`", kind.val, log_line(e)));
	}
	let rest: Vec<&Tok> = it.collect();
	let mut i = 0;
	// Bare words name the dialog or the notice kind.
	let bare: Vec<&Tok> = rest
		.iter()
		.copied()
		.filter(|t| t.key.is_none() && !t.quoted && !t.val.eq_ignore_ascii_case("default"))
		.collect();
	for (b, a) in bare.iter().zip(&w.args) {
		if !b.val.eq_ignore_ascii_case(a) {
			return Err(format!("wanted {}, got `{}`", b.val, log_line(e)));
		}
	}
	while let Some(t) = rest.get(i) {
		i += 1;
		if t.quoted && t.key.is_none() {
			if Some(&t.val) != w.text.as_ref() {
				return Err(format!("wanted {}, got `{}`", quote(&t.val), log_line(e)));
			}
		} else if t.key.is_none() && t.val.eq_ignore_ascii_case("default") {
			let want = rest.get(i).map(|t| t.val.as_str()).unwrap_or("");
			i += 1;
			let got = w.fields.iter().find(|(k, _)| *k == "default").map(|f| &f.1);
			if got.map(|g| g.to_ascii_lowercase()) != Some(want.to_ascii_lowercase()) {
				return Err(format!("wanted default {want}, got `{}`", log_line(e)));
			}
		} else if let Some(k) = &t.key {
			let got = w.fields.iter().find(|(f, _)| f == k).map(|f| &f.1);
			if got != Some(&t.val) {
				return Err(format!(
					"wanted {k}={}, got `{}`",
					quote(&t.val),
					log_line(e)
				));
			}
		}
	}
	Ok(())
}

/// A script: files to put on the disk, then the lines.
pub struct Flow {
	/// Test documents to copy to the disk, by name.
	pub copy: Vec<String>,
	/// Files to create: name and text.
	pub files: Vec<(String, String)>,
	/// Names that cannot be created.
	pub locked: Vec<String>,
	/// Names whose write fails at the end.
	pub full: Vec<String>,
	/// The recovery protocol is on.
	pub recovery: bool,
	/// Documents another Quill window has open with changes.
	pub alive: Vec<String>,
	/// Places that are not fixed disks: the start of the path and what it is (`removable`, `readonly` or `missing`).
	pub drives: Vec<(String, String)>,
	/// The step lines with their line numbers.
	lines: Vec<(usize, String)>,
}

/// Reads a script. Directives are `@copy NAME` (a file of `tests/docs`), `@file NAME "text"`, `@locked NAME` (the name cannot be
/// created), `@full NAME` (writing the name cannot be finished; `*.RCV` stands for every name that ends so), `@recovery` (the editor
/// runs the recovery protocol), `@alive NAME` (another Quill window has NAME open with changes) and `@drive PATH removable|readonly|missing`.
pub fn parse_flow(text: &str) -> Result<Flow, String> {
	let mut fl = Flow {
		copy: Vec::new(),
		files: Vec::new(),
		locked: Vec::new(),
		full: Vec::new(),
		recovery: false,
		alive: Vec::new(),
		drives: Vec::new(),
		lines: Vec::new(),
	};
	for (i, raw) in text.lines().enumerate() {
		let toks = tokens(raw).map_err(|e| format!("line {}: {e}", i + 1))?;
		let Some(first) = toks.first() else {
			continue;
		};
		match first.val.as_str() {
			"@copy" => fl.copy.extend(toks.get(1).map(|t| t.val.clone())),
			"@locked" => fl
				.locked
				.extend(toks.get(1).map(|t| t.val.to_ascii_uppercase())),
			"@full" => fl
				.full
				.extend(toks.get(1).map(|t| t.val.to_ascii_uppercase())),
			"@recovery" => fl.recovery = true,
			"@alive" => fl
				.alive
				.extend(toks.get(1).map(|t| t.val.to_ascii_uppercase())),
			"@drive" => match (toks.get(1), toks.get(2)) {
				(Some(p), Some(k)) => fl.drives.push((p.val.to_ascii_uppercase(), k.val.clone())),
				_ => return Err(format!("line {}: @drive PATH KIND", i + 1)),
			},
			"@file" => {
				let (n, c) = (toks.get(1), toks.get(2));
				match (n, c) {
					(Some(n), Some(c)) => fl.files.push((n.val.clone(), c.val.clone())),
					_ => return Err(format!("line {}: @file NAME \"text\"", i + 1)),
				}
			}
			_ => fl.lines.push((i + 1, raw.to_string())),
		}
	}
	Ok(fl)
}

/// The disk a script starts with: its `@copy` files from `docs` and its `@file` files.
pub fn flow_files(fl: &Flow, docs: &std::path::Path) -> Disk {
	let mut disk = Disk {
		locked: fl.locked.clone(),
		full: fl.full.clone(),
		..Disk::default()
	};
	let files = &mut disk.inner;
	for (p, k) in &fl.drives {
		let kind = match k.as_str() {
			"removable" => Drive::Removable,
			"readonly" => Drive::ReadOnly,
			_ => Drive::Missing,
		};
		files.drives.push((p.clone(), kind));
	}
	for n in &fl.alive {
		let lock = quill_app::recovery::beside(
			quill_app::recovery::Style::native(),
			quill_app::recovery::Kind::Lock,
			n.as_bytes(),
			0,
		)
		.expect("a lock name");
		files
			.alive
			.push(String::from_utf8_lossy(lock.bytes()).to_ascii_uppercase());
		files.files.insert(
			String::from_utf8_lossy(lock.bytes()).to_ascii_uppercase(),
			vec![0; 32],
		);
	}
	for n in &fl.copy {
		let bytes = std::fs::read(docs.join(n)).unwrap_or_else(|_| panic!("missing test file {n}"));
		files.files.insert(n.to_ascii_uppercase(), bytes);
	}
	for (n, c) in &fl.files {
		files
			.files
			.insert(n.to_ascii_uppercase(), c.as_bytes().to_vec());
	}
	disk
}

// ---- the interpreter ------------------------------------------------------------------------------------------------------------

struct Run<'a, D: Driver> {
	d: &'a mut D,
	log: String,
	events: VecDeque<Event>,
	current: Option<Shown>,
}

impl<D: Driver> Run<'_, D> {
	fn idle(&self) -> Result<(), String> {
		if let Some(e) = self.events.front() {
			return Err(format!("unexpected `{}`", log_line(e)));
		}
		if self.current.is_some() {
			return Err("a dialog is open: answer it first".to_string());
		}
		Ok(())
	}

	/// Takes in what the driver raised: each event is logged and queued for `expect`, and the last dialog becomes the open one.
	fn raised(&mut self, evs: Vec<Event>) {
		for e in evs {
			let _ = writeln!(self.log, "{}", log_line(&e));
			if let Event::Show(s) = &e {
				self.current = Some(s.clone());
			}
			self.events.push_back(e);
		}
	}

	fn step(&mut self, line: &str) -> Result<(), String> {
		let toks = tokens(line)?;
		let Some(verb) = toks.first().map(|t| t.val.as_str()) else {
			return Ok(());
		};
		let arg = |i: usize| toks.get(i).map(|t| t.val.as_str());
		let need = |i: usize, what: &str| arg(i).ok_or(format!("{verb} needs {what}"));
		if verb == "expect" {
			let e = self.events.pop_front().ok_or("nothing to expect")?;
			return matches(toks.get(1..).unwrap_or(&[]), &e);
		}
		let _ = writeln!(
			self.log,
			"> {}",
			line.split('#').next().unwrap_or("").trim()
		);
		match verb {
			"type" => {
				self.idle()?;
				let mut evs = Vec::new();
				for b in need(1, "text")?.bytes() {
					evs.extend(
						self.d
							.key(if b == b'\n' { Key::Enter } else { Key::Char(b) }),
					);
				}
				self.raised(evs);
			}
			"key" => {
				self.idle()?;
				let mut evs = Vec::new();
				for t in toks.get(1..).unwrap_or(&[]) {
					let k = parse_key(&t.val).ok_or(format!("unknown key {}", t.val))?;
					evs.extend(self.d.key(k));
				}
				self.raised(evs);
			}
			"cmd" => {
				self.idle()?;
				let c = parse_command(need(1, "a command")?, arg(2))
					.ok_or(format!("unknown command {}", arg(1).unwrap_or("")))?;
				let evs = self.d.command(c);
				self.raised(evs);
			}
			"rm" => {
				self.idle()?;
				self.d.remove(need(1, "a file name")?);
			}
			"put" => {
				self.idle()?;
				let (n, t) = (need(1, "a file name")?, need(2, "text")?);
				self.d.put(n, t);
			}
			"@clock" | "@quiet" => {
				self.idle()?;
				let n: u32 = need(1, "a number of seconds")?
					.trim_start_matches('+')
					.trim_end_matches('s')
					.parse()
					.map_err(|_| format!("{verb} takes seconds, such as +30s"))?;
				if verb == "@clock" {
					self.d.clock(n);
				} else {
					self.d.quiet(n);
				}
			}
			"autosave" => {
				self.idle()?;
				let evs = self.d.autosave();
				self.raised(evs);
			}
			"restart" => {
				self.idle()?;
				let evs = self.d.restart();
				self.raised(evs);
			}
			"damage" => {
				self.idle()?;
				self.d.damage();
			}
			"dump" => {
				self.events_done()?;
				let st = self.d.state();
				dump_state(&st, &mut self.log);
			}
			_ => self.reply_step(verb, &toks)?,
		}
		Ok(())
	}

	fn events_done(&self) -> Result<(), String> {
		match self.events.front() {
			Some(e) => Err(format!("unexpected `{}`", log_line(e))),
			None => Ok(()),
		}
	}

	/// A step that answers the open dialog.
	fn reply_step(&mut self, verb: &str, toks: &[Tok]) -> Result<(), String> {
		// The question or dialog that was just raised does not have to be expected before it is answered.
		if matches!(self.events.front(), Some(Event::Show(_))) && self.events.len() == 1 {
			self.events.pop_front();
		}
		self.events_done()?;
		let shown = self
			.current
			.clone()
			.ok_or(format!("{verb}: no dialog is open"))?;
		let arg = |i: usize| toks.get(i).map(|t| t.val.as_str());
		let field = |k: &str| {
			toks.iter()
				.find(|t| t.key.as_deref() == Some(k))
				.map(|t| t.val.as_str())
		};
		let flag = |k: &str| {
			toks.iter()
				.any(|t| t.key.is_none() && !t.quoted && t.val == k)
		};
		let bytes = |s: Option<&str>| s.unwrap_or("").as_bytes().to_vec();
		let reply = match (verb, &shown) {
			("answer", Shown::Ask(..)) => Reply::Answer(
				arg(1)
					.and_then(parse_answer)
					.ok_or("answer yes, no or cancel")?,
			),
			("answer", Shown::Recover(_)) => Reply::Answer(
				arg(1)
					.and_then(parse_recover_answer)
					.ok_or("answer recover, discard, later or original (or yes, no, cancel)")?,
			),
			("choose", Shown::File { .. }) => Reply::Chosen(bytes(arg(1))),
			("cancel", _) => Reply::Cancel,
			("close", _) => Reply::Close,
			("find", Shown::Find { .. }) => Reply::Find {
				action: match arg(1) {
					Some("next") => FindAction::Next,
					Some("replace") => FindAction::Replace,
					Some("all") => FindAction::ReplaceAll,
					_ => return Err("find next, replace or all".to_string()),
				},
				find: bytes(arg(2)),
				with: bytes(field("with")),
				case: flag("case"),
				word: flag("word"),
			},
			("spell", Shown::Spell { .. }) => Reply::Spell {
				button: match arg(1) {
					Some("ignore") => SpellButton::Ignore,
					Some("ignoreall") => SpellButton::IgnoreAll,
					Some("change") => SpellButton::Change,
					Some("changeall") => SpellButton::ChangeAll,
					Some("add") => SpellButton::Add,
					_ => {
						return Err("spell ignore, ignoreall, change, changeall or add".to_string())
					}
				},
				with: bytes(arg(2)),
			},
			("tabs", Shown::Tabs) => {
				let num = |k| field(k).and_then(|v| v.parse().ok()).unwrap_or(0);
				Reply::Tab {
					button: match arg(1) {
						Some("set") => TabButton::Set,
						Some("clear") => TabButton::Clear,
						Some("clearall") => TabButton::ClearAll,
						_ => return Err("tabs set, clear or clearall".to_string()),
					},
					text: bytes(arg(2)),
					kind: num("kind"),
					leader: num("leader"),
				}
			}
			("fonts", Shown::Fonts) => {
				let mut flags = 0;
				for (name, bit) in [("bold", 1), ("italic", 2), ("underline", 4), ("strike", 8)] {
					if flag(name) {
						flags |= bit;
					}
				}
				Reply::Fonts {
					face: bytes(arg(1)),
					half_points: arg(2)
						.and_then(|v| v.parse().ok())
						.ok_or("fonts FACE HALF-POINTS")?,
					flags,
				}
			}
			("print", Shown::Print) => Reply::Print(bytes(arg(1))),
			_ => return Err(format!("{verb} does not fit the open dialog")),
		};
		let out = self.d.reply(&shown, reply);
		if out.closed {
			self.current = None;
		}
		self.raised(out.events);
		Ok(())
	}
}

fn text_or_hash(b: &[u8]) -> String {
	let plain = !b.is_empty()
		&& b.len() <= 400
		&& b.iter()
			.all(|c| matches!(c, b'\n' | b'\r' | b'\t' | 0x20..=0x7E));
	if plain {
		format!("{} bytes {}", b.len(), bytes_quote(b))
	} else {
		let mut h = 0xcbf2_9ce4_8422_2325u64;
		for c in b {
			h = (h ^ u64::from(*c)).wrapping_mul(0x0100_0000_01b3);
		}
		format!("{} bytes fnv1a={h:016x}", b.len())
	}
}

/// Writes the state block of a transcript.
pub fn dump_state(st: &State, out: &mut String) {
	let _ = writeln!(out, "= state");
	let _ = writeln!(out, "text: {}", quote(&st.text));
	let sel = st
		.selection
		.map_or("none".to_string(), |(a, b)| format!("{a}..{b}"));
	let _ = writeln!(out, "caret: {}  selection: {sel}", st.caret);
	let _ = writeln!(out, "name: {}", bytes_quote(&st.name));
	let _ = writeln!(out, "modified: {}", st.modified);
	let _ = writeln!(out, "status: {}", bytes_quote(&st.status));
	let recent: Vec<String> = st.recent.iter().map(|r| bytes_quote(r)).collect();
	let _ = writeln!(out, "recent: [{}]", recent.join(", "));
	let _ = writeln!(out, "folder: {}", bytes_quote(&st.folder));
	let _ = writeln!(
		out,
		"font: {} size={} flags={}",
		bytes_quote(&st.font.0),
		st.font.1,
		st.font.2
	);
	let tabs: Vec<String> = st
		.tabs
		.iter()
		.map(|(p, k, l)| format!("{p}:{k}:{l}"))
		.collect();
	let _ = writeln!(out, "tabs: [{}]", tabs.join(", "));
	let _ = writeln!(out, "quit: {}", st.quit);
	for (n, b) in &st.files {
		let _ = writeln!(out, "file {n}: {}", text_or_hash(b));
	}
}

/// Plays a script through a driver and returns the transcript: each step, each dialog and notice raised, and the final state.
/// `Err` names the line that failed and what the editor did instead.
pub fn run_flow<D: Driver>(fl: &Flow, d: &mut D) -> Result<String, String> {
	let mut r = Run {
		d,
		log: String::new(),
		events: VecDeque::new(),
		current: None,
	};
	let evs = r.d.start();
	r.raised(evs);
	for (n, line) in &fl.lines {
		r.step(line)
			.map_err(|e| format!("line {n}: {e}\n{}", r.log))?;
	}
	r.events_done()
		.map_err(|e| format!("at the end: {e}\n{}", r.log))?;
	if r.current.is_some() {
		return Err(format!("at the end: a dialog is still open\n{}", r.log));
	}
	// A script that ends on a dump has already shown its final state.
	if fl.lines.last().is_none_or(|(_, l)| l.trim() != "dump") {
		let st = r.d.state();
		dump_state(&st, &mut r.log);
	}
	Ok(r.log)
}
