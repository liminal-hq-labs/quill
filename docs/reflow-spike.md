# Reflow spike (J0)

Spike J0 of the threading plan measures the whole-document layout, then proves a fix before it is built. Everything here is throwaway: a `reflow-spike` feature in `quill-core` (`layout/reflow_spike.rs`), the `quill-spike-reflow` crate (tests and a report binary), and `win9x/spike-thread`. No behaviour of `quill-app` or `quill-core` changed. J3 and J4 write the real `LayoutCursor` and checkpoint cache; this page is what they build against.

Every number below was measured on the host (Linux, release build unless it says dev) or in a guest, and the source of each is named. Where something is an estimate it says so.

## Result against the J0 row

| Pass criterion | Met | Evidence |
|---|---|---|
| A report with per-key layout calls before and after (target at most 3 paragraphs) | Partly | Before: 6 whole-document layouts per typed key and paint on the text screen, 10 on the page view. After: 1.02 to 1.04 paragraph visits per key on the letter and 1.40 to 1.72 on a 1 MB document, and 98.3 to 98.9 per cent of keys visit one. But 1.1 to 1.7 per cent of keys on the 1 MB document visit five or more (at most 36 typing in one place, 123 at random places), because a line that wraps moves the page breaks of the pages after it. The mean and the median meet the target; the worst key does not |
| Checkpoint lines equal full layout for every `tests/docs` file | Yes | `tests/reflow.rs`: 7 files, 2 measures each, 10,000 random edits each (inserts, deletes, paragraph formats, character sizes), checkpoints and lines compared with `layout_story` after every fifth edit and the last; and `layout_from` from the start equals `layout_story` line for line for all five stories of every file |
| The snapshot write's bytes equal a write of the pre-edit document | Yes | `tests/cow.rs`: a 1,009,452-byte `.QLD` written on a std thread from a snapshot while 1,000 random edits ran on the live document |
| The Windows 95 worker round trip works on real Windows 95 | Yes | `docs/evidence/j0-thread95-windows95.log`, from Windows 95 OSR2.5 in DOSBox-X, and under Wine; the import and Pentium scans pass |

## (a) What a key and a paint cost today

`App::lines` (`crates/quill-app/src/lib.rs`) calls `layout_story` from the first character every time, and nothing keeps its result. A counting `Measure` (`counting.rs`) counts them: `App` lays out through `&mut self.measure.clone()`, so each layout runs on a clone whose first call is `resolution()`; a backtrace on that call says whether `layout_story` or `walk_line` is the callee and which `quill_app` function asked. `walk_line` is one line's characters, not a layout, and is not counted below. A layout visits every paragraph (the loop has no early exit), so paragraph visits are layouts times paragraphs.

**m2-letter** (`tests/scenarios/m2-letter.keys`, 86 keys, 9 paragraphs, text screen), from a dev build with backtraces:

| Who asked | Layouts in the run |
|---|---|
| `line_for` (`caret_line` and the caret's line after a movement key; 86 of them are the paint's `caret_line`) | 192 |
| `paint::draw`'s two `lines` passes | 174 |
| `scroll` | 86 |
| `line_at`, `line_count` | 3 and 3 |

Every paint made exactly 3 layouts (the test asserts it for all 86); the keys made 200 and the paints 258, 458 in all. That is 2.33 layouts per key and 3.00 per paint (5.33 together). `print_text` lays the body out pages plus 2 times: 5 for the 3 pages of `print3.rtf`, which is also the claim about `print_pages`.

**Typing 40 letters at the middle of a document**, one key and one paint each. The page view's paint is rebuilt from `win9x/quill-win9x/src/main.rs` and `gdi.rs`: `app.key`, `point_of` (from `ensure_visible`), then `draw`, `body_pages`, `paint_page`'s line loop and `caret_xy`. I did not run the Windows shell for this count, and `update_scroll` was not instrumented, so the page-view numbers are a floor.

