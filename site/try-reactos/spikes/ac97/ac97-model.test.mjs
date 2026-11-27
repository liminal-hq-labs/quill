// Register-model tests for the AC'97 spike device: reset values, codec ready, descriptor-list walking, status bits and the interrupt line.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import test from "node:test";
import assert from "node:assert/strict";
import { Ac97, SR, CR, GS } from "./ac97-model.js";

const PO = 0x10;

function machine() {
	const mem = new Uint8Array(1 << 20);
	const irq = { raised: 0, lowered: 0 };
	const dev = new Ac97({ readMem: (a, n) => mem.subarray(a, a + n), raiseIrq: () => irq.raised++, lowerIrq: () => irq.lowered++ });
	return { mem, irq, dev };
}

// A descriptor list at `bdl` of `count` buffers, each `frames` stereo frames of a constant sample, at 0x20000 + i * 0x1000.
function fillList(mem, bdl, count, frames, { ioc = true, value = 8192 } = {}) {
	const dv = new DataView(mem.buffer);
	for (let i = 0; i < count; i++) {
		const addr = 0x20000 + i * 0x1000;
		dv.setUint32(bdl + i * 8, addr, true);
		dv.setUint32(bdl + i * 8 + 4, (frames * 2) | (ioc ? 0x80000000 : 0), true);
		for (let f = 0; f < frames; f++) {
			dv.setInt16(addr + f * 4, value, true);
			dv.setInt16(addr + f * 4 + 2, -value, true);
		}
	}
}

test("codec resets to a ready, muted plain AC'97 2.3 codec", () => {
	const { dev } = machine();
	assert.equal(dev.namRead(0x26, 2) & 0xf, 0xf, "power-down status: all sections ready");
	assert.equal(dev.namRead(0x02, 2), 0x8000, "master volume muted");
	assert.equal(dev.namRead(0x28, 2) & 1, 1, "variable rate audio offered");
	assert.equal(dev.namRead(0x7c, 2), 0x8384);
	assert.equal(dev.namRead(0x2c, 2), 48000);
	assert.equal(dev.nabmRead(0x30, 4) & GS.CODEC_READY, GS.CODEC_READY, "primary codec ready in the global status");
	assert.equal(dev.nabmRead(PO + 6, 2) & SR.DCH, SR.DCH, "PCM out halted");
});

test("the sample rate follows the register only once variable rate is on", () => {
	const { dev } = machine();
	dev.namWrite(0x2c, 2, 44100);
	assert.equal(dev.rate, 48000);
	dev.namWrite(0x2a, 2, 1);
	dev.namWrite(0x2c, 2, 44100);
	assert.equal(dev.rate, 44100);
	assert.equal(dev.namRead(0x2c, 2), 44100);
	dev.namWrite(0x00, 2, 0);
	assert.equal(dev.rate, 48000, "a codec reset puts it back");
});

test("byte, word and dword access to the same register agree", () => {
	const { dev } = machine();
	dev.nabmWrite(PO, 4, 0x12345678);
	assert.equal(dev.nabmRead(PO, 4), 0x12345678 & ~7, "buffer list base is 8-byte aligned");
	assert.equal(dev.nabmRead(PO + 1, 1), 0x56);
	dev.nabmWrite(PO + 5, 1, 0xff);
	assert.equal(dev.nabmRead(PO + 5, 1), 0x1f, "last valid index is five bits");
});

test("walks the descriptor list, sets IOC and last-valid status, and halts", () => {
	const { mem, irq, dev } = machine();
	fillList(mem, 0x10000, 3, 100);
	dev.nabmWrite(PO, 4, 0x10000);
	dev.nabmWrite(PO + 5, 1, 2);
	dev.nabmWrite(PO + 0xb, 1, CR.RPBM | CR.IOCE | CR.LVBIE);
	assert.equal(dev.nabmRead(PO + 6, 2) & SR.DCH, 0, "running");
	const l = new Float32Array(500);
	const r = new Float32Array(500);
	const made = dev.readFrames(500, l, r);
	assert.equal(made, 300);
	assert.equal(l[0], 0.25);
	assert.equal(r[0], -0.25);
	assert.equal(dev.counters.buffersDone, 3);
	const sr = dev.nabmRead(PO + 6, 2);
	assert.equal(sr & (SR.BCIS | SR.LVBCI | SR.DCH | SR.CELV), SR.BCIS | SR.LVBCI | SR.DCH | SR.CELV);
	assert.equal(dev.nabmRead(PO + 4, 1), 2, "current index stays on the last valid buffer");
	assert.equal(dev.nabmRead(0x30, 4) & GS.POINT, GS.POINT, "PCM-out interrupt in the global status");
	assert.equal(irq.raised, 1, "one interrupt line assertion");
	dev.nabmWrite(PO + 6, 2, SR.BCIS | SR.LVBCI);
	assert.equal(irq.lowered, 1, "writing ones clears the status and drops the line");
	assert.equal(dev.nabmRead(0x30, 4) & GS.POINT, 0);
});

