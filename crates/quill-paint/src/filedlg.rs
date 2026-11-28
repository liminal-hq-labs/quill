// The file dialog drawn as a Windows 3.1 window: the graphics screen's Open, Save As and Insert Picture.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `quill_dlg::filelayout` gives the dialog's parts as rectangles of 8 by 22 pixel cells, the same description the text painter uses;
//! this module turns a cell into pixels the way [`crate::dialog`] does (the window, its title bar and its border are that module's)
//! and draws each part with the toolkit's own pieces: bevelled buttons, sunken fields, the scroll bar of [`crate::scroll`], the
//! list's items at a pitch of [`GFX_ITEM_H`] pixels. What is written in the dialog (sizes, dates, the line of counts) comes from
//! `quill_app::filetext`, which the text painter shares. File types are drawn as small pictures built from rectangles until the
//! icon strip of the file dialog plan replaces them.
//!
//! The boxes that open over the dialog (a question, a notice, the name of a new folder) are not drawn here: they are the app's
//! [`quill_app::Overlay`], drawn by [`crate::dialog`] on top of what this module drew. The keyboard is the app's, and so is what a
//! pointer does: [`hit`] names the part of the dialog under a pixel from the same rectangles [`draw`] fills, as the
//! [`quill_dlg::filedlg::Point`] the dialog takes, and [`list_bar`] gives the list's scroll bar.

use quill_app::filetext::{counts, date_text, icon_of, size_text, time_text, type_name};
use quill_app::{recent, FileView, Overlay};
use quill_core::store::ReadStore;
use quill_dlg::filedlg::{
	EntryKind, FileDlg, PlaceKind, Point, SortBy, CANCEL, LIST, LOOK_IN, NAME, NEW_FOLDER, OK,
	PLACES, TYPES, UP,
};
use quill_dlg::filelayout::{
	self as layout, default_button, text, title, type_label, Icon, Layout, Part, Screen, GFX_ITEM_H,
};
use quill_dlg::fs::DriveKind;
use quill_dlg::types::Kind;

use crate::dialog::{
	button_face, edit_face, sunken, window_rect, BORDER, CELL_W, CTL_H, ROW_H, TITLE_H,
};
use crate::scroll;
use crate::ui::{bevel, outline, Ui, FACE, INK, LIGHT, SELECT, SHADOW};
use crate::{Rect, Rgb, Surface};

/// The pitch of a list item, as an `i32`.
const ITEM: i32 = GFX_ITEM_H as i32;
const YELLOW: Rgb = Rgb(255, 255, 0);
const OLIVE: Rgb = Rgb(128, 128, 0);
const RED: Rgb = Rgb(255, 0, 0);
const BLUE: Rgb = Rgb(0, 0, 255);
const GREEN: Rgb = Rgb(0, 128, 0);

/// The dialog's window as an overlay with a title and a size and no widgets, for [`crate::dialog::draw_dialog`].
pub fn window(kind: Kind, off: (i16, i16)) -> Overlay {
	let mut o = Overlay {
		off,
		..Overlay::default()
	};
	let (w, h) = Layout::new(Screen::Graphics, kind).map_or((0, 0), |l| (l.dialog.w, l.dialog.h));
	let t = title(kind);
	for (d, s) in o.title.iter_mut().zip(t) {
		*d = *s;
	}
	o.title_len = t.len().min(o.title.len()) as u8;
	o.w = w as u8;
	o.h = h as u8;
	o
}

/// Where cells are on the screen: the dialog's own corner and the pixel of its first row.
#[derive(Copy, Clone)]
struct Geo {
	left: i32,
	top: i32,
	cell_x: u16,
	cell_y: u16,
}

impl Geo {
	/// The pixels of a rectangle of cells, as tall as its rows.
	#[inline(never)]
	fn cells(&self, r: layout::Rect) -> Rect {
		let x = self.left + i32::from(r.x - self.cell_x) * CELL_W;
		let y = self.top + (i32::from(r.y - self.cell_y) - 1) * ROW_H;
		Rect::new(
			x,
			y,
			x + i32::from(r.w) * CELL_W,
			y + i32::from(r.h) * ROW_H,
		)
	}

