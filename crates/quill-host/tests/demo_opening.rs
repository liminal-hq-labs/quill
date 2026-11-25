// The shipped demo documents open on the heading `Hi, this is Quill` with the document icon inline at the end of that heading's one
// line, not in front of it and not on a line of its own.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use quill_core::doc::{Doc, StoryKind};
use quill_core::layout::{layout_story, picture_size, Line};
use quill_formats::io::SliceSource;
use quill_host::corpus::Var96;
use quill_host::HostStore;

fn open(name: &str) -> (Doc, HostStore) {
	let path = format!("{}/../../samples/{name}", env!("CARGO_MANIFEST_DIR"));
	let bytes = std::fs::read(&path).unwrap();
	let mut s = HostStore::new();
	let mut d = Doc::new();
	let mut src = SliceSource::new(&bytes);
	if name.ends_with(".rtf") {
		quill_formats::rtf::read(&mut d, &mut s, &mut src).unwrap();
	} else {
		quill_formats::qld::read(&mut d, &mut s, &mut src).unwrap();
	}
	(d, s)
}

#[test]
fn the_icon_ends_the_opening_heading_on_its_line() {
	for set in ["demo", "demo-reactos", "demo-reactos16"] {
		for ext in ["rtf", "qld"] {
			let name = format!("{set}.{ext}");
			let (d, s) = open(&name);
			let mut lines: Vec<Line> = Vec::new();
			layout_story(&d, &s, &mut Var96, StoryKind::Body, |l| lines.push(*l));
			let first = lines[0];
			assert_eq!((first.para, first.line, first.start), (0, 0, 0), "{name}");
			// The heading is one line, and the next paragraph starts below it.
			assert_eq!(lines[1].para, 1, "{name}: the heading wrapped");
			assert!(lines[1].y > first.y, "{name}");
			// The picture is the last character of that line.
			let text = d.story(StoryKind::Body);
			let pic = first.start + first.len - 1;
			assert!(
				picture_size(&d, &s, text, pic, 96, 96).is_some(),
				"{name}: the heading does not end with the picture"
			);
			for at in 0..pic {
				assert!(text.object_at(&s, at).is_none(), "{name}: picture at {at}");
			}
			// It sits on the baseline: the line is as tall as the picture, whose bottom is the baseline.
			let (_, h) = picture_size(&d, &s, text, pic, 96, 96).unwrap();
			assert!(first.ascent >= h, "{name}: line {first:?} lower than {h}");
		}
	}
}
