// A PS/2 mouse's reports as pointer events: the position the reports add up to, clamped to the screen.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! With no INT 33h driver the shell reads the PS/2 mouse itself, and a PS/2 mouse reports motion and buttons, not a position. This
//! keeps the position: each report moves it by the counts, one count a pixel, and keeps it on the screen, and the buttons and the
//! wheel become the same [`Ev`]s the INT 33h reader makes. It knows nothing of ports or interrupts, so it runs on the host.

use crate::Ev;

/// What one packet of the mouse said, as the PS/2 driver decodes it.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Packet {
	/// Motion to the right, in counts.
	pub dx: i16,
	/// Motion down the screen, in counts.
	pub dy: i16,
	/// The left button is down now.
	pub left: bool,
	/// The wheel's turn; positive is towards the user.
	pub wheel: i8,
}

/// The pointer a PS/2 mouse moves.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Ps2Pointer {
	/// The screen in counts (pixels of the mouse's own), and how many make a shell unit.
	w: i32,
	h: i32,
	div: (i32, i32),
	/// The position in counts.
	pos: (i32, i32),
	left: bool,
	/// The position last reported, in shell units.
	unit: (i32, i32),
}

impl Ps2Pointer {
	/// A pointer on a screen of `w` by `h` shell units, in the middle of it. `div` is how many counts make a unit: 1 in graphics,
	/// 8 on the text screen, where a cell is eight pixels of the mouse's own.
	pub fn new(w: i32, h: i32, div: (i32, i32)) -> Self {
		let div = (div.0.max(1), div.1.max(1));
		let (w, h) = (w.max(1) * div.0, h.max(1) * div.1);
		let at = (w / 2, h / 2);
		Ps2Pointer {
			w,
			h,
			div,
			pos: at,
			left: false,
			unit: (at.0 / div.0, at.1 / div.1),
		}
	}

	/// Where the pointer is, in shell units.
	pub fn at(&self) -> (i32, i32) {
		self.unit
	}

	/// Takes one packet and calls `emit` for what it did, in order: the move, the press or release at the new place, the wheel.
	pub fn feed(&mut self, p: Packet, mut emit: impl FnMut(Ev)) {
		self.pos = (
			(self.pos.0 + i32::from(p.dx)).clamp(0, self.w - 1),
			(self.pos.1 + i32::from(p.dy)).clamp(0, self.h - 1),
		);
		let to = (self.pos.0 / self.div.0, self.pos.1 / self.div.1);
		if to != self.unit {
			self.unit = to;
			emit(Ev::Move { x: to.0, y: to.1 });
		}
		if p.left != self.left {
			self.left = p.left;
			let (x, y) = self.unit;
			emit(if p.left {
				Ev::Down { x, y }
			} else {
				Ev::Up { x, y }
			});
		}
		if p.wheel != 0 {
			emit(Ev::Wheel(i32::from(p.wheel)));
		}
	}
}

#[cfg(test)]
mod tests {
	extern crate std;

	use super::*;
	use std::vec;
	use std::vec::Vec;

	fn run(m: &mut Ps2Pointer, p: Packet) -> Vec<Ev> {
		let mut v = Vec::new();
		m.feed(p, |e| v.push(e));
		v
	}

	#[test]
	fn the_pointer_starts_in_the_middle_and_follows_the_counts() {
		let mut m = Ps2Pointer::new(640, 480, (1, 1));
		assert_eq!(m.at(), (320, 240));
		let e = run(
			&mut m,
			Packet {
				dx: 5,
				dy: -3,
				..Packet::default()
			},
		);
		assert_eq!(e, vec![Ev::Move { x: 325, y: 237 }]);
		assert_eq!(run(&mut m, Packet::default()), vec![]);
	}

	#[test]
	fn the_pointer_stays_on_the_screen() {
		let mut m = Ps2Pointer::new(640, 480, (1, 1));
		run(
			&mut m,
			Packet {
				dx: -2000,
				dy: -2000,
				..Packet::default()
			},
		);
		assert_eq!(m.at(), (0, 0));
		run(
			&mut m,
			Packet {
				dx: 2000,
				dy: 2000,
				..Packet::default()
			},
		);
		assert_eq!(m.at(), (639, 479));
		assert_eq!(
			run(
				&mut m,
				Packet {
					dx: 9,
					dy: 9,
					..Packet::default()
				}
			),
			vec![],
			"no move when it cannot"
		);
	}

	#[test]
	fn a_press_comes_after_the_move_that_came_with_it_and_a_release_ends_it() {
		let mut m = Ps2Pointer::new(640, 480, (1, 1));
		let e = run(
			&mut m,
			Packet {
				dx: 10,
				dy: 0,
				left: true,
				wheel: 0,
			},
		);
		assert_eq!(
			e,
			vec![Ev::Move { x: 330, y: 240 }, Ev::Down { x: 330, y: 240 }]
		);
		// Held: motion only, no second press.
		let e = run(
			&mut m,
			Packet {
				dx: 0,
				dy: 4,
				left: true,
				wheel: 0,
			},
		);
		assert_eq!(e, vec![Ev::Move { x: 330, y: 244 }]);
		let e = run(
			&mut m,
			Packet {
				dx: 0,
				dy: 0,
				left: false,
				wheel: 0,
			},
		);
		assert_eq!(e, vec![Ev::Up { x: 330, y: 244 }]);
	}

	#[test]
	fn the_wheel_is_reported_in_notches_with_its_sign() {
		let mut m = Ps2Pointer::new(640, 480, (1, 1));
		assert_eq!(
			run(
				&mut m,
				Packet {
					wheel: 1,
					..Packet::default()
				}
			),
			vec![Ev::Wheel(1)]
		);
		assert_eq!(
			run(
				&mut m,
				Packet {
					wheel: -2,
					..Packet::default()
				}
			),
			vec![Ev::Wheel(-2)]
		);
	}

	#[test]
	fn a_text_screen_in_cells_works_the_same() {
		let mut m = Ps2Pointer::new(80, 25, (8, 8));
		assert_eq!(m.at(), (40, 12));
		// Eight counts make a cell: a few counts move nothing a cell could show.
		assert_eq!(
			run(
				&mut m,
				Packet {
					dx: 3,
					dy: 3,
					..Packet::default()
				}
			),
			vec![]
		);
		assert_eq!(
			run(
				&mut m,
				Packet {
					dx: 5,
					dy: 0,
					..Packet::default()
				}
			),
			vec![Ev::Move { x: 41, y: 12 }]
		);
		run(
			&mut m,
			Packet {
				dx: 5000,
				dy: 5000,
				..Packet::default()
			},
		);
		assert_eq!(m.at(), (79, 24));
	}
}
