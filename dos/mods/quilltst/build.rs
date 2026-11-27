// Links quilltst as a module: it starts at `app_start` and has an `app_stop`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
include!("../link.rs");

fn main() {
	println!("cargo:rerun-if-env-changed=QUILL_BUILD_ID");
	emit_link_arguments("app_start", &["app_stop"]);
}
