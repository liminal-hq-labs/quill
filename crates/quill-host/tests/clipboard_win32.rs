// The Windows clipboard's contract, played by a fake: moveable blocks that end in a NUL and are rounded up in size, a clipboard another
// program may hold, an RTF that must reach it before text does. `win9x/quill-win9x/src/clip.rs` implements the same contract over the
// real calls; `win9x/tools/wine-clipboard.sh` checks that under Wine (WP-ED-6, W9X-140).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use quill_app::{App, Clipboard, Command, Flavour, Flavours, Key};
use quill_clip::ClipWrite;
use quill_formats::io::{Sink, SliceSource};
use quill_host::script::MemFiles;
use quill_host::HostStore;

const SAMPLE: &[u8] = b"{\\rtf1\\ansi\\deff0{\\fonttbl{\\f0 Times New Roman;}}\\pard\\fs24 Plain \\b bold\\b0  \\i italic\\i0  \\fs40 big\\par}";

/// The clipboard as the Windows calls present it.
#[derive(Default)]
struct FakeOs {
	/// One block per format: its bytes, then zeros up to a multiple of 16, as `GlobalSize` reports a block.
	blocks: Vec<(Flavour, Vec<u8>)>,
	/// Another program has the clipboard open: `OpenClipboard` fails every time.
	busy: bool,
	open: bool,
	rtf_ok: bool,
	/// How many times the clipboard was opened.
	opened: u32,
}

struct Put<'a> {
	flavour: Flavour,
	data: Vec<u8>,
	to: &'a mut Vec<(Flavour, Vec<u8>)>,
	rtf_ok: &'a mut bool,
}

impl Sink for Put<'_> {
	fn write(&mut self, bytes: &[u8]) -> bool {
		self.data.extend_from_slice(bytes);
		true
	}
}

impl ClipWrite for Put<'_> {
	fn commit(mut self) -> bool {
		if self.flavour != Flavour::Dib {
			self.data.push(0);
		}
		while !self.data.len().is_multiple_of(16) {
			self.data.push(0);
		}
		if self.flavour == Flavour::Rtf {
			*self.rtf_ok = true;
		}
		self.to.push((self.flavour, self.data));
		true
	}
}

impl Clipboard for FakeOs {
	type W<'a> = Put<'a>;
	type R<'a> = SliceSource<'a>;

	fn clear(&mut self) -> bool {
		if self.busy {
			return false;
		}
		self.open = true;
		self.opened += 1;
		self.blocks.clear();
		self.rtf_ok = false;
		true
	}

	fn offers(&mut self) -> Flavours {
		self.blocks
			.iter()
			.fold(Flavours::NONE, |set, (f, _)| set.with(*f))
	}

	fn put(&mut self, f: Flavour) -> Option<Put<'_>> {
		if !self.open || (f != Flavour::Rtf && !self.rtf_ok) {
			return None;
		}
		Some(Put {
			flavour: f,
			data: Vec::new(),
			to: &mut self.blocks,
			rtf_ok: &mut self.rtf_ok,
		})
	}

	fn get(&mut self, f: Flavour) -> Option<SliceSource<'_>> {
		if self.busy {
			return None;
		}
		let (_, data) = self.blocks.iter().find(|(g, _)| *g == f)?;
		// The same rule as the shell: a text block is as long as its first NUL.
		let len = if f == Flavour::Dib {
			data.len()
		} else {
			data.iter().position(|&b| b == 0)?
		};
		Some(SliceSource::new(&data[..len]))
	}
}

fn open_rtf(bytes: &[u8]) -> (App, HostStore, MemFiles) {
	let mut s = HostStore::new();
	let mut app = App::new();
	app.reset(&mut s).unwrap();
	app.serve_clipboard(true);
	let mut f = MemFiles::default();
	f.files.insert("C:\\IN.RTF".to_string(), bytes.to_vec());
	app.open(&mut s, &mut f, b"IN.RTF").unwrap();
	(app, s, f)
}

