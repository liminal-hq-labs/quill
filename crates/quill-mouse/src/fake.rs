// A model of an INT 33h driver for host tests: position, range, button counters, hide count and the CuteMouse wheel.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

extern crate std;

use std::vec::Vec;

use crate::{func, Int33, Regs, WHEEL_API_SIGNATURE};

/// A driver in memory. The test moves it and presses its button; the code under test sees only INT 33h.
#[derive(Debug)]
pub struct FakeInt33 {
	/// False models a machine with no driver: every call fails.
	pub present: bool,
	/// True for a driver with the CuteMouse wheel API.
	pub wheel_api: bool,
	/// True when the mouse has a wheel: with the API but no wheel, CuteMouse answers 574Dh and CX = 0 (as it does on a PS/2 mouse it
	/// has not probed for a wheel).
	pub has_wheel: bool,
	/// The mickey ratio last set by function 000Fh, across and down per eight pixels (the driver's default is 8 and 16).
	pub mickeys: (u16, u16),
	/// Horizontal and vertical range as set by functions 0007h and 0008h.
	pub range: ((i32, i32), (i32, i32)),
	/// The driver's position, always inside the range.
	pub pos: (i32, i32),
	/// The hide counter (function 0002h decrements it; the cursor shows only at 0).
	pub hidden: i32,
	/// Left button state.
	pub left: bool,
	presses: u16,
	releases: u16,
	pressed_at: (i32, i32),
	released_at: (i32, i32),
	wheel: i32,
	/// Every function number called, in order.
	pub calls: Vec<u16>,
}

impl Default for FakeInt33 {
	fn default() -> Self {
		FakeInt33 {
			present: true,
			wheel_api: true,
			has_wheel: true,
			mickeys: (8, 16),
			range: ((0, 639), (0, 199)),
			pos: (320, 100),
			hidden: 1,
			left: false,
			presses: 0,
			releases: 0,
			pressed_at: (0, 0),
			released_at: (0, 0),
			wheel: 0,
			calls: Vec::new(),
		}
	}
}

impl FakeInt33 {
	/// A driver that is not there.
	pub fn absent() -> Self {
		FakeInt33 {
			present: false,
			..FakeInt33::default()
		}
	}

	/// Moves the pointer, as the driver would clamp it to its range.
	pub fn move_to(&mut self, x: i32, y: i32) {
		self.pos = (
			x.clamp(self.range.0 .0, self.range.0 .1),
			y.clamp(self.range.1 .0, self.range.1 .1),
		);
	}

	/// Presses the left button at the current position.
	pub fn press(&mut self) {
		self.left = true;
		self.presses = self.presses.wrapping_add(1);
		self.pressed_at = self.pos;
	}

	/// Releases the left button at the current position.
	pub fn release(&mut self) {
		self.left = false;
		self.releases = self.releases.wrapping_add(1);
		self.released_at = self.pos;
	}

	/// Turns the wheel; positive is towards the user.
	pub fn turn(&mut self, notches: i32) {
		self.wheel += notches;
	}
}

