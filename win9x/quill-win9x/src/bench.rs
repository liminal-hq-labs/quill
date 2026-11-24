// Measuring Quill for Windows 9x: the `bench` cargo feature (W9X-2). Nothing here is in the product build.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! What a key and a paint cost, in the numbers that do not depend on the machine (calls of each GDI and USER function, characters and
//! lines drawn, pixels filled, bytes blitted) and in the numbers that do (the time of each phase). `sys.rs` puts a counting function in
//! front of every import of the paint and key paths when the feature is on; `main.rs`, `gdi.rs` and `ui.rs` mark the phases with
//! `span!`. The result is written to the log file (`D:\SPELL.LOG`, or `BenchLog` in the `[Quill]` section of `QUILL.INI`), at the end of
//! every other key's paint, at the end of each spelling scan and (with `BenchEach=1`) after every paint.
//!
//! Counts go to the bucket the shell is working in: `Key` while a key is handled, `Keyed` for the paint that follows a key, `Paint` for
//! every other paint, and `Other` for the rest.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::sys::{Handle, Rect};
use crate::Shell;

#[link(name = "kernel32", kind = "raw-dylib", import_name_type = "undecorated")]
extern "system" {
	fn QueryPerformanceCounter(c: *mut i64) -> i32;
	fn QueryPerformanceFrequency(f: *mut i64) -> i32;
}

/// The most functions that can be counted.
const MAXN: usize = 192;

/// The phases that are timed. Each is inclusive of the ones inside it.
#[derive(Clone, Copy)]
#[repr(usize)]
pub enum P {
	/// The whole of `press`.
	KTotal,
	KSig,
	KApp,
	KDialogs,
	KTitle,
	KStory,
	KScroll,
	KEnsure,
	KInval,
	KArm,
	/// The whole of `paint` (`BeginPaint` to `EndPaint`).
	PTotal,
	/// The clock and the status line.
	PClock,
	/// The DOS text grid the app draws.
	PGrid,
	/// The grey under everything.
	PGrey,
	/// The toolbars and the ruler.
	PChrome,
	/// `paint_page`, the desk, the paper, the lines and the caret.
	PPage,
	/// The desk and the paper.
	PFills,
	/// `draw_line`, including the layout's walk along the line.
	PLine,
	/// The runs of text of those lines, from the first selection of a font to the last call.
	PText,
	PWave,
	PCaret,
	/// The `BitBlt` from the back buffer to the window.
	PBlit,
	Count,
}

const NP: usize = P::Count as usize;
const PNAMES: [&str; NP] = [
	"key",
	"signature",
	"app.key",
	"dialogs",
	"retitle",
	"story",
	"update_scroll",
	"ensure_visible",
	"invalidate",
	"spell_arm",
	"paint",
	"clock+status",
	"grid",
	"grey",
	"toolbars+ruler",
	"paint_page",
	"fills",
	"draw_line",
	"text",
	"waves",
	"caret",
	"blit",
];

#[derive(Clone, Copy)]
#[repr(usize)]
enum B {
	Other,
	Key,
	Keyed,
	Paint,
	Scratch,
	KeyScratch,
}
const NB: usize = 6;
const BNAMES: [&str; NB] = ["other", "key", "keyed paint", "other paint", "scratch", "scratch"];

#[derive(Clone, Copy)]
struct Bucket {
	/// How many keys or paints were added.
	n: u32,
	calls: [u32; MAXN],
	ticks: [i64; NP],
	chars: u32,
	lines: u32,
	fill_px: u64,
	blit_bytes: u64,
}

const EMPTY: Bucket = Bucket {
	n: 0,
	calls: [0; MAXN],
	ticks: [0; NP],
	chars: 0,
	lines: 0,
	fill_px: 0,
	blit_bytes: 0,
};

