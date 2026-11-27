// W32SPIKE.EXE: checkpoint A of the autosave plan. A `CreateThread` worker serializes an owned copy of a 1 MB document to a file while the
// main thread keeps taking "keystrokes" and painting; it posts `WM_APP` when done. Measures key-to-paint latency with and without the worker,
// the worker's throughput, 200 or more start and finish cycles with the output checked, and an exit while a worker runs.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Throwaway code, built apart from QUILL32.EXE (see docs/w32-worker-spike.md). Reads `SPIKE.CFG` from its own folder (key=value lines, see
//! `Cfg`), writes `SPIKE.LOG` beside it, and paints its whole window white while it runs, green when every check passed and red when one failed.
#![no_std]
#![no_main]

mod rt;
mod sys;
mod worker;

use core::ptr::{addr_of_mut, null, null_mut};
use core::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

use sys::*;
use worker::{Job, CANCELLED, OK};

const WM_JOB_DONE: u32 = WM_APP;
const WM_KEY: u32 = WM_APP + 1;
const WATCHDOG: usize = 7;

const BASE: u8 = 0;
const WORKER: u8 = 1;
const LOW: u8 = 2;
const INLINE: u8 = 3;
const STRESS: u8 = 4;
const EXIT: u8 = 5;
const COOP: u8 = 6;
const NAMES: [&[u8]; 7] = [b"base", b"worker", b"low", b"inline", b"stress", b"exit", b"coop"];

static HWND: AtomicUsize = AtomicUsize::new(0);
static TYPIST_ON: AtomicU32 = AtomicU32::new(0);
static TYPIST_QUIT: AtomicU32 = AtomicU32::new(0);
static TYPIST_MS: AtomicU32 = AtomicU32::new(100);
static POST_FAILS: AtomicU32 = AtomicU32::new(0);

struct Cfg {
	doc: usize,
	keys: u32,
	keyms: u32,
	cycles: u32,
	stress_doc: usize,
	exit_abrupt: bool,
	hold: bool,
	yield_every: u32,
	exit_slow_ms: u32,
	paint_rep: u32,
	idle_prio: bool,
	no_write: bool,
	write_chunk: u32,
	job_timeout_ms: u32,
	phases: [u8; 8],
	nphases: usize,
}

struct State {
	cfg: Cfg,
	dir: [u8; 260],
	dirlen: usize,
	freq: u64,
	hwnd: Handle,
	brush: [Handle; 3],
	colour: usize,
	doc: *mut u8,
	typed: u32,
	rows: [[u8; 60]; 16],
	rowlen: usize,
	pending: bool,
	stamp: u32,
	coalesced: u32,
	lat: [u32; 400],
	nlat: usize,
	// phase
	phase: usize,
	kind: u8,
	active: bool,
	draining: bool,
	keys_left: u32,
	inline_done: bool,
	// job
	job: *mut Job,
	coop: *mut Job,
	thread: Handle,
	job_start_tick: u32,
	done_seen: u32,
	seq: u32,
	stress_done: u32,
	exp_crc: u32,
	exp_len: u32,
	exp_in_crc: u32,
	// per-phase worker stats
	jobs: u32,
	bytes: u64,
	t_ser: u64,
	t_wr: u64,
	t_tot: u64,
	t_copy: u64,
	// totals
	fails: u32,
	total_jobs: u32,
	max_job_ms: u32,
	med: [u32; 8],
}

static mut STATE: core::mem::MaybeUninit<State> = core::mem::MaybeUninit::zeroed();
fn s() -> &'static mut State {
	// SAFETY: zero is a valid State (nulls, zeros, false), and only the main thread touches it.
	unsafe { &mut *addr_of_mut!(STATE).cast::<State>() }
}

// ---- the log ----

static mut LOG: [u8; 32768] = [0; 32768];
static mut LOGN: usize = 0;

