// The ReactOS experiment page: starts v86 with the machine the builder panel describes, when asked and not before. Exposes
// window.tryReactos for the measurement harness; the page's own API is the only way input reaches the guest.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

const player = document.querySelector("[data-player]");
const startButton = player.querySelector("[data-start]");
const stopButton = player.querySelector("[data-stop]");
const fullButton = player.querySelector("[data-fullscreen]");
const statusLine = player.querySelector("[data-status]");
const stage = player.querySelector("[data-stage]");
const screen = player.querySelector("[data-screen]");
const form = document.querySelector("[data-config]");
const readout = document.querySelector("[data-readout]");

const BOOT = { auto: 0, cd: 0x213, hd: 0x132 };
const MB = 1024 * 1024;

let emulator = null;
let startedAt = 0;
let tick = null;

const stats = {
	startedAt: 0,
	emulatorStartedMs: null,
	ideReads: 0,
	ideReadBytes: 0,
	firstIdeReadMs: null,
	lastIdeReadMs: null,
	screenSizes: [],
	mouseEnabled: false,
	vmwareAbsolute: false,
	downloads: {},
	mips: 0,
};

function say(text) {
	statusLine.textContent = text;
}

function canRun() {
	try {
		return typeof WebAssembly === "object" && typeof WebAssembly.instantiate === "function";
	} catch (e) {
		return false;
	}
}

// Fill the panel from the query string, so a run can be named by its URL.
function readQuery() {
	const q = new URLSearchParams(location.search);
	for (const el of form.elements) {
		if (!el.name || !q.has(el.name)) continue;
		if (el.type === "checkbox") el.checked = q.get(el.name) !== "0";
		else el.value = q.get(el.name);
	}
	if (!form.elements.run.value) form.elements.run.value = "r" + Math.random().toString(36).slice(2, 8);
	// The server reads the run id from the Referer, so it belongs in the page's own address.
	const url = new URL(location.href);
	url.searchParams.set("run", form.elements.run.value);
	history.replaceState(null, "", url);
}

function config(overrides) {
	const c = {
		ram: Number(form.elements.ram.value),
		vga: Number(form.elements.vga.value),
		acpi: form.elements.acpi.checked,
		jit: form.elements.jit.checked,
		cpuid: form.elements.cpuid.value.trim(),
		boot: form.elements.boot.value,
		disk: form.elements.disk.value.trim(),
		cdrom: form.elements.cdrom.value.trim(),
		state: form.elements.state.value.trim(),
		chunk: Number(form.elements.chunk.value) || 0,
	};
	return Object.assign(c, overrides || {});
}

async function sizeOf(url) {
	const r = await fetch(url, { method: "HEAD" });
	if (!r.ok) throw new Error(url + ": " + r.status);
	return Number(r.headers.get("content-length"));
}

// The whole file read with range requests, or 1 MiB (or other) zstd parts named DIR/START-END.img.zst, as in v86's own ReactOS demo.
// For parts the URL is DIR/.img.zst and DIR/manifest.json holds the image size.
async function asyncImage(url, chunk) {
	if (!chunk) return { url, async: true, size: await sizeOf(url) };
	const manifest = await (await fetch(url.replace(/[^/]*$/, "manifest.json"))).json();
	return { url, async: true, size: manifest.size, fixed_chunk_size: chunk, use_parts: true };
}

function render() {
	const now = performance.now();
	const rows = {
		state: emulator ? (emulator.is_running() ? "running" : "stopped") : "off",
		"seconds since start": startedAt ? ((now - startedAt) / 1000).toFixed(1) : "-",
		MIPS: stats.mips.toFixed(1),
		"IDE reads": stats.ideReads + " (" + (stats.ideReadBytes / MB).toFixed(1) + " MB)",
		"first IDE read": stats.firstIdeReadMs === null ? "-" : (stats.firstIdeReadMs / 1000).toFixed(2) + " s",
		"video mode": stats.screenSizes.length ? stats.screenSizes.map((s) => s.w + "x" + s.h + "x" + s.bpp).slice(-4).join(", ") : "-",
		"PS/2 mouse": String(stats.mouseEnabled),
		"VMware pointer": String(stats.vmwareAbsolute),
	};
	readout.replaceChildren();
	for (const [k, v] of Object.entries(rows)) {
		const dt = document.createElement("dt");
		dt.textContent = k;
		const dd = document.createElement("dd");
		dd.textContent = v;
		readout.append(dt, dd);
	}
}

let lastInstr = 0;
let lastAt = 0;
function sample() {
	if (emulator && emulator.is_running()) {
		const now = performance.now();
		const n = emulator.get_instruction_counter();
		if (lastAt) stats.mips = (n - lastInstr) / ((now - lastAt) / 1000) / 1e6;
		lastInstr = n;
		lastAt = now;
	}
	render();
}