static mut BUCKETS: [Bucket; NB] = [EMPTY; NB];
static mut LAST: Bucket = EMPTY;
static mut LAST_SEQ: u32 = 0;
static mut LAST_KEY: Bucket = EMPTY;
static mut CUR: usize = 0;
static mut PREV: usize = 0;
static mut KEY_DIALOG: bool = false;
static mut NAMES: [&str; MAXN] = [""; MAXN];
static mut NNAMES: usize = 1;
static mut QPC_CALLS: u32 = 0;
static mut BYTES_PX: i32 = 0;

pub static mut B_KEY: i64 = 0;
pub static mut B_LAT: [i64; 64] = [0; 64];
pub static mut B_NLAT: usize = 0;
pub static mut B_T0: i64 = 0;
pub static mut B_TICKS: u32 = 0;
pub static mut B_CPU: i64 = 0;
pub static mut B_MAXTICK: i64 = 0;
pub static mut B_LOAD: i64 = 0;
pub static mut B_WORDS: u32 = 0;
pub static mut B_PAINT_N: u32 = 0;
pub static mut B_PAINT_SUM: i64 = 0;
pub static mut B_PAINT_MAX: i64 = 0;
pub static mut B_SCANS: Vec<(u32, i64, u32, i64, i64)> = Vec::new();
static mut LOG_EACH: bool = false;
static mut LOG_PATH: Vec<u8> = Vec::new();

pub fn qpc() -> i64 {
	let mut c = 0i64;
	unsafe {
		QPC_CALLS += 1;
		QueryPerformanceCounter(&mut c);
	}
	c
}

fn us(t: i64) -> i64 {
	let mut f = 1i64;
	unsafe { QueryPerformanceFrequency(&mut f) };
	t * 1_000_000 / f.max(1)
}

/// Counts one call of the function whose id is kept in `id` (0 until its first call).
///
/// # Safety
/// `id` must point to the function's own static.
pub unsafe fn hit(id: *mut u16, name: &'static str) {
	if *id == 0 {
		if NNAMES >= MAXN {
			return;
		}
		NAMES[NNAMES] = name;
		*id = NNAMES as u16;
		NNAMES += 1;
	}
	BUCKETS[CUR].calls[*id as usize] += 1;
}

/// Counts the characters one text call draws.
pub unsafe fn text(len: u32) {
	BUCKETS[CUR].chars += len;
}

/// Counts one line of the page drawn.
pub unsafe fn line() {
	BUCKETS[CUR].lines += 1;
}

/// Counts the pixels `FillRect` fills.
pub unsafe fn fill_rect(r: *const Rect) {
	if let Some(r) = r.as_ref() {
		BUCKETS[CUR].fill_px += ((r.right - r.left).max(0) as u64) * ((r.bottom - r.top).max(0) as u64);
	}
}

/// Counts the bytes a blit writes to a device of this colour depth.
pub unsafe fn blit(dc: Handle, w: i32, h: i32) {
	if BYTES_PX == 0 {
		// BITSPIXEL times PLANES, through the uncounted import.
		use crate::sys::raw_gdi::GetDeviceCaps;
		let bits = GetDeviceCaps(dc, 12) * GetDeviceCaps(dc, 14).max(1);
		BYTES_PX = (bits / 8).max(1);
	}
	BUCKETS[CUR].blit_bytes += (w.max(0) as u64) * (h.max(0) as u64) * BYTES_PX as u64;
}

/// Times a phase until it is dropped.
pub struct Span(P, i64);

pub fn span(p: P) -> Span {
	Span(p, qpc())
}

impl Drop for Span {
	fn drop(&mut self) {
		unsafe { BUCKETS[CUR].ticks[self.0 as usize] += qpc() - self.1 };
	}
}

/// A key is about to be handled.
pub unsafe fn key_begin() {
	CUR = B::KeyScratch as usize;
	BUCKETS[B::KeyScratch as usize] = EMPTY;
	B_KEY = qpc();
}

/// The key opened a dialog, which waits for the person; its time is not the key's.
pub unsafe fn dialog_taken() {
	KEY_DIALOG = true;
}

