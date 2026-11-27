// Checks the page's microphone (SITE-55) in headless Chromium with its fake audio capture (a looped 1 kHz tone made here), or in Firefox with its fake
// device: the page never asks for the microphone until the Microphone button is pressed; a guest recording at 8,000, 11,025, 16,000, 22,050, 32,000,
// 44,100 and 48,000 Hz finds the tone at the right frequency and level with no dropped or repeated frame; silence with the microphone off; every track
// ended after Stop, Reset, Shut Down, a page hide and a second press; a refusal, no device and an insecure page say why; and no request carries anything.
// The guest is a bare BIOS (no disk): the check drives the card's PCM-in registers itself, as a driver would, in memory the guest is not using.
// Usage: BASE=http://127.0.0.1:8161 [BROWSER=firefox] bun check-mic.ts OUTDIR     (set DISPLAY to a harness Xvfb and unset WAYLAND_DISPLAY)
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import fs from "node:fs";
import path from "node:path";
import { chromium, firefox, type Browser, type Page } from "playwright-core";
import { BASE, CHROMIUM, save, sleep } from "./lib.ts";

const outDir = process.argv[2];
if (!outDir) throw new Error("usage: check-mic.ts OUTDIR");
const isFirefox = process.env.BROWSER === "firefox";
const failures: string[] = [];
const expect = (ok: boolean, message: string) => {
	if (!ok) {
		failures.push(message);
		console.error("FAIL " + message);
	} else console.log("ok   " + message);
};
const report: Record<string, unknown> = {};

// A looped 1 kHz tone at half scale, 48 kHz mono 16-bit, a minute: a whole number of periods. Chromium's fake device leaves a gap of about 1.7 ms where the file loops, so a recording that meets one is repeated.
const toneFile = path.join(outDir, "fake-microphone-1khz.wav");
function writeTone() {
	const rate = 48000;
	const n = rate * 60;
	const buf = Buffer.alloc(44 + n * 2);
	buf.write("RIFF", 0);
	buf.writeUInt32LE(36 + n * 2, 4);
	buf.write("WAVEfmt ", 8);
	buf.writeUInt32LE(16, 16);
	buf.writeUInt16LE(1, 20);
	buf.writeUInt16LE(1, 22);
	buf.writeUInt32LE(rate, 24);
	buf.writeUInt32LE(rate * 2, 28);
	buf.writeUInt16LE(2, 32);
	buf.writeUInt16LE(16, 34);
	buf.write("data", 36);
	buf.writeUInt32LE(n * 2, 40);
	for (let i = 0; i < n; i++) buf.writeInt16LE(Math.round(16384 * Math.sin((2 * Math.PI * 1000 * i) / rate)), 44 + i * 2);
	save(outDir, "fake-microphone-1khz.wav", buf);
}
writeTone();

interface Opened {
	browser: Browser;
	page: Page;
	requests: { url: string; method: string; post: string | null }[];
	sockets: string[];
}

async function open(flags: { fakeDevice: boolean; fakeUi: boolean }, secure = true): Promise<Opened> {
	const browser = isFirefox
		? await firefox.launch({ headless: true, firefoxUserPrefs: { "media.navigator.streams.fake": flags.fakeDevice, "media.navigator.permission.disabled": flags.fakeUi } })
		: await chromium.launch({
				executablePath: CHROMIUM,
				headless: true,
				args: ["--no-first-run", ...(flags.fakeDevice ? ["--use-fake-device-for-media-stream", `--use-file-for-fake-audio-capture=${toneFile}`] : []), ...(flags.fakeUi ? ["--use-fake-ui-for-media-stream"] : [])],
			});
	const context = await browser.newContext({ viewport: { width: 1280, height: 1000 }, serviceWorkers: "block" });
	await context.addInitScript({ path: new URL("./inpage.js", import.meta.url).pathname });
	await context.addInitScript((isSecure: boolean) => {
		// Count the asks and keep every stream, so the check can see what was asked for and that every track ended.
		(window as any).__asks = 0;
		(window as any).__streams = [];
		const md = navigator.mediaDevices;
		if (md && md.getUserMedia) {
			const real = md.getUserMedia.bind(md);
			md.getUserMedia = async (c?: MediaStreamConstraints) => {
				(window as any).__asks++;
				const stream = await real(c);
				(window as any).__streams.push(stream);
				return stream;
			};
		}
		if (!isSecure) Object.defineProperty(window, "isSecureContext", { value: false });
	}, secure);
	const page = await context.newPage();
	const requests: Opened["requests"] = [];
	const sockets: string[] = [];
	page.on("request", (r) => requests.push({ url: r.url(), method: r.method(), post: r.postData() }));
	page.on("websocket", (w) => sockets.push(w.url()));
	page.on("pageerror", (e) => failures.push("page error: " + String(e)));
	return { browser, page, requests, sockets };
}

