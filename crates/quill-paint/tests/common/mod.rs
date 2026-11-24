// Shared by the painter's tests: the test font, documents opened through the app, and frames drawn on both surfaces.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::OnceLock;

use quill_app::picture::WINDOW;
use quill_app::App;
use quill_host::script::MemFiles;
use quill_host::HostStore;
use quill_paint::page::{paint_page, PageView};
use quill_paint::palette::{rgb256, WINDOWS16};
use quill_paint::raster::{Linear8, Planar4, PLANAR_BYTES};
use quill_paint::{Rect, Rgb, Surface};
use quill_strike::testfont::test_font;
use quill_strike::{SliceSource, StrikeMeasure, Strikes};

pub type TApp = App<StrikeMeasure<'static>>;

pub fn root() -> PathBuf {
	PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The committed test font, checked against what the generator writes (and rewritten with `QUILL_UPDATE_GOLDEN`).
pub fn font_bytes() -> &'static [u8] {
	static BYTES: OnceLock<&'static [u8]> = OnceLock::new();
	BYTES.get_or_init(|| {
		let path = root().join("tests/fonts/QUILLTST.FNT");
		let built = test_font();
		if std::env::var_os("QUILL_UPDATE_GOLDEN").is_some()
			&& std::fs::read(&path).ok().as_deref() != Some(&built[..])
		{
			// Written beside and renamed, because every test is a process of its own and they start together.
			std::fs::create_dir_all(path.parent().unwrap()).unwrap();
			let tmp = path.with_extension(format!("tmp{}", std::process::id()));
			std::fs::write(&tmp, &built).unwrap();
			std::fs::rename(&tmp, &path).unwrap();
		}
		let committed = std::fs::read(&path)
			.expect("tests/fonts/QUILLTST.FNT is missing; run with QUILL_UPDATE_GOLDEN=1");
		assert!(
			committed == built,
			"tests/fonts/QUILLTST.FNT is not what quill-strike's test font builder writes; run with QUILL_UPDATE_GOLDEN=1"
		);
		Box::leak(committed.into_boxed_slice())
	})
}

pub fn strikes() -> &'static Strikes<'static> {
	static S: OnceLock<&'static Strikes<'static>> = OnceLock::new();
	S.get_or_init(|| {
		let b = font_bytes();
		Box::leak(Box::new(Strikes::parse(b, b.len() as u32).unwrap()))
	})
}

pub fn new_app() -> (TApp, HostStore, MemFiles) {
	let mut s = HostStore::new();
	let mut app = App::with_measure(StrikeMeasure::new(strikes()));
	app.reset(&mut s).unwrap();
	(app, s, MemFiles::default())
}

/// An app with `bytes` opened under `name`; `None` when the app refuses the file.
pub fn app_with(name: &str, bytes: &[u8]) -> Option<(TApp, HostStore, MemFiles)> {
	let (mut app, mut s, mut f) = new_app();
	f.files.insert(name.to_ascii_uppercase(), bytes.to_vec());
	app.open(&mut s, &mut f, name.as_bytes()).ok()?;
	Some((app, s, f))
}

pub fn open_doc(name: &str) -> (TApp, HostStore, MemFiles) {
	let bytes = std::fs::read(root().join("tests/docs").join(name)).unwrap();
	app_with(name, &bytes).unwrap()
}

/// The page area of a 640 by 480 screen with two toolbar rows and a ruler above, a status bar below and a scroll bar beside.
pub const AREA: Rect = Rect::new(0, 78, 624, 460);

pub fn view() -> PageView {
	PageView {
		area: AREA,
		hscroll: 0,
	}
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Mode {
	C16,
	C256,
}

impl Mode {
	pub fn name(self) -> &'static str {
		match self {
			Mode::C16 => "16",
			Mode::C256 => "256",
		}
	}
	pub fn depth(self) -> u8 {
		match self {
			Mode::C16 => 4,
			Mode::C256 => 8,
		}
	}
	pub fn palette(self) -> Vec<[u8; 3]> {
		match self {
			Mode::C16 => WINDOWS16.iter().map(|c| [c.0, c.1, c.2]).collect(),
			Mode::C256 => (0..=255u8)
				.map(|i| {
					let c = rgb256(i);
					[c.0, c.1, c.2]
				})
				.collect(),
		}
	}
}

