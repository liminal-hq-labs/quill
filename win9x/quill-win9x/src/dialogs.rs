// The native dialogs: About, Find and Replace, Fonts and Tabs from the dialog resources, and the common file dialogs.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The editor says which dialog it wants (`quill_app::Dialog`); these functions show it, read what the user chose and hand it back
// through the editor's own methods, so the state layer is the one the text-cell dialogs of the DOS build use (D86).
use alloc::string::String;
use alloc::vec::Vec;
use core::ptr::{null, null_mut};

use quill_app::{Command, FindAction};

use crate::chrome::{family_of, SIZES};
use crate::sys::*;
use crate::Shell;

const IDOK: i32 = 1;
const IDCANCEL: i32 = 2;
const WM_INITDIALOG: u32 = 0x0110;
const WM_COMMAND: u32 = 0x0111;
const BM_SETCHECK: u32 = 0xF1;
const CB_ADDSTRING: u32 = 0x0143;
const CB_GETCURSEL: u32 = 0x0147;
const CB_GETLBTEXT: u32 = 0x0148;
const CB_SETCURSEL: u32 = 0x014E;
const CB_FINDSTRINGEXACT: u32 = 0x0158;
const LB_ADDSTRING: u32 = 0x0180;
const LB_RESETCONTENT: u32 = 0x0184;
const LB_GETCURSEL: u32 = 0x0188;
const SW_HIDE: i32 = 0;
const SWP_NOSIZE_NOZORDER: u32 = 0x0005;
const BST_CHECKED: u32 = 1;

const DLG_ABOUT: usize = 1001;
const DLG_FIND: usize = 1002;
const DLG_FONTS: usize = 1003;
const DLG_TABS: usize = 1004;
const DLG_SPELLING: usize = 1005;

static mut SHELL: *mut Shell = null_mut();
static mut OWNER: Handle = null_mut();
static mut REPLACE: bool = false;

fn c(text: &str) -> Vec<u8> {
	let mut v = text.as_bytes().to_vec();
	v.push(0);
	v
}

/// Puts the dialog in the middle of the window that owns it.
unsafe fn centre(dlg: Handle) {
	let (mut o, mut d) = (
		Rect {
			left: 0,
			top: 0,
			right: 0,
			bottom: 0,
		},
		Rect {
			left: 0,
			top: 0,
			right: 0,
			bottom: 0,
		},
	);
	GetWindowRect(OWNER, &mut o);
	GetWindowRect(dlg, &mut d);
	let x = o.left + ((o.right - o.left) - (d.right - d.left)) / 2;
	let y = o.top + ((o.bottom - o.top) - (d.bottom - d.top)) / 2;
	SetWindowPos(dlg, null_mut(), x.max(0), y.max(0), 0, 0, SWP_NOSIZE_NOZORDER);
}

unsafe fn set_text(dlg: Handle, id: i32, text: &str) {
	SetDlgItemTextA(dlg, id, c(text).as_ptr());
}

unsafe fn get_text(dlg: Handle, id: i32) -> Vec<u8> {
	let mut buf = [0u8; 128];
	let n = GetDlgItemTextA(dlg, id, buf.as_mut_ptr(), 128) as usize;
	buf[..n.min(127)].to_vec()
}

unsafe fn shell() -> &'static mut Shell {
	&mut *SHELL
}

unsafe fn run(owner: Handle, sh: &mut Shell, id: usize, proc_: unsafe extern "system" fn(Handle, u32, usize, isize) -> isize) {
	SHELL = sh;
	OWNER = owner;
	DialogBoxParamA(sh.instance, id as *const u8, owner, proc_, 0);
}

// ---- About ---------------------------------------------------------------------------------------------------------------------

/// Looks a function up by name in a module that is already loaded, so nothing is imported (and nothing is loaded) on a system that
/// lacks it. `name` ends in a NUL.
unsafe fn lookup(module: &[u8], name: &[u8]) -> Option<*const u8> {
	let m = GetModuleHandleA(module.as_ptr());
	if m.is_null() {
		return None;
	}
	let p = GetProcAddress(m, name.as_ptr());
	if p.is_null() {
		None
	} else {
		Some(p)
	}
}

