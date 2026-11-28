# ac97

A virtual Intel AC'97 sound card for [v86](https://github.com/copy/v86), written for the Quill ReactOS demo (SITE-51). ReactOS's own `ac97.sys` drives it, so a guest that plays a sound reaches the visitor's speakers through Web Audio, and a guest that records hears the visitor's microphone when the visitor turns it on (and silence otherwise). It has no runtime dependency, and no v86 or browser dependency in its chip model.

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
card.startMicrophone();      // from a click: asks the browser for the microphone and feeds the guest's recording; resolves with its state
card.stopMicrophone();       // ends every track; the guest records silence again
card.micState();             // "off" | "asking" | "on" | "denied" | "no-device" | "insecure" | "unavailable" | "error" (micReason() says why)
card.detach();               // silences, stops the microphone, stops answering the guest, lets the machine go; safe twice
```

`attachAc97` must be called after `new V86(...)` and before the machine runs. It registers the card when v86 builds its devices a moment later. `state` is checked with `isAc97State`; a state of another shape is refused (`stats().stateError`) and the card starts as if on a cold boot.

**A saved state and the card go together.** A state saved with the card has its PCI function in the saved bus, so it must be restored with the card attached; a state saved without it must be restored without. The page keeps the registers in the manifest's `audio` object, and adds the card to a restored machine only when that object is there (SITE-53).

**The guest never waits on the speakers.** When nothing is pulling sound (no context, a context the browser holds suspended, a hidden tab), the card's buffers are still taken at the real-time rate and dropped (`stats().drainedFrames`), so a guest that plays a sound and waits for it finishes on time.

**Without an AudioWorklet** (an insecure origin or an old browser) the output falls back to a `ScriptProcessorNode`, which is deprecated but present in every browser (`card.sink.kind`).

## Recording and the microphone

The PCM-in and mic-in channels record: the guest sets a buffer list, the rate (the ADC rate register, 0x32, when variable rate is on), the record select (0x1A: the microphone, or any other source, which records silence) and the record gain (0x1C: 0 to 22.5 dB in 1.5 dB steps; muted at reset, as on a real codec, so the driver or mixer unmutes it), and runs the channel. The card fills the guest's buffers a buffer at a time and raises its interrupt as playback does, waiting for the guest to take each buffer's interrupt before the next completes (two completions merging into one interrupt confuses `ac97.sys`).

- **The guest always has an input.** With the microphone off the card records silence (zeros) at the real-time rate, so a recording program finds a working device and finishes on time.
- **The microphone is the visitor's to turn on.** The package asks the browser for it only when `startMicrophone()` is called, which the page does only from a press of its Microphone button. It asks for a mono stream with no echo cancelling, noise suppression or automatic gain, listens through an `AudioWorklet` that mixes to mono and hands 10 ms blocks to the card, and resamples them (the band-limited resampler below) to the guest's record rate. Nothing it hears is kept, stored, uploaded or sent: it goes into the emulated card's memory in the page, and a test scans the package's source for any way to send audio anywhere (`fetch`, `XMLHttpRequest`, `sendBeacon`, `WebSocket`, `RTCPeerConnection`, `MediaRecorder`, web storage). `stopMicrophone()`, `detach()` and a track the browser ends all stop every track.
- **Clocks.** A microphone that is on feeds the card from its own clock, so no frame is dropped or repeated; if it falls silent for 500 ms (a suspended audio context) the card's own clock supplies silence until it returns. A guest that does not take its buffers for a second loses frames, which `stats().counters.capDropped` counts.

## The resampler

`src/resample.ts` (a pure typed module, no browser or emulator) resamples one channel: a Kaiser-windowed sinc low-pass (90 dB stopband, 32 taps per phase at the lower rate, a flat passband to 0.6 of its Nyquist frequency) evaluated from a table of exact fractional phases made once per rate pair and shared. Rates that reduce to at most 2,048 phases (every pair of the standard rates does: 44,100 to 48,000 is 160 phases, 22,050 is 320, 11,025 is 640, 8,000 is 6) step exactly, with no accumulated error; any other pair uses 2,048 phases and the nearest. The fractional position and the filter's history carry across calls, so a stream split at any point gives the one-shot result (tested to 1e-6). Input may be 16-bit integers with a stride (one side of interleaved stereo) or floats; output is floats. The hot path allocates nothing. Measured on every standard rate to and from 48 kHz (`test/quality-report.ts`): passband flat within 0.002 dB to 0.45 of the lower Nyquist frequency, at least 90 dB of rejection of what would alias or image into it, THD+N of a 1 kHz tone below -90 dB. In Chromium one second of 48 kHz stereo playback costs 2.5 to 2.9 ms of the page's thread, and a second of microphone capture 1.1 to 1.4 ms.

## Rates

The guest may set any whole rate from 8,000 to 48,000 Hz (a value outside is clamped) once variable rate is on: the extended audio ID (0x28) reports variable rate audio (bit 0), the extended control and status register (0x2A) turns it on, and the front DAC rate (0x2C) and the ADC rate (0x32) hold the rates. The standard rates, which have exact tables, are 8,000, 11,025, 16,000, 22,050, 32,000, 44,100 and 48,000 Hz (`STANDARD_RATES`). With variable rate off both are 48,000. The separate microphone rate (0x34, variable rate microphone) is not offered; the mic-in channel records at the ADC rate. ReactOS's `ac97.sys` turns variable rate on and writes the DAC rate for each sound (22,050 for *Critical Stop*, 8,000 after a reset); the recording rates it programs are in the *Sound* section of `site/try-reactos/README.md`.

## Layout

| File | What |
|---|---|
| `src/model.ts` | The chip: ICH bus-master registers, the AC'97 2.3 codec, descriptor-list walking, status bits, the interrupt line, resampling and the codec's volume. No emulator, no browser |
| `src/v86.ts` | The v86 glue: PCI space, I/O port handlers, `onceBeforeInit`, `registerOn`. The only file that knows v86's internals, pinned with v86 |
| `src/resample.ts` | The band-limited polyphase resampler, used for playback (the guest's rate to the output's) and capture (the microphone's to the guest's) |
| `src/capture.ts` | The microphone: the permission, an `AudioWorklet` that hands out mono blocks, and a stop that ends every track |
| `src/sink.ts` | The output: an `AudioWorklet` ring player that counts what it plays (and can record it), or a script-processor fallback |
| `src/index.ts` | `attachAc97` and the public types |
| `test/` | `bun test`: the register model, recording, the resampler, the microphone on fakes, the glue on a fake v86, the output on a fake audio context; `quality-report.ts` prints the resampler's measurements |
| `build.ts` | Builds `../../lib/ac97.js` (`--check` fails on drift; 36 KB budget) |

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

The card is PCI 8086:2415 (82801AA AC'97 audio) in the first free slot from 3, with a mixer BAR of 256 bytes and a bus-master BAR of 64 bytes, interrupt pin A. The codec answers as a plain AC'97 2.3 codec with variable rate audio (the vendor ID is a SigmaTel STAC9700's). PCM-out plays; PCM-in and mic-in record. The interrupt is level-style: raised when an enabled status bit is set, lowered when the guest clears it.

## Tested with

- v86 0.5.473 (`site/try-reactos/v86/PROVENANCE.md`; `V86_VERSION` is exported and a test keeps it equal to `tools/fetch-v86.sh`'s pin), ReactOS 0.4.16 with its `ac97.sys`, in headless Chromium driven by `playwright-core`.
- Bun 1.4.4 for the tests and the build, TypeScript 5.9.3 for the type check.
- Not run: Firefox, Safari, mobile browsers, a hidden tab, real speakers. See the *Sound* section of `site/try-reactos/README.md`.
