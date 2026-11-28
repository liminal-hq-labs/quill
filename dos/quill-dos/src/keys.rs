// The keyboard through the BIOS (INT 16h).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// INT 16h AH=00h gives an ASCII code in AL and a scan code in AH; `quill_app::bios::decode` turns the pair into a key, using the scan
// code to tell Enter, Backspace and Tab from Ctrl+M, Ctrl+H and Ctrl+I.

use core::arch::asm;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use desk_hal::irq::{Chain, Irq, IrqHandler, IrqRegistry, SharedIrqService};
use desk_hal::x86::RawPorts;
use desk_hal::PortIo;
use quill_app::{Dir, Key};
use rt::irq::DosIrq;

/// Pixels Ctrl+Left and Ctrl+Right scroll the page sideways.
const SCROLL_STEP: i32 = 48;

/// Set when a key was typed while Alt was held, so that letting Alt go afterwards is the end of a chord and not a tap.
static ALT_USED: AtomicBool = AtomicBool::new(false);

/// The BIOS key (AH:AL) last handed out and the BIOS tick it was handed out at; `NONE` before the first.
static LAST_KEY: AtomicU32 = AtomicU32::new(NONE);
static LAST_TICK: AtomicU32 = AtomicU32::new(0);
const NONE: u32 = u32::MAX;

/// Ticks (18.2 a second) a key may be silent and still be taken as the same hold: a little over the longest typematic delay (1 s).
const HOLD_TICKS: u32 = 22;

/// What the keyboard's interrupt saw: one flag for each key press, in order, that says whether it was the keyboard repeating the key
/// before (the same scan code again with no release between). `HEAD` counts the flags written and `TAIL` the ones read.
struct Snoop {
	flags: AtomicU32,
	head: AtomicU32,
	tail: AtomicU32,
	last_make: AtomicU32,
	broken: AtomicBool,
}

static SNOOP: Snoop = Snoop {
	flags: AtomicU32::new(0),
	head: AtomicU32::new(0),
	tail: AtomicU32::new(0),
	last_make: AtomicU32::new(0),
	broken: AtomicBool::new(true),
};

impl SharedIrqService for Snoop {
	/// Looks at the scan code the controller holds, before the BIOS handler takes it: reading the data port again is what the BIOS does
	/// next, and the controller keeps the byte until the next one comes.
	fn on_irq(&self) {
		// SAFETY: a DPMI client may read the keyboard controller's data port; nothing else is touched.
		let code = u32::from(unsafe { RawPorts::new() }.inb(0x60));
		// The prefixes of the extended keys, a release (bit 7) and the keys that put nothing in the BIOS buffer: Ctrl, the Shifts, Alt
		// and the three locks.
		if code == 0xE0 || code == 0xE1 {
			return;
		}
		if code & 0x80 != 0 {
			self.broken.store(true, Ordering::Relaxed);
			return;
		}
		if matches!(code, 0x1D | 0x2A | 0x36 | 0x38 | 0x3A | 0x45 | 0x46) {
			return;
		}
		let repeat = code == self.last_make.load(Ordering::Relaxed) && !self.broken.load(Ordering::Relaxed);
		self.last_make.store(code, Ordering::Relaxed);
		self.broken.store(false, Ordering::Relaxed);
		let head = self.head.load(Ordering::Relaxed);
		let bit = 1u32 << (head % 32);
		let flags = self.flags.load(Ordering::Relaxed);
		self.flags
			.store(if repeat { flags | bit } else { flags & !bit }, Ordering::Relaxed);
		self.head.store(head.wrapping_add(1), Ordering::Release);
	}
}

/// Starts watching the keyboard for repeats. If the interrupt cannot be taken nothing is watched and every key counts as a press, the
/// way it did before.
pub fn watch_repeats() {
	let _ = DosIrq.register(Irq::KEYBOARD, IrqHandler::from_shared(&SNOOP), Chain::Previous);
}

/// Whether the key just taken from the BIOS buffer was the keyboard repeating one: the flag of the oldest press not yet taken.
fn next_flag() -> bool {
	let head = SNOOP.head.load(Ordering::Acquire);
	let mut tail = SNOOP.tail.load(Ordering::Relaxed);
	if head.wrapping_sub(tail) > 32 {
		tail = head.wrapping_sub(32);
	}
	if tail == head {
		return false;
	}
	SNOOP.tail.store(tail.wrapping_add(1), Ordering::Relaxed);
	SNOOP.flags.load(Ordering::Relaxed) & (1u32 << (tail % 32)) != 0
}

/// With no key waiting, what the interrupt saw for keys that never reached the buffer is let go, so the flags and the keys line up. The
/// count is read before the buffer is looked at, so a key that arrives between the two keeps its flag.
fn resync() {
	let head = SNOOP.head.load(Ordering::Acquire);
	if !key_ready() {
		SNOOP.tail.store(head, Ordering::Relaxed);
	}
}

/// What `wait_until` came back with.
pub enum Wait {
	/// A key was typed; `None` when Quill has no use for it.
	Key(Option<Key>),
	/// The same key again, with no release in between: the keyboard's auto-repeat of a key still held. Only the keys that repeat
	/// (`App::key_repeats`) are to be acted on.
	Held(Key),
	/// The caller's check asked to stop waiting.
	Break,
	/// Ctrl+Left or Ctrl+Right: scroll the page sideways by this many pixels (the app has no key for either).
	Scroll(i32),
}