/// Sets (`on`) or clears the reason Windows Vista and later shows when something blocks a shutdown, and remembers which in
/// `sh.blocked`. `ShutdownBlockReasonCreate` and `ShutdownBlockReasonDestroy` are looked up in USER32 at run time, so nothing is
/// imported and Windows 95, 98, Me and NT 4, which lack them, start as before.
///
/// # Safety
/// `hwnd` must be the main window.
pub unsafe fn shutdown_block(hwnd: Handle, sh: &mut Shell, on: bool) {
	if on == sh.blocked {
		return;
	}
	sh.blocked = on;
	if on {
		if let Some(p) = lookup(b"user32.dll\0", b"ShutdownBlockReasonCreate\0") {
			// The reason is UTF-16, built on the stack.
			let mut w = [0u16; 32];
			for (d, s) in w.iter_mut().zip(b"Quill has unsaved changes") {
				*d = u16::from(*s);
			}
			let f: unsafe extern "system" fn(Handle, *const u16) -> i32 = core::mem::transmute(p);
			f(hwnd, w.as_ptr());
		}
	} else if let Some(p) = lookup(b"user32.dll\0", b"ShutdownBlockReasonDestroy\0") {
		let f: unsafe extern "system" fn(Handle) -> i32 = core::mem::transmute(p);
		f(hwnd);
	}
}

/// The temporary folder, ending in a backslash, written at the start of `out`; its length. `GetTempPathA` is looked up at run time.
///
/// # Safety
/// None beyond the call it makes.
pub unsafe fn temp_dir(out: &mut [u8]) -> Option<usize> {
	let p = lookup(b"kernel32.dll\0", b"GetTempPathA\0")?;
	let f: unsafe extern "system" fn(u32, *mut u8) -> u32 = core::mem::transmute(p);
	let n = f(out.len() as u32, out.as_mut_ptr()) as usize;
	(n > 0 && n < out.len()).then_some(n)
}

/// The About box's platform line: the product name from the platform id and version, and Wine's version when it is Wine.
///
/// `GetVersionEx` tells a program without a manifest that it runs on Windows 8 (6.2) from Windows 8.1 on, so on NT the real
/// version comes from `RtlGetVersion` in `NTDLL.DLL`, which is not subject to that. NT systems have `NTDLL.DLL` loaded in every
/// process; Windows 9x has no such module and keeps what `GetVersionEx` says, which is exact there.
unsafe fn platform_line() -> quill_app::platform::Line {
	use quill_app::platform::{platform_line, Os, PLATFORM_NT};
	let mut v = OsVersionInfo {
		size: 148,
		major: 0,
		minor: 0,
		build: 0,
		platform: 0,
		csd: [0; 256],
	};
	GetVersionExA(&mut v);
	if v.platform == PLATFORM_NT {
		if let Some(p) = lookup(b"ntdll.dll\0", b"RtlGetVersion\0") {
			// RtlGetVersion takes the wide record, whose five leading fields are the same; ours is larger, so it fits.
			let rtl: unsafe extern "system" fn(*mut OsVersionInfo) -> i32 = core::mem::transmute(p);
			let mut real = OsVersionInfo {
				size: 276,
				major: 0,
				minor: 0,
				build: 0,
				platform: 0,
				csd: [0; 256],
			};
			if rtl(&mut real) >= 0 && real.major != 0 {
				v = real;
			}
		}
	}
	// Windows 9x keeps the build in the low word of the build number.
	let build = if v.platform == PLATFORM_NT {
		v.build
	} else {
		v.build & 0xFFFF
	};
	// Wine exports its own version from NTDLL.DLL as a C string.
	let mut wine = [0u8; 24];
	let mut wine_len = 0;
	if let Some(p) = lookup(b"ntdll.dll\0", b"wine_get_version\0") {
		let get: unsafe extern "system" fn() -> *const u8 = core::mem::transmute(p);
		let s = get();
		while !s.is_null() && wine_len < wine.len() && *s.add(wine_len) != 0 {
			wine[wine_len] = *s.add(wine_len);
			wine_len += 1;
		}
	}
	// Another system may export an unrelated symbol under that name; only a version number (digits and dots) is taken.
	let wine_str = core::str::from_utf8(&wine[..wine_len])
		.ok()
		.filter(|w| !w.is_empty() && w.bytes().all(|b| b.is_ascii_digit() || b == b'.'));
	platform_line(&Os {
		platform: v.platform,
		major: v.major,
		minor: v.minor,
		build,
		wine: wine_str,
	})
}

