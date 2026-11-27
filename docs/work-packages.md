# Work packages: the architecture follow-ups and the editor features

The build order for two tracks that share `crates/quill-app`: the architecture follow-ups (`WP-ARCH-*`: `quill-chrome`, `quill-picture`, `quill-session`, `quill-input`, `quill-menu`, `quill-settings`, `quill-flow`, `quill-a11y`) and the editor features on every build (`WP-ED-*`: Unicode U1 and U2, the editing review's PRs 2 to 4, Page Break and word keys, Edit menu state, the rich clipboard, preferences, and the Page Layout, Numbering and Header and Footer dialogs). A third set, Reveal Codes (`WP-CODES-1` to `WP-CODES-10`), is planned in [plans/reveal-codes.md](plans/reveal-codes.md) and placed in the waves below from wave 9. Each package is one pull request for one Sonnet builder. The rules that keep builders out of each other's files are [ARCH-6](decisions.md); the layer order is ARCH-7. Packages are ticked off in [progress.md](progress.md) as they merge; this page changes only when the order does.

Written against `origin/main` at `535470a`, with the DOS autosave branch (`feat/autosave-dos`, DOS-47, DOS-48, CORE-69) in flight. Sizes marked **[E]** are estimates for the package to measure; everything else was read in the code.

## Table of contents

- [The order at a glance](#the-order-at-a-glance)
- [What the UI track needs first](#what-the-ui-track-needs-first)
- [Rules for every package](#rules-for-every-package)
- [Budgets](#budgets)
- [Decision ids](#decision-ids)
- [Gates and evidence](#gates-and-evidence)
- [The common brief](#the-common-brief)
- [Architecture packages](#architecture-packages)
- [Editor feature packages](#editor-feature-packages)
- [Found while planning](#found-while-planning)

## The order at a glance

Two lanes edit `crates/quill-app/src`, each with at most one open branch there: the **ARCH lane** (moves, serial) and the **ED lane** (behaviour). A third builder slot, when the cap of three allows, takes the packages marked *side*. Waves are what can be open at the same time.

| Wave | ARCH lane | ED lane | Side | Waits for |
|---|---|---|---|---|
| 1 (now) | WP-ARCH-1 `quill-chrome` | WP-ED-1 Enter flags, then WP-ED-2 paste undo | (the DOS autosave builder) | — |
| 2 | WP-ARCH-2 `quill-picture` | WP-ED-3 End affinity, WP-ED-4 Unicode U1a | — | DOS autosave merged, for WP-ED-3 |
| 3 | WP-ARCH-3 `quill-session` (or WP-ARCH-4 first, if autosave has not merged) | WP-ED-5 rich clipboard in the editor | — | DOS autosave merged, for WP-ARCH-3 |
| 4 | WP-ARCH-4 `quill-input` | WP-ED-6 rich clipboard on Windows | — | — |
| 5 | WP-ARCH-5 `quill-menu` | WP-ED-7 word keys | — | — |
| 6 | WP-ARCH-6 `quill-settings` | WP-ED-8 Page Break, WP-ED-9 Edit menu state | WP-ARCH-9 Win16 strings from resources | — |
| 7 | WP-ARCH-7 `quill-flow` | (WP-ED-8 or WP-ED-9 finishing) | — | — |
| 8 (freeze) | WP-ED-10 Unicode U1b, then WP-ED-11 Unicode U2 | — (nothing else open in `quill-app`, `quill-paint` or the shells' key maps and painters) | Windows-shell-only work: WP-ED-6 if late | every earlier package merged |
| 9 | WP-ARCH-8 `quill-a11y` (after WP-GTK-5) | WP-ED-12 default tab stops, WP-PREF-2 settings by id (was WP-ED-13) | WP-CODES-1 Reveal Codes core (`quill-core` and `quill-cli` only) | the freeze ended |
| 10 | WP-CODES-2 Reveal Codes on the DOS text screen | WP-PREF-5 Tools > Options on DOS (was WP-ED-15) | WP-PREF-3 Win16 profile functions (was WP-ED-14) | WP-CODES-1, WP-ARCH-8 and WP-PREF-2, for WP-CODES-2 |
| 11 | WP-CODES-3 deleting a code | WP-PREF-6 Options on the DOS graphics screen; then WP-PREF-7 and WP-PREF-8 on Windows (were WP-ED-16) | WP-ED-17 Page Layout | — |
| 12 | WP-CODES-4 Reveal Codes on DOS graphics, the pointer on both DOS screens | WP-ED-18 Numbering | WP-ED-19 Header and Footer | — |
| 13 | WP-CODES-7 editing a code, pending codes | — | WP-CODES-5 Reveal Codes on Windows 95 | WP-ED-19, for WP-CODES-7 |
| 14 | WP-CODES-8 dragging codes | — | WP-CODES-6 Reveal Codes on Windows 3.x | WP-CODES-5 and WP-ARCH-9, for WP-CODES-6 |

From wave 10 the ARCH lane's slot in `crates/quill-app/src` carries the Reveal Codes packages, one at a time, since the ARCH packages end with WP-ARCH-8; the GTK and WinUI panes (WP-CODES-9 and WP-CODES-10) run in the UI track's lane once WP-GTK-5 and WP-WINUI-9 have merged. WP-CODES-5 and WP-CODES-9 touch no shared crate and can be pulled forward to any wave after WP-CODES-3.

Why this order and not the list's: `quill-session` owns the idle cadence the DOS autosave builder is writing into `dos/quill-dos/src/main.rs` and `idle.rs` right now, so it waits for that merge; `quill-chrome` touches no file the autosave branch changes except one `use` line, so it goes first. `quill-a11y` is built from the snapshot GTK's WP-GTK-5 makes (LNX-24), once the text is UTF-8 (WP-ED-11), so it is written once. Unicode U1b and U2 run in a freeze after every move, because they touch about 70 call sites across `quill-app`, `quill-paint` and the shells; a move landing under them would mean rebasing the widest change in the track.

## What the UI track needs first

The GTK4 and WinUI 3 builders run in their own lane, planned in [ui-track.md](ui-track.md) (LNX-21 to LNX-24, WUI-1, WUI-2). That plan waits for no split: each of its packages works against `main` as it is and names the split it will later move onto, and moving client code onto a split is the split's own pull request. Cross-checked against it, what passes between the tracks is:

| From this track | To the UI track | Kind |
|---|---|---|
| WP-ED-5, the clipboard request (`App::take_clip_request`, `copy_to`, `paste_from`, the 2,048-byte buffer gone) | WP-GTK-2 (G5) and WP-WINUI-7 (W7) | **Hard** (LNX-23): neither starts before WP-ED-5 merges. This answers the UI plan's open question of who builds the bridge |
| The shared parts of WP-ED-7, WP-ED-8, WP-ED-15, WP-ED-17, WP-ED-18, WP-ED-19 (word keys, Page Break, Options, Page Layout, Numbering, Header and Footer) | WP-GTK-7, item by item, and the matching WinUI items | **Hard, per item:** each UI slice paragraph below is the hand-over |
| WP-ARCH-1 `quill-chrome` and WP-ARCH-3 `quill-session` | WP-WINUI-2 and WP-WINUI-3 keep a local view model shaped like GTK's `view.rs` until these land | Soft: the earlier they land, the less local code moves |
| WP-ARCH-4 `quill-input` | WP-WINUI-4 uses `quill_app::Key` and `accel::from_vk` today and prefers `quill-input` | Soft: the shim keeps the old paths working |
| WP-ARCH-5 `quill-menu`, WP-ARCH-7 `quill-flow`, WP-ARCH-6 `quill-settings` | WP-GTK-1 and WP-WINUI-5 use `quill_app::menu_items`, `quill_app::session` and `App::menu_mark`; WP-WINUI-2 uses `each_setting` and `apply_setting` | Soft: the menu and settings moves keep re-exports; the flow and session moves change those clients' `use` lines in their own pull requests |
| WP-ARCH-2 `quill-picture` | Nothing in the UI plan | None |
| WP-ARCH-8 `quill-a11y` | WP-GTK-5 builds a local snapshot of its shape in `linux/quill-gtk/src/a11y.rs` (LNX-24); WP-WINUI-9 is the second user | **This track's dependency on the UI track:** WP-ARCH-8 is built from that snapshot once WP-GTK-5 merges, and lands before or with WP-WINUI-9 |

Two consequences for the packages here:

1. **Kit and editor-upper moves edit the clients' `use` lines.** WP-ARCH-1, WP-ARCH-2, WP-ARCH-3 and WP-ARCH-7 change `use` lines and manifests in `linux/quill-gtk` and, once WP-WINUI-2 has made it, `winui/quill-winui`, and nothing else there. A UI branch open at the same time meets only those lines; whichever merges second rebases. A UI branch is never asked to wait for one.
2. **The UI slices are the UI track's to number.** A slice takes the UI track's next free LNX or WUI id; LNX-60 and WUI-60 upward stay reserved here only for a slice this track ends up building itself. Nothing here clashes with LNX-21 to LNX-24, WUI-1 or WUI-2.

## Rules for every package

These are ARCH-6 in full.

1. **Files are owned.** Each package lists the files it owns. Two open branches never own the same file, except: `crates/quill-app/src/lib.rs` (owned by the items each package names), `SPEC.md`, `docs/decisions.md` and `docs/progress.md` (append rows; resolve on rebase), the lockfiles (regenerate), `crates/quill-dialogs/src/generated.rs` (regenerate with `emit-resources --dialogs`), `win16/examples/quill/segments.json` (only the lines for the package's crate) and `scripts/layer-allow.txt` (delete lines only).
2. **A move changes no behaviour; a behaviour change moves no code.** A move's `git diff tests/golden` is empty and its sizes stay within the move limits under [Budgets](#budgets).
3. **Shims where the layers allow them.** A move into a crate below `quill-app` (`quill-input`, `quill-menu`, `quill-settings`) leaves `pub use` re-exports in `quill-app`, so no shell changes. A move into a crate above it (`quill-chrome`, `quill-picture`, `quill-flow`, `quill-session`) cannot, because `quill-app` cannot depend upward: it changes the shells' `use` lines and manifests in the same pull request and nothing else in a shell file.
4. **The freeze.** While WP-ED-10 or WP-ED-11 is open, no other branch is open in `crates/quill-app/src`, `crates/quill-paint/src`, `crates/quill-clip`, `crates/quill-spell`, `crates/quill-grid`, or a shell's key map or page painter.
5. **Branch late, rebase, never merge main in.** A package branches from `origin/main` once the packages it depends on have merged, and is brought up to date by rebase and `git push --force-with-lease`.
6. **New lists edit themselves once.** WP-ARCH-1 writes the final `ORDER` of `scripts/check-layers.py` for every crate this track adds, so no later package edits it.
7. **UI slices are handed over, not built here.** A `WP-ED-*` package is done when its shared part and the DOS, Windows 3.x and Windows 95 slices are in; its GTK and WinUI slices are the UI track's (WP-GTK-7 and the WinUI packages of [ui-track.md](ui-track.md)), and progress records them as "with the UI track" until they land.

## Budgets

Measured at the branch point and again before the pull request; the pull request states both and the difference.

| Limit | Now | Rule |
|---|---|---|
| `QUILLDOS.EXE` | 418,304 of 432,128 bytes on `main`; about 436,000 of 442,368 once DOS autosave merges (DOS-48) | A move: Δ ≤ +1 KB. A feature: the brief's estimate; past the budget, take the savings the brief names, then raise the budget by the measured cost rounded up to 4 KB in a DOS row of the package's own, with the DOS floppy's free bytes from `installer/tools/verify-disks.sh`. Stop and ask once the budget would pass 480 KB (491,520) or the floppy would keep less than 128 KB free (DOS-70) |
| Win16 data segment | about 2 KB free (measure with `win16/build.sh`) | A move: Δ ≤ 0. Relief comes first: WP-ED-5 (−2,048, the clipboard to the store), WP-ARCH-5 (about −260, the command table), WP-ARCH-9 (about −2,100, hints and tips to string resources). A package that spends data names its bytes, and nothing merges below 1,024 free (W16-60) |
| Win16 stack | `check_stack_log.py` margin | ≥ 2,048 bytes to spare in every run |
| Win16 code segments | `check_segments.py` against `segments.json` | A move that renames a module adds its crate's rule to `segments.json` where the old module had one |
| DOS, Windows 3.x and Windows 95 floppies | DOS 437,248 bytes free before autosave, about 427,000 after (DOS-48) | Each feature package reports `installer/tools/verify-disks.sh` for the disks it grows |
| `QUILL32.EXE` imports | `win9x/tools/imports.allow` | A new import is named in the brief and added to the list in the same change |

The DOS ledger the track expects, all **[E]**: moves 0; WP-ED-1 to WP-ED-4 about +1 KB; WP-ED-5 +2 to +4; WP-ED-7 +1 to +1.5; WP-ED-8 +0.5; WP-ED-9 +1; WP-ED-10 +0 to +1.5; WP-ED-11 +5 to +8; WP-ED-12 +0.3; WP-ED-13 +2 to +4; WP-ED-15 +6 to +10; WP-ED-17 to WP-ED-19 +3 to +5 each. That is +31 to +48 KB, about 467 to 484 KB, so the stop at 480 KB is likely to be reached near the dialogs, and the DOS floppy (which packs the executable to about 58 %) still has room. Reveal Codes adds +11 to +16.5 KB more after WP-ED-19 ([plans/reveal-codes.md](plans/reveal-codes.md#budgets)), so it is the package set most likely to meet that stop. [plans/modules.md](plans/modules.md) moves the DOS file dialog and the modal dialogs into `QUILLDLG.DMD`, which would leave the executable at about 438 to 458 KB with Reveal Codes in.

## Decision ids

Ranges reserved for this track, above every id on `main`, in the autosave branch and in the other plans. A package takes the next free id inside its area's range; unused ids stay spare.

| Area | Range | Notes |
|---|---|---|
| ARCH | ARCH-6 to ARCH-10 | This plan (ARCH-6 to ARCH-9 are taken by it) |
| ARCH | ARCH-11 to ARCH-29 | The ARCH packages |
| CORE | CORE-41 to CORE-47, CORE-55 | The editing review's own ids, kept so the review still reads right |
| CORE | CORE-100 to CORE-139 | Unicode U1 and U2 (the Unicode plan's CORE-62 to CORE-68 were taken by other work), clipboard, menu state, preferences, dialogs |
| DOS | DOS-70 to DOS-89 | DOS-70 is this plan's budget rule |
| W16 | W16-60 to W16-79 | W16-60 is this plan's data rule |
| W9X | W9X-60 to W9X-79 | |
| LNX | LNX-60 to LNX-69 | Only for a UI slice this track builds itself; the UI track numbers its own from its next free id (LNX-25 on) |
| WUI | WUI-60 to WUI-69 | The same, for WinUI (the UI track's own start at WUI-3) |
| CORE, ARCH, DOS, W16, W9X, LNX, WUI, INS | CORE-150 to CORE-169, ARCH-30 to ARCH-34, DOS-80 to DOS-89 (carved out of the range above, which keeps DOS-70 to DOS-79), W16-90 to W16-99, W9X-110 to W9X-119, LNX-60 to LNX-64, WUI-60 to WUI-64, INS-8 and INS-9 | The preferences series, [plans/preferences.md](plans/preferences.md) (ARCH-30). The ids WP-ED-13 to WP-ED-16 named stay spare |
| LNX | LNX-80 to LNX-89 | GTK printing, help and dark paper, [plans/gtk-print-help.md](plans/gtk-print-help.md) |
| CORE, ARCH, DOS, W16, W9X, LNX, WUI | CORE-180 to CORE-199, ARCH-35, DOS-90 to DOS-99, W16-100 to W16-104, W9X-100 to W9X-104, LNX-70 to LNX-74, WUI-70 to WUI-74 | Reveal Codes, [plans/reveal-codes.md](plans/reveal-codes.md) |
| CORE, DOS, W16, W9X, INS, ARCH | CORE-90 to CORE-99, DOS-50 to DOS-59, W16-40 to W16-49, W9X-40 and W9X-41, INS-20 to INS-22, ARCH-40 to ARCH-42 | Optional modules, [plans/modules.md](plans/modules.md) (WP-MOD-1 to WP-MOD-8). DOS-52 revises DOS-70's budget rule once a module exists |
| CORE, DOS, W16, W9X, LNX, WUI | CORE-200 to CORE-209, DOS-100 to DOS-109, W16-110 to W16-114, W9X-120 to W9X-124, LNX-90 to LNX-94, WUI-80 to WUI-84 | The status bar, [plans/status-bar.md](plans/status-bar.md) |

## Gates and evidence

The gates, named once and cited by each package:

- **G-HOST:** `make validate` (format, clippy, nextest, headers, floats, layers, magic, wasm, bare).
- **G-DOS:** `make dos` (every DOSBox scenario equal to the host's) and `bash dos/tools/size-report.sh`.
- **G-W16:** `bash win16/build.sh` with `check_segments.py`, the data segment figure, `check_stack_log.py`, and `bash win16/tools/play-w31.sh` with the `m1-edit` scenario on Windows 3.11 in DOSBox-X, plus the e2e scripts the brief names.
- **G-W32:** `bash win9x/build.sh`, `bash win9x/tools/check-imports.sh`, `bash win9x/tools/scan-pentium.sh`, `bash win9x/tools/wine-e2e.sh`, plus the Wine scripts the brief names.
- **G-GTK:** `make gtk` under the builder's own Xvfb, plus `linux/quill-gtk/tools/chrome-shots.sh` where the brief asks for shots.
- **G-DISK:** `make disks` (`build-disks.sh --stage` then `verify-disks.sh`) when the package grows an executable.
- **G-MD:** `docker run --rm -v "$PWD:/workdir" davidanson/markdownlint-cli2:v0.17.2 "**/*.md"`.

**Evidence** goes to `~/Documents/Liminal HQ/Quill/Editor features/<WP-ID>/`, one PNG per platform and state, named `<platform>-<state>.png` with `before-` and `after-` prefixes for a move. Platforms: `dos-text-25`, `dos-text-50`, `dos-gfx-12h`, `dos-gfx-vbe`, `win16-311` (Windows for Workgroups 3.11 in DOSBox-X), `win32-wine`, `win95` (DOSBox-X, only where the brief asks), `gtk-xvfb`, and `winui` once the WinUI client exists. Shots come from the repository's own scripts (`dos/tools/shot.sh`, `dos/tools/shot-text.sh`, `win16/tools/run-w31.sh`, `win9x/tools/wine-layout.sh`, `linux/quill-gtk/tools/chrome-shots.sh`), never from the host desktop. A move proves "unchanged" by comparing the pairs (`win16/tools/compare_frames.py` or a pixel diff) and writes the result beside them in `compare.txt`.

## The common brief

Every brief below starts with this block. Paste it, then the package's own brief.

```text
You are a Sonnet builder on Quill (liminal-hq-labs/quill). Read AGENTS.md, SPEC.md, docs/decisions.md and
docs/work-packages.md (the rules, budgets, gates and your package's section) before you start.

Where to work: never touch ~/source/experiments/quill16 except `git fetch` and `git worktree add/remove`.
Make your worktree with
  git -C ~/source/experiments/quill16 fetch origin
  git -C ~/source/experiments/quill16 worktree add -b <branch> ~/source/worktrees/quill/<topic> origin/main
Build output stays where cargo puts it; if another agent holds the same cargo lock, set CARGO_TARGET_DIR under
~/.cache/quill-target/<topic> and say so in the PR. /tmp is a ramdisk: keep large files out of it.

Rules: Canadian English; no hard-wrapped markdown; tabs except YAML; SPDX header on every new source, script,
TOML and YAML file. Stage explicit paths, never `git add -A` or `git commit -a`; no wildcard deletes. Commit
with a Conventional Commit subject and a bold-label markdown body hard-wrapped at about 72 columns, written to
a file and committed with `git commit -F`, ending with a `Co-Authored-By:` trailer naming your own model.
Update SPEC.md, docs/decisions.md (ids from your package's range) and docs/progress.md in the same change as
the behaviour. Push only your branch; open one PR with `gh pr create`: a capitalized imperative title of about
70 characters, `## Summary` with bold-lead bullets, `## Test plan` with `- [x]`/`- [ ]` items naming the
commands run, ending with an unchecked "Not verified:" line. Do not merge.

Emulators and displays: your own Xvfb display and your own emulator processes only; kill only PIDs you
started, never by name. One heavy job (DOSBox, DOSBox-X, Wine, a full build) at a time. Never print or write a
VM password.

If a decision is needed and nobody answers, take the default your brief gives, make it easy to swap, record it
as an Assumed decision with "OWNER DECISION:" in its text, and keep going. Report: the PR URL, the gates run
and their results, the budget figures before and after, the evidence folder, and what was not verified.
```

## Architecture packages

### WP-ARCH-1 — `quill-chrome`: the shared window geometry

- **Finish line:** a kit crate `crates/quill-chrome` holds the free geometry of `quill_app::view` and `quill_app::zoom_field` (toolbar description and buttons, ruler state and hits, `PageGeom`, `Drag`, `COLOURS`, `SIZES`, `family_of`, the zoom arithmetic, `ZoomLayout` and the zoom field's text and levels), every path a shell used as `quill_app::view::X` or `quill_app::zoom_field::X` resolves as `quill_chrome::view::X` or `quill_chrome::zoom_field::X`, the `impl App` blocks and whatever they need stay in `quill-app` as `quill_app::scroll`, and nothing a user sees changes.
- **Owns:** `crates/quill-chrome/**` (new); `crates/quill-app/src/view.rs` and `zoom_field.rs` (split, then deleted); `crates/quill-app/src/scroll.rs` (new); in `crates/quill-app/src/lib.rs` the `mod` lines for these modules and the two uses of `view::PAGE_TOP` (and nothing else); the `use crate::view` line of `crates/quill-app/src/mouse.rs`; `use` lines and manifests in `crates/quill-paint`, `crates/quill-host/tests`, `win16/examples/quill`, `win9x/quill-win9x/src/ui.rs`, `linux/quill-gtk`; the root `Cargo.toml` members; `Cargo.lock`, `win16/Cargo.lock`, `win9x/Cargo.lock`, `dos/Cargo.lock`, `linux/quill-gtk/Cargo.lock`; `ORDER` in `scripts/check-layers.py`; the crate table and layer text of `docs/architecture.md`.
- **Must not touch:** any function body outside the moved modules; `idle.rs`, `recovery.rs`, `paint.rs`, `overlay.rs`, `widgets.rs` (the autosave branch's); `picture_geom.rs` (it stays in the editor: `mouse.rs` drives it); any shell file beyond its `use` lines.
- **Depends on:** nothing. Parallel with WP-ED-1, WP-ED-2 and the DOS autosave branch (the only shared file is `crates/quill-paint/tests/shell.rs`, where both change `use` lines).
- **Budget:** move limits: DOS Δ ≤ +1 KB, Win16 data Δ 0, no new import.
- **Tests:** G-HOST (with `crates/quill-host/tests/view.rs`, `scroll.rs`, the `quill-paint` zoom, chrome and mouse tests and the zoom field's moved unit tests), G-DOS, G-W16 with `e2e-zoom-w31.sh` and `e2e-zoombox-w31.sh`, G-W32 with `wine-zoom.sh` and `wine-layout.sh`, G-GTK. `python3 -I scripts/check-layers.py` lists no new violation.
- **Evidence:** before and after, equal: `dos-gfx-12h` and `dos-gfx-vbe` at 150 % with the status control, `dos-text-25` (smoke), `win16-311` at 150 % with the ruler, `win32-wine` with the toolbar, ruler and zoom box, `gtk-xvfb` from `chrome-shots.sh`.
- **Decisions:** ARCH-11.

Brief:

```text
Package WP-ARCH-1: move the shared window geometry into a new kit crate, quill-chrome. A pure move.

Branch feat/quill-chrome, worktree ~/source/worktrees/quill/quill-chrome. If feat/autosave-dos has merged when
you start, branch after it; if it merges while you work, rebase (only `use` lines in
crates/quill-paint/tests/shell.rs should meet).

1. Read crates/quill-app/src/view.rs (916 lines) and zoom_field.rs (280 lines). Each has an `impl<M> App<M>`
   block (view.rs from about line 615: set_view_height, hscroll_pos, scroll_to, vscroll_by, line_at_y, click,
   drag_select_line and the rest; zoom_field.rs: zoom_field_text, zoom_field_current) that uses App's private
   fields (vsub, top, view_h). An inherent impl cannot leave its crate, so those blocks, and every constant,
   type or function they use (PAGE_TOP, SCROLL_LINE, PageGeom if they use it, and so on), move to a new
   crates/quill-app/src/scroll.rs as `pub mod scroll`. Everything else moves to quill-chrome.
2. Create crates/quill-chrome: Cargo.toml with the workspace package fields and
     [package.metadata.quill]
     layer = "kit"
     no_std = true
     alloc = false
     float = false
   (check view.rs and zoom_field.rs hold no f32 or f64; if they do, stop and report), depending on quill-core
   and quill-app only. src/lib.rs: SPDX header, `#![no_std]`, the same clippy deny block quill-app uses (unwrap,
   expect, panic, indexing_slicing), `pub mod view; pub mod zoom_field;`. In view.rs re-export what stayed in
   quill_app::scroll (`pub use quill_app::scroll::{PAGE_TOP, SCROLL_LINE, ...};`) so that EVERY public path
   quill_app::view::X had now exists as quill_chrome::view::X, and the same for zoom_field. Move the
   modules' #[cfg(test)] tests with the code they test. If PageGeom::of reads `app.vsub`, give App a public
   read-only accessor in scroll.rs rather than widening the field.
3. In quill-app: delete view.rs and zoom_field.rs, add `pub mod scroll;`, point the two `view::PAGE_TOP` uses
   in lib.rs and `use crate::view` in mouse.rs at `scroll`. Change nothing else in lib.rs.
4. Shells and tests: replace quill_app::view and quill_app::zoom_field with quill_chrome::view and
   quill_chrome::zoom_field in use lines only, and add the quill-chrome path dependency, in crates/quill-paint
   (src and tests), crates/quill-host (tests: add it as a dev-dependency if only tests use it),
   win16/Cargo.toml and win16/examples/quill/{bench,chrome,dialogs,gdi,main,tips}.rs,
   win9x/quill-win9x/Cargo.toml and src/ui.rs, linux/quill-gtk/Cargo.toml and
   src/{dialogs,doc,format_bar,ruler,status,toolbar,view,window}.rs and tests/{chrome,editing}.rs.
   Run `grep -rn "quill_app::view\|quill_app::zoom_field"` across the repository until it is empty. Add
   crates/quill-chrome to the root members and regenerate the five lockfiles (Cargo.lock, win16/, win9x/, dos/,
   linux/quill-gtk/) with a build in each workspace; commit them.
5. scripts/check-layers.py: set ORDER to
     "base": unchanged,
     "editor": ["quill-core", "quill-formats", "quill-clip", "quill-app", "quill-flow", "quill-session"],
     "kit": ["quill-dialogs", "quill-strike", "quill-picture", "quill-chrome", "quill-a11y", "quill-paint",
             "quill-fonts", "quill-grid", "quill-raster", "quill-tui"],
   (chrome and picture before paint, because quill-paint uses them; session at the editor's top, ARCH-7 and
   ARCH-8). Nothing new goes into scripts/layer-allow.txt.
6. Docs: add quill-chrome to the crate table of docs/architecture.md, update the sentence about the order
   inside a layer, add decision ARCH-11 to docs/decisions.md ("quill-chrome holds the window geometry the
   graphical shells share; the editor's scroll state and its methods stay in quill_app::scroll", Status
   Assumed, alternatives: moving the impl blocks as free functions taking &mut App, which would widen App's
   fields; leaving view in quill-app, which keeps every graphical shell's geometry inside the editor), and
   tick WP-ARCH-1 in docs/progress.md. SPEC.md does not change (no behaviour).
7. Gates: G-HOST; G-DOS (size-report before and after: Δ ≤ +1 KB); G-W16 plus win16/tools/e2e-zoom-w31.sh and
   e2e-zoombox-w31.sh (data segment Δ 0, segments pass, stack margin ≥ 2,048); G-W32 plus
   win9x/tools/wine-zoom.sh and wine-layout.sh; G-GTK; G-MD; `git diff origin/main -- tests/golden` empty.
8. Evidence in ~/Documents/Liminal HQ/Quill/Editor features/WP-ARCH-1/: before- and after- shots of
   dos-gfx-12h and dos-gfx-vbe at 150 % (DEMO.QLD), dos-text-25, win16-311 at 150 % with the ruler,
   win32-wine with the toolbar, ruler and zoom box, gtk-xvfb from chrome-shots.sh; a pixel compare of each
   pair in compare.txt (all equal).
9. PR title: "Move the shared window geometry into quill-chrome". Labels: refactor-style changes take
   `chore` plus scope labels.

Finish line: quill_app::view and quill_app::zoom_field no longer exist, every shell builds against
quill_chrome, every gate above passes, the goldens are untouched and every pair of shots is equal.
```

### WP-ARCH-2 — `quill-picture`: a picture as drawn, and its cache

- **Finish line:** `crates/quill-picture` (kit) holds `quill_app::picture` (`PictureSource`, `fingerprint`, `render`, `WINDOW`) and `Cache`, the fingerprint-keyed display cache generalized from Win9x's `PicCache` with its memory lent by the caller (no `alloc`); Win9x's `PicCache` keeps its GDI handles and uses `Cache` for the keying and eviction; no pixel changes.
- **Owns:** `crates/quill-picture/**`; `crates/quill-app/src/picture.rs` (deleted) and its `mod` line; the `use` lines of `crates/quill-paint/src/{page,shell}.rs` and its tests, `win16/examples/quill/picture.rs`, `dos/quill-dos/src/main.rs`, `linux/quill-gtk/src/doc.rs`, `crates/quill-fonts/tests/pixel.rs`; `PicCache` in `win9x/quill-win9x/src/gdi.rs`; manifests and lockfiles; the crate row in `docs/architecture.md`.
- **Must not touch:** `quill-image`'s API (the palette argument and caller-owned working memory are later); `quill-paint`'s decode on every paint (reading the cache there is a behaviour and speed change for its own package); `picture_geom.rs`.
- **Depends on:** WP-ARCH-1.
- **Budget:** move limits.
- **Tests:** G-HOST (`pictures.qld` goldens, `quill-paint` gfx and zoom tests), G-DOS, G-W16 (a pictures document opened and scrolled on 3.11), G-W32 with `wine-picture.sh` and `wine-pic8.sh`, G-GTK.
- **Evidence:** before and after, equal: the pictures test document on `dos-gfx-12h`, `dos-gfx-vbe`, `win16-311`, `win32-wine`, `gtk-xvfb`.
- **Decisions:** ARCH-12.

```text
Package WP-ARCH-2: move quill_app::picture into a new kit crate, quill-picture, with the display cache
generalized from Win9x's PicCache. A pure move. Branch feat/quill-picture after WP-ARCH-1 merges.
Metadata: layer "kit", no_std, no alloc, no float. The cache keeps the key (fingerprint, size, place), the
replacement rule and the hit test; the caller lends the slots (a trait), so Win9x keeps its HBITMAPs and a
later GTK or WinUI cache keeps its textures. Shells change use lines only, except PicCache in
win9x/quill-win9x/src/gdi.rs, which becomes a thin wrapper over quill_picture::Cache with the same behaviour.
Do not make quill-paint read the cache, and do not change quill-image. Gates: G-HOST, G-DOS (Δ ≤ +1 KB),
G-W16 (data Δ 0; open and scroll tests/docs pictures on 3.11), G-W32 with wine-picture.sh and wine-pic8.sh,
G-GTK, G-MD, goldens untouched. Evidence: before/after pairs of the pictures document on dos-gfx-12h,
dos-gfx-vbe, win16-311, win32-wine and gtk-xvfb in Editor features/WP-ARCH-2, compare.txt all equal.
Decision ARCH-12. PR: "Move pictures and their display cache into quill-picture".
```

### WP-ARCH-3 — `quill-session`: the idle cadence and the end of a session

- **Finish line:** `crates/quill-session` (editor, top, ARCH-8) holds `Pump`, the state machine every shell otherwise copies to decide when to call `App::idle` (fed the time, whether input waits and whether a dialog, drag or composition is open; answering "call idle with these quiet seconds and this budget" or "wait this long"), and `end`, moved from `quill_app::session` (`end_session_allowed`, `ANSWER_*`). The DOS screens' idle loop (DOS-47) runs on `Pump` with the same timings, so every DOS run is byte for byte as before. No request queue yet: that is a second package once `quill-flow` exists.
- **Owns:** `crates/quill-session/**`; `crates/quill-app/src/session.rs` (deleted) and its `mod` line; the idle loop of `dos/quill-dos/src/main.rs`; the `use` lines of `win16/examples/quill/dialogs.rs`, `win9x/quill-win9x/src/main.rs`, `linux/quill-gtk/src/window.rs`; `crates/quill-host`'s `@sched` driver if it reproduces the cadence; manifests and lockfiles.
- **Must not touch:** `idle.rs`'s job order (CORE-17); `recovery.rs`; the Win16 and Win32 timers (their autosave packages adopt `Pump`); GTK's and WinUI's executors (the UI track's J14).
- **Depends on:** WP-ARCH-2 and the DOS autosave branch merged.
- **Budget:** DOS ≤ +0.5 KB; Win16 data Δ 0.
- **Tests:** `Pump` unit tests and a seeded `quill_job::Sim` property (the same trace of input and time gives the same calls); G-HOST; G-DOS (the recovery scenarios `g17-gfx-recover` and `m16-big` unchanged); G-W16 with `e2e-endsession-w31.sh`; G-W32 with `wine-endsession.sh`; G-GTK.
- **Evidence:** the recovery question after a forced exit on `dos-text-25` and `dos-gfx-vbe`, and the end-of-session question on `win16-311` and `win32-wine`, before and after, equal.
- **Decisions:** ARCH-13.

```text
Package WP-ARCH-3: a new editor crate, quill-session (top of the editor layer, ARCH-8), with Pump and the
end-of-session verdict. Branch feat/quill-session after WP-ARCH-2 and feat/autosave-dos have merged.
Read crates/quill-app/src/idle.rs, session.rs and the DOS idle loop (dos/quill-dos/src/main.rs, DOS-47: two
seconds of quiet, a poll every 9 BIOS ticks, the budget per call). Write Pump as a pure no_std, no-alloc state
machine over milliseconds (the DOS shell converts ticks) with no clock of its own; move session.rs to
quill_session::end; switch the DOS loop to Pump with exactly the same calls (prove it: dos/check.sh equal,
and a host test that replays the DOS loop's old schedule and Pump's side by side). Shells change use lines
for `end`. Do not touch idle.rs's order, recovery.rs or the Windows timers. Gates: G-HOST, G-DOS
(Δ ≤ +0.5 KB), G-W16 with e2e-endsession-w31.sh, G-W32 with wine-endsession.sh, G-GTK, G-MD. Evidence in
Editor features/WP-ARCH-3 (recovery and end-session questions, before/after equal). Decision ARCH-13. PR:
"Give every shell one idle cadence in quill-session".
```

### WP-ARCH-4 — `quill-input`: keys

- **Finish line:** `Key`, `Dir`, `accel`, `bios` and `script` live in `crates/quill-input` (base); `quill-app` re-exports them (`pub use quill_input::{Key, Dir, accel, bios, script};`), so no shell changes; `quill-dlg`'s `DlgKey` takes `From<Key>`; one key-map table test (`crates/quill-input/tests/keymap.rs`, the review's R72) checks `accel::from_vk` and `bios::decode` against a single table.
- **Owns:** `crates/quill-input/**`; `crates/quill-app/src/{accel,bios,script}.rs` (deleted); in `lib.rs` the `Key` and `Dir` enums and their `mod` and `pub use` lines; `crates/quill-dlg/src/key.rs`; manifests and lockfiles.
- **Must not touch:** `Key::Char(u8)` (WP-ED-10 makes it `char`); new keys (WP-ED-7); the shells' `map_vk`.
- **Depends on:** WP-ARCH-2. Independent of WP-ARCH-3, so it goes first if the autosave branch has not merged.
- **Budget:** move limits.
- **Tests:** G-HOST, G-DOS, G-W16 (`m1-edit`), G-W32 with `wine-editing.sh`, G-GTK.
- **Evidence:** a typed paragraph on `dos-text-25` and `win16-311`, before and after, equal.
- **Decisions:** ARCH-14.

```text
Package WP-ARCH-4: move the key vocabulary into a new base crate, quill-input, with re-exports in quill-app
so no shell changes. Branch feat/quill-input after WP-ARCH-2. Metadata: layer "base", no_std, no alloc, no
float. In lib.rs touch only the Key and Dir enums and the mod/use lines. quill-dlg: DlgKey gets From<Key>
(replace it only if every use maps one to one). Add crates/quill-input/tests/keymap.rs: one table of (key,
modifiers, expected Key) checked against accel::from_vk and bios::decode. Keep Key::Char(u8). Gates:
G-HOST, G-DOS, G-W16 (m1-edit), G-W32 with wine-editing.sh, G-GTK, G-MD; goldens untouched. Evidence:
dos-text-25 and win16-311 editing frames before/after in Editor features/WP-ARCH-4. Decision ARCH-14. PR:
"Move keys and key decoding into quill-input".
```

### WP-ARCH-5 — `quill-menu`: commands and the menu tree

- **Finish line:** `crates/quill-menu` (base) holds `Command`, the undo `label` constants, the menu tree (`Item`, `MENU_COUNT`, titles, `mnemonic`, `menu_for_mnemonic`, `item_for_mnemonic`, `Slot`, `slot`, `menu_items`, `menu_items_for`), the tips and hints (`command_tip`, `command_hint`, `combo_tip`, `combo_hint`, `COLOUR_TIP`, `COLOUR_HINT`), and a new `command_id` and `command_for_id` table that `emit-resources`, the Win16 shell (its `CMDS` static goes) and the Win32 shell (its `Vec` goes) all number from; `quill-app` re-exports everything; `menu_box` and `MENU_COLS` stay in `quill-app` (text-screen columns); the generated `resources.json` and `quill.json` are byte-identical.
- **Owns:** `crates/quill-menu/**`; in `lib.rs` the `Command` enum and the `label` module; in `paint.rs` the menu tables (about lines 15 to 270) and the tips and hints (about lines 1130 to 1270); `emit-resources`' command numbering; `CMDS` and `build_menu` in `win16/examples/quill/main.rs`; the menu `Vec` in `win9x/quill-win9x/src/main.rs`; manifests and lockfiles.
- **Must not touch:** `paint.rs`'s text painter (`draw`, `draw_rows`, `status_text`, `line_label`); enabled and checked state (WP-ED-9); new commands.
- **Depends on:** WP-ARCH-4 and the DOS autosave branch merged (`paint.rs`).
- **Budget:** DOS Δ ≤ +1 KB; Win16 data about −260 **[E]**, measured.
- **Tests:** G-HOST (`tests/menus.rs`), G-DOS, G-W16 with `check_resources.py`, G-W32 with `wine-e2e.sh`, G-GTK (the GTK menus come from the re-exported tree).
- **Evidence:** the File and Edit menus open on `dos-text-25`, `dos-gfx-12h`, `win16-311`, `win32-wine`, `gtk-xvfb`, before and after, equal.
- **Decisions:** ARCH-15.

```text
Package WP-ARCH-5: move Command, the menu tree, the undo labels and the tips and hints into a new base crate,
quill-menu, with re-exports in quill-app; add one command id table that emit-resources, Win16 and Win32 all
use. Branch feat/quill-menu after WP-ARCH-4 and feat/autosave-dos merge. In lib.rs touch only the Command
enum and the label module; in paint.rs only the menu tables and the tips/hints sections (menu_box and
MENU_COLS stay). command_id(Command) -> u16 and command_for_id(u16) -> Option<Command> as a const table
starting at today's numbers, so resources.json and quill.json come out byte-identical (prove it with a test).
Win16 drops CMDS (report the data segment before/after), Win32 drops its Vec. Gates: G-HOST, G-DOS, G-W16 with
check_resources.py, G-W32, G-GTK, G-MD; goldens untouched. Evidence: File and Edit menus open on dos-text-25,
dos-gfx-12h, win16-311, win32-wine and gtk-xvfb, before/after, in Editor features/WP-ARCH-5. Decision
ARCH-15. PR: "Move commands and the menu tree into quill-menu".
```

### WP-ARCH-6 — `quill-settings`: INI text, profiles and the recent list

- **Finish line:** `crates/quill-settings` (base, ARCH-9) holds `ini::parse(&[u8], |section, key, value|)`, `ini::Writer` over a byte closure, `trait Profile { get; set(None deletes) }`, `recent` (`RecentList`, `RECENT_MAX`, `shorten_path`) and `keys` (the section and key names); `App::load_settings_from<P: Profile>` and `save_settings_to` carry today's logic, and `load_settings` and `save_settings` stay as wrappers over a new `quill_app::IniFile<F: Files>`; Win9x's `ini_load` and `ini_save` become `impl Profile` over the profile calls; every `QUILL.INI` written is byte-identical.
- **Owns:** `crates/quill-settings/**`; `crates/quill-app/src/recent.rs` (deleted); in `lib.rs` `each_setting`, `apply_setting`, `load_settings`, `save_settings` and their helpers; `crates/quill-app/src/ini_file.rs` (new); the `use` lines of `recovery.rs`; `ini_load` and `ini_save` in `win9x/quill-win9x/src/ui.rs`; manifests and lockfiles.
- **Must not touch:** what a setting means (B1 to B8 of the preferences design are WP-ED-13 and WP-ED-14); Win16's whole-file write; DOS's `Kept`.
- **Depends on:** WP-ARCH-5 and the DOS autosave branch merged (`recovery.rs` keeps its list in `QUILL.INI`).
- **Budget:** DOS Δ ≤ +1 KB; Win16 data Δ ≤ 0.
- **Tests:** property tests at 10,000 cases (write then parse is the identity on the key set; random bytes never panic); `m4-quill.ini` and `m11-recent` goldens unchanged; `tests/recovery_files.rs`; G-HOST, G-DOS, G-W16 (recent list after a restart), G-W32 (`QUILL.INI` keys under Wine).
- **Evidence:** File menu with recent files on `dos-text-25`, `win16-311`, `win32-wine`, before and after, equal; the `QUILL.INI` bytes before and after as `ini-diff.txt` (empty).
- **Decisions:** ARCH-16.

```text
Package WP-ARCH-6: move the INI text, a Profile trait and the recent list into a new base crate,
quill-settings, that knows no Store or Source (it works on byte slices and closures, ARCH-9). Branch
feat/quill-settings after WP-ARCH-5 and feat/autosave-dos merge. App gains load_settings_from/save_settings_to
over a Profile; load_settings/save_settings stay as wrappers over quill_app::IniFile<F: Files> (new file
ini_file.rs). Win9x's ini_load/ini_save become impl Profile. No setting changes meaning: every INI byte, every
golden the same. Gates: G-HOST with the property tests, G-DOS (Δ ≤ +1 KB), G-W16 (data Δ ≤ 0; recent list
survives a restart on 3.11), G-W32, G-MD. Evidence in Editor features/WP-ARCH-6. Decision ARCH-16. PR: "Move
INI handling and the recent list into quill-settings".
```

### WP-ARCH-7 — `quill-flow`: the answering layer

- **Finish line:** `crates/quill-flow` (editor, above `quill-app`) holds `quill_app::flow`; `App` exposes at most ten `#[doc(hidden)] pub` items for it, listed in the decision; past ten, the move stops and `flow` stays a module of free functions in `quill-app` (the review's fallback), recorded as the decision instead; `segments.json` places `quill_flow::*` in the EDIT segment on Win16.
- **Owns:** `crates/quill-flow/**`; `crates/quill-app/src/flow.rs` (deleted), its `mod` line and the visibility of the items it needs; `crates/quill-host/src/flow.rs` and the `use` lines of its tests; the EDIT line of `win16/examples/quill/segments.json`; manifests and lockfiles.
- **Must not touch:** the two `parse_inches` and the copied unsaved-changes wording (deduplicated with behaviour in WP-ED-17 and WP-ED-13); new flows.
- **Depends on:** WP-ARCH-6.
- **Budget:** move limits; EDIT within its segment budget.
- **Tests:** `tests/flows.rs`, `native.rs`, `recovery_files.rs` unchanged; G-HOST, G-DOS, G-W16 with `e2e-saveclose-w31.sh`, G-W32 with `wine-dialogs.sh`, G-GTK.
- **Evidence:** Save As and the unsaved-changes question on `dos-text-25`, `win16-311`, `win32-wine`, `gtk-xvfb`, before and after, equal.
- **Decisions:** ARCH-17.

```text
Package WP-ARCH-7: move quill_app::flow into a new editor crate above quill-app, quill-flow. Branch
feat/quill-flow after WP-ARCH-6. Count the App internals flow.rs uses; expose at most ten as
#[doc(hidden)] pub and list them in decision ARCH-17. If it needs more, do not move it: report, and record
the fallback (flow stays a module of free functions in quill-app). Add "quill_flow::*" to the EDIT segment
of win16/examples/quill/segments.json. No flow changes behaviour. Gates: G-HOST (flows, native,
recovery_files), G-DOS, G-W16 with e2e-saveclose-w31.sh, G-W32 with wine-dialogs.sh, G-GTK, G-MD. Evidence
in Editor features/WP-ARCH-7. PR: "Move the dialog flows into quill-flow".
```

### WP-ARCH-8 — `quill-a11y`: the text a screen reader reads

- **Finish line:** `crates/quill-a11y` (kit, after `quill-chrome`) builds a read-only snapshot through callbacks, with no `alloc`: the visible lines as UTF-8, each character's x offset in device pixels through `quill_chrome::view`'s zoom, the caret and selection in code points, line bounds, role and name, and a revision; host tests over the demo document; no shell adopts it (GTK G7 and WinUI W9 do).
- **Owns:** `crates/quill-a11y/**`, the root members, the crate row in `docs/architecture.md`; if `App` lacks a read-only accessor, one new file `crates/quill-app/src/a11y_read.rs` and its `mod` line.
- **Must not touch:** any shell except GTK's `a11y.rs` and its `use` lines; any other `quill-app` file.
- **Depends on:** WP-ARCH-1, WP-ED-11 (the text is UTF-8, so the snapshot hands it over as stored) and WP-GTK-5 (its `a11y.rs` is the shape this crate takes, LNX-24). Lands before or with WP-WINUI-9, its second user; moving GTK onto it is part of this package (`use` lines and the snapshot type only).
- **Budget:** none (no retro shell links it).
- **Tests:** G-HOST; properties: offsets rise along a line, the caret lies on a listed line, a selection's ends are character boundaries.
- **Evidence:** a text dump of the snapshot for `DEMO.QLD` at 100 % and 150 % in the evidence folder; screen shots come with G7 and W9.
- **Decisions:** ARCH-18.

```text
Package WP-ARCH-8: a new kit crate, quill-a11y, that describes the visible text for accessibility APIs.
Branch feat/quill-a11y after WP-ARCH-1, WP-ED-11 and WP-GTK-5 merge; take the shape of
linux/quill-gtk/src/a11y.rs (LNX-24), switch GTK onto the crate, and land before or with WP-WINUI-9. Callbacks, not collections (no alloc): lines as
UTF-8, per-character x in device pixels via quill_chrome::view's zoom, caret and selection, line bounds, role,
name, revision. Read App only through public methods; if one is missing, add it in a new
crates/quill-app/src/a11y_read.rs and nowhere else. No shell changes. Gates: G-HOST, G-MD. Evidence: snapshot
dumps for DEMO.QLD at 100 and 150 % in Editor features/WP-ARCH-8. Decision ARCH-18. PR: "Describe the visible
text for screen readers in quill-a11y".
```

### WP-ARCH-9 — Win16 hints and tips from string resources (side)

- **Finish line:** the Win16 build turns off `quill-menu`'s `text` feature, `emit-resources` writes the hints and tips as `RT_STRING` entries, and the Win16 shell loads each with `LoadString` into a stack buffer when it is shown; the data segment drops by about 2,100 bytes **[E]**.
- **Owns:** the `text` feature of `crates/quill-menu`; `emit-resources`' Win16 string output; `win16/examples/quill/tips.rs` and the hint call sites in `main.rs`; `win16/examples/quill/resources.json` (generated); `win16/Cargo.toml` features.
- **Must not touch:** the other shells.
- **Depends on:** WP-ARCH-5. Parallel with WP-ARCH-6 and WP-ARCH-7 (it touches no `quill-app` file).
- **Budget:** Win16 data about −2,100 **[E]**; stack margin ≥ 2,048 with a tooltip showing.
- **Tests:** G-W16 with `check_resources.py`, a tooltip and a status hint shown on 3.11.
- **Evidence:** a toolbar tooltip and the status hint for File > Open on `win16-311`, before and after, equal.
- **Decisions:** W16-63.

```text
Package WP-ARCH-9 (Win16 only): load the menu hints and toolbar tips from RT_STRING resources instead of the
data segment. Branch feat/w16-hint-strings after WP-ARCH-5. quill-menu gets a default-on feature `text`;
with it off, command_tip and command_hint return string ids. emit-resources writes the strings. The Win16
shell LoadStrings into a stack buffer. Gates: G-W16 with check_resources.py (report the data segment
before/after and the stack margin with a tooltip up), G-HOST, G-MD. Evidence in Editor features/WP-ARCH-9.
Decision W16-63. PR: "Load the Windows 3.x hints and tooltips from string resources".
```

## Editor feature packages

Each feature is shared core first, then the DOS text and graphics screens, Windows 3.x and Windows 95; the GTK and WinUI slices are handed to the UI track (rule 7). The terminal client (`linux/quill-tty`) follows the DOS text screen wherever it shares the code.

### WP-ED-1 — Enter never carries a page break or a numbering restart (review PR 2)

- **Finish line:** the paragraph Enter creates never has "page break before" or "restart numbering" (R5); one Enter after a manual page break leaves one break.
- **Owns:** the Enter split in `crates/quill-app/src/lib.rs` (`key_edit`'s Enter arm and the split it calls) and `quill-core` if the split is there; `tests/scenarios/m14-pagebreak-enter.keys` and its goldens; new tests appended to `crates/quill-host/tests/editing.rs`.
- **Must not touch:** anything WP-ARCH-1 owns; `form.rs` (WP-ED-2).
- **Depends on:** nothing. Parallel with WP-ARCH-1 and WP-ED-2.
- **Budget:** DOS about +0.1 KB; Win16 data 0.
- **Tests:** units for a split at the end, middle and start; the Enter-then-Backspace property; G-HOST, G-DOS, G-W16 (`m1-edit`), G-W32.
- **Evidence:** a page break, then Enter, then the page count, on `dos-text-25`, `dos-gfx-vbe`, `win16-311`, `win32-wine`, `gtk-xvfb`.
- **Decisions:** CORE-41.
- **UI slice:** none; GTK and WinUI get it through `App`.

```text
Package WP-ED-1: editing review PR 2 (R5) — the paragraph Enter creates never carries page break before or
restart numbering. Branch fix/enter-split-flags now. Read "Quill editing rules review.md" (in
~/Documents/Liminal HQ/Quill) rows R5 and the PR 2 line. Touch only the Enter split. Add
m14-pagebreak-enter.keys with goldens and unit tests (end, middle, start). Gates: G-HOST, G-DOS, G-W16,
G-W32, G-MD. Evidence in Editor features/WP-ED-1 (dos-text-25, dos-gfx-vbe, win16-311, win32-wine,
gtk-xvfb). Decision CORE-41; SPEC Typing section. PR: "Keep page breaks and numbering restarts out of a new
paragraph".
```

### WP-ED-2 — Paste is one undo step; an oversized Copy or Cut is refused (review PR 4)

- **Finish line:** one Undo removes a whole paste, however many paragraphs (R41); a Copy or Cut larger than the clipboard holds is refused with the existing message and changes nothing (R42, CORE-44, until WP-ED-5 raises the limit to the store's).
- **Owns:** the clipboard half of `crates/quill-app/src/form.rs`; undo grouping in `quill-core/src/text.rs` if needed; `tests/scenarios/m14-paste-undo.keys`; paste and cut property tests.
- **Must not touch:** `App.clip`'s size (WP-ED-5); `lib.rs` beyond the paste call.
- **Depends on:** nothing. Parallel with WP-ED-1 and WP-ARCH-1.
- **Budget:** DOS about +0.1 KB; Win16 data 0.
- **Tests:** "paste then undo gives the document back byte for byte"; "a cut within the capacity survives cut then paste, a larger one changes nothing"; G-HOST, G-DOS, G-W16, G-W32.
- **Evidence:** a three-paragraph paste and one Undo on `dos-text-25`, `win16-311`, `win32-wine`, `gtk-xvfb`; the refusal message on `dos-text-25`.
- **Decisions:** CORE-43, CORE-44.

```text
Package WP-ED-2: editing review PR 4 (R41, R42) — paste is one undo group; a Copy or Cut over the clipboard's
capacity is refused and nothing is lost. Branch fix/paste-undo-and-clip-guard now. Touch form.rs's clipboard
commands and, if grouping needs it, quill-core's text.rs. Scenario m14-paste-undo.keys plus the two property
tests. Gates: G-HOST, G-DOS, G-W16, G-W32, G-MD. Evidence in Editor features/WP-ED-2. Decisions CORE-43,
CORE-44; SPEC Editing and Clipboard. PR: "Undo a paste in one step and refuse a cut the clipboard cannot
hold".
```

### WP-ED-3 — End at a soft wrap stays on its line (review PR 3)

- **Finish line:** the caret has an end-of-line affinity at a soft wrap (R16): End puts it after the line's last character on the same line, End again leaves it there, and Home, End, Up and Down and every painter place it through one `caret_xy`.
- **Owns:** the affinity bit and `move_caret` in `lib.rs`; `caret_xy`; the caret call sites in `paint.rs` (text), `crates/quill-paint` (page), `win16/examples/quill/gdi.rs`, `win9x/quill-win9x/src/gdi.rs`; `m14-end-wrapped.keys`.
- **Must not touch:** GTK's `page.rs` (one call site at most: hand it to the UI lead as the UI slice if `caret_xy`'s signature changes).
- **Depends on:** WP-ED-1, WP-ARCH-1 and the DOS autosave branch merged (`paint.rs`).
- **Budget:** DOS about +0.3 KB; Win16 data +1 byte.
- **Tests:** End idempotence and "Home and End never change the caret line's index" properties; goldens that change where End lands on a wrapped line, reviewed frame by frame; G-HOST, G-DOS, G-W16, G-W32, G-GTK.
- **Evidence:** End on a wrapped line, then End again, on `dos-text-25`, `dos-gfx-12h`, `win16-311`, `win32-wine`, `gtk-xvfb`.
- **Decisions:** CORE-42.

```text
Package WP-ED-3: editing review PR 3 (R16) — an end-of-line affinity so End on a wrapped line stays on it.
Branch fix/caret-end-affinity after WP-ED-1, WP-ARCH-1 and feat/autosave-dos merge. If every painter already
asks App for the caret's line, the change is App-only; otherwise each painter's caret call goes through
caret_xy. GTK: if a GTK call must change, leave it and write the one-line UI slice in the PR. Scenario
m14-end-wrapped.keys and the two properties; review every golden that moves. Gates: G-HOST, G-DOS, G-W16,
G-W32, G-GTK, G-MD. Evidence in Editor features/WP-ED-3. Decision CORE-42. PR: "Keep the caret on its line
when End reaches a soft wrap".
```

### WP-ED-4 — Unicode U1a: text positions as `Pos`

- **Finish line:** `quill-core` has `Pos`, a text offset only it can make from a number, `Range<Pos>`, and a checked reading and editing API (`next`, `prev`, `chars`, `insert(&str)`), used inside `quill-core` and `quill-formats`; `read_byte` stays public but hidden, so callers outside move in WP-ED-10; a boundary property test is in place (trivially true while the text is Windows-1252). No behaviour change.
- **Owns:** `crates/quill-core/src/**`, `crates/quill-formats/src/**`.
- **Must not touch:** `quill-app`, `quill-paint`, `quill-clip`, `quill-spell`, the shells.
- **Depends on:** WP-ED-1 to WP-ED-3 merged (they touch the core's edit paths). Parallel with the ARCH lane (no ARCH package touches `quill-core` or `quill-formats`).
- **Budget:** DOS ≤ +0.5 KB; Win16 data 0.
- **Tests:** G-HOST (goldens unchanged), G-DOS, G-W16, G-W32.
- **Evidence:** none visual (no change); the size and bench figures in the PR.
- **Decisions:** CORE-100.

```text
Package WP-ED-4: Unicode plan U1a ("Quill Unicode plan.md", Phase 1) — Pos, Range<Pos> and the checked API in
quill-core and quill-formats. Branch refactor/text-pos after WP-ED-1..3 merge. Keep read_byte public with
#[doc(hidden)] (WP-ED-10 makes it private); no caller outside core and formats changes. Every golden
unchanged. Gates: G-HOST, G-DOS (Δ ≤ +0.5 KB), G-W16 (data Δ 0), G-W32, G-MD. Decision CORE-100 (the plan's
CORE-62, renumbered). PR: "Give text offsets their own type in the core".
```

### WP-ED-5 — The rich clipboard in the editor

- **Finish line:** Copy, Cut and Paste go through `quill-clip`: `App` leaves a request (`take_clip_request`), and a shell with a system clipboard serves it with `copy_to` and `paste_from` over its own `Clipboard`; a shell that does not serve it keeps working, because `App`'s default transport keeps the flavours in chunks of the document's store (the CORE-8 pattern); `App.clip: [u8; 2048]` is gone; a copy writes Rich Text Format and Windows-1252 text (and a DIB for a lone picture), a paste prefers the native or RTF flavour, so a copy inside Quill keeps its formatting on every build; Cut's limit is the store's (CORE-44 follows).
- **Owns:** in `lib.rs` the `clip` field and the Copy, Cut and Paste arms; the clipboard half of `form.rs`; `crates/quill-app/src/clip.rs` (new); `crates/quill-clip` where its API needs it; `tests/scenarios` clipboard scenarios and new rich copy-and-paste flows; `docs/clipboard.md`; SPEC's clipboard section.
- **Must not touch:** the shells (they keep the default transport until WP-ED-6 and GTK G5).
- **Depends on:** WP-ED-2. Parallel with the ARCH lane.
- **Budget:** Win16 data −2,048 plus a few bytes of chunk ids **[E]**; DOS +2 to +4 KB **[E]** (the RTF reader and writer are linked already).
- **Tests:** "copy then paste of a random range keeps the text and every format"; `scripts/check-clip-libreoffice.sh`; the `m2` clipboard goldens unchanged except where formatting now survives, reviewed; G-HOST, G-DOS, G-W16, G-W32, G-GTK.
- **Evidence:** a bold and italic run copied and pasted with its formatting on `dos-text-25`, `dos-gfx-vbe`, `win16-311`, `win32-wine`, `gtk-xvfb`.
- **Decisions:** CORE-101.
- **UI slice:** WP-GTK-2 (G5) serves the request with `GdkClipboard` and WP-WINUI-7 (W7) with `CF_UNICODETEXT` and RTF; both wait for this package (LNX-23).

```text
Package WP-ED-5: App adopts quill-clip (architecture review C7, ARCH-4, docs/clipboard.md). Branch
feat/clip-requests after WP-ED-2. App.clip goes; the default transport is chunks in the document's Store
(as the spelling lists are, CORE-8), so shells that serve nothing still copy and paste, now with formatting.
Request API: take_clip_request() -> Option<ClipRequest>, copy_to(s, &mut impl Clipboard),
paste_from(s, &mut impl Clipboard); key() and command() keep their signatures. Shells unchanged. Report the
Win16 data segment before/after (expect about -2,048) and the DOS size. Gates: G-HOST (round-trip property,
check-clip-libreoffice.sh), G-DOS, G-W16, G-W32, G-GTK, G-MD. Evidence in Editor features/WP-ED-5. Decision
CORE-101; SPEC Clipboard. PR: "Copy and paste with formatting through quill-clip".
```

### WP-ED-6 — The rich clipboard on Windows 3.x and Windows 95

- **Finish line:** Win16 and Win32 serve the clipboard request over the system clipboard: `CF_TEXT`, the registered `Rich Text Format`, and `CF_DIB` for a lone picture (Win16 tiles a DIB over 64 KB with `__AHINCR`); bold text copied from Quill pastes bold into Write (3.11) and WordPad (95), and the reverse.
- **Owns:** `win16/examples/quill/clip.rs` (new) and the clipboard arm of `main.rs`; the Win16 import table additions; `win9x/quill-win9x/src/clip.rs` (new) and the clipboard arm of `main.rs`; `win9x/tools/imports.allow`; a new `win9x/tools/wine-clipboard.sh`.
- **Must not touch:** `quill-app`; the hint call sites of Win16 `main.rs` (WP-ARCH-9).
- **Depends on:** WP-ED-5. Parallel with the ARCH lane and WP-ED-7; a shell-only package, so it may run during the freeze.
- **Budget:** Win16 data about +40 **[E]**; Win16 FILE code +3 to +6 KB **[E]**; Win32 imports `OpenClipboard`, `CloseClipboard`, `EmptyClipboard`, `SetClipboardData`, `GetClipboardData`, `IsClipboardFormatAvailable`, `RegisterClipboardFormatA`.
- **Tests:** G-W16 with a play script that copies into Write; G-W32 with `wine-clipboard.sh` (text and RTF both ways); G-DISK.
- **Evidence:** Write on `win16-311` holding pasted bold; WordPad on `win95` holding pasted bold; Quill on `win32-wine` holding RTF pasted from WordPad.
- **Decisions:** W16-61, W9X-60.

```text
Package WP-ED-6: implement quill_clip::Clipboard on Windows 3.x and Windows 95 and serve App's clip
request. Branch feat/clip-windows after WP-ED-5. New files win16/examples/quill/clip.rs and
win9x/quill-win9x/src/clip.rs; in each main.rs only the clipboard arm. CF_TEXT, registered "Rich Text
Format", CF_DIB for a lone picture (Win16: __AHINCR for >64 KB). Add the imports to win9x/tools/imports.allow
and the Win16 import table. Gates: G-W16 (data segment and FILE segment figures), G-W32 with a new
wine-clipboard.sh, G-DISK, G-HOST, G-MD. Evidence: Write on win16-311 and WordPad on win95 with pasted bold,
Quill on win32-wine with pasted RTF, in Editor features/WP-ED-6. Decisions W16-61, W9X-60. PR: "Copy and paste
with formatting through the Windows clipboard".
```

### WP-ED-7 — Word movement and word deletion keys (review PR 5)

- **Finish line:** one word rule (classes and stops, CORE-45) serves Ctrl+Left and Ctrl+Right, Ctrl+Backspace and Ctrl+Delete, double click, Find's whole word and the spelling scan; `Dir` gains `WordLeft`, `WordRight`, `ParaUp`, `ParaDown`, `DocStart` and `DocEnd`, and `Key` gains `DeleteWord`; Left and Right collapse a selection to its edge and Shift with Ctrl+Home or Ctrl+End extends it (CORE-47); the Win16 and Win32 `map_vk` become one `quill_input::vk::map`; DOS reads scans 0x73 and 0x74 and the extended keys; the terminal's key map follows.
- **Owns:** `crates/quill-input`; `crates/quill-app/src/word.rs` (new); `move_caret` in `lib.rs`; `word_range` in `pointer.rs`; the whole-word test in `find.rs`; the scanner's word call in `spelling.rs`; `dos/quill-dos/src/keys.rs`; `map_vk` in `win16/examples/quill/main.rs` and `win9x/quill-win9x/src/main.rs`; `linux/quill-tty/src/keymap.rs`; `m15-word-keys.keys`.
- **Must not touch:** `linux/quill-gtk` (UI slice).
- **Depends on:** WP-ARCH-4 and WP-ED-3.
- **Budget:** DOS +1 to +1.5 KB; Win16 data 0.
- **Tests:** word properties (Ctrl+Right visits rising stops to the end, Ctrl+Left the same stops back); the key-map table; G-HOST, G-DOS, G-W16, G-W32.
- **Evidence:** the caret after three Ctrl+Right and the text after Ctrl+Backspace on `dos-text-25`, `dos-gfx-12h`, `win16-311`, `win32-wine`.
- **Decisions:** CORE-45, CORE-47, DOS-71, W16-64, W9X-61.
- **UI slice:** GTK's `keys.rs` maps to the shared `Dir` and `Key` and drops LNX-13's GTK-layer word moves; WinUI W4 uses `quill_input::vk::map`.

```text
Package WP-ED-7: editing review PR 5 (R15, R18, R19, R20, R24) — one word rule; word movement and deletion keys
in every retro shell. Branch feat/word-keys after WP-ARCH-4 and WP-ED-3. Win16 and Win32 map_vk become one
quill_input::vk::map with Ctrl and Shift. Do not touch linux/quill-gtk; write its slice in the PR. Gates:
G-HOST (word properties, key-map table), G-DOS, G-W16, G-W32, G-MD. Evidence in Editor features/WP-ED-7.
Decisions CORE-45, CORE-47, DOS-71, W16-64, W9X-61; SPEC Typing and the key table. PR: "Move and delete by
word with one word rule on every build".
```

### WP-ED-8 — Page Break, Shift+Enter and Tab in a list (review PR 6)

- **Finish line:** Insert > Page Break and Ctrl+Enter start a new paragraph with "page break before"; Shift+Enter inserts a line break (0x0B, which layout and RTF already handle); Tab and Shift+Tab at a list item's start change its level; every build's Insert menu shows Page Break with Ctrl+Enter.
- **Owns:** `Command::PageBreak` and the Insert item in `crates/quill-menu`; the Enter and Tab arms of `key_edit` in `lib.rs`; the regenerated `resources.json` and `quill.json`; the Win16 and Win32 accelerator tables; DOS's Ctrl+Enter and Shift+Enter in `keys.rs`; `m14-page-break.keys`, `m15-list-tab.keys`.
- **Must not touch:** `paint.rs`'s menu drawing (WP-ED-9).
- **Depends on:** WP-ARCH-5 and WP-ED-7.
- **Budget:** DOS about +0.5 KB; Win16 data 0 to +20 (menus are resources).
- **Tests:** units; G-HOST, G-DOS, G-W16, G-W32, G-GTK (the item appears from the tree).
- **Evidence:** the Insert menu and a manual page break's dotted line on `dos-text-25`, `dos-gfx-vbe`, `win16-311`, `win32-wine`, `gtk-xvfb`.
- **Decisions:** CORE-46.
- **UI slice:** GTK and WinUI accelerators for Ctrl+Enter and Shift+Enter if their tree mapping does not give them.

```text
Package WP-ED-8: editing review PR 6 (R8, R9, R11, R12) — Insert > Page Break and Ctrl+Enter, Shift+Enter line
break, Tab and Shift+Tab level at a list item's start. Branch feat/page-break-keys after WP-ARCH-5 and
WP-ED-7. Gates: G-HOST, G-DOS, G-W16, G-W32, G-GTK, G-MD. Evidence in Editor features/WP-ED-8. Decision
CORE-46; SPEC menu table and Typing. PR: "Add Page Break, Shift+Enter and list Tab on every build".
```

### WP-ED-9 — Edit menu state: greyed items, Undo labels and ticks

- **Finish line:** `App::command_state(Command) -> quill_menu::State { enabled, checked }` and the Undo and Redo texts (`Undo Typing`, `Redo Paste`, `Can't Undo`) answer every item; the DOS text and graphics menus draw a disabled item in the disabled attribute and the arrows skip it; Win16 and Win32 answer `WM_INITMENUPOPUP` with `EnableMenuItem`, `CheckMenuItem` and `ModifyMenu`; the Cut, Copy, Paste and Undo buttons grey on Win16, Win32 and DOS graphics.
- **Owns:** `crates/quill-app/src/menu_state.rs` (new); `State` in `crates/quill-menu`; the menu drawing in `paint.rs`; the menu drawing and toolbar greying in `crates/quill-paint`; menu items in `overlay.rs` and `widgets.rs`; the `WM_INITMENUPOPUP` arms and toolbar greying in `win16/examples/quill/{main,chrome}.rs` and `win9x/quill-win9x/src/{main,chrome}.rs`.
- **Must not touch:** `quill-input`; `key_edit`.
- **Depends on:** WP-ARCH-5 (and WP-ARCH-1 for the toolbar).
- **Budget:** DOS about +1 KB; Win16 data about +60 **[E]** (the labels exist in `label`).
- **Tests:** units for every command's state; DOS menu goldens change (greyed items), reviewed frame by frame; G-HOST, G-DOS, G-W16, G-W32.
- **Evidence:** the Edit menu with nothing selected and with a selection, after typing (`Undo Typing`), on `dos-text-25`, `dos-gfx-12h`, `win16-311`, `win32-wine`.
- **Decisions:** CORE-102, DOS-72, W16-65, W9X-62.
- **UI slice:** GTK actions' enabled state and the Undo label; WinUI the same on `MenuFlyoutItem`.

```text
Package WP-ED-9: Edit menu state — greyed items, Undo and Redo labels, ticks — on every retro build. Branch
feat/edit-menu-state after WP-ARCH-5. One App::command_state for every command; menus and the toolbar grey
from it. DOS menu goldens will change: regenerate only those, in a commit of their own, and review each frame.
Gates: G-HOST, G-DOS, G-W16 (data figure), G-W32, G-MD. Evidence in Editor features/WP-ED-9. Decisions
CORE-102, DOS-72, W16-65, W9X-62; SPEC menu paragraph. PR: "Grey and label the menu items by the editor's
state".
```

### WP-ED-10 — Unicode U1b: every caller on `Pos` and characters (freeze)

- **Finish line:** as the Unicode plan's U1b: `quill-app`, `quill-paint`, `quill-clip`, `quill-spell` and `quill-grid` read text through `Pos` and `chars`; `Key::Char(char)` in `quill-input` and every shell, converted to Windows-1252 at the edge and refused as today; clippy's `disallowed_methods` forbids `read_byte` outside `quill-core`. No behaviour change.
- **Owns:** the freeze set (rule 4).
- **Depends on:** WP-ARCH-7, WP-ED-4 and every open `quill-app` package merged.
- **Budget:** DOS +0 to +1.5 KB; Win16 code +0 to +1 KB, data 0.
- **Tests:** every golden byte-identical on the host, DOS, Win16, Win32 and GTK; G-HOST, G-DOS, G-W16, G-W32, G-GTK.
- **Evidence:** before and after, equal: an accented French paragraph on `dos-text-25`, `dos-gfx-vbe`, `win16-311`, `win32-wine`, `gtk-xvfb`.
- **Decisions:** CORE-100 (continued).
- **UI slice:** GTK's `keys.rs` and text input hand `char`; the refusal (LNX-14) stays until U5.

```text
Package WP-ED-10: Unicode plan U1b — callers on Pos and chars, Key::Char(char), read_byte forbidden outside
quill-core. FREEZE: tell the orchestrator before you branch; nothing else may be open in quill-app,
quill-paint, quill-clip, quill-spell, quill-grid or a shell's key map or painter until you merge. Branch
refactor/app-pos-chars after WP-ARCH-7 and WP-ED-4. No behaviour change: every golden byte-identical. Gates:
G-HOST, G-DOS, G-W16, G-W32, G-GTK, G-MD. Evidence in Editor features/WP-ED-10. Decision CORE-100. PR: "Read
text by character everywhere outside the core".
```

### WP-ED-11 — Unicode U2: UTF-8 in the buffers (freeze)

- **Finish line:** as the Unicode plan's U2: the text buffers hold UTF-8; readers turn Windows-1252, CP437 and RTF into UTF-8 and writers turn it back; layout decodes with an ASCII fast path; the GDI painters transcode runs; `MAX_TEXT` is 1,048,575 bytes of UTF-8 (the plan's Q2 default); the `.QLD` writer counts code points; input still refuses what Windows-1252 lacks, so every saved file is byte-identical; the 386 and 486 bench gate holds.
- **Owns:** the freeze set.
- **Depends on:** WP-ED-10.
- **Budget:** DOS +5 to +8 KB **[E]** (expect a DOS-70 raise, with savings named first); Win16 code +4 to +7 KB, data 0 to +16 **[E]**.
- **Tests:** every golden and saved file byte-identical; the fuzz target; `win16/tools/bench-w31.sh` and the DOS key-to-paint figures within the gate; G-HOST, G-DOS, G-W16, G-W32, G-GTK, G-DISK.
- **Evidence:** the accented French document on all five platforms, before and after, equal; the bench table.
- **Decisions:** CORE-103 (supersedes D11), CORE-104 (`MAX_TEXT` in bytes).
- **UI slice:** none needed for parity; U3 and U5 (placeholders, input) are not in this track.

```text
Package WP-ED-11: Unicode plan U2 — UTF-8 storage, files byte-identical. FREEZE as WP-ED-10. Branch
feat/text-utf8 after WP-ED-10. Measure the 386/486 bench before and after (win16/tools/bench-w31.sh, the DOS
key-to-paint run) and the DOS size; if the DOS budget is passed, take savings first, then a DOS row raising it
by the measured cost rounded to 4 KB with the floppy's free bytes (DOS-70). Gates: G-HOST (fuzz target too),
G-DOS, G-W16, G-W32, G-GTK, G-DISK, G-MD. Evidence in Editor features/WP-ED-11. Decisions CORE-103, CORE-104;
SPEC limits and characters. PR: "Keep the document's text as UTF-8".
```

### WP-ED-12 — Default tab stops in the document

- **Finish line:** the document holds its default tab interval (twips, 720 by default) in `DOCP` as an optional trailing field that a 1.0 reader skips, RTF reads and writes `\deftabN`, layout uses it, and every build reads and writes it (the owner's decision: default tabs are offered, so the file must carry them).
- **Owns:** `PageSetup` in `crates/quill-core/src/doc.rs` and layout's default tab; `DOCP` in `crates/quill-formats/src/qld.rs`; `\deftab` in `rtf.rs`; SPEC's file format section; file goldens with the field.
- **Depends on:** WP-ED-11 (the freeze).
- **Budget:** DOS about +0.3 KB; Win16 data 0.
- **Tests:** round trips; a 1.0 fixture opens; LibreOffice reads `\deftab`; G-HOST, G-DOS, G-W16, G-W32.
- **Evidence:** a document with a 0.75-inch default tab on `dos-text-25`, `win16-311`, `win32-wine`, `gtk-xvfb`.
- **Decisions:** CORE-105.

```text
Package WP-ED-12: store the default tab interval in DOCP (optional trailing field) and RTF \deftab. Branch
feat/default-tab after WP-ED-11. Follow the .QLD compatibility rule: a 1.0 reader skips it, a file without it
reads 720. Gates: G-HOST (round trips, 1.0 fixture), G-DOS, G-W16, G-W32, scripts/check-rtf-libreoffice.sh,
G-MD. Evidence in Editor features/WP-ED-12. Decision CORE-105; SPEC file format. PR: "Keep the default tab
interval in the document".
```

### WP-ED-13 — Settings by id, from one schema (preferences PRs 1, 3 and 4)

**Replaced** by the preferences series in [plans/preferences.md](plans/preferences.md) (ARCH-30): this package is now WP-PREF-2, with the model built first as new files in WP-PREF-1. The text below is kept for reference.

- **Finish line:** as the preferences design's steps 1, 3 and 4: B1, B5 and B6 fixed; `assets/settings/settings.json` is the one description and `emit-resources --settings` generates `crates/quill-settings/src/generated.rs` with a sync test; `App::get` and `App::set` by id; layers (build default, `QUILLDEF.INI`, `QUILL.INI`, session switches); a value equal to the inherited default deletes its key; unknown lines kept; `[Spelling] Language=en-CA` with a one-time migration from `[Quill] Language`; DOS `Rows` and `Graphics` are settings and the switches session overrides (B3, B4; `Kept` goes); the SPEC's settings table is generated; the unsaved-changes wording is one text in `quill-flow` (V7).
- **Owns:** `crates/quill-settings`; `assets/settings/settings.json`; `emit-resources --settings`; the settings region of `lib.rs` and `ini_file.rs`; the settings load and save in `dos/quill-dos/src/main.rs`; the key list in `win9x/quill-win9x/src/ui.rs`; the confirm texts in `quill-flow` and the two Windows shells; SPEC's settings table.
- **Depends on:** WP-ARCH-6, WP-ARCH-7, WP-ED-11.
- **Budget:** DOS +2 to +4 KB **[E]**; Win16 data ≤ +750, or the schema from `RT_RCDATA` (the owner's rule).
- **Tests:** the preferences design's property and sync tests; `m4-quill.ini` changes, reviewed; a 4 KB INI loads whole; `/50` is not saved; G-HOST, G-DOS, G-W16, G-W32.
- **Evidence:** `QUILL.INI` before and after a language change and back (text files) for DOS, Win16 and Win32; the spelling language shown on `dos-text-25` and `win32-wine`.
- **Decisions:** CORE-106, CORE-107, ARCH-19.

```text
Package WP-ED-13: preferences design steps 1, 3 and 4 ("Quill preferences design.md", sections 0, 2 and 5,
with the owner's answers at its end): bugs B1, B3 to B6, the settings schema and generator, App::get/set by
id, the layers, the language key migration, the DOS switches as session overrides, one unsaved-changes
wording. Branch feat/settings-by-id after WP-ARCH-6, WP-ARCH-7 and WP-ED-11. Win16: if the schema table costs
more than 750 bytes of data, serve it from RT_RCDATA. Gates: G-HOST, G-DOS, G-W16 (data figure), G-W32, G-MD.
Evidence in Editor features/WP-ED-13. Decisions CORE-106, CORE-107, ARCH-19; SPEC Settings. PR: "Read and
write every setting by id from one schema".
```

### WP-ED-14 — Windows 3.x settings through the profile functions (preferences PR 5, side)

**Replaced** by the preferences series in [plans/preferences.md](plans/preferences.md) (ARCH-30): this package is now WP-PREF-3. The text below is kept for reference.

- **Finish line:** the Win16 build reads and writes `QUILL.INI` through `GetPrivateProfileString` and `WritePrivateProfileString` with a bare name, so the file is in the Windows directory (B2); the program's folder holds none.
- **Owns:** the Win16 `Profile` implementation and its imports.
- **Depends on:** WP-ED-13. Parallel with WP-ED-15.
- **Budget:** Win16 data and stack gates.
- **Tests:** G-W16 (`C:\WINDOWS\QUILL.INI` holds the keys after a run on 3.11).
- **Evidence:** a directory listing and the file, as text, from the 3.11 run.
- **Decisions:** W16-62.

```text
Package WP-ED-14 (Win16 only): QUILL.INI through the profile functions in the Windows directory (B2). Branch
feat/w16-profile-api after WP-ED-13. Gates: G-W16 (data, stack, and the file in C:\WINDOWS after a run), G-MD.
Evidence in Editor features/WP-ED-14. Decision W16-62; SPEC Settings. PR: "Keep the Windows 3.x settings in
the Windows directory".
```

### WP-ED-15 — Tools > Options: the flow and the DOS dialog (preferences PRs 7, 8, 9, 11)

**Replaced** by the preferences series in [plans/preferences.md](plans/preferences.md) (ARCH-30): this package is now WP-PREF-5 (the flow and the text screens) and WP-PREF-6 (the graphics screen). The text below is kept for reference.

- **Finish line:** a Tools menu with Options... on the DOS, Windows 3.x and Windows 95 builds (the owner's choice; Spelling's Options button opens the Spelling page); `Command::Options(page)`, `Dialog::Options(page)`, a `Draft` the dialog edits, the flow entry points and the host verbs; `CaretBlink` (System for users, Off in every harness); the DOS text-cell Options dialog built from the schema at 80 × 25, 80 × 50 and mono, and the graphics overlay with the mouse; Options sets the defaults for new documents only (CORE-108).
- **Owns:** the Tools menu in `crates/quill-menu`; the Options flows in `crates/quill-flow`; `crates/quill-app/src/options.rs` (new) and its overlay and widget entries; `MENU_COLS` in `paint.rs`; the options drawing in `crates/quill-paint`; the restart notice in `dos/quill-dos/src/main.rs`; flows and goldens.
- **Depends on:** WP-ED-13, WP-ED-12, WP-ED-9.
- **Budget:** DOS +6 to +10 KB **[E]**.
- **Tests:** the six flows of the design; the Tools menu moves every DOS text screen's menu bar, so every screen golden changes: regenerate them in one commit of their own and review; the DOS run equals the host cell for cell; G-HOST, G-DOS.
- **Evidence:** the Options dialog's pages on `dos-text-25`, `dos-text-50`, mono, `dos-gfx-12h`, `dos-gfx-vbe`; the Tools menu on each.
- **Decisions:** CORE-108, CORE-109, DOS-73.
- **UI slice:** GTK `AdwPreferencesDialog` from the schema (Ctrl+,); WinUI a Settings page with a navigation pane.

```text
Package WP-ED-15: preferences design steps 7, 8, 9 and 11 — Tools > Options, the flow, CaretBlink, the DOS
text and graphics dialog from the schema. Branch feat/options-dos after WP-ED-13, WP-ED-12 and WP-ED-9. The new
menu shifts every DOS text menu bar: regenerate those goldens in a separate commit and review them. Gates:
G-HOST, G-DOS (size; DOS-70 rule if over), G-DISK, G-MD. Evidence in Editor features/WP-ED-15. Decisions
CORE-108, CORE-109, DOS-73; SPEC Settings and menus. PR: "Add Tools > Options to the DOS screens".
```

### WP-ED-16 — Options on Windows 3.x and Windows 95 (preferences PRs 12, 13)

**Replaced** by the preferences series in [plans/preferences.md](plans/preferences.md) (ARCH-30): this package is now WP-PREF-7 (Windows 95) and WP-PREF-8 (Windows 3.x). The text below is kept for reference.

- **Finish line:** Windows 95 shows a COMCTL32 4.00 property sheet with OK, Cancel, Apply and What's This, `Folder32`; Windows 3.x a category dialog with a description line and F1 popups; both from `dialogs.json` pages whose controls carry `"setting": "<id>"`, with a binding test.
- **Owns:** the Options entries in `assets/dialogs/dialogs.json` (regenerating `generated.rs`); the Options code in `win16/examples/quill/dialogs.rs` and `win9x/quill-win9x/src/dialogs.rs`; `res_pack.py` styles; `imports.allow` (`PropertySheetA`).
- **Depends on:** WP-ED-15, WP-ED-14, WP-ARCH-9.
- **Budget:** Win16 data: 1,024 bytes still free after, or the schema from `RT_RCDATA`; stack margin ≥ 2,048 with the dialog open.
- **Tests:** binding test; G-W16 (3.11 play-back through every page); G-W32 (Wine opens every tab, Apply writes a key that reads back, Cancel leaves the file byte-identical); G-DISK.
- **Evidence:** every page on `win16-311` and `win32-wine`, one on `win95`.
- **Decisions:** W16-66, W9X-63.

```text
Package WP-ED-16: preferences design steps 12 and 13 — the Options property sheet on Windows 95 and the
category dialog on Windows 3.x. Branch feat/options-windows after WP-ED-15, WP-ED-14 and WP-ARCH-9.
dialogs.json is shared: append your entries; on a rebase conflict in generated.rs, take either side and rerun
emit-resources --dialogs. Gates: G-W16 (data, stack with the dialog open), G-W32 (imports, Pentium scan,
Wine), G-DISK, G-HOST, G-MD. Evidence in Editor features/WP-ED-16. Decisions W16-66, W9X-63. PR: "Add the
Options dialog to Windows 3.x and Windows 95".
```

### WP-ED-17 — The Page Layout dialog

- **Finish line:** Document > Page Layout... sets the margins, the header and footer distances, the starting page number and the units (all already in `PageSetup`), as one undo step and a relayout, on every build; one measure parser (inches and centimetres) in `quill-flow` replaces the two Windows copies (V8), its rules written in the SPEC.
- **Owns:** the `page_layout` entry in `dialogs.json`; the flow and the measure parser in `crates/quill-flow`; `crates/quill-app/src/page_layout.rs` (new, the text form); the Document item in `quill-menu`; the Page Layout code in both Windows `dialogs.rs`; tests.
- **Depends on:** WP-ARCH-7, WP-ED-11. Parallel with WP-ED-13 to WP-ED-16 under the `dialogs.json` rule.
- **Budget:** DOS +3 to +5 KB **[E]**; Win16 data about +100 **[E]**.
- **Tests:** the measure parser's table (both old parsers' cases; where they differ, the SPEC rule decides and the test says which); flows; G-HOST, G-DOS, G-W16, G-W32.
- **Evidence:** the dialog and the page after a 2-inch left margin on `dos-text-25`, `dos-gfx-vbe`, `win16-311`, `win32-wine`.
- **Decisions:** CORE-110, DOS-74, W16-67, W9X-64.
- **UI slice:** GTK builds it from `quill-dialogs` (G3); WinUI from the same (W5).

```text
Package WP-ED-17: Document > Page Layout... on every build, with one measure parser. Branch
feat/page-layout-dialog after WP-ARCH-7 and WP-ED-11. Compare win16 dialogs.rs parse_inches with win9x's; write
one in quill-flow and a table test of both sets of cases. dialogs.json rule as in WP-ED-16. Gates: G-HOST,
G-DOS, G-W16, G-W32, G-DISK, G-MD. Evidence in Editor features/WP-ED-17. Decisions CORE-110, DOS-74, W16-67,
W9X-64; SPEC Printing and preview. PR: "Add the Page Layout dialog to every build".
```

### WP-ED-18 — The Numbering dialog, Restart and Continue Numbering

- **Finish line:** Paragraph > Numbering... chooses the style per level (`1.`, `1)`, `a.`, `a)`, `i.`, `A.`, `I.`, `1.1.`), the start number and whether this paragraph restarts (SPEC); Restart Numbering and Continue Numbering are commands (CORE-55); every build. If the list table cannot hold a style per level, the format addition (`.QLD` and RTF `\pn` levels) lands first as its own commit with its goldens.
- **Owns:** the `numbering` entry in `dialogs.json`; the flow; `crates/quill-app/src/numbering.rs` (new); the list table in `quill-core` and its formats if needed; the Paragraph items in `quill-menu`; the Numbering code in both Windows `dialogs.rs`.
- **Depends on:** WP-ED-17.
- **Budget:** DOS +3 to +5 KB **[E]**; Win16 data about +100 **[E]**.
- **Tests:** `m2-lists` check; round trips; LibreOffice reads the numbering; G-HOST, G-DOS, G-W16, G-W32.
- **Evidence:** a three-level outline in `1.1.` style and a restarted list on `dos-text-25`, `dos-gfx-12h`, `win16-311`, `win32-wine`.
- **Decisions:** CORE-55, CORE-111, DOS-75, W16-68, W9X-65.
- **UI slice:** as WP-ED-17.

```text
Package WP-ED-18: Paragraph > Numbering... and the Restart and Continue Numbering commands (review PR 11). Branch
feat/numbering-dialog after WP-ED-17. If the list table lacks per-level styles, add them to the model and the
formats first, in their own commit, with goldens. Gates: G-HOST, G-DOS, G-W16, G-W32, check-rtf-libreoffice.sh,
G-DISK, G-MD. Evidence in Editor features/WP-ED-18. Decisions CORE-55, CORE-111, DOS-75, W16-68, W9X-65; SPEC
Lists. PR: "Add the Numbering dialog and restart numbering on every build".
```

### WP-ED-19 — The Header and Footer dialogs

- **Finish line:** Document > Header... and Footer... switch to the header or footer view and open the small modeless dialog the SPEC describes (distance from top or bottom, Different first page with First page and Other pages, the four field buttons, Clear and Return to Document) on Windows 3.x and 95 (`CreateDialog` and `IsDialogMessage`); on DOS a one-row strip over the status line in the header view, with the same controls and mnemonics, drawn by the text painter and the graphics overlay.
- **Owns:** the `header` and `footer` entries in `dialogs.json`; the flow; `crates/quill-app/src/header_box.rs` (new); the modeless handling in both Windows shells; the strip in `paint.rs` and `quill-paint`.
- **Depends on:** WP-ED-17 (parallel with WP-ED-18 under the `dialogs.json` rule).
- **Budget:** DOS +3 to +5 KB **[E]**; Win16 data about +80 **[E]**.
- **Tests:** flows (open, insert a field, distance, Clear, Return); the header view's undo steps survive Return (SPEC); G-HOST, G-DOS, G-W16, G-W32.
- **Evidence:** the header view with the dialog or strip on `dos-text-25`, `dos-gfx-vbe`, `win16-311`, `win32-wine`; a printed preview with the header.
- **Decisions:** CORE-112, DOS-76, W16-69, W9X-66.
- **UI slice:** GTK a non-modal `AdwDialog` or a revealer bar; WinUI an `InfoBar`-style strip.

```text
Package WP-ED-19: Document > Header... and Footer... with the SPEC's modeless dialog (Windows) and a strip in
the header view (DOS). Branch feat/header-footer-dialog after WP-ED-17. Gates: G-HOST, G-DOS, G-W16 (data,
stack with the dialog up), G-W32, G-DISK, G-MD. Evidence in Editor features/WP-ED-19. Decisions CORE-112,
DOS-76, W16-69, W9X-66; SPEC Headers and footers. PR: "Add the Header and Footer dialogs to every build".
```

## Found while planning

- **CORE-64 is used twice** in `docs/decisions.md` (the default-extension rule, and the page view scrolling in pixels). Neither is renumbered here, because the SPEC and the code comments cite both; the next change that touches either should give the second one a sub-number under CORE-65 and fix its citations.
- **The layer of `quill-session` was written two ways**: the web and layering addendum put it in the kit, the threading plan at the top of the editor. ARCH-8 settles it as the editor's top.
- **`quill-base` (the review's B1) is still not built**, so five base crates use the document model for `Store` and `Source` (V3 in `scripts/layer-allow.txt`). This track does not need it: `quill-settings` works on slices (ARCH-9) and nothing else here is base-and-store. It stays deferred.
- **The Unicode plan's decision ids CORE-62 to CORE-68** were taken by the pictures and zoom work after it was written; its U1 and U2 take CORE-100 to CORE-104 here.
- **`docs/architecture.md`'s directory sketch** still lists `quill-pack` (not built) and leaves out `quill-job`, `quill-mouse` and `quill-text`; WP-ARCH-1 fixes the crate table and sketch when it edits that page.
