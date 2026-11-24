// Embeds the manifest that `winui/tools/stage-runtime.sh` writes, so the exe finds the bundled Windows App Runtime.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::PathBuf;

fn main() {
	println!("cargo:rerun-if-env-changed=QUILL_WINUI_MANIFEST");
	if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
		return;
	}
	let Some(manifest) = std::env::var_os("QUILL_WINUI_MANIFEST").map(PathBuf::from) else {
		panic!("QUILL_WINUI_MANIFEST is not set; run winui/tools/stage-runtime.sh and use winui/tools/build.sh");
	};
	println!("cargo:rerun-if-changed={}", manifest.display());
	println!("cargo:rustc-link-arg-bins=/MANIFEST:EMBED");
	// The static CRT import libraries carry debug-info references that do not exist on this machine.
	println!("cargo:rustc-link-arg-bins=/IGNORE:4099");
	println!("cargo:rustc-link-arg-bins=/MANIFESTINPUT:{}", manifest.display());
}