/// The About box's memory line. `GlobalMemoryStatusEx` counts past 4 GB but does not exist on Windows 95, so it is looked up; the
/// older call is the fallback, and its 32-bit counts are exact on the machines that lack the newer one.
unsafe fn memory_line() -> quill_app::platform::Line {
	if let Some(p) = lookup(b"kernel32.dll\0", b"GlobalMemoryStatusEx\0") {
		let ex: unsafe extern "system" fn(*mut MemoryStatusEx) -> i32 = core::mem::transmute(p);
		let mut m = MemoryStatusEx {
			length: core::mem::size_of::<MemoryStatusEx>() as u32,
			load: 0,
			total_phys: 0,
			avail_phys: 0,
			total_page: 0,
			avail_page: 0,
			total_virtual: 0,
			avail_virtual: 0,
			avail_extended: 0,
		};
		if ex(&mut m) != 0 {
			return quill_app::platform::memory_line(m.avail_phys, m.total_phys);
		}
	}
	let mut m = MemoryStatus {
		length: 32,
		load: 0,
		total_phys: 0,
		avail_phys: 0,
		total_page: 0,
		avail_page: 0,
		total_virtual: 0,
		avail_virtual: 0,
	};
	GlobalMemoryStatus(&mut m);
	quill_app::platform::memory_line(u64::from(m.avail_phys), u64::from(m.total_phys))
}

unsafe extern "system" fn about_proc(dlg: Handle, msg: u32, wparam: usize, _lparam: isize) -> isize {
	match msg {
		WM_INITDIALOG => {
			set_text(dlg, 22, &alloc::format!("Version {}", quill_app::VERSION));
			let platform = platform_line();
			let memory = memory_line();
			set_text(
				dlg,
				25,
				if platform.as_str().is_empty() {
					"Windows (version unknown)"
				} else {
					platform.as_str()
				},
			);
			set_text(
				dlg,
				26,
				if memory.as_str().is_empty() {
					"Free memory: unknown"
				} else {
					memory.as_str()
				},
			);
			centre(dlg);
			1
		}
		WM_COMMAND if wparam & 0xFFFF == IDOK as usize || wparam & 0xFFFF == IDCANCEL as usize => {
			EndDialog(dlg, 1);
			1
		}
		_ => 0,
	}
}

/// Shows the About box.
///
/// # Safety
/// `owner` must be the main window.
pub unsafe fn about(owner: Handle, sh: &mut Shell) {
	run(owner, sh, DLG_ABOUT, about_proc);
}

// ---- Find and Replace ----------------------------------------------------------------------------------------------------------

unsafe extern "system" fn find_proc(dlg: Handle, msg: u32, wparam: usize, _lparam: isize) -> isize {
	match msg {
		WM_INITDIALOG => {
			let sh = shell();
			let (find, with, case, word) = sh.app.last_find();
			set_text(dlg, 10, &String::from_utf8_lossy(find));
			set_text(dlg, 11, &String::from_utf8_lossy(with));
			CheckDlgButton(dlg, 12, u32::from(case));
			CheckDlgButton(dlg, 13, u32::from(word));
			if !REPLACE {
				for id in [11, 31, 101, 102] {
					ShowWindow(GetDlgItem(dlg, id), SW_HIDE);
				}
				set_text(dlg, 0, "");
			}
			SetWindowTextA(dlg, if REPLACE { b"Replace\0".as_ptr() } else { b"Find\0".as_ptr() });
			centre(dlg);
			1
		}
		WM_COMMAND => {
			let id = (wparam & 0xFFFF) as i32;
			let action = match id {
				100 => FindAction::Next,
				101 => FindAction::Replace,
				102 => FindAction::ReplaceAll,
				IDCANCEL => {
					EndDialog(dlg, 0);
					return 1;
				}
				_ => return 0,
			};
			let sh = shell();
			let (find, with) = (get_text(dlg, 10), get_text(dlg, 11));
			let (case, word) = (
				IsDlgButtonChecked(dlg, 12) == BST_CHECKED,
				IsDlgButtonChecked(dlg, 13) == BST_CHECKED,
			);
			let found = sh.app.find_text(&mut sh.store, &find, &with, case, word, action);
			InvalidateRect(OWNER, null(), 0);
			if !found && !find.is_empty() {
				MessageBoxA(dlg, c("Quill cannot find that text.").as_ptr(), c("Quill").as_ptr(), 0x40);
			}
			if action == FindAction::ReplaceAll {
				let text = String::from_utf8_lossy(sh.app.message()).into_owned();
				if !text.is_empty() {
					MessageBoxA(dlg, c(&text).as_ptr(), c("Quill").as_ptr(), 0x40);
				}
			}
			1
		}
		_ => 0,
	}
}

