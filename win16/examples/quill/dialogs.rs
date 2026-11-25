// The native dialogs of the Windows 3.1 window: About, Find and Replace, Tabs and Spelling from the dialog resources
// (`assets/dialogs/dialogs.json`), and Open, Save As, Insert Picture, Fonts and Print Setup from COMMDLG.DLL.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The editor says which dialog it wants (`quill_app::Dialog`); these functions show it, read what was chosen and hand it back through
// the editor's own methods, as the Win32 build's `dialogs.rs` does (D86). The dialog procedures are exported far procedures, so they
// run with DS loaded from SS, and `DialogBox` takes them as segment 1 offsets.
use quill_app::view;
use quill_app::{Command, Files, FindAction};
use w16::*;

use crate::gdi::{Rect16, fp, np};
use crate::{Win16Files, app, store};

export_proc!(AboutDlgProc, about_proc);
export_proc!(FindDlgProc, find_proc);
export_proc!(TabsDlgProc, tabs_proc);
export_proc!(SpellDlgProc, spell_proc);

const LB_ADDSTRING: u32 = WM_USER + 1;
const LB_RESETCONTENT: u32 = WM_USER + 5;
const LB_SETCURSEL: u32 = WM_USER + 7;
const LB_GETCURSEL: u32 = WM_USER + 9;
const LB_GETTEXT: u32 = WM_USER + 10;
const MB_ICONQUESTION_YNC: u32 = 0x23;
const MB_ICONEXCLAMATION: u32 = 0x30;

const DLG_ABOUT: u32 = 1001;
const DLG_FIND: u32 = 1002;
const DLG_TABS: u32 = 1004;
const DLG_SPELLING: u32 = 1005;

static mut OWNER: u32 = 0;
static mut REPLACE: bool = false;

/// A short text built without `alloc`, ending in a zero for Windows.
pub struct Text<const N: usize> {
	buf: [u8; N],
	n: usize,
}

impl<const N: usize> Text<N> {
	pub fn new() -> Self {
		Text { buf: [0; N], n: 0 }
	}
	pub fn b(&mut self, t: &[u8]) -> &mut Self {
		for c in t {
			if self.n + 1 < N {
				self.buf[self.n] = *c;
				self.n += 1;
			}
		}
		self.buf[self.n] = 0;
		self
	}
	pub fn s(&mut self, t: &str) -> &mut Self {
		self.b(t.as_bytes())
	}
	pub fn dec(&mut self, v: u32) -> &mut Self {
		let mut d = [0u8; 10];
		let k = crate::chrome::dec(v, &mut d);
		self.b(&d[..k])
	}
	/// Twips as inches with two decimals, as the Tabs dialog shows them.
	pub fn inches(&mut self, twips: i32) -> &mut Self {
		let t = twips.max(0) as u32;
		self.dec(t / 1440).s(".");
		let frac = (t % 1440) * 100 / 1440;
		if frac < 10 {
			self.s("0");
		}
		self.dec(frac)
	}
	pub fn ptr(&self) -> u32 {
		np(self.buf.as_ptr())
	}
	pub fn bytes(&self) -> &[u8] {
		&self.buf[..self.n]
	}
}

fn set_text(dlg: u32, id: u32, t: &[u8]) {
	let mut z: Text<96> = Text::new();
	z.b(t);
	unsafe { SetDlgItemText(dlg, id, z.ptr()) };
}

/// The text of a dialog item, into `out`; its length.
fn get_text(dlg: u32, id: u32, out: &mut [u8; 64]) -> usize {
	out.fill(0);
	let n = unsafe { GetDlgItemText(dlg, id, np(out.as_mut_ptr()), 63) } as usize;
	n.min(63)
}

fn checked(dlg: u32, id: u32) -> bool {
	unsafe { IsDlgButtonChecked(dlg, id) & 0xFFFF == 1 }
}

fn message(owner: u32, text: &[u8], flags: u32) -> u32 {
	let mut z: Text<120> = Text::new();
	z.b(text);
	crate::diag::check(0xF003);
	let r = unsafe { MessageBox(owner, z.ptr(), cs!("Quill"), flags) & 0xFFFF };
	crate::diag::check(0xF004);
	r
}