test("no interrupt without the enable bits, and PICB counts down within a buffer", () => {
	const { mem, irq, dev } = machine();
	fillList(mem, 0x10000, 2, 100);
	dev.nabmWrite(PO, 4, 0x10000);
	dev.nabmWrite(PO + 5, 1, 1);
	dev.nabmWrite(PO + 0xb, 1, CR.RPBM);
	const l = new Float32Array(60);
	const r = new Float32Array(60);
	dev.readFrames(60, l, r);
	assert.equal(dev.nabmRead(PO + 8, 2), 200 - 120, "samples left in the buffer (16-bit words)");
	assert.equal(dev.nabmRead(PO + 0xa, 1), 1, "prefetched index is the next one");
	dev.readFrames(200, new Float32Array(200), new Float32Array(200));
	assert.equal(irq.raised, 0);
	assert.ok(dev.nabmRead(PO + 6, 2) & SR.BCIS, "status still shows the completion");
});

test("a new last valid index resumes a halted channel at the next buffer", () => {
	const { mem, dev } = machine();
	fillList(mem, 0x10000, 4, 50);
	dev.nabmWrite(PO, 4, 0x10000);
	dev.nabmWrite(PO + 5, 1, 1);
	dev.nabmWrite(PO + 0xb, 1, CR.RPBM | CR.IOCE);
	dev.readFrames(1000, new Float32Array(1000), new Float32Array(1000));
	assert.equal(dev.running, false);
	dev.nabmWrite(PO + 5, 1, 3);
	assert.equal(dev.running, true);
	const made = dev.readFrames(1000, new Float32Array(1000), new Float32Array(1000));
	assert.equal(made, 100, "buffers 2 and 3");
	assert.equal(dev.counters.dmaFrames, 200);
});

test("the reset bit clears the channel", () => {
	const { mem, dev } = machine();
	fillList(mem, 0x10000, 2, 50);
	dev.nabmWrite(PO, 4, 0x10000);
	dev.nabmWrite(PO + 5, 1, 1);
	dev.nabmWrite(PO + 0xb, 1, CR.RPBM);
	dev.nabmWrite(PO + 0xb, 1, CR.RR);
	assert.equal(dev.nabmRead(PO, 4), 0);
	assert.equal(dev.nabmRead(PO + 6, 2), SR.DCH);
	assert.equal(dev.nabmRead(PO + 0xb, 1), 0);
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
	assert.ok(real >= 4790, "4800 output frames take about 4410 input frames: " + real);
	assert.ok(Math.abs(dev.counters.dmaFrames - 4410) < 8);
	assert.ok(Math.abs(l[100] - 0.5) < 1e-6);
	dev.namWrite(0x02, 2, 0x8000);
	dev.render(100, 48000, l, r);
	assert.equal(l[10], 0, "muted");
});

test("the saved state round-trips", () => {
	const { mem, dev } = machine();
	fillList(mem, 0x10000, 2, 50);
	dev.namWrite(0x02, 2, 0x0a0a);
	dev.nabmWrite(PO, 4, 0x10000);
	dev.nabmWrite(PO + 5, 1, 1);
	const other = machine();
	other.dev.setState(JSON.parse(JSON.stringify(dev.getState())));
	assert.equal(other.dev.namRead(0x02, 2), 0x0a0a);
	assert.equal(other.dev.nabmRead(PO, 4), 0x10000);
	assert.equal(other.dev.nabmRead(PO + 5, 1), 1);
});
