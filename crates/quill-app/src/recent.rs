// The recent-file list: four names, most recent first, and how a long path is shortened to fit a menu.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::NAME_MAX;

/// How many files the list keeps (the SPEC's limit).
pub const RECENT_MAX: usize = 4;

/// The last files opened or saved: most recent first, each once, at most [`RECENT_MAX`].
#[derive(Copy, Clone)]
pub struct RecentList {
	names: [[u8; NAME_MAX]; RECENT_MAX],
	lens: [usize; RECENT_MAX],
}

impl Default for RecentList {
	fn default() -> Self {
		Self::new()
	}
}

/// File names compare as DOS and Windows compare them: without regard to case.
fn same(a: &[u8], b: &[u8]) -> bool {
	a.eq_ignore_ascii_case(b)
}

impl RecentList {
	/// An empty list.
	pub const fn new() -> Self {
		RecentList {
			names: [[0; NAME_MAX]; RECENT_MAX],
			lens: [0; RECENT_MAX],
		}
	}

	/// How many names it holds.
	pub fn len(&self) -> usize {
		self.lens.iter().take_while(|l| **l > 0).count()
	}

	/// True when it holds none.
	pub fn is_empty(&self) -> bool {
		self.len() == 0
	}

	/// The name at `i`, 0 being the most recent.
	pub fn get(&self, i: usize) -> Option<&[u8]> {
		let len = self.lens.get(i).copied().filter(|l| *l > 0)?;
		self.names.get(i)?.get(..len)
	}

	/// The names, most recent first.
	pub fn iter(&self) -> impl Iterator<Item = &[u8]> {
		(0..RECENT_MAX).map_while(|i| self.get(i))
	}

	/// Makes `name` the most recent: an earlier entry for the same file goes, the others move down and the oldest falls off the end.
	/// A name longer than the list holds is kept as far as it goes; an empty name changes nothing.
	pub fn push(&mut self, name: &[u8]) {
		if name.is_empty() {
			return;
		}
		let n = name.len().min(NAME_MAX);
		let name = name.get(..n).unwrap_or(&[]);
		let at = self
			.iter()
			.position(|e| same(e, name))
			.unwrap_or(RECENT_MAX - 1);
		for i in (1..=at).rev() {
			let (prev, plen) = (
				self.names.get(i - 1).copied().unwrap_or([0; NAME_MAX]),
				self.lens.get(i - 1).copied().unwrap_or(0),
			);
			if let (Some(d), Some(dl)) = (self.names.get_mut(i), self.lens.get_mut(i)) {
				*d = prev;
				*dl = plen;
			}
		}
		if let (Some(d), Some(dl)) = (self.names.first_mut(), self.lens.first_mut()) {
			*d = [0; NAME_MAX];
			if let Some(dst) = d.get_mut(..n) {
				dst.copy_from_slice(name);
			}
			*dl = n;
		}
	}

	/// Adds `name` as the oldest entry, as reading a settings file does: skipped when the list is full or already has it.
	pub fn append(&mut self, name: &[u8]) {
		let at = self.len();
		if name.is_empty() || at >= RECENT_MAX || self.iter().any(|e| same(e, name)) {
			return;
		}
		let n = name.len().min(NAME_MAX);
		if let (Some(d), Some(dl)) = (self.names.get_mut(at), self.lens.get_mut(at)) {
			*d = [0; NAME_MAX];
			if let (Some(dst), Some(src)) = (d.get_mut(..n), name.get(..n)) {
				dst.copy_from_slice(src);
			}
			*dl = n;
		}
	}

	/// Takes `name` out of the list, closing the gap.
	pub fn remove(&mut self, name: &[u8]) {
		let Some(at) = self.iter().position(|e| same(e, name)) else {
			return;
		};
		for i in at..RECENT_MAX {
			let (next, nlen) = (
				self.names.get(i + 1).copied().unwrap_or([0; NAME_MAX]),
				self.lens.get(i + 1).copied().unwrap_or(0),
			);
			if let (Some(d), Some(dl)) = (self.names.get_mut(i), self.lens.get_mut(i)) {
				*d = next;
				*dl = nlen;
			}
		}
	}
}

/// Shortens `path` to at most `max` bytes for a menu, the way Windows does: the drive and the end of the path stay and the middle
/// becomes `...`, as in `D:\...\Letters\LETTER.QLD`. The tail starts at a backslash when one allows it, so the file name is whole
/// whenever it fits. A path that fits comes back as it is. Returns how many bytes of `out` it wrote (at most `out.len()`).
pub fn shorten_path(path: &[u8], max: usize, out: &mut [u8]) -> usize {
	let max = max.min(out.len());
	let mut n = 0;
	let mut put = |bytes: &[u8], n: &mut usize| {
		for b in bytes {
			if let Some(slot) = out.get_mut(*n) {
				*slot = *b;
				*n += 1;
			}
		}
	};
	if path.len() <= max || max < 8 {
		put(path.get(..path.len().min(max)).unwrap_or(&[]), &mut n);
		return n;
	}
	// The head is the drive and its root, or a UNC server and share: up to and including the first backslash that follows the
	// first component.
	let mut head = 0;
	let slashes = if path.starts_with(b"\\\\") { 3 } else { 1 };
	let mut seen = 0;
	for (i, b) in path.iter().enumerate() {
		if *b == b'\\' {
			seen += 1;
			if seen == slashes {
				head = i + 1;
				break;
			}
		}
	}
	if head + 4 >= max {
		head = 0;
	}
	let room = max - head - 4;
	// The longest tail that fits and begins just after a backslash; failing that, the last `room` bytes.
	let mut from = path.len() - room;
	if let Some(rel) = path
		.get(from..)
		.and_then(|t| t.iter().position(|b| *b == b'\\'))
	{
		if from + rel + 1 < path.len() {
			from = from + rel + 1;
		}
	}
	let from = from.max(head);
	put(path.get(..head).unwrap_or(&[]), &mut n);
	put(b"...\\", &mut n);
	put(path.get(from..).unwrap_or(&[]), &mut n);
	n
}
