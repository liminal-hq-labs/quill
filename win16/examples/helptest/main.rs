// Opens QUILL.HLP (next to the program) in the real WinHelp, jumps to a topic by its context hash, then waits so a harness can take screenshots.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// HELPCTX.TXT (or, without one, the contents) picks the call: `C` HELP_CONTENTS, `X <hash in hex>` HELP_CONTEXT.
#![no_std]
#![no_main]
use w16::*;

export_proc!(HelpWndProc, help_wndproc);

extern "C" fn help_wndproc(hwnd: u32, msg: u32, wparam: u32, lparam: u32) -> u32 {
	unsafe { DefWindowProc(hwnd, msg, wparam, lparam) }
}

const HELP_CONTEXT: u32 = 1;
const HELP_CONTENTS: u32 = 3;
const HELP_COMMAND: u32 = 0x0102;

#[unsafe(no_mangle)]
pub extern "C" fn w16_main(hinst: u32, hprev: u32, _show: u32) -> u32 {
	unsafe {
		// WinHelp is given a real window, as an application's main window would be.
		if hprev == 0 {
			let class = WndClass {
				style: 0,
				wndproc: code_selector() << 16 | HelpWndProc as usize as u32 & 0xFFFF,
				cls_extra: 0,
				wnd_extra: 0,
				instance: hinst as u16,
				icon: 0,
				cursor: LoadCursor(0, far(0, int_resource(32512))) as u16,
				background: 6,
				menu: 0,
				class_name: far(data_selector(), cs!("HelpTest")),
			};
			RegisterClass(&class as *const WndClass as u32);
		}
		let hwnd = CreateWindow(
			cs!("HelpTest") as u32,
			cs!("Help test") as u32,
			0x00CF0000,
			0,
			0,
			160,
			60,
			0,
			0,
			hinst,
			0,
		);
		ShowWindow(hwnd, 1);
		UpdateWindow(hwnd);
		let mut path = [0u8; 112];
		let n = GetModuleFileName(hinst, path.as_mut_ptr() as u32, 90) as usize;
		let mut end = n;
		while end > 0 && path[end - 1] != b'\\' {
			end -= 1;
		}
		let name = b"QUILL.HLP\0";
		path[end..end + name.len()].copy_from_slice(name);
		// HELPCTX.TXT next to the program, if there is one, holds `X` and a context hash in hex (the run= line takes no arguments).
		let mut cmd = [0u8; 80];
		let mut len = 0;
		let mut cp = path;
		let cn = b"HELPCTX.TXT\0";
		cp[end..end + cn.len()].copy_from_slice(cn);
		let f = _lopen(cp.as_ptr() as u32, 0);
		if f != HFILE_ERROR {
			len = _lread(f, cmd.as_mut_ptr() as u32, 79) as usize;
			_lclose(f);
		}
		let ok = if len >= 3 && cmd[0] == b'C' {
			// `C` and a WinHelp macro, e.g. JumpId("QUILL.HLP","keys").
			let mut m = [0u8; 64];
			let k = (len - 2).min(63);
			m[..k].copy_from_slice(&cmd[2..2 + k]);
			// Opens the file first (as an application does), then runs the macro.
			WinHelp(hwnd, path.as_ptr() as u32, HELP_CONTENTS, 0);
			let wait = GetTickCount() + 4000;
			let mut q = [0u16; 14];
			while GetTickCount() < wait {
				Yield();
				PeekMessage(q.as_mut_ptr() as u32, 0, 0, 0, 1);
			}
			WinHelp(
				0,
				path.as_ptr() as u32,
				HELP_COMMAND,
				far(data_selector(), m.as_ptr() as u32),
			)
		} else if len >= 3 && cmd[0] == b'Y' {
			// `Y` and a context number in hex: the file is open first, then the context is asked for.
			let mut h = 0u32;
			for &c in &cmd[2..len] {
				let d = match c {
					b'0'..=b'9' => c - b'0',
					b'A'..=b'F' => c - b'A' + 10,
					b'a'..=b'f' => c - b'a' + 10,
					_ => continue,
				};
				h = h << 4 | u32::from(d);
			}
			WinHelp(hwnd, path.as_ptr() as u32, HELP_CONTENTS, 0);
			let wait = GetTickCount() + 4000;
			let mut q = [0u16; 14];
			while GetTickCount() < wait {
				Yield();
				PeekMessage(q.as_mut_ptr() as u32, 0, 0, 0, 1);
			}
			WinHelp(hwnd, path.as_ptr() as u32, HELP_CONTEXT, h)
		} else if len >= 3 && cmd[0] == b'X' {
			let mut h = 0u32;
			for &c in &cmd[2..len] {
				let d = match c {
					b'0'..=b'9' => c - b'0',
					b'A'..=b'F' => c - b'A' + 10,
					b'a'..=b'f' => c - b'a' + 10,
					_ => continue,
				};
				h = h << 4 | u32::from(d);
			}
			WinHelp(hwnd, path.as_ptr() as u32, HELP_CONTEXT, h)
		} else {
			WinHelp(hwnd, path.as_ptr() as u32, HELP_CONTENTS, 0)
		};
		let mut log = [0u8; 64];
		let m = b"WinHelp returned ";
		log[..m.len()].copy_from_slice(m);
		log[m.len()] = b'0' + (ok & 1) as u8;
		log[m.len() + 1] = b' ';
		log[m.len() + 2] = b'0' + (len / 10 % 10) as u8;
		log[m.len() + 3] = b'0' + (len % 10) as u8;
		let mut lp = path;
		let ln = b"HELPTEST.LOG\0";
		lp[end..end + ln.len()].copy_from_slice(ln);
		write_file(lp.as_ptr(), &log[..m.len() + 4]);
		let mut msg = [0u16; 14];
		let until = GetTickCount() + 300_000;
		while GetTickCount() < until {
			Yield();
			PeekMessage(msg.as_mut_ptr() as u32, 0, 0, 0, 1);
		}
	}
	0
}