/// Shows Find (or Replace) and stays until it is closed.
///
/// # Safety
/// `owner` must be the main window.
pub unsafe fn find(owner: Handle, sh: &mut Shell, replace: bool) {
	REPLACE = replace;
	run(owner, sh, DLG_FIND, find_proc);
}

// ---- Fonts ---------------------------------------------------------------------------------------------------------------------

unsafe extern "system" fn fonts_proc(dlg: Handle, msg: u32, wparam: usize, _lparam: isize) -> isize {
	match msg {
		WM_INITDIALOG => {
			let sh = shell();
			let cs = sh.app.char_state(&sh.store);
			for f in &sh.faces {
				let mut z = f.clone();
				z.push(0);
				SendDlgItemMessageA(dlg, 10, CB_ADDSTRING, 0, z.as_ptr() as isize);
			}
			let mut z = cs.face[..usize::from(cs.face_len)].to_vec();
			z.push(0);
			let at = SendDlgItemMessageA(dlg, 10, CB_FINDSTRINGEXACT, usize::MAX, z.as_ptr() as isize);
			SendDlgItemMessageA(dlg, 10, CB_SETCURSEL, at as usize, 0);
			for s in SIZES {
				SendDlgItemMessageA(dlg, 11, CB_ADDSTRING, 0, c(&alloc::format!("{s}")).as_ptr() as isize);
			}
			set_text(dlg, 11, &alloc::format!("{}", cs.size / 2));
			for (id, bit) in [(15, 1u16), (16, 2), (17, 4), (18, 8)] {
				CheckDlgButton(dlg, id, u32::from(cs.flags & bit != 0));
			}
			centre(dlg);
			1
		}
		WM_COMMAND => match (wparam & 0xFFFF) as i32 {
			IDOK => {
				let sh = shell();
				let at = SendDlgItemMessageA(dlg, 10, CB_GETCURSEL, 0, 0);
				let mut face = [0u8; 64];
				let n = if at >= 0 {
					SendDlgItemMessageA(dlg, 10, CB_GETLBTEXT, at as usize, face.as_mut_ptr() as isize).max(0) as usize
				} else {
					0
				};
				let size = core::str::from_utf8(&get_text(dlg, 11))
					.ok()
					.and_then(|t| t.trim().parse::<u16>().ok())
					.unwrap_or(12)
					.clamp(4, 144);
				let mut flags = 0u16;
				for (id, bit) in [(15, 1u16), (16, 2), (17, 4), (18, 8)] {
					if IsDlgButtonChecked(dlg, id) == BST_CHECKED {
						flags |= bit;
					}
				}
				if n > 0 {
					sh.app
						.set_font(&mut sh.store, &face[..n], family_of(&face[..n]), size * 2, flags);
				} else {
					let cs = sh.app.char_state(&sh.store);
					sh.app
						.set_font(&mut sh.store, &cs.face[..usize::from(cs.face_len)], 1, size * 2, flags);
				}
				EndDialog(dlg, 1);
				1
			}
			IDCANCEL => {
				EndDialog(dlg, 0);
				1
			}
			_ => 0,
		},
		_ => 0,
	}
}

/// Shows the Fonts dialog and applies it.
///
/// # Safety
/// `owner` must be the main window.
pub unsafe fn fonts(owner: Handle, sh: &mut Shell) {
	run(owner, sh, DLG_FONTS, fonts_proc);
}

// ---- Tabs ----------------------------------------------------------------------------------------------------------------------

fn parse_inches(t: &[u8]) -> Option<i32> {
	let text = core::str::from_utf8(t).ok()?.trim();
	let (whole, frac) = text.split_once('.').unwrap_or((text, ""));
	let w: i32 = if whole.is_empty() { 0 } else { whole.parse().ok()? };
	let mut f = 0i32;
	let mut scale = 100;
	for ch in frac.chars().take(2) {
		f += ch.to_digit(10)? as i32 * scale / 10;
		scale /= 10;
	}
	w.checked_mul(1440)?.checked_add(f * 1440 / 100)
}

