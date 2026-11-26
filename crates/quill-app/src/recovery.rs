// Recovery files: where they live and what they are called, the list in QUILL.INI, the lock note, the two-copy rule and the questions.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// A recovery file is a `.QLD` with one `RCOV` chunk (`quill_formats::qld::write_recovery`), written beside the document it protects, or
// in a central folder for a document with no name and for one whose folder cannot take it. `App::save_recovery` is the one writer:
// autosave, the end of a session and the emergency copy share it (CORE-11). At most two copies of a document exist, the current one and
// one set-aside copy (CORE-12). Two questions put the copies back in the person's hands, one at launch and one at Open (CORE-13), and a
// lock note tells a second window that the first has the document open with changes (CORE-14).
//
// Everything here is off until a shell turns it on with `set_recovery`: a shell that has no way to ask the questions keeps writing its
// end-of-session copy and nothing else.
use quill_core::layout::Measure;
use quill_core::store::{ChunkId, ReadStore, Store};
use quill_formats::io::Source;
use quill_formats::qld::{
	self, Recovery, RCV_AUTOSAVE, RCV_CENTRAL, RCV_EMERGENCY, RCV_END_OF_SESSION, RCV_UNTITLED,
};

use crate::flow::{words, Answer, Ask, Buttons, Text, ASK_MAX};
use crate::{datefmt, safe, App, Drive, Files, LockMade, Owner, NAME_MAX};

/// The originals the `QUILL.INI` list keeps, newest first.
pub const LIST_MAX: usize = 8;
/// The status-line words a shell may keep in its own resources: `Files::string` asks for them by these numbers.
pub const STR_DAMAGED: u8 = 1;
/// See `STR_DAMAGED`.
pub const STR_CANNOT_AUTOSAVE: u8 = 2;
/// See `STR_DAMAGED`.
pub const STR_SECOND_WINDOW: u8 = 3;
/// See `STR_DAMAGED`. The start of `Autosaving LETTER.QLD...`, said before a write that input cannot cut short.
pub const STR_AUTOSAVING: u8 = 4;

/// The words for `id` (`STR_*`) written into `out`, and their length: what `Files::string` gives when the shell has no resources of
/// its own. A shell that overrides `string` does not link these.
pub fn default_string(id: u8, out: &mut [u8]) -> usize {
	let text = match id {
		STR_DAMAGED => words::DAMAGED_RECOVERY,
		STR_CANNOT_AUTOSAVE => words::CANNOT_AUTOSAVE,
		STR_SECOND_WINDOW => words::SECOND_WINDOW_STATUS,
		STR_AUTOSAVING => words::AUTOSAVING,
		_ => "",
	};
	let n = text.len().min(out.len());
	if let (Some(d), Some(s)) = (out.get_mut(..n), text.as_bytes().get(..n)) {
		d.copy_from_slice(s);
	}
	n
}

/// How many hash values a writer tries before it gives a slot up as taken.
const PROBES: u8 = 8;
/// The room for a recovery file's whole path: a folder, the longest name and its suffix.
pub const PATH_MAX: usize = NAME_MAX + 40;

// The bits of `State::flags`.
/// A shell has turned the recovery protocol on: the questions, the lock note and the list.
const ENABLED: u16 = 1;
/// The names are dot-files (`.~Quill-NAME.rcv`), as the Linux builds keep them.
const DOT: u16 = 1 << 1;
/// This session wrote, or adopted, the current copy.
const WROTE: u16 = 1 << 2;
/// A current copy that this session did not write is on the disk and has been left alone: the person chose Open original.
const LEFT: u16 = 1 << 3;
/// A set-aside copy exists.
const ASIDE: u16 = 1 << 4;
/// This window holds the document's lock note.
const LOCKED: u16 = 1 << 5;
/// Another window has the document open: nothing is written for it from here.
const SECOND: u16 = 1 << 6;
/// The launch question is waiting.
const Q_LAUNCH: u16 = 1 << 7;
/// The Open question is waiting.
const Q_OPEN: u16 = 1 << 8;
/// The second-window question is waiting.
const Q_SECOND: u16 = 1 << 9;
/// The waiting question has been handed to the shell.
const ASKED: u16 = 1 << 10;
/// The recovered document came from the old `QUILL.RCV`, which goes with the next save.
const LEGACY: u16 = 1 << 11;
/// The disk-full message has been said this session.
const SAID_FULL: u16 = 1 << 12;
/// `App::folder` holds the path of a deleted original, which Save As offers.
pub(crate) const SUGGEST: u16 = 1 << 13;
/// A recovered document has been opened this session, so no more launch questions are asked.
const RECOVERED: u16 = 1 << 14;
/// What a document's own state leaves out when the document goes.
const KEEP: u16 = ENABLED | DOT | SAID_FULL | RECOVERED;
const PENDING: u16 = Q_LAUNCH | Q_OPEN | Q_SECOND;

/// What the editor keeps about recovery: a few bytes, since the Windows 3.x data segment has no room for more. The list, the central
/// folder and the question's file name are in one `Store` chunk.
#[derive(Copy, Clone)]
pub(crate) struct State {
	list: Option<ChunkId>,
	pub(crate) flags: u16,
	session: u32,
	pub(crate) orig_size: u32,
	pub(crate) orig_time: u32,
	interval: u8,
	interval_default: u8,
	writer: u8,
	skip: u8,
	debug_seconds: u16,
}

impl State {
	pub(crate) const fn new() -> State {
		State {
			list: None,
			flags: 0,
			session: 0,
			orig_size: u32::MAX,
			orig_time: 0,
			interval: 10,
			interval_default: 10,
			writer: 0,
			skip: 0,
			debug_seconds: 0,
		}
	}
}

/// Why a recovery copy is written.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Why {
	/// The autosave timer.
	Autosave,
	/// The session is ending, or a shutdown was refused.
	EndOfSession,
	/// The program is going down after an internal error; the copy never replaces a good one.
	Emergency,
}

/// Asks whether input waits while a recovery copy is written. `NoInput` never does and costs nothing; any `FnMut() -> bool` is asked about
/// every `qld::PROGRESS_STEP` bytes.
pub trait Input {
	/// True when the type polls, so a build that never uses one does not link the hook.
	const POLLS: bool;
	/// True when a key or click waits.
	fn waiting(&mut self) -> bool;
}

/// The write is never cut short.
pub struct NoInput;

impl Input for NoInput {
	const POLLS: bool = false;
	fn waiting(&mut self) -> bool {
		false
	}
}

impl<F: FnMut() -> bool> Input for F {
	const POLLS: bool = true;
	fn waiting(&mut self) -> bool {
		self()
	}
}

/// How a write of a recovery copy ended.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Wrote {
	/// The copy is on the disk.
	Done,
	/// Input was waiting, so the write stopped and the disk is as it was.
	Abandoned,
	/// Nothing was written: the document is clean, another window has it, or no place took the file.
	Failed,
}

// ---- names -----------------------------------------------------------------------------------------------------------------------

/// A path built in a fixed buffer.
#[derive(Copy, Clone)]
pub struct Path {
	buf: [u8; PATH_MAX],
	len: usize,
	over: bool,
}

impl Path {
	/// The empty path.
	pub const fn new() -> Path {
		Path {
			buf: [0; PATH_MAX],
			len: 0,
			over: false,
		}
	}

	fn of(b: &[u8]) -> Path {
		let mut p = Path::new();
		p.push(b);
		p
	}

	fn push(&mut self, b: &[u8]) -> &mut Self {
		for &c in b {
			match self.buf.get_mut(self.len) {
				Some(slot) => {
					*slot = c;
					self.len += 1;
				}
				None => self.over = true,
			}
		}
		self
	}

	fn hex(&mut self, v: u32, digits: usize) -> &mut Self {
		for i in (0..digits).rev() {
			let d = ((v >> (i * 4)) & 15) as u8;
			self.push(&[if d < 10 { b'0' + d } else { b'A' + d - 10 }]);
		}
		self
	}

	/// The bytes.
	pub fn bytes(&self) -> &[u8] {
		self.buf.get(..self.len).unwrap_or(&[])
	}

	fn done(self) -> Option<Path> {
		(!self.over).then_some(self)
	}
}