	/// The pixels of a control of one row: [`CTL_H`] tall.
	#[inline(never)]
	fn control(&self, r: layout::Rect) -> Rect {
		let c = self.cells(r);
		Rect::new(c.left, c.top, c.right, c.top + CTL_H)
	}

	/// The pixel a column of cells starts at.
	fn column(&self, x: u16) -> i32 {
		self.left + i32::from(x - self.cell_x) * CELL_W
	}
}

/// Where the dialog's cells are when its window is `win`.
fn geo_of(win: &Overlay, l: &Layout) -> Geo {
	let w = window_rect(win);
	Geo {
		left: w.left + BORDER,
		top: w.top + BORDER + TITLE_H + 8,
		cell_x: l.dialog.x,
		cell_y: l.dialog.y,
	}
}

#[inline(never)]
fn rect_of(l: &Layout, p: Part) -> layout::Rect {
	l.rect(p).unwrap_or_default()
}

/// Draws a label of one line at the left of `r`, centred in its height.
#[inline(never)]
fn label<Sf: Surface + ?Sized>(sf: &mut Sf, ui: &Ui<Sf::Font>, r: Rect, t: &[u8]) {
	ui.label(sf, r.left, r.top + (r.height() - ui.height) / 2, t, INK);
}

/// Draws the dialog's parts inside its window `win` (drawn already by [`crate::dialog::draw_dialog`]).
#[inline(never)]
pub fn draw<Sf: Surface + ?Sized, S: ReadStore + ?Sized>(
	sf: &mut Sf,
	ui: &Ui<Sf::Font>,
	win: &Overlay,
	v: &FileView<'_>,
	s: &S,
	(held, bar): (Option<usize>, Option<scroll::Part>),
) {
	let dlg = v.dlg;
	let Some(l) = Layout::new(Screen::Graphics, dlg.kind()) else {
		return;
	};
	let geo = geo_of(win, &l);
	let held = held.map_or(255, |h| h.min(255) as u8);
	// While a box is open over the dialog the dialog shows no focus.
	let focus = if v.covered() {
		255
	} else {
		dlg.focus().focused()
	};
	look_in(sf, ui, &geo, &l, dlg, (focus, held));
	places(sf, ui, &geo, &l, dlg, s, focus);
	list(sf, ui, &geo, &l, v, s, (focus, bar));
	preview(sf, ui, &geo, &l, v, s);
	bottom(sf, ui, &geo, &l, dlg, (focus, held));
	drops(sf, ui, &geo, &l, dlg);
	sf.clip(None);
}

/// One item of a list, in the row `r`: the navy (or grey) bar when it is the choice, its picture, and its text.
#[inline(never)]
fn item<Sf: Surface + ?Sized>(
	sf: &mut Sf,
	ui: &Ui<Sf::Font>,
	r: Rect,
	bar: Option<Rgb>,
	(icon, icon_x, text_x): (Option<Icon>, i32, i32),
	t: &[u8],
) {
	if let Some(c) = bar {
		sf.fill_rect(r, c);
	}
	if let Some(i) = icon {
		picture(sf, icon_x, r.top, i);
	}
	let ink = if bar.is_some() { LIGHT } else { INK };
	ui.text(sf, text_x, r.top + (ITEM - ui.height) / 2, t, ink);
}

/// A drop-down field: the text in a sunken box, on the navy bar when it has the keyboard, with an arrow button at its right.
#[inline(never)]
fn combo<Sf: Surface + ?Sized>(
	sf: &mut Sf,
	ui: &Ui<Sf::Font>,
	field: Rect,
	arrow: Rect,
	t: &[u8],
	(focused, open): (bool, bool),
) {
	edit_face(sf, ui, field, t, (0, 0), false);
	if focused {
		let inner = Rect::new(
			field.left + 3,
			field.top + 3,
			field.right - 3,
			field.bottom - 3,
		);
		let (x, y) = (field.left + 5, field.top + (field.height() - ui.height) / 2);
		sf.fill_rect(inner, SELECT);
		sf.clip(Some(inner));
		ui.text(sf, x, y, t, LIGHT);
		sf.clip(None);
	}
	scroll::arrow(sf, arrow, 1, (true, open));
}

