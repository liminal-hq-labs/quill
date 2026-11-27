// The file dialog as rectangles in cells: one description for the text painter, the graphics painter and both pointer hit tests.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// A cell is a character on the text screen and 8 by 22 pixels on the graphics screen (the grid the other graphics dialogs use). The
// layout names the parts a person can point at, gives each a rectangle, answers `hit` for a cell, and holds the strings, their
// mnemonics and the CP437 glyphs the text painter draws file types with. It draws nothing and reads no state: the same `Layout` and
// the same `FileDlg` give the same screen, and a click on a cell reaches the operation the key for that part reaches.
//
// The text dialog is 76 cells wide and `rows - 5` tall at column 1, row 1, so the menu bar and the status line stay visible: three
// panes (Places 14 wide, the list 36, the preview 20) with `rows - 13` list rows (12 at 25 rows, 37 at 50). The graphics dialog is 77
// by 17 cells (616 by 374 pixels), centred; its list is the pane's height less a header row, and the painter divides that into
// items of its own pitch (16 pixels), asking [`Layout::items_that_fit`] how many.

use crate::filedlg::SortBy;
use crate::filedlg::{CANCEL, LIST, LOOK_IN, NAME, NEW_FOLDER, OK, PLACES, TYPES, UP};
use crate::types::Kind;

/// The columns of the text screen.
pub const TEXT_COLS: u16 = 80;
/// The cells of the graphics screen (640 by 480 pixels of 8 by 22).
pub const GFX_COLS: u16 = 80;
pub const GFX_ROWS: u16 = 21;
/// The height of a cell on the graphics screen, in pixels.
pub const GFX_CELL_H: u32 = 22;
/// The pitch of a list item on the graphics screen, in pixels.
pub const GFX_ITEM_H: u32 = 16;
/// The smallest text screen the dialog fits on: the list shows 3 rows.
pub const TEXT_ROWS_MIN: u16 = 16;

/// A rectangle of cells.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub struct Rect {
	pub x: u16,
	pub y: u16,
	pub w: u16,
	pub h: u16,
}

impl Rect {
	pub const fn new(x: u16, y: u16, w: u16, h: u16) -> Rect {
		Rect { x, y, w, h }
	}

	/// True when the cell is inside.
	pub fn contains(&self, x: u16, y: u16) -> bool {
		x >= self.x && y >= self.y && x - self.x < self.w && y - self.y < self.h
	}

	/// The cell at the middle (rounding down).
	pub fn centre(&self) -> (u16, u16) {
		(self.x + self.w / 2, self.y + self.h / 2)
	}

	/// True when no cell is in both.
	pub fn apart_from(&self, o: &Rect) -> bool {
		self.x + self.w <= o.x
			|| o.x + o.w <= self.x
			|| self.y + self.h <= o.y
			|| o.y + o.h <= self.y
	}

	/// True when every cell of `o` is in this one.
	pub fn encloses(&self, o: &Rect) -> bool {
		o.x >= self.x
			&& o.y >= self.y
			&& o.x + o.w <= self.x + self.w
			&& o.y + o.h <= self.y + self.h
	}
}

/// What a cell can be pointed at.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Part {
	/// `Look in:`, which focuses its field
	LookInLabel,
	/// The path field of the Look in drop-down
	LookIn,
	/// Its arrow
	LookInDrop,
	Up,
	/// Save As only
	NewFolder,
	/// The places' rows
	Places,
	/// A column header, which sorts by it
	Header(SortBy),
	/// The list's rows
	List,
	/// The list's scroll bar
	ListScroll,
	/// The preview pane
	Preview,
	NameLabel,
	Name,
	TypeLabel,
	TypeBox,
	TypeDrop,
	Open,
	Cancel,
	/// The line of counts under the panes
	Status,
}

