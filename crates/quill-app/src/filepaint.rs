// The text painter of the file dialog: `filelayout`'s rectangles drawn into the 80-column grid with the dialogs' attribute roles.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// It reads the dialog (`FileDlg`) and the store and draws nothing it was not told: the frame and shadow, the Look in field and its
// buttons, the three panes (places, the list with its header and scroll bar, the preview), the name and type fields, the status
// line and the buttons, then the drop-down that is open, the New folder box and the message box over everything. File names are
// DOS bytes in code page 437 and are written as they are. The attribute roles are the dialogs plan's: frame and labels 0x70, field
// and list body 0x07, the selection 0x1F with focus and 0x70 without (0x70 and 0x0F in monochrome), a focused button 0x07, the
// default button's brackets 0x7F.

use quill_core::layout::Measure;
use quill_core::store::ReadStore;
use quill_dlg::filedlg::{
	Entry, EntryKind, FileDlg, PlaceKind, SortBy, CANCEL, LIST, LOOK_IN, NAME, NEW_FOLDER, OK,
	PLACES, TYPES, UP,
};
use quill_dlg::filelayout::{
	default_button, file_icon, glyph, text, title, type_label, Icon, Layout, Part, Rect, Screen,
};
use quill_dlg::fs::DriveKind;
use quill_dlg::widgets::thumb;
use quill_grid::Grid;

use super::{put_label, shadow, Cursor, HOT};
use crate::filedialog::{FileBox, Line};
use crate::filetext::{counts, date_text, field_window, icon_of, size_text, time_text, type_name};
use crate::{recent, App, Scheme};

const CHROME: u8 = 0x70;
const BODY: u8 = 0x07;
const DEFAULT_BRACKET: u8 = 0x7F;
const SHADOW_GAP: usize = 2;

/// The attributes the scheme gives each role.
#[derive(Copy, Clone)]
struct Roles {
	/// The selected entry of a list that has the focus, and of one that has not
	sel: u8,
	sel_idle: u8,
	/// What a file type's glyph is drawn in on the body
	mono: bool,
}

impl Roles {
	fn of(s: Scheme) -> Roles {
		let mono = s == Scheme::Mono;
		Roles {
			sel: if mono { 0x70 } else { 0x1F },
			sel_idle: if mono { 0x0F } else { 0x70 },
			mono,
		}
	}
}

fn rect<const R: usize>(l: &Layout, p: Part) -> Rect {
	let _ = R;
	l.rect(p).unwrap_or_default()
}

fn str_of(b: &[u8]) -> &str {
	core::str::from_utf8(b).unwrap_or("")
}

/// Writes at most `max` bytes of `t` from (x, y).
fn put<const R: usize>(g: &mut Grid<R>, x: u16, y: u16, t: &[u8], a: u8, max: usize) {
	g.text(
		usize::from(x),
		usize::from(y),
		t.get(..t.len().min(max)).unwrap_or(&[]),
		a,
	);
}

fn fill<const R: usize>(g: &mut Grid<R>, r: Rect, a: u8) {
	g.fill(
		usize::from(r.x),
		usize::from(r.y),
		usize::from(r.w),
		usize::from(r.h),
		b' ',
		a,
	);
}

/// Writes `n` right-aligned so it ends at column `end` (inclusive).
fn put_right<const R: usize>(g: &mut Grid<R>, end: u16, y: u16, t: &[u8], a: u8) {
	let start = (end + 1).saturating_sub(t.len() as u16);
	put(g, start, y, t, a, t.len());
}

