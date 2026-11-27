// The file-dialog seam: how the window asks for a file name and for the answer to the unsaved-changes question.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The window never builds a `gtk::FileDialog` or an alert itself: it asks a `Chooser` for a path and an `Asker` for Save, Don't Save
// or Cancel, and goes on in a callback, as GTK's own dialogs make it. `GtkChooser` and `AlertAsker` are the real ones. A
// `ScriptChooser` answers both from a queue, so a test or `quill --script-chooser FILE` runs every flow that reaches a file dialog
// or the question with no one to click. The filters (what Open, Save As and Insert Picture list) and the completion of a typed
// name are plain data and functions here, and are tested without a display.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use adw::prelude::*;
use gtk::{gio, glib};

/// A type of file a chooser lists: the name shown and the suffixes it matches (none matches every file).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct FilterSpec {
	/// The words in the dialog's type list.
	pub name: &'static str,
	/// The suffixes, without a dot.
	pub suffixes: &'static [&'static str],
}

/// What Open lists, in the Windows build's order: Quill documents, Rich Text, Text and All files.
pub const OPEN_FILTERS: [FilterSpec; 4] = [
	FilterSpec {
		name: "Quill documents (*.qld)",
		suffixes: &["qld"],
	},
	FilterSpec {
		name: "Rich Text (*.rtf)",
		suffixes: &["rtf"],
	},
	FilterSpec {
		name: "Text (*.txt)",
		suffixes: &["txt"],
	},
	FilterSpec {
		name: "All files",
		suffixes: &[],
	},
];

/// What Save As lists: the three types `quill_app::savename` completes a name for.
pub const SAVE_FILTERS: [FilterSpec; 3] = [
	FilterSpec {
		name: "Quill documents (*.qld)",
		suffixes: &["qld"],
	},
	FilterSpec {
		name: "Rich Text (*.rtf)",
		suffixes: &["rtf"],
	},
	FilterSpec {
		name: "Text (*.txt)",
		suffixes: &["txt"],
	},
];

/// What Insert Picture lists.
pub const PICTURE_FILTERS: [FilterSpec; 3] = [
	FilterSpec {
		name: "Pictures (*.bmp, *.png)",
		suffixes: &["bmp", "png"],
	},
	FilterSpec {
		name: "Bitmaps (*.bmp)",
		suffixes: &["bmp"],
	},
	FilterSpec {
		name: "PNG pictures (*.png)",
		suffixes: &["png"],
	},
];

/// Which chooser is asked for.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Purpose {
	/// File > Open.
	Open,
	/// File > Save As.
	SaveAs,
	/// Insert > Picture.
	Picture,
}

impl Purpose {
	/// The dialog's title.
	pub fn title(self) -> &'static str {
		match self {
			Purpose::Open => "Open",
			Purpose::SaveAs => "Save As",
			Purpose::Picture => "Insert Picture",
		}
	}

	/// The word a script and the trace use for it.
	pub fn word(self) -> &'static str {
		match self {
			Purpose::Open => "open",
			Purpose::SaveAs => "save-as",
			Purpose::Picture => "picture",
		}
	}

	/// The types it lists.
	pub fn filters(self) -> &'static [FilterSpec] {
		match self {
			Purpose::Open => &OPEN_FILTERS,
			Purpose::SaveAs => &SAVE_FILTERS,
			Purpose::Picture => &PICTURE_FILTERS,
		}
	}
}

/// What a chooser is asked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
	/// Which dialog.
	pub purpose: Purpose,
	/// The folder it starts in.
	pub folder: Option<PathBuf>,
	/// The name it suggests (Save As).
	pub name: Option<String>,
	/// The type listed first, as an index into `purpose.filters()`.
	pub default_filter: usize,
}

/// A file the person chose.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Choice {
	/// Where it is, or where it will be written.
	pub path: PathBuf,
	/// The 0-based type the person chose, when the chooser can say. GTK's `FileDialog` cannot, so this is `None` there.
	pub filter: Option<usize>,
}

/// Why there is no file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failure {
	/// The person closed the dialog: not a failure to report.
	Cancelled,
	/// The dialog could not be shown or answered.
	Failed(String),
}

/// What a chooser answers.
pub type Outcome = Result<Choice, Failure>;

/// Called once with the outcome, later than the request, as GTK's dialogs call back.
pub type Answer = Box<dyn FnOnce(Outcome)>;

