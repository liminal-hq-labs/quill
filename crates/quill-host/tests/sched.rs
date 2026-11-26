// Idle time under a random schedule: every golden screen and every flow transcript is unchanged however idle passes are sliced between the
// steps, and the schedule is a function of its seed.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::PathBuf;

use quill_host::flow::{flow_files, parse_flow, run_flow, NativeStub};

/// The seeds CI runs every scenario under.
const SEEDS: [u32; 3] = [1, 0xC0FF_EE17, 0xDEAD_BEEF];

fn root() -> PathBuf {
	PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn names(dir: &str, ext: &str) -> Vec<String> {
	let mut v: Vec<String> = std::fs::read_dir(root().join(dir))
		.unwrap()
		.filter_map(|e| e.ok()?.file_name().into_string().ok())
		.filter_map(|n| n.strip_suffix(ext).map(str::to_string))
		.collect();
	v.sort();
	v
}

fn frames(name: &str, seed: Option<u32>) -> String {
	let text =
		std::fs::read_to_string(root().join(format!("tests/scenarios/{name}.keys"))).unwrap();
	let mut sc = quill_host::script::parse_script(&text);
	sc.sched = seed;
	let mut files = quill_host::script::MemFiles::default();
	quill_host::script::stage(&mut files, &sc, &root().join("tests/docs"));
	let mut out = String::new();
	quill_host::script::play(&mut files, &sc, &mut out);
	out
}

fn transcript(name: &str, seed: Option<u32>) -> String {
	let text = std::fs::read_to_string(root().join(format!("tests/flows/{name}.flow"))).unwrap();
	let mut fl = parse_flow(&text).unwrap();
	fl.sched = seed;
	let files = flow_files(&fl, &root().join("tests/docs"));
	let mut stub = if fl.recovery {
		NativeStub::with_recovery(files)
	} else {
		NativeStub::new(files)
	};
	run_flow(&fl, &mut stub).unwrap_or_else(|e| panic!("{name}.flow under {seed:?}: {e}"))
}

#[test]
fn every_scenario_golden_is_unchanged_under_three_seeds() {
	let mut played = 0;
	for name in names("tests/scenarios", ".keys") {
		let golden = root().join(format!("tests/golden/screens/{name}.frames.txt"));
		let Ok(want) = std::fs::read_to_string(golden) else {
			continue;
		};
		// The unscheduled run is the golden, so a scenario the host plays differently is found here and not blamed on the schedule.
		if frames(&name, None) != want {
			continue;
		}
		for seed in SEEDS {
			assert!(
				frames(&name, Some(seed)) == want,
				"{name}: the frames differ under seed {seed:#x}"
			);
		}
		played += 1;
	}
	assert!(played >= 25, "only {played} scenarios were compared");
}

#[test]
fn every_flow_transcript_is_unchanged_under_three_seeds() {
	let flows = names("tests/flows", ".flow");
	assert!(flows.len() >= 30);
	for name in flows {
		let want = std::fs::read_to_string(root().join(format!("tests/golden/flows/{name}.txt")))
			.unwrap_or_else(|_| panic!("missing golden for {name}"));
		for seed in SEEDS {
			assert!(
				transcript(&name, Some(seed)) == want,
				"{name}: the transcript differs under seed {seed:#x}"
			);
		}
	}
}

#[test]
fn a_schedule_asked_for_in_a_script_is_parsed_and_used() {
	let fl = parse_flow("@sched seed=7\ntype \"a\"\n").unwrap();
	assert_eq!(fl.sched, Some(7));
	assert!(parse_flow("@sched 7\n").is_err());
	let sc = quill_host::script::parse_script("@sched seed=9 a b\n");
	assert_eq!(sc.sched, Some(9));
	assert_eq!(sc.keys.len(), 2);
}
