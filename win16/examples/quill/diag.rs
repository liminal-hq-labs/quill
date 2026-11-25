// Diagnostics for a build made with `W16_FEATURES=diag`: a check that watches Quill's picture slots and the 32-bit stack alias, and
// the test-only `/BADSLOT` switch. Without the feature every function here is empty and inlined away, so the shipped QUILL.EXE does
// not carry any of it.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// `check(tag)` is called at the entry and exit of the window procedure (the tag is the message number, with 0x10000 added on the way
// out) and around the places a host could write into the data segment: GetSaveFileName (0xF001, 0xF002), MessageBox (0xF003,
// 0xF004) and the save (0xF005, 0xF006). It appends one line to CRUMBS.LOG, beside the program, when the stack alias has moved (the
// first call gives the starting point) and, once, when a picture slot's state byte is not a state, with every slot's bytes. The log
// names the last call that saw the slots whole and the first that did not.

#[cfg(feature = "diag")]
mod on {
	use crate::picture;

	static mut REPORTED: bool = false;
	static mut LAST_MOVES: u32 = u32::MAX;

	struct Line {
		buf: [u8; 192],
		n: usize,
	}

	impl Line {
		fn s(&mut self, t: &[u8]) -> &mut Self {
			for b in t {
				if self.n < self.buf.len() {
					self.buf[self.n] = *b;
					self.n += 1;
				}
			}
			self
		}
		fn hex(&mut self, v: u32, digits: u32) -> &mut Self {
			let mut i = digits;
			while i > 0 {
				i -= 1;
				let d = ((v >> (i * 4)) & 0xF) as u8;
				self.s(&[if d < 10 { b'0' + d } else { b'A' + d - 10 }]);
			}
			self
		}
	}

	/// Appends `line` to CRUMBS.LOG beside the program, sharing the file with `crumb`.
	fn append(line: &Line) {
		unsafe {
			let was = crate::USE_HERE;
			crate::USE_HERE = true;
			let mut name = [0u8; 128];
			crate::c_name(b"CRUMBS.LOG", &mut name);
			crate::USE_HERE = was;
			let h = if crate::CRUMBS_MADE {
				let h = w16::_lopen(name.as_ptr() as u32, 1);
				if h != w16::HFILE_ERROR {
					w16::_llseek(h, 0, 2);
				}
				h
			} else {
				crate::CRUMBS_MADE = true;
				w16::_lcreat(name.as_ptr() as u32, 0)
			};
			if h == w16::HFILE_ERROR {
				return;
			}
			w16::_lwrite(h, line.buf.as_ptr() as u32, line.n as u32);
			w16::_lclose(h);
		}
	}

	pub fn check(tag: u32) {
		unsafe {
			let moves = w16::stack_moves();
			if moves != LAST_MOVES {
				LAST_MOVES = moves;
				let (ss16, ss32) = w16::stack_selectors();
				let mut l = Line {
					buf: [0; 192],
					n: 0,
				};
				l.s(b"ALIAS tag=").hex(tag, 8).s(b" moves=").hex(moves, 4);
				l.s(b" ss16=")
					.hex(ss16, 4)
					.s(b" ss32=")
					.hex(ss32, 4)
					.s(b"\r\n");
				append(&l);
			}
			if REPORTED {
				return;
			}
			let mut bad = false;
			for i in 0..picture::SLOTS {
				bad |= picture::slot_bytes(i)[picture::WHY_AT] > picture::WHY_MAX;
			}
			if !bad {
				return;
			}
			REPORTED = true;
			for i in 0..picture::SLOTS {
				let mut l = Line {
					buf: [0; 192],
					n: 0,
				};
				l.s(if i == 0 { b"SLOT tag=" } else { b"SLOT     " });
				if i == 0 {
					l.hex(tag, 8);
				}
				l.s(b" n=").hex(i as u32, 1).s(b" bytes");
				for b in picture::slot_bytes(i) {
					l.s(b" ").hex(u32::from(*b), 2);
				}
				l.s(b"\r\n");
				append(&l);
			}
		}
	}

	/// The `/BADSLOT` test: slot 1 gets a state byte that is not a state.
	pub fn plant() {
		picture::plant_bad_slot(1, 0x78);
	}
}

#[cfg(feature = "diag")]
pub use on::{check, plant};

#[cfg(not(feature = "diag"))]
#[inline(always)]
pub fn check(_tag: u32) {}

#[cfg(not(feature = "diag"))]
#[inline(always)]
pub fn plant() {}