/// Puts a dialog in the middle of the window that owns it.
fn centre(dlg: u32) {
	let (mut o, mut d) = (Rect16::default(), Rect16::default());
	unsafe {
		GetWindowRect(OWNER, np(&mut o));
		GetWindowRect(dlg, np(&mut d));
		let x = i32::from(o.left) + (i32::from(o.right - o.left) - i32::from(d.right - d.left)) / 2;
		let y = i32::from(o.top) + (i32::from(o.bottom - o.top) - i32::from(d.bottom - d.top)) / 2;
		// SWP_NOSIZE | SWP_NOZORDER
		SetWindowPos(dlg, 0, x.max(0) as u32, y.max(0) as u32, 0, 0, 0x0005);
	}
}

fn run(owner: u32, id: u32, proc_: unsafe extern "C" fn()) {
	unsafe {
		OWNER = owner;
		DialogBox(
			crate::sh().hinst,
			far(0, int_resource(id)),
			owner,
			proc_ as usize as u32 & 0xFFFF,
		);
	}
}

/// Asks the window to paint the page again (the dialog changed the document behind it).
fn repaint_owner() {
	unsafe { InvalidateRect(OWNER, 0, 0) };
}

// ---- About ---------------------------------------------------------------------------------------------------------------------

extern "C" fn about_proc(dlg: u32, msg: u32, wparam: u32, _lparam: u32) -> u32 {
	match msg {
		WM_INITDIALOG => {
			let mut t: Text<64> = Text::new();
			t.s("Version ").s(quill_app::VERSION);
			set_text(dlg, 22, t.bytes());
			let v = unsafe { GetVersion() };
			let (major, minor) = (v & 0xFF, (v >> 8) & 0xFF);
			let mut t: Text<64> = Text::new();
			t.s(app().about_platform())
				.s(" on Windows ")
				.dec(major)
				.s(".");
			if minor < 10 {
				t.s("0");
			}
			t.dec(minor);
			set_text(dlg, 25, t.bytes());
			let free = unsafe { GetFreeSpace(0) };
			let mut t: Text<64> = Text::new();
			t.s("Free memory: ").dec(free / 1024).s(" KB");
			set_text(dlg, 26, t.bytes());
			centre(dlg);
			1
		}
		WM_COMMAND if wparam == IDOK || wparam == IDCANCEL => {
			unsafe { EndDialog(dlg, 1) };
			1
		}
		_ => 0,
	}
}

// ---- Find and Replace ----------------------------------------------------------------------------------------------------------

extern "C" fn find_proc(dlg: u32, msg: u32, wparam: u32, _lparam: u32) -> u32 {
	match msg {
		WM_INITDIALOG => {
			let (find, with, case, word) = app().last_find();
			set_text(dlg, 10, find);
			set_text(dlg, 11, with);
			unsafe {
				CheckDlgButton(dlg, 12, u32::from(case));
				CheckDlgButton(dlg, 13, u32::from(word));
				if !REPLACE {
					for id in [11, 31, 101, 102] {
						ShowWindow(GetDlgItem(dlg, id), 0);
					}
				}
				SetWindowText(dlg, if REPLACE { cs!("Replace") } else { cs!("Find") });
			}
			centre(dlg);
			1
		}
		WM_COMMAND => {
			let action = match wparam {
				100 => FindAction::Next,
				101 => FindAction::Replace,
				102 => FindAction::ReplaceAll,
				IDCANCEL => {
					unsafe { EndDialog(dlg, 0) };
					return 1;
				}
				_ => return 0,
			};
			let (mut find, mut with) = ([0u8; 64], [0u8; 64]);
			let nf = get_text(dlg, 10, &mut find);
			let nw = get_text(dlg, 11, &mut with);
			let (case, word) = (checked(dlg, 12), checked(dlg, 13));
			let found = app().find_text(store(), &find[..nf], &with[..nw], case, word, action);
			repaint_owner();
			if !found && nf > 0 {
				message(dlg, b"Quill cannot find that text.", MB_ICONINFORMATION);
			}
			if action == FindAction::ReplaceAll && !app().message().is_empty() {
				let mut t: Text<100> = Text::new();
				t.b(app().message());
				message(dlg, t.bytes(), MB_ICONINFORMATION);
			}
			1
		}
		_ => 0,
	}
}

// ---- Tabs ----------------------------------------------------------------------------------------------------------------------

