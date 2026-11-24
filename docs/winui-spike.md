# WinUI 3 spike S0a

What spike S0a found: can Windows Reactor, a WinUI 3 library for Rust, show a pixel-exact Quill page from a zip built on Linux and run on a clean Windows 11 machine. The plan is in [winui.md](winui.md); the code is in `winui/spike`, with the build and runtime staging in `winui/tools`. This is the S0a half only: input methods and accessibility (S0b) are not covered.

## Result

**Go for S0b.** Every S0a acceptance check passed on Windows 11 22H2 (build 22621.6060) in the `WinDev2407Eval` VirtualBox VM, with its network cable off and nothing installed for the app.

| Check | Result |
|---|---|
| Builds on Linux with `cargo xwin` | Yes. Reactor 0.100.0 compiles and links for `x86_64-pc-windows-msvc` with the static CRT. A clean build takes about 15 s on four jobs |
| Zip starts with nothing installed, cable off | Yes, at 100%, 150% and 175% |
| Checkerboard 100% exact | **100.000%** of the pixels of the whole 1000 × 521, 1500 × 782 and 1750 × 912 frames equal the source at 100%, 150% and 175%; maximum channel difference 0; the best shift is (0, 0) in all three |
| Menus open over the page | Yes: the File menu drops over the page at all three scales (100%, 150% and 175% screenshots); choosing Open logged `menu: Open` at 150% |
| Combo box opens over the page | Yes: its list drops over the page at all three scales; choosing Heading 1 logged `combo: Heading 1` at 175% |
| Zip size | 23.3 MB (22.2 MiB) for 294 files; 61.5 MB unpacked; the exe is 2.6 MB |
| Startup | About 250 to 460 ms from process start to the first presented frame (below) |

## How the page is shown

The page is never drawn by WinUI. `pattern::render` builds the whole frame in device pixels with tiny-skia: a surround of `#123456`, a one-pixel checkerboard, vertical lines every four pixels, a ruler, and a white page with six lines of Times New Roman outlines from skrifa (unhinted, advances rounded once per glyph from twips, as in the Linux spike). A Direct3D 11 device creates a composition swap chain of exactly the panel's device size, `SetMatrixTransform(1 / scale)` makes one buffer pixel one screen pixel, and `CopyResource` fills the back buffer. The panel size in device pixels is the `Metrics` event's DIPs times the scale, rounded; at 150% the panel is 1000 × 521.33 DIPs, which is 782 whole pixels, so the layout rounding did not need correcting.

The comparison follows the GTK spike's `analyse.py`. The app writes the exact pixels it presented (`frame-*.png`); `winui/spike/tools/analyse.py` finds the `#123456` bounding box in a `VBoxManage controlvm screenshotpng` frame, crops the same size and compares it with the source, reporting the share of equal pixels, the largest channel difference, whether the checkerboard stayed pure black and white, and the best match over a shift of two pixels in each direction.

```text
scale 1.00   source 1000x521   whole panel exact 100.000%   max difference 0   checkerboard 100.00% pure and alternating
scale 1.50   source 1500x782   whole panel exact 100.000%   max difference 0   checkerboard 100.00% pure and alternating
scale 1.75   source 1750x912   whole panel exact 100.000%   max difference 0   checkerboard 100.00% pure and alternating
```

Nothing fractional shows: not at 150% and not at 175% (a ratio the GTK spike could not reach). The text is inside the same comparison, so glyph placement is exact too.

## Build and staging

