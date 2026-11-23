// make-docs: writes the committed test documents from their builders (tests/docs/*.qld).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::process::ExitCode;

use quill_host::docs;
use quill_host::HostStore;

fn main() -> ExitCode {
	let Some(dir) = std::env::args().nth(1) else {
		eprintln!("usage: make-docs DIR");
		return ExitCode::from(2);
	};
	let mut s = HostStore::new();
	match docs::letter_bytes(&mut s) {
		Ok(b) => {
			if let Err(e) = std::fs::write(format!("{dir}/letter.qld"), b) {
				eprintln!("make-docs: {e}");
				return ExitCode::from(1);
			}
		}
		Err(e) => {
			eprintln!("make-docs: {e:?}");
			return ExitCode::from(1);
		}
	}
	let words = std::fs::read("dict/test-words.txt").unwrap_or_default();
	if let Err(e) = std::fs::write(format!("{dir}/en_ca.dic"), docs::test_dictionary(&words)) {
		eprintln!("make-docs: {e}");
		return ExitCode::from(1);
	}
	match docs::plain_bytes(&mut s) {
		Ok(b) => {
			if let Err(e) = std::fs::write(format!("{dir}/plain.qld"), b) {
				eprintln!("make-docs: {e}");
				return ExitCode::from(1);
			}
			ExitCode::SUCCESS
		}
		Err(e) => {
			eprintln!("make-docs: {e:?}");
			ExitCode::from(1)
		}
	}
}
