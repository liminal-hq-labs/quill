// The toolbar, the ruler and the status bar of the Windows 3.1 window: drawn with GDI from the layout `quill_app::view` gives, with
// three real combo boxes for the style, face and size.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
use quill_app::view::{self, Act, RulerState, TOOLBAR, Toggle};
use w16::*;

use crate::gdi::{BLACK, FACE, QApp, Rect16, SHADOW, SRCCOPY, TRANSPARENT, WHITE, fill, fp, np};
use crate::store::FarStore;

/// Control identifiers of the combo boxes, and the colour list's commands (500 Automatic, 501 to 516 the colours).
pub const ID_STYLE: u32 = 303;
pub const ID_FACE: u32 = 301;
pub const ID_SIZE: u32 = 302;
pub const ID_COLOUR: u32 = 500;

const CB_ADDSTRING: u32 = WM_USER + 3;
const CB_GETCURSEL: u32 = WM_USER + 7;
const CB_GETLBTEXT: u32 = WM_USER + 8;
const CB_SETCURSEL: u32 = WM_USER + 14;
const CB_FINDSTRINGEXACT: u32 = WM_USER + 24;
const WM_SETFONT: u32 = 0x0030;
const WS_CHILD: u32 = 0x4000_0000;
const WS_VISIBLE: u32 = 0x1000_0000;
const WS_VSCROLL: u32 = 0x0020_0000;
const CBS_DROPDOWNLIST: u32 = 3;

/// The window's own drawing state: the small font of the bars, the toolbar's glyph strip and a DC to copy glyphs from.
pub struct Chrome {
	pub ui_font: u32,
	pub ui_h: i32,
	strip_dc: u32,
	strip_old: u32,
	pub style: u32,
	pub face: u32,
	pub size: u32,
	last_style: i32,
	last_face: [u8; 32],
	last_size: u16,
}

pub static mut CHROME: Chrome = Chrome {
	ui_font: 0,
	ui_h: 13,
	strip_dc: 0,
	strip_old: 0,
	style: 0,
	face: 0,
	size: 0,
	last_style: -2,
	last_face: [0xFF; 32],
	last_size: 0,
};

pub fn chrome() -> &'static mut Chrome {
	unsafe { &mut *core::ptr::addr_of_mut!(CHROME) }
}

/// The TrueType faces installed, from the `[fonts]` section of WIN.INI ("Arial (TrueType)" and the like, bold and italic files folded
/// into their face), sorted. Windows 3.1's `EnumFontFamilies` would need a callback of a shape the runtime does not export.
pub static mut FACES: [[u8; 32]; 24] = [[0; 32]; 24];
pub static mut NFACES: usize = 0;

pub fn faces() -> &'static [[u8; 32]] {
	unsafe {
		let f = &*core::ptr::addr_of!(FACES);
		f.get(..NFACES).unwrap_or(&[])
	}
}

fn name_of(f: &[u8; 32]) -> &[u8] {
	let n = f.iter().position(|b| *b == 0).unwrap_or(32);
	f.get(..n).unwrap_or(&[])
}

/// Reads the installed TrueType faces into `FACES`.
pub fn read_faces() {
	let mut buf = [0u8; 2048];
	let n = unsafe {
		GetProfileString(
			cs!("fonts"),
			0,
			cs!(""),
			np(buf.as_mut_ptr()),
			buf.len() as u32 - 1,
		)
	} as usize;
	let list = unsafe { &mut *core::ptr::addr_of_mut!(FACES) };
	let mut count = 0usize;
	for key in buf.get(..n).unwrap_or(&[]).split(|b| *b == 0) {
		let Some(at) = key.windows(11).position(|w| w == b" (TrueType)") else {
			continue;
		};
		let mut name = key.get(..at).unwrap_or(&[]);
		for suffix in [&b" Bold Italic"[..], b" Italic", b" Bold"] {
			if name.ends_with(suffix) {
				name = name.get(..name.len() - suffix.len()).unwrap_or(&[]);
			}
		}
		if name.is_empty() || name.len() > 31 || list.iter().take(count).any(|f| name_of(f) == name)
		{
			continue;
		}
		if let Some(slot) = list.get_mut(count) {
			*slot = [0; 32];
			slot.get_mut(..name.len()).map(|d| d.copy_from_slice(name));
			count += 1;
		}
	}
	// Sorted, by a simple insertion sort: there are a dozen at most.
	for i in 1..count {
		let mut j = i;
		while j > 0 && name_of(&list[j - 1]) > name_of(&list[j]) {
			list.swap(j - 1, j);
			j -= 1;
		}
	}
	unsafe { NFACES = count };
}

