// The toolbar and the ruler: drawn by the painter, with two real combo boxes for the face and size, and clicks turned into commands.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;
use core::ptr::{null, null_mut};

use quill_app::{App, Command};
use quill_core::doc::cflag;
use quill_heap::HeapStore;

use crate::gdi::GdiMeasure;
use crate::scheme;
use crate::sys::*;

/// Height of each of the two toolbar rows, in pixels: a 24-pixel button and two pixels either side.
pub const TB_H: i32 = 28;
/// Height of the ruler.
pub const RULER_H: i32 = 22;
const BTN_W: i32 = 24;
const BTN_H: i32 = 24;
/// Control identifiers of the combo boxes.
pub const ID_FACE: usize = 301;
pub const ID_SIZE: usize = 302;
pub const ID_STYLE: usize = 303;
/// Pop-up menu command numbers for the colour list: 500 is Automatic, 501 to 516 the 16 colours.
const ID_COLOUR: usize = 500;

static STRIP: &[u8] = include_bytes!("../../../assets/toolbar/toolbar.bmp");

/// The glyphs in the strip, in order.
const GLYPH_COLOUR: i32 = 13;
const GLYPH_ARROW: i32 = 21;

/// What a button does.
#[derive(Copy, Clone, PartialEq)]
pub enum Act {
	Cmd(Command),
	Colour,
	None,
}

/// What makes a button show pressed.
#[derive(Copy, Clone, PartialEq)]
pub enum Toggle {
	No,
	Bold,
	Italic,
	Underline,
	Align(u8),
	Bullets,
	Numbering,
}

/// One toolbar button.
#[derive(Copy, Clone)]
pub struct Button {
	pub row: u8,
	pub x: i32,
	pub glyph: i32,
	pub act: Act,
	pub toggle: Toggle,
}

/// Where the toolbar's parts are: the standard row (0) and the formatting row (1).
pub struct Layout {
	pub buttons: Vec<Button>,
	pub style_x: i32,
	pub style_w: i32,
	pub face_x: i32,
	pub face_w: i32,
	pub size_x: i32,
	pub size_w: i32,
}

/// Lays the two rows out from the left. Row 0: the file group, the clipboard, undo, spelling. Row 1: the style, face and size boxes,
/// the character buttons, alignment and lists.
pub fn layout() -> Layout {
	let mut b = Vec::new();
	let add = |b: &mut Vec<Button>, row: u8, x: &mut i32, glyph: i32, act: Act, toggle: Toggle| {
		b.push(Button {
			row,
			x: *x,
			glyph,
			act,
			toggle,
		});
		*x += BTN_W;
	};
	let mut x = 4;
	for (g, c) in [
		(0, Command::New),
		(1, Command::Open),
		(2, Command::Save),
		(3, Command::Print),
		(4, Command::Preview),
	] {
		add(&mut b, 0, &mut x, g, Act::Cmd(c), Toggle::No);
	}
	x += 8;
	for (g, c) in [(5, Command::Cut), (6, Command::Copy), (7, Command::Paste)] {
		add(&mut b, 0, &mut x, g, Act::Cmd(c), Toggle::No);
	}
	x += 8;
	for (g, c) in [(8, Command::Undo), (9, Command::Redo)] {
		add(&mut b, 0, &mut x, g, Act::Cmd(c), Toggle::No);
	}
	x += 8;
	add(&mut b, 0, &mut x, 20, Act::Cmd(Command::Spelling), Toggle::No);
	let mut x = 4;
	let (style_x, style_w) = (x, 104);
	x += style_w + 4;
	let (face_x, face_w) = (x, 140);
	x += face_w + 4;
	let (size_x, size_w) = (x, 48);
	x += size_w + 8;
	add(&mut b, 1, &mut x, 10, Act::Cmd(Command::Bold), Toggle::Bold);
	add(&mut b, 1, &mut x, 11, Act::Cmd(Command::Italic), Toggle::Italic);
	add(&mut b, 1, &mut x, 12, Act::Cmd(Command::Underline), Toggle::Underline);
	add(&mut b, 1, &mut x, GLYPH_COLOUR, Act::Colour, Toggle::No);
	x += 8;
	for n in 0..4u8 {
		add(
			&mut b,
			1,
			&mut x,
			14 + i32::from(n),
			Act::Cmd(Command::Align(n)),
			Toggle::Align(n),
		);
	}
	x += 8;
	add(&mut b, 1, &mut x, 18, Act::Cmd(Command::Bullets), Toggle::Bullets);
	add(&mut b, 1, &mut x, 19, Act::Cmd(Command::Numbering), Toggle::Numbering);
	Layout {
		buttons: b,
		style_x,
		style_w,
		face_x,
		face_w,
		size_x,
		size_w,
	}
}

