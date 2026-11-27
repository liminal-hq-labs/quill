// Shared helpers for the sound checks: an AudioWorklet probe that sits after the page's volume node and counts (and can record) what would reach
// the speakers, the Run box, and a WAV writer. Headless Chromium has no sound card, so the probe is the proof of what a speaker would have received.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Page } from "playwright-core";
import { KEY, sleep } from "./lib.ts";

const PROBE = `
class Probe extends AudioWorkletProcessor {
	constructor() {
		super();
		this.s = { frames: 0, loud: 0, peak: 0 };
		this.rec = null;
		this.port.onmessage = (e) => {
			const m = e.data;
			if (m.t === "stats") this.port.postMessage({ t: "stats", s: this.s });
			else if (m.t === "rec") this.rec = m.on ? { l: [], r: [] } : this.rec;
			else if (m.t === "take") { const r = this.rec; this.rec = null; this.port.postMessage({ t: "take", l: r ? r.l : [], r: r ? r.r : [] }); }
		};
	}
	process(inputs, outputs) {
		const i = inputs[0];
		if (!i || !i[0]) return true;
		const l = i[0];
		const r = i[1] || i[0];
		for (let n = 0; n < l.length; n++) {
			const a = Math.max(Math.abs(l[n]), Math.abs(r[n]));
			if (a > this.s.peak) this.s.peak = a;
			if (a > 0.0001) this.s.loud++;
		}
		this.s.frames += l.length;
		if (this.rec) { this.rec.l.push(l.slice(0)); this.rec.r.push(r.slice(0)); }
		return true;
	}
}
registerProcessor("quill-probe", Probe);
`;

export interface ProbeStats {
	/** Frames that reached the probe, silent or not. */
	frames: number;
	/** Frames louder than 0.0001. */
	loud: number;
	peak: number;
}

/** Puts a probe after the machine's volume node. Call again after every Start or Reset: a new machine has a new output. */
export async function installProbe(page: Page): Promise<void> {
	await page.evaluate(async (source) => {
		const handle = (window as any).tryReactos.soundHandle;
		const sink = handle.sink;
		if (!sink) throw new Error("the machine has no sound output yet");
		const ctx: AudioContext = sink.context;
		const w = window as any;
		if (!w.__probeLoaded || w.__probeLoaded !== ctx) {
			const url = URL.createObjectURL(new Blob([source], { type: "text/javascript" }));
			await ctx.audioWorklet.addModule(url);
			w.__probeLoaded = ctx;
		}
		const node = new AudioWorkletNode(ctx, "quill-probe", { numberOfInputs: 1, numberOfOutputs: 1, outputChannelCount: [2] });
		const mute = ctx.createGain();
		mute.gain.value = 0;
		sink.output.connect(node);
		node.connect(mute).connect(ctx.destination);
		const waiting: ((m: any) => void)[] = [];
		node.port.onmessage = (e) => waiting.splice(0).forEach((f) => f(e.data));
		w.__probe = { node, ask: (msg: unknown) => new Promise((resolve) => (waiting.push(resolve), node.port.postMessage(msg))) };
	}, PROBE);
}

export const probeStats = (page: Page): Promise<ProbeStats> => page.evaluate(async () => ((await (window as any).__probe.ask({ t: "stats" })) as any).s);
export const probeRecord = (page: Page) => page.evaluate(() => (window as any).__probe.node.port.postMessage({ t: "rec", on: true }));

/** The recording as 16-bit stereo, and what it holds. */
export async function probeTake(page: Page): Promise<{ left: Float32Array; right: Float32Array }> {
	const m = await page.evaluate(async () => {
		const r: any = await (window as any).__probe.ask({ t: "take" });
		const join = (chunks: Float32Array[]) => {
			const out = new Float32Array(chunks.length * 128);
			chunks.forEach((c, i) => out.set(c, i * 128));
			return Array.from(out);
		};
		return { l: join(r.l), r: join(r.r) };
	});
	return { left: Float32Array.from(m.l), right: Float32Array.from(m.r) };
}

