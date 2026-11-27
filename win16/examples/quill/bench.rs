// The paint benchmark of a build made with `W16_FEATURES=bench` (QUILLB.EXE): it counts every Win16 call and the work the shell does
// for one key, and the windowed `/BENCH SCRIPT.TXT [/END]` driver that runs a script of keys through the real window procedure and
// writes BENCH.LOG. Without the feature every function here is empty and inlined away, so the shipped QUILL.EXE carries none of it.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The counting import stubs (`tools/gen_imports.py --count`) add one to a 32-bit counter per import in a block of far memory. The
// block holds 256 counters: one per import in table order, then (from `SH_BASE`) the shell's own counters that the drawing code
// bumps through `add`. Names are not in the program; `tools/bench_report.py` maps an index to a name from `tools/imports.tbl`.
//
// The driver sends each key to the window the way Windows would (`WM_KEYDOWN`, then `WM_CHAR` for text; a key that needs Ctrl held
// goes through one private message to the same `press` the character path calls), forces the paint with `UpdateWindow`, and keeps
// what each key cost: the calls made while the key was handled, and the calls made by the paint it caused. The driver's own
// `SendMessage` and `UpdateWindow` calls are taken off the counts. Nothing is written until the script is done, so the log's own file
// calls are never in a count.
//
// A script is the `/PLAY` key tokens, one per word, plus: `NAME*N` (the token N times), `type:TEXT` (each letter, digit and
// punctuation mark of TEXT as a key), `ctrl+LETTER`, `ctrl+home`, `ctrl+end`, `@phase NAME` (the keys after it are one group in the
// report) and `@open FILE`.

#![cfg_attr(not(feature = "bench"), allow(dead_code))]

/// The shell's counters, by slot (added to `SH_BASE` in the counter block).
pub const INV_PX: usize = 0;
pub const PAINT_PX: usize = 1;
pub const BLIT_PX: usize = 2;
pub const FILL_PX: usize = 3;
pub const LINES: usize = 4;
pub const RUNS: usize = 5;
pub const CHARS: usize = 6;
pub const TOOLBAR: usize = 7;
pub const RULER: usize = 8;
pub const STATUS: usize = 9;
pub const CACHES: usize = 10;
pub const METRICS: usize = 11;
pub const WALKS: usize = 12;
pub const PAINTS: usize = 13;
pub const FILLS: usize = 14;
/// How many shell counters there are.
pub const SH_N: usize = 15;

#[cfg(feature = "bench")]
mod on {
	use super::*;
	use crate::far::FarBlock;
	use crate::gdi::Rect16;
	use crate::{Play, app, c_name, press, sh};
	use quill_app::Key;
	use w16::*;

	/// The private message that carries a key the window procedure would otherwise get from the keyboard state (Ctrl held).
	pub const KEY_MSG: u32 = WM_USER + 0x51;
	const WM_KEYDOWN: u32 = 0x0100;
	const WM_CHAR: u32 = 0x0102;

	const NCNT: usize = 256;
	const SH_BASE: usize = 224;
	const MAX_PHASES: usize = 8;
	const MAX_KEYS: usize = 512;
	/// One key's record: the phase, six numbers from the calls and the clock, then the shell counters, as 32-bit words.
	const REC_WORDS: usize = 7 + SH_N;
	const REC_BYTES: usize = REC_WORDS * 4;
	const OFF_SNAP: u32 = 0x400;
	const OFF_ACC: u32 = 0x800;
	const OFF_REC: u32 = OFF_ACC + (MAX_PHASES as u32) * 2 * 0x400;
	const OFF_NAMES: u32 = OFF_REC + (MAX_KEYS * REC_BYTES) as u32;
	const NAME_LEN: usize = 16;
	const BLOCK_LEN: u32 = OFF_NAMES + (MAX_PHASES * NAME_LEN) as u32;
	const _: () = assert!(BLOCK_LEN < 0xFFF0 && COUNT_IMPORTS < SH_BASE && SH_BASE + SH_N <= NCNT);

	static mut BLOCK: FarBlock = FarBlock::empty();
	static mut SCRIPT: Option<(u32, usize)> = None;
	static mut END: bool = false;
	static mut PENDING: Option<Key> = None;

