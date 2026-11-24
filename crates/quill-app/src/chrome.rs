// What the toolbar and ruler need from the editor: the format at the caret, and commands that set a face, size, colour, indent or tab stop.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use quill_core::doc::{cflag, CFmt, Colour, Font, PFmt, TabSet, TabStop, NO_TABS};
use quill_core::layout::Measure;
use quill_core::store::{ReadStore, Store};

use crate::App;

/// The character format at the caret, in terms a toolbar shows.
#[derive(Copy, Clone)]
pub struct CharState {
	/// The face name.
	pub face: [u8; 31],
	/// How many bytes of `face`.
	pub face_len: u8,
	/// Size in half points.
	pub size: u16,
	/// `cflag` bits.
	pub flags: u16,
	/// Red, green and blue, or `None` for Automatic.
	pub colour: Option<(u8, u8, u8)>,
}

/// A built-in paragraph preset: direct formatting, not a style sheet (D85). The style box shows its name while a paragraph matches it.
struct Preset {
	align: u8,
	left: i32,
	right: i32,
	first: i32,
	before: i32,
	after: i32,
	/// Character size in half points and the bold, italic and underline bits.
	size: u16,
	flags: u16,
}

#[allow(clippy::too_many_arguments)] // a table row: every field is a number and the order is the struct's
const fn preset(
	align: u8,
	left: i32,
	right: i32,
	first: i32,
	before: i32,
	after: i32,
	size: u16,
	flags: u16,
) -> Preset {
	Preset {
		align,
		left,
		right,
		first,
		before,
		after,
		size,
		flags,
	}
}

const BI: u16 = cflag::BOLD | cflag::ITALIC;
const PRESETS: [Preset; 8] = [
	preset(0, 0, 0, 0, 0, 0, 24, 0),
	preset(0, 0, 0, 0, 240, 120, 32, cflag::BOLD),
	preset(0, 0, 0, 0, 200, 100, 28, cflag::BOLD),
	preset(0, 0, 0, 0, 160, 80, 24, BI),
	preset(1, 0, 0, 0, 0, 240, 40, cflag::BOLD),
	preset(0, 720, 720, 0, 120, 120, 24, cflag::ITALIC),
	preset(0, 720, 0, 0, 0, 60, 24, 0),
	preset(0, 720, 0, -720, 0, 0, 24, 0),
];

/// The names of the built-in paragraph presets, in the order the style box lists them.
pub const STYLE_NAMES: [&str; 8] = [
	"Normal",
	"Heading 1",
	"Heading 2",
	"Heading 3",
	"Title",
	"Quote",
	"List Paragraph",
	"Hanging Indent",
];

const STYLE_BITS: u16 = cflag::BOLD | cflag::ITALIC | cflag::UNDERLINE;

/// Which part of the window chrome a command shows or hides.
pub const CHROME_TOOLBAR: u8 = 1;
/// The ruler.
pub const CHROME_RULER: u8 = 2;
/// The formatting bar: style, font, size, character and paragraph buttons.
pub const CHROME_FORMAT: u8 = 8;
/// The status bar.
pub const CHROME_STATUS: u8 = 4;

impl<M: Measure + Clone> App<M> {
	/// True when the toolbar shows.
	pub fn show_toolbar(&self) -> bool {
		self.chrome & CHROME_TOOLBAR != 0
	}

	/// True when the formatting bar shows.
	pub fn show_format_bar(&self) -> bool {
		self.chrome & CHROME_FORMAT != 0
	}

	/// True when the ruler shows.
	pub fn show_ruler(&self) -> bool {
		self.chrome & CHROME_RULER != 0
	}

	/// True when the status bar shows.
	pub fn show_status(&self) -> bool {
		self.chrome & CHROME_STATUS != 0
	}

	/// True when the part of the chrome named by `bit` shows.
	pub fn chrome_on(&self, bit: u8) -> bool {
		self.chrome & bit != 0
	}

