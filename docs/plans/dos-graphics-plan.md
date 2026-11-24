# DOS graphics mode for Quill: implementation plan

Graphics mode can sit beside text mode in the same `QUILLDOS.EXE`. Most of the hardware work already exists in Desk and can be copied with a provenance record, as `dos/rt` was. The DPMI 0800h mapping call is already in Quill's runtime. On the Quill side, six of the nine new PRs can start now; the rest wait for #19, #20 and #21.

The user's request settles the open D79 proposal in `docs/desk-app.md` (make M10 the Desk port): Quill gets its own graphics mode, M10 as written. Reusing Desk's drivers and faces, behind a `Surface` trait, keeps a later Desk port possible. Recorded as decision 1 in section 6.

Repo root: `/home/scott/source/experiments/quill16` (read from `origin/main` at 84a2e30). Desk: `/home/scott/source/liminal-hq/desk` (commit 4a021a7, the same commit `dos/PROVENANCE.md` names). [V] = verified in code, a document or a binary. [U] = not verified.

## Facts the plan rests on

**Quill's code**:

- **No `Surface` trait exists anywhere.** `quill-app/src/paint.rs` only paints the text `Grid`. The real page painter is `win9x/quill-win9x/src/gdi.rs`, and it uses only public `App` methods: `doc`, `for_each_line`, `selection`, `caret_xy`, `editing`, `story_view`, `field_text`, plus `line_label` and `walk_chars`. A painter can therefore live in a new crate without editing `quill-app`. [V]
- **`App<M: Measure + Clone = GridMeasure>` is generic, and `walk_chars` clones the measure on every call.** [V] The DOS measure must therefore be one type for both modes (an enum) and cheap to clone. Two instances of `App` in one exe would roughly double the core's code (Win16 measured core plus app at 77.9 KB of code16 [V]).
- **The view geometry is already 96 dpi.** `quill-app/src/view.rs` has `DPI = 96`, `TB_H = 28`, `RULER_H = 22` and the 16 Windows colours in `COLOURS` (0x80/0xC0 values). [V]
- **Page fit at 640 × 480.** A Letter page at 96 dpi is 816 px wide, so only the 6.5-inch text column (624 px) fits beside a scroll bar. The page area is about 346 px tall with two toolbar rows, the ruler and the status bar. [V by arithmetic]
- **The toolbar art is ready for 16 colours.** `assets/toolbar/toolbar.bmp` is 352 × 15, 4 bits per pixel, Windows palette (c0c0c0, 808080, …, plus dfdfdf). [V]
- **The dither uses the wrong 16 colours.** `quill-image` has an ordered dither to the CGA-style VGA palette (0xAA/0x55), not the Windows one. [V]
- **The runtime already has the DPMI calls.** `dos/rt/src/dpmi.rs` has `map_physical` (0800h), `unmap_physical`, `simulate_int` (0300h) and `alloc_dos_memory` (0100h). DS is raised to a 4 GiB limit, so `linear_to_ptr(0xA0000)` and a mapped linear frame buffer are plain near pointers. The first megabyte sits at linear 0 under CWSDPMI. [V]
- **Files can seek.** `rt::dos::File::seek_start` exists, so font strikes can load lazily. [V]
- **The runtime has no fast `memcpy`/`memset`.** Desk's `dos/rt/src/mem.rs` (`rep movs`/`stos`) was dropped in Quill's copy. [V]
- **The text-mode loop.** It is `keys::wait()` (INT 16h AH=0, blocking) then `quill_app::draw` into `Grid<25>`, then B8000h. There is no mouse and no timer yet. [V]
- **DOS targets a 486, not a 386.** `dos-elf.json` is an i486 target and the SPEC's DOS row says 486 or later. [V]
- **Size today.** `QUILLDOS.EXE` is 196–217 KB across the worktrees, against a 360 KB unpacked budget. [V]
- **Menu state reaches the shell only after #21.** `menu_open() -> Option<(u8, u8)>`, `menu_items_for` and `menu_box` arrive with #21, not `main`. [V]
- **Native dialog requests exist.** `set_native_dialogs` and `Dialog` cover 10 dialogs. [V]
- **Dialog widgets are already free of drawing.** `quill-dlg` widgets and focus carry no drawing code (D121, #27). [V]

**Desk's prior art**:

- **`desk-vga`** (`hal/drivers/vga`) covers mode 12h and 13h register tables, the DAC, the planar and linear canvases, VBE 4F00h–4F05h, and a software arrow cursor with save-under. **`desk-int33`** polls INT 33h. Both are host-tested against a register mock (82 tests). [V]
- **`dos/rt/src/hal.rs`** has `DosBios` (DPMI 0300h with a 512-byte low buffer) and `DosDisplay` (0800h mapping). [V]
- **`shell/present`** has a planar presenter that writes whole bytes and never reads video memory, plus read-back for captures. [V]
- **What Desk ran.** Under DOSBox Staging 0.83, `svga_s3`, CWSDPMI, it mapped the linear frame buffer at physical C0000000h through 0800h, and checksummed mode 12h, mode 13h and VBE 640 × 480 × 8. `vgaonly` has no VBE. [V per Desk's `docs/A-VGA-VBE.md`; not rerun]
- **The fonts are OFL 1.1 with no Reserved Font Name.** That covers Desk Sans, Serif and Mono (four styles each) and the pixel faces Desk UI, Title, Fixed and Small. [V `docs/FONTS.md`]
- **Desk's outline faces are unhinted.** In monochrome they look "lumpy below about 20 px"; in grey they read cleanly from about 14 px. [V `docs/FONTS.md`]

**Tools**:

- **DOSBox Staging 0.83** (vendored) offers the machines `svga_s3`, `svga_et4000`, `svga_paradise`, `vesa_nolfb`, `vesa_oldvbe` and `vgaonly`. [V, from the binary's strings]
- **`dosrun.sh`** defaults to `svga_s3`, `memsize = 16`, `cpu_cycles = max`. `win16/tools/dosbox-x/386sx-4mb.conf` exists. [V]
- **skrifa 0.50** has an autohinter and a TrueType interpreter (`Engine::AutoFallback`), and `tiny-skia` is in the local registry. [V]
- **The float ban** (`scripts/check-no-floats.sh`) covers only `quill-core` and `quill-formats`. [V]

**Decision numbers.** `main` ends at D105. D106–D119 are reserved for Linux, and D120–D124 are used by open PRs. I propose keeping this work's decisions inside `docs/dos-graphics.md` as `docs/linux.md` does, reserving **D130–D149** (re-check when G0 lands).

## 1. Video modes

**What to target**:

| Tier | Mode | Shadow buffer | When |
|---|---|---|---|
| Baseline | VGA mode 12h, 640 × 480 × 16, planar | 150 KB (four planes of 38,400 bytes) | Any VGA (INT 10h AX=1A00h returns AL=1Ah) |
| A | VBE 2.0 linear frame buffer, 640 × 480 × 8 | 300 KB | A VBE BIOS reports mode attribute bit 7 and a `PhysBasePtr` |
| B | VBE linear frame buffer, 800 × 600 × 8 | 480 KB | Only when asked for (`/G800`), because of memory on 4 MB |

**Drop:**

- **CGA, Hercules and MDA graphics.** Those machines keep text mode.
- **EGA mode 10h.** Its 640 × 350 pixels are not square, so 96 dpi layout would distort.
- **1024 × 768.** Not in the SPEC, and 768 KB of shadow.
- **Banked VBE (VBE 1.2, or VBE without a linear frame buffer).** Fall back to mode 12h instead; 4F05h is wrapped in Desk but never run. A later PR can add it if real hardware demands it.

**How to drive it**:

- **Mode 12h:** INT 10h AX=0012h, then Desk's identity attribute palette, then the DAC loaded through 3C8h/3C9h. Desk did port I/O from ring 3 under CWSDPMI without a fault. [V Desk] Recommended DAC: the Windows 16 colours (`view.rs COLOURS`), so document colours, the toolbar bitmap and the Windows 3.1 greys are exact. Decision 7 covers this.
- **VBE detection and setup:**
  - Call 4F00h ("VBE2" signature) and 4F01h through `simulate_int` with ES:DI pointing at an `alloc_dos_memory` buffer.
  - Read far pointers below 1 MB through `linear_to_ptr`.
  - Set the mode with 4F02h | 0x4000, map with `map_physical(phys, pitch × height)` and use the pointer directly.
  - Honour `BytesPerScanLine`, which can be larger than the width.
  - Load the DAC through ports unless mode attribute bit 5 says the DAC is not VGA-compatible; then use 4F09h. [U on real BIOSes]
- **INT 10h calls with return segments** (ROM font pointer AX=1130h, VBE pointers) must go through 0300h. A direct `int 0x10` cannot return a valid real-mode ES. [U for CWSDPMI's reflection; 0300h is safe]
- **Clean exit:** register `leave` with `cleanup`, as `video.rs` does, so a panic or an exit restores the saved text mode and `unmap_physical`.

**Double buffering and speed.** The `dos/rt` wrapper needs nothing new for any of this, apart from `mem.rs`.

- **Draw into a RAM shadow in the device's own format.** Mode 12h gets four plane arrays, so presenting is a straight copy and no pixels are converted to planes at present time. VBE gets one byte per pixel.
- **Present only rows that changed.** Keep a u32 hash per row per plane (about 2 KB) instead of a second frame copy. For each changed row:
  - set the map mask (sequencer register 2) to one plane, the bit mask to FFh and set/reset off;
  - copy whole bytes with `rep movsd`;
  - never read video memory (Desk's `PlanarPresent` rule).

  The linear frame buffer is a straight `rep movsd` per row.
- **Copy Desk's `mem.rs`** (`rep movsd`/`stosd`) into `dos/rt`. Fizz 2 measured a 64 KB present dropping from 3,589 to 599 µs with it. [V that the Desk doc reports it; not rerun]
- **The caret and the mouse cursor are composited at present time** over a small rectangle and never enter the shadow (Desk decision). A blink redraws one rectangle.
- **The 256-colour palette:**
  - a 6 × 6 × 6 cube (216 entries);
  - the Windows 16 (14 of them not already in the cube);
  - a few spare entries for chrome.

  Smoothed black text uses grey, silver and white, which are already in the 16. Other colours blend to the nearest cube entry.
- **Speed on real hardware** [U, estimates only]:
  - A full mode 12h frame is 150 KB: about 30–150 ms on 8- or 16-bit ISA VGA, under 15 ms on VLB or PCI.
  - A keystroke that changes one line is about 20 rows × 320 bytes, or 6 KB: 1–6 ms.
  - Repainting the whole shadow in RAM per keystroke (about 2,000 glyph blits) is the CPU risk on a 486SX-25. G10 measures it, and adds line-level damage (redraw only lines whose signature changed) if needed.
  - A 386 is not a target, because the DOS build is i486.

**Testing hosts**:

- **DOSBox Staging:** `svga_s3` for VBE, `vgaonly` and `vesa_nolfb` for the fallbacks to mode 12h [V machine names], `DOS_CONF` for `memsize = 4`, and fixed cycles for timing.
- **DOSBox-X:** the existing `386sx-4mb.conf` pattern with `cputype = 486` and `memsize = 4`, and `svga_et4000`. [U what VBE DOSBox-X's ET4000 offers]
- **Real hardware** [U throughout]:
  - Many 1993–95 cards have only VBE 1.2 without UniVBE, so they get mode 12h.
  - Some clone DACs want delays between writes; `PortIo::io_wait` exists in Desk.
  - CWSDPMI's 0800h has not been tried outside DOSBox.
  - A Windows 9x DOS box (Windows as the DPMI host) may restrict 0800h.

## 2. Fonts

**Recommendation: keep D39 (Desk's faces rendered offline into strikes), with three changes.** Render grey at build time, synthesise the large sizes, and load strikes lazily.

**Source faces** (decision 2)

- **Default: Desk Serif, Sans and Mono**, as the SPEC says. They are OFL with no Reserved Font Name, so a rendering may keep the names "Desk …". The OFL text must still travel with `QUILL.FNT` (`OFL.TXT` on the disk), and the About box names it, as the SPEC already says.
- **Alternative: the Liberation-derived "Quill Test" subsets from #25.** They are metric-compatible with Times, Arial and Courier, so DOS line breaks would come close to Windows. Any rendering is an OFL Modified Version, so the "Liberation", "Arimo", "Tinos" and "Cousine" names must not appear. #25 already renamed them; a shipping name such as "Quill Serif" would follow the same rule (D116 in `docs/linux.md`).
- **Desk's grids are untouched.** Its pixel faces (`ui.dfn`, `title.dfn`) are about 6 KB each and serve the menus, dialogs and status bar in the Windows 3.1 style (Title is bold, like the System font).

**The build-time generator** (`crates/quill-fntgen`, host only, std, floats allowed)

- skrifa outlines, autohinted at each pixel size (`Engine::AutoFallback`), drawn into tiny-skia paths at 96 dpi.
- Coverage is quantised to **2 bits per pixel**. The four levels map exactly onto white, silver, grey and black in the Windows palette (ink 0, ¼, ½, 1).
- Coloured text and selected text threshold the same data at ½, so they stay crisp 1-bit.
- A `[Quill] Smooth=0` setting forces 1-bit everywhere.

**Sizes** (decision 4)

- Render **8, 10, 12, 14, 18 and 24 pt** (11–32 px).
- **36, 48 and 72 pt are exact pixel doublings or triplings** of 18 and 24 pt (48 px = 2 × 24, 64 = 2 × 32, 96 = 3 × 32). The 2× and 3× scales of every strike also give 16, 20, 28 and other sizes.
- Any other size uses the nearest strike, and the status line says so (SPEC).

**`QUILL.FNT` format** (specified in G0)

- **Header:** magic, version, faces, and the strike directory (face, style bits, size in half points, pixel size, ascent, descent, leading, bits per pixel, offset, length).
- **Per strike:** 224 one-byte advances and per-glyph boxes (x and y bearing, width, height, offset).
- **Bitmaps:** rows padded to bytes.
- **Not compressed inside.** The `.QPK` pack deflates it on the floppy.
- **Loading:** at start-up only the directory and the advance tables are read (72 strikes × about 240 B, roughly 20 KB). Bitmaps load on first use with `seek_start`, each at most one 32 KB `Store` chunk.

**Measure and draw from one engine** (the Windows 95 caret-drift lesson)

- `StrikeMeasure` reports resolution (96, 96), the strike's integer advances (×2 or ×3 when scaled) and its metrics.
- The painter blits each glyph at the `x` that `walk_chars` gives. Layout widths, caret positions, hit testing, justification and drawing are therefore the same integers by construction.
- A single function `strike_for(&FontRef) -> (StrikeId, scale)` is used by both sides. A property test (G3) proves they agree.
- Superscript and subscript take two-thirds size through the same mapping.

**Comparison with Windows metrics.** As #25 did, `quill-fntgen --report` lays the letter out with `StrikeMeasure` and with `CapturedMeasure` (the Windows ME widths in `tests/metrics`). It lists glyph-width differences of a pixel or more, and lines that break differently, for both candidate face sets. It is a report, not a pass mark. G1 produces it together with proof sheets (1-bit, 1-bit hinted, 2-bit at 8–24 pt) so the user can choose by eye.

**Size and memory** [U, estimates; G1 measures the real figures]

- Ink area is about 0.3 em² per glyph × 224 glyphs × 12 styles, summed over 11–32 px. That is about 250 KB at 1 bit and about 500 KB at 2 bits, packing to about 110 KB and about 200 KB.
- The roadmap budgets `QUILL.FNT` at 250 KB unpacked and 120 KB packed. 2-bit grey needs it raised to about 400–500 KB and 200 KB. The DOS disk sums to about 950–1,050 KB of its 1,300 KB, so that fits, but it eats most of the slack.
- Fallbacks if it does not fit: grey only up to 14 pt, or drop Mono's italic and bold italic.
- Memory: about 20 KB resident, plus about 10–20 KB per strike in use. Trivial on 4 MB.

## 3. The painter and the chrome

**The `Surface` trait** lives in a new portable crate `crates/quill-paint`: `no_std`, no `alloc`, no floats, no panics, depending on `quill-core` and `quill-app`. It is integer, pixel-addressed and generic (no `dyn`, per D31):

```rust
pub trait Surface {
	type Font: Copy;
	fn size(&self) -> (i32, i32);
	fn clip(&mut self, r: Option<Rect>);
	fn font(&mut self, f: &FontRef) -> Self::Font;
	fn fill_rect(&mut self, r: Rect, c: Rgb);
	fn hline(&mut self, x0: i32, x1: i32, y: i32, c: Rgb);
	fn vline(&mut self, x: i32, y0: i32, y1: i32, c: Rgb);
	fn glyph_run(&mut self, f: Self::Font, base_y: i32, glyphs: &[(u8, i32)], fg: Rgb, bg: Option<Rgb>);
	fn bitmap4(&mut self, x: i32, y: i32, rows: &[u8], w: i32, h: i32, pal: &[Rgb; 16]);
	fn picture(&mut self, index: u16, r: Rect) -> bool;
	fn invert_rect(&mut self, r: Rect);
}
```

The painter in `quill-paint` is written fresh from `gdi.rs` `draw_line`, `paint_page` and `paper_sheet`. It covers:

- runs (each glyph at the layout's `x`), underline, strikethrough, raised and lowered text, colour;
- tab leaders, list labels, field values;
- selection (navy background, white text), the caret;
- the sheets, the page gap and headers and footers in preview.

Later, C3 of the Linux plan moves the Win32 and Win16 GDI painters onto the same trait. This DOS work does not touch `gdi.rs`, so it never conflicts with #21.

**Two surfaces in `quill-paint`, over caller-owned `&mut [u8]`:**

- `Planar4` (640 × 480 × 4 planes);
- `Linear8` (any width, given pitch).

Both are pure code, so the host runs exactly the code DOS runs. Host goldens and DOS frames are then byte-identical, and DOSBox only has to prove the presenter.

**Text mode stays as it is.** `quill_app::draw` into a `Grid`, unchanged. The DOS measure becomes `enum ScreenMeasure { Grid(GridMeasure), Strikes(StrikeMeasure) }` in `dos/quill-dos`. `StrikeMeasure` holds a `&'static` directory leaked once at start, so `Clone` is a pointer copy. The exe carries one `App<ScreenMeasure>`.

**Choosing the mode**:

- `/G`: best 640 × 480 (VBE 256 colours, else mode 12h).
- `/G16`: mode 12h only.
- `/G800`: 800 × 600 × 256.
- `[Quill] Graphics=0|1|16|800`.
- `/T` overrides the setting back to text.

Fall back to text mode, with one status-line message, when there is no VGA, when `QUILL.FNT` is missing or fails its checks, or when the shadow cannot be allocated. `Rows` and `/50` stay text-only. There is no switching while running in v1, because that would need `App::set_measure` and a relayout.

**Chrome** (Windows 3.1 look, geometry from `view.rs`, keys exactly as in text mode):

- **Menu bar and pull-downs:** drawn graphically from `menu_open()`, `menu_items_for` and `menu_box` (#21), in Desk Title and UI. The menu logic stays in `App` (DOS leads, D37).
- **Toolbar:** `toolbar.bmp` through `bitmap4`, with 3D bevels and pressed or mixed toggles from `view::is_on`.
- **Face and size boxes:** drawn as fixed fields with drop-down lists of the strike sizes and the three faces.
- **Ruler:** `view::ruler_ticks` and `ruler_marks`, dragged through `view::Drag`.
- **Scroll bars and status bar:** status text from `status_text`.
- **Caption:** none at 640 × 480 (it costs 20 px); shown at 800 × 600 (decision 8).

**Dialogs** (decision 5)

- **v1 = text-cell overlay.** The SPEC says "Everything else is as in text mode, drawn in graphics", and Word for DOS 5.5's graphics mode did the same.
- A new `quill_app::draw_overlay` (in `paint.rs`, after #21) paints only menus, prompts, dialogs and the help viewer into a `Grid<30>`. Untouched cells hold a "transparent" marker.
- The shell draws the cells at 8 × 16 with the VGA ROM font (INT 10h 1130h BH=06h through 0300h, exact CP437), mapping text attributes to silver, black, navy and white.
- This brings Fonts, Tabs, Find, Spelling, About, the `quill-dlg` file dialog (#26–#28) and the `QUILL.HLX` viewer with no new dialog code.
- **Later (G12, optional):** graphical dialogs from `assets/dialogs/dialogs.json`, the same descriptions the Windows resources use, driven by `quill-dlg` widgets. That needs a native-dialog mask so the shell takes the dialogs it can draw.

**Pictures:** decode rows with `quill-image`, box-scale, then dither to the active palette. Give the dither a palette argument: the Windows 16 for mode 12h, the cube for 256 colours. The shell caches results in `Store` chunks with a fixed budget (decision in G9). A failed decode or one over budget shows as a grey box, as on Win32.

**Colour schemes.** `Scheme` stays text-only. In graphics, `/MONO` maps everything to black, greys and white for VGA mono LCD panels.

**Mouse**:

- INT 33h function 0 (present), 7/8 (range set to the mode's size, because drivers do not know VBE modes), 3 (position and buttons), 5/6 (press and release counters, so short clicks are not lost), 2 (hide the driver's cursor).
- Register-only calls can use `int 0x33` directly, as `int 0x10` does today. [U for 33h]
- The arrow is a software cursor (Desk's `cursor.rs` sprite), composited at present time.
- Double-click timing comes from the BIOS tick at 0040:046C (18.2 Hz).
- The same module gives text mode its missing mouse (M2).

**Print preview:** one or two sheets fitted to the screen. Text is greeked as grey bars per run below 50 %, and pictures are scaled. It uses the `quill-paint` page walk with a scale; keys as in Win32. Printing stays plain text (SPEC).

## 4. What the core needs, what goes in `dos/`, and tests

**Portable work (no floats, `no_std`, no panics)**

- `crates/quill-strike`: `QUILL.FNT` parsing, with every offset checked once in `parse` so lookups cannot fail; `StrikeMeasure`; `strike_for`; glyph access by strike, over a `StrikeSource` trait the shell implements.
- `crates/quill-paint`: `Surface`, `Planar4`, `Linear8`, the page painter and chrome painters.
- `quill-image`: palette-parameterised dither.
- `quill-app` (after #21): `draw_overlay`, and later the native-dialog mask.

`Store` holds strike bitmaps and the picture cache. The shadow buffers are shell-owned `Vec`s, because `dos/` may use `alloc`.

**DOS-specific code**:

- **`dos/hal/`:** a new host-testable workspace with the subset of `desk-hal` that is needed (`port`, `mmio`, `bios`, `video`, `error`), `desk-vga`, `desk-int33` and the mock, copied from Desk 4a021a7 and listed in `dos/PROVENANCE.md`. This follows Desk's own `hal/` layout. [V that Desk's `shell/dos` depends on these by path]
- **`dos/rt`:** add `mem.rs` and `bios.rs` (`DosBios`, from Desk's `rt/src/hal.rs`).
- **`dos/quill-gfx` (DOS target):** mode selection and fallback, VBE and mapping, the DAC, the presenters with row hashes, the ROM font, the mouse, the cursor, the strike loader (`StrikeSource` over `rt::dos::File`) and the frame read-back.
- **`dos/quill-dos`:** the `ScreenMeasure` enum, the main loop with non-blocking keys (INT 16h AH=11h/10h), mouse polling, caret blink, and the switch between text and graphics.

**Tests**:

- **Host golden pixel tests** (`crates/quill-paint/tests`, goldens in `tests/golden/gfx/`):
  - render `letter.rtf`, `lists`, `headers` and `pictures` at 640 × 480 × 16 and × 256 through `Planar4` and `Linear8` with a committed test `QUILL.FNT`;
  - write 4- or 8-bit indexed PNGs (extend `quill-host/src/png.rs`) plus a `.sums` file of CRC-32s per frame;
  - `QUILL_UPDATE_GOLDEN=1` rewrites them.
- **Property tests** (proptest, random documents, formats, sizes, super and subscript, the 2× and 3× sizes):
  - a recording `Surface` shows every glyph's `x` equals `walk_chars`' `x`;
  - `pos_at_point` at the caret's point gives back the same position;
  - a click left or right of a glyph's half-advance gives i or i+1;
  - nothing is drawn outside the clip;
  - the painter never panics on damaged documents.
- **Scenario frames:** the host player runs `tests/scenarios/*.keys` through `App` and `quill-paint` and writes per-frame sums. Mouse lines (`@click x y`, `@drag …`) are added to the scenario format, because AUTOTYPE cannot drive a mouse [V progress.md]. DOS reads them from a `QUILL_MOUSE` file in place of INT 33h.
- **DOSBox:** `QUILL_DUMP` in graphics writes `=== frame N crc32 … ===` per present, computed from video memory read back (planes through the read map select, or the linear frame buffer), as Desk did. `QUILL_SHOTS=D:\` writes `.RAW` and `.PAL` for chosen frames. `dos/tools/e2e.sh` gains `run_gfx_scenario`; `dos/tools/raw2png.py` is copied from Desk. Runs on `svga_s3`, `vesa_nolfb` and `vgaonly` check each fallback.
- **Screenshots:** the exact read-back PNGs (1× and 2× nearest) for the blog, the site and `docs/media`. A `DOS_X11=1` Xvfb grab of the DOSBox window for "in the emulator" shots (the `testing.md` method).

## 5. Work breakdown (stacked PRs, each sized for one Sonnet implementer)

| # | Branch / base | Scope | Acceptance | Can start |
|---|---|---|---|---|
| G0 | `docs/dos-graphics` on `main` | `docs/dos-graphics.md`: this design, the `QUILL.FNT` byte layout, the `Surface` trait, D130–D149. No SPEC edits yet (#19 edits the neighbouring rows) | markdownlint; owner answers section 6 | **Now** |
| G1 | `feat/fntgen` on `feat/linux-fonts` (#25, shares skrifa and the lockfile) | `crates/quill-fntgen`: skrifa autohint, tiny-skia, 1- or 2-bit strikes, the `QUILL.FNT` writer; proof-sheet PNGs; the Windows-metrics report for Desk and Quill Test faces; Desk TTFs vendored under `assets/fonts/desk/` with `OFL.txt` and SHA-256 sums | Reproducible bytes (built twice, identical); size table printed; proof sheets in `docs/media`; report committed | **Now** |
| G2 | `feat/strike` on `main` | `crates/quill-strike`: parser, `StrikeMeasure`, `strike_for` and scaling; tests on hand-built strikes plus damaged and random bytes | clippy with the portable lint set; float ban covers the crate; truncated, overlapping or out-of-range files refused without panics | **Now** (parallel with G1) |
| G3 | `feat/paint` on G2 | `crates/quill-paint`: `Surface`, `Planar4`, `Linear8`, page painter, sheets, caret, selection; host PNG and sums goldens; the agreement and hit-test property tests | goldens for four documents at 16 and 256 colours; property tests at 10,000 cases | **Now** (uses only public `App` API on `main`) |
| G4 | `feat/dos-hal` on `feat/dos-rt-dir` (#20, which edits the `dos/` members) | `dos/hal/` copied from Desk, `rt/mem.rs`, `rt/bios.rs`, PROVENANCE rows | `cd dos/hal && cargo test` (Desk's tests pass); `dos/check.sh`; a 12h and VBE probe example checksums in DOSBox `svga_s3` and exits 2 on `vgaonly` | **Now** |
| G5 | `feat/dos-gfx-boot` on `main` after #19, #20, #21 merge, plus G3 and G4 | `dos/quill-gfx`; `/G`, `/G16`, `Graphics=`; fallback; strike loading; `App<ScreenMeasure>`; page-only screen in 12h; row-hash presenter; frame sums; `e2e.sh` graphics scenario | DOSBox frame CRCs equal the host's sums for `m1-edit`-style keys; Esc/exit and panic return to text; exe size reported against 360 KB | After #19–#21 |
| G6 | `feat/dos-gfx-chrome` on G5 | Menu bar and pull-downs, toolbar, ruler, scroll bars, status bar; Windows DAC palette | host goldens of each menu open; DOSBox sums equal for `m3`-style keys | |
| G7 | `feat/dos-gfx-overlay` on G6 | `quill_app::draw_overlay`, the ROM font, cell theme; every text dialog, the file dialog and help in graphics | `m2-dialogs` and `m8-spelling` scenarios: sums equal; the text-mode goldens unchanged | After #26–#28 for the file dialog |
| G8 | `feat/dos-mouse` on G6 (independent of G7) | INT 33h module, software cursor, click, drag, double-click, toolbar and ruler hits; text-mode mouse too; `@click` scenario lines | host tests on `Int33Model`; scripted-mouse scenario sums equal in DOSBox; works with `builtin_dos_mouse_driver = off` (no mouse) | |
| G9 | `feat/dos-vbe` on G6 | VBE 640 × 480 × 256 and 800 × 600 × 256, cube palette, `Linear8` presenter, smoothed colour ramps; pictures dithered to either palette with a cache budget | sums equal on `svga_s3`; `vesa_nolfb` and `vgaonly` fall back to 12h; `pictures.qld` golden | |
| G10 | `perf/dos-gfx` on G9 | PIT stopwatch (Fizz 2 method), frame-time log, line-damage painting if needed, the 4 MB run | keystroke and scroll times at fixed cycles logged in progress.md; `memsize = 4` run opens a 3 MB picture document without failing | |
| G11 | `feat/dos-preview` on G9 | Graphical print preview (one or two pages, greeked) | goldens; DOSBox sums | |
| G12 (optional) | `feat/dos-gfx-dialogs` | Graphical dialogs from `dialogs.json` and `quill-dlg`, with a native-dialog mask in `App` | per-dialog goldens; text-cell path still green | Owner's choice |
| G13 | `docs/dos-graphics-done` | SPEC (DOS graphics section, settings, command line, build differences), decisions, progress, roadmap budgets, screenshots, site scene 14 | docs lint; the evidence linked | Last |

**Start now:** G0, G1, G2, G3 and G4. None of them touches files that #19, #21 or #26–#28 edit. G1 stacks on #25 and G4 on #20, which only edit lines next to these changes. **Waits:** G5 and later wait for #19, #20 and #21, because they edit `dos/quill-dos/src/main.rs`, `keys.rs`, `video.rs` and `quill-app` `lib.rs`/`paint.rs`/`view.rs`. G7's file dialog waits for #26–#28.

## 6. Decisions, risks and what I could not verify

**Decisions for the user (recommended default in bold)**:

1. **Quill's own DOS graphics mode now (M10 as written), reusing Desk's drivers and faces.** The Desk port stays possible later through `Surface`. This supersedes the `desk-app.md` proposal in D79.
2. Faces: **Desk Serif, Sans and Mono**, or the Liberation-derived renamed faces (Windows-like line breaks). Decide after G1's proof sheets and report.
3. **2-bit grey for black text on white, 1-bit for coloured and selected text**, with `Smooth=0` available; or 1-bit everywhere.
4. Sizes: **render 8–24 pt and get 36, 48 and 72 by doubling or tripling**; or render 36 and 48 too (about 3× the bytes).
5. Dialogs: **text-cell overlay in v1**; graphical Windows 3.1 dialogs (G12) later or never.
6. Modes: **12h baseline, VBE linear frame buffer 640 × 480 × 256, and 800 × 600 on request; no banked modes, no 1024, no EGA, CGA or Hercules.** Switches `/G`, `/G16`, `/G800`, `/T`.
7. 16-colour DAC: **the Windows palette (0x80/0xC0)**, or the BIOS CGA-style one (0xAA/0x55).
8. Caption bar: **none at 640 × 480, shown at 800 × 600**.
9. Budgets: **raise `QUILL.FNT` to about 450 KB unpacked and 200 KB packed** if 2-bit holds.
10. **No switching between text and graphics while running in v1.**
11. Sources: **vendor the 12 Desk TTFs** (about 850 KB in the repository), or fetch them by checksum.

**Risks**:

- **Memory on 4 MB.** The 800 × 600 shadow (480 KB), the picture cache, the dictionary (300 KB) and the document all compete. Cap the picture cache, keep 800 × 600 opt-in, and test with `memsize = 4`. CWSDPMI swapping hides overruns as slowness.
- **Speed on a 486SX with ISA VGA.** Full repaints of the shadow and full presents on scroll may be slow. Presenting changed rows only is in from G5; line damage comes in G10 if measured. A 386 is out of target.
- **Availability of VESA and CWSDPMI.** Real cards often have only VBE 1.2, so they get mode 12h. 0800h under other DPMI hosts and in Windows DOS boxes is untested.
- **Exe size.** It is about 200–217 KB now. Graphics adds an estimated 80–110 KB [U], which is near the 360 KB budget; G5 measures it. One `App` instance is essential.
- **Font quality.** Unhinted Desk outlines are weak in mono below 20 px, so the plan depends on the autohinter and grey rendering. That is unproven until G1.
- **Conflicts.** G5–G7 depend on the merge order of #19, #20 and #21.

**Not verified**:

- Desk's DOSBox results were read, not rerun.
- skrifa's autohint quality on Desk's faces.
- Every size, speed and memory figure above is an estimate.
- INT 33h called directly from protected mode under CWSDPMI.
- The ROM font pointer through 0300h.
- DOSBox-X's ET4000 VBE.
- AUTOTYPE in graphics modes.
- Any real hardware.
- Whether #21's `menu_open` signature survives review.

### Critical Files for Implementation

- /home/scott/source/experiments/quill16/win9x/quill-win9x/src/gdi.rs
- /home/scott/source/experiments/quill16/crates/quill-app/src/view.rs
- /home/scott/source/experiments/quill16/dos/rt/src/dpmi.rs
- /home/scott/source/experiments/quill16/dos/quill-dos/src/main.rs
- /home/scott/source/liminal-hq/desk/hal/drivers/vga/src/planar.rs