/// Makes the bars' font (MS Sans Serif at 8 points, as Windows 3.1's own dialogs use) and loads the toolbar's glyphs.
pub fn init(hinst: u32) {
	let c = chrome();
	c.ui_font = crate::gdi::make_font(-11, b"MS Sans Serif", false, false, 0, 0x20 | 2);
	unsafe {
		let dc = GetDC(0);
		let old = SelectObject(dc, c.ui_font);
		let mut tm = [0u8; 32];
		GetTextMetrics(dc, np(tm.as_mut_ptr()));
		SelectObject(dc, old);
		c.ui_h = i32::from(i16::from_le_bytes([tm[0], tm[1]])).max(8);
		c.strip_dc = CreateCompatibleDC(dc);
		ReleaseDC(0, dc);
		let strip = LoadBitmap(hinst, far(0, int_resource(2)));
		if c.strip_dc != 0 && strip != 0 {
			c.strip_old = SelectObject(c.strip_dc, strip);
		}
	}
}

/// The status bar's height.
pub fn status_h() -> i32 {
	chrome().ui_h + 6
}

fn glyph(dc: u32, k: i32, x: i32, y: i32) {
	let c = chrome();
	if c.strip_dc != 0 {
		unsafe {
			BitBlt(
				dc,
				x as u32,
				y as u32,
				16,
				15,
				c.strip_dc,
				(k * 16) as u32,
				0,
				SRCCOPY,
			)
		};
	}
}

/// A raised or sunken three-dimensional button face.
fn bevel(dc: u32, x: i32, y: i32, w: i32, h: i32, down: bool) {
	fill(dc, x, y, w, h, BLACK);
	fill(dc, x + 1, y + 1, w - 2, h - 2, FACE);
	let (tl, br) = if down {
		(SHADOW, WHITE)
	} else {
		(WHITE, SHADOW)
	};
	fill(dc, x + 1, y + 1, w - 3, 1, tl);
	fill(dc, x + 1, y + 1, 1, h - 3, tl);
	fill(dc, x + 2, y + h - 2, w - 3, 1, br);
	fill(dc, x + w - 2, y + 2, 1, h - 3, br);
}

/// Paints the toolbar rows across `width` pixels at the top of the client area.
pub fn paint_toolbar(dc: u32, width: i32, app: &QApp, s: &FarStore) {
	let (h0, h1) = view::rows(app);
	let cs = app.char_state(s);
	for (row, top, h) in [(0u8, 0, h0), (1u8, h0, h1)] {
		if h == 0 {
			continue;
		}
		fill(dc, 0, top, width, view::TB_H, FACE);
		fill(dc, 0, top + view::TB_H - 1, width, 1, SHADOW);
		fill(dc, 0, top, width, 1, WHITE);
		for b in TOOLBAR.buttons.iter().filter(|b| b.row == row) {
			let down = b.toggle != Toggle::No && view::is_on(app, s, b.toggle);
			bevel(dc, b.x, top + 2, view::BTN_W, view::BTN_H, down);
			let off = i32::from(down);
			glyph(dc, b.glyph, b.x + 4 + off, top + 6 + off);
			if b.act == Act::Colour {
				if let Some((r, g, bl)) = cs.colour {
					fill(
						dc,
						b.x + 6 + off,
						top + 17 + off,
						12,
						3,
						u32::from(r) | (u32::from(g) << 8) | (u32::from(bl) << 16),
					);
				}
			}
		}
	}
}

fn triangle(dc: u32, pts: [(i32, i32); 3], colour: u32) {
	let p: [i16; 6] = [
		pts[0].0 as i16,
		pts[0].1 as i16,
		pts[1].0 as i16,
		pts[1].1 as i16,
		pts[2].0 as i16,
		pts[2].1 as i16,
	];
	unsafe {
		let b = CreateSolidBrush(colour);
		let old = SelectObject(dc, b);
		Polygon(dc, np(p.as_ptr()), 3);
		SelectObject(dc, old);
		DeleteObject(b);
	}
}

/// The mark of a tab stop kind (0 left, 1 centre, 2 right, 3 decimal) with its corner on the scale's baseline at `x`.
fn tab_mark(dc: u32, kind: u8, x: i32, y: i32) {
	fill(dc, x, y - 5, 1, 6, BLACK);
	match kind {
		0 => fill(dc, x, y, 5, 1, BLACK),
		2 => fill(dc, x - 4, y, 5, 1, BLACK),
		_ => {
			fill(dc, x - 3, y, 7, 1, BLACK);
			if kind == 3 {
				fill(dc, x + 2, y - 3, 1, 1, BLACK);
			}
		}
	}
}