| Document | Paint | Layouts per key | Layouts per paint | Paragraph visits per key and paint | Host time per key | Host time per paint |
|---|---|---|---|---|---|---|
| `letter.rtf`, 9 paragraphs | text screen | 3.00 | 3.00 | 54 | 108 µs | 202 µs |
| `letter.rtf` | page view | 4.00 | 6.00 | 90 | 124 µs | 190 µs |
| generated, 4,700 paragraphs, 1 MB | text screen | 3.17 | 3.00 | about 29,000 | 24.2 ms | 27.2 ms |
| generated, 1 MB | page view | 4.17 | 6.00 | about 48,000 | 41.0 ms | 63.1 ms |

One full `layout_story` pass over the 1 MB document takes 7.5 ms with the grid measure and 10.4 ms with the 96 dpi measure (16,065 to 21,630 lines). So a typed key plus its paint costs 51 to 104 ms on this host for a document that is the most Quill can hold, and scales with the document: the claim in the plan is confirmed. The benchmarks are `cargo run --release -p quill-spike-reflow` (times) and `cargo run -p quill-spike-reflow -- trace` (a dev build, counts); `tests/counts.rs` keeps the 3-per-paint and pages-plus-2 facts true.

The generated document (`corpus::big_doc`) is 999,729 bytes: paragraphs of 20 to 400 characters, now and then a tab or a hard break, centred headings and a page break before every seventh heading.

## (b) The checkpoint cache

`layout_from(doc, s, m, kind, start, f, after_para)` is the body of `layout_story` with a starting state and a callback after each paragraph that can stop it. It is the real function's code, copied, so it shares `para_end`, `Stops`, `font_ref`, the font cache and the rest. A checkpoint is `{pos, para, page, y}`: four 32-bit numbers, 16 bytes, taken at each paragraph boundary.

`Reflow::repair` takes the edit as the edit log would report it (`at`, bytes removed, bytes inserted, and the end of the range whose format changed), restarts at the checkpoint of the paragraph holding the first changed byte, and stops at the first paragraph boundary where the new `{pos, y}` equals an old checkpoint shifted by the edit, past the changed range. The rest is the old rest with `pos` shifted by the edit and `para` and `page` shifted by a constant. That works because layout never reads the page or paragraph number, only `y`: a hard page break that renumbers every page after it costs the repair of one paragraph, not the rest of the document. (My first version compared the page too, and a page-break toggle then re-laid 4,475 paragraphs.)

**After**, typing 2,000 letters (200 on the letter), each key an insert, the repair and the caret's line:

| Document and measure | Paragraphs | Checkpoints | Paragraph visits per key | Most in one key | Keys visiting 1 / 2 / 3 / 4 / 5 or more | Host time per key |
|---|---|---|---|---|---|---|
| `letter.rtf`, text screen | 9 | 144 B | 1.02 | 2 | 196 / 4 / 0 / 0 / 0 | 22.9 µs |
| `letter.rtf`, 96 dpi | 9 | 144 B | 1.04 | 2 | 192 / 8 / 0 / 0 / 0 | 21.1 µs |
| 1 MB, 96 dpi, one place | 4,700 | 73 KB | 1.40 | 36 | 1,977 / 0 / 0 / 0 / 23 | 196 µs |
| 1 MB, 96 dpi, random places | 4,700 | 73 KB | 1.72 | 123 | 1,979 / 0 / 0 / 0 / 21 | 270 µs |
| 1 MB, text screen, one place | 4,700 | 73 KB | 1.58 | 36 | 1,966 / 0 / 0 / 0 / 34 | 241 µs |

The cache for the 1 MB document is 73 KB (4,700 checkpoints, 16 bytes each), built once in 7.4 to 11.2 ms, about 11 per cent of the 640 KB the plan guessed for a line table. Against the before column, the host time for the key falls from 24 to 41 ms to 0.2 ms, and the layout part of the paint is what is left (below). Two shapes are visible: a key either leaves the paragraph's height alone and visits one paragraph, or it adds a line and the change ripples to the next page-break resync. In the generated document a resync is a few paragraphs to a page; the 36 and 123 are those.