pub unsafe fn key_end() {
	CUR = B::Other as usize;
	if core::mem::take(&mut KEY_DIALOG) {
		B_KEY = 0;
		return;
	}
	let sc = BUCKETS[B::KeyScratch as usize];
	add(&mut BUCKETS[B::Key as usize], &sc);
	LAST_KEY = sc;
}

/// A paint begins: its counts go to a scratch bucket until it is known where they belong.
pub unsafe fn paint_begin() -> i64 {
	// A paint inside a key (UpdateWindow) returns to the key's bucket.
	PREV = CUR;
	CUR = B::Scratch as usize;
	BUCKETS[B::Scratch as usize] = EMPTY;
	qpc()
}

fn add(dst: &mut Bucket, src: &Bucket) {
	dst.n += 1;
	for (a, b) in dst.calls.iter_mut().zip(src.calls.iter()) {
		*a += *b;
	}
	for (a, b) in dst.ticks.iter_mut().zip(src.ticks.iter()) {
		*a += *b;
	}
	dst.chars += src.chars;
	dst.lines += src.lines;
	dst.fill_px += src.fill_px;
	dst.blit_bytes += src.blit_bytes;
}

/// A paint ends (after `EndPaint`); `t_paint` is what `paint_begin` returned.
pub unsafe fn paint_end(sh: &mut Shell, t_paint: i64) {
	let dp = qpc() - t_paint;
	let sc = &mut BUCKETS[B::Scratch as usize];
	sc.ticks[P::PTotal as usize] = dp;
	let keyed = B_KEY != 0;
	let sc = *sc;
	add(&mut BUCKETS[if keyed { B::Keyed } else { B::Paint } as usize], &sc);
	if sc.ticks[P::PPage as usize] > 0 {
		LAST = sc;
		LAST_SEQ = B_PAINT_N + 1;
	}
	CUR = PREV;
	B_PAINT_N += 1;
	B_PAINT_SUM += dp;
	B_PAINT_MAX = B_PAINT_MAX.max(dp);
	if keyed {
		let d = qpc() - B_KEY;
		B_KEY = 0;
		if B_NLAT < 64 {
			B_LAT[B_NLAT] = d;
			B_NLAT += 1;
			if B_NLAT % 2 == 0 {
				write_log(sh);
			}
		}
	} else if LOG_EACH {
		write_log(sh);
	}
}

/// The words of at least two letters in `lo..hi`.
pub unsafe fn count_words(sh: &Shell, lo: u32, hi: u32) -> u32 {
	let mut n = 0;
	let mut inword = 0;
	let mut p = lo;
	while p < hi {
		let mut buf = [0u8; 256];
		let got = sh.app.doc().body.read(&sh.store, p, &mut buf[..((hi - p) as usize).min(256)]);
		if got == 0 {
			break;
		}
		for b in &buf[..got] {
			if b.is_ascii_alphanumeric() || *b == b'\'' {
				inword += 1;
			} else {
				if inword >= 2 {
					n += 1;
				}
				inword = 0;
			}
		}
		p += got as u32;
	}
	n
}

/// Reads where the log goes and whether it is written after every paint, from the `[Quill]` section of `QUILL.INI`.
pub unsafe fn init() {
	use crate::sys::GetPrivateProfileStringA;
	let mut buf = [0u8; 264];
	let n = GetPrivateProfileStringA(
		b"Quill\0".as_ptr(),
		b"BenchLog\0".as_ptr(),
		b"\0".as_ptr(),
		buf.as_mut_ptr(),
		260,
		b"QUILL.INI\0".as_ptr(),
	) as usize;
	LOG_PATH = if n > 0 { buf[..n].to_vec() } else { b"D:\\SPELL.LOG".to_vec() };
	let n = GetPrivateProfileStringA(
		b"Quill\0".as_ptr(),
		b"BenchEach\0".as_ptr(),
		b"0\0".as_ptr(),
		buf.as_mut_ptr(),
		260,
		b"QUILL.INI\0".as_ptr(),
	) as usize;
	LOG_EACH = n > 0 && buf[0] == b'1';
}

