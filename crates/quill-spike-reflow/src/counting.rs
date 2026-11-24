// A measure that counts the layouts asked of it, and says which `App` method asked.
//
// `App` lays out through `&mut self.measure.clone()`, so every layout runs on a clone of the app's own measure, and the app's own
// measure answers only the questions the app asks directly. A clone's `resolution()` is the first thing `layout_story`,
// `walk_line` and `x_of` call. With tracing on, a call captures a backtrace and files it under the chain of `quill_app` functions
// that led to it (innermost first) and whether `layout_story` or `walk_line` was the callee. Tracing is slow: use it to count, never
// to time.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use quill_core::layout::{FontMetrics, FontRef, Measure, GLYPHS};

/// What was counted.
#[derive(Default, Debug, Clone)]
pub struct Tally {
	/// Calls of `layout_story`.
	pub layout_story: u64,
	/// Calls of `walk_line` (one line's characters, not a layout of the document).
	pub walk_line: u64,
	/// Other clone resolutions (the app measuring something itself).
	pub other: u64,
	/// `layout_story` calls by the chain of `quill_app` functions that made them.
	pub by_site: BTreeMap<String, u64>,
}

/// The shared tally.
pub type Shared = Rc<RefCell<Tally>>;

/// A measure wrapped to count.
pub struct Counting<M: Measure + Clone> {
	inner: M,
	tally: Shared,
	is_clone: bool,
	trace: bool,
}

impl<M: Measure + Clone> Counting<M> {
	/// Wraps `inner`; `trace` turns on the backtraces.
	pub fn new(inner: M, trace: bool) -> (Self, Shared) {
		let tally: Shared = Rc::default();
		(
			Counting {
				inner,
				tally: tally.clone(),
				is_clone: false,
				trace,
			},
			tally,
		)
	}
}

impl<M: Measure + Clone> Clone for Counting<M> {
	fn clone(&self) -> Self {
		Counting {
			inner: self.inner.clone(),
			tally: self.tally.clone(),
			is_clone: true,
			trace: self.trace,
		}
	}
}

/// Removes every balanced `<...>` group.
fn strip_generics(s: &str) -> String {
	let (mut depth, mut out) = (0u32, String::new());
	for c in s.chars() {
		match c {
			'<' => depth += 1,
			'>' => depth = depth.saturating_sub(1),
			_ if depth == 0 => out.push(c),
			_ => {}
		}
	}
	out
}

/// Reads a backtrace, innermost frame first: which of `layout_story` and `walk_line` is the callee (the first of them met, since
/// `draw` calls `walk` from inside a `lines` callback, which is inside `layout_story`), and the `quill_app` functions between,
/// without generics or closures.
fn chain(bt: &str) -> (bool, bool, String) {
	let (mut story, mut walk, mut decided) = (false, false, false);
	let mut names: Vec<String> = Vec::new();
	for line in bt.lines() {
		let l = line.trim();
		let Some((num, rest)) = l.split_once(": ") else {
			continue;
		};
		if num.is_empty() || !num.chars().all(|c| c.is_ascii_digit()) {
			continue;
		}
		let plain = strip_generics(rest);
		if !decided {
			if plain.contains("layout::layout_story") {
				story = true;
				decided = true;
			} else if plain.contains("layout::walk_line") {
				walk = true;
				decided = true;
			}
		}
		if rest.contains("quill_app::") {
			let parts: Vec<&str> = plain
				.split("::")
				.filter(|p| {
					!p.is_empty()
						&& !p.starts_with("{{closure")
						&& !p.starts_with("{closure")
						&& !p.starts_with('h')
				})
				.collect();
			let name = parts[parts.len().saturating_sub(2)..].join("::");
			if names.last() != Some(&name) && !name.is_empty() {
				names.push(name);
			}
		}
	}
	names.retain(|n| !n.ends_with("::lines") && n != "lines");
	names.truncate(2);
	(story, walk, names.join(" < "))
}

impl<M: Measure + Clone> Measure for Counting<M> {
	fn resolution(&self) -> (u16, u16) {
		if self.is_clone {
			let mut t = self.tally.borrow_mut();
			if self.trace {
				let bt = std::backtrace::Backtrace::force_capture().to_string();
				let (story, walk, site) = chain(&bt);
				if story {
					t.layout_story += 1;
					*t.by_site.entry(site).or_default() += 1;
				} else if walk {
					t.walk_line += 1;
				} else {
					t.other += 1;
				}
			} else {
				t.layout_story += 1;
			}
		}
		self.inner.resolution()
	}
	fn metrics(&mut self, f: &FontRef) -> FontMetrics {
		self.inner.metrics(f)
	}
	fn widths(&mut self, f: &FontRef, out: &mut [u16; GLYPHS]) {
		self.inner.widths(f, out);
	}
}

/// A 96 dots per inch screen where every character is 8 pixels wide and a line 16 high (the page view's geometry).
#[derive(Clone, Copy)]
pub struct Screen96;

impl Measure for Screen96 {
	fn resolution(&self) -> (u16, u16) {
		(96, 96)
	}
	fn metrics(&mut self, _: &FontRef) -> FontMetrics {
		FontMetrics {
			ascent: 12,
			descent: 4,
			leading: 0,
		}
	}
	fn widths(&mut self, _: &FontRef, out: &mut [u16; GLYPHS]) {
		out.fill(8);
	}
}

/// A 96 dots per inch measure with widths that vary by character and size, so lines wrap in many places and a size change moves
/// them: the character width is `(4 + c % 6)` pixels at 12 points, and every size scales it.
#[derive(Clone, Copy)]
pub struct Prop96;

impl Measure for Prop96 {
	fn resolution(&self) -> (u16, u16) {
		(96, 96)
	}
	fn metrics(&mut self, f: &FontRef) -> FontMetrics {
		let px = i32::from(f.size) * 2 / 3;
		FontMetrics {
			ascent: px * 4 / 5,
			descent: px / 5 + 1,
			leading: 0,
		}
	}
	fn widths(&mut self, f: &FontRef, out: &mut [u16; GLYPHS]) {
		for (i, w) in out.iter_mut().enumerate() {
			*w = ((4 + (i as u32 % 6)) * u32::from(f.size) / 24).max(1) as u16;
		}
	}
}
