// DOS 8.3 names and paths: validation, wildcards, the reserved device names, and joining and resolving paths.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Names are shown upper case, as DOS returns them. A name that is not a valid 8.3 name is refused, never shortened: DOS would
// turn `LONGFILENAME.QLD` into `LONGFILE.QLD` and could overwrite another file with it.

/// The longest path the dialogs accept, drive and name included. A DOS path is limited to 64 bytes for the folder and a file
/// name; 79 leaves room for both and still fits one text-mode row.
pub const PATH_MAX: usize = 79;
/// The longest base name.
pub const BASE_MAX: usize = 8;
/// The longest extension.
pub const EXT_MAX: usize = 3;

/// Why a name or path was refused.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum NameError {
	/// Nothing was typed.
	Empty,
	/// A control character, a character above `~`, or one of `" * + , / : ; < = > ? [ \ ] |` (the byte is in the variant).
	BadChar(u8),
	/// The name starts with a dot, so its base is empty.
	LeadingDot,
	/// More than one dot.
	TwoDots,
	/// A base over eight bytes.
	BaseTooLong,
	/// An extension over three bytes.
	ExtTooLong,
	/// The base is a device name such as `CON` or `LPT1`, whatever the extension.
	Reserved,
	/// The path would be longer than [`PATH_MAX`].
	PathTooLong,
	/// `..` at the root.
	AboveRoot,
	/// A drive letter that is not a letter.
	BadDrive,
	/// A path with an empty part, such as `A\\B`.
	EmptyPart,
}

/// A path or name of at most [`PATH_MAX`] bytes, held in place.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct PathBuf {
	bytes: [u8; PATH_MAX],
	len: u8,
}

impl Default for PathBuf {
	fn default() -> Self {
		PathBuf::new()
	}
}

impl PathBuf {
	/// An empty buffer.
	pub const fn new() -> Self {
		PathBuf {
			bytes: [0; PATH_MAX],
			len: 0,
		}
	}

	/// Copies `b`, or `None` if it is longer than [`PATH_MAX`].
	pub fn from_bytes(b: &[u8]) -> Option<PathBuf> {
		let mut p = PathBuf::new();
		if p.push_bytes(b) {
			Some(p)
		} else {
			None
		}
	}

	/// The root folder of a drive, `C:\`. A byte that is not a letter gives `A:\`.
	pub fn root(drive: u8) -> PathBuf {
		let d = if drive.is_ascii_alphabetic() {
			drive.to_ascii_uppercase()
		} else {
			b'A'
		};
		let mut p = PathBuf::new();
		let _ = p.push_bytes(&[d, b':', b'\\']);
		p
	}

	/// The bytes held.
	pub fn as_bytes(&self) -> &[u8] {
		self.bytes.get(..usize::from(self.len)).unwrap_or(&[])
	}

	pub fn len(&self) -> usize {
		usize::from(self.len)
	}

	pub fn is_empty(&self) -> bool {
		self.len == 0
	}

	/// Appends `b`; false (and no change) when it would not fit.
	pub fn push_bytes(&mut self, b: &[u8]) -> bool {
		let at = self.len();
		let end = at + b.len();
		if end > PATH_MAX {
			return false;
		}
		match self.bytes.get_mut(at..end) {
			Some(dst) => dst.copy_from_slice(b),
			None => return false,
		}
		self.len = end as u8;
		true
	}

	/// Appends one byte; false when full.
	pub fn push_byte(&mut self, b: u8) -> bool {
		self.push_bytes(&[b])
	}

	/// Keeps the first `n` bytes.
	pub fn truncate(&mut self, n: usize) {
		if n < self.len() {
			self.len = n as u8;
		}
	}

	pub fn clear(&mut self) {
		self.len = 0;
	}

	/// The drive letter (upper case) when the path starts with `X:`.
	pub fn drive(&self) -> Option<u8> {
		match self.as_bytes() {
			[d, b':', ..] if d.is_ascii_alphabetic() => Some(d.to_ascii_uppercase()),
			_ => None,
		}
	}

