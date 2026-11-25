// The words of a program's command line: bare or in double quotes, as the Windows builds receive it.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `QUILL.EXE [FILE] [/SWITCH [VALUE]]…`. A word ends at a space, tab or any control character; a word that starts with `"` runs to the
//! next `"` (or the end of the line) and keeps its spaces, which long file names under Windows on ReactOS and Wine need. The shells
//! keep their own switches; this only finds the words.

/// The next word of `line` at or after `*at`, as the range of its text (without the quotes), moving `*at` past it. `None` at the end.
pub fn next_word(line: &[u8], at: &mut usize) -> Option<(usize, usize)> {
	let mut i = *at;
	while line.get(i).is_some_and(|b| *b <= b' ') {
		i += 1;
	}
	if i >= line.len() {
		*at = i;
		return None;
	}
	let quoted = line.get(i) == Some(&b'"');
	if quoted {
		i += 1;
	}
	let start = i;
	while let Some(b) = line.get(i) {
		if (quoted && *b == b'"') || (!quoted && *b <= b' ') {
			break;
		}
		i += 1;
	}
	let end = i;
	*at = if quoted && i < line.len() { i + 1 } else { i };
	Some((start, end))
}

/// Whether the word is a switch (`/PLAY`) and not a file name: it starts with `/` and was not quoted.
pub fn is_switch(line: &[u8], start: usize) -> bool {
	line.get(start) == Some(&b'/') && !(start > 0 && line.get(start - 1) == Some(&b'"'))
}

#[cfg(test)]
#[allow(clippy::indexing_slicing)]
mod tests {
	use super::*;

	fn words(line: &[u8]) -> [Option<&[u8]>; 6] {
		let mut out = [None; 6];
		let mut at = 0;
		for slot in out.iter_mut() {
			*slot = next_word(line, &mut at).and_then(|(s, e)| line.get(s..e));
		}
		out
	}

	#[test]
	fn bare_and_quoted_words() {
		let w = words(b" C:\\QUILL\\DEMO16.QLD /MONO \"C:\\My Docs\\a b.qld\"  ");
		assert_eq!(w[0], Some(&b"C:\\QUILL\\DEMO16.QLD"[..]));
		assert_eq!(w[1], Some(&b"/MONO"[..]));
		assert_eq!(w[2], Some(&b"C:\\My Docs\\a b.qld"[..]));
		assert_eq!(w[3], None);
	}

	#[test]
	fn control_characters_end_words() {
		let w = words(b"A.QLD\r\n/PLAY\tK.TXT\r\n");
		assert_eq!(w[0], Some(&b"A.QLD"[..]));
		assert_eq!(w[1], Some(&b"/PLAY"[..]));
		assert_eq!(w[2], Some(&b"K.TXT"[..]));
		assert_eq!(w[3], None);
	}

	#[test]
	fn an_unclosed_quote_runs_to_the_end() {
		let w = words(b"\"C:\\My Docs\\a.qld");
		assert_eq!(w[0], Some(&b"C:\\My Docs\\a.qld"[..]));
		assert_eq!(w[1], None);
	}

	#[test]
	fn empty_quotes_and_empty_lines() {
		assert_eq!(words(b"")[0], None);
		assert_eq!(words(b"   ")[0], None);
		assert_eq!(words(b"\"\" x")[0], Some(&b""[..]));
	}

	#[test]
	fn a_quoted_slash_is_a_file_name() {
		let line = b"/PLAY \"/odd name\"";
		let mut at = 0;
		let (s, _) = next_word(line, &mut at).unwrap_or((0, 0));
		assert!(is_switch(line, s));
		let (s, e) = next_word(line, &mut at).unwrap_or((0, 0));
		assert_eq!(line.get(s..e), Some(&b"/odd name"[..]));
		assert!(!is_switch(line, s));
	}
}
