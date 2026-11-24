// dialoggen: turns assets/dialogs/dialogs.json into the Rust source of quill-dialogs' generated.rs.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The JSON has no schema beyond what the Win16 and Win32 packers read, plus a `name` on each dialog and on each control the code
//! refers to. A control with no name gets no constant. Anything the generator does not understand is an error, so a new style or class
//! has to be added here and in `quill-dialogs` together.

use std::collections::BTreeSet;
use std::fmt::Write;

/// A parsed JSON value. Only what the dialog file uses: integers, strings, arrays and objects.
#[derive(Debug, Clone, PartialEq)]
pub enum Json {
	Null,
	Bool(bool),
	Num(i64),
	Str(String),
	Arr(Vec<Json>),
	Obj(Vec<(String, Json)>),
}

impl Json {
	fn get(&self, key: &str) -> Option<&Json> {
		match self {
			Json::Obj(o) => o.iter().find(|(k, _)| k == key).map(|(_, v)| v),
			_ => None,
		}
	}
}

struct Parser<'a> {
	b: &'a [u8],
	at: usize,
}

impl Parser<'_> {
	fn ws(&mut self) {
		while matches!(self.b.get(self.at), Some(b' ' | b'\t' | b'\r' | b'\n')) {
			self.at += 1;
		}
	}

	fn err<T>(&self, what: &str) -> Result<T, String> {
		Err(format!("dialogs.json: {what} at byte {}", self.at))
	}

	fn eat(&mut self, c: u8) -> Result<(), String> {
		self.ws();
		if self.b.get(self.at) == Some(&c) {
			self.at += 1;
			Ok(())
		} else {
			self.err(&format!("expected '{}'", c as char))
		}
	}

	fn value(&mut self) -> Result<Json, String> {
		self.ws();
		match self.b.get(self.at).copied() {
			Some(b'{') => {
				self.at += 1;
				let mut o = Vec::new();
				self.ws();
				if self.b.get(self.at) == Some(&b'}') {
					self.at += 1;
					return Ok(Json::Obj(o));
				}
				loop {
					self.ws();
					let Json::Str(k) = self.value()? else {
						return self.err("expected a key");
					};
					self.eat(b':')?;
					o.push((k, self.value()?));
					self.ws();
					match self.b.get(self.at) {
						Some(b',') => self.at += 1,
						Some(b'}') => {
							self.at += 1;
							return Ok(Json::Obj(o));
						}
						_ => return self.err("expected ',' or '}'"),
					}
				}
			}
			Some(b'[') => {
				self.at += 1;
				let mut a = Vec::new();
				self.ws();
				if self.b.get(self.at) == Some(&b']') {
					self.at += 1;
					return Ok(Json::Arr(a));
				}
				loop {
					a.push(self.value()?);
					self.ws();
					match self.b.get(self.at) {
						Some(b',') => self.at += 1,
						Some(b']') => {
							self.at += 1;
							return Ok(Json::Arr(a));
						}
						_ => return self.err("expected ',' or ']'"),
					}
				}
			}
			Some(b'"') => {
				self.at += 1;
				let mut s = String::new();
				// UTF-16 units from \u escapes, held until something else arrives so a surrogate pair stays whole.
				let mut units: Vec<u16> = Vec::new();
				loop {
					let Some(&c) = self.b.get(self.at) else {
						return self.err("unterminated string");
					};
					self.at += 1;
					if c == b'\\' && self.b.get(self.at) == Some(&b'u') {
						let hex = self
							.b
							.get(self.at + 1..self.at + 5)
							.and_then(|h| std::str::from_utf8(h).ok());
						let Some(v) = hex.and_then(|h| u16::from_str_radix(h, 16).ok()) else {
							return self.err("bad \\u escape");
						};
						self.at += 5;
						units.push(v);
						continue;
					}
					push_units(&mut s, &mut units);
					match c {
						b'"' => return Ok(Json::Str(s)),
						b'\\' => {
							let Some(&e) = self.b.get(self.at) else {
								return self.err("unterminated escape");
							};
							self.at += 1;
							match e {
								b'"' => s.push('"'),
								b'\\' => s.push('\\'),
								b'/' => s.push('/'),
								b'n' => s.push('\n'),
								b't' => s.push('\t'),
								b'r' => s.push('\r'),
								_ => return self.err("unknown escape"),
							}
						}
						_ => {
							// Copy a whole UTF-8 sequence.
							let start = self.at - 1;
							let mut end = self.at;
							while self.b.get(end).is_some_and(|x| x & 0xC0 == 0x80) {
								end += 1;
							}
							match std::str::from_utf8(self.b.get(start..end).unwrap_or(&[])) {
								Ok(t) => s.push_str(t),
								Err(_) => return self.err("invalid UTF-8"),
							}
							self.at = end;
						}
					}
				}
			}
			Some(b't') if self.b.get(self.at..self.at + 4) == Some(b"true") => {
				self.at += 4;
				Ok(Json::Bool(true))
			}
			Some(b'f') if self.b.get(self.at..self.at + 5) == Some(b"false") => {
				self.at += 5;
				Ok(Json::Bool(false))
			}
			Some(b'n') if self.b.get(self.at..self.at + 4) == Some(b"null") => {
				self.at += 4;
				Ok(Json::Null)
			}
			Some(b'-' | b'0'..=b'9') => {
				let start = self.at;
				self.at += 1;
				while matches!(self.b.get(self.at), Some(b'0'..=b'9')) {
					self.at += 1;
				}
				let t =
					std::str::from_utf8(self.b.get(start..self.at).unwrap_or(&[])).unwrap_or("");
				match t.parse() {
					Ok(n) => Ok(Json::Num(n)),
					Err(_) => self.err("bad number"),
				}
			}
			_ => self.err("unexpected character"),
		}
	}
}

