// The generated file matches dialogs.json, and the IDs match what the shells and the packers use.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::BTreeSet;
use std::path::PathBuf;

use quill_dialogs::{style, Class, ALL};
use quill_host::dialoggen::{generate, parse, Json};

fn root() -> PathBuf {
	PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(path: &str) -> String {
	std::fs::read_to_string(root().join(path)).unwrap_or_else(|e| panic!("{path}: {e}"))
}

#[test]
fn generated_rs_is_what_dialogs_json_generates() {
	let want = generate(&read("assets/dialogs/dialogs.json")).unwrap();
	let file = root().join("crates/quill-dialogs/src/generated.rs");
	if std::env::var_os("QUILL_UPDATE_GOLDEN").is_some() {
		std::fs::write(&file, &want).unwrap();
		return;
	}
	assert_eq!(
		std::fs::read_to_string(&file).unwrap(),
		want,
		"generated.rs differs from dialogs.json; QUILL_UPDATE_GOLDEN=1 or `emit-resources --dialogs` rewrites it"
	);
}

#[test]
fn the_constants_hold_what_the_json_holds() {
	let Json::Obj(top) = parse(&read("assets/dialogs/dialogs.json")).unwrap() else {
		panic!("not an object");
	};
	let Json::Arr(dialogs) = &top[0].1 else {
		panic!("no dialogs");
	};
	assert_eq!(dialogs.len(), ALL.len());
	let num = |v: &Json, k: &str| match v {
		Json::Obj(o) => o.iter().find_map(|(n, x)| match x {
			Json::Num(n2) if n == k => Some(*n2),
			_ => None,
		}),
		_ => None,
	};
	let string = |v: &Json, k: &str| match v {
		Json::Obj(o) => o.iter().find_map(|(n, x)| match x {
			Json::Str(s) if n == k => Some(s.clone()),
			_ => None,
		}),
		_ => None,
	};
	for (j, d) in dialogs.iter().zip(ALL) {
		assert_eq!(num(j, "id"), Some(i64::from(d.id)));
		assert_eq!(string(j, "caption").unwrap(), d.caption);
		assert_eq!(string(j, "name").unwrap(), d.name);
		for (k, v) in [("x", d.x), ("y", d.y), ("cx", d.cx), ("cy", d.cy)] {
			assert_eq!(num(j, k), Some(i64::from(v)), "{} {k}", d.name);
		}
		let Json::Obj(o) = j else { panic!() };
		let Some((_, Json::Arr(items))) = o.iter().find(|(k, _)| k == "items") else {
			panic!()
		};
		assert_eq!(items.len(), d.items.len(), "{}", d.name);
		for (ji, it) in items.iter().zip(d.items) {
			assert_eq!(num(ji, "id"), Some(i64::from(it.id)));
			assert_eq!(string(ji, "text").unwrap_or_default(), it.text);
			assert_eq!(num(ji, "icon").unwrap_or(0), i64::from(it.icon));
			for (k, v) in [("x", it.x), ("y", it.y), ("cx", it.cx), ("cy", it.cy)] {
				assert_eq!(
					num(ji, k),
					Some(i64::from(v)),
					"{} control {} {k}",
					d.name,
					it.id
				);
			}
		}
	}
}

#[test]
fn every_dialog_has_one_default_button_a_close_and_unique_ids_and_mnemonics() {
	for d in ALL {
		let ids: BTreeSet<u16> = d.items.iter().map(|i| i.id).collect();
		assert_eq!(
			ids.len(),
			d.items.len(),
			"{}: a control ID is used twice",
			d.name
		);
		let defaults: Vec<_> = d
			.items
			.iter()
			.filter(|i| i.has(style::DEFPUSHBUTTON))
			.collect();
		assert_eq!(defaults.len(), 1, "{}", d.name);
		assert_eq!(defaults[0].id, d.default_button);
		assert!(
			d.item(1).is_some() || d.item(2).is_some(),
			"{} has no OK, Cancel or Close",
			d.name
		);
		let mut seen = BTreeSet::new();
		for i in d
			.items
			.iter()
			.filter(|i| i.class == Class::Button || i.class == Class::Static)
		{
			if let Some(m) = i.mnemonic() {
				assert!(
					seen.insert(m),
					"{}: mnemonic {} is used twice",
					d.name,
					m as char
				);
			}
		}
	}
}

#[test]
fn mnemonics_skip_doubled_ampersands() {
	let find = quill_dialogs::find::DIALOG;
	assert_eq!(
		find.item(quill_dialogs::find::WHAT_LABEL)
			.unwrap()
			.mnemonic(),
		Some(b'n')
	);
	assert_eq!(
		find.item(quill_dialogs::find::CLOSE).unwrap().mnemonic(),
		None
	);
	let amp = quill_dialogs::Item {
		text: "A && &B",
		..*find.item(quill_dialogs::find::CLOSE).unwrap()
	};
	assert_eq!(amp.mnemonic(), Some(b'b'));
}

/// The numbers after `needle` up to the next non-digit, for each place `needle` appears.
fn numbers_after(text: &str, needle: &str) -> Vec<u16> {
	let mut out = Vec::new();
	let mut rest = text;
	while let Some(at) = rest.find(needle) {
		rest = &rest[at + needle.len()..];
		let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
		if let Ok(n) = digits.parse() {
			out.push(n);
		}
	}
	out
}

/// Every control ID a dialog procedure names with a literal: the argument after `dlg`, a `id == N` test, a match arm `N =>`, `N | M =>`
/// or `N if ... =>`, the list `[a, b, c]` the Find procedure hides, and the ranges of the radio groups.
fn control_literals(src: &str) -> BTreeSet<u16> {
	let mut ids: BTreeSet<u16> = BTreeSet::new();
	ids.extend(numbers_after(src, "dlg, "));
	ids.extend(numbers_after(src, "id == "));
	for line in src.lines() {
		let t = line.trim_start();
		if let Some(arrow) = t.find(" =>") {
			let head = &t[..arrow];
			let head = head.split(" if ").next().unwrap_or(head);
			let parts: Vec<&str> = head.split(" | ").collect();
			if parts
				.iter()
				.all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
			{
				ids.extend(parts.iter().filter_map(|p| p.parse::<u16>().ok()));
			}
		}
		if let Some(at) = t.find("for id in [") {
			let list = &t[at + 11..];
			let list = list.split(']').next().unwrap_or("");
			ids.extend(list.split(',').filter_map(|p| p.trim().parse::<u16>().ok()));
		}
		if let Some(at) = t.find("CheckRadioButton(dlg, ") {
			let args = t[at + 22..].split(')').next().unwrap_or("");
			ids.extend(args.split(',').filter_map(|p| p.trim().parse::<u16>().ok()));
		}
		if let Some(at) = t.find("(11..=14)").or_else(|| t.find("(15..=18)")) {
			let r = &t[at + 1..at + 8];
			let (a, b) = r.split_once("..=").unwrap();
			let (a, b): (u16, u16) = (a.parse().unwrap(), b.parse().unwrap());
			ids.extend(a..=b);
		}
	}
	ids.remove(&0);
	ids
}

fn shell_files() -> [(&'static str, String); 2] {
	[
		("win16", read("win16/examples/quill/dialogs.rs")),
		("win9x", read("win9x/quill-win9x/src/dialogs.rs")),
	]
}

#[test]
fn both_shells_use_the_dialog_ids_that_are_generated() {
	for (shell, src) in shell_files() {
		let mut found = 0;
		for line in src.lines() {
			let Some(rest) = line.trim().strip_prefix("const DLG_") else {
				continue;
			};
			let (name, value) = rest.split_once(':').unwrap();
			let n: u16 = value
				.rsplit('=')
				.next()
				.unwrap()
				.trim()
				.trim_end_matches(';')
				.parse()
				.unwrap();
			let d = ALL
				.iter()
				.find(|d| d.name.eq_ignore_ascii_case(name))
				.unwrap_or_else(|| panic!("{shell}: DLG_{name} names no dialog"));
			assert_eq!(d.id, n, "{shell}: DLG_{name}");
			found += 1;
		}
		assert!(found >= 4, "{shell} names {found} dialogs");
	}
}

#[test]
fn every_control_id_the_shells_write_as_a_number_is_a_generated_one() {
	let mut known: BTreeSet<u16> = ALL
		.iter()
		.flat_map(|d| d.items.iter().map(|i| i.id))
		.collect();
	// IDYES and IDNO, which a message box returns and Win16's Save changes question tests.
	known.extend([6, 7]);
	for (shell, src) in shell_files() {
		let used = control_literals(&src);
		assert!(used.len() >= 20, "{shell}: found only {used:?}");
		let strangers: Vec<_> = used.difference(&known).collect();
		assert!(
			strangers.is_empty(),
			"{shell} uses control IDs no dialog has: {strangers:?}"
		);
	}
}

/// The controls the shells read or write, by name. The numbers are what the shells have in their source today; a change in
/// dialogs.json that moves one fails here until the shells follow.
#[test]
fn the_controls_the_shells_rely_on_have_the_ids_they_use() {
	use quill_dialogs::{about, find, fonts, spelling, tabs};
	let relied: &[(&str, u16, u16)] = &[
		("about version", about::VERSION, 22),
		("about platform", about::PLATFORM, 25),
		("about memory", about::MEMORY, 26),
		("about ok", about::OK, 1),
		("find what", find::WHAT, 10),
		("find with", find::WITH, 11),
		("find match case", find::MATCH_CASE, 12),
		("find whole word", find::WHOLE_WORD, 13),
		("find next", find::FIND_NEXT, 100),
		("find replace", find::REPLACE, 101),
		("find replace all", find::REPLACE_ALL, 102),
		("find with label", find::WITH_LABEL, 31),
		("tabs position", tabs::POSITION, 10),
		("tabs left", tabs::LEFT, 11),
		("tabs decimal", tabs::DECIMAL, 14),
		("tabs leader none", tabs::LEADER_NONE, 15),
		("tabs leader underline", tabs::LEADER_UNDERLINE, 18),
		("tabs stops", tabs::STOPS, 19),
		("tabs set", tabs::SET, 20),
		("tabs clear", tabs::CLEAR, 21),
		("tabs clear all", tabs::CLEAR_ALL, 22),
		("spelling word", spelling::WORD, 20),
		("spelling context", spelling::CONTEXT, 21),
		("spelling change to", spelling::CHANGE_TO, 10),
		("spelling suggestions", spelling::SUGGESTIONS, 11),
		("spelling ignore", spelling::IGNORE, 100),
		("spelling ignore all", spelling::IGNORE_ALL, 101),
		("spelling change", spelling::CHANGE, 102),
		("spelling change all", spelling::CHANGE_ALL, 103),
		("spelling add", spelling::ADD, 104),
		("fonts font", fonts::FONT, 10),
		("fonts size", fonts::SIZE, 11),
		("fonts bold", fonts::BOLD, 15),
		("fonts strikeout", fonts::STRIKEOUT, 18),
	];
	let (w16, w9x) = (
		read("win16/examples/quill/dialogs.rs"),
		read("win9x/quill-win9x/src/dialogs.rs"),
	);
	let (u16s, u9x) = (control_literals(&w16), control_literals(&w9x));
	for (what, got, want) in relied {
		assert_eq!(got, want, "{what}");
	}
	// Each shell writes the IDs of the dialogs it drives: Win16 has About, Find, Tabs and Spelling, Win95 also Fonts.
	for (what, id) in [
		("find what", find::WHAT),
		("tabs stops", tabs::STOPS),
		("spelling suggestions", spelling::SUGGESTIONS),
		("spelling add", spelling::ADD),
	] {
		assert!(u16s.contains(&id), "win16 never names {what} ({id})");
		assert!(u9x.contains(&id), "win9x never names {what} ({id})");
	}
}

#[test]
fn the_win16_resource_check_lists_every_dialog() {
	let build = read("win16/build.sh");
	let listed: BTreeSet<u16> = numbers_after(&build, " 5:").into_iter().collect();
	let have: BTreeSet<u16> = ALL.iter().map(|d| d.id).collect();
	assert_eq!(
		listed, have,
		"win16/build.sh's check_resources list (type 5, RT_DIALOG) and dialogs.json differ"
	);
}

#[test]
fn the_generator_reads_escapes_and_refuses_what_it_does_not_know() {
	assert_eq!(
		parse(r#"{"a": ["café", "😀", "q\"\\\n", -3, true, null]}"#).unwrap(),
		Json::Obj(vec![(
			"a".into(),
			Json::Arr(vec![
				Json::Str("café".into()),
				Json::Str("\u{1f600}".into()),
				Json::Str("q\"\\\n".into()),
				Json::Num(-3),
				Json::Bool(true),
				Json::Null,
			])
		)])
	);
	assert!(parse("{\"a\": 1,}").is_err());
	assert!(parse("{} x").is_err());
	let one = |extra: &str| {
		format!(
			r#"{{"dialogs": [{{"id": 1, "name": "d", "caption": "D", "x": 0, "y": 0, "cx": 1, "cy": 1, "items": [{extra}]}}]}}"#
		)
	};
	let item = |class: &str, id: u32, more: &str| {
		format!(r#"{{"class": "{class}", "id": {id}, "x": 0, "y": 0, "cx": 1, "cy": 1 {more}}}"#)
	};
	assert!(generate(&one(&item("static", 1, ""))).is_ok());
	assert!(generate(&one(&item("tree", 1, "")))
		.unwrap_err()
		.contains("unknown class"));
	assert!(
		generate(&one(&item("button", 1, r#", "style": ["sparkly"]"#)))
			.unwrap_err()
			.contains("unknown style")
	);
	assert!(generate(&one(&format!(
		"{}, {}",
		item("static", 1, ""),
		item("static", 1, "")
	)))
	.unwrap_err()
	.contains("used twice"));
	assert!(generate(&one(&item("static", 1, r#", "name": "id""#)))
		.unwrap_err()
		.contains("reserved"));
	assert!(generate(&one(&item("static", 1, r#", "name": "Bad Name""#))).is_err());
}