#[inline(never)]
fn look_in<Sf: Surface + ?Sized>(
	sf: &mut Sf,
	ui: &Ui<Sf::Font>,
	geo: &Geo,
	l: &Layout,
	dlg: &FileDlg,
	(focus, held): (u8, u8),
) {
	label(
		sf,
		ui,
		geo.control(rect_of(l, Part::LookInLabel)),
		text::LOOK_IN,
	);
	let field = geo.control(rect_of(l, Part::LookIn));
	let drop = geo.control(rect_of(l, Part::LookInDrop));
	let mut shown = [0u8; 80];
	let n = recent::shorten_path(dlg.folder(), 38, &mut shown);
	combo(
		sf,
		ui,
		field,
		drop,
		shown.get(..n).unwrap_or(&[]),
		(focus == LOOK_IN, dlg.look_box().is_open()),
	);
	if let Some(up) = l.rect(Part::Up) {
		let r = geo.control(up);
		push(
			sf,
			ui,
			r,
			text::UP,
			(false, focus == UP, held == UP),
			dlg.folder().len() > 3,
		);
	}
	if let Some(nf) = l.rect(Part::NewFolder) {
		let r = geo.control(nf);
		push(
			sf,
			ui,
			r,
			text::NEW_FOLDER,
			(false, focus == NEW_FOLDER, held == NEW_FOLDER),
			true,
		);
	}
}

/// A push button; greyed out, its label is the shadow colour.
#[inline(never)]
fn push<Sf: Surface + ?Sized>(
	sf: &mut Sf,
	ui: &Ui<Sf::Font>,
	r: Rect,
	t: &[u8],
	(default, focus, down): (bool, bool, bool),
	enabled: bool,
) {
	if enabled {
		button_face(sf, ui, r, t, (default, focus), down);
	} else {
		button_face(sf, ui, r, b"", (false, false), false);
		let x = r.left + (r.width() - ui.label_width(sf, t)) / 2;
		ui.label(sf, x, r.top + (r.height() - ui.height) / 2, t, SHADOW);
	}
}

/// A pane's header: a flat raised bar with a label.
#[inline(never)]
fn header<Sf: Surface + ?Sized>(sf: &mut Sf, ui: &Ui<Sf::Font>, r: Rect, t: &[u8]) {
	sf.fill_rect(r, FACE);
	bevel(sf, r, true);
	ui.label(sf, r.left + 4, r.top + (r.height() - ui.height) / 2, t, INK);
}

#[inline(never)]
fn places<Sf: Surface + ?Sized, S: ReadStore + ?Sized>(
	sf: &mut Sf,
	ui: &Ui<Sf::Font>,
	geo: &Geo,
	l: &Layout,
	dlg: &FileDlg,
	s: &S,
	focus: u8,
) {
	let pane = geo.cells(l.places_pane);
	header(
		sf,
		ui,
		Rect::new(pane.left, pane.top, pane.right, pane.top + CTL_H),
		text::PLACES,
	);
	let body = geo.cells(rect_of(l, Part::Places));
	sunken(sf, body);
	let inner = Rect::new(body.left + 2, body.top + 2, body.right - 2, body.bottom - 2);
	sf.clip(Some(inner));
	let list = dlg.places();
	for row in 0..list.rows() {
		let idx = list.top() + row;
		let Some(p) = dlg.place(s, idx) else {
			break;
		};
		let icon = match p.kind {
			PlaceKind::Drive(_, DriveKind::Fixed) => Icon::HardDisk,
			PlaceKind::Drive(_, DriveKind::Removable) => Icon::Floppy,
			PlaceKind::Drive(_, DriveKind::CdRom) => Icon::CdRom,
			PlaceKind::Drive(_, DriveKind::Network) => Icon::Network,
			PlaceKind::Folder => Icon::Folder,
			PlaceKind::Recent => layout::file_icon(p.label()),
		};
		// The bar shows only while the places have the keyboard: they are places to go, not choices.
		let y = inner.top + row as i32 * ITEM;
		let bar = (focus == PLACES && list.selected() == Some(idx)).then_some(SELECT);
		item(
			sf,
			ui,
			Rect::new(inner.left + 20, y, inner.right, y + ITEM),
			bar,
			(Some(icon), inner.left + 2, inner.left + 22),
			p.label(),
		);
	}
	sf.clip(None);
}

