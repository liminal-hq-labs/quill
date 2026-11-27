// The output on a fake audio context: the script-processor fallback, the volume node and what a muted or a suspended context does.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, test } from "bun:test";
import { attachAc97, createSink, type V86Cpu, type V86Emulator, type V86Machine } from "../src/index.ts";
import { CR } from "../src/model.ts";

/** Just enough of an AudioContext for the script-processor path: no `audioWorklet`, a gain node and a processor whose callback the test calls. */
function fakeContext(state: AudioContextState = "running") {
	const gains: number[] = [];
	const snapped: number[] = [];
	let processor: { onaudioprocess: ((event: unknown) => void) | null } | null = null;
	const listeners = new Set<() => void>();
	const context = {
		sampleRate: 48000,
		currentTime: 0,
		state,
		destination: {},
		createGain: () => ({ gain: { cancelScheduledValues() {}, setTargetAtTime: (value: number) => gains.push(value), setValueAtTime: (value: number) => snapped.push(value) }, connect: (next: unknown) => next, disconnect() {} }),
		createScriptProcessor: () => {
			processor = { onaudioprocess: null, connect: (next: unknown) => next, disconnect() {} } as never;
			return processor;
		},
		addEventListener: (_: string, listener: () => void) => listeners.add(listener),
		removeEventListener: (_: string, listener: () => void) => listeners.delete(listener),
		resume: async () => {
			context.state = "running";
			listeners.forEach((listener) => listener());
		},
		close: async () => {},
	};
	const block = (frames: number) => {
		const left = new Float32Array(frames);
		const right = new Float32Array(frames);
		processor?.onaudioprocess?.({ outputBuffer: { getChannelData: (c: number) => (c === 0 ? left : right) } });
		return { left, right };
	};
	return { context: context as unknown as AudioContext, raw: context, gains, snapped, block };
}

function guest() {
	const memory = new Uint8Array(1 << 20);
	const ports = new Map<number, { r16?: () => number; w8?: (v: number) => void; w16?: (v: number) => void; w32?: (v: number) => void }>();
	const cpu: V86Cpu = {
		devices: { pci: { devices: {}, register_device() {}, raise_irq() {}, lower_irq() {} } },
		io: {
			register_read: (port, _o, _r8, r16) => void ports.set(port, { ...ports.get(port), r16 }),
			register_write: (port, _o, w8, w16, w32) => void ports.set(port, { ...ports.get(port), w8, w16, w32 }),
		},
		read_blob: (a, n) => memory.subarray(a, a + n),
		write_blob: (blob, a) => memory.set(blob, a),
	};
	class Machine implements V86Machine {
		cpu = cpu;
		init() {}
	}
	const emulator: V86Emulator = {};
	const boot = () => {
		emulator.v86 = new Machine();
		emulator.v86.init();
	};
	const play = (frames: number, value: number) => {
		const dv = new DataView(memory.buffer);
		dv.setUint32(0x10000, 0x20000, true);
		dv.setUint32(0x10004, (frames * 2) | 0x80000000, true);
		for (let i = 0; i < frames * 2; i++) dv.setInt16(0x20000 + i * 2, value, true);
		ports.get(0xd000 + 0x02)?.w16?.(0x0000); // master 0 dB
		ports.get(0xd000 + 0x18)?.w16?.(0x0808); // PCM out 0 dB
		ports.get(0xd400 + 0x10)?.w32?.(0x10000);
		ports.get(0xd400 + 0x15)?.w8?.(0);
		ports.get(0xd400 + 0x1b)?.w8?.(CR.RPBM);
	};
	return { emulator, boot, play };
}

describe("the output", () => {
	test("without an AudioWorklet a script processor carries the sound, at the context's rate", async () => {
		const fake = fakeContext();
		const sink = await createSink(fake.context);
		expect(sink.kind).toBe("script");
		expect(sink.rate).toBe(48000);
		expect(sink.live()).toBe(false); // nothing has asked yet
		sink.onNeed = (frames) => ({ l: new Float32Array(frames).fill(0.5), r: new Float32Array(frames).fill(-0.5) });
		const { left, right } = fake.block(256);
		expect(left[0]).toBe(0.5);
		expect(right[255]).toBe(-0.5);
		expect(sink.live()).toBe(true);
		sink.setGain(0.25);
		expect(fake.gains.at(-1)).toBe(0.25);
		expect(sink.gain()).toBe(0.25);
		expect((await sink.stats())?.out).toBe(256);
		sink.close();
	});

	test("the guest's sound reaches the output, and mute puts the gain to zero without stopping the card", async () => {
		const fake = fakeContext();
		const g = guest();
		const handle = attachAc97(g.emulator, { context: fake.context, volume: 0.7 });
		g.boot();
		await new Promise((resolve) => setTimeout(resolve, 5)); // the sink is built asynchronously
		expect(handle.stats().sink).toBe("script");
		expect(handle.stats().output).toBe("running");
		expect(fake.gains.at(-1)).toBeCloseTo(0.49, 10);
		g.play(4800, 16384);
		const { left, right } = fake.block(1024);
		expect(Math.abs((left[10] ?? 0) - 0.5)).toBeLessThan(1e-6);
		expect(Math.abs((right[10] ?? 0) - 0.5)).toBeLessThan(1e-6); // the same sample on both sides
		handle.setMuted(true);
		expect(fake.gains.at(-1)).toBe(0);
		expect(fake.snapped.at(-1)).toBe(0); // the ramp ends on exactly zero
		fake.block(1024); // the guest keeps playing underneath: the card is not paused by a mute
		expect(handle.stats().counters?.dmaFrames).toBeGreaterThanOrEqual(2048);
		handle.setMuted(false);
		expect(fake.gains.at(-1)).toBeCloseTo(0.49, 10);
		handle.detach();
	});

	test("a suspended context is not live, so the card drains at real time, and resume brings the output back", async () => {
		const fake = fakeContext("suspended");
		const g = guest();
		const handle = attachAc97(g.emulator, { context: fake.context });
		g.boot();
		await new Promise((resolve) => setTimeout(resolve, 5));
		expect(handle.outputState()).toBe("suspended");
		g.play(24000, 1000); // half a second
		await new Promise((resolve) => setTimeout(resolve, 150));
		expect(handle.stats().drainedFrames).toBeGreaterThan(2000);
		expect(await handle.resume()).toBe("running");
		expect(handle.outputState()).toBe("running");
		handle.detach();
		expect(handle.outputState()).toBe("none"); // a detached card has no output, though the page's context goes on
	});

	test("an output that cannot be built is reported, and the card still runs", async () => {
		const broken = fakeContext() as { context: AudioContext; raw: Record<string, unknown> };
		broken.raw.createGain = () => {
			throw new Error("no gain");
		};
		const g = guest();
		const handle = attachAc97(g.emulator, { context: broken.context });
		g.boot();
		await new Promise((resolve) => setTimeout(resolve, 5));
		expect(handle.outputState()).toBe("failed");
		expect(handle.stats().registered).toBe(true);
		handle.detach();
	});
});
