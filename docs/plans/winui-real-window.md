# A real WinUI 3 window for Quill

The plan for Quill's Windows 11 client as a genuine WinUI 3 application: a `Microsoft.UI.Xaml.Window` with WinUI's own controls for every piece of chrome, and the page drawn by the shared painter inside it. The owner, 2026-11-27: the WinUI client should be "a real WinUI window", not a plain Win32 window imitating WinUI. This page refreshes [winui.md](../winui.md) and the WinUI half of [ui-track.md](../ui-track.md) for that answer: the route, what the first measurement package (S0c) must test on the Windows dev VM to settle it, the control strategy, the Windows 11 look, accessibility, printing, clipboard, packaging, testing, the work packages `WP-WINUI-1` to `WP-WINUI-12` in build order, and their briefs, the first in full.

Written on 2026-11-27 against `origin/main` at `aaacc62`, with spikes S0a and S0b merged ([winui-spike.md](../winui-spike.md)), `quill-pdf` and `quill-pdfdoc` merged, and the architecture splits (`quill-session`, `quill-input`, `quill-menu`, `quill-settings`, `quill-flow`, `quill-a11y`) still ahead in [work-packages.md](../work-packages.md). Claims are marked **[V]** when a source or a file was read and **[U]** when unverified; S0c exists to settle the **[U]** items that decide the route. Nothing here was built or run, and the VM was not started. Decisions WUI-85 to WUI-94 are reserved in [id-ranges.md](../id-ranges.md) and recorded in [decisions.md](../decisions.md) as **Assumed**.

## Table of contents

