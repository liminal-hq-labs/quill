I recommend a native GTK4 client, but the codebase is not quite what the brief assumes, so the plan starts with what is there.

**What the code has today.** I checked the newest work branches (PR #15 and PR #11), not just `main`:

- **There is no `Gui` or `Surface` trait.** The only public traits are `Store`, `Measure`, `Files`, `Source`, `Sink` and `Pod`. `Shell` is a plain struct in `win9x/quill-win9x/src/main.rs`.
- **The real-font page painter belongs to the Windows build.** `quill-app` only draws the text grid. The page painter is `win9x/quill-win9x/src/gdi.rs` (`paint_page`, `draw_line`), built on `App::for_each_line` and `walk_chars`. `Surface` is still a plan (D31, D79), and D79 says to add it when its second user arrives. The Linux client is that second user.
- **The shared view code assumes 96 dpi.** PR #11 adds `crates/quill-app/src/view.rs` (D95), whose page geometry is hard-wired to 96 dots per inch. `pointer.rs` (PR #12) works at any resolution.
- **There is no system clipboard yet.** `App` keeps its own 2 KB text clipboard, and nothing calls the Windows clipboard.
- **Saves truncate the file in place.** `Win32Files::create` opens with `CREATE_ALWAYS` and `close` returns `true`. The SPEC's write-a-temporary-then-replace save is not built (D66 deferred it).
- **The emergency save to `~QUILL01.QLD` is not built.** The Win32 panic handler just calls `ExitProcess(3)`, and the DOS one prints and exits.
- **`modified` is a plain flag.** Undoing back to the saved state still shows the document as changed.
- **Text measurement works through width tables.** `Measure` gives 224 integer advance widths per font, with no kerning or shaping. That is why one engine can measure and draw identically, and why Pango-shaped runs would bring back the Windows 95 caret drift.

## Recommendation

1. **Today: a terminal client (`quill-tty`).** It reads and edits every `.QLD`, RTF and text file this week, and its screens match the DOS goldens exactly. It is one pull request with no conflicts.
2. **The real client: GTK4.** Use gtk4-rs 0.11 with optional libadwaita. The page is drawn by Quill's own pure-Rust renderer: `skrifa` for metrics and outlines, `tiny-skia` for pixels. It renders at the exact device-pixel size and is handed to GTK as a texture. GTK supplies only the window, menus, dialogs through the portal, IME, clipboard, accessibility and dark mode.
3. **Also, without waiting:** a viewer/converter CLI (`cat`, `to-rtf`, `to-html`, `to-pdf`), a thumbnailer and the `.desktop` file, all reusing PR #10's MIME files.
4. **Wine stays a documented stopgap only.** It cannot be made crisp because Quill hard-codes 96 dpi (below).

**Why one renderer for measuring and drawing.** The Windows 95 bug came from measuring widths through a scaled mapping mode, so the widths and the drawn glyphs disagreed. Here the widths and the outlines come from the same font file with no hinting, and the same function places the glyphs, the caret and the hit tests. Because the renderer is pure Rust and independent of fontconfig and cairo versions, golden pixel tests behave the same on CI and on your machine. The same renderer gives the thumbnailer, PDF export, print preview and later the shared `Surface` painter that Desk needs.

## The four options

| | (1) GTK4 + own renderer | (2) Terminal (`quill-tty`) | (3) CLI, thumbnailer, MIME | (4) Wine with a DPI recipe |
|---|---|---|---|---|
| **Effort** | About 12 pull requests to full editing (G0–G8 plus core) | 1 pull request | 3–4 pull requests | A doc page |
| **Fidelity to the core** | Same `App`, commands and files. Line breaks come from the real font metrics at 1440 units an inch, so screen, PDF and print agree (better than Windows 95, D81). An optional Windows-metrics mode reproduces Windows line breaks | Exact: the same `Grid` as DOS, compared character for character | Exact for files. Thumbnails and PDF use the GTK client's renderer | It is the Windows 95 build itself |
| **Fractional scaling on Wayland** | Correct: renders at exact device pixels (to be confirmed by spike G0) | Not affected (the terminal draws) | Not applicable | Poor: blurry or the wrong size, because Quill assumes 96 dpi |
| **Fonts** | fontconfig finds the installed Times New Roman, Arial and Courier New (they are installed here), with Liberation as fallback | One cell font | Same as (1) | Wine's fonts |
| **IME and clipboard** | `GtkIMMulticontext` (compose keys, dead keys, IBus); clipboard needs core work C1 | IME is the terminal's (UTF-8 converted to Windows-1252); internal clipboard only | Not applicable | Wine's |
| **File dialogs** | `GtkFileDialog` (portal inside Flatpak), or ashpd 0.13 | Quill's own text prompt | Not applicable | Wine's dialogs |
| **Accessibility** | GTK4 `GtkAccessibleText` (not verified) | Screen readers read the terminal | Not applicable | Almost none |
| **Packaging** | AUR, Flatpak (`org.gnome.Platform//51`) | AUR, same package | AUR, same package | Not applicable |
| **Fit with Desk and a shared painter** | Creates the `Surface` seam Desk needs (D79) | Proves `App` plus `Grid` on a third host | Shares the renderer | None |

**Other toolkits:**

- **Qt via cxx-qt:** the best fractional scaling and accessibility, but a C++ build, foreign on your GNOME desktop, and QPainter's shaping must be bypassed anyway. No.
- **iced and egui:** pure Rust and fine at fractional scales (egui has AccessKit), but no native menus or portal dialogs without extra crates, and a weaker IME. egui is the fallback if GTK4 fails the spike.
- **slint:** would only work by rendering into an image buffer; awkward for a custom page. No.
- **Tauri with a canvas:** WebKitGTK plus pushing pixels or JavaScript text per frame. No.
- **winit + softbuffer + tiny-skia:** maximum control, but menus, dialogs, accessibility and clipboard all become Quill's job. That duplicates the Desk port. It stays possible later because the renderer is independent of the toolkit.

## GTK3 versus GTK4

What I could check on this machine:

- **Arch ships** gtk3 3.24.52, gtk4 4.24.1, libadwaita 1.10.0, pango 1.58.2 and cairo 1.18.6.
- **GTK3's Rust bindings are unmaintained.** The `gtk` 0.18.2 crate describes itself as "UNMAINTAINED Rust bindings for the GTK+ 3 library (use gtk4 instead)", and its README says the same.
- **GTK4's bindings are current.** `gtk4` 0.11.5 has a `v4_24` feature matching Arch's GTK and needs Rust 1.92 (the project uses 1.98). glib, cairo-rs and pango are on 0.22, and libadwaita 0.9.2.
- **Fractional scaling:** `libgtk-4.so` contains `wp_fractional_scale_v1` and the header has `gdk_surface_get_scale` (marked available since 4.12). `libgtk-3.so` has neither. GTK3 draws at the next whole scale (2×) and the compositor shrinks it to your 1.5×: slightly soft text and about 78 % more pixels drawn.
- **IME:** both have `zwp_text_input_v3`, so input methods work in both.
- **Flathub** offers `org.gnome.Platform` 49, 50 and 51 (all installed here), KDE 6.10 and 6.11, and freedesktop 25.08 and 26.08.
- **Your session** is GNOME on Wayland at scale 1.5, with `prefer-dark` set.

What I could not check:

- Whether GTK4 shows a texture of exactly device size one-to-one at 1.5× without resampling. Spike G0 settles this.
- Whether a cairo drawing area in GTK4 is sharp at fractional scales.
- The exact GTK version that added `GtkAccessibleText` (I believe 4.14), and whether gtk4-rs exposes it for implementing.
- Whether the GNOME runtimes still include GTK3.
- Whether `GtkFileDialog` uses the portal outside Flatpak on GNOME.
- How mature mnemonics are in `PopoverMenuBar`.

**Trade-off.** GTK4's real limits are:

- **Menus are popovers.** The menu bar is a `PopoverMenuBar` built from a `GMenuModel`, not a classic menu bar.
- **Its file and font dialogs are newer async APIs.** The old ones are deprecated since 4.10.
- **There are no window positions on Wayland,** but that is equally true of GTK3.

GTK3's advantages are a classic `GtkMenuBar` with mnemonics, which suits a Write-style app, and the mature ATK accessibility. Against that: bindings frozen on the glib 0.18 line (I inferred this from version pairing and did not check), a toolkit only getting fixes, and no fractional scaling on exactly your 1.5× display.

**GTK4.** "Limited support" is true of GTK3's Rust side, not of GTK4.

## Architecture

**Where the crates go.** The new pure-Rust crates live in the root workspace and are tested by the existing CI image. The GTK crate gets its own workspace, as `win9x/` and `dos/` do (D30), so root CI never needs GTK headers.

| Crate | Kind | Holds |
|---|---|---|
| `crates/quill-fonts` | std | Font loading from bytes. `OutlineMeasure: Measure` built on `skrifa`. The family fallback (roman → Times New Roman → Liberation Serif; swiss → Arial → Liberation Sans; modern → Courier New → Liberation Mono; Desk faces if installed). Glyph rasterizing: skrifa outlines into tiny-skia paths, giving alpha masks cached by font, pixel size and quarter-pixel x offset |
| `crates/quill-raster` | std | `Mapping` (layout units to device pixels) and `render_page(app, store, mapping, rect) -> Pixmap`, following `gdi.rs::draw_line`: runs, bold, italic, underline, strikethrough, super and subscript, colours, tab leaders, `line_label`, selection, caret, sheets, pictures through `quill-image`, headers and footers in preview. It becomes the shared `Surface` painter in C3 |
| `crates/quill-cli` | std | The `quill` subcommands `cat`, `convert` (replacing `quill-convert`), `info`, `to-html`, `to-pdf`, plus the `quill-thumbnailer` binary (no GTK, so it starts fast in the thumbnailer sandbox) |
| `linux/quill-tty` | std, crossterm | Terminal client |
| `linux/quill-gtk` | gtk4-rs (+ libadwaita), binary `quill` | Window, `QuillPage` widget, menus, dialogs, IME, clipboard, printing |
| `linux/data/` | — | `.desktop`, `.thumbnailer`, AppStream `metainfo.xml`, icon install map, Flatpak manifest, PKGBUILD |

**Measuring and drawing with one engine:**

1. **Layout resolution:** `OutlineMeasure::resolution()` returns `(1440, 1440)`, so layout units are twips.
2. **Widths:** advance = round(advance in font units × 10 × size in half points / units per em). Even a 144-point glyph fits in `u16`, and the rounding error is under 1/2880 inch per glyph.
3. **Vertical metrics:** ascent and descent come from the OS/2 table's `usWinAscent` and `usWinDescent`, and leading from the `hhea` table as GDI derives it. GDI's `tmAscent` and `tmDescent` come from those values, so line heights match Windows.
4. **Never hinted:** advances never depend on pixel size, zoom or scale. This removes the Windows 95 failure by construction.
5. **One mapping:** `Mapping { zoom_pct, scale_120 }` turns layout units into device pixels as x × zoom × 96 × scale_120 ÷ (1440 × 100 × 120), in `i64`. Wayland reports scales in 1/120ths, so GTK's `f64` scale becomes an integer `n` in the shell; the float ban only covers portable crates. Glyphs, the caret, the selection, hit tests and the ruler all go through `Mapping`. The reverse (pixel to layout) feeds `pos_at_point`.
6. **Missing styles:** if there is no real bold or italic file, the same function adds synthetic emboldening to both the measured advance and the drawn outline.
7. **A Windows check mode:** an optional `[Quill] Metrics=windows` uses `CapturedMeasure` for layout. Glyphs are placed at its positions, which looks slightly uneven but reproduces Windows ME line breaks for comparison.

**How the page is drawn:**

1. `QuillPage` (a GTK4 widget subclass) gets device width = logical width × scale.
2. `quill-raster` renders the visible strip in 512-pixel tiles cached by layout revision. Typing re-renders only the lines that changed.
3. The result goes to GTK as a `gdk::MemoryTexture` via `snapshot.append_texture` at logical bounds.
4. Fallback if spike G0 shows resampling: cairo with a FreeType face from the same file, drawing glyphs one by one at `Mapping` positions. Measurement stays in skrifa.

**Rule for the page:** never use Pango layout. Pango is used only for the GTK chrome.

**Shell responsibilities** (`QuillShell` is the GTK equivalent of the Win32 `Shell` struct):

- **Files:** `Files` over `std::fs`. `create` writes `.name.~tmp` and `close` fsyncs and renames, which gives the Linux build the safe save without touching the core.
- **Names:**
  - The app sees the base name converted to Windows-1252, which is what the title, the `[File]` field and the type-by-extension rule need.
  - The shell keeps the real `PathBuf`.
  - Recent files go in `[RecentLinux]` in `$XDG_CONFIG_HOME/quill/QUILL.INI`, same INI format, and are also registered with GTK's recent-files list.
- **Keys:** GTK key events go through the IME filter first.
  - Shortcuts go through `quill_app::accel`.
  - IME commits are UTF-8, converted to Windows-1252. A character with no Windows-1252 form is refused with a status message (default; to decide).
  - The candidate window is placed from the caret rectangle given by `Mapping`.
- **Mouse:** uses `pointer.rs` (click, drag, double and triple click, left margin).
- **Menus:** a `GMenuModel` from `menu_items` and `menu_title`, one action per `Command`. Enabled and checked state refresh after each command. It picks up mnemonics once the menu-keyboard work lands.
- **Dialogs:** `set_native_dialogs(true)`, then `take_dialog()`:
  - Open and Save As: `GtkFileDialog`.
  - Save prompt: `AdwAlertDialog`.
  - About: `AdwAboutDialog`.
  - Find, Replace, Fonts, Tabs and Spelling: a generic builder from `assets/dialogs/dialogs.json`, as D86 intends.
- **Clipboard:** `GdkClipboard` offering `text/rtf`, `application/rtf`, `text/plain;charset=utf-8`, and `image/png` for a lone picture. This needs core work C1.
- **Printing:** `quill-raster` draws the preview. Printing writes a PDF and hands it to the print portal through ashpd (or `GtkPrintOperation`; to decide).
- **Text files:** Linux `.txt` files are usually UTF-8. Add a UTF-8 text option in `quill-formats` (C4): read UTF-8 when valid or marked, and offer "Text Files (UTF-8)" on Save As, as DOS does with CP437.

**How the files stay byte-for-byte the same.** Every client links the same `quill-formats` and `quill-core` with `HeapStore`. `.QLD` and RTF go through the existing readers and writers with no Linux-specific code. The test: play the same script on the host, the terminal client's event mapper and the GTK shell's command layer, save, and compare bytes with `tests/golden/files/*`.

**Fit with Desk.** C3 defines `Surface` with only what Desk has: `fill`, `line`, `text` with a position for each character, and `blit` with palette indices. Win32, Win16, `quill-raster` and a future `quill-desk` all implement it. `OutlineMeasure`'s host tests are also the pattern for `DeskMeasure`'s host twin.

**Desktop integration** (reuses PR #10):

- **Desktop entry:** `Type=Application`, `Name=Quill`, `GenericName=Word Processor`, `Exec=quill %F`, `Icon=<app-id>`, `MimeType=application/x-quill-document;application/rtf;text/rtf;text/plain;`, `Categories=Office;WordProcessor;`.
- **Thumbnailer:** `/usr/share/thumbnailers/quill.thumbnailer` with `Exec=quill-thumbnailer -s %s %i %o` and `MimeType=application/x-quill-document;` only, so LibreOffice keeps RTF thumbnails.
- **MIME:** install PR #10's `assets/mime/quill.xml` as is, plus one line, `<generic-icon name="x-office-document"/>`.
- **Icons:** `assets/icon/quill-{16…512}.png` and `quill.svg` go into hicolor.
- **libmagic:** `quill.magic` stays a documented per-user `~/.magic`. A package should not edit `/etc/magic`.

**Packaging:**

- **AUR:** `quill-git`, depends on `gtk4` and `libadwaita`, optionally on `ttf-liberation`. It builds `linux/` and the CLI with `--locked`.
- **Flatpak:** `org.gnome.Platform//51` with `--socket=wayland --socket=fallback-x11 --share=ipc --device=dri`. No network and no filesystem access: files come through the portals, which matches the SPEC's never-opens-a-socket rule. Crates are vendored with `flatpak-cargo-generator`.
- **AppImage:** skip; bundling GTK4 is fragile.
- **Releases:** there is no release workflow yet (only `ci.yml`). Add one that builds a tarball and a Flatpak bundle on a tag. It needs a CI image with GTK4 development files; I don't know if `tauri-ci-desktop` has them.

**Wine stopgap.** None of these is verified on your machine. Your Wine prefix has `LogPixels`=96 and no Graphics driver set. Wine 11.21's Wayland driver mentions fractional scale.

- **(a) Leave DPI at 96 and let the compositor scale the window:** the right size, but blurry. This is the recommended stopgap.
- **(b) Raise DPI to 144 in `winecfg` with the Wayland driver** (`HKCU\Software\Wine\Drivers` `Graphics=wayland,x11`): menus and dialogs grow, but the page and toolbar stay at Quill's fixed 96 dpi. Pair it with View > Zoom 150 %.
- I could not read GNOME's experimental mutter features, which likely explain the wrong size you see.

## Tests

- **Layout dumps:** text goldens for `OutlineMeasure` at 1440 dpi.
- **A comparison report:** Liberation and Times New Roman widths against `tests/metrics/screen96-windows-me.txt` at 96 dpi, saying which lines of `letter` break differently. A report, not a failure.
- **Property tests,** in addition to the core's existing ones (`quill-core/tests/props.rs`, `quill-host/tests/editing.rs`):
  - For random documents at scales 1.0, 1.25, 1.5, 1.75 and 2.0 and zooms 50–200 %, the caret's x at every character boundary equals the next glyph's drawn x.
  - `pos_at_point` of that position returns the same character.
  - Mapping round trips stay within one pixel.
- **Pixel goldens:** render `letter`, `lists`, `headers` and `pictures` to PNG with fonts vendored for tests (Liberation, Latin-1 subset, licence kept). Pass when every pixel is within 2 of the golden and no more than 0.1 % of pixels differ. Update with `QUILL_UPDATE_GOLDEN`.
- **The terminal client:** the existing `tests/golden/screens/*.frames.txt`, played through its key mapper, must match exactly.
- **The GTK client:** a headless smoke test that opens a file, saves it and compares bytes; screenshots for `docs/media/`. A Wayland screenshot at 1.5× in CI is optional; whether a headless compositor supports fractional scale is not verified.

## Pull requests

Notes for the whole list:

- **Start now** means only new files plus one line in the root `Cargo.toml` members list (a trivial conflict).
- **Decision numbers:** D90–D96 are taken across the open branches. Reserve D100–D119 for this work so nothing collides.
- **SPEC edits:** adding Linux rows to the SPEC's Platforms and Components tables is safe now, because the open stack's SPEC changes are elsewhere. New rows in the Settings table should wait (#13 and #15 touch nearby lines).

| PR | Contents | Acceptance | Start |
|---|---|---|---|
| P1 | `docs/linux.md` (this plan), reserve D100+, SPEC Platforms and Components rows | markdownlint passes | Now |
| P2 | `linux/quill-tty`: crossterm main loop; `Grid<25>` or `Grid<50>` by terminal height; `cp437()` per cell; attributes to colours; keys to `Key`; mouse; `Files` over `std::fs` with temp and rename; XDG settings | All `tests/golden/screens` frame scripts match through its mapper; open, edit and save of `letter.rtf` gives the same bytes as the host; documented that Ctrl+I equals Tab in legacy terminals (use kitty keyboard mode or the menus) | Now |
| P3 | `crates/quill-cli`: `cat` (`--ansi` shows bold and italic), `convert`, `info` | Golden stdout for the test documents; `quill convert` output equals `quill-convert`'s | Now |
| P4 | `crates/quill-fonts`: `OutlineMeasure` and the fallback table; vendored subset fonts with OFL text | Layout dump goldens; the Windows ME comparison report; no hinting dependency (a test that widths are identical when asked at any pixel size) | Now |
| C4 | `quill-formats`: Windows-1252 to Unicode table made public; UTF-8 text read and write option | Round-trip property test; invalid UTF-8 falls back to ANSI | Now (no open PR touches `quill-formats`) |
| Q1a | `quill-core` `text.rs`: a step serial and `clean` mark, so `is_clean()` is true again after undoing back to a save | Property test of save, edit, undo, redo; the mark is lost when the saved step falls off the undo ring | Now (no open PR touches `quill-core`) |
| P5 | `crates/quill-raster`: `Mapping` and `render_page` | Pixel goldens with tolerance; caret and glyph property tests | Base on #15 (needs `line_label` and `walk_chars` as there); rebase when the stack merges |
| P6 | `quill-thumbnailer` and `linux/data` (desktop, thumbnailer, metainfo, icon map); `generic-icon` added to `quill.xml` | `desktop-file-validate` and `appstreamcli validate` pass; Nautilus shows a page-1 thumbnail; PNG golden | After P5 and #10 |
| P7a | HTML writer in `quill-formats` (`no_std`, over `Sink`) and `quill to-html` | Golden HTML; LibreOffice opens it with the same counts as the RTF check | Now |
| P7b | `quill to-pdf`: `pdf-writer` with `subsetter`-embedded fonts and a ToUnicode map, glyphs at layout positions | `pdftotext` gives the document's text; page count matches; `pdftoppm` of page 1 within tolerance of `quill-raster` | After P5 |
| G0 | GTK4 spike: window, `QuillPage`, a checkerboard plus a raster page at device scale, IME commit log, `GtkFileDialog` | A screenshot at 1.5× shows a 1-pixel checkerboard with no blur; IBus compose works; written result in `docs/linux.md` | After P5 |
| G1 | Read-only viewer: open (command line and dialog), scrolling, zoom, title, name mapping, XDG settings | Opens every file in `tests/docs`; layout equals P4's goldens | After G0 |
| G2 | Editing: keys, IME, caret blink, mouse through `pointer.rs` | The host's editing scenarios replayed through the GTK command layer save identical bytes | After G1 and #12 |
| G3 | Menus and native dialogs from `dialogs.json` | Every `Dialog` variant is shown and answered (test hook like `native.rs`) | After G2 |
| G4 | Toolbar, formatting bar, ruler (from `view.rs`), status bar | Same commands as the host chrome tests | After G3 and #11 |
| C1 | `quill-app`: `copy_rtf`, `copy_text`, `paste_rtf`, `paste_text` over `Sink` and `Source` (selection copied into a scratch document) | Round trip into LibreOffice; also unblocks the Win32 clipboard the SPEC describes | After the #12–#15 stack merges |
| G5 | GTK clipboard on C1 | Copy from Quill and paste into LibreOffice keeps bold; the reverse too | After C1 |
| C2 | `view.rs` geometry taken from `Measure::resolution()` instead of 96 | Win16 golden still 12 of 12; Win9x Wine screenshots unchanged | After #11 |
| G6 | Print preview; print by PDF through the portal | Preview equals the raster goldens; a PDF reaches the print dialog | After P7b |
| C3 | `Surface` trait and one shared painter extracted from `gdi.rs`, Win16's painter and `quill-raster` | All Windows and raster goldens unchanged | After the stack merges and G1 |
| G7 | libadwaita style, dark mode, accessibility (role, name, `GtkAccessibleText`) | Orca reads the line at the caret (manual check) | After G4 |
| G8 | PKGBUILD, Flatpak manifest, `linux` CI job, release workflow | `makepkg` and `flatpak-builder` succeed; the app installs and opens a `.QLD` from Nautilus | After G3 |

**Milestones:**

- **L0, readable now:** P2 and P3.
- **L1, desktop integration:** P4, P5, P6.
- **L2, viewer:** G0, G1.
- **L3, editor:** G2, G3, G4, C1, G5.
- **L4, print and finish:** P7, G6, G7, G8, C3.

## Part B: modern conveniences

Ordered by priority:

| # | Feature | DOS | Win3.x | Win95 | Linux | Cost on old hardware | SPEC change |
|---|---|---|---|---|---|---|---|
| 1 | **Safe save** (temporary file, then replace); a prerequisite for autosave | INT 21h 56h rename | Rename through DOS3Call | `DeleteFileA` then `MoveFileA` | rename with fsync | One extra directory write. On a full floppy it needs room for two copies, so it fails with the existing "disk is full" message | None (already specified); D66 closed |
| 2 | **Unsaved marker** (from Q1a's clean mark) | `Mod` exists; add `*` after the name in the status line | Title `Quill - LETTER.QLD *` | Same | Title `*Letter.qld – Quill` | One `SetWindowText` per change of state, not per key | Window and chrome, DOS status line |
| 3 | **"Saved" flash** | `Saved LETTER.QLD.` in the status line until the next key, and `Saving LETTER.QLD...` before a slow write | Same | Same | A toast | None | Files |
| 4 | **Word count** (`Document > Word Count...`: words, characters, paragraphs, pages, for the selection or the whole document; same word rules as the spelling scanner) | Dialog | Dialog | Dialog, plus an optional status-bar field updated when idle | Same as Win95 | A whole-document pass of about 1–2 s on a 386 for 1 MB, so on demand only on DOS and Win16 | Menus table, a new item |
| 5 | **Autosave and recovery** (to a recovery folder, as a `.QLD` with an optional `RCOV` chunk naming the original path; offered at the next start with Recover or Discard; deleted on a clean save or exit) | Every 10 minutes when idle for 2 s; off when the folder is on a floppy (IOCTL 4408h); one blocking pause | Same rule, using `GetDriveType`; folder is `TEMP` or beside `QUILL.INI` | Every 5 minutes | Every 2 minutes and when the window loses focus; `$XDG_STATE_HOME/quill` | A 50 KB file to a 386's hard disk takes about 0.2–0.5 s and stalls (no threads), hence idle-only and long intervals. A floppy at about 30–50 KB/s takes seconds, so it is off there | New settings `[Quill] Autosave=` (minutes, 0 off) and `RecoveryDir=`; the optional `RCOV` chunk (only in recovery files); recovery prompt text; Security section gains recovery files |
| 6 | **Emergency save on panic**, using the same slot and prompt | Cleanup hook in `rt/panic.rs` | Panic handler | `rt.rs` | Panic hook and `SIGTERM` | Fixed buffer; may fail if the heap is damaged | None (already specified) |
| 7 | **Spelling as you type** (queued) | Red background | Off by default (300 KB dictionary on a 4 MB machine) | On, wavy line, plus right-click suggestions (Word 95 had them) | On, wavy line scaled for HiDPI, context menu | About 100 ms a visible page on a 486 (the progress report's estimate) | Add the context menu to the Spelling section |
| 8 | **Drag a file onto the window to open it** | — | `DragAcceptFiles` (SHELL.DLL) | Same, plus `SHAddToRecentDocs` | Drop target and GTK's recent files | None | Files |
| 9 | **Colours** | Existing schemes | Paper from Control Panel `COLOR_WINDOW` | Same | Dark chrome by default; optional dark paper for display only (Automatic text already follows the window text colour; explicit colours lightened on screen, never saved) | None | Window section, `[Quill] Paper=` |
| 10 | **Smart quotes** (optional) | Off: CP437 has no curly quotes, so they would show as `▒` | Optional | Optional | On | None | Typing, `[Quill] SmartQuotes=` |

**Too costly on old machines, said plainly:**

- A live word count on Win16 or DOS.
- Autosaving to a floppy, or while the user is typing.
- Background (threaded) saving on DOS or Win16.
- Version snapshots.
- Spelling as you type on by default on Win16.
- Unicode documents (D11).
- Anything networked: sync, sharing, collaboration.

Small extras that are fine everywhere: remember the caret position per recent file (a `File1Pos=` key, which older builds ignore), and a warning when a file is already open in another Quill window (share-mode locks; not on DOS).

**Order:** safe save (1), then Q1a and the marker (2, 3), word count (4), autosave and the emergency save (5, 6), the spelling underline that is already queued (7), then 8 to 10.

**Which parts can start now:**

- **Now:** the core clean mark (Q1a), the `RCOV` chunk in `quill-formats`, and the word counter in the `quill-spell` scanner.
- **After the #12–#15 stack:** everything touching `quill-app/src/lib.rs` or `win9x/main.rs`.
- **Coordinate with the DOS file-dialog plan:** the DOS safe save touches `dos/quill-dos/src/files.rs`.

## Decisions for you (recommended default in bold)

1. **Toolkit:** GTK4 or GTK3. **GTK4.**
2. **libadwaita:** yes or no. **Yes:** dark mode, About and Alert dialogs, toasts, with a classic menu bar kept inside.
3. **Page drawing:** Quill's own renderer or cairo with FreeType. **Own renderer** (deterministic goldens, shared with the thumbnailer, PDF and Desk), with cairo as fallback if G0 fails.
4. **Layout resolution:** 1440 units an inch or 96. **1440**, with an optional Windows-metrics check mode.
5. **App id:** **`io.github.liminal_hq_labs.Quill`**, unless Liminal HQ owns a domain for it.
6. **Binaries:** **`quill` (GUI plus subcommands), `quill-tty`, `quill-thumbnailer`.**
7. **Characters with no Windows-1252 form** (from the IME or a UTF-8 file): **refuse with a status message when typed; `?` when read from a file**, with a count reported.
8. **Plain text on Linux:** **read UTF-8 when valid; Save As offers UTF-8 as well as ANSI.**
9. **Thumbnails for RTF:** **no**, only `.QLD`.
10. **Printing:** PDF through the portal or `GtkPrintOperation`. **PDF through the portal.**
11. **Title marker:** **`*` before the name on Linux, `*` after the name on Windows.**
12. **Autosave intervals:** **DOS and Win16 10 minutes, Win95 5, Linux 2; off on floppies.**
13. **Packaging:** **AUR and Flatpak; no AppImage.**
14. **Vendored test fonts:** **a Latin-1 subset of Liberation 2.1.5.** I did not check whether its OFL has a Reserved Font Name; if it does, the subset must be renamed.
15. **Decision numbers:** **reserve D100–D119.**

## Risks

- **GTK4 may resample the texture at 1.5×.** G0 tests this first; the fallback is cairo, then egui.
- **Layout at 1440 is not yet exercised.** `view.rs` and parts of the painter assume 96, so 1440 may expose integer assumptions in `quill-app`. C2 and property tests at 1440 cover it.
- **Two painters until C3.** The GDI painter and `quill-raster` will drift until C3 merges them. Keep C3 on the plan.
- **Font metric policy.** If OS/2 and `hhea` handling differs from GDI, line heights won't match Windows. The P4 report shows it.
- **IME preedit** in a custom GTK4 widget is untested.
- **Terminals send Tab for Ctrl+I,** and VTE's support for the kitty keyboard protocol is unknown.
- **Inside Flatpak,** recent files depend on the document portal keeping access across restarts.
- **CI** needs an image with GTK4 development files.
- **Autosave on DOS and Win16** is a visible pause by design.

## Verified and not verified

- **Verified from the code** (`main`, PR #15's and PR #11's branches, PR #10's branch, `gh pr list`): everything in the first section, and the PR #10 file contents. No open PR touches `quill-core` or `quill-formats`. The SPEC's Platforms and Components sections are untouched by the open stack. The spelling cost figures come from `docs/progress.md`.
- **Verified on this machine:** package versions, the crate metadata, the strings in the GTK libraries, the Flathub runtimes, the scale of 1.5, `prefer-dark`, that fontconfig resolves Times New Roman, Arial and Courier New, the Wine prefix's DPI, and the Wine Wayland driver's strings.
- **Not verified:** everything listed under GTK3 versus GTK4, the Wine recipes, how the running menu-mnemonic and DOS Ctrl+I work will shape the menu API, and the cost figures beyond the progress report. I built and ran nothing.

### Critical Files for Implementation

- `/home/scott/source/experiments/quill16.wt-win9x-editing/win9x/quill-win9x/src/gdi.rs` (the painter and `GdiMeasure` to mirror, then extract)
- `/home/scott/source/experiments/quill16.wt-win9x-editing/crates/quill-app/src/lib.rs` (`App`, `Files`, `Key`, `Command`, the `modified` flag, the internal clipboard)
- `/home/scott/source/experiments/quill16.wt-win16-gui/crates/quill-app/src/view.rs` (shared geometry fixed at 96 dpi; C2)
- `/home/scott/source/experiments/quill16.wt-win9x-editing/crates/quill-core/src/layout.rs` (`Measure`, the 224-entry width tables)
- `/home/scott/source/experiments/quill16.wt-win9x-editing/crates/quill-core/src/text.rs` (undo log, for the clean mark)
- `/home/scott/source/experiments/quill16.wt-magic/assets/mime/quill.xml` (PR #10's MIME file to install)