- **Reactor 0.100.0** from crates.io, pinned with `=`, edition 2024, MSRV 1.95. Its pre-generated bindings meant no `.winmd` files and no bindgen run. The raw `windows-bindgen` retry was not needed.
- **`windows` 0.62 for Direct3D.** Reactor uses `windows-core` 0.100 and the `windows` crate is 0.62, so their `IUnknown` types differ. The swap chain is handed over as a raw pointer (`into_raw`, then `IUnknown::from_raw` of the 0.100 type). It works; it is a seam to remove when the `windows` crate catches up.
- **Runtime.** `winui/tools/stage-runtime.sh` downloads `Microsoft.WindowsAppSDK.Runtime` **2.4.0** from nuget.org (156.4 MB, once, into `~/.cache/quill-winui`), checks SHA-256 `93f48b096416ab7b908ea30690bba47de8264cd2f22dd7b0ad97187a86d00120` kept in `winui/tools/runtime.sha256`, unzips the nupkg's `tools/MSIX/win10-x64/Microsoft.WindowsAppRuntime.2.msix`, unzips that, and copies the 118 entries `runtime.txt` lists (all 118 existed in the package; 57 MB). Version 2.4.0 is what `windows-reactor-setup` 0.100.0 pins; master has moved to 2.6.4 and Reactor 0.101, whose API differs (`MenuBar::items`, for one). The hash was taken from the first download; nothing else vouches for it.
- **Manifest.** `windows-reactor-setup`'s `app.manifest` has no DPI or OS entries, so the script writes its own: `PerMonitorV2` and `true/pm`, the Windows 10/11 `supportedOS` ID, `longPathAware`, the `<description>windows-reactor-self-contained</description>` marker Reactor looks for in resource 24/1, and 980 registration-free `activatableClass` entries in 26 files read from the runtime package's own `AppxManifest.xml`. All 920 classes in the upstream manifest are among them. `build.rs` embeds it with `/MANIFEST:EMBED /MANIFESTINPUT`, which lld-link accepts. WebView2 entries are left out because the app has no WebView.
- **Static CRT** through `.cargo/config.toml`; the exe imports nothing from `vcruntime` or the UCRT DLLs. `/IGNORE:4099` hides lld's missing-PDB notices for the CRT libraries.
- `winui/tools/build.sh` builds, copies the staged runtime and the exe into one folder and zips it. Build output goes under `CARGO_TARGET_DIR`, outside the repository.

## What failed, with the exact errors

1. **A `ComboBox` cannot go in a `CommandBar` in Reactor 0.100.0.** The first run died at once with exit code `0xC0000409` (a Rust `abort`, which is why the panic hook logged nothing). Redirecting stderr showed:

   ```text
   windows-reactor failed command 34: InsertChild { parent: NodeId { index: 13, generation: 0 }, slot: Some(CommandBarPrimaryCommands), child: NodeId { index: 17, generation: 0 }, index: 2 }: Native(-2147467262)
   windows-reactor fatal native command failure at 34: Native(-2147467262)
   ```

   `-2147467262` is `0x80004002`, `E_NOINTERFACE`: the native code casts each child to `ICommandBarElement`, and a `ComboBox` is not one. Reactor has no `AppBarElementContainer`, which is how WinUI hosts arbitrary controls in a command bar. The spike puts the `ComboBox` in the same grid row, beside the `CommandBar`. The style, font and size boxes of the plan therefore need either a container Reactor does not have, or a hand-built row. Any native failure aborts the process, so a wrong child kills the app instead of returning an error.
2. **The `CommandBar` collapsed to its overflow button.** `Bold` and `Italic` (icon-less `AppBarButton`s) sit behind the `…` button even though the row has room; opening it shows them over the page. Not investigated; the buttons have no icons, which may be why.
3. **A console window sits behind the app in the screenshots.** It is the `cmd /c` wrapper that redirects stderr, not the app, which uses `windows_subsystem = "windows"`.

Not seen: `Class not registered` and missing runtime files (the staged set and manifest were enough on the first start that got past the abort). A black flash before the first frame was not sampled: screenshots were taken seconds after the start, and the first frame is presented about 250 to 460 ms after the process starts.

## Startup and size

Times are from the app's own log (`main` to first presented frame) and from PowerShell's process `StartTime`. The first run after each reboot is cold.

| Run | `main` to first frame | Process start to first frame |
|---|---|---|
| 175%, first start after the abort fix | 279 ms | not measured |
| 150%, cold, after a reboot | 405 ms | not measured |
| 100%, cold, after a reboot | 463 ms | not measured |
| 100%, warm | not logged | 244 ms (process start to `main` 23 ms) |

