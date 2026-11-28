// The page's end of the host link: attaches the `host-link` package's link to the machine's COM1 when it starts, drops it when it stops, and
// carries the shared clipboard between the browser and the ReactOS guest: the chip and its buttons, the keys the page catches, the paste
// box, the diagnostics rows. The rules (when the browser clipboard is read or written, the limits, what is shown) are in
// docs/plans/host-integration.md and docs/host-link.md; nothing here makes a network request, and clipboard content is never stored.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

// The ?v= is the first 8 hex of the module's SHA-256; tools/stamp-web.py keeps it current (SITE-14).
import { attach as attachLink, ClipboardSync, contentFromPaste, contentFromRead, createBrowserPort } from "./vendor/v86-host-link/host-link.js?v=0d03ac0f";
// kit-link: begin
import * as kitLink from "./kit-link.js?v=fabefa1b";
// kit-link: end

const readout = document.querySelector("[data-link-readout]");
const clipReadout = document.querySelector("[data-clip-readout]");
const stage = document.querySelector("[data-stage]");
const bar = document.querySelector("[data-clip-bar]");
const chip = document.querySelector("[data-clip-chip]");
const toggle = document.querySelector("[data-clip-toggle]");
const copyButton = document.querySelector("[data-clip-copy]");
const pasteButton = document.querySelector("[data-clip-paste]");
const cancelButton = document.querySelector("[data-clip-cancel]");
const progress = document.querySelector("[data-clip-progress]");
const dialog = document.querySelector("[data-clip-dialog]");
const dialogText = document.querySelector("[data-clip-dialog-text]");
const dialogSend = document.querySelector("[data-clip-dialog-send]");

let host = null;
let sync = null;
let emulatorRef = null;
let timer = null;
let flash = "";
let flashTimer = null;
let selected = false;
let pasteKey = null;
let pasteTimer = null;
const swallowUp = new Set();
const held = { ctrl: false, shift: false };
const isMac = /Mac|iPhone|iPad/.test(navigator.platform || "");
const port = createBrowserPort();

// Sharing is on by default and the visitor's choice is remembered (localStorage may be missing or refuse: the page works without it).
// `?clipboard=off` turns it off for a visit. Whatever the setting, nothing is shared until the visitor has clicked or pressed a key on the
// screen once: the first click is the visitor choosing to use the machine, and it is the gesture the browsers' clipboard rules want.
const STORE_KEY = "quill.try-reactos.clipboard";
function readSetting() {
	if (new URLSearchParams(location.search).get("clipboard") === "off") return false;
	try {
		return localStorage.getItem(STORE_KEY) !== "off";
	} catch (e) {
		return true;
	}
}
let enabled = readSetting();
if (toggle) toggle.checked = enabled;

function codes(counts) {
	const entries = Object.entries(counts);
	return entries.length ? entries.map(([code, n]) => code + " " + n).join(", ") : "none";
}

function describe(stats) {
	if (stats.state === "closed") return "off";
	if (!stats.peer) return stats.bytesOut === 0 ? "waiting for the guest to speak first" : "no helper";
	const caps = stats.peer.caps || [];
	const mods = stats.peer.mods || [];
	const peer = (stats.peer.app || "helper") + ", protocol " + stats.peer.proto + (caps.length ? ", offers " + caps.join(" ") : "") + (mods.length ? ", modules " + mods.join(" ") : "");
	if (stats.state === "version") return peer + ": a different version, so the link is off";
	const credit = stats.credit.window - stats.credit.outstanding + " of " + stats.credit.window + " bytes of credit";
	const state = stats.state === "stalled" ? " (waiting for credit)" : "";
	return peer + state + ". In " + stats.bytesIn + " bytes, " + stats.messagesIn + " messages. Out " + stats.bytesOut + " bytes, " + stats.messagesOut + " messages. " + credit + ". Errors: " + codes(stats.errors) + ". Reported by the helper: " + codes(stats.peerErrors) + ".";
}

function row(target, title, text) {
	if (!target) return;
	const dt = document.createElement("dt");
	dt.textContent = title;
	const dd = document.createElement("dd");
	dd.textContent = text;
	target.replaceChildren(dt, dd);
}

const KIND = { text: "text", rtf: "rich text", dib: "picture" };
function bytes(n) {
	return n < 10240 ? n + " bytes" : n < 10485760 ? (n / 1024).toFixed(0) + " KB" : (n / 1048576).toFixed(1) + " MB";
}

