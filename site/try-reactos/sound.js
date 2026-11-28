// The page's sound: the `ac97` package's virtual sound card on the machine, the Sound button and the volume slider, and the choice remembered in this
// browser. The page calls `prepare` from the click that starts a machine (a browser lets sound begin only from a user gesture), `attach` when the
// machine is built and `detach` when it goes (SITE-51, SITE-52, SITE-53).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

// The ?v= is the first 8 hex of the module's SHA-256; tools/stamp-web.py keeps it current (SITE-14).
import { attachAc97, isAc97State } from "./lib/ac97.js?v=46e2be12";

const STORE = "quill-try-reactos-sound";
const DEFAULTS = { volume: 0.7, muted: false };
// How long a stopped machine leaves the audio output running before it is suspended, so a Reset (which stops and starts at once) keeps one context.
const SUSPEND_AFTER_MS = 1500;

const button = document.querySelector("[data-sound]");
const slider = document.querySelector("[data-volume]");
const readout = document.querySelector("[data-sound-readout]");

let prefs = loadPrefs();
let context = null;
let handle = null;
// "ready" (a card will be or is attached), or "none" (this saved desktop has no sound card). Without Web Audio the card is still attached (a saved
// desktop made with it needs it), and plays nowhere.
let availability = "ready";
let webAudio = supported();
let timer = null;
let suspendTimer = null;

function loadPrefs() {
	try {
		const saved = JSON.parse(localStorage.getItem(STORE) || "null");
		if (saved && typeof saved === "object") {
			const volume = Number(saved.volume);
			return { volume: Number.isFinite(volume) ? Math.min(1, Math.max(0, volume)) : DEFAULTS.volume, muted: saved.muted === true };
		}
	} catch (e) {
		// Storage is blocked or holds something else: the defaults stand.
	}
	return { ...DEFAULTS };
}

function savePrefs() {
	try {
		localStorage.setItem(STORE, JSON.stringify(prefs));
	} catch (e) {
		// Not remembered; the page still works.
	}
}

function supported() {
	return typeof AudioContext === "function";
}

// Called from the click that starts a machine, before anything is awaited: makes the audio context (or wakes it) while the browser still
// counts the click as the visitor's own. Safe to call again and with no machine to follow.
export function prepare() {
	if (suspendTimer !== null) clearTimeout(suspendTimer);
	suspendTimer = null;
	webAudio = supported();
	if (webAudio) {
		try {
			if (!context || context.state === "closed") context = new AudioContext({ sampleRate: 48000, latencyHint: "interactive" });
			if (context.state !== "running") context.resume().catch(() => {});
		} catch (e) {
			context = null;
			webAudio = false;
		}
	}
	show();
}

// `restoring` is whether the machine starts from a saved desktop; `audio` is the card's registers the manifest saved with it (or null). A
// cold boot has no state, so it gets a card to find. A saved desktop made without the card must not get one: its PCI bus has no such slot.
export function attach(emulator, { restoring = false, audio = null } = {}) {
	detach();
	const saved = audio && isAc97State(audio) ? audio : null;
	if (restoring && !saved) {
		availability = "none";
		show();
		return;
	}
	availability = "ready";
	handle = attachAc97(emulator, { state: saved, volume: prefs.volume, muted: prefs.muted, context: webAudio ? context : null, onChange: show });
	if (window.tryReactos) window.tryReactos.soundHandle = handle;
	timer = setInterval(show, 1000);
	show();
}

export function detach() {
	if (timer !== null) clearInterval(timer);
	timer = null;
	if (handle) handle.detach();
	handle = null;
	if (window.tryReactos) window.tryReactos.soundHandle = null;
	availability = "ready";
	if (context && suspendTimer === null) {
		suspendTimer = setTimeout(() => {
			suspendTimer = null;
			if (!handle && context && context.state === "running") context.suspend().catch(() => {});
		}, SUSPEND_AFTER_MS);
	}
	show();
}

function outputState() {
	if (handle) return handle.outputState();
	return context ? context.state : "none";
}

// Sound is off for the visitor when it is muted, at zero, or held back by the browser.
function audible() {
	return !prefs.muted && prefs.volume > 0;
}

function show() {
	if (button) {
		const usable = availability === "ready" && webAudio;
		const blocked = Boolean(handle) && handle.outputState() !== "running" && handle.outputState() !== "none";
		const on = usable && audible() && !blocked;
		button.disabled = !usable;
		button.setAttribute("aria-pressed", String(on));
		button.textContent = on ? "Sound on" : "Sound off";
		button.title = usable
			? blocked
				? "The browser is holding the sound back. Select to turn it on."
				: on
					? "Select to mute the sound."
					: "Select to turn the sound on."
			: availability === "none"
				? "This saved desktop has no sound card."
				: "This browser cannot play the sound.";
	}
	if (slider) {
		slider.disabled = availability !== "ready" || !webAudio;
		slider.value = String(Math.round(prefs.volume * 100));
		slider.setAttribute("aria-valuetext", Math.round(prefs.volume * 100) + " percent" + (prefs.muted ? ", muted" : ""));
	}
	if (readout) {
		const dt = document.createElement("dt");
		dt.textContent = "Sound";
		const dd = document.createElement("dd");
		dd.textContent = describe();
		readout.replaceChildren(dt, dd);
	}
}

function describe() {
	if (availability === "none") return "this saved desktop has no sound card";
	if (!handle) return "off (" + Math.round(prefs.volume * 100) + " percent" + (prefs.muted ? ", muted" : "") + ")";
	const s = handle.stats();
	const parts = [webAudio ? "" : "this browser has no Web Audio", "AC'97 card " + (s.registered ? "in PCI slot " + (s.pciId >> 3) : "waiting for the machine"), "output " + s.output + (s.sink ? " (" + s.sink + ")" : "")];
	parts.push(Math.round(s.volume * 100) + " percent" + (s.muted ? ", muted" : ""));
	if (s.counters) parts.push("guest " + (s.playing ? "playing at " + s.guestRate + " Hz" : "idle") + ", " + s.counters.dmaFrames + " frames taken, " + s.drainedFrames + " dropped unheard");
	if (s.stateError) parts.push("saved registers refused: " + s.stateError);
	return parts.filter(Boolean).join(", ");
}

if (button) {
	button.addEventListener("click", async () => {
		const blocked = Boolean(handle) && handle.outputState() !== "running" && handle.outputState() !== "none";
		if (blocked) {
			// The browser held the sound back: this click is the gesture it wanted.
			prefs.muted = false;
			if (handle) {
				handle.setMuted(false);
				await handle.resume();
			}
		} else {
			prefs.muted = audible();
			if (!prefs.muted && prefs.volume === 0) prefs.volume = DEFAULTS.volume;
			if (handle) {
				handle.setMuted(prefs.muted);
				handle.setVolume(prefs.volume);
				if (!prefs.muted) await handle.resume();
			}
		}
		savePrefs();
		show();
	});
}

if (slider) {
	slider.addEventListener("input", async () => {
		prefs.volume = Number(slider.value) / 100;
		prefs.muted = false;
		if (handle) {
			handle.setVolume(prefs.volume);
			handle.setMuted(false);
			await handle.resume();
		}
		savePrefs();
		show();
	});
}

show();
