// Register-preservation probes: load EBX, ESI, EDI, EBP, FS and GS with patterns, make one Windows call that may let another task run,
// and report what came back.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Rust code on this target keeps live values in the 32-bit registers, and a 16-bit callee need not preserve their high halves. The
// import stubs and exported-procedure wrappers save and restore them for that reason; these probes call the APIs raw, to find out whether
// the guest would have needed it. The calls that yield (GetMessage waiting on a timer, DialogBox, SendMessage to another task, Yield)
// are the ones where Windows switches tasks, and a second copy of the program runs the same kind of probes with other patterns, so a
// task switch that leaves one task's high halves in the other's registers shows up as a mismatch.
use core::arch::global_asm;

use crate::Log;

/// The patterns the first task loads: high halves non-zero, low halves distinct.
pub const PAT_A: [u32; 4] = [0x1111_BE01, 0x2222_BE02, 0x3333_BE03, 0x4444_BE04];
/// The patterns the second task loads.
pub const PAT_B: [u32; 4] = [0x5555_CE01, 0x6666_CE02, 0x7777_CE03, 0x8888_CE04];

/// Scratch message buffer (a 16-bit MSG is 18 bytes).
#[unsafe(no_mangle)]
pub static mut PROBE_MSG: [u8; 32] = [0; 32];
/// FS and GS as the probe loads them.
#[unsafe(no_mangle)]
pub static mut PROBE_SEG_IN: [u16; 2] = [0; 2];
/// FS and GS as they come back.
#[unsafe(no_mangle)]
pub static mut PROBE_SEG_OUT: [u32; 2] = [0; 2];
/// A window handle for the cross-task SendMessage probe.
#[unsafe(no_mangle)]
pub static mut PROBE_HWND: u16 = 0;
/// The instance handle, as a word the probes can push.
#[unsafe(no_mangle)]
pub static mut PROBE_HINST: u16 = 0;

macro_rules! probe {
	($name:ident, [$ebx:literal, $esi:literal, $edi:literal, $ebp:literal], $module:literal, $ordinal:literal, [$($line:literal),*]) => {
		global_asm!(
			".code16",
			concat!(".section .text.", stringify!($name), ",\"ax\""),
			concat!(".globl ", stringify!($name)),
			concat!(stringify!($name), ":"),
			"push ebx",
			"push esi",
			"push edi",
			"push ebp",
			"mov ax, word ptr [PROBE_SEG_IN]",
			"mov fs, ax",
			"mov ax, word ptr [PROBE_SEG_IN+2]",
			"mov gs, ax",
			concat!("mov ebx, ", $ebx),
			concat!("mov esi, ", $esi),
			concat!("mov edi, ", $edi),
			concat!("mov ebp, ", $ebp),
			$($line,)*
			".byte 0x9a",
			concat!(".globl __w16imp_", $module, "_", $ordinal, "_", stringify!($name)),
			concat!("__w16imp_", $module, "_", $ordinal, "_", stringify!($name), ":"),
			".word 0, 0",
			"mov eax, esp",
			"movzx esp, sp",
			"push ebp",
			"push edi",
			"push esi",
			"push ebx",
			"mov bp, sp",
			"mov di, [bp+36]",
			"mov edx, [bp]",
			"mov [di], edx",
			"mov edx, [bp+4]",
			"mov [di+4], edx",
			"mov edx, [bp+8]",
			"mov [di+8], edx",
			"mov edx, [bp+12]",
			"mov [di+12], edx",
			"mov [di+16], eax",
			"mov dx, fs",
			"movzx edx, dx",
			"mov dword ptr [PROBE_SEG_OUT], edx",
			"mov dx, gs",
			"movzx edx, dx",
			"mov dword ptr [PROBE_SEG_OUT+4], edx",
			"add sp, 16",
			"push ds",
			"pop es",
			"pop ebp",
			"pop edi",
			"pop esi",
			"pop ebx",
			".byte 0x66, 0xc3",
		);
	};
}

