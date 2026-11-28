// The recording side of the card: the PCM-in and mic-in channels, the record select and gain registers, the ADC rate, holding for the guest's interrupt,
// and what the guest finds in its buffers.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, test } from "bun:test";
import { Ac97, CR, SR } from "../src/model.ts";

const PI = 0x00;
const MC = 0x20;

function machine() {
	const mem = new Uint8Array(1 << 21);
	const irq = { raised: 0, lowered: 0 };
	const dev = new Ac97({
		readMem: (a, n) => mem.subarray(a, a + n),
		writeMem: (a, bytes) => mem.set(bytes, a),
		raiseIrq: () => irq.raised++,
		lowerIrq: () => irq.lowered++,
	});
	return { mem, irq, dev };
}

/** A list of `count` empty buffers of `frames` frames (`per` 16-bit samples a frame) at 0x100000 + i * 0x10000, the descriptor list at 0x10000. */
function emptyList(mem: Uint8Array, count: number, frames: number, per = 2, ioc = true): void {
	const dv = new DataView(mem.buffer);
	for (let i = 0; i < count; i++) {
		dv.setUint32(0x10000 + i * 8, 0x100000 + i * 0x10000, true);
		dv.setUint32(0x10000 + i * 8 + 4, ((frames * per) | (ioc ? 0x80000000 : 0)) >>> 0, true);
	}
}

function record(dev: Ac97, base: number, last: number, extra = 0): void {
	dev.nabmWrite(base, 4, 0x10000);
	dev.nabmWrite(base + 5, 1, last);
	dev.nabmWrite(base + 0xb, 1, CR.RPBM | CR.IOCE | extra);
}

const samples = (mem: Uint8Array, buffer: number, count: number) => new Int16Array(mem.buffer.slice(0x100000 + buffer * 0x10000, 0x100000 + buffer * 0x10000 + count * 2));

