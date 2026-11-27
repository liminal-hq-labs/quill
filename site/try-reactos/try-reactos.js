// The ReactOS experiment page: starts v86 with the machine the builder panel describes, when asked and not before. Exposes
// window.tryReactos for the measurement harness; the page's own API is the only way input reaches the guest.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

// The ?v= on a module's address is the first 8 hex of its SHA-256; tools/stamp-web.py keeps them current (SITE-14).
import { applyManifest, buildLine, emulatorFiles, loadManifest, refetchFresh } from "./manifest.js?v=73de4d88";
import * as hostLink from "./host-link.js?v=01f7d6f8";
import * as sound from "./sound.js?v=295d6486";

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
const buildLineEl = document.querySelector("[data-build]");
const freshLink = document.querySelector("[data-fresh]");

const BOOT = { auto: 0, cd: 0x213, hd: 0x132 };
const MB = 1024 * 1024;

let emulator = null;
let startedAt = 0;
let tick = null;
// The web-manifest.json the page started from (see manifest.js); settled before Start does anything.
let loadedManifest = { status: "none", manifest: null, url: "", message: "" };
let manifestReady = Promise.resolve();

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

// Fill the panel from the query string, so a run can be named by its URL. The page adds nothing to the address.
function readQuery() {
	const q = new URLSearchParams(location.search);
	for (const el of form.elements) {
		if (!el.name || !q.has(el.name)) continue;
		if (el.type === "checkbox") el.checked = q.get(el.name) !== "0";
		else el.value = q.get(el.name);
	}
}

// Read web-manifest.json, fill the panel's disk and state from it (the address bar's own fields win) and show its build line.
function loadWebManifest() {
	const q = new URLSearchParams(location.search);
	manifestReady = loadManifest(q.get("manifest") || "", false).then((loaded) => {
		loadedManifest = loaded;
		if (loaded.status === "ready") applyManifest(form, q, loaded);
		else if (loaded.status !== "none") {
			document.querySelector("[data-diag]").open = true;
			say(loaded.message);
		}
		buildLineEl.textContent = buildLine(loaded);
	});
}

