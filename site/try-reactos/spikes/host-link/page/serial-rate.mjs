// WP-HOST-1's v86 run: boots the QAGRR disk with H.EXE on C:\ in v86 in headless Chromium, drives the guest through the emulator's own keyboard
// API (never the host desktop), and measures the serial port in both directions, the display modes and SetLocalTime. Writes one JSON object per
// step to OUT (default ~/.cache/quill-host-spike/out/v86-results.jsonl) and shots beside it.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// usage: env -u WAYLAND_DISPLAY DISPLAY=:99 node serial-rate.mjs [--steps a,b,c]    (the server: PORT=18186 IMAGES_DIR=... bun harness/serve.ts)
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { chromium } from "playwright-core";

const OUT = process.env.OUT || path.join(os.homedir(), ".cache/quill-host-spike/out");
const BASE = process.env.BASE || "http://127.0.0.1:18186";
const SIZE = 4294967296;
const steps = (process.argv.includes("--steps") ? process.argv[process.argv.indexOf("--steps") + 1] : "boot,ping,display,time,source,echo,blast").split(",");
const sleep = (ms) => new Promise((ok) => setTimeout(ok, ms));
fs.mkdirSync(OUT, { recursive: true });
const record = (step, data) => {
	const line = JSON.stringify({ step, at: new Date().toISOString(), ...data });
	fs.appendFileSync(path.join(OUT, "v86-results.jsonl"), line + "\n");
	console.log(line);
};

const browser = await chromium.launch({
	executablePath: "/usr/bin/chromium",
	headless: true,
	env: { ...process.env, DISPLAY: process.env.DISPLAY || ":99", WAYLAND_DISPLAY: undefined },
	args: ["--no-first-run", "--disable-background-timer-throttling", "--disable-renderer-backgrounding", "--disable-backgrounding-occluded-windows"],
});
const context = await browser.newContext({ viewport: { width: 1100, height: 820 }, serviceWorkers: "block" });
const page = await context.newPage();
page.on("pageerror", (e) => console.log("pageerror", e.message));
const shot = async (name) => page.locator("#screen").screenshot({ path: path.join(OUT, name + ".png") });
const fresh = process.env.FRESH === "1";
const guest = async (command, expect, timeoutMs = 90000) => {
	if (fresh && /h\.exe c /.test(command)) await page.evaluate(() => window.spike.load());
	const from = await page.evaluate(() => window.spike.com2.length);
	await page.evaluate((c) => window.spike.run(c), command);
	if (process.env.SHOTS) {
		await sleep(6000);
		await shot("after-" + command.replace(/[^a-z0-9]+/gi, "-").slice(0, 30));
	}
	return { from, match: expect ? await page.evaluate(([r, t, f]) => window.spike.waitFor(new RegExp(r), t, f).then((m) => (m ? m[0] : null)), [expect, timeoutMs, from]) : null };
};
const logSince = (from) => page.evaluate((f) => window.spike.com2.slice(f).replace(/\r/g, ""), from);

await page.goto(`${BASE}/try-reactos/spikes/host-link/page/serial-rate.html?size=${SIZE}`);
await page.waitForFunction(() => window.spike?.ready, null, { timeout: 30000 });

if (steps.includes("boot")) {
	const t0 = Date.now();
	let ready = false;
	while (Date.now() - t0 < 420000) {
		await sleep(5000);
		// The desktop is up when the taskbar's grey is at the bottom edge and the desktop's own colour fills the middle (Afterglow's dark wallpaper or
		// ReactOS's stock blue).
		const px = await page.evaluate(() => { try { return [window.spike.pixel(300, 592), window.spike.pixel(400, 100), window.spike.screen()]; } catch { return null; } });
		const grey = (p) => p && Math.abs(p[0] - 212) < 12 && Math.abs(p[1] - 208) < 12 && Math.abs(p[2] - 200) < 12;
		if (px && grey(px[0]) && !grey(px[1]) && px[1][3] === 255) { ready = true; break; }
	}
	await sleep(70000); // the desktop shows long before the shell takes keys
	await shot("v86-desktop");
	const stateBytes = await page.evaluate(() => window.spike.save());
	record("boot", { ready, seconds: Math.round((Date.now() - t0) / 1000), stateBytes });
	if (!ready) process.exit(1);
}

if (steps.includes("ping")) {
	const r = await guest("c:\\h.exe p", "PING", 120000);
	record("ping", { matched: r.match, log: await logSince(r.from) });
}

