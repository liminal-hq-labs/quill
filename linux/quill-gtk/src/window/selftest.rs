// The test hook behind `--self-test-dialogs`: every dialog shown, pressed and answered through the window, with what each did.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// It runs in the real window with a `ScriptChooser` in place of GTK's file dialogs and question, so Open, Save As, Insert Picture
// and the unsaved-changes question are answered with no one to click. Each dialog and each button prints one line, `dialog NAME
// [button ID]: STATE; effect: WHAT`, and the question prints `question COMMAND ANSWER: effect: WHAT`, read from the document and
// the disk after the answer, not from the widgets. `tests/dialogs.rs` runs this under its own Xvfb and checks the lines.

use std::path::PathBuf;
use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;
use quill_app::{Command, Dialog};
use quill_gtk::choose::{Reply, ScriptChooser, Step};
use quill_gtk::dialogs::{self, document_text, DialogView};
use quill_gtk::keys::{Mapped, Move};

use super::Ui;

/// A one-pixel, 24-bit Windows bitmap.
fn pixel_bmp() -> Vec<u8> {
	let mut b = b"BM".to_vec();
	b.extend_from_slice(&58u32.to_le_bytes());
	b.extend_from_slice(&[0; 4]);
	b.extend_from_slice(&54u32.to_le_bytes());
	b.extend_from_slice(&40u32.to_le_bytes());
	b.extend_from_slice(&1i32.to_le_bytes());
	b.extend_from_slice(&1i32.to_le_bytes());
	b.extend_from_slice(&1u16.to_le_bytes());
	b.extend_from_slice(&24u16.to_le_bytes());
	b.extend_from_slice(&[0; 4]);
	b.extend_from_slice(&4u32.to_le_bytes());
	b.extend_from_slice(&[0x13, 0x0b, 0, 0, 0x13, 0x0b, 0, 0]);
	b.extend_from_slice(&[0; 8]);
	b.extend_from_slice(&[0xff, 0x80, 0x20, 0]);
	b
}

/// Runs the main loop's pending turns, which is where a script answers and a dialog maps.
fn pump() {
	let ctx = glib::MainContext::default();
	for _ in 0..6 {
		while ctx.iteration(false) {}
		std::thread::sleep(std::time::Duration::from_millis(20));
	}
}

fn say(line: &str) {
	println!("{line}");
}

impl Ui {
	/// The test hook behind `--self-test-dialogs`.
	pub(super) fn self_test_dialogs(self: &Rc<Self>) {
		let script = ScriptChooser::new(vec![], true);
		self.use_script(script.clone());
		// A test that wants to look at the files names the folder and keeps it.
		let kept = std::env::var_os("QUILL_SELFTEST_DIR").map(PathBuf::from);
		let dir = kept.clone().unwrap_or_else(|| {
			std::env::temp_dir().join(format!("quill-dialogs-{}", std::process::id()))
		});
		let _ = std::fs::create_dir_all(&dir);
		let mut t = Test {
			ui: self.clone(),
			script,
			dir,
		};
		t.display_dialogs();
		t.file_dialogs();
		t.questions();
		if kept.is_none() {
			let _ = std::fs::remove_dir_all(&t.dir);
		}
		self.closing.set(true);
		self.window.close();
	}
}

struct Test {
	ui: Rc<Ui>,
	script: Rc<ScriptChooser>,
	dir: PathBuf,
}

impl Test {
	fn text(&self) -> String {
		document_text(&self.ui.doc.borrow())
	}

	fn selection(&self) -> Option<(u32, u32)> {
		self.ui.doc.borrow().selection()
	}

	/// A new document holding `text`.
	fn fresh(&self, text: &str) {
		self.script.clear();
		let mut d = self.ui.doc.borrow_mut();
		d.new_document();
		d.type_text(text);
		drop(d);
		self.ui.changed();
	}

	fn to_start(&self) {
		self.ui.doc.borrow_mut().apply(Mapped::Move {
			to: Move::DocStart,
			extend: false,
		});
	}