	/// True for `C:\`.
	pub fn is_root(&self) -> bool {
		matches!(self.as_bytes(), [d, b':', b'\\'] if d.is_ascii_alphabetic())
	}

	/// The last part after the final backslash (the whole path when there is none, empty for a root).
	pub fn file_name(&self) -> &[u8] {
		last_part(self.as_bytes())
	}

	/// The folder this path is in: `C:\DOCS\A.QLD` gives `C:\DOCS`, `C:\DOCS` gives `C:\`. `None` at a root or when the path has
	/// no backslash.
	pub fn parent(&self) -> Option<PathBuf> {
		let b = self.as_bytes();
		if self.is_root() {
			return None;
		}
		let cut = b.iter().rposition(|c| *c == b'\\')?;
		let mut p = PathBuf::new();
		// `C:\DOCS` cuts at 2: keep the backslash so the result is the root.
		let keep = if cut <= 2 { cut + 1 } else { cut };
		let _ = p.push_bytes(b.get(..keep)?);
		Some(p)
	}

	/// This folder with `part` below it. `part` must be a valid 8.3 name (folders follow the same rule); it is upper-cased.
	pub fn join(&self, part: &[u8]) -> Result<PathBuf, NameError> {
		let name = Name83::parse(part)?;
		let mut p = *self;
		if !p.is_root() && !p.is_empty() && !p.push_byte(b'\\') {
			return Err(NameError::PathTooLong);
		}
		if !p.push_bytes(name.as_bytes()) {
			return Err(NameError::PathTooLong);
		}
		Ok(p)
	}
}

/// The part after the last `\` or `:`.
pub fn last_part(path: &[u8]) -> &[u8] {
	match path.iter().rposition(|c| *c == b'\\' || *c == b':') {
		Some(i) => path.get(i + 1..).unwrap_or(&[]),
		None => path,
	}
}

/// True when `base` (any case) is a reserved device name: `CON PRN AUX NUL CLOCK$` and `COM1` to `COM9`, `LPT1` to `LPT9`.
pub fn is_device(base: &[u8]) -> bool {
	const NAMED: [&[u8]; 5] = [b"CON", b"PRN", b"AUX", b"NUL", b"CLOCK$"];
	if NAMED.iter().any(|d| base.eq_ignore_ascii_case(d)) {
		return true;
	}
	match base {
		[a, b, c, d] => {
			let head = [*a, *b, *c];
			(head.eq_ignore_ascii_case(b"COM") || head.eq_ignore_ascii_case(b"LPT"))
				&& (b'1'..=b'9').contains(d)
		}
		_ => false,
	}
}

/// True for `LPT1` and `LPT2`, the two ports the Print dialog accepts.
fn is_printer_port(base: &[u8]) -> bool {
	base.eq_ignore_ascii_case(b"LPT1") || base.eq_ignore_ascii_case(b"LPT2")
}

fn bad_char(b: u8) -> bool {
	!(0x21..=0x7E).contains(&b) || b"\"*+,/:;<=>?[\\]|".contains(&b)
}

/// A valid 8.3 name, upper-cased: `BASE` or `BASE.EXT`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Name83 {
	bytes: [u8; BASE_MAX + 1 + EXT_MAX],
	base_len: u8,
	ext_len: u8,
	dot: bool,
	/// The text ended in a dot (`LETTER.`): the name has no extension and none is to be added.
	pub bare_dot: bool,
}

impl Name83 {
	/// Checks `input` as a file name. Wildcards are refused here; a caller that accepts them checks [`is_wild`] first.
	pub fn parse(input: &[u8]) -> Result<Name83, NameError> {
		Self::parse_with(input, false)
	}

	/// As [`Name83::parse`], but `LPT1` and `LPT2` pass: the Print dialog's file name box.
	pub fn parse_printer(input: &[u8]) -> Result<Name83, NameError> {
		Self::parse_with(input, true)
	}

