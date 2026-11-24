# WinUI 3 spike S0a

What spike S0a found: can Windows Reactor, a WinUI 3 library for Rust, show a pixel-exact Quill page from a zip built on Linux and run on a clean Windows 11 machine. The plan is in [winui.md](winui.md); the code is in `winui/spike`, with the build and runtime staging in `winui/tools`. S0b, input methods and accessibility, is in the second half of this page.

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

The final binary in the zip differs from the one tested first only by two lint-driven edits (`as_chunks_mut` in the pixel swap, one removed `.into()`). That final build was run once more at 175% and compared with `analyse.py`: 100.000% of the pixels equal, maximum channel difference 0, best shift (0, 0), so the lint edits changed nothing.

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

## WinUI 3 spike S0b

What spike S0b found: can a page that Quill draws itself take typed text, dead keys, Alt codes and input methods, and be read by UI Automation, inside a Reactor window. The code is `winui/spike/src/s0b` (`quill-winui-s0b.exe`, built by the same `winui/tools/build.sh`); `--p1` and `--p2` pick the hosting, `--popup` makes P2 an owned top-level window, `--imeon` opens the input method at focus, `--nobackdrop` drops Mica. The log is `quill-winui-s0b.log` next to the exe. The guest was `WinDev2407Eval` from the snapshot `Password never expires`, cable off, at 175%, with the US-International layout added through `Set-WinUserLanguageList`.

### Result

**P2 as a child window fails. P2 as an owned pop-up window works for typing and is the only hosting that was measured. P1 was built and never run. The go/no-go is therefore not "go" yet: see the end.** The run was cut short: the VM host ran out of memory, so the VM was powered off and restored with the checks below still open.

| Check | P1 (swap-chain panel and proxy `TextBox`) | P2 (own window, IMM32, AccessKit) |
|---|---|---|
| Builds on Linux, clippy clean | Yes | Yes (`accesskit` 0.25.2, `accesskit_windows` 0.35.2) |
| `CoreTextEditContext` | Probe built, never run | not applicable |
| Dead key then e (US-International) logs 0xE9 | Not run | **Yes**: `WM_KEYDOWN vk=0xDE`, `WM_KEYDOWN vk=0x45`, `WM_CHAR 0x00E9`, `insert via WM_CHAR: U+00E9 -> 0xE9` |
| Alt+0233 logs 0xE9 | Not run | **Yes**: `WM_CHAR 0x00E9`, `insert via WM_CHAR: U+00E9 -> 0xE9` |
| An emoji is refused with a status message | Code written, not run | **Not shown.** The emoji panel opened (Win+.) but no emoji was chosen: no key or pointer path to pick one was tried. The refusal is covered by the host test (`doc.rs`: `an_emoji_is_refused_whole`) and the code path logs `refused via ...` |
| IME candidate window at the caret | Not run | **Not shown.** See "Input methods" |
| PowerShell UI Automation reads text, selection, caret | Not run | **Partly**: text yes, selection and caret not completed |
| Narrator reads the line | Not run | Not run (manual steps below) |
| Tab moves between the command bar and the page | Not run | Not run |
| Dialog and flyout not hidden | Not run | Not run |

### The child window is hidden by the XAML content

