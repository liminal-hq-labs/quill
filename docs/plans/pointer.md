# The pointer on the web pages: follow by default, capture as an option

A plan for one pointer module shared by the Try Quill page (`site/try/`, DOSBox in js-dos) and the ReactOS page (`site/try-reactos/`, v86), after the owner's report of 2026-11-27: in Firefox 159 on a Wayland session at 1.5 times scaling with a low-DPI mouse, the guest pointer snaps to horizontal and vertical moves on both pages. Written on 2026-11-27 against `main` at `992797a`, js-dos 8.5.3 and v86 0.5.473 as vendored and fetched. Where this plan and [decisions.md](../decisions.md) disagree later, the decisions win.

## Table of contents

- [Recommendation](#recommendation)
- [What was measured](#what-was-measured)
- [What the pages do today](#what-the-pages-do-today)
- [The design](#the-design)
- [Measuring it](#measuring-it)
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
| Where the code lives | **One module, `site/js/pointer.js`**, with an adapter per emulator: v86's bus on the ReactOS page, js-dos's `ci.sendMouseRelativeMotion` and `ci.sendMouseButton` on Try Quill. The page owns every pointer event; neither emulator's own mouse handler runs | Both pages have the same problem and the same guest model (a relative PS/2 mouse that clamps at the edges) |
| Default mode | **Follow**: no pointer lock; the guest pointer is moved, once an animation frame, to the guest pixel under the visitor's pointer, worked out from the event's `clientX` and `clientY`, which are fractional where the browser has them. The visitor's pointer is hidden over the picture | Positions keep the sub-pixel motion that Firefox on Wayland throws away from a locked pointer's `movementX` and `movementY` |
| Capture | **An option on both pages**, and the default only where `requestPointerLock({ unadjustedMovement: true })` succeeds. That is the "raw supported" test; it is answered by the browser on the first click and remembered | Raw counts are the one case where capture is as good as the hand. Chromium on Linux refuses raw as Firefox does (measured), so a Linux visitor gets Follow in either |
| Whole-number movement | Detected while captured (every movement an integer at a non-integer `devicePixelRatio`) and used only for a hint ("Follow is smoother here") and the diagnostics, not for the default | Integers alone do not prove a loss: Chromium also reports integers and keeps the remainder |
| Coalesced samples | **Not needed for Follow**: the frame's last position is the whole answer, because one packet goes out a frame. Capture keeps counting every coalesced sample, where the browser offers them | `getCoalescedEvents` exists only in a secure context (measured in both browsers), and the published site is HTTP only |
| Pointer speed slider | Shown in Capture and for touch, hidden in Follow, where the guest pointer is under the visitor's by construction. The ReactOS page gains the same slider | A slider that does nothing is worse than none |
| Fractions | Kept by the page on both pages, which sends whole counts only | v86 would keep them too, but then sends a packet per call; DOSBox may not (WP-PTR-2 finds out); the page's model needs whole counts anyway |
| Measurement | **Direction error over short windows of travel** (8 guest pixels) against a known path, at three levels: the module alone in Node, the module and the real emulator bus in headless Chromium and Firefox, and the guest's own pixels | The one-axis percentage counts slow true diagonals as failures and fast snapped ones as passes |

Build order: WP-PTR-1 (the module, its lab page and its tests) and WP-PTR-2 (the js-dos units spike) together; then WP-PTR-3 (Try Quill) and WP-PTR-4 (ReactOS) together; WP-PTR-5 (the owner's hardware check and the tuning it asks for) last.

## What was measured

### The owner's report

Firefox 159 (its user agent says X11; the session is Wayland), `devicePixelRatio` 1.5, a low-DPI mouse, on the owner's desktop:

- Under pointer lock, `movementX` and `movementY` are whole numbers only; slow moves snap to horizontal and vertical.
- `requestPointerLock({ unadjustedMovement: true })` is refused, so a page that asks only for raw never gets the lock.
- Plain `pointermove` positions are fractional in 87 to 89 percent of samples, with about 7 coalesced samples per event, and a line drawn from them is smooth.

### Probes for this plan

Headless, on this machine, `deviceScaleFactor` 1.5, through Playwright 1.65.0 (the harness's pin) with Chromium 156 (`/usr/bin/chromium`) and Playwright's Firefox 159 build. `page.mouse.move` from (100.3, 100.7) to (400.6, 250.2) in 60 steps, then a click that asks for a raw lock. The scripts and their output are in `~/Documents/Liminal HQ/Quill/Web demo/pointer/plan-evidence/`.

| | Chromium 156 | Firefox 159 |
|---|---|---|
| `clientX`/`clientY` of a synthetic move | Fractional (61 of 61) | Whole (0 of 61): Playwright's Firefox driver rounds the position |
| `movementX`/`movementY` | Whole, as the difference of rounded positions (5, 3, 5, 2 …: the remainder is kept) | Whole |
| `getCoalescedEvents` from `http://127.0.0.1` (a secure context) | Present (1 sample a synthetic event) | Present |
| `getCoalescedEvents` from `about:blank` (not a secure context) | **Absent** | **Absent** |
| `requestPointerLock({ unadjustedMovement: true })` | Rejected, `NotSupportedError` | Rejected, `NotSupportedError` |
| A plain pointer lock, headless | `pointerlockerror` | `pointerlockerror` |

Consequences: headless Firefox cannot make trusted fractional moves, so Firefox runs use dispatched `PointerEvent`s with fractional `clientX`; neither headless browser locks, so capture is tested with dispatched `movementX` as `harness/mouse-axis.ts` already does; and the published pages (`http://labs.liminalhq.ca/quill/…`, HTTP only, see [web-demo.md](web-demo.md#hosting-headers)) never see coalesced samples.

### The emulators, from their own files

- **v86 0.5.473** (`libv86.mjs`, SHA-256 `f9f3b4ca…`, as `tools/fetch-v86.sh` pins it): `mouse_send_delta(a, b)` adds `1*a` and `1*b` to `mouse_delta_x` and `mouse_delta_y`, sends `mouse_delta_x | 0` and `mouse_delta_y | 0` in one packet at once, and keeps the remainder. So v86 keeps fractions itself, with no scale factor, and makes one PS/2 packet per call that has a whole count in it.
- **js-dos 8.5.3** (`try/js-dos/js-dos.js`): with `mouseCapture` on, a press on the screen asks for `requestPointerLock({ unadjustedMovement: true })`, and when that is refused it logs "Failed to request pointer lock with unadjusted movement, falling back to default" and takes a plain lock. Each locked `pointermove`'s `movementX` and `movementY` are cut to ±50 and scaled by `8^(2t-1)` (`t` is the sensitivity the page sets), then sent one call per event. So on the owner's Firefox the plain lock delivers whole-number moves, and the Try Quill page snaps exactly as the report says. With `mouseCapture` off, js-dos sends absolute positions (`sendMouseMotion`), which SITE-5 measured drifting through a PS/2 driver. js-dos listens on its own overlay element in the bubble phase, so a capture-phase listener on the page's stage that stops propagation keeps every mouse, pointer and touch event from it (the page already does this for touch). The returned object has `setMouseCapture` and `setNoCursor`, and the `mouseCapture` option overrides `autolock=true` from the bundle's `dosbox.conf` when the bundle is ready.
- **js-dos has no wheel.** Nothing in the player or the emulator files sends a wheel event, so Quill's wheel support (CuteMouse's wheel API) cannot be reached from js-dos 8.5.3.
- **DOSBox takes fractions.** The wasm export behind `Module._mouseMove` (`Ga` in `wdosbox.wasm`) takes `(f32, f32, i32, i32, i32)`: x and y arrive as floats. Whether DOSBox's PS/2 path keeps the fraction or drops it is not known (WP-PTR-2).
- **Quill DOS** (`crates/quill-mouse`) sets the driver's range to its own screen, sets 8 mickeys per 8 pixels on both axes and puts the pointer in the centre when it opens. So the guest's position is reset at every start of Quill, which the page cannot see.

### A model of the loss

A 60-line model (`sim.py` in the evidence folder; WP-PTR-1 replaces it with `scripts/pointer/`) of a straight hand movement reported at 1000 Hz, in logical pixels per millisecond, scored by the direction error defined under [Measuring it](#measuring-it): (a) capture, with each report cut to whole device pixels at scale 1.5 and a report with both axes 0 dropped (the behaviour `widget/gtk/nsWindowWayland.cpp` has, as `site/try-reactos/README.md` records), summed per 16.7 ms frame; (b) follow, the frame's last position rounded to a guest pixel.

| Angle, speed | Capture under Firefox on Wayland: median, p95 error, windows on one axis only | Follow: median, p95 error, one axis only |
|---|---|---|
| 27°, 0.4 px/ms | 90°, 90° (the pointer does not move: each report is under one device pixel) | 0.4°, 3.3°, 0 % |
| 27°, 0.8 px/ms | 27°, 27°, **100 %** (pure horizontal) | 0.4°, 3.3°, 0 % |
| 10°, 0.8 px/ms | 10°, 10°, 100 % | 1.9°, 3.0°, 0 % |
| 45°, 0.1 px/ms | 90°, 90° (no movement) | 0°, 0°, 0 % |

Follow's worst p95 over 10°, 27° and 45° at 0.1 to 0.8 px/ms is 6.4°, which is the guest's one-pixel grid at an 8-pixel window. The report rate matters: the slower the reports, the larger each one, and the less Firefox throws away, which is why a low-DPI mouse is hit and a fast one less so.

## What the pages do today

- **ReactOS page** (`site/try-reactos/try-reactos.js`, the block from "The pointer." to `hookPointer`): v86's own mouse adapter is off (`disable_mouse: true`). Two modes: *lock* (`?mouse=lock`, the default elsewhere) and *follow* (`?mouse=follow`, the default when the user agent says Firefox on Linux, since `9ec0b12`). Follow moves the guest pointer to the guest pixel under the system pointer (`followTo`, `Math.round`), parks the guest pointer in the top-left corner when the screen is selected and on every re-entry, and hides the system pointer over the stage. Lock counts coalesced moves, scales by the shown size and `?speed` (1.5), carries the fraction, and sends one packet a frame. Touch: drag moves, a short tap clicks. Wheel: one notch per `wheel` event. `?cursor=local` draws a predicted arrow (off by default).
- **The report says the ReactOS page snaps too.** Follow should not snap on the owner's Firefox, and `harness/mouse-axis.ts` showed it landing on target, so this is not explained. Candidates, in order: the copy of the page the owner used predates `9ec0b12` or had `?mouse=lock` in the address; the per-frame staircase of a 1024 by 768 guest pointer drawn at 1.5 device pixels a guest pixel reads as snapping at very slow speeds (it is one pixel at a time, not runs); something in the restored guest (acceleration was measured off). WP-PTR-1 adds a mode line and a trace to the diagnostics, and the first item of the owner's check list settles it.
- **Try Quill page** (`site/try/try.js`): js-dos's own pointer lock (`autolock=true` in `scripts/try-dosbox.conf`, so `mouseCapture` is on), with the sensitivity set from the shown size and the *Pointer speed* slider (`fitPointer`). Touch has the page's own trackpad (`onTouch`: drag, tap, hold then drag), sent as `ci.sendMouseRelativeMotion` with measured units of about 1.37 by 1.1 screen pixels each. On Firefox the lock's movements are whole numbers, so it snaps; there is no follow mode.

## The design

### One module, two adapters

`site/js/pointer.js` is a classic script (no build step, no import map) that sets `globalThis.QuillPointer`. It loads before each page's own script: `<script src="../js/pointer.js?v=…" defer>` in `try/index.html`, where `scripts/stamp-try.py` already stamps every `?v=` reference, and the same tag in `try-reactos/index.html`, which WP-WEB-1's stamp covers once it lands. It has two parts:

- `QuillPointer.core`: pure functions and small state machines with no DOM: the follow mapper, the capture accumulator, the counts splitter, the wheel accumulator, the touch gesture machine. Node's test runner loads the file and tests these.
- `QuillPointer.attach(options)`: binds the core to a stage element and an adapter, and returns a handle.

The contract between the module and the pages (WP-PTR-1 implements it; WP-PTR-3 and WP-PTR-4 consume it):

```js
const handle = QuillPointer.attach({
	stage,                        // the element the visitor points at; listeners go here, in the capture phase
	picture: () => ({ el, w, h }),// the element the guest's picture is drawn in and the guest's size in pointer units
	units: { x: 1, y: 1 },        // guest counts per guest pixel, per axis (v86: 1; js-dos: from WP-PTR-2, per display mode)
	send: {
		move(dx, dy) {},          // whole counts, y down; called at most once an animation frame with both axes
		buttons(left, middle, right) {},
		wheel(notches) {},        // or null when the emulator has no wheel
	},
	maxCount: 120,                // largest count one call may carry; longer moves are split (v86's packet byte)
	parkOnEnter: false,           // park when a mouse enters the picture again (js-dos: true until WP-PTR-2 says the units are exact)
	defaults: { mode: "auto", speed: 1 },
	storageKey: "quill-pointer",  // localStorage prefix; every read and write is guarded
	onState(state) {},            // { mode, raw, wholeOnly, following, captured, secure, coalesced, fractionalShare, packetsPerFrame }
	onSelect() {},                // the first press in the picture since a press elsewhere (the page gives it the keyboard)
});
handle.setMode("follow" | "capture" | "auto");
handle.setSpeed(n);
handle.park();                    // after a guest resolution change or a restart
handle.trace.start(); handle.trace.stop(); // -> a JSON-ready object, see "Diagnostics and traces"
handle.detach();
```

The adapters are a few lines each. **v86:** `move` is `emulator.bus.send("mouse-delta", [dx, -dy])`, `buttons` is `mouse-click`, `wheel` is `mouse-wheel`; units 1 (one count is one guest pixel, measured), `maxCount` 120; `picture` is the v86 canvas and its `width` and `height`. **js-dos:** `move` is `ci.sendMouseRelativeMotion(dx, dy)`, `buttons` maps left to 0 and right to 1 (`ci.sendMouseButton`; js-dos has no middle button), `wheel` is `null`; the units come from WP-PTR-2 for each display mode, and `picture` gives the shown canvas with the pointer's coordinate range for the mode (640 by 480 in both graphics modes; in text mode the driver's virtual 640 by 200, which Quill divides by its 8 by 8 cells).

### Modes and how the default is chosen

| Mode | What it does | Default when |
|---|---|---|
| **Follow** | Guest pointer under the visitor's pointer, from positions; no lock | Always to begin with, and stays the default unless raw capture is known to work |
| **Capture** | The page's own pointer lock on the stage; moves from `movementX` and `movementY`, scaled by the shown size and the speed slider | `requestPointerLock({ unadjustedMovement: true })` has succeeded in this browser (remembered) |
| **Touch** | A trackpad: relative moves from a finger, taps and holds for buttons | The pointer is a finger (`pointerType` `touch`), whatever the mode |
| `?mouse=v86` (ReactOS only) | v86's own adapter, the module not attached | Never; for comparing |

**Auto** is the stored default. With no stored answer about raw, the page follows on hover from the start. The first press in the picture asks for a raw lock: if the promise resolves, the page is in Capture from that press on and stores `raw=yes`; if it rejects (Firefox, and Chromium on Linux, as measured), the page stays in Follow, stores `raw=no`, and the press goes to the guest as a click. A browser whose `requestPointerLock` returns no promise is treated as `raw=no`, and a lock it may have taken is released at once. The stored answer carries the user agent string and is asked again when that changes. A visitor's explicit choice of Follow or Capture is stored and wins over Auto. `?mouse=follow|capture` (and the old `lock`) overrides for the session, so the harness and the owner can force either.

Chromium-family browsers on Windows, macOS and ChromeOS support raw capture, so the owner's "capture by default on Chromium" holds there; on Linux it does not, and the default is Follow, because Chromium's lock on Linux reports adjusted whole-number moves too.

### Follow in detail

- **The mapping.** For each `pointermove` the module takes `clientX` and `clientY`, subtracts the left and top of the picture as it is shown (the canvas can be letterboxed; the existing `shownRect` arithmetic), scales by the guest's size over the shown size, and takes the guest pixel under it (`Math.floor`; the hotspot is in that pixel's square), clamped to `0 … w-1` and `0 … h-1`. Only the frame's last position matters: one call goes out per animation frame, and the difference between the guest's position and the target telescopes, so the samples in between add nothing. Coalesced samples are recorded in a trace when they exist, and not used.
- **Counts and the model.** The module keeps the guest's position as it believes it to be (whole guest pixels) and, per axis, a count remainder. The frame's move is `(target − position) × units`; whole counts go out, the fraction is carried to the next frame, and the position advances by what was sent divided by the units, clamped as the guest clamps. With v86 the units are 1 and there is never a fraction.
- **The visitor's pointer** is hidden (`cursor: none`) while it is over the picture itself and shown over the letterbox bars and the rest of the page.
- **Edges resync for free.** When the target is clamped on an axis (the visitor's pointer is at or past that edge of the picture), the module sends, once per arrival at the edge, an extra 16 guest pixels' worth of counts outward on that axis. The guest clamps, the model clamps, and that axis is exact again. Touching an edge is common (menus at the top, scroll bars at the right, the taskbar at the bottom), so drift from inexact units cannot build up for long.
- **Leaving.** When the pointer leaves the picture without a button held, the guest pointer stays where the edge put it. With a button held, the press set pointer capture on the stage (`setPointerCapture`), so moves keep arriving outside it: the target is clamped (a drag-select that runs off the bottom keeps the guest pointer on the bottom edge, where Quill and ReactOS scroll), and the release is delivered wherever it happens. `lostpointercapture`, `blur` and `visibilitychange` to hidden release every button in the guest.
- **Re-entering.** The model is still right (every count was sent and every edge resynced), so no park is needed on v86, and the page no longer parks on each re-entry as today's follow mode does (a park sweeps the guest pointer across the screen to a corner, which can raise hover effects). For js-dos, `parkOnEnter` stays on until WP-PTR-2 shows the units are exact.
- **Parking.** On the first follow after a start or a restart (a restored state or Quill's own start puts the guest pointer somewhere the page does not know), after a guest resolution change (`screen-set-size` on v86; a display mode change on js-dos), and on re-entry where `parkOnEnter` is set, and never while a button is held: the module sends counts toward the corner nearest the target (the screen's size plus 16 pixels per axis, split at `maxCount`), sets the model to that corner, and moves to the target in the same frame.
- **Clicks.** A press in the picture goes to the guest at the place the visitor sees. The first press since a press elsewhere also calls `onSelect`, and the page gives the emulator the keyboard, as selecting the screen does today. The press is not swallowed: in Follow the guest pointer is already under the visitor's, so the click lands where it was aimed.

### Capture in detail

- The page's own lock on the stage, for both pages; js-dos's lock is never used (`mouseCapture: false`). The press that takes the lock is not sent to the guest. `Esc` releases it (the browser's rule); on Try Quill the same `Esc` also reaches Quill, as now.
- Moves: every coalesced sample's `movementX` and `movementY` when `getCoalescedEvents` exists, the event's own otherwise; times the guest size over the shown size, times the base gain (Try Quill: 1, ReactOS: 1.5, today's defaults), times the slider; whole counts out once a frame, fraction carried. Raw counts are treated as CSS pixels, so one slider serves both.
- The model follows the counts and clamps, so leaving capture for Follow needs no park.
- **Whole-number detector.** Over the first 128 non-zero samples of a capture, if every value is an integer and `devicePixelRatio` is not, `wholeOnly` is set; the hint line then says "This browser reports captured moves in whole pixels, which can lose diagonals. Follow is smoother here." with a *Use Follow* button. It never switches by itself.

### The Pointer speed slider

One slider on each page (Try Quill keeps its own; the ReactOS page gains the same control), 0.5 to 2 in steps of 0.1, default 1, remembered (guarded). Shown while the mode is Capture or the last pointer was a finger; hidden in Follow, and the hint says why ("In Follow, Quill's pointer stays under yours"). `?speed=` sets it on the ReactOS page, as now. Try Quill's `fitPointer` and its `setMouseSensitivity` call go: the module scales the counts itself and js-dos's sensitivity is used only by its own handler, which no longer runs.

### Touch and pen

- **Touch** stays a trackpad on both pages, moved from touch events to pointer events (`pointerType` `touch`, `touch-action: none` on the picture): a drag moves the guest pointer by the finger's distance times the shown scale, `FINGER_GAIN` 1.4 and the slider; a short tap (under 300 ms, under 10 CSS pixels of travel) clicks; a finger held still for 350 ms and then dragged holds the button (the ReactOS page gains this); a two-finger tap is a right click (new on both). The keys row on Try Quill is unchanged.
- **Pen** (`pointerType` `pen`) follows, whatever the mode, because a pen on a tablet or a screen points at a place: hover moves the guest pointer where the pen reports hover, contact is the left button, the barrel button (`buttons & 2`) the right; the eraser end is ignored.

### Buttons and the wheel

- Left, middle and right go to v86; left and right to js-dos (middle is dropped there: js-dos has no third button). The context menu is suppressed over the picture.
- **Wheel, ReactOS:** while the pointer is over the picture and the screen has been selected (so a visitor scrolling past the demo still scrolls the page), deltas are accumulated in pixels (`deltaMode` lines times 16, pages times the picture's height), one notch per 50 pixels, the remainder kept and dropped after 300 ms without a wheel event, and `preventDefault` on each one used. Today one event is one notch, which a touchpad turns into dozens. The 50 is a starting value: WP-PTR-5 sets it from the owner's mouse and touchpad.
- **Wheel, Try Quill:** none; the page scrolls as usual. js-dos 8.5.3 has no way to send one.

### Where the fractions are kept

- **v86:** it would keep them (see above), but then every call with a whole count in it is a packet, and the guest keeps about 100 packets and drops the rest of a burst (measured, `site/try-reactos/README.md`). The page batches per frame and sends whole counts, so v86's own remainder stays 0. Nothing in v86 changes, and SITE-7's "unchanged from npm" holds.
- **js-dos:** DOSBox gets floats, but whether its PS/2 path keeps the fraction is unknown, and the units are not one count per pixel. The page sends whole counts in DOSBox's units, so the answer does not matter to correctness; WP-PTR-2 still finds it out, because it explains the odd touch units.

### The js-dos side

- `Dos(host, { …, mouseCapture: false })` and `setMouseCapture(false)` if js-dos ever turns it back on: no "click to lock" overlay, and js-dos never asks for a lock. The bundle and its `autolock=true` stay as they are (no new bundle hash for this).
- The stage's capture-phase listeners stop `pointerdown`, `pointermove`, `pointerup`, `pointercancel`, `mousedown`, `mousemove`, `mouseup`, `mouseleave` and the touch events from reaching js-dos, as the page already does for touch pointers. The keyboard is untouched.
- The guest's units per display mode, their linearity, and whether a move is acted on without a lock on a desktop browser (the touch path suggests yes) come from WP-PTR-2.
- If WP-PTR-2 finds CuteMouse's `/R13` is not linear (an automatic resolution that grows with speed would break the model), the fix is the driver's switches in `scripts/try-disk/*.BAT`, which changes the bundle and amends SITE-5; that is an owner decision (below).

### Diagnostics and traces

- **ReactOS:** the diagnostics panel gains a *Pointer* row: mode, raw (yes, no, unknown), secure context, coalesced samples available, share of fractional positions, whole-number movement, `devicePixelRatio`, calls a frame, last park.
- **Try Quill:** `?pointer=debug` shows the same as one line under the screen.
- **Trace:** on both pages, with `?pointer=debug`, a *Save pointer trace* button saves the last 20 seconds as JSON: `{ version: 1, page, userAgent, dpr, secure, mode, units, picture: { shown, guest }, input: [{ t, type, pointerType, x, y, mx, my, buttons, coalesced }], output: [{ t, dx, dy, model: [x, y] }], events: [{ t, kind }] }` (kinds: park, edge, mode, lock, unlock, select, leave, enter). `scripts/pointer/score-trace.mjs` scores a trace with the metric below, so a moment that "feels wrong" on the owner's machine becomes numbers.

## Measuring it

### The metric

The **direction error** compares two paths over the same stretch of time: the input (where the visitor's pointer went, in guest pixels: the positions given to the browser, or the true hand path before a simulated quantization) and the output (where the guest pointer went: the counts sent, cumulated and clamped; or the bus; or the guest's pixels).

- At each output frame `i`, walk back to the latest earlier frame `j` at which the input had travelled at least `L` guest pixels before frame `i`. Windows where the input travelled less than `L` in the whole run are skipped.
- `a` is the input's displacement from `j` to `i`, `b` the output's. The error is the angle between them, `atan2(|a × b|, a · b)` in degrees, and 90° when `b` is zero.
- A window is **one axis only** when the input's smaller component is at least one guest pixel and the output's is zero.
- Reported per run: the number of windows, the median and 95th percentile error, the share of one-axis windows, the endpoint error (output minus input at the last frame, in guest pixels), the path ratio (output length over input length) and the calls per frame.
- `L` is 8 guest pixels by default; 4 and 16 are reported as well. A lag of up to two frames between input and output (the guest draws a frame later) is allowed by taking, per run, the lag in 0 to 2 frames that gives the lowest median.

The one-axis percentage of raw events is not used: a slow true diagonal reported at 1000 Hz is mostly one-axis events in any browser, and a fast snapped one can be all two-axis.

### Three levels

| Level | What runs | Where | Input | Output |
|---|---|---|---|---|
| 1. Model | `QuillPointer.core` in Node's test runner (`node --test scripts/pointer/`), no browser | CI, a new *Pointer tests* job on the host runner | Generated paths | The counts the core emits, with a stand-in guest that clamps |
| 2. Page | The lab page (`site/pointer-lab/`) and the real pages, in headless Chromium 156 and Playwright's Firefox 159, through `playwright-core` from `site/try-reactos/harness/` | Local; the lab check can run in CI later | Chromium: trusted `page.mouse.move` with fractional positions. Firefox: dispatched `PointerEvent`s with fractional `clientX` (its driver rounds trusted ones). Capture in both: dispatched `movementX` (neither locks headless) | The adapter's calls (the lab's stand-in guest; v86's bus via `emulator.bus.register` alongside the page's sends; a wrapper around js-dos's `ci`) |
| 3. Guest | The real pages with the real guest | Local, headless Chromium (and Firefox where the emulator runs in it) | As level 2 | The guest pointer found in the guest's pixels (the v86 canvas; js-dos's `ci.screenshot()`), at stops along a path |

**The matrix** (levels 1 and 2): angles 0°, 7°, 27°, 45°, 63°, 83° and 90°, a circle of radius 100 and a figure eight; speeds 0.03, 0.1, 0.4 and 1.5 CSS pixels a millisecond; 125 and 1000 Hz; `devicePixelRatio` 1, 1.25, 1.5 and 2; and three input models for capture: exact, Chromium's (the difference of rounded positions) and Firefox on Wayland (each report cut to whole device pixels, both-zero reports dropped).

**Pass lines.**

- Follow, every cell of the matrix, levels 1 and 2: p95 error at most 10° at `L` = 8, one-axis windows at most 1 %, endpoint within 1 guest pixel.
- Capture with exact or Chromium-style input: the same lines.
- **Capture with Firefox-on-Wayland input must fail them** at 27°, 0.8 px/ms, 1000 Hz, scale 1.5 (the model gives p95 27° and 100 % one-axis). The test asserts the failure, so the metric is shown to see the bug.
- Calls per frame: at most 1 in Follow, at most 2 in Capture (a long move split at `maxCount`).
- Level 3: every stop of a sweep within 2 guest pixels on ReactOS and within 2 pointer units in Try Quill's graphics modes (one cell in text mode); after leaving at one edge and re-entering at another, the guest pointer under the visitor's within the same bounds; no button held after a release outside the picture.

`harness/mouse-axis.ts` keeps its `--check` and gains the direction error for its follow sweep; its lock section stays as the record of the bug.

### The owner's check list

On the owner's desktop (Firefox 159, Wayland, 1.5 times, the low-DPI mouse), then Chromium on the same desktop, then a phone. Each item is a yes or no; any no gets a pointer trace saved into `~/Documents/Liminal HQ/Quill/Web demo/pointer/owner/` with the item's number in the name.

1. Lab page (`/quill/pointer-lab/`, on the published site so the address is HTTP as visitors have it): the readout says mode Follow, raw no, secure no, coalesced no, fractional share above 80 %.
2. Lab page: slow straight strokes at about 10°, 27°, 45°, 63° and 80°: the drawn guest path looks straight, and the readout's p95 error stays under 10°.
3. Lab page: slow and fast circles look round.
4. Lab page: Capture: the hint about whole pixels appears; strokes there do snap (the bug is visible, and Follow is the cure).
5. Try Quill, graphics with 256 colours, Follow: the system pointer is hidden over the screen and Quill's is under where it was; a slow diagonal drag selects diagonally; the menus, the toolbar and the formatting bar are reachable; the right scroll bar's arrows and the status line's zoom box at the edges are reachable.
6. Try Quill: leave the screen to the right while dragging a selection, release outside, come back in at the top: no button stuck; Quill's pointer is under yours again.
7. Try Quill, 16 colours and text mode: the same feel; in text mode the highlighted cell is under the pointer.
8. Try Quill: switch to Capture: `Esc` releases; the Pointer speed slider appears and changes the speed; switch back to Follow: the slider hides.
9. ReactOS page: the diagnostics *Pointer* row says Follow; the taskbar and Start are reachable by pushing past the bottom edge; a slow diagonal across the desktop draws a straight rubber band; one wheel click scrolls Quill or Notepad by one notch, and a two-finger touchpad scroll by a sensible amount.
10. ReactOS page: right click opens the desktop's menu; a drag of a window's title bar off the screen and back leaves no button stuck.
11. Chromium on the same desktop: both pages start in Follow (raw is refused on Linux); Capture works there and does not snap.
12. Phone, both pages: drag moves, tap clicks, hold then drag selects, a two-finger tap opens a context menu on ReactOS; the slider changes the finger's gain.
13. Anything that feels off: save a trace and name the item.

## Decisions to record

The next free decision numbers on `main` are after SITE-19 (SITE-14 to SITE-19 are reserved by [web-demo.md](web-demo.md)). This plan reserves **SITE-20 to SITE-24**; each package writes its own rows when it opens its pull request.

| Id | Decision | Written by |
|---|---|---|
| SITE-20 | One pointer module for both pages; Follow by default; Capture as an option and the default only where raw capture succeeds; Touch and pen as above; the slider only where it acts | WP-PTR-1 |
| SITE-21 | The direction error over 8-pixel windows is the pointer's measure, at the model, page and guest levels, with the pass lines above; the one-axis share of events is not | WP-PTR-1 |
| SITE-22 | Try Quill's page owns the pointer: js-dos's `mouseCapture` off and its handlers stopped, whole counts in DOSBox's measured units, edge resync and park on entry, no wheel. Amends SITE-5's pointer-lock and sensitivity sentences | WP-PTR-3 (with WP-PTR-2's numbers) |
| SITE-23 | The ReactOS page uses the module through v86's bus with v86's adapter off, keeps fractions in the page, no park on re-entry, the wheel accumulated to notches; `?cursor=local` is removed | WP-PTR-4 |
| SITE-24 | Spare, for what the owner's check changes (the wheel's pixels a notch, the gains, any default) | WP-PTR-5 |

## Rules for every package

- **One package, one branch, one worktree** under `~/source/worktrees/quill/<topic>`, from `origin/main`. `~/source/experiments/quill16` is used only for `git fetch` and `git worktree add` and `remove`.
- **Browsers are headless, servers bind 127.0.0.1**, each started by the builder with its PID recorded and stopped by that PID (never `pkill` or `killall` by name). Every command that starts a browser sets `DISPLAY` explicitly and unsets `WAYLAND_DISPLAY` (`env -u WAYLAND_DISPLAY DISPLAY=:99 …`); nothing is ever shown on or typed into the host desktop.
- **Playwright's Firefox** is installed with `PLAYWRIGHT_SKIP_BROWSER_GC=1 node node_modules/playwright-core/cli.js install firefox` from `site/try-reactos/harness` (about 114 MB into `~/.cache/ms-playwright`). Without that variable the installer deletes other cached browser revisions it thinks are unused.
- **Shared files.** `site/js/pointer.js` belongs to WP-PTR-1. WP-PTR-3 and WP-PTR-4 may change it only additively (a new adapter option, a fixed bug with its test), each such change listed in their pull request; the second to merge rebases. `site/try-reactos/try-reactos.js` is also WP-WEB-1's: WP-PTR-4 replaces only the pointer block (from the "The pointer." comment to the end of `hookPointer`, the `pointer` export in `window.tryReactos`, the click handler on the stage and `disable_mouse`'s comment), and whichever of the two merges second rebases. `harness/lib.ts` is not touched by any pointer package; scripts launch their own browsers.
- **No publishing.** No release, tag, upload or Pages dispatch; push only the package's branch.
- **Evidence** into `~/Documents/Liminal HQ/Quill/Web demo/pointer/<package>/`; the few shots that document a feature go under `docs/media/` as `web-pointer-*.png`.
- **Gates.** `bash scripts/check-headers.sh`, markdownlint in Docker, `python3 -I scripts/check-site.py`, `node --test scripts/pointer/`, `python3 -I scripts/stamp-try.py --check site` when `site/try/` or `site/js/` changes, and the package's own checks.

## Order and parallelism

| # | Package | Starts when | Runs beside |
|---|---|---|---|
| 1 | **WP-PTR-1** The module, the lab page, the metric and its tests | Now | WP-PTR-2 |
| 2 | **WP-PTR-2** Spike: js-dos's units, linearity, fractions and unlocked moves | Now | WP-PTR-1 |
| 3 | **WP-PTR-3** Try Quill on the module | WP-PTR-1 merged and WP-PTR-2's numbers in | WP-PTR-4 |
| 4 | **WP-PTR-4** ReactOS on the module | WP-PTR-1 merged | WP-PTR-3, and any WP-WEB package |
| 5 | **WP-PTR-5** The owner's check and the tuning it asks for | WP-PTR-3 and WP-PTR-4 merged, the owner has run the check list | — |

Two builders for the first wave and two for the second, inside the cap of three, leaving a slot for the web demo's packages. If WP-PTR-2 shows js-dos's units cannot be made exact, WP-PTR-3 still ships Follow with `parkOnEnter` on and edge resync, and the README says the pointer can drift by a few pixels between edges.

## Work packages

### WP-PTR-1 The module, the lab page, the metric and its tests

- **Finish line.** `site/js/pointer.js` implements the contract under [One module, two adapters](#one-module-two-adapters), with Follow, Capture (raw probe, whole-number detector), Touch and pen, buttons with pointer capture, the wheel accumulator, parking, edge resync, the diagnostics state and the trace. `site/js/pointer-metric.js` implements the metric. `site/pointer-lab/index.html` (unlinked from the site's pages, with its own title and headings so `check-site.py` passes) runs the module against a stand-in guest of 640 by 480 drawn on a canvas, with switchable units (1 by 1, and 1.37 by 1.1 as a js-dos-like case), shows the readout and the live metric of the last stroke, draws the input and guest paths, and saves traces. `scripts/pointer/score-trace.mjs` scores a saved trace. The level 1 matrix passes in Node, including the asserted failure; `site/try-reactos/harness/pointer-lab.ts` runs the lab page in headless Chromium and Firefox and passes level 2's lines for the lab's stand-in guest in both, and exits non-zero otherwise. No page uses the module yet.
- **Files owned.** New `site/js/pointer.js`, `site/js/pointer-metric.js`, `site/pointer-lab/index.html` (and `pointer-lab.js`, `pointer-lab.css` if wanted), new `scripts/pointer/` (`pointer.test.mjs`, `metric.test.mjs`, `paths.mjs` for the generated paths and the input models, `score-trace.mjs`), new `site/try-reactos/harness/pointer-lab.ts`, `.github/workflows/ci.yml` (one new *Pointer tests* job: checkout and `node --test scripts/pointer/` on the host runner), `site/README.md` (a short *The pointer* section and the lab page's row), `docs/decisions.md` (SITE-20, SITE-21).
- **Must not touch.** `site/try/**`, `site/try-reactos/try-reactos.js`, `index.html` files of either page, `harness/lib.ts`, `harness/mouse-*.ts`, `scripts/build-try.sh`, `scripts/try-disk/**`, `crates/**`, `dos/**`.
- **Dependencies.** None.
- **Tests.** Level 1: the matrix in [Measuring it](#measuring-it) for Follow and Capture; the asserted failure; the counts splitter (no call over `maxCount`, sum exact); the remainder (a million random fractional moves sum to within one count); clamping and edge resync (the model equals the stand-in guest after any path that touches an edge, even with units off by 3 %); parking (the model equals the guest after a park from any place); buttons (press, leave, release outside, `lostpointercapture`, `blur`: no button left down); the wheel (lines, pixels, pages, a touchpad's 120 small deltas, the 300 ms reset); touch (tap, hold then drag, two-finger tap, a drag below the tap distance is a tap); pen (hover, contact, barrel); the raw probe's three outcomes (resolves, rejects, no promise) with a stub `requestPointerLock`; the stored answer and its user agent check; the whole-number detector (integers at 1.5 set it, at 1 and 2 do not, one fraction clears it). Level 2 in `pointer-lab.ts`: both browsers, Follow over the matrix's angles at two speeds and two scales, Capture with dispatched exact and Firefox-style movement, hidden pointer over the picture and shown over the letterbox, leave and re-enter, a drag released outside, the wheel's notches.
- **Evidence.** The lab page in each browser with a 27° stroke in Follow and in Capture under the Firefox model (canvas shots from Playwright, never the host desktop), the level 1 and level 2 tables as JSON and as a short Markdown summary, into `~/Documents/Liminal HQ/Quill/Web demo/pointer/WP-PTR-1/`; commit `docs/media/web-pointer-lab.png`.
- **Parallel.** With WP-PTR-2.

### WP-PTR-2 Spike: js-dos's units, linearity, fractions and unlocked moves

- **Finish line.** A short findings section added to this plan (under *What was measured*, "js-dos units", with the date), answering, for each display mode (graphics 256, graphics 16, text) in headless Chromium and, if js-dos runs there, headless Firefox: (1) does `ci.sendMouseRelativeMotion` move Quill's pointer on a desktop browser with `mouseCapture: false` and no lock; (2) the pointer units moved per count on each axis, fitted over moves of 1, 2, 4, 8, 16, 32 and 64 counts and over a long sweep, with the residual; (3) is it linear (a count of 32 moves as far as 32 counts of 1), or does CuteMouse `/R13` speed it up; (4) does DOSBox keep a fraction (64 calls of 0.25 against 16 of 1); (5) does `mouseCapture: false` remove js-dos's overlay and its handlers once the page's capture-phase listeners stop the events; (6) where Quill's pointer starts after the menu key and after a mode's start. If (3) is no, the same answers for the switches that make it linear (`/R11`, or none), and a recommendation.
- **Files owned.** New `site/try-reactos/harness/jsdos-units.ts` (builds nothing; drives a locally built Try Quill page), a findings section in `docs/plans/pointer.md`, and evidence. No change to the published page.
- **Must not touch.** Everything else, including `site/try/**` (a spike copy of the page, if needed, lives under the evidence folder or in the harness and is not committed), `scripts/try-disk/**` (switches are tried on a local bundle only).
- **Dependencies.** None. Needs a local bundle: `bash scripts/build-try.sh --build` (mtools and the pinned Rust toolchain; the DOS toolchain as `dos/tools/vendor-dir.sh` finds it), served by a static server on 127.0.0.1.
- **Tests.** The pointer is found in `ci.screenshot()` frames by difference against a frame with the pointer parked in a known corner (the way `mouse-axis.ts` finds ReactOS's), with Quill's arrow's hotspot offset measured once from a shot; each number from at least five runs, with the spread.
- **Evidence.** The frames, the fits as JSON, a table in the findings, into `~/Documents/Liminal HQ/Quill/Web demo/pointer/WP-PTR-2/`.
- **Parallel.** With WP-PTR-1.

### WP-PTR-3 Try Quill on the module

- **Finish line.** In headless Chromium (and Firefox if js-dos runs there) the Try Quill page in each mode starts in Follow, passes level 2 through a wrapper of `ci` and level 3 at stops along a sweep, edges included, with no button stuck after a release outside; Capture works through the page's own lock (tested with dispatched movement) and the slider scales it; touch and pen keep or gain the gestures above; js-dos's overlay is gone; `?pointer=debug` shows the readout and saves a trace. The help text under the screen and the hints describe Follow and Capture. `stamp-try.py --check` passes with `pointer.js` stamped.
- **Files owned.** `site/try/try.js` (the pointer, the touch and the hint code replaced by the module and its adapter; `fitPointer`, `UNIT_X`, `UNIT_Y` and `POINTER_SPEED` go), `site/try/index.html` (the `pointer.js` tag, a *Mouse* control with *Follow* and *Capture* beside the Pointer speed slider, the help text), `site/css/try.css` (the control; `touch-action`), new `site/try-reactos/harness/try-pointer.ts`, `site/README.md` (the Try Quill pointer sentence), `docs/decisions.md` (SITE-22, and an amendment line under SITE-5), `docs/media/web-pointer-try.png`. Additive changes to `site/js/pointer.js` only as the rules say.
- **Must not touch.** `site/try/js-dos/**`, `scripts/try-dosbox.conf` and `scripts/try-disk/**` (unless the owner chooses new CuteMouse switches, then only the `CTMOUSE` lines), `site/try-reactos/**` except the new harness script, `crates/**`, `dos/**`.
- **Dependencies.** WP-PTR-1 merged; WP-PTR-2's numbers.
- **Tests.** `try-pointer.ts` as in the finish line, against a locally built bundle; `stamp-try.py --check`; `check-site.py`; `node --test scripts/pointer/` still passes.
- **Evidence.** A Playwright shot of the page with a diagonal selection made in Follow, the level 2 and 3 tables, into `~/Documents/Liminal HQ/Quill/Web demo/pointer/WP-PTR-3/`.
- **Parallel.** With WP-PTR-4.

### WP-PTR-4 ReactOS on the module

- **Finish line.** In headless Chromium and Firefox, the ReactOS page (restored QAGRR state) starts in Follow, passes level 2 on the bus and level 3 at the stops of `mouse-axis.ts`'s follow sweep, with the direction error added there; Capture passes with dispatched exact movement; the wheel sends one notch per 50 accumulated pixels; touch gains hold then drag and the two-finger right click; the diagnostics panel shows the *Pointer* row; `?mouse=follow|capture|lock|v86` and `?speed=` still work; `?cursor=local` is gone. `qagrr/check-state.sh` still passes.
- **Files owned.** `site/try-reactos/try-reactos.js` (only the pointer block, as the rules say), `site/try-reactos/index.html` (the `pointer.js` tag, the *Mouse* control and the slider, the *Pointer* diagnostics row), `site/try-reactos/try-reactos.css`, `harness/mouse-axis.ts` (the direction error in the follow sweep; Firefox as a second browser through its own launch), `site/try-reactos/README.md` (*The mouse*, rewritten), `docs/decisions.md` (SITE-23). Additive changes to `site/js/pointer.js` only as the rules say.
- **Must not touch.** `harness/lib.ts`, `manifest.js`, `host-files.js` and `sound.js` if WP-WEB-1 has made them, `qagrr/**`, `guest/**`, `site/try/**`, v86's files.
- **Dependencies.** WP-PTR-1 merged. The QAGRR parts and state in `~/.cache/quill-reactos-liminal-work/dist` (read-only).
- **Tests.** `mouse-axis.ts --check` with the direction error, in both browsers; `check-state.sh`; `node --test scripts/pointer/`.
- **Evidence.** A canvas shot of a diagonal rubber band on the desktop made in Follow, the tables, into `~/Documents/Liminal HQ/Quill/Web demo/pointer/WP-PTR-4/`.
- **Parallel.** With WP-PTR-3 and the web demo's packages.

### WP-PTR-5 The owner's check and the tuning it asks for

- **Finish line.** Every item of the owner's check list is yes on both pages, or has a recorded reason; each trace the owner saved is scored and its cause named; the constants the check moved (the wheel's pixels a notch, the gains, the edge overshoot, `parkOnEnter`) are set and recorded in SITE-24.
- **Files owned.** The constants in `site/js/pointer.js` and the two adapters, `docs/decisions.md` (SITE-24), the READMEs' numbers.
- **Must not touch.** Anything structural; a design change goes back to the owner as a question.
- **Dependencies.** WP-PTR-3 and WP-PTR-4 merged and published or served for the owner; the owner's run of the check list and traces in `~/Documents/Liminal HQ/Quill/Web demo/pointer/owner/`.
- **Tests.** `score-trace.mjs` over each trace before and after; the level 1 and 2 suites.
- **Evidence.** The scored traces into `~/Documents/Liminal HQ/Quill/Web demo/pointer/WP-PTR-5/`.
- **Parallel.** None.

## Builder briefs

Each brief is the common preamble followed by the package's own part. Paste both.

### Common preamble

```text
You are a Sonnet builder for Quill (repo liminal-hq-labs/quill), working on the pointer of the web pages
(site/try/ on js-dos and site/try-reactos/ on v86). Read AGENTS.md, docs/plans/pointer.md (your package,
"The design", "Measuring it" and "Rules for every package"), the SITE- decisions in docs/decisions.md
(SITE-1, SITE-4, SITE-5, SITE-7 at least) and site/try-reactos/README.md "The mouse".

Where to work: never touch ~/source/experiments/quill16 except `git fetch` and `git worktree add/remove`.
Create your worktree at ~/source/worktrees/quill/<topic> on branch <branch> from origin/main. Remove it
after the PR merges, with plain commands and full literal paths, one per command (`git worktree remove
<path>`, `git worktree prune`, `git branch -d <name>`); never globs, variables or `find -delete` in a
removal. Keep `df -h /home` above 20 GB; check free memory before a large write to /tmp (it is RAM).

Rules: Canadian English, real em dashes, sentence case, no hard-wrapped markdown. SPDX header on every new
source, script and YAML file (JavaScript and TypeScript take the `//` form). Tabs except YAML. Never
`git add -A` or `git commit -a`; stage named paths. Commits: Conventional Commits, body in markdown with
bold labels and no headings, hard-wrapped at about 100 columns, written to a file with a single-quoted
heredoc and committed with `git commit -F`, ending with `Co-Authored-By: Claude Sonnet <noreply@anthropic.com>`
naming your own model. Push only your branch; open one PR (title capitalized, imperative, no prefix; body
`## Summary` with bold lead-in bullets, `## Test plan` with the commands run, ending with an unchecked
"Not verified:" line). Do not merge.

Browsers and servers: every browser is headless (Chromium at /usr/bin/chromium, or Playwright's Firefox
159, installed with `PLAYWRIGHT_SKIP_BROWSER_GC=1 node node_modules/playwright-core/cli.js install firefox`
from site/try-reactos/harness after `bun install`), and every command that starts one sets DISPLAY
explicitly with WAYLAND_DISPLAY unset (`env -u WAYLAND_DISPLAY DISPLAY=:99 ...`). Servers bind 127.0.0.1.
Record the PID of everything you start and stop it by that PID; never pkill or killall by name. Never
show anything on, or send input to, the host desktop. Pixels come from the page's canvas, js-dos's
`ci.screenshot()` or Playwright's page screenshot.

Stay inside "Files owned". If you need a change in a must-not-touch path, stop and report what and why.
When a choice is not covered, take the reasonable default, record it in your SITE- row marked **Assumed**,
list it in the PR as "OWNER DECISION: question | default taken | where to swap", and keep going.

Evidence: into ~/Documents/Liminal HQ/Quill/Web demo/pointer/<package>/; commit the few shots that
document the feature under docs/media/ as web-pointer-*.png. Before the PR: bash scripts/check-headers.sh,
markdownlint in Docker (docker run --rm -v "$PWD:/workdir" davidanson/markdownlint-cli2:v0.17.2 "**/*.md"),
python3 -I scripts/check-site.py, node --test scripts/pointer/ (once it exists), python3 -I
scripts/stamp-try.py --check site if you touched site/try/ or site/js/, and your package's own checks.
Report: PR URL, what was and was not run, the evidence paths, OWNER DECISION lines, and anything you
stopped on.
```

### WP-PTR-1 The module, the lab page, the metric and its tests (full brief)

```text
Package WP-PTR-1, the pointer module, the lab page, the metric and its tests. Branch feat/pointer-module,
worktree ~/source/worktrees/quill/pointer-module.

Read first: docs/plans/pointer.md in full (the contract in "One module, two adapters" is what WP-PTR-3 and
WP-PTR-4 build on: keep its names); site/try-reactos/try-reactos.js from "The pointer." to the end of
hookPointer (shownRect, sendMove's remainder, flushMove's per-frame packet and its 120 limit, park,
followTo) and site/try/try.js's onTouch and pointerUnits (the trackpad gestures you are generalizing);
site/try-reactos/harness/mouse-axis.ts (finding a pointer, the Firefox model in trace()); the evidence in
~/Documents/Liminal HQ/Quill/Web demo/pointer/plan-evidence/ (probe.ts, sim.py: the metric's first draft).

Goal: a module both pages can adopt with a few lines of adapter, proven by a direction-error metric in
Node and in two headless browsers, before either page changes.

Build, in this order, one commit each where it makes sense:

1. site/js/pointer-metric.js: a classic script that sets globalThis.QuillPointerMetric =
   { directionError(input, output, { window = 8, lags = [0, 1, 2] }) -> { windows, median, p95, axisOnly,
   endpoint: [dx, dy], pathRatio } } over arrays of { t, x, y } in guest pixels, as "The metric" defines
   (the lag that gives the lowest median wins). scripts/pointer/metric.test.mjs: a straight output scores
   0; an output rotated by 10 degrees scores 10; a pure-horizontal output of a 27-degree input scores 27
   with axisOnly 1; a frozen output scores 90; windows below the travel are skipped. Load the classic
   file in tests with `await import("../../site/js/pointer-metric.js")` and read the global.
2. scripts/pointer/paths.mjs: generated input paths (angles, circle, figure eight; speeds; 125 and 1000
   Hz) and the three capture input models (exact; Chromium: the difference of rounded positions; Firefox
   on Wayland: each report cut to whole device pixels at the scale, both-zero reports dropped), all pure
   and seeded.
3. site/js/pointer.js, QuillPointer.core first (pure, no DOM): a follow mapper (shown rect, guest size,
   units -> per-frame counts, remainder, clamp, model position, edge resync with a 16-pixel overshoot
   once per arrival, park toward the nearest corner split at maxCount); a capture accumulator (samples
   times gain times speed times guest-over-shown -> per-frame counts, remainder, model clamp; the
   whole-number detector over the first 128 non-zero samples); a counts splitter; a wheel accumulator
   (lines x16, pages x picture height, one notch per 50 px, remainder dropped after 300 ms idle); a touch
   gesture machine (tap under 300 ms and 10 CSS px, hold 350 ms then drag holds the button, two-finger
   tap is a right click, drag moves with FINGER_GAIN 1.4 times the speed); the raw probe's decision
   (resolves -> capture and raw=yes; rejects -> follow and raw=no, the press goes to the guest; no promise
   -> raw=no and exitPointerLock). scripts/pointer/pointer.test.mjs: everything in the plan's WP-PTR-1
   Tests line at level 1, the full matrix through the metric with the plan's pass lines, and the asserted
   failure for capture under the Firefox model at 27 degrees, 0.8 px/ms, 1000 Hz, scale 1.5.
4. QuillPointer.attach: listeners on the stage in the capture phase (pointerdown/move/up/cancel,
   pointerenter/leave, lostpointercapture, wheel with passive false, contextmenu), window blur and
   visibilitychange; one requestAnimationFrame flush a frame; setPointerCapture on a press in Follow;
   cursor none over the shown picture only; requestPointerLock on the stage for Capture, with
   pointerlockchange; the stored mode, raw answer (with the user agent) and speed under storageKey, every
   localStorage access in try/catch; ?mouse= and ?speed= read by the page and passed in as defaults (the
   module does not read location); onState on every change; trace.start/stop keeping 20 s of input,
   output and events in a ring, in the JSON shape the plan gives. A pen follows in every mode; a finger
   is the trackpad in every mode.
5. site/pointer-lab/index.html (+ .js, .css): title "Pointer lab", one h1, a short paragraph saying what it
   is for; a 640 by 480 stand-in guest canvas shown at the page's width (letterboxed), drawing a small
   arrow at the model's guest position and the input and guest paths of the last stroke; a Follow and
   Capture switch and the speed slider (shown as the plan says); a units choice (1 x 1, 1.37 x 1.1); the
   readout (mode, raw, secure, coalesced, fractional share, whole-only, dpr, calls a frame) and the last
   stroke's median and p95 error and one-axis share; Save pointer trace. Every string in plain Canadian
   English; the switch is a real radio group with a label; the readout is aria-live polite. Not linked
   from the other pages; check-site.py must pass over it.
6. scripts/pointer/score-trace.mjs FILE...: prints the metric for each stroke in a trace (strokes split
   at pauses of 250 ms or more).
7. site/try-reactos/harness/pointer-lab.ts: serve site/ on 127.0.0.1 (reuse harness/serve.ts as a child
   process with PORT set, or a small static server of your own; record and kill its PID), then for
   Chromium (executablePath /usr/bin/chromium) and Firefox (Playwright's build), each launched here, not
   through lib.ts: Follow over the plan's angles at 0.1 and 0.8 px/ms and dpr 1 and 1.5 (Chromium:
   trusted page.mouse.move with fractional positions; Firefox: dispatched PointerEvents with fractional
   clientX, since its driver rounds trusted ones), Capture with dispatched movementX (exact and Firefox
   model), hidden pointer over the picture and shown over the letterbox, leave and re-enter, a drag
   released outside, wheel notches; read the stand-in guest's calls from the page and score them with
   the metric; print a table and exit 1 on any miss of the plan's pass lines.
8. .github/workflows/ci.yml: a new job "Pointer tests" on ubuntu-latest (the host runner, as the site
   job), actions/checkout pinned as the other jobs pin it, and `node --test scripts/pointer/`. One job,
   one check. site/README.md: a short "The pointer" section (the module, the lab, the commands).
   docs/decisions.md: SITE-20 and SITE-21 from the plan's "Decisions to record", with alternatives (UA
   sniffing for the default; capture everywhere with a quantization detector; js-dos's or v86's own
   handlers; coalesced samples in Follow; the one-axis share as the measure) and consequences, in the
   SITE- rows' style.

Must not touch: site/try/**, both pages' index.html and try-reactos.js, site/try-reactos/harness/lib.ts and
mouse-*.ts, scripts/build-try.sh, scripts/try-disk/**, scripts/stamp-try.py, crates/**, dos/**, v86's and
js-dos's files.

Finish line checks (all must hold):
- node --test scripts/pointer/ passes, including the asserted capture failure.
- pointer-lab.ts passes in Chromium 156 and Firefox 159, headless, and its table is in the PR.
- check-site.py, the headers check and markdownlint pass.
- No page other than the lab loads pointer.js yet.

Evidence: into ~/Documents/Liminal HQ/Quill/Web demo/pointer/WP-PTR-1/: the lab page in each browser after a
27-degree stroke in Follow and in Capture under the Firefox model (Playwright page screenshots), the level
1 and 2 tables as JSON and a short Markdown summary. Commit docs/media/web-pointer-lab.png (the Follow
stroke in Firefox). PR title suggestion: "Add a pointer module that follows the visitor's pointer".
```

### WP-PTR-2 Spike: js-dos's units

```text
Package WP-PTR-2, spike: js-dos's units, linearity, fractions and unlocked moves. Branch
spike/jsdos-pointer-units, worktree ~/source/worktrees/quill/jsdos-pointer-units.

Read first: the plan's "The emulators, from their own files", "The js-dos side" and WP-PTR-2; site/try/try.js
(start, the Dos options, onTouch, the stopPropagation of touch pointers); scripts/try-dosbox.conf;
scripts/try-disk/*.BAT (CTMOUSE /R13); crates/quill-mouse/src/lib.rs (open: range, 8/8 mickeys, centre);
site/README.md on build-try.sh.

Build a local bundle (bash scripts/build-try.sh --build), serve site/ on 127.0.0.1, and write
site/try-reactos/harness/jsdos-units.ts that opens /try/?pointer=spike in headless Chromium (and
Playwright's Firefox if js-dos starts there), selects a mode, waits for Quill, and answers the six
questions in the plan's WP-PTR-2 finish line with numbers. A spike-only switch for mouseCapture and the
capture-phase stop may be injected by the script (page.addInitScript or by wrapping window.Dos); do not
change site/try/. Find Quill's pointer by difference in ci.screenshot() frames against a frame with the
pointer pushed into the top-left corner (a large negative move clamps it). If /R13 is not linear, rebuild
a local bundle with /R11 and with no /R and repeat. Write the findings into docs/plans/pointer.md under
"What was measured" as "js-dos units (<date>)": a table per mode, the fits, the spread over five runs, and
a recommendation for WP-PTR-3's units and parkOnEnter. PR title suggestion: "Measure js-dos's pointer
units for the Try Quill page". The PR changes only the plan and adds the script.
```

### WP-PTR-3, WP-PTR-4 and WP-PTR-5

```text
Package <id>, <title from the plan>. Branch <feat/try-quill-pointer | feat/reactos-pointer |
fix/pointer-tuning>, worktree ~/source/worktrees/quill/<same topic>.

Read first: the plan's section for your package, "The design" and "Measuring it", the SITE-20 and SITE-21
rows WP-PTR-1 wrote, site/js/pointer.js and site/pointer-lab/ as merged, and the files your package owns.
WP-PTR-3 also reads WP-PTR-2's findings in the plan and site/try/js-dos/PROVENANCE.md; it sets
mouseCapture: false, stops js-dos's mouse, pointer and touch events in the capture phase, uses
ci.sendMouseRelativeMotion and ci.sendMouseButton through the module's adapter with the measured units,
removes fitPointer, and restamps with scripts/stamp-try.py. WP-PTR-4 replaces only try-reactos.js's
pointer block (rebase on WP-WEB-1 if it has merged), keeps ?mouse=v86 as v86's own adapter for comparing,
maps ?mouse=lock to capture, removes ?cursor=local, and adds the direction error and Firefox to
mouse-axis.ts. WP-PTR-5 starts from the owner's check list results and traces in
~/Documents/Liminal HQ/Quill/Web demo/pointer/owner/, scores them with scripts/pointer/score-trace.mjs,
and changes only constants.

Build what the plan's Finish line says, in small commits, staying inside Files owned. Finish line, tests,
evidence and decision numbers: the plan's package section. PR title suggestions: "Follow the visitor's
pointer on the Try Quill page", "Follow the visitor's pointer on the ReactOS page", "Tune the web pages'
pointer from the owner's check".
```

## Owner decisions

Each is taken with the default so no package waits; the last column is where to change it.

| Question | Default taken | Where to swap |
|---|---|---|
| Default mode on browsers with raw capture (Chromium on Windows, macOS, ChromeOS) | Capture, as asked; Follow everywhere else, including Chromium on Linux | `defaults.mode` and the probe in `pointer.js` (WP-PTR-1); SITE-20 |
| Does the first click in Follow reach the guest? | Yes: the guest pointer is already under the visitor's, so the click lands where aimed; it also gives the keyboard | The press handling in `attach` (WP-PTR-1) |
| Park on every re-entry, as the ReactOS page does today? | No on ReactOS (the model stays exact and edges resync it); yes on Try Quill until WP-PTR-2 shows its units are exact | `parkOnEnter` in each adapter (WP-PTR-3, WP-PTR-4) |
| Wheel on the ReactOS page | One notch per 50 accumulated pixels, only after the screen is selected | The wheel accumulator's constant (WP-PTR-5 tunes it); SITE-23 |
| Wheel on Try Quill | None: js-dos 8.5.3 cannot send one | A later js-dos update; `site/try/js-dos/PROVENANCE.md` |
| CuteMouse's switches if `/R13` is not linear | Keep `/R13` and rely on edge resync and parking; propose `/R11` with numbers | `scripts/try-disk/*.BAT` and SITE-5 (owner's decision there) |
| `?cursor=local` on the ReactOS page | Removed (off by default, and pointless in Follow) | SITE-23 (WP-PTR-4) |
| Two-finger tap as a right click on touch | Yes, on both pages | The touch gesture machine (WP-PTR-1) |
| Publish the lab page with the site | Yes, unlinked at `/quill/pointer-lab/`, so the owner can check on the real HTTP origin | `site/pointer-lab/` (WP-PTR-1) |

## Risks

- **The ReactOS report is not explained.** If Follow itself snaps on the owner's machine, the cause is outside this design's model (the guest, or Firefox's positions); the trace and item 1 and 9 of the check list find it before WP-PTR-4 is called done.
- **js-dos's units** may not be exact or linear. Edge resync and parking bound the drift; the owner chooses the driver's switches if needed.
- **Hidden pointer and lag.** In Follow the visitor sees only the guest's pointer, which is drawn a frame or two later (8 to 17 ms measured on ReactOS). A visitor on a slow machine may feel it; Capture stays one click away.
- **Raw probe on the first click.** On a browser that supports raw, the first press both captures and is swallowed, which a visitor may not expect; the hint says how to leave (`Esc`) and how to choose Follow.
- **js-dos internals.** Stopping its events depends on its listeners being on its overlay in the bubble phase (js-dos 8.5.3). An update could move them; `PROVENANCE.md` gains a line to check it.
- **Shared files** with WP-WEB-1 (`try-reactos.js`, `index.html`): limited to the pointer block and one control row; the second to merge rebases.

## What was not verified

- That Follow on the current ReactOS page snaps on the owner's machine, or why it would.
- That js-dos's relative moves act without a lock on a desktop browser, its units, linearity and handling of fractions (WP-PTR-2).
- That js-dos and v86 run in headless Firefox well enough for level 3 there.
- Raw capture on Chromium for Windows or macOS (no such machine was used; the claim rests on Chromium's documented support).
- Safari, on any device, and a pen.
- The owner's mouse's report rate; the model assumes 1000 Hz, and a slower mouse loses less in Capture.