impl Default for Path {
	fn default() -> Self {
		Path::new()
	}
}

/// How recovery files are named.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Style {
	/// 8.3 names: `~LETT3A7.RCV`.
	Dos,
	/// Long names: `~Quill-LETTER.QLD.rcv`.
	Long,
	/// Long names that are dot-files: `.~Quill-letter.qld.rcv`.
	Dot,
}

impl Style {
	/// The style of the build: 8.3 names, or long ones where the build keeps long names.
	pub const fn native() -> Style {
		if cfg!(feature = "long-names") {
			Style::Long
		} else {
			Style::Dos
		}
	}
}

/// The files that belong to one recovery slot.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Kind {
	/// The current copy.
	Current,
	/// The set-aside copy.
	Aside,
	/// What a write goes to before it replaces the current copy.
	Temp,
	/// The lock note.
	Lock,
	/// The copy written after an internal error.
	Emergency,
	/// A copy that failed its check, put out of the way.
	Bad,
}

fn ext(style: Style, kind: Kind) -> &'static [u8] {
	match (style == Style::Dos, kind) {
		(true, Kind::Current) => b".RCV",
		(true, Kind::Aside) => b".RC0",
		(true, Kind::Temp) => b".$$$",
		(true, Kind::Lock) => b".LCK",
		(true, Kind::Emergency) => b".RCE",
		(true, Kind::Bad) => b".BAD",
		(false, Kind::Current) => b".rcv",
		(false, Kind::Aside) => b".rcv.old",
		(false, Kind::Temp) => b".rcv.$$$",
		(false, Kind::Lock) => b".lck",
		(false, Kind::Emergency) => b".rce",
		(false, Kind::Bad) => b".rcv.bad",
	}
}

fn sep(style: Style) -> u8 {
	if style == Style::Dot {
		b'/'
	} else {
		b'\\'
	}
}

fn is_sep(b: u8) -> bool {
	matches!(b, b'\\' | b'/' | b':')
}

/// Where the file name starts in `path`.
pub(crate) fn base_at(path: &[u8]) -> usize {
	path.iter().rposition(|b| is_sep(*b)).map_or(0, |i| i + 1)
}

/// The file name of `path`, without its folder.
pub fn base_name(path: &[u8]) -> &[u8] {
	path.get(base_at(path)..).unwrap_or(&[])
}

fn same(a: &[u8], b: &[u8]) -> bool {
	a.eq_ignore_ascii_case(b)
}

/// FNV-1a over the upper-cased bytes, so `a.qld` and `A.QLD` share a slot.
pub fn hash(bytes: &[u8]) -> u32 {
	let mut h = 0x811C_9DC5u32;
	for b in bytes {
		h = (h ^ u32::from(b.to_ascii_uppercase())).wrapping_mul(0x0100_0193);
	}
	h
}

/// Twelve bits of a hash: the three hex digits of an 8.3 name.
fn hash12(bytes: &[u8]) -> u32 {
	let h = hash(bytes);
	(h ^ (h >> 12) ^ (h >> 24)) & 0xFFF
}

/// `~` and the first four characters of the stem, as an 8.3 name allows them, then three hex digits.
fn dos_stem(p: &mut Path, file: &[u8], basis: &[u8], probe: u8) {
	let dot = file.iter().rposition(|b| *b == b'.').unwrap_or(file.len());
	p.push(b"~");
	for &c in file.get(..dot).unwrap_or(&[]).iter().take(4) {
		let c = c.to_ascii_uppercase();
		let ok = c.is_ascii_alphanumeric()
			|| matches!(
				c,
				b'_' | b'-'
					| b'$' | b'!' | b'#'
					| b'&' | b'@' | b'^'
					| b'(' | b')' | b'{'
					| b'}' | b'\'' | b'~'
					| b'%'
			);
		p.push(&[if ok { c } else { b'_' }]);
	}
	p.hex(hash12(basis).wrapping_add(u32::from(probe)) & 0xFFF, 3);
}

/// The recovery file of kind `kind` for `orig`, beside it. `probe` moves to the next hash value when another document holds the slot
/// (8.3 names only; the long names are unique).
pub fn beside(style: Style, kind: Kind, orig: &[u8], probe: u8) -> Option<Path> {
	let at = base_at(orig);
	let (dir, file) = (orig.get(..at)?, orig.get(at..)?);
	let mut p = Path::new();
	p.push(dir);
	match style {
		Style::Dos => dos_stem(&mut p, file, file, probe),
		Style::Long => {
			p.push(b"~Quill-").push(file);
		}
		Style::Dot => {
			p.push(b".~Quill-").push(file);
		}
	}
	p.push(ext(style, kind));
	p.done()
}

/// The recovery file of kind `kind` in the central folder `dir`: for an original that could not have it beside it (the hash covers its
/// whole path, since files from many folders meet here), or for a document with no name (`~Q` and six hex digits of the session).
pub fn central(
	style: Style,
	kind: Kind,
	dir: &[u8],
	orig: Option<&[u8]>,
	session: u32,
	probe: u8,
) -> Option<Path> {
	let mut p = Path::new();
	p.push(dir);
	if !dir.is_empty() && !dir.last().is_some_and(|b| is_sep(*b)) {
		p.push(&[sep(style)]);
	}
	let prefix: &[u8] = if style == Style::Dot {
		b".~Quill-"
	} else {
		b"~Quill-"
	};
	match (orig, style) {
		(None, Style::Dos) => {
			p.push(b"~Q").hex(session & 0xFF_FFFF, 6);
		}
		(None, _) => {
			p.push(prefix)
				.push(b"Untitled-")
				.hex(session & 0xFF_FFFF, 6);
		}
		(Some(o), Style::Dos) => dos_stem(&mut p, base_name(o), o, probe),
		(Some(o), _) => {
			p.push(prefix).push(base_name(o)).push(b"-");
			p.hex(hash(o) & 0xFF_FFFF, 6);
		}
	}
	p.push(ext(style, kind));
	p.done()
}

/// How many bytes of `path` come before the suffix that says what kind of file it is.
fn stem_len(path: &[u8], style: Style) -> usize {
	for k in [
		Kind::Aside,
		Kind::Temp,
		Kind::Bad,
		Kind::Current,
		Kind::Emergency,
		Kind::Lock,
	] {
		let e = ext(style, k);
		if let Some(cut) = path.len().checked_sub(e.len()) {
			if path.get(cut..).is_some_and(|t| t.eq_ignore_ascii_case(e)) {
				return cut;
			}
		}
	}
	path.len()
}

/// The path of kind `kind` in the same slot as `any`, which may be a path of any kind: the copies of one document share a stem.
pub fn kind_of(any: &Path, style: Style, kind: Kind) -> Path {
	let mut p = Path::of(
		any.bytes()
			.get(..stem_len(any.bytes(), style))
			.unwrap_or(&[]),
	);
	p.push(ext(style, kind));
	p
}

/// The pattern that finds the current copies in a folder.
fn pattern(style: Style) -> &'static [u8] {
	match style {
		Style::Dos => b"~*.RCV",
		Style::Long => b"~Quill-*.rcv",
		Style::Dot => b".~Quill-*.rcv",
	}
}

fn section(style: Style) -> &'static [u8] {
	match style {
		Style::Dos => b"Recovery",
		Style::Long => b"Recovery32",
		Style::Dot => b"RecoveryLinux",
	}
}

fn dir_key(style: Style) -> &'static [u8] {
	match style {
		Style::Dos => b"RecoveryDir",
		Style::Long => b"RecoveryDir32",
		Style::Dot => b"RecoveryDirLinux",
	}
}

// ---- the time --------------------------------------------------------------------------------------------------------------------

/// A DOS date and time (the date in the high word) in the pieces a person reads.
fn unpack(t: u32) -> (u32, u32, u32, u32, u32) {
	let (d, tm) = (t >> 16, t & 0xFFFF);
	(
		1980 + (d >> 9),
		(d >> 5) & 15,
		d & 31,
		tm >> 11,
		(tm >> 5) & 63,
	)
}

