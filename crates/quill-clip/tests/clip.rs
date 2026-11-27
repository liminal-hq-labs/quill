// Clipboard tests: copy then paste keeps the text and formats, damaged data never panics, limits hold, and the RTF is stable.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::PathBuf;

use proptest::prelude::*;
use quill_clip::{ClipError, ClipWrite, Clipboard, Flavour, Flavours, StoreClipboard};
use quill_core::doc::{CFmt, Colour, Doc, Font, PFmt};
use quill_formats::io::{Sink, Source};
use quill_heap::HeapStore;

fn root() -> PathBuf {
	PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A run of text in one of the character styles, and the paragraph format its paragraph takes.
type Run = (String, u8);
type Para = (Vec<Run>, u8);

const STYLES: usize = 6;
const PARAS: usize = 4;

/// Builds a document from paragraphs of styled runs.
fn build(s: &mut HeapStore, paras: &[Para]) -> Doc {
	let mut d = Doc::new();
	d.init(s).unwrap();
	d.fonts.intern(s, Font::new(b"Arial", 2)).unwrap();
	d.colours.intern(s, Colour { r: 200, g: 0, b: 0 }).unwrap();
	d.colours.intern(s, Colour { r: 0, g: 0, b: 200 }).unwrap();
	let styles: [CFmt; STYLES] = [
		CFmt {
			font: 0,
			size: 24,
			colour: 0,
			vpos: 0,
			flags: 0,
		},
		CFmt {
			font: 0,
			size: 24,
			colour: 0,
			vpos: 0,
			flags: 1,
		},
		CFmt {
			font: 0,
			size: 24,
			colour: 0,
			vpos: 0,
			flags: 2 | 4,
		},
		CFmt {
			font: 1,
			size: 20,
			colour: 1,
			vpos: 0,
			flags: 0,
		},
		CFmt {
			font: 1,
			size: 32,
			colour: 2,
			vpos: 1,
			flags: 1 | 8,
		},
		CFmt {
			font: 0,
			size: 28,
			colour: 0,
			vpos: 2,
			flags: 0,
		},
	];
	let pfmts: [PFmt; PARAS] = [
		PFmt::DEFAULT,
		PFmt {
			align: 1,
			..PFmt::DEFAULT
		},
		PFmt {
			align: 2,
			left: 720,
			..PFmt::DEFAULT
		},
		PFmt {
			align: 3,
			left: 360,
			first: -360,
			..PFmt::DEFAULT
		},
	];
	let cf: Vec<u16> = styles
		.iter()
		.map(|c| d.cfmts.intern(s, *c).unwrap())
		.collect();
	let pf: Vec<u16> = pfmts
		.iter()
		.map(|p| d.pfmts.intern(s, *p).unwrap())
		.collect();
	d.body.clear(s);
	for (runs, p) in paras {
		let pi = pf[usize::from(*p) % PARAS];
		for (text, c) in runs {
			if !text.is_empty() {
				d.body
					.append_original(s, text.as_bytes(), cf[usize::from(*c) % STYLES], pi)
					.unwrap();
			}
		}
		d.body.append_original(s, b"\r", cf[0], pi).unwrap();
	}
	d
}

/// What a reader sees of one character, whatever the table indices are.
#[derive(Debug, PartialEq, Eq)]
struct Look {
	ch: u8,
	font: Vec<u8>,
	size: u16,
	colour: Option<(u8, u8, u8)>,
	vpos: u8,
	flags: u16,
}

/// What a reader sees of one paragraph mark.
#[derive(Debug, PartialEq, Eq)]
struct ParaLook {
	align: u8,
	left: i32,
	right: i32,
	first: i32,
}

fn looks(s: &HeapStore, d: &Doc, from: u32, to: u32) -> (Vec<Look>, Vec<ParaLook>) {
	let mut chars = Vec::new();
	let mut paras = Vec::new();
	for pos in from..to {
		let mut b = [0u8];
		d.body.read(s, pos, &mut b);
		if b[0] == b'\r' {
			let p = d.pfmts.get(s, d.body.pfmt_at(s, pos).unwrap()).unwrap();
			paras.push(ParaLook {
				align: p.align,
				left: p.left,
				right: p.right,
				first: p.first,
			});
			chars.push(Look {
				ch: b'\r',
				font: vec![],
				size: 0,
				colour: None,
				vpos: 0,
				flags: 0,
			});
			continue;
		}
		let c = d.cfmts.get(s, d.body.cfmt_at(s, pos).unwrap()).unwrap();
		let font = d.fonts.get(s, c.font).unwrap();
		let colour = (c.colour != 0).then(|| {
			let k = d.colours.get(s, u16::from(c.colour)).unwrap();
			(k.r, k.g, k.b)
		});
		chars.push(Look {
			ch: b[0],
			font: font.name().to_vec(),
			size: c.size,
			colour,
			vpos: c.vpos,
			flags: c.flags,
		});
	}
	(chars, paras)
}

fn bytes_of(cb: &mut StoreClipboard<HeapStore>, f: Flavour) -> Vec<u8> {
	let mut r = cb.get(f).expect("flavour offered");
	let mut out = Vec::new();
	let mut buf = [0u8; 100];
	loop {
		let n = r.read(&mut buf);
		out.extend_from_slice(&buf[..n]);
		if n < buf.len() {
			return out;
		}
	}
}

fn put_bytes(cb: &mut StoreClipboard<HeapStore>, f: Flavour, bytes: &[u8]) {
	let mut w = cb.put(f).unwrap();
	assert!(w.write(bytes));
	assert!(w.commit());
}

fn para_strategy() -> impl Strategy<Value = Vec<Para>> {
	let run = ("[a-zA-Z0-9 .,]{0,12}", 0u8..STYLES as u8);
	let para = (proptest::collection::vec(run, 0..4), 0u8..PARAS as u8);
	proptest::collection::vec(para, 1..5)
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(300))]

	#[test]
	fn copy_then_paste_keeps_text_and_formats(paras in para_strategy(), a in any::<u32>(), b in any::<u32>()) {
		let mut s = HeapStore::new();
		let d = build(&mut s, &paras);
		let len = d.body.len();
		let (lo, hi) = (a % len, b % len);
		let (from, to) = (lo.min(hi), lo.max(hi) + 1);
		let mut cb = StoreClipboard::new(HeapStore::new());
		let before = s.live_chunks();
		quill_clip::copy(&d, &mut s, from..to, Flavours::TEXT_AND_RTF, &mut cb).unwrap();
		prop_assert_eq!(s.live_chunks(), before, "the scratch document is freed");
		prop_assert!(cb.offers().has(Flavour::Rtf));
		let mut frag = Doc::new();
		let got = quill_clip::paste_fragment(&mut cb, &[Flavour::Rtf, Flavour::Text1252], &mut s, &mut frag).unwrap();
		prop_assert_eq!(got, Flavour::Rtf);
		let (want_chars, want_paras) = looks(&s, &d, from, to);
		let (got_chars, got_paras) = looks(&s, &frag, 0, frag.body.len());
		prop_assert_eq!(&want_chars, &got_chars);
		prop_assert_eq!(&want_paras, &got_paras);
		frag.reset(&mut s);
		// The plain text flavour carries the same characters, with CR LF between paragraphs.
		let got = quill_clip::paste_fragment(&mut cb, &[Flavour::Text1252], &mut s, &mut frag);
		let plain: Vec<u8> = want_chars.iter().map(|l| l.ch).collect();
		if plain.iter().all(|c| *c == b'\r') && plain.is_empty() {
			prop_assert!(got.is_err());
		} else if plain.iter().any(|c| *c != b'\r') {
			prop_assert_eq!(got.unwrap(), Flavour::Text1252);
			let mut t = vec![0u8; frag.body.len() as usize];
			frag.body.read(&s, 0, &mut t);
			prop_assert_eq!(t, plain);
		}
		frag.reset(&mut s);
		let mut d = d;
		d.reset(&mut s);
		prop_assert_eq!(s.live_chunks(), 0);
	}

	#[test]
	fn damaged_rtf_never_panics_or_leaks(
		paras in para_strategy(),
		flips in proptest::collection::vec((any::<u16>(), any::<u8>()), 1..10),
		cut in any::<u16>(),
	) {
		let mut s = HeapStore::new();
		let d = build(&mut s, &paras);
		let mut cb = StoreClipboard::new(HeapStore::new());
		quill_clip::copy(&d, &mut s, 0..d.body.len(), Flavours::TEXT_AND_RTF, &mut cb).unwrap();
		let mut bytes = bytes_of(&mut cb, Flavour::Rtf);
		for (i, v) in flips {
			let n = bytes.len();
			bytes[usize::from(i) % n] = v;
		}
		let keep = usize::from(cut) % (bytes.len() + 1);
		bytes.truncate(keep);
		cb.clear();
		put_bytes(&mut cb, Flavour::Rtf, &bytes);
		let mut fs = HeapStore::new();
		let mut frag = Doc::new();
		let _ = quill_clip::paste_fragment(&mut cb, &[Flavour::Rtf, Flavour::Text1252], &mut fs, &mut frag);
		frag.reset(&mut fs);
		prop_assert_eq!(fs.live_chunks(), 0);
	}
}

