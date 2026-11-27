# Quill print set

Printable media for Quill's release disks: floppy labels, the CD face and jewel case inserts, a sticker sheet, a foldable manual and a keyboard shortcut card. Everything is drawn as vector SVG masters by a script and laid out on US Letter and A4 pages with crop marks, fold ticks and cut lines. The layout language comes from the Lieutenant Fizz packaging concepts (decision INS-23); the identity is Quill's own: the gold ring and nib on deep ink, parchment and amber.

## Files

| File | What |
| --- | --- |
| `gen_media.py` | The generator and the one place for the words, the sizes and the version (`VERSION`, `EDITION`). Edit it, never a master |
| `render.py` | Lays the masters out on Letter and A4 pages and writes the PDFs, the complete PDF with bookmarks and page labels, the previews and a `README.txt` (needs `rsvg-convert` and `qpdf`) |
| `check_media.py` | Fails when a master differs from a fresh run of the generator, when type is under 6 pt, or when a label, disc or tray card does not name the version |
| `outline.py` | Turns text into SVG outlines from a TrueType face (standard library only), from Afterglow's dialog masters with kerning added |
| `masters/option-a/` | Option A, era-accurate retro: each disk in its own period |
| `masters/option-b/` | Option B, modern Afterglow dusk |
| `masters/stickers/` | Twelve 2 in stickers |
| `masters/booklet/` | The eight manual pages and the shortcut card |

Each option has `floppy-dos.svg`, `floppy-w31.svg`, `floppy-w95.svg`, `floppy-all.svg`, `floppy-documents.svg`, `disc.svg`, `jewel-front.svg` and `tray-card.svg`.

```sh
python3 assets/media/gen_media.py                     # the masters (needs only Python)
python3 -I assets/media/check_media.py                # the masters are current
python3 assets/media/render.py "$HOME/Documents/Liminal HQ/Quill/Print set"   # the PDFs and previews
```

## Changing the version or the words

`VERSION` at the top of `gen_media.py` is the release the media is worded for (today `0.1.0`, the demo release); `EDITION` says what that release is. The printed serial follows from the version (`serial()`): the owner's catalogue prefix `QD05` (`SERIAL_PREFIX`), then the major version as one hex digit, the minor as two and the patch as one, so `0.1.0` is `QD05-0010` (INS-25). Change them there, run the three commands above and commit `gen_media.py` with the regenerated masters. The tagline, the honest note, the licence, the addresses and the disk set (`DISKS`) are constants beside it.

## The pieces and the standards they target

All lengths in millimetres unless stated. Crop marks are 0.25 pt hairlines, 5 mm long, starting 1 mm outside the bleed. No layout is matched to a die-cut sheet's positions: print on full-sheet sticker paper or card and cut on the marks.

| Piece | Trim | Bleed | Standard |
| --- | --- | --- | --- |
| Floppy label | 69.85 x 69.85 (2.75 in square): 50.8 (2 in) on the front, 3.3 over the edge, 15.75 on the back, upside down | 1.5 | The 3.5 in diskette label of the Avery 5196 family. Six to a page, two across and three down, with fold ticks |
| CD face | 118 outer, 41 inner diameter on a 120 disc | 1 both ways | The usual printable disc area, and a 117 mm full-face label's 41 mm hole; dashed cut circles |
| Jewel case front inlay | 120 x 120 | 3 | The standard jewel case front |
| Tray card | 150 x 118, with 6 mm spines inside (folds at 6 and 144) | 3 | The standard jewel case tray card; fold ticks above and below |
| Stickers | 50.8 (2 in), six round and six square with 3 mm corners | 1.5 | Common 2 in sticker sizes, three across and four down; dashed cut paths |
| Booklet page | 120 x 120, eight pages on two 240 x 120 sheets | 3 | The jewel case booklet, saddle stitched: sheet 1 is pages 8 and 1, then 2 and 7; sheet 2 is 6 and 3, then 4 and 5 |
| Shortcut card | 152.4 x 101.6 (6 x 4 in) | 3 | The 6 x 4 in index card, two to a page |

The booklet prints landscape and double-sided, flipping on the short edge. Its pages also go to `booklet-reading.pdf` in reading order, for the screen.

## The two directions

- **Option A, era-accurate retro.** The DOS disk is a VGA text screen: the grey menu bar with red mnemonics, a double-bordered dialog with a black shadow, `QUILL` in block characters and a status line. The Windows 3.x disk is a Program Manager group in the 3.11 colours Quill's screenshots show, with the 16-colour Quill icon, Quill Help and Uninstall Quill. The Windows 95 disk is the teal desktop and the cascade Start, Programs, Quill, as the installed program appears. The all-in-one disk puts the three side by side over a parchment title band. The CD is a Windows 95 Setup window on teal, the jewel front the hero's three windows over a period title band with a starburst, the tray card a Windows 3.1 dialog.
- **Option B, Afterglow dusk.** Quill's ink gradient, the gold ring and nib from `assets/icon/quill.svg`, Desk Serif in parchment, an amber chip naming the system, and the stepped dusk bands of the Afterglow palette at the foot. On the CD the gold ring of the Q goes round the centre hole.

Both share the words, the sizes and the stickers, booklet and card, which use Quill's own identity with period accents (each system's heading in its own colours). No Microsoft, ReactOS or Rust logo is drawn: the Start button carries the Quill icon, and the systems are named in words.

## Type and art

Every word is outlined from the Desk faces in `assets/fonts/desk` (SIL Open Font License 1.1, which allows embedding the outlines; the booklet's credits page names it), so no font is needed to render or print. Wherever the logo stands as a mark at print size (every sticker but the Windows 3.x one, the booklet, the shortcut card, the tray card, the CD, the option B labels and the title bands of option A) it is the master emblem from `assets/icon/quill.svg`. The 16-colour pixel icons from `assets/icon/QUILL31.ICO` and `QUILL.ICO` appear only inside period screens: the Program Manager group (on option A's labels and the Windows 3.x sticker), the Start menu, the window title bars and the Start button. Every Start button (the Windows 95 and all-in-one labels of option A, and the Windows 95 sticker) is the classic grey Windows 95 one, with the small Quill icon where the flag would be and the word Start. The screenshots are linked from `docs/media` by a relative path and inlined by `render.py`. Text paths carry `data-text` and `data-pt` for the check.

## Not yet checked

No physical print has been made: the fold of the floppy strip, the spine folds and the booklet's duplex registration are drawn to the standard sizes but not tried on paper.