	fn to_end(&self) {
		self.ui.doc.borrow_mut().apply(Mapped::Move {
			to: Move::DocEnd,
			extend: false,
		});
	}

	/// A named, saved document holding `text`.
	fn saved(&self, name: &str, text: &str) -> PathBuf {
		self.fresh(text);
		let path = self.dir.join(name);
		self.ui.doc.borrow_mut().save_as(&path).unwrap();
		path
	}

	/// Types more, so the document has unsaved changes.
	fn dirty(&self) {
		self.to_end();
		self.ui.doc.borrow_mut().type_text(" changed");
	}

	fn run(&self, c: Command) {
		self.ui.run_command(c);
		pump();
	}

	/// Shows `v`, says so, and lets the dialog map.
	fn shown(&self, name: &str, v: &Option<Rc<DialogView>>) -> bool {
		pump();
		match v {
			Some(v) if v.dialog.is_mapped() => {
				say(&format!(
					"dialog {name}: shown; effect: focus in control {:?}",
					v.focused_id()
				));
				true
			}
			Some(_) => {
				say(&format!("dialog {name}: shown but not mapped"));
				false
			}
			None => {
				say(&format!("dialog {name}: not shown"));
				false
			}
		}
	}

	/// Presses button `id` of `v` and says what `effect` finds afterwards.
	fn press(&self, name: &str, v: &Rc<DialogView>, id: u16, effect: impl FnOnce(&Test) -> String) {
		let pressed = v.press(id);
		pump();
		let state = if pressed {
			"answered"
		} else {
			"no such button"
		};
		say(&format!(
			"dialog {name} button {id}: {state}; effect: {}",
			effect(self)
		));
	}

	fn close(&self, v: &Rc<DialogView>) {
		if v.dialog.is_mapped() {
			v.dialog.force_close();
		}
		pump();
	}

	fn env(&self) -> dialogs::Env {
		self.ui.env()
	}

