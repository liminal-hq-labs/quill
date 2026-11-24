// quill: the command-line tool.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::io::{Read, Write};
use std::process::ExitCode;

use quill_cli::{cat, convert, explain, info, load, Format, Loaded};

const USAGE: &str = "Usage: quill COMMAND [OPTIONS] FILE...

Commands:
  cat [--ansi] FILE            write the text of a document, one line to a paragraph
  convert [--to FMT] IN OUT    write IN as OUT; the format is OUT's extension (.rtf, .txt, else .qld) or --to qld|rtf|txt
  info FILE...                 describe documents: format, size, counts, fonts, paper and headers

A FILE of - is read from standard input, and an OUT of - is written to standard output. Documents are read by content:
.QLD, RTF or plain text.";

fn fail(code: u8, msg: impl std::fmt::Display) -> ExitCode {
	eprintln!("quill: {msg}");
	ExitCode::from(code)
}

fn read_input(name: &str) -> Result<Vec<u8>, String> {
	let mut bytes = Vec::new();
	let r = if name == "-" {
		std::io::stdin().lock().read_to_end(&mut bytes).map(|_| ())
	} else {
		std::fs::read(name).map(|b| bytes = b)
	};
	r.map(|()| bytes).map_err(|e| format!("{name}: {e}"))
}

fn open(name: &str) -> Result<Loaded, String> {
	let label = if name == "-" { "standard input" } else { name };
	load(&read_input(name)?).map_err(|e| format!("{label} {}", explain(e)))
}

fn main() -> ExitCode {
	let mut args = std::env::args().skip(1);
	let Some(cmd) = args.next() else {
		eprintln!("{USAGE}");
		return ExitCode::from(2);
	};
	let rest: Vec<String> = args.collect();
	match cmd.as_str() {
		"-h" | "--help" | "help" => {
			println!("{USAGE}");
			ExitCode::SUCCESS
		}
		"-V" | "--version" => {
			println!("quill {}", env!("CARGO_PKG_VERSION"));
			ExitCode::SUCCESS
		}
		"cat" => run_cat(&rest),
		"convert" => run_convert(&rest),
		"info" => run_info(&rest),
		other => fail(2, format!("unknown command {other}; try quill --help")),
	}
}

fn run_cat(args: &[String]) -> ExitCode {
	let mut ansi = false;
	let mut files = Vec::new();
	for a in args {
		match a.as_str() {
			"--ansi" => ansi = true,
			s if s.starts_with("--") => return fail(2, format!("cat: unknown option {s}")),
			f => files.push(f),
		}
	}
	let [file] = files[..] else {
		return fail(2, "cat: give one FILE");
	};
	match open(file) {
		Ok(l) => {
			let text = cat(&l, ansi);
			match std::io::stdout().lock().write_all(text.as_bytes()) {
				Ok(()) => ExitCode::SUCCESS,
				// A closed pipe (`quill cat x | head`) is not an error worth a message.
				Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => ExitCode::SUCCESS,
				Err(e) => fail(1, e),
			}
		}
		Err(e) => fail(1, e),
	}
}

fn run_convert(args: &[String]) -> ExitCode {
	let mut to = None;
	let mut files = Vec::new();
	let mut it = args.iter();
	while let Some(a) = it.next() {
		match a.as_str() {
			"--to" => match it.next().and_then(|w| Format::from_word(w)) {
				Some(f) => to = Some(f),
				None => return fail(2, "convert: --to takes qld, rtf or txt"),
			},
			s if s.starts_with("--") => return fail(2, format!("convert: unknown option {s}")),
			f => files.push(f),
		}
	}
	let [input, output] = files[..] else {
		return fail(2, "convert: give IN and OUT");
	};
	let l = match open(input) {
		Ok(l) => l,
		Err(e) => return fail(1, e),
	};
	let format = to.unwrap_or_else(|| Format::from_name(output));
	let bytes = match convert(&l, format) {
		Ok(b) => b,
		Err(e) => return fail(1, format!("{output} {}", explain(e))),
	};
	let r = if output == "-" {
		std::io::stdout().lock().write_all(&bytes)
	} else {
		std::fs::write(output, &bytes)
	};
	match r {
		Ok(()) => ExitCode::SUCCESS,
		Err(e) => fail(1, format!("{output}: {e}")),
	}
}

fn run_info(args: &[String]) -> ExitCode {
	if args.is_empty() || args.iter().any(|a| a.starts_with("--")) {
		return fail(2, "info: give one or more FILEs");
	}
	let mut code = ExitCode::SUCCESS;
	for (i, f) in args.iter().enumerate() {
		match open(f) {
			Ok(l) => {
				if i > 0 {
					println!();
				}
				print!("{}", info(f, &l));
			}
			Err(e) => code = fail(1, e),
		}
	}
	code
}
