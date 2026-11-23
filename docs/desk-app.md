# Quill as a Desk application

Planning only: no Desk-specific code exists in Quill, and nothing in Desk's SDK is built yet. This page lists what Quill's core needs from a platform, maps each need to what the Desk SDK ([`SDK.md`](https://github.com/liminal-hq/desk/blob/main/docs/SDK.md), [`APP-MODEL.md`](https://github.com/liminal-hq/desk/blob/main/docs/APP-MODEL.md), [`HIG.md`](https://github.com/liminal-hq/desk/blob/main/docs/HIG.md), [`FONTS.md`](https://github.com/liminal-hq/desk/blob/main/docs/FONTS.md), [`LOADER.md`](https://github.com/liminal-hq/desk/blob/main/docs/LOADER.md) and [`LAYERS.md`](https://github.com/liminal-hq/desk/blob/main/docs/LAYERS.md)) offers or lacks, says what that means for milestone M10, and names the changes Desk would need. It was written from those documents as they stood on 23 November 2026; where one says a function is "still a design", so is the mapping below.

## Table of contents

- [What a Desk app is](#what-a-desk-app-is)
- [What Quill needs, and what Desk offers](#what-quill-needs-and-what-desk-offers)
- [Shape of a `quill-desk` shell](#shape-of-a-quill-desk-shell)
- [M10 and the Desk port](#m10-and-the-desk-port)
- [Changes Desk would need](#changes-desk-would-need)
- [What Quill promises meanwhile](#what-quill-promises-meanwhile)

## What a Desk app is

A Desk app is a module the core loads: a `.DXE` today (the hosted track, built as a Rust `staticlib` with LTO, `panic = "abort"` and `-Zbuild-std`, converted and linked with `dxe3gen`), later a `.DKX` on the native track. It exports `app_start`, `app_event` and `app_stop`, and the Desk calls it one event at a time (`key`, `char`, `mouse_*`, `paint`, `menu`, `timer`, `resize`, `clipboard`, `open_file`, `close`). It draws only inside a `paint` event through `desk_fill`, `desk_line`, `desk_text` and `desk_blit`, with colour **roles** and palette indices rather than RGB, it gets its heap from `desk_alloc`, and it never touches hardware. A panic unloads the app and shows a toast.

Quill's shape fits this well. `quill-app` already is "keys in, a picture out": `App::key` takes one key and the painter reads the document afterwards, which is the `app_event` model. The core is `no_std`, allocates only through a `Store`, and `quill-heap` already backs a `Store` with a global allocator, so a Rust app whose allocator forwards to `desk_alloc` needs no new storage code. Nothing in the core names a platform.

## What Quill needs, and what Desk offers

| Quill needs | Quill's seam | Desk offers | Missing or to decide |
|---|---|---|---|
| **Memory** in chunks of at most 32 KB, fallible | `Store` (`quill-heap` over the global allocator) | `desk_alloc` and `desk_free`, 8-byte aligned, NULL when short, counted per app | Nothing. Large documents mean large allocations in the core's one address space, so Desk should publish a per-app limit |
| **Key input** with Ctrl, Alt and Shift chords, repeat | `Key` and `App::key` | `key` and `char` events with `mods` | The exact encoding of function keys, Insert, Ctrl+Home and Shift+arrows, and whether Alt+letter opens the app's menu or is delivered |
| **Mouse** | none yet (the DOS and Win32 shells could not be driven by test tools) | `mouse_down`, `mouse_up`, `mouse_move` | Comes with the SDK: the first platform where the mouse is testable. Quill needs hit testing, which `walk_line` and `caret_xy` already support |
| **Measure** (character widths, vertical metrics) for layout | `Measure` (`resolution`, `metrics`, `widths`) | `desk_text_width(s, len, font)` | No vertical metrics (ascent, descent, leading) and no per-face size list. Quill asks for 224 widths per font: 224 calls to `desk_text_width`, cached by the core (eight fonts, as today) |
| **Fonts** by face, size in half points, bold, italic | `FontRef` | A font handle `u32` per face and size; Desk Serif, Sans, Mono and the UI faces, bitmap strikes at 8 to 48 points | Arbitrary sizes: Desk has fixed strikes, which is what M10 already assumed (nearest strike, said in the status line). Quill needs a way to list strikes and their widths |
| **Draw surface** for the page, ruler, toolbar and status bar | a `Surface` trait (planned in the architecture), generics | `desk_fill`, `desk_line`, `desk_text`, `desk_blit` inside `paint` | Text in an arbitrary colour: Desk offers roles, and a document has any of 16.7 million colours (the table holds 256). Underline and strikethrough are `desk_line`; superscript and subscript are smaller strikes at a shifted baseline |
| **Pictures** | `quill-image` decodes rows and dithers to 16 or the halftone palette | `desk_blit` with palette indices; `desk_palette_lend` for an app's own colours | Fits: Quill already produces palette indices at the shown size. Banded decoding fits the "shared buffer" idea in `APP-MODEL.md` |
| **Files**: open, save, create, replace | `Files` (`open`, `create`, `close`) over a streaming `Source` and `Sink` | `desk_file_read(path, out)` and `desk_file_write(path, data)` whole-file calls, `desk_file_unique`, `desk_pick_*` dialogs, `open_file` events | No streaming, no offset, no size query, no seek: the `Source` and `Sink` traits cannot sit on whole-file calls without reading a whole document into memory, and dictionary and pack paging cannot sit on them at all |
| **Pack and dictionary paging** | `PageSource` (planned for `quill-spell` and the pack reader); the `.QPK` reader, to become `.LPK` (D71) | none | A positioned read: `desk_file_read_at(path, offset, out, len)` and `desk_file_size(path)`. With those, `li-lpk`'s reader and Quill's dictionary pager work unchanged |
| **Clipboard** in RTF, text and a lone picture | a `Clipboard` seam (own, text-only on DOS today) | `desk_clip_put` and `desk_clip_get` with formats `text` and `bitmap` | An `rtf` format (or an app-private blob format) so formatting survives a copy to another Quill window or Notes |
| **Timers** (idle layout, spelling as you type, caret blink) | the idle loop in each shell | `desk_timer`, `desk_timer_stop`, `desk_ticks`; `timer` events | An idle or low-priority event for background work, so layout slices run when nothing else wants the app |
| **Menus** and accelerators | `menu_items` and `menu_title`: one tree for every build (the DOS grid, the Win32 `.res`) | `desk_menu_new`, `desk_menu_add(id, label, key, flags)`, `menu` events | Fits directly: the ids are the `Command` order, as in the `.res` writer. `HIG.md` fixes the menu order (File, Edit, View, then the app's own, Help last), so Quill's bar (Insert, Find, Character, Paragraph, Document) needs a decision on View |
| **Dialogs**: Open, Save As, Print, Find, Replace, Fonts, Tabs, About, Confirm | text-cell dialogs in `quill-app`; the common dialogs on Win32 | `desk_pick_*` (files, with type filters and 8.3 names), `desk_ask` (the only modal), `desk_control` kinds (`button`, `checkbox`, `text_field`, `list`) in a `dialog` window | File dialogs and confirms map to Desk's. Find, Replace, Fonts and Tabs are forms of text fields, choices and check boxes, which `quill-app`'s form engine models; a `quill-desk` shell would build them from controls |
| **Printing** | `print_text` to a name; GDI printing on Windows (not built) | none | A print path: at least `desk_print_text(bytes)` to a device or file; page images later |
| **Settings** and recent files | `QUILL.INI` through `Files` | `desk_setting_get` and `desk_setting_put`, one section per app in `DESK.INI` | Fits: the INI text becomes settings keys |
| **Date and time** for the Date field | `set_print_date` | `desk_now` (BCD from INT 1Ah) | Fits |
| **Help** | `QUILL.HLP` (WinHelp) and `QUILL.HLX` (own viewer) | none | Quill's own viewer inside the app, drawn like the document; or a Desk help service |
| **About** with memory and mode | `set_about` | `desk_log`, core counters per app | A way to ask the Desk for free memory |

## Shape of a `quill-desk` shell

It would be a crate beside `quill-win9x` and `quill-dos`, built for Desk's `core/dos.json` target as a `staticlib`, with the rest of the workspace untouched:

- `app_start` makes the `App` (with a `DeskMeasure`), a `Store` over `desk_alloc`, registers the menu from `quill_app::menu_items`, and opens a window.
- `app_event` translates `key`, `char`, `menu`, `open_file`, `timer` and `clipboard` events into `App::key` and `App::command`, and asks for a repaint.
- A `paint` handler draws the document from `for_each_line` and `walk_chars` (the same calls the GDI painter uses), line by line, with `desk_text` per run, `desk_line` for underline and strikethrough, `desk_blit` for pictures, and the app's own dialogs as controls.
- `DeskMeasure` implements `Measure` from `desk_text_width`, with a host twin that reads the same widths from the fonts' tables (`tests/metrics/`, as `CapturedMeasure` does), so the golden layout dumps keep working.

## M10 and the Desk port

M10 in the roadmap is "DOS graphics mode": `QUILLDOS.EXE /G` at 640 by 480 with Desk's faces rendered offline into bitmap strikes (`QUILL.FNT`), Quill's own chrome in the Windows 3.1 look, and a screenshot compared with a host PNG made with `DeskMeasure`. That plan builds a small graphics shell in Quill that duplicates what Desk is: a windowing layer, a mouse, menus, controls and fonts on a VGA.

**Proposal:** replace M10 with the Desk port, and keep the part of M10 that is a contract.

- **Keep** `DeskMeasure` and the host page render with Desk's face widths. They are the contract between the core and any shell that draws with Desk's faces, and they cost a table and a test.
- **Replace** the separate DOS graphics shell with `quill-desk`, once the SDK has the functions the table marks missing. DOS text mode stays Quill's own DOS build (it is small, runs without Desk and is the reference for editing behaviour, D37); the graphics experience on DOS becomes "Quill under Desk".
- **Keep the fallback.** If the Desk SDK is not usable when the other milestones are done, M10 stays as written, and it stays on the cut list (D54) where it already is.
- The roadmap's cut order does not change: M10 is cut before anything that touches the file format.

What this gets: proportional fonts, a mouse and real controls on DOS without Quill writing a graphics layer, and one more platform to run the same golden scenarios on (the host player already drives `App` with keys, so a Desk shell can be checked by playing the same scenarios through its event handler). What it costs: Quill's quality on DOS graphics depends on Desk's schedule, and Desk's cooperative single address space (an app crash is not a sandbox) is the environment.

## Changes Desk would need

For Quill, in the order they block it:

1. **Positioned file reads and file size**: `desk_file_read_at` and `desk_file_size` (or open, read, seek and close handles). Without them neither a large document, a pack nor a dictionary can be paged.
2. **Vertical font metrics and a face and size query**: ascent, descent and leading per font, and the list of strikes per face.
3. **Arbitrary text colour**: a role set wide enough, or `desk_text_rgb` (mapped to the nearest palette entry), or text drawn with a lent palette range.
4. **A clipboard format for rich text** (`rtf`, or an app-private blob that Desk carries untouched).
5. **A print path**: bytes to LPT or a file at the least.
6. **An idle event**, so layout can run in slices without a timer.
7. **Documented module limits**: how large an app may be (Quill's core, formats, images and app code will be on the order of 100 to 200 KB with LTO), its stack size (the layout and print routines keep a few kilobytes of buffers on the stack; Desk says a stack overflow takes the whole Desk down), and per-app memory.
8. **Key event details**: the full chord set and how Alt and F10 menu activation is delivered.
9. **A View menu rule** in `HIG.md` for document apps with page views (Quill has Document and Insert menus beyond the standard ones).

## What Quill promises meanwhile

The core stays free of any platform assumption, so a `quill-desk` shell can be added without touching it: nothing in `quill-core`, `quill-formats`, `quill-image`, `quill-inflate`, `quill-app` or `quill-grid` names DOS, Windows or Desk; everything platform-shaped (memory, files, fonts, drawing, the clock, the clipboard, printing, paging) comes in through a trait the shell implements. Where a platform trait does not yet exist for something (the clipboard, `PageSource`, `Surface`), it is added before the second implementation of that need, not after. This is recorded as D79.