Working set 112 to 117 MB. These are a VM with VBoxSVGA and a hardware Direct3D 11 device (`d3d11 device: hardware`), not a measurement of a real machine.

## How the VM was driven

The VM started from the snapshot `Password never expires` with the network cable off, a tip snapshot `before winui spike` was added, only the zip, `QUILL32.EXE` and `letter.rtf` were copied in with `guestcontrol copyto`, and the zip was unpacked with `Expand-Archive`. A scheduled task created with `schtasks /create /it /ru User` starts the exe in the user's desktop session; the task's window appears on the desktop, which is what the screenshots show. Whether a `guestcontrol run` process would appear there was not tried. Screenshots are `VBoxManage controlvm screenshotpng` only. Keys went in with `keyboardputscancode` (Tab, Enter, Down, Escape); the menu bar is reached by Tab, because Reactor 0.100.0 gives menu titles no access keys.

The guest starts at 175% (`AppliedDPI` 168, `PerMonitorSettings` `DpiValue` 3). Setting `DpiValue` to 2 and 0 and restarting gave 150% (`AppliedDPI` 144) and 100% (96); a restart was enough, no sign-out. Afterwards the VM was powered off and `Password never expires` restored. The password was not changed or written anywhere.

The final binary in the zip differs from the one tested only by two lint-driven edits (`as_chunks_mut` in the pixel swap, one removed `.into()`); the three comparisons ran on the build before them.

## Baseline: `QUILL32.EXE` on Windows 11

`QUILL32.EXE` is the Windows 95 build from `main` (369,152 bytes, SHA-256 `34aa685ca69aa1d9580a25de16fb4803dc43a616dd606b2d49561098709de8bc`), opening `tests/docs/letter.rtf`. Screenshots are in `docs/media/winui-spike/`.

- **It runs and edits.** The page, ruler, toolbar and status bar all draw.
- **Blurry at 150%.** [quill32-150.png](media/winui-spike/quill32-150.png) shows the soft, stretched text and toolbar glyphs of a DPI-unaware app that Windows scales as a bitmap; [quill32-100.png](media/winui-spike/quill32-100.png) is sharp.
- **Unthemed controls.** The menu bar, toolbars, combo boxes, scroll bar and status bar are the classic flat grey look with square corners, inside a Windows 11 frame with the new caption buttons. The size, font and style boxes are Windows 9x combo boxes.
- **No dark mode, no Mica**, as expected without a manifest.

## What is not proven

- Mica: `WindowBackdrop::Mica` was requested, but the title area looks like a flat light surface in the screenshots, and no run compared it with an unrequested backdrop.
- Dark mode, high contrast, and DPI changes while the app runs (the scale was set before each start; the `Metrics` handler rebuilds the swap chain on a change but that path was not exercised).
- A real GPU: the VM's VBoxSVGA path only. A machine with no hardware adapter falls back to WARP in the code, untested.
- Whether the guest lacks a Windows App Runtime or Visual C++ runtime of its own. The VM has Rust installed and was not checked with `Get-AppxPackage`, so a missing dependency could be masked; the CI clean-machine check in the plan still applies.
- Windows 10, ARM64, and the 125%, 200% and larger scales.
- Keyboard focus from Tab into the swap-chain panel, input methods and UI Automation: the work of S0b.
- Whether Reactor exposes the underlying XAML objects or the window handle. 0.100.0 gives typed requests for the swap chain, focus, image sources, WebView2 and a composition host, and `AutomationExt` properties, but no raw `HWND` or `UIElement`. The Win32 route (P2) can find the window by title with `EnumWindows`.

## For S0b

Start with the hidden `TextBox` proxy (P1) and the child window with IMM32 (P2) side by side. The first needs `TextBox` and focus control from Reactor, which exist; the second needs the window handle, found through `EnumWindows` on the process id. Custom automation peers need the raw `UIElement`, which Reactor does not give, so P1's Narrator work likely means the proxy `TextBox` carrying the text, or generating bindings with `windows-bindgen` as the plan's fallback describes.