- [The answer in short](#the-answer-in-short)
- [What the spikes already proved](#what-the-spikes-already-proved)
- [How Rust gets a genuine WinUI 3 window](#how-rust-gets-a-genuine-winui-3-window)
- [The route and its fallbacks](#the-route-and-its-fallbacks)
- [What S0c must measure](#what-s0c-must-measure)
- [Go and no-go](#go-and-no-go)
- [Control strategy](#control-strategy)
- [The Windows 11 look](#the-windows-11-look)
- [Text input](#text-input)
- [Accessibility](#accessibility)
- [Printing, preview and Export as PDF](#printing-preview-and-export-as-pdf)
- [Clipboard](#clipboard)
- [Packaging and deployment](#packaging-and-deployment)
- [What the client needs from the other lanes](#what-the-client-needs-from-the-other-lanes)
- [Testing](#testing)
- [Budgets](#budgets)
- [Decisions](#decisions)
- [Work packages](#work-packages)
- [Builder briefs](#builder-briefs)
- [Owner decisions](#owner-decisions)

## The answer in short

- **GO for a real WinUI 3 window, with one measurement package first.** The route (H1) is a WinUI 3 application in Rust over Quill's own `windows-bindgen` projection of the Windows App SDK's WinUI metadata: `Application::Start`, a `Microsoft.UI.Xaml.Window`, WinUI's `MenuBar`, `CommandBar`, `ContentDialog`, `NavigationView` and the rest for the chrome, and the page in a `SwapChainPanel` inside a composed `QuillPage` control, so there is no child window, no pop-up and no airspace. Typed text and input methods go through `CoreTextEditContext` on that control, and Narrator reads it through a composed automation peer with the UI Automation text pattern.
- **Why not Reactor alone.** Windows Reactor 0.100.0 proved the window, the runtime staging and the pixel-exact page (S0a), and its upstream master now gives the window handle, key and character events and a composition host. Neither version can create a custom automation peer, receive text composition events or hand out the underlying `UIElement` [V, read in the crate sources], so a page Narrator can read cannot be built on it. Quill therefore projects the WinUI types itself, as `naui-winui3` and `winio-winui3` do, and composes the three classes it must subclass (`Application`, the page control and its automation peer).
- **Fallbacks, in order, all real WinUI:** F1 the same window with another text input (a TSF text store, or a composed `TextBox` carrying the composition); F2 S0b's owned pop-up page over the WinUI window (IMM32 and AccessKit, measured partly); F3 a XAML Islands host (Quill's Win32 frame, WinUI controls in `DesktopWindowXamlSource` islands, the page in a sibling window). Option B, a Win32 window that imitates Fluent, is no longer a fallback (WUI-85); it comes back only if the owner says so after every real route has failed.
- **S0c decides** with one build of H1 on the Windows dev VM: the window handle and focus, typing, dead keys, Alt codes and a Japanese or Korean input method through `CoreTextEditContext`, the text pattern through the composed peer, Narrator, Tab handover, dialogs and flyouts over the page, moving and minimizing, 100, 150 and 175 % with a live change, Mica against no Mica, light and dark, the title bar and snap layouts, startup time, build time and the size of the self-contained zip.

## What the spikes already proved

| Question | Answer so far | Source |
|---|---|---|
| Can a Linux host build a WinUI 3 app? | Yes: Reactor 0.100.0 under `cargo xwin`, static CRT, about 15 s clean | S0a [V] |
| Does a self-contained zip start on a clean Windows 11 with the cable off? | Yes, at 100, 150 and 175 %; the Windows App Runtime 2.4.0 files staged on Linux with a pinned SHA-256 and a registration-free manifest of 980 classes | S0a [V] |
| Is a page in a `SwapChainPanel` pixel-exact? | Yes: 100.000 % of pixels equal at all three scales, best shift (0, 0) | S0a [V] |
| Do menus and combo box lists open over the page? | Yes, over a swap-chain page | S0a [V] |
| Size and start | 23.3 MB zip, 61.5 MB unpacked, 2.6 MB exe; 250 to 460 ms to the first frame in the VM | S0a [V] |
| A Win32 child window over XAML | Invisible: the island is composed above it | S0b [V] |
| An owned pop-up over the XAML window | Shows the page and takes typing, dead keys and Alt codes; AccessKit returns the document text; IME, the rest of the text pattern, Narrator, Tab and dialogs never run | S0b [V] |
| `CoreTextEditContext` in the WinUI window | Probe built, never run | S0b [V] |
| Turning a Japanese input method on from outside the guest | `LoadKeyboardLayoutW` and `ImmSetOpenStatus` did nothing; the input method is a TSF text service, so S0c opens it through TSF (below) | S0b [V] |

What changed since S0b: a third-party WinUI 3 editor that draws its own page, unpackaged on Windows App SDK 2.2.1 or later, reports that `CoreTextServicesManager.GetForCurrentView()` works in a WinUI 3 desktop window and composes Korean and CJK text through it (`RichEditor.Ime.cs` in [centwon/WinUIRichEditor](https://github.com/centwon/WinUIRichEditor), read on 2026-11-27) [V that the code and its comment say so; U for Quill]. The report Quill's plan rested on (microsoft-ui-xaml #4239) is from the early SDKs. Microsoft's printing guide for WinUI 3 desktop apps uses `PrintManagerInterop.GetForWindow(hwnd)` and `ShowPrintUIForWindowAsync(hwnd)` ([Print from your app](https://learn.microsoft.com/en-us/windows/uwp/devices-sensors/print-from-your-app)) [V from the search summary; U from Rust].

## How Rust gets a genuine WinUI 3 window

| Way | What it is | Custom automation peer | Text composition | Raw elements and handle | Verdict |
|---|---|---|---|---|---|
| **Reactor 0.100.0** (crates.io) | Microsoft's declarative WinUI 3 library in windows-rs, pre-generated bindings | No | No | No handle, no element | Proven for the window and page (S0a); cannot serve Narrator |
| **Reactor master** (windows-rs `cd03f93`, mirrored as `windows-reactor-pre` 0.108.0) | The same, later: needs runtime 2.6.4; `Context::run_window` gives the HWND; `on_preview_key_down`, `on_character_received`, `on_key_up`, pointer and focus events, `is_tab_stop`; `observe_composition_host` and `request_set_child_visual` on a `Grid`; `request_notification` (UIA notifications) | No | No | Handle yes, element no | Better for keys, still no text pattern; not released on crates.io [V, read] |
| **Own projection** (`windows-bindgen` over `Microsoft.UI.Xaml.winmd`) | Quill generates bindings for exactly the types it uses from the pinned `Microsoft.WindowsAppSDK.WinUI` package and composes subclasses with the outer-object pattern | Yes, by composing `FrameworkElementAutomationPeer` | Yes: `CoreTextEditContext` (Windows SDK, in the `windows` crate) or `TextBox` composition events | Everything; the HWND through `IWindowNative` | **The route (H1)** |
| `naui-winui3` 0.20.1, `winio-winui3` 0.7.0-alpha.8 | Third-party projections of WinUI 3 with a composition helper and self-contained bootstrap (`naui-winui3` is 400,843 lines of bindings) | Not included in either | `naui-winui3` has `TextCompositionStarted` and `CandidateWindowBoundsChanged` | Yes | References for the projection and the compose helper, not dependencies [V, read] |
| XAML Islands (`DesktopWindowXamlSource`) in Quill's own Win32 window | Real WinUI controls in islands; Quill owns the frame | Yes, if projected | Page in Quill's own window: IMM32 | Yes | Fallback F3 |
| `.xaml` markup and the XAML compiler | Needs MSBuild and a Visual Studio project | — | — | — | Out (D161 stands on this) |

**Packaged or unpackaged.** Unpackaged and self-contained first, as S0a did (D162): the app carries the Windows App Runtime files beside `Quill.exe`, initializes through undocked registration-free WinRT (the manifest S0a's `stage-runtime.sh` writes) and needs nothing installed. Framework-dependent unpackaged mode needs a runtime install and, through `TryCreatePackageDependency`, Windows 11 only; packaged (MSIX) gives package identity (`ApplicationData`, file types in the manifest, clean uninstall, Store and winget) and is the second channel. Package identity for an unpackaged build ("packaged with external location", a sparse package) is a later option, not needed for anything in this plan.

**The runtime version.** S0a pinned runtime 2.4.0 (what Reactor 0.100.0 asks for). The projection is generated from the `Microsoft.WindowsAppSDK.WinUI` package of the same release as the staged runtime, and both are pinned with SHA-256 in `winui/tools/`. S0c stays on 2.4.0 unless a check fails for want of a later one; moving to the current release (2.7.0, 17 November 2026) is one script change and a new hash.

## The route and its fallbacks

```text
H1  Microsoft.UI.Xaml.Window (own projection, unpackaged, self-contained)
    ├─ chrome: MenuBar, CommandBar + AppBarElementContainer, ContentDialog, Flyout, NavigationView, InfoBar, TextBlocks
    ├─ ruler:  SwapChainPanel drawn by quill-paint
    └─ page:   QuillPage (composed Control) ─ SwapChainPanel drawn by quill-paint, 1 buffer pixel = 1 device pixel
               ├─ keys, pointer, focus: the control's own overrides
               ├─ text input: CoreTextEditContext            (F1: TSF text store, or a composed TextBox)
               └─ Narrator: composed FrameworkElementAutomationPeer, ITextProvider2 + ITextRangeProvider

F2  the same WinUI window, page in an owned WS_POPUP window (S0b): IMM32, AccessKit
F3  Quill's Win32 window, WinUI chrome in XAML islands, page in a sibling child window: IMM32, AccessKit
```

Each fallback keeps the page, the painter, the measure, the core and every host test; what changes is the page's host module and its input and accessibility adapters. The shell is written so that the page host is one module behind a trait (`PageHost`: show a frame, place the caret, focus, raise text and selection changes), so a fallback replaces one module, not the shell.

## What S0c must measure

S0c (WP-WINUI-1) builds H1 as a spike (`winui/spike/src/s0c/`, with the projection in `winui/spike/xaml/`) and measures it in `WinDev2407Eval`. Every check runs at 100 % and 150 %, and the pixel check also at 175 %. A check is **pass**, **fail** or **not run**, with the log line or the framebuffer shot that proves it.

| # | Check | How | Gate |
|---|---|---|---|
| C1 | The projection builds on Linux | `windows-bindgen` over the pinned `.winmd`, filtered to the types used; `cargo xwin build --release --locked`; clippy clean | **Gate** |
| C2 | The window starts self-contained, cable off | The zip from `build.sh`, the runtime staged on the host; `Get-AppxPackage *WindowsAppRuntime*` recorded so a masked dependency shows | **Gate** |
| C3 | Window handle and focus | `IWindowNative::WindowHandle` logged; `QuillPage` takes focus by click, by Tab and programmatically; focus survives a menu, a flyout and a dialog closing; `GotFocus`, `LostFocus` and the window's `Activated` logged | **Gate** |
| C4 | Pixel-exact page | S0a's checkerboard and `analyse.py` through `QuillPage`'s swap chain; then a live scale change (100 to 150 % while running): the `CompositionScaleChanged` path rebuilds the chain and stays 100.000 % | **Gate** |
| C5 | Typing | `keyboardputstring`, US-International dead key then e, Alt+0233: each logs `TextUpdating` (or the key path) and `0xE9` on the page | **Gate** |
| C6 | Input method | Japanese (or, if it will not open, Korean) composition at the caret: `CompositionStarted`, `TextUpdating` with the composing range, `FormatUpdating` underline drawn, the candidate window beside the caret from `LayoutRequested`, the commit typed or refused by D166 with the count | **Gate** (one language is enough) |
| C7 | Emoji refused | Win+. and an emoji chosen with VirtualBox mouse clicks on the panel; the refusal message and no change to the document | Gate |
| C8 | UI Automation text pattern | `uia.ps1` extended: the page is `Edit` with name `Quill page`; `DocumentRange` text; `GetSelection`; `ITextProvider2.GetCaretRange`; `RangeFromPoint`; `Move` by character, word and line; `GetBoundingRectangles` within a pixel of the drawn glyphs; `TextSelectionChanged` and `TextChanged` events after keys. The script has a 10 s watchdog per call so a hang is a recorded fail, not a lost run | **Gate** |
| C9 | Narrator | Narrator on (Win+Ctrl+Enter), focus on the page, Caps Lock+Right, Caps Lock+L; evidence is Narrator's focus rectangle on the page in the framebuffer and the provider's log of the calls Narrator made (speech cannot be heard from the host; the owner's listening pass is listed as not verified) | Gate |
| C10 | Tab handover | Tab and Shift+Tab cycle menu bar, command bar, page, status bar; F6 moves between the regions; the order logged | Gate |
| C11 | Over the page | A `ContentDialog`, a `MenuFlyout`, a `Flyout`, a `ComboBox` list and a `ToolTip` all show above the page; none is clipped | **Gate** |
| C12 | Move, resize, minimize, maximize, snap | The page follows with no lag frame and no stale area (shots during a scripted resize); restore after minimize repaints | Gate |
| C13 | Mica and dark mode | `MicaBackdrop` against no backdrop, in light and dark (the guest's `AppsUseLightTheme` switched while running): the chrome changes, the paper stays white; `ActualThemeChanged` logged | Measured, not a gate |
| C14 | Title bar | The system title bar over Mica with the caption following the theme, then the same window with `ExtendsContentIntoTitleBar` and WinUI's `TitleBar`: drag, double-click, snap layouts flyout on the maximize button, Narrator's name for the window | Measured, not a gate (picks WUI-91's swap) |
| C15 | Startup | Process start to first presented frame, cold after a reboot and warm, three runs each; working set after the letter is shown | Budget |
| C16 | Size | Exe, zip and unpacked sizes; the number of runtime files; clean and incremental build times on the host | Budget |
| C17 | High contrast | Aquatic and Desert themes on: chrome follows, page uses the contrast colours (WUI-91) | Measured |
| C18 | Probes | `PrintManagerInterop.ShowPrintUIForWindowAsync` opens the Windows print dialog for the window; `Microsoft.Windows.Storage.Pickers.FileOpenPicker` with `AppWindow.Id`; `OpenClipboard` on the window handle with `CF_UNICODETEXT` | Probes for WP-WINUI-5, 7 and 10; a failure moves that package to its fallback, not the route |

**Turning the input method on.** S0b could not: the Japanese input method is a TSF text service, so `LoadKeyboardLayoutW` and IMM32 calls do not reach it. S0c's `--imeon` uses TSF: `ITfInputProcessorProfileMgr::ActivateProfile` with the Japanese profile the guest already registers (`0411:{03B5835F-F03C-411B-9CE2-AA23E1171E36}{A76C93D9-5523-4E90-AAFA-4DB112F9AC76}`; S0b logged the first half, the profile GUID is to be checked against the guest's registration), then the open-close compartment (`GUID_COMPARTMENT_KEYBOARD_OPENCLOSE`) set to 1. If that fails, the Korean input method (`IMEKR`, present) in Hangul mode composes syllables from plain keys, which `keyboardputstring` can type. A language bar click through the VirtualBox mouse tools is the last way. No download.

**If H1 fails a gate.** C6 or C5: try F1's text inputs in the same build (both are small: a TSF `ITextStoreACP2` associated with the window's input site while `QuillPage` has focus, and a composed `TextBox` subclass at the caret carrying the composition); C8 or C9: check that the peer is created at all (`OnCreateAutomationPeer` logged) and that `GetPatternCore(Text)` is reached, then retry with `ITextProvider` only; C11 or C3: these cannot fail by construction in H1 (one XAML tree), so a failure is a bug to report. If the composed classes cannot be built from Rust at all (C1, C3), run S0b's open checks on F2 with the existing code, then build F3 only if F2 fails C11.

## Go and no-go

- **GO for H1** when C1 to C6, C8 and C11 pass and none of C7, C9, C10 and C12 fails for a reason the route itself causes. WUI-1's hosting row becomes **Decided by measurement: H1**, and WP-WINUI-3 starts.
- **GO for H1 with F1's input** when everything passes except C5 or C6 through `CoreTextEditContext`, and one F1 input passes them.
- **GO for F2** when the composed classes fail (C1 or C3, or C8 by a peer that cannot be composed) and F2 passes C5, C6, C8 and C11.
- **GO for F3** when F2 fails C11 or C10 and F3 passes them.
- **NO-GO** when no real route passes C5, C6 and C8. The lead stops the WinUI packages after WP-WINUI-2 and asks the owner whether to take Option B (OWNER DECISION 1). The core, the painter and the host tests are kept either way.

## Control strategy

**The page and the ruler are drawn; everything else is WinUI.** Drawing the page with the shared painter is what keeps Windows and Linux breaking lines and placing glyphs identically (D164, D165), and S0a proved it exact. Everything a person reaches with the keyboard, and everything Narrator reads apart from the document, is a native XAML control, so focus, Tab order, access keys, tooltips, theming and UI Automation come from WinUI.

| Part | Control | Notes |
|---|---|---|
| Menu bar | `MenuBar`, `MenuBarItem`, `MenuFlyoutItem`, `ToggleMenuFlyoutItem`, `MenuFlyoutSubItem`, `MenuFlyoutSeparator` | Built from the shared menu tree (`quill_app::menu_items` today, `quill-menu` later); `AccessKey` from each title's `&`; `KeyboardAccelerators` from `accel`; checked and greyed from `App::menu_mark` and `command_state` (WP-ED-9) |
| Command bars | Two `CommandBar` rows (D85, D168) with `AppBarButton`s and Segoe Fluent Icons; style, font and size `ComboBox`es inside `AppBarElementContainer` (the container Reactor lacked) | `DefaultLabelPosition` collapsed, icons from the font, tips from the shared command tips |
| Colour | `DropDownButton` with a `Flyout` of swatches | |
| Ruler | A `SwapChainPanel` strip drawn by `quill-paint`, with pointer drags | Accessible as a `Group` with the indents and tabs named |
| Page | `QuillPage`: a composed `Control` holding a `SwapChainPanel` | Below |
| Status bar | A `Grid` of `TextBlock`s in `CaptionTextBlockStyle` on `LayerFillColorDefaultBrush` with a top divider; subtle `ZoomOut` and `ZoomIn` buttons round a `DropDownButton`, then `FitPage` | As WUI-80 and WUI-81 from the status-bar plan; the lead's live region through `AutomationProperties.LiveSetting` |
| Modal dialogs | `ContentDialog` built from `assets/dialogs/dialogs.json` (D86) with `Grid` rows, `TextBox`, `NumberBox`, `ComboBox`, `CheckBox`, `RadioButtons`, `ListView` | Primary, secondary and close buttons map Enter and Esc; access keys from the labels' `&` |
| Find and Replace | An in-window find panel at the page's top right, as Notepad and Edge have, not a modal dialog (WUI-90) | Modeless as on Win32; F3 and Shift+F3 work with it open |
| The unsaved-changes question | `ContentDialog` with Save, Don't save and Cancel | Through today's `quill_app::session`, later `quill-flow` |
| File pickers | `Microsoft.Windows.Storage.Pickers` with `AppWindow.Id`; `IFileOpenDialog` and `IFileSaveDialog` if the probe fails | Type filters as on Win32 |
| Settings | A page in the window with `NavigationView` and setting cards (WP-PREF-11, WUI-60) | |
| Reveal Codes | Two grid rows and a splitter, the pane drawn as the page is (WP-CODES-10, WUI-70, WUI-71) | |
| Notices | `InfoBar` under the command bars (recovered file, refused characters with the count) | Not a toast |
| Help | A help viewer window later, over the shared help parser (WP-WINUI-12) | F1 opens it |

**Why not draw the chrome too.** It would be the imitation the owner ruled out, and every accessibility and theming behaviour would have to be rebuilt by hand. **Why not native XAML text for the page.** `RichEditBox` lays text out with its own engine (the Windows 95 caret drift again), cannot show Quill's page model and would make the Windows client a different editor.

## The Windows 11 look

- **Fluent controls** come from WinUI: Segoe UI Variable, the 4 and 8 px corner radii, the control brushes and the focus visuals. Quill sets no colours on chrome controls except the paper.
- **Mica** through `Window.SystemBackdrop = MicaBackdrop` (Windows 11 22H2 or later, D163); S0c compares it with no backdrop because S0a's shots did not show it. The command bars and status bar are transparent over it; the page area is the desk colour.
- **Rounded corners and the shadow** are the system's on Windows 11 for a standard window; nothing to do.
- **Dark mode.** The chrome follows the system (`Application.RequestedTheme` left unset, `ActualThemeChanged` repaints the ruler and the desk); the paper stays white unless `[Quill] PaperColour=theme` (LNX-88's rules, the `ThemedSurface` moved into `quill-paint` with WinUI as its second user).
- **High contrast.** The chrome follows the contrast theme through WinUI; the page draws paper and text in the theme's `Window` and `WindowText` colours and the selection in `Highlight` and `HighlightText`, through the same colour wrapper (WUI-91).
- **Title bar.** The system title bar in v1, with its caption colours following the theme (`AppWindow.TitleBar.PreferredTheme`), so drag, snap layouts and Narrator are the system's. WinUI's `TitleBar` control with the content extended into it, as Notepad and Paint have, is measured in S0c (C14) and is the swap.
- **Icons.** Segoe Fluent Icons for commands; the app icon from the Quill SVG master rendered to `.ico` by script.
- **Snap layouts** on the maximize button (free with the system title bar).
- **Never macOS chrome** in mock-ups or art.

## Text input

- **`QuillPage` owns a `CoreTextEditContext`** from `CoreTextServicesManager::GetForCurrentView()` (the `windows` crate's `UI_Text_Core`), created when the control loads, `NotifyFocusEnter` and `NotifyFocusLeave` with the control's focus, `InputScope` text.
- **What the text service sees** is the caret's paragraph as UTF-16 with the selection in UTF-16 positions (`TextRequested`, `SelectionRequested`), as the reference editor does. `TextUpdating` gives a range and a string: the shell converts the range to the paragraph's code points, refuses the whole update with the count if a character has no Windows-1252 form (D166, D188; the `Failed` result tells the service), otherwise replaces the range as one undo step. `FormatUpdating` underlines the composing range on the page (a painter overlay, not document formatting). `LayoutRequested` answers the caret and the control's bounds in screen pixels so the candidate window sits beside the caret. Changes Quill makes itself (a paste, an undo, a click) call `NotifyTextChanged`, `NotifySelectionChanged` and `NotifyLayoutChanged`.
- **Keys** that are commands (arrows, Home, Enter, Backspace, Delete, accelerators) come through `OnKeyDown` and `PreviewKeyDown` and go to the editor's `Key` through the core's `VirtualKey` table (WP-WINUI-2); characters come only through `TextUpdating`, so dead keys and Alt codes take the same path as an input method.
- **The ED lane is asked** for a read of one paragraph's text and a replace of a range in it as one undo step (E1 below); until then the shell selects the range and types over it, which the host tests check gives the same bytes.
- **Fallbacks** (F1): a TSF text store (`ITextStoreACP2`) of Quill's own, associated with the window's input site while the page has focus; or a composed `TextBox` subclass kept at the caret, whose `TextCompositionStarted`, `TextCompositionChanged`, `TextCompositionEnded` and `CandidateWindowBoundsChanged` events drive the same overlay. The second puts focus on the `TextBox`, so its automation peer must be the page's (it is composed, so it can be).
- **Touch and pen** act as a mouse through the pointer events; a touch drag scrolls; the touch keyboard comes up through `InputPaneDisplayPolicy` automatic.

## Accessibility

- **The page.** `QuillPage` overrides `OnCreateAutomationPeer` and returns a composed `FrameworkElementAutomationPeer` (`QuillPagePeer`): control type `Edit`, class `QuillPage`, name `Quill page` with the document's name in `FullDescription`, `IsKeyboardFocusable`, and `GetPatternCore(PatternInterface::Text)` returning a `QuillTextProvider` that implements `ITextProvider2` and hands out `QuillTextRange` objects (`ITextRangeProvider`). Ranges are pairs of code-point positions over the snapshot with a revision; a stale range is clamped, never a crash.
- **The snapshot** is `quill-a11y`'s shape (R5, ARCH-lane WP-ARCH-8): the visible lines as UTF-8, each character's x in device pixels, the caret and selection, line bounds, role, name, a revision. WinUI is its second user after GTK (LNX-24): WP-WINUI-9 builds against the crate if it has landed, else against a local copy of GTK's `a11y.rs` shape that moves onto the crate in WP-ARCH-8's pull request.
- **What the provider serves:** `DocumentRange`, `GetSelection`, `GetCaretRange` (with `isActive`), `RangeFromPoint`, `GetVisibleRanges`, `Move`, `MoveEndpointByUnit` and `ExpandToEnclosingUnit` by character, format (a run of one style), word, line, paragraph and document, `GetText`, `GetBoundingRectangles`, `GetAttributeValue` for font name, size, bold (`FontWeight`), italic, underline, foreground colour and `IsReadOnly`, `ScrollIntoView`, `Select`. Offscreen text is served from the document, bounds only for visible lines.
- **Events:** `RaiseAutomationEvent(TextPatternOnTextSelectionChanged)` after caret and selection moves, `TextPatternOnTextChanged` after edits, coalesced to one per frame.
- **The chrome** is WinUI's own: names from the shared command tips through `AutomationProperties.Name`, help text from the status-bar and settings schemas, the status bar's lead as a polite live region (WUI-80), status messages also through `RaiseNotificationEvent` so Narrator says a refused character without moving focus.
- **Checks:** `uia.ps1` in the VM and in CI (below), Accessibility Insights' FastPass in the VM, and an owner's Narrator listening checklist in `docs/winui.md`.

## Printing, preview and Export as PDF

The GTK client prints the PDF it exports, byte for byte (LNX-80). Windows has no system consumer that turns a PDF into a print job, so on Windows the paper and the PDF are two outputs of one layout, held together by tests, and both keep text as text.

- **Export as PDF** is `quill_pdfdoc::export(app, store, &mut measure, &options)` with the window's own `OutlineMeasure`, exactly as GTK's WP-GTK-3b does (the same bytes for the same document and fonts). The item is File > Export as PDF… until the shared tree has `Command::ExportPdf` (S4 of [gtk-print-help.md](gtk-print-help.md)).
- **Print** (WUI-92): File > Print… calls `PrintManagerInterop::ShowPrintUIForWindowAsync(hwnd)`, the Windows 11 print dialog with its own preview. Quill answers `PrintTaskRequested` with its own `IPrintDocumentSource` (`IPrintDocumentPageSource` and `IPrintPreviewPageCollection`) over Direct2D's `ID2D1PrintControl`, the path DirectX apps use, and draws each page with `quill_paint::page::paint_sheet` on a Direct2D `Surface` whose `glyph_run` is DirectWrite's `DrawGlyphRun` with the layout's own advances from the same font files, so glyph positions are the layout's and the printer receives vector text through XPS. Not XAML's `PrintDocument`, which prints XAML elements.
- **The fallback** is `PrintDlgExW` with the same Direct2D drawing into an XPS print job (`IPrintDocumentPackageTargetFactory::CreateDocumentPackageTargetForPrintJob`), then a GDI DC with `ExtTextOutW` and `lpDx` if Direct2D printing fails in the VM. Raster bands (D169) are not used: D169 is superseded.
- **Print preview** is Quill's own (File > Print Preview, as on Win32 and GTK), drawn by `paint_sheet` on the pixel surface into the page area.
- **Tests:** the Direct2D surface's calls recorded on the host against a fake (every glyph at the layout's x); in the VM, Microsoft Print to PDF output read with `pdftotext -bbox` on the host and compared with the exported PDF's word boxes (same words, same page breaks, positions within a point).

## Clipboard

Through the Win32 clipboard on the window's handle, not `Windows.ApplicationModel.DataTransfer.Clipboard`, so the formats and the code match the Windows 95 shell's half of WP-ED-6 (WUI-93): a copy offers `Rich Text Format` (`\ansicpg1252`), `CF_UNICODETEXT` (from Windows-1252 through `quill-text`) and `CF_DIB` for a lone picture; a paste takes RTF first, then the picture, then Unicode text, refusing characters Windows-1252 lacks with the count (D188). Delayed rendering is not used. Clipboard history and cloud clipboard see the same formats. It waits for WP-ED-5's request (`App::take_clip_request`, `copy_to`, `paste_from`). The check is a PowerShell round trip (`Get-Clipboard -TextFormatType Rtf`, `Set-Clipboard`) that keeps bold.

## Packaging and deployment

In order (D162, WUI-94):

1. **A self-contained zip**, unpackaged: `Quill.exe`, the Windows App Runtime files and the manifest, built on Linux by `winui/tools/build.sh`, runtime and WinUI metadata pinned by SHA-256. This is what every package before WP-WINUI-11 ships to the VM.
2. **A winget manifest of type zip and `portable`** pointing at the release zip, drafted and validated with `winget validate`, not submitted until the owner says (OWNER DECISION 8).
3. **MSIX**, self-contained (the runtime inside the package, so no framework dependency to install), with `makeappx` and `signtool` on the Windows runner and a test certificate; installed on a fresh VM snapshot. Package identity brings file types and clean uninstall; the app ID is `LiminalHQ.Quill` (D167).
4. **A Liminal Installer target**: the installer already has an `msix` output and the Windows 11 `fizz-modern` look; a per-user setup for the WinUI build sits beside the Windows 95 floppy `SETUP.EXE` in `installer/`. The Windows 11 installer for Quill was deferred by the owner on 2026-11-25, so this waits for the owner's call.

Signing: none for the zip and winget portable in the experiment; a real code-signing certificate is an owner decision (OWNER DECISION 7). Windows 11 22H2 (22621) or later, x64 (D163); ARM64 later.

## What the client needs from the other lanes

None of these blocks S0c or the core. The client works against `main` as it is and names the split it moves onto later, as the UI track's rule says.

| From | What | WinUI package | Hard or soft |
|---|---|---|---|
| ARCH: WP-ARCH-1 `quill-chrome` | View geometry, the status bar layout (`quill_chrome::status_bar`) | WP-WINUI-2, 3, 6 | Soft: a local view model shaped like GTK's `view.rs` until then |
| ARCH: WP-ARCH-3 `quill-session` | `Pump` (the idle cadence) and the end-of-session verdict | WP-WINUI-4 (shutdown guard: `AppWindow.Closing` and `WM_QUERYENDSESSION`), autosave PR 19 | Soft |
| ARCH: WP-ARCH-4 `quill-input` | `Key`, `accel`, the input event model | WP-WINUI-4 | Soft: `quill_app::Key` and `accel::from_vk` today |
| ARCH: WP-ARCH-5 `quill-menu` | The command vocabulary and the menu tree, mnemonics and accelerators as data | WP-WINUI-5, 6 | Soft: `quill_app::menu_items` today |
| ARCH: WP-ARCH-6 `quill-settings` | INI, profiles, the recent list, the schema the Settings page draws | WP-WINUI-2, WP-PREF-11 | Soft |
| ARCH: WP-ARCH-7 `quill-flow` | The answering layer: confirm-save routing, dialog answers | WP-WINUI-5 | Soft: `quill_app::session` today |
| ARCH: WP-ARCH-8 `quill-a11y` | The text snapshot | WP-WINUI-9 | **Lands before or with WP-WINUI-9** (WinUI is the second user); WP-WINUI-9 may start on a local copy |
| ARCH: S2 of gtk-print-help | One walker for preview and paper | WP-WINUI-10 | Soft |
| ARCH: S4 of gtk-print-help | `Command::ExportPdf` in the shared tree | WP-WINUI-10 | Soft: a WinUI-only item until then |
| PDF owner: S1, S3, S6 | Embedded TrueType, pictures at their own resolution, a list of pages | WP-WINUI-10 | Soft |
| ED: WP-ED-5 | The clipboard request | WP-WINUI-7 | **Hard** |
| ED: WP-ED-9 | Edit menu state (`command_state`) | WP-WINUI-5 | Soft: everything enabled until then |
| ED: WP-ED-11 | UTF-8 buffers (input still refuses what 1252 lacks) | WP-WINUI-4 | Soft: the UTF-16 positions map to code points either way |
| ED: E1 (asked here) | `App` reads one paragraph's text and replaces a range in it as one undo step, for text services | WP-WINUI-4 | Soft: select and type over until then; the same call serves GTK's input method |
| ED: the parked dialogs | Page Layout, Numbering, Header and Footer, Size Picture, Page Break, word keys | WP-WINUI-5 takes what has landed | Hard per item |
| Preferences: WP-PREF-11 | The Settings page | After WP-WINUI-5 | — |
| Reveal Codes: WP-CODES-10 | The codes pane | After WP-WINUI-6 and WP-WINUI-9 | — |
| Status bar: WUI-80, WUI-81 | The bar and Ctrl+0 | WP-WINUI-6 | — |
| Autosave PR 19 | The WinUI executor, autosave, the logout guard | After WP-WINUI-4 | — |

## Testing

**Three places, most of it on the host.**

- **The Linux host** (every package): the `core` module names no Windows crate and carries the logic: UTF-16 and 1252, the name table, `VirtualKey` to `Key`, the view model with property tests at 100 to 300 %, the text-service range mapping (UTF-16 positions to code points and back, refusal), the automation range arithmetic over a snapshot (move by unit, expand, clamp after an edit), the menu and dialog models built from the shared data, the Direct2D surface's calls against a recording fake, the command-layer replay of the host's editing scenarios with saved bytes compared. `cargo xwin build` and clippy for the Windows target.
- **A Windows runner in CI** (WP-WINUI-11a): `windows-2025` builds natively with MSVC and runs the core tests and the replay tests there too; `Quill.exe --self-test` starts the window off the desktop's way, opens `tests/docs/letter.rtf`, presents one frame, writes its pixels, saves and compares bytes with `tests/golden/files`, and exits; `uia.ps1` reads the page's `DocumentRange` and caret [U whether the hosted runner's session allows a WinUI window and UI Automation; the job records the answer and skips the window steps, not the build, if it cannot]. The zip is uploaded as an artifact. Not in CI: input methods, Narrator, Mica, scales other than the runner's, printing.
- **The Windows dev VM** (`WinDev2407Eval`, Windows 11 22H2 build 22621, VirtualBox, 8 GB guest memory, 4 CPUs, VBoxSVGA): framebuffer shots, scales, input methods, Narrator, Mica and themes, printing to Microsoft Print to PDF, installs.

**VM rules** (every package that starts it):

- Before starting: `free -g` shows 12 GB available or more (8 GB guest, VirtualBox's own, and headroom); `df -h /mnt/games` 20 GB free (the differencing disk grows about 11 GB a boot); `df -h /home` above 20 GB. If any fails, do not start it; report.
- **The VM counts against the agent cap.** While it runs, at most one other builder runs, host-only (no Wine, DOSBox, Xvfb and Weston, or second VM), so the lead runs two builders instead of three for that time. A VM package says so in its pull request and the lead schedules it.
- From the snapshot `Password never expires`, network cable off; restore it and power off at the end. Build on the host and copy only the zip in. Never print or write the guest password. GUI runs through a scheduled task in the user's session; screenshots with `VBoxManage controlvm screenshotpng` only; keys with `keyboardputscancode` or `keyboardputstring`; clicks through the VirtualBox mouse tools. Scales through `PerMonitorSettings` `DpiValue` and a restart (S0a's recipe).
- Kill only processes you started, by PID.

**Evidence.** Every shot into `~/Documents/Liminal HQ/Quill/WinUI/<package>/` named `winui-<check>-<scale>-<theme>.png`; the logs and the `uia.ps1` output beside them as text; the few deciding shots committed under `docs/media/winui-spike/` (S0c) or `docs/media/winui-*.png`.

## Budgets

| Item | S0a measured | Budget | Checked in |
|---|---|---|---|
| Zip (self-contained) | 23.3 MB | 35 MB | Every package, `build.sh` prints it |
| Unpacked | 61.5 MB | 90 MB | Every package |
| `Quill.exe` | 2.6 MB (spike) | 10 MB release, stripped | Every package |
| Process start to first frame, warm (VM) | 244 ms | 500 ms | S0c, WP-WINUI-3, 11 |
| Cold after reboot (VM) | 405 to 463 ms | 1,000 ms | S0c, WP-WINUI-3 |
| Working set with the letter open | 112 to 117 MB | 160 MB | S0c, WP-WINUI-3 |
| Key to presented frame, letter, 150 % (VM) | — | Median 16 ms, 95th percentile 33 ms | WP-WINUI-4 |
| Clean host build | 15 s | 5 min with the projection | S0c |
| Generated bindings in the repository | — | 150,000 lines; filtered to the types used | S0c, WP-WINUI-3 |

A package that would break a budget stops and reports with the numbers; the lead or the owner raises it.

## Decisions

Reserved WUI-85 to WUI-94 (`scripts/next-id.sh`), recorded in [decisions.md](../decisions.md) as **Assumed**. WUI-1 and WUI-2 are revised there; D160, D161 and D169 are marked superseded in [winui.md](../winui.md).

| Id | Decision | Default taken | Swap |
|---|---|---|---|
| WUI-85 | The WinUI client is a genuine WinUI 3 application; fallbacks are real WinUI (F1 to F3); Option B leaves the plan | The owner's answer of 2026-11-27 | Option B returns only by the owner's word after a NO-GO |
| WUI-86 | Quill's own `windows-bindgen` projection of the pinned WinUI metadata, filtered to the types used, generated bindings committed with a drift check; composable classes through one outer-object helper; Reactor stays the spikes' | Own projection | Reactor, if a release gives custom automation peers and text composition |
| WUI-87 | The page is a `SwapChainPanel` in a composed `QuillPage` control in the window's own XAML tree; no child or pop-up window; the page host is one module behind `PageHost` | H1 | F2 or F3 by replacing the module |
| WUI-88 | Text input through `CoreTextEditContext` on the page; the paragraph as the service's text; refusals fail the update with the count | `CoreTextEditContext` | F1: a TSF text store or a composed `TextBox` |
| WUI-89 | The page's UI Automation through a composed `FrameworkElementAutomationPeer` serving `ITextProvider2` and `ITextRangeProvider` from the `quill-a11y` snapshot; status messages also as UIA notifications | Composed peer | AccessKit on F2's or F3's own window |
| WUI-90 | Native XAML for all chrome (menu bar, command bars with `AppBarElementContainer`, status bar, `ContentDialog` dialogs, `InfoBar` notices, settings page); the page and ruler drawn by `quill-paint`; Find and Replace as an in-window panel | As listed | Find and Replace as a `ContentDialog` or an owned window |
| WUI-91 | The Windows 11 look: Mica (`MicaBackdrop`), the system title bar following the theme, the system's corners, chrome following light, dark and high contrast; paper white unless `PaperColour=theme`; high contrast paints the page in the theme's colours | System title bar | WinUI `TitleBar` with extended content (S0c C14) |
| WUI-92 | Print through `PrintManagerInterop` and Quill's own `IPrintDocumentSource` over `ID2D1PrintControl`, pages by `paint_sheet` on a Direct2D surface with the layout's advances (vector text); Export as PDF through `quill_pdfdoc::export` as on GTK; preview Quill's own; supersedes D169 | Modern print dialog, vector | `PrintDlgExW` with the same drawing to an XPS job; GDI `ExtTextOutW` |
| WUI-93 | The Win32 clipboard on the window's handle with `quill-clip`'s formats (RTF, `CF_UNICODETEXT`, `CF_DIB`), after WP-ED-5 | Win32 clipboard | `Windows.ApplicationModel.DataTransfer.Clipboard` |
| WUI-94 | Distribution and CI: the self-contained zip first, then a winget portable manifest (drafted), then a self-contained MSIX with a test certificate, then the Liminal Installer target by the owner's call; a `winui` CI job on `windows-2025` from WP-WINUI-11a; Windows 11 22H2 or later, x64 | As listed | The order in WP-WINUI-11b |

## Work packages

The existing numbering is kept; WP-WINUI-1 and WP-WINUI-4, 9 and 10 change content, WP-WINUI-11 splits into 11a (CI, early) and 11b (packaging), and WP-WINUI-12 (help) is new. One WinUI builder at a time from WP-WINUI-3 on, because every package wires into the window module; WP-WINUI-1 and WP-WINUI-2 run together (VM work and host code in new files).

| # | Package | Starts when | VM | Runs beside |
|---|---|---|---|---|
| 1 | **WP-WINUI-1** Hosting decision (S0c): H1 measured | Now | Yes | WP-WINUI-2, GTK packages (host-only while the VM runs) |
| 2 | **WP-WINUI-2** The portable shell core | Now | No | WP-WINUI-1, GTK packages |
| 3 | **WP-WINUI-3** Viewer (W3), the projection promoted | WP-WINUI-1 decided, WP-WINUI-2 merged | Yes | GTK packages |
| 4 | **WP-WINUI-11a** CI job | After WP-WINUI-3 | No | Any WinUI package (touches `.github/` only) |
| 5 | **WP-WINUI-4** Editing and text input (W4) | After WP-WINUI-3 | Yes | GTK packages |
| 6 | **WP-WINUI-5** Menus and dialogs (W5) | After WP-WINUI-4 | Yes | GTK packages |
| 7 | **WP-WINUI-6** Command bars, ruler and status bar (W6) | After WP-WINUI-5 | Yes | GTK packages |
| 8 | **WP-WINUI-7** Clipboard (W7) | After WP-WINUI-4 and WP-ED-5 | Yes | GTK packages |
| 9 | **WP-WINUI-8** Mica, themes, title bar, dark paper (W8) | After WP-WINUI-6 | Yes | GTK packages |
| 10 | **WP-WINUI-9** Narrator and the text pattern (W9) | After WP-WINUI-4; with or after WP-ARCH-8 | Yes | GTK packages |
| 11 | **WP-WINUI-10** Printing, preview and Export as PDF (W10) | After WP-WINUI-6 | Yes | GTK packages |
| 12 | WP-PREF-11 Settings page | After WP-WINUI-5 | Yes | — |
| 13 | WP-CODES-10 Reveal Codes pane | After WP-WINUI-6, WP-WINUI-9, WP-CODES-3 | Yes | — |
| 14 | **WP-WINUI-12** Help viewer | After WP-GTK-8 and WP-WINUI-5 | Yes | — |
| 15 | **WP-WINUI-11b** Packaging: winget draft, MSIX, installer target (W11) | By the owner's call | Yes | — |

Common to every package: **Files owned** are `winui/**` except as stated, the package's rows of `docs/winui.md`, its row of `docs/progress.md` and its `WUI-` rows; **must not touch** `crates/**`, `assets/**`, `linux/**`, `dos/**`, `win16/**`, `win9x/**`, the root `Cargo.toml` and `Cargo.lock`, `.github/**` (except WP-WINUI-11a), `SPEC.md` beyond the Windows 11 platform row; a needed shared change stops the package and is reported. **Gates**: the WinUI workspace's `cargo fmt --check`, `cargo clippy -D warnings` and host tests on Linux, `cargo xwin build --release --locked`, `bash scripts/check-headers.sh`, `python3 scripts/check-ids.py`, markdownlint in Docker.

### WP-WINUI-1 Hosting decision (S0c)

- **Finish line.** H1 built as a spike and measured on C1 to C18 at 100 and 150 % (C4 also 175 %); the fallbacks tried as [If H1 fails a gate](#what-s0c-must-measure) says; a "WinUI 3 spike S0c" section in `docs/winui-spike.md` with the table, the numbers and the go or no-go; WUI-1's row **Decided by measurement**; WUI-86 to WUI-91 confirmed or changed with the reason.
- **Files owned.** `winui/spike/**` (new `src/s0c/`, new `xaml/` for the projection and the compose helper, `tools/uia.ps1` extended, new `tools/tsf-imeon.ps1` if a script is needed), `winui/tools/**` (a `stage-winmd.sh` for the pinned `Microsoft.WindowsAppSDK.WinUI` package and its hash), `docs/winui-spike.md`, the WUI-1 and WUI-86 to WUI-91 rows, `docs/media/winui-spike/`.
- **Tests.** Host tests for the compose helper's `QueryInterface` and reference counting with fake interfaces; the range mapping for `TextUpdating`; the VM checks.
- **Evidence.** `~/Documents/Liminal HQ/Quill/WinUI/WP-WINUI-1/`: every check's shot and log; committed: the checkerboard at 150 %, the composition with the candidate window, the dialog over the page, Narrator's focus rectangle, Mica and no Mica, light and dark.
- **Budget.** Startup, size and build-time numbers recorded against the table; a miss is reported, not fixed here.
- **VM.** Yes; two builders at most while it runs.

### WP-WINUI-2 The portable shell core

- **Finish line.** `winui/quill-winui` exists as its own workspace with a `core` module that names no Windows crate and builds and tests on Linux: UTF-16 to Windows-1252 and back through `quill-text` (refusal with a count), the name table for UTF-16 paths (the `LinuxFiles` rule), `VirtualKey` to the editor's `Key` and `accel::from_vk`, the view model (scroll in document pixels, zoom, `Mapping` at 1/120 scales, reveal, device-to-document mapping, in the shape of GTK's `view.rs`), the settings path (`%APPDATA%\Quill\QUILL.INI`, `[RecentWinUI]`, the `*WinUI` family keys), safe save's Windows half behind a trait (`ReplaceFileW` or `MoveFileExW`), and the two pieces H1 needs: `core::textsvc` (UTF-16 ranges of a paragraph to code points and back, the refusal of an update with its count, the composing range for the overlay) and `core::uia` (the text-range arithmetic over a snapshot of the `quill-a11y` shape: move and expand by character, word, line, paragraph and document, clamp to a new revision).
- **Files owned.** New `winui/quill-winui/**` (no Windows module yet beyond an empty `main` behind `cfg(windows)`), `winui/tools/build.sh` (a second target), `docs/winui.md` (W3's prerequisites).
- **Must not touch.** `crates/**`, `linux/**`, `winui/spike/**`, the root workspace.
- **Dependencies.** None. Moves onto `quill-chrome`, `quill-session`, `quill-input`, `quill-settings` and `quill-a11y` when they land (WUI-2).
- **Tests.** Host tests of each module; property tests for the view at 100 to 300 %, for `textsvc` over random UTF-16 with surrogate pairs, and for `uia` over random edits.
- **Evidence.** None visual. No VM.

### WP-WINUI-3 Viewer (W3)

- **Finish line.** `winui/quill-winui` gains its Windows modules: the projection moved from the spike into `winui/quill-winui/xaml` with its drift check (`tools/gen-bindings.sh --check`), `Application` composed, the `Window` with Mica, `QuillPage` with its swap chain, `PageHost`; `Quill.exe` opens a document from the command line and the picker, shows every sheet pixel-exact through `quill_paint::pixel` at 100, 150 and 175 % and across a live scale change, scrolls, zooms, and keeps the window placement, recent files and zoom in `%APPDATA%\Quill\QUILL.INI`; starts from the zip with the cable off within the budgets.
- **Tests.** Every file in `tests/docs` opens on the host (core) and in the VM; the checkerboard and page analysis on framebuffer frames; `/LINES` equals `letter.outline-1440.txt`.
- **Evidence.** The letter and the demo document at three scales, light and dark.

### WP-WINUI-11a CI job

- **Finish line.** A `winui` job (one job per check: `winui-host` on Ubuntu running the core tests and `cargo xwin` build; `winui-windows` on `windows-2025` building with MSVC, running the tests, `Quill.exe --self-test` and `uia.ps1`, uploading the zip). Records whether the hosted session shows the window; skips the window steps with a notice if not.
- **Files owned.** `.github/workflows/ci.yml` (the two jobs only), `winui/tools/ci-*.ps1`.
- **Tests.** actionlint in Docker; the run is not dispatched by the builder (the owner's rule); the pull request's own CI run is the proof.

### WP-WINUI-4 Editing and text input (W4)

- **Finish line.** Keys, accelerators, the caret and its blink (off when animations are off), the selection, the pointer, touch and pen as a mouse; all typed text through `CoreTextEditContext` (or the F1 input S0c chose): US-International dead keys, Alt codes, a Japanese or Korean composition with the inline underline and the candidate window beside the caret, refusals with the count in an `InfoBar`; Save and Save As with safe save; the unsaved-changes `ContentDialog`; the shutdown guard (`AppWindow.Closing`, `WM_QUERYENDSESSION`, `ShutdownBlockReasonCreate`).
- **Tests.** The host's editing scenarios through the command layer save identical bytes; the text-service mapping over the scenarios; a `SendInput` session in the VM matches `tests/golden/files`.
- **Budget.** Key to frame as in the table.
- **Evidence.** Typing, a composition, the refusal, the question.

### WP-WINUI-5 Menus and dialogs (W5)

- **Finish line.** The `MenuBar` from the shared tree with access keys, accelerators, check marks and greyed items; every `Dialog` variant as a `ContentDialog` built from `quill-dialogs` (About, Fonts, Tabs, Spelling, the question, and the parked block's that have landed), Find and Replace as the in-window panel, Open, Save As and Insert Picture through the Windows App SDK pickers (or the shell dialogs if S0c's probe failed); Print waits for WP-WINUI-10.
- **Tests.** A test hook answering each dialog (like GTK's `--self-test-dialogs`); `uia.ps1` finds each dialog by name and its controls by label; Enter, Esc and access keys driven by scancodes.
- **Evidence.** Each menu and dialog in light and dark at 150 %.

### WP-WINUI-6 Command bars, ruler and status bar (W6)

- **Finish line.** The two command bars of D85 with the boxes in `AppBarElementContainer`s, the colour flyout, the ruler with drags, the status bar of WUI-80 and WUI-81 laid out by `quill_chrome::status_bar` (or its local copy).
- **Tests.** The same commands as GTK's `tests/chrome.rs`; the status layout as data at each width; narrowing below 560 px.
- **Evidence.** Wide and narrow, light and dark.

### WP-WINUI-7 Clipboard (W7)

- **Finish line.** WUI-93's formats through `quill-clip`; the PowerShell round trip keeps bold; refusals counted.
- **Dependencies.** **Hard:** WP-ED-5.
- **Evidence.** The round trip's output and the refusal notice.

### WP-WINUI-8 Mica, themes, title bar and dark paper (W8)

- **Finish line.** Mica shown against a run without it; light, dark and two contrast themes followed live; the title bar as WUI-91 (or its swap); snap layouts; `PaperColour=theme` through the colour wrapper moved into `quill-paint` (a shared change: the package asks the architecture lane, and draws dark paper locally until then).
- **Evidence.** Each theme; the snap layouts flyout.

### WP-WINUI-9 Narrator and the text pattern (W9)

- **Finish line.** `QuillPagePeer` and `QuillTextProvider` serve everything [Accessibility](#accessibility) lists from the snapshot; events raised; the chrome's names complete; Accessibility Insights' FastPass has no failures in the window; the owner's Narrator checklist written in `docs/winui.md`.
- **Dependencies.** WP-WINUI-4; WP-ARCH-8 lands before or with it (a local snapshot of its shape until then, moved by WP-ARCH-8's pull request).
- **Evidence.** The `uia.ps1` dump, the FastPass report, Narrator's focus rectangle on a line.

### WP-WINUI-10 Printing, preview and Export as PDF (W10)

- **Finish line.** WUI-92: Export as PDF through `quill_pdfdoc::export`; Print through the Windows print dialog with Quill's document source and the Direct2D surface (or the fallback S0c's probe chose); Quill's own Print Preview; Microsoft Print to PDF output's word boxes within a point of the exported PDF's, the same page breaks.
- **Evidence.** The print dialog with its preview, the preview, the two PDFs' `pdftotext -bbox` comparison.

### WP-WINUI-11b Packaging (W11)

The winget portable manifest (validated, not submitted), the self-contained MSIX with a test certificate installed on a fresh snapshot, and the installer target, by the owner's call (OWNER DECISIONS 7 and 8). Brief written when the call is made.

### WP-WINUI-12 Help viewer

The GTK help viewer's model over `quill_help::qhm` (WP-GTK-8, LNX-87) as a WinUI window with a `TreeView` of contents, `RichTextBlock` topics, Back and Forward, search and F1. After WP-GTK-8 has settled the shared parts. Brief written then.

## Builder briefs

Each brief is the common preamble of [ui-track.md](../ui-track.md#common-preamble) followed by the package's own part. Paste both.

### WP-WINUI-1 Hosting decision, S0c (full brief)

```text
Package WP-WINUI-1, Hosting decision (S0c): measure a real WinUI 3 window. Branch spike/winui-s0c, worktree
~/source/worktrees/quill/winui-s0c from origin/main.

Read first: docs/plans/winui-real-window.md (all of it: the route H1, "What S0c must measure", "Go and
no-go", "Text input", "Accessibility", "Testing" and the VM rules), docs/winui-spike.md (S0a and S0b in
full), docs/winui.md, decisions WUI-1, WUI-85 to WUI-91; winui/spike (Cargo.toml, src/main.rs,
src/swapchain.rs, src/s0b/*, tools/analyse.py, tools/uia.ps1), winui/tools/build.sh and stage-runtime.sh.
For the projection and the compose helper, read (do not depend on) naui-winui3 0.20.1 src/compose.rs and
src/self_contained.rs, and the windows-bindgen 0.100 documentation.

Goal: build route H1 as a spike and measure it in the Windows dev VM, so WUI-1 becomes "Decided by
measurement". H1 is a genuine WinUI 3 app: Application::Start, a Microsoft.UI.Xaml.Window, WinUI chrome,
and the page in a SwapChainPanel inside a composed QuillPage control, with CoreTextEditContext for text
and a composed automation peer for UI Automation. No child window, no pop-up.

Build on the host, in this order, one commit each where it makes sense:

1. winui/tools/stage-winmd.sh: download Microsoft.WindowsAppSDK.WinUI of the same release as the staged
   runtime (2.4.0 unless a check needs later), into ~/.cache/quill-winui, check a SHA-256 kept in
   winui/tools/winmd.sha256 (taken from the first download; say so), extract the .winmd files.
2. winui/spike/xaml: a crate with bindings generated by windows-bindgen from those .winmd files, filtered
   to the types the spike uses (Application and its overrides, Window, IWindowNative, MicaBackdrop,
   Grid, Control and its overrides, SwapChainPanel, ISwapChainPanelNative, MenuBar and items, CommandBar,
   AppBarButton, AppBarElementContainer, ComboBox, Flyout, ContentDialog, TextBlock, TitleBar,
   FrameworkElementAutomationPeer and its overrides, the Automation.Provider ITextProvider, ITextProvider2,
   ITextRangeProvider, the automation enums and event ids, and what they pull in). A generator script
   winui/spike/xaml/gen.sh writes src/bindings.rs (committed, rustfmt-skipped, SPDX header added by the
   script); record its line count. A compose.rs helper for COM aggregation (outer object, non-delegating
   inner, QueryInterface falls through to inner), with host tests on fake interfaces.
3. src/s0c/: Application composed (OnLaunched creates the Window); Window with MicaBackdrop (--nobackdrop
   to drop it), a MenuBar (File: Open, Exit; View: Dialog, Flyout), a CommandBar with Bold and Italic
   AppBarButtons with Segoe Fluent Icons and a ComboBox inside an AppBarElementContainer, a ruler strip,
   the page, and a status TextBlock. QuillPage composed from Control (focusable, IsTabStop, overrides for
   OnKeyDown, OnPointerPressed, OnGotFocus, OnLostFocus, OnCreateAutomationPeer) holding a SwapChainPanel
   fed exactly as src/swapchain.rs does (one buffer pixel per device pixel, rebuilt on
   CompositionScaleChanged and SizeChanged). The page shows S0a's checkerboard (--pattern) or a small
   editable document of a few paragraphs drawn with skrifa and tiny-skia, with a caret and selection.
4. Text input: a CoreTextEditContext from CoreTextServicesManager::GetForCurrentView() on the UI thread,
   NotifyFocusEnter and Leave with QuillPage's focus; TextRequested and SelectionRequested serve the caret's
   paragraph as UTF-16; TextUpdating converts the range, refuses an update with a character Windows-1252
   lacks (Result Failed, a status message with the count) or applies it; FormatUpdating underlines the
   composing range; LayoutRequested gives the caret and control bounds in screen pixels. Log every event.
   --imeon activates the Japanese profile through ITfInputProcessorProfileMgr::ActivateProfile
   (0411:{03B5835F-F03C-411B-9CE2-AA23E1171E36}{A76C93D9-5523-4E90-AAFA-4DB112F9AC76}; check it
   against the guest's registration first) and sets
   GUID_COMPARTMENT_KEYBOARD_OPENCLOSE to 1; --imekr does the same for the Korean input method.
   If CoreText fails in the guest, add F1's two inputs behind --tsf (an ITextStoreACP2 associated with the
   input site while the page has focus) and --textbox (a composed TextBox at the caret using its
   TextComposition* and CandidateWindowBoundsChanged events), and measure them.
5. UI Automation: QuillPagePeer composed from FrameworkElementAutomationPeer (Edit, class QuillPage, name
   "Quill page"); GetPatternCore(Text) returns a QuillTextProvider (ITextProvider2) whose ranges implement
   ITextRangeProvider over the document as code-point pairs with a revision: DocumentRange, GetSelection,
   GetCaretRange, RangeFromPoint, GetVisibleRanges, Move, MoveEndpointByUnit and ExpandToEnclosingUnit by
   character, word, line, paragraph and document, GetText, GetBoundingRectangles, Select, ScrollIntoView,
   GetAttributeValue for FontName, FontSize, FontWeight, IsItalic. Raise TextSelectionChanged and
   TextChanged. Log every call with its arguments and timing.
6. Extend tools/uia.ps1 to run every C8 call with a 10 s watchdog per call (a hang is a recorded fail), and
   write its output to a file. Keep S0b's --p2 --popup build working: it is fallback F2.
7. Logging: main to first presented frame, process start time, working set, every focus change, every
   ActualThemeChanged and scale change.

Gates on the host before the VM: cargo fmt --check, cargo clippy --target x86_64-pc-windows-msvc -- -D
warnings under cargo xwin, the host tests, winui/tools/build.sh, bash scripts/check-headers.sh,
python3 scripts/check-ids.py, markdownlint in Docker.

The VM (read the VM rules in the plan's Testing section and follow them exactly):
- Before starting it: free -g must show 12 GB available or more, df -h /mnt/games 20 GB free, df -h /home
  above 20 GB. If any fails, do not start it; report. Tell the lead before you start it: while it runs at
  most one other builder may run, host-only. Close any heavy tool of your own first.
- WinDev2407Eval from the snapshot "Password never expires", cable off; restore and power off at the end.
  Never print or write the guest password (read it from the memory file only into the command that needs
  it). Copy only the zip in. Run through a scheduled task in the user's desktop session. Shots with
  VBoxManage controlvm screenshotpng only; keys with keyboardputscancode or keyboardputstring; clicks with
  the VirtualBox mouse tools. Scales through PerMonitorSettings DpiValue and a restart, as S0a did; the
  live scale change through the Settings app by clicks if the registry path cannot change it live (say
  which). Themes through AppsUseLightTheme; high contrast through the Settings app.
- Record Get-AppxPackage *WindowsAppRuntime* and the VC++ runtime's presence, so a masked dependency shows.

Run C1 to C18 of the plan's table at 100 % and 150 % (C4 also at 175 %). Follow "If H1 fails a gate"
exactly: F1 inputs in the same build, then F2 (S0b's open checks with the existing code), then F3 only if
F2 fails C11 or C10, and only if time allows; otherwise report F3 as not run.

Write the result into docs/winui-spike.md as "WinUI 3 spike S0c": a table like S0b's with one column per
route tried, the numbers (startup, working set, exe, zip and unpacked sizes, file count, build times,
bindings line count) against the plan's budgets, what failed with exact errors, and the go or no-go by the
plan's rules. Update decisions.md: WUI-1's hosting row to "Decided by measurement" (H1, H1 with F1's input,
F2, F3 or NO-GO, naming the checks that decided it), and WUI-86 to WUI-91 confirmed or changed with the
reason. Update docs/winui.md's "Risks and what is not proven" for what was settled.

Must not touch: crates/**, assets/**, linux/**, dos/**, win16/**, win9x/**, winui/quill-winui/** (WP-WINUI-2
owns it), the root Cargo.toml and Cargo.lock, .github/**, SPEC.md.

Evidence: every shot and log into ~/Documents/Liminal HQ/Quill/WinUI/WP-WINUI-1/ named
winui-<check>-<scale>-<theme>.png (for example winui-c6-ime-150-light.png), uia.ps1's output and the app
logs as text beside them. Commit under docs/media/winui-spike/: s0c-checkerboard-150.png,
s0c-ime-candidate-150.png, s0c-dialog-over-page-150.png, s0c-narrator-focus-150.png,
s0c-mica-light.png, s0c-nomica-light.png, s0c-dark.png, and any shot that decided a fallback.

Not verified, to say in the PR: Narrator's speech (heard by the owner later), a real GPU, Windows 10 and
ARM64, the hosted CI runner.

PR title suggestion: "Measure a real WinUI 3 window for the hosting decision (S0c)". Labels: spike if it
exists, else none. Do not merge. Report: PR URL, the go or no-go and the route, each check's result, the
numbers against the budgets, the evidence paths, OWNER DECISION lines, and anything you stopped on.
```

### WP-WINUI-2 The portable shell core

```text
Package WP-WINUI-2, The portable shell core. Branch feat/winui-core. No VM.
Read: docs/plans/winui-real-window.md ("Text input", "Accessibility", "Testing", WP-WINUI-2), docs/winui.md (Architecture, Tests), decisions WUI-2, WUI-87 to WUI-89, D160 to D169 in
docs/winui.md, ARCH-3; linux/quill-gtk/src/view.rs, names.rs and settings.rs (the shapes to follow), and
the R5 row of docs/plans/web-and-layering-addendum.md with WP-ARCH-8 in docs/work-packages.md.
Build: a new workspace winui/quill-winui (own Cargo.toml, rust-toolchain.toml, rustfmt.toml), a lib whose
`core` module names no Windows crate: utf16 (to and from Windows-1252 through quill-text, refusal with a
count), names (the UTF-16 path table, display names as LinuxFiles does), keys (VirtualKey numbers to
quill_app::Key, accel::from_vk), view (the GTK view model's shape: document-pixel scroll, zoom, Mapping at
1/120 scales, reveal, device to document), settings (paths, [RecentWinUI], *WinUI family keys), save (a
trait for the replace step with a host implementation), textsvc (a paragraph's UTF-16 positions to code
points and back; an update refused whole with its count; the composing range for the overlay), uia (range
arithmetic over a snapshot in quill-a11y's shape: move and expand by character, word, line, paragraph and
document; clamp to a new revision; bounding boxes from per-character x). The bin is an empty main behind
cfg(windows).
Tests on Linux: each module; proptest for the view at 100, 125, 150, 175, 200, 225, 250 and 300 % (caret
x equals the glyph's drawn x; pos_at_point round-trips), for textsvc (round trip of positions over random
UTF-16 with surrogate pairs; refusal leaves the text alone) and for uia (move by n then by -n returns;
expand is idempotent; a stale range after random edits stays in bounds).
Must not touch crates/**, linux/**, winui/spike/**, the root workspace. Decisions: the next WUI- rows inside
WUI-85 to WUI-94 are taken; if you need one, reserve with scripts/next-id.sh WUI 3 "WinUI core
(WP-WINUI-2)" and say so. PR title suggestion: "Add the WinUI client's host-tested core".
```

### WP-WINUI-3 Viewer

```text
Package WP-WINUI-3, Viewer (W3). Branch feat/winui-viewer. Starts after WP-WINUI-1 decided the route and
WP-WINUI-2 merged; if the route is not H1, stop and ask the lead for the brief's variant.
Read: the plan's WP-WINUI-3, "Control strategy", "The Windows 11 look", "Budgets"; docs/winui-spike.md S0c;
winui/spike/xaml and src/s0c (what you promote).
Build: move the projection into winui/quill-winui/xaml with tools/gen-bindings.sh and a --check mode
(drift); Application composed; the Window with Mica and the system title bar; QuillPage and PageHost over
quill_paint::pixel with the app's OutlineMeasure (fonts from %WINDIR%\Fonts and the per-user fonts folder);
open from the command line and the picker; scroll, zoom, keep placement, recent files and zoom in
%APPDATA%\Quill\QUILL.INI; winui/tools/build.sh builds Quill.exe and Quill-winui.zip.
Tests: tests/docs on the host; in the VM every file opens, the checkerboard is exact at 100, 150 and 175 %
and across a live change, /LINES equals letter.outline-1440.txt; the budgets measured.
VM rules as the plan says. Evidence (WinUI/WP-WINUI-3/): the letter and the demo document at three scales,
light and dark; commit docs/media/winui-viewer-150-light.png and -dark.png.
PR title suggestion: "Show documents in a real WinUI 3 window".
```

### WP-WINUI-4 to WP-WINUI-10 and WP-WINUI-11a

These are written by the lead from the package entries above and the common preamble when each start condition holds, because their inputs (the route S0c picks, the splits that have landed, WP-ED-5) are not on `main` yet. Each brief names the package's finish line, files, tests, evidence folder (`~/Documents/Liminal HQ/Quill/WinUI/<package>/`), the VM rules and the budgets row it checks.

## Owner decisions

Each is taken with the default so no package waits.

| # | Question | Default taken | Where to swap |
|---|---|---|---|
| 1 | If no real WinUI route passes S0c, take Option B (an imitation in a Win32 window)? | No: stop after WP-WINUI-2 and ask | WUI-85; the plan's "Go and no-go" |
| 2 | Quill's own WinUI projection, or wait for Reactor to gain custom automation peers and text composition? | Own projection (generated, committed, drift-checked) | WUI-86; `winui/quill-winui/xaml` |
| 3 | The system title bar, or content extended into a WinUI `TitleBar` as Notepad has? | System title bar in v1; S0c measures the other | WUI-91 |
| 4 | Find and Replace: an in-window panel, or a dialog? | In-window panel at the page's top right | WUI-90 |
| 5 | Printing: the Windows 11 print dialog with vector pages from Direct2D, or the classic dialog? | The Windows 11 dialog (`PrintManagerInterop`), classic as the fallback | WUI-92 |
| 6 | High contrast: the page in the contrast theme's colours, or white paper always? | Contrast colours | WUI-91 |
| 7 | A real code-signing certificate? | None in the experiment: unsigned zip and winget portable, a test certificate for MSIX in CI | WUI-94; WP-WINUI-11b |
| 8 | Distribution order and whether to submit to winget | Zip, then a winget portable manifest drafted but not submitted, then MSIX, then the installer target | WUI-94; WP-WINUI-11b |
| 9 | Windows 11 22H2 or later, x64 only? | Yes (D163); ARM64 later | D163 in winui.md |
| 10 | A help viewer on WinUI too? | Yes, after GTK's (WP-WINUI-12) | This page's package table |
