# Screenshots

Real captures from Quill running on its target systems, small PNGs at native size, for the blog series and the documentation page. Each caption says what it shows and where it was taken.

| File | Caption | Taken on |
|---|---|---|
| `m2-letter-dos-colour.png` | The same letter in DOS text mode: bold is bright white, italic is cyan, underline magenta, strikethrough dark grey, raised and lowered text brown, and the price list has dot leaders on its tab stops. | `QUILLDOS.EXE LETTER.RTF` in DOSBox-X, colour scheme |
| `m3-letter-windows-me.png` | The Win32 build on Windows ME: the same screen drawn in a window in the system's OEM font, from the same code that runs in DOS. | `QUILL32.EXE` on the Windows ME VirtualBox VM |
| `m3-gdi-letter-windows-me.png` | The Win32 build's real page on Windows ME: Times New Roman and Arial from Windows' own fonts, a bold title, a red word, raised and lowered text, a price list with dot leaders on its tab stops, a hanging indent and a right-aligned closing, with the native menu bar. | `QUILL32.EXE LETTER.RTF` on the Windows ME VM |
| `m7-pictures-win32-wine.png` | Pictures in the Win32 page: a PNG and a BMP (the same gradient) drawn at their shown size inside the text, and a PNG whose data is damaged shown as a grey box because its checksum fails. | `QUILL32.EXE` under Wine in Xvfb (not a real Windows; to be retaken on the ME VM and Windows 95) |
| `m3-toolbar-ruler-win32-wine.png` | The Win32 build's two toolbar rows and ruler over the letter at 640 pixels wide (the width of a 640 by 480 screen): the standard bar (file, clipboard, undo, spelling) and the formatting bar (style, font and size boxes, Bold, Italic, Underline, colour, four alignments, two lists), with Bold shown pressed where the caret sits in bold text, and the ruler's scale and indent markers. | `QUILL32.EXE` under Wine in Xvfb (to be retaken on Windows ME and Windows 95) |
| `m3-style-preset-win32-wine.png` | Above, the Heading 2 preset applied to a paragraph from the style box (the box names it); below, the same document after one Undo, the box blank because the title paragraph matches no preset. | `QUILL32.EXE` under Wine in Xvfb |
| `quill-ico-images.png` | Every image in `QUILL.ICO`, enlarged on a magenta ground that shows the transparency: 16, 32 and 48 px at 4, 8, 24 and 32 bits (reading left to right, then down), then the 256 px image. | `tools/make_icons.py --sheet`, decoded by the script's own parser |
| `w31-icon-16-colour-video.png` | Windows for Workgroups 3.11 in 16-colour VGA with Quill's 16-colour icon: drawn in an open window and, minimised, on the desktop. | `ICONTEST.EXE` in DOSBox-X (`machine=vgaonly`, `VGA.DRV`, 640 by 480) |
| `w31-icon-16-colour-on-256-video.png` | The same icon on a 256-colour display: the 16 standard colours are all in the system palette, so it is exact. | `ICONTEST.EXE` in DOSBox-X (`machine=svga_et4000`, `SVGA256.DRV`, 640 by 480) |
| `w31-icon-256-colour-video.png` | The optional 256-colour icon on the same 256-colour display: Windows 3.11 draws it with the 20 static system colours, so the gradients become flat patches and the ground turns black in places. | `ICONT256.EXE` in DOSBox-X, as above |
| `w31-icon-256-on-16-colour-video.png` | The 256-colour icon on a 16-colour display: the shape and mask are right, the ink-blue ground maps to black. | `ICONT256.EXE` in DOSBox-X, as the 16-colour capture |
| `w31-icon-256-high-colour.png` | The 256-colour icon on a 64K-colour display: here Windows draws its gradients. | `ICONT256.EXE` in DOSBox-X (`machine=svga_s3`, `S3TRIO.DRV`, 800 by 600) |

More arrive with the milestones: Windows 3.11 (from M5), the real GDI page (from M3's later half), print preview and the help file.
