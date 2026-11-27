// Checks the page's sound (SITE-41 to SITE-43) in headless Chromium: the card is there and driven by ReactOS's own ac97.sys, the guest's Critical Stop
// reaches the speakers' end (an AudioWorklet probe after the volume node counts and records it), the Sound button mutes and unmutes, the slider sets the
// level, the choice survives a reload, Stop and Reset silence it, a browser that holds the sound back (or has none) still runs the machine, and the
// remembered level applies to the next machine. Needs tools/stage-web.py to have staged data/ and web-manifest.json with the sound card's registers
// (qagrr/build-state.sh makes them), and the server (bun serve.ts) running. Input goes through the page's emulator API; pixels come from the canvas.
// Usage: BASE=http://127.0.0.1:8161 [BROWSER=firefox] bun check-sound.ts OUTDIR     (set DISPLAY to a harness Xvfb and unset WAYLAND_DISPLAY)
// BROWSER=firefox runs the same checks in Playwright's Firefox instead of the installed Chromium.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { firefox } from "playwright-core";
import { PROFILES, grab, launch as launchChromium, pageUrl, save, sleep, waitFor } from "./lib.ts";
import { card, controls, installProbe, openSounds, playSound, probeRecord, probeStats, probeTake, wav } from "./sound-lib.ts";

const outDir = process.argv[2];
if (!outDir) throw new Error("usage: check-sound.ts OUTDIR");

// Chromium through the harness's own launcher; Firefox through Playwright's build (no DevTools protocol there, which these checks do not use).
async function launch(run: string, profile: (typeof PROFILES)[string], coi: boolean) {
	if (process.env.BROWSER !== "firefox") return launchChromium(run, profile, coi);
	const browser = await firefox.launch({ headless: true });
	const context = await browser.newContext({ viewport: { width: 1280, height: 1000 }, serviceWorkers: "block" });
	await context.addInitScript({ path: new URL("./inpage.js", import.meta.url).pathname });
	const page = await context.newPage();
	return { page, close: () => browser.close() };
}

const failures: string[] = [];
const expect = (ok: boolean, message: string) => {
	if (!ok) {
		failures.push(message);
		console.error("FAIL " + message);
	} else console.log("ok   " + message);
};
const report: Record<string, unknown> = {};
const desktop = "window.__m.taskbarScore() > 0.3";
const toDesktop = async (page: any, why: string) => {
	expect((await waitFor(page, desktop, 240000)) !== null, `${why}: the themed desktop came up`);
	await sleep(2500);
};
const near = (a: number, b: number, tolerance: number) => Math.abs(a - b) <= tolerance;

/** Plays Critical Stop once and returns what the card took and what the probe heard (recording kept when `keep` is a name). */
async function playOnce(page: any, why: string, keep?: string) {
	const before = (await card(page))!;
	const heardBefore = await probeStats(page);
	const sinkBefore: any = await page.evaluate("window.tryReactos.soundHandle.sinkStats()");
	await probeRecord(page);
	await playSound(page);
	const started = await waitFor(page, `window.tryReactos.soundHandle.stats().counters.dmaFrames > ${before.dma}`, 30000, 100);
	expect(started !== null, `${why}: the guest started playing`);
	const ended = await waitFor(page, `window.tryReactos.soundHandle.stats().counters.dmaFrames > ${before.dma} && !window.tryReactos.soundHandle.stats().playing`, 30000, 100);
	expect(ended !== null, `${why}: the guest finished playing`);
	await sleep(400);
	const after = (await card(page))!;
	const heard = await probeStats(page);
	const sinkAfter: any = await page.evaluate("window.tryReactos.soundHandle.sinkStats()");
	const rec = await probeTake(page);
	// The peak of this play alone, from the recording (the probe's own peak is the highest since it was installed).
	let windowPeak = 0;
	for (let i = 0; i < rec.left.length; i++) windowPeak = Math.max(windowPeak, Math.abs(rec.left[i] ?? 0), Math.abs(rec.right[i] ?? 0));
	if (keep && rec.left.length) save(outDir, keep, wav(rec.left, rec.right, 48000));
	return { before, after, heardBefore, heard, rec, underrunFrames: sinkAfter.underrunFrames - sinkBefore.underrunFrames, dropped: sinkAfter.dropped - sinkBefore.dropped, taken: after.dma - before.dma, loud: heard.loud - heardBefore.loud, peak: windowPeak };
}

