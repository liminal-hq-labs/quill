// The quill-fntgen command: writes QUILL.FNT, the size table, the proof sheets and the Windows ME comparison report.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use quill_fntgen::faceset::{self, FaceSet, SETS};
use quill_fntgen::fnt::Font;
use quill_fntgen::render::Opts;
use quill_fntgen::{build, fnt, report, sheet, sizes, strikes_where};

const USAGE: &str = "usage: quill-fntgen [--set desk|test] [--bpp 1|2] [--unhinted] -o QUILL.FNT
       quill-fntgen --sizes [FILE]       print the size table for every set and mode (and write FILE)
       quill-fntgen --sheets DIR         write the proof sheets (PNG) into DIR
       quill-fntgen --report [FILE]      print the Windows ME comparison for every set (and write FILE)";

fn flag(args: &[String], name: &str) -> bool {
	args.iter().any(|a| a == name)
}

fn value<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
	args.iter()
		.position(|a| a == name)
		.and_then(|i| args.get(i + 1))
		.map(String::as_str)
}

fn optional_path(args: &[String], name: &str) -> Option<PathBuf> {
	value(args, name)
		.filter(|v| !v.starts_with("--"))
		.map(PathBuf::from)
}

const MODES: [(&str, Opts); 4] = [
	(
		"1-bit unhinted",
		Opts {
			bpp: 1,
			hinted: false,
		},
	),
	(
		"1-bit hinted",
		Opts {
			bpp: 1,
			hinted: true,
		},
	),
	(
		"2-bit unhinted",
		Opts {
			bpp: 2,
			hinted: false,
		},
	),
	(
		"2-bit hinted",
		Opts {
			bpp: 2,
			hinted: true,
		},
	),
];

type Pick = Box<dyn Fn(u8, u8, u16) -> Option<Opts>>;

fn variants() -> Vec<(&'static str, Pick)> {
	let one = Opts {
		bpp: 1,
		hinted: true,
	};
	let two = Opts {
		bpp: 2,
		hinted: true,
	};
	vec![
		(
			"2-bit up to 14 pt, 1-bit at 18 and 24 pt",
			Box::new(move |_, _, pt| Some(if pt <= 14 { two } else { one })),
		),
		(
			"2-bit up to 12 pt, 1-bit above",
			Box::new(move |_, _, pt| Some(if pt <= 12 { two } else { one })),
		),
		(
			"2-bit up to 14 pt only (no 18 or 24 pt strikes)",
			Box::new(move |_, _, pt| (pt <= 14).then_some(two)),
		),
		(
			"2-bit all sizes, Mono without italic and bold italic",
			Box::new(move |face, style, _| (face != 2 || style < 2).then_some(two)),
		),
		(
			"2-bit up to 14 pt, 1-bit above, Mono without italic and bold italic",
			Box::new(move |face, style, pt| {
				(face != 2 || style < 2).then_some(if pt <= 14 { two } else { one })
			}),
		),
	]
}

fn write(path: &Path, data: &[u8]) -> Result<(), String> {
	std::fs::write(path, data).map_err(|e| format!("{}: {e}", path.display()))
}

fn run(args: &[String]) -> Result<(), String> {
	let one_set = |args: &[String]| -> Result<FaceSet, String> {
		let id = value(args, "--set").unwrap_or("desk");
		faceset::by_id(id).ok_or_else(|| format!("unknown set {id}"))
	};
	if flag(args, "--sizes") {
		let mut text = String::new();
		for set in SETS {
			for (name, opts) in MODES {
				let bytes = build(&set, opts);
				text.push_str(&sizes::describe(&format!("{} / {name}", set.title), &bytes));
				text.push('\n');
			}
		}
		text.push_str("Mixed files (Desk set; hinted; bits per pixel by size and style)\n");
		for (name, f) in variants() {
			let bytes = fnt::write(&strikes_where(&faceset::DESK, f));
			let packed = sizes::deflated(&bytes);
			text.push_str(&format!(
				"  {name}: raw {} bytes ({:.1} KB), deflated {packed} bytes ({:.1} KB)\n",
				bytes.len(),
				bytes.len() as f64 / 1024.0,
				packed as f64 / 1024.0
			));
		}
		print!("{text}");
		if let Some(p) = optional_path(args, "--sizes") {
			write(&p, text.as_bytes())?;
		}
		return Ok(());
	}
	if let Some(dir) = value(args, "--sheets") {
		let dir = Path::new(dir);
		for set in SETS {
			for (name, opts) in MODES {
				let bytes = build(&set, opts);
				let font = Font::parse(&bytes).map_err(|e| format!("{e:?}"))?;
				let stem = name.replace([' ', '-'], "");
				let title = format!("{}: {name}", set.id);
				write(
					&dir.join(format!("dos-fnt-{}-{stem}.png", set.id)),
					&sheet::size_sheet(&font, &title),
				)?;
			}
			let bytes = build(
				&set,
				Opts {
					bpp: 2,
					hinted: true,
				},
			);
			let font = Font::parse(&bytes).map_err(|e| format!("{e:?}"))?;
			write(
				&dir.join(format!("dos-fnt-{}-styles.png", set.id)),
				&sheet::style_sheet(&font, &format!("{}: styles, 2-bit hinted", set.id)),
			)?;
		}
		return Ok(());
	}
	if flag(args, "--report") {
		let mut text = String::new();
		for set in SETS {
			let bytes = build(
				&set,
				Opts {
					bpp: 2,
					hinted: true,
				},
			);
			text.push_str(&report::windows_me_report(&set, &bytes, set.id));
			text.push('\n');
		}
		print!("{text}");
		if let Some(p) = optional_path(args, "--report") {
			write(&p, text.as_bytes())?;
		}
		return Ok(());
	}
	let out = value(args, "-o").ok_or(USAGE)?;
	let opts = Opts {
		bpp: match value(args, "--bpp") {
			None | Some("2") => 2,
			Some("1") => 1,
			Some(v) => return Err(format!("--bpp {v}: 1 or 2")),
		},
		hinted: !flag(args, "--unhinted"),
	};
	let bytes = build(&one_set(args)?, opts);
	write(Path::new(out), &bytes)?;
	println!(
		"{out}: {} bytes, sha-256 {}",
		bytes.len(),
		sizes::sha256(&bytes)
	);
	Ok(())
}

fn main() -> ExitCode {
	let args: Vec<String> = std::env::args().skip(1).collect();
	if args.is_empty() || flag(&args, "--help") {
		println!("{USAGE}");
		return ExitCode::SUCCESS;
	}
	match run(&args) {
		Ok(()) => ExitCode::SUCCESS,
		Err(e) => {
			eprintln!("quill-fntgen: {e}");
			ExitCode::FAILURE
		}
	}
}
