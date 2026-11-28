// Register-model tests for the AC'97 device: reset values, codec ready, descriptor-list walking, status bits, the interrupt line and the saved state.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, test } from "bun:test";
import { Ac97, CR, GS, SR, isAc97State } from "../src/model.ts";

const PO = 0x10;

function machine() {
	const mem = new Uint8Array(1 << 20);
	const irq = { raised: 0, lowered: 0 };
	const dev = new Ac97({ readMem: (a, n) => mem.subarray(a, a + n), raiseIrq: () => irq.raised++, lowerIrq: () => irq.lowered++ });
	return { mem, irq, dev };
}

/** A descriptor list at `bdl` of `count` buffers, each `frames` stereo frames of a constant sample, at 0x20000 + i * 0x1000. */
function fillList(mem: Uint8Array, bdl: number, count: number, frames: number, { ioc = true, value = 8192 } = {}): void {
	const dv = new DataView(mem.buffer);
	for (let i = 0; i < count; i++) {
		const addr = 0x20000 + i * 0x1000;
		dv.setUint32(bdl + i * 8, addr, true);
		dv.setUint32(bdl + i * 8 + 4, ((frames * 2) | (ioc ? 0x80000000 : 0)) >>> 0, true);
		for (let f = 0; f < frames; f++) {
			dv.setInt16(addr + f * 4, value, true);
			dv.setInt16(addr + f * 4 + 2, -value, true);
		}
	}
}

