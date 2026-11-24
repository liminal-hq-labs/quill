// Plain text: ANSI, with CRLF, LF or CR ending a paragraph.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use quill_core::doc::Doc;
use quill_core::store::{ReadStore, Store};
use quill_core::text::MAX_TEXT;

use crate::io::{Sink, Source};
use crate::{FormatError, Result};

/// Reads plain text into `doc` (replacing it). CRLF, LF and CR each end a paragraph; a form feed becomes a paragraph break;
/// other control characters except the tab are dropped. The text always ends with a paragraph mark.
pub fn read<S: Store + ?Sized, R: Source>(doc: &mut Doc, s: &mut S, r: &mut R) -> Result<()> {
	doc.init(s)?;
	let body = &mut doc.body;
	body.clear(s);
	let mut buf = [0u8; 512];
	let mut out = [0u8; 512];
	let mut prev_cr = false;
	let mut total = 0u32;
	let mut last_mark = true;
	loop {
		let n = r.read(&mut buf);
		let mut o = 0;
		for &b in buf.iter().take(n) {
			let c = match b {
				0x0A if prev_cr => {
					prev_cr = false;
					continue;
				}
				0x0A | 0x0C => 0x0D,
				0x0D => 0x0D,
				0x09 | 0x20..=0xFF => b,
				_ => {
					prev_cr = false;
					continue;
				}
			};
			prev_cr = b == 0x0D;
			if let Some(slot) = out.get_mut(o) {
				*slot = c;
			}
			o += 1;
			last_mark = c == 0x0D;
		}
		if o > 0 {
			total += o as u32;
			if total > MAX_TEXT {
				return Err(FormatError::TooLarge);
			}
			body.append_original(s, out.get(..o).unwrap_or(&[]), 0, 0)?;
		}
		if n < buf.len() {
			break;
		}
	}
	if !last_mark || total == 0 {
		body.append_original(s, b"\r", 0, 0)?;
	}
	body.forget_history(s);
	Ok(())
}

/// Writes the body as ANSI text with CRLF after every paragraph and a form feed's place left to the page-break flag (not yet
/// written). Pictures and headers are left out.
pub fn write<S: ReadStore + ?Sized, K: Sink>(doc: &Doc, s: &S, k: &mut K) -> Result<()> {
	let t = &doc.body;
	let mut at = 0u32;
	let mut buf = [0u8; 256];
	let mut out = [0u8; 512];
	while at < t.len() {
		let n = t.read(s, at, &mut buf);
		if n == 0 {
			break;
		}
		let mut o = 0;
		for &b in buf.iter().take(n) {
			let pair: &[u8] = match b {
				0x0D => b"\r\n",
				0x0B => b"\r\n",
				0x01 => b"",
				_ => core::slice::from_ref(&b),
			};
			for &c in pair {
				if let Some(slot) = out.get_mut(o) {
					*slot = c;
				}
				o += 1;
			}
		}
		if !k.write(out.get(..o).unwrap_or(&[])) {
			return Err(FormatError::Write);
		}
		at += n as u32;
	}
	Ok(())
}