fn put(t: &[u8]) {
	// SAFETY: main thread only.
	unsafe {
		let n = LOGN;
		let k = t.len().min(32768 - n);
		core::ptr::copy_nonoverlapping(t.as_ptr(), addr_of_mut!(LOG).cast::<u8>().add(n), k);
		LOGN = n + k;
	}
}
fn putn(mut v: u64) {
	let mut b = [0u8; 20];
	let mut i = 20;
	loop {
		i -= 1;
		b[i] = b'0' + (v % 10) as u8;
		v /= 10;
		if v == 0 {
			break;
		}
	}
	put(&b[i..]);
}
fn nl() {
	put(b"\r\n");
}
fn kv(k: &[u8], v: u64) {
	put(b" ");
	put(k);
	put(b"=");
	putn(v);
}
fn fail(msg: &[u8]) {
	s().fails += 1;
	put(b"FAIL ");
	put(msg);
	nl();
}

fn qpc() -> u64 {
	let mut t = 0u64;
	// SAFETY: writes one u64.
	unsafe { QueryPerformanceCounter(&mut t) };
	t
}
fn us(ticks: u64) -> u64 {
	ticks * 1_000_000 / s().freq.max(1)
}

fn write_log() {
	let st = s();
	let mut p = [0u8; 280];
	p[..st.dirlen].copy_from_slice(&st.dir[..st.dirlen]);
	let name = b"SPIKE.LOG\0";
	p[st.dirlen..st.dirlen + name.len()].copy_from_slice(name);
	// SAFETY: plain file calls on a buffer we own.
	unsafe {
		let f = CreateFileA(p.as_ptr(), 0x4000_0000, 0, null(), 2, 0, null_mut());
		if f as isize != -1 {
			let mut w = 0u32;
			WriteFile(f, addr_of_mut!(LOG).cast::<u8>(), LOGN as u32, &mut w, null_mut());
			CloseHandle(f);
		}
	}
}

// ---- the configuration ----

fn parse_num(v: &[u8]) -> u32 {
	let mut n = 0u32;
	for &c in v {
		if c.is_ascii_digit() {
			n = n.wrapping_mul(10).wrapping_add(u32::from(c - b'0'));
		}
	}
	n
}

fn load_cfg(st: &mut State) {
	st.cfg = Cfg {
		doc: 1 << 20,
		keys: 40,
		keyms: 150,
		cycles: 200,
		stress_doc: 65536,
		exit_abrupt: false,
		hold: false,
		yield_every: 0,
		exit_slow_ms: 10,
		paint_rep: 1,
		idle_prio: false,
		no_write: false,
		write_chunk: 0,
		job_timeout_ms: 120_000,
		phases: [BASE, WORKER, LOW, INLINE, STRESS, EXIT, 0, 0],
		nphases: 6,
	};
	let mut p = [0u8; 280];
	p[..st.dirlen].copy_from_slice(&st.dir[..st.dirlen]);
	p[st.dirlen..st.dirlen + 10].copy_from_slice(b"SPIKE.CFG\0");
	let mut buf = [0u8; 2048];
	let mut n = 0u32;
	// SAFETY: plain file calls on buffers we own.
	unsafe {
		let f = CreateFileA(p.as_ptr(), GENERIC_READ, 1, null(), OPEN_EXISTING, 0, null_mut());
		if f as isize == -1 {
			return;
		}
		ReadFile(f, buf.as_mut_ptr(), 2047, &mut n, null_mut());
		CloseHandle(f);
	}
	for line in buf[..n as usize].split(|&c| c == b'\n') {
		let line = line.strip_suffix(b"\r").unwrap_or(line);
		let Some(eq) = line.iter().position(|&c| c == b'=') else {
			continue;
		};
		let (k, v) = (&line[..eq], &line[eq + 1..]);
		match k {
			b"doc" => st.cfg.doc = parse_num(v) as usize,
			b"keys" => st.cfg.keys = parse_num(v),
			b"keyms" => st.cfg.keyms = parse_num(v),
			b"cycles" => st.cfg.cycles = parse_num(v),
			b"stressdoc" => st.cfg.stress_doc = parse_num(v) as usize,
			b"exit" => st.cfg.exit_abrupt = v == b"abrupt",
			b"hold" => st.cfg.hold = parse_num(v) != 0,
			b"idleprio" => st.cfg.idle_prio = parse_num(v) != 0,
			b"wchunk" => st.cfg.write_chunk = parse_num(v),
			b"nowrite" => st.cfg.no_write = parse_num(v) != 0,
			b"paintrep" => st.cfg.paint_rep = parse_num(v).max(1),
			b"exitslow" => st.cfg.exit_slow_ms = parse_num(v),
			b"yield" => st.cfg.yield_every = parse_num(v),
			b"jobtimeout" => st.cfg.job_timeout_ms = parse_num(v),
			b"phases" => {
				st.cfg.nphases = 0;
				for name in v.split(|&c| c == b',') {
					if let Some(i) = NAMES.iter().position(|n| *n == name) {
						if st.cfg.nphases < 8 {
							st.cfg.phases[st.cfg.nphases] = i as u8;
							st.cfg.nphases += 1;
						}
					}
				}
			}
			_ => {}
		}
	}
	st.cfg.stress_doc = st.cfg.stress_doc.min(st.cfg.doc);
}

