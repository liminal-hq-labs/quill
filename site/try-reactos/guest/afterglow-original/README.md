# Afterglow original: a visual style for ReactOS 0.4.16

An original Afterglow visual style (`afterglow.msstyles`) for the ReactOS demo guest: dusk captions, a near-black taskbar with a gold Start button, rose and gold accents, and classic cream client areas so applications such as Quill32 keep their classic look. Every bitmap is drawn by `agart.py` from the Liminal HQ Afterglow palette and every line of the style INI is written by hand in `ini/dusk-normal.ini`. Nothing is taken from Lunar, Lautus or any other style; `PROVENANCE.md` says how each part was made.

## Files

| File | What |
|---|---|
| `agdraw.py` | Palette, a small pixel canvas (rounded shapes, outlines, two-band gloss) and a BMP writer that picks a 4-bit, 8-bit or 24-bit encoding |
| `agart.py` | Every bitmap: captions, frames, caption buttons and glyphs, scroll bars, dialog controls, toolbars, status bars, the taskbar, the Start button, task buttons and the tray |
| `ini/dusk-normal.ini` | The style's INI (colour Dusk, size Normal), hand-written; `{tokens}` are palette colours |
| `gen.py` | Draws the bitmaps, fills the INI, checks that every image the INI names exists and every drawn image is used, writes `themes.ini` and `afterglow.rc` |
| `build.sh` | `gen.py`, then `llvm-rc` and `lld-link /Brepro` into a resource-only x86 DLL. `--dist` copies the result to `dist/` with its SHA-256 |
| `dist/afterglow.msstyles` | The built style (119,808 bytes) and `afterglow.msstyles.sha256`; rebuilding gives the same bytes |
| `cursors/gen-cursors.py` | Draws the Afterglow pointer set (14 pointers, 32 × 32 with 8-bit alpha) and writes `.cur` files, plus 8-frame `.ani` versions of the two busy pointers |
| `cursors/afterglow-*.cur`, `*.ani` | The generated pointers (committed; `build.sh --dist` refreshes them) |
| `cursors/patch-user32.py` | Puts the pointers into a copy of the guest's `user32.dll` as its system cursors (see below) |
| `afterglow-original.reg` | Selects the style (`ColorName` `Dusk`, `SizeName` `Normal`), sets the title colours of the classic scheme, the caption metrics and the "Afterglow" pointer scheme |
| `polish.reg`, `apply-polish.cmd` | Demo polish, kept separate: no version watermark on the desktop, and the smooth dusk wallpaper |
| `apply.cmd`, `deploy.sh`, `vm.sh` | Test tooling: import both registry files in the guest, copy the style, pointers, patched `user32.dll` and wallpaper onto a disk copy with mtools, run a private QEMU VM with its own monitor and pid file |

## Build and try

```sh
bash build.sh ~/.cache/reactos/web/afterglow-original --dist     # WORKDIR first; --dist refreshes dist/
cp --sparse=always ~/.cache/reactos/web/reactos16-trim.img ~/.cache/reactos/web/reactos16-afterglow2.img
WALLPAPER=".../quill-wallpaper-smooth-1024x768.bmp" bash deploy.sh ~/.cache/reactos/web/reactos16-afterglow2.img apply
bash vm.sh shot out.png
bash vm.sh stop
```

