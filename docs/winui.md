# Quill on Windows 11

What is planned for a native Windows 11 build of Quill, and why. The behaviour of each client is in [SPEC.md](../SPEC.md). The decisions made here are numbered D160 to D169 below and move into [decisions.md](decisions.md) when the open branches that also append to it have merged. The range is D160 to D169 because D100 to D105 are the Windows 9x decisions, D120 to D129 belong to the DOS dialogs and polish, D130 to D149 to DOS graphics, D150 and D151 to the flow work, D170 to the About box, and D180 to D199 to Linux ([linux.md](linux.md)).

Claims are marked **[V]** when a source or the repository confirms them and **[U]** when they are unverified. The spikes in the work list exist to settle the unverified ones.

## Table of contents

- [Recommendation](#recommendation)
- [WinUI 3 from Rust in 2026](#winui-3-from-rust-in-2026)
- [The alternatives](#the-alternatives)
- [Architecture](#architecture)
- [Build and test](#build-and-test)
- [Work list](#work-list)
- [Decisions](#decisions)
- [Risks and what is not proven](#risks-and-what-is-not-proven)

## Recommendation

**Refreshed on 2026-11-27** for the owner's answer that the client is "a real WinUI window", not a Win32 window imitating WinUI: the route, the S0c measurements, the control strategy, printing, clipboard, packaging, testing and the packages are in [plans/winui-real-window.md](plans/winui-real-window.md) (WUI-85 to WUI-94), which wins where this page disagrees. In short: a WinUI 3 window in Rust over Quill's own `windows-bindgen` projection of the WinUI metadata (Reactor cannot make a custom automation peer or receive text composition), the page in a `SwapChainPanel` inside a composed control with no child or pop-up window, `CoreTextEditContext` for typed text and input methods, a composed automation peer with the text pattern for Narrator, and WinUI's own controls for every piece of chrome. Option B below is no longer a fallback (WUI-85). The rest of this page is the original plan, kept for its sources.

Build a real WinUI 3 client, but only after two spike PRs say yes. In May 2026 Microsoft merged **Windows Reactor**, a WinUI 3 library written in Rust, into windows-rs. It is in preview, at version 0.100/0.101. It has `SwapChainPanel`, `MenuBar`, `CommandBar`, `ComboBox`, keyboard accelerators and automation metadata, and it needs no XAML compiler, MSBuild or .NET. A Rust WinUI 3 window is an ordinary Cargo build, not a C# project.

Three things are unproven, and the spikes settle them:

1. **IME input into a page Quill draws itself.** The WinRT text API for custom edit controls was reported broken in WinUI 3 desktop apps, and Microsoft closed that report as not planned.
2. **Narrator reading that page** through the UI Automation text pattern.
3. **A Linux-built self-contained zip** starting in a clean Windows 11 machine with no network.

WinUI never draws the page. Layout uses `OutlineMeasure` at 1440 units an inch, the same as the Linux client. The same shared `Surface` painter (`quill-raster`) draws it, and the result is shown one to one at device pixels. Measuring and drawing therefore use one engine, and the Windows 95 caret drift cannot come back.

**Fallback if the spikes fail:** a Win32 window with DirectComposition, Direct2D and DirectWrite, plus Mica, rounded corners and dark mode set through DWM, and custom-drawn Fluent-style chrome. It uses the same page and the same `Surface`, and it still covers Windows 11. *Superseded by WUI-85: the fallbacks are now real WinUI routes (another text input in the same window, S0b's pop-up page, or a XAML Islands host), and this imitation returns only by the owner's word.*

The Linux GTK4 client shares this modern design: the same menu tree, command rows and paper rules, so the two clients look like one product on two platforms.

## WinUI 3 from Rust in 2026

### Rust projections

- **[V] Windows Reactor.** windows-rs PR #4479, merged 28 May 2026. It is a declarative WinUI 3 library with "55+ widgets" and needs the Windows App SDK 2.0.1+ runtime on Windows 10 1809+ or Windows 11.
  - [V] A later PR is titled "third preview", and the README says public APIs may change.
  - [V] docs.rs 0.100.0 lists `SwapChainPanel` (events `Metrics{width, height, scale_x, scale_y}` and `Rendering`), `MenuBar`, `CommandBar`, `ComboBox`, `TextBox`, `AutomationExt` and `KeyAccelerators`. Only about 14% of the crate is documented.
  - [U] It is not documented whether Reactor exposes the raw `Microsoft.UI.Xaml` objects or the window's handle.
  - Sources: <https://github.com/microsoft/windows-rs/pull/4479>, <https://github.com/microsoft/windows-rs/issues/4483>, <https://docs.rs/windows-reactor/latest/windows_reactor/>, <https://docs.rs/windows-reactor/latest/windows_reactor/enum.SwapChainPanelEvent.html>
- **[V] Crate sources.** Reactor's `Cargo.toml` (0.101.0) has MSRV 1.95 and depends only on windows-rs crates, including `windows-window`. Its bindings are pre-generated (`native/bindings.rs`), so the build needs no `.winmd` metadata files. The windows-rs README also lists `windows-canvas` (Direct2D), `windows-text` (DirectWrite), `windows-composition`, `windows-pickers` and `windows-reactor-setup` ("Stages the Windows App Runtime"). Source: <https://github.com/microsoft/windows-rs>
- **[V] How Reactor starts the runtime.** `native/bootstrap.rs` uses the OS dynamic-dependency API `TryCreatePackageDependency` for the family `Microsoft.WindowsAppRuntime.2_8wekyb3d8bbwe`, with a minimum of 2.6.4 on master. That API exists only on Windows 11 (build 22000 or later), so in the framework-dependent mode Reactor is Windows 11 only. Sources: <https://raw.githubusercontent.com/microsoft/windows-rs/master/crates/libs/reactor/src/native/bootstrap.rs>, <https://learn.microsoft.com/en-us/windows/win32/api/appmodel/nf-appmodel-trycreatepackagedependency>
- **[V] `windows-reactor-setup` (self-contained mode).** It downloads `Microsoft.WindowsAppSDK.Runtime` and WebView2 from nuget.org with `%SystemRoot%\System32\curl.exe`, extracts `MSIX/win10-x64/Microsoft.WindowsAppRuntime.2.msix` with `tar.exe`, copies the listed files next to the exe and embeds a modified `app.manifest` through linker arguments. It does no checksum check, and its hard-coded `curl.exe` and `tar.exe` paths mean it does not run on a Linux host as is. Source: <https://raw.githubusercontent.com/microsoft/windows-rs/master/crates/libs/reactor-setup/src/lib.rs>
- **[V] Other Rust routes.** `winio-winui3` implements `IApplicationOverrides` in Rust, which shows the composable "outer object" pattern works for `Application` (<https://docs.rs/crate/winio-winui3/0.3.6/source/src/xaml_app.rs>). The older WinUI 3 support in winrt-rs was dropped, according to commenters on microsoft-ui-xaml #2488 (<https://github.com/microsoft/microsoft-ui-xaml/issues/2488>).
- **[V] Slint does not use WinUI.** Its `fluent` style only imitates Fluent (<https://docs.slint.dev/latest/docs/slint/reference/std-widgets/style/>).
- **[U] Microsoft's stance.** No statement was found that Rust is a supported WinUI language. Reactor is the closest to one.

### What a Rust WinUI 3 app needs

- **[V] Windows App SDK versions.** The SDK is 2.7.0 (17 November 2026). The `Microsoft.WindowsAppSDK.Runtime` package is 164.7 MB and `Microsoft.WindowsAppSDK.WinUI` is 59.5 MB. Version 2.0 moved to SemVer and split into sub-packages. Sources: <https://www.nuget.org/packages/Microsoft.WindowsAppSDK>, <https://www.nuget.org/packages/Microsoft.WindowsAppSDK.Runtime>, <https://www.nuget.org/packages/Microsoft.WindowsAppSDK.WinUI>, <https://learn.microsoft.com/en-us/windows/apps/windows-app-sdk/release-notes/windows-app-sdk-2-0>
- **[V] Self-contained deployment.** It needs no framework MSIX and no bootstrapper. Unpackaged self-contained apps need "undocked registration-free WinRT" initialization, and C++ apps need the hybrid CRT. The result can be xcopy-deployed. Source: <https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/self-contained-deploy/deploy-self-contained-apps>
- **XAML.** Reactor builds the UI in code. `XamlReader.Load` is a possible escape hatch [U for Rust]. The `.xaml` compiler path is out: it brings in MSBuild and a Visual Studio project.
- **MSIX and signing.** `makeappx` and `signtool` come from the Windows SDK, so they run in CI or a Windows machine. [V] `osslsigncode` signs APPX files (<https://github.com/mtrojnar/osslsigncode>); [U] MSIX by name. [U] Packing on Linux with `makemsix` relies on community wrappers.
- **[V] Minimum OS.** WinUI 3 targets Windows 10 1809 (17763), but there are reports that newer SDKs fail on 1809 LTSC (<https://github.com/microsoft/WindowsAppSDK/issues/5519>). Mica as a DWM attribute needs build 22621 (<https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/ne-dwmapi-dwmwindowattribute>).
- **Building from Linux.** [V] `cargo xwin` builds MSVC targets, and windows-rs links through `windows-link` with no import libraries (<https://docs.rs/crate/cargo-xwin/latest/source/README.md>). [U] That Reactor itself builds under cargo-xwin; spike S0a checks it. The runtime files are staged by Quill's own host script: download, pin a SHA-256, unzip the nupkg, then unzip the MSIX. Nothing has to be built on Windows.
- **Maturity risks.** Reactor is in preview with a moving API. Its PR thread reports a "Class not registered" error at start and a black flash before Mica appears.

## The alternatives

| Option | Verdict |
|---|---|
| **A. WinUI 3 window (Reactor or raw bindings); chrome in WinUI controls; page in a `SwapChainPanel` (P1) or Quill's own child window inside the WinUI window (P2)** | **Recommended.** Refined as route H1 (own projection, `SwapChainPanel` in a composed control) with F1 and F2 behind it ([plans/winui-real-window.md](plans/winui-real-window.md#the-route-and-its-fallbacks)) |
| B. Win32 + DirectComposition/Direct2D/DirectWrite, DWM Mica (`DWMWA_SYSTEMBACKDROP_TYPE`, 22621+), dark title bar (`DWMWA_USE_IMMERSIVE_DARK_MODE`, 22000+), rounded corners by default, custom-drawn Fluent toolbar, Win32 menus and common dialogs | **Out** (WUI-85): it only imitates WinUI, which the owner ruled out on 2026-11-27. Cross-builds fully from Linux and needs no runtime |
| C. A Win32 window hosting WinUI 3 through XAML Islands (`DesktopWindowXamlSource`, no longer experimental since SDK 1.4) | Last real-WinUI fallback (F3): real WinUI chrome with a Win32 page beside the islands. [U] whether pop-ups and flyouts escape the island |
| D. Windows.UI.Xaml (UWP) XAML Islands | No. A dead-end stack |
| E. Slint or iced with a Fluent theme | No. Imitation without the advantages of B |
| F. `QUILL32.EXE` as it is | A baseline, not a Windows 11 client (below) |

Source for C: <https://learn.microsoft.com/windows/apps/windows-app-sdk/release-notes/windows-app-sdk-1-4>

### How `QUILL32.EXE` looks on Windows 11 today

- [V from the repository] It has no manifest, uses ANSI APIs and assumes 96 dpi.
- Windows therefore treats it as DPI-unaware and bitmap-stretches it: the right size at 150%, but blurry ([V] Microsoft's description of DPI-unaware apps: <https://learn.microsoft.com/en-us/windows/desktop/hidpi>).
- Without a comctl32 v6 manifest, its controls use the classic unthemed look inside a Windows 11 frame [U until screenshotted; `docs/winui-spike.md` records the baseline].
- Names with characters outside Windows-1252 become `?`.
- A cheap later improvement: a `<gdiScaling>true</gdiScaling>` manifest for crisper GDI text. Windows 9x ignores manifests [U on the exact element].

## Architecture

### Where it lives and how it plugs in

- **Workspace.** `winui/quill-winui` (own workspace, D30), target `x86_64-pc-windows-msvc`, with `std` and stable Rust 1.98.
- **Shell.** The app is `App<OutlineMeasure>`, driven exactly as the Win32 `Shell` struct drives `App<GdiMeasure>`: through `command`, `key`, `take_dialog`/`set_native_dialogs(true)` (D86), `for_each_line`, `walk_chars` and `caret_xy`.
- **No new painter.** The shell presents what `quill-raster::render_page` produces, which becomes the shared `Surface` painter in C3.
- **Open question.** The DOS graphics plan puts `Surface` in `crates/quill-paint`, and the Linux plan in `quill-raster`. That has to be settled before W3.

### One text engine

- Layout widths always come from `OutlineMeasure` (skrifa, never hinted, 1440 units an inch, D186).
- The font bytes are the files DirectWrite resolves from the system font collection, so the font box lists only faces Quill can load.
- Drawing places every glyph at the layout's x. The v1 drawing is the pure-Rust raster (greyscale antialiasing), so pixel goldens run on the host and match Linux.
- An optional later `Surface` over Direct2D `DrawGlyphRun` would pass explicit advances from the layout, so only the rasterizing differs, never the positions.
- The `CapturedMeasure` width tables stay a "Windows check" mode only.
- Linux and Windows 11 break lines identically for the same font files.

**Unicode** (the core stays Windows-1252, D11)

- **Typed and IME text.** UTF-16 is converted to 1252 through C4's public table. A character with no 1252 form is refused with a status message, as D188 does on Linux.
- **File names.** The shell keeps the real UTF-16 path in a name table, and the app sees a 1252 display name, as `LinuxFiles` (D183) does.
- **Safe save.** Write a temporary file, then `ReplaceFileW` or `MoveFileExW`.
- **Settings and recent files.** `%APPDATA%\Quill\QUILL.INI`, with a `[RecentWinUI]` section holding UTF-16 paths.
- **Clipboard** (after C1): `CF_UNICODETEXT`, `Rich Text Format` (`\ansicpg1252`) and `CF_DIB`.

### Display

- **DPI.** Per-monitor v2 manifest. The scale comes from `SwapChainPanel` `Metrics` and is rounded to 1/120ths for the existing `Mapping`.
- **Half-pixel offset.** The GTK spike's half-pixel lesson applies: spike S0a measures it with a checkerboard. [U] whether XAML layout rounding is enough.
- **Dark mode.** The chrome follows the system and the paper stays white (the `[Quill] Paper=` option from the Linux plan). Mica is set through `SystemBackdrop`.
- **Snap layouts** work with the standard caption. A custom title bar is avoided in v1.

### Chrome and dialogs

- **Menus.** `MenuBar` built from the `quill-app` menu tree (Write's menus, D17), with mnemonics after #21.
- **Toolbars.** Two `CommandBar` rows mirroring D85, with `ComboBox`es for style, font and size, and a colour flyout.
- **Ruler.** A custom strip drawn by the painter, which needs C2 (resolution from `Measure`, not 96).
- **Dialogs.** D86 dialogs are generated from `assets/dialogs/dialogs.json`.
- **File pickers.** `Microsoft.Windows.Storage.Pickers`, built with `AppWindow.Id` (SDK 1.8+, works elevated) [V] (<https://learn.microsoft.com/windows/apps/develop/files/using-file-folder-pickers>). The older WinRT pickers need `IInitializeWithWindow` [V] (<https://learn.microsoft.com/en-us/windows/apps/develop/ui/display-ui-objects>). Fallback: `IFileOpenDialog`.

### Printing

Superseded by WUI-92: the Windows 11 print dialog through `PrintManagerInterop` with vector pages drawn by `paint_sheet` on a Direct2D surface, and Export as PDF through `quill_pdfdoc::export` as on GTK ([plans/winui-real-window.md](plans/winui-real-window.md#printing-preview-and-export-as-pdf)). The original plan:

- Win32 `PrintDlgExW` and a GDI printer DC, with page bands rendered by `quill-raster` at the printer's dpi.
- `PrintManagerInterop` with XAML `PrintDocument` is later and [U] (<https://learn.microsoft.com/en-us/windows/uwp/devices-sensors/print-from-your-app>).

**IME and Narrator** (the hard part, settled by spike S0b)

- **P1, a page in a `SwapChainPanel`:**
  - IME through `CoreTextEditContext`. [V] Getting it fails on WinUI 3 desktop (<https://github.com/microsoft/microsoft-ui-xaml/issues/4239>, closed as not planned). Fallback: a hidden `TextBox` proxy placed at the caret.
  - Narrator through a custom automation peer with `ITextProvider`/`ITextRangeProvider`, using the composable outer-object pattern [U].
- **P2, Quill's own child window over the page area:**
  - IME through IMM32 (`WM_IME_COMPOSITION`, `ImmSetCompositionWindow`).
  - UI Automation through AccessKit. [V] AccessKit's Windows adapter implements `ITextRangeProvider` (`adapters/windows/src/text.rs`, accesskit_windows 0.35.2, windows-core 0.62).
  - Risks: in-XAML pop-ups (`ContentDialog`) might be hidden behind the child window [U], and focus handover between XAML and the child window [U].
- **Touch and pen.** Pointer events go to `pointer.rs` (pen acts as a mouse, touch drag scrolls, a tap places the caret).

**Shared with the Linux client:** `quill-fonts`, `quill-raster`/`Surface`, `Mapping`, the 1252 table (C4), the name-table rule and the clean mark (Q1a).

## Build and test

### On the Linux host

- **Already there.** `cargo-xwin` is installed, with `xwin`, `clang-cl` and `cmake` in `~/.cache/cargo-xwin` [V].
- **`winui/tools/stage-runtime.sh`.** Downloads `Microsoft.WindowsAppSDK.Runtime` at a pinned version from `https://www.nuget.org/api/v2/package/...` (164.7 MB, once, into `~/.cache`) and checks a SHA-256 kept in the repository. It unzips the package, then copies Reactor's `runtime.txt` list from the framework MSIX next to the exe, and writes the manifest: per-monitor v2, supported OS IDs, and the registration-free WinRT entries.
- **Linking.** Static CRT (`+crt-static`).
- **Metadata.** `Microsoft.WindowsAppSDK.WinUI` (59 MB) is needed only if Reactor falls through and bindings are generated from its `.winmd` files with `windows-bindgen`.
- **Build output.** Set `CARGO_TARGET_DIR` outside the repository.

### In a Windows 11 test machine

- **Driving it.** A VM driven from the host, with only the zip copied in and the network cable off.
- **Running in the desktop session.** GUI runs go through a scheduled task created with `schtasks /create /it /ru <user>` and started with `schtasks /run`, so the app appears in the auto-logged-on session. [U] whether processes started by the hypervisor's guest-control are visible on the desktop.
- **Screenshots.** The hypervisor's own framebuffer capture, never the host desktop.
- **150% scale.** Set through the guest's per-user DPI registry values and a sign-out [U on the exact keys].
- **Input.** The runtime is staged on the host, so no network is needed. IME tests use US-International dead keys, Alt+0233 and Win+. (the emoji panel, which goes through TSF and tests the refusal). A Japanese IME needs a download and is skipped.
- **Masking caveat.** A development VM with Rust installed probably has the VC++ runtime, which hides a CRT dependency. A clean-machine check happens in CI.

**CI** (no open PR touches `.github`, so this conflicts with nothing) [V]

- A `winui` job on `windows-latest`: `cargo build --locked`, the shell's unit tests, and a `/SELFTEST` play script that opens `tests/docs/letter.rtf`, saves it and compares bytes with `tests/golden/files`.
- A PowerShell UI Automation smoke test that reads the page's text pattern. [U] whether the hosted runner's session allows it.
- The zip is uploaded as an artifact.

### Tests

- **Host pixel goldens** of the page through `quill-raster`, shared with Linux. No new goldens are needed for Windows.
- **Property tests** at the scales Windows offers (100, 125, 150, 175, 200, 225, 250, 300%): the caret x equals the drawn glyph x, and `pos_at_point` round-trips.
- **Host tests** of the UTF-16 conversion, the name table and the VirtualKey-to-`Key` mapping. They live in a std module with no WinRT, so they run on Linux.

### Packaging, in order

1. An unpackaged self-contained zip.
2. MSIX built with `makeappx` and a test certificate in CI, sideloaded on a fresh snapshot.
3. A winget manifest of type zip/portable (supported since winget 1.5 [V], <https://docs.rs/winget-types/0.4.2/src/winget_types/installer/mod.rs.html>), drafted but not submitted.
4. A Liminal Installer target: the installer already has a `msix` output and a `win11`/`fizz-modern` look [V installer `SPEC.md`]. The same `installer/` project carries both targets: the Windows 95 floppy `SETUP.EXE` installs `QUILL32.EXE`, and the Windows 11 per-user setup or MSIX installs the WinUI build.

## Work list

From W3 on, the items are ordered as work packages `WP-WINUI-1` (a hosting decision, S0c, since S0b did not reach a go) to `WP-WINUI-12` in [plans/winui-real-window.md](plans/winui-real-window.md#work-packages), with their dependencies and builder briefs (WUI-1, WUI-2, WUI-85 to WUI-94); [ui-track.md](ui-track.md) orders them beside the GTK packages. The table below is the original plan.

Each item is sized for one pull request. Decisions D160 to D169 are kept here until they move into `decisions.md`.

| PR | Contents | Acceptance |
|---|---|---|
| **W0** `docs/winui` | This document; reserves D160 to D169; no SPEC or `decisions.md` edits | markdownlint passes |
| **S0a** `spike/winui-s0a` | New `winui/spike` workspace: a Reactor window with `MenuBar`, a `CommandBar` with a `ComboBox`, and a `SwapChainPanel` showing a one-pixel checkerboard and a page drawn by skrifa and tiny-skia; `stage-runtime.sh`; baseline screenshots of `QUILL32.EXE` on Windows 11 | Built on Linux with `cargo xwin`; the zip starts on a clean Windows 11 machine with the cable off; the checkerboard is 100% exact at 100% and 150% (by the G0 analysis method, on framebuffer frames); menus and the combo box open over the page; startup time and zip size recorded |
| **S0b** `spike/winui-s0b` | Input and accessibility for P1 (CoreText, then a hidden `TextBox` proxy; a custom automation peer through the outer-object pattern) and P2 (child window, IMM32, AccessKit) | For at least one hosting: the dead key and Alt+0233 log 0xE9; an emoji is refused; the candidate window sits at the caret; a PowerShell UI Automation script reads `DocumentRange` text and the caret; Narrator reads the line (manual); Tab moves between the command bar and the page; no dialog or flyout is hidden. Results in `docs/winui-spike.md` |
| W3 | `winui/quill-winui` viewer: manifest, `WinUIFiles` with the name table and safe save, INI, open from the command line and the picker, scroll, zoom, page from `quill-raster` | Opens every file in `tests/docs`; `/LINES` equals `letter.outline-1440.txt`; screenshots at 100% and 150% |
| W4 | Editing: keys, `accel`, the text input chosen in S0b, caret, `pointer.rs`, touch and pen | The host editing scripts through the shell's command layer save identical bytes; a SendInput session matches `tests/golden/files` |
| W5 | Menus, accelerators, enabled and checked state, D86 dialogs, pickers | Every `Dialog` variant is shown and answered (a test hook like `native.rs`); the UI Automation script finds each one by title |
| W6 | Command bars, font, size and style boxes, colour flyout, ruler, status line | The same commands as the host chrome tests; light and dark screenshots |
| W7 | Clipboard | Round trip through `Get-Clipboard -TextFormatType Rtf` and Unicode text keeps bold and refuses non-1252 characters |
| W8 | Dark mode, Mica, high contrast, snap layout check, paper option | Screenshots; snap layouts flyout appears |
| W9 | Full text pattern and Narrator checklist | The UI Automation script reads lines, selection and caret moves; manual Narrator pass |
| W10 | Printing and preview | Microsoft Print to PDF output, compared with `pdftoppm`, is within tolerance of the raster golden |
| W11 | CI job, zip artifact, MSIX with a test certificate, winget draft, installer target | actionlint passes; CI green; the MSIX installs on a fresh snapshot |
| W12 | SPEC rows, decisions moved into `decisions.md`, `docs/media` screenshots | markdownlint passes; SPEC rows added |

W3 starts after the spikes say go, after P4 (`OutlineMeasure`), P5 (`quill-raster`) and C3 (`Surface`) land, and after the `Surface` home is decided. W4 follows W3 and C4, W5 follows W4 and the menu-mnemonics work (#21), W6 follows W5 and C2, W7 follows C1, W8 follows W6, W9 follows W4, W10 follows W6, W11 follows W5, and W12 follows the decision-file PRs.

### Go/no-go after S0b

- **Go** if S0a passes and one hosting (P1 or P2) passes IME, the UI Automation text pattern and focus handover.
- If Reactor itself fails (it does not build under cargo-xwin, does not start self-contained, or gives no access to the underlying elements), retry once with raw `windows-bindgen` bindings in the `winio-winui3` style.
- **No-go** switches W3 onward to option B (Win32 + DirectComposition + Direct2D, DWM Mica and dark mode, custom-drawn chrome, IMM32, AccessKit). The page, `Surface`, the measure and the tests stay the same.

### Order relative to the Linux GTK4 client and the shared painter

- W0, S0a and S0b run in parallel with the Linux and graphics work. They touch only `winui/` and `docs/winui*.md`, and avoid `SPEC.md`, `decisions.md`, `quill-app/src/lib.rs`, `view.rs` and the root `Cargo.toml`, which other open work changes.
- W3 onward follows the Linux order. Windows 11 must not add a third painter; it joins GTK as a second user of C3.

## Decisions

Proposed, and recorded here as D160 to D169 until they move into `decisions.md`.

- **D160. Real WinUI 3, gated by the spikes.** The Win32 Fluent build (option B) is the fallback. *The fallback is superseded by WUI-85: real WinUI fallbacks only.*
- **D161. Reactor first, raw `windows-bindgen` bindings second.** *Reversed by WUI-86: Quill's own projection first, because Reactor 0.100.0 and its master have no custom automation peer, no text composition and no access to the underlying elements; Reactor stays the spikes'.*
- **D162. An unpackaged self-contained zip first, then MSIX.** Not framework-dependent: that needs a runtime install, and Reactor's bootstrap is Windows 11 only.
- **D163. Windows 11 22H2 (22621) or later and x64.** Windows 10 is untested, and ARM64 comes later.
- **D164. Draw with the shared raster.** Glyphs are placed at layout positions; Direct2D drawing is optional later.
- **D165. `OutlineMeasure` at 1440 units an inch,** as on Linux (D186).
- **D166. Characters with no 1252 form are refused when typed and become `?` when read** (the D188 rule).
- **D167. Workspace `winui/quill-winui`, binary `Quill.exe`,** app ID `LiminalHQ.Quill` unless a domain is chosen.
- **D168. A classic `MenuBar` plus two `CommandBar` rows,** no ribbon. *The boxes go in `AppBarElementContainer`s (WUI-90).*
- **D169. Print through `PrintDlgExW` and GDI** with raster bands. *Superseded by WUI-92.*

Still open, and not numbered: the single home for `Surface` (`quill-paint` or `quill-raster`), to be picked before W3.

## Risks and what is not proven

The refreshed list, with what S0c settles, is in [plans/winui-real-window.md](plans/winui-real-window.md#what-s0c-must-measure). Since this page was written: S0a settled Reactor under cargo-xwin, the self-contained start and the runtime file list; S0b found a child window under XAML invisible; Reactor's master gives the window handle but still no custom automation peer or text composition; a third-party WinUI 3 editor reports `CoreTextEditContext` working in an unpackaged desktop window [U for Quill].

- **Rust WinUI maturity.** Reactor is a preview with 14% of its docs written. Startup errors ("Class not registered", a black flash before Mica) are reported in its PR thread.
- **Custom-page IME and the text pattern** are unproven in WinUI 3, and the CoreText route is known to be broken.
- **Windows App SDK dependency.** About 165 MB downloaded per version, a runtime folder shipped beside the exe, the hybrid CRT question, and a development VM that may hide a missing VC++ runtime.
- **MSIX friction.** It needs Windows tooling, and sideloading needs a trusted certificate, which means elevation.
- **ANSI core against a Unicode OS.** Refused characters, `?` file names and paste losses.
- **Airspace** in P2.
- **Two painters** if C3 slips.
- **Costs of testing.** Booting the VM writes about 11 GB to its differencing disk, and the host needs about 0.3 GB of NuGet cache plus a build directory of perhaps 1 to 3 GB [U].

Not verified: Reactor building under cargo-xwin or running self-contained; whether Reactor gives access to the underlying elements or the window handle; CoreText on current SDKs; the composable automation peer pattern from Rust; pop-up and airspace behaviour; the size of the runtime file list; `makemsix` packing on Linux; guest-session visibility of guest-control processes and the DPI registry recipe; the WinUI 3 smoke test on the GitHub runner; the exact `gdiScaling` element; and whether the Windows 11 build under test has WordPad (24H2 removed it).