// What is shown about the clipboard: sizes and kinds, never content.
function clipText(status) {
	if (!sync) return "off";
	const link = { off: "no link", "no-helper": "guest helper not found", "not-ready": "guest not ready", connected: "connected" }[status.link];
	const parts = [status.enabled ? "on" : "off", link];
	if (status.enabled && status.link === "connected" && !status.active) parts.push("waiting for a click on the screen");
	const c = status.counters;
	let text = parts.join(", ") + ". Offers from the guest " + c.offers + ", copied to this computer " + c.copiedOut + ", pasted into the guest " + c.pastedIn + ", refused " + c.refused + ", left for the button " + c.skipped + ".";
	if (status.held) {
		const sizes = Object.entries(status.held.sizes).map(([fmt, n]) => (KIND[fmt] || fmt) + " " + bytes(n));
		text += " Last copy in the guest: " + (sizes.join(", ") || "nothing shareable") + (status.held.written ? " (written here)" : " (not written yet)") + ".";
	}
	if (status.busy !== "idle") text += " " + (status.busy === "sending" ? "Sending" : "Copying") + " " + Math.round(status.progress * 100) + " %.";
	return text;
}

function chipLabel(status) {
	if (!status.enabled) return { state: "off", text: "Clipboard: off" };
	if (status.link === "no-helper") return { state: "absent", text: "Clipboard: guest helper not found" };
	if (status.link !== "connected") return { state: "waiting", text: "Clipboard: guest not ready" };
	if (status.busy === "sending") return { state: "busy", text: "Sending to ReactOS… " + Math.round(status.progress * 100) + " %" };
	if (status.busy === "copying") return { state: "busy", text: "Copying from ReactOS… " + Math.round(status.progress * 100) + " %" };
	if (status.note) return { state: "note", text: status.note };
	if (flash) return { state: "note", text: flash };
	if (!status.active) return { state: "connected", text: "Clipboard: connected. Click the screen to start sharing" };
	return { state: "connected", text: "Clipboard: connected" };
}

function render() {
	if (readout) row(readout, "Link", host ? describe(host.stats()) : "off");
	const status = sync ? sync.status() : null;
	row(clipReadout, "Clipboard", status ? clipText(status) : "off");
	if (!bar) return;
	bar.hidden = !host;
	if (!status) return;
	const shown = chipLabel(status);
	if (chip) {
		chip.dataset.state = shown.state;
		if (chip.textContent !== shown.text) chip.textContent = shown.text;
	}
	const ready = status.enabled && status.link === "connected";
	if (copyButton) copyButton.disabled = !(status.enabled && status.held && Object.keys(status.held.sizes).length > 0 && status.busy !== "sending");
	if (pasteButton) pasteButton.disabled = !ready || status.busy === "copying";
	if (cancelButton) cancelButton.hidden = status.busy !== "sending";
	if (progress) {
		progress.hidden = status.busy === "idle";
		progress.value = Math.round(status.progress * 100);
	}
}

function say(text, ms = 4000) {
	flash = text;
	if (flashTimer) clearTimeout(flashTimer);
	flashTimer = setTimeout(() => {
		flash = "";
		render();
	}, ms);
	render();
}

// ---- keys ----

const MODIFIERS = new Set(["Control", "Shift", "Alt", "Meta", "AltGraph", "CapsLock", "NumLock", "ScrollLock", "OS"]);
const MAC_KEYS = { c: [0x2e], x: [0x2d], v: [0x2f], a: [0x1e], z: [0x2c] };

function editable(target) {
	return target instanceof HTMLElement && (target.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName));
}

// A key to the guest as the machine's own scancodes: Ctrl (or Shift), the key down and up, then the modifier up when the visitor has let go.
function sendChord(modifier, key) {
	if (!emulatorRef) return;
	const mod = modifier === "shift" ? [0x2a] : [0x1d];
	const release = modifier === "shift" ? !held.shift : !held.ctrl;
	const down = [...mod, ...key];
	const up = [...key.map((c, i) => (i === key.length - 1 ? c | 0x80 : c)), ...(release ? mod.map((c) => c | 0x80) : [])];
	emulatorRef.keyboard_send_scancodes([...down, ...up]);
}

