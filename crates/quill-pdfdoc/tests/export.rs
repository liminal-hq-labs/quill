// A laid-out document as a PDF: the page count and size, the words in order through a text extractor, the fonts, and a picture.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::PathBuf;
use std::process::Command;

use quill_app::App;
use quill_host::script::MemFiles;
use quill_host::HostStore;
use quill_pdfdoc::{export, Options, Std14Measure};

const DPI: u16 = 600;

fn root() -> String {
	format!("{}/../..", env!("CARGO_MANIFEST_DIR"))
}

fn open(path: &str) -> (App, HostStore) {
	let mut s = HostStore::new();
	let mut f = MemFiles::default();
	f.files.insert(
		"DOC".into(),
		std::fs::read(format!("{}/{path}", root())).unwrap(),
	);
	let mut app = App::new();
	app.reset(&mut s).unwrap();
	app.open(&mut s, &mut f, b"DOC").unwrap();
	(app, s)
}

fn pdf_of(path: &str) -> Vec<u8> {
	let (app, s) = open(path);
	let mut m = Std14Measure { dpi: DPI };
	export(
		&app,
		&s,
		&mut m,
		&Options {
			title: b"Test",
			created: Some(b"20260101000000"),
			..Options::default()
		},
	)
}

fn tool(name: &'static str) -> Option<&'static str> {
	let found = Command::new(name).arg("-v").output().is_ok();
	if !found {
		eprintln!("skipped: {name} is not installed");
	}
	found.then_some(name)
}

fn write(name: &str, bytes: &[u8]) -> PathBuf {
	let dir = std::env::temp_dir().join(format!("quill-pdfdoc-{}", std::process::id()));
	std::fs::create_dir_all(&dir).unwrap();
	let path = dir.join(name);
	std::fs::write(&path, bytes).unwrap();
	path
}

fn run(tool: &str, args: &[&str]) -> String {
	let out = Command::new(tool).args(args).output().unwrap();
	assert!(
		out.status.success(),
		"{tool}: {}",
		String::from_utf8_lossy(&out.stderr)
	);
	String::from_utf8_lossy(&out.stdout).into_owned()
}

fn text_of(bytes: &[u8], name: &str) -> Option<String> {
	let pdftotext = tool("pdftotext")?;
	let path = write(name, bytes);
	Some(run(pdftotext, &["-layout", path.to_str().unwrap(), "-"]))
}

#[test]
fn the_letter_is_one_checked_letter_page_with_its_words() {
	let bytes = pdf_of("tests/docs/letter.qld");
	assert_eq!(bytes, pdf_of("tests/docs/letter.qld"), "repeatable");
	let path = write("letter.pdf", &bytes);
	if let Some(qpdf) = tool("qpdf") {
		let out = run(qpdf, &["--check", path.to_str().unwrap()]);
		assert!(out.contains("No syntax or stream encoding errors"), "{out}");
	}
	if let Some(info) = tool("pdfinfo") {
		let out = run(info, &[path.to_str().unwrap()]);
		assert!(out.contains("612 x 792 pts (letter)"), "{out}");
		assert!(
			out.lines()
				.any(|l| l.starts_with("Pages:") && l.ends_with('1')),
			"{out}"
		);
	}
	let Some(text) = text_of(&bytes, "letter.txt") else {
		return;
	};
	let lines: Vec<String> = text
		.lines()
		.map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
		.filter(|l| !l.is_empty())
		.collect();
	assert_eq!(lines[0], "Quill Letter");
	assert_eq!(lines[1], "Dear reader,");
	assert!(
		lines[2].starts_with("This letter has bold, italic, underlined and struck"),
		"{lines:?}"
	);
	// The tab leaders are dots between the columns; the figures are where the right tab puts them.
	let dots = |l: &str| {
		l.replace('.', " ")
			.split_whitespace()
			.collect::<Vec<_>>()
			.join(" ")
	};
	assert!(
		lines.iter().any(|l| dots(l) == "Item Qty Price"),
		"{lines:?}"
	);
	assert!(
		lines
			.iter()
			.any(|l| l.starts_with("Quills 12.") && l.ends_with("3.50")),
		"{lines:?}"
	);
	assert!(
		lines.iter().any(|l| l.contains("Paper (500 sheets)")),
		"{lines:?}"
	);
	// The justified paragraph reads in order across its lines, with its curly quotes and em dash.
	let paragraph = lines.join(" ");
	assert!(
		paragraph.contains("A justified paragraph with a hanging indent."),
		"{paragraph}"
	);
	assert!(
		paragraph.contains("\u{201c}curly quotes\u{201d} and an em dash\u{2014}here."),
		"{paragraph}"
	);
	assert!(lines.iter().any(|l| l == "Yours truly,"), "{lines:?}");
	assert!(lines.last().unwrap() == "The Quill team", "{lines:?}");
}

#[test]
fn the_letter_matches_its_golden() {
	let bytes = pdf_of("tests/docs/letter.qld");
	let path = format!("{}/tests/golden/letter.pdf", env!("CARGO_MANIFEST_DIR"));
	if std::env::var_os("QUILL_UPDATE_GOLDEN").is_some() {
		std::fs::write(&path, &bytes).unwrap();
		return;
	}
	let golden = std::fs::read(&path)
		.unwrap_or_else(|_| panic!("missing golden {path}; run with QUILL_UPDATE_GOLDEN=1"));
	assert!(
		golden == bytes,
		"{path} differs; QUILL_UPDATE_GOLDEN=1 rewrites it and the diff is reviewed like code"
	);
}

#[test]
fn fonts_follow_the_documents_faces() {
	let bytes = pdf_of("tests/docs/letter.qld");
	let text = String::from_utf8_lossy(&bytes);
	// Times New Roman body with bold and italic, Arial (Helvetica) for the title, Courier New for the typewriter phrase.
	for font in [
		"Times-Roman",
		"Times-Bold",
		"Times-Italic",
		"Helvetica-Bold",
		"Courier ",
	] {
		assert!(text.contains(&format!("/BaseFont /{font}")), "{font}");
	}
}

#[test]
fn headers_footers_and_fields_are_on_every_page() {
	let bytes = pdf_of("tests/docs/print3.rtf");
	let Some(text) = text_of(&bytes, "print3.txt") else {
		return;
	};
	let pages: Vec<&str> = text
		.split('\x0c')
		.filter(|p| !p.trim().is_empty())
		.collect();
	assert_eq!(pages.len(), 3, "{text}");
	// The header's page and page count fields show this page's values, and the footer is the same on each.
	for (i, p) in pages.iter().enumerate() {
		let squeezed = p.split_whitespace().collect::<Vec<_>>().join(" ");
		assert!(
			squeezed.starts_with(&format!("Page {} of 3", i + 1)),
			"{squeezed}"
		);
		assert!(squeezed.ends_with("Quill print test"), "{squeezed}");
	}
	assert!(pages[0].contains("First page."), "{text}");
}

#[test]
fn lists_get_their_labels() {
	let bytes = pdf_of("tests/docs/lists.rtf");
	let Some(text) = text_of(&bytes, "lists.txt") else {
		return;
	};
	assert!(text.contains('\u{2022}'), "{text}");
	assert!(
		text.lines().any(|l| l.trim_start().starts_with("1.")),
		"{text}"
	);
	assert!(
		text.lines().any(|l| l.trim_start().starts_with("2.")),
		"{text}"
	);
}

#[test]
fn pictures_are_embedded_as_images() {
	let bytes = pdf_of("tests/docs/pictures.rtf");
	let text = String::from_utf8_lossy(&bytes);
	assert!(text.contains("/Subtype /Image"), "an image XObject");
	assert!(text.contains("/Filter /FlateDecode"));
	if let Some(images) = tool("pdfimages") {
		let path = write("pictures.pdf", &bytes);
		let out = run(images, &["-list", path.to_str().unwrap()]);
		assert!(out.lines().count() >= 3, "{out}");
	}
}

#[test]
fn a_page_range_writes_only_those_pages() {
	let (app, s) = open("tests/docs/print3.rtf");
	let mut m = Std14Measure { dpi: DPI };
	let all = export(&app, &s, &mut m, &Options::default());
	let one = export(
		&app,
		&s,
		&mut m,
		&Options {
			from: 2,
			to: 2,
			..Options::default()
		},
	);
	let count = |b: &[u8]| String::from_utf8_lossy(b).matches("/Type /Page ").count();
	assert_eq!(count(&all), 3);
	assert_eq!(count(&one), 1);
}
