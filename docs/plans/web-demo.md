# The web demo: Quill on ReactOS in the browser

A plan for the first (and perhaps only) public version of `site/try-reactos/`: a clean address, a PDF printer inside the guest with the PDFs downloadable from the page, sound, and somewhere to host about 66 MB of disk parts and saved desktop. Written on 2026-11-27 against `main` at `dd8efd2`, v86 0.5.473, the QAGRR build in `~/.cache/quill-reactos-liminal-work/dist` (reactos-liminal `77b9a83`) and the owner's morning QA of the same day. Where this plan and [decisions.md](../decisions.md) disagree later, the decisions win.

## Table of contents

- [Recommendation](#recommendation)
- [What was measured](#what-was-measured)
- [Goal 1: a clean address](#goal-1-a-clean-address)
- [Goal 2: a PDF printer and downloads](#goal-2-a-pdf-printer-and-downloads)
- [Goal 3: sound](#goal-3-sound)
- [Goal 4: hosting](#goal-4-hosting)
- [The wallpaper name and the savers](#the-wallpaper-name-and-the-savers)
- [Rules for every package](#rules-for-every-package)
- [Order and parallelism](#order-and-parallelism)
- [Work packages](#work-packages)
- [What the demo needs from reactos-liminal](#what-the-demo-needs-from-reactos-liminal)
- [Builder briefs](#builder-briefs)
- [WP-WEB-4 report](#wp-web-4-report)
- [Owner decisions](#owner-decisions)
- [Risks](#risks)
- [What was not verified](#what-was-not-verified)

## Recommendation

| Goal | Recommendation | Why, in one line |
|---|---|---|
| 1. Clean address | `/try-reactos/` reads `web-manifest.json` (fetched uncached) that names content-hashed `data/parts-<hash>/` and `data/state-<hash>.l22.bin.zst`, as SITE-4 does for the DOS page; every panel field still works from the address bar, and the page stops writing `?run=` into the address | The DOS page's scheme already works on Pages, which cannot set cache headers |
| 2. PDF printer | Quill's own **Print to PDF** in the Win32 build: the print layout drawn by GDI into a memory bitmap at 150 dpi, written as an image PDF by a new `no_std` crate, `quill-pdf` | ReactOS 0.4.16 has no printer driver of any kind (measured), so no system printer can produce a page; Quill already draws its pages to any device context |
| 2. Downloads | A **host file channel on COM1**: a one-line header, the bytes, a one-line trailer; the page collects files from v86's `serial0-output-byte` events and lists them with a Download button | COM1 carried 360,448 bytes in 1.2 s, byte for byte, from the existing saved state with no guest change; LPT1 stalled at 16 KiB; ReactOS has no 9p or virtio driver |
| 3. Sound | An **AC'97 device added to v86 from the page** (v86 itself stays unchanged), after a one-day spike; ReactOS's own `ac97.sys` drives it. Sound starts on the Start click, with a sound button and a volume slider | v86 offers only a Sound Blaster 16 and ReactOS has no SB16 driver; ReactOS plays sound through AC'97 (measured in QEMU on the QAGRR disk) |
| 4. Hosting | **GitHub Pages, same origin** (`labs.liminalhq.ca/quill/try-reactos/`); the Pages workflow pulls one pinned, SHA-256-checked data tarball from a Quill release asset and unpacks it into the site. No COOP or COEP | Pages sends `Access-Control-Allow-Origin: *`, ranges and ETags and fits 66 MB easily; release assets cannot be fetched by a page (no CORS on the redirect, measured) |

The build order is WP-WEB-1 (manifest) first, then the channel, the PDF printer and the sound spike side by side, then sound, the wallpaper name, the hosting pipeline, and one final image and state build that brings them together.

**Since written (2026-11-27):** the shared clipboard, files dropped into the guest, time sync and screen size join the downloads on one host link, planned in [host-integration.md](host-integration.md). WP-WEB-2 is built there as WP-HOST-5; a helper in the guest owns COM1, so Quill's Print to PDF hands its file to the helper (`[Web] HostPort=LINK`, W9X-100) and `SENDHOST.CMD` goes through it, with this plan's direct frame as the fallback; WP-WEB-8 also waits for WP-HOST-3 to WP-HOST-7. HTTPS on `labs.liminalhq.ca` now works (owner, 2026-11-27), so the published page is a secure context.

## What was measured

All of it on this machine on 2026-11-27, headless Chromium 156 through `harness/lib.ts`, v86 0.5.473 fetched by `tools/fetch-v86.sh`, the QAGRR parts and state served by `harness/serve.ts`, and QEMU with KVM on a copy-on-write (`snapshot=on`) view of `qagrr.img`. Screenshots are in `~/Documents/Liminal HQ/Quill/Web demo/plan-evidence/`.

### The guest-to-host ports in v86

Commands typed into the Run box of the restored QAGRR desktop; bytes counted from v86's bus events (`serial0-output-byte`, `parallel0-data-output`, `parallel0-control-output`).

| Command | Bytes expected | Bytes received | Time, first to last byte |
|---|---:|---:|---:|
| `cmd /c echo QSERIAL-OK>COM1` | 12 | 12 (`QSERIAL-OK\r\n`) | under 1 ms |
| `cmd /c echo QLPT-OK>LPT1` | 9 | 9, with 18 control writes (the strobe) | 1 ms |
| `cmd /c copy /b C:\Quill\DEMO.QLD COM1` | 4,028 | 4,028, starting `RIFF…QLD1HEAD` | |
| `cmd /c copy /b C:\Quill\DEMO.QLD LPT1` | 4,028 | 4,028, with 8,056 control writes | |
| `cmd /c copy /b C:\ReactOS\notepad.exe COM1` | 360,448 | 360,448 | 1.21 s (about 0.3 MB/s) |
| `cmd /c copy /b C:\ReactOS\notepad.exe LPT1` | 360,448 | **16,384**, then nothing for 15 s | 46 ms for those |

COM1 needs nothing new in the guest: ReactOS's `serial.sys` is on the disk, the UART is in every v86 machine and in the saved state, and the copy is binary-safe. LPT1 stops after 16 KiB because `parport.sys` waits for an acknowledge interrupt that v86's port does not raise by itself (the page could pulse `parallel0-status-input`, but COM1 already works). v86's 9p file system needs a virtio driver that ReactOS does not have.

### The emulated machine

- **PCI devices** in the restored machine: 82441FX, PIIX3 ISA, `ne2k`, `acpi`, `vga`, `ide`. v86 creates a Sound Blaster 16 on ISA in every machine (`this.devices.sb16 = new SB16(…)`), and the parallel port LPT1 at 0x378, IRQ 7. Its speaker adapter (`disable_speaker: false`) gives the PC speaker and the SB16's DAC, through `AudioWorklet` when present and `ScriptProcessorNode` otherwise.
- **No other sound card.** v86 0.5.473's build and the current upstream `src/` hold `sb16.js` and no AC'97, ES1370 or HDA device.
- **A device can be added from the page.** `emulator.v86.cpu.devices.pci.register_device({ pci_id, pci_space, pci_bars, name })`, `cpu.io.register_read`/`register_write`, `cpu.device_raise_irq` and `cpu.read_blob` are all reachable from the page (the probe read the PCI device list this way). This is v86's internal API, so it is pinned with v86.
- **No cross-origin isolation is needed.** The page runs with `serve.ts`'s default (no COOP, no COEP); v86 uses no `SharedArrayBuffer`.

### ReactOS 0.4.16 sound and printing

- **Drivers on the QAGRR disk** (compact trim): `ac97.sys` with `inf\ac97.inf`, `portcls.sys`, `ks.sys`, `drmk.sys`, `kmixer.sys`, `sysaudio.sys`, `wdmaud.sys`/`wdmaud.drv`, `audiosrv.exe`, `beep.sys`, `serial.sys`, `parport.sys`, and the four `Media\ReactOS_*.wav` sounds. The untrimmed golden disk adds only `cmipci.sys`. **There is no Sound Blaster driver**: ReactOS's `drivers/wdm/audio/drivers/sb16` is a four-file stub (15 KB, untouched since the 2017 git conversion) that `CMakeLists.txt` does not build; it builds `ac97` and `CMIDriver` only.
- **AC'97 works.** QEMU, `-device AC97` on a copy-on-write view of the installed QAGRR disk (which was installed without it): the driver installed silently on that boot (no Found New Hardware window), Sound and Audio Devices showed *AC'97 Sound Card* for playback and recording, and playing *ReactOS_Critical_Stop.wav* from the Sounds tab wrote 116,714 frames of 44.1 kHz 16-bit stereo to QEMU's WAV capture (peak 9,019). The *ReactOS Standard* scheme maps Critical Stop to *(None)*, so an error box alone is silent.
- **No printer driver at all.** On the untrimmed golden disk `system32\spool\drivers\w32x86\3` is empty; the spooler (`spoolsv.exe`, `localspl.dll`, `localmon.dll`, `winprint.dll`, `printui.dll`, `ntprint.dll`) is there with nothing to drive. No PostScript, Unidrv or Generic / Text Only driver ships with ReactOS 0.4.16, and Microsoft's are not redistributable.

### Hosting headers

| Where | Result |
|---|---|
| Pages, `http://labs.liminalhq.ca/quill/try/js-dos/emulators/wdosbox.wasm` | `Access-Control-Allow-Origin: *`, `Cache-Control: max-age=600`, an `ETag`, `206 Partial Content` for `Range: bytes=0-99`, `Content-Type: application/wasm`, and gzip on request (1,459,535 bytes sent as 507,677) |
| Pages over HTTPS on the custom domain | The certificate does not name `labs.liminalhq.ca`, so the site is HTTP only today. Over HTTP the page is not a secure context: no `AudioWorklet` (v86 falls back to `ScriptProcessorNode`), no clipboard API |
| `labs.liminalhq.ca/quill/try-reactos/` | Already served (200): the page shell without v86 or data, so Start fails there |
| A GitHub release asset (`github.com/…/releases/download/…`) | `302` to `release-assets.githubusercontent.com` with no `Access-Control-Allow-Origin`, so a page on another origin cannot fetch it |

### Sizes

| What | Size |
|---|---:|
| Disk parts (512 of 8 MiB, `zstd -19`, 466 of them empty) plus `manifest.json` | 51,453,198 bytes in 513 files |
| Saved state, `zstd -22` | about 14.3 MB |
| v86 (`libv86.mjs`, `v86.wasm`, `v86-fallback.wasm`, two BIOS files) | about 4.4 MB |
| One session (restore, menus, dialogs; from the README) | 25.9 MB served, 19 requests |

## Goal 1: a clean address

Today the page boots only when the address names the disk and the state (`?state=…&disk=…`), and it always adds `?run=r…` to the address (`readQuery`), because the measuring server reads the run id from the Referer.

**Options.** (a) Default values written into `index.html`: every rebuild edits the page, and a browser or Pages cache can pair a new page with an old state for ten minutes. (b) A fixed `data/state.bin.zst` name with a `?t=` buster: a CDN may ignore query strings, and a returning visitor can get a state that does not belong to the parts. (c) **A manifest fetched uncached that names content-hashed files**, SITE-4's scheme: the state and the parts it was taken from always arrive together, and nothing named by a hash ever changes.

**Recommended: (c).**

- `site/try-reactos/web-manifest.json`, fetched with `cache: 'no-store'` and `?t=`: `{ "version": 1, "built": …, "parts": "data/parts-<12 hex of the image SHA-256>/", "partsSha256": …, "imageSize": …, "chunk": 8388608, "state": "data/state-<12 hex of its SHA-256>.l22.bin.zst", "stateSha256": …, "stateBytes": …, "audio": { … } or absent, "v86": { "version": "0.5.473", "libv86": "<8 hex>", "wasm": "<8 hex>" }, "quill": "<commit>", "reactosLiminal": "<commit>" }`. The parts folder keeps its own `manifest.json` (v86's chunk size and image size), as now.
- With no query string the page reads the manifest and boots the state over the parts. A field in the address bar still overrides its manifest value, so the builder panel and the harness keep working; `?manifest=URL` names another manifest.
- The page no longer writes `run` into the address. The harness passes `?run=` itself when it wants per-client numbers.
- `libv86.mjs`, `v86.wasm` and the BIOS files load with `?v=` from the manifest, and the page's own `try-reactos.js` and `.css` carry `?v=` from a stamp script (as `scripts/stamp-try.py` does for `try/`). A saved state is tied to one v86 build, so the manifest records the v86 it was taken with and the page refuses a mismatch with a plain message.
- The diagnostics panel stays, closed by default, and shows the manifest's build line (hashes, date, commits) and a *Force fresh copy* link.
- Without a manifest (a local folder with no data) the page says so and opens the panel.

## Goal 2: a PDF printer and downloads

### The printer

| Option | Verdict |
|---|---|
| A system printer: a PostScript or Unidrv driver on `FILE:` or `COM1:`, the spool file converted on the page | **No.** ReactOS 0.4.16 ships no printer driver (measured), Microsoft's are not redistributable, and Wine's `wineps.drv` is not an NT driver. Even with a driver, the page would need a PostScript interpreter (Ghostscript in WebAssembly is about 15 MB and AGPL) |
| **Quill's own Print to PDF, with a text layer** | **Recommended, and built** (W9X-90 to W9X-93; the owner chose it over a 150 dpi raster). `gdi::print_document` already lays out and draws each page on any device context; `quill-pdfdoc` does the same walk into `quill-pdf`, which writes the words as selectable text in the standard 14 fonts, each character at the layout's position, with pictures as images |
| A raster PDF (the page drawn into a DIB at 150 dpi and wrapped as an image) | No. 6.3 MB a page to draw, no text to select or search; the owner asked for a text layer from the start |
| The page makes the PDF: the guest sends the `.QLD`, the page converts it with Quill's core in WebAssembly | Later. Needs P7b and a wasm build; attractive once both exist, because it would work for the Windows 3.x build too |

**Recommended: File > Print to PDF… in `QUILL32.EXE`** (built: W9X-90 to W9X-94). Its own menu item, beside Print, so it works on a ReactOS that has no printer. It asks for a file name (a Save As dialog with the PDF type, default `C:\Printed\<document>.pdf`, the folder made if missing), lays the document out with the screen's fonts (at 96 dpi by default, so the lines break as the window breaks them; `[Print] PdfDpi` changes it) and hands the laid-out lines to `quill-pdfdoc`, which gives `quill-pdf` text runs, rules and pictures. `quill-pdf` is a `no_std` + `alloc` crate: a PDF 1.4 writer whose pages hold selectable text in the standard 14 fonts (Helvetica, Times, Courier, chosen from the face), each character placed at the layout's own position, pictures as Flate-compressed `/Image` XObjects, the page size from the document's paper, the title, `/Producer (Quill)` and a creation date only when asked for (so output is repeatable), and its own deflate encoder (fixed Huffman codes and a hash-chain LZ77 matcher), so no crate is added. A text page is a few KB. The Win16 build is out of scope. Fonts are not embedded (W9X-92 lists the limits and the later step).

### The channel to the page

| Option | Verdict |
|---|---|
| **COM1, framed** | **Recommended.** Works now, byte for byte, about 0.3 MB/s |
| LPT1 | Stalls after 16 KiB without an acknowledge pulse from the page |
| v86's 9p file system | ReactOS has no virtio-9p driver |
| Reading the guest's disk from the page (parse FAT32 in the written blocks) | No guest change, but the guest's cache may not have written the file or its directory entry yet; a FAT reader is more code than the channel |
| The network (`ne2k` with v86's fetch relay and an HTTP upload) | ReactOS's network stack, a client in the guest and a trap in the relay; far more moving parts |

**The frame** (ASCII lines, then raw bytes; anything outside a frame is ignored, so COM1 stays usable for other output):

```text
LHQ-FILE 1 <size in bytes> <name>\r\n
<exactly size bytes>
LHQ-END [<SHA-256 in hex>]\r\n
```

`<name>` is the file's own name (the page keeps letters, digits, space, `.`, `-`, `_`, `(`, `)` and replaces the rest). The page checks the length and, when present, the SHA-256 (WebCrypto), and marks a file whose trailer is missing or whose sum differs as damaged. Received files are kept in memory only, listed under the screen (*From the PC*: name, size, time, **Download**), and the first one of a session also shows a short status line. The page never starts a download by itself: browsers ask before a page downloads several files without a click.

**Senders in the guest.**

- Quill, after Print to PDF, when `QUILL.INI` has `[Web] HostPort=COM1` (only the demo disk sets it): opens `COM1`, writes the frame with the SHA-256, closes it, and says *Sent to the page* in the status bar. Quill itself has no idea of a browser; it writes a file to a port named in its settings.
- `C:\Quill\SENDHOST.CMD`: `echo LHQ-FILE 1 %~z1 %~nx1>COM1`, `copy /b "%~1" COM1 >nul`, `echo LHQ-END>COM1`. No checksum (a batch file cannot make one), so the page checks only the length. Reached by dropping a file on a desktop shortcut, *Send to this computer*, or from the Send To menu once rl can place a shortcut there (see [the dependencies](#what-the-demo-needs-from-reactos-liminal)).

## Goal 3: sound

| Option | Verdict |
|---|---|
| v86's SB16 with a ReactOS SB16 driver | **No driver.** ReactOS's SB16 source is an unbuilt stub; Creative's and Microsoft's drivers are not redistributable; writing a port-class SB16 miniport is a project of its own |
| **AC'97 added to v86 from the page** | **Recommended, after a spike.** ReactOS's `ac97.sys` is on the disk and plays sound (measured in QEMU); v86 stays the pinned npm build; the device is about 500 lines of JavaScript written from Intel's AC'97 and ICH documents (not ported from QEMU, whose `ac97.c` is GPL) |
| AC'97 in a v86 fork, built and vendored | Fallback if the page cannot register the device before the BIOS enumerates PCI or cannot keep its state. Breaks SITE-7's "unchanged from npm" and needs v86's Closure build |
| An ES1370 or CMI8738 device | ReactOS's `cmipci.sys` is trimmed from the compact disk, and AC'97 is the simpler and better-documented controller |
| PC speaker only | Beeps from `beep.sys` come free once the page stops passing `disable_speaker: true`, but no saver or system sound plays |

**How the device works.** The page creates the V86 with `autostart: false`, registers the AC'97 PCI function (8086:2415, one I/O BAR of 256 bytes for the mixer, one of 64 for the bus master, interrupt pin A on a line v86's ACPI routes; the spike settles which) on `emulator-loaded`, then runs. Only the PCM-out channel plays: the device reads the buffer descriptor list from guest memory with `cpu.read_blob`, pulls samples when the audio callback asks for them (as v86's SB16 DAC does), advances the current index, sets the status bits and raises the interrupt on IOC and at the end of the list. The codec registers answer as a plain AC'97 2.3 codec with variable rate (`ac97.sys` then plays 44.1 kHz without resampling in the guest). PCM-in and the modem functions are absent or always empty.

**The saved state.** v86's `save_state` does not know the page's device. The state is built with the device present (the guest installs `ac97.sys` silently on the state build's cold boot, as QEMU showed), saved while no sound plays, and the device's own registers go into the manifest's `audio` object (or a small `state-<hash>.audio.json`), restored on `emulator-loaded` before `run`. A state without that object boots with the device absent.

**On the page.** Sound starts with the Start click (the `AudioContext` is created or resumed inside the click handler, which every browser's autoplay rules accept, Safari's included). A **Sound** button (on or muted) and a volume slider sit with Start, Stop and Full screen; the choice is remembered in `localStorage` (guarded, so a private window still works). Default: on, at 70 %. The output goes through one gain node shared with v86's PC speaker. Stop and Reset suspend the context; a hidden tab keeps playing, as a media page would.

## Goal 4: hosting

| Option | Range requests | CORS | Cache control | Limits | Verdict |
|---|---|---|---|---|---|
| **GitHub Pages, same origin** | Yes (206) | Same origin (and `*` anyway) | `max-age=600` fixed, ETags | 1 GB site, 100 MB a file, about 100 GB a month soft (about 3,800 sessions at 26 MB), 10 builds an hour | **Default.** 66 MB fits; content-hashed names make the fixed `max-age` harmless; the parts need no range requests (v86 fetches whole parts) |
| GitHub release assets, fetched by the page | Yes | **No**: the 302 from `github.com` has no `Access-Control-Allow-Origin` | None useful | 1,000 assets a release | **No** for the page. **Yes** as the store the Pages workflow pulls from |
| jsDelivr from a repository | Yes | Yes | Long | 50 MB a file; the data would have to be committed | No: 66 MB of GPL binaries per rebuild in git history |
| Cloudflare R2 or Pages | Yes | Configurable | `immutable` possible | No egress fees; Pages has 25 MiB a file and 20,000 files | Later, if traffic outgrows Pages. A second origin means CORS rules and a "no request outside its own origin" change |

**Recommended: Pages.** `pages.yml` gains three steps: `tools/fetch-v86.sh`; download one tarball, `qagrr-web-<hash>.tar`, from a Quill release asset named in a committed pin file (`site/try-reactos/web-data.pin`: URL, SHA-256, size), check its SHA-256 and unpack it into `site/try-reactos/data/` and `web-manifest.json`; and a check that every file the manifest names exists with its sum. Nothing is fetched from the release by the page. COOP and COEP are not needed and Pages cannot set them anyway. `data/` is ignored by git.

Publishing the tarball is publishing ReactOS and WineVDM binaries: it needs the corresponding source or a written offer, the licence texts and the list of changes, as `README.md`'s *Licences* section and reactos-liminal's release (`SOURCE.txt`) already lay out. WP-WEB-7 prepares all of it and the page's licences section; **nothing is published until the owner says so**.

## The wallpaper name and the savers

- **Wallpaper name.** Display Properties lists a wallpaper by its file name without the extension, so the remix's `C:\ReactOS\afterglow-wallpaper.bmp` shows as *afterglow-wallpaper*. QAGRR sets `settings.wallpaper = "C:\\ReactOS\\Quill Afterglow.bmp"` and writes its BMP to `ReactOS/Quill Afterglow.bmp` (WP-WEB-6). The picture stays Quill's own (the Afterglow dusk scene with the emblem, SITE-9 and SITE-10); the morning QA's "the Quill one, not the Afterglow wallpaper" is read as being about the name. The same rename in reactos-liminal's compact recipe is a suggestion, not a dependency.
- **The savers** (Quill Ghostwriter, the four Afterglow savers and the Collection) are packed by a separate plan. This plan needs only that they land before the final image (WP-WEB-8), so their sounds are heard through WP-WEB-5's device.

## Rules for every package

- **One package, one branch, one worktree** under `~/source/worktrees/quill/<topic>`, from `origin/main`. `~/source/experiments/quill16` is used only for `git fetch` and `git worktree add` and `remove`.
- **Large files stay outside git**: images, parts, states and tarballs under `~/.cache/quill-reactos-liminal-work/` or `~/.cache/reactos/web/`; `site/try-reactos/data/` is ignored by git. Never `git add -A`.
- **No publishing.** No release, no tag, no upload, no Pages dispatch, no push except the package's own branch.
- **The guest is driven only through the page's API** (`window.tryReactos.input`) or QEMU's own monitor; pixels come from the emulator canvas or QEMU's `screendump`. Never the host desktop. Kill only processes you started, by PID.
- **Shared files.** `site/try-reactos/try-reactos.js` and `index.html` are touched by several packages: WP-WEB-1 splits the script into modules (`manifest.js`, and empty seams for `host-files.js` and `sound.js`), and each later package adds its own module and one marked block in `index.html`. `docs/decisions.md`: each package takes the next free `SITE-` numbers when it opens its pull request. `site/try-reactos/README.md`: each package owns its own section.
- **Image changes** (recipe, programs, guest scripts) are proved on the package's own local build (`qagrr/build-with-rl.sh`, `build-state.sh`, `check-state.sh`); the published build is made once, by WP-WEB-8.
- **Evidence** goes to `~/Documents/Liminal HQ/Quill/Web demo/<package>/`; the few shots that document a feature are committed under `docs/media/` as `web-*.png`.
- **Gates.** `bash scripts/check-headers.sh`, markdownlint in Docker, the `ReactOS demo scripts` tests (`python3 -I -m unittest discover -s site/try-reactos/guest -p 'test_*.py'`, as `ci.yml` runs them), `make validate` when `crates/**` changes, `win9x/build.sh` when `win9x/**` changes, and the package's own harness check.

## Order and parallelism

| # | Package | Starts when | Runs beside |
|---|---|---|---|
| 1 | **WP-WEB-1** Clean address: manifest, hashed data, module split | Now | WP-WEB-4, WP-WEB-6 |
| 2 | **WP-WEB-2** Host file channel (COM1) and the downloads list (built as WP-HOST-5, [host-integration.md](host-integration.md)) | After WP-WEB-1 | WP-WEB-3, WP-WEB-4 |
| 3 | **WP-WEB-3** Print to PDF in `QUILL32.EXE` and `quill-pdf` | Now for the crate and the Win32 command; the send-to-port step after WP-WEB-2's frame is merged | WP-WEB-2, WP-WEB-4 |
| 4 | **WP-WEB-4** Sound spike: an AC'97 device from the page | Now | WP-WEB-1, WP-WEB-3 |
| 5 | **WP-WEB-5** Sound: the device, the controls, the state's audio | WP-WEB-4 says go, and WP-WEB-1 is merged | WP-WEB-3 |
| 6 | **WP-WEB-6** "Quill Afterglow" | Now (small) | Anything |
| 7 | **WP-WEB-7** Hosting pipeline: pin, Pages steps, licences, staging script | After WP-WEB-1 | WP-WEB-5 |
| 8 | **WP-WEB-8** The demo build: one image and state with everything, end to end | WP-WEB-2, 3, 5, 6, 7 merged, WP-HOST-3 to WP-HOST-7 merged, and the savers plan's packing | — |

With the cap of three builders: WP-WEB-1, WP-WEB-3 and WP-WEB-4 start together (WP-WEB-6 is small enough to go to whichever finishes first); then WP-WEB-2, WP-WEB-5 and WP-WEB-7; WP-WEB-8 last. If WP-WEB-4 says no-go, WP-WEB-5 becomes "PC speaker only" (a page change of a few lines) and the owner decides between the v86 fork and no sound.

## Work packages

### WP-WEB-1 Clean address: manifest, hashed data, module split

- **Finish line.** `http://127.0.0.1:8086/try-reactos/` with no query string, served by `harness/serve.ts` from the worktree with a staged `data/`, boots the saved desktop from `web-manifest.json`; the address stays `/try-reactos/` throughout. Every panel field still works from the address bar and overrides the manifest. The page adds nothing to the address. A manifest whose v86 hashes differ from the loaded files stops with a plain message. The diagnostics panel shows the build line and *Force fresh copy*.
- **Files owned.** `site/try-reactos/try-reactos.js` (split into `try-reactos.js`, new `manifest.js`, and empty `host-files.js` and `sound.js` with their exported hooks), `index.html`, `try-reactos.css`, new `tools/stage-web.py` (DIST to `data/parts-<hash>/`, `data/state-<hash>.l22.bin.zst`, `web-manifest.json`; `--check` verifies sums), new `tools/stamp-web.py` (or a `--page try-reactos` mode in `scripts/stamp-try.py`, the builder's choice, recorded), `.gitignore` (`/data/`, `/web-manifest.json`), `harness/lib.ts` and the harness scripts that build URLs (`pageUrl` passes `run` and accepts `manifest`), new `harness/check-clean.ts`, `README.md` (*Run it* and a new *The address and the manifest* section), `.github/workflows/ci.yml` (one test line in the `ReactOS demo scripts` job), `docs/decisions.md` (SITE-14).
- **Must not touch.** `site/try/**`, `scripts/build-try.sh`, `scripts/check-try.sh` (unless the stamp is shared, then only additively), `qagrr/**`, `guest/**`, `crates/**`, `win9x/**`, `.github/**` beyond that one line.
- **Dependencies.** None.
- **Tests.** `tools/stage-web.py` unit tests (names follow content and nothing else; a second run with the same input is identical; `--check` fails on a changed byte); `harness/check-clean.ts`: no query string boots to the themed taskbar and Quill's window (the existing `taskbarScore` and window checks), the address is unchanged after Start, a `?state=` override is honoured, a stale v86 hash refuses; the existing `check-state.sh` still passes through the new URL handling.
- **Evidence.** The restored desktop with the address bar showing `/try-reactos/` (from a Playwright page screenshot of the page, not the host desktop), the panel's build line.
- **Parallel.** With WP-WEB-4 and WP-WEB-6.

### WP-WEB-2 Host file channel (COM1) and the downloads list

- **Moved.** Built as WP-HOST-5 in [host-integration.md](host-integration.md#wp-host-5-files-both-ways-and-print-to-pdf-to-the-list), on the host link, with the finish line below unchanged.
- **Finish line.** A file sent by `SENDHOST.CMD` or by any program writing the frame to COM1 appears under the screen within a second of its last byte, with its name and size, and **Download** saves the same bytes (SHA-256 equal). A frame with a wrong length or sum is listed as damaged. Bytes outside frames are ignored. The disk carries `C:\Quill\SENDHOST.CMD` and a desktop shortcut *Send to this computer* that accepts a dropped file.
- **Files owned.** `site/try-reactos/host-files.js` (the parser as a pure function over bytes, the list, the Download button), its block in `index.html` and `try-reactos.css`, new `qagrr/SENDHOST.CMD`, `qagrr/qagrr.toml` (the `[[files]]` entry and the shortcut), new `harness/check-host-files.ts`, new `site/try-reactos/host-files.test.mjs` (Node's test runner, no dependency), `README.md` (*Files from the PC*), `docs/decisions.md` (SITE-15: the frame and COM1).
- **Must not touch.** `manifest.js`, `sound.js`, `guest/**` (the old chain), `crates/**`, `win9x/**`.
- **Dependencies.** WP-WEB-1 merged.
- **Tests.** The parser: a frame split at every byte boundary, two frames back to back, noise between frames, a short frame followed by a good one, a missing trailer, a wrong sum, a name with path separators and control characters. `check-host-files.ts`: in the restored QAGRR desktop, `cmd /c C:\Quill\SENDHOST.CMD C:\ReactOS\notepad.exe` (through the Run box) gives a listed file of 360,448 bytes whose SHA-256 equals the disk's copy (read with mtools from the image), and a Playwright download of it has the same sum.
- **Evidence.** The list with one good and one damaged file; the timing of the 360 KB send.
- **Parallel.** With WP-WEB-3 and WP-WEB-4.

### WP-WEB-3 Print to PDF in `QUILL32.EXE` and `quill-pdf`

- **Finish line.** File > Print to PDF… in the Win32 build writes a PDF with a real text layer whose pages, rendered by `pdftoppm`, look as the screen shows them (the same lines, the same fonts' positions), with the document's page count and size; `qpdf --check` passes and `pdftotext` returns the words in order. Under Wine and on the QAGRR disk the demo letter makes a one-page PDF. With `[Web] HostPort=COM1` in `QUILL.INI`, Quill also writes the WP-WEB-2 frame with its SHA-256 to that port and says so in the status bar; without it, nothing is sent.
- **Files owned.** New `crates/quill-pdf/` (`no_std` + `alloc`: the writer and its deflate encoder, tests, `Cargo.toml` with the house `[package.metadata.quill]` layer `core`), the root `Cargo.toml` members line and `Cargo.lock` (the new crate only), `win9x/quill-win9x/Cargo.toml`, `src/main.rs` (the command, the menu item, the INI key), `src/gdi.rs` (a `render_pages_to_dib` beside `print_document`, sharing `draw_page`), `src/dialogs.rs` (the PDF save filter), the menu and string tables the Win32 build reads, `docs/decisions.md` (the next `W9X-` or `D` number for the command and the INI key, as the area's prefix rule says), `SPEC.md`'s printing row for Windows 95 (one sentence), `docs/progress.md` (one row).
- **Must not touch.** `win16/**`, `dos/**`, `linux/**`, `winui/**`, `site/**` (the page side is WP-WEB-2's), `crates/quill-core/**` and other shared crates beyond reading them. If the print layout cannot be drawn into a memory DC without a change in `quill-core`, stop and report.
- **Dependencies.** None for the crate and the command; the send-to-port step uses WP-WEB-2's frame (the text above is the contract, so it can be built before WP-WEB-2 merges).
- **Tests.** `quill-pdf`: the deflate encoder round-trips through `quill-inflate` on random and real rows; a PDF of two synthetic pages parses (`qpdf --check` and `pdfinfo` when present on the host, skipped with a message when not), `pdftotext` returns its words and their positions and `pdftoppm` gives a picture's pixels back; the output is byte-repeatable and has a golden. `quill-pdfdoc`: the letter, headers, footers, fields, lists and pictures of the test documents, with a golden. Win32: a Wine run (as `docs/testing.md`'s Wine stage runs the Win32 build) exports `tests/docs/letter.qld` and the demo to PDF and compares page counts and a rendered page with the preview's own bitmap; the size of the demo's PDF is recorded (target under 0.5 MB a page).
- **Evidence.** The Print to PDF dialog on ReactOS (QEMU `screendump`), the PDF's first page rendered beside the screen, the file sizes.
- **Parallel.** With WP-WEB-2 and WP-WEB-4.

### WP-WEB-4 Sound spike: an AC'97 device from the page

- **Finish line.** A yes or no, with numbers, on four questions, each answered in v86 0.5.473 in headless Chromium: (1) Can the page register a PCI function before the BIOS enumerates PCI (`autostart: false`, `emulator-loaded`, `run`) and does SeaBIOS give it I/O BARs and an interrupt line? (2) Does ReactOS, cold-booted from the QAGRR parts, install `ac97.sys` for it without a window? (3) Does playing *ReactOS_Critical_Stop.wav* from the Sounds tab deliver samples to the device at the right rate (count the frames the device took against the WAV's length, and the peak)? (4) Can the device's registers be saved beside a v86 state and restored so that a second sound plays after a restore? Plus the CPU cost (MIPS with and without the device while playing).
- **Files owned.** New `site/try-reactos/spikes/ac97/` (the device as a module, a spike page or a flag in the spike copy of the page, a harness script), and a findings section in `site/try-reactos/README.md`. No change to the published page.
- **Must not touch.** `try-reactos.js`, `index.html`, `manifest.js`, `qagrr/**`, `guest/**`, v86's files.
- **Dependencies.** None. Uses the existing QAGRR parts in `~/.cache/quill-reactos-liminal-work/dist`.
- **Tests.** The device's register model in Node (reset values, codec ready, BDL walking, IOC and LVBCI status bits, the interrupt line) against values from Intel's AC'97 2.3 and ICH documents; the in-browser numbers above.
- **Evidence.** The Sounds tab with *AC'97 Sound Card* in v86, the frame counts, a WAV made from the samples the device took (kept under `~/Documents/Liminal HQ/Quill/Web demo/WP-WEB-4/`).
- **Parallel.** With WP-WEB-1 and WP-WEB-3.

### WP-WEB-5 Sound: the device, the controls, the state's audio

- **Finish line.** On the published layout, after Start, the saved desktop plays a system sound from the Sounds tab and an Afterglow saver's sound (when the savers are packed) through the speakers of a real browser; the Sound button mutes and unmutes, the slider sets the level, the choice survives a reload; Stop and Reset silence it; a browser that refuses audio still runs the machine. Headless, the device's sample counter proves the playback.
- **Files owned.** `site/try-reactos/sound.js`, new `site/try-reactos/ac97.js` (from the spike, hardened), their block in `index.html` and `try-reactos.css`, `harness/build-state.ts` (registering the device for the state build, saving its registers), `tools/stage-web.py` (the `audio` object), new `harness/check-sound.ts`, `README.md` (*Sound*), `site/try-reactos/v86/PROVENANCE.md` (one paragraph on the internal API used), `docs/decisions.md` (SITE-16, SITE-17).
- **Must not touch.** `host-files.js`, `qagrr/**` except a comment, `guest/**`, v86's files.
- **Dependencies.** WP-WEB-4 go; WP-WEB-1 merged.
- **Tests.** The Node register tests from the spike; `check-sound.ts` (restore, open the Sounds tab through the Run box, play, count frames and peak, mute and check the gain node's value, Reset and check silence); `check-state.sh` still passes.
- **Evidence.** The page's controls, the counts, a short WAV of a saver's sound taken from the device.
- **Parallel.** With WP-WEB-3 or WP-WEB-7.

### WP-WEB-6 "Quill Afterglow"

- **Finish line.** Display Properties' background list on the QAGRR disk shows *Quill Afterglow* selected, and the desktop is unchanged pixel for pixel.
- **Files owned.** `site/try-reactos/qagrr/qagrr.toml` (`settings.wallpaper` and the BMP's destination), `qagrr/build-with-rl.sh` (only if it names the path), `README.md` (one sentence), `docs/decisions.md` (SITE-18, or an amendment line under SITE-10).
- **Must not touch.** Everything else.
- **Dependencies.** None; rl writes the registry value with the space escaped as it stands (checked in `reactos_liminal/settings.py`).
- **Tests.** `build-with-rl.sh --dry-run`; a local image; QEMU `screendump` of Display Properties' Desktop tab; `check-state.sh` on a fresh state.
- **Evidence.** The Desktop tab.
- **Parallel.** With anything.

### WP-WEB-7 Hosting pipeline: pin, Pages steps, licences

- **Finish line.** A local run of the new Pages steps (`act` is not needed: the steps are a script, `scripts/fetch-web-data.sh`, the workflow calls) turns a pin file and a local tarball into `site/try-reactos/data/` and `web-manifest.json`, refusing a wrong sum; `scripts/check-site.py` passes over the result; the page boots from it through a plain static server (`python3 -m http.server` under a `quill/` path, as `site/README.md` describes), which proves nothing depends on `serve.ts`. The licences section of the page names ReactOS 0.4.16 (tag and commit), WineVDM, SeaBIOS 1.16.2, v86, the Afterglow commit, Quill and reactos-liminal commits, and the written offer. The workflow change is written but its data step is guarded off until the pin names a real release.
- **Files owned.** New `scripts/fetch-web-data.sh`, new `site/try-reactos/web-data.pin` (a placeholder that the script recognises as "no data yet"), new `qagrr/pack-web.sh` (stage-web output to one tarball and its pin line), `.github/workflows/pages.yml` (`fetch-v86.sh` and the data step), `scripts/check-site.py` (only to know `data/` may be absent), `site/try/licences.html` or a new `site/try-reactos/licences.html` (the builder's choice, recorded), `site/README.md` (the try-reactos paragraph), `docs/decisions.md` (SITE-19).
- **Must not touch.** The page's modules, `qagrr.toml`, `crates/**`, `win9x/**`.
- **Dependencies.** WP-WEB-1 merged (the layout). Must not create a release, a tag or a Pages deployment.
- **Tests.** `fetch-web-data.sh` against a local tarball (good, one byte changed, truncated, a path escaping the folder); the placeholder pin skips cleanly; the static-server boot through `check-clean.ts` with `BASE` pointed at it.
- **Evidence.** The boot from a plain static server; the licences page.
- **Parallel.** With WP-WEB-5.

### WP-WEB-8 The demo build: everything together

- **Finish line.** One QAGRR build from a pinned reactos-liminal commit with the Quill programs of the same `main` commit (Print to PDF and `HostPort=COM1`), `SENDHOST.CMD`, the wallpaper name, the packed savers, and a state built with the AC'97 device; staged, packed and pinned; and an end-to-end run on the plain static server: no query string, the desktop, File > Print to PDF in Quill, the PDF in the list, a download whose SHA-256 equals the guest's file and whose first page matches Quill's preview, a saver's sound counted by the device. The tarball, its pin and the licences are ready for the owner; **nothing is published**.
- **Files owned.** `qagrr/reactos-liminal.pin`, `site/try-reactos/web-data.pin` (the local tarball's sum, marked unpublished), new `harness/check-demo.ts`, `README.md` (*The published build*), `docs/progress.md`, `docs/decisions.md` (only if something changed on the way).
- **Must not touch.** The modules and programs (bugs found go back to their packages as issues or small fixes in their own branches).
- **Dependencies.** WP-WEB-2, 3, 5, 6 and 7 merged; WP-HOST-3 to WP-HOST-7 merged (and the link's checks in the finish line, as [host-integration.md](host-integration.md#wp-web-8-additions) lists); the savers packing merged; the reactos-liminal changes in the next section that the owner chose.
- **Tests.** `check-state.sh`, `check-clean.ts`, `check-host-files.ts`, `check-sound.ts` and the new `check-demo.ts`, throttled to 50 Mbit/s and 10 Mbit/s once each (time to the desktop recorded beside the README's table).
- **Evidence.** A short sequence of canvas shots (desktop, Print to PDF, the list, the PDF's page), the timings, the sizes, into `~/Documents/Liminal HQ/Quill/Web demo/WP-WEB-8/`.
- **Parallel.** None.

## What the demo needs from reactos-liminal

| Need | Hard or soft | Why | Fallback |
|---|---|---|---|
| The compact trim keeps the audio stack: `ac97.sys`, `inf\ac97.inf`, `portcls.sys`, `ks.sys`, `drmk.sys`, `kmixer.sys`, `sysaudio.sys`, `wdmaud.sys`, `wdmaud.drv`, `audiosrv.exe`, `winmm.dll`, `dsound.dll`, `mmsys.cpl`, `sndvol32.exe`, `beep.sys`, the `Media` sounds; and `serial.sys` | **Hard** (all are kept at `77b9a83`; a test in rl that keeps them is the ask) | Sound and the channel | QAGRR's `[[files]]` could put them back, but they are ReactOS's own files and belong with the trim |
| A recipe can add QEMU devices to the install and image runs (`[install] qemu_devices = ["AC97"]` or similar) | Soft | `ac97.sys` bound during the install instead of on the state build's first boot in v86; a quieter, repeatable state build | The v86 cold boot installs it silently (measured in QEMU after an install without the device) |
| A `[[shortcuts]]` entry can go to the Send To folder (`location = "sendto"`) | Soft | *Send to this computer* in every Explorer context menu | A desktop shortcut that accepts a dropped file (WP-WEB-2) |
| The compact recipe's wallpaper file named for people (`Afterglow Wallpaper.bmp`) | Soft | The AGR compact image has the same lowercase name | QAGRR names its own (WP-WEB-6) |
| The release's source pointer and licence list are reusable for a downstream remix | Soft | WP-WEB-7's licences page and the source offer | Quill writes its own from the same facts |
| The pinned commit moves forward once for WP-WEB-8 | Hard | The savers packing and any trim test land there | Stay on `77b9a83` if nothing changed |

## Builder briefs

Each brief is the common preamble followed by the package's own part. Paste both.

### Common preamble

```text
You are a Sonnet builder for Quill (repo liminal-hq-labs/quill), working on the web demo of Quill on
ReactOS (site/try-reactos/). Read AGENTS.md, site/try-reactos/README.md, docs/plans/web-demo.md (your
package, "Rules for every package" and the goal section it belongs to) and the SITE- decisions in
docs/decisions.md.

Where to work: never touch ~/source/experiments/quill16 except `git fetch` and `git worktree add/remove`.
Create your worktree at ~/source/worktrees/quill/<topic> on branch <branch> from origin/main. Remove it
after the PR merges, with plain commands and full literal paths (`git worktree remove <path>`,
`git worktree prune`, `git branch -d <name>`); never globs, variables or `find -delete` in a removal.

Rules: Canadian English, real em dashes, sentence case, no hard-wrapped markdown. SPDX header on every new
source, script, TOML and YAML file. Tabs except YAML. Never `git add -A` or `git commit -a`; stage named
paths. Commits: Conventional Commits, body in markdown with bold labels and no headings, hard-wrapped at
about 100 columns, written to a file with a single-quoted heredoc and committed with `git commit -F`, ending
with `Co-Authored-By: Claude Sonnet <noreply@anthropic.com>` naming your own model. Push only your branch;
open one PR (title capitalized, imperative, no prefix; body `## Summary` with bold lead-in bullets,
`## Test plan` with the commands run, ending with an unchecked "Not verified:" line). Do not merge.

Never publish: no release, no tag, no upload of images or states, no Pages dispatch. Large files (images,
parts, states, tarballs) stay under ~/.cache; site/try-reactos/data/ is ignored by git. Check free memory
before a large write to /tmp (it is RAM).

The guest is driven only through the page's own API (window.tryReactos.input) in headless Chromium
(/usr/bin/chromium through harness/lib.ts) or through QEMU's own monitor socket; pixels come from the
emulator canvas or QEMU's screendump. Never the host desktop, never key injection into it. Kill only
processes you started, by PID (never pkill or killall by name).

Stay inside "Files owned". If you need a change in a must-not-touch path, stop and report what and why. When
a choice is not covered, take the reasonable default, record it as the next free SITE- decision marked
**Assumed** (or the area's own prefix for Win32 work), list it in the PR as "OWNER DECISION: question |
default taken | where to swap", and keep going.

Evidence: every shot into ~/Documents/Liminal HQ/Quill/Web demo/<package>/; commit the few that document the
feature under docs/media/ as web-*.png. Before the PR: bash scripts/check-headers.sh, markdownlint in Docker
(docker run --rm -v "$PWD:/workdir" davidanson/markdownlint-cli2:v0.17.2 "**/*.md"), the ReactOS demo
script tests (python3 -I -m unittest discover -s site/try-reactos/guest -p 'test_*.py'), make validate if you touched crates/**,
win9x/build.sh if you touched win9x/**, and your package's own checks. Update the README section you own and
decisions.md in the same change. Report: PR URL, what was and was not run, the evidence paths, OWNER
DECISION lines, and anything you stopped on.
```

### WP-WEB-1 Clean address (full brief)

```text
Package WP-WEB-1, Clean address: manifest, hashed data, module split. Branch feat/web-demo-manifest,
worktree ~/source/worktrees/quill/web-demo-manifest.

Read first: docs/plans/web-demo.md "Goal 1: a clean address"; decision SITE-4 and scripts/stamp-try.py,
scripts/check-try.sh and site/try/try.js (the DOS page's manifest, `?v=` stamps and Force fresh copy, which
this package mirrors); site/try-reactos/try-reactos.js (readQuery, config, asyncImage, start), index.html,
harness/serve.ts, harness/lib.ts (pageUrl), harness/check-state.ts and qagrr/check-state.sh;
~/.cache/quill-reactos-liminal-work/dist/ (parts/, parts/manifest.json, state.l22.bin.zst, build-manifest.json:
your input; read-only).

Goal: http://127.0.0.1:<port>/try-reactos/ with no query string boots the saved desktop, and the address
never changes.

Build, in this order, one commit each where it makes sense:

1. Module split with no change in behaviour: try-reactos.js keeps the player and the pointer; new
   manifest.js (load and check the manifest, merge it with the address bar's fields); new host-files.js and
   sound.js that export an `attach(emulator, ui)` doing nothing yet (WP-WEB-2 and WP-WEB-5 fill them).
   index.html keeps one <script type="module">. Keep window.tryReactos's shape; the harness depends on it.
2. tools/stage-web.py DIST OUT: copies DIST/parts/* to OUT/data/parts-<first 12 hex of the image SHA-256 from
   parts/manifest.json>/ and DIST/state.l22.bin.zst to OUT/data/state-<first 12 hex of its own SHA-256>.l22.bin.zst,
   and writes OUT/web-manifest.json with the fields in the plan (version 1, built, parts, partsSha256,
   imageSize, chunk, state, stateSha256, stateBytes, v86 {version, libv86, wasm, fallback, seabios, vgabios}
   as the first 8 hex of each file's SHA-256 from site/try-reactos/v86/, quill and reactosLiminal from
   DIST/build-manifest.json when present). Deterministic: sorted keys, no timestamps except `built`, which
   comes from build-manifest.json (or --built). --check OUT verifies every sum and that no other data/
   entries remain. Copy, never move; use hard links when DIST and OUT are on one file system and say so.
   Python 3, standard library only, `python3 -I`. Tests beside it with unittest, as guest/test_*.py are, and a
   line in ci.yml's ReactOS demo scripts job.
3. manifest.js: fetch web-manifest.json (or ?manifest=URL) with cache 'no-store' and ?t=Date.now(); every
   form field present in the address bar wins over the manifest; disk becomes <parts>.img.zst with
   chunk from the manifest; state from the manifest. No manifest and no state in the address: say "No demo
   data on this server" in the status line, open the diagnostics panel, keep Start for a cold boot.
4. Stop writing `run` into the address: readQuery still reads ?run= (the harness passes it) but a page
   without one makes a run id in memory only. Check harness/serve.ts: per-client numbers come from the
   Referer, so the harness must keep passing ?run= (pageUrl already does); say in the README that a
   visitor's page is counted under "no run".
5. v86 and page versions: libv86.mjs, v86.wasm, v86-fallback.wasm and the two BIOS URLs get ?v=<hash> from
   the manifest; a page whose fetched libv86.mjs does not match (hash it with crypto.subtle, once) refuses
   with "This saved desktop needs another emulator build: reload" instead of starting. try-reactos.js and
   .css and the new modules get ?v= stamps written into index.html by a stamp script: extend
   scripts/stamp-try.py with a --page try-reactos mode if that stays small and additive, otherwise
   tools/stamp-web.py; record which (SITE-14) and make --check fail on a stale stamp. Static imports
   between modules carry the same stamp (write them through the stamp or use one versioned import map;
   pick one and record it).
6. The diagnostics panel: a "Build" row (state and parts hashes, built date, Quill and reactos-liminal
   commits, v86 version) and a Force fresh copy link that re-fetches the page's files with cache 'reload'
   and reloads. The panel stays closed by default and keeps every field.
7. .gitignore: /data/ and /web-manifest.json. README: "Run it" now stages data
   (python3 -I site/try-reactos/tools/stage-web.py ~/.cache/quill-reactos-liminal-work/dist site/try-reactos)
   and opens /try-reactos/ with no query; a new "The address and the manifest" section. harness/serve.ts
   needs no change if it serves site/ statically; confirm.
8. harness/check-clean.ts (bun, playwright-core through lib.ts): stage into the worktree (gitignored), serve,
   open /try-reactos/ with only ?run= (the harness's own), Start, wait for taskbarScore > 0.15 and Quill's
   window (reuse check-state.ts's tests), assert the address's path and query are unchanged except the run;
   then ?state=<the same state> as an override; then a manifest with a wrong libv86 hash refuses. Then
   run qagrr/check-state.sh to show nothing else broke.

Must not touch: site/try/**, scripts/build-try.sh, scripts/check-try.sh (except additive tests if stamp-try.py
gains a mode), qagrr/**, guest/**, crates/**, win9x/**, win16/**, .github/** (except the one test line in ci.yml), site/try-reactos/v86/**.

Finish line checks (all must hold):
- check-clean.ts passes: no query boots, address unchanged, override honoured, stale v86 refused.
- stage-web.py's tests pass, and --check passes on a fresh stage and fails after one byte changes.
- The stamp's --check passes, and fails after editing a module without restamping.
- qagrr/check-state.sh still passes.
- The headers check and markdownlint pass.

Evidence: into ~/Documents/Liminal HQ/Quill/Web demo/WP-WEB-1/: a Playwright screenshot of the whole page
(the address shows in the harness log, not the shot; write it into the PR), the restored canvas, the open
panel with the Build row. Commit docs/media/web-clean-address.png (the page with the desktop).

Docs in the same change: README sections above; decisions.md SITE-14 (the manifest, the hashed names, the
override rule, the stamp choice, no run in the address) with alternatives and consequences as the SITE-
rows have them. PR title suggestion: "Boot the ReactOS demo from a manifest at a clean address".
```

### WP-WEB-2 Host file channel

```text
Package WP-WEB-2, Host file channel (COM1) and the downloads list. Branch feat/web-demo-host-files, worktree
~/source/worktrees/quill/web-demo-host-files. Starts after WP-WEB-1 has merged.

Read first: the plan's "Goal 2" (the frame is the contract; do not change it without recording why),
"What was measured" (the port table), site/try-reactos/host-files.js and manifest.js as WP-WEB-1 left
them, qagrr/qagrr.toml ([[files]], [[shortcuts]]), qagrr/build-with-rl.sh, harness/run-in-guest.ts (typing
into the Run box) and harness/lib.ts.

Build: (1) a pure parser, feed(bytes) -> events {file, damaged}, keeping at most one frame's bytes, with
Node tests (node --test) for every case in the plan's Tests line; (2) attach(emulator, ui) listening to
serial0-output-byte, batching per animation frame, and a "From the PC" list under the screen (name, size,
time, Download through a Blob URL revoked after use, Clear), every string in plain Canadian English, the list
announced politely to screen readers; (3) qagrr/SENDHOST.CMD as in the plan (CRLF line endings, quoting for
spaces) and its [[files]] entry to C:\Quill\SENDHOST.CMD; a desktop shortcut "Send to this computer"
(target cmd.exe /c C:\Quill\SENDHOST.CMD, so a dropped file arrives as %1; check that a drop on a shortcut
passes the path in ReactOS 0.4.16, and if it does not, record it and keep the Run box route); (4)
harness/check-host-files.ts on a local build (build-with-rl.sh --build-quill, build-state.sh): send
notepad.exe through the Run box, compare the listed file's SHA-256 and a Playwright download's with the
disk's copy (mtools from the image), and time it.

Must not touch: manifest.js, sound.js, try-reactos.js beyond one import, guest/**, crates/**, win9x/**,
.github/**. Finish line, tests and evidence: the plan's WP-WEB-2. Decision SITE-15 (COM1, the frame, the
in-memory list, no automatic download). PR title suggestion: "Collect files the ReactOS guest sends on COM1".
```

### WP-WEB-3 Print to PDF

```text
Package WP-WEB-3, Print to PDF in QUILL32.EXE and quill-pdf. Branch feat/win32-print-to-pdf, worktree
~/source/worktrees/quill/win32-print-to-pdf.

Read first: the plan's "Goal 2"; win9x/quill-win9x/src/gdi.rs (print_document, PageLayout, draw_page,
GdiMeasure) and src/main.rs (print_job, the File menu, QUILL.INI handling); crates/quill-inflate (style and
lint set for a no_std codec); docs/testing.md (the Wine scenarios) and the W9X- decisions; AGENTS.md on new
crates and layers.

Build (owner change: a text layer from the start, not a raster): (1) crates/quill-pdf, no_std + alloc,
forbid(unsafe_code), the house clippy denies: a deflate encoder and a writer for text runs in the standard 14
fonts, rectangles and images; (2) crates/quill-pdfdoc, which walks a laid-out document as draw_page does;
(3) File > Print to PDF... after Print Setup, Ctrl+Shift+P, a Save As with the PDF type and C:\Printed\<name>.pdf
as the default (create the folder), status bar "Printed to <name>", /PDF OUT on the command line, [Print] PdfDpi;
(4) with [Web] HostPort=COM1 (any COMn), open the port, write the frame with the file's SHA-256, close, status
"Sent to the page": a later step, after WP-WEB-2's frame is merged; (5) Wine: a run of the Win32 build
(win9x/tools/wine-pdf.sh) exports tests/docs/letter.qld and the demo; (6) on a local QAGRR build, QEMU screendump
of the menu and the dialog, and the file pulled out with mtools.

Must not touch: win16/**, dos/**, linux/**, winui/**, site/**, shared crates other than the new quill-pdf
(stop and report if the layout needs a quill-core change). Finish line, tests and evidence: the plan's
WP-WEB-3. Decisions: the command, the dpi, the INI keys, the in-tree encoder (the area's next free numbers).
PR title suggestion: "Print to PDF from the Windows 95 build".
```

### WP-WEB-4 Sound spike

```text
Package WP-WEB-4, Sound spike: an AC'97 device from the page. Branch spike/web-demo-ac97, worktree
~/source/worktrees/quill/web-demo-ac97. Time box: one working day; the answer is the deliverable.

Read first: the plan's "Goal 3" and "What was measured"; site/try-reactos/v86/v86.d.ts; v86's own src/sb16.js,
src/pci.js and src/ne2k.js on GitHub at commit 315b2b0 (read only; do not copy code from QEMU's GPL ac97.c);
Intel's AC'97 2.3 specification and the ICH (82801AA) audio register documents; harness/build-state.ts.

Build under site/try-reactos/spikes/ac97/: the device module (PCI function 8086:2415, mixer and bus-master
BARs, codec reset and ready, PCM-out BDL walking with read_blob, status bits, the interrupt), registered
on emulator-loaded with autostart false; a spike page (a copy of the page that imports it) and a harness
script that cold-boots the QAGRR parts with the device, waits for the desktop, opens the Sounds tab
(rundll32 shell32.dll,Control_RunDLL mmsys.cpl,,1 through the Run box), assigns ReactOS_Critical_Stop.wav to
Critical Stop with the keyboard as the plan's QEMU probe did, plays it, and counts frames and the peak; then
saves the state with the device's registers beside it, restores, and plays again. Answer the four questions
with numbers, plus MIPS with and without the device while playing. If (1) fails, try registering before
construction by wrapping the PCI object, and say which worked.

Must not touch: the page, qagrr/**, guest/**, v86's files. Report go or no-go with the numbers; on no-go,
say what a v86 fork would need. Evidence into ~/Documents/Liminal HQ/Quill/Web demo/WP-WEB-4/. PR title
suggestion: "Spike an AC'97 device for the ReactOS demo" (a findings PR; the spike folder may be removed by
WP-WEB-5).
```

### WP-WEB-5, WP-WEB-6, WP-WEB-7 and WP-WEB-8

```text
Package <id>, <title from the plan>. Branch <feat/web-demo-sound | feat/qagrr-wallpaper-name |
feat/web-demo-hosting | feat/web-demo-build>, worktree ~/source/worktrees/quill/<same topic>.

Read first: the plan's section for your package and the goal it serves, the decisions it names, and the
files it owns. Build what the plan's Finish line says, in small commits, staying inside Files owned.
WP-WEB-5 starts from the WP-WEB-4 findings and moves the device out of spikes/ into ac97.js. WP-WEB-6 is
one recipe change and its proof in QEMU. WP-WEB-7 writes the pipeline but must not create a release, a tag
or a Pages deployment; its data step stays guarded off until the owner pins a real release. WP-WEB-8 builds
and checks, and hands the owner a tarball, its pin line and the licences, unpublished.

Finish line, tests, evidence and decision numbers: the plan's package section. PR title suggestions:
"Play the ReactOS demo's sound through an AC'97 device", "Name the remix's wallpaper Quill Afterglow",
"Prepare the ReactOS demo's data for Pages", "Build the ReactOS web demo end to end".
```

## WP-WEB-4 report

**Verdict: GO.** A page-side AC'97 device (about 500 lines of JavaScript in `site/try-reactos/spikes/ac97/`, v86 0.5.473 unchanged) is driven by ReactOS's own `ac97.sys`: the guest played *ReactOS_Critical_Stop.wav* from the Sounds tab, the page's audio sink received it at the right rate with no glitches, and the device's registers saved beside a v86 state and restored so that sounds played again. Run on 2026-11-27 in headless Chromium (no sound card; the sink is an `AudioWorklet` that counts and records what it plays), cold-booting the QAGRR parts at `~/.cache/quill-reactos-liminal-work/dist`. Evidence (waveforms, captured WAV, logs, screenshots) is in `~/Documents/Liminal HQ/Quill/Web demo/WP-WEB-4/`.

| Question | Answer | Numbers |
|---|---|---|
| 1. Can the page register a PCI function before the BIOS enumerates PCI, and does SeaBIOS give it BARs and an interrupt? | **Yes, by two routes** | `autostart: false` with `emulator-loaded` works for a cold boot. For a saved state, v86 restores the PCI space *before* `emulator-loaded`, so the device must exist earlier: the spike wraps `emulator.v86.init` (a property setter on the instance) and registers right after v86 creates its devices and before `restore_state`. Both routes gave the same result: slot 3 (`pci_id` 24), SeaBIOS moved the BARs from `0xD000`/`0xD400` to `0xC000`/`0xC100`, interrupt line 11 (the PIIX3's PIRQ registers read 10, 10, 11, 11), pin A. No change to v86's files and no need to wrap the PCI object. The I/O handlers must be registered at the BARs' power-on addresses *before* `register_device`, because v86 copies the port entries then and moves them when a BAR moves |
| 2. Does ReactOS install `ac97.sys` for it without a window? | **Yes** | The QAGRR disk was installed without the device. On the first cold boot with it, the driver bound silently (no Found New Hardware window); the first codec access came 21 s after the taskbar appeared. Sound and Audio Devices, Audio tab, shows *AC'97 Sound Card* for playback and recording |
| 3. Right rate and peak? | **Yes** | One play: the driver set variable rate to 22,050 Hz; the device took 39,045 frames (1.77 s at that rate, which matches the sink's 1.64 s of non-silent output plus the quiet tail), 177 buffers, 166 interrupts, peak 0.755, no sample-to-sample jump above 6,000 in the capture. Twenty-five plays 2.3 s apart over 57 s: 25 bursts of 1.64 s (to within 0.16 s), 987,885 frames consumed (44.8 s of audio at 22,050 Hz, as expected), **0 sink underruns, 0 dropped frames, 0 starved frames** |
| 4. Registers saved beside a state and restored? | **Yes, and needed** | The state (118.8 MB before zstd) was saved with the channel halted; the device's registers (`getState()`, about 1 KB of JSON) went into a sidecar. Restored in a fresh page, two plays: 78,325 frames, peak 0.755, 0 underruns. **Without the sidecar the restored guest stays silent** (0 frames in two plays; the codec's volume and rate registers live in the device). A state built without the device (the current `state.l22.bin.zst`) restored with the device present boots normally and never touches it |

**CPU cost.** Guest idle at the desktop, four 10 s windows per run, two runs each on one machine: **5.4 to 5.5 MIPS without the device, 5.5 to 5.7 with it, and 5.6 with the sink running**; the difference is inside the noise. On the page, rendering the device's stream cost 1.8 ms per second idle (silence) and 5.3 ms per second while playing, so about 0.5 % of one core. The worklet itself is a ring buffer copy.

**What the device had to do** (so WP-WEB-5 can harden it, not re-discover it): ICH bus-master registers (the PCM-out channel's buffer list base, current, last valid and prefetched indexes, status, position in buffer, control), a codec that answers as a plain AC'97 2.3 codec (reset, powered-up status `0x000F`, variable rate with `0x2C` honoured only when `0x2A` bit 0 is set, volume registers applied as gain, vendor ID `0x8384 0x7600`), the global control and status words with the primary codec ready bit, and a codec access semaphore that always reads free. The driver loops buffers with an interrupt on completion (about 72 interrupts per second at 22,050 Hz), clears the run bit when a sound ends and writes a new last valid index to resume; the model also resumes a channel halted at the last valid buffer when a new index is written with the run bit still set. The interrupt is level-style (`raise_irq` when an enabled status bit is set, `lower_irq` when the guest clears it).

**Fallback if it ever regresses.** PC speaker only (the page stops passing `disable_speaker: true`; beeps from `beep.sys`, no system sounds or saver scores), or a v86 fork carrying the device in `src/` (breaks SITE-7's "unchanged from npm" and needs v86's Closure build). Neither is needed now.

**Hand-offs for WP-WEB-5.**

- Register the device by wrapping `emulator.v86.init`, not on `emulator-loaded`, so the same code serves cold boots and states. Keep the `autostart: false` route as a documented alternative for cold boots only.
- Keep the device's registers in the manifest's `audio` object (or `state-<hash>.audio.json`) and always restore them with the state; save the state while no sound plays (the channel is halted, so only registers matter).
- The sink here pulls from the page: the worklet asks every 10.7 ms and the page tops its ring up to 120 ms. It runs in a headless browser with no sound card, so the autoplay and speaker paths are untested (below).
- The guest's first codec access comes about 21 s after the desktop on a cold boot; a state built with the device present avoids that wait, as the plan's state build already assumes.

**Not verified:** real speakers (the sink was measured, not heard), Firefox and Safari (the `AudioContext` start inside the Start click, `AudioWorklet` support, autoplay), mobile browsers, a hidden tab's timers, the Win32 and DOS builds (this is the ReactOS demo only), Quill's own sounds and the Afterglow savers' scores (the Sounds tab's WAV was the test), and CPU cost on a slow machine (MIPS were measured on a loaded workstation at 5.5 idle).

## Owner decisions

Each is taken with the default so no package waits; the last column is where to change it.

| Question | Default taken | Where to swap |
|---|---|---|
| A system-wide PDF printer, or Quill's own Print to PDF? | Quill's own (no printer driver exists for ReactOS 0.4.16) | Goal 2; WP-WEB-3 |
| Raster PDF now, text-layer later? | **Decided by the owner: a text layer from the start** (W9X-91); embedded font subsets later (W9X-92) | WP-WEB-3's `[Print] PdfDpi` (now the layout resolution, W9X-94) and `docs/linux.md` P7b |
| Which port carries files to the page? | COM1 with the `LHQ-FILE` frame | SITE-15 (WP-WEB-2) |
| Generic "Send to this computer" for any file? | Yes, `SENDHOST.CMD` and a desktop drop-target shortcut; Send To once rl supports it | WP-WEB-2; the reactos-liminal table |
| Sound through a page-side AC'97 device, a v86 fork, or none? | **Page-side device: WP-WEB-4 said go** (SITE-19); otherwise PC speaker only until the owner picks the fork | WP-WEB-4's report; the order table |
| Sound on or muted after Start? | On, at 70 %, remembered per browser | `sound.js` (WP-WEB-5) |
| Hosting | GitHub Pages, same origin, data pulled by the workflow from a pinned release asset | Goal 4; WP-WEB-7 |
| HTTPS on `labs.liminalhq.ca` | Ask the owner to fix the certificate (Pages' *Enforce HTTPS* after the domain's certificate is issued); the demo works over HTTP meanwhile | Pages settings for the organization |
| When to publish the data (and with it ReactOS and WineVDM binaries)? | Not until the owner says so; WP-WEB-8 stops at a local tarball | `web-data.pin`; the release |
| The remix wallpaper's picture and name | Keep Quill's emblem scene, named *Quill Afterglow* | WP-WEB-6; QAGRR's `settings.wallpaper` |

## Risks

- **The page-side device** depends on v86's internal API (`emulator.v86.cpu.devices.pci`, `cpu.io`, `cpu.read_blob`); a v86 update can move it. It is pinned with v86, and `PROVENANCE.md` will say so.
- **Interrupt routing.** v86's ACPI tables may not route a new PCI function's interrupt the way ReactOS with ACPI expects; WP-WEB-4 finds out first.
- **The state build with sound** adds a device the old states never had; every state must be rebuilt with it, and a state without the `audio` object must boot without the device.
- **COM1 speed** is fine for PDFs (0.3 MB/s) but a large file sent by `SENDHOST.CMD` holds the guest's `copy` for seconds; the page shows progress per frame.
- **Bandwidth** on Pages is soft-limited to about 100 GB a month, about 3,800 full sessions; a launch post could exceed it. Cloudflare is the fallback.
- **HTTP only.** Until the certificate is fixed the page is not a secure context; v86's `ScriptProcessorNode` fallback is deprecated (still in every browser) and the AC'97 output must have the same fallback.

## What was not verified

- That ReactOS in **v86** (not QEMU) installs and drives an AC'97 device: WP-WEB-4's question.
- That Quill's print layout reaches a PDF through `quill-pdfdoc` from the shell's own measure, and what a page weighs: WP-WEB-3 (W9X-90 to W9X-94).
- A drop on a desktop shortcut passing the path in ReactOS 0.4.16.
- Throughput and correctness of COM1 under throttled CPU (the machine was shared and loaded), and on Firefox and Safari.
- Pages' behaviour for `.zst` files (content type, on-the-fly compression); v86 decompresses the bytes itself, so only a changed byte would matter, and none is expected.