#[inline(never)]
fn list<Sf: Surface + ?Sized, S: ReadStore + ?Sized>(
	sf: &mut Sf,
	ui: &Ui<Sf::Font>,
	geo: &Geo,
	l: &Layout,
	v: &FileView<'_>,
	s: &S,
	(focus, bar): (u8, Option<scroll::Part>),
) {
	let dlg = v.dlg;
	let sort = dlg.sort();
	for (by, name) in [
		(SortBy::Name, text::HEADER_NAME),
		(SortBy::Size, text::HEADER_SIZE),
		(SortBy::Date, text::HEADER_DATE),
	] {
		let r = geo.control(rect_of(l, Part::Header(by)));
		header(sf, ui, r, name);
		if sort.by == by {
			// The column the list is sorted by has a small triangle: up for ascending.
			let at = (r.right - 12, r.top + CTL_H / 2);
			scroll::triangle(sf, at, u8::from(sort.descending), INK);
		}
	}
	let body = geo.cells(rect_of(l, Part::List));
	sunken(sf, body);
	let inner = Rect::new(body.left + 2, body.top + 2, body.right - 2, body.bottom - 2);
	let c = l.columns;
	let (icon_x, name_x, size_end, date_x) = (
		geo.column(c.icon_x),
		geo.column(c.name_x),
		geo.column(c.size_x + c.size_w) - 6,
		geo.column(c.date_x),
	);
	let lst = dlg.list();
	for row in 0..lst.rows() {
		let idx = lst.top() + row;
		let Some(e) = dlg.entry(s, idx) else {
			break;
		};
		let y = inner.top + row as i32 * ITEM;
		let line = Rect::new(inner.left, y, inner.right, y + ITEM);
		let bar =
			(lst.selected() == Some(idx)).then_some(if focus == LIST { SELECT } else { SHADOW });
		sf.clip(Some(line));
		let ink = if bar.is_some() { LIGHT } else { INK };
		let ty = y + (ITEM - ui.height) / 2;
		item(sf, ui, line, bar, (Some(icon_of(&e)), icon_x, name_x), &[]);
		// The name has its column; a longer one is cut at it.
		sf.clip(Some(Rect::new(name_x, y, size_end - 40, y + ITEM)));
		ui.text(sf, name_x, ty, e.name(), ink);
		sf.clip(Some(line));
		if e.kind() == EntryKind::File {
			let mut sz = [0u8; 12];
			let n = size_text(e.size(), &mut sz);
			let t = sz.get(..n).unwrap_or(&[]);
			let tw = ui.width(sf, t);
			ui.text(sf, size_end - tw, ty, t, ink);
		}
		if e.kind() != EntryKind::Up && e.date() != 0 {
			let mut out = [0u8; 16];
			let n = date_text(v.date, e.date(), false, &mut out);
			ui.text(sf, date_x, ty, out.get(..n).unwrap_or(&[]), ink);
		}
	}
	sf.clip(None);
	scroll::draw(sf, &list_parts(geo, l, dlg), lst.overflows(), bar);
}

fn list_parts(geo: &Geo, l: &Layout, dlg: &FileDlg) -> scroll::Parts {
	let lst = dlg.list();
	scroll::parts(
		geo.cells(rect_of(l, Part::ListScroll)),
		true,
		lst.top() as i32,
		lst.count() as i32,
		lst.rows() as i32,
	)
}

