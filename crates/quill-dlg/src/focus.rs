// The focus ring and mnemonics: which control has the keyboard, Tab and Shift+Tab, Alt+letter, and the default button.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::key::DlgKey;

/// What a control is, which decides how Enter and an Alt mnemonic treat it.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Control {
	Field,
	List,
	Combo,
	Check,
	Radio,
	Button,
}

/// The mnemonic of a label such as `File &name`: the upper-cased letter after the first single `&`. `&&` is a literal `&`.
pub fn mnemonic(label: &[u8]) -> Option<u8> {
	let mut i = 0;
	while let Some(c) = label.get(i) {
		if *c == b'&' {
			match label.get(i + 1) {
				Some(b'&') => i += 1,
				Some(n) if n.is_ascii_alphanumeric() => return Some(n.to_ascii_uppercase()),
				_ => {}
			}
		}
		i += 1;
	}
	None
}

/// Copies `label` into `out` without its `&` marks, and returns the length and the index in `out` of the mnemonic letter, which a
/// painter shows in the mnemonic colour. `None` when `out` is too small.
pub fn strip_label(label: &[u8], out: &mut [u8]) -> Option<(usize, Option<usize>)> {
	let (mut n, mut hot) = (0, None);
	let mut i = 0;
	while let Some(c) = label.get(i) {
		let mut c = *c;
		if c == b'&' {
			match label.get(i + 1) {
				Some(b'&') => i += 1,
				Some(next) if hot.is_none() && next.is_ascii_alphanumeric() => {
					hot = Some(n);
					i += 1;
					c = *next;
				}
				_ => {}
			}
		}
		*out.get_mut(n)? = c;
		n += 1;
		i += 1;
	}
	Some((n, hot))
}

/// What a key did to the focus.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Nav {
	/// Not the focus ring's: give the key to the focused control
	Unhandled,
	/// The focus moved to this control
	Moved(u8),
	/// This button was pressed (or this check box or radio group was reached by its mnemonic and is to be toggled)
	Press(u8),
	/// Enter on a control that is not a button, and the default button is not available: the caller decides
	Enter(u8),
	/// Esc
	Cancel,
}

/// The controls of a dialog (up to `N`, at most 16), in Tab order, with the one that has the focus.
#[derive(Copy, Clone, Debug)]
pub struct Focus<const N: usize> {
	kinds: [Control; N],
	hot: [u8; N],
	enabled: u16,
	count: u8,
	focus: u8,
	default: Option<u8>,
}

impl<const N: usize> Default for Focus<N> {
	fn default() -> Self {
		Focus::new()
	}
}

impl<const N: usize> Focus<N> {
	pub const fn new() -> Self {
		const { assert!(N <= 16) };
		Focus {
			kinds: [Control::Field; N],
			hot: [0; N],
			enabled: 0,
			count: 0,
			focus: 0,
			default: None,
		}
	}

	/// Adds a control at the end of the Tab order, enabled. `label` carries its `&` mnemonic. Returns its number, or `None` when full.
	pub fn add(&mut self, kind: Control, label: &[u8]) -> Option<u8> {
		let id = usize::from(self.count);
		*self.kinds.get_mut(id)? = kind;
		*self.hot.get_mut(id)? = mnemonic(label).unwrap_or(0);
		self.enabled |= 1 << id;
		self.count += 1;
		Some(id as u8)
	}

	pub fn count(&self) -> usize {
		usize::from(self.count)
	}

	/// The control with the focus. Meaningless when there are none.
	pub fn focused(&self) -> u8 {
		self.focus
	}

	pub fn kind(&self, id: u8) -> Option<Control> {
		(id < self.count)
			.then(|| self.kinds.get(usize::from(id)).copied())
			.flatten()
	}

	pub fn is_enabled(&self, id: u8) -> bool {
		id < self.count && self.enabled & (1 << id) != 0
	}

	/// Enables or disables a control. A disabled control is skipped by Tab and mnemonics; if it had the focus, the focus moves on.
	pub fn set_enabled(&mut self, id: u8, on: bool) {
		if id >= self.count {
			return;
		}
		if on {
			self.enabled |= 1 << id;
		} else {
			self.enabled &= !(1 << id);
			if self.focus == id {
				self.step(true);
			}
		}
	}

	/// Sets the default button, which Enter presses.
	pub fn set_default(&mut self, id: Option<u8>) {
		self.default = id.filter(|i| *i < self.count);
	}

	pub fn default_button(&self) -> Option<u8> {
		self.default
	}

	/// Gives the focus to `id` if it exists and is enabled.
	pub fn set_focus(&mut self, id: u8) -> bool {
		if self.is_enabled(id) {
			self.focus = id;
			true
		} else {
			false
		}
	}

	/// Moves to the next (or previous) enabled control, wrapping. Stays put when no other is enabled.
	fn step(&mut self, forward: bool) -> u8 {
		let n = self.count;
		for k in 1..=n {
			let i = if forward {
				(self.focus + k) % n
			} else {
				(self.focus + n - k % n) % n
			};
			if self.is_enabled(i) {
				self.focus = i;
				break;
			}
		}
		self.focus
	}