/// "1.25" as twips.
fn parse_inches(t: &[u8]) -> Option<i32> {
	let t = t.trim_ascii();
	let (whole, frac) = match t.iter().position(|b| *b == b'.') {
		Some(i) => (&t[..i], &t[i + 1..]),
		None => (t, &b""[..]),
	};
	let mut w: i32 = 0;
	for b in whole {
		if !b.is_ascii_digit() {
			return None;
		}
		w = w.checked_mul(10)?.checked_add(i32::from(b - b'0'))?;
	}
	let mut f = 0i32;
	let mut scale = 100;
	for b in frac.iter().take(2) {
		if !b.is_ascii_digit() {
			return None;
		}
		f += i32::from(b - b'0') * scale / 10;
		scale /= 10;
	}
	if whole.is_empty() && frac.is_empty() {
		return None;
	}
	w.checked_mul(1440)?.checked_add(f * 1440 / 100)
}

fn tabs_list(dlg: u32) {
	unsafe { SendDlgItemMessage(dlg, 19, LB_RESETCONTENT, 0, 0) };
	let mut stops = [(0, 0, 0); 16];
	let n = app().tab_stops_full(store(), &mut stops);
	for (pos, kind, leader) in stops.iter().take(n) {
		let mut t: Text<48> = Text::new();
		t.inches(*pos)
			.s(" in  ")
			.s(["Left", "Centre", "Right", "Decimal"][usize::from(*kind & 3)])
			.s("  ")
			.s(["", "dots", "hyphens", "underline"][usize::from(*leader & 3)]);
		unsafe { SendDlgItemMessage(dlg, 19, LB_ADDSTRING, 0, fp(t.buf.as_ptr())) };
	}
}

extern "C" fn tabs_proc(dlg: u32, msg: u32, wparam: u32, lparam: u32) -> u32 {
	match msg {
		WM_INITDIALOG => {
			unsafe {
				CheckRadioButton(dlg, 11, 14, 11);
				CheckRadioButton(dlg, 15, 18, 15);
			}
			set_text(dlg, 10, b"0.50");
			tabs_list(dlg);
			centre(dlg);
			1
		}
		WM_COMMAND => {
			let (a, s) = (app(), store());
			match wparam {
				20 | 21 => {
					let mut buf = [0u8; 64];
					let n = get_text(dlg, 10, &mut buf);
					let Some(pos) = parse_inches(&buf[..n]) else {
						message(
							dlg,
							b"Type a position in inches, such as 1.25.",
							MB_ICONEXCLAMATION,
						);
						return 1;
					};
					if wparam == 20 {
						let kind = (11..=14).position(|r| checked(dlg, r)).unwrap_or(0) as u8;
						let leader = (15..=18).position(|r| checked(dlg, r)).unwrap_or(0) as u8;
						a.set_tab(s, pos, kind, leader);
					} else {
						a.remove_tab(s, pos);
					}
					tabs_list(dlg);
					repaint_owner();
					1
				}
				22 => {
					a.clear_tabs(s);
					tabs_list(dlg);
					repaint_owner();
					1
				}
				// LBN_SELCHANGE: the chosen stop goes into the position box and the kind buttons.
				19 if lparam >> 16 == 1 => {
					let at = unsafe { SendDlgItemMessage(dlg, 19, LB_GETCURSEL, 0, 0) } & 0xFFFF;
					let mut stops = [(0, 0, 0); 16];
					let n = a.tab_stops_full(s, &mut stops);
					if (at as usize) < n {
						let (pos, kind, leader) = stops[at as usize];
						let mut t: Text<16> = Text::new();
						t.inches(pos);
						set_text(dlg, 10, t.bytes());
						unsafe {
							CheckRadioButton(dlg, 11, 14, 11 + u32::from(kind & 3));
							CheckRadioButton(dlg, 15, 18, 15 + u32::from(leader & 3));
						}
					}
					1
				}
				IDCANCEL => {
					unsafe { EndDialog(dlg, 0) };
					1
				}
				_ => 0,
			}
		}
		_ => 0,
	}
}

// ---- Spelling ------------------------------------------------------------------------------------------------------------------