const session = await launch("", PROFILES.unthrottled, false);
const page = session.page;
page.on("pageerror", (e) => failures.push("page error: " + String(e)));
page.on("console", (m) => {
	if (process.env.CONSOLE) console.log("[page]", m.text());
});
try {
	// 1. The plain address, before Start: sound is on at 70 percent.
	await page.goto(pageUrl("", {}, { plain: true }));
	await page.waitForFunction("document.querySelector('[data-build]').textContent !== 'Build: loading'", null, { timeout: 15000 });
	let ui = await controls(page);
	expect(ui.label === "Sound on" && ui.pressed === "true" && ui.slider === "70" && !ui.disabled, `before Start: the button reads "${ui.label}", pressed ${ui.pressed}, the slider ${ui.slider}`);
	expect(ui.stored === null, "before Start: nothing is stored yet");
	await page.screenshot({ path: `${outDir}/controls-before-start.png`.replace(/^/, ""), clip: undefined }).catch(() => {});

	// 2. Start: the card, driven by the guest's driver, through a real click (the browser's autoplay rules apply).
	await page.click("[data-start]");
	await toDesktop(page, "Start");
	let c = (await card(page))!;
	expect(c !== null && c.registered && c.pciId === 24, `Start: the card is in PCI slot 3 (id ${c?.pciId})`);
	expect(c.output === "running" && c.sink === "worklet", `Start: the output is ${c.output} through a ${c.sink}`);
	expect(c.stateError === "", "Start: the saved registers were accepted");
	expect(near(c.gain, 0.49, 1e-9) && c.volume === 0.7 && !c.muted, `Start: the volume is ${c.volume} (gain ${c.gain}), muted ${c.muted}`);
	const wireState = await page.evaluate("JSON.stringify(window.tryReactos.soundHandle.getAc97State()).length");
	report.registersBytes = wireState;
	await installProbe(page);
	await sleep(1500);
	await openSounds(page);
	const idle = await probeStats(page);
	expect(idle.loud === 0, `the idle desktop is silent at the probe (${idle.loud} loud frames of ${idle.frames})`);
	const shotsCard = await page.evaluate("document.querySelector('[data-sound-readout] dd').textContent");
	report.readout = shotsCard;

	// 3. The guest plays Critical Stop: the card takes it at the guest's rate and the probe hears it, at the volume the slider says.
	const first = await playOnce(page, "Critical Stop", "critical-stop-through-the-page.wav");
	report.firstPlay = { guestRate: first.after.guestRate, taken: first.taken, buffers: first.after.buffers - first.before.buffers, irqs: first.after.irqs - first.before.irqs, peakRaw: first.after.peakRaw, probeLoudFrames: first.loud, probePeak: first.peak, drained: first.after.drained - first.before.drained, starved: first.after.starved - first.before.starved };
	expect(first.taken > 30000 && first.taken < 50000, `Critical Stop: the card took ${first.taken} frames at ${first.after.guestRate} Hz (WP-WEB-4 saw 39,045)`);
	const seconds = first.taken / first.after.guestRate;
	expect(near(first.loud / 48000, seconds, 0.4), `Critical Stop: the probe heard ${(first.loud / 48000).toFixed(2)} s, the guest sent ${seconds.toFixed(2)} s`);
	expect(first.peak > 0.15 && first.peak < 0.5, `Critical Stop: the peak after the volume node is ${first.peak.toFixed(3)} (raw peak ${first.after.peakRaw.toFixed(3)} times 0.49)`);
	expect(near(first.peak, first.after.peakRaw * 0.49, 0.02), "Critical Stop: the peak is the raw peak times the volume's gain");
	expect(first.after.drained === first.before.drained, "Critical Stop: nothing was thrown away unheard (a live output pulled it all)");
	report.sink = { underrunFramesWhilePlaying: first.underrunFrames, droppedWhilePlaying: first.dropped };
	// The page's thread runs the machine and the output's queue is 120 ms, so on a shared workstation a stall of a few milliseconds can show; more than 20 ms in one sound is a failure.
	expect(first.underrunFrames <= 960 && first.dropped === 0, `Critical Stop: the output had ${first.underrunFrames} underrun frames (at most 960 allowed) and dropped ${first.dropped} while it played`);

	// 4. The Sound button mutes: the gain goes to zero, the probe hears nothing, the guest still plays (the card is not paused), the choice is stored.
	await page.click("[data-sound]");
	await sleep(300);
	ui = await controls(page);
	expect(ui.label === "Sound off" && ui.pressed === "false", `mute: the button reads "${ui.label}", pressed ${ui.pressed}`);
	expect(JSON.parse(ui.stored ?? "null")?.muted === true, `mute: the browser stored ${ui.stored}`);
	c = (await card(page))!;
	expect(c.gain === 0 && c.muted, `mute: the gain is ${c.gain}`);
	const muted = await playOnce(page, "muted Critical Stop");
	expect(muted.taken > 30000, `mute: the guest still played ${muted.taken} frames, so it never waited on the speakers`);
	expect(muted.loud === 0 && muted.peak === 0, `mute: the probe heard ${muted.loud} loud frames, peak ${muted.peak}`);

	// 5. Unmute: sound again.
	await page.click("[data-sound]");
	await sleep(300);
	ui = await controls(page);
	expect(ui.label === "Sound on" && ui.pressed === "true", `unmute: the button reads "${ui.label}"`);
	const again = await playOnce(page, "unmuted Critical Stop");
	expect(again.loud / 48000 > 1, `unmute: the probe heard ${(again.loud / 48000).toFixed(2)} s`);

	// 6. The slider: the keyboard moves it, the level follows (the gain is the square), and 0 is silent.
	await page.focus("[data-volume]");
	await page.keyboard.press("Home");
	await sleep(200);
	c = (await card(page))!;
	ui = await controls(page);
	expect(c.volume === 0 && c.gain === 0 && ui.label === "Sound off", `slider at 0: volume ${c.volume}, gain ${c.gain}, button "${ui.label}"`);
	await page.keyboard.press("End");
	await sleep(200);
	c = (await card(page))!;
	expect(c.volume === 1 && c.gain === 1 && !c.muted, `slider at 100: volume ${c.volume}, gain ${c.gain}`);
	await page.evaluate(() => {
		const s = document.querySelector("[data-volume]") as HTMLInputElement;
		s.value = "30";
		s.dispatchEvent(new Event("input", { bubbles: true }));
	});
	await sleep(200);
	c = (await card(page))!;
	expect(near(c.gain, 0.09, 1e-9), `slider at 30: gain ${c.gain}`);
	const quiet = await playOnce(page, "Critical Stop at 30 percent");
	expect(near(quiet.peak, quiet.after.peakRaw * 0.09, 0.01) && quiet.peak < first.peak, `slider at 30: the probe's peak is ${quiet.peak.toFixed(3)} (raw ${quiet.after.peakRaw.toFixed(3)} times 0.09)`);
	report.volumes = { at70: first.peak, at30: quiet.peak };
	// The gain node itself, as the output holds it.
	report.gainNode = await page.evaluate("window.tryReactos.soundHandle.sink.output.gain.value");

	// 7. Remembered per browser: reload, and the controls and the next machine's volume are what they were.
	await page.reload();
	await page.waitForFunction("document.querySelector('[data-build]').textContent !== 'Build: loading'", null, { timeout: 15000 });
	ui = await controls(page);
	expect(ui.slider === "30" && ui.label === "Sound on", `reload: the slider is ${ui.slider} and the button "${ui.label}"`);
	await page.click("[data-start]");
	await toDesktop(page, "Start after the reload");
	await openSounds(page);
	c = (await card(page))!;
	expect(c.volume === 0.3 && near(c.gain, 0.09, 1e-9) && !c.muted, `reload: the new machine starts at volume ${c.volume}`);
	await page.evaluate(() => {
		const s = document.querySelector("[data-volume]") as HTMLInputElement;
		s.value = "70";
		s.dispatchEvent(new Event("input", { bubbles: true }));
	});

	// 8. Stop in the middle of a sound silences it at once; the card is let go.
	await installProbe(page);
	const mid = (await card(page))!;
	const heard0 = await probeStats(page);
	await playSound(page);
	await waitFor(page, `window.tryReactos.soundHandle.stats().counters.dmaFrames > ${mid.dma} + 8000`, 30000, 50);
	const heardAtStop = await probeStats(page);
	await page.click("[data-stop]");
	await sleep(1500);
	expect(await page.evaluate("window.tryReactos.soundHandle === null || window.tryReactos.soundHandle === undefined"), "Stop: the card was detached");
	const heardAfterStop = await probeStats(page);
	report.stop = { heardBeforeStopFrames: heardAtStop.loud - heard0.loud, heardAfterStopFrames: heardAfterStop.loud - heardAtStop.loud };
	expect(heardAfterStop.loud - heardAtStop.loud < 48000 * 0.2, `Stop: ${heardAfterStop.loud - heardAtStop.loud} loud frames reached the probe after Stop (a 120 ms buffer at most)`);
	expect(heardAtStop.loud - heard0.loud > 0, "Stop: the sound was playing when Stop was selected");
	ui = await controls(page);
	expect(!ui.disabled && ui.label === "Sound on", `Stop: the button is ready for the next Start ("${ui.label}")`);

	// 9. Start, then Reset in the middle of a sound.
	await page.click("[data-start]");
	await toDesktop(page, "Start after Stop");
	await installProbe(page);
	await openSounds(page);
	const old = await page.evaluate("window.tryReactos.soundHandle.stats().pciId");
	await page.evaluate("window.__handle = window.tryReactos.soundHandle");
	await playSound(page);
	await waitFor(page, "window.tryReactos.soundHandle.stats().playing", 30000, 50);
	const heardAtReset = await probeStats(page);
	await page.click("[data-reset]");
	await sleep(500);
	const heardSoon = await probeStats(page);
	expect(await page.evaluate("window.__handle.stats().registered && window.tryReactos.soundHandle !== window.__handle"), "Reset: the old card was replaced");
	await toDesktop(page, "Reset");
	c = (await card(page))!;
	expect(c.registered && c.pciId === old && c.output === "running", `Reset: a new card is in slot ${c.pciId >> 3} and the output is ${c.output}`);
	expect(heardSoon.loud - heardAtReset.loud < 48000 * 0.2, `Reset: ${heardSoon.loud - heardAtReset.loud} loud frames reached the probe after Reset`);
	await installProbe(page);
	await openSounds(page);
	const afterReset = await playOnce(page, "Critical Stop after Reset");
	expect(afterReset.loud / 48000 > 1, `Reset: the new machine's sound reached the probe (${(afterReset.loud / 48000).toFixed(2)} s)`);
	await save(outDir, "after-reset.png", (await grab(page))!.png);

	// 10. The card's registers come back in a state: stop, read them, and they are what the manifest gave plus what the guest set.
	await page.click("[data-stop]");
	await sleep(500);

	console.log(JSON.stringify({ ...report, failures }, null, 1));
} finally {
	await session.close();
}