describe("the register model", () => {
	test("the codec resets to a ready, muted plain AC'97 2.3 codec", () => {
		const { dev } = machine();
		expect(dev.namRead(0x26, 2) & 0xf).toBe(0xf); // power-down status: all sections ready
		expect(dev.namRead(0x02, 2)).toBe(0x8000); // master volume muted
		expect(dev.namRead(0x28, 2) & 1).toBe(1); // variable rate audio offered
		expect(dev.namRead(0x7c, 2)).toBe(0x8384);
		expect(dev.namRead(0x2c, 2)).toBe(48000);
		expect(dev.nabmRead(0x30, 4) & GS.CODEC_READY).toBe(GS.CODEC_READY); // primary codec ready in the global status
		expect(dev.nabmRead(PO + 6, 2) & SR.DCH).toBe(SR.DCH); // PCM out halted
	});

	test("the sample rate follows the register only once variable rate is on", () => {
		const { dev } = machine();
		dev.namWrite(0x2c, 2, 44100);
		expect(dev.rate).toBe(48000);
		dev.namWrite(0x2a, 2, 1);
		dev.namWrite(0x2c, 2, 44100);
		expect(dev.rate).toBe(44100);
		expect(dev.namRead(0x2c, 2)).toBe(44100);
		dev.namWrite(0x00, 2, 0);
		expect(dev.rate).toBe(48000); // a codec reset puts it back
	});

	test("byte, word and dword access to the same register agree", () => {
		const { dev } = machine();
		dev.nabmWrite(PO, 4, 0x12345678);
		expect(dev.nabmRead(PO, 4)).toBe(0x12345678 & ~7); // buffer list base is 8-byte aligned
		expect(dev.nabmRead(PO + 1, 1)).toBe(0x56);
		dev.nabmWrite(PO + 5, 1, 0xff);
		expect(dev.nabmRead(PO + 5, 1)).toBe(0x1f); // last valid index is five bits
	});

	test("walks the descriptor list, sets IOC and last-valid status, and halts", () => {
		const { mem, irq, dev } = machine();
		fillList(mem, 0x10000, 3, 100);
		dev.nabmWrite(PO, 4, 0x10000);
		dev.nabmWrite(PO + 5, 1, 2);
		dev.nabmWrite(PO + 0xb, 1, CR.RPBM | CR.IOCE | CR.LVBIE);
		expect(dev.nabmRead(PO + 6, 2) & SR.DCH).toBe(0); // running
		const l = new Float32Array(500);
		const r = new Float32Array(500);
		const made = dev.readFrames(500, l, r);
		expect(made).toBe(300);
		expect(l[0]).toBe(0.25);
		expect(r[0]).toBe(-0.25);
		expect(dev.counters.buffersDone).toBe(3);
		const sr = dev.nabmRead(PO + 6, 2);
		expect(sr & (SR.BCIS | SR.LVBCI | SR.DCH | SR.CELV)).toBe(SR.BCIS | SR.LVBCI | SR.DCH | SR.CELV);
		expect(dev.nabmRead(PO + 4, 1)).toBe(2); // current index stays on the last valid buffer
		expect(dev.nabmRead(0x30, 4) & GS.POINT).toBe(GS.POINT); // PCM-out interrupt in the global status
		expect(irq.raised).toBe(1); // one interrupt line assertion
		dev.nabmWrite(PO + 6, 2, SR.BCIS | SR.LVBCI);
		expect(irq.lowered).toBe(1); // writing ones clears the status and drops the line
		expect(dev.nabmRead(0x30, 4) & GS.POINT).toBe(0);
	});

	test("no interrupt without the enable bits, and PICB counts down within a buffer", () => {
		const { mem, irq, dev } = machine();
		fillList(mem, 0x10000, 2, 100);
		dev.nabmWrite(PO, 4, 0x10000);
		dev.nabmWrite(PO + 5, 1, 1);
		dev.nabmWrite(PO + 0xb, 1, CR.RPBM);
		dev.readFrames(60, new Float32Array(60), new Float32Array(60));
		expect(dev.nabmRead(PO + 8, 2)).toBe(200 - 120); // samples left in the buffer (16-bit words)
		expect(dev.nabmRead(PO + 0xa, 1)).toBe(1); // prefetched index is the next one
		dev.readFrames(200, new Float32Array(200), new Float32Array(200));
		expect(irq.raised).toBe(0);
		expect(dev.nabmRead(PO + 6, 2) & SR.BCIS).toBeTruthy(); // status still shows the completion
	});

	test("a new last valid index resumes a halted channel at the next buffer", () => {
		const { mem, dev } = machine();
		fillList(mem, 0x10000, 4, 50);
		dev.nabmWrite(PO, 4, 0x10000);
		dev.nabmWrite(PO + 5, 1, 1);
		dev.nabmWrite(PO + 0xb, 1, CR.RPBM | CR.IOCE);
		dev.readFrames(1000, new Float32Array(1000), new Float32Array(1000));
		expect(dev.running).toBe(false);
		dev.nabmWrite(PO + 5, 1, 3);
		expect(dev.running).toBe(true);
		const made = dev.readFrames(1000, new Float32Array(1000), new Float32Array(1000));
		expect(made).toBe(100); // buffers 2 and 3
		expect(dev.counters.dmaFrames).toBe(200);
	});

	test("the reset bit clears the channel", () => {
		const { mem, dev } = machine();
		fillList(mem, 0x10000, 2, 50);
		dev.nabmWrite(PO, 4, 0x10000);
		dev.nabmWrite(PO + 5, 1, 1);
		dev.nabmWrite(PO + 0xb, 1, CR.RPBM);
		dev.nabmWrite(PO + 0xb, 1, CR.RR);
		expect(dev.nabmRead(PO, 4)).toBe(0);
		expect(dev.nabmRead(PO + 6, 2)).toBe(SR.DCH);
		expect(dev.nabmRead(PO + 0xb, 1)).toBe(0);
	});

	test("render resamples 44.1 kHz to 48 kHz and applies the volume registers", () => {
		const { mem, dev } = machine();
		fillList(mem, 0x10000, 2, 4410, { value: 16384 });
		dev.namWrite(0x2a, 2, 1);
		dev.namWrite(0x2c, 2, 44100);
		dev.namWrite(0x02, 2, 0x0000); // master 0 dB
		dev.namWrite(0x18, 2, 0x0808); // PCM out 0 dB
		dev.nabmWrite(PO, 4, 0x10000);
		dev.nabmWrite(PO + 5, 1, 1);
		dev.nabmWrite(PO + 0xb, 1, CR.RPBM);
		const l = new Float32Array(4800);
		const r = new Float32Array(4800);
		const real = dev.render(4800, 48000, l, r);
		expect(real).toBeGreaterThanOrEqual(4790); // 4800 output frames take about 4410 input frames
		expect(Math.abs(dev.counters.dmaFrames - 4410)).toBeLessThan(24); // the filter reads 16 frames ahead
		expect(Math.abs((l[100] ?? 0) - 0.5)).toBeLessThan(1e-6);
		dev.namWrite(0x02, 2, 0x8000);
		dev.render(100, 48000, l, r);
		expect(l[10]).toBe(0); // muted
	});

	test("silence while the guest is not playing, and no starved frames counted", () => {
		const { dev } = machine();
		const l = new Float32Array(64).fill(1);
		const r = new Float32Array(64).fill(1);
		expect(dev.render(64, 48000, l, r)).toBe(0);
		expect(l.every((v) => v === 0)).toBe(true);
		expect(dev.counters.starvedFrames).toBe(0);
	});
});

