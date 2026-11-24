// A block of global memory held for the life of the program and reached through far pointers, for data that is too big for the
// 64 KB automatic data segment.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Rust addresses everything through DS, so a far block is never reached with an ordinary reference: bytes go in and out through
// `far_read` and `far_write` (which load FS or ES for the copy and leave DS alone), or the block's far pointer goes to Windows. A
// block over 64 KB is a chain of selectors `AHINCR` (8 in protected mode, KERNEL's equate 114) apart, which is what `ptr_at` steps.
use w16::{GlobalAlloc, GlobalFree, GlobalLock, GlobalUnlock, far_read, far_write};

const GMEM_MOVEABLE: u32 = 0x0002;
const GMEM_ZEROINIT: u32 = 0x0040;
/// The step between the selectors of a block over 64 KB.
const AHINCR: u32 = 8;
/// The most one `rep movsb` moves here: the count register is 16 bits, and the copy stays in one 64 KB segment.
const CHUNK: usize = 0x4000;

pub struct FarBlock {
	handle: u16,
	ptr: u32,
	len: u32,
}

impl FarBlock {
	pub const fn empty() -> Self {
		FarBlock {
			handle: 0,
			ptr: 0,
			len: 0,
		}
	}

	/// Allocates and locks `len` bytes of moveable global memory (cleared when `zero`). A moveable block keeps its selector when
	/// Windows moves it, so the far pointer stays good until `free`.
	pub fn alloc(len: u32, zero: bool) -> Option<Self> {
		let flags = GMEM_MOVEABLE | if zero { GMEM_ZEROINIT } else { 0 };
		let handle = unsafe { GlobalAlloc(flags, len.max(1)) } as u16;
		if handle == 0 {
			return None;
		}
		let ptr = unsafe { GlobalLock(u32::from(handle)) };
		if ptr == 0 {
			unsafe { GlobalFree(u32::from(handle)) };
			return None;
		}
		Some(FarBlock { handle, ptr, len })
	}

	pub fn len(&self) -> u32 {
		self.len
	}

	pub fn is_empty(&self) -> bool {
		self.handle == 0
	}

	/// The far pointer to byte `pos`, stepping to the next selector every 64 KB.
	pub fn ptr_at(&self, pos: u32) -> u32 {
		let sel = (self.ptr >> 16) + (pos >> 16) * AHINCR;
		sel << 16 | (self.ptr & 0xFFFF).wrapping_add(pos & 0xFFFF) & 0xFFFF
	}

	/// How many bytes from `pos` stay inside one selector.
	fn run(&self, pos: u32) -> usize {
		let in_seg = 0x1_0000 - ((self.ptr & 0xFFFF) + (pos & 0xFFFF));
		(in_seg as usize).min(CHUNK)
	}

	pub fn write(&self, mut pos: u32, mut data: &[u8]) {
		while !data.is_empty() && pos < self.len {
			let n = data.len().min(self.run(pos)).min((self.len - pos) as usize);
			if n == 0 {
				break;
			}
			unsafe { far_write(self.ptr_at(pos), data.as_ptr(), n) };
			data = data.get(n..).unwrap_or(&[]);
			pos += n as u32;
		}
	}

	pub fn read(&self, mut pos: u32, out: &mut [u8]) {
		let mut done = 0usize;
		while done < out.len() && pos < self.len {
			let n = (out.len() - done)
				.min(self.run(pos))
				.min((self.len - pos) as usize);
			if n == 0 {
				break;
			}
			if let Some(dst) = out.get_mut(done..) {
				unsafe { far_read(self.ptr_at(pos), dst.as_mut_ptr(), n) };
			}
			done += n;
			pos += n as u32;
		}
	}

	pub fn free(&mut self) {
		if self.handle != 0 {
			unsafe {
				GlobalUnlock(u32::from(self.handle));
				GlobalFree(u32::from(self.handle));
			}
			*self = FarBlock::empty();
		}
	}
}
