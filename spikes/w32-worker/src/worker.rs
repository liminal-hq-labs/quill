// The worker module: serializes an owned copy of a document into a file, a slice at a time. It reaches kernel32 only (worker.allow lists
// every function it imports), so it could run as it is in a shipped build; the thread entry in main.rs adds the one USER32 call.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::ffi::c_void;
use core::sync::atomic::{AtomicU32, Ordering};

pub type Handle = *mut c_void;

#[link(name = "kernel32", kind = "raw-dylib", import_name_type = "undecorated")]
extern "system" {
	fn GetProcessHeap() -> Handle;
	fn HeapAlloc(heap: Handle, flags: u32, bytes: usize) -> *mut u8;
	fn HeapFree(heap: Handle, flags: u32, p: *mut u8) -> i32;
	fn CreateFileA(
		name: *const u8,
		access: u32,
		share: u32,
		security: *const c_void,
		disposition: u32,
		flags: u32,
		template: Handle,
	) -> Handle;
	fn WriteFile(file: Handle, buf: *const u8, len: u32, written: *mut u32, overlapped: *mut c_void) -> i32;
	fn CloseHandle(h: Handle) -> i32;
	fn DeleteFileA(name: *const u8) -> i32;
	fn SetEvent(event: Handle) -> i32;
	fn GetLastError() -> u32;
	fn Sleep(ms: u32);
	fn QueryPerformanceCounter(count: *mut u64) -> i32;
}

const GENERIC_WRITE: u32 = 0x4000_0000;
const CREATE_ALWAYS: u32 = 2;
const INVALID_HANDLE_VALUE: Handle = -1isize as Handle;

/// Input bytes handled between two looks at the cancel flag; the output of one slice is at most six times as long.
pub const SLICE: usize = 4096;
pub const OUT_SLICE: usize = SLICE * 6;

pub const OK: u32 = 1;
pub const CANCELLED: u32 = 2;
pub const IO_ERROR: u32 = 3;

/// One serialization job. The worker owns `input` (a copy the main thread made) and frees it; the main thread owns the job record and
/// frees it after the completion message and the thread's end.
#[repr(C)]
pub struct Job {
	pub input: *mut u8,
	pub len: usize,
	pub cancel: AtomicU32,
	pub status: AtomicU32,
	pub done_event: Handle,
	pub path: [u8; 64],
	/// Sleep(0) after this many slices (0 = never): a worker that gives its time slice back.
	pub yield_every: u32,
	/// Sleep(n) after every slice (0 = never): a slow disk, so that a job is still running when the test needs it to be.
	pub slice_sleep: u32,
	pub seq: u32,
	pub verify: u32,
	/// Skip the WriteFile calls (a diagnostic: does the file system's lock, not the CPU, slow the main thread?).
	pub no_write: u32,
	/// Gather this many output bytes before each WriteFile (0 = write every slice): fewer, larger calls into the file system.
	pub write_chunk: u32,
	/// Where an I/O error happened (1 no memory, 2 CreateFileA, 3 WriteFile), GetLastError there, and the bytes WriteFile reported.
	pub err_stage: u32,
	pub err_code: u32,
	pub err_aux: u32,
	// The state of a job in progress (private to begin, step and finish).
	pub file: Handle,
	pub out: *mut u8,
	pub pos: usize,
	pub crc: u32,
	pub total: u32,
	pub pending: usize,
	pub slices: u32,
	pub t_begin: u64,
	pub state: u32,
	// Results.
	pub out_len: u32,
	pub out_crc: u32,
	pub in_crc_start: u32,
	pub in_crc_end: u32,
	pub ticks_serialize: u64,
	pub ticks_write: u64,
	pub ticks_total: u64,
}

pub static mut CRC_TABLE: [u32; 256] = [0; 256];

pub fn crc_init() {
	let mut n = 0;
	while n < 256 {
		let mut c = n as u32;
		let mut k = 0;
		while k < 8 {
			c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
			k += 1;
		}
		// SAFETY: called once before any thread starts.
		unsafe { (*core::ptr::addr_of_mut!(CRC_TABLE))[n] = c };
		n += 1;
	}
}

pub fn crc_update(mut crc: u32, data: &[u8]) -> u32 {
	// SAFETY: the table is written once, before the first thread.
	let t = unsafe { &*core::ptr::addr_of!(CRC_TABLE) };
	for &b in data {
		crc = t[((crc ^ u32::from(b)) & 0xFF) as usize] ^ (crc >> 8);
	}
	crc
}

fn hex(n: u8) -> u8 {
	b"0123456789abcdef"[usize::from(n & 15)]
}

/// The document's bytes as an RTF-like body: a paragraph break, a brace or a backslash, or a byte outside printable ASCII is escaped. Returns
/// the bytes written to `out` (at most six for each input byte).
pub fn serialize_slice(inp: &[u8], out: &mut [u8]) -> usize {
	let mut o = 0;
	for &b in inp {
		match b {
			b'\n' => {
				out[o..o + 6].copy_from_slice(b"\\par\r\n");
				o += 6;
			}
			b'\\' | b'{' | b'}' => {
				out[o] = b'\\';
				out[o + 1] = b;
				o += 2;
			}
			0x20..=0x7E => {
				out[o] = b;
				o += 1;
			}
			_ => {
				out[o] = b'\\';
				out[o + 1] = b'\'';
				out[o + 2] = hex(b >> 4);
				out[o + 3] = hex(b);
				o += 4;
			}
		}
	}
	o
}

