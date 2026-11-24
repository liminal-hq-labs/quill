// The size table: bytes of a QUILL.FNT raw and deflated, against the budgets.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::faceset::{FACE_NAMES, STYLE_NAMES};
use crate::fnt::{Font, BOXES_LEN, ENTRY_LEN, HEADER_LEN, MAX_DATA_LEN};
use crate::render::GLYPHS;

/// The unpacked budget for `QUILL.FNT`, in bytes (450 KB).
pub const BUDGET_RAW: usize = 450 * 1024;
/// The deflated budget, in bytes (200 KB).
pub const BUDGET_PACKED: usize = 200 * 1024;

/// Deflate level 9, as the `.QPK` pack would take it (a stand-in: the pack's own encoder may differ by a few per cent).
pub fn deflated(data: &[u8]) -> usize {
	miniz_oxide::deflate::compress_to_vec(data, 9).len()
}

/// The SHA-256 of `data` as lower-case hex.
pub fn sha256(data: &[u8]) -> String {
	Sha256::digest(data).iter().fold(String::new(), |mut s, b| {
		let _ = write!(s, "{b:02x}");
		s
	})
}

/// Prints the breakdown of one file: totals against the budgets, bytes by face, by size and the resident part.
pub fn describe(label: &str, bytes: &[u8]) -> String {
	let font = Font::parse(bytes).expect("a file this crate wrote");
	let mut out = String::new();
	let kb = |n: usize| format!("{:.1} KB", n as f64 / 1024.0);
	let packed = deflated(bytes);
	let _ = writeln!(out, "{label}");
	let _ = writeln!(
		out,
		"  strikes {}, raw {} bytes ({}), deflated {} bytes ({}), sha-256 {}",
		font.entries.len(),
		bytes.len(),
		kb(bytes.len()),
		packed,
		kb(packed),
		sha256(bytes)
	);
	let _ = writeln!(
		out,
		"  budget 450 KB raw: {} ({} {}), 200 KB deflated: {} ({} {})",
		if bytes.len() <= BUDGET_RAW {
			"within"
		} else {
			"OVER"
		},
		kb(bytes.len().abs_diff(BUDGET_RAW)),
		if bytes.len() <= BUDGET_RAW {
			"to spare"
		} else {
			"over"
		},
		if packed <= BUDGET_PACKED {
			"within"
		} else {
			"OVER"
		},
		kb(packed.abs_diff(BUDGET_PACKED)),
		if packed <= BUDGET_PACKED {
			"to spare"
		} else {
			"over"
		},
	);
	let resident = HEADER_LEN + font.entries.len() * (ENTRY_LEN + GLYPHS);
	let biggest = font.entries.iter().map(|e| e.data_len).max().unwrap_or(0);
	let _ = writeln!(
		out,
		"  resident at start (header, directory, advances): {resident} bytes; largest strike chunk {biggest} bytes of {MAX_DATA_LEN}; boxes {BOXES_LEN} bytes per strike"
	);
	let _ = writeln!(
		out,
		"  bytes by face and style (strike data, all six sizes):"
	);
	for face in 0..3u8 {
		let mut line = format!("    {:<5}", FACE_NAMES[usize::from(face)]);
		let mut total = 0;
		for style in 0..4u8 {
			let n: usize = font
				.entries
				.iter()
				.filter(|e| e.face == face && e.style == style)
				.map(|e| e.data_len as usize + GLYPHS)
				.sum();
			total += n;
			let _ = write!(line, " {:>11} {:>7}", STYLE_NAMES[usize::from(style)], n);
		}
		let _ = writeln!(out, "{line}  = {total}");
	}
	let _ = writeln!(out, "  bytes by size (all faces and styles):");
	let mut sizes: Vec<u16> = font.entries.iter().map(|e| e.half_points).collect();
	sizes.sort_unstable();
	sizes.dedup();
	for hp in sizes {
		let n: usize = font
			.entries
			.iter()
			.filter(|e| e.half_points == hp)
			.map(|e| e.data_len as usize + GLYPHS)
			.sum();
		let px = font
			.entries
			.iter()
			.find(|e| e.half_points == hp)
			.map_or(0, |e| e.px);
		let _ = writeln!(out, "    {:>2} pt ({px} px) {n:>8}", hp / 2);
	}
	out
}