describe("the saved state", () => {
	test("round-trips through JSON", () => {
		const { mem, dev } = machine();
		fillList(mem, 0x10000, 2, 50);
		dev.namWrite(0x02, 2, 0x0a0a);
		dev.nabmWrite(PO, 4, 0x10000);
		dev.nabmWrite(PO + 5, 1, 1);
		const saved = JSON.parse(JSON.stringify(dev.getState()));
		expect(saved.version).toBe(1);
		expect(isAc97State(saved)).toBe(true);
		const other = machine();
		other.dev.setState(saved);
		expect(other.dev.namRead(0x02, 2)).toBe(0x0a0a);
		expect(other.dev.nabmRead(PO, 4)).toBe(0x10000);
		expect(other.dev.nabmRead(PO + 5, 1)).toBe(1);
	});

	test("refuses a state of the wrong shape and keeps the registers it had", () => {
		const { dev } = machine();
		dev.namWrite(0x02, 2, 0x0a0a);
		const good = dev.getState();
		expect(isAc97State(null)).toBe(false);
		expect(isAc97State({ ...good, version: 2 })).toBe(false);
		expect(isAc97State({ ...good, nam: good.nam.slice(1) })).toBe(false);
		expect(isAc97State({ ...good, nam: good.nam.map(() => 70000) })).toBe(false);
		expect(isAc97State({ ...good, ch: good.ch.slice(1) })).toBe(false);
		expect(() => dev.setState({ ...good, version: 2 } as never)).toThrow();
		expect(dev.namRead(0x02, 2)).toBe(0x0a0a);
	});

	test("a restored interrupt that was pending is asserted again", () => {
		const { mem, irq, dev } = machine();
		fillList(mem, 0x10000, 1, 10);
		dev.nabmWrite(PO, 4, 0x10000);
		dev.nabmWrite(PO + 5, 1, 0);
		dev.nabmWrite(PO + 0xb, 1, CR.RPBM | CR.IOCE);
		dev.readFrames(100, new Float32Array(100), new Float32Array(100));
		const saved = dev.getState();
		const other = machine();
		other.dev.setState(saved);
		expect(other.irq.raised).toBe(1);
		expect(irq.raised).toBe(1);
	});
});

describe("holding for the guest's interrupt", () => {
	function playing() {
		const m = machine();
		fillList(m.mem, 0x10000, 4, 50);
		m.dev.nabmWrite(PO, 4, 0x10000);
		m.dev.nabmWrite(PO + 5, 1, 3);
		m.dev.nabmWrite(PO + 0xb, 1, CR.RPBM | CR.IOCE);
		return m;
	}

	test("without a hold, buffers complete back to back and their interrupts merge", () => {
		const { dev } = playing();
		expect(dev.readFrames(200, new Float32Array(200), new Float32Array(200))).toBe(200);
		expect(dev.counters.iocs).toBe(4);
		expect(dev.counters.irqRaised).toBe(1);
	});

	test("with a hold, a buffer's interrupt must be taken before the next buffer plays", () => {
		const { dev, irq } = playing();
		const l = new Float32Array(200);
		const r = new Float32Array(200);
		expect(dev.readFrames(200, l, r, 0, true)).toBe(50); // the first buffer, then it waits
		expect(irq.raised).toBe(1);
		expect(dev.readFrames(200, l, r, 0, true)).toBe(0); // the guest has not cleared it
		dev.nabmWrite(PO + 6, 2, SR.BCIS); // the guest's handler
		expect(irq.lowered).toBe(1);
		expect(dev.readFrames(200, l, r, 0, true)).toBe(50);
		expect(dev.counters.iocs).toBe(2);
		expect(dev.counters.irqRaised).toBe(2); // one interrupt for each buffer
	});

	test("a hold does not wait when the guest has the interrupt masked", () => {
		const { dev } = playing();
		dev.nabmWrite(PO + 0xb, 1, CR.RPBM); // IOC enable off
		expect(dev.readFrames(200, new Float32Array(200), new Float32Array(200), 0, true)).toBe(200);
	});

	test("starved frames are not counted for a hold, which is silence on purpose", () => {
		const { dev } = playing();
		const l = new Float32Array(480);
		const r = new Float32Array(480);
		dev.render(480, 48000, l, r, true);
		expect(dev.counters.starvedFrames).toBe(0);
	});
});
