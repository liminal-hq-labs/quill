// Fills 512-byte stack buffers with a pattern, makes calls that yield and write files, and checks the buffers afterwards.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Reproduces the lost bytes Quill's play-back saw on Windows 3.11 (a stack buffer lost four bytes at fixed offsets). Writes
// STACKPRB.LOG next to the program: the runtime's stack selectors, the rounds run and, for each mismatch, the buffer's address, the
// offset and the value found. `--features noalias` builds the same program with Rust on the real 16-bit stack.
#![no_std]
#![no_main]
use w16::*;

const ROUNDS: u32 = 300;

fn pattern(round: u32, i: usize) -> u8 {
	(i as u32).wrapping_mul(7).wrapping_add(round) as u8 | 1
}

#[inline(never)]
fn one_round(round: u32, log: &mut Log, bad: &mut u32) {
	let mut a = [0u8; 512];
	let mut b = [0u8; 512];
	for i in 0..512 {
		a[i] = pattern(round, i);
		b[i] = pattern(round + 1, i);
	}
	let spin_until = unsafe { GetTickCount() } + 30;
	let mut msg = [0u16; 14];
	unsafe {
		// Yield and look at the queue across timer ticks.
		while GetTickCount() < spin_until {
			Yield();
			PeekMessage(msg.as_mut_ptr() as u32, 0, 0, 0, 1);
		}
		// A file write from a stack buffer, as the play-back does.
		let h = _lcreat(c"STACKPRB.TMP".as_ptr() as u32, 0);
		if h != HFILE_ERROR {
			_lwrite(h, a.as_ptr() as u32, 512);
			_lclose(h);
		}
	}
	for (name, buf, salt) in [("a", &a, 0u32), ("b", &b, 1u32)] {
		for i in 0..512 {
			if buf[i] != pattern(round + salt, i) {
				*bad += 1;
				if *bad <= 40 {
					log.s("round ")
						.dec(round)
						.s(" buf ")
						.s(name)
						.s(" at ")
						.hex(buf.as_ptr() as u32, 8)
						.s(" offset ")
						.dec(i as u32);
					log.s(" found ")
						.hex(u32::from(buf[i]), 2)
						.s(" want ")
						.hex(u32::from(pattern(round + salt, i)), 2)
						.nl();
				}
			}
		}
	}
}

#[unsafe(no_mangle)]
pub extern "C" fn w16_main(hinst: u32, _hprev: u32, _show: u32) -> u32 {
	let log = log();
	let (ss16, ss32) = stack_selectors();
	log.s("stackprobe ss16 ")
		.hex(ss16, 4)
		.s(" ss32 ")
		.hex(ss32, 4)
		.s(if ss16 == ss32 {
			" (no alias)"
		} else {
			" (alias)"
		})
		.nl();
	let mut bad = 0;
	for round in 0..ROUNDS {
		one_round(round, log, &mut bad);
	}
	log.s("rounds ").dec(ROUNDS).s(" bad bytes ").dec(bad).nl();
	unsafe {
		let mut path = [0u8; 112];
		let n = GetModuleFileName(hinst, path.as_mut_ptr() as u32, 90) as usize;
		let mut end = n;
		while end > 0 && path[end - 1] != b'\\' {
			end -= 1;
		}
		let name = b"STACKPRB.LOG\0";
		path[end..end + name.len()].copy_from_slice(name);
		write_file(path.as_ptr(), log.bytes());
		ExitWindows(0, 0);
	}
	0
}
