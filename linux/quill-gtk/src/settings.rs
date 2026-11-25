// The window's own settings: its size and whether it was maximized.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// They live in `$XDG_CONFIG_HOME/quill/gtk.ini`, next to the `QUILL.INI` that holds the recent files and the zoom, because those
// two are the editor's own and every client reads them. The file is a small INI with one section, `[Window]`.

use std::path::{Path, PathBuf};

/// The smallest and largest window size kept, in logical pixels.
pub const SIZE_MIN: i32 = 320;
/// The largest window size kept.
pub const SIZE_MAX: i32 = 16384;

/// Where the window was.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowSettings {
	/// Width in logical pixels.
	pub width: i32,
	/// Height in logical pixels.
	pub height: i32,
	/// True when it was maximized; the size is then the one to restore to.
	pub maximized: bool,
}

impl Default for WindowSettings {
	fn default() -> Self {
		WindowSettings {
			width: 900,
			height: 700,
			maximized: false,
		}
	}
}

impl WindowSettings {
	/// The file in the settings folder `dir`.
	pub fn path(dir: &Path) -> PathBuf {
		dir.join("gtk.ini")
	}

	/// Reads the settings; anything missing or out of range stays at its default.
	pub fn load(dir: &Path) -> WindowSettings {
		let text = std::fs::read_to_string(Self::path(dir)).unwrap_or_default();
		Self::parse(&text)
	}

	/// Parses the text of the file.
	pub fn parse(text: &str) -> WindowSettings {
		let mut s = WindowSettings::default();
		let mut in_window = false;
		for line in text.lines().map(str::trim) {
			if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
				in_window = name.eq_ignore_ascii_case("Window");
			} else if let (true, Some((k, v))) = (in_window, line.split_once('=')) {
				let (k, v) = (k.trim(), v.trim());
				let num = v
					.parse::<i32>()
					.ok()
					.filter(|n| (SIZE_MIN..=SIZE_MAX).contains(n));
				if k.eq_ignore_ascii_case("Width") {
					s.width = num.unwrap_or(s.width);
				} else if k.eq_ignore_ascii_case("Height") {
					s.height = num.unwrap_or(s.height);
				} else if k.eq_ignore_ascii_case("Maximized") {
					s.maximized = v == "1";
				}
			}
		}
		s
	}

	/// The text of the file.
	pub fn text(&self) -> String {
		format!(
			"[Window]\nWidth={}\nHeight={}\nMaximized={}\n",
			self.width,
			self.height,
			u8::from(self.maximized)
		)
	}

	/// Writes the settings, making the folder if need be; false when it could not.
	pub fn save(&self, dir: &Path) -> bool {
		std::fs::create_dir_all(dir).is_ok() && std::fs::write(Self::path(dir), self.text()).is_ok()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn settings_round_trip_through_the_file() {
		let d = tempfile::tempdir().unwrap();
		let s = WindowSettings {
			width: 1024,
			height: 768,
			maximized: true,
		};
		assert!(s.save(&d.path().join("quill")));
		assert_eq!(WindowSettings::load(&d.path().join("quill")), s);
	}

	#[test]
	fn a_missing_file_or_a_bad_value_leaves_the_default() {
		let d = tempfile::tempdir().unwrap();
		assert_eq!(WindowSettings::load(d.path()), WindowSettings::default());
		let s = WindowSettings::parse(
			"[Window]\nWidth=12\nHeight=abc\nMaximized=1\n[Other]\nWidth=999\n",
		);
		assert_eq!(s.width, WindowSettings::default().width);
		assert_eq!(s.height, WindowSettings::default().height);
		assert!(s.maximized);
	}
}