impl Int33 for FakeInt33 {
	fn call(&mut self, r: &mut Regs) -> bool {
		if !self.present {
			return false;
		}
		self.calls.push(r.ax);
		match r.ax {
			func::RESET => {
				r.ax = 0xFFFF;
				r.bx = 2;
				self.hidden = 1;
			}
			func::HIDE => self.hidden += 1,
			func::POSITION => {
				let w = (self.wheel.clamp(-128, 127) as i8 as u8 as u16) << 8;
				self.wheel = 0;
				r.bx = u16::from(self.left)
					| if self.wheel_api && self.has_wheel {
						w
					} else {
						0
					};
				r.cx = self.pos.0 as u16;
				r.dx = self.pos.1 as u16;
			}
			func::SET_POSITION => self.move_to(i32::from(r.cx as i16), i32::from(r.dx as i16)),
			func::PRESSES | func::RELEASES => {
				// Only the left button (BX = 0) is modelled.
				let (n, at) = if r.ax == func::PRESSES {
					(core::mem::take(&mut self.presses), self.pressed_at)
				} else {
					(core::mem::take(&mut self.releases), self.released_at)
				};
				r.ax = u16::from(self.left);
				r.bx = n;
				r.cx = at.0 as u16;
				r.dx = at.1 as u16;
			}
			func::RANGE_X => {
				let (a, b) = (i32::from(r.cx as i16), i32::from(r.dx as i16));
				self.range.0 = (a.min(b), a.max(b));
				self.move_to(self.pos.0, self.pos.1);
			}
			func::MICKEYS => self.mickeys = (r.cx, r.dx),
			func::RANGE_Y => {
				let (a, b) = (i32::from(r.cx as i16), i32::from(r.dx as i16));
				self.range.1 = (a.min(b), a.max(b));
				self.move_to(self.pos.0, self.pos.1);
			}
			func::WHEEL_API if self.wheel_api => {
				r.ax = WHEEL_API_SIGNATURE;
				r.cx = u16::from(self.has_wheel);
			}
			_ => {}
		}
		true
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{Ev, Mouse};
	use std::vec;

	fn events(m: &mut Mouse<FakeInt33>) -> Vec<Ev> {
		let mut v = Vec::new();
		m.poll(|e| v.push(e));
		v
	}

	#[test]
	fn no_driver_means_no_mouse() {
		assert!(Mouse::open(FakeInt33::absent(), 640, 480, (1, 1)).is_none());
	}

	#[test]
	fn open_sets_the_range_to_the_mode_and_hides_the_drivers_cursor() {
		let mut m = Mouse::open(FakeInt33::default(), 640, 480, (1, 1)).unwrap();
		let d = m.driver();
		assert_eq!(d.range, ((0, 639), (0, 479)));
		assert_eq!(d.pos, (320, 240));
		assert_eq!(
			d.hidden, 2,
			"reset leaves it hidden once and the hide call adds one"
		);
		assert_eq!(d.mickeys, (8, 8), "as far down as across");
		for f in [
			func::RESET,
			func::MICKEYS,
			func::RANGE_X,
			func::RANGE_Y,
			func::HIDE,
			func::SET_POSITION,
		] {
			assert!(d.calls.contains(&f), "function {f:04x}");
		}
	}

	#[test]
	fn text_mode_ranges_are_eight_driver_units_a_cell() {
		let mut m = Mouse::open(FakeInt33::default(), 80, 25, (8, 8)).unwrap();
		assert_eq!(m.driver().range, ((0, 639), (0, 199)));
		m.driver().move_to(17, 9);
		assert_eq!(events(&mut m), vec![Ev::Move { x: 2, y: 1 }]);
	}

	#[test]
	fn a_press_and_release_across_polls() {
		let mut m = Mouse::open(FakeInt33::default(), 640, 480, (1, 1)).unwrap();
		m.driver().move_to(100, 50);
		m.driver().press();
		assert_eq!(
			events(&mut m),
			vec![Ev::Move { x: 100, y: 50 }, Ev::Down { x: 100, y: 50 }]
		);
		m.driver().move_to(120, 50);
		assert_eq!(events(&mut m), vec![Ev::Move { x: 120, y: 50 }]);
		m.driver().release();
		assert_eq!(events(&mut m), vec![Ev::Up { x: 120, y: 50 }]);
		assert_eq!(events(&mut m), vec![]);
	}

	#[test]
	fn a_click_shorter_than_a_poll_is_not_lost() {
		let mut m = Mouse::open(FakeInt33::default(), 640, 480, (1, 1)).unwrap();
		m.driver().move_to(10, 20);
		m.driver().press();
		m.driver().release();
		assert_eq!(
			events(&mut m),
			vec![
				Ev::Move { x: 10, y: 20 },
				Ev::Down { x: 10, y: 20 },
				Ev::Up { x: 10, y: 20 }
			]
		);
	}

	#[test]
	fn two_clicks_in_one_poll_are_two_clicks() {
		let mut m = Mouse::open(FakeInt33::default(), 640, 480, (1, 1)).unwrap();
		m.driver().move_to(10, 20);
		for _ in 0..2 {
			m.driver().press();
			m.driver().release();
		}
		let down = Ev::Down { x: 10, y: 20 };
		let up = Ev::Up { x: 10, y: 20 };
		assert_eq!(
			events(&mut m),
			vec![Ev::Move { x: 10, y: 20 }, down, up, down, up]
		);
		// A press that is still held after a quick click: the click, then the press.
		m.driver().press();
		m.driver().release();
		m.driver().press();
		assert_eq!(events(&mut m), vec![down, up, down]);
	}

	#[test]
	fn the_press_position_is_where_the_pointer_was_then_not_where_it_went() {
		let mut m = Mouse::open(FakeInt33::default(), 640, 480, (1, 1)).unwrap();
		m.driver().move_to(10, 20);
		m.driver().press();
		m.driver().move_to(300, 400);
		let e = events(&mut m);
		assert_eq!(e[1], Ev::Down { x: 10, y: 20 });
		assert_eq!(e.last(), Some(&Ev::Move { x: 300, y: 400 }));
	}

	#[test]
	fn the_wheel_comes_through_the_cutemouse_api_only() {
		let mut m = Mouse::open(FakeInt33::default(), 640, 480, (1, 1)).unwrap();
		assert!(m.has_wheel());
		m.driver().turn(-2);
		assert_eq!(events(&mut m), vec![Ev::Wheel(-2)]);
		let mut plain = Mouse::open(
			FakeInt33 {
				wheel_api: false,
				..FakeInt33::default()
			},
			640,
			480,
			(1, 1),
		)
		.unwrap();
		assert!(!plain.has_wheel());
		plain.driver().turn(3);
		assert_eq!(events(&mut plain), vec![]);
	}

	#[test]
	fn a_driver_with_the_api_and_no_wheel_has_no_wheel() {
		// CuteMouse on a PS/2 mouse it was not told to probe: 574Dh, CX = 0, and the counter in BH stays 0.
		let mut m = Mouse::open(
			FakeInt33 {
				has_wheel: false,
				..FakeInt33::default()
			},
			640,
			480,
			(1, 1),
		)
		.unwrap();
		assert!(!m.has_wheel());
		m.driver().turn(2);
		assert_eq!(events(&mut m), vec![]);
	}

	#[test]
	fn a_click_before_the_program_started_is_not_a_click() {
		let mut d = FakeInt33::default();
		d.press();
		d.release();
		let mut m = Mouse::open(d, 640, 480, (1, 1)).unwrap();
		assert_eq!(events(&mut m), vec![]);
	}

	#[test]
	fn the_pointer_stays_inside_the_mode() {
		let mut m = Mouse::open(FakeInt33::default(), 640, 480, (1, 1)).unwrap();
		m.driver().move_to(5000, -4);
		assert_eq!(events(&mut m), vec![Ev::Move { x: 639, y: 0 }]);
	}
}
