// The rules of the page's Host Link Kit glue (kit-link.js) as pure functions with the words they make: the taper between the page's slider and the guest's
// mixer, the echo rules of Keep in step, the badge, the switches and the capability list each one makes, the System row's lines and the log buffer. No
// DOM, no card, no link; kit-link.test.js holds them to their rules. They are the rules of v86-host-link's demo, and the page's own strings are in `T`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

// Every string the kit glue shows. `{name}` is filled in by `fill`.
export const T = {
	pcSound: "PC sound",
	system: "System",
	log: "Log",
	badge: "Muted in the PC",
	badgeTitle: "The PC has muted its own sound. Select to unmute it.",
	step: "Keep the PC's volume in step with the slider",
	stepOn: "The PC's volume follows the slider, and the slider follows the PC.",
	stepNoHelper: "Needs the PC's helper with its audio module.",
	stepNoMixer: "The PC's sound driver has no volume control.",
	test: "Play a test sound",
	testNeeds: "Needs the PC's helper with its audio module.",
	testing: "Playing a one-second tone in the PC…",
	testCounted: "The card counted {n} frames. The tone is one second at 22,050 Hz.",
	testSilent: "The PC played it, but the card counted no frames.",
	testFailed: "The PC could not play it.",
	switches: "What the PC may share",
	switchesNote: "Turning one off sends a new hello, so the PC stops using that channel at once.",
	groups: { clip: "Clipboard", audio: "Audio", sys: "System and log", power: "Power" },
	shutdown: "Shut down the PC",
	shutdownConfirm: "Shut the PC down now? Programs in it may ask to save their work.",
	shutdownYes: "Shut down",
	shutdownNo: "Cancel",
	shutdownNeeds: "Needs the PC's helper with its power module, and the Power switch on.",
	shutdownAsking: "Asking the PC to shut down…",
	shutdownRefused: "The PC refused to shut down.",
	ending: { shutdown: "The PC says it is shutting down.", logoff: "The PC says it is logging off.", end: "The PC says its session is ending." },
	off: "The PC has powered off.",
	insert: "Insert the Host Link Kit CD",
	eject: "Eject the CD",
	inserting: "Inserting…",
	inserted: "The Host Link Kit CD is in the PC's drive. Open it in My Computer and run WIN32\\SETUP.EXE.",
	ejected: "The CD is out.",
	insertFailed: "Could not insert the CD: {why}",
	copy: "Copy the log",
	copied: "Copied {n} lines.",
	copyFailed: "Could not copy.",
	level: "Show",
	levels: { info: "everything", warn: "warnings and errors", error: "errors only" },
	closed: "Open the panel to ask.",
	waiting: "Waiting for the PC…",
	noSys: "The PC's helper has no system module, or the System and log switch is off.",
	noLog: "No log yet.",
	noHelper: "the helper has not said yet",
	noName: "a sound device",
	mixerEmpty: "its Volume Control is empty",
	up: "up {time}",
	memory: "memory {free} of {total} MiB free",
	idle: "idle {n} %",
	noIdle: "no processor figure",
	clock: "clock {clock}, UTC{offset}{zone}",
};

export const fill = (text, values) => text.replace(/\{(\w+)\}/g, (_, key) => String(values[key]));

/** A codec write within this long of the page's own `audio set` that matches it is that request coming back, not the visitor. */
export const ECHO_MS = 500;
/** How far, in percent of the mixer's range, a codec level may differ from what was asked and still be that request (the codec steps 1.5 dB). */
export const ECHO_TOLERANCE = 4;
/** The master volume's range in the codec: 31 steps of 1.5 dB. */
export const ATTENUATION_DB = 46.5;
/** The badge stays up this long after the card last played, so a short sound that ended is still explained. */
export const PLAYED_WITHIN_MS = 2000;

const clamp = (n, low, high) => Math.min(high, Math.max(low, n));

// ---- the taper ----
// The page's slider is perceptual (the speaker's gain is its square); the guest's mixer percent is a number of decibels. The same slider position is the
// same decibels in both places, so the page converts: 40 log10(slider) is the gain in decibels.

/** The mixer percent that attenuates the guest as much as the speaker gain (the slider squared) would. */
export function sliderToMixer(slider) {
	if (!(slider > 0)) return 0;
	return clamp(Math.round(100 * (1 + (40 * Math.log10(clamp(slider, 0, 100) / 100)) / ATTENUATION_DB)), 0, 100);
}

/** The slider position a mixer percent stands for: the inverse of `sliderToMixer`. */
export function mixerToSlider(mixer) {
	if (!(mixer > 0)) return 0;
	return clamp(Math.round(100 * Math.pow(10, ((clamp(mixer, 0, 100) / 100 - 1) * ATTENUATION_DB) / 40)), 0, 100);
}