/// Shows the word the check stopped at, its sentence and its suggestions.
fn spelling_fill(dlg: u32) {
	let (a, s) = (app(), store());
	set_text(dlg, 20, a.spell_word());
	let mut ctx = [0u8; 72];
	let n = a.spell_context(s, &mut ctx);
	set_text(dlg, 21, &ctx[..n]);
	unsafe { SendDlgItemMessage(dlg, 11, LB_RESETCONTENT, 0, 0) };
	let sg = a.spell_suggestions(s);
	for i in 0..sg.len() {
		if let Some(c) = sg.get(i) {
			let mut t: Text<40> = Text::new();
			t.b(c.bytes());
			unsafe { SendDlgItemMessage(dlg, 11, LB_ADDSTRING, 0, fp(t.buf.as_ptr())) };
		}
	}
	// The first suggestion is the one Change takes; the edit box shows it and can be typed over.
	match sg.get(0) {
		Some(c) => {
			unsafe { SendDlgItemMessage(dlg, 11, LB_SETCURSEL, 0, 0) };
			set_text(dlg, 10, c.bytes());
		}
		None => set_text(dlg, 10, b""),
	}
	repaint_owner();
}

extern "C" fn spell_proc(dlg: u32, msg: u32, wparam: u32, lparam: u32) -> u32 {
	match msg {
		WM_INITDIALOG => {
			spelling_fill(dlg);
			centre(dlg);
			1
		}
		WM_COMMAND => {
			let (a, s) = (app(), store());
			let mut with = [0u8; 64];
			let nw = get_text(dlg, 10, &mut with);
			let more = match wparam {
				100 => a.spell_ignore(s, false),
				101 => a.spell_ignore(s, true),
				102 | 103 if nw == 0 => return 1,
				102 => a.spell_change(s, &with[..nw], false),
				103 => a.spell_change(s, &with[..nw], true),
				104 => a.spell_add(s, &mut Win16Files),
				11 if lparam >> 16 == 1 => {
					// LBN_SELCHANGE: the chosen suggestion goes into the box.
					let at = unsafe { SendDlgItemMessage(dlg, 11, LB_GETCURSEL, 0, 0) } & 0xFFFF;
					let mut buf = [0u8; 64];
					if at != 0xFFFF {
						let n = unsafe {
							SendDlgItemMessage(dlg, 11, LB_GETTEXT, at, fp(buf.as_mut_ptr()))
						} & 0xFFFF;
						set_text(dlg, 10, &buf[..(n as usize).min(63)]);
					}
					return 1;
				}
				IDCANCEL => {
					a.spell_cancel();
					unsafe { EndDialog(dlg, 0) };
					return 1;
				}
				_ => return 0,
			};
			if more {
				spelling_fill(dlg);
			} else {
				unsafe { EndDialog(dlg, 1) };
				message(
					unsafe { OWNER },
					b"The spelling check is complete.",
					MB_ICONINFORMATION,
				);
			}
			1
		}
		_ => 0,
	}
}

fn spelling(owner: u32) {
	let (a, s) = (app(), store());
	if a.spell_begin(s, &mut Win16Files) {
		run(owner, DLG_SPELLING, SpellDlgProc);
	} else if a.message().is_empty() {
		message(
			owner,
			b"The spelling check is complete.",
			MB_ICONINFORMATION,
		);
	} else {
		let mut t: Text<100> = Text::new();
		t.b(a.message());
		message(owner, t.bytes(), MB_ICONINFORMATION);
	}
}

// ---- the common dialogs --------------------------------------------------------------------------------------------------------

/// OPENFILENAME as COMMDLG.DLL of Windows 3.1 lays it out (72 bytes, far pointers).
#[repr(C, packed)]
struct OpenFileName {
	size: u32,
	owner: u16,
	instance: u16,
	filter: u32,
	custom: u32,
	max_custom: u32,
	filter_index: u32,
	file: u32,
	max_file: u32,
	file_title: u32,
	max_file_title: u32,
	initial_dir: u32,
	title: u32,
	flags: u32,
	file_offset: u16,
	file_ext: u16,
	def_ext: u32,
	cust_data: u32,
	hook: u32,
	template: u32,
}

#[derive(Copy, Clone, PartialEq)]
enum FileKind {
	Open,
	Save,
	Picture,
}

const DOC_FILTER: &[u8] =
	b"Quill Documents (*.qld)\0*.qld\0Rich Text Format (*.rtf)\0*.rtf\0Text Files (*.txt)\0*.txt\0All Files (*.*)\0*.*\0\0";
