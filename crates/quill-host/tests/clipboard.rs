// The clipboard in the editor: copy and paste keep the formatting, the shell's request and the app's own clipboard agree, and a
// damaged clipboard never panics or leaks (WP-ED-5, CORE-101).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use proptest::prelude::*;
use quill_app::{App, ClipRequest, Clipboard, Command, Flavour, Flavours, Key};
use quill_clip::{ClipWrite, StoreClipboard};
use quill_formats::io::Sink;
use quill_host::script::MemFiles;
use quill_host::HostStore;

const TOO_LARGE: &[u8] = b"The document is too large for Quill.";

/// A document that uses every format a clipboard carries: fonts, sizes, colours, bold, italic, underline, alignment and lists.
const RICH: &[u8] = b"{\\rtf1\\ansi\\ansicpg1252\\deff0{\\fonttbl{\\f0\\froman Times New Roman;}{\\f1\\fswiss Arial;}}\
{\\colortbl;\\red200\\green0\\blue0;\\red0\\green0\\blue200;}\
\\paperw12240\\paperh15840\\margl1800\\margr1800\\margt1440\\margb1440\r\n\
\\pard\\qc\\f0\\fs24 Plain \\b bold\\b0  \\i italic\\i0  \\ul under\\ulnone  \\f1\\fs40 big\\f0\\fs24  \\cf1 red\\cf2 blue\\cf0 .\\par\r\n\
\\pard\\qr\\fs20 Right\\par\r\n\
\\pard{\\*\\pn\\pnlvlblt\\pnindent360{\\pntxtb\\'95}}\\li360\\fi-360\\fs24 Quills\\par\r\n\
\\pard{\\*\\pn\\pnlvlbody\\pndec\\pnstart1\\pnindent360{\\pntxta .}}\\li360\\fi-360 First step\\par\r\n\
\\pard\\qj\\sa120 Justified \\b\\i both\\par\r\n}";

fn setup() -> (App, HostStore, MemFiles) {
	let mut s = HostStore::new();
	let mut app = App::new();
	app.reset(&mut s).unwrap();
	(app, s, MemFiles::default())
}