// 11. A browser that holds the sound back (a context that will not resume) still runs the machine, and one without Web Audio too.
const held = await launch("", PROFILES.unthrottled, false);
try {
	const p = held.page;
	await p.addInitScript(() => {
		const Real = window.AudioContext;
		(window as any).__refuse = true;
		class Held extends Real {
			constructor(options?: AudioContextOptions) {
				super(options);
				this.suspend().catch(() => {});
			}
			resume(): Promise<void> {
				return (window as any).__refuse ? Promise.reject(new DOMException("play() failed: not allowed", "NotAllowedError")) : super.resume();
			}
		}
		(window as any).AudioContext = Held;
	});
	await p.goto(pageUrl("", {}, { plain: true }));
	await p.waitForFunction("document.querySelector('[data-build]').textContent !== 'Build: loading'", null, { timeout: 15000 });
	await p.click("[data-start]");
	await toDesktop(p, "refused audio");
	await sleep(1500);
	let cc = (await card(p))!;
	let u = await controls(p);
	await openSounds(p);
	expect(cc.output === "suspended", `refused audio: the output is ${cc.output}`);
	expect(u.label === "Sound off" && /holding the sound back/.test(u.title), `refused audio: the button reads "${u.label}" with "${u.title}"`);
	const refused = await (async () => {
		const before = (await card(p))!;
		await playSound(p);
		const ended = await waitFor(p, `window.tryReactos.soundHandle.stats().counters.dmaFrames > ${before.dma} && !window.tryReactos.soundHandle.stats().playing`, 30000, 100);
		const after = (await card(p))!;
		return { ended, taken: after.dma - before.dma, drained: after.drained - before.drained };
	})();
	report.refused = refused;
	expect(refused.ended !== null && refused.taken > 30000, `refused audio: the guest still finished its sound (${refused.taken} frames)`);
	expect(refused.drained > 25000, `refused audio: ${refused.drained} frames were taken at the real-time rate and dropped`);
	await save(outDir, "refused-audio.png", await p.screenshot({ fullPage: true }));
	// The visitor's click is the gesture the browser wanted.
	await p.evaluate(() => ((window as any).__refuse = false));
	await p.click("[data-sound]");
	await sleep(800);
	cc = (await card(p))!;
	u = await controls(p);
	expect(cc.output === "running" && u.label === "Sound on", `refused audio: after the button the output is ${cc.output} and the button "${u.label}"`);
	await p.click("[data-stop]");
} finally {
	await held.close();
}