/// `25 Nov 2026 at 2:32 PM`: the date as the editor's date formatter writes it, and the time on a twelve-hour clock.
fn when(t: u32, out: &mut Path) {
	let (y, mo, d, h, mi) = unpack(t);
	let mut buf = [0u8; 16];
	let n = datefmt::format(b"d MMM yyyy", y, mo, d, &mut buf);
	out.push(buf.get(..n).unwrap_or(&[])).push(b" at ");
	let h12 = if h % 12 == 0 { 12 } else { h % 12 };
	let mut digits = [b'0' + (h12 / 10) as u8, b'0' + (h12 % 10) as u8];
	let from = usize::from(h12 < 10);
	out.push(digits.get_mut(from..).unwrap_or(&mut []));
	out.push(&[b':', b'0' + (mi / 10) as u8, b'0' + (mi % 10) as u8]);
	out.push(if h < 12 { b" AM" } else { b" PM" });
}

// ---- the chunk -------------------------------------------------------------------------------------------------------------------

/// One slot of the chunk: a length and a path.
const SLOT: u16 = 2 + NAME_MAX as u16;
/// Slot 0 holds the file a waiting question is about, slot 1 the central folder, slots 2 to 9 the list.
const SLOTS: u16 = 2 + LIST_MAX as u16;
const PENDING_SLOT: u16 = 0;
const DIR_SLOT: u16 = 1;
const LIST_SLOT: u16 = 2;

fn slot_at(i: u16) -> u16 {
	i * SLOT
}

fn slot_get<S: ReadStore + ?Sized>(s: &S, id: ChunkId, i: u16) -> Path {
	let mut len = [0u8; 2];
	s.read(id, slot_at(i), &mut len);
	let n = usize::from(u16::from_le_bytes(len)).min(NAME_MAX);
	let mut p = Path::new();
	if let Some(dst) = p.buf.get_mut(..n) {
		s.read(id, slot_at(i) + 2, dst);
		p.len = n;
	}
	p
}

fn slot_put<S: Store + ?Sized>(s: &mut S, id: ChunkId, i: u16, b: &[u8]) {
	let n = b.len().min(NAME_MAX);
	s.write(id, slot_at(i), &(n as u16).to_le_bytes());
	s.write(id, slot_at(i) + 2, b.get(..n).unwrap_or(&[]));
}

// ---- the editor's side -----------------------------------------------------------------------------------------------------------

/// What a recovery file says about the original it came from, now.
#[derive(Copy, Clone, PartialEq, Eq)]
enum Orig {
	/// As it was when the copy was written.
	Same,
	/// Saved since then.
	Changed,
	/// Gone.
	Deleted,
}

/// What a candidate copy is.
#[derive(Copy, Clone)]
struct Cand {
	path: Path,
	/// The old `QUILL.RCV`, which has no `RCOV` chunk.
	legacy: bool,
}

impl<M: Measure + Clone> App<M> {
	/// Turns the recovery protocol on or off. A shell turns it on when it can ask the questions: the lock note, the list in `QUILL.INI`,
	/// and the questions at Open and at launch all follow.
	pub fn set_recovery(&mut self, on: bool) {
		if on {
			self.rcv.flags |= ENABLED;
		} else {
			self.rcv.flags &= !ENABLED;
		}
	}

	/// True when the recovery protocol is on.
	pub fn recovery_on(&self) -> bool {
		self.rcv.flags & ENABLED != 0
	}

	/// What the build says about itself: the default autosave interval in minutes, the writer number the `RCOV` chunk records (0 DOS
	/// text, 1 DOS graphics, 2 Win16, 3 Win32, 4 terminal, 5 GTK, 6 WinUI, 7 web) and whether the copies are dot-files.
	pub fn configure_recovery(&mut self, minutes: u8, writer: u8, dot_files: bool) {
		self.rcv.interval = minutes.min(60);
		self.rcv.interval_default = minutes.min(60);
		self.rcv.writer = writer;
		if dot_files {
			self.rcv.flags |= DOT;
		} else {
			self.rcv.flags &= !DOT;
		}
	}

	/// The minutes between autosaves, 0 for never.
	pub fn autosave_minutes(&self) -> u8 {
		self.rcv.interval
	}

	/// Sets the minutes between autosaves (0 to 60, 0 for never).
	pub fn set_autosave_minutes(&mut self, minutes: u8) {
		self.rcv.interval = minutes.min(60);
	}

	/// The test hook `[Debug] AutosaveSeconds`: an interval in seconds, 0 when absent.
	pub fn autosave_debug_seconds(&self) -> u16 {
		self.rcv.debug_seconds
	}

	/// Seconds between autosaves: the debug hook's, else the minutes of the setting; 0 for never.
	pub(crate) fn autosave_interval(&self) -> u32 {
		if self.rcv.debug_seconds > 0 {
			u32::from(self.rcv.debug_seconds)
		} else {
			u32::from(self.rcv.interval) * 60
		}
	}

	/// Seconds of quiet before an autosave: 2 on the 8.3 builds, 1 on the clients that have long names and the debug hook.
	pub(crate) fn autosave_wait(&self) -> u32 {
		if self.rcv.debug_seconds > 0 || self.rcv.writer >= 4 {
			1
		} else {
			2
		}
	}

	/// True when another window has this document, so nothing is written for it here.
	pub(crate) fn second_window(&self) -> bool {
		self.rcv.flags & SECOND != 0
	}

	pub(crate) fn rcv_each_setting(&self, f: &mut crate::Emit<'_>) {
		if self.rcv.interval != self.rcv.interval_default {
			let v = self.rcv.interval;
			let digits = [b'0' + v / 10, b'0' + v % 10];
			f(
				b"Quill",
				b"Autosave",
				digits.get(usize::from(v < 10)..).unwrap_or(&[]),
			);
		}
	}

	/// Takes in `Autosave` and `AutosaveSeconds`; true when the key was one of them.
	pub(crate) fn rcv_apply_setting(&mut self, key: &[u8], value: &[u8]) -> bool {
		let num = |v: &[u8]| {
			v.iter()
				.take_while(|b| b.is_ascii_digit())
				.fold(0u16, |n, b| {
					n.saturating_mul(10).saturating_add(u16::from(b - b'0'))
				})
		};
		if key.eq_ignore_ascii_case(b"Autosave") {
			self.rcv.interval = num(value).min(60) as u8;
		} else if key.eq_ignore_ascii_case(b"AutosaveSeconds") {
			self.rcv.debug_seconds = num(value);
		} else {
			return false;
		}
		true
	}

	fn style(&self) -> Style {
		// A build without the `dot-files` feature never names a copy as a dot-file, so the code for those names is not linked.
		if cfg!(feature = "dot-files") && self.rcv.flags & DOT != 0 {
			Style::Dot
		} else {
			Style::native()
		}
	}

	fn on(&self) -> bool {
		self.rcv.flags & ENABLED != 0
	}

	fn chunk<S: Store + ?Sized>(&mut self, s: &mut S) -> Option<ChunkId> {
		if let Some(id) = self.rcv.list {
			return Some(id);
		}
		let id = s.alloc(slot_at(SLOTS))?;
		for i in 0..SLOTS {
			s.write(id, slot_at(i), &[0, 0]);
		}
		self.rcv.list = Some(id);
		Some(id)
	}

	/// The session's number, which names an untitled document's copy.
	fn session<F: Files>(&mut self, f: &mut F) -> u32 {
		if self.rcv.session == 0 {
			let t = f.now().wrapping_mul(0x9E37_79B1) ^ f.instance().wrapping_mul(0x85EB_CA6B);
			self.rcv.session = (t & 0xFF_FFFF) | 1;
		}
		self.rcv.session
	}

	// ---- the list and the central folder ----------------------------------------------------------------------------------------

	/// How many originals the list holds.
	pub fn recovery_list_len<S: ReadStore + ?Sized>(&self, s: &S) -> usize {
		let Some(id) = self.rcv.list else { return 0 };
		(0..LIST_MAX as u16)
			.take_while(|i| slot_get(s, id, LIST_SLOT + i).len > 0)
			.count()
	}

	/// The original at `i` in the list, 0 being the newest.
	pub fn recovery_list_get<S: ReadStore + ?Sized>(&self, s: &S, i: usize) -> Option<Path> {
		let id = self.rcv.list?;
		let p = slot_get(s, id, LIST_SLOT + i as u16);
		(i < LIST_MAX && p.len > 0).then_some(p)
	}

