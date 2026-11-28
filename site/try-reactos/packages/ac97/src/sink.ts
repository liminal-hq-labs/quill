// The audio output: an AudioWorklet that plays a ring buffer the page tops up (or, where a page has no worklet, as on an insecure origin or an old
// browser, a ScriptProcessorNode that asks for each block). Both count what they play, so a run with no sound card can still prove what a
// speaker would have received.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/** The ring player's code, loaded as a module from a Blob URL. Measures what goes to the output and can keep a copy. */
export const WORKLET_SOURCE = `
class RingPlayer extends AudioWorkletProcessor {
	constructor() {
		super();
		this.cap = 96000;
		this.l = new Float32Array(this.cap);
		this.r = new Float32Array(this.cap);
		this.rd = 0;
		this.wr = 0;
		this.n = 0;
		this.popped = 0;
		this.primed = false;
		this.starving = false;
		this.s = { out: 0, silent: 0, peak: 0, underrunEvents: 0, underrunFrames: 0, dropped: 0, quanta: 0 };
		this.rec = null;
		this.tick = 0;
		this.port.onmessage = (e) => {
			const m = e.data;
			if (m.t === "push") {
				const len = m.l.length;
				for (let i = 0; i < len; i++) {
					if (this.n >= this.cap) { this.s.dropped += len - i; break; }
					this.l[this.wr] = m.l[i];
					this.r[this.wr] = m.r[i];
					this.wr = (this.wr + 1) % this.cap;
					this.n++;
				}
				this.primed = true;
			} else if (m.t === "clear") {
				this.rd = 0;
				this.wr = 0;
				this.n = 0;
				this.primed = false;
				this.starving = false;
				this.port.postMessage({ t: "cleared", popped: this.popped });
			} else if (m.t === "rec") {
				this.rec = m.on ? { l: [], r: [], frames: 0, max: m.max || 48000 * 60 } : null;
			} else if (m.t === "take") {
				const r = this.rec;
				this.rec = null;
				this.port.postMessage({ t: "rec", l: r ? r.l : [], r: r ? r.r : [] });
			} else if (m.t === "stats") {
				this.port.postMessage({ t: "stats", s: this.s, n: this.n });
			}
		};
	}
	process(inputs, outputs) {
		const o = outputs[0];
		const len = o[0].length;
		const ol = o[0];
		const or = o[1] || o[0];
		const have = Math.min(this.n, len);
		for (let i = 0; i < have; i++) {
			ol[i] = this.l[this.rd];
			or[i] = this.r[this.rd];
			this.rd = (this.rd + 1) % this.cap;
		}
		this.n -= have;
		this.popped += have;
		for (let i = have; i < len; i++) { ol[i] = 0; or[i] = 0; }
		if (this.primed && have < len) {
			if (!this.starving) this.s.underrunEvents++;
			this.starving = true;
			this.s.underrunFrames += len - have;
		} else if (have === len) {
			this.starving = false;
		}
		const st = this.s;
		st.quanta++;
		for (let i = 0; i < len; i++) {
			const a = Math.max(Math.abs(ol[i]), Math.abs(or[i]));
			if (a > st.peak) st.peak = a;
			if (a < 0.0001) st.silent++;
		}
		st.out += len;
		if (this.rec && this.rec.frames < this.rec.max) {
			this.rec.l.push(ol.slice(0));
			this.rec.r.push(or.slice(0));
			this.rec.frames += len;
		}
		if (++this.tick % 4 === 0) this.port.postMessage({ t: "level", popped: this.popped });
		return true;
	}
}
registerProcessor("ac97-ring-player", RingPlayer);
`;

/** What the output has played so far. `out` counts every frame, silent or not; `silent` those below 0.0001. */
export interface SinkStats {
	out: number;
	silent: number;
	peak: number;
	underrunEvents: number;
	underrunFrames: number;
	dropped: number;
	quanta: number;
}

/** Frames the device should render for the output, as left and right arrays the output then owns. */
export type NeedFn = (frames: number) => { l: Float32Array; r: Float32Array } | null;