/// The heights of the standard and formatting rows as they are showing, and the top of the formatting row.
pub fn rows(app: &App<GdiMeasure>) -> (i32, i32) {
	(i32::from(app.show_toolbar()) * TB_H, i32::from(app.show_format_bar()) * TB_H)
}

unsafe fn fill(dc: Handle, x: i32, y: i32, w: i32, h: i32, colour: u32) {
	let r = Rect {
		left: x,
		top: y,
		right: x + w,
		bottom: y + h,
	};
	let b = CreateSolidBrush(colour);
	FillRect(dc, &r, b);
	DeleteObject(b);
}

/// The strip as drawn: a copy whose silver background and light-grey shading are the scheme's face and its light colour.
static mut LIVE_STRIP: Vec<u8> = Vec::new();

/// Palette entries of the strip that are chrome colours rather than picture: the silver background and the light grey of a highlight.
const STRIP_FACE: usize = 0;
const STRIP_LIGHT: usize = 11;
/// Where the strip's palette starts: the 14-byte file header and the 40-byte information header.
const STRIP_PALETTE: usize = 54;

/// Rebuilds the glyph strip for the current scheme, so a button's picture sits on the button's face on any theme.
pub fn recolour_strip() {
	let mut v = STRIP.to_vec();
	for (entry, colour) in [(STRIP_FACE, scheme::face()), (STRIP_LIGHT, light())] {
		if let Some(p) = v.get_mut(STRIP_PALETTE + 4 * entry..STRIP_PALETTE + 4 * entry + 4) {
			p.copy_from_slice(&[(colour >> 16) as u8, (colour >> 8) as u8, colour as u8, 0]);
		}
	}
	// SAFETY: the shell is single-threaded; nothing else reads the strip while the scheme is being applied.
	unsafe {
		*core::ptr::addr_of_mut!(LIVE_STRIP) = v;
	}
}

unsafe fn glyph(dc: Handle, k: i32, x: i32, y: i32) {
	let live = &*core::ptr::addr_of!(LIVE_STRIP);
	let strip: &[u8] = if live.len() == STRIP.len() { live } else { STRIP };
	let off = u32::from_le_bytes([strip[10], strip[11], strip[12], strip[13]]) as usize;
	let info = strip.as_ptr().add(14).cast::<BitmapInfoHeader>();
	SetDIBitsToDevice(dc, x, y, 16, 15, k * 16, 0, 0, 15, strip.as_ptr().add(off), info, 0);
}

/// How a toolbar button looks.
#[derive(Copy, Clone, PartialEq)]
pub enum Face {
	/// Flat: no border, the toolbar's own colour.
	Flat,
	/// Under the pointer: a thin raised bevel.
	Raised,
	/// Pressed, or switched on: a thin sunken bevel.
	Down,
}

/// The inner ring of a bevel is halfway between the face and the edge: the Windows 95 toolbar bevel is two pixels deep and has no black.
fn light() -> u32 {
	scheme::mix(scheme::face(), scheme::hilight())
}

fn mid() -> u32 {
	scheme::mix(scheme::face(), scheme::shadow())
}

