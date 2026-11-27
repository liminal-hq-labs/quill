# Reveal Codes

A pane under the page that shows the text around the caret with its formatting as visible codes, the way WordPerfect's Reveal Codes did: `[Bold On]quote[Bold Off]`, `[Font: Arial]`, `[Align: Centred]`, `[Tab]`, `[HRt]`. The caret sits between codes, Left and Right step over them one at a time, and deleting a code removes that formatting in one undo step. Alt+F3 shows and hides it on every build.

This page is the design and the build order: one view model in the shared core, one pane per shell (DOS text, DOS graphics, Windows 3.x, Windows 95, GTK, WinUI and the terminal), and the work packages `WP-CODES-1` to `WP-CODES-10` with their briefs. It was written against `origin/main` at `992797a` (WP-ARCH-1 merged, the rest of [work-packages.md](../work-packages.md) ahead). Sizes marked **[E]** are estimates for the package to measure; everything else was read in the code. Nothing here changes behaviour until a package lands, so `SPEC.md` and `docs/decisions.md` change with the packages, not with this page.

## Table of contents

- [What the user sees](#what-the-user-sees)
- [Decisions](#decisions)
- [The code vocabulary](#the-code-vocabulary)
- [Paired codes, overlap and the order of a cluster](#paired-codes-overlap-and-the-order-of-a-cluster)
- [One view model](#one-view-model)
- [Caret and selection between the two views](#caret-and-selection-between-the-two-views)
- [Deleting a code, and undo](#deleting-a-code-and-undo)
- [Editing, inserting and dragging codes](#editing-inserting-and-dragging-codes)
- [Scrolling, sync and the split](#scrolling-sync-and-the-split)
- [The pane on each build](#the-pane-on-each-build)
- [Accessibility](#accessibility)
- [Budgets](#budgets)
- [Tests and evidence](#tests-and-evidence)
- [What it needs from the splits and the Unicode freeze](#what-it-needs-from-the-splits-and-the-unicode-freeze)
- [Work packages](#work-packages)
- [Briefs](#briefs)
- [Questions for the owner](#questions-for-the-owner)
- [Found while planning](#found-while-planning)

## What the user sees

**View > Reveal Codes** (Alt+F3; F11 too on Windows 3.x, Windows 95 and WinUI) splits the page area. The page keeps the top part and scrolls as before; below a separator, the codes pane shows the lines around the caret as a stream of characters and codes. The pane always keeps the caret's line in its middle row, so it needs no scroll bar of its own. The command is a tick in the View menu, and the pane starts hidden in every session, as WordPerfect's did.

The DOS text screen at 80 × 25 with the ruler on, the caret just before `quote`:

```text
 File  Edit  Insert  Find  Character  Paragraph  Document  View  Help
 L....|....1....|....2....|....3....|....4....|....5....|....6..R.....
            Dear Ms. Tremblay,

            Thank you for the quote. We would like to order the
            Desk Sans set in two weights, and the
            ...
══ Reveal Codes ════════════════════════════════════════════ Bold on ══
[HRt]
[Align: Justified]Thank you for the [Bold On]quote[Bold Off]. We would [SRt]
like to order the [Italic On]Desk Sans[Italic Off] set in two weights,[SRt]
and the[Tab][Font: Arial][Size: 11 pt]two[Size: 12 pt][Font: Times New...
...
 LETTER.QLD           Pg 1 Ln 4 Col 22   Times New Roman 12
```

The highlighted token (here `[Bold On]`, drawn in reverse) is the one after the caret, which is what Delete removes. The separator names it in full at its right end. Typing at that point gives plain text; one Right moves past `[Bold On]` and typing there gives bold text.

## Decisions

Proposed here; each package copies its rows into `docs/decisions.md` (Status **Assumed** unless the owner has answered) and the SPEC in the same change. The ids are reserved in [work-packages.md](../work-packages.md#decision-ids): **CORE-180 to CORE-199**, **ARCH-35**, **DOS-90 to DOS-99**, **W16-100 to W16-104**, **W9X-100 to W9X-104**, **LNX-70 to LNX-74**, **WUI-70 to WUI-74**.

| Id | Decision | Choice | Alternatives | Why |
|---|---|---|---|---|
| CORE-180 | Codes are a view, not data | Codes are derived from the runs (`CFmt` per piece) and the paragraph formats (`PFmt` on each mark) every time the pane is drawn; nothing is stored, and `.QLD`, RTF and text are unchanged | Store WordPerfect-style codes in the text | The model already holds the formats; a stored code would be a second truth to keep in step, and every build must keep writing the same bytes |
| CORE-181 | The vocabulary | Paired character codes for bold, italic, underline, strikethrough, superscript and subscript; single value codes for font, size and colour; paragraph codes for alignment, spacing, indents, space before and after and the tab set; per-paragraph codes for page break, style preset, list and restart; the special characters (tab, paragraph end, line break, hard space, soft hyphen, picture, the five fields) as codes that are also characters; `[SRt]` at a soft wrap, for show only | A WordPerfect 5.1 copy (`[BOLD]`/`[bold]`, `[HRt]` everywhere, `[Just:Center]`); document-level codes (margins, page size, headers) at the start as WordPerfect's initial codes | Everything a run or a paragraph holds has a code, so the pane is a faithful encoding (the decode property in the tests); document settings are not at a position, so they stay in their dialogs |
| CORE-182 | Where codes sit | A **cluster** of codes sits at a position when the format changes there; a cluster is ordered Off codes, then paragraph codes, then value codes, then On codes, each group in a fixed order; the mark of a non-empty paragraph takes no codes of its own | Codes in the order the user applied them (not recorded anywhere); codes around every mark | A fixed order makes the caret's place inside a cluster mean one format, so typing there is defined; marks carry formats nobody chose, and showing them would put noise round every `[HRt]` |
| CORE-183 | What a paragraph code means | Value codes (alignment, spacing, indents, space, tab set) show where a paragraph's value differs from the previous paragraph's, and stay in force until the next one, as WordPerfect's open codes did; page break, style, list and restart show on every paragraph that has them | Every paragraph shows all its values | Thirty justified paragraphs show `[Align: Justified]` once, and deleting it is one meaningful change |
| CORE-184 | The codes caret | The caret is a document position and a **slot** inside the cluster there (0 to its length). The slot is kept only while the caret and the document revision are unchanged; otherwise it is the default: the end of the cluster at a paragraph's start, 0 elsewhere, which is exactly CORE-40's typing format | A caret position in token space stored everywhere | Every caret move in the editor stays as it is; the default slot shows, in the pane, the format typing will take |
| CORE-185 | Moving and deleting with the pane shown | While the pane is shown, Left and Right step one token (a code or a character); Delete removes the token after the caret and Backspace the one before; every other key does what it does now. With the pane hidden nothing changes | A separate focus for the pane with its own keys (Windows and GTK still get focus by F6) | This is how WordPerfect 5.1 behaved with Reveal Codes on, and it gives the DOS screens, which have one keyboard focus, the whole feature |
| CORE-186 | Deleting a code | A code governs an **extent**: from it to the next change of its attribute. Deleting either code of a pair, or a value code, gives the extent the value it had before the code (so the runs merge and the pair, or the code and the one that ends its extent, disappear together); a per-paragraph code clears that property of its paragraph; a character code deletes its character as Delete does. One undo step, labelled `Delete Code` | WordPerfect's "Delete [Bold]? No (Yes)" prompt when the pane is hidden | Deleting a code removes the formatting it stands for and nothing else, and Undo gives the document back byte for byte |
| CORE-187 | Typing between codes | Text typed at slot `k` takes the format the first `k` codes of the cluster make; the codes then split round it, the first `k` before the new text and the rest after. A format chosen since the caret moved still wins (CORE-40) | Ignore the slot when typing | It is what the caret in the pane shows, so what you see is what you type |
| CORE-188 | The pane follows the caret | The pane has no scroll position: each paint puts the caret's pane row in the middle row (or as low as the document's start allows), so it never needs a scroll bar or an idle job. Pane rows follow document lines: a row ends at `[HRt]`, at a line break and at a soft wrap (`[SRt]`), and a line wider than the pane continues on the next row | A pane that scrolls on its own with a scroll bar | WordPerfect's pane followed the cursor; no state means nothing to keep in step between the two panes and nothing to save |
| CORE-189 | Showing, size and keys | `Command::RevealCodes` in View (`Reveal &Codes`, `Alt+F3`), ticked; the pane starts hidden each session; its height is `[Quill] CodesSize`, a percentage of the page area (15 to 70, default 30), changed by dragging the separator and saved with the other settings; Alt+F3 on every build, F11 also where it is free (Windows 3.x, Windows 95, WinUI; GTK keeps F11 for Full Screen, and DOS never receives it, see [Found while planning](#found-while-planning)). A shell that does not draw the pane yet leaves the item greyed (WP-ED-9's `command_state`) | Remember on or off; Ctrl+F3 (WordPerfect's screen key) | A mode you leave on by accident is a surprise at the next start; the size is a preference |
| CORE-190 | Edit a code | Enter on the code after the caret, or a double click on a code, selects the code's extent and opens the dialog that sets that property (Fonts for font, size, colour and the character codes; Indents and Spacing for the paragraph values; Tabs; Numbering for a list code); Esc in the dialog leaves the extent selected | Edit the value in place in the pane | The dialogs exist on every build and are answered by every shell; selecting the extent makes them edit exactly what the code covers |
| CORE-191 | Pending codes | When a format was chosen with no selection (CORE-40's typing format), the pane shows the codes it would make as a dimmed pair round the caret (`[Bold On]`, the caret, `[Bold Off]`), which go when the caret moves | Nothing | It answers "why did my typing come out bold" in the one place that shows formats |
| CORE-192 | Dragging a code | On the graphical builds, a code dragged along the pane moves the edge of its extent to where it is dropped: dragging `[Bold On]` left makes more text bold, right makes less; a pair's two codes cannot cross; one undo step, `Move Code` | Drag and drop of codes onto other text | It is the one drag that has a clear meaning when codes are derived |
| CORE-193 | Accessibility text | Each code has a short label (the pane) and a long name (`Bold on`, `Font: Arial`, `Left indent: 0.5 inch`, `Paragraph end`); the long name of the token after the caret shows at the right end of the separator, or in the status bar's message pane on the window builds, and the modern clients hand the pane to the screen reader through the `quill-a11y` snapshot shape | Labels only | A screen reader and a DOS user both get the code in words |
| ARCH-35 | Where the code lives | `quill_core::codes` holds the vocabulary, clusters, extents and removal plans, as pure reads of `Doc` (no strings, no `App`); `quill_app::codes` holds the state, keys, commands, the pane's flow model, the labels and the edits; the text pane is drawn by `quill_app::codes_paint`, the DOS graphics pane by `quill_paint::codes`, and each window build draws its own from the flow model | One new editor crate, `quill-codes`, above `quill-app` | The derivation is a function of the document and belongs with it; the edits need `App`'s undo groups, caret and selection, which an upper crate could reach only through more `#[doc(hidden)]` items than ARCH-17 allows |
| DOS-90 | The text pane | A separator row of `═` in the menu bar's colours with ` Reveal Codes ` at its left and the long name of the token after the caret at its right, then the pane rows, between the page and the status line; the pane is `CodesSize` percent of the text rows (at least 2), the page at least 3; codes in yellow on blue (bright in mono), characters as the page's regular attribute (formats are shown by the codes, not by colour), the token after the caret in reverse and the selection in the selection attribute | A pop-up box over the page; codes replacing the page (a full-screen toggle) | A real split keeps the page and the codes in view at once, and 80 × 25 has room for about seven pane rows with the page still at fourteen |
| DOS-91 | Alt+F3 on DOS | `quill_input::bios::decode` reads Alt+F1 to Alt+F10 (scans 0x68 to 0x71) as `Key::AltF(1..10)`, and Alt+F3 runs Reveal Codes on both DOS screens and in the terminal client | Switch INT 16h to function 10h so F11 arrives | Function 00h is what the DOS shell uses and every DOS scenario is recorded against; changing it is its own package |
| DOS-92 | The graphics pane | Below the page and its horizontal scroll bar: a 4-pixel splitter in button face with a highlight and a shadow line, then the pane on white in the Desk UI face, 16 pixels a row; codes as raised chips (a 1-pixel dark grey frame round a button-face fill, black label), characters in black, the caret a steady 2-pixel bar between tokens, the selection navy behind white; the vertical scroll bar spans the page only | Text-mode brackets in graphics | It is the Windows 3.1 look the graphics screen already draws, and chips are WordPerfect for Windows' own look |
| DOS-93 | The pointer in the pane | On both DOS screens: a click in the pane puts the caret before the token under it (its left half) or after it (its right half), Shift with a click extends the selection, a drag selects characters, the wheel scrolls the page, and a drag of the separator resizes the pane in whole rows; on the text screen the separator row is the handle | No pointer in the pane on the text screen | The text screen already works the menus and the page with the mouse (DOS-13 to DOS-28) |
| W16-100 | The Windows 3.x pane | The DOS graphics geometry and look in GDI: the pane and the splitter are parts of the main window's client area, drawn by the shell from the flow model in `ANSI_VAR_FONT`, with `IDC_SIZENS` over the splitter and `SetCapture` while it is dragged; the page's scroll bar controls are moved to span the page only; the pane's caret is drawn, steady, because the system caret is the page's | A child window with its own class | A child window costs a class, a window procedure and data; the page is already drawn this way |
| W16-101 | Labels on Windows 3.x | The label and long-name texts come from `RT_STRING` resources (`quill-app`'s feature `codes-text` is off in the Win16 build), loaded into a stack buffer when a token is drawn | Labels in the data segment | About 1.2 KB of text and pointers would come out of the data segment that must keep 1,024 bytes free (W16-60) |
| W16-102 | Where the code sits on Windows 3.x | `quill_core::codes` and `quill_app::codes` go in the EDIT segment, or in a segment of their own if EDIT would pass its budget in `segments.json` | Leave them to the packer's default | A segment overflow is a build error found late; naming the segment keeps `check_segments.py` meaningful |
| W9X-100 | The Windows 95 pane | As W16-100 in the system colours (`COLOR_WINDOW`, `COLOR_BTNFACE` chips, `COLOR_HIGHLIGHT`), `DEFAULT_GUI_FONT`, drawn into the off-screen buffer with the page; the long name in the status bar's first pane | A list view or a RichEdit control | The same model and look as the other retro builds, with no new control and no new import if it can be helped |
| W9X-101 | Keys on the Windows builds | Alt+F3 through `accel::from_vk` (`WM_SYSKEYDOWN` with `VK_F3`) and F11 through the shells' key maps, on Windows 95 and Windows 3.x alike; no new accelerator table | An `ACCELERATORS` resource | Both shells already take their shortcuts from `from_vk`, so one row serves both |
| LNX-70 | The GTK pane | A vertical `gtk::Paned` round the page area and a `gtk::DrawingArea` drawn with Cairo and Pango from the flow model, with codes as Adwaita pills that follow the dark style and the accent colour; the paned's position is `CodesSize`; F6 takes the pane into WP-GTK-4's focus cycle; Alt+F3 toggles (F11 stays Full Screen) | A `GtkTextView` with tags | The pane's editing rules are the core's, not a text view's; drawing it keeps one model on every build |
| LNX-71 | GTK accessibility | The drawing area implements `GtkAccessibleText` over the pane's text (labels as words) with the caret and selection in it, from the `quill-a11y` snapshot shape, and announces the long name of a deleted code | No accessible pane | Orca must be able to read the codes |
| LNX-72 | GTK keys | Alt+F3 is an application accelerator for `win.reveal-codes`; F11 stays `win.fullscreen`; Left, Right, Delete and Backspace reach the editor as now, which applies CORE-185 | Take F11 for Reveal Codes and move Full Screen | Full Screen on F11 is the GNOME convention |
| WUI-70 | The WinUI pane | Two rows of the page area's grid with a splitter, the pane drawn the way WUI-1's hosting draws the page, codes as Fluent chips; Alt+F3 and F11 | A `RichEditBox` | As LNX-70 |
| WUI-71 | WinUI accessibility | UI Automation's text pattern over the pane's snapshot in the `quill-a11y` shape, after WP-WINUI-9 | No accessible pane | As LNX-71, for Narrator |

## The code vocabulary

Labels are Title Case, as menu items are; a long name is sentence case. Units follow `[Quill] Units` (inches or centimetres); points for sizes and spacing. Where the long name is left empty, it is the label without brackets in sentence case (`Underline on`).

| Code | Pane label | Long name | From | Kind | Delete does |
|---|---|---|---|---|---|
| Bold | `[Bold On]`, `[Bold Off]` | Bold on, Bold off | `CFmt.flags` bit 0 | Pair | Clears bold over the extent |
| Italic | `[Italic On]`, `[Italic Off]` | Italic on, Italic off | bit 1 | Pair | Clears italic over the extent |
| Underline | `[Underline On]`, `[Underline Off]` | | bit 2 | Pair | Clears underline |
| Strikethrough | `[Strikethrough On]`, `[Strikethrough Off]` | Strikethrough on, off | bit 3 | Pair | Clears strikethrough |
| Superscript | `[Superscript On]`, `[Superscript Off]` | | `CFmt.vpos` 1 | Pair | Sets normal position |
| Subscript | `[Subscript On]`, `[Subscript Off]` | | `CFmt.vpos` 2 | Pair | Sets normal position |
| Font | `[Font: Arial]` | Font: Arial | `CFmt.font` | Value | The extent takes the font before the code |
| Size | `[Size: 14 pt]`, `[Size: 10.5 pt]` | Size: 14 points | `CFmt.size` (half points) | Value | The size before |
| Colour | `[Colour: Red]`, `[Colour: Automatic]`, `[Colour: #336699]` | | `CFmt.colour` (the 16 VGA names, else hex) | Value | The colour before |
| Alignment | `[Align: Left]`, `Centred`, `Right`, `Justified` | | `PFmt.align` | Paragraph value | The paragraphs of the extent take the previous paragraph's alignment |
| Spacing | `[Spacing: Single]`, `1½`, `Double` | | `PFmt.spacing` | Paragraph value | As alignment |
| Left indent | `[Left Indent: 0.5"]` | Left indent: 0.5 inch | `PFmt.left` | Paragraph value | As alignment |
| Right indent | `[Right Indent: 0.25"]` | | `PFmt.right` | Paragraph value | As alignment |
| First line | `[First Line: 0.5"]`, `[Hanging: 0.25"]` (negative) | | `PFmt.first` | Paragraph value | As alignment |
| Space before | `[Space Before: 6 pt]` | | `PFmt.before` | Paragraph value | As alignment |
| Space after | `[Space After: 12 pt]` | | `PFmt.after` | Paragraph value | As alignment |
| Tab set | `[Tab Set]` | Tab set: left at 1 inch, centre at 3.25 inches | `PFmt.tabset` | Paragraph value | As alignment (back to the previous set, or the default stops) |
| Page break | `[Page Break]` | Page break before | `PFmt.flags` bit 1 | Per paragraph | Clears the flag on that paragraph |
| Style | `[Style: Heading 1]` | | The paragraph matches a preset other than Normal (`App::style_of`'s rule) | Per paragraph | Applies Normal to that paragraph, as the style box does |
| List | `[Bullet]`, `[Bullet: Level 2]`, `[Number]`, `[Number: Level 3]` | Bulleted list, level 2 | `PFmt.flags` bits 0 and 2, `level`, `list` | Per paragraph | Takes the paragraph out of the list, as Backspace at its start does (`end_list`) |
| Restart | `[Restart Numbering]` | | `PFmt.flags` bit 3 | Per paragraph | Clears the flag |
| Tab | `[Tab]` | Tab | the character 0x09 | Character | Deletes the character |
| Paragraph end | `[HRt]` | Paragraph end | 0x0D | Character | Delete's rule at a paragraph's end (join, D98) |
| Line break | `[Line Break]` | | 0x0B | Character | Deletes it |
| Hard space | `[Hard Space]` | Non-breaking space | U+00A0 | Character | Deletes it |
| Soft hyphen | `[Soft Hyphen]` | | U+00AD | Character | Deletes it |
| Picture | `[Picture 320 × 200]` (`x` on the text screen) | Picture, 320 by 200 pixels | an object, kind 0 | Character | Deletes the picture, as Delete on a selected picture does |
| Fields | `[Page Number]`, `[Page Count]`, `[Date]`, `[File Name]`, `[Time]` | Page number field | an object, kinds 1 to 5 | Character | Deletes the field |
| Soft return | `[SRt]` | Soft return | a soft wrap in the layout | Display only, no caret stop | — |
| Pending | dimmed `[Bold On]`, `[Bold Off]` round the caret | Typing will be bold | CORE-40's typing format | Display only | — (moving the caret drops it) |

Not shown: the page setup, the default tab interval, Different First Page and the header and footer stories, which are not at a position (owner question 1). In the header and footer views the pane shows the story being edited, fields included.

## Paired codes, overlap and the order of a cluster

Quill keeps no codes, only a full format on every run, so a "pair" is the two ends of a maximal stretch of text that has an attribute. From that follow the rules a builder needs:

- **Pairs never nest with themselves.** Two touching bold stretches are one stretch (the piece table merges equal runs, and the derivation compares formats, not pieces), so `[Bold On][Bold On]` cannot appear and every Off has exactly one On before it.
- **Pairs of different attributes overlap freely.** Bold over "bold both" and italic over "both italic" is `[Bold On]bold [Italic On]both[Bold Off] italic[Italic Off]`, a crossing that WordPerfect also showed. Nothing tries to "balance" them, because there is no tree to balance.
- **An attribute open at the end** of the story has no Off: `[Bold On]` with nothing after it is in force to the end.
- **Value codes are not paired.** `[Font: Arial]` is in force until the next `[Font: ...]`, and the return to the previous font is a code of its own (`[Font: Times New Roman]`). Deleting the first makes both go.
- **Superscript and subscript are one property** with three values (normal, superscript, subscript), shown as two pairs: a change from one to the other is `[Superscript Off][Subscript On]`.
- **The base** is the document's format 0 (`Doc::init`'s Times New Roman 12, or the first format a reader interned) and `PFmt::DEFAULT`. The story's first cluster, at position 0, is the difference from the base, so a document typed all in Arial 11 starts with `[Font: Arial][Size: 11 pt]` once.
- **Marks.** A paragraph mark's own character format is ignored unless its paragraph is empty (CORE-40 uses it there, and it is the only character such a paragraph has): the state carries across `[HRt]` from the last text character, so a bold paragraph after a bold paragraph shows no Off and On round the mark. Extents include the marks inside them, so an edit keeps the marks' formats in step with their text.
- **A cluster's order** at position `p`:
  1. Off codes, in the order Subscript, Superscript, Strikethrough, Underline, Italic, Bold.
  2. If `p` starts a paragraph: Page Break, Style, Alignment, Spacing, Left Indent, Right Indent, First Line, Space Before, Space After, Tab Set, List, Restart.
  3. Value codes: Font, Size, Colour.
  4. On codes: Bold, Italic, Underline, Strikethrough, Superscript, Subscript.

Each attribute appears at most once in a cluster, so applying the first `k` codes to the state before the cluster gives a format whose difference from that state is exactly those `k` codes, and whose difference from the state after is exactly the rest. That is the property that makes typing at slot `k` (CORE-187) put the first `k` codes before the new text and the others after it, and it is tested as such. A cluster holds at most 6 + 12 + 3 + 5 = 26 codes, so `MAX_CLUSTER` is 32 and the slot is a `u8`.

## One view model

Two modules, no `alloc`, no floats, no panics (ARCH-35).

**`quill_core::codes`** (new file `crates/quill-core/src/codes.rs`) reads a `Doc` and one story and knows no strings:

- `Flag` (Bold, Italic, Underline, Strike, Super, Sub); `Code` (`On(Flag)`, `Off(Flag)`, `Font(u16)`, `Size(u16)`, `Colour(u8)`, `PageBreak`, `Style(u8)`, `Align(u8)`, `Spacing(u8)`, `LeftIndent(i32)`, `RightIndent(i32)`, `FirstLine(i32)`, `Before(i32)`, `After(i32)`, `TabSet(u16)`, `List { numbered: bool, list: u16, level: u8 }`, `Restart`); `CharKind` (`Text`, `Tab`, `ParaEnd`, `LineBreak`, `HardSpace`, `SoftHyphen`, `Picture(u16)`, `Field(u8)`).
- `Cluster::at(doc, story, s, pos, style: Option<u8>) -> Cluster` (a fixed array and a length), with `len`, `get(i)` and `state_after(k, before: CFmt) -> CFmt`. The style preset is the caller's to say (the presets live in `quill-app`).
- `for_each_token(doc, story, s, from, to, style: impl Fn(Pos) -> Option<u8>, f: impl FnMut(Token))`, `Token` being `Code { at, slot, code }` or `Char { at, kind }`, in story order: each position's cluster, then its character.
- `extent(doc, story, s, at, slot) -> Option<Extent>`: the range a code governs (characters for character codes, whole paragraphs for paragraph values, one paragraph for the per-paragraph codes).
- `removal(doc, story, s, at, slot, style) -> Option<Removal>`: the plan for deleting the token: `Chars { lo, hi, change }` (clear a flag, set a position, or set font, size or colour to a value), `Paras { first, end, change }`, `ParaFlag { para, bit }`, `EndList { para }`, `Normal { para }`, or `Char { at }`. A plan, not an edit: `App` carries it out, so undo groups, the caret and the modified flag stay where they are.

**`quill_app::codes`** (new file `crates/quill-app/src/codes.rs`) is the editor's side:

- `State { shown: bool, slot: u8, slot_at: Pos, slot_rev: u32, size: u8 }` in `App` (about 12 bytes).
- `Command::RevealCodes` and Alt+F3 (`Key::AltF(3)`), F11; `App::codes_shown()`; `App::set_codes_pane(bool)`, which a shell calls when it draws the pane (as `set_recovery`), so a shell that does not yet leaves the command greyed and does nothing.
- `App::codes_caret(s) -> (Pos, u8)`, the token stepping of Left and Right, Delete and Backspace on codes, typing at a slot, `delete_code`, and the range forms of `char_command` and `para_command` that both the old commands and `delete_code` use (one refactor, behaviour unchanged).
- The flow model: `App::codes_pane(s, spec: PaneSpec, measure: impl Fn(&Label) -> i32, f: impl FnMut(Placed))`, where `PaneSpec` is the pane's width, rows and row height in the shell's units (cells or pixels) and `Placed` is a token with its row, x, width, look (`Code`, `Char`, `Wrap`, `Pending`), and whether the caret is before it and it is selected. The text screen measures a label in cells, the graphics and window builds in pixels.
- `App::codes_hit(s, spec, measure, x, y) -> Option<(Pos, u8)>`; `App::codes_split(total, unit, min_page, min_pane) -> (page, bar, pane)` for every build's geometry.
- `codes_label(app, s, token, out: &mut [u8]) -> usize` and `codes_name(...)`, the only functions that reach text, behind the default-on feature `codes-text`; with it off they write a resource id instead (W16-101). Labels are one packed table of bytes, not `&str`s, so they cost the bytes they hold.

**`quill_app::codes_paint`** (new file) draws the text pane into a `Grid`; **`quill_paint::codes`** (new file) draws the graphics pane onto the DOS graphics surface. The window builds draw from `codes_pane` themselves.

## Caret and selection between the two views

- **One caret.** The page shows it as always. The pane shows it before the token at `(caret, slot)`: on the text screen the whole token after it is in reverse; on the graphical builds a steady 2-pixel bar stands between tokens (the page's caret is the one that blinks).
- **The slot** is the only new caret state (CORE-184). The default is 0, except at a paragraph's start where it is the cluster's length. A stored slot is used only while `caret == slot_at` and the story's revision equals `slot_rev`, so no movement code changes: anything that moves the caret, edits or undoes makes the default come back.
- **Moving by token** (pane shown): Right from `(p, k)` with `k < len` goes to `(p, k + 1)`; from `(p, len)` it passes the character at `p` to `(p + 1, 0)`. Left is the reverse. Up, Down, Home, End, the page keys and the word keys move the caret as now, with the default slot. Shift with Left or Right extends the selection by a character, as now.
- **Selection** stays a range of characters (`anchor`, `caret`). The pane highlights the characters in the range and the clusters strictly inside it (at `lo < p < hi`); the clusters at its ends are outside it. Deleting a selection deletes characters, and the codes that were inside go with the formats they described. A selection of codes alone does not exist.
- **The pointer in the pane** places the caret by hit test (DOS-93): the left half of a token puts the caret before it, the right half after it; Shift extends; a drag selects by characters.

## Deleting a code, and undo

1. `App` asks `quill_core::codes::removal` for the plan of the token after the caret (Delete) or before it (Backspace).
2. A character plan is today's `delete_forward` or `backspace`, so pictures, fields and the paragraph join keep their rules.
3. A format plan opens one undo group labelled `label::DELETE_CODE` ("Delete Code"), changes every run of the extent with the range form of `char_command` (or each paragraph with that of `para_command`, or `end_list`, or `apply_preset(Normal)`), and closes it. The piece table merges equal neighbours as it already does (`try_merge`), so the run count never rises.
4. The caret stays at `p`; the slot stays `k` (now the code that followed), stored with the new revision.
5. Undo is the piece table's: it restores the runs and the paragraph formats, and the pane shows the code again at its old place. Redo deletes it again.

Example: in `[Bold On]one [Italic On]two[Bold Off] three[Italic Off]`, Delete before `[Bold Off]` clears bold over "one two", leaving `one [Italic On]two three[Italic Off]`; Undo brings back the first form byte for byte.

## Editing, inserting and dragging codes

- **Editing** (CORE-190, WP-CODES-7): Enter with a code after the caret, or a double click on a code, selects the extent and runs the existing command for the dialog: `Command::Fonts`, the Indents and Spacing dialog, `Command::Tabs`, or the Numbering dialog once WP-ED-18 has built it. The picture code selects the picture (Size Picture once built). Fields and `[HRt]` do nothing.
- **Inserting** needs no new command: Ctrl+B with a selection makes a pair round it; with no selection it sets the typing format, which the pane shows as pending codes (CORE-191), and typing at a slot inside a cluster (CORE-187) puts new text exactly where the caret is between codes.
- **Dragging** (CORE-192, WP-CODES-8): on the graphical builds a press on a code and a drag along the pane shows an insertion bar; the drop moves the edge of the code's extent there (set the attribute on the gap, or clear it), one undo step, `Move Code`. A drop beyond the code's partner, or into a different story, does nothing. The DOS text screen has no drag of codes.

## Scrolling, sync and the split

- **The page** scrolls as it does now, in a shorter area: the text screen scrolls by `App::page_rows()` (the rows less the pane and its separator) instead of `rows`, and every pixel build passes the shorter page height to `set_view_height`.
- **The pane** follows the caret (CORE-188): `codes_pane` walks back from the caret's line until half the pane's rows are filled, then lays out forward. Scrolling the page without moving the caret leaves the pane where it is; a click in the page moves the caret and so the pane.
- **The split** (CORE-189): `codes_split` gives the page, the separator and the pane from the area between the bars and the status line, the pane being `CodesSize` percent rounded to whole rows, at least 2 rows (3 lines in pixels) and leaving the page at least 3 rows (5 lines). A window too small for both says `The window is too small to reveal codes.` and leaves the pane hidden. A drag of the separator sets `CodesSize` from the rows it leaves, and `each_setting` saves it when it is not 30.
- **Keyboard sizing** is not in this plan: the default size is usable, GTK's paned has its own keys (F8), and the setting is in `QUILL.INI` (and on the Options page once WP-PREF-5 lists it).

## The pane on each build

| Build | Split | Pane | Separator | Caret | Package |
|---|---|---|---|---|---|
| DOS text 80 × 25, 80 × 50, mono | Rows: 7 of 22 at 80 × 25 with the ruler, 14 of 47 at 80 × 50 | `[Code]` in yellow on blue (bright in mono), characters regular, `[SRt]` dim | `═` row in the menu colours, title and long name | Token after it in reverse | WP-CODES-2 (keys, paint), WP-CODES-4 (pointer) |
| Linux terminal (`quill-tty`) | As DOS text (it shares the painter) | As DOS text | As DOS text | As DOS text | WP-CODES-2 (its key map's Alt+F3 only) |
| DOS graphics 12h and VBE | About 6 rows of 16 pixels at 640 × 480 | Chips in the Desk UI face on white | 4-pixel bevelled splitter | 2-pixel bar | WP-CODES-4 |
| Windows 95 (`QUILL32.EXE`) | Pixels, system metrics | Chips in `COLOR_BTNFACE`, `DEFAULT_GUI_FONT` | 4 pixels, `IDC_SIZENS` | Drawn bar | WP-CODES-5 |
| Windows 3.x (`QUILL.EXE`) | As Windows 95 | Chips in light grey, `ANSI_VAR_FONT` | As Windows 95 | Drawn bar | WP-CODES-6 |
| GTK4 | `gtk::Paned`, vertical | Pango text, Adwaita pills, dark style | The paned's handle | Drawn bar | WP-CODES-9 |
| WinUI 3 | Grid rows and a splitter | Fluent chips | Splitter | Drawn bar | WP-CODES-10 |

Windows UI lands on Windows 95 first and Windows 3.x ports it (D34, D37); editing behaviour lands on the DOS text screen first, which is why WP-CODES-2 is that screen.

## Accessibility

- **Every build** names the token after the caret in words (CORE-193): at the right of the DOS separator, in the status bar's message pane on Windows (the first pane on Windows 95, the status line on Windows 3.x), and in the GTK and WinUI panes' accessible description.
- **GTK** (LNX-71): the pane's `GtkAccessibleText` serves its rows as UTF-8 lines with the labels written out, the caret offset and the selection, through the snapshot shape `quill-a11y` defines (WP-ARCH-8); a deleted code is announced (`Bold removed`). The pane is a read-only multi-line text with the name "Reveal Codes".
- **WinUI** (WUI-70): UI Automation's text pattern over the same snapshot, after WP-WINUI-9.
- **Keys alone** reach everything except resizing and dragging: show, step, delete, edit, undo.
- **Contrast**: the code attribute on the text screen and the chips keep the contrast of the existing chrome; the mono scheme uses bright and reverse only.

## Budgets

Measured at the branch point and before the pull request, as [work-packages.md](../work-packages.md#budgets) asks; the PR states both and the difference.

| Package | `QUILLDOS.EXE` | Win16 data | Win16 code | `QUILL32.EXE` | Notes |
|---|---|---|---|---|---|
| WP-CODES-1 | 0 (nothing calls it yet) | 0 | 0 | 0 | Host only, plus `quill codes` in `quill-cli` |
| WP-CODES-2 | +5 to +7 KB **[E]** (the core and the editor side, the labels, the text pane) | ≤ +16 (the state in `APP`; no label text is linked, because only painters reach it) | +4 to +6 KB **[E]** | +3 to +5 KB **[E]** (linked, unused) | Prove Win16's `.rodata` unchanged |
| WP-CODES-3 | +1.5 to +2.5 KB **[E]** | 0 | +1 to +2 KB | +1 to +2 KB | |
| WP-CODES-4 | +2.5 to +3.5 KB **[E]** | 0 | 0 | 0 | |
| WP-CODES-5 | 0 | 0 | 0 | +2 to +4 KB **[E]** | No new import expected; any is named and added to `imports.allow` |
| WP-CODES-6 | 0 | ≤ +40 (the pane font handle and the drag state in `SH`) | +2 to +4 KB **[E]** | 0 | Strings as `RT_STRING`; stack margin ≥ 2,048 with the pane drawn; `segments.json` places `quill_core::codes` and `quill_app::codes` |
| WP-CODES-7 | +1 to +1.5 KB **[E]** | ≤ +8 | +1 KB | +1 KB | |
| WP-CODES-8 | +1 to +2 KB **[E]** (graphics drag) | ≤ +8 | +1 to +2 KB | +1 to +2 KB | |
| WP-CODES-9, 10 | — | — | — | — | GTK and WinUI have no size budget |

**DOS.** The budget is 442,368 bytes with about 6 KB free today (owner, 2026-11-27), and the editor track's own ledger expects 467 to 484 KB once WP-ED-19 is in. Reveal Codes adds +11 to +16.5 KB **[E]** in all, so it is likely to reach DOS-70's stop at 480 KB (491,520 bytes). The savings each package takes first: one packed label table shared by both DOS screens; the measure text from the ruler's existing formatter; the graphics chips drawn with the dialog code's bevel; no long names beyond the separator's. Then, by DOS-70, a DOS row of the package's own raises the budget by the measured cost rounded up to 4 KB, with the DOS floppy's free bytes from `installer/tools/verify-disks.sh` (the floppy packs the executable to about 58 %, so +16 KB of code costs about 9.5 KB of disk). Owner question 5 covers the stop.

**Win16 data segment.** Nothing merges below 1,024 bytes free (W16-60). Reveal Codes is designed to spend almost none: the state is about 12 bytes in `APP`, the pane is drawn from the stack, and the texts are string resources (W16-101).

**Floppies.** Each package that grows an executable runs G-DISK and reports the free bytes of the disks it grows.

## Tests and evidence

**Host** (G-HOST):

- `quill codes FILE` (WP-CODES-1) prints a story's tokens one a line (`0 code On(Bold)`, `0 char Text 'q'`), and its output for a new fixture, `tests/docs/codes.rtf` (every code kind: overlapping bold and italic, a font and size change and back, colour, super and subscript, each alignment, indents, a hanging indent, space before and after, two tab sets, a page break, a heading preset, a bulleted and a numbered list with levels and a restart, a tab, a line break, a hard space, a soft hyphen, a picture, the five fields), is the golden `tests/golden/cli/codes.txt`.
- **Properties** at 10,000 cases over random documents (random text, random format commands over random ranges, random paragraph commands):
  - *decode*: applying the tokens in order from the base gives every text character its exact `CFmt`, and every paragraph its exact `PFmt`;
  - *pairs*: every Off has one On before it with no Off or On of that flag between; no cluster holds an attribute twice;
  - *prefix*: for every cluster and every `k`, the codes between `state_after(k)` and the state before are the first `k`, and between it and the state after are the rest;
  - *delete*: deleting a random code changes only that attribute and only inside its extent, never raises the piece count, and makes the code disappear (and its partner); Undo then gives byte-identical `qld::write` output, and Redo the deleted form again;
  - *typing*: typing a character at `(p, k)` gives a document whose cluster before the new character is the first `k` codes and whose cluster after it is the rest;
  - *stepping*: Right from the start visits every token once and ends at the end; Left from the end visits them in reverse; the count is the characters plus the codes;
  - *hit*: for every placed token, a hit at its left and right halves gives the caret before and after it.
- **Scenarios** with screen goldens (`tests/scenarios`, the next free prefix `m18-`): `m18-codes-show` (Alt+F3 and View > Reveal Codes, 25 and 50 rows and mono), `m18-codes-step` (Right through a cluster, typing at each slot), `m18-codes-delete` (Delete and Backspace on a pair, a value code, a paragraph value, a list code, Undo, Redo), `m18-codes-header` (the pane in the header view with fields), `m18-codes-small` (`@rows 25` with a size the screen cannot fit); graphics `g18-gfx-codes` (12h and VBE, with clicks and a separator drag), `g18-gfx-codes-drag` (WP-CODES-8).

**DOS** (G-DOS): every `m18-` and `g18-` scenario played in DOSBox gives the host's frames, cell for cell and CRC for CRC. If DOSBox's AUTOTYPE cannot send Alt+F3, the DOS copies open the pane through the View menu (`f10 v c`) and one host-only scenario keeps Alt+F3.

**Windows 3.x** (G-W16): a new `win16/tools/e2e-codes-w31.sh` plays the show, step, delete, undo and drag states on Windows for Workgroups 3.11 in DOSBox-X, and a new `check_codes.py` checks each frame (the splitter's band, the chips' count and order against the host flow model for that state, the caret's column), with CRC goldens of the pane band in `tests/golden/win16/` where two runs are bit-identical.

**Windows 95** (G-W32): a new `win9x/tools/wine-codes.sh` drives the same states under Wine, `wine-pixels.sh` gains the codes states (old and new builds equal elsewhere), and the `--calls` golden `tests/golden/win9x/paint-calls.txt` records the pane's GDI calls.

**GTK** (G-GTK): pixel goldens of the pane in `tests/golden/gtk/` (`codes.1.00.png`, `codes.1.50.dark.png`) from the offscreen renderer, and the AT-SPI script reading the pane.

**WinUI**: host tests of the shell core's pane model and framebuffer shots from the VM.

**Evidence** for every package goes to `~/Documents/Liminal HQ/Quill/Reveal Codes/<WP-ID>/`, one PNG per platform and state, named `<platform>-<state>.png` with the platforms of [work-packages.md](../work-packages.md#gates-and-evidence) (`dos-text-25`, `dos-text-50`, `dos-gfx-12h`, `dos-gfx-vbe`, `win16-311`, `win32-wine`, `win95`, `gtk-xvfb`, `winui`) and the states `shown`, `in-cluster`, `deleted`, `undone`, `resized`, `selection`, `edit`, `drag`, from the repository's own shot scripts only.

## What it needs from the splits and the Unicode freeze

Reveal Codes starts after the freeze (wave 8 of the order) has ended, because it touches the freeze set (`quill-app`, `quill-paint`, the shells' key maps and painters).

| From | What Reveal Codes uses | Kind |
|---|---|---|
| WP-ED-10, WP-ED-11 (Unicode U1b, U2) | Positions are `Pos`, characters come from `chars` (a hard space and a soft hyphen are U+00A0 and U+00AD, not bytes), font names in labels are UTF-8 and the text painter transcodes labels as it does the page | **Hard**: WP-CODES-1 branches after WP-ED-11 |
| WP-ARCH-4 `quill-input` | `Key::AltF(u8)` and Alt+F1 to F10 in `bios::decode`; Alt+F3 in `accel::from_vk` (and `vk::map` after WP-ED-7) | **Hard** for WP-CODES-2 |
| WP-ARCH-5 `quill-menu` | `Command::RevealCodes`, its View item, tip and hint, and its `command_id` | **Hard** for WP-CODES-2 |
| WP-ED-9 menu state | The tick on View > Reveal Codes, greying where the shell draws no pane, and the `Undo Delete Code` label | **Hard** for WP-CODES-2 (it merged in wave 6) |
| WP-ARCH-6 `quill-settings` and WP-PREF-2 settings by id | `CodesSize` as an entry of `assets/settings/settings.json` | **Hard** for WP-CODES-2 (WP-PREF-2 is wave 9) |
| WP-ARCH-1 `quill-chrome` (merged) | The pixel builds' geometry (`scroll_layout`) takes the shorter page height | Soft: nothing in `quill-chrome` changes |
| WP-ARCH-2 `quill-picture` | Nothing: the picture code reads `Doc::pictures`, which stays in the core | None, beyond having merged before the freeze |
| WP-ARCH-3 `quill-session` | Nothing: the pane has no idle work | None |
| WP-ARCH-7 `quill-flow` | Nothing until WP-CODES-7, which opens existing dialogs through their commands | Soft |
| WP-ARCH-8 `quill-a11y` | The snapshot shape the GTK and WinUI panes serve | **Hard** for WP-CODES-9 and WP-CODES-10 |
| WP-ED-12 default tab stops | `[Tab Set]`'s long name lists the default interval after the last stop | Soft |
| WP-ED-18 Numbering dialog | Enter on a list code opens it | Soft for WP-CODES-7 (without it, Enter on a list code does nothing) |
| WP-GTK-4, WP-GTK-5 | The focus cycle and colours the GTK pane joins, and its accessibility seam | **Hard** for WP-CODES-9 |
| WP-WINUI-6, WP-WINUI-9 | The WinUI page area and the text pattern | **Hard** for WP-CODES-10 |

## Work packages

Each package is one pull request for one Sonnet builder, under the rules of [work-packages.md](../work-packages.md#rules-for-every-package) (files owned, branch late, rebase, never merge main in). The packages that edit `crates/quill-app/src` (2, 3, 4, 7, 8) take the ARCH lane's slot there once WP-ARCH-8 has merged, one at a time; the shell-only packages (5, 6) take the side slot; the GTK and WinUI panes (9, 10) run in the UI track's lane.

| Wave | Package | Lane | Waits for | Runs beside |
|---|---|---|---|---|
| 9 | WP-CODES-1 the core: vocabulary, clusters, extents, `quill codes` | Side (touches only `quill-core` and `quill-cli`) | WP-ED-11 merged (the freeze ended) | WP-ARCH-8, WP-ED-12, WP-PREF-2 (WP-ED-12 owns `doc.rs` and `layout.rs`; WP-CODES-1 owns only its new file and one `mod` line) |
| 10 | WP-CODES-2 the pane on the DOS text screen | ARCH lane | WP-CODES-1, WP-ARCH-8, WP-PREF-2 | WP-PREF-5, WP-PREF-3 (WP-PREF-5 also moves DOS text goldens: whichever merges second regenerates its own) |
| 11 | WP-CODES-3 deleting a code | ARCH lane | WP-CODES-2 | WP-PREF-6 to WP-PREF-8, WP-ED-17 |
| 12 | WP-CODES-4 the DOS graphics pane and the pointer on both DOS screens | ARCH lane | WP-CODES-3 | WP-ED-18, WP-ED-19 (both draw in `quill-paint`; WP-CODES-4 owns only its new `codes.rs`, `layout.rs` and the pane arms of `pointer.rs`, and whichever merges second rebases `shell.rs`) |
| 13 | WP-CODES-5 the Windows 95 pane | Side | WP-CODES-3 (WP-CODES-4 is not needed) | WP-CODES-7 |
| 13 | WP-CODES-7 editing a code and pending codes | ARCH lane | WP-CODES-4, WP-ED-19 | WP-CODES-5 |
| 14 | WP-CODES-6 the Windows 3.x pane | Side | WP-CODES-5 (Win16 follows Win32), WP-ARCH-9 | WP-CODES-8 |
| 14 | WP-CODES-8 dragging codes | ARCH lane | WP-CODES-7 | WP-CODES-6 (it adds the drag to the Win16 pane after WP-CODES-6 merges, or leaves the Win16 half as a follow-up named in its PR) |
| UI | WP-CODES-9 the GTK pane | UI track (one GTK builder) | WP-CODES-3, WP-GTK-5, WP-ARCH-8 | WinUI packages |
| UI | WP-CODES-10 the WinUI pane | UI track (one WinUI builder) | WP-CODES-3, WP-WINUI-6, WP-WINUI-9 | GTK packages |

With the cap of three builders, waves 9 to 12 each use the slot the editor track left free, so the track's own order does not move. WP-CODES-5 and WP-CODES-9 can be pulled forward to any wave after WP-CODES-3 when a slot is free, since neither touches `crates/`.

### WP-CODES-1 — The core: vocabulary, clusters, extents

- **Finish line:** `quill_core::codes` derives every code of the vocabulary from a story, in the cluster order of CORE-182, with `Cluster::state_after`, `for_each_token`, `extent` and `removal` as plans; `quill codes FILE [--story header|footer|first-header|first-footer]` prints the tokens; the fixture `tests/docs/codes.rtf` and its golden; the six properties of [Tests and evidence](#tests-and-evidence) that need no `App` (decode, pairs, prefix, stepping over tokens, and delete and typing by applying the plan with `Text`'s own calls in the test).
- **Owns:** `crates/quill-core/src/codes.rs` (new), its `pub mod` line in `crates/quill-core/src/lib.rs`; `crates/quill-cli/src/main.rs` (the `codes` subcommand only); `tests/docs/codes.rtf` (new); `tests/golden/cli/codes.txt` (new); `crates/quill-core/tests/codes.rs` (new); SPEC's command-line table (the `quill codes` row); decision rows CORE-180 to CORE-183 and ARCH-35; the progress row.
- **Must not touch:** `crates/quill-core/src/{doc,text,layout}.rs` (WP-ED-12's); `quill-app`; every shell.
- **Depends on:** WP-ED-11 merged.
- **Budget:** 0 on every retro build (nothing links it yet; state the sizes anyway).
- **Tests:** G-HOST; G-MD. No DOS, Win16 or Win32 run is needed beyond the size figures (`dos/tools/size-report.sh`, the Win16 data line, the Win32 size).
- **Evidence:** `quill codes tests/docs/codes.rtf` output and `quill codes samples/` output for each sample, as text, in `Reveal Codes/WP-CODES-1/`.
- **Decisions:** CORE-180, CORE-181, CORE-182, CORE-183, ARCH-35.

### WP-CODES-2 — The pane on the DOS text screen

- **Finish line:** View > Reveal Codes and Alt+F3 show and hide the pane on the DOS text screens (25, 50, mono) and in `quill-tty`; the page scrolls in its shorter area; Left and Right step by token, typing at a slot takes the slot's format, the separator names the token after the caret; `CodesSize` is read and saved; the item is greyed on the graphics screen and every other shell until it draws the pane.
- **Owns:** `crates/quill-app/src/codes.rs` and `codes_paint.rs` (new) and their `mod` lines; in `lib.rs` the `codes` field of `App`, the Left, Right and typing arms that ask `codes` first, `page_rows` and the scroll reads it replaces, the `RevealCodes` and `AltF(3)` and `F(11)` arms; in `paint.rs` the row split of `draw_rows` and one call to `codes_paint::draw`; `Command::RevealCodes`, the View item, its tip and hint and id in `crates/quill-menu`; `Key::AltF` and `bios::decode`'s Alt+F rows and `accel`'s Alt+F3 in `crates/quill-input`; `codes_size` in `assets/settings/settings.json` and the generated files it regenerates; `linux/quill-tty/src/keymap.rs` (Alt+F3); `crates/quill-app/src/script.rs` (`altf3` token); the `m18-codes-show`, `m18-codes-step`, `m18-codes-header`, `m18-codes-small` scenarios and goldens; SPEC (View menu table, a Reveal Codes section under Behaviour, DOS text mode, the settings table, the build differences row); CORE-184, CORE-185, CORE-187, CORE-188, CORE-189, CORE-193, DOS-90, DOS-91.
- **Must not touch:** `quill-paint`; `mouse.rs` and `pointer.rs` (WP-CODES-4); any Windows, GTK or WinUI file beyond the regenerated resources; the delete arms (WP-CODES-3).
- **Depends on:** WP-CODES-1, WP-ARCH-8, WP-PREF-2.
- **Budget:** DOS +5 to +7 KB **[E]** (DOS-70 rule); Win16 data ≤ +16 with `.rodata` unchanged (prove it with the map); Win32 size reported.
- **Tests:** G-HOST (the stepping and typing properties through `App`, scenarios), G-DOS (the scenarios equal in DOSBox), G-W16 and G-W32 (the item greyed, nothing else changed: `m1-edit` and `wine-e2e.sh`), G-GTK (the item absent or greyed), G-DISK, G-MD.
- **Evidence:** `dos-text-25`, `dos-text-50` and mono in the states `shown`, `in-cluster`, `selection`, `resized` (through `CodesSize` in `QUILL.INI`).
- **Decisions:** as listed.

### WP-CODES-3 — Deleting a code

- **Finish line:** with the pane shown, Delete and Backspace on a code carry out its removal plan in one `Delete Code` undo step on every build that draws the pane (the DOS text screens now; every later pane gets it free); `char_command` and `para_command` take a range form both they and `delete_code` use; the delete properties pass through `App`.
- **Owns:** the Delete and Backspace arms' codes branch in `lib.rs`; `char_command`, `para_command` (made range-based; behaviour unchanged); `label::DELETE_CODE` and its Undo text in `crates/quill-menu`; `crates/quill-app/src/codes.rs` (the delete half); `m18-codes-delete` and its goldens; SPEC Undo and Reveal Codes; CORE-186.
- **Must not touch:** `codes_paint.rs` beyond what a deletion's message needs; every shell.
- **Depends on:** WP-CODES-2.
- **Budget:** DOS +1.5 to +2.5 KB **[E]**; Win16 data 0.
- **Tests:** G-HOST (delete property, the `m2` and `m14` goldens unchanged, proving the range refactor), G-DOS, G-W16 (`m1-edit`), G-W32, G-MD.
- **Evidence:** `dos-text-25` and `dos-text-50` in `deleted` and `undone` for a pair, a value code, a paragraph value and a list code.
- **Decisions:** CORE-186.

### WP-CODES-4 — The DOS graphics pane and the pointer on both DOS screens

- **Finish line:** the pane on the graphics screen in 12h and VBE (DOS-92); `Layout` gains the splitter and the pane; clicks, Shift+click, drags and the separator drag work in the pane on both DOS screens (DOS-93); the wheel over the pane scrolls the page; the graphics shell calls `set_codes_pane(true)`.
- **Owns:** `crates/quill-paint/src/codes.rs` (new), `layout.rs`, the pane arms of `pointer.rs`, the one call in `shell.rs`; the pane hit arms of `crates/quill-app/src/mouse.rs`; `dos/quill-dos/src/main.rs` (`set_codes_pane` for both screens and the resize pointer if the pointer module needs one); the `g18-gfx-codes` scenarios and goldens; SPEC DOS graphics mode; DOS-92, DOS-93.
- **Must not touch:** the page painter (`page.rs`); `quill-app`'s key arms.
- **Depends on:** WP-CODES-3.
- **Budget:** DOS +2.5 to +3.5 KB **[E]**.
- **Tests:** G-HOST (gfx goldens, hit property in pixels), G-DOS (`g18-` equal in DOSBox in both modes), G-DISK, G-MD.
- **Evidence:** `dos-gfx-12h` and `dos-gfx-vbe` in `shown`, `in-cluster`, `selection`, `resized`, `deleted`; `dos-text-25` with a click in the pane and a dragged separator.
- **Decisions:** DOS-92, DOS-93.

### WP-CODES-5 — The Windows 95 pane

- **Finish line:** `QUILL32.EXE` draws the pane (W9X-100) with the splitter and its cursor, clicks and drags as DOS-93, Alt+F3 and F11, the long name in the status bar's first pane, in the system colours and following a scheme change; `set_codes_pane(true)`.
- **Owns:** `win9x/quill-win9x/src/codes.rs` (new); in `ui.rs` and `view.rs` the page and pane geometry and the scroll bar controls' placement; the pane arms of `main.rs` (paint, `WM_SETCURSOR`, the mouse messages, the key arms); `win9x/tools/wine-codes.sh` (new), the codes states of `wine-pixels.sh`, `tests/golden/win9x/paint-calls.txt`; `imports.allow` if an import is new; SPEC build differences row; W9X-100, W9X-101.
- **Must not touch:** `crates/**`; `dialogs.rs`.
- **Depends on:** WP-CODES-3.
- **Budget:** `QUILL32.EXE` +2 to +4 KB **[E]**; imports named.
- **Tests:** G-W32 with `wine-codes.sh`, `wine-pixels.sh` (old against new equal outside the codes states), `wine-layout.sh`; G-DISK; G-MD.
- **Evidence:** `win32-wine` in every state; `win95` in `shown` and `deleted`.
- **Decisions:** W9X-100, W9X-101.

### WP-CODES-6 — The Windows 3.x pane

- **Finish line:** `QUILL.EXE` draws the pane as Windows 95 does in the Windows 3.1 look (W16-100), with its texts from `RT_STRING` (W16-101); `set_codes_pane(true)`.
- **Owns:** `win16/examples/quill/codes.rs` (new); the pane arms of `main.rs` and `chrome.rs`; the scroll bar controls' placement; `emit-resources`' codes strings and the regenerated `resources.json`; the `codes-text` feature line in `win16/Cargo.toml`; `segments.json` lines for `quill_core::codes` and `quill_app::codes`; `win16/tools/e2e-codes-w31.sh` and `check_codes.py` (new); `tests/golden/win16/` codes CRCs; W16-100 to W16-102.
- **Must not touch:** `crates/**` beyond `emit-resources`; the tips and hints code (WP-ARCH-9's).
- **Depends on:** WP-CODES-5, WP-ARCH-9.
- **Budget:** Win16 data ≤ +40 and 1,024 free after; stack margin ≥ 2,048 with the pane drawn; code within its segment budgets.
- **Tests:** G-W16 with `e2e-codes-w31.sh` and `check_resources.py`; G-DISK; G-MD.
- **Evidence:** `win16-311` in every state.
- **Decisions:** W16-100, W16-101, W16-102 (where the codes code sits in the segments).

### WP-CODES-7 — Editing a code and pending codes

- **Finish line:** Enter on a code and a double click on one select its extent and open its dialog (CORE-190) on every build with a pane; pending codes show round the caret (CORE-191) on every pane.
- **Owns:** the Enter arm's codes branch in `lib.rs`; the double click arm in `mouse.rs`; `crates/quill-app/src/codes.rs` (the edit and pending halves); the `Pending` look in `codes_paint.rs` and `quill-paint/src/codes.rs`, and in the Win32 and Win16 pane files (one draw arm each); `m18-codes-edit` and `g18-gfx-codes-edit`; CORE-190, CORE-191.
- **Must not touch:** the dialogs themselves.
- **Depends on:** WP-CODES-4 and WP-ED-19 (WP-ED-18's Numbering dialog if merged).
- **Budget:** DOS +1 to +1.5 KB **[E]**; Win16 data ≤ +8.
- **Tests:** G-HOST (Fonts on an extent changes only the extent; Esc leaves it selected), G-DOS, G-W16, G-W32, G-MD.
- **Evidence:** `dos-text-25`, `dos-gfx-vbe`, `win16-311`, `win32-wine` in `edit` and with pending codes.
- **Decisions:** CORE-190, CORE-191.

### WP-CODES-8 — Dragging codes

- **Finish line:** CORE-192 on DOS graphics, Windows 95 and Windows 3.x; the text screen unchanged.
- **Owns:** `crates/quill-app/src/codes.rs` (the move half); the drag arms of the pane in `quill-paint/src/pointer.rs`, `win9x/quill-win9x/src/codes.rs`, `win16/examples/quill/codes.rs`; `label::MOVE_CODE`; `g18-gfx-codes-drag`; CORE-192.
- **Depends on:** WP-CODES-7 (and WP-CODES-6 for the Win16 half).
- **Budget:** DOS +1 to +2 KB **[E]**; Win16 data ≤ +8.
- **Tests:** a property (dragging a code and Undo is the identity; a drag never makes a pair cross); G-HOST, G-DOS, G-W16, G-W32, G-MD.
- **Evidence:** `dos-gfx-vbe`, `win16-311`, `win32-wine` in `drag` (the insertion bar and the result).
- **Decisions:** CORE-192.

### WP-CODES-9 — The GTK pane

- **Finish line:** LNX-70 and LNX-71: the paned, the drawing area, the pills in light and dark, F6, Alt+F3, the accessible text and the announcement, the delete, edit and drag behaviour from the core.
- **Owns:** `linux/quill-gtk/src/codes.rs` (new); `window.rs` (the paned and wiring), `keys.rs` (Alt+F3), `a11y.rs` (the pane's snapshot); `tests/codes.rs` (new); `tests/golden/gtk/codes.*.png`; `docs/linux.md` rows; LNX-70, LNX-71, LNX-72.
- **Must not touch:** `crates/**`.
- **Depends on:** WP-CODES-3, WP-GTK-5, WP-ARCH-8 (WP-CODES-7 and 8 if merged; otherwise their GTK halves are follow-ups).
- **Tests:** G-GTK; the AT-SPI script reads the pane's line and the caret after Right.
- **Evidence:** `gtk-xvfb` in every state, light and dark, scales 1 and 2.

### WP-CODES-10 — The WinUI pane

- **Finish line:** WUI-70 and WUI-71 in the hosting WUI-1 chose.
- **Owns:** `winui/quill-winui/**` (the pane module and its wiring); `docs/winui.md` rows; WUI-70, WUI-71.
- **Depends on:** WP-CODES-3, WP-WINUI-6, WP-WINUI-9.
- **Tests:** the shell core's pane tests on the host; the UI Automation dump in the VM.
- **Evidence:** `winui` in every state, light and dark.

## Briefs

Every brief starts with [the common brief](../work-packages.md#the-common-brief) of work-packages.md, with two changes for this track: decision ids come from this page's reserved ranges, and evidence goes to `~/Documents/Liminal HQ/Quill/Reveal Codes/<WP-ID>/`. Paste the common brief, then the package's own.

### WP-CODES-1 (full)

```text
Package WP-CODES-1: the core of Reveal Codes — the code vocabulary, clusters, extents and removal plans in a new
module quill_core::codes, and a `quill codes FILE` command that prints them. Read docs/plans/reveal-codes.md in
full first: the vocabulary table, "Paired codes, overlap and the order of a cluster" and "One view model" are
your specification. No UI, no App, no shell.

Branch feat/codes-core, worktree ~/source/worktrees/quill/codes-core, from origin/main after WP-ED-11 (UTF-8 in
the buffers) has merged. WP-ED-12 may be open at the same time: it owns quill-core's doc.rs and layout.rs, so you
touch neither; your files are new.

1. Read crates/quill-core/src/doc.rs (CFmt, PFmt, the cflag bits, TabSet, ListDef, Picture, Doc::init — format 0
   is the base), text.rs (Piece: cfmt on every piece, pfmt on the paragraph marks; for_each_run, cfmt_at,
   pfmt_at, object_at — an object's kind is in the high half: 0 picture, 1 page number, 2 page count, 3 date,
   4 file name, 5 time) and the Pos and chars API WP-ED-4 and WP-ED-10 added. Read crates/quill-app/src/chrome.rs
   style_of and PRESETS (a style is the caller's to say: you take it as a parameter) and lib.rs end_list (what
   removing a list code will do).
2. Create crates/quill-core/src/codes.rs (SPDX header, no alloc, no floats, no panics, no indexing: the clippy
   deny block the crate already uses applies) and `pub mod codes;` in lib.rs. Types, as the plan names them:
   Flag, Code, CharKind, Token, Cluster (a [Code; 32] and a u8 length), Extent, Removal, and the change enums
   Removal carries. Functions:
     Cluster::at(doc, story, s, pos, style: Option<u8>) -> Cluster
     Cluster::len, Cluster::get(i), Cluster::state_after(k, before: CFmt) -> CFmt
     char_kind(story, s, pos) -> Option<CharKind>
     for_each_token(doc, story, s, from, to, style: impl Fn(Pos) -> Option<u8>, f: impl FnMut(Token))
     extent(doc, story, s, at, slot, style) -> Option<Extent>
     removal(doc, story, s, at, slot, style) -> Option<Removal>
   Rules (CORE-181 to CORE-183): the base is format 0 and PFmt::DEFAULT; a cluster at p compares the state
   before p (the last character before p that is not the mark of a non-empty paragraph) with the format at p
   (no character codes at the mark of a non-empty paragraph); paragraph values compare with the previous
   paragraph's PFmt (the first with PFmt::DEFAULT); page break, style, list and restart appear on every
   paragraph that has them; the order is Off codes (Sub, Super, Strike, Underline, Italic, Bold), paragraph
   codes (PageBreak, Style, Align, Spacing, LeftIndent, RightIndent, FirstLine, Before, After, TabSet, List,
   Restart), value codes (Font, Size, Colour), On codes (Bold, Italic, Underline, Strike, Super, Sub).
   Superscript and subscript are vpos 1 and 2: a change between them is an Off then an On. Removal is a plan:
   do not edit the document in this module.
3. crates/quill-cli: a `codes` subcommand: `quill codes FILE [--story body|header|footer|first-header|
   first-footer]` prints one token a line: position, `code` or `char`, and the Debug form of the code or the
   kind with the character quoted (`0 code On(Bold)`, `0 char Text 'q'`, `12 char ParaEnd`). Add its row to
   SPEC's command-line table ("Print a story's formatting as Reveal Codes tokens").
4. Fixture tests/docs/codes.rtf, written by hand in the RTF subset the SPEC lists, with every row of the
   plan's vocabulary table that a document can hold (see "Tests and evidence"); check it opens with
   `quill info`. Golden tests/golden/cli/codes.txt is `quill codes tests/docs/codes.rtf`, compared in a test
   and rewritten by QUILL_UPDATE_GOLDEN=1. Read the golden line by line against the RTF and fix the code, not
   the golden, wherever they disagree.
5. crates/quill-core/tests/codes.rs, proptest at 10,000 cases (follow the crate's existing property tests for
   how documents are built): decode (applying the tokens from the base gives every text character its CFmt
   and every paragraph its PFmt), pairs (every Off has one On before it; no attribute twice in a cluster),
   prefix (state_after(k) differs from the state before by exactly the first k codes and from the state
   after by the rest), stepping (walking tokens forwards and backwards visits each once), delete (carry out a
   random removal plan with Text's own set_cfmt and set_pfmt inside begin_group and end_group in the test:
   only that attribute changes, only inside the extent, the piece count does not rise, the code and its
   partner are gone; undo gives byte-identical qld::write output), typing (insert a character at (p, k) with
   state_after(k)'s format, interned: the cluster before it is the first k codes and after it the rest).
   Units for the examples in the plan (the overlapping pair, the font and back, super to sub, a bold
   paragraph across a mark, a document all in Arial 11, an empty paragraph's mark).
6. Docs: decisions CORE-180 to CORE-183 and ARCH-35 from the plan's table into docs/decisions.md (Status
   Assumed); a WP-CODES-1 row in docs/progress.md. SPEC changes only by the command-line row (the pane is not
   behaviour yet).
7. Gates: G-HOST, G-MD. Report the sizes too, unchanged: `bash dos/tools/size-report.sh` (QUILLDOS.EXE),
   the Win16 data figure from `bash win16/build.sh`, and QUILL32.EXE's size from `bash win9x/build.sh`.
8. Evidence in ~/Documents/Liminal HQ/Quill/Reveal Codes/WP-CODES-1/: codes.txt (the fixture's output) and
   one <sample>.txt for each file in samples/ and tests/docs.
9. PR title: "Derive Reveal Codes from the document's runs and paragraphs". Labels: enhancement.

Finish line: `quill codes` prints the plan's vocabulary for the fixture exactly as the golden says, every
property passes at 10,000 cases, nothing outside quill-core, quill-cli, the fixture, the golden and the docs
changed, and every retro executable is the size it was.

Defaults if nobody answers (record each as an Assumed decision with "OWNER DECISION:"): the vocabulary is the
plan's table (owner question 1); value codes are single codes, not pairs; paragraph values are changes from the
previous paragraph. Keep the code kinds in one table so a filter can be added later without moving code.
```

### WP-CODES-2

```text
Package WP-CODES-2: Reveal Codes on the DOS text screens and the terminal client — the pane, the toggle, moving
by token and typing between codes; read-only otherwise (deleting a code is WP-CODES-3). Branch feat/codes-text
after WP-CODES-1, WP-ARCH-8 and WP-PREF-2 merge. Read docs/plans/reveal-codes.md: "One view model", "Caret and
selection", "Scrolling, sync and the split", the DOS text row of "The pane on each build", and decisions CORE-184,
CORE-185, CORE-187, CORE-188, CORE-189, CORE-193, DOS-90, DOS-91.
New crates/quill-app/src/codes.rs (State, the slot rule, codes_caret, stepping, typing at a slot, codes_pane,
codes_hit, codes_split, page_rows, set_codes_pane, codes_label and codes_name behind the default-on feature
codes-text, one packed label table) and codes_paint.rs (the separator and the pane into the Grid). In lib.rs
only: the codes field, the Left, Right and typing arms asking codes first when the pane is shown, the scroll reads
of rows that become page_rows(), and the RevealCodes, AltF(3) and F(11) arms. In paint.rs only the row split in
draw_rows and the call to codes_paint. quill-menu: Command::RevealCodes, "Reveal &Codes" with "Alt+F3" last in
View, its tip and hint, its command_id, and its command_state (ticked when shown, greyed unless the shell called
set_codes_pane(true)); regenerate resources.json and quill.json. quill-input: Key::AltF(u8), bios::decode for
scans 0x68..=0x71, Alt+VK_F3 in accel. settings.json: codes_size (15..70, default 30). The DOS text screens call
set_codes_pane(true) (in quill-app's text path or the DOS shell's start, whichever is one line), and
linux/quill-tty maps Alt+F3. No label text may be reachable from App::key or App::command: prove it with the
Win16 link map (.rodata the same size). Scenarios m18-codes-show, -step, -header, -small with goldens at 25, 50
and mono (if DOSBox AUTOTYPE cannot send Alt+F3, the DOS copies use f10 v c and one host-only scenario keeps
altf3). Gates: G-HOST, G-DOS (DOS-70 rule), G-W16 (data Δ ≤ +16), G-W32, G-GTK (item greyed), G-DISK, G-MD.
SPEC: View menu table, a "Reveal Codes" section, DOS text mode, settings table, build differences. Evidence in
Reveal Codes/WP-CODES-2 (dos-text-25, dos-text-50, mono: shown, in-cluster, selection, resized). PR: "Reveal codes
under the page on the DOS text screens".
```

### WP-CODES-3

```text
Package WP-CODES-3: deleting a code — Delete and Backspace on a code carry out quill_core::codes::removal in one
"Delete Code" undo step. Branch feat/codes-delete after WP-CODES-2. Read the plan's "Deleting a code, and undo"
and CORE-186. First, in a commit of its own, give char_command and para_command range forms (char_range(lo, hi,
f, flag, label), para_range(first, end, f, label)) that the commands call, with every golden unchanged; then
delete_code on top. A character plan goes to delete_forward or backspace unchanged; list removal to end_list;
style removal to apply_preset(Normal) inside your group. Add label::DELETE_CODE and its Undo text in quill-menu.
The caret stays, the slot stays (re-stored with the new revision). Scenario m18-codes-delete (a pair, a value
code, a paragraph value, a list code, a tab, Undo, Redo) with goldens; the delete property through App at
10,000 cases. Gates: G-HOST (m2 and m14 goldens untouched by the refactor commit), G-DOS, G-W16 (data 0), G-W32,
G-MD. Decision CORE-186; SPEC Undo and Reveal Codes. Evidence in Reveal Codes/WP-CODES-3. PR: "Delete a code in
Reveal Codes to remove its formatting".
```

### WP-CODES-4

```text
Package WP-CODES-4: the Reveal Codes pane on the DOS graphics screen, and the pointer in the pane on both DOS
screens. Branch feat/codes-gfx after WP-CODES-3. Read the plan's DOS-92 and DOS-93. New
crates/quill-paint/src/codes.rs draws codes_pane's Placed tokens: chips with the dialog code's bevel, the Desk UI
face, a steady 2-pixel caret, the selection colours. Layout gains `codes: Option<(Rect, Rect)>` (splitter, pane)
from App::codes_split; the vertical bar spans the page only; set_view_height gets the page's height. Pointer:
click, Shift+click, drag-select in the pane through codes_hit; the separator drags in whole rows and sets
CodesSize; the wheel over the pane scrolls the page; the same on the text screen (the separator row is the
handle) in quill-app's mouse.rs pane arms only. The graphics shell calls set_codes_pane(true). If WP-ED-19 is
open in quill-paint, rebase over it in shell.rs. Scenarios g18-gfx-codes (12h and VBE) with goldens, and a text
screen pointer scenario. Gates: G-HOST, G-DOS (both modes equal), G-DISK, G-MD; DOS-70 rule. Decisions DOS-92,
DOS-93; SPEC DOS graphics mode. Evidence in Reveal Codes/WP-CODES-4. PR: "Reveal codes on the DOS graphics
screen, with the mouse on both".
```

### WP-CODES-5

```text
Package WP-CODES-5 (Windows 95 only): the Reveal Codes pane in QUILL32.EXE. Branch feat/codes-win32 after
WP-CODES-3. Read the plan's W9X-100 and the pane table. New win9x/quill-win9x/src/codes.rs draws from
App::codes_pane into the off-screen buffer with DEFAULT_GUI_FONT (measure with GetTextExtentPoint32A), chips in
COLOR_BTNFACE with COLOR_BTNSHADOW frames, COLOR_HIGHLIGHT selection, a drawn steady caret. ui.rs and view.rs:
the page, splitter and pane from App::codes_split; the SB_CTL bars span the page; set_view_height. main.rs:
WM_SETCURSOR (IDC_SIZENS over the splitter), SetCapture drag, clicks through codes_hit, Alt+F3 and F11, the long
name in the status bar's first pane, set_codes_pane(true), a scheme change repaints. Prefer imports already in
win9x/tools/imports.allow; name any new one. New win9x/tools/wine-codes.sh; add the codes states to
wine-pixels.sh and update tests/golden/win9x/paint-calls.txt in the same commit. Do not touch crates/. Gates:
G-W32 with wine-codes.sh, wine-pixels.sh, wine-layout.sh; G-DISK; G-MD. Decisions W9X-100, W9X-101. Evidence in
Reveal Codes/WP-CODES-5 (win32-wine every state, win95 shown and deleted). PR: "Reveal codes in Quill for
Windows 95".
```

### WP-CODES-6

```text
Package WP-CODES-6 (Windows 3.x only): the Reveal Codes pane in QUILL.EXE, ported from WP-CODES-5's design.
Branch feat/codes-win16 after WP-CODES-5 and WP-ARCH-9. Read W16-100 to W16-102 and docs/win16-data-segment.md.
Turn quill-app's codes-text feature off in win16/Cargo.toml; emit-resources writes the label and long-name
strings as RT_STRING (as WP-ARCH-9 did for hints); the pane LoadStrings each label into a stack buffer.
ANSI_VAR_FONT, light grey chips, the Windows 3.1 bevel, a drawn caret, IDC_SIZENS, SetCapture. Place
quill_core::codes and quill_app::codes in segments.json where check_segments.py passes. New
win16/tools/e2e-codes-w31.sh and check_codes.py; CRC goldens of the pane band in tests/golden/win16/ only if two
runs are bit-identical (say which). Gates: G-W16 (data Δ ≤ +40 with 1,024 free after; stack margin ≥ 2,048
with the pane drawn; segments; check_resources.py), G-DISK, G-MD. Evidence in Reveal Codes/WP-CODES-6
(win16-311 every state). PR: "Reveal codes in Quill for Windows 3.x".
```

### WP-CODES-7

```text
Package WP-CODES-7: editing a code and pending codes. Branch feat/codes-edit after WP-CODES-4 and WP-ED-19. Read
CORE-190 and CORE-191. Enter with a code after the caret, or a double click on a code, selects
quill_core::codes::extent and runs Fonts, Indents and Spacing, Tabs or Numbering (if WP-ED-18 has merged;
otherwise a list code's Enter does nothing, said in the PR). Pending codes: codes_pane emits the difference
between CORE-40's typing format and the slot's format as Pending tokens round the caret; draw them dimmed in
codes_paint.rs, quill-paint's codes.rs, and the Win32 and Win16 pane files (one arm each). Scenarios
m18-codes-edit and g18-gfx-codes-edit. Gates: G-HOST, G-DOS, G-W16 (data ≤ +8), G-W32, G-MD. Evidence in
Reveal Codes/WP-CODES-7. PR: "Edit a code's formatting from Reveal Codes".
```

### WP-CODES-8

```text
Package WP-CODES-8: dragging a code on the graphical builds (CORE-192). Branch feat/codes-drag after
WP-CODES-7 (and WP-CODES-6 for the Win16 half; if it has not merged, leave that half and name it in the PR).
quill_app::codes gains move_code(at, slot, to): set or clear the attribute over the gap, one "Move Code" undo
step (label::MOVE_CODE), refused past the partner or the story's ends. Pane drags in quill-paint's pointer.rs,
win9x codes.rs and win16 codes.rs show an insertion bar and drop. Property: drag then Undo is the identity; a
pair never crosses. Scenario g18-gfx-codes-drag. Gates: G-HOST, G-DOS, G-W16, G-W32, G-MD. Evidence in Reveal
Codes/WP-CODES-8. PR: "Drag a code in Reveal Codes to move where its formatting starts or ends".
```

### WP-CODES-9

```text
Package WP-CODES-9 (GTK only, UI track rules of docs/ui-track.md): the Reveal Codes pane in the GTK client.
Branch feat/gtk-codes after WP-CODES-3, WP-GTK-5 and WP-ARCH-8. Read LNX-70 to LNX-72 in the plan. A vertical
gtk::Paned round the page area; a gtk::DrawingArea drawn with Cairo and Pango from App::codes_pane (measure with
Pango), Adwaita pills following the dark style and the accent, a drawn caret; the paned's position is CodesSize;
F6 joins WP-GTK-4's cycle; Alt+F3 toggles and F11 stays Full Screen; GtkAccessibleText over the pane's snapshot
in the quill-a11y shape, with an announcement on deletion; set_codes_pane(true). Do not touch crates/; a shared
change you need stops the package. Pixel goldens tests/golden/gtk/codes.1.00.png and codes.1.50.dark.png; the
AT-SPI script reads the pane's line and caret. Gates: G-GTK, headers, G-MD. Evidence in Reveal
Codes/WP-CODES-9 (light and dark, scales 1 and 2, every state). PR: "Reveal codes in the GTK client".
```

### WP-CODES-10

```text
Package WP-CODES-10 (WinUI only, UI track rules): the Reveal Codes pane in the WinUI client. Branch
feat/winui-codes after WP-CODES-3, WP-WINUI-6 and WP-WINUI-9. Read WUI-70 and WUI-71. Two rows of the page
area's grid with a splitter; the pane drawn the way WUI-1's hosting draws the page; Fluent chips; UI Automation's
text pattern over the pane's snapshot; Alt+F3 and F11; set_codes_pane(true). Host tests of the pane model in the
shell core; the UI Automation dump and framebuffer shots from the VM (cable off, snapshot restored at the end).
Evidence in Reveal Codes/WP-CODES-10. PR: "Reveal codes in the WinUI client".
```

## Questions for the owner

Each has a default the builders take if nobody answers, recorded as an Assumed decision with "OWNER DECISION:".

| # | Question | Default taken | Where to swap |
|---|---|---|---|
| 1 | Which codes to show: the full vocabulary above, or fewer (no `[SRt]`, no paragraph values, no `[Style]`), or more (document settings at the start as WordPerfect's initial codes)? | The full vocabulary, no document-level codes, paragraph values as changes from the previous paragraph, `[HRt]` and `[SRt]` as the two WordPerfect names and plain words for the rest | The code-kind table in `quill_core::codes` (a filter there) and the label table in `quill_app::codes`; CORE-181 and CORE-183 |
| 2 | Editable first, or read-only first? | Editable first: deleting a code (WP-CODES-3) comes straight after the DOS text pane, before the other shells | Reorder the wave table: move WP-CODES-3 after WP-CODES-6; no code changes, because deleting is one branch of the Delete and Backspace arms |
| 3 | The DOS pane: a real split pane on both DOS screens, or a pop-up strip, or codes replacing the page while shown? | A real split on both screens, 30 % of the page area, the text screen's separator row naming the code at the caret | `App::codes_split` (one function for every build) and DOS-90, DOS-92 |
| 4 | While the pane is shown, do Left, Right, Delete and Backspace work on codes (WordPerfect 5.1), or does the pane need focus first (F6 or a click)? | They work on codes while the pane is shown, on every build; F6 also moves the focus on GTK and WinUI | The `shown` test in `codes.rs`'s key arms becomes `shown && focused`; CORE-185 |
| 5 | DOS size: Reveal Codes adds +11 to +16.5 KB and will likely pass DOS-70's stop at 480 KB. Go past it? | Yes, by the measured cost rounded to 4 KB, as long as the DOS floppy keeps 128 KB free; each package records its row | DOS-70 and the package's DOS row; or build `QUILLDOS.EXE` without the `codes` feature on the graphics screen |
| 6 | Remember whether the pane was shown, as the View bars are remembered? | No: it starts hidden each session; only its size (`CodesSize`) is saved | `each_setting` and `apply_setting` (one key); CORE-189 |

## Found while planning

- **F11 never reaches the DOS build.** `quill_app::bios::decode` maps scans 0x85 and 0x86 to F11 and F12, but `dos/quill-dos/src/keys.rs` reads keys with INT 16h function 00h, which drops the keys an enhanced keyboard added (F11, F12 and the grey cursor keys' own codes); only function 10h returns them. That is why DOS-91 binds Alt+F3 alone. The dead rows can go, or a later package can move the DOS keyboard to function 10h with every DOS scenario re-recorded.
- **F11 is Full Screen on GTK** (`win.fullscreen`, `linux/quill-gtk/src/window.rs`), so the GTK client cannot take WordPerfect's other key; Alt+F3 works there.
- **The SPEC's menu table is behind the menu tree.** It lists Ruler, Toolbar, Status Bar and Zoom under Document, while `paint.rs` has a View menu (Toolbar, Formatting Bar, Ruler, Status Bar, Zoom) and Document holds the header and footer items; it gives Exit as Alt+F4 where the tree says Ctrl+Q. WP-CODES-2 adds Reveal Codes to View and should bring the table's View rows in line as it does.
- **The scenario prefix `m17-` is taken** by the recovery scenarios, so Reveal Codes uses `m18-` and `g18-`.
