// quill: the GTK4 client. A read-only viewer for now: open, scroll, zoom.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod page;
mod window;

use std::cell::RefCell;
use std::path::PathBuf;
use std::process::ExitCode;
use std::rc::Rc;

use gtk::gio;
use gtk::prelude::*;
use quill_fonts::FontSet;
use quill_gtk::doc::Document;
use quill_gtk::files::GuiFiles;
use window::{Opts, Ui};

const USAGE: &str = "Usage: quill [OPTIONS] [FILE]

Options:
  -h, --help           Show this help
  -V, --version        Show the version
      --new-instance   Do not hand the file to a Quill that is already running

For tests and screenshots:
      --scale-120 N    Draw at N/120 (180 is 1.5) whatever the display says
      --zoom PERCENT   Start at this zoom
      --scroll Y       Start scrolled to this document pixel
      --size WxH       Window size in logical pixels
      --screenshot F   Render the window to a PNG with GTK's renderer, then quit
      --dump F         Write the page's texture to a PNG and print where it sits
      --quit-after MS  Quit after this long
      --dark, --light  Force the desk's theme
      --vendored-fonts Use only the fonts shipped with Quill
      --open-dialog    Open the Open dialog at start
      --no-settings    Do not read or write the window settings";

fn parse() -> Result<(Opts, Vec<PathBuf>, bool), String> {
	let mut o = Opts::default();
	let mut files = Vec::new();
	let mut new_instance = false;
	let mut args = std::env::args().skip(1);
	let value = |name: &str, args: &mut dyn Iterator<Item = String>| {
		args.next().ok_or_else(|| format!("{name} needs a value"))
	};
	while let Some(a) = args.next() {
		match a.as_str() {
			"--scale-120" => o.scale_120 = value(&a, &mut args)?.parse().ok(),
			"--zoom" => o.zoom = value(&a, &mut args)?.parse().ok(),
			"--scroll" => o.scroll = value(&a, &mut args)?.parse().ok(),
			"--size" => {
				let v = value(&a, &mut args)?;
				o.size = v
					.split_once('x')
					.and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)));
			}
			"--screenshot" => o.screenshot = Some(PathBuf::from(value(&a, &mut args)?)),
			"--dump" => o.dump = Some(PathBuf::from(value(&a, &mut args)?)),
			"--quit-after" => o.quit_after = value(&a, &mut args)?.parse().ok(),
			"--dark" => o.dark = Some(true),
			"--light" => o.dark = Some(false),
			"--vendored-fonts" => o.vendored_fonts = true,
			"--open-dialog" => o.open_dialog = true,
			"--no-settings" => o.no_settings = true,
			"--new-instance" => new_instance = true,
			s if s.starts_with("--") => return Err(format!("unknown option {s}")),
			_ => files.push(PathBuf::from(a)),
		}
	}
	Ok((o, files, new_instance))
}

fn main() -> ExitCode {
	for a in std::env::args().skip(1) {
		match a.as_str() {
			"-h" | "--help" => {
				println!("{USAGE}");
				return ExitCode::SUCCESS;
			}
			"-V" | "--version" => {
				println!("quill {}", env!("CARGO_PKG_VERSION"));
				return ExitCode::SUCCESS;
			}
			_ => {}
		}
	}
	let (opts, files, new_instance) = match parse() {
		Ok(p) => p,
		Err(e) => {
			eprintln!("quill: {e}\n\n{USAGE}");
			return ExitCode::from(2);
		}
	};
	if gtk::init().is_err() {
		eprintln!("quill: cannot open a display");
		return ExitCode::FAILURE;
	}
	let mut flags = gio::ApplicationFlags::HANDLES_OPEN;
	if new_instance || opts.screenshot.is_some() {
		flags |= gio::ApplicationFlags::NON_UNIQUE;
	}
	let app = gtk::Application::builder()
		.application_id("io.github.liminal_hq_labs.Quill")
		.flags(flags)
		.build();
	let ui: Rc<RefCell<Option<Rc<Ui>>>> = Rc::default();
	let make = {
		let ui = ui.clone();
		let opts = opts.clone();
		move |app: &gtk::Application| -> Option<Rc<Ui>> {
			if let Some(u) = ui.borrow().clone() {
				return Some(u);
			}
			let fonts = if opts.vendored_fonts {
				FontSet::vendored()
			} else {
				FontSet::system()
			};
			let doc = match Document::new(fonts, GuiFiles::from_env()) {
				Ok(d) => d,
				Err(e) => {
					eprintln!("quill: {e}");
					return None;
				}
			};
			let u = Ui::new(app, doc, opts.clone());
			*ui.borrow_mut() = Some(u.clone());
			Some(u)
		}
	};
	let first = files.first().cloned();
	let m = make.clone();
	app.connect_activate(move |app| {
		if let Some(u) = m(app) {
			u.present(first.as_deref());
		}
	});
	let m = make.clone();
	app.connect_open(move |app, files, _| {
		let path = files.first().and_then(|f| f.path());
		if let Some(u) = m(app) {
			u.present(path.as_deref());
		}
	});
	// A file named on the command line goes through GApplication, so a running Quill takes it.
	let args: Vec<String> = std::iter::once("quill".to_string())
		.chain(files.iter().map(|f| f.to_string_lossy().into_owned()))
		.collect();
	let code = app.run_with_args(&args);
	ExitCode::from(code.get())
}