export interface Sink {
	readonly kind: "worklet" | "script";
	readonly context: AudioContext;
	/** The node every sound passes on its way to the speakers; its gain is the volume. */
	readonly output: GainNode;
	/** Frames per second the device is asked to render at. */
	readonly rate: number;
	onNeed: NeedFn | null;
	/** Whether the output asked for sound within the last 100 ms (false while the context is suspended or closed). */
	live(): boolean;
	setGain(gain: number): void;
	gain(): number;
	/** Drops what is queued, so a stopped machine falls silent at once. */
	clear(): void;
	stats(): Promise<SinkStats | null>;
	/** Keeps a copy of what the output plays (for tests): `record(true)`, then `take()`. */
	record(on: boolean): void;
	take(): Promise<{ l: Float32Array[]; r: Float32Array[] }>;
	close(): void;
}

const loaded = new WeakMap<BaseAudioContext, Promise<void>>();

function loadWorklet(context: AudioContext): Promise<void> {
	let ready = loaded.get(context);
	if (!ready) {
		const url = URL.createObjectURL(new Blob([WORKLET_SOURCE], { type: "text/javascript" }));
		ready = context.audioWorklet.addModule(url).finally(() => URL.revokeObjectURL(url));
		loaded.set(context, ready);
		ready.catch(() => loaded.delete(context));
	}
	return ready;
}

/** Moves a gain node to `gain` in a short ramp (so a mute does not click) and then sets it exactly, so a mute is silence and not a value near zero. */
function rampTo(node: GainNode, context: AudioContext, gain: number): void {
	const now = context.currentTime;
	const param = node.gain;
	param.cancelScheduledValues(now);
	param.setTargetAtTime(gain, now, 0.01);
	param.setValueAtTime(gain, now + 0.1);
}

const TARGET_MS = 120;
const LIVE_MS = 100;

class WorkletSink implements Sink {
	readonly kind = "worklet";
	readonly output: GainNode;
	readonly rate: number;
	onNeed: NeedFn | null = null;
	// Frames sent to the worklet, and frames it has played (as of its last message): the difference is what is queued. Counting both ends, not
	// reading a level, keeps a late message (the page's thread is busy running the machine) from making the page send the same frames twice.
	private pushed = 0;
	private popped = 0;
	private clearing = false;
	private lastAsk = Number.NEGATIVE_INFINITY;
	private target: number;
	private wanted = 1;
	private pending: ((message: { t: string; s?: SinkStats; l?: Float32Array[]; r?: Float32Array[] }) => void)[] = [];

	constructor(
		readonly context: AudioContext,
		private readonly node: AudioWorkletNode,
	) {
		this.rate = context.sampleRate;
		this.target = Math.round((this.rate * TARGET_MS) / 1000);
		this.output = context.createGain();
		node.connect(this.output).connect(context.destination);
		node.port.onmessage = (event: MessageEvent) => {
			const m = event.data as { t: string; popped?: number; s?: SinkStats; l?: Float32Array[]; r?: Float32Array[] };
			if (m.t === "level") {
				this.popped = m.popped ?? this.popped;
				this.lastAsk = performance.now();
				this.top();
			} else if (m.t === "cleared") {
				this.popped = m.popped ?? this.popped;
				this.pushed = this.popped;
				this.clearing = false;
			} else if (m.t === "stats" || m.t === "rec") {
				this.pending.splice(0).forEach((resolve) => resolve(m));
			}
		};
	}

	private top(): void {
		if (this.clearing) return;
		const need = this.target - (this.pushed - this.popped);
		if (need < 256 || !this.onNeed) return;
		const out = this.onNeed(need);
		if (!out) return;
		this.pushed += need;
		this.node.port.postMessage({ t: "push", l: out.l, r: out.r }, [out.l.buffer, out.r.buffer]);
	}

	live(): boolean {
		return this.context.state === "running" && performance.now() - this.lastAsk < LIVE_MS;
	}

