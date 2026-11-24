// The platform line of the About box: what the DOS build is running on and how much memory is left.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

struct Out<'a> {
	buf: &'a mut [u8],
	n: usize,
}

impl Out<'_> {
	fn s(&mut self, t: &str) -> &mut Self {
		for b in t.bytes() {
			if let Some(slot) = self.buf.get_mut(self.n) {
				*slot = b;
				self.n += 1;
			}
		}
		self
	}

	fn num(&mut self, v: u32, width: usize) -> &mut Self {
		let mut digits = [0u8; 10];
		let mut i = digits.len();
		let mut v = v;
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
		for _ in digits.len() - i..width {
			self.s("0");
		}
		for d in digits.get(i..).unwrap_or(&[]) {
			if let Some(slot) = self.buf.get_mut(self.n) {
				*slot = *d;
				self.n += 1;
			}
		}
		self
	}
}

/// Writes the DOS build's About text into `out` and returns its length: a line naming the system (`MS-DOS, DPMI 0.9 on DOS 6.22`) and
/// a line of memory (`Free memory: 90266 KB, largest block 65536 KB`), separated by a line feed. `free_kb` is `None` when the DPMI
/// host does not say how much is free.
pub fn dos_about(
	out: &mut [u8],
	dos: (u8, u8),
	dpmi: (u8, u8),
	largest_kb: u32,
	free_kb: Option<u32>,
) -> usize {
	let mut o = Out { buf: out, n: 0 };
	o.s("MS-DOS, DPMI ")
		.num(u32::from(dpmi.0), 1)
		.s(".")
		.num(u32::from(dpmi.1), 1)
		.s(" on DOS ")
		.num(u32::from(dos.0), 1)
		.s(".")
		.num(u32::from(dos.1), 2)
		.s("\n")
		.s("Free memory: ");
	if let Some(f) = free_kb {
		o.num(f, 1).s(" KB, largest block ");
	}
	o.num(largest_kb, 1).s(" KB");
	if free_kb.is_none() {
		o.s(" free");
	}
	o.n
}
