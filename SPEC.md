# Quill specification

The source of truth for behaviour. A change that alters behaviour updates this file in the same change. How it is built is in [docs/architecture.md](docs/architecture.md), why in [docs/decisions.md](docs/decisions.md) and when in [docs/roadmap.md](docs/roadmap.md).

## Table of contents

- [Purpose](#purpose)
- [Platforms](#platforms)
- [Users and scenarios](#users-and-scenarios)
- [Non-goals](#non-goals)
- [Scope](#scope)
- [Components](#components)
- [Behaviour](#behaviour)
- [Interfaces](#interfaces)
- [Settings and defaults](#settings-and-defaults)
- [Security and privacy](#security-and-privacy)
- [Limits](#limits)

## Purpose

Quill is a small rich-text word processor for Windows 3.1, Windows 95 and DOS, written in Rust and built without a C compiler. It sits between Write and Word for Windows 2.0: fonts, colours, lists, tab stops, headers and footers, pictures, a spelling checker, a help file, print preview, and RTF that today's word processors read, in a program that fits on a floppy and runs on a 386 with 4 MB of memory. Under the three front ends sits one portable core, so a letter started in WordPerfect-blue text mode on a DOS laptop opens, fonts and all, on the family's Windows 3.1 PC.

Quill is the companion application to the Liminal HQ blog series *Cross-compiling to the past*: the series explains the toolchains, and Quill is the real program built with all three of them.

## Platforms

| Build | File | Runs on | Role |
|---|---|---|---|
| Win16 | `QUILL.EXE` (NE) | Windows 3.1 and 3.11 in 386 Enhanced mode, 386 or later. It also runs on Windows 95, 98 and ME, as a bonus | The primary build |
| Win32 | `QUILL32.EXE` (PE, subsystem 4.0) | Windows 95, 98 and ME on a Pentium or later; later, optionally, NT 4.0 to Windows 11 | The build for Windows 9x, with long file names |
| DOS | `QUILLDOS.EXE` (go32 with a built-in DPMI host) | MS-DOS 5 or later on a 486 or later with a VGA | Text mode (80 × 25 or 80 × 50) first; a graphics mode with real fonts (`/G`) second |
| Linux terminal | `quill-tty` | Linux, in a terminal of at least 80 × 25 | The DOS text screen on crossterm, with the same editor; see [docs/linux.md](docs/linux.md) |
| Linux command line | `quill` | Linux | `cat`, `convert` and `info` for documents, with no screen |
| Linux desktop | `quill` (`linux/quill-gtk`) | Linux with GTK 4.20 or later, on Wayland or X11 | A libadwaita window with the Windows menu bar, toolbar, formatting bar, ruler, status bar and dialogs over an editable page (keys, input method, caret, selection, pointer, Save); every dialog but Print is answered through the window, with Alt mnemonics, Enter and Esc, drawn by Quill's own renderer at the exact device scale. Built in its own workspace; see [docs/linux.md](docs/linux.md) |

- Every build reads and writes byte-identical `.QLD` and RTF files and has the same document limits, so a document moves between them unchanged. A build that cannot show a property keeps it (see [Build differences](#build-differences)).
- Windows 95 on a 486 uses the Win16 build: the Win32 build's floor is a Pentium (D33).

## Users and scenarios

- **A retro-computing hobbyist** writes a letter on a real 486 or in DOSBox-X, prints it on a period printer and carries the `.RTF` file to a modern machine, where LibreOffice or Word opens it with its fonts, lists, pictures and page numbers intact.
- **A collector restoring a period machine** wants a word processor that installs from floppies with a Windows 3.1 setup program, runs in 4 MB, has a proper Help menu and does not need Win32s.
- **A Windows 98 or ME enthusiast** wants a small native word processor with long file names that opens the same `.QLD` files the family's Windows 3.1 machine writes.
- **Someone on a DOS laptop with no Windows** types a draft in text mode, where bold is bright white and italic is cyan, runs the spelling checker, and prints it later from Windows with the fonts it was given.
- **A modern user** receives an RTF written on Windows 3.1 and expects it to open cleanly; Quill's RTF output is the contract that makes that work.
- **A Rust developer** studies how a `no_std` core, 16-bit code segments and the NE format fit together, and uses Quill's host tools (the reference renderer, the golden tests, the help compiler) without ever starting Windows.
- **A reader of the blog series** follows a link to this repository and wants to see how one core became three programs, without knowing what a selector is.

## Non-goals

- **Windows 3.0, Standard mode or real mode.** 386 Enhanced mode is the floor (D4).
- **Win32s.** The Win32 build targets Windows 9x's own Win32, not the Win32s subset on Windows 3.1.
- **Wide-character (Unicode) Windows APIs and Unicode text.** Documents are Windows ANSI (code page 1252), and both Windows builds use ANSI entry points only, because Windows 9x lacks most `...W` functions (D11).
- **Being Word.** No tables, columns, footnotes, user-defined style sheets (the formatting bar's style box applies eight built-in paragraph presets as direct formatting), fields beyond the four header and footer fields, mail merge, revision marks, OLE or DDE.
- **Grammar checking, hyphenation and a thesaurus.**
- **Network, macros or scripting.** Quill never opens a socket and never runs code from a document. Help file macros are limited to the standard WinHelp browse and button macros Quill's own help uses.

## Scope

Everything in this table is v1 unless the last column says otherwise. When time runs short, features are cut in the order in [docs/roadmap.md](docs/roadmap.md#if-time-runs-out).

| Feature | v1 | Notes |
|---|---|---|
| Fonts and sizes from any installed font | Yes | TrueType, raster and printer fonts |
| Bold, italic, underline, strikethrough | Yes | |
| Superscript and subscript | Yes | Two-thirds size, raised or lowered by a third of the font height |
| Text colour | Yes | The 16 VGA colours in the toolbar; any colour through the Fonts dialog |
| Alignment: left, centred, right, justified | Yes | |
| Indents (left, right, first line, hanging) on the ruler | Yes | |
| Line spacing single, 1½, double; space before and after | Yes | |
| Tab stops: left, centred, right, decimal, with leaders | Yes | Set and dragged on the ruler, or in the Tabs dialog |
| Bulleted lists | Yes | |
| Numbered lists, nine levels | Yes | `1.`, `a.`, `i.`, `A.`, `I.` and `1.1.` styles |
| Manual page breaks | Yes | |
| Headers and footers, different first page | Yes | With page number, page count, date and file name fields |
| Inline pictures: BMP (1, 4, 8, 24 bit) and PNG | Yes | Scaled with the aspect ratio kept; reduced to what the display shows |
| JPEG pictures | Later | When a `no_std` decoder fits the budgets |
| Toolbar, ruler, status bar | Yes | Each can be hidden |
| Caret, mouse and keyboard selection, navigation | Yes | |
| Overtype mode (Insert key) | Yes | |
| Drag and drop of selected text within the document | Yes | Ctrl copies instead of moving |
| Undo and redo, multi-level | Yes | |
| Cut, copy, paste of text, RTF and pictures | Yes | |
| Find and replace | Yes | Match case, whole word |
| Spelling checker with suggestions | Yes | Canadian English by default; American and British too |
| Check spelling as you type | Yes | Wavy underline, drawn in idle time; off by default on Win16 |
| New, Open, Save, Save As, recent files | Yes | |
| Native `.QLD`, RTF subset, plain text | Yes | |
| Write `.WRI` import | Later | Undocumented format; a nice period touch, not essential |
| Printing with page setup | Yes | |
| Print preview, one or two pages | Yes | |
| Zoom (50 % to 200 %) | Stretch | Ships if the milestone it rides on lands with time to spare |
| WinHelp help file (`QUILL.HLP`) | Yes | Built by Quill's own help compiler (D50) |
| About box with free memory and system resources | Yes | |
| Tables, columns, footnotes, user-defined style sheets | No | Each is a large layout feature. The style box offers eight built-in presets that apply direct formatting (see Toolbar and ruler) |

## Components

- **Quill for Windows 3.1** (`QUILL.EXE`, crate `quill-win16`): the Win16 application.
- **Quill for Windows 95** (`QUILL32.EXE`, crate `quill-win9x`): the Win32 application.
- **Quill for DOS** (`QUILLDOS.EXE`, crate `quill-dos`): the DOS application, text mode and graphics mode in one executable.
- **Quill for Linux terminals** (`quill-tty`): the Linux terminal application, drawing `quill-grid`'s screen with crossterm.
- **Quill for Linux desktops** (`quill-gtk`, binary `quill`): the GTK4 application. It shows a document's sheets through `quill-paint`'s pixel surface into a texture of exactly device size, with scrolling, zoom, the Open dialog, drag and drop and the recent files, over the same `App`; its libadwaita chrome (menu bar, toolbar, formatting bar, ruler, status bar with the zoom control of CORE-67 at its right end, dialogs) runs the shared commands, and the page edits: keys, an input method with preedit, a caret that blinks, selection by keys and pointer, and Save, with the clipboard still the editor's own.
- **Quill command** (`quill`, crate `quill-cli`): `quill cat`, `quill convert` and `quill info`.
- **Quill core** (`quill-core`): the document model, editing, undo and layout. Portable `no_std` Rust with no Windows or DOS dependency.
- **Quill formats** (`quill-formats`): the native, RTF and plain text readers and writers.
- **Quill image** (`quill-image`): the BMP and PNG decoders, scaling and colour reduction.
- **Quill module interface** (`quill-modabi`): the C-style, versioned interface between a program and an optional module (structs of `u16` and `u32`, status codes, kinds, the picture kind's operations and the build-id check), with no loader yet. Its packaging for DOS is Desk's `.DMD` format and packer, copied into `dos/module` (CORE-90, DOS-50).
- **Quill spell** (`quill-spell`): the dictionary reader, word checking and suggestions.
- **Quill app** (`quill-app`): the platform-neutral editor controller: commands, the menu tree and its state, status text, and the painting of the page and chrome through a drawing trait.
- **Quill grid** (`quill-grid`): the text-mode screen renderer for DOS text mode and the host's golden screens.
- **Quill dialogs** (`quill-dlg`): the logic of the file dialogs, apart from drawing: DOS 8.3 names and paths, the file types, the name Save As starts with and the one default-extension rule (CORE-64), the state of the widgets the text dialogs share, and the file dialog's state machine over a file system trait: one list of `..`, folders and files with size and date, sortable by each column, with places (drives, named folders, recent files), a Look in path, prefix type-ahead and a 2,000-entry cap (DOS-32).
- **Help compiler** (`quill-help`, command `hlpc`): a host tool that compiles Quill's help sources into a WinHelp 3.1 `.HLP` file (and a DOS help file), with a reader (`hlpdump`) for tests. Built to be lifted out into its own repository later.
- **Dictionary builder** (`quill-dict`): a host tool that builds `.DIC` files from SCOWL word lists.
- **Quill fonts** (`quill-fonts`): `OutlineMeasure`, a layout `Measure` over outline font files with no hinting, the font fallback table and the Quill test fonts.
- **Host tools** (`quill-host`): the reference renderer (`quill-render`) that writes PNG pages, text-mode screens or a text dump, and the test harnesses.
- **Installer** (`installer/`): a Liminal Installer project that builds floppy images: a Windows `SETUP.EXE` that installs the right Windows build for the version it runs on, and a DOS `INSTALL.EXE` for the DOS build.

## Behaviour

Everything below describes the Windows builds; the DOS build follows it with the changes in [DOS text mode](#dos-text-mode) and [DOS graphics mode](#dos-graphics-mode). Editing commands, keys, undo, find, spelling and the files are the same in all of them.

### Window and chrome

- **Dialogs are the system's.** On the Windows builds every dialog is a real dialog window, modal and centred over the main window: About (icon, name, version, copyright, licence, platform and free memory), Find and Replace, Fonts, Tabs, the question about unsaved changes (a message box with Yes, No and Cancel), and the common dialogs for Open, Save As (with the overwrite prompt), Insert Picture and Print. File pickers use `GetOpenFileName` and `GetSaveFileName` with filters for Quill documents, RTF, text and all files (bitmaps and PNG pictures for Insert Picture), the default extension `QLD`, and start in the folder of the last one, which `QUILL.INI` remembers. Text-cell dialogs exist only in the DOS text build, which drives the same commands (D86, D87).
- **Chrome colours.** The Win32 build draws its toolbars, ruler band, the desk around the page and the selection in the system colour scheme (button face, shadow, highlight and text, window, application workspace, highlight), and follows a change of scheme while it runs; the paper stays white with black text. The DOS and Win16 builds keep their fixed colours (W9X-5).
- The main window's title is `Quill - <FILE.EXT>`, with `(Untitled)` for a new document, in the style of Windows 3.1 Write. The Win16 build shows the file name in capitals as DOS stores it; the Win32 build shows the long name as the file system stores it.
- Below the menu bar, top to bottom: the toolbar (a standard row), the formatting bar, the ruler, the editing view with a vertical scroll bar (and a horizontal one when the page is wider than the window), and the status bar. **View** menu items show or hide the toolbar, the formatting bar, the ruler and the status bar, each on its own, and tick when shown; the choices are remembered.
- The editing view shows the page's text width with the left and right margins as white space. Automatic page breaks are a dotted line across the view; manual page breaks a dotted line with the words `Page break`. Headers and footers are not shown in the editing view (they are in the header view, print preview and print), as in Write. Every graphical build starts the view at the top of the first sheet, so the top margin, where the header prints, is the first thing seen (CORE-233).
- The status bar shows `Page N of M`, `Ln N`, `Col N`, `OVR` in overtype mode, and the menu help line for the highlighted menu item while a menu is open. On the Windows 95 build it is the system's status bar (`msctls_statusbar32`, COMCTL32 4.00) with sunken panes and a size grip: the hint or message, the document, `Pg N Ln N Col N`, the font and size, `OVR` and `Modified`, and near the right end the zoom control: a minus sign, the zoom (`Page Width` or a percentage), a plus sign and a Page Width button (a bar at each end of a double-headed arrow, greyed while the zoom is Page Width), flat in one sunken pane with thin lines between them and the size grip's margin inside its right end; a pane's text is set only when it changes, and the page area ends above it (D105). Clicking the zoom box opens a list of Page Width, 200, 150, 125, 100, 75 and 50 percent with the current level ticked, and choosing one sets the zoom as View > Zoom does; the buttons step to the next level down or up from the percentage shown and are greyed when there is none (Page Width is left by either and reached from the list); the Page Width button sets Page Width, as the list's first entry does, and Ctrl+0 does the same from the keyboard; each part has a tooltip (`Zoom Out (Ctrl+-)`, `Zoom (click for the list)`, `Zoom In (Ctrl+numpad +)`, `Page Width (Ctrl+0)`) and, while the pointer is over it, a sentence in the first pane; the box is as wide as `Page Width` in the bar's font needs; a window narrower than 622 pixels leaves the control out, and it does nothing in print preview (CORE-66, CORE-67, CORE-206, W9X-14, W9X-15, W9X-121).
- Only one document is open per window. Starting Quill again opens a second window with its own document (D16).

### Menus and accelerators

Menus follow the Windows 3.1 conventions: a mnemonic on every item, an ellipsis on items that open a dialog, check marks on toggles, unavailable items greyed out (Cut and Copy with no selection, Paste with nothing on the clipboard, Redo with nothing to redo). Every title and item has one mnemonic, written with `&` in the menu tree and shown underlined by Windows (CORE-1). A rule separates the groups of a menu, as in Windows 95 and Word 97: File holds New and Open, then Save and Save As, then Print, Print Preview and Print Setup (and Print to PDF on the Windows 95 build), then the recent files, then Exit; Edit holds Undo and Redo, then Cut, Copy and Paste, then Select All; Character and Paragraph group their alignments, spacings and lists the same way. The DOS text menu draws the same rules (D98).

| Menu | Item | Accelerator |
|---|---|---|
| File | New | Ctrl+N |
| | Open... | Ctrl+O |
| | Save | Ctrl+S |
| | Save As... | |
| | Print... | Ctrl+P |
| | Print Preview | |
| | Print Setup... | |
| | Print to PDF... (Windows 95 build) | Ctrl+Shift+P |
| | 1–4 recent files | |
| | Exit | Alt+F4 |
| Edit | Undo `<action>` | Ctrl+Z, Alt+Backspace |
| | Redo `<action>` | Ctrl+Y |
| | Cut | Ctrl+X, Shift+Del |
| | Copy | Ctrl+C, Ctrl+Ins |
| | Paste | Ctrl+V, Shift+Ins |
| | Delete | Del |
| | Select All | Ctrl+A |
| | Size Picture... | |
| Insert | Page Break | Ctrl+Enter |
| | Picture... | |
| | Page Number, Page Count, Date, File Name, Time | |
| Find | Find... | Ctrl+F |
| | Find Next | F3 |
| | Replace... | Ctrl+H |
| | Spelling... | F7 |
| | Check Spelling as You Type | |
| Character | Regular | F5 |
| | Bold, Italic, Underline | Ctrl+B, Ctrl+I, Ctrl+U |
| | Strikethrough | |
| | Superscript, Subscript | Ctrl+Shift+=, Ctrl+= |
| | Reduce Font, Enlarge Font | Ctrl+Shift+, and Ctrl+Shift+. |
| | Colour... | |
| | Fonts... | |
| Paragraph | Normal | |
| | Left, Centred, Right, Justified | Ctrl+L, Ctrl+E, Ctrl+R, Ctrl+J |
| | Single Space, 1½ Space, Double Space | Ctrl+1, Ctrl+5, Ctrl+2 |
| | Bullets | |
| | Numbering... | |
| | Increase Level, Decrease Level | Alt+Shift+Right, Alt+Shift+Left |
| | Indents and Spacing... | |
| | Tabs... | |
| Document | Header... | |
| | Footer... | |
| | Ruler, Toolbar, Status Bar | |
| | Zoom... (stretch) | Page Width: Ctrl+0 or Ctrl+numpad 0 (Alt+0 on DOS); Zoom In: Ctrl+numpad + (Alt+numpad + on DOS); Zoom Out: Ctrl+minus or Ctrl+numpad minus (Alt+minus on DOS) |
| | Page Layout... | |
| Help | Contents | F1 (outside a dialog) |
| | Search for Help On... | |
| | How to Use Help | |
| | About Quill... | |

**Held keys** (CORE-235, DOS-145). A key held down is sent again by the keyboard or the system; text and movement keep repeating, and a toggle or a command acts once for the press. What repeats: characters, Tab and Enter in the document, the cursor keys, Home and End, the page keys and their selecting forms, Backspace and Delete, Undo and Redo (Ctrl+Z and Ctrl+Y), and the zoom commands. In a menu only the arrows repeat; in a dialog's fields only text (not a space) and the movement and deleting keys do. Everything else acts once however long it is held: Bold, Italic, Underline, Strikethrough, Superscript, Subscript, Regular, the alignments, the spacing commands, Insert (Overtype and Insert), the F-keys, Save, Open, Find, Print, Cut, Copy, Paste and the View items, and Enter, Esc and a letter that answers a question or picks a menu item. Every build applies the one rule (`App::key_repeats`): Windows 95 and 3.x take the repeat from bit 30 of the key message (the key was already down), GTK from a key code pressed again before its release, and DOS from the same BIOS key arriving again with no release seen (the keyboard controller's data port, read when the buffer is empty) within about 1.2 seconds. A shell that cannot tell a repeat from a press passes every key and the old behaviour is the fallback.

The Edit menu names what Undo and Redo will do: `Undo Typing`, `Undo Bold`, `Redo Paste`. Character and Paragraph items show a check mark when the whole selection has that property. Reduce Font and Enlarge Font step through 8, 9, 10, 11, 12, 14, 16, 18, 20, 22, 24, 26, 28, 36, 48 and 72 points. The Insert field items put a field at the caret in the body, a header or a footer: the page number and the page count show their value on the page, in print preview and in print, and `[Page]` and `[Pages]` in the header and footer views (D98). Date and Time show the system's short date and its time without seconds (D104). On Windows 3.x the date is DOS's, written in `WIN.INI` `[intl]` `sShortDate`, and the time is DOS's, written by `iTime`, `sTime`, `s1159` and `s2359` (W16-4); on DOS both are DOS's, in the country's formats (DOS-11). F1 in a dialog opens that dialog's help topic; F1 with a menu item highlighted opens that command's topic.

### Toolbar and ruler

- The toolbar is two rows of 24-pixel buttons, as in Word 6, so a 640 by 480 screen keeps a usable page under them and the ruler (about 350 pixels of page with the title bar, menu bar, both rows, the ruler and the status bar showing). The **standard bar** holds, left to right: New, Open, Save, Print, Print Preview; Cut, Copy, Paste; Undo, Redo; Spelling. The **formatting bar** holds: a paragraph style drop-down, a font name drop-down and a font size drop-down; Bold, Italic, Underline; a colour button with a 16-colour drop-down; Left, Centred, Right, Justified; Bullets, Numbering. On the Windows 95 build the buttons are flat until the pointer is over them, then show a thin two-pixel raised bevel (white and light grey on the top and left, dark grey on the bottom and right, no black); a pressed button and a toggle that the selection has show the same bevel sunken, a toggle on a lighter face, with its picture drawn on that lighter colour and not on a square of the ordinary face; groups of buttons are set apart by an etched line. Earlier builds draw the Windows 3.1 grey three-dimensional style. Toggles show neither state when the selection is mixed. **Tooltips** name each button and each drop-down box (`New`, `Open`, `Save`, `Print`, `Print Preview`, `Cut`, `Copy`, `Paste`, `Undo`, `Redo`, `Spelling`, `Style`, `Font`, `Font Size`, `Bold`, `Italic`, `Underline`, `Font Colour`, `Align Left`, `Centre`, `Align Right`, `Justify`, `Bullets`, `Numbering`), and the status line shows a sentence about it while the pointer is there; the same sentence shows for a highlighted menu item (D100).
- The **style box** is not a style sheet. It offers eight built-in presets (Normal, Heading 1, Heading 2, Heading 3, Title, Quote, List Paragraph, Hanging Indent), each a fixed set of direct paragraph formatting (alignment, indents, space before and after) and character formatting (size, and bold, italic and underline) applied to the paragraphs the selection touches, in one undo step; tab stops, face and colour stay. The box shows a preset's name while the paragraph at the caret matches it exactly. Failing an exact match, a paragraph that matches in indents, list flags, size and the bold, italic and underline bits reads as that preset whatever its alignment, space before and after, and line spacing, so body text written with `\sa120` reads Normal (the first matching preset wins); the box is blank after any other change, such as a different size, a bold, italic or underlined first character or an indent no preset has (CORE-142). Each preset has a next preset, used only by Enter at the end of a paragraph (see Typing). Presets are not stored in the file: RTF carries only the resulting formatting, and `\stylesheet` stays skipped on reading and is not written (D85).
- The font name box lists the fonts on the display and the current printer, TrueType first. A name that is not installed is kept in the document; Windows substitutes a similar font for display and printing. The size box accepts any whole number from 4 to 144.
- The ruler shows the text width in the current units, the first-line indent (upper triangle), the left indent (lower triangle with a box that moves both), the right indent, and the paragraph's tab stops as `L`, `C`, `R` and `D` marks. Clicking the ruler's tab-kind button cycles the kind; clicking the scale adds a stop of that kind; dragging a stop moves it; dragging it off the ruler removes it. Each drag is one undo step. Default stops (every half inch, 1.25 cm in metric) show as ticks after the last set stop.

### Caret and selection

- The caret sits between characters. A selection is a range of characters shown in the system highlight colours, kept unhighlighted while focus is elsewhere. A picture counts as one character.
- **Mouse.** Click places the caret. Drag selects; dragging past the view's edge scrolls. Shift+click extends the selection. Double-click selects a word and its trailing spaces; double-click and drag extends by words. Triple-click selects the paragraph. A click in the white space left of the text selects the line, and a double-click there the paragraph (D99). Clicking a picture selects it and shows its frame.
- **Drag and drop.** Pressing on a selection and dragging shows a dotted insertion caret; releasing moves the selection there, or copies it with Ctrl held. Escape during the drag cancels. Dropping inside the selection does nothing. A move is one undo step.
- **Keyboard.** Arrows move by character and line; Ctrl+Left and Ctrl+Right by word; Home and End to the line's ends; Ctrl+Up and Ctrl+Down by paragraph; Page Up and Page Down by a window; Ctrl+Home and Ctrl+End to the document's ends. Shift with any movement extends the selection. Up and Down keep the caret's horizontal position across short lines. On DOS, Ctrl+Page Up and Ctrl+Page Down scroll the view three lines (a wheel notch) without moving the caret (DOS-134).
- **Typing** replaces the selection. Enter ends a paragraph (and continues a list), Enter on an empty list paragraph ends the list, Shift+Enter breaks the line inside the paragraph, Tab inserts a tab (in an empty list paragraph, it increases the level), Backspace and Delete remove a character or the selection; Backspace at the start of a bullet or numbered paragraph takes it out of the list (no bullet, no hanging indent) and does nothing else, and Backspace at the start of any other paragraph, or Delete at the end of one, joins it to its neighbour, which keeps the format of the paragraph that comes first unless that one is empty (D98), Ctrl+Backspace and Ctrl+Delete remove a word. New text takes the format of the character before the caret, except at a paragraph's start, where it takes the format of the character at the caret (an empty paragraph's own mark), never the previous paragraph's (CORE-40); a format chosen since the caret last moved wins over both. **Enter and the next preset** (CORE-39): Enter at the end of a paragraph, with no selection, that matches a preset exactly gives the new paragraph that preset's next one, in the same undo step as the Enter: Heading 1 to 3 and Title go to Normal, and Normal, Quote, List Paragraph and Hanging Indent continue themselves. Enter in the middle of a paragraph keeps both halves in its format; Enter at the start of a non-empty paragraph leaves an empty paragraph of the same format above and the caret with the text; Enter on an empty heading leaves the heading and starts a Normal paragraph below. A paragraph that no longer matches a preset keeps the format it had. Backspace on the new empty paragraph joins it back and gives the paragraph exactly as it was. **Enter and the start-of-paragraph flags** (CORE-41): the paragraph Enter creates never has "page break before" or "restart numbering"; they stay with the first paragraph that holds text, so one Enter after a manual page break leaves one break, Enter in the middle keeps the break on the first half, and Enter at the start leaves an empty paragraph with none above the text that keeps it. Every other paragraph format still carries over. **Paste and the clipboard limit** (CORE-43, CORE-44): a paste is one undo step however many paragraphs it holds, so one Undo takes it back (and the selection it replaced returns) and one Redo puts it all back. Until the clipboard moves to the document's store, it holds 2,048 bytes: a Copy or Cut of a longer selection is refused with "The document is too large for Quill." on the status line, and nothing changes, so a Cut never deletes text the clipboard did not keep.
- **Overtype.** The Insert key toggles overtype, shown as `OVR`. In overtype, a typed character replaces the one after the caret, except a paragraph mark, which it never replaces. Undo restores the replaced characters.

### Character formatting

- Character formats apply to the selection, or to the typing format when nothing is selected. A character format is a font, a size in half points, a colour, bold, italic, underline and strikethrough flags, and a vertical position (normal, superscript or subscript).
- **Colour...** opens the common colour dialog (`ChooseColor`); the toolbar's colour drop-down offers the 16 VGA colours and `Automatic` (the window text colour). On a 16-colour display, other colours draw as the nearest of the 16 and print as chosen.
- **Fonts...** opens the common font dialog (`ChooseFont`) with effects (strikethrough, underline, colour).
- **Regular** removes bold, italic, underline, strikethrough and superscript or subscript, and keeps the font, size and colour.

### Paragraph formatting

- Paragraph formats apply to every paragraph the selection touches: alignment, left, right and first-line indents, line spacing, space before and after, tab stops, list (none, bullet or numbered, with a level from 1 to 9), and page break before.
- **Justified** paragraphs spread each line's extra width across its spaces, except the last line and lines ending in a line break. A line with no spaces is left-aligned.
- **Tabs.** A tab moves to the next stop after the current position: a left stop starts the text there, a centred stop centres the text up to the next tab or line end on it, a right stop ends it there, and a decimal stop aligns the first `.` (or the end of the number) on it. A leader (dots, hyphens or underline) fills the gap. Past the last set stop, the default stops apply. **Tabs...** sets stops by position, kind and leader, and clears one or all.
- **Bullets** draw `•` (ANSI 0x95) in the paragraph's first character format at the first-line indent; the text starts at the left indent. Turning on a list for a paragraph with no hanging indent sets one of 0.25 inch (0.6 cm) per level.
- **Numbering...** chooses the style per level (`1.`, `1)`, `a.`, `a)`, `i.`, `A.`, `I.`, or outline `1.1.`), the start number and whether this paragraph restarts the numbering. Numbers count the paragraphs of the same list and level since the last restart or the last paragraph at a higher level; a paragraph that is not in the list does not interrupt the count. Numbers update as paragraphs are added, removed and moved.
- **Indents and Spacing...** opens a dialog with the alignment, indents, spacing and the page-break-before check box.

### Headers and footers

- **Document > Header...** and **Document > Footer...** switch the editing view to the header (or footer) and open a small modeless dialog, as Write did: `Distance from top` (or `from bottom`), `Different first page`, buttons that insert the Page Number, Page Count, Date and File Name fields, `Clear` and `Return to Document`. With `Different first page`, the dialog has a `First page` and `Other pages` choice, and an empty first-page header prints nothing.
- Headers and footers hold rich text: any character and paragraph format, tabs, and pictures. They print within the top and bottom margins at their distance from the page edge; a header taller than the space pushes the body text down on that page.
- Fields show their values in print and preview, and their names (`[Page]`, `[Pages]`, `[Date]`, `[File]`) in the header view. The date is the date of printing in the short date format of `WIN.INI` `[intl] sShortDate` (Windows) or the DOS country setting. `[File]` is the file name without its path, or `(Untitled)`.
- Undo works inside the header view; Return to Document keeps the header's undo steps in the document's history.

### Pictures

- **Insert > Picture...** opens a file dialog for `*.BMP` and `*.PNG` and inserts the picture at the caret as one character on the line's baseline. Pictures also arrive by Paste (`CF_DIB` or RTF) and inside opened RTF and `.QLD` files.
- A new picture is shown at its own size at 96 dots per inch, or scaled down to the text width when wider, keeping its aspect ratio.
- **Selecting.** A picture is selected when the selection is exactly its character: click it, or Shift+Left or Shift+Right over it (F8 and an arrow on the text screen). Esc drops the selection, Delete removes it in one undo step, and Tab and Shift+Tab select the next and previous picture. The status line says what it is: `Picture 320 × 200 pixels, 3.33 × 2.08 in (100%)`. A picture inside a larger selection shows a navy border inside its edge. This works on Windows 3.x, on Windows 95 and later and on the DOS graphics screen (CORE-62, W16-16); the text screen has the box in the selection attribute and the status line only, and the Linux viewer does not select pictures yet.
- **Sizing.** A selected picture shows a one-pixel frame outside its edge and eight 7-pixel handles at the corners and sides. Dragging a corner scales with the aspect ratio kept (Shift frees it); dragging a side stretches one way. The picture and the text round it follow the pointer, and the drag is one undo step (CORE-63). **Size Picture...** (not built yet) sets width and height as a percentage or in the current units, with `Keep proportions` checked by default, and `Reset` returns to the original size. Pictures cannot be smaller than 0.1 inch or larger than the page's text area.
- **Display.** Each picture is decoded once to the screen's colour depth and the size it is shown at: dithered to the 16 VGA colours on a 16-colour display, mapped to a shared halftone palette on a 256-colour display, and kept in full colour otherwise. The original file bytes are kept untouched for saving and printing.
- **Windows 3.x.** A BMP is drawn from its own bytes, copied once into global memory (three pictures and 1 MB are kept) and scaled by GDI, so a shrunk picture is not box-filtered or dithered by Quill. A PNG, a bitmap that is top-down, RLE4, 16 or 32 bits a pixel, a damaged one, or one that does not fit in memory shows as the grey box. The store's 160 blocks hold a picture of up to 4 MB, the most a document takes; a picture the store cannot hold is dropped by the reader and the status line says `Picture too large for the memory available` (D200).
- **Bitmap palettes.** An 8-, 4- or 1-bit bitmap's palette is the entries before the pixel offset in the file header (at most the depth allows, or `biClrUsed`), and a pixel whose index is past it is black; neither is a reason to refuse the picture (W9X-10).
- **Memory.** Pictures are decoded when first scrolled into view and dropped from memory when far out of view and memory is needed. A picture that cannot be decoded in the memory available shows as a grey box with the text `Picture too large for the memory available` (or `Picture could not be read` for a damaged file); it is kept in the document and saved unchanged.
- **Printing** decodes the picture again at the printer's resolution in bands, so a large picture prints without needing its whole decoded size in memory at once.

### Layout and scrolling

- Text wraps at the page's text width. Lines break after spaces and hyphens; a word longer than a line breaks at the last character that fits; a picture wider than the remaining line moves to the next line.
- Character widths come from the current printer when one is installed, so the lines, page breaks and page numbers on the screen are the ones that print (D10). With no printer installed, Quill lays out for the screen and says `No printer installed: layout uses screen fonts` in the status bar once.
- A line is as tall as its tallest font or picture, with superscript and subscript raising the line when they reach past it.
- Changing the printer, the paper or the margins lays the whole document out again; the caret stays on the same character.
- **Build note (D81).** In the Windows 9x build the page view and the preview use the screen's 96 dpi layout and printing lays the document out again at the printer's resolution, so a line can break differently on paper than on screen; Print Preview has a status line and the keys PgUp, PgDn, T for one or two pages, P to print and Esc; the Windows 3.x build has the bar of push buttons too (W16-81).
- The view scrolls to keep the caret visible after every edit and caret movement.
- **Window size (Windows 9x build, D97).** The window asks for room for the 8.5 inch page at 96 dpi and opens in the middle of the work area (the screen less the taskbar); on a screen too small for that, such as 640 by 480 or 800 by 600, it opens maximised. Every part of the window is drawn off screen and copied to it in one step, and a key press repaints the page and the status line only, so typing does not flicker.
- **Zoom** (D99). **View > Zoom** offers Page Width, 150, 100, 75 and 50 percent (25 to 200 are kept); it changes only the view and is remembered in `QUILL.INI`. The default, Page Width, is the largest zoom up to 100 percent at which the page fits the window with a little desk either side, so a normal page needs no horizontal scroll bar at 640 by 480 (about 70 percent there). The page and the ruler are centred when the page fits and otherwise sit at the left edge with a horizontal scroll bar; the ruler follows the zoom and the page. On Windows 3.x the page, its text, pictures, shadows, caret, selection and ruler are drawn at the zoom, and clicks, the ruler's markers and both scroll bars work in it (W16-15, W16-16). The page shows only the document's own sheets, so a zoomed-out view of a one-page document has desk below its sheet and no second sheet; after a zoom change the scroll position is held inside the new range, and when the whole of the caret's sheet fits the page area it is shown from its top (W16-80, CORE-141, CORE-233). On Windows 95 the zoomed page is drawn in device pixels too: the text a word at a time in a font made at the zoomed height, each word starting and ending where the layout puts it with the difference spread over the letter gaps inside it, so the page reads the same however the system scales fonts (W9X-80, CORE-140); before each run goes out the painter asks the device context about its font, which ReactOS needs to draw a run in the font that was selected (W9X-82). **The zoom control.** A graphical build's status bar shows the zoom near its right end in a box, and a click on it lists the levels (CORE-66); a minus and a plus button either side of the box step to the next level down or up and grey at the ends (Page Width is left by either), a Page Width button right of the plus sets Page Width and greys while the zoom is Page Width (CORE-206), and a margin of about one character (8 pixels) is left at the bar's right edge (CORE-67); there is no slider and nothing on the toolbars. **Ctrl+0 is Page Width** on the Windows builds (the View > Zoom > Page Width item shows it) and **Alt+0** on the DOS screens, where the BIOS reports no Ctrl with a digit (DOS-105); the Linux terminal client takes Ctrl+0 where the terminal reports it (the kitty keyboard protocol does) and Alt+0 elsewhere. **The zoom keys** (CORE-235, W16-121, W9X-144, DOS-116) step through the same list as the buttons, from the percentage shown (`zoom_step`, one rule: Page Width is left by either key, and at 200 and at 50 percent the key does nothing); View > Zoom In and Zoom Out in the View menu run the same steps with the keys written beside them and are greyed at the ends on the DOS screens (the Windows builds' native menus show them live and a step at an end changes nothing):

| Action | Windows 95 to Me, NT and Windows 3.x | DOS graphics and text | GTK (WP-BAR-1G) |
| --- | --- | --- | --- |
| Zoom In | Ctrl+numpad plus | Alt+numpad plus, Alt+= | Ctrl+numpad plus |
| Zoom Out | Ctrl+minus, Ctrl+numpad minus | Alt+minus, Alt+numpad minus | Ctrl+minus, Ctrl+numpad minus |
| Page Width | Ctrl+0, Ctrl+numpad 0 (Num Lock on) | Alt+0 | Ctrl+9 (Ctrl+0 and Ctrl+numpad 0 are 100 percent there until WP-BAR-1G) |
| Wheel | Ctrl+wheel zooms about the pointer on Windows 98, Me and NT; on Windows 95 the same through the driver's wheel message when Ctrl can be read | The wheel scrolls (INT 33h); no Ctrl+wheel | Ctrl+wheel about the pointer |

Plain Ctrl+plus and Ctrl+= are not zoom keys: Ctrl+= is Subscript and Ctrl+Shift+= is Superscript (LNX-9), so the plus is the keypad's. With Num Lock off the keypad's 0 is Insert, so Ctrl+numpad 0 is not Page Width then and Ctrl+Insert is still Copy and Shift+Insert Paste (the key mapping does not read Insert as a zoom key). On DOS the BIOS gives Alt+minus as scan code 82h, Alt+= as 83h and, on a real keyboard, Alt with the keypad's plus and minus as 4Eh and 4Ah (the Alt+keypad digits are the BIOS's own ASCII-code entry, so Alt+numpad 0 is not used, and Ctrl+numpad 0 arrives as Ctrl+Insert, 92h, which Quill does not read: Alt+0 on the main row is Page Width); DOSBox's BIOS gives the program no key for Alt with the keypad's plus or minus, so Alt+= is the plus that is proved under it (`dos/tools/keys-x11.sh`: Alt+=, Alt+minus and Alt+0 through the real INT 16h end on the host's frame). On the DOS text screens the keys and the menu items set the remembered zoom as View > Zoom does (the text screen is drawn at one size, so nothing changes on screen; QUILL.INI keeps it for the graphics screen). **Tooltips** on every build that has them, in Title Case, with the key: `Zoom Out (Ctrl+-)`, `Zoom (click for the list)`, `Zoom In (Ctrl+numpad +)` and `Page Width (Ctrl+0)` (`Zoom Out (Alt+-)`, `Zoom In (Alt+numpad +)` and `Page Width (Alt+0)` on the DOS graphics screen); the Windows 3.x tip is the popup the toolbar's tips use. The status bar's first field says what each part does while the pointer is over it: `Fit the page to the width of the window`, `Show the page smaller`, `Choose how large the page is shown`, `Show the page larger`. The Windows 95 build (W9X-14, W9X-15), the Windows 3.x build (W16-18, W16-19), the Linux client (LNX-20) and the DOS graphics screens (DOS-44 to DOS-46) have it; the DOS text screens stay with the menu.

- **Scrolling.** The view has a vertical scroll bar. The wheel scrolls three lines a notch (`WM_MOUSEWHEEL` on Windows 98, Me and NT; on Windows 95 the registered message `MSWHEEL_ROLLMSG` that the IntelliPoint driver sends to the window with the focus). With Ctrl held the wheel zooms instead (W9X-145): one step of the zoom list a notch, up for Zoom In and down for Zoom Out, about the pointer, so the point of the page under the pointer stays under it (on Windows 98, Me and NT the message's `MK_CONTROL` flag says Ctrl is down; on Windows 95 the driver's message carries no key state, so Quill reads `GetKeyState(VK_CONTROL)`, which is not verified on a real Windows 95 and is the one place a driver could differ). Page Up and Page Down move the caret and the view by a screenful, the view moving by a screenful when the caret cannot; Ctrl+Home and Ctrl+End go to the ends of the document; typing and movement scroll to keep the caret in view with a little room around it. The view can scroll until the bottom of the last page is in the middle of the window, so the end of the document is never against the window's edge. In the Windows 3.x and DOS graphics builds the bar counts pixels of the page, not lines (CORE-64): zero is the top of the first sheet, with a grey gap of 8 pixels above it and the top margin showing under it, as on Windows 95 and Linux (CORE-233), and the view scrolls until the bottom of the last sheet, with its shadow and a little desk, is at the bottom of the window. An empty or short document has a bar too and scrolls to the foot of its sheet; the arrows move 16 pixels, the track a window less two arrows, and Page Up, Page Down, Ctrl+Home and Ctrl+End move the view when the caret cannot (an empty page, the first or last line), the caret staying where it is until a key moves it. The Windows 3.x build has no wheel. Each page is a sheet with an edge and a shadow, and the desk shows between sheets: the print layout view of Word, so an automatic page break is the gap between two sheets, and a manual one starts the next sheet.

### Spelling

- **Find > Spelling...** (F7) checks from the caret to the end and then from the start, skipping words in all capitals, words with digits, and text marked by Ignore All. Each unknown word is selected and the Spelling dialog shows it with the sentence around it and up to eight suggestions, closest first.
- Buttons: `Ignore`, `Ignore All` (for this session and document), `Change`, `Change All` (every occurrence in the document, one undo step), `Add` (to the user dictionary), `Suggest` (when suggestions are off for speed), `Cancel`. Finishing says `The spelling check is complete.`.
- Suggestions are words within two edits (a letter added, removed, changed, or two letters swapped). The search stops after a time budget so the dialog never pauses for more than about half a second on a 386; when it stops early, the list says `More...` and Suggest continues.
- **Check Spelling as You Type** (Find menu, ticked when on) marks unknown words with a wavy red underline, checked only when the keyboard and the mouse have been quiet for about 0.3 s and only for the lines on the screen, a few words at a time, so typing is never slowed. The rules are the F7 check's: words in all capitals, words with digits, one-letter words, the user dictionary and Ignore All are skipped. The word being typed is not looked at until the caret leaves it, and a mark goes the moment the text under it changes. Scrolling checks the lines that come into view. The marks live in `Store` chunks, 64 at most. It is on by default in the Windows 95 build and off in the others (each build says; `[Quill] AsYouType` overrides it) (D201, D202). On the DOS text screens a marked word has a red background (reverse in mono) and the check runs when no key has come for about 0.3 s; there is no pop-up menu, so F7 is the way to its suggestions (DOS-41). On Windows 3.x the wave and the menu are as on Windows 95 (the menu opens on the right button or Shift+F10; Windows 3.1 answers it through `WM_COMMAND`) (W16-21).
- **Right-click menu (Windows 95 and 3.x).** On a marked word the right button, or the menu key or Shift+F10 with the caret in or against the word, opens a menu of up to five suggestions, closest first, then `Ignore All` and `Add to Dictionary`. A suggestion replaces the word in one undo step; the caret stays where it was in the text.
- Languages are Canadian English (default), American English and British English, set in **Find > Spelling... > Options**. The user dictionary is shared by all languages.
- **Build note (D88).** The dictionary is a file, `EN_CA.DIC`, `EN_US.DIC` or `EN_GB.DIC`, found in the current folder or beside the program, and read whole into memory when the first check begins; the user dictionary is `QUILL.UDC`, a text file with one word a line. The language is set by `[Quill] Language` in `QUILL.INI` (0 Canadian, 1 American, 2 British) until the Options page is built; `[Quill] AsYouType` (`0` or `1`) is the as-you-type setting.

### Files

- **New** starts an empty `(Untitled)` document with the default formats. **Open...** and **Save As...** use the common file dialogs with these types:

| Type in the dialog | Pattern | Read | Write |
|---|---|---|---|
| Quill Documents | `*.QLD` | Yes | Yes |
| Rich Text Format | `*.RTF` | Yes | Yes |
| Text Files | `*.TXT` | Yes | Yes |
| All Files | `*.*` | By content | — |

- A file is read by its content: a RIFF header with form type `QLD1` is native, `{\rtf` is RTF, anything else is plain text.
- **Save** writes the format the document was opened in, or `.QLD` for a new one. Saving as text asks first: `Saving in text format will remove all formatting, pictures, headers and footers. Save anyway?`.
- **Safe save.** Every save of a document, and the copy written when a Windows session ends, goes to a temporary file in the target's folder first: `~QTMP` and the hash of the target's name in hex, then `.TMP` (`~QTMP3A7.TMP`, a legal 8.3 name; the Win32 build uses eight hex digits). The file is written through a 1 KB buffer, flushed, closed and its size checked against the bytes written; only then does it take the original's place. The original is never touched before that, so a full disk, a refused write, a read-only file, a removed disk or a power cut leaves the old file or the whole new one. The status line says `Quill cannot save; the disk may be full.` (also said for a read-only file or a refused write), the document stays modified, and no temporary file is left. Replacing is `MoveFileExA` with `MOVEFILE_REPLACE_EXISTING` on Windows NT and later, `rename` on Linux (the terminal and GTK clients write `.<name>.~tmp` and rename it), and on DOS, Windows 3.x and Windows 95, 98 and Me, where a rename cannot replace, the old file is deleted and the new one renamed, in that order and only once the new file is whole. If that rename fails after the delete, the status line names the temporary file that holds the new document (`Work kept in "~QTMP3A7.TMP".`), and the document stays modified. The temporary file needs room beside the original, so a save to a nearly full floppy that fitted in place can fail now. A leftover temporary file is replaced by the next save of that document and removed when the document is opened, unless the document itself is missing. The Saved state and the modified marker change only after the replace succeeded.
- Closing, New, Open, opening a recent file and Exit ask `Save the changes to LETTER.QLD before a new document starts?` (`before another document opens?` for Open and a recent file, `before Quill closes?` for Exit; `(Untitled)` for a document with no name) with Yes, No and Cancel, Yes the default, when there are unsaved changes. Yes saves first, asking for a name when there is none, and goes on only if the save worked; No goes on without saving, and the changes are dropped only when the command completes: if the Open dialog that follows is cancelled, the document is still modified and still asks on the next close; Cancel does nothing.
- **The end of a Windows session.** The Windows builds also ask when the system logs off or shuts down (`WM_QUERYENDSESSION`), with the question for Exit: Yes saves and lets it go on only if the save worked, No lets it go on, Cancel (or a failed save) refuses it. When the session really ends (`WM_ENDSESSION`), or when the question is refused, a modified document is also written as a recovery file (below), without renaming the document or clearing its modified state. On Windows Vista and later the Win32 build also gives the system a reason to show while it is waiting, `Quill has unsaved changes`, from the time the document is modified until it is saved or closed. Windows 95, 98, Me and NT 4 do not have that call, so the build looks it up at run time. DOS has no such message. See W16-8, W9X-8, W9X-9 and CORE-7.
- **Recovery files** (CORE-10 to CORE-14). A recovery file is a `.QLD` with one `RCOV` chunk (see the native format), written by one routine, `App::save_recovery`, for an autosave, the end of a session and the copy after an internal error alike. It goes beside the document it protects, in a name that carries part of the document's: on DOS and Windows 3.x `~` and the first four characters of the stem and three hex digits of a 12-bit hash of the whole file name (`LETTER1.QLD` is `~LETT3A7.RCV`, `LETTER2.QLD` another slot, `A.QLD` is `~A5F0.RCV`), with `.RC0` for the set-aside copy, `.$$$` for the file a write goes to first, `.LCK` for the lock note, `.RCE` for the copy after an internal error and `.BAD` for a damaged one; on Win32 `~Quill-LETTER.QLD.rcv`, `.rcv.old`, `.rcv.$$$`, `.lck`, `.rce` and `.rcv.bad`; on Linux the same as dot-files (`.~Quill-letter.qld.rcv`). A slot another document already holds, found by the path in its `RCOV` chunk, sends the writer to the next hash value (up to eight). A document with no name, one on a floppy or other removable drive, one in a read-only place and one whose folder refuses the file use the central folder: the person's `RecoveryDir` first, then what the build offers (`RECOVER\` beside the program, then the program's own folder, then the temporary folder on Win32), under `~Q` and six hex digits of the session for a document with no name (`~Quill-Untitled-1A2B3C.rcv`), or under the original's name with the hash taken over its whole path. A copy is written to its `.$$$` file, closed and flushed, then replaces the current copy, so a full disk or a cut leaves the old copy; a `.$$$` file that checks out and is newer than the copy is promoted at the next check, and any other is removed. An autosave that cannot be written leaves `Quill cannot autosave: the disk is full.` on the status line once in a session. The recovery protocol is off until a shell turns it on (`set_recovery`); a shell that has not still writes the end-of-session copy, and offers nothing back.
- **The list.** `QUILL.INI` keeps up to eight originals that have a recovery copy beside them, newest first, in `[Recovery]` (`[Recovery32]` on Win32, `[RecoveryLinux]` on Linux) as `File1` to `File8`, so launch needs no scan of the disks. It is written when a document's first copy is made and again when the last one goes, not only at exit. An entry whose copies are gone leaves silently; one on a drive that is not there stays and is skipped that time. Documents with no name and those in the central folder are not listed: launch reads that one folder. The list, the central folder and the file a question is about share one `Store` chunk, so the Windows 3.x data segment holds only its handle.
- **At most two copies of a document**: the current copy and one set-aside copy. A copy found at Open that the person leaves alone (Open original) becomes the set-aside copy at the first autosave that follows, replacing an older set-aside copy; every autosave after that replaces only the current copy. A successful Save or Save As removes both, and so does Discard. Don't Save at close, New, Open or Exit removes the current copy this session wrote and brings the set-aside copy back as the current one, so the work left alone at Open is where it was. A close or exit with no changes this session leaves what is there. A refused shutdown keeps the copy. A document with only a set-aside copy is still offered, as an older recovered copy.
- **The questions.** At launch, once the window is up and before a file named on the command line opens, `Quill found changes to LETTER.QLD that were not saved, from 25 Nov 2026 at 2:32 PM.` asks Recover, Discard or Later (Recover is the default and Esc is Later), one copy at a time, the list's newest first, then the central folder, then an old `QUILL.RCV`, which is offered as an untitled document with the file's own time and goes with the first Save or Discard. At Open (File > Open, a recent file, the command line), a document that has a copy asks `LETTER.QLD has changes that were not saved, from 25 Nov 2026 at 2:32 PM.` with Recover, Open original and Discard (Recover is the default and Esc is Open original, so Esc never deletes anything). Variants: an untitled document (`Quill found an untitled document that was not saved`), a copy written after an internal error (`Quill closed after an internal error and kept a copy of LETTER.QLD ... Parts of the last change may be missing.`, used when it is newer than the current copy), only an older copy (`An older recovered copy of LETTER.QLD exists, from ...`), both copies (`An older copy is also kept.`), an original saved since (`LETTER.QLD has been saved since then; the copy will open as a new document.`) and a deleted original (`LETTER.QLD is no longer in C:\LETTERS\.`). Recover opens the copy under the original's name, marked modified, when the original is as the copy recorded it (its size and time); a changed or deleted original gives a document with no name, and for a deleted one Save As starts at its old folder and name. A folder that moved with its files is the same document in its new place, and the list follows it. After one launch Recover no more copies are offered that session. A copy that fails its check (Adler-32 over everything after the `RCOV` chunk) is renamed to its `.BAD` name, never offered, and the status line says `Quill found a damaged recovery file and set it aside.`.
- **Idle time and autosave** (CORE-16, CORE-17, CORE-20). A shell calls `App::idle` when its input has been quiet, with how many seconds that is, how many units of work the pass may do and whether a dialog, print, drag or composition is under way; the call does at most that much, in a fixed order that never depends on timing (reflow tail, spelling, visible layout, background layout, word count, dictionary, pictures, autosave: only spelling and autosave exist yet; the DOS screens call it from the wait for a key), and says whether more is ready now, whether an autosave is waiting for its time and whether a copy was written. A job never edits the document and never reads the keyboard, so a typed key is never lost or reordered; the shell asks for `App::take_damage` (the range of the body idle work changed) to repaint. A key or command never runs a job. Spelling is a unit a word. An autosave is due when all of these hold: the recovery protocol is on and the interval is not 0; the document has changes the last autosave does not have; the interval has passed since the document first became dirty after its last save or autosave; the keyboard has been quiet for the idle wait; the editor is in plain editing; and no other window holds the document. What the interval and the wait are depends on the mode (`[Quill] AutosaveMode`, CORE-161), and what is written is always the recovery copy, never the document itself, so Save and Don't Save keep their meaning. **Every N minutes** (`minutes`): the interval is `[Quill] Autosave` minutes and the wait is 2 seconds on DOS, Windows 3.x and Windows 95 and 1 on the Linux, WinUI and web clients. **As you work** (`work`): the interval is a floor of 10 seconds after the document first changed and the wait is `[Quill] AutosavePause` seconds (1 to 30, 3 by default), so a copy is written at the first pause after the floor and again at the first pause after more typing; the `Autosave` minutes stay as the backstop, and an abandoned write (below) is forced only once they have passed since the first change; no `Autosaved.` flash is shown. **Off** (`off`): no copy is written. The defaults (CORE-150, CORE-161): 5 minutes on DOS, Windows 3.x and Windows 95 and 2 on Linux, WinUI and the web; Every N minutes on DOS and Windows 3.x, As you work on Windows 95, Linux, WinUI and the web. `[Debug] AutosaveSeconds` still overrides the interval, with a one-second wait, in every mode. The write is one pass of `save_recovery`, bounded by a poll for input every 4 KB of output rather than by units: when a key waits, the write stops, its temporary file goes, the copy that was there stays, and the key waits at most one poll (20 to 30 ms on a 386, estimated). After two writes input cut short, the third is announced on the status line (`Autosaving LETTER.QLD...`, the shell paints it) and runs to the end on the pass after; in As you work a cut-short write is only tried again at the next pause until the backstop's minutes have passed since the document first changed, so a slow machine is not stopped at every pause. A write that fails backs off one interval and says `Quill cannot autosave: the disk is full.` once a session. A Save, New or Open starts the interval over. The `Autosaved.` flash and the title marker come with the save polish.
- **The lock note.** The first modification of a document that has a name makes a 32-byte note beside it (build, instance, time), which a Save or a close removes. A second window that finds a note whose owner is provably alive (where the shell can say) asks `LETTER.QLD is open in another Quill window. Open it here as well? Changes made here are not autosaved.` (Yes or No, No the default) and, if it opens, writes no recovery copy for that document; one that finds a live note only when it first modifies the document says so on the status line and stops writing copies. A note nothing proves alive, which is what a crash leaves behind, is taken over.
- The recent-file list holds the last four files, most recent first, each once; Open and Save As add to it. On the Windows 95 build it shows in the File menu between Print Setup and Exit, set off by rules, as `1 D:\LETTER.QLD` with the digit as the mnemonic and a long path shortened in the middle (`D:\...\Letters\LETTER.QLD`). Choosing one asks about unsaved changes first. A missing file is removed from the list with the message `Quill cannot find D:\LETTER.QLD.` (D101).
- **Save As** starts with the document's own file name, in the folder of the last file dialog and the type that matches its extension; a document with no name starts empty with the default extension `QLD`. Changing `Save as type` keeps the name and swaps only the extension (`LETTER.QLD` to `LETTER.RTF`) on the Windows 95 build, whose Explorer-style dialog reports the change to a hook (D101). A name typed with no extension gets the chosen type's (`QLD`, `RTF` or `TXT`; All Files gives `QLD`) on both Windows builds, whether or not the system's dialog added it, and the overwrite prompt asks about that final name (W16-5, W9X-7). A name that ends in a dot is kept as typed. A save that fails leaves the document's name, format and recent list as they were.
- A file Quill cannot read says why (`LETTER.RTF is not a document Quill can read.` or `...is too large for Quill.`) and leaves the current document open.
- `QUILL.EXE LETTER.RTF` opens the file; `QUILL.EXE /P LETTER.RTF` prints it on the default printer and exits, as File Manager's Print command expects.

### Clipboard

- Cut and Copy place the selection on the clipboard as `Rich Text Format` (a registered format) and `CF_TEXT`; a selection that is exactly one picture also goes as `CF_DIB`.
- Paste prefers `Rich Text Format`, then `CF_DIB`, then `CF_TEXT`. Pasting replaces the selection as one undo step.
- A selection that ends inside a paragraph pastes inline, with no paragraph end of its own; a selection that includes a paragraph's end keeps it and the paragraph's format. A format that fails to read is skipped for the next one.
- Rich Text Format and a picture bring their own character and paragraph formats, merged into the document's fonts, colours and formats; the first paragraph end of a paste that starts inside a paragraph keeps that paragraph's format. Text takes the format at the caret. Nothing outside Windows-1252 enters a document: the text flavour is Windows-1252 and the reader turns a character in RTF that it has no form for into a question mark.
- Tables are not supported: Quill's Rich Text Format reader has no table, so a table copied from another program pastes as its text.
- A shell that serves the system clipboard (`serve_clipboard`) answers a request after Copy, Cut or Paste with `copy_to` and `paste_from`, and tells the app what its clipboard offers (`set_clip_offers`) so Paste can grey; a shell that does not keeps the app's own clipboard (CORE-230).

### Undo and redo

- Every change is undoable: typing, deletion, overtype, paste, drag and drop, formatting, ruler changes, list changes, picture sizing, header and footer edits, Replace, Replace All and Change All. Typing coalesces into one step until the caret moves by other means, a format changes, or a word ends after more than 32 characters.
- Undo restores the text, the formats and the selection. Redo replays a step undone; a new change after Undo discards what could have been redone. Saving does not clear undo.

### Find and replace

- **Find...** and **Replace...** open the common modeless find and replace dialogs with Match case and Match whole word only; Find has Up and Down. The dialog stays open while editing continues.
- A search starts at the selection and wraps once, then says `Quill has finished searching the document.`; text not found says `Quill cannot find that text.` in a message box (F3 says `Cannot find "<text>".` on the status line). Find searches the body text, or the header or footer when that view is open.
- **Replace** replaces the selection only when it is a match for the text in the box, then finds the next one. **Replace All** replaces every match as one undo step and says `Quill made N replacements.` in a message box. **Find Next** (F3) repeats the last search; before any search it opens the Find dialog.

### Printing and preview

- **Print...** opens the common print dialog for copies, page range (all, selection or pages) and printer. Printing shows `Printing LETTER.QLD on HP LaserJet III on LPT1:` with Cancel, which stops between pages.
- **Print Setup...** opens the print dialog's setup page; changing the printer or paper lays the document out again.
- **Print to PDF...** (Windows 95 build, Ctrl+Shift+P; W9X-90) writes the document to a PDF file without a printer. It asks for a name in Save As (type PDF, the document's own name with `.pdf`, in `C:\Printed`, made when missing) and says `Printed to NAME.PDF` in the status bar. The file holds every page at the document's paper size, in the layout the window shows (`[Print] PdfDpi` in `QUILL.INI` lays it out at 96 to 1,200 dots to the inch instead; default 96, W9X-94), with real text: selectable and searchable, in the standard 14 fonts (Helvetica, Times or Courier, chosen from the face and its family) with every character at the position the layout gave it, headers, footers, list labels and fields with their printed values, underline and strikeout, colour, and pictures as images (W9X-91, W9X-92). The document's text is Windows-1252 and the file keeps it, accents and curly quotes included; a face that is not a look-alike of a standard font prints in the nearest one, so its shapes change and its positions do not. Fonts are not embedded. `QUILL32.EXE FILE /PDF OUT` does the same without a window.
- **Page Layout...** sets the margins (default 1.25 inch left and right, 1 inch top and bottom), the header and footer distances (default 0.5 inch), the starting page number and the measurement units.
- **Print Preview** replaces the view with one or two whole pages, headers and footers included, under a bar of push buttons, each with a tooltip and a status line sentence: Print, Next Page, Previous Page, Two Pages (One Page while two show), Zoom (the whole page, or its actual size with scroll bars) and Close. The toolbars and the drop-down boxes are hidden while it shows. On Windows 95 each sheet's text is drawn in device pixels at the preview's scale, as the zoomed page's is (a word at a time, fitted to the layout), so it reads the same whatever a system does to scaled fonts (W9X-81). Page Up and Page Down change pages; Escape and Close leave it, and the page view comes back with the scroll position and centring it had (D100). The Windows 3.x build has the same bar (Print, Next Page, Previous Page, Two Pages or One Page, Zoom or Whole Page, Close) made of push-button controls, a status line that reads `Page N of M` and the keys, and the keys PgUp, PgDn, Home and End for the page, T, Z and P for the buttons of those names, the arrow keys to turn the pages or, at actual size, to move the sheet, and Esc; it draws the sheets at the percentage that fits them to the window (10 to 100) with the page view's zoomed text, so the words are placed as the layout places them. The toolbars, ruler, boxes and scroll bars are put away while it shows, the editing keys, the pointer in the page and the status bar's zoom control do nothing, and any other command from the menu closes it first. Print is File > Print's dialog, which on Windows 3.x ends in the message that printing is not in this build yet (W16-81).

### Help

- **Help > Contents** (F1) opens `QUILL.HLP` in WinHelp at its contents topic; **Search for Help On...** opens WinHelp's search; **How to Use Help** opens Windows' own `WINHELP.HLP`. F1 in a dialog opens the dialog's topic. Windows 95 opens the same file in its WinHelp.
- A missing help file says `Quill cannot find its help file, QUILL.HLP.`.

### About box

**About Quill...** shows the icon, `Quill`, the version, `A Liminal HQ Labs experiment`, the licence, the mode (`386 Enhanced Mode`), free memory in KB and free system resources as a percentage, in the manner of Program Manager's About box.

On Windows 95, 98 and Me the Win32 build names the system exactly and shows free memory in MB. The platform line names the system it runs on and how it was found (`Windows 98 SE (Win32, 4.10)`, `Windows 11 (Win32, 10.0.26100)`, `Windows 8 under Wine 11.21 (Win32)`): Windows 9x by version and build, Windows NT by version, with 10.0 from build 22000 called Windows 11, and Wine by `wine_get_version`. On Windows NT the version is read with `RtlGetVersion`, because `GetVersionEx` tells a program without a manifest that it runs on Windows 8 from Windows 8.1 on. The memory line is `Free memory: 40 MB of 64 MB`, in GB with one decimal from 4 GB total, read with `GlobalMemoryStatusEx` where the system has it.

### DOS text mode

`QUILLDOS.EXE` starts in the VGA's 80 × 25 colour text mode (80 × 50 with `/50`), in the spirit of WordPerfect 5.1 and Word for DOS 5.5: white on blue, with the page's text in the middle of the screen.

- **Screen.** Row 1 is the menu bar, with the same tree and accelerators as Windows minus the items in the differences table. The last row is the status line: the file name, `Pg 1 Ln 14 Col 22`, the font and size at the caret (`Times New Roman 12`), `OVR`, and `Mod` when there are unsaved changes. Row 2 shows the ruler when it is on, with `L`, `F` and `R` indent marks and `L C R D` tab marks on a scale in tenths of an inch.
- **Selection without Shift.** Shift with a movement key extends the selection as on Windows. F8 also toggles extend mode (`EXT` on the status line), in which plain movement keys extend the selection until F8 or Escape ends it; it exists for keyboards and test harnesses that cannot hold Shift.
- **Menus and dialogs.** Alt (pressed and let go) or F10 activates the menu bar; Alt+letter opens the menu with that mnemonic, which is drawn in a colour of its own; the arrow keys move, a letter picks the item with that mnemonic (1 to 4 the recent files), Enter runs it and Esc leaves. The bar carries the same nine menus as Windows, and File has no Print Preview or Print Setup. Ctrl+I, Ctrl+H and Ctrl+M are the shortcuts of Italic and Replace, not Tab, Backspace and Enter (DOS-1). Dialogs (Open, Save As, Find, Replace, Spelling, Fonts, Indents and Spacing, Tabs, Numbering, Page Layout, About) are shadowed boxes with `< OK >` and `< Cancel >`; Tab moves between fields.
- **Styles.** Text mode has one font in one size, so formats are shown with attributes, as WordPerfect did:

| Format | Colour scheme | Mono scheme |
|---|---|---|
| Regular | Light grey on blue | Normal |
| Bold | Bright white | Bright |
| Italic | Light cyan | Underline on MDA, reverse on VGA mono |
| Underline | Light magenta | Underline |
| Bold italic, bold underline, italic underline | Yellow, light red, light green | Bright with underline |
| Strikethrough | Dark grey | Dim |
| Superscript, subscript | Brown, on the same row | Normal; shown in the status line |
| Text colour | Not shown; named in the status line | Not shown |
| Misspelt word (as you type) | Red background | Reverse |
| Selection | Black on light grey | Reverse |

- **Fonts and sizes** are kept and shown only in the status line for the character at the caret. Text is laid out as if every character were 1/10 inch wide and every line 1/6 inch tall (a 12-point typewriter), so a screen line holds what a 10-pitch printer prints.
- **Paragraphs.** Alignment is drawn in the grid; indents and tab stops round to whole columns; leaders draw as `.`, `-` or `_`. Single and 1½ spacing draw one row per line, double leaves a blank row. Bullets are the CP437 bullet (0x07) and numbers are drawn as text. A manual page break is a row of `═` with `Page break`; an automatic one a row of `─`.
- **Pictures** show as a box of `░` the height the picture would print, at least one row, labelled `[Picture 320 × 200]`. They can be selected, deleted, copied within Quill and sized through Size Picture....
- **Headers and footers** are edited in the header view as on Windows and print in the plain text output.
- **Characters.** The document stays ANSI. The screen shows each character through the CP437 font: characters with no CP437 form show as `▒` and are kept. Alt+keypad codes are taken as CP437 and stored as their ANSI equivalent.
- **Clipboard** is Quill's own unless the shell serves the system's, and it keeps formatting: a copy holds Rich Text Format and Windows-1252 text, up to 1 MB of each, in the document's memory until Quill exits, and a paste inside Quill brings back the fonts, sizes, colours, bold, italic, underline, alignment, lists and pictures it copied (CORE-230). A selection whose Rich Text Format does not fit is refused with the "too large" message and a Cut leaves the text (CORE-44). A paste is one undo step (CORE-43). Paste is greyed while the clipboard offers nothing Quill reads (CORE-231). Running in a Windows DOS box, Copy also puts plain text on the Windows clipboard through the WinOldAp interface, and Paste offers to take text from it.
- **Mouse.** With a mouse driver (INT 33h), the mouse places the caret, selects (drag, Shift+click, double click for a word, triple click for a paragraph, a click left of the text for a line), scrolls three lines a notch with a wheel, opens menus and presses the buttons, check boxes and lists of the dialogs on the text screen. The pointer is a block cell on the text screen and an arrow in graphics mode, where it also works the menus, dialogs, toolbars, ruler and scroll bar (see [DOS graphics mode](#dos-graphics-mode)). The right button does nothing and there is no drag and drop. Everything works without a driver (DOS-13 to DOS-15, DOS-18 to DOS-28); with no INT 33h driver installed and a PS/2 mouse (the BIOS says so), graphics mode reads the mouse itself, wheel included. A Date or Time field put in by a click reads the clock as one put in by a key does.
- **Printing** writes plain text with form feeds, headers and footers to `LPT1`, `LPT2` or a file. PostScript output is a stretch.
- **Files** use DOS 8.3 names (DOS-31) and Quill's own file dialog (DOS-34), the same for Open, Save As and Insert Picture. Its frame is 76 cells wide and the screen's height less five, with the menu bar and the status line still showing: a `Look in:` field with `< Up >` (and `< New folder >` in Save As), a Places pane (the drives, with a floppy or CD-ROM glyph where DOS says so, then Documents when `[Quill] DocFolder` is set, Samples when installed, Quill's own folder, the last folder used and the four recent files), one list of `..`, the folders and the files of the chosen type with their size (`12 KB`, rounded up; `<DIR>` for a folder) and date in the country's order, and a Preview pane with the selected file's kind, size and date. The File name field is first in the Tab order, then the type, Open or Save, Cancel, Look in, Up, New folder, the places and the list (Shift+Tab goes back); Alt with N, T, I, P, L, U or F goes to a control directly. In the list Enter opens a folder or takes a file, Backspace goes up, a letter jumps to the first entry that starts with it (more letters make a longer prefix, the same letter again steps to the next one, and the prefix starts again after about half a second), Alt+1, Alt+2 and Alt+3 sort by name, size or date and the same one again reverses it (folders stay before files and `..` first), and F5 reads the folder again; F4 opens a drop-down. A typed name that is a folder, a drive, `..` or a wildcard goes there or filters the list. Hidden and system files and the recovery and lock files beside a document (`~NAME.RCV`, `.RC0`, `.RCE`, `.$$$`, `.LCK`) are not listed, and a folder lists at most 2,000 entries, ending with `(more files: type a pattern such as R*.QLD)` (DOS-32). Save As adds the type's extension to a name with none (CORE-64), asks before replacing a file (`C:\DOCS\LETTER.QLD already exists. Do you want to replace it?`, No the default), refuses a read-only file (`LETTER.QLD is read-only. Save it under another name.`) and writes the format the typed extension names, otherwise the type's. A path longer than 79 bytes is refused (`That path is too long for Quill.`). The text screens of `QUILLDOS.EXE` show this dialog (its file system is INT 21h, DOS-36); a drive with no disk fails the call instead of stopping on DOS's "Abort, Retry, Fail?" and the dialog says `Drive A: is not ready. Insert a disk and choose Retry.`, `The disk in drive A: is write-protected.` or `The disk in drive A: cannot be read.` and stays where it was. The graphics screen (`/G`, `/G16`) shows the same dialog as a Windows 3.1 window with the same parts and the same keys: bevelled buttons, sunken fields with drop-down arrows, a list of 15 items of 16 pixels with a header row that shows the sort, a scroll bar, a small picture for each folder, drive and kind of file, four-digit years, and the preview's time on a line of its own. A question, a notice or the New folder name opens over it as an ordinary dialog box. The pointer works the graphics dialog (DOS-39): a press on a list entry selects it (a file's name goes in the field), a second press within a double click's time opens a folder or takes a file, a press on a place goes there, on a column header sorts by it (the same one again reverses), on the File name field puts the caret between two characters and gives it the keyboard, on a drop-down's field or arrow opens its list (a press on a row chooses it, a press anywhere else shuts it and does nothing more, and a row lights as the pointer passes over it), and the scroll bar's arrows, track and thumb scroll the list as they do the page, an arrow or the track held repeating; the wheel scrolls the list three entries a notch. `Up`, `New folder`, Open or Save and Cancel act when the button is let go over them and show sunk while held; a box over the dialog takes the pointer and the dialog under it takes none. The text screens have no pointer in this dialog. Print to file keeps the one-line `File name:` prompt on every screen.
- **Autosave and recovery** (DOS-47, CORE-69). All three screens write a recovery copy as the section on recovery files says: every five minutes (`[Quill] Autosave`, 1 to 60; an old 0 reads as off), after two seconds with no key or button, or, with `AutosaveMode=work`, a few seconds (`AutosavePause`, 3 by default) after each pause once ten seconds have passed since the first change (CORE-150, CORE-161), from the idle loop that waits for a key, which asks the app twice a second; a key that arrives while the copy is written stops the write. The copy goes beside the document, or into `RECOVER\` beside `QUILLDOS.EXE` for an untitled document, one on a floppy or a CD-ROM, and one whose folder refuses it; `[Quill] RecoveryDir` names another folder. At start, before a file named on the command line opens, a copy left by a session that ended without saving is offered in a box over the screen: `Quill found changes to LETTER.QLD that were not saved, from 25 Nov 2026 at 2:32 PM.` with `< Recover >  < Discard >  < Later >`; the arrow keys, Tab, the first letter of a button and a click choose, Enter takes the button that is lit (Recover), and Esc is Later. An old `QUILL.RCV` in `RECOVER\` or beside the program is offered the same way, as an untitled document. Opening a document that has a copy asks Recover, Open original or Discard. A save, a close with Don't Save and Discard remove the copy. Not yet on DOS: the copy after an internal error (`.RCE`), and a flush of SMARTDRV's delayed writes.
- **Help** (F1) opens Quill's help viewer on `QUILL.HLX`, with the same topics as the Windows help: links are highlighted, Tab moves between them, Enter follows, Backspace goes back.

### DOS graphics mode

`QUILLDOS.EXE /G` uses 640 × 480 in 16 colours on any VGA, or a VBE linear frame buffer of 640 × 480 or 800 × 600 in 256 colours where one exists. It draws the same chrome as the Windows builds, in the Windows 3.1 look, with real fonts, sizes, colours and pictures.

- **Fonts.** Font families map to Desk's faces: roman to Desk Serif, swiss to Desk Sans, modern to Desk Mono, others to Desk Sans. Each comes as bitmap strikes at 8, 10, 12, 14, 18, 24, 36 and 48 points at 96 dots per inch, in regular, bold, italic and bold italic; other sizes use the nearest strike and say so in the status line. Underline and strikethrough are drawn.
- **Layout** uses those faces' widths, so lines break where Desk's fonts put them, not where Arial or Times New Roman would. The document is unchanged; only line breaks differ.
- **Chrome.** The menu bar, the toolbar and formatting bar (the same buttons and glyphs as the Windows builds, with sunk buttons for what holds at the caret), the ruler, vertical and horizontal scroll bars and the status bar are drawn at 640 × 480; View > Toolbar, Formatting Bar, Ruler and Status Bar, and `[Quill] Toolbar`, `FormattingBar`, `Ruler` and `StatusBar` in `QUILL.INI`, hide and show them and the page takes the room.
- **Menus and dialogs** are drawn as Windows 3.1 controls: pull-downs with check marks, greyed items, accelerator text and underlined mnemonics; dialogs as windows with a title bar, buttons, check boxes, radio buttons, text fields with a caret and list boxes. One control has the visible focus (a dotted rectangle or the caret), and the keys do what they do in text mode.
- **Mouse** (DOS-18 to DOS-20). The pointer follows Windows. A click on a menu title opens its pull-down, which stays open until a click; hovering an item lights it and hovering another title opens that menu; a click on an item runs it; a press on a title, a slide to an item and a release runs it; a click anywhere else closes the menu and goes no further; greyed items do nothing; the keys work on a menu the mouse opened. In a dialog, buttons, check boxes and radio buttons act when the button is let go over them (a button shows sunk while held and the pointer is over it, and a release elsewhere cancels it); a text field takes the focus, and a click puts the caret between two characters, a drag selects, a double click selects a word, and Left, Right, Home, End, Delete, Backspace and Shift with the arrows act at the caret (the text screens too); a list entry is selected by a click, taken by a double click and moved by the wheel; the title bar does nothing and a dialog is not dragged. A toolbar button runs on release, shows sunk while held and, after half a second of rest, a tip with its name and shortcut (`Bold (Ctrl+B)`); the style, face and size boxes and the colour button drop lists (a click chooses, a press elsewhere closes; Up, Down, Enter, Esc and the wheel work in them). On the ruler, the button at the left end goes round the kinds of tab (left, centre, right, decimal) and a click on the scale sets a tab of that kind, a tab stop dragged along the ruler moves and dragged off it is cleared, and the indent markers and right margin drag. The scroll bars' arrows scroll a line (the horizontal bar 16 pixels), their tracks a page and their thumbs follow the pointer, and a held arrow or track repeats after 385 ms, every 55 ms, until the pointer leaves it or, for a track, the thumb reaches it. The vertical bar counts pixels of the page (an arrow is 16 of them) and its thumb stands for the page area's height over the page's, so a page of one line scrolls to the foot of its sheet (CORE-64). The horizontal bar scrolls when a line is wider than the screen (half-inch margins, a wide picture): Shift and the wheel and Ctrl+Left and Ctrl+Right scroll sideways, the caret scrolls into view, and when nothing is wider the bar is drawn disabled, with no thumb and grey arrows, as is the vertical bar when everything fits.
- **Zoom** (DOS-44 to DOS-46). The page is drawn at the zoom, in 16 and in 256 colours: Page Width, 200, 150, 125, 100, 75 or 50 percent (25 to 200 are kept). The layout stays at 100 percent and 96 dots to the inch, so lines break where they do at every zoom; each character, picture, sheet, caret and selection edge is placed where the layout puts it, taken to the screen by the zoom, and the text is drawn from the strike nearest the size the zoom asks for. A strike within an eighth of that size is drawn as it is; further off, its glyphs are resampled to the size wanted by area, with the baseline kept, so 50 percent text is half the height of the 100 percent text and not the 11-pixel strike overlapping itself. Page Width is the largest zoom up to 100 percent at which the sheet fits the 624-pixel page area with 8 pixels of desk either side (74 percent for Letter). The screen starts at 100 percent, the size the strikes are drawn at, unless `QUILL.INI` has a `Zoom`. Clicks, drags, the ruler's scale, markers and tab stops, a picture's handles and the scroll bars work in the zoom: an arrow of the vertical bar moves 16 screen pixels at any zoom, the sideways bar appears when the sheet is wider than the area, and the scroll position stays where the page's top was. **The control** is at the right end of the status line: a minus button, a 76-pixel box with `Page Width` or the percentage, a plus button, a Page Width button (a raised 20-pixel button with the mark of a double-headed arrow between two bars, greyed while the zoom is Page Width, sunk while held) and a margin of 8 pixels at the screen's edge (CORE-67, CORE-206); resting the pointer on a part shows its tooltip above it (`Zoom Out (Alt+-)`, `Zoom (click for the list)`, `Zoom In (Alt+numpad +)`, `Page Width (Alt+0)`). A click on the box lists the levels above it with the current one lit (Up, Down, Enter, Esc and the wheel work in the list, and a press elsewhere closes it); a button steps to the next level down or up from the percentage shown on release, shows sunk while held, and is greyed at the ends (50 and 200); the margin does nothing. **From the keyboard**, View > Zoom (Enter on it, or its mnemonic `Z`) opens the same list, then Up, Down and Enter; **Alt+0** is Page Width on both DOS screens (the BIOS gives AL 0 and AH 81h; the zoom list's first row shows `Alt+0`), **Alt+numpad plus** (or Alt+=) and **Alt+minus** step up and down as the buttons do (DOS-116), and the zoom list reaches every other level (DOS-105). The control is left out when the View menu hides the status bar.
- Pictures are dithered to the 16 colours, or to the halftone palette in 256 colours. Everything else is as in text mode, drawn in graphics.

### Build differences

Every build keeps every property of the document. The table says what each shows and does.

| Feature | Win16 | Win32 | DOS text | DOS graphics |
|---|---|---|---|---|
| Fonts and sizes | Yes | Yes | Status line only | Desk faces at fixed strikes |
| Bold, italic, underline, strikethrough | Drawn | Drawn | Attributes | Drawn |
| Superscript and subscript | Drawn | Drawn | Attribute, same row | Drawn |
| Text colour | Nearest of 16 on VGA | As the display allows | Status line only | Nearest of 16 or 256 |
| Alignment, indents, tabs, lists | Exact | Exact | Rounded to columns | Exact for Desk metrics |
| Line spacing 1½ | Exact | Exact | As single | Exact |
| Layout matches the printer | Yes (printer metrics) | Yes | 10-pitch typewriter | Desk metrics |
| Pictures | Dithered to the display | Full colour where possible | Placeholder boxes | Dithered |
| Headers and footers | Header view, preview, print | As Win16 | Header view, text print | As Win16, text print |
| Toolbar and ruler | Yes | Yes | Ruler as text | Yes, with tips and drop-down lists (no tab-kind button) |
| Print preview | Yes | Yes | No | Yes |
| Zoom (stretch) | Yes | Yes | No | Yes (DOS-44) |
| Zoom box in the status bar, with minus and plus | Yes (W16-18, W16-19) | Yes (W9X-14, W9X-15) | No | Yes (DOS-46) |
| Caret | Blinks at the system's rate (W16-17) | Blinks at the system's rate (W9X-12) | Steady | Steady |
| Printing | Windows printer drivers | Windows printer drivers, and Print to PDF with a text layer (W9X-90) | Plain text | Plain text; PostScript stretch |
| Clipboard | Windows: RTF, DIB, text | Windows: RTF, DIB, text | Own; Windows text in a DOS box | As text mode |
| Spelling as you type | Off by default; wave and pop-up menu (W16-21) | On by default | Off by default; a red background, reverse in mono (DOS-41) | Off by default; a red wave (DOS-43) |
| Help | WinHelp 3.1 | WinHelp 4 (same file) | Own viewer, `QUILL.HLX` | As text mode |
| File names | 8.3 | Long names | 8.3 | 8.3 |
| Common dialogs | `COMMDLG.DLL` | `COMDLG32.DLL` | Quill's own | Quill's own |
| Plain text code page | ANSI | ANSI | CP437, or ANSI by choice | As text mode |

### Errors

- Every error that stops a command shows a message box naming the file or action and the cause in plain words, then leaves the document as it was. There are no error codes in messages.
- **Out of memory.** When memory runs out during an edit, the edit is refused, the document is unchanged, and Quill says `There is not enough memory to complete this operation. Close other applications or save and close documents, then try again.`. Pictures out of view are dropped from memory before Quill gives up. Saving needs only a fixed buffer, so it always remains possible.
- **Internal error.** If Quill reaches an internal error it cannot recover from, it says `Quill has met an internal error and must close. Your changes were kept in ~LETT3A7.RCE.` when the emergency copy (the `.RCE` file of the document's recovery slot; see Recovery files) is written, or without the second sentence when it does not, and exits. The copy never replaces the document's good recovery copy. The shells write it as they gain autosave.

## Interfaces

### Command line

The same forms work for `QUILL32.EXE` and `QUILLDOS.EXE`; options are case-insensitive and may start with `-` or `/`.

| Form | Meaning |
|---|---|
| `QUILL.EXE` | Open a new, untitled document |
| `QUILL.EXE FILE` | Open `FILE`, of any supported type |
| `QUILL.EXE /P FILE` | Print `FILE` on the default printer (DOS: to `LPT1`), then exit |
| `QUILL32.EXE FILE /PDF OUT` | Write `FILE` as a PDF to `OUT` (Windows 95 build), then exit |
| `QUILL.EXE /SELFTEST SCRIPT` | Run a test script, write results to `QUILL.LOG` and exit (on Windows 3.1, exit Windows too). Test builds only; see [docs/testing.md](docs/testing.md) |
| `QUILLDOS.EXE /50`, `/G`, `/G16`, `/T`, `/MONO` | 80 × 50 text mode (a screen with no 50-row mode stays at 80 × 25; the switch is not saved), graphics mode (`/GFX` is the same as `/G`; a VBE 640 × 480 mode with 256 colours if the card has one, else mode 12h), graphics mode 12h only, text mode over a saved `Graphics=` setting, the mono attribute scheme (D124, DOS-3) |

### Native format (`.QLD`)

A RIFF file (little-endian, chunks padded to even length) with form type `QLD1`. Unknown chunks are skipped, so a later minor version adds chunks without breaking older readers. A file whose `HEAD` major version is newer is refused with `LETTER.QLD was written by a newer version of Quill.`. v1 writes version 1.0.

| Chunk | Contents |
|---|---|
| `HEAD` | Version (major `u16`, minor `u16`), story count (`u16`), flags (`u16`, zero) |
| `DOCP` | Paper width and height, margins left, right, top and bottom, header and footer distances, all `i32` twips; first page number (`u16`); flags (`u16`: bit 0 different first page); units (`u8`: 0 inches, 1 centimetres); spelling language (`u8`: 0 en-CA, 1 en-US, 2 en-GB) |
| `FONT` | Font table: count (`u16`), then per font a family (`u8`: 0 don't care, 1 roman, 2 swiss, 3 modern, 4 script, 5 decorative), pitch (`u8`), character set (`u8`) and name (`u8` length, ANSI, at most 31) |
| `COLR` | Colour table: count (`u16`), then `r`, `g`, `b`, `u8` each. Index 0 is reserved for Automatic |
| `CFMT` | Character formats: count (`u16`), then per format a font index (`u16`), size in half points (`u16`), colour index (`u8`), vertical position (`u8`: 0 normal, 1 superscript, 2 subscript) and flags (`u16`: bit 0 bold, 1 italic, 2 underline, 3 strikethrough) |
| `TABS` | Tab sets: count (`u16`), then per set a stop count (`u8`, at most 16) and per stop a position (`i32` twips), kind (`u8`: 0 left, 1 centre, 2 right, 3 decimal) and leader (`u8`: 0 none, 1 dots, 2 hyphens, 3 underline) |
| `LIST` | List definitions: count (`u16`), then per list nine levels of style (`u8`), start (`u16`) and indent (`i32` twips) |
| `PFMT` | Paragraph formats: count (`u16`), then per format an alignment (`u8`: 0 left, 1 centre, 2 right, 3 justify), line spacing (`u8`: 0 single, 1 one and a half, 2 double), flags (`u16`: bit 0 bullet, bit 1 page break before, bit 2 numbered, bit 3 restart numbering), list index (`u16`), level (`u8`, 0–8), tab set index (`u16`, `0xFFFF` for none), left, right and first-line indents and space before and after (`i32` twips each) |
| `PICT` | One per picture, in order: kind (`u8`: 0 BMP, 1 PNG), width and height in pixels (`u16` each), shown width and height (`i32` twips each), then the original file bytes |
| `LIST` container, form type `STRY` | One per story, in the order body, header, footer, first-page header, first-page footer (empty header and footer stories are left out), each holding the chunks below. A reader tells it from the list definitions chunk by its first bytes: the form type `STRY`, where a definitions chunk starts with a count of at most 64 |
| `STHD` | Inside a story: story kind (`u8`: 0 body, 1 header, 2 footer, 3 first header, 4 first footer) and character count (`u32`) |
| `TEXT` | Inside a story: ANSI text. Paragraphs end with `0x0D`, a line break is `0x0B`, a tab `0x09`, and `0x01` marks an inline object. The last paragraph has its `0x0D` |
| `RUNS` | Inside a story: count (`u32`), then per run a length in characters (`u32`) and a `CFMT` index (`u16`). Lengths sum to the character count |
| `PARA` | Inside a story: one `PFMT` index (`u16`) per paragraph |
| `OBJS` | Inside a story: one entry per `0x01` in its text, in order: kind (`u8`: 0 picture, 1 page number, 2 page count, 3 date, 4 file name) and a picture index (`u16`, zero for fields) |
| `PARA` and `RUNS` order | A story's chunks are written in the order `STHD`, `TEXT`, `RUNS`, `PARA`, `OBJS`, and a reader needs `TEXT` before the others |
| `INFO` | Optional: title, author and comments, each a `u8` length and ANSI bytes |
| `RCOV` | Only in a recovery file, right after `HEAD`: see below. Every reader skips it, so any build opens a recovery file as an ordinary document |

#### Recovery file

A recovery file is a complete `.QLD` with one `RCOV` chunk right after `HEAD`; nothing else differs, and the bytes after the chunk are exactly what `qld::write` gives for the document. The chunk holds, in order: version (`u16`, 1), flags (`u16`: 1 untitled, 2 emergency, 4 end of session, 8 autosave, 16 path is UTF-8, 32 in the central folder), the document's own format (`u8`: 0 `.QLD`, 1 RTF, 2 text), the writer (`u8`: 0 DOS text, 1 DOS graphics, 2 Windows 3.x, 3 Win32, 4 terminal, 5 GTK, 6 WinUI, 7 web), the Quill version (`u8` major, `u8` minor), when it was written (`u32`, DOS packed date and time), `Doc::revision` (`u32`), the original's size (`u32`, `0xFFFFFFFF` for none) and its DOS date and time (`u32`), the caret and the top line (`u32` each), the Adler-32 of every byte after the chunk (including its pad byte) to the end of the file (`u32`), how many bytes that is (`u32`), and the original's full path (`u16` length of at most 259, then the bytes; the chunk's size is 42 plus the length, and RIFF pads it to even).

`qld::read_header` reads the chunk from the first 338 bytes and checks it against the file's own sizes; `qld::verify` also checks the checksum and that the file ends where its `RIFF` chunk does, in 1,024-byte reads, and allocates nothing. A file that is not a recovery file, has a path length over 259, a size that disagrees with it, or any damaged byte after the chunk is `BadFile`; a newer `HEAD` major or `RCOV` version is `Newer`. The header's own fields are not under the checksum, so a caller that uses a recovery file calls `verify` and then reads the document with `qld::read`.

The writer computes the checksum in its counting pass (the first of its two passes), so a recovery file is written once and never read back. `qld::write_with_progress` and `qld::write_recovery` take a hook, called about every 4,096 bytes with the bytes done and the file's size (0 while the first pass counts); a hook that returns false stops the write at once, and the result is `FormatError::Write`, the same as a full disk.

### RTF subset

Quill reads RTF 1.x as written by Word 2 to Word 2003, WordPad and LibreOffice, keeping what the document model holds and skipping the rest safely, and writes a canonical subset those programs read.

| Area | Read | Written |
|---|---|---|
| Header | `\rtf1`, `\ansi`, `\ansicpgN` (1250, 1251, 1252, 1253, 1254 and 1257; the double-byte pages 932, 936, 949, 950 and 1361 read as `?`), `\deff`, `\fonttbl` (with `\fcharsetN` per font),  `\colortbl`, `\stylesheet` (skipped), `\info` (`\title`, `\author`, `\doccomm` kept) | `\rtf1\ansi\ansicpg1252\deff0`, font table, colour table, `\info` when set |
| Document | `\paperw`, `\paperh`, `\margl`, `\margr`, `\margt`, `\margb`, `\headery`, `\footery`, `\titlepg`, `\pgnstart` | The same |
| Characters | `\plain`, `\f`, `\fs`, `\b`, `\i`, `\ul` (and other underline kinds as `\ul`), `\ulnone`, `\strike`, `\super`, `\sub`, `\nosupersub`, `\up` and `\dn` (as super and sub), `\cf`, with their `0` forms | `\plain`, `\f`, `\fs`, `\b`, `\i`, `\ul`, `\strike`, `\super`, `\sub`, `\cf` |
| Paragraphs | `\pard`, `\par`, `\ql`, `\qc`, `\qr`, `\qj`, `\li`, `\ri`, `\fi`, `\sb`, `\sa`, `\sl` with `\slmult` (to the nearest spacing), `\pagebb`, `\page` | `\pard`, `\par`, alignment, indents, spacing, `\pagebb` |
| Tabs | `\tx`, `\tqc`, `\tqr`, `\tqdec`, `\tldot`, `\tlhyph`, `\tlul`, `\tlth` (as underline) | The same, without `\tlth` |
| Lists | Word 97 `\listtable`, `\listoverridetable`, `\ls`, `\ilvl` (bullet and number formats); Word 6 `{\*\pn ...}` with `\pnlvlblt`, `\pnlvlbody`, `\pnlvl`, `\pndec`, `\pnlcltr`, `\pnucltr`, `\pnlcrm`, `\pnucrm`, `\pnstart`; `{\pntext ...}` and `{\listtext ...}` skipped | Both: `\listtable` and `\ls` for Word 97 and later, with `{\*\pn ...}` and `{\pntext ...}` for older readers |
| Headers and footers | `{\header ...}`, `{\footer ...}`, `{\headerf ...}`, `{\footerf ...}`, `{\headerr ...}` and `{\footerr ...}` (as the plain ones); `\headerl` and `\footerl` skipped | `{\header}`, `{\footer}`, and `{\headerf}` and `{\footerf}` with `\titlepg` |
| Fields | `{\field{\*\fldinst PAGE}...}`, `NUMPAGES`, `DATE`, `FILENAME` become Quill fields; `\chpgn` becomes Page Number; other fields keep their result text | `{\field{\*\fldinst PAGE}{\fldrslt 1}}` and the like |
| Pictures | `{\pict ...}` with `\dibitmap`, `\wbitmap` (with `\wbmbitspixel`, `\wbmplanes`, `\wbmwidthbytes`), `\pngblip`; `\picw`, `\pich`, `\picwgoal`, `\pichgoal`, `\picscalex`, `\picscaley`; hex or `\bin` data; inside `{\*\shppict ...}` preferred to `{\nonshppict ...}`. `\wmetafile`, `\emfblip`, `\jpegblip` and others become a `Picture could not be read` placeholder that is kept as an empty box | PNG pictures as `{\*\shppict{\pict\pngblip ...}}` with a `{\nonshppict{\pict\dibitmap0 ...}}` copy for older readers; BMP pictures as `\dibitmap0` |
| Special characters | `\line`, `\tab`, `\~`, `\-`, `\_`, `\emdash`, `\endash`, `\lquote`, `\rquote`, `\ldblquote`, `\rdblquote`, `\bullet`, `\'hh` (in the font's code page, else the document's), `\uN` with `\ucN` (to Windows-1252 where it has the character, `?` otherwise) | `\line`, `\tab`, `\'hh` above 0x7F, `\{`, `\}`, `\\` |
| Skipped safely | Every other `{\* ...}` destination, `\object`, footnotes, annotations, tables (each cell becomes text followed by a tab, each row a paragraph) | — |

Text codes in RTF: a `\'hh` escape, and a byte of 8 bits written as it is, mean a character of the font's code page when the font table gives the font a `\fcharsetN` (0 is 1252, 238 is 1250, 204 is 1251, 161 is 1253, 162 is 1254, 186 is 1257), and of the document's `\ansicpgN` otherwise (1252 when it is absent or not one of those). The fonts with a double-byte charset (128, 129, 130, 134, 136) give one `?` for each two-byte character. The document holds Windows-1252 bytes, so a character that page has no form for becomes `?`; `\uN` is the same: a signed 16-bit value, a surrogate pair is one character, and all 27 characters Windows-1252 has in 0x80 to 0x9F are kept. Each replacement is counted (`rtf::read_counted`), so a shell can say how many characters were lost. A byte a code page leaves undefined keeps its number, as Windows-1252's five do (CORE-61). Characters are read as Unicode first and narrowed to Windows-1252 in a step of their own, which the UTF-8 model replaces (CORE-63).

Limits while reading: groups nest at most 64 deep (deeper groups are skipped), the font and colour tables hold 64 and 256 entries, and a control word longer than 32 characters or a number outside `i32` ends the read with `...is not a document Quill can read.`.

### Plain text

- Read as ANSI. CRLF, LF and CR each end a paragraph; a form feed starts a new paragraph with a page break before it; other control characters below 0x20 except tab are dropped.
- Written as ANSI with CRLF after every paragraph and line break, a tab for each tab, and a form feed for each page break. Pictures, headers and footers are left out.
- The DOS build reads and writes CP437 by default, converting to and from ANSI; Save As also offers `Text Files (ANSI)`. A character with no CP437 form is written as `?`.

### Clipboard formats

| Format | Read | Written |
|---|---|---|
| `Rich Text Format` (registered) | Yes, preferred | Yes |
| `CF_DIB` | Yes | When the selection is one picture |
| `CF_TEXT` | Yes | Yes |

### Dictionary format (`.DIC`)

Built by `quill-dict` from SCOWL (see [Security and privacy](#security-and-privacy) for the attribution it carries) and read in 32 KB pages, so only the pages a check touches are in memory.

| Part | Contents |
|---|---|
| Header (64 bytes) | Magic `QDIC`, version (`u16`), language tag (8 ANSI bytes, `en-CA`), word count (`u32`), node count (`u32`), root edge index (`u32`), page size (`u16`, 32768), flags (`u16`) |
| Edges | The word list as a minimized acyclic automaton (a DAWG): each edge is 4 bytes, a character (8 bits, ANSI lower case), an end-of-word flag, a last-edge-of-node flag and the index of the child node's first edge (22 bits). Edges of one node are contiguous and sorted |
| Notice | The SCOWL copyright and licence text, as ANSI, after the edges |

The user dictionary, `CUSTOM.DIC`, is plain ANSI text with one word per line, sorted, at most 5,000 words, beside `QUILL.INI`.

### Help source format

Quill's help is written in `.qhm` files (Quill help markup, a small Markdown dialect) and a `help.toml` project file, compiled by `hlpc` into `QUILL.HLP` (WinHelp 3.1) and `QUILL.HLX` (DOS). The full grammar lives with the compiler; the shape is:

| Construct | Means |
|---|---|
| `# Opening a document {#open_document}` | A topic with its title and context string |
| `keywords: open; files; RTF` | Search keywords for the topic |
| `browse: files:020` | Browse sequence and position |
| `**bold**`, `*italic*`, `` `code` `` | Character formats |
| `[Saving a copy](#save_as)` | A jump to a topic |
| `[pica](^def_pica)` | A popup of a topic |
| `![Toolbar](toolbar.bmp)` | A bitmap, inline |
| `- item`, `1. item` | Lists, drawn with hanging indents |
| `> Note text` | A shaded note paragraph |

`help.toml` names the title, the contents topic, the files, the map from context strings to the numbers Quill passes to `WinHelp` (`HELP_CONTEXT`), the main window's size and colours, and the browse-button macro.

### Settings file

Settings live in `QUILL.INI` in the Windows directory (both Windows builds share it) or, for the DOS build, beside `QUILLDOS.EXE`. It is read at start and written when a setting changes. The Win32 build reads and writes its settings one key at a time through the profile functions, so what other builds keep in the same file is left alone, and keeps its recent files in `[Recent32]`, because they may be long names the Win16 build cannot open. It also reads `[Print] PdfDpi` (96 to 1,200, default 96), the resolution Print to PDF lays the pages out at (W9X-94); nothing writes it. The Linux builds keep `QUILL.INI` in `$XDG_CONFIG_HOME/quill` (`~/.config/quill`), and the GTK client also keeps its window's size in `gtk.ini` beside it.

## Settings and defaults

| Setting | Default | Meaning |
|---|---|---|
| `[Quill] Toolbar`, `Ruler`, `StatusBar` | `1` | Show each part of the chrome |
| `[Quill] Units` | From `WIN.INI` `[intl] iMeasure` (DOS: country code) | Ruler and dialog units: `in` or `cm` |
| `[Quill] Window` | Windows' default | Last window position and size as `x y cx cy maximized` |
| `[Quill] Font` | `Times New Roman` | Font for new documents |
| `[Quill] Size` | `12` | Font size for new documents, in points (4–144) |
| `[Quill] Margins` | `1800 1800 1440 1440` | Left, right, top, bottom margins for new documents, in twips (0–14400 each, leaving at least one inch of text width) |
| `[Quill] Backup` | `0` | When `1`, Save keeps the previous file as `<name>.BAK` |
| `[Quill] PictureMemory` | `1024` (Win16), `16384` (Win32), `4096` (DOS) | Most memory, in KB, decoded pictures may hold at once (256–65536) |
| `[Quill] Zoom` | `Width` (DOS graphics: 100) | Zoom: `Width` (the page fits the window, never larger than 100 percent) or a percentage from 25 to 200 |
| `[Quill] Blink` | `1` | Windows builds: `0` keeps the caret steady; otherwise it blinks at the system's rate (`GetCaretBlinkTime`), steady when that is zero or infinite. A `/PLAY` run does the same, and the Win32 build and the Linux client also read the environment variable `QUILL_NO_BLINK`, set to anything (W9X-12, W16-17, LNX-15). The Win32 build never writes it back; the Win16 build keeps `Blink=0` in the file it rewrites at exit, since that file is its own |
| `[Spelling] Language` | `en-CA` | `en-CA`, `en-US` or `en-GB` |
| `[Quill] AsYouType` | `0` (Win16, DOS), `1` (Windows 95) | Check spelling as you type |
| `[Spelling] IgnoreCaps` | `1` | Skip words in all capitals |
| `[Recent] File1`–`File4` | Empty | Recent files, most recent first (Win16 and DOS) |
| `[Recent32] File1`–`File4` | Empty | Recent files for the Win32 build and the GTK client |
| `gtk.ini` `[Window] Width`, `Height`, `Maximized` | `900`, `700`, `0` | The GTK client's window size in logical pixels (320 to 16384) and whether it was maximized |
| `[Quill] Rows` | `25` | DOS text mode rows: `25` or `50`; the same as `/50`, and kept when Quill saves its settings (D124) |
| `[Quill] Graphics` | `0` | DOS: `0` text mode, `1` graphics (the best 640 × 480 the card gives), `16` graphics in mode 12h only; kept when Quill saves its settings (DOS-3) |
| `[Quill] Scheme` | `colour` | DOS text mode attributes: `colour` or `mono` |
| `[Debug] Trace` | `0` | When `1`, write a trace to `QUILL.LOG` beside the program |
| `[Quill] Autosave` | `5` (DOS, Win16 and Win32), `2` (Linux) | Minutes between recovery copies in Every N minutes, and the backstop in As you work, 1 to 60; in a file with no `AutosaveMode`, 0 reads as Off (the old meaning). Written only when it differs from the build's default. The idle pass's autosave timer measures it (CORE-17, CORE-150) |
| `[Quill] AutosaveMode`, `AutosaveMode32`, `AutosaveModeLinux`, `AutosaveModeWinUI` | `minutes` (DOS and Win16), `work` (Win32, Linux, WinUI) | When the recovery copy is written: `off`, `work` (As you work) or `minutes` (Every N minutes), case-insensitive; anything else is the default. One key for each family of builds, because Windows 3.x and Windows 95 share a `QUILL.INI` and take different defaults; a build ignores the others' keys. Written only when it differs from the build's default (CORE-161) |
| `[Quill] AutosavePause`, `AutosavePause32`, `AutosavePauseLinux`, `AutosavePauseWinUI` | `3` | Seconds of quiet before an As you work copy, 1 to 30; the same families as the mode. Written only when it differs from the default (CORE-161) |
| `[Quill] RecoveryDir`, `RecoveryDir32`, `RecoveryDirLinux` | Empty | The central recovery folder; empty is the build's own (`RECOVER\` beside the program) |
| `[Recovery] File1`–`File8`, `[Recovery32]`, `[RecoveryLinux]` | Empty | Originals that have a recovery copy beside them, newest first; written by Quill |
| `[Debug] AutosaveSeconds` | Absent | Test hook: the autosave interval in seconds, with a one-second idle wait |

## Security and privacy

- Quill reads and writes only the files the user opens, saves or inserts, the temporary file beside a file being saved, `QUILL.INI`, `CUSTOM.DIC`, the dictionaries and help files it ships, the recovery files and lock notes beside a document or in the recovery folder, the emergency copy after an internal error, and `QUILL.LOG` when tracing is on.
- It never opens a network connection, never starts another program except WinHelp for its own help, and never runs anything embedded in a document. Embedded objects and metafiles in RTF are skipped, not decoded.
- The RTF, native, text, BMP, PNG, dictionary and help readers treat every file as untrusted: every count, length, dimension and nesting depth is checked against the limits below before memory is reserved, and each reader is fuzzed (see [docs/testing.md](docs/testing.md)).
- The trace records commands and timings, never document text.
- **Attribution.** The dictionaries are built from SCOWL and VarCon (copyright Kevin Atkinson and the authors of the word lists they draw on, under SCOWL's permissive licence, which allows redistribution with its notices kept; the exact notice text is copied from the SCOWL release used and checked when `dict/` is added). Each `.DIC` carries the notice, the About box names it, and `docs/` keeps the full text. The DOS graphics faces are Desk's, under the SIL Open Font License 1.1.

## Limits

| Limit | Value | At the limit |
|---|---|---|
| Body text | 1,048,575 characters | The edit or open is refused: `The document is too large for Quill.` |
| Header and footer text | 4,096 characters each | As above, in the header view |
| Paragraphs | 32,767 | As above |
| Characters in one paragraph | 65,535 | Typing or pasting past it is refused with the same message |
| Fonts in one document | 64 | Further fonts map to the nearest family in the table |
| Colours in one document | 256 | Further colours map to the nearest in the table |
| Character formats | 4,096 distinct | Unused formats are collected first, then the edit is refused |
| Paragraph formats | 1,024 distinct | As character formats |
| Tab stops | 16 per paragraph, 256 distinct tab sets | The Tabs dialog and ruler refuse a 17th stop |
| Lists | 64 list definitions, 9 levels | Further lists reuse the last definition |
| Pictures | 256 per document, each at most 4,096 × 4,096 pixels and 4 MB of file data | Larger pictures are refused on insert; in an opened file they become placeholders and are kept |
| Decoded picture memory | `[Quill] PictureMemory` | Pictures out of view are dropped; one that still does not fit shows the placeholder |
| Undo history | 100 steps or 64 KB | Oldest steps are dropped |
| User dictionary | 5,000 words | `The user dictionary is full.` |
| Spelling suggestions | 8, within two edits, about 0.5 s | `More...` continues the search |
| Recent files | 4 | Oldest is dropped |
| Path length | 127 characters on Win16 and DOS, 259 on Win32 | Longer paths cannot be chosen in the dialogs |
| Memory | Win16 runs in 2 MB, with 4 MB the design target; Win32 needs what Windows 95 needs; DOS needs 4 MB | See the out-of-memory behaviour |

The document limits are the same in every build, even where a build has room for more, so any document one build writes, the others open. What 4 MB means in practice on Windows 3.1: a 100-page document with a dozen screen-sized pictures and spelling as you type is comfortable; a 1 MB document is possible but slow to lay out on a 386SX; a single photograph larger than the screen is shown reduced and prints in bands, and a 24-bit picture at 2,000 × 2,000 pixels is near the edge of what a 4 MB machine can print at all.