/// Something that asks for a file name.
pub trait Chooser {
	/// Asks as `request` says and calls `then` with the outcome.
	fn choose(&self, request: Request, then: Answer);

	/// File > Open.
	fn open(&self, folder: Option<PathBuf>, then: Answer) {
		self.choose(
			Request {
				purpose: Purpose::Open,
				folder,
				name: None,
				default_filter: 0,
			},
			then,
		);
	}

	/// File > Save As, suggesting `name` and listing `default_filter` first.
	fn save_as(&self, folder: Option<PathBuf>, name: String, default_filter: usize, then: Answer) {
		self.choose(
			Request {
				purpose: Purpose::SaveAs,
				folder,
				name: Some(name),
				default_filter,
			},
			then,
		);
	}

	/// Insert > Picture.
	fn picture(&self, folder: Option<PathBuf>, then: Answer) {
		self.choose(
			Request {
				purpose: Purpose::Picture,
				folder,
				name: None,
				default_filter: 0,
			},
			then,
		);
	}
}

/// The answer to "Save changes to NAME?".
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Reply {
	/// Save, then go on.
	Save,
	/// Go on without saving.
	Discard,
	/// Stay where things are (also Esc and closing the question).
	Cancel,
}

/// Something that asks the unsaved-changes question.
pub trait Asker {
	/// Asks about the document called `name` and calls `then` with the reply.
	fn ask_save(&self, name: &str, then: Box<dyn FnOnce(Reply)>);
}

/// A `gtk::FileFilter` for `spec`.
fn gtk_filter(spec: &FilterSpec) -> gtk::FileFilter {
	let f = gtk::FileFilter::new();
	f.set_name(Some(spec.name));
	if spec.suffixes.is_empty() {
		f.add_pattern("*");
	}
	for s in spec.suffixes {
		f.add_suffix(s);
	}
	f
}

/// The real chooser: a `gtk::FileDialog` over the window.
pub struct GtkChooser {
	window: glib::WeakRef<gtk::Window>,
}

impl GtkChooser {
	/// A chooser whose dialogs belong to `window`.
	pub fn new(window: &impl IsA<gtk::Window>) -> GtkChooser {
		GtkChooser {
			window: window.upcast_ref::<gtk::Window>().downgrade(),
		}
	}
}

impl Chooser for GtkChooser {
	fn choose(&self, request: Request, then: Answer) {
		let store = gio::ListStore::new::<gtk::FileFilter>();
		let filters = request.purpose.filters();
		let made: Vec<gtk::FileFilter> = filters.iter().map(gtk_filter).collect();
		for f in &made {
			store.append(f);
		}
		let dialog = gtk::FileDialog::builder()
			.title(request.purpose.title())
			.modal(true)
			.filters(&store)
			.build();
		if let Some(f) = made.get(request.default_filter).or(made.first()) {
			dialog.set_default_filter(Some(f));
		}
		if let Some(folder) = &request.folder {
			dialog.set_initial_folder(Some(&gio::File::for_path(folder)));
		}
		if let Some(name) = &request.name {
			dialog.set_initial_name(Some(name));
		}
		let window = self.window.upgrade();
		let done = move |res: Result<gio::File, glib::Error>| {
			then(match res {
				Ok(file) => file
					.path()
					.map(|path| Choice { path, filter: None })
					.ok_or_else(|| Failure::Failed("The file has no local path.".to_string())),
				Err(e)
					if e.matches(gtk::DialogError::Dismissed)
						|| e.matches(gtk::DialogError::Cancelled) =>
				{
					Err(Failure::Cancelled)
				}
				Err(e) => Err(Failure::Failed(e.to_string())),
			});
		};
		match request.purpose {
			Purpose::SaveAs => dialog.save(window.as_ref(), gio::Cancellable::NONE, done),
			Purpose::Open | Purpose::Picture => {
				dialog.open(window.as_ref(), gio::Cancellable::NONE, done)
			}
		}
	}
}

/// The real question: an `adw::AlertDialog` with Cancel, Don't Save and Save.
pub struct AlertAsker {
	window: glib::WeakRef<gtk::Window>,
}

impl AlertAsker {
	/// A question that belongs to `window`.
	pub fn new(window: &impl IsA<gtk::Window>) -> AlertAsker {
		AlertAsker {
			window: window.upcast_ref::<gtk::Window>().downgrade(),
		}
	}
}