/// A button's text and its rectangle's width: `<` and `>` round the label, centred, with the mnemonic letter in its own colour.
fn button<const R: usize>(
	g: &mut Grid<R>,
	r: Rect,
	label: &[u8],
	focused: bool,
	default: bool,
	enabled: bool,
) {
	let mut shown = [0u8; 24];
	let (len, _) = quill_dlg::focus::strip_label(label, &mut shown).unwrap_or((0, None));
	let inner = usize::from(r.w).saturating_sub(2);
	let pad_l = inner.saturating_sub(len) / 2;
	let a = if !enabled {
		0x78
	} else if focused {
		BODY
	} else {
		CHROME
	};
	let bracket = if focused {
		BODY
	} else if default {
		DEFAULT_BRACKET
	} else {
		a
	};
	let (x, y) = (usize::from(r.x), usize::from(r.y));
	g.fill(x, y, usize::from(r.w), 1, b' ', a);
	g.put(x, y, b'<', bracket);
	g.put(x + usize::from(r.w) - 1, y, b'>', bracket);
	// The label is written with its `&` marks at its place, so the mnemonic letter shows.
	let hot = if focused { HOT_FOCUSED } else { HOT };
	put_label(g, x + 1 + pad_l, y, str_of(label), a, hot);
}

/// The mnemonic's attribute on a focused (black) button.
const HOT_FOCUSED: u8 = 0x0E;

pub(super) fn draw<M: Measure + Clone, S: ReadStore + ?Sized, const ROWS: usize>(
	app: &App<M>,
	s: &S,
	g: &mut Grid<ROWS>,
	rows: usize,
) -> Option<Cursor> {
	let dlg = app.fd.dlg.as_ref()?;
	let kind = dlg.kind();
	let l = Layout::new(Screen::Text(rows as u16), kind)?;
	let roles = Roles::of(app.scheme);
	let d = l.dialog;
	let focus = dlg.focus().focused();
	let boxed = app.fd.boxed;
	let quiet = boxed != FileBox::None || dlg.new_folder().is_some();

	// The frame, the title and the shadow.
	fill(g, d, CHROME);
	g.frame(
		usize::from(d.x),
		usize::from(d.y),
		usize::from(d.w),
		usize::from(d.h),
		CHROME,
	);
	let t = title(kind);
	let tx = usize::from(d.x) + (usize::from(d.w) - t.len() - 2) / 2;
	g.text(tx, usize::from(d.y), b" ", CHROME);
	g.text(tx + 1, usize::from(d.y), t, CHROME);
	g.text(tx + 1 + t.len(), usize::from(d.y), b" ", CHROME);
	shadow(
		g,
		usize::from(d.x),
		usize::from(d.y),
		usize::from(d.w),
		usize::from(d.h),
	);

	look_in(g, &l, dlg, &roles, focus);
	places(g, &l, dlg, s, &roles, focus);
	list(g, &l, dlg, s, app, &roles, focus);
	preview(g, &l, dlg, s, app);
	bottom(g, &l, dlg, s, &roles, focus);
	let mut cursor = None;
	if !quiet && focus == NAME {
		let name = rect::<ROWS>(&l, Part::Name);
		let (start, _) = field_window(dlg.name().len(), dlg.name().cursor(), usize::from(name.w));
		cursor = Some((
			usize::from(name.x) + dlg.name().cursor() - start,
			usize::from(name.y),
		));
	}
	drops(g, &l, dlg, &roles);
	if let Some(f) = dlg.new_folder() {
		cursor = new_folder_box(g, &l, f.as_bytes(), f.cursor());
	}
	match boxed {
		FileBox::None => {}
		FileBox::Ask(focus, l1, l2) => {
			message_box(
				g,
				&l,
				l1,
				l2,
				&[(b"&Yes", focus == 0), (b"&No", focus == 1)],
			);
			cursor = None;
		}
		FileBox::Say(retry, focus, l1, l2) => {
			if retry.is_some() {
				message_box(
					g,
					&l,
					l1,
					l2,
					&[(b"&Retry", focus == 0), (b"&Cancel", focus == 1)],
				);
			} else {
				message_box(g, &l, l1, l2, &[(b"OK", true)]);
			}
			cursor = None;
		}
	}
	cursor
}