fn push_units(s: &mut String, units: &mut Vec<u16>) {
	for c in char::decode_utf16(units.drain(..)) {
		s.push(c.unwrap_or('\u{fffd}'));
	}
}

/// Parses JSON text.
pub fn parse(text: &str) -> Result<Json, String> {
	let mut p = Parser {
		b: text.as_bytes(),
		at: 0,
	};
	let v = p.value()?;
	p.ws();
	if p.at != p.b.len() {
		return p.err("trailing text");
	}
	Ok(v)
}

const CLASSES: [(&str, &str); 6] = [
	("static", "Static"),
	("button", "Button"),
	("edit", "Edit"),
	("listbox", "ListBox"),
	("combobox", "ComboBox"),
	("scrollbar", "ScrollBar"),
];

const STYLES: [&str; 14] = [
	"pushbutton",
	"defpushbutton",
	"autocheckbox",
	"autoradiobutton",
	"groupbox",
	"icon",
	"tabstop",
	"group",
	"border",
	"vscroll",
	"autohscroll",
	"notify",
	"dropdown",
	"dropdownlist",
];

fn rust_str(s: &str) -> String {
	let mut o = String::from("\"");
	for c in s.chars() {
		match c {
			'"' => o.push_str("\\\""),
			'\\' => o.push_str("\\\\"),
			' '..='~' => o.push(c),
			c => {
				let _ = write!(o, "\\u{{{:x}}}", c as u32);
			}
		}
	}
	o.push('"');
	o
}

fn num(v: &Json, key: &str, what: &str) -> Result<i64, String> {
	match v.get(key) {
		Some(Json::Num(n)) => Ok(*n),
		_ => Err(format!("{what}: `{key}` must be a number")),
	}
}

fn text(v: &Json, key: &str) -> String {
	match v.get(key) {
		Some(Json::Str(s)) => s.clone(),
		_ => String::new(),
	}
}

fn ident(name: &str, what: &str) -> Result<String, String> {
	let ok = !name.is_empty()
		&& name
			.bytes()
			.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
		&& !name.starts_with(|c: char| c.is_ascii_digit());
	if ok {
		Ok(name.to_string())
	} else {
		Err(format!("{what}: `{name}` is not a lower-case name"))
	}
}