function hook(e) {
	e.add_listener("emulator-started", () => {
		stats.emulatorStartedMs = performance.now() - startedAt;
	});
	e.add_listener("ide-read-end", (a) => {
		const t = performance.now() - startedAt;
		stats.ideReads++;
		stats.ideReadBytes += a[1];
		if (stats.firstIdeReadMs === null) stats.firstIdeReadMs = t;
		stats.lastIdeReadMs = t;
	});
	e.add_listener("screen-set-size", (a) => {
		stats.screenSizes.push({ t: performance.now() - startedAt, w: a[0], h: a[1], bpp: a[2] });
	});
	e.add_listener("mouse-enable", (v) => {
		stats.mouseEnabled = v;
	});
	e.add_listener("vmware-absolute-mouse", (v) => {
		stats.vmwareAbsolute = v;
	});
	e.add_listener("download-progress", (d) => {
		stats.downloads[d.file_name] = { loaded: d.loaded, total: d.total };
	});
}

async function start(overrides) {
	const c = config(overrides);
	startButton.disabled = true;
	say("Loading the emulator…");
	try {
		const { V86 } = await import("./v86/libv86.mjs?run=" + encodeURIComponent(form.elements.run.value));
		const opts = {
			wasm_path: "v86/v86.wasm",
			memory_size: c.ram * MB,
			vga_memory_size: c.vga * MB,
			screen: { container: screen },
			bios: { url: "v86/bios/seabios.bin" },
			vga_bios: { url: "v86/bios/vgabios.bin" },
			acpi: c.acpi,
			disable_jit: !c.jit,
			boot_order: BOOT[c.boot] || 0,
			autostart: true,
			disable_speaker: true,
		};
		if (c.cpuid) opts.cpuid_level = Number(c.cpuid);
		if (c.disk) opts.hda = await asyncImage(c.disk, c.chunk);
		if (c.cdrom) opts.cdrom = await asyncImage(c.cdrom);
		if (c.state) opts.initial_state = { url: c.state };
		startedAt = performance.now();
		stats.startedAt = startedAt;
		say("Starting…");
		emulator = new V86(opts);
		hook(emulator);
		window.tryReactos.emulator = emulator;
		window.tryReactos.config = c;
		tick = setInterval(sample, 1000);
		stopButton.disabled = false;
		fullButton.disabled = false;
		say("Running. Select the screen to give it the keyboard.");
	} catch (error) {
		startButton.disabled = false;
		say("Could not start: " + error.message);
		throw error;
	}
}

async function stop() {
	if (!emulator) return;
	clearInterval(tick);
	try {
		await emulator.destroy();
	} catch (e) {
		// Already gone.
	}
	emulator = null;
	window.tryReactos.emulator = null;
	startButton.disabled = false;
	stopButton.disabled = true;
	fullButton.disabled = true;
	say("Stopped. Writes to the guest disk are gone.");
	render();
}

function download(name, blob) {
	const a = document.createElement("a");
	a.href = URL.createObjectURL(blob);
	a.download = name;
	a.click();
	setTimeout(() => URL.revokeObjectURL(a.href), 5000);
}

// Typing into the guest, mouse moves and clicks: all through the emulator's own API.
const input = {
	key(codes) {
		emulator.keyboard_send_scancodes(codes);
	},
	text(s) {
		emulator.keyboard_send_text(s);
	},
	mouseDelta(dx, dy) {
		emulator.bus.send("mouse-delta", [dx, -dy]);
	},
	mouseClick(left, middle, right) {
		emulator.bus.send("mouse-click", [left, middle, right]);
	},
};

window.tryReactos = {
	stateName: new URLSearchParams(location.search).get("statename") || "state",
	emulator: null,
	config: null,
	stats,
	start,
	stop,
	input,
	canvas: () => screen.querySelector("canvas"),
	instructions: () => (emulator ? emulator.get_instruction_counter() : 0),
	elapsedMs: () => (startedAt ? performance.now() - startedAt : 0),
};

if (!canRun()) {
	document.getElementById("nowasm").hidden = false;
	startButton.disabled = true;
	say("This browser can't run the emulator.");
} else {
	readQuery();
	startButton.addEventListener("click", () => start().catch(() => {}));
	stopButton.addEventListener("click", stop);
	fullButton.addEventListener("click", () => {
		if (stage.requestFullscreen) stage.requestFullscreen();
	});
	stage.addEventListener("click", () => {
		if (emulator) emulator.lock_mouse();
	});
	document.querySelector("[data-shot]").addEventListener("click", () => {
		window.tryReactos.canvas().toBlob((b) => b && download("reactos.png", b));
	});
	document.querySelector("[data-save-state]").addEventListener("click", async () => {
		if (!emulator) return;
		say("Saving state…");
		const s = await emulator.save_state();
		download("state.bin", new Blob([s]));
		say("State saved (" + (s.byteLength / MB).toFixed(1) + " MB, uncompressed).");
	});
	render();
}
