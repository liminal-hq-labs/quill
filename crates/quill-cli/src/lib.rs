// quill-cli: reading Quill documents from the command line.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `cat` writes a document's text, `info` describes it and `convert` rewrites it in another format. The functions here return
//! text and bytes; `main` only reads arguments and files.

use quill_core::doc::{cflag, Doc, StoryKind};
use quill_core::store::Store;
use quill_core::text::SRC_OBJECT;
use quill_formats::io::SliceSource;
use quill_formats::FormatError;
use quill_host::docs::VecSink;
use quill_host::{GridMeasure, HostStore};

/// A document format.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Format {
	/// The native `.QLD` file.
	Qld,
	/// Rich Text Format.
	Rtf,
	/// Plain text.
	Text,
}

impl Format {
	/// The format an output file name asks for: `.rtf` and `.txt`, and anything else is native.
	pub fn from_name(name: &str) -> Format {
		match name
			.rsplit('.')
			.next()
			.unwrap_or("")
			.to_ascii_lowercase()
			.as_str()
		{
			"rtf" => Format::Rtf,
			"txt" => Format::Text,
			_ => Format::Qld,
		}
	}

	/// The format a `--to` option names.
	pub fn from_word(word: &str) -> Option<Format> {
		match word.to_ascii_lowercase().as_str() {
			"qld" => Some(Format::Qld),
			"rtf" => Some(Format::Rtf),
			"txt" | "text" => Some(Format::Text),
			_ => None,
		}
	}

	/// How `info` names it.
	pub fn label(self) -> &'static str {
		match self {
			Format::Qld => "Quill document (QLD)",
			Format::Rtf => "Rich Text Format (RTF)",
			Format::Text => "Plain text",
		}
	}
}

/// A document read into memory.
pub struct Loaded {
	/// The document.
	pub doc: Doc,
	/// Where its text lives.
	pub store: HostStore,
	/// The format it was read as, by content.
	pub format: Format,
	/// The file's size in bytes.
	pub size: usize,
}

/// The sentence for a failure, in the editor's words.
pub fn explain(e: FormatError) -> &'static str {
	match e {
		FormatError::BadFile => "is not a document Quill can read",
		FormatError::Newer => "was written by a newer version of Quill",
		FormatError::TooLarge => "is too large for Quill",
		FormatError::NoMemory => "does not fit in memory",
		FormatError::Write => "could not be written",
	}
}

/// Reads a document by content: a RIFF header is native, `{\rtf` is RTF and anything else is plain text.
pub fn load(bytes: &[u8]) -> Result<Loaded, FormatError> {
	let mut store = HostStore::new();
	let mut doc = Doc::new();
	let mut src = SliceSource::new(bytes);
	let format = if bytes.starts_with(b"RIFF") {
		quill_formats::qld::read(&mut doc, &mut store, &mut src)?;
		Format::Qld
	} else if bytes.starts_with(b"{\\rtf") {
		quill_formats::rtf::read(&mut doc, &mut store, &mut src)?;
		Format::Rtf
	} else {
		quill_formats::text::read(&mut doc, &mut store, &mut src)?;
		Format::Text
	};
	Ok(Loaded {
		doc,
		store,
		format,
		size: bytes.len(),
	})
}

/// Writes a loaded document in `format`.
pub fn convert(l: &Loaded, format: Format) -> Result<Vec<u8>, FormatError> {
	let mut out = VecSink(Vec::new());
	match format {
		Format::Rtf => quill_formats::rtf::write(&l.doc, &l.store, &mut out)?,
		Format::Text => quill_formats::text::write(&l.doc, &l.store, &mut out)?,
		Format::Qld => quill_formats::qld::write(&l.doc, &l.store, &mut out)?,
	}
	Ok(out.0)
}

/// Windows-1252 as Unicode.
fn ansi(b: u8) -> char {
	const HIGH: [char; 32] = [
		'€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8d}', 'Ž',
		'\u{8f}', '\u{90}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9d}',
		'ž', 'Ÿ',
	];
	match b {
		0x80..=0x9F => HIGH.get(usize::from(b - 0x80)).copied().unwrap_or('?'),
		_ => char::from(b),
	}
}

/// The SGR sequence that sets exactly `flags` from a plain start.
fn sgr(flags: u16) -> String {
	let mut s = String::from("\x1b[0");
	for (bit, code) in [
		(cflag::BOLD, 1),
		(cflag::ITALIC, 3),
		(cflag::UNDERLINE, 4),
		(cflag::STRIKE, 9),
	] {
		if flags & bit != 0 {
			s.push_str(&format!(";{code}"));
		}
	}
	s.push('m');
	s
}

