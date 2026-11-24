// The About box's platform lines: which Windows the program found itself on, or what the DOS build runs on, and how much memory it has, as short text.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The shell reads the numbers from the system (`GetVersionEx`, `RtlGetVersion`, `GlobalMemoryStatusEx`) and this module only turns
//! them into words, with integer arithmetic and no allocation, so every mapping is tested on the host.

use core::fmt::{self, Write};

/// `VER_PLATFORM_WIN32_WINDOWS`: Windows 95, 98 and Me.
pub const PLATFORM_WINDOWS: u32 = 1;
/// `VER_PLATFORM_WIN32_NT`: Windows NT 4.0 and everything after it.
pub const PLATFORM_NT: u32 = 2;

/// The first build of Windows 11 (it still reports 10.0).
pub const WINDOWS_11_BUILD: u32 = 22000;

/// The build of Windows 98 Second Edition; Windows 98 is the one before.
const WIN98_SE_BUILD: u32 = 2222;

/// A line of text in a fixed buffer, long enough for any About line; a longer write is cut, never a panic.
pub struct Line {
	buf: [u8; 96],
	len: usize,
}

impl Line {
	fn new() -> Line {
		Line {
			buf: [0; 96],
			len: 0,
		}
	}

	/// The text.
	pub fn as_str(&self) -> &str {
		self.buf
			.get(..self.len)
			.and_then(|b| core::str::from_utf8(b).ok())
			.unwrap_or("")
	}
}

impl Write for Line {
	fn write_str(&mut self, s: &str) -> fmt::Result {
		for &b in s.as_bytes() {
			if let Some(slot) = self.buf.get_mut(self.len) {
				*slot = b;
				self.len += 1;
			}
		}
		Ok(())
	}
}

/// What the system said about itself.
#[derive(Copy, Clone)]
pub struct Os<'a> {
	/// `dwPlatformId`: [`PLATFORM_WINDOWS`] or [`PLATFORM_NT`].
	pub platform: u32,
	pub major: u32,
	pub minor: u32,
	/// `dwBuildNumber`; on Windows 9x the caller passes the low word, which holds the build.
	pub build: u32,
	/// The version Wine reports about itself (`11.21`), when the program runs under Wine.
	pub wine: Option<&'a str>,
}

/// The product name for a version, or `None` when it is not one the table knows.
pub fn os_name(platform: u32, major: u32, minor: u32, build: u32) -> Option<&'static str> {
	match (platform, major, minor) {
		(PLATFORM_WINDOWS, 4, 0) => Some("Windows 95"),
		(PLATFORM_WINDOWS, 4, 10) if build >= WIN98_SE_BUILD => Some("Windows 98 SE"),
		(PLATFORM_WINDOWS, 4, 10) => Some("Windows 98"),
		(PLATFORM_WINDOWS, 4, 90) => Some("Windows Me"),
		(PLATFORM_NT, 4, 0) => Some("Windows NT 4.0"),
		(PLATFORM_NT, 5, 0) => Some("Windows 2000"),
		(PLATFORM_NT, 5, 1) => Some("Windows XP"),
		(PLATFORM_NT, 5, 2) => Some("Windows Server 2003"),
		(PLATFORM_NT, 6, 0) => Some("Windows Vista"),
		(PLATFORM_NT, 6, 1) => Some("Windows 7"),
		(PLATFORM_NT, 6, 2) => Some("Windows 8"),
		(PLATFORM_NT, 6, 3) => Some("Windows 8.1"),
		(PLATFORM_NT, 10, 0) if build >= WINDOWS_11_BUILD => Some("Windows 11"),
		(PLATFORM_NT, 10, 0) => Some("Windows 10"),
		_ => None,
	}
}