fn open_rtf(bytes: &[u8]) -> (App, HostStore, MemFiles) {
	let (mut app, mut s, mut f) = setup();
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

/// What a character looks like, resolved through the tables so documents with different table orders compare equal: the face, size,
/// colour, vertical position and flags, and for a paragraph mark the paragraph's format.
type Look = (
	Vec<u8>,
	u16,
	(u8, u8, u8),
	u8,
	u16,
	Option<(u8, u8, u16, u8, i32, i32, i32, i32, i32)>,
);

fn looks(app: &App, s: &HostStore, from: u32, to: u32) -> Vec<Look> {
	let d = app.doc();
	let b = &d.body;
	(from..to)
		.map(|i| {
			let cf = d.cfmts.get(s, b.cfmt_at(s, i).unwrap()).unwrap();
			let font = d.fonts.get(s, cf.font).unwrap();
			let colour = if cf.colour == 0 {
				(0, 0, 0)
			} else {
				let c = d.colours.get(s, u16::from(cf.colour)).unwrap();
				(c.r, c.g, c.b)
			};
			let para = (b.read_byte(s, i) == Some(0x0D)).then(|| {
				let p = d.pfmts.get(s, b.pfmt_at(s, i).unwrap()).unwrap();
				(
					p.align, p.spacing, p.flags, p.level, p.left, p.right, p.first, p.before,
					p.after,
				)
			});
			(
				font.name[..usize::from(font.name_len)].to_vec(),
				cf.size,
				colour,
				cf.vpos,
				cf.flags,
				para,
			)
		})
		.collect()
}

/// Selects the whole body, copies it, and puts the caret at the end.
fn copy_all(app: &mut App, s: &mut HostStore, f: &mut MemFiles) {
	app.command(s, f, Command::SelectAll);
	app.command(s, f, Command::Copy);
	app.key(s, f, Key::CtrlEnd);
}

#[test]
fn copy_and_paste_keep_every_format_inside_the_app() {
	let (mut app, mut s, mut f) = open_rtf(RICH);
	let n = app.doc().body.len();
	let (text, before) = (body(&app, &s), looks(&app, &s, 0, n));
	copy_all(&mut app, &mut s, &mut f);
	assert!(app.message().is_empty(), "{:?}", app.message());
	app.command(&mut s, &mut f, Command::Paste);
	let all = body(&app, &s);
	// The selection stops before the last mark, so the second copy starts after the first copy's text and runs on into the old mark.
	assert_eq!(all.len() as u32, n + n - 1);
	let m = n - 1;
	assert_eq!(&all[..m as usize], &text[..m as usize]);
	assert_eq!(&all[m as usize..(2 * m) as usize], &text[..m as usize]);
	let after = looks(&app, &s, 0, 2 * m);
	// The first paragraph mark of the copy ends the paragraph the paste started in, so it keeps that paragraph's look.
	let first_mark = text.iter().position(|&b| b == 0x0D).unwrap();
	for i in 0..m as usize {
		assert_eq!(after[i], before[i], "the original at {i}");
		if i == first_mark {
			assert_eq!(after[m as usize + i].5, before[n as usize - 1].5);
			assert_eq!(after[m as usize + i].0, before[i].0);
		} else {
			assert_eq!(after[m as usize + i], before[i], "the copy at {i}");
		}
	}
}

#[test]
fn a_paste_is_one_undo_step_and_redoes() {
	let (mut app, mut s, mut f) = open_rtf(RICH);
	copy_all(&mut app, &mut s, &mut f);
	let before = (body(&app, &s), looks(&app, &s, 0, app.doc().body.len()));
	app.command(&mut s, &mut f, Command::Paste);
	let pasted = body(&app, &s);
	assert_ne!(pasted, before.0);
	app.command(&mut s, &mut f, Command::Undo);
	assert_eq!(
		(body(&app, &s), looks(&app, &s, 0, app.doc().body.len())),
		before
	);
	app.command(&mut s, &mut f, Command::Redo);
	assert_eq!(body(&app, &s), pasted);
}

#[test]
fn plain_text_pastes_in_the_format_at_the_caret() {
	let (mut app, mut s, mut f) = open_rtf(RICH);
	let mut cb = StoreClipboard::new(HostStore::new());
	let mut w = cb.put(Flavour::Text1252).unwrap();
	assert!(w.write(b"one\r\ntwo"));
	assert!(w.commit());
	// The caret is after "Plain " in the first paragraph; bold the word before pasting.
	for _ in 0..8 {
		app.key(&mut s, &mut f, Key::Right);
	}
	let here = looks(&app, &s, 7, 8)[0].clone();
	assert!(app.paste_from(&mut s, &mut cb));
	let got = looks(&app, &s, 8, 8 + 7);
	assert_eq!(&body(&app, &s)[8..15], b"one\rtwo");
	assert_eq!(got[0].0, here.0);
	assert_eq!(got[0].1, here.1);
	assert_eq!(got[0].4, here.4);
}

#[test]
fn a_shell_that_serves_the_clipboard_gets_requests_and_the_app_keeps_nothing() {
	let (mut app, mut s, mut f) = open_rtf(RICH);
	app.serve_clipboard(true);
	app.set_clip_offers(Flavours::NONE);
	assert!(
		app.menu_mark(&s, Command::Paste).disabled,
		"nothing on the clipboard"
	);
	app.command(&mut s, &mut f, Command::SelectAll);
	app.command(&mut s, &mut f, Command::Copy);
	assert_eq!(
		app.take_clip_request(),
		Some(ClipRequest::Copy { cut: false })
	);
	assert_eq!(app.take_clip_request(), None);
	let mut cb = StoreClipboard::new(HostStore::new());
	assert!(app.copy_to(&mut s, &mut cb, false));
	assert!(cb.len(Flavour::Rtf) > 0 && cb.len(Flavour::Text1252) > 0);
	app.set_clip_offers(Flavours::TEXT_AND_RTF);
	assert!(!app.menu_mark(&s, Command::Paste).disabled);
	app.key(&mut s, &mut f, Key::CtrlEnd);
	app.command(&mut s, &mut f, Command::Paste);
	assert_eq!(app.take_clip_request(), Some(ClipRequest::Paste));
	let n = app.doc().body.len();
	assert!(app.paste_from(&mut s, &mut cb));
	assert_eq!(app.doc().body.len(), n + n - 1);
	// The app's own clipboard was never used.
	app.serve_clipboard(false);
	assert!(app.menu_mark(&s, Command::Paste).disabled);
}

#[test]
fn a_cut_through_a_shell_deletes_only_when_the_copy_succeeded() {
	struct Refuses;
	struct Never;
	impl Sink for Never {
		fn write(&mut self, _: &[u8]) -> bool {
			false
		}
	}
	impl ClipWrite for Never {
		fn commit(self) -> bool {
			false
		}
	}
	impl Clipboard for Refuses {
		type W<'a> = Never;
		type R<'a> = quill_formats::io::SliceSource<'a>;
		fn clear(&mut self) -> bool {
			false
		}
		fn offers(&mut self) -> Flavours {
			Flavours::NONE
		}
		fn put(&mut self, _: Flavour) -> Option<Never> {
			None
		}
		fn get(&mut self, _: Flavour) -> Option<Self::R<'_>> {
			None
		}
	}
	let (mut app, mut s, mut f) = open_rtf(RICH);
	app.command(&mut s, &mut f, Command::SelectAll);
	let before = body(&app, &s);
	assert!(!app.copy_to(&mut s, &mut Refuses, true));
	assert_eq!(body(&app, &s), before);
	assert_eq!(app.message(), TOO_LARGE);
}