if (steps.includes("display")) {
	let r = await guest("c:\\h.exe m", "MODES count", 120000);
	record("modes", { log: await logSince(r.from), screen: await page.evaluate(() => window.spike.screen()) });
	await page.evaluate(() => (window.spike.events.length = 0));
	r = await guest("c:\\h.exe x 1280 720", "MODE Change", 120000);
	await sleep(4000);
	await shot("v86-mode-1280");
	record("mode-1280", { log: await logSince(r.from), events: await page.evaluate(() => window.spike.events), screen: await page.evaluate(() => window.spike.screen()) });
	await page.evaluate(() => (window.spike.events.length = 0));
	r = await guest("c:\\h.exe x 800 600", "MODE Change", 120000);
	await sleep(4000);
	await shot("v86-mode-800");
	record("mode-800", { log: await logSince(r.from), events: await page.evaluate(() => window.spike.events), screen: await page.evaluate(() => window.spike.screen()) });
	r = await guest("c:\\h.exe x 1024 768", "MODE Change", 120000);
	await sleep(4000);
	await shot("v86-mode-1024");
	record("mode-1024", { log: await logSince(r.from), events: await page.evaluate(() => window.spike.events), screen: await page.evaluate(() => window.spike.screen()) });
}

if (steps.includes("time")) {
	for (const [name, command, expect] of [["local", "c:\\h.exe s 20261127143000", "TIME SetLocalTime[^]*local clock now[^]*local clock now[^]*local clock now[^\\n]*\\n"], ["local-priv", "c:\\h.exe s 20261127143000 -T", "TIME SetLocalTime[^]*local clock now[^]*local clock now[^]*local clock now[^\\n]*\\n"], ["utc-priv", "c:\\h.exe S 20261127143000 -T", "SetSystemTime[^]*local clock now[^]*local clock now[^]*local clock now[^\\n]*\\n"], ["utc", "c:\\h.exe S 20261127143000", "SetSystemTime[^]*local clock now[^]*local clock now[^]*local clock now[^\\n]*\\n"]]) {
		const r = await guest(command, expect, 120000);
		await sleep(3000);
		await shot("v86-time-" + name);
		record("time-" + name, { log: await logSince(r.from) });
	}
}

const MIB = 1048576;
const N = Number(process.env.N || MIB);
if (steps.includes("source")) {
	for (const style of ["t", "w"]) {
		await page.evaluate(() => { window.spike.resetCom1(); window.spike.checkPattern = true; });
		const r = await guest(`c:\\h.exe c o ${style} ${N} ${process.env.OPTS || ""}`, "COM SetCommTimeouts", 120000);
		const result = await page.evaluate((n) => window.spike.sourceTest(n), N);
		await page.evaluate((f) => window.spike.waitFor(/COM closed/, 20000, f), r.from);
		record("source-" + style, { bytes: N, ...result, log: await logSince(r.from) });
	}
}

if (steps.includes("echo")) {
	// [style, window, credit, kick after ms of no echo (0 = never)]. The overlapped read (r) and the synchronous one (t) leave nothing pending when the
	// probe exits, so they go first; w leaves a WaitCommEvent pending and the next probe may not get the port for a while.
	const echoCases = process.env.ECHO ? JSON.parse(process.env.ECHO) : [["r", 16384, 4096, 0], ["t", 16384, 4096, 0], ["r", 16384, 4096, 150], ["t", 16384, 4096, 150], ["w", 16384, 4096, 0], ["w", 65536, 16384, 0]];
	for (const [style, windowBytes, credit, kickAfter] of echoCases) {
		const r = await guest(`c:\\h.exe c e ${style} ${N} ${process.env.OPTS || ""}`, "COM SetCommTimeouts", 120000);
		const result = await page.evaluate(([n, w, c, k]) => window.spike.echoTest(n, w, c, k), [N, windowBytes, credit, kickAfter]);
		await page.evaluate((f) => window.spike.waitFor(/COM closed/, 20000, f), r.from);
		record(`echo-${style}-${windowBytes}-${credit}-kick${kickAfter}`, { bytes: N, ...result, log: (await logSince(r.from)).split("\n").filter((l) => !l.includes("progress")).join("\n") });
	}
}

if (steps.includes("ack")) {
	// [style, window, credit]: the link's own pattern, a 16-byte credit for every 4 KiB the guest takes.
	const ackCases = process.env.ACK ? JSON.parse(process.env.ACK) : [["r", 16384, 4096], ["t", 16384, 4096], ["w", 16384, 4096], ["r", 32768, 4096], ["r", 8192, 2048]];
	for (const [style, windowBytes, credit] of ackCases) {
		const r = await guest(`c:\\h.exe c a ${style} ${N} ${process.env.OPTS || ""}`, "COM SetCommTimeouts", 120000);
		const result = await page.evaluate(([n, w, c]) => window.spike.ackTest(n, w, c), [N, windowBytes, credit]);
		await page.evaluate((f) => window.spike.waitFor(/COM closed/, 20000, f), r.from);
		record(`ack-${style}-${windowBytes}-${credit}`, { bytes: N, ...result, log: (await logSince(r.from)).split("\n").filter((l) => !l.includes("progress")).join("\n") });
	}
}