	fn parse_with(input: &[u8], printer: bool) -> Result<Name83, NameError> {
		if input.is_empty() {
			return Err(NameError::Empty);
		}
		if let Some(b) = input.iter().find(|b| bad_char(**b)) {
			return Err(NameError::BadChar(*b));
		}
		let (base, rest) = match input.iter().position(|b| *b == b'.') {
			Some(i) => (input.get(..i).unwrap_or(&[]), input.get(i + 1..)),
			None => (input, None),
		};
		let ext = rest.unwrap_or(&[]);
		if ext.contains(&b'.') {
			return Err(NameError::TwoDots);
		}
		if base.is_empty() {
			return Err(NameError::LeadingDot);
		}
		if base.len() > BASE_MAX {
			return Err(NameError::BaseTooLong);
		}
		if ext.len() > EXT_MAX {
			return Err(NameError::ExtTooLong);
		}
		if is_device(base) && !(printer && is_printer_port(base)) {
			return Err(NameError::Reserved);
		}
		let mut n = Name83 {
			bytes: [0; BASE_MAX + 1 + EXT_MAX],
			base_len: base.len() as u8,
			ext_len: ext.len() as u8,
			dot: !ext.is_empty(),
			bare_dot: rest.is_some() && ext.is_empty(),
		};
		let mut at = 0;
		let mut put = |b: u8, bytes: &mut [u8]| {
			if let Some(slot) = bytes.get_mut(at) {
				*slot = b.to_ascii_uppercase();
			}
			at += 1;
		};
		for b in base {
			put(*b, &mut n.bytes);
		}
		if n.dot {
			put(b'.', &mut n.bytes);
			for b in ext {
				put(*b, &mut n.bytes);
			}
		}
		Ok(n)
	}

	/// `BASE.EXT`, or `BASE` when there is no extension.
	pub fn as_bytes(&self) -> &[u8] {
		let n = usize::from(self.base_len)
			+ if self.dot {
				1 + usize::from(self.ext_len)
			} else {
				0
			};
		self.bytes.get(..n).unwrap_or(&[])
	}

	/// The base, without the dot.
	pub fn base(&self) -> &[u8] {
		self.bytes.get(..usize::from(self.base_len)).unwrap_or(&[])
	}

	/// The extension, without the dot (empty when there is none).
	pub fn ext(&self) -> &[u8] {
		let start = usize::from(self.base_len) + 1;
		self.bytes
			.get(start..start + usize::from(self.ext_len))
			.unwrap_or(&[])
	}
}

/// True when `s` has a `*` or a `?`.
pub fn is_wild(s: &[u8]) -> bool {
	s.iter().any(|b| *b == b'*' || *b == b'?')
}

/// Matches one part of a name (the base or the extension) with DOS's rules: `*` is any run, `?` is one character or, at the end,
/// none. Case does not matter.
fn part_match(p: &[u8], n: &[u8]) -> bool {
	let (mut pi, mut ni) = (0usize, 0usize);
	let mut star: Option<(usize, usize)> = None;
	loop {
		let pc = p.get(pi).copied();
		let nc = n.get(ni).copied();
		match (pc, nc) {
			(Some(b'*'), _) => {
				pi += 1;
				star = Some((pi, ni));
				continue;
			}
			(Some(b'?'), Some(_)) => {
				pi += 1;
				ni += 1;
				continue;
			}
			(Some(b'?'), None) => {
				pi += 1;
				continue;
			}
			(Some(a), Some(b)) if a.eq_ignore_ascii_case(&b) => {
				pi += 1;
				ni += 1;
				continue;
			}
			(None, None) => return true,
			_ => {}
		}
		// A mismatch: let the last `*` take one more character, if there is one.
		match star {
			Some((sp, sn)) if sn < n.len() => {
				star = Some((sp, sn + 1));
				pi = sp;
				ni = sn + 1;
			}
			_ => return false,
		}
	}
}

fn split_dot(s: &[u8]) -> (&[u8], Option<&[u8]>) {
	match s.iter().position(|b| *b == b'.') {
		Some(i) => (s.get(..i).unwrap_or(&[]), s.get(i + 1..)),
		None => (s, None),
	}
}