	/// The character format a toolbar shows: the typing format, or the one at the start of the selection or before the caret.
	pub fn char_state<S: ReadStore + ?Sized>(&self, s: &S) -> CharState {
		let at = self
			.selection()
			.map_or(self.caret.saturating_sub(1), |(lo, _)| lo);
		let cur = self
			.typing_fmt
			.or_else(|| self.doc.body.cfmt_at(s, at))
			.unwrap_or(0);
		let cf = self.doc.cfmts.get(s, cur).unwrap_or(CFmt {
			font: 0,
			size: 24,
			colour: 0,
			vpos: 0,
			flags: 0,
		});
		let font = self
			.doc
			.fonts
			.get(s, cf.font)
			.unwrap_or(Font::new(b"Times New Roman", 1));
		let colour = (cf.colour != 0)
			.then(|| self.doc.colours.get(s, u16::from(cf.colour)))
			.flatten()
			.map(|c| (c.r, c.g, c.b));
		CharState {
			face: font.name,
			face_len: font.name_len,
			size: cf.size,
			flags: cf.flags,
			colour,
		}
	}

	/// The paragraph format at the caret.
	pub fn para_state<S: ReadStore + ?Sized>(&self, s: &S) -> PFmt {
		self.para_pfmt(s, self.caret)
	}

	/// The tab stops of the paragraph at the caret: (position in twips, kind).
	pub fn tab_stops<S: ReadStore + ?Sized>(&self, s: &S, out: &mut [(i32, u8); 16]) -> usize {
		let pf = self.para_pfmt(s, self.caret);
		if pf.tabset == NO_TABS {
			return 0;
		}
		let set = self.doc.tabsets.get(s, pf.tabset).unwrap_or_default();
		let n = usize::from(set.count).min(16);
		for (o, t) in out.iter_mut().zip(set.stops.iter().take(n)) {
			*o = (t.pos, t.kind);
		}
		n
	}

	/// The tab stops of the paragraph at the caret with their leaders: (position in twips, kind, leader).
	pub fn tab_stops_full<S: ReadStore + ?Sized>(
		&self,
		s: &S,
		out: &mut [(i32, u8, u8); 16],
	) -> usize {
		let pf = self.para_pfmt(s, self.caret);
		if pf.tabset == NO_TABS {
			return 0;
		}
		let set = self.doc.tabsets.get(s, pf.tabset).unwrap_or_default();
		let n = usize::from(set.count).min(16);
		for (o, t) in out.iter_mut().zip(set.stops.iter().take(n)) {
			*o = (t.pos, t.kind, t.leader);
		}
		n
	}

	/// Sets the face of the selection (or of what is typed next).
	pub fn set_face<S: Store + ?Sized>(&mut self, s: &mut S, name: &[u8], family: u8) {
		if let Ok(fi) = self.doc.fonts.intern(s, Font::new(name, family)) {
			self.char_command(s, move |c| CFmt { font: fi, ..c }, None);
		}
		self.sync_layout(s);
	}

	/// Sets the size, in half points.
	pub fn set_size<S: Store + ?Sized>(&mut self, s: &mut S, half_points: u16) {
		let size = half_points.clamp(8, 288);
		self.char_command(s, move |c| CFmt { size, ..c }, None);
		self.sync_layout(s);
	}

	/// Sets the text colour, or Automatic for `None`.
	pub fn set_colour<S: Store + ?Sized>(&mut self, s: &mut S, rgb: Option<(u8, u8, u8)>) {
		let idx = match rgb {
			None => 0u8,
			Some((r, g, b)) => match self.doc.colours.intern(s, Colour { r, g, b }) {
				Ok(i) => u8::try_from(i).unwrap_or(0),
				Err(_) => return,
			},
		};
		self.char_command(s, move |c| CFmt { colour: idx, ..c }, None);
		self.sync_layout(s);
	}

	/// Sets the left, first-line and right indents of the paragraphs in the selection, in twips.
	pub fn set_indents<S: Store + ?Sized>(&mut self, s: &mut S, left: i32, first: i32, right: i32) {
		self.para_command(s, move |p| PFmt {
			left: left.max(0),
			first,
			right: right.max(0),
			..p
		});
		self.sync_layout(s);
	}

	/// Adds (or changes) a tab stop at `pos` twips of the given kind (0 left, 1 centre, 2 right, 3 decimal).
	pub fn add_tab<S: Store + ?Sized>(&mut self, s: &mut S, pos: i32, kind: u8) {
		self.tab_edit(s, "< Set >", Some(pos), kind, 0);
		self.sync_layout(s);
	}

