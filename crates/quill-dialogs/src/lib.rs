// Quill's dialogs as data: what each dialog holds, where, and under which control ID.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `assets/dialogs/dialogs.json` is the one description of the dialogs. The Win16 and Win32 packers read it to build the resources;
//! `emit-resources --dialogs` turns it into [`generated`], so a shell, the text-cell dialogs and the tests all name a control by the
//! same constant and read its label, mnemonic and geometry from the same place. Geometry is in dialog units, as the packers read it.
//!
//! Each dialog is a module (`find`) holding its [`Dialog`] description (`find::DIALOG`), its ID (`find::ID`) and one constant per
//! named control (`find::WHAT`). [`ALL`] lists them in resource order.

#![no_std]
#![deny(
	clippy::unwrap_used,
	clippy::expect_used,
	clippy::panic,
	clippy::indexing_slicing
)]

#[rustfmt::skip]
mod generated;

pub use generated::*;

/// The kind of control, as the resource packers name it.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Class {
	/// A label, an icon or a frame of text.
	Static,
	/// A push button, check box, radio button or group box.
	Button,
	/// A single-line edit field.
	Edit,
	/// A list box.
	ListBox,
	/// A combo box.
	ComboBox,
	/// A scroll bar.
	ScrollBar,
}

/// Style bits, one for each style name the dialogs use. They are this crate's own and are not Windows' `WS_` or `BS_` values.
pub mod style {
	/// A push button.
	pub const PUSHBUTTON: u16 = 1 << 0;
	/// The default push button, which Enter presses.
	pub const DEFPUSHBUTTON: u16 = 1 << 1;
	/// A check box that toggles itself.
	pub const AUTOCHECKBOX: u16 = 1 << 2;
	/// A radio button that clears the others of its group.
	pub const AUTORADIOBUTTON: u16 = 1 << 3;
	/// A frame with a caption around other controls.
	pub const GROUPBOX: u16 = 1 << 4;
	/// A static control that shows an icon.
	pub const ICON: u16 = 1 << 5;
	/// Tab reaches the control.
	pub const TABSTOP: u16 = 1 << 6;
	/// The control starts a group of radio buttons or a run of arrow-key movement.
	pub const GROUP: u16 = 1 << 7;
	/// A border.
	pub const BORDER: u16 = 1 << 8;
	/// A vertical scroll bar.
	pub const VSCROLL: u16 = 1 << 9;
	/// An edit field that scrolls sideways.
	pub const AUTOHSCROLL: u16 = 1 << 10;
	/// A list box that tells its dialog about selection changes.
	pub const NOTIFY: u16 = 1 << 11;
	/// A combo box with an edit field.
	pub const DROPDOWN: u16 = 1 << 12;
	/// A combo box that only chooses.
	pub const DROPDOWNLIST: u16 = 1 << 13;
}

/// One control of a dialog.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Item {
	/// The kind of control.
	pub class: Class,
	/// The control ID. 1 is the OK button and 2 Cancel or Close, as Windows expects.
	pub id: u16,
	/// The label, with `&` before the mnemonic letter. Empty when the program sets the text.
	pub text: &'static str,
	/// The icon resource ID of an icon control, or 0.
	pub icon: u16,
	/// Left edge, in dialog units.
	pub x: i16,
	/// Top edge, in dialog units.
	pub y: i16,
	/// Width, in dialog units.
	pub cx: i16,
	/// Height, in dialog units.
	pub cy: i16,
	/// [`style`] bits.
	pub style: u16,
}

impl Item {
	/// True when every bit of `bits` is in the style.
	pub const fn has(&self, bits: u16) -> bool {
		self.style & bits == bits
	}

	/// The mnemonic letter of the label in lower case: the byte after the first single `&`.
	pub fn mnemonic(&self) -> Option<u8> {
		let t = self.text.as_bytes();
		let mut i = 0;
		while let Some(&b) = t.get(i) {
			if b == b'&' {
				match t.get(i + 1) {
					Some(b'&') => i += 2,
					Some(&c) => return Some(c.to_ascii_lowercase()),
					None => return None,
				}
			} else {
				i += 1;
			}
		}
		None
	}
}

/// One dialog.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Dialog {
	/// The dialog's resource ID.
	pub id: u16,
	/// The name its module and constants carry.
	pub name: &'static str,
	/// The caption.
	pub caption: &'static str,
	/// Left edge, in dialog units (0 centres the dialog on its owner).
	pub x: i16,
	/// Top edge, in dialog units.
	pub y: i16,
	/// Width, in dialog units.
	pub cx: i16,
	/// Height, in dialog units.
	pub cy: i16,
	/// The ID of the default push button, or 0.
	pub default_button: u16,
	/// The controls, in tab and draw order.
	pub items: &'static [Item],
}

impl Dialog {
	/// The control with `id`.
	pub fn item(&self, id: u16) -> Option<&'static Item> {
		self.items.iter().find(|i| i.id == id)
	}

	/// The control whose label has `letter` as its mnemonic (case ignored), which Alt and the letter reaches.
	pub fn item_for_mnemonic(&self, letter: u8) -> Option<&'static Item> {
		let l = letter.to_ascii_lowercase();
		self.items.iter().find(|i| i.mnemonic() == Some(l))
	}
}

/// The dialog with resource ID `id`.
pub fn by_id(id: u16) -> Option<&'static Dialog> {
	ALL.iter().copied().find(|d| d.id == id)
}
