// Spike J0 (a): every paint of the text screen lays the whole body out three times, and a typed key at least twice more.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use quill_spike_reflow::scenario::letter_script;

#[test]
fn a_paint_lays_the_document_out_three_times_in_m2_letter() {
	let (rows, tally) = letter_script(true);
	assert!(rows.len() > 50);
	for r in &rows {
		assert_eq!(
			r.paint_passes, 3,
			"{}: caret_line plus two lines passes",
			r.what
		);
	}
	// Typed keys ask again from `scroll` (the caret's line) and from the line the caret moves to.
	assert!(tally.layout_story as usize >= rows.len() * 3);
}

/// `print_text` lays the body out twice and then once more for every page: two more than the pages it prints.
#[test]
fn print_pages_lays_the_document_out_once_per_page_and_twice_more() {
	use quill_app::App;
	use quill_grid::GridMeasure;
	use quill_host::script::MemFiles;
	use quill_host::HostStore;
	use quill_spike_reflow::corpus;
	use quill_spike_reflow::counting::Counting;

	let (m, tally) = Counting::new(GridMeasure, true);
	let mut s = HostStore::new();
	let mut files = MemFiles::default();
	let bytes = std::fs::read(corpus::root().join("tests/docs/print3.rtf")).unwrap();
	files.files.insert("PRINT3.RTF".into(), bytes);
	let mut app = App::with_measure(m);
	app.reset(&mut s).unwrap();
	app.open(&mut s, &mut files, b"PRINT3.RTF").unwrap();
	let pages = {
		let mut p = 0;
		app.for_each_line(&s, |_, l| p = p.max(l.page));
		p
	};
	let before = tally.borrow().by_site.clone();
	assert!(app.print_text(&mut s, &mut files, b"OUT.TXT"));
	let t = tally.borrow();
	let body: u64 = t
		.by_site
		.iter()
		.filter(|(k, _)| k.contains("print_pages"))
		.map(|(k, n)| n - before.get(k).copied().unwrap_or(0))
		.sum();
	println!(
		"print3.rtf: {pages} pages, {body} body layouts from print_pages; sites: {:?}",
		t.by_site
	);
	assert!(pages >= 3);
	assert_eq!(body, u64::from(pages) + 2);
}
