// emit-resources: writes win9x/resources/quill.json, the one description the Win32 .res writer reads, from the app's own menu tree.
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

fn main() {
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
		for (i, it) in items.iter().enumerate() {
			let label: String = it.label.chars().collect();
			let text = if it.accel.is_empty() {
				label
			} else {
				format!("{label}\t{}", it.accel)
			};
			let _ = writeln!(
				out,
				"\t\t\t{{ \"text\": {}, \"id\": {} }}{}",
				json_str(&text),
				id,
				if i + 1 < items.len() { "," } else { "" }
			);
			id += 1;
		}
		out.push_str("\t\t] }");
	}
	out.push_str("\n\t]\n}\n");
	print!("{out}");
}