impl Part {
	/// The control of the dialog's focus ring that a press on this part focuses, in [`crate::filedlg`]'s numbering.
	pub fn control(self) -> Option<u8> {
		match self {
			Part::LookInLabel | Part::LookIn | Part::LookInDrop => Some(LOOK_IN),
			Part::Up => Some(UP),
			Part::NewFolder => Some(NEW_FOLDER),
			Part::Places => Some(PLACES),
			Part::Header(_) | Part::List | Part::ListScroll => Some(LIST),
			Part::NameLabel | Part::Name => Some(NAME),
			Part::TypeLabel | Part::TypeBox | Part::TypeDrop => Some(TYPES),
			Part::Open => Some(OK),
			Part::Cancel => Some(CANCEL),
			Part::Preview | Part::Status => None,
		}
	}
}

/// The screen the dialog is laid out for.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Screen {
	/// 80 columns and this many rows (25 or 50, or any from [`TEXT_ROWS_MIN`])
	Text(u16),
	/// The 640 by 480 screen in 8 by 22 cells
	Graphics,
}

/// Where the list's columns are, as columns of cells on the screen.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub struct Columns {
	/// The type icon
	pub icon_x: u16,
	/// The name, up to 12 bytes
	pub name_x: u16,
	pub name_w: u16,
	/// The size, right-aligned in this many cells
	pub size_x: u16,
	pub size_w: u16,
	pub date_x: u16,
	pub date_w: u16,
}

const MAX_PARTS: usize = 24;

/// The dialog's parts for one screen and one kind of dialog.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Layout {
	screen: Screen,
	kind: Kind,
	/// The whole dialog, frame and title included (the text shadow is two cells beyond it)
	pub dialog: Rect,
	/// The three panes with their frames (text) or their edges (graphics)
	pub places_pane: Rect,
	pub list_pane: Rect,
	pub preview_pane: Rect,
	/// How many cells tall the list's rows are
	pub list_rows: u16,
	pub columns: Columns,
	parts: [(Part, Rect); MAX_PARTS],
	n: usize,
}

impl Layout {
	/// The layout of a dialog of `kind` on `screen`. `None` when the text screen is too short.
	pub fn new(screen: Screen, kind: Kind) -> Option<Layout> {
		match screen {
			Screen::Text(rows) if rows >= TEXT_ROWS_MIN => Some(Layout::text(rows, kind)),
			Screen::Text(_) => None,
			Screen::Graphics => Some(Layout::graphics(kind)),
		}
	}

	fn empty(screen: Screen, kind: Kind, dialog: Rect) -> Layout {
		Layout {
			screen,
			kind,
			dialog,
			places_pane: Rect::default(),
			list_pane: Rect::default(),
			preview_pane: Rect::default(),
			list_rows: 0,
			columns: Columns::default(),
			parts: [(Part::Status, Rect::default()); MAX_PARTS],
			n: 0,
		}
	}

	fn push(&mut self, p: Part, x: u16, y: u16, w: u16, h: u16) {
		let (dx, dy) = (self.dialog.x, self.dialog.y);
		if let Some(slot) = self.parts.get_mut(self.n) {
			*slot = (p, Rect::new(dx + x, dy + y, w, h));
			self.n += 1;
		}
	}