if (steps.includes("stats")) {
	// The same transfer over and over, to count how often the guest stops taking bytes. On a stall: v86's UART and PIC state, then a fresh edge on
	// the interrupt line (spike.repoke), then whether the transfer carries on.
	const trials = process.env.TRIALS ? JSON.parse(process.env.TRIALS) : [["a", "r", 6], ["s", "r", 3], ["e", "r", 3]];
	for (const [kind, style, times, windowBytes = 16384, credit = 4096] of trials) {
		for (let i = 0; i < times; i++) {
			const count = kind === "s" ? 262144 : N;
			const r = await guest(`c:\\h.exe c ${kind} ${style} ${count}`, "COM SetCommTimeouts", 120000);
			const result = await page.evaluate(async ([k, n, w, c]) => {
				const s = window.spike;
				if (k === "s") {
					await s.blastTest(n, 4096, 0);
					await s.sleep(3000);
					return { kind: k, queued: s.emulator.v86.cpu.devices.uart0.input.length };
				}
				const test = k === "a" ? s.ackTest(n, w, c) : s.echoTest(n, w, c);
				const out = await test;
				if (out.stalls.length) {
					out.diagnose = s.diagnose();
					const before = k === "a" ? Math.floor(s.com1.count / 16) : s.com1.count;
					s.repoke();
					await s.sleep(2500);
					const after = k === "a" ? Math.floor(s.com1.count / 16) : s.com1.count;
					out.repoke = { before, after, recovered: after > before, diagnoseAfter: s.diagnose().uart };
				}
				return out;
			}, [kind, count, windowBytes, credit]);
			await sleep(1500);
			record(`stats-${kind}-${style}-${windowBytes}-${credit}-${i + 1}`, { count, ...result, log: (await logSince(r.from)).split("\n").filter((l) => l.startsWith("COM sink") || l.startsWith("COM ack") || l.startsWith("COM echo")).join("\n") });
		}
	}
}

if (steps.includes("dual")) {
	// Data in on COM2 (serial 1), credits out on COM1: no port both sends and receives.
	for (const [windowBytes, credit] of [[16384, 4096], [32768, 4096]]) {
		const r = await guest(`c:\\h.exe c d`, "COM SetCommTimeouts", 120000);
		const result = await page.evaluate(([n, w, c]) => window.spike.ackTest(n, w, c, 0, 400000, 1), [N, windowBytes, credit]);
		await page.evaluate((f) => window.spike.waitFor(/COM dual/, 30000, f), r.from);
		record(`dual-${windowBytes}-${credit}`, { bytes: N, ...result, log: (await logSince(r.from)).split("\n").filter((l) => !l.includes("progress")).join("\n") });
	}
}

if (steps.includes("zone")) {
	for (const bias of [300, 0, 540]) {
		const r = await guest(`c:\\h.exe z ${bias}`, "ZONE SetTimeZoneInformation[^\\n]*\\n", 120000);
		await sleep(2000);
		record("zone-" + bias, { log: await logSince(r.from) });
	}
	await shot("v86-zone");
}

if (steps.includes("blast")) {
	for (const [style, count, chunkBytes, everyMs] of [["w", 262144, 0, 0], ["w", 262144, 4096, 0], ["t", 262144, 0, 0], ["w", 262144, 4096, 20]]) {
		const r = await guest(`c:\\h.exe c s ${style} ${count} ${process.env.OPTS || ""}`, "COM SetCommTimeouts", 120000);
		const queued = await page.evaluate(([n, c, e]) => window.spike.blastTest(n, c, e), [count, chunkBytes, everyMs]);
		await page.evaluate((f) => window.spike.waitFor(/COM closed/, 60000, f), r.from);
		const from = r.from;
		await sleep(1000);
		record(`blast-${style}-${count}-${chunkBytes}-${everyMs}`, { ...queued, log: (await logSince(from)).split("\n").filter((l) => !l.includes("progress")).join("\n") });
	}
}

if (steps.includes("stall")) {
	// [label, style, count, chunk bytes (0 = one call), ms apart, kick after ms of no progress (0 = never)]
	const cases = [
		["burst", "w", 65536, 0, 0, 0],
		["4k", "w", 65536, 4096, 0, 0],
		["256-5ms", "w", 16384, 256, 5, 0],
		["burst-kick", "w", 65536, 0, 0, 150],
		["4k-kick", "w", 262144, 4096, 0, 150],
		["burst-t", "t", 65536, 0, 0, 0],
		["burst-t-kick", "t", 65536, 0, 0, 150],
	];
	for (const [label, style, count, chunkBytes, everyMs, kickAfter] of cases) {
		const r = await guest(`c:\\h.exe c s ${style} ${count} ${process.env.OPTS || ""}`, "COM SetCommTimeouts", 120000);
		const result = await page.evaluate(([n, c, e, k]) => window.spike.sinkTest(n, c, e, k), [count, chunkBytes, everyMs, kickAfter]);
		const closed = await page.evaluate(([f]) => window.spike.waitFor(/COM closed/, 45000, f).then((m) => Boolean(m)), [r.from]);
		record("stall-" + label, { count, style, chunkBytes, everyMs, kickAfter, closed, ...result, log: (await logSince(r.from)).split("\n").filter((l) => !l.includes("progress")).join("\n") });
	}
}

await browser.close();