/// Matches `name` against one pattern: the base and the extension are matched separately, as DOS does. A pattern with no dot
/// matches only names with no extension, unless it holds a `*`, which then spans the extension too (`A*` is `A*.*`).
pub fn wild_match(pattern: &[u8], name: &[u8]) -> bool {
	let (pb, pe) = split_dot(pattern);
	let (nb, ne) = split_dot(name);
	let pe = match pe {
		Some(e) => e,
		None if pb.contains(&b'*') => b"*",
		None => b"",
	};
	part_match(pb, nb) && part_match(pe, ne.unwrap_or(&[]))
}

/// Matches `name` against a list of patterns separated by `;`, as in `*.BMP;*.PNG`.
pub fn matches_any(patterns: &[u8], name: &[u8]) -> bool {
	patterns
		.split(|b| *b == b';')
		.any(|p| !p.is_empty() && wild_match(p, name))
}

/// What [`resolve`] made of the text typed.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Resolved {
	/// The full path, upper case, with no `.` or `..` parts and no trailing backslash (except a root).
	pub path: PathBuf,
	/// The text named a folder by its form: it ended in `\`, in `.` or `..`, or was only a drive or a root.
	pub dir_hint: bool,
}

/// Makes the text typed into a full path. `cwd` is the folder the dialog is in; `drive_cwd` gives the current folder of another
/// drive for `D:NAME`, or `None` when that drive cannot be asked. `.` and `..` are resolved here and never reach the file system.
/// The text must hold no wildcard: the caller takes those as a pattern first.
pub fn resolve<F: FnMut(u8) -> Option<PathBuf>>(
	cwd: &PathBuf,
	input: &[u8],
	mut drive_cwd: F,
) -> Result<Resolved, NameError> {
	if input.is_empty() {
		return Err(NameError::Empty);
	}
	let mut rest = input;
	let mut base = *cwd;
	let mut dir_hint = false;
	if let [d, b':', tail @ ..] = rest {
		if !d.is_ascii_alphabetic() {
			return Err(NameError::BadDrive);
		}
		let d = d.to_ascii_uppercase();
		rest = tail;
		dir_hint = true;
		if cwd.drive() != Some(d) {
			base = drive_cwd(d).unwrap_or_else(|| PathBuf::root(d));
		}
	}
	if let [b'\\', tail @ ..] = rest {
		base = PathBuf::root(base.drive().unwrap_or(b'C'));
		rest = tail;
		dir_hint = true;
	}
	let mut parts = rest.split(|b| *b == b'\\').peekable();
	while let Some(part) = parts.next() {
		let last = parts.peek().is_none();
		match part {
			[] if last => dir_hint = true,
			[] => return Err(NameError::EmptyPart),
			[b'.'] => dir_hint = last,
			[b'.', b'.'] => {
				base = base.parent().ok_or(NameError::AboveRoot)?;
				dir_hint = last;
			}
			_ => {
				if base.drive().is_none() {
					return Err(NameError::BadDrive);
				}
				base = base.join(part)?;
				dir_hint = false;
			}
		}
	}
	if base.drive().is_none() {
		return Err(NameError::BadDrive);
	}
	Ok(Resolved {
		path: base,
		dir_hint,
	})
}

#[cfg(test)]
#[allow(
	clippy::unwrap_used,
	clippy::expect_used,
	clippy::panic,
	clippy::indexing_slicing
)]
mod tests {
	use super::*;
	use proptest::prelude::*;
	use std::vec::Vec;

	fn ok(s: &str) -> Name83 {
		Name83::parse(s.as_bytes()).unwrap()
	}

	#[test]
	fn names_are_upper_cased_and_split() {
		let n = ok("letter.qld");
		assert_eq!(n.as_bytes(), b"LETTER.QLD");
		assert_eq!(n.base(), b"LETTER");
		assert_eq!(n.ext(), b"QLD");
		assert_eq!(ok("README").as_bytes(), b"README");
		assert_eq!(ok("README").ext(), b"");
		assert!(ok("A.B").as_bytes() == b"A.B");
		assert!(ok("~a$-_.1").as_bytes() == b"~A$-_.1");
	}