export function wav(left: Float32Array, right: Float32Array, rate: number): Buffer {
	const n = left.length;
	const out = Buffer.alloc(44 + n * 4);
	out.write("RIFF", 0);
	out.writeUInt32LE(36 + n * 4, 4);
	out.write("WAVEfmt ", 8);
	out.writeUInt32LE(16, 16);
	out.writeUInt16LE(1, 20);
	out.writeUInt16LE(2, 22);
	out.writeUInt32LE(rate, 24);
	out.writeUInt32LE(rate * 4, 28);
	out.writeUInt16LE(4, 32);
	out.writeUInt16LE(16, 34);
	out.write("data", 36);
	out.writeUInt32LE(n * 4, 40);
	for (let i = 0; i < n; i++) {
		out.writeInt16LE(Math.max(-32768, Math.min(32767, Math.round((left[i] ?? 0) * 32767))), 44 + i * 4);
		out.writeInt16LE(Math.max(-32768, Math.min(32767, Math.round((right[i] ?? 0) * 32767))), 46 + i * 4);
	}
	return out;
}

/** Runs a command from the Run box (Win+R), through the page's own input API. */
export async function runBox(page: Page, command: string): Promise<void> {
	await page.evaluate(
		([win, r]) => {
			const i = (window as any).tryReactos.input;
			i.key(win);
			i.key(r);
			i.key(r.map((v: number) => v | 0x80));
			i.key(win.map((v: number) => v | 0x80));
		},
		[KEY.win, KEY.r],
	);
	await sleep(2500);
	for (const ch of command) {
		await page.evaluate((x) => (window as any).tryReactos.input.text(x), ch);
		await sleep(90);
	}
	await sleep(600);
	await page.evaluate((c) => {
		const i = (window as any).tryReactos.input;
		i.key(c);
		i.key(c.map((v: number) => v | 0x80));
	}, KEY.enter);
}

const press = (page: Page, codes: number[]) =>
	page.evaluate((c) => {
		const i = (window as any).tryReactos.input;
		i.key(c);
		i.key(c.map((v: number) => v | 0x80));
	}, codes);

/**
 * Opens Sound and Audio Devices on its Sounds tab through the Run box and moves to the play button beside the "Critical Stop" event's sound
 * (ReactOS_Critical_Stop.wav): Tab to the program events, Down four times, Tab to the sound, Down, Tab to the button. Afterwards \`playSound\` presses it.
 */
export async function openSounds(page: Page): Promise<void> {
	await runBox(page, "rundll32 shell32.dll,Control_RunDLL mmsys.cpl,,1");
	await sleep(8000);
	for (let n = 0; n < 3; n++) {
		await press(page, KEY.tab);
		await sleep(300);
	}
	for (let n = 0; n < 4; n++) {
		await press(page, KEY.down);
		await sleep(300);
	}
	await press(page, KEY.tab);
	await sleep(500);
	await press(page, KEY.down);
	await sleep(800);
	await press(page, KEY.tab);
	await sleep(500);
}

/** Presses the play button on the Sounds tab: the guest plays Critical Stop. */
export const playSound = (page: Page) => press(page, KEY.enter);

/** What the card has taken so far and whether the guest is playing. */
export const card = (page: Page) =>
	page.evaluate(() => {
		const h = (window as any).tryReactos.soundHandle;
		if (!h) return null;
		const s = h.stats();
		return { registered: s.registered, pciId: s.pciId, guestRate: s.guestRate, playing: s.playing, dma: s.counters.dmaFrames, buffers: s.counters.buffersDone, iocs: s.counters.iocs, irqs: s.counters.irqRaised, starved: s.counters.starvedFrames, drained: s.drainedFrames, output: s.output, sink: s.sink, volume: s.volume, muted: s.muted, gain: s.gain, stateError: s.stateError, peakRaw: s.counters.peakRaw };
	});
export type Card = NonNullable<Awaited<ReturnType<typeof card>>>;

/** The controls as a visitor sees them. */
export const controls = (page: Page) =>
	page.evaluate(() => {
		const button = document.querySelector("[data-sound]") as HTMLButtonElement;
		const slider = document.querySelector("[data-volume]") as HTMLInputElement;
		let stored: string | null = null;
		try {
			stored = localStorage.getItem("quill-try-reactos-sound");
		} catch {
			stored = "blocked";
		}
		return {
			label: button.textContent ?? "",
			pressed: button.getAttribute("aria-pressed"),
			disabled: button.disabled,
			title: button.title,
			slider: slider.value,
			sliderDisabled: slider.disabled,
			stored,
			readout: document.querySelector("[data-sound-readout] dd")?.textContent ?? "",
		};
	});