The first P2 build created a `WS_CHILD` window over the page area, found with `EnumWindows` on the process id (the top-level window's children were `DesktopChildSiteBridge`, `InputSiteWindowClass` and `InputNonClientPointerSource`; ours was listed first, which is the top of the z-order). Its `WM_PAINT` ran with the full 1750 × 910 rectangle and the page was blitted, yet the screenshots show only the XAML surface: [s0b-p2-child-hidden.png](media/winui-spike/s0b-p2-child-hidden.png). It was hidden with the Mica backdrop and with `--nobackdrop`. The XAML island is composed above the GDI content of the top-level window, so a child window of the Reactor window cannot show anything under or over XAML. This is the airspace problem in its worst form, and it is independent of the z-order.

The workaround that works is an owned top-level pop-up (`WS_POPUP`, owner the Reactor window) placed over the page area, with a 30 ms timer that follows the owner's client rectangle and hides it when the owner is minimized. [s0b-p2-popup-typed.png](media/winui-spike/s0b-p2-popup-typed.png) shows the page drawn by skrifa and tiny-skia in it, with the caret and typed text. Its costs: it is not a child (no automatic clipping, no sharing of focus with the island, flicker while the window moves), the owner's title bar looks inactive while the pop-up has focus, and the timer is a polling loop.

### Typing

With the pop-up focused, `keyboardputstring "abc"` produced `WM_KEYDOWN` and `WM_CHAR` and three characters on the page. The sequences in the table above went through `WM_CHAR` only; no `WM_IME_*` message arrived, because no input method was composing. Windows also sent `IMR_QUERYCHARPOSITION` to the window, which the code answers with the caret's screen position (logged as `IMR_QUERYCHARPOSITION -> screen (197, 394)`), so the text services framework does ask for the caret.

A system caret (`CreateCaret`, `SetCaretPos`) tracks the page caret. Before it existed the emoji panel opened at the bottom right of the screen ([s0b-p2-popup-emoji-panel.png](media/winui-spike/s0b-p2-popup-emoji-panel.png) is the later run, with the caret); with it the panel opened higher up, still at the right of the screen, so this run did not show it following the caret.

### Input methods

- **Installed.** `C:\Windows\System32\IME` holds `IMEJP`, `IMEKR` and `IMETC`, and the Microsoft Japanese text input processor `{03B5835F-F03C-411B-9CE2-AA23E1171E36}` is registered. `Set-WinUserLanguageList` accepted `ja-JP` with the input method tip `0411:{03B5835F-...}{A76C93D9-...}` and the tray showed the IME's `A` (direct input) indicator with the cable off. No download was started.
- **Not usable from outside the machine.** The page never received `WM_IME_STARTCOMPOSITION` or `WM_IME_COMPOSITION`. `Alt+Shift` changed the language, but typing `nihon` produced Latin letters. With `--imeon` the app called `LoadKeyboardLayoutW("E0010411", KLF_ACTIVATE)`, which returned the current layout (`HKL(0x04090409)`), and `ImmSetOpenStatus` and `ImmSetConversionStatus` succeeded without any effect on typing, so the thread kept the US-International layout. Alt with the backtick key, `Ctrl+Space` and the layout switch did not turn the input method on. Whether the half-installed Japanese IME works at all in this guest is therefore open. The IMM32 code (`WM_IME_COMPOSITION`, `ImmSetCompositionWindow`, `ImmSetCandidateWindow` with `CFS_EXCLUDE`, `IMR_QUERYCHARPOSITION`, `WM_CHAR` surrogate pairs) is written and compiles, and none of it was exercised.

### UI Automation

The AccessKit adapter answered `WM_GETOBJECT` (`WM_GETOBJECT answered by AccessKit` in the log). `uia.ps1` (`System.Windows.Automation`, run in the desktop session through a scheduled task) found the element by `AutomationId` `page` and read:

```text
element: name='Quill page' class='QuillPageChild' type=ControlType.Edit rect=100,294,1750,910
DocumentRange text: Dear Reader,\nType here: a dead key then e, Alt+0233, or Win+. for an emoji.\nHi
selection ranges: 1
```

So the text pattern's `DocumentRange` returns the document text exactly. The script then stopped at the first call on the selection range (`GetText` or `GetBoundingRectangles`); the output file has nothing after `selection ranges: 1`, and the log shows the page taking focus again (`WM_IME_SETCONTEXT`, `page: focus gained`) just after. Whether that was a hang in the provider, in PowerShell, or an exception that was not logged was not determined before the VM had to be stopped. Selection, caret rectangles, lines, words and `RangeFromPoint` are unread.

### P1

`TextBox` in Reactor 0.100.0 has `text`, `on_text_changed`, `accepts_return`, opacity and `request_focus`, and nothing for selection, caret, composition or key events; `AutomationExt` has only name, id and heading level; `SwapChainPanel` cannot take focus. The P1 build therefore treats a hidden single-line `TextBox` as the document (opacity 0, placed near the caret, `automation_id` `page`), draws the page from its text, reverts the box when a character has no 1252 form, and probes `CoreTextServicesManager::GetForCurrentView`. It was built and never started in the guest. What follows from the API alone: the caret cannot be read or set, so editing in the middle of the text, arrow keys and clicks cannot work through the proxy, and a refused character resets the box's caret to the start. The UI Automation text pattern would be the `TextBox`'s own.

### Go/no-go

- **P2 as a child window: no-go.** It is invisible.
- **P2 as an owned pop-up with IMM32 and AccessKit: promising, not proven.** Typing, dead keys and Alt codes work, AccessKit exposes an `Edit` with a correct document text, and TSF queries the caret position. Open: input method composition and the candidate window, the rest of the text pattern, Narrator, Tab handover, and whether `ContentDialog` and flyouts draw over a pop-up that sits above the owner.
- **P1: unproven and weak by API.** Reactor 0.100.0 gives no selection or caret on the proxy.
- **Option B** (Win32, DirectComposition, Direct2D, DWM Mica, IMM32, AccessKit) removes the airspace problem by construction, because there is one window and no XAML island. The evidence here, a child window of a XAML window being invisible and the pop-up being a polling workaround, is the argument for it.

The recommendation is to repeat S0b's open items once, on a machine with RAM to spare, before committing W3 to the pop-up; if the pop-up fails the dialog and Tab checks, take Option B.

### Steps to repeat the open checks

1. Start `quill-winui-s0b.exe --p2 --popup` from the zip's folder in the interactive session; dismiss any toast first, because a toast can take the keyboard (the host's keystrokes went to it twice) and `Alt+Tab` to the app.
2. Turn on a Japanese input method by hand (the language bar or `Win+Space`), type `nihon`, and capture the composition and candidate window.
3. Run `uia.ps1` and `Win+.` with an emoji chosen by hand. Read the log for `refused via`.
4. Narrator: `Win+Ctrl+Enter`, click the page, then `Caps Lock+Right`/`Left` for the character, `Caps Lock+L` for the line; compare with the log's `WM_GETOBJECT` lines. Inspect.exe or Accessibility Insights show the `Edit` and its text pattern.
5. `Tab` from the page logs `Tab leaves the page`; check the focus that follows. `View > Open dialog` and the Flyout button show the airspace result against the pop-up.

### What is not proven

Everything marked "not run" above, and: an IME of any kind, `CoreTextEditContext` on this VM, Narrator, selection and caret through UI Automation, Tab handover, `ContentDialog` and flyout order against the pop-up, pop-up behaviour while the owner moves, resizes or is minimized, and 100% and 150% scales.
