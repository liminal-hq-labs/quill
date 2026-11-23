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

unsafe extern "system" fn about_proc(dlg: Handle, msg: u32, wparam: usize, _lparam: isize) -> isize {
	match msg {
		WM_INITDIALOG => {
			let sh = shell();
			set_text(dlg, 22, &alloc::format!("Version {}", quill_app::VERSION));
			let v = GetVersion();
			let (major, minor) = (v & 0xFF, (v >> 8) & 0xFF);
			set_text(
				dlg,
				25,
				&alloc::format!("{} on Windows {major}.{minor:02}", sh.app.about_platform()),
			);
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
			set_text(
				dlg,
				26,
				&alloc::format!("Free memory: {} KB of {} KB", m.avail_phys / 1024, m.total_phys / 1024),
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
