// The v86 glue and the page API on a fake machine: registering before init, the ports, saved registers, volume and mute, the real-time drain and detach.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { attachAc97, gainFor, onceBeforeInit, pciSpace, registerOn, V86_VERSION, type V86Cpu, type V86Emulator, type V86Machine } from "../src/index.ts";
import { CR, GS } from "../src/model.ts";

type PortFns = { r8: () => number; r16: () => number; r32: () => number; w8: (v: number) => void; w16: (v: number) => void; w32: (v: number) => void };

/** A v86 look-alike: a cpu with a PCI bus, an I/O port table and 1 MiB of memory, and a machine whose `init` builds it. */
function fakeV86(taken: number[] = []) {
	const memory = new Uint8Array(1 << 20);
	const ports = new Map<number, Partial<PortFns>>();
	const irq: string[] = [];
	const devices: Record<number, unknown> = {};
	for (const slot of taken) devices[slot << 3] = { name: "other" };
	const registrations: { pci_id: number; pci_space: number[]; name: string }[] = [];
	const cpu: V86Cpu = {
		devices: {
			pci: {
				devices,
				register_device(device) {
					devices[device.pci_id] = device;
					registrations.push(device);
				},
				raise_irq: (id) => irq.push("raise " + id),
				lower_irq: (id) => irq.push("lower " + id),
			},
		},
		io: {
			register_read(port, _owner, r8, r16, r32) {
				ports.set(port, { ...ports.get(port), r8, r16, r32 });
			},
			register_write(port, _owner, w8, w16, w32) {
				ports.set(port, { ...ports.get(port), w8, w16, w32 });
			},
		},
		read_blob: (address, length) => memory.subarray(address, address + length),
	};
	const calls = { init: 0 };
	class Machine implements V86Machine {
		cpu: V86Cpu = cpu;
		init(): string {
			calls.init++;
			return "initialised";
		}
	}
	class Emulator implements V86Emulator {
		v86?: V86Machine | undefined;
		/** What v86's constructor does a moment after it returns: build the machine, then run its init. */
		boot(): unknown {
			this.v86 = new Machine();
			return this.v86.init();
		}
	}
	const port = (n: number) => ports.get(n) as PortFns;
	return { emulator: new Emulator(), cpu, memory, ports, port, irq, registrations, calls, devices };
}

/** A descriptor list of `count` buffers of `frames` frames of a constant sample (all the same value, left and right). */
function playFrom(v86: ReturnType<typeof fakeV86>, count: number, frames: number, value: number): void {
	const dv = new DataView(v86.memory.buffer);
	for (let i = 0; i < count; i++) {
		const address = 0x20000 + i * 0x8000;
		dv.setUint32(0x10000 + i * 8, address, true);
		dv.setUint32(0x10000 + i * 8 + 4, (frames * 2) | 0x80000000, true);
		for (let f = 0; f < frames * 2; f++) dv.setInt16(address + f * 2, value, true);
	}
	const nabm = 0xd400;
	v86.port(nabm + 0x10).w32?.(0x10000);
	v86.port(nabm + 0x15).w8?.(count - 1);
	v86.port(nabm + 0x1b).w8?.(CR.RPBM | CR.IOCE);
}

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

describe("the pinned v86", () => {
	test("the version the glue was written against is the one the page fetches", () => {
		const script = readFileSync(new URL("../../../tools/fetch-v86.sh", import.meta.url), "utf8");
		expect(/^version="([0-9.]+)"/m.exec(script)?.[1]).toBe(V86_VERSION);
	});
});