	/// Puts `orig` first in the list; true when the list changed.
	pub fn recovery_list_add<S: Store + ?Sized>(&mut self, s: &mut S, orig: &[u8]) -> bool {
		let Some(id) = self.chunk(s) else {
			return false;
		};
		let n = self.recovery_list_len(s);
		let at = (0..n)
			.find(|i| {
				self.recovery_list_get(s, *i)
					.is_some_and(|p| same(p.bytes(), orig))
			})
			.unwrap_or(n.min(LIST_MAX - 1));
		if at == 0
			&& n > 0 && self
			.recovery_list_get(s, 0)
			.is_some_and(|p| same(p.bytes(), orig))
		{
			return false;
		}
		for i in (1..=at as u16).rev() {
			s.copy(
				(id, slot_at(LIST_SLOT + i - 1)),
				(id, slot_at(LIST_SLOT + i)),
				SLOT,
			);
		}
		slot_put(s, id, LIST_SLOT, orig);
		true
	}

	/// Takes `orig` out of the list; true when it was there.
	pub fn recovery_list_remove<S: Store + ?Sized>(&mut self, s: &mut S, orig: &[u8]) -> bool {
		let Some(id) = self.rcv.list else {
			return false;
		};
		let n = self.recovery_list_len(s);
		let Some(at) = (0..n).find(|i| {
			self.recovery_list_get(s, *i)
				.is_some_and(|p| same(p.bytes(), orig))
		}) else {
			return false;
		};
		for i in at as u16..n as u16 - 1 {
			s.copy(
				(id, slot_at(LIST_SLOT + i + 1)),
				(id, slot_at(LIST_SLOT + i)),
				SLOT,
			);
		}
		s.write(id, slot_at(LIST_SLOT + n as u16 - 1), &[0, 0]);
		true
	}

	/// Calls `f(section, key, value)` for the central folder and the list, as `QUILL.INI` keeps them.
	pub fn each_recovery_setting<S: ReadStore + ?Sized>(&self, s: &S, f: &mut crate::Emit<'_>) {
		let Some(id) = self.rcv.list else { return };
		let dir = slot_get(s, id, DIR_SLOT);
		if dir.len > 0 {
			f(b"Quill", dir_key(self.style()), dir.bytes());
		}
		for i in 0..self.recovery_list_len(s) {
			if let Some(p) = self.recovery_list_get(s, i) {
				f(
					section(self.style()),
					&[b'F', b'i', b'l', b'e', b'1' + i as u8],
					p.bytes(),
				);
			}
		}
	}

	/// Takes in one setting that belongs to the central folder or the list; keys it does not know are ignored. The list's entries
	/// arrive newest first and are kept in that order.
	pub fn apply_recovery_setting<S: Store + ?Sized>(
		&mut self,
		s: &mut S,
		sec: &[u8],
		key: &[u8],
		value: &[u8],
	) {
		let style = self.style();
		if sec.eq_ignore_ascii_case(b"Quill") && key.eq_ignore_ascii_case(dir_key(style)) {
			if let Some(id) = self.chunk(s) {
				slot_put(s, id, DIR_SLOT, value);
			}
		} else if sec.eq_ignore_ascii_case(section(style)) {
			if let [b'F' | b'f', b'i' | b'I', b'l' | b'L', b'e' | b'E', d @ b'1'..=b'8'] = key {
				let i = u16::from(d - b'1');
				if let Some(id) = self.chunk(s) {
					if i as usize == self.recovery_list_len(s) && !value.is_empty() {
						slot_put(s, id, LIST_SLOT + i, value);
					}
				}
			}
		}
	}

	/// Reads the central folder and the list from `QUILL.INI`, line by line in a small buffer.
	pub fn load_recovery<S: Store + ?Sized, F: Files>(&mut self, s: &mut S, f: &mut F) {
		let Some(mut r) = f.open(b"QUILL.INI") else {
			return;
		};
		let mut sec = [0u8; 16];
		let mut sec_len = 0;
		let mut line = [0u8; NAME_MAX + 24];
		let mut n = 0;
		let mut over = false;
		let mut block = [0u8; 64];
		loop {
			let got = r.read(&mut block);
			for &b in block.get(..got).unwrap_or(&[]).iter().chain(if got == 0 {
				&b"\n"[..]
			} else {
				&[]
			}) {
				if b != b'\n' {
					match line.get_mut(n) {
						Some(slot) => {
							*slot = b;
							n += 1;
						}
						None => over = true,
					}
					continue;
				}
				let text = line.get(..n).unwrap_or(&[]);
				let text = text.strip_suffix(b"\r").unwrap_or(text);
				if over {
				} else if let Some(name) =
					text.strip_prefix(b"[").and_then(|l| l.strip_suffix(b"]"))
				{
					sec_len = name.len().min(sec.len());
					if let (Some(d), Some(sv)) = (sec.get_mut(..sec_len), name.get(..sec_len)) {
						d.copy_from_slice(sv);
					}
				} else if let Some(eq) = text.iter().position(|b| *b == b'=') {
					if let (Some(k), Some(v)) = (text.get(..eq), text.get(eq + 1..)) {
						let name = sec.get(..sec_len).unwrap_or(&[]);
						self.apply_recovery_setting(s, name, k, v);
					}
				}
				n = 0;
				over = false;
			}
			if got == 0 {
				break;
			}
		}
	}

	/// `load_settings`, then the central folder and the list.
	pub fn load_settings_with<S: Store + ?Sized, F: Files>(&mut self, s: &mut S, f: &mut F) {
		self.load_settings(f);
		self.load_recovery(s, f);
	}

	/// `save_settings`, then the central folder and the list: a shell that keeps the list writes `QUILL.INI` with this one, since
	/// `save_settings` alone would leave the list out.
	pub fn save_settings_with<S: ReadStore + ?Sized, F: Files>(&self, s: &S, f: &mut F) {
		self.write_ini(f, &mut |out| self.each_recovery_setting(s, out));
	}

	fn persist<S: Store + ?Sized, F: Files>(&mut self, s: &mut S, f: &mut F) {
		self.save_settings_with(&*s, f);
	}

	/// The `n`th central folder the writer tries: the person's `RecoveryDir`, then the shell's own choices.
	fn central_dir<S: ReadStore + ?Sized, F: Files>(
		&self,
		s: &S,
		f: &mut F,
		n: u8,
	) -> Option<Path> {
		let mut i = n;
		if let Some(id) = self.rcv.list {
			let dir = slot_get(s, id, DIR_SLOT);
			if dir.len > 0 {
				if i == 0 {
					return Some(dir);
				}
				i -= 1;
			}
		}
		let mut p = Path::new();
		let len = f.recovery_dir(i, p.buf.get_mut(..NAME_MAX).unwrap_or(&mut []));
		(len > 0).then(|| {
			p.len = len.min(NAME_MAX);
			p
		})
	}

	// ---- finding copies ---------------------------------------------------------------------------------------------------------

	/// Whether the copy at `path` is the copy of `orig`: its `RCOV` path is `orig`, or is a place `orig` has moved from with the same
	/// name and the stamp the copy recorded. A file that is not a readable recovery file is taken as ours to replace.
	fn mine<F: Files>(&self, f: &mut F, path: &[u8], orig: &[u8]) -> Option<bool> {
		let mut r = f.open(path)?;
		let Ok(rec) = qld::read_header(&mut r) else {
			return Some(true);
		};
		// A copy named for the session is this session's, whatever original it came from.
		if orig.is_empty() || same(rec.path(), orig) {
			return Some(true);
		}
		let moved = same(base_name(rec.path()), base_name(orig))
			&& f.stamp(orig).is_some_and(|st| {
				st.size == rec.orig_size && st.time == rec.orig_stamp && rec.orig_size != u32::MAX
			});
		Some(moved)
	}

	/// Who holds a slot: `None` free, `Some(true)` this document, `Some(false)` another.
	fn slot<F: Files>(&self, f: &mut F, current: &Path, orig: &[u8]) -> Option<bool> {
		let style = self.style();
		let mut state = None;
		// A temporary file counts: a power cut between the delete and the rename leaves the whole copy there.
		for k in [Kind::Current, Kind::Aside, Kind::Temp] {
			let p = kind_of(current, style, k);
			match (self.mine(f, p.bytes(), orig), state) {
				(Some(true), _) => return Some(true),
				(Some(false), _) => state = Some(false),
				(None, _) => {}
			}
		}
		state
	}