	/// The enabled control whose mnemonic is `c` (any case).
	pub fn find_mnemonic(&self, c: u8) -> Option<u8> {
		let c = c.to_ascii_uppercase();
		(0..self.count)
			.find(|i| self.is_enabled(*i) && self.hot.get(usize::from(*i)) == Some(&c) && c != 0)
	}

	/// Handles Tab, Shift+Tab, Alt+letter, Enter and Esc, and Space on a button. Everything else is `Unhandled`.
	pub fn key(&mut self, k: DlgKey) -> Nav {
		if self.count == 0 {
			return if k == DlgKey::Esc {
				Nav::Cancel
			} else {
				Nav::Unhandled
			};
		}
		let focused_button = self.kind(self.focus) == Some(Control::Button);
		match k {
			DlgKey::Tab => Nav::Moved(self.step(true)),
			DlgKey::BackTab => Nav::Moved(self.step(false)),
			DlgKey::Esc => Nav::Cancel,
			DlgKey::Alt(c) => match self.find_mnemonic(c) {
				Some(id) => {
					self.focus = id;
					match self.kind(id) {
						Some(Control::Button) | Some(Control::Check) => Nav::Press(id),
						_ => Nav::Moved(id),
					}
				}
				None => Nav::Unhandled,
			},
			DlgKey::Enter if focused_button => Nav::Press(self.focus),
			DlgKey::Enter => match self.default.filter(|d| self.is_enabled(*d)) {
				Some(d) => Nav::Press(d),
				None => Nav::Enter(self.focus),
			},
			DlgKey::Char(b' ') if focused_button => Nav::Press(self.focus),
			_ => Nav::Unhandled,
		}
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
	use crate::key::tests::any_key;
	use proptest::prelude::*;
	use std::vec::Vec;

	/// The file dialog's controls: name, files, directories, type, drives, OK and Cancel.
	fn file_dialog() -> Focus<8> {
		let mut f = Focus::<8>::new();
		for (k, l) in [
			(Control::Field, &b"File &name:"[..]),
			(Control::List, b"Fi&les:"),
			(Control::List, b"&Directories:"),
			(Control::Combo, b"Save file as &type:"),
			(Control::Combo, b"Dri&ves:"),
			(Control::Button, b"OK"),
			(Control::Button, b"Cancel"),
		] {
			f.add(k, l).unwrap();
		}
		f.set_default(Some(5));
		f
	}

	#[test]
	fn labels_give_their_mnemonic() {
		assert_eq!(mnemonic(b"File &name"), Some(b'N'));
		assert_eq!(mnemonic(b"&Save"), Some(b'S'));
		assert_eq!(mnemonic(b"Save && Exit"), None);
		assert_eq!(mnemonic(b"A && &B"), Some(b'B'));
		assert_eq!(mnemonic(b"trailing &"), None);
		assert_eq!(mnemonic(b"& x"), None);
		assert_eq!(mnemonic(b"&1st"), Some(b'1'));
		assert_eq!(mnemonic(b""), None);
	}

	#[test]
	fn labels_are_stripped_and_the_letter_found() {
		let mut out = [0u8; 16];
		assert_eq!(strip_label(b"File &name", &mut out), Some((9, Some(5))));
		assert_eq!(&out[..9], b"File name");
		assert_eq!(strip_label(b"A && &B", &mut out), Some((5, Some(4))));
		assert_eq!(&out[..5], b"A & B");
		assert_eq!(strip_label(b"Plain", &mut out), Some((5, None)));
		assert_eq!(strip_label(b"toolongforthebuffer", &mut out[..4]), None);
	}

	#[test]
	fn tab_walks_the_ring_and_back_tab_returns() {
		let mut f = file_dialog();
		let mut seen = Vec::new();
		for _ in 0..7 {
			match f.key(DlgKey::Tab) {
				Nav::Moved(id) => seen.push(id),
				other => panic!("{other:?}"),
			}
		}
		assert_eq!(seen, [1, 2, 3, 4, 5, 6, 0]);
		assert_eq!(f.key(DlgKey::BackTab), Nav::Moved(6));
		assert_eq!(f.key(DlgKey::BackTab), Nav::Moved(5));
	}

	#[test]
	fn disabled_controls_are_skipped() {
		let mut f = file_dialog();
		f.set_enabled(1, false);
		assert_eq!(f.key(DlgKey::Tab), Nav::Moved(2));
		f.set_enabled(2, false);
		assert_eq!(f.focused(), 3);
		assert_eq!(f.key(DlgKey::BackTab), Nav::Moved(0));
		assert!(!f.set_focus(1));
		assert_eq!(f.find_mnemonic(b'n'), Some(0));
		assert_eq!(f.find_mnemonic(b'l'), None);
		for id in 0..7 {
			f.set_enabled(id, false);
		}
		assert_eq!(f.key(DlgKey::Tab), Nav::Moved(f.focused()));
	}

	#[test]
	fn mnemonics_jump_or_press() {
		let mut f = file_dialog();
		assert_eq!(f.key(DlgKey::Alt(b'd')), Nav::Moved(2));
		assert_eq!(f.focused(), 2);
		assert_eq!(f.key(DlgKey::Alt(b'T')), Nav::Moved(3));
		assert_eq!(f.key(DlgKey::Alt(b'q')), Nav::Unhandled);
		let mut g = Focus::<4>::new();
		g.add(Control::Check, b"Match &case").unwrap();
		g.add(Control::Button, b"&Find Next").unwrap();
		assert_eq!(g.key(DlgKey::Alt(b'c')), Nav::Press(0));
		assert_eq!(g.key(DlgKey::Alt(b'F')), Nav::Press(1));
		assert_eq!(g.focused(), 1);
	}

	#[test]
	fn enter_presses_the_default_unless_a_button_has_focus() {
		let mut f = file_dialog();
		assert_eq!(f.key(DlgKey::Enter), Nav::Press(5));
		f.set_focus(6);
		assert_eq!(f.key(DlgKey::Enter), Nav::Press(6));
		assert_eq!(f.key(DlgKey::Char(b' ')), Nav::Press(6));
		f.set_focus(2);
		assert_eq!(f.key(DlgKey::Char(b' ')), Nav::Unhandled);
		f.set_enabled(5, false);
		assert_eq!(f.key(DlgKey::Enter), Nav::Enter(2));
		assert_eq!(f.key(DlgKey::Esc), Nav::Cancel);
		assert_eq!(f.key(DlgKey::Up), Nav::Unhandled);
		let mut none = Focus::<2>::new();
		assert_eq!(none.key(DlgKey::Tab), Nav::Unhandled);
		assert_eq!(none.key(DlgKey::Esc), Nav::Cancel);
	}

	#[test]
	fn a_full_ring_refuses_more() {
		let mut f = Focus::<2>::new();
		assert!(f.add(Control::Field, b"").is_some());
		assert!(f.add(Control::Field, b"").is_some());
		assert!(f.add(Control::Field, b"").is_none());
		assert_eq!(f.count(), 2);
		assert_eq!(f.kind(1), Some(Control::Field));
		assert_eq!(f.kind(2), None);
	}

	proptest! {
		#[test]
		fn focus_always_rests_on_an_enabled_control(
			mask in 1u8..=0x7F,
			ops in proptest::collection::vec(prop_oneof![
				any_key().prop_map(|k| (0u8, k, 0u8, false)),
				(0u8..8, any_key(), 0u8..8, any::<bool>()).prop_map(|(_, k, id, on)| (1u8, k, id, on)),
			], 0..80),
		) {
			let mut f = file_dialog();
			for id in 0..7u8 {
				f.set_enabled(id, mask & (1 << id) != 0);
			}
			for (kind, k, id, on) in ops {
				if kind == 1 {
					// Never disable the last enabled control: the dialog always has one.
					let others = (0..7u8).filter(|i| *i != id && f.is_enabled(*i)).count();
					if on || others > 0 {
						f.set_enabled(id, on);
					}
				}
				let nav = f.key(k);
				prop_assert!(f.is_enabled(f.focused()));
				match nav {
					Nav::Moved(i) | Nav::Press(i) | Nav::Enter(i) => prop_assert!(f.is_enabled(i)),
					_ => {}
				}
			}
		}

		#[test]
		fn tab_visits_every_enabled_control_once_and_back_tab_undoes_it(mask in 1u8..=0x7F) {
			let mut f = file_dialog();
			for id in 0..7u8 {
				f.set_enabled(id, mask & (1 << id) != 0);
			}
			let start = f.focused();
			let enabled = (0..7u8).filter(|i| f.is_enabled(*i)).count();
			let mut seen = Vec::new();
			for _ in 0..enabled {
				f.key(DlgKey::Tab);
				seen.push(f.focused());
			}
			prop_assert_eq!(f.focused(), start);
			seen.sort_unstable();
			seen.dedup();
			prop_assert_eq!(seen.len(), enabled);
			f.key(DlgKey::Tab);
			f.key(DlgKey::BackTab);
			prop_assert_eq!(f.focused(), start);
		}

		#[test]
		fn mnemonics_never_panic(label in proptest::collection::vec(any::<u8>(), 0..20), c in any::<u8>()) {
			let m = mnemonic(&label);
			let mut out = [0u8; 24];
			if let Some((n, hot)) = strip_label(&label, &mut out) {
				prop_assert!(n <= label.len());
				if let Some(h) = hot {
					prop_assert!(h < n);
					prop_assert_eq!(Some(out[h].to_ascii_uppercase()), m);
				}
			}
			let mut f = Focus::<2>::new();
			f.add(Control::Button, &label);
			let _ = f.find_mnemonic(c);
		}
	}
}