#[test]
fn a_selection_inside_a_paragraph_pastes_inline_and_a_whole_paragraph_keeps_its_end() {
	let mut s = HeapStore::new();
	let d = build(&mut s, &[(vec![("hello world".into(), 0)], 0)]);
	let mut cb = StoreClipboard::new(HeapStore::new());
	let mut frag = Doc::new();
	quill_clip::copy(&d, &mut s, 0..5, Flavours::TEXT_AND_RTF, &mut cb).unwrap();
	for f in [Flavour::Rtf, Flavour::Text1252] {
		quill_clip::paste_fragment(&mut cb, &[f], &mut s, &mut frag).unwrap();
		let mut t = vec![0u8; frag.body.len() as usize];
		frag.body.read(&s, 0, &mut t);
		assert_eq!(t, b"hello", "{f:?}");
	}
	quill_clip::copy(&d, &mut s, 0..12, Flavours::TEXT_AND_RTF, &mut cb).unwrap();
	for f in [Flavour::Rtf, Flavour::Text1252] {
		quill_clip::paste_fragment(&mut cb, &[f], &mut s, &mut frag).unwrap();
		let mut t = vec![0u8; frag.body.len() as usize];
		frag.body.read(&s, 0, &mut t);
		assert_eq!(t, b"hello world\r", "{f:?}");
	}
}

