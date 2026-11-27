// HLPROBE.EXE: the guest end of the host-link spike (WP-HOST-1), run on ReactOS 0.4.16 from the Run box as `D:\H.EXE <mode> ...`.
// Every mode appends lines to C:\hlprobe.log and, when COM2 opens, writes them to COM2 as they happen; `l` copies the log to COM1.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Throwaway code (docs/plans/host-integration.md, WP-HOST-1). Modes:
//! `w` watch the clipboard through the viewer chain and log every change; `f` log the clipboard once;
//! `r` set `Rich Text Format` and `CF_TEXT`; `t TEXT` set `CF_TEXT` only; `u` set `CF_UNICODETEXT` only; `b` set `CF_BITMAP` only;
//! `d` set `CF_DIB` only (each of the set modes then logs what the clipboard offers back);
//! `c e|s|o [t] [N]` COM1: echo, sink (count only) or source (N bytes, default 1 MiB), overlapped, or `t` for 50 ms read timeouts;
//! `l` copy C:\hlprobe.log to COM1; `s YYYYMMDDHHMMSS` SetLocalTime; `m` list the display modes; `x W H [BPP]` change the mode; `p` ping.
#![no_std]
#![no_main]
#![allow(static_mut_refs)]

mod rt;
mod sys;

use core::ptr::{null, null_mut};
use core::sync::atomic::{AtomicUsize, Ordering};

use sys::*;

static NEXT_VIEWER: AtomicUsize = AtomicUsize::new(0);
static DRAWS: AtomicUsize = AtomicUsize::new(0);
static RTF_FORMAT: AtomicUsize = AtomicUsize::new(0);
/// Option letters after the mode: S skips SetupComm, D skips SetCommState, Q reads in 1-byte pieces, P polls with Sleep(1), L no progress logs.
static OPTIONS: AtomicUsize = AtomicUsize::new(0);

fn option(letter: u8) -> bool {
	OPTIONS.load(Ordering::Relaxed) as u32 & (1 << (letter - b'A')) != 0
}

const LOG: &[u8] = b"C:\\hlprobe.log\0";
const RTF_TEXT: &[u8] = b"{\\rtf1\\ansi{\\fonttbl{\\f0 Arial;}}\\f0\\fs28 plain {\\b bold} {\\i italic}\\par}\0";

/// A line being built: bytes appended with `s`, decimal with `d`, hexadecimal with `x`, bytes shown printable with `p`.
struct Line {
	b: [u8; 700],
	n: usize,
}

impl Line {
	fn new() -> Line {
		Line { b: [0; 700], n: 0 }
	}
	fn s(&mut self, t: &[u8]) -> &mut Line {
		for &c in t {
			if self.n < self.b.len() - 2 {
				self.b[self.n] = c;
				self.n += 1;
			}
		}
		self
	}
	fn d(&mut self, mut v: u32) -> &mut Line {
		let mut tmp = [0u8; 10];
		let mut i = tmp.len();
		loop {
			i -= 1;
			tmp[i] = b'0' + (v % 10) as u8;
			v /= 10;
			if v == 0 {
				break;
			}
		}
		self.s(&tmp[i..])
	}
	fn x(&mut self, v: u32) -> &mut Line {
		self.s(b"0x");
		let mut started = false;
		let mut shift = 32;
		while shift > 0 {
			shift -= 4;
			let nib = ((v >> shift) & 15) as u8;
			if nib != 0 || started || shift == 0 {
				started = true;
				self.s(&[if nib < 10 { b'0' + nib } else { b'a' + nib - 10 }]);
			}
		}
		self
	}
	fn p(&mut self, t: &[u8]) -> &mut Line {
		for &c in t {
			if (0x20..0x7f).contains(&c) {
				self.s(&[c]);
			} else {
				self.s(b"\\x");
				let hex = b"0123456789abcdef";
				self.s(&[hex[(c >> 4) as usize], hex[(c & 15) as usize]]);
			}
		}
		self
	}
}

fn emit(l: &mut Line) {
	l.s(b"\r\n");
	// SAFETY: plain Win32 calls on handles opened here; the buffer outlives the calls.
	unsafe {
		let f = CreateFileA(LOG.as_ptr(), GENERIC_WRITE, 1, null(), OPEN_ALWAYS, 0, null_mut());
		if f != INVALID_HANDLE_VALUE {
			SetFilePointer(f, 0, null_mut(), FILE_END);
			let mut done = 0;
			WriteFile(f, l.b.as_ptr(), l.n as u32, &mut done, null_mut());
			CloseHandle(f);
		}
		// COM2 is opened for each line and closed again, so that a probe running beside another can still log.
		let c2 = CreateFileA(
			b"\\\\.\\COM2\0".as_ptr(),
			GENERIC_WRITE,
			0,
			null(),
			OPEN_EXISTING,
			0,
			null_mut(),
		);
		if c2 != INVALID_HANDLE_VALUE {
			let mut done = 0;
			WriteFile(c2, l.b.as_ptr(), l.n as u32, &mut done, null_mut());
			CloseHandle(c2);
		}
	}
}

fn say(t: &[u8]) {
	let mut l = Line::new();
	l.s(t);
	emit(&mut l);
}

fn format_name(f: u32, l: &mut Line) {
	let name: &[u8] = match f {
		1 => b"CF_TEXT",
		2 => b"CF_BITMAP",
		3 => b"CF_METAFILEPICT",
		7 => b"CF_OEMTEXT",
		8 => b"CF_DIB",
		9 => b"CF_PALETTE",
		13 => b"CF_UNICODETEXT",
		14 => b"CF_ENHMETAFILE",
		15 => b"CF_HDROP",
		16 => b"CF_LOCALE",
		17 => b"CF_DIBV5",
		_ => b"",
	};
	if !name.is_empty() {
		l.s(name);
		return;
	}
	let mut buf = [0u8; 64];
	// SAFETY: the buffer is 64 bytes.
	let n = unsafe { GetClipboardFormatNameA(f, buf.as_mut_ptr(), 64) };
	l.s(b"\"").s(&buf[..n.clamp(0, 64) as usize]).s(b"\"");
}

