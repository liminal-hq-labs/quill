# Plan: a proper DOS file dialog for Quill, and a review of every other dialog

**What the DOS Save As is today.** It is a one-line typed path. On Save As, Open and Insert Picture the DOS build draws a fixed 62×7 box at row 8 with `File name:` and an empty field. It has no file list, no folders, no drives and no type choice. Nothing is pre-filled, nothing is checked, there is no overwrite question and no default extension. Characters are upper-cased, and spaces and control characters are dropped.

It is also unsafe in four ways:

- The document's name is changed before the save, so a failed save leaves the document renamed.
- The file is written in place with AH=3Ch, so a failed write destroys the original.
- `DosFiles::close` always reports success, so a write error is never seen.
- A name longer than 64 bytes is cut short without a word.

D87 calls this "DOS keeps its own text-mode name prompt".

**Sources.** I read `feat/m5-win16` (the same as `main` for these files, apart from the Help menu), the polish worktree's uncommitted changes, `fix/win9x-shell` (#13), `fix/win9x-caret-zoom2` and the `feat/win16-gui` worktree (#11). There is no `quill-ui` crate. The text renderer is `crates/quill-app/src/paint.rs` drawing into `quill-grid`.

**Legend.** [V] means I checked it in the code or the golden screens. [U] means an estimate or something not yet tested.

---

## 1. Audit

Severity: **High** loses data or blocks a task; **Med** means a SPEC feature is missing or awkward; **Low** is polish.

Paths are relative to `/home/scott/source/experiments/quill16`.