const cold = `${BASE}/try-reactos/?manifest=none&state=&disk=`;
const gum = (page: Page) => page.evaluate("window.__asks") as Promise<number>;
const tracks = (page: Page) =>
	page.evaluate(() => {
		const all = ((window as any).__streams as MediaStream[]).flatMap((s) => s.getTracks());
		return { total: all.length, live: all.filter((t) => t.readyState === "live").length };
	});
const ui = (page: Page) =>
	page.evaluate(() => {
		const b = document.querySelector("[data-mic]") as HTMLButtonElement;
		const live = document.querySelector("[data-mic-live]") as HTMLElement;
		const note = document.querySelector("[data-mic-note]") as HTMLElement;
		return { disabled: b.disabled, pressed: b.getAttribute("aria-pressed"), title: b.title, liveShown: !live.hidden, liveText: live.textContent ?? "", note: note.hidden ? "" : (note.textContent ?? "") };
	});

/** Drives the card's PCM-in channel like a driver: a list of buffers, the rate, the gain, run; a handler taking each buffer's interrupt. Returns what it recorded. */
async function record(page: Page, rate: number, seconds: number) {
	const frames = Math.floor(rate / 8);
	const buffers = Math.min(31, Math.round(seconds * 8));
	const run = await page.evaluate(
		async ({ rate, frames, buffers, seconds }) => {
			const h = (window as any).tryReactos.soundHandle;
			const m = h.model;
			const e = (window as any).tryReactos.emulator;
			const base = 0x08000000;
			const dv = new DataView(new ArrayBuffer(buffers * 8));
			for (let i = 0; i < buffers; i++) {
				dv.setUint32(i * 8, base + 0x100000 + i * 0x40000, true);
				dv.setUint32(i * 8 + 4, ((frames * 2) | 0x80000000) >>> 0, true);
				e.v86.cpu.write_blob(new Uint8Array(frames * 4), base + 0x100000 + i * 0x40000); // cleared: a buffer the card never wrote shows
			}
			e.v86.cpu.write_blob(new Uint8Array(dv.buffer), base);
			m.namWrite(0x2a, 2, 1);
			m.namWrite(0x32, 2, rate);
			m.namWrite(0x1a, 2, 0);
			m.namWrite(0x1c, 2, 0);
			const before = { ...m.counters };
			// Tap the raw microphone blocks: a run of exact zeros in them is the fake device's own gap (its file loops), not the card's.
			const w = window as any;
			w.__zeroRuns = 0;
			w.__rawSamples = 0;
			if (!m.__tapped) {
				const real = m.feedCapture.bind(m);
				m.feedCapture = (samples: Float32Array, count: number, micRate: number) => {
					let run = 0;
					for (let i = 0; i < count; i++) {
						if (samples[i] === 0) run++;
						else {
							if (run >= 16) w.__zeroRuns++;
							run = 0;
						}
					}
					if (run >= 16) w.__zeroRuns++;
					w.__rawSamples += count;
					return real(samples, count, micRate);
				};
				m.__tapped = true;
			}
			m.nabmWrite(0x0b, 1, 2); // reset the channel (control register, RR)
			m.nabmWrite(0x00, 4, base);
			m.nabmWrite(0x05, 1, buffers - 1);
			// The guest's interrupt handler, every 5 ms.
			const isr = setInterval(() => m.nabmWrite(0x06, 2, 0x1c), 5);
			const t0 = performance.now();
			m.nabmWrite(0x0b, 1, 0x11);
			while (m.recording && performance.now() - t0 < seconds * 1000 + 6000) await new Promise((r) => setTimeout(r, 20));
			clearInterval(isr);
			const elapsed = performance.now() - t0;
			const after = { ...m.counters };
			const out = [];
			for (let i = 0; i < buffers; i++) out.push(e.read_memory(base + 0x100000 + i * 0x40000, frames * 4));
			const all = new Int16Array(buffers * frames * 2);
			out.forEach((b, i) => all.set(new Int16Array(b.buffer.slice(b.byteOffset, b.byteOffset + b.length)), i * frames * 2));
			let bin = "";
			const bytes = new Uint8Array(all.buffer);
			for (let i = 0; i < bytes.length; i += 32768) bin += String.fromCharCode.apply(null, bytes.subarray(i, i + 32768) as any);
			return { zeroRuns: w.__zeroRuns as number, rawSamples: w.__rawSamples as number, elapsed, frames: after.capFrames - before.capFrames, silent: after.capSilentFrames - before.capSilentFrames, dropped: after.capDropped - before.capDropped, buffers: after.capBuffers - before.capBuffers, adc: m.adcRate, b64: btoa(bin) };
		},
		{ rate, frames, buffers, seconds },
	);
	return { ...run, samples: new Int16Array(Buffer.from(run.b64, "base64").buffer.slice(0)), wantFrames: frames * buffers };
}