fn body(app: &App, s: &HostStore) -> Vec<u8> {
	let b = &app.doc().body;
	let mut bytes = vec![0u8; b.len() as usize];
	let n = b.read(s, 0, &mut bytes);
	bytes.truncate(n);
	bytes
}

/// Runs a Copy or Cut through the shell's request loop.
fn copy(app: &mut App, s: &mut HostStore, f: &mut MemFiles, os: &mut FakeOs, cut: bool) -> bool {
	app.command(s, f, if cut { Command::Cut } else { Command::Copy });
	assert!(app.take_clip_request().is_some());
	app.copy_to(s, os, cut)
}

fn paste(app: &mut App, s: &mut HostStore, f: &mut MemFiles, os: &mut FakeOs) -> bool {
	app.command(s, f, Command::Paste);
	assert!(app.take_clip_request().is_some());
	app.paste_from(s, os)
}

fn block(os: &FakeOs, f: Flavour) -> &[u8] {
	&os.blocks.iter().find(|(g, _)| *g == f).unwrap().1
}

#[test]
fn a_copy_puts_rtf_and_text_that_end_in_a_nul() {
	let (mut app, mut s, mut f) = open_rtf(SAMPLE);
	let mut os = FakeOs::default();
	app.command(&mut s, &mut f, Command::SelectAll);
	assert!(copy(&mut app, &mut s, &mut f, &mut os, false));
	let rtf = block(&os, Flavour::Rtf);
	assert!(rtf.starts_with(b"{\\rtf1"));
	assert!(rtf.windows(3).any(|w| w == b"\\b "), "bold survives");
	let text_block = block(&os, Flavour::Text1252);
	let end = text_block.iter().position(|&b| b == 0).unwrap();
	assert_eq!(&text_block[..end], b"Plain bold italic big");
	assert!(os.blocks.iter().all(|(g, _)| *g != Flavour::Dib));
	assert_eq!(os.opened, 1, "one open for the whole copy");
}

#[test]
fn a_paste_of_rtf_from_another_program_keeps_its_bold() {
	let (mut app, mut s, mut f) = open_rtf(b"{\\rtf1\\ansi\\par}");
	let mut os = FakeOs::default();
	let mut w = os.clear().then(|| os.put(Flavour::Rtf)).flatten().unwrap();
	w.write(SAMPLE);
	assert!(w.commit());
	assert!(paste(&mut app, &mut s, &mut f, &mut os));
	assert!(body(&app, &s).starts_with(b"Plain bold italic big"));
	// Bold is the second run: copy only "bold" back out and see the format in the RTF.
	app.key(&mut s, &mut f, Key::CtrlHome);
	for _ in 0..6 {
		app.key(&mut s, &mut f, Key::Right);
	}
	for _ in 0..4 {
		app.key(&mut s, &mut f, Key::Select(quill_app::Dir::Right));
	}
	assert!(copy(&mut app, &mut s, &mut f, &mut os, false));
	assert!(block(&os, Flavour::Rtf).windows(3).any(|w| w == b"\\b "));
}

#[test]
fn damaged_rtf_falls_back_to_text() {
	let (mut app, mut s, mut f) = open_rtf(b"{\\rtf1\\ansi\\par}");
	let mut os = FakeOs::default();
	os.clear();
	let mut w = os.put(Flavour::Rtf).unwrap();
	w.write(b"this is not rich text at all");
	assert!(w.commit());
	let mut w = os.put(Flavour::Text1252).unwrap();
	w.write(b"fallback words");
	assert!(w.commit());
	assert!(paste(&mut app, &mut s, &mut f, &mut os));
	assert!(body(&app, &s).starts_with(b"fallback words"));
}