function onKeyDown(e) {
	if (e.key === "Control") held.ctrl = true;
	if (e.key === "Shift") held.shift = true;
	if (!sync || !enabled || !selected) return;
	if (editable(e.target) && !(e.target instanceof HTMLElement && e.target.hasAttribute("data-clip-box"))) return;
	const key = e.key.length === 1 ? e.key.toLowerCase() : e.key;
	if (key === "Escape" && sync.sending) {
		sync.cancel();
		pasteKey = null;
		e.preventDefault();
		e.stopImmediatePropagation();
		return;
	}
	const cmd = isMac && e.metaKey && !e.ctrlKey && !e.altKey;
	if (cmd && MAC_KEYS[key]) {
		// Cmd on a Mac is the guest's Ctrl: v86 would send the Windows key, which opens the Start menu.
		e.preventDefault();
		e.stopImmediatePropagation();
		swallowUp.add(e.code);
		if (key === "v") startPaste("ctrl");
		else {
			if (key === "c" || key === "x") sync.gesture();
			sendChord("ctrl", MAC_KEYS[key]);
		}
		return;
	}
	if (!e.altKey && ((e.ctrlKey && !e.shiftKey && key === "v") || (e.shiftKey && !e.ctrlKey && key === "Insert"))) {
		// The V (or Insert) is held back from the guest: the browser pastes into the paste box, the page pushes what it got, then sends the key.
		e.stopImmediatePropagation();
		swallowUp.add(e.code);
		startPaste(e.shiftKey ? "shift" : "ctrl");
		return;
	}
	if (!MODIFIERS.has(e.key)) sync.gesture();
}

function onKeyUp(e) {
	if (e.key === "Control") held.ctrl = false;
	if (e.key === "Shift") held.shift = false;
	if (swallowUp.delete(e.code)) e.stopImmediatePropagation();
}

const pasteBox = document.createElement("textarea");
pasteBox.setAttribute("data-clip-box", "");
pasteBox.setAttribute("aria-hidden", "true");
pasteBox.tabIndex = -1;
pasteBox.style.cssText = "position:fixed;left:-9999px;top:0;width:1px;height:1px;opacity:0;";
document.body.appendChild(pasteBox);

function startPaste(modifier) {
	pasteKey = { modifier, key: modifier === "shift" ? [0xe0, 0x52] : [0x2f] };
	pasteBox.value = "";
	pasteBox.focus({ preventScroll: true });
	clearTimeout(pasteTimer);
	// No paste event in 300 ms (a browser that will not paste here): give the keyboard back and send the key as it was.
	pasteTimer = setTimeout(() => finishPaste(null), 300);
}

function backToScreen() {
	if (stage) stage.focus({ preventScroll: true });
}

function sendPasteKey(key) {
	const mod = key.modifier === "shift" ? [0x2a] : [0x1d];
	const release = key.modifier === "shift" ? !held.shift : !held.ctrl;
	const base = key.key;
	const up = base.map((c, i) => (i === base.length - 1 ? c | 0x80 : c));
	if (emulatorRef) emulatorRef.keyboard_send_scancodes([...mod, ...base, ...up, ...(release ? mod.map((c) => c | 0x80) : [])]);
}

// The paste: push what the browser gave, then the key. A refusal shows why and sends nothing; a guest that has no helper just gets its key.
async function finishPaste(content) {
	clearTimeout(pasteTimer);
	const key = pasteKey;
	pasteKey = null;
	backToScreen();
	if (!key || !emulatorRef) return;
	if (!content || !sync) {
		sendPasteKey(key);
		return;
	}
	const result = await sync.push(content);
	const sendAnyway = result.ok || result.reason === "empty" || result.reason === "not-ready" || result.reason === "off";
	if (sendAnyway && emulatorRef) sendPasteKey(key);
	render();
}

pasteBox.addEventListener("paste", (e) => {
	e.preventDefault();
	if (!pasteKey) return;
	finishPaste(contentFromPaste(e));
});

// The Paste into ReactOS button: the browser may ask once (Chromium) or show its own Paste button (Firefox, Safari). If reading is not
// possible the visitor pastes into a box, which works on a phone too.
async function pasteFromButton() {
	if (!sync) return;
	let content;
	try {
		content = await contentFromRead();
	} catch (error) {
		openDialog();
		return;
	}
	const result = await sync.push(content);
	if (result.ok) say("Your clipboard is in ReactOS: paste there (Ctrl+V, or Edit then Paste).");
}