/** Frequency, amplitude and the steadiest phase of the strongest tone in the left channel: a phase step shows a dropped or repeated frame. */
function analyse(samples: Int16Array, rate: number) {
	const n = samples.length / 2;
	const x = new Float64Array(n);
	for (let i = 0; i < n; i++) x[i] = (samples[2 * i] as number) / 32768;
	const from = Math.floor(n * 0.08);
	const to = Math.floor(n * 0.92);
	const project = (f: number, a: number, b: number) => {
		let s = 0;
		let c = 0;
		for (let i = a; i < b; i++) {
			s += (x[i] as number) * Math.sin((2 * Math.PI * f * i) / rate);
			c += (x[i] as number) * Math.cos((2 * Math.PI * f * i) / rate);
		}
		return { s, c, mag: Math.hypot(s, c) };
	};
	let lo = 900;
	let hi = 1100;
	for (let k = 0; k < 40; k++) {
		const m1 = lo + (hi - lo) / 3;
		const m2 = hi - (hi - lo) / 3;
		if (project(m1, from, to).mag < project(m2, from, to).mag) lo = m1;
		else hi = m2;
	}
	const frequency = (lo + hi) / 2;
	const whole = project(frequency, from, to);
	const amplitude = (2 * whole.mag) / (to - from);
	// The phase in 10 ms windows, against its straight-line trend.
	const win = Math.max(8, Math.floor(rate / 100));
	const phases: number[] = [];
	for (let a = from; a + win <= to; a += win) {
		const p = project(frequency, a, a + win);
		phases.push(Math.atan2(p.c, p.s) - (2 * Math.PI * frequency * 0) );
	}
	const unwrapped: number[] = [];
	phases.forEach((p, i) => {
		if (i === 0) return void unwrapped.push(p);
		let d = p - (phases[i - 1] as number);
		while (d > Math.PI) d -= 2 * Math.PI;
		while (d < -Math.PI) d += 2 * Math.PI;
		unwrapped.push((unwrapped[i - 1] as number) + d);
	});
	// Straight-line fit of the unwrapped phase; the largest departure is the steadiness.
	const m = unwrapped.length;
	let sx = 0, sy = 0, sxx = 0, sxy = 0;
	unwrapped.forEach((y, i) => { sx += i; sy += y; sxx += i * i; sxy += i * y; });
	const slope = (m * sxy - sx * sy) / (m * sxx - sx * sx);
	const icpt = (sy - slope * sx) / m;
	let worst = 0;
	unwrapped.forEach((y, i) => (worst = Math.max(worst, Math.abs(y - (icpt + slope * i)))));
	let rest = 0, total = 0;
	for (let i = from; i < to; i++) {
		const fit = whole.s * Math.sin((2 * Math.PI * frequency * i) / rate) * (2 / (to - from)) + whole.c * Math.cos((2 * Math.PI * frequency * i) / rate) * (2 / (to - from));
		rest += ((x[i] as number) - fit) ** 2;
		total += (x[i] as number) ** 2;
	}
	return { frequency, amplitude, phaseWander: worst, residualDb: 10 * Math.log10(rest / total) };
}