/// Paints the ruler across `width` pixels with its top at `top`.
pub fn paint_ruler(
	dc: u32,
	top: i32,
	width: i32,
	text_left: i32,
	text_w: i32,
	st: &RulerState,
	kind: u8,
) {
	fill(dc, 0, top, width, view::RULER_H, FACE);
	fill(dc, 0, top + view::RULER_H - 1, width, 1, SHADOW);
	fill(dc, text_left, top + 8, text_w, 7, WHITE);
	fill(dc, text_left - 1, top + 8, 1, 7, BLACK);
	fill(dc, text_left + text_w, top + 8, 1, 7, BLACK);
	unsafe {
		let old = SelectObject(dc, chrome().ui_font);
		SetBkMode(dc, TRANSPARENT);
		SetTextColor(dc, BLACK);
		for (dx, h, inch) in view::ruler_ticks(text_w) {
			let x = text_left + dx;
			if h > 0 {
				fill(dc, x, top + 8 + h, 1, 7 - 2 * h, BLACK);
			} else if inch > 0 {
				let d = [b'0' + (inch % 10) as u8];
				TextOut(dc, (x - 2) as u32, (top + 7) as u32, np(d.as_ptr()), 1);
			}
		}
		SelectObject(dc, old);
	}
	for (pos, k) in st.stops() {
		tab_mark(dc, *k, view::tab_x(*pos, text_left), top + 14);
	}
	let (fx, lx, rx) = view::ruler_marks(st, text_left, text_w);
	triangle(
		dc,
		[(fx - 4, top + 1), (fx + 4, top + 1), (fx, top + 8)],
		WHITE,
	);
	triangle(
		dc,
		[(lx, top + 14), (lx - 4, top + 19), (lx + 4, top + 19)],
		WHITE,
	);
	fill(dc, lx - 4, top + 19, 9, 3, WHITE);
	triangle(
		dc,
		[(rx, top + 14), (rx - 4, top + 19), (rx + 4, top + 19)],
		WHITE,
	);
	bevel(dc, 2, top + 2, 18, 18, false);
	tab_mark(dc, kind, 8 + i32::from(kind == 2) * 3, top + 14);
}

/// Paints the status bar: a sunken panel across the bottom with the editor's status text in it.
pub fn paint_status(dc: u32, top: i32, width: i32, text: &[u8]) {
	let h = status_h();
	fill(dc, 0, top, width, h, FACE);
	fill(dc, 0, top, width, 1, WHITE);
	fill(dc, 2, top + 2, width - 4, 1, SHADOW);
	fill(dc, 2, top + 2, 1, h - 4, SHADOW);
	fill(dc, 2, top + h - 2, width - 4, 1, WHITE);
	fill(dc, width - 3, top + 2, 1, h - 4, WHITE);
	unsafe {
		let old = SelectObject(dc, chrome().ui_font);
		SetBkMode(dc, TRANSPARENT);
		SetTextColor(dc, BLACK);
		let r = Rect16::new(4, top + 3, width - 4, top + h - 2);
		let saved = SaveDC(dc);
		IntersectClipRect(
			dc,
			r.left as u32,
			r.top as u32,
			r.right as u32,
			r.bottom as u32,
		);
		TextOut(
			dc,
			6,
			(top + 3) as u32,
			np(text.as_ptr()),
			text.len() as u32,
		);
		RestoreDC(dc, saved);
		SelectObject(dc, old);
	}
}

/// Makes the style, face and size boxes as children of `hwnd` and fills them.
pub fn create_combos(hwnd: u32, hinst: u32) {
	let c = chrome();
	let make = |id: u32, (x, w): (i32, i32)| unsafe {
		let h = CreateWindow(
			cs!("COMBOBOX"),
			cs!(""),
			WS_CHILD | WS_VISIBLE | WS_VSCROLL | CBS_DROPDOWNLIST,
			x as u32,
			(view::TB_H + 3) as u32,
			w as u32,
			200,
			hwnd,
			id,
			hinst,
			0,
		);
		SendMessage(h, WM_SETFONT, c.ui_font, 0);
		h
	};
	c.style = make(ID_STYLE, TOOLBAR.style);
	c.face = make(ID_FACE, TOOLBAR.face);
	c.size = make(ID_SIZE, TOOLBAR.size);
	let mut z = [0u8; 40];
	for n in quill_app::STYLE_NAMES {
		add(c.style, n.as_bytes(), &mut z);
	}
	for f in faces() {
		add(c.face, name_of(f), &mut z);
	}
	for sz in view::SIZES {
		let mut d = [0u8; 4];
		let k = dec(u32::from(sz), &mut d);
		add(c.size, d.get(..k).unwrap_or(&[]), &mut z);
	}
}

fn add(h: u32, text: &[u8], z: &mut [u8; 40]) {
	let n = text.len().min(39);
	z.fill(0);
	z.get_mut(..n)
		.map(|d| d.copy_from_slice(text.get(..n).unwrap_or(&[])));
	unsafe { SendMessage(h, CB_ADDSTRING, 0, fp(z.as_ptr())) };
}