// The same probes, but through the import stubs the runtime uses, which save and restore EBX, ESI, EDI and EBP around the call. The
// arguments are pushed as 32-bit values, the last first, as the stubs expect; `$post` removes them.
macro_rules! stub_probe {
	($name:ident, [$ebx:literal, $esi:literal, $edi:literal, $ebp:literal], $stub:literal, [$($line:literal),*], $post:literal) => {
		global_asm!(
			".code16",
			concat!(".section .text.", stringify!($name), ",\"ax\""),
			concat!(".globl ", stringify!($name)),
			concat!(stringify!($name), ":"),
			"push ebx",
			"push esi",
			"push edi",
			"push ebp",
			"mov ax, word ptr [PROBE_SEG_IN]",
			"mov fs, ax",
			"mov ax, word ptr [PROBE_SEG_IN+2]",
			"mov gs, ax",
			concat!("mov ebx, ", $ebx),
			concat!("mov esi, ", $esi),
			concat!("mov edi, ", $edi),
			concat!("mov ebp, ", $ebp),
			$($line,)*
			".byte 0x66, 0xe8",
			concat!(".long ", $stub, " - . - 4"),
			$post,
			"mov eax, esp",
			"movzx esp, sp",
			"push ebp",
			"push edi",
			"push esi",
			"push ebx",
			"mov bp, sp",
			"mov di, [bp+36]",
			"mov edx, [bp]",
			"mov [di], edx",
			"mov edx, [bp+4]",
			"mov [di+4], edx",
			"mov edx, [bp+8]",
			"mov [di+8], edx",
			"mov edx, [bp+12]",
			"mov [di+12], edx",
			"mov [di+16], eax",
			"mov dx, fs",
			"movzx edx, dx",
			"mov dword ptr [PROBE_SEG_OUT], edx",
			"mov dx, gs",
			"movzx edx, dx",
			"mov dword ptr [PROBE_SEG_OUT+4], edx",
			"add sp, 16",
			"push ds",
			"pop es",
			"pop ebp",
			"pop edi",
			"pop esi",
			"pop ebx",
			".byte 0x66, 0xc3",
		);
	};
}

// A busy loop with no API call: only timer interrupts and the virtual machine manager can disturb the registers.
global_asm!(
	".code16",
	".section .text.probe_busy,\"ax\"",
	".globl probe_busy",
	"probe_busy:",
	"push ebx",
	"push esi",
	"push edi",
	"push ebp",
	"mov ax, word ptr [PROBE_SEG_IN]",
	"mov fs, ax",
	"mov ax, word ptr [PROBE_SEG_IN+2]",
	"mov gs, ax",
	"mov ebx, 0x1111BE01",
	"mov esi, 0x2222BE02",
	"mov edi, 0x3333BE03",
	"mov ebp, 0x4444BE04",
	"mov ecx, 12000000",
	"2:",
	"dec ecx",
	"jnz 2b",
	"mov eax, esp",
	"movzx esp, sp",
	"push ebp",
	"push edi",
	"push esi",
	"push ebx",
	"mov bp, sp",
	"mov di, [bp+36]",
	"mov edx, [bp]",
	"mov [di], edx",
	"mov edx, [bp+4]",
	"mov [di+4], edx",
	"mov edx, [bp+8]",
	"mov [di+8], edx",
	"mov edx, [bp+12]",
	"mov [di+12], edx",
	"mov [di+16], eax",
	"mov dx, fs",
	"movzx edx, dx",
	"mov dword ptr [PROBE_SEG_OUT], edx",
	"mov dx, gs",
	"movzx edx, dx",
	"mov dword ptr [PROBE_SEG_OUT+4], edx",
	"add sp, 16",
	"pop ebp",
	"pop edi",
	"pop esi",
	"pop ebx",
	".byte 0x66, 0xc3",
);

