// emit-resources: writes win9x/resources/quill.json, the one description the Win32 .res writer reads, from the app's own menu tree; with
// `--dialogs` it writes quill-dialogs' generated.rs from assets/dialogs/dialogs.json instead.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::fmt::Write;

fn json_str(s: &str) -> String {
	let mut o = String::from("\"");
	for c in s.chars() {
		match c {
			'"' => o.push_str("\\\""),
			'\\' => o.push_str("\\\\"),
			'\t' => o.push_str("\\t"),
			c if (c as u32) < 0x20 || (c as u32) > 0x7E => {
				let _ = write!(o, "\\u{:04x}", c as u32);
			}
			c => o.push(c),
		}
	}
	o.push('"');
	o
}

/// `--dialogs [--check]`: writes `crates/quill-dialogs/src/generated.rs`, or with `--check` only compares it and fails when it differs.
fn dialogs(check: bool) {
	let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
	let json = std::fs::read_to_string(root.join("assets/dialogs/dialogs.json"))
		.expect("read assets/dialogs/dialogs.json");
	let src = match quill_host::dialoggen::generate(&json) {
		Ok(s) => s,
		Err(e) => {
			eprintln!("emit-resources: {e}");
			std::process::exit(1);
		}
	};
	let file = root.join("crates/quill-dialogs/src/generated.rs");
	if check {
		if std::fs::read_to_string(&file).ok().as_deref() != Some(src.as_str()) {
			eprintln!("emit-resources: {} is out of date; run `cargo run -p quill-host --bin emit-resources -- --dialogs`", file.display());
			std::process::exit(1);
		}
		return;
	}
	std::fs::write(&file, src).expect("write generated.rs");
}

fn main() {
	let args: Vec<String> = std::env::args().skip(1).collect();
	if args.iter().any(|a| a == "--dialogs") {
		dialogs(args.iter().any(|a| a == "--check"));
		return;
	}
	let mut out = String::from("{\n\t\"version\": { \"file\": [0, 0, 0, 1], \"company\": \"Liminal HQ\", \"description\": \"Quill for Windows 95\", \"copyright\": \"(c) Copyright 2026 Liminal HQ, Scott Morris\", \"original\": \"QUILL32.EXE\", \"product\": \"Quill\" },\n\t\"icon\": \"quill\",\n\t\"menus\": [\n");
	let mut id = 100;
	let mut first_menu = true;
	for m in 0..quill_app::MENU_COUNT {
		let items = quill_app::menu_items(m);
		if items.is_empty() {
			continue;
		}
		if !first_menu {
			out.push_str(",\n");
		}
		first_menu = false;
		let title: String = quill_app::menu_title(m)
			.iter()
			.map(|b| char::from(*b))
			.collect();
		let _ = writeln!(out, "\t\t{{ \"title\": {}, \"items\": [", json_str(&title));
		// One line per entry, in the order the shell numbers its commands: a rule above an item is an entry of its own with no id, an
		// item with a submenu is a popup whose entries take the ids, and any other item takes the next id.
		let mut lines: Vec<String> = Vec::new();
		for it in items {
			if it.rule {
				lines.push("\t\t\t{ \"separator\": true }".to_string());
			}
			if it.sub.is_empty() {
				let label: String = it.label.chars().collect();
				let text = if it.accel.is_empty() {
					label
				} else {
					format!("{label}\t{}", it.accel)
				};
				lines.push(format!(
					"\t\t\t{{ \"text\": {}, \"id\": {} }}",
					json_str(&text),
					id
				));
				id += 1;
			} else {
				let mut sub = String::new();
				for (k, e) in it.sub.iter().enumerate() {
					let _ = write!(
						sub,
						"{}{{ \"text\": {}, \"id\": {} }}",
						if k == 0 { "" } else { ", " },
						json_str(e.label),
						id
					);
					id += 1;
				}
				lines.push(format!(
					"\t\t\t{{ \"text\": {}, \"items\": [ {} ] }}",
					json_str(it.label),
					sub
				));
			}
		}
		out.push_str(&lines.join(",\n"));
		out.push('\n');
		out.push_str("\t\t] }");
	}
	out.push_str("\n\t]\n}\n");
	print!("{out}");
}
