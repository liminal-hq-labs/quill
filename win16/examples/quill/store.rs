// The Win16 `Store`: documents live in global memory, in blocks of at most 32 KB that are locked once and reached through far pointers.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// A moveable block in protected mode keeps its selector when Windows moves it, so the far pointer from the single `GlobalLock` stays
// good until the block is freed. The table of blocks lives in the data segment (160 entries of 8 bytes: 5 MB at 32 KB each, which holds a 4 MB picture, the most a document takes, and the rest of the document).
use core::cell::Cell;
use quill_core::store::{ChunkId, ReadStore, Store};
use quill_text::codepage::{CodePage, resource_offset};
use w16::{
	FindResource, GlobalAlloc, GlobalFree, GlobalLock, GlobalUnlock, LoadResource, LockResource,
	far, far_read, far_write, int_resource,
};

const CHUNKS: usize = 160;
const GMEM_MOVEABLE_ZEROINIT: u32 = 0x0042;

/// The code page tables of `quill-text` (`assets/codepages/CPAGES.BIN`), an `RCDATA` resource: the data segment has no room for
/// 1,536 bytes of tables (D200), and a resource is read through its own far pointer like a store chunk is.
const CPAGES_ID: u32 = 100;
const RT_RCDATA: u32 = 10;
/// `FarStore::cpages` before the first lookup, and after a lookup that found no resource.
const CP_UNLOADED: u32 = 0;
const CP_MISSING: u32 = 1;

/// For tracing: the last place ES was found different from DS, and the two selectors.
pub static mut BAD: [u16; 4] = [0; 4];

/// The stack pointer when the program started and the lowest one seen since (sampled at every store call, which are deep in the
/// drawing and layout code).
pub static mut TOP_SP: u16 = 0;
pub static mut MIN_SP: u16 = 0xFFFF;

/// What the first refusal by Windows looked like, for `PICS.LOG`: the blocks held then, the size asked, and the free space.
pub static mut REFUSED: (u16, u16, u32) = (0, 0, 0);
pub static mut REFUSALS: u16 = 0;

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

/// Notes the stack pointer, where the drawing code calls into GDI (the deepest frames that are not store calls).
pub fn sample() {
	let sp: u16;
	unsafe {
		core::arch::asm!("mov {0:x}, sp", out(reg) sp, options(nomem, nostack));
		if sp < MIN_SP {
			MIN_SP = sp;
		}
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
	/// The far pointer to the code page tables (after the resource's 4-byte length), once found.
	cpages: Cell<u32>,
}

impl FarStore {
	pub const fn new() -> Self {
		FarStore {
			handle: [0; CHUNKS],
			ptr: [0; CHUNKS],
			len: [0; CHUNKS],
			cpages: Cell::new(CP_UNLOADED),
		}
	}

	/// Finds, loads and locks the table resource. The lock is never released: the program needs it until it ends.
	fn load_cpages() -> u32 {
		unsafe {
			let h = FindResource(
				w16::HINST,
				far(0, int_resource(CPAGES_ID)),
				far(0, int_resource(RT_RCDATA)),
			);
			if h == 0 {
				return CP_MISSING;
			}
			let g = LoadResource(w16::HINST, h);
			if g == 0 {
				return CP_MISSING;
			}
			match LockResource(g) {
				0 => CP_MISSING,
				// The packer puts the resource's length in four bytes in front of its data.
				p => p + 4,
			}
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

impl ReadStore for FarStore {
	fn to_unicode(&self, cp: CodePage, byte: u8) -> char {
		if byte < 0x80 {
			return char::from(byte);
		}
		let mut base = self.cpages.get();
		if base == CP_UNLOADED {
			base = Self::load_cpages();
			self.cpages.set(base);
		}
		if base == CP_MISSING {
			// No table: Latin-1, which is right for the bytes the pages share with it and keeps the text readable.
			return char::from(byte);
		}
		let mut unit = [0u8; 2];
		unsafe {
			far_read(
				base + resource_offset(cp, byte) as u32,
				unit.as_mut_ptr(),
				2,
			)
		};
		char::from_u32(u32::from(u16::from_le_bytes(unit))).unwrap_or('\u{FFFD}')
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
}

impl Store for FarStore {
	fn alloc(&mut self, len: u16) -> Option<ChunkId> {
		let Some(i) = self.handle.iter().position(|h| *h == 0) else {
			unsafe {
				if REFUSALS == 0 {
					REFUSED = (CHUNKS as u16, len, 0xFFFF_FFFF);
				}
				REFUSALS = REFUSALS.saturating_add(1);
			}
			return None;
		};
		let h = unsafe { GlobalAlloc(GMEM_MOVEABLE_ZEROINIT, u32::from(len.max(1))) } as u16;
		if h == 0 {
			unsafe {
				if REFUSALS == 0 {
					let held = self.handle.iter().filter(|h| **h != 0).count() as u16;
					REFUSED = (held, len, w16::GetFreeSpace(0));
				}
				REFUSALS = REFUSALS.saturating_add(1);
			}
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
