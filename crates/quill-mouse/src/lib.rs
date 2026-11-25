// The pointer events every shell hands the editor, and the INT 33h mouse driver model for DOS.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A shell turns whatever its pointer gives it into [`Ev`]s in its own units (text cells or pixels). Under DOS the pointer is the
//! mouse driver behind INT 33h; [`Mouse`] reads it through the [`Int33`] trait, so the same code runs against the real driver in
//! `QUILLDOS.EXE` and against [`fake::FakeInt33`] in host tests.
//!
//! The driver is polled, and a click can start and end between two polls. Functions 0005h and 0006h count presses and releases, so
//! a poll that sees the button up before and after, with a press counted, reports the press and the release it would have missed.
#![no_std]

#[cfg(any(test, feature = "fake"))]
pub mod fake;

/// A pointer event, in the shell's units. Only the left button is reported; the others are ignored.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Ev {
	/// The left button went down at (x, y).
	Down { x: i32, y: i32 },
	/// The pointer moved to (x, y), with the button down or up.
	Move { x: i32, y: i32 },
	/// The left button went up at (x, y).
	Up { x: i32, y: i32 },
	/// The wheel turned by this many notches; positive is towards the user, which scrolls the page down.
	Wheel(i32),
}

/// The registers an INT 33h call takes and gives.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Regs {
	pub ax: u16,
	pub bx: u16,
	pub cx: u16,
	pub dx: u16,
}

/// Function numbers (the value of AX on entry).
pub mod func {
	pub const RESET: u16 = 0x0000;
	pub const HIDE: u16 = 0x0002;
	pub const POSITION: u16 = 0x0003;
	pub const SET_POSITION: u16 = 0x0004;
	pub const PRESSES: u16 = 0x0005;
	pub const RELEASES: u16 = 0x0006;
	pub const RANGE_X: u16 = 0x0007;
	pub const RANGE_Y: u16 = 0x0008;
	/// The CuteMouse wheel API: AX = 574Dh when the driver reports a wheel (the counter in BH of function 0003h stays 0 without one).
	pub const WHEEL_API: u16 = 0x0011;
}

/// The answer of `func::WHEEL_API` from a driver that implements the wheel API.
pub const WHEEL_API_SIGNATURE: u16 = 0x574D;

/// One call into the mouse driver.
pub trait Int33 {
	/// Runs INT 33h with `r`. False when there is no driver to answer (an empty vector, or one that points at an `IRET`).
	fn call(&mut self, r: &mut Regs) -> bool;
}

/// The mouse behind an [`Int33`]: reset, ranges, and the events a poll finds.
pub struct Mouse<D: Int33> {
	d: D,
	wheel: bool,
	/// Driver units per shell unit: 8 in text mode (a cell is eight of the driver's virtual pixels), 1 in graphics.
	div: (i32, i32),
	left: bool,
	at: (i32, i32),
}

impl<D: Int33> Mouse<D> {
	/// Finds the driver, resets it, limits it to a screen of `w` by `h` shell units (functions 0007h and 0008h, because a driver does
	/// not know a VBE mode), hides its own cursor, and puts the pointer in the middle. `div` is how many driver units make one shell
	/// unit. `None` when there is no driver: nothing then breaks, the shell simply has no pointer.
	pub fn open(mut d: D, w: i32, h: i32, div: (i32, i32)) -> Option<Mouse<D>> {
		let mut r = Regs::default();
		if !d.call(&mut r) || r.ax != 0xFFFF {
			return None;
		}
		let mut r = Regs {
			ax: func::WHEEL_API,
			..Regs::default()
		};
		let wheel = d.call(&mut r) && r.ax == WHEEL_API_SIGNATURE;
		let div = (div.0.max(1), div.1.max(1));
		let mut set = |ax: u16, lo: i32, hi: i32| {
			let mut r = Regs {
				ax,
				cx: lo as u16,
				dx: hi as u16,
				..Regs::default()
			};
			d.call(&mut r);
		};
		set(func::RANGE_X, 0, w * div.0 - 1);
		set(func::RANGE_Y, 0, h * div.1 - 1);
		let mid = (w / 2, h / 2);
		let mut r = Regs {
			ax: func::SET_POSITION,
			cx: (mid.0 * div.0) as u16,
			dx: (mid.1 * div.1) as u16,
			..Regs::default()
		};
		d.call(&mut r);
		let mut r = Regs {
			ax: func::HIDE,
			..Regs::default()
		};
		d.call(&mut r);
		let mut m = Mouse {
			d,
			wheel,
			div,
			left: false,
			at: mid,
		};
		// Counts from before this program ran are not clicks of this session.
		m.edges(func::PRESSES);
		m.edges(func::RELEASES);
		Some(m)
	}