	fn text(rows: u16, kind: Kind) -> Layout {
		let h = rows - 5;
		let list_rows = rows - 13;
		let mut l = Layout::empty(Screen::Text(rows), kind, Rect::new(1, 1, 76, h));
		let (dx, dy) = (l.dialog.x, l.dialog.y);
		let at = |x: u16, y: u16, w: u16, h: u16| Rect::new(dx + x, dy + y, w, h);
		l.list_rows = list_rows;
		l.places_pane = at(2, 2, 14, list_rows + 2);
		l.list_pane = at(17, 2, 36, list_rows + 2);
		l.preview_pane = at(54, 2, 20, list_rows + 2);
		l.columns = Columns {
			icon_x: dx + 18,
			name_x: dx + 20,
			name_w: 12,
			size_x: dx + 33,
			size_w: 7,
			date_x: dx + 41,
			date_w: 8,
		};
		let name_row = list_rows + 4;
		l.push(Part::LookInLabel, 2, 1, 8, 1);
		l.push(Part::LookIn, 11, 1, 37, 1);
		l.push(Part::LookInDrop, 48, 1, 2, 1);
		l.push(Part::Up, 52, 1, 6, 1);
		if kind == Kind::SaveAs {
			l.push(Part::NewFolder, 59, 1, 14, 1);
		}
		l.push(Part::Places, 3, 3, 12, list_rows);
		l.push(Part::Header(SortBy::Name), 18, 2, 14, 1);
		l.push(Part::Header(SortBy::Size), 32, 2, 8, 1);
		l.push(Part::Header(SortBy::Date), 40, 2, 11, 1);
		l.push(Part::List, 18, 3, 33, list_rows);
		l.push(Part::ListScroll, 51, 3, 1, list_rows);
		l.push(Part::Preview, 55, 3, 18, list_rows);
		l.push(Part::NameLabel, 2, name_row, 10, 1);
		l.push(Part::Name, 13, name_row, 22, 1);
		l.push(Part::TypeLabel, 38, name_row, 14, 1);
		l.push(Part::TypeBox, 53, name_row, 19, 1);
		l.push(Part::TypeDrop, 72, name_row, 2, 1);
		l.push(Part::Status, 2, name_row + 2, 46, 1);
		l.push(Part::Open, 52, name_row + 2, 10, 1);
		l.push(Part::Cancel, 64, name_row + 2, 10, 1);
		l
	}

	fn graphics(kind: Kind) -> Layout {
		let mut l = Layout::empty(Screen::Graphics, kind, Rect::new(1, 2, 77, 17));
		let (dx, dy) = (l.dialog.x, l.dialog.y);
		let at = |x: u16, y: u16, w: u16, h: u16| Rect::new(dx + x, dy + y, w, h);
		// Row 0 is the title bar; the panes start at row 2 and have a header row of their own.
		let pane_h = 12;
		l.list_rows = pane_h - 1;
		l.places_pane = at(1, 2, 13, pane_h);
		l.list_pane = at(15, 2, 37, pane_h);
		l.preview_pane = at(53, 2, 23, pane_h);
		l.columns = Columns {
			icon_x: dx + 15,
			name_x: dx + 17,
			name_w: 12,
			size_x: dx + 30,
			size_w: 8,
			date_x: dx + 39,
			date_w: 10,
		};
		l.push(Part::LookInLabel, 1, 1, 8, 1);
		l.push(Part::LookIn, 9, 1, 32, 1);
		l.push(Part::LookInDrop, 41, 1, 2, 1);
		l.push(Part::Up, 44, 1, 5, 1);
		if kind == Kind::SaveAs {
			l.push(Part::NewFolder, 50, 1, 12, 1);
		}
		l.push(Part::Places, 1, 3, 13, pane_h - 1);
		l.push(Part::Header(SortBy::Name), 15, 2, 15, 1);
		l.push(Part::Header(SortBy::Size), 30, 2, 9, 1);
		l.push(Part::Header(SortBy::Date), 39, 2, 11, 1);
		l.push(Part::List, 15, 3, 35, pane_h - 1);
		l.push(Part::ListScroll, 50, 2, 2, pane_h);
		l.push(Part::Preview, 53, 2, 23, pane_h);
		l.push(Part::NameLabel, 1, 14, 11, 1);
		l.push(Part::Name, 12, 14, 23, 1);
		l.push(Part::TypeLabel, 36, 14, 14, 1);
		l.push(Part::TypeBox, 50, 14, 25, 1);
		l.push(Part::TypeDrop, 75, 14, 1, 1);
		l.push(Part::Status, 1, 15, 50, 1);
		l.push(Part::Open, 55, 15, 10, 1);
		l.push(Part::Cancel, 66, 15, 10, 1);
		l
	}