describe("registering with v86", () => {
	test("the callback runs once, after init, and before anything else touches the PCI bus", () => {
		const v86 = fakeV86();
		let order = "";
		onceBeforeInit(v86.emulator, (cpu) => {
			order += "callback ";
			expect(cpu).toBe(v86.cpu);
			expect(v86.calls.init).toBe(1);
		});
		expect(v86.emulator.v86).toBeUndefined();
		expect(v86.emulator.boot()).toBe("initialised"); // init's own result is passed on
		order += "booted";
		expect(order).toBe("callback booted");
		expect(v86.emulator.v86?.cpu).toBe(v86.cpu);
	});

	test("registers the function in the first free slot, with the ICH's ids and both BARs", () => {
		const v86 = fakeV86([3, 4]);
		const handle = attachAc97(v86.emulator, { context: null });
		v86.emulator.boot();
		expect(v86.registrations).toHaveLength(1);
		const [device] = v86.registrations;
		expect(device?.pci_id).toBe(5 << 3);
		expect(device?.name).toBe("AC97");
		expect(device?.pci_space).toEqual(pciSpace());
		const space = Uint8Array.from(pciSpace());
		expect(space[0]! | (space[1]! << 8)).toBe(0x8086);
		expect(space[2]! | (space[3]! << 8)).toBe(0x2415);
		expect(space[0x0b]).toBe(0x04);
		expect(space[0x0a]).toBe(0x01);
		expect(handle.stats().pciId).toBe(5 << 3);
		handle.detach();
	});

	test("the handlers sit at the BARs' power-on addresses and reach the model", () => {
		const v86 = fakeV86();
		const handle = attachAc97(v86.emulator, { context: null });
		v86.emulator.boot();
		expect(v86.port(0xd000 + 0x28).r16()).toBe(1); // extended audio ID: variable rate
		expect(v86.port(0xd400 + 0x30).r32() & GS.CODEC_READY).toBe(GS.CODEC_READY);
		v86.port(0xd000 + 0x02).w16?.(0x0000);
		expect(v86.port(0xd000 + 0x02).r16()).toBe(0);
		expect(v86.ports.has(0xd000 + 255)).toBe(true);
		expect(v86.ports.has(0xd000 + 256)).toBe(false);
		expect(v86.ports.has(0xd400 + 63)).toBe(true);
		handle.detach();
	});

	test("with no free slot it refuses rather than overwrite a device", () => {
		const v86 = fakeV86(Array.from({ length: 29 }, (_, i) => i + 3));
		expect(() => registerOn(v86.cpu)).toThrow(/no free PCI slot/);
	});

	test("an undone wrap leaves v86 as it was", () => {
		const v86 = fakeV86();
		const undo = onceBeforeInit(v86.emulator, () => {
			throw new Error("must not run");
		});
		undo();
		v86.emulator.boot();
		expect(v86.calls.init).toBe(1);
		expect(v86.emulator.v86).toBeDefined();
	});

	test("a machine that is already built is registered at once", () => {
		const v86 = fakeV86();
		v86.emulator.boot();
		let got: V86Cpu | null = null;
		onceBeforeInit(v86.emulator, (cpu) => (got = cpu));
		expect(got as unknown).toBe(v86.cpu);
	});
});

