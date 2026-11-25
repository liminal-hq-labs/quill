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

/// Writes the time `hour` (0 to 23) and `minute` the way WIN.INI's `[intl]` section or a DOS country says: `h24` for a 24-hour clock
/// (`18:45`) or the 12-hour one (`6:45 PM`), `sep` the separator (empty means a colon), `am` and `pm` the markers of the 12-hour clock
/// (empty means `AM` and `PM`). Returns how many bytes it wrote.
pub fn format_time(
	hour: u32,
	minute: u32,
	h24: bool,
	sep: &[u8],
	am: &[u8],
	pm: &[u8],
	out: &mut [u8],
) -> usize {
	let mut n = 0;
	let h = if h24 {
		hour
	} else {
		match hour % 12 {
			0 => 12,
			h => h,
		}
	};
	push_num(out, &mut n, h, if h24 { 2 } else { 1 });
	for b in if sep.is_empty() { b":" } else { sep } {
		push(out, &mut n, *b);
	}
	push_num(out, &mut n, minute, 2);
	if !h24 {
		push(out, &mut n, b' ');
		let (mark, default): (&[u8], &[u8]) = if hour < 12 { (am, b"AM") } else { (pm, b"PM") };
		for b in if mark.is_empty() { default } else { mark } {
			push(out, &mut n, *b);
		}
	}
	n
}

/// The date pattern a DOS country gives (INT 21h AX=3800h: `order` is the word at offset 0, 0 month first, 1 day first, 2 year
/// first, and `sep` the date separator): `MM/dd/yyyy`, `dd/MM/yyyy` or `yyyy/MM/dd`, ready for `format`. Returns the length written.
pub fn dos_pattern(order: u16, sep: u8, out: &mut [u8; 10]) -> usize {
	let sep = if sep == 0 { b'/' } else { sep };
	let parts: [&[u8]; 3] = match order {
		1 => [b"dd", b"MM", b"yyyy"],
		2 => [b"yyyy", b"MM", b"dd"],
		_ => [b"MM", b"dd", b"yyyy"],
	};
	let mut n = 0;
	for (i, p) in parts.iter().enumerate() {
		if i > 0 {
			push(out, &mut n, sep);
		}
		for b in *p {
			push(out, &mut n, *b);
		}
	}
	n
}
