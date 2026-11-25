// Tooltips: the name of a toolbar button or box and its shortcut, in a small box under the pointer's target.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The words are `quill_app`'s tip tables ([`command_tip`], [`combo_tip`], [`COLOUR_TIP`]), shared with the Windows shells; the
//! shortcut is the accelerator the command's menu item shows. [`text`] and [`rect`] are pure.

use quill_app::view::{Act, TOOLBAR};
use quill_app::{combo_tip, command_tip, menu_items_for, Command, COLOUR_TIP, MENU_COUNT};

use crate::layout::Layout;
use crate::shell::{SCREEN_H, SCREEN_W};
use crate::toolbar::{box_rect, button_rect, Box3};
use crate::ui::{Ui, INK};
use crate::{Rect, Rgb, Surface};

/// Ticks the pointer rests on a button before its tip shows: half a second at 18.2 ticks a second.
pub const TIP_TICKS: u32 = 9;
/// The longest tip text.
pub const TIP_MAX: usize = 48;
/// The fill of a tip: the pale yellow of a tooltip (white on a 16-colour screen).
pub const TIP_FILL: Rgb = Rgb(255, 255, 225);

/// What a tip is for.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Target {
	/// A toolbar button, as an index into `TOOLBAR.buttons`.
	Button(usize),
	/// One of the formatting bar's boxes.
	Box(Box3),
}

/// The accelerator the menus show for `c` (`Ctrl+B`), if any.
pub fn shortcut(c: Command) -> &'static str {
	for m in 0..MENU_COUNT {
		for it in menu_items_for(m, true) {
			if it.command == c && !it.accel.is_empty() {
				return it.accel;
			}
		}
	}
	""
}

/// The tip's text: the name, and the shortcut after it in brackets when the command has one. The length.
pub fn text(t: Target, out: &mut [u8; TIP_MAX]) -> usize {
	let (name, key) = match t {
		Target::Button(i) => match TOOLBAR.buttons.get(i).map(|b| b.act) {
			Some(Act::Cmd(c)) => (command_tip(c), shortcut(c)),
			Some(Act::Colour) => (COLOUR_TIP, ""),
			None => ("", ""),
		},
		Target::Box(b) => (
			combo_tip(match b {
				Box3::Style => 0,
				Box3::Face => 1,
				Box3::Size => 2,
			}),
			"",
		),
	};
	let mut n = 0;
	let mut put = |s: &str| {
		for c in s.bytes() {
			if let Some(d) = out.get_mut(n) {
				*d = c;
				n += 1;
			}
		}
	};
	put(name);
	if !key.is_empty() {
		put(" (");
		put(key);
		put(")");
	}
	n
}

/// The rectangle a target's tip points at, or `None` when its bar is hidden.
pub fn anchor(lay: &Layout, t: Target) -> Option<Rect> {
	match t {
		Target::Button(i) => button_rect(lay, i),
		Target::Box(b) => box_rect(lay, b),
	}
}

/// Where a tip `w` by `h` goes: under its target, left edges aligned, moved back inside the screen.
pub fn rect(anchor: Rect, w: i32, h: i32) -> Rect {
	let left = anchor.left.min(SCREEN_W - w).max(0);
	let top = (anchor.bottom + 2).min(SCREEN_H - h).max(0);
	Rect::new(left, top, left + w, top + h)
}

/// Draws the tip of `t`.
pub fn draw<Sf: Surface + ?Sized>(sf: &mut Sf, ui: &Ui<Sf::Font>, lay: &Layout, t: Target) {
	let Some(a) = anchor(lay, t) else { return };
	let mut buf = [0u8; TIP_MAX];
	let n = text(t, &mut buf);
	let words = buf.get(..n).unwrap_or(&[]);
	let w = ui.width(sf, words) + 8;
	let r = rect(a, w, ui.height + 4);
	sf.clip(None);
	sf.fill_rect(r, TIP_FILL);
	crate::ui::outline(sf, r, INK);
	ui.text(sf, r.left + 4, r.top + 2, words, INK);
}
