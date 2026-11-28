// COM1, owned by one thread: opened overlapped with a 64 KiB queue, read with a time-out so nothing is ever left pending (serial.sys cannot
// cancel a WaitCommEvent, WP-HOST-1), and written one call at a time, each waited for.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::ptr::{null, null_mut};

use quill_hostlink_guest::Port;

use crate::sys::*;

pub struct Com {
	handle: Handle,
	read_event: Handle,
	write_event: Handle,
	read_ov: Overlapped,
	read_pending: bool,
}

fn overlapped(event: Handle) -> Overlapped {
	Overlapped {
		internal: 0,
		internal_high: 0,
		offset: 0,
		offset_high: 0,
		event,
	}
}

impl Com {
	/// Opens COM1 or says `None` (no port, or another program has it): the helper then has nothing to do.
	///
	/// Nothing the page already said is discarded: the driver's receive queue may hold the page's `hello` from before this program started
	/// (the driver takes bytes in as soon as the guest starts it), and reading it answers the page at once. Only the helper's old output goes.
	pub fn open() -> Option<Com> {
		// SAFETY: the port is opened and configured on this thread; every struct passed is a local that outlives its call.
		unsafe {
			let handle = CreateFileA(
				b"\\\\.\\COM1\0".as_ptr(),
				GENERIC_READ | GENERIC_WRITE,
				0,
				null(),
				OPEN_EXISTING,
				FILE_FLAG_OVERLAPPED,
				null_mut(),
			);
			if handle == INVALID_HANDLE_VALUE {
				return None;
			}
			// The receive queue first: without it QEMU lost and corrupted bytes, and the queue holds 65,535 (WP-HOST-1).
			SetupComm(handle, 65_536, 65_536);
			PurgeComm(handle, PURGE_TX);
			let mut dcb: Dcb = core::mem::zeroed();
			dcb.length = 28;
			GetCommState(handle, &mut dcb);
			dcb.length = 28;
			dcb.baud = 115_200;
			dcb.byte_size = 8;
			dcb.parity = 0;
			dcb.stop_bits = 0;
			// Binary, DTR and RTS on, no flow control either way, no abort on error.
			dcb.flags = 1 | (1 << 4) | (1 << 12);
			SetCommState(handle, &dcb);
			// An overlapped read that completes with the first byte, or with nothing after 250 ms.
			let timeouts = CommTimeouts {
				read_interval: MAXDWORD,
				read_multiplier: MAXDWORD,
				read_constant: 250,
				write_multiplier: 0,
				write_constant: 0,
			};
			SetCommTimeouts(handle, &timeouts);
			let read_event = CreateEventA(null(), 1, 0, null());
			let write_event = CreateEventA(null(), 1, 0, null());
			if read_event.is_null() || write_event.is_null() {
				CloseHandle(handle);
				return None;
			}
			Some(Com {
				handle,
				read_event,
				write_event,
				read_ov: overlapped(read_event),
				read_pending: false,
			})
		}
	}

	/// The event that is signalled when the pending read completes.
	pub fn read_event(&self) -> Handle {
		self.read_event
	}

	/// Starts a read if none is pending. `Some(n)` when it finished at once with `n` bytes (possibly none), `None` when it is pending or the
	/// port failed (`failed` says which).
	pub fn start_read(&mut self, buf: &mut [u8]) -> Result<Option<u32>, ()> {
		if self.read_pending {
			return Ok(None);
		}
		// SAFETY: the buffer and the OVERLAPPED live in the caller's frame and in `self` until the read is collected or the port is closed
		// (`close` waits for it first).
		unsafe {
			self.read_ov = overlapped(self.read_event);
			let mut done = 0u32;
			let ok = ReadFile(self.handle, buf.as_mut_ptr(), buf.len() as u32, &mut done, &mut self.read_ov);
			if ok != 0 {
				return Ok(Some(done));
			}
			if GetLastError() == ERROR_IO_PENDING {
				self.read_pending = true;
				return Ok(None);
			}
		}
		Err(())
	}

	/// If the pending read has completed, takes its byte count.
	pub fn collect_read(&mut self) -> Result<Option<u32>, ()> {
		if !self.read_pending {
			return Ok(None);
		}
		// SAFETY: the same OVERLAPPED the read was started with.
		unsafe {
			if WaitForSingleObject(self.read_event, 0) != WAIT_OBJECT_0 {
				return Ok(None);
			}
			let mut done = 0u32;
			let ok = GetOverlappedResult(self.handle, &mut self.read_ov, &mut done, 0);
			self.read_pending = false;
			if ok == 0 {
				return Err(());
			}
			Ok(Some(done))
		}
	}

	/// Leaves nothing pending, and gives the port back.
	pub fn close(&mut self) {
		// SAFETY: waits for the read it started (it times out within 250 ms), then closes what it opened.
		unsafe {
			if self.read_pending {
				WaitForSingleObject(self.read_event, 1_000);
				self.read_pending = false;
			}
			PurgeComm(self.handle, PURGE_ALL);
			CloseHandle(self.handle);
			CloseHandle(self.read_event);
			CloseHandle(self.write_event);
		}
		self.handle = INVALID_HANDLE_VALUE;
	}
}

impl Port for Com {
	fn write(&mut self, bytes: &[u8]) -> bool {
		let mut at = 0usize;
		while at < bytes.len() {
			let part = bytes.get(at..).unwrap_or(&[]);
			let mut ov = overlapped(self.write_event);
			let mut done = 0u32;
			// SAFETY: the buffer and the OVERLAPPED outlive the call, which waits for the write to finish.
			let ok = unsafe {
				let started = WriteFile(self.handle, part.as_ptr(), part.len() as u32, &mut done, &mut ov);
				started != 0
					|| (GetLastError() == ERROR_IO_PENDING && GetOverlappedResult(self.handle, &mut ov, &mut done, 1) != 0)
			};
			if !ok || done == 0 {
				return false;
			}
			at += done as usize;
		}
		true
	}
}