// ---- the typist: stands in for the keyboard driver ----

unsafe extern "system" fn typist(_: *mut core::ffi::c_void) -> u32 {
	while TYPIST_QUIT.load(Ordering::SeqCst) == 0 {
		Sleep(TYPIST_MS.load(Ordering::SeqCst));
		if TYPIST_ON.load(Ordering::SeqCst) != 0 {
			let stamp = qpc() as u32;
			let hwnd = HWND.load(Ordering::SeqCst) as Handle;
			PostMessageA(hwnd, WM_KEY, 0, stamp as isize);
		}
	}
	0
}

// ---- jobs ----

unsafe extern "system" fn job_thread(p: *mut core::ffi::c_void) -> u32 {
	let job = p.cast::<Job>();
	worker::run(job);
	let seq = (*job).seq;
	let hwnd = HWND.load(Ordering::SeqCst) as Handle;
	if PostMessageA(hwnd, WM_JOB_DONE, seq as usize, job as isize) == 0 {
		POST_FAILS.fetch_add(1, Ordering::SeqCst);
	}
	0
}

/// Copies the first `len` bytes of the document into a fresh block, makes a job that owns the copy and starts a thread on it.
fn spawn_job(len: usize, verify: bool, prio: i32, slice_sleep: u32) -> bool {
	let st = s();
	// SAFETY: heap and thread calls on blocks this function creates; the job's input is handed to the worker, which frees it.
	unsafe {
		let heap = GetProcessHeap();
		let t0 = qpc();
		let copy = HeapAlloc(heap, 0, len);
		if copy.is_null() {
			fail(b"no memory for the snapshot");
			return false;
		}
		core::ptr::copy_nonoverlapping(st.doc, copy, len);
		st.t_copy += qpc() - t0;
		if verify {
			let c = core::slice::from_raw_parts(copy, len);
			let (crc, olen) = worker::expected(c);
			st.exp_crc = crc;
			st.exp_len = olen;
			st.exp_in_crc = !worker::crc_update(!0, c);
		}
		let job = HeapAlloc(heap, 8, core::mem::size_of::<Job>()).cast::<Job>();
		let done = CreateEventA(null(), 1, 0, null());
		if job.is_null() || done.is_null() {
			fail(b"no job record or event");
			return false;
		}
		st.seq += 1;
		let j = &mut *job;
		j.input = copy;
		j.len = len;
		j.done_event = done;
		j.yield_every = st.cfg.yield_every;
		j.slice_sleep = slice_sleep;
		j.no_write = u32::from(st.cfg.no_write);
		j.write_chunk = st.cfg.write_chunk;
		j.seq = st.seq;
		j.verify = u32::from(verify);
		j.path[..st.dirlen].copy_from_slice(&st.dir[..st.dirlen]);
		j.path[st.dirlen..st.dirlen + 11].copy_from_slice(b"SPIKE.TMP\0\0");
		let mut id = 0u32;
		let th = CreateThread(null(), 0, job_thread, job.cast(), 0, &mut id);
		if th.is_null() {
			fail(b"CreateThread failed");
			return false;
		}
		if prio != 0 {
			SetThreadPriority(th, prio);
		}
		st.job = job;
		st.thread = th;
		st.job_start_tick = GetTickCount();
		st.done_seen = 0;
		true
	}
}