fn look_in<const R: usize>(g: &mut Grid<R>, l: &Layout, dlg: &FileDlg, roles: &Roles, focus: u8) {
	let label = rect::<R>(l, Part::LookInLabel);
	put_label(
		g,
		usize::from(label.x),
		usize::from(label.y),
		str_of(text::LOOK_IN),
		CHROME,
		HOT,
	);
	let field = rect::<R>(l, Part::LookIn);
	let drop = rect::<R>(l, Part::LookInDrop);
	let a = if focus == LOOK_IN { roles.sel } else { BODY };
	fill(g, field, a);
	fill(g, drop, a);
	let mut shown = [0u8; 80];
	let n = recent::shorten_path(
		dlg.folder(),
		usize::from(field.w).saturating_sub(1),
		&mut shown,
	);
	put(
		g,
		field.x,
		field.y,
		shown.get(..n).unwrap_or(&[]),
		a,
		usize::from(field.w),
	);
	g.put(
		usize::from(drop.x),
		usize::from(drop.y),
		0x1F,
		if focus == LOOK_IN { roles.sel } else { CHROME },
	);
	let enabled_up = dlg.folder().len() > 3;
	if let Some(up) = l.rect(Part::Up) {
		button(g, up, text::UP, focus == UP, false, enabled_up);
	}
	if let Some(nf) = l.rect(Part::NewFolder) {
		button(g, nf, text::NEW_FOLDER, focus == NEW_FOLDER, false, true);
	}
}

fn glyph_attr(roles: &Roles, icon: Icon) -> u8 {
	if roles.mono {
		BODY
	} else {
		glyph(icon).attr
	}
}

fn places<const R: usize, S: ReadStore + ?Sized>(
	g: &mut Grid<R>,
	l: &Layout,
	dlg: &FileDlg,
	s: &S,
	roles: &Roles,
	focus: u8,
) {
	let pane = l.places_pane;
	g.frame(
		usize::from(pane.x),
		usize::from(pane.y),
		usize::from(pane.w),
		usize::from(pane.h),
		CHROME,
	);
	put_label(
		g,
		usize::from(pane.x) + 2,
		usize::from(pane.y),
		" &Places ",
		CHROME,
		HOT,
	);
	let body = rect::<R>(l, Part::Places);
	fill(g, body, BODY);
	let list = dlg.places();
	for row in 0..usize::from(body.h) {
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
			PlaceKind::Recent => file_icon(p.label()),
		};
		// The bar shows only while the places have the keyboard: they are a list of places to go, not of choices.
		let on = focus == PLACES && list.selected() == Some(idx);
		let y = body.y + row as u16;
		let bar = if on { roles.sel } else { BODY };
		g.fill(
			usize::from(body.x),
			usize::from(y),
			usize::from(body.w),
			1,
			b' ',
			bar,
		);
		let gl = glyph(icon);
		g.put(
			usize::from(body.x),
			usize::from(y),
			gl.ch,
			if on { bar } else { glyph_attr(roles, icon) },
		);
		put(
			g,
			body.x + 2,
			y,
			p.label(),
			bar,
			usize::from(body.w).saturating_sub(2),
		);
	}
}