	setGain(gain: number): void {
		this.wanted = gain;
		this.ramp(gain);
	}

	private ramp(gain: number): void {
		rampTo(this.output, this.context, gain);
	}

	gain(): number {
		return this.wanted;
	}

	clear(): void {
		this.clearing = true;
		this.node.port.postMessage({ t: "clear" });
	}

	private ask(message: { t: string }): Promise<{ t: string; s?: SinkStats; l?: Float32Array[]; r?: Float32Array[] }> {
		return new Promise((resolve) => {
			this.pending.push(resolve);
			this.node.port.postMessage(message);
		});
	}

	async stats(): Promise<SinkStats | null> {
		return (await this.ask({ t: "stats" })).s ?? null;
	}

	record(on: boolean): void {
		this.node.port.postMessage({ t: "rec", on });
	}

	async take(): Promise<{ l: Float32Array[]; r: Float32Array[] }> {
		const m = await this.ask({ t: "take" });
		return { l: m.l ?? [], r: m.r ?? [] };
	}

	close(): void {
		this.node.port.onmessage = null;
		try {
			this.node.disconnect();
			this.output.disconnect();
		} catch {
			// Already disconnected.
		}
	}
}

/** The fallback where there is no AudioWorklet: the page's own thread fills each block as the browser asks. */
class ScriptSink implements Sink {
	readonly kind = "script";
	readonly output: GainNode;
	readonly rate: number;
	onNeed: NeedFn | null = null;
	private lastAsk = Number.NEGATIVE_INFINITY;
	private wanted = 1;
	private blocks = 0;
	private frames = 0;
	private readonly node: ScriptProcessorNode;

	constructor(readonly context: AudioContext) {
		this.rate = context.sampleRate;
		this.output = context.createGain();
		// 2048 frames is 43 ms at 48 kHz: coarse, but the guest's own buffers are longer.
		this.node = context.createScriptProcessor(2048, 0, 2);
		this.node.onaudioprocess = (event) => {
			const left = event.outputBuffer.getChannelData(0);
			const right = event.outputBuffer.getChannelData(1);
			this.lastAsk = performance.now();
			this.blocks++;
			this.frames += left.length;
			const out = this.onNeed ? this.onNeed(left.length) : null;
			if (out) {
				left.set(out.l);
				right.set(out.r);
			} else {
				left.fill(0);
				right.fill(0);
			}
		};
		this.node.connect(this.output).connect(context.destination);
	}

	live(): boolean {
		return this.context.state === "running" && performance.now() - this.lastAsk < LIVE_MS * 2;
	}

	setGain(gain: number): void {
		this.wanted = gain;
		this.ramp(gain);
	}

	private ramp(gain: number): void {
		rampTo(this.output, this.context, gain);
	}

	gain(): number {
		return this.wanted;
	}

	clear(): void {}

	async stats(): Promise<SinkStats | null> {
		return { out: this.frames, silent: 0, peak: 0, underrunEvents: 0, underrunFrames: 0, dropped: 0, quanta: this.blocks };
	}

	record(): void {}

	async take(): Promise<{ l: Float32Array[]; r: Float32Array[] }> {
		return { l: [], r: [] };
	}

	close(): void {
		this.node.onaudioprocess = null;
		try {
			this.node.disconnect();
			this.output.disconnect();
		} catch {
			// Already disconnected.
		}
	}
}

/**
 * Builds the output on `context`: a worklet when the browser has one, a script processor when it does not. Does not resume the context; the
 * caller does that from a user gesture.
 */
export async function createSink(context: AudioContext): Promise<Sink> {
	if (context.audioWorklet && typeof AudioWorkletNode === "function") {
		try {
			await loadWorklet(context);
			const node = new AudioWorkletNode(context, "ac97-ring-player", { numberOfInputs: 0, numberOfOutputs: 1, outputChannelCount: [2] });
			return new WorkletSink(context, node);
		} catch {
			// Fall through to the script processor.
		}
	}
	return new ScriptSink(context);
}
