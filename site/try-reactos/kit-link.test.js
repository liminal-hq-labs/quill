// The kit glue's rules (kit-rules.js): the taper, the echo rules of Keep in step, the badge, 1,000 seeded random sequences of the visitor's, the guest's own
// and the card's changes that must never loop, the switches, the panel's timers and the log buffer, and the words of the System and PC sound rows.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, test } from "bun:test";
import {
	ECHO_MS,
	T,
	capsFor,
	channelOf,
	createLogBuffer,
	createPoller,
	describePc,
	fill,
	initialSwitches,
	offsetText,
	switchBoard,
	systemLines,
	uptimeText,
	badgeVisible,
	codecMixerPercent,
	codecWrite,
	createStep,
	hostOutput,
	mixerMessage,
	mixerToSlider,
	playTracker,
	sliderMoved,
	sliderToMixer,
	speakerVolume,
	switchedOff,
	switchedOn,
	unmuteRequest,
} from "./kit-rules.js";

const codec = (masterMuted, pcmMuted) => ({ master: { muted: masterMuted }, pcm: { muted: pcmMuted } });

describe("the taper", () => {
	test("the ends and the middle", () => {
		expect(sliderToMixer(100)).toBe(100);
		expect(sliderToMixer(0)).toBe(0);
		// 50 percent on the page's slider is a gain of 0.25: -12 dB, which is 74 percent of the codec's 46.5 dB.
		expect(sliderToMixer(50)).toBe(74);
		expect(mixerToSlider(100)).toBe(100);
		expect(mixerToSlider(0)).toBe(0);
	});

	test("it goes up with the slider and round-trips within a percent above the bottom", () => {
		let last = -1;
		for (let slider = 0; slider <= 100; slider++) {
			const mixer = sliderToMixer(slider);
			expect(mixer).toBeGreaterThanOrEqual(last);
			last = mixer;
			if (slider >= 10) expect(Math.abs(mixerToSlider(mixer) - slider)).toBeLessThanOrEqual(2);
		}
	});

	test("the codec's attenuation as a percent of the mixer's range", () => {
		expect(codecMixerPercent(0)).toBe(100);
		expect(codecMixerPercent(0x1f1f)).toBe(0);
		expect(codecMixerPercent(0x1010)).toBe(48);
		// The mute bit and the right channel do not change the level.
		expect(codecMixerPercent(0x8000)).toBe(100);
	});

	test("the speaker's gain is held at 1 while the guest follows the slider", () => {
		expect(speakerVolume(true, 30)).toBe(1);
		expect(speakerVolume(false, 30)).toBe(0.3);
		expect(speakerVolume(false, 140)).toBe(1);
	});
});

describe("Keep in step", () => {
	test("off, the slider asks for nothing and the guest's changes move nothing", () => {
		const state = createStep();
		expect(sliderMoved(state, 40, 0).set).toBeNull();
		expect(mixerMessage(state, { source: "user", master: 10, lines: 6 }).slider).toBeUndefined();
		expect(codecWrite(state, 0x1010, 0).slider).toBeUndefined();
	});

	test("turning it on sends the slider's level; moving the slider sends again", () => {
		const on = switchedOn(createStep(), 50, 1000);
		expect(on.set).toEqual({ master: 74 });
		expect(on.state.on).toBe(true);
		expect(sliderMoved(on.state, 100, 1200).set).toEqual({ master: 100 });
		expect(switchedOff(on.state).state.on).toBe(false);
	});

	test("a mixer with source self is the page's own answer and is ignored; one from the guest moves the slider and never asks", () => {
		const { state } = switchedOn(createStep(), 50, 1000);
		expect(mixerMessage(state, { source: "self", master: 74, lines: 6 })).toEqual({ state, set: null, slider: undefined });
		const user = mixerMessage(state, { source: "user", master: 100, lines: 6 });
		expect(user.slider).toBe(100);
		expect(user.set).toBeNull();
	});

	test("a codec write that matches the page's last request within 500 ms is its echo; later, or different, it is the guest's", () => {
		const { state } = switchedOn(createStep(), 50, 1000);
		// 74 percent of the range is attenuation step 8 (0x0808).
		expect(codecWrite(state, 0x0808, 1000 + ECHO_MS).slider).toBeUndefined();
		expect(codecWrite(state, 0x0808, 1000 + ECHO_MS + 1).slider).toBe(mixerToSlider(codecMixerPercent(0x0808)));
		expect(codecWrite(state, 0x0000, 1100).slider).toBe(100);
		expect(codecWrite(state, 0x0808, 1100).set).toBeNull();
	});

	test("the mute bit alone, at the same level, is still an echo", () => {
		const { state } = switchedOn(createStep(), 50, 1000);
		expect(codecWrite(state, 0x8808, 1200).slider).toBeUndefined();
	});
});