#[test]
fn a_paste_with_nothing_on_the_clipboard_changes_nothing() {
	let (mut app, mut s, mut f) = open_rtf(RICH);
	let before = body(&app, &s);
	app.command(&mut s, &mut f, Command::Paste);
	assert_eq!(body(&app, &s), before);
	assert!(app.menu_mark(&s, Command::Paste).disabled);
	app.command(&mut s, &mut f, Command::SelectAll);
	app.command(&mut s, &mut f, Command::Copy);
	assert!(!app.menu_mark(&s, Command::Paste).disabled);
}

/// A document of `n` bytes of one long paragraph, as RTF.
fn big_rtf(n: usize) -> Vec<u8> {
	let mut v = b"{\\rtf1\\ansi\\deff0{\\fonttbl{\\f0 Times New Roman;}}\\pard\\f0\\fs24 ".to_vec();
	v.extend((0..n).map(|i| b'a' + (i % 26) as u8));
	v.extend(b"\\par}");
	v
}

#[test]
fn a_selection_of_hundreds_of_kilobytes_copies_and_pastes_whole() {
	let (mut app, mut s, mut f) = open_rtf(&big_rtf(400_000));
	copy_all(&mut app, &mut s, &mut f);
	assert!(app.message().is_empty(), "{:?}", app.message());
	let n = app.doc().body.len();
	app.command(&mut s, &mut f, Command::Paste);
	assert_eq!(app.doc().body.len(), n + n - 1);
	app.command(&mut s, &mut f, Command::Undo);
	assert_eq!(app.doc().body.len(), n);
}

#[test]
fn a_one_megabyte_paste_arrives_whole() {
	// The clipboard holds one megabyte of text: paste it into an empty document.
	let (mut app, mut s, mut f) = setup();
	let text: Vec<u8> = (0..1_000_000).map(|i| b'a' + (i % 26) as u8).collect();
	let mut cb = StoreClipboard::new(HostStore::new());
	let mut w = cb.put(Flavour::Text1252).unwrap();
	assert!(w.write(&text));
	assert!(w.commit());
	assert!(app.paste_from(&mut s, &mut cb));
	assert!(app.message().is_empty(), "{:?}", app.message());
	assert_eq!(app.doc().body.len(), 1_000_001);
	assert_eq!(&body(&app, &s)[..1_000_000], &text[..]);
	app.command(&mut s, &mut f, Command::Undo);
	assert_eq!(body(&app, &s), b"\r");
}

#[test]
fn a_selection_whose_rtf_does_not_fit_is_refused_and_a_cut_keeps_the_text() {
	let (mut app, mut s, mut f) = open_rtf(&big_rtf(1_048_500));
	app.command(&mut s, &mut f, Command::SelectAll);
	let before = body(&app, &s);
	app.command(&mut s, &mut f, Command::Cut);
	assert_eq!(body(&app, &s), before);
	assert_eq!(app.message(), TOO_LARGE);
	assert!(app.menu_mark(&s, Command::Paste).disabled);
}