	/// Removes the tab stop at `pos` twips.
	pub fn remove_tab<S: Store + ?Sized>(&mut self, s: &mut S, pos: i32) {
		self.tab_edit(s, "< Clear >", Some(pos), 0, 0);
		self.sync_layout(s);
	}

	/// The empty tab set, for callers that build one.
	pub fn no_tabs() -> TabSet {
		TabSet::default()
	}

	/// The index of the preset the paragraph at the caret matches exactly (paragraph format and the character format at its start), if any.
	pub fn style_of<S: ReadStore + ?Sized>(&self, s: &S) -> Option<usize> {
		let pf = self.para_pfmt(s, self.caret);
		let start = self.para_start(s, self.caret);
		let cf = self
			.doc
			.body
			.cfmt_at(s, start)
			.and_then(|i| self.doc.cfmts.get(s, i))?;
		PRESETS.iter().position(|p| {
			pf.align == p.align
				&& pf.left == p.left
				&& pf.right == p.right
				&& pf.first == p.first
				&& pf.before == p.before
				&& pf.after == p.after
				&& pf.spacing == 0
				&& pf.flags & 5 == 0
				&& cf.size == p.size
				&& cf.flags & STYLE_BITS == p.flags
		})
	}

	/// Applies preset `n` to every paragraph the selection touches, as one undo step: the paragraph format, and the size and the bold,
	/// italic and underline bits of every character in the paragraphs. Tab stops, face and colour stay.
	pub fn apply_style<S: Store + ?Sized>(&mut self, s: &mut S, n: usize) {
		let Some(p) = PRESETS.get(n) else {
			return;
		};
		let (lo, hi) = self.selection().unwrap_or((self.caret, self.caret));
		let total = self.doc.body.len();
		let mut at = self.para_start(s, lo);
		self.doc.body.begin_group(crate::label::PARA_FORMAT);
		while let Some(mark) = self.para_mark(s, at) {
			let old = self
				.doc
				.body
				.pfmt_at(s, mark)
				.and_then(|i| self.doc.pfmts.get(s, i))
				.unwrap_or(PFmt::DEFAULT);
			let new = PFmt {
				align: p.align,
				spacing: 0,
				flags: 0,
				list: 0,
				level: 0,
				left: p.left,
				right: p.right,
				first: p.first,
				before: p.before,
				after: p.after,
				tabset: old.tabset,
			};
			if let Ok(idx) = self.doc.pfmts.intern(s, new) {
				let _ =
					self.doc
						.body
						.set_pfmt(s, at, mark + 1 - at, idx, crate::label::PARA_FORMAT);
			}
			let mut pos = at;
			while pos < mark + 1 {
				let mut runs = [(0u32, 0u32, 0u16); 32];
				let mut n_runs = 0;
				self.doc
					.body
					.for_each_run(s, pos, mark + 1 - pos, |rp, run| {
						if let Some(slot) = runs.get_mut(n_runs) {
							*slot = (rp, run.len, run.cfmt);
							n_runs += 1;
						}
					});
				if n_runs == 0 {
					break;
				}
				for &(rp, len, cf) in runs.iter().take(n_runs) {
					if let Some(c) = self.doc.cfmts.get(s, cf) {
						let nc = CFmt {
							size: p.size,
							flags: (c.flags & !STYLE_BITS) | p.flags,
							..c
						};
						if let Ok(idx) = self.doc.cfmts.intern(s, nc) {
							let _ =
								self.doc
									.body
									.set_cfmt(s, rp, len, idx, crate::label::PARA_FORMAT);
						}
					}
					pos = rp + len;
				}
			}
			at = mark + 1;
			if at > hi || at >= total {
				break;
			}
		}
		self.doc.body.end_group();
		self.typing_fmt = None;
		self.modified = true;
		self.sync_layout(s);
	}

	/// Shows or hides a part of the chrome (`CHROME_*`).
	pub fn toggle_chrome(&mut self, bit: u8) {
		self.chrome ^= bit;
	}
}

/// A stop for tests and callers that need one.
pub fn tab_stop(pos: i32, kind: u8) -> TabStop {
	TabStop {
		pos,
		kind,
		leader: 0,
	}
}
