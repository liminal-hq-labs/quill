// The Host Link Kit's page side, as far as the sound card goes: the Audio row in Diagnostics (what the guest chose, what the card took, what the
// output played and where the guest's own volume stands, read from the card's registers) and the "Muted in the PC" badge beside the Sound button.
// sound.js hands over the card's handle; nothing here talks to the guest, and nothing leaves the tab (SITE-122, SITE-123).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/** The Audio row is redrawn at most twice a second, and only while Diagnostics is open. */
export const AUDIO_REFRESH_MS = 500;
/** The badge stays up this long after the card last played, so a short sound that ended is still explained. */
export const PLAYED_WITHIN_MS = 2000;

export const BADGE_TEXT = "Muted in the PC";
export const BADGE_HINT = "The PC has muted its own sound: open its Volume Control (the speaker in the taskbar, or Control Panel > Sounds and Audio Devices) to unmute";

const NUMBER = new Intl.NumberFormat("en-CA");

/** "1 buffer", "177 buffers": the count with its separators and the noun in the right number. */
export function count(n, noun) {
	return NUMBER.format(n) + " " + noun + (n === 1 ? "" : "s");
}

/** A line's level in words: "80 percent", or "80 percent left, 60 percent right" when the sides differ. */
export function level(line) {
	return line.left === line.right ? line.left + " percent" : line.left + " percent left, " + line.right + " percent right";
}

/** "a", "a and b", "a, b and c". */
export function andList(names) {
	return names.length < 2 ? names.join("") : names.slice(0, -1).join(", ") + " and " + names[names.length - 1];
}

/** Whether the guest has muted what the card plays: its master volume or its wave (PCM out) volume. */
export function pcMuted(codec) {
	return Boolean(codec) && (codec.master.muted || codec.pcm.muted);
}

/** The volume half of the Audio row, from the codec's registers: "PC volume: master 100 percent, wave 100 percent, record 0 percent; master muted". */
export function describeVolume(codec) {
	const text = "PC volume: master " + level(codec.master) + ", wave " + level(codec.pcm) + ", record " + level(codec.record);
	const muted = [codec.master.muted ? "master" : "", codec.pcm.muted ? "wave" : "", codec.record.muted ? "record" : ""].filter(Boolean);
	return text + (muted.length ? "; " + andList(muted) + " muted" : "");
}

/**
 * The Audio row's text. `stats` is the card's `stats()`, `sink` its `sinkStats()` (or null without an output), `codec` its `codec()`. The guest's PCM
 * out is always 16-bit stereo; the rate is the one the guest chose.
 */
export function describeAudio(stats, sink, codec) {
	if (!stats || !stats.registered || !stats.counters) return "waiting for the machine";
	const c = stats.counters;
	const guest = NUMBER.format(stats.guestRate) + " Hz, 16-bit stereo, " + (stats.playing ? "playing" : "idle") + ". ";
	const taken = count(c.buffersDone, "buffer") + ", " + count(c.dmaFrames, "frame") + ", " + NUMBER.format(c.starvedFrames) + " starved. ";
	const out = sink
		? "Output: " + count(sink.underrunEvents, "underrun") + " (" + count(sink.underrunFrames, "frame") + "), " + count(sink.dropped, "frame") + " dropped, " + count(sink.silent, "silent frame") + ". "
		: "Output: none. ";
	return guest + taken + out + (codec ? describeVolume(codec) + "." : "");
}

/** Whether the Audio row should be redrawn now: the panel is open and at least AUDIO_REFRESH_MS has passed since the last time. */
export function shouldRefresh(open, now, last) {
	return open && now - last >= AUDIO_REFRESH_MS;
}

/**
 * Remembers when the card last played, from what it reports each time it is looked at: playing now, or more frames taken since the last look (a sound
 * shorter than the gap between two looks still counts). `recent(now)` is whether that was within PLAYED_WITHIN_MS.
 */
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
		reset() {
			lastFrames = null;
			lastPlayed = -Infinity;
		},
	};
}

/** The badge's rule: the guest's volume is muted while the card plays, or just played. */
export function badgeVisible(codec, recentlyPlayed) {
	return pcMuted(codec) && recentlyPlayed;
}

let handle = null;
let env = null;
let timer = null;
let busy = false;
const tracker = playTracker();

function open() {
	return Boolean(env.diag) && env.diag.open;
}

function setRow(text) {
	if (env.row) env.row.textContent = text;
}

function drawBadge() {
	if (!env.badge) return;
	const stats = handle ? handle.stats() : null;
	tracker.observe(stats, env.now());
	const show = Boolean(handle) && badgeVisible(handle.codec(), tracker.recent(env.now()));
	env.badge.hidden = !show;
	env.badge.textContent = show ? BADGE_TEXT : "";
	env.badge.title = show ? BADGE_HINT : "";
}

async function drawRow() {
	if (!handle || busy || !open()) return;
	busy = true;
	const mine = handle;
	try {
		const sink = await mine.sinkStats();
		if (handle === mine && open()) setRow(describeAudio(mine.stats(), sink, mine.codec()));
	} finally {
		busy = false;
	}
}

function tick() {
	drawBadge();
	drawRow();
}

// The panel's timer exists only while the panel is open and a card is attached.
function sync() {
	const want = Boolean(handle) && open();
	if (want && timer === null) {
		timer = env.setInterval(tick, AUDIO_REFRESH_MS);
		tick();
	} else if (!want && timer !== null) {
		env.clearInterval(timer);
		timer = null;
	}
	if (!want) setRow(handle ? "off while the panel is closed" : "off");
}

function onToggle() {
	sync();
}

/** Starts watching `handle` (the card's handle from attachAc97). `deps` is for tests: a document and timers. */
export function attachCard(card, deps = {}) {
	detachCard();
	const doc = deps.document || document;
	env = {
		diag: doc.querySelector("[data-diag]"),
		row: doc.querySelector("[data-audio-readout] dd"),
		badge: doc.querySelector("[data-sound-badge]"),
		setInterval: deps.setInterval || ((fn, ms) => setInterval(fn, ms)),
		clearInterval: deps.clearInterval || ((id) => clearInterval(id)),
		now: deps.now || (() => performance.now()),
	};
	handle = card;
	tracker.reset();
	if (env.diag) env.diag.addEventListener("toggle", onToggle);
	sync();
	drawBadge();
}

/** Lets the card go: the timer stops, the row says off and the badge hides. */
export function detachCard() {
	if (!env) return;
	if (env.diag) env.diag.removeEventListener("toggle", onToggle);
	handle = null;
	sync();
	drawBadge();
	env = null;
}

/** A change the page was told about (the output changed, the guest wrote a mixer register): the badge is brought up to date. */
export function update() {
	if (env && handle) drawBadge();
}