probe!(
	probe_getversion,
	["0x1111BE01", "0x2222BE02", "0x3333BE03", "0x4444BE04"],
	"KERNEL",
	"3",
	[]
);
probe!(
	probe_globalalloc,
	["0x1111BE01", "0x2222BE02", "0x3333BE03", "0x4444BE04"],
	"KERNEL",
	"15",
	["push 2", "push 0", "push 64"]
);
probe!(
	probe_yield,
	["0x1111BE01", "0x2222BE02", "0x3333BE03", "0x4444BE04"],
	"KERNEL",
	"29",
	[]
);
probe!(
	probe_peekmessage,
	["0x1111BE01", "0x2222BE02", "0x3333BE03", "0x4444BE04"],
	"USER",
	"109",
	[
		"push ds",
		"mov ax, offset PROBE_MSG",
		"push ax",
		"push 0",
		"push 0",
		"push 0",
		"push 1"
	]
);
probe!(
	probe_getmessage,
	["0x1111BE01", "0x2222BE02", "0x3333BE03", "0x4444BE04"],
	"USER",
	"108",
	[
		"push ds",
		"mov ax, offset PROBE_MSG",
		"push ax",
		"push 0",
		"push 0",
		"push 0"
	]
);
probe!(
	probe_sendmessage,
	["0x1111BE01", "0x2222BE02", "0x3333BE03", "0x4444BE04"],
	"USER",
	"111",
	[
		"mov ax, word ptr [PROBE_HWND]",
		"push ax",
		"push 0",
		"push 0",
		"push 0",
		"push 0"
	]
);
probe!(
	probe_dialogbox,
	["0x1111BE01", "0x2222BE02", "0x3333BE03", "0x4444BE04"],
	"USER",
	"87",
	[
		"mov ax, word ptr [PROBE_HINST]",
		"push ax",
		"push 0",
		"push 101",
		"push 0",
		"push cs",
		"mov ax, offset ProbeDlgProc",
		"push ax"
	]
);
stub_probe!(
	stub_yield,
	["0x1111BE01", "0x2222BE02", "0x3333BE03", "0x4444BE04"],
	"Yield",
	[],
	""
);
stub_probe!(
	stub_getmessage,
	["0x1111BE01", "0x2222BE02", "0x3333BE03", "0x4444BE04"],
	"GetMessage",
	[
		"xor eax, eax",
		"push eax",
		"xor eax, eax",
		"push eax",
		"xor eax, eax",
		"push eax",
		"mov ax, offset PROBE_MSG",
		"movzx eax, ax",
		"push eax"
	],
	"add sp, 16"
);
stub_probe!(
	stub_peekmessage,
	["0x1111BE01", "0x2222BE02", "0x3333BE03", "0x4444BE04"],
	"PeekMessage",
	[
		"mov eax, 1",
		"push eax",
		"xor eax, eax",
		"push eax",
		"xor eax, eax",
		"push eax",
		"xor eax, eax",
		"push eax",
		"mov ax, offset PROBE_MSG",
		"movzx eax, ax",
		"push eax"
	],
	"add sp, 20"
);
stub_probe!(
	stub_sendmessage,
	["0x1111BE01", "0x2222BE02", "0x3333BE03", "0x4444BE04"],
	"SendMessage",
	[
		"xor eax, eax",
		"push eax",
		"xor eax, eax",
		"push eax",
		"xor eax, eax",
		"push eax",
		"movzx eax, word ptr [PROBE_HWND]",
		"push eax"
	],
	"add sp, 16"
);
stub_probe!(
	stub_dialogbox,
	["0x1111BE01", "0x2222BE02", "0x3333BE03", "0x4444BE04"],
	"DialogBox",
	[
		"mov eax, offset ProbeDlgProc",
		"push eax",
		"xor eax, eax",
		"push eax",
		"mov eax, 101",
		"push eax",
		"movzx eax, word ptr [PROBE_HINST]",
		"push eax"
	],
	"add sp, 16"
);
stub_probe!(
	stub_yield_b,
	["0x5555CE01", "0x6666CE02", "0x7777CE03", "0x8888CE04"],
	"Yield",
	[],
	""
);
stub_probe!(
	stub_peekmessage_b,
	["0x5555CE01", "0x6666CE02", "0x7777CE03", "0x8888CE04"],
	"PeekMessage",
	[
		"mov eax, 1",
		"push eax",
		"xor eax, eax",
		"push eax",
		"push eax",
		"push eax",
		"mov ax, offset PROBE_MSG",
		"movzx eax, ax",
		"push eax"
	],
	"add sp, 20"
);
// The second task's probes, with other patterns.
probe!(
	probe_yield_b,
	["0x5555CE01", "0x6666CE02", "0x7777CE03", "0x8888CE04"],
	"KERNEL",
	"29",
	[]
);
probe!(
	probe_peekmessage_b,
	["0x5555CE01", "0x6666CE02", "0x7777CE03", "0x8888CE04"],
	"USER",
	"109",
	[
		"push ds",
		"mov ax, offset PROBE_MSG",
		"push ax",
		"push 0",
		"push 0",
		"push 0",
		"push 1"
	]
);

