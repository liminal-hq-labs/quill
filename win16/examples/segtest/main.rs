// T2: a program of three code segments that call each other and back, through arguments, 64-bit results, deep nesting and function
// pointers, and writes what happened to SEGTEST.LOG.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The functions named `s2_*` are placed in code segment 2 and the `s3_*` ones in segment 3 with `link_section`; the rest (this file's
// `w16_main` and the runtime) is segment 1. Every call between them crosses a segment, so it goes through a veneer, and every use of
// a function's address goes through a slot in the table that is at the same offset in each segment.
#![no_std]
#![no_main]
use core::hint::black_box;
use w16::*;

static mut PATH: [u8; 96] = [0; 96];

#[inline(never)]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.seg3.add")]
pub extern "C" fn s2_add(a: u32, b: u32) -> u32 {
	black_box(a).wrapping_add(black_box(b))
}

/// Calls back into segment 3, which calls back here: depth `n` of alternating segments.
#[inline(never)]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.seg3.ping")]
pub extern "C" fn s2_ping(n: u32, acc: u32) -> u32 {
	if n == 0 {
		return acc;
	}
	s3_pong(n - 1, s2_add(acc, n))
}

#[inline(never)]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.seg3.many")]
pub extern "C" fn s2_many(a: u32, b: u32, c: u32, d: u32, e: u32, f: u32, g: u32, h: u32) -> u32 {
	black_box(a)
		+ 2 * black_box(b)
		+ 3 * black_box(c)
		+ 4 * black_box(d)
		+ 5 * black_box(e)
		+ 6 * black_box(f)
		+ 7 * black_box(g)
		+ 8 * black_box(h)
}

#[inline(never)]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.seg3.ptr")]
pub extern "C" fn s2_via_table(i: u32, x: u32) -> u32 {
	let f = unsafe { TABLE[black_box(i) as usize % 3] };
	f(x)
}

#[inline(never)]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.seg4.pong")]
pub extern "C" fn s3_pong(n: u32, acc: u32) -> u32 {
	if n == 0 {
		return acc;
	}
	s2_ping(n - 1, s3_mul(acc, 3) as u32)
}

#[inline(never)]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.seg4.mul")]
pub extern "C" fn s3_mul(a: u32, b: u32) -> u64 {
	u64::from(black_box(a)) * u64::from(black_box(b)) % 1_000_003
}

#[inline(never)]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.seg4.ptr")]
pub extern "C" fn s3_via_table(i: u32, x: u32) -> u32 {
	let f = unsafe { TABLE[black_box(i) as usize % 3] };
	f(x)
}

#[inline(never)]
#[unsafe(no_mangle)]
pub extern "C" fn s1_inc(x: u32) -> u32 {
	black_box(x) + 1
}

#[inline(never)]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.seg3.dbl")]
pub extern "C" fn s2_dbl(x: u32) -> u32 {
	black_box(x) * 2
}

#[inline(never)]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.seg4.sq")]
pub extern "C" fn s3_sq(x: u32) -> u32 {
	black_box(x) * black_box(x)
}

// Function pointers to all three segments, in a static the linker fills with code addresses.
static mut TABLE: [extern "C" fn(u32) -> u32; 3] = [s1_inc, s2_dbl, s3_sq];

fn expect(log: &mut Log, ok: &mut bool, name: &str, got: u64, want: u64) {
	log.s(name).s(": ").dec(got as u32).s(if got == want {
		" ok"
	} else {
		" WRONG, wanted "
	});
	if got != want {
		log.dec(want as u32);
		*ok = false;
	}
	log.nl();
}

#[unsafe(no_mangle)]
pub extern "C" fn w16_main(hinst: u32, _hprev: u32, _show: u32) -> u32 {
	let log = log();
	log.s("Quill T2 code segments").nl();
	let mut ok = true;
	expect(
		log,
		&mut ok,
		"seg1 -> seg2 add",
		u64::from(s2_add(40, 2)),
		42,
	);
	expect(
		log,
		&mut ok,
		"seg1 -> seg3 u64 result",
		s3_mul(123_456, 789),
		123_456u64 * 789 % 1_000_003,
	);
	expect(
		log,
		&mut ok,
		"eight arguments through a veneer",
		u64::from(s2_many(1, 2, 3, 4, 5, 6, 7, 8)),
		204,
	);
	// Nesting across segments: 60 levels (a shadow stack of 256 holds them).
	let mut want = 0u32;
	let mut n = 60;
	let mut acc = 0u32;
	let mut ping = true;
	while n > 0 {
		if ping {
			acc = acc.wrapping_add(n);
		} else {
			acc = (u64::from(acc) * 3 % 1_000_003) as u32;
		}
		ping = !ping;
		n -= 1;
	}
	want = want.wrapping_add(acc);
	expect(
		log,
		&mut ok,
		"60 levels, seg2 and seg3 alternating",
		u64::from(s2_ping(60, 0)),
		u64::from(want),
	);
	// Function pointers, called from each segment.
	for i in 0..3u32 {
		let want = match i {
			0 => 11,
			1 => 20,
			_ => 100,
		};
		let from1 = unsafe { TABLE[i as usize](10) };
		expect(log, &mut ok, "pointer from seg1", u64::from(from1), want);
		expect(
			log,
			&mut ok,
			"pointer from seg2",
			u64::from(s2_via_table(i, 10)),
			want,
		);
		expect(
			log,
			&mut ok,
			"pointer from seg3",
			u64::from(s3_via_table(i, 10)),
			want,
		);
	}
	log.s(if ok { "RESULT: PASS" } else { "RESULT: FAIL" }).nl();
	unsafe {
		let p = &mut *core::ptr::addr_of_mut!(PATH);
		let n = GetModuleFileName(hinst, p.as_mut_ptr() as u32, 90) as usize;
		let mut end = n;
		while end > 0 && p[end - 1] != b'\\' {
			end -= 1;
		}
		let name = b"SEGTEST.LOG\0";
		let mut full = [0u8; 112];
		full[..end].copy_from_slice(&p[..end]);
		full[end..end + name.len()].copy_from_slice(name);
		write_file(full.as_ptr(), log.bytes());
		ExitWindows(0, 0);
	}
	0
}