describe("the badge", () => {
	test("it shows when the guest has muted master or PCM out and the card played lately", () => {
		expect(badgeVisible(codec(true, false), true)).toBe(true);
		expect(badgeVisible(codec(false, true), true)).toBe(true);
		expect(badgeVisible(codec(false, false), true)).toBe(false);
		expect(badgeVisible(codec(true, false), false)).toBe(false);
		expect(badgeVisible(null, true)).toBe(false);
	});

	test("the played tracker sees frames that rose between two looks", () => {
		const tracker = playTracker();
		tracker.observe({ playing: false, counters: { dmaFrames: 0 } }, 0);
		expect(tracker.recent(100)).toBe(false);
		tracker.observe({ playing: false, counters: { dmaFrames: 500 } }, 1000);
		expect(tracker.recent(2500)).toBe(true);
		expect(tracker.recent(3100)).toBe(false);
	});

	test("the button unmutes the master, and the PCM out when it is muted too", () => {
		expect(unmuteRequest(codec(true, false))).toEqual({ mastermute: false });
		expect(unmuteRequest(codec(true, true))).toEqual({ mastermute: false, wavemute: false });
	});
});

describe("audio host", () => {
	test("output is what the visitor hears", () => {
		expect(hostOutput("running", false)).toBe("on");
		expect(hostOutput("running", true)).toBe("muted");
		expect(hostOutput("suspended", false)).toBe("suspended");
		expect(hostOutput("closed", false)).toBe("off");
	});
});

// A seeded generator, so a failing sequence can be replayed.
function mulberry32(seed) {
	return () => {
		seed = (seed + 0x6d2b79f5) | 0;
		let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
		t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
		return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
	};
}

/** The 5-bit attenuation the AC'97 driver writes for a mixer percent (ReactOS: linear in steps, measured in docs/audio-probe.md). */
const attenuation = (percent) => Math.round(31 * (1 - percent / 100));

