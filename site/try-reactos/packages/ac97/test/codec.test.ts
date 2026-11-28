// The codec's mixer as the page reads it: `codec()` decodes every step of each line and the mute bit, the power-on state is muted, and `onCodec` reports a
// guest's writes once per register per 50 ms window and stops with detach.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, test } from "bun:test";
import { attachAc97 } from "../src/index.ts";
import { fakeV86 } from "./fake-v86.ts";

const NAM = 0xd000;
const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

function card(options: Parameters<typeof attachAc97>[1] = {}) {
	const v86 = fakeV86();
	const calls: [number, number][] = [];
	const handle = attachAc97(v86.emulator, { context: null, onCodec: (register, value) => calls.push([register, value]), ...options });
	const write = (register: number, value: number) => v86.port(NAM + register).w16?.(value);
	return { v86, handle, calls, write, boot: () => v86.emulator.boot() };
}

describe("codec()", () => {
	test("is null before the card is registered", () => {
		const { handle } = card();
		expect(handle.codec()).toBeNull();
		handle.detach();
	});

	test("starts with master and PCM out muted, the codec's power-on state", () => {
		const { handle, boot } = card();
		boot();
		const codec = handle.codec();
		expect(codec?.master).toEqual({ raw: 0x8000, left: 100, right: 100, muted: true });
		expect(codec?.pcm.muted).toBe(true);
		expect(codec?.pcm.raw).toBe(0x8808);
		expect(codec?.record.muted).toBe(true);
		expect(codec?.mic.muted).toBe(true);
		expect(codec?.rate).toBe(48000);
		expect(codec?.vra).toBe(false);
		handle.detach();
	});

	test("decodes every step of master and PCM out on each side, in percent of the range", () => {
		const { handle, boot, write } = card();
		boot();
		for (const [register, line] of [
			[0x02, "master"],
			[0x18, "pcm"],
		] as const) {
			for (let step = 0; step < 32; step++) {
				const want = Math.round(((31 - step) * 100) / 31);
				write(register, (step << 8) | 31);
				expect(handle.codec()?.[line]).toEqual({ raw: (step << 8) | 31, left: want, right: 0, muted: false });
				write(register, (31 << 8) | step);
				expect(handle.codec()?.[line].right).toBe(want);
			}
		}
		handle.detach();
	});

	test("the loudest step is 100 percent and the quietest 0", () => {
		const { handle, boot, write } = card();
		boot();
		write(0x02, 0x0000);
		expect(handle.codec()?.master).toEqual({ raw: 0, left: 100, right: 100, muted: false });
		write(0x02, 0x1f1f);
		expect(handle.codec()?.master).toEqual({ raw: 0x1f1f, left: 0, right: 0, muted: false });
		handle.detach();
	});

	test("decodes every step of the record gain (4 bits, 15 loudest)", () => {
		const { handle, boot, write } = card();
		boot();
		for (let step = 0; step < 16; step++) {
			const want = Math.round((step * 100) / 15);
			write(0x1c, (step << 8) | 0);
			expect(handle.codec()?.record).toEqual({ raw: step << 8, left: want, right: 0, muted: false });
			write(0x1c, step);
			expect(handle.codec()?.record.right).toBe(want);
		}
		handle.detach();
	});

	test("decodes every step of the microphone volume (one 5-bit channel) and its mute", () => {
		const { handle, boot, write } = card();
		boot();
		for (let step = 0; step < 32; step++) {
			const want = Math.round(((31 - step) * 100) / 31);
			write(0x0e, step);
			expect(handle.codec()?.mic).toEqual({ raw: step, left: want, right: want, muted: false });
		}
		write(0x0e, 0x8008);
		expect(handle.codec()?.mic.muted).toBe(true);
		handle.detach();
	});

	test("the mute bit alone mutes, whatever the steps", () => {
		const { handle, boot, write } = card();
		boot();
		write(0x02, 0x0000);
		expect(handle.codec()?.master.muted).toBe(false);
		write(0x02, 0x8000);
		expect(handle.codec()?.master.muted).toBe(true);
		write(0x18, 0x0808);
		expect(handle.codec()?.pcm).toEqual({ raw: 0x0808, left: 74, right: 74, muted: false });
		write(0x18, 0x8808);
		expect(handle.codec()?.pcm.muted).toBe(true);
		handle.detach();
	});

	test("the record select holds two source numbers, and the rate follows variable rate audio", () => {
		const { handle, boot, write } = card();
		boot();
		write(0x1a, 0x0402);
		expect(handle.codec()?.recordSelect).toMatchObject({ raw: 0x0402, left: 4, right: 2, muted: false });
		expect(handle.codec()?.rate).toBe(48000);
		write(0x2a, 1);
		write(0x2c, 22050);
		expect(handle.codec()).toMatchObject({ rate: 22050, vra: true });
		handle.detach();
	});

	test("a codec reset brings the muted power-on state back", () => {
		const { handle, boot, write } = card();
		boot();
		write(0x02, 0x0000);
		write(0x00, 0);
		expect(handle.codec()?.master.muted).toBe(true);
		handle.detach();
	});

	test("reads the registers a saved desktop restored", () => {
		const first = fakeV86();
		const one = attachAc97(first.emulator, { context: null });
		first.emulator.boot();
		first.port(NAM + 0x02).w16?.(0x0a0a);
		const state = one.getAc97State();
		one.detach();
		const second = fakeV86();
		const two = attachAc97(second.emulator, { context: null, state });
		second.emulator.boot();
		expect(two.codec()?.master).toEqual({ raw: 0x0a0a, left: 68, right: 68, muted: false });
		two.detach();
	});
});