	pub fn screen(&self) -> Screen {
		self.screen
	}

	pub fn kind(&self) -> Kind {
		self.kind
	}

	/// The parts and their rectangles, in no particular order; no two overlap.
	pub fn parts(&self) -> impl Iterator<Item = (Part, Rect)> + '_ {
		self.parts.iter().take(self.n).copied()
	}

	/// The rectangle of a part; `None` for a part this dialog does not have (New folder in Open).
	pub fn rect(&self, part: Part) -> Option<Rect> {
		self.parts().find(|(p, _)| *p == part).map(|(_, r)| r)
	}

	/// The part under a cell, or `None` on the frame, the shadow, a gap or off the dialog.
	pub fn hit(&self, x: u16, y: u16) -> Option<Part> {
		self.parts().find(|(_, r)| r.contains(x, y)).map(|(p, _)| p)
	}

	/// How many items of `pitch_px` pixels fit in the list, for the graphics painter (`cell_h` is 22 there).
	pub fn items_that_fit(&self, cell_h: u32, pitch_px: u32) -> u32 {
		let h = self.rect(Part::List).map_or(0, |r| u32::from(r.h));
		(h * cell_h).checked_div(pitch_px).unwrap_or(0)
	}

	/// How many items of the graphics list's pitch the list and the places show: 15.
	pub fn graphics_items(&self) -> u8 {
		self.items_that_fit(GFX_CELL_H, GFX_ITEM_H).min(255) as u8
	}

	/// The row of the list a cell is on, counted from the first visible entry, in text cells.
	pub fn list_row_at(&self, y: u16) -> Option<usize> {
		let r = self.rect(Part::List)?;
		(y >= r.y && y - r.y < r.h).then(|| usize::from(y - r.y))
	}
}

/// The text of the dialog, with `&` before each mnemonic. These are the labels of the focus ring in [`crate::filedlg`] and what the
/// painters draw.
pub mod text {
	pub const LOOK_IN: &[u8] = b"Look &in:";
	pub const UP: &[u8] = b"&Up";
	pub const NEW_FOLDER: &[u8] = b"New &folder";
	pub const PLACES: &[u8] = b"&Places";
	pub const LIST: &[u8] = b"Fi&les";
	pub const NAME: &[u8] = b"File &name:";
	pub const TYPE_OPEN: &[u8] = b"Files of &type:";
	pub const TYPE_SAVE: &[u8] = b"Save as &type:";
	pub const OPEN: &[u8] = b"Open";
	pub const SAVE: &[u8] = b"Save";
	pub const CANCEL: &[u8] = b"Cancel";
	/// The title in the frame
	pub const TITLE_OPEN: &[u8] = b"Open";
	pub const TITLE_SAVE: &[u8] = b"Save As";
	pub const TITLE_PICTURE: &[u8] = b"Insert Picture";
	/// The pane titles and the column headers
	pub const PLACES_TITLE: &[u8] = b"Places";
	pub const PREVIEW_TITLE: &[u8] = b"Preview";
	pub const HEADER_NAME: &[u8] = b"Name";
	pub const HEADER_SIZE: &[u8] = b"Size";
	pub const HEADER_DATE: &[u8] = b"Modified";
	/// What a folder's size shows in text mode
	pub const DIR_MARK: &[u8] = b"<DIR>";
	/// The last row of a list that was cut at the cap
	pub const MORE: &[u8] = b"(more files: type a pattern such as R*.QLD)";
	pub const READING: &[u8] = b"Reading...";
}

/// The title for a dialog of `kind`.
pub fn title(kind: Kind) -> &'static [u8] {
	match kind {
		Kind::Open => text::TITLE_OPEN,
		Kind::SaveAs => text::TITLE_SAVE,
		Kind::Picture => text::TITLE_PICTURE,
	}
}