/// Draws a button's face: flat until the pointer is over it, then a two-pixel raised bevel (white and light grey on the top and left,
/// dark grey on the bottom and right); pressed or switched on, the same bevel sunken, with a lighter face for a switched-on toggle.
unsafe fn button_face(dc: Handle, x: i32, y: i32, w: i32, h: i32, face: Face, lit: bool) {
	let (face_c, shadow, white, light, mid) = (scheme::face(), scheme::shadow(), scheme::hilight(), light(), mid());
	let base = if face == Face::Down && lit { light } else { face_c };
	fill(dc, x, y, w, h, base);
	if face == Face::Flat {
		return;
	}
	let (tl, tl2, br, br2) = if face == Face::Down {
		(shadow, mid, white, light)
	} else {
		(white, light, shadow, mid)
	};
	// Outer ring.
	fill(dc, x, y, w - 1, 1, tl);
	fill(dc, x, y, 1, h - 1, tl);
	fill(dc, x, y + h - 1, w, 1, br);
	fill(dc, x + w - 1, y, 1, h, br);
	// Inner ring.
	fill(dc, x + 1, y + 1, w - 3, 1, tl2);
	fill(dc, x + 1, y + 1, 1, h - 3, tl2);
	fill(dc, x + 1, y + h - 2, w - 2, 1, br2);
	fill(dc, x + w - 2, y + 1, 1, h - 2, br2);
}

/// Whether the toggle's property holds at the caret.
fn is_on(app: &App<GdiMeasure>, s: &HeapStore, t: Toggle) -> bool {
	match t {
		Toggle::No => false,
		Toggle::Bold | Toggle::Italic | Toggle::Underline => {
			let f = app.char_state(s).flags;
			f & match t {
				Toggle::Bold => cflag::BOLD,
				Toggle::Italic => cflag::ITALIC,
				_ => cflag::UNDERLINE,
			} != 0
		}
		Toggle::Align(n) => app.para_state(s).align == n,
		Toggle::Bullets => app.para_state(s).flags & 1 != 0,
		Toggle::Numbering => app.para_state(s).flags & 4 != 0,
	}
}