impl Asker for AlertAsker {
	fn ask_save(&self, name: &str, then: Box<dyn FnOnce(Reply)>) {
		let dialog = adw::AlertDialog::new(
			Some(&format!("Save changes to {name}?")),
			Some("Your changes will be lost if you do not save them."),
		);
		dialog.add_responses(&[
			("cancel", "_Cancel"),
			("discard", "_Don\u{2019}t Save"),
			("save", "_Save"),
		]);
		dialog.set_response_appearance("discard", adw::ResponseAppearance::Destructive);
		dialog.set_response_appearance("save", adw::ResponseAppearance::Suggested);
		dialog.set_default_response(Some("save"));
		dialog.set_close_response("cancel");
		let window = self.window.upgrade();
		dialog.choose(window.as_ref(), gio::Cancellable::NONE, move |response| {
			then(match response.as_str() {
				"save" => Reply::Save,
				"discard" => Reply::Discard,
				_ => Reply::Cancel,
			});
		});
	}
}

/// One answer of a script.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
	/// A file dialog answers with this path and, when given, the 0-based type.
	Pick(PathBuf, Option<usize>),
	/// A file dialog is closed without a choice.
	Cancel,
	/// A file dialog fails with this message.
	Fail(String),
	/// The unsaved-changes question is answered.
	Reply(Reply),
}

/// Parses a script: one answer per line, `#` starts a comment.
///
/// ```text
/// open PATH             a file dialog picks PATH
/// save-as PATH [N]      the same, with the 1-based type N chosen (the dialog's list order)
/// picture PATH          the same
/// cancel                a file dialog is closed
/// fail MESSAGE          a file dialog fails
/// save | discard | stay the unsaved-changes question is answered
/// ```
pub fn parse_script(text: &str) -> Result<Vec<Step>, String> {
	let mut steps = Vec::new();
	for (n, raw) in text.lines().enumerate() {
		let line = raw.trim();
		if line.is_empty() || line.starts_with('#') {
			continue;
		}
		let (word, rest) = line.split_once(' ').unwrap_or((line, ""));
		let rest = rest.trim();
		let at = |m: &str| format!("script line {}: {m}", n + 1);
		steps.push(match word {
			"open" | "picture" if !rest.is_empty() => Step::Pick(PathBuf::from(rest), None),
			"save-as" if !rest.is_empty() => {
				// A trailing number is the type; a path that ends in a space and a number is not supported.
				match rest.rsplit_once(' ') {
					Some((path, idx)) if idx.chars().all(|c| c.is_ascii_digit()) => {
						let i: usize = idx.parse().map_err(|_| at("bad type number"))?;
						Step::Pick(PathBuf::from(path.trim()), Some(i.saturating_sub(1)))
					}
					_ => Step::Pick(PathBuf::from(rest), None),
				}
			}
			"cancel" => Step::Cancel,
			"fail" => Step::Fail(rest.to_string()),
			"save" => Step::Reply(Reply::Save),
			"discard" => Step::Reply(Reply::Discard),
			"stay" => Step::Reply(Reply::Cancel),
			_ => return Err(at(&format!("cannot read {line:?}"))),
		});
	}
	Ok(steps)
}

/// A chooser and an asker that answer from a queue.
#[derive(Default)]
pub struct ScriptChooser {
	steps: RefCell<VecDeque<Step>>,
	trace: bool,
	asked: RefCell<Vec<Request>>,
}

impl ScriptChooser {
	/// A script with these answers. With `trace` each question and answer is printed on standard output, one line each.
	pub fn new(steps: Vec<Step>, trace: bool) -> Rc<ScriptChooser> {
		Rc::new(ScriptChooser {
			steps: RefCell::new(steps.into()),
			trace,
			asked: RefCell::new(Vec::new()),
		})
	}

	/// Queues one more answer.
	pub fn push(&self, step: Step) {
		self.steps.borrow_mut().push_back(step);
	}

	/// Drops every answer that is still queued.
	pub fn clear(&self) {
		self.steps.borrow_mut().clear();
	}

	/// How many answers are left.
	pub fn left(&self) -> usize {
		self.steps.borrow().len()
	}

	/// The requests the chooser has been asked, oldest first.
	pub fn asked(&self) -> Vec<Request> {
		self.asked.borrow().clone()
	}