/// The label of the type drop-down for a dialog of `kind`.
pub fn type_label(kind: Kind) -> &'static [u8] {
	if kind == Kind::SaveAs {
		text::TYPE_SAVE
	} else {
		text::TYPE_OPEN
	}
}

/// The caption of the default button.
pub fn default_button(kind: Kind) -> &'static [u8] {
	if kind == Kind::SaveAs {
		text::SAVE
	} else {
		text::OPEN
	}
}

/// What an icon stands for.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Icon {
	Parent,
	Folder,
	HardDisk,
	Floppy,
	CdRom,
	Network,
	Document,
	Rtf,
	Text,
	Picture,
	Other,
}

/// A text-mode glyph: a CP437 byte and its attribute on the list's black body (foreground only; a selected row takes the bar's).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Glyph {
	pub ch: u8,
	pub attr: u8,
}

/// The glyph for an icon. They carry the meaning without colour, so the monochrome scheme draws the same bytes and ignores the colours.
pub const fn glyph(icon: Icon) -> Glyph {
	match icon {
		Icon::Parent => Glyph {
			ch: 0x18,
			attr: 0x07,
		},
		Icon::Folder => Glyph {
			ch: 0xFE,
			attr: 0x06,
		},
		Icon::HardDisk | Icon::Network => Glyph {
			ch: 0x16,
			attr: 0x07,
		},
		Icon::Floppy | Icon::CdRom => Glyph {
			ch: 0xE9,
			attr: 0x07,
		},
		Icon::Document => Glyph {
			ch: 0x14,
			attr: 0x09,
		},
		Icon::Rtf => Glyph {
			ch: 0x15,
			attr: 0x0C,
		},
		Icon::Text => Glyph {
			ch: 0xF0,
			attr: 0x07,
		},
		Icon::Picture => Glyph {
			ch: 0x0F,
			attr: 0x0A,
		},
		Icon::Other => Glyph {
			ch: 0xFA,
			attr: 0x08,
		},
	}
}