unsafe extern "C" {
	fn probe_getversion(out: *mut [u32; 5]);
	fn probe_globalalloc(out: *mut [u32; 5]);
	fn probe_yield(out: *mut [u32; 5]);
	fn probe_peekmessage(out: *mut [u32; 5]);
	fn probe_getmessage(out: *mut [u32; 5]);
	fn probe_sendmessage(out: *mut [u32; 5]);
	fn probe_dialogbox(out: *mut [u32; 5]);
	fn probe_busy(out: *mut [u32; 5]);
	fn stub_yield(out: *mut [u32; 5]);
	fn stub_yield_b(out: *mut [u32; 5]);
	fn stub_peekmessage_b(out: *mut [u32; 5]);
	fn stub_getmessage(out: *mut [u32; 5]);
	fn stub_peekmessage(out: *mut [u32; 5]);
	fn stub_sendmessage(out: *mut [u32; 5]);
	fn stub_dialogbox(out: *mut [u32; 5]);
	fn probe_yield_b(out: *mut [u32; 5]);
	fn probe_peekmessage_b(out: *mut [u32; 5]);
}

/// How many probes found a changed register since the log was started.
pub static mut MISMATCHES: u32 = 0;

fn report(log: &mut Log, name: &str, r: &[u32; 5], want: [u32; 4], seg: u32, count: bool) -> bool {
	let mut ok = true;
	log.s("probe ")
		.s(name)
		.s(": EBX=")
		.hex(r[0], 8)
		.s(" ESI=")
		.hex(r[1], 8)
		.s(" EDI=")
		.hex(r[2], 8)
		.s(" EBP=")
		.hex(r[3], 8)
		.s(" ESP=")
		.hex(r[4], 8);
	for i in 0..4 {
		if r[i] != want[i] {
			ok = false;
		}
	}
	let (fs, gs) = unsafe { (PROBE_SEG_OUT[0], PROBE_SEG_OUT[1]) };
	log.s(" FS=").hex(fs, 4).s(" GS=").hex(gs, 4);
	// FS and GS are reported but never decide the verdict: Rust code keeps nothing in them, and the far-memory helpers load one per call.
	if fs != seg || gs != seg {
		log.s(" (segment registers reloaded)");
	}
	log.s(if ok { " PRESERVED" } else { " CLOBBERED" }).nl();
	if !ok && count {
		unsafe { MISMATCHES += 1 };
	}
	ok
}

/// Sets the FS and GS values the probes load next.
pub fn set_segments(sel: u16) {
	unsafe { PROBE_SEG_IN = [sel, sel] };
}

/// The cases that do not yield. Returns true when every register survived.
pub fn run_plain(log: &mut Log, tag: &str, seg: u16) -> bool {
	let mut r = [0u32; 5];
	let mut all = true;
	set_segments(seg);
	log.s("-- plain probes (")
		.s(tag)
		.s(", FS=GS=")
		.hex(u32::from(seg), 4)
		.s(")")
		.nl();
	unsafe {
		probe_getversion(&mut r);
		all &= report(log, "GetVersion", &r, PAT_A, u32::from(seg), false);
		probe_globalalloc(&mut r);
		all &= report(log, "GlobalAlloc", &r, PAT_A, u32::from(seg), false);
		probe_peekmessage(&mut r);
		all &= report(log, "PeekMessage", &r, PAT_A, u32::from(seg), false);
		probe_busy(&mut r);
		all &= report(
			log,
			"busy loop under timer interrupts",
			&r,
			PAT_A,
			u32::from(seg),
			false,
		);
	}
	all
}

