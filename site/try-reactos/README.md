# Try Quill on ReactOS

ReactOS 0.4.16 with Quill, running in [v86](https://copy.sh/v86/) (an x86 emulator in WebAssembly) inside a browser tab. The page starts from a saved machine state (about 13 MB compressed) over a disk served as 8 MiB zstd parts (512 files, about 55 MB in all, read on demand), so a live desktop with Quill open needs about 16 MB. The page makes no request outside its own origin.

## Layout

| Path | What |
|---|---|
| `index.html`, `try-reactos.js`, `try-reactos.css` | The page: Start, Stop, Full screen, a status line and a builder and diagnostics panel (RAM, VGA memory, ACPI, JIT, boot order, disk, disk chunk size, CD and state URLs, a live read-out of IDE reads, video mode and MIPS, Save state) |
| `v86/` | v86 0.5.473 and its BIOS files, fetched by `tools/fetch-v86.sh` (not committed). `v86/PROVENANCE.md` lists versions, hashes and licences (v86 is BSD-2-Clause; SeaBIOS and SeaVGABIOS are LGPL-3.0) |
| `harness/serve.ts` | The local server: static files, HTTP range requests, per-client byte counters, a JSON request log |
| `harness/measure.ts`, `lib.ts`, `inpage.js` | The measurement run, on `playwright-core` driving the installed Chromium |
| `harness/desktop-shot.ts` | Restores the saved state and saves the guest's own pixels (the emulator canvas) as it restores and with the desktop shown, for checking a build headlessly |
| `harness/check-state.ts` | Restores a state over disk parts and checks the themed taskbar, Quill's window and a File > Open dialog from the canvas |
| `harness/watch.ts`, `explore.ts`, `build-state.ts`, `run-all.sh`, `summarise.ts`, `chunk-session.ts` | Screenshots on a timer, key-sequence exploration, saved-state builder, the full measurement set, the results table, a disk-touching session for comparing part sizes |
| `tools/fetch-v86.sh`, `make-parts.mjs`, `build-manifest.py`, `job-summary.py` | Downloads and checks the pinned v86; splits a raw disk image into zstd parts for v86's chunked disk; writes the build manifest; prints it as a CI job summary |
| `demo/` | The pitch document Quill opens: `demo.rtf` and `demo.qld`, and `demo16.*` for the Windows 3.x icon; `make-demo.sh` rebuilds them |
| `guest/` | Builds the guest disk and the artifacts from the official ReactOS install CD: `build-all.sh` (the whole chain), `install-golden.sh` with `unattend.inf` (the unattended install), `build-image.sh` with the trim lists, registry settings, wallpaper, desktop icon (`make_desk_icon.py`), shortcut writer and the build VM scripts (see [Build everything with one command](#build-everything-with-one-command) and [Rebuild the trimmed guest](#rebuild-the-trimmed-guest)) |
| `qagrr/` | The Quill Afterglow Remaster Remix (QAGRR), the recipe that replaces `guest/` and `tools/` one piece at a time: `qagrr.toml` (extends the compact Afterglow recipe in reactos-liminal, with Quill's names, programs, documents and shortcuts), `trim-extra.txt` (the six network card drivers), `quill-run.reg` (the cold-boot `Run` key and the error-report switches), `reactos-liminal.pin` (the pinned reactos-liminal commit) `build-with-rl.sh` (fetches that commit and runs `rl`), `build-state.sh` (the saved v86 state for those parts) and `check-state.sh` (restores it and looks); see [Build with rl](#build-with-rl) |

The disk images, CDs, states and parts live outside git, in `~/.cache/reactos/web/` (the server maps it to `/images/`; override with `IMAGES_DIR`). Never copy them into the repository.

## Run it

Needs Bun or Node 22+, `curl`, `tar`, and Chromium at `/usr/bin/chromium` (no browser is downloaded).

```sh
bash site/try-reactos/tools/fetch-v86.sh   # once: downloads the pinned v86 and checks its SHA-256 sums
cd site/try-reactos/harness
bun install                      # playwright-core only
bun serve.ts                     # http://127.0.0.1:8086/try-reactos/  (PORT, HOST, IMAGES_DIR, COI=1, LOG_FILE)
```

The server stops with a message naming `tools/fetch-v86.sh` when the emulator files are missing. `HOST=0.0.0.0` serves the LAN, read-only (the write endpoints answer this machine only). Then open `http://127.0.0.1:8086/try-reactos/?disk=/images/reactos16-spike-a.img&cdrom=/images/quill-demo.iso&boot=hd&ram=256` and select Start. Every field in the panel can be set from the address bar by its name (`ram`, `vga`, `acpi=0`, `jit=0`, `cpuid`, `boot=auto|cd|hd`, `disk`, `chunk`, `cdrom`, `state`, `run`, `speed`, `lock=os|raw`, `cursor=guest|local`). Also `scale=int|crisp|smooth` (how the screen is scaled) and `mouse=lock|follow|v86` (how the pointer reaches the guest; `v86` is v86's own mouse adapter, for comparing).

1. **Smoke test, the live CD:** `?cdrom=/images/livecd.iso&boot=cd` boots the ReactOS 0.4.16 installer. Or `node watch.ts --name livecd --seconds 90 --param cdrom=/images/livecd.iso --param boot=cd`; frames go to `out/livecd/`.
2. **A copied disk:** `node measure.ts --name spike-a --app quill32` (or `--app quill16` for the Win16 build, `--profile 50mbit|10mbit`).
3. **Make a saved state:** `node build-state.ts --name state-q32`, then `zstd -19 -T0 ~/.cache/reactos/web/state-q32.bin -o ~/.cache/reactos/web/state-q32.l19.bin.zst`.
4. **Make disk parts:** `node tools/make-parts.mjs IMAGE ~/.cache/reactos/web/parts/spike-a 8388608 19` (the size is optional; 8 MiB is the default, and `parts/manifest.json` records it, so the page needs no `chunk`).
5. **Restore from a state:** `node measure.ts --name restore --param state=/images/state-q32.l19.bin.zst` (add `--param disk=/images/parts/spike-a/.img.zst --param chunk=8388608` for the chunked disk; the page reads the size from the manifest when `chunk` is absent).
6. **The full set and the table:** `bash run-all.sh`, then `node summarise.ts`.
7. **Demo documents:** `demo/make-demo.sh "<path>/quill-document.svg" <path>/target/release/quill` (build the CLI with `cargo build --release -p quill-cli`).

Server endpoints: `GET /__stats` (bytes and requests per client, split into page, engine, bios, disk, cdrom, state; distinct disk bytes), `POST /__stats/reset`, `POST /__stats/flush` (writes `out/requests.json`), `POST /__put/NAME?offset=N` (stores a body in the images folder). A client is the `run` id in the page's address.

Input reaches the guest only through the page's own emulator API (`keyboard_send_scancodes`, `keyboard_send_text`, the mouse bus); the headless browser never touches the host desktop.

## The mouse

Measured with `harness/mouse-latency.ts` (events sent on the emulator's bus, pixels read from the canvas on every animation frame), `harness/mouse-real.ts` (real browser pointer events under pointer lock, from the event's own timestamp) and `harness/mouse-axis.ts` (diagonals on the bus, through the lock handler and in follow mode; `--check` fails when follow mode misses by more than 3 pixels or the lock handler sends more than 2 packets a frame). Headless Chromium 156, a restored desktop, on a machine at load average 30 to 60 on 16 threads, so every figure is a range.

| Question | Result |
|---|---|
| Is the guest slow to draw the pointer? | No. One bus event to the first canvas frame with the moved pointer: median 8.5 ms, 90th percentile 16.7 ms (20 trials; this is one animation frame, 60 Hz). While moving at 125 events a second the canvas changed on 59.7 of 60 frames a second, with the guest at 7.6 MIPS |
| Real browser event to pixel | v86's own adapter: median 30.3 ms (90th 34.9). The page's handler: 27.3 ms (90th 32.0). The same within noise; the browser's input queue and one or two frames are the cost, not the emulator |
| Guest pointer acceleration | None measured. ReactOS has `MouseSpeed` 1, thresholds 6 and 10, but one count of 1, 2, 4, 8, 16, 32 and 64 moved the pointer 1, 2, 4, 8, 16, 32 and 64 pixels, and sixteen counts of 2 went as far as one of 32. One count is one guest pixel |
| Pointer speed | The cause of "slow". v86 asks for `unadjustedMovement` (raw counts, no system acceleration) and sends one count per guest pixel whatever the size of the picture, so the guest pointer crosses the screen at a fraction of the system pointer's speed, and on a phone, where 1024 guest pixels fit in 360 CSS pixels, very slowly. 30 CSS pixels of hand movement moved the guest pointer 30 pixels with v86's adapter and 45 with the page's (default `speed` 1.5) |
| `VMware pointer: false` | v86's absolute pointer needs a guest `vmmouse` driver; ReactOS has none. The pointer stays relative |

What the page does now (`try-reactos.js`, behind the switches in the table above):

- **Own mouse handler, lock** (`?mouse=lock`, the default except in Firefox on Linux): every coalesced move is counted (`getCoalescedEvents`); the pointer lock is the system's own, so it follows the system's pointer settings (`?lock=raw` asks for unadjusted counts); a move is scaled by how large the picture is shown, so the guest pointer covers as much of the picture as the system pointer would, times `?speed` (1.5 by default, 1 is exact); whole counts are kept and the fraction carried; the counts of one animation frame go out as one PS/2 packet with both axes, so a diagonal stays diagonal and a fast hand cannot overrun the guest's input queue (it keeps about 100 packets: a burst of 120 one-count packets moved the pointer 100); a tap on a touch screen clicks, a drag moves. `?mouse=v86` gives v86's adapter back.
- **Own mouse handler, follow** (`?mouse=follow`, the default in Firefox on Linux): no pointer lock. The guest pointer is moved, once a frame, to the guest pixel under the system pointer (hidden over the screen), from the system pointer's absolute position; selecting the screen and entering it again parks the guest pointer in a corner first, so the dead reckoning starts from a known place. Why: Firefox on Wayland reports a locked pointer's moves as whole device pixels, each move cut to an integer on its own and a move of less than one pixel on both axes dropped (`relative_pointer_handle_relative_motion` in `widget/gtk/nsWindowWayland.cpp`). A mouse at 1000 Hz moves a fraction of a pixel per report, so the slower axis of a slow diagonal is lost and the guest pointer moves in horizontal and vertical runs, worst at low DPI. `harness/mouse-axis.ts` replays such a sweep: a 2:1 diagonal of 240 by 120 logical pixels, at 0.8 and 0.4 pixels per report and scale 1.5, arrives as 300 by 0 device pixels where 360 by 180 were moved (the guest pointer moved 451 by 0, pure horizontal), and at twice the speed as 600 by 300 where 720 by 360 were moved (a 17% loss on both axes); the same sweep with positions kept arrives exact. The guest, v86 and the image are not involved: on the bus, 400 by 400, 300 by 150 and 200 by 200 counts moved the pointer exactly that far on both snapshot builds, and in follow mode every stop of a sweep landed on the target pixel.
- **`?cursor=local`**: an arrow drawn by the page at the predicted guest position, moved in the event handler itself. The guest is relative with no acceleration and clamps at the edges, so the position is dead reckoning; the first lock parks the guest pointer in the corner and moves it to the middle to calibrate. The cost: the guest still draws its own pointer a frame or so later (and an I-beam over text), so a trailing second pointer shows while moving, and the overlay is always an arrow. Because the measured guest delay is one frame, the gain is small (the overlay is placed in the event's own task, the guest pointer 8 to 17 ms after it), so it is off by default. Hiding the guest's pointer needs a transparent cursor scheme in the guest (`HKCU\Control Panel\Cursors`, every role pointing at a blank `.cur`); a pointer set that does this is for the Afterglow theme (below) and is not in the pinned source yet.
- **Not changed:** `screen_set_scale` and the animation-frame cadence: the canvas already changes on every frame while moving, so there is nothing to gain.

`guest/polish/desktop.reg` switches the version watermark off. `guest/polish/mouse.reg` sets `MouseSpeed` 0, `MouseThreshold1` 0, `MouseThreshold2` 0 and `MouseSensitivity` 10, so that one count stays one pixel. It has no measurable effect on this guest (it was already linear). `guest/polish/apply-mouse.cmd` applies every `polish/*.reg`; `build-image.sh` copies them to the settings CD in one marked block and `apply-settings.cmd` calls the script last. `SmoothMouseXCurve` is not set (ReactOS does not read it).

## The Windows 3.x icon

The desktop shortcut is `Quill (Windows 3.x, slow)`: `C:\Program Files\otvdm\otvdmw.exe C:\Quill\QUILL.EXE C:\Quill\DEMO16.QLD`, start folder `C:\Quill`, a description that says it is slow, "Normal window". Both builds centre their window on the screen at the same size (neither stores a position in `QUILL.INI`), and WineVDM passes Win16 coordinates through, so the two windows open on top of each other; a shortcut cannot change that. Cascading needs a change in Quill (the Win16 build offering a small offset when another Quill window exists).

WineVDM's launcher is named in the shortcut on purpose. Run through ReactOS's own 16-bit hand-off (`C:\Quill\QUILL.EXE C:\Quill\DEMO16.QLD` in a shortcut or the Run box), the task's command line arrives empty (the PSP length byte is 0 and the tail is a lone carriage return), so Quill opens Untitled. Run through `otvdm.exe` or `otvdmw.exe` it arrives whole (length 0x13, `C:\Quill\DEMO16.QLD`, at `ES:BX` = PSP:0081), and `otvdmw.exe` has no console window. Measured in the guest with a build that dumped the PSP.

A disk must carry the Win16 program of the same source as the rest. A `QUILL.EXE` kept from a day earlier showed a `SEGV` box from WineVDM at startup (`access address=000009f2`): a build from before `fix(win16): pass GetProfileString's key as a far pointer`, which gave `GetProfileString` a near offset where a far pointer is wanted, so the host read selector 0 at that offset. Windows 3.11 reads selector 0 as NULL and goes on; WineVDM's host faults. `build-with-rl.sh --build-quill` builds both programs from the checkout, and the script refuses a `--quill-dir` older than the last commit to the sources (`--allow-stale-quill` overrides). `qagrr/check-win16.sh IMAGE` boots a copy of the disk headless in QEMU, double-clicks the shortcut and checks from the screendump that the Win16 window is open; run it after each image build (it needs QEMU, ImageMagick and a few minutes; KVM when present).

## The desktop icon

The two desktop shortcuts use `C:\Quill\QUILLDESK.ICO`, which `guest/make_desk_icon.py` builds at disk-build time from `assets/icon` through `tools/make_icons.py`'s own functions (nothing binary is committed). Quill's `QUILL.ICO` draws 16 to 32 px from the small-size art (a thicker ring, a plain nib) and 48 px and up from the detailed master. The demo desktop has room for the master, so `QUILLDESK.ICO` has the small art at 16 px (4, 8, 24 and 32 bits) and the master at 32 and 48 px (8, 24 and 32 bits) and 256 px (PNG). `demo-settings.reg` sets `HKCU\Control Panel\Desktop\WindowMetrics\Shell Icon Size` to `48`, which ReactOS 0.4.16's shell honours: the desktop shows 48 px icons and the 48 px frame is used as it is, with no scaling. The small icons (menus, taskbar) stay 16 px. The Windows 3.x shortcut uses the same file, because a 32 px 16-colour Windows 3.x icon would be stretched to 48 px. `make-lnk.py --icon FILE [--icon-index N]` writes the icon location; without it a link takes its target's first icon. The two-line label of the Windows 3.x shortcut (`Quill (Windows 3.x, slow)`) fits the icon grid with room before the next icon. The programs' own embedded icons are not changed: `QUILL32.EXE` carries `QUILL.ICO`, so its 32 px frame is the small art and its 48 px frame is the master.

## The Afterglow theme

The demo boots with the Afterglow desktop theme: the dusk caption, the near-black taskbar with a gold Start button, classic scroll bars, and the smooth Afterglow dusk wallpaper with the Quill emblem round the setting sun (`WALLPAPER_STYLE=stepped` builds the stepped one). The theme is made in the Afterglow repository ([`liminal-hq/afterglow-themes`](https://github.com/liminal-hq/afterglow-themes)); none of its art or binaries is committed here. The build fetches a pinned source and adds Quill's own overlay.

| File in `guest/` | What |
|---|---|
| `afterglow.pin` | The pin: the Afterglow commit, the SHA-256 of its tar, the SHA-256 of the built `afterglow.msstyles`, the wallpaper and the apply file used |
| `fetch-afterglow.sh` | Downloads the GitHub tarball of the pinned commit, checks the SHA-256 of the uncompressed tar (GitHub may recompress an archive, so the sum is of the tar), refuses a path that leaves the folder and extracts it to `~/.cache/reactos/afterglow/<commit>` (never inside the repository) |
| `prepare-afterglow.sh OUT` | Fetches, runs `desktop/style/build.sh` from the fetched tree (the default build, classic scroll bars, colour scheme Dusk), checks the style's SHA-256 against the pin, copies the apply registry file, builds the 16 pointers with `desktop/pointers/gen-cursors.py` (Pillow needed) and runs `quillify.py`; writes `afterglow.msstyles`, `afterglow.reg`, `pointers.reg`, `cursors/` and `quill-wallpaper.bmp` to `OUT` |
| `prepare-pointers.sh DISK OUT` | Takes `user32.dll` from the golden disk with mtools, patches a copy with `desktop/pointers/patch-user32.py` (which accepts only ReactOS 0.4.16's file) and checks the result against `desktop/pointers/SHA256SUMS.user32`; writes `OUT/user32-afterglow.dll`. `build-all.sh` runs it after the golden disk is ready |
| `quillify.py`, `quill-emblem.svg` | The overlay, which stays in this repository: it renders Afterglow's `afterglow-dusk-dark-smooth.svg` (`WALLPAPER_STYLE=stepped` takes `afterglow-dusk-dark-stepped.svg`) at 3840 × 2160 with `rsvg-convert`, draws the emblem over it (the gilt ring centred on the sun at (2880, 1846), whose gold disc has radius 104, so the sun sits inside the ring; the nib is the tail of the Q), takes the centre 4:3 crop (x 480, 2880 wide, as Afterglow's own render script does) and scales it to the 1024 × 768 24-bit BMP with ImageMagick. For the smooth wallpaper it then adds the seeded ±1 level triangular noise of Afterglow's `scripts/render-wallpapers.mjs` (mulberry32, seed 2026, the same key; the output is byte-identical to that script's own BMP when there is no emblem) after the crop and scale, because a smooth dark gradient bands at 24 bits and scaling dithered pixels would average the noise away. The guest's display is true colour (the `bochsmp` driver offers 32 bits only and the screen shows the BMP's pixels exactly), so no 16-bit reduction is needed; `--rgb565` (`WALLPAPER_RGB565=1`) reduces to 5-6-5 levels with an ordered dither for a 16-bit display. The sun lands at (853, 656) with radius 37. It stops if the fetched wallpaper's sun is not where it expects |
| `test_afterglow.py` | Tests: the pin's shape, the fetch on a local tarball (good, changed, unsafe path, recompressed), the dither against Afterglow's own values, the 5-6-5 reduction, the desktop icon's frames, the shortcut's icon field, the crop and sun geometry, and a render of a synthetic wallpaper (CI job `ReactOS demo scripts`) |

`build-all.sh` runs `prepare-afterglow.sh` by default and hands its folder to `build-image.sh` as `AFTERGLOW_DIR`, which copies the style onto the disk (`C:\ReactOS\Resources\Themes\Afterglow\`), puts the pointers in `C:\ReactOS\Cursors\Afterglow\`, replaces `C:\ReactOS\system32\user32.dll` with the patched copy (ReactOS 0.4.16 ignores pointer schemes and loads the system pointers from that file; the patched file is on the demo disk only and is never committed), applies `afterglow.reg` and `pointers.reg` with the other polish and uses `quill-wallpaper.bmp` as the desktop. `AFTERGLOW=0` skips it, and so does a `build-image.sh` run with `AFTERGLOW_DIR` unset: the demo then has ReactOS's classic look with the Quill wallpaper (`quill-wallpaper.svg`). An `AFTERGLOW_DIR` set for `build-all.sh` is used as it is.

Prerequisites for the Afterglow step: network (once; the source is then cached), `curl`, `tar`, Python 3 with numpy and Pillow (the style and pointer generators), mtools, `llvm-rc` and `lld-link` (Ubuntu: `llvm` and `lld`), `rsvg-convert` and ImageMagick 7.

To move to a newer Afterglow, change `AFTERGLOW_COMMIT` in `afterglow.pin`, run `AFTERGLOW_PRINT_SUMS=1 guest/fetch-afterglow.sh` to see the tar's sum, then `guest/prepare-afterglow.sh` to see the style's (it stops on a mismatch and prints the new one), and put both in the pin. When Afterglow publishes a release the pin can name its tag instead; there is none yet, and this repository tags nothing.

The pointer set is part of the pinned source: the pointers are generated, `user32.dll` is patched on the demo disk only, and neither the patched file nor the generated pointers are committed or published outside the private workflow artifact.

## Build with rl

The ReactOS pipeline now lives in [reactos-liminal](https://github.com/liminal-hq-labs/reactos-liminal) (the `rl` command), which builds the Afterglow ReactOS remaster (AGR). Quill is a downstream consumer: its own remix, the Quill Afterglow Remaster Remix (QAGRR), is the recipe in `qagrr/`. It extends the AGR compact recipe (`afterglow-compact.toml`: the full base recipe with its `base` trim, which the Quill demo keeps) and owns only what is Quill's: the computer and owner names, the trim beyond the compact profile, the programs, the demo documents, the two desktop shortcuts, the 48 px icon size, the cold-boot `Run` key (`quill-run.reg`, a recipe-relative `.reg` file in `settings.reg_files`) and the wallpaper. The orb, the icons, the logon, the boot logo and the `afterfetch` and `neofetch` files come from the base and are not repeated (`[[files]]` merge by destination).

```sh
site/try-reactos/qagrr/build-with-rl.sh --check-trim                 # base profile + trim-extra.txt = the old trim lists
site/try-reactos/qagrr/build-with-rl.sh --dry-run                    # rl explain, then the plans for install and image
site/try-reactos/qagrr/build-with-rl.sh --quill-dir DIR --work DIR   # install, image, parts, manifest
site/try-reactos/qagrr/build-with-rl.sh --build-quill --work DIR     # the same, with both programs built from this checkout first
site/try-reactos/qagrr/check-win16.sh WORK/images/qagrr.img SHOTS    # boot a copy, open the Windows 3.x shortcut, check the window
```

`build-with-rl.sh` fetches the commit in `qagrr/reactos-liminal.pin` into `~/.cache/quill-reactos-liminal` (outside the repository), then runs `rl explain`, `fetch`, `install`, `image`, `parts` and `manifest`. Before the image it renders the wallpaper with `guest/quillify.py` (the base recipe's pinned Afterglow scene with the Quill emblem on its sun), which the recipe lists in `[[files]]`; rl then skips its own scene. rl's `base` trim keeps the Applications Manager and the browser stub, which the old lists removed, so `--check-trim` expects those three entries to differ. `--work` is rl's work directory. Set `RL_QEMU_LOCK` to a lock file to run the two QEMU steps under `flock`. The `reactos-qagrr.yml` workflow does the same on a runner, by hand only.

### The saved state

```sh
site/try-reactos/qagrr/build-state.sh DIST   # DIST holds parts/ from build-with-rl.sh; writes DIST/state.l22.bin.zst
site/try-reactos/qagrr/check-state.sh DIST    # restores it in headless Chromium: desktop, Quill window, File > Open; then the session numbers
```

`build-state.sh` serves `DIST` with `harness/serve.ts` on 127.0.0.1 (port 18086), boots `DIST/parts` in v86 in headless Chromium through `harness/build-state.ts`, waits for the Afterglow desktop, then for Quill's window with the demo letter (the `Run` key from `quill-run.reg` opens it on a cold boot) and for the disk to go quiet, saves the machine state through the page's own `save_state()` and compresses it with `zstd -22 --ultra`. Nothing touches the host desktop: input goes through the page's emulator API and pixels come from the emulator canvas. The raw state (about 111 MB) is deleted afterwards; the compressed file is about 14 MB.

The state is not reproducible byte for byte: it is a snapshot of a boot whose timing depends on the host, so two runs differ and the compressed size moves by a few per cent. The disk parts are the checked artefact (SHA-256 in the build manifest); `check-state.sh` checks what the state shows instead. A state belongs with the parts it was taken from. `check-state.sh` runs `harness/check-state.ts` (the themed taskbar, a Quill window with the demo letter, and a File > Open dialog that changes at least 2% of the screen; it saves `restored.png` and `open-dialog.png` from the canvas) and then `harness/chunk-session.ts`, which prints the seconds to the live desktop, the disk bytes and requests, and the IDE reads of a session that opens menus and dialogs.

Measured on the first QAGRR build (4 GiB image, 512 parts of 8 MiB, 51.4 MB of parts, 466 of them empty): the state build took 95 s with the zstd step, 2.5 s of it the save; the state is 111,037,796 bytes raw and 14,259,534 compressed. A restore over the parts fetched 16.9 MB of state and 9.0 MB of parts in 8 disk requests (19 requests in all, 25.9 MB served), and showed the desktop in 0.3 s unthrottled and 3.3 s at 50 Mbit/s with 40 ms latency.

The old chain's state (`guest/build-all.sh`, which still runs `harness/build-state.ts` itself) stays until the owner retires it.

## Build everything with one command

```sh
site/try-reactos/guest/build-all.sh
```

From the official ReactOS 0.4.16 install CD to the final artifacts, with no hand step. Needs `qemu-system-i386` (KVM when `/dev/kvm` is writable, else software emulation: hours, not minutes), `mtools`, `xorriso`, `zstd`, ImageMagick, `rsvg-convert`, Python 3, `curl`, `unzip`, Bun or Node 22.18+ with `playwright-core` (the script installs it into `harness/`) and a Chromium (`CHROMIUM`, default `/usr/bin/chromium`). The stages, each timed:

| Stage | What it does |
|---|---|
| `inputs` | Takes `QUILL.EXE` and `QUILL32.EXE` from `$QUILL_DIR` (default: extracted from `~/.cache/reactos/quill-latest.iso`), the demo documents from there or from `demo/`, and renders the wallpaper from `guest/quill-wallpaper.svg` |
| `install-golden` | Downloads the ISO (or reuses a copy whose hash matches) and installs ReactOS onto a blank raw 4 GiB disk with no prompt (below). Skipped when the disk's stamp matches the ISO hash, `unattend.inf`, `install-golden.sh` and `patch-iso.py` |
| `build-image` | `guest/build-image.sh`: copy, trim, put Quill, the documents and the wallpaper on the disk, apply the settings in a build VM, power off |
| `zero-free` | `guest/zero-free.py` and `fallocate --dig-holes`: free clusters become holes |
| `make-parts` | `tools/make-parts.mjs`: 8 MiB zstd parts and `parts/manifest.json` |
| `build-state` | Starts `harness/serve.ts` on port 18086, boots the image in v86 in headless Chromium until Quill shows the demo letter, saves the machine state |
| `zstd-state` | `zstd -22 --ultra` |

Outputs, in `$DIST` (default `$WORK/dist`): `parts/` with `manifest.json`, `state.l22.bin.zst`, and `build-manifest.json` (SHA-256 and size of every output, the pinned versions, the Quill build hashes, the stage timings and the peak disk use). Every large file stays under `$WORK` (default `~/.cache/reactos/web/ci/work`; the script refuses a path inside the repository), and the install's VM files, screenshots and serial logs under `~/.cache/reactos/web/ci/install/`. Other settings: `GOLDEN_IMG` (start from an existing golden disk), `QUILL_COMMIT`, `PORT`, `ACCEL`, `STAGE_TIMEOUT`, `BOOT_WAIT`, `STATE_TIMEOUT` (see the header of `build-all.sh`).

Measured on a 16-thread development machine shared with other builds, with KVM:

| Stage | Quiet machine | Loaded machine |
|---|---:|---:|
| `inputs` | 1 | 1 |
| `install-golden` (the unattended install; text, graphical and first-boot runs took 56 to 91 s, 40 to 56 s and 71 to 76 s) | 167 | 162 |
| `build-image` | 84 | 83 |
| `zero-free` | 2 | 5 |
| `make-parts` | 78 | 155 |
| `build-state` | 91 | 151 |
| `zstd-state` | 29 | 99 |
| Whole chain, from the ISO to the manifest | about 7.5 minutes | 658 s (11 minutes) |

| Output | Hand-made disk | Scripted |
|---|---:|---:|
| Saved state, `zstd -22` | 13.2 MB | 13.2 MB (13,213,268 bytes) |
| Disk parts (`zstd -19`) | about 52 to 54 MB | 53.8 MB (4,096 parts, 3,820 of them empty) |
| Peak allocated disk (golden, image, state, install CD) | | 847 MB at the end of a stage (the ISO adds about 400 MB while it is copied) |

### The unattended install

ReactOS setup reads `UNATTEND.INF` from its install source, which on the CD is `\I386\`. The official disc ships one with `UnattendSetupEnabled = no`, so `install-golden.sh` copies the ISO and overwrites that file in place with `guest/unattend.inf` (`guest/patch-iso.py`; each patched file keeps its size and place, so the rest of the disc is byte for byte as shipped, and the download is never touched). Rebuilding the image with xorriso was tried first and did not work: the boot loader could not load the registry hive from the rebuilt disc. Setup then copies the file to the end of `System32\$winnt$.inf`, where the graphical second stage reads its own keys.

| Key | What it does |
|---|---|
| `Signature = "$ReactOS$"`, `UnattendSetupEnabled = yes` | Turns the unattended mode on |
| `DestinationDiskNumber`, `DestinationPartitionNumber`, `AutoPartition` | Install to disk 0, partition 1; on a blank disk the whole disk becomes that one partition |
| `InstallationDirectory = ReactOS` | `C:\ReactOS` |
| `FormatPartition = 1`, `FsType = 0` | Format with FAT (FAT32 at this size; BtrFS is 1) |
| `BootLoaderLocation = 2` | The boot loader goes on the system partition (MBR and boot sector) |
| `LocaleID = 409` | English (United States) with the US keyboard layout. The computer type (ACPI PC uniprocessor), display (VESA) and keyboard driver are the first entries of setup's lists, which unattended mode takes |
| `FullName`, `ComputerName`, `TimeZoneIndex`, `RappsDownload`, `InstallationType` | Second-stage answers: owner, computer name, time zone (GMT), no downloads from the application manager, workstation |
| `[GuiRunOnce]` | At the first logon, writes `ci: desktop` to COM1 and runs `shutdown -s -t 45`, so the host knows the desktop came up and the disk is closed cleanly |
| `[Shell] DefaultThemesOff = yes` | The classic look, as the hand install has it |

Three more changes go into the derived disc or the installed disk, because this release is a debug build. `TXTSETUP.SIF` starts setup's own kernel without the debugger (`/NODEBUG`; with it, setup broke into a debugger nobody was attached to, on one run in three). `FREELDR.INI` on the disc defaults to the text-mode setup instead of the live environment. And the installed `freeldr.ini`, which an unattended install of a debug build sets to start the kernel debugger on COM1 with no delay, is put back to the interactive install's (`DefaultOS=ReactOS`, `TimeOut=3`) right after the first stage, so the guest boots normally and COM1 is free.

The three QEMU runs use `-machine pc -cpu pentium3 -m 768 -vga std -display none`, a raw disk, QEMU's own monitor socket and pidfile in `~/.cache/reactos/web/ci/install/`, and `-no-reboot`, so a guest restart ends the run: text-mode setup (`-boot order=d`), the graphical second stage and the first boot (`-boot order=c`). Each has a timeout (30 minutes with KVM, 2 hours without), and a screenshot from QEMU's monitor every minute goes to `install/shots/`. The disk must be raw: on a qcow2 disk setup stops with "Setup failed to install the FAT32 bootcode on the system partition". No key is sent to the guest at any point.

Compared with the hand-installed golden disk: the same partition table (one bootable FAT32 partition at sector 2048, 8,386,528 sectors), the same boot sector apart from the volume serial, an identical `freeldr.ini` with the same attributes, and 1,739 files against 1,750. The 12 files only on the hand-made disk are leftovers of its use (`Quill.ini`, the test documents in My Documents, a cookie index, the application manager's folder, `cmos.ram`); the one only on the scripted disk is the installer package, whose name is random. Ten files differ in size, all registry hives and their logs, `setuplog.txt` and `$winnt$.inf` (which here ends with the answers file).

### What is pinned

| What | Value |
|---|---|
| ReactOS | 0.4.16, tag `0.4.16-release`; `https://downloads.sourceforge.net/project/reactos/ReactOS/0.4.16/ReactOS-0.4.16-i386.zip`, SHA-256 `e5851510cf5b79ef51a76b8155dd5ef7faca1155f44d0a1d41c35b77ece4ce2a`; the ISO inside it, SHA-256 `6f15ca6e1edf5527b8489958d16c54f0da0e7948c305d7c89e74687c6cbf6855` (both in `install-golden.sh`, which stops on a mismatch) |
| v86 | 0.5.473 and its BIOS files, downloaded by `tools/fetch-v86.sh` against pinned SHA-256 sums (`v86/PROVENANCE.md`) |
| Chromium and Playwright | `playwright-core` 1.65.0 (`harness/package.json`); CI uses Playwright's own Chromium |
| Quill | The build hashes and the commit are in `build-manifest.json` |
| Tools | Whatever the runner or machine installs: QEMU, mtools, xorriso, zstd (no version is pinned) |

### In CI

`.github/workflows/reactos-demo.yml` runs on manual dispatch only (a weekly schedule is there, commented out), never on a pull request. One job builds `QUILL32.EXE` and `QUILL.EXE` from the checkout (`win9x/build.sh`, `win16/build.sh`); the second installs QEMU and the tools, opens `/dev/kvm` with the usual udev rule (software emulation otherwise, with a 300-minute limit), installs Playwright's Chromium, restores the golden disk from `actions/cache` (key: the hash of `unattend.inf`, `install-golden.sh` and `patch-iso.py`, the last of which holds the ISO address and hashes), fetches v86, runs `build-all.sh` with its work under `/mnt` (a hosted runner has about 14 GB free on `/` and much more on `/mnt`; a run needs under 2 GB), and uploads `dist` as the `reactos-demo-image` artifact for seven days. The job summary has the size and timing tables and the licence list below. The first run installs (a few minutes with KVM, much longer without); later runs restore the cached disk.

### Licences

The image holds ReactOS (GPL-2.0-or-later, with LGPL-2.1-or-later for the Wine-derived DLLs and other free licences for bundled parts), WineVDM (GPL-2.0) and, in the page, v86 (BSD-2-Clause) with SeaBIOS and SeaVGABIOS (LGPL-3.0). It stays a private workflow artifact with a short retention until a decision to publish. Publishing it means publishing the corresponding source or a written offer for it (the ReactOS 0.4.16 source at its release tag, the WineVDM release and its source, the SeaBIOS 1.16.2 source, and, from the Afterglow repository, the Afterglow source at the pinned commit with the Quillify overlay, and the script that patches `user32.dll` on the demo disk once a pointer set is applied), the licence texts, and the list of what changed (the trim lists, `guest/demo-settings.reg`, `guest/unattend.inf`, the wallpaper and shortcuts, and Quill in `C:\Quill`). The job summary repeats the list. The fonts are ReactOS's metric-compatible clones; take a font inventory first.

## Rebuild the trimmed guest

`build-all.sh` runs everything below. The demo guest is the QEMU-installed ReactOS 0.4.16 disk (made by `install-golden.sh`, or an existing golden disk such as the hand-installed `~/.cache/reactos/reactos16-golden.img`, read only, never written), trimmed, set up for the demo and saved as a state. Everything below writes to copies under `~/.cache/reactos/web/`. Needs `mtools`, `xorriso`, QEMU with KVM, ImageMagick, `rsvg-convert`, `clang`, `lld-link`, `llvm-dlltool`, `zstd`, Python 3 and the harness set up as above. `guest/vm/` drives a build VM through QEMU's own monitor (`sendkey` and `screendump` on its own socket); it never touches the host desktop, and the VM binary is a symlink called `vmbox` so a name-based kill elsewhere cannot hit it.

1. **Wallpaper.** `guest/make-wallpaper.sh "<Quill>/desktop backgrounds/quill-wallpaper.svg" ~/.cache/reactos/web/p2/wp/quill-1024x768.bmp` crops the 4:3 centre of the 3840 × 2160 art and scales it to 1024 × 768 as a 24-bit BMP (2.4 MB, 62 KB with `zstd -19`).
2. **Memory-fill tool (only for the zeroing experiment).** `guest/build-memfill.sh ~/.cache/reactos/web/p2/MEMFILL.EXE`.
3. **Disk.** `guest/build-image.sh ~/.cache/reactos/reactos16-golden.img ~/.cache/reactos/web/reactos16-trim.img ~/.cache/reactos/quill-closefix.iso ~/.cache/reactos/web/demo-cd ~/.cache/reactos/web/p2/wp/quill-1024x768.bmp`. It copies the golden disk sparsely, runs `trim.sh` with `trim-list-1.txt`, `-2` and `-3` (a log of bytes saved per item goes to `$VMDIR/trimN.log`), puts `C:\Quill\` (both builds, the four demo documents, `QUILL.HLP`), the wallpaper and the two desktop shortcuts on it, builds a settings CD, boots the build VM, applies `demo-settings.reg` and the 64 MB page file inside the guest through `apply-settings.cmd` (Win+R, retried until the guest's serial port reports it), and waits for the guest to power itself off. `build-all.sh` ran it start to finish on a disk made by `install-golden.sh` (84 seconds; the first Run box took). The `demo-cd` folder holds `DEMO.QLD`, `DEMO.RTF`, `DEMO16.QLD`, `DEMO16.RTF` (from `demo/`, upper-cased).
4. **Zero the free space and make the file sparse again.** `python3 -I guest/zero-free.py IMAGE; fallocate --dig-holes IMAGE`. Deleted files leave their data in the image; this makes it compress and split into tiny parts.
5. **Optional: copy the memory-fill tool onto the disk.** `mcopy -i IMAGE@@1048576 ~/.cache/reactos/web/p2/MEMFILL.EXE ::/ReactOS/MEMFILL.EXE` (the state experiment `--zero` runs it).
6. **State.** Start the server, then `node harness/build-state.ts --name state-p2` (cold boot; the image opens `DEMO.QLD` itself from its Run key; add `--param ram=192` or `--param vga=4` to try those; `--zero` runs the memory-fill tool first). Compress: `zstd -19 -T0 state-p2.bin` (16 MB) or `zstd -22 --ultra` (13 MB).
7. **Parts.** `node tools/make-parts.mjs ~/.cache/reactos/web/reactos16-trim.img ~/.cache/reactos/web/parts/trim 8388608 19`.
8. **Measure.** `node harness/measure.ts --name p2-cold-whole --app auto` (cold boot, whole file; `--app auto` waits for Quill to open itself), and with `--param state=/images/state-p2.l19.bin.zst` for the restore. `harness/run-phase2.sh` runs the set.

To look at the guest or change a setting by hand, `guest/vm/qrun.sh IMAGE CDISO` starts the build VM, `guest/vm/run.sh 'cmd /c D:\SCRIPT.CMD'` runs a script from the CD in the Run box, `guest/vm/shot.sh out.png` takes a screenshot, and `system_powerdown` on the monitor (`VMDIR/mon.sock`) shuts the guest down cleanly. Always shut the guest down before reading the disk with mtools (hidden and system files such as `freeldr.ini` only show with `mdir -a`).

### What the trim removes and what the demo uses

Lists and per-item bytes are in `guest/trim-list-*.txt`. Kept on purpose: `explorer`, `cmd`, `notepad`, `regedit`, `reg`, `shutdown`, the control panel applets, the display, disk, keyboard, mouse, PS/2 and network drivers, every `.inf` (new-hardware detection reads them), the classic look, and WineVDM (`Program Files\otvdm`, 29 MB) for the Windows 3.x icon. The Windows 3.x icon needs nothing outside `Program Files\otvdm` (WineVDM's `otvdm.exe`, its `*.dll16`, `*.drv16`, `*.exe16` and `*.mod16` files, `dll\` and `WINDOWS\`) and the Win32 DLLs ReactOS already has; the trim leaves all of it, and the 16-bit build still starts after every trim batch. The fonts are metric-compatible clones (Arial, Times New Roman, Courier New, Tahoma and the like are Liberation, Croscore and Katayama Hirofumi MZ clones, not Microsoft's), so there is no font licence problem; the demo uses Times New Roman, Arial, Courier New, Tahoma, Symbol and Wingdings.

## Results

Measured in headless Chromium 156 on a machine shared with other builds (load average 12 to 38 on 16 threads), so treat times as a range.

| Measure | Result |
|---|---|
| The QEMU image in v86 (256 MB, ACPI on) | Boots unchanged to the desktop; 800 × 600 × 32; no prompts; both Quill builds start |
| Cold boot to the desktop, whole-file disk | 21 to 27 s unthrottled; 75.8 MB served; 156 s at 50 Mbit/s, 322 s at 10 Mbit/s |
| Cold boot, 1 MiB zstd parts | 40.0 MB served, 155 requests; 48 s at 50 Mbit/s |
| Saved state | 132 MB raw, 22.2 MB at `zstd -19` (27.6 at -9, 32.5 at -3) |
| Restore to a live desktop with Quill open | 0.5 s unthrottled, 4.8 s at 50 Mbit/s, 20.8 s at 10 Mbit/s; 24.7 MB served; 0 disk bytes |
| Typing in `QUILL32.EXE` | Cold: median 80 ms, 95th percentile 100 to 113 ms. After a restore: unresolved under load (49 to 300 ms median) |
| Typing in `QUILL.EXE` (WineVDM) | Median about 1 s |
| Scrolling | 10 to 20 changed frames a second |
| Renderer memory | 580 to 1,300 MB |
| ACPI off | Does not boot |

### The trimmed guest (same machine, same load caveat)

| Measure | Untrimmed | Trimmed |
|---|---|---|
| Guest disk, files | 421.6 MB | 272.0 MB (237 MB allocated; parts 53.8 MB, was 154 MB) |
| Saved state, `zstd -19` / `-22` | 22.2 MB | 16.2 MB / 13.2 MB |
| Restore to a live desktop: seconds (unthrottled, 50, 10 Mbit/s) and MB | 0.5, 4.8, 20.8 s; 24.7 MB | 0.5, 3.4, 13.6 s; 15.8 MB |
| Cold boot, whole file / 1 MiB parts (MB, unthrottled) | 75.8 / 40.0 | 64.0 / 31.0 |
| Cold boot, 50 Mbit/s, whole file / parts | 156 s / 48 s | 135 s / 30 s |
| Video | 800 × 600 × 32 | 1024 × 768 × 32 (`bochsmp` offers no 16-bit mode) |
| RAM 192 MB, VGA 4 MB, zeroed free memory | not tried | no gain (state within 1 to 2 %) |

`node harness/summarise.ts` prints every run in `out/results/` as a table.

### Disk part size

The disk is 4 GiB raw and nearly all of it is empty, so the parts are mostly a few bytes each; the count is what GitHub Pages deploys, a Release (at most 1,000 assets) and a CDN have to carry. v86 downloads a whole part for every read that lands in it, so bigger parts cost more bytes per session. The same disk (the Afterglow build) cut at five sizes, `zstd -19`, and one session from the saved state in headless Chromium: restore, Quill's File menu and Open dialog, the Start menu and Programs, Run, six Page Downs (`harness/chunk-session.ts`). The saved state is 14.3 MB and the page and emulator about 2.6 MB more, the same for every size; 22 IDE reads (1.4 MB) in every run.

| Part size | Files | All parts | Largest part | Disk bytes in the session | Session total | Requests | Disk time at 50 / 10 Mbit/s | To the desktop at 50 / 10 Mbit/s |
|---|---:|---:|---:|---:|---:|---:|---:|---|
| 1 MiB | 4,096 | 54.7 MB | 0.95 MB | 2.9 MB (15 requests) | 19.8 MB | 26 | 0.5 / 2.3 s | 3.3 / 14.3 s |
| 2 MiB | 2,048 | 52.6 MB | 1.44 MB | 3.7 MB (11) | 20.7 MB | 22 | 0.6 / 3.0 s | 3.3 / 14.3 s |
| 4 MiB | 1,024 | 50.7 MB | 2.13 MB | 6.9 MB (11) | 23.8 MB | 22 | 1.1 / 5.5 s | 3.3 / 14.3 s |
| **8 MiB** | **512** | 49.2 MB | 2.92 MB | 8.9 MB (8) | 25.9 MB | 19 | 1.4 / 7.2 s | 3.3 / 14.3 s |
| 16 MiB | 256 | 47.8 MB | 4.05 MB | 12.9 MB (7) | 29.9 MB | 18 | 2.1 / 10.3 s | 3.3 / 14.3 s |

The first frame shows at 0.15 s on both throttle profiles and the restored desktop is live when the state has arrived, so the time to the desktop is the state's download and does not change with the part size; the disk is read after it. The pitch session in `measure.ts` (typing, scrolling) reads no disk at all after a restore, so it cannot tell the sizes apart. 8 MiB is the default: 512 files, a session download 1.3 times that of 1 MiB parts (the limit was 1.5), and 1 to 5 s more disk time spread over the session. 4 MiB (1,024 files) would not fit a Release; 16 MiB reaches 1.5 times. Part sets of any size still load: the page reads `chunk` from the parts' `manifest.json`, and `?chunk=` overrides it.