	/// The current-copy path of the slot that holds `orig`'s copies (the current copy, the set-aside copy or both), if any.
	fn locate<S: ReadStore + ?Sized, F: Files>(
		&self,
		s: &S,
		f: &mut F,
		orig: &[u8],
	) -> Option<Path> {
		let style = self.style();
		let session = self.rcv.session;
		if !orig.is_empty() {
			for probe in 0..PROBES {
				let p = beside(style, Kind::Current, orig, probe)?;
				match self.slot(f, &p, orig) {
					Some(true) => return Some(p),
					Some(false) => {}
					None => break,
				}
			}
		}
		let mut n = 0;
		while let Some(dir) = self.central_dir(s, f, n) {
			n += 1;
			for probe in 0..PROBES {
				let p = central(
					style,
					Kind::Current,
					dir.bytes(),
					(!orig.is_empty()).then_some(orig),
					session,
					probe,
				)?;
				match self.slot(f, &p, orig) {
					Some(true) => return Some(p),
					Some(false) => {}
					None => break,
				}
			}
		}
		None
	}

	/// Puts the shell's string `id` on the status line.
	fn say<F: Files>(&mut self, f: &mut F, id: u8) {
		let mut buf = [0u8; 96];
		let n = f.string(id, &mut buf);
		self.msg.clear();
		self.msg.push(buf.get(..n).unwrap_or(&[]));
	}

	/// Takes a damaged copy out of the way and says so.
	fn set_bad<F: Files>(&mut self, f: &mut F, path: &Path) {
		let bad = kind_of(path, self.style(), Kind::Bad);
		f.remove(bad.bytes());
		if !f.rename(path.bytes(), bad.bytes()) {
			f.remove(path.bytes());
		}
		self.say(f, STR_DAMAGED);
	}

	/// Settles a slot left by a write that a power cut stopped: a temporary file that checks out and is newer than the current copy
	/// becomes the current copy; any other temporary file goes.
	fn settle<F: Files>(&mut self, f: &mut F, current: &Path) {
		let temp = kind_of(current, self.style(), Kind::Temp);
		let Some(mut r) = f.open(temp.bytes()) else {
			return;
		};
		let fresh = qld::verify(&mut r).ok();
		let old = self.header_of(f, current);
		let promote = match (&fresh, &old) {
			(Some(n), Some(o)) => n.written >= o.written,
			(Some(_), None) => true,
			_ => false,
		};
		if promote {
			f.remove(current.bytes());
			f.rename(temp.bytes(), current.bytes());
		} else {
			f.remove(temp.bytes());
		}
	}

	/// The copy to offer for the slot of `current`: the current copy if it checks out, else the set-aside one. A copy that fails its
	/// check is put aside as damaged and the next is tried.
	fn vet<F: Files>(&mut self, f: &mut F, current: &Path) -> Option<Path> {
		self.settle(f, current);
		let aside = kind_of(current, self.style(), Kind::Aside);
		let mut have = [false; 2];
		for (i, p) in [*current, aside].iter().enumerate() {
			let Some(mut r) = f.open(p.bytes()) else {
				continue;
			};
			if let (true, Some(h)) = (qld::verify(&mut r).is_ok(), have.get_mut(i)) {
				*h = true;
			} else {
				self.set_bad(f, p);
			}
		}
		match have {
			[true, _] => Some(*current),
			[false, true] => Some(aside),
			_ => None,
		}
	}

	/// An emergency copy of the slot of `base` that is newer than `copy`, if there is one.
	fn emergency_for<F: Files>(&mut self, f: &mut F, copy: &Path, base: &Path) -> Option<Path> {
		let e = kind_of(base, self.style(), Kind::Emergency);
		let mut r = f.open(e.bytes())?;
		let eh = qld::verify(&mut r).ok()?;
		let ch = self.header_of(f, copy);
		ch.is_none_or(|c| eh.written > c.written).then_some(e)
	}

	fn header_of<F: Files>(&self, f: &mut F, p: &Path) -> Option<Recovery> {
		let mut r = f.open(p.bytes())?;
		qld::read_header(&mut r).ok()
	}

	// ---- writing ----------------------------------------------------------------------------------------------------------------

	/// Writes the document as a recovery copy: beside the original when that place is a fixed disk that takes the file, else in the
	/// central folder. It does not rename the document, clear its modified flag or touch the recent list. `Why::Emergency` writes the
	/// `.RCE` copy and changes nothing else. False when nothing was written: the document is clean, another window has it, or no
	/// place took the file.
	pub fn save_recovery<S: Store + ?Sized, F: Files>(
		&mut self,
		s: &mut S,
		f: &mut F,
		why: Why,
	) -> bool {
		self.save_recovery_with(s, f, why, NoInput) == Wrote::Done
	}

	/// `save_recovery` that a key can cut short: while the copy is written, `input` is asked about every `PROGRESS_STEP` bytes, and
	/// when it says true the write stops, its temporary file goes and the copy that was there stays (`Wrote::Abandoned`).
	pub fn save_recovery_with<I: Input, S: Store + ?Sized, F: Files>(
		&mut self,
		s: &mut S,
		f: &mut F,
		why: Why,
		mut input: I,
	) -> Wrote {
		if !self.modified || (why != Why::Emergency && self.rcv.flags & SECOND != 0) {
			return Wrote::Failed;
		}
		let style = self.style();
		let mut orig = [0u8; NAME_MAX];
		let olen = self.name_len;
		if let (Some(d), Some(sv)) = (orig.get_mut(..olen), self.name.get(..olen)) {
			d.copy_from_slice(sv);
		}
		let orig = orig.get(..olen).unwrap_or(&[]);
		let kind = if why == Why::Emergency {
			Kind::Emergency
		} else {
			Kind::Current
		};
		if !orig.is_empty() && f.drive(orig) == Drive::Fixed {
			for probe in 0..PROBES {
				let (Some(cur), Some(p)) = (
					beside(style, Kind::Current, orig, probe),
					beside(style, kind, orig, probe),
				) else {
					break;
				};
				if self.slot(f, &cur, orig) == Some(false) {
					continue;
				}
				match self.write_copy(s, f, (&p, false), why, orig, &mut input) {
					Wrote::Failed => {}
					done => return done,
				}
				break;
			}
		}
		let session = self.session(f);
		let mut n = 0;
		while let Some(dir) = self.central_dir(&*s, f, n) {
			n += 1;
			f.make_dir(dir.bytes());
			let o = (!orig.is_empty()).then_some(orig);
			for probe in 0..PROBES {
				let (Some(cur), Some(p)) = (
					central(style, Kind::Current, dir.bytes(), o, session, probe),
					central(style, kind, dir.bytes(), o, session, probe),
				) else {
					break;
				};
				if self.slot(f, &cur, orig) == Some(false) {
					continue;
				}
				match self.write_copy(s, f, (&p, true), why, orig, &mut input) {
					Wrote::Failed => {}
					done => return done,
				}
				break;
			}
		}
		if why == Why::Autosave && self.rcv.flags & SAID_FULL == 0 {
			self.rcv.flags |= SAID_FULL;
			self.say(f, STR_CANNOT_AUTOSAVE);
		}
		Wrote::Failed
	}

