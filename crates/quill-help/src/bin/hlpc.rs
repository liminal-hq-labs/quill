// hlpc: compiles a `.qhm` help source into a WinHelp 3.1 file.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// usage: hlpc SOURCE.qhm OUT.HLP
fn main() {
	let args: Vec<String> = std::env::args().collect();
	if args.len() != 3 {
		eprintln!("usage: hlpc SOURCE.qhm OUT.HLP");
		std::process::exit(2);
	}
	let src = std::fs::read_to_string(&args[1]).unwrap_or_else(|e| {
		eprintln!("hlpc: {}: {e}", args[1]);
		std::process::exit(1);
	});
	let help = quill_help::qhm::parse(&src)
		.and_then(|h| quill_help::write::build(&h))
		.unwrap_or_else(|e| {
			eprintln!("hlpc: {}: {e}", args[1]);
			std::process::exit(1);
		});
	if let Err(e) = std::fs::write(&args[2], &help) {
		eprintln!("hlpc: {}: {e}", args[2]);
		std::process::exit(1);
	}
	println!("hlpc: {} ({} bytes)", args[2], help.len());
}
