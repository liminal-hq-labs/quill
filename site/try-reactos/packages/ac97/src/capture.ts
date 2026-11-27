// The microphone: asked for only when `start` is called (from a visitor's click), heard through an AudioWorklet that hands mono blocks to a callback, and let go
// by `stop`, which ends every track. What it hears goes only to the callback: this file keeps nothing, stores nothing and sends nothing.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

export const CAPTURE_WORKLET = `
class CaptureBlocks extends AudioWorkletProcessor {
	constructor() {
		super();
		this.block = new Float32Array(480);
		this.filled = 0;
	}
	process(inputs) {
		const input = inputs[0];
		if (input && input.length > 0) {
			const channels = input.length;
			const length = input[0].length;
			for (let i = 0; i < length; i++) {
				let sum = 0;
				for (let c = 0; c < channels; c++) sum += input[c][i];
				this.block[this.filled++] = sum / channels;
				if (this.filled === this.block.length) {
					const out = this.block;
					this.block = new Float32Array(480);
					this.filled = 0;
					this.port.postMessage(out, [out.buffer]);
				}
			}
		}
		return true;
	}
}
registerProcessor("ac97-capture", CaptureBlocks);
`;

/**
 * Where the microphone stands: `off` (never asked, or let go), `asking` (the browser's permission question is open), `on`, or why not: `denied` (the visitor
 * or the browser said no), `no-device` (no microphone), `insecure` (the page is not a secure context, where browsers give no microphone) or `unavailable`.
 */
export type MicState = "off" | "asking" | "on" | "denied" | "no-device" | "insecure" | "unavailable" | "error";

/** The plain reason for a state that is not `off` or `on`, for a button's title. */
export function micReason(state: MicState): string {
	switch (state) {
		case "asking":
			return "Waiting for your browser's permission to use the microphone.";
		case "denied":
			return "The microphone is blocked. Allow it for this site in your browser's settings, then try again.";
		case "no-device":
			return "No microphone was found.";
		case "insecure":
			return "Browsers give a microphone only to pages served over HTTPS (or from this computer).";
		case "unavailable":
			return "This browser cannot give the page a microphone.";
		case "error":
			return "The microphone could not be started.";
		default:
			return "";
	}
}

const loaded = new WeakMap<BaseAudioContext, Promise<void>>();

function loadWorklet(context: AudioContext): Promise<void> {
	let ready = loaded.get(context);
	if (!ready) {
		const url = URL.createObjectURL(new Blob([CAPTURE_WORKLET], { type: "text/javascript" }));
		ready = context.audioWorklet.addModule(url).finally(() => URL.revokeObjectURL(url));
		loaded.set(context, ready);
		ready.catch(() => loaded.delete(context));
	}
	return ready;
}

export interface MicrophoneOptions {
	/** Called with each block of mono samples (floats in [-1, 1]) at `rate` frames a second. The block is the callback's to keep or drop. */
	onBlock: (samples: Float32Array, rate: number) => void;
	onChange?: () => void;
}

export class Microphone {
	private current: MicState = "off";
	private stream: MediaStream | null = null;
	private nodes: AudioNode[] = [];
	private generation = 0;
	/** The time of the last block (performance.now()), 0 before the first. */
	lastBlockAt = 0;

	constructor(
		private readonly context: AudioContext,
		private readonly options: MicrophoneOptions,
	) {}

	get state(): MicState {
		return this.current;
	}

	private set(state: MicState): void {
		this.current = state;
		if (this.options.onChange) this.options.onChange();
	}

	/** Asks the browser for the microphone and starts listening. Call it only from a click or key press the visitor made for this. */
	async start(): Promise<MicState> {
		if (this.current === "on" || this.current === "asking") return this.current;
		if (typeof isSecureContext === "boolean" && !isSecureContext) {
			this.set("insecure");
			return this.current;
		}
		const devices = typeof navigator === "object" ? navigator.mediaDevices : undefined;
		if (!devices || typeof devices.getUserMedia !== "function" || !this.context.audioWorklet) {
			this.set("unavailable");
			return this.current;
		}
		const mine = ++this.generation;
		this.set("asking");
		let stream: MediaStream;
		try {
			// No echo cancelling, noise suppression or automatic gain: the guest asked for a microphone, not for a conference call's clean-up.
			stream = await devices.getUserMedia({ audio: { echoCancellation: false, noiseSuppression: false, autoGainControl: false, channelCount: 1 }, video: false });
		} catch (error) {
			if (mine !== this.generation) return this.current;
			const name = error instanceof DOMException || error instanceof Error ? error.name : "";
			this.set(name === "NotAllowedError" || name === "SecurityError" ? "denied" : name === "NotFoundError" || name === "OverconstrainedError" ? "no-device" : "error");
			return this.current;
		}
		if (mine !== this.generation) {
			// Stopped (or detached) while the question was open: let the microphone go at once.
			for (const track of stream.getTracks()) track.stop();
			return this.current;
		}
		try {
			await loadWorklet(this.context);
			if (mine !== this.generation) {
				for (const track of stream.getTracks()) track.stop();
				return this.current;
			}
			const source = this.context.createMediaStreamSource(stream);
			const node = new AudioWorkletNode(this.context, "ac97-capture", { numberOfInputs: 1, numberOfOutputs: 1, outputChannelCount: [1] });
			node.port.onmessage = (event: MessageEvent) => {
				this.lastBlockAt = performance.now();
				this.options.onBlock(event.data as Float32Array, this.context.sampleRate);
			};
			// A worklet is only run when it leads to the output, so it ends in a gain of zero: nothing the microphone hears reaches the speakers.
			const silent = this.context.createGain();
			silent.gain.value = 0;
			source.connect(node).connect(silent).connect(this.context.destination);
			this.nodes = [source, node, silent];
			this.stream = stream;
			for (const track of stream.getAudioTracks()) track.addEventListener("ended", () => this.stop());
			this.set("on");
		} catch {
			for (const track of stream.getTracks()) track.stop();
			this.set("error");
		}
		return this.current;
	}

	/** Lets the microphone go: every track is stopped and the graph is taken apart. Safe in any state. */
	stop(): void {
		this.generation++;
		if (this.stream) for (const track of this.stream.getTracks()) track.stop();
		this.stream = null;
		for (const node of this.nodes) {
			try {
				if (node instanceof AudioWorkletNode) node.port.onmessage = null;
				node.disconnect();
			} catch {
				// Already disconnected.
			}
		}
		this.nodes = [];
		this.lastBlockAt = 0;
		if (this.current !== "off") this.set("off");
	}

	/** The tracks still live, for a test that checks nothing leaks. */
	liveTracks(): number {
		return this.stream ? this.stream.getTracks().filter((track) => track.readyState === "live").length : 0;
	}
}