/// Writes `v` in decimal into `out` and returns its length.
pub fn dec(mut v: u32, out: &mut [u8]) -> usize {
	let mut tmp = [0u8; 10];
	let mut i = 0;
	loop {
		tmp[i] = b'0' + (v % 10) as u8;
		i += 1;
		v /= 10;
		if v == 0 || i == tmp.len() {
			break;
		}
	}
	let n = i.min(out.len());
	for k in 0..n {
		out[k] = tmp[i - 1 - k];
	}
	n
}

/// Shows or hides the boxes with the formatting row and moves them under the standard row.
pub fn show_combos(app: &QApp) {
	let c = chrome();
	let on = app.show_format_bar();
	let top = view::rows(app).0 + 3;
	for (h, (x, w)) in [
		(c.style, TOOLBAR.style),
		(c.face, TOOLBAR.face),
		(c.size, TOOLBAR.size),
	] {
		if h != 0 {
			unsafe {
				MoveWindow(h, x as u32, top as u32, w as u32, 200, 1);
				ShowWindow(h, if on { 5 } else { 0 });
			}
		}
	}
}

/// Shows the style, face and size at the caret in the boxes.
pub fn sync_combos(app: &QApp, s: &FarStore) {
	let c = chrome();
	if c.style == 0 {
		return;
	}
	let st = app.style_of(s).map_or(-1, |n| n as i32);
	unsafe {
		if st != c.last_style {
			SendMessage(c.style, CB_SETCURSEL, st as u32 & 0xFFFF, 0);
			c.last_style = st;
		}
		let cs = app.char_state(s);
		let mut face = [0u8; 32];
		let n = usize::from(cs.face_len).min(31);
		face.get_mut(..n)
			.map(|d| d.copy_from_slice(cs.face.get(..n).unwrap_or(&[])));
		if face != c.last_face {
			let at = SendMessage(c.face, CB_FINDSTRINGEXACT, 0xFFFF, fp(face.as_ptr()));
			SendMessage(c.face, CB_SETCURSEL, at & 0xFFFF, 0);
			c.last_face = face;
		}
		if cs.size != c.last_size {
			let pts = cs.size / 2;
			let at = view::SIZES
				.iter()
				.position(|v| *v == pts)
				.map_or(0xFFFF, |i| i as u32);
			SendMessage(c.size, CB_SETCURSEL, at, 0);
			c.last_size = cs.size;
		}
	}
}

/// The text of the current choice in box `id`, into `out`; its length, or None when nothing is chosen.
pub fn choice(id: u32, out: &mut [u8; 40]) -> Option<usize> {
	let c = chrome();
	let h = match id {
		ID_STYLE => c.style,
		ID_FACE => c.face,
		_ => c.size,
	};
	unsafe {
		let at = SendMessage(h, CB_GETCURSEL, 0, 0) & 0xFFFF;
		if at == 0xFFFF {
			return None;
		}
		out.fill(0);
		let n = SendMessage(h, CB_GETLBTEXT, at, fp(out.as_mut_ptr())) & 0xFFFF;
		if n == 0xFFFF || n == 0 {
			return None;
		}
		Some((n as usize).min(39))
	}
}

/// The index chosen in box `id`, or None.
pub fn chosen_index(id: u32) -> Option<usize> {
	let h = if id == ID_STYLE {
		chrome().style
	} else {
		chrome().face
	};
	let at = unsafe { SendMessage(h, CB_GETCURSEL, 0, 0) } & 0xFFFF;
	(at != 0xFFFF).then_some(at as usize)
}

/// Drops the colour list below (`x`, `y`) in the client area; the choice comes back as a WM_COMMAND of `ID_COLOUR` to `ID_COLOUR + 16`.
pub fn colour_menu(hwnd: u32, x: i32, y: i32) {
	unsafe {
		let m = CreatePopupMenu();
		AppendMenu(m, 0, ID_COLOUR, cs!("Automatic"));
		let mut z = [0u8; 40];
		for (i, (name, _)) in view::COLOURS.iter().enumerate() {
			let b = name.as_bytes();
			z.fill(0);
			z.get_mut(..b.len()).map(|d| d.copy_from_slice(b));
			AppendMenu(m, 0, ID_COLOUR + 1 + i as u32, np(z.as_ptr()));
		}
		let mut p = [x as i16, y as i16];
		ClientToScreen(hwnd, np(p.as_mut_ptr()));
		TrackPopupMenu(m, 0, p[0] as u16 as u32, p[1] as u16 as u32, 0, hwnd, 0);
		DestroyMenu(m);
	}
}