/// The cooperative baseline: a job on the main thread, one slice each time the message queue is empty (what Quill's own idle job would do).
fn spawn_coop() {
	let st = s();
	// SAFETY: heap and event calls on blocks this function creates; `finish` frees the copy.
	unsafe {
		let heap = GetProcessHeap();
		let t0 = qpc();
		let copy = HeapAlloc(heap, 0, st.cfg.doc);
		let job = HeapAlloc(heap, 8, core::mem::size_of::<Job>()).cast::<Job>();
		let done = CreateEventA(null(), 1, 0, null());
		if copy.is_null() || job.is_null() || done.is_null() {
			fail(b"coop: no memory");
			return;
		}
		core::ptr::copy_nonoverlapping(st.doc, copy, st.cfg.doc);
		st.t_copy += qpc() - t0;
		let j = &mut *job;
		j.input = copy;
		j.len = st.cfg.doc;
		j.done_event = done;
		j.no_write = u32::from(st.cfg.no_write);
		j.write_chunk = st.cfg.write_chunk;
		j.path[..st.dirlen].copy_from_slice(&st.dir[..st.dirlen]);
		j.path[st.dirlen..st.dirlen + 11].copy_from_slice(b"SPIKE.TMP\0\0");
		st.coop = job;
		if !worker::begin(job) {
			coop_done();
		}
	}
}

/// One idle slice of the cooperative job; collects it when it ends and starts the next while the phase has keys left.
fn coop_step() {
	let st = s();
	// SAFETY: the job is ours and only this thread touches it.
	let more = unsafe { worker::step(st.coop) };
	if !more {
		coop_done();
		if st.active && st.kind == COOP {
			if st.keys_left > 0 {
				spawn_coop();
			} else {
				phase_ready();
			}
		}
	}
}

fn coop_done() {
	let st = s();
	// SAFETY: the job came from `spawn_coop`.
	unsafe {
		let job = st.coop;
		worker::finish(job);
		let j = &mut *job;
		if j.status.load(Ordering::SeqCst) == OK {
			st.jobs += 1;
			st.bytes += j.len as u64;
			st.t_ser += j.ticks_serialize;
			st.t_wr += j.ticks_write;
			st.t_tot += j.ticks_total;
		} else {
			fail(b"the cooperative job failed");
		}
		CloseHandle(j.done_event);
		HeapFree(GetProcessHeap(), 0, job.cast());
		st.coop = null_mut();
	}
}

fn job_prio(kind: u8) -> i32 {
	match kind {
		LOW if s().cfg.idle_prio => THREAD_PRIORITY_IDLE,
		LOW => THREAD_PRIORITY_LOWEST,
		_ => 0,
	}
}

fn job_done(job: *mut Job) {
	let st = s();
	// SAFETY: `job` came from the worker's own post, so it is ours again; its thread has finished or is about to.
	unsafe {
		if job != st.job {
			fail(b"completion for a job that is not the current one");
			return;
		}
		let j = &mut *job;
		if WaitForSingleObject(st.thread, 10_000) != WAIT_OBJECT_0 {
			fail(b"the worker thread did not end after posting");
		}
		CloseHandle(st.thread);
		CloseHandle(j.done_event);
		let status = j.status.load(Ordering::SeqCst);
		st.total_jobs += 1;
		let ms = (us(j.ticks_total) / 1000) as u32;
		st.max_job_ms = st.max_job_ms.max(ms);
		if status == OK {
			st.jobs += 1;
			st.bytes += j.len as u64;
			st.t_ser += j.ticks_serialize;
			st.t_wr += j.ticks_write;
			st.t_tot += j.ticks_total;
		} else if status != CANCELLED {
			put(b"FAIL the worker reported an I/O error:");
			kv(b"stage", u64::from(j.err_stage));
			kv(b"win32_error", u64::from(j.err_code));
			kv(b"job", u64::from(j.seq));
			nl();
			st.fails += 1;
		}
		if j.verify != 0 && status == OK {
			if j.out_crc != st.exp_crc || j.out_len != st.exp_len {
				fail(b"stress: the serialized bytes differ from the oracle");
			}
			if j.in_crc_start != st.exp_in_crc || j.in_crc_end != st.exp_in_crc {
				fail(b"stress: the owned copy changed under the worker");
			}
		}
		HeapFree(GetProcessHeap(), 0, job.cast());
		st.job = null_mut();
		st.thread = null_mut();
	}
}