	#[test]
	fn a_trailing_dot_means_no_extension() {
		let n = ok("letter.");
		assert_eq!(n.as_bytes(), b"LETTER");
		assert!(n.bare_dot);
		assert!(!ok("letter").bare_dot);
	}

	#[test]
	fn invalid_names_are_refused() {
		let e = |s: &str| Name83::parse(s.as_bytes()).unwrap_err();
		assert_eq!(e(""), NameError::Empty);
		assert_eq!(e("LONGFILENAME.QLD"), NameError::BaseTooLong);
		assert_eq!(e("A.LONG"), NameError::ExtTooLong);
		assert_eq!(e("A.B.C"), NameError::TwoDots);
		assert_eq!(e(".QLD"), NameError::LeadingDot);
		assert_eq!(e("."), NameError::LeadingDot);
		assert_eq!(e("A B"), NameError::BadChar(b' '));
		assert_eq!(e("A*B"), NameError::BadChar(b'*'));
		assert_eq!(e("A|B"), NameError::BadChar(b'|'));
		assert_eq!(e("A/B"), NameError::BadChar(b'/'));
		assert_eq!(e("A\\B"), NameError::BadChar(b'\\'));
		assert_eq!(e("A\u{e9}"), NameError::BadChar(0xC3));
		assert_eq!(e("A\x7f"), NameError::BadChar(0x7F));
	}

	#[test]
	fn device_names_are_refused_whatever_the_extension() {
		for s in [
			"CON", "con.txt", "Prn.QLD", "AUX", "NUL.", "COM1", "com9.x", "LPT1", "lpt9.qld",
			"CLOCK$",
		] {
			assert_eq!(
				Name83::parse(s.as_bytes()).unwrap_err(),
				NameError::Reserved,
				"{s}"
			);
		}
		for s in [
			"COM0", "LPT0", "COM10", "CONSOLE", "COMX", "LPT", "CLOCK", "CON1",
		] {
			assert!(Name83::parse(s.as_bytes()).is_ok(), "{s}");
		}
	}

	#[test]
	fn the_print_dialog_allows_two_ports_only() {
		assert!(Name83::parse_printer(b"lpt1").is_ok());
		assert!(Name83::parse_printer(b"LPT2").is_ok());
		assert_eq!(
			Name83::parse_printer(b"LPT3").unwrap_err(),
			NameError::Reserved
		);
		assert_eq!(
			Name83::parse_printer(b"CON").unwrap_err(),
			NameError::Reserved
		);
	}

	#[test]
	fn wildcards() {
		assert!(wild_match(b"*.*", b"A.QLD"));
		assert!(wild_match(b"*.*", b"README"));
		assert!(wild_match(b"*", b"README"));
		assert!(wild_match(b"*", b"A.QLD"));
		assert!(wild_match(b"*.QLD", b"letter.qld"));
		assert!(!wild_match(b"*.QLD", b"LETTER.RTF"));
		assert!(!wild_match(b"*.QLD", b"QLD"));
		assert!(wild_match(b"L*R.QLD", b"LETTER.QLD"));
		assert!(wild_match(b"LETTER?.QLD", b"LETTER.QLD"));
		assert!(wild_match(b"LETTER?.QLD", b"LETTER1.QLD"));
		assert!(!wild_match(b"LETTER?.QLD", b"LETTER12.QLD"));
		assert!(wild_match(b"A?C", b"ABC"));
		assert!(!wild_match(b"A?C", b"AC"));
		assert!(wild_match(b"A*", b"ABC.TXT"));
		assert!(!wild_match(b"ABC", b"ABC.TXT"));
		assert!(wild_match(b"ABC.", b"ABC"));
		assert!(wild_match(b"*.?", b"A.B"));
		assert!(wild_match(b"*.?", b"A"));
		assert!(wild_match(b"*A*A*", b"BANANA"));
		assert!(matches_any(b"*.BMP;*.PNG", b"x.png"));
		assert!(matches_any(b"*.BMP;*.PNG", b"x.bmp"));
		assert!(!matches_any(b"*.BMP;*.PNG", b"x.gif"));
		assert!(!matches_any(b"", b"x.gif"));
		assert!(!matches_any(b";;", b"x.gif"));
	}

