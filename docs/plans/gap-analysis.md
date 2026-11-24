# Quill gap analysis, per build (read-only; nothing written)

## 0. What I read, and the in-flight work

- **Core and Win 95.** `quill16.wt-win9x-editing`, branch `fix/win9x-caret-zoom2`, HEAD e433311 (PR #15). Its working tree has uncommitted edits, so line numbers can drift. Win 95 here does not include `feat/m9-help` (PR #7).
- **Win 3.x.** `quill16.wt-polish`, branch `feat/win16-polish`, HEAD 85860fe. It is `feat/win16-gui` (PR #11) plus a merge of the editing core. It has no PR.
- **DOS.** `dos/quill-dos` (307 lines in total) plus the core's text view.
- **Uncommitted work, so nothing here is merged or in a PR:**
  - `wt-polish`:
    - Menu mnemonics (`&` in every label), `item_for_mnemonic`, and `Key::Alt` with F10 on the text menu.
    - `crates/quill-app/src/bios.rs`, which decodes DOS Ctrl keys from scan codes.
    - `crates/quill-host/tests/menus.rs`.
    - Win16 tooltips (`win16/examples/quill/tips.rs`) and menu separators (`MF_SEPARATOR` in `win16/examples/quill/main.rs`).
    - Scroll bars as child controls.
    - The `win9x/resources/quill.json` menu regenerated with mnemonics.
  - `wt-win9x-editing`:
    - A `Time` field (`Command::Field(5)`).
    - `refresh_clock` calling `set_print_date` and `set_print_time` (`win9x/quill-win9x/src/ui.rs:~966`).
    - `field_digits` in `crates/quill-core/src/layout.rs` giving Date, File Name and Time proper width.
- **PRs.** #15 → #13 → #12 → #6 (Win 95 stack). #11 → #8, and #8 sits on #7 (Win 3.x and help). #5 (play harness), #10 (file magic) and #4 (site) are unrelated.

## 1. Feature table

Legend: done · partial (how) · stub · missing · **IF** = in flight. Linux column omitted (n/a).

| Area / feature | DOS | Win 3.x | Win 95 |
|---|---|---|---|
| File: New, Open, Save, Save As, Exit | partial: single-line prompt (§1a) | done (COMMDLG) | done |
| File: recent files in menu | partial: core list, shown in the text menu | missing: no recent code in the Win16 shell | done (`[Recent32]`) |
| File: Print | partial: prompt for a file or device, plain text (`lib.rs:1902`, `print.rs:79`) | **stub**: dialog, then "Printing is not in the Windows 3.1 build yet." (`dialogs.rs:789`) | done under Wine; no Cancel dialog (progress.md) |
| File: Print Preview | stub: menu item prints "That is not available in this version." (`lib.rs:1450`). SPEC says DOS has none, so the item should be hidden. | stub: same message | done (keys plus preview bar) |
| File: Print Setup | stub: same message | done (COMMDLG `PrintDlg` setup page) | done |
| `/P FILE` | missing (only `/mono` and a file name, `dos/quill-dos/src/main.rs:55`) | missing (`/PLAY /DUMP /MONO /FAULT` only, `main.rs:~1367`) | done |
| Edit: Undo, Redo, Cut, Copy, Paste, Select All | done (internal) | done (internal) | done (internal) |
| Windows clipboard (RTF, DIB, CF_TEXT) | missing (and no WinOldAp) | missing | **missing**: no `OpenClipboard` anywhere in `win9x/`, `win16/` or `dos/`. The core clipboard is a 2048-byte, text-only buffer (`lib.rs:371`, `form.rs:415`). |
| Edit: Undo/Redo names, greyed items | missing | missing | missing (no `EnableMenuItem`; only View items are ticked, `main.rs:~1264`) |
| Edit: Delete, Size Picture | missing | missing | missing |
| Insert: Picture | partial: placeholder box, as SPEC | partial: **grey box** (`gdi.rs:328-333`, header comment line 7) | done (no selection or sizing) |
| Insert: Page Number, Page Count | partial: `{P` / `{N` marker in the body | **missing**: `gdi.rs:328` returns at `0x01` without drawing fields | done (body shows value) |
| Insert: Date, File Name | partial: `{D` / `{F` marker in the body, empty on print | **missing**: same code | **Date: bug, IF**. Committed code never calls `set_print_date` (`print.rs:33`; zero callers in `win9x/`, `win16/`, `dos/`). The value is empty, nothing is drawn, and the field was only two digits wide. The fix is uncommitted in `wt-win9x-editing`. File Name was not tested (PR #13). |
| Insert: Page Break (Ctrl+Enter) | missing | missing | missing (no menu item, key or Command) |
| Find, Find Next, Replace | done (text box) | partial: modal custom dialog | partial: modal custom dialog (D74), not common modeless |
| Spelling (F7 dialog) | done | partial: dialog exists, "pending" | done |
| As-you-type spelling and its menu item | missing | missing (SPEC: off by default) | missing (SPEC: on by default) |
| Spelling Options (language) | missing | missing | missing (INI only, and under `[Quill]` not `[Spelling]`, `lib.rs:660`) |
| Character: Regular, B, I, U, Strike, Super, Sub | done | done | done |
| Character: Fonts... | partial: text form | done (`ChooseFont`) | done |
| Character: Colour... menu, Reduce/Enlarge Font | missing | missing | partial: toolbar colour list only; Reduce/Enlarge missing |
| Paragraph: Left/Centre/Right/Justify, spacing, Bullets, levels, Tabs | done | done | done |
| Paragraph: Normal | missing | missing | missing |
| Paragraph: Numbering... | stub: toggles style 0 only (`lib.rs:1344`); no dialog, styles, start or restart | same | same |
| Paragraph: Indents and Spacing... | missing | missing | missing (ruler drag only) |
| Justified text spread | n/a | unverified | missing (progress.md "gaps: justified text is not spread") |
| Document: Header/Footer | partial: four View items, no distance, Clear or field buttons | same | same |
| Document: Page Layout... (margins, units, start page) | missing | missing | missing |
| Document: Toolbar/Ruler/Status Bar | partial: toggles exist but no ruler row in text mode | done | done |
| View: Zoom | n/a | missing (no zoom code in `main.rs` or `gdi.rs`) | done |
| Help: Contents / F1 | missing (no viewer, no `.HLX`) | done (committed, `main.rs:~622`) | **IF** #7 (not in this branch; Help has only About, `paint.rs:116`) |
| Help: Search for Help On, How to Use Help | missing | missing | missing |
| Help: F1 in a dialog or on a menu item, `HELP_CONTEXT` | missing | missing; `HELP_CONTEXT` unverified (progress M9) | same |
| About box | partial: `set_about("Host test run")` (`dos/quill-dos/src/main.rs:52`); no memory or resources line | done; free memory and resources not checked | done; not checked |
| Menu mnemonics, Alt and F10 | **IF** (uncommitted `wt-polish`) | **IF** (uncommitted) | **IF**: committed `quill.json` has no `&`. F10 and a bare Alt tap likely work via `DefWindowProc` (not tested). |
| Menu separators | done (D91) | **IF**: committed `main.rs:1192` has no `MF_SEPARATOR` | done |
| Accelerators: Ctrl+1/2/5, Ctrl+=, Shift+Del/Ins, Ctrl+Ins, Alt+Bksp | partial (`bios.rs` IF) | **missing**: `map_vk` (`main.rs:651`) has no `accel::from_vk`. Ctrl+Backspace is read as Ctrl+H, so it opens Replace. | done (`main.rs:358`) |
| Word and paragraph movement, Ctrl+Bksp/Del word, Shift+Enter, Alt+Shift+Left/Right, Ctrl+Shift+,/. | missing | missing | missing (no `Key` variants or handlers, `lib.rs:50-95`, `1729-1830`) |
| Status bar | partial: row 25, `Pg N Ln N Col N`, no "of M" | partial: GDI strip with the same text; no panes | **partial: a DOS strip.** The 25th row of the text grid is drawn in `OEM_FIXED_FONT` (`main.rs:1527-1528`); there is no `msctls_statusbar32`. The text is `Pg/Ln/Col`, not "Page N of M". |
| Toolbar, formatting bar, ruler | n/a | done (mouse tab-stop drag missing) | done |
| Tooltips | n/a | **IF** (uncommitted `tips.rs`) | done (COMCTL32 4.00) |
| Mouse: click, drag, Shift+click | missing (no INT 33h code in `dos/`) | done | done |
| Mouse: double, triple, margin click | missing | missing (no `WM_LBUTTONDBLCLK`) | done |
| Drag and drop of text | missing | missing | missing (zero hits in `pointer.rs` and `ui.rs`) |
| Picture select, frame, resize, Size Picture | missing | missing | missing (no picture hit-testing in `pointer.rs`) |
| Mouse wheel | n/a | n/a | partial: `WM_MOUSEWHEEL` (`main.rs:450`) proven under Wine only. The `MSWHEEL_ROLLMSG` path (`main.rs:1965`) is from docs, untested on 95. |
| Scroll bars | n/a | done (child controls are IF) | done |
| Settings (§1b) | partial | partial; read from the current directory, not the Windows directory | partial (profile calls, `[Recent32]`) |
| Save safety and error messages (§1c) | missing | missing | missing |
| RTF, QLD, text read and write | done | done | done |
| WRI import, JPEG | later | later | later |
| DOS `/G` graphics mode, `/50`, QUILL.FNT | missing (M10 not started) | n/a | n/a |
| Installer, release disks (`installer/`, `.QPK`) | missing (M11) | missing | missing |

### 1a. DOS file dialog quality

`key_prompt` (`lib.rs:1902-1957`) is one text line. Specifically:

- It accepts only characters above 0x20, so no spaces, and forces upper case.
- There is no cursor movement or editing beyond Backspace.
- The name field is 64 characters (`NAME_MAX`).
- Save As starts empty rather than with the document's name.
- There is no default extension, no drive or directory list, no `[..]`, and no file-type filter.
- There is no overwrite prompt.
- Any failure says only "Quill cannot open that file."

The SPEC (DOS "Files") promises a path field, a directory list with drives, and the same file types.

### 1b. Settings

- Written and read: recent files, `[Quill] Language` (SPEC puts it under `[Spelling]`), `Folder`, `Zoom`, and `Toolbar`/`Ruler`/`StatusBar`/`FormattingBar` (`lib.rs:654-760`).
- Never read or written: `Units`, `Window`, `Font`, `Size`, `Margins`, `Backup`, `PictureMemory`, `AsYouType`, `IgnoreCaps`, `Rows`, `Graphics`, `Scheme`, `Debug Trace`.
- `Scheme` is handled only as a `/mono` argument on the DOS command line.

### 1c. Save safety and error messages

- `App::save` (`lib.rs:~844`) does `f.create(name)` on the target itself and never writes a temp file then replaces. A failed write destroys the original, against SPEC "Files".
- `do_save` (`lib.rs:1636`) says only "Quill cannot save the file." with no file name or cause.
- Missing:
  - The "Saving in text format will remove..." warning.
  - The `.BAK` backup.
  - The `~QUILL01.QLD` emergency save and its internal-error box.
  - The named open errors ("is not a document Quill can read", "too large").
  - The overwrite prompt in the DOS dialog.

## 2. Commands accepted but doing nothing or little

Command enum: `crates/quill-app/src/lib.rs:139-230`. Handler: `run`, `lib.rs:1228-1500`.

- **`Command::Preview`, `Command::PrintSetup`** (`lib.rs:1450-1452`). They only set the message "That is not available in this version." This is what DOS and Win16 run for Print Preview, the toolbar Preview button and Print Setup (Win16 intercepts Print Setup in the shell, `main.rs` `run_command`). Win95 intercepts both in the shell (`main.rs:994-998`).
- **`Command::Field(3)`**: empty on every build in committed code (`print.rs:33` and `:57`, never fed). On Win16 no field of any kind is drawn.
- **`Command::Numbering`** (`lib.rs:1344-1373`). It is only a toggle with a hard-coded style 0 and 360-twip indents.
- **`Command::Zoom(z)`**. The core stores it, but DOS and Win16 shells ignore it. The Win16 menu has the submenu and no effect (polish).
- **`Command::Paste`** (`form.rs:438-451`). It re-inserts raw bytes through `insert`, and the buffer is 2048 bytes. Copying across a picture or field marker (0x01) or a long selection truncates silently or yields marker bytes with no object. I did not run this, so treat it as a suspected bug.
- **`Command::Chrome(2)` (Ruler) in DOS text**. The toggle flips a bit, but `paint.rs` draws no ruler row.
- **`Command::Help`** exists on the polish branch only. On Win95 it is not in the enum.

I found no `todo!`, `unimplemented!` or TODO markers in `quill-app`.

## 3. SPEC promises with no code at all

- Insert > Page Break and Ctrl+Enter. No command or key sets PFmt "page break before".
- Edit > Delete and Edit > Size Picture, and picture selection, frame, handles, sizing and `Reset`.
- Drag and drop of text.
- Edit menu naming ("Undo Typing") and greying of Cut, Copy, Paste and Redo.
- Windows clipboard: RTF, `CF_DIB` and `CF_TEXT`, and the WinOldAp path.
- Word, paragraph and Ctrl+Backspace/Delete keys, Shift+Enter, Alt+Shift+Left/Right, Ctrl+Shift+,/..
- Reduce Font, Enlarge Font, Character > Colour... (`ChooseColor`), and Paragraph > Normal.
- Dialogs: Indents and Spacing, Numbering styles, Page Layout, Header/Footer modeless dialog, Size Picture, Spelling Options, and the "Printing ... Cancel" dialog.
- Check Spelling as You Type and the wavy underline.
- Help > Search for Help On and How to Use Help. DOS help viewer and `QUILL.HLX`. F1 on a dialog or highlighted menu item.
- Atomic save, `.BAK`, text-save warning, emergency save, `Quill cannot find its help file` message.
- `/SELFTEST`, DOS `/50` `/G` `/P`, and Win16 `/P`.
- Mouse on DOS (INT 33h), DOS ruler row, and DOS "Page N of M".
- Win16 pictures, printing, preview, zoom, double/triple click, recent files, `/P`.
- The Win95 and Win16 status bar as "Page N of M, Ln, Col, OVR" panes.
- Header taller than its margin pushing the body down (progress.md).
- The Windows-directory `QUILL.INI` on Win16, and the settings in §1b.
- The Win16 "as you type" default-off setting, and `PictureMemory`.
- Installer, release disks, DOS graphics fonts (M10, M11).

## 4. What to fix first

Cheap items are marked **(cheap)**.

1. **Insert Date/File Name (cheap).**
   - Win95 is IF; commit the uncommitted `refresh_clock` and `field_digits`.
   - For Win3.x, draw fields in `gdi.rs:328` using `field_text` as `win9x/gdi.rs:483-505` does, and call `set_print_date` (`GetDateFormat` does not exist on Win16; use `GetProfileString [intl] sShortDate` plus the DOS date).
   - DOS also never sets the date (INT 21h/2Ah).
2. **Windows clipboard (CF_TEXT first).** Without it, copy and paste with other programs fails on both Windows builds. CF_TEXT only is moderate; RTF and DIB are bigger. Also raise the 2 KB buffer.
3. **Commit the polish work: mnemonics, Alt/F10, Win16 separators and tooltips (cheap, already written).** The Win95 `quill.json` is regenerated there too. Re-run `make dos` first, because all the DOS goldens changed.
4. **Win16 accelerators (cheap).** Call `quill_app::accel::from_vk` from `map_vk`. That fixes Ctrl+Backspace opening Replace, Ctrl+1/2/5, Shift+Del/Ins, Ctrl+Ins and Alt+Backspace.
5. **Save safety.** Temp file plus rename (the `Files` trait has no rename, so add one) and real error text.
6. **Win95 real status bar.** Use `msctls_statusbar32` with parts, and make `status_text` say "Page N of M". The Win16 strip is already GDI, so only its content is wrong.
7. **Hide or implement Print Preview and Print Setup on DOS (cheap to hide).** The menu items currently do nothing, against SPEC (DOS has none).
8. **Win16 pictures** (one decode path through BMP; the core's row decoders exist). The segment budget is the constraint: the data segment has 688 bytes spare (progress.md), so this likely needs far memory.
9. **Win16 printing.** Big, and also data-segment-limited; schedule after 8.
10. **Edit menu state (greying, Undo labels, tick marks).** Moderate; the labels (`label::*`) already exist in the core.
11. **Page Break, word movement, Shift+Enter, Page Layout, Indents dialog, Header/Footer dialog.** Core work, then dialogs.
12. **As-you-type spelling.** The lookup costs about 600 ns on the host (PR #13), so it is feasible; it needs idle timing and underline painting.
13. **Help: merge #7 into the Win95 line.** Add Search and How to Use Help (`WinHelp` commands 0x0101 and `HELP_HELPONHELP`), then verify `HELP_CONTEXT`.
14. **DOS:** a proper file dialog, mouse, ruler row, help viewer, and an About line (remove "Host test run").

## 5. Things the SPEC does not say that your testing suggests it should

- **Every Insert menu item must produce a visible result on every build and in the header views.** The SPEC says only that fields "show their value". Add an acceptance rule: each field shows non-empty text, with a width reserved for it.
- **Which platform source supplies the date.** The SPEC says "short date of `WIN.INI [intl]`" for Windows and "country setting" for DOS. State the time format too (a Time field is now being added, uncommitted) and the Win16 method.
- **Menu keyboard access** (Alt, F10, a mnemonic per item, no clashes). It is promised in the "Menus" text and the DOS text-mode section, but there is no acceptance test. The polish `menus.rs` test is a good model.
- **Status bar contract per build.** Real status bar control on Win95, panes, and text for each pane; today only the DOS layout is specified.
- **What a build without a feature does.** Win16 print and pictures say nothing in the SPEC about being absent. A feature missing from a build needs a defined message or a greyed item, not a silent no-op or grey box.
- **Greying and tick behaviour for unavailable commands on DOS** (Print Preview, Print Setup).
- **Clipboard size limit and failure behaviour** (silent truncation at 2 KB is not specified).
- **Ctrl-key and key-down rules** (Ctrl+Backspace, Ctrl+H, Ctrl+I, Ctrl+M clashes) for every shell. This belongs in the keyboard section so Win16 and DOS follow it.
- **Win16 data-segment budget** (688 bytes free). Features such as pictures, print and zoom need a stated memory plan.
- **Settings file location per build** (Win16 reads the current directory today) and which keys each build honours.
- **Wheel-less Windows 95.** The SPEC promises `MSWHEEL_ROLLMSG` but has no test plan for it, since the test guest has no wheel driver.

## Not verified in code

I read only the docs for these: Windows 98 and Me behaviour, the real-Windows 95 result of every Win95 feature (PR bodies and progress.md only), Win16 `HELP_CONTEXT`, print and preview correctness, and spelling timing. I also did not run any build, test or screenshot.
