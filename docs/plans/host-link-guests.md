# Host link guests: one clipboard pattern, a small helper per guest

A plan for making the host link (the protocol in [host-link.md](../host-link.md), the page's `host-link` package and the guest's `quill-hostlink` and `quill-hostlink-guest` crates) a reusable pattern: the same clipboard, files and printing between a browser page and any guest that v86 (or js-dos) runs, with one small helper per guest family. The owner, 2026-11-28: "For the shared clipboard, you said it uses an executable: is there a way to build this for different guests for the same clipboard pattern? I think you called it host link." Followed the same day by "Can TypeScript pass the printer through as well?" and "Let's also add USB two or three". Written on 2026-11-28 against `main` at `995296b`, with the guest helper (WP-HOST-3, pull request 200, branch `feat/host-guest-helper`) and the page clipboard (WP-HOST-4, pull request 204, branch `feat/page-clipboard`) open, the host link spike (WP-HOST-1, pull request 197) reported, v86 0.5.473 and js-dos 8.5.3 as vendored. Where this plan and [decisions.md](../decisions.md) disagree later, the decisions win.

## Table of contents

- [Recommendation](#recommendation)
- [What was checked](#what-was-checked)
- [The layering](#the-layering)
- [Guest families](#guest-families)
- [Transports](#transports)
- [Printing through the host link](#printing-through-the-host-link)
- [The package boundary](#the-package-boundary)
- [The first demo](#the-first-demo)
- [Privacy and security](#privacy-and-security)
- [Testing](#testing)
- [Budgets](#budgets)
- [Decisions to record](#decisions-to-record)
- [Rules for every package](#rules-for-every-package)
- [Order and parallelism](#order-and-parallelism)
- [Work packages](#work-packages)
- [Builder briefs](#builder-briefs)
- [Owner decisions](#owner-decisions)
- [Risks](#risks)
- [What was not verified](#what-was-not-verified)

## Recommendation

| Question | Recommendation | Why, in one line |
|---|---|---|
| Is the pattern reusable? | **Yes.** About four fifths of the code is already guest-independent: the frames, bodies, credits and `hello` (`quill-hostlink`, `frame.ts`, `message.ts`, `flow.ts`, `link.ts`), the helper's decisions (`quill-hostlink-guest`: offers once per change, the parts of a `set` gathered first, one packet at a time, the credit after each packet) and the page's clipboard rules (`clip/state.ts`, the converters) | `quill-hostlink-guest` already talks to the guest only through two traits, `Port` and `Clipboard`; `HOSTLINK.EXE` is the Win32 around it in about 1,000 lines |
| What a new guest needs | **Four pieces of glue**: a `Clipboard` implementation, a `Port` (the transport), an event loop that wakes for the port, the clipboard and a timer, and a start-up hook; plus a format table entry when the guest's native formats differ | Each is 100 to 300 lines on Win32; [the layering](#the-layering) has the estimate per guest |
| Guests, in order | **ReactOS and Windows 95 to XP** done (`HOSTLINK.EXE`); **Linux under X11** next (a Rust daemon, `hostlinkd`); **terminal-only Linux** through the page alone (OSC 52 in an xterm.js console); **Windows 3.x** after that, for images the owner supplies; **DOS** keeps the two shared disks of the [DOS page bridge](dos-page-bridge.md) | See [Guest families](#guest-families) for every verdict |
| Transport | **Serial now, everywhere it exists** (COM1 on Windows, `ttyS1` on Linux so the console and `getty` keep `ttyS0`); **virtio-console** for Linux once a spike proves it (no 16550 quirks, chunks not bytes); **the 9p share** for files on Linux only; **a custom v86 device last**, only if serial becomes the bottleneck | v86 0.5.473 has `virtio_console` and 9p (`create_file`, `read_file`, `handle9p`) and no vsock; a custom device needs a driver per guest OS |
| Printing | **A Print... button beside Download** for every PDF in the page's list, page-only, after WP-HOST-5 (new **WP-HOST-8**): the PDF opens in a hidden frame and the browser's own print dialog prints it on the visitor's real printer. Text jobs (DOS, Windows 3.x Generic / Text Only, LPT1 bytes) become a text PDF in the page later. **PostScript and PCL: NO-GO** (a WASM Ghostscript is 15.6 to 16.3 MB and AGPL-3.0). **Raw USB printing through WebUSB: later, optional** (Chromium only) | See [Printing through the host link](#printing-through-the-host-link) |
| USB | **Nothing now.** v86 0.5.473 has no USB controller (no UHCI, OHCI, EHCI or xHCI in `libv86.mjs`); an emulated USB printer (class 7) plugs into the same print path once the controller exists, which the AC'97 reuse plan (`docs/plans/v86-ac97-reuse.md`, being written) covers | See [USB printers](#usb-printers-emulated-and-real) |
| Package boundary | **A family of small packages named `v86-*`**: `v86-host-link` (the protocol, conformance sessions and vectors, the TypeScript page library, the Rust guest core, one reference helper per guest family) beside `v86-ac97`; in one Labs repository if the AC'97 plan picks one, otherwise their own | One name pattern lets the two live together as "virtual hardware and host integration for v86" |
| The first demo | **Unchanged: ReactOS only**, plus the Print... button (WP-HOST-8) if WP-HOST-5 lands in time | It is page-only and small; nothing else here is on the launch's critical path |
| Packages | **WP-HLG-1** (spike: a Linux guest in v86) and **WP-HLG-2** (conformance sessions and the page's transport seam) first, beside the launch work; then WP-HLG-3 (`hostlinkd`), WP-HLG-4 (the Linux guest image), WP-HLG-5 (terminal guests), WP-HLG-6 (Windows 3.x helper), WP-HLG-7 (print jobs from every guest), WP-HLG-8 (extraction), WP-HLG-9 (the live demo page) | See [Order and parallelism](#order-and-parallelism) |

## What was checked

All on this machine on 2026-11-28, by reading; no emulator, browser or server was started for this plan. The notes are in `~/Documents/Liminal HQ/Quill/Web demo/host-link-guests/plan-evidence/`.

- **The host link as built.** `docs/host-link.md` on `main` and on `feat/host-guest-helper`; `crates/quill-hostlink` on `main` (1,295 lines in six files: frames, bodies, CRC, credits, reassembly); `crates/quill-hostlink-guest` on `feat/host-guest-helper` (809 lines in `lib.rs`, 267 in `text.rs`, 1,009 of tests: `Port` and `Clipboard` traits, the helper's whole state machine); `win9x/hostlink` on the same branch (`main.rs` 291, `port.rs` 175, `clip.rs` 253, `sys.rs` 178, `rt.rs` 116 lines, and an import allowlist of 46 ANSI functions, `GetClipboardSequenceNumber` found with `GetProcAddress` so Windows 95 loads it); the page package on `main` (`frame.ts`, `message.ts`, `flow.ts`, `link.ts`, `index.ts`: 1,139 lines) and `src/clip/` on `feat/page-clipboard` (`state.ts` 651, `rtf.ts` 288, `dib.ts` 134, `browser.ts` 124).
- **The spike's verdicts** (pull request 197): the viewer chain GO with dedupe on the sequence number; serial reads with time-outs GO and `WaitCommEvent` NO-GO; one packet in flight from the page (8 of 8 transfers of 1 MiB, against 0 of 3 at two packets); 0.56 to 0.86 MB/s page to guest and 0.33 MB/s guest to page; `SetLocalTime` NO-GO in v86. WP-HOST-3 found the cold-boot deafness of `serial.sys` (the page must not speak first, SITE-52 on that branch).
- **v86 0.5.473** (`libv86.mjs`, SHA-256 `f9f3b4ca…`, the vendored file): `virtio_console` is an option and `virtio-console0-input-bytes` and `virtio-console0-output-bytes` carry `Uint8Array` chunks both ways; the 9p file system (`filesystem`, `basefs`, `baseurl`, `handle9p` for a custom 9p server, `create_file` and `read_file` on the emulator); four UARTs (`uart1` to `uart3` are options); two parallel ports (`parallel0` always, `parallel1` an option) with `parallel0-data-output`, `parallel0-control-output` and `parallel0-status-input`; and **no** vsock, UHCI, OHCI, EHCI, xHCI or anything named USB. The AC'97 package (SITE-51) already adds a PCI device through v86's internal objects (`cpu.devices.pci`, `cpu.io.register_read`, `cpu.read_blob`), so a custom device is possible on the page's side.
- **js-dos 8.5.3** (`wdosbox.wasm`): `CSerialDummy` and no `CPrinter` or `CParallel`; one string "printer" and one "parallel" (help text). The DOS page bridge's SITE-60 already found no serial device.
- **Linux images.** No Linux guest image is in the repository or under `~/.cache`. v86's own test images on `i.copy.sh`: `buildroot-bzimage68.bin` 10,068,480 bytes, `linux4.iso` 7,731,200, `linux.iso` 6,547,456, `TinyCore-11.0.iso` 19,922,944 (X11 included), published without a source offer beside them. v86's `tools/docker/alpine/Dockerfile` builds Alpine 3.21 (i386, the `virt` kernel) with a 9p root, `agetty` on `tty1` **and on `ttyS0`**.
- **WASM Ghostscript** (`npm view`): `@jspawn/ghostscript-wasm` 0.0.2, AGPL-3.0, 16,319,763 bytes unpacked; `@okathira/ghostpdl-wasm` 1.1.0, AGPL-3.0-or-later, 15,612,478 bytes.
- **WebUSB** (the WICG specification's source): the protected interface classes are audio (0x01), HID (0x03), mass storage (0x08), hub (0x09), smart card (0x0B), video (0x0E), audio and video (0x10) and wireless controller (0xE0). **The printer class (0x07) is not protected.**
- **Windows 3.x.** `docs/win16-data-segment.md`: one 64 KB data segment for statics, stack and local heap, and a Rust reference can never point into a `GlobalAlloc` block. `win16/tools/run-w31.sh` runs the owner's Windows 3.11 tree in DOSBox-X.

## The layering

```text
                guest-independent                                   per guest family (the glue)
Page   ┌──────────────────────────────────────────────┐
       │ frame.ts · message.ts · flow.ts · link.ts    │ ◄── Transport: serial n │ virtio-console │ (custom device)
       │ clip/state.ts · rtf.ts · html · dib/png      │ ◄── ClipboardPort (the browser), unchanged for every guest
       │ files · print                                │
       └──────────────────────────────────────────────┘
Guest  ┌──────────────────────────────────────────────┐
       │ quill-hostlink: frames, bodies, CRC, credits │
       │ quill-hostlink-guest: Helper, the decisions  │ ◄── Port (COM1 │ /dev/ttyS1 │ /dev/hvc0 │ OpenComm)
       │   offers once per change, set gathered first │ ◄── Clipboard (viewer chain │ X11 selections │ Win16 chain)
       │   one packet at a time, credit after each    │ ◄── event loop + clock (MsgWait… │ poll(2) │ WM_COMMNOTIFY)
       └──────────────────────────────────────────────┘ ◄── start-up hook (Run key │ autostart │ WIN.INI load=)
```

### What is guest-independent

- **The wire.** The two frame kinds, the body format, the five channels, ids, reassembly, the error codes, `hello` and its capabilities, credits and the one-packet window, the priority order. All in `quill-hostlink` (`no_std`, no allocation, host-tested against 30-odd shared vectors) and the page's `frame.ts`, `message.ts`, `flow.ts` and `link.ts` (the same vectors). Nothing in them names Windows.
- **The helper's decisions.** `quill-hostlink-guest`'s `Helper`: answering `hello`, the credit after every packet taken, the 30 ms debounce, offers deduplicated on the clipboard sequence, `source=self` for its own writes, the parts of a `set` staged before the clipboard is touched, `refused` with a reason over a limit, the big message sent one packet between reads. Tested on the host with a fake `Port` and a fake `Clipboard`.
- **The page's rules.** `clip/state.ts` (gestures, the 5 s write window, one write per gesture, the paste push, dedupe by hash, the privacy rules as tests), the RTF and HTML converters, DIB and PNG. None of it knows which guest is on the other end.

### What is not yet guest-independent

Four small things tie the pattern to Windows today; each is an additive change (WP-HLG-2), none changes a byte of version 1's existing messages:

1. **The page's transport.** `attach(emulator)` listens on `serial0-output-byte` and writes with `serial_send_bytes(0, …)`. `Link` itself already takes a `write` function; a `Transport` (`serial(n)`, `virtioConsole(0)`) passed to `attach` is about 40 lines.
2. **The formats.** `fmt` is `text`, `rtf` or `dib`, which is what Windows holds. An X11 guest holds `UTF8_STRING`, `text/html` and `image/png`; making it produce RTF and DIBs would put converters in every new helper. Version 1 can carry two more format names, `html` and `png`, offered by the guests that have them and accepted by a page whose `hello` lists them (`caps=clip,clip-html,clip-png,…`); a page that does not list them is never sent them. The page already makes HTML and PNG for the host clipboard, so its converters get shorter, not longer.
3. **The `app` key** names `HOSTLINK/0.1.0`; a new helper names itself (`hostlinkd/0.1.0`, `HOSTLNK16/0.1.0`) and the page shows it in the *Link* row. Nothing reads it but the diagnostics.
4. **Buffers.** `HOSTLINK.EXE` reassembles into a 4 MiB static lane. A Windows 3.x helper cannot (one 64 KB segment, and references cannot reach far memory): `quill-hostlink-guest` needs a streaming lane that hands each 4,096-byte payload to a sink the shell implements (`far_write` into a `GlobalAlloc` block). The `Clipboard` trait's `stage` already takes one part at a time; the change is in how `Mux` delivers a message.

### What each guest's glue costs

| Glue | Win32 (`HOSTLINK.EXE`, measured) | Linux X11 (`hostlinkd`, estimate **[E]**) | Windows 3.x (estimate **[E]**) |
|---|---|---|---|
| Transport (`Port`) | `port.rs` 175 lines: `CreateFileA` overlapped, `SetupComm(64 KiB)`, reads with a 250 ms time-out | 60 to 100: `open("/dev/ttyS1")`, `cfmakeraw`, `tcsetattr`, `B115200`, no echo, non-blocking `read` and `write` | 100 to 150: `OpenComm`, `BuildCommDCB` and `SetCommState`, `ReadComm`, `WriteComm`, `GetCommError` after each |
| Clipboard (`Clipboard`) | `clip.rs` 253: viewer chain, `CF_UNICODETEXT`, `Rich Text Format`, `CF_DIB` | 250 to 400: X11 selections over the X protocol (`XFixesSelectSelectionInput` for changes; ownership and `SelectionRequest` for `TARGETS`, `UTF8_STRING`, `text/html`, `image/png`; `INCR` over 256 KiB) | 150 to 250: `SetClipboardViewer`, `WM_DRAWCLIPBOARD`, `CF_TEXT`, `Rich Text Format`, `CF_DIB` (no sequence number: a counter) |
| Event loop | `main.rs` 291: `MsgWaitForMultipleObjects` on the read's event and the queue, the helper's timers | 80 to 150: `poll(2)` on the tty and the X connection with the helper's next deadline | 100 to 150: `EnableCommNotification` (`WM_COMMNOTIFY`, `CN_RECEIVE`) and `SetTimer`; no polling thread (Win16 has none) |
| Runtime | `rt.rs` and `sys.rs` 294: entry point, imports, no C runtime | 0: Rust `std`, statically linked against musl for i686 | The rust-win16 runtime as `QUILL.EXE` has it |
| Start-up hook | One Run value in `qagrr/quill-run.reg` | One line: an OpenRC service, or `~/.xinitrc` before the window manager | `load=HOSTLNK.EXE` in `WIN.INI`, or the StartUp group |
| Total glue | About 1,000 lines | 400 to 650 lines | 450 to 700 lines |

## Guest families

| Family | Verdict | Mechanism | Helper |
|---|---|---|---|
| ReactOS 0.4.16, Windows 95 to XP | **GO, done** on ReactOS (pull request 200); Windows 95, 98, Me, 2000 and XP **untested** but designed for (ANSI imports present on Windows 95, the sequence number looked up at run time) | COM1, the viewer chain, `CF_UNICODETEXT` or `CF_TEXT`, `Rich Text Format`, `CF_DIB` | `HOSTLINK.EXE` |
| Linux under X11, in v86 | **GO in principle, untested**: WP-HLG-1 spikes it | `/dev/ttyS1` (a second UART, `uart1: true`) or `/dev/hvc0` (virtio-console); X11 `CLIPBOARD` (and optionally `PRIMARY`) through XFixes | `hostlinkd`, Rust `std`, static i686 |
| Linux under Wayland | **NO-GO in v86**, possible elsewhere | A Wayland compositor needs far more than v86 gives a guest (no GPU, about 5 MIPS idle measured for ReactOS); where one runs, `ext-data-control-v1` (wlroots compositors) lets a client watch and set the clipboard, GNOME's does not | Later, `hostlinkd`'s second backend |
| Linux, terminal only | **GO, page-only** (no guest helper) | v86's `serial_console` with xterm.js: a program that writes an OSC 52 escape (`ESC ] 52 ; c ; <base64> BEL`: tmux, Vim, Neovim, `osc52` scripts) asks the page to copy, and the page writes the host clipboard inside the visitor's gesture window; paste is xterm.js's own (bracketed paste, typed into the line). The VGA text console of the Linux kernel does not pass OSC 52 on, so this works only with the console on a serial terminal in the page | None; an xterm.js console and its clipboard add-on on the page (MIT) |
| Windows 3.x | **GO in principle, untested**: `SetClipboardViewer`, `WM_DRAWCLIPBOARD`, `OpenComm`, `ReadComm`, `WriteComm` and `EnableCommNotification` are in Windows 3.1's USER; the helper is a rust-win16 program under the packer's segment budgets, with the streaming lane above and lower limits (text 32 KiB, RTF 64 KiB, a picture 256 KiB) | COM1, the clipboard viewer chain, `CF_TEXT`, `Rich Text Format`, `CF_DIB` | `HOSTLNK.EXE` (16-bit), WP-HLG-6; Quill for Windows 3.x under WineVDM on ReactOS needs nothing new, the Win32 helper serves it |
| DOS (the Try Quill page) | **GO through the disks**, as [the DOS page bridge](dos-page-bridge.md) plans (SITE-60): js-dos has no serial device and DOS has no system clipboard | Two shared disk images, one writer each; Quill's `/W` writes `C:\HOSTCLIP.RTF` and reads `D:\PASTE.RTF` | None beyond Quill's `/W`; the page's **clipboard rules are the same** (gestures, the chip, the converters), only the transport differs |
| DOS in v86 (not a page today) | **Possible, not planned**: v86's UART works for DOS programs that program the 16550 directly or through a FOSSIL driver | A TSR or Quill itself on COM1 | Only Quill's own clipboard: a later choice |
| KolibriOS | **Untested; out of scope** | Runs in v86 (one of v86's own demos); its clipboard is system function 54 and its serial ports need a driver; helpers are written in FASM or C for its own runtime | Nobody here writes KolibriOS programs; no demo asks for it |
| 9front | **Untested; out of scope for now** | Runs in v86; the clipboard is `/dev/snarf` (read and written as a file, no change notice, so a helper polls it twice a second), serial is `/dev/eia0`; a helper is about 200 lines of Plan 9 C | A later, small package if a 9front demo is ever wanted |
| Haiku | **Untested; out of scope** | `BClipboard` with `B_CLIPBOARD_CHANGED` watching, serial as `/dev/ports/pc_serial0`; a C++ helper. Haiku is slow in v86 | Not before a Haiku demo exists |
| FreeBSD, OpenBSD | **Later, cheap**: the same `hostlinkd` with BSD's tty calls | `/dev/cuau1`, X11 the same | `hostlinkd` compiled for the target |
| macOS, classic Mac OS | **Out of scope** | v86 is a 32-bit x86 PC: classic Mac OS is 68k or PowerPC, and Mac OS X on x86 needs far more than v86 offers and its licence forbids non-Apple hardware | — |
| OS/2, BeOS and other proprietary guests | **Out of scope** | No image the project can ship | — |

### ReactOS and Windows 95 to XP

`HOSTLINK.EXE` is the reference helper and stays as WP-HOST-3 built it. What changes for reuse is outside it: its `Clipboard` and `Port` become the Win32 implementations of the core's traits (they already are), and its tests move into the conformance sessions (WP-HLG-2) so every helper is held to the same bytes. A Windows 95 or 98 image cannot be shipped (Microsoft's licence), so Windows 95 to XP are tested only on images the owner supplies; QEMU with such an image and the level-3 harness of the host integration plan is enough.

### Linux under X11

- **The daemon.** `hostlinkd`, Rust with `std`, statically linked against musl for `i686-unknown-linux-musl` (a Pentium Pro floor; v86 runs i686 code), at most 512 KB stripped. One thread: `poll(2)` on the transport and the X connection with the helper's next deadline as the time-out. It links `quill-hostlink` and `quill-hostlink-guest` unchanged.
- **The clipboard.** The X protocol directly, either through `x11rb` (pure Rust, MIT or Apache-2.0, no libxcb needed with its `RustConnection`) or a hand-written subset; WP-HLG-3 picks by size. Changes: `XFixesSelectSelectionInput` on `CLIPBOARD` (`PRIMARY` off by default, owner decision) gives an event per new owner, which is the sequence number's stand-in (a counter). Reading: `ConvertSelection` to `TARGETS`, then `UTF8_STRING`, `text/html` and `image/png`, with `INCR` for anything over the server's request size. Setting: the daemon owns `CLIPBOARD` and answers `SelectionRequest` from what the page sent, and keeps the data until another program takes ownership (X11 has no clipboard manager by default, which is exactly the daemon's job).
- **The port.** `/dev/ttyS1` raw (`cfmakeraw`, 115,200, no flow control, `VMIN` 0 and `VTIME` 0 with `poll`), because a v86 Linux image usually has the kernel console and a `getty` on `ttyS0` (v86's own Alpine recipe runs `agetty` there). The page then attaches to serial 1. With virtio-console (if WP-HLG-1 finds it sound), `/dev/hvc0` raw and the page's `virtioConsole(0)` transport.
- **Start-up.** An OpenRC service started before the display manager, or a line in `~/.xinitrc`; it waits for `$DISPLAY` and retries every second.
- **The one rule from ReactOS that may not apply.** Linux's 8250 driver reads the UART's registers on every interrupt rather than once, so the cold-boot deafness (SITE-52 on pull request 200) may not happen; the page keeps the same rule (never speak first) for every guest, because it costs nothing.

### The Linux guest image

Nothing in the repository or the cache boots Linux in v86 today, so an image must be fetched for the spike and built for anything published.

| Image | Size | X11 | Licence and source | Use |
|---|---|---|---|---|
| `TinyCore-11.0.iso` from `i.copy.sh` | 19,922,944 bytes | Yes (Xvesa or Xfbdev, FLWM) | GPL-2.0 and others; sources on tinycorelinux.net, none beside the copy | **The spike** (WP-HLG-1), never published |
| `buildroot-bzimage68.bin` from `i.copy.sh` | 10,068,480 bytes | No | GPL-2.0 (kernel, BusyBox); no source offer beside it | The spike's terminal-only checks |
| v86's Alpine recipe (`tools/docker/alpine`), plus `xorg-server`, `xf86-video-fbdev`, a small window manager and a text editor | Not measured **[E]** 40 to 80 MB on disk, loaded file by file over 9p, so the first screen needs a fraction of it | Yes | Every package's source is in Alpine's `aports` at the release's tag | **The published demo** (WP-HLG-4), built by a script in the repository |
| A Buildroot image of our own (`make legal-info` lists every licence and keeps the sources) | **[E]** 15 to 30 MB with TinyX | Yes | GPL-2.0, MIT and others, with the sources Buildroot collects | The alternative if Alpine is too slow to boot in v86 |

The source obligation is real: publishing a Linux image means offering the corresponding source of its GPL parts, which the site's licences page would do as it does for SeaBIOS. Publishing stays the owner's.

### Terminal-only guests

For a guest whose interface is a shell, the page needs no helper at all. v86's `serial_console` option (type `xtermjs`) puts a terminal on the page; xterm.js's clipboard add-on (MIT) turns the OSC 52 escape a guest program writes into a request to set the host clipboard, which the page honours only inside the visitor's gesture window (as for any guest copy) and otherwise shows **Copy to your computer**. Reading the host clipboard through OSC 52 (`ESC ] 52 ; c ; ? BEL`) is refused always: a guest must never read the host clipboard on its own. Paste is the terminal's own paste of the browser's `paste` event. This is a page module (WP-HLG-5), useful for a Linux shell demo and for any serial-console guest.

### Windows 3.x

A Win16 helper is the same core, built by rust-win16 for 16-bit code segments, with three constraints from the data segment: the reassembly streams into far memory (the core's streaming lane, WP-HLG-2), the read buffer is 4 KiB and not 8, and the limits are lower and announced (the helper's `refused` with `reason=too-large` over them). `OpenComm`'s queues are at most 64 KiB and live outside the data segment. `EnableCommNotification` posts `WM_COMMNOTIFY` when bytes arrive, so the helper needs no timer to read; a 250 ms timer drives the core's debounce and time-outs. Windows 3.1 has no `GetClipboardSequenceNumber`: a counter of `WM_DRAWCLIPBOARD` messages, as Windows 95 has. No Windows 3.x image can be published, so the helper is tested in DOSBox-X with the owner's Windows 3.11 tree (DOSBox-X's `serial1=nullmodem` reaches a TCP socket the harness opens) and on the owner's own v86 images. The [shared libraries plan](https://github.com/liminal-hq-labs/liminal-installer/blob/main/docs/plans/shared-libraries.md)'s DLL runtime is not needed: the helper is one small program.

### DOS and the DOS page bridge

The DOS page has no serial line, so it cannot speak `LHQ-PKT`; what it shares with the host link is everything above the transport. Both pages catch Ctrl+V the same way, use the same gesture rules, the same chip and menu and the same converters (`clip/rtf.ts` for RTF and HTML), and the same files list. The recommendation is to make that explicit: the clipboard state machine's `send` and `on` go through a small `GuestClipboard` interface that the host link implements with messages and the DOS bridge implements with its disks (`C:\HOSTCLIP.*` and `D:\PASTE.*`). WP-DOSWEB-4 (the DOS page's clipboard) then reuses `state.ts` and the converters instead of copying them; the interface lands in WP-HLG-2, and WP-DOSWEB-4 uses it if it has merged.

## Transports

| Transport | v86 0.5.473 | Guest driver | Speed | Verdict |
|---|---|---|---|---|
| **Serial (16550)** | Four UARTs; `serial_send_bytes(n, …)` and `serialN-output-byte` | In every guest: `serial.sys`, Linux's 8250, Windows 3.x's `COMM.DRV`, BSD's `uart` | 0.33 MB/s out, 0.56 to 0.86 MB/s in, one packet in flight (measured on ReactOS) | **Now**, for every guest that has one |
| **virtio-console** | `virtio_console: true`; `virtio-console0-input-bytes` and `-output-bytes` carry chunks | Linux (`CONFIG_VIRTIO_CONSOLE`, `/dev/hvc0`); none on ReactOS or Windows 9x | Not measured; no per-byte events, so likely faster than serial | **Linux, after WP-HLG-1** proves it; the page's `virtioConsole(0)` transport is part of WP-HLG-2 |
| **9p file system** | `filesystem: {}` (an empty share), `create_file`, `read_file`, `handle9p` for a custom server | Linux (`9p`, `9pnet_virtio`), mounted with `mount -t 9p host9p /mnt/host`; none on ReactOS (the web demo plan's reason for COM1) | A file at a time | **Linux, for files only** (WP-HLG-3 or later): *From your computer* becomes a folder the page writes, *To your computer* one it reads; not for the clipboard (no change events) |
| **vsock** | Not present | — | — | **NO-GO** |
| **A custom `host-link` device** (a PCI function with a shared-memory ring in guest RAM, an interrupt each way) | Possible: the AC'97 package registers a PCI device the same way | A driver per OS: a Linux module or a `uio_pci_generic` user-space mapping (cheap); a ReactOS or Windows NT kernel driver (a WDM driver in Rust or C, the most expensive piece of this plan); Windows 9x can reach ports from Win32 code without a driver; Windows 3.x a VxD | Ring transfers at memory speed; no cold-boot deafness | **Last, and only on evidence**: if a demo needs more than the serial line gives (a 30 MB file takes about 90 s out), or serial's quirks cost more than a driver |

**Order:** serial now; virtio-console for Linux once the spike has measured it; 9p for Linux files when files come to Linux; the custom device not before a measured need. The page's `Transport` seam (WP-HLG-2) keeps every choice a one-line swap in the page's glue.

## Printing through the host link

The owner, 2026-11-28: "Can TypeScript pass the printer through as well?" Yes for PDFs, cheaply; yes for plain text with a small converter; no for PostScript and PCL in the page; later for real printers over USB.

### What a print job is

A **print job** is a file with an intent: the guest wants it printed, not saved. It travels exactly as a file does, on the `file` channel with its credits, priorities and SHA-256, so the codecs change nothing. Its `begin` message carries extra keys (version 1 bodies allow up to 16 fields, and a page that does not know a key ignores it):

```text
op=begin
id=7
name=Letter to Ana.pdf
size=48211
sha256=…
intent=print
mime=application/pdf
title=Letter to Ana
pages=2
paper=letter
```

- `intent` is `print` (a job) or absent (a file); `mime` is `application/pdf`, `text/plain` (with `charset=` `utf-8`, `windows-1252` or `cp437`), `application/postscript`, `application/vnd.hp-pcl` or `application/vnd.epson.escp` (raw printer bytes); `title`, `pages` and `paper` (`letter`, `a4`, `legal` or `WxH` in points) are what the guest knows, all optional.
- The helper announces it can send jobs with `print` in `hello`'s `caps`, and sends them only to a page whose `hello` lists `print`; an older page would list the job as a plain file, which is still correct.
- A dedicated `print` channel was the alternative: cleaner to read, but a change to both codecs, the vectors and every helper for no behaviour the keys cannot carry (owner decision below).

### How guests produce jobs

| Guest | How a job is made | Verdict |
|---|---|---|
| ReactOS, Windows 95 to XP: **Quill** | Print to PDF (W9X-90 to W9X-94) writes the PDF; with `[Web] HostPort=LINK` it hands the file to the helper (WP-HOST-5, W9X-100). Today that is a download. A **File > Print to your computer** item (or `[Web] PrintTo=HOST`) would send the same PDF with `intent=print` | **GO**: page-only for the Print... button (WP-HOST-8); the intent is one key in WP-HOST-5's hand-off, later |
| ReactOS, Windows: **other programs** | No printer driver ships with ReactOS 0.4.16. A spool folder the helper watches (`C:\Printed\To your computer\`, `FindFirstChangeNotificationA`, present on Windows 95 and ReactOS) takes any PDF a program saves there; a "Generic / Text Only" printer on a `FILE:` or a named-pipe port would give text jobs if ReactOS has that driver (not verified) | **Later** (WP-HLG-7); the spool folder first, the driver if it exists |
| Windows 3.x | The **Generic / Text Only** driver ships with Windows 3.1; printing to `LPT1:` in v86 sends its bytes to the page (below), printing to `FILE:` writes a file the helper sends | **Later** (WP-HLG-7), text only |
| DOS (Try Quill page) | Quill's File > Print writes plain text to a file on `C:`; the DOS page bridge's WP-DOSWEB-2 already offers **Download as PDF** for a `.PRN` or `.TXT` | **GO** through the disks; the Print... button reuses the same text PDF |
| DOS in v86, any LPT program | `LPT1` bytes (`parallel0-data-output`) collected by the page into a job that ends after 2 s of quiet, as DOSBox's printer capture does | **Later**, if a DOS-in-v86 demo exists |
| Linux | No CUPS in a small image. A shell script named `lpr` (and `lp`) that runs `hostlinkd --print FILE` (`application/pdf` or `text/plain` by its first bytes) is the whole "printer"; a CUPS backend only if an image already carries CUPS | **GO** once `hostlinkd` exists (WP-HLG-7) |

**The parallel port.** v86 does emulate LPT ports: `parallel0` is always there and `parallel1` is an option, with `parallel0-data-output` (a byte the guest wrote), `parallel0-control-output` (its strobe) and `parallel0-status-input` (the page sets BUSY and ACK). The web demo plan's measurement ([web-demo.md](web-demo.md)) found ReactOS's `parport.sys` stalls at 16 KiB because it waits for an acknowledge interrupt v86 does not raise; the page can raise it by pulsing ACK through `parallel0-status-input` after each strobe. DOS programs and Windows 3.x usually poll BUSY instead of waiting for the interrupt, so they may work as they are (not verified). js-dos's DOSBox has no parallel port and no printer capture (no `CPrinter` or `CParallel` in `wdosbox.wasm`), so `parallel1=file` is not available on the DOS page: its disks are the way.

### The page

- **The panel.** Every PDF in the *From the PC* list (WP-HOST-5's) gets a **Print...** button beside **Download**; a job with `intent=print` is listed as *Ready to print* and its Print... button is the primary one. Nothing prints by itself: the print dialog needs the visitor's click.
- **Printing a PDF.** The click puts the PDF's `Blob` URL in a hidden same-origin `iframe` and calls `iframe.contentWindow.print()` when it has loaded; Chromium's and Firefox's built-in PDF viewers print the document, not the page, through the operating system's dialog, so a real printer works without anything new. Where that fails (Safari, some mobile browsers: not verified), the button opens the PDF in a new tab, where the viewer's own Print button is one click away. The `Blob` URL is revoked when the dialog closes or the list is cleared.
- **Text jobs** become a PDF in the page: Courier, 80 columns at 10 characters to the inch, 66 lines a page, form feeds as page breaks, `cp437`, `windows-1252` or UTF-8 by the job's charset, with the converter the DOS page bridge's WP-DOSWEB-2 builds (shared, not copied). ESC/P and PCL control bytes in a text job are stripped to their printable text.
- **PostScript and PCL: NO-GO in the page.** The only renderers are Ghostscript builds for WebAssembly (`@jspawn/ghostscript-wasm` 16.3 MB unpacked, `@okathira/ghostpdl-wasm` 15.6 MB, both AGPL-3.0): about twice the whole ReactOS page's code and wasm, and a licence the project does not take on. A PostScript job is offered as a download with its type named; the guest's side should print to PDF instead.
- **Raw printing to a real printer** (WebUSB for a USB printer, Web Serial for a serial one): **later, optional, Chromium only**. See below.

### USB printers, emulated and real

The owner, 2026-11-28: "Let's also add USB two or three". A USB host controller for v86 (EHCI for USB 2.0, xHCI for USB 3.x) belongs to the AC'97 reuse plan (`docs/plans/v86-ac97-reuse.md`, being written)'s family of virtual hardware, and v86 0.5.473 has none today. For printing it matters in two ways:

- **An emulated USB printer (class 7) inside the guest.** Once a controller exists, a page-side USB device of class 7 (printer, protocol 2, bidirectional) with a `GET_DEVICE_ID` string naming what it accepts (`CMD:PDF,ESCP,PCL` or `CMD:PDF` only) gives a guest an ordinary printer to install: Windows XP and later and Linux's `usblp` know the class; ReactOS's USB printing is not verified. Every job it receives goes to the **same panel** as a print job from the host link, with its MIME type from the device ID's command set or a sniff of the first bytes. **Later**: it depends on the controller (the other plan), on a guest driver for the class (Windows 95 and Windows 3.x have none) and on a printer driver in the guest that emits PDF or text, which is the hard part on ReactOS. The host link's job channel stays the path that works without any of that.
- **A real USB printer through WebUSB.** The printer class is not in WebUSB's protected classes (checked in the specification), so Chromium can open a printer the visitor picks in its device chooser and send it bytes: ESC/P to a dot-matrix printer for the retro feel, PCL to many laser printers, or a PDF to the few that take one. Against it: Chromium only (Firefox and Safari have no WebUSB); the operating system's own printer driver usually holds the device (Windows' `usbprint.sys`, Linux's `usblp`, macOS's), so opening it fails unless the visitor detaches or replaces that driver; raw bytes depend on the printer's language; and a page that can write to a printer is a new kind of permission the visitor must understand. **Verdict: later, an optional "send raw to a printer" item in the panel for ESC/P text jobs, Chromium only, off unless chosen**, worth doing for a retro dot-matrix moment and not for ordinary printing, which the browser's dialog already does better. Web Serial (a USB-to-serial or serial printer) is the same verdict.
- **What is GO now:** the host link job keys and the Print... button. **Later:** text jobs from every guest, the LPT capture, the emulated USB printer. **Depends on the USB controller work:** only the emulated printer.

### Privacy, limits and tests for printing

- The print dialog opens only from the visitor's click on **Print...**; a guest can never open it, and a job never prints by arriving. Nothing is uploaded: the PDF is a `Blob` in the tab, printed by the browser.
- Limits: a job is a received file, so the 64 MiB download limit holds; a text job over 4 MiB or 2,000 pages is refused with its size; the page renders at most 2,000 pages of text.
- WebUSB, when it comes, asks for a device only from a click, keeps no device between visits unless the browser does, and sends only a job the visitor chose.
- Tests: headless Chromium cannot print to paper, so the checks are the transfer (a job's bytes and SHA-256 equal), the `Blob` (a valid PDF: `%PDF-` and an `%%EOF`, the page count by `pdf-lib` in the test only, or by counting `/Type /Page`), and that **Print...** calls `print` on the frame's window (a stub installed before the click records the call) and never without a click (a job arriving does not call it). The text PDF's goldens come from the DOS page bridge's tests.

### Where it fits

| Item | Cost | First demo |
|---|---|---|
| Print... beside Download for every PDF (WP-HOST-8) | Page only, about 150 lines and a test, after WP-HOST-5 | **Yes, if WP-HOST-5 is in time**: Quill's Print to PDF then prints on the visitor's printer in two clicks |
| `intent=print` from Quill (*Print to your computer*) | One INI key or menu item in `quill-win9x` and one key in the hand-off | No; after the launch |
| Text jobs, the spool folder, LPT capture, Linux `lpr` (WP-HLG-7) | Page converter shared with WP-DOSWEB-2; small helper modules | No |
| Emulated USB printer | Depends on the USB controller | No |
| WebUSB raw ESC/P | A page module and a permission story | No |

## The package boundary

### The proposal

Extract the pattern into its own package once a second guest works (WP-HLG-8), named for what it is and for the family it joins:

| Name | What it holds |
|---|---|
| **`v86-host-link`** (repository and npm-style package name; the crates `host-link-wire` and `host-link-guest`) | `SPEC.md` (today's `docs/host-link.md`, with the guest formats and print keys), `vectors/` (the frame and message vectors) and `sessions/` (the conformance sessions), the TypeScript page library (`frame`, `message`, `flow`, `link`, `transport`, `clip`, `files`, `print`), the Rust guest core (`host-link-wire` from `quill-hostlink`, `host-link-guest` from `quill-hostlink-guest`), and one reference helper per family under `helpers/`: `win32/` (`HOSTLINK.EXE`), `linux/` (`hostlinkd`), later `win16/` |
| **`v86-ac97`** (the AC'97 reuse plan's) | The sound card |
| The family | "Virtual hardware and host integration for v86": the same layout (strict TypeScript, `bun test`, a committed built module with a drift check, a pinned v86 version and a test that it matches), the same licence, one hero banner style with a distinct palette per package, and a shared demo page |

If the AC'97 reuse plan (`docs/plans/v86-ac97-reuse.md`, being written) chooses one repository for the family (for example `v86-devices` with `packages/ac97` and `packages/host-link`), the host link joins it under that layout and the names above become folder names. That plan was not pushed when this one was written; the two planners' outputs are reconciled by whichever merges second (owner decision below).

### What the repository carries

- **Licence**: `Apache-2.0 OR MIT`, with SPDX headers on every source, script, TOML, YAML and SVG file, as here. v86 is BSD-2-Clause and is a peer dependency, never vendored into the package; the demo page fetches its own pinned copy.
- **Docs**: a README with what it does and the browsers it was tested in, the specification, one page per helper (how to build, install and start it in its guest), the privacy rules, and the conformance guide (how to test a new helper).
- **A hero banner**, an SVG master rendered down by script, GNOME-style window chrome (two guests side by side sharing a clipboard).
- **A live demo page** that cold-boots a small Linux guest (WP-HLG-4's image) and, beside it or by a switch, the ReactOS remix, with one clipboard chip shared by both, so a copy in one reaches the host and pastes into the other (WP-HLG-9).
- **Owner-gated publishing.** Builders never create the repository, push to it, publish a package or deploy the demo page; WP-HLG-8 prepares everything inside this repository's `packages/` tree and the owner creates the repository and runs the first publish.
- Quill keeps using the package: `site/try-reactos/packages/host-link` becomes a pinned copy (or a git dependency) of `v86-host-link`, and `win9x/hostlink` its Win32 helper with Quill's start-up specifics.

## The first demo

- **Stays as it is.** The launch's first demo is the ReactOS remix with Quill, `HOSTLINK.EXE` and the page clipboard (pull requests 200 and 204), then WP-HOST-5 to WP-HOST-7 and WP-WEB-8 as the launch checklist orders them. Nothing in this plan is on that critical path, and no package here edits a file those packages own until they have merged.
- **Could ship with it cheaply.** The Print... button (WP-HOST-8): page-only, after WP-HOST-5, before WP-WEB-8. The `Transport` seam and the `GuestClipboard` interface (WP-HLG-2) are refactors with no visible change; they may merge before the launch if they are ready, or after.
- **Not in the first demo.** Any Linux guest, Windows 3.x, terminal guests, print jobs from other guests, USB, the new repository.

## Privacy and security

The host integration plan's rules hold for every guest; the pattern adds rules where new guests make new paths.

1. **The host clipboard is read only on the visitor's paste gesture** (the caught Ctrl+V and the `paste` event, or a click on **Paste from your computer**). No guest can ask for it: the protocol has no such message for any guest, OSC 52 read requests are refused, and a helper cannot reach it through any transport. The page's unit tests forge a `clip get`, an OSC 52 query and a print job, and assert `ClipboardPort.read` and `print` are never called.
2. **The host clipboard is written only after the visitor's own copy** in the guest (a gesture under 5 s old, one write per gesture) or a click on **Copy to your computer**, whatever the guest.
3. **Limits** are enforced on both sides before data moves, and a helper with smaller limits (Windows 3.x) announces them by refusing; the page shows the size and the limit.
4. **The page shows what is shared**: the chip names the guest's helper (`hostlinkd 0.1.0` in the *Link* row), each transfer as it happens, and the off switch covers every guest on the page.
5. **No network.** The package and every helper make no network request; v86's network stays off on the demo pages; pasted HTML that points at a web address is dropped with its alt text.
6. **Content is rebuilt, not passed through**, for every format: HTML from either side goes through the converter's model; a PNG from a Linux guest is decoded and re-encoded through the canvas before it reaches the host clipboard (a crafted PNG never reaches the host's image parsers as the guest sent it).
7. **Printing** opens the browser's dialog only from a click and uploads nothing; WebUSB, when it comes, asks for one device from one click.
8. **The helpers run with the least they need**: `hostlinkd` as the desktop user (it needs the tty group for `ttyS1`, nothing else; `time set` needs root and is a separate, optional service), `HOSTLINK.EXE` as the logged-on user.

## Testing

| Level | What runs | Where | Checks |
|---|---|---|---|
| 1. Units | `cargo nextest` for both crates (the streaming lane, the new formats, the print keys); `bun test` for the page package (the transports with a fake bus, `GuestClipboard`, the print button's rules, OSC 52 parsing) | Local and CI (the existing *Host link tests* job) | The shared vectors; the privacy tests extended to OSC 52 and print jobs; never a panic on 10,000 seeded inputs |
| 2. Conformance sessions | `tests/host-link/sessions/*.json`: scripted exchanges (what the page sends, when, and the bytes and messages the helper must answer), replayed by `tools/hostlink-conformance` against a helper on a pty or a socket, with a fake clipboard script the helper's test mode reads (`HLCLIP.EXE` on Windows, `hostlinkd --test-clipboard` on Linux) | Local; CI for the Linux helper (a pty on the runner) and the Win32 helper (Wine in the existing image, if it fits) | `hello`, a credit after every packet, an offer once per change, `set` then `ok`, `refused` over a limit, the big message one packet at a time, a `crc` error answered, a restart mid-message; every helper passes every session that its `caps` covers |
| 3. Guest in an emulator | Linux: v86 in headless Chromium with the spike image (WP-HLG-1) or the built image (WP-HLG-4); X11 on the guest's own frame buffer; input only through v86's keyboard API, pixels only from the canvas. Windows 3.x: DOSBox-X with the owner's tree, `serial1=nullmodem` to a harness socket | Local | Copy in `xterm` or the editor, the host clipboard has it within 1 s; paste back; a PNG; `PRIMARY` off; the helper survives the X server restarting; Windows 3.x: Notepad copy and paste through the socket |
| 4. The page | Headless Chromium and Playwright's Firefox, the page served on `127.0.0.1` by the harness's server, as the host integration plan's level 4 | Local | Both guests' clipboards on one page (WP-HLG-9); the Print... button calls the stubbed `print` once per click and never on arrival; a text job becomes a valid PDF |
| 5. The owner | Real browsers, a real printer, a phone | The owner's desktop | Print a Quill PDF on paper; copy between the Linux guest and the host |

Every browser is headless, every server binds `127.0.0.1` with a PID file, every command that starts a browser, Wine, Xvfb, DOSBox-X or QEMU sets `DISPLAY` explicitly with `WAYLAND_DISPLAY` unset, and every process is stopped by its own PID.

## Budgets

| What | Budget |
|---|---|
| `hostlinkd` | At most 512 KB stripped, static, i686; idle: no wake-up without a byte, an X event or the helper's own deadline |
| `HOSTLNK.EXE` (Windows 3.x) | At most 24 KB; its data segment at most 24 KB of the 64 KB (statics, a 4 KiB read buffer, the stack); the packer's segment budgets as `QUILL.EXE`'s |
| The page package | Stays under its 40 KB minified budget with the transports, `GuestClipboard` and print (WP-HLG-2 and WP-HOST-8 measure it); the xterm.js console is a separate module loaded only by a terminal page |
| The Linux demo image | The first screen within 60 s of a cold boot in headless Chromium on this machine **[E]**; at most 32 MB fetched before the first screen |
| Print... | The print call within 1 s of the click for a 1 MB PDF |
| A text job | 100 pages to a PDF in under 1 s in the page |

## Decisions to record

The next free SITE id on `main` is SITE-70, but SITE-54 to SITE-57 (the recording package, `feat/ac97-recording-resampler`) and SITE-70 to SITE-73 (the page clipboard, `feat/page-clipboard`) are taken on open branches, and SITE-51 and SITE-52 are used twice (the sound package on `main`, the helper on `feat/host-guest-helper`; the owner settles which keeps them). This plan reserves **SITE-80 to SITE-89** by hand, leaving SITE-74 to SITE-79 for whoever needs them next. SITE-80 to SITE-82 are written now as **Assumed**; the rest are for the packages.

| Id | Decision | Written by |
|---|---|---|
| SITE-80 | The host link is a reusable pattern: a guest-independent core (wire, helper decisions, page rules) and a small helper per guest family behind `Port` and `Clipboard` | This plan |
| SITE-81 | Transports in order: serial now, virtio-console for Linux after a spike, 9p for Linux files, a custom v86 device only on a measured need; the page chooses through a `Transport` seam | This plan |
| SITE-82 | Printing: a print job is a file with `intent=print` and a `mime`; the page prints PDFs through the browser's dialog from a click, makes text into a PDF, refuses to render PostScript and PCL, and leaves raw USB printing for later | This plan |
| SITE-83 | The guest formats `html` and `png`, offered by guests that have them to a page whose `hello` lists them | WP-HLG-2 |
| SITE-84 | The `GuestClipboard` interface shared by the host link and the DOS bridge | WP-HLG-2 |
| SITE-85 | `hostlinkd`: Rust `std`, static musl, `ttyS1` (or `hvc0`), X11 `CLIPBOARD` through XFixes, `PRIMARY` off | WP-HLG-3 |
| SITE-86 | The Linux demo image: its base, its size, its licences and where its source is offered | WP-HLG-4 |
| SITE-87 | The Windows 3.x helper's streaming lane and lower limits | WP-HLG-6 |
| SITE-88 | The Print... button (WP-HOST-8) | WP-HOST-8 |
| SITE-89 | The package family's name and repository | WP-HLG-8, with the owner |

## Rules for every package

- **One package, one branch, one worktree** under `~/source/worktrees/quill/<topic>`, from `origin/main`. `~/source/experiments/quill16` is used only for `git fetch` and `git worktree add` and `remove`.
- **Processes.** Browsers headless; Wine, Xvfb, DOSBox-X and QEMU on a private display (`env -u WAYLAND_DISPLAY DISPLAY=:<n> …` in every command, a display number nobody else uses); servers on `127.0.0.1` with a PID file, never on the owner's ports 8095 to 8098; every PID recorded and stopped by that PID; never `pkill` or `killall` by name; never the host desktop.
- **Disk.** Keep `df -h /home` above 20 GB (27 GB were free when this plan was written); a Linux image goes under `~/.cache/quill-host-link-guests/`; `/tmp` is RAM, so check free memory before writing over 1 GB there.
- **Large files outside git**: images, ISOs and states under `~/.cache`; never `git add -A`.
- **No publishing.** No repository creation, release, tag, package publish, upload or Pages dispatch; push only the package's branch.
- **Launch first.** Until WP-WEB-8 has merged, no package here edits `site/try-reactos/host-link.js`, `try-reactos.js`, `index.html`, `try-reactos.css`, `qagrr/**` or `win9x/hostlink/**` except WP-HOST-8's marked block; the page package's files are edited additively, with tests, and rebased onto whatever WP-HOST-5 to WP-HOST-7 merge first.
- **Evidence** into `~/Documents/Liminal HQ/Quill/Web demo/host-link-guests/<package>/`; the few shots that document a feature are committed under `docs/media/` as `web-hlg-*.png`.
- **Gates.** `bash scripts/check-headers.sh`, markdownlint in Docker, `make validate` when `crates/**` changes, the page package's `bun test`, type check and `check:built`, `win9x/build.sh` and its import checks when `win9x/**` changes, `win16/build.sh` and the packer's budgets when `win16/**` changes, and the package's own checks.

## Order and parallelism

| # | Package | Starts when | Runs beside |
|---|---|---|---|
| 0 | Pull requests 200 and 204 merge; WP-HOST-5, WP-HOST-6, WP-HOST-7, WP-WEB-8 and the launch checklist keep their order | — | — |
| 1 | **WP-HLG-1** Spike: a Linux guest in v86 (one day) | Now: it touches only a spike folder | Any launch package |
| 2 | **WP-HLG-2** Conformance sessions, the `Transport` seam, `GuestClipboard`, the guest formats and the streaming lane | Pull requests 200 and 204 merged | WP-HOST-5 or WP-HOST-6 (rebases on them) |
| 3 | **WP-HOST-8** Print... beside Download | WP-HOST-5 merged | WP-HOST-6, WP-HOST-7 |
| 4 | **WP-HLG-3** `hostlinkd`, the Linux helper | WP-HLG-1's findings and WP-HLG-2 merged | WP-HLG-4 |
| 5 | **WP-HLG-4** The Linux guest image | WP-HLG-1's findings | WP-HLG-3 |
| 6 | **WP-HLG-5** Terminal guests: the xterm.js console and OSC 52 | WP-HLG-2 merged | WP-HLG-3, WP-HLG-4 |
| 7 | **WP-HLG-6** The Windows 3.x helper | WP-HLG-2 merged; after the launch | WP-HLG-7 |
| 8 | **WP-HLG-7** Print jobs from every guest: `intent=print` from Quill, the spool folder, text jobs, LPT capture, Linux `lpr` | WP-HOST-8 and WP-HLG-3 merged; WP-DOSWEB-2's text PDF merged | WP-HLG-6 |
| 9 | **WP-HLG-8** Extraction into `v86-host-link` (prepared here; the owner creates the repository) | WP-HLG-3 merged; the AC'97 reuse plan's family decision | — |
| 10 | **WP-HLG-9** The live demo page: Linux and ReactOS sharing one clipboard | WP-HLG-3, WP-HLG-4 and WP-HLG-8 merged; the owner's go | — |

**With the cap of three builders**, the launch packages come first: WP-HLG-1 can take a free slot for its day now, because it touches nothing the launch touches; WP-HLG-2 and WP-HOST-8 take a slot only when WP-HOST-5 has merged; everything from WP-HLG-3 on waits for the launch unless the owner says otherwise.

## Work packages

### WP-HLG-1 Spike: a Linux guest in v86

- **Finish line.** A report section in this plan ("WP-HLG-1 report", dated) answering, each with numbers or a shot: (1) the spike image boots in v86 0.5.473 in headless Chromium with a second UART (`uart1: true`) and, separately, `virtio_console: true`, and how long to a shell and to X; (2) a static i686 probe (Rust, `std`, musl) reads and writes `/dev/ttyS1` raw at 115,200 with `poll`: 1 MiB each way with the host link's one-packet-in-flight pacing, the rates, and whether the guest goes deaf when the page sends before the guest has opened the port (the ReactOS failure); (3) the same through `/dev/hvc0` (if the image's kernel has `virtio_console`), with the rates; (4) X11: the probe sees each `CLIPBOARD` owner change through XFixes (once per copy, in `xterm` and one other program), reads `TARGETS`, `UTF8_STRING` and, where offered, `text/html` and `image/png`, and owns the selection so another program pastes what it set; (5) the 9p share: `mount -t 9p host9p` in the guest and a file the page made with `create_file` read back, and one the guest wrote read with `read_file`; (6) which UART the image's kernel console and `getty` use, and what the probe must avoid; (7) the image's licence and where its source is, and what the published image should be built from (Alpine's recipe or Buildroot), with sizes.
- **Files owned.** New `site/try-reactos/spikes/host-link-linux/` (the probe, a copy of a minimal page and its driver; removed by WP-HLG-3), the report section of this plan, evidence.
- **Must not touch.** Everything else; the image is fetched into `~/.cache/quill-host-link-guests/` and never committed.
- **Dependencies.** None.
- **Budgets.** One working day; the image at most 64 MB on disk.
- **Tests.** The questions above, in v86 in headless Chromium, the page served by the spike's own server on `127.0.0.1`.
- **Evidence.** Logs, timings and shots into `~/Documents/Liminal HQ/Quill/Web demo/host-link-guests/WP-HLG-1/`.
- **Parallel.** With any launch package.

### WP-HLG-2 Conformance sessions, transports and the guest-neutral seams

- **Finish line.** `tests/host-link/sessions/` holds at least 20 scripted sessions (as [Testing](#testing) level 2 says) as JSON, written by a generator from the specification as the vectors are; `tools/hostlink-conformance` (Python, standard library only) replays them against a helper over a pty or a Unix socket and reports pass or fail per session; `HOSTLINK.EXE` passes them under Wine; the page package's `attach` takes a `Transport` (`serial(n)` the default, `virtioConsole(n)`), tested with a fake bus; `clip/state.ts` talks to the guest through `GuestClipboard`, with the host link's implementation and a fake; `html` and `png` are accepted when `hello` lists them (page and `quill-hostlink-guest`), with vectors; `quill-hostlink-guest` has a streaming lane (each payload to a sink) beside the buffered one, tested; `docs/host-link.md` gains the formats and the print keys as additive rows; SITE-83 and SITE-84.
- **Files owned.** `tests/host-link/sessions/**`, `tests/host-link/gen_sessions.py`, `tools/hostlink-conformance/**`, the page package's `src/transport.ts`, `src/index.ts` (the `attach` options), `src/clip/guest.ts`, the matching lines in `src/clip/state.ts`, their tests, the built module, `crates/quill-hostlink-guest/**` (the lane and the formats), `crates/quill-hostlink/**` (format names only, if the codec lists them), `docs/host-link.md` (additive rows and the *Guests* section's links), `docs/decisions.md` (SITE-83, SITE-84).
- **Must not touch.** `site/try-reactos/host-link.js` beyond the `attach` call's options, `win9x/**` (a helper bug goes to a small fix branch), the converters, `qagrr/**`.
- **Dependencies.** Pull requests 200 and 204 merged.
- **Budgets.** The page package stays under 40 KB minified; the crate stays `no_std`, no `alloc`, no panics.
- **Tests.** Level 1 and level 2 against `HOSTLINK.EXE` in Wine; `make validate`.
- **Evidence.** The sessions' pass table; the package's size before and after.
- **Parallel.** With WP-HOST-5 or WP-HOST-6.

### WP-HOST-8 Print... beside Download

- **Finish line.** Every PDF in the *From the PC* list has a **Print...** button that prints it through the browser's dialog from a hidden same-origin frame, falls back to a new tab where the frame cannot print, and never prints without a click; a received file with `intent=print` is listed as *Ready to print* with Print... first; tests with a stubbed `print` in headless Chromium and Firefox; SITE-88.
- **Files owned.** The page package's `src/print.ts` and its tests, the print part of `site/try-reactos/host-link.js` (a marked block), the list's buttons in `index.html` and `try-reactos.css` (a marked block), `harness/check-print.ts`, `site/try-reactos/README.md` (*Printing*), `docs/decisions.md` (SITE-88).
- **Must not touch.** `files.ts` beyond reading its list, `win9x/**`, `qagrr/**`, `src/clip/**`.
- **Dependencies.** WP-HOST-5 merged.
- **Budgets.** At most 2 KB minified added to the package.
- **Tests.** Level 1 and level 4 for printing.
- **Evidence.** The list with the button; the stub's record of one call per click.
- **Parallel.** With WP-HOST-6 and WP-HOST-7.

### WP-HLG-3 `hostlinkd`, the Linux helper

- **Finish line.** `hostlinkd` builds as a static i686 musl binary within its budget, passes every conformance session its `caps` cover on a pty in CI, and in v86 with the spike image (or WP-HLG-4's) shares `CLIPBOARD` text, HTML and PNG both ways with the page, on the transport WP-HLG-1 chose; its `--print FILE` sends a job; SITE-85.
- **Files owned.** New `guests/linux/hostlinkd/**` (its own Cargo workspace, `rust-toolchain.toml` pinned as the repository's, a `build.sh`), the CI job that builds it and runs the sessions, `site/try-reactos/spikes/host-link-linux/` (removed), `docs/decisions.md` (SITE-85).
- **Must not touch.** `crates/**` beyond reading (a core change goes to a small fix branch with its test), the page, `win9x/**`.
- **Dependencies.** WP-HLG-1's report, WP-HLG-2 merged.
- **Budgets.** As [Budgets](#budgets).
- **Tests.** Levels 1 to 3.
- **Evidence.** The session table, the binary's size, a copy and a paste in the guest with the host clipboard read back.
- **Parallel.** With WP-HLG-4.

### WP-HLG-4 The Linux guest image

- **Finish line.** A script (`guests/linux/image/build.sh`, Docker or Podman) builds the demo image from WP-HLG-1's choice, with X11, a window manager, a terminal, a text editor and `hostlinkd` started at log-in, as a 9p `basefs` and a kernel the page boots; `make legal-info` or the Alpine package list records every licence and the source location; the first screen within the budget; SITE-86. Nothing is uploaded.
- **Files owned.** New `guests/linux/image/**`, `docs/decisions.md` (SITE-86), its README.
- **Must not touch.** The ReactOS page and its images.
- **Dependencies.** WP-HLG-1's report.
- **Budgets.** As [Budgets](#budgets).
- **Tests.** The image boots in headless Chromium to the desktop; the licence list is complete (a check that every package has a licence line).
- **Evidence.** The boot timings, the size, a shot of the desktop.
- **Parallel.** With WP-HLG-3.

### WP-HLG-5 Terminal guests

- **Finish line.** A page module that runs a v86 guest with `serial_console` in xterm.js (vendored with its `PROVENANCE.md`, MIT), honours OSC 52 writes inside the gesture window, refuses OSC 52 reads, and pastes through the terminal; tests for the escape parser and the refusals.
- **Files owned.** New `site/js/terminal-clipboard.js` (or the package's `src/terminal.ts`), the vendored xterm.js under `site/vendor/xterm/`, a lab page under `site/terminal-lab/`.
- **Must not touch.** The ReactOS and DOS pages.
- **Dependencies.** WP-HLG-2 merged.
- **Tests.** Level 1; level 4 with the buildroot image from WP-HLG-1.
- **Parallel.** With WP-HLG-3 and WP-HLG-4.

### WP-HLG-6 The Windows 3.x helper

- **Finish line.** `HOSTLNK.EXE` (rust-win16) within its segment budgets, using the streaming lane, passes the conformance sessions through DOSBox-X's null modem with the owner's Windows 3.11 tree, and shares `CF_TEXT` and `Rich Text Format` with the page; SITE-87.
- **Files owned.** New `win16/hostlink/**` (or the path the Win16 workspace's layout calls for), its segment budget file, `docs/decisions.md` (SITE-87).
- **Must not touch.** `win16/src/**` (Quill's), the packer.
- **Dependencies.** WP-HLG-2 merged; after the launch.
- **Tests.** Level 2 over DOSBox-X's serial; level 3 in DOSBox-X.
- **Parallel.** With WP-HLG-7.

### WP-HLG-7 Print jobs from every guest

- **Finish line.** Quill's *Print to your computer* sends its PDF with `intent=print` (an INI key in the Win32 shell's hand-off); `HOSTLINK.EXE` watches `C:\Printed\To your computer\` and sends any PDF saved there; text jobs (a `.PRN` or `.TXT`, the Generic / Text Only output) become a PDF in the page with the DOS bridge's converter; the page collects LPT1 bytes into a job after 2 s of quiet (pulsing ACK for drivers that wait for it); `hostlinkd`'s `lpr` shim; tests for each.
- **Files owned.** The page package's `src/print.ts` (text jobs, LPT capture), `win9x/hostlink/src/spool.rs` and its dispatch line, the hand-off lines in `win9x/quill-win9x/src/main.rs` (the key only), `guests/linux/hostlinkd/src/print.rs`, the shim.
- **Must not touch.** The converters' cores (shared with WP-DOSWEB-2: fixes go there), `quill-pdf`.
- **Dependencies.** WP-HOST-8 and WP-HLG-3 merged; WP-DOSWEB-2's text PDF merged.
- **Tests.** Levels 1 to 4 for each source.
- **Parallel.** With WP-HLG-6.

### WP-HLG-8 Extraction into `v86-host-link`

- **Finish line.** `packages/v86-host-link/` in this repository is the new repository's tree, complete: the specification, vectors, sessions, the TypeScript library, the two crates renamed, `helpers/win32` and `helpers/linux`, the README, licence files, headers, CI workflow, the hero banner's SVG master and its render script; Quill's page and helper build from it; a short note for the owner says what to create and push. SITE-89.
- **Files owned.** New `packages/v86-host-link/**`, the paths that move into it, the build lines that point at it.
- **Must not touch.** Anything outside the host link.
- **Dependencies.** WP-HLG-3 merged; the AC'97 reuse plan's family decision.
- **Tests.** Every existing gate, run from the new layout.
- **Parallel.** None.

### WP-HLG-9 The live demo page

- **Finish line.** A page in the new repository's tree that cold-boots the Linux image and the ReactOS remix (one at a time, or both on a large screen), one clipboard chip shared by both; a copy in one pastes into the other through the host; level 4 checks; the owner's look.
- **Dependencies.** WP-HLG-3, WP-HLG-4 and WP-HLG-8 merged; the owner's go. Publishing is the owner's.

## Builder briefs

The common preamble is the host integration plan's ([Builder briefs](host-integration.md#common-preamble)), with "the host link of the ReactOS web demo" read as "the host link's guests", this plan in place of that one, and the evidence folder `~/Documents/Liminal HQ/Quill/Web demo/host-link-guests/<package>/`. Paste the preamble, then the package's part.

### WP-HLG-1 Spike: a Linux guest in v86 (full brief)

```text
Package WP-HLG-1, Spike: a Linux guest in v86. Branch spike/host-link-linux, worktree
~/source/worktrees/quill/host-link-linux-spike. Time box: one working day; the answers are the
deliverable.

Read first: docs/plans/host-link-guests.md in full ("Guest families", "Transports" and this package are
what your answers decide); docs/host-link.md (the protocol, "Credits" and "Before the helper is up");
docs/plans/host-integration.md "WP-HOST-1 report" on main or branch spike/host-link (how the ReactOS spike
measured the serial line: copy its method and its one-packet pacing); crates/quill-hostlink-guest (the
Port and Clipboard traits the probe will later become); site/try-reactos/v86/v86.d.ts (uart1,
virtio_console, filesystem, create_file, read_file, serial_send_bytes, the serial1 and virtio-console0
events) and site/try-reactos/v86/PROVENANCE.md (fetch v86 with tools/fetch-v86.sh into your worktree).

Fetch, into ~/.cache/quill-host-link-guests/ (check df -h /home first; stop under 20 GB):
https://i.copy.sh/TinyCore-11.0.iso (19,922,944 bytes, X11) and https://i.copy.sh/buildroot-bzimage68.bin
(10,068,480 bytes, terminal only). Record their SHA-256. They are for this spike only: never commit,
upload or publish them. If TinyCore cannot run X in v86 within 3 minutes, try v86's Alpine recipe
(tools/docker/alpine in the v86 repository at a fixed commit, built with podman or docker into the same
cache folder, adding xorg-server, xf86-video-fbdev, xterm and a small window manager) and say which you
used.

Build, under site/try-reactos/spikes/host-link-linux/ (a throwaway; WP-HLG-3 removes it):

1. hlprobe, a Rust std program built static for i686-unknown-linux-musl (rustup target add; say if the
   pinned toolchain lacks it and what you used), getting into the guest by a second disk, a 9p share or
   typed base64, your choice (say which). Modes: `tty DEV` (open raw with cfmakeraw, 115200, VMIN 0,
   VTIME 0, poll; echo every byte back with a running count, log the rate every second to stderr),
   `sink DEV` (count only, never write), `clip` (connect to $DISPLAY, XFixesSelectSelectionInput on
   CLIPBOARD, log one line per owner change with the time, the owner window and TARGETS, and the first 40
   bytes of UTF8_STRING, text/html and image/png when offered), `own TEXT` (take CLIPBOARD and answer
   SelectionRequest for TARGETS and UTF8_STRING until another program takes it). Use x11rb with its
   RustConnection, or the raw protocol; note the stripped size of each choice.
2. A minimal page (a copy of nothing from the product page: a new index.html and driver.mjs) that boots
   the image in v86 with uart1: true (and, in a second run, virtio_console: true), served by your own
   server on 127.0.0.1 on a free port (not 8095 to 8098) with a PID file, driven by playwright-core from
   headless Chromium (/usr/bin/chromium). Guest input only through the emulator's keyboard API; pixels only
   from the v86 canvas or Playwright's page screenshot.
3. Measure, and answer in the report: (1) boot to a shell and to X, in seconds, with each device option;
   (2) through /dev/ttyS1 (the page on serial 1): 1 MiB page to guest at one 4,096-byte packet in flight
   (wait for the echo count to pass each packet), 1 MiB guest to page, and 1 MiB at two and at four
   packets in flight, five runs each, mismatches counted; and the cold-boot case: the page sends 300 bytes
   before hlprobe opens the port, then hlprobe opens it: does it read them, and does the line keep
   working; (3) the same through /dev/hvc0 if present (virtio-console0 events on the page); (4) clip and
   own with xterm and one other X program: changes per copy, the targets offered, a paste of what own set;
   (5) 9p: an empty filesystem: {} share, mount -t 9p host9p /mnt in the guest, a page create_file read in
   the guest and a guest file read with read_file; (6) which tty the kernel console and getty use in the
   image, from /proc/cmdline and /etc/inittab; (7) the image's licences and source location, and what the
   published image should be built from, with sizes.

Must not touch: everything outside site/try-reactos/spikes/host-link-linux/ and the report section of
docs/plans/host-link-guests.md. No image, ISO or state in git.

Finish line: the section "WP-HLG-1 report (date)" in docs/plans/host-link-guests.md under "What was
checked", one row per question with the numbers and a GO or NO-GO, plus what WP-HLG-3 and WP-HLG-4 must
do differently from the plan because of them. Evidence (logs, timings, shots, the probe's source) into
~/Documents/Liminal HQ/Quill/Web demo/host-link-guests/WP-HLG-1/. Stop every process by its own PID and
delete the cached images only if the report says the next package does not need them. PR title
suggestion: "Spike a Linux guest for the host link in v86" (a findings PR; labels documentation,
architecture; do not merge).
```

### WP-HLG-2 to WP-HLG-9 and WP-HOST-8

```text
Package <id>, <title from the plan>. Branch <feat/host-link-sessions | feat/page-print-button |
feat/hostlinkd | feat/linux-guest-image | feat/terminal-clipboard | feat/hostlink-win16 |
feat/print-jobs | feat/v86-host-link-package | feat/host-link-demo>, worktree
~/source/worktrees/quill/<same topic>.

Read first: docs/plans/host-link-guests.md (your package, "The layering", the section your package
serves, "Privacy and security", "Rules for every package"), docs/host-link.md and the vectors as merged,
the WP-HLG-1 report once it exists, and the files your package owns.

Build what the plan's Finish line says, in small commits, staying inside Files owned. Finish line, tests,
budgets, evidence and decision numbers: the plan's package section. Until WP-WEB-8 has merged, touch the
ReactOS page only where your package says, in marked blocks, and rebase onto the launch packages rather
than ask them to wait. PR title suggestions: "Hold every host link helper to the same sessions",
"Print a PDF from the page's list", "Share the clipboard with a Linux guest", "Build a Linux guest
image for the host link", "Copy from a terminal guest with OSC 52", "Share the clipboard with Windows
3.x", "Print from every guest through the host link", "Prepare the v86-host-link package", "Show two
guests sharing one clipboard".
```

## Owner decisions

Each is taken with the default so no package waits; the last column is where to change it.

| Question | Default taken | Where to swap |
|---|---|---|
| Which guest comes after ReactOS | **Linux under X11**, then terminal guests, then Windows 3.x | This plan's order table |
| The Linux helper's transport | `ttyS1` unless WP-HLG-1 shows virtio-console is sound and faster | WP-HLG-3; SITE-85; the page's `Transport` option |
| The Linux image for publishing | Built here from v86's Alpine recipe with X11 added; Buildroot if Alpine boots too slowly | WP-HLG-4; SITE-86 |
| X11 `PRIMARY` (middle-click selection) shared too | Off: `CLIPBOARD` only, as Windows has one clipboard | `hostlinkd`'s option; SITE-85 |
| Print jobs on the `file` channel with `intent=print`, or a new `print` channel | The `file` channel with keys: no codec change | SITE-82; `docs/host-link.md`'s print rows (WP-HLG-2) |
| Print... in the first demo | Yes if WP-HOST-5 merges in time; page-only | WP-HOST-8; the launch checklist |
| PostScript and PCL rendering in the page | No (Ghostscript for WebAssembly is 15.6 to 16.3 MB and AGPL-3.0); offered as a download | SITE-82 |
| Raw printing to a real USB printer (WebUSB) | Later, optional, Chromium only, ESC/P text jobs only | SITE-82; a later package |
| An emulated USB printer | After the USB controller of the AC'97 reuse plan's family exists | That plan; WP-HLG-7's successor |
| The package name and repository | `v86-host-link`, beside `v86-ac97`; one family repository if the AC'97 plan prefers it | SITE-89; WP-HLG-8 |
| Who creates the repository and publishes | The owner; builders prepare the tree in `packages/` | WP-HLG-8 |
| SITE-51 and SITE-52 used twice (the sound package on `main`, the helper on pull request 200) | Not renumbered here; flagged | The owner, before pull request 200 merges |
| A Windows 3.x guest image | None shipped (Microsoft's licence); the owner's own tree in DOSBox-X for tests | WP-HLG-6 |

## Risks

- **Linux in v86 may be slow.** X11 on a frame buffer at about 5 MIPS idle may take minutes to boot cold; a saved state (as the ReactOS demo uses) is the remedy, and WP-HLG-1 measures it.
- **The serial line's quirks may differ.** One packet in flight was measured on ReactOS's `serial.sys`; Linux's 8250 driver may tolerate more, or show a quirk of its own. The conformance sessions and WP-HLG-1's runs are where it shows.
- **The family naming** depends on a plan not yet written; reconciling it later costs a rename in WP-HLG-8.
- **Licences.** A published Linux image carries GPL obligations; xterm.js and `x11rb` are MIT or Apache-2.0; Ghostscript is AGPL-3.0, which is why it is not used.
- **Printing from a frame** behaves differently across browsers; the new-tab fallback covers what the frame does not.
- **WebUSB and printer drivers.** The operating system usually owns a USB printer, so WebUSB printing may fail for most visitors; it stays optional.
- **Scope.** Every guest family is a demo nobody has asked for yet except Linux by implication; the plan keeps them after the launch and behind the owner's go.

## What was not verified

- Any Linux guest in v86 (boot, the 8250 driver's behaviour under the link's pacing, virtio-console, 9p), the X11 selection calls from a static musl program, and the sizes of a Linux demo image: WP-HLG-1.
- Windows 3.1's `EnableCommNotification` and the clipboard viewer chain from a rust-win16 program, and DOSBox-X's null modem with the link's pacing: WP-HLG-6.
- `HOSTLINK.EXE` on Windows 95, 98, Me, 2000 or XP.
- Printing a PDF from a hidden frame in Chromium, Firefox and Safari, and the new-tab fallback; whether ReactOS 0.4.16 has a Generic / Text Only driver; whether DOS and Windows 3.x print to v86's LPT1 without the ACK pulse.
- WebUSB claiming a real printer on Linux, Windows or macOS with the system driver loaded.
- KolibriOS, 9front and Haiku as host link guests (read about, not run).
- The AC'97 reuse plan's naming, which had not been pushed.