/// Paints the toolbar rows across `width` pixels at the top of the client area. `hover` is the button the pointer is over, `pressed`
/// the one the mouse button is down on.
///
/// # Safety
/// `dc` must be a valid device context.
pub unsafe fn paint_toolbar(
	dc: Handle,
	width: i32,
	app: &App<GdiMeasure>,
	s: &HeapStore,
	hover: Option<usize>,
	pressed: Option<usize>,
) {
	let (h0, h1) = rows(app);
	let lay = layout();
	for (row, top, h) in [(0u8, 0, h0), (1u8, h0, h1)] {
		if h == 0 {
			continue;
		}
		fill(dc, 0, top, width, TB_H, scheme::face());
		fill(dc, 0, top + TB_H - 1, width, 1, scheme::shadow());
		fill(dc, 0, top, width, 1, scheme::hilight());
		let mut prev_end: Option<i32> = None;
		for (i, b) in lay.buttons.iter().enumerate().filter(|(_, b)| b.row == row) {
			// An etched line fills the gap between groups of buttons.
			if let Some(e) = prev_end {
				if b.x - e > 4 {
					fill(dc, e + (b.x - e) / 2 - 1, top + 5, 1, TB_H - 10, scheme::shadow());
					fill(dc, e + (b.x - e) / 2, top + 5, 1, TB_H - 10, scheme::hilight());
				}
			}
			prev_end = Some(b.x + BTN_W);
			let on = is_on(app, s, b.toggle);
			let face = if pressed == Some(i) || on {
				Face::Down
			} else if hover == Some(i) {
				Face::Raised
			} else {
				Face::Flat
			};
			button_face(dc, b.x, top + 2, BTN_W, BTN_H, face, on);
			let off = i32::from(face == Face::Down);
			glyph(dc, b.glyph, b.x + 4 + off, top + 6 + off);
			if b.act == Act::Colour {
				// The colour button shows the text colour under the A.
				if let Some((r, g, bl)) = app.char_state(s).colour {
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

/// The index in the layout of the toolbar button at (`x`, `y`), and the top of its row.
pub fn button_index(app: &App<GdiMeasure>, x: i32, y: i32) -> Option<(usize, i32)> {
	let (b, top) = button_at(app, x, y)?;
	layout()
		.buttons
		.iter()
		.position(|c| c.row == b.row && c.x == b.x)
		.map(|i| (i, top))
}

/// The rectangle of button `i`, in the client area, and the command it runs when it has one.
pub fn button_rect(app: &App<GdiMeasure>, i: usize) -> Option<(Rect, Button)> {
	let b = *layout().buttons.get(i)?;
	let (h0, _) = rows(app);
	let top = if b.row == 0 { 0 } else { h0 };
	Some((
		Rect {
			left: b.x,
			top: top + 2,
			right: b.x + BTN_W,
			bottom: top + 2 + BTN_H,
		},
		b,
	))
}

/// Which toolbar button is at (`x`, `y`), and the top of its row.
pub fn button_at(app: &App<GdiMeasure>, x: i32, y: i32) -> Option<(Button, i32)> {
	let (h0, h1) = rows(app);
	let (row, top) = if y < h0 {
		(0u8, 0)
	} else if y < h0 + h1 {
		(1u8, h0)
	} else {
		return None;
	};
	if !(top + 2..top + 2 + BTN_H).contains(&y) {
		return None;
	}
	layout()
		.buttons
		.into_iter()
		.find(|b| b.row == row && x >= b.x && x < b.x + BTN_W)
		.map(|b| (b, top))
}

/// The sixteen colours of the colour list: name and red, green, blue.
const COLOURS: [(&[u8], (u8, u8, u8)); 16] = [
	(b"Black\0", (0, 0, 0)),
	(b"Maroon\0", (128, 0, 0)),
	(b"Green\0", (0, 128, 0)),
	(b"Olive\0", (128, 128, 0)),
	(b"Navy\0", (0, 0, 128)),
	(b"Purple\0", (128, 0, 128)),
	(b"Teal\0", (0, 128, 128)),
	(b"Silver\0", (192, 192, 192)),
	(b"Grey\0", (128, 128, 128)),
	(b"Red\0", (255, 0, 0)),
	(b"Lime\0", (0, 255, 0)),
	(b"Yellow\0", (255, 255, 0)),
	(b"Blue\0", (0, 0, 255)),
	(b"Fuchsia\0", (255, 0, 255)),
	(b"Aqua\0", (0, 255, 255)),
	(b"White\0", (255, 255, 255)),
];

/// Drops the colour list below the button and returns the choice: `Some(None)` for Automatic, `Some(Some(rgb))` for a colour.
///
/// # Safety
/// `hwnd` must be the main window.
pub unsafe fn colour_menu(hwnd: Handle, x: i32, y: i32) -> Option<Option<(u8, u8, u8)>> {
	let m = CreatePopupMenu();
	AppendMenuA(m, MF_STRING, ID_COLOUR, b"Automatic\0".as_ptr());
	for (i, (name, _)) in COLOURS.iter().enumerate() {
		AppendMenuA(m, MF_STRING, ID_COLOUR + 1 + i, name.as_ptr());
	}
	let mut p = Point { x, y };
	ClientToScreen(hwnd, &mut p);
	// TPM_RETURNCMD: the command number comes back instead of being posted.
	let cmd = TrackPopupMenu(m, 0x0100, p.x, p.y, 0, hwnd, null());
	DestroyMenu(m);
	let cmd = usize::try_from(cmd).ok()?;
	match cmd {
		ID_COLOUR => Some(None),
		c if (ID_COLOUR + 1..=ID_COLOUR + 16).contains(&c) => COLOURS.get(c - ID_COLOUR - 1).map(|(_, rgb)| Some(*rgb)),
		_ => None,
	}
}

// ---- the face list -------------------------------------------------------------------------------------------------------------

static mut FACES: Vec<Vec<u8>> = Vec::new();

unsafe extern "system" fn face_callback(logfont: *const u8, _metric: *const u8, font_type: u32, _lparam: isize) -> i32 {
	// LOGFONT's face name starts at byte 28; only TrueType faces (4) are offered, and not the vertical ones that start with '@'.
	if font_type & 4 != 0 {
		let name = core::slice::from_raw_parts(logfont.add(28), 32);
		let n = name.iter().position(|b| *b == 0).unwrap_or(0);
		let name = &name[..n];
		let faces = &mut *core::ptr::addr_of_mut!(FACES);
		if n > 0 && name[0] != b'@' && !faces.iter().any(|f| f == name) {
			faces.push(name.to_vec());
		}
	}
	1
}

/// Lists the TrueType faces on the display, sorted.
///
/// # Safety
/// `dc` must be a valid device context.
pub unsafe fn enumerate_faces(dc: Handle) -> Vec<Vec<u8>> {
	EnumFontFamiliesA(dc, null(), face_callback, 0);
	let faces = &mut *core::ptr::addr_of_mut!(FACES);
	faces.sort();
	core::mem::take(faces)
}

/// The sizes the size box offers, in points.
pub const SIZES: [u16; 15] = [8, 9, 10, 11, 12, 14, 16, 18, 20, 24, 28, 32, 36, 48, 72];

const CB_ADDSTRING: u32 = 0x0143;
const CB_GETCURSEL: u32 = 0x0147;
const CB_GETLBTEXT: u32 = 0x0148;
const CB_SETCURSEL: u32 = 0x014E;
const CB_FINDSTRINGEXACT: u32 = 0x0158;
const WM_SETFONT: u32 = 0x0030;
const WS_CHILD: u32 = 0x4000_0000;
const WS_VISIBLE: u32 = 0x1000_0000;
const WS_VSCROLL: u32 = 0x0020_0000;
const CBS_DROPDOWNLIST: u32 = 3;

/// The two combo boxes.
pub struct Combos {
	pub style: Handle,
	pub face: Handle,
	pub size: Handle,
	last_face: Vec<u8>,
	last_size: u16,
	last_style: i32,
}

/// Makes the face and size boxes as children of `hwnd` and fills them.
///
/// # Safety
/// `hwnd` must be the main window.
pub unsafe fn create_combos(hwnd: Handle, instance: Handle, faces: &[Vec<u8>]) -> Combos {
	let lay = layout();
	let font = GetStockObject(17); // DEFAULT_GUI_FONT
	let make = |id: usize, x: i32, w: i32| {
		let h = CreateWindowExA(
			0,
			b"COMBOBOX\0".as_ptr(),
			b"\0".as_ptr(),
			WS_CHILD | WS_VISIBLE | WS_VSCROLL | CBS_DROPDOWNLIST,
			x,
			TB_H + 4,
			w,
			240,
			hwnd,
			id as Handle,
			instance,
			null(),
		);
		SendMessageA(h, WM_SETFONT, font as usize, 0);
		h
	};
	let style = make(ID_STYLE, lay.style_x, lay.style_w);
	let face = make(ID_FACE, lay.face_x, lay.face_w);
	let size = make(ID_SIZE, lay.size_x, lay.size_w);
	for n in quill_app::STYLE_NAMES {
		let t = alloc::format!("{n}\0");
		SendMessageA(style, CB_ADDSTRING, 0, t.as_ptr() as isize);
	}
	for f in faces {
		let mut z = f.clone();
		z.push(0);
		SendMessageA(face, CB_ADDSTRING, 0, z.as_ptr() as isize);
	}
	for s in SIZES {
		let t = alloc::format!("{s}\0");
		SendMessageA(size, CB_ADDSTRING, 0, t.as_ptr() as isize);
	}
	Combos {
		style,
		face,
		size,
		last_face: Vec::new(),
		last_size: 0,
		last_style: -2,
	}
}

impl Combos {
	/// Shows or hides the boxes with the toolbar.
	///
	/// # Safety
	/// The handles must be live.
	pub unsafe fn show(&self, app: &App<GdiMeasure>, preview: bool) {
		let on = app.show_format_bar() && !preview;
		let lay = layout();
		let top = rows(app).0 + 4;
		for (h, x, w) in [
			(self.style, lay.style_x, lay.style_w),
			(self.face, lay.face_x, lay.face_w),
			(self.size, lay.size_x, lay.size_w),
		] {
			MoveWindow(h, x, top, w, 240, 1);
			ShowWindow(h, i32::from(on) * 5);
		}
	}

	/// Shows the face and size at the caret.
	///
	/// # Safety
	/// The handles must be live.
	pub unsafe fn sync(&mut self, app: &App<GdiMeasure>, s: &HeapStore) {
		let st = app.style_of(s).map_or(-1, |n| n as i32);
		if st != self.last_style {
			SendMessageA(self.style, CB_SETCURSEL, st as usize, 0);
			self.last_style = st;
		}
		let cs = app.char_state(s);
		let face = cs.face.get(..usize::from(cs.face_len)).unwrap_or(&[]).to_vec();
		if face != self.last_face {
			let mut z = face.clone();
			z.push(0);
			let at = SendMessageA(self.face, CB_FINDSTRINGEXACT, usize::MAX, z.as_ptr() as isize);
			SendMessageA(self.face, CB_SETCURSEL, at as usize, 0);
			self.last_face = face;
		}
		if cs.size != self.last_size {
			let pts = cs.size / 2;
			let at = SIZES.iter().position(|s| *s == pts).map_or(-1isize, |i| i as isize);
			SendMessageA(self.size, CB_SETCURSEL, at as usize, 0);
			self.last_size = cs.size;
		}
	}

	/// The text of the current choice in a box, or None when there is none.
	///
	/// # Safety
	/// The handles must be live.
	pub unsafe fn choice(&self, id: usize) -> Option<Vec<u8>> {
		let h = match id {
			ID_FACE => self.face,
			ID_STYLE => self.style,
			_ => self.size,
		};
		let at = SendMessageA(h, CB_GETCURSEL, 0, 0);
		if at < 0 {
			return None;
		}
		let mut buf = [0u8; 64];
		let n = SendMessageA(h, CB_GETLBTEXT, at as usize, buf.as_mut_ptr() as isize);
		(n > 0).then(|| buf[..n as usize].to_vec())
	}
}

/// A guess at a face's family from its name, for a document that records only the name.
pub fn family_of(name: &[u8]) -> u8 {
	let lower: Vec<u8> = name.iter().map(u8::to_ascii_lowercase).collect();
	let has = |w: &[u8]| lower.windows(w.len()).any(|x| x == w);
	if has(b"courier") || has(b"mono") || has(b"console") || has(b"fixed") {
		3
	} else if has(b"arial") || has(b"helvetica") || has(b"sans") || has(b"tahoma") || has(b"verdana") || has(b"trebuchet") {
		2
	} else {
		1
	}
}

// ---- the ruler -----------------------------------------------------------------------------------------------------------------

/// What a click on the ruler is aimed at.
#[derive(Copy, Clone, PartialEq)]
pub enum Hit {
	/// The first-line indent marker.
	First,
	/// The left indent marker alone.
	Left,
	/// The box under it, which moves the left and first-line indents together.
	Both,
	/// The right indent marker.
	Right,
	/// The tab stop at this position in twips.
	Tab(i32),
	/// The scale, at this position in twips from the left margin.
	Scale(i32),
	/// The button that cycles the kind of tab stop.
	Kind,
}

/// A paragraph's indents and tab stops as the ruler shows them, possibly changed by a drag in progress.
#[derive(Copy, Clone)]
pub struct RulerState {
	pub left: i32,
	pub first: i32,
	pub right: i32,
	pub tabs: [(i32, u8); 16],
	pub ntabs: usize,
}

impl RulerState {
	/// The state at the caret.
	pub fn of(app: &App<GdiMeasure>, s: &HeapStore) -> Self {
		let pf = app.para_state(s);
		let mut tabs = [(0, 0u8); 16];
		let ntabs = app.tab_stops(s, &mut tabs);
		RulerState {
			left: pf.left,
			first: pf.left + pf.first,
			right: pf.right,
			tabs,
			ntabs,
		}
	}
}

/// Twips to pixels at `pct` percent, and back.
pub fn px(twips: i32, pct: i32) -> i32 {
	(i64::from(twips) * i64::from(pct) / 1500) as i32
}

fn tw(pixels: i32, pct: i32) -> i32 {
	(i64::from(pixels) * 1500 / i64::from(pct.max(1))) as i32
}

/// The text width in pixels, and where the text's left edge is in the window, with the page scrolled `hscroll` pixels, at `pct` percent.
pub fn text_extent(app: &App<GdiMeasure>, client_w: i32, hscroll: i32, pct: i32) -> (i32, i32) {
	use crate::view::{dev, page_x0, paper};
	let page = &app.doc().page;
	let x0 = page_x0(client_w, dev(paper(app).0, pct), hscroll);
	let left = x0 + dev(quill_core::layout::to_units(page.margin_l, 96), pct);
	let w = dev(
		quill_core::layout::to_units(page.paper_w - page.margin_l - page.margin_r, 96),
		pct,
	);
	(left, w)
}

/// A number that changes whenever anything the toolbars and the ruler show changes: the toggles, the colour, the indents and the tabs.
/// A key press that leaves it alone needs no repaint of them.
pub fn signature(app: &App<GdiMeasure>, s: &HeapStore) -> u64 {
	let mut h: u64 = 0xcbf2_9ce4_8422_2325;
	let mut mix = |v: u64| {
		h = (h ^ v).wrapping_mul(0x0000_0100_0000_01b3);
	};
	let (r0, r1) = rows(app);
	mix(r0 as u64);
	mix(r1 as u64);
	mix(u64::from(app.show_ruler()));
	for b in layout().buttons.iter() {
		mix(u64::from(is_on(app, s, b.toggle)));
	}
	if let Some((r, g, b)) = app.char_state(s).colour {
		mix(1 + u64::from(r) + (u64::from(g) << 8) + (u64::from(b) << 16));
	}
	let st = RulerState::of(app, s);
	mix(st.left as u64);
	mix(st.first as u64);
	mix(st.right as u64);
	for t in st.tabs.iter().take(st.ntabs) {
		mix(t.0 as u64);
		mix(u64::from(t.1));
	}
	h
}

unsafe fn triangle(dc: Handle, pts: [(i32, i32); 3], fill_colour: u32) {
	let b = CreateSolidBrush(fill_colour);
	let old = SelectObject(dc, b);
	let p = [
		Point {
			x: pts[0].0,
			y: pts[0].1,
		},
		Point {
			x: pts[1].0,
			y: pts[1].1,
		},
		Point {
			x: pts[2].0,
			y: pts[2].1,
		},
	];
	Polygon(dc, p.as_ptr(), 3);
	SelectObject(dc, old);
	DeleteObject(b);
}

/// Draws the mark of a tab stop kind (0 left, 1 centre, 2 right, 3 decimal) with its corner at the scale's baseline at `x`.
unsafe fn tab_mark(dc: Handle, kind: u8, x: i32, y: i32) {
	match kind {
		0 => {
			fill(dc, x, y - 5, 1, 6, scheme::text());
			fill(dc, x, y, 5, 1, scheme::text());
		}
		2 => {
			fill(dc, x, y - 5, 1, 6, scheme::text());
			fill(dc, x - 4, y, 5, 1, scheme::text());
		}
		_ => {
			fill(dc, x, y - 5, 1, 6, scheme::text());
			fill(dc, x - 3, y, 7, 1, scheme::text());
			if kind == 3 {
				fill(dc, x + 2, y - 3, 1, 1, scheme::text());
			}
		}
	}
}

/// Paints the ruler across `width` pixels with its top at `top`.
///
/// # Safety
/// `dc` must be a valid device context.
pub unsafe fn paint_ruler(dc: Handle, top: i32, width: i32, text_left: i32, text_w: i32, st: &RulerState, kind: u8, pct: i32) {
	fill(dc, 0, top, width, RULER_H, scheme::face());
	fill(dc, 0, top + RULER_H - 1, width, 1, scheme::shadow());
	// The scale: white over the text width, grey in the margins.
	fill(dc, text_left, top + 8, text_w, 7, scheme::window());
	fill(dc, text_left - 1, top + 8, 1, 7, scheme::window_text());
	fill(dc, text_left + text_w, top + 8, 1, 7, scheme::window_text());
	let font = GetStockObject(17);
	let old = SelectObject(dc, font);
	SetBkMode(dc, TRANSPARENT);
	SetTextColor(dc, scheme::window_text());
	SetTextAlign(dc, 0);
	// An eighth of an inch is 12 page units; the labels are inches.
	for i in 0..=(crate::view::log(text_w, pct) * 8 / 96) {
		let x = text_left + crate::view::dev(i * 12, pct);
		let h = if i % 8 == 0 {
			0
		} else if i % 4 == 0 {
			3
		} else {
			5
		};
		if h > 0 {
			fill(dc, x, top + 8 + h, 1, 7 - 2 * h, scheme::window_text());
		} else if i > 0 {
			let t = alloc::format!("{}", i / 8);
			TextOutA(dc, x - 2, top + 8, t.as_ptr(), t.len() as i32);
		}
	}
	SelectObject(dc, old);
	// The tab stops, then the markers.
	for (pos, k) in st.tabs.iter().take(st.ntabs) {
		tab_mark(dc, *k, text_left + px(*pos, pct), top + 14);
	}
	let fx = text_left + px(st.first, pct);
	triangle(dc, [(fx - 4, top + 1), (fx + 4, top + 1), (fx, top + 8)], scheme::window());
	let lx = text_left + px(st.left, pct);
	triangle(dc, [(lx, top + 14), (lx - 4, top + 19), (lx + 4, top + 19)], scheme::window());
	fill(dc, lx - 4, top + 19, 9, 3, scheme::window());
	let rx = text_left + text_w - px(st.right, pct);
	triangle(dc, [(rx, top + 14), (rx - 4, top + 19), (rx + 4, top + 19)], scheme::window());
	// The button that picks the kind of tab stop a click on the scale makes.
	button_face(dc, 2, top + 2, 18, 18, Face::Raised, false);
	tab_mark(dc, kind, 8 + i32::from(kind == 2) * 3, top + 14);
}

/// What is at (`x`, `y`) on the ruler whose top is at `top`.
pub fn ruler_hit(x: i32, y: i32, top: i32, text_left: i32, text_w: i32, st: &RulerState, pct: i32) -> Option<Hit> {
	let y = y - top;
	if !(0..RULER_H).contains(&y) {
		return None;
	}
	if x < 22 {
		return Some(Hit::Kind);
	}
	let near = |a: i32, b: i32| (a - b).abs() <= 4;
	let (fx, lx, rx) = (
		text_left + px(st.first, pct),
		text_left + px(st.left, pct),
		text_left + text_w - px(st.right, pct),
	);
	if y < 9 && near(x, fx) {
		return Some(Hit::First);
	}
	if y >= 19 && near(x, lx) {
		return Some(Hit::Both);
	}
	if y >= 12 && near(x, lx) {
		return Some(Hit::Left);
	}
	if y >= 12 && near(x, rx) {
		return Some(Hit::Right);
	}
	for (pos, _) in st.tabs.iter().take(st.ntabs) {
		if near(x, text_left + px(*pos, pct)) && y >= 8 {
			return Some(Hit::Tab(*pos));
		}
	}
	if x >= text_left && x <= text_left + text_w && (8..15).contains(&y) {
		return Some(Hit::Scale(tw(x - text_left, pct)));
	}
	None
}

/// A position in twips, snapped to a sixteenth of an inch.
pub fn snap(twips: i32) -> i32 {
	(twips + 45).div_euclid(90) * 90
}

/// The state of a drag on the ruler.
#[derive(Copy, Clone)]
pub struct Drag {
	pub hit: Hit,
	pub state: RulerState,
	pub start: RulerState,
}

/// Applies the pointer position `x` to a drag, as a preview.
pub fn drag_to(d: &mut Drag, x: i32, text_left: i32, text_w: i32, pct: i32) {
	let wide = tw(text_w, pct);
	let t = snap(tw(x - text_left, pct)).clamp(0, wide);
	match d.hit {
		Hit::First => d.state.first = t,
		Hit::Left => d.state.left = t,
		Hit::Both => {
			let delta = t - d.start.left;
			d.state.left = t;
			d.state.first = (d.start.first + delta).max(0);
		}
		Hit::Right => d.state.right = snap(wide - tw(x - text_left, pct)).clamp(0, wide),
		_ => {}
	}
}
