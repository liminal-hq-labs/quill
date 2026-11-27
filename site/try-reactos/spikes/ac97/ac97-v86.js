// Attach the AC'97 model to a v86 machine from the page, through v86's internal API (PCI, I/O ports, interrupts, guest memory).
// v86 itself is the unchanged npm build. Two ways in: `installBeforeInit` catches the machine as its devices are created and before a saved
// state is restored (the way that works for states), and `attachNow` registers on `emulator-loaded` (autostart: false), before the BIOS runs.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { Ac97, NAM_SIZE, NABM_SIZE } from "./ac97-model.js";

const NAM_BASE = 0xd000;
const NABM_BASE = 0xd400;

// 64 bytes of PCI configuration space: 8086:2415 (82801AA AC'97 audio), class 04:01:00, two I/O BARs, interrupt pin A.
export function pciSpace() {
	const s = new Uint8Array(64);
	const put16 = (o, v) => {
		s[o] = v & 255;
		s[o + 1] = v >> 8;
	};
	put16(0x00, 0x8086);
	put16(0x02, 0x2415);
	put16(0x04, 0x0003);
	put16(0x06, 0x0280);
	s[0x08] = 0x02; // revision
	s[0x0a] = 0x01; // sub-class: audio
	s[0x0b] = 0x04; // class: multimedia
	put16(0x10, NAM_BASE | 1);
	put16(0x14, NABM_BASE | 1);
	put16(0x2c, 0x8086); // subsystem vendor and id
	put16(0x2e, 0x0000);
	s[0x3d] = 1; // interrupt pin A
	s[0x3e] = 0x02;
	s[0x3f] = 0x18;
	return Array.from(s);
}

export function attachToCpu(cpu, { trace = null, slot = null } = {}) {
	const pci = cpu.devices.pci;
	const io = cpu.io;
	let id = slot === null ? null : slot << 3;
	if (id === null) {
		for (let s = 3; s < 32; s++) {
			if (!pci.devices[s << 3]) {
				id = s << 3;
				break;
			}
		}
	}
	const model = new Ac97({
		readMem: (a, n) => cpu.read_blob(a, n),
		raiseIrq: () => pci.raise_irq(id),
		lowerIrq: () => pci.lower_irq(id),
		trace,
	});
	// The handlers are registered at the BARs' power-on addresses, before the PCI function: v86 copies the port entries when the function is registered and
	// moves them when the BIOS or the guest moves a BAR.
	const wire = (base, size, read, write) => {
		for (let i = 0; i < size; i++) {
			const off = i;
			io.register_read(base + i, model, () => read(off, 1), () => read(off, 2), () => read(off, 4));
			io.register_write(base + i, model, (v) => write(off, 1, v), (v) => write(off, 2, v), (v) => write(off, 4, v));
		}
	};
	wire(NAM_BASE, NAM_SIZE, (o, n) => model.namRead(o, n), (o, n, v) => model.namWrite(o, n, v));
	wire(NABM_BASE, NABM_SIZE, (o, n) => model.nabmRead(o, n), (o, n, v) => model.nabmWrite(o, n, v));
	pci.register_device({ pci_id: id, pci_space: pciSpace(), pci_bars: [{ size: NAM_SIZE }, { size: NABM_SIZE }], name: "AC97" });
	return { model, pciId: id, pci };
}

// Wrap the machine so `onCpu(cpu)` runs right after v86 creates its devices and before it restores a state or the BIOS runs.
export function installBeforeInit(emulator, onCpu) {
	let inner;
	Object.defineProperty(emulator, "v86", {
		configurable: true,
		get: () => inner,
		set(value) {
			inner = value;
			if (!value) return;
			const original = value.init.bind(value);
			value.init = (...args) => {
				const result = original(...args);
				onCpu(value.cpu);
				return result;
			};
		},
	});
}

// Register on `emulator-loaded`: with `autostart: false` the BIOS has not run yet.
export function attachOnLoaded(emulator, onCpu) {
	emulator.add_listener("emulator-loaded", () => onCpu(emulator.v86.cpu));
}
