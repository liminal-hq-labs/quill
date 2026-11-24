# Quill web and layering addendum: GTK4, WinUI 3, web (wasm32) and Desk

Written 2026-11-24 by an Opus planning agent (read only), saved by the orchestrator. Builds on "Quill architecture review.md", "Quill work queue.md", "Quill WinUI 3 plan.md" and "Quill Linux client plan.md".

**[V]** means checked in code or a document. **[G]** means a guess. **[X]** means outside knowledge that was not checked.

An earlier draft of this plan proposed a single request outbox (`App::take_request`) and a `quill-vfs` crate. This final version folds the outbox idea into `quill-session` and the deferred file commit into `quill-caps`. The threading planner's work (D250 onward) may revise the idle and timer parts.

## What was read

- The four Quill planning documents above.
- Quill code on `origin/main` at ee39d9f (#51), through `git show`. The checkout at `/home/scott/source/experiments/quill16` is on `feat/m5-win16`, 93 commits behind.
- `scripts/check-layers.py` and `layer-allow.txt`, which exist only on the unmerged `feat/layer-check` branch (005c97e).
- `docs/desk-app.md`, only on the unmerged `docs/desk-app` branch (ac79130).
- `docs/dos-graphics.md` on `origin/main`.
- Desk on `origin/main` at 4cbf588. The local Desk checkout is 119 commits behind.

Nothing was built and no `cargo` command was run.

## 0. Findings that change other plans

1. **Decision numbers clash.** The architecture review recommends D171–D189. Linux has moved to D180–D199 (Quill commit 1d182d2, and `dos-graphics.md` cites D196) [V]. So D180–D189 is claimed twice. Suggestion: layering decisions use D171–D179 plus D200–D209. D200 is now taken by the Windows 3.x data-segment work (#51), D201–D209 by the DOS graphics slice, D210–D214 by PX1, D250–D259 by the threading plan. **Needs a user decision.**
2. **D130 settles the Desk question.** The owner decided that Quill gets its own DOS graphics mode, reusing Desk's drivers, with a Desk port staying possible through `Surface` [V]. The unmerged `desk-app.md` proposal to replace the DOS graphics milestone with the Desk port is out of date. Desk is a fifth shell family that comes after DOS graphics, not instead of it.
3. **D220–D249 are free** (the only `D2xx` hit in the repo was one stray D200, since taken).
4. **wasm tooling is partly installed.** `wasm32-unknown-unknown` is installed for the pinned 1.98 toolchain, and so are `wasm-bindgen` and `wasm-pack`. `wasm-opt` and `trunk` are not [V].

## 1. What blocks or complicates a wasm32 web shell, by crate

A grep of the portable crates for `static mut`, `asm!`, `target_arch`, `f32`/`f64`, threads, `Instant`/`SystemTime` and `std::` outside tests found **no hits** in `quill-core`, `quill-formats`, `quill-inflate`, `quill-image`, `quill-spell`, `quill-grid`, `quill-app`, `quill-dlg`, `quill-paint` or `quill-strike` [V]. wasm32 has a 32-bit `usize`, like the i386 targets already built, so that adds nothing new. Nothing was compiled; the first spike PR proves the "it compiles" claims [G].

| Crate | Status | What blocks or complicates the web build |
|---|---|---|
| `quill-core` | `no_std`, no `alloc` | Nothing found. |
| `quill-formats` | `no_std` | Nothing. Web text is UTF-8, so C4 (UTF-8 read and write) matters more here than anywhere else. |
| `quill-inflate` | `no_std`, `forbid(unsafe)` | Nothing. Its 32 KB window fits easily under wasm's default stack [G]. |
| `quill-image` | `no_std` | Nothing. The dither palette argument is needed for the RGBA path. |
| `quill-spell` | `no_std` | `App` opens `EN_CA.DIC`, `EN_US.DIC` or `EN_GB.DIC` by file name through `Files` (`quill-app/src/spelling.rs:84-120`) [V]. A browser must fetch the dictionary asynchronously first, and the `.DIC` is built by `quill-dict`, not committed. |
| `quill-grid` | `no_std` | Nothing. It is only pulled in by the `GridMeasure` default. |
| `quill-app` | `no_std`, no `alloc` | Contract assumptions rather than compile errors [V]: (a) `Files` is synchronous and takes `name: &[u8]`, and `close` returns `bool` at once (`lib.rs:127-139`), while File System Access, IndexedDB and `fetch` are async; (b) `load_settings` and `save_settings` open the literal `QUILL.INI` (`lib.rs:772-800`); (c) `Key::Char(u8)` is ANSI only, with no composition or preedit (`lib.rs:53`); (d) the clipboard is an internal `clip: [u8; 2048]`; (e) `view.rs:29` fixes `DPI = 96`; (f) `take_help` assumes a WinHelp-style viewer; (g) `platform::os_name` only knows Windows. Date and time are already pushed in by the shell (`print.rs:33`), which is good. |
| `quill-dlg` | `no_std` | Assumes 8.3 names and drive letters. The web shell uses pickers, so this is not a blocker. |
| `quill-paint` | `no_std` | Depends on `quill-app` and on `quill-strike` unconditionally. There is no RGBA surface yet (PX1 is in progress). It decodes pictures on every paint, which is costly on the web. |
| `quill-strike` | `no_std` | Nothing. |
| `quill-heap` | `alloc` | Works with Rust's own wasm32 allocator [G]. It needs a limit, because browser tabs die rather than refuse an allocation. |
| `quill-fonts` | `std` | `set.rs:131-215` reads `std::fs` and `HOME`/`XDG_*`. On wasm this probably compiles and finds nothing [G]. `vendored()` embeds the 12 Quill Test faces, about 213 KB [V], and skrifa is pure Rust. |
| `quill-host` | tool | Must stay out of the web dependency graph. `quill-tty` and `quill-cli` still depend on it [V]. |
| `quill-raster` (planned, PX1) | `std` | tiny-skia uses `f32`, which is allowed in a std kit crate and fine on wasm. It should be `alloc` rather than `std`, because it does no I/O. |

Cross-cutting issues:

- **Threads:** none used anywhere, so a single-threaded wasm build with no COOP/COEP headers is fine. GitHub Pages cannot set those headers.
- **Floats:** `devicePixelRatio` and DOM pointer coordinates are `f64`. Convert at the shell edge to `scale_120` and `i32`, exactly as the GTK plan does.
- **Panics:** the release profile sets `panic = "abort"` [V], so a panic is a wasm trap and no emergency save can run. Autosave to IndexedDB covers this.
- **Global statics:** none in portable crates. Only shells have them (for example Win16's `CMDS`).
- **Time:** no clock in the core. The shell pushes the date, the time and timer ticks.
- **Binary size [G]:** about 0.8–1.5 MB of wasm before `wasm-opt`, including fonts. Needs a budget.

## 2. Splits and tightenings beyond the architecture review

Ranked by how much they share across GTK4, WinUI, web and Desk. Each crate needs two users on the day it lands (the review's D79 rule); the web and Desk count as users.

| Rank | Proposal (decision) | What moves in | What stays | Where it sits |
|---|---|---|---|---|
| R1 | **`quill-caps`** (L1 base, `no_std`), D221 | Host capability traits: `Clock` (date and time pieces) and `Idle` (`App::next_deadline()` plus `App::tick(now)`, for caret blink, autosave and later spelling as you type); `Resources` (fetch by logical name: dictionary, help, `QUILL.FNT`) replacing file names inside the editor; a **deferred commit** where `Files::close` returns `Commit::{Done, Failed, Pending(Token)}` and the shell later calls `App::file_committed(token, ok)` (serves web, GTK portals and Desk); and a `NameTable` mapping real UTF-8 or UTF-16 paths to 1252 display names (the same rule repeated in the GTK, WinUI, web and Desk plans). | The `Files` streaming shape, and `Store`, stay in `quill-base`. | After B1 and before C4. `App::load_settings(Files)` goes once C4's `Profile` lands. |
| R2 | **`quill-input` gets an event model**, D222 | `Event::{Key(Key, Mods), Text(…), Compose{Start, Update(preedit, cursor), Commit, Cancel}, Pointer{Down/Up/Move/Wheel, x, y in device pixels, button, clicks}}`. `Text` goes through `quill-text`'s refusal of characters with no 1252 form, with a count. A `caret_rect()` query places the IME window, and a `Preedit` is drawn at the caret by the painter. | GDK keyval, DOM `KeyboardEvent`, VK and Desk event mapping stay in each shell. | Widen C1 (call it C1b) before G2, W4 and WEB4. |
| R3 | **`quill-session`** (L3 kit, `no_std`), D223 | The pixel-shell driver that the Win9x `Shell` struct, the GTK `QuillShell`, the WinUI shell, the web shell and the Desk app would otherwise each re-implement: input events into `App::key`, `command` and pointer calls; the request queue (dialog, help, clipboard, file commit); flows; damage rectangles and the repaint request. Generic over a `Host` bundle of R1 traits plus `Clipboard`. The pixel counterpart of `quill-tui`'s `TextShell`. | OS calls, widgets and event loops stay in shells. | After C6 and C7, before G1, W3 and WEB3. |
| R4 | **Menu state and neutral dialog descriptions**, D224 | `quill_flow::menu_state(app)` returns an enabled/checked bitset per `Command`, so one function feeds GMenu actions, the WinUI `MenuBar`, the HTML menu and Desk's menu enabling. `dialogs.json` gains control kinds that map onto Desk's control kinds and onto HTML elements. | `menu_box` (text cells) stays in grid. | Inside C2 and A2. |
| R5 | **`quill-a11y`** (L3 kit, `no_std`), D225 | A read-only text-accessibility snapshot: the visible lines as UTF-8; per-character x offsets in device pixels through `Mapping`; caret, selection, line bounds, role and name; a `revision` counter. Adapters: WinUI through AccessKit (the S0b spike already builds `MultilineTextInput` and `TextRun` nodes in `winui/spike/src/s0b/child.rs:93-145` [V]), GTK through `GtkAccessibleText`, and a web ARIA mirror. Desk has none. | Platform APIs stay in the shells. | After P1, before G7, W9 and WEB7. Lands when two users exist. |
| R6 | **Tighten `quill-paint`**, D226 | Remove the `quill-app` dependency through `quill-chrome`'s `PageGeom` and `quill-picture`. Put `Planar4`/`Linear8` behind a `strike` feature. Add a `RecordSurface` (an `alloc` feature) that records drawing ops, for text goldens, damage diffs and Desk command-mode tests. | The `Surface` trait as it is. | With P2 and P3. |
| R7 | **Split `quill-fonts`**, D227 | `quill-outline` (`alloc`): `Face`, `OutlineMeasure` and glyph rasterizing. `quill-fonts` (`std`): only `FontSet::system` folder discovery. | Vendored fonts go with `quill-outline`. | Low priority. Only matters for no-std hosts using outline fonts, such as Desk's native track. Whether skrifa and tiny-skia build with no `std` is a guess. |
| — | **No separate display-list crate now**, D228 | The `Surface` trait already is the seam between layout and painter. Desk's `fill` and `text` calls are a command API a `Surface` can call directly. `RecordSurface` covers testing and diffing. | | Promote to a crate if a retained-mode host appears. |

**Rule (D229):** no crate in L1, L2 or L3 names a browser, Desk or an OS. `check-layers.py`'s platform pattern gains `wasm-bindgen|web-sys|js-sys|desk-.*`.

## 3. Web build spike

**Where it lives (D230):** `web/quill-web`, its own workspace, using `wasm-bindgen` and `web-sys`; a static `web/www/` page; `App<OutlineMeasure>` over `HeapStore` and an in-memory `MemFiles`.

**Hosting (D231):** `pages.yml` today uploads `site/` only and triggers only on `site/**` [V]. The build job gains steps to install wasm32 and wasm-bindgen, build, run `quill-dict` for `EN_CA.DIC`, and copy everything into `site/app/` inside the artifact. Nothing generated is committed. Trigger paths gain `crates/**` and `web/**`. The app is served at `labs.liminalhq.ca/quill/app/`.

**Design choices:**

- **Canvas (D232):** `canvas.width` is the CSS width times `dpr`, with `scale_120 = round(dpr × 120)`. The page is drawn with `putImageData` from `quill-raster` (PX1) tiles.
- **Fonts (D233):** only the vendored Quill Test faces; system font bytes are not readable from a page.
- **Input (D234):** a hidden `<textarea>` follows the caret rectangle. `keydown` handles navigation and Ctrl chords (with `preventDefault`); `beforeinput` and `input` handle `insertText`; `compositionstart`, `update` and `end` handle preedit. The textarea is cleared after each commit, and pointer events go to `pos_at_point`.
- **Accessibility (D235):** the textarea holds the caret's paragraph with its selection mirrored, plus an `aria-live` status line. Whether NVDA and Orca read this well is a guess until a manual pass.
- **Files (D236):** open uses `showOpenFilePicker`, then `arrayBuffer`, then `MemFiles`, then `app.open`. Save uses `createWritable`, then `write`, then `close`, then `file_committed`. Browsers without File System Access fall back to `<input type=file>` and a Blob download (Firefox and Safari have no File System Access [G]).
- **IndexedDB (D237):** recent files as file handles, the INI text and autosave bytes. Write every 60 s and on `visibilitychange` and `pagehide`; offer recovery at the next start.
- **Clipboard (D238):** DOM `copy`, `cut` and `paste` events give synchronous `clipboardData` with `text/plain` and `text/html`, not RTF [G]. `quill-clip` gains `Flavour::Html`, which needs the HTML writer (P7a).
- **Later (D239):** help as HTML generated from `.qhm`; printing through `window.print` of page images.
- **Optional tier 0 (D220):** put `QUILLDOS.EXE` on the site through js-dos. Desk's S5 spike ran DOS programs, including the VBE linear frame buffer, under js-dos 8 in headless Chromium [V]. It needs no Rust work and gives a demo in days.

**PRs, each sized for one Sonnet builder:**

| PR | Scope | Pass criteria | After |
|---|---|---|---|
| WEB0 | `docs/web.md`, reserving D220–D239 | markdownlint passes | Now |
| WEB1 | CI job: `cargo check --target wasm32-unknown-unknown` for core, formats, inflate, image, spell, grid, app, dlg, paint, strike, heap and fonts | 0 errors; a list of fixes if any appear | Now |
| WEB2 | `web/quill-web` in text mode: `quill-grid` cells on a canvas with a CP437 cell font, `MemFiles`, `keydown` mapped to `Key` | `letter.rtf` shows. Playing the existing `tests/golden/screens` scripts in headless Chromium gives the same grid text. Gzipped wasm ≤ 600 KB [G budget] | WEB1. Does not wait for PX1 |
| WEB3 | Page canvas through `quill-raster` | Canvas pixels at `dpr` 1, 1.5 and 2 equal the host PX1 goldens byte for byte. First paint under 2 s on the CI runner | P1 (PX1), C5 |
| WEB4 | Textarea input, composition and the refusal message | CDP `Input.imeSetComposition` and `insertText` of "Café" store 0xE9. An emoji is refused with a status message. A scripted session saves bytes equal to `tests/golden/files` | WEB3, C1b |
| WEB5 | Files: File System Access plus the input and download fallback | In CI, open with `setInputFiles` and save through the download event, round-tripping every file in `tests/docs` with equal bytes. File System Access checked by hand in Chromium | WEB3, R1 |
| WEB6 | IndexedDB: settings, recent files, autosave and recovery | After a reload the recent list holds the file. After the page is killed, recovery restores the same bytes | WEB5 |
| WEB7 | ARIA mirror through `quill-a11y` | The Playwright accessibility snapshot shows the caret line's text and the caret offset. axe reports 0 critical issues. Manual Orca or NVDA pass | WEB4, R5 |
| WEB8 | Clipboard (text and HTML) | Copying bold into LibreOffice through HTML keeps bold. Pasting text works | C7, P7a |
| WEB9 | HTML menus and dialogs from `quill-menu` and `dialogs.json` | Every `Dialog` variant is shown and answered through a test hook | C2, A2, C6 |
| WEB10 | Pages deployment | The deployed URL loads, `check-site.py --external` passes, the app loads under `/quill/app/` | WEB3 (WEB2 can ship first) |

## 4. Desk OS as the fifth shell family

**What Desk offers today [V]:**

- `desk-hal`: `KeyEvent {code, pressed, modifiers}`, `MouseEvent {dx, dy, buttons, wheel}` and `InputSource` (`hal/hal/src/input.rs:186-225`).
- `desk-gfx`: an 8-bit palette-index back buffer `Surface` (`shell/gfx/src/surface.rs:11`), damage tracking, the compositor and presenters.
- `desk-font`: four `.DFN` bitmap faces, plus Desk Sans, Mono and Serif outlines.
- `desk-ui`, `desk-wm` and the `.DMD`/`.DKX` module format (`desk-module`, `desk-guest`).
- The kernel loads `HELLOWIN.DMD` from an embedded buffer and draws a window through only `desk_window_new`, `desk_fill` and `desk_text`. No input reaches the module and it is not loaded from disk (R48's "stopgap and not the app interface").
- `SDK.md` says the S1 DXE3 spike proved only `desk_log`, `desk_alloc`, `desk_free`, a stub `desk_window_new` and `desk_panic`. Everything else is design.

**What Desk still lacks [V, ROADMAP]:** R79 (app interface), R75 (filesystem), R88 (shared chrome), R93 (clipboard), R141–R143 (threads, the window system as a task), `std` ("later" in `DESK-STD.md`), and any accessibility.

**How Quill maps onto Desk:**

| Quill seam | Desk |
|---|---|
| `quill-caps` `Clock` and `Idle` | `desk_now` (BCD), `desk_ticks`, `desk_timer`. Needs an idle event. |
| `Files` with deferred commit | `desk_file_read` and `write`, whole file only. Needs a positioned read plus a size. |
| `Profile` | `desk_setting_get` and `put`. A direct fit. |
| `Resources` | Dictionary and `QUILL.FNT` through a positioned read. |
| `Clipboard` | `desk_clip` with `text` (UTF-8) and `bitmap`. Needs an `rtf` or app-private format. |
| `quill-input` events | `key` and `char` events with `mods`. Desk strings are UTF-8, so they pass through `quill-text` refusal. No IME, so `Compose` is never used. |
| Painter, route A (recommended, D241) | `quill-paint` `Linear8` with `quill-strike` into a client buffer, then `desk_blit` of palette indices from a `desk_palette_lend` range. The pixels are identical to DOS graphics at VBE 256 colours, so they share goldens. |
| Painter, route B | A `Surface` over `desk_fill`, `desk_line` and `desk_text`. Rejected for the page: colours are roles only, `desk_text_width` has no vertical metrics, and the faces are not Quill's. |
| Fonts | `QUILL.FNT` with the Quill Test faces (D131), or Desk faces as alternative strikes (D140). |
| Menus and chrome | `quill-menu` into `desk_menubar`, `desk_toolbar` and `desk_status`, following Desk's HIG rather than Quill's Windows 3.1-style chrome. |
| Accessibility | None. Recorded as a known gap; `quill-a11y` stays unused here. |

**DOS graphics and Desk share L3 and diverge at L4.** Shared kit: `quill-strike`, `quill-paint` (`Linear8`), `quill-picture`, `quill-chrome` geometry, `quill-session` and `quill-image` palettes. DOS graphics owns mode setting, VBE, presenters, the mouse and its own Windows 3.1-style chrome painter (`dos/quill-gfx`, plus `dos/hal` copied from Desk per `dos/PROVENANCE.md`). Desk owns modes, presentation, window chrome, menus and pickers.

**Dependency direction (D242):** `desk/quill-desk` is an L4 shell in the Quill repo with its own workspace. It depends down on Quill's base, editor and kit crates, and on Desk's `desk-sdk` runtime crate pinned by git revision. Desk never depends on Quill. `check-layers` treats `desk-*` as a platform crate.

**Changes Desk would need**, ordered by what blocks Quill:

1. R79 subset: events delivered to a module (key, char, mouse, paint, menu, timer) and `desk_blit` with palette lending.
2. Positioned file read and file size.
3. A rich-text or private clipboard format.
4. An idle event.
5. Published module size, stack and memory limits. `SDK.md`: a module without LTO is 193 KB and does not load, and a stack overflow takes the whole Desk down. `APP-MODEL.md`: the default thread stack target is 16 KiB. Quill's DOS exe is 212 KB, and Win16 needed about 28 KB of stack.
6. Alt and F10 menu delivery.
7. A HIG rule for document apps' View, Insert and Document menus.
8. A print path.

**Changes Quill would need:** R1–R4; the whole Desk dependency graph buildable `no_std` plus `alloc` for Desk's i486 target with `build-std` (fine except `quill-fonts`, which route A avoids); a `#[global_allocator]` that forwards to `desk_alloc`, in the shell; a measured stack peak per scenario.

**Desk spike PRs:**

| PR | Scope | Pass criteria | After |
|---|---|---|---|
| DK0 | `docs/desk.md`: refresh `desk-app.md` against D130, reserve D240–D249 | markdownlint passes | Now |
| DK1 | Build the portable crates plus `quill-paint` (`Linear8`) for Desk's target spec with `build-std` | They build. Sizes recorded with X1's script | X1 |
| DK2 | Host-only `quill-desk` logic against a fake `DeskHost` (a mock SDK) | The golden scenarios played through Desk event translation save bytes equal to `tests/golden/files` | R1–R3 |
| DK3 | Desk side: the visible-module spike gains input events and `desk_blit` | The module logs the key events from a scenario, and the QEMU digest matches the host | A Desk R79 slice |
| DK4 | `QUILL.DMD` shows `letter.rtf` in native Desk under QEMU | The page-area digest equals the host `Linear8` golden | DK2, DK3, G-series G3 |
| DK5 | Typing and saving through the Desk file write | Saved bytes equal the golden | DK4, Desk R75 |
| DK6 | Menus through `desk_menubar` from `quill-menu` | Each `Command` is reachable. Scenario transcript matches | DK5, R4 |

There is a go or no-go after DK4, tied to Desk's R79 schedule.

## 5. Decisions

**D220–D239 (web and layering):**

- D220: js-dos tier 0, yes or no.
- D221: `quill-caps`.
- D222: input events.
- D223: `quill-session`.
- D224: menu state and dialog descriptions.
- D225: `quill-a11y`.
- D226: `quill-paint` tightening.
- D227: font split.
- D228: no display-list crate.
- D229: the platform-name ban.
- D230: web workspace.
- D231: Pages pipeline.
- D232: canvas and `dpr`.
- D233: vendored fonts only.
- D234: textarea input.
- D235: ARIA mirror.
- D236: File System Access with fallback.
- D237: IndexedDB.
- D238: HTML clipboard.
- D239: help and print later.

**D240–D249 (Desk):**

- D240: Desk is the fifth shell family, after DOS graphics (D130 holds).
- D241: route A (blit) for the page; Desk chrome for menus.
- D242: where `quill-desk` lives, and the direction rule.
- D243: Quill Test strikes, with Desk faces as an option.
- D244: the list of what Quill asks of Desk.
- D245: module limits to be published by Desk.
- D246: no accessibility on Desk, recorded.
- D247: the shared-golden rule with DOS graphics.
- D248 and D249: reserved.

Note: D200 is now used by the data-segment work, so the D220–D249 numbers here keep their meaning; PX1 uses D210–D214, DOS graphics D201–D209, and the threading plan D250–D259.

## Risks

- File System Access is Chromium-only [X]. Firefox and Safari get open and download only, and no recent-file handles.
- Panics trap and lose the session, so autosave matters more on the web.
- tiny-skia's `f32` results might differ slightly between native and wasm code paths [G]; the WEB3 tolerance covers that.
- If R3 slips, GTK, WinUI, web and Desk each grow their own driver. Keep R3 ahead of G1 and W3.
- Widening C1, C2 and A2 makes those PRs bigger. Split any that goes over about 600 lines.
- Desk's app interface (R79) is unbuilt, so the Desk spike is gated on Desk's own schedule.

## Needs a decision from you

1. The D-range fix (layering decisions D171–D179 plus D200–D209, now partly used).
2. js-dos tier 0 (D220): put the DOS build on the site as a quick demo?
3. Web files: Chromium first, with a download fallback (D236)?
4. Web clipboard carries text and HTML, not RTF (D238)?
5. Desk: the blit route and its timing after DOS graphics (D240, D241), and where `quill-desk` lives (D242).
