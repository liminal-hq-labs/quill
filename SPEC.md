# Quill specification

The source of truth for behaviour. A change that alters behaviour updates this file in the same change. How it is built is in [docs/architecture.md](docs/architecture.md), why in [docs/decisions.md](docs/decisions.md) and when in [docs/roadmap.md](docs/roadmap.md).

## Purpose

Quill is a small rich-text word processor for Windows 3.1 and Windows for Workgroups 3.11, written in Rust and built without a C compiler. It sits between Notepad and Word for Windows: more than Write in the places that matter (a toolbar, justified text, bullets, undo and redo, print preview, RTF that today's word processors read), and small enough to fit on one floppy and run on a 386 with 4 MB of memory. It is also a proof that modern Rust can ship a real, period-correct Win16 application.

## Users and scenarios

- **A retro-computing hobbyist** writes a letter on a real 486 or in DOSBox-X, prints it on a period printer and carries the `.RTF` file to a modern machine, where LibreOffice or Word opens it with its fonts, styles and bullets intact.
- **A collector restoring a period machine** wants a word processor that installs from a floppy with a Windows 3.1 setup program, runs in 4 MB and does not need Win32s.
- **A Rust developer** studies how a `no_std` core, a 16-bit code segment and the NE format fit together, and uses Quill's host tools (the reference renderer, the golden tests) without ever starting Windows.
- **A modern user** receives an RTF written on Windows 3.1 and expects it to open cleanly; Quill's RTF output is the contract that makes that work.

## Non-goals

- **Windows 3.0, Standard mode or real mode.** 386 Enhanced mode is the floor (D4).
- **Win32s or a 32-bit build.** Quill is a Win16 NE executable only.
- **Being Word.** No tables, images, columns, headers and footers, footnotes, styles sheets, fields, mail merge, revision marks, OLE or DDE in v1 (the scope table says what is later and why).
- **Unicode text.** Documents are Windows ANSI (code page 1252) in v1 (D11).
- **Spelling and grammar checking**, hyphenation and thesaurus.
- **Help files.** There is no WinHelp `.HLP` in v1; the Windows 3.1 Help Compiler is not redistributable.
- **Network, macros or scripting.** Quill never opens a socket and never runs code from a document.

## Scope

| Feature | v1 | Later | Why |
|---|---|---|---|
| Fonts and sizes from any installed font (TrueType, raster, printer) | Yes | — | The heart of rich text |
| Bold, italic, underline | Yes | — | Cheap, expected |
| Strikethrough, superscript, subscript, text colour | — | v1.1 | Baseline shifts and colour tables add layout and RTF work without changing the design |
| Alignment: left, centred, right, justified | Yes | — | Justification is the visible proof of an own layout engine |
| Indents (left, right, first line, hanging) and the ruler to drag them | Yes | — | Needed for bullets and real documents |
| Line spacing: single, 1½, double; space before and after | Yes | — | Cheap once layout exists |
| Bulleted paragraphs | Yes | — | Requested; one paragraph flag and a hanging indent |
| Numbered lists, multi-level lists | — | v1.1 | Numbering state across paragraphs and RTF `\listtable` round trips are their own project |
| Custom tab stops (left, centre, right, decimal) | — | v1.1 | Default stops every half inch cover v1; custom stops need ruler editing and RTF `\tx` |
| Manual page breaks | Yes | — | One paragraph flag; RTF `\page` is common |
| Toolbar, ruler, status bar (each can be hidden) | Yes | — | The chrome that makes it feel like a word processor |
| Caret, mouse and keyboard selection, keyboard navigation | Yes | — | Core editing |
| Overtype mode (Insert key) | — | v1.1 | Rarely used; Shift+Insert is paste in Windows 3.1 |
| Drag and drop of selected text | — | v1.1 | Not a Windows 3.1 convention |
| Undo and redo, multi-level | Yes | — | Write had one level; multi-level is a clear improvement |
| Cut, copy, paste (text and RTF) | Yes | — | Interop with other applications |
| Find and replace (match case, whole word) | Yes | — | Common dialogs make it cheap |
| New, Open, Save, Save As, recent files | Yes | — | Basic file handling |
| Native format `.QLD` | Yes | — | Fast, exact, versioned (D12) |
| RTF subset read and write | Yes | — | The interchange format (D13) |
| Plain text read and write | Yes | — | Everything reads it |
| Write `.WRI` import | — | Later | Nice period touch, undocumented format |
| Printing with page setup and margins | Yes | — | A word processor prints |
| Print preview (one or two pages) | Yes | — | Cheap because layout already uses printer metrics (D10, D23) |
| Zoom in the editing view | — | Later | Needs scaled font selection; preview covers the need |
| Pictures, tables, columns, headers and footers | — | Not planned for v1.x | Each is a large layout feature |
| About box with free memory and system resources | Yes | — | Period-correct and useful when memory is tight |
| WinHelp help file | — | Not planned | Help Compiler licence (non-goals) |

## Components

- **Quill for Windows** (`QUILL.EXE`): the Win16 application. The only user-facing component.
- **Quill core** (`quill-core`): the document model, editing, undo and layout. Portable `no_std` Rust with no Windows dependency.
- **Quill formats** (`quill-formats`): the native, RTF and plain text readers and writers.
- **Host tools** (`quill-host`): a reference renderer (`quill-render`) that lays out a document and writes PNG pages or a text dump, used by tests and by developers.
- **Installer** (`installer/`): a Liminal Installer project that builds Windows 3.1 floppy images with a `SETUP.EXE`.

## Behaviour

### Window and chrome

- The main window's title is `Quill - <FILE.EXT>`, with `(Untitled)` for a new document, in the style of Windows 3.1 Write. The file name is shown in capitals as DOS stores it.
- Below the menu bar, top to bottom: the toolbar, the ruler, the editing view with a vertical scroll bar (and a horizontal one when the page is wider than the window), and the status bar. **Document** menu items show or hide the toolbar, ruler and status bar; the choice is remembered.
- The editing view shows the page's text width with the left and right margins as white space, on a white background (`COLOR_WINDOW`). Automatic page breaks are a dotted line across the view; manual page breaks a dotted line with the words `Page break`.
- The status bar shows `Page N of M`, `Ln N` and `Col N` for the caret (line within the page and character within the line), and the menu help line for the highlighted menu item while a menu is open.
- Only one document is open per window. Starting Quill again opens a second window with its own document (single-document interface, D17).

### Menus and accelerators

Menus follow the Windows 3.1 conventions: a mnemonic on every item, an ellipsis on items that open a dialog, check marks on toggles, unavailable items greyed out. Items are greyed when they cannot apply (Cut and Copy with no selection, Paste with nothing on the clipboard, Redo with nothing to redo).

| Menu | Item | Accelerator |
|---|---|---|
| File | New | Ctrl+N |
| | Open... | Ctrl+O |
| | Save | Ctrl+S |
| | Save As... | |
| | Print... | Ctrl+P |
| | Print Preview | |
| | Print Setup... | |
| | 1–4 recent files | |
| | Exit | Alt+F4 |
| Edit | Undo `<action>` | Ctrl+Z, Alt+Backspace |
| | Redo `<action>` | Ctrl+Y |
| | Cut | Ctrl+X, Shift+Del |
| | Copy | Ctrl+C, Ctrl+Ins |
| | Paste | Ctrl+V, Shift+Ins |
| | Delete | Del |
| | Select All | Ctrl+A |
| | Insert Page Break | Ctrl+Enter |
| Find | Find... | Ctrl+F |
| | Find Next | F3 |
| | Replace... | Ctrl+H |
| Character | Regular | F5 |
| | Bold | Ctrl+B |
| | Italic | Ctrl+I |
| | Underline | Ctrl+U |
| | Reduce Font | Ctrl+Shift+, |
| | Enlarge Font | Ctrl+Shift+. |
| | Fonts... | |
| Paragraph | Normal | |
| | Left | Ctrl+L |
| | Centred | Ctrl+E |
| | Right | Ctrl+R |
| | Justified | Ctrl+J |
| | Single Space | Ctrl+1 |
| | 1½ Space | Ctrl+5 |
| | Double Space | Ctrl+2 |
| | Bullets | |
| | Indents and Spacing... | |
| Document | Ruler | |
| | Toolbar | |
| | Status Bar | |
| | Page Layout... | |
| Help | About Quill... | |

The Edit menu names what Undo and Redo will do: `Undo Typing`, `Undo Bold`, `Redo Paste`. Character and Paragraph items show a check mark when the whole selection has that property. Reduce Font and Enlarge Font step through 8, 9, 10, 11, 12, 14, 16, 18, 20, 22, 24, 26, 28, 36, 48 and 72 points.

### Toolbar and ruler

- The toolbar holds, left to right: New, Open, Save, Print, Print Preview; Cut, Copy, Paste; Undo; a font name drop-down and a font size drop-down; Bold, Italic, Underline; Left, Centred, Right, Justified; Bullets. Buttons are drawn in the Windows 3.1 three-dimensional grey style; toggles show pressed when the selection has the property and show neither state when it is mixed.
- The font name box lists the fonts available on the display and on the current printer, with TrueType fonts first. Typing a name that is not installed keeps it in the document; Windows substitutes a similar font for display and printing.
- The size box lists the Reduce/Enlarge steps and accepts any whole number from 4 to 144.
- The ruler shows the page's text width in the current units (inches or centimetres), the first-line indent (upper triangle), the left indent (lower triangle with a box that moves both) and the right indent. Dragging a marker changes the indents of every selected paragraph when the mouse is released, as one undo step. Default tab stops show as small ticks every half inch (1.25 cm in metric).

### Caret and selection

- The caret is an I-beam-height bar placed between characters. A selection is a range of characters shown in the system highlight colours. The caret and selection exist only while the editing view has the focus; a selection is kept, unhighlighted, while focus is elsewhere.
- **Mouse.** Click places the caret. Drag selects; dragging above or below the view scrolls automatically. Shift+click extends the selection. Double-click selects a word and the trailing spaces; double-click and drag extends by words.
- **Keyboard.** Arrows move by character and line; Ctrl+Left and Ctrl+Right by word; Home and End to the start and end of the line; Ctrl+Up and Ctrl+Down to the previous and next paragraph start; Page Up and Page Down by one window height; Ctrl+Home and Ctrl+End to the start and end of the document. Holding Shift with any movement extends the selection. Up and Down keep the caret's original horizontal position across short lines.
- **Typing** replaces the selection. Enter ends a paragraph, Shift+Enter inserts a line break inside the paragraph, Tab inserts a tab, Backspace and Delete remove a character or the selection, Ctrl+Backspace and Ctrl+Delete remove a word. New text takes the character format of the character before the caret (or the one after at the start of a paragraph), unless a format was chosen with the caret not moved since, which then applies to what is typed next.

### Character and paragraph formatting

- Character formats apply to the selection, or to the caret's typing format when nothing is selected. A character format is a font name, a size in half points, and bold, italic and underline flags.
- Paragraph formats apply to every paragraph the selection touches. A paragraph format is an alignment, left, right and first-line indents, line spacing (single, 1½ or double), space before and after in points, a bullet flag and a page-break-before flag.
- **Bullets** draw a bullet character (`•`, ANSI 0x95) in the paragraph's first character format at the first-line indent, with the text at the left indent. Turning bullets on for a paragraph with no hanging indent sets one of 0.25 inch (0.6 cm).
- **Justified** paragraphs spread the extra width of every line but the last, and of lines ending in a line break, across the line's spaces. A line with no spaces is left-aligned.
- **Indents and Spacing...** opens a dialog with the alignment, the three indents in the current units, line spacing, space before and after, and the bullet check box.
- **Fonts...** opens the common font dialog (`ChooseFont`) with the selection's font, size and style.
- **Normal** (Paragraph menu) resets the selected paragraphs to the default paragraph format; **Regular** (Character menu) removes bold, italic and underline.

### Layout and scrolling

- Text wraps at the page's text width (the paper width from the current printer minus the left and right margins). Lines break after spaces and after hyphens; a word longer than a line breaks at the last character that fits.
- Character widths come from the current printer when one is installed, so the lines, page breaks and page numbers on the screen are the ones that print (D10). With no printer installed, Quill lays out for the screen and says `No printer installed: layout uses screen fonts` in the status bar once.
- Changing the printer, the paper or the margins lays the whole document out again; the caret stays on the same character.
- Scrolling is by line (arrows on the scroll bar), by window (the scroll bar's track) and by dragging the thumb. The view scrolls to keep the caret visible after every edit and caret movement.

### Files

- **New** starts an empty `(Untitled)` document with the default formats. **Open...** and **Save As...** use the common file dialogs with these types:

| Type in the dialog | Pattern | Read | Write |
|---|---|---|---|
| Quill Documents | `*.QLD` | Yes | Yes |
| Rich Text Format | `*.RTF` | Yes | Yes |
| Text Files | `*.TXT` | Yes | Yes |
| All Files | `*.*` | By content | — |

- A file is read by its content, not its extension: a RIFF header with form type `QLD1` is native, `{\rtf` is RTF, anything else is plain text.
- **Save** writes the document in the format it was opened in, or `.QLD` for a new one. Saving in Text Files format asks first: `Saving in text format will remove all formatting. Save anyway?`. Saving RTF from a document with nothing RTF cannot hold asks nothing, since every v1 property has an RTF form.
- Saving writes a temporary file in the same directory, then replaces the original, so a failed save (disk full, write-protected floppy) leaves the original untouched and says why: `Quill cannot save LETTER.QLD: the disk is full.`.
- Closing a window, New, Open, opening a recent file and Exit ask `Save changes to LETTER.QLD?` with Yes, No and Cancel when the document has unsaved changes.
- The recent-file list holds the last four files opened or saved, as full paths, most recent first. A missing file is removed from the list with a message when chosen.
- Opening a file Quill cannot read says why (`LETTER.RTF is not a document Quill can read.` or `...is too large for Quill.`) and leaves the current document open.
- A document opened from the command line or from File Manager (`QUILL.EXE LETTER.RTF`) opens in the new window. `QUILL.EXE /P LETTER.RTF` prints the file on the default printer and exits, as File Manager's Print command expects.

### Clipboard

- Cut and Copy place the selection on the clipboard as `Rich Text Format` (a registered format) and as `CF_TEXT` (ANSI, CRLF line ends, paragraph formats dropped).
- Paste prefers `Rich Text Format` and falls back to `CF_TEXT`. Pasted text replaces the selection as one undo step. Pasting RTF brings its character and paragraph formats; pasting text takes the typing format.

### Undo and redo

- Every change to the document is undoable: typing, deletion, paste, formatting, paragraph formatting, ruler drags, Replace and Replace All. Typing coalesces into one step until the caret moves by other means, a format changes, or a word ends after more than 32 characters.
- Undo restores the text, the formats and the selection as they were. Redo replays a step undone. A new change after Undo discards what could have been redone.
- Undo keeps at least the last 100 steps or the last 64 KB of undo data, whichever is less; older steps are dropped silently. Saving does not clear undo.

### Find and replace

- **Find...** and **Replace...** open the common modeless find and replace dialogs (`FindText`, `ReplaceText`) with Match case and Match whole word only; Find has Up and Down. The dialog stays open while editing continues.
- A search starts at the selection and wraps around the document once, then says `Quill has finished searching the document.`. Text not found says `Cannot find "<text>".`.
- **Replace All** replaces every match as one undo step and says `Quill made N replacements.`. Replaced text takes the format of the first character of each match.
- **Find Next** (F3) repeats the last search without the dialog.

### Printing and preview

- **Print...** opens the common print dialog (`PrintDlg`) for the copy count, page range (all, selection or pages from–to) and printer. Printing shows a `Printing LETTER.QLD on HP LaserJet III on LPT1:` dialog with Cancel; Cancel stops the job between pages.
- **Print Setup...** opens the print dialog's setup page; changing the printer or paper lays the document out again.
- **Page Layout...** sets the margins (default 1.25 inch left and right, 1 inch top and bottom) and the measurement units.
- **Print Preview** replaces the editing view with one or two whole pages scaled to the window, with Print, Next Page, Previous Page, One Page/Two Pages and Close buttons. Page Up and Page Down move between pages; Escape closes the preview.
- Page numbers are not printed in v1.

### About box

**About Quill...** shows the icon, `Quill`, the version, `A Liminal HQ Labs experiment`, the licence, the mode (`386 Enhanced Mode`), free memory in KB and free system resources as a percentage, in the manner of Program Manager's About box.

### Errors

- Every error that stops a command shows a message box naming the file or the action and the cause in plain words, then leaves the document as it was before the command. There are no error codes in messages.
- **Out of memory.** When memory runs out during an edit, the edit is refused, the document is unchanged, and Quill says `There is not enough memory to complete this operation. Close other applications or save and close documents, then try again.`. Saving needs no extra memory beyond a fixed buffer, so it always remains possible.
- **Internal error.** If Quill reaches an internal error it cannot recover from, it says `Quill has met an internal error and must close. Your document has been saved as ~QUILL01.QLD.` when the emergency save succeeds, or without the second sentence when it does not, and exits.

## Interfaces

### Command line

| Form | Meaning |
|---|---|
| `QUILL.EXE` | Open a new, untitled document |
| `QUILL.EXE FILE` | Open `FILE`, of any supported type |
| `QUILL.EXE /P FILE` | Print `FILE` on the default printer, then exit |
| `QUILL.EXE /SELFTEST SCRIPT` | Run a test script and exit Windows when it ends (test builds only; see [docs/testing.md](docs/testing.md)) |

### Native format (`.QLD`)

A RIFF file (little-endian, chunks padded to even length) with form type `QLD1`. Unknown chunks are skipped, so a later version can add chunks without breaking v1 readers. A file whose `HEAD` version is newer in its major number is refused with `LETTER.QLD was written by a newer version of Quill.`.

| Chunk | Contents |
|---|---|
| `HEAD` | Format version (major `u16`, minor `u16`), character count (`u32`), paragraph count (`u32`), flags (`u32`, zero) |
| `DOCP` | Paper width and height, margins left, right, top and bottom, all `i32` twips; units (`u8`: 0 inches, 1 centimetres) |
| `FONT` | Font table: count (`u16`), then per font a family (`u8`: 0 don't care, 1 roman, 2 swiss, 3 modern, 4 script, 5 decorative), a pitch (`u8`), a character set (`u8`) and the name (`u8` length, ANSI bytes, at most 31) |
| `CFMT` | Character formats: count (`u16`), then per format a font index (`u16`), a size in half points (`u16`) and flags (`u16`: bit 0 bold, bit 1 italic, bit 2 underline; others zero) |
| `PFMT` | Paragraph formats: count (`u16`), then per format an alignment (`u8`: 0 left, 1 centre, 2 right, 3 justify), line spacing (`u8`: 0 single, 1 one and a half, 2 double), flags (`u16`: bit 0 bullet, bit 1 page break before), left, right and first-line indent and space before and after (`i32` twips each) |
| `TEXT` | The text in ANSI: paragraphs end with `0x0D`, a line break inside a paragraph is `0x0B`, a tab `0x09`. The last paragraph has its `0x0D` |
| `RUNS` | Character runs: count (`u32`), then per run a length in characters (`u32`) and a `CFMT` index (`u16`). Run lengths sum to the character count |
| `PARA` | One `PFMT` index (`u16`) per paragraph, in order |
| `INFO` | Optional: title, author and comments, each a `u8` length and ANSI bytes |

### RTF subset

Quill reads RTF 1.x as written by Word 2 to Word 2003, WordPad and LibreOffice, keeping what the document model holds and skipping the rest safely; it writes a small, canonical subset that those programs read.

| Area | Read | Written |
|---|---|---|
| Header | `\rtf1`, `\ansi`, `\ansicpg1252`, `\deff`, `\fonttbl` (`\fN`, family, `\fcharset`, `\fprq`, name), `\colortbl` (parsed, ignored), `\stylesheet` and `\info` (skipped, except `\title`, `\author`, `\doccomm`) | `\rtf1\ansi\ansicpg1252\deff0`, font table, `\info` when set |
| Document | `\paperw`, `\paperh`, `\margl`, `\margr`, `\margt`, `\margb` | The same |
| Characters | `\plain`, `\fN`, `\fsN`, `\b`, `\i`, `\ul`, `\ulnone` and their `0` forms; other underline kinds read as `\ul` | `\plain`, `\f`, `\fs`, `\b`, `\i`, `\ul` |
| Paragraphs | `\pard`, `\par`, `\ql`, `\qc`, `\qr`, `\qj`, `\li`, `\ri`, `\fi`, `\sb`, `\sa`, `\sl` with `\slmult` (mapped to the nearest of single, 1½ and double), `\pagebb`, `\page` | `\pard`, `\par`, alignment, indents, `\sb`, `\sa`, `\sl240`/`\sl360`/`\sl480` with `\slmult1`, `\pagebb` |
| Bullets | `{\pntext ...}` skipped, `{\*\pn ... \pnlvlblt ...}` sets the bullet flag; `\ls` with a bullet `\listtext` sets the bullet flag | Word 6 form: `{\pntext\f<sym>\'B7\tab}{\*\pn\pnlvlblt\pnf<sym>\pnindent0{\pntxtb\'B7}}` |
| Special characters | `\line`, `\tab`, `\~`, `\-`, `\_`, `\emdash`, `\endash`, `\lquote`, `\rquote`, `\ldblquote`, `\rdblquote`, `\bullet`, `\'hh`, `\uN` with `\ucN` (to ANSI where it exists, `?` otherwise) | `\line`, `\tab`, `\'hh` for bytes above 0x7F, `\{`, `\}`, `\\` |
| Skipped safely | Every `{\* ...}` destination, `\pict`, `\object`, fields (the result text is kept), headers, footers, footnotes, tables (each cell becomes text followed by a tab, each row a paragraph) | — |

Limits while reading: groups nest at most 64 deep (deeper groups are skipped), the font table holds 64 fonts, and a control word longer than 32 characters or a number outside `i32` ends the read with `...is not a document Quill can read.`.

### Plain text

- Read as ANSI. CRLF, LF and CR each end a paragraph; a form feed (`0x0C`) starts a new paragraph with a page break before it; other control characters below 0x20 except tab are dropped. Text gets the default character and paragraph formats.
- Written as ANSI with CRLF after every paragraph, line breaks as CRLF, and a form feed for each page break.

### Clipboard formats

| Format | Read | Written |
|---|---|---|
| `Rich Text Format` (registered) | Yes, preferred | Yes |
| `CF_TEXT` | Yes | Yes |

### Settings file

Settings live in `QUILL.INI` in the Windows directory, read with `GetPrivateProfileString` at start and written when changed.

## Settings and defaults

| Setting | Default | Meaning |
|---|---|---|
| `[Quill] Toolbar` | `1` | Show the toolbar |
| `[Quill] Ruler` | `1` | Show the ruler |
| `[Quill] StatusBar` | `1` | Show the status bar |
| `[Quill] Units` | From `WIN.INI` `[intl] iMeasure` (`0` metric, `1` inches) | Ruler and dialog units: `in` or `cm` |
| `[Quill] Window` | Windows' default position | Last window position and size as `x y cx cy maximized` |
| `[Quill] Font` | `Times New Roman` | Font for new documents |
| `[Quill] Size` | `12` | Font size for new documents, in points (4–144) |
| `[Quill] Margins` | `1800 1800 1440 1440` | Left, right, top, bottom margins for new documents, in twips (0–14400 each, leaving at least one inch of text width) |
| `[Quill] Backup` | `0` | When `1`, Save keeps the previous file as `<name>.BAK` |
| `[Recent] File1`–`File4` | Empty | Recent files, most recent first |
| `[Debug] Trace` | `0` | When `1`, write a trace to `QUILL.LOG` next to `QUILL.EXE` |

## Security and privacy

- Quill reads and writes only the files the user opens or saves, the temporary file beside a file being saved, `QUILL.INI`, the emergency file after an internal error, and `QUILL.LOG` when tracing is on.
- It never opens a network connection, never starts another program and never runs anything embedded in a document. Embedded objects and pictures in RTF are skipped, not decoded.
- The RTF, native and text readers treat every file as untrusted: every count, length and nesting depth is checked against the limits below, and they are fuzzed (see [docs/testing.md](docs/testing.md)).
- The trace records commands and timings, never document text.

## Limits

| Limit | Value | At the limit |
|---|---|---|
| Document length | 1,048,575 characters | The edit or open is refused: `The document is too large for Quill.` |
| Paragraphs | 32,767 | As above |
| Characters in one paragraph | 65,535 | Typing or pasting that would pass it is refused with the same message |
| Fonts in one document | 64 | Further fonts are mapped to the nearest family already in the table |
| Character formats | 4,096 distinct | As fonts: unused formats are collected first, then the edit is refused |
| Paragraph formats | 1,024 distinct | As character formats |
| Undo history | 100 steps or 64 KB | Oldest steps are dropped |
| Clipboard copy | Whole document | Copy that cannot get global memory says `There is not enough memory to copy the selection.` |
| Recent files | 4 | Oldest is dropped |
| Path length | 127 characters (DOS limit with drive) | Longer paths cannot be chosen in the dialogs |
| Memory | Runs in 2 MB; 4 MB is the design target | See the out-of-memory behaviour above |
