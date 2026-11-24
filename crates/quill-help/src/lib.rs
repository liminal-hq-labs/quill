// quill-help: a WinHelp 3.1 file reader (for hlpdump and the tests) and writer (for hlpc).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The format is implemented from published descriptions of it (Manfred Winterhoff's helpfile.txt, and the way the free `winhlp32` reads
// a file, which explains behaviour without any of its code being used here) and checked against the real WINHELP.EXE of Windows 3.11
// and against the .HLP files that shipped with it. Nothing here depends on the rest of Quill.
pub mod read;
pub mod write;

/// Why a help file could not be read or built.
#[derive(Debug)]
pub enum HelpError {
	/// The file is not a WinHelp file, or is damaged: what was wrong.
	Bad(&'static str),
	/// The source text could not be compiled: the line and what was wrong.
	Source(usize, String),
}

impl std::fmt::Display for HelpError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			HelpError::Bad(m) => write!(f, "bad help file: {m}"),
			HelpError::Source(l, m) => write!(f, "line {l}: {m}"),
		}
	}
}

impl std::error::Error for HelpError {}

/// The hash WinHelp gives a context name (`[MAP]` and `\a` jumps use it).
pub fn hash(name: &str) -> i32 {
	let mut h: i32 = 0;
	for c in name.bytes() {
		let x = match c {
			b'A'..=b'Z' => i32::from(c - b'A') + 17,
			b'a'..=b'z' => i32::from(c - b'a') + 17,
			b'1'..=b'9' => i32::from(c - b'0'),
			b'0' => 10,
			b'.' => 12,
			b'_' => 13,
			_ => 0,
		};
		if x != 0 {
			h = h.wrapping_mul(43).wrapping_add(x);
		}
	}
	h
}