fn list<const R: usize, M: Measure + Clone, S: ReadStore + ?Sized>(
	g: &mut Grid<R>,
	l: &Layout,
	dlg: &FileDlg,
	s: &S,
	app: &App<M>,
	roles: &Roles,
	focus: u8,
) {
	let pane = l.list_pane;
	g.frame(
		usize::from(pane.x),
		usize::from(pane.y),
		usize::from(pane.w),
		usize::from(pane.h),
		CHROME,
	);
	// The header sits on the frame's top edge: `Name`, `Size` and `Modified`, with an arrow on the column that sorts.
	let c = l.columns;
	let sort = dlg.sort();
	let arrow = if sort.descending { 0x1F } else { 0x1E };
	let y = pane.y;
	put(g, c.name_x, y, text::HEADER_NAME, CHROME, 4);
	put_right(g, c.size_x + c.size_w - 1, y, text::HEADER_SIZE, CHROME);
	put(g, c.date_x, y, text::HEADER_DATE, CHROME, 8);
	let (ax, aw) = match sort.by {
		SortBy::Name => (c.name_x + 4, 1),
		SortBy::Size => (c.size_x + c.size_w, 1),
		SortBy::Date => (c.date_x + 8, 1),
	};
	if aw == 1 {
		g.put(usize::from(ax), usize::from(y), arrow, CHROME);
	}
	let body = rect::<R>(l, Part::List);
	fill(g, body, BODY);
	let lst = dlg.list();
	for row in 0..usize::from(body.h) {
		let idx = lst.top() + row;
		let Some(e) = dlg.entry(s, idx) else {
			break;
		};
		let y = body.y + row as u16;
		let on = lst.selected() == Some(idx);
		let bar = if !on {
			BODY
		} else if focus == LIST {
			roles.sel
		} else {
			roles.sel_idle
		};
		g.fill(
			usize::from(body.x),
			usize::from(y),
			usize::from(body.w),
			1,
			b' ',
			bar,
		);
		let icon = icon_of(&e);
		g.put(
			usize::from(c.icon_x),
			usize::from(y),
			glyph(icon).ch,
			if on { bar } else { glyph_attr(roles, icon) },
		);
		put(g, c.name_x, y, e.name(), bar, usize::from(c.name_w));
		match e.kind() {
			EntryKind::Up => {}
			EntryKind::Folder => {
				put_right(g, c.size_x + c.size_w - 1, y, text::DIR_MARK, bar);
				stamp(g, app, &e, c.date_x, y, bar);
			}
			EntryKind::File => {
				let mut sz = [0u8; 12];
				let n = size_text(e.size(), &mut sz);
				put_right(
					g,
					c.size_x + c.size_w - 1,
					y,
					sz.get(..n).unwrap_or(&[]),
					bar,
				);
				stamp(g, app, &e, c.date_x, y, bar);
			}
		}
	}
	// A scroll bar when the entries do not all fit: arrows, a track and a thumb.
	let bar = rect::<R>(l, Part::ListScroll);
	if lst.overflows() && bar.h >= 3 {
		let track = usize::from(bar.h) - 2;
		for yy in 1..=track {
			g.put(usize::from(bar.x), usize::from(bar.y) + yy, 0xB0, CHROME);
		}
		g.put(usize::from(bar.x), usize::from(bar.y), 0x1E, CHROME);
		g.put(
			usize::from(bar.x),
			usize::from(bar.y + bar.h - 1),
			0x1F,
			CHROME,
		);
		let at = thumb(lst.count(), lst.rows(), lst.top(), track);
		g.put(
			usize::from(bar.x),
			usize::from(bar.y) + 1 + at,
			0xDB,
			CHROME,
		);
	}
}

/// A file's date in its column, with two-digit years.
fn stamp<const R: usize, M: Measure + Clone>(
	g: &mut Grid<R>,
	app: &App<M>,
	e: &Entry,
	x: u16,
	y: u16,
	a: u8,
) {
	if e.date() == 0 {
		return;
	}
	let mut out = [0u8; 16];
	let n = date_text(app.fd.date_pattern(), e.date(), true, &mut out);
	put(g, x, y, out.get(..n).unwrap_or(&[]), a, 8);
}

