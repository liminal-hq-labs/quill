# Progress

What is built, how it was shown to work, what is blocked and what was assumed. Updated with every merge. Behaviour is in [SPEC.md](../SPEC.md), the order in [roadmap.md](roadmap.md), the choices in [decisions.md](decisions.md).

## Table of contents

- [Status](#status)
- [Evidence](#evidence)
- [Blocked](#blocked)
- [Assumptions](#assumptions)

## Status

| Step | State | Notes |
|---|---|---|
| M7 Pictures | Started | `quill-inflate` (streaming deflate, 32 KB window, checked against real zlib streams) and `quill-image`: BMP (1, 4, 8, 24 bits, both directions; RLE and 16 or 32 bits refused) and PNG (every colour type and depth, all filters, split IDAT; interlaced refused) decoded by rows, a box-filter row shrinker and a 16-colour ordered dither with a hashed golden. Pictures are now in documents: a picture table and pool (D77), picture boxes in layout, `PICT` in `.QLD`, Insert > Picture for BMP and PNG, and the DOS placeholder box (host, DOSBox and Wine scenario `m7-picture`). Not yet: RTF `\pict` (so a picture is lost on saving as RTF), display on the Windows builds, printing of pictures, the 4 MB memory test, PNG checksums |
| M4 Win32 complete | Partly | About box, recent files and `QUILL.INI` in every build; plain-text printing with headers, footers and form feeds (DOS, and the same code in the Win32 text view), matched on the host, DOSBox and Wine. Not done: GDI printing and preview, the native print and page-setup dialogs, the PostScript and Wine PDF page comparisons |
| M3 Win32 editor | Done, with gaps | `QUILL32.EXE` draws a GDI page with real fonts, a native menu loaded from a `.res`, common Open and Save As dialogs; on Windows ME the host's `CapturedMeasure` breaks the same lines (D73); the text-grid view still matches the host on Wine and ME. Gaps: justified text is not spread, own Find and Fonts dialogs (D74), no scroll bar or mouse |
| M2 Rich core on DOS | Done except the mouse | Formats with selection, tab stops with leaders, overtype, both attribute schemes, lists, headers and footers with fields, find and replace, the Fonts and Tabs dialogs, a text clipboard, and RTF read and write checked in LibreOffice; every scenario matches the host under DOSBox. The mouse and the WinOldAp clipboard are not built (AUTOTYPE cannot drive them) |
| M1 DOS text editor | Done | `QUILLDOS.EXE` opens, edits and saves a document; its screens and saved file equal the host's |
| T1 Register probes | Done for Windows 3.11 at the fast profile | `win16/` copied from `rust-win16` with provenance; two-task probe program; findings below. Wine, the 386SX profile and Windows ME not run |
| T4 DOS foundation | Done | `dos/` copied with `dos/PROVENANCE.md`; `QUILLDOS.EXE` draws the `quill-grid` hello screen and `dos/tools/e2e.sh` compares it with the host golden |
| M0 Foundations | Done on the host, with two items open | The workspace, `Store` and `FarVec`, the piece table with undo, the document, layout, the `.QLD` reader and writer, `quill-render`. The fuzz smoke job is not set up (needs nightly and `cargo-fuzz`); the property tests include damaged-file and random-byte inputs in its place |

## Evidence

### M0

- `make validate` passes: format, clippy with the portable-crate lint set, 16 tests under `cargo nextest`, headers, float ban.
- Property tests against a `Vec` and a `String` model run with 8-byte and 96-byte chunks, so every few operations cross a chunk edge: after any edit sequence the text and both format layers match the model, undoing every step reaches each earlier state, redoing reaches each later one, and an out-of-memory failure leaves the text as it was.
- `tests/docs/plain.qld` is built by `quill-host`'s `make-docs`, read back, laid out with `GridMeasure` and compared with `tests/golden/layout/plain.grid.txt`; read then write gives the same bytes.
- `tests/golden/pages/plain.grid.png` is the page map (see D61).

### T4

- `make dos` (or `dos/tools/e2e.sh`) builds `QUILLDOS.EXE` (about 34 KB, CWSDPMI inside) with the pinned toolchain and no C compiler, runs it under headless DOSBox Staging 0.83.0 at `cputype = 486`, and compares the screen it read back from video memory (B8000h) with `tests/golden/screens/hello.80x25.txt`: identical, characters and attributes.
- The DOS side needs `-Zbuild-std` through `RUSTC_BOOTSTRAP=1`, as Desk does; it is not part of `make validate`, which stays host-only.

### M2 (partly)

- **RTF.** `quill-formats/src/rtf.rs` reads the subset (characters, paragraphs, tab stops with kinds and leaders, page setup, font and colour tables, `\'hh`, `\uN`, the named punctuation) and skips every other destination; it writes the canonical subset. `tests/rtf.rs` checks the hand-written `tests/docs/letter.rtf`, that write, read, write is stable, tolerance of unknown words and destinations, refusal of non-RTF and of control words over 32 characters, and 300 damaged or truncated variants of the letter for panics and leaks.
- **LibreOffice.** `scripts/check-rtf-libreoffice.sh` reads `letter.rtf`, writes it with Quill, converts both to HTML with `soffice --headless`, and requires the same text and the same counts of bold, italic, underline, strike, superscript, subscript, centred, right, justified, red text, Arial and Courier runs. It passes (3 bold, 1 of each of the others).
- **Editing.** Selection with Shift or F8, typing over it, Character menu (Regular, Bold, Italic, Underline, Strikethrough, Superscript, Subscript, with toggles over mixed text), Paragraph menu (alignments, spacing), overtype, undo and redo of all of it, tab stops with left, centre, right and decimal kinds and dot, hyphen or underline leaders drawn in the grid, colour and mono schemes (`/MONO`).
- **Screens.** `tests/scenarios/m2-letter.keys` and `m2-mono.keys` open the letter, format it from the menus, overtype, undo, save as RTF and exit; `dos/tools/e2e.sh` plays them in DOSBox and compares every screen with `tests/golden/screens/m2-letter.frames.txt` and `m2-mono.frames.txt`, and the saved `OUT.RTF` with `tests/golden/files/m2-letter.rtf`.

`docs/media/m2-letter-dos-colour.png` is a DOSBox-X screenshot of `QUILLDOS.EXE LETTER.RTF` (the documentation plan's scene 12): bold bright white, italic cyan, underline magenta, strike dark grey, raised and lowered text brown, tab leaders, a hanging indent and `▒` where CP437 has no curly quote.

Missing from M2 as the roadmap states it: numbered and bulleted lists, headers and footers, find and replace, the Tabs and Fonts dialogs, the mouse, and the DOS clipboard. The RTF reader skips list and header groups, so such files open with their text and character and paragraph formats.

### M3 (started)

- **Wine.** `win9x/tools/wine-e2e.sh` builds `QUILL32.EXE` (about 116 KB, Pentium target, `core` built from source) and plays seven scenarios into it under Wine on Xvfb with `/PLAY` and `/DUMP`; every screen equals the host's golden frames and the two saved files equal the host's bytes.
- **Windows ME.** On the VirtualBox VM (snapshot "before quill m3" taken first, audio driver none, restored afterwards) the same program was started from the Run box off the shared folder with `m1-edit.keys`: its 35 screens equal the host's golden frames and the saved `.QLD` is byte-identical. `docs/media/m3-letter-windows-me.png` is a screenshot of the letter on that VM.
- **GDI page.** The default view paints the laid-out document with GDI fonts (`win9x/quill-win9x/src/gdi.rs`): per-run fonts with bold, italic, underline and strikeout, colour, raised and lowered text, tab leaders, list labels, selection and caret. Screenshot of it on Wine and Windows ME: `docs/media/m3-gdi-letter-windows-me.png`.
- **Metrics.** `/CAPTURE` on the ME VM wrote `tests/metrics/screen96-windows-me.txt`; `/LINES` wrote the layout dump of `letter.rtf` there; `captured_metrics_lay_the_letter_out_as_windows_me_did` shows the host's `CapturedMeasure` produces the same dump, line for line.
- **Resources.** `emit-resources` writes `win9x/resources/quill.json` from the app's menu tree; `tools/res_pack.py` writes the `.res` (menu, icon, version information); `tools/test_res_pack.py` reads it back independently and `llvm-cvtres` accepts it; `lld-link` links it and the program loads its menu from it.
- **Checks.** `tools/scan-pentium.sh` finds no MMX, SSE or CMOV in the executable; `tools/check-imports.sh` holds the 53 imports to `tools/imports.allow` and refuses `...W` entry points. The allowlist is hand-checked, not read from a Windows 95 image.
- **Icon.** `assets/icon/` holds the Q-and-nib icon (D75): SVG masters, PNGs from 16 to 512 px, a 1280 by 640 social preview and `QUILL.ICO` (16, 32 and 48 px at 4, 8, 24 and 32 bits, 256 px as a PNG; D76), all written by `tools/make_icons.py` and checked by `tools/test_icons.py`. `res_pack.py` now takes every image in `QUILL.ICO` for the Win32 `.res` (13 `RT_ICON` resources and the `RT_GROUP_ICON`); `lld-link` put them in `QUILL32.EXE`, which was not run on Windows 95, 98 or ME with the new icon.
- Not done: justification in the GDI painter, printing and preview (M4), a scroll bar, the mouse.

### M1

`dos/tools/e2e.sh` builds `QUILLDOS.EXE` (about 96 KB), copies `tests/docs/plain.qld` to a scratch drive and types `tests/scenarios/m1-edit.keys` into headless DOSBox Staging with `AUTOTYPE`: F10, File > Open, the name, text typed at the start of the centred title, Enter (splitting the paragraph), cursor keys, a letter and a Backspace, File > Save, File > Exit. The program writes every screen it shows, read back from B8000h, and the run is compared with the host's play of the same keys through the same `quill-app` code:

- all 35 frames equal `tests/golden/screens/m1-edit.frames.txt`, character for character and attribute for attribute;
- the saved `PLAIN.QLD` is byte-identical to `tests/golden/files/m1-edit.qld`.

What M1 does not have yet: selection, overtype, find, formats beyond what a file carries, the other menus (the bar shows them, only File and Edit open), the ruler, and the temporary-file save (D66).

### T1

Run with `win16/tools/run-w31.sh` (DOSBox-X 2025.02.01 in Docker, Xvfb, `machine=svga_s3`, `cputype=pentium`, Windows for Workgroups 3.11 in 386 Enhanced mode, `GetWinFlags=00000C29`). Logs: `docs/evidence/t1-probe-parent.log` and `t1-probe-child.log`. A second copy of the program, started with `WinExec`, yields in a loop with other patterns while the first runs every case.

- **A task switch leaves the other task's high halves in EBX, ESI, EDI and EBP.** Raw calls to `Yield`, `PeekMessage`, `GetMessage` and `DialogBox` returned `ESI=6666BE02` to a task that loaded `2222BE02`: the low half is restored, the high half belongs to the task that ran meanwhile. The child lost all four high halves about 150,000 times in 326,000 rounds. Through the runtime's import stubs, which push and pop the full 32-bit EBX, ESI, EDI and EBP, there were 0 changes in the parent's five cases (twice, with FS and GS set and clear) and 0 in the child's 163,000 stubbed rounds. So the stubs are required and enough; every exported-procedure wrapper must save the four registers too (the template's `export_proc!` does).
- **EBX is scratch in every API,** low half included (`GlobalAlloc`, `PeekMessage`, `GetMessage`, `DialogBox`, `SendMessage` all return a different EBX).
- **FS and GS do not survive** calls that enter the memory manager or the message loop (`GlobalAlloc`, `PeekMessage`, `Yield`, `GetMessage`, `DialogBox` give 0). `GetVersion` and a 12-million-iteration busy loop under timer interrupts keep them. So the far-memory helpers load FS or GS inside each helper and never hold them across a call, as `toolchain-needs.md` says.
- **ESP's high half becomes 0x8001** after the first message-loop call and stays there. It is harmless on a 16-bit stack and the stubs clear it (`movzx esp, sp`).
- **`SendMessage` to Program Manager** (another task, 16-bit code) leaves ESI, EDI and EBP alone: only a target task that uses the high halves leaks.
- **Jump tables:** a four-way `match` compiled to `jmp [ebx*4+0x10]` and its entry landed on `0001:11D0`, the middle of a `call`, which Windows reported as an illegal instruction. Jump tables are off in `win16/.cargo/config.toml` until the cause is found (the table is in the data segment while its entries are code offsets; the packer or the linker script is the suspect). T2 should look at it before multi-segment code, since a jump table inside a segment is exactly the case the plan calls safe.
- A screenshot of the fault dialog on a real Windows 3.11 desktop is in the scratch directory only; the committed screenshots start at M5.

## Blocked

- **Pending: the Windows ME VM.** The owner is using it interactively, so no further VM run was started. Waiting on it: the GDI print and preview checks (PostScript to a file, rendered by Ghostscript), the About box and print scenarios on ME, and the justification screenshot. The VM was last left powered off, restored to the snapshot "before quill gdi" (taken before the GDI tests), audio driver back at its original value (default).
- **T2 (multi-segment code).** Not started: the veneer pass, the linker script generator and the indirect-call table are a project of their own. The jump-table fault could not be reproduced with synthetic code in `win16/` (LLVM chose compare chains for every `match` tried), so its cause is still unknown; `win16/tools/check_branches.py` fails the build if a jump table ever appears. Because T2 and the 64 KB code segment come first, **M5 and M6 (Win16 bring-up and the full Win16 editor) are blocked on T2.**

Nothing is blocked by the machine. These were not started, in roadmap order, for lack of time in this run:

- **The rest of M3 and M4 (Win32).** Needs T3 first: `rust-win9x` copied from `~/source/win9x-rust` with provenance, the `.res` writer, the import allowlist and the instruction scan, then the Windows ME VM and Wine harnesses. `quill-app`, `quill-heap` and the file formats are ready for it; only the platform layer is missing. The ME VM rules in the task (snapshots, audio none, no builds inside) still apply when it is used.
- **M5 to M6 (Win16).** T0 and T1 are done: `win16/` has the runtime, packer and a Windows 3.11 harness (`win16/tools/run-w31.sh`). Icons from JSON are done (`ne_pack.py` writes `RT_GROUP_ICON` and `RT_ICON` from an `.ico`; `win16/examples/icontest` shows them in Windows 3.11 in 16-colour, 256-colour and 64K-colour video, see `docs/media`). Still to do: resources from JSON (menus, accelerators), the far-memory `Store`, the window procedures, T2's multi-segment code, and the jump-table fault (D64).
- **M7 to M11.** Pictures, spelling, help, DOS graphics mode and the release disks.
- **T1 under Wine and on Windows ME, and at the 386SX profile:** not run.
- **T2, T3, T5 and the help compiler track (H0 to H3):** not started.
- **CI:** the new workflow jobs (format, clippy, tests) have never run on GitHub; nothing was pushed. The DOS and LibreOffice checks are local (`make dos`, `scripts/check-rtf-libreoffice.sh`).
- **Fuzzing:** no `cargo-fuzz` targets or 60-second CI smoke; property tests with damaged and random inputs stand in.
- **Panic-site count:** not implemented (D19).

## Assumptions

D58 to D69 in [decisions.md](decisions.md) were made while implementing; each stands until the owner confirms or changes it.
