// A test program: its own entry point and a fixed base, as the shell's.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

fn main() {
	println!("cargo:rustc-link-arg-bins=/ENTRY:start");
	println!("cargo:rustc-link-arg-bins=/FIXED");
	println!("cargo:rerun-if-changed=build.rs");
}