function openDialog() {
	if (!dialog) return;
	dialog.hidden = false;
	if (dialogText) {
		dialogText.value = "";
		dialogText.focus();
	}
}

if (dialogSend) {
	dialogSend.addEventListener("click", async () => {
		const text = dialogText ? dialogText.value : "";
		dialog.hidden = true;
		if (sync && text) {
			const result = await sync.push({ text });
			if (result.ok) say("Your text is in ReactOS: paste there.");
		}
	});
}

if (copyButton) {
	copyButton.addEventListener("click", async () => {
		if (!sync) return;
		// A click is the gesture: the copy is written at once, whatever the age of the last key.
		await sync.copyOut();
	});
}
if (pasteButton) pasteButton.addEventListener("click", pasteFromButton);
if (cancelButton) cancelButton.addEventListener("click", () => sync && sync.cancel());

if (toggle) {
	toggle.addEventListener("change", () => {
		enabled = toggle.checked;
		try {
			localStorage.setItem(STORE_KEY, enabled ? "on" : "off");
		} catch (e) {
			// The setting is for this visit only.
		}
		if (sync) sync.setEnabled(enabled);
		render();
	});
}

// Selecting the screen (a click or a touch on it) is the first gesture; a click elsewhere puts the keyboard back to the page.
window.addEventListener(
	"pointerdown",
	(e) => {
		const inside = stage && stage.contains(e.target);
		const inBar = bar && bar.contains(e.target);
		if (inside) {
			selected = true;
			if (sync) sync.setActive(true);
		} else if (!inBar) selected = false;
	},
	true,
);
if (stage) {
	stage.addEventListener(
		"pointerup",
		() => {
			if (sync && selected) sync.gesture();
		},
		true,
	);
}
window.addEventListener("keydown", onKeyDown, true);
window.addEventListener("keyup", onKeyUp, true);
document.addEventListener("visibilitychange", () => {
	if (document.hidden && sync) sync.clear();
});

// ---- the machine ----

/**
 * Attaches to a started machine. `restored` is true when it loads a saved state, whose helper is already listening; a cold boot waits for
 * the helper to speak first (SITE-91).
 */
export function attach(emulator, options = {}) {
	detach();
	emulatorRef = emulator;
	host = attachLink(emulator, { mode: options.restored ? "restored" : "cold" });
	sync = new ClipboardSync(host, port, { restored: !!options.restored, onChange: render });
	sync.setEnabled(enabled);
	// A click on the screen before the machine started counts: the visitor pressed Start, which is a click, but sharing waits for the screen.
	sync.setActive(false);
	selected = false;
	if (window.tryReactos) {
		window.tryReactos.hostLink = host;
		window.tryReactos.clipboard = sync;
	}
	// kit-link: begin
	kitLink.attach(host, emulator);
	// kit-link: end
	render();
	timer = setInterval(render, 500);
}

export function detach() {
	if (timer !== null) clearInterval(timer);
	timer = null;
	clearTimeout(pasteTimer);
	pasteKey = null;
	if (sync) sync.dispose();
	sync = null;
	// kit-link: begin
	kitLink.detach();
	// kit-link: end
	if (host) host.detach();
	if (window.tryReactos) {
		window.tryReactos.hostLink = null;
		window.tryReactos.clipboard = null;
	}
	host = null;
	emulatorRef = null;
	flash = "";
	if (dialog) dialog.hidden = true;
	render();
}

/**
 * Whether a state saved now would restore a quiet serial port (SITE-91): nothing queued, the port's pending bits clear, no transfer under way.
 * The state builder's half is the same rule (harness/host-link-boot.ts).
 */
export function saveCheck() {
	if (!host || !emulatorRef) return { ok: true, why: "" };
	try {
		const uart = emulatorRef.v86.cpu.devices.uart0;
		if (uart.input.length !== 0 || uart.ints !== 0) return { ok: false, why: "The serial port has bytes waiting (" + uart.input.length + " queued, interrupts " + uart.ints + "). A state saved now would start with a deaf helper. Wait a few seconds and save again." };
	} catch (e) {
		// Another build of v86: the port cannot be looked at.
	}
	if (sync && !sync.quiet) return { ok: false, why: "A copy or a paste is moving. Save when it has finished." };
	return { ok: true, why: "" };
}