	fn say(&self, line: &str) {
		if self.trace {
			println!("{line}");
		}
	}
}

/// Runs `f` on the main loop's next turn, as GTK's dialogs answer later than they are asked.
fn later(f: impl FnOnce() + 'static) {
	glib::idle_add_local_once(f);
}

impl Chooser for ScriptChooser {
	fn choose(&self, request: Request, then: Answer) {
		self.asked.borrow_mut().push(request.clone());
		let word = request.purpose.word();
		let step = self.steps.borrow_mut().pop_front();
		let outcome = match step {
			Some(Step::Pick(path, filter)) => {
				self.say(&format!("chooser {word}: answered {}", path.display()));
				Ok(Choice { path, filter })
			}
			Some(Step::Fail(m)) => {
				self.say(&format!("chooser {word}: failed {m}"));
				Err(Failure::Failed(m))
			}
			Some(Step::Cancel) => {
				self.say(&format!("chooser {word}: cancelled"));
				Err(Failure::Cancelled)
			}
			Some(other) => {
				self.say(&format!(
					"chooser {word}: the script had {other:?}, which is not a file; cancelled"
				));
				Err(Failure::Cancelled)
			}
			None => {
				self.say(&format!("chooser {word}: the script is empty; cancelled"));
				Err(Failure::Cancelled)
			}
		};
		later(move || then(outcome));
	}
}

impl Asker for ScriptChooser {
	fn ask_save(&self, name: &str, then: Box<dyn FnOnce(Reply)>) {
		let reply = match self.steps.borrow_mut().pop_front() {
			Some(Step::Reply(r)) => r,
			_ => Reply::Cancel,
		};
		self.say(&format!("question {name}: answered {reply:?}"));
		later(move || then(reply));
	}
}

/// The path to save to once a type is known: the name keeps a typed extension, and a bare name takes the type's (`filter` is the
/// 0-based index into `SAVE_FILTERS`; `quill_app::savename` has the rule every build shares).
pub fn complete_path(path: &Path, filter: usize) -> PathBuf {
	let Some(file) = path.file_name().map(|f| f.to_string_lossy().into_owned()) else {
		return path.to_path_buf();
	};
	let mut buf = [0u8; 256];
	let bytes = file.as_bytes();
	if bytes.len() + 8 > buf.len() {
		return path.to_path_buf();
	}
	buf[..bytes.len()].copy_from_slice(bytes);
	let n = quill_app::savename::complete_name(&mut buf, bytes.len(), filter as u32 + 1);
	match std::str::from_utf8(&buf[..n]) {
		Ok(done) => path.with_file_name(done),
		Err(_) => path.to_path_buf(),
	}
}