	fn display_dialogs(&mut self) {
		use quill_dialogs::{find as f, fonts as fo, spelling as sp, tabs as ta};
		let ui = self.ui.clone();

		// About.
		ui.handle_dialog(Dialog::About);
		pump();
		let about = ui.window.visible_dialog().is_some();
		say(&format!(
			"dialog about: {}",
			if about { "shown" } else { "not shown" }
		));
		if let Some(d) = ui.window.visible_dialog() {
			d.force_close();
		}
		pump();

		// Find.
		self.fresh("alpha beta alpha");
		self.to_start();
		let v = dialogs::find(&self.env(), false);
		if self.shown("find", &v) {
			let v = v.unwrap();
			v.set_text(f::WHAT, "alpha");
			self.press("find", &v, f::FIND_NEXT, |t| {
				format!("selection {:?}", t.selection())
			});
			v.set_text(f::WITH, "omega");
			self.press("find", &v, f::REPLACE, |t| {
				format!("text {:?}, selection {:?}", t.text(), t.selection())
			});
			v.set_text(f::WHAT, "zzz");
			self.press("find", &v, f::FIND_NEXT, |t| {
				format!("not found, text {:?}", t.text())
			});
			v.set_text(f::WHAT, "ALPHA");
			v.set_checked(f::MATCH_CASE, true);
			self.press("find", &v, f::FIND_NEXT, |t| {
				format!("match case, nothing found, selection {:?}", t.selection())
			});
			v.set_checked(f::MATCH_CASE, false);
			v.set_checked(f::WHOLE_WORD, true);
			v.set_text(f::WHAT, "alph");
			self.press("find", &v, f::FIND_NEXT, |t| {
				format!("whole word, nothing found, selection {:?}", t.selection())
			});
			v.set_checked(f::WHOLE_WORD, false);
			v.set_text(f::WHAT, "alpha");
			v.set_text(f::WITH, "omega");
			self.press("find", &v, f::REPLACE_ALL, |t| {
				format!("text {:?}", t.text())
			});
			self.press("find", &v, 2, |t| format!("text {:?}", t.text()));
			pump();
		}

		// Replace: the same box with the focus on the replacement.
		let v = dialogs::find(&self.env(), true);
		if self.shown("replace", &v) {
			let v = v.unwrap();
			say(&format!(
				"dialog replace: focus on the replacement is {}",
				v.focused_id() == Some(f::WITH)
			));
			self.press("replace", &v, 2, |t| format!("text {:?}", t.text()));
		}

		// Fonts.
		self.fresh("Some text to restyle");
		self.run(Command::SelectAll);
		let v = dialogs::fonts(&self.env());
		if self.shown("fonts", &v) {
			let v = v.unwrap();
			v.set_text(fo::SIZE, "14");
			v.set_checked(fo::BOLD, true);
			v.set_checked(fo::ITALIC, true);
			self.press("fonts", &v, 2, |t| format!("cancelled, {}", t.char_state()));
		}
		let v = dialogs::fonts(&self.env());
		if self.shown("fonts", &v) {
			let v = v.unwrap();
			v.set_text(fo::SIZE, "14");
			v.set_checked(fo::BOLD, true);
			v.set_checked(fo::ITALIC, true);
			v.set_checked(fo::UNDERLINE, true);
			self.press("fonts", &v, fo::OK, |t| t.char_state());
		}

		// Tabs.
		self.fresh("Tab stops");
		let v = dialogs::tabs(&self.env());
		if self.shown("tabs", &v) {
			let v = v.unwrap();
			v.set_text(ta::POSITION, "1.00");
			v.set_checked(ta::LEFT, false);
			v.set_checked(ta::CENTRE, true);
			v.set_checked(ta::LEADER_NONE, false);
			v.set_checked(ta::DOTS, true);
			self.press("tabs", &v, ta::SET, |t| t.tab_stops());
			v.set_text(ta::POSITION, "2.50");
			self.press("tabs", &v, ta::SET, |t| t.tab_stops());
			self.press("tabs", &v, ta::CLEAR_ALL, |t| t.tab_stops());
			self.press("tabs", &v, 2, |t| t.tab_stops());
		}

		// Spelling: needs the dictionary beside the program or in the current folder.
		self.fresh("teh brown fox");
		self.to_start();
		let v = dialogs::spelling(&self.env());
		if self.shown("spelling", &v) {
			let v = v.unwrap();
			say(&format!(
				"dialog spelling: word {:?}, suggestions {:?}",
				v.text(sp::WORD),
				v.text(sp::SUGGESTIONS)
			));
			v.set_text(sp::CHANGE_TO, "the");
			self.press("spelling", &v, sp::CHANGE, |t| {
				format!("text {:?}", t.text())
			});
			self.close(&v);
		} else {
			say("dialog spelling: no dictionary here, so nothing was checked");
		}
	}

	fn char_state(&self) -> String {
		self.ui.doc.borrow().read(|app, s| {
			let cs = app.char_state(s);
			let face = cs.face.get(..usize::from(cs.face_len)).unwrap_or(&[]);
			format!(
				"face {:?}, size {} half-points, bold {}, italic {}, underline {}, strikeout {}",
				quill_tty::cp1252::ansi_to_string(face),
				cs.size,
				cs.flags & quill_core::doc::cflag::BOLD != 0,
				cs.flags & quill_core::doc::cflag::ITALIC != 0,
				cs.flags & quill_core::doc::cflag::UNDERLINE != 0,
				cs.flags & quill_core::doc::cflag::STRIKE != 0
			)
		})
	}

	fn tab_stops(&self) -> String {
		self.ui.doc.borrow().read(|app, s| {
			let mut out = [(0, 0, 0); 16];
			let n = app.tab_stops_full(s, &mut out);
			format!("tab stops {:?}", out.get(..n).unwrap_or(&[]))
		})
	}

	fn pictures(&self) -> u32 {
		self.ui.doc.borrow().read(|app, _| app.doc().pictures.len())
	}

