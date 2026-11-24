# Plan: make the DOS dialog flows follow the Windows 3.1 and Windows 95 builds

## What I read

- **The code.** `origin/main` at 84a2e30, through the `quill16.wt-about-platform` worktree, which is at that commit with one uncommitted line in `lib.rs`. Paths below are relative to the repo.
- **The open PRs.** I read each one at the commit GitHub has for it: #19 d3e3f79, #20 0afd144, #21 8b60117, #26 a72273b, #27 bbc1bdc, #28 c87986f.
- **The PRs are stale and being rebased.** #19, #20, #21 and #26 are all based on ec95a54, which is #11 and comes before the Win9x stack. The local branches have since moved to other commits, so someone is rebasing them now.
- **#21 includes #20.** #21 carries #20's D122 and its `dos/rt` work.

## Five findings that shape the plan

1. **Most of the flow logic is copied into each Windows shell, not shared.** Each shell has its own copy of:
   - the unsaved-changes wording (`win16/examples/quill/main.rs:531-549` and `win9x/quill-win9x/src/dialogs.rs:449-467`);
   - saving before the command goes on, `save_current` (Win16 `dialogs.rs:559-569`, Win95 `main.rs:911-920`);
   - the "not found" and count message boxes;
   - the spelling-complete box;
   - the bad tab position message;
   - inch parsing (`parse_inches`);
   - the open and picture error strings.

   DOS has a third version of the same flows in the core's `Mode::Confirm` and `Mode::Prompt` (`crates/quill-app/src/lib.rs:1261-1299, 1691-1712, 1927-1980`). It uses different wording and has no default button.
2. **Bug on both Windows builds: F3 before any search.** `Command::FindNext` with no previous search calls `open_find`, which sets the text-cell `Mode::Find` even when native dialogs are on (`lib.rs:1508-1514`). The reason is that `native_request` (`native.rs:73-96`) has no case for it. Windows then sends keys to an invisible Find box.
3. **Bug on all three builds: a failed Save As still renames the document.** `save_as` sets the name before it saves (`lib.rs:517-525`).
4. **Bug on both Windows builds: wrong question for a recent file.** Choosing a recent file with unsaved changes asks "…before a new document starts?", because `Recent` falls into the default branch of both copies of the wording.
5. **#28 decides three things differently from Windows.** Windows lets the extension decide the format (`lib.rs:863-894`) and #28 uses the type instead. #28 also asks a text-format warning that no Windows build asks, and it starts in the document's folder where Windows starts in `last_folder()`.

---

## (1) Flow by flow

Classes: **F** is flow, **W** is wording, **C** is content (what the dialog shows).

**Rule where Win16 and Win95 differ: DOS follows Windows 95.** Windows 95 is where the Windows UI is built first (D34), and it is the build that implements what the SPEC asks (D101). The one exception is noted in its row.

### Files