fn preview<const R: usize, M: Measure + Clone, S: ReadStore + ?Sized>(
	g: &mut Grid<R>,
	l: &Layout,
	dlg: &FileDlg,
	s: &S,
	app: &App<M>,
) {
	let pane = l.preview_pane;
	g.frame(
		usize::from(pane.x),
		usize::from(pane.y),
		usize::from(pane.w),
		usize::from(pane.h),
		CHROME,
	);
	g.text(
		usize::from(pane.x) + 2,
		usize::from(pane.y),
		b" Preview ",
		CHROME,
	);
	let body = rect::<R>(l, Part::Preview);
	fill(g, body, CHROME);
	let Some(e) = dlg.list().selected().and_then(|i| dlg.entry(s, i)) else {
		return;
	};
	if e.kind() == EntryKind::Up {
		return;
	}
	let (x, y) = (body.x, body.y);
	// A box with the type's glyph in it, and the name and the kind of file beside it.
	let icon = icon_of(&e);
	g.put(usize::from(x), usize::from(y), 0xDA, CHROME);
	g.put(usize::from(x) + 1, usize::from(y), 0xC4, CHROME);
	g.put(usize::from(x) + 2, usize::from(y), 0xBF, CHROME);
	g.put(usize::from(x), usize::from(y) + 1, 0xB3, CHROME);
	g.put(
		usize::from(x) + 1,
		usize::from(y) + 1,
		glyph(icon).ch,
		CHROME,
	);
	g.put(usize::from(x) + 2, usize::from(y) + 1, 0xB3, CHROME);
	g.put(usize::from(x), usize::from(y) + 2, 0xC0, CHROME);
	g.put(usize::from(x) + 1, usize::from(y) + 2, 0xC4, CHROME);
	g.put(usize::from(x) + 2, usize::from(y) + 2, 0xD9, CHROME);
	let w = usize::from(body.w);
	put(g, x + 4, y + 1, e.name(), CHROME, w - 4);
	put(g, x + 4, y + 2, type_name(&e), CHROME, w - 4);
	if e.kind() == EntryKind::File {
		let mut sz = [0u8; 12];
		let n = size_text(e.size(), &mut sz);
		put(g, x + 4, y + 3, sz.get(..n).unwrap_or(&[]), CHROME, w - 4);
	}
	if e.date() != 0 {
		let mut out = [0u8; 16];
		let n = date_text(app.fd.date_pattern(), e.date(), true, &mut out);
		put(g, x, y + 4, out.get(..n).unwrap_or(&[]), CHROME, w);
		let mut t = [0u8; 16];
		let m = time_text(app.fd.h24, e.time(), &mut t);
		put(
			g,
			x + n as u16 + 2,
			y + 4,
			t.get(..m).unwrap_or(&[]),
			CHROME,
			w,
		);
	}
	for xx in 0..body.w {
		g.put(usize::from(x + xx), usize::from(y) + 5, 0xC4, CHROME);
	}
}