#[inline(never)]
fn preview<Sf: Surface + ?Sized, S: ReadStore + ?Sized>(
	sf: &mut Sf,
	ui: &Ui<Sf::Font>,
	geo: &Geo,
	l: &Layout,
	v: &FileView<'_>,
	s: &S,
) {
	let dlg = v.dlg;
	let pane = geo.cells(l.preview_pane);
	header(
		sf,
		ui,
		Rect::new(pane.left, pane.top, pane.right, pane.top + CTL_H),
		text::PREVIEW_TITLE,
	);
	let body = Rect::new(pane.left, pane.top + ROW_H, pane.right, pane.bottom);
	sunken(sf, body);
	let Some(e) = dlg.list().selected().and_then(|i| dlg.entry(s, i)) else {
		return;
	};
	if e.kind() == EntryKind::Up {
		return;
	}
	let inner = Rect::new(body.left + 2, body.top + 2, body.right - 2, body.bottom - 2);
	sf.clip(Some(inner));
	// The type's picture in a frame, with the name and the kind of file beside it.
	let (x, y) = (inner.left + 8, inner.top + 8);
	let frame = Rect::new(x, y, x + 32, y + 32);
	sf.fill_rect(frame, FACE);
	bevel(sf, frame, false);
	picture(sf, x + 8, y + 8, icon_of(&e));
	let tx = x + 40;
	ui.text(sf, tx, y + 2, e.name(), INK);
	ui.text(sf, tx, y + 2 + ROW_H - 4, type_name(&e), INK);
	sf.hline(inner.left + 8, inner.right - 8, y + 44, SHADOW);
	let mut line = y + 52;
	if e.kind() == EntryKind::File {
		let mut sz = [0u8; 12];
		let n = size_text(e.size(), &mut sz);
		ui.text(sf, x, line, sz.get(..n).unwrap_or(&[]), INK);
		line += ROW_H - 4;
	}
	if e.date() != 0 {
		let mut out = [0u8; 16];
		let n = date_text(v.date, e.date(), false, &mut out);
		ui.text(sf, x, line, out.get(..n).unwrap_or(&[]), INK);
		let mut t = [0u8; 16];
		let m = time_text(v.h24, e.time(), &mut t);
		ui.text(sf, x, line + ROW_H - 4, t.get(..m).unwrap_or(&[]), INK);
	}
	sf.clip(None);
}

#[inline(never)]
fn bottom<Sf: Surface + ?Sized>(
	sf: &mut Sf,
	ui: &Ui<Sf::Font>,
	geo: &Geo,
	l: &Layout,
	dlg: &FileDlg,
	(focus, held): (u8, u8),
) {
	let kind = dlg.kind();
	label(sf, ui, geo.control(rect_of(l, Part::NameLabel)), text::NAME);
	let name = geo.control(rect_of(l, Part::Name));
	let f = dlg.name();
	let (lo, hi) = f.selection();
	let anchor = if f.cursor() == hi { lo } else { hi };
	edit_face(
		sf,
		ui,
		name,
		f.as_bytes(),
		(f.cursor(), anchor),
		focus == NAME,
	);
	label(
		sf,
		ui,
		geo.control(rect_of(l, Part::TypeLabel)),
		type_label(kind),
	);
	let tb = geo.control(rect_of(l, Part::TypeBox));
	let td = geo.control(rect_of(l, Part::TypeDrop));
	let arrow = Rect::new(td.right - 2 * CELL_W, td.top, td.right, td.bottom);
	let field = Rect::new(tb.left, tb.top, arrow.left, tb.bottom);
	let kinds = dlg.types();
	let t = kinds
		.get(dlg.type_index())
		.map_or(&b""[..], |t| t.label.as_bytes());
	combo(
		sf,
		ui,
		field,
		arrow,
		t,
		(focus == TYPES, dlg.type_box().is_open()),
	);
	// The line of counts, or the cap's hint.
	let st = geo.control(rect_of(l, Part::Status));
	let ty = st.top + (st.height() - ui.height) / 2;
	if dlg.more() {
		ui.text(sf, st.left, ty, text::MORE, INK);
	} else {
		let mut c = [0u8; 40];
		let n = counts(dlg, &mut c);
		ui.text(sf, st.left, ty, c.get(..n).unwrap_or(&[]), INK);
	}
	button_face(
		sf,
		ui,
		geo.control(rect_of(l, Part::Open)),
		default_button(kind),
		(focus != CANCEL, focus == OK),
		held == OK,
	);
	button_face(
		sf,
		ui,
		geo.control(rect_of(l, Part::Cancel)),
		text::CANCEL,
		(false, focus == CANCEL),
		held == CANCEL,
	);
}