#[test]
#[allow(clippy::reversed_empty_ranges)] // a reversed range is the case under test
fn bad_ranges_and_empty_clipboards_say_so() {
	let mut s = HeapStore::new();
	let d = build(&mut s, &[(vec![("abc".into(), 0)], 0)]);
	let mut cb = StoreClipboard::new(HeapStore::new());
	for r in [0..0, 3..2, 0..99] {
		assert_eq!(
			quill_clip::copy(&d, &mut s, r, Flavours::ALL, &mut cb),
			Err(ClipError::Empty)
		);
	}
	let mut frag = Doc::new();
	assert_eq!(
		quill_clip::paste_fragment(
			&mut cb,
			&[Flavour::Rtf, Flavour::Text1252],
			&mut s,
			&mut frag
		),
		Err(ClipError::Empty)
	);
	// Not RTF at all, offered as RTF: the next preference is tried.
	put_bytes(&mut cb, Flavour::Rtf, b"not rich text");
	put_bytes(&mut cb, Flavour::Text1252, b"plain\r\n");
	assert_eq!(
		quill_clip::paste_fragment(
			&mut cb,
			&[Flavour::Rtf, Flavour::Text1252],
			&mut s,
			&mut frag
		),
		Ok(Flavour::Text1252)
	);
	let mut t = vec![0u8; frag.body.len() as usize];
	frag.body.read(&s, 0, &mut t);
	assert_eq!(t, b"plain\r");
}

#[test]
fn a_flavour_over_its_limit_is_dropped_and_nothing_leaks() {
	let mut s = HeapStore::new();
	let mut d = Doc::new();
	d.init(&mut s).unwrap();
	let n = quill_clip::MAX_TEXT as usize - 4;
	let mut body = vec![b'a'; n];
	body.push(b'\r');
	d.body.load(&mut s, &body, 0, 0).unwrap();
	let len = d.body.len();
	let baseline = s.live_chunks();
	let mut cb = StoreClipboard::new(HeapStore::new());
	// The text fits and its RTF does not, so the copy succeeds with text only.
	quill_clip::copy(&d, &mut s, 0..len, Flavours::TEXT_AND_RTF, &mut cb).unwrap();
	assert!(cb.offers().has(Flavour::Text1252));
	assert!(!cb.offers().has(Flavour::Rtf));
	assert_eq!(cb.len(Flavour::Rtf), 0);
	assert_eq!(s.live_chunks(), baseline);
	// Asking for RTF alone fails outright and leaves the clipboard empty.
	assert_eq!(
		quill_clip::copy(
			&d,
			&mut s,
			0..len,
			Flavours::NONE.with(Flavour::Rtf),
			&mut cb
		),
		Err(ClipError::Refused)
	);
	assert!(cb.is_empty());
	assert_eq!(cb.store().live_chunks(), 0);
	assert_eq!(s.live_chunks(), baseline);
	// A clipboard store out of memory fails the same way.
	let mut tiny = StoreClipboard::new(HeapStore::with_limit(1));
	assert!(quill_clip::copy(&d, &mut s, 0..len, Flavours::TEXT_AND_RTF, &mut tiny).is_err());
	assert!(tiny.is_empty());
	assert_eq!(tiny.store().live_chunks(), 0);
	assert_eq!(s.live_chunks(), baseline);
}