Needs Python 3 with numpy and Pillow (Pillow only for the contact sheet), `llvm-rc`, `lld-link`, and for testing `mtools`, `xorriso`, QEMU with KVM and ImageMagick. By hand, in any ReactOS 0.4.16 guest: copy `dist/afterglow.msstyles` to `C:\ReactOS\Resources\Themes\Afterglow\`, import `afterglow-original.reg` and log on again, or pick Afterglow in Display Properties, Appearance. Picking it there also copies the style's system colours into the scheme; those are the classic cream values except the title colours.

## Caption size

The ReactOS theme engine draws the title in a box that starts 3 px below the button row and ends 2 px above the caption's bottom (`nonclient.c`), so the box is `SM_CYCAPTION - 11` px tall. With the classic `CaptionHeight` of 18 (`SM_CYCAPTION` 19) that is 8 px, and an 8 pt bold title is cut off. `afterglow-original.reg` sets `CaptionHeight` and `CaptionWidth` to 24 px (`-360` twips) and `SmCaptionHeight` and `SmCaptionWidth` to 23 (`-345`), and the style's `[SysMetrics]` carries the same sizes for Display Properties. The bar is then 29 px (4 px frame and 25), the title box 14 px, and the 20 px button cells hold a 16 px face with a clear ring. Menu bars, client areas, scroll bars and the taskbar keep their sizes; windows lose 6 px of client height.

## Pointers

ReactOS 0.4.16 never applies a pointer scheme: `User32SetupDefaultCursors` says "FIXME load system cursor scheme", `SPI_SETCURSORS` is unimplemented and `SetSystemCursor` cannot replace a cursor once one is loaded. The system pointers always come from `user32.dll`'s own cursor resources at boot. So the deploy script does two things: it installs the `.cur` and `.ani` files in `C:\ReactOS\Cursors\Afterglow\` with an "Afterglow" scheme in `HKCU\Control Panel\Cursors` (listed and previewed in Control Panel, Mouse, Pointer), and it runs `cursors/patch-user32.py`, which appends the 14 images to the guest's `user32.dll` in a new section and points its cursor groups at them. That patch is what makes them the default pointers; it changes a ReactOS system file on the demo disk only, and it is skipped when the file is already patched. The system busy pointer is static: `user32` resources hold static cursors only. Each pointer has the footprint and hot spot of the stock ReactOS pointer it replaces (measured from the 1-bit cursors in the guest's `user32.dll`: the arrow about 11 x 19 with its tip at (2, 2), the I-beam 7 x 16 at (6, 9), the crosshair 17 x 17 at (11, 11) and so on), drawn in a 32 x 32 image with clear padding. `REPATCH=1 bash deploy.sh IMAGE` replaces the pointers in a `user32.dll` that was patched before (`patch-user32.py --force`; the result is byte-identical to patching the original file).

## What the ReactOS engine draws

| Area | Classes and parts | Themed |
|---|---|---|
| Window frame | `Window`: `Caption`, `SmallCaption`, `MaxCaption`, `MinCaption`, `FrameLeft`, `FrameRight`, `FrameBottom` | Yes, every window with a caption, cc6 or not |
| Caption buttons | `Window`: `MinButton`, `MaxButton`, `RestoreButton`, `CloseButton`, `SmallCloseButton`, `HelpButton` (eight engine states each) | Yes |
| Scroll bars | `Scrollbar`: `ArrowBtn` (16 states), `ThumbBtnVert/Horz`, `GripperVert/Horz`, the four track parts, `SizeBox` | Yes in themed windows; Quill32's own scroll bars stay classic |
| Dialog controls (cc6 only) | `Button` (`PushButton`, `CheckBox`, `RadioButton`, `GroupBox`), `Edit.EditText`, `ComboBox`, `Spin`, `Tab` (items, `Pane`, `Body`), `Progress`, `Header`, `Trackbar`, `TreeView.Glyph`, `ListBox`, `ListView`, `TreeView` borders | Yes |
| Bars | `Toolbar`, `Rebar`, `Status` | Yes; the status bar is drawn to match the classic one, so Quill32's status bar barely changes |
| Shell | `TaskBar` backgrounds and sizing bars, `TaskBar::Rebar`, `Start::Button`, `TaskBand::Toolbar`, `TaskBand.FlashButton`, `TrayNotify` (and `TrayNotifyHoriz::`, `TrayNotifyVert::`), `TrayNotify::Clock`, `TrayNotify::Toolbar`, `ShowDesktop::Button` | Yes |
| Known engine issue: ghost caption buttons | After a window is maximised (title-bar button, caption double-click or Alt+Space, X), a second set of caption buttons stays at the old, restored-width positions until the caption is repainted (minimise and restore, or switch away and back). It happens with this style, with another ReactOS style (Lunar) and in Notepad as well as Quill32, and not with the style switched off, so it is a ReactOS `uxtheme` bug, not the art or the metrics: the button rectangles are cached per window (`rcCaptionButtons`) and only recomputed in the `WM_SIZE` / `WM_WINDOWPOSCHANGED` hooks, so the first non-client paint after maximising draws the buttons at the old positions over a fresh caption, and the later repaint at the new positions does not cover them. A style cannot work around it (the engine places the buttons); the upstream fix would be to recompute the button rectangles in `ThemePaintWindow` before drawing | No: engine |
| Not drawn by the engine | Menus and menu bars, the classic Start menu body and its side banner, tooltips, list view selection colours (system colours), the corner square between two scroll bars (filled with `ButtonFace`), the Start orb icon | No: these keep the classic scheme |