unsafe fn tabs_list(dlg: Handle) {
	let sh = shell();
	SendDlgItemMessageA(dlg, 19, LB_RESETCONTENT, 0, 0);
	let mut stops = [(0, 0, 0); 16];
	let n = sh.app.tab_stops_full(&sh.store, &mut stops);
	for (pos, kind, leader) in stops.iter().take(n) {
		let line = alloc::format!(
			"{}.{:02} in  {}  {}",
			pos / 1440,
			(pos % 1440) * 100 / 1440,
			["Left", "Centre", "Right", "Decimal"][usize::from(*kind & 3)],
			["", "dots", "hyphens", "underline"][usize::from(*leader & 3)]
		);
		SendDlgItemMessageA(dlg, 19, LB_ADDSTRING, 0, c(&line).as_ptr() as isize);
	}
}

unsafe extern "system" fn tabs_proc(dlg: Handle, msg: u32, wparam: usize, _lparam: isize) -> isize {
	match msg {
		WM_INITDIALOG => {
			CheckRadioButton(dlg, 11, 14, 11);
			CheckRadioButton(dlg, 15, 18, 15);
			set_text(dlg, 10, "0.50");
			tabs_list(dlg);
			centre(dlg);
			1
		}
		WM_COMMAND => {
			let id = (wparam & 0xFFFF) as i32;
			let sh = shell();
			match id {
				20 | 21 => {
					let Some(pos) = parse_inches(&get_text(dlg, 10)) else {
						MessageBoxA(
							dlg,
							c("Type a position in inches, such as 1.25.").as_ptr(),
							c("Quill").as_ptr(),
							0x30,
						);
						return 1;
					};
					if id == 20 {
						let kind = (11..=14).position(|r| IsDlgButtonChecked(dlg, r) == BST_CHECKED).unwrap_or(0) as u8;
						let leader = (15..=18).position(|r| IsDlgButtonChecked(dlg, r) == BST_CHECKED).unwrap_or(0) as u8;
						sh.app.set_tab(&mut sh.store, pos, kind, leader);
					} else {
						sh.app.remove_tab(&mut sh.store, pos);
					}
					tabs_list(dlg);
					InvalidateRect(OWNER, null(), 0);
					1
				}
				22 => {
					sh.app.clear_tabs(&mut sh.store);
					tabs_list(dlg);
					InvalidateRect(OWNER, null(), 0);
					1
				}
				19 if wparam >> 16 == 1 => {
					// LBN_SELCHANGE: the chosen stop goes into the position box and the kind buttons.
					let at = SendDlgItemMessageA(dlg, 19, LB_GETCURSEL, 0, 0);
					let mut stops = [(0, 0, 0); 16];
					let n = sh.app.tab_stops_full(&sh.store, &mut stops);
					if at >= 0 && (at as usize) < n {
						let (pos, kind, leader) = stops[at as usize];
						set_text(dlg, 10, &alloc::format!("{}.{:02}", pos / 1440, (pos % 1440) * 100 / 1440));
						CheckRadioButton(dlg, 11, 14, 11 + i32::from(kind & 3));
						CheckRadioButton(dlg, 15, 18, 15 + i32::from(leader & 3));
					}
					1
				}
				IDCANCEL => {
					EndDialog(dlg, 0);
					1
				}
				_ => 0,
			}
		}
		_ => 0,
	}
}

/// Shows the Tabs dialog; each button acts at once, as in the text build.
///
/// # Safety
/// `owner` must be the main window.
pub unsafe fn tabs(owner: Handle, sh: &mut Shell) {
	run(owner, sh, DLG_TABS, tabs_proc);
}

// ---- the common dialogs --------------------------------------------------------------------------------------------------------

/// Asks whether to save the changes before `command` goes on: 6 yes, 7 no, 2 cancel.
///
/// # Safety
/// `owner` must be the main window.
pub unsafe fn confirm_save(owner: Handle, name: &[u8], command: Command) -> i32 {
	let what = match command {
		Command::Exit => "before Quill closes",
		Command::Open => "before another document opens",
		_ => "before a new document starts",
	};
	let shown = if name.is_empty() {
		String::from("(Untitled)")
	} else {
		String::from_utf8_lossy(name).into_owned()
	};
	// MB_YESNOCANCEL | MB_ICONQUESTION
	MessageBoxA(
		owner,
		c(&alloc::format!("Save the changes to {shown} {what}?")).as_ptr(),
		c("Quill").as_ptr(),
		0x23,
	)
}