/// Logs every format on the open clipboard with its size and the first bytes of the text formats.
fn dump_clip() {
	let mut fmt = 0;
	loop {
		// SAFETY: the clipboard is open on this thread.
		fmt = unsafe { EnumClipboardFormats(fmt) };
		if fmt == 0 {
			break;
		}
		let mut l = Line::new();
		l.s(b"  fmt ").d(fmt).s(b" ");
		format_name(fmt, &mut l);
		match fmt {
			2 | 3 | 9 | 14 => {
				l.s(b" (handle)");
			}
			_ => {
				// SAFETY: the clipboard is open; the block is read while locked.
				unsafe {
					let h = GetClipboardData(fmt);
					if h.is_null() {
						l.s(b" GetClipboardData NULL err ").d(GetLastError());
					} else {
						let size = GlobalSize(h);
						l.s(b" size ").d(size as u32);
						let p = GlobalLock(h);
						if !p.is_null() {
							let take = size.min(40);
							let bytes = core::slice::from_raw_parts(p, take);
							let rtf = RTF_FORMAT.load(Ordering::Relaxed) as u32;
							if fmt == 1 || fmt == 7 || (rtf != 0 && fmt == rtf) {
								l.s(b" [").p(bytes).s(b"]");
							} else if fmt == 13 {
								l.s(b" [");
								let mut i = 0;
								while i + 1 < take {
									let u = u16::from_le_bytes([bytes[i], bytes[i + 1]]);
									if (0x20..0x7f).contains(&u) {
										l.s(&[u as u8]);
									} else {
										l.s(b"<").x(u32::from(u)).s(b">");
									}
									i += 2;
								}
								l.s(b"]");
							} else if fmt == 8 && take >= 16 {
								let w = i32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
								let hgt = i32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
								let bpp = u16::from_le_bytes([bytes[14], bytes[15]]);
								l.s(b" bih ").d(w as u32).s(b"x").d(hgt as u32).s(b"x").d(u32::from(bpp));
							}
							GlobalUnlock(h);
						}
					}
				}
			}
		}
		emit(&mut l);
	}
}

fn open_clip(hwnd: Handle) -> bool {
	for _ in 0..20 {
		// SAFETY: plain call.
		if unsafe { OpenClipboard(hwnd) } != 0 {
			return true;
		}
		// SAFETY: plain call.
		unsafe { Sleep(10) };
	}
	false
}

unsafe extern "system" fn wnd_proc(hwnd: Handle, msg: u32, wparam: usize, lparam: isize) -> isize {
	match msg {
		WM_DRAWCLIPBOARD => {
			let n = DRAWS.fetch_add(1, Ordering::Relaxed) + 1;
			let mut l = Line::new();
			l.s(b"DRAW #")
				.d(n as u32)
				.s(b" tick ")
				.d(GetTickCount())
				.s(b" seq ")
				.d(GetClipboardSequenceNumber());
			l.s(b" owner ").x(GetClipboardOwner() as usize as u32);
			emit(&mut l);
			if open_clip(hwnd) {
				dump_clip();
				CloseClipboard();
			} else {
				let mut l = Line::new();
				l.s(b"  OpenClipboard failed err ").d(GetLastError());
				emit(&mut l);
			}
			let next = NEXT_VIEWER.load(Ordering::Relaxed);
			if next != 0 {
				SendMessageA(next as Handle, msg, wparam, lparam);
			}
			0
		}
		WM_CHANGECBCHAIN => {
			let next = NEXT_VIEWER.load(Ordering::Relaxed);
			let mut l = Line::new();
			l.s(b"CHANGECBCHAIN removed ")
				.x(wparam as u32)
				.s(b" new next ")
				.x(lparam as u32)
				.s(b" mine ")
				.x(next as u32);
			emit(&mut l);
			if wparam == next {
				NEXT_VIEWER.store(lparam as usize, Ordering::Relaxed);
			} else if next != 0 {
				SendMessageA(next as Handle, msg, wparam, lparam);
			}
			0
		}
		WM_DESTROY => {
			ChangeClipboardChain(hwnd, NEXT_VIEWER.load(Ordering::Relaxed) as Handle);
			PostQuitMessage(0);
			0
		}
		_ => DefWindowProcA(hwnd, msg, wparam, lparam),
	}
}

fn watch() {
	// SAFETY: a window class and a hidden window on this thread, then the message loop.
	unsafe {
		let instance = GetModuleHandleA(null());
		let class = WndClassA {
			style: 0,
			wnd_proc,
			cls_extra: 0,
			wnd_extra: 0,
			instance,
			icon: null_mut(),
			cursor: null_mut(),
			background: null_mut(),
			menu_name: null(),
			class_name: b"HLProbe\0".as_ptr(),
		};
		RegisterClassA(&class);
		let hwnd = CreateWindowExA(
			0,
			b"HLProbe\0".as_ptr(),
			b"HLProbe\0".as_ptr(),
			WS_POPUP,
			0,
			0,
			0,
			0,
			null_mut(),
			null_mut(),
			instance,
			null(),
		);
		let mut l = Line::new();
		l.s(b"WATCH start hwnd ").x(hwnd as usize as u32);
		emit(&mut l);
		let next = SetClipboardViewer(hwnd);
		NEXT_VIEWER.store(next as usize, Ordering::Relaxed);
		let mut l = Line::new();
		l.s(b"WATCH SetClipboardViewer next ")
			.x(next as usize as u32)
			.s(b" err ")
			.d(GetLastError());
		emit(&mut l);
		let mut msg: Msg = core::mem::zeroed();
		while GetMessageA(&mut msg, null_mut(), 0, 0) > 0 {
			TranslateMessage(&msg);
			DispatchMessageA(&msg);
		}
	}
}

