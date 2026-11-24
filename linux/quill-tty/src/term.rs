// The grid as escape sequences, and the terminal's modes.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::io::{self, Write};

use crossterm::cursor::{Hide, MoveTo, SetCursorStyle, Show};
use crossterm::event::{
	DisableBracketedPaste, EnableBracketedPaste, KeyboardEnhancementFlags,
	PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::style::{
	Attribute, Color, Print, SetAttribute, SetBackgroundColor, SetForegroundColor,
};
use crossterm::terminal::{
	disable_raw_mode, enable_raw_mode, supports_keyboard_enhancement, Clear, ClearType,
	EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::{execute, queue};
use quill_grid::{cp437, Cell, COLS};

use crate::session::Screen;

/// How VGA colours are sent.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Palette {
	/// The terminal's sixteen colours, whatever its theme makes of them.
	Ansi16,
	/// The VGA palette itself, as 24-bit colour.
	TrueColour,
}

impl Palette {
	/// True colour when `COLORTERM` says the terminal has it.
	pub fn detect() -> Palette {
		match std::env::var("COLORTERM") {
			Ok(v) if v == "truecolor" || v == "24bit" => Palette::TrueColour,
			_ => Palette::Ansi16,
		}
	}

	fn colour(self, vga: u8) -> Color {
		const ANSI: [Color; 16] = [
			Color::Black,
			Color::DarkBlue,
			Color::DarkGreen,
			Color::DarkCyan,
			Color::DarkRed,
			Color::DarkMagenta,
			Color::DarkYellow,
			Color::Grey,
			Color::DarkGrey,
			Color::Blue,
			Color::Green,
			Color::Cyan,
			Color::Red,
			Color::Magenta,
			Color::Yellow,
			Color::White,
		];
		const RGB: [(u8, u8, u8); 16] = [
			(0x00, 0x00, 0x00),
			(0x00, 0x00, 0xAA),
			(0x00, 0xAA, 0x00),
			(0x00, 0xAA, 0xAA),
			(0xAA, 0x00, 0x00),
			(0xAA, 0x00, 0xAA),
			(0xAA, 0x55, 0x00),
			(0xAA, 0xAA, 0xAA),
			(0x55, 0x55, 0x55),
			(0x55, 0x55, 0xFF),
			(0x55, 0xFF, 0x55),
			(0x55, 0xFF, 0xFF),
			(0xFF, 0x55, 0x55),
			(0xFF, 0x55, 0xFF),
			(0xFF, 0xFF, 0x55),
			(0xFF, 0xFF, 0xFF),
		];
		let i = usize::from(vga & 15);
		match self {
			Palette::Ansi16 => ANSI[i],
			Palette::TrueColour => {
				let (r, g, b) = RGB[i];
				Color::Rgb { r, g, b }
			}
		}
	}
}

/// Draws a screen on a terminal, sending only the cells that changed.
pub struct Painter {
	palette: Palette,
	shown: Vec<Option<Cell>>,
	rows: usize,
}

impl Painter {
	/// A painter with nothing on the terminal yet.
	pub fn new(palette: Palette) -> Painter {
		Painter {
			palette,
			shown: Vec::new(),
			rows: 0,
		}
	}

	/// Forgets what is on the terminal, so the next `present` draws it all.
	pub fn forget(&mut self) {
		self.shown.clear();
	}

	/// Writes `screen` with its left edge at terminal column `left`, and puts the cursor where `cursor` says (inside the
	/// screen) or hides it. `block` is the overtype cursor.
	pub fn present<W: Write>(
		&mut self,
		out: &mut W,
		screen: &Screen,
		left: u16,
		cursor: Option<(usize, usize)>,
		block: bool,
	) -> io::Result<()> {
		let rows = screen.rows();
		if self.shown.len() != rows * COLS || self.rows != rows {
			self.shown = vec![None; rows * COLS];
			self.rows = rows;
			queue!(out, SetAttribute(Attribute::Reset), Clear(ClearType::All))?;
		}
		queue!(out, Hide)?;
		let mut at: Option<(usize, usize)> = None;
		let mut pen: Option<u8> = None;
		for y in 0..rows {
			for x in 0..COLS {
				let Some(cell) = screen.get(x, y) else {
					continue;
				};
				let slot = &mut self.shown[y * COLS + x];
				if *slot == Some(cell) {
					continue;
				}
				*slot = Some(cell);
				if at != Some((x, y)) {
					queue!(out, MoveTo(left + x as u16, y as u16))?;
				}
				if pen != Some(cell.attr) {
					self.set_pen(out, cell.attr)?;
					pen = Some(cell.attr);
				}
				queue!(out, Print(cp437(cell.ch)))?;
				at = Some((x + 1, y));
			}
		}
		queue!(out, SetAttribute(Attribute::Reset))?;
		if let Some((x, y)) = cursor {
			let style = if block {
				SetCursorStyle::BlinkingBlock
			} else {
				SetCursorStyle::BlinkingUnderScore
			};
			queue!(out, MoveTo(left + x as u16, y as u16), style, Show)?;
		}
		out.flush()
	}

	fn set_pen<W: Write>(&self, out: &mut W, attr: u8) -> io::Result<()> {
		// Monochrome underline: the MDA's attribute 01h is blue on black, which on a VGA is drawn as underline.
		let underline = attr & 0x77 == 0x01;
		let (fg, bg) = if underline {
			(7 | (attr & 8), 0)
		} else {
			(attr & 15, (attr >> 4) & 15)
		};
		queue!(
			out,
			SetAttribute(if underline {
				Attribute::Underlined
			} else {
				Attribute::NoUnderline
			}),
			SetForegroundColor(self.palette.colour(fg)),
			SetBackgroundColor(self.palette.colour(bg))
		)
	}
}

/// The terminal put into Quill's modes; dropping it puts everything back.
pub struct Terminal {
	enhanced: bool,
}

impl Terminal {
	/// Raw mode, the alternate screen, bracketed paste and, if the terminal answers for it and `kitty` is true, the kitty keyboard
	/// protocol's disambiguated keys.
	pub fn enter(kitty: bool) -> io::Result<Terminal> {
		enable_raw_mode()?;
		let mut out = io::stdout();
		let enhanced = kitty && supports_keyboard_enhancement().unwrap_or(false);
		let r = (|| {
			execute!(out, EnterAlternateScreen, EnableBracketedPaste)?;
			if enhanced {
				execute!(
					out,
					PushKeyboardEnhancementFlags(
						KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
					)
				)?;
			}
			Ok(())
		})();
		let t = Terminal { enhanced };
		r.map(|()| t)
	}

	/// True when keys such as Ctrl+I arrive as pressed; false when the terminal can only send the legacy bytes.
	pub fn enhanced(&self) -> bool {
		self.enhanced
	}
}

impl Drop for Terminal {
	fn drop(&mut self) {
		restore(self.enhanced);
	}
}

/// Puts the terminal back as it was. Safe to call twice.
pub fn restore(enhanced: bool) {
	let mut out = io::stdout();
	if enhanced {
		let _ = execute!(out, PopKeyboardEnhancementFlags);
	}
	let _ = execute!(
		out,
		DisableBracketedPaste,
		SetCursorStyle::DefaultUserShape,
		Show,
		LeaveAlternateScreen
	);
	let _ = disable_raw_mode();
}

#[cfg(test)]
mod tests {
	use super::*;
	use quill_grid::Grid;

	fn screen_with(text: &[u8], attr: u8) -> Screen {
		let mut g = Grid::<25>::new(0x17);
		g.text(0, 0, text, attr);
		Screen::Rows25(Box::new(g))
	}

	#[test]
	fn the_first_present_draws_and_the_second_sends_nothing() {
		let mut p = Painter::new(Palette::TrueColour);
		let s = screen_with(b"Quill", 0x70);
		let mut first = Vec::new();
		p.present(&mut first, &s, 0, Some((5, 0)), false).unwrap();
		let text = String::from_utf8_lossy(&first);
		assert!(
			text.contains('Q') && text.contains("38;2;0;0;0"),
			"{text:?}"
		);
		let mut again = Vec::new();
		p.present(&mut again, &s, 0, Some((5, 0)), false).unwrap();
		assert!(
			!String::from_utf8_lossy(&again).contains('Q'),
			"an unchanged screen is not redrawn"
		);
	}

	#[test]
	fn only_the_changed_cells_are_sent() {
		let mut p = Painter::new(Palette::Ansi16);
		p.present(
			&mut Vec::new(),
			&screen_with(b"Quill", 0x70),
			0,
			None,
			false,
		)
		.unwrap();
		let mut out = Vec::new();
		p.present(&mut out, &screen_with(b"Quilt", 0x70), 0, None, false)
			.unwrap();
		let text = String::from_utf8_lossy(&out).into_owned();
		assert!(
			text.contains('t') && !text.contains('Q') && !text.contains('u'),
			"{text:?}"
		);
	}

	#[test]
	fn box_characters_are_sent_as_unicode() {
		let mut p = Painter::new(Palette::Ansi16);
		let mut out = Vec::new();
		p.present(
			&mut out,
			&screen_with(&[0xC9, 0xCD, 0xBB], 0x1F),
			0,
			None,
			false,
		)
		.unwrap();
		let text = String::from_utf8_lossy(&out).into_owned();
		assert!(text.contains("╔═╗"), "{text:?}");
	}

	#[test]
	fn vga_colours_map_to_the_sixteen() {
		assert_eq!(Palette::Ansi16.colour(1), Color::DarkBlue);
		assert_eq!(Palette::Ansi16.colour(0x1F), Color::White);
		assert_eq!(
			Palette::TrueColour.colour(6),
			Color::Rgb {
				r: 0xAA,
				g: 0x55,
				b: 0
			}
		);
	}
}