/// The platform line: `Windows 95 (Win32, 4.00)`, `Windows 11 (Win32, 10.0.26100)` or `Windows 8 under Wine 11.21 (Win32)`.
///
/// Windows 9x shows `major.minor` with two digits for the minor; Windows NT adds the build, which tells a reader which release of
/// Windows 10 it is. Under Wine the version is the one Wine was configured to report, so the build is left out. A version the table
/// does not know is shown as `Windows` and its number.
pub fn platform_line(os: &Os) -> Line {
	let mut line = Line::new();
	let name = os_name(os.platform, os.major, os.minor, os.build);
	match name {
		Some(n) => {
			let _ = write!(line, "{n}");
		}
		None => {
			let _ = write!(line, "Windows");
		}
	}
	if let Some(w) = os.wine {
		let _ = write!(line, " under Wine {w} (Win32)");
		return line;
	}
	let _ = write!(line, " (Win32, {}.{:02}", os.major, os.minor);
	if os.platform == PLATFORM_NT {
		let _ = write!(line, ".{}", os.build);
	}
	let _ = write!(line, ")");
	line
}

const MIB: u64 = 1 << 20;
const GIB: u64 = 1 << 30;

/// The memory line: `Free memory: 40 MB of 64 MB`, in gigabytes with one decimal (`Free memory: 9.5 GB of 15.6 GB`) once the
/// total reaches 4 GB. Both are byte counts; 64 bits keep a machine with more than 4 GB from wrapping.
pub fn memory_line(avail: u64, total: u64) -> Line {
	let mut line = Line::new();
	if total >= 4 * GIB {
		let _ = write!(line, "Free memory: ");
		tenths(&mut line, avail, GIB);
		let _ = write!(line, " GB of ");
		tenths(&mut line, total, GIB);
		let _ = write!(line, " GB");
	} else {
		let _ = write!(
			line,
			"Free memory: {} MB of {} MB",
			avail / MIB,
			total / MIB
		);
	}
	line
}

/// `bytes / unit` rounded down to a tenth, written `9.5`.
fn tenths(line: &mut Line, bytes: u64, unit: u64) {
	let t = (bytes / MIB) * 10 / (unit / MIB);
	let _ = write!(line, "{}.{}", t / 10, t % 10);
}

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

#[cfg(test)]
mod tests {
	use super::*;

	fn line(platform: u32, major: u32, minor: u32, build: u32, wine: Option<&str>) -> Line {
		platform_line(&Os {
			platform,
			major,
			minor,
			build,
			wine,
		})
	}

	#[test]
	fn windows_9x_versions_are_exact() {
		assert_eq!(
			line(1, 4, 0, 950, None).as_str(),
			"Windows 95 (Win32, 4.00)"
		);
		assert_eq!(
			line(1, 4, 0, 1111, None).as_str(),
			"Windows 95 (Win32, 4.00)"
		);
		assert_eq!(
			line(1, 4, 10, 1998, None).as_str(),
			"Windows 98 (Win32, 4.10)"
		);
		assert_eq!(
			line(1, 4, 10, 2222, None).as_str(),
			"Windows 98 SE (Win32, 4.10)"
		);
		assert_eq!(
			line(1, 4, 90, 3000, None).as_str(),
			"Windows Me (Win32, 4.90)"
		);
	}

	#[test]
	fn windows_nt_versions_are_named_with_the_build() {
		let cases = [
			((4, 0, 1381), "Windows NT 4.0 (Win32, 4.00.1381)"),
			((5, 0, 2195), "Windows 2000 (Win32, 5.00.2195)"),
			((5, 1, 2600), "Windows XP (Win32, 5.01.2600)"),
			((5, 2, 3790), "Windows Server 2003 (Win32, 5.02.3790)"),
			((6, 0, 6002), "Windows Vista (Win32, 6.00.6002)"),
			((6, 1, 7601), "Windows 7 (Win32, 6.01.7601)"),
			((6, 2, 9200), "Windows 8 (Win32, 6.02.9200)"),
			((6, 3, 9600), "Windows 8.1 (Win32, 6.03.9600)"),
			((10, 0, 19045), "Windows 10 (Win32, 10.00.19045)"),
			((10, 0, 21999), "Windows 10 (Win32, 10.00.21999)"),
			((10, 0, 22000), "Windows 11 (Win32, 10.00.22000)"),
			((10, 0, 26100), "Windows 11 (Win32, 10.00.26100)"),
		];
		for ((major, minor, build), want) in cases {
			assert_eq!(line(2, major, minor, build, None).as_str(), want);
		}
	}