/// The CRC and length that serializing `inp` must give (the oracle: the main thread computes it from its own copy of the same bytes).
pub fn expected(inp: &[u8]) -> (u32, u32) {
	let mut buf = [0u8; OUT_SLICE];
	let (mut crc, mut len) = (!0u32, 0u32);
	for slice in inp.chunks(SLICE) {
		let n = serialize_slice(slice, &mut buf);
		crc = crc_update(crc, &buf[..n]);
		len += n as u32;
	}
	(!crc, len)
}

fn now() -> u64 {
	let mut t = 0u64;
	// SAFETY: writes one u64.
	unsafe { QueryPerformanceCounter(&mut t) };
	t
}

/// Starts a job: allocates the output block, creates the file and checksums the input when `verify` is set. Returns false when the job
/// has already failed (`finish` still has to be called).
///
/// # Safety
/// `job` must be valid, and `job.input` must point to `job.len` bytes from the process heap that nothing else uses.
pub unsafe fn begin(job: *mut Job) -> bool {
	let j = &mut *job;
	j.t_begin = now();
	j.state = OK;
	j.crc = !0;
	let heap = GetProcessHeap();
	j.out = HeapAlloc(heap, 0, OUT_SLICE + j.write_chunk as usize);
	j.file = CreateFileA(
		j.path.as_ptr(),
		GENERIC_WRITE,
		0,
		core::ptr::null(),
		CREATE_ALWAYS,
		0,
		core::ptr::null_mut(),
	);
	if j.out.is_null() || j.file == INVALID_HANDLE_VALUE {
		j.state = IO_ERROR;
		j.err_stage = if j.out.is_null() { 1 } else { 2 };
		j.err_code = GetLastError();
		return false;
	}
	if j.verify != 0 {
		j.in_crc_start = !crc_update(!0, core::slice::from_raw_parts(j.input, j.len));
	}
	true
}

/// Does one slice of a job: polls `cancel`, serializes up to `SLICE` input bytes and writes the output when a write is due. Returns true
/// while there is more to do. The worker thread calls it in a loop; the cooperative baseline calls it from the main thread between messages.
///
/// # Safety
/// As for `begin`, and only after `begin` returned true.
pub unsafe fn step(job: *mut Job) -> bool {
	let j = &mut *job;
	if j.cancel.load(Ordering::SeqCst) != 0 {
		j.state = CANCELLED;
		return false;
	}
	if j.pos < j.len {
		let end = (j.pos + SLICE).min(j.len);
		let slice = core::slice::from_raw_parts(j.input.add(j.pos), end - j.pos);
		let outbuf = core::slice::from_raw_parts_mut(j.out, OUT_SLICE + j.write_chunk as usize);
		let t0 = now();
		let n = serialize_slice(slice, &mut outbuf[j.pending..]);
		j.crc = crc_update(j.crc, &outbuf[j.pending..j.pending + n]);
		j.pending += n;
		j.total += n as u32;
		j.pos = end;
		j.slices += 1;
		let t1 = now();
		j.ticks_serialize += t1 - t0;
		if j.pending >= j.write_chunk as usize && !flush(j) {
			return false;
		}
		if j.yield_every != 0 && j.slices % j.yield_every == 0 {
			Sleep(0);
		}
		if j.slice_sleep != 0 {
			Sleep(j.slice_sleep);
		}
		return true;
	}
	// The end: write what is left.
	flush(j);
	false
}

unsafe fn flush(j: &mut Job) -> bool {
	if j.pending == 0 {
		return true;
	}
	let t0 = now();
	let mut written = 0u32;
	let ok = if j.no_write != 0 {
		written = j.pending as u32;
		1
	} else {
		WriteFile(j.file, j.out, j.pending as u32, &mut written, core::ptr::null_mut())
	};
	j.ticks_write += now() - t0;
	if ok == 0 || written as usize != j.pending {
		j.state = IO_ERROR;
		j.err_stage = 3;
		j.err_code = GetLastError();
		j.err_aux = written;
		return false;
	}
	j.pending = 0;
	true
}

/// Ends a job: closes the file (deleting it unless the job finished), frees the output block and the owned input, records the result and
/// signals `done_event`.
///
/// # Safety
/// As for `begin`.
pub unsafe fn finish(job: *mut Job) {
	let j = &mut *job;
	let heap = GetProcessHeap();
	if j.state == OK && j.verify != 0 {
		j.in_crc_end = !crc_update(!0, core::slice::from_raw_parts(j.input, j.len));
	}
	j.out_len = j.total;
	j.out_crc = !j.crc;
	if j.file != INVALID_HANDLE_VALUE && !j.file.is_null() {
		CloseHandle(j.file);
		if j.state != OK {
			DeleteFileA(j.path.as_ptr());
		}
	}
	if !j.out.is_null() {
		HeapFree(heap, 0, j.out);
	}
	HeapFree(heap, 0, j.input);
	j.input = core::ptr::null_mut();
	j.ticks_total = now() - j.t_begin;
	j.status.store(j.state, Ordering::SeqCst);
	SetEvent(j.done_event);
}

/// Runs a job to its end on the calling thread (the worker thread, or the main thread for the inline baseline).
///
/// # Safety
/// As for `begin`.
pub unsafe fn run(job: *mut Job) {
	if begin(job) {
		while step(job) {}
	}
	finish(job);
}