/// Puts one block on the open clipboard. Returns whether `SetClipboardData` accepted it.
fn put(format: u32, bytes: &[u8]) -> bool {
	// SAFETY: allocates a movable block, fills it and hands it to the clipboard, which then owns it.
	unsafe {
		let h = GlobalAlloc(GMEM_MOVEABLE | GMEM_DDESHARE, bytes.len());
		if h.is_null() {
			return false;
		}
		let p = GlobalLock(h);
		if p.is_null() {
			return false;
		}
		core::ptr::copy_nonoverlapping(bytes.as_ptr(), p, bytes.len());
		GlobalUnlock(h);
		!SetClipboardData(format, h).is_null()
	}
}

fn report_availability() {
	let mut l = Line::new();
	l.s(b"  available: TEXT=")
		.d(avail(CF_TEXT))
		.s(b" UNICODETEXT=")
		.d(avail(CF_UNICODETEXT))
		.s(b" OEMTEXT=")
		.d(avail(CF_OEMTEXT));
	l.s(b" BITMAP=").d(avail(CF_BITMAP)).s(b" DIB=").d(avail(CF_DIB));
	emit(&mut l);
}

fn avail(f: u32) -> u32 {
	// SAFETY: plain call.
	unsafe { IsClipboardFormatAvailable(f) as u32 }
}

fn set_mode(mode: u8, rest: &[u8]) {
	// SAFETY: registers the RTF format; the clipboard is opened, emptied and filled on this thread.
	unsafe {
		RTF_FORMAT.store(
			RegisterClipboardFormatA(b"Rich Text Format\0".as_ptr()) as usize,
			Ordering::Relaxed,
		);
		let rtf = RTF_FORMAT.load(Ordering::Relaxed) as u32;
		if !open_clip(null_mut()) {
			say(b"SET OpenClipboard failed");
			return;
		}
		EmptyClipboard();
		let mut l = Line::new();
		match mode {
			b'r' => {
				let a = put(rtf, RTF_TEXT);
				let b = put(CF_TEXT, b"plain bold italic\0");
				l.s(b"SET rtf ok=")
					.d(a as u32)
					.s(b" text ok=")
					.d(b as u32)
					.s(b" rtf id ")
					.d(rtf);
			}
			b't' => {
				let mut text = [0u8; 120];
				let n = rest.len().min(110);
				text[..n].copy_from_slice(&rest[..n]);
				l.s(b"SET CF_TEXT only ok=").d(put(CF_TEXT, &text[..n + 1]) as u32);
			}
			b'u' => {
				// "Zo", e-acute, " ", euro sign, " ", a smiley outside the BMP's Windows-1252 and a terminating NUL.
				let units: [u16; 8] = [0x5A, 0x6F, 0xE9, 0x20, 0x20AC, 0x20, 0x263A, 0];
				let mut bytes = [0u8; 16];
				for (i, u) in units.iter().enumerate() {
					bytes[i * 2..i * 2 + 2].copy_from_slice(&u.to_le_bytes());
				}
				l.s(b"SET CF_UNICODETEXT only ok=").d(put(CF_UNICODETEXT, &bytes) as u32);
			}
			b'b' => {
				let screen = GetDC(null_mut());
				let bitmap = CreateCompatibleBitmap(screen, 64, 48);
				let mem = CreateCompatibleDC(screen);
				SelectObject(mem, bitmap);
				PatBlt(mem, 0, 0, 64, 48, 0x0042_0001); // BLACKNESS is 0x42; WHITENESS is 0xFF0062; DSTINVERT (0x550009) after BLACKNESS makes white
				PatBlt(mem, 0, 0, 64, 48, 0x0055_0009);
				SelectObject(mem, null_mut());
				DeleteDC(mem);
				ReleaseDC(null_mut(), screen);
				l.s(b"SET CF_BITMAP only ok=")
					.d(!SetClipboardData(CF_BITMAP, bitmap).is_null() as u32)
					.s(b" bitmap ")
					.x(bitmap as usize as u32);
			}
			_ => {
				// A 4 by 2 24-bit DIB: header, then two rows of four pixels padded to 12 bytes.
				let mut dib = [0u8; 40 + 24];
				let header = BitmapInfoHeader {
					size: 40,
					width: 4,
					height: 2,
					planes: 1,
					bit_count: 24,
					compression: 0,
					size_image: 24,
					x_pels: 0,
					y_pels: 0,
					clr_used: 0,
					clr_important: 0,
				};
				core::ptr::copy_nonoverlapping(&header as *const BitmapInfoHeader as *const u8, dib.as_mut_ptr(), 40);
				for b in dib[40..].iter_mut() {
					*b = 0x80;
				}
				l.s(b"SET CF_DIB only ok=").d(put(CF_DIB, &dib) as u32);
			}
		}
		emit(&mut l);
		CloseClipboard();
		// Ask for the formats a second program would see: this reads back through a fresh OpenClipboard.
		report_availability();
		if open_clip(null_mut()) {
			if mode == b't' || mode == b'u' {
				let other = if mode == b't' { CF_UNICODETEXT } else { CF_TEXT };
				let h = GetClipboardData(other);
				let mut l = Line::new();
				l.s(b"  GetClipboardData(").d(other).s(b") = ").x(h as usize as u32);
				emit(&mut l);
			}
			if mode == b'b' {
				let h = GetClipboardData(CF_DIB);
				let mut l = Line::new();
				l.s(b"  GetClipboardData(CF_DIB) = ")
					.x(h as usize as u32)
					.s(b" err ")
					.d(GetLastError());
				emit(&mut l);
			}
			if mode == b'd' {
				let h = GetClipboardData(CF_BITMAP);
				let mut l = Line::new();
				l.s(b"  GetClipboardData(CF_BITMAP) = ")
					.x(h as usize as u32)
					.s(b" err ")
					.d(GetLastError());
				emit(&mut l);
			}
			dump_clip();
			CloseClipboard();
		}
	}
}