fn bottom<const R: usize, S: ReadStore + ?Sized>(
	g: &mut Grid<R>,
	l: &Layout,
	dlg: &FileDlg,
	_s: &S,
	roles: &Roles,
	focus: u8,
) {
	let kind = dlg.kind();
	let label = rect::<R>(l, Part::NameLabel);
	put_label(
		g,
		usize::from(label.x),
		usize::from(label.y),
		str_of(text::NAME),
		CHROME,
		HOT,
	);
	let name = rect::<R>(l, Part::Name);
	fill(g, name, BODY);
	let f = dlg.name();
	let (start, shown) = field_window(f.len(), f.cursor(), usize::from(name.w));
	let bytes = f.as_bytes().get(start..start + shown).unwrap_or(&[]);
	put(g, name.x, name.y, bytes, BODY, bytes.len());
	let (sel_a, sel_b) = f.selection();
	if sel_b > sel_a {
		let from = sel_a.max(start);
		let to = sel_b.min(start + shown);
		if let Some(part) = f.as_bytes().get(from..to).filter(|p| !p.is_empty()) {
			put(
				g,
				name.x + (from - start) as u16,
				name.y,
				part,
				0x70,
				part.len(),
			);
		}
	}
	let tl = rect::<R>(l, Part::TypeLabel);
	put_label(
		g,
		usize::from(tl.x),
		usize::from(tl.y),
		str_of(type_label(kind)),
		CHROME,
		HOT,
	);
	let tb = rect::<R>(l, Part::TypeBox);
	let td = rect::<R>(l, Part::TypeDrop);
	let a = if focus == TYPES { roles.sel } else { BODY };
	fill(g, tb, a);
	fill(g, td, a);
	let kinds = dlg.types();
	if let Some(t) = kinds.get(dlg.type_index()) {
		put(g, tb.x, tb.y, t.label.as_bytes(), a, usize::from(tb.w));
	}
	g.put(
		usize::from(td.x),
		usize::from(td.y),
		0x1F,
		if focus == TYPES { roles.sel } else { CHROME },
	);
	// The line of counts, or the cap's hint.
	let st = rect::<R>(l, Part::Status);
	if dlg.more() {
		put(g, st.x, st.y, text::MORE, CHROME, usize::from(st.w));
	} else {
		let mut c = [0u8; 40];
		let n = counts(dlg, &mut c);
		put(
			g,
			st.x,
			st.y,
			c.get(..n).unwrap_or(&[]),
			CHROME,
			usize::from(st.w),
		);
	}
	button(
		g,
		rect::<R>(l, Part::Open),
		default_button(kind),
		focus == OK,
		focus != CANCEL,
		true,
	);
	button(
		g,
		rect::<R>(l, Part::Cancel),
		text::CANCEL,
		focus == CANCEL,
		false,
		true,
	);
}

/// The drop-down that is open: types above their box, Look in below its field.
fn drops<const R: usize>(g: &mut Grid<R>, l: &Layout, dlg: &FileDlg, roles: &Roles) {
	let tb = rect::<R>(l, Part::TypeBox);
	let td = rect::<R>(l, Part::TypeDrop);
	if dlg.type_box().is_open() {
		let n = dlg.type_box().count().min(6) as u16;
		let box_ = Rect::new(tb.x, tb.y.saturating_sub(n + 2), tb.w + td.w, n + 2);
		pop(g, box_);
		let lst = dlg.type_box().list();
		for i in 0..n {
			let idx = lst.top() + usize::from(i);
			let Some(t) = dlg.types().get(idx) else {
				break;
			};
			let a = if lst.selected() == Some(idx) {
				roles.sel
			} else {
				BODY
			};
			g.fill(
				usize::from(box_.x) + 1,
				usize::from(box_.y + 1 + i),
				usize::from(box_.w) - 2,
				1,
				b' ',
				a,
			);
			put(
				g,
				box_.x + 1,
				box_.y + 1 + i,
				t.label.as_bytes(),
				a,
				usize::from(box_.w) - 2,
			);
		}
	}
	if dlg.look_box().is_open() {
		let f = rect::<R>(l, Part::LookIn);
		let dr = rect::<R>(l, Part::LookInDrop);
		let n = dlg.look_box().count().min(6) as u16;
		let box_ = Rect::new(f.x, f.y + 1, f.w + dr.w, n + 2);
		pop(g, box_);
		let lst = dlg.look_box().list();
		for i in 0..n {
			let idx = lst.top() + usize::from(i);
			let Some(item) = dlg.look_item(idx) else {
				break;
			};
			let a = if lst.selected() == Some(idx) {
				roles.sel
			} else {
				BODY
			};
			let y = box_.y + 1 + i;
			g.fill(
				usize::from(box_.x) + 1,
				usize::from(y),
				usize::from(box_.w) - 2,
				1,
				b' ',
				a,
			);
			let indent =
				box_.x
					+ 1 + 2 * u16::from(item.indent.saturating_sub(1)) * u16::from(!item.is_drive());
			if item.is_drive() {
				put(g, box_.x + 1, y, &[item.letter, b':'], a, 2);
			} else {
				put(
					g,
					indent + 3,
					y,
					item.label(),
					a,
					usize::from(box_.w).saturating_sub(6),
				);
			}
		}
	}
}

