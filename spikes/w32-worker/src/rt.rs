// The runtime pieces a program with no C library has to supply: the panic handler, the stack probe and the 64-bit division helpers
// (copied from QUILL32.EXE's rt.rs, without its allocator: the spike takes memory from HeapAlloc directly).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::arch::global_asm;

use crate::sys::ExitProcess;

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
	// SAFETY: ends the process.
	unsafe { ExitProcess(3) }
}

// The stack probe LLVM calls for frames over a page: touch each page in turn so the guard page moves down.
global_asm!(
	".globl __chkstk",
	"__chkstk:",
	"push ecx",
	"lea ecx, [esp + 8]",
	"2:",
	"cmp eax, 0x1000",
	"jb 3f",
	"sub ecx, 0x1000",
	"test [ecx], eax",
	"sub eax, 0x1000",
	"jmp 2b",
	"3:",
	"sub ecx, eax",
	"test [ecx], eax",
	"lea eax, [esp + 4]",
	"mov esp, ecx",
	"mov ecx, [eax - 4]",
	"push dword ptr [eax]",
	"ret",
);

// The 64-bit division helpers the MSVC ABI expects (\`_alldiv\` and friends): stdcall, arguments on the stack, result in EDX:EAX. Each wraps a
// Rust routine that divides by shifting and subtracting, since a division here would call the helper again.
fn udivmod(n: u64, d: u64) -> (u64, u64) {
	if d == 0 {
		return (0, n);
	}
	let (mut q, mut r) = (0u64, 0u64);
	let mut i = 64;
	while i > 0 {
		i -= 1;
		r = (r << 1) | ((n >> i) & 1);
		if r >= d {
			r -= d;
			q |= 1 << i;
		}
	}
	(q, r)
}

fn join(lo: u32, hi: u32) -> u64 {
	(u64::from(hi) << 32) | u64::from(lo)
}

extern "C" fn udiv64(al: u32, ah: u32, bl: u32, bh: u32) -> u64 {
	udivmod(join(al, ah), join(bl, bh)).0
}

extern "C" fn urem64(al: u32, ah: u32, bl: u32, bh: u32) -> u64 {
	udivmod(join(al, ah), join(bl, bh)).1
}

extern "C" fn idiv64(al: u32, ah: u32, bl: u32, bh: u32) -> u64 {
	let (a, b) = (join(al, ah) as i64, join(bl, bh) as i64);
	let q = udivmod(a.unsigned_abs(), b.unsigned_abs()).0;
	if (a < 0) != (b < 0) {
		q.wrapping_neg()
	} else {
		q
	}
}

extern "C" fn irem64(al: u32, ah: u32, bl: u32, bh: u32) -> u64 {
	let (a, b) = (join(al, ah) as i64, join(bl, bh) as i64);
	let r = udivmod(a.unsigned_abs(), b.unsigned_abs()).1;
	if a < 0 {
		r.wrapping_neg()
	} else {
		r
	}
}

macro_rules! stdcall_wrapper {
	($name:literal, $f:path) => {
		global_asm!(
			concat!(".globl ", $name),
			concat!($name, ":"),
			"push dword ptr [esp + 16]",
			"push dword ptr [esp + 16]",
			"push dword ptr [esp + 16]",
			"push dword ptr [esp + 16]",
			"call {f}",
			"add esp, 16",
			"ret 16",
			f = sym $f,
		);
	};
}

stdcall_wrapper!("__aulldiv", udiv64);
stdcall_wrapper!("__aullrem", urem64);
stdcall_wrapper!("__alldiv", idiv64);
stdcall_wrapper!("__allrem", irem64);

/// Tells the MSVC linker that floating point is in use somewhere (compiler-builtins mentions it); nothing of ours uses any.
#[no_mangle]
pub static _fltused: i32 = 0;