| Dialog | DOS today [V] | SPEC | Windows 3.x (#11) / 95 (#13) [V] | Gap | Sev |
|---|---|---|---|---|---|
| **Save As** | One-line box (`paint.rs:500-514`, `lib.rs:1153-1159`, `key_prompt` at `lib.rs:1776-1829`). Empty field, at most 63 characters, upper-cased. Enter renames, then `do_save`. No overwrite question, no default extension, no folder; the format comes from the extension, otherwise the opened format (`save` at `lib.rs:792-819`). Failure shows `Quill cannot save the file.` on the status line. Golden: `m7-picture` screen 38 (`File name: OUT.QLD`), also `m2-letter` and `m4-about` | Own file dialog: path field, directory list with `[..]` and drives, the same types (SPEC 329); overwrite question (116); temporary file then replace (261); text-format warning (260); messages that name the file (261) | Win16: `GetSaveFileName`, default `qld`, `OFN_OVERWRITEPROMPT`, last folder; **no pre-fill, no type swap** (`win16/examples/quill/dialogs.rs:480-541`). Win95: pre-fills the base name, picks the type from the extension, a `CDN_TYPECHANGE` hook swaps the extension (`win9x/quill-win9x/src/main.rs:759-892`) | Almost everything. Silent overwrite, unsafe write, renamed on failure, silent truncation | **High** |
| **Open** | Same box, titled ` Open ` (`lib.rs:1134-1141`). `Folder=` is read but never used on DOS. Errors on the status line | As Save As, the four types, the read-by-content rule | Common dialog, `OFN_FILEMUSTEXIST`, last folder | No browsing. You have to know the name | **High** |
| **Insert Picture** | Same box (`lib.rs:1321-1327`). `Quill cannot use that picture.` | File dialog for `*.BMP` and `*.PNG` (222) | Common dialog with the picture filters | As Open | **High** |
| **Unsaved changes** | `Mode::Confirm` box ` Quill `, `Save changes to X?` with `< Yes > < No > < Cancel >`, none highlighted (`paint.rs:665-680`). Only Y, N and Esc work; Enter and Tab do nothing (`lib.rs:1534-1555`). `Recent` is not confirmed on `main` (#12 fixes it) | Yes, No, Cancel message box (262) | `MessageBox` with Yes, No and Cancel | No default button; Enter is dead; no mouse | **Med** |
| **Message boxes** | None. Every error and notice is a one-key status-line message (`Msg`, 96 bytes) | Errors that stop a command are message boxes naming the file and the cause (366-370) | MessageBox (partly) | No component; wording differs from the SPEC | **High** |
| **Overwrite question** | None | Required | Common dialogs | Missing | **High** |
| **Text-format warning** | None | `Saving in text format will remove…` (260) | Not built [V]: nothing in either Windows shell | Missing | Med |
| **Find** | Box: Find what, Match case, Match whole word only, `< Find Next > < Close >` (`paint.rs:515-568`, `find.rs:278-357`). Tab moves forward only; Enter in the field finds the next. Fixed at row 8, so it can **cover the match**. Not found goes to the status line | Match case, whole word, **Up and Down**; stays open while you edit (279) | `dialogs.json` 1002, with mnemonics | No direction; hides the match; no Shift+Tab, mnemonics or mouse | Med |
| **Replace** | As Find plus Replace with, Replace, Replace All. Height 11 leaves three empty rows (`m2-find` golden). The count goes to the status line | `Quill made N replacements.` | 1002 | Layout, no mnemonics | Low |
| **Fonts** | Form (`form.rs:131-199`): the face cycles through three fixed names. **A face not among the three is shown as Times New Roman, and OK writes Times New Roman over it** (`form.rs:153-155`, `296-323`). Size field, four check boxes, OK and Cancel stacked as rows. Golden `m2-dialogs` | Fonts dialog with effects | Win16 `ChooseFont`; Win32 from `dialogs.json` or `ChooseFont` | Damages data; no colour or super/subscript; layout | **High** |
| **Tabs** | Form (`form.rs:201-238`): Position (inches only), Kind, Leader, Set, Clear, Clear All, Close. Enter in a field presses Set | Set by position, kind and leader; clear one or all; current units (208, 483) | 1004 with a list of stops | No list of existing stops; inches only | Med |
| **Spelling** | Box (`paint.rs:621-664`, `spelling.rs:369-439`): word, context, up to 8 suggestions, 6 buttons, Up/Down choose. Change to cannot be edited; Change with no suggestion does nothing | Change to; Suggest; `More...`; Options with language (241-245) | 1005 with a Change to field and a list | No Change to field, Suggest or Options | Med |
| **About** | Box with Quill, version, Labs line, `Apache-2.0 OR MIT`, and a platform line **hard-coded to `Host test run`** (`dos/quill-dos/src/main.rs:52`). No copyright, no memory figure | Mode and free memory (297); SCOWL credited (507) | 1001 with copyright and licence | Wrong platform line; copyright and SCOWL missing | Low |
| **Print** | Prompt ` Print to file or device `; the typed name, such as `LPT1`, is created with 3Ch (`lib.rs:1332-1338`). Goldens `m4-print`, `m4-print-headers` | LPT1, LPT2 or a file (328) | `PrintDlg` | No destination choice, copies or range | Med |
| **Print Setup / Preview** | Shown in the DOS File menu; both say `That is not available in this version.` | DOS has no preview (356); the menu drops items in the differences table (303) | Real | Items should be hidden | Low |
| **Page Layout** | None, and no menu item | Margins, header and footer distances, start page, units (287) | Not built on either | Missing; margins cannot be changed on DOS | **High** |
| **Indents and Spacing** | None, and no menu item | Required (211) | Not built | Missing | **High** |
| **Numbering** | `Paragraph > Numbering` toggles numbering with style 0 (`lib.rs:1223-1255`) | Style per level, start, restart (210) | Not built | Missing | Med |
| **Header/Footer, Size Picture, Colour, Help** | Header and Footer are menu commands only; Size Picture and Colour are absent; F1 says `Help is in the Windows builds for now` | A small Write-style box (215); Size Picture (224); colour (200); `QUILL.HLX` viewer (330) | Header box not built; ChooseColor not checked [U] | Missing | Low/Med |

**Problems shared by all the DOS dialogs** [V]:

- **Placement.** Every box is at x=8, y=8 (`paint.rs:686-692`), not centred and not aware of the row count.
- **80×50 does not exist.** The shell uses `Grid::<25>` and mode 3 only, and reads no `/50` (`dos/quill-dos/src/main.rs`).
- **No Shift+Tab.** BIOS (0, 0x0F) decodes to nothing, and `Key` has no back-tab.
- **No Alt+letter inside dialogs.** Polish adds `Key::Alt` for menus only.
- **No mouse anywhere.** There is no INT 33h code.
- **Buttons are inconsistent.** Some sit in a row (Find, Spelling, Confirm), some in a column (Fonts, Tabs). No default button is marked, and focus is shown as attribute 0x07.
- **No mnemonics**, although `assets/dialogs/dialogs.json` has `&` labels for all five dialogs.

---

## 2. The DOS file dialog (Open, Save As and Insert Picture share it)

### Wireframes (generated to the exact width)

**80×25.** The dialog is 68×19 at column 6, row 3. The menu bar (row 0) and the status line (row 24) stay visible. The list height is `L = clamp(ROWS-17, 6, 24)`, which gives 8 here, and the dialog height is `L + 11`. ▒ marks the existing `shadow()` (the cell's character kept, attribute 0x08).

```text
00| File  Edit  Insert  Find  Character  Paragraph  Document  View  Help           |
03|      ┌──────────────────────────── Save As ─────────────────────────────┐      |
04|      │                                                                  │▒▒    |
05|      │ File name:            Directories:                               │▒▒    |
06|      │ LETTER.QLD░░░░░░░░    C:\DOCS\LETTERS            <   OK   >      │▒▒    |
07|      │ ┌──────────────────┐  ┌──────────────────────────┐ < Cancel >    │▒▒    |
08|      │ │ APRIL.QLD       ▲│  │ [..]                     │               │▒▒    |
09|      │ │ BUDGET.QLD      █│  │ [2025]                   │               │▒▒    |
10|      │ │ LETTER.QLD      ░│  │ [OLD]                    │               │▒▒    |
11|      │ │ MEMO.QLD        ░│  │ [-A-]                    │               │▒▒    |
12|      │ │ MINUTES.QLD     ░│  │ [-C-]                    │               │▒▒    |
13|      │ │ NOTES.QLD       ░│  │ [-D-]                    │               │▒▒    |
14|      │ │ REPORT.QLD      ░│  │                          │               │▒▒    |
15|      │ │ SPRING.QLD      ▼│  │                          │               │▒▒    |
16|      │ └──────────────────┘  └──────────────────────────┘               │▒▒    |
18|      │ Save file as type:                Drives:                        │▒▒    |
19|      │ [Quill documents (*.QLD)      ▼]   [C:           ▼]              │▒▒    |
21|      └──────────────────────────────────────────────────────────────────┘▒▒    |
22|        ▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒    |
24| LETTER.QLD   Pg 1 Ln 1 Col 1   Times New Roman 12                              |
```

**80×50.** The same dialog with `L = 24`: 68×35 at row 7. Rows 19 to 33 are left out below.

```text
07|      ┌──────────────────────────── Save As ─────────────────────────────┐▒▒
10|      │ LETTER.QLD░░░░░░░░    C:\DOCS\LETTERS            <   OK   >      │▒▒
11|      │ ┌──────────────────┐  ┌──────────────────────────┐ < Cancel >    │▒▒
12|      │ │ APRIL.QLD       ▲│  │ [..]                     │               │▒▒
13|      │ │ BUDGET.QLD      ░│  │ [2025]                   │               │▒▒
14|      │ │ LETTER.QLD      █│  │ [OLD]                    │               │▒▒
   …     (24 list rows in all)
35|      │ │ F15.QLD         ▼│  │                          │               │▒▒
36|      │ └──────────────────┘  └──────────────────────────┘               │▒▒
39|      │ [Quill documents (*.QLD)      ▼]   [C:           ▼]              │▒▒
41|      └──────────────────────────────────────────────────────────────────┘▒▒
```

**Drawing.**

- The dialog is drawn last in `draw()`, after the page, menu bar and status line, as now. Each part uses the existing `Grid::fill`, `frame`, a centred title and `shadow`.
- A scroll bar column (▲ 0x1E, ░ 0xB0 track, █ 0xDB thumb, ▼ 0x1F) appears only when the list overflows.
- An open drop-down is a framed list with its own shadow. It drops downward, or upward when there is no room; at 80×25 the type list opens upward over rows 13 to 18.
- The hardware cursor sits in the File name field and is hidden while focus is on a list or a button, as the current code does.
- Labels carry `&` mnemonics: File &name, Fi&les (not in Windows 3.1, but useful in text cells), &Directories, Save file as &type / List files of &type, Dri&ves.
- There is no Read Only box, because Quill has no read-only mode.

**Attribute roles.** One table in each scheme, shared by every dialog:

| Role | Colour | Mono |
|---|---|---|
| Frame, labels | 0x70 | 0x70 |
| Mnemonic letter | polish's `HOT` 0x74 | Polish's mono choice |
| Field and list body | 0x07 (as now) | 0x07 |
| Selected item, focused | 0x1F | 0x70 |
| Selected item, unfocused | 0x70 | 0x0F |
| Focused button | 0x07 (as now) | 0x07 |
| Default button | Brackets in 0x7F | 0x7F |
| Disabled | 0x78 | 0x70, no brackets |
| Shadow | 0x08 (as now) | 0x08 |

### Behaviour

- **Types.**
  - Open: Quill documents (`*.QLD`), Rich Text Format (`*.RTF`), Text files (`*.TXT`), All files (`*.*`).
  - Save As: the first three only. `Text Files (ANSI)` waits for CP437 text writing, which does not exist yet [V]: `quill-formats/src/text.rs` has no code-page choice.
  - Insert Picture: Pictures (`*.BMP;*.PNG`), Bitmaps, PNG pictures, All files. These are the same lists as the Windows filters, from one shared table.
- **Save As starts with the document's own name.** The field holds the base name of the current document, all selected, so typing replaces it. The type comes from the extension, or from the format the file was opened in when the extension is unknown. The folder is the document's own, then `[Quill] Folder=` if it still exists, then the current directory. An untitled document starts with an empty name and Quill documents.
- **Changing the type keeps the name and swaps only the extension.** On DOS any extension is replaced, since 8.3 has only one; Win32 keeps its rule of swapping only known extensions. An empty name, or one with `*` or `?`, is left alone and only the list is filtered again. The default extension is added when the name has no dot.
- **Enter (OK) reads the name field this way:**
  1. A name with a wildcard becomes the list's pattern.
  2. `..`, `C:`, `\DOCS`, `DOCS\` or an existing folder changes folder (Save As keeps the typed name).
  3. Otherwise the path is resolved without `.` and `..`. A result over 79 bytes is refused.
  4. The last part is checked as an 8.3 name.
  5. Open and Insert Picture: the file must exist, otherwise `Quill cannot find C:\DOCS\X.QLD.`. With no dot, Open also tries the type's extension, as `lpstrDefExt` does.
  6. Save As:
     - An existing folder: change to it.
     - A read-only file: `LETTER.QLD is read-only. Save it under another name.`
     - An existing file: `C:\DOCS\LETTER.QLD already exists. Replace it?` with Yes and No, **No the default**.
     - The Text type: the SPEC's warning, Yes the default.
     - Then a safe save with the format **chosen by type**, not by extension. The document's name changes only after success.
- **Never change the current directory.** The dialog keeps its own folder string and the app stores the full path. Today `QUILL.INI`, the dictionaries and `QUILL.UDC` are opened by relative name, so changing directory would scatter them. The status line and the `[File]` field show the base name. Recent entries become full paths, so the `m4-quill.ini` golden changes.

### DOS 8.3 names (in a pure, property-tested module)

- **How names are shown.** Upper case, as DOS returns them. Typed letters are upper-cased (as now).
- **What is refused.**
  - Characters below 0x21, characters above 0x7E, and `" * + , / : ; < = > ? [ \ ] |` (wildcards are taken as a pattern before this check).
  - More than one dot, a leading dot, an empty base, a base over 8 or an extension over 3.
  - Reserved device names as the base whatever the extension: `CON PRN AUX NUL COM1–COM9 LPT1–LPT9 CLOCK$`. So `CON.TXT` is refused. The Print dialog is the one place where `LPT1` and `LPT2` are allowed.
- **Nothing is truncated.** DOS would turn `LONGFILENAME.QLD` into `LONGFILE.QLD` and could overwrite another file. Quill refuses instead: `LONGFILENAME.QLD is not a DOS file name: use up to 8 letters, a dot and up to 3.`
- **A trailing dot** (`LETTER.`) means "no extension", and no default is added (the Windows rule).

### Reading directories

**What the runtime has today** [V], in `dos/rt/src/dos.rs`:

- Open, create, read, write, seek, close.
- Delete (41h), mkdir (39h), rmdir (3Ah), attributes (4300h), free space (36h).
- Everything goes through a 32 KB transfer buffer and the DPMI real-mode call 0300h.
- `dpmi` has `alloc_dos_memory` and `set_rm_vector`.

**What is missing and has to be added:**

- Set DTA (1Ah) to a 128-byte area in conventional memory.
- Find first and find next (4Eh/4Fh): the path at DS:DX in the transfer buffer, the 43-byte DTA giving the attribute at 15h, the size at 1Ah and the name at 1Eh.
- Get current drive (19h) and current directory per drive (47h, a 64-byte buffer).
- Drive validity with IOCTL 4409h, which does not touch the media (error 0Fh means no such drive); 440Eh to drop a phantom B:; INT 11h for the number of floppy drives.
- Rename (56h) for the safe save; DOS version (30h) and DPMI free memory (0500h) for About.
- **A critical-error handler** for INT 24h: a 3-byte real-mode stub, `mov al,3; iret`, in a 1-paragraph DOS block, installed with `set_rm_vector` and removed by `cleanup`. Without it, an empty A: makes DOS print "Abort, Retry, Fail" over the text screen and wait for a key. With it, Quill says `Drive A: is not ready.` and stays in the previous folder. [U] DOSBox cannot make a drive not ready.

**How the list is read and stored:**

- **One pass.** `*.*` with attribute 10h (folders). Hidden, system and volume entries are not returned. DOS's own `.` and `..` are skipped and `[..]` is made up for any folder below the root.
- **Matching.** Files are matched against the type's patterns in the core (DOS `*` and `?` rules for base and extension), so `*.BMP;*.PNG` needs one pass.
- **Re-reading.** The folder is read again on a change of folder, drive or type.
- **Storage.** The entries live in **Store chunks** (16 bytes each: 12-byte name, attribute, kind), freed when the dialog closes. Files are capped at **512** (8 KB) and folders at 256 (4 KB). Past the cap the last row reads `(more files)` and a note asks for a pattern. 512 is also the FAT16 root-directory limit.
- **Sorting.** Heapsort in place in the chunk: no `alloc`, O(n log n), the same order on every build. The order DOS (or a DOSBox mount) returns does not matter.
- **Drives.** They come from a bit mask, not the list. They show as `[-C-]` at the end of Directories (SPEC 329) and in the Drives drop-down. A: and B: are touched only when chosen.
- **Speed and memory** [U, to measure at `cputype=386` with fixed cycles through `DOS_CONF`]:
  - Each find-next is one real-mode switch plus DOS's directory scan, perhaps 0.1–0.5 ms on a cached hard disk. So 512 entries take about 0.1–0.3 s, and a floppy root of 224 entries takes about 1 s.
  - Sorting 512 is about 9k comparisons, under 10 ms.
  - The dialog state stays under about 400 bytes inside `Mode`; the lists stay in the Store.

### Keyboard and mouse

- **Tab order.** File name → Files → Directories → Type → Drives → OK → Cancel. Shift+Tab goes back; this needs `Key::BackTab` from BIOS (0, 0x0F) in `quill_app::bios`. Alt+mnemonic jumps to a control.
- **Enter** acts as above. In Directories it opens the folder; in Files it accepts the file.
- **Esc** closes an open drop-down, otherwise cancels.
- **File name field.** Insert, Backspace and Delete; Left, Right, Home and End; Shift+movement selects; typing replaces the selection.
- **Lists.** Up, Down, PgUp, PgDn, Home and End. A letter jumps through the entries that start with it, as a Windows 3.1 list box does; there is no timer, so goldens stay the same. Moving in Files copies the name into the field.
- **Drop-downs.** Up and Down change the choice in place and apply it. Alt+Down or F4 opens the list.
- **Mouse** (when INT 33h is present):
  - A click focuses and selects; a double click on a file accepts it, on a folder or drive opens it.
  - Arrows scroll, the track pages, a click on ▼ opens a drop-down.
  - Buttons act on release, as Windows does. Clicks outside the dialog are ignored.
  - The shell hides the driver's cursor (AX=2) before writing to video memory and shows it again (AX=1) afterwards, or old cells reappear.
  - Cells are pixels ÷ 8 (AX=8 sets the range for 50 rows).
  - A double click is two presses on the same cell within 9 BIOS ticks.

---

## 3. Where the code lives

**A new portable crate, `crates/quill-dlg`.** It follows the same rules as the other portable crates: `no_std`, no `alloc`, no floats, no panics. It depends on `quill-core` only (for `Store` and `ChunkId`), not on `quill-grid`, so DOS graphics mode can draw it later. Modules:

- **`name83`.** Validation, split and join, parent, normalising, wildcard matching, device names.
- **`types`.** One table of document and picture types. The Win16 and Win32 `\0` filter strings are built from it, replacing `DOC_FILTER`, `PIC_FILTER` and the `docs`/`pics` literals.
- **`savename`.** The starting name and type, the extension swap (`Rule::Any` or `Rule::KnownOnly`) and the default extension. It replaces Win32's `with_extension` and `SAVE_EXTS`.
- **`widgets`.** Field, List (selection, top row, type-ahead, paging), Combo, Check, Radio, Buttons, the focus ring and mnemonic dispatch. Input is a small `DlgKey` enum, because `Key` lives in `quill-app` and a dependency there would be circular.
- **`fs`.** `trait FileSystem { list(dir, each), drives() -> u32, current_dir(drive, out), exists(path) -> Option<attr>, rename, remove }` and `FsError` (NotReady, NotFound, NoPath, Denied, Full, Unsupported).
- **`filedlg`.** `FileDlg::new(kind, start, fs, store)`, then `key` and `mouse` returning `Outcome::{None, Ask(Question), Accept{path, type}, Cancel, Error(..)}`, and a read-only view for the painter.

**In `quill-app`:**

- `Mode::File(FileDlg)` replaces `Mode::Prompt` for Open, Save As and Picture.
- `Mode::Message(MsgBox)` carries a continuation (Confirm, Overwrite, TextWarning, Info). The file dialog is kept in an `App` field while a message box is on top of it.
- `dlgview.rs` paints widgets into `Grid<ROWS>` with the role table.
- New `App::save_as_format(path, fmt)`.
- `Files` gains the `FileSystem` methods **with default bodies that return `Unsupported`**, and an adapter feeds `FileDlg`. So `Win16Files` and the Win32 files type compile unchanged.

**Tests:**

- Host property tests in `quill-dlg`, using proptest-generated folder trees including ones over 512 entries:
  - It never panics; the selection stays in range; the top row is at or before the selection, which stays on screen.
  - Accept returns only valid paths within the limit; join and parent round-trip; the extension swap is idempotent and keeps the base.
  - A counting Store holds zero live chunks after close or cancel.
- `quill-host`'s `MemFiles` becomes a drive and folder tree that mirrors the DOSBox disk: C: holds `QUILLDOS.EXE` and the copied files, and the drives are C:, D: and Z:. Scenarios gain `@dir NAME`, `@copy NAME DIR\` and `@rows 50`.

**The Windows builds keep their native dialogs.** `native_request` is unchanged. Win32 swaps its local helpers for `quill-dlg::savename` and `types` with no change in behaviour (checked by `wine-dialogs.sh`). Win16 can later pre-fill the name (plain `lpstrFile`) and then add a COMMDLG 3.1 hook (`cmb1` `CBN_SELCHANGE` → `edt1`); that needs `MakeProcInstance`, so Win16 follows.

**Watch:** the Win16 play-back (`/PLAY`, `set_native_dialogs(!gridview)`) and the Win32 text view still draw the text-cell dialogs [V]. Their file lists will differ from the host's, which affects `m1-edit`'s Open frame (decision 9).

---

## 4. The other dialogs, in priority order

**What every dialog should share.**

- **Placement.** Centred and row-aware, except Find and Replace (below).
- **Buttons.** A right-hand column in dialogs with lists, a bottom row in message boxes.
- **Default and Enter.** The default button has bright brackets. Enter presses the default unless focus is on another button.
- **Keys.** Esc always cancels or closes. Space toggles check boxes and presses the focused button; arrows move within a radio group. Tab and Shift+Tab move focus.
- **Mnemonics.** Alt+mnemonic, with the label text taken from `dialogs.json` so the letters match Windows. Plain letters work in message boxes; the goldens' `n` keeps working.
- **Mouse** everywhere.

| # | Dialog | Fix | Goldens to update |
|---|---|---|---|
| 1 | Unsaved changes | MsgBox: Yes as default, Enter and Tab work, Y/N/C letters, mouse | `m2-dialogs`, `m2-find`, `m2-headers`, `m2-lists`, `m2-mono`, `m4-print-headers`, `m8-spelling` |
| 2 | Errors and notices | Message boxes with the SPEC's wording, mapped from `FormatError` and `FsError`: cannot open, save or use a picture (naming the file and cause), not found, finished searching, N replacements, spelling complete | `m2-find`, `m8-spelling`, plus new error scenarios |
| 3 | Fonts | Stop overwriting the face: list the document's font table plus the three, select the current face, apply only the fields that changed. Size list, superscript and subscript, the 16 colours and Automatic, mnemonics | `m2-dialogs` |
| 4 | Page Layout (new) | Margins, header and footer distances, first page number, units; a Document menu item; a core command that sets `page` and lays out again | New `m11-page` |
| 5 | Indents and Spacing (new) | Alignment, left, right and first-line indents, spacing, before and after, page break before; a Paragraph menu item | New |
| 6 | Numbering (new) | Style per level, start, restart (PFMT bit 3); `Numbering...` opens it | New, plus `m2-lists` |
| 7 | Print | Destination radio (LPT1, LPT2, File plus name), copies, range; overwrite question for a file | `m4-print`, `m4-print-headers` |
| 8 | Find and Replace | Up and Down for Find; drawn in the half of the screen away from the caret row; Replace at the right height; Find Next as default; Shift+Tab, mnemonics | `m2-find` |
| 9 | Tabs | List of current stops (choosing one fills the fields); the current units; buttons in a column as in 1004 | `m2-dialogs` |
| 10 | Spelling | Editable Change to, Suggest and `More...`, Options with the language (writes `Language=`) | `m8-spelling` |
| 11 | About | DOS version, DPMI host and free KB on a real run; copyright and SCOWL lines; the default OK. The e2e run keeps a fixed platform line when `QUILL_DUMP` is set, or host and DOS screens differ | `m4-about` |
| 12 | Menus | Hide Print Preview and Print Setup in the DOS text build | All screen goldens (once, on top of polish) |
| 13 | Later | Header/Footer box, Size Picture, Colour, F1 topics once the DOS help viewer exists | New |

New dialogs also need their native Windows versions under D86 (Win32 first, then Win16). Those are separate pull requests outside this plan.

---

## 5. Stacked pull requests

**Start now, with no conflicts.** These touch only new files, `dos/rt`, or `dos/quill-dos/src/{main,video}.rs`. Polish touches only `keys.rs` on the DOS side, and #6, #12, #13 and caret-zoom do not touch these paths.

| PR | Base | Scope | Acceptance and tests |
|---|---|---|---|
| **A** `feat/dos-rt-dir` | `main` | The runtime calls in §2, the INT 24h stub, and a `dos/rt` test program | Run through `dosrun.sh` on a prepared C: (folders, 600 files): its sorted output equals the host's upper-cased listing; drives C, D, Z; invalid drive gives 0Fh with no hang; rename works; timing at 386 fixed cycles goes in `progress.md`. Not-ready drive [U] |
| **B** `feat/dlg-names` | `main` | `quill-dlg` with `name83`, `types`, `savename`; workspace member; architecture and AGENTS lists | Property tests (validity, round trips, swap idempotent); `make validate` |
| **C** `feat/dlg-widgets` | B | Widgets, focus, mnemonics, `DlgKey` | Property tests: any key sequence keeps every widget's invariants |
| **D** `feat/dlg-filedlg` | C | `FileSystem`, `FileDlg`, a fake file system | Property tests (as §3); chunk leak check; overwrite, read-only, trailing dot, device names, the 512 cap |
| **E** `feat/dos-80x50` | `main` | `/50` and `[Quill] Rows`: INT 10h 1112h, `Grid<50>`, `set_rows` | `hello.80x50` golden; one `@rows 50` scenario in host and e2e |

**After polish is committed.** Polish rewrites every screen golden and `paint.rs` and brings in #12's `recent.rs` and `shorten_path`.

| PR | Base | Scope | Acceptance and tests |
|---|---|---|---|
| **F** `feat/dos-msgbox` | polish | MsgBox, attribute roles, centred chrome in `dlgview`, `Key::BackTab` | Goldens #1 regenerated and reviewed; e2e passes |
| **G** `feat/dos-file-dialog` | F, merging D | `Mode::File` for Open, Save As and Picture; `MemFiles` tree and new directives; SPEC 305 and 329; amend D87, new decision | New `m10-open`, `m10-save-as` (pre-fill, type swap, overwrite No and Yes, bad and reserved names, wildcard, `..`, a change to D:) and a host-only 80×50 golden. Update `m2-letter`, `m4-about`, `m7-picture` and `m4-quill.ini`. `size.rs` still passes; add a `Mode` size test. Also needs caret-zoom's `set_name` change (a small conflict) |
| **H** `feat/dos-files-listing` | G + A | `DosFiles` implements the listing; `@mkdir` in `e2e.sh`; `QUILL.INI` beside the program | `m10-*` match the host in DOSBox character for character; saved files byte-identical |
| **I** `fix/dos-safe-save` | H | Temporary file plus rename, close errors reported, read-only, text warning, wording per SPEC 261 | Host test with a file system that fails mid-write: the original survives and the name is unchanged; DOSBox full-disk run with `-freesize` |
| **J** `feat/dos-mouse` | G + polish's polling key loop | INT 33h, `App::mouse` for menus, dialogs and the page; script tokens `click`, `dclick`, `drag` | Host goldens; in DOSBox, a `QUILL_EVENTS` file injects events at the INT 33h poll; locally `DOS_X11=1` with xdotool [U] |
| **K1–K6** | F (each its own) | Find/Replace, Fonts, Tabs, Spelling, About, Print (§4) | The goldens named in §4 |
| **L1–L3** | F | Page Layout, Indents and Spacing, Numbering | New scenarios and core tests |
| **W32** | #13 + B | Win32 uses the `quill-dlg` helpers | `wine-dialogs.sh` and a Windows 95 hand test |
| **W16** | #11 + B | Pre-filled Save As name; type hook later | A Windows 3.11 run |

**How they relate to the open pull requests.**

- #6, #12, #13 and caret-zoom touch `quill-app/src/lib.rs` (`set_name`, `NAME_MAX`, recent files), so G lands after them.
- #7 and #8 are unrelated until F1 inside dialogs.
- W16 waits for #11.
- F to L wait for polish to be committed and stacked.

---

## 6. Decisions for you (recommended default in **bold**), and risks

1. Dialog logic in a new `quill-dlg` crate, or a module inside `quill-app`? **New crate.**
2. Text-cell layouts generated from `dialogs.json`, or laid out by hand? **By hand, with label text and mnemonics from the JSON.** Converting dialog units to cells lays text out badly.
3. Show names in **upper case**?
4. Long or invalid names: **refuse with a message**, never truncate.
5. A trailing dot means **no extension, and no default is added**.
6. On a type change, DOS **replaces any extension**.
7. The overwrite question's default button: **No**.
8. The cap: **512 files and 256 folders**; hidden and system files **not shown**.
9. Win16 play-back and the Win32 text view: **compare the saved files and skip frames while a file dialog is open**, rather than adding listing to both.
10. Drives in **both the Drives drop-down and as `[-C-]` entries in Directories**.
11. Save As types: **three until CP437 text exists**, then add `Text Files (ANSI)`.
12. Find results and Replace counts in **message boxes**.
13. **Never change the current directory; store full paths.**
14. Untitled Save As starts with an **empty name**.
15. New decision numbers start after D96. D92–D95 already mean different things on different branches, so **renumber them before merging**.

**Risks:**

- **Win16 size.** `quill-app` is linked into QUILL.EXE (about 172 KB of code [V], with segment budgets), so the dialog code costs Win16 segments. If the budget fails, put the text dialogs behind a feature that only DOS and the host turn on.
- **`NAME_MAX` is 64.** A DOS path can be 79 bytes, and `set_name` and `remember_folder` cut longer paths without a word [V]. Raise it to 80 for the 8.3 builds, or refuse longer paths.
- **Golden churn.** Polish already rewrites all 12 screen goldens, so anything that changes screens before polish is committed will conflict.
- **INT 24h under CWSDPMI** and a not-ready floppy are untested; DOSBox cannot reproduce them [U].
- **DOSBox Z:** differs between DOSBox versions, so no golden may open it.
- **Mouse.** AUTOTYPE cannot drive it [V, per `progress.md`]; xdotool driving DOSBox Staging is untested [U].
- **DOS Find stays modal.** The SPEC calls Find modeless; document this as a DOS difference.

### Critical files for implementation

- /home/scott/source/experiments/quill16/crates/quill-app/src/lib.rs
- /home/scott/source/experiments/quill16/crates/quill-app/src/paint.rs
- /home/scott/source/experiments/quill16/dos/rt/src/dos.rs
- /home/scott/source/experiments/quill16/crates/quill-host/src/script.rs
- /home/scott/source/experiments/quill16.wt-win9x-editing/win9x/quill-win9x/src/main.rs