/// The body's text as UTF-8, one line per paragraph. Tabs stay tabs, a line break (`0x0B`) ends the line, bullets and numbers
/// are written before their paragraphs, a picture is `[Picture W x H]` and a field is its name in brackets. With `ansi`, bold,
/// italic, underline and strikethrough are shown with terminal escape sequences, reset at every line end.
pub fn cat(l: &Loaded, ansi_styles: bool) -> String {
	let (doc, s) = (&l.doc, &l.store);
	let text = &doc.body;
	let mut out = String::new();
	let mut para = String::new();
	let mut style = 0u16;
	let mut number = 0u32;
	text.for_each_run(s, 0, text.len(), |pos, p| {
		let flags = doc.cfmts.get(s, p.cfmt).map_or(0, |c| c.flags);
		let mut buf = vec![0u8; p.len as usize];
		text.read_piece(s, &p, 0, &mut buf);
		for (i, &b) in buf.iter().enumerate() {
			let at = pos + i as u32;
			match b {
				0x0D | 0x0B => {
					if style != 0 {
						para.push_str("\x1b[0m");
						style = 0;
					}
					if b == 0x0B {
						para.push('\n');
						continue;
					}
					let pf = text.pfmt_at(s, at).and_then(|i| doc.pfmts.get(s, i));
					let mut prefix = String::new();
					if let Some(pf) = pf {
						let marked = pf.flags & 0b101 != 0;
						if marked {
							prefix.push_str(&"  ".repeat(usize::from(pf.level)));
						}
						if pf.flags & 1 != 0 {
							number = 0;
							prefix.push_str("• ");
						} else if pf.flags & 4 != 0 {
							number = if pf.flags & 8 != 0 { 1 } else { number + 1 };
							prefix.push_str(&format!("{number}. "));
						} else {
							number = 0;
						}
					}
					out.push_str(&prefix);
					out.push_str(&para);
					out.push('\n');
					para.clear();
				}
				_ => {
					if ansi_styles && flags != style {
						para.push_str(&sgr(flags));
						style = flags;
					}
					if b == 0x01 && p.source == SRC_OBJECT {
						para.push_str(&object_name(l, at));
					} else if b == 0x09 || b >= 0x20 {
						para.push(ansi(b));
					}
				}
			}
		}
	});
	out
}

fn object_name(l: &Loaded, at: u32) -> String {
	let v = l.doc.body.object_at(&l.store, at).unwrap_or(0);
	match v >> 16 {
		0 => match l.doc.pictures.get(&l.store, v & 0xFFFF) {
			Some(p) => format!("[Picture {} x {}]", p.w, p.h),
			None => "[Picture]".to_string(),
		},
		1 => "[Page]".to_string(),
		2 => "[Pages]".to_string(),
		3 => "[Date]".to_string(),
		4 => "[File]".to_string(),
		_ => "[Object]".to_string(),
	}
}

/// Counts for `info`.
struct Counts {
	characters: u32,
	words: u32,
	paragraphs: u32,
}

fn count<S: Store + ?Sized>(doc: &Doc, s: &S) -> Counts {
	let t = &doc.body;
	let mut c = Counts {
		characters: 0,
		words: 0,
		paragraphs: 0,
	};
	let mut in_word = false;
	let mut at = 0;
	let mut buf = [0u8; 256];
	while at < t.len() {
		let n = t.read(s, at, &mut buf);
		if n == 0 {
			break;
		}
		for &b in buf.iter().take(n) {
			if b == 0x0D {
				c.paragraphs += 1;
				in_word = false;
				continue;
			}
			c.characters += 1;
			let space = matches!(b, 0x09 | 0x0B | 0x20 | 0xA0);
			if !space && !in_word {
				c.words += 1;
			}
			in_word = !space;
		}
		at += n as u32;
	}
	c
}

fn inches(twips: i32) -> String {
	let hundredths = (i64::from(twips) * 100 + 720) / 1440;
	format!("{}.{:02}", hundredths / 100, hundredths % 100)
}

/// A description of the document: what `quill info` prints.
pub fn info(name: &str, l: &Loaded) -> String {
	let (doc, s) = (&l.doc, &l.store);
	let c = count(doc, s);
	let mut o = String::new();
	let mut row = |k: &str, v: String| o.push_str(&format!("{k:<12}{v}\n"));
	row("File", name.to_string());
	row("Format", l.format.label().to_string());
	row("Size", format!("{} bytes", l.size));
	row("Characters", c.characters.to_string());
	row("Words", c.words.to_string());
	row("Paragraphs", c.paragraphs.to_string());
	row("Pictures", doc.pictures.len().to_string());
	let mut fonts = Vec::new();
	let mut i = 0;
	while let Some(f) = doc.fonts.get(s, i) {
		fonts.push(String::from_utf8_lossy(f.name()).into_owned());
		i += 1;
	}
	row("Fonts", fonts.join(", "));
	let p = &doc.page;
	row(
		"Paper",
		format!("{} x {} in", inches(p.paper_w), inches(p.paper_h)),
	);
	row(
		"Margins",
		format!(
			"left {}, right {}, top {}, bottom {} in",
			inches(p.margin_l),
			inches(p.margin_r),
			inches(p.margin_t),
			inches(p.margin_b)
		),
	);
	let named = [
		(StoryKind::Header, "header"),
		(StoryKind::Footer, "footer"),
		(StoryKind::FirstHeader, "first-page header"),
		(StoryKind::FirstFooter, "first-page footer"),
	];
	let with: Vec<&str> = named
		.iter()
		.filter(|(k, _)| doc.story(*k).len() > 1)
		.map(|(_, n)| *n)
		.collect();
	row(
		"Headers",
		if with.is_empty() {
			"none".to_string()
		} else {
			with.join(", ")
		},
	);
	let (_, pages) = pages(doc, s);
	row("Text pages", pages.to_string());
	o
}

/// The pages the document takes laid out for the DOS text screen (10 characters to the inch, 6 lines): the same layout the
/// DOS build shows. Real pages depend on the printer's fonts.
fn pages<S: Store + ?Sized>(doc: &Doc, s: &S) -> (u32, u32) {
	let mut lines = 0;
	let pages =
		quill_core::layout::layout_story(doc, s, &mut GridMeasure, StoryKind::Body, |_| lines += 1);
	(lines, pages)
}
