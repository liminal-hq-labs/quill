// The Win16 `Store`: documents live in global memory, in blocks of at most 32 KB that are locked once and reached through far pointers.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// A moveable block in protected mode keeps its selector when Windows moves it, so the far pointer from the single `GlobalLock` stays
// good until the block is freed. The table of blocks lives in the data segment (128 entries: 4 MB at 32 KB each).
use quill_core::store::{ChunkId, Store};
use w16::{GlobalAlloc, GlobalFree, GlobalLock, GlobalUnlock, far_read, far_write};

const CHUNKS: usize = 128;
const GMEM_MOVEABLE_ZEROINIT: u32 = 0x0042;

/// For tracing: the last place ES was found different from DS, and the two selectors.
pub static mut BAD: [u16; 4] = [0; 4];

/// The stack pointer when the program started and the lowest one seen since (sampled at every store call, which are deep in the
/// drawing and layout code).
pub static mut TOP_SP: u16 = 0;
pub static mut MIN_SP: u16 = 0xFFFF;

unsafe extern "C" {
	/// The end of the program's data (`link.ld`); the packer puts the shadow return stack right after it, then the stack.
	static __w16_data_end: u8;
}

/// The lowest address the stack may use: the data's end, the packer's shadow return stack (a 6-byte header and 256 four-byte
/// entries), and nothing else.
pub fn stack_floor() -> u16 {
	(core::ptr::addr_of!(__w16_data_end) as usize + 6 + 4 * 256) as u16
}

pub fn note_top() {
	let sp: u16;
	unsafe {
		core::arch::asm!("mov {0:x}, sp", out(reg) sp, options(nomem, nostack));
		TOP_SP = sp;
	}
}

fn check(tag: u16) {
	let sp: u16;
	unsafe { core::arch::asm!("mov {0:x}, sp", out(reg) sp, options(nomem, nostack)) };
	unsafe {
		if sp < MIN_SP {
			MIN_SP = sp;
		}
	}
	let (es, ds): (u16, u16);
	unsafe {
		core::arch::asm!("mov {0:x}, es", "mov {1:x}, ds", out(reg) es, out(reg) ds, options(nomem, nostack))
	};
	if es != ds {
		unsafe {
			BAD = [tag, es, ds, 0];
			crate::crumb(b"BAD ES");
			core::arch::asm!("push ds", "pop es", options(nomem, nostack));
		}
	}
}

pub struct FarStore {
	handle: [u16; CHUNKS],
	ptr: [u32; CHUNKS],
	len: [u16; CHUNKS],
}

impl FarStore {
	pub const fn new() -> Self {
		FarStore {
			handle: [0; CHUNKS],
			ptr: [0; CHUNKS],
			len: [0; CHUNKS],
		}
	}

	/// The far address of byte `offset` of chunk `id`, and how many bytes remain after it.
	fn at(&self, id: ChunkId, offset: u16) -> Option<(u32, usize)> {
		let i = usize::from(id.0).checked_sub(1)?;
		let len = *self.len.get(i)?;
		let ptr = *self.ptr.get(i)?;
		if ptr == 0 || offset > len {
			return None;
		}
		Some((ptr + u32::from(offset), usize::from(len - offset)))
	}
}

impl Store for FarStore {
	fn alloc(&mut self, len: u16) -> Option<ChunkId> {
		let i = self.handle.iter().position(|h| *h == 0)?;
		let h = unsafe { GlobalAlloc(GMEM_MOVEABLE_ZEROINIT, u32::from(len.max(1))) } as u16;
		if h == 0 {
			return None;
		}
		let p = unsafe { GlobalLock(u32::from(h)) };
		if p == 0 {
			unsafe { GlobalFree(u32::from(h)) };
			return None;
		}
		self.handle[i] = h;
		self.ptr[i] = p;
		self.len[i] = len;
		Some(ChunkId(i as u16 + 1))
	}

	fn free(&mut self, id: ChunkId) {
		let Some(i) = usize::from(id.0).checked_sub(1) else {
			return;
		};
		let Some(h) = self.handle.get(i).copied().filter(|h| *h != 0) else {
			return;
		};
		unsafe {
			GlobalUnlock(u32::from(h));
			GlobalFree(u32::from(h));
		}
		self.handle[i] = 0;
		self.ptr[i] = 0;
		self.len[i] = 0;
	}

	fn read(&self, id: ChunkId, offset: u16, out: &mut [u8]) {
		let have = match self.at(id, offset) {
			Some((p, left)) => {
				let n = left.min(out.len());
				// Far copies move at most 32 KB, which fits the count register.
				unsafe { far_read(p, out.as_mut_ptr(), n) };
				check(1);
				n
			}
			None => 0,
		};
		for b in out.iter_mut().skip(have) {
			*b = 0;
		}
	}

	fn write(&mut self, id: ChunkId, offset: u16, data: &[u8]) {
		if let Some((p, left)) = self.at(id, offset) {
			let n = left.min(data.len());
			check(2);
			unsafe { far_write(p, data.as_ptr(), n) };
			check(3);
		}
	}

	fn copy(&mut self, from: (ChunkId, u16), to: (ChunkId, u16), len: u16) {
		// Through a small buffer, in the direction that is safe when the ranges overlap within one chunk.
		let mut buf = [0u8; 128];
		let len = usize::from(len);
		let backwards = from.0 == to.0 && to.1 > from.1;
		let mut done = 0usize;
		while done < len {
			let n = (len - done).min(buf.len());
			let at = if backwards { len - done - n } else { done };
			let b = &mut buf[..n];
			self.read(from.0, from.1.wrapping_add(at as u16), b);
			self.write(to.0, to.1.wrapping_add(at as u16), b);
			done += n;
		}
	}
}