/** A codec master or PCM register as a percent of the mixer's range: its attenuation, 0 to 31 steps, from the loudest. */
export const codecMixerPercent = (value) => Math.round((100 * (31 - (value & 31))) / 31);

/** The speaker gain: held at 1 while the guest follows the slider (two gains in series must never both follow it), else the slider's. */
export const speakerVolume = (keepInStep, slider) => (keepInStep ? 1 : clamp(slider, 0, 100) / 100);

// ---- Keep in step ----

export const createStep = () => ({ on: false, lastSet: null });

const ask = (state, slider, now) => {
	const mixer = sliderToMixer(slider);
	return { state: { ...state, lastSet: { mixer, at: now } }, set: { master: mixer }, slider: undefined };
};

/** The visitor moved the slider: with Keep in step on, the guest's master follows, once. */
export const sliderMoved = (state, slider, now) => (state.on ? ask(state, slider, now) : { state, set: null, slider: undefined });
/** The visitor turned Keep in step on: the slider's position goes to the guest. */
export const switchedOn = (state, slider, now) => ask({ ...state, on: true }, slider, now);
export const switchedOff = (state) => ({ state: { ...state, on: false, lastSet: null }, set: null, slider: undefined });

/** The guest's `audio mixer`: `source=self` is the answer to the page's own request and is ignored; a change from the guest moves the slider. Never asks. */
export function mixerMessage(state, mixer) {
	if (mixer.source === "self" || !state.on || mixer.master === undefined) return { state, set: null, slider: undefined };
	return { state, set: null, slider: mixerToSlider(mixer.master) };
}

/** The guest wrote the codec's master register: within ECHO_MS of the page's own request and equal to it, it is that request coming back. Never asks. */
export function codecWrite(state, value, now) {
	if (!state.on) return { state, set: null, slider: undefined };
	const level = codecMixerPercent(value);
	const last = state.lastSet;
	if (last && now - last.at <= ECHO_MS && Math.abs(level - last.mixer) <= ECHO_TOLERANCE) return { state, set: null, slider: undefined };
	return { state, set: null, slider: mixerToSlider(level) };
}

// ---- the badge ----

/** Whether the guest has muted what the card plays: its master volume or its PCM-out volume. */
export const pcMuted = (codec) => Boolean(codec) && (codec.master.muted || codec.pcm.muted);
export const badgeVisible = (codec, recentlyPlayed) => pcMuted(codec) && recentlyPlayed;

/** Remembers when the card last played, from what it reports each time it is looked at. `recent(now)` is whether that was within PLAYED_WITHIN_MS. */
export function playTracker() {
	let lastFrames = null;
	let lastPlayed = -Infinity;
	return {
		observe(stats, now) {
			const frames = stats && stats.counters ? stats.counters.dmaFrames : null;
			if (stats && stats.playing) lastPlayed = now;
			else if (frames !== null && lastFrames !== null && frames > lastFrames) lastPlayed = now;
			lastFrames = frames;
		},
		recent: (now) => now - lastPlayed <= PLAYED_WITHIN_MS,
	};
}

/** The `audio set` that clears what the badge says: the master's mute, and the PCM-out's when the codec shows it muted too. */
export const unmuteRequest = (codec) => (codec && codec.pcm.muted ? { mastermute: false, wavemute: false } : { mastermute: false });

/** `output` for `audio host`: what the visitor hears. */
export const hostOutput = (contextState, muted) => (muted ? "muted" : contextState === "running" ? "on" : contextState === "suspended" ? "suspended" : "off");

// ---- text ----

const plural = (n, noun) => n + " " + noun + (n === 1 ? "" : "s");
const percent = (n, label, muted) => label + " " + n + " percent" + (muted ? " (muted)" : "");

/** The guest's half of the sound: its devices and mixer as the helper told them. */
export function describePc(devices, mixer) {
	if (!devices && !mixer) return T.noHelper;
	const named = devices ? (devices.outname || T.noName) + (devices.driver ? " (driver " + devices.driver + ")" : "") + ", " + devices.out + " out, " + devices.in + " in. " : "";
	if (!mixer) return named;
	if (mixer.lines === 0) return named + T.mixerEmpty + ".";
	const parts = [];
	if (mixer.master !== undefined) parts.push(percent(mixer.master, "master", mixer.mastermute));
	if (mixer.wave !== undefined) parts.push(percent(mixer.wave, "wave", mixer.wavemute));
	return named + "mixer: " + (parts.join(", ") || "no levels") + ", " + plural(mixer.lines, "line") + ".";
}

