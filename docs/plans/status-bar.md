# Status bar: one layout for every build

The owner asked on 2026-11-27 for a tidier status bar across the board: review what every build shows, then normalize it. This page is the review, one layout with its rules, the mock-ups, the decisions and the work packages, `WP-BAR-1` to `WP-BAR-9`. Written against `origin/main` at `1f4f28a`. The rules, budgets, gates (G-HOST, G-DOS, G-W16, G-W32, G-GTK, G-DISK, G-MD) and the common brief are those of [work-packages.md](../work-packages.md).

The zoom reset button and the zoom tooltips the owner asked for (`Zoom Out`, `Zoom (click for the list)`, `Zoom In`, and `Page Width (Ctrl+0)`) are this plan's first package, `WP-BAR-1`, so they land in the layout the rest of the plan builds.

The owner answered three questions on 2026-11-27 and the page follows them: the Page Width button sits **right of the plus** (question 1); where the SPEC and the code disagree about the status line, **the SPEC wins**, so the DOS text screens gain superscript, subscript and the text colour's name (DOS-104, in WP-BAR-3); and the DOS key for Page Width was left to this plan: **Alt+0** (DOS-105), since the BIOS reports no Ctrl with a digit.

## Table of contents

- [What every build shows today](#what-every-build-shows-today)
- [What is wrong with it](#what-is-wrong-with-it)
- [The normalized layout](#the-normalized-layout)
- [The fields](#the-fields)
- [The lead](#the-lead)
- [Narrowing](#narrowing)
- [Widths and spacing](#widths-and-spacing)
- [The look on each build](#the-look-on-each-build)
- [Hints and tooltips](#hints-and-tooltips)
- [Reveal Codes](#reveal-codes)
- [View > Status Bar and the Options window](#view--status-bar-and-the-options-window)
- [Accessibility](#accessibility)
- [What is shared and what each shell draws](#what-is-shared-and-what-each-shell-draws)
- [Budgets](#budgets)
- [Mock-ups](#mock-ups)
- [Decisions](#decisions)
- [Work packages](#work-packages)
- [Briefs](#briefs)
- [Questions for the owner](#questions-for-the-owner)
- [Found while planning](#found-while-planning)

## What every build shows today

Read from the code on `main` and checked against the screenshots in `~/Documents/Liminal HQ/Quill/` (the ReactOS zoom review, the remix shots, WP-ARCH-1's before and after shots). Crops of the real bars are in `Status bar/evidence/`.

| Build | Drawn by | Fields, left to right | Look | Zoom control | When narrow | Hints |
| --- | --- | --- | --- | --- | --- | --- |
| Windows 95, 98, Me and ReactOS (`QUILL32.EXE`) | `win9x/quill-win9x/src/ui.rs` `status_layout`, `status_update`, `draw_zoom_control` | Six `msctls_statusbar32` panes: the hint or message (the rest, at least 60 px), the document as opened (`C:\Quill\DEMO.QLD`, 105 px), `Pg 1 Ln 1 Col 1` (105 px), `Times New Roman 16` (130 px), `OVR Modified` (70 px), the zoom pane | Native sunken panes, size grip | One owner-drawn pane: minus, a 72 px box, plus, a 16 px margin for the grip, etched lines between (W9X-14, W9X-15) | Under 600 px the zoom pane gets no width; the other widths are fixed, so the name pane clips first | Menu items and toolbar buttons put a sentence in the first pane; the zoom parts have neither tooltip nor hint |
| Windows 3.x (`QUILL.EXE`) | `win16/examples/quill/chrome.rs` `paint_status`, `main.rs` | One sunken panel holding `quill_app::status_text`, a space and then `D:\SCROLL.RTF   Pg 1 Ln 1 Col 1   Times New Roman 12   OVR   EXT   Mod` | Flat, drawn on the bar's own panel | Flat on the panel: minus, a 70 px box, plus, the 8 px margin (W16-18, W16-19) | Under 560 px the control goes; the text is clipped at its right, so `Mod` goes first | A hint replaces the whole text; the zoom parts have neither |
| DOS graphics (`QUILLDOS.EXE /G`) | `crates/quill-paint/src/shell.rs` `status_line`, `zoombox.rs` | One sunken inset with `status_text`, cut a byte at a time until it fits | A black line on top, a sunken inset, raised zoom buttons | Raised minus and plus (20 px), a sunken 76 px box, the 8 px margin (DOS-44 to DOS-46) | Fixed 640 px; long text loses its end, so `Mod` goes first | None: tooltips on the toolbar only |
| DOS text, 80 × 25 and 80 × 50 | `crates/quill-app/src/paint.rs` `status_into`, the last row | A space, then `(Untitled)   Pg 1 Ln 1 Col 6   Times New Roman 12   OVR   EXT   Mod`, packed from the left, three spaces apart | Black on grey (`attr::CHROME`), no separators | None (the View menu) | 80 bytes and the rest dropped: with a long path or a header view the font, `OVR`, `EXT` and `Mod` all go | None; a message replaces the whole row, `Mod` with it |
| Terminal client (`quill-tty`) | The DOS text painter (`draw` into a 25- or 50-row grid) | As DOS text | As DOS text | None | As DOS text | None |
| GTK (`quill-gtk`) | `linux/quill-gtk/src/status.rs` | The message (expands, ellipsized), `Page 1 of 1`, `Ln 1, Col 1`, `Arial 16`, `Mod` in the accent colour (when modified) | Caption labels, dimmed, 16 px apart, 12 px start margin; no name (the header bar has it) | `list-remove-symbolic` (`Zoom out`), a menu button `100%` (`Zoom`), `list-add-symbolic` (`Zoom in`), then the 8 px margin (LNX-20) | Nothing drops: the labels set the window's least width | No menu help (GNOME menus have none) |
| WinUI 3 | Not built | W6 of the WinUI plan says "status line" and no more | — | — | — | — |

## What is wrong with it

1. **Seven builds, five vocabularies.** `Pg 1 Ln 1 Col 1` and `Page 1 of 1` with `Ln 1, Col 1`; `Mod` and `Modified`; the path, the file name or nothing; `OVR` next to `Modified` in one pane or three spaces apart.
2. **The most important flag drops first.** The DOS text row is cut at 80 bytes, the DOS graphics text a byte at a time, the Win16 text at the zoom control: in every case `Mod` goes before the file name. A header view with a long path (`[First header] C:\DOCS\LETTERS\2026\NOVEMBER\LETTER1.QLD`) leaves no font, no `OVR`, no `EXT` and no `Mod` on the DOS text row (`before-dos-text-25-long.png`).
3. **A message hides the state.** On DOS a message replaces the whole row, so `Mod` vanishes until the next key.
4. **Fields jump.** Everything is packed from the left, so the position, the font and the flags move whenever the name or a number changes length, and `OVR` or `Mod` appearing shifts nothing to its right but changes the row a screen reader reads.
5. **Room spent twice.** The Windows bars show the path the title bar already shows, in a 105 px pane that clips it, and the font and size the formatting bar already shows; the Windows 95 bar's widest pane is usually empty (`evidence/win32-reactos-1024.png`).
6. **Three numbers for one control.** The zoom box is 70, 72 and 76 px; the control leaves at 560 or 600 px or never; the margin is 8 or 16 px. The rule (CORE-67) is one, the constants are not.
7. **The zoom parts are unnamed.** Only GTK has tooltips (in sentence case); no build has a status hint for them, and there is no way back to Page Width but the list.
8. **GTK cannot narrow.** No label can hide or shorten, so the bar sets the window's least width.
9. **No live region, no role.** GTK's bar is a box of labels; a screen reader is never told that a save failed.

## The normalized layout

One left-to-right order, the same on every build, with the state anchored to the right edge:

```text
| Lead (flexible)              | Position | Count | Font | Language | Mode | State | Zoom: − box + reset margin |
```

- **The lead** is the only flexible field. It shows a hint, a message, the code at the caret, the selected picture or the document, in that order of precedence (CORE-202).
- **The other fields are anchored to the right** and laid out from the right edge, so the lead's length never moves them (CORE-201).
- **Position, Mode and State never drop.** Count, Font and Language are optional and drop first when the bar is narrow; the zoom control loses its Page Width button (the outermost part), then its minus and plus, then its box (CORE-203).
- **Two forms.** Every field has a short form (`Pg 3 Ln 14 Col 22`, `Mod`), used on the text screens, Windows 3.x and the DOS graphics screen, and a long form (`Page 3 of 5   Ln 14, Col 22`, `Modified · backed up`), used on Windows 95, GTK and WinUI until the bar is too narrow for it (CORE-201, CORE-203).
- **One function places them.** `quill_chrome::status_bar::layout` takes the bar's width, the shell's metrics and a way to measure text, and returns each field's span and the zoom control's parts. Every shell draws from its answer (CORE-200).

The mock-ups below come from a Python model of that function (`Status bar/tools/make_mockups.py`, `normalized_layout`), so the pictures and the rules are the same rules.

## The fields

| Field | Short form | Long form | Shown | Drops | Accessible name (GTK, WinUI) |
| --- | --- | --- | --- | --- | --- |
| Lead | See [The lead](#the-lead) | The same | Always | Never; it shrinks to its least width, then truncates | The text shown; a message is also announced |
| Position | `Pg 3 Ln 14 Col 22`; in a header or footer `Hdr Ln 2 Col 5`, `Ftr`, `1st Hdr`, `1st Ftr` | `Page 3 of 5   Ln 14, Col 22`; `Header`, `Footer`, `First page header`, `First page footer` in place of the page | Always | Never | `Page 3 of 5, line 14, column 22` |
| Count | `Wd 1234`; with a selection `Wd 12/1234` | `1,234 words`; `12 of 1,234 words` | Where the build has the word count and `[Quill] StatusCount` is on (CORE-208) | Step 5; goes short at step 2 | The long form |
| Font | `Times New Roman 12` (the face cut to 16 characters); on the text screens also `Sup` or `Sub` and the text colour's name when it is not Automatic: `Times New Roman 12 Sup Red` (DOS-104) | The same | Only when no formatting bar shows it: always on the text screens, otherwise when View > Formatting Bar is off, and `[Quill] StatusFont` is on (the text screens ignore the setting: the SPEC says fonts show there only) | Step 6: its face shortens to 8 characters on the text screens, where it never drops; it drops elsewhere | `Font Times New Roman, 12 point` |
| Language | `en-CA` | `en-CA` | Where `[Quill] StatusLanguage` is on; dimmed when Check Spelling as You Type is off | Step 1 | `Spelling language Canadian English, checking as you type` (or `not checking`) |
| Mode | `OVR`, `EXT` | The same | Retro builds: always, the flags greyed when off (CORE-204); modern builds: only when on | Never | `Overtype on`, `Extend selection on` |
| State | `Mod` | `Modified`, or `Modified · backed up` when the recovery copy holds every change (CORE-207) | When there are unsaved changes; retro builds keep its room when clean | Never; goes short at step 4 | `Modified, not saved; a recovery copy holds every change` or `Modified, not saved` |
| Zoom | Minus, the box, plus, the Page Width button, the margin | The same | Every pixel build with a page (not the text screens, not print preview) | Reset at step 7, the buttons at 8, the box at 9 | See [Hints and tooltips](#hints-and-tooltips) |

**Why Mode then State, with State last before the zoom.** `Mod` sits nearest the right edge, where the DOS row has always ended and where the eye goes, and it is the one flag that matters at every width. The owner's list put modified before insert and overwrite; see question 10.

**Selection size** is part of Count (`12 of 1,234 words`), not a field of its own: one number about the text, not two. Where Count is off, the selection shows nothing new; the picture description (`Picture 320 × 200 pixels ...`) stays the lead's, as today.

**"Backed up"** means that the last recovery write (CORE-10 to CORE-20) holds every change since the last save, so a crash loses nothing even though the document is not saved. It is what makes As you work autosave (CORE-161, the default on Windows 95, Linux, WinUI and the web) visible without a flash: the field reads `Modified` while typing and `Modified · backed up` a few seconds after the typing pauses. A document whose copy is held by another window, or with autosave off, never reads backed up. The short form has no room for it and stays `Mod`.

**Clean is blank.** A saved document shows nothing in State on every build: no `Saved`, so the field draws the eye only when there is something to do.

## The lead

What the lead shows, first match wins (CORE-202):

1. **A hint**: the sentence for the menu item that is highlighted, or for the toolbar button, box or zoom part under the pointer.
2. **A message** the last command left (`Printed to LETTER.PDF`, `Quill cannot save; the disk may be full.`), until the next key or command, as now.
3. **The code at the caret**, while Reveal Codes is shown on a build whose pane has no separator text of its own (Windows, GTK, WinUI; CORE-193). On DOS the separator row names it.
4. **The selected picture**: `Picture 320 × 200 pixels, 3.33 × 2.08 in (100%)`.
5. **The document**, by the rule "the status bar never repeats the title": the **path** where there is no title bar (DOS text and graphics, the terminal client), the **file name** where the title shows the path (Windows 3.x, Windows 95), and **nothing** where a header bar shows name and folder (GTK, WinUI).

**A hint or a message may take more room** (CORE-205). When one is wider than the lead, the lead grows over Position, Count, Font and Language, which hide while it shows; Mode, State and the zoom control stay. So a message never hides `Mod` again, and menu help on an 80-column row has room for its sentence (`after-dos-text-25-hint.png`).

**Truncation.** A path loses folders from its left and always keeps the file name (`..\LETTER1.QLD`, `../letter1.qld`); a hint or a message loses its end (`...` on the retro builds, `…` on GTK and WinUI). The lead's least width is 12 cells on the text screens (an 8.3 name with its extension), and the width of eight average characters on the pixel builds.

## Narrowing

When the lead would be narrower than its least width, the bar gives up one thing at a time, in this order, and tries again (CORE-203). A step that does not apply on a build (a field it does not show, a form already short) is skipped.

| Step | What goes | Why this order |
| --- | --- | --- |
| 1 | Language | The least asked-for; it is in Find > Spelling and the Options window |
| 2 | Count goes short (`Wd 1234`) | Keeps the number while saving about half its width |
| 3 | Position goes short (`Pg 3 Ln 14 Col 22`) | The page total is the first word nobody needs to read every second |
| 4 | State goes short (`Mod`) | The flag stays; "backed up" stays in the accessible name and the tooltip |
| 5 | Count | Shown again when the window widens |
| 6 | Font: on the text screens its face shortens a character at a time down to 8 (`Times Ne. 10.5 Sub Fuchsia`) and the field never drops; elsewhere it drops | The formatting bar, where there is one, has it anyway; the text screens have nowhere else to show it, nor superscript, subscript or the colour (DOS-104) |
| 7 | The Page Width button | The outermost part goes first; Page Width is still in the list and on Ctrl+0 (Alt+0 on DOS) |
| 8 | The minus and plus buttons | The list still sets any level |
| 9 | The zoom box | View > Zoom still sets it; this happens only on a window narrower than about 300 px |

Position, Mode, State and the lead never go. What each build reaches at its usual sizes:

| Build | Wide | Narrow |
| --- | --- | --- |
| Windows 95 | 868 px: every field long, Font off while the formatting bar shows (`after-win32-wide.png`) | 528 px: Language gone, Count, Position and State short, the whole zoom control kept (`after-win32-narrow.png`); today the control and nothing else goes |
| Windows 3.x | 808 px: short forms, Font off while the formatting bar shows | 528 px: unchanged but for a shorter lead; today the control goes |
| DOS graphics | 640 px, fixed: short forms, the whole zoom control | — |
| DOS text, terminal | 80 cells, fixed: short forms, the lead from 15 to 23 cells | — |
| GTK | 1,020 px: long forms, Count and Language | 600 px: Language gone, the rest short, the zoom control kept; today the bar stops the window narrowing past about 540 px |
| WinUI | As GTK | As GTK |

## Widths and spacing

One table in `quill_chrome::status_bar` (`Metrics`, one constant a shell) holds every number; the layout function is the only code that adds them up. Widths in cells on the text screens and in device pixels elsewhere; text widths come from the shell's own measure (cells on the text screens, `GetTextExtent` on Windows, the strike widths on the DOS graphics screen, Pango on GTK, DirectWrite on WinUI).

| Shell | Bar height | Left edge | Gap between fields | Inside each field | Lead least | Zoom: Page Width button, minus and plus, box | Right margin | Forms |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| DOS text, terminal | 1 row | 1 cell | 3 cells (` │ `); 1 between Mode and State | 0 | 12 cells | — | 1 cell | Short |
| DOS graphics | 20 px | 4 px | 3 px | 4 px | 72 px | 20, 20, 76 | 8 px (`ZOOM_MARGIN`) | Short |
| Windows 3.x | font height + 6 | 2 px | 3 px | 4 px | 80 px | 22, 20, `Page Width` + 12 | 8 px | Short |
| Windows 95 | the control's own | 0 | the control's 2 px | 4 px | 80 px | 22, 20, `Page Width` + 12 | 16 px (the size grip) | Long, then short |
| GTK | 36 px | 12 px | 18 px | 0 | 60 px | 34, 34, natural | 8 px | Long, then short |
| WinUI | 32 px | 12 px | 16 px | 0 | 60 px | 32, 32, natural | 8 px | Long, then short |

- **The zoom box is sized from its widest text** (`Page Width` in the bar's font, plus 6 px a side) on every build that measures, so 70, 72 and 76 become one rule; the DOS graphics screen keeps 76 because its strike is fixed. `ZoomMetrics` gains `reset` (0 for none).
- **Position keeps a floor** of `Pg 8 Ln 88 Col 88` (17 cells), so it does not move as the caret walks from line 9 to line 10.
- **The flags keep their places** on the retro builds: Mode's room is always `OVR EXT`, State's always `Mod` (or the long form's widest on Windows 95), so nothing on the right moves when a flag comes or goes (CORE-204). The modern builds hide an empty field and let the row reflow, as GNOME and Windows 11 applications do.
- **No minimum client width.** `ZOOM_MIN_CLIENT_W` (600) in `ui.rs` and `ZOOM_MIN_CLIENT` (560) in `chrome.rs` go: narrowing decides.

## The look on each build

| Build | Fields | Separators | Zoom control | Colours and type |
| --- | --- | --- | --- | --- |
| DOS text, terminal | Text on the grey row | `│` (CP437 `0xB3`, U+2502 in the terminal) in the middle of each gap, none inside the flags group | — | `attr::CHROME`; an off flag in dark grey (`0x78`) on the colour screen and blank in mono |
| DOS graphics | One 1-pixel sunken box a field on the face | The gaps | One sunken box, etched lines between minus, the box, plus and the Page Width button; the signs flat, embossed grey when they have no level | `FACE`, `INK`; the Windows 3.1 look of the rest of the screen; a black line along the top as now |
| Windows 3.x | As DOS graphics | The gaps | As DOS graphics, as the zoom polish drew it (flat signs, etched lines) | `COLOR_BTNFACE`; a white line along the top as now |
| Windows 95 | Native `msctls_statusbar32` panes (`SB_SETPARTS` from the layout) | The control's | One owner-drawn pane, etched lines between the parts, the Page Width part after the plus, then the size grip inside its right end | System colours; `DEFAULT_GUI_FONT`; off flags in `COLOR_GRAYTEXT` embossed |
| GTK | `caption` labels; Position, Count, Font, Language `dim-label`; State and Mode `caption-heading` | Spacing only: no lines, no bevels (Adwaita) | A `linked` box of flat buttons: `zoom-out-symbolic`, a menu button with the zoom and its arrow, `zoom-in-symbolic`, `zoom-fit-best-symbolic` | Follows light and dark; no accent on `Modified` (the accent is for actions) |
| WinUI | `CaptionTextBlockStyle`; secondary fields in `TextFillColorSecondaryBrush` | Spacing only | Subtle buttons with Segoe Fluent Icons `ZoomOut` and `ZoomIn` round a `DropDownButton` with the zoom, then `FitPage` | Mica window, the bar on `LayerFillColorDefaultBrush` with a top divider |

The Page Width glyph on the retro builds is an 11 × 9 pixel mark: two bars with a double-headed arrow between them (`after-win32-reset-tip.png`); GTK and WinUI use their icon themes' fit-to-width icons. Nothing in any build's chrome borrows from macOS.

## Hints and tooltips

The zoom parts get a tooltip (the short name) and a hint (the sentence in the lead), on every build that has tooltips, from one table in `quill_chrome::zoom_field` (CORE-206):

| Part | Tooltip | Hint | Greyed when |
| --- | --- | --- | --- |
| Page Width button | `Page Width (Ctrl+0)`; `Page Width (Alt+0)` on DOS | `Fit the page to the width of the window` | The zoom is already Page Width |
| Minus | `Zoom Out` | `Show the page smaller` | No level below the percentage shown (CORE-67) |
| Box | `Zoom (click for the list)` | `Choose how large the page is shown` | Never (print preview: the control does nothing, as now) |
| Plus | `Zoom In` | `Show the page larger` | No level above |

- **Ctrl+0 is Page Width on every build but DOS** (CORE-206): one accelerator in the menu tree (`View > Zoom > Page Width`, `Ctrl+0`) and `accel::from_vk`'s `0x30` on Windows. GTK's Ctrl+0 (100 % today, LNX-3) becomes Page Width and its Ctrl+9 stays as a second key for it (LNX-91).
- **Alt+0 on DOS** (DOS-105). The DOS screens read keys with INT 16h, which gives no Ctrl with a digit but gives Alt with one: AL 0 and AH 78h to 80h for Alt+1 to Alt+9 and **81h for Alt+0** (82h is Alt+minus, 83h Alt+equals). `quill_app::bios::decode` already turns 81h into `Key::Alt(b'0')` (its `alt_letter` maps 78h to 80h and 81h), and the DOSBox scenarios already type `alt1`, `alt2` and `alt3` into the file dialog's sort keys through the same decoder, equal to the host. In plain editing `Key::Alt` goes to `menu_for_mnemonic`, and no menu title has a digit for its mnemonic, so Alt+0 is free; inside a menu it is the same; the file dialog takes Alt+1 to Alt+3 itself and leaves Alt+0 alone. So on both DOS screens Alt+0 runs `Command::Zoom(0)`, as View > Zoom > Page Width does (on the text screens that sets the remembered zoom only, as the menu item does). It is shown where DOS shows keys: the zoom list's Page Width row reads `Page Width   Alt+0`, the button's tooltip on the graphics screen is `Page Width (Alt+0)`, the help's Keyboard shortcuts topic (`assets/help/quill.qhm`) says `Ctrl+0 Page Width (Alt+0 on DOS)`, and the SPEC's DOS keys name it. WP-BAR-1 proves 81h in DOSBox with an `alt0` line in the `g11-gfx-zoom` scenario.
- **The reset button** runs `Command::Zoom(0)`, the same command as the list's Page Width, so it adds no state.
- **Tooltips are Title Case**, as every other tooltip in the SPEC (`Font Size`, `Print Preview`); GTK's `Zoom out` and `Zoom in` change.
- **Other fields** get a tooltip on Windows 95, GTK and WinUI where the field's short text needs one: State (`Modified, not saved; a recovery copy holds every change`), Language (`Spelling: Canadian English`), Position in its short form (`Page 3 of 5, line 14, column 22`). The retro builds without a tooltip control for the bar (Windows 3.x, DOS) do not add one for these.
- **Menu help on DOS** (WP-BAR-9): the DOS screens show `command_hint` in the lead while a menu item is highlighted and, on the graphics screen, while the pointer rests on a toolbar button or a zoom part, as the Windows builds do. It is the last package because it costs most of the DOS budget left (see [Budgets](#budgets)).

## Reveal Codes

Reveal Codes ([reveal-codes.md](reveal-codes.md)) puts a separator and a pane between the page and the status bar. The status bar does not change with it:

- **DOS text and graphics**: the separator row names the code after the caret at its right end (DOS-90, CORE-193). The status row stays the last row and is not repeated: the code's name is not put in the lead (`after-dos-text-50-codes.png`).
- **Windows, GTK and WinUI**: the code's long name goes to the lead at precedence 3, below a hint and a message (CORE-202). W9X-95's "first pane" is the lead.
- **The page's rows**: the pane and its separator come out of the page area, never out of the status bar.

## View > Status Bar and the Options window

- **View > Status Bar** (`[Quill] StatusBar`) hides and shows the whole bar on every build, as now; while it is hidden there is no zoom control and no hint line, and the menu help is not shown anywhere.
- **Which fields** (CORE-208): three settings in the Options window's View page, beside the bars, with per-build defaults.

| Setting | Key | Values | Default | Builds |
| --- | --- | --- | --- | --- |
| Word count in the status bar | `[Quill] StatusCount` | 0, 1 | 1 on Windows 95, GTK, WinUI; 0 elsewhere | Not on DOS or Windows 3.x (the code is left out of those builds) |
| Font and size in the status bar | `[Quill] StatusFont` | 0, 1 | 1 | All; shown only when no formatting bar shows them |
| Spelling language in the status bar | `[Quill] StatusLanguage` | 0, 1 | 1 on Windows 95, GTK, WinUI; 0 elsewhere | Not on DOS or Windows 3.x |

Position, Mode, State and the zoom control are not optional: they are what the bar is for. The keys follow the existing `Toolbar`, `FormattingBar`, `Ruler` and `StatusBar` keys; [preferences.md](preferences.md)'s schema takes them as three more rows of its View page when WP-PREF-2 has merged, and until then `each_setting` and `apply_setting` carry them.

## Accessibility

- **GTK** (LNX-90): the bar has `gtk::AccessibleRole::Status`, so Orca's "speak the status bar" command reads it; each field's accessible label is its long sentence from the table above (`Page 3 of 5, line 14, column 22`), whatever form shows; each zoom part's label is its tooltip without the key (`Zoom Out`, `Zoom, 100 percent`, `Zoom In`, `Page Width`). A message, and only a message, is announced with `gtk_accessible_announce` at polite priority (GTK 4.14 and later; the floor is 4.20): `Quill cannot save; the disk may be full.` is spoken, the caret's position and the hints are not.
- **WinUI** (WUI-80): the same names through `AutomationProperties.Name`; the lead's `TextBlock` has `AutomationProperties.LiveSetting="Polite"` and raises `LiveRegionChanged` for a message only.
- **Windows 95** (W9X-122): every field but the zoom control stays a real text pane, so the system's status bar accessibility (and Active Accessibility where installed) reads `SB_GETTEXT`; the zoom pane is owner-drawn, so its parts are named by their tooltips, and View > Zoom and Ctrl+0 are the keyboard's way to them.
- **Windows 3.x and DOS**: no accessibility interface; a DOS screen reader reads the screen buffer, and the right-anchored fields keep `Mod` and `OVR` in the same columns every time, so a reader's "read the status line" key says the same thing in the same order.
- **One sentence table.** The spoken sentences are `quill_chrome::status_bar::spoken`, behind a `spoken` feature that only GTK and WinUI turn on, so the retro executables carry none of them.

## What is shared and what each shell draws

**Shared, in `crates/quill-chrome/src/status_bar.rs`** (new, `no_std`, no `alloc`, no floats; CORE-200):

```rust
pub enum Field { Lead, Position, Count, Font, Language, Mode, State }
pub enum Form { Short, Long }
pub enum Lead { Hint, Message, Code, Picture, Document }

/// What the bar says, gathered once a paint from the editor and the shell.
pub struct Status { /* lead kind and text, story, page, pages, line, column, face, size, words,
	selected words, language, as-you-type, overtype, extend, modified, backed up, zoom */ }
impl Status {
	pub fn of<M: Measure + Clone, S: ReadStore + ?Sized>(app: &App<M>, s: &S, shell: &ShellState) -> Status;
}

/// One shell's numbers (the table under "Widths and spacing").
pub struct Metrics { pub gap: i32, pub joined_gap: i32, pub pad: i32, pub edge_left: i32, pub lead_min: i32,
	pub long: bool, pub reserve_flags: bool, pub names: Names, pub zoom: Option<ZoomMetrics>, pub margin: i32 }

/// Which optional fields the settings and the chrome allow.
pub struct Shown { pub count: bool, pub font: bool, pub language: bool }

pub struct Placement { pub spans: [Option<(i32, i32)>; 7], pub forms: [Form; 7], pub zoom: Option<ZoomLayout> }

pub fn layout(width: i32, m: &Metrics, shown: Shown, status: &Status, measure: &mut dyn FnMut(&[u8]) -> i32) -> Placement;
pub fn field_text(status: &Status, field: Field, form: Form, out: &mut [u8]) -> usize; // Windows-1252
pub fn lead_text(status: &Status, max: i32, measure: &mut dyn FnMut(&[u8]) -> i32, out: &mut [u8]) -> usize;
pub const NARROWING: [Step; 9];
#[cfg(feature = "spoken")]
pub fn spoken(status: &Status, field: Field, out: &mut [u8]) -> usize;
```

`quill_chrome::zoom_field` gains `ZoomPart::Reset`, `ZoomMetrics::reset`, `ZoomLayout::reset` and the parts a narrowed control keeps, `zoom_tip(part)`, `zoom_hint(part)` and `reset_live(zoom)` (WP-BAR-1). `quill_app::status_text` stays until the last shell has moved, then goes with its host test (WP-BAR-6).

**Each shell** draws what `layout` returns, in its own look, and nothing else: the DOS text painter puts cells in the grid (`quill-app` `paint.rs`, which the terminal client shares); `quill-paint` draws the graphics screen's boxes (`shell.rs`, `zoombox.rs`); Windows 3.x paints with GDI (`chrome.rs`); Windows 95 sets the parts and the pane texts and owner-draws the zoom pane (`ui.rs`); GTK and WinUI hide and show their widgets from `Placement` in their layout pass. A shell's `Metrics` is a `const` beside its painter.

**What the editor adds** (one ED-lane slot, WP-BAR-2): `App::backed_up()` (the last recovery write holds every change), and, in WP-BAR-8, the word count as an idle job (the slot CORE-17 already lists) with `App::word_count()` and `App::selection_words()`.

## Budgets

| Limit | Now | This plan | Rule |
| --- | --- | --- | --- |
| `QUILLDOS.EXE` | 437,760 of 442,368 bytes (4,608 free) | WP-BAR-1 +0.3 to +0.5 KB, WP-BAR-3 +0.5 to +1.2 KB (the shared layout and the right-anchored row, less `status_into`), WP-BAR-4 +0.3 to +0.8 KB **[E]**; about +1.1 to +2.5 KB in all, leaving 2.1 to 3.5 KB. Count, Language and the spoken sentences are left out of the DOS build (`status-extras` and `spoken` off). WP-BAR-9 (DOS menu help) adds +2.0 to +2.6 KB **[E]** (the 57 hint sentences are 1,696 bytes) | Past the budget, the DOS-70 rule: raise it by the measured cost rounded up to 4 KB in a DOS row of the package's own, with the floppy's free bytes from `verify-disks.sh`; WP-BAR-9 is expected to need it (question 8) |
| Win16 data segment | 1,908 bytes free | WP-BAR-1 about +220 bytes (four tooltips and four hints); WP-BAR-6 about +80 (the field table, the story tags); at most 300 in all, leaving 1,600 or more. Once WP-ARCH-9 has merged, the strings go to the string table and the cost is the table only | Nothing merges below 1,024 free (W16-60) |
| Win16 code segments | `check_segments.py` | The layout function lands in `quill-chrome`'s segment rule | Each package reports the segment sizes |
| `QUILL32.EXE` imports | `imports.allow` | None: the tooltips use `tooltips_class32`'s `TTM_ADDTOOLA` with a rectangle, already imported | A new import is named in the brief |
| Floppies | `verify-disks.sh` | Reported by WP-BAR-3, WP-BAR-4 and WP-BAR-9 | G-DISK |

## Mock-ups

SVG masters and 2× PNG renders in `~/Documents/Liminal HQ/Quill/Status bar/`, in the side-by-side viewer's pairs: `before-<name>.png` is the bar on `main` as the code draws it, `after-<name>.png` the proposal. `tools/render.sh` writes both from `tools/make_mockups.py`; `masters/` holds the SVGs and `index.tsv` the captions; `evidence/` holds crops of real screenshots of today's bars. The Windows shots use the Afterglow ReactOS frame, the GTK shots an Adwaita window, the text screens the VGA palette at 720 × 400.

| Name | What it shows |
| --- | --- |
| `win32-wide`, `win32-narrow` | Windows 95 on ReactOS at 868 and 528 px |
| `win32-hint` | The menu help for File > Save As keeping `Modified · backed up` and the zoom control |
| `win32-reset-tip`, `dos-gfx-reset-tip` | The pointer on the new Page Width button: `Page Width (Ctrl+0)`, and `Page Width (Alt+0)` on DOS |
| `win32-long` | A first-page header, overtype, extend and a selection |
| `win16-wide`, `win16-narrow`, `win16-long` | Windows 3.x at 808 and 528 px, and the long case |
| `dos-gfx`, `dos-gfx-long`, `dos-gfx-hint` | The DOS graphics screen: plain, a long path in a header view, menu help |
| `dos-text-25`, `dos-text-25-long`, `dos-text-25-hint` | The DOS text screen: plain, the long case that loses every flag today, menu help |
| `dos-text-25-format`, `dos-text-25-format-long` | Red superscript, and fuchsia subscript in the long case: the face shortens; the size, `Sup` or `Sub` and the colour stay (DOS-104) |
| `dos-text-50-codes` | 80 × 50 with Reveal Codes: the separator names the code, the status row is unchanged |
| `tty`, `tty-long`, `tty-format` | The terminal client, with Unix paths, and navy subscript |
| `gtk-wide`, `gtk-narrow`, `gtk-dark`, `gtk-reset-tip` | GTK at 1,020 and 600 px, dark with the long case, and the reset tooltip |
| `winui-wide`, `winui-narrow` | WinUI as proposed; the before is a placeholder, since nothing is built |

## Decisions

Proposed here; each package copies its rows into `docs/decisions.md` (Status **Assumed** unless the owner has answered) and the SPEC in the same change. The ids are reserved in [work-packages.md](../work-packages.md#decision-ids): **CORE-200 to CORE-209**, **DOS-100 to DOS-109**, **W16-110 to W16-114**, **W9X-120 to W9X-124**, **LNX-90 to LNX-94**, **WUI-80 to WUI-84**.

| Id | Decision | Choice | Instead of | Why |
| --- | --- | --- | --- | --- |
| CORE-200 | One status bar model | `quill_chrome::status_bar`: the fields, their forms and texts, the narrowing table and one `layout` function that returns spans in cells or pixels; every shell draws from it | Each shell composing its own text (today's `status_into`, `status_update`, `StatusInfo`) | Seven builds drifted into five vocabularies; one function cannot drift |
| CORE-201 | Order and forms | Lead, Position, Count, Font, Language, Mode, State, Zoom; everything but the lead anchored to the right edge; short forms on the text screens, Windows 3.x and DOS graphics, long forms on Windows 95, GTK and WinUI | Packing from the left; one form | Fields that never move; long words where there is room |
| CORE-202 | The lead | Hint, message, code at the caret, selected picture, document; the path where there is no title bar, the file name where the title has the path, nothing where a header bar has it; a path loses folders from the left, a sentence its end | The path everywhere; an empty first pane | The status bar never repeats the title, and the file name always survives |
| CORE-203 | Narrowing | The nine steps of [Narrowing](#narrowing); Lead, Position, Mode and State never drop; no minimum client width | Thresholds per build (560, 600 px); clipping at the right | `Mod` was the first thing to go; now it is never gone |
| CORE-204 | Fixed flags on the retro builds | Mode's and State's room is kept when empty; off flags are greyed (Windows, DOS graphics) or dark grey (DOS text, blank in mono); the modern builds hide an empty field | Flags that come and go everywhere | Nothing moves on a fixed screen, and a screen reader finds `Mod` in the same column; GNOME and Windows 11 reflow |
| CORE-205 | A hint or message takes room | A hint or message wider than the lead covers Position, Count, Font and Language while it shows; Mode, State and Zoom stay | A message replacing the whole row (DOS today); hints clipped to the lead | Menu help fits on 80 columns, and a message never hides `Mod` |
| CORE-206 | The reset button and the zoom names | **Owner, 2026-11-27:** a Page Width button right of the plus, before the margin (`Command::Zoom(0)`, greyed at Page Width); Ctrl+0 is Page Width in the menu tree and `accel::from_vk` (Alt+0 on DOS, DOS-105); tooltips `Page Width (Ctrl+0)`, `Zoom Out`, `Zoom (click for the list)`, `Zoom In` and the hints of [Hints and tooltips](#hints-and-tooltips), one table in `zoom_field` | No reset; the button left of the minus; Ctrl+0 as 100 % | The owner's request; a way back to the default that needs no list, at the end of the control where it does not split minus, box and plus |
| CORE-207 | The save state | Blank when clean; `Mod` short; `Modified` long, and `Modified · backed up` when the recovery copy holds every change (`App::backed_up`) | A separate autosave indicator; `Saved` when clean; an `Autosaved.` flash | As you work autosave made visible in the one field that already says "not saved" |
| CORE-208 | Optional fields | Count (words; `12 of 1,234 words` with a selection, counted by the idle job), Font (only without a formatting bar) and Language; `[Quill] StatusCount`, `StatusFont`, `StatusLanguage` with per-build defaults; Count and Language left out of the DOS and Windows 3.x builds | A character count; a separate selection field; the font always | Words are what a writer counts; the formatting bar already shows the font |
| CORE-209 | Spoken text | One sentence a field (`Page 3 of 5, line 14, column 22`), behind the `spoken` feature; only a message is announced | Announcing every change | The position changes at every key; a failed save should be heard once |
| DOS-100 | The text row | The lead from column 1, the rest anchored at column 79, ` │ ` between fields, `OVR EXT Mod` one group of fixed places, the path cut from the left; the 25- and 50-row screens and the terminal client alike | Today's 80-byte cut | Every field fits 80 columns at once (`after-dos-text-25-long.png`) |
| DOS-101 | The graphics bar | One 1-pixel sunken box a field on the face, the zoom control one box with etched dividers and the Page Width part, the strike widths as the measure | One inset with the old text | The Windows 3.x bar's look, from the same layout |
| DOS-102 | What DOS leaves out | Count, Language and the spoken sentences (`status-extras` and `spoken` off) | All fields everywhere | The budget: 4,608 bytes left |
| DOS-104 | Superscript, subscript and colour on the text screens | **Owner, 2026-11-27: the SPEC wins.** The text screens' Font field ends with `Sup` or `Sub` when the character at the caret is raised or lowered, and the text colour's name (the nearest of the sixteen in `quill_chrome::view::COLOURS`) when it is not Automatic, in colour and in mono; to fit 80 columns the face shortens to 8 characters and the field never drops (the terminal client alike) | Correcting the SPEC to what is drawn | The SPEC's attribute and feature tables promise it, and the status line is the only place the text screens can say it |
| DOS-105 | Page Width from the DOS keyboard | Alt+0 (INT 16h AH 81h, AL 0) runs `Command::Zoom(0)` on both DOS screens; the zoom list, the graphics tooltip, the help and the SPEC show `Alt+0` | Ctrl+0, which the BIOS never reports; no key on DOS | The owner left the route to the plan; Alt+0 is free (menu mnemonics are letters, the file dialog uses Alt+1 to Alt+3) and the decoder already reads it |
| DOS-103 | Menu help on DOS | `command_hint` in the lead while a menu item is highlighted (both screens) and the pointer rests on a toolbar button or zoom part (graphics), by CORE-205; built last, under DOS-70's raise rule | No hints on DOS | The Windows builds have had them since D100 |
| W16-110 | The Windows 3.x bar | One sunken box a field and the zoom box, from `layout` with `Metrics` beside `paint_status`; `ZOOM_MIN_CLIENT` goes | One panel with the shared string | The same layout as every build, in the 3.1 look |
| W16-111 | Windows 3.x data | At most 300 bytes for the plan; the strings move to the string table with WP-ARCH-9 | Strings in the data segment for good | 1,908 bytes free, a 1,024 floor |
| W16-112 | Windows 3.x tooltips for the zoom | `tips.rs` gains the four zoom parts as targets, with the tooltip and the hint | None | The same names on every build |
| W9X-120 | The Windows 95 panes | `SB_SETPARTS` from `layout` on every size and hint change; the texts set only when they change, as now; `ZOOM_MIN_CLIENT_W` goes | Fixed pane widths | One rule, and the widest pane is no longer the empty one |
| W9X-121 | Windows 95 tooltips for the bar | `TTM_ADDTOOLA` rectangle tools on the status bar for the four zoom parts (and State, Language and short Position), moved on every `SB_SETPARTS`; the hint in the lead | Nothing on the zoom parts | Named parts; no new import |
| W9X-122 | Windows 95 accessibility | Text panes stay real text; only the zoom pane is owner-drawn | An owner-drawn bar | `SB_GETTEXT` reads every field |
| LNX-90 | The GTK bar | Adwaita: caption labels, spacing only, a linked group of flat buttons with `zoom-out-symbolic`, the menu button, `zoom-in-symbolic`, `zoom-fit-best-symbolic`; `AccessibleRole::Status`; messages announced; State without the accent colour | Today's bar with `list-remove`/`list-add` | Adwaita's own zoom icons and no bevels |
| LNX-91 | GTK keys | Ctrl+0 is Page Width; Ctrl+9 stays a second key for it; 100 % is in the list | Ctrl+0 for 100 % (LNX-3) | One meaning for Ctrl+0 on every build |
| LNX-92 | GTK narrowing | A `gtk::LayoutManager` on the bar that measures each child's natural width, calls `layout` and hides what it drops, so the window can narrow past the bar | A fixed bar | The window is no longer held wide by its status bar |
| LNX-93 | The terminal client | The DOS text row (DOS-100) with U+2502 and `/` paths | Its own row | It shares the painter |
| WUI-80 | The WinUI bar | The same model and forms as GTK, in Fluent: caption text, a top divider, subtle `ZoomOut` and `ZoomIn` round a `DropDownButton`, then `FitPage`; `LiveSetting` polite on the lead for messages | A status `InfoBar`; a zoom slider | The model, not a new design |
| WUI-81 | WinUI keys | Ctrl+0 is Page Width, as on every build but DOS | — | — |

## Work packages

Build order, three builders at most. `WP-BAR-1` and `WP-BAR-1G` are the zoom reset and tooltip work the owner asked for, and go first; nothing else waits for them except `WP-BAR-2`'s zoom parts.

| Wave | Retro lane | Shared and ED lane | UI lane | Waits for |
| --- | --- | --- | --- | --- |
| 1 | WP-BAR-1 zoom reset and tooltips (shared, Win32, Win16, DOS graphics) | — | WP-BAR-1G GTK reset, tooltips, icons, Ctrl+0 | Nothing |
| 2 | — | WP-BAR-2 the status model and layout | — | WP-BAR-1 merged; an ED-lane slot for `App::backed_up` |
| 3 | WP-BAR-5 Windows 95 | WP-BAR-3 the text screens and the terminal | WP-BAR-6 Windows 3.x | WP-BAR-2 merged; WP-BAR-3 needs the ED-lane slot in `paint.rs` and no Unicode freeze open |
| 4 | WP-BAR-4 DOS graphics | WP-BAR-8 word count, Language and the settings | WP-BAR-7 GTK | WP-BAR-3 for WP-BAR-4 (shared row text) and WP-BAR-8 (ED lane); WP-BAR-1G for WP-BAR-7 |
| 5 | WP-BAR-9 DOS menu help | — | WinUI W6 takes WUI-80 and WUI-81 | WP-BAR-3 and WP-BAR-4; the owner's answer to question 8 |

`WP-BAR-6` is Windows 3.x but sits in the UI lane's slot in wave 3 because the retro lane holds Windows 95 then; the two never share a file. Reveal Codes' `WP-CODES-2` and `WP-BAR-3` both edit `crates/quill-app/src/paint.rs` and take turns in the ED lane: whichever starts second waits.

### WP-BAR-1 — The zoom reset button and the zoom tooltips

- **Finish line:** the zoom control on Windows 95, Windows 3.x and the DOS graphics screen has a Page Width button right of the plus, greyed at Page Width, that runs `Command::Zoom(0)`; each of the four parts has a tooltip (`Page Width (Ctrl+0)`, or `(Alt+0)` on DOS, `Zoom Out`, `Zoom (click for the list)`, `Zoom In`) and a hint in the status bar; Ctrl+0 is Page Width on Windows 95 and Windows 3.x (the menu shows it) and Alt+0 on both DOS screens (the zoom list shows it), proved in DOSBox; the help's key list names both; the box is sized from `Page Width`'s width on the Windows builds; the rest of each bar is unchanged.
- **Owns:** `crates/quill-chrome/src/zoom_field.rs`; `crates/quill-app/src/accel.rs` (one arm), the `ZOOM` table in `crates/quill-app/src/paint.rs` (one accelerator) and a `Key::Alt(b'0')` arm in `crates/quill-app/src/lib.rs`'s editing keys (ED-lane slot, a few lines in all); `assets/help/quill.qhm` (the Keyboard shortcuts line); `crates/quill-paint/src/drop.rs` (the zoom list's `Alt+0`); `crates/quill-paint/src/zoombox.rs`, `crates/quill-paint/src/tip.rs`; `win9x/quill-win9x/src/ui.rs` (the status bar section), `win9x/quill-win9x/src/tips.rs`; `win16/examples/quill/chrome.rs` (`paint_status`, `zoom_layout`, `ZOOM_METRICS`), `win16/examples/quill/tips.rs`, the zoom part of `win16/examples/quill/main.rs`'s click handler.
- **Must not touch:** `linux/**` (WP-BAR-1G); the other fields of any bar (WP-BAR-3 to WP-BAR-7); `quill_app::status_text`.
- **Depends on:** nothing.
- **Budget:** DOS +0.3 to +0.5 KB; Win16 data about +220 bytes (report the figure; stop below 1,024 free); no new import.
- **Tests:** a host test that `bios::decode(0, 0x81)` is `Key::Alt(b'0')`, that it runs `Command::Zoom(0)` in plain editing and opens no menu; `zoom_field` units (the layout with and without the Page Width part, `hit` on every part's edges, `reset_live`, the tips and hints tables non-empty and Title Case); `quill-paint` zoom and pointer tests for the reset part (press, release, greyed at Page Width); `crates/quill-host/tests` for Ctrl+0 through `from_vk`; G-HOST, G-DOS (with `g11-gfx-zoom` extended by a Page Width click and an `alt0` key, which proves INT 16h's 81h in DOSBox), G-W16 with `e2e-zoombox-w31.sh` extended, G-W32 with `wine-zoom.sh` and `wine-layout.sh` at 640, 800 and 1,024.
- **Evidence:** `~/Documents/Liminal HQ/Quill/Status bar/WP-BAR-1/`: `before-` and `after-` `win32-wine`, `win16-311`, `dos-gfx-12h`, `dos-gfx-vbe` at 100 % and at Page Width (reset greyed), one with each tooltip showing on Windows 95 and Windows 3.x.
- **Decisions:** CORE-206, DOS-105, W16-112, W9X-121 (the zoom part).

### WP-BAR-1G — The GTK reset button, tooltips and icons

- **Finish line:** GTK's zoom control is a `linked` box of flat buttons: `zoom-out-symbolic`, the menu button, `zoom-in-symbolic`, then `zoom-fit-best-symbolic` (Page Width, greyed at Page Width), with the four tooltips from `zoom_field`; Ctrl+0 is Page Width and Ctrl+9 stays; the rest of the bar is unchanged.
- **Owns:** `linux/quill-gtk/src/status.rs` (the zoom part), `linux/quill-gtk/src/window.rs` (the actions and keys), `linux/quill-gtk/src/view.rs` (`zoom_reset`), `docs/linux.md` (the zoom lines).
- **Must not touch:** `crates/**` (it reads `zoom_field` as WP-BAR-1 leaves it; until then it carries the four strings locally with a comment naming the table they move to).
- **Depends on:** nothing; rebases on WP-BAR-1 to read the shared table.
- **Tests:** `cargo test` in `linux/quill-gtk` (the tooltips, the reset action, Ctrl+0 and Ctrl+9); G-GTK with `chrome-shots.sh` (light and dark, the reset greyed and live).
- **Evidence:** `Status bar/WP-BAR-1G/` `before-` and `after-` `gtk-xvfb` light and dark.
- **Decisions:** LNX-91, LNX-90 (the zoom part).

### WP-BAR-2 — The status model and the layout function

- **Finish line:** `quill_chrome::status_bar` as [What is shared](#what-is-shared-and-what-each-shell-draws) describes: `Status::of`, `Metrics`, `Shown`, `layout`, `field_text`, `lead_text`, `NARROWING`, `spoken` (feature), and a `Metrics` constant for each shell in the module's tests; `App::backed_up`; no shell changes.
- **Owns:** `crates/quill-chrome/src/status_bar.rs` (new), `crates/quill-chrome/src/lib.rs` (the `mod` line), `crates/quill-chrome/Cargo.toml` (the features), `crates/quill-app/src/recovery.rs` (`backed_up` and the flag it reads; ED-lane slot), `crates/quill-host/tests/status_bar.rs` (new).
- **Must not touch:** every shell; `paint.rs`.
- **Depends on:** WP-BAR-1 merged.
- **Budget:** nothing links it yet: every executable the same size.
- **Tests:** table tests of `layout` for every shell's metrics at the widths of the mock-ups (80 cells; 640; 528, 600, 808, 868 and 1,020 px) against the spans `make_mockups.py` prints (the brief says how); properties over random widths and statuses: no two spans overlap, every span is inside the bar, the zoom margin is constant, the narrowing steps are taken in order and a wider bar never shows fewer fields; `field_text` for every form and story; `lead_text` truncation of paths and sentences; `backed_up` through an autosave cycle in As you work and Every N minutes.
- **Evidence:** `Status bar/WP-BAR-2/layout-table.txt`, the spans the tests check.
- **Decisions:** CORE-200 to CORE-205, CORE-207, CORE-209.

### WP-BAR-3 — The text screens and the terminal client

- **Finish line:** the DOS 25- and 50-row screens, mono and colour, and `quill-tty` draw the status row from `layout` (DOS-100, LNX-93); messages keep the flags (CORE-205); the Font field names superscript, subscript and the text colour as the SPEC requires (DOS-104), shortening the face to fit and never dropping; `quill_app::status_text` is now built from `field_text`.
- **Owns:** `crates/quill-app/src/paint.rs` (`status_into`, `Row`, the status part of `draw_rows`; ED-lane slot), `tests/golden/screens/**`, `tests/golden/flows/**` where a status row changes, `linux/quill-tty/tests`.
- **Must not touch:** `quill-paint` (WP-BAR-4); the Windows shells.
- **Depends on:** WP-BAR-2; no Unicode freeze open; not at the same time as WP-CODES-2.
- **Budget:** DOS +0.6 to +1.5 KB (DOS-104 is about 0.1 to 0.3 KB of it: the colour names are already in the executable for the graphics screen's colour list).
- **Tests:** every screen golden rewritten with `QUILL_UPDATE_GOLDEN=1` and checked to change in the status row and its attribute row only (a filter over `git diff tests/golden/screens` the brief gives); new `m19-status` scenario: a long path, a header view, OVR, EXT, Mod, a message, at 25 and 50 rows and mono; new `m19-status-format` scenario with goldens at 25 rows, 50 rows and mono: the caret in superscript, in subscript, in red, in a colour that is not one of the sixteen (named by the nearest), in Automatic (no name), and in subscript fuchsia with a long path and a header view (the face shortened; the size, `Sub`, `Fuchsia` and `OVR EXT Mod` kept); the terminal client's scenario test the same; G-HOST, G-DOS, G-DISK.
- **Evidence:** `Status bar/WP-BAR-3/` `before-` and `after-` `dos-text-25`, `dos-text-50`, `dos-text-25-mono`, the long case, the format cases (`dos-text-25-format`, `dos-text-25-format-long`, `dos-text-25-mono-format`), `tty` and `tty-format`.
- **Decisions:** DOS-100, DOS-102, DOS-104, LNX-93.

### WP-BAR-4 — The DOS graphics screen

- **Finish line:** the graphics status bar draws from `layout` in boxes (DOS-101), in 12h and VBE; the text no longer clips from its end.
- **Owns:** `crates/quill-paint/src/shell.rs` (`status_line`), `crates/quill-paint/src/zoombox.rs`, `tests/golden/gfx/**`.
- **Must not touch:** `paint.rs`; the Windows shells.
- **Depends on:** WP-BAR-3 (the shared texts it builds on).
- **Budget:** DOS +0.3 to +0.8 KB.
- **Tests:** the gfx goldens rewritten and checked to change below the page area only; `gfxmouse` tests for the zoom hits at their new places; G-HOST, G-DOS, G-DISK.
- **Evidence:** `Status bar/WP-BAR-4/` `before-` and `after-` `dos-gfx-12h`, `dos-gfx-vbe`, the long case.
- **Decisions:** DOS-101.

### WP-BAR-5 — Windows 95

- **Finish line:** `QUILL32.EXE` sets its parts from `layout` on every size, hint and message change (W9X-120), long forms with the narrowing steps, the font pane only without the formatting bar, the flags greyed in their places, tooltips on State, Language and the short Position (W9X-121), the zoom pane as WP-BAR-1 left it.
- **Owns:** `win9x/quill-win9x/src/ui.rs` (the status bar section), `win9x/quill-win9x/src/tips.rs`, `win9x/quill-win9x/src/main.rs` (the calls into the status bar only).
- **Must not touch:** `crates/**`; the zoom pane's drawing.
- **Depends on:** WP-BAR-2.
- **Budget:** no new import; report the size.
- **Tests:** G-W32 with `wine-layout.sh` at 528, 640, 800, 868 and 1,024 px, `wine-zoom.sh`, `wine-e2e.sh`; a ReactOS shot through the remix recipe if the builder has it.
- **Evidence:** `Status bar/WP-BAR-5/` `before-` and `after-` `win32-wine` at each width, with a hint, a message and the long case.
- **Decisions:** W9X-120, W9X-121, W9X-122.

### WP-BAR-6 — Windows 3.x

- **Finish line:** `QUILL.EXE` paints one sunken box a field from `layout` (W16-110), the flags greyed in their places, `ZOOM_MIN_CLIENT` gone; `quill_app::status_text` and its host test go once no shell calls it.
- **Owns:** `win16/examples/quill/chrome.rs` (`paint_status`), `win16/examples/quill/main.rs` (the status paint call), `win16/examples/quill/preview.rs` (its status line takes the same boxes), `crates/quill-app/src/paint.rs` (deleting `status_text` only, after WP-BAR-3 and WP-BAR-4 have merged), `crates/quill-host/tests/view.rs` (its status test).
- **Must not touch:** the Windows 95 shell; the zoom part (WP-BAR-1).
- **Depends on:** WP-BAR-2 (and WP-BAR-3 and WP-BAR-4 for the deletion; without them it leaves `status_text` and says so).
- **Budget:** Win16 data about +80 bytes; segments within `segments.json`.
- **Tests:** G-W16 with `e2e-zoombox-w31.sh`, `e2e-scroll-w31.sh`, `e2e-preview-w31.sh`.
- **Evidence:** `Status bar/WP-BAR-6/` `before-` and `after-` `win16-311` at 640, 808 and 1,024, the long case, the preview.
- **Decisions:** W16-110, W16-111.

### WP-BAR-7 — GTK

- **Finish line:** the GTK bar from `layout` through a `gtk::LayoutManager` (LNX-92), long and short forms, Count and Language, State without the accent, `AccessibleRole::Status`, the spoken labels and announced messages (LNX-90).
- **Owns:** `linux/quill-gtk/src/status.rs`, `linux/quill-gtk/src/window.rs` (the refresh call), `docs/linux.md` (the status bar lines).
- **Must not touch:** `crates/**`.
- **Depends on:** WP-BAR-2, WP-BAR-1G; WP-BAR-8 for the count (until then Count is not shown).
- **Tests:** `cargo test` (the layout hides in the order of the table at 600, 800 and 1,020; the accessible labels); G-GTK with `chrome-shots.sh` light and dark at 600 and 1,020; an AT-SPI dump of the bar's role and labels.
- **Evidence:** `Status bar/WP-BAR-7/` `before-` and `after-` `gtk-xvfb` at both widths, light and dark.
- **Decisions:** LNX-90, LNX-92.

### WP-BAR-8 — Word count, Language and the field settings

- **Finish line:** the word count as an idle job (`App::word_count`, `App::selection_words`), the Count and Language fields on Windows 95 and GTK, `[Quill] StatusCount`, `StatusFont`, `StatusLanguage` read and written, and their rows in the Options window's View page where it exists (CORE-208); the DOS and Windows 3.x builds leave Count and Language out.
- **Owns:** `crates/quill-app/src/idle.rs` (the word count slot; ED-lane slot), `crates/quill-app/src/lib.rs` (the three settings in `each_setting` and `apply_setting`, or the schema rows if WP-PREF-2 has merged), `crates/quill-chrome/src/status_bar.rs` (`Shown` from the settings).
- **Must not touch:** the shells' painters (they read `Shown`).
- **Depends on:** WP-BAR-3 (ED lane order).
- **Budget:** DOS and Win16 unchanged (left out); report both.
- **Tests:** host tests of the count (words across paragraphs, a selection, a document edited while counting, the idle units), the settings round trip; G-HOST, G-W32, G-GTK.
- **Evidence:** `Status bar/WP-BAR-8/` `win32-wine` and `gtk-xvfb` with a selection.
- **Decisions:** CORE-208.

### WP-BAR-9 — Menu help on the DOS screens

- **Finish line:** both DOS screens show `command_hint` in the lead while a menu item is highlighted, and the graphics screen while the pointer rests on a toolbar button, a box or a zoom part (DOS-103), under CORE-205.
- **Owns:** `crates/quill-app/src/paint.rs` (the lead's hint on the text screen; ED-lane slot), `crates/quill-paint/src/tip.rs`, `crates/quill-paint/src/shell.rs` (the hint), `dos/quill-dos/src/main.rs` (nothing if it can be helped), `tests/golden/screens/m10-menus.frames.txt` and the gfx menu goldens.
- **Must not touch:** the Windows and Linux shells.
- **Depends on:** WP-BAR-3, WP-BAR-4; the owner's answer to question 8.
- **Budget:** DOS +2.0 to +2.6 KB **[E]**: past the budget, DOS-70's raise rule with the floppy's free bytes.
- **Tests:** the menu goldens with the hint in the lead; G-DOS, G-DISK.
- **Evidence:** `Status bar/WP-BAR-9/` `after-` `dos-text-25`, `dos-text-50`, `dos-gfx-12h` with File > Save As highlighted.
- **Decisions:** DOS-103.

**WinUI** takes WUI-80 and WUI-81 into its W6 package (command bars, boxes, ruler, status line) in the UI track; the brief there reads this page's tables and `quill_chrome::status_bar` as WP-BAR-7 leaves them.

## Briefs

Every brief starts with [the common brief](../work-packages.md#the-common-brief) of work-packages.md, with two changes for this track: decision ids come from this page's reserved ranges, and evidence goes to `~/Documents/Liminal HQ/Quill/Status bar/<WP-ID>/`. Paste the common brief, then the package's own.

### WP-BAR-1 (full)

```text
Package WP-BAR-1: the zoom reset button and the zoom tooltips on Windows 95, Windows 3.x and the DOS graphics
screen. Read docs/plans/status-bar.md first: "Hints and tooltips", the CORE-206 row, and WP-BAR-1 under "Work
packages". Look at ~/Documents/Liminal HQ/Quill/Status bar/after-win32-reset-tip.png and after-win16-wide.png:
that is the control you are building (the rest of those bars is later packages; leave it as it is).

Branch feat/zoom-reset, worktree ~/source/worktrees/quill/zoom-reset, from origin/main. WP-BAR-1G (GTK) may be
open at the same time; it touches nothing under crates/, win9x/ or win16/.

1. crates/quill-chrome/src/zoom_field.rs:
   - ZoomMetrics gains `reset: i32` (0 means no reset part); `width()` adds it.
   - ZoomPart gains `Reset`. ZoomLayout gains `reset: Option<(i32, i32)>`, laid out right of plus, between plus
     and the margin (owner, 2026-11-27); `left()` stays minus's left edge; the margin is still the last
     ZOOM_MARGIN (or grip) pixels; `hit` answers Reset. Keep the order of the parts in ZoomLayout::new in one
     place.
   - `pub fn reset_live(zoom: u16) -> bool` (false when the setting is 0, Page Width).
   - `pub fn zoom_tip(part: ZoomPart) -> &'static str` and `pub fn zoom_hint(part: ZoomPart) -> &'static str`
     with exactly the plan's table: "Page Width (Ctrl+0)", "Zoom Out", "Zoom (click for the list)", "Zoom In";
     and `pub const RESET_TIP_DOS: &str = "Page Width (Alt+0)"` for the DOS graphics screen;
     "Fit the page to the width of the window", "Show the page smaller", "Choose how large the page is shown",
     "Show the page larger".
   - `pub const fn field_for(text_w: i32) -> i32` = text_w + 12 (the box from its widest text, 6 px a side).
   - Unit tests: the existing ones keep passing with reset 0; new ones for a layout with reset (every edge of
     every part, the margin still constant), reset_live, and every tip and hint non-empty and Title Case.
2. Ctrl+0: in crates/quill-app/src/paint.rs the ZOOM table's Page Width item gets accelerator "Ctrl+0" (that
   one line only; this is an ED-lane file, say so in the PR); in crates/quill-app/src/accel.rs `0x30` with Ctrl
   is `Accel::Command(Command::Zoom(0))`. A host test drives from_vk(0x30, ctrl) and checks the command, and the
   menu test that every accelerator is unique still passes.
   DOS (DOS-105): the BIOS gives no Ctrl with a digit, so DOS uses Alt+0. INT 16h returns AL 0, AH 81h for
   Alt+0 (78h to 80h are Alt+1 to Alt+9; 82h is Alt+minus); bios.rs's alt_letter already maps 81h to b'0'. In
   lib.rs's plain-editing key match add `Key::Alt(b'0') => self.run(s, f, Command::Zoom(0))` before the
   `Key::Alt(c) if !self.native` menu-mnemonic arm (no mnemonic is a digit, and the file dialog handles its
   own Alt+1 to Alt+3, so nothing clashes: a host test proves Alt+0 neither opens a menu nor reaches the
   dialog's sort). The DOS zoom list (quill-paint drop.rs, Pop::Zoom) shows "Alt+0" right-aligned on its Page
   Width row. assets/help/quill.qhm's Keyboard shortcuts topic gains "Ctrl+0 Page Width (Alt+0 on DOS)";
   rebuild the help file the way crates/quill-help says.
3. Windows 95 (win9x/quill-win9x/src/ui.rs, tips.rs): ZOOM_METRICS gets reset 22 and the box from field_for of
   GetTextExtentPoint32A("Page Width") in DEFAULT_GUI_FONT, measured once at start. draw_zoom_control draws the
   Page Width glyph in the reset part (11 by 9: two 1-pixel bars 10 apart, a 7-pixel line between them with
   3-pixel arrow heads; embossed grey when !reset_live) with an etched line before it, as plus has.
   status_proc's WM_LBUTTONUP runs Command::Zoom(0) on Reset when reset_live. Tooltips: add four rectangle
   tools (TTM_ADDTOOLA with TTF_SUBCLASS and the part's rect in the status bar's client area, uId 900 to 903)
   to the tooltip window tips.rs already makes, re-set their rects (TTM_NEWTOOLRECTA if it is in COMCTL32
   4.00's header: it is WM_USER+6; otherwise delete and re-add) whenever status_layout runs. The hint: while
   the pointer is over a part (WM_MOUSEMOVE in status_proc), put zoom_hint in the first pane the way
   hint_text does for toolbar buttons, and clear it when the pointer leaves (TrackMouseEvent is not on
   Windows 95: clear it from the main window's WM_MOUSEMOVE as the toolbar hint does). No new import: check
   win9x/tools/check-imports.sh. Keep ZOOM_MIN_CLIENT_W for now (WP-BAR-5 removes it) but raise it by the
   reset's 22 pixels so the panes do not crowd.
4. Windows 3.x (win16/examples/quill/chrome.rs, tips.rs, main.rs): ZOOM_METRICS reset 22, the box from field_for
   of the ui font's GetTextExtent("Page Width"); paint_status draws the glyph and an etched line; the click
   handler that calls zoom_layout(...).hit runs Command::Zoom(0) on Reset. tips.rs: add the four parts as
   targets (Target::Zoom(ZoomPart)), with zoom_tip and zoom_hint as the words; the pointer test uses the same
   hit. ZOOM_MIN_CLIENT rises by 22 for now. Measure the data segment before and after (win16/build.sh) and
   put both figures in the PR; stop and ask if free space would fall below 1,024.
5. DOS graphics (crates/quill-paint/src/zoombox.rs, tip.rs, pointer.rs as needed): METRICS reset 20 (the box
   stays 76: the strike is fixed); draw the glyph in a raised button right of plus, like the others, sunk
   while held, embossed when !reset_live; a release on it runs Command::Zoom(0); tip.rs shows zoom_tip for the
   parts after the usual rest, as it does for toolbar buttons, with RESET_TIP_DOS for the Page Width button.
   The status text ends 3 px before the control's new left edge, as now.
6. Tests: quill-paint zoom and gfxmouse tests for the reset (press, release, greyed at Page Width, the text's
   right edge); extend the g11-gfx-zoom scenario with a Page Width click and, after zooming to 150 %, an `alt0`
   key (AUTOTYPE's name for Alt+0; this is the DOSBox proof of AH 81h: the frame after it must be Page
   Width, equal to the host's), and rewrite its goldens with
   QUILL_UPDATE_GOLDEN=1 (only the status line's rows may change: check with git diff); extend
   win16/tools/e2e-zoombox-w31.sh with a click on the reset after zooming to 150 %, and wine-zoom.sh the same.
7. Docs: CORE-206, DOS-105, W16-112 and the zoom part of W9X-121 into docs/decisions.md (Status Assumed, from
   the plan's table); SPEC.md's zoom control paragraph (line "The zoom control." and the Windows 95 status bar
   sentence) gains the Page Width button, its glyph, Ctrl+0 and the four tooltips; the DOS graphics zoom
   paragraph gains Alt+0 and loses "there is no shortcut, as in the Windows builds"; the View menu row shows
   Ctrl+0; a WP-BAR-1 row in docs/progress.md.
8. Gates: G-HOST, G-DOS (with bash dos/tools/size-report.sh before and after), G-W16 (with the data figure and
   e2e-zoombox-w31.sh), G-W32 (with wine-zoom.sh and wine-layout.sh at 640, 800, 1024), G-DISK, G-MD.
9. Evidence in ~/Documents/Liminal HQ/Quill/Status bar/WP-BAR-1/: before- and after- for win32-wine,
   win16-311, dos-gfx-12h, dos-gfx-vbe at 100 % and at Page Width, and after- shots with each tooltip showing
   on Windows 95 and Windows 3.x (win32-wine-tip-reset.png and so on).
10. PR title: "Add a Page Width button and name every part of the zoom control". Labels: enhancement.

Finish line: on all three builds the Page Width button is right of the plus, greyed at Page Width, runs Page
Width on a click; every part has its tooltip and hint; Ctrl+0 is Page Width on both Windows builds and in
their View menus, Alt+0 on both DOS screens (proved in DOSBox) and in the zoom list; nothing else in any
status bar moved; the DOS and Win16 figures are in the PR.

Answered by the owner (2026-11-27), not defaults: the button sits right of the plus; Alt+0 on DOS. Default if
nobody answers (record it as an Assumed decision with "OWNER DECISION:"): Ctrl+0 is Page Width on the other
builds (question 2); the glyph is the two bars and the arrow.
```

### WP-BAR-1G

```text
Package WP-BAR-1G: the GTK zoom control gets the Page Width button, Adwaita's zoom icons, the four tooltips and
Ctrl+0. Branch feat/gtk-zoom-reset, worktree ~/source/worktrees/quill/gtk-zoom-reset, from origin/main. Read
docs/plans/status-bar.md ("Hints and tooltips", "The look on each build", LNX-90 and LNX-91) and
linux/quill-gtk/src/status.rs, window.rs and view.rs. Look at after-gtk-wide.png and after-gtk-reset-tip.png in
~/Documents/Liminal HQ/Quill/Status bar/.

1. status.rs: the control becomes a gtk::Box with the "linked" class holding four flat buttons, left to right:
   a Button with zoom-out-symbolic, the MenuButton, a Button with zoom-in-symbolic, and a Button with
   zoom-fit-best-symbolic (action win.zoom-page-width, insensitive at Page Width) right of the plus. Tooltips "Page Width (Ctrl+0)",
   "Zoom Out", "Zoom (click for the list)", "Zoom In": from quill_chrome::zoom_field::zoom_tip if WP-BAR-1 has
   merged, otherwise four local constants with a comment naming the function they move to. Accessible labels
   are the tooltips without the key. StatusInfo gains can_reset.
2. window.rs: action zoom-page-width runs Command::Zoom(0) through the same routing as the list; Ctrl+0 binds to
   it, Ctrl+9 stays bound to it too; view.rs's zoom_reset goes (100 % is in the list).
3. docs/linux.md: the zoom keys (Ctrl+0 Page Width, Ctrl+9 the same) and the status bar line; LNX-91 into
   docs/decisions.md with LNX-3 marked as amended by it.
4. Tests: cargo test in linux/quill-gtk: the action exists, Ctrl+0 and Ctrl+9 map to it, can_reset follows the
   zoom, the tooltips are the plan's. Gates: G-GTK with chrome-shots.sh (light and dark, at Page Width and at
   150 %), G-MD.
5. Evidence: Status bar/WP-BAR-1G/ before- and after- gtk-xvfb light and dark.
6. PR title: "Give the GTK zoom control a Page Width button and Adwaita's zoom icons". Labels: enhancement.

Finish line: the four buttons with their icons, tooltips and accessible labels; Ctrl+0 and Ctrl+9 give Page
Width; the rest of the bar unchanged.
```

### WP-BAR-2

```text
Package WP-BAR-2: quill_chrome::status_bar, the one model and layout every status bar will draw from, and
App::backed_up. No shell changes. Branch feat/status-model, worktree ~/source/worktrees/quill/status-model,
after WP-BAR-1 merges, in an ED-lane slot (recovery.rs). Read docs/plans/status-bar.md in full: "The fields",
"The lead", "Narrowing", "Widths and spacing" and "What is shared" are your specification, and
~/Documents/Liminal HQ/Quill/Status bar/tools/make_mockups.py is a Python model of the same function
(normalized_layout): where the plan's prose and the model disagree, ask; the model drew the pictures.

1. crates/quill-chrome/src/status_bar.rs (SPDX header; no alloc, no floats, no panics, no indexing; the crate's
   clippy denies apply): Field, Form, Lead, Status (fixed arrays for the texts: lead 96 bytes, face 32), Shown,
   Metrics, Placement, Step and NARROWING, layout, field_text (Windows-1252 bytes), lead_text (path from the
   left keeping the file name, `..\` or `../`; a sentence from its end with the Metrics' ellipsis bytes),
   spoken behind a `spoken` feature, Count and Language behind `status-extras` (default features: both on;
   the DOS and Win16 manifests will turn them off in their own packages).
2. Status::of(app, s, shell): reads status_position, char_state, story_view, message, name, overtype, extend,
   modified, picture_status and the new backed_up; the shell passes what only it knows (the page total if it
   has one cheaply, the hint, the code name, the zoom and Shown).
3. crates/quill-app/src/recovery.rs: `pub fn backed_up(&self) -> bool`: true while modified and the last
   recovery write holds every change (the revision the write captured equals the document's), false with
   recovery off, autosave Off, or a lock held elsewhere. Unit tests through an As you work cycle and an Every
   N minutes cycle with [Debug] AutosaveSeconds.
4. Tests in the module and in crates/quill-host/tests/status_bar.rs: each shell's Metrics (write them as the
   plan's table says: text 1 cell; DOS graphics, Windows 3.x, Windows 95, GTK, WinUI in pixels with a fixed-
   advance fake measure the tests define) at 80 cells and 528, 600, 640, 808, 868 and 1,020 px with the
   mock-ups' scenarios (NORMAL, LONG, HINT in make_mockups.py): the spans equal the table you write in
   Status bar/WP-BAR-2/layout-table.txt; properties (proptest, 10,000 cases): spans never overlap and stay in
   the bar, the margin is constant, Position, Mode and State are never dropped while the bar is wider than
   their sum, a wider bar never shows fewer fields, the steps are taken in NARROWING's order.
5. Docs: CORE-200 to CORE-205, CORE-207 and CORE-209 into docs/decisions.md (Assumed); docs/architecture.md's
   quill-chrome line gains status_bar; a progress row. SPEC unchanged (no behaviour yet).
6. Gates: G-HOST, G-MD; size-report, the Win16 data figure and QUILL32.EXE's size unchanged.
7. PR title: "Lay out every status bar from one model in quill-chrome". Labels: enhancement.
```

### WP-BAR-3

```text
Package WP-BAR-3: the DOS text screens (25 and 50 rows, colour and mono) and the terminal client draw the
status row from quill_chrome::status_bar (DOS-100, DOS-102, LNX-93, CORE-205). Branch feat/status-text after
WP-BAR-2, in the ED-lane slot (paint.rs), never while WP-CODES-2 or the Unicode freeze is open. Read the plan's
"The lead", "Narrowing", "The look on each build" (the text row) and look at after-dos-text-25*.png,
after-dos-text-50-codes.png and after-tty*.png.

1. crates/quill-app/src/paint.rs: the status part of draw_rows calls Status::of and layout with the text
   Metrics (1-cell edge, 3-cell gap with U+2502 / CP437 0xB3 in its middle, joined gap 1 between Mode and
   State, reserve_flags, short forms, path names, lead_min 12) and puts the cells; off flags in attr 0x78 on the
   colour scheme and blank in mono. status_into and Row's status use go; status_text keeps its signature and
   is built from field_text for the Windows 3.x shell until WP-BAR-6. The DOS and quill-tty manifests turn off
   quill-chrome's status-extras and spoken features.
2. A message keeps Mode and State (CORE-205): the old "message replaces the row" goes.
   Required by the SPEC (DOS-104, owner 2026-11-27): the Font field ends with " Sup" or " Sub" when the
   character before the caret is raised or lowered (CharState's vpos 1 or 2) and with the colour's name when
   it is not Automatic: the name of the nearest of quill_chrome::view::COLOURS by squared RGB distance, in
   colour and in mono. With the text Metrics' font_keeps, the narrowing step shortens the face a character at
   a time to 8 (cut with a "."), and the field never drops; the size, Sup or Sub and the name never shorten.
   Look at after-dos-text-25-format.png and after-dos-text-25-format-long.png.
3. Goldens: QUILL_UPDATE_GOLDEN=1 cargo nextest run --workspace; then check that only status rows changed:
   for f in $(git diff --name-only tests/golden/screens); do git diff -U0 "$f" | grep '^[-+][^-+]' ; done
   must show only each frame's last row and its attribute row. New scenario tests/scenarios/m19-status.keys:
   open a document by a long path, Mod, Insert for OVR, F8 for EXT, the header view, a message (F3 with
   nothing found), at 25, 50 and /MONO. And tests/scenarios/m19-status-format.keys: superscript, subscript, a
   red word, a colour from an RTF fixture that is not one of the sixteen, Automatic, then the long case in
   subscript fuchsia, at 25, 50 and /MONO; check each frame's last row by eye against the plan's mock-ups.
4. SPEC: the DOS screen paragraph (the last row) and the status bar sentence describe the new row, with Sup,
   Sub and the colour's name as the attribute and feature tables already say; decisions DOS-100, DOS-102,
   DOS-104, LNX-93; a progress row.
5. Gates: G-HOST, G-DOS (size-report before and after), G-DISK, G-MD; quill-tty's tests.
6. Evidence: Status bar/WP-BAR-3/ before- and after- dos-text-25, dos-text-50, dos-text-25-mono, the long case,
   tty.
7. PR title: "Anchor the text screens' status fields to the right and keep every flag". Labels: enhancement.
```

### WP-BAR-4

```text
Package WP-BAR-4: the DOS graphics status bar from quill_chrome::status_bar, one sunken box a field (DOS-101).
Branch feat/status-gfx after WP-BAR-3. Read the plan's "The look on each build" and after-dos-gfx*.png.

1. crates/quill-paint/src/shell.rs status_line: layout with the DOS graphics Metrics (20 px bar, 4 px edge, 3 px
   gap, 4 px inside, lead_min 72, short forms, path names, reserve_flags; the measure is the strike's width);
   a 1-pixel sunken box a field (the bevel helper), the flags embossed grey when off, the black line along the
   top kept. zoombox.rs takes its rectangle from Placement's zoom, so the narrowing steps are the same code as
   everywhere (at 640 px nothing narrows).
2. Goldens: rewrite tests/golden/gfx with QUILL_UPDATE_GOLDEN=1 and check that only the rows of the status bar
   changed (compare the frames above y = 460 with win16/tools/compare_frames.py or a numpy diff; write the
   result to the evidence folder's compare.txt).
3. SPEC: the graphics screen's status bar; DOS-101 into decisions; a progress row.
4. Gates: G-HOST, G-DOS, G-DISK, G-MD.
5. Evidence: Status bar/WP-BAR-4/ before- and after- dos-gfx-12h, dos-gfx-vbe, the long case.
6. PR title: "Draw the DOS graphics status bar as one box a field". Labels: enhancement.
```

### WP-BAR-5

```text
Package WP-BAR-5: the Windows 95 status bar from quill_chrome::status_bar (W9X-120 to W9X-122). Branch
feat/status-win32 after WP-BAR-2. Read the plan's tables and after-win32-*.png.

1. ui.rs status_layout: measure each field's text with GetTextExtentPoint32A in DEFAULT_GUI_FONT, call layout
   with the Windows 95 Metrics (long forms allowed, file-name lead, reserve_flags, 2 px gap, 4 px inside, zoom
   margin 16 for the grip) and SB_SETPARTS from the spans: one part a visible field plus the zoom pane last.
   Re-run it on WM_SIZE, on a hint or message change that changes CORE-205's takeover, and when View >
   Formatting Bar changes (the font pane). ZOOM_MIN_CLIENT_W goes.
2. status_update: pane texts from field_text in the forms the layout chose; Mode as one owner-drawn pane that
   draws OVR and EXT in their places, embossed COLOR_GRAYTEXT when off (or a plain pane with the two words and
   spaces, if owner-draw for it costs more than 300 bytes: say which in the PR); set each text only when it
   changes, as now.
3. Tooltips (tips.rs): rectangle tools for State, Language and the short Position, beside WP-BAR-1's four,
   moved on every layout; their texts from the plan's table.
4. Print preview keeps its two texts in the lead and Position.
5. SPEC: the Windows 95 status bar sentence; W9X-120 to W9X-122; a progress row.
6. Gates: G-W32 with wine-layout.sh at 528, 640, 800, 868 and 1024, wine-zoom.sh, wine-e2e.sh, check-imports.sh
   (no new import), G-MD.
7. Evidence: Status bar/WP-BAR-5/ before- and after- win32-wine at each width, with a hint, a message, the long
   case and the preview.
8. PR title: "Lay out the Windows 95 status bar's panes from the shared model". Labels: enhancement.
```

### WP-BAR-6

```text
Package WP-BAR-6: the Windows 3.x status bar from quill_chrome::status_bar, one sunken box a field (W16-110,
W16-111). Branch feat/status-win16 after WP-BAR-2. Read the plan's tables and after-win16-*.png.

1. chrome.rs paint_status: layout with the Windows 3.x Metrics (short forms, file-name lead, reserve_flags,
   2 px edge, 3 px gap, 4 px inside, lead_min 80, the box from field_for); a 1-pixel sunken box a field on the
   face (the white line along the top kept), the flags embossed grey when off; ZOOM_MIN_CLIENT goes.
2. main.rs: the hint from tips::hint() is the lead's Hint; the message the lead's Message. preview.rs's status
   line uses the same boxes (lead and Position).
3. win16's quill-chrome dependency turns off status-extras and spoken. Measure the data segment before and
   after; stop below 1,024 free. If WP-ARCH-9 has merged, the story tags and the zoom tips and hints go to the
   string table instead.
4. If WP-BAR-3 and WP-BAR-4 have merged: delete quill_app::status_text and its host test (the last caller is
   gone); otherwise leave it and say so.
5. SPEC: the Windows 3.x status bar; W16-110, W16-111; a progress row.
6. Gates: G-W16 with e2e-zoombox-w31.sh, e2e-scroll-w31.sh, e2e-preview-w31.sh, the segments and stack checks;
   G-MD.
7. Evidence: Status bar/WP-BAR-6/ before- and after- win16-311 at 640, 808 and 1024, the long case, the preview.
8. PR title: "Draw the Windows 3.x status bar as one box a field". Labels: enhancement.
```

### WP-BAR-7

```text
Package WP-BAR-7: the GTK status bar from quill_chrome::status_bar (LNX-90, LNX-92). Branch feat/status-gtk after
WP-BAR-2 and WP-BAR-1G. Read the plan's tables, "Accessibility" and after-gtk-*.png.

1. status.rs: one label a field (caption; dim-label for Position, Count, Font and Language;
   caption-heading for Mode and State, no accent), the lead label first; texts from field_text in the forms
   the layout chose, converted from Windows-1252 as today.
2. A gtk::LayoutManager subclass on the bar: in allocate, measure each child's natural width, call layout with
   the GTK Metrics (long forms, no name, 12 px edge, 18 px gap, lead_min 60, zoom 34/34/natural, margin 8) and
   set child visibility and the forms; measure() reports the least width as the lead's least plus Position,
   Mode, State and the zoom box, so the window can narrow.
3. Accessibility: the bar's accessible role is Status; each label's accessible label is spoken() (enable
   quill-chrome's spoken feature); a new message calls gtk_accessible_announce (polite).
4. Count and Language show when WP-BAR-8 has merged (Shown from the settings); until then Shown leaves them off.
5. docs/linux.md: the status bar paragraph; LNX-90, LNX-92 into decisions; a progress row.
6. Gates: G-GTK with chrome-shots.sh light and dark at 600 and 1020 px; an AT-SPI dump (accerciser or a
   python3-gi script under your own Xvfb and dbus) of the bar's role and labels; G-MD.
7. Evidence: Status bar/WP-BAR-7/ before- and after- gtk-xvfb at both widths, light and dark.
8. PR title: "Lay out the GTK status bar from the shared model and let the window narrow". Labels: enhancement.
```

### WP-BAR-8

```text
Package WP-BAR-8: the word count and the optional fields' settings (CORE-208). Branch feat/status-count after
WP-BAR-3, in the ED-lane slot (idle.rs, lib.rs). Read the plan's "The fields", "View > Status Bar and the
Options window" and SPEC's idle-time paragraph (CORE-16, CORE-17).

1. idle.rs: a word count job in the order slot CORE-17 reserves ("word count"): counts words of the body a
   paragraph a unit, restarting at the first changed paragraph after an edit; App::word_count() -> Option<u32>
   (None until done) and App::selection_words() (counted directly, bounded by the selection). Behind a cargo
   feature the DOS and Win16 builds leave off.
2. Settings: [Quill] StatusCount, StatusFont, StatusLanguage (0 or 1) with the plan's per-build defaults, in
   each_setting and apply_setting, or as schema rows if WP-PREF-2 has merged; Shown is built from them and the
   formatting bar's state in quill-chrome. If the Options window's View page exists, add the three rows.
3. Tests: the count over the sample documents (a fixed table you check by hand), after edits, with a selection;
   the settings round trip; the job's units stay within the idle budget.
4. SPEC: the status bar's optional fields and the three keys in the settings table; CORE-208; a progress row.
5. Gates: G-HOST, G-W32 (Count and Language show), G-GTK if WP-BAR-7 has merged; size-report and the Win16 data
   figure unchanged.
6. PR title: "Count words in the background and let each status field be turned off". Labels: enhancement.
```

### WP-BAR-9

```text
Package WP-BAR-9: menu help on the DOS screens (DOS-103). Branch feat/status-dos-hints after WP-BAR-3 and
WP-BAR-4 and the owner's answer to question 8 (default: build it, raising the budget by DOS-70's rule). Read the
plan's "The lead" (CORE-205) and after-dos-text-25-hint.png, after-dos-gfx-hint.png.

1. paint.rs: while a menu item is highlighted the lead's Hint is command_hint of its command (the recent files
   use Command::Recent's hint); quill-paint: the same on the graphics screen, and tip.rs passes the hint of
   the toolbar button, box or zoom part the pointer rests on.
2. Measure: size-report before and after; if past 442,368, raise the budget by the measured cost rounded up
   to 4 KB in a DOS row of your own with the floppy's free bytes from installer/tools/verify-disks.sh; stop and
   ask past 480 KB or under 128 KB free on the floppy.
3. Goldens: the menu frames (m10-menus and the gfx menu goldens) gain the hint; check that only the status
   rows changed.
4. SPEC: the status bar's menu help sentence covers DOS; DOS-103; a progress row.
5. Gates: G-DOS, G-DISK, G-MD.
6. PR title: "Show menu help on the DOS status line". Labels: enhancement.
```

## Questions for the owner

Each has a default the packages take if nobody answers, recorded as an Assumed decision with "OWNER DECISION:".

| # | Question | Default taken | Where to swap |
| --- | --- | --- | --- |
| 1 | Where does the Page Width button go? | **Answered 2026-11-27: right of the plus.** It narrows first, as the outermost part | `ZoomLayout::new` in `zoom_field.rs`, one ordering (WP-BAR-1) |
| 2 | Ctrl+0 for Page Width on every build but DOS, which moves GTK's Ctrl+0 off 100 %? | Yes; GTK keeps Ctrl+9 as a second key for Page Width, and 100 % is in the list. **DOS answered 2026-11-27 (route left to the plan): Alt+0** (DOS-105) | The `ZOOM` table in `paint.rs`, `accel.rs`'s `0x30`, GTK's `window.rs` (WP-BAR-1, WP-BAR-1G) |
| 3 | Which document name: the path on DOS and the terminal, the file name on Windows, nothing on GTK and WinUI? | Yes, by "the status bar never repeats the title" | `Metrics::names` per shell |
| 4 | Drop the font and size from the bar while the formatting bar shows them? | Yes | `Shown::font` in `status_bar.rs` (WP-BAR-8) |
| 5 | Word count and spelling language: on by default on Windows 95, GTK and WinUI, left out of DOS and Windows 3.x? | Yes; a character count is not offered | `[Quill] StatusCount` and `StatusLanguage` defaults; the `status-extras` feature in each manifest |
| 6 | Is `Modified · backed up` the right words for "not saved, but a recovery copy has everything"? | Yes (others: `Modified (recoverable)`, or nothing visible and the tooltip only) | `field_text` for State (WP-BAR-2) |
| 7 | On the retro builds, show OVR and EXT greyed when off (Word 6 style) so nothing moves? | Yes; the modern builds hide them | `Metrics::reserve_flags` per shell |
| 8 | Menu help on DOS costs about 2.0 to 2.6 KB of the 4.6 KB left: build it and raise the budget by DOS-70's rule? | Yes, last (WP-BAR-9) | Not starting WP-BAR-9 |
| 9 | Windows 3.x: one sunken box a field (as Windows 95 and Word 6 do) instead of today's single flat panel? | Yes | `paint_status` in `chrome.rs` (WP-BAR-6) |
| 10 | Order of the flags: Mode then State (`OVR EXT Mod`, Mod at the edge as on DOS today), or State then Mode as your list had it? | Mode then State | `FIELD_ORDER` in `status_bar.rs`, one array |

## Found while planning

- **The SPEC promises DOS text-screen status words that are not drawn.** The text screen's attribute table says superscript and subscript are "shown in the status line" on a mono screen and the text colour is "named in the status line" on a colour one, and the feature table says "Status line only" for the colour; `status_into` shows neither. The owner's rule is that the SPEC wins: WP-BAR-3 draws them (DOS-104), in mono and colour alike.
- **The SPEC says the status bar shows menu help "while a menu is open"** in a paragraph that covers every build; the DOS screens have none until WP-BAR-9.
- **The DOS row's line number is 0 when the caret has no line** (`status_into` uses `line_in_page`'s fallback 0) where `status_position` says 1. WP-BAR-2's `Status::of` uses `status_position` everywhere, which fixes it.
- **DOS has no Ctrl with a digit.** `bios.rs` maps no Ctrl+digit and INT 16h reports none, so the spacing keys Ctrl+1, Ctrl+2 and Ctrl+5 in the menu tree do nothing on the DOS screens. Page Width takes Alt+0 there (DOS-105); the spacing keys could take Alt+1, Alt+2 and Alt+5 the same way, outside this plan (the file dialog's Alt+1 to Alt+3 are its own and do not clash, since the dialog takes its keys before the editor). The SPEC should say which keys DOS lacks (WP-BAR-1 writes the sentence).
- **INT 16h's Alt+digit codes.** Alt+1 to Alt+9 are AH 78h to 80h and Alt+0 is 81h, with AL 0; 82h and 83h are Alt+minus and Alt+equals, so "78h to 82h for Alt+1 to Alt+0" is one too many. `bios.rs` already decodes 78h to 81h; DOSBox's agreement is shown for 78h to 7Ah by the `m16` file-dialog scenarios, and WP-BAR-1's `alt0` shows it for 81h.
- **The terminal client is the DOS painter.** `quill-tty` draws through `quill_app::paint::draw`, so it changes with WP-BAR-3 and needs no package of its own.
- **GTK's LNX-3 is amended**, not replaced: the zoom levels and the setting stay; only Ctrl+0's meaning moves (LNX-91).
- **Reveal Codes plan.** W9X-95 puts the code's long name "in the status bar's first pane"; with this plan that pane is the lead and the name sits at precedence 3, below a hint and a message. WP-CODES-5 and WP-CODES-6 should read CORE-202.