/// The type Save As lists first: the one that matches the document's own extension, so a bare name keeps the document's kind.
pub fn default_save_filter(name: Option<&str>) -> usize {
	let ext = name
		.and_then(|n| Path::new(n).extension())
		.map(|e| e.to_string_lossy().to_ascii_lowercase());
	SAVE_FILTERS
		.iter()
		.position(|f| ext.as_deref().is_some_and(|e| f.suffixes.contains(&e)))
		.unwrap_or(0)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn open_lists_the_windows_types_in_order() {
		let names: Vec<&str> = OPEN_FILTERS.iter().map(|f| f.name).collect();
		assert_eq!(
			names,
			[
				"Quill documents (*.qld)",
				"Rich Text (*.rtf)",
				"Text (*.txt)",
				"All files"
			]
		);
		assert!(OPEN_FILTERS[3].suffixes.is_empty());
	}

	#[test]
	fn save_as_lists_the_three_types_savename_knows() {
		let ext: Vec<&[u8]> = SAVE_FILTERS
			.iter()
			.map(|f| f.suffixes[0].as_bytes())
			.collect();
		assert_eq!(ext, quill_app::savename::SAVE_EXTS);
	}

	#[test]
	fn a_bare_name_takes_the_chosen_types_extension_and_a_typed_one_is_kept() {
		let p = Path::new("/tmp/letter");
		assert_eq!(complete_path(p, 0), Path::new("/tmp/letter.qld"));
		assert_eq!(complete_path(p, 1), Path::new("/tmp/letter.rtf"));
		assert_eq!(complete_path(p, 2), Path::new("/tmp/letter.txt"));
		assert_eq!(
			complete_path(Path::new("/tmp/letter.doc"), 1),
			Path::new("/tmp/letter.doc")
		);
		assert_eq!(
			complete_path(Path::new("/tmp/a.b/letter"), 1),
			Path::new("/tmp/a.b/letter.rtf")
		);
	}

	#[test]
	fn the_first_save_type_follows_the_documents_extension() {
		assert_eq!(default_save_filter(None), 0);
		assert_eq!(default_save_filter(Some("Letter.QLD")), 0);
		assert_eq!(default_save_filter(Some("Letter.rtf")), 1);
		assert_eq!(default_save_filter(Some("notes.txt")), 2);
		assert_eq!(default_save_filter(Some("notes.doc")), 0);
	}

	#[test]
	fn a_script_reads_every_kind_of_line() {
		let text = "# a comment\nopen /tmp/a b.qld\nsave-as /tmp/out 2\nsave-as /tmp/out dir/x\npicture /tmp/p.bmp\ncancel\nfail no way\nsave\ndiscard\nstay\n";
		let steps = parse_script(text).unwrap();
		assert_eq!(
			steps,
			[
				Step::Pick("/tmp/a b.qld".into(), None),
				Step::Pick("/tmp/out".into(), Some(1)),
				Step::Pick("/tmp/out dir/x".into(), None),
				Step::Pick("/tmp/p.bmp".into(), None),
				Step::Cancel,
				Step::Fail("no way".into()),
				Step::Reply(Reply::Save),
				Step::Reply(Reply::Discard),
				Step::Reply(Reply::Cancel),
			]
		);
		assert!(parse_script("frobnicate").is_err());
		assert!(parse_script("open").is_err());
	}

	/// Runs the main loop's pending turns, which is where the script answers.
	fn pump() {
		let ctx = glib::MainContext::default();
		while ctx.iteration(false) {}
	}

	// The default main context belongs to one thread, and a test runs on a thread of its own, so the cases that use it run in one test.
	#[test]
	fn the_script_answers_files_and_questions() {
		a_script_answers_in_order_on_the_next_turn_and_then_cancels();
		the_question_takes_a_reply_and_stays_put_without_one();
		a_save_as_request_carries_the_name_and_the_first_type();
	}

	fn a_script_answers_in_order_on_the_next_turn_and_then_cancels() {
		let script = ScriptChooser::new(
			vec![
				Step::Pick("/tmp/one.qld".into(), None),
				Step::Cancel,
				Step::Reply(Reply::Discard),
			],
			false,
		);
		let got: Rc<RefCell<Vec<Outcome>>> = Rc::default();
		for _ in 0..4 {
			let g = got.clone();
			script.open(None, Box::new(move |o| g.borrow_mut().push(o)));
		}
		assert!(got.borrow().is_empty(), "the answers come on a later turn");
		pump();
		let got = got.borrow();
		assert_eq!(got[0].as_ref().unwrap().path, Path::new("/tmp/one.qld"));
		assert_eq!(got[1], Err(Failure::Cancelled));
		// A reply is not a file: a file dialog asked for one cancels and uses it up.
		assert_eq!(got[2], Err(Failure::Cancelled));
		assert_eq!(got[3], Err(Failure::Cancelled));
		assert_eq!(script.asked().len(), 4);
		assert_eq!(script.left(), 0);
	}

	fn the_question_takes_a_reply_and_stays_put_without_one() {
		let script = ScriptChooser::new(vec![Step::Reply(Reply::Save)], false);
		let got: Rc<RefCell<Vec<Reply>>> = Rc::default();
		for _ in 0..2 {
			let g = got.clone();
			script.ask_save("a.qld", Box::new(move |r| g.borrow_mut().push(r)));
		}
		pump();
		assert_eq!(*got.borrow(), [Reply::Save, Reply::Cancel]);
	}

	fn a_save_as_request_carries_the_name_and_the_first_type() {
		let script = ScriptChooser::new(vec![], false);
		script.save_as(Some("/tmp".into()), "x.rtf".into(), 1, Box::new(|_| {}));
		let r = &script.asked()[0];
		assert_eq!(r.purpose, Purpose::SaveAs);
		assert_eq!(r.name.as_deref(), Some("x.rtf"));
		assert_eq!(r.default_filter, 1);
		assert_eq!(r.purpose.filters().len(), 3);
	}
}