/// The cases that let another task run. `hwnd` is another task's window (Program Manager), or zero.
pub fn run_yielding(log: &mut Log, tag: &str, seg: u16, hwnd: u32) -> bool {
	let mut r = [0u32; 5];
	let mut all = true;
	set_segments(seg);
	unsafe {
		PROBE_HWND = hwnd as u16;
		PROBE_HINST = crate::HINST as u16;
	}
	log.s("-- yielding probes (")
		.s(tag)
		.s(", FS=GS=")
		.hex(u32::from(seg), 4)
		.s(")")
		.nl();
	unsafe {
		probe_yield(&mut r);
		all &= report(log, "Yield", &r, PAT_A, u32::from(seg), false);
		// GetMessage waits for the timer message, and other tasks run while it waits.
		let id = crate::SetTimer(0, 1, 60, 0);
		probe_getmessage(&mut r);
		all &= report(
			log,
			"GetMessage waiting on a timer",
			&r,
			PAT_A,
			u32::from(seg),
			false,
		);
		crate::KillTimer(0, id);
		if hwnd != 0 {
			probe_sendmessage(&mut r);
			all &= report(
				log,
				"SendMessage to another task's window",
				&r,
				PAT_A,
				u32::from(seg),
				false,
			);
		} else {
			log.s("probe SendMessage: skipped (no other window found)")
				.nl();
		}
		probe_dialogbox(&mut r);
		all &= report(
			log,
			"DialogBox (modal loop)",
			&r,
			PAT_A,
			u32::from(seg),
			false,
		);
	}
	all
}

/// One of the yielding cases through the runtime's import stubs (0 to 4). These are the calls the program relies on, and they must
/// all be PRESERVED even while another task is running.
pub fn stubbed_case(log: &mut Log, n: u32, seg: u16, hwnd: u32) -> bool {
	let mut r = [0u32; 5];
	set_segments(seg);
	unsafe {
		PROBE_HWND = hwnd as u16;
		PROBE_HINST = crate::HINST as u16;
	}
	let s = u32::from(seg);
	unsafe {
		if n == 0 {
			stub_yield(&mut r);
			report(log, "Yield via stub", &r, PAT_A, s, true)
		} else if n == 1 {
			stub_peekmessage(&mut r);
			report(log, "PeekMessage via stub", &r, PAT_A, s, true)
		} else if n == 2 {
			let id = crate::SetTimer(0, 1, 60, 0);
			stub_getmessage(&mut r);
			crate::KillTimer(0, id);
			report(log, "GetMessage via stub", &r, PAT_A, s, true)
		} else if n == 3 && hwnd != 0 {
			stub_sendmessage(&mut r);
			report(log, "SendMessage via stub", &r, PAT_A, s, true)
		} else if n == 4 {
			stub_dialogbox(&mut r);
			report(log, "DialogBox via stub", &r, PAT_A, s, true)
		} else {
			true
		}
	}
}

/// The second task: yields for `millis` milliseconds with its own patterns, alternating raw calls and calls through the stubs.
/// Returns (rounds, raw rounds that came back changed, stub rounds that came back changed).
pub fn run_child(log: &mut Log, millis: u32) -> (u32, u32, u32) {
	let mut r = [0u32; 5];
	let (mut raw_bad, mut stub_bad, mut rounds) = (0, 0, 0u32);
	set_segments(0);
	let t0 = unsafe { crate::GetTickCount() };
	while unsafe { crate::GetTickCount() }.wrapping_sub(t0) < millis {
		let kind = rounds % 4;
		unsafe {
			match kind {
				0 => probe_yield_b(&mut r),
				1 => probe_peekmessage_b(&mut r),
				2 => stub_yield_b(&mut r),
				_ => stub_peekmessage_b(&mut r),
			}
		}
		rounds += 1;
		let ok = r[0] == PAT_B[0] && r[1] == PAT_B[1] && r[2] == PAT_B[2] && r[3] == PAT_B[3];
		if !ok {
			if kind < 2 {
				raw_bad += 1;
				if raw_bad <= 4 {
					report(log, "child raw call", &r, PAT_B, 0, false);
				}
			} else {
				stub_bad += 1;
				if stub_bad <= 4 {
					report(log, "child stub call", &r, PAT_B, 0, true);
				}
			}
		}
	}
	(rounds, raw_bad, stub_bad)
}