describe("1,000 seeded sequences of the visitor's, the guest's and the card's changes", () => {
	test("Keep in step never loops, never asks in answer to the guest, and leaves mute to the visitor", () => {
		for (let seed = 1; seed <= 1000; seed++) {
			const random = mulberry32(seed);
			let now = 0;
			let state = createStep();
			let slider = 70;
			let speakerMuted = false; // the visitor's, and nobody else's
			let guestMixer = 100; // the guest's master as a percent of its range
			let sends = 0;
			let visitorMoves = 0;
			// What the guest has yet to say: the module's mixer message and the codec's register write, with the time each is due.
			let due = [];
			const apply = (result) => {
				state = result.state;
				if (result.slider !== undefined) slider = result.slider;
				if (result.set) {
					sends++;
					guestMixer = result.set.master;
					due.push({ at: now + Math.floor(random() * 200), kind: "codec", source: "self", level: guestMixer });
					due.push({ at: now + Math.floor(random() * 300), kind: "mixer", source: "self", level: guestMixer });
				}
			};
			for (let step = 0; step < 60; step++) {
				now += Math.floor(random() * 400);
				const action = random();
				if (action < 0.3) {
					slider = Math.floor(random() * 101);
					visitorMoves++;
					apply(sliderMoved(state, slider, now));
				} else if (action < 0.4) {
					visitorMoves++;
					apply(state.on ? switchedOff(state) : switchedOn(state, slider, now));
				} else if (action < 0.5) {
					speakerMuted = !speakerMuted;
				} else if (action < 0.8) {
					// The guest's own volume control moves: a codec write soon, a mixer message a little later (or never, for a driver with no callback).
					guestMixer = Math.floor(random() * 101);
					due.push({ at: now + 10 + Math.floor(random() * 50), kind: "codec", source: "user", level: guestMixer });
					if (random() < 0.8) due.push({ at: now + Math.floor(random() * 400), kind: "mixer", source: "user", level: guestMixer });
				}
				due.sort((a, b) => a.at - b.at);
				while (due.length && due[0].at <= now) {
					const event = due.shift();
					const before = { sends, speakerMuted };
					if (event.kind === "codec") apply(codecWrite(state, (attenuation(event.level) << 8) | attenuation(event.level), event.at));
					else apply(mixerMessage(state, { source: event.source, master: event.level, lines: 6 }));
					// A message from the guest never makes the page ask, and never touches the speaker's mute.
					expect(sends).toBe(before.sends);
					expect(speakerMuted).toBe(before.speakerMuted);
				}
			}
			// Everything the guest still has to say is said: the page asks for nothing more.
			const asked = sends;
			for (const event of due.sort((a, b) => a.at - b.at)) {
				if (event.kind === "codec") apply(codecWrite(state, (attenuation(event.level) << 8) | attenuation(event.level), event.at));
				else apply(mixerMessage(state, { source: event.source, master: event.level, lines: 6 }));
			}
			expect(sends).toBe(asked);
			// A request goes out only for the visitor's own move of the slider or of the switch.
			expect(sends).toBeLessThanOrEqual(visitorMoves);
		}
	});
});

describe("switches", () => {
	const page = ["clip", "file", "time", "display", "audio"];

	test("a token belongs to its channel whatever its shape", () => {
		expect(["audio", "audio.2", "audio-set", "log"].map(channelOf)).toEqual(["audio", "audio", "audio", "log"]);
	});

	test("the defaults: clipboard and audio follow the page, system and log and power are on", () => {
		expect(initialSwitches(page)).toEqual({ clip: true, audio: true, sys: true, power: true });
		expect(initialSwitches(["file", "time", "display"])).toEqual({ clip: false, audio: false, sys: true, power: true });
	});

	test("the first hello lists the defaults: sys, log and power join the page's list", () => {
		expect(switchBoard(page).tokens()).toEqual([...page, "sys", "log", "power"]);
	});

	test("a group that is off loses every token of its channels, and gets them back", () => {
		const board = switchBoard(["clip", "audio", "audio.2", "audio-set", "file"]);
		expect(board.set("audio", false)).toEqual(["clip", "file", "sys", "log", "power"]);
		expect(board.set("sys", false)).toEqual(["clip", "file", "power"]);
		expect(board.set("audio", true)).toEqual(["clip", "audio", "audio.2", "audio-set", "file", "power"]);
		expect(board.set("sys", true)).toEqual(["clip", "audio", "audio.2", "audio-set", "file", "sys", "log", "power"]);
	});

	test("capsFor keeps tokens of no group and drops a switched-off clipboard", () => {
		expect(capsFor(["clip", "clip-html", "file", "time"], { clip: false, audio: true, sys: false, power: false })).toEqual(["file", "time"]);
	});

	test("a switch turned off and on any number of times does not grow the list", () => {
		const board = switchBoard(page);
		for (let n = 0; n < 20; n++) {
			board.set("power", false);
			board.set("power", true);
		}
		expect(board.tokens()).toEqual([...page, "sys", "log", "power"]);
	});

	test("every switch off leaves only the tokens no group controls", () => {
		const board = switchBoard(page);
		for (const key of ["clip", "audio", "sys", "power"]) board.set(key, false);
		expect(board.tokens()).toEqual(["file", "time", "display"]);
	});
});

