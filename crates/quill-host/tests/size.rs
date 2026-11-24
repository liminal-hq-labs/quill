// The editor's size in memory: the Win16 build has a 30 KB stack and keeps the editor in a static, so a growing `App` matters.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use quill_app::App;

/// Building an `App` on the Win16 stack once ran it past its bottom into the program's statics (four zero bytes in the screen dump of
/// the Windows 3.11 run). `App::new()` is still built as a temporary, so keep it well under the stack and look at `win16/build.sh`'s
/// stack size when this fails.
#[test]
fn the_editor_stays_small_enough_for_the_win16_stack() {
	let size = core::mem::size_of::<App>();
	assert!(size <= 16 * 1024, "App is {size} bytes; the Win16 stack is 30 KB");
}
