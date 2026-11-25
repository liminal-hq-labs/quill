// Small form dialogs (Fonts and Tabs) and the clipboard commands.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use quill_core::doc::{cflag, CFmt, Font, PFmt, TabSet, TabStop, MAX_STOPS, NO_TABS};
use quill_core::store::{ReadStore, Store};

use quill_core::layout::Measure;

use crate::{label, App, Command, Key, Mode};

pub(crate) const FONT_NAMES: [&str; 3] = ["Times New Roman", "Arial", "Courier New"];
pub(crate) const FONT_FAMILY: [u8; 3] = [1, 2, 3];
const KINDS: [&str; 4] = ["Left", "Centre", "Right", "Decimal"];
const LEADERS: [&str; 4] = ["None", "Dots", "Hyphens", "Underline"];

/// Which form is open.
#[derive(Copy, Clone, PartialEq, Eq)]
pub(crate) enum FormKind {
	Fonts,
	Tabs,
}

/// One row of a form.
#[derive(Copy, Clone, PartialEq, Eq)]
pub(crate) struct Row {
	pub label: &'static str,
	/// 0 text, 1 choice, 2 check box, 3 button.
	pub kind: u8,
	pub text: [u8; 12],
	pub len: usize,
	pub val: u8,
	pub names: &'static [&'static str],
}

const EMPTY: Row = Row {
	label: "",
	kind: 3,
	text: [0; 12],
	len: 0,
	val: 0,
	names: &[],
};

/// A form dialog.
#[derive(Copy, Clone, PartialEq, Eq)]
pub(crate) struct Form {
	pub kind: FormKind,
	pub rows: [Row; 8],
	pub n: usize,
	pub focus: usize,
}

fn text_row(label: &'static str, t: &[u8]) -> Row {
	let mut r = Row {
		label,
		kind: 0,
		..EMPTY
	};
	for (d, s) in r.text.iter_mut().zip(t) {
		*d = *s;
	}
	r.len = t.len().min(12);
	r
}

fn decimal(mut v: u32, out: &mut [u8; 12]) -> usize {
	let mut tmp = [0u8; 12];
	let mut i = tmp.len();
	loop {
		i -= 1;
		if let Some(t) = tmp.get_mut(i) {
			*t = b'0' + (v % 10) as u8;
		}
		v /= 10;
		if v == 0 {
			break;
		}
	}
	let n = tmp.len() - i;
	for (d, s) in out.iter_mut().zip(tmp.get(i..).unwrap_or(&[])) {
		*d = *s;
	}
	n
}

fn parse(r: &Row) -> Option<u32> {
	let mut v = 0u32;
	let t = r.text.get(..r.len)?;
	if t.is_empty() {
		return None;
	}
	for &c in t {
		if !c.is_ascii_digit() {
			return None;
		}
		v = v.checked_mul(10)?.checked_add(u32::from(c - b'0'))?;
	}
	Some(v)
}

/// Parses a position in inches with up to two decimals ("1", "1.5", "0.25") into twips.
fn parse_inches(r: &Row) -> Option<i32> {
	crate::flow::parse_inches(r.text.get(..r.len)?)
}

impl<M: Measure + Clone> App<M> {
	pub(crate) fn open_fonts<S: ReadStore + ?Sized>(&mut self, s: &S) {
		let t = &self.doc.body;
		let at = if self.caret > 0 { self.caret - 1 } else { 0 };
		let cf = self
			.typing_fmt
			.or_else(|| t.cfmt_at(s, at))
			.and_then(|i| self.doc.cfmts.get(s, i));
		let c = cf.unwrap_or(CFmt {
			font: 0,
			size: 24,
			colour: 0,
			vpos: 0,
			flags: 0,
		});
		let name = self
			.doc
			.fonts
			.get(s, c.font)
			.map(|f| f.name_len)
			.unwrap_or(0);
		let _ = name;
		let current = self.doc.fonts.get(s, c.font);
		let pick = current
			.and_then(|f| FONT_NAMES.iter().position(|n| n.as_bytes() == f.name()))
			.unwrap_or(0) as u8;
		let mut size = [0u8; 12];
		let n = decimal(u32::from(c.size / 2), &mut size);
		let mut rows = [EMPTY; 8];
		rows[0] = Row {
			label: "Font:",
			kind: 1,
			val: pick,
			names: &FONT_NAMES,
			..EMPTY
		};
		rows[1] = text_row("Size:", size.get(..n).unwrap_or(&[]));
		for (i, (l, bit)) in [
			("Bold", cflag::BOLD),
			("Italic", cflag::ITALIC),
			("Underline", cflag::UNDERLINE),
			("Strikeout", cflag::STRIKE),
		]
		.iter()
		.enumerate()
		{
			if let Some(r) = rows.get_mut(2 + i) {
				*r = Row {
					label: l,
					kind: 2,
					val: u8::from(c.flags & bit != 0),
					..EMPTY
				};
			}
		}
		rows[6] = Row {
			label: "< OK >",
			..EMPTY
		};
		rows[7] = Row {
			label: "< Cancel >",
			..EMPTY
		};
		self.mode = Mode::Form(Form {
			kind: FormKind::Fonts,
			rows,
			n: 8,
			focus: 0,
		});
	}

