// `memcpy` and `memset` with the 32-bit string instructions, for copying and filling a screen.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// `compiler_builtins` (the `compiler-builtins-mem` feature) supplies byte-at-a-time versions as weak symbols. These strong ones
// win at link time, so the portable crates need no `unsafe` to get a fast copy. Each moves whole dwords with `rep movsd` or
// `rep stosd`, then the 0 to 3 bytes left over with the byte forms (`memcpy` saves and restores `esi`, which LLVM may be using, and names every register it touches, because a free register chosen by the compiler could be `esi`). The direction flag is clear on entry (the ABI), and
// `memmove` and `memcmp` stay with `compiler_builtins`.
use core::arch::asm;
use core::ffi::c_void;

/// # Safety
/// The usual `memcpy` contract: `n` valid bytes at both pointers, not overlapping.
#[no_mangle]
pub unsafe extern "C" fn memcpy(dst: *mut c_void, src: *const c_void, n: usize) -> *mut c_void {
	asm!(
		"push esi",
		"mov esi, edx",
		"rep movsd",
		"mov ecx, eax",
		"rep movsb",
		"pop esi",
		inout("eax") n & 3 => _,
		inout("edx") src => _,
		inout("ecx") n >> 2 => _,
		inout("edi") dst => _,
		options(preserves_flags),
	);
	dst
}

/// # Safety
/// The usual `memset` contract: `n` valid bytes at `dst`.
#[no_mangle]
pub unsafe extern "C" fn memset(dst: *mut c_void, c: i32, n: usize) -> *mut c_void {
	let byte = c as u8 as u32;
	asm!(
		"rep stosd",
		"mov ecx, edx",
		"rep stosb",
		in("edx") n & 3,
		inout("ecx") n >> 2 => _,
		inout("edi") dst => _,
		in("eax") byte.wrapping_mul(0x0101_0101),
		options(preserves_flags, nostack),
	);
	dst
}
