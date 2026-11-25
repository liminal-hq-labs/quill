// The PS/2 mouse, read directly when no INT 33h driver is loaded.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! On a machine with no mouse driver (bare DOS, QEMU's PS/2 mouse) Quill drives the mouse itself with Desk's `desk-i8042`: the
//! controller's auxiliary port is enabled, the mouse reset and probed for a wheel, an IRQ 12 handler (`rt::irq`, the DPMI vector and
//! the end-of-interrupt) assembles its packets into a ring, and the main loop drains the ring into the same events the INT 33h reader
//! makes ([`quill_mouse::ps2::Ps2Pointer`]). A machine whose BIOS reports no pointing device is not probed, so the keyboard's
//! controller is left alone.

use desk_hal::irq::{Chain, InterruptController, Irq, IrqHandler, IrqRegistry};
use desk_hal::ring::{Consumer, SpscRing};
use desk_hal::x86::RawPorts;
use desk_hal::MouseEvent;
use desk_hal::PortIo;
use desk_i8042::{IrqStats, MouseIrq, I8042};
use desk_pic::Pic8259;
use quill_mouse::ps2::{Packet, Ps2Pointer};
use quill_mouse::Ev;
use rt::dpmi;
use rt::irq::{DosIrq, Slot};

static RING: SpscRing<MouseEvent, 32> = SpscRing::new();
static STATS: IrqStats = IrqStats::new();
static SERVICE: Slot<MouseIrq<RawPorts>> = Slot::new();

/// The mouse behind the controller and the pointer it moves.
pub struct Ps2Mouse {
	rx: Consumer<'static, MouseEvent, 32>,
	ptr: Ps2Pointer,
}

/// True when the BIOS says a pointing device is installed: bit 2 of the equipment word at 0040:0010.
fn bios_has_pointing_device() -> bool {
	// SAFETY: the BIOS data area is in the first megabyte, which the host maps at linear 0.
	unsafe { dpmi::linear_to_ptr::<u16>(0x410).read_volatile() & 4 != 0 }
}

fn ports() -> RawPorts {
	// SAFETY: a DPMI client may use the controller's ports, and this is the only code that does while the mouse is open.
	unsafe { RawPorts::new() }
}

/// The keyboard's interrupt held off while the program talks to the controller: the BIOS's IRQ 1 handler would otherwise read the
/// controller's own replies (the configuration byte, say) as if they were scancodes before the program could.
struct KeyboardHeldOff {
	was_masked: bool,
	/// The controller has answered the program (a reply was read), so a request is latched in the PIC.
	replied: bool,
}

impl KeyboardHeldOff {
	fn new() -> Self {
		let pic = Pic8259::new(ports());
		let was = pic.is_masked(Irq::KEYBOARD);
		pic.mask(Irq::KEYBOARD);
		KeyboardHeldOff {
			was_masked: was,
			replied: false,
		}
	}
}

/// Leaves the controller's data register holding a keyboard acknowledgement. Every reply the program read from the controller while
/// IRQ 1 was held off latched a request in the PIC, and the BIOS handler that gets it when the line is let go reads the data port
/// again, which still holds the last byte read (a configuration byte of 41h is the scan code of F7, and F7 starts the spelling
/// check): an acknowledgement, 0FAh, is what that handler expects from the keyboard and ignores.
fn settle_keyboard() {
	let ctl = I8042::new(ports());
	// A byte waiting is a key typed meanwhile: the BIOS handler will read that one, and the register holds it.
	if ctl.status() & desk_i8042::status::OUTPUT_FULL != 0 {
		return;
	}
	if ctl.write_data(0xF4).is_ok() {
		for _ in 0..desk_i8042::POLLS {
			if ctl.status() & desk_i8042::status::OUTPUT_FULL != 0 {
				break;
			}
		}
		let _ = ports().inb(desk_i8042::DATA_PORT);
	}
}

impl Drop for KeyboardHeldOff {
	fn drop(&mut self) {
		// A controller that never answered (no mouse there) leaves nothing latched, and a typed key waiting in it is not touched.
		if self.replied {
			settle_keyboard();
		}
		if !self.was_masked {
			Pic8259::new(ports()).unmask(Irq::KEYBOARD);
		}
	}
}

impl Ps2Mouse {
	/// Finds a PS/2 mouse and starts it, for a screen of `w` by `h` shell units (`div` counts to a unit, as for INT 33h).
	pub fn open(w: i32, h: i32, div: (i32, i32)) -> Option<Ps2Mouse> {
		if !bios_has_pointing_device() {
			return None;
		}
		let mut held = KeyboardHeldOff::new();
		let ctl = I8042::new(ports());
		let info = ctl.start_mouse(true).ok()?;
		held.replied = true;
		// A mouse that streams with nobody to read it would fill the controller's output buffer and hold the keyboard up.
		let give_up = |registered: bool| {
			if registered {
				let _ = DosIrq.unregister(Irq::MOUSE);
			}
			let _ = ctl.stop_mouse();
		};
		let Some((tx, rx)) = RING.split() else {
			give_up(false);
			return None;
		};
		let Some(svc) = SERVICE.place(MouseIrq::new(ports(), tx, &STATS, info.id == 3)) else {
			give_up(false);
			return None;
		};
		if DosIrq
			.register(Irq::MOUSE, IrqHandler::from_static(svc), Chain::None)
			.is_err()
		{
			give_up(false);
			return None;
		}
		// The keyboard's interrupt stays on, as the BIOS left it.
		if ctl.set_interrupts(true, true).is_err() {
			give_up(true);
			return None;
		}
		Some(Ps2Mouse {
			rx,
			ptr: Ps2Pointer::new(w, h, div),
		})
	}

	/// Where the pointer is, in shell units.
	pub fn at(&self) -> (i32, i32) {
		self.ptr.at()
	}

	/// Turns what the mouse reported since the last call into events.
	pub fn poll(&mut self, mut emit: impl FnMut(Ev)) {
		while let Some(e) = self.rx.pop() {
			let p = Packet {
				dx: e.dx,
				dy: e.dy,
				left: e.buttons & MouseEvent::LEFT != 0,
				wheel: e.wheel,
			};
			self.ptr.feed(p, &mut emit);
		}
	}

	/// Gives the mouse back: its interrupt off, the vector put back, the mouse told to stop reporting, and what it had already sent
	/// read off the controller (a byte left in the output buffer would hold the keyboard up).
	pub fn close(self) {
		let mut held = KeyboardHeldOff::new();
		held.replied = true;
		let ctl = I8042::new(ports());
		let _ = ctl.set_interrupts(true, false);
		let _ = DosIrq.unregister(Irq::MOUSE);
		let _ = ctl.stop_mouse();
	}
}
