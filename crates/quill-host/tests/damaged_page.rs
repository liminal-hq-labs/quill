// A page setup read from a damaged file can hold any numbers; laying the document out must not overflow on them.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use quill_core::doc::{Doc, StoryKind};
use quill_core::layout::layout_story;
use quill_host::{GridMeasure, HostStore};

#[test]
fn extreme_margins_do_not_overflow_the_layout() {
	for (paper, margin) in [
		(i32::MIN, i32::MAX),
		(i32::MAX, i32::MIN),
		(i32::MIN, i32::MIN),
		(0, i32::MAX),
		(i32::MAX, i32::MAX),
	] {
		let mut s = HostStore::new();
		let mut d = Doc::new();
		d.init(&mut s).unwrap();
		d.body.load(&mut s, b"Some text\r", 0, 0).unwrap();
		d.page.paper_w = paper;
		d.page.paper_h = paper;
		d.page.margin_l = margin;
		d.page.margin_r = margin;
		d.page.margin_t = margin;
		d.page.margin_b = margin;
		layout_story(&d, &s, &mut GridMeasure, StoryKind::Body, |_| {});
	}
}