const RATES = [8000, 11025, 16000, 22050, 32000, 44100, 48000];

// ---------------------------------------------------------------------------------------------------------------------------------------------
{
	const o = await open({ fakeDevice: true, fakeUi: true });
	const { page } = o;
	try {
		await page.goto(cold);
		await page.waitForFunction("document.querySelector('[data-build]').textContent !== 'Build: loading'", null, { timeout: 15000 });
		let u = await ui(page);
		expect(u.disabled && u.pressed === "false" && !u.liveShown, `before Start: the Microphone button is disabled and off ("${u.title}")`);
		save(outDir, "mic-before-start.png", await page.screenshot({ clip: { x: 0, y: 330, width: 1280, height: 140 } }));
		await page.click("[data-start]");
		await page.waitForFunction("window.tryReactos.soundHandle && window.tryReactos.soundHandle.stats().registered", null, { timeout: 60000 });
		await sleep(3000);
		u = await ui(page);
		expect(!u.disabled && u.pressed === "false" && !u.liveShown, "after Start: the button is ready, the microphone is off and no indicator shows");
		expect((await gum(page)) === 0, `after Start and a card that is running: the browser was asked for the microphone ${await gum(page)} times`);
		const idleRecord = await record(page, 22050, 0.5);
		expect((await gum(page)) === 0, "a guest recording with the microphone off does not ask for it either");
		expect(idleRecord.silent >= idleRecord.frames && idleRecord.frames === idleRecord.wantFrames && idleRecord.samples.every((v) => v === 0), `the guest records ${idleRecord.frames} of ${idleRecord.wantFrames} frames, ${idleRecord.silent} of them fed as silence, ${idleRecord.samples.filter((v) => v !== 0).length} non-zero samples, with the microphone off`);
		expect(Math.abs(idleRecord.elapsed - 500) < 250, `the silence arrives in real time (${Math.round(idleRecord.elapsed)} ms for 500 ms)`);

		// Turn it on with a real click.
		await page.click("[data-mic]");
		await page.waitForFunction("window.tryReactos.soundHandle.micState() === 'on'", null, { timeout: 10000 });
		u = await ui(page);
		expect((await gum(page)) === 1, "the press asked the browser once");
		expect(u.pressed === "true" && u.liveShown && /Microphone on/.test(u.liveText), `the indicator reads "${u.liveText}" and the button is pressed`);
		save(outDir, "mic-on-indicator.png", await page.screenshot({ clip: { x: 0, y: 330, width: 1280, height: 140 } }));

		const rows: any[] = [];
		for (const rate of RATES) {
			let r = await record(page, rate, 2);
			let retries = 0;
			while (r.zeroRuns > 0 && retries < 5) {
				retries++;
				r = await record(page, rate, 2);
			}
			const a = analyse(r.samples, rate);
			rows.push({ rate, retriesForTheFakeDevicesLoopGap: retries, rawMicSamples: r.rawSamples, adc: r.adc, frames: r.frames, want: r.wantFrames, silent: r.silent, dropped: r.dropped, elapsedMs: Math.round(r.elapsed), frequency: Math.round(a.frequency * 1000) / 1000, amplitude: Math.round(a.amplitude * 10000) / 10000, phaseWanderRad: Math.round(a.phaseWander * 10000) / 10000, residualDb: Math.round(a.residualDb * 10) / 10 });
			expect(r.adc === rate && r.frames === r.wantFrames && r.dropped === 0, `${rate} Hz: the guest's ADC rate is ${r.adc} and ${r.frames} of ${r.wantFrames} frames were recorded, ${r.dropped} dropped`);
			expect(r.silent <= Math.round(rate * 0.1), `${rate} Hz: ${r.silent} frames of silence (the microphone was already on; at most 100 ms of an earlier gap)`);
			if (!isFirefox) {
				expect(Math.abs(a.frequency - 1000) < 1, `${rate} Hz: the tone is at ${a.frequency.toFixed(3)} Hz`);
				expect(Math.abs(a.amplitude - 0.5) < 0.02, `${rate} Hz: the tone's level is ${a.amplitude.toFixed(4)} (0.5 in)`);
				expect(a.phaseWander < 0.05, `${rate} Hz: the phase wanders ${a.phaseWander.toFixed(4)} rad (a dropped or repeated frame would be at least ${(2 * Math.PI * 1000 / rate).toFixed(3)})`);
				expect(a.residualDb < -40, `${rate} Hz: THD+N ${a.residualDb.toFixed(1)} dB`);
			}
			if (rate === 22050 || rate === 48000) save(outDir, `mic-recorded-${rate}.wav`, wavBuffer(r.samples, rate));
		}
		report.rates = rows;

		// A second press turns it off and ends the track; the guest records silence again.
		await page.click("[data-mic]");
		await sleep(300);
		let t = await tracks(page);
		u = await ui(page);
		expect(t.total >= 1 && t.live === 0, `a second press: ${t.live} of ${t.total} tracks live`);
		expect(u.pressed === "false" && !u.liveShown, "a second press: the indicator is gone");
		const quiet = await record(page, 22050, 0.5);
		expect(quiet.samples.every((v) => v === 0) && quiet.frames === quiet.wantFrames, "after the second press the guest records exact silence again");

		// Stop, Reset and Shut Down each end the track.
		for (const how of ["stop", "reset", "shutdown"] as const) {
			await page.click("[data-mic]");
			await page.waitForFunction("window.tryReactos.soundHandle && window.tryReactos.soundHandle.micState() === 'on'", null, { timeout: 10000 }).catch(() => {});
			const on = (await tracks(page)).live;
			if (how === "stop") await page.click("[data-stop]");
			else if (how === "reset") await page.click("[data-reset]");
			else await page.evaluate("window.tryReactos.emulator.stop()");
			await sleep(1500);
			t = await tracks(page);
			u = await ui(page);
			expect(on >= 1 && t.live === 0, `${how}: ${on} track(s) live before, ${t.live} after`);
			expect(u.pressed === "false" && !u.liveShown, `${how}: the indicator is gone`);
			if (how === "stop") await page.click("[data-start]");
			if (how === "shutdown") await page.click("[data-start]");
			await page.waitForFunction("window.tryReactos.soundHandle && window.tryReactos.soundHandle.stats().registered", null, { timeout: 60000 });
			await sleep(1000);
			expect((await ui(page)).pressed === "false", `${how}: the next machine starts with the microphone off`);
		}

		// Leaving the page.
		await page.click("[data-mic]");
		await page.waitForFunction("window.tryReactos.soundHandle.micState() === 'on'", null, { timeout: 10000 });
		await page.evaluate(() => window.dispatchEvent(new Event("pagehide")));
		await sleep(500);
		t = await tracks(page);
		expect(t.live === 0, `page hide: ${t.live} tracks live`);

		// A reload starts with it off, and nothing is stored.
		await page.reload();
		await page.waitForFunction("document.querySelector('[data-build]').textContent !== 'Build: loading'", null, { timeout: 15000 });
		u = await ui(page);
		expect(u.pressed === "false" && (await gum(page)) === 0, "a reload starts with the microphone off and no ask");
		const stored = await page.evaluate(() => JSON.stringify({ local: { ...localStorage }, session: { ...sessionStorage } }));
		expect(!/mic/i.test(stored), `nothing about the microphone is stored in the browser (${stored})`);

		// Nothing was sent anywhere with the microphone's audio.
		const odd = o.requests.filter((r) => r.method !== "GET" && r.method !== "HEAD");
		const foreign = o.requests.filter((r) => !r.url.startsWith(BASE) && !r.url.startsWith("data:") && !r.url.startsWith("blob:"));
		const withBody = o.requests.filter((r) => r.post);
		report.requests = { total: o.requests.length, nonGet: odd.length, foreignOrigin: foreign.length, withBody: withBody.length, websockets: o.sockets.length };
		expect(odd.length === 0 && foreign.length === 0 && withBody.length === 0 && o.sockets.length === 0, `network: ${o.requests.length} requests, ${odd.length} not GET or HEAD, ${foreign.length} to another origin, ${withBody.length} with a body, ${o.sockets.length} web sockets`);
	} finally {
		await o.browser.close();
	}
}

