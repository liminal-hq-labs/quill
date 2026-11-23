// T1: the register and segment-register probes for calls that yield, run as two tasks.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The first copy of the program runs the probes with one set of patterns and starts a second copy, which yields with another set. Both
// write their findings to PROBE.LOG (the first) and CHILD.LOG (the second) in the directory the program runs from and on C:, and the
// first ends Windows when it is done so a harness can stop.
#![no_std]
#![no_main]
use w16::probe;
use w16::*;

export_proc!(ProbeDlgProc, probe_dlgproc);

const WM_TIMER: u32 = 0x0113;

extern "C" fn probe_dlgproc(hdlg: u32, msg: u32, _wparam: u32, _lparam: u32) -> u32 {
	unsafe {
		match msg {
			WM_INITDIALOG => {
				SetTimer(hdlg, 7, 120, 0);
				1
			}
			WM_TIMER => {
				KillTimer(hdlg, 7);
				EndDialog(hdlg, 1);
				1
			}
			_ => 0,
		}
	}
}

static mut PATH: [u8; 96] = [0; 96];
static mut CMD: [u8; 112] = [0; 112];

fn save(log: &mut Log, name: &[u8]) {
	// The program's own directory (a scratch drive in the harness), then C:\QUILL.
	unsafe {
		let p = &mut *core::ptr::addr_of_mut!(PATH);
		let n = GetModuleFileName(HINST, p.as_mut_ptr() as u32, 90) as usize;
		let mut end = n;
		while end > 0 && p[end - 1] != b'\\' {
			end -= 1;
		}
		let mut full = [0u8; 112];
		full[..end].copy_from_slice(&p[..end]);
		let mut i = end;
		for &b in name {
			full[i] = b;
			i += 1;
		}
		write_file(full.as_ptr(), log.bytes());
		dos_mkdir(b"C:\\QUILL\0".as_ptr());
		let mut c = *b"C:\\QUILL\\\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0";
		let mut j = 9;
		for &b in name {
			c[j] = b;
			j += 1;
		}
		write_file(c.as_ptr(), log.bytes());
	}
}

fn child(log: &mut Log) -> u32 {
	log.s("Quill T1 register probes: second task").nl();
	let (rounds, raw, stubbed) = probe::run_child(log, 11000);
	log.s("second task: ")
		.dec(rounds)
		.s(" rounds; raw calls changed registers ")
		.dec(raw)
		.s(" times, calls through the stubs ")
		.dec(stubbed)
		.s(" times")
		.nl();
	log.s(if stubbed == 0 {
		"CHILD: PASS"
	} else {
		"CHILD: FAIL"
	})
	.nl();
	save(log, b"CHILD.LOG");
	0
}

#[unsafe(no_mangle)]
pub extern "C" fn w16_main(_hinst: u32, hprev: u32, _show: u32) -> u32 {
	let log = log();
	unsafe { SetErrorMode(1) };
	if hprev != 0 {
		return child(log);
	}
	log.s("Quill T1 register probes (code16)").nl();
	let v = unsafe { GetVersion() };
	let flags = unsafe { GetWinFlags() };
	log.s("GetVersion=")
		.hex(v, 8)
		.s(" GetWinFlags=")
		.hex(flags, 8);
	if flags & 0x20 != 0 {
		log.s(" 386 Enhanced mode");
	}
	log.nl();
	let ds = data_selector() as u16;

	// Start the second task and give it time to begin.
	unsafe {
		let p = &mut *core::ptr::addr_of_mut!(PATH);
		let n = GetModuleFileName(HINST, p.as_mut_ptr() as u32, 90) as usize;
		let c = &mut *core::ptr::addr_of_mut!(CMD);
		c[..n].copy_from_slice(&p[..n]);
		c[n] = 0;
		let h = WinExec(c.as_ptr() as u32, 4);
		log.s("WinExec second task: ")
			.dec(h)
			.s(if h >= 32 { " started" } else { " FAILED" })
			.nl();
		let t0 = GetTickCount();
		while GetTickCount().wrapping_sub(t0) < 1500 {
			Yield();
		}
	}
	let hwnd = unsafe { FindWindow(far(data_selector(), cs!("Progman")), 0) };
	log.s("FindWindow Progman: ").hex(hwnd, 4).nl();

	let mut ok = true;
	probe::run_plain(log, "second task running", 0);
	save(log, b"PROBE.LOG");
	probe::run_plain(log, "second task running", ds);
	save(log, b"PROBE.LOG");
	probe::run_yielding(log, "second task running", 0, hwnd);
	save(log, b"PROBE.LOG");
	probe::run_yielding(log, "second task running", ds, hwnd);
	save(log, b"PROBE.LOG");
	for seg in [0u16, ds] {
		log.s("-- probes through the import stubs (second task running, FS=GS=")
			.hex(u32::from(seg), 4)
			.s(")")
			.nl();
		for n in 0..5 {
			log.s("about to run stub case ").dec(n).nl();
			save(log, b"PROBE.LOG");
			ok &= probe::stubbed_case(log, n, seg, hwnd);
		}
		save(log, b"PROBE.LOG");
	}
	log.s("raw calls are expected to lose registers; the stubbed calls decide the result")
		.nl();
	log.s("stub mismatches: ")
		.dec(unsafe { probe::MISMATCHES })
		.nl();
	log.s(if ok { "RESULT: PASS" } else { "RESULT: FAIL" }).nl();
	save(log, b"PROBE.LOG");
	unsafe {
		// Let the second task finish its rounds before Windows ends.
		let t0 = GetTickCount();
		while GetTickCount().wrapping_sub(t0) < 8000 {
			Yield();
		}
		ExitWindows(0, 0);
	}
	0
}