describe("recording", () => {
	test("the guest records silence when no microphone is on, and the buffers complete in order", () => {
		const { mem, dev, irq } = machine();
		mem.fill(0x55, 0x100000, 0x100000 + 3 * 0x10000);
		emptyList(mem, 3, 100);
		record(dev, PI, 2);
		expect(dev.recording).toBe(true);
		dev.feedSilence(100);
		expect(dev.counters.capFrames).toBe(100);
		expect(dev.counters.capBuffers).toBe(1);
		expect(Array.from(samples(mem, 0, 200)).every((v) => v === 0)).toBe(true); // written as zeros over what was there
		expect(irq.raised).toBe(1);
		// The guest has not taken the interrupt: the next buffer waits.
		dev.feedSilence(100);
		expect(dev.counters.capBuffers).toBe(1);
		dev.nabmWrite(PI + 6, 2, SR.BCIS);
		dev.feedSilence(100);
		expect(dev.counters.capBuffers).toBe(2);
		expect(dev.counters.capFrames).toBe(200);
		expect(dev.counters.capDropped).toBe(0);
	});

	test("the record gain register scales what is heard, and is muted at reset", () => {
		const { mem, dev } = machine();
		emptyList(mem, 2, 64);
		record(dev, PI, 1);
		const tone = Float32Array.from({ length: 64 }, () => 0.5);
		dev.nabmWrite(PI + 6, 2, SR.BCIS);
		dev.feedCapture(tone, 64, 48000);
		expect(Array.from(samples(mem, 0, 128)).every((v) => v === 0)).toBe(true); // 0x8000 at reset: muted
		dev.nabmWrite(PI + 6, 2, SR.BCIS);
		dev.namWrite(0x1c, 2, 0x0000); // 0 dB
		dev.feedCapture(tone, 64, 48000);
		expect(samples(mem, 1, 4)[0]).toBe(Math.round(0.5 * 32767));
	});

	test("+22.5 dB of record gain, and a clip at full scale", () => {
		const { mem, dev } = machine();
		emptyList(mem, 1, 8);
		dev.namWrite(0x1c, 2, 0x0f0f);
		record(dev, PI, 0);
		dev.feedCapture(Float32Array.from({ length: 8 }, () => 0.5), 8, 48000);
		expect(samples(mem, 0, 2)[0]).toBe(32767); // 0.5 * 13.3 is past full scale
		dev.namWrite(0x1c, 2, 0x0101); // +1.5 dB
		const { mem: mem2, dev: dev2 } = machine();
		emptyList(mem2, 1, 8);
		dev2.namWrite(0x1c, 2, 0x0101);
		record(dev2, PI, 0);
		dev2.feedCapture(Float32Array.from({ length: 8 }, () => 0.25), 8, 48000);
		expect(Math.abs((samples(mem2, 0, 2)[0] as number) / 32767 - 0.25 * Math.pow(10, 1.5 / 20))).toBeLessThan(1e-4);
	});

	test("the record select register picks the microphone; any other source records silence", () => {
		const { mem, dev } = machine();
		emptyList(mem, 1, 8);
		dev.namWrite(0x1c, 2, 0x0000);
		dev.namWrite(0x1a, 2, 0x0004); // right: line in
		record(dev, PI, 0);
		dev.feedCapture(Float32Array.from({ length: 8 }, () => 0.5), 8, 48000);
		const s = samples(mem, 0, 4);
		expect(s[0]).toBe(16384);
		expect(s[1]).toBe(0);
	});

	test("an ADC rate the guest sets is clamped and used, and the microphone is resampled to it", () => {
		const { mem, dev } = machine();
		dev.namWrite(0x2a, 2, 1);
		dev.namWrite(0x32, 2, 1000);
		expect(dev.adcRate).toBe(8000);
		dev.namWrite(0x32, 2, 60000);
		expect(dev.adcRate).toBe(48000);
		dev.namWrite(0x32, 2, 22050);
		expect(dev.namRead(0x32, 2)).toBe(22050);
		expect(dev.adcRate).toBe(22050);
		dev.namWrite(0x1c, 2, 0x0000);
		emptyList(mem, 8, 4000);
		record(dev, PI, 7);
		// One second of a 1 kHz tone at 48 kHz in 10 ms pieces, the guest taking each buffer's interrupt as it comes.
		for (let block = 0; block < 100; block++) {
			const piece = Float32Array.from({ length: 480 }, (_, i) => 0.5 * Math.sin((2 * Math.PI * 1000 * (block * 480 + i)) / 48000));
			dev.feedCapture(piece, 480, 48000);
			dev.nabmWrite(PI + 6, 2, SR.BCIS);
		}
		expect(Math.abs(dev.counters.capFrames - 22050)).toBeLessThanOrEqual(24);
		expect(dev.counters.capDropped).toBe(0);
		// The tone's frequency in the guest's buffer, from its zero crossings.
		const s = samples(mem, 0, 8000);
		let crossings = 0;
		let first = -1;
		let last = -1;
		for (let i = 1; i < 4000; i++) {
			if ((s[2 * (i - 1)] as number) < 0 && (s[2 * i] as number) >= 0) {
				crossings++;
				if (first < 0) first = i;
				last = i;
			}
		}
		const frequency = ((crossings - 1) * 22050) / (last - first);
		expect(Math.abs(frequency - 1000)).toBeLessThan(5);
	});

	test("the mic-in channel records one sample a frame", () => {
		const { mem, dev } = machine();
		dev.namWrite(0x1c, 2, 0x0000);
		emptyList(mem, 1, 16, 1);
		record(dev, MC, 0);
		expect(dev.recording).toBe(true);
		dev.feedCapture(Float32Array.from({ length: 16 }, (_, i) => i / 32), 16, 48000);
		expect(dev.counters.capFrames).toBe(16);
		const s = samples(mem, 0, 16);
		expect(s[8]).toBe(Math.round((8 / 32) * 32767));
	});

	test("nothing is kept when the guest is not recording", () => {
		const { dev } = machine();
		dev.feedCapture(new Float32Array(480).fill(0.5), 480, 48000);
		dev.feedSilence(480);
		expect(dev.counters.capFrames).toBe(0);
		expect(dev.counters.capSilentFrames).toBe(0);
	});

	test("a guest that never takes its interrupt loses frames after a second, and they are counted", () => {
		const { mem, dev } = machine();
		emptyList(mem, 4, 100);
		record(dev, PI, 3);
		for (let n = 0; n < 12; n++) dev.feedSilence(8000);
		expect(dev.counters.capBuffers).toBe(1);
		expect(dev.counters.capDropped).toBeGreaterThan(40000);
	});

	test("recorded frames are exactly the frames fed: a ramp comes back without a gap or a repeat", () => {
		const { mem, dev } = machine();
		dev.namWrite(0x1c, 2, 0x0000);
		emptyList(mem, 8, 500);
		record(dev, PI, 7);
		let next = 0;
		for (let block = 0; block < 64; block++) {
			const piece = Float32Array.from({ length: 53 }, () => (next++ % 2000) / 4000);
			dev.feedCapture(piece, 53, 48000);
			dev.nabmWrite(PI + 6, 2, SR.BCIS);
		}
		expect(dev.counters.capFrames).toBe(53 * 64);
		const all = [...samples(mem, 0, 1000), ...samples(mem, 1, 1000), ...samples(mem, 2, 1000), ...samples(mem, 3, 1000), ...samples(mem, 4, 1000), ...samples(mem, 5, 1000)];
		for (let i = 0; i < 6 * 500; i++) expect(all[2 * i]).toBe(Math.round(((i % 2000) / 4000) * 32767));
	});
});
