// The runtime pieces a program or module with no C library has to supply, and the module loader. The runtime is a macro, not code of
// this crate: each program and module expands it into its own crate, so the pieces sit in that crate's own code and a module never
// shares a function or a heap with the program (W9X-133).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The runtime a Quill Win32 program or module needs when it has no C library.
//!
//! `runtime!()` expands the stack probe, the 64-bit division helpers, `_fltused` and the panic handler into the calling crate;
//! `runtime!(heap)` adds the global allocator over the process heap; `runtime!(dll)` writes `_fltused` as an assembly label, which a
//! `cdylib` does not export. A module adds `dll_entry!()`. A program that loads modules uses the `loader` feature's [`module`].

#![no_std]

#[cfg(feature = "loader")]
pub mod module;
pub mod sys;

/// The exit code of a panic, in the program and in every module (W9X-133).
pub const PANIC_EXIT_CODE: u32 = 3;

/// The global allocator over the process heap: what `runtime!(heap)` adds. Needs the `heap` feature.
#[doc(hidden)]
#[macro_export]
macro_rules! __allocator {
	() => {
		struct ProcessHeap;

		// SAFETY: HeapAlloc and HeapFree are thread-safe and return 8-byte aligned blocks, which covers every type here.
		unsafe impl ::core::alloc::GlobalAlloc for ProcessHeap {
			unsafe fn alloc(&self, layout: ::core::alloc::Layout) -> *mut u8 {
				if layout.align() > 8 {
					return ::core::ptr::null_mut();
				}
				$crate::sys::HeapAlloc($crate::sys::GetProcessHeap(), 0, layout.size().max(1))
			}

			unsafe fn dealloc(&self, ptr: *mut u8, _layout: ::core::alloc::Layout) {
				$crate::sys::HeapFree($crate::sys::GetProcessHeap(), 0, ptr);
			}
		}

		#[global_allocator]
		static HEAP: ProcessHeap = ProcessHeap;
	};
}

/// `_fltused`: tells the MSVC linker that floating point is in use somewhere (compiler-builtins mentions it); nothing of ours uses
/// any. A `#[no_mangle]` static in a program, an assembly label in a DLL (a `cdylib` exports every `#[no_mangle]` item).
#[doc(hidden)]
#[macro_export]
macro_rules! __fltused {
	(static) => {
		#[no_mangle]
		pub static _fltused: i32 = 0;
	};
	(label) => {
		::core::arch::global_asm!(".data", ".globl __fltused", "__fltused:", ".long 0", ".text");
	};
}