/// A drop-down that is open: a white list with a black edge and the navy bar on its choice.
#[inline(never)]
fn pop<Sf: Surface + ?Sized>(sf: &mut Sf, r: Rect) {
	sf.fill_rect(r, LIGHT);
	outline(sf, r, INK);
}

/// The list of the drop-down `which` (`TYPES` or `LOOK_IN`) when it is open: above its field for the types, below it for Look in.
fn drop_rect(geo: &Geo, l: &Layout, dlg: &FileDlg, which: u8) -> Rect {
	if which == TYPES {
		let tb = geo.control(rect_of(l, Part::TypeBox));
		let td = geo.control(rect_of(l, Part::TypeDrop));
		let n = dlg.type_box().count().min(6) as i32;
		Rect::new(tb.left, tb.top - (n * ITEM + 2), td.right, tb.top)
	} else {
		let f = geo.control(rect_of(l, Part::LookIn));
		let d = geo.control(rect_of(l, Part::LookInDrop));
		let n = dlg.look_box().count().min(6) as i32;
		Rect::new(f.left, f.bottom, d.right, f.bottom + n * ITEM + 2)
	}
}

#[inline(never)]
fn drops<Sf: Surface + ?Sized>(
	sf: &mut Sf,
	ui: &Ui<Sf::Font>,
	geo: &Geo,
	l: &Layout,
	dlg: &FileDlg,
) {
	if dlg.type_box().is_open() {
		let r = drop_rect(geo, l, dlg, TYPES);
		let n = dlg.type_box().count().min(6) as i32;
		pop(sf, r);
		let lst = dlg.type_box().list();
		for i in 0..n {
			let idx = lst.top() + i as usize;
			let Some(t) = dlg.types().get(idx) else {
				break;
			};
			let y = r.top + 1 + i * ITEM;
			let bar = (lst.selected() == Some(idx)).then_some(SELECT);
			let row = Rect::new(r.left + 1, y, r.right - 1, y + ITEM);
			item(sf, ui, row, bar, (None, 0, r.left + 4), t.label.as_bytes());
		}
	}
	if dlg.look_box().is_open() {
		let r = drop_rect(geo, l, dlg, LOOK_IN);
		let n = dlg.look_box().count().min(6) as i32;
		pop(sf, r);
		let lst = dlg.look_box().list();
		for i in 0..n {
			let idx = lst.top() + i as usize;
			let Some(it) = dlg.look_item(idx) else {
				break;
			};
			let y = r.top + 1 + i * ITEM;
			let bar = (lst.selected() == Some(idx)).then_some(SELECT);
			let row = Rect::new(r.left + 1, y, r.right - 1, y + ITEM);
			// A drive, and the folders below it indented, each with its own picture.
			let x = r.left + 2 + 10 * i32::from(it.indent.saturating_sub(1));
			let letter = [it.letter, b':'];
			let (icon, t) = if it.is_drive() {
				(Icon::HardDisk, letter.as_slice())
			} else {
				(Icon::Folder, it.label())
			};
			item(sf, ui, row, bar, (Some(icon), x, x + 20), t);
		}
	}
}

