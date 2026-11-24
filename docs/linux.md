# Quill on Linux

What exists for Linux, what is planned, and why. The behaviour of each client is in [SPEC.md](../SPEC.md); the decisions made here are numbered D100 to D119 below and move into [decisions.md](decisions.md) when the open branches that also append to it have merged.

## Table of contents

- [Where things stand](#where-things-stand)
- [The terminal client](#the-terminal-client)
- [The command line](#the-command-line)
- [Fonts and measuring](#fonts-and-measuring)
- [The GTK4 client](#the-gtk4-client)
- [Desktop integration and packaging](#desktop-integration-and-packaging)
- [Wine as a stopgap](#wine-as-a-stopgap)
- [Work list](#work-list)
- [Modern conveniences](#modern-conveniences)
- [Decisions](#decisions)
- [Risks and what is not proven](#risks-and-what-is-not-proven)

## Where things stand

| Piece | Crate | State |
|---|---|---|
| Terminal client | `linux/quill-tty` | Built. Edits, saves and prints through the same `App` as DOS |
| Command line (`quill cat`, `convert`, `info`) | `crates/quill-cli` | Built |
| Outline-font measure and test fonts | `crates/quill-fonts` | Built. Nothing draws with it yet |
| Page renderer, thumbnailer, PDF | `crates/quill-raster`, `quill-thumbnailer` | Planned |
| GTK4 client | `linux/quill-gtk` | Planned; the toolkit is chosen (D101) |

The editor core needs nothing Linux-specific. Every client links the same `quill-core`, `quill-formats` and `quill-app`, so a `.QLD` or RTF file written on Linux is byte for byte what the other builds write.

What the open branches change, and what that means here:

- **There is no `Gui` or `Surface` trait.** The page painter that draws with real fonts belongs to the Windows 95 build (`gdi.rs`). `Surface` is still a plan (D31, D79), and the GTK client is the second user that D79 waits for.
- **Shared view geometry assumes 96 dpi** (`view.rs`, D95). The GTK client lays out at 1440 units to the inch, so that module takes its resolution from `Measure::resolution()` first.
- **There is no system clipboard.** `App` keeps its own 2 KB text clipboard. The GTK client needs `copy_rtf`, `copy_text`, `paste_rtf` and `paste_text` in `quill-app` (work item C1).

## The terminal client

`quill-tty [--mono] [--no-kitty] [FILE]` draws the DOS text screen in a terminal. The screen is `quill-grid`'s `Grid`: 80 columns by 25 rows, or 50 on a terminal 50 rows or more tall, centred across a wider window. Each CP437 cell is sent as its Unicode character with the VGA attribute as colours, so a screen here equals the DOS and host screens cell for cell (D102).

- **Colour.** The sixteen terminal colours by default, so the terminal's theme shows through. When `COLORTERM` is `truecolor` or `24bit` the VGA palette is sent as 24-bit colour. The monochrome scheme (`--mono`) draws the MDA underline attribute as an underline.
- **Redraw.** Only the cells that changed since the last frame are sent. A resize redraws everything. A terminal smaller than 80 by 25 shows a message instead.
- **Cursor.** A blinking underline in insert mode and a block in overtype, as on DOS, and hidden while a menu is open.
- **Paste** (bracketed paste) types the text: line ends are Enter, tabs are Tab, and characters Windows-1252 lacks are skipped.

### Keys

The mapper turns a terminal key event into a `Key`. Printable characters are converted from Unicode to Windows-1252, and anything with Alt or Super is ignored. F10 opens the menus. Some terminals keep F10 for their own menu (GNOME Terminal does until the shortcut is turned off in its preferences); Esc and the arrow keys work the same once a menu is open.

| Terminal sends | Editor key |
|---|---|
| Ctrl with a letter | `Ctrl(letter)`, so Ctrl+S saves and Ctrl+B is Bold |
| Shift with an arrow, Home, End, Page Up or Page Down | `Select`, which extends the selection |
| Ctrl+Home, Ctrl+End | `CtrlHome`, `CtrlEnd` |
| Ctrl with an arrow | Nothing (no such key in the editor) |
| F1 to F12, Insert, Delete, Esc, Tab, Enter, Backspace | The same keys |

**Ctrl+I is the Tab byte (0x09) in a legacy terminal**, and Ctrl+M is the Enter byte (0x0D). A terminal that speaks the kitty keyboard protocol reports each key as pressed. `quill-tty` asks the terminal whether it does and, if so, requests disambiguated keys (flag 1, the least that separates Ctrl+I from Tab). Then Ctrl+I is Italic. In a legacy terminal Tab stays Tab and **Italic is on the Character menu** (F10, Character, Italic). `--no-kitty` turns the request off. Ctrl+H (Replace) and Ctrl+J (Justify) are not affected, because a raw-mode terminal reports bytes 0x08 and 0x0A as those chords and sends 0x7F for Backspace; a terminal that sends 0x08 for Backspace will open Replace instead, and the kitty protocol fixes it.

### Files

The editor names files by Windows-1252 bytes. `LinuxFiles` turns a name into UTF-8 and resolves it (D103):

- `QUILL.INI` and `QUILL.UDC` live in `$XDG_CONFIG_HOME/quill` (`~/.config/quill`), in the same formats as on Windows.
- A bare dictionary name (`EN_CA.DIC`) is searched for in the current folder, beside the program, in `$XDG_DATA_HOME/quill` and in `quill` under each folder of `$XDG_DATA_DIRS`.
- Any other name is a path relative to the current folder. A name that does not exist is retried without regard to case, so a DOS-style `LETTER.RTF` finds `letter.rtf`, and a save goes over the file that is there.
- A command-line name must be representable in Windows-1252; otherwise `quill-tty` says so and exits.
- **Safe save.** `create` writes `.name.~tmp` next to the target; `close` flushes, syncs and renames it over the target and syncs the folder. A failed or abandoned save removes the temporary file and leaves the old one whole. A save through a symbolic link replaces the file it points to.

### What is not there yet

- **Mouse.** `quill-app` has no pointer entry on `main`; the pointer work is on an open branch.
- **System clipboard.** Copy and paste stay inside Quill, as on DOS. Pasting from the terminal works through bracketed paste.
- **Alt+letter menus.** The editor takes F10 only.
- **UTF-8 text files.** Plain text is read as Windows-1252 until the text option in `quill-formats` lands.

## The command line

`quill` (crate `quill-cli`) works on documents without a screen:

- `quill cat [--ansi] FILE` writes the text, one line to a paragraph, as UTF-8. Bullets and numbers are written before their paragraphs, a picture is `[Picture W x H]`, and a field is its name in brackets. `--ansi` shows bold, italic, underline and strikethrough as escape sequences, reset at the end of each line.
- `quill convert [--to qld|rtf|txt] IN OUT` reads by content and writes by the extension of OUT (`.rtf`, `.txt`, otherwise `.qld`). It writes the same bytes as `quill-convert`, which stays in `quill-host` for the LibreOffice check script.
- `quill info FILE...` prints the format, size, character, word and paragraph counts, pictures, fonts, paper, margins, headers and the pages of the DOS text layout.

A file of `-` is standard input and an OUT of `-` is standard output. Failures use the editor's words (`... is not a document Quill can read`) and exit 1; a usage error exits 2 (D104).

The name `quill` is the command-line tool for now. The GTK client wants the same name (D105).

## Fonts and measuring

The Windows 95 failure was widths measured through a scaled mapping mode that no longer agreed with the glyphs drawn. The Linux answer is one engine for both: the widths and the outlines come from the same font file, with no hinting, and the same mapping places the glyphs, the caret and the hit tests.

`quill-fonts` provides `OutlineMeasure`, a `quill_core::layout::Measure` over font files read with `skrifa` (D106):

- **Resolution.** 1440 units to the inch, so a layout unit is a twip. Screen, PDF and print then break lines in the same places. Another resolution can be asked for; 96 is used for the Windows comparison.
- **Advances.** The `hmtx` advance in font units, times the size in half points and the resolution, divided by 144 times the units per em, rounded once. A 144-point glyph still fits in `u16`.
- **Vertical metrics.** Ascent and descent are `usWinAscent` and `usWinDescent`, which GDI takes for `tmAscent` and `tmDescent`. External leading is the `hhea` line (ascender, descender and gap) beyond that extent.
- **Never hinted.** An advance depends on the font and the size in points, never on a pixel size, a zoom or a screen's scale. The tests check that skrifa's unhinted metrics are the `hmtx` advances scaled, at eleven pixel sizes.
- **Missing styles.** A face without a bold of its own is widened by 1/64 em in the advance, and the drawn outline gets the same emboldening; a missing italic keeps its advances and is slanted when drawn (D107).
- **Fallback.** The face asked for; then Times New Roman, Liberation Serif and the Quill test serif for a serif face (Arial, Liberation Sans and the test sans for a sans face; Courier New, Liberation Mono and the test mono for a monospaced one). A name the tables do not know uses the font table's family code. Faces are found by reading the usual font folders directly (`/usr/share/fonts`, `~/.fonts`, the XDG data folders); fontconfig is not needed, which keeps layout the same on every machine.

### The test fonts and their licence

The crate carries a Windows-1252 subset of Liberation 2.1.5 (Serif, Sans and Mono in four styles, about 215 KB, unhinted) so that goldens are the same everywhere. Liberation is licensed under the SIL Open Font License 1.1, which allows bundling, modifying and redistributing the fonts with software as long as they are not sold by themselves and the licence and copyright notices go with them.

**The licence does reserve font names.** The licence file that ships with `ttf-liberation` 2.1.5 says "with Reserved Font Arimo, Tinos and Cousine" for Google's digitized data and "with Reserved Font Name Liberation" for Red Hat. A subset is a Modified Version under the OFL, and a Modified Version may not use a Reserved Font Name. So the subsets are renamed Quill Test Serif, Quill Test Sans and Quill Test Mono, and the trademark and description entries of the name table, which name the reserved fonts, are dropped. The copyright, licence, designer and version entries stay, the licence text is in `crates/quill-fonts/fonts/LICENSE-LIBERATION.txt`, and the recipe and source checksums are beside the fonts. This is a reading of the licence, not legal advice.

Times New Roman, Arial and Courier New are never committed. They are used when installed, and nothing in a test depends on them.

### Comparison with Windows ME

`tests/golden/layout/letter.outline96-vs-windows-me.report.txt` compares the measure, at 96 dpi, with the widths and vertical metrics `QUILL32.EXE /CAPTURE` read on Windows ME, and lists the lines of the letter that start in a different place. It is a report, not a pass mark: GDI hints advances to whole pixels, and the outline measure does not. Today, 5,399 of 26,880 glyph widths differ by at least a pixel (most of them in Times New Roman and Arial; Courier New matches exactly from 9 point up), and the letter's 14 lines break the same way except in two paragraphs. On this machine the installed Times New Roman gives the same report as the Liberation subset, so the Liberation advances equal Times New Roman's for the 224 characters measured. Ascent is usually a pixel more on Windows at small sizes (11 against 10 at 8 point), which suggests GDI rounds the cell up; that is a guess the report does not settle.

## The GTK4 client

The client is GTK4 with libadwaita, in its own workspace under `linux/quill-gtk` so root CI never needs GTK headers (D101). The reasons, from what was checked on an Arch machine with GNOME on Wayland at a scale of 1.5:

- **GTK3's Rust bindings are unmaintained**, and GTK3 draws at the next whole scale and lets the compositor shrink it. GTK4's bindings (`gtk4` 0.11, `v4_24` feature) are current, and `libgtk-4.so` carries `wp_fractional_scale_v1`.
- **Qt via `cxx-qt`** has the best fractional scaling and accessibility, but needs a C++ build, is foreign on GNOME, and `QPainter`'s shaping would be bypassed anyway. **iced and egui** are pure Rust but lack native menus and portal dialogs, and egui has a weaker IME; egui is the fallback if GTK4 fails its first spike. **Tauri with a canvas** pushes pixels or text through WebKitGTK every frame.

### Architecture

| Crate | Holds |
|---|---|
| `crates/quill-fonts` | Built. Font loading, `OutlineMeasure`, the fallback table. Later: glyph rasterizing (skrifa outlines into tiny-skia paths, cached by font, size and quarter-pixel offset) |
| `crates/quill-raster` | `Mapping` (layout units to device pixels) and `render_page(app, store, mapping, rect) -> Pixmap`, following `gdi.rs::draw_line`: runs, bold, italic, underline, strikethrough, super and subscript, colours, tab leaders, `line_label`, selection, caret, sheets, pictures, headers and footers in preview. It becomes the shared `Surface` painter |
| `crates/quill-cli` | Built, plus the `quill-thumbnailer` binary (no GTK, so it starts fast in the thumbnailer sandbox) |
| `linux/quill-gtk` | gtk4-rs and libadwaita: window, the `QuillPage` widget, menus, dialogs, IME, clipboard, printing |
| `linux/data` | `.desktop`, `.thumbnailer`, AppStream metainfo, icon install map, Flatpak manifest, PKGBUILD |

**Mapping.** `Mapping { zoom_pct, scale_120 }` turns layout units into device pixels as x × zoom × 96 × scale_120 ÷ (1440 × 100 × 120), in `i64`. Wayland reports scales in 120ths, so GTK's `f64` scale becomes an integer in the shell; the float ban covers only the portable crates. Glyphs, the caret, the selection, hit tests and the ruler all go through `Mapping`, and its reverse feeds `pos_at_point`.

**Drawing the page.** `QuillPage` (a GTK4 widget subclass) takes its device width as logical width times scale. `quill-raster` renders the visible strip in 512-pixel tiles cached by layout revision, and typing re-renders only the lines that changed. The result is a `gdk::MemoryTexture` appended to the snapshot at logical bounds. If spike G0 shows GTK resampling the texture, the fallback is cairo with a FreeType face from the same file, drawing glyphs at `Mapping` positions with measurement still in skrifa. **The page never uses Pango layout;** Pango is for GTK's own chrome.

**A Windows check mode.** `[Quill] Metrics=windows` would lay out with `CapturedMeasure` and place glyphs at its positions: slightly uneven, but the same line breaks as Windows ME.

### The shell

`QuillShell` is the GTK equivalent of the Win32 `Shell`:

- **Files** over `std::fs` with the same safe save as the terminal client. The app sees the base name in Windows-1252; the shell keeps the real `PathBuf`. Recent files go in `[RecentLinux]` of `$XDG_CONFIG_HOME/quill/QUILL.INI` and in GTK's recent-files list.
- **Keys** go through the IME filter first, then `quill_app::accel`. An IME commit is UTF-8 converted to Windows-1252, and a character with no such form is refused with a status message (D108). The candidate window is placed from the caret rectangle given by `Mapping`.
- **Mouse** uses `pointer.rs`.
- **Menus** are a `GMenuModel` from `menu_items` and `menu_title`, one action per `Command`, refreshed after each command. They are popovers, not a classic menu bar.
- **Dialogs:** `set_native_dialogs(true)` then `take_dialog()`. Open and Save As use `GtkFileDialog`, the save prompt `AdwAlertDialog`, About `AdwAboutDialog`, and Find, Replace, Fonts, Tabs and Spelling a generic builder from `assets/dialogs/dialogs.json` (D86).
- **Clipboard:** `GdkClipboard` offering `text/rtf`, `application/rtf`, `text/plain;charset=utf-8` and `image/png` for a lone picture.
- **Printing** writes a PDF from `quill-raster`'s page and hands it to the print portal (D109).
- **Text files** on Linux are usually UTF-8, so `quill-formats` gains a UTF-8 option: read UTF-8 when valid, and offer "Text Files (UTF-8)" on Save As, as DOS does with CP437 (D110).

### Spike G0

The first GTK work is a spike that answers what is not known: whether GTK4 shows a texture of exactly device size one to one at a scale of 1.5 without resampling; whether a cairo drawing area is sharp at fractional scales; whether IBus compose and preedit work in a custom widget; and whether `GtkFileDialog` uses the portal outside Flatpak. The result is written back into this file.

## Desktop integration and packaging

- **Desktop entry:** `Type=Application`, `Name=Quill`, `GenericName=Word Processor`, `Exec=quill %F`, `MimeType=application/x-quill-document;application/rtf;text/rtf;text/plain;`, `Categories=Office;WordProcessor;`.
- **Thumbnailer:** `/usr/share/thumbnailers/quill.thumbnailer` with `Exec=quill-thumbnailer -s %s %i %o` and `MimeType=application/x-quill-document;` only, so LibreOffice keeps RTF thumbnails (D111).
- **MIME:** install `assets/mime/quill.xml` from the magic branch as is, plus `<generic-icon name="x-office-document"/>`. `quill.magic` stays a documented per-user `~/.magic`, since a package should not edit `/etc/magic`.
- **App id:** `io.github.liminal_hq_labs.Quill` unless Liminal HQ owns a domain for it (D112).
- **Packaging:** the AUR package `quill-git` (depends on `gtk4` and `libadwaita`, optionally `ttf-liberation`) and a Flatpak on `org.gnome.Platform//51` with `--socket=wayland --socket=fallback-x11 --share=ipc --device=dri` and no network or filesystem access, so files come through the portals, which matches the SPEC's never-opens-a-socket rule. No AppImage: bundling GTK4 is fragile (D113).
- **Releases.** There is no release workflow yet; one that builds a tarball and a Flatpak bundle on a tag needs a CI image with GTK4 development files.

## Wine as a stopgap

The Windows 95 build runs under Wine, but Quill hard-codes 96 dpi, so it cannot be made crisp at a fractional scale. Leaving the DPI at 96 and letting the compositor scale the window gives the right size and a blurry page; that is the recommended stopgap. Raising the DPI to 144 in `winecfg` with the Wayland driver makes menus and dialogs grow while the page and toolbar stay at 96 dpi, so pair it with View > Zoom 150 %. Neither was verified on a running prefix.

## Work list

Items marked *new files* touch only new files and one line of the root `Cargo.toml`, so they can land beside the open branches.

| Item | Contents | Acceptance | Needs |
|---|---|---|---|
| P1 | This page; Linux rows in the SPEC's Platforms and Components tables | markdownlint passes | New files |
| P2 | `linux/quill-tty` | Frame scripts match; letter bytes match the host | New files. **Built** |
| P3 | `crates/quill-cli` | Golden stdout; `convert` equals `quill-convert` | New files. **Built** |
| P4 | `crates/quill-fonts` | Layout dump goldens; the Windows ME report; no hinting dependence | New files. **Built** |
| C4 | `quill-formats`: the Windows-1252 table made public; a UTF-8 text option | Round-trip property test; invalid UTF-8 falls back to ANSI | Nothing open touches `quill-formats` |
| Q1a | `quill-core` `text.rs`: a step serial and `clean` mark, so `is_clean()` is true after undoing back to a save | Property test of save, edit, undo, redo | Nothing open touches `quill-core` |
| P5 | `crates/quill-raster` | Pixel goldens (within 2, at most 0.1 % of pixels differ); caret and glyph property tests | `line_label` and `walk_chars` from the Windows 95 branches |
| P6 | `quill-thumbnailer` and `linux/data` | `desktop-file-validate` and `appstreamcli validate`; Nautilus shows a page-1 thumbnail | P5 and the magic branch |
| P7a | HTML writer in `quill-formats` and `quill to-html` | Golden HTML; LibreOffice opens it with the same counts | New code in `quill-formats` |
| P7b | `quill to-pdf` with `pdf-writer` and subset fonts | `pdftotext` gives the text; the page count matches | P5 |
| G0 | The GTK4 spike | A 1.5× screenshot shows a one-pixel checkerboard with no blur | P5 |
| G1 | Read-only viewer | Opens every file in `tests/docs`; layout equals the goldens | G0 |
| G2 | Editing: keys, IME, caret, mouse | The host's editing scenarios replayed through the GTK command layer save identical bytes | G1 and the pointer work |
| G3 | Menus and native dialogs | Every `Dialog` variant is shown and answered | G2 |
| G4 | Toolbar, formatting bar, ruler, status bar | The same commands as the host chrome tests | G3 and the shared view |
| C1 | `quill-app` clipboard: `copy_rtf`, `copy_text`, `paste_rtf`, `paste_text` | Round trip into LibreOffice; unblocks the Win32 clipboard the SPEC describes | After the open `quill-app` branches merge |
| G5 | GTK clipboard | Copy from Quill and paste into LibreOffice keeps bold, and back | C1 |
| C2 | `view.rs` geometry from `Measure::resolution()` instead of 96 | Win16 golden still 12 of 12; Wine screenshots unchanged | The shared view |
| G6 | Print preview; print by PDF through the portal | Preview equals the raster goldens; a PDF reaches the print dialog | P7b |
| C3 | `Surface` and one shared painter extracted from `gdi.rs`, Win16's painter and `quill-raster` | All Windows and raster goldens unchanged | The open branches and G1 |
| G7 | libadwaita style, dark mode, accessibility | Orca reads the line at the caret (manual) | G4 |
| G8 | PKGBUILD, Flatpak manifest, a `linux` CI job, release workflow | `makepkg` and `flatpak-builder` succeed; Nautilus opens a `.QLD` | G3 |

**Milestones.** L0, readable now: P2 and P3. L1, desktop integration: P4, P5, P6. L2, viewer: G0, G1. L3, editor: G2, G3, G4, C1, G5. L4, print and finish: P7, G6, G7, G8, C3.

### Tests the later items carry

- **Property tests** at scales 1.0, 1.25, 1.5, 1.75 and 2.0 and zooms 50 to 200 %: the caret's x at every character boundary equals the next glyph's drawn x; `pos_at_point` of that position returns the same character; `Mapping` round trips stay within a pixel.
- **Pixel goldens** render `letter`, `lists`, `headers` and `pictures` with the test fonts, update with `QUILL_UPDATE_GOLDEN`.
- **The GTK client** gets a headless smoke test that opens a file, saves it and compares bytes, and screenshots for `docs/media/`. Whether a headless compositor supports fractional scale is not verified.

## Modern conveniences

Ideas that apply to every build, with what each costs on old hardware, in priority order. Each needs a SPEC change before it is built. Safe save is already specified (D66) and the terminal client has it.

1. **Safe save**: a temporary file, then replace. A prerequisite for autosave. One extra directory write; on a full floppy it needs room for two copies and fails with the existing "disk is full" message.
2. **Unsaved marker**, from Q1a's clean mark: an asterisk after the name on the DOS status line, an asterisk after the name (and a space) on the Windows title, and an asterisk before the name on Linux (D114).
3. **A "Saved" flash**: `Saved LETTER.QLD.` until the next key, and `Saving LETTER.QLD...` before a slow write. A toast on Linux.
4. **Word count** (Document > Word Count...: words, characters, paragraphs and pages, for the selection or the whole document, with the same word rules as the spelling scanner). A whole-document pass is about 1 to 2 seconds on a 386 for 1 MB, so it runs on demand only on DOS and Win16.
5. **Autosave and recovery** to a recovery folder, as a `.QLD` with an optional `RCOV` chunk naming the original path, offered at the next start and deleted on a clean save or exit. DOS and Win16 every 10 minutes when idle for 2 seconds, Win95 every 5, Linux every 2 and when the window loses focus; off on floppies (D115). A 50 KB file to a 386's hard disk takes 0.2 to 0.5 seconds and stalls, since there are no threads.
6. **Emergency save on panic** to the same slot, with the same prompt. Fixed buffer; may fail if the heap is damaged.
7. **Spelling as you type** (queued): on with a wavy line and a context menu on Win95 and Linux, off by default on Win16 (a 300 KB dictionary on a 4 MB machine).
8. **Dropping a file onto the window** opens it, with `DragAcceptFiles` on Windows and a drop target on GTK.
9. **Colours:** dark chrome by default on Linux, and an optional dark paper for display only (Automatic text follows the window text colour; explicit colours are lightened on screen and never saved).
10. **Smart quotes**, optional: off on DOS (CP437 has no curly quotes), on in the GTK client.

**Too costly on old machines:** a live word count on Win16 or DOS; autosaving to a floppy or while typing; background saving on DOS or Win16; version snapshots; spelling as you type by default on Win16; Unicode documents (D11); anything networked.

## Decisions

Numbers D100 to D119 are reserved for the Linux work, so they cannot collide with the other open branches. **Assumed** means chosen without the owner's confirmation and standing until confirmed.

| # | Decision | Choice | Why |
|---|---|---|---|
| D100 | A terminal client | **Decided.** `linux/quill-tty` draws `quill-grid`'s `Grid` with crossterm, the same screen as DOS text mode | Proves `App` and `Grid` on a third host; exact comparison with the goldens |
| D101 | Toolkit | **Decided by the owner.** GTK4 with libadwaita for the GUI, in its own workspace. egui is the fallback if the first spike fails | Maintained bindings, fractional scaling, portal dialogs, IME |
| D102 | Terminal screen | **Assumed.** 80 × 25, or 80 × 50 when the terminal is 50 rows tall; 16 ANSI colours, 24-bit when `COLORTERM` says so | Keeps the theme by default and the VGA palette on request |
| D103 | Linux file names and places | **Assumed.** Names are Windows-1252 in the editor and UTF-8 on disk; settings in `$XDG_CONFIG_HOME/quill`; dictionaries searched in the XDG data folders; case-insensitive retry; save through a temporary file | One rule for every Linux client, and DOS-style names keep working |
| D104 | The `quill` command | **Assumed.** `cat`, `convert` and `info` over the core; `convert` equals `quill-convert`; exit 1 for a failure and 2 for usage | Documents are readable without a screen |
| D105 | The name `quill` | **To decide.** The command-line tool takes it now; the plan was for the GTK client to be `quill` too, with the tool as its subcommands | One name for the user, but two binaries cannot share it |
| D106 | Measuring with outline fonts | **Assumed.** `OutlineMeasure` at 1440 units to the inch from `hmtx`, `usWinAscent`, `usWinDescent` and `hhea`, never hinted | Screen, PDF and print agree, and the Windows 95 mismatch cannot happen |
| D107 | Missing bold and italic | **Assumed.** A missing bold widens the advance by 1/64 em; a missing italic keeps its advances | The measured width matches the emboldened glyph |
| D108 | Characters with no Windows-1252 form | **Assumed.** Refused with a status message when typed or composed; `?` with a count when read from a file | The document stays ANSI (D11) |
| D109 | Printing | **Assumed.** A PDF handed to the print portal | No printer driver code in Quill |
| D110 | Plain text on Linux | **Assumed.** Read UTF-8 when valid; Save As offers UTF-8 as well as ANSI | Linux text files are UTF-8 |
| D111 | Thumbnails | **Assumed.** `.QLD` only, so LibreOffice keeps RTF thumbnails | Not displacing the user's other tools |
| D112 | App id | **Assumed.** `io.github.liminal_hq_labs.Quill` | Needed by Flatpak and AppStream; change it if a domain exists |
| D113 | Packaging | **Assumed.** AUR and Flatpak; no AppImage | Bundling GTK4 is fragile |
| D114 | Unsaved marker | **Assumed.** An asterisk before the name on Linux, and after it (with a space) on Windows and DOS | Each platform's convention |
| D115 | Autosave intervals | **Assumed.** DOS and Win16 10 minutes, Win95 5, Linux 2; off on floppies | A stall costs more the slower the machine |
| D116 | Test fonts | **Decided.** A renamed Windows-1252 subset of Liberation 2.1.5, with its licence, never Times New Roman | The OFL reserves the names Liberation, Arimo, Tinos and Cousine, and a subset is a Modified Version |
| D117–D119 | Reserved | | |

## Risks and what is not proven

- **GTK4 may resample the texture at 1.5×.** Spike G0 tests this first; the fallback is cairo, then egui.
- **Layout at 1440 is not yet exercised by the editor.** `view.rs` and parts of the painter assume 96, so 1440 may expose integer assumptions in `quill-app`. C2 and the property tests cover it.
- **Two painters until C3.** The GDI painter and `quill-raster` will drift until `Surface` merges them.
- **Font metric policy.** If the `OS/2` and `hhea` handling differs from GDI, line heights will not match Windows. The report above is the measure of that.
- **IME preedit** in a custom GTK4 widget is untested.
- **Terminal keys.** The kitty protocol was checked in two ways: against a pseudo-terminal that answers the query and sends Ctrl+I as `CSI 105;5u` (Italic is applied, the document is not modified), and in tmux 3.8, a legacy terminal, where Ctrl+I is Tab (the status line shows `Mod` and the cursor moves a tab). It was not run in kitty, foot, WezTerm or a VTE terminal, so whether each of them answers the query is unproven; VTE's support in particular is unknown.
- **Inside Flatpak,** recent files depend on the document portal keeping access across restarts.
- **CI** needs an image with GTK4 development files; whether `tauri-ci-desktop` has them is not known.
- **Autosave on DOS and Win16** is a visible pause by design.
- **Unverified claims in this page:** the Wine recipes; the GTK items under the spike; `GtkAccessibleText` and how `gtk4-rs` exposes it; whether the GNOME runtimes still include GTK3; mnemonic maturity in `PopoverMenuBar`.