	/// True when the driver reported a wheel.
	pub fn has_wheel(&self) -> bool {
		self.wheel
	}

	/// Where the pointer was at the last poll, in shell units.
	pub fn at(&self) -> (i32, i32) {
		self.at
	}

	/// The driver, for the shell's clean-up (a reset).
	pub fn driver(&mut self) -> &mut D {
		&mut self.d
	}

	/// Gives the mouse back to whatever runs next: a reset, so the range and the cursor are the driver's defaults again.
	pub fn close(mut self) {
		let mut r = Regs::default();
		self.d.call(&mut r);
	}

	fn units(&self, x: u16, y: u16) -> (i32, i32) {
		(
			i32::from(x as i16) / self.div.0,
			i32::from(y as i16) / self.div.1,
		)
	}

	/// Function 0005h or 0006h for the left button: how many and where at the last.
	fn edges(&mut self, ax: u16) -> (u16, (i32, i32)) {
		let mut r = Regs {
			ax,
			bx: 0,
			..Regs::default()
		};
		if !self.d.call(&mut r) {
			return (0, self.at);
		}
		(r.bx, self.units(r.cx, r.dx))
	}

	/// Reads the driver and calls `emit` for what happened since the last poll, in order: moves, presses, releases, the wheel.
	pub fn poll(&mut self, mut emit: impl FnMut(Ev)) {
		let mut r = Regs {
			ax: func::POSITION,
			..Regs::default()
		};
		if !self.d.call(&mut r) {
			return;
		}
		let now = r.bx & 1 != 0;
		let wheel = if self.wheel {
			i32::from((r.bx >> 8) as u8 as i8)
		} else {
			0
		};
		let pos = self.units(r.cx, r.dx);
		let (presses, pressed_at) = self.edges(func::PRESSES);
		let (releases, released_at) = self.edges(func::RELEASES);
		let mut at = self.at;
		let mut go = |to: (i32, i32), emit: &mut dyn FnMut(Ev)| {
			if to != at {
				at = to;
				emit(Ev::Move { x: to.0, y: to.1 });
			}
		};
		let emit: &mut dyn FnMut(Ev) = &mut emit;
		match (self.left, now) {
			(false, false) => {
				// A click shorter than a poll: the state is the same before and after, the counters are not. Clicks that came in
				// one poll are all reported (up to three, as many as a triple click needs), at the place of the last, so a quick
				// double click is still a double click when the program was busy.
				for _ in 0..presses.min(releases).min(3) {
					go(pressed_at, emit);
					emit(Ev::Down {
						x: pressed_at.0,
						y: pressed_at.1,
					});
					go(released_at, emit);
					emit(Ev::Up {
						x: released_at.0,
						y: released_at.1,
					});
				}
				go(pos, emit);
			}
			(false, true) => {
				// Clicks the poll missed, and then the press that is still held.
				for _ in 0..presses.saturating_sub(1).min(releases).min(2) {
					go(pressed_at, emit);
					emit(Ev::Down {
						x: pressed_at.0,
						y: pressed_at.1,
					});
					go(released_at, emit);
					emit(Ev::Up {
						x: released_at.0,
						y: released_at.1,
					});
				}
				let from = if presses > 0 { pressed_at } else { pos };
				go(from, emit);
				emit(Ev::Down {
					x: from.0,
					y: from.1,
				});
				go(pos, emit);
			}
			(true, true) => {
				if releases > 0 && presses > 0 {
					go(released_at, emit);
					emit(Ev::Up {
						x: released_at.0,
						y: released_at.1,
					});
					go(pressed_at, emit);
					emit(Ev::Down {
						x: pressed_at.0,
						y: pressed_at.1,
					});
				}
				go(pos, emit);
			}
			(true, false) => {
				let to = if releases > 0 { released_at } else { pos };
				go(to, emit);
				emit(Ev::Up { x: to.0, y: to.1 });
				go(pos, emit);
			}
		}
		if wheel != 0 {
			emit(Ev::Wheel(wheel));
		}
		self.left = now;
		self.at = at;
	}
}
