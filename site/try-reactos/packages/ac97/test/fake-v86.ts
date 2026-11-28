// A v86 look-alike for the card's tests: a cpu with a PCI bus, an I/O port table and 1 MiB of memory, and a machine whose `init` builds it.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { V86Cpu, V86Emulator, V86Machine } from "../src/index.ts";

export type PortFns = { r8: () => number; r16: () => number; r32: () => number; w8: (v: number) => void; w16: (v: number) => void; w32: (v: number) => void };

/** A v86 look-alike: a cpu with a PCI bus, an I/O port table and 1 MiB of memory, and a machine whose `init` builds it. */
export function fakeV86(taken: number[] = []) {
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
		write_blob: (blob, address) => memory.set(blob, address),
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
