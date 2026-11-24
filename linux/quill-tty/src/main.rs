// quill-tty: the terminal client.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::io::{self, IsTerminal, Write};
use std::process::ExitCode;

use crossterm::event::{self, Event};
use crossterm::terminal::size;
use quill_tty::keymap::paste_keys;
use quill_tty::session::WIDTH;
use quill_tty::term::{restore, Painter, Palette, Terminal};
use quill_tty::{cp1252, map_key, Dirs, LinuxFiles, Options, Session};

const USAGE: &str = "Usage: quill-tty [--mono] [--no-kitty] [FILE]\n\n\
Quill in a terminal: the DOS text screen, 80 columns by 25 rows (50 on a terminal that tall).\n\
  --mono       the monochrome attribute scheme\n\
  --no-kitty   do not ask the terminal for the kitty keyboard protocol (Ctrl+I is then Tab)\n\
  --version    print the version\n\
F10 opens the menus; Character > Italic is Ctrl+I where the terminal can send it.";

fn main() -> ExitCode {
	let mut opts = Options {
		about: format!("Terminal client {}", env!("CARGO_PKG_VERSION")),
		..Options::default()
	};
	let mut kitty = true;
	for a in std::env::args().skip(1) {
		match a.as_str() {
			"--help" | "-h" => {
				println!("{USAGE}");
				return ExitCode::SUCCESS;
			}
			"--version" | "-V" => {
				println!("quill-tty {}", env!("CARGO_PKG_VERSION"));
				return ExitCode::SUCCESS;
			}
			"--mono" => opts.mono = true,
			"--no-kitty" => kitty = false,
			s if s.starts_with("--") => {
				eprintln!("quill-tty: unknown option {s}\n\n{USAGE}");
				return ExitCode::from(2);
			}
			path => match cp1252::name_to_ansi(path) {
				Some(name) => opts.open = Some(name),
				None => {
					eprintln!("quill-tty: {path}: the name has characters Windows-1252 lacks");
					return ExitCode::from(2);
				}
			},
		}
	}
	if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
		eprintln!("quill-tty: needs a terminal");
		return ExitCode::from(2);
	}
	match run(&mut opts, kitty) {
		Ok(()) => ExitCode::SUCCESS,
		Err(e) => {
			eprintln!("quill-tty: {e}");
			ExitCode::FAILURE
		}
	}
}

fn run(opts: &mut Options, kitty: bool) -> Result<(), String> {
	let (cols, rows) = size().map_err(|e| e.to_string())?;
	opts.tall = rows >= 50;
	let mut session = Session::new(LinuxFiles::new(Dirs::from_env()), opts)?;
	let term = Terminal::enter(kitty).map_err(|e| e.to_string())?;
	let enhanced = term.enhanced();
	let hook = std::panic::take_hook();
	std::panic::set_hook(Box::new(move |info| {
		restore(enhanced);
		hook(info);
	}));
	let mut painter = Painter::new(Palette::detect());
	let mut out = io::stdout().lock();
	let (mut cols, mut rows) = (cols, rows);
	loop {
		if cols < WIDTH as u16 || rows < 25 {
			let _ = write!(
				out,
				"\x1b[2J\x1b[HQuill needs a terminal of at least 80 by 25."
			);
			let _ = out.flush();
			painter.forget();
		} else {
			let left = (cols - WIDTH as u16) / 2;
			let at = session.cursor();
			painter
				.present(&mut out, session.screen(), left, at, session.overtype())
				.map_err(|e| e.to_string())?;
		}
		if session.quit() {
			break;
		}
		match event::read().map_err(|e| e.to_string())? {
			Event::Key(k) => {
				if let Some(k) = map_key(&k) {
					session.key(k);
				}
			}
			Event::Paste(text) => {
				for k in paste_keys(&text) {
					session.key(k);
				}
			}
			Event::Resize(c, r) => {
				cols = c;
				rows = r;
				session.set_tall(r >= 50);
				painter.forget();
			}
			_ => {}
		}
	}
	session.finish();
	drop(out);
	drop(term);
	Ok(())
}