export function uptimeText(seconds) {
	const days = Math.floor(seconds / 86400);
	const hours = Math.floor((seconds % 86400) / 3600);
	const minutes = Math.floor((seconds % 3600) / 60);
	if (!days && !hours && !minutes) return (seconds % 60) + " s";
	const parts = [];
	if (days) parts.push(days + " d");
	if (days || hours) parts.push(hours + " h");
	parts.push((days || hours ? String(minutes).padStart(2, "0") : minutes) + " min");
	return parts.join(" ");
}

/** `bias` is minutes west of UTC, so UTC-5 is 300. */
export function offsetText(bias) {
	if (bias === 0) return "±0";
	const minutes = Math.abs(bias);
	return (bias > 0 ? "−" : "+") + Math.floor(minutes / 60) + (minutes % 60 ? ":" + String(minutes % 60).padStart(2, "0") : "");
}

/** The words of the System row for one `sys info`. */
export function systemLines(info) {
	return [
		info.os + ", " + info.app,
		fill(T.up, { time: uptimeText(info.uptime) }),
		fill(T.memory, { free: Math.round(info.memfree / 1024), total: Math.round(info.memtotal / 1024) }),
		info.idle === undefined ? T.noIdle : fill(T.idle, { n: info.idle }),
		fill(T.clock, { clock: info.clock.replace("T", " "), offset: offsetText(info.bias), zone: info.zone ? " " + info.zone : "" }),
	];
}

// ---- the switches ----

// A switch is a group of channels. Clipboard and Audio are on when the page lists them; System and log and Power are on by default (SITE-124).
export const GROUPS = [
	{ key: "clip", channels: ["clip"], defaults: [] },
	{ key: "audio", channels: ["audio"], defaults: [] },
	{ key: "sys", channels: ["sys", "log"], defaults: ["sys", "log"] },
	{ key: "power", channels: ["power"], defaults: ["power"] },
];

/** The channel a `caps` token belongs to: `audio`, `audio.2` and `audio-set` are all `audio`. */
export const channelOf = (token) => token.split(/[.-]/)[0];

export function initialSwitches(tokens) {
	const have = new Set(tokens.map(channelOf));
	return Object.fromEntries(GROUPS.map((group) => [group.key, group.defaults.length > 0 || group.channels.some((channel) => have.has(channel))]));
}

/** `tokens` with each group set as `switches` say: a group that is off loses every token of its channels; one that is on gains its defaults when it has none. */
export function capsFor(tokens, switches) {
	const out = tokens.filter((token) => {
		const group = GROUPS.find((candidate) => candidate.channels.includes(channelOf(token)));
		return !group || switches[group.key];
	});
	for (const group of GROUPS) {
		if (!switches[group.key]) continue;
		for (const channel of group.defaults) if (!out.some((token) => channelOf(token) === channel)) out.push(channel);
	}
	return out;
}

/** The switches over the page's own list, so turning one back on restores exactly the tokens the page began with. */
export function switchBoard(initial) {
	const base = [...initial];
	const switches = initialSwitches(base);
	return {
		switches,
		set(key, on) {
			switches[key] = on;
			return this.tokens();
		},
		tokens: () => capsFor([...base, ...GROUPS.flatMap((group) => group.defaults).filter((channel) => !base.some((token) => channelOf(token) === channel))], switches),
	};
}

// ---- the panel's timers and the log ----

/** Asks at once and then every `every` ms while open, and not at all otherwise. */
export function createPoller(ask, timers, every = 1000) {
	let handle = null;
	return {
		open() {
			if (handle !== null) return;
			ask();
			handle = timers.setInterval(ask, every);
		},
		close() {
			if (handle === null) return;
			timers.clearInterval(handle);
			handle = null;
		},
		get running() {
			return handle !== null;
		},
	};
}

/** The last `max` lines, kept only while open: `close` forgets them. `wanted` is the least severe level shown. */
export function createLogBuffer(max = 200) {
	let lines = [];
	let open = false;
	const rank = { error: 0, warn: 1, info: 2 };
	return {
		open: () => void (open = true),
		close() {
			open = false;
			lines = [];
		},
		push(line) {
			if (!open) return false;
			lines.push(line);
			if (lines.length > max) lines = lines.slice(-max);
			return true;
		},
		get lines() {
			return lines.slice();
		},
		text: (wanted = "info") =>
			lines
				.filter((line) => rank[line.level] <= rank[wanted])
				.map((line) => line.level.padEnd(5) + " " + (line.src || "-") + ": " + line.text)
				.join("\n"),
	};
}