	fn cwd() -> PathBuf {
		PathBuf::from_bytes(b"C:\\DOCS\\LETTERS").unwrap()
	}

	fn res(s: &str) -> Result<Resolved, NameError> {
		resolve(&cwd(), s.as_bytes(), |d| {
			(d == b'D').then(|| PathBuf::from_bytes(b"D:\\WORK").unwrap())
		})
	}

	#[test]
	fn paths_resolve() {
		let r = res("memo.qld").unwrap();
		assert_eq!(r.path.as_bytes(), b"C:\\DOCS\\LETTERS\\MEMO.QLD");
		assert!(!r.dir_hint);
		assert_eq!(res("..").unwrap().path.as_bytes(), b"C:\\DOCS");
		assert!(res("..").unwrap().dir_hint);
		assert_eq!(res("..\\..").unwrap().path.as_bytes(), b"C:\\");
		assert_eq!(res("..\\..\\..").unwrap_err(), NameError::AboveRoot);
		assert_eq!(res("\\").unwrap().path.as_bytes(), b"C:\\");
		assert_eq!(res("\\docs").unwrap().path.as_bytes(), b"C:\\DOCS");
		assert_eq!(
			res("old\\").unwrap().path.as_bytes(),
			b"C:\\DOCS\\LETTERS\\OLD"
		);
		assert!(res("old\\").unwrap().dir_hint);
		assert_eq!(res("c:").unwrap().path.as_bytes(), b"C:\\DOCS\\LETTERS");
		assert!(res("c:").unwrap().dir_hint);
		assert_eq!(res("d:").unwrap().path.as_bytes(), b"D:\\WORK");
		assert_eq!(res("d:a.txt").unwrap().path.as_bytes(), b"D:\\WORK\\A.TXT");
		assert_eq!(res("e:\\x").unwrap().path.as_bytes(), b"E:\\X");
		assert_eq!(res("e:").unwrap().path.as_bytes(), b"E:\\");
		assert_eq!(
			res(".\\a.txt").unwrap().path.as_bytes(),
			b"C:\\DOCS\\LETTERS\\A.TXT"
		);
		assert_eq!(res("a\\\\b").unwrap_err(), NameError::EmptyPart);
		assert_eq!(res("1:x").unwrap_err(), NameError::BadDrive);
		assert_eq!(res("con.txt").unwrap_err(), NameError::Reserved);
		assert_eq!(res("a\\con").unwrap_err(), NameError::Reserved);
		assert_eq!(res("").unwrap_err(), NameError::Empty);
	}

	#[test]
	fn a_path_over_the_limit_is_refused() {
		let mut p = PathBuf::root(b'C');
		let mut refused = false;
		for _ in 0..20 {
			match p.join(b"ABCDEFGH") {
				Ok(q) => p = q,
				Err(e) => {
					assert_eq!(e, NameError::PathTooLong);
					refused = true;
					break;
				}
			}
		}
		assert!(refused);
		assert!(p.len() <= PATH_MAX);
	}

	#[test]
	fn parent_and_join_agree() {
		let p = PathBuf::from_bytes(b"C:\\DOCS\\A.QLD").unwrap();
		assert_eq!(p.parent().unwrap().as_bytes(), b"C:\\DOCS");
		assert_eq!(p.parent().unwrap().parent().unwrap().as_bytes(), b"C:\\");
		assert!(p.parent().unwrap().parent().unwrap().parent().is_none());
		assert_eq!(p.file_name(), b"A.QLD");
		assert_eq!(PathBuf::root(b'c').drive(), Some(b'C'));
	}

