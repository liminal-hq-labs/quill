// The microphone's privacy and lifecycle on fakes: never asked for until `start`, every track ended on stop and detach, the reasons for a refusal,
// the guest's silence when the microphone is off, and a scan that the package has no way to send audio anywhere.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import { readdirSync, readFileSync } from "node:fs";
import { attachAc97, micReason, type V86Cpu, type V86Emulator, type V86Machine } from "../src/index.ts";
import { CR } from "../src/model.ts";

class FakeTrack {
	readyState: "live" | "ended" = "live";
	private listeners: (() => void)[] = [];
	stop() {
		this.readyState = "ended";
	}
	addEventListener(_: string, listener: () => void) {
		this.listeners.push(listener);
	}
	end() {
		this.readyState = "ended";
		this.listeners.forEach((l) => l());
	}
}
class FakeStream {
	tracks = [new FakeTrack()];
	getTracks() {
		return this.tracks;
	}
	getAudioTracks() {
		return this.tracks;
	}
}

const asked: unknown[] = [];
let streams: FakeStream[] = [];
let outcome: "grant" | "NotAllowedError" | "NotFoundError" | "late" = "grant";
let release: (() => void) | null = null;
const ports: { onmessage: ((e: { data: Float32Array }) => void) | null; postMessage: (m: unknown) => void }[] = [];

class FakeNode {
	port: { onmessage: ((e: { data: Float32Array }) => void) | null; postMessage: (m: unknown) => void } = { onmessage: null, postMessage() {} };
	constructor() {
		ports.push(this.port);
	}
	connect(next: unknown) {
		return next;
	}
	disconnect() {}
}

function fakeContext() {
	return {
		sampleRate: 48000,
		currentTime: 0,
		state: "running",
		destination: {},
		audioWorklet: { addModule: async () => {} },
		createGain: () => ({ gain: { value: 1, cancelScheduledValues() {}, setTargetAtTime() {}, setValueAtTime() {} }, connect: (n: unknown) => n, disconnect() {} }),
		createScriptProcessor: () => ({ connect: (n: unknown) => n, disconnect() {}, onaudioprocess: null }),
		createMediaStreamSource: () => new FakeNode(),
		addEventListener() {},
		removeEventListener() {},
		resume: async () => {},
		close: async () => {},
	} as unknown as AudioContext;
}