The caret query costs nothing extra on a typed key: the repair keeps the lines it laid out and `line_at` finds the caret in them. Painting a screenful still needs lines. Laying out a window of four paragraphs (41 lines) from a checkpoint is four visits and 266 µs; a real painter would keep the window's lines between keys and shift them with the edit, which is cheap integer work, not layout. I did not build that.

**The proof** (`tests/reflow.rs`, `prove.rs`): after each checked edit the cached checkpoints must equal the checkpoints of a fresh full build, and laying out one paragraph from each cached checkpoint in turn must give the same lines, in the same order, as `layout_story`, and the same page count. Mutation checks: removing the `y` equality or the dirty-range rule from the convergence test fails the property within the first edits. A failure prints the seed (`0x514c494c4c204a30` by default; `QUILL_SPIKE_SEED` overrides it), the document, the edit number and the first differing line or checkpoint. The edit mix is 40 to 70 per cent inserts of one to eight bytes from letters, spaces, hyphens, tabs, hard breaks and paragraph marks, deletes of one to eight bytes, paragraph formats on one whole paragraph (alignment, spacing, a page break before, an indent, space after) and character sizes on a range, with the length kept near where it started. The 1 MB document gets 300 edits in the test (a full layout of it in a dev build takes seconds), checked every 25th.

### What J3 and J4 must handle

- **Line indices.** `App` addresses lines by index (`top`, the index `line_for` returns, `click`, `line_at_y`), and a paragraph checkpoint has no line count. J4 either moves those to `(paragraph, line)` or positions, or adds a line count to the checkpoint (20 bytes a paragraph).
- **Undo and redo are not modelled.** The prototype takes its note from the edit that was made. `Text::undo` and `redo` do not report the range they changed, so J3 needs them to, or the cache is invalid after either.
- **Format changes** have to pass the range they changed (`dirty_end`): converging inside it is wrong, and the proof catches that.
- **Whole-document changes** (page setup, the font table, a different story) invalidate everything. The prototype does not handle them; the real one must name them.
- **The font cache.** `layout_from` builds a `FontCache` per call, as `layout_story` does. On the host that is nothing; on DOS it may not be (see below).
- **Page cascades** are the one place a key is not bounded. J4's criterion should say median and ninety-ninth percentile, or bound the repair per idle slice and finish it from `idle`.

## (c) Copy-on-write snapshot

`CowStore` holds chunks as `Arc<Vec<u8>>`; a write calls `Arc::make_mut` and counts the bytes it copies when a snapshot shares the chunk. `snapshot()` clones the chunk table. `Doc` is not `Clone` and has no `header_copy` yet, so the spike copies its bytes (`ptr::read`) after asserting at run time that `Doc` has no destructor; it is 4,976 bytes. J11's `Doc::header_copy` replaces that.

`tests/cow.rs`, on a document of 999,729 bytes (1,009,452 as `.QLD`, 38 live chunks):

- A thread writes `.QLD` from the snapshot, yielding after every sink call (6,761 of them), while the main thread makes 1,000 random edits (inserts, deletes, paragraph and character formats). The snapshot's bytes equal the pre-edit document's. The live document's bytes differ.
- The 1,000 edits copied **98,304 bytes in 3 chunk copies**: the add buffer's last chunk, the piece list's and the undo log's. A chunk is copied once per snapshot; a second key in the same place copies nothing, and with no snapshot alive an edit copies nothing.
- The first key after a snapshot, over 40 snapshots at random places: 2.95 chunks and 96,665 bytes on average, 3 at most. That is the plan's "at most 3 chunks".
- The plan guessed about 150 chunks for a 1 MB document with pictures; without pictures it is 38.

## (d) Windows 95 worker thread