const PIC_FILTER: &[u8] = b"Pictures (*.bmp;*.png)\0*.bmp;*.png\0Bitmaps (*.bmp)\0*.bmp\0PNG Pictures (*.png)\0*.png\0All Files (*.*)\0*.*\0\0";

/// Shows the common Open, Save As or Insert Picture dialog and returns the chosen path (in `out`, its length). It starts in the
/// folder of the last one, and the folder of what was chosen is remembered for the next (`QUILL.INI`).
fn file_dialog(owner: u32, kind: FileKind, out: &mut [u8; 128]) -> Option<usize> {
	out.fill(0);
	let mut dir: Text<128> = Text::new();
	dir.b(app().last_folder());
	let title: u32 = match kind {
		FileKind::Open => cs!("Open"),
		FileKind::Save => cs!("Save As"),
		FileKind::Picture => cs!("Insert Picture"),
	};
	let ds = data_selector();
	let mut ofn = OpenFileName {
		size: core::mem::size_of::<OpenFileName>() as u32,
		owner: owner as u16,
		instance: 0,
		filter: fp(if kind == FileKind::Picture {
			PIC_FILTER
		} else {
			DOC_FILTER
		}
		.as_ptr()),
		custom: 0,
		max_custom: 0,
		filter_index: 1,
		file: fp(out.as_mut_ptr()),
		max_file: 128,
		file_title: 0,
		max_file_title: 0,
		initial_dir: if dir.n > 0 { fp(dir.buf.as_ptr()) } else { 0 },
		title: far(ds, title),
		// OFN_HIDEREADONLY | OFN_PATHMUSTEXIST, then OFN_OVERWRITEPROMPT (save) or OFN_FILEMUSTEXIST (open)
		flags: 0x0004
			| 0x0800 | if kind == FileKind::Save {
			0x0002
		} else {
			0x1000
		},
		file_offset: 0,
		file_ext: 0,
		def_ext: if kind == FileKind::Save {
			far(ds, cs!("qld"))
		} else {
			0
		},
		cust_data: 0,
		hook: 0,
		template: 0,
	};
	crate::diag::check(0xF001);
	let ok = unsafe {
		if kind == FileKind::Save {
			GetSaveFileName(np(&mut ofn))
		} else {
			GetOpenFileName(np(&mut ofn))
		}
	} & 0xFFFF;
	crate::diag::check(0xF002);
	if ok == 0 {
		common_error();
		return None;
	}
	let mut n = out.iter().position(|b| *b == 0).unwrap_or(0);
	if kind == FileKind::Save {
		// COMMDLG adds `lpstrDefExt` only for a name it can see, and not every host does; so a name typed without an extension
		// gets the chosen type's here, and the overwrite prompt then asks about the name that will be written.
		let filter_index = ofn.filter_index;
		let typed = n;
		n = quill_app::savename::complete_name(&mut out[..], n, filter_index);
		if n != typed && Win16Files.open(&out[..n]).is_some() {
			// MB_YESNO | MB_ICONQUESTION
			if message(owner, b"That file already exists. Replace it?", 0x24) != 6 {
				return None;
			}
		}
	}
	app().remember_folder(&out[..n]);
	Some(n)
}

/// A common dialog that returned without a choice: nothing when the person cancelled, the error number on the status line otherwise
/// (`CommDlgExtendedError`, such as 0x1007 for no default printer).
fn common_error() {
	let e = unsafe { CommDlgExtendedError() } & 0xFFFF;
	if e != 0 {
		let mut t: Text<64> = Text::new();
		t.s("The dialog could not open (COMMDLG error ")
			.dec(e)
			.s(").");
		if let Ok(text) = core::str::from_utf8(t.bytes()) {
			app().set_message(text);
		}
	}
}

/// Saves the current document, asking for a name first when it has none. False when cancelled or the save failed.
fn save_current(owner: u32) -> bool {
	if app().name().is_empty() {
		let mut path = [0u8; 128];
		let Some(n) = file_dialog(owner, FileKind::Save, &mut path) else {
			return false;
		};
		crate::diag::check(0xF005);
		let saved = app().save_as(store(), &mut Win16Files, &path[..n]);
		crate::diag::check(0xF006);
		return saved;
	}
	crate::diag::check(0xF005);
	app().command(store(), &mut Win16Files, Command::Save);
	crate::diag::check(0xF006);
	!app().modified()
}

