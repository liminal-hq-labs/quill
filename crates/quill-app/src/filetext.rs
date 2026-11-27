// What the two painters of the file dialog say in words and numbers: sizes, dates, the line of counts and a field's window.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The text painter (`filepaint`) and the graphics painter (`quill-paint`'s `filedlg`) draw the same dialog, so what is written in
// it is decided once, here: `12 KB`, the country's date and time, `3 items, 1 folder`, the kind of a file and which part of a
// long name fits its field.

use quill_dlg::filedlg::{Entry, EntryKind, FileDlg};
use quill_dlg::filelayout::{file_icon, Icon};

use crate::datefmt;

/// `12 KB`: the size in kilobytes, rounded up as Windows 95 shows it (an empty file is `0 KB`).
pub fn size_text(size: u32, out: &mut [u8; 12]) -> usize {
	let kb = size.div_ceil(1024);
	let mut digits = [0u8; 10];
	let mut i = digits.len();
	let mut v = kb;
	loop {
		i -= 1;
		if let Some(d) = digits.get_mut(i) {
			*d = b'0' + (v % 10) as u8;
		}
		v /= 10;
		if v == 0 {
			break;
		}
	}
	let mut n = 0;
	for b in digits.get(i..).unwrap_or(&[]).iter().chain(b" KB") {
		if let Some(slot) = out.get_mut(n) {
			*slot = *b;
			n += 1;
		}
	}
	n
}

/// The date of a DOS stamp in the dialog's pattern, with two-digit years when `short`.
pub fn date_text(src: &[u8], date: u16, short: bool, out: &mut [u8; 16]) -> usize {
	let mut pat = [0u8; 10];
	let mut n = 0;
	let mut i = 0;
	while let Some(&c) = src.get(i) {
		// `yyyy` becomes `yy` on the text screen.
		if short && c == b'y' && src.get(i..i + 4) == Some(b"yyyy") {
			for _ in 0..2 {
				if let Some(slot) = pat.get_mut(n) {
					*slot = b'y';
					n += 1;
				}
			}
			i += 4;
			continue;
		}
		if let Some(slot) = pat.get_mut(n) {
			*slot = c;
			n += 1;
		}
		i += 1;
	}
	let year = 1980 + u32::from(date >> 9);
	let (month, day) = (u32::from((date >> 5) & 15), u32::from(date & 31));
	datefmt::format(pat.get(..n).unwrap_or(&[]), year, month, day, out)
}

/// The time of a DOS stamp, on a 24-hour clock when `h24`.
pub fn time_text(h24: bool, time: u16, out: &mut [u8; 16]) -> usize {
	let (h, m) = (u32::from(time >> 11), u32::from((time >> 5) & 63));
	datefmt::format_time(h, m, h24, b"", b"", b"", out)
}

pub fn icon_of(e: &Entry) -> Icon {
	match e.kind() {
		EntryKind::Up => Icon::Parent,
		EntryKind::Folder => Icon::Folder,
		EntryKind::File => file_icon(e.name()),
	}
}

pub fn type_name(e: &Entry) -> &'static [u8] {
	match e.kind() {
		EntryKind::Up | EntryKind::Folder => b"Folder",
		EntryKind::File => match file_icon(e.name()) {
			Icon::Document => b"Quill document",
			Icon::Rtf => b"Rich text file",
			Icon::Text => b"Text file",
			Icon::Picture => b"Picture",
			_ => b"File",
		},
	}
}

/// How many items and folders, in the words of the status line.
pub fn counts(dlg: &FileDlg, out: &mut [u8; 40]) -> usize {
	fn push(out: &mut [u8; 40], n: &mut usize, b: &[u8]) {
		for c in b {
			if let Some(slot) = out.get_mut(*n) {
				*slot = *c;
				*n += 1;
			}
		}
	}
	fn num(out: &mut [u8; 40], n: &mut usize, mut v: usize) {
		let mut d = [0u8; 8];
		let mut i = d.len();
		loop {
			i -= 1;
			if let Some(x) = d.get_mut(i) {
				*x = b'0' + (v % 10) as u8;
			}
			v /= 10;
			if v == 0 {
				break;
			}
		}
		push(out, n, d.get(i..).unwrap_or(&[]));
	}
	let folders = dlg.folder_count();
	let up = usize::from(dlg.folder().len() > 3);
	let items = dlg.len().saturating_sub(up);
	let mut n = 0;
	num(out, &mut n, items);
	push(
		out,
		&mut n,
		if items == 1 { b" item, " } else { b" items, " },
	);
	num(out, &mut n, folders);
	push(
		out,
		&mut n,
		if folders == 1 {
			b" folder"
		} else {
			b" folders"
		},
	);
	n
}

/// Which part of a field's text shows when it is `w` cells wide and the caret is at `cursor`: the start, and how much.
pub fn field_window(len: usize, cursor: usize, w: usize) -> (usize, usize) {
	let start = if cursor >= w { cursor + 1 - w } else { 0 };
	(start, len.saturating_sub(start).min(w))
}
