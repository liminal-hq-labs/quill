# Quill's site

The product page published at <http://labs.liminalhq.ca/quill/>. The organization's Pages sit on a custom domain, so `https://liminal-hq-labs.github.io/quill/` redirects there. Plain HTML, CSS and a little JavaScript, with no framework and no build step: the folder is published as it is.

## Layout

| Path | What it is |
|---|---|
| `index.html` | The page |
| `404.html` | The page GitHub Pages shows for a missing path |
| `css/site.css` | Styles, the light and dark themes (Quill's palette, D70) |
| `try/` | The Try Quill page (`index.html`, `try.js`, `bridge.js`, `licences.html`), the DOS bridge module `try/lib/dos-bridge.js` built from `try/packages/dos-bridge/`, the vendored js-dos player in `try/js-dos/` (see its `PROVENANCE.md`) and, built and not committed, `quill.<hash>.jsdos` and `try-manifest.json` |
| `css/try.css` | The Try Quill page's styles |
| `js/site.js` | The theme toggle and the screenshot lightbox; the page works without it |
| `js/pointer.js`, `js/pointer-metric.js` | The pointer module for the demo pages and its direction-error measure (see *The pointer*); the Try Quill page loads the module |
| `pointer-lab/` | The pointer lab: an unlinked test bench (published at `/quill/pointer-lab/`) that runs the module against a stand-in guest |
| `img/` | The icon (from `assets/icon/`), the link preview `social-card.png` (written by `tools/make_social_card.py`), and `shots/` |
| `favicon.ico` | The favicon: 16, 32 and 48 px from `assets/icon/` |

Every URL in `index.html` is relative, so the page works from `http://labs.liminalhq.ca/quill/`, from the root of any server, from a fork's Pages address and from a local folder. `scripts/check-site.py` resolves each reference the way a browser does below `/quill/` and at the root, and fails if the two reach different files. `404.html` is the exception: GitHub serves it for a missing path at any depth, so its links start with `/quill/`. The Open Graph and canonical URLs have to be absolute, and name the published address.

## The Try Quill page

`try/` runs `QUILLDOS.EXE` on FreeDOS 1.3 in the visitor's browser with js-dos (SITE-1). The disk bundle `try/quill.<hash>.jsdos` and the manifest `try/try-manifest.json` that names it are build outputs, ignored by git (SITE-4: the bundle is named by its content, and the page asks the manifest for the current name): `bash scripts/build-try.sh --build` makes it (the Pages workflow runs the same command), and without `--build` it packs the programs named by `QUILLDOS_EXE`, `QUILL_FNT` and `QUILL_DICT`. It needs mtools and, with `--build`, the pinned Rust toolchain. `bash scripts/stamp-try.py` writes the `?v=` versions of the page's own files into `try/index.html` (a build does too), which is committed; `bash scripts/check-try.sh` tests the scheme. Without the bundle the page loads and Start reports that the disk could not be fetched.

The player is loaded only when Start is selected, and makes no request outside the site. After updating `try/js-dos/`, open the page with the network panel open and start it. `scripts/check-site.py` does not scan the vendored player.

## The DOS page's disks

The DOS page reaches its guest through two small hard disks that the page and FreeDOS share (SITE-60; the plan is [docs/plans/dos-page-bridge.md](../docs/plans/dos-page-bridge.md)), because js-dos has no serial link or shared folder to use. `C:` (`c.img`, 8 MB, FAT12, label `QUILLDOCS`) is the guest's: only DOS writes it, and the page only reads its image. `D:` (`d.img`, 32 MB, FAT16, label `FROMPC`) is the page's: only the page writes it, rebuilding it whole, and DOS only reads it. Each disk has one writer, so no protocol is needed. Both images are in the bundle, empty (`scripts/make-bridge-disks.py` makes them), and `scripts/try-dosbox.conf` mounts them before FreeDOS boots.

Each side has to be told about the other's writes, because each caches. Before DOS reads `D:` it resets its disks (INT 21h function 0Dh), or it answers from its buffers; after DOS writes `C:` it resets and then reads one sector of the disk (INT 13h), or the last sector it wrote stays in the emulator's buffer and the page reads zeros. `SYNC` on `A:\` does both, and the start-up batch files run it. At the prompt, run `SYNC` after copying a file to `C:`, and before listing `D:` after the page has changed it.

What the page puts on `D:` today is `SETDATE.BAT`, which sets FreeDOS's date to the visitor's (it was 25 July 1998 on every start). `bridge.js` writes it when the player is ready, before FreeDOS's start-up menu is answered, and the batch files call it if it is there. A line under the screen shows the state (`bridge: on, D: written 47 ms, date 12-25-2026`). `?bridge=off` turns the bridge off: `bridge.js` does nothing and the module is not fetched (the disks stay mounted, and the date is 1998 again).

`try/packages/dos-bridge/` is the page's side, a TypeScript package laid out like `host-link` and `ac97` (`fat.ts`, `names.ts`, `disks.ts`, `index.ts`); `bun run build` writes `try/lib/dos-bridge.js`, which is committed, and `bun run check:built` fails on drift. `try.js` has one line for the bridge, on `ci-ready`. `scripts/stamp-try.py` stamps `bridge.js` and the module (through the script tag's `data-module`) with `?v=` like the page's other files.

```sh
cd site/try/packages/dos-bridge && bun install --frozen-lockfile && bun test && bun run typecheck && bun run check:built
python3 -I tests/dos-bridge/make-goldens.py    # rewrites the FAT12 and FAT16 images the tests read (needs mtools)
bash scripts/build-try.sh                      # the bundle, with the two disks (needs the programs, or --build)
cd site/try-reactos/harness && bun install --frozen-lockfile   # once
env -u WAYLAND_DISPLAY DISPLAY=:97 PLAYWRIGHT_SKIP_BROWSER_GC=1 PORT=18431 PIDFILE=/tmp/dos-bridge.pid bun check-dos-bridge.ts /tmp/dos-bridge-out
```

The check starts its own server on 127.0.0.1, runs the page in headless Chromium and Firefox in each display mode with the browser's clock fixed to 25 December 2026, and fails unless `DATE` says so, a file the page writes on `D:` is in DOS after `SYNC`, a file DOS writes on `C:` is read by the page after `SYNC`, `?bridge=off` gives 25 July 1998, and every request is a plain GET to the server. Set `BASELINE_SITE` to a site folder built from `main` to compare Quill's first screen with `?bridge=off` pixel for pixel.

## The pointer

`js/pointer.js` is one pointer module for both demo pages (SITE-20): a classic script that sets `QuillPointer`, with the logic in `QuillPointer.core` (no DOM, tested in Node) and `QuillPointer.attach` binding it to a stage element and an emulator adapter (`send.move`, `send.buttons`, `send.wheel`). **Follow** is the default: the guest pointer is moved once an animation frame to the guest pixel under the visitor's pointer, worked out from the event's fractional `clientX` and `clientY`, so nothing is lost to the whole-number `movementX` that Firefox on Wayland reports from a locked pointer. **Capture** (the pointer lock, `movementX` summed per frame) is an option, and the default only where `requestPointerLock({ unadjustedMovement: true })` succeeds. A finger is a trackpad, a pen follows, and wheel deltas are accumulated into notches. `docs/plans/pointer.md` has the design. The Try Quill page uses it (SITE-22).

**On Try Quill** the page owns the pointer. js-dos is started with `mouseCapture: false`, the stage stops its mouse, pointer and touch events in the capture phase (the module's `stopEvents`), and `try.js` gives the module an adapter: `ci.sendMouseRelativeMotion` for moves, `ci.sendMouseButton` for left (0) and right (1), no wheel (js-dos 8.5.3 cannot send one). The units are CuteMouse's: the disk's copy of the driver has its speed-dependent acceleration switched off by `scripts/patch-ctmouse.py` (two bytes; without it a count moves the pointer 0.4 to 1.3 pixels depending on how many counts DOSBox joins into one report, and the page cannot know), so with `/R13` one count is 111/256 of a pixel across and 193/256 down at any speed (measured over 1200 and 400 counts), and the page sends 256/111 and 256/193 counts a pixel. In text mode the pointer's range is a virtual 640 by 200 over the whole 4 by 3 screen, so the module is given a 640 by 480 picture with the vertical units scaled by 200/480. The pointer is parked once at the start and when the guest's display size changes (Quill starting recentres it). The **Mouse** control under the screen switches Follow and Capture; the *Pointer speed* slider shows only where it acts (Capture, a finger); `?mouse=follow|capture` and `?speed=N` set a visit; `?pointer=debug` shows a line of the module's state and a *Save pointer trace* button whose file `scripts/pointer/score-trace.mjs` scores. When updating `try/js-dos/`, check that js-dos still listens for the mouse, pointer and touch events on its own elements in the bubble phase (the stage's capture-phase stop depends on it) and that `mouseCapture` and `setMouseCapture` still exist.

`js/pointer-metric.js` is the measure (SITE-21): the direction error of the guest pointer's path against the visitor's over windows of 8 guest pixels, the share of windows that moved on one axis only, and the end point. `scripts/pointer/score-trace.mjs FILE...` scores a trace saved by the lab or by a page with the trace button.

`pointer-lab/` is not linked from the site's pages. It draws the module's stand-in guest (640 by 480) on a canvas, switches Follow and Capture and the guest's units, reads out the mode, the raw-capture answer and the share of fractional positions, scores the last stroke and saves a pointer trace. `?mouse=follow|capture`, `?speed=N` and `?lockless=1` (Capture without a lock, for tests that dispatch movement) change its start.

```sh
node --test scripts/pointer/*.test.mjs                         # the model, the matrix and the stage binding on a stand-in browser
node scripts/pointer/serve.mjs                                 # site/ on a free port of 127.0.0.1 (PORT, PIDFILE)
cd site/try-reactos/harness && bun install --frozen-lockfile   # once
env -u WAYLAND_DISPLAY DISPLAY=:99 node pointer-lab.ts         # the lab in headless Chromium and Firefox, exits 1 on a miss
env -u WAYLAND_DISPLAY DISPLAY=:99 node try-pointer.ts         # the Try Quill page and the real Quill (--browser chromium,firefox; needs a local bundle: bash scripts/build-try.sh --build)
env -u WAYLAND_DISPLAY DISPLAY=:99 node jsdos-units.ts         # js-dos's pointer units, per display mode (the findings are in docs/plans/pointer.md)
```

The browser run needs Playwright's Firefox (`PLAYWRIGHT_SKIP_BROWSER_GC=1 node node_modules/playwright-core/cli.js install firefox`, so the installer keeps the other cached builds) and Chromium at `/usr/bin/chromium`. Capture under the Firefox-on-Wayland model is expected to fail the pass lines; the run fails if it does not.

## Screenshots

The files in `img/shots/` are real captures of Quill, copied from `docs/media/` (where `docs/media/README.md` says where each was taken), reduced to 256 colours and run through `optipng`. The Windows 3.11 captures come from the play harness (`win16/tools`, `docs/play.md`) and the Windows 95 ones from Liminal Installer's test bed (`scripts/win95`), each on a private display. Each caption on the page says where its screenshot was taken. Add a screenshot only if it shows Quill running with nothing else of anyone's in frame (no other program's icons, no product key, no personal name such as a registered owner on a Windows system screen), and retake it when the program it shows has changed.

## Checks

```sh
python3 -I scripts/check-site.py   # every link, image, script and stylesheet in site/ resolves
bash scripts/check-headers.sh
docker run --rm -v "$PWD:/repo" -w /repo rhysd/actionlint
```

To look at it locally the way Pages serves it, put the folder under a `quill` path and start a static server:

```sh
mkdir -p /tmp/pages && ln -sfn "$PWD/site" /tmp/pages/quill
python3 -m http.server 8000 --directory /tmp/pages   # then open http://localhost:8000/quill/
```

## Publishing

`.github/workflows/pages.yml` checks the links (fetching the outside ones), uploads `site/` and deploys it when a change under `site/` (or the workflow itself) reaches `main`, or when it is run by hand from the **Actions** tab. The repository's Pages setting is **Source: GitHub Actions** (build type `workflow`), and the `github-pages` environment the workflow uses is created on the first run. The address comes from the organization's custom domain; nothing in this folder names it except the canonical and Open Graph URLs in `index.html`.
