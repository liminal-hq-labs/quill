// The caret's blink: when it shows, from the time and what the person is doing.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// A pure state machine over a clock in milliseconds, so it is tested without a display or a timer. The caret is solid while a
// key or the pointer has just been used (it stops blinking while typing), solid while the window is inactive, and solid when
// blinking is off (GTK's `gtk-cursor-blink`, or `QUILL_NO_BLINK` in tests). Otherwise it is on for half a blink time and off for
// half, from the end of the solid spell.

/// How long the caret stays solid after an input, in blink times.
const HOLD_CYCLES: u64 = 1;

/// The caret's phase.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Blink {
	enabled: bool,
	active: bool,
	/// A whole blink, on and off, in milliseconds.
	cycle_ms: u64,
	solid_until: u64,
}

impl Blink {
	/// A blink of `cycle_ms` for a window that is active. With `enabled` off the caret never blinks.
	pub fn new(enabled: bool, cycle_ms: u32) -> Blink {
		Blink {
			enabled,
			active: true,
			cycle_ms: u64::from(cycle_ms.clamp(200, 10_000)),
			solid_until: 0,
		}
	}

	/// The system's setting: `gtk-cursor-blink` and `gtk-cursor-blink-time`, with `QUILL_NO_BLINK` (any value but 0) turning it off.
	pub fn from_settings(
		system_blink: bool,
		system_time_ms: i32,
		no_blink_env: Option<&str>,
	) -> Blink {
		let off = no_blink_env.is_some_and(|v| v != "0");
		Blink::new(system_blink && !off, system_time_ms.max(0) as u32)
	}

	/// An input just happened at `now`: the caret shows, and stays solid for a while.
	pub fn input(&mut self, now: u64) {
		self.solid_until = now + self.cycle_ms * HOLD_CYCLES;
	}

	/// The window gained or lost the focus: an inactive window shows a solid caret.
	pub fn set_active(&mut self, active: bool, now: u64) {
		self.active = active;
		if active {
			self.input(now);
		}
	}

	/// Whether the caret shows at `now`.
	pub fn on(&self, now: u64) -> bool {
		if !self.enabled || !self.active || now < self.solid_until {
			return true;
		}
		((now - self.solid_until) / (self.cycle_ms / 2)).is_multiple_of(2)
	}

	/// Whether the caret ever changes: the timer is needed only then.
	pub fn blinks(&self) -> bool {
		self.enabled && self.active
	}

	/// The milliseconds between looks at the clock.
	pub fn step_ms(&self) -> u64 {
		self.cycle_ms / 2
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn it_is_solid_while_typing_then_blinks_on_and_off() {
		let mut b = Blink::new(true, 1000);
		b.input(0);
		assert!(b.on(0) && b.on(999));
		assert!(b.on(1000), "the first half after the hold is on");
		assert!(!b.on(1500));
		assert!(b.on(2000));
		b.input(2100);
		assert!(b.on(2600), "a key press makes it solid again");
	}

	#[test]
	fn it_is_solid_when_blinking_is_off_or_the_window_is_inactive() {
		let off = Blink::new(false, 1000);
		assert!(off.on(1500) && !off.blinks());
		let mut b = Blink::new(true, 1000);
		b.set_active(false, 0);
		assert!(b.on(1500) && !b.blinks());
		b.set_active(true, 3000);
		assert!(b.on(3500));
		assert!(!b.on(4500));
	}

	#[test]
	fn the_system_setting_and_the_test_switch_decide_whether_it_blinks() {
		assert!(Blink::from_settings(true, 1200, None).blinks());
		assert!(!Blink::from_settings(false, 1200, None).blinks());
		assert!(!Blink::from_settings(true, 1200, Some("1")).blinks());
		assert!(Blink::from_settings(true, 1200, Some("0")).blinks());
	}
}