/// The 50-byte LOGFONT inside CHOOSEFONT.
#[repr(C)]
struct LogFont {
	height: i16,
	width: i16,
	escapement: i16,
	orientation: i16,
	weight: i16,
	italic: u8,
	underline: u8,
	strike: u8,
	charset: u8,
	out_precision: u8,
	clip_precision: u8,
	quality: u8,
	pitch_family: u8,
	face: [u8; 32],
}

/// CHOOSEFONT as COMMDLG.DLL of Windows 3.1 lays it out (46 bytes).
#[repr(C, packed)]
struct ChooseFontStruct {
	size: u32,
	owner: u16,
	dc: u16,
	log_font: u32,
	point_size: i16,
	flags: u32,
	colour: u32,
	cust_data: u32,
	hook: u32,
	template: u32,
	instance: u16,
	style: u32,
	font_type: u16,
	size_min: i16,
	size_max: i16,
}

/// Character > Fonts: the common Font dialog, with every face, style, size, effect and colour the display offers, applied to the
/// selection (or to what is typed next).
fn fonts(owner: u32) {
	let (a, s) = (app(), store());
	let cs = a.char_state(s);
	let mut lf = LogFont {
		height: -(((i32::from(cs.size) * 96 + 72) / 144) as i16),
		width: 0,
		escapement: 0,
		orientation: 0,
		weight: if cs.flags & quill_core::doc::cflag::BOLD != 0 {
			700
		} else {
			400
		},
		italic: u8::from(cs.flags & quill_core::doc::cflag::ITALIC != 0),
		underline: u8::from(cs.flags & quill_core::doc::cflag::UNDERLINE != 0),
		strike: u8::from(cs.flags & quill_core::doc::cflag::STRIKE != 0),
		charset: 0,
		out_precision: 0,
		clip_precision: 0,
		quality: 0,
		pitch_family: 0,
		face: [0; 32],
	};
	let n = usize::from(cs.face_len).min(31);
	lf.face[..n].copy_from_slice(&cs.face[..n]);
	let colour = cs.colour.map_or(0, |(r, g, b)| {
		u32::from(r) | (u32::from(g) << 8) | (u32::from(b) << 16)
	});
	let mut cf = ChooseFontStruct {
		size: core::mem::size_of::<ChooseFontStruct>() as u32,
		owner: owner as u16,
		dc: 0,
		log_font: fp(&lf),
		point_size: 0,
		// CF_SCREENFONTS | CF_INITTOLOGFONTSTRUCT | CF_EFFECTS
		flags: 0x0001 | 0x0040 | 0x0100,
		colour,
		cust_data: 0,
		hook: 0,
		template: 0,
		instance: 0,
		style: 0,
		font_type: 0,
		size_min: 0,
		size_max: 0,
	};
	if unsafe { ChooseFont(np(&mut cf)) } & 0xFFFF == 0 {
		common_error();
		return;
	}
	let face_n = lf.face.iter().position(|b| *b == 0).unwrap_or(31);
	let face = &lf.face[..face_n];
	// iPointSize is in tenths of a point; the document keeps half points.
	let tenths = cf.point_size;
	let half_points = (i32::from(tenths) / 5).clamp(8, 288) as u16;
	let mut flags = 0u16;
	if lf.weight >= 600 {
		flags |= quill_core::doc::cflag::BOLD;
	}
	if lf.italic != 0 {
		flags |= quill_core::doc::cflag::ITALIC;
	}
	if lf.underline != 0 {
		flags |= quill_core::doc::cflag::UNDERLINE;
	}
	if lf.strike != 0 {
		flags |= quill_core::doc::cflag::STRIKE;
	}
	a.set_font(s, face, view::family_of(face), half_points, flags);
	let rgb = cf.colour;
	let chosen = (rgb & 0xFF_FFFF != 0).then_some((
		(rgb & 0xFF) as u8,
		((rgb >> 8) & 0xFF) as u8,
		((rgb >> 16) & 0xFF) as u8,
	));
	if chosen != cs.colour {
		a.set_colour(s, chosen);
	}
}

/// PRINTDLG as COMMDLG.DLL of Windows 3.1 lays it out (52 bytes).
#[repr(C, packed)]
struct PrintDlgStruct {
	size: u32,
	owner: u16,
	dev_mode: u16,
	dev_names: u16,
	dc: u16,
	flags: u32,
	from_page: u16,
	to_page: u16,
	min_page: u16,
	max_page: u16,
	copies: u16,
	instance: u16,
	cust_data: u32,
	print_hook: u32,
	setup_hook: u32,
	print_template: u32,
	setup_template: u32,
	print_res: u16,
	setup_res: u16,
}

