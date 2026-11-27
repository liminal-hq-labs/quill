// Tooltips: the tooltip control of COMCTL32 4.00, the version Windows 95 shipped with, asked for the text of a button when it is needed.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The toolbar's buttons are painted, not windows, so each is a tool with a rectangle; the drop-down boxes and the preview's push
//! buttons are windows and are tools by handle. The control subclasses the main window (`TTF_SUBCLASS`) to see the pointer, and asks
//! for each text with `TTN_NEEDTEXT` when it is about to show it, so the strings live in the core and nowhere else. Only messages
//! and structure sizes that comctl32 4.00 has are used: `TOOLINFO` is 40 bytes, with no `lParam` member.

use alloc::vec::Vec;
use core::ptr::null_mut;

use crate::sys::*;

const WM_USER: u32 = 0x0400;
const TTM_SETDELAYTIME: u32 = WM_USER + 3;
const TTM_ADDTOOLA: u32 = WM_USER + 4;
const TTM_DELTOOLA: u32 = WM_USER + 5;
const TTS_ALWAYSTIP: u32 = 1;
const TTF_IDISHWND: u32 = 1;
const TTF_SUBCLASS: u32 = 0x10;
const TTDT_INITIAL: usize = 3;
/// `TTN_NEEDTEXTA`: `TTN_FIRST` is 0U - 520U.
pub const TTN_NEEDTEXTA: u32 = 0xFFFF_FDF8;
const LPSTR_TEXTCALLBACK: isize = -1;
const WS_POPUP: u32 = 0x8000_0000;
const CW_USEDEFAULT: i32 = i32::MIN;

#[repr(C)]
struct ToolInfo {
	size: u32,
	flags: u32,
	hwnd: Handle,
	id: usize,
	rect: Rect,
	inst: Handle,
	text: isize,
}

#[repr(C)]
struct NmHdr {
	hwnd_from: Handle,
	id_from: usize,
	code: u32,
}

/// What `TTN_NEEDTEXTA` carries: the header, a pointer to the text, and room to put the text in.
#[repr(C)]
struct NmTtDispInfo {
	hdr: NmHdr,
	text: *const u8,
	buf: [u8; 80],
	inst: Handle,
	flags: u32,
}

/// The tooltip control and the tools it has been given.
pub struct Tips {
	wnd: Handle,
	/// Each tool added: its id (a button's number or a window handle), whether it is a window, and the window that owns it.
	added: Vec<(usize, bool, Handle)>,
}

impl Tips {
	/// Makes the control as a pop-up owned by `owner`. None when the common controls are not there.
	///
	/// # Safety
	/// `owner` must be the main window.
	pub unsafe fn new(owner: Handle, instance: Handle) -> Option<Tips> {
		InitCommonControls();
		let wnd = CreateWindowExA(
			0,
			b"tooltips_class32\0".as_ptr(),
			core::ptr::null(),
			WS_POPUP | TTS_ALWAYSTIP,
			CW_USEDEFAULT,
			CW_USEDEFAULT,
			CW_USEDEFAULT,
			CW_USEDEFAULT,
			owner,
			null_mut(),
			instance,
			core::ptr::null(),
		);
		if wnd.is_null() {
			return None;
		}
		// A shorter wait than the default half second: a toolbar is explored quickly.
		SendMessageA(wnd, TTM_SETDELAYTIME, TTDT_INITIAL, 400);
		Some(Tips { wnd, added: Vec::new() })
	}

	fn info(owner: Handle, id: usize, is_window: bool, rect: Rect) -> ToolInfo {
		ToolInfo {
			size: 40,
			flags: TTF_SUBCLASS | if is_window { TTF_IDISHWND } else { 0 },
			hwnd: owner,
			id,
			rect,
			inst: null_mut(),
			text: LPSTR_TEXTCALLBACK,
		}
	}

	/// Takes every tool away, ready for a new set.
	///
	/// # Safety
	/// `owner` must be the window the tools were added for.
	pub unsafe fn clear(&mut self, owner: Handle) {
		let none = Rect {
			left: 0,
			top: 0,
			right: 0,
			bottom: 0,
		};
		for (id, is_window, tool_owner) in self.added.drain(..) {
			let ti = Self::info(if tool_owner.is_null() { owner } else { tool_owner }, id, is_window, none);
			SendMessageA(self.wnd, TTM_DELTOOLA, 0, core::ptr::addr_of!(ti) as isize);
		}
	}

	/// Adds a tool for a painted button: `id` names it in `text_for`, `rect` is where it is in the client area.
	///
	/// # Safety
	/// `owner` must be the main window.
	pub unsafe fn add_rect(&mut self, owner: Handle, id: usize, rect: Rect) {
		let ti = Self::info(owner, id, false, rect);
		if SendMessageA(self.wnd, TTM_ADDTOOLA, 0, core::ptr::addr_of!(ti) as isize) != 0 {
			self.added.push((id, false, null_mut()));
		}
	}

	/// Adds a tool for a painted part of a child window of the main window (the status bar): `rect` is in `window`'s client area, and
	/// the text is asked of `window` (a `WM_NOTIFY` to it), because the tooltip control watches the pointer over `window` itself.
	///
	/// # Safety
	/// `window` must be a live window.
	pub unsafe fn add_rect_in(&mut self, window: Handle, id: usize, rect: Rect) {
		let ti = Self::info(window, id, false, rect);
		if SendMessageA(self.wnd, TTM_ADDTOOLA, 0, core::ptr::addr_of!(ti) as isize) != 0 {
			self.added.push((id, false, window));
		}
	}

	/// Adds a tool for a child window of `owner`.
	///
	/// # Safety
	/// `child` must be a live child window of `owner`.
	pub unsafe fn add_window(&mut self, owner: Handle, child: Handle) {
		let none = Rect {
			left: 0,
			top: 0,
			right: 0,
			bottom: 0,
		};
		let ti = Self::info(owner, child as usize, true, none);
		if SendMessageA(self.wnd, TTM_ADDTOOLA, 0, core::ptr::addr_of!(ti) as isize) != 0 {
			self.added.push((child as usize, true, null_mut()));
		}
	}
}

/// Answers a `WM_NOTIFY` from the control: when it is asking for a tool's text, `text_for(id, is_window)` gives it and it is put in
/// the notification. Returns true when the message was one of those.
///
/// # Safety
/// `lparam` must be the `WM_NOTIFY` message's `lParam`.
pub unsafe fn answer(lparam: isize, text_for: impl Fn(usize, bool) -> Vec<u8>) -> bool {
	let nm = lparam as *mut NmTtDispInfo;
	if nm.is_null() || (*nm).hdr.code != TTN_NEEDTEXTA {
		return false;
	}
	let text = text_for((*nm).hdr.id_from, (*nm).flags & TTF_IDISHWND != 0);
	let n = text.len().min(79);
	let buf = &mut (*nm).buf;
	buf.fill(0);
	if let (Some(d), Some(s)) = (buf.get_mut(..n), text.get(..n)) {
		d.copy_from_slice(s);
	}
	(*nm).text = (*nm).buf.as_ptr();
	true
}