/// The stack probe LLVM calls for frames over a page: touch each page in turn so the guard page moves down.
#[doc(hidden)]
#[macro_export]
macro_rules! __chkstk {
	() => {
		::core::arch::global_asm!(
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
	};
}

/// A stdcall wrapper for one of the 64-bit division helpers the MSVC ABI expects.
#[doc(hidden)]
#[macro_export]
macro_rules! __stdcall_wrapper {
	($name:literal, $f:path) => {
		::core::arch::global_asm!(
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

#[doc(hidden)]
#[macro_export]
macro_rules! __runtime {
	($fltused:tt; $($extra:tt)*) => {
		mod rt {
			use ::core::arch::global_asm;

			$($extra)*

			#[panic_handler]
			fn panic(_: &::core::panic::PanicInfo) -> ! {
				// SAFETY: ends the process.
				unsafe { $crate::sys::ExitProcess($crate::PANIC_EXIT_CODE) }
			}

			// The stack probe LLVM calls for frames over a page: touch each page in turn so the guard page moves down.
			$crate::__chkstk!();

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

			$crate::__stdcall_wrapper!("__aulldiv", udiv64);
			$crate::__stdcall_wrapper!("__aullrem", urem64);
			$crate::__stdcall_wrapper!("__alldiv", idiv64);
			$crate::__stdcall_wrapper!("__allrem", irem64);

			$crate::__fltused!($fltused);
		}
	};
}

/// Expands the runtime into the calling crate: the stack probe, the division helpers, `_fltused` and the panic handler (exit code 3).
/// `runtime!(heap)` adds the global allocator over the process heap (the `heap` feature); `runtime!(dll)` is for a `cdylib`.
#[macro_export]
macro_rules! runtime {
	() => {
		$crate::__runtime!(static;);
	};
	(heap) => {
		$crate::__runtime!(static; $crate::__allocator!(););
	};
	(dll) => {
		$crate::__runtime!(label;);
	};
}

/// Emits a DLL's entry point, `QuillDllEntry@12`: stores nothing, runs nothing under the loader lock, returns TRUE. Written in
/// assembly so that the `cdylib` does not export it. Link it with `/ENTRY:QuillDllEntry` (the module's `build.rs`).
#[macro_export]
macro_rules! dll_entry {
	() => {
		::core::arch::global_asm!(".globl _QuillDllEntry@12", "_QuillDllEntry@12:", "mov eax, 1", "ret 12");
	};
}

/// Reads a build id from the text of `QUILL_BUILD_ID`: up to eight hex digits, the top bit the dirty flag (`quill_modabi::BUILD_DIRTY`).
/// Missing or malformed text is a development build, dirty.
pub const fn parse_build_id(text: Option<&str>) -> u32 {
	const DIRTY: u32 = 0x8000_0000;
	let Some(text) = text else { return DIRTY };
	let bytes = text.as_bytes();
	if bytes.is_empty() || bytes.len() > 8 {
		return DIRTY;
	}
	let mut id = 0u32;
	let mut i = 0;
	while i < bytes.len() {
		let digit = match bytes[i] {
			b @ b'0'..=b'9' => b - b'0',
			b @ b'a'..=b'f' => b - b'a' + 10,
			b @ b'A'..=b'F' => b - b'A' + 10,
			_ => return DIRTY,
		};
		id = (id << 4) | digit as u32;
		i += 1;
	}
	id
}

/// The build id the calling crate was built with: `QUILL_BUILD_ID` in the environment of the build (`win9x/build.sh` sets one for the
/// program and every module), or a dirty id when there is none.
#[macro_export]
macro_rules! build_id {
	() => {
		$crate::parse_build_id(::core::option_env!("QUILL_BUILD_ID"))
	};
}

#[cfg(any(feature = "dll", feature = "loader"))]
pub use abi::policy_for;

#[cfg(any(feature = "dll", feature = "loader"))]
mod abi {
	use quill_modabi::{is_dirty, Policy};

	/// The policy a build with this id applies (W9X-135): release when the id is clean, development when its dirty bit is set.
	pub const fn policy_for(build: u32) -> Policy {
		if is_dirty(build) {
			Policy::Development
		} else {
			Policy::Release
		}
	}
}

/// What a module's exports do with the struct a caller hands in, before they read a field of it.
#[cfg(feature = "dll")]
pub mod dll {
	use quill_modabi::{Status, QS_CORRUPT};

	/// The struct of type `T` at the flat address `addr`, or `QS_CORRUPT` when the address is null or misaligned or the struct's own
	/// leading `u16` size is below `size_of::<T>()`. Only the two size bytes are read before the check, so a short struct is safe.
	///
	/// # Safety
	/// A non-null, aligned `addr` must be readable and writable for at least its own leading `u16` and, when that says so, for
	/// `size_of::<T>()` bytes.
	pub unsafe fn struct_at<'a, T>(addr: u32) -> Result<&'a mut T, Status> {
		let p = addr as usize;
		if p == 0 || !p.is_multiple_of(core::mem::align_of::<T>()) {
			return Err(QS_CORRUPT);
		}
		let declared = core::ptr::read(p as *const u16);
		if (declared as usize) < core::mem::size_of::<T>() {
			return Err(QS_CORRUPT);
		}
		Ok(&mut *(p as *mut T))
	}

	/// The same for a struct with no size field: only null and alignment are checked.
	///
	/// # Safety
	/// A non-null, aligned `addr` must be readable and writable for `size_of::<T>()` bytes.
	pub unsafe fn plain_at<'a, T>(addr: u32) -> Result<&'a mut T, Status> {
		let p = addr as usize;
		if p == 0 || !p.is_multiple_of(core::mem::align_of::<T>()) {
			return Err(QS_CORRUPT);
		}
		Ok(&mut *(p as *mut T))
	}
}