function fakeMachine() {
	const memory = new Uint8Array(1 << 23);
	const ports = new Map<number, { w8?: (v: number) => void; w16?: (v: number) => void; w32?: (v: number) => void }>();
	const cpu: V86Cpu = {
		devices: { pci: { devices: {}, register_device() {}, raise_irq() {}, lower_irq() {} } },
		io: {
			register_read() {},
			register_write: (port, _o, w8, w16, w32) => void ports.set(port, { w8, w16, w32 }),
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
	const record = () => {
		const dv = new DataView(memory.buffer);
		for (let i = 0; i < 32; i++) {
			dv.setUint32(0x10000 + i * 8, 0x100000 + i * 0x10000, true);
			dv.setUint32(0x10000 + i * 8 + 4, 480 * 2, true);
		}
		ports.get(0xd000 + 0x1c)?.w16?.(0x0000);
		ports.get(0xd400)?.w32?.(0x10000);
		ports.get(0xd400 + 5)?.w8?.(31);
		ports.get(0xd400 + 0xb)?.w8?.(CR.RPBM);
	};
	return { emulator, boot, record, memory };
}

const original = { mediaDevices: Object.getOwnPropertyDescriptor(navigator, "mediaDevices"), node: (globalThis as Record<string, unknown>).AudioWorkletNode, secure: (globalThis as Record<string, unknown>).isSecureContext };

beforeEach(() => {
	asked.length = 0;
	streams = [];
	ports.length = 0;
	outcome = "grant";
	release = null;
	Object.defineProperty(navigator, "mediaDevices", {
		configurable: true,
		value: {
			getUserMedia: async (constraints: unknown) => {
				asked.push(constraints);
				if (outcome === "NotAllowedError" || outcome === "NotFoundError") throw new DOMException("no", outcome);
				if (outcome === "late") await new Promise<void>((resolve) => (release = resolve));
				const stream = new FakeStream();
				streams.push(stream);
				return stream;
			},
		},
	});
	(globalThis as Record<string, unknown>).AudioWorkletNode = FakeNode;
	(globalThis as Record<string, unknown>).isSecureContext = true;
});

afterEach(() => {
	if (original.mediaDevices) Object.defineProperty(navigator, "mediaDevices", original.mediaDevices);
	else delete (navigator as unknown as Record<string, unknown>).mediaDevices;
	(globalThis as Record<string, unknown>).AudioWorkletNode = original.node;
	(globalThis as Record<string, unknown>).isSecureContext = original.secure;
});

const tick = () => new Promise((resolve) => setTimeout(resolve, 5));

describe("the microphone is off until it is asked for", () => {
	test("attaching a card, booting and recording never ask the browser", async () => {
		const m = fakeMachine();
		const card = attachAc97(m.emulator, { context: fakeContext() });
		m.boot();
		m.record();
		await tick();
		expect(asked).toHaveLength(0);
		expect(card.micState()).toBe("off");
		expect(card.stats().mic).toBe("off");
		card.detach();
	});

	test("start asks once, with no clean-up filters and no video, and feeds the card's recording", async () => {
		const m = fakeMachine();
		const card = attachAc97(m.emulator, { context: fakeContext() });
		m.boot();
		m.record();
		expect(await card.startMicrophone()).toBe("on");
		expect(asked).toHaveLength(1);
		expect(asked[0]).toEqual({ audio: { echoCancellation: false, noiseSuppression: false, autoGainControl: false, channelCount: 1 }, video: false });
		// A press while it is on does not ask again.
		await card.startMicrophone();
		expect(asked).toHaveLength(1);
		// Blocks from the worklet reach the guest's buffer.
		const block = new Float32Array(480).fill(0.25);
		ports[ports.length - 1]?.onmessage?.({ data: block });
		const counters = card.stats().counters!;
		expect(counters.capFrames - counters.capSilentFrames).toBe(480);
		expect(new Int16Array(m.memory.buffer, 0x100000, 2 * counters.capFrames)[2 * counters.capSilentFrames]).toBe(Math.round(0.25 * 32767));
		card.detach();
	});

	test("stop ends every track, and so does detach", async () => {
		const m = fakeMachine();
		const card = attachAc97(m.emulator, { context: fakeContext() });
		m.boot();
		await card.startMicrophone();
		expect(card.microphone?.liveTracks()).toBe(1);
		card.stopMicrophone();
		expect(streams[0]?.tracks.every((t) => t.readyState === "ended")).toBe(true);
		expect(card.micState()).toBe("off");
		await card.startMicrophone();
		expect(streams).toHaveLength(2);
		card.detach();
		expect(streams[1]?.tracks.every((t) => t.readyState === "ended")).toBe(true);
		expect(card.micState()).toBe("off");
	});

	test("a track the browser ends (the visitor revokes it) turns the microphone off", async () => {
		const m = fakeMachine();
		const card = attachAc97(m.emulator, { context: fakeContext() });
		m.boot();
		await card.startMicrophone();
		streams[0]?.tracks[0]?.end();
		expect(card.micState()).toBe("off");
		card.detach();
	});

	test("stopping while the permission question is open lets the microphone go the moment it is given", async () => {
		const m = fakeMachine();
		const card = attachAc97(m.emulator, { context: fakeContext() });
		m.boot();
		outcome = "late";
		const pending = card.startMicrophone();
		await tick();
		expect(card.micState()).toBe("asking");
		card.stopMicrophone();
		release?.();
		await pending;
		expect(card.micState()).toBe("off");
		expect(streams[0]?.tracks.every((t) => t.readyState === "ended")).toBe(true);
		card.detach();
	});

	test("a refusal, a missing microphone and an insecure page each say why", async () => {
		const m = fakeMachine();
		const card = attachAc97(m.emulator, { context: fakeContext() });
		m.boot();
		outcome = "NotAllowedError";
		expect(await card.startMicrophone()).toBe("denied");
		expect(micReason("denied")).toMatch(/blocked/);
		outcome = "NotFoundError";
		expect(await card.startMicrophone()).toBe("no-device");
		expect(micReason("no-device")).toMatch(/No microphone/);
		(globalThis as Record<string, unknown>).isSecureContext = false;
		expect(await card.startMicrophone()).toBe("insecure");
		expect(micReason("insecure")).toMatch(/HTTPS/);
		(globalThis as Record<string, unknown>).isSecureContext = true;
		delete (navigator as unknown as Record<string, unknown>).mediaDevices;
		expect(await card.startMicrophone()).toBe("unavailable");
		card.detach();
	});

	test("with no audio output at all the microphone is unavailable and the guest still records", async () => {
		const m = fakeMachine();
		const card = attachAc97(m.emulator, { context: null });
		m.boot();
		m.record();
		expect(await card.startMicrophone()).toBe("unavailable");
		expect(asked).toHaveLength(0);
		await new Promise((resolve) => setTimeout(resolve, 120));
		expect(card.stats().counters?.capSilentFrames).toBeGreaterThan(1000);
		card.detach();
	});
});

describe("what the guest hears with the microphone off", () => {
	test("silence at the real-time rate, in whole buffers, until a microphone is turned on", async () => {
		const m = fakeMachine();
		const card = attachAc97(m.emulator, { context: fakeContext() });
		m.boot();
		m.record();
		await new Promise((resolve) => setTimeout(resolve, 200));
		const stats = card.stats().counters!;
		expect(stats.capSilentFrames).toBeGreaterThan(5000);
		expect(stats.capFrames).toBe(stats.capSilentFrames);
		expect(new Int16Array(m.memory.buffer, 0x100000, 960).every((v) => v === 0)).toBe(true);
		card.detach();
	});
});

describe("nothing goes anywhere", () => {
	test("the package has no way to send, store or record audio beyond the card", () => {
		const forbidden = /\b(fetch|XMLHttpRequest|sendBeacon|WebSocket|RTCPeerConnection|MediaRecorder|localStorage|sessionStorage|indexedDB|BroadcastChannel|postMessage\(\s*[^)]*targetOrigin|navigator\.share|caches)\b/;
		const dir = new URL("../src/", import.meta.url);
		for (const name of readdirSync(dir)) {
			const code = readFileSync(new URL(name, dir), "utf8")
				.split("\n")
				.filter((line) => !line.trim().startsWith("//") && !line.trim().startsWith("*") && !line.trim().startsWith("/**"))
				.join("\n");
			const found = forbidden.exec(code);
			expect(found === null ? "" : `${name}: ${found[0]}`).toBe("");
		}
	});
});