	fn block() -> &'static FarBlock {
		unsafe { &*core::ptr::addr_of!(BLOCK) }
	}

	pub fn set_script(text: u32, len: usize) {
		unsafe { SCRIPT = Some((text, len)) };
	}

	pub fn armed() -> bool {
		unsafe { (*core::ptr::addr_of!(SCRIPT)).is_some() }
	}

	pub fn set_end() {
		unsafe { END = true };
	}

	pub fn end() -> bool {
		unsafe { END }
	}

	pub fn take_key() -> Option<Key> {
		unsafe { (*core::ptr::addr_of_mut!(PENDING)).take() }
	}

	/// Adds `n` to the shell counter `slot`.
	pub fn add(slot: usize, n: u32) {
		let sel = counter_selector();
		if sel == 0 {
			return;
		}
		let p = far(sel, ((SH_BASE + slot) * 4) as u32);
		let mut v = [0u8; 4];
		unsafe {
			far_read(p, v.as_mut_ptr(), 4);
			let t = u32::from_le_bytes(v).wrapping_add(n);
			far_write(p, t.to_le_bytes().as_ptr(), 4);
		}
	}

	fn counter_selector() -> u32 {
		// The stubs read this word; the block's selector is the one thing the counting needs near.
		w16::counter_selector()
	}

	pub fn px(r: &Rect16) -> u32 {
		let w = i32::from(r.right) - i32::from(r.left);
		let h = i32::from(r.bottom) - i32::from(r.top);
		if w > 0 && h > 0 { (w * h) as u32 } else { 0 }
	}

	/// An `InvalidateRect` of `r`.
	pub fn inv(r: &Rect16) {
		add(INV_PX, px(r));
	}

	/// An `InvalidateRect` of the whole client area.
	pub fn inv_all() {
		let s = sh();
		add(INV_PX, (s.back_w.max(0) as u32) * (s.back_ch.max(0) as u32));
	}

	pub fn painted(r: &Rect16) {
		add(PAINTS, 1);
		add(PAINT_PX, px(r));
	}

	pub fn blitted(w: i32, h: i32) {
		if w > 0 && h > 0 {
			add(BLIT_PX, (w * h) as u32);
		}
	}

	pub fn filled(w: i32, h: i32) {
		add(FILLS, 1);
		add(FILL_PX, (w * h) as u32);
	}

	fn is_wm(idx: usize) -> bool {
		matches!(
			idx,
			IDX_SetWindowText
				| IDX_MoveWindow
				| IDX_ShowWindow
				| IDX_SetScrollRange
				| IDX_SetScrollPos
				| IDX_SetWindowPos
		)
	}

	fn words(a: &mut [u32; 64]) -> &mut [u8] {
		unsafe { core::slice::from_raw_parts_mut(a.as_mut_ptr().cast::<u8>(), 256) }
	}

	/// Makes the layout-walk gauge a counter: its slot holds the editor's running total.
	fn gauge() {
		let walks = app().layout_visits().to_le_bytes();
		block().write(((SH_BASE + WALKS) * 4) as u32, &walks);
	}

	/// Remembers the counters as they are now, so the next `delta` counts from here.
	fn rebase() {
		gauge();
		let b = block();
		for chunk in 0..NCNT / 64 {
			let at = (chunk * 64 * 4) as u32;
			let mut c = [0u32; 64];
			b.read(at, words(&mut c));
			b.write(OFF_SNAP + at, words(&mut c));
		}
	}

	/// Folds what the counters gained since the last call into the accumulator of `phase` and `kind` (0 the key, 1 its paint), taking
	/// `fix_n` calls of import `fix_idx` off (the driver's own). Returns the calls and the window-manager calls, and adds the shell's
	/// counters into `sh_sum`.
	fn delta(
		phase: usize,
		kind: usize,
		fix_idx: usize,
		fix_n: u32,
		sh_sum: &mut [u32; SH_N],
	) -> (u32, u32) {
		gauge();
		let b = block();
		let acc = OFF_ACC + ((phase * 2 + kind) * 0x400) as u32;
		let (mut calls, mut wm) = (0u32, 0u32);
		for chunk in 0..NCNT / 64 {
			let at = (chunk * 64 * 4) as u32;
			let (mut c, mut s, mut a) = ([0u32; 64], [0u32; 64], [0u32; 64]);
			b.read(at, words(&mut c));
			b.read(OFF_SNAP + at, words(&mut s));
			b.read(acc + at, words(&mut a));
			for i in 0..64 {
				let idx = chunk * 64 + i;
				let mut d = c[i].wrapping_sub(s[i]);
				if idx == fix_idx {
					d = d.saturating_sub(fix_n);
				}
				a[i] = a[i].wrapping_add(d);
				if idx < COUNT_IMPORTS {
					calls += d;
					if is_wm(idx) {
						wm += d;
					}
				} else if idx >= SH_BASE && idx - SH_BASE < SH_N {
					sh_sum[idx - SH_BASE] += d;
				}
			}
			b.write(acc + at, words(&mut a));
			b.write(OFF_SNAP + at, words(&mut c));
		}
		(calls, wm)
	}

	/// The virtual key for a character the keyboard would send.
	fn vk_of(c: u8) -> u32 {
		match c {
			b'a'..=b'z' => u32::from(c - 32),
			b'.' => 0xBE,
			b'-' => 0xBD,
			b',' => 0xBC,
			_ => u32::from(c),
		}
	}

	/// Sends `k` to the window; returns how many messages that took.
	fn send(hwnd: u32, k: Key) -> u32 {
		let msg = |m: u32, w: u32| unsafe {
			SendMessage(hwnd, m, w, 0);
		};
		match k {
			Key::Char(c) => {
				msg(WM_KEYDOWN, vk_of(c));
				msg(WM_CHAR, u32::from(c));
				2
			}
			Key::Enter | Key::Backspace | Key::Tab | Key::Esc => {
				let c = match k {
					Key::Enter => 0x0D,
					Key::Backspace => 0x08,
					Key::Tab => 0x09,
					_ => 0x1B,
				};
				msg(WM_KEYDOWN, c);
				msg(WM_CHAR, c);
				2
			}
			Key::Left
			| Key::Right
			| Key::Up
			| Key::Down
			| Key::Home
			| Key::End
			| Key::PageUp
			| Key::PageDown
			| Key::Delete
			| Key::Insert
			| Key::F(_) => {
				let vk = match k {
					Key::Left => 0x25,
					Key::Up => 0x26,
					Key::Right => 0x27,
					Key::Down => 0x28,
					Key::Home => 0x24,
					Key::End => 0x23,
					Key::PageUp => 0x21,
					Key::PageDown => 0x22,
					Key::Delete => 0x2E,
					Key::Insert => 0x2D,
					Key::F(n) => 0x6F + u32::from(n),
					_ => 0,
				};
				msg(WM_KEYDOWN, vk);
				1
			}
			_ => {
				unsafe { PENDING = Some(k) };
				msg(KEY_MSG, 0);
				1
			}
		}
	}

	/// The key a script word names, with its repeat count: `NAME*N` is N presses of `NAME`.
	struct Step<'a> {
		word: &'a [u8],
		times: u32,
	}

	fn step(word: &[u8]) -> Step<'_> {
		if let Some(star) = word.iter().position(|b| *b == b'*') {
			let mut n = 0u32;
			for b in &word[star + 1..] {
				n = n * 10 + u32::from(b.wrapping_sub(b'0'));
			}
			return Step {
				word: &word[..star],
				times: n.max(1),
			};
		}
		Step { word, times: 1 }
	}

	struct Run {
		hwnd: u32,
		phase: usize,
		phases: usize,
		keys: usize,
		first_tick: u32,
	}

	impl Run {
		/// Sends one key and records what it cost.
		fn key(&mut self, k: Key) {
			if self.keys >= MAX_KEYS {
				return;
			}
			let mut sh_sum = [0u32; SH_N];
			let t0 = unsafe { GetTickCount() };
			rebase();
			let sent = send(self.hwnd, k);
			let (ck, wk) = delta(self.phase, 0, IDX_SendMessage, sent, &mut sh_sum);
			unsafe { UpdateWindow(self.hwnd) };
			let (cp, wp) = delta(self.phase, 1, IDX_UpdateWindow, 1, &mut sh_sum);
			let ms = unsafe { GetTickCount() }.wrapping_sub(t0);
			let mut rec = [0u32; REC_WORDS];
			rec[0] = self.phase as u32;
			rec[1] = ck;
			rec[2] = cp;
			rec[3] = wk;
			rec[4] = wp;
			rec[5] = ms;
			rec[6] = t0.wrapping_sub(self.first_tick);
			rec[7..].copy_from_slice(&sh_sum);
			let bytes =
				unsafe { core::slice::from_raw_parts(rec.as_ptr().cast::<u8>(), REC_BYTES) };
			block().write(OFF_REC + (self.keys * REC_BYTES) as u32, bytes);
			self.keys += 1;
		}

		fn word(&mut self, w: &[u8]) {
			let st = step(w);
			for _ in 0..st.times {
				if let Some(text) = st.word.strip_prefix(b"type:") {
					for c in text {
						self.key(Key::Char(*c));
					}
				} else if let Some(l) = st.word.strip_prefix(b"ctrl+") {
					match l {
						b"home" => self.key(Key::CtrlHome),
						b"end" => self.key(Key::CtrlEnd),
						[c] => self.key(Key::Ctrl(c.to_ascii_lowercase())),
						_ => {}
					}
				} else if let Some(k) = core::str::from_utf8(st.word)
					.ok()
					.and_then(quill_app::script::parse_token)
				{
					self.key(k);
				}
			}
		}
	}

	/// A line of the log, written to the file when it ends.
	struct Out {
		h: u32,
		buf: [u8; 160],
		n: usize,
	}

	impl Out {
		fn s(&mut self, t: &str) -> &mut Self {
			for b in t.bytes() {
				self.byte(b);
			}
			self
		}
		fn byte(&mut self, b: u8) {
			if self.n < self.buf.len() {
				self.buf[self.n] = b;
				self.n += 1;
			}
		}
		fn d(&mut self, mut v: u32) -> &mut Self {
			let mut t = [0u8; 10];
			let mut n = 0;
			loop {
				t[n] = b'0' + (v % 10) as u8;
				v /= 10;
				n += 1;
				if v == 0 {
					break;
				}
			}
			while n > 0 {
				n -= 1;
				self.byte(t[n]);
			}
			self
		}
		fn flush(&mut self) {
			self.byte(b'\r');
			self.byte(b'\n');
			unsafe { _lwrite(self.h, self.buf.as_ptr() as u32, self.n as u32) };
			self.n = 0;
		}
	}

	/// Runs the script when the timer fires; true when this was the bench's timer message.
	pub fn run(hwnd: u32) -> bool {
		let Some((text, len)) = (unsafe { (*core::ptr::addr_of_mut!(SCRIPT)).take() }) else {
			return false;
		};
		unsafe { KillTimer(hwnd, 1) };
		let Some(b) = FarBlock::alloc(BLOCK_LEN, true) else {
			unsafe { DestroyWindow(hwnd) };
			return true;
		};
		unsafe { *core::ptr::addr_of_mut!(BLOCK) = b };
		let (cw, ch) = crate::client_size(hwnd);
		let (bits, planes) = unsafe {
			let dc = GetDC(hwnd);
			let r = (GetDeviceCaps(dc, 12), GetDeviceCaps(dc, 14));
			ReleaseDC(hwnd, dc);
			(u32::from(r.0 as u16), u32::from(r.1 as u16))
		};
		let (page_w, hmax) = (
			crate::layout(cw, ch, false).page_w,
			quill_chrome::view::hscroll_max(app(), crate::layout(cw, ch, false).page_w),
		);
		let first_back = (sh().back_state, sh().back_w, sh().back_h);
		let first_tick = unsafe { GetTickCount() };
		unsafe { set_counter_selector(block().ptr_at(0) >> 16) };
		let mut run = Run {
			hwnd,
			phase: 0,
			phases: 1,
			keys: 0,
			first_tick,
		};
		// Everything counted so far (start-up) is not a key's.
		rebase();
		let mut pl = Play {
			text,
			len,
			at: 0,
			frames: 0,
			out: 0,
			stage: 0,
			staged: 0,
		};
		let mut w = [0u8; 24];
		loop {
			let n = pl.word(&mut w);
			if n == 0 {
				break;
			}
			let word = &w[..n];
			if word == b"@phase" {
				let m = pl.word(&mut w);
				if run.keys > 0 || run.phases > 1 {
					if run.phases >= MAX_PHASES {
						continue;
					}
					run.phase = run.phases;
					run.phases += 1;
				}
				let mut name = [b' '; NAME_LEN];
				for (d, s) in name.iter_mut().zip(w[..m].iter()) {
					*d = *s;
				}
				block().write(OFF_NAMES + (run.phase * NAME_LEN) as u32, &name);
			} else if word[0] == b'@' {
				if word == b"@open" || word == b"@copy" {
					pl.word(&mut w);
				}
			} else {
				run.word(word);
			}
		}
		unsafe { set_counter_selector(0) };
		finish(hwnd, &run, (cw, ch, bits, planes, page_w, hmax), first_back);
		unsafe { DestroyWindow(hwnd) };
		true
	}

	fn finish(
		_hwnd: u32,
		run: &Run,
		info: (i32, i32, u32, u32, i32, i32),
		first_back: (u8, i32, i32),
	) {
		let mut name = [0u8; 128];
		unsafe { crate::USE_HERE = true };
		c_name(b"BENCH.LOG", &mut name);
		let h = unsafe { _lcreat(name.as_ptr() as u32, 0) };
		if h == HFILE_ERROR {
			return;
		}
		let mut o = Out {
			h,
			buf: [0; 160],
			n: 0,
		};
		let b = block();
		let back = |s: u8| match s {
			crate::BACK_FULL => "full",
			crate::BACK_BAND => "band",
			_ => "none",
		};
		o.s("BENCH 1").flush();
		o.s("I imports ").d(COUNT_IMPORTS as u32);
		o.s(" client ").d(info.0 as u32).s("x").d(info.1 as u32);
		o.s(" bits ").d(info.2).s(" planes ").d(info.3);
		o.s(" page_w ")
			.d(info.4 as u32)
			.s(" hmax ")
			.d(info.5 as u32);
		o.flush();
		o.s("B first ").s(back(first_back.0)).s(" ");
		o.d(first_back.1 as u32).s("x").d(first_back.2 as u32);
		o.s(" worst ")
			.s(back(sh().back_worst))
			.s(" now ")
			.s(back(sh().back_state));
		o.s(" ").d(sh().back_w as u32).s("x").d(sh().back_h as u32);
		o.s(" moves ").d(w16::stack_moves()).flush();
		for p in 0..run.phases {
			let mut nm = [0u8; NAME_LEN];
			b.read(OFF_NAMES + (p * NAME_LEN) as u32, &mut nm);
			o.s("P ").d(p as u32).s(" ");
			for c in nm.iter().take_while(|c| **c != 0) {
				if *c != b' ' {
					o.byte(*c);
				}
			}
			o.flush();
		}
		for i in 0..run.keys {
			let mut rec = [0u32; REC_WORDS];
			let bytes = unsafe {
				core::slice::from_raw_parts_mut(rec.as_mut_ptr().cast::<u8>(), REC_BYTES)
			};
			b.read(OFF_REC + (i * REC_BYTES) as u32, bytes);
			o.s("K ").d(i as u32);
			for v in rec {
				o.s(" ").d(v);
			}
			o.flush();
		}
		for p in 0..run.phases {
			for kind in 0..2usize {
				let acc = OFF_ACC + ((p * 2 + kind) * 0x400) as u32;
				let mut c = [0u32; NCNT];
				for chunk in 0..4 {
					let mut part = [0u32; 64];
					b.read(acc + (chunk * 256) as u32, words(&mut part));
					c[chunk * 64..chunk * 64 + 64].copy_from_slice(&part);
				}
				o.s("F ").d(p as u32).s(" ").d(kind as u32);
				for (i, v) in c.iter().enumerate() {
					if *v == 0 || (i >= COUNT_IMPORTS && i < SH_BASE) {
						continue;
					}
					if o.n > 130 {
						o.flush();
						o.s("F ").d(p as u32).s(" ").d(kind as u32);
					}
					o.s(" ").d(i as u32).s("=").d(*v);
				}
				o.flush();
			}
		}
		o.s("E keys ").d(run.keys as u32).flush();
		unsafe { _lclose(h) };
	}
}

#[cfg(feature = "bench")]
pub use on::*;
