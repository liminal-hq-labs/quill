# Try Quill on ReactOS

ReactOS 0.4.16 with Quill, running in [v86](https://copy.sh/v86/) (an x86 emulator in WebAssembly) inside a browser tab. The page makes no request outside its own origin.

## Layout

| Path | What |
|---|---|
| `index.html`, `try-reactos.js`, `try-reactos.css` | The page: Start, Stop, Full screen, a status line and a builder and diagnostics panel (RAM, VGA memory, ACPI, JIT, boot order, disk, disk chunk size, CD and state URLs, a live read-out of IDE reads, video mode and MIPS, Save state) |
| `v86/` | v86 0.5.473 and its BIOS files, fetched by `tools/fetch-v86.sh` (not committed). `v86/PROVENANCE.md` lists versions, hashes and licences (v86 is BSD-2-Clause; SeaBIOS and SeaVGABIOS are LGPL-3.0) |
| `harness/serve.ts` | The local server: static files, HTTP range requests, per-client byte counters, a JSON request log |
| `harness/measure.ts`, `lib.ts`, `inpage.js` | The measurement run, on `playwright-core` driving the installed Chromium |
| `harness/watch.ts`, `explore.ts`, `build-state.ts`, `run-all.sh`, `summarise.ts` | Screenshots on a timer, key-sequence exploration, saved-state builder, the full measurement set, the results table |
| `tools/fetch-v86.sh` | Downloads the pinned v86 release, checks its SHA-256 sums and unpacks the emulator and BIOS files into `v86/` |
| `tools/make-parts.mjs` | Splits a raw disk image into zstd parts for v86's chunked disk |
| `demo/` | The pitch document Quill opens: `demo.rtf` and `demo.qld`, and `demo16.*` for the Windows 3.x icon; `make-demo.sh` rebuilds them |

The disk images, CDs, states and parts live outside git, in `~/.cache/reactos/web/` (the server maps it to `/images/`; override with `IMAGES_DIR`). Never copy them into the repository.

## Run it

Needs Bun or Node 22+, and Chromium at `/usr/bin/chromium` (no browser is downloaded).

```sh
bash site/try-reactos/tools/fetch-v86.sh   # once: the emulator and BIOS files are not committed
cd site/try-reactos/harness
bun install                      # playwright-core only
bun serve.ts                     # http://127.0.0.1:8086/try-reactos/  (PORT, IMAGES_DIR, COI=1, LOG_FILE)
```

Then open `http://127.0.0.1:8086/try-reactos/?disk=/images/reactos16-spike-a.img&cdrom=/images/quill-demo.iso&boot=hd&ram=256` and select Start. Every field in the panel can be set from the address bar by its name (`ram`, `vga`, `acpi=0`, `jit=0`, `cpuid`, `boot=auto|cd|hd`, `disk`, `chunk`, `cdrom`, `state`, `run`).

1. **Smoke test, the live CD:** `?cdrom=/images/livecd.iso&boot=cd` boots the ReactOS 0.4.16 installer. Or `node watch.ts --name livecd --seconds 90 --param cdrom=/images/livecd.iso --param boot=cd`; frames go to `out/livecd/`.
2. **Spike A, the copied disk:** `node measure.ts --name spike-a --app quill32` (or `--app quill16` for the Win16 build, `--profile 50mbit|10mbit`).
3. **Make a saved state:** `node build-state.ts --name state-q32`, then `zstd -19 -T0 ~/.cache/reactos/web/state-q32.bin -o ~/.cache/reactos/web/state-q32.l19.bin.zst`.
4. **Make disk parts:** `node tools/make-parts.mjs IMAGE ~/.cache/reactos/web/parts/spike-a 1048576 19`.
5. **Restore from a state:** `node measure.ts --name restore --param state=/images/state-q32.l19.bin.zst` (add `--param disk=/images/parts/spike-a/.img.zst --param chunk=1048576` for the chunked disk).
6. **The full set and the table:** `bash run-all.sh`, then `node summarise.ts`.
7. **Demo documents:** `demo/make-demo.sh "<path>/quill-document.svg" <path>/target/release/quill` (build the CLI with `cargo build --release -p quill-cli`).

Server endpoints: `GET /__stats` (bytes and requests per client, split into page, engine, bios, disk, cdrom, state; distinct disk bytes), `POST /__stats/reset`, `POST /__stats/flush` (writes `out/requests.json`), `POST /__put/NAME?offset=N` (stores a body in the images folder). A client is the `run` id in the page's address.

Input reaches the guest only through the page's own emulator API (`keyboard_send_scancodes`, `keyboard_send_text`, the mouse bus); the headless browser never touches the host desktop.

## Results

Measured on 2026-11-25 in headless Chromium 156, on a machine shared with other builds (load average 12 to 38 on 16 threads), so treat times as a range. The full tables, caveats and recommendations are in the results note.

| Measure | Result |
|---|---|
| Spike A: the QEMU image in v86 (256 MB, ACPI on) | Boots unchanged to the desktop; 800 × 600 × 32; no prompts; both Quill builds start |
| Cold boot to the desktop, whole-file disk | 21 to 27 s unthrottled; 75.8 MB served; 156 s at 50 Mbit/s, 322 s at 10 Mbit/s |
| Cold boot, 1 MiB zstd parts | 40.0 MB served, 155 requests; 48 s at 50 Mbit/s |
| Saved state | 132 MB raw, 22.2 MB at `zstd -19` (27.6 at -9, 32.5 at -3) |
| Restore to a live desktop with Quill open | 0.5 s unthrottled, 4.8 s at 50 Mbit/s, 20.8 s at 10 Mbit/s; 24.7 MB served; 0 disk bytes |
| Typing in `QUILL32.EXE` | Cold: median 80 ms, 95th percentile 100 to 113 ms. After a restore: unresolved under load (49 to 300 ms median) |
| Typing in `QUILL.EXE` (WineVDM) | Median about 1 s |
| Scrolling | 10 to 20 changed frames a second |
| Renderer memory | 580 to 1,300 MB |
| ACPI off | Does not boot |

`node harness/summarise.ts` prints every run in `out/results/` as a table.