// ---- phases ----

fn sorted_lat() -> ([u32; 400], usize) {
	let st = s();
	let mut a = st.lat;
	let n = st.nlat;
	for i in 1..n {
		let mut j = i;
		while j > 0 && a[j - 1] > a[j] {
			a.swap(j - 1, j);
			j -= 1;
		}
	}
	(a, n)
}

fn begin_phase() {
	let st = s();
	st.kind = st.cfg.phases[st.phase];
	st.nlat = 0;
	st.coalesced = 0;
	st.pending = false;
	st.keys_left = st.cfg.keys;
	st.inline_done = false;
	st.draining = false;
	st.jobs = 0;
	st.bytes = 0;
	st.t_ser = 0;
	st.t_wr = 0;
	st.t_tot = 0;
	st.t_copy = 0;
	st.stress_done = 0;
	st.active = true;
	put(b"BEGIN ");
	put(NAMES[usize::from(st.kind)]);
	nl();
	match st.kind {
		WORKER | LOW => {
			spawn_job(st.cfg.doc, false, job_prio(st.kind), 0);
		}
		COOP => spawn_coop(),
		STRESS => {
			spawn_job(st.cfg.stress_doc, true, 0, 0);
			// The original changes while the worker holds its copy.
			// SAFETY: the document block lives for the process.
			unsafe { *st.doc ^= 1 };
		}
		EXIT => {
			exit_phase();
			return;
		}
		_ => {}
	}
	TYPIST_ON.store(1, Ordering::SeqCst);
}

fn end_phase() {
	let st = s();
	TYPIST_ON.store(0, Ordering::SeqCst);
	st.active = false;
	put(b"PHASE ");
	put(NAMES[usize::from(st.kind)]);
	if st.kind != STRESS {
		let (a, n) = sorted_lat();
		let g = |i: usize| {
			if n == 0 {
				0
			} else {
				us(u64::from(a[i.min(n - 1)]))
			}
		};
		kv(b"keys", n as u64);
		kv(b"coalesced", u64::from(st.coalesced));
		kv(b"lat_min_us", g(0));
		kv(b"lat_med_us", g(n / 2));
		kv(b"lat_p95_us", g(n * 95 / 100));
		kv(b"lat_max_us", g(n.wrapping_sub(1)));
		if st.phase < 8 {
			st.med[st.phase] = g(n / 2) as u32;
		}
	}
	if st.jobs > 0 {
		let ms = us(st.t_tot) / 1000;
		kv(b"jobs", u64::from(st.jobs));
		kv(b"bytes", st.bytes);
		kv(b"job_ms_mean", ms / u64::from(st.jobs));
		kv(b"serialize_ms_mean", us(st.t_ser) / 1000 / u64::from(st.jobs));
		kv(b"write_ms_mean", us(st.t_wr) / 1000 / u64::from(st.jobs));
		kv(b"copy_us_mean", us(st.t_copy) / u64::from(st.jobs));
		// Input bytes per second, in KB (a 486 does a few hundred).
		kv(b"kb_per_s", (st.bytes * 1000 / 1024) / ms.max(1));
	}
	nl();
	st.phase += 1;
	if st.phase < st.cfg.nphases {
		begin_phase();
	} else {
		finish();
	}
}

fn phase_ready() {
	let st = s();
	if !st.active {
		return;
	}
	let done = match st.kind {
		BASE | INLINE => st.keys_left == 0,
		WORKER | LOW => st.keys_left == 0 && st.job.is_null(),
		COOP => st.keys_left == 0 && st.coop.is_null(),
		STRESS => st.stress_done >= st.cfg.cycles && st.job.is_null(),
		_ => false,
	};
	if done {
		end_phase();
	}
}

