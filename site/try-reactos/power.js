// Tells the page when the guest has powered itself off, which v86 does not report (SITE-40). The details are below the licence header.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

// v86 has no power-off of its own: it emits `emulator-stopped` only when
// something calls stop(), and a guest that finishes Shut Down just goes on running, spinning in its ACPI loop at a fixed address with
// the screen stuck on the last frame. What the guest does do is write the sleep-enable bit of the ACPI PM1a control register (the
// port at PORT_ACPI_PM_BASE + 4, 0xB004). ReactOS writes it as 0x2001 and then 0x2000 and polls for the wake bit forever, so this
// module wraps that port's write handlers and reports the first write that has the bit set (SITE-40).
//
// It also reports `emulator-stopped`, so anything that stops the machine (a script, a future feature) is seen the same way.
// The port wrapper reaches into v86's cpu (`emulator.v86.cpu.io.ports`); the page pins one v86 build through the manifest, and when
// the ports are not there the wrapper is skipped and only `emulator-stopped` is reported.
//

const PM1A_CONTROL = 0xb004;
const SLP_EN = 0x2000;

// Calls onPowerOff(reason) once, at most, with "acpi" (the guest asked for power-off or sleep) or "stopped" (the emulator stopped).
// Returns a function that restores the port handlers and silences the callbacks.
export function watch(emulator, onPowerOff) {
	let live = true;
	let told = false;
	const restore = [];
	const tell = (reason) => {
		if (!live || told) return;
		told = true;
		// Out of the emulator's own loop, which is the caller of a port write.
		setTimeout(() => live && onPowerOff(reason), 0);
	};
	// v86 also emits emulator-stopped while it restores a saved state, before the machine has run, so a stop counts only after a start.
	let ran = false;
	emulator.add_listener("emulator-started", () => (ran = true));
	emulator.add_listener("emulator-stopped", () => {
		if (ran) tell("stopped");
	});
	// v86 builds its cpu after the constructor returns, so the port handlers are wrapped when the machine starts running.
	let wrapped = false;
	const wrapPorts = () => {
		if (wrapped || !live) return;
		try {
			const ports = emulator.v86.cpu.io.ports;
			// A 16- or 32-bit write to 0xB004 carries the bit in the value; a byte write to 0xB005 is the high byte of the same register.
			const wrap = (port, width, bit) => {
				const slot = ports[port];
				const original = slot && slot[width];
				if (typeof original !== "function") return;
				slot[width] = function (value) {
					if (value & bit) tell("acpi");
					return original.apply(this, arguments);
				};
				restore.push(() => (slot[width] = original));
			};
			wrap(PM1A_CONTROL, "write16", SLP_EN);
			wrap(PM1A_CONTROL, "write32", SLP_EN);
			wrap(PM1A_CONTROL + 1, "write8", SLP_EN >> 8);
			wrapped = true;
		} catch (e) {
			// Another build of v86: the stop event alone still works.
		}
	};
	emulator.add_listener("emulator-started", wrapPorts);
	wrapPorts();
	return () => {
		live = false;
		for (const undo of restore) undo();
	};
}