/// A framed box with a shadow, over what is there.
fn pop<const R: usize>(g: &mut Grid<R>, r: Rect) {
	g.fill(
		usize::from(r.x),
		usize::from(r.y),
		usize::from(r.w),
		usize::from(r.h),
		b' ',
		BODY,
	);
	g.frame(
		usize::from(r.x),
		usize::from(r.y),
		usize::from(r.w),
		usize::from(r.h),
		CHROME,
	);
	shadow(
		g,
		usize::from(r.x),
		usize::from(r.y),
		usize::from(r.w),
		usize::from(r.h),
	);
}

/// A box in the middle of the dialog, `w` by `h`, framed in the dialog's colours.
fn centred<const R: usize>(g: &mut Grid<R>, l: &Layout, w: u16, h: u16, title: &[u8]) -> Rect {
	let d = l.dialog;
	let r = Rect::new(d.x + (d.w - w) / 2, d.y + (d.h.saturating_sub(h)) / 2, w, h);
	fill(g, r, CHROME);
	g.frame(
		usize::from(r.x),
		usize::from(r.y),
		usize::from(r.w),
		usize::from(r.h),
		CHROME,
	);
	let tx = usize::from(r.x) + (usize::from(r.w) - title.len() - 2) / 2;
	g.text(tx, usize::from(r.y), b" ", CHROME);
	g.text(tx + 1, usize::from(r.y), title, CHROME);
	g.text(tx + 1 + title.len(), usize::from(r.y), b" ", CHROME);
	shadow(
		g,
		usize::from(r.x),
		usize::from(r.y),
		usize::from(r.w),
		usize::from(r.h),
	);
	let _ = SHADOW_GAP;
	r
}

fn message_box<const R: usize>(
	g: &mut Grid<R>,
	l: &Layout,
	l1: Line,
	l2: Line,
	buttons: &[(&[u8], bool)],
) {
	let r = centred(g, l, 50, 7, b"Quill");
	put(g, r.x + 2, r.y + 2, l1.bytes(), CHROME, 46);
	put(g, r.x + 2, r.y + 3, l2.bytes(), CHROME, 46);
	// The buttons sit side by side, centred; the focused one is in normal video.
	let widths: [u16; 2] = [
		buttons.first().map_or(0, |b| button_width(b.0)),
		buttons.get(1).map_or(0, |b| button_width(b.0)),
	];
	let gap = 4;
	let total = widths[0] + if widths[1] > 0 { gap + widths[1] } else { 0 };
	let mut x = r.x + (r.w - total) / 2;
	for (i, (label, focused)) in buttons.iter().enumerate() {
		let w = widths.get(i).copied().unwrap_or(0);
		button(
			g,
			Rect::new(x, r.y + 5, w, 1),
			label,
			*focused,
			*focused,
			true,
		);
		x += w + gap;
	}
}

fn button_width(label: &[u8]) -> u16 {
	let mut shown = [0u8; 16];
	let (n, _) = quill_dlg::focus::strip_label(label, &mut shown).unwrap_or((0, None));
	n as u16 + 4
}

fn new_folder_box<const R: usize>(
	g: &mut Grid<R>,
	l: &Layout,
	name: &[u8],
	cursor: usize,
) -> Option<Cursor> {
	let r = centred(g, l, 36, 7, b"New Folder");
	put(g, r.x + 2, r.y + 2, b"Name of the new folder:", CHROME, 32);
	let field = Rect::new(r.x + 2, r.y + 3, 14, 1);
	fill(g, field, BODY);
	put(g, field.x, field.y, name, BODY, 12);
	put(
		g,
		r.x + 2,
		r.y + 5,
		b"Enter makes it, Esc cancels.",
		CHROME,
		32,
	);
	Some((usize::from(field.x) + cursor.min(12), usize::from(field.y)))
}