/// `g NAME`: receives a file on COM2 (the host's second serial socket) and writes it to C:\NAME, so a new probe reaches the guest without a reboot.
fn receive(name: &[u8]) {
	// SAFETY: plain calls on local buffers.
	unsafe {
		let com = CreateFileA(
			b"\\\\.\\COM2\0".as_ptr(),
			GENERIC_READ | GENERIC_WRITE,
			0,
			null(),
			OPEN_EXISTING,
			0,
			null_mut(),
		);
		if com == INVALID_HANDLE_VALUE {
			return;
		}
		let t = CommTimeouts {
			read_interval: 0,
			read_multiplier: 0,
			read_constant: 100,
			write_multiplier: 0,
			write_constant: 0,
		};
		SetCommTimeouts(com, &t);
		let mut path = [0u8; 64];
		path[..3].copy_from_slice(b"C:\\");
		let n = name.len().min(50);
		path[3..3 + n].copy_from_slice(&name[..n]);
		let file = CreateFileA(path.as_ptr(), GENERIC_WRITE, 0, null(), 2, 0, null_mut());
		let mut buf = [0u8; 4096];
		let (mut total, mut idle) = (0u32, 0u32);
		while idle < 30 {
			let mut done = 0;
			ReadFile(com, buf.as_mut_ptr(), 4096, &mut done, null_mut());
			if done == 0 {
				if total > 0 {
					idle += 1;
				}
				continue;
			}
			idle = 0;
			let mut written = 0;
			WriteFile(file, buf.as_ptr(), done, &mut written, null_mut());
			total += done;
		}
		CloseHandle(file);
		CloseHandle(com);
		let mut l = Line::new();
		l.s(b"RECEIVED ").d(total).s(b" bytes into ").s(&path[..3 + n]);
		emit(&mut l);
	}
}

/// Sets the option bits from every token that starts with `-` (`-SDQPL`).
fn parse_options(args: &[u8]) {
	let mut bits = 0u32;
	let mut i = 0;
	while i < args.len() {
		if args[i] == b'-' && (i == 0 || args[i - 1] == b' ') {
			i += 1;
			while i < args.len() && args[i] != b' ' {
				if args[i].is_ascii_uppercase() {
					bits |= 1 << (args[i] - b'A');
				}
				i += 1;
			}
		} else {
			i += 1;
		}
	}
	OPTIONS.store(bits as usize, Ordering::Relaxed);
}

fn find(args: &[u8], n: usize) -> &[u8] {
	let mut i = 0;
	let mut idx = 0;
	while i < args.len() {
		while i < args.len() && args[i] == b' ' {
			i += 1;
		}
		let start = i;
		while i < args.len() && args[i] != b' ' {
			i += 1;
		}
		if start < i && args[start] != b'-' {
			if idx == n {
				return &args[start..i];
			}
			idx += 1;
		}
	}
	&[]
}

fn number(t: &[u8]) -> u32 {
	let mut v = 0u32;
	for &c in t {
		if c.is_ascii_digit() {
			v = v.wrapping_mul(10).wrapping_add(u32::from(c - b'0'));
		}
	}
	v
}

fn pattern(i: u32) -> u8 {
	(i.wrapping_mul(31).wrapping_add(7) & 0xFF) as u8
}

struct Com {
	h: Handle,
	overlapped: bool,
	style: u8,
	ev_read: Handle,
	ev_write: Handle,
	ev_wait: Handle,
	ov_wait: Overlapped,
	mask: u32,
	pending: bool,
}

impl Com {
	fn open(style: u8) -> Option<Com> {
		Com::open_port(b"\\\\.\\COM1\0", style)
	}

	fn open_port(port: &[u8], style: u8) -> Option<Com> {
		let overlapped = style != b't';
		// SAFETY: the port is opened and configured on this thread; every struct passed is a local.
		unsafe {
			let flags = if overlapped { FILE_FLAG_OVERLAPPED } else { 0 };
			let h = CreateFileA(
				port.as_ptr(),
				GENERIC_READ | GENERIC_WRITE,
				0,
				null(),
				OPEN_EXISTING,
				flags,
				null_mut(),
			);
			let mut l = Line::new();
			l.s(b"COM open overlapped=")
				.d(overlapped as u32)
				.s(b" handle ")
				.x(h as usize as u32)
				.s(b" err ")
				.d(GetLastError());
			emit(&mut l);
			if h == INVALID_HANDLE_VALUE {
				return None;
			}
			let r = if option(b'S') { 1 } else { SetupComm(h, 65536, 65536) };
			let mut l = Line::new();
			l.s(b"COM SetupComm(64K) = ").d(r as u32).s(b" err ").d(GetLastError());
			emit(&mut l);
			PurgeComm(h, PURGE_ALL);
			let mut dcb: Dcb = core::mem::zeroed();
			dcb.length = 28;
			let got = GetCommState(h, &mut dcb);
			dcb.length = 28;
			dcb.baud = 115_200;
			dcb.byte_size = 8;
			dcb.parity = 0;
			dcb.stop_bits = 0;
			dcb.flags = 1 | (1 << 4) | (1 << 12);
			let set = if option(b'D') { 1 } else { SetCommState(h, &dcb) };
			let mut l = Line::new();
			l.s(b"COM GetCommState = ")
				.d(got as u32)
				.s(b" SetCommState = ")
				.d(set as u32)
				.s(b" err ")
				.d(GetLastError());
			emit(&mut l);
			let t = match style {
				b't' => CommTimeouts {
					read_interval: 0,
					read_multiplier: 0,
					read_constant: 50,
					write_multiplier: 0,
					write_constant: 0,
				},
				b'r' => CommTimeouts {
					read_interval: MAXDWORD,
					read_multiplier: MAXDWORD,
					read_constant: 250,
					write_multiplier: 0,
					write_constant: 0,
				},
				_ => CommTimeouts {
					read_interval: MAXDWORD,
					read_multiplier: 0,
					read_constant: 0,
					write_multiplier: 0,
					write_constant: 0,
				},
			};
			let r = SetCommTimeouts(h, &t);
			let mut l = Line::new();
			l.s(b"COM SetCommTimeouts(")
				.s(match style {
					b't' => b"50 ms, synchronous",
					b'r' => b"250 ms, overlapped read",
					_ => b"immediate",
				})
				.s(b") = ")
				.d(r as u32)
				.s(b" err ")
				.d(GetLastError());
			emit(&mut l);
			if style == b'w' {
				let m = SetCommMask(h, EV_RXCHAR);
				let mut l = Line::new();
				l.s(b"COM SetCommMask(EV_RXCHAR) = ")
					.d(m as u32)
					.s(b" err ")
					.d(GetLastError());
				emit(&mut l);
			}
			Some(Com {
				h,
				overlapped,
				style,
				ev_read: CreateEventA(null(), 1, 0, null()),
				ev_write: CreateEventA(null(), 1, 0, null()),
				ev_wait: CreateEventA(null(), 1, 0, null()),
				ov_wait: Com::ov(null_mut()),
				mask: 0,
				pending: false,
			})
		}
	}