fn exit_phase() {
	let st = s();
	if !spawn_job(st.cfg.doc, false, 0, st.cfg.exit_slow_ms) {
		finish();
		return;
	}
	// SAFETY: sleeping lets the worker get going; the calls after it use the job and handles this thread made.
	unsafe {
		Sleep(250);
		if st.cfg.exit_abrupt {
			put(b"EXIT abrupt: ExitProcess with the worker running\r\n");
			finish_log();
			ExitProcess(0);
		}
		let t0 = qpc();
		(*st.job).cancel.store(1, Ordering::SeqCst);
		let r = WaitForSingleObject((*st.job).done_event, 10_000);
		let ms = us(qpc() - t0) / 1000;
		put(b"EXIT join:");
		kv(b"cancel_to_done_ms", ms);
		kv(b"status", u64::from((*st.job).status.load(Ordering::SeqCst)));
		nl();
		if (*st.job).status.load(Ordering::SeqCst) != CANCELLED {
			fail(b"exit: the worker was not running when cancelled, so the test did not exercise an exit with a running worker");
		}
		if r != WAIT_OBJECT_0 {
			fail(b"exit: the worker did not stop after cancel");
			finish();
			return;
		}
		// Collect it through the same path as any other completion, once its message arrives; here the thread is known to be ending.
		let job = st.job;
		WaitForSingleObject(st.thread, 10_000);
		job_done(job);
	}
	st.active = false;
	finish();
}

fn finish_log() {
	let st = s();
	put(b"SUMMARY");
	kv(b"jobs_total", u64::from(st.total_jobs));
	kv(b"max_job_ms", u64::from(st.max_job_ms));
	kv(b"post_fails", u64::from(POST_FAILS.load(Ordering::SeqCst)));
	kv(b"freq", st.freq);
	nl();
	if POST_FAILS.load(Ordering::SeqCst) != 0 {
		st.fails += 1;
		put(b"FAIL PostMessageA from the worker failed\r\n");
	}
	put(if st.fails == 0 {
		b"RESULT PASS\r\n" as &[u8]
	} else {
		b"RESULT FAIL\r\n"
	});
	write_log();
}

fn finish() {
	let st = s();
	TYPIST_ON.store(0, Ordering::SeqCst);
	finish_log();
	st.colour = if st.fails == 0 { 1 } else { 2 };
	// SAFETY: window calls on the main thread.
	unsafe {
		InvalidateRect(st.hwnd, null(), 1);
		if !st.cfg.hold {
			DestroyWindow(st.hwnd);
		}
	}
}

// ---- the window ----

fn on_key(stamp: u32) {
	let st = s();
	if !st.active {
		return;
	}
	st.typed = st.typed.wrapping_add(1);
	// The edit: a byte of the live document changes, and a letter joins the last row.
	// SAFETY: the document block has `cfg.doc` bytes.
	unsafe { *st.doc.add((st.typed as usize).wrapping_mul(7919) % st.cfg.doc) = b'a' + (st.typed % 26) as u8 };
	if st.rowlen == 60 {
		st.rows.copy_within(1.., 0);
		st.rows[15] = [b' '; 60];
		st.rowlen = 0;
	}
	st.rows[15][st.rowlen] = b'a' + (st.typed % 26) as u8;
	st.rowlen += 1;
	if st.pending {
		st.coalesced += 1;
	} else {
		st.pending = true;
		st.stamp = stamp;
	}
	// SAFETY: window call on the main thread.
	unsafe { InvalidateRect(st.hwnd, null(), 0) };
}

fn on_paint(hwnd: Handle) {
	let st = s();
	// SAFETY: GDI calls on the main thread with the paint context it just got.
	unsafe {
		let mut ps: PaintStruct = core::mem::zeroed();
		let dc = BeginPaint(hwnd, &mut ps);
		let r = Rect {
			left: 0,
			top: 0,
			right: 640,
			bottom: 480,
		};
		FillRect(dc, &r, st.brush[st.colour]);
		SetBkMode(dc, TRANSPARENT);
		// `paintrep` repeats the rows to make a paint cost what Quill's does (0.64 s for a page at 12,000 cycles).
		for _ in 0..st.cfg.paint_rep {
			for (i, row) in st.rows.iter().enumerate() {
				TextOutA(dc, 8, 8 + 16 * i as i32, row.as_ptr(), 60);
			}
		}
		EndPaint(hwnd, &ps);
	}
	if st.pending {
		let d = (qpc() as u32).wrapping_sub(st.stamp);
		if st.nlat < st.lat.len() {
			st.lat[st.nlat] = d;
			st.nlat += 1;
		}
		st.pending = false;
		if st.active {
			if st.kind == INLINE && !st.inline_done && st.keys_left == st.cfg.keys / 2 {
				st.inline_done = true;
				inline_block();
			}
			st.keys_left = st.keys_left.saturating_sub(1);
			phase_ready();
		}
	}
}

