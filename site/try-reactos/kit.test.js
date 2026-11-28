// The kit's rules on fake handles: the Audio row's wording, the badge's condition, the refresh gate and the panel's timer (none while it is closed).
// Run with `bun test site/try-reactos/kit.test.js` from the repository root.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, test } from "bun:test";
import { andList, attachCard, AUDIO_REFRESH_MS, badgeVisible, BADGE_TEXT, count, describeAudio, describeVolume, detachCard, level, pcMuted, playTracker, PLAYED_WITHIN_MS, shouldRefresh, update } from "./kit.js";

const line = (left, right = left, muted = false) => ({ raw: 0, left, right, muted });
const codec = (over = {}) => ({ master: line(100), pcm: line(100), record: line(0), mic: line(0, 0, true), recordSelect: line(0), rate: 22050, vra: true, ...over });
const stats = (over = {}) => ({
	registered: true,
	guestRate: 22050,
	playing: true,
	counters: { buffersDone: 177, dmaFrames: 39045, starvedFrames: 0 },
	...over,
});
const sink = { underrunEvents: 0, underrunFrames: 0, dropped: 0, silent: 0 };

describe("wording", () => {
	test("counts take their separators and the right number", () => {
		expect(count(39045, "frame")).toBe("39,045 frames");
		expect(count(1, "buffer")).toBe("1 buffer");
		expect(count(0, "underrun")).toBe("0 underruns");
	});

	test("a level is one number, or one per side when they differ", () => {
		expect(level(line(80))).toBe("80 percent");
		expect(level(line(80, 60))).toBe("80 percent left, 60 percent right");
	});

	test("lists read as prose", () => {
		expect(andList(["master"])).toBe("master");
		expect(andList(["master", "wave"])).toBe("master and wave");
		expect(andList(["master", "wave", "record"])).toBe("master, wave and record");
	});

	test("the volume half names what is muted", () => {
		expect(describeVolume(codec())).toBe("PC volume: master 100 percent, wave 100 percent, record 0 percent");
		expect(describeVolume(codec({ master: line(100, 100, true) }))).toBe("PC volume: master 100 percent, wave 100 percent, record 0 percent; master muted");
		expect(describeVolume(codec({ master: line(100, 100, true), pcm: line(74, 74, true) }))).toContain("; master and wave muted");
	});

	test("the Audio row says what the guest chose, what the card took, what the output did and the PC's volume", () => {
		expect(describeAudio(stats(), sink, codec({ master: line(100, 100, true) }))).toBe(
			"22,050 Hz, 16-bit stereo, playing. 177 buffers, 39,045 frames, 0 starved. Output: 0 underruns (0 frames), 0 frames dropped, 0 silent frames. PC volume: master 100 percent, wave 100 percent, record 0 percent; master muted.",
		);
		expect(describeAudio(stats({ playing: false }), null, codec())).toContain("idle. ");
		expect(describeAudio(stats(), null, codec())).toContain("Output: none. ");
		expect(describeAudio(stats({ registered: false, counters: null }), null, null)).toBe("waiting for the machine");
	});
});

describe("the badge's rule", () => {
	test("a muted master or wave volume counts, and nothing else does", () => {
		expect(pcMuted(codec())).toBe(false);
		expect(pcMuted(codec({ master: line(100, 100, true) }))).toBe(true);
		expect(pcMuted(codec({ pcm: line(74, 74, true) }))).toBe(true);
		expect(pcMuted(codec({ record: line(0, 0, true), mic: line(0, 0, true) }))).toBe(false);
		expect(pcMuted(null)).toBe(false);
	});

	test("it shows only while the card plays, or just played, and the PC is muted", () => {
		const muted = codec({ master: line(100, 100, true) });
		expect(badgeVisible(muted, true)).toBe(true);
		expect(badgeVisible(muted, false)).toBe(false);
		expect(badgeVisible(codec(), true)).toBe(false);
		expect(badgeVisible(null, true)).toBe(false);
	});

	test("the tracker holds for 2 s after playing stops, and notices a sound that came and went between two looks", () => {
		const tracker = playTracker();
		expect(tracker.recent(0)).toBe(false);
		tracker.observe(stats({ playing: false }), 0);
		expect(tracker.recent(0)).toBe(false);
		tracker.observe(stats({ playing: true }), 1000);
		expect(tracker.recent(1000)).toBe(true);
		tracker.observe(stats({ playing: false }), 2000);
		expect(tracker.recent(1000 + PLAYED_WITHIN_MS)).toBe(true);
		expect(tracker.recent(1001 + PLAYED_WITHIN_MS)).toBe(false);
		const frames = (n) => stats({ playing: false, counters: { buffersDone: 1, dmaFrames: n, starvedFrames: 0 } });
		tracker.observe(frames(100), 10000);
		expect(tracker.recent(10000)).toBe(false);
		tracker.observe(frames(500), 11000);
		expect(tracker.recent(11000)).toBe(true);
		tracker.reset();
		expect(tracker.recent(11000)).toBe(false);
	});
});

