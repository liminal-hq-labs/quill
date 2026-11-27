// The link flags that only the program has: its entry point, a fixed base with no relocations, and its resources.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

fn main() {
	// A DLL in the same workspace must not get these (a fixed program image, `start` as its entry, the program's icons), so they are
	// not in `.cargo/config.toml`'s shared flags; `-bins` keeps them off a library.
	println!("cargo:rustc-link-arg-bins=/ENTRY:start");
	// Windows 9x predates ASLR; load at the preferred base with no relocations.
	println!("cargo:rustc-link-arg-bins=/FIXED");
	// Menu, icon and version information, written by tools/res_pack.py (build.sh makes it).
	println!("cargo:rustc-link-arg-bins=resources/quill.res");
	println!("cargo:rerun-if-changed=../resources/quill.res");
	println!("cargo:rerun-if-changed=build.rs");
}