function wavBuffer(samples: Int16Array, rate: number): Buffer {
	const out = Buffer.alloc(44 + samples.length * 2);
	out.write("RIFF", 0);
	out.writeUInt32LE(36 + samples.length * 2, 4);
	out.write("WAVEfmt ", 8);
	out.writeUInt32LE(16, 16);
	out.writeUInt16LE(1, 20);
	out.writeUInt16LE(2, 22);
	out.writeUInt32LE(rate, 24);
	out.writeUInt32LE(rate * 4, 28);
	out.writeUInt16LE(4, 32);
	out.writeUInt16LE(16, 34);
	out.write("data", 36);
	out.writeUInt32LE(samples.length * 2, 40);
	Buffer.from(samples.buffer, samples.byteOffset, samples.byteLength).copy(out, 44);
	return out;
}

// A refusal, no microphone, and an insecure page each say why, and the guest still records.
for (const [name, flags, secure, want] of [
	["refused", { fakeDevice: true, fakeUi: false }, true, isFirefox ? /blocked|Waiting for your browser/ : /blocked/],
	["no microphone", { fakeDevice: false, fakeUi: true }, true, /No microphone|cannot give|could not be started/],
	["insecure page", { fakeDevice: true, fakeUi: true }, false, /HTTPS/],
] as const) {
	// Firefox on this machine finds a real (null) input even with its fake device off, so there is no "no microphone" to test there; its permission
	// question stays open in headless mode instead of being refused, and the page says so.
	if (isFirefox && name === "no microphone") continue;
	const o = await open(flags, secure);
	try {
		await o.page.goto(cold);
		await o.page.waitForFunction("document.querySelector('[data-build]').textContent !== 'Build: loading'", null, { timeout: 15000 });
		await o.page.click("[data-start]");
		await o.page.waitForFunction("window.tryReactos.soundHandle && window.tryReactos.soundHandle.stats().registered", null, { timeout: 60000 });
		await sleep(2000);
		await o.page.click("[data-mic]");
		await sleep(1500);
		const u = await ui(o.page);
		report[name] = { state: await o.page.evaluate("window.tryReactos.soundHandle.micState()"), note: u.note };
		expect(want.test(u.note) && u.pressed === "false" && !u.liveShown, `${name}: the page says "${u.note}"`);
		if (name === "refused") save(outDir, "mic-refused-note.png", await o.page.screenshot({ clip: { x: 0, y: 330, width: 1280, height: 160 } }));
		const r = await record(o.page, 22050, 0.5);
		expect(r.frames === r.wantFrames && r.samples.every((v) => v === 0), `${name}: the guest still records, and hears silence`);
		expect((await tracks(o.page)).live === 0, `${name}: no track is live`);
	} finally {
		await o.browser.close();
	}
}

fs.writeFileSync(path.join(outDir, `check-mic-${isFirefox ? "firefox" : "chromium"}.json`), JSON.stringify({ ...report, failures }, null, 1));
console.log(JSON.stringify({ ...report, failures }, null, 1));
if (failures.length > 0) {
	console.error(failures.join("\n"));
	process.exit(1);
}