describe("the refresh gate", () => {
	test("never while closed, and at most twice a second while open", () => {
		expect(shouldRefresh(false, 10000, 0)).toBe(false);
		expect(shouldRefresh(true, 10000, 9600)).toBe(false);
		expect(shouldRefresh(true, 10000, 10000 - AUDIO_REFRESH_MS)).toBe(true);
	});
});

// A page of two elements and a panel, and timers that only count.
function fakePage(open) {
	const listeners = [];
	const diag = {
		open,
		addEventListener: (type, fn) => listeners.push([type, fn]),
		removeEventListener: (type, fn) => listeners.splice(0, listeners.length, ...listeners.filter(([t, f]) => t !== type || f !== fn)),
	};
	const row = { textContent: "off" };
	const badge = { hidden: true, textContent: "", title: "" };
	const doc = {
		querySelector: (selector) => ({ "[data-diag]": diag, "[data-audio-readout] dd": row, "[data-sound-badge]": badge })[selector] ?? null,
	};
	const timers = new Map();
	let next = 1;
	let clock = 0;
	const deps = {
		document: doc,
		now: () => clock,
		setInterval: (fn, ms) => (timers.set(next, { fn, ms }), next++),
		clearInterval: (id) => timers.delete(id),
	};
	const toggle = (value) => {
		diag.open = value;
		for (const [type, fn] of [...listeners]) if (type === "toggle") fn();
	};
	return { diag, row, badge, timers, deps, toggle, listeners, advance: (ms) => (clock += ms) };
}

function fakeCard(initial = {}) {
	const card = {
		sinkCalls: 0,
		stats: () => stats(),
		codec: () => codec({ master: line(100, 100, true) }),
		sinkStats: async () => (card.sinkCalls++, sink),
	};
	if (initial.stats) card.stats = () => initial.stats;
	return card;
}

describe("the panel and the badge", () => {
	test("closed: no timer at all, the row is off, and the badge still follows the card", () => {
		const page = fakePage(false);
		const card = fakeCard();
		attachCard(card, page.deps);
		expect(page.timers.size).toBe(0);
		expect(page.row.textContent).toBe("off while the panel is closed");
		expect(card.sinkCalls).toBe(0);
		expect(page.badge.hidden).toBe(false);
		expect(page.badge.textContent).toBe(BADGE_TEXT);
		expect(page.badge.title).toContain("Volume Control");
		detachCard();
	});

	test("opening starts one timer at twice a second and closing stops it", async () => {
		const page = fakePage(false);
		const card = fakeCard();
		attachCard(card, page.deps);
		page.toggle(true);
		expect(page.timers.size).toBe(1);
		expect([...page.timers.values()][0].ms).toBe(AUDIO_REFRESH_MS);
		await Promise.resolve();
		await Promise.resolve();
		expect(page.row.textContent).toContain("22,050 Hz");
		page.toggle(true);
		expect(page.timers.size).toBe(1);
		page.toggle(false);
		expect(page.timers.size).toBe(0);
		expect(page.row.textContent).not.toContain("Hz");
		detachCard();
	});

	test("a card that opens with the panel already open starts its timer at once, and detach stops it", () => {
		const page = fakePage(true);
		attachCard(fakeCard(), page.deps);
		expect(page.timers.size).toBe(1);
		detachCard();
		expect(page.timers.size).toBe(0);
		expect(page.listeners).toEqual([]);
		expect(page.badge.hidden).toBe(true);
		expect(page.row.textContent).toBe("off");
	});

	test("the badge goes when the guest unmutes, comes back when it mutes while playing, and holds 2 s after the sound ends", () => {
		const page = fakePage(false);
		const card = fakeCard();
		attachCard(card, page.deps);
		expect(page.badge.hidden).toBe(false);
		card.codec = () => codec();
		update();
		expect(page.badge.hidden).toBe(true);
		card.codec = () => codec({ pcm: line(74, 74, true) });
		update();
		expect(page.badge.hidden).toBe(false);
		card.stats = () => stats({ playing: false });
		page.advance(PLAYED_WITHIN_MS - 1);
		update();
		expect(page.badge.hidden).toBe(false);
		page.advance(2);
		update();
		expect(page.badge.hidden).toBe(true);
		detachCard();
	});

	test("an idle card on a muted PC shows nothing: the badge needs sound", () => {
		const page = fakePage(false);
		attachCard(fakeCard({ stats: stats({ playing: false }) }), page.deps);
		expect(page.badge.hidden).toBe(true);
		detachCard();
	});

	test("update with no card is safe", () => {
		detachCard();
		update();
	});
});
