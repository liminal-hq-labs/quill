// A virtual Intel AC'97 sound card for v86, written for the Quill ReactOS demo: `attachAc97(emulator, options)` adds the card to a machine that is
// about to start, plays what the guest sends it through Web Audio, and gives back the card's registers so they can be saved beside a state.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { Ac97, isAc97State, type Ac97Counters, type Ac97Options, type Ac97State } from "./model.ts";
import { Microphone, type MicState } from "./capture.ts";
import { createSink, type Sink, type SinkStats } from "./sink.ts";
import { onceBeforeInit, registerOn, type Registered, type V86Emulator } from "./v86.ts";

export * from "./model.ts";
export * from "./sink.ts";
export * from "./capture.ts";
export * from "./resample.ts";
export { onceBeforeInit, pciSpace, registerOn, V86_VERSION } from "./v86.ts";
export type { Registered, V86Cpu, V86Emulator, V86Io, V86Machine, V86Pci } from "./v86.ts";

export interface AttachOptions {
	/** Registers saved by `getAc97State` beside the machine state being restored. Leave out for a machine that boots from scratch. */
	state?: Ac97State | null;
	/** Volume from 0 to 1 (a perceptual taper: the gain is the square). Default 0.7. */
	volume?: number;
	muted?: boolean;
	/**
	 * The `AudioContext` to play through. Create it inside a click handler (a browser lets sound start only from a user gesture) and pass it here.
	 * Left out, the package makes its own, which stays suspended until `resume()` is called from a gesture. `null` plays nowhere: the card still
	 * runs, and its buffers are taken at their real-time rate and thrown away, so the guest never waits on a sound that nothing is playing.
	 */
	context?: AudioContext | null;
	/** Called when the output is ready, fails or changes state, and when the card is registered. */
	onChange?: () => void;
	/** Called for each register access (for debugging; slow). */
	trace?: Ac97Options["trace"];
}

/** Where the sound goes: `none` (no output), or the state of the context, or `failed` (the output could not be built). */
export type OutputState = "none" | "failed" | AudioContextState;

export interface Ac97Stats {
	registered: boolean;
	pciId: number | null;
	/** The PCM-out rate the guest has chosen, in frames per second (0 before the card is registered). */
	guestRate: number;
	/** Whether the guest's PCM-out channel is running now. */
	playing: boolean;
	/** Whether the card holds its interrupt line up now. */
	interrupt: boolean;
	counters: Ac97Counters | null;
	/** Frames taken at real-time rate and thrown away because no output was asking (a suspended or missing context). */
	drainedFrames: number;
	output: OutputState;
	/** Where the microphone stands (`off` unless the visitor turned it on), and whether the guest is recording now. */
	mic: MicState;
	recording: boolean;
	sink: "worklet" | "script" | null;
	volume: number;
	muted: boolean;
	/** The gain the output applies now (0 when muted). */
	gain: number;
	/** A message when the saved registers were refused. */
	stateError: string;
}

export interface Ac97Handle {
	/** The card's registers, for saving beside a machine state; `null` before the card is registered. Save while no sound plays. */
	getAc97State(): Ac97State | null;
	setVolume(volume: number): void;
	getVolume(): number;
	setMuted(muted: boolean): void;
	isMuted(): boolean;
	/** Starts or resumes the output. Call it from a click or key handler; resolves with the output's state. */
	resume(): Promise<OutputState>;
	/** The output's state now. */
	outputState(): OutputState;
	stats(): Ac97Stats;
	/**
	 * Turns the microphone on: asks the browser for it (call this only from a click the visitor made for it) and feeds it to the card's recording channels,
	 * resampled to the rate the guest records at. The audio goes into the emulated card's memory and nowhere else. Resolves with the microphone's state.
	 */
	startMicrophone(): Promise<MicState>;
	/** Turns the microphone off and lets every track go. The guest then records silence. Safe in any state. */
	stopMicrophone(): void;
	micState(): MicState;
	/** The microphone, for tests (`liveTracks`). */
	readonly microphone: Microphone | null;
	/** What the output has played, counted where the samples leave the worklet (before the volume). `null` without an output. */
	sinkStats(): Promise<SinkStats | null>;
	/** The output, for tests that need to listen to it (`record`, `take`, the gain node). `null` before it is built. */
	readonly sink: Sink | null;
	/** The card's model, for tests. `null` before it is registered. */
	readonly model: Ac97 | null;
	/** Resolves once the card is registered (v86 has built its devices). */
	readonly registered: Promise<void>;
	/** Silences the output, stops answering the guest and lets the machine go. Safe to call twice. */
	detach(): void;
}

const clamp01 = (value: number): number => (Number.isFinite(value) ? Math.min(1, Math.max(0, value)) : 0);

/** The gain for a volume setting: the square, which feels even to the ear. */
export function gainFor(volume: number, muted: boolean): number {
	const v = clamp01(volume);
	return muted ? 0 : v * v;
}

const DRAIN_MS = 10;
const DRAIN_MAX_FRAMES = 4800;
const MIC_STALL_MS = 500;

/**
 * Adds the card to `emulator`, a v86 `V86` that has been constructed and has not started yet. v86 builds its devices a moment after its constructor
 * returns; the card is registered then, before the BIOS or a restored state runs, which is the only moment that serves a cold boot and a state.
 */
