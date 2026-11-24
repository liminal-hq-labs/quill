# `QUILL.FNT` format

`QUILL.FNT` holds the bitmap strikes the DOS graphics mode draws text with: Serif, Sans and Mono in four styles at six sizes, each a whole face rendered at one pixel size at 96 dpi. `quill-fntgen` writes it on the host; the DOS shell reads it. The format is the contract between them, so it is specified here first. If a fuller `docs/dos-graphics.md` lands, this page is the byte-level reference it links to.

All integers are little-endian. All offsets are from the start of the file. A strike is one (face, style, size) with the bitmaps of the 224 characters of Windows-1252 from 0x20 to 0xFF.

## Layout

```text
header          32 bytes
directory       strike_count x 24 bytes
advances        strike_count x 224 bytes        (one table per strike, in directory order)
strike data     per strike: boxes 224 x 6 bytes, then bitmaps   (in directory order)
```

The first three parts are small and are read at start-up (about 18 KB for 72 strikes). A strike's data is read with one seek the first time the strike is drawn; it is at most 32 KB (`MAX_DATA_LEN`), so it fits one `Store` chunk.

### Header

| Offset | Size | Field | Value |
|---|---|---|---|
| 0 | 4 | magic | `QFNT` |
| 4 | 2 | version | 1 |
| 6 | 2 | strike_count | strikes in the directory |
| 8 | 2 | glyph_count | 224 |
| 10 | 1 | first_char | 0x20 |
| 11 | 1 | entry_len | 24 |
| 12 | 4 | dir_offset | 32 |
| 16 | 4 | file_len | the file's length |
| 20 | 4 | crc32 | CRC-32 (zlib polynomial) of bytes 32 to the end |
| 24 | 8 | reserved | zero |

### Directory entry (24 bytes)

| Offset | Size | Field | Notes |
|---|---|---|---|
| 0 | 1 | face | 0 Serif, 1 Sans, 2 Mono (the document font table's family codes 1, 2 and 3, less one) |
| 1 | 1 | style | bit 0 bold, bit 1 italic |
| 2 | 2 | half_points | 16, 20, 24, 28, 36 or 48 for 8, 10, 12, 14, 18 and 24 pt |
| 4 | 1 | px | the em in pixels at 96 dpi (11, 13, 16, 19, 24, 32) |
| 5 | 1 | ascent | pixels above the baseline for a line (the font's Windows ascent) |
| 6 | 1 | descent | pixels below it |
| 7 | 1 | leading | external leading in pixels |
| 8 | 1 | bpp | 1 or 2 |
| 9 | 3 | reserved | zero |
| 12 | 4 | adv_offset | offset of this strike's 224 advances |
| 16 | 4 | data_offset | offset of this strike's boxes; the bitmaps follow them |
| 20 | 4 | data_len | bytes of boxes and bitmaps (224 x 6 plus the bitmaps) |

The directory is ordered by face, then style, then size. A file may leave strikes out (a reader finds a strike by its fields, not its position) and may mix 1-bit and 2-bit strikes.

### Advances

One byte per character: the advance in whole pixels. Layout, caret placement and drawing all use these integers, so they cannot disagree. A strike drawn at 2 or 3 times takes the advances times 2 or 3.

### Glyph box (6 bytes)

| Offset | Size | Field | Notes |
|---|---|---|---|
| 0 | 1 | x | signed: pixels from the pen position to the left edge of the bitmap |
| 1 | 1 | y | signed: rows from the baseline up to the top row of the bitmap |
| 2 | 1 | w | width in pixels; 0 for a blank glyph |
| 3 | 1 | h | height in rows; 0 for a blank glyph |
| 4 | 2 | offset | offset of the bitmap within the strike's bitmaps; 0 for a blank glyph |

The bitmap is cropped to the pixels that carry ink.

### Bitmaps

`h` rows of `ceil(w x bpp / 8)` bytes, top row first, most significant bit first, rows padded to bytes with zero bits. A 1-bit pixel is ink or not. A 2-bit pixel is a level, 0 to 3, which the shell maps to the Windows palette colours white, silver, grey and black (ink 0, 1/4, 1/2 and 1 of the text colour's contrast). Coloured and selected text thresholds the level at 2 and above.

## Checks a reader makes once

`parse` checks, before any lookup: the magic and version; `file_len` equal to the file; the checksum; `glyph_count`, `first_char` and `entry_len`; for every strike, `bpp` of 1 or 2, `face` 0 to 2, `style` 0 to 3, that its advances and data lie inside the file, that `data_len` is at least the box table and at most 32 KB; and for every glyph, that `offset + h x ceil(w x bpp / 8)` lies inside the strike's bitmaps. After that no lookup can fail or read outside the file.

## Compatibility

A reader refuses a version it does not know. A change that keeps old readers working adds only to the reserved bytes and bumps nothing; any other change bumps the version.

## How the generator draws

`quill-fntgen` draws each face with skrifa's autohinter (`Engine::AutoFallback`; the source faces carry no TrueType instructions, so that selects the autohinter) and tiny-skia, at the pixel size a point size has at 96 dpi, to the nearest pixel.

- **2-bit** strikes use the smooth hinting target and anti-aliased coverage, quantised to the nearest of the ink fractions 0, 1/4, 1/2 and 1, which are exactly white, silver, grey and black of the Windows palette.
- **1-bit** strikes use skrifa's mono hinting target and aliased (pixel-centre) rendering.
- **Advances** are the autohinter's advances rounded to whole pixels.
- The output depends only on the font files and the options; building twice gives the same bytes.

## Decisions

| # | Decision |
|---|---|
| D145 | `QUILL.FNT` is the layout above: a 32-byte header with a checksum, a directory of 24-byte entries, advances for every strike up front, and per-strike boxes and bitmaps loaded on first use. Nothing is compressed inside; the pack deflates it on the floppy. |
| D146 | A strike holds one depth, 1 or 2 bits, chosen when the file is generated. The four 2-bit levels are the ink fractions 0, 1/4, 1/2 and 1 (white, silver, grey and black of the Windows 16 colours). |
| D147 | The source faces are vendored under `assets/fonts/desk/` (Desk Serif, Sans and Mono, SIL OFL 1.1, no Reserved Font Name, licence text kept beside them) and `quill-fntgen` renders them and the Quill Test faces alike, so the choice between them is made on the proof sheets and the Windows comparison report. |