/// A frame: the surface's native bytes and one palette index a byte, row by row.
pub struct Frame {
	pub native: Vec<u8>,
	pub pixels: Vec<u8>,
}

pub fn planar_pixels(native: &[u8]) -> Vec<u8> {
	let mut out = vec![0u8; 640 * 480];
	for y in 0..480 {
		for b in 0..80 {
			let planes: [u8; 4] = std::array::from_fn(|p| native[p * 38400 + y * 80 + b]);
			for bit in 0..8 {
				let m = 0x80u8 >> bit;
				out[y * 640 + b * 8 + bit] = (u8::from(planes[0] & m != 0))
					| (u8::from(planes[1] & m != 0) << 1)
					| (u8::from(planes[2] & m != 0) << 2)
					| (u8::from(planes[3] & m != 0) << 3);
			}
		}
	}
	out
}

/// Something that draws on either surface (they are different types, and a closure cannot be generic).
pub trait Job {
	fn run<Sf: Surface>(&mut self, sf: &mut Sf);
}

/// Runs `job` on a fresh surface of `mode` (all silver first) and returns the frame.
pub fn frame<J: Job>(mode: Mode, job: &mut J) -> Frame {
	let mut src = SliceSource(font_bytes());
	match mode {
		Mode::C16 => {
			let mut buf = vec![0u8; PLANAR_BYTES];
			{
				let mut sf = Planar4::new(&mut buf, *strikes(), &mut src).unwrap();
				sf.fill_rect(Rect::new(0, 0, 640, 480), Rgb::SILVER);
				job.run(&mut sf);
			}
			let pixels = planar_pixels(&buf);
			Frame {
				native: buf,
				pixels,
			}
		}
		Mode::C256 => {
			let mut buf = vec![0u8; 640 * 480];
			{
				let mut sf = Linear8::new(&mut buf, 640, 480, 640, *strikes(), &mut src).unwrap();
				sf.fill_rect(Rect::new(0, 0, 640, 480), Rgb::SILVER);
				job.run(&mut sf);
			}
			Frame {
				pixels: buf.clone(),
				native: buf,
			}
		}
	}
}

/// Paints the page of an app.
pub struct PageJob<'a> {
	pub app: &'a TApp,
	pub s: &'a HostStore,
	pub window: Box<[u8; WINDOW]>,
}

impl<'a> PageJob<'a> {
	pub fn new(app: &'a TApp, s: &'a HostStore) -> Self {
		PageJob {
			app,
			s,
			window: Box::new([0; WINDOW]),
		}
	}
}

impl Job for PageJob<'_> {
	fn run<Sf: Surface>(&mut self, sf: &mut Sf) {
		paint_page(sf, self.app, self.s, &view(), &mut self.window);
	}
}

/// A two-page document with a header and a footer (no separate first page), fields, all three faces, and list-free paragraphs.
pub fn long_rtf() -> Vec<u8> {
	let mut r = String::from(
		"{\\rtf1\\ansi\\deff0{\\fonttbl{\\f0\\froman Times New Roman;}{\\f1\\fswiss Arial;}{\\f2\\fmodern Courier New;}}\\paperw12240\\paperh15840\\margl1800\\margr1800\\margt1440\\margb1440\n",
	);
	r.push_str("{\\header \\pard\\qr\\f0\\fs20 Page {\\field{\\*\\fldinst PAGE}{\\fldrslt 1}} of {\\field{\\*\\fldinst NUMPAGES}{\\fldrslt 1}}\\par}\n");
	r.push_str("{\\footer \\pard\\qc\\f0\\fs20 Quill long document\\par}\n");
	for n in 1..=45 {
		let face = n % 3;
		r.push_str(&format!(
			"\\pard\\f{face}\\fs{} Paragraph {n}. The quick brown fox jumps over the lazy dog, then \\b bolder\\b0  and \\i slanted\\i0  and \\ul underlined\\ul0  and back again, so that the lines wrap more than once on a page of this width. 0123456789.\\par\n",
			20 + 4 * (n % 4)
		));
	}
	r.push_str("}\n");
	r.into_bytes()
}