export function attachAc97(emulator: V86Emulator, options: AttachOptions = {}): Ac97Handle {
	let volume = clamp01(options.volume ?? 0.7);
	let muted = Boolean(options.muted);
	let reg: Registered | null = null;
	let sink: Sink | null = null;
	let failed = false;
	let detached = false;
	let ownsContext = false;
	let stateError = "";
	let drainedFrames = 0;
	let lastDrain = performance.now();
	let context: AudioContext | null = null;
	let resolveRegistered: () => void = () => {};
	const registered = new Promise<void>((resolve) => (resolveRegistered = resolve));
	const changed = (): void => {
		if (!detached && options.onChange) options.onChange();
	};

	const scratchLeft = new Float32Array(DRAIN_MAX_FRAMES);
	const scratchRight = new Float32Array(DRAIN_MAX_FRAMES);
	const drain = (): void => {
		const now = performance.now();
		if (sink && sink.live()) {
			lastDrain = now;
			return;
		}
		const elapsed = now - lastDrain;
		lastDrain = now;
		const model = reg ? reg.model : null;
		if (!model || !model.running) return;
		const rate = sink ? sink.rate : 48000;
		const frames = Math.min(DRAIN_MAX_FRAMES, Math.round((elapsed * rate) / 1000));
		if (frames < 1) return;
		model.render(frames, rate, scratchLeft, scratchRight, true);
		drainedFrames += frames;
	};
	let lastRecord = performance.now();
	// The guest records silence in real time when no microphone is feeding the card, so a recording application finds an input that works.
	const recordSilence = (): void => {
		const now = performance.now();
		const elapsed = now - lastRecord;
		lastRecord = now;
		const model = reg ? reg.model : null;
		if (!model || !model.recording) return;
		// A microphone that is on feeds the card from its own clock; only a long silence from it (a suspended context) lets the card's own clock take over, so a
		// busy page that delivers a block late never has silence put in front of it.
		if (mic && mic.state === "on" && now - mic.lastBlockAt < MIC_STALL_MS) return;
		const frames = Math.min(DRAIN_MAX_FRAMES, Math.round((elapsed * model.adcRate) / 1000));
		if (frames > 0) model.feedSilence(frames);
	};
	const drainTimer = setInterval(() => {
		drain();
		recordSilence();
	}, DRAIN_MS);

	let mic: Microphone | null = null;
	const applyGain = (): void => {
		if (sink) sink.setGain(gainFor(volume, muted));
	};

	const onStateChange = (): void => changed();
	const buildSink = (ctx: AudioContext): void => {
		createSink(ctx).then(
			(built) => {
				if (detached) {
					built.close();
					return;
				}
				sink = built;
				sink.onNeed = (frames) => {
					const model = reg ? reg.model : null;
					if (!model) return null;
					const l = new Float32Array(frames);
					const r = new Float32Array(frames);
					model.render(frames, built.rate, l, r);
					return { l, r };
				};
				applyGain();
				changed();
			},
			() => {
				failed = true;
				changed();
			},
		);
	};

	const AudioContextClass = typeof AudioContext === "function" ? AudioContext : null;
	if (options.context) {
		context = options.context;
	} else if (options.context === undefined && AudioContextClass) {
		try {
			context = new AudioContextClass({ sampleRate: 48000, latencyHint: "interactive" });
			ownsContext = true;
		} catch {
			failed = true;
		}
	}
	if (context) {
		context.addEventListener("statechange", onStateChange);
		buildSink(context);
	}

	if (context) {
		mic = new Microphone(context, {
			onBlock: (samples, rate) => {
				if (reg && !detached) reg.model.feedCapture(samples, samples.length, rate);
			},
			onChange: changed,
		});
	}

	const undoWrap = onceBeforeInit(emulator, (cpu) => {
		if (detached) return;
		reg = registerOn(cpu, options.trace ?? null);
		if (options.state) {
			if (isAc97State(options.state)) {
				try {
					reg.model.setState(options.state);
				} catch (error) {
					stateError = error instanceof Error ? error.message : String(error);
				}
			} else {
				stateError = "the saved registers are not an AC'97 state";
			}
		}
		resolveRegistered();
		changed();
	});

	const outputState = (): OutputState => (detached ? "none" : failed ? "failed" : context ? context.state : "none");

	return {
		getAc97State: () => (reg ? reg.model.getState() : null),
		setVolume(next) {
			volume = clamp01(next);
			applyGain();
		},
		getVolume: () => volume,
		setMuted(next) {
			muted = next;
			applyGain();
		},
		isMuted: () => muted,
		async resume() {
			if (context && context.state !== "running" && context.state !== "closed") {
				try {
					await context.resume();
				} catch {
					// The browser refused: the state below says so.
				}
			}
			return outputState();
		},
		outputState,
		stats() {
			const model = reg ? reg.model : null;
			return {
				registered: reg !== null,
				pciId: reg ? reg.pciId : null,
				guestRate: model ? model.rate : 0,
				playing: model ? model.running : false,
				interrupt: model ? model.interruptAsserted : false,
				counters: model ? { ...model.counters } : null,
				drainedFrames,
				output: outputState(),
				mic: mic ? mic.state : "off",
				recording: model ? model.recording : false,
				sink: sink ? sink.kind : null,
				volume,
				muted,
				gain: gainFor(volume, muted),
				stateError,
			};
		},
		startMicrophone: async () => (mic && !detached ? mic.start() : "unavailable"),
		stopMicrophone() {
			if (mic) mic.stop();
		},
		micState: () => (mic ? mic.state : "off"),
		get microphone() {
			return mic;
		},
		sinkStats: async () => (sink ? sink.stats() : null),
		get sink() {
			return sink;
		},
		get model() {
			return reg ? reg.model : null;
		},
		registered,
		detach() {
			if (detached) return;
			detached = true;
			clearInterval(drainTimer);
			if (mic) mic.stop();
			undoWrap();
			if (reg) reg.retire();
			if (sink) {
				sink.clear();
				sink.close();
			}
			if (context) {
				context.removeEventListener("statechange", onStateChange);
				if (ownsContext) void context.close().catch(() => {});
			}
			sink = null;
			resolveRegistered();
		},
	};
}
