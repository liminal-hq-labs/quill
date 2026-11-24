// Dates in the user's short date format: `M/d/yy`, `dd-MM-yyyy` and the like, as WIN.INI's `[intl] sShortDate` writes them.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

const MONTHS: [&str; 12] = [
	"Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

fn push(out: &mut [u8], n: &mut usize, b: u8) {
	if let Some(slot) = out.get_mut(*n) {
		*slot = b;
		*n += 1;
	}
}

fn push_num(out: &mut [u8], n: &mut usize, v: u32, width: usize) {
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
		push(out, n, b'0');
	}
	for d in digits.get(i..).unwrap_or(&[]) {
		push(out, n, *d);
	}
}

/// Writes the date `year`, `month` (1 to 12), `day` in the pattern `fmt` into `out` and returns how many bytes it wrote. `M` and `d`
/// are the number, `MM` and `dd` the number with two digits, `MMM` the month's short name, `yy` the two low digits of the year and
/// `yyyy` the whole year; any other character is copied. An empty pattern is month, day and year with slashes.
pub fn format(fmt: &[u8], year: u32, month: u32, day: u32, out: &mut [u8]) -> usize {
	let fmt: &[u8] = if fmt.is_empty() { b"M/d/yy" } else { fmt };
	let mut n = 0;
	let mut i = 0;
	while let Some(&c) = fmt.get(i) {
		let run = fmt
			.get(i..)
			.unwrap_or(&[])
			.iter()
			.take_while(|b| **b == c)
			.count();
		i += run;
		match c {
			b'M' if run >= 3 => {
				let name = MONTHS.get((month as usize).wrapping_sub(1)).unwrap_or(&"");
				for b in name.bytes() {
					push(out, &mut n, b);
				}
			}
			b'M' => push_num(out, &mut n, month, run),
			b'd' => push_num(out, &mut n, day, run.min(2)),
			b'y' if run <= 2 => push_num(out, &mut n, year % 100, 2),
			b'y' => push_num(out, &mut n, year, 4),
			_ => {
				for _ in 0..run {
					push(out, &mut n, c);
				}
			}
		}
	}
	n
}