/// The common Print dialog (`setup`: its Print Setup page). Returns true when the person pressed OK.
pub fn print_dialog(owner: u32, setup: bool) -> bool {
	let mut pd = PrintDlgStruct {
		size: core::mem::size_of::<PrintDlgStruct>() as u32,
		owner: owner as u16,
		dev_mode: 0,
		dev_names: 0,
		dc: 0,
		// PD_PRINTSETUP, or PD_NOSELECTION | PD_NOPAGENUMS for the print page.
		flags: if setup { 0x0040 } else { 0x0004 | 0x0008 },
		from_page: 1,
		to_page: 1,
		min_page: 1,
		max_page: 1,
		copies: 1,
		instance: 0,
		cust_data: 0,
		print_hook: 0,
		setup_hook: 0,
		print_template: 0,
		setup_template: 0,
		print_res: 0,
		setup_res: 0,
	};
	let ok = unsafe { PrintDlg(np(&mut pd)) } & 0xFFFF != 0;
	if !ok {
		common_error();
	}
	unsafe {
		if pd.dev_mode != 0 {
			GlobalFree(u32::from(pd.dev_mode));
		}
		if pd.dev_names != 0 {
			GlobalFree(u32::from(pd.dev_names));
		}
	}
	ok
}

// ---- the requests --------------------------------------------------------------------------------------------------------------

/// Shows every dialog the editor has asked for, as a real dialog (D86), and applies the answers.
pub fn handle(owner: u32) {
	while let Some(d) = app().take_dialog() {
		let (a, s) = (app(), store());
		match d {
			quill_app::Dialog::About => run(owner, DLG_ABOUT, AboutDlgProc),
			quill_app::Dialog::Open => {
				let mut path = [0u8; 128];
				if let Some(n) = file_dialog(owner, FileKind::Open, &mut path) {
					crate::set_clock();
					if a.open(s, &mut Win16Files, &path[..n]).is_err() {
						a.set_message("Quill cannot open that file.");
					}
				}
			}
			quill_app::Dialog::SaveAs => {
				let mut path = [0u8; 128];
				if let Some(n) = file_dialog(owner, FileKind::Save, &mut path) {
					a.save_as(s, &mut Win16Files, &path[..n]);
				}
			}
			quill_app::Dialog::Picture => {
				let mut path = [0u8; 128];
				if let Some(n) = file_dialog(owner, FileKind::Picture, &mut path) {
					if a.insert_picture(s, &mut Win16Files, &path[..n]).is_err() {
						a.set_message("Quill cannot use that picture.");
					}
				}
			}
			quill_app::Dialog::Print => {
				if print_dialog(owner, false) {
					message(
						owner,
						b"Printing is not in the Windows 3.1 build yet.",
						MB_ICONINFORMATION,
					);
				}
			}
			quill_app::Dialog::Fonts => fonts(owner),
			quill_app::Dialog::Tabs => run(owner, DLG_TABS, TabsDlgProc),
			quill_app::Dialog::Find(replace) => {
				unsafe { REPLACE = replace };
				run(owner, DLG_FIND, FindDlgProc);
			}
			quill_app::Dialog::Spelling => spelling(owner),
			quill_app::Dialog::ConfirmSave(c) => {
				let mut text = [0u8; 120];
				let n = crate::confirm_text(c, &mut text);
				match message(owner, &text[..n], MB_ICONQUESTION_YNC) {
					6 => {
						if save_current(owner) {
							app().run_confirmed(store(), &mut Win16Files, c);
						}
					}
					7 => {
						app().discard_changes();
						app().run_confirmed(store(), &mut Win16Files, c);
					}
					_ => {}
				}
			}
		}
	}
}

// The structures' sizes as COMMDLG.DLL checks them in their first field.
const _: () = assert!(core::mem::size_of::<OpenFileName>() == 72);
const _: () = assert!(core::mem::size_of::<ChooseFontStruct>() == 46);
const _: () = assert!(core::mem::size_of::<PrintDlgStruct>() == 52);
const _: () = assert!(core::mem::size_of::<LogFont>() == 50);