// ---- Spelling ------------------------------------------------------------------------------------------------------------------

/// Shows the word the check stopped at, its sentence and its suggestions.
unsafe fn spelling_fill(dlg: Handle) {
	let sh = shell();
	set_text(dlg, 20, &String::from_utf8_lossy(sh.app.spell_word()));
	let mut ctx = [0u8; 72];
	let n = sh.app.spell_context(&sh.store, &mut ctx);
	set_text(dlg, 21, &String::from_utf8_lossy(&ctx[..n]));
	SendDlgItemMessageA(dlg, 11, LB_RESETCONTENT, 0, 0);
	let sg = sh.app.spell_suggestions(&sh.store);
	for i in 0..sg.len() {
		if let Some(c) = sg.get(i) {
			let mut z = c.bytes().to_vec();
			z.push(0);
			SendDlgItemMessageA(dlg, 11, LB_ADDSTRING, 0, z.as_ptr() as isize);
		}
	}
	// The first suggestion is the one Change takes; the edit box shows it and can be typed over.
	if sg.is_empty() {
		set_text(dlg, 10, "");
	} else {
		SendDlgItemMessageA(dlg, 11, 0x186, 0, 0); // LB_SETCURSEL
		set_text(dlg, 10, &String::from_utf8_lossy(sg.get(0).map_or(&[][..], |c| c.bytes())));
	}
	InvalidateRect(OWNER, null(), 0);
}

unsafe extern "system" fn spelling_proc(dlg: Handle, msg: u32, wparam: usize, _lparam: isize) -> isize {
	match msg {
		WM_INITDIALOG => {
			spelling_fill(dlg);
			centre(dlg);
			1
		}
		WM_COMMAND => {
			let id = (wparam & 0xFFFF) as i32;
			let sh = shell();
			let with = get_text(dlg, 10);
			let more = match id {
				100 => sh.app.spell_ignore(&mut sh.store, false),
				101 => sh.app.spell_ignore(&mut sh.store, true),
				102 | 103 if with.is_empty() => return 1,
				102 => sh.app.spell_change(&mut sh.store, &with, false),
				103 => sh.app.spell_change(&mut sh.store, &with, true),
				104 => sh.app.spell_add(&mut sh.store, &mut sh.files),
				11 if wparam >> 16 == 1 => {
					// LBN_SELCHANGE: the chosen suggestion goes into the box.
					let at = SendDlgItemMessageA(dlg, 11, LB_GETCURSEL, 0, 0);
					let mut buf = [0u8; 64];
					if at >= 0 {
						let n = SendDlgItemMessageA(dlg, 11, 0x189, at as usize, buf.as_mut_ptr() as isize).max(0) as usize; // LB_GETTEXT
						set_text(dlg, 10, &String::from_utf8_lossy(&buf[..n]));
					}
					return 1;
				}
				IDCANCEL => {
					sh.app.spell_cancel();
					EndDialog(dlg, 0);
					return 1;
				}
				_ => return 0,
			};
			if more {
				spelling_fill(dlg);
			} else {
				EndDialog(dlg, 1);
				MessageBoxA(
					OWNER,
					c("The spelling check is complete.").as_ptr(),
					c("Quill").as_ptr(),
					0x40,
				);
			}
			1
		}
		_ => 0,
	}
}

/// Runs the spelling check from the caret, a dialog per unknown word.
///
/// # Safety
/// `owner` must be the main window.
pub unsafe fn spelling(owner: Handle, sh: &mut Shell) {
	if sh.app.spell_begin(&mut sh.store, &mut sh.files) {
		run(owner, sh, DLG_SPELLING, spelling_proc);
	} else if sh.app.message().is_empty() {
		MessageBoxA(
			owner,
			c("The spelling check is complete.").as_ptr(),
			c("Quill").as_ptr(),
			0x40,
		);
	} else {
		let text = String::from_utf8_lossy(sh.app.message()).into_owned();
		MessageBoxA(owner, c(&text).as_ptr(), c("Quill").as_ptr(), 0x40);
	}
}
