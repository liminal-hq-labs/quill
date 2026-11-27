# ac97

A virtual Intel AC'97 sound card for [v86](https://github.com/copy/v86), written for the Quill ReactOS demo (SITE-41). ReactOS's own `ac97.sys` drives it, so a guest that plays a sound reaches the visitor's speakers through Web Audio. It has no runtime dependency, and no v86 or browser dependency in its chip model.

v86 has no sound card of its own beyond the PC speaker. This package adds one from the page, to v86 0.5.473 unchanged, through v86's internal PCI, I/O port, interrupt and memory API (the device is registered by wrapping `emulator.v86.init`, so it exists before the BIOS enumerates PCI and before a saved state is restored). The WP-WEB-4 spike proved the approach and its numbers are in `docs/plans/web-demo.md`; SITE-19 records the decision.

## API

```ts
import { attachAc97, isAc97State, gainFor } from "./lib/ac97.js";

// In a click handler: browsers start sound only from a user gesture.
const context = new AudioContext({ sampleRate: 48000, latencyHint: "interactive" });
void context.resume();

const emulator = new V86({ /* autostart, initial_state, ... */ });
const card = attachAc97(emulator, {
	state: savedRegisters,   // what getAc97State() returned beside the machine state being restored; leave out for a cold boot
	volume: 0.7,             // 0 to 1, a squared taper (gainFor)
	muted: false,
	context,                 // omit to let the package make one (suspended until resume()); null plays nowhere
	onChange: () => {},      // output ready or failed, context state changed, card registered
});

card.getAc97State();         // the card's registers (JSON-safe), or null before it is registered. Save while no sound plays
card.setVolume(0.5);
card.setMuted(true);
await card.resume();         // from a gesture: starts or resumes the output; resolves with its state
card.outputState();          // "none" | "failed" | "suspended" | "running" | "closed" | "interrupted"
card.stats();                // registered, pciId, guestRate, playing, counters (frames taken, buffers, interrupts), drainedFrames, gain ...
await card.sinkStats();      // what the output played: frames, silent frames, peak, underruns
card.detach();               // silences, stops answering the guest, lets the machine go; safe twice
```

`attachAc97` must be called after `new V86(...)` and before the machine runs. It registers the card when v86 builds its devices a moment later. `state` is checked with `isAc97State`; a state of another shape is refused (`stats().stateError`) and the card starts as if on a cold boot.

**A saved state and the card go together.** A state saved with the card has its PCI function in the saved bus, so it must be restored with the card attached; a state saved without it must be restored without. The page keeps the registers in the manifest's `audio` object, and adds the card to a restored machine only when that object is there (SITE-43).

**The guest never waits on the speakers.** When nothing is pulling sound (no context, a context the browser holds suspended, a hidden tab), the card's buffers are still taken at the real-time rate and dropped (`stats().drainedFrames`), so a guest that plays a sound and waits for it finishes on time.

**Without an AudioWorklet** (an insecure origin or an old browser) the output falls back to a `ScriptProcessorNode`, which is deprecated but present in every browser (`card.sink.kind`).

## Layout

| File | What |
|---|---|
| `src/model.ts` | The chip: ICH bus-master registers, the AC'97 2.3 codec, descriptor-list walking, status bits, the interrupt line, resampling and the codec's volume. No emulator, no browser |
| `src/v86.ts` | The v86 glue: PCI space, I/O port handlers, `onceBeforeInit`, `registerOn`. The only file that knows v86's internals, pinned with v86 |
| `src/sink.ts` | The output: an `AudioWorklet` ring player that counts what it plays (and can record it), or a script-processor fallback |
| `src/index.ts` | `attachAc97` and the public types |
| `test/` | `bun test`: the register model, the glue on a fake v86, the output on a fake audio context |
| `build.ts` | Builds `../../lib/ac97.js` (`--check` fails on drift; 24 KB budget) |

## Use

```sh
bun install
bun test
bun run typecheck
bun run build        # writes site/try-reactos/lib/ac97.js; commit it
bun run check:built  # what CI runs
```

The page's glue is `site/try-reactos/sound.js`; its checks in a real browser are `harness/check-sound.ts` (the guest plays Critical Stop and an AudioWorklet probe after the volume node hears it) and the sound lines of `harness/check-power.ts`.

## What the chip answers

The card is PCI 8086:2415 (82801AA AC'97 audio) in the first free slot from 3, with a mixer BAR of 256 bytes and a bus-master BAR of 64 bytes, interrupt pin A. The codec answers as a plain AC'97 2.3 codec with variable rate audio (the vendor ID is a SigmaTel STAC9700's). Only the PCM-out channel plays; PCM-in and mic-in answer their registers and never run. The interrupt is level-style: raised when an enabled status bit is set, lowered when the guest clears it.

## Tested with

- v86 0.5.473 (`site/try-reactos/v86/PROVENANCE.md`; `V86_VERSION` is exported and a test keeps it equal to `tools/fetch-v86.sh`'s pin), ReactOS 0.4.16 with its `ac97.sys`, in headless Chromium driven by `playwright-core`.
- Bun 1.4.4 for the tests and the build, TypeScript 5.9.3 for the type check.
- Not run: Firefox, Safari, mobile browsers, a hidden tab, real speakers. See the *Sound* section of `site/try-reactos/README.md`.