#[test]
fn a_writer_dropped_without_a_commit_leaves_nothing() {
	let mut cb = StoreClipboard::new(HeapStore::new());
	{
		let mut w = cb.put(Flavour::Text1252).unwrap();
		assert!(w.write(b"abandoned"));
	}
	assert!(cb.is_empty());
	assert_eq!(cb.store().live_chunks(), 0);
}

#[test]
fn a_new_copy_replaces_every_old_flavour() {
	let mut s = HeapStore::new();
	let d = build(&mut s, &[(vec![("abc".into(), 0)], 0)]);
	let mut cb = StoreClipboard::new(HeapStore::new());
	quill_clip::copy(&d, &mut s, 0..3, Flavours::TEXT_AND_RTF, &mut cb).unwrap();
	quill_clip::copy(
		&d,
		&mut s,
		0..3,
		Flavours::NONE.with(Flavour::Text1252),
		&mut cb,
	)
	.unwrap();
	assert_eq!(cb.offers(), Flavours::NONE.with(Flavour::Text1252));
}

#[test]
fn a_bold_italic_underlined_selection_writes_the_golden_rtf() {
	let mut s = HeapStore::new();
	let d = build(
		&mut s,
		&[(
			vec![
				("Plain ".into(), 0),
				("bold ".into(), 1),
				("italic and underlined ".into(), 2),
				("plain again".into(), 0),
			],
			0,
		)],
	);
	let mut cb = StoreClipboard::new(HeapStore::new());
	// From "bold" through "underlined ".
	quill_clip::copy(&d, &mut s, 6..33, Flavours::TEXT_AND_RTF, &mut cb).unwrap();
	let rtf = String::from_utf8(bytes_of(&mut cb, Flavour::Rtf)).unwrap();
	let file = root().join("tests/golden/files/clip-selection.rtf");
	if std::env::var_os("QUILL_UPDATE_GOLDEN").is_some() {
		std::fs::write(&file, &rtf).unwrap();
		return;
	}
	let want = std::fs::read_to_string(&file)
		.unwrap_or_else(|_| panic!("missing golden; run with QUILL_UPDATE_GOLDEN=1"));
	assert_eq!(
		want, rtf,
		"QUILL_UPDATE_GOLDEN=1 rewrites it and the diff is reviewed like code"
	);
	assert!(rtf.contains("\\b ") || rtf.contains("\\b\\"), "{rtf}");
	assert!(rtf.contains("\\i") && rtf.contains("\\ul"));
	assert_eq!(
		bytes_of(&mut cb, Flavour::Text1252),
		b"bold italic and underlined "
	);
}

#[cfg(feature = "dib")]
mod dib {
	use super::*;

	fn picture_doc(s: &mut HeapStore, bmp: &[u8]) -> Doc {
		let mut d = Doc::new();
		d.init(s).unwrap();
		let mut at = 0;
		let idx = d
			.add_picture(s, 0, 4, 4, 9000, |out| {
				let n = out.len().min(bmp.len() - at);
				out[..n].copy_from_slice(&bmp[at..at + n]);
				at += n;
				n
			})
			.unwrap();
		d.body.clear(s);
		d.body.append_original(s, b"x", 0, 0).unwrap();
		d.body.append_object(s, u32::from(idx), 0, 0).unwrap();
		d.body.append_original(s, b"y\r", 0, 0).unwrap();
		d
	}

