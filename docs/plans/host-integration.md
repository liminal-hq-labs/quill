# Host integration: one link between the page and the ReactOS guest

A plan for the shared clipboard of the ReactOS web demo (`site/try-reactos/`), carried on one host-integration link together with the downloads, file drop, time and screen size items the [web demo plan](web-demo.md) and the owner asked for. The owner, 2026-11-27: "a shared clipboard, so folks can copy and paste in and out of the VM seamlessly", wanted in the first demo with the PDF printer and sound, since there may be no later one. Written on 2026-11-27 against `main` at `40a76c5`, v86 0.5.473, the QAGRR image in `~/.cache/quill-reactos-liminal-work/images/qagrr.img` (ReactOS 0.4.16), the WP-WEB-4 report (pull request 171) and the owner's decision of the same day that the AC'97 device becomes a typed TypeScript package. Where this plan and [decisions.md](../decisions.md) disagree later, the decisions win.

## Table of contents

- [Recommendation](#recommendation)
- [What was measured](#what-was-measured)
- [WP-HOST-1 report](#wp-host-1-report)
- [The link](#the-link)
- [The guest helper](#the-guest-helper)
- [The page](#the-page)
- [The clipboard flows](#the-clipboard-flows)
- [Privacy and security](#privacy-and-security)
- [Quill's own rich copy and paste](#quills-own-rich-copy-and-paste)
- [Files, time and screen size on the same link](#files-time-and-screen-size-on-the-same-link)
- [What is in the first demo](#what-is-in-the-first-demo)
- [Testing](#testing)
- [Budgets](#budgets)
- [Decisions to record](#decisions-to-record)
- [Rules for every package](#rules-for-every-package)
- [Order and parallelism](#order-and-parallelism)
- [Work packages](#work-packages)
- [Builder briefs](#builder-briefs)
- [Owner decisions](#owner-decisions)
- [Owner's check list](#owners-check-list)
- [Risks](#risks)
- [What was not verified](#what-was-not-verified)

## Recommendation

| Question | Recommendation | Why, in one line |
|---|---|---|
| Transport | **One link on COM1**: the `LHQ-FILE` frame stays exactly as planned, and a new `LHQ-PKT` frame carries short, checksummed packets on named channels (`ctl`, `clip`, `file`, `time`, `display`), both ways | COM1 is the one guest-to-page path that works today (360,448 bytes in 1.21 s, byte for byte); v86's UART takes bytes from the page with `serial_send_bytes` |
| Who owns COM1 in the guest | **A helper, `C:\Quill\HOSTLINK.EXE`**, started from the Run key, opens COM1 once and keeps it; every other sender (`SENDHOST.CMD`, Quill's Print to PDF) hands it a file instead of opening the port | A serial port opens for one handle at a time, and the clipboard needs the port open all the time to hear the page |
| Watching the guest clipboard | **The clipboard viewer chain** (`SetClipboardViewer`, `WM_DRAWCLIPBOARD`, `WM_CHANGECBCHAIN`) | ReactOS 0.4.16's `user32.dll` has no `AddClipboardFormatListener` (export table read from the QAGRR disk); the chain is there and works (ReactOS's own Clipboard Viewer updated on a copy, in QEMU) |
| Guest formats | `CF_UNICODETEXT` and `CF_TEXT`, the registered `Rich Text Format`, `CF_DIB`; file lists later | What Notepad, Quill and the system put there; `CF_DIB` covers Print Screen and Quill's pictures |
| Page side | **A typed TypeScript package, `host-link`**, laid out as the AC'97 package is (types, README, tests, pinned v86), and one glue module on the page | The owner's style for page code since 2026-11-27; the protocol and the clipboard rules want types and unit tests |
| Copy out of the guest | The helper announces the change and sends text and RTF at once; the page writes the host clipboard **when the visitor's own copy gesture is under 5 seconds old**, otherwise the clipboard chip offers **Copy to your computer** | Firefox refuses a write more than 5 s after a gesture (measured); one rule for every browser keeps the behaviour predictable |
| Paste into the guest | Ctrl+V (or Shift+Insert, or Cmd+V on a Mac) on the selected screen is caught **before v86 sees the V**; the page moves focus to a hidden text box, takes the host clipboard from the browser's own `paste` event, pushes it to the guest, waits for the helper's acknowledgement, then sends the V | The `paste` event needs no permission and works over HTTP; v86 cancels every key it handles, so without the catch no `paste` event ever fires (measured in both browsers) |
| Rich text to the host | RTF from the guest becomes **HTML** on the host clipboard (plus plain text); the reverse takes the host's HTML (or RTF where the browser offers it) and makes RTF | Neither Chromium 156 nor Firefox 159 can write `text/rtf` (`ClipboardItem.supports` false in both, measured); LibreOffice and Word both paste HTML with its formatting |
| Pictures | `CF_DIB` in the guest, `image/png` on the host, converted by the page | Both browsers write `image/png` (measured); the guest needs no image encoder |
| Privacy | Nothing leaves the tab; the host clipboard is read only on a paste gesture or a click, never polled; a chip shows the state and switches it off | The owner's rules, enforced in the package's types and tests |
| First demo | Clipboard (text, rich text, pictures), downloads, drop or upload into the guest, time sync; **Fit to window** if it is ready; file lists on the clipboard later | See [What is in the first demo](#what-is-in-the-first-demo) |

Build order: WP-HOST-1 (spike) and WP-HOST-2 (protocol, codecs, package) now, beside WP-WEB-5; then WP-HOST-3 (guest helper) and WP-HOST-4 (page clipboard); then WP-HOST-5 (files both ways, which takes over WP-WEB-2) and WP-HOST-6 (time and screen size); WP-HOST-7 (Quill's rich clipboard end to end) once WP-ED-6's Win32 half has merged; WP-WEB-8 last.

## What was measured

All on this machine on 2026-11-27. Scripts, output and shots are in `~/Documents/Liminal HQ/Quill/Web demo/host-integration/plan-evidence/`.

### ReactOS 0.4.16 on the QAGRR disk

- **No `AddClipboardFormatListener`.** `user32.dll` copied from `qagrr.img` with mtools and read with `winedump -j export` (743 exports, the list saved as `reactos-0.4.16-user32-exports.txt`): `SetClipboardViewer`, `ChangeClipboardChain`, `GetClipboardViewer`, `GetClipboardSequenceNumber`, `GetClipboardOwner`, `OpenClipboard`, `EmptyClipboard`, `SetClipboardData`, `GetClipboardData`, `EnumClipboardFormats`, `RegisterClipboardFormatA`, `IsClipboardFormatAvailable`, `ChangeDisplaySettingsA`, `EnumDisplaySettingsA`, `FindWindowA` and `SendMessageA` are there; `AddClipboardFormatListener` and `RemoveClipboardFormatListener` are not.
- **The viewer chain works.** Private QEMU (KVM, `snapshot=on`, `-display none`, input only through QMP): `clipbrd.exe` (ReactOS's Clipboard Viewer, which imports `SetClipboardViewer` and `ChangeClipboardChain`) started from the Run box, then "Viewer chain OK" typed into the Run box, Ctrl+A, Ctrl+C: the viewer showed the text at once (`qemu-clipbrd-viewer-chain.png`).
- **The serial calls are there.** `kernel32.dll` exports `SetCommState`, `SetCommTimeouts`, `SetCommMask`, `WaitCommEvent`, `SetupComm`, `PurgeComm`, `ClearCommError`, `EscapeCommFunction`, `GetOverlappedResult`, `CreateThread`, `CreateMutexA`, `SetLocalTime` and `SetSystemTime`.
- **Programs on the disk** that use the clipboard: Notepad, Clipboard Viewer, Explorer, Quill (Win32) and Quill (Windows 3.x under WineVDM). There is no WordPad and no Paint.

### The browsers

Headless Chromium 156 (`/usr/bin/chromium`) and Playwright's Firefox 159, through `playwright-core` from the harness, a page served from `http://127.0.0.1` (a secure context), with a window `keydown` listener in the bubble phase that cancels every key outside a text box, as v86's does (`clip-probe.mjs`, `clip-probe.out`, `clip-probe-ff.out`). Chromium had `clipboard-read` and `clipboard-write` granted; Firefox had nothing granted.

| | Chromium 156 | Firefox 159 |
|---|---|---|
| `navigator.userActivation` | Present | Present |
| `ClipboardItem.supports` | `text/plain`, `text/html`, `image/png`, `web text/rtf`: yes; `text/rtf`: **no** | `text/plain`, `text/html`, `image/png`: yes; `text/rtf`, `web text/rtf`: **no** |
| Write with a `ClipboardItem` whose value is a promise that resolves 1.5 s after a Ctrl+C keydown | Written | Written |
| Write 2 s after a keydown (`userActivation.isActive` true) | Written | Written |
| Write 7 s after a keydown (`isActive` false) | Written (the page has focus and the permission) | **Refused**: `NotAllowedError: Clipboard write was blocked due to lack of user activation` |
| Ctrl+V caught in a capture-phase `window` listener (`stopPropagation`, focus moved to an off-screen `textarea`, no `preventDefault`) | A `paste` event on the `textarea` with `text/plain` and `text/html`; focus back on the screen afterwards | The same |
| The keys the stand-in for v86 saw | Control, then nothing for the V | The same |

### v86's serial port and keyboard, from its source

From `src/uart.js`, `src/browser/keyboard.js` and `src/browser/starter.js` at v86 commit `315b2b0` (the vendored 0.5.473):

- **Guest to page:** each byte the guest writes to the transmit register goes out at once as a `serial0-output-byte` bus event; the transmitter is always empty, so nothing paces the guest.
- **Page to guest:** `emulator.serial_send_bytes(0, bytes)` (or a `serial0-input` event per byte) appends to an unbounded array and raises the receive interrupt; the guest's interrupt handler reads while the data-ready bit is set. There is no baud-rate pacing and no overrun in the emulated UART, so loss can only happen in the guest driver's own buffer, which is what the link's credits prevent. The array is **not** in a saved state.
- **Modem lines:** the page can set CTS, DSR, DCD and RI (`serial0-clear-to-send-input` and the others).
- **Keyboard:** v86 listens for `keydown` and `keyup` on `window` in the bubble phase, skips events whose target is an `INPUT` or `TEXTAREA` (unless it has the `phone_keyboard` class), and calls `preventDefault` on every key it handles. So a page that wants the browser's `paste` event must catch the V in the capture phase before v86 does, and a text box that holds focus also hides every key from v86 until focus returns.

### Quill on the guest today

`QUILL32.EXE` does not use the Windows clipboard yet: Copy and Paste go through the app's own clipboard (RTF and text in the document's store, since WP-ED-5; the `ClipRequest` that a shell serves with `copy_to` and `paste_from` exists and no shell serves it yet). The system clipboard arrives with WP-ED-6 (serving it on Windows 3.x and Windows 95 with `CF_TEXT`, `Rich Text Format` and `CF_DIB`). Until WP-ED-6's Win32 half merges, nothing Quill copies can leave the guest, and nothing pasted from the host reaches Quill, whatever the link does.

## WP-HOST-1 report

Run on 2026-11-27 for the questions of [WP-HOST-1](#wp-host-1-spike-the-guest-end-and-the-browsers), on ReactOS 0.4.16 from the QAGRR image (build 20260829-0.4.16-release-0-g822e864, the user is the autologon Administrator), in a private headless QEMU 11.1.3 (KVM, `-display none`, input only through QMP, pixels only from `screendump`, COM1 and COM2 on Unix sockets) and in v86 0.5.473 in headless Chromium 156 (the page's own emulator options, plus `uart1: true` so the probe can log on COM2 as it runs; the product page has COM1 only). The guest end is `H.EXE`, a throwaway `no_std` Rust Win32 program of 18,944 bytes built like `QUILL32.EXE`, with one mode per question (`w` watch the clipboard, `r`, `t`, `u`, `b`, `d` set one format, `c` the COM1 tests, `s` and `S` the clock, `z` the time zone, `m` and `x` the display); its source, the QEMU and v86 drivers and the browser probes are in `site/try-reactos/spikes/host-link/` (WP-HOST-3 removes it), and every log, shot and result file is in `~/Documents/Liminal HQ/Quill/Web demo/host-integration/WP-HOST-1/`. QEMU answers the Win32 questions; **v86 answers every question about the serial port and the clock**, because its UART and RTC are what the demo runs on (QEMU's 16550 stalled on receive in most runs for reasons of its own, and QEMU paces transmit at the baud rate, so no QEMU serial rate is quoted).

| # | Question | Answer | Verdict |
|---|---|---|---|
| 1 | Does the viewer chain deliver `WM_DRAWCLIPBOARD` to a `no_std` Rust program, once per copy, for Notepad, the Run box and Print Screen? | Yes for all three and for Alt+Print Screen, but **not once per copy**: a text copy arrives **twice**, 0 to 15 ms apart with the same `GetClipboardSequenceNumber`; a bitmap copy arrives once. `SetClipboardViewer` itself sends one message at once (sequence 0, nothing on the clipboard). With ReactOS's Clipboard Viewer in front of the helper a text copy arrives three times; closing it sends `WM_CHANGECBCHAIN` (`removed` is the viewer's handle, `next` is the helper's own) and the helper's handler keeps the chain whole: the next copy still arrives. Print Screen moves the sequence number by 2. Notepad and the Run box own the clipboard (a window); Print Screen and a program that opens it with a null window have owner 0 | **GO**, dedupe on the sequence number (the 30 ms debounce alone is not enough: 15 ms was seen, 30 ms is the margin) and ignore the message sent on joining |
| 2 | Does ReactOS make the missing text and picture formats? | Yes, both ways. `CF_TEXT` alone gives `CF_UNICODETEXT`, `CF_OEMTEXT` and `CF_LOCALE`; `CF_UNICODETEXT` alone (Zoë € ☺) gives `CF_TEXT` as Windows-1252 best fit (é is 0xE9, € is 0x80, ☺ is `?`) and `CF_OEMTEXT` (é is 0x82, € is `?`, ☺ is 0x01); `CF_BITMAP` alone (Print Screen, or a program's own) gives `CF_DIB` at 32 bits (64 by 48 gives 12,340 bytes); `CF_DIB` alone (24-bit) gives `CF_BITMAP`. They are enumerated and read like the real ones. A full 1,024 by 768 Print Screen is a 3,145,780-byte `CF_DIB`, an Alt+Print Screen of a 768 by 555 window 1,705,012 bytes | **GO** |
| 3 | Is a registered `Rich Text Format` set by one program read by another? | Yes: set with `CF_TEXT` by one process, read by the watcher in another as format 49339 (the number is the registry's, never to be assumed), 75 bytes, and `CF_UNICODETEXT` and `CF_OEMTEXT` made from the `CF_TEXT` next to it | **GO** |
| 4 | Does overlapped I/O on COM1 work in `serial.sys`? | **Partly.** `CreateFileA` with `FILE_FLAG_OVERLAPPED`, `SetupComm`, `SetCommState`, `SetCommTimeouts`, `SetCommMask`, overlapped `WriteFile` (1 MiB in 4 KiB writes, 0 short writes) and overlapped `ReadFile` all work. **`WaitCommEvent` does not**: `SetCommMask(0)` does not complete a pending wait (still pending after 2 s); a probe that exits with one pending cannot be ended by `taskkill /f` in QEMU and keeps COM1 (error 5 for the next open) until the guest restarts, and in v86 a receive loop built on it stalled at 479,232 bytes. Reads with time-outs have none of these: an overlapped `ReadFile` with `ReadIntervalTimeout` and `ReadTotalTimeoutMultiplier` of `MAXDWORD` and `ReadTotalTimeoutConstant` 250, or a synchronous one with a 50 ms constant, both ran 1 MiB clean. `SetupComm(64 KiB)` is needed (without it QEMU lost and corrupted bytes: 689 received, 450 wrong, reads of 15 bytes), and the queue holds 65,535 bytes (a 65,536-byte burst loses the last byte, `CE_RXOVER`) | **GO for overlapped writes and timed reads, NO-GO for `WaitCommEvent`** |
| 4b | (found on the way) Does the receive path keep going while the guest sends? | **Not reliably.** In v86 the guest stops taking bytes from COM1 with its receive queue full (UART: interrupt enabled, `IIR` 0x0C character timeout, `FCR` 5, 16,384 bytes waiting) and does not recover: not on a fresh edge on the interrupt line, not on modem-status interrupts (201 CTS toggles). It happens when the page has more than one packet queued while the guest also transmits (an echo, or a 16-byte credit for every 4 KiB): **one packet in flight (4,096 bytes, then wait for the guest's credit): 8 of 8 transfers of 1 MiB completed; two packets (8 KiB): 0 of 3; four packets (16 KiB): 2 of 11** (the two were on a quiet host; the rest on a busier one, so the odds move with host load); a sink that never transmits took 256 KiB queued as 64 calls without a stall. A stop-and-wait echo (4 KiB each way) completed 3 of 5 | **NO-GO for the plan's 16 KiB window, GO for one packet in flight** |
| 5 | Page to guest and guest to page, 1 MiB | **Page to guest**, one 4,096-byte packet in flight and a 16-byte credit back for each: 1,048,576 bytes in 1,225 to 1,857 ms, **0.56 to 0.86 MB/s** (8 runs; 0.78 to 0.86 for six of them), 0 mismatches. The first run on a quiet host with a 16 KiB window and 4 KiB credits: 1,127 ms, 0.93 MB/s (and 0.94 MB/s with synchronous reads); an 8 KiB window with 2 KiB credits 0.82 MB/s; 32 KiB with 4 KiB 0.85 MB/s (these windows stall on a busier host, see 4b). **Guest to page**, 4 KiB writes: 1,048,576 bytes in 3,149 ms (synchronous) and 3,134 ms (overlapped), **0.33 MB/s** (334,591 and 335,426 bytes a second), 0 mismatches, the same as the 0.3 MB/s measured with `copy`. A 1 MiB echo at one packet in flight (4 KiB each way) took 4.8 to 5.1 s, 0.21 to 0.22 MB/s each way | **GO** against the budgets (0.25 MB/s out, 0.1 MB/s in) |
| 6 | Does `SetLocalTime` work for the demo's user without a privilege? | **In QEMU, yes** (14:30:00 read back at once, no `AdjustTokenPrivileges`). **In v86, no**: `SetLocalTime` and `SetSystemTime` return TRUE with last error 0, with and without `SeSystemtimePrivilege` enabled (`OpenProcessToken`, `LookupPrivilegeValueA` and `AdjustTokenPrivileges` all succeed), and the clock does not move (it keeps the host's time to the second, and the taskbar clock agrees). `SetTimeZoneInformation` changes the bias that `GetTimeZoneInformation` reports but not `GetLocalTime`, and it makes the guest re-read the RTC: the guest clock, which had been seen 4 minutes behind the host, jumped to the host's time. The v86 guest runs on the host's UTC (the RTC reads UTC), so the taskbar shows UTC, not the visitor's time | **NO-GO** as planned in v86 (cause not found) |
| 7 | Which modes does `EnumDisplaySettingsA` list, and does `ChangeDisplaySettingsA` switch without a restart? | v86: 16 modes, 640 by 480 up to 1,920 by 1,080, all 32 bits at 60 Hz (the list includes 800 by 600, 1,024 by 768 and 1,280 by 720; the cold-booted disk starts at 800 by 600). QEMU's standard VGA lists 19. With `ChangeDisplaySettingsA(&mode, 0)` (flags 0, nothing written to the registry) the call returns 0 (`DISP_CHANGE_SUCCESSFUL`) and the desktop switches at once, in both: v86 800 by 600 to 1,280 by 720, back to 800 by 600, to 1,024 by 768 (`GetSystemMetrics` agrees; the emulator's `screen-set-size` fires twice for each change, first 640 by 480 and then the new size) | **GO** |
| 8 | Headless Chromium and Firefox: the paste event with native RTF, and a promised `image/png` after 3 s | **A promised `ClipboardItem` whose `image/png` resolves 3 s after the keydown is written in both** (Chromium 156 after 3,020 ms, Firefox 159 after 3,001 ms; read back as a 250- and a 200-byte PNG, and pasted as a file). **The paste event offers `text/rtf` in both** when the system clipboard holds native RTF: tested headed on a private Xvfb with a PyQt6 selection owner holding `text/rtf`, `application/rtf`, `text/html` and `text/plain`, both browsers listed `text/rtf` in `clipboardData.types` and returned the RTF from `getData("text/rtf")` (not `application/rtf`). The async `navigator.clipboard.read()` returns only `text/plain` and `text/html` in Chromium and waits for a prompt in Firefox | **GO** (Linux and X11 only) |
| 9 | Does a file dropped on a desktop shortcut reach its target as `%1`? | **Not established.** A hand-made shortcut to `H.EXE` was put on the desktop and A.TXT dragged onto it from Explorer with the pointer through QMP: the drag ghost showed over the shortcut and nothing started on release (no `H.EXE` run in the log). One attempt, with a shortcut the repository's `make-lnk.py` writes without an icon | **Open** |

### What WP-HOST-3 does differently from the plan

- **Read the port with time-outs, never `WaitCommEvent`.** The I/O thread does an overlapped `ReadFile` with the 250 ms constant above (or a synchronous 50 ms read) and writes between reads, as the plan's fallback already says; that fallback becomes the design. Leave no I/O pending when it exits; call `SetupComm(64 KiB)` before anything else and `PurgeComm` after.
- **One packet in flight from the page.** The link's credit is per packet, not per 4 KiB of a window: the helper sends a 16-byte `credit` after **each** packet it has taken off the port, and the page does not queue the next until it arrives. `hello` keeps a `window` key but its default is one packet (at most 4,096 payload bytes plus a header), and WP-HOST-2's specification and `link.ts` follow. A credit that does not arrive in 5 s means the guest has stalled: the page shows it and stops, because nothing the page can do to the UART brings it back (the diagnostics row says so).
- **Half-duplex bulk data.** Avoid a large transmission from the guest at the same time as a page packet in flight (a 4 KiB-each-way echo stalled in 2 of 5 runs); the helper sends one packet at a time per channel as planned and the page does not start a paste push while a download from the guest is arriving.
- **Clipboard.** Dedupe offers on `GetClipboardSequenceNumber` and ignore the first message after joining the chain; handle `WM_CHANGECBCHAIN` as written (the chain survived a viewer leaving). Read `CF_DIB` straight from the clipboard (ReactOS makes it from `CF_BITMAP`); it is 32 bits, so a full-screen Print Screen is 3.1 MB. Registered format numbers are looked up at start.
- **Time: do not plan on `SetLocalTime`.** It is a no-op in v86; WP-HOST-6 starts by finding why (the call chain is `NtSetSystemTime`; the RTC re-read after `SetTimeZoneInformation` shows the kernel does read it) and otherwise the first demo ships without time sync. Enabling `SeSystemtimePrivilege` is not the cause. The helper imports `AdjustTokenPrivileges` with `GetProcAddress` if it ever needs it (Windows 95 has none).
- **Display.** `EnumDisplaySettingsA` and `ChangeDisplaySettingsA(&mode, 0)` are as the plan has them; the modes include 800 by 600, 1,024 by 768 and 1,280 by 720, and the call needs no restart.
- **Shortcut drop.** Do not depend on a file dropped on a shortcut reaching `%1`; `SENDHOST.CMD` and the picker stay the way in.

### What WP-HOST-4 does differently from the plan

- **Paste into the guest is paced by acknowledgements**, at about 4.7 ms a 4 KiB packet, so 1 MiB takes 1.2 to 1.9 s, 100 KB of RTF about 0.15 s and 1 KB of text one round trip: all inside the budgets. The chip's *Sending…* progress counts credits.
- **Rich text from the host** can take `text/rtf` from the paste event where the browser offers it, which both browsers did on Linux (to be confirmed on Windows and macOS by the owner's check); HTML to RTF stays the fallback, and the async `read()` never offers RTF.
- **Copy out of the guest** runs at 0.33 MB/s, so only a copy under about 1.6 MB lands inside the 5 s gesture window: text and RTF always, a full-screen picture (3.1 MB) never, so pictures take the chip's button as the plan has it. A promised `image/png` item resolving 3 s after the keydown works in both browsers, so on browsers without `navigator.userActivation` a picture the page expects (from an offer's size) can be promised at the keydown; not measured beyond 3 s.
- **Test the 16 KiB window as a failure.** WP-HOST-4's fake helper and level 4 runs use one packet in flight.

### Decisions this leaves to the owner

- **Time sync in the first demo.** Default: dropped unless WP-HOST-6 finds a working call; the restored desktop then shows the clock of the state's build, as before. The guest clock was also seen 4 minutes behind the host's after a long run, which no setting here explains.
- **A second UART for page-to-guest data (COM2) was tried once and did not work**: with `uart1: true` the guest received nothing on COM2 in 25 s. Not investigated; the single-port, one-packet design above does not need it, and it would change the saved state's machine.
- **Raising v86's UART handling** (an upstream change so that a receive interrupt is raised again while data is waiting) would remove the one-packet limit; the plan does not take it on.

## The link

### Shape

```text
Page (one tab)                                            ReactOS guest in v86
host-link package ── serial_send_bytes ──► UART COM1 ──► serial.sys ──► HOSTLINK.EXE ─┬─ clipboard (viewer chain)
  clip · file · time · display     ◄── serial0-output-byte ◄────────────────────────── ├─ C:\From your computer\
  the chip, the list, the drop zone                                                    ├─ SetLocalTime
                                                                                       └─ ChangeDisplaySettingsA
SENDHOST.CMD, Quill's Print to PDF ── WM_COPYDATA ──► HOSTLINK.EXE (or COM1 directly when the helper is not running)
```

### Frames

Two frame kinds share the line. Anything outside a frame is ignored, as WP-WEB-2 planned.

- `LHQ-FILE 1 <size> <name>\r\n<size bytes>LHQ-END [<SHA-256>]\r\n` — unchanged from [web-demo.md](web-demo.md#the-channel-to-the-page), guest to page only. `SENDHOST.CMD` keeps it as its fallback when the helper is not running.
- `LHQ-PKT 1 <channel> <id> <M|E> <length> <crc>\r\n<length bytes>` — both ways. `<channel>` is one of `ctl`, `clip`, `file`, `time`, `display` (lowercase ASCII, unknown ones ignored); `<id>` is the message number, decimal, per direction, wrapping at 2^32; `M` means more packets of the same message follow and `E` ends it; `<length>` is 0 to 4,096; `<crc>` is the CRC-32 (IEEE, as zlib's) in 8 lowercase hex digits of the header line from `LHQ-PKT` up to the space before the CRC, then the payload. A header line is at most 64 bytes.

A **message** is the payloads of one channel and id in order, up to the `E` packet. Its body starts with ASCII `key=value` lines, one per line ending in `\n`, then an empty line, then the binary data, if any. The first key is always `op`. Example, the guest offering its clipboard:

```text
LHQ-PKT 1 clip 41 E 52 1c0ffee5\r\n
op=offer\nseq=17\ntext=23\nrtf=1840\ndib=0\nsource=user\n\n
```

### Channels and messages

| Channel | Direction | `op` values |
|---|---|---|
| `ctl` | both | `hello` (`proto=1`, `role=guest` or `page`, `app=HOSTLINK/0.1.0` or `host-link/0.1.0`, `caps=clip,file,time,display`, `window=` the bytes the page may have in flight, `max=` the largest message), `credit` (`n=` bytes), `pause` and `resume` (`channel=`), `error` (`code=`, `id=`, `text=`), `bye` |
| `clip` | guest to page | `offer` (`seq=` the guest's clipboard sequence, a size per format present, `source=user\|self`), `data` (`seq=`, `fmt=text\|rtf\|dib`, then the bytes), `ok` (`seq=` of a `set`), `refused` (`seq=`, `reason=`) |
| `clip` | page to guest | `get` (`seq=`, `fmt=`), `set` (`seq=`, `formats=`, then one `part` message per format), `part` (`seq=`, `fmt=`, the bytes) |
| `file` | both | `begin` (`id=`, `name=`, `size=`, `sha256=`), `chunk` (`id=`, the bytes; at most 4,096 per packet), `end` (`id=`), `saved` and `failed` (`id=`, `path=` or `reason=`) |
| `time` | page to guest | `set` (`local=` the visitor's wall time as `YYYY-MM-DDTHH:MM:SS.mmm`, `utc=` milliseconds, `zone=` the IANA name, for the log only) |
| `display` | both | page: `fit` (`w=`, `h=` the stage's size in device pixels); guest: `modes` (the modes `EnumDisplaySettingsA` lists), `changed` (`w=`, `h=`, `bpp=`) |

Text on the link is UTF-8. A `dib` body is a packed DIB (a `BITMAPINFOHEADER`, its colour table if any, the bits), as `CF_DIB` holds it.

### Flow control

- **Page to guest: one packet in flight.** The helper's `hello` names a window, which is one packet (at most 4,096 payload bytes plus a header; v86 measured, see the [WP-HOST-1 report](#wp-host-1-report): a larger window stalls the guest's receive path). The helper sends a `credit` after **each** packet it has taken off the port, and the page queues the next only when it arrives. A credit not answered in 5 s stops the sender with a status line rather than flooding the guest.
- **Guest to page: none needed.** The page drains every byte as v86 delivers it. The helper writes at most one packet at a time per channel.
- **Fairness.** Each side queues messages per channel and sends packets round robin in priority order `ctl`, `clip`, `time`, `display`, `file`, so a 30 MB file never holds a paste up for more than one packet (about 14 ms at the measured rate).

### Errors

- A line that starts `LHQ-` but does not parse is skipped; the receiver scans for the next `LHQ-` at the start of a line.
- A packet whose CRC is wrong is dropped with every other packet of its message; the receiver sends `ctl op=error code=crc id=<id>` and counts it in the diagnostics. Version 1 does not resend: every clipboard and time message is safe to repeat, the visitor repeats a copy or paste, and a file is marked damaged in the list, as `LHQ-FILE` is.
- A message larger than the receiver's `max` is refused with `code=too-large` before its data is read; an unknown `op` or channel gets `code=unknown` and is otherwise ignored.
- v86's serial line is lossless, so errors mean a bug or a second writer; the harness's counts of each code are part of every test.

### Versions and restore

- The digit after the frame name is the frame version; `hello`'s `proto` is the protocol version. Both sides speak version 1. A side that sees a higher `proto` it does not know answers `code=version` and stays quiet; the page then says "The PC's link helper is a different version: clipboard sharing is off".
- The helper sends `hello` when it opens the port. The page sends `hello` when it attaches to a machine (a cold boot or a restored state) and, until it hears one back, again every 5 s at most. After a restore the helper is already running inside the saved state with the port open, so it answers at once; bytes the page queued before the save are gone with v86's input array, which `hello` resets. The page also sends `time set` after every `hello` exchange.
- `docs/host-link.md` (WP-HOST-2) is the specification; golden byte vectors under `tests/host-link/` are read by the Rust codec's tests and the TypeScript package's tests alike, so both ends are held to the same bytes.

## The guest helper

- **What it is.** `HOSTLINK.EXE`, a `no_std` Rust Win32 program in the `win9x/` workspace (`win9x/hostlink`), built with the same Pentium target, no C runtime, ANSI imports only, from `win9x/build.sh`. It uses `crates/quill-hostlink`, the link's portable codec (no allocation, no panics), which the root workspace tests on the host.
- **Where and when.** `C:\Quill\HOSTLINK.EXE`, started by a second Run value in `qagrr/quill-run.reg` (`"HostLink"="C:\\Quill\\HOSTLINK.EXE"`), so it is running when the state is saved. One instance only (a named mutex, then `FindWindowA` on its window class `LHQHostLink`); `HOSTLINK.EXE /quit` ends the running one; `HOSTLINK.EXE /send <file>` hands a file to it and exits with 0, or with 3 when no helper is running.
- **No window shown.** A hidden top-level window owns the clipboard chain and gets `WM_COPYDATA`. No tray icon in the first version (owner decision below).
- **The port.** Opened with `CreateFileA("\\\\.\\COM1")`, no sharing, 115,200 8N1 with no hardware or software flow control (`SetCommState`), a 64 KiB input buffer (`SetupComm`), `PurgeComm` on open. One I/O thread does all reading and writing, with reads that time out (an overlapped `ReadFile` with a 250 ms total time-out, or a synchronous 50 ms one) and writes between reads; never `WaitCommEvent`, which `serial.sys` cannot cancel (WP-HOST-1). The window thread and the I/O thread meet through two queues and an event; the clipboard is only ever touched on the window thread.
- **Watching the clipboard.** `SetClipboardViewer` at start; `WM_DRAWCLIPBOARD`: pass it on with `SendMessageA` to the next viewer, then (after a 30 ms debounce, so an application's several `SetClipboardData` calls make one offer) read the formats; `WM_CHANGECBCHAIN`: repair the chain as the documentation says; `ChangeClipboardChain` on exit. A change made by the helper itself (`GetClipboardOwner` is its window) is reported as `source=self` and the page ignores it. `GetClipboardSequenceNumber` (present on ReactOS) numbers the offers; on Windows 95, where it is missing, a counter does.
- **Reading.** `CF_UNICODETEXT` (or `CF_TEXT` through the ANSI code page when only that is there) as UTF-8; `Rich Text Format` as bytes up to its terminating NUL; `CF_DIB` (ReactOS makes it from a `CF_BITMAP`, which is what Print Screen puts there, as Windows does). Text up to 256 KiB and RTF up to 1 MiB go to the page right after the offer, so a copy reaches the host inside the gesture window; a picture is sent only when the page asks (`clip get`).
- **Setting.** On `clip set` the helper gathers the parts, then `OpenClipboard`, `EmptyClipboard` and `SetClipboardData` for `CF_UNICODETEXT`, `CF_TEXT` (Windows-1252 best fit, `?` otherwise), `Rich Text Format` and `CF_DIB` as sent, `CloseClipboard`, and answers `ok`. Memory is `GlobalAlloc(GMEM_MOVEABLE | GMEM_DDESHARE)`, as the clipboard requires.
- **The other channels** are small modules beside the clipboard's: `files.rs` (writes into `C:\From your computer\`, a name already there becomes `name (2).ext`, a partial file is deleted on `failed`, Explorer opens on the folder once a session after the first file), `time.rs` (`SetLocalTime`), `display.rs` (`EnumDisplaySettingsA`, `ChangeDisplaySettingsA` with `CDS_UPDATEREGISTRY` off, so the next boot keeps the image's mode), `send.rs` (the `/send` and `WM_COPYDATA` path, streaming a file as `LHQ-FILE` frames).
- **Imports** are checked by its own allowlist, `win9x/tools/hostlink-imports.allow`, every entry an ANSI function present on Windows 95 and in ReactOS 0.4.16 (checked against the export lists above), and by `scan-pentium.sh`.

## The page

### The package

`host-link` sits beside the AC'97 package and copies its layout and build: `package.json` (private, ES module, no runtime dependency), `tsconfig.json` (strict), `README.md` (what it does, the API, the protocol's link to `docs/host-link.md`, the browsers it was tested in), `src/`, `test/`, and the pinned v86 version it was tested against. If the AC'97 package has not merged when WP-HOST-2 starts, WP-HOST-2 uses `site/try-reactos/packages/host-link/`, tests with `bun test`, builds one ES module with `bun build` into `site/try-reactos/lib/host-link.js`, commits it, and adds a drift check; WP-WEB-5's choice wins when it lands, and the second to merge follows the first.

```text
src/index.ts        attach(emulator, options) -> HostLink; the public types
src/frame.ts        the frame parser (both kinds, any split, resync) and encoder; CRC-32
src/link.ts         hello, credits, channels, priorities, the send queue, statistics
src/clip/state.ts   the clipboard state machine (offers, gestures, pushes, dedupe), no DOM
src/clip/browser.ts the only file that touches navigator.clipboard and paste events, behind a ClipboardPort interface
src/clip/rtf.ts     RTF to a small document model; the model to RTF
src/clip/html.ts    HTML to the model (through DOMParser, a whitelist); the model to HTML
src/clip/text.ts    UTF-8, UTF-16 and Windows-1252
src/clip/dib.ts     DIB to RGBA and back; PNG through the canvas
src/files.ts        received files (both frame kinds), sends from a drop or a picker
src/time.ts, src/display.ts
```

The page's glue, `site/try-reactos/host-link.js` (WP-WEB-1's empty `host-files.js` seam, renamed, one import line in `try-reactos.js`), builds the UI from the package's events: the clipboard chip and its menu, the *From the PC* list, the drop zone, the *Send a file to the PC* button and the diagnostics rows. Every string is plain Canadian English in the glue, not the package.

### The keys the page catches

Only while the screen has the keyboard (the visitor selected it) and sharing is on; otherwise every key goes to v86 as today.

| Keys | The page | The guest gets |
|---|---|---|
| Ctrl+C, Ctrl+X, Ctrl+Insert, Shift+Delete | Lets v86 send them; records a copy gesture (the time), and on a browser without `navigator.userActivation` also arms a promised write (below) | The keys |
| Ctrl+V, Shift+Insert | Stops the V or Insert in the capture phase (v86 never sees it), focuses the hidden paste box, takes the `paste` event, pushes the clipboard, then sends the V or Insert down and up with `keyboard_send_scancodes`, adding Ctrl or Shift down and up itself if the visitor let go meanwhile | The new clipboard, then the key |
| Cmd+C, Cmd+X, Cmd+V, Cmd+A, Cmd+Z on a Mac | The same as the Ctrl forms, with Ctrl sent to the guest instead of the Windows key (v86 would send Meta, which opens the Start menu) | Ctrl+ the key |

The paste box is an off-screen `textarea` (`aria-hidden`, no `phone_keyboard` class, so v86 ignores keys while it has focus); focus returns to the screen inside the `paste` handler, before the visitor's key-up, so v86 sees the Ctrl release. If no `paste` event comes within 300 ms, focus returns and the key is sent anyway.

### The clipboard chip

A small control in the row with Start, Stop, Sound and Full screen: an icon and one word, *Clipboard*, with a state: **Shared** (idle), **Off**, **Copied to your computer** and **Pasted into the PC** (each for 2 s), **Sending… 40 %** (a push over 1 s), and **PC clipboard changed** with a **Copy to your computer** button when a write could not be made. Its menu holds **Share the clipboard** (a checkbox, remembered in `localStorage` behind `try`), **Paste from your computer** and **Copy to your computer**, and one line saying what sharing does. Its text is announced politely to screen readers. The diagnostics panel gains a *Link* row (helper version, protocol, bytes each way, messages, errors by code, credit) and a *Clipboard* row (offers, writes, pushes, refusals; never content).

## The clipboard flows

### Copy in the guest, paste on the host

1. The visitor copies in the guest (Ctrl+C or a menu). The page has recorded the gesture: the keydown on the screen, or a pointer-up on it for a menu or a toolbar button.
2. The guest program fills its clipboard. The helper gets `WM_DRAWCLIPBOARD`, waits 30 ms, reads the formats and sends `offer` followed by `data` for the text and the RTF.
3. The page, on the offer: if the newest gesture is under 5 s old and the offer's `source` is `user`, it converts (text as is; RTF to HTML; a DIB, fetched with `get`, to PNG through an `OffscreenCanvas` or a canvas) and calls `navigator.clipboard.write([new ClipboardItem({ "text/plain", "text/html", "image/png" as present })])`. The chip says *Copied to your computer*.
4. If there was no recent gesture, the write is refused, or the picture is still arriving when 5 s have passed, nothing is written; the chip shows *PC clipboard changed* and its **Copy to your computer** button, whose click is a new gesture and writes the data the page already holds.
5. At most one automatic write per gesture; a guest program that changes its clipboard again and again only keeps the chip lit.

**Browsers without `navigator.userActivation`** (some Safari versions): the Ctrl+C keydown itself calls `navigator.clipboard.write` with a `ClipboardItem` whose `text/plain` and `text/html` values are promises the offer resolves (Safari's own pattern for a delayed write; Chromium and Firefox accept it too, measured). A promise not resolved within 2 s rejects, the write fails quietly and the chip offers the button. Pictures always take the button there, because the item's types must be named at the keydown.

**No async clipboard** (an insecure address on a local network): the button uses `document.execCommand("copy")` with a `copy` listener that sets `text/plain` and `text/html`.

### Copy on the host, paste in the guest

1. The visitor presses Ctrl+V (Shift+Insert, Cmd+V) with the screen selected. The page stops the V, focuses the paste box, and the browser fires `paste` there.
2. The page reads the event's `clipboardData`: `text/plain`, `text/html`, `text/rtf` if the browser offers it, and image files (`kind` `file`, `image/*`). It reads nothing else and keeps nothing after the push.
3. If the content's hash equals the last push and the guest has not offered anything since, the guest already has it: the page sends the V at once.
4. Otherwise it converts: text to UTF-8 (the helper makes `CF_UNICODETEXT` and `CF_TEXT`); `text/rtf` as is, or HTML to RTF; the first image decoded with `createImageBitmap` to a 24-bit DIB (at most 4,096 pixels a side). It sends `clip set` and the parts within the credit, and the chip shows *Sending…* if it takes over 1 s; Esc on the screen cancels, and the V is then not sent.
5. On the helper's `ok` (normally under 0.5 s for text) the page sends the V; on `refused` or after 10 s it shows why and sends nothing.

### Paste with the mouse, and phones

The page cannot see the guest's Edit > Paste or a right-click Paste. **Paste from your computer** in the chip's menu calls `navigator.clipboard.read()` (Chromium asks for the permission once; Firefox shows its own Paste button; Safari its Paste callout), pushes the result, and says "Your clipboard is in the PC: paste there". The same item is how a phone or tablet pastes; if `read` is missing or refused, it opens a small dialog with a text box to paste into by a long press, and a **Send to the PC** button. Copy on a phone is the chip's **Copy to your computer** button.

### Firefox, Safari and mobile, in short

- **Chromium and Firefox** take the main path as measured. Firefox refuses a write after 5 s, which is the rule the page uses everywhere. Firefox's own Paste button appears on the chip's **Paste from your computer**, not on Ctrl+V.
- **Safari** (not measured): the `paste` event path is standard; writes use the promised item when `navigator.userActivation` is missing; Cmd keys are mapped. Pictures copy through the button.
- **Mobile**: no keyboard shortcuts; the chip's two items are the way in and out.

## Privacy and security

1. **Nothing leaves the tab.** The link runs between the page and the emulator in the same tab. The package makes no network request; an image in pasted HTML that points at a web address is dropped with its alt text, never fetched. Clipboard content lives in memory only, for the length of a push or until the next copy, and is cleared on Stop, Reset and page hide; it is never written to storage. Only the on or off setting is stored.
2. **The host clipboard is read only on an explicit paste gesture**: Ctrl+V, Shift+Insert or Cmd+V on the selected screen (the `paste` event, no permission), or a click on **Paste from your computer** (`navigator.clipboard.read`). Never on a timer, focus, visibility change, guest request or page load. The protocol has no message by which the guest can ask for the host clipboard, and `ClipboardPort.read` is called only from those two gestures, which a unit test asserts against a forged `clip get` from the guest.
3. **The host clipboard is written only after the visitor's own copy** in the guest (a gesture under 5 s old, one write per gesture) or a click on **Copy to your computer**.
4. **A visible indicator**: the chip shows *Shared* or *Off* at all times while the machine runs, and names each transfer as it happens.
5. **A setting to turn it off**: **Share the clipboard** in the chip's menu (default on, remembered), and `?clipboard=off` in the address. Off: no keys are caught (Ctrl+V reaches the guest as keys, as today), offers are ignored, and the page sends `ctl op=pause channel=clip`, so the helper stops sending data.
6. **Size limits**, enforced on both sides before data moves: text 1 MiB, RTF 4 MiB, a picture 4 MiB as a DIB from the guest (about 14 s on the link) and 4,096 pixels a side into the guest; a dropped file 32 MiB; a received file 64 MiB. Over a limit: refused with the size and the limit in the chip's line; nothing partial is written.
7. **Content is rebuilt, not passed through.** HTML from the host is parsed into the converter's model and only the model reaches the guest as RTF; RTF from the guest becomes HTML written by the converter, with every character escaped. No script, style sheet or link survives either way.
8. **Files** move only when the visitor drops or picks them (into the guest) or clicks **Download** (out of it).

## Quill's own rich copy and paste

- **What it needs.** WP-ED-2, WP-ED-5 and the Win32 half of WP-ED-6 merged before WP-WEB-8, so `QUILL32.EXE` puts `Rich Text Format`, `CF_TEXT` and (for a lone picture) `CF_DIB` on the Windows clipboard and reads them back. The editor track has WP-ED-2 ready and WP-ED-5 waiting only for it; this plan asks for the three to be pulled forward (owner decision below).
- **Quill to the host.** Copy in Quill gives the helper RTF and text; the page turns the RTF into HTML, and LibreOffice or Word on the host pastes it with its formatting. What the converter keeps: bold, italic, underline, strikethrough, face (as `font-family` with a generic fallback), size, colour, highlight, alignment, left, right and first-line indents, space before and after, line breaks, tabs (as a tab character in a `white-space: pre-wrap` span), bullets and numbered paragraphs (as `ul` and `ol` when the RTF says so, otherwise as text), and pictures (`\pngblip` and `\dibitmap` as `img` with a `data:` address). Page breaks, headers, footers, fields and Quill's styles by name do not travel; a lone picture also goes as `image/png`.
- **The host to Quill.** LibreOffice and Word put HTML and RTF on their clipboard. Where the browser's `paste` event offers `text/rtf`, it goes to the guest as it is (Quill's RTF reader takes what it understands); otherwise the page converts the HTML to RTF with the same subset. Quill's paste prefers `Rich Text Format`, then `CF_DIB`, then `CF_TEXT` (SPEC), so formatting arrives where Quill can show it.
- **Characters.** Quill is Windows-1252 until Unicode U2 (WP-ED-11): characters outside it paste into Quill as `?`, while Notepad gets them through `CF_UNICODETEXT`.
- **Quill for Windows 3.x** under WineVDM shares the Win32 clipboard for `CF_TEXT` at least; the RTF and DIB bridges are not verified and not in the first demo.
- **If WP-ED-6 is late** (owner decision below): a stopgap in the Win32 shell only, which copies `App.clip`'s text to `CF_TEXT` after Copy and Cut and reads `CF_TEXT` into it before Paste, gives plain text both ways; WP-ED-6 replaces it.

## Files, time and screen size on the same link

- **Downloads (WP-WEB-2's goal, now WP-HOST-5).** `LHQ-FILE` frames and `file` messages from the guest both land in the *From the PC* list (name, size, time, **Download**, damaged when the length or sum fails). Quill's Print to PDF (W9X-90) hands the PDF to the helper when `QUILL.INI` has `[Web] HostPort=LINK`: `FindWindowA("LHQHostLink")` and `WM_COPYDATA` with the path; `HostPort=COM1` keeps the plan's direct write for a disk with no helper. Quill still knows nothing of a browser.
- **`SENDHOST.CMD`** for any other file: `"C:\Quill\HOSTLINK.EXE" /send "%~1"`, and when that exits with 3 (no helper), the direct `LHQ-FILE` frame to COM1 as planned. The desktop shortcut *Send to this computer* runs `HOSTLINK.EXE /send`, so a file dropped on it is sent (whether ReactOS passes a dropped file to a shortcut is still to check).
- **Into the guest.** A file dragged onto the screen (or picked with **Send a file to the PC**, which phones need) is sent as `file begin`, chunks and `end` with its SHA-256; the helper writes it to `C:\From your computer\` and Explorer opens on that folder after the first one. The page shows progress per file and refuses over 32 MiB.
- **Time.** The saved state's clock is the moment the state was built. On every `hello` exchange, every 15 minutes, and when the tab becomes visible again after an hour, the page sends the visitor's local wall time and the helper calls `SetLocalTime`. The guest's time zone is left alone, so the taskbar clock reads the visitor's time.
- **Screen size.** **Fit to window** (off by default, in the diagnostics panel first): the page sends the stage's size, the helper picks the largest mode from `EnumDisplaySettingsA` that fits (never below 800 by 600), changes to it for this session only, and answers `changed`; the pointer module parks on v86's `screen-set-size`. The saved state stays 1024 by 768.

## What is in the first demo

| Item | First demo | Why |
|---|---|---|
| Clipboard text both ways (Notepad, Run box, Explorer, Quill) | **Yes** | The owner's ask |
| Rich text both ways (Quill RTF to HTML; HTML or RTF to Quill) | **Yes**, with WP-ED-6's Win32 half; plain text through the stopgap if it is late | The owner's ask; Quill is the demo |
| Pictures both ways (`CF_DIB` and `image/png`) | **Yes** | Print Screen and Quill's pictures |
| Clipboard chip, setting, limits | **Yes** | The privacy rules come with the feature |
| Downloads list, `SENDHOST.CMD`, Print to PDF to the list | **Yes** | WP-WEB-2's goal, already in the first demo |
| Drop or pick a file into the guest | **Yes** | The same channel the other way; small once the link exists |
| Time sync | **Yes** | A restored desktop otherwise shows the day the state was built |
| Fit to window | **If ready** before WP-WEB-8 starts; off by default | Useful, not asked for first |
| File lists on the clipboard (`CF_HDROP`), helper tray icon, delayed rendering of large pictures, resending damaged packets | Later | Not needed for the demo's story |

## Testing

| Level | What runs | Where | Checks |
|---|---|---|---|
| 1. Units | `bun test` in the package; `cargo nextest` for `quill-hostlink` | Local and CI (a new *Host link tests* job on the host runner) | Both codecs against the shared vectors; a frame split at every byte; noise, bad CRC, bad header, too large, unknown channel; credits and priorities with a fake bus; the clipboard state machine with a fake clock, fake gestures and a fake `ClipboardPort` (the privacy rules as tests); RTF and HTML converters against goldens made from `tests/docs/*.rtf`, LibreOffice's and Word's HTML and RTF; Windows-1252 both ways; DIB to RGBA and back; the Rust codec never panics on 10,000 seeded random inputs |
| 2. Wine | `HOSTLINK.EXE` and a test-only `HLCLIP.EXE` (sets and reads the clipboard from the command line) in a Wine prefix on a private Xvfb, COM1 linked to a pty the harness opens (`pty.openpty`, `dosdevices/com1`) | Local | Offer on a copy, data, `set`, loop prevention, chain repair when another viewer leaves, credits, `/send`, one instance, `/quit` |
| 3. QEMU | The QAGRR image with `snapshot=on` (no copy), a second small FAT disk made with `mformat` and `mcopy` holding `HOSTLINK.EXE` and `HLCLIP.EXE`, COM1 as `-serial unix:/tmp/qh-<pid>.sock,server=on,wait=off`, QMP on `/tmp/qm-<pid>.sock`, KVM, `-display none`; input only through QMP, pixels only from `screendump` | Local | The viewer chain in the real ReactOS, Notepad copy and paste through the socket, Print Screen as a DIB, overlapped I/O or timeouts, the page-to-guest rate with credits, `SetLocalTime` |
| 4. Page and guest | A local QAGRR build with the helper and a state rebuilt with it, the page served by `harness/serve.ts`, headless Chromium with `clipboard-read` and `clipboard-write` granted (its real headless clipboard), and Playwright's Firefox with the same flows (its headless clipboard where it works, a mocked `navigator.clipboard` and synthetic `paste` events where it would prompt) | Local | Text typed in Notepad, Ctrl+A, Ctrl+C: the host clipboard has it within 1 s; HTML written to the host clipboard, Ctrl+V in Notepad, then Ctrl+A, Ctrl+C back: the same text; Alt+Print Screen: an `image/png` of the window; the gesture rule (a copy 7 s after the last key lights the chip and writes nothing); sharing off (Ctrl+V reaches the guest as keys, no read); a file dropped arrives in `C:\From your computer\` with its sum (read with mtools from a saved disk image or sent back with `SENDHOST.CMD`); the clock after `time set` |
| 5. Into LibreOffice | The HTML the converter writes for the test documents, converted with `soffice --headless --convert-to fodt` on a private display, the result read with XPath | Local | Bold, italic, size, colour, alignment and lists survive LibreOffice's HTML import |
| 6. The owner | [The check list](#owners-check-list) | Real browsers and a real LibreOffice or Word | What headless runs cannot show |

Every browser is headless, every server binds 127.0.0.1, every command that starts a browser, Wine, Xvfb or QEMU sets `DISPLAY` explicitly with `WAYLAND_DISPLAY` unset, and every process is stopped by its own PID.

## Budgets

| What | Budget |
|---|---|
| `HOSTLINK.EXE` | At most 64 KiB (a guard in its build), ANSI imports on its allowlist, no wide entry point, the Pentium scan clean |
| The helper at rest | No timer; it wakes only for the port or a message (or at most 20 times a second with read timeouts); the guest's idle MIPS in v86 within the 5.4 to 5.7 measured by WP-WEB-4 |
| Copy out, 1 KB of text | Host clipboard written within 400 ms of the guest's copy (v86, Chromium) |
| Paste in, 1 KB of text | The V reaches the guest within 400 ms of the keydown; 100 KB of RTF within 1.5 s |
| Rates | Guest to page at least 0.25 MB/s (0.33 measured); page to guest target 0.1 MB/s (0.56 to 0.86 measured at one packet in flight) |
| The package | At most 40 KB minified, no runtime dependency |
| Memory | Page: the largest message under the limits, freed after use; helper: twice the largest message |
| The saved state | The running helper adds at most 1 MB before compression |

## Decisions to record

The next free decision numbers on `main` are SITE-15 (reserved by the web demo plan to SITE-19) and, after the pointer plan's SITE-20 to SITE-24, SITE-25. This plan reserves **SITE-30 to SITE-39** and **W9X-100 to W9X-101** (W9X-95 to W9X-99 belong to Reveal Codes and W9X-110 to W9X-119 to Preferences). Each package writes its own rows when it opens its pull request.

| Id | Decision | Written by |
|---|---|---|
| SITE-15 | (The web demo plan's number, unchanged in meaning) COM1 and the `LHQ-FILE` frame for files from the guest; the list in memory; no automatic download | WP-HOST-5 |
| SITE-30 | One host link on COM1: `LHQ-PKT` frames, channels, credits, no resend in version 1, `docs/host-link.md` as the specification and the shared vectors | WP-HOST-2 |
| SITE-31 | The page side is the `host-link` TypeScript package, laid out as the AC'97 package is; one glue module on the page | WP-HOST-2 |
| SITE-32 | `HOSTLINK.EXE` owns COM1 from logon, watches the clipboard through the viewer chain (ReactOS 0.4.16 has no format listener) and is the one sender for every other program | WP-HOST-3 |
| SITE-33 | The clipboard flows: the caught Ctrl+V and the `paste` event; writes inside a 5 s gesture or by the button; the promised write where `userActivation` is missing; Cmd keys mapped on a Mac | WP-HOST-4 |
| SITE-34 | The clipboard's privacy rules and limits, the chip and the setting (on by default) | WP-HOST-4 |
| SITE-35 | Format mapping: RTF and HTML through one model, DIB and PNG, UTF-8 on the link and Windows-1252 best fit | WP-HOST-4 |
| SITE-36 | Files into the guest land in `C:\From your computer\`; `SENDHOST.CMD` goes through the helper, with the direct frame as its fallback | WP-HOST-5 |
| SITE-37 | Time sync to the visitor's wall clock; Fit to window, off by default, for the session only | WP-HOST-6 |
| SITE-38, SITE-39 | Spare, for what WP-HOST-7 or the owner's check changes | WP-HOST-7 |
| W9X-100 | `[Web] HostPort=LINK` hands Print to PDF's file to the helper through `WM_COPYDATA`; `COMn` keeps the direct frame | WP-HOST-5 |
| W9X-101 | Spare (the stopgap clipboard bridge in the Win32 shell, if the owner chooses it) | WP-HOST-7 |

## Rules for every package

- **One package, one branch, one worktree** under `~/source/worktrees/quill/<topic>`, from `origin/main`. `~/source/experiments/quill16` is used only for `git fetch` and `git worktree add` and `remove`.
- **Browsers, Wine, Xvfb and QEMU** are started by the builder, headless or on a private display (`env -u WAYLAND_DISPLAY DISPLAY=:<n> …` in every command, a display number nobody else uses), with their PIDs recorded and stopped by those PIDs; never `pkill` or `killall` by name; never the host desktop. QEMU sockets go under `/tmp` with short names that carry the PID (`/tmp/qh-<pid>.sock`, `/tmp/qm-<pid>.sock`). QEMU boots the QAGRR image with `snapshot=on` or a copy, never in place.
- **Disk.** Keep `df -h /home` above 20 GB: there were 21 GB free when this plan was written, and a local QAGRR image is 4 GB with its build files on top. Check before a build and stop with a report rather than go under. `/tmp` is RAM: check free memory before writing more than 1 GB there.
- **Large files stay outside git**: images, parts, states under `~/.cache/quill-reactos-liminal-work/` or `~/.cache/reactos/web/`; `site/try-reactos/data/` is ignored. Never `git add -A`.
- **No publishing.** No release, tag, upload or Pages dispatch; push only the package's branch.
- **Shared files.** `site/try-reactos/try-reactos.js`: one import line and the key-catching hook only (the second to merge rebases, as with the pointer and WP-WEB-1); `index.html` and `try-reactos.css`: one marked block per package; `qagrr/qagrr.toml` and `qagrr/quill-run.reg`: one entry per package (the wallpaper, savers and sound packages touch the same files); `win9x/hostlink/src/main.rs`: the dispatch lines only, each channel in its own module; `site/try-reactos/README.md`: each package owns its section; `docs/decisions.md`: the reserved numbers above.
- **Evidence** goes to `~/Documents/Liminal HQ/Quill/Web demo/host-integration/<package>/`; the few shots that document a feature are committed under `docs/media/` as `web-host-*.png`.
- **Gates.** `bash scripts/check-headers.sh`, markdownlint in Docker, the ReactOS demo script tests (`python3 -I -m unittest discover -s site/try-reactos/guest -p 'test_*.py'`), the package's `bun test` and type check, `make validate` when `crates/**` changes, `win9x/build.sh` and the import checks when `win9x/**` changes, and the package's own harness checks.

## Order and parallelism

| # | Package | Starts when | Runs beside |
|---|---|---|---|
| 1 | **WP-HOST-1** Spike: the guest end and the browsers (one day) | Now | WP-HOST-2, WP-WEB-5 |
| 2 | **WP-HOST-2** The link: specification, vectors, `quill-hostlink`, the `host-link` package's core | Now | WP-HOST-1, WP-WEB-5 |
| 3 | **WP-HOST-3** The guest helper and its place on the disk | WP-HOST-2 merged and WP-HOST-1's findings in | WP-HOST-4 |
| 4 | **WP-HOST-4** The page's clipboard: flows, converters, chip, privacy | WP-HOST-2 merged | WP-HOST-3 |
| 5 | **WP-HOST-5** Files both ways (takes over WP-WEB-2) and Print to PDF to the list | WP-HOST-3 merged | WP-HOST-6 |
| 6 | **WP-HOST-6** Time and Fit to window | WP-HOST-3 merged | WP-HOST-5 |
| 7 | **WP-HOST-7** Quill's rich clipboard end to end, and the owner's check | WP-HOST-4 merged and WP-ED-6's Win32 half merged (or the stopgap chosen) | — |
| 8 | **WP-WEB-8** The demo build (its finish line gains the link's checks) | WP-HOST-3 to WP-HOST-7, WP-WEB-3, WP-WEB-5, WP-WEB-6, WP-WEB-7 and the savers packing merged | — |

**With the cap of three builders**, one suggestion: WP-WEB-5 (sound, already going), WP-HOST-2 and WP-HOST-1 now; WP-ED-2 in WP-HOST-1's slot after its day; then WP-HOST-3 and WP-HOST-4 as WP-WEB-5 and WP-ED-2 finish, with WP-WEB-6 and WP-WEB-7 fitted in; then WP-ED-5, WP-HOST-5 and WP-HOST-6; then WP-ED-6 (Win32 half first) and WP-HOST-7; WP-WEB-8 last. Sound goes first because it is underway and touches no file of this plan except `index.html`'s blocks; the wallpaper and the savers packing touch `qagrr.toml` only, so any order works and the second to merge rebases. Everything that changes the image is proved on the package's own local build; the published image is built once, by WP-WEB-8.

## Work packages

### WP-HOST-1 Spike: the guest end and the browsers

- **Finish line.** A findings section in this plan (under *What was measured*, "WP-HOST-1 findings", dated) answering, each with numbers or a shot: (1) the viewer chain delivers `WM_DRAWCLIPBOARD` to a `no_std` Rust Win32 program on ReactOS 0.4.16, once per copy, for Notepad, the Run box and Print Screen; (2) ReactOS makes `CF_UNICODETEXT` from `CF_TEXT` and the reverse, and `CF_DIB` from `CF_BITMAP`; (3) a registered `Rich Text Format` set by one program is read by another; (4) overlapped I/O on COM1 works in `serial.sys` (if not, read timeouts of 50 ms do); (5) the page-to-guest rate on v86 with a 16 KiB window and 4 KiB credits, and the guest-to-page rate through the helper's writes; (6) `SetLocalTime` works for the demo's user without enabling a privilege (if not, what `AdjustTokenPrivileges` needs); (7) the modes `EnumDisplaySettingsA` lists in v86 and whether `ChangeDisplaySettingsA` switches without a restart; (8) in headless Chromium and Firefox: what the `paste` event offers when the system clipboard holds RTF (Chromium's `text/rtf`), and whether a promised `ClipboardItem` holding `image/png` resolves after 3 s; (9) whether a file dropped on a desktop shortcut reaches its target as `%1` on ReactOS 0.4.16.
- **Files owned.** New `site/try-reactos/spikes/host-link/` (a throwaway Rust program in the `win9x` style, the page-side probes and the scripts that drive them; removed by WP-HOST-3), the [report](#wp-host-1-report) of this plan, evidence.
- **Must not touch.** Everything else: `site/try-reactos/*.js`, `qagrr/**`, `win9x/quill-win9x/**`, `crates/**`.
- **Dependencies.** None. The QAGRR image and parts in `~/.cache/quill-reactos-liminal-work/` (read-only).
- **Tests.** The questions above, run in private QEMU (as [Testing](#testing) level 3 describes) and, for (5) and (7), in v86 through a copy of the page in the spike folder that feeds the serial port.
- **Evidence.** Shots and logs into `~/Documents/Liminal HQ/Quill/Web demo/host-integration/WP-HOST-1/`.
- **Parallel.** With WP-HOST-2 and WP-WEB-5.

### WP-HOST-2 The link: specification, vectors, codec and package core

- **Finish line.** `docs/host-link.md` specifies the frames, messages, channels, credits, errors and versions of [The link](#the-link) exactly enough to implement from; `tests/host-link/vectors/` holds at least 30 named cases (frames, split points, errors, every message kind) as `.bin` and `.json`; `crates/quill-hostlink` (`no_std`, no `alloc`, no panics, the house lints) parses and writes both frame kinds and reassembles messages into caller buffers, passing every vector and a seeded random-input test; the `host-link` package's `frame.ts` and `link.ts` pass the same vectors, a fake-bus test of `hello`, credits, priorities, resync and errors, and the type check; `host-link.js` replaces the `host-files.js` seam with `attach` and `detach` wired to v86's serial events and the link's statistics shown in the diagnostics *Link* row. No clipboard, file or time behaviour yet.
- **Files owned.** New `docs/host-link.md`, `tests/host-link/**`, `crates/quill-hostlink/**` (with its `[package.metadata.quill]` layer), the root `Cargo.toml` members line and `Cargo.lock` (the new crate only), the package folder (`site/try-reactos/packages/host-link/` unless the AC'97 package set another layout), its built module if the layout commits one, `site/try-reactos/host-link.js` (renamed from `host-files.js`), one import line in `try-reactos.js`, the diagnostics row in `index.html`, `.github/workflows/ci.yml` (one *Host link tests* job), `site/try-reactos/README.md` (*The host link*), `docs/decisions.md` (SITE-30, SITE-31).
- **Must not touch.** `sound.js`, the AC'97 package, `manifest.js`, the pointer block of `try-reactos.js`, `qagrr/**`, `win9x/**`, `guest/**`.
- **Dependencies.** None; reads the AC'97 package's layout if it has landed.
- **Tests.** As in the finish line; `make validate`; the package's `bun test` and `tsc --noEmit`.
- **Evidence.** The test counts, the package's size, the diagnostics row on a running page (a cold boot needs no helper: the row shows "no helper").
- **Parallel.** With WP-HOST-1 and WP-WEB-5.

### WP-HOST-3 The guest helper and its place on the disk

- **Finish line.** `HOSTLINK.EXE` builds from `win9x/build.sh`, passes its import allowlist, the Pentium scan and the 64 KiB guard; under Wine (level 2) and in QEMU (level 3) it says `hello`, offers every clipboard change once with its text and RTF, sends a DIB on `get`, sets all four formats on `set` and answers `ok`, ignores its own changes, keeps the chain whole when another viewer joins and leaves, honours credits, runs once, and stops on `/quit`; `/send` exists as a stub that answers 3 until WP-HOST-5. A local QAGRR build carries it with the Run value, a state built with it running restores with the helper answering the page's `hello` within 1 s, and `check-state.sh` still passes.
- **Files owned.** New `win9x/hostlink/**` (`main.rs` with the dispatch, `port.rs`, `clip.rs`, `link.rs`), the `win9x/Cargo.toml` members line and `win9x/Cargo.lock`, `win9x/build.sh` (build and copy `HOSTLINK.EXE` and the test-only `HLCLIP.EXE`), new `win9x/tools/hostlink-imports.allow` and the check script's second exe, new `win9x/tools/wine-hostlink.sh`, new `site/try-reactos/harness/qemu-hostlink.py` (level 3), `qagrr/qagrr.toml` (its `[[files]]` entry), `qagrr/quill-run.reg` (the `HostLink` value), `qagrr/build-with-rl.sh` (only to copy the exe if it lists programs), `site/try-reactos/README.md` (*The link helper*), `docs/decisions.md` (SITE-32), removal of `site/try-reactos/spikes/host-link/`.
- **Must not touch.** `win9x/quill-win9x/**`, `crates/**` other than reading `quill-hostlink` (a codec bug goes back to WP-HOST-2's crate as a small fix with its test, listed in the pull request), the page's modules.
- **Dependencies.** WP-HOST-2 merged; WP-HOST-1's findings.
- **Tests.** Levels 2 and 3; `win9x/build.sh`; the import and Pentium checks; `check-state.sh` on the new state.
- **Evidence.** The Wine and QEMU logs, a QEMU shot of Notepad after a `set`, the exe size, the state's size before and after.
- **Parallel.** With WP-HOST-4.

### WP-HOST-4 The page's clipboard

- **Finish line.** With a local build carrying the helper: the flows of [The clipboard flows](#the-clipboard-flows) work in headless Chromium and Firefox (level 4), the privacy rules hold as tests (level 1), the chip and its menu work with the keyboard and a screen reader's names, `?clipboard=off` and the setting turn everything off, Cmd keys map on a Mac user agent, the phone dialog works at 390 pixels wide, the converters pass their goldens and the LibreOffice import check (level 5), and the size limits refuse with the right line.
- **Files owned.** The package's `src/clip/**` and its tests and goldens (new `tests/host-link/clip/`), the clipboard part of `host-link.js`, the key-catching hook in `try-reactos.js` (one call into `host-link.js` from the existing key path, nothing else), the chip's block in `index.html` and `try-reactos.css`, new `harness/check-clipboard.ts` and `harness/check-clipboard-ff.ts`, new `scripts/check-clip-html-libreoffice.sh`, `site/try-reactos/README.md` (*The clipboard*), `docs/decisions.md` (SITE-33 to SITE-35), `docs/media/web-host-clipboard.png`.
- **Must not touch.** `win9x/**` (a helper bug goes to WP-HOST-3's branch or a small fix branch), `frame.ts` and `link.ts` beyond additive changes with tests, `sound.js`, the pointer block, `qagrr/**`.
- **Dependencies.** WP-HOST-2 merged. Builds against WP-HOST-1's spike program or a fake helper until WP-HOST-3 merges; the level 4 runs need WP-HOST-3.
- **Tests.** Levels 1, 4 and 5 for the clipboard.
- **Evidence.** The chip in each state (Playwright shots of the page), the level 4 table with timings, the LibreOffice import result.
- **Parallel.** With WP-HOST-3.

### WP-HOST-5 Files both ways, and Print to PDF to the list

- **Finish line.** WP-WEB-2's finish line (the list, Download with an equal SHA-256, damaged frames, `SENDHOST.CMD`, the *Send to this computer* shortcut) through the helper, with the direct-frame fallback proved by stopping the helper; a dropped or picked file arrives in `C:\From your computer\` with its SHA-256 equal, Explorer opens on the folder after the first, a second file of the same name becomes `name (2).ext`, over 32 MiB is refused; Print to PDF in `QUILL32.EXE` with `[Web] HostPort=LINK` lists the PDF on the page.
- **Files owned.** The package's `src/files.ts` and its tests, the files part of `host-link.js`, the list and drop zone blocks in `index.html` and `try-reactos.css`, `win9x/hostlink/src/files.rs` and `send.rs` (and their dispatch lines), new `qagrr/SENDHOST.CMD` and its `[[files]]` and `[[shortcuts]]` entries, `qagrr/QUILL.INI` or the recipe's INI setting for `HostPort=LINK`, `win9x/quill-win9x/src/main.rs` (the `HostPort` hand-off only: `FindWindowA`, `WM_COPYDATA`, the status line) and `imports.allow` (those two functions), new `harness/check-host-files.ts`, `site/try-reactos/README.md` (*Files to and from the PC*), `docs/decisions.md` (SITE-15, SITE-36, W9X-100), `docs/plans/web-demo.md` (WP-WEB-2 marked as done here).
- **Must not touch.** `src/clip/**`, `clip.rs`, `sound.js`, the Print to PDF layout code in `quill-win9x` beyond the hand-off.
- **Dependencies.** WP-HOST-3 merged.
- **Tests.** The parser cases of WP-WEB-2; level 4 for files (notepad.exe out, a 1 MB and a 30 MB file in, the sums); Wine for the `WM_COPYDATA` hand-off from `QUILL32.EXE`.
- **Evidence.** The list with a good and a damaged file, the folder in the guest, timings both ways.
- **Parallel.** With WP-HOST-6.

### WP-HOST-6 Time and Fit to window

- **Finish line.** After a restore, the guest's taskbar clock shows the visitor's time within a minute of Start (level 4, with the browser's clock set through Playwright's `clock` to a known time), and again after the tab is hidden for over an hour (simulated); **Fit to window** in the diagnostics panel switches the guest to the largest listed mode that fits the stage and back to 1024 by 768 when turned off, the pointer stays under the visitor's after the change, and nothing is written to the guest's registry.
- **Files owned.** The package's `src/time.ts`, `src/display.ts` and tests, their parts of `host-link.js` and the panel, `win9x/hostlink/src/time.rs` and `display.rs` (and their dispatch lines), new `harness/check-time-display.ts`, `site/try-reactos/README.md` (*Time and screen size*), `docs/decisions.md` (SITE-37).
- **Must not touch.** `src/clip/**`, `files.ts`, the pointer module (it already parks on `screen-set-size`; a needed change goes to the pointer plan's packages).
- **Dependencies.** WP-HOST-3 merged.
- **Tests.** Level 4 for both; level 3 for `SetLocalTime` and the mode change.
- **Evidence.** The clock before and after; the desktop at a fitted size.
- **Parallel.** With WP-HOST-5.

### WP-HOST-7 Quill's rich clipboard end to end, and the owner's check

- **Finish line.** On a local build with WP-ED-6's Win32 half: a paragraph with bold, italic, a colour, a size and a list, and a lone picture, copied in Quill, reach the host clipboard as HTML and PNG that LibreOffice imports with those formats (level 5 on the captured HTML); the same content, written to the host clipboard as LibreOffice's HTML and as RTF, pastes into Quill with them (checked by copying back out and comparing the model); characters outside Windows-1252 become `?` in Quill and survive in Notepad. The owner's check list is ready to run and every trace or note the owner leaves is answered; SITE-38 and SITE-39 record what it changed. If the owner chose the stopgap: plain text both ways through `App.clip` and `CF_TEXT`, W9X-101.
- **Files owned.** New `harness/check-quill-clipboard.ts`, the converters' constants and goldens (fixes with tests), `site/try-reactos/README.md` (*Quill's clipboard on the host*), `docs/decisions.md` (SITE-38, SITE-39, W9X-101 if used), the stopgap in `win9x/quill-win9x/src/main.rs` only if chosen.
- **Must not touch.** `quill-app`, `quill-clip` (WP-ED-5's), the helper's structure.
- **Dependencies.** WP-HOST-4 merged; WP-ED-6's Win32 half merged, or the owner's choice of the stopgap.
- **Tests.** Level 4 with Quill; level 5; the owner's check list.
- **Evidence.** The host clipboard's HTML, LibreOffice's import, Quill after a paste (canvas shot).
- **Parallel.** None.

### WP-WEB-8 additions

WP-WEB-8's finish line gains: the helper running in the published state, the clipboard round trip in Notepad and in Quill, a picture out, a file in and out, and the clock, all through `check-demo.ts` on the plain static server; its dependencies gain WP-HOST-3 to WP-HOST-7.

## Builder briefs

Each brief is the common preamble followed by the package's own part. Paste both.

### Common preamble

```text
You are a Sonnet builder for Quill (repo liminal-hq-labs/quill), working on the host link of the ReactOS
web demo (site/try-reactos/): the clipboard, files, time and screen size between the page and the guest
over COM1. Read AGENTS.md, docs/plans/host-integration.md (your package, "The link", "Rules for every
package" and the section your package serves), docs/plans/web-demo.md ("Goal 2" and "Rules for every
package"), site/try-reactos/README.md and the SITE- decisions in docs/decisions.md.

Where to work: never touch ~/source/experiments/quill16 except `git fetch` and `git worktree add/remove`.
Create your worktree at ~/source/worktrees/quill/<topic> on branch <branch> from origin/main. Remove it
after the PR merges, with plain commands and full literal paths, one per command (`git worktree remove
<path>`, `git worktree prune`, `git branch -d <name>`); never globs, variables or `find -delete` in a
removal. Keep `df -h /home` above 20 GB (it was 21 GB when the plan was written: check before any image
build and stop with a report rather than go under); check free memory before writing over 1 GB to /tmp
(it is RAM).

Rules: Canadian English, real em dashes, sentence case, no hard-wrapped markdown. SPDX header on every new
source, script, TOML and YAML file (TypeScript and Rust take the `//` form). Tabs except YAML. Never
`git add -A` or `git commit -a`; stage named paths. Commits: Conventional Commits, body in markdown with
bold labels and no headings, hard-wrapped at about 100 columns, written to a file with a single-quoted
heredoc and committed with `git commit -F`, ending with `Co-Authored-By: Claude Sonnet <noreply@anthropic.com>`
naming your own model. Push only your branch; open one PR (title capitalized, imperative, no prefix;
body `## Summary` with bold lead-in bullets, `## Test plan` with the commands run, ending with an
unchecked "Not verified:" line). Do not merge.

Never publish: no release, no tag, no upload of images or states, no Pages dispatch. Images, parts and
states stay under ~/.cache; site/try-reactos/data/ is ignored by git.

Processes: every browser is headless (Chromium at /usr/bin/chromium through harness/lib.ts, or
Playwright's Firefox 159), every Wine, Xvfb or QEMU run uses a private display, and every command that
starts any of them sets DISPLAY explicitly with WAYLAND_DISPLAY unset (`env -u WAYLAND_DISPLAY DISPLAY=:<n>
...`, a display number of your own). QEMU boots the QAGRR image with `snapshot=on` (or a copy), with
`-display none`, KVM, QMP on /tmp/qm-<pid>.sock and COM1 on /tmp/qh-<pid>.sock (short paths, your own
PID); guest input only through QMP or the page's API (window.tryReactos.input), pixels only from QMP
screendump, the v86 canvas or Playwright's page screenshot. Servers bind 127.0.0.1. Record every PID you
start and stop it by that PID; never pkill or killall by name. Never show anything on, or send input to,
the host desktop.

Stay inside "Files owned". If you need a change in a must-not-touch path, stop and report what and why.
When a choice is not covered, take the reasonable default, record it in your SITE- row marked **Assumed**,
list it in the PR as "OWNER DECISION: question | default taken | where to swap", and keep going.

Evidence: into ~/Documents/Liminal HQ/Quill/Web demo/host-integration/<package>/; commit the few shots
that document the feature under docs/media/ as web-host-*.png. Before the PR: bash
scripts/check-headers.sh, markdownlint in Docker (docker run --rm -v "$PWD:/workdir"
davidanson/markdownlint-cli2:v0.17.2 "**/*.md"), the ReactOS demo script tests (python3 -I -m unittest
discover -s site/try-reactos/guest -p 'test_*.py'), the host-link package's tests and type check once it
exists, make validate if you touched crates/**, win9x/build.sh and its import checks if you touched
win9x/**, and your package's own checks. Update the README section you own and decisions.md in the same
change. Report: PR URL, what was and was not run, the evidence paths, OWNER DECISION lines, and anything
you stopped on.
```

### WP-HOST-1 Spike: the guest end and the browsers (full brief)

```text
Package WP-HOST-1, Spike: the guest end and the browsers. Branch spike/host-link, worktree
~/source/worktrees/quill/host-link-spike. Time box: one working day; the answers are the deliverable.

Read first: docs/plans/host-integration.md in full ("What was measured" is what you extend, "The guest
helper" is what your answers decide); win9x/quill-win9x/src/sys.rs, rt.rs and main.rs (how a no_std
Win32 program is built here: the target win9x/win9x.json, the entry point, the extern blocks, WndProc);
win9x/build.sh and win9x/tools/check-imports.sh; site/try-reactos/qagrr/check-win16.sh (QMP from Python:
copy its qmp() and screendump() helpers); the plan's evidence in ~/Documents/Liminal HQ/Quill/Web
demo/host-integration/plan-evidence/ (clip-probe.mjs, viewer-probe.py: start from them); v86's
src/uart.js at commit 315b2b0 (read only).

Build, under spikes/hostlink/ (a throwaway; WP-HOST-3 removes it):

1. A tiny no_std Win32 probe, HLPROBE.EXE, in its own Cargo workspace under spikes/hostlink/ that reuses
   win9x/win9x.json by path: a hidden window that joins the viewer chain and appends one line per
   WM_DRAWCLIPBOARD to C:\hlprobe.log (tick count, GetClipboardSequenceNumber, owner hwnd, every format
   from EnumClipboardFormats with its size, the first 40 bytes of CF_UNICODETEXT, CF_TEXT and "Rich Text
   Format"); and command-line modes: /set-rtf FILE (sets "Rich Text Format" and CF_TEXT), /set-text
   STRING (CF_TEXT only, then reports whether CF_UNICODETEXT reads back), /com (opens COM1 overlapped,
   echoes every byte back with a running count and logs the rate; /com-timeouts does the same with
   50 ms read timeouts), /time ISO (SetLocalTime, logs GetLastError), /modes (logs EnumDisplaySettingsA's
   list) and /mode W H (ChangeDisplaySettingsA without CDS_UPDATEREGISTRY, logs the result).
2. A small FAT disk (mformat, mcopy; a few MB under ~/.cache/quill-host-spike/) holding HLPROBE.EXE.
   Boot the QAGRR image (~/.cache/quill-reactos-liminal-work/images/qagrr.img) in QEMU with snapshot=on,
   that disk as the second IDE drive, COM1 on /tmp/qh-<pid>.sock and QMP on /tmp/qm-<pid>.sock, KVM,
   -display none. Drive it from Python through QMP only: run D:\HLPROBE.EXE from the Run box, then copy
   in Notepad, in the Run box, Print Screen and Alt+Print Screen; run the /set modes and paste into
   Notepad; read C:\hlprobe.log back by typing `type C:\hlprobe.log > COM1` (or `copy`) and reading the
   socket. Questions (1) to (4), (6), (7) and (9) of the plan's WP-HOST-1 finish line. For (4), stream
   1 MB each way through the socket against /com and /com-timeouts and time it.
3. v86: a copy of the page under spikes/hostlink/ (not the published page) that cold-boots the QAGRR
   parts with a second disk holding HLPROBE.EXE (or restores the existing state and types a command that
   copies it from a place the disk already has, your choice; say which), runs /com, and pushes 1 MB with
   emulator.serial_send_bytes in windows of 16 KiB, waiting for the echo count to advance before each
   window; then times the guest-to-page direction from the echoes. Question (5).
4. Browsers: extend clip-probe.mjs (headless Chromium with clipboard permissions, Playwright's Firefox)
   for question (8): a promised ClipboardItem with image/png resolved after 3 s; and in Chromium, what
   the paste event offers after a native RTF write is impossible from the page, so say how you tested
   text/rtf (for example a page-written "web text/rtf" does not count) or that it could not be tested
   headless.

Must not touch: everything outside spikes/hostlink/ and the plan's findings section; the QAGRR image
itself (snapshot=on or a copy only).

Finish line: the findings section "WP-HOST-1 findings (date)" under "What was measured" in
docs/plans/host-integration.md, one row per question with yes or no and the numbers, plus what WP-HOST-3
must do differently from the plan because of them. Evidence (logs, shots, the probe's source) into
~/Documents/Liminal HQ/Quill/Web demo/host-integration/WP-HOST-1/. PR title suggestion: "Spike the host
link's guest end on ReactOS" (a findings PR).
```

### WP-HOST-2 The link (full brief)

```text
Package WP-HOST-2, The link: specification, vectors, codec and package core. Branch feat/host-link,
worktree ~/source/worktrees/quill/host-link.

Read first: docs/plans/host-integration.md "The link" (the contract), "The page" (the package layout)
and "Budgets"; docs/plans/web-demo.md "The channel to the page" (LHQ-FILE, which stays as it is);
site/try-reactos/host-files.js and sound.js (the seams WP-WEB-1 left), try-reactos.js (start, stop, the
diagnostics readout); the AC'97 package if it has merged (copy its package.json, tsconfig, README shape,
test runner and build exactly; if it has not, use the plan's default layout and say so); crates/quill-inflate
(the style and lint set of a no_std codec crate) and a crate's [package.metadata.quill] layer;
v86/v86.d.ts (serial0-output-byte, serial_send_bytes).

Build, in this order, one commit each where it makes sense:

1. docs/host-link.md: the specification, written from the plan's "The link" and made exact: the header
   grammar as ABNF, the CRC's coverage with a worked example, the message body format, every op with its
   keys, types, limits and direction, credits with a timeline, the error codes, version rules, and the
   restore sequence. Markdown, no hard wrap.
2. tests/host-link/vectors/: at least 30 cases, each NAME.bin (the bytes on the line) and NAME.json (the
   frames or messages they decode to, or the error and the byte at which the parser resyncs). Include:
   each op; a message over several packets; LHQ-FILE with and without SHA-256; noise before, between and
   after frames; a header split anywhere; a bad CRC; a bad header; a length over 4,096; an unknown channel;
   a message over max; a zero-length payload; the largest header.
3. crates/quill-hostlink: no_std, no alloc, forbid(unsafe_code), the house clippy denies (no unwrap,
   expect, panic, indexing). A push parser (feed bytes, get events: packet, file begin, file bytes, file
   end, error) holding at most one header line; an encoder writing a packet into a caller's buffer; CRC-32;
   a reassembler over a caller's buffer; the key=value body reader and writer. Tests: every vector, every
   vector fed one byte at a time and at random split points, and 10,000 seeded random inputs without a
   panic. Add it to the root workspace; make validate must pass.
4. The package: package.json, tsconfig (strict), README, src/index.ts (attach, detach, the public types),
   src/frame.ts (the same parser and encoder in TypeScript), src/link.ts (hello and its 5 s retry, credits,
   per-channel queues sent round robin in the plan's priority order, the error replies, statistics); tests
   that read the same vectors, and a fake-bus test (an object with add_listener and serial_send_bytes)
   for hello, credits, priorities (a file never delays a clip message by more than one packet), resync
   and errors. The type check passes.
5. site/try-reactos/host-link.js: git mv host-files.js host-link.js, change the one import in
   try-reactos.js, attach the link on start and detach on stop, and a "Link" row in the diagnostics
   panel (helper version or "no helper", protocol, bytes each way, messages, errors by code, credit).
   Index.html: the row only.
6. .github/workflows/ci.yml: one "Host link tests" job on the host runner: checkout (pinned as the other
   jobs pin it), bun (as the AC'97 package's job sets it up, or the setup the site jobs use), the
   package's tests and type check. README: "The host link" section. decisions.md: SITE-30 and SITE-31,
   with alternatives (a binary framing such as COBS, LPT1, the network relay, one frame per channel,
   resending in version 1, AddClipboardFormatListener) and consequences, in the SITE- rows' style.

Must not touch: sound.js, the AC'97 package, manifest.js, the pointer block of try-reactos.js,
qagrr/**, win9x/**, guest/**, site/try/**.

Finish line checks (all must hold): every vector passes in both codecs; the random-input test passes;
make validate passes; the package's tests and type check pass; a cold boot of the page (no helper on the
disk yet) shows "no helper" in the Link row and nothing else changes (qagrr/check-state.sh still passes);
the headers check and markdownlint pass. Evidence into ~/Documents/Liminal HQ/Quill/Web
demo/host-integration/WP-HOST-2/. PR title suggestion: "Specify the host link and build its codecs".
```

### WP-HOST-3 to WP-HOST-7

```text
Package <id>, <title from the plan>. Branch <feat/host-link-helper | feat/host-link-clipboard |
feat/host-link-files | feat/host-link-time-display | feat/host-link-quill-clipboard>, worktree
~/source/worktrees/quill/<same topic>.

Read first: the plan's section for your package, "The link", the section your package serves ("The guest
helper", "The clipboard flows" and "Privacy and security", "Files, time and screen size on the same
link", or "Quill's own rich copy and paste"), docs/host-link.md and the vectors as WP-HOST-2 merged them,
WP-HOST-1's findings in the plan, and the files your package owns.

WP-HOST-3 builds HOSTLINK.EXE in win9x/hostlink from WP-HOST-1's probe and quill-hostlink, with its own
import allowlist, the Wine harness (a pty for COM1, HLCLIP.EXE for the clipboard) and the QEMU harness,
then adds it to the QAGRR recipe and the Run key and proves a state built with it (build-with-rl.sh,
build-state.sh, check-state.sh on a local build). WP-HOST-4 builds src/clip/** with the ClipboardPort
seam first and its unit tests (the privacy rules are tests), then the converters with goldens, then the
chip and the key hook, then the level 4 runs in both browsers and the LibreOffice import check.
WP-HOST-5 moves WP-WEB-2's finish line onto the link, adds the drop zone and the picker, SENDHOST.CMD
with its fallback, and Quill's HostPort=LINK hand-off (W9X-100). WP-HOST-6 adds time and Fit to window.
WP-HOST-7 waits for WP-ED-6's Win32 half (or the owner's stopgap choice), proves Quill's rich copy and
paste with the host, and prepares and answers the owner's check list.

Build what the plan's Finish line says, in small commits, staying inside Files owned. Finish line,
tests, evidence and decision numbers: the plan's package section. PR title suggestions: "Add the
ReactOS demo's link helper to the guest", "Share the clipboard between the page and the ReactOS guest",
"Move files both ways between the page and the guest", "Set the guest's clock and fit its screen from
the page", "Carry Quill's rich copy and paste to the host".
```

## Owner decisions

Each is taken with the default so no package waits; the last column is where to change it.

| Question | Default taken | Where to swap |
|---|---|---|
| Clipboard sharing on or off when the page opens | **On**, remembered per browser, with the chip always showing it | `host-link.js`'s default; SITE-34 (WP-HOST-4) |
| Write the host clipboard automatically after a guest copy, or always through a button | Automatically when the visitor's own gesture is under 5 s old; the button otherwise | The gesture window in `src/clip/state.ts`; SITE-33 |
| Rich text on the host as HTML (no browser can write `text/rtf`) | HTML plus plain text; RTF only from host to guest, where the `paste` event offers it | `src/clip/browser.ts`; SITE-35 |
| Pull WP-ED-2, WP-ED-5 and WP-ED-6's Win32 half forward so Quill's copies reach the host in the first demo | **Yes**, ahead of the other editor packages | `docs/work-packages.md`'s waves; this plan's order table |
| If WP-ED-6 is late: ship a plain-text stopgap in the Win32 shell | Only if WP-ED-5 has not merged when WP-HOST-4 does | WP-HOST-7; W9X-101 |
| The helper owns COM1, and Quill's Print to PDF hands its file to it (`HostPort=LINK`) instead of opening the port | Yes; `HostPort=COM1` stays for a disk without the helper | W9X-100 (WP-HOST-5); WP-WEB-3's send step |
| Where dropped files land in the guest | `C:\From your computer\`, Explorer opening on it after the first | `win9x/hostlink/src/files.rs`; SITE-36 |
| Size limits | Text 1 MiB, RTF 4 MiB, picture 4 MiB from the guest and 4,096 pixels a side into it, drop 32 MiB, download 64 MiB | The limits table in `docs/host-link.md`; SITE-34 |
| Time sync | The visitor's wall time into the guest, the guest's time zone unchanged | `time.rs`; SITE-37 |
| Fit to window in the first demo | Only if ready before WP-WEB-8; off by default | SITE-37; WP-HOST-6 |
| A tray icon for the helper | None in the first version | `win9x/hostlink`; SITE-32 |
| The package's layout and build if the AC'97 package has not set one | `site/try-reactos/packages/host-link/`, `bun test`, one built module committed with a drift check | SITE-31 (WP-HOST-2) |
| Disk space for the local image builds | Builders stop below 20 GB free; 21 GB were free on 2026-11-27, and `~/.cache/quill-reactos-liminal-work/images/qagrr.before2.img` and the older of the two `golden/qagrr-*.img` (4 GB each) look like candidates for the owner to remove | The owner |
| HTTPS on `labs.liminalhq.ca` | Now works (owner, 2026-11-27), so the published page is a secure context and the async clipboard is there; the `execCommand` fallback stays for local addresses | `src/clip/browser.ts` |

## Owner's check list

On the owner's desktop (Firefox on Wayland, then Chromium), with LibreOffice and, if at hand, Word, then a phone. Each item is a yes or no; any no gets a note and, where it helps, the diagnostics panel's rows, into `~/Documents/Liminal HQ/Quill/Web demo/host-integration/owner/`.

1. The chip says *Clipboard: Shared* once the desktop is up; switching it off and reloading keeps it off.
2. Type a line in Notepad in the guest, select it, Ctrl+C; paste into a text editor on the desktop: the same line.
3. Copy a line on the desktop, click the screen, Ctrl+V in Notepad: the line appears, once, with no stray "v".
4. In Quill, a bold and italic phrase in red, 14 pt, and a two-item list; Ctrl+C; paste into LibreOffice Writer (and Word): the formatting is there.
5. Copy a formatted paragraph in LibreOffice; Ctrl+V in Quill: bold, italic, size and colour arrive.
6. Alt+Print Screen in the guest; paste into an image editor or LibreOffice on the desktop: the window's picture.
7. Copy a picture on the desktop (a screenshot); Ctrl+V in Quill: the picture.
8. In the guest, Edit > Copy with the mouse, wait 10 seconds, then look at the chip: *PC clipboard changed*, and **Copy to your computer** works.
9. Clipboard off: Ctrl+V in Notepad pastes whatever the guest had, and nothing from the desktop.
10. Drag a file from the desktop onto the screen: it appears in *From your computer* in the guest. Print to PDF in Quill: the PDF appears under the screen and downloads.
11. The taskbar clock shows the desktop's time.
12. Phone: **Paste from your computer** and **Copy to your computer** both work, and a picked file arrives in the guest.

## Risks

- **v86 internals.** The link uses only v86's public serial API (`serial0-output-byte`, `serial_send_bytes`), but the key catching depends on v86's keyboard listening on `window` in the bubble phase and skipping text boxes (v86 0.5.473); a v86 update could move it. `PROVENANCE.md` gains a line to check it.
- **ReactOS's `serial.sys` in v86** stops taking bytes when more than one packet is queued while the guest transmits (WP-HOST-1: see its report); one packet in flight avoided it in every run, and a stalled guest is noticed by the 5 s credit time-out, not repaired.
- **Browser rules move.** The 5 s activation, the promised `ClipboardItem` and the Paste prompts differ by browser and version; the page decides by feature, not by user agent, and always keeps the button.
- **Safari** was not measured; its path is the documented one.
- **The editor track.** Quill's part of the clipboard needs three editor packages that were not planned for the demo's date; the stopgap is the fallback.
- **The shared files** (`try-reactos.js`, `index.html`, `qagrr.toml`, `quill-run.reg`) are touched by the sound, pointer, wallpaper and savers packages too; blocks and one-line hooks keep the rebases small.
- **Disk space** is near the floor for image builds; a builder that cannot build an image stops, and the owner frees space.
- **A guest program that sets its clipboard constantly** lights the chip constantly; the page rate-limits the chip's announcements to one a second.

## What was not verified

- Why the guest stops taking bytes from COM1 when more than one packet is queued while it transmits (the UART state is in the [WP-HOST-1 report](#wp-host-1-report); the cause in `serial.sys` or v86's UART was not found), and whether the odds depend on host load (they moved between runs).
- A second UART (COM2) for page-to-guest data: one attempt, nothing arrived.
- Why `SetLocalTime` does nothing in v86, and a file dropped on a desktop shortcut (one attempt, nothing started).
- What the `paste` event offers from a native RTF clipboard on Windows and macOS (Linux and X11 only: both browsers offered `text/rtf`), and how LibreOffice and Word paste the converter's HTML through a real clipboard.
- Safari on any device, mobile browsers, and Firefox's Paste button on **Paste from your computer**.
- Quill for Windows 3.x's clipboard through WineVDM beyond `CF_TEXT`.
- The size of the helper, its idle cost in v86, and its effect on the saved state.