fn calls_line(s: &mut String, b: &Bucket) {
	let mut v: Vec<(u32, &str)> = Vec::new();
	for i in 1..unsafe { NNAMES } {
		if b.calls[i] > 0 {
			v.push((b.calls[i], unsafe { NAMES[i] }));
		}
	}
	v.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(b.1)));
	let total: u32 = v.iter().map(|x| x.0).sum();
	s.push_str(&format!("  calls: total={total}"));
	for (n, name) in v {
		s.push_str(&format!(" {name}={n}"));
	}
	s.push_str("\r\n");
}

fn bucket_lines(s: &mut String, title: &str, b: &Bucket) {
	s.push_str(&format!(
		"{title}: n={} chars={} lines={} fill_px={} blit_bytes={}\r\n",
		b.n, b.chars, b.lines, b.fill_px, b.blit_bytes
	));
	calls_line(s, b);
	s.push_str("  phases_us:");
	for (i, name) in PNAMES.iter().enumerate() {
		if b.ticks[i] > 0 {
			s.push_str(&format!(" {}={}", name.replace(' ', "_"), us(b.ticks[i])));
		}
	}
	s.push_str("\r\n");
}

/// Writes the log.
pub unsafe fn write_log(sh: &mut Shell) {
	let mut s = String::new();
	{
		use crate::sys::raw_gdi::GetDeviceCaps;
		let dc = sh.screen_dc;
		s.push_str(&format!(
			"display: {}x{}, {} bits\r\n",
			GetDeviceCaps(dc, 8),
			GetDeviceCaps(dc, 10),
			GetDeviceCaps(dc, 12) * GetDeviceCaps(dc, 14).max(1)
		));
	}
	s.push_str(&format!("dictionary load: {} us\r\n", us(B_LOAD)));
	for (words, total, ticks, cpu, maxt) in B_SCANS.iter() {
		s.push_str(&format!(
			"scan: {} words, {} us elapsed, {} ticks, {} us cpu in spell_idle, longest tick {} us\r\n",
			words,
			us(*total),
			ticks,
			us(*cpu),
			us(*maxt)
		));
	}
	s.push_str(&format!(
		"paint: {} paints, mean {} us, longest {} us\r\n",
		B_PAINT_N,
		us(B_PAINT_SUM / (B_PAINT_N.max(1) as i64)),
		us(B_PAINT_MAX)
	));
	let n = B_NLAT;
	let mut sorted: Vec<i64> = B_LAT[..n].iter().map(|t| us(*t)).collect();
	sorted.sort_unstable();
	let sum: i64 = sorted.iter().sum();
	s.push_str(&format!(
		"keys: {} median {} us mean {} us max {} us\r\n",
		n,
		sorted.get(n / 2).copied().unwrap_or(0),
		sum / (n.max(1) as i64),
		sorted.last().copied().unwrap_or(0)
	));
	for v in &sorted {
		s.push_str(&format!("{v} "));
	}
	s.push_str("\r\n");
	s.push_str(&format!("timer calls: {}\r\n", QPC_CALLS));
	for b in [B::Key, B::Keyed, B::Paint, B::Other] {
		bucket_lines(&mut s, BNAMES[b as usize], &BUCKETS[b as usize]);
	}
	let (last, last_key) = (LAST, LAST_KEY);
	bucket_lines(&mut s, &format!("last key"), &last_key);
	bucket_lines(&mut s, &format!("last page paint #{LAST_SEQ} of {B_PAINT_N}"), &last);
	let mut path = LOG_PATH.clone();
	path.retain(|c| *c != 0);
	use quill_app::Files as _;
	if let Some(mut w) = sh.files.create(&path) {
		let _ = quill_formats::io::Sink::write(&mut w, s.as_bytes());
		sh.files.close(w);
	}
}