/// A file type's small picture, 16 pixels square at (`x`, `y`), from rectangles: a folder (with an arrow for the one above), a drive
/// (a floppy is navy), or a page whose lines are in the type's colour.
#[inline(never)]
fn picture<Sf: Surface + ?Sized>(sf: &mut Sf, x: i32, y: i32, icon: Icon) {
	let rect = |sf: &mut Sf, l: i32, t: i32, r: i32, b: i32, c: Rgb| {
		sf.fill_rect(Rect::new(x + l, y + t, x + r, y + b), c);
	};
	match icon {
		Icon::Folder | Icon::Parent => {
			rect(sf, 1, 3, 7, 14, OLIVE);
			rect(sf, 1, 5, 15, 14, OLIVE);
			rect(sf, 2, 6, 14, 13, YELLOW);
			if icon == Icon::Parent {
				rect(sf, 7, 7, 9, 12, INK);
				rect(sf, 6, 8, 11, 9, INK);
			}
		}
		Icon::HardDisk | Icon::Network | Icon::Floppy | Icon::CdRom => {
			let body = if icon == Icon::Floppy {
				Rgb::NAVY
			} else {
				FACE
			};
			rect(sf, 1, 5, 15, 12, INK);
			rect(sf, 2, 6, 14, 11, body);
		}
		_ => {
			rect(sf, 3, 1, 13, 15, INK);
			rect(sf, 4, 2, 12, 14, Rgb::WHITE);
			let ink = match icon {
				Icon::Document => BLUE,
				Icon::Rtf => RED,
				Icon::Picture => GREEN,
				_ => SHADOW,
			};
			for i in 0..4 {
				rect(sf, 5, 4 + 2 * i, 11, 5 + 2 * i, ink);
			}
		}
	}
}

/// The part of the open dialog `dlg` (in the window `win`) under pixel (`x`, `y`), as the press it is: an entry or place by its
/// index, a header, a button, a drop-down's field or row, the name field, the scroll bar. Everything else is `Away`. While a
/// drop-down is open its list comes first, as it is drawn over the dialog.
#[inline(never)]
pub fn hit(win: &Overlay, dlg: &FileDlg, x: i32, y: i32) -> Point {
	let Some(l) = Layout::new(Screen::Graphics, dlg.kind()) else {
		return Point::Away;
	};
	let geo = geo_of(win, &l);
	let inside = |r: Rect| x >= r.left && x < r.right && y >= r.top && y < r.bottom;
	let row_in = |top: i32| ((y - top).max(0) / ITEM) as usize;
	for (which, combo) in [(TYPES, dlg.type_box()), (LOOK_IN, dlg.look_box())] {
		let r = drop_rect(&geo, &l, dlg, which);
		if combo.is_open() && inside(r) {
			let i = combo.list().top() + row_in(r.top + 1);
			return if i < combo.count() && y > r.top {
				Point::Pick(i)
			} else {
				Point::Away
			};
		}
	}
	for (part, cells) in l.parts() {
		let px = match part {
			Part::Places | Part::List | Part::ListScroll => geo.cells(cells),
			_ => geo.control(cells),
		};
		if !inside(px) {
			continue;
		}
		let (lst, from) = match part {
			Part::Places => (dlg.places(), px.top + 2),
			Part::List => (dlg.list(), px.top + 2),
			_ => (dlg.list(), 0),
		};
		let i = lst.top() + row_in(from);
		return match part {
			Part::LookIn | Part::LookInDrop => Point::Drop(LOOK_IN),
			Part::TypeBox | Part::TypeDrop => Point::Drop(TYPES),
			Part::Up => Point::Button(UP),
			Part::NewFolder => Point::Button(NEW_FOLDER),
			Part::Open => Point::Button(OK),
			Part::Cancel => Point::Button(CANCEL),
			Part::Header(by) => Point::Header(by),
			Part::Name => Point::Name(0),
			Part::ListScroll => Point::Bar,
			Part::Places if y >= from && i < lst.count() && i - lst.top() < lst.rows() => {
				Point::Place(i)
			}
			Part::List if y >= from && i < lst.count() && i - lst.top() < lst.rows() => {
				Point::Item(i)
			}
			_ => Point::Away,
		};
	}
	Point::Away
}

/// The list's scroll bar as it is drawn.
pub fn list_bar(win: &Overlay, dlg: &FileDlg) -> Option<scroll::Parts> {
	let l = Layout::new(Screen::Graphics, dlg.kind())?;
	Some(list_parts(&geo_of(win, &l), &l, dlg))
}

/// The file name field's rectangle, for a press to find the character under it.
pub fn name_rect(win: &Overlay, dlg: &FileDlg) -> Rect {
	Layout::new(Screen::Graphics, dlg.kind()).map_or(Rect::new(0, 0, 0, 0), |l| {
		geo_of(win, &l).control(rect_of(&l, Part::Name))
	})
}
