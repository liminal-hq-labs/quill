// The default extension of a name typed into a Save As dialog, so every shell completes it the same way.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// The extensions of the Save As types, in the order of the dialog's list (Quill Documents, Rich Text Format, Text Files). The
/// fourth type, All Files, has none of its own, so it takes the first.
pub const SAVE_EXTS: [&[u8]; 3] = [b"qld", b"rtf", b"txt"];

/// The extension (no dot) that Save As adds for the 1-based `filter_index` of the type the person chose.
pub fn ext_for_filter(filter_index: u32) -> &'static [u8] {
	let i = (filter_index as usize).saturating_sub(1);
	SAVE_EXTS.get(i).copied().unwrap_or(b"qld")
}

/// True when the last path component of `name` has a dot. A name that ends in a dot has an empty extension, which the common
/// dialogs also count as typed, so it is left alone.
fn has_extension(name: &[u8]) -> bool {
	let part = name
		.iter()
		.rposition(|b| matches!(*b, b'\\' | b'/' | b':'))
		.map_or(name, |i| name.get(i + 1..).unwrap_or(&[]));
	part.contains(&b'.')
}

/// Adds the type's extension to the `len` bytes of name at the start of `buf` when the last component has none, and returns the new
/// length (the same `len` when nothing was added, the name is empty or the extension does not fit). A NUL follows the name when
/// there is room.
pub fn complete_name(buf: &mut [u8], len: usize, filter_index: u32) -> usize {
	let Some(name) = buf.get(..len) else {
		return len;
	};
	if len == 0 || has_extension(name) {
		return len;
	}
	let ext = ext_for_filter(filter_index);
	let end = len + 1 + ext.len();
	let Some(tail) = buf.get_mut(len..end) else {
		return len;
	};
	if let Some((dot, rest)) = tail.split_first_mut() {
		*dot = b'.';
		rest.copy_from_slice(ext);
	}
	if let Some(z) = buf.get_mut(end) {
		*z = 0;
	}
	end
}

#[cfg(test)]
mod tests {
	use super::*;

	#[allow(clippy::indexing_slicing)]
	fn run(name: &[u8], filter: u32) -> ([u8; 64], usize) {
		let mut buf = [0u8; 64];
		buf[..name.len()].copy_from_slice(name);
		let n = complete_name(&mut buf, name.len(), filter);
		(buf, n)
	}

	#[allow(clippy::indexing_slicing)]
	fn same(got: ([u8; 64], usize), want: &[u8]) {
		assert_eq!(&got.0[..got.1], want);
	}

	#[test]
	fn a_bare_name_takes_the_extension_of_each_type() {
		same(run(b"LETTER", 1), b"LETTER.qld");
		same(run(b"LETTER", 2), b"LETTER.rtf");
		same(run(b"LETTER", 3), b"LETTER.txt");
	}

	#[test]
	fn all_files_and_a_bad_index_fall_back_to_qld() {
		same(run(b"LETTER", 4), b"LETTER.qld");
		same(run(b"LETTER", 0), b"LETTER.qld");
	}

	#[test]
	fn a_typed_extension_is_kept() {
		same(run(b"LETTER.DOC", 2), b"LETTER.DOC");
		same(run(b"LETTER.", 2), b"LETTER.");
	}

	#[test]
	fn a_dot_in_a_folder_name_does_not_count() {
		same(run(b"C:\\MY.DOCS\\LETTER", 3), b"C:\\MY.DOCS\\LETTER.txt");
		same(
			run(b"C:\\MY.DOCS\\LETTER.RTF", 3),
			b"C:\\MY.DOCS\\LETTER.RTF",
		);
		same(run(b"C:LETTER", 1), b"C:LETTER.qld");
	}

	#[test]
	fn empty_and_full_buffers_are_left_alone() {
		same(run(b"", 1), b"");
		let mut buf = *b"ABCD";
		assert_eq!(complete_name(&mut buf, 4, 1), 4);
		assert_eq!(&buf, b"ABCD");
	}

	#[test]
	#[allow(clippy::indexing_slicing)]
	fn a_nul_follows_when_there_is_room() {
		let mut buf = [b'x'; 16];
		buf[..3].copy_from_slice(b"ABC");
		let n = complete_name(&mut buf, 3, 2);
		assert_eq!((&buf[..n], buf[n]), (&b"ABC.rtf"[..], 0));
	}
}
