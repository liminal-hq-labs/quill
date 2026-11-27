// Registers the AC'97 model with a v86 machine from the page, through v86's internal API: its PCI bus, I/O ports, interrupt lines and guest
// memory. v86 itself is the unchanged npm build, pinned (site/try-reactos/v86/PROVENANCE.md), so the shapes below are those of 0.5.473.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { Ac97, NABM_SIZE, NAM_SIZE, type Ac97Options } from "./model.ts";

/** The v86 version whose internals this file was written against and tested with. */
export const V86_VERSION = "0.5.473";

const NAM_BASE = 0xd000;
const NABM_BASE = 0xd400;

/** The parts of v86's PCI bus the device uses. */
export interface V86Pci {
	devices: Record<number, unknown>;
	register_device(device: { pci_id: number; pci_space: number[]; pci_bars: { size: number }[]; name: string }): void;
	raise_irq(id: number): void;
	lower_irq(id: number): void;
}

type PortRead = () => number;
type PortWrite = (value: number) => void;

/** The parts of v86's I/O port table the device uses. */
export interface V86Io {
	register_read(port: number, owner: object, read8: PortRead, read16: PortRead, read32: PortRead): void;
	register_write(port: number, owner: object, write8: PortWrite, write16: PortWrite, write32: PortWrite): void;
}

/** The parts of v86's CPU the device uses. */
export interface V86Cpu {
	devices: { pci: V86Pci };
	io: V86Io;
	read_blob(address: number, length: number): Uint8Array;
}

/** The machine object v86 builds after its constructor returns (`emulator.v86`). */
export interface V86Machine {
	cpu: V86Cpu;
	init: (...args: unknown[]) => unknown;
}

/** The part of v86's `V86` object the device needs. */
export interface V86Emulator {
	v86?: V86Machine | undefined;
}

/** 64 bytes of PCI configuration space: 8086:2415 (82801AA AC'97 audio), class 04:01:00, two I/O BARs, interrupt pin A. */
export function pciSpace(): number[] {
	const space = new Uint8Array(64);
	const put16 = (offset: number, value: number): void => {
		space[offset] = value & 255;
		space[offset + 1] = value >> 8;
	};
	put16(0x00, 0x8086);
	put16(0x02, 0x2415);
	put16(0x04, 0x0003);
	put16(0x06, 0x0280);
	space[0x08] = 0x02; // revision
	space[0x0a] = 0x01; // sub-class: audio
	space[0x0b] = 0x04; // class: multimedia
	put16(0x10, NAM_BASE | 1);
	put16(0x14, NABM_BASE | 1);
	put16(0x2c, 0x8086); // subsystem vendor and id
	put16(0x2e, 0x0000);
	space[0x3d] = 1; // interrupt pin A
	space[0x3e] = 0x02;
	space[0x3f] = 0x18;
	return Array.from(space);
}

export interface Registered {
	model: Ac97;
	/** The PCI function id (slot << 3). */
	pciId: number;
	/** Makes the device stop answering and drop its interrupt line (v86 cannot unregister a port or a PCI function). */
	retire(): void;
}

/** Registers a new device on the machine's PCI bus and I/O ports. Must run before the BIOS enumerates PCI, and before a state is restored. */
export function registerOn(cpu: V86Cpu, trace: Ac97Options["trace"] = null): Registered {
	const pci = cpu.devices.pci;
	let slot = 3;
	while (slot < 32 && pci.devices[slot << 3]) slot++;
	if (slot >= 32) throw new Error("no free PCI slot for the AC'97 device");
	const pciId = slot << 3;
	let live = true;
	const model = new Ac97({
		readMem: (address, length) => cpu.read_blob(address, length),
		raiseIrq: () => live && pci.raise_irq(pciId),
		lowerIrq: () => pci.lower_irq(pciId),
		trace,
	});
	// The handlers are registered at the BARs' power-on addresses, before the PCI function: v86 copies the port entries when the function is
	// registered and moves them when the BIOS or the guest moves a BAR.
	const wire = (base: number, size: number, read: (offset: number, width: 1 | 2 | 4) => number, write: (offset: number, width: 1 | 2 | 4, value: number) => void): void => {
		for (let offset = 0; offset < size; offset++) {
			cpu.io.register_read(
				base + offset,
				model,
				() => (live ? read(offset, 1) : 0xff),
				() => (live ? read(offset, 2) : 0xffff),
				() => (live ? read(offset, 4) : 0xffffffff),
			);
			cpu.io.register_write(
				base + offset,
				model,
				(value) => live && write(offset, 1, value),
				(value) => live && write(offset, 2, value),
				(value) => live && write(offset, 4, value),
			);
		}
	};
	wire(NAM_BASE, NAM_SIZE, (o, w) => model.namRead(o, w), (o, w, v) => model.namWrite(o, w, v));
	wire(NABM_BASE, NABM_SIZE, (o, w) => model.nabmRead(o, w), (o, w, v) => model.nabmWrite(o, w, v));
	pci.register_device({ pci_id: pciId, pci_space: pciSpace(), pci_bars: [{ size: NAM_SIZE }, { size: NABM_SIZE }], name: "AC97" });
	return {
		model,
		pciId,
		retire() {
			model.releaseIrq();
			live = false;
		},
	};
}

/**
 * Calls `onCpu(cpu)` once, right after v86 creates its devices and before it restores a saved state or the BIOS runs. That is the only
 * moment that serves both a cold boot and a state: v86 restores the PCI space before `emulator-loaded`, so a function registered on that
 * event would be too late. Works by wrapping `emulator.v86` (a property v86 sets after its constructor returns) and then its `init`.
 *
 * Returns a function that undoes the wrapping, if it has not run yet.
 */
export function onceBeforeInit(emulator: V86Emulator, onCpu: (cpu: V86Cpu) => void): () => void {
	const existing = emulator.v86;
	if (existing && existing.cpu && existing.cpu.devices && existing.cpu.devices.pci) {
		// Already built: the caller attached after init; there is nothing to wait for.
		onCpu(existing.cpu);
		return () => {};
	}
	let inner: V86Machine | undefined = existing;
	let undoInit: (() => void) | null = null;
	let done = false;
	const wrapInit = (machine: V86Machine): void => {
		const original = machine.init;
		undoInit = () => {
			machine.init = original;
			undoInit = null;
		};
		machine.init = (...args: unknown[]): unknown => {
			const result = original.apply(machine, args);
			if (undoInit) undoInit();
			if (!done) {
				done = true;
				onCpu(machine.cpu);
			}
			return result;
		};
	};
	Object.defineProperty(emulator, "v86", {
		configurable: true,
		enumerable: true,
		get: () => inner,
		set(value: V86Machine | undefined) {
			inner = value;
			if (value && !done) wrapInit(value);
		},
	});
	if (inner) wrapInit(inner);
	return () => {
		done = true;
		if (undoInit) undoInit();
		Object.defineProperty(emulator, "v86", { configurable: true, enumerable: true, writable: true, value: inner });
	};
}