	fn ov(event: Handle) -> Overlapped {
		Overlapped {
			internal: 0,
			internal_high: 0,
			offset: 0,
			offset_high: 0,
			event,
		}
	}

	/// Reads what is there (overlapped, immediate timeouts) or waits up to 50 ms (timeouts mode).
	fn read(&self, buf: &mut [u8]) -> u32 {
		// SAFETY: the buffer and the OVERLAPPED outlive the call, which is waited for before returning.
		unsafe {
			let mut done = 0;
			if self.overlapped {
				let mut ov = Com::ov(self.ev_read);
				ResetEvent(self.ev_read);
				if ReadFile(self.h, buf.as_mut_ptr(), buf.len() as u32, &mut done, &mut ov) == 0 {
					if GetLastError() == ERROR_IO_PENDING {
						GetOverlappedResult(self.h, &mut ov, &mut done, 1);
					} else {
						return 0;
					}
				}
			} else if ReadFile(self.h, buf.as_mut_ptr(), buf.len() as u32, &mut done, null_mut()) == 0 {
				return 0;
			}
			done
		}
	}

	fn write(&self, buf: &[u8]) -> u32 {
		// SAFETY: as in `read`.
		unsafe {
			let mut done = 0;
			if self.overlapped {
				let mut ov = Com::ov(self.ev_write);
				ResetEvent(self.ev_write);
				if WriteFile(self.h, buf.as_ptr(), buf.len() as u32, &mut done, &mut ov) == 0 {
					if GetLastError() == ERROR_IO_PENDING {
						GetOverlappedResult(self.h, &mut ov, &mut done, 1);
					} else {
						return 0;
					}
				}
			} else if WriteFile(self.h, buf.as_ptr(), buf.len() as u32, &mut done, null_mut()) == 0 {
				return 0;
			}
			done
		}
	}

	/// Waits for `EV_RXCHAR` for up to 250 ms, keeping one `WaitCommEvent` pending across calls. Returns 1 when the event came.
	fn wait_event(&mut self) -> u32 {
		// SAFETY: the OVERLAPPED and the mask live in `self`, which outlives the pending call.
		unsafe {
			if !self.pending {
				ResetEvent(self.ev_wait);
				self.ov_wait = Com::ov(self.ev_wait);
				if WaitCommEvent(self.h, &mut self.mask, &mut self.ov_wait) != 0 {
					return 1;
				}
				if GetLastError() != ERROR_IO_PENDING {
					return 0;
				}
				self.pending = true;
			}
			if WaitForSingleObject(self.ev_wait, 250) == WAIT_OBJECT_0 {
				let mut done = 0;
				GetOverlappedResult(self.h, &mut self.ov_wait, &mut done, 0);
				self.pending = false;
				return 1;
			}
			0
		}
	}
}

/// `c d`: the two-port design. Data arrives on COM2 (the guest never writes to it) and a 16-byte credit goes out on COM1 for every 4096 bytes taken
/// off COM2, so no port ever sends and receives at once. Nothing is logged until COM2 is closed, because COM2 is the data port.
fn dual_test() {
	let (Some(mut data), Some(mut credit)) = (Com::open_port(b"\\\\.\\COM2\0", b't'), Com::open(b't')) else {
		return;
	};
	let mut buf = [0u8; 4096];
	// SAFETY: plain call.
	let started = unsafe { GetTickCount() };
	let (mut rx, mut acks, mut bad, mut pending) = (0u32, 0u32, 0u32, 0u32);
	let (mut first_rx, mut last_rx) = (0u32, 0u32);
	loop {
		let n = data.read(&mut buf);
		// SAFETY: plain call.
		let now = unsafe { GetTickCount() };
		if n > 0 {
			if rx == 0 {
				first_rx = now;
			}
			last_rx = now;
			for (i, &b) in buf[..n as usize].iter().enumerate() {
				if b != pattern(rx.wrapping_add(i as u32)) {
					bad += 1;
				}
			}
			rx = rx.wrapping_add(n);
			pending += n;
			while pending >= 4096 {
				pending -= 4096;
				acks += 1;
				credit.write(b"ACK-CREDIT-4096\n");
			}
		} else if (rx > 0 && now.wrapping_sub(last_rx) > 3000) || (rx == 0 && now.wrapping_sub(started) > 25_000) {
			break;
		}
	}
	// SAFETY: the handles are ours.
	unsafe {
		CloseHandle(data.h);
	}
	let mut l = Line::new();
	l.s(b"COM dual: rx ")
		.d(rx)
		.s(b" acks ")
		.d(acks)
		.s(b" bad ")
		.d(bad)
		.s(b" rx_ms ")
		.d(last_rx.wrapping_sub(first_rx));
	emit(&mut l);
	com_end(&mut credit);
}