	#[test]
	fn wine_replaces_the_number_with_its_own_version() {
		assert_eq!(
			line(2, 6, 2, 9200, Some("11.21")).as_str(),
			"Windows 8 under Wine 11.21 (Win32)"
		);
		assert_eq!(
			line(2, 10, 0, 19045, Some("9.0")).as_str(),
			"Windows 10 under Wine 9.0 (Win32)"
		);
	}

	#[test]
	fn an_unknown_version_still_says_what_it_is() {
		assert_eq!(
			line(2, 11, 0, 30000, None).as_str(),
			"Windows (Win32, 11.00.30000)"
		);
		assert_eq!(line(1, 4, 5, 1, None).as_str(), "Windows (Win32, 4.05)");
		assert_eq!(line(3, 1, 0, 0, None).as_str(), "Windows (Win32, 1.00)");
	}

	#[test]
	fn memory_is_in_megabytes_below_four_gigabytes() {
		assert_eq!(
			memory_line(40 * MIB, 64 * MIB).as_str(),
			"Free memory: 40 MB of 64 MB"
		);
		assert_eq!(memory_line(0, 0).as_str(), "Free memory: 0 MB of 0 MB");
		// The 32-bit cap of GlobalMemoryStatus: 4 GB less 1 KB is still megabytes.
		let cap = 4 * GIB - 1024;
		assert_eq!(
			memory_line(cap, cap).as_str(),
			"Free memory: 4095 MB of 4095 MB"
		);
	}

	#[test]
	fn memory_over_four_gigabytes_is_in_gigabytes_with_a_tenth() {
		let total = 16 * GIB - 400 * MIB;
		let avail = 9 * GIB + 512 * MIB;
		assert_eq!(
			memory_line(avail, total).as_str(),
			"Free memory: 9.5 GB of 15.6 GB"
		);
		assert_eq!(
			memory_line(4 * GIB, 4 * GIB).as_str(),
			"Free memory: 4.0 GB of 4.0 GB"
		);
		assert_eq!(
			memory_line(100 * GIB, 128 * GIB).as_str(),
			"Free memory: 100.0 GB of 128.0 GB"
		);
		assert_eq!(
			memory_line(u64::MAX, u64::MAX).as_str(),
			"Free memory: 17179869183.9 GB of 17179869183.9 GB"
		);
	}

	#[test]
	fn a_long_line_is_cut_not_a_panic() {
		let mut l = Line::new();
		for _ in 0..40 {
			let _ = write!(l, "abcdefgh");
		}
		assert_eq!(l.as_str().len(), 96);
	}

	#[test]
	fn the_dos_about_text_names_the_system_and_the_memory() {
		let mut buf = [0u8; 120];
		let n = dos_about(&mut buf, (6, 22), (0, 9), 65536, Some(90266));
		assert_eq!(
			core::str::from_utf8(buf.get(..n).unwrap_or(&[])),
			Ok("MS-DOS, DPMI 0.9 on DOS 6.22\nFree memory: 90266 KB, largest block 65536 KB")
		);
		let n = dos_about(&mut buf, (7, 1), (1, 0), 4096, None);
		assert_eq!(
			core::str::from_utf8(buf.get(..n).unwrap_or(&[])),
			Ok("MS-DOS, DPMI 1.0 on DOS 7.01\nFree memory: 4096 KB free")
		);
	}
}