	pub(crate) fn open_tabs(&mut self) {
		let mut rows = [EMPTY; 8];
		rows[0] = text_row("Position (inches):", b"");
		rows[1] = Row {
			label: "Kind:",
			kind: 1,
			names: &KINDS,
			..EMPTY
		};
		rows[2] = Row {
			label: "Leader:",
			kind: 1,
			names: &LEADERS,
			..EMPTY
		};
		rows[3] = Row {
			label: "< Set >",
			..EMPTY
		};
		rows[4] = Row {
			label: "< Clear >",
			..EMPTY
		};
		rows[5] = Row {
			label: "< Clear All >",
			..EMPTY
		};
		rows[6] = Row {
			label: "< Close >",
			..EMPTY
		};
		self.mode = Mode::Form(Form {
			kind: FormKind::Tabs,
			rows,
			n: 7,
			focus: 0,
		});
	}

	#[inline(never)]
	pub(crate) fn key_form<S: Store + ?Sized>(&mut self, s: &mut S, k: Key, mut f: Form) {
		let n = f.n;
		let row = f.rows.get(f.focus).copied().unwrap_or(EMPTY);
		match k {
			Key::Esc => {
				self.mode = Mode::Edit;
				return;
			}
			Key::Tab => f.focus = (f.focus + 1) % n,
			Key::Char(c) if row.kind == 0 => {
				if let Some(r) = f.rows.get_mut(f.focus) {
					if r.len < 12 && (c.is_ascii_digit() || c == b'.') {
						if let Some(slot) = r.text.get_mut(r.len) {
							*slot = c;
							r.len += 1;
						}
					}
				}
			}
			Key::Backspace if row.kind == 0 => {
				if let Some(r) = f.rows.get_mut(f.focus) {
					r.len = r.len.saturating_sub(1);
				}
			}
			Key::Char(b' ') | Key::Down | Key::Up if row.kind == 1 || row.kind == 2 => {
				if let Some(r) = f.rows.get_mut(f.focus) {
					let m = if r.kind == 1 { r.names.len() as u8 } else { 2 };
					r.val = if k == Key::Up {
						(r.val + m - 1) % m
					} else {
						(r.val + 1) % m
					};
				}
			}
			Key::Enter => {
				// Enter on a button presses it; elsewhere it presses the first button.
				let press = if row.kind == 3 {
					f.focus
				} else {
					f.rows.iter().take(n).position(|r| r.kind == 3).unwrap_or(0)
				};
				if self.press(s, &mut f, press) {
					return;
				}
			}
			_ => {}
		}
		self.mode = Mode::Form(f);
	}

	/// Presses button `i`. Returns true when the form has closed.
	fn press<S: Store + ?Sized>(&mut self, s: &mut S, f: &mut Form, i: usize) -> bool {
		let label = f.rows.get(i).map_or("", |r| r.label);
		match (f.kind, label) {
			(FormKind::Fonts, "< OK >") => {
				let pick = usize::from(f.rows.first().map_or(0, |r| r.val));
				let size = f.rows.get(1).and_then(parse).unwrap_or(12).clamp(4, 144) as u16 * 2;
				let flag = |j: usize, bit: u16| {
					if f.rows.get(j).is_some_and(|r| r.val == 1) {
						bit
					} else {
						0
					}
				};
				let flags = flag(2, cflag::BOLD)
					| flag(3, cflag::ITALIC)
					| flag(4, cflag::UNDERLINE)
					| flag(5, cflag::STRIKE);
				let name = FONT_NAMES.get(pick).copied().unwrap_or("Times New Roman");
				let family = FONT_FAMILY.get(pick).copied().unwrap_or(1);
				self.mode = Mode::Edit;
				if let Ok(fi) = self.doc.fonts.intern(s, Font::new(name.as_bytes(), family)) {
					self.char_command(
						s,
						move |c| CFmt {
							font: fi,
							size,
							flags: (c.flags & !0xF) | flags,
							..c
						},
						None,
					);
				}
				true
			}
			(FormKind::Fonts, _) => {
				self.mode = Mode::Edit;
				true
			}
			(FormKind::Tabs, "< Close >") => {
				self.mode = Mode::Edit;
				true
			}
			(FormKind::Tabs, l) => {
				let pos = f.rows.first().and_then(parse_inches);
				let kind = f.rows.get(1).map_or(0, |r| r.val);
				let leader = f.rows.get(2).map_or(0, |r| r.val);
				self.tab_edit(s, l, pos, kind, leader);
				false
			}
		}
	}

