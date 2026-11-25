// Flow scripts: each `tests/flows/*.flow` is played through the native stub, which answers the editor's dialogs through
// `quill_app::flow` as the Windows shells do, and its transcript is compared with `tests/golden/flows/`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::PathBuf;

use quill_host::flow::{flow_files, parse_flow, run_flow, NativeStub};

fn root() -> PathBuf {
	PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Plays `tests/flows/NAME.flow` and compares the transcript with `tests/golden/flows/NAME.txt`.
fn play(name: &str) {
	let text = std::fs::read_to_string(root().join(format!("tests/flows/{name}.flow"))).unwrap();
	let fl = parse_flow(&text).unwrap_or_else(|e| panic!("{name}.flow: {e}"));
	let mut stub = NativeStub::new(flow_files(&fl, &root().join("tests/docs")));
	let transcript = run_flow(&fl, &mut stub).unwrap_or_else(|e| panic!("{name}.flow: {e}"));
	let golden = root().join(format!("tests/golden/flows/{name}.txt"));
	if std::env::var_os("QUILL_UPDATE_GOLDEN").is_some() {
		std::fs::write(&golden, &transcript).unwrap();
		return;
	}
	let want = std::fs::read_to_string(&golden)
		.unwrap_or_else(|_| panic!("missing golden for {name}; run with QUILL_UPDATE_GOLDEN=1"));
	assert_eq!(
		want, transcript,
		"{name}: the transcript differs; QUILL_UPDATE_GOLDEN=1 rewrites it and the diff is reviewed like code"
	);
}

macro_rules! flows {
	($($test:ident => $name:literal,)*) => {
		$(#[test] fn $test() { play($name); })*

		/// Every script in the folder has a test above.
		#[test]
		fn every_script_is_listed() {
			let mut found: Vec<String> = std::fs::read_dir(root().join("tests/flows"))
				.unwrap()
				.filter_map(|e| e.ok()?.file_name().into_string().ok())
				.filter_map(|n| n.strip_suffix(".flow").map(str::to_string))
				.collect();
			found.sort();
			let mut listed = vec![$($name.to_string()),*];
			listed.sort();
			assert_eq!(found, listed);
		}
	};
}

flows! {
	new_document => "new",
	open_documents => "open",
	save_documents => "save",
	save_as_documents => "save-as",
	save_as_that_fails => "save-as-fails",
	save_that_fails_keeps_the_original => "save-fails-keeps-original",
	exit_with_unsaved_changes => "exit",
	exit_without_saving => "exit-discard",
	exit_with_a_cancelled_save_as => "exit-cancelled",
	new_with_a_cancelled_save_as => "new-cancelled",
	open_after_no_then_cancelled => "open-cancelled",
	recent_after_no_that_is_gone => "recent-cancelled",
	recent_files => "recent",
	find_and_find_next => "find",
	replace_and_replace_all => "replace",
	spelling_check => "spelling",
	tabs_dialog => "tabs",
	fonts_dialog => "fonts",
	insert_picture => "picture",
	print_dialog => "print",
}

/// A script that expects something the editor does not do fails, and says which line.
#[test]
fn a_wrong_expectation_names_its_line() {
	let fl = parse_flow("type \"x\"\ncmd New\nexpect ask \"Save the changes?\"\n").unwrap();
	let mut stub = NativeStub::new(Default::default());
	let err = run_flow(&fl, &mut stub).unwrap_err();
	assert!(
		err.starts_with("line 3: wanted \"Save the changes?\""),
		"{err}"
	);
}

#[test]
fn an_unexpected_notice_fails_the_script() {
	let fl = parse_flow("cmd Open\nchoose \"GONE.QLD\"\ndump\n").unwrap();
	let mut stub = NativeStub::new(Default::default());
	let err = run_flow(&fl, &mut stub).unwrap_err();
	assert!(err.contains("unexpected `< notice Status"), "{err}");
}
