# The AC'97 card for any v86 machine, and the virtual hardware after it

A plan for turning Quill's virtual AC'97 sound card (`site/try-reactos/packages/ac97/`, merged in #194, with the microphone, the record channels and the band-limited resampler of #198) into a reusable package for any v86 machine that boots from scratch, not only the ReactOS desktop the demo restores. It answers which guests can drive the card, what a page writes to use it on a cold boot, where the package lives, how Quill keeps using it, which other virtual devices the same pattern can carry (a USB host controller first, at the owner's request), and what can go wrong. The owner's words, 2026-11-28: "Since we have the sound card set up then it seems like a cool reusable pattern to be able to use it from a VM that boots fresh", and then "Let's also add USB two or three."

**Status, 2026-11-28.** A plan; nothing is built. Written against Quill `main` at `995296b`, the `feat/ac97-recording-resampler` branch (#198, open), v86 0.5.473 (commit `315b2b0`), and read-only checks of the sources named below. No spike was run: every question below was settled by reading, or is marked untested. Decision ids SITE-100 to SITE-109 are reserved by hand (SITE-51 to SITE-57, SITE-60 to SITE-69, SITE-70 to SITE-73, SITE-80 to SITE-89 and SITE-90 to SITE-91 are taken on `main` and in open branches).

## Table of contents

- [Recommendation](#recommendation)
- [What the card is today](#what-the-card-is-today)
- [Which guests can use it](#which-guests-can-use-it)
- [What the model must add](#what-the-model-must-add)
- [The cold-boot pattern](#the-cold-boot-pattern)
- [The v86 internals it uses](#the-v86-internals-it-uses)
- [The package boundary and the repository](#the-package-boundary-and-the-repository)
- [How Quill consumes it](#how-quill-consumes-it)
- [Other virtual hardware](#other-virtual-hardware)
- [A USB host controller](#a-usb-host-controller)
- [Risks](#risks)
- [Decisions to record](#decisions-to-record)
- [Order and parallelism](#order-and-parallelism)
- [Work packages](#work-packages)
- [Builder briefs](#builder-briefs)
- [Owner decisions](#owner-decisions)
- [What was not verified](#what-was-not-verified)

## Recommendation

1. **The card is already a cold-boot device.** `attachAc97` registers the PCI function by wrapping `emulator.v86.init`, which runs before the BIOS enumerates PCI on every boot, saved state or not; the restore path is the extra, not the base. A page that cold-boots any guest gets the card by calling `attachAc97(emulator, { context })` right after `new V86(...)`. Three things are missing for guests other than ReactOS: the DMA position must follow a clock rather than the speakers' pulls (Linux measures it), a guest reboot must reset the card (v86 does not reset page devices), and the codec's registers must keep only the bits a real codec keeps (drivers probe them).
2. **Guests.** ReactOS 0.4.16: **GO**, measured. Linux with `snd-intel8x0`: **untested, GO on paper** once the DMA position follows a clock; no published v86 Linux image has sound, so the work includes a small test image. 9front, Haiku, KolibriOS, SerenityOS, FreeBSD and OpenBSD ship AC'97 ICH drivers: **untested**. Windows XP has an inbox driver: **untested** (v86 runs XP only under conditions). Windows 98, ME and 2000 need a vendor driver the user brings: **untested, never bundled**. DOS and Windows 3.x: **NO-GO** for AC'97; v86's own Sound Blaster 16 is the right device there and is out of scope.
3. **A new Labs repository, `liminal-hq-labs/v86-devices`**, holding `v86-device-kit` (the attach-to-v86 core, one file that knows v86's internals) and `v86-ac97` (the card), with a live demo page that cold-boots a small Linux guest and the ReactOS 0.4.16 live CD with sound. Publishing to npm is the owner's step; the names `v86-devices`, `v86-ac97`, `v86-device-kit` and `v86-usb` are free on npm and on GitHub today. The host link is its own repository, `v86-host-link` (SITE-129): it uses none of the device kit, has no dependency on `v86-ac97`, and the USB serial and printer devices stay in `v86-devices` as transports a page hands to the link.
4. **Quill keeps its own copy through the first demo.** Extraction starts after the launch; once the new repository tags its first release, Quill replaces `packages/ac97/` with a vendored build (`site/try-reactos/vendor/v86-ac97/`, the built module, its types and a `PROVENANCE.md` with the tag, commit and SHA-256), as `dos/` vendors Desk's modules. No npm dependency enters the site.
5. **USB next, starting small.** v86 0.5.473 has no USB controller. Start with a UHCI controller as the PIIX3's own USB function (8086:7020, an I/O BAR, so the AC'97 glue carries over) and an emulated HID tablet, the pair QEMU's `-usb -device usb-tablet` is and that Quill's QAGRR check already drives ReactOS with; then mass storage, then a printer class device and a CDC serial device for the host link; EHCI when a device needs the speed; xHCI last. The owner's suggestion of a minimal EHCI first is the swap (see [A USB host controller](#a-usb-host-controller)).

## What the card is today

`site/try-reactos/packages/ac97/` (SITE-51 to SITE-53 on `main`; SITE-54 to SITE-57 in #198):

| File | Lines | What |
|---|---|---|
| `src/model.ts` | 548 (727 with #198) | ICH bus master (PCM in, PCM out, mic in), the AC'97 2.3 codec, descriptor walking, status bits, the interrupt line; no v86 and no browser |
| `src/v86.ts` | 177 | The only file that knows v86: PCI space, I/O handlers, `onceBeforeInit`, `registerOn`; `V86_VERSION = "0.5.473"` |
| `src/sink.ts` | 348 | An `AudioWorklet` ring player that counts what it plays, or a `ScriptProcessorNode` fallback |
| `src/index.ts` | 270 | `attachAc97(emulator, options)` and the handle |
| `src/resample.ts` (#198) | 262 | Kaiser-windowed sinc polyphase resampler, 90 dB, no allocation in the hot path |
| `src/capture.ts` (#198) | 182 | The microphone: the permission, a worklet that hands out mono blocks, a stop that ends every track |
| `test/` | 678 on `main` | `bun test`: 35 tests on `main`, 87 with #198 |

The built `site/try-reactos/lib/ac97.js` is 19 KB on `main` (24 KB budget) and 30 KB with #198 (36 KB budget). The card answers as PCI 8086:2415 (82801AA, class 04:01:00), subsystem 8086:0000, two I/O BARs (mixer 256 bytes at power-on `0xD000`, bus master 64 bytes at `0xD400`; SeaBIOS moves them, to `0xC000` and `0xC100` in WP-WEB-4), interrupt pin A (line 11 in WP-WEB-4), and a codec with the SigmaTel STAC9700 vendor id (`0x8384 0x7600`) and variable rate audio. What WP-WEB-4, WP-WEB-5 and WP-WEB-5b measured is in [web-demo.md](web-demo.md); the evidence is in `~/Documents/Liminal HQ/Quill/Web demo/WP-WEB-4/`, `WP-WEB-5/` and `WP-WEB-5b/`.

The chip is the same one VirtualBox and QEMU emulate (ICH 8086:2415 with a STAC9700 codec), so a guest that plays sound in either with their AC'97 device has the driver this card needs. That is the main evidence below where nothing was run.

## Which guests can use it

"Cold boot finds it" means the guest's PCI scan sees the function and binds a driver without a window. The verdicts: **GO** (measured here), **untested** (with what reading says), **NO-GO**.

| Guest | In v86 | Driver | Cold boot finds it | Verdict | What the model must add | Test |
|---|---|---|---|---|---|---|
| ReactOS 0.4.16 | Works | `ac97.sys` (`drivers/wdm/audio/drivers/ac97`, built in 0.4.16) | **Yes, measured**: bound silently, first codec access 21 s after the taskbar (WP-WEB-4) | **GO** | Nothing; recheck after the clock-paced position (below) | `check-sound.ts` on a cold boot of the 0.4.16 live CD: Sounds tab, *Critical Stop*, frames and rate |
| Linux, `snd-intel8x0` with `snd-ac97-codec` | Works (32-bit) | In mainline; the STAC9700 entry (`patch_sigmatel_stac9700`) matches the vendor id | Should: the driver's probe (read from `sound/pci/intel8x0.c` and `sound/pci/ac97/ac97_codec.c`) needs the primary codec ready bit in global status (set), the codec semaphore free (always), a record gain register that keeps `0x8A05` and `0x8A06` (it does: `0x1C` stores what is written), the power-down register's low four ready bits (`0x26` reads `0x000F`), and a channel reset bit that clears (it clears at once) | **Untested, GO on paper** | **The DMA position must follow a clock.** With `ac97_clock=0` (the default) and subsystem 8086:0000 (not in its clock list, not QEMU's 1AF4:1100 that switches the measurement off), the driver starts PCM out, sleeps 50 ms and reads `PICB`. Today the position moves when the speakers pull: a live worklet tops its ring 120 ms ahead at once (the driver rejects the result three times and keeps 48 kHz, after about a second of retries), but the drain path that runs while the context is suspended moves in 10 ms steps, which can land between 40,000 and 60,000 and retune the clock, so every sound plays at the wrong pitch. Also: register write masks (the volume-resolution probe) and a cold reset through global control | No published v86 image has sound: `chschnell/v86-buildroot`'s `linux.config` (the source of v86's Buildroot image) has no `CONFIG_SND` and `# CONFIG_USB_SUPPORT is not set`. Build a test image: v86-buildroot plus `CONFIG_SND_INTEL8X0=y` and `alsa-utils`, booted with `console=ttyS0`; the harness runs `cat /proc/asound/cards`, `dmesg \| grep -i ac97` (the measured clock), `speaker-test -t sine -f 1000 -l 1` and `arecord -d 2 -r 22050` over the serial line and counts frames, rate and pitch at the probe |
| 9front | Works | `audioac97` (in the `pc` kernel's configuration: `audioac97 pci audioac97mix`) | Should | **Untested** | Probably none; it polls position with `CIV` and `PICB` | Boot the copy.sh 9front image with the card; `audio/mp3dec < x.mp3 > /dev/audio` or a raw tone to `/dev/audio`; count frames |
| Haiku | Works | `auich` (`src/add-ons/kernel/drivers/audio/ac97/auich`) | Should | **Untested** | Probably none | Media preferences lists the card; play a system sound; count frames |
| KolibriOS | Works | `drivers/audio/ac97.asm` | Should, if the sound service loads it | **Untested** | Probably none | Boot, run the player, count frames |
| SerenityOS (32-bit) | Works | `AC97` in its kernel | Should | **Untested** | Probably none | `aplay` a WAV, count frames |
| FreeBSD, OpenBSD | Works (OpenBSD with `disable mpbios`) | `snd_ich`, `auich` | Should | **Untested** | Probably none | `cat tone.raw > /dev/dsp`, `audioplay` |
| Windows XP | "Under certain conditions" (v86 Readme) | Inbox for 82801AA (libosinfo switched XP guests to `ac97` for that reason) | Should, after the usual Found New Hardware on a fresh install | **Untested** | Probably none | Low priority: XP in v86 is slow and its images are the user's |
| Windows 2000 | Works ("Standard PC") | No inbox driver confirmed; Intel, Realtek and SigmaTel AC'97 drivers exist and are proprietary | After the user installs a driver | **Untested; never bundled** | Probably none | The user's own image and driver; the demo page offers "bring your own disk" |
| Windows 98, ME | Works | Vendor drivers (Realtek AC'97, Intel), proprietary | After the user installs a driver | **Untested; never bundled** | Probably none | As above |
| Windows 95, 3.x | Works | No WDM audio and no driver for this chip | No | **NO-GO** | — | — |
| FreeDOS, MS-DOS | Works | DOS programs speak Sound Blaster; AC'97 needs per-game drivers | No | **NO-GO**: v86's own SB16 (`src/sb16.js`) is the device for DOS, out of scope here | — | — |

## What the model must add

In the order a builder should take them; the first three are needed before any guest but ReactOS is tried.

1. **A clock-paced DMA position.** The card advances `CIV`, `PICB` and the buffer completions at the guest's rate from a clock (the page's `performance.now()`, read when the guest reads a position register and on a timer), and fills a small ring that the output pulls from; the output no longer drives the guest. This also makes `PICB` a true position for ALSA's pointer callback. The two cases WP-WEB-5 found stay covered: with no listener the ring is dropped at the real-time rate, and the interrupt hold (two completions never merge into one interrupt) stays in the clocked path. ReactOS must be rechecked with `check-sound.ts`.
2. **A reset on a guest reboot.** v86's `reboot_internal` resets the CPU, the virtio devices and the keyboard and reloads the BIOS; a page device is not told. Today a guest that reboots mid-sound leaves PCM out running from stale memory with its interrupt line up. The kit wraps `reboot_internal` (one more internal, listed below) and calls each device's `reset()`; `emulator.restart()` goes the same way.
3. **Codec write masks and zero for absent registers.** Each codec register keeps only the bits a STAC9700 keeps (five volume bits per side and mute for master and PCM out, four bits for the record gain, and so on), and registers the codec does not have read 0. Linux's `check_volume_resolution` and Windows mixers probe this way. The extended status register (`0x2A`) must not set bit 7 (that is the surround DAC's ready bit; the front DAC's readiness is in `0x26`).
4. **Global control.** A cold reset (bit 1 of `GLOB_CNT` cleared, then set) resets the codec and sets codec ready again; the AC-link shut-off bit drops codec ready while set. Linux does this at probe.
5. **Options for other guests.** `subsystem` (default 8086:0000; a vendor driver's INF may match a subsystem), `slot` (default the first free from 3), `trace`. Nothing else: 4 and 6 channel output, S/PDIF, the modem codec and the separate microphone rate (`0x34`, VRM) stay out; the card does not claim them (global status bits 20 and 21 zero, extended id bits clear), so no driver asks.
6. **Not needed.** PCI power management capability (no driver above requires one), MSI, bus-master memory BARs (ICH4 and later).

## The cold-boot pattern

The card registers itself in the one moment that serves every boot: right after v86 builds its devices and before the BIOS or a restore runs (`onceBeforeInit`, SITE-19). A page writes:

```js
import { V86 } from "./v86/libv86.mjs";
import { attachAc97 } from "./lib/v86-ac97.js";

startButton.addEventListener("click", () => {
	const context = new AudioContext({ latencyHint: "interactive" }); // inside the click: browsers start sound only from a gesture
	const emulator = new V86({ wasm_path: "v86/v86.wasm", bios: { url: "v86/bios/seabios.bin" }, vga_bios: { url: "v86/bios/vgabios.bin" }, cdrom: { url: "images/linux-sound.iso" }, memory_size: 128 << 20, autostart: true });
	const card = attachAc97(emulator, { context, volume: 0.7 }); // same tick as new V86: before v86 builds its devices
	muteButton.onclick = () => card.setMuted(!card.isMuted());
	stopButton.onclick = () => { card.detach(); emulator.destroy(); };
});
```

Rules the package documents and tests:

- **Call it in the same tick as `new V86(...)`.** v86 builds its devices after its WebAssembly loads; `attachAc97` waits for that. Attached later (the machine already built), it registers at once, which is too late for the BIOS's PCI scan: the package then reports `stats().late = true` and the guest sees the card only if it rescans PCI.
- **Restore is the option, not the base.** `state` (what `getAc97State()` returned beside a saved v86 state) is passed only when restoring a state saved with the card; a state saved without it is restored without it (SITE-53). A page that only cold-boots never touches `state`.
- **A reboot is a cold boot.** The card resets on `reboot_internal` (item 2 above).
- **One card per machine, any number of machines.** Each `attachAc97` makes its own model and output; two machines on one page each get their own (tested).
- **The guest never waits on the speakers.** Unchanged from WP-WEB-5: with no output, or a suspended one, the guest's sound runs at the real-time rate and is dropped.

## The v86 internals it uses

None of these is a published interface; each is tied to the pinned v86 version (0.5.473, commit `315b2b0`). Fragility is how likely an ordinary v86 change is to break the line.

| Internal | Used for | Where in v86 | Fragility |
|---|---|---|---|
| `emulator.v86` (assigned after the constructor returns) | Finding the machine before `init` | `src/browser/starter.js` | **Medium**: the device wraps the property with `defineProperty`; a v86 that builds the machine synchronously, or stores it under another name, breaks the wrap (the code already handles "already built", which then means "too late") |
| `v86.init(settings)` | The moment after the devices exist and before the BIOS runs or a state is restored | `src/main.js` | **Low**: the shape has been stable for years |
| `cpu.devices.pci.devices`, `register_device({ pci_id, pci_space, pci_bars, name })` | Taking a free slot and registering the function | `src/pci.js` | **Low to medium**: every v86 PCI device uses it |
| `pci.raise_irq(pci_id)`, `lower_irq(pci_id)` | The interrupt line (pin to PIRQ through the PIIX3's `0x60` registers) | `src/pci.js` | **Low** |
| I/O BAR move semantics: port handlers registered at the BARs' power-on addresses before `register_device`, because v86 copies the port entries then and moves them when a BAR moves | Letting SeaBIOS relocate the BARs | `src/pci.js` (`set_io_bars`) | **High**: an undocumented ordering rule; a change there silently leaves the card deaf. A test on a fake v86 pins the order, and the browser check catches the rest |
| `cpu.io.register_read`, `register_write` | The port handlers | `src/io.js` | **Low**: used by every device |
| `cpu.read_blob(address, length)`, `cpu.write_blob(bytes, address)` | Reading the descriptor lists and samples, writing recorded samples | `src/cpu.js` | **Low** (`write_blob` also clears the JIT's cache for those pages) |
| `cpu.reboot_internal` (new, item 2) | Resetting on a guest reboot | `src/cpu.js` | **Medium**: a private method |
| `cpu.io.mmap_register(addr, size, r8, w8, r32, w32)` (USB only, EHCI and later) | A memory BAR | `src/io.js` | **Medium**: 128 KB granularity, and v86 does not move memory BARs ("Changing memory bar not supported, ignored"), so the device must sit at a fixed address SeaBIOS will not reuse |

What v86 does not offer, and the package works around: page devices are not in `save_state` (the state's device slots are fixed indexes in `cpu.js`), hence the sidecar registers; no device timer hook (`run_hardware_timers` ticks only the PIT, RTC, ACPI and APIC), hence the page's timers; no reset call for page devices, hence the wrap.

**Upstream.** A small public device API in v86 would remove every line above but the model's own: a `devices` option (or `emulator.add_device`) called during `init` with a host object that offers PCI registration with relocatable I/O and memory BARs, an interrupt line, guest memory access, a reset callback, and a slot in `save_state` and `restore_state` for the device's own state. The plan drafts it as an issue for copy/v86 in WP-AC97-6; **nothing is opened** until the owner says so, and the package keeps working without it.

## The package boundary and the repository

**Repository: `liminal-hq-labs/v86-devices`** ("Virtual hardware for the v86 emulator"). One repository for the family keeps one demo page, one CI and one kit; each device is its own package and its own built module, so a page loads only what it uses.

| Package | npm name (owner-gated) | What | Budget (built, minified) |
|---|---|---|---|
| `packages/device-kit` | `v86-device-kit` | `onceBeforeInit`, `registerPci` (slot, BARs, ports before the function), the interrupt line, guest memory, the reboot reset, the late-attach report, `V86_VERSION`; the only code that knows v86's internals | 4 KB |
| `packages/ac97` | `v86-ac97` | The chip model, the output (worklet and fallback), the resampler, the microphone, `attachAc97`; the kit bundled in, so the built module has no runtime dependency | 36 KB with the kit (Quill's budget today) |
| `packages/usb` (later) | `v86-usb` | The USB controller family (below) | Set by WP-USB-1 |

**The host link.** The host link is its own repository, `v86-host-link` (SITE-129): it uses none of the device kit, has no dependency on `v86-ac97`, and the USB serial and printer devices stay in `v86-devices` as transports a page hands to the link.

The repository follows the Labs template (`labs-template/docs/new-project.md`): created from the template, public from day one; its own hero banner (`assets/hero.svg`, a scene of its own: a sound card's edge connector and a waveform, in a palette no other Labs repo uses, checked with `scripts/check-banner.sh`); labels cloned from Mailslot with the house extras (`documentation`, `architecture`, `audio`, `usb`); topics `liminal-hq-labs`, `experimental`, `typescript`, `v86`, `emulation`, `web-audio`; merge commits only, no squash or rebase merges, branches deleted on merge, a read-only workflow token, secret scanning on, Dependabot off; Canadian English; SPDX headers on every source, script, YAML, TOML and SVG file (`(c) Copyright 2026 Liminal HQ, Scott Morris`, `SPDX-License-Identifier: Apache-2.0 OR MIT`), with `LICENSE-APACHE` and `LICENSE-MIT`.

**Contents.**

- `README.md`: the pitch, the ten-line example, the supported guests table (the one above, kept current), the live demo link, *Status: experimental*.
- `docs/how-it-works.md` (the attach moment, the BAR rule, the clocked DMA, the output and its fallback), `docs/guests.md` (per guest: driver, how to test, what was measured), `docs/limits.md` (what the card does not do), `docs/privacy.md` (the microphone rules below), `docs/v86-internals.md` (the table above, per v86 version), `docs/releasing.md`, `docs/decisions.md` (prefix `VD-`), `SPEC.md`, `AGENTS.md` and `CLAUDE.md`.
- `demo/`: one static page that cold-boots a guest picked from a list: the test Linux image (built in the repository from v86-buildroot, GPL-2.0 kernel and Buildroot packages, its source published beside the image) and the ReactOS 0.4.16 live CD (GPL, from reactos.org, with the source offer the licence asks). Windows guests only as "bring your own disk image": a file picker, the image stays in the tab, never uploaded. No proprietary driver or Windows image is ever bundled or linked.
- `PROVENANCE.md` and `CREDITS.md`: v86 (BSD-2-Clause, fetched by `tools/fetch-v86.sh` with SHA-256 sums as Quill does today, never committed), SeaBIOS and SeaVGABIOS (LGPL-3.0, source offered), the guest images and their licences, and the note that the chip model is written from Intel's AC'97 2.3 and 82801AA documents, not from QEMU's or VirtualBox's sources (both GPL).
- Tests: `bun test` (the model, the kit on a fake v86, the output on a fake audio context, the resampler's quality report, the microphone on fakes, the network-free scan), `tsc --noEmit`, and browser checks in headless Chromium and Firefox through `playwright-core`: each guest's cold boot plays a tone and records one, counted at the probe.
- CI, one job per check: headers, markdownlint, Prettier, typecheck, unit tests, the built modules' drift and budgets, the network-free scan, the Linux image build (cached, only when its config changes), the Chromium guest check, the Firefox guest check, the Pages build. Test results reach the pull request through `liminal-hq-labs/pr-results`, as in the other Labs repositories.
- Releases: a pushed `v*` tag builds the modules, attaches them with their SHA-256 sums and the Linux test image to a GitHub release, and stops. Tagging is publishing (Chapter 09), so it happens only on the owner's go. **npm publishing is a separate, owner-gated step**: the workflow has a manual `npm-publish` job with provenance, never run by an agent.
- Versioning: semver of the package's own API, with the v86 version it was tested against in `V86_VERSION`, in `peerDependencies` (`"v86": "0.5.473"`, exact while internals are used) and in the README; a v86 update is a minor release after the browser checks pass, never a silent range.

**Microphone privacy, as the package's contract (SITE-103).** Off by default; the package never calls `getUserMedia` except inside `startMicrophone()`, which the page must call from a click made for it (the package refuses when `navigator.userActivation.isActive` is false, where the browser has it); mono, no processing, nothing stored, uploaded or sent (the network-free scan stays a test); an `onMicrophone(state)` hook for the page's visible indicator, and the README requires one; every track ends on `stopMicrophone()`, `detach()`, a page hide or unload, and a track the browser ends; no permission is remembered by the package (the browser's own grant is the browser's).

## How Quill consumes it

**Choice: Quill keeps its copy until after the first demo, then vendors the built module with provenance** (SITE-104).

1. **Now to the launch.** `site/try-reactos/packages/ac97/` stays the source of truth; #198 merges there; the launch ships it. No extraction work touches Quill before the launch's smoke test passes (first-demo-launch.md).
2. **WP-AC97-1 to WP-AC97-5 run in the new repository**, seeded from Quill's package at a named commit (history kept with `git filter-repo --path site/try-reactos/packages/ac97`, so blame survives), and tag `v0.1.0` on the owner's go.
3. **WP-AC97-7 in Quill.** `packages/ac97/` is removed; `site/try-reactos/vendor/v86-ac97/` holds `v86-ac97.js`, `v86-ac97.d.ts` and `PROVENANCE.md` (the tag, the commit, the release asset's URL and SHA-256, what Quill changed: nothing). `sound.js` imports it. The CI job *AC'97 sound card tests* becomes *Vendored AC'97 module*: the file's SHA-256 equals the one in `PROVENANCE.md`, and its size stays within Quill's 36 KB budget (`site/try-reactos/tools/check-vendored.py`). `harness/check-sound.ts`, `check-mic.ts` and `check-state.ts` stay in Quill: they test Quill's page and state.
4. **A refresh** is one command (`tools/fetch-v86-ac97.sh <tag>`) that downloads the release asset, checks its sum and rewrites `PROVENANCE.md`; then the browser checks and a state rebuild if the card's saved shape changed (`isAc97State` version).

Why not a pinned npm dependency: the site has no install step and serves committed files (SITE-7's rule for v86 itself); an npm dependency would add a lockfile, a registry fetch in the Pages pipeline and a build step for one 36 KB file. Why not keep the source vendored: two places to fix a bug, and a drift check that compares two copies of source instead of one hash. Why not extract before the launch: the launch is on the critical path and the card is proven as it is; a move now risks the one thing the demo already does well.

## Other virtual hardware

The kit carries any device that is a PCI function or a set of ISA ports with an interrupt, reading and writing guest memory. Five candidates, short:

| Device | Value | Cost | Verdict |
|---|---|---|---|
| **USB host controller** (UHCI first) with emulated devices | High: an absolute pointer (a USB tablet ends the Follow and Capture compromise in the ReactOS demo), files in and out as a USB disk, a printer and a serial line every guest already has drivers for, and later real devices through WebUSB | High, but in steps; see [A USB host controller](#a-usb-host-controller) | **Next**, after WP-AC97-5 |
| **MPU-401 MIDI** (ports `0x330` and `0x331`, UART mode) to a small soft synth or to WebMIDI | Medium: ReactOS builds `mpu401` (`drivers/wdm/audio/drivers/mpu401`), Windows 9x and DOS games speak it; WebMIDI reaches the visitor's own synth (permission prompt) | Low for the port (UART mode is a byte pipe); medium for a synth with a General MIDI sound set (a licence to check) | Later; a good small second device |
| **Sound Blaster 16 through the shared output** (v86's own SB16, routed through the ring player and resampler, plus the OPL3 FM that v86 lacks) | Medium for DOS guests (v86 serves them, though Quill's DOS demo runs in js-dos) | Medium; an upstream change to v86 more than a page device | Offer upstream; not a page device |
| **Intel HD Audio** | Low now: it serves Windows Vista and later, which v86 runs poorly; every guest above already drives AC'97 | High: the CORB and RIRB rings, a codec graph of verbs, stream descriptors | No, until a guest needs it |
| **The host link as a device** (a PCI mailbox instead of COM1) | Low: every guest would need a new driver; COM1 needs none, and USB CDC and printer devices (below) give the same reach with drivers that exist | High (a driver per guest) | No; use USB CDC or the printer class instead |

## A USB host controller

The owner's addition, 2026-11-28: "Let's also add USB two or three." USB 2 (EHCI) and USB 3 (xHCI), with USB 1.1 (UHCI or OHCI) as the possible first step.

### What v86 has today

**No USB controller.** v86 0.5.473's `libv86.mjs` has no `usb`, `uhci`, `ohci`, `ehci` or `xhci` string, and `cpu.js`'s `init` builds only the PCI host and the PIIX3 ISA bridge, ACPI, RTC, DMA, VGA, PS/2, the VMware mouse port, four UARTs, two parallel ports, the floppy and IDE controllers, the PIT, NE2000 or virtio-net, virtio-9p, virtio-console, virtio-balloon and the SB16. v86's SeaBIOS build has `# CONFIG_USB is not set`, so the BIOS neither scans nor owns a USB controller (no handoff is needed), and v86's Buildroot kernel has `# CONFIG_USB_SUPPORT is not set`.

### The controllers

| Controller | PCI | Registers | Schedule | Code (TypeScript, estimate) | CPU |
|---|---|---|---|---|---|
| **UHCI** (USB 1.1, Intel) | Class 0C/03/00; as the PIIX3's own USB function 1.2, 8086:7020; I/O BAR at `0x20` (32 bytes); `LEGSUP` at config `0xC0`; pin D | `USBCMD`, `USBSTS`, `USBINTR`, `FRNUM`, `FLBASEADD`, `SOFMOD`, `PORTSC1`, `PORTSC2` | A 1,024-entry frame list in guest memory, walked once per 1 ms frame: queue heads and transfer descriptors (TDs) | 1,500 to 2,000 lines with the root hub | The frame walk runs whether or not a device is busy; batched (4 to 10 frames per timer tick, the frame number caught up to the clock), well under 1 ms of the page's thread per second at idle |
| OHCI (USB 1.1) | 0C/03/10; a 4 KB memory BAR | HCCA, operational registers | Endpoint and transfer descriptor lists, a periodic table in the HCCA | 2,000 to 2,500 | Like UHCI |
| **EHCI** (USB 2.0) | 0C/03/20 (for example 8086:24CD, ICH4); a memory BAR (capability and operational registers, about 256 bytes; one 128 KB block at a fixed address); `SBRN` and `FLADJ` at config `0x60` and `0x61`; `USBLEGSUP` extended capability | `USBCMD`, `USBSTS`, `USBINTR`, `FRINDEX`, `PERIODICLISTBASE`, `ASYNCLISTADDR`, `CONFIGFLAG`, `PORTSC[n]` | A periodic frame list (125 µs microframes) and a circular asynchronous list of queue heads and qTDs; full- and low-speed devices go to companion controllers (UHCI or OHCI) or behind a transaction translator, unless every emulated device is high-speed | 2,500 to 3,500 | The asynchronous list is polled continuously while enabled (guests leave it on); must be throttled: walk on the doorbell and on a timer only |
| **xHCI** (USB 3.x) | 0C/03/30; a 64 KB memory BAR; MSI or MSI-X preferred (v86 has neither for page devices: legacy INTx only) | Capability, operational, runtime (interrupters) and doorbell register sets; extended capabilities (supported protocols, legacy support) | Device context base array, device and endpoint contexts, a command ring, event rings with segment tables, a transfer ring per endpoint: event-driven, rung by doorbells | 5,000 to 8,000 | The cheapest at idle (nothing runs until a doorbell); the largest spec by far |

Shared by all of them: a USB device framework (descriptors, control transfers, `SET_ADDRESS`, `SET_CONFIGURATION`, strings, a hub's port status), about 600 lines, and the kit's PCI, interrupt and memory access. UHCI fits the kit as it is (an I/O BAR, like AC'97); OHCI, EHCI and xHCI need a memory BAR at a fixed address, because v86 will not move one.

### Guests and their drivers on a cold boot

| Guest | UHCI or OHCI | EHCI | xHCI | Device classes |
|---|---|---|---|---|
| Linux | `uhci-hcd`, `ohci-hcd` | `ehci-hcd` | `xhci-hcd` | HID, mass storage, printer (`usblp`), CDC serial (`cdc_acm`), audio (`snd-usb-audio`): all mainstream, if the kernel is configured (a test image again) |
| ReactOS 0.4.16 | `usbuhci`, `usbohci` (built) | `usbehci` (built) | **None in 0.4.16** (the tree's `drivers/usb` has no xHCI; ReactOS's first xHCI stack landed in later development builds) | HID (`hidusb`, `kbdhid`, `mouhid`), mass storage (`usbstor`), composite (`usbccgp`); **no printer class (`usbprint`) and no CDC serial (`usbser`)**; `usbaudio` is in the tree but not built. The QAGRR trim keeps every USB driver, and `qagrr/check-win16.sh` already drives the remix in QEMU with `-usb -device usb-tablet` (PIIX3 UHCI and a HID tablet) |
| Windows 98, ME | Inbox (USB 1.1) | No | No | HID inbox; mass storage inbox on ME, a vendor driver on 98; the printer class not confirmed |
| Windows 2000 | Inbox | SP4 | No | HID, mass storage, printer (`usbprint.sys`) inbox; CDC serial needs an INF for `usbser.sys` |
| Windows XP | Inbox | SP1 and later | No | As 2000 |
| Windows 7 | Inbox | Inbox | Vendor driver | As 2000 (v86 runs 7 poorly) |
| Windows 8 and later | Inbox | Inbox | Inbox | Plus CDC serial inbox on 10 (out of v86's reach in practice) |
| 9front (32-bit `pc` kernel) | `usbuhci`, `usbohci` | `usbehci` | No (not in the 32-bit kernel's sources) | HID, disk, serial through its user-space USB drivers |
| Haiku | `uhci`, `ohci` | `ehci` | `xhci` | HID, mass storage, CDC serial, audio |
| KolibriOS | UHCI, OHCI | EHCI | Untested | HID, mass storage |
| DOS | No (SeaBIOS's USB support is off in v86's build) | No | No | — |

### Emulated devices (backed by the page)

| Device | What it gives | Guests with a driver | Cost | Verdict |
|---|---|---|---|---|
| **HID tablet** (absolute pointer, full speed) | The pointer in the guest follows the visitor's exactly, no capture: the fix the pointer plan works around | Every guest above with USB | Low (one interrupt endpoint, a report descriptor) | **GO for the proof**: ReactOS, Linux, Windows 98 to XP, Haiku, 9front |
| HID keyboard | Keys the PS/2 path cannot send (media keys), little else | All | Low | Later, with the tablet's code |
| **Mass storage** (bulk-only transport, SCSI `INQUIRY`, `READ CAPACITY`, `READ(10)`, `WRITE(10)`) over a page-backed FAT image | Files in and out without the host link: the page builds a small FAT volume (the DOS bridge's `fat.ts` already does) and reads it back after the guest ejects | ReactOS (`usbstor`), Linux, Windows ME, 2000, XP, Haiku | Medium (900 lines and the image) | **GO** after the proof |
| **Printer class** (class 7, bidirectional) that spools each job to the page | Print from any application to a printer the guest already knows: Linux CUPS with a PostScript or PDF queue, Windows 2000 and XP with a PostScript driver, and the page turns the job into a PDF | Linux, Windows 2000 and XP (98 and ME not confirmed); **not ReactOS 0.4.16** (no `usbprint`) | Low for the device (250 lines), the job handling is shared with the host-link guests plan | **GO on paper for Linux, Windows 2000 and XP; NO-GO on ReactOS 0.4.16.** Coordinate with `docs/plans/host-link-guests.md` (another planner is writing it): that plan owns the print job channel and its PDF conversion; this device is one more transport into it |
| **CDC serial** (ACM) | A host-link transport with no custom driver on Linux and Haiku | Linux (`cdc_acm`), Haiku; Windows 2000 and XP with an INF; **not ReactOS** | Medium (400 lines) | **GO for Linux**; COM1 stays the link on ReactOS |
| Audio class | An alternative to AC'97 | Linux, Haiku; not ReactOS 0.4.16 (`usbaudio` not built) | High: isochronous transfers in the controller | **No**: AC'97 already covers these guests |
| Video class (a webcam from `getUserMedia`) | A camera in the guest | Linux, Windows XP SP2 and later | High (isochronous, UVC descriptors) and a privacy surface | **No** for now |

### Real devices through WebUSB

- **Where.** Chromium-based browsers only (Chrome, Edge, Opera, desktop and Android), in a secure context only: the Quill site's HTTPS must work first (web-demo.md's open item). Not in Firefox or Safari.
- **What it can reach.** The spec's protected interface classes cannot be claimed: audio (`0x01`), HID (`0x03`), mass storage (`0x08`), hub (`0x09`), smart card (`0x0B`), video (`0x0E`), audio and video (`0x10`) and wireless controller (`0xE0`) (WICG `webusb/index.bs`, *Protected interface classes*). The printer class (`0x07`) and CDC (`0x02`, `0x0A`) are not protected, but the visitor's own system usually holds them (Linux's `usblp` and `cdc_acm`, Windows without WinUSB), so `claimInterface` fails; in practice WebUSB reaches vendor-class (`0xFF`) devices and devices that ship WinUSB descriptors. Chromium also blocks some devices by id (security keys).
- **The other ways in.** Web Serial reaches serial ports the system already drives (a real CDC device included), so a real serial device goes to the guest as an emulated CDC device or as COM2; WebHID reaches HID devices except the ones it blocks (keyboards, mice and security keys among them).
- **Safety, as the package's default.** Never attach on its own: a device goes to the guest only after the visitor picks it in the browser's chooser from a click made for it, and the package never re-attaches a device the browser remembers (`navigator.usb.getDevices()`) without a new click; a visible indicator per attached device (an `onDevices` hook the page must show); detached on Stop, Reset, a page hide or unload and when unplugged; nothing sent anywhere but the guest.
- **Verdict.** Later and optional (WP-USB-6), after the emulated devices; most visitors' devices are in the protected classes.

### Verdicts per guest and device class

| | HID tablet | Mass storage | Printer | CDC serial | Audio class | WebUSB pass-through |
|---|---|---|---|---|---|---|
| ReactOS 0.4.16 (UHCI) | **Untested, likely GO** (QEMU's usb-tablet passes Quill's QAGRR check) | Untested, likely GO | **NO-GO** (no driver) | **NO-GO** (no driver) | NO-GO (not built) | Untested |
| Linux (UHCI, EHCI, xHCI) | Untested, GO on paper | Untested, GO on paper | Untested, GO on paper | Untested, GO on paper | Untested | Untested |
| Windows 98, ME (UHCI) | Untested | ME only | Untested | No | No | Untested |
| Windows 2000, XP (UHCI, EHCI) | Untested | Untested | Untested | With an INF | Untested | Untested |
| Haiku | Untested | Untested | Untested | Untested | Untested | Untested |
| 9front | Untested | Untested | — | Untested | — | Untested |
| DOS | NO-GO | NO-GO | NO-GO | NO-GO | NO-GO | NO-GO |

### Build order

**Recommended: UHCI and a HID tablet first** (WP-USB-1), then the device framework's second device, mass storage (WP-USB-2), the printer class (WP-USB-3, with the host-link guests plan) and CDC serial (WP-USB-4), then EHCI (WP-USB-5) when mass storage wants the speed, WebUSB pass-through (WP-USB-6), and xHCI last or never (WP-USB-7). Why not the owner's suggested minimal EHCI with a CDC or printer device as the proof:

- UHCI is the PIIX3's own USB function, so the machine stays the 440FX and PIIX3 v86 already is; it uses an I/O BAR, which the kit already moves correctly; EHCI needs a memory BAR at a fixed address in a 128 KB block, which v86 will not move.
- UHCI has drivers in every guest that has USB at all, Windows 98 and ReactOS included; EHCI excludes Windows 98 and ME and needs SP1 on XP.
- The HID tablet has drivers in every guest, ReactOS included, and fixes the demo's pointer; the printer and CDC devices have no ReactOS driver, so a proof built on them could not run on the one guest this project ships.
- QEMU's `-usb -device usb-tablet` is the same pair, and Quill's own QAGRR check already shows ReactOS driving it.

If the owner still wants EHCI first, the swap is WP-USB-1's controller line (EHCI with a high-speed tablet and no companion); the device framework and the tablet are the same.

### Proposing USB upstream

A USB stack is a large addition for v86 to carry, and v86's maintainers keep the emulator small. The plan does not propose USB to v86 as code. It proposes the device API above (the `devices` option with memory BARs that move and a `save_state` slot), which USB needs more than the card does: memory BARs, timers and state. With that API in v86, `v86-usb` stays a page package; without it, the kit's fixed-address workaround is the fallback.

## Risks

- **v86 internals drift.** Every line of the internals table can break on a v86 update; the BAR ordering rule breaks silently. Mitigation: the exact pin, `V86_VERSION` checked against `fetch-v86.sh` in tests, the kit as the only file that knows v86, browser checks per guest before a version moves, and the upstream API proposal.
- **Shared interrupt lines.** v86's PIC and I/O APIC hold one level per line, not per device; two PCI functions on one PIRQ (the card on line 11 beside a network card or the USB controller) could see one device's `lower_irq` drop the other's request. A test with the NE2000 and the card on the same line comes first in WP-AC97-1; if it fails, the kit keeps its own per-line count of raised devices and lowers only at zero.
- **Browser autoplay and the AudioWorklet.** A context made outside a gesture stays suspended (the card drains, the guest runs); the worklet needs a secure context, so an HTTP page gets the deprecated `ScriptProcessorNode`; Safari's worklet timing is unmeasured.
- **Latency.** The ring holds 120 ms today; the clocked DMA (item 1) makes the ring the only buffer, so latency can drop toward 40 ms, at more risk of underruns on a busy page thread.
- **CPU on phones.** Resampling costs 2.5 to 2.9 ms a second on a workstation; a phone may be five to ten times slower, and USB's frame walk adds to it. Budgets per device and a measure on a real phone before any "works on mobile" claim.
- **Guest driver hangs.** A driver that waits on a bit the model never sets hangs the guest's boot (Linux probes with timeouts; older Windows drivers may not). The trace option and a per-guest register log in `docs/guests.md`; `detach()` and the reboot reset never leave the interrupt line up.
- **The microphone.** A page that wires `startMicrophone()` to page load would break the promise; the package refuses without a user activation where the browser reports it, documents the rule, and keeps the network-free scan.
- **Licences.** Windows images and vendor drivers are never bundled or linked; the Linux test image carries its sources; SeaBIOS stays LGPL with its source offered.
- **USB's spec surface.** The schedules are large and guests use them in different ways; a USB package that tries every controller at once will stall. One controller and one device per package, each with its own guest checks.
- **Names.** `v86-devices`, `v86-ac97`, `v86-device-kit` and `v86-usb` are free on npm and GitHub today and may not be when the owner publishes.

## Decisions to record

Written to `docs/decisions.md` with this plan, all **Assumed** until the owner confirms. Spare: SITE-108 and SITE-109.

| Id | Decision |
|---|---|
| SITE-100 | The AC'97 card leaves Quill as `liminal-hq-labs/v86-devices`, with `v86-device-kit` and `v86-ac97` as its first packages; the host link is its own repository (SITE-129) |
| SITE-101 | The card's base path is a cold boot; a saved state's registers are an option; a guest reboot resets the card |
| SITE-102 | The card's DMA position follows a clock, not the output's pulls |
| SITE-103 | The microphone's safe path is the package's default contract |
| SITE-104 | Quill ships its own copy through the first demo, then vendors the built module with provenance |
| SITE-105 | A public device API for v86 is drafted as an issue and opened only on the owner's go |
| SITE-106 | USB starts with a UHCI controller and a HID tablet; EHCI follows, xHCI last |
| SITE-107 | Real USB devices reach the guest only through the browser's chooser, never on their own |

## Order and parallelism

| Step | Package | Where | Waits for | Parallel with |
|---|---|---|---|---|
| 0 | The first demo's launch; #198 merged | Quill | — | — |
| 1 | WP-AC97-1 The new repository and the kit | `v86-devices` (created by the owner or on the owner's go) | The launch, and the owner's go to create the repository | — |
| 2 | WP-AC97-2 The clocked DMA, the reboot reset, the codec masks | `v86-devices` | WP-AC97-1 | WP-AC97-3 |
| 2 | WP-AC97-3 The Linux test image and its harness | `v86-devices` | WP-AC97-1 | WP-AC97-2 |
| 3 | WP-AC97-4 The guest checks (Linux, ReactOS live CD, then 9front and Haiku) | `v86-devices` | WP-AC97-2, WP-AC97-3 | WP-AC97-5 |
| 3 | WP-AC97-5 The demo page, the docs, the release workflow | `v86-devices` | WP-AC97-2 | WP-AC97-4 |
| 4 | WP-AC97-6 The upstream device API draft (text only) | this plan's folder | WP-AC97-4 | anything |
| 5 | WP-AC97-7 Quill vendors `v86-ac97` v0.1.0 | Quill | the owner's `v0.1.0` tag | WP-USB-1 |
| 5 | WP-USB-1 UHCI and a HID tablet (spike, then package) | `v86-devices` | WP-AC97-1's kit | WP-AC97-7 |
| 6 | WP-USB-2 Mass storage | `v86-devices` | WP-USB-1 | WP-USB-3 |
| 6 | WP-USB-3 Printer class | `v86-devices` | WP-USB-1; the host-link guests plan's job channel | WP-USB-2 |
| 7 | WP-USB-4 CDC serial | `v86-devices` | WP-USB-1 | WP-USB-5 |
| 7 | WP-USB-5 EHCI | `v86-devices` | WP-USB-2 | WP-USB-4 |
| 8 | WP-USB-6 WebUSB pass-through | `v86-devices` | WP-USB-1, the site's HTTPS | — |
| — | WP-USB-7 xHCI | `v86-devices` | WP-USB-5 and a guest that needs it | — |

At most three builders at once (the house limit); the packages that share a file never run together (WP-AC97-2 owns `packages/ac97/src/model.ts`; WP-AC97-4 only reads it).

## Work packages

### WP-AC97-1 The new repository and the attach kit

- **Finish line.** `liminal-hq-labs/v86-devices` exists from the Labs template with the settings above (hero, labels, topics, merge settings, Dependabot off); `packages/device-kit` holds `onceBeforeInit`, `registerPci`, the interrupt line with a per-line count, guest memory access, the reboot reset and the late-attach report; `packages/ac97` is Quill's package at a named commit with its history, rewired onto the kit, its tests passing, the built module within 36 KB; CI has one job per check and is green.
- **Files owned.** The whole new repository.
- **Must not touch.** Quill (read only), v86's files (fetched, never committed or changed).
- **Tests.** Every test Quill's package has (87 with #198); the kit on a fake v86 (slot choice, port entries before the function, a moved BAR, the reboot wrap, attach after init reports late, detach undoes every wrap); the shared-line test (two fake devices on one PIRQ).
- **Evidence.** The hero frames, the CI run, `bun test` output, in `~/Documents/Liminal HQ/v86-devices/WP-AC97-1/`.
- **Parallel.** None; it is the base.

### WP-AC97-2 The clocked DMA position, the reboot reset and the codec masks

- **Finish line.** Items 1 to 5 of [What the model must add](#what-the-model-must-add): `CIV`, `PICB` and completions follow a clock at the guest's rate; a guest reboot resets the card and drops its line; codec registers keep only a STAC9700's bits; a cold reset through global control works; `subsystem` and `slot` options. ReactOS's *Critical Stop* still plays 39,045 frames in 177 buffers through `check-sound.ts`'s equivalent here.
- **Files owned.** `packages/ac97/src/model.ts`, `index.ts`, their tests; `packages/device-kit/src/reset.ts`.
- **Must not touch.** `sink.ts`, `resample.ts`, `capture.ts` except their call sites.
- **Tests.** A fake driver that measures the clock as Linux does (start, wait 50 ms of the fake clock, read `PICB`) gets 48,000 within 0.5 percent with a live output, a suspended one and none; the volume-resolution probe reads five bits; `0x2A` never sets bit 7; reboot mid-sound leaves the channel halted and the line low.
- **Evidence.** The measured clock in each mode, a WAV of the ReactOS sound after the change.
- **Parallel.** With WP-AC97-3.

### WP-AC97-3 The Linux test image and its harness

- **Finish line.** A 32-bit Linux image built from `chschnell/v86-buildroot` (pinned commit) with `CONFIG_SND_INTEL8X0=y`, `CONFIG_SND_AC97_CODEC=y`, `alsa-utils` and a serial console, under 16 MB, reproducible from `images/linux/` with its config and the GPL source tarball as a release asset; a harness that cold-boots it with the card and drives `aplay`, `speaker-test` and `arecord` over the serial line.
- **Files owned.** `images/linux/**`, `harness/linux.ts`.
- **Must not touch.** `packages/**`.
- **Tests.** The image builds twice to the same SHA-256; the harness boots to a shell in Chromium.
- **Evidence.** Boot log, `/proc/asound/cards`, the `dmesg` lines of the probe.
- **Parallel.** With WP-AC97-2.

### WP-AC97-4 The guest checks

- **Finish line.** Each guest's row in `docs/guests.md` is measured, in this order: Linux (playback at 8,000, 22,050, 44,100 and 48,000 Hz with pitch within 0.5 percent, a recording from Chromium's fake microphone at 22,050 Hz), the ReactOS 0.4.16 live CD on a cold boot (the Sounds tab), then 9front and Haiku from the copy.sh images. Each verdict becomes GO or NO-GO with numbers, or stays untested with the reason.
- **Files owned.** `harness/guests/**`, `docs/guests.md`.
- **Must not touch.** `packages/**` (a needed model change goes back to WP-AC97-2's owner as a finding).
- **Tests.** `harness/check-guest.ts --guest linux|reactos|9front|haiku` in Chromium and Firefox.
- **Evidence.** Screenshots of each guest's sound settings, the counts, a WAV per guest.
- **Parallel.** With WP-AC97-5.

### WP-AC97-5 The demo page, the docs and the release workflow

- **Finish line.** `demo/index.html` cold-boots the Linux image or the ReactOS live CD with the card, with Sound and Microphone buttons and the indicator, and a "bring your own disk" picker; the docs listed above; a `v*` tag workflow that builds and attaches the modules and the image and stops; a manual, owner-only npm job; Pages publishes the demo.
- **Files owned.** `demo/**`, `docs/**`, `README.md`, `.github/workflows/release.yml`, `.github/workflows/pages.yml`.
- **Must not touch.** `packages/**`, `images/**`.
- **Tests.** The demo's own browser check (start, sound counted, mute, microphone on and off, every track ended, no request off the origin).
- **Evidence.** The demo page's screenshots on a desktop and at phone width.
- **Parallel.** With WP-AC97-4.

### WP-AC97-6 The upstream device API draft

- **Finish line.** `docs/upstream-device-api.md` in `v86-devices`: the proposed `devices` option, the host object's methods, how a device's state rides in `save_state`, a migration path for `v86-ac97`, and the issue text for copy/v86. Not opened.
- **Files owned.** That file.
- **Parallel.** With anything.

### WP-AC97-7 Quill vendors v86-ac97

- **Finish line.** As [How Quill consumes it](#how-quill-consumes-it) step 3: `site/try-reactos/vendor/v86-ac97/` with the release's module, types and `PROVENANCE.md`; `packages/ac97/` and its CI job removed; the *Vendored AC'97 module* job checks the hash and the 36 KB budget; `check-sound.ts`, `check-mic.ts`, `check-state.ts` and `check-power.ts` pass; the saved state is rebuilt if the card's state version changed; SITE-104 marked **Decided** when the owner agrees.
- **Files owned.** `site/try-reactos/vendor/v86-ac97/**`, `site/try-reactos/sound.js`, `site/try-reactos/tools/check-vendored.py`, `site/try-reactos/tools/fetch-v86-ac97.sh`, the CI job, `site/try-reactos/v86/PROVENANCE.md`'s internal-API paragraph, `site/try-reactos/README.md`'s *Sound* section.
- **Must not touch.** `try-reactos.js` beyond the import line, `qagrr/**`, `guest/**`.
- **Parallel.** With WP-USB-1.

### WP-USB-1 UHCI and a HID tablet

- **Finish line.** Part 1, a spike with a yes or no and numbers: a UHCI controller as PCI 8086:7020 (function 1.2 beside the PIIX3, or the first free slot if v86 refuses a function there) with two root ports and a full-speed HID tablet registered from the page, in v86 0.5.473 unchanged; the Linux test image (rebuilt with `CONFIG_USB_UHCI_HCD` and `CONFIG_USB_HID`) and ReactOS 0.4.16 each enumerate it on a cold boot and move the pointer to absolute positions the page sends; the page's thread cost at idle and while moving. Part 2, if go: `packages/usb` with the controller, the device framework and the tablet, tests, a built module within the budget the spike sets.
- **Files owned.** `packages/usb/**`, `harness/usb/**`, `images/linux/` (the USB options only, coordinated with WP-AC97-3's owner).
- **Must not touch.** `packages/ac97/**`; the kit except additions reviewed with its owner.
- **Tests.** The controller's registers and the frame walk in Node (reset values, `FRNUM` advancing with the clock, TD status and short packets, interrupts on completion, port connect and reset); the device's descriptors and reports against the USB HID 1.11 tables; in Chromium: Linux's `lsusb` and `evtest` lines, ReactOS's Device Manager and a click at a known pixel.
- **Evidence.** `~/Documents/Liminal HQ/v86-devices/WP-USB-1/`: the enumeration logs, a pointer path drawn in each guest, the CPU numbers.
- **Parallel.** With WP-AC97-7.

### WP-USB-2 to WP-USB-7

- **WP-USB-2 Mass storage.** Bulk-only transport and the SCSI commands above over a page-backed FAT image (the DOS bridge's `fat.ts` as the model for building and reading one, copied with provenance); files in and out on ReactOS and Linux; eject before the page reads.
- **WP-USB-3 Printer class.** Class 7, bidirectional, a device id string for a PostScript printer; each job goes to the host-link guests plan's print job channel; Linux CUPS and Windows XP print a test page that arrives as a PDF. Its brief is written with that plan's owner.
- **WP-USB-4 CDC serial.** ACM with a host-link transport on Linux and Haiku; an INF for Windows 2000 and XP written, not bundled with any Microsoft file.
- **WP-USB-5 EHCI.** A memory BAR at a fixed address, the asynchronous and periodic schedules with throttled walks, UHCI as the companion; mass storage at high speed.
- **WP-USB-6 WebUSB pass-through.** The chooser, the indicator, detach rules, vendor-class devices only in practice; Chromium only.
- **WP-USB-7 xHCI.** Only when a guest in v86 needs it.

## Builder briefs

### Common preamble

```text
You are a Sonnet builder for v86-devices (liminal-hq-labs/v86-devices). Work in ~/source/worktrees/v86-devices/<topic> (branch <type>/<topic> from origin/main); the main checkout is touched only by `git fetch` and `git worktree add/remove`. Read AGENTS.md, SPEC.md and Quill's docs/plans/v86-ac97-reuse.md (the plan) first. One PR; do NOT merge.

Rules: never push to main, tag, publish to npm or dispatch workflows (pushing the PR branch is allowed); never `git add -A` or `git commit -a`; no wildcard rm; clean up with plain commands and full literal paths, one per command, `ls` first in a separate command; kill only your own PIDs, never pkill or killall by name. Browser checks use your own static server on 127.0.0.1 with a PID file (never ports 8095, 8096, 8097 or 8098) and your own Xvfb when a display is needed, with DISPLAY set explicitly in EVERY command and `unset WAYLAND_DISPLAY`; never the host desktop. Commits via `git commit -F`, Conventional Commit subject, a markdown body with bold labels and no headings, hard-wrapped at 100, trailer `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`. PR title capitalized imperative with no prefix; labels from `gh label list`; body `## Summary` with bold lead-in bullets and `## Test plan` ending with an unchecked "Not verified:" line, written to a uniquely named file under /tmp/claude-1000/ for `gh pr create --body-file`. Rebase and force-with-lease, never merge main in. Canadian English; SPDX headers on every source, script, YAML, TOML and SVG file; update SPEC, docs and decisions in the same change. Keep `df -h /home` above 20 GB free. Guest images and caches go in ~/.cache/v86-devices/; evidence in ~/Documents/Liminal HQ/v86-devices/<WP>/.

Report: PR URL, sizes before and after, tests run, not verified, "OWNER DECISION: question | default taken | where to swap" lines.
```

### WP-AC97-1 The new repository and the attach kit (full brief)

```text
<the common preamble>

Build WP-AC97-1 exactly as the plan's "WP-AC97-1 The new repository and the attach kit" says. The owner has created liminal-hq-labs/v86-devices from labs-template (or said you may: `gh repo create liminal-hq-labs/v86-devices --public --template liminal-hq-labs/labs-template --description "Virtual hardware for the v86 emulator"`); if neither is true, stop and report.

1. Settings (labs-template docs/new-project.md, steps 2 to 6): run scripts/init-project.sh v86-devices "<the description>" --title "v86 devices" on chore/init-project and merge it as its own PR first; remove rustfmt.toml and the Rust jobs; labels from Mailslot plus documentation, architecture, audio, usb; topics liminal-hq-labs, experimental, typescript, v86, emulation, web-audio; merge commits only, delete branch on merge, read-only workflow token, secret scanning and push protection; no dependabot.yml.
2. Hero: assets/hero.svg as a scene of its own (a sound card's edge connector and a waveform; GNOME-style window chrome if a window appears, never macOS traffic lights) in a palette no other Labs repository uses (look at each repo's assets/hero.svg first and list their palettes in the PR); the SVG is the master; `bash scripts/check-banner.sh --shots /tmp/claude-1000/v86-devices-shots` and copy the frames to the evidence folder.
3. Seed packages/ac97 from Quill with history: in a scratch clone (never the Quill checkout), `git filter-repo --path site/try-reactos/packages/ac97 --path-rename site/try-reactos/packages/ac97/:packages/ac97/` at Quill's main after #198 merged (name the commit in PROVENANCE.md), then merge that history into the new repo with a merge commit. Fetch v86 0.5.473 with tools/fetch-v86.sh copied from Quill's site/try-reactos/tools/fetch-v86.sh (same sums; binaries never committed; licences and v86.d.ts committed).
4. packages/device-kit: move onceBeforeInit and the PCI, port, interrupt and memory parts of packages/ac97/src/v86.ts into it (src/attach.ts, src/pci.ts, src/irq.ts with a per-line raised count, src/reset.ts wrapping cpu.reboot_internal, src/version.ts with V86_VERSION); `stats().late` when attached after init. packages/ac97 imports the kit and the build bundles it (no runtime dependency in the built module).
5. Builds: packages/ac97/build.ts writes dist/v86-ac97.js (36 KB budget, --check for drift against the committed copy) and dist/v86-ac97.d.ts; packages/device-kit/build.ts writes dist/v86-device-kit.js (4 KB).
6. Tests: every Quill test still passes under bun test; new kit tests on a fake v86: slot choice, port entries registered before register_device, a BAR moved by a fake BIOS moves the handlers, the reboot wrap calls reset once per reboot, attach after init reports late, detach undoes every wrap and leaves no line raised, two devices on one PIRQ (one lowering does not drop the other's request); a test that V86_VERSION equals tools/fetch-v86.sh's pin; the network-free scan over every package's src.
7. CI (.github/workflows/ci.yml), one job per check: headers, markdownlint, Prettier, typecheck per package, bun test per package, built-module drift and budget per package, the network-free scan; results to the PR through the pr-results publisher as Quill does (read Quill's .github/workflows/ci.yml and publish-test-results.yml).
8. Docs: SPEC.md (the kit's and the card's behaviour), docs/decisions.md (prefix VD-, starting at VD-1: the repository, the kit, the budgets), docs/progress.md, PROVENANCE.md (Quill's commit, v86, SeaBIOS) and CREDITS.md, README with the ten-line example from the plan and Status: experimental.

Budgets: v86-ac97.js 36 KB, v86-device-kit.js 4 KB. Out of scope: the clocked DMA, the codec masks and every model change (WP-AC97-2), guest images (WP-AC97-3), the demo page (WP-AC97-5), any change to Quill, any npm publish or tag.
Evidence: ~/Documents/Liminal HQ/v86-devices/WP-AC97-1/ (hero frames, the CI run's summary, bun test output). Say what each shows and what it does not verify.
```

### WP-AC97-2 to WP-AC97-7 and WP-USB-1

Each is written from its section above with the common preamble, by the lead who starts it, once the package before it has merged: WP-AC97-2 owns the model, so its brief names the fake Linux clock test first; WP-USB-1's brief is a spike brief (finish line part 1) whose go or no-go decides part 2.

## Owner decisions

Each is taken with the default so nothing waits; the last column is where to change it.

| Question | Default taken | Where to swap |
|---|---|---|
| One repository for the family, or one per device? | One, `liminal-hq-labs/v86-devices`, with `v86-device-kit`, `v86-ac97` and later `v86-usb` | SITE-100; the package table; WP-AC97-1 |
| The npm names: unscoped or under a scope? | Unscoped `v86-ac97` and `v86-device-kit` (the v86 ecosystem's own naming; both free today); publishing is the owner's step | SITE-100; WP-AC97-5's npm job |
| When to extract | After the first demo's launch; Quill ships its own copy | SITE-104; the order table |
| How Quill consumes it | A vendored built module with `PROVENANCE.md` and a hash check, no npm dependency | SITE-104; WP-AC97-7 |
| Which guests the demo page hosts | The Linux test image and the ReactOS 0.4.16 live CD; Windows only as the visitor's own disk, never hosted | WP-AC97-5 |
| The card's PCI subsystem id | 8086:0000, with an option; not QEMU's 1AF4:1100 (that would switch Linux's clock measurement off by claiming to be QEMU) | SITE-102; WP-AC97-2 |
| Open the v86 device API issue? | Draft only; not opened | SITE-105; WP-AC97-6 |
| The first USB controller | UHCI with a HID tablet (the owner suggested a minimal EHCI with a CDC or printer device) | SITE-106; WP-USB-1's controller line |
| WebUSB pass-through | Later, optional, Chromium only, chooser and indicator required | SITE-107; WP-USB-6 |
| xHCI | Last, only when a guest needs it | WP-USB-7 |

## What was not verified

- That any guest but ReactOS drives the card: Linux, 9front, Haiku, KolibriOS, SerenityOS, the BSDs and Windows are judged from their drivers' sources and from QEMU and VirtualBox emulating the same chip and codec. No spike was run.
- That Linux's clock measurement retunes the clock with today's drain path: reasoned from `intel8x0_measure_ac97_clock` and the drain's 10 ms steps, not measured.
- That Windows XP's inbox driver binds 8086:2415 with subsystem 8086:0000, and whether Windows 2000 has an inbox driver at all.
- The cold boot with the card that the orchestrator's builder is measuring for the sound preview (`~/Documents/Liminal HQ/Quill/Web demo/Sound demo/`): its README had not appeared when this plan was written; WP-AC97-4 takes its numbers when it does.
- Every USB verdict: no controller was built. ReactOS's UHCI and HID tablet support is from QEMU (Quill's QAGRR check), not v86.
- The shared-interrupt-line behaviour in v86's PIC and I/O APIC with two PCI devices on one line.
- CPU on a phone, Safari, a hidden tab, real speakers and a real microphone (as in WP-WEB-5 and WP-WEB-5b).
- That the names are still free when the owner creates the repository or publishes.