/// The icon of a file by its extension: Quill documents, RTF, text, the pictures Quill reads, and anything else.
pub fn file_icon(name: &[u8]) -> Icon {
	let ext = crate::savename::ext_of(name).unwrap_or(&[]);
	let is = |e: &[u8]| ext.eq_ignore_ascii_case(e);
	if is(b"QLD") {
		Icon::Document
	} else if is(b"RTF") {
		Icon::Rtf
	} else if is(b"TXT") {
		Icon::Text
	} else if is(b"BMP") || is(b"PNG") {
		Icon::Picture
	} else {
		Icon::Other
	}
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
	use crate::focus::{mnemonic, strip_label};
	use std::string::String;
	use std::vec::Vec;

	fn all() -> Vec<Layout> {
		let mut v = Vec::new();
		for kind in [Kind::Open, Kind::SaveAs, Kind::Picture] {
			for rows in [TEXT_ROWS_MIN, 20, 25, 43, 50, 60] {
				v.push(Layout::new(Screen::Text(rows), kind).unwrap());
			}
			v.push(Layout::new(Screen::Graphics, kind).unwrap());
		}
		v
	}

	fn screen_rect(l: &Layout) -> Rect {
		match l.screen() {
			Screen::Text(rows) => Rect::new(0, 0, TEXT_COLS, rows),
			Screen::Graphics => Rect::new(0, 0, GFX_COLS, GFX_ROWS),
		}
	}

	#[test]
	fn the_list_has_rows_minus_13_rows_and_the_dialog_rows_minus_5_on_the_text_screen() {
		for (rows, list) in [(25u16, 12u16), (50, 37), (43, 30)] {
			let l = Layout::new(Screen::Text(rows), Kind::Open).unwrap();
			assert_eq!(l.list_rows, list);
			assert_eq!(l.rect(Part::List).unwrap().h, list);
			assert_eq!(l.dialog, Rect::new(1, 1, 76, rows - 5));
			// The shadow is two cells to the right and a row below, and the status line stays.
			assert!(l.dialog.x + l.dialog.w + 2 <= TEXT_COLS);
			assert!(l.dialog.y + l.dialog.h < rows - 1);
		}
		assert!(Layout::new(Screen::Text(TEXT_ROWS_MIN - 1), Kind::Open).is_none());
	}

	#[test]
	fn every_part_is_inside_the_dialog_and_the_dialog_inside_the_screen() {
		for l in all() {
			assert!(screen_rect(&l).encloses(&l.dialog), "{:?}", l.screen());
			for (p, r) in l.parts() {
				assert!(r.w > 0 && r.h > 0, "{p:?} is empty on {:?}", l.screen());
				assert!(
					l.dialog.encloses(&r),
					"{p:?} leaves the dialog on {:?}",
					l.screen()
				);
			}
			for pane in [l.places_pane, l.list_pane, l.preview_pane] {
				assert!(l.dialog.encloses(&pane));
			}
		}
	}

	#[test]
	fn no_two_parts_overlap() {
		for l in all() {
			let parts: Vec<(Part, Rect)> = l.parts().collect();
			for (i, (a, ra)) in parts.iter().enumerate() {
				for (b, rb) in &parts[i + 1..] {
					assert!(
						ra.apart_from(rb),
						"{a:?} and {b:?} overlap on {:?}",
						l.screen()
					);
				}
			}
		}
	}

	#[test]
	fn hit_of_every_cell_of_a_part_is_that_part_and_the_gaps_are_nothing() {
		for l in all() {
			for (p, r) in l.parts() {
				for y in r.y..r.y + r.h {
					for x in r.x..r.x + r.w {
						assert_eq!(l.hit(x, y), Some(p), "({x},{y}) on {:?}", l.screen());
					}
				}
				let (cx, cy) = r.centre();
				assert_eq!(l.hit(cx, cy), Some(p));
			}
			// Off the dialog, on its frame and in the shadow there is nothing.
			let d = l.dialog;
			assert_eq!(l.hit(0, 0), None);
			assert_eq!(l.hit(d.x, d.y), None);
			assert_eq!(l.hit(d.x + d.w + 1, d.y + 1), None);
			assert_eq!(l.hit(d.x + d.w - 1, d.y + d.h - 1), None);
			assert_eq!(l.hit(u16::MAX, u16::MAX), None);
		}
	}

	#[test]
	fn the_cells_hit_agree_with_the_rectangles_over_the_whole_screen() {
		for l in all() {
			let s = screen_rect(&l);
			for y in 0..s.h {
				for x in 0..s.w {
					let by_rect: Vec<Part> = l
						.parts()
						.filter(|(_, r)| r.contains(x, y))
						.map(|(p, _)| p)
						.collect();
					assert!(by_rect.len() <= 1);
					assert_eq!(l.hit(x, y), by_rect.first().copied());
				}
			}
		}
	}

	#[test]
	fn new_folder_is_there_for_save_as_only() {
		for l in all() {
			assert_eq!(l.rect(Part::NewFolder).is_some(), l.kind() == Kind::SaveAs);
		}
	}

	#[test]
	fn the_columns_sit_inside_the_list_in_order() {
		for l in all() {
			let list = l.rect(Part::List).unwrap();
			let c = l.columns;
			assert!(c.icon_x >= list.x);
			assert!(c.icon_x < c.name_x);
			assert!(c.name_x + c.name_w <= c.size_x);
			assert!(c.size_x + c.size_w <= c.date_x);
			assert!(c.date_x + c.date_w <= list.x + list.w, "{:?}", l.screen());
			// A header sits above its column.
			let name = l.rect(Part::Header(SortBy::Name)).unwrap();
			let size = l.rect(Part::Header(SortBy::Size)).unwrap();
			let date = l.rect(Part::Header(SortBy::Date)).unwrap();
			assert!(name.x <= c.name_x && c.name_x + c.name_w <= name.x + name.w);
			assert!(size.x <= c.size_x && c.size_x + c.size_w <= size.x + size.w);
			assert!(date.x <= c.date_x && c.date_x + c.date_w <= date.x + date.w);
		}
	}

	#[test]
	fn labels_fit_their_rectangles_and_mnemonics_are_unique() {
		let mut seen = Vec::new();
		let labels: [(&[u8], Part); 8] = [
			(text::LOOK_IN, Part::LookInLabel),
			(text::UP, Part::Up),
			(text::NEW_FOLDER, Part::NewFolder),
			(text::NAME, Part::NameLabel),
			(text::TYPE_OPEN, Part::TypeLabel),
			(text::TYPE_SAVE, Part::TypeLabel),
			(text::OPEN, Part::Open),
			(text::CANCEL, Part::Cancel),
		];
		for l in all() {
			for (label, part) in labels {
				let Some(r) = l.rect(part) else {
					continue;
				};
				let mut out = [0u8; 32];
				let (n, _) = strip_label(label, &mut out).unwrap();
				// A button draws `< ` and ` >` around its caption in text.
				let extra =
					if matches!(part, Part::Up | Part::NewFolder | Part::Open | Part::Cancel)
						&& l.screen() != Screen::Graphics
					{
						4
					} else {
						0
					};
				assert!(
					usize::from(r.w) >= n + extra,
					"{:?} is {n} wide in {} on {:?}",
					String::from_utf8_lossy(label),
					r.w,
					l.screen()
				);
			}
		}
		for label in [
			text::LOOK_IN,
			text::UP,
			text::NEW_FOLDER,
			text::PLACES,
			text::LIST,
			text::NAME,
			text::TYPE_OPEN,
		] {
			let m = mnemonic(label).unwrap();
			assert!(!seen.contains(&m), "{} twice", m as char);
			seen.push(m);
		}
		// The two type labels share a mnemonic and are never on screen together.
		assert_eq!(mnemonic(text::TYPE_OPEN), mnemonic(text::TYPE_SAVE));
		// Titles fit in the frame with a space either side.
		for l in all() {
			assert!(title(l.kind()).len() + 4 <= usize::from(l.dialog.w));
		}
	}

	#[test]
	fn the_dialogs_focus_ring_is_made_of_these_labels() {
		assert_eq!(default_button(Kind::SaveAs), text::SAVE);
		assert_eq!(default_button(Kind::Open), text::OPEN);
		assert_eq!(type_label(Kind::SaveAs), text::TYPE_SAVE);
		assert_eq!(type_label(Kind::Picture), text::TYPE_OPEN);
	}

	#[test]
	fn parts_name_the_control_a_press_focuses() {
		let l = Layout::new(Screen::Text(25), Kind::SaveAs).unwrap();
		let ctl = |p: Part| p.control();
		assert_eq!(ctl(Part::Name), Some(NAME));
		assert_eq!(ctl(Part::NameLabel), Some(NAME));
		assert_eq!(ctl(Part::List), Some(LIST));
		assert_eq!(ctl(Part::Header(SortBy::Date)), Some(LIST));
		assert_eq!(ctl(Part::Places), Some(PLACES));
		assert_eq!(ctl(Part::Open), Some(OK));
		assert_eq!(ctl(Part::Preview), None);
		for (p, _) in l.parts() {
			if let Some(c) = p.control() {
				assert!(c <= LIST);
			}
		}
	}

	#[test]
	fn the_graphics_list_divides_into_items_of_the_painters_pitch() {
		let l = Layout::new(Screen::Graphics, Kind::Open).unwrap();
		let rows = l.rect(Part::List).unwrap().h;
		assert_eq!(l.items_that_fit(22, 16), u32::from(rows) * 22 / 16);
		assert!(l.items_that_fit(22, 16) >= 15);
		assert_eq!(l.items_that_fit(22, 0), 0);
		// On the text screen the pitch is one row.
		let t = Layout::new(Screen::Text(25), Kind::Open).unwrap();
		assert_eq!(t.items_that_fit(1, 1), 12);
		assert_eq!(t.list_row_at(t.rect(Part::List).unwrap().y + 2), Some(2));
		assert_eq!(t.list_row_at(0), None);
	}

	#[test]
	fn the_graphics_dialog_is_about_616_by_374_pixels_and_centred() {
		let l = Layout::new(Screen::Graphics, Kind::Open).unwrap();
		assert_eq!(
			(u32::from(l.dialog.w) * 8, u32::from(l.dialog.h) * 22),
			(616, 374)
		);
		let left = l.dialog.x;
		let right = GFX_COLS - (l.dialog.x + l.dialog.w);
		assert!(left.abs_diff(right) <= 1);
		let top = l.dialog.y;
		let bottom = GFX_ROWS - (l.dialog.y + l.dialog.h);
		assert!(top.abs_diff(bottom) <= 1);
	}

	#[test]
	fn glyphs_are_the_cp437_table_and_distinct_enough_to_tell_apart_without_colour() {
		let icons = [
			Icon::Parent,
			Icon::Folder,
			Icon::HardDisk,
			Icon::Floppy,
			Icon::Document,
			Icon::Rtf,
			Icon::Text,
			Icon::Picture,
			Icon::Other,
		];
		let bytes: Vec<u8> = icons.iter().map(|i| glyph(*i).ch).collect();
		assert_eq!(
			bytes,
			[0x18, 0xFE, 0x16, 0xE9, 0x14, 0x15, 0xF0, 0x0F, 0xFA]
		);
		let mut sorted = bytes.clone();
		sorted.sort();
		sorted.dedup();
		assert_eq!(sorted.len(), bytes.len());
		// Brown folder, blue document, red RTF, green picture, grey the rest, on the list's black body.
		assert_eq!(glyph(Icon::Folder).attr, 0x06);
		assert_eq!(glyph(Icon::Document).attr, 0x09);
		assert_eq!(glyph(Icon::Rtf).attr, 0x0C);
		assert_eq!(glyph(Icon::Picture).attr, 0x0A);
		assert_eq!(glyph(Icon::Other).attr, 0x08);
		assert!(bytes_all_visible());
		assert_eq!(glyph(Icon::CdRom), glyph(Icon::Floppy));
		assert_eq!(glyph(Icon::Network), glyph(Icon::HardDisk));
	}

	fn bytes_all_visible() -> bool {
		// No glyph is black on black.
		[
			Icon::Parent,
			Icon::Folder,
			Icon::HardDisk,
			Icon::Floppy,
			Icon::Document,
			Icon::Rtf,
			Icon::Text,
			Icon::Picture,
			Icon::Other,
		]
		.iter()
		.all(|i| glyph(*i).attr & 0x0F != 0)
	}

	#[test]
	fn files_get_their_icon_by_extension() {
		assert_eq!(file_icon(b"LETTER.QLD"), Icon::Document);
		assert_eq!(file_icon(b"memo.rtf"), Icon::Rtf);
		assert_eq!(file_icon(b"NOTES.TXT"), Icon::Text);
		assert_eq!(file_icon(b"PIC.BMP"), Icon::Picture);
		assert_eq!(file_icon(b"PIC.PNG"), Icon::Picture);
		assert_eq!(file_icon(b"README"), Icon::Other);
		assert_eq!(file_icon(b"QUILLDOS.EXE"), Icon::Other);
		assert_eq!(file_icon(b""), Icon::Other);
	}
}
