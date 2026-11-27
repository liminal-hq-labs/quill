// File > Print to PDF: lays the document out with the screen's fonts at a printer-like resolution and writes a PDF with a text layer.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::string::String;
use alloc::vec::Vec;
use core::ptr::{null, null_mut};

use quill_app::{Command, Files};
use quill_formats::io::Sink;
use quill_pdfdoc::Options;

use crate::gdi::GdiMeasure;
use crate::sys::*;
use crate::{ui, Shell};

/// The menu id of File > Print to PDF (the commands take 100 and up, the recent files 600 and the spelling pop-up 700).
pub const ID_PRINT_PDF: usize = 650;

/// The folder Print to PDF offers, made when it is missing.
const DEFAULT_DIR: &[u8] = b"C:\\Printed\0";

/// The resolution the page is laid out at when `QUILL.INI` has no `[Print] PdfDpi`: the screen's own 96, so the lines break where the
/// window and the print preview break them and the PDF is the page as it looks.
const DEFAULT_DPI: u16 = 96;

/// The resolution the PDF's pages are laid out at: `[Print] PdfDpi` in `QUILL.INI`, 96 to 1,200, or 96. A higher one lays the page out
/// as a printer of that resolution would, with the fonts' finer widths, and may break lines differently from the screen.
unsafe fn layout_dpi() -> u16 {
	let mut buf = [0u8; 16];
	let n = GetPrivateProfileStringA(
		b"Print\0".as_ptr(),
		b"PdfDpi\0".as_ptr(),
		b"\0".as_ptr(),
		buf.as_mut_ptr(),
		16,
		b"QUILL.INI\0".as_ptr(),
	) as usize;
	let mut value = 0u32;
	for b in buf.get(..n).unwrap_or(&[]).iter().filter(|b| b.is_ascii_digit()) {
		value = value.saturating_mul(10).saturating_add(u32::from(b - b'0'));
	}
	if value == 0 {
		DEFAULT_DPI
	} else {
		value.clamp(96, 1200) as u16
	}
}

/// The document's PDF at `dpi` layout resolution.
unsafe fn render(sh: &Shell, dpi: u16) -> Vec<u8> {
	let mut measure = GdiMeasure {
		dc: sh.screen_dc,
		grid: false,
		dpi,
	};
	// The title is the document's name without its folder and extension.
	let own = sh.app.name();
	let leaf = own
		.iter()
		.rposition(|b| *b == b'\\' || *b == b':')
		.map_or(own, |i| own.get(i + 1..).unwrap_or(&[]));
	let title = leaf
		.iter()
		.rposition(|b| *b == b'.')
		.map_or(leaf, |i| leaf.get(..i).unwrap_or(&[]));
	quill_pdfdoc::export(
		&*sh.app,
		&sh.store,
		&mut measure,
		&Options {
			title,
			..Options::default()
		},
	)
}

/// Writes `bytes` to the file `path` (no NUL). False when the file could not be made or the disk refused it.
fn write_file(sh: &mut Shell, path: &[u8], bytes: &[u8]) -> bool {
	let Some(mut out) = sh.files.create(path) else {
		return false;
	};
	let wrote = out.write(bytes);
	sh.files.close(out) && wrote
}

/// Writes the document as a PDF to `path`, for the command line (`/PDF OUT`). False when it could not be written.
pub unsafe fn write_document(sh: &mut Shell, path: &[u8]) -> bool {
	let bytes = render(sh, layout_dpi());
	write_file(sh, path, &bytes)
}

/// The name Print to PDF starts with: the document's own, with its extension changed to `.pdf`.
fn default_name(own: &[u8]) -> Vec<u8> {
	let base = own
		.iter()
		.rposition(|b| *b == b'\\' || *b == b':')
		.map_or(own, |i| own.get(i + 1..).unwrap_or(&[]));
	let stem = base
		.iter()
		.rposition(|b| *b == b'.')
		.map_or(base, |i| base.get(..i).unwrap_or(&[]));
	let mut name = if stem.is_empty() {
		b"Document".to_vec()
	} else {
		stem.to_vec()
	};
	name.extend_from_slice(b".pdf");
	name
}

/// Asks where to put the PDF and writes it there; the status line says how it went.
pub unsafe fn print_to_pdf(hwnd: Handle, sh: &mut Shell) {
	CreateDirectoryA(DEFAULT_DIR.as_ptr(), null());
	let mut file = [0u8; 260];
	let name = default_name(sh.app.name());
	let n = name.len().min(259);
	file[..n].copy_from_slice(name.get(..n).unwrap_or(&[]));
	let filter: &[u8] = b"PDF Documents (*.pdf)\0*.pdf\0All Files (*.*)\0*.*\0\0";
	let mut ofn = crate::OpenFileNameA {
		size: 76,
		owner: hwnd,
		instance: null_mut(),
		filter: filter.as_ptr(),
		custom: null_mut(),
		max_custom: 0,
		filter_index: 1,
		file: file.as_mut_ptr(),
		max_file: 260,
		title_file: null_mut(),
		max_title: 0,
		initial_dir: DEFAULT_DIR.as_ptr(),
		title: b"Print to PDF\0".as_ptr(),
		// OFN_EXPLORER | OFN_HIDEREADONLY | OFN_PATHMUSTEXIST | OFN_OVERWRITEPROMPT
		flags: 0x0008_0000 | 0x0004 | 0x0800 | 0x0002,
		file_offset: 0,
		file_ext: 0,
		def_ext: b"pdf\0".as_ptr(),
		cust_data: 0,
		hook: null(),
		template: null(),
	};
	if crate::GetSaveFileNameA(&mut ofn) == 0 {
		return;
	}
	let len = file.iter().position(|b| *b == 0).unwrap_or(0);
	let path = file.get(..len).unwrap_or(&[]).to_vec();
	sh.app.set_message("Printing to PDF...");
	ui::status_update(hwnd, sh);
	let bytes = render(sh, layout_dpi());
	let leaf = path
		.iter()
		.rposition(|b| *b == b'\\' || *b == b':')
		.map_or(&path[..], |i| path.get(i + 1..).unwrap_or(&[]));
	let mut message = String::new();
	if write_file(sh, &path, &bytes) {
		message.push_str("Printed to ");
		message.extend(leaf.iter().map(|b| char::from(*b)));
	} else {
		message.push_str("Quill could not write the PDF.");
	}
	sh.app.set_message(&message);
	ui::status_update(hwnd, sh);
}

/// Adds File > Print to PDF after Print Setup in the menu `bar`, with the shortcut the key handler answers.
pub unsafe fn add_menu_item(bar: Handle) {
	let file = GetSubMenu(bar, 0);
	if file.is_null() {
		return;
	}
	// The position after Print Setup: each item and each rule before it takes a place in the menu.
	let mut at = 0;
	for it in quill_app::menu_items(0) {
		if it.rule {
			at += 1;
		}
		at += 1;
		if it.command == Command::PrintSetup {
			InsertMenuA(
				file,
				at,
				MF_BYPOSITION | MF_STRING,
				ID_PRINT_PDF,
				b"Print to P&DF...\tCtrl+Shift+P\0".as_ptr(),
			);
			return;
		}
	}
}
