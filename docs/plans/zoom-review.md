# Zoom review: the zoomed page, the previews, the paging and the toolbar

The root causes behind items 1 to 6 of the owner's morning QA of 2026-11-27 (zoom rendering first), and one brief per fix for a Sonnet builder. Written against `origin/main` at `dd8efd2`, with WP-ARCH-1 (`quill-chrome`) and WP-GTK-1 in flight. The rules, budgets, gates (G-HOST, G-DOS, G-W16, G-W32, G-DISK, G-MD) and the common brief are those of [work-packages.md](../work-packages.md); every brief below starts with its common brief.

## Table of contents

- [What the QA saw](#what-the-qa-saw)
- [Root causes](#root-causes)
- [Order and lanes](#order-and-lanes)
- [Decision ids, evidence and goldens](#decision-ids-evidence-and-goldens)
- [Z1: Win32 zoomed page text](#z1-win32-zoomed-page-text)
- [Z2: Win32 print preview text](#z2-win32-print-preview-text)
- [Z3: Win16 phantom sheet](#z3-win16-phantom-sheet)
- [Z4: the paging rule on a zoom change](#z4-the-paging-rule-on-a-zoom-change)
- [Z5: Win16 print preview](#z5-win16-print-preview)
- [Z6: the style box reads Normal](#z6-the-style-box-reads-normal)
- [Z7: the italic glyph and the pressed button's face](#z7-the-italic-glyph-and-the-pressed-buttons-face)
- [Z8: Win16 zoomed words fitted to the layout](#z8-win16-zoomed-words-fitted-to-the-layout)
- [Owner decisions](#owner-decisions)
- [Not in this review](#not-in-this-review)

## What the QA saw

| Item | Build | What the owner saw | Reproduced |
|---|---|---|---|
| 1 | Win32 | Text squashed and overlapping at 75 and 50 %, letters spread apart at 125, 150 and 200 %; 100 % fine. Win16 much better | Under Wine on a private Xvfb: at 50 % Times New Roman is squashed and Arial and Courier New are spread; 200 % is close to right under Wine. The owner's ReactOS is worse at both ends (see [Root causes](#root-causes)) |
| 2 | Win32 | Print preview text squashed | Under Wine: the preview (about 43 %) has the same uneven spacing |
| 2 | Win16 | Print Preview never opens from the File menu | By reading the code: there is no Win16 preview |
| 3 | Win16 | A phantom second page at 50 % on a one-page document | By reading the code; Win32 and DOS bound the sheets by the page count |
| 5 | All | The style box is blank on Normal paragraphs | Host test: the demo documents' body paragraphs read as no style |
| 6 | Win32 | The italic glyph looks odd; a pressed button shows a two-tone square | Under Wine: Bold and Centre switched on show the glyph's cell as a darker square |

The before shots are in `~/Documents/Liminal HQ/Quill/Zoom review/before/`.

## Root causes

**Z1, Win32 zoomed text.** `win9x/quill-win9x/src/gdi.rs` `paint_page` sets `MM_ANISOTROPIC` (`map_pages`: window extent 100, viewport extent `pct`) and draws text in page units with the 100 % font (`font_handle_at(fr, extra, 96)`, `lfHeight` in logical units), leaving the system to realise the font at the scaled size. Because `per_char` is `pct != 100`, every character goes out in its own `TextOutA` at the layout's x. The layout's advances are the 100 % font's whole-pixel widths (`GetCharWidthA` on the unmapped `mdc`), so the gap between two characters is the 100 %-hinted advance times the zoom, while the glyph drawn is whatever the system realises for the mapped font. They agree only when the realisation is exactly linear in the scale. Under Wine at 50 % Times realises wider than half (squashed) and Arial and Courier narrower (spread); on ReactOS the owner saw squashed below 100 % and spread above, which is what a glyph that stays near its 100 % size does. Win16 (W16-15) does two things differently: it makes the font at the zoomed device height with no mapping mode (`font_handle_at(fr, extra, pct)`), and it places a word at a time, so inside a word the font's own advances apply and only a word's start follows the layout. That is why Win16 looks better; its leftover kerning is the drift inside a word (Z8).

**Z2, Win32 preview text.** `paint_preview` maps the sheet with `map_pages(pct)` at the preview's fitted scale (10 to 200 %, usually 40 to 70 %) and calls `draw_page`, which draws with `per_char: pct != 100` through the same `draw_line` path. Same cause as Z1, at a scale that is rarely a menu level.

**Z3, Win16 phantom sheet.** `win16/examples/quill/gdi.rs` `paint_page` draws sheets in a `loop` that stops only when a sheet's top passes the bottom of the window (or `p > 400`); it never asks `app.page_total`. A letter sheet at 50 % is 528 pixels, so any page area taller than about 540 pixels shows a second, empty sheet, and 25 % shows several. Win32 stops at `body_pages` and the DOS graphics painter at `page_total`.

**Z4, the paging rule.** `App::set_zoom` stores the zoom and nothing else; the scroll position is kept in page units (W16-15, CORE-65), so after zooming out from a scrolled view the top of the page stays off screen even when the whole sheet would fit, and Win32's `view::vscroll_max` lets the last sheet's bottom reach the middle of the view, so a one-page document that fits can still be scrolled half away. There is no rule that says "when the sheet fits, show it from its top".

**Z5, Win16 preview.** The menu item and its id are fine (`Command::Preview` from `quill_app::paint`'s tree, through the Win16 resource builder). `run_command` in `win16/examples/quill/main.rs` intercepts only `Command::PrintSetup`; `Command::Preview` falls through to `App::command`, whose arm `Command::Preview | Command::PrintSetup` sets the message `That is not available in this version.` Win16 has no preview code at all. The data segment is not the cause, but it is the constraint on the fix (about 2 KB free; nothing merges below 1,024, W16-60).

**Z6, the blank style box.** `App::style_of` (`crates/quill-app/src/chrome.rs`) returns a preset only on exact equality of alignment, indents, space before and after, line spacing, list flags, size and the bold, italic and underline bits. Normal's preset has no space after, but the shipped samples (`samples/demo*.rtf`) and most RTF write Normal body paragraphs with `\sa120`, so they read as no preset and every build shows a blank box. A host run over `demo-reactos.rtf` gives `Some(1)` for the headings and `None` for every body paragraph; a new document's paragraphs read `Some(0)`.

**Z7, the toolbar.** `chrome.rs` `glyph` in the Win32 build copies the 16 by 15 cell with `SetDIBitsToDevice`, opaque. `recolour_strip` sets palette entry 0 to the scheme's face, but a switched-on toggle (`button_face` with `lit`) fills its base with `light()`, the mix of face and highlight, so the cell shows as a face-coloured square on a lighter button: the two-tone square. Win16's `bevel` always fills `FACE`, so it does not show there. The italic glyph (`assets/toolbar/make_toolbar.py` `glyph_italic`) has its serifs off centre of the stem's ends (top serif 8 to 12 on a stem starting at 10 and 11, bottom serif 4 to 8 on a stem ending at 6 and 7), so at 15 pixels it reads as a slanted J.

## Order and lanes

Each brief is one pull request for one Sonnet builder. Three builders at most at a time. Only one branch at a time is open in `crates/quill-app/src` in the ED lane, and WP-ARCH-1 owns `crates/quill-app/src/view.rs`, `zoom_field.rs` and the new `scroll.rs` until it merges, so the one brief that needs shared scroll code (Z4) waits for it. WP-ARCH-1 also edits `use` lines in `win16/examples/quill/*.rs` and `win9x/quill-win9x/src/ui.rs`; a brief here that edits function bodies in those files meets it only on `use` lines, and whichever merges second rebases. Nothing here touches `linux/quill-gtk` (WP-GTK-1).

| Wave | Builder 1 (Win32) | Builder 2 (Win16) | Builder 3 (shared) | Waits for |
|---|---|---|---|---|
| 1 (now) | Z1 Win32 zoomed text | Z3 Win16 phantom sheet | Z6 style box (ED lane slot) | Z6: no other ED-lane branch open |
| 2 | Z2 Win32 preview text | Z5 Win16 print preview | Z7 toolbar glyphs | Z2: Z1 merged. Z5: Z3 merged. Z7: nothing |
| 3 | — | Z8 Win16 fitted words | Z4 paging rule (ED lane slot) | Z8: Z1 (for `quill_core::fit`) and Z5 merged. Z4: WP-ARCH-1 and Z6 merged |

Z1 is priority 1. Z7 can swap with Z6 in wave 1 if the ED lane is busy.

## Decision ids, evidence and goldens

**Ids.** This review takes ids above the ranges [work-packages.md](../work-packages.md) reserves: CORE-140 to CORE-144, W16-80 to W16-84, W9X-80 to W9X-84 (the Win32 decisions are W9X, not W32). Each brief names the ids it takes.

**Evidence** goes to `~/Documents/Liminal HQ/Quill/Zoom review/<Z-id>/`, one PNG per platform, state and zoom, named `<before|after>-<platform>-<state>-z<NNN>.png` (platforms as work-packages.md names them: `win32-wine`, `win16-311`, `dos-gfx-12h`, `dos-gfx-vbe`), plus a 3x crop of the text block for each zoom. Shots come from the repository's scripts on the builder's own Xvfb or emulator, never the host desktop: `unset WAYLAND_DISPLAY` and set `DISPLAY` explicitly in every command, and use `xdotool` only against that display.

**Goldens at every zoom.** Each painter brief adds frames at 50, 75, 100, 125, 150 and 200 % of a new test document, `tests/docs/zoomtext.rtf`, which Z1 writes: one paragraph each of Times New Roman, Arial and Courier New at 12 point, and one of Times at 9 point, holding `minimum illicit WWW mummy iiii llll`, a bold and an italic word, and a justified line. Win16 frames are exact goldens (DOSBox-X is deterministic). Win32 frames are goldens with a tolerance (`compare -metric AE -fuzz 10%`, at most 0.5 % of the crop's pixels) and are compared only when `wine --version` equals the version recorded beside them in `tests/golden/zoom/WINE_VERSION`; otherwise the script says so and runs only the measured checks. The measured check is the same on both platforms, a new shared script `tools/zoom_text.py` (Python, `-I`, Pillow): on each line of the test document it finds the words by the columns with no ink and checks each word's ink width against the same word in the 100 % frame times the zoom, within the larger of 2 pixels and 4 %, and that no two words have touched. On `origin/main` under Wine the 50 % Times line fails it; the builder records the before figures in the pull request.

## Z1: Win32 zoomed page text

- **Finish line:** at 50, 75, 125, 150 and 200 % the Win32 page draws its text in device pixels with a font made at the zoomed height, a word at a time, each word starting at the layout's x and ending at the layout's end, with the gaps inside it taken from the zoomed font's own widths and the difference spread across them. 100 % draws exactly as before.
- **Owns:** `crates/quill-core/src/fit.rs` (new) and its `pub mod fit;` line in `crates/quill-core/src/lib.rs`; `crates/quill-core/tests/fit.rs` (new); in `win9x/quill-win9x/src/gdi.rs` the page view path (`paint_page`, `draw_line`'s zoomed branch, `font_handle_at`, a zoomed-widths cache, `picture_marks`); `win9x/quill-win9x/src/view.rs` (a `Zoom` helper if wanted); `win9x/tools/wine-zoom-text.sh` (new); `tools/zoom_text.py` (new); `tests/docs/zoomtext.rtf` (new); `tests/golden/zoom/` (new, `win32-wine-z*.png` and `WINE_VERSION`).
- **Must not touch:** `crates/quill-app/**` (WP-ARCH-1 and the ED lane); `paint_preview`, `draw_page`, `print_document` and the printer path (Z2 and printing keep the mapped path until Z2); `win9x/quill-win9x/src/chrome.rs` (Z7); `win16/**`.
- **Depends on:** nothing.
- **Budget:** `QUILL32.EXE` grows by at most 4 KB; no new import (`ExtTextOutA`, `GetCharWidthA` and `CreateFontA` are on `imports.allow`). DOS Δ 0 and Win16 data Δ 0 (the new `quill_core::fit` is linked only where it is called; measure both and say so).
- **Tests:** G-HOST with `crates/quill-core/tests/fit.rs`; G-W32 with `wine-zoom.sh`, `wine-layout.sh`, `wine-picture.sh`, `wine-pixels.sh` and the new `wine-zoom-text.sh` (goldens at 50 to 200 %); every existing golden unchanged.
- **Evidence:** `before-` and `after-win32-wine-page-z050` to `z200`, with crops.
- **Decisions:** W9X-80 (the zoomed page draws in device pixels, word by word, fitted), CORE-140 (`quill_core::fit`).

Brief:

```text
Package Z1: draw the Win32 page's text at a zoom the way the font wants it, with each word where the layout
puts it. Branch fix/win32-zoom-text, worktree ~/source/worktrees/quill/win32-zoom-text.

Read first: docs/plans/zoom-review.md (Root causes, Z1), decisions W16-15, W9X-11, CORE-65, D102, and
win16/examples/quill/gdi.rs (font_handle_at, draw_line's zoomed branch, Zoom) — Win16 is the model.

The cause: win9x/quill-win9x/src/gdi.rs paint_page maps page units with MM_ANISOTROPIC (map_pages) and
draws each character alone (per_char) at the layout x with the 100 % font, letting the system realise the
font at the scale. The layout's advances are the 100 % font's whole-pixel widths; the realised glyphs are
not that times the zoom (Wine squashes Times at 50 %; ReactOS squashes below 100 and spreads above).

1. crates/quill-core/src/fit.rs (no_std, no float, no allocation, no panic; the crate's clippy denies):
     /// Spreads `target - sum(adv)` over the gaps between the glyphs of a word, so the word's advances
     /// sum to `target`: the first `|r| % gaps` gaps take one more pixel than the rest, in order, and the
     /// last glyph's advance takes nothing (it has no gap after it). One glyph: its advance becomes
     /// `target`. No advance goes below 0.
     pub fn fit_advances(adv: &mut [i32], target: i32)
   Host tests in crates/quill-core/tests/fit.rs with proptest: the sum is the target whenever the target
   is at least 0; a residual of 0 changes nothing; no gap moves by more than ceil(|r| / gaps); the result
   is the same for the same input; 1 and 0 glyphs. Record CORE-140.
2. In gdi.rs give the zoomed page its own path and leave 100 % alone:
   - Under a zoom, paint_page draws in device pixels with no mapping mode, as Win16 does: every position
     is view::dev of the page-unit position, measured from the first sheet's corner at (x0, area.top),
     rounded down so edges that meet in the layout meet on the screen. map_pages stays only for the
     preview (Z2 removes it).
   - font_handle_at gains the zoom: height -((size * dpi * pct + 7200) / 14400).max(2) at 96 dpi
     (Win16's formula), kept in the same ring with the zoom in its key.
   - A cache of the zoomed fonts' widths (GetCharWidthA 32..255 on mdc with the zoomed font selected,
     once per font and zoom, a ring of 16 like MEASURED).
   - draw_line under a zoom splits a run into words at spaces (a space never goes out; the next word
     starts at its own layout x, so justified spaces are the layout's). For each word: x = dev(layout x),
     end = dev(layout x + sum of layout advances); the advances are the zoomed font's widths, fitted with
     quill_core::fit::fit_advances(&mut dx, end - x); one ExtTextOutA with that lpDx. A selected word fills
     from x to end (the cells touch). The wave, tab leaders (eight dots a call at the dots' device x),
     fields, list labels, pictures and their handles (W9X-11, now in pixels: a 1-pixel frame and
     quill_app::picture_geom::HANDLE pixels) and the caret (2 pixels wide) all go to device pixels.
   - At 100 % nothing changes: the runs and ExtTextOutA with the layout's advances, as today.
3. tests/docs/zoomtext.rtf as zoom-review.md "Goldens" says, and tools/zoom_text.py: `frames DIR` reads
   z050.png .. z200.png and z100.png and runs the measured check; exits 1 naming the line, word and
   widths that fail.
4. win9x/tools/wine-zoom-text.sh [--shots DIR] [--update]: modelled on wine-zoom.sh (its own Xvfb,
   default :183, QUILL_ZOOM_TEXT_DISPLAY; WINEPREFIX under ~/.cache/quill-wine-zoom-text; unset
   WAYLAND_DISPLAY; DISPLAY passed explicitly to every wine, xdotool and import call; kill only the PIDs it
   started). For each zoom write [Quill] Zoom=N and Blink=0 to QUILL.INI, start QUILL32.EXE on
   zoomtext.rtf, crop the page's text block, run tools/zoom_text.py, and compare against
   tests/golden/zoom/win32-wine-zNNN.png as the doc says (--update rewrites them and WINE_VERSION).
   Run it on origin/main first and keep the failing figures for the PR.
5. Update SPEC.md (Win32 zoom), docs/decisions.md (W9X-80, CORE-140; W9X-11's handles now in pixels),
   docs/progress.md.

Gates: G-HOST, G-W32 (with wine-zoom.sh, wine-layout.sh, wine-picture.sh, wine-pixels.sh,
wine-zoom-text.sh). Budget: QUILL32.EXE ≤ +4 KB, no new import; state the DOS size and Win16 data figures
unchanged. Evidence: ~/Documents/Liminal HQ/Quill/Zoom review/Z1/ (before- and after- at 50 to 200 %, page
and 3x crops).

If the device-pixel page turns out larger than the brief expects, OWNER DECISION default: keep device
pixels for the whole zoomed page (one coordinate system, Win16's); the swap is to draw only the text in
device pixels and keep the mapping for fills and pictures, inside draw_line's zoomed branch.
```

## Z2: Win32 print preview text

- **Finish line:** the Win32 print preview, one page or two, fitted or actual size, draws its body, header and footer text through Z1's device-pixel path at the preview's scale; the mapped `per_char` path is gone.
- **Owns:** in `win9x/quill-win9x/src/gdi.rs` `paint_preview`, `draw_page`, `PageLayout`, `map_pages` (deleted) and the mapped branch of `draw_line` (deleted); `win9x/tools/wine-zoom-text.sh` (a `--preview` mode); `tests/golden/zoom/win32-wine-preview-*.png`.
- **Must not touch:** `print_document`'s device path (the printer stays at its own dpi with no zoom); `crates/**`; `chrome.rs`; `win16/**`.
- **Depends on:** Z1 merged (same file).
- **Budget:** `QUILL32.EXE` Δ ≤ 0 (code goes away); no new import.
- **Tests:** G-W32 with `wine-print.sh`, `wine-zoom-text.sh` and `wine-zoom-text.sh --preview`: the preview at the two window sizes that give about 50 and 75 % and at actual size, one page and two, measured against the page view's 100 % frame scaled by the preview's percentage (the status line or `preview_scale` gives it), with goldens.
- **Evidence:** `before-` and `after-win32-wine-preview-{one,two,actual}`.
- **Decisions:** W9X-81.

Brief:

```text
Package Z2: the Win32 print preview draws its text like the zoomed page. Branch fix/win32-preview-text,
worktree ~/source/worktrees/quill/win32-preview-text. Starts after Z1 has merged.

Read docs/plans/zoom-review.md (Z2), W9X-80, D81, and gdi.rs paint_preview, draw_page and preview_scale.
paint_preview maps each sheet with map_pages(pct) and draw_page draws with per_char at that scale: the
Z1 cause at a scale that is rarely a menu level (10 to 200 %).

1. Give draw_page the preview's percentage and the sheet's corner in device pixels, and draw its header,
   footer and body lines through draw_line's zoomed path (LineCtx.pct is the real scale, not 100). The
   printer calls draw_page at its own dpi with pct 100 and no zoom: its output must not change (compare
   wine-print.sh's output file before and after, byte for byte).
2. Delete map_pages, the per_char flag and the mapped branch of draw_line.
3. Add --preview to win9x/tools/wine-zoom-text.sh: open File > Print Preview by the keys (Alt+F, V), at a
   window size that gives about 50 % and one that gives about 75 %, then Two Pages and Zoom (actual size),
   crop each sheet's text, check it with tools/zoom_text.py against the page view's 100 % frame scaled by
   the preview's percentage, and keep goldens as Z1 does.
4. SPEC.md (preview), docs/decisions.md W9X-81 (and D81's "scales the 96 dpi page with a mapping mode" is
   superseded), docs/progress.md.

Gates: G-W32 with wine-print.sh, wine-zoom-text.sh, wine-zoom-text.sh --preview. Budget: QUILL32.EXE
Δ ≤ 0. Evidence: ~/Documents/Liminal HQ/Quill/Zoom review/Z2/.
```

## Z3: Win16 phantom sheet

- **Finish line:** the Win16 page draws no sheet past the document's last page at any zoom, and a zoom change brings the scroll position back inside the new range.
- **Owns:** in `win16/examples/quill/gdi.rs` the sheet loop of `paint_page`; in `win16/examples/quill/main.rs` the zoom branch of `run_command`; `win16/tools/e2e-zoom-w31.sh` and `win16/tools/check_zoom.py` (a one-sheet check); `tests/golden/zoom/win16-311-z*.png` (new).
- **Must not touch:** `crates/**`; `draw_line` and the text path (Z8); anything preview (Z5); `win9x/**`.
- **Depends on:** nothing. Meets WP-ARCH-1 only on `use` lines.
- **Budget:** Win16 data Δ 0 (code only); the code segment holding `paint_page` stays under its budget in `segments.json`.
- **Tests:** G-W16 with `e2e-zoom-w31.sh` and `e2e-scroll-w31.sh`; the new frames at 50, 75, 100, 125, 150 and 200 % of `zoomtext.rtf` (one page) and `tests/docs/scroll.rtf` (several) as exact goldens; at 50 and 25 % on the one-page document everything below the sheet's shadow is the desk's grey.
- **Evidence:** `before-` and `after-win16-311-page-z050` and `z025`, one page and several.
- **Decisions:** W16-80.

Brief:

```text
Package Z3: the Win16 page shows only the document's own sheets. Branch fix/win16-sheets, worktree
~/source/worktrees/quill/win16-sheets.

Read docs/plans/zoom-review.md (Z3), W16-15, CORE-64, and win16/examples/quill/gdi.rs paint_page: its sheet
loop stops only when a sheet's top passes the window's bottom (or p > 400) and never asks
app.page_total(s), so at 50 % a one-page document shows a second, empty sheet (a letter sheet is 528
pixels). Win32 stops at body_pages and the DOS painter at page_total.

1. Bound the loop by app.page_total(s).max(1) (it is already computed further down; compute it once).
2. In run_command, after a Command::Zoom, bring the scroll position back into the new range:
   app().set_vscroll_pos(store(), app().vscroll_pos(store())) after the view height is set for the new
   zoom, then the bars. (The rule for showing the page's top when it fits is Z4's, after WP-ARCH-1.)
3. Extend e2e-zoom-w31.sh and check_zoom.py: `onesheet FRAME BX BY0` exits 0 when every pixel below the
   first sheet's shadow is the desk's grey; frames at 50, 75, 100, 125, 150 and 200 % (QUILL.INI Zoom) of
   tests/docs/zoomtext.rtf (from Z1; if Z1 has not merged, copy the file as zoom-review.md describes it
   and let whichever merges second keep one) and tests/docs/scroll.rtf, saved as exact goldens under
   tests/golden/zoom/win16-311-zNNN.png, compared with compare -metric AE (0 pixels).
4. SPEC.md, docs/decisions.md W16-80, docs/progress.md.

Gates: G-W16 with e2e-zoom-w31.sh and e2e-scroll-w31.sh. Budget: data Δ 0 (state the figure, at least
1,024 free), stack margin ≥ 2,048, segments within budget. Evidence: ~/Documents/Liminal HQ/Quill/Zoom
review/Z3/.
```

## Z4: the paging rule on a zoom change

- **Finish line:** on every build with a pixel page view (Win16, Win32, DOS graphics), when the zoom changes or the window is resized: if the caret's whole sheet fits in the view, the view shows that sheet from its top (the desk pad above it visible); otherwise the scroll position is clamped to the range and the caret kept in view. When the whole stack fits, the view cannot scroll. A second sheet shows only when the document has one.
- **Owns:** `crates/quill-app/src/scroll.rs` (after WP-ARCH-1: a new `App::fit_view_to_page` and the change to `vscroll_max`); `crates/quill-host/tests/scroll.rs`; `win9x/quill-win9x/src/view.rs` (`vscroll_max`); the zoom and resize branches of `win9x/quill-win9x/src/ui.rs`, `win16/examples/quill/main.rs` and `dos/quill-dos/src` (the graphics shell's zoom command only); `crates/quill-paint/tests/zoom.rs` goldens if they move.
- **Must not touch:** `crates/quill-chrome/**` beyond a call (WP-ARCH-1's); painters (`gdi.rs` in either Windows build, `crates/quill-paint/src/page.rs`); `quill-app` files other than `scroll.rs` and its line in `lib.rs`.
- **Depends on:** WP-ARCH-1 merged; Z6 merged (the ED lane); Z3 merged (Win16 `main.rs`).
- **Budget:** DOS Δ ≤ +0.5 KB **[E]**; Win16 data Δ 0; `QUILL32.EXE` Δ ≤ +1 KB.
- **Tests:** G-HOST with host tests in `crates/quill-host/tests/scroll.rs`: a one-page letter at 50 % in a view taller than the sheet has a range of 0 and shows the sheet's top; zooming out from 100 % scrolled to the foot of page 1 shows page 1's top; a two-page document at 50 % in a tall view shows page 2 below page 1 and nothing below it; a document too tall to fit keeps the caret in view. G-DOS (the graphics zoom scenarios), G-W16 with `e2e-zoom-w31.sh` and `e2e-scroll-w31.sh`, G-W32 with `wine-zoom.sh` and `wine-zoom-text.sh`.
- **Evidence:** zoom out from a scrolled 100 % to 50 % on each build, `before-` and `after-`.
- **Decisions:** CORE-141, W9X-82.

Brief:

```text
Package Z4: a zoom change shows the whole page from its top when it fits. Branch fix/zoom-paging,
worktree ~/source/worktrees/quill/zoom-paging. Starts after WP-ARCH-1, Z6 and Z3 have merged.

Read docs/plans/zoom-review.md (Z4), CORE-64, CORE-65, W16-15, W9X-14, and after WP-ARCH-1
crates/quill-app/src/scroll.rs (vscroll_pos, vscroll_max, set_vscroll_pos) and quill_chrome::view
(PageGeom, zoom_pct). App::set_zoom stores the zoom and nothing else, so the scroll position (page units)
survives a zoom-out and the page's top can stay off screen; Win32's view::vscroll_max lets the last
sheet's bottom reach the middle of the view, so a one-page document that fits still scrolls.

1. In scroll.rs, App::fit_view_to_page(&mut self, s, view_h_page_units): if the caret's sheet plus
   PAGE_TOP above it and the shadow below fits in the view, scroll so that sheet's top is PAGE_TOP below
   the view's top (or to 0 when the whole stack fits); otherwise clamp to vscroll_max and keep the caret's
   line in view. vscroll_max is 0 when the whole stack fits. Host tests as zoom-review.md lists.
2. Call it from each shell after a zoom change and after a resize that changes the view height: Win16
   run_command's zoom branch (replacing Z3's re-clamp) and WM_SIZE; Win32 ui.rs's Command::Zoom path and
   WM_SIZE, with win9x view::vscroll_max returning 0 when the stack fits (W9X-82); the DOS graphics
   shell's zoom command.
3. SPEC.md (paging), docs/decisions.md CORE-141 and W9X-82, docs/progress.md.

OWNER DECISION default: snap to the caret's page top when it fits. The swap is a clamp only (no snap), in
fit_view_to_page.

Gates: G-HOST, G-DOS, G-W16 (e2e-zoom-w31.sh, e2e-scroll-w31.sh), G-W32 (wine-zoom.sh,
wine-zoom-text.sh). Budget: DOS ≤ +0.5 KB, Win16 data Δ 0, QUILL32.EXE ≤ +1 KB. Evidence:
~/Documents/Liminal HQ/Quill/Zoom review/Z4/.
```

## Z5: Win16 print preview

- **Finish line:** File > Print Preview and the toolbar's preview button open a preview of the page as it will print in the Win16 main window: one sheet fitted to the page area, Page Down and Page Up to the next and previous page, `2` for two pages, `P` to print, Esc to close, the status line saying `Page 1 of 2` and the keys. Text is drawn word by word at the preview's scale through the Win16 zoomed path.
- **Owns:** `win16/examples/quill/preview.rs` (new); in `main.rs` the `Command::Preview` intercept in `run_command`, the preview state in `Shell`, and the key, paint and close branches that check it; in `gdi.rs` a `pub fn paint_sheet(dc, area, app, s, page, pct, ...)` that draws one page's header, footer and body at a scale from the existing `draw_line`; `win16/examples/quill/segments.json` (the new module's line); `win16/examples/quill/resources.json` (a string table entry if used); `win16/tools/e2e-preview-w31.sh` (new); `tests/golden/zoom/win16-311-preview-*.png`.
- **Must not touch:** `crates/quill-app/**` (the `"That is not available"` arm stays for the other builds; Win16 intercepts before it); `chrome.rs` beyond hiding the zoom control while previewing; `draw_line`'s word path (Z8).
- **Depends on:** Z3 merged (`gdi.rs`, `main.rs`).
- **Budget:** Win16 data ≤ +256 bytes, at least 1,024 free after (W16-60); the status text and key hints from a string resource (`LoadString`, USER.176, already bound in `win16/src/imports.rs`) rather than near data; stack margin ≥ 2,048; segments within budget.
- **Tests:** G-W16 with the new `e2e-preview-w31.sh`: the menu opens the preview (the frame changes and shows one sheet with text); Page Down on `tests/docs/scroll.rtf` shows page 2 and the status line says so; `2` shows two sheets; Esc brings the page back unchanged; `zoomtext.rtf` checked with `tools/zoom_text.py` against the 100 % page frame scaled by the preview's percentage; exact goldens.
- **Evidence:** `after-win16-311-preview-{one,two,page2}` and the closed state.
- **Decisions:** W16-81.

Brief:

```text
Package Z5: Print Preview on Windows 3.x. Branch feat/win16-preview, worktree
~/source/worktrees/quill/win16-preview. Starts after Z3 has merged.

Read docs/plans/zoom-review.md (Z5), D81, W16-15, W16-60, docs/win16-data-segment.md, the Win32 preview
(win9x/quill-win9x/src/ui.rs preview_open, preview_close, preview_step, preview_command; gdi.rs
paint_preview, preview_scale) and roadmap M6. Why it never opened: win16 run_command intercepts only
Command::PrintSetup; Command::Preview reaches App::command, which answers "That is not available in this
version." There is no Win16 preview code.

1. preview.rs: the state (first page, two pages, 6 bytes or so, in the Shell), preview_scale as Win32's
   (fit one or two sheets with 16-pixel gaps, 10 to 100 %, as u8 for Zoom), open, close, step, and the
   key map: PgDn, PgUp, Home, End, 2, P (Print through the existing print path), Esc.
2. gdi.rs paint_sheet: the desk, the sheet and its shadow, the header, footer and body lines of one page,
   drawn with the existing draw_line at the preview's Zoom (word at a time, W16-15), list labels counted
   from the document's start, fields with their printed values. Lines come from the layout at 96 dpi
   (the page view's), as Win32's preview does.
3. main.rs: run_command intercepts Command::Preview (opens it); WM_PAINT draws paint_sheet while the
   preview is open; WM_KEYDOWN goes to the preview's key map; the toolbar, ruler and editing keys are
   inert while it shows; a click does nothing; the status line shows "Page N of M" and the keys from a
   string resource.
4. win16/tools/e2e-preview-w31.sh as zoom-review.md lists, with exact goldens.
5. SPEC.md, docs/decisions.md W16-81, docs/progress.md; segments.json for the new module.

OWNER DECISION default: keys and the status line, no button bar (D81, and the data segment). The swap is a
row of buttons like Win32's, which costs their captions and hit tests in data.

Gates: G-W16 with e2e-preview-w31.sh, e2e-zoom-w31.sh. Budget: data ≤ +256 bytes and ≥ 1,024 free
(state before and after), stack margin ≥ 2,048, segments within budget. Evidence: ~/Documents/Liminal
HQ/Quill/Zoom review/Z5/.
```

## Z6: the style box reads Normal

- **Finish line:** a paragraph that matches a preset in everything but space before, space after, line spacing and alignment reads as that preset when no preset matches exactly, so the demo's body paragraphs show Normal on every build; a direct change of size, bold, italic, underline or indents still makes the box blank.
- **Owns:** `App::style_of` and its helpers in `crates/quill-app/src/chrome.rs`; `crates/quill-host/tests/chrome.rs` and `crates/quill-host/tests/next_style.rs` (new cases); host goldens whose style box text changes (`QUILL_UPDATE_GOLDEN=1`, reviewed frame by frame).
- **Must not touch:** `PRESETS` values, `apply_style`, `next_preset` (CORE-39), the file formats, `view.rs` and `zoom_field.rs` (WP-ARCH-1), any shell.
- **Depends on:** the ED lane free (no other ED branch open in `crates/quill-app/src`).
- **Budget:** DOS Δ ≤ +0.3 KB **[E]**; Win16 data Δ 0 (a second pass over `PRESETS` is code).
- **Tests:** G-HOST with: each of `samples/demo.rtf`, `demo-reactos.rtf` and `demo-reactos16.qld` reads Heading 1 on its headings and Normal on its body paragraphs; `\sa120` and `\sl360\slmult1` on a 12-point plain paragraph read Normal; a centred 12-point plain paragraph reads Normal; a 12-point plain paragraph with `\li720` reads List Paragraph; one with `\li360` reads blank; every preset reads back as itself (the existing loop); bulleted paragraphs read as before. G-DOS (the graphics toolbar's box), G-W16 and G-W32 smoke (`play-w31.sh m1-edit`, `wine-e2e.sh`).
- **Evidence:** the demo open on each build with the caret in a body paragraph, `before-` and `after-` (`dos-gfx-12h`, `win16-311`, `win32-wine`).
- **Decisions:** CORE-142.

Brief:

```text
Package Z6: the style box shows Normal for a Normal paragraph with space after it. Branch
fix/style-box-normal, worktree ~/source/worktrees/quill/style-box-normal. Takes the ED lane's slot in
crates/quill-app/src: start only when no other ED branch is open there.

Read docs/plans/zoom-review.md (Z6), CORE-39, CORE-40 and crates/quill-app/src/chrome.rs (PRESETS,
style_of). style_of asks for exact equality including space before and after and line spacing; Normal has
no space after, but the samples and most RTF write body paragraphs with \sa120, so they read as None and
every build's box is blank (a host run over samples/demo-reactos.rtf: headings Some(1), body None).

1. style_of: first the exact match, as now. If none, a second pass that compares alignment-free and
   spacing-free: indents (left, right, first), the list flags, the size and the bold, italic and underline
   bits; the first preset that matches wins (PRESETS' order). Title stays distinct by its size.
2. The host tests zoom-review.md lists, in crates/quill-host/tests/chrome.rs; regenerate only the goldens
   whose style box text changed, and say which in the PR.
3. SPEC.md (the style box), docs/decisions.md CORE-142, docs/progress.md.

OWNER DECISION default: the loose second pass. The swap is a style index stored with the paragraph (set by
apply_style and RTF \sN), a file format change for QLD that is its own package.

Gates: G-HOST, G-DOS, G-W16 smoke, G-W32 smoke. Budget: DOS ≤ +0.3 KB, Win16 data Δ 0. Evidence:
~/Documents/Liminal HQ/Quill/Zoom review/Z6/.
```

## Z7: the italic glyph and the pressed button's face

- **Finish line:** the italic glyph is a slanted I whose serifs sit centred on its stem's ends, the same weight and height as Bold and Underline; on Win32 a switched-on or pressed button's glyph shows the button's own base colour behind it, with no square.
- **Owns:** `assets/toolbar/make_toolbar.py` (`glyph_italic`), `assets/toolbar/toolbar.bmp` (regenerated); in `win9x/quill-win9x/src/chrome.rs` `glyph`, `recolour_strip` and `LIVE_STRIP` (a second recoloured copy, or the face entry patched for the base before each draw); host goldens that show the toolbar (`tests/golden/gfx`, regenerated); `win9x/tools/wine-layout.sh` or a new `wine-toolbar.sh` check.
- **Must not touch:** `crates/quill-app/src/view.rs` (it embeds the strip; the path does not change, and WP-ARCH-1 owns the file); the toolbar's layout; Win16 `chrome.rs` (its `bevel` always fills `FACE`, so the opaque glyph is right there).
- **Depends on:** nothing.
- **Budget:** the strip stays 352 by 15 at 4 bits, so DOS Δ 0, Win16 data Δ 0, `QUILL32.EXE` Δ ≤ +0.5 KB.
- **Tests:** G-HOST (the regenerated gfx goldens, reviewed), G-DOS, G-W32 with a check that, with Bold switched on, the 16 by 15 cell inside the button holds no pixel of the face colour where the button's base is the light colour; G-W16 smoke for the new strip.
- **Evidence:** `make_toolbar.py --sheet` at 6x before and after; 4x crops of the Win32 format bar with Bold and Centre on, before and after; the same bar on Win16 and DOS graphics after.
- **Decisions:** W9X-83 (the glyph is drawn on the button's base), and a line in the toolbar's existing decision for the new italic.

Brief:

```text
Package Z7: a cleaner italic glyph, and no square behind a pressed toolbar glyph on Windows 95. Branch
fix/toolbar-glyphs, worktree ~/source/worktrees/quill/toolbar-glyphs.

Read docs/plans/zoom-review.md (Z7) and win9x/quill-win9x/src/chrome.rs (glyph, recolour_strip,
button_face). glyph() copies the cell opaque with SetDIBitsToDevice; recolour_strip sets palette entry 0 to
the face, but a switched-on toggle's base is light() (face mixed with highlight), so the cell shows as a
darker square on a lighter button.

1. Win32: draw the glyph on the colour it sits on. Keep two recoloured copies (entry 0 as face, and as
   light()) built by recolour_strip, or patch entry 0 of LIVE_STRIP to the base just before each
   SetDIBitsToDevice; glyph() takes the base colour. No new import.
2. make_toolbar.py glyph_italic: a slanted stem two pixels wide from about (10,2) to (6,10), the same rows
   as the U glyph, with a top serif centred on the stem's top and a bottom serif centred on its foot, each
   four or five pixels. Regenerate toolbar.bmp and write a --sheet PNG at 6x for the evidence; the owner
   judges the look (put before and after side by side).
3. Regenerate the host goldens that show the toolbar (QUILL_UPDATE_GOLDEN=1), check each changed frame
   differs only in the italic button, and say so in the PR.
4. SPEC.md if it describes the glyphs, docs/decisions.md W9X-83, docs/progress.md.

Gates: G-HOST, G-DOS, G-W32 (wine-layout.sh and the new check), G-W16 smoke. Budget: strip unchanged in
size; QUILL32.EXE ≤ +0.5 KB. Evidence: ~/Documents/Liminal HQ/Quill/Zoom review/Z7/.
```

## Z8: Win16 zoomed words fitted to the layout

- **Finish line:** a zoomed Win16 word ends where the layout ends it, with the gaps inside it from the zoomed font's widths fitted by `quill_core::fit::fit_advances`, so the leftover kerning and spacing drift the owner saw on Win16 is gone; 100 % unchanged.
- **Owns:** in `win16/examples/quill/gdi.rs` the zoomed word path of `draw_line` and a zoomed-widths cache in far memory (W16-11's block, or a second block); `win16/examples/quill/far.rs` if the cache needs it; `tests/golden/zoom/win16-311-z*.png` (regenerated).
- **Must not touch:** `crates/**` (it calls `quill_core::fit` as Z1 left it); the sheet loop (Z3); the preview (Z5).
- **Depends on:** Z1 merged (`quill_core::fit`); Z5 merged (`gdi.rs`).
- **Budget:** Win16 data Δ 0 (the widths in far memory; the `lpDx` array of at most 64 words on the stack, so the stack margin must stay ≥ 2,048); `ExtTextOut` (GDI.351) and `GetCharWidth` (GDI.350) are already bound in `win16/src/imports.rs`.
- **Tests:** G-W16 with `e2e-zoom-w31.sh` and its goldens at 50 to 200 %, checked with `tools/zoom_text.py` (each word's ink width within the larger of 2 pixels and 4 % of the 100 % frame times the zoom); the paint benchmark (`bench-w31.sh`) calls a key at Page Width no higher than before.
- **Evidence:** `before-` and `after-win16-311-page-z050` to `z200` with crops.
- **Decisions:** W16-82.

Brief:

```text
Package Z8: fit each zoomed word on Windows 3.x to the layout's width. Branch fix/win16-zoom-fit,
worktree ~/source/worktrees/quill/win16-zoom-fit. Starts after Z1 and Z5 have merged.

Read docs/plans/zoom-review.md (Z1, Z8), W16-11, W16-15, W16-60, docs/win16-data-segment.md and
crates/quill-core/src/fit.rs. Win16 already draws a zoomed word at its layout x in a font made at the
zoomed height; inside the word the font's own advances apply, so the word's end drifts by a pixel or two
from the layout's (the kerning and spacing issues in the QA).

1. A cache of the zoomed fonts' widths in far memory (GetCharWidth once per font and zoom), no near data.
2. The zoomed word path: dx[i] from that cache, fit_advances(&mut dx, dev(layout end) - dev(layout x)),
   one ExtTextOut with lpDx (16-bit words). Selection cells and leaders as now.
3. Regenerate tests/golden/zoom/win16-311-z*.png and run tools/zoom_text.py on them.
4. docs/decisions.md W16-82 (W16-15's "drift of a pixel or two" is gone), docs/progress.md.

Gates: G-W16 with e2e-zoom-w31.sh and bench-w31.sh. Budget: data Δ 0 (state it, ≥ 1,024 free), stack
margin ≥ 2,048, segments within budget. Evidence: ~/Documents/Liminal HQ/Quill/Zoom review/Z8/.
```

## Owner decisions

Each brief takes the default and keeps the swap small.

| Question | Default taken | Where to swap |
|---|---|---|
| Win32 zoomed page: device pixels throughout, or device pixels for text only under the old mapping | Device pixels throughout, as Win16 | Z1 step 2, W9X-80 |
| A zoomed word: fit its inner gaps to the layout's end, or let it drift inside the word as Win16 does now | Fit (`quill_core::fit::fit_advances`) | Z1 step 1 and Z8, CORE-140 |
| Style box: a loose second match ignoring spacing and alignment, or a stored style index | Loose second match | Z6 step 1, CORE-142 |
| Win16 preview: keys and the status line, or a button bar like Win32's | Keys and the status line | Z5, W16-81 |
| A zoom change: snap to the caret's page top when it fits, or clamp only | Snap | Z4 `App::fit_view_to_page`, CORE-141 |
| Win32 scroll range when the whole stack fits: none, or half a view of overscroll | None | Z4, `win9x/quill-win9x/src/view.rs` `vscroll_max`, W9X-82 |

## Not in this review

- **Item 4** (Win16 at 100 % a little wonky, the caret slow): the text is the Windows 3.11 raster fonts under WineVDM; the caret's speed belongs with the blink timer (W16-17) and the paint benchmark, and needs its own measurement on DOSBox-X before a brief.
- **The DOS graphics page** places each character at `dev` of its layout x with a strike chosen for the zoomed size (CORE-68, DOS-44), so it can show the Z1 effect in small form; `quill_core::fit` would serve it too, as a later DOS package.
- **Items 7 to 13** (the zoom control, the remix, the stretch ideas) are not zoom rendering.
- **ReactOS** was not run: the Wine frames show the cause, and the fixes stop relying on how a system realises a font under a mapping mode. The owner's ReactOS image is the check after Z1 and Z2 merge.