// Replace the browser's copy of the manifest and the emulator files, then load the page again at the same address.
async function forceFresh(event) {
	event.preventDefault();
	say("Fetching fresh copies…");
	await manifestReady;
	await refetchFresh(loadedManifest);
	location.reload();
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

// The disk parts' size when neither the address bar nor the parts' manifest.json says: the size the pipeline makes (tools/make-parts.mjs).
const DEFAULT_CHUNK = 8 * 1024 * 1024;

// The whole file read with range requests, or zstd parts named DIR/START-END.img.zst, as in v86's own ReactOS demo.
// For parts the URL is DIR/.img.zst and DIR/manifest.json holds the image size and the part size; a chunk given in the panel or as
// ?chunk= overrides the manifest (an older part set without a "chunk" field falls back to DEFAULT_CHUNK).
async function asyncImage(url, chunk) {
	const parts = Boolean(chunk) || /\/\.img\.zst$/.test(url);
	if (!parts) return { url, async: true, size: await sizeOf(url) };
	const manifest = await (await fetch(url.replace(/[^/]*$/, "manifest.json"))).json();
	return { url, async: true, size: manifest.size, fixed_chunk_size: chunk || manifest.chunk || DEFAULT_CHUNK, use_parts: true };
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
		await manifestReady;
		const run = form.elements.run.value.trim();
		// The saved state belongs to the emulator build the manifest names; it is used only when the state is the manifest's own.
		const named = loadedManifest.status === "ready" && c.state && new URL(c.state, location.href).pathname.endsWith(loadedManifest.manifest.state.split("/").pop());
		const files = await emulatorFiles(named ? loadedManifest.manifest : null, run ? "run=" + encodeURIComponent(run) : "", false);
		const { V86 } = await import(files.libv86);
		const opts = {
			wasm_path: files.wasm,
			memory_size: c.ram * MB,
			vga_memory_size: c.vga * MB,
			screen: { container: screen },
			bios: { url: files.bios },
			vga_bios: { url: files.vgabios },
			acpi: c.acpi,
			disable_jit: !c.jit,
			boot_order: BOOT[c.boot] || 0,
			autostart: true,
			disable_speaker: true,
			disable_mouse: !LEGACY_MOUSE, // the page's own handler below sends the pointer (locked or followed, one packet a frame)
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
		hostLink.attach(emulator);
		sound.attach(emulator);
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
	hostLink.detach();
	sound.detach();
	if (pointer.following) stopFollowing();
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

// The pointer. v86's own mouse adapter is off; this one has two ways to move the guest pointer, and differs from v86's in five ways.
//
// Lock (the default): a click on the screen locks the system pointer and the moves it reports are sent as relative counts. Every
// coalesced move the browser has is counted (getCoalescedEvents). A move is scaled by how large the screen is shown, so the guest
// pointer crosses the picture as far as the hand moves the system pointer (the stage can be 360 CSS pixels wide on a phone and 1024
// pixels of guest screen). ?speed=N multiplies that (default 1.5; 1 is exact). ?lock=raw asks for unadjusted counts, without the
// system's acceleration. The counts of one animation frame go out as one PS/2 packet with both axes (or several, when the move is
// longer than a packet carries): a diagonal stays diagonal however small the browser's moves are, and a fast hand cannot fill the
// guest's input queue (the guest keeps about 100 packets and drops the rest of a burst).
//
// Follow (?mouse=follow, and the default in Firefox on Linux): no pointer lock. The guest pointer is moved to the place under the
// system pointer, frame by frame, from the system pointer's absolute position. Firefox on Wayland reports a locked pointer's moves as
// whole device pixels, each move cut to an integer on its own (widget/gtk/nsWindowWayland.cpp), so the slower axis of a slow
// diagonal is dropped and the guest pointer moves in horizontal and vertical runs; the absolute position has no such loss. The guest
// is relative with no acceleration (one count is one pixel, see guest/polish/) and clamps at the edges, so the position is dead
// reckoning: selecting the screen, and entering it again, parks the guest pointer in a corner and moves it to the system pointer.
// ?mouse=lock forces the lock, ?mouse=v86 turns the page's handler off and uses v86's own mouse adapter (for comparing).
//
// ?cursor=local draws an arrow that follows the hand at once, at the dead-reckoned guest position. The first lock parks the guest
// pointer in a corner and moves it to the centre to calibrate. The guest still draws its own arrow, one frame or so behind, unless
// the transparent scheme in guest/polish/ is applied.
const query = new URLSearchParams(location.search);
const ua = navigator.userAgent;
const MOUSE_MODE = ["v86", "lock", "follow"].includes(query.get("mouse")) ? query.get("mouse") : /Firefox\//.test(ua) && /Linux/.test(ua) && !/Android/.test(ua) ? "follow" : "lock";
const LEGACY_MOUSE = MOUSE_MODE === "v86";
const FOLLOW = MOUSE_MODE === "follow";
const pointer = {
	mode: MOUSE_MODE,
	speed: Number(query.get("speed")) || 1.5,
	raw: query.get("lock") === "raw",
	local: query.get("cursor") === "local",
	x: 0,
	y: 0,
	calibrated: false,
	residX: 0,
	residY: 0,
	// Counts waiting for the next animation frame, and how many packets have gone to the guest.
	pendX: 0,
	pendY: 0,
	frame: 0,
	packets: 0,
	// Follow: the screen was selected (and is not deselected by a click elsewhere), and the system pointer is over it.
	following: false,
	inside: false,
	buttons: 0,
	events: 0,
	lastTouch: null,
};

function guestSize() {
	const c = screen.querySelector("canvas");
	return { w: c && c.width ? c.width : 1024, h: c && c.height ? c.height : 768, canvas: c };
}

// Where the guest's screen is shown, in CSS pixels relative to the viewport (the canvas can be letterboxed).
function shownRect(g) {
	const r = g.canvas.getBoundingClientRect();
	const w = Math.min(r.width, (r.height * g.w) / g.h);
	const h = (w * g.h) / g.w;
	return { left: r.left + (r.width - w) / 2, top: r.top + (r.height - h) / 2, w, h };
}

// How many guest pixels one CSS pixel of the page's pointer covers, from how large the picture is shown.
function cssToGuest() {
	const g = guestSize();
	if (!g.canvas) return 1;
	const s = shownRect(g);
	return s.w > 0 ? g.w / s.w : 1;
}

function placeLocalCursor() {
	if (!pointer.local || !pointer.calibrated) return;
	const g = guestSize();
	if (!g.canvas) return;
	const s = shownRect(g);
	const sr = stage.getBoundingClientRect();
	localCursor.style.transform = "translate(" + (s.left - sr.left + (pointer.x * s.w) / g.w).toFixed(2) + "px," + (s.top - sr.top + (pointer.y * s.h) / g.h).toFixed(2) + "px) scale(" + (s.w / g.w).toFixed(3) + ")";
	localCursor.style.transformOrigin = "0 0";
	localCursor.removeAttribute("hidden");
}

// Send what the frame collected: both axes in one packet, split only when a packet's signed byte cannot carry it.
function flushMove() {
	pointer.frame = 0;
	if (!emulator) return;
	let rx = pointer.pendX;
	let ry = pointer.pendY;
	pointer.pendX = 0;
	pointer.pendY = 0;
	while (rx || ry) {
		const sx = Math.max(-120, Math.min(120, rx));
		const sy = Math.max(-120, Math.min(120, ry));
		emulator.bus.send("mouse-delta", [sx, -sy]);
		pointer.packets++;
		rx -= sx;
		ry -= sy;
	}
}

// Count whole guest pixels, remember the fraction, follow the guest's position (it clamps at the edges), send on the next frame.
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
	pointer.pendX += ix;
	pointer.pendY += iy;
	if (!pointer.frame) pointer.frame = requestAnimationFrame(flushMove);
	pointer.x = Math.max(0, Math.min(g.w - 1, pointer.x + ix));
	pointer.y = Math.max(0, Math.min(g.h - 1, pointer.y + iy));
	pointer.events++;
	placeLocalCursor();
}

// Park the guest pointer in the top-left corner: afterwards the position is known. Counts not yet sent are dropped (the park
// covers them).
function park() {
	const g = guestSize();
	if (pointer.frame) cancelAnimationFrame(pointer.frame);
	pointer.frame = 0;
	pointer.pendX = 0;
	pointer.pendY = 0;
	for (let n = 0; n < Math.ceil(g.w / 100) + 1; n++) emulator.bus.send("mouse-delta", [-100, 0]);
	for (let n = 0; n < Math.ceil(g.h / 100) + 1; n++) emulator.bus.send("mouse-delta", [0, 100]);
	pointer.x = 0;
	pointer.y = 0;
	pointer.residX = 0;
	pointer.residY = 0;
	pointer.calibrated = true;
}

// Lock: park, then move to the middle.
function calibrate() {
	const g = guestSize();
	park();
	sendMove(g.w / 2, g.h / 2);
}

// Follow: the guest pixel under the system pointer, and a move there.
function followTo(e) {
	const g = guestSize();
	if (!g.canvas) return;
	const s = shownRect(g);
	const tx = Math.max(0, Math.min(g.w - 1, Math.round(((e.clientX - s.left) * g.w) / s.w)));
	const ty = Math.max(0, Math.min(g.h - 1, Math.round(((e.clientY - s.top) * g.h) / s.h)));
	sendMove(tx - pointer.x, ty - pointer.y);
}

function startFollowing(e) {
	pointer.following = true;
	pointer.inside = true;
	stage.style.cursor = "none";
	park();
	followTo(e);
}

function stopFollowing() {
	pointer.following = false;
	stage.style.removeProperty("cursor");
	if (pointer.buttons) emulator && emulator.bus.send("mouse-click", [false, false, false]);
	pointer.buttons = 0;
}

function lockPointer() {
	if (!emulator) return;
	const el = document.body;
	const ask = pointer.raw ? el.requestPointerLock({ unadjustedMovement: true }) : el.requestPointerLock();
	Promise.resolve(ask).catch(() => el.requestPointerLock && el.requestPointerLock());
}

// The guest has the pointer: locked to the page, or followed while over the screen.
function captured() {
	return !!emulator && (!!document.pointerLockElement || (pointer.following && pointer.inside));
}

function pointerActive(e) {
	return !!emulator && (document.pointerLockElement || stage.contains(e.target));
}

function onMove(e) {
	if (!emulator) return;
	if (FOLLOW) {
		if (pointer.following && pointer.inside) followTo(e);
		return;
	}
	if (!document.pointerLockElement) return;
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
	if (FOLLOW) {
		// A mouse (not a finger) entering the screen again: the guest pointer may have stopped at an edge, so park and move.
		stage.addEventListener("pointerenter", (e) => {
			if (e.pointerType !== "mouse" || !emulator || !pointer.following) return;
			pointer.inside = true;
			park();
			followTo(e);
		});
		stage.addEventListener("pointerleave", (e) => {
			if (e.pointerType === "mouse") pointer.inside = false;
		});
		// A click outside the screen gives the pointer back to the page.
		window.addEventListener("mousedown", (e) => { if (pointer.following && !stage.contains(e.target)) stopFollowing(); }, true);
	}
	window.addEventListener("mousedown", (e) => { if (pointerActive(e) && captured()) { sendButtons(e); e.preventDefault(); } });
	window.addEventListener("mouseup", (e) => { if (emulator && (captured() || pointer.buttons)) { sendButtons(e); e.preventDefault(); } });
	window.addEventListener("wheel", (e) => {
		if (!pointerActive(e) || !captured()) return;
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
	loadWebManifest();
	if (freshLink) freshLink.addEventListener("click", forceFresh);
	startButton.addEventListener("click", () => start().catch(() => {}));
	stopButton.addEventListener("click", stop);
	fullButton.addEventListener("click", () => {
		if (stage.requestFullscreen) stage.requestFullscreen();
	});
	stage.addEventListener("click", (e) => {
		if (!emulator || document.pointerLockElement) return;
		if (LEGACY_MOUSE) emulator.lock_mouse();
		else if (FOLLOW) {
			if (!pointer.following) startFollowing(e);
		} else lockPointer();
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