`win9x/spike-thread` builds `THREAD95.EXE` (73,728 bytes, most of it the resource file the workspace's link line adds). The window thread makes a hidden window and calls `CreateThread` with a 64 KB stack; the worker computes a checksum three times and posts each to the window with `PostMessageA(WM_APP)`; the window procedure checks the checksum against its own, checks it runs on the window thread, and quits after the third; the window thread then joins with `WaitForSingleObject` and writes `THREAD.LOG`. A timer fails the run after eight seconds if nothing arrives.

- Built by `win9x/build.sh` with the rest of the workspace. `check-imports.sh` and `scan-pentium.sh` pass on it (17 imports; no SSE, MMX or CMOV). Five names went on `tools/imports.allow`: `CreateThread`, `WaitForSingleObject`, `GetCurrentThreadId`, `Sleep` and `PostMessageA`. All are in Windows 95's KERNEL32 and USER32.
- **Wine** (`win9x/tools/wine-thread.sh`, its own Xvfb display and prefix): `main=00000140 worker=00000144 arrived=3 sums=3 on_window_thread=3 join=0 result=PASS`.
- **Windows 95 OSR2.5 in DOSBox-X** (Liminal Installer's `scripts/win95/run-win95.sh -w 40`, with a home, display and container of my own): the same line with `main=ffffab15 worker=ffffb3ad`, `result=PASS`, exit status 0 (`docs/evidence/j0-thread95-windows95.log`). The guest was DOSBox-X's usual emulated Pentium, so this shows the thread API and message posting work on Windows 95, not how fast they are.

What it does not show: a worker running while the window thread paints, the cost of a thread on a 4 MB machine, or `GetInputState` yielding.

## (e) What the cache would save on the DOS graphics branch

On #54 (`feat/dos-gfx-g6`, `docs/dos-graphics.md`, "Measured") one pass over the laid-out document takes 27 ms and a key takes 575 ms at 25,000 emulated cycles, with the laid-out document rebuilt "several times" per redraw, roughly half of the paint. The document there is `letter.rtf`: 9 paragraphs, 15 lines. Nothing below was run on that branch; it is an estimate.

- From the document's own `paint` figure (560 ms) and "roughly half", the layout share is about 280 ms, which is about 10 passes of 27 ms. The host count for the page view is 10 layouts per key and paint, so the two agree.
- With the cache, a typed key repairs one paragraph. The paint still has to produce the page's 15 lines, which on a 9-paragraph document is one pass of layout (27 ms) unless the lines are kept between keys, in which case it is only the repaired paragraph (about 3 ms, a ninth of a pass).
- So the layout part would fall from about 280 ms to between about 30 ms (lines not kept) and about 3 ms (lines kept), and the key from about 575 ms to about 325 ms or less. The other half, drawing about 500 glyphs, is untouched and is what line damage (G10) is for.
- **A guess I did not test:** 27 ms for a 15-line document is far more than the host's 3 µs a paragraph, so it may be mostly a fixed cost per layout (building the font cache, which calls the strike font's `widths` for each font) rather than a cost per paragraph. If so, paragraph checkpoints save less than the paragraph count suggests, and keeping the `FontCache` alive between layouts matters as much as the cache. A 4,700-paragraph document on that machine would save the most, but a 1 MB document is not what a 486 will be asked to open.

## Reproducing

```sh
cargo nextest run -p quill-spike-reflow                 # the proofs (a minute or so)
cargo run --release -p quill-spike-reflow               # the timings and the "after" table
cargo build -p quill-spike-reflow && target/debug/reflow-spike trace   # who asks for each layout
(cd win9x && ./build.sh) && win9x/tools/check-imports.sh win9x/target/win9x/release/thread95.exe \
  && win9x/tools/scan-pentium.sh win9x/target/win9x/release/thread95.exe && win9x/tools/wine-thread.sh
```

## Proposed decisions

For `docs/decisions.md`, in the new prefix scheme, once the prefix change lands:

- **CORE-1** Layout is resumable at paragraph boundaries and a checkpoint is `{pos, para, page, y}`. Convergence after an edit compares `{pos, y}` and carries `para` and `page` as constant offsets.
- **CORE-1.1** The edit log reports the first changed byte, the bytes removed and inserted, and the end of any changed format range; undo and redo report theirs.
- **CORE-2** A snapshot is a copy-on-write store and a copy of the document header; a typed key copies at most three chunks once per snapshot.