fn com_test(kind: u8, style: u8, count: u32) {
	if kind == b'd' {
		dual_test();
		return;
	}
	let Some(mut com) = Com::open(style) else { return };
	let mut buf = [0u8; 4096];
	// SAFETY: plain calls.
	let started = unsafe { GetTickCount() };
	if kind == b'o' {
		let (mut sent, mut at) = (0u32, 0u32);
		let mut short = 0u32;
		while at < count {
			let n = (count - at).min(buf.len() as u32) as usize;
			for (i, b) in buf[..n].iter_mut().enumerate() {
				*b = pattern(at + i as u32);
			}
			let w = com.write(&buf[..n]);
			if w as usize != n {
				short += 1;
			}
			sent += w;
			at += n as u32;
		}
		// SAFETY: plain call.
		let ended = unsafe { GetTickCount() };
		let mut l = Line::new();
		l.s(b"COM source: ")
			.d(sent)
			.s(b" bytes of ")
			.d(count)
			.s(b" written in ")
			.d(ended.wrapping_sub(started))
			.s(b" ms, short writes ")
			.d(short);
		emit(&mut l);
		// SAFETY: plain call.
		unsafe { Sleep(2000) };
		com_end(&mut com);
		return;
	}
	let (mut rx, mut tx, mut bad, mut first_bad) = (0u32, 0u32, 0u32, 0u32);
	let (mut first_rx, mut last_rx, mut max_read) = (0u32, 0u32, 0u32);
	let (mut hits, mut waits, mut reads) = (0u32, 0u32, 0u32);
	let mut pending_ack = 0u32;
	let mut next_report = started.wrapping_add(2000);
	loop {
		let n = if option(b'Q') {
			com.read(&mut buf[..1])
		} else {
			com.read(&mut buf)
		};
		// SAFETY: plain call.
		let now = unsafe { GetTickCount() };
		if !option(b'L') && now.wrapping_sub(next_report) < 0x8000_0000 {
			next_report = now.wrapping_add(2000);
			let mut errors = 0;
			let mut stat = ComStat {
				flags: 0,
				in_queue: 0,
				out_queue: 0,
			};
			// SAFETY: locals passed by pointer.
			unsafe { ClearCommError(com.h, &mut errors, &mut stat) };
			let mut l = Line::new();
			l.s(b"COM progress in_queue ")
				.d(stat.in_queue)
				.s(b" errors ")
				.x(errors)
				.s(b" tick ")
				.d(now)
				.s(b" rx ")
				.d(rx)
				.s(b" tx ")
				.d(tx)
				.s(b" reads ")
				.d(reads)
				.s(b" ev_hits ")
				.d(hits)
				.s(b" ev_timeouts ")
				.d(waits);
			emit(&mut l);
		}
		if n > 0 {
			if rx == 0 {
				first_rx = now;
			}
			last_rx = now;
			reads += 1;
			max_read = max_read.max(n);
			for (i, &b) in buf[..n as usize].iter().enumerate() {
				if b != pattern(rx.wrapping_add(i as u32)) {
					if bad == 0 {
						first_bad = rx.wrapping_add(i as u32);
					}
					bad += 1;
				}
			}
			rx = rx.wrapping_add(n);
			if kind == b'e' {
				tx = tx.wrapping_add(com.write(&buf[..n as usize]));
			} else if kind == b'a' {
				// The link's own pattern: a 16-byte acknowledgement (a credit) for every 4096 bytes taken off the port.
				pending_ack = pending_ack.wrapping_add(n);
				while pending_ack >= 4096 {
					pending_ack -= 4096;
					tx = tx.wrapping_add(com.write(b"ACK-CREDIT-4096\n"));
				}
			}
		} else {
			if option(b'P') {
				// SAFETY: plain call.
				unsafe { Sleep(1) };
			} else if com.style == b'w' {
				if com.wait_event() != 0 {
					hits += 1;
				} else {
					waits += 1;
				}
			}
			if rx > 0 && now.wrapping_sub(last_rx) > 3000 {
				break;
			}
			if rx == 0 && now.wrapping_sub(started) > 25_000 {
				break;
			}
		}
	}
	let mut l = Line::new();
	l.s(b"COM ")
		.s(match kind {
			b'e' => b"echo",
			b'a' => b"ack",
			_ => b"sink",
		})
		.s(match com.style {
			b't' => b" (sync, 50 ms timeouts)",
			b'r' => b" (overlapped read, 250 ms)",
			_ => b" (overlapped, WaitCommEvent)",
		});
	l.s(b": rx ")
		.d(rx)
		.s(b" tx ")
		.d(tx)
		.s(b" bad ")
		.d(bad)
		.s(b" firstbad ")
		.d(first_bad);
	l.s(b" rx_ms ")
		.d(last_rx.wrapping_sub(first_rx))
		.s(b" reads ")
		.d(reads)
		.s(b" maxread ")
		.d(max_read);
	l.s(b" ev_hits ").d(hits).s(b" ev_timeouts ").d(waits);
	emit(&mut l);
	com_end(&mut com);
}

fn com_end(com: &mut Com) {
	let mut errors = 0;
	let mut stat = ComStat {
		flags: 0,
		in_queue: 0,
		out_queue: 0,
	};
	// SAFETY: locals passed by pointer.
	unsafe {
		ClearCommError(com.h, &mut errors, &mut stat);
	}
	let mut l = Line::new();
	l.s(b"COM end: ClearCommError errors ")
		.x(errors)
		.s(b" in_queue ")
		.d(stat.in_queue)
		.s(b" out_queue ")
		.d(stat.out_queue);
	l.s(b" pending_wait ").d(com.pending as u32);
	emit(&mut l);
	// SAFETY: the handle is ours; SetCommMask cancels a pending WaitCommEvent.
	unsafe {
		if com.pending {
			say(b"COM cancelling the pending wait with SetCommMask(0)");
			SetCommMask(com.h, 0);
			let r = WaitForSingleObject(com.ev_wait, 2000);
			let mut l = Line::new();
			l.s(b"COM the pending wait completed after the cancel: ")
				.d((r == WAIT_OBJECT_0) as u32);
			emit(&mut l);
		}
		CloseHandle(com.h);
	}
	say(b"COM closed");
}

/// Copies C:\hlprobe.log to COM1 (non-overlapped).
fn dump_log() {
	let Some(com) = Com::open(b't') else { return };
	// SAFETY: plain file calls with a local buffer.
	unsafe {
		let f = CreateFileA(LOG.as_ptr(), GENERIC_READ, 3, null(), OPEN_EXISTING, 0, null_mut());
		if f == INVALID_HANDLE_VALUE {
			return;
		}
		let mut buf = [0u8; 1024];
		let mut sent = 0;
		com.write(b"=====LOG-BEGIN=====\r\n");
		loop {
			let mut done = 0;
			if ReadFile(f, buf.as_mut_ptr(), 1024, &mut done, null_mut()) == 0 || done == 0 {
				break;
			}
			sent += com.write(&buf[..done as usize]);
		}
		com.write(b"=====LOG-END=====\r\n");
		CloseHandle(f);
		Sleep(2000);
		let _ = sent;
		CloseHandle(com.h);
	}
}

