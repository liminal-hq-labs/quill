# The UI track

The work packages that finish Quill's two modern clients: the GTK4 client to parity with the Windows builds, then the WinUI 3 client from the spike to a full application. Each package is one builder's job and one pull request, or a short stack of them. Behaviour is in [SPEC.md](../SPEC.md), the client designs in [linux.md](linux.md) and [winui.md](winui.md), the spikes in [winui-spike.md](winui-spike.md), and the choices in [decisions.md](decisions.md) (LNX-21 to LNX-24, WUI-1 and WUI-2 set up this track). The WinUI half follows [plans/winui-real-window.md](plans/winui-real-window.md), which turned the owner's answer of 2026-11-27 ("a real WinUI window") into the route, the S0c measurements and the packages (WUI-85 to WUI-94).

## Table of contents

- [Where the clients stand](#where-the-clients-stand)
- [Rules for every package](#rules-for-every-package)
- [Order and parallelism](#order-and-parallelism)
- [What the track needs from other work](#what-the-track-needs-from-other-work)
- [GTK4 packages](#gtk4-packages)
- [WinUI 3 packages](#winui-3-packages)
- [Builder briefs](#builder-briefs)
- [Owner decisions](#owner-decisions)

## Where the clients stand

**GTK4** (`linux/quill-gtk`). G1 (viewer), G2a (libadwaita shell and the Windows chrome) and G2 (editing, input method, pointer, Save) are built. Of the plan's later items, G4 (bars, ruler, status) came with G2a; G3 is mostly there (Find, Replace, Fonts, Tabs, Spelling, About, the unsaved-changes question, Open, Save As and Insert Picture through `GtkFileDialog`) but the file dialogs were never answered by a test, mnemonics were never run, and `Dialog::Print` is a toast. G3 has since been built (WP-GTK-1, LNX-25 to LNX-28). Not started: the system clipboard (G5), printing (G6), accessibility (G7), spelling marks on the page, header and footer editing on the page, packaging (G8).

**WinUI 3** (`winui/`). Only the spikes exist. S0a passed (a pixel-exact page in a Reactor window from a zip built on Linux). S0b did not reach a go: a child window under XAML is invisible, an owned pop-up takes typing, dead keys and Alt codes, and AccessKit returns the document text, but input methods, the rest of the text pattern, Narrator, Tab handover and dialogs over the pop-up were never run. So there is no viewer (W3) yet, and the hosting is undecided (WUI-1). The owner asked on 2026-11-27 for a real WinUI window, which takes Option B (a Win32 window imitating WinUI) off the table (WUI-85): S0c now measures route H1, a WinUI 3 window over Quill's own projection with the page in a `SwapChainPanel` inside a composed control, `CoreTextEditContext` for text and a composed automation peer for Narrator, with real-WinUI fallbacks ([plans/winui-real-window.md](plans/winui-real-window.md)).

## Rules for every package

- **One package, one branch, one worktree** under `~/source/worktrees/quill/<topic>`, from `origin/main`. The main checkout at `~/source/experiments/quill16` is used only for `git fetch` and `git worktree add` and `remove`.
- **GTK-only and WinUI-only work stays in the client's crate** (`linux/quill-gtk`, `winui/quill-winui`) until the architecture split it belongs to has landed (LNX-21, WUI-2). A package that finds it needs a change in a shared crate (`crates/**`, `assets/dialogs/dialogs.json`) stops and reports instead of making it; the lead routes it to the architecture track or to the parked block.
- **Shared files are touched only as listed.** Each package updates its own rows in `docs/linux.md` or `docs/winui.md`, its own row in `docs/progress.md`, its own `LNX-` or `WUI-` decisions (the next free numbers when it opens its pull request), and the SPEC's rows for its client. Nothing else in `SPEC.md`.
- **One GTK builder at a time** by default, because every GTK package wires into `window.rs`. The same holds for WinUI after WP-WINUI-3. The exceptions are named in the order table.
- **Evidence.** Screenshots are taken in a private Xvfb, a headless Weston or the Windows VM's framebuffer, never the host desktop. Every shot the builder took goes to `~/Documents/Liminal HQ/Quill/GTK4/<package>/` or `~/Documents/Liminal HQ/Quill/WinUI/<package>/`; the few that document the feature are also committed under `docs/media/` with the existing `gtk-*` and `winui-*` names. GTK shots show libadwaita's own GNOME look (Adwaita header bar, window controls at the right as GNOME draws them); never macOS-style chrome in mock-ups or art.
- **Gates.** GTK: `make gtk` (format, clippy with `-D warnings`, tests in `linux/quill-gtk`). WinUI: the workspace's own format, clippy and host tests, plus `winui/tools/build.sh` under `cargo xwin`. Both: `bash scripts/check-headers.sh` and markdownlint in Docker. `make validate` too when a package touches anything under `crates/` (none should).
- **Windows VM.** `WinDev2407Eval` (8 GB guest) from the snapshot `Password never expires`, network cable off, restored and powered off at the end. Check free memory (`free -g`, 12 GB available or more), `df -h /mnt/games` (20 GB free) and `df -h /home` (above 20 GB) before starting it, and do not start it if any fails; S0b was cut short when the host ran out of memory. While it runs it counts against the builder cap: at most one other builder, host-only. Build on the host and copy only the zip in. The guest password is never printed or written.
- **One heavy tool at a time** (the VM, a Weston and Xvfb pair, a full Wine or DOSBox run), and `CARGO_TARGET_DIR` outside the repository for the WinUI build.

## Order and parallelism

The two clients' sequences are independent of each other except where noted; within a client the packages run in the order given unless the last column says otherwise.

| # | Package | Starts when | Runs beside |
|---|---|---|---|
| 1 | **WP-GTK-1** Dialogs and keyboard (G3) | Built | — |
| 2 | **WP-GTK-3a** Print Preview (G6) | Now: needs nothing shared | WP-WINUI-* |
| 3 | **WP-GTK-8** Help viewer | After WP-GTK-3a when pull request #167 (`quill-pdf`, `quill-pdfdoc`) has not merged; otherwise after WP-GTK-3c | WP-WINUI-* |
| 4 | **WP-GTK-3b** Export as PDF (G6) | WP-GTK-3a merged and `quill-pdf` and `quill-pdfdoc` (WP-WEB-3, #167) merged | WP-WINUI-* |
| 5 | **WP-GTK-3c** Print (G6) | After WP-GTK-3b | WP-WINUI-* |
| 6 | **WP-GTK-2** System clipboard (G5) | The shared clipboard bridge in `quill-app` has merged (below); takes the next free GTK slot | WP-WINUI-* |
| 7 | **WP-GTK-4** libadwaita finish and dark mode | After WP-GTK-3c | WP-WINUI-* |
| 8 | **WP-GTK-10** Dark paper | After WP-GTK-4 | WP-WINUI-* |
| 9 | **WP-GTK-5** Accessibility (G7) | After WP-GTK-10 | WP-WINUI-* |
| 10 | **WP-GTK-6** The page's own parity: spelling marks, headers and footers | Anywhere after WP-GTK-1 | WP-WINUI-* |
| 11 | **WP-GTK-7** The parked block's dialogs and keys on GTK | Each item when its shared part has merged | WP-WINUI-* |
| 12 | **WP-GTK-9** Packaging and a `linux` CI job (G8) | Later, by owner's call | — |
| 13 | **WP-WINUI-1** Hosting decision (S0c): route H1 measured | Now (VM time; one heavy tool at a time) | GTK packages (host-only while the VM runs), WP-WINUI-2 |
| 14 | **WP-WINUI-2** The portable shell core | Now (host only, new files) | GTK packages, WP-WINUI-1 |
| 15 | **WP-WINUI-3** Viewer (W3), the projection promoted | WP-WINUI-1 has decided the hosting; WP-WINUI-2 merged | GTK packages |
| 16 | **WP-WINUI-11a** CI job | After WP-WINUI-3 | Any WinUI package |
| 17 | **WP-WINUI-4** Editing and text input (W4) | After WP-WINUI-3 | GTK packages |
| 18 | **WP-WINUI-5** Menus and dialogs (W5) | After WP-WINUI-4 | GTK packages |
| 19 | **WP-WINUI-6** Command bars, ruler and status bar (W6) | After WP-WINUI-5 | GTK packages |
| 20 | **WP-WINUI-7** Clipboard (W7) | After WP-WINUI-4 and WP-ED-5 | GTK packages |
| 21 | **WP-WINUI-8** Mica, themes, title bar and dark paper (W8) | After WP-WINUI-6 | GTK packages |
| 22 | **WP-WINUI-9** Narrator and the text pattern (W9) | After WP-WINUI-4; with or after WP-ARCH-8 | GTK packages |
| 23 | **WP-WINUI-10** Printing, preview and Export as PDF (W10) | After WP-WINUI-6 | GTK packages |
| 24 | **WP-WINUI-12** Help viewer | After WP-GTK-8 and WP-WINUI-5 | GTK packages |
| 25 | **WP-WINUI-11b** Packaging (W11) | Later, by owner's call | — |

With the builder cap of three, the steady state is one GTK builder, one WinUI builder, and the third slot for the parked block or the architecture track. WP-WINUI-1 and WP-WINUI-2 are the only pair inside one client that run together (one is VM work, the other host code in new files), and WP-WINUI-11a (CI only) may run beside any WinUI package. While any package has the Windows VM running, the lead runs two builders, not three.

## What the track needs from other work

The architecture follow-ups (`quill-session`, `quill-chrome`, `quill-picture`, `quill-a11y`, `quill-input`, `quill-menu`, `quill-settings`, `quill-flow`) run in parallel and change `quill-app`. No package here waits for a split: each one works against `main` as it is, keeps its client-only code in the client's crate, and names the split it will later move onto. Moving client code onto a split is the split's own pull request, not part of a package here.

| Needed from | What | Packages |
|---|---|---|
| Parked block: rich clipboard copy | The bridge `docs/clipboard.md` describes: `App::take_clip_request`, `copy_to`, `paste_from`, with the 2,048-byte buffer gone. Nothing of it is in `quill-app` today | WP-GTK-2 (hard), WP-WINUI-7 (hard) |
| Parked block: Page Layout, Numbering, Header and Footer dialogs; preferences (Tools > Options); Size Picture; Page Break and word movement keys; Edit menu state | The shared descriptions and `App` calls | WP-GTK-7 (hard, per item); WP-WINUI-5 takes whatever has landed |
| Parked block: GTK and Win16 picture wiring | Pictures on the GTK page | None here; WP-GTK-3a to 3c print what the page shows |
| Web lane: `quill-pdf` and `quill-pdfdoc` (WP-WEB-3, pull request #167) | The PDF writer with a text layer and the document walker, on `main` | WP-GTK-3b (hard), WP-GTK-3c (hard) |
| The owner of `quill-pdf` and `quill-pdfdoc`, then the architecture lane: S1 to S4 and S6 of [gtk-print-help.md](plans/gtk-print-help.md#what-the-shared-crates-are-asked-for) | Embedded TrueType (WP-PDF-1), one walker for preview and paper (WP-PDF-2), pictures at their own resolution, a list of pages, `Command::ExportPdf` in the menu tree | WP-GTK-3b and 3c (soft: they ship the first stage without them) |
| Content lane: S5 | A help topic per dialog with its `[MAP]` number | WP-GTK-8 (soft: F1 in a dialog opens the contents) |
| Autosave plan PR 18 (`feat/gtk-executor`) and PR 19 (`feat/winui-executor`) | Worker threads, autosave and the logout inhibit on GTK and WinUI | None here; PR 18 can land between any two GTK packages, PR 19 after WP-WINUI-4 |
| `quill-flow` (C6) | Confirm-save routing, menu state | WP-GTK-1 and WP-WINUI-5 use today's `quill_app::session` and `App::menu_mark`; they move later |
| `quill-menu` (C2) | Command vocabulary and the menu tree | WP-GTK-1, WP-WINUI-5 use `quill_app::menu_items` today |
| `quill-input` (C1, with the event model R2) | `Key`, `accel`, the input event model | WP-WINUI-4 uses `quill_app::Key` and `accel::from_vk` today |
| `quill-chrome` (C5) and `quill-session` (R3) | Window geometry and the pixel-shell driver | WP-WINUI-2 and WP-WINUI-3 keep a local view model shaped like GTK's `view.rs` until they land |
| `quill-settings` (C4) | INI and the recent list | WP-WINUI-2 uses `App::each_setting` and `apply_setting` today |
| `quill-picture` (P2) | The picture cache | None here |
| `quill-a11y` (R5) | The text-accessibility snapshot | WP-GTK-5 builds a local snapshot of R5's shape (LNX-24); WP-WINUI-9 is its second user, which is when R5 can land |

## GTK4 packages

### WP-GTK-1 Dialogs and keyboard (G3)

- **Finish line.** Every `quill_app::native::Dialog` variant except `Print` is reached from the menu and from the keyboard in the real window and answered through it, and a test proves each answer reaches the document: About, Open, Save As, Insert Picture, Fonts, Tabs, Find, Replace, Spelling, and the unsaved-changes question for every command that asks (New, Open, a recent file, a drop, a hand-off, Exit, closing the window). The file dialogs go through a seam that a test can answer. Alt mnemonics work on the menu bar and in every dialog; Enter is the default button and Esc cancels.
- **Files owned.** New `linux/quill-gtk/src/choose.rs` (the file-dialog seam), new `linux/quill-gtk/tests/dialogs.rs`; `linux/quill-gtk/src/dialogs.rs`, `src/main.rs` (test hooks), `src/window.rs` (wiring only), `tools/chrome-shots.sh` or a new `tools/dialog-shots.sh`; the G3 rows of `docs/linux.md`, the GTK rows of `docs/progress.md`, new `LNX-` rows.
- **Must not touch.** `crates/**`, `assets/**`, `dos/**`, `win16/**`, `win9x/**`, `winui/**`, the root `Cargo.toml` and `Cargo.lock`, `.github/**`, other packages' rows.
- **Dependencies.** None. Later moves onto `quill-flow` (C6) and `quill-menu` (C2).
- **Tests.** Every variant answered through the seam with the bytes saved or the document opened checked; Cancel leaves the document alone; the type filters; the unsaved-changes question for every asking command; `--self-test-dialogs` extended to all of them; Alt mnemonics and Enter and Esc driven with xdotool in a private Xvfb.
- **Evidence.** Every dialog in light and dark at scale 1 and 2; the File menu opened by Alt+F; the real Open and Save As choosers shown under a headless Weston.
- **Parallel.** With WP-WINUI-1 and WP-WINUI-2.

### WP-GTK-2 System clipboard (G5)

- **Finish line.** Cut, Copy and Paste use the system clipboard through `quill-clip`: a copy offers `text/rtf` (and `application/rtf`), UTF-8 text and, for a lone BMP picture, `image/bmp`; a paste takes RTF first, then the picture, then text, with UTF-8 read through `quill-text` and characters Windows-1252 lacks refused with the count (D188). Bold survives a round trip with LibreOffice Writer.
- **Files owned.** New `linux/quill-gtk/src/clip.rs` (`impl quill_clip::Clipboard` over buffers read ahead from `gdk::Clipboard`, a `gdk::ContentProvider` serving the committed flavours); `linux/quill-gtk/Cargo.toml` (adds `quill-clip`, `quill-text`); `src/window.rs` (serving the clipboard request); `docs/linux.md` (the clipboard section, LNX-18 marked replaced), `docs/clipboard.md` (the GTK row only).
- **Must not touch.** Everything WP-GTK-1 lists, plus `crates/quill-clip/**`.
- **Dependencies.** **Hard:** the parked block's rich clipboard bridge in `quill-app`. No plain-text stand-in (LNX-23).
- **Tests.** The `Clipboard` implementation against `quill-clip`'s own tests' cases; under a private Xvfb, `xclip -selection clipboard -t text/rtf -o` after a copy gives what `quill_clip::copy` writes, and `xclip -i -t text/rtf` then Paste saves the expected bytes; a damaged RTF with text beside it pastes the text; an emoji in UTF-8 text is refused with the count.
- **Evidence.** A shot of a paste from LibreOffice keeping bold and colour, and of the refusal toast.
- **Parallel.** With WinUI packages. Waits for the bridge; the print packages and WP-GTK-8 go first if the bridge is late.

### WP-GTK-3a, 3b and 3c Printing, preview and Export as PDF (G6)

The owner's answer of 2026-11-27 (a proper PDF with a text layer, LNX-80) replaced the raster design of LNX-22. The design, the three packages (Print Preview, Export as PDF, Print), their files, tests, evidence and briefs are in [plans/gtk-print-help.md](plans/gtk-print-help.md): `quill_pdfdoc::export` (pull request #167) with the window's own `OutlineMeasure` writes the PDF that is both the exported file and what `gtk::PrintDialog` prints (through CUPS, or the print portal under Flatpak); preview is Quill's own from the pixel surface.

### WP-GTK-4 libadwaita finish and dark mode

- **Finish line.** The window looks and behaves like a current GNOME application in a real GNOME compositor: the desktop's dark preference, accent colour and high contrast are followed (the ruler, selection, focus rings and desk colours included); the window adapts below 600 logical pixels (an `adw::Breakpoint` moves the formatting bar's boxes into an overflow and drops the zoom box, as the Win32 build drops its zoom pane under 560); F6 moves the focus between the page, the toolbar, the formatting bar, the ruler and the status bar; `gtk-enable-animations` off stops the caret blink. The paper stays white (LNX-5); the dark paper option is WP-GTK-10, after this package, through the colour wrapper this package adds for the accent.
- **Files owned.** `linux/quill-gtk/src/window.rs`, `src/ruler.rs`, `src/status.rs`, `src/format_bar.rs`, `src/toolbar.rs`, `src/blink.rs`, `tools/chrome-shots.sh`; docs rows.
- **Must not touch.** As WP-GTK-1.
- **Dependencies.** None.
- **Tests.** The colour choices as data (scheme, accent, high contrast in; colours out); the breakpoint's layout as data; F6 order; the blink with animations off.
- **Evidence.** Light, dark, high contrast and two accent colours at scale 1 and 2, the narrow layout, all from a real GNOME compositor run headless (`mutter --headless --virtual-monitor` or `gnome-shell --headless`, whichever the host has; Xvfb only if neither starts, and say so).
- **Parallel.** With WinUI packages.

### WP-GTK-5 Accessibility (G7)

- **Finish line.** Orca reads the page: the page widget implements `GtkAccessibleText` (GTK 4.14 or later) as a multi-line text box with the line at the caret, the caret offset, the selection and the text attributes that matter (bold, italic, underline, font, size), and it announces caret and content changes. Every toolbar and formatting-bar control has an accessible label (the shared tips), the ruler is described, the status bar's message is announced, and every dialog control is labelled by its row.
- **Files owned.** New `linux/quill-gtk/src/a11y.rs` (a snapshot of R5's shape: the visible lines as UTF-8, per-character x in device pixels, caret, selection, line bounds, role, name, a revision; LNX-24), new `tests/a11y.rs`, new `tools/atspi-read.py`; `src/page.rs`, `src/toolbar.rs`, `src/format_bar.rs`, `src/ruler.rs`, `src/status.rs`, `src/dialogs.rs` (labels only); docs rows.
- **Must not touch.** As WP-GTK-1; no `crates/quill-a11y` (the architecture track's).
- **Dependencies.** None required. `quill-a11y` (R5) later takes `a11y.rs`'s snapshot when WP-WINUI-9 is its second user.
- **Tests.** The snapshot over real documents (text, caret, selection, line bounds at scales 1 to 2); an AT-SPI script (`gi.repository.Atspi` on a private session bus with `at-spi-bus-launcher`, under a private Xvfb) reads the caret line and offset after keys and the selection after a Shift+arrow; a manual Orca checklist in `docs/linux.md`.
- **Evidence.** Accerciser or the AT-SPI dump as text, and a shot of Orca's speech viewer if a session with Orca can be run headless.
- **Parallel.** With WinUI packages.

### WP-GTK-6 The page's own parity: spelling marks, headers and footers

- **Finish line.** Check Spelling as You Type draws the shared painter's waves on the GTK page (`spell_idle` from a `glib` timeout after 300 ms of quiet, as Win32 does), and a right click or Shift+F10 on a marked word opens a popover of suggestions, Ignore All and Add. Document > Header, Footer, First Page Header and Footer show the story being edited with its caret and selection.
- **Files owned.** New `linux/quill-gtk/src/spell.rs`; `src/page.rs`, `src/doc.rs`, `src/window.rs`; docs rows.
- **Must not touch.** As WP-GTK-1. A change needed in `quill-paint` to draw a header story with a caret stops the package.
- **Dependencies.** None.
- **Tests.** Marks equal what F7 finds once the scan ends (as `spell_live.rs` does on the host); the popover's choices change the document as the Windows menu does; typing in a header saves the bytes the host's header scenario saves.
- **Evidence.** Waves and the popover in light and dark; a header being edited.
- **Parallel.** With WinUI packages; between any two GTK packages after WP-GTK-1.

### WP-GTK-7 The parked block's dialogs and keys on GTK

- **Finish line.** Each dialog and key the parked block adds to the shared core is on GTK the day after it lands: Page Layout, Numbering, Header and Footer, Tools > Options (as an `adw::PreferencesDialog` over the same settings), Size Picture, Page Break, and the editor's word movement keys (the GTK layer of LNX-13 removed when the editor has them), Edit menu state.
- **Files owned.** `linux/quill-gtk/src/dialogs.rs`, `src/keys.rs`, `src/command.rs`, `src/window.rs`; docs rows.
- **Must not touch.** As WP-GTK-1.
- **Dependencies.** **Hard, per item:** the shared part of each in the parked block.
- **Tests.** Each new `Dialog` variant through WP-GTK-1's seam and self-test; the host's scenarios for the new keys replayed through `tests/replay.rs`.
- **Evidence.** Each new dialog in light and dark.
- **Parallel.** Small items; each is its own pull request when its shared part merges.

### WP-GTK-8 Help viewer

An in-app libadwaita window over the parsed `assets/help/quill.qhm` (LNX-87): contents, jumps, pop-ups, Back and Forward, search and F1, in [plans/gtk-print-help.md](plans/gtk-print-help.md#wp-gtk-8-help-viewer), with its brief. Wanted by the owner (2026-11-27).

### WP-GTK-9 Packaging and CI (G8)

As G8 in [linux.md](linux.md#work-list): PKGBUILD, Flatpak manifest on `org.gnome.Platform`, a `linux` CI job in a shared image with GTK4 development files, AppStream data, a release workflow; D185 (the `quill` name) decided first. Inside Flatpak it adds the host's `/run/host/fonts` and `/run/host/user-fonts` to the font set, so the layout matches the native build, and checks printing through the real print portal. Later, by the owner's call; no brief yet.

### WP-GTK-10 Dark paper

`[Quill] PaperColour=theme` draws the editing page as dark paper in the dark style, by the colour rules of LNX-88; print, preview, export and thumbnails stay white. In [plans/gtk-print-help.md](plans/gtk-print-help.md#wp-gtk-10-dark-paper), with its brief. Wanted by the owner (2026-11-27), off by default, after WP-GTK-4.

## WinUI 3 packages

The WinUI packages, their finish lines, files, tests, evidence, budgets and VM use are in [plans/winui-real-window.md](plans/winui-real-window.md#work-packages), which replaced this section's earlier entries on 2026-11-27 for the owner's answer that the client is a real WinUI window. The changes from the earlier list: WP-WINUI-1 measures route H1 (a WinUI 3 window over Quill's own projection, the page in a `SwapChainPanel` in a composed control) instead of comparing the pop-up with Option B; WP-WINUI-2 gains the text-service and automation-range modules; WP-WINUI-4 carries all typed text through `CoreTextEditContext`, input methods included; WP-WINUI-9 is Narrator and the text pattern; WP-WINUI-10 prints vector pages through the Windows print dialog and exports the GTK client's PDF (WUI-92, D169 superseded); WP-WINUI-11 splits into 11a (a CI job, early) and 11b (packaging, by the owner's call); WP-WINUI-12, a help viewer, is new.

## Builder briefs

Each brief is the common preamble followed by the package's own part. Paste both.

### Common preamble

```text
You are a Sonnet builder for Quill (repo liminal-hq-labs/quill). Read AGENTS.md, SPEC.md, docs/ui-track.md
(your package and "Rules for every package"), and the client's design doc (docs/linux.md or docs/winui.md).

Where to work: never touch ~/source/experiments/quill16 except `git fetch` and `git worktree add/remove`.
Create your worktree at ~/source/worktrees/quill/<topic> on branch <branch> from origin/main.
Remove it after the PR merges.

Rules: Canadian English, real em dashes, sentence case, no hard-wrapped markdown. SPDX header on every new
source, script and TOML file. Never `git add -A` or `git commit -a`; stage named paths. Never delete with
globs. Commits: Conventional Commits, body in markdown with bold labels and no headings, hard-wrapped at
about 72 to 100 columns, written to a file with a single-quoted heredoc and committed with `git commit -F`,
ending with `Co-Authored-By: Claude Sonnet <noreply@anthropic.com>` naming your own model. Push only your
branch; open one PR (title capitalized, imperative, no prefix; body `## Summary` with bold lead-in bullets,
`## Test plan` with the commands run, ending with an unchecked "Not verified:" line). Do not merge.

Stay inside "Files owned". If you need a change in crates/**, assets/** or any must-not-touch path, stop and
report what and why instead of making it. When a choice is not covered, take the reasonable default, record
it as the next free LNX- or WUI- decision marked **Assumed**, list it in the PR as "OWNER DECISION: question |
default taken | where to swap", and keep going.

Evidence: screenshots only from your own Xvfb, headless Weston or mutter, or the Windows VM framebuffer;
never the host desktop and never key injection into it. Kill only processes you started, by PID. Put every
shot in ~/Documents/Liminal HQ/Quill/GTK4/<package>/ (or WinUI/<package>/), and commit the few that document
the feature under docs/media/. GNOME/Adwaita look, never macOS chrome.

Before the PR: the client's gates (make gtk, or the WinUI workspace's fmt, clippy -D warnings and host tests),
bash scripts/check-headers.sh, and markdownlint in Docker. Update docs/progress.md (your row), the client doc's
section and work list row, and decisions.md in the same change. Report: PR URL, what was and was not run,
the evidence paths, OWNER DECISION lines, and anything you stopped on.
```

### WP-GTK-1 Dialogs and keyboard (full brief)

```text
Package WP-GTK-1, Dialogs and keyboard (G3). Branch feat/gtk-dialogs-g3, worktree
~/source/worktrees/quill/gtk-dialogs-g3.

Read first: docs/linux.md sections "The libadwaita shell and the chrome (G2a)" and "Editing in the GTK
client (G2)", decisions LNX-7 to LNX-11, crates/quill-app/src/native.rs (the Dialog enum),
crates/quill-app/src/savename.rs, linux/quill-gtk/src/window.rs (run_command and the Dialog match near
line 1110; the three gtk::FileDialog builders near lines 1300 to 1410), src/dialogs.rs and src/main.rs.

Goal: every quill_app::native::Dialog variant except Print is reached from the menu and from the keyboard in
the real window, answered through it, and a test proves each answer reaches the document. Print stays a
toast (WP-GTK-3).

Build, in this order, one commit each where it makes sense:

1. A file-dialog seam, new src/choose.rs. A `Chooser` trait with open, save_as and picture, each taking the
   start folder, the suggested name and the filter list and answering through a callback with an optional
   path (and, for Save As, the chosen filter index if GTK reports one). `GtkChooser` wraps gtk::FileDialog as
   window.rs does today; `ScriptChooser` answers from a queue for tests. window.rs's Open, Save As and Insert
   Picture go through it with no change in behaviour.
2. Filters as the Win32 build has them: Open lists Quill documents (*.qld), Rich Text (*.rtf), Text (*.txt)
   and All files; Save As lists Quill document, Rich Text and Text and completes the name with
   quill_app::savename (complete_name, ext_for_filter); Insert Picture lists BMP and PNG. Find out whether
   gtk::FileDialog::save reports the chosen filter on GTK 4.20 (it may not). If it does, choosing a type
   swaps the extension and keeps the name, as on Win32; if it does not, the typed extension decides and
   you record that as the next LNX- decision.
3. The unsaved-changes question for every asking command: New, Open, a recent file, a dropped file, a file
   handed to a running window, Exit and closing the window, each with Save, Don't Save and Cancel, and Save
   with no name going to Save As first (LNX-11). Most exist; test each one.
4. Keyboard. Alt mnemonics on the PopoverMenuBar (Alt+F opens File, then the item's letter runs it); in the
   dialogs, the `&` of each label from quill-dialogs becomes a GTK mnemonic (use_underline) on the control
   it labels; Enter runs the default button (set the dialog's default widget from the description's default);
   Esc is Cancel; focus starts in the first field with its text selected (Find); Tab order follows the
   description's control order.
5. Test hooks. Extend `quill --self-test-dialogs` to every variant (the file dialogs through ScriptChooser
   when `--script-chooser FILE` gives answers) and print one line per dialog and button: shown, answered,
   effect. Keep `--dialog NAME`.
6. New tests/dialogs.rs: for each variant, the answer reaches the document (opened text, saved bytes compared
   with what a headless Document writes, inserted picture count, font and tab changes, find position,
   replace count, spelling change); Cancel changes nothing; filters and name completion; the question for
   each asking command with each answer.
7. Display checks, all in your own Xvfb (and a headless Weston for the real choosers, with a private session
   bus and GTK_USE_PORTAL=0, as docs/linux.md describes): xdotool drives Alt+F and an item, Alt+letter in
   Find and Fonts, Enter and Esc; the result is asserted from the program's output, not by eye alone. A
   tools/dialog-shots.sh (or an extension of chrome-shots.sh) takes the shots below.

Must not touch: crates/**, assets/** (dialogs.json included: if a control lacks a `name` you need, stop and
report), dos/**, win16/**, win9x/**, winui/**, the root Cargo.toml and Cargo.lock, .github/**, SPEC.md
outside the "Linux desktop" platform row and the Quill for Linux desktops component line.

Architecture: needs no split. Do not anticipate quill-flow or quill-menu; use quill_app::session and
menu_items as they are. Autosave plan PR 18 (GTK executor, logout inhibit) may land meanwhile; rebase onto it
and leave its code alone.

Finish line checks (all must hold):
- make gtk passes; the G1 frames check (tools/frames.sh) is still 100.000 % at scales 1 and 2.
- tests/dialogs.rs covers every variant except Print, with Cancel, and every asking command.
- --self-test-dialogs reports every variant answered.
- Alt+F, an Alt mnemonic in a dialog, Enter and Esc are proved under Xvfb by script.
- The real Open and Save As choosers are shown under headless Weston (not answered by input: say so).

Evidence: into ~/Documents/Liminal HQ/Quill/GTK4/WP-GTK-1/: every dialog in light and dark at GDK_SCALE 1
and 2 (Find, Replace, Fonts, Tabs, Spelling, About, the question), the File menu opened by Alt+F with the
mnemonics underlined, the Open and Save As choosers under Weston (crop or use a scratch HOME so no real home
folder shows). Commit under docs/media/: gtk-dialog-replace-light.png, gtk-dialog-mnemonics-light.png,
gtk-dialog-open-weston.png and any dialog shot that is new since G2a.

Docs in the same change: docs/linux.md, a "Dialogs and keyboard (G3)" section (what each dialog does on
GTK, the seam, the filters, the keys, what was and was not proved) and the work list row G3 marked Built;
docs/progress.md, a "GTK4 dialogs (G3)" row; decisions.md, the next free LNX- rows for every choice you
made (the filter report, the mnemonic mapping, the seam); SPEC.md's Linux desktop row if its wording of the
dialogs changes. PR title suggestion: "Answer every dialog in the GTK client, with mnemonics".
```

### WP-GTK-2 System clipboard

```text
Package WP-GTK-2, System clipboard (G5). Branch feat/gtk-clipboard-g5. Start only when the parked block's
clipboard bridge (App::take_clip_request, copy_to, paste_from; docs/clipboard.md "How App asks") is on main;
if it is not, stop and report.
Read: docs/clipboard.md, decisions ARCH-3, ARCH-4, LNX-18, D188; crates/quill-clip; GTK's gdk::Clipboard and
gdk::ContentProvider docs for gtk4-rs 0.11.
Build: src/clip.rs implementing quill_clip::Clipboard. Copy commits the flavours into owned buffers and
installs a ContentProvider offering text/rtf, application/rtf, text/plain;charset=utf-8 (from Windows-1252
through quill-text) and image/bmp for a lone BMP picture. Paste reads the offered formats asynchronously
first (RTF, then image/bmp, then UTF-8 text), then hands paste_from a SliceSource; text from UTF-8 refuses
characters Windows-1252 lacks with the count in a toast. Serve the request in window.rs after each command.
Tests: unit tests over the buffers; under your own Xvfb, xclip reads after a copy and writes before a paste
(RTF, text, BMP), saved bytes compared; damaged RTF with text beside it pastes the text; emoji refused.
Evidence (GTK4/WP-GTK-2/): a paste from LibreOffice Writer keeping bold and colour (soffice in the same
Xvfb), the refusal toast. Docs: linux.md clipboard section (LNX-18 superseded), clipboard.md's GTK row,
progress row, LNX- rows. Must not touch: as WP-GTK-1, plus crates/quill-clip.
```

### WP-GTK-3a, 3b, 3c, WP-GTK-8 and WP-GTK-10

Their briefs are in [plans/gtk-print-help.md](plans/gtk-print-help.md#builder-briefs), the first (WP-GTK-3a Print Preview) in full.

### WP-GTK-4 libadwaita finish and dark mode

```text
Package WP-GTK-4, libadwaita finish and dark mode. Branch feat/gtk-adwaita-finish.
Read: decisions LNX-5, LNX-7, LNX-12, LNX-15, LNX-20; docs/linux.md "Not proved" lists of G2a and G2.
Build: follow adw::StyleManager's dark, accent and high-contrast state in the ruler, the selection colour on
screen (the paper stays white), focus rings and the desk; an adw::Breakpoint under 600 logical pixels that
moves the formatting bar's boxes into an overflow menu and drops the zoom box; F6 and Shift+F6 cycle the
focus page, toolbar, formatting bar, ruler, status bar; gtk-enable-animations off keeps the caret steady.
Run the window in a real GNOME compositor headless (mutter --headless --virtual-monitor, or gnome-shell
--headless; your own session bus; xdg-desktop-portal-gnome if present) and set the preferences through
gsettings in that session only: color-scheme, accent-color, high-contrast. If neither compositor starts,
fall back to Xvfb and say so. Tests: the colour choices and the breakpoint layout as data; F6 order; blink.
Evidence (GTK4/WP-GTK-4/): light, dark, high contrast, two accents, narrow, at scale 1 and 2.
Dark paper is WP-GTK-10's; build the colour wrapper for the accent so it can extend it.
```

### WP-GTK-5 Accessibility

```text
Package WP-GTK-5, Accessibility (G7). Branch feat/gtk-a11y-g7.
Read: decisions LNX-24, the R5 row of docs/plans/web-and-layering-addendum.md, winui/spike/src/s0b/child.rs
(the AccessKit node shape), GTK 4.14's GtkAccessibleText and how gtk4-rs 0.11 exposes it (verify; if the
binding is missing, implement through the raw interface in this crate and record it).
Build: src/a11y.rs, a snapshot of the visible lines as UTF-8 with per-character x in device pixels, caret,
selection, line bounds, role, name and a revision, built from the App and the view; the page implements
GtkAccessibleText from it (contents, caret position, selection, attributes for bold, italic, underline, font
and size), with update_caret_position, update_selection_bound and update_contents after changes; labels on
every toolbar, formatting bar and status control from command_tip; dialog rows labelled.
Tests: the snapshot over tests/docs at scales 1 to 2; tools/atspi-read.py (gi.repository.Atspi on a private
at-spi bus in your own Xvfb) reads the caret line and offset after keys and the selection after Shift+Right.
Manual Orca checklist in docs/linux.md. Evidence (GTK4/WP-GTK-5/): the AT-SPI dump. Do not create
crates/quill-a11y.
```

### WP-GTK-6 The page's own parity

```text
Package WP-GTK-6, spelling marks and header and footer editing on the GTK page. Branch feat/gtk-page-parity.
Read: docs/progress.md "Spelling as you type" (Win32 and DOS sections), crates/quill-app/src/spell_live.rs,
crates/quill-paint/src/page.rs (the wave and the story kinds), docs/linux.md G2 "Not yet live".
Build: src/spell.rs: a glib timeout 300 ms after the last key, click, scroll or command, spell_set_view from
the visible lines, spell_idle in batches of six words every 55 ms, repaint of the changed lines, stop when
done; a right click or Shift+F10 on a mark opens a popover with up to eight suggestions, Ignore All and Add.
Header, Footer, First Page Header and Footer: paint the story being edited with its caret and selection.
If quill-paint lacks what a header story with a caret needs, stop and report.
Tests: the marks equal F7's stops once the scan is done; the popover's choices change the document as the
Win32 menu does; header typing saves the host scenario's bytes. Evidence (GTK4/WP-GTK-6/).
```

### WP-GTK-7 and the WinUI packages

WP-GTK-7's brief is written by the lead when each item's shared part has merged, from its entry above and the common preamble. The WinUI briefs are in [plans/winui-real-window.md](plans/winui-real-window.md#builder-briefs): WP-WINUI-1 (S0c) in full, WP-WINUI-2 and WP-WINUI-3 ready to paste, the rest written by the lead when their start conditions hold.

## Owner decisions

Each is taken with the default so no package waits; the last column is where to change it.

| Question | Default taken | Where to swap |
|---|---|---|
| Start the GTK packages now, beside the parked block, or after it as the queue's section C says? | Start WP-GTK-1, WP-WINUI-1 and WP-WINUI-2 now: they touch only the clients' own folders and need nothing from the block | The "Starts when" column above |
| WinUI hosting: the Reactor window with an owned pop-up page (P2), or one Win32 window with DirectComposition and DWM Mica (Option B, which imitates WinUI's chrome)? | **Answered by the owner** (2026-11-27): a real WinUI window. WP-WINUI-1 measures route H1, with real-WinUI fallbacks only; Option B returns only by the owner's word after a no-go (WUI-85) | WUI-1, WUI-85; [plans/winui-real-window.md](plans/winui-real-window.md#owner-decisions) |
| Who builds the clipboard bridge in `quill-app`? | The parked block's "rich clipboard copy" item, before WP-GTK-2 | This page's "What the track needs" table; or add a WP-CORE-CLIP package to this track |
| GTK printing: raster pages through `gtk::PrintOperation`, or a text-layer PDF? | **Decided by the owner** (2026-11-27): a PDF with a text layer from `quill-pdf`, printed through `gtk::PrintDialog` (LNX-80, LNX-81) | [plans/gtk-print-help.md](plans/gtk-print-help.md#owner-decisions) |
| A dark paper option for display? | **Decided by the owner** (2026-11-27): yes, off by default, after WP-GTK-4 (WP-GTK-10, LNX-88) | [plans/gtk-print-help.md](plans/gtk-print-help.md#owner-decisions) |
| A help viewer on GTK (and later WinUI)? | **Decided by the owner** (2026-11-27): yes on GTK, an in-app window over the help source (WP-GTK-8, LNX-87) | [plans/gtk-print-help.md](plans/gtk-print-help.md#owner-decisions) |
| Reactor version for W3 onward | Replaced: neither Reactor 0.100.0 nor its master can make a custom automation peer or receive text composition, so the client uses Quill's own projection of the WinUI metadata; Reactor stays the spikes' (WUI-86) | WUI-86 and `winui/quill-winui/xaml` |