	fn types_asked(&self) -> String {
		match self.script.asked().last() {
			Some(r) => format!(
				"{} types {:?}, first {}",
				r.purpose.title(),
				r.purpose
					.filters()
					.iter()
					.map(|f| f.name)
					.collect::<Vec<_>>(),
				r.default_filter
			),
			None => "no request".to_string(),
		}
	}

	fn file_dialogs(&mut self) {
		let ui = self.ui.clone();

		// Open.
		let alpha = self.saved("alpha.qld", "alpha text");
		self.saved("other.qld", "something else");
		self.script.push(Step::Pick(alpha.clone(), None));
		self.run(Command::Open);
		say(&format!(
			"dialog open: {}; effect: name {:?}, text {:?}",
			self.types_asked(),
			ui.doc.borrow().name(),
			self.text()
		));
		self.script.push(Step::Cancel);
		self.run(Command::Open);
		say(&format!(
			"dialog open button cancel: answered; effect: name {:?}, text {:?}",
			ui.doc.borrow().name(),
			self.text()
		));
		let missing = self.dir.join("missing.qld");
		self.script.push(Step::Pick(missing, None));
		self.run(Command::Open);
		say(&format!(
			"dialog open missing file: answered; effect: name {:?}, text {:?}",
			ui.doc.borrow().name(),
			self.text()
		));
		ui.window.visible_dialog().inspect(|d| d.force_close());
		pump();

		// Save As, with each type and with a bare name.
		for (label, leaf, pick, want) in [
			("type 1", "bare1", Some(0), "bare1.qld"),
			("type 2", "bare2", Some(1), "bare2.rtf"),
			("type 3", "bare3", Some(2), "bare3.txt"),
			("typed extension kept", "typed.rtf", Some(0), "typed.rtf"),
			("no type reported", "plain", None, "plain.qld"),
		] {
			self.fresh("Saved by the dialog.");
			self.script.push(Step::Pick(self.dir.join(leaf), pick));
			self.run(Command::SaveAs);
			let path = self.dir.join(want);
			say(&format!(
				"dialog save-as {label}: {}; effect: {} exists {}, {} bytes, name {:?}, modified {}",
				self.types_asked(),
				want,
				path.is_file(),
				std::fs::metadata(&path).map_or(0, |m| m.len()),
				ui.doc.borrow().name(),
				ui.doc.borrow().modified()
			));
		}
		self.fresh("Never saved");
		self.script.push(Step::Cancel);
		self.run(Command::SaveAs);
		say(&format!(
			"dialog save-as button cancel: answered; effect: name {:?}, modified {}",
			ui.doc.borrow().name(),
			ui.doc.borrow().modified()
		));
		// Save of a document with no name asks for one.
		self.script
			.push(Step::Pick(self.dir.join("named-by-save"), None));
		self.run(Command::Save);
		say(&format!(
			"dialog save-as from Save: answered; effect: named-by-save.qld exists {}, modified {}",
			self.dir.join("named-by-save.qld").is_file(),
			ui.doc.borrow().modified()
		));

		// Insert Picture.
		let bmp = self.dir.join("pixel.bmp");
		std::fs::write(&bmp, pixel_bmp()).unwrap();
		let text = self.dir.join("not-a-picture.bmp");
		std::fs::write(&text, b"not a picture at all").unwrap();
		self.fresh("Picture here: ");
		let before = self.pictures();
		self.script.push(Step::Pick(bmp, None));
		self.run(Command::Picture);
		say(&format!(
			"dialog picture: {}; effect: pictures {} -> {}",
			self.types_asked(),
			before,
			self.pictures()
		));
		let before = self.pictures();
		self.script.push(Step::Pick(text, None));
		self.run(Command::Picture);
		say(&format!(
			"dialog picture not a picture: answered; effect: pictures {} -> {}",
			before,
			self.pictures()
		));
		self.script.push(Step::Cancel);
		self.run(Command::Picture);
		say(&format!(
			"dialog picture button cancel: answered; effect: pictures {}",
			self.pictures()
		));
	}