| Flow | Win 3.1 | Win 95 | DOS today [verified in code] | Gap and fix | Class |
|---|---|---|---|---|---|
| **New** | If modified, asks "Save the changes to X before a new document starts?" with Yes/No/Cancel; Yes is the default. Then a fresh document | Same | If modified, a `Mode::Confirm` box "Save changes to X?" with no default button. Only Y, N and Esc work; Enter is dead | DOS uses the Windows wording and a message box with Yes as default (Enter, Tab, Y, N, Esc) | F, W |
| **Open** | COMMDLG Open: starts in `last_folder()`, first type Quill, the file must exist, remembers the folder. A read error puts "Quill cannot open that file." on the status line | Same, Explorer style | Typed path in a one-line box, upper-cased. Same error text. **Never calls `remember_folder`**, so DOS never updates `Folder=` | Use the `quill-dlg` file dialog. Start in `last_folder()`, remember the folder on OK, show "Quill cannot find X." inside the dialog | F, C |
| **Open recent** | Not in the menu. The list is kept and saved to `[Recent]` but never shown | Menu between Print Setup and Exit, as `&1 shortened path`, with the hint "Open this document". Asks about changes first. A missing file is dropped with "Quill cannot find X." (core) | Listed after Exit on `main`; #21 moves it before Exit. The core flow is the same | Fix the wording to "before another document opens" (core, for all three). Win16's missing menu is a Win16 follow-up | F, W |
| **Save** | `do_save` writes in the format the extension names. On failure, "Quill cannot save the file." on the status line | Same | Same core code. `DosFiles::close` always returns true (`dos/quill-dos/src/files.rs`) | Same flow already. Make `close` report errors in the DOS file-system PR | F |
| **Save As: name** | Empty name; first type Quill | Base name filled in; the type matches the extension | Empty box, at most 63 characters | DOS follows Win95: base name filled in and selected | F |
| **Save As: type change** | Nothing changes | Hook: the name stays and only a known extension is swapped; the default extension follows the type (`win9x/.../main.rs:775-825`) | No type list | Keep the name and swap the extension. `Rule::Any` is forced by 8.3, which has room for only one extension | F, C |
| **Save As: default extension** | `qld` always [not checked: does COMMDLG 3.1 follow the type?] | The type's extension | None | The type's extension (#26 `with_default_ext`) | F |
| **Save As: format** | By extension (core) | By extension | By extension | **Keep "by extension"**: #28's `chosen_format` must not override it | F |
| **Save As: overwrite** | COMMDLG asks; default button not checked | comdlg32 asks; default button not checked | None: overwrites silently | Ask with the Win95 wording. The default button must match Windows (decision 3) | F, W |
| **Save As: text warning** | None | None | None | #28 asks one. Recommend not asking until it goes into the core for all three (decision 4) | F |
| **Save As: case** | Upper case (`set_name`) | Kept as typed (`long-names`) | Upper case | No gap | — |
| **Save As: folder** | `last_folder()` | `last_folder()` | Current directory | `last_folder()`, then the current directory. Not the document's own folder as #28 does | F |
| **Save As: failure** | Status line, and the name has already changed (bug) | Same bug | Same bug | Fix in the core: rename only after a successful save | F |
| **Revert** | None | None | None | No gap | — |

### Close and Exit with unsaved changes

| Flow | Win 3.1 | Win 95 | DOS today [verified in code] | Gap and fix | Class |
|---|---|---|---|---|---|
| **Close and Exit** | The close box sends Exit. "…before Quill closes?" with Yes/No/Cancel, Yes the default | Same, plus Alt+F4 | Ctrl+Q and File > Exit; no Alt+F4 | Add Alt+F4. Use the Windows wording and box | F, W |
| **Yes** | Untitled: Save As, then the command. With a name: save, then the command | Same | Same, through `pending` | Same flow. One core path for all three (see part 2) | F |
| **Save As cancelled or save failed** | The command is dropped; on failure the status line says so | Same | Same | No gap | — |
| **No** | Discard the changes, then run the command | Same | Same | No gap | — |
| **Cancel or Esc** | Nothing happens | Same | Same | No gap | — |

### Insert Picture

| Flow | Win 3.1 | Win 95 | DOS today [verified in code] | Gap and fix | Class |
|---|---|---|---|---|---|
| **Insert Picture** | COMMDLG with the picture filters. On failure, "Quill cannot use that picture." on the status line | Same | Typed path, same message | File dialog of the Picture kind starting in `last_folder()` | C |

### Find and Replace (both Windows builds use template 1002)

| Flow | Win 3.1 and Win 95 | DOS today [verified in code] | Gap and fix | Class |
|---|---|---|---|---|
| **Find** | Modal box that stays open. Fields come from `last_find()`; Find Next is the default button | Stays open, Find Next is the default; no mnemonics and no Shift+Tab | Mnemonics from `dialogs.json`, Shift+Tab | C |
| **Not found** | Message box over the dialog: "Quill cannot find that text." The core also leaves `Cannot find "x".` on the status line | Status line only | A message box from a core notice (decision 5) | F, W |
| **Wrap** | Wraps once, silently. "Quill has finished searching the document." exists nowhere | Same | No gap; the SPEC differs from all three | — |
| **Find Next (F3)** | No box when nothing is found, only the status line. Bug before any search (finding 2) | Opens Find | Fix the bug in the core: F3 with no search requests `Find(false)` | F |
| **Replace** | Replaces the selection, **whatever it is**, then finds the next match | Same code | Fix in the core: replace only when the selection matches (all three) | F |
| **Replace All** | One undo step, then a message box with "Quill made N replacements."; the dialog stays open | Status line; stays open | Message box | F |

### Spelling (both Windows builds use template 1005)

| Flow | Win 3.1 and Win 95 | DOS today [verified in code] | Gap and fix | Class |
|---|---|---|---|---|
| **Start (F7)** | `spell_begin`. Nothing to fix, or no dictionary: a message box ("The spelling check is complete." or "The spelling dictionary was not found.") | Status line | Message box | F |
| **The dialog** | Not-in-dictionary word and context; a **Change to** field filled with the first suggestion that can be typed over; choosing a suggestion fills the field; Change does nothing when the field is empty; Change is the default button; Cancel calls `spell_cancel` | No Change to field; Change uses the selected suggestion and does nothing without one; Esc does what `spell_cancel` does | Add the Change to field and a Suggestions list with focus | C |
| **End** | The dialog closes, then a box says "The spelling check is complete." | Status line | Message box | F |
| **Suggest and Options** | Neither exists on Windows | Neither | No gap | — |

### Fonts

| Flow | Win 3.1 | Win 95 | DOS today [verified in code] | Gap and fix | Class |
|---|---|---|---|---|---|
| **Fonts** | `ChooseFont`: every face, size, effect and colour | Template 1003: system faces, size, Bold, Italic, Underline, Strikeout; **keeps the face when it is not in the list** | Three fixed faces. **Any other face is shown as Times New Roman and written over on OK** (`form.rs:153-155, 296-323`) | Follow Win95/1003: list the document's fonts and the defaults, select the current face, keep it when unchanged. Colour goes through the colour command, so Win16 is the exception here | F, C |

### Tabs (both Windows builds use template 1004)

| Flow | Win 3.1 and Win 95 | DOS today [verified in code] | Gap and fix | Class |
|---|---|---|---|---|
| **Tabs** | List of stops (choosing one fills the fields); position starts at "0.50"; kind and leader radios; Set is the default; Set, Clear and Clear All act at once; a bad position gives a box "Type a position in inches, such as 1.25."; a 17th stop puts "A paragraph holds at most 16 tab stops." on the status line | Empty position; no list; a bad position is **silently ignored** (`tab_edit` returns, `form.rs:405`) | Add the list and the 0.50 default, and the bad-position notice as a message box from the core | F, C |

### Paragraph and Document menu

| Flow | Win 3.1 | Win 95 | DOS today | Gap and fix | Class |
|---|---|---|---|---|---|
| **Numbering and Bullets** | A toggle with style 0; no dialog | Same | Same | No gap (the SPEC asks for a dialog that no build has) | — |
| **Header, Footer, First Page** | Document menu commands that switch the view; no dialog | Same | Same | No gap | — |
| **Page Layout, Indents and Spacing** | Not built | Not built | Not built | No gap (all three miss the SPEC) | — |

### Zoom, printing and print preview

| Flow | Win 3.1 | Win 95 | DOS today | Gap and fix | Class |
|---|---|---|---|---|---|
| **Zoom** | `main` shows a "Zoom" item that does nothing visible; #21 adds the submenu | View > Zoom works and is saved in the INI | View > Zoom sets a value it never shows and saves it; on `main` the View menu is not on the bar (`paint.rs:19-20`, fixed by #21) | Hide Zoom in the DOS text menu (the SPEC says DOS has no zoom), as #21 hides Preview and Setup | F |
| **Print** | `PrintDlg` with no ranges, then a box: "Printing is not in the Windows 3.1 build yet." | `PrintDlg` (printer, all or pages, copies), GDI printing; on failure "Quill could not print." on the status line | Prompt "Print to file or device"; "Quill cannot print to that file." | Follow Win95: a text Print box with destination (LPT1, LPT2, File plus a name), copies, all or pages from–to; then `print_text` with the range; one failure message from the core (decision 6) | F, C, W |
| **Print Setup** | `PrintDlg` setup page | Same | "That is not available in this version."; hidden by #21 | Hidden on DOS: no gap after #21 | F |
| **Print Preview** | "That is not available in this version." | Preview bar with keys: PgUp, PgDn, T, Z, P, Esc | Same message; hidden by #21 | Nothing replaces it on DOS (SPEC). If it is reached anyway, say "Print Preview is not in the DOS build." | W |

### Insert fields

| Flow | Win 3.1 | Win 95 | DOS today | Gap and fix | Class |
|---|---|---|---|---|---|
| **Page number, page count, file name** | The same core command | Same | Same | No gap | — |
| **Date and Time** | `main` gives no date; #21 adds the date (`datefmt`) | `GetDateFormat`/`GetTimeFormat` (D104) | **Empty**: the DOS shell never calls `set_print_date` or `set_print_time` | DOS shell: INT 21h 2Ah and 2Ch, the country format from 38h, through `datefmt` (#21) | F |

### Status line, About, Help

| Flow | Win 3.1 | Win 95 | DOS today | Gap and fix | Class |
|---|---|---|---|---|---|
| **Status hints** | None on `main`; #21 adds them | `command_hint` shows for the highlighted menu item | None | The DOS status line shows `command_hint` while a menu item is highlighted | F |
| **Modified marker and title** | `status_text` shows `Mod`; title "Quill - NAME" | Status pane "Modified"; title "Quill - NAME" | `Mod` on the status line | No gap (`Mod` is a width choice) | — |
| **About** | 1001: version, "Windows 3.x, Win16 on Windows 3.10", free memory; OK is the default | 1001: version, platform line, memory line | Box hard-coded to "Host test run" (`dos/quill-dos/src/main.rs:52`); #21 adds `dos_about` | 1001's content with the DOS version and DPMI free memory (#20 and #21); goldens keep a fixed line | C |
| **Help (F1)** | WinHelp `QUILL.HLP`. No F1 inside dialogs. The SPEC's missing-file message is not built | Same | "Help is in the Windows builds for now" | Keep it until the help viewer exists | — |

### Errors, settings, recent files, undo

| Flow | Win 3.1 | Win 95 | DOS today | Gap and fix | Class |
|---|---|---|---|---|---|
| **Error messages** | Open, save, picture and print errors go on the **status line**; find, count, spelling and tab notices are **message boxes**; COMMDLG failures show as "The dialog could not open (COMMDLG error N)." on the status line | Same, without the COMMDLG text | Everything on the status line | DOS copies the split exactly; the SPEC's "box for every error" is in no build | F |
| **Disk full and safe save** | No temporary file | Same | Same | No gap; a later core change for all three | — |
| **Settings** | Whole `QUILL.INI` written at exit | One key at a time after Chrome, Zoom, Recent, Open, Save and Save As, and at close | Whole file at exit; `Folder=` never written | DOS remembers the folder from its dialog. Saving after each file command is optional (decision 8) | F |
| **Recent-file details** | — | Four files, case-insensitive, `[Recent32]` | `[Recent]` | No gap after #21 puts the list before Exit | — |
| **Undo and Redo labels** | "Undo" and "Redo" | Same | Same | No gap (all three differ from the SPEC's `Undo Typing`) | — |

---

## (2) Making the DOS build use the same code path

### Where each flow lives today

| In the core (`quill-app`): DOS already shares it | In each Windows shell only (copied twice) |
|---|---|
| When to ask about changes (`native_request` and `Mode::Confirm` test the same condition twice); `pending` and `run_confirmed`; `do_save` running the next command | The question text (`confirm_text` and `confirm_save`) |
| `open`, `save`, `save_as`, `insert_picture`, `print_text` | `save_current` (untitled → Save As → save → command) |
| Recent list, missing file, `shorten_path`; `remember_folder` and `last_folder` | Save As name filled in, type from extension, type-change hook (Win95 only) |
| Find, Replace, Replace All and their status messages | The messages for "not found", the count and spelling complete; error strings for open and picture |
| The spelling engine and its messages; `tab_edit`; `set_font`; `set_colour` | `parse_inches` and the bad-position message |
| The status text, `command_hint`, menu tree, settings `each_setting` and `apply_setting` | Print flow (`print_job`); Print Preview; Print Setup; INI timing; About platform and memory lines |

### Design: one answering layer, one text shell

1. **`quill_app::flow`, a new module.**
   - Each shell shows one of two things:
     - `Ask { text, buttons: YesNoCancel | YesNo, default }` for a question;
     - `Notice { text, kind: Info | Warn }` for a message box.
   - The core produces the text for both. The shell only decides how to show it.
   - New entry points:
     - `confirm_question(c)` and `confirm_answered(s, f, c, Answer)`. On Yes with an untitled document, the core itself asks for `Dialog::SaveAs` with `pending = c`. This is what DOS's `Mode::Confirm` Y does today, so `save_current` goes away.
     - `dialog_cancelled(Dialog)`, which clears `pending`.
     - `save_as_start() -> {folder, name, type_index}`, built from `quill-dlg::savename`.
     - `save_as_chosen(path) -> Result<(), Notice>`, which renames only on success, remembers the folder and runs `pending`.
     - `open_chosen`, `picture_chosen`, `print_chosen(dest, copies, range)`.
     - `find_pressed(fields, action) -> Option<Notice>`.
     - `spell_started` and `spell_pressed(button, with) -> SpellStep::{Next, Done(Notice)}`.
     - `tab_pressed(text, kind, leader, button) -> Option<Notice>`, with the inch parser moved into the core.
   - Both Windows shells move onto these entry points, so their copies are deleted.
2. **Split the `native` flag in two.** Make it `dialogs_native` (requests go to the shell) and `menus_native` (F10 and the text menu off). DOS sets the first and not the second.
3. **`quill-tui`, a new portable `no_std` crate.** It depends on `quill-app`, `quill-dlg` and `quill-grid`.
   - `TextShell { dlg: Option<TextDlg> }` with:
     - `key(app, s, f, fs, k)`: sends the key to the open dialog or to `app.key`, then takes `take_dialog()` and builds the `TextDlg`;
     - `draw<ROWS>(app, s, g) -> Cursor`: draws the page with `quill_app::draw`, then the dialog over it.
   - One text-cell implementation per `Dialog` kind: `MsgBox` (questions and notices), `File` (`FileDlg` of #28), `Find`, `Fonts`, `Tabs`, `Spell`, `About`, `Print`. Each answers through the `flow` entry points above.
   - **Users:**
     - the DOS shell;
     - the host script player, so the goldens test exactly what DOS runs;
     - the Win32 grid view and Win16 `/PLAY` (see decision 7).
   - This is a state machine, not a nested modal loop. It keeps the DOS event loop flat for mouse polling later, and the host can drive it one key at a time.
4. **Delete the text-cell modes.** `Mode::Prompt`, `Mode::Confirm`, `Mode::Find`, `Mode::Form`, `Mode::Spell` and `Mode::About` leave `App`, one PR at a time, so `Mode` keeps only `Edit` and `Menu`.

### What this needs from the `quill-dlg` PRs

- **#26:** usable as it is. `savename::initial` must take the folder from `last_folder()`, not the document's own folder.
- **#27:** usable as it is. Every dialog above builds on `Field`, `List`, `Combo`, `Check`, `Radio` and `Focus`.
- **#28, before it merges:**
  - make `chosen_format` advisory, or sync the type to a typed known extension, so the extension decides the format;
  - put the text warning behind a flag, off by default;
  - use the Windows overwrite wording and default button;
  - start in `last_folder()`.
- **New for `quill-dlg`:** a `MsgBox` state (buttons, default, Y/N mnemonics, Enter, Esc, Tab, Left and Right).
- **Painters:** in `quill-tui`, one per dialog, with labels and `&` mnemonics read from `assets/dialogs/dialogs.json`.

### What moves into the core for all three builds

- the question wording, with the recent-file fix;
- the Yes → Save As chaining;
- every notice text;
- inch parsing;
- the Save As starting state (Win16 then gets the name filled in for free);
- the print range and copies for text printing;
- the fixes for the F3 bug and for renaming on a failed save.

### What stays in the shells

- the Win95 type-change hook (it calls `quill-dlg::savename`);
- `PrintDlg` and GDI printing;
- Print Preview;
- WinHelp;
- when the INI is written;
- the platform and memory lines.

---

## (3) Stacked pull requests

| PR | Base | Scope | How it is accepted |
|---|---|---|---|
| **F0** verify Windows (no code) | — | On the Windows 95 test bed and in Windows 3.11 under DOSBox-X: the overwrite prompt's text and default button, whether COMMDLG 3.1's default extension follows the type, Y/N in the unsaved-changes box | Screenshots in `docs/media`, a table in `progress.md`. **Can start now** |
| **F1** `feat/app-flow` | `main` (rebase over #21 if that lands first) | `flow.rs` with `Ask` and `Notice`, the entry points above, the F3 fix, rename-only-on-success, the recent-file wording, `dialog_cancelled`, inch parsing in the core. Keep `lib.rs` and `native.rs` edits to the hooks | New tests in `tests/native.rs`: every question's text and default button; untitled Yes → SaveAs request → cancel clears `pending`; a failed Save As keeps the old name (fails before the fix); F3 before any search gives `Find(false)` with the mode staying `Edit`. `make validate`. **Can start now** |
| **F1w** Win95 shell uses `flow` | F1 | Delete `save_current`, `confirm_save`, the notice strings and `parse_inches` from `win9x` | `wine-dialogs.sh`, plus a hand check on Windows 95: the same boxes and texts as before, except the recent-file wording |
| **F1x** Win16 shell uses `flow` | F1, #21 | The same for `win16`, plus Save As with the name filled in (plain `lpstrFile`) | Windows 3.11 hand run; segment budget passes |
| **F2** `test/flow-scripts` | F1 | A `.flow` format at the level of commands and answers (`type`, `cmd`, `expect ask "…" default Yes`, `answer No`, `expect file SaveAs name=… type=…`, `choose PATH`, `expect notice "…"`). A `NativeStub` runner that answers `take_dialog()` through `flow`, as the Windows shells do. Scripts for New, Open, Save, Save As, confirm on Exit, recent, Find, Replace All, spelling, tabs and fonts. Compares the dumped document, saved files, `name`, `modified`, recent list, `last_folder` and the log of questions and notices | Scripts pass against the stub and become the record of how Windows behaves. **Can start now** on F1 |
| **F3** `feat/dlg-msgbox` | #28 | `quill-dlg::msgbox`, plus the #28 changes listed in part 2 | Property tests (focus stays on a button, Enter presses the default, Esc means Cancel or No). **Can start now** |
| **F4a** `feat/tui-confirm` | #21, F1, F3 | The `quill-tui` crate, `TextShell`, the flag split, the `MsgBox` painter. DOS answers `ConfirmSave` and the notices. Remove `Mode::Confirm`. The script player goes through `TextShell` | F2 gets a `TextShell` runner that turns answers into keys; it must give the same state log as `NativeStub` for every script. Goldens regenerated and reviewed: `m2-dialogs`, `m2-find`, `m2-headers`, `m2-lists`, `m2-mono`, `m4-print-headers`, `m8-spelling` |
| **F4b** `feat/tui-file` | F4a, #28 | The file dialog painter; Open, Save As and Picture through `TextShell` and `flow`. Remove `Mode::Prompt` for those three. `MemFiles` gets `FileSystem` (or reuses `FakeFs`) | Flow scripts for name filled in, type swap, overwrite No and Yes, bad name, `..`, `D:`. New `m10-open` and `m10-save-as` goldens; `m2-letter`, `m7-picture` and `m4-about` updated |
| **F4c** `feat/dos-fs` | F4b, #20 | `DosFiles` implements `FileSystem`; `close` reports errors; DOS e2e plays `.keys` generated from `.flow` | DOSBox Staging: frames match the host's `TextShell` run character for character, saved files are byte-identical, the final state dump is the same |
| **F5** Find and Replace | F4a | `TextDlg::Find` per 1002 with notices as boxes; remove `Mode::Find`; replace only a matching selection (core) | `m2-find` regenerated; flow scripts for not found and the count |
| **F6** Spelling | F4a | Per 1005 with a Change to field and a suggestion list; start and end notices as boxes; remove `Mode::Spell` | `m8-spelling`; flow script ending in the complete box |
| **F7** Fonts and Tabs | F4a | Fonts per 1003 (the face is kept); Tabs per 1004 (list, 0.50, bad-position box); remove `Mode::Form` | `m2-dialogs`; a test that OK leaves an Arial selection as Arial (fails today) |
| **F8** About and Print | F4a, #20, #21 | About per 1001 with the DOS version and DPMI memory (fixed line in goldens); the Print box (destination, copies, range) with `print_text` taking a range and copies; remove `Mode::About` and the last prompt | `m4-about`, `m4-print`; byte check of a printed range |
| **F9** DOS status and menus | #21 | Menu hint on the status line; Date and Time fields (INT 21h 2Ah, 2Ch, 38h); Alt+F4; Zoom hidden in the DOS text menu | Host goldens for a highlighted menu item; `fields` test with a fixed date |

**Order:**

- **Start now:** F0, F1, F2, F3, and the #28 changes.
- **Then:**
  1. Rebase and merge #20 and #21; #19 can go any time (`TextShell` is generic over the row count).
  2. F1w, F1x, F9.
  3. F4a → F4b → F4c.
  4. F5 to F8 in parallel, each on F4a.

---

## (4) Decisions for you (recommended default in **bold**)

1. **Follow Windows 95** where Win16 and Win95 differ. The exception is Fonts, which uses 1003 with no colour.
2. **Format from the extension** (Windows), not from the type (#28).
3. Overwrite default button: **whatever Windows 95 shows (F0)**. Until then, No as in #28.
4. Text-format warning: **not asked on any build for now**; later, add it to the core for all three.
5. "Not found" wording: **the Windows box text "Quill cannot find that text."** on all three, and the core's status line message is dropped.
6. Print failure wording: **"Quill could not print."** on all three.
7. Win16 `/PLAY` and the Win32 grid view: **link `quill-tui` and measure the segment cost**. If the budget fails, play back without dialog frames and compare files only.
8. When DOS writes the INI: **at exit, plus after each file command** as Win95 does.
9. Zoom in the DOS menu: **hidden**.
10. Decision numbers: **renumber before merging**. #21's D97–D101 clash with `main`'s D97–D105, and the PRs reserve D100–D119 for Linux, which `main` already uses.

## Risks

- **Golden churn.** Every step from F4a to F8 rewrites screen goldens, and so does #21. Nothing that changes screens should land before #21.
- **Win16 size.** `flow` and `quill-tui` add code to Win16's segments (decision 7).
- **Path length.** `NAME_MAX` is 64 while a DOS path can be 79 bytes; F4b must refuse longer paths or raise the limit.
- **Moving branches.** The PR branches are being rebased right now, so the line numbers in this plan may move.
- **DOSBox AUTOTYPE limits.** It cannot hold Shift or Alt, so the generated `.keys` must use only single keys for the text dialogs. Mnemonics without Alt only in message boxes; Tab navigation elsewhere.

## Checked and not checked

- **Checked by reading the code:**
  - every Windows and DOS behaviour stated in part 1 for `main`, and the core paths behind them;
  - the `dialogs.json` default buttons: OK, Find Next, OK, Set, Change;
  - which goldens contain the unsaved-changes box;
  - the API surfaces of #26 to #28;
  - #21's DOS File menu and where it puts the recent files.
- **Not checked:**
  - I ran nothing: no builds, no tests, no emulators;
  - the overwrite prompt's wording and default button on either Windows;
  - whether COMMDLG 3.1's default extension follows the type;
  - the rebased PR branches;
  - #21's Win16 runtime claims beyond its description.

### Critical files for implementation

- /home/scott/source/experiments/quill16.wt-about-platform/crates/quill-app/src/native.rs
- /home/scott/source/experiments/quill16.wt-about-platform/crates/quill-app/src/lib.rs
- /home/scott/source/experiments/quill16.wt-about-platform/win9x/quill-win9x/src/main.rs (`file_dialog`, `save_hook`, `handle_dialogs`) and `win9x/quill-win9x/src/dialogs.rs`
- /home/scott/source/experiments/quill16.wt-about-platform/win16/examples/quill/dialogs.rs
- /home/scott/source/experiments/quill16.wt-dlg-filedlg/crates/quill-dlg/src/filedlg.rs (PR #28)