#[test]
fn a_picture_travels_with_its_copy() {
	let bmp = std::fs::read(format!(
		"{}/../../tests/docs/pic.bmp",
		env!("CARGO_MANIFEST_DIR")
	))
	.unwrap();
	let (mut app, mut s, mut f) = setup();
	f.files.insert("C:\\PIC.BMP".to_string(), bmp);
	app.insert_picture(&mut s, &mut f, b"PIC.BMP").unwrap();
	let pictures = app.doc().pictures.len();
	app.key(&mut s, &mut f, Key::Home);
	app.command(&mut s, &mut f, Command::SelectAll);
	app.command(&mut s, &mut f, Command::Copy);
	app.key(&mut s, &mut f, Key::CtrlEnd);
	let n = app.doc().body.len();
	app.command(&mut s, &mut f, Command::Paste);
	assert_eq!(app.doc().body.len(), n + 1);
	assert_eq!(app.doc().pictures.len(), pictures + 1);
}

#[test]
fn text_outside_windows_1252_never_reaches_a_document() {
	// RTF with characters 1252 lacks (a Greek letter, an emoji's surrogates) and a euro sign that it has.
	let rtf = b"{\\rtf1\\ansi\\deff0{\\fonttbl{\\f0 Times New Roman;}}\\pard a\\u960?b\\u-10179?\\u-8704?c\\u8364?d\\par}";
	let (mut app, mut s, _f) = setup();
	let mut cb = StoreClipboard::new(HostStore::new());
	let mut w = cb.put(Flavour::Rtf).unwrap();
	assert!(w.write(rtf));
	assert!(w.commit());
	assert!(app.paste_from(&mut s, &mut cb));
	let text = body(&app, &s);
	assert!(text.iter().all(|&b| b == 0x0D || b >= 0x20), "{text:?}");
	assert!(text.starts_with(b"a"));
	assert!(
		text.contains(&0x80),
		"the euro sign is 0x80 in Windows-1252"
	);
}

proptest! {
	#[test]
	fn damaged_rtf_on_the_clipboard_never_panics_or_leaks(
		mut bytes in proptest::collection::vec(any::<u8>(), 0..400),
		cut in any::<usize>(),
		plain in any::<bool>(),
	) {
		let mut good = RICH.to_vec();
		let at = cut % good.len();
		good.truncate(at);
		good.append(&mut bytes);
		let (mut app, mut s, mut f) = setup();
		let mut cb = StoreClipboard::new(HostStore::new());
		let mut w = cb.put(Flavour::Rtf).unwrap();
		prop_assert!(w.write(&good));
		prop_assert!(w.commit());
		if plain {
			let mut t = cb.put(Flavour::Text1252).unwrap();
			prop_assert!(t.write(b"fallback"));
			prop_assert!(t.commit());
		}
		app.reset(&mut s).unwrap();
		let base = s.live_chunks();
		let before = body(&app, &s);
		let ok = app.paste_from(&mut s, &mut cb);
		if !ok {
			prop_assert_eq!(body(&app, &s), before.clone());
		}
		app.command(&mut s, &mut f, Command::Undo);
		prop_assert_eq!(body(&app, &s), before.clone());
		// Nothing of the fragment stays in the store: only what the undone paste's history holds.
		app.reset(&mut s).unwrap();
		prop_assert_eq!(s.live_chunks(), base);
	}
}

#[test]
fn pasting_at_a_paragraph_start_keeps_the_copied_paragraph_formats() {
	let (mut app, mut s, mut f) = open_rtf(RICH);
	let n = app.doc().body.len();
	let before = looks(&app, &s, 0, n);
	app.command(&mut s, &mut f, Command::SelectAll);
	app.command(&mut s, &mut f, Command::Copy);
	app.key(&mut s, &mut f, Key::CtrlHome);
	app.command(&mut s, &mut f, Command::Paste);
	let after = looks(&app, &s, 0, 2 * n - 1);
	assert_eq!(&after[..(n - 1) as usize], &before[..(n - 1) as usize]);
	assert_eq!(&after[(n - 1) as usize..], &before[..]);
}
