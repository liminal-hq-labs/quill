// Safe save: write a temporary file beside the target, check it, then replace the target with it.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::{Files, Replaced, SaveMode, NAME_MAX};
use quill_formats::io::Buffered;
use quill_formats::FormatError;

/// How a write ended.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Saved {
	/// The target holds the new document.
	Done,
	/// The target is as it was, or was never there; no temporary file is left.
	Failed,
	/// The old target was removed and the new document waits in the temporary file.
	Kept,
}

/// The longest folder-and-name the shells keep, with room for the temporary name's own part.
const TEMP_MAX: usize = NAME_MAX + 16;

/// The digits of the target's hash in a temporary name: three on 8.3 systems, eight where long names are kept.
const DIGITS: usize = if cfg!(feature = "long-names") { 8 } else { 3 };

/// The name of the file a save writes before it replaces the target: `~QTMP` and the hash of the target's name in hex, then `.TMP`,
/// in the target's folder. It is a legal 8.3 name, so every system takes it, and it differs from the recovery names (`~Q…​.RCV`).
pub struct TempName {
	buf: [u8; TEMP_MAX],
	dir: usize,
	len: usize,
}

impl TempName {
	/// The temporary name for a save to `target`.
	pub fn of(target: &[u8]) -> TempName {
		let mut t = TempName {
			buf: [0; TEMP_MAX],
			dir: 0,
			len: 0,
		};
		// The folder is everything up to the last separator or drive colon.
		let dir = target
			.iter()
			.rposition(|b| matches!(b, b'\\' | b'/' | b':'))
			.map_or(0, |i| i + 1)
			.min(NAME_MAX);
		t.push(target.get(..dir).unwrap_or(&[]));
		t.dir = dir;
		t.push(b"~QTMP");
		// FNV-1a over the upper-cased name, so `a.qld` and `A.QLD` share a temporary name.
		let mut h = 0x811C_9DC5u32;
		for b in target {
			h = (h ^ u32::from(b.to_ascii_uppercase())).wrapping_mul(0x0100_0193);
		}
		for i in (0..DIGITS).rev() {
			let d = ((h >> (i * 4)) & 15) as u8;
			t.push(&[if d < 10 { b'0' + d } else { b'A' + d - 10 }]);
		}
		t.push(b".TMP");
		t
	}

	fn push(&mut self, b: &[u8]) {
		for &c in b {
			if let Some(slot) = self.buf.get_mut(self.len) {
				*slot = c;
				self.len += 1;
			}
		}
	}

	/// The whole name, folder included.
	pub fn bytes(&self) -> &[u8] {
		self.buf.get(..self.len).unwrap_or(&[])
	}

	/// The name without its folder.
	pub fn file(&self) -> &[u8] {
		self.buf.get(self.dir..self.len).unwrap_or(&[])
	}
}

/// Writes a document to `target` so that the target is never left half written. `write` puts the bytes into the sink it is given
/// (which gathers small writes into kilobyte blocks). On a shell that can rename, the bytes go to a temporary file first; it is closed,
/// its size checked against what was written, and only then does it take the target's place. The old file is never touched before that.
pub(crate) fn write_replace<F: Files>(
	f: &mut F,
	target: &[u8],
	write: impl FnOnce(&mut Buffered<'_, F::W>) -> Result<(), FormatError>,
) -> Saved {
	let mode = f.save_mode();
	let temp = TempName::of(target);
	let path = if mode == SaveMode::TempRename {
		temp.bytes()
	} else {
		target
	};
	let Some(mut w) = f.create(path) else {
		return Saved::Failed;
	};
	let wrote = {
		let mut b = Buffered::new(&mut w);
		write(&mut b).ok().and_then(|()| b.finish())
	};
	let closed = f.close(w);
	let (Some(total), true) = (wrote, closed) else {
		// A save written in place has already hurt the file; one written to a temporary file takes the half file away.
		if mode == SaveMode::TempRename {
			f.remove(path);
		}
		return Saved::Failed;
	};
	if mode != SaveMode::TempRename {
		return Saved::Done;
	}
	if f.len(path).is_some_and(|n| n != total) {
		f.remove(path);
		return Saved::Failed;
	}
	match f.replace(path, target) {
		Replaced::Done => Saved::Done,
		Replaced::Refused => {
			f.remove(path);
			Saved::Failed
		}
		Replaced::Lost => Saved::Kept,
	}
}

/// Takes away the temporary file a save that did not finish may have left beside `target`, once the target itself is known to be
/// there: a temporary file next to a missing target may hold the only copy of a document.
pub(crate) fn clean_leftover<F: Files>(f: &mut F, target: &[u8]) {
	if f.save_mode() == SaveMode::TempRename {
		f.remove(TempName::of(target).bytes());
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn the_temporary_name_is_a_legal_8_3_name_in_the_targets_folder() {
		let t = TempName::of(b"C:\\DOCS\\LETTER.QLD");
		assert!(t.bytes().starts_with(b"C:\\DOCS\\~QTMP"));
		let file = t.file();
		assert!(file.starts_with(b"~QTMP") && file.ends_with(b".TMP"));
		assert_eq!(file.len(), 5 + DIGITS + 4);
		if !cfg!(feature = "long-names") {
			assert_eq!(file.len(), 12, "8.3");
		}
		assert_eq!(
			TempName::of(b"a.qld").bytes(),
			TempName::of(b"A.QLD").bytes()
		);
		assert_ne!(
			TempName::of(b"A.QLD").bytes(),
			TempName::of(b"B.QLD").bytes()
		);
		assert!(TempName::of(b"D:A.QLD").bytes().starts_with(b"D:~QTMP"));
	}
}