/// `s YYYYMMDDHHMMSS` calls SetLocalTime, `S ...` calls SetSystemTime (UTC); both read the clock back at once, after 1 s and after 3 s.
fn set_time(digits: &[u8], utc: bool) {
	let field = |from: usize, len: usize| number(digits.get(from..from + len).unwrap_or(&[])) as u16;
	let t = SystemTime {
		year: field(0, 4),
		month: field(4, 2),
		day_of_week: 0,
		day: field(6, 2),
		hour: field(8, 2),
		minute: field(10, 2),
		second: field(12, 2),
		millis: 0,
	};
	// SAFETY: locals passed by pointer.
	unsafe {
		let mut before: SystemTime = core::mem::zeroed();
		GetLocalTime(&mut before);
		if option(b'T') {
			// Enable SeSystemtimePrivilege in the process token first.
			let mut token: Handle = null_mut();
			let opened = OpenProcessToken(GetCurrentProcess(), 0x28, &mut token);
			let mut luid = [0u32; 2];
			let looked = LookupPrivilegeValueA(null(), b"SeSystemtimePrivilege\0".as_ptr(), &mut luid);
			let tp = TokenPrivileges {
				count: 1,
				luid_low: luid[0],
				luid_high: luid[1] as i32,
				attributes: 2,
			};
			let adjusted = AdjustTokenPrivileges(token, 0, &tp, 0, null_mut(), null_mut());
			let mut l = Line::new();
			l.s(b"TIME privilege: OpenProcessToken ")
				.d(opened as u32)
				.s(b" LookupPrivilegeValue ")
				.d(looked as u32)
				.s(b" AdjustTokenPrivileges ")
				.d(adjusted as u32);
			l.s(b" last error ").d(GetLastError());
			emit(&mut l);
		}
		let r = if utc { SetSystemTime(&t) } else { SetLocalTime(&t) };
		let err = GetLastError();
		let mut l = Line::new();
		l.s(if utc { b"TIME SetSystemTime(" } else { b"TIME SetLocalTime(" })
			.d(u32::from(t.year))
			.s(b"-")
			.d(u32::from(t.month))
			.s(b"-")
			.d(u32::from(t.day))
			.s(b" ");
		l.d(u32::from(t.hour))
			.s(b":")
			.d(u32::from(t.minute))
			.s(b":")
			.d(u32::from(t.second))
			.s(b") = ")
			.d(r as u32)
			.s(b" err ")
			.d(err);
		l.s(b"; before ")
			.d(u32::from(before.hour))
			.s(b":")
			.d(u32::from(before.minute))
			.s(b":")
			.d(u32::from(before.second));
		emit(&mut l);
		for wait in [0u32, 1000, 2000] {
			Sleep(wait);
			let mut back: SystemTime = core::mem::zeroed();
			GetLocalTime(&mut back);
			let mut l = Line::new();
			l.s(b"  local clock now ")
				.d(u32::from(back.year))
				.s(b"-")
				.d(u32::from(back.month))
				.s(b"-")
				.d(u32::from(back.day))
				.s(b" ");
			l.d(u32::from(back.hour))
				.s(b":")
				.d(u32::from(back.minute))
				.s(b":")
				.d(u32::from(back.second))
				.s(b" tick ")
				.d(GetTickCount());
			emit(&mut l);
		}
	}
}

/// `z BIAS`: SetTimeZoneInformation with only the bias set (minutes west of UTC, as the page's getTimezoneOffset gives), then the local clock.
fn set_zone(bias: i32) {
	// SAFETY: a zeroed TIME_ZONE_INFORMATION passed by pointer.
	unsafe {
		let mut token: Handle = null_mut();
		OpenProcessToken(GetCurrentProcess(), 0x28, &mut token);
		let mut luid = [0u32; 2];
		LookupPrivilegeValueA(null(), b"SeSystemtimePrivilege\0".as_ptr(), &mut luid);
		let tp = TokenPrivileges {
			count: 1,
			luid_low: luid[0],
			luid_high: luid[1] as i32,
			attributes: 2,
		};
		AdjustTokenPrivileges(token, 0, &tp, 0, null_mut(), null_mut());
		let mut zone = [0u8; 172];
		let before = GetTimeZoneInformation(&mut zone);
		let old_bias = i32::from_le_bytes([zone[0], zone[1], zone[2], zone[3]]);
		let mut local: SystemTime = core::mem::zeroed();
		GetLocalTime(&mut local);
		let mut l = Line::new();
		l.s(b"ZONE before: GetTimeZoneInformation = ")
			.d(before)
			.s(b" bias ")
			.d(old_bias as u32)
			.s(b" local ")
			.d(u32::from(local.hour))
			.s(b":")
			.d(u32::from(local.minute));
		emit(&mut l);
		let mut set = [0u8; 172];
		set[..4].copy_from_slice(&bias.to_le_bytes());
		let r = SetTimeZoneInformation(&set);
		let err = GetLastError();
		Sleep(1500);
		GetLocalTime(&mut local);
		let mut after = [0u8; 172];
		GetTimeZoneInformation(&mut after);
		let new_bias = i32::from_le_bytes([after[0], after[1], after[2], after[3]]);
		let mut l = Line::new();
		l.s(b"ZONE SetTimeZoneInformation(bias ")
			.d(bias as u32)
			.s(b") = ")
			.d(r as u32)
			.s(b" err ")
			.d(err)
			.s(b"; bias now ")
			.d(new_bias as u32);
		l.s(b" local ")
			.d(u32::from(local.hour))
			.s(b":")
			.d(u32::from(local.minute))
			.s(b":")
			.d(u32::from(local.second));
		emit(&mut l);
	}
}

fn dev_u32(d: &DevMode, at: usize) -> u32 {
	u32::from_le_bytes([d.0[at], d.0[at + 1], d.0[at + 2], d.0[at + 3]])
}