	pub(crate) fn tab_edit<S: Store + ?Sized>(
		&mut self,
		s: &mut S,
		what: &str,
		pos: Option<i32>,
		kind: u8,
		leader: u8,
	) {
		let pf = self.para_pfmt(s, self.caret);
		let mut set = if pf.tabset == NO_TABS {
			TabSet::default()
		} else {
			self.doc.tabsets.get(s, pf.tabset).unwrap_or_default()
		};
		match (what, pos) {
			("< Set >", Some(p)) => {
				let n = usize::from(set.count);
				let existing = set.stops.iter().take(n).position(|t| t.pos == p);
				if let Some(i) = existing {
					if let Some(t) = set.stops.get_mut(i) {
						*t = TabStop {
							pos: p,
							kind,
							leader,
						};
					}
				} else if n < MAX_STOPS {
					if let Some(t) = set.stops.get_mut(n) {
						*t = TabStop {
							pos: p,
							kind,
							leader,
						};
					}
					set.count += 1;
					let c = usize::from(set.count);
					if let Some(live) = set.stops.get_mut(..c) {
						live.sort_unstable_by_key(|t| t.pos);
					}
				} else {
					self.msg.set("A paragraph holds at most 16 tab stops.");
					return;
				}
			}
			("< Clear >", Some(p)) => {
				let n = usize::from(set.count);
				if let Some(i) = set.stops.iter().take(n).position(|t| t.pos == p) {
					for j in i..n.saturating_sub(1) {
						let next = set.stops.get(j + 1).copied().unwrap_or_default();
						if let Some(t) = set.stops.get_mut(j) {
							*t = next;
						}
					}
					if let Some(t) = set.stops.get_mut(n - 1) {
						*t = TabStop::default();
					}
					set.count -= 1;
				}
			}
			("< Clear All >", _) => set = TabSet::default(),
			_ => return,
		}
		let idx = if set.count == 0 {
			NO_TABS
		} else {
			self.doc.tabsets.intern(s, set).unwrap_or(NO_TABS)
		};
		self.para_command(s, move |p| PFmt { tabset: idx, ..p });
	}

	/// Copies the selection to the clipboard, which lives until Quill exits.
	pub(crate) fn clip_copy<S: ReadStore + ?Sized>(&mut self, s: &S) -> bool {
		let Some((lo, hi)) = self.selection() else {
			return false;
		};
		let n = ((hi - lo) as usize).min(self.clip.len());
		self.clip_len = self
			.clip
			.get_mut(..n)
			.map_or(0, |b| self.doc.body.read(s, lo, b));
		true
	}

	pub(crate) fn clip_command<S: Store + ?Sized>(&mut self, s: &mut S, c: Command) {
		match c {
			Command::Copy => {
				self.clip_copy(s);
			}
			Command::Cut => {
				if self.clip_copy(s) {
					self.end_typing();
					self.delete_selection(s);
				}
			}
			Command::Paste => {
				let text = self.clip;
				let n = self.clip_len;
				self.end_typing();
				self.doc.body.begin_group(label::TYPING);
				self.typing = true;
				if self.selection().is_some() {
					self.delete_selection(s);
				}
				for &b in text.get(..n).unwrap_or(&[]) {
					self.insert(s, b);
				}
				self.end_typing();
			}
			Command::SelectAll => {
				self.anchor = Some(0);
				self.caret = self.doc.body.len().saturating_sub(1);
			}
			_ => {}
		}
	}
}