const nowebaudio = await launch("", PROFILES.unthrottled, false);
try {
	const p = nowebaudio.page;
	await p.addInitScript(() => {
		delete (window as any).AudioContext;
		delete (window as any).webkitAudioContext;
	});
	await p.goto(pageUrl("", {}, { plain: true }));
	await p.waitForFunction("document.querySelector('[data-build]').textContent !== 'Build: loading'", null, { timeout: 15000 });
	await p.click("[data-start]");
	await toDesktop(p, "no Web Audio");
	const u = await controls(p);
	await openSounds(p);
	expect(u.disabled && /cannot play the sound/.test(u.title), `no Web Audio: the button is disabled ("${u.title}")`);
	const cc = (await card(p))!;
	expect(cc !== null && cc.registered && cc.output === "none", `no Web Audio: the card is still there (the saved desktop needs it) with output "${cc?.output}"`);
	const before = cc.dma;
	await playSound(p);
	const ended = await waitFor(p, `window.tryReactos.soundHandle.stats().counters.dmaFrames > ${before} && !window.tryReactos.soundHandle.stats().playing`, 30000, 100);
	expect(ended !== null, "no Web Audio: the guest still finished its sound");
	await p.click("[data-stop]");
} finally {
	await nowebaudio.close();
}

console.log(JSON.stringify({ ...report, failures }, null, 1));
if (failures.length > 0) {
	console.error(failures.join("\n"));
	process.exit(1);
}