	fn write_copy<I: Input, S: Store + ?Sized, F: Files>(
		&mut self,
		s: &mut S,
		f: &mut F,
		(target, in_central): (&Path, bool),
		why: Why,
		orig: &[u8],
		input: &mut I,
	) -> Wrote {
		let style = self.style();
		let emergency = why == Why::Emergency;
		if !emergency && self.rcv.flags & LEFT != 0 && f.exists(target.bytes()) {
			// The copy the person left alone at Open becomes the set-aside copy, replacing an older one, before the new copy exists.
			let aside = kind_of(target, style, Kind::Aside);
			f.remove(aside.bytes());
			if f.rename(target.bytes(), aside.bytes()) {
				self.rcv.flags = (self.rcv.flags & !LEFT) | ASIDE;
			}
		}
		let mut rec = Recovery::default();
		rec.flags = match why {
			Why::Autosave => RCV_AUTOSAVE,
			Why::EndOfSession => RCV_END_OF_SESSION,
			Why::Emergency => RCV_EMERGENCY,
		} | if orig.is_empty() { RCV_UNTITLED } else { 0 }
			| if in_central { RCV_CENTRAL } else { 0 };
		rec.format = self.fmt;
		rec.writer = self.rcv.writer;
		rec.quill = (0, 1);
		rec.written = f.now();
		rec.revision = self.doc.revision();
		rec.orig_size = self.rcv.orig_size;
		rec.orig_stamp = self.rcv.orig_time;
		rec.caret = self.caret;
		rec.top_line = self.top;
		if rec.set_path(orig).is_err() {
			return Wrote::Failed;
		}
		let temp = kind_of(target, style, Kind::Temp);
		let doc = &self.doc;
		let store = &*s;
		let mut cut = false;
		// A caller that does not poll for input (every one but the idle autosave) links no hook.
		let saved = if I::POLLS {
			let mut hook = |_done: u32, _total: u32| {
				cut = input.waiting();
				!cut
			};
			safe::write_replace_via(f, target.bytes(), temp.bytes(), |w| {
				qld::write_recovery(doc, store, w, &rec, Some(&mut hook))
			})
		} else {
			safe::write_replace_via(f, target.bytes(), temp.bytes(), |w| {
				qld::write_recovery(doc, store, w, &rec, None)
			})
		};
		if saved == safe::Saved::Failed {
			return if cut { Wrote::Abandoned } else { Wrote::Failed };
		}
		if !emergency {
			self.rcv.flags |= WROTE;
			if !in_central && self.on() && !orig.is_empty() && self.recovery_list_add(s, orig) {
				self.persist(s, f);
			}
		}
		Wrote::Done
	}

	// ---- the document changes ---------------------------------------------------------------------------------------------------

	/// The first modification of a named document takes the lock note; a second window that finds one held by a live owner stops
	/// writing recovery copies. Called after each key and command.
	pub(crate) fn rcv_after<F: Files>(&mut self, f: &mut F) {
		self.idle_dirty(f);
		if self.rcv.flags & (ENABLED | LOCKED | SECOND) != ENABLED
			|| !self.modified
			|| self.name_len == 0
		{
			return;
		}
		let Some(lock) = self.lock_path() else { return };
		let mut stamp = [0u8; 32];
		stamp[0] = self.rcv.writer;
		let now = f.now().to_le_bytes();
		if let Some(d) = stamp.get_mut(4..8) {
			d.copy_from_slice(&now);
		}
		let who = f.instance().to_le_bytes();
		if let Some(d) = stamp.get_mut(8..12) {
			d.copy_from_slice(&who);
		}
		match f.lock_make(lock.bytes(), &stamp) {
			LockMade::Made => self.rcv.flags |= LOCKED,
			LockMade::Refused => {}
			LockMade::Exists => {
				if f.lock_owner(lock.bytes()) == Owner::Alive {
					self.rcv.flags |= SECOND;
					self.say(f, STR_SECOND_WINDOW);
				} else {
					// A note nothing proves alive is what a crash leaves: take it over.
					f.lock_drop(lock.bytes());
					if f.lock_make(lock.bytes(), &stamp) == LockMade::Made {
						self.rcv.flags |= LOCKED;
					}
				}
			}
		}
	}

	fn lock_path(&self) -> Option<Path> {
		let orig = self.name.get(..self.name_len)?;
		beside(self.style(), Kind::Lock, orig, 0)
	}

	fn unlock<F: Files>(&mut self, f: &mut F) {
		if self.rcv.flags & LOCKED != 0 {
			if let Some(p) = self.lock_path() {
				f.lock_drop(p.bytes());
			}
		}
		self.rcv.flags &= !LOCKED;
	}

	/// The path of this document's copies, if it has any: beside the original or in the central folder.
	fn own_copy<S: ReadStore + ?Sized, F: Files>(&self, s: &S, f: &mut F) -> Option<Path> {
		let orig = self.name.get(..self.name_len)?;
		self.locate(s, f, orig)
	}

	/// The document is going away or being replaced: Don't Save (the document is modified) removes this session's copy and brings a
	/// set-aside copy back; a clean close leaves what is there. The lock note goes either way.
	pub(crate) fn rcv_leave<S: Store + ?Sized, F: Files>(&mut self, s: &mut S, f: &mut F) {
		self.idle_reset();
		if self.rcv.flags & (ENABLED | WROTE | LEFT | ASIDE | LOCKED) == 0 {
			return;
		}
		if self.modified && self.rcv.flags & WROTE != 0 {
			if let Some(cur) = self.own_copy(&*s, f) {
				let aside = kind_of(&cur, self.style(), Kind::Aside);
				f.remove(cur.bytes());
				if self.rcv.flags & ASIDE != 0 {
					f.rename(aside.bytes(), cur.bytes());
				}
				let temp = kind_of(&cur, self.style(), Kind::Temp);
				f.remove(temp.bytes());
			}
		}
		self.unlock(f);
		self.rcv.flags &= KEEP;
		self.rcv.orig_size = u32::MAX;
		self.rcv.orig_time = 0;
	}

	/// The document was saved under its name (and was `old` before, when that differs): both copies go, the lock note goes, the list
	/// loses the entry and the original's stamp is read again.
	pub(crate) fn rcv_saved<S: Store + ?Sized, F: Files>(
		&mut self,
		s: &mut S,
		f: &mut F,
		old: &[u8],
	) {
		self.idle_reset();
		if self.rcv.flags & (WROTE | LEFT | ASIDE | LEGACY) != 0 {
			let name = self.name;
			let name = name.get(..self.name_len).unwrap_or(&[]);
			for orig in [old, name] {
				self.remove_copies(s, f, orig);
			}
			if self.rcv.flags & LEGACY != 0 {
				let mut n = 0;
				while let Some(p) = legacy_path(f, n, self.style()) {
					f.remove(p.bytes());
					n += 1;
				}
			}
		}
		self.unlock(f);
		self.rcv.flags &= KEEP;
		let name = self.name;
		self.rcv_note_original(f, name.get(..self.name_len).unwrap_or(&[]));
	}

	/// Removes every copy of `orig` (an empty name is the untitled document), and its entry in the list.
	fn remove_copies<S: Store + ?Sized, F: Files>(&mut self, s: &mut S, f: &mut F, orig: &[u8]) {
		let style = self.style();
		for _ in 0..PROBES {
			let Some(cur) = self.locate(&*s, f, orig) else {
				break;
			};
			for k in [Kind::Current, Kind::Aside, Kind::Temp, Kind::Emergency] {
				f.remove(kind_of(&cur, style, k).bytes());
			}
		}
		if !orig.is_empty() && self.recovery_list_remove(s, orig) && self.on() {
			self.persist(s, f);
		}
	}

	/// The document in memory is given up without saving: the person said Don't Save at a shutdown, or the shell closes it. Same as
	/// closing it with Don't Save.
	pub fn recovery_closed<S: Store + ?Sized, F: Files>(&mut self, s: &mut S, f: &mut F) {
		self.rcv_leave(s, f);
	}

	/// Records the original's size and time as the file system has them now.
	pub(crate) fn rcv_note_original<F: Files>(&mut self, f: &mut F, name: &[u8]) {
		let st = f.stamp(name);
		self.rcv.orig_size = st.map_or(u32::MAX, |st| st.size);
		self.rcv.orig_time = st.map_or(0, |st| st.time);
	}

	// ---- the questions ----------------------------------------------------------------------------------------------------------

	/// True while a recovery question has been raised and not answered.
	pub fn recovery_pending(&self) -> bool {
		self.rcv.flags & PENDING != 0
	}

