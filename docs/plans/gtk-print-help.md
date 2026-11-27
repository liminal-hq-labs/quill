# GTK printing, help and dark paper

The plan for three pieces of the GTK client (`linux/quill-gtk`): printing, print preview and Export as PDF from one PDF with a real text layer; a help viewer; and a dark paper option for the screen. It turns the owner's answers of 2026-11-27 into work packages for the UI track ([ui-track.md](../ui-track.md)), with what each needs from the shared crates and ready-to-paste builder briefs. Written against `origin/main` at `904c0e4`, with `quill-pdf` and `quill-pdfdoc` as they stand in pull request #167 (WP-WEB-3 of [web-demo.md](web-demo.md), W9X-90 to W9X-94); the package that first uses them rechecks the API against what merged.

## Table of contents

- [Owner answers](#owner-answers)
- [What exists](#what-exists)
- [Printing: the design](#printing-the-design)
- [Fonts and layout parity](#fonts-and-layout-parity)
- [Pictures](#pictures)
- [Print preview](#print-preview)
- [Flatpak and the portal](#flatpak-and-the-portal)
- [Printing tests](#printing-tests)
- [The help viewer](#the-help-viewer)
- [Dark paper](#dark-paper)
- [Decisions](#decisions)
- [What the shared crates are asked for](#what-the-shared-crates-are-asked-for)
- [Order](#order)
- [Work packages](#work-packages)
- [Builder briefs](#builder-briefs)
- [Owner decisions](#owner-decisions)

## Owner answers

1. **GTK printing is a proper PDF with a real text layer**, not raster pages, so pages print correctly. This replaces LNX-22 (raster pages through `gtk::PrintOperation`) and the raster tests of WP-GTK-3.
2. **A help viewer on GTK** (WP-GTK-8) is wanted.
3. **A dark paper option** for the screen is wanted, off by default, after libadwaita and dark mode (WP-GTK-4).
4. **One UI track builder at a time** in `linux/quill-gtk`, because every package wires into `window.rs`.

## What exists

| Piece | Where | State |
|---|---|---|
| The page painter | `quill_paint::page::paint_sheet` over the `Surface` trait (`crates/quill-paint/src/lib.rs`) | Draws a whole sheet as preview and print show it: paper, header, footer, fields with the page number and count, body lines, pictures. Text arrives as `glyph_run(font, baseline, &[(byte, x)], colour)`, rules as `fill_rect`, `hline`, `vline`, pictures as `rgb_row` rows decoded at the layout's 96 dpi |
| The screen surface | `quill_paint::pixel` with `quill_fonts::glyphs::MeasureGlyphs` | The GTK page: logical pixels at 96 to the inch, a `Scale` in 1/256 to device pixels, glyphs from the outlines of the same font files the layout measured (LNX-1) |
| The layout | `quill_fonts::OutlineMeasure` at 96, `FontSet::system()` reading the font folders directly, no fontconfig (D186) | `Resolved` names the face file; `Face::bytes()` gives its data |
| Fields | `App::field_text`, `set_print_date`, `set_print_time` (`crates/quill-app/src/print.rs`) | The shell sets the date and time |
| A PDF writer | `crates/quill-pdf` (layer `base`), pull request #167 | `no_std` + `alloc`, PDF 1.4: `Page::new(w, h, dpi)`, `text(&Run)` in the standard 14 fonts with every character nudged (`TJ`) to the layout's own position, `rect`, `image` (Flate RGB), `Pdf::add_page`, `finish`; `Font::pick(face, family, bold, italic)`; repeatable bytes when no date is given. No embedded fonts, no knowledge of documents |
| A document-to-PDF walker | `crates/quill-pdfdoc` (layer `kit`), pull request #167 | `export(app, store, &mut measure, &Options) -> Vec<u8>`: lays the body, headers and footers out with **any** `Measure` (its resolution is the page's unit) and draws each page as the Win32 shell's `draw_page` does (list labels, fields, tab leaders, vertical offsets, underline and strikeout as rectangles, pictures as images). `Options { title, created, from, to, image_dpi }` (a picture is kept at no more than the layout's own resolution). `Std14Measure { dpi }` lays a document out with the standard fonts' advances, for callers with no font files; `std14_metrics`. The Win32 build's File > Print to PDF… (W9X-90) uses it at the screen's 96 dpi (W9X-94) |
| GTK print wiring | `linux/quill-gtk/src/window.rs` | `Command::Print`, `Preview`, `PrintSetup` and `Dialog::Print` are toasts. `Command::Help` is a toast |
| GTK file seam | `linux/quill-gtk/src/choose.rs` (LNX-25) | `Chooser` (Open, Save As, Insert Picture) with `GtkChooser` and `ScriptChooser` |
| Help sources | `assets/help/quill.qhm`, parsed by `quill_help::qhm::parse` into `Help { topics: [Topic { context, title, map, paras: [Para { style, runs }] }] }` | Six topics; runs are text, bold, jump and pop-up; styles are body, heading and bulleted item. `hlpc` compiles the same source to `QUILL.HLP`. The SPEC's richer grammar (italic, code, pictures, notes, keywords, browse) is not in the parser yet |
| Settings | `preferences.md`'s View page has `[Quill] PaperColour` (`white`, `theme`, default `white`, GTK and WinUI, live) | Planned (WP-PREF-10, wave 11); nothing reads it yet |
| GTK | gtk4-rs 0.11 with `v4_20`, libadwaita 0.9 (`v1_6`); the host has GTK 4.24.1, libadwaita 1.10 | `gtk::PrintDialog` (GTK 4.14) and `gtk::PrintSetup` are in the bindings |
| Host tools | `pdftotext`, `pdftoppm`, `pdfinfo`, `qpdf`, `Xvfb`, `weston`, `mutter`, `gtk4-broadwayd`, `xdotool`, `/usr/lib/xdg-desktop-portal`, `-gtk` and `-gnome` | All present on the owner's machine |

## Printing: the design

**One walker, one PDF, three uses.** GTK writes its PDF with `quill_pdfdoc::export(app, store, &mut measure, &options)` from pull request #167, the same walker the Windows 95 build's Print to PDF uses (W9X-91), and adds nothing of its own to the PDF. The measure it passes is a clone of the window's own `OutlineMeasure` at 96 to the inch, the one the page was laid out with (LNX-1), so `export` lays the document out again into the very lines and pages the window shows. Those bytes are:

- **File > Export as PDF…**: written to the file the save chooser names.
- **File > Print…**: handed to `gtk::PrintDialog::print` as a stream, after `PrintDialog::setup` has shown the print dialog. GTK sends it to CUPS on a normal desktop and to the print portal inside Flatpak; Quill does neither itself.
- **Tests**: compared with golden PDFs and read back with `pdftotext`.

So the paper is the export, byte for byte. The preview is drawn by `quill_paint::page::paint_sheet` on the pixel surface, a second walker over the same layout, and the tests hold the two together (word positions, page count, headers and fields).

```text
                    App + OutlineMeasure at 96 (one layout)
                   /                                       \
     paint_sheet (quill-paint)                    quill_pdfdoc::export (kit)
     on the pixel surface                         into quill-pdf pages (base)
        |            |                                 |                 |
     the page   Print Preview                    Export as PDF…   PrintDialog::print(stream)
                                                                   |               |
                                                                 CUPS      print portal (Flatpak)
```

**What was weighed.**

| Route | For | Against |
|---|---|---|
| **`quill_pdfdoc::export` bytes through `gtk::PrintDialog` (taken)** | One generator for paper, export, the Win32 Print to PDF and a later `quill to-pdf`; nothing new in a shared crate; the paper can be tested as a file; GTK 4.14's dialog picks the portal by itself | Page ranges and page sets are Quill's to apply; the standard 14 fonts until embedding lands; the preview's walker is a different one, held to it by tests |
| A `quill_paint::Surface` over `quill_pdf::Page`, so `paint_sheet` draws the PDF | The preview and the paper share a walker | A second PDF walker beside `quill-pdfdoc`, which the Windows build already uses; the earlier draft of this plan, dropped when #167 arrived |
| `gtk::PrintOperation` with `draw-page` replaying the painter onto cairo (glyphs through FreeType) | GTK applies ranges, n-up and scaling; cairo embeds subset fonts | A second renderer whose output is not the export; cairo-rs's `freetype` feature and `freetype-rs`; GTK's own preview would be a third |
| Raster pages through `PrintOperation` (LNX-22) | Nothing new | No text layer (the owner's first answer), 300 dpi cap, large spool files |
| `GtkPrintUnixDialog` and `GtkPrintJob::set_source_fd`, plus our own D-Bus call to `org.freedesktop.portal.Print` under Flatpak | Full control of the dialog | Two code paths that `PrintDialog` already merges |

**What GTK passes.** `measure`: `window.measure().clone()` (the `OutlineMeasure` with the same `FontSet`; never `Std14Measure`, which is for callers with no font files and would break lines differently from the screen). `Options`: `title` the document's `\info` title or its file name in Windows-1252; `created` as below; `from` and `to` as below; `image_dpi` 300 (pictures are capped at the layout's 96 today, S3). GTK's code is a small `linux/quill-gtk/src/pdf.rs`: `PdfJob` (the options worked out from the window and the dialog) and `document_pdf(window_state, &PdfJob) -> Vec<u8>`, which calls `export` and nothing else; no display is needed, so the tests run it directly.

**Pages, ranges and settings.** `PrintDialog::setup` returns a `gtk::PrintSetup` holding `PrintSettings` and `PageSetup`. `quill_pdfdoc::Options` takes one run of pages (`from`, `to`). When the dialog asks for all pages, the current page (the caret's) or one range, Quill writes just those pages and sets the settings back to all pages before `PrintDialog::print(setup)`, so nothing applies them twice. Several ranges, even or odd pages, or reverse order are left in the settings for the print system, with the whole document written, until `quill-pdfdoc` takes a list of pages (S6). Copies, collation, scale and pages per sheet are left to the print system. The first package to print proves on a four-page document that a range of 2–3 gives two sheets through Print to File and, if a CUPS queue is available, through CUPS; if either applies the range again, the record says so and the settings handling changes (LNX-81).

- **The dialog is seeded** with the document's paper size and orientation (`gtk::PaperSize::new_custom` from the twips, or the matching named size) and the last settings of the session.
- **Selection** is not offered: `PrintDialog` has no switch for it. The Windows builds' Selection choice is missing on GTK (LNX-81).
- **Printing shows** a toast, `Printing LETTER.QLD on <printer>.`, once the stream is written. Cancel is the desktop's print queue: the job is the system's as soon as GTK has it, so the SPEC's "Cancel stops between pages" has nothing to stop. Writing the PDF runs one page per idle callback on the main thread, so a long document does not freeze the window; the autosave plan's executor (PR 18) can take it later.
- **Print Setup…** runs `PrintDialog::setup` alone and keeps the `PrintSetup` for the session, to seed the next Print. It does not change the document: paper and margins are the document's (Page Layout, in the parked block), and the PDF's pages are the document's paper. A printer paper that differs is the print system's to fit (CUPS `print-scaling=auto`). The SPEC's "changing the printer or paper lays the document out again" is a Windows build note, since GTK's layout never depends on a printer (D186).

**Headers, footers and fields.** `export` draws them (first-page header and footer included) and asks the app for the field text; `paint_sheet` draws them for the preview. Before each print, preview or export the window calls `set_print_date` with the local short date (`glib::DateTime::now_local().format("%x")`) and `set_print_time` with `%X`, converted to Windows-1252 with `?` for what it lacks; tests set both through `--print-date` and `--print-time`. `[File]` is the file name, or `(Untitled)`.

**The PDF's own fields.** Title: the document's `\info` title, else the file name; creation date: now, in UTC, except under `--fixed-date` or with `SOURCE_DATE_EPOCH` set, which leave it out so the bytes repeat.

**Export as PDF…** is a GTK item after Print Setup… in File, `win.export-pdf`, with no accelerator (GNOME has none for it and Ctrl+Shift+E is often taken). The shared menu tree has no such command; the GTK client inserts the item as it inserts the recent files, until the shared tree gains one (S4). The chooser goes through `choose.rs` (a new `export_pdf` call: the document's folder, its stem with `.pdf`, one PDF type) so a test can answer it. A failed write says `Quill cannot write <name>.` as Save does.

## Fonts and layout parity

**The rule:** the layout is made once, at 96 to the inch, by `OutlineMeasure` over the font files `quill-fonts` finds, and the screen, the preview and the paper all place each character at that layout's position. GTK's own fonts (Pango and fontconfig) draw the chrome and the help viewer only; they never touch the page, the preview or the PDF.

| | Where a character goes | Its shape |
|---|---|---|
| Screen | The layout × `Scale` | The resolved face's outline (`MeasureGlyphs`) |
| Preview | The same | The same |
| PDF, first | The layout, exactly (`TJ` nudges per character) | The standard 14 font that `Font::pick` chooses: Times for Times New Roman and serif faces, Helvetica for Arial and sans faces, Courier for monospaced ones. Not embedded: the viewer or the printer (Ghostscript or Poppler, with the URW base 35 faces) draws it |
| PDF, later (S1) | The same | The resolved face itself, embedded as a TrueType simple font with `WinAnsiEncoding`, so paper and screen are the same glyphs |

**What that means for a document.** Line breaks, page breaks, page numbers and every word's position are the same on screen, in preview and on paper in both stages, because nothing re-lays out (the Windows 95 build's D81 split does not happen here). In the first stage a face that is not Times New Roman, Arial or Courier New (or their Liberation equivalents, which share their advances) prints in the nearest standard shape at the layout's positions: readable and in place, but not the screen's shape. Bold and italic map to the standard bold and italic; a synthetic bold on screen (D187) is a real bold on paper.

**Names.** The document names a face (`Times New Roman`) and a family code; `quill-fonts` resolves it through its fallback table (the face, then Times New Roman, Liberation Serif and Quill Test Serif for a serif face, and so on). The PDF font is chosen from the *request* in the first stage (`quill-pdfdoc` calls `Font::pick` with the face name, the family code and the bold and italic flags) and from the *resolved file* in the second. The formatting bar lists what `FontSet::families` finds, as now.

**Embedding (S1).** Asked of `quill-pdf`'s and `quill-pdfdoc`'s owner, not built here (W9X-92 already plans embedded subsets): in `quill-pdf`, `Pdf::embed_truetype(bytes, index) -> FontId` and a `Run` that names either a standard font or an embedded one; in `quill-pdfdoc`, a way for the caller to hand it the resolved face's bytes for a `FontRef` (an optional trait beside `Measure`, which `quill-fonts`' `Face::bytes()` can serve on GTK); and a `/FontFile2` stream, `/Widths` for bytes 32 to 255 from the layout's own advances (so a viewer's widths and the `TJ` nudges agree), a `ToUnicode` map for the text layer, and a subset made by emptying the `glyf` entries of glyphs no page uses (keeps every table, needs no rewrite of `cmap`). It honours the face's `OS/2 fsType`: a face that forbids embedding (restricted licence, `0x0002`) stays a standard font. Budget: a page of Liberation Serif in one style adds about 20 to 40 KB [E].

**Flatpak.** Inside the sandbox the host's fonts are at `/run/host/fonts` and `/run/host/user-fonts`, which `FontSet::system()` does not read. WP-GTK-9 adds both with `FontSet::add_dir` when `/.flatpak-info` exists; without that, a Flatpak build lays out with the runtime's fonts and breaks lines differently from the same file opened in the native build.

## Pictures

`quill_pdfdoc::export` decodes each picture through `quill_app::picture::render` at `Options::image_dpi`, but never above the layout's own resolution, so with GTK's 96 dpi layout a picture goes into the PDF at 96 dpi whatever `image_dpi` says: on paper it has the screen's pixels at 100 percent, soft at arm's length but in the right place and size (the Windows build has the same limit, W9X-94). The second stage needs `quill-pdfdoc` to keep pictures at `image_dpi` from the source's own pixels, up to 300 dpi and never more than the source has, independent of the layout's resolution (S3): a 4 by 3 inch picture is then at most 1,200 by 900 pixels (3.2 MB raw, Flate-compressed in the file). GTK already passes `image_dpi: 300`, so it needs no change when S3 lands. The preview draws pictures at the screen's resolution, as the page does.

## Print preview

Quill's own, in the window, as on the Windows builds (SPEC "Printing and preview", D100), from `paint_sheet` into the pixel surface at the window's device scale, so it is as sharp as the page and equal to it. GTK's print dialog shows no preview button (`PrintDialog` has none), and GTK's external previewer is not used.

- **Where:** a second page of the window's `adw::ViewStack`, `preview`, which replaces the page, the toolbar, the formatting bar and the ruler while it shows; the status bar says `Page N of M`.
- **Bar:** Print, Next Page, Previous Page, Two Pages (One Page while two show), Zoom (the whole page, or its actual size with scroll bars) and Close, flat buttons in a row under the header bar with tooltips and the status sentence of the Windows build.
- **Keys:** Page Up and Page Down, Home and End, T (one or two pages), P (Print), Esc and Close leave it, and the page comes back with the scroll position it had.
- **Paper:** always white, whatever the paper colour setting (LNX-88).
- **State:** a `preview::State` (page, one or two, whole or actual) driven by keys and buttons as data, so a test can run it without a display.

## Flatpak and the portal

- `gtk::PrintDialog` uses `org.freedesktop.portal.Print` when GTK runs sandboxed (`PreparePrint` for the dialog, `Print` with a file descriptor of the PDF). The manifest of WP-GTK-9 therefore needs no CUPS socket and no file access for printing.
- The portal's dialog is the host's (`xdg-desktop-portal-gnome` or `-gtk`); its preview button, if any, is the host's business.
- Outside Flatpak, `GDK_DEBUG=portals` makes GTK use the portal too, which is how the portal path is tested on the host: a private session bus running `/usr/lib/xdg-desktop-portal` and `/usr/lib/xdg-desktop-portal-gtk` against the test's own Xvfb, with `XDG_CURRENT_DESKTOP=GNOME`.
- Export as PDF goes through the file chooser portal as Save As does (LNX-25's seam).

## Printing tests

Every command that starts a GTK process sets its display explicitly and never inherits the host's: `env -u WAYLAND_DISPLAY DISPLAY=:<n> GDK_BACKEND=x11` for the test's own `Xvfb :<n>`; `env -u DISPLAY WAYLAND_DISPLAY=<socket> GDK_BACKEND=wayland` for its own headless Weston or `mutter --headless --virtual-monitor`; `env -u DISPLAY -u WAYLAND_DISPLAY GDK_BACKEND=broadway BROADWAY_DISPLAY=:<n>` for its own `gtk4-broadwayd :<n>`. Each also gets a private session bus (`dbus-run-session`), a scratch `XDG_RUNTIME_DIR`, `GSETTINGS_BACKEND=memory` and a scratch `HOME`. Processes are stopped by the PID the script started, never by name.

| Test | What it proves | Display |
|---|---|---|
| **Golden PDFs** (`linux/quill-gtk/tests/golden/pdf/letter.pdf`, `print3.pdf`, `headers.pdf`, `pictures.pdf`) from `document_pdf` (so `quill_pdfdoc::export` with the window's `OutlineMeasure`) with the vendored fonts and no date, compared byte for byte; `QUILL_UPDATE_GOLDEN=1` rewrites them | The bytes repeat; a change is reviewed as a diff | None |
| **The text comes back** (`pdftotext -layout`, page by page, against the document's text per page from the layout) | The text layer, its order, the page each line is on, headers, footers and the page number and count on every page of `print3.rtf` | None |
| **The words are where the screen put them** (`pdftotext -bbox-layout`: each word's left edge, in points, equals the pen x of its first character × 0.75 within 0.05 pt, where the pen x comes from a recording `Surface` run through `paint_sheet`, the preview's walker) | The two walkers, `paint_sheet` and `quill_pdfdoc::export`, put every word in the same place on the same page | None |
| **Structure** (`qpdf --check`; `pdfinfo` page count and page size equal the document's sheets and paper) | A valid file of the right shape | None |
| **Rules and pictures** (`pdftoppm -r 96` against `paint_sheet` on the pixel surface at scale 1, compared only outside text boxes: underlines, picture pixels within 8 levels) | Non-text drawing matches | None |
| **The same pages** (`pdfinfo`'s page count equals the preview's sheet count, and each page's first and last words equal the first and last words `paint_sheet` draws on that sheet, for every file in `tests/docs`) | Page breaks agree between preview and paper | None |
| **Page selection** as data (all, current, ranges, even, odd, reverse over a six-page document) and through the seam (`--script-printer FILE` writes the stream it would print) | Quill applies the dialog's choices once | None |
| **The real print dialog** under Xvfb, driven by `xdotool`: Print to File, a scratch output name, Print; the file equals the export's bytes (or the same `pdftotext` and page count if GTK rewrites it, which the record then says) | The GTK path works end to end | Own Xvfb |
| **The portal** with `GDK_DEBUG=portals` and a private `xdg-desktop-portal` with the GTK backend; the same Print to File check | The Flatpak path | Own Xvfb and session bus; best effort, and said so if it does not start |
| **Preview** state machine as data; the preview's texture equals `paint_sheet` on the pixel surface at scales 1 and 2 (byte for byte); keys under `xdotool` | Preview equals the page; the keys | Own Xvfb |

**Evidence** goes to `~/Documents/Liminal HQ/Quill/GTK4/<package>/`: the print dialog seeded with the document's paper; the preview at one and two pages in light and dark at scales 1 and 2; a PDF page rendered by `pdftoppm` beside the screen at the same size; `pdftotext` output of `print3.pdf`; the Print to File result's `pdfinfo`. The shots that document the feature are also committed as `docs/media/gtk-print-dialog-light.png`, `gtk-preview-two-light.png` and `gtk-pdf-beside-screen.png`.

## The help viewer

**Source.** `assets/help/quill.qhm`, the source `hlpc` compiles into `QUILL.HLP` for the Windows builds, compiled into the GTK binary with `include_str!` and parsed at first use by `quill_help::qhm::parse`. No help file is installed, so it cannot be missing, and the topics on GTK are the Windows help's topics. The viewer reads the parsed model (`Help`, `Topic`, `Para`, `Run`), not the compiled `.HLP` (whose reader gives marked-up strings for tests). A parse error is a test failure, never a run-time state.

**Not Yelp, not WebKit.** Yelp needs Mallard or DocBook pages installed under `/usr/share/help`, a converter from `.qhm`, and inside Flatpak it is often not there to open `help:` addresses. WebKitGTK would add tens of megabytes for six topics. The viewer is GTK widgets.

**The window.** An `adw::Window` of its own, not modal, one per application (F1 again brings it forward), 480 by 640 logical pixels by default, titled `Quill Help — <topic title>`:

- **Header bar:** Back (Alt+Left), Forward (Alt+Right), Contents (Alt+Home), and a search toggle (Ctrl+F); Ctrl+W and Esc (with nothing open inside) close it.
- **Page:** a `gtk::ScrolledWindow` holding an `adw::Clamp` (640 wide) of one `gtk::Label` per paragraph: wrapped, left-aligned, selectable, with Pango markup built from the runs (text escaped, bold as `<b>`, a jump as `<a href="jump:context">`, a pop-up as `<a href="popup:context">`). Headings use the `title-2` style and the heading role; items are a bullet and a label with a hanging indent. Labels give links Tab focus, Enter to follow and Orca's reading for free.
- **Jumps** replace the topic and push the history. **Pop-ups** open a `gtk::Popover` holding the pop-up topic's paragraphs, pointing at the click, or at the label when Enter followed the link; Esc closes it.
- **Search:** a `gtk::SearchBar` with a `gtk::SearchEntry` (Ctrl+F, or typing anywhere in the window); results in a `gtk::ListBox` in place of the topic, ranked by title prefix, title words, keywords once the parser keeps them, then body text, case-folded; Enter opens the first, Down moves into the list, Esc ends the search.
- **History:** `help::History` (back and forward stacks of context names, capped at 64) as data.

**F1.** F1 in the main window opens the contents topic (`Command::Help`, which the window now answers instead of a toast). F1 with a Quill dialog focused opens that dialog's topic when the help maps one (the `map` numbers and context names of `help.toml`'s `[MAP]`, through a `topic_for_dialog(name)` table in `help.rs`), else the contents. The help sources have no dialog topics yet; adding them is content work (S5), and the table makes it a one-line change per dialog.

**What it shows today and later.** Today's runs and styles; anything the parser gains later (italic, code, pictures through `quill-image` into a `gdk::MemoryTexture`, shaded notes) maps to a Pango attribute or a widget in `help.rs` when it lands, and an unknown run shows as plain text.

**Size budget.** The source is about 2.5 KB, the parser and model a few KB of code; the release binary may grow by at most 150 KB with the viewer (measured from a stripped release build before and after), and the Flatpak gains no file. Pictures in help, when the format has them, count against the same 150 KB plus their own size.

**Tests.** Every topic opens (`--help-topic CONTEXT` for each, under Xvfb, with the window's title checked); every jump and pop-up names a topic that exists (no display: over the parsed model, so it also guards the content); search finds every topic by its own title, and the ranking is data; the history; F1 from the window and from a focused dialog; Tab reaches each link and Enter follows it (`xdotool`); the AT-SPI tree of the help window has the heading and the links by name (with WP-GTK-5's `tools/atspi-read.py` if merged; otherwise a manual note).

**Evidence** (`GTK4/WP-GTK-8/`): the contents, a topic, a pop-up, a search with results, in light and dark at scales 1 and 2; `docs/media/gtk-help-contents-light.png` and `gtk-help-popup-dark.png`.

## Dark paper

**What it is.** `[Quill] PaperColour` (`white` or `theme`, default `white`; preferences.md's View page). With `theme` and the dark style on (libadwaita's `StyleManager::is_dark`, which WP-GTK-4 follows), the editing page is drawn as dark paper with light text. It changes what is drawn, never the document: nothing is saved, and the RTF and QLD bytes are the same.

**Where it applies:** the editing page and the header and footer views. **Where it never applies:** Print, Export as PDF, Print Preview (it previews paper), thumbnails, the clipboard, and `--dump` and `--screenshot` unless `--paper theme` asks for it.

**How.** A `ThemedSurface<S: Surface>` in `linux/quill-gtk/src/paper.rs` wraps the pixel surface and maps colours by the kind of call, with integer arithmetic so it can move into `quill-paint` when WinUI is its second user (WP-WINUI-8). WP-GTK-4 adds the selection's accent colour through the same wrapper, so dark paper extends that table rather than adding a second one.

| Drawn | On white paper | On dark paper |
|---|---|---|
| The paper (`fill_rect` white) | White | Adwaita's dark view colour, `#1e1e1e` |
| Text in the automatic colour, and black text | Black | `#deddda` |
| Text in another colour | As set | As set if its contrast with the paper is at least 4.5:1; otherwise its lightness is turned over (hue and saturation kept, lightness 100 − L in HSL), then raised until it reaches 4.5:1 |
| White text | White (invisible, as on Windows) | White |
| Underline, strikethrough, tab leaders, the page-break line, the spelling wave | Their colour | By the text rule (so the red wave becomes a light red) |
| Selection | WP-GTK-4's accent, with white text | The same; the selection is not remapped |
| Caret | Inverted pixels | Inverted pixels, so it shows light on dark |
| Other fills (picture placeholder, field shading) | As drawn | Lightness turned over when it is lighter than 50 percent; else as drawn |
| The sheet's edge and shadow | Black and the shadow colour | The edge `#5e5c64` (Adwaita's dark border), the shadow as WP-GTK-4 draws it on the dark desk |
| Pictures (`rgb_row`, `bitmap4`) | As decoded | As decoded: never inverted |
| High contrast and dark | — | Paper `#000000`, text `#ffffff`, other colours raised to 7:1 |

**Turning it on.** Until WP-PREF-10's Preferences dialog exists, a GTK check item **View > Dark Paper** (`win.dark-paper`, after Status Bar) sets `PaperColour` and is greyed in the light style; the value is kept in `gtk.ini` (`[Window] PaperColour`), and WP-PREF-10 moves it to `QUILL.INI`'s `[Quill] PaperColour`, reading the `gtk.ini` value once. A `--paper white|theme` option sets it for tests.

**Tests.** The colour table as data (the sixteen Windows colours and the automatic colour in, the mapped colours out, each checked at 4.5:1, or 7:1 in high contrast); the wrapper over a recording surface for a selected word, a picture and a wave; a golden texture `tests/golden/gtk/letter.1.00.darkpaper.png`; with `theme` set and the dark style forced, the preview's texture and `document_pdf`'s bytes equal their white goldens.

**Evidence** (`GTK4/WP-GTK-10/`): the same document on white and dark paper in the dark style, the high-contrast dark paper, a selection and a picture on dark paper, the preview still white, at scales 1 and 2, from WP-GTK-4's headless GNOME compositor run.

## Decisions

Reserved for this plan: **LNX-80 to LNX-89** (above LNX-28 on `main`, LNX-60 to LNX-64 of the preferences plan and LNX-70 to LNX-74 of the Reveal Codes plan). LNX-80 is recorded in `docs/decisions.md` now, because the owner decided it; each package copies its other rows in when it opens its pull request, marked **Assumed** unless the owner has answered, and records its own further choices as sub-ids of its rows (LNX-81.1 and so on).

| Id | Decision | Package |
|---|---|---|
| LNX-80 | GTK prints a PDF with a text layer: `quill_pdfdoc::export` with the window's own `OutlineMeasure` at 96 writes the bytes for Print, Export as PDF and the tests (supersedes LNX-22) | WP-GTK-3b |
| LNX-81 | Printing goes through `gtk::PrintDialog` (setup, then print a stream); Quill applies all, current or one range (`from`, `to`) and resets it in the settings, and leaves several ranges, even or odd and reverse to the print system until S6; copies, collation, scale and pages per sheet are the print system's; no Selection choice | WP-GTK-3c |
| LNX-82 | The PDF's pages are the document's paper; Print Setup chooses a printer and paper for the session and never changes the document; a different printer paper is fitted by the print system | WP-GTK-3c |
| LNX-83 | Fonts in the PDF: the standard 14 by face then family, every character at the layout's position; the resolved face embedded when `quill-pdf` and `quill-pdfdoc` can (S1), honouring `fsType` | WP-GTK-3b |
| LNX-84 | Pictures in the PDF at the layout's 96 dpi until `quill-pdfdoc` keeps them at `image_dpi` from the source (S3); GTK passes 300 now | WP-GTK-3b |
| LNX-85 | Print Preview is Quill's own, a page of the window's view stack drawn by the pixel surface, with the Windows build's bar and keys | WP-GTK-3a |
| LNX-86 | Export as PDF… is a GTK item after Print Setup… with no accelerator, through the chooser seam; the date and time fields use the local `%x` and `%X`; the file's creation date is left out under `--fixed-date` or `SOURCE_DATE_EPOCH` | WP-GTK-3b |
| LNX-87 | Help is an in-app libadwaita window over `quill_help::qhm`'s model of `assets/help/quill.qhm`, compiled in: labels with links, Back and Forward, search, pop-ups as popovers, F1 and the dialog table; not Yelp and not WebKit | WP-GTK-8 |
| LNX-88 | Dark paper: `PaperColour=theme` in the dark style draws the editing page through a colour-mapping surface by the rules above; print, preview, export and thumbnails stay white; a View > Dark Paper item and `gtk.ini` until WP-PREF-10 | WP-GTK-10 |
| LNX-89 | Spare | — |

## What the shared crates are asked for

None of these blocks a GTK package except S0. Each is the lead's to route to the architecture lane, the web lane that owns `quill-pdf` and `quill-pdfdoc`, or the content lane; the GTK package that would use it names the stage it ships without it.

| Ask | What | Owner | Needed by | Without it |
|---|---|---|---|---|
| **S0** `quill-pdf` and `quill-pdfdoc` merged | Pull request #167 (WP-WEB-3) on `main`: `quill_pdfdoc::export`, `Options`, `Std14Measure`, and `quill-pdf` under it | Web lane (WP-WEB-3) | WP-GTK-3b, 3c: **hard** | WP-GTK-3a and WP-GTK-8 go first |
| **S1** Embedded TrueType | In `quill-pdf`: `embed_truetype`, `/FontFile2`, `/Widths` from the layout, `ToUnicode`, glyph-emptying subset, `fsType` respected; in `quill-pdfdoc`: a way to be handed the resolved face's bytes; proposed as **WP-PDF-1** (W9X-92's "embedded subsets later") | The owner of `quill-pdf` and `quill-pdfdoc`, after WP-WEB-3 | WP-GTK-3b's second stage: soft | The standard 14 shapes at the layout's positions |
| **S2** One walker for preview and paper | Draw `quill-pdfdoc`'s pages and `paint_sheet`'s sheets from one walk (a `Surface` sink in `quill-pdfdoc`, or `paint_sheet` built on its line drawing), so the parity tests of WP-GTK-3b become true by construction; proposed as **WP-PDF-2**, after WP-ARCH-2 (`quill-picture`) | Architecture lane | Nothing: soft | Two walkers over one layout, held together by the tests |
| **S3** Pictures at their own resolution | `quill-pdfdoc` keeps a picture at `Options::image_dpi` from the source's pixels (never more than it has), not capped at the layout's resolution | The owner of `quill-pdfdoc` | Sharp pictures on paper: soft | Pictures at 96 dpi |
| **S4** `Command::ExportPdf` in the shared menu tree | One command for GTK's Export as PDF… and Win32's Print to PDF… (W9X-90's item lives in the Win32 shell today), with its tip; rides with or after WP-ARCH-5 (`quill-menu`) | Architecture lane | Nothing: soft | A GTK-only item |
| **S5** Help content for dialogs | A topic per dialog in `assets/help/quill.qhm` with `[MAP]` numbers, and a "Using help" topic | Content (docs) lane | F1 in a dialog: soft | F1 in a dialog opens the contents |
| **S6** A list of pages | `quill_pdfdoc::Options` takes the pages to write, in order (several ranges, even or odd, reverse), beside `from` and `to` | The owner of `quill-pdfdoc` | WP-GTK-3c: soft | Several ranges, even or odd and reverse are left to the print system |

## Order

This replaces the GTK rows of [ui-track.md](../ui-track.md#order-and-parallelism); the WinUI rows are unchanged. One GTK builder at a time.

| # | Package | Starts when | Notes |
|---|---|---|---|
| 1 | WP-GTK-1 Dialogs and keyboard (G3) | Built | — |
| 2 | **WP-GTK-3a** Print Preview | **Now** | Needs nothing shared |
| 3 | **WP-GTK-8** Help viewer | After WP-GTK-3a, if pull request #167 (`quill-pdf`, `quill-pdfdoc`) has not merged; otherwise after WP-GTK-3c | Fills the GTK slot while S0 is pending |
| 4 | **WP-GTK-3b** Export as PDF | WP-GTK-3a merged and S0 merged | — |
| 5 | **WP-GTK-3c** Print | After WP-GTK-3b | — |
| 6 | WP-GTK-2 System clipboard (G5) | The clipboard bridge has merged | Takes the next free slot when it does |
| 7 | WP-GTK-4 libadwaita finish and dark mode | After WP-GTK-3c | — |
| 8 | **WP-GTK-10** Dark paper | After WP-GTK-4 | The owner's order |
| 9 | WP-GTK-5 Accessibility (G7) | After WP-GTK-10 | — |
| 10 | WP-GTK-6, WP-GTK-7 | As before | — |
| 11 | WP-GTK-9 Packaging (G8) | By the owner's call | Adds `/run/host/fonts`; the portal print check in a real Flatpak |

## Work packages

Every package follows "Rules for every package" in [ui-track.md](../ui-track.md#rules-for-every-package), its gate `make gtk`, `bash scripts/check-headers.sh` and markdownlint in Docker, and its evidence folder `~/Documents/Liminal HQ/Quill/GTK4/<package>/`. **Must not touch** for all of them, unless the package says otherwise: `crates/**` (including `quill-pdf`, `quill-paint`, `quill-help`, `quill-fonts`), `assets/**`, `dos/**`, `win16/**`, `win9x/**`, `winui/**`, the root `Cargo.toml` and `Cargo.lock`, `.github/**`, other packages' rows, and `SPEC.md` beyond the Linux desktop platform row and the Quill for Linux desktops component line.

### WP-GTK-3a Print Preview

- **Finish line.** File > Print Preview and the toolbar's Print Preview button replace the page with one or two whole sheets drawn by `paint_sheet` on the pixel surface at the window's scale, with headers, footers and fields (the print date and time set first), under the bar of SPEC "Printing and preview"; Page Up, Page Down, Home, End, T, P (a toast until WP-GTK-3c) and Esc work; Close brings the page back at the scroll position it had; the paper is white. Print Setup and Print stay toasts.
- **Files owned.** New `linux/quill-gtk/src/preview.rs` (`State` as data, the preview widget, the bar), new `tests/preview.rs`, new `tools/preview-shots.sh`; `src/window.rs` (the view stack page and the commands), `src/main.rs` (`--preview`, `--print-date`, `--print-time` test options), `src/lib.rs` (the module line); the GTK rows of `docs/linux.md`, `docs/progress.md`, LNX-85.
- **Dependencies.** None.
- **Tests.** `State` over every key and button; the preview's texture equals `paint_sheet` on the pixel surface at scales 1 and 2, byte for byte, for `letter.rtf` and page 2 of `print3.rtf`; header, footer and `Page 2 of 3` present (the painter's own field text, checked through a recording surface); keys under `xdotool` in an own Xvfb.
- **Evidence.** One and two pages, whole and actual size, light and dark, scales 1 and 2; `docs/media/gtk-preview-two-light.png`.
- **Parallel.** With WinUI packages.

### WP-GTK-3b Export as PDF

- **Finish line.** File > Export as PDF… writes the document through `quill_pdfdoc::export` with the window's own `OutlineMeasure` at 96: every sheet, its headers, footers and fields, the text at the layout's positions in the standard 14 fonts, rules and pictures; `qpdf --check` passes; `pdftotext` gives the document's text page by page; the words and page breaks equal the preview's; the goldens repeat byte for byte.
- **Files owned.** New `linux/quill-gtk/src/pdf.rs` (`PdfJob` and `document_pdf`, which builds `quill_pdfdoc::Options` and calls `export`, nothing more; no GTK in it), new `tests/pdf.rs` (with a recording `Surface` for `paint_sheet` as a test helper), new `tests/golden/pdf/*.pdf`; `src/choose.rs` (an `export_pdf` call in `Chooser`, `GtkChooser` and `ScriptChooser`), `src/window.rs` (`win.export-pdf`, the item after Print Setup…), `src/menu.rs` (the inserted item), `src/main.rs` (`--export-pdf OUT`, `--fixed-date`, `--print-date`, `--print-time`), `linux/quill-gtk/Cargo.toml` and `Cargo.lock` (adds `quill-pdfdoc`; `quill-pdf` only if a type of it is named); docs rows, LNX-80 (the row exists; add what the package learned as LNX-80.1 and on), LNX-83, LNX-84, LNX-86.
- **Must not touch.** As every package, and `crates/quill-pdf/**` and `crates/quill-pdfdoc/**` in particular: a difference between the two walkers that a test finds is reported (with the file, page and word) to `quill-pdfdoc`'s owner, not patched, and the test that shows it is kept, marked `#[ignore]` with the report's reason.
- **Dependencies.** **Hard:** S0. Soft: S1, S3 (later stages), S2 (one walker), S4 (moves the item onto the shared command).
- **Tests.** As [Printing tests](#printing-tests): the goldens, the text, the word positions, the same pages, the structure, rules and pictures; Export through the seam (bytes on disk equal `document_pdf`'s); a write to a read-only folder gives the message.
- **Evidence.** `letter.pdf` and `print3.pdf` rendered by `pdftoppm` beside the screen at the same size; `pdftotext` output; the Export chooser under a headless Weston; `docs/media/gtk-pdf-beside-screen.png`.
- **Parallel.** With WinUI packages.

### WP-GTK-3c Print

- **Finish line.** File > Print… (Ctrl+P, the toolbar's Print, P in the preview) opens `gtk::PrintDialog` seeded with the document's paper and the session's last settings, writes the chosen pages with WP-GTK-3b's `document_pdf` (all, the current page or one range as `from` and `to`) and prints them as a stream; Print Setup… runs the setup alone and keeps it for the session; `Dialog::Print` is answered; the toast names the document and the printer. Under `GDK_DEBUG=portals` the same works through the print portal.
- **Files owned.** New `linux/quill-gtk/src/print.rs` (a `Printer` seam with `GtkPrinter` over `PrintDialog` and `ScriptPrinter` for `--script-printer FILE`; `PageChoice` from the settings as data; the paper seeding), new `tests/print.rs`, new `tools/print-check.sh` (the Xvfb and portal runs); `src/pdf.rs` (only to take a `PageChoice`), `src/window.rs`, `src/preview.rs` (P and the bar's Print), `src/main.rs`; docs rows, LNX-81, LNX-82.
- **Must not touch.** As WP-GTK-3b.
- **Dependencies.** WP-GTK-3b merged. Soft: S6 (several ranges, even or odd, reverse written by Quill).
- **Tests.** `PageChoice` over a six-page document built in the test (all, current, one range give `from` and `to` and reset the settings; several ranges, even, odd and reverse keep the whole document and the settings); the seam's stream equals `document_pdf` for the choice; under an own Xvfb, Print to File through the real dialog gives the export's bytes (or the same text and pages, recorded); a range of 2–3 on the six-page document gives two pages, not one (no double range); pages 1–2 and 5 give what the print system makes of the whole document, recorded; the portal run, best effort.
- **Evidence.** The dialog seeded with Letter and with A4, the portal's dialog if it ran, `pdfinfo` of the printed file; `docs/media/gtk-print-dialog-light.png`.
- **Parallel.** With WinUI packages.

### WP-GTK-8 Help viewer

- **Finish line.** Help > Contents and F1 open the help window on the contents topic of `assets/help/quill.qhm`; topics show headings, paragraphs, bold, bulleted items, jumps and pop-ups; Back, Forward, Contents and search work from the keyboard; F1 with a dialog focused opens its topic when the table maps one; Orca can read the topic (labels).
- **Files owned.** New `linux/quill-gtk/src/help.rs` (the model: markup from runs, `History`, `search`, `topic_for_dialog`; no widgets) and `src/help_window.rs`, new `tests/help.rs`, new `tools/help-shots.sh`; `linux/quill-gtk/Cargo.toml` and `Cargo.lock` (adds `quill-help`), `src/window.rs` (`Command::Help` and F1 in dialogs), `src/dialogs.rs` (F1 reporting the dialog's name only), `src/main.rs` (`--help-topic`); docs rows, LNX-87.
- **Must not touch.** As every package, including `crates/quill-help/**` and `assets/help/**`: missing topics are S5.
- **Dependencies.** None. S5 for dialog topics.
- **Tests.** As [The help viewer](#the-help-viewer); plus the size budget (stripped release binary before and after, at most 150 KB more).
- **Evidence.** As that section.
- **Parallel.** With WinUI packages.

### WP-GTK-10 Dark paper

- **Finish line.** With View > Dark Paper on and the dark style, the editing page and the header views are dark paper by LNX-88's rules, at once and back again when the style changes; print, export, preview and thumbnails stay white; the light style greys the item and shows white paper.
- **Files owned.** New `linux/quill-gtk/src/paper.rs` (the colour table and `ThemedSurface`), new `tests/paper.rs`, new `tests/golden/gtk/letter.1.00.darkpaper.png`; `src/page.rs` (wrapping the surface), `src/settings.rs` (`PaperColour` in `gtk.ini`), `src/window.rs` and `src/menu.rs` (the item), `src/main.rs` (`--paper`); docs rows, LNX-88.
- **Dependencies.** WP-GTK-4 merged (the style, the accent wrapper). WP-PREF-10 later takes over the setting.
- **Tests.** As [Dark paper](#dark-paper).
- **Evidence.** As that section.
- **Parallel.** With WinUI packages.

## Builder briefs

Paste the common preamble of [ui-track.md](../ui-track.md#common-preamble), then the package's brief. Decision ids come from LNX-80 to LNX-89 as the package lists them, with sub-ids for anything further.

### WP-GTK-3a Print Preview (full brief)

```text
Package WP-GTK-3a, Print Preview in the GTK client. Branch feat/gtk-print-preview, worktree
~/source/worktrees/quill/gtk-print-preview, from origin/main.

Read first: docs/plans/gtk-print-help.md (all of "Print preview", "Printing tests" for the display rules, and
WP-GTK-3a), SPEC.md "Printing and preview" and D100, decisions LNX-1, LNX-2, LNX-7, LNX-15, LNX-25;
crates/quill-paint/src/page.rs (paint_sheet: what it draws and where) and src/pixel/mod.rs (Scale,
PixelBuffer); crates/quill-app/src/print.rs (set_print_date, set_print_time, field_text);
linux/quill-gtk/src/window.rs (the view stack near line 306, run_command's Print, Preview and PrintSetup toast
near line 1078), src/page.rs (how the page texture is made at the device scale) and src/main.rs (options).

Goal: File > Print Preview (and the toolbar's Print Preview) shows one or two whole sheets, drawn by paint_sheet
on the pixel surface at the window's device scale, in place of the page. Print and Print Setup stay toasts.

Build, one commit each where it makes sense:

1. src/preview.rs, State as data: page (1-based), two (bool), zoom (Whole or Actual), and fn key(&mut self,
   key, pages) and fn button(&mut self, which, pages) returning what to do (redraw, print, close). Page Up and
   Page Down step one sheet (two in two-page mode), Home and End go to the ends, T toggles one or two pages,
   P asks to print, Esc closes. Two pages show sheets n and n+1 side by side; at the last sheet only one.
2. The preview widget: a texture of exactly device size like the page's, sheets centred on the desk colour the
   page uses, Whole fitting the height (and both sheets' width), Actual at 100 percent with scroll bars. Draw
   each sheet with paint_sheet into a PixelBuffer at the window's Scale; before drawing set the print date and
   time (glib::DateTime::now_local() "%x" and "%X", converted to Windows-1252 with '?' for anything outside
   it), or the values of --print-date and --print-time.
3. The bar, under the header bar while the preview shows: Print, Next Page, Previous Page, Two Pages (labelled
   One Page while two show), Zoom and Close, flat buttons with tooltips and the Windows build's status
   sentences in the status bar; the status bar's page field says "Page N of M". The toolbar, formatting bar
   and ruler hide while it shows and come back after.
4. window.rs: a "preview" page in the existing adw::ViewStack; Command::Preview shows it; Close and Esc go back
   to the page with the scroll position it had; P and the bar's Print give the existing Print toast. Keys go to
   State only while the preview shows. --preview [N] opens the preview at sheet N for tests and shots.
5. Tests, new tests/preview.rs: State over every key and button at the ends (one sheet, two sheets, an odd
   count); the preview texture equals paint_sheet on the pixel surface at scales 1 and 2 byte for byte for
   tests/docs/letter.rtf and sheet 2 of tests/docs/print3.rtf; a recording Surface (a test helper that logs
   glyph runs) proves the header, footer and "2" and "3" fields are drawn on sheet 2 of print3.rtf; under your
   own Xvfb (env -u WAYLAND_DISPLAY DISPLAY=:<n> GDK_BACKEND=x11, a private session bus, scratch HOME and
   XDG_RUNTIME_DIR, GSETTINGS_BACKEND=memory) xdotool presses Page Down, T, Esc and the program's output shows
   the state after each.
6. tools/preview-shots.sh: its own Xvfb, stopped by its PID; shots at GDK_SCALE 1 and 2, light and dark.

Must not touch: crates/** (if paint_sheet cannot draw a sheet the preview needs, stop and report), assets/**,
dos/**, win16/**, win9x/**, winui/**, the root Cargo.toml and Cargo.lock, .github/**, other packages' rows,
SPEC.md beyond the Linux desktop row. The paper in the preview is always white; do not anticipate dark paper.

Finish line checks (all must hold): make gtk passes; tools/frames.sh is still 100.000 % at scales 1 and 2;
tests/preview.rs passes with the byte-for-byte comparison at both scales; the xdotool run is in the test log.

Evidence into ~/Documents/Liminal HQ/Quill/GTK4/WP-GTK-3a/: one page and two pages, Whole and Actual, light and
dark, scales 1 and 2. Commit docs/media/gtk-preview-two-light.png.

Docs in the same change: docs/linux.md a "Print preview (G6, part 1)" section (what it does, the keys, what was
and was not proved) and the work list's G6 row marked "Preview built"; docs/progress.md a "GTK4 print preview"
row; docs/decisions.md LNX-85 from the plan, plus LNX-85.1 and on for any choice of your own; SPEC.md's Linux
desktop row if its wording of print changes. PR title suggestion: "Show print preview in the GTK client".
```

### WP-GTK-3b Export as PDF

```text
Package WP-GTK-3b, Export as PDF in the GTK client. Branch feat/gtk-export-pdf, worktree
~/source/worktrees/quill/gtk-export-pdf. Start only when crates/quill-pdf and crates/quill-pdfdoc are on main
(pull request #167, WP-WEB-3); if not, stop and report.

Read first: docs/plans/gtk-print-help.md ("Printing: the design", "Fonts and layout parity", "Pictures",
"Printing tests", WP-GTK-3b), decisions LNX-1, LNX-25, LNX-80, W9X-90 to W9X-94, D186, D187;
crates/quill-pdfdoc/src/lib.rs (export, Options, how it lays out with the measure it is given) and measure.rs
(Std14Measure: not for GTK); crates/quill-pdf/src/lib.rs (Info, finish); win9x/quill-win9x/src/pdf.rs (how the
Win32 shell calls export: copy its shape); crates/quill-paint/src/page.rs (paint_sheet, for the tests'
recording surface); linux/quill-gtk/src/choose.rs, window.rs (where the OutlineMeasure lives), menu.rs.
Recheck every name here against what merged; if export's signature or Options differ, follow the merged code
and say so in the PR.

Build: (1) src/pdf.rs, no GTK in it: PdfJob { title, created, from, to } and
document_pdf(app, store, measure: &OutlineMeasure, job) -> Vec<u8>, which clones the measure (the window's own,
at 96, with the window's FontSet: never Std14Measure, which would break lines differently from the screen) and
calls quill_pdfdoc::export(app, store, &mut measure, &Options { title, created, from, to, image_dpi: 300 }).
Nothing else draws into the PDF. (2) The window sets the print date and time before export, as the preview
does (glib::DateTime::now_local() "%x" and "%X" in Windows-1252, or --print-date and --print-time). Title: the
document's \info title, else its file name; created: UTC now as YYYYMMDDHHMMSS, or None under --fixed-date or
with SOURCE_DATE_EPOCH set. (3) choose.rs: export_pdf in Chooser, GtkChooser and ScriptChooser (the document's
folder, its stem with .pdf, one PDF type). (4) window.rs and menu.rs: "Export as PDF..." after Print Setup...
in File as win.export-pdf, no accelerator; write the bytes with the safe-save path Save uses; on failure
"Quill cannot write <name>." as a toast. (5) main.rs: --export-pdf OUT (no window needed), --fixed-date.
(6) Cargo.toml: quill-pdfdoc by path (quill-pdf only if you name its types); commit the lockfile change.

Tests (new tests/pdf.rs; none needs a display): golden PDFs tests/golden/pdf/letter.pdf, print3.pdf,
headers.pdf, pictures.pdf with --vendored-fonts and no date, byte for byte, QUILL_UPDATE_GOLDEN=1 rewrites;
pdftotext -layout per page against the layout's text per page, with headers, footers and the page fields;
pdftotext -bbox-layout word left edges equal the pen x a recording Surface sees through paint_sheet, times
0.75, within 0.05 pt; pdfinfo's page count equals the preview's sheet count and each page's first and last
words equal paint_sheet's for every file in tests/docs; qpdf --check and the page size; pdftoppm -r 96
against paint_sheet outside the text boxes (underlines, picture pixels within 8 levels); Export through
ScriptChooser writes document_pdf's bytes; a read-only folder gives the message. Skip a test with a printed
note only when its tool is missing from the host.

Must not touch: crates/** (quill-pdf and quill-pdfdoc included: embedded fonts are S1, sharper pictures S3, a
page list S6; a disagreement between paint_sheet and export goes to quill-pdfdoc's owner with the file, page
and word, and its test stays, #[ignore] with the reason), assets/**, and the rest of the common list.
Decisions: LNX-80.1 and on, LNX-83, LNX-84, LNX-86 from the plan. Evidence (GTK4/WP-GTK-3b/): pdftoppm pages
beside the screen, pdftotext output, the chooser under headless Weston; commit
docs/media/gtk-pdf-beside-screen.png. PR title suggestion: "Export a PDF with a text layer from the GTK
client".
```

### WP-GTK-3c Print

```text
Package WP-GTK-3c, Print in the GTK client. Branch feat/gtk-print, worktree ~/source/worktrees/quill/gtk-print.
Starts after WP-GTK-3b has merged.

Read first: docs/plans/gtk-print-help.md ("Printing: the design" pages and settings, "Flatpak and the
portal", "Printing tests", WP-GTK-3c), LNX-80 to LNX-86; quill_pdfdoc::Options (from and to are the only page
choice it has); gtk4-rs 0.11 PrintDialog (setup, print, print_file), PrintSetup, PrintSettings, PageSetup,
PaperSize; linux/quill-gtk/src/pdf.rs and preview.rs.

Build: src/print.rs: a Printer seam (setup and print) with GtkPrinter over gtk::PrintDialog and ScriptPrinter
(--script-printer FILE writes the stream it would print and answers setup from the file's first line);
PageChoice as data from PrintSettings: all, current (the caret's sheet) or one range become PdfJob's from and
to and the settings are reset to all pages before printing; several ranges, even or odd, or reverse keep the
whole document and leave the settings for the print system (S6 will let Quill write them). Seed the dialog
with the document's paper (a named PaperSize when the twips match one, else custom) and orientation, and with
the session's last PrintSetup. Print: setup, then document_pdf for the choice, then print(setup) and write
the bytes to the stream in chunks from idle callbacks; toast "Printing LETTER.QLD on <printer>." when done;
an error from GTK as a toast. Print Setup...: setup alone, kept for the session. Wire Command::Print,
Dialog::Print, Ctrl+P, the toolbar's Print and the preview's P.

Tests (new tests/print.rs, tools/print-check.sh): PageChoice over a six-page document built in the test; the
seam's stream equals document_pdf for the choice; under your own Xvfb (env -u WAYLAND_DISPLAY DISPLAY=:<n>
GDK_BACKEND=x11, a private session bus, scratch HOME and XDG_RUNTIME_DIR) with xdotool, the real dialog's
Print to File writes the export's bytes (or the same pdftotext and page count: then say so); a 2-3 range
gives two pages (no double range); 1-2,5 gives what the print system makes of the whole document, recorded;
with GDK_DEBUG=portals, a private session bus running /usr/lib/xdg-desktop-portal and
/usr/lib/xdg-desktop-portal-gtk with XDG_CURRENT_DESKTOP=GNOME on the same Xvfb, the same Print to File
check, best effort and reported either way. Never touch the host's CUPS queues; if a local CUPS is wanted,
run it in Docker.

Must not touch: as WP-GTK-3b. Decisions: LNX-81, LNX-82 and sub-ids for what the dialog actually did (whether
settings changes in the PrintSetup stick, whether GTK rewrites the file, what the print system does with
several ranges). Evidence (GTK4/WP-GTK-3c/): the dialog seeded with Letter and with A4, the portal dialog if
it ran, pdfinfo of the printed file; commit docs/media/gtk-print-dialog-light.png. PR title suggestion:
"Print from the GTK client through GTK's print dialog".
```

### WP-GTK-8 Help viewer

```text
Package WP-GTK-8, Help viewer in the GTK client. Branch feat/gtk-help, worktree ~/source/worktrees/quill/gtk-help.

Read first: docs/plans/gtk-print-help.md ("The help viewer", WP-GTK-8), SPEC.md "Help" and "Help source
format", crates/quill-help/src/qhm.rs and write.rs (Help, Topic, Para, Run, Style), assets/help/quill.qhm,
linux/quill-gtk/src/window.rs (Command::Help's toast), src/dialogs.rs.

Build: (1) src/help.rs, no widgets: the help parsed once from include_str!("../../../assets/help/quill.qhm")
with quill_help::qhm::parse; markup for a paragraph (text escaped; bold as <b>; jumps as <a href="jump:ctx">;
pop-ups as <a href="popup:ctx">); History (back and forward, cap 64); search(query) ranked by title prefix,
title words, then body text, case-folded; topic_for_dialog(name) returning None for every dialog today.
(2) src/help_window.rs: one adw::Window per application, 480 by 640, title "Quill Help — <topic>"; header bar
with Back (Alt+Left), Forward (Alt+Right), Contents (Alt+Home), a search toggle (Ctrl+F); ScrolledWindow,
adw::Clamp 640, a Label per paragraph (wrap, xalign 0, selectable, markup; headings title-2 with the heading
role; items a bullet and a hanging indent); activate-link opens jumps and shows pop-ups in a Popover pointed
at the click or the label; SearchBar and SearchEntry with key capture, results in a ListBox, Enter opens the
first, Esc ends the search; Ctrl+W and Esc close. (3) window.rs: Command::Help and F1 present the window at the
contents, F1 with a dialog focused at topic_for_dialog's answer or the contents. (4) main.rs: --help-topic CTX.
(5) Cargo.toml: quill-help by path, the lockfile.

Tests (tests/help.rs): every topic opens by --help-topic under your own Xvfb with the title checked; every
jump and pop-up resolves (over the model, no display); search finds every topic by its own title; History;
F1 from the window and from Find; Tab reaches each link of the contents and Enter follows it (xdotool); the
release binary, stripped, grows by at most 150 KB (measure before and after, write both numbers in the PR).
If WP-GTK-5's tools/atspi-read.py is on main, read the heading and links over AT-SPI; otherwise say so.

Must not touch: crates/quill-help/**, assets/help/** (missing dialog topics are S5: list them in the PR), and
the common list. Decision LNX-87. Evidence (GTK4/WP-GTK-8/): contents, a topic, a pop-up, a search, light and
dark, scales 1 and 2; commit docs/media/gtk-help-contents-light.png and gtk-help-popup-dark.png. PR title
suggestion: "Open Quill's help in the GTK client".
```

### WP-GTK-10 Dark paper

```text
Package WP-GTK-10, Dark paper in the GTK client. Branch feat/gtk-dark-paper, worktree
~/source/worktrees/quill/gtk-dark-paper. Starts after WP-GTK-4 has merged.

Read first: docs/plans/gtk-print-help.md ("Dark paper", WP-GTK-10), docs/plans/preferences.md (the View page's
PaperColour row, WP-PREF-10), decisions LNX-5, LNX-7, LNX-15 and WP-GTK-4's rows; crates/quill-paint/src/lib.rs
(Surface) and page.rs (which calls draw the paper, text, selection, caret, waves, pictures); linux/quill-gtk
src/page.rs and the colour wrapper WP-GTK-4 added for the accent.

Build: src/paper.rs: the colour table of the plan with integer arithmetic only (WCAG contrast from 8-bit sRGB
through a 256-entry linear table; HSL lightness turned over; raise until 4.5:1, or 7:1 in high contrast) and
ThemedSurface<S: Surface> mapping by the kind of call (paper fill, text and lines, other fills, edge;
selection, invert_rect, rgb_row and bitmap4 untouched), extending WP-GTK-4's wrapper rather than adding a
second one. page.rs wraps the editing page's surface only when PaperColour is theme and the style is dark,
and repaints when either changes. settings.rs keeps [Window] PaperColour=white|theme in gtk.ini. View > Dark
Paper (win.dark-paper, after Status Bar), greyed in the light style. --paper white|theme for tests.

Tests (tests/paper.rs): the table for the sixteen Windows colours and the automatic colour, each mapped
colour at 4.5:1 (7:1 in high contrast); the wrapper over a recording surface for a selected word, a picture
and a wave; golden tests/golden/gtk/letter.1.00.darkpaper.png; with theme and --dark, the preview texture and
document_pdf's bytes equal their white goldens; the gtk.ini round trip. Evidence (GTK4/WP-GTK-10/) from
WP-GTK-4's headless GNOME compositor run (mutter --headless --virtual-monitor with WAYLAND_DISPLAY set to its
socket and DISPLAY unset): white and dark paper in the dark style, high contrast, a selection and a picture
on dark paper, the preview still white, scales 1 and 2. Decision LNX-88. PR title suggestion: "Offer dark paper
in the GTK client's dark style".
```

## Owner decisions

Each is taken with the default so no package waits; the last column is where to change it.

| Question | Default taken | Where to swap |
|---|---|---|
| Print through `gtk::PrintDialog` with Quill's PDF, or through `gtk::PrintOperation` drawing with cairo? | `PrintDialog` with `quill_pdfdoc::export`'s bytes: the paper is the export | LNX-80, LNX-81; WP-GTK-3c |
| The PDF's fonts before embedding lands: the standard 14 at the layout's positions, or wait for embedded faces? | Ship the standard 14 now; embed when S1 lands | LNX-83; S1 (WP-PDF-1) |
| Printer paper unlike the document's: fit by the print system, or lay the document out again as Windows does? | Fit by the print system; the document's paper is the document's | LNX-82 |
| Lay the PDF out with the window's `OutlineMeasure` (lines as the screen breaks them) or with `Std14Measure` (no font files, the standard fonts' own widths)? | The window's `OutlineMeasure` at 96 | LNX-80; `src/pdf.rs` |
| Several ranges, even or odd pages and reverse order: written by Quill, or left to the print system? | Left to the print system until `quill-pdfdoc` takes a list of pages (S6) | LNX-81 |
| A Selection choice in Print? | None (`PrintDialog` has no switch) | LNX-81 |
| Export as PDF…: its name, place and accelerator | "Export as PDF…" after Print Setup…, no accelerator | LNX-86; S4 for the shared command |
| Help: an in-app window, or Mallard pages for Yelp? | In-app window over the `.qhm` model | LNX-87 |
| Help menu: add Search for Help On… and How to Use Help on GTK? | No; search is Ctrl+F in the help window, and the shared tree keeps Contents and About | LNX-87; the shared menu tree |
| Dark paper: a View > Dark Paper item before Preferences exists? | Yes, kept in `gtk.ini` until WP-PREF-10 | LNX-88; WP-GTK-10 |
| Pictures on dark paper: as decoded, or dimmed? | As decoded | LNX-88's table |
| Print Preview on dark paper? | Always white, since it previews paper | LNX-85, LNX-88 |
| Help before Export as PDF when pull request #167 is late? | Yes, WP-GTK-8 takes the slot | [Order](#order) |