	#[test]
	fn a_lone_bitmap_goes_out_as_a_dib_and_comes_back_the_same() {
		let bmp = std::fs::read(root().join("tests/docs/pic.bmp")).unwrap();
		let mut s = HeapStore::new();
		let d = picture_doc(&mut s, &bmp);
		let mut cb = StoreClipboard::new(HeapStore::new());
		// Around other text the picture is not a lone picture.
		quill_clip::copy(&d, &mut s, 0..3, Flavours::ALL, &mut cb).unwrap();
		assert!(!cb.offers().has(Flavour::Dib));
		quill_clip::copy(&d, &mut s, 1..2, Flavours::ALL, &mut cb).unwrap();
		assert!(cb.offers().has(Flavour::Dib));
		assert!(!cb.offers().has(Flavour::Text1252), "a picture has no text");
		assert_eq!(bytes_of(&mut cb, Flavour::Dib), &bmp[14..]);
		let mut frag = Doc::new();
		let got = quill_clip::paste_fragment(&mut cb, &[Flavour::Dib], &mut s, &mut frag).unwrap();
		assert_eq!(got, Flavour::Dib);
		assert_eq!(frag.pictures.len(), 1);
		let mut back = vec![0u8; bmp.len()];
		assert_eq!(frag.picture_bytes(&s, 0, 0, &mut back), bmp.len());
		assert_eq!(back, bmp);
		assert_eq!(frag.body.len(), 1);
		assert!(frag.body.object_at(&s, 0).is_some());
		// RTF carries the picture along with the surrounding text.
		quill_clip::copy(&d, &mut s, 0..3, Flavours::TEXT_AND_RTF, &mut cb).unwrap();
		quill_clip::paste_fragment(&mut cb, &[Flavour::Rtf], &mut s, &mut frag).unwrap();
		assert_eq!(frag.pictures.len(), 1);
		frag.reset(&mut s);
	}

	#[test]
	fn damaged_dibs_are_refused_without_a_leak() {
		let bmp = std::fs::read(root().join("tests/docs/pic.bmp")).unwrap();
		let mut cb = StoreClipboard::new(HeapStore::new());
		for bad in [
			vec![],
			bmp[14..40].to_vec(),
			{
				let mut v = bmp[14..].to_vec();
				v[14] = 7; // seven bits a pixel
				v
			},
			bmp[14..bmp.len() - 20].to_vec(),
		] {
			cb.clear();
			put_bytes(&mut cb, Flavour::Dib, &bad);
			let mut fs = HeapStore::new();
			let mut frag = Doc::new();
			let r = quill_clip::paste_fragment(&mut cb, &[Flavour::Dib], &mut fs, &mut frag);
			assert!(r.is_err() || frag.pictures.len() == 1);
			frag.reset(&mut fs);
			assert_eq!(fs.live_chunks(), 0);
		}
	}
}

#[test]
fn the_scratch_document_is_small_enough_for_a_win16_stack() {
	// `copy` holds one `Doc` on the stack (about 5 KB on the host); the Win16 stack is 28 KB.
	let n = std::mem::size_of::<Doc>();
	assert!(n < 8192, "Doc is {n} bytes");
}

#[test]
fn a_held_clipboard_in_the_documents_own_store_copies_pastes_and_frees() {
	use quill_clip::{insert_fragment, Held, PasteStyle};
	let mut s = HeapStore::new();
	let mut doc = build(
		&mut s,
		&[(vec![("abc".to_string(), 1), ("def".to_string(), 3)], 1)],
	);
	let mut held: Held<4, 0> = Held::new(256);
	let base = s.live_chunks();
	let n = doc.body.len();
	held.copy_from(&doc, &mut s, 0..n - 1, Flavours::TEXT_AND_RTF)
		.unwrap();
	assert!(held.ready().has(Flavour::Rtf) && held.ready().has(Flavour::Text1252));
	assert!(!held.ready().has(Flavour::Dib));
	let mut frag = Doc::new();
	assert_eq!(
		held.paste_to(&[Flavour::Rtf, Flavour::Text1252], &mut s, &mut frag),
		Ok(Flavour::Rtf)
	);
	let mut done = 0;
	insert_fragment(
		&mut doc,
		&mut s,
		&frag,
		n - 1,
		PasteStyle::Rich {
			first_pfmt: Some(0),
		},
		1,
		&mut done,
	)
	.unwrap();
	assert_eq!(done, 6);
	assert_eq!(doc.body.len(), n + 6);
	frag.reset(&mut s);
	held.clear(&mut s);
	assert!(held.is_empty());
	// Only what the document took remains: nothing of the clipboard or the fragment.
	doc.reset(&mut s);
	assert_eq!(s.live_chunks(), 0, "{base}");
}
