// The document the S0b page edits: lines of text, one caret, and the Windows-1252 rule for what may be typed.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Quill's text is Windows-1252 bytes. Input arrives as UTF-16 from the keyboard, an input method or the emoji panel, so
//! every insertion is converted character by character and an insertion with any character that has no 1252 form is
//! refused whole, as the Linux client does.

/// What an insertion did.
#[derive(Debug, PartialEq, Eq)]
pub enum Inserted {
	/// The 1252 bytes that were stored, in order.
	Bytes(Vec<u8>),
	/// The first character with no 1252 form; nothing was stored.
	Refused(char),
}

/// The 1252 byte for `c`, or `None` when the code page has no such character.
pub fn to_1252(c: char) -> Option<u8> {
	let mut buf = [0u8; 4];
	let s = c.encode_utf8(&mut buf);
	let (bytes, _, unmappable) = encoding_rs::WINDOWS_1252.encode(s);
	(!unmappable && bytes.len() == 1).then(|| bytes[0])
}

#[derive(Clone, Debug)]
pub struct Doc {
	pub lines: Vec<String>,
	/// Line and character index (not byte index) of the caret.
	pub caret: (usize, usize),
}

impl Doc {
	pub fn new(lines: &[&str]) -> Doc {
		let lines: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
		let last = lines.len().saturating_sub(1);
		let col = lines.last().map(|l| l.chars().count()).unwrap_or(0);
		Doc {
			lines,
			caret: (last, col),
		}
	}

	/// Builds a document from text with `\r`, `\n` or `\r\n` line ends, the caret at the end.
	pub fn from_text(text: &str) -> Doc {
		let normalised = text.replace("\r\n", "\n").replace('\r', "\n");
		let lines: Vec<&str> = normalised.split('\n').collect();
		Doc::new(&lines)
	}

	pub fn text(&self) -> String {
		self.lines.join("\n")
	}

	fn byte_at(&self, line: usize, col: usize) -> usize {
		let l = &self.lines[line];
		l.char_indices().nth(col).map(|(i, _)| i).unwrap_or(l.len())
	}

	pub fn insert_str(&mut self, s: &str) -> Inserted {
		let mut bytes = Vec::new();
		for c in s.chars() {
			if c == '\r' || c == '\n' {
				continue;
			}
			match to_1252(c) {
				Some(b) => bytes.push(b),
				None => return Inserted::Refused(c),
			}
		}
		let at = self.byte_at(self.caret.0, self.caret.1);
		let kept: String = s.chars().filter(|c| *c != '\r' && *c != '\n').collect();
		self.lines[self.caret.0].insert_str(at, &kept);
		self.caret.1 += kept.chars().count();
		Inserted::Bytes(bytes)
	}

	pub fn enter(&mut self) {
		let at = self.byte_at(self.caret.0, self.caret.1);
		let rest = self.lines[self.caret.0].split_off(at);
		self.lines.insert(self.caret.0 + 1, rest);
		self.caret = (self.caret.0 + 1, 0);
	}

	pub fn backspace(&mut self) {
		let (l, c) = self.caret;
		if c > 0 {
			let a = self.byte_at(l, c - 1);
			let b = self.byte_at(l, c);
			self.lines[l].replace_range(a..b, "");
			self.caret.1 -= 1;
		} else if l > 0 {
			let tail = self.lines.remove(l);
			let col = self.lines[l - 1].chars().count();
			self.lines[l - 1].push_str(&tail);
			self.caret = (l - 1, col);
		}
	}

	pub fn left(&mut self) {
		if self.caret.1 > 0 {
			self.caret.1 -= 1;
		} else if self.caret.0 > 0 {
			self.caret.0 -= 1;
			self.caret.1 = self.lines[self.caret.0].chars().count();
		}
	}

	pub fn right(&mut self) {
		let len = self.lines[self.caret.0].chars().count();
		if self.caret.1 < len {
			self.caret.1 += 1;
		} else if self.caret.0 + 1 < self.lines.len() {
			self.caret = (self.caret.0 + 1, 0);
		}
	}

	pub fn up(&mut self) {
		if self.caret.0 > 0 {
			self.caret.0 -= 1;
			self.caret.1 = self.caret.1.min(self.lines[self.caret.0].chars().count());
		}
	}

	pub fn down(&mut self) {
		if self.caret.0 + 1 < self.lines.len() {
			self.caret.0 += 1;
			self.caret.1 = self.caret.1.min(self.lines[self.caret.0].chars().count());
		}
	}

	pub fn home(&mut self) {
		self.caret.1 = 0;
	}

	pub fn end(&mut self) {
		self.caret.1 = self.lines[self.caret.0].chars().count();
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn accented_letters_have_a_1252_byte() {
		assert_eq!(to_1252('\u{e9}'), Some(0xE9));
		assert_eq!(to_1252('\u{20ac}'), Some(0x80));
	}

	#[test]
	fn an_emoji_is_refused_whole() {
		let mut d = Doc::new(&[""]);
		assert_eq!(d.insert_str("a\u{1f600}"), Inserted::Refused('\u{1f600}'));
		assert_eq!(d.text(), "");
	}

	#[test]
	fn editing_moves_the_caret() {
		let mut d = Doc::new(&["ab"]);
		d.left();
		assert_eq!(d.insert_str("\u{e9}"), Inserted::Bytes(vec![0xE9]));
		assert_eq!(d.text(), "a\u{e9}b");
		d.enter();
		assert_eq!(d.lines, vec!["a\u{e9}".to_string(), "b".to_string()]);
		d.backspace();
		assert_eq!(d.text(), "a\u{e9}b");
	}
}