	fn part() -> impl Strategy<Value = Vec<u8>> {
		(
			"[A-Za-z0-9_~$-]{1,8}",
			proptest::option::of("[A-Za-z0-9]{1,3}"),
		)
			.prop_map(|(b, e)| {
				let mut v = b.into_bytes();
				if let Some(e) = e {
					v.push(b'.');
					v.extend_from_slice(e.as_bytes());
				}
				v
			})
	}

	proptest! {
		#[test]
		fn parse_never_panics_and_ok_names_are_valid(input in proptest::collection::vec(any::<u8>(), 0..40)) {
			if let Ok(n) = Name83::parse(&input) {
				let b = n.as_bytes();
				prop_assert!(b.len() <= 12);
				prop_assert!(!n.base().is_empty() && n.base().len() <= BASE_MAX);
				prop_assert!(n.ext().len() <= EXT_MAX);
				prop_assert!(b.iter().all(|c| !bad_char(*c) && !c.is_ascii_lowercase()));
				prop_assert!(!is_device(n.base()));
				// Parsing the canonical form gives the same name.
				let again = Name83::parse(b).unwrap();
				prop_assert_eq!(again.as_bytes(), b);
				prop_assert!(!again.bare_dot);
			}
		}

		#[test]
		fn a_valid_name_matches_itself_and_its_wildcards(p in part()) {
			let n = Name83::parse(&p).unwrap();
			prop_assert!(wild_match(n.as_bytes(), n.as_bytes()));
			prop_assert!(wild_match(b"*.*", n.as_bytes()));
			prop_assert!(wild_match(b"*", n.as_bytes()));
			let mut by_ext = *b"*.\0\0\0";
			let ext = n.ext();
			if !ext.is_empty() {
				by_ext[2..2 + ext.len()].copy_from_slice(ext);
				prop_assert!(wild_match(&by_ext[..2 + ext.len()], n.as_bytes()));
			}
		}

		#[test]
		fn wildcards_never_panic(pat in proptest::collection::vec(prop_oneof![Just(b'*'), Just(b'?'), Just(b'.'), Just(b';'), 0x41u8..0x44], 0..12),
			name in proptest::collection::vec(prop_oneof![Just(b'.'), 0x41u8..0x44], 0..12)) {
			let _ = wild_match(&pat, &name);
			let _ = matches_any(&pat, &name);
		}

		#[test]
		fn join_and_parent_round_trip(parts in proptest::collection::vec(part(), 0..6)) {
			let mut p = PathBuf::root(b'D');
			let mut chain = [PathBuf::root(b'D'); 8];
			let mut n = 0;
			for part in &parts {
				match p.join(part) {
					Ok(q) => {
						p = q;
						n += 1;
						chain[n] = q;
					}
					Err(e) => {
						prop_assert_eq!(e, NameError::PathTooLong);
						break;
					}
				}
			}
			// Walking up retraces the chain exactly.
			while n > 0 {
				let up = p.parent().unwrap();
				n -= 1;
				prop_assert_eq!(up, chain[n]);
				p = up;
			}
			prop_assert!(p.is_root());
			prop_assert!(p.parent().is_none());
		}

		#[test]
		fn resolve_never_panics_and_ok_paths_are_clean(input in proptest::collection::vec(
			prop_oneof![Just(b'\\'), Just(b'.'), Just(b':'), 0x41u8..0x45, 0x2Du8..0x30], 0..30)) {
			if let Ok(r) = resolve(&cwd(), &input, |d| PathBuf::from_bytes(&[d, b':', b'\\', b'W']))
			{
				let b = r.path.as_bytes();
				prop_assert!(b.len() <= PATH_MAX);
				prop_assert!(r.path.drive().is_some());
				prop_assert!(b.get(2) == Some(&b'\\'));
				for comp in b.get(3..).unwrap_or(&[]).split(|c| *c == b'\\').filter(|c| !c.is_empty()) {
					prop_assert!(comp != b"." && comp != b"..");
					prop_assert!(Name83::parse(comp).is_ok());
				}
				prop_assert!(b.len() == 3 || b.last() != Some(&b'\\'));
			}
		}
	}
}