/// The cooperative baseline: the same serialization and write, done in the main thread in one go.
fn inline_block() {
	let st = s();
	let t0 = qpc();
	// SAFETY: heap and file calls on blocks this function owns; `run` frees the copy.
	unsafe {
		let heap = GetProcessHeap();
		let copy = HeapAlloc(heap, 0, st.cfg.doc);
		let job = HeapAlloc(heap, 8, core::mem::size_of::<Job>()).cast::<Job>();
		let done = CreateEventA(null(), 1, 0, null());
		if copy.is_null() || job.is_null() || done.is_null() {
			fail(b"inline: no memory");
			return;
		}
		core::ptr::copy_nonoverlapping(st.doc, copy, st.cfg.doc);
		let j = &mut *job;
		j.input = copy;
		j.len = st.cfg.doc;
		j.done_event = done;
		j.path[..st.dirlen].copy_from_slice(&st.dir[..st.dirlen]);
		j.path[st.dirlen..st.dirlen + 11].copy_from_slice(b"SPIKE.TMP\0\0");
		worker::run(job);
		CloseHandle(done);
		HeapFree(heap, 0, job.cast());
	}
	put(b"INLINE block");
	kv(b"us", us(qpc() - t0));
	nl();
}

unsafe extern "system" fn wnd_proc(hwnd: Handle, msg: u32, wparam: usize, lparam: isize) -> isize {
	let st = s();
	match msg {
		WM_KEY => {
			on_key(lparam as u32);
			0
		}
		WM_JOB_DONE => {
			let job = lparam as *mut Job;
			job_done(job);
			if st.active {
				match st.kind {
					WORKER | LOW => {
						if st.keys_left > 0 {
							spawn_job(st.cfg.doc, false, job_prio(st.kind), 0);
						} else {
							st.draining = true;
						}
					}
					STRESS => {
						st.stress_done += 1;
						if st.stress_done < st.cfg.cycles {
							spawn_job(st.cfg.stress_doc, true, 0, 0);
							// The original changes while the worker holds its copy.
							*st.doc.add(st.stress_done as usize % st.cfg.stress_doc) ^= 1;
						}
					}
					_ => {}
				}
				phase_ready();
			}
			0
		}
		WM_TIMER if wparam == WATCHDOG => {
			if !st.job.is_null() {
				let waited = GetTickCount().wrapping_sub(st.job_start_tick);
				if WaitForSingleObject((*st.job).done_event, 0) == WAIT_OBJECT_0 {
					st.done_seen += 1;
					if st.done_seen >= 6 {
						fail(b"WM_APP lost: the worker finished but no message arrived in 3 s");
						finish_log();
						ExitProcess(5);
					}
				} else if waited > st.cfg.job_timeout_ms {
					fail(b"HANG: a worker has run past the job timeout");
					finish_log();
					ExitProcess(4);
				}
			}
			0
		}
		WM_PAINT => {
			on_paint(hwnd);
			0
		}
		WM_ERASEBKGND => 1,
		WM_DESTROY => {
			PostQuitMessage(0);
			0
		}
		_ => DefWindowProcA(hwnd, msg, wparam, lparam),
	}
}