describe("onCodec", () => {
	test("reports a write after the 50 ms window, with the register and the word", async () => {
		const { handle, boot, write, calls } = card();
		boot();
		write(0x02, 0x0808);
		expect(calls).toEqual([]);
		await sleep(90);
		expect(calls).toEqual([[0x02, 0x0808]]);
		handle.detach();
	});

	test("a burst of writes to one register is one call with the last word; other registers keep their own", async () => {
		const { handle, boot, write, calls } = card();
		boot();
		for (let step = 0; step < 20; step++) write(0x02, step);
		write(0x18, 0x0808);
		write(0x1c, 0x0f0f);
		await sleep(90);
		expect(calls.filter(([register]) => register === 0x02)).toEqual([[0x02, 19]]);
		expect(calls).toContainEqual([0x18, 0x0808]);
		expect(calls).toContainEqual([0x1c, 0x0f0f]);
		expect(calls).toHaveLength(3);
		handle.detach();
	});

	test("one register is reported at most once per 50 ms, however long the guest keeps writing", async () => {
		const times: number[] = [];
		const v86 = fakeV86();
		const live = attachAc97(v86.emulator, { context: null, onCodec: () => times.push(performance.now()) });
		v86.emulator.boot();
		const until = performance.now() + 400;
		while (performance.now() < until) {
			v86.port(NAM + 0x02).w16?.(Math.floor(Math.random() * 32));
			await sleep(5);
		}
		await sleep(90);
		expect(times.length).toBeGreaterThanOrEqual(3);
		for (let i = 1; i < times.length; i++) expect((times[i] as number) - (times[i - 1] as number)).toBeGreaterThanOrEqual(45);
		live.detach();
	});

	test("covers the mixer registers and not the card's rate or other registers", async () => {
		const { handle, boot, write, calls } = card();
		boot();
		write(0x2a, 1);
		write(0x2c, 22050);
		write(0x20, 0x1234);
		await sleep(90);
		expect(calls).toEqual([]);
		write(0x1e, 0x8000);
		write(0x00, 0);
		await sleep(90);
		expect(calls.map(([register]) => register).sort()).toEqual([0x00, 0x1e]);
		handle.detach();
	});

	test("a byte write reports the whole word", async () => {
		const { handle, boot, v86, calls } = card();
		boot();
		v86.port(NAM + 0x03).w8?.(0x0a);
		await sleep(90);
		expect(calls).toEqual([[0x02, 0x0a00]]);
		handle.detach();
	});

	test("detach drops a write that is waiting, and later writes report nothing", async () => {
		const { handle, boot, write, calls } = card();
		boot();
		write(0x02, 0x0808);
		handle.detach();
		await sleep(90);
		write(0x02, 0x0000);
		await sleep(90);
		expect(calls).toEqual([]);
	});

	test("no timer runs while nothing is waiting", async () => {
		const real = globalThis.setTimeout;
		let started = 0;
		globalThis.setTimeout = ((...args: Parameters<typeof setTimeout>) => (started++, real(...args))) as typeof setTimeout;
		try {
			const { handle, boot, write } = card();
			boot();
			const before = started;
			await sleep(120);
			expect(started - before).toBe(1); // only the sleep itself
			write(0x02, 1);
			expect(started - before).toBe(2);
			await sleep(90);
			handle.detach();
		} finally {
			globalThis.setTimeout = real;
		}
	});
});