describe("the panel's timers and the log", () => {
	const fake = () => {
		const live = new Set();
		let next = 0;
		return {
			live,
			setInterval: () => (live.add(++next), next),
			clearInterval: (id) => live.delete(id),
		};
	};

	test("the poller asks at once, runs one timer while open and none otherwise", () => {
		const timers = fake();
		let asked = 0;
		const poller = createPoller(() => asked++, timers);
		expect(timers.live.size).toBe(0);
		poller.open();
		poller.open();
		expect(asked).toBe(1);
		expect(timers.live.size).toBe(1);
		expect(poller.running).toBe(true);
		poller.close();
		poller.close();
		expect(timers.live.size).toBe(0);
	});

	test("the log keeps lines only while open, at most the last 200, and forgets them on close", () => {
		const log = createLogBuffer(3);
		expect(log.push({ level: "info", src: "audio", text: "early" })).toBe(false);
		log.open();
		for (const text of ["a", "b", "c", "d"]) log.push({ level: "info", src: "sys", text });
		expect(log.lines.map((line) => line.text)).toEqual(["b", "c", "d"]);
		log.close();
		expect(log.lines).toEqual([]);
	});

	test("the level filter shows the least severe line asked for and the rest", () => {
		const log = createLogBuffer();
		log.open();
		log.push({ level: "info", src: "hostlink", text: "hello" });
		log.push({ level: "warn", src: "audio", text: "no mixer" });
		log.push({ level: "error", src: "", text: "boom" });
		expect(log.text("error")).toBe("error -: boom");
		expect(log.text("warn").split("\n")).toEqual(["warn  audio: no mixer", "error -: boom"]);
		expect(log.text("info").split("\n").length).toBe(3);
	});
});

describe("the words", () => {
	test("uptime and offset", () => {
		expect(uptimeText(42)).toBe("42 s");
		expect(uptimeText(185)).toBe("3 min");
		expect(uptimeText(3 * 3600 + 5 * 60)).toBe("3 h 05 min");
		expect(uptimeText(2 * 86400 + 3600)).toBe("2 d 1 h 00 min");
		expect(offsetText(300)).toBe("−5");
		expect(offsetText(-330)).toBe("+5:30");
		expect(offsetText(0)).toBe("±0");
	});

	test("the System row reads from one sys info", () => {
		const lines = systemLines({ os: "ReactOS 0.4.16", app: "HLSYS/0.1.0", uptime: 125, memtotal: 262144, memfree: 131072, clock: "2026-11-28T09:30:00", bias: 300, zone: "EST" });
		expect(lines).toEqual(["ReactOS 0.4.16, HLSYS/0.1.0", "up 2 min", "memory 128 of 256 MiB free", "no processor figure", "clock 2026-11-28 09:30:00, UTC−5 EST"]);
		expect(systemLines({ os: "x", app: "y", uptime: 1, memtotal: 1024, memfree: 1024, idle: 97, clock: "2026-01-01T00:00:00", bias: 0 })[3]).toBe("idle 97 %");
	});

	test("the PC sound row: devices, then the mixer", () => {
		expect(describePc(null, null)).toBe(T.noHelper);
		const devices = { outname: "AC'97 Audio", driver: "5.1.2600.0", out: 1, in: 1 };
		expect(describePc(devices, null)).toBe("AC'97 Audio (driver 5.1.2600.0), 1 out, 1 in. ");
		expect(describePc(devices, { lines: 0 })).toContain("its Volume Control is empty");
		expect(describePc(devices, { lines: 6, master: 80, mastermute: true, wave: 100 })).toBe("AC'97 Audio (driver 5.1.2600.0), 1 out, 1 in. mixer: master 80 percent (muted), wave 100 percent, 6 lines.");
	});

	test("fill puts values in {names}", () => {
		expect(fill("{n} of {total}", { n: 1, total: 2 })).toBe("1 of 2");
	});
});
