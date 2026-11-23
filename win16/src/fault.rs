// A fault handler for debugging: registers a ToolHelp interrupt callback that records the registers and the stack at a General Protection
// Fault and writes them to FAULT.LOG, so a fault can be read instead of guessed at.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// `install()` calls `InterruptRegister` for the current task. ToolHelp then calls `FaultHandler` with the fault's frame on the stack. The
// handler is assembler: it saves every register, including the whole of each 32-bit one, copies the first 48 words of the stack it was
// entered on, switches to a private stack in the data segment (the entry stack may be another segment's) and calls `fault_report`, which
// formats the record and writes it. It then restores everything and returns zero, so the fault is passed on to the next handler and the
// system's own box still appears.
use core::arch::global_asm;

use crate::imports::*;
use crate::write_file;

/// The registers saved at entry: EAX, ECX, EDX, EBX, ESP (as pushed), EBP, ESI, EDI.
pub static mut REGS: [u32; 8] = [0; 8];
/// Segment registers at entry: DS, ES, FS, GS, SS, and the entry SP.
pub static mut SEGS: [u16; 6] = [0; 6];
/// The first 48 words of the stack the handler was entered on.
pub static mut FRAME: [u16; 48] = [0; 48];
/// How many times the handler ran.
pub static mut COUNT: u16 = 0;
static mut STACK: [u8; 1024] = [0; 1024];
static mut SAVED_SS_SP: [u16; 2] = [0; 2];

global_asm!(
	".code16",
	".section .text.fault,\"ax\"",
	".globl FaultHandler",
	".globl __w16exp_FaultHandler",
	"FaultHandler:",
	"__w16exp_FaultHandler:",
	// Save all registers in full before touching anything.
	".byte 0x66, 0x60",           // pushad: EAX, ECX, EDX, EBX, ESP, EBP, ESI, EDI (EDI at the lowest address)
	"push ds",
	"push es",
	"push fs",
	"push gs",
	"mov bp, sp",                 // BP addresses the entry stack from here on, 16-bit
	// DS is ours: a selector word the loader fills in.
	".byte 0xb8",
	".globl __w16sel_data_90",
	"__w16sel_data_90:",
	".word 0",
	"mov ds, ax",
	"mov es, ax",
	"cld",
	// The saved dwords: pushad's block is above the four segment words.
	"mov si, bp",
	"add si, 8",
	"mov di, offset {regs}",
	"mov cx, 16",                 // 8 dwords as 16 words
	"2:",
	"mov ax, ss:[si]",
	"mov [di], ax",
	"add si, 2",
	"add di, 2",
	"loop 2b",
	// The segment registers and the entry SP.
	"mov ax, ss:[bp+6]",          // DS as pushed first of the four (at the highest address)
	"mov [{segs}], ax",
	"mov ax, ss:[bp+4]",
	"mov [{segs}+2], ax",
	"mov ax, ss:[bp+2]",
	"mov [{segs}+4], ax",
	"mov ax, ss:[bp]",
	"mov [{segs}+6], ax",
	"mov [{segs}+8], ss",
	"mov [{segs}+10], bp",
	// The first 48 words of the entry stack, from just above the registers we pushed.
	"mov si, bp",
	"add si, 40",
	"mov di, offset {frame}",
	"mov cx, 48",
	"3:",
	"mov ax, ss:[si]",
	"mov [di], ax",
	"add si, 2",
	"add di, 2",
	"loop 3b",
	// Switch to the private stack, report, and come back.
	"mov [{saved}], ss",
	"mov [{saved}+2], sp",
	"mov ax, ds",
	"mov ss, ax",
	"mov sp, offset {stack} + 1024",
	"movzx esp, sp",
	".byte 0x66, 0xe8",
	".long {report} - . - 4",
	"mov ss, [{saved}]",
	"mov sp, [{saved}+2]",
	"pop gs",
	"pop fs",
	"pop es",
	"pop ds",
	".byte 0x66, 0x61",           // popad
	"xor ax, ax",                 // not handled: pass the fault on
	".byte 0xcb",
	regs = sym REGS,
	segs = sym SEGS,
	frame = sym FRAME,
	saved = sym SAVED_SS_SP,
	stack = sym STACK,
	report = sym fault_report,
);

unsafe extern "C" fn fault_report() {
	unsafe {
		COUNT += 1;
		let log = crate::log();
		log.s("Quill fault record").nl();
		let names = ["EAX", "ECX", "EDX", "EBX", "ESP", "EBP", "ESI", "EDI"];
		// pushad stores EDI first (lowest address), so the saved order is the reverse of the names.
		for (i, n) in names.iter().enumerate() {
			let v = REGS[7 - i];
			log.s(n)
				.s("=")
				.hex(v, 8)
				.s(if i % 4 == 3 { "\r\n" } else { " " });
		}
		let sg = SEGS;
		log.s("DS=")
			.hex(u32::from(sg[0]), 4)
			.s(" ES=")
			.hex(u32::from(sg[1]), 4)
			.s(" FS=")
			.hex(u32::from(sg[2]), 4)
			.s(" GS=")
			.hex(u32::from(sg[3]), 4)
			.nl();
		log.s("SS=")
			.hex(u32::from(sg[4]), 4)
			.s(" entry SP=")
			.hex(u32::from(sg[5]), 4)
			.nl();
		log.s("frame words from the entry stack:").nl();
		let fr = FRAME;
		for (i, w) in fr.iter().enumerate() {
			log.hex(u32::from(*w), 4)
				.s(if i % 8 == 7 { "\r\n" } else { " " });
		}
		write_file(b"FAULT.LOG\0".as_ptr(), log.bytes());
	}
}

/// Registers the handler for the current task. Returns true when ToolHelp accepted it.
pub fn install() -> bool {
	unsafe {
		let task = GetCurrentTask();
		unsafe extern "C" {
			fn FaultHandler();
		}
		InterruptRegister(task, FaultHandler as usize as u32) != 0
	}
}