#[test]
fn text_alone_pastes_and_trailing_padding_is_not_text() {
	let (mut app, mut s, mut f) = open_rtf(b"{\\rtf1\\ansi\\par}");
	let mut os = FakeOs::default();
	os.clear();
	os.blocks.push((
		Flavour::Text1252,
		b"line one\r\nline two\0\0\0\0\0\0\0".to_vec(),
	));
	assert!(paste(&mut app, &mut s, &mut f, &mut os));
	let got = body(&app, &s);
	assert!(got.starts_with(b"line one\rline two"), "{:?}", got);
	assert!(!got.contains(&0));
}

#[test]
fn a_lone_picture_goes_out_as_a_dib_and_a_dib_comes_back_in() {
	let bmp = std::fs::read(format!(
		"{}/../../tests/docs/pic.bmp",
		env!("CARGO_MANIFEST_DIR")
	))
	.unwrap();
	let (mut app, mut s, mut f) = open_rtf(b"{\\rtf1\\ansi\\par}");
	f.files.insert("C:\\PIC.BMP".to_string(), bmp.clone());
	app.insert_picture(&mut s, &mut f, b"PIC.BMP").unwrap();
	let mut os = FakeOs::default();
	app.key(&mut s, &mut f, Key::Home);
	app.command(&mut s, &mut f, Command::SelectAll);
	assert!(copy(&mut app, &mut s, &mut f, &mut os, false));
	let dib = block(&os, Flavour::Dib);
	assert_eq!(
		dib[..bmp.len() - 14],
		bmp[14..],
		"the DIB is the file without its header"
	);
	// Only the picture on the clipboard, as a program that copies a bitmap leaves it.
	os.blocks.retain(|(g, _)| *g == Flavour::Dib);
	let pictures = app.doc().pictures.len();
	app.key(&mut s, &mut f, Key::CtrlEnd);
	assert!(paste(&mut app, &mut s, &mut f, &mut os));
	assert_eq!(app.doc().pictures.len(), pictures + 1);
}

#[test]
fn a_paste_is_one_undo_step() {
	let (mut app, mut s, mut f) = open_rtf(SAMPLE);
	let mut os = FakeOs::default();
	let before = body(&app, &s);
	app.command(&mut s, &mut f, Command::SelectAll);
	assert!(copy(&mut app, &mut s, &mut f, &mut os, false));
	app.key(&mut s, &mut f, Key::CtrlEnd);
	assert!(paste(&mut app, &mut s, &mut f, &mut os));
	assert_ne!(body(&app, &s), before);
	app.command(&mut s, &mut f, Command::Undo);
	assert_eq!(body(&app, &s), before);
}

#[test]
fn a_clipboard_another_program_holds_changes_nothing_and_a_cut_keeps_the_text() {
	let (mut app, mut s, mut f) = open_rtf(SAMPLE);
	let mut os = FakeOs {
		busy: true,
		..FakeOs::default()
	};
	let before = body(&app, &s);
	app.command(&mut s, &mut f, Command::SelectAll);
	assert!(!copy(&mut app, &mut s, &mut f, &mut os, true));
	assert_eq!(
		body(&app, &s),
		before,
		"a Cut that kept nothing deletes nothing"
	);
	assert!(!paste(&mut app, &mut s, &mut f, &mut os));
	assert_eq!(body(&app, &s), before);
}

#[test]
fn text_is_refused_when_the_rtf_did_not_reach_the_clipboard() {
	let mut os = FakeOs::default();
	os.clear();
	assert!(os.put(Flavour::Text1252).is_none());
	assert!(os.put(Flavour::Dib).is_none());
	assert!(os.put(Flavour::Rtf).is_some());
}

#[test]
fn what_the_menu_offers_follows_the_clipboard() {
	let (mut app, mut s, mut f) = open_rtf(SAMPLE);
	let mut os = FakeOs::default();
	app.set_clip_offers(os.offers());
	assert!(app.menu_mark(&s, Command::Paste).disabled);
	app.command(&mut s, &mut f, Command::SelectAll);
	assert!(copy(&mut app, &mut s, &mut f, &mut os, false));
	app.set_clip_offers(os.offers());
	assert!(!app.menu_mark(&s, Command::Paste).disabled);
}