	/// Looks for the next recovery copy to offer at launch: the list's originals newest first, then the central folder, then the old
	/// `QUILL.RCV`. True when a question is waiting; fetch it with `take_recovery_question`. A shell calls this once the window is up,
	/// before it opens a file named on the command line, and `recovery_answered` goes on to the next copy by itself. Once a copy has
	/// been recovered no more are offered: the rest wait for the next start.
	pub fn recovery_check_launch<S: Store + ?Sized, F: Files>(
		&mut self,
		s: &mut S,
		f: &mut F,
	) -> bool {
		if !self.on() || self.rcv.flags & RECOVERED != 0 {
			return false;
		}
		if self.recovery_pending() {
			return true;
		}
		let Some(id) = self.chunk(s) else {
			return false;
		};
		while let Some(c) = self.candidate(s, f, self.rcv.skip) {
			// A candidate is a current-copy path or the old file; a copy that fails its check is set aside and the search goes on.
			let copy = if c.legacy {
				Some(c.path)
			} else {
				self.vet(f, &c.path)
			};
			if let Some(copy) = copy {
				slot_put(s, id, PENDING_SLOT, copy.bytes());
				self.rcv.flags |= Q_LAUNCH;
				return true;
			}
		}
		false
	}

	/// The `n`th place that may hold a recovery copy, not counting the ones already passed with Later: list entries whose copies exist,
	/// the central folders' current copies, then the old `QUILL.RCV`.
	fn candidate<S: Store + ?Sized, F: Files>(
		&mut self,
		s: &mut S,
		f: &mut F,
		n: u8,
	) -> Option<Cand> {
		let style = self.style();
		let mut left = n;
		let mut i = 0;
		while let Some(orig) = self.recovery_list_get(&*s, i) {
			if f.drive(orig.bytes()) == Drive::Missing {
				// A drive that is not there is skipped this time and kept.
				i += 1;
			} else if let Some(path) = self.locate(&*s, f, orig.bytes()) {
				if left == 0 {
					return Some(Cand {
						path,
						legacy: false,
					});
				}
				left -= 1;
				i += 1;
			} else if !self.recovery_list_remove(s, orig.bytes()) {
				// The entry's copies are gone: it leaves the list silently.
				i += 1;
			} else {
				self.persist(s, f);
			}
		}
		let mut d = 0;
		while let Some(dir) = self.central_dir(&*s, f, d) {
			d += 1;
			let dir = with_sep(dir.bytes(), style);
			let mut pat = dir;
			pat.push(pattern(style));
			let mut name = [0u8; NAME_MAX];
			for k in 0.. {
				let len = f.find_nth(pat.bytes(), k, &mut name);
				if len == 0 {
					break;
				}
				if left > 0 {
					left -= 1;
					continue;
				}
				let mut path = dir;
				path.push(name.get(..len.min(NAME_MAX)).unwrap_or(&[]));
				return Some(Cand {
					path,
					legacy: false,
				});
			}
		}
		for d in 0.. {
			let p = legacy_path(f, d, style)?;
			if f.exists(p.bytes()) {
				if left == 0 {
					return Some(Cand {
						path: p,
						legacy: true,
					});
				}
				left -= 1;
			}
		}
		None
	}

	/// What a document being opened needs first: a second window's question or a recovery question. True when one was raised and the
	/// open waits for the answer.
	pub(crate) fn rcv_gate<S: Store + ?Sized, F: Files>(
		&mut self,
		s: &mut S,
		f: &mut F,
		name: &[u8],
	) -> bool {
		if !self.on() || self.recovery_pending() || !f.exists(name) {
			return false;
		}
		let Some(id) = self.chunk(s) else {
			return false;
		};
		if let Some(lock) = beside(self.style(), Kind::Lock, name, 0) {
			if f.exists(lock.bytes()) && f.lock_owner(lock.bytes()) == Owner::Alive {
				slot_put(s, id, PENDING_SLOT, name);
				self.rcv.flags |= Q_SECOND;
				return true;
			}
		}
		let Some(cur) = self.locate(&*s, f, name) else {
			return false;
		};
		if self.vet(f, &cur).is_none() {
			return false;
		}
		slot_put(s, id, PENDING_SLOT, name);
		self.rcv.flags |= Q_OPEN;
		true
	}

	/// The question that is waiting, to show once.
	pub fn take_recovery_question<S: Store + ?Sized, F: Files>(
		&mut self,
		s: &mut S,
		f: &mut F,
	) -> Option<Ask> {
		if !self.recovery_pending() || self.rcv.flags & ASKED != 0 {
			return None;
		}
		let id = self.rcv.list?;
		let slot = slot_get(&*s, id, PENDING_SLOT);
		let style = self.style();
		let launch = self.rcv.flags & Q_LAUNCH != 0;
		let ask = if self.rcv.flags & Q_SECOND != 0 {
			Ask {
				text: Text::from_parts(&[base_name(slot.bytes()), words::SECOND_WINDOW.as_bytes()]),
				buttons: Buttons::YesNo,
				default: Answer::No,
			}
		} else {
			let legacy = launch && slot.bytes().ends_with(b"QUILL.RCV");
			// The slot's current-copy path, and the copy on offer: the current one, else the set-aside one.
			let base = if launch {
				kind_of(&slot, style, Kind::Current)
			} else {
				self.locate(&*s, f, slot.bytes())?
			};
			let aside = kind_of(&base, style, Kind::Aside);
			let mut copy = if legacy {
				slot
			} else if f.exists(base.bytes()) && !(launch && slot.bytes() == aside.bytes()) {
				base
			} else {
				aside
			};
			let only_aside = !legacy && copy.bytes() == aside.bytes();
			let both = !legacy && !only_aside && f.exists(aside.bytes());
			let mut rec = (!legacy).then(|| self.header_of(f, &copy)).flatten();
			if !legacy && !only_aside {
				if let Some(e) = self.emergency_for(f, &copy, &base) {
					copy = e;
					rec = self.header_of(f, &e);
				}
			}
			let emergency = rec.as_ref().is_some_and(|r| r.flags & RCV_EMERGENCY != 0);
			let mut date = Path::new();
			when(
				rec.as_ref().map_or_else(
					|| f.stamp(copy.bytes()).map_or(0, |st| st.time),
					|r| r.written,
				),
				&mut date,
			);
			let orig = match (&rec, launch) {
				(Some(r), true) => Path::of(r.path()),
				(Some(_), false) => slot,
				(None, _) => Path::new(),
			};
			let name = base_name(orig.bytes());
			let (from, date) = (words::FROM.as_bytes(), date.bytes());
			let mut t = Text::<ASK_MAX>::new();
			if only_aside {
				t.push(words::OLDER_COPY.as_bytes()).push(name);
				t.push(words::EXISTS_FROM.as_bytes()).push(date).push(b".");
			} else if emergency {
				t.push(words::INTERNAL_ERROR.as_bytes());
				t.push(if name.is_empty() {
					b"an untitled document"
				} else {
					name
				});
				t.push(from)
					.push(date)
					.push(words::MAY_BE_MISSING.as_bytes());
			} else if name.is_empty() {
				t.push(words::FOUND_UNTITLED.as_bytes())
					.push(from)
					.push(date)
					.push(b".");
			} else if launch {
				t.push(words::FOUND_CHANGES.as_bytes()).push(name);
				t.push(words::NOT_SAVED.as_bytes())
					.push(from)
					.push(date)
					.push(b".");
			} else {
				t.push(name).push(words::HAS_CHANGES.as_bytes()).push(from);
				t.push(date).push(b".");
			}
			if let (Some(r), false) = (&rec, name.is_empty()) {
				match self.orig_state(f, orig.bytes(), r) {
					Orig::Same => {}
					Orig::Changed => {
						t.push(b" ").push(name).push(words::SAVED_SINCE.as_bytes());
					}
					Orig::Deleted => {
						let dir = orig.bytes().get(..base_at(orig.bytes())).unwrap_or(&[]);
						t.push(b" ").push(name).push(words::NO_LONGER_IN.as_bytes());
						t.push(dir).push(b".");
					}
				}
			}
			if both {
				t.push(words::OLDER_ALSO_KEPT.as_bytes());
			}
			Ask {
				text: t,
				buttons: if launch {
					Buttons::RecoverDiscardLater
				} else {
					Buttons::RecoverOriginalDiscard
				},
				default: Answer::Yes,
			}
		};
		self.rcv.flags |= ASKED;
		Some(ask)
	}