#[no_mangle]
pub extern "C" fn start() -> ! {
	let st = s();
	worker::crc_init();
	// SAFETY: start-up calls on buffers this function owns.
	unsafe {
		let n = GetModuleFileNameA(null_mut(), st.dir.as_mut_ptr(), 260) as usize;
		let mut k = n;
		while k > 0 && st.dir[k - 1] != b'\\' && st.dir[k - 1] != b'/' {
			k -= 1;
		}
		st.dirlen = k;
		load_cfg(st);
		QueryPerformanceFrequency(&mut st.freq);
		let heap = GetProcessHeap();
		st.doc = HeapAlloc(heap, 0, st.cfg.doc);
		if st.doc.is_null() {
			ExitProcess(2);
		}
		// A document: lower-case words, a paragraph break every 70 or so bytes, and now and then a brace, a backslash or a byte over 127.
		let mut x = 0x2545_F491u32;
		for i in 0..st.cfg.doc {
			x ^= x << 13;
			x ^= x >> 17;
			x ^= x << 5;
			let r = x >> 8;
			*st.doc.add(i) = match r % 97 {
				0 => b'\n',
				1 => b'{',
				2 => b'\\',
				3 => 0xE9,
				4..=17 => b' ',
				_ => b'a' + (r % 26) as u8,
			};
		}
		put(b"W32SPIKE");
		kv(b"doc", st.cfg.doc as u64);
		kv(b"keys", u64::from(st.cfg.keys));
		kv(b"keyms", u64::from(st.cfg.keyms));
		kv(b"cycles", u64::from(st.cfg.cycles));
		kv(b"stressdoc", st.cfg.stress_doc as u64);
		kv(b"freq", st.freq);
		nl();
		// The oracle's own cost on the main thread: what the same serialization costs inline, without the write.
		let t0 = qpc();
		let (crc, len) = worker::expected(core::slice::from_raw_parts(st.doc, st.cfg.doc));
		put(b"ORACLE serialize on the main thread, no write:");
		kv(b"us", us(qpc() - t0));
		kv(b"out_bytes", u64::from(len));
		kv(b"crc", u64::from(crc));
		nl();
		let inst = GetModuleHandleA(null());
		let class = b"W32Spike\0";
		let wc = WndClassA {
			style: 0,
			wnd_proc,
			cls_extra: 0,
			wnd_extra: 0,
			instance: inst,
			icon: null_mut(),
			cursor: null_mut(),
			background: null_mut(),
			menu_name: null(),
			class_name: class.as_ptr(),
		};
		RegisterClassA(&wc);
		st.brush = [
			CreateSolidBrush(0x00FF_FFFF),
			CreateSolidBrush(0x0000_C000),
			CreateSolidBrush(0x0000_00D0),
		];
		st.rows = [[b' '; 60]; 16];
		st.hwnd = CreateWindowExA(
			0,
			class.as_ptr(),
			b"W32SPIKE\0".as_ptr(),
			WS_POPUP | WS_VISIBLE,
			0,
			0,
			640,
			480,
			null_mut(),
			null_mut(),
			inst,
			null(),
		);
		if st.hwnd.is_null() {
			ExitProcess(2);
		}
		HWND.store(st.hwnd as usize, Ordering::SeqCst);
		TYPIST_MS.store(st.cfg.keyms, Ordering::SeqCst);
		let mut id = 0u32;
		let ty = CreateThread(null(), 0, typist, null_mut(), 0, &mut id);
		if !ty.is_null() {
			SetThreadPriority(ty, THREAD_PRIORITY_TIME_CRITICAL);
		} else {
			fail(b"CreateThread failed for the typist");
		}
		SetTimer(st.hwnd, WATCHDOG, 500, null());
		st.phase = 0;
		begin_phase();
		let mut m: Msg = core::mem::zeroed();
		// While a cooperative job is running the loop peeks, and takes a slice when the queue is empty; otherwise it sleeps in GetMessage.
		loop {
			if st.coop.is_null() {
				if GetMessageA(&mut m, null_mut(), 0, 0) <= 0 {
					break;
				}
			} else if PeekMessageA(&mut m, null_mut(), 0, 0, PM_REMOVE) == 0 {
				coop_step();
				continue;
			} else if m.message == WM_QUIT {
				break;
			}
			DispatchMessageA(&m);
		}
		TYPIST_QUIT.store(1, Ordering::SeqCst);
		if !ty.is_null() {
			WaitForSingleObject(ty, 2000);
		}
		ExitProcess(if st.fails == 0 { 0 } else { 1 })
	}
}