	/// What each command that asks the question does with each answer.
	fn questions(&mut self) {
		let ui = self.ui.clone();
		let target = self.saved("target.qld", "target text");
		let recent_target = self.saved("recent.qld", "recent text");
		let _ = &recent_target;
		for answer in [Reply::Cancel, Reply::Discard, Reply::Save] {
			for label in ["New", "Open", "Recent", "drop", "hand-off"] {
				let file = self.saved("subject.qld", "subject text");
				self.dirty();
				let changed = self.text();
				match answer {
					Reply::Cancel => self.script.push(Step::Reply(Reply::Cancel)),
					Reply::Discard => self.script.push(Step::Reply(Reply::Discard)),
					Reply::Save => self.script.push(Step::Reply(Reply::Save)),
				}
				if label == "Open" && answer != Reply::Cancel {
					// After the question, Open asks for a file.
					self.script.push(Step::Pick(target.clone(), None));
				}
				match label {
					"New" => self.run(Command::New),
					"Open" => self.run(Command::Open),
					"Recent" => self.run(Command::Recent(0)),
					_ => {
						// A drop and a file handed to a running window both arrive as a path.
						ui.open_path(&target);
						pump();
					}
				}
				let on_disk = std::fs::read(&file).map(|b| b.len()).unwrap_or(0);
				say(&format!(
					"question {label} {answer:?}: effect: name {:?}, text {:?}, modified {}, file is {} bytes, was {:?}",
					ui.doc.borrow().name(),
					self.text(),
					ui.doc.borrow().modified(),
					on_disk,
					changed
				));
			}
		}
		// Discard, then Cancel in the Open dialog: nothing is lost.
		self.saved("subject.qld", "subject text");
		self.dirty();
		self.script.push(Step::Reply(Reply::Discard));
		self.script.push(Step::Cancel);
		self.run(Command::Open);
		say(&format!(
			"question Open Discard then Cancel in Open: effect: name {:?}, text {:?}, modified {}",
			ui.doc.borrow().name(),
			self.text(),
			ui.doc.borrow().modified()
		));
		// Save with no name goes to Save As first.
		self.fresh("Unnamed work");
		self.script.push(Step::Reply(Reply::Save));
		self.script
			.push(Step::Pick(self.dir.join("from-question"), None));
		self.run(Command::New);
		say(&format!(
			"question New Save with no name: effect: from-question.qld exists {}, text {:?}",
			self.dir.join("from-question.qld").is_file(),
			self.text()
		));
		// Save with no name, then Cancel in Save As: nothing goes on.
		self.fresh("Unnamed work");
		self.script.push(Step::Reply(Reply::Save));
		self.script.push(Step::Cancel);
		self.run(Command::New);
		say(&format!(
			"question New Save then Cancel in Save As: effect: text {:?}, modified {}",
			self.text(),
			ui.doc.borrow().modified()
		));
		// Closing the window.
		for answer in [Reply::Cancel, Reply::Discard, Reply::Save] {
			self.saved("closing.qld", "closing text");
			self.dirty();
			self.script.push(Step::Reply(answer));
			let before = ui.closes.get();
			ui.window.close();
			pump();
			say(&format!(
				"question close window {answer:?}: effect: closed {}, modified {}",
				ui.closes.get() > before,
				ui.doc.borrow().modified()
			));
		}
		// Exit last: the editor keeps its request to quit.
		for answer in [Reply::Cancel, Reply::Save, Reply::Discard] {
			self.saved("exiting.qld", "exiting text");
			self.dirty();
			self.script.push(Step::Reply(answer));
			let before = ui.closes.get();
			self.run(Command::Exit);
			say(&format!(
				"question Exit {answer:?}: effect: closed {}, modified {}",
				ui.closes.get() > before,
				ui.doc.borrow().modified()
			));
		}
	}
}