/// Waits for the next key and decodes it; keys Quill has no use for come back as `Key(None)`. Alt pressed and let go with no key between
/// comes back as F10, which is how Alt activates the menu bar: the BIOS reports only the state of Alt, so this polls it while it waits.
/// `check` is called each time round the idle loop (many times a tick, while no key is waiting) and `Break` comes back when it says
/// true: the mouse is polled this way, so a click is acted on while the keyboard is quiet.
pub fn wait_until(check: &mut dyn FnMut() -> bool) -> Wait {
	let mut alt_down = false;
	while !key_ready() {
		if check() {
			return Wait::Break;
		}
		if alt_flag() {
			alt_down = true;
		} else if alt_down {
			// Alt went up; a key that Alt let through (a keypad code) may have arrived in the same instant.
			if ALT_USED.swap(false, Ordering::Relaxed) {
				alt_down = false;
			} else if !key_ready() {
				return Wait::Key(Some(Key::F(10)));
			} else {
				break;
			}
		} else {
			ALT_USED.store(false, Ordering::Relaxed);
		}
		resync();
		idle();
	}
	if alt_flag() {
		ALT_USED.store(true, Ordering::Relaxed);
	}
	let ax: u32;
	// SAFETY: INT 16h is a BIOS service with register arguments only.
	unsafe { asm!("int 0x16", inout("eax") 0u32 => ax) };
	let flagged = next_flag();
	if ax as u8 == 0 && matches!((ax >> 8) as u8, 0x73 | 0x74) {
		return Wait::Scroll(if (ax >> 8) as u8 == 0x73 { -SCROLL_STEP } else { SCROLL_STEP });
	}
	let Some(key) = quill_app::bios::decode(ax as u8, (ax >> 8) as u8) else {
		return Wait::Key(None);
	};
	let key = with_shift(key);
	match key {
		Some(k) if is_repeat(ax, flagged) => Wait::Held(k),
		_ => Wait::Key(key),
	}
}

/// True when BIOS key `ax` is a repeat: the keyboard's interrupt saw the same scan code again with no release, and it is the key last
/// handed out, not long ago. The BIOS buffer gives each typematic repeat as a new key and shows no releases, so this is the only
/// way to tell one from a press. The last test is the safety: a release the interrupt never saw ends the hold after `HOLD_TICKS`, so a
/// toggle is never stuck off.
fn is_repeat(ax: u32, flagged: bool) -> bool {
	let now = crate::mouse::ticks();
	let same = flagged
		&& LAST_KEY.load(Ordering::Relaxed) == (ax & 0xFFFF)
		&& now.wrapping_sub(LAST_TICK.load(Ordering::Relaxed)) <= HOLD_TICKS;
	LAST_KEY.store(ax & 0xFFFF, Ordering::Relaxed);
	LAST_TICK.store(now, Ordering::Relaxed);
	same
}

/// The key as the shift keys make it: Shift with a movement key extends the selection.
fn with_shift(key: Key) -> Option<Key> {
	if shift_down() {
		let dir = match key {
			Key::Left => Dir::Left,
			Key::Right => Dir::Right,
			Key::Up => Dir::Up,
			Key::Down => Dir::Down,
			Key::Home => Dir::Home,
			Key::End => Dir::End,
			Key::PageUp => Dir::PageUp,
			Key::PageDown => Dir::PageDown,
			other => return Some(other),
		};
		return Some(Key::Select(dir));
	}
	Some(key)
}

/// True while a Shift key is held (INT 16h AH=02h, bits 0 and 1 of the shift flags).
pub fn shift_down() -> bool {
	let ax: u32;
	// SAFETY: INT 16h is a BIOS service with register arguments only.
	unsafe { asm!("int 0x16", inout("eax") 0x0200u32 => ax) };
	ax & 3 != 0
}

/// True when a key is waiting in the BIOS buffer (INT 16h AH=01h clears ZF when there is one).
pub fn key_ready() -> bool {
	let ready: u32;
	// SAFETY: INT 16h is a BIOS service with register arguments only; the flags come back in the byte after `lahf`.
	unsafe {
		asm!("int 0x16", "lahf", "movzx {r:e}, ah", r = out(reg) ready, inout("eax") 0x0100u32 => _);
	}
	ready & 0x40 == 0
}

/// True while an Alt key is held (bit 3 of the shift flags, INT 16h AH=02h).
fn alt_flag() -> bool {
	let ax: u32;
	// SAFETY: INT 16h is a BIOS service with register arguments only.
	unsafe { asm!("int 0x16", inout("eax") 0x0200u32 => ax) };
	ax & 8 != 0
}

/// Gives the rest of the time slice back (INT 2Fh AX=1680h), so waiting for a key does not spin an emulator's or a multitasker's CPU.
fn idle() {
	// SAFETY: INT 2Fh is a DOS service with register arguments only; a host that has no idle call leaves AL at 80h and does nothing.
	unsafe { asm!("int 0x2f", inout("eax") 0x1680u32 => _) };
}