	fn orig_state<F: Files>(&self, f: &mut F, orig: &[u8], rec: &Recovery) -> Orig {
		match f.stamp(orig) {
			None if f.exists(orig) => Orig::Same,
			None => Orig::Deleted,
			Some(st) if st.size == rec.orig_size && st.time == rec.orig_stamp => Orig::Same,
			Some(_) => Orig::Changed,
		}
	}

	/// The answer to the waiting question. At launch Yes is Recover, No Discard and Cancel (Esc) Later; at Open Yes is Recover, No
	/// Discard and Cancel (Esc) Open original, so Esc never deletes anything. The second-window question is Yes (open it here as well)
	/// or No. After a launch answer other than Recover the next copy's question is raised.
	pub fn recovery_answered<S: Store + ?Sized, F: Files>(
		&mut self,
		s: &mut S,
		f: &mut F,
		a: Answer,
	) {
		if !self.recovery_pending() || self.rcv.flags & ASKED == 0 {
			return;
		}
		let Some(id) = self.rcv.list else { return };
		let slot = slot_get(&*s, id, PENDING_SLOT);
		let flags = self.rcv.flags;
		self.rcv.flags &= !(PENDING | ASKED);
		self.msg.clear();
		if flags & Q_SECOND != 0 {
			if a == Answer::Yes && self.open_plain(s, f, slot.bytes()).is_ok() {
				self.rcv.flags |= SECOND;
			}
		} else if flags & Q_OPEN != 0 {
			self.answer_open(s, f, slot.bytes(), a);
		} else {
			self.answer_launch(s, f, &slot, a);
		}
		self.sync_layout(s);
		self.scroll(s);
	}

	fn answer_open<S: Store + ?Sized, F: Files>(
		&mut self,
		s: &mut S,
		f: &mut F,
		name: &[u8],
		a: Answer,
	) {
		let style = self.style();
		let cur = self.locate(&*s, f, name);
		match (a, cur) {
			(Answer::Yes, Some(cur)) => {
				let Some(copy) = self.vet(f, &cur) else {
					return;
				};
				let copy = self.emergency_for(f, &copy, &cur).unwrap_or(copy);
				self.recover_copy(s, f, &copy, Some(name), false);
			}
			(Answer::No, cur) => {
				if cur.is_some() {
					self.remove_copies(s, f, name);
				}
				if self.open_plain(s, f, name).is_err() {
					self.msg.set(words::CANNOT_OPEN);
				}
			}
			(_, cur) => {
				if self.open_plain(s, f, name).is_err() {
					self.msg.set(words::CANNOT_OPEN);
				} else if let Some(cur) = cur {
					if f.exists(cur.bytes()) {
						self.rcv.flags |= LEFT;
					}
					if f.exists(kind_of(&cur, style, Kind::Aside).bytes()) {
						self.rcv.flags |= ASIDE;
					}
				}
			}
		}
	}

	fn answer_launch<S: Store + ?Sized, F: Files>(
		&mut self,
		s: &mut S,
		f: &mut F,
		copy: &Path,
		a: Answer,
	) {
		let style = self.style();
		let legacy = copy.bytes().ends_with(b"QUILL.RCV");
		match a {
			Answer::Yes => {
				self.recover_copy(s, f, copy, None, legacy);
				return;
			}
			Answer::No if legacy => {
				f.remove(copy.bytes());
			}
			Answer::No => {
				let orig = self.header_of(f, copy).map(|r| Path::of(r.path()));
				for k in [Kind::Current, Kind::Aside, Kind::Temp, Kind::Emergency] {
					f.remove(kind_of(copy, style, k).bytes());
				}
				if let Some(o) = orig {
					if self.recovery_list_remove(s, o.bytes()) {
						self.persist(s, f);
					}
				}
			}
			Answer::Cancel => self.rcv.skip = self.rcv.skip.saturating_add(1),
		}
		self.recovery_check_launch(s, f);
	}

	/// Opens a recovery copy as the document. `open_as` is the name the person opened (a copy found at Open), `None` at launch, where
	/// the copy's own record says. The original's state decides the name: as it was, the document takes its name and is marked
	/// modified; saved since, or untitled, it is `(Untitled)`; deleted, it is `(Untitled)` and Save As starts at its folder and name.
	fn recover_copy<S: Store + ?Sized, F: Files>(
		&mut self,
		s: &mut S,
		f: &mut F,
		copy: &Path,
		open_as: Option<&[u8]>,
		legacy: bool,
	) {
		let style = self.style();
		let rec = (!legacy).then(|| self.header_of(f, copy)).flatten();
		// A document that is given up for its own recovery copy does not take that copy with it.
		if let Some(own) = self.own_copy(&*s, f) {
			if kind_of(&own, style, Kind::Current).bytes()
				== kind_of(copy, style, Kind::Current).bytes()
			{
				self.rcv.flags &= !(WROTE | ASIDE | LEFT);
			}
		}
		if self.open_file(s, f, copy.bytes()).is_err() {
			if !legacy {
				self.set_bad(f, copy);
			}
			self.msg.set(words::CANNOT_OPEN);
			return;
		}
		self.recent.remove(copy.bytes());
		let mut name = Path::new();
		if let Some(r) = &rec {
			self.fmt = r.format.min(2);
			self.rcv.orig_size = r.orig_size;
			self.rcv.orig_time = r.orig_stamp;
			let target = open_as.unwrap_or(r.path());
			if !target.is_empty() && r.flags & RCV_UNTITLED == 0 {
				match self.orig_state(f, target, r) {
					Orig::Same => name = Path::of(target),
					Orig::Changed => {}
					Orig::Deleted => {
						// Save As starts at the folder and name the document had.
						let n = r.path().len().min(NAME_MAX);
						if let (Some(d), Some(sv)) = (self.folder.get_mut(..n), r.path().get(..n)) {
							d.copy_from_slice(sv);
						}
						self.folder_len = n;
						self.rcv.flags |= SUGGEST;
					}
				}
			}
			self.caret = r.caret.min(self.doc.body.len().saturating_sub(1));
			self.top = r.top_line;
			// A folder that moved with its files: the list follows the document.
			if open_as.is_some_and(|o| name.len > 0 && !same(o, r.path())) {
				self.recovery_list_remove(s, r.path());
				self.recovery_list_add(s, name.bytes());
				self.persist(s, f);
			}
		}
		self.set_name(name.bytes());
		self.modified = true;
		self.rcv.flags |= WROTE | RECOVERED;
		if legacy {
			self.rcv.flags |= LEGACY;
		} else if f.exists(kind_of(copy, style, Kind::Aside).bytes()) {
			self.rcv.flags |= ASIDE;
		}
		if name.len == 0 && !legacy {
			// The copy of a document with no name belongs to this session now: it takes the session's name in the central folder.
			let session = self.session(f);
			let mut n = 0;
			while let Some(dir) = self.central_dir(&*s, f, n) {
				n += 1;
				let to = central(style, Kind::Current, dir.bytes(), None, session, 0);
				if to.is_some_and(|to| f.rename(copy.bytes(), to.bytes())) {
					// Nothing is beside the original any more, so the list does not name it.
					if let Some(r) = &rec {
						if !r.path().is_empty() && self.recovery_list_remove(s, r.path()) {
							self.persist(s, f);
						}
					}
					break;
				}
			}
		}
		self.rcv_after(f);
		self.reflow.invalidate();
	}
}

/// `dir` with a separator on the end when it has none.
fn with_sep(dir: &[u8], style: Style) -> Path {
	let mut p = Path::of(dir);
	if !dir.is_empty() && !dir.last().is_some_and(|b| is_sep(*b)) {
		p.push(&[sep(style)]);
	}
	p
}

/// Where the old `QUILL.RCV` may be: `QUILL.RCV` in the shell's `n`th folder.
fn legacy_path<F: Files>(f: &mut F, n: u8, style: Style) -> Option<Path> {
	let mut dir = Path::new();
	let len = f.recovery_dir(n, dir.buf.get_mut(..NAME_MAX).unwrap_or(&mut []));
	if len == 0 {
		return None;
	}
	dir.len = len.min(NAME_MAX);
	let mut p = with_sep(dir.bytes(), style);
	p.push(b"QUILL.RCV");
	Some(p)
}
