// The spike page's script: boots v86 from the QAGRR parts, adds the AC'97 device, and exposes `window.spike` for the harness.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { V86 } from "../../v86/libv86.mjs";
import { Sink, SINK_RATE } from "./ac97-sink.js";
import { attachToCpu, installBeforeInit, attachOnLoaded } from "./ac97-v86.js";

const MB = 1024 * 1024;
const q = new URLSearchParams(location.search);
const status = document.getElementById("status");

const spike = {
	emulator: null,
	model: null,
	sink: null,
	pciId: null,
	trace: [],
	traceCount: {},
	traceOn: true,
	attachWay: null,
	pciDevices: null,
	events: [],
};
window.spike = spike;

function trace(kind, off, value) {
	if (!spike.traceOn) return;
	const k = kind + ":" + off.toString(16);
	spike.traceCount[k] = (spike.traceCount[k] || 0) + 1;
	if (spike.trace.length < 30000) spike.trace.push([Math.round(performance.now()), kind, off, value]);
}

async function image(url) {
	const manifest = await (await fetch(url.replace(/[^/]*$/, "manifest.json"))).json();
	return { url, async: true, size: manifest.size, fixed_chunk_size: manifest.chunk, use_parts: true };
}

// opts: ac97 (bool), attach ("hook" or "loaded"), state (url or ""), disk, ram, jit.
spike.start = async (opts = {}) => {
	const o = Object.assign({ ac97: true, attach: "hook", state: "", disk: "/images/parts/.img.zst", ram: 256, vga: 8, jit: true, audioState: null }, opts);
	const v86opts = {
		wasm_path: "../../v86/v86.wasm",
		memory_size: o.ram * MB,
		vga_memory_size: o.vga * MB,
		screen: { container: document.getElementById("screen") },
		bios: { url: "../../v86/bios/seabios.bin" },
		vga_bios: { url: "../../v86/bios/vgabios.bin" },
		acpi: true,
		disable_jit: !o.jit,
		boot_order: 0x132,
		autostart: o.attach === "hook",
		disable_speaker: true,
		disable_mouse: false,
		hda: await image(o.disk),
	};
	if (o.state) v86opts.initial_state = { url: o.state };
	const emulator = new V86(v86opts);
	spike.emulator = emulator;
	spike.startedAt = performance.now();
	spike.attachWay = o.ac97 ? o.attach : null;
	const onCpu = (cpu) => {
		spike.pciDevicesBefore = Object.keys(cpu.devices.pci.devices).filter((k) => cpu.devices.pci.devices[k]).map((k) => [Number(k), cpu.devices.pci.devices[k].name]);
		if (o.ac97) {
			const a = attachToCpu(cpu, { trace });
			spike.model = a.model;
			spike.pciId = a.pciId;
			spike.pci = a.pci;
			spike.cpu = cpu;
		}
		spike.pciDevices = Object.keys(cpu.devices.pci.devices).filter((k) => cpu.devices.pci.devices[k]).map((k) => [Number(k), cpu.devices.pci.devices[k].name]);
		spike.cpu = cpu;
	};
	if (o.attach === "hook") installBeforeInit(emulator, onCpu);
	else attachOnLoaded(emulator, onCpu);
	spike.ideBytes = 0;
	emulator.add_listener("ide-read-end", (a) => {
		spike.ideBytes += a[1];
	});
	emulator.add_listener("emulator-loaded", () => {
		spike.events.push(["emulator-loaded", Math.round(performance.now() - spike.startedAt)]);
		// The device's own registers ride beside the state (v86's state carries the PCI space but knows nothing of the page's device).
		if (o.audioState && spike.model) spike.model.setState(o.audioState);
		if (o.attach !== "hook") emulator.run();
	});
	status.textContent = "starting";
};

// The audio side: create the context and let the sink pull from the device.
spike.startAudio = async () => {
	const sink = new Sink();
	spike.sink = sink;
	sink.onNeed = (n) => {
		const l = new Float32Array(n);
		const r = new Float32Array(n);
		if (spike.model) spike.model.render(n, SINK_RATE, l, r);
		return { l, r };
	};
	const state = await sink.start();
	status.textContent = "audio " + state;
	return state;
};

spike.key = (codes) => {
	spike.emulator.keyboard_send_scancodes(codes);
	spike.emulator.keyboard_send_scancodes(codes.map((c) => c | 0x80));
};
spike.chord = (down) => {
	for (const c of down) spike.emulator.keyboard_send_scancodes(c);
	for (const c of [...down].reverse()) spike.emulator.keyboard_send_scancodes(c.map((v) => v | 0x80));
};
spike.text = (s) => spike.emulator.keyboard_send_text(s);
spike.canvas = () => document.querySelector("#screen canvas");
spike.instr = () => spike.emulator.get_instruction_counter();
spike.modelState = () => (spike.model ? spike.model.getState() : null);
spike.counters = () => (spike.model ? { ...spike.model.counters } : null);

// Save the machine and the device's registers together; the PCI space rides inside v86's own state.
spike.save = async () => {
	await spike.emulator.stop();
	const state = await spike.emulator.save_state();
	return { bytes: state.byteLength, audio: spike.modelState(), state };
};