describe("the page API", () => {
	test("the registers are null before the card exists, then follow the guest", () => {
		const v86 = fakeV86();
		const handle = attachAc97(v86.emulator, { context: null });
		expect(handle.getAc97State()).toBeNull();
		expect(handle.stats().registered).toBe(false);
		v86.emulator.boot();
		expect(handle.stats().registered).toBe(true);
		v86.port(0xd000 + 0x18).w16?.(0x0a0a);
		expect(handle.getAc97State()?.nam[0x18 >> 1]).toBe(0x0a0a);
		handle.detach();
	});

	test("saved registers are restored as the card is registered, before the guest runs", () => {
		const first = fakeV86();
		const one = attachAc97(first.emulator, { context: null });
		first.emulator.boot();
		first.port(0xd000 + 0x18).w16?.(0x0b0b);
		first.port(0xd000 + 0x2a).w16?.(1);
		first.port(0xd000 + 0x2c).w16?.(22050);
		const saved = JSON.parse(JSON.stringify(one.getAc97State()));
		one.detach();

		const second = fakeV86();
		const two = attachAc97(second.emulator, { context: null, state: saved });
		second.emulator.boot();
		expect(second.port(0xd000 + 0x18).r16()).toBe(0x0b0b);
		expect(two.stats().guestRate).toBe(22050);
		expect(two.stats().stateError).toBe("");
		two.detach();
	});

	test("a refused state is reported and the card still registers with its defaults", () => {
		const v86 = fakeV86();
		const handle = attachAc97(v86.emulator, { context: null, state: { version: 9 } as never });
		v86.emulator.boot();
		expect(handle.stats().registered).toBe(true);
		expect(handle.stats().stateError).toMatch(/not an AC'97 state/);
		expect(v86.port(0xd000 + 0x02).r16()).toBe(0x8000);
		handle.detach();
	});

	test("volume is clamped, with a squared taper, and mute is separate from it", () => {
		const v86 = fakeV86();
		const handle = attachAc97(v86.emulator, { context: null });
		expect(handle.getVolume()).toBe(0.7);
		handle.setVolume(2);
		expect(handle.getVolume()).toBe(1);
		handle.setVolume(-1);
		expect(handle.getVolume()).toBe(0);
		handle.setVolume(Number.NaN);
		expect(handle.getVolume()).toBe(0);
		handle.setVolume(0.5);
		handle.setMuted(true);
		expect(handle.isMuted()).toBe(true);
		expect(handle.getVolume()).toBe(0.5);
		expect(handle.stats().gain).toBe(0);
		handle.setMuted(false);
		expect(handle.stats().gain).toBe(0.25);
		expect(gainFor(0.7, false)).toBeCloseTo(0.49, 10);
		expect(gainFor(1, true)).toBe(0);
		handle.detach();
	});

	test("with no output the guest's buffers are still taken at the real-time rate, so it never waits", async () => {
		const v86 = fakeV86();
		const handle = attachAc97(v86.emulator, { context: null });
		v86.emulator.boot();
		// Two buffers of 4,800 frames: 200 ms at 48 kHz.
		playFrom(v86, 2, 4800, 8000);
		expect(handle.stats().playing).toBe(true);
		// The guest's interrupt handler, a little late each time: the card holds each buffer's interrupt until it has been taken.
		const guest = setInterval(() => v86.port(0xd400 + 0x16).w16?.(0x1c), 5);
		await sleep(100);
		const half = handle.stats().drainedFrames;
		expect(half).toBeGreaterThan(1500); // some of it has gone, but not all at once
		expect(half).toBeLessThan(8000);
		await sleep(300);
		clearInterval(guest);
		const done = handle.stats();
		expect(done.playing).toBe(false);
		expect(done.counters?.dmaFrames).toBe(9600);
		expect(done.counters?.iocs).toBe(2);
		expect(v86.irq).toContain("raise 24");
		handle.detach();
	});

	test("detach stops answering, drops the interrupt line and is safe twice", () => {
		const v86 = fakeV86();
		const handle = attachAc97(v86.emulator, { context: null });
		v86.emulator.boot();
		playFrom(v86, 1, 100, 1000);
		handle.model?.readFrames(100, new Float32Array(100), new Float32Array(100)); // finishes the buffer: the interrupt is up
		expect(v86.irq.at(-1)).toBe("raise 24");
		handle.detach();
		expect(v86.irq.at(-1)).toBe("lower 24");
		expect(v86.port(0xd000 + 0x28).r16()).toBe(0xffff);
		expect(v86.port(0xd400 + 0x1b).r8()).toBe(0xff);
		v86.port(0xd000 + 0x02).w16?.(0x1234); // ignored
		expect(handle.getAc97State()?.nam[1]).toBe(0x8000); // the write was ignored
		handle.detach();
	});

	test("detach before the machine is built means it never registers", () => {
		const v86 = fakeV86();
		const handle = attachAc97(v86.emulator, { context: null });
		handle.detach();
		v86.emulator.boot();
		expect(v86.registrations).toHaveLength(0);
	});

	test("a second machine can be attached after the first is detached", () => {
		for (let round = 0; round < 3; round++) {
			const v86 = fakeV86();
			const handle = attachAc97(v86.emulator, { context: null });
			v86.emulator.boot();
			expect(v86.registrations).toHaveLength(1);
			handle.detach();
		}
	});
});