/// Generates the Rust source of `quill-dialogs/src/generated.rs` from the text of `dialogs.json`.
pub fn generate(json: &str) -> Result<String, String> {
	let root = parse(json)?;
	let Some(Json::Arr(dialogs)) = root.get("dialogs") else {
		return Err("dialogs.json: no `dialogs` array".into());
	};
	let mut o = String::new();
	o.push_str("// The dialogs of assets/dialogs/dialogs.json as constants.\n//\n// (c) Copyright 2026 Liminal HQ, Scott Morris\n// SPDX-License-Identifier: Apache-2.0 OR MIT\n\n");
	o.push_str("// Generated by `cargo run -p quill-host --bin emit-resources -- --dialogs`. Do not edit: change dialogs.json and regenerate.\n\n");
	let mut dialog_ids = BTreeSet::new();
	let mut dialog_names = BTreeSet::new();
	let mut order = Vec::new();
	for d in dialogs {
		let id = num(d, "id", "a dialog")?;
		let name = ident(&text(d, "name"), &format!("dialog {id}"))?;
		let what = format!("dialog `{name}`");
		if !dialog_ids.insert(id) || !dialog_names.insert(name.clone()) {
			return Err(format!("{what}: the ID or name is used twice"));
		}
		let Some(Json::Arr(items)) = d.get("items") else {
			return Err(format!("{what}: no `items`"));
		};
		let _ = writeln!(o, "pub mod {name} {{");
		let _ = writeln!(
			o,
			"\t/// The dialog's resource ID.\n\tpub const ID: u16 = {id};"
		);
		let mut ids = BTreeSet::new();
		let mut const_names = BTreeSet::new();
		let mut default = 0;
		let mut lines = String::new();
		for it in items {
			let iid = num(it, "id", &what)?;
			let cls = text(it, "class");
			let Some((_, variant)) = CLASSES.iter().find(|(n, _)| *n == cls) else {
				return Err(format!("{what}: control {iid}: unknown class `{cls}`"));
			};
			let mut bits = Vec::new();
			if let Some(Json::Arr(st)) = it.get("style") {
				for s in st {
					let Json::Str(s) = s else {
						return Err(format!("{what}: control {iid}: a style must be a name"));
					};
					if !STYLES.contains(&s.as_str()) {
						return Err(format!("{what}: control {iid}: unknown style `{s}`"));
					}
					if s == "defpushbutton" {
						default = iid;
					}
					bits.push(format!("crate::style::{}", s.to_ascii_uppercase()));
				}
			}
			if !ids.insert(iid) {
				return Err(format!("{what}: control ID {iid} is used twice"));
			}
			let id_expr = match it.get("name") {
				Some(Json::Str(n)) => {
					let n = ident(n, &format!("{what}: control {iid}"))?;
					let c = n.to_ascii_uppercase();
					if c == "ID" || c == "DIALOG" || !const_names.insert(c.clone()) {
						return Err(format!(
							"{what}: control name `{n}` is reserved or used twice"
						));
					}
					let _ = writeln!(o, "\tpub const {c}: u16 = {iid};");
					c
				}
				_ => iid.to_string(),
			};
			let icon = match it.get("icon") {
				Some(Json::Num(n)) => *n,
				_ => 0,
			};
			let style = if bits.is_empty() {
				"0".to_string()
			} else {
				bits.join(" | ")
			};
			let _ = writeln!(
				lines,
				"\t\t\tcrate::Item {{ class: crate::Class::{variant}, id: {id_expr}, text: {}, icon: {icon}, x: {}, y: {}, cx: {}, cy: {}, style: {style} }},",
				rust_str(&text(it, "text")),
				num(it, "x", &what)?,
				num(it, "y", &what)?,
				num(it, "cx", &what)?,
				num(it, "cy", &what)?,
			);
		}
		let _ = writeln!(o, "\n\t/// The dialog's description.\n\tpub const DIALOG: crate::Dialog = crate::Dialog {{");
		let _ = writeln!(
			o,
			"\t\tid: ID,\n\t\tname: {},\n\t\tcaption: {},\n\t\tx: {},\n\t\ty: {},\n\t\tcx: {},\n\t\tcy: {},\n\t\tdefault_button: {default},\n\t\titems: &[",
			rust_str(&name),
			rust_str(&text(d, "caption")),
			num(d, "x", &what)?,
			num(d, "y", &what)?,
			num(d, "cx", &what)?,
			num(d, "cy", &what)?,
		);
		o.push_str(&lines);
		o.push_str("\t\t],\n\t};\n}\n\n");
		order.push(name);
	}
	o.push_str(
		"/// Every dialog, in the order of the file.\npub const ALL: &[&crate::Dialog] = &[\n",
	);
	for n in &order {
		let _ = writeln!(o, "\t&{n}::DIALOG,");
	}
	o.push_str("];\n");
	Ok(o)
}