fn modes() {
	// SAFETY: a zeroed DEVMODE with dmSize set, as the call requires.
	unsafe {
		let mut d = DevMode([0; 156]);
		d.0[36] = 156;
		let mut i = 0;
		let mut l = Line::new();
		let mut shown = 0;
		while EnumDisplaySettingsA(null(), i, &mut d) != 0 && i < 400 {
			l.d(dev_u32(&d, 108))
				.s(b"x")
				.d(dev_u32(&d, 112))
				.s(b"x")
				.d(dev_u32(&d, 104))
				.s(b"@")
				.d(dev_u32(&d, 120))
				.s(b" ");
			shown += 1;
			if shown == 8 {
				emit(&mut l);
				l = Line::new();
				l.s(b"  ");
				shown = 0;
			}
			i += 1;
		}
		if shown > 0 {
			emit(&mut l);
		}
		let mut l = Line::new();
		l.s(b"MODES count ").d(i).s(b"; current ENUM_CURRENT: ");
		if EnumDisplaySettingsA(null(), 0xFFFF_FFFF, &mut d) != 0 {
			l.d(dev_u32(&d, 108)).s(b"x").d(dev_u32(&d, 112)).s(b"x").d(dev_u32(&d, 104));
		} else {
			l.s(b"failed");
		}
		l.s(b"; metrics ")
			.d(GetSystemMetrics(0) as u32)
			.s(b"x")
			.d(GetSystemMetrics(1) as u32);
		emit(&mut l);
	}
}

fn change_mode(w: u32, h: u32, bpp: u32) {
	// SAFETY: a DEVMODE filled from the current mode, then changed; flags 0 means this session only.
	unsafe {
		let mut d = DevMode([0; 156]);
		d.0[36] = 156;
		EnumDisplaySettingsA(null(), 0xFFFF_FFFF, &mut d);
		let bpp = if bpp == 0 { dev_u32(&d, 104) } else { bpp };
		d.0[104..108].copy_from_slice(&bpp.to_le_bytes());
		d.0[108..112].copy_from_slice(&w.to_le_bytes());
		d.0[112..116].copy_from_slice(&h.to_le_bytes());
		d.0[40..44].copy_from_slice(&(0x40000u32 | 0x80000 | 0x100000).to_le_bytes());
		let before = (GetSystemMetrics(0), GetSystemMetrics(1));
		let r = ChangeDisplaySettingsA(&d, 0);
		Sleep(1500);
		let mut l = Line::new();
		l.s(b"MODE ChangeDisplaySettingsA(")
			.d(w)
			.s(b"x")
			.d(h)
			.s(b"x")
			.d(bpp)
			.s(b", 0) = ")
			.d(r as u32)
			.s(b" (0 ok, 1 restart, 4294967295 failed, 4294967294 badmode)");
		l.s(b"; metrics ")
			.d(before.0 as u32)
			.s(b"x")
			.d(before.1 as u32)
			.s(b" -> ")
			.d(GetSystemMetrics(0) as u32)
			.s(b"x")
			.d(GetSystemMetrics(1) as u32);
		emit(&mut l);
	}
}

fn args() -> &'static [u8] {
	// SAFETY: the command line is a NUL-terminated string that lives for the process.
	unsafe {
		let p = GetCommandLineA();
		let mut n = 0;
		while *p.add(n) != 0 {
			n += 1;
		}
		let all = core::slice::from_raw_parts(p, n);
		let mut i = 0;
		if all.first() == Some(&b'"') {
			i = 1;
			while i < n && all[i] != b'"' {
				i += 1;
			}
			i += 1;
		} else {
			while i < n && all[i] != b' ' {
				i += 1;
			}
		}
		&all[i.min(n)..]
	}
}

#[no_mangle]
pub extern "C" fn start() -> ! {
	let a = args();
	parse_options(a);
	let mode = find(a, 0).first().copied().unwrap_or(b'p');
	let mut l = Line::new();
	l.s(b"START mode ")
		.s(&[mode])
		.s(b" tick ")
		.d(unsafe { GetTickCount() })
		.s(b" version ")
		.x(unsafe { GetVersion() });
	l.s(b" cmdline [").p(&a[..a.len().min(200)]).s(b"]");
	emit(&mut l);
	match mode {
		b'w' => watch(),
		b'f' => {
			// SAFETY: registers the RTF name so the dump can show it.
			RTF_FORMAT.store(
				unsafe { RegisterClipboardFormatA(b"Rich Text Format\0".as_ptr()) } as usize,
				Ordering::Relaxed,
			);
			report_availability();
			if open_clip(null_mut()) {
				dump_clip();
				// SAFETY: plain call.
				unsafe { CloseClipboard() };
			}
		}
		b'r' | b'u' | b'b' | b'd' => set_mode(mode, &[]),
		b't' => {
			let at = a.iter().position(|&c| c == b't').map_or(a.len(), |p| p + 1);
			let rest = a.get(at..).unwrap_or(&[]);
			let rest = if rest.first() == Some(&b' ') { &rest[1..] } else { rest };
			set_mode(b't', rest)
		}
		b'c' => {
			let kind = find(a, 1).first().copied().unwrap_or(b'e');
			let style = match find(a, 2) {
				b"t" => b't',
				b"r" => b'r',
				_ => b'w',
			};
			let named = matches!(find(a, 2), b"t" | b"r" | b"w");
			let count = number(find(a, if named { 3 } else { 2 }));
			com_test(kind, style, if count == 0 { 1_048_576 } else { count });
		}
		b'l' => dump_log(),
		b'g' => receive(find(a, 1)),
		b's' => set_time(find(a, 1), false),
		b'S' => set_time(find(a, 1), true),
		b'm' => modes(),
		b'z' => set_zone(number(find(a, 1)) as i32),
		b'x' => change_mode(number(find(a, 1)), number(find(a, 2)), number(find(a, 3))),
		_ => say(b"PING"),
	}
	// SAFETY: ends the process.
	unsafe { ExitProcess(0) }
}
