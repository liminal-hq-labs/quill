// The ReactOS experiment page: starts v86 with the machine the builder panel describes, when asked and not before. Exposes
// window.tryReactos for the measurement harness; the page's own API is the only way input reaches the guest.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

const player = document.querySelector("[data-player]");
const startButton = player.querySelector("[data-start]");
const stopButton = player.querySelector("[data-stop]");
const fullButton = player.querySelector("[data-fullscreen]");
const resetButton = player.querySelector("[data-reset]");
const localCursor = player.querySelector("[data-local-cursor]");
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
			disable_mouse: !LEGACY_MOUSE, // the page's own handler below sends the pointer (every coalesced move, scaled, optionally predicted)
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
		resetButton.disabled = false;
		pointer.calibrated = false;
		say(c.state ? "Running. Select the screen to give it the keyboard and the mouse." : "Starting ReactOS. This takes a while: there is no saved desktop to load.");
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
	resetButton.disabled = true;
	localCursor.setAttribute("hidden", "");
	say("Stopped. Writes to the guest disk are gone.");
	render();
}

// Back to the saved desktop (or a fresh boot when there is no state URL) without reloading the page: the machine is rebuilt from
// the same settings, and the browser already holds the downloaded files.
async function reset() {
	if (!emulator) return;
	resetButton.disabled = true;
	say("Resetting…");
	if (document.pointerLockElement) document.exitPointerLock();
	await stop();
	stats.ideReads = 0;
	stats.ideReadBytes = 0;
	stats.firstIdeReadMs = null;
	stats.screenSizes.length = 0;
	await start().catch(() => {});
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

// The pointer. v86's own mouse adapter is off; this one differs in four ways. It sends every coalesced move the browser has
// (getCoalescedEvents), never throttled. It scales a move by how large the screen is shown, so the guest pointer crosses the picture
// as far as the hand moves the system pointer (the stage can be 360 CSS pixels wide on a phone and 1024 pixels of guest screen).
// ?speed=N multiplies that (default 1.5; 1 is exact). ?lock=raw asks for unadjusted counts (what v86 does), without the system's
// acceleration. And ?cursor=local draws an arrow that follows the hand at once and is placed by dead reckoning: the guest's pointer
// is relative, one PS/2 count is one guest pixel with no acceleration in the guest (measured, see guest/polish/), and the guest
// clamps at the screen edge as this does. The first lock parks the guest pointer in a corner and moves it to the centre to
// calibrate. The guest still draws its own arrow, one frame or so behind, unless the transparent scheme in guest/polish/ is applied.
const query = new URLSearchParams(location.search);
// ?mouse=v86 turns the page's handler off and uses v86's own mouse adapter, as before (for comparing).
const LEGACY_MOUSE = query.get("mouse") === "v86";
const pointer = {
	speed: Number(query.get("speed")) || 1.5,
	raw: query.get("lock") === "raw",
	local: query.get("cursor") === "local",
	x: 0,
	y: 0,
	calibrated: false,
	residX: 0,
	residY: 0,
	buttons: 0,
	events: 0,
	lastTouch: null,
};

function guestSize() {
	const c = screen.querySelector("canvas");
	return { w: c && c.width ? c.width : 1024, h: c && c.height ? c.height : 768, canvas: c };
}

// How many guest pixels one CSS pixel of the page's pointer covers, from how large the picture is shown.
function cssToGuest() {
	const g = guestSize();
	if (!g.canvas) return 1;
	const r = g.canvas.getBoundingClientRect();
	const shown = Math.min(r.width, (r.height * g.w) / g.h);
	return shown > 0 ? g.w / shown : 1;
}

function placeLocalCursor() {
	if (!pointer.local || !pointer.calibrated) return;
	const g = guestSize();
	if (!g.canvas) return;
	const r = g.canvas.getBoundingClientRect();
	const sr = stage.getBoundingClientRect();
	const shownW = Math.min(r.width, (r.height * g.w) / g.h);
	const shownH = (shownW * g.h) / g.w;
	const left = r.left - sr.left + (r.width - shownW) / 2;
	const top = r.top - sr.top + (r.height - shownH) / 2;
	localCursor.style.transform = "translate(" + (left + (pointer.x * shownW) / g.w).toFixed(2) + "px," + (top + (pointer.y * shownH) / g.h).toFixed(2) + "px) scale(" + (shownW / g.w).toFixed(3) + ")";
	localCursor.style.transformOrigin = "0 0";
	localCursor.removeAttribute("hidden");
}

// Send whole counts, remember the fraction, and follow the guest's position (it clamps at the edges).
function sendMove(dx, dy) {
	if (!emulator) return;
	const g = guestSize();
	pointer.residX += dx;
	pointer.residY += dy;
	const ix = Math.trunc(pointer.residX);
	const iy = Math.trunc(pointer.residY);
	if (!ix && !iy) return;
	pointer.residX -= ix;
	pointer.residY -= iy;
	// A PS/2 packet carries one signed byte per axis, so a long move goes as several.
	let rx = ix;
	let ry = iy;
	while (rx || ry) {
		const sx = Math.max(-120, Math.min(120, rx));
		const sy = Math.max(-120, Math.min(120, ry));
		emulator.bus.send("mouse-delta", [sx, -sy]);
		rx -= sx;
		ry -= sy;
	}
	pointer.x = Math.max(0, Math.min(g.w - 1, pointer.x + ix));
	pointer.y = Math.max(0, Math.min(g.h - 1, pointer.y + iy));
	pointer.events++;
	placeLocalCursor();
}

// Park the guest pointer in the top-left corner, then move it to the middle: afterwards the position is known.
function calibrate() {
	const g = guestSize();
	for (let n = 0; n < Math.ceil(g.w / 100) + 1; n++) emulator.bus.send("mouse-delta", [-100, 0]);
	for (let n = 0; n < Math.ceil(g.h / 100) + 1; n++) emulator.bus.send("mouse-delta", [0, 100]);
	pointer.x = 0;
	pointer.y = 0;
	pointer.residX = 0;
	pointer.residY = 0;
	pointer.calibrated = true;
	sendMove(g.w / 2, g.h / 2);
}

function lockPointer() {
	if (!emulator) return;
	const el = document.body;
	const ask = pointer.raw ? el.requestPointerLock({ unadjustedMovement: true }) : el.requestPointerLock();
	Promise.resolve(ask).catch(() => el.requestPointerLock && el.requestPointerLock());
}

function pointerActive(e) {
	return !!emulator && (document.pointerLockElement || stage.contains(e.target));
}

function onMove(e) {
	if (!emulator || !document.pointerLockElement) return;
	const k = pointer.speed * (pointer.raw ? 1 : cssToGuest());
	const list = typeof e.getCoalescedEvents === "function" ? e.getCoalescedEvents() : [];
	if (list.length) for (const ev of list) sendMove(ev.movementX * k, ev.movementY * k);
	else sendMove(e.movementX * k, e.movementY * k);
}

function sendButtons(e) {
	pointer.buttons = e.buttons;
	emulator.bus.send("mouse-click", [!!(e.buttons & 1), !!(e.buttons & 4), !!(e.buttons & 2)]);
}

// Touch: a drag moves the guest pointer by the finger's distance, a short tap clicks.
function onTouch(e) {
	if (!emulator || !stage.contains(e.target)) return;
	const t = e.changedTouches[e.changedTouches.length - 1];
	if (e.type === "touchstart") pointer.lastTouch = { x: t.clientX, y: t.clientY, at: performance.now(), moved: 0 };
	else if (pointer.lastTouch && e.type === "touchmove") {
		const k = pointer.speed * cssToGuest();
		sendMove((t.clientX - pointer.lastTouch.x) * k, (t.clientY - pointer.lastTouch.y) * k);
		pointer.lastTouch.moved += Math.abs(t.clientX - pointer.lastTouch.x) + Math.abs(t.clientY - pointer.lastTouch.y);
		pointer.lastTouch.x = t.clientX;
		pointer.lastTouch.y = t.clientY;
		e.preventDefault();
	} else if (pointer.lastTouch && e.type === "touchend") {
		if (pointer.lastTouch.moved < 8 && performance.now() - pointer.lastTouch.at < 300) {
			emulator.bus.send("mouse-click", [true, false, false]);
			setTimeout(() => emulator && emulator.bus.send("mouse-click", [false, false, false]), 60);
		}
		pointer.lastTouch = null;
	}
}

function hookPointer() {
	document.addEventListener("pointermove", onMove, { passive: true });
	document.addEventListener("pointerlockchange", () => {
		if (!emulator) return;
		const locked = !!document.pointerLockElement;
		emulator.bus.send("mouse-pointer-lock", locked);
		if (locked && pointer.local && !pointer.calibrated) calibrate();
		if (locked) placeLocalCursor();
		else if (pointer.buttons) emulator.bus.send("mouse-click", [false, false, false]);
	});
	window.addEventListener("mousedown", (e) => { if (pointerActive(e) && document.pointerLockElement) { sendButtons(e); e.preventDefault(); } });
	window.addEventListener("mouseup", (e) => { if (emulator && document.pointerLockElement) { sendButtons(e); e.preventDefault(); } });
	window.addEventListener("wheel", (e) => {
		if (!pointerActive(e) || !document.pointerLockElement) return;
		emulator.bus.send("mouse-wheel", [e.deltaY < 0 ? 1 : e.deltaY > 0 ? -1 : 0, 0]);
		e.preventDefault();
	}, { passive: false });
	window.addEventListener("contextmenu", (e) => { if (emulator && stage.contains(e.target)) e.preventDefault(); });
	window.addEventListener("touchstart", onTouch, { passive: true });
	window.addEventListener("touchmove", onTouch, { passive: false });
	window.addEventListener("touchend", onTouch, { passive: true });
	window.addEventListener("resize", placeLocalCursor);
}

window.tryReactos = {
	stateName: new URLSearchParams(location.search).get("statename") || "state",
	emulator: null,
	config: null,
	stats,
	start,
	stop,
	input,
	reset,
	pointer,
	canvas: () => screen.querySelector("canvas"),
	instructions: () => (emulator ? emulator.get_instruction_counter() : 0),
	elapsedMs: () => (startedAt ? performance.now() - startedAt : 0),
};

// The guest draws a fixed number of pixels (1024 x 768). Shown at a fractional number of device pixels per guest pixel, sharp
// sampling stretches some columns more than others, which squishes the glyphs; so the picture is scaled smoothly, and sharp pixels
// are used only when the ratio is a whole number. ?scale=crisp forces sharp pixels, ?scale=smooth forces smoothing, and ?scale=int
// sizes the screen to the largest whole-number multiple that fits (letterboxed), which is exact on any display.
function updateScale() {
	const c = screen.querySelector("canvas");
	if (!c || !c.width) return;
	const mode = new URLSearchParams(location.search).get("scale");
	c.style.removeProperty("width");
	c.style.removeProperty("height");
	if (mode === "int") {
		const fit = (stage.getBoundingClientRect().width * devicePixelRatio) / c.width;
		const n = Math.max(1, Math.floor(fit));
		c.style.setProperty("width", (c.width * n) / devicePixelRatio + "px", "important");
		c.style.setProperty("height", (c.height * n) / devicePixelRatio + "px", "important");
		c.classList.add("crisp");
		c.style.imageRendering = "pixelated";
		return;
	}
	const ratio = (c.getBoundingClientRect().width * devicePixelRatio) / c.width;
	const whole = ratio >= 1 && Math.abs(ratio - Math.round(ratio)) < 0.02;
	const crisp = mode === "crisp" || (mode !== "smooth" && whole);
	c.classList.toggle("crisp", crisp);
	// v86 sets image-rendering inline, which beats the stylesheet.
	c.style.imageRendering = crisp ? "pixelated" : "auto";
}

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
		if (!emulator || document.pointerLockElement) return;
		if (LEGACY_MOUSE) emulator.lock_mouse();
		else lockPointer();
	});
	new ResizeObserver(updateScale).observe(stage);
	window.addEventListener("resize", updateScale);
	setInterval(updateScale, 1000);
	resetButton.addEventListener("click", () => reset());
	if (!LEGACY_MOUSE) hookPointer();
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
