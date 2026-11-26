// The panic handler: undo what the program set up, report where and why, exit with status 3.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Status 3 is the skeleton's: a status DOS can hand back that no ordinary run returns. The report goes to standard output because
// that is what the headless harness captures. The clean-up list runs first, so a hooked IRQ 0 is off the machine before any
// formatting happens (formatting can allocate, and allocation can fail, which panics again). A panic inside the handler, or inside
// a clean-up function, exits at once.
use core::panic::PanicInfo;
use core::sync::atomic::{AtomicBool, Ordering};

static PANICKING: AtomicBool = AtomicBool::new(false);

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
	if PANICKING.swap(true, Ordering::SeqCst) {
		crate::dos::exit_now(3);
	}
	crate::cleanup::run_all();
	// Written without `core::fmt`, which the program would otherwise carry for this one line: the message when it is a plain string,
	// and the place as file and line.
	crate::io::write_str("RT PANIC: ");
	if let Some(m) = info.message().as_str() {
		crate::io::write_str(m);
	}
	if let Some(at) = info.location() {
		crate::io::write_str(" at ");
		crate::io::write_str(at.file());
		crate::io::write_str(":");
		let mut digits = [0u8; 10];
		let mut i = digits.len();
		let mut v = at.line();
		loop {
			i -= 1;
			if let Some(d) = digits.get_mut(i) {
				*d = b'0' + (v % 10) as u8;
			}
			v /= 10;
			if v == 0 {
				break;
			}
		}
		if let Some(d) = digits.get(i..).and_then(|d| core::str::from_utf8(d).ok()) {
			crate::io::write_str(d);
		}
	}
	crate::io::write_str("\r\n");
	crate::dos::exit_now(3)
}
