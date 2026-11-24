// hlpdump: lists what is inside a WinHelp 3.1 file.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// usage: hlpdump FILE.HLP
use quill_help::read::Hlp;

fn main() {
	let Some(path) = std::env::args().nth(1) else {
		eprintln!("usage: hlpdump FILE.HLP");
		std::process::exit(2);
	};
	let data = std::fs::read(&path).unwrap_or_else(|e| {
		eprintln!("hlpdump: {path}: {e}");
		std::process::exit(1);
	});
	let h = Hlp::new(&data).unwrap_or_else(|e| {
		eprintln!("hlpdump: {e}");
		std::process::exit(1);
	});
	println!("directory:");
	match h.directory() {
		Ok(d) => {
			for (n, off) in d {
				println!("  {n:<16} at {off:#x}");
			}
		}
		Err(e) => println!("  {e}"),
	}
	match h.system() {
		Ok(s) => println!("system: version 1.{} flags {:#x} title {:?} copyright {:?} contents {:?}", s.minor, s.flags, s.title, s.copyright, s.contents),
		Err(e) => println!("system: {e}"),
	}
	match h.contexts() {
		Ok(c) => {
			println!("contexts: {}", c.len());
			for (h, o) in c {
				println!("  hash {:08X} topic {o:#x}", h as u32);
			}
		}
		Err(e) => println!("contexts: {e}"),
	}
	if std::env::args().nth(2).as_deref() == Some("--raw") {
		if let Ok(bs) = h.topic_blocks() {
			for (i, b) in bs.iter().enumerate() {
				println!("block {i}: {} bytes", b.len());
				let mut at = 0usize;
				while at + 0x15 <= b.len() {
					let size = u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]]) as usize;
					let end = (at + size).min(b.len());
					let hex: Vec<String> = b[at..end.min(at + 80)].iter().map(|x| format!("{x:02x}")).collect();
					println!("  rec at {at:#x} type {:#x} size {size}: {}", b[at + 0x14], hex.join(" "));
					let next = u32::from_le_bytes([b[at + 12], b[at + 13], b[at + 14], b[at + 15]]);
					if next == 0xFFFF_FFFF || next < 0x0C {
						break;
					}
					at = (next - 0x0C) as usize & 0x3FFF;
				}
			}
		}
		return;
	}
	match h.topics() {
		Ok(ts) => {
			for t in ts {
				println!("topic {:#010x} {:?}", t.offset, t.title);
				for p in &t.paragraphs {
					println!("    {}", p.replace('\n', "\\n"));
				}
			}
		}
		Err(e) => println!("topics: {e}"),
	}
}
