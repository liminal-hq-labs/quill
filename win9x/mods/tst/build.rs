// The link flags that make this a module: its own entry, a base of its own, relocations kept, 512-byte file alignment, a map for fault
// addresses (W9X-132).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::env;
use std::path::PathBuf;

fn main() {
	// The test builds relink this module at the program's own base (0x00400000, always taken, so it must be relocated) with
	// QUILLTST_BASE; the name of the map follows QUILLTST_MAP so the variants keep a map each.
	let base = env::var("QUILLTST_BASE").unwrap_or_else(|_| "0x61F00000".to_string());
	let map = env::var("QUILLTST_MAP").unwrap_or_else(|_| "QUILLTST.MAP".to_string());
	// OUT_DIR is <target>/win9x/release/build/<package>-<hash>/out: the map goes beside the DLL, in <target>/win9x/release.
	let out = PathBuf::from(env::var("OUT_DIR").expect("cargo sets OUT_DIR"));
	let profile_dir = out
		.ancestors()
		.nth(3)
		.expect("OUT_DIR is three levels under the profile folder");
	println!("cargo:rustc-link-arg-cdylib=/ENTRY:QuillDllEntry");
	println!("cargo:rustc-link-arg-cdylib=/BASE:{base}");
	println!("cargo:rustc-link-arg-cdylib=/DYNAMICBASE:NO");
	println!("cargo:rustc-link-arg-cdylib=/FILEALIGN:512");
	println!("cargo:rustc-link-arg-cdylib=/MAP:{}", profile_dir.join(map).display());
	println!("cargo:rerun-if-env-changed=QUILLTST_BASE");
	println!("cargo:rerun-if-env-changed=QUILLTST_MAP");
	println!("cargo:rerun-if-changed=build.rs");
}
