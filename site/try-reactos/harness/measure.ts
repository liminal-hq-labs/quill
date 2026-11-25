// Boot the machine in headless Chromium and measure it: bytes and time to the desktop, typing latency in Quill, scroll frame rate,
// guest speed, and memory. Writes one JSON file per run to out/results/ and screenshots to out/<name>/.
//
// Usage: node measure.ts [--name N] [--app quill32|quill16|none] [--profile unthrottled|50mbit|10mbit] [--keys 50]
//                        [--param k=v ...]     (page parameters: ram, acpi, vga, disk, cdrom, state, boot, cpuid, jit)
// The server must be running (bun harness/serve.ts). Input reaches the guest only through the page's emulator API.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { KEY, PROFILES, grab, launch, pageUrl, processMemory, save, sleep, stats, flushLog } from "./lib.ts";

const here = path.dirname(fileURLToPath(import.meta.url));
const args = process.argv.slice(2);
const opt = (k: string, d: string) => (args.includes("--" + k) ? args[args.indexOf("--" + k) + 1] : d);
const name = opt("name", "spike-a");
const app = opt("app", "quill32");
const profile = PROFILES[opt("profile", "unthrottled")];
const nKeys = Number(opt("keys", "50"));
const params: Record<string, string> = { disk: "/images/reactos16-spike-a.img", cdrom: "/images/quill.iso", boot: "hd", ram: "256" };
args.forEach((a, i) => {
	if (a === "--param") {
		const eq = args[i + 1].indexOf("=");
		params[args[i + 1].slice(0, eq)] = args[i + 1].slice(eq + 1);
	}
});

const run = `${name}-${Date.now().toString(36)}`;
const outDir = path.join(here, "..", "out", name);
const resultsDir = path.join(here, "..", "out", "results");
const median = (a: number[]) => (a.length ? [...a].sort((x, y) => x - y)[Math.floor(a.length / 2)] : null);
const pct = (a: number[], p: number) => (a.length ? [...a].sort((x, y) => x - y)[Math.min(a.length - 1, Math.floor(a.length * p))] : null);
const round = (n: number | null, d = 1) => (n === null ? null : Math.round(n * 10 ** d) / 10 ** d);

const result: Record<string, any> = {
	run,
	name,
	when: new Date().toISOString(),
	machine: { cpu: os.cpus()[0]?.model, cores: os.cpus().length, memGB: Math.round(os.totalmem() / 2 ** 30), node: process.version },
	v86: "0.5.473",
	params,
	profile,
	app,
	notes: [] as string[],
};

const s = await launch(run, profile, false);
result.browserPid = s.pid;
result.browser = s.browser.version();
console.log("browser pid", s.pid, "run", run);
const pageErrors: string[] = [];
s.page.on("pageerror", (e) => pageErrors.push(e.message));

const ev = <T>(f: string): Promise<T> => s.page.evaluate(f) as Promise<T>;
const snapStats = async () => {
	const st = await stats(run);
	const ide = await ev<number>("window.tryReactos.stats.ideReadBytes");
	return { served: st?.bytes ?? 0, requests: st?.requests ?? 0, disk: st?.kinds?.disk?.bytes ?? 0, diskRequests: st?.kinds?.disk?.requests ?? 0, distinctDisk: st?.diskDistinctBytes ?? 0, ide, kinds: st?.kinds };
};
const shot = async (label: string) => {
	const f = await grab(s.page);
	if (f) save(outDir, `${label}.png`, f.png);
};
const sendKey = (c: number[]) => s.page.evaluate((k) => { window.tryReactos.input.key(k); window.tryReactos.input.key(k.map((v: number) => v | 0x80)); }, c);
const type = (t: string) => s.page.evaluate((x) => window.tryReactos.input.text(x), t);
const mips = async (ms: number) => {
	const a = await ev<number>("window.tryReactos.instructions()");
	const t = Date.now();
	await sleep(ms);
	const b = await ev<number>("window.tryReactos.instructions()");
	return (b - a) / ((Date.now() - t) / 1000) / 1e6;
};

try {
	await s.page.goto(pageUrl(run, params));
	const wall0 = Date.now();
	await s.page.click("[data-start]");

	const restored = Boolean(params.state);
	let restoreReady: number | null = null;
	if (restored) {
		// From a saved state the guest is live as soon as the state is in: Quill is already on screen (a mostly white frame).
		const whiteExpr = "(() => { const c = window.tryReactos.canvas(); if (!c || !c.width) return false; const d = c.getContext('2d').getImageData(0, 0, c.width, c.height).data; let n = 0; for (let i = 0; i < d.length; i += 16) if (d[i] > 250 && d[i+1] > 250 && d[i+2] > 250) n++; return n / (d.length / 16) > 0.3; })()";
		while (Date.now() - wall0 < 600_000) {
			await sleep(100);
			if (await ev<boolean>(whiteExpr)) {
				restoreReady = Date.now() - wall0;
				break;
			}
		}
		const at = await snapStats();
		const m0 = await ev<number>("window.tryReactos.instructions()");
		await sleep(1000);
		const m1 = await ev<number>("window.tryReactos.instructions()");
		result.restore = { seconds: round(restoreReady === null ? null : restoreReady / 1000, 2), guestAliveMips: round((m1 - m0) / 1e6), ...at };
		await shot("01-restored");
		// The disk traffic that follows: ten more seconds of whatever the guest reads once it runs.
		await sleep(10000);
		const later = await snapStats();
		result.restore.diskBytesNext10s = later.disk - at.disk;
		const settle = Number(opt("settle", "0"));
		if (settle > 0) await sleep(settle * 1000);
		result.restore.totalServedAfter10s = later.served;
		console.log("restored", result.restore.seconds, "s", at.served, "bytes served;", later.disk - at.disk, "disk bytes in the next 10 s");
		if (restoreReady === null) throw new Error("state did not restore");
	}
	if (!restored) {
	// 1. The desktop: a cream taskbar on the bottom rows, seen twice in a row.
	let desktopMs: number | null = null;
	let seen = 0;
	while (Date.now() - wall0 < 900_000) {
		await sleep(500);
		const score = await ev<number>("window.__m.taskbarScore()");
		seen = score > 0.3 ? seen + 1 : 0;
		if (seen >= 2) {
			desktopMs = Date.now() - wall0 - 500;
			break;
		}
	}
	const atDesktop = await snapStats();
	result.desktop = { seconds: round(desktopMs === null ? null : desktopMs / 1000), ...atDesktop };
	const modes: any[] = await ev("window.tryReactos.stats.screenSizes");
	result.video = modes.map((m) => `${m.w}x${m.h}x${m.bpp}`);
	result.firstDiskReadSeconds = round(await ev<number>("window.tryReactos.stats.firstIdeReadMs / 1000"), 2);
	await shot("01-desktop");
	console.log("desktop", result.desktop.seconds, "s", atDesktop.served, "bytes");
	if (desktopMs === null) throw new Error("no desktop within 15 minutes");

	// 2. Quiet: the disk idle for 5 seconds.
	let quietAt: number | null = null;
	let lastIde = await ev<number>("window.tryReactos.stats.ideReadBytes");
	let quietSince = Date.now();
	while (Date.now() - wall0 < 900_000) {
		await sleep(500);
		const ide = await ev<number>("window.tryReactos.stats.ideReadBytes");
		if (ide !== lastIde) {
			lastIde = ide;
			quietSince = Date.now();
		} else if (Date.now() - quietSince >= 5000) {
			quietAt = quietSince - wall0;
			break;
		}
	}
	const atQuiet = await snapStats();
	result.quiet = { seconds: round(quietAt === null ? null : quietAt / 1000), ...atQuiet };
	result.idleMips = round(await mips(3000));
	console.log("quiet", result.quiet.seconds, "s", atQuiet.served, "bytes; idle MIPS", result.idleMips);

	}
	if (app !== "none" && !restored) {
		// 3. Start Quill with Win+R, from the Quill CD (the only CD drive, D:).
		const exe = opt("exe", app === "quill32" ? "D:\\QUILL32.EXE" : "D:\\QUILL.EXE");
		const white = "(() => { const c = window.tryReactos.canvas(); const d = c.getContext('2d').getImageData(0, 0, c.width, c.height).data; let n = 0; for (let i = 0; i < d.length; i += 16) if (d[i] > 250 && d[i+1] > 250 && d[i+2] > 250) n++; return n / (d.length / 16) > 0.3; })()";
		const before = await snapStats();
		let ready: number | null = null;
		let tEnter = 0;
		let attempts = 0;
		// Win+R, then the path typed one key at a time: a burst of keys has crashed Explorer's Run box before.
		while (ready === null && attempts < 3) {
			attempts++;
			await s.page.evaluate((c) => { window.tryReactos.input.key(c[0]); window.tryReactos.input.key(c[1]); window.tryReactos.input.key(c[1].map((v: number) => v | 0x80)); window.tryReactos.input.key(c[0].map((v: number) => v | 0x80)); }, [KEY.win, KEY.r]);
			await sleep(2500);
			for (const ch of exe) {
				await type(ch);
				await sleep(120);
			}
			await sleep(800);
			tEnter = Date.now();
			await sendKey(KEY.enter);
			while (Date.now() - tEnter < (attempts < 3 ? 40_000 : 240_000)) {
				await sleep(250);
				if (await ev<boolean>(white)) {
					ready = Date.now() - tEnter;
					break;
				}
			}
			if (ready === null) {
				result.notes.push(`attempt ${attempts}: ${exe} did not appear`);
				await shot(`02-attempt${attempts}`);
				await sendKey(KEY.esc);
				await sleep(1500);
			}
		}
		await sleep(3000);
		const after = await snapStats();
		result.appStart = { exe, attempts, seconds: round(ready === null ? null : ready / 1000), diskBytes: after.disk - before.disk, servedBytes: after.served - before.served };
		await shot("02-app");
		console.log("app ready", result.appStart.seconds, "s");

		if (ready !== null) {
			await typingAndScroll();
		}
	}
	if (restored && restoreReady !== null) await typingAndScroll();
	async function typingAndScroll() {
		{
			// 4. Typing latency: a wide letter, so many pixels change; the threshold sits above a blinking caret.
			// --down N moves the caret down N lines first: the opening line of the demo holds a picture, which is laid out again on every key.
			for (let i = 0; i < Number(opt("down", "0")); i++) {
				await sendKey(KEY.down);
				await sleep(200);
			}
			await sleep(1500);
			const noise = await ev<number>("window.__m.noise(2500)");
			const threshold = Math.max(noise + 30, 60);
			const lat: number[] = [];
			let missed = 0;
			for (let i = 0; i < nKeys; i++) {
				const ms = await s.page.evaluate(([make, th]) => window.__m.keyLatency(make, th, 3000), [[0x11], threshold] as [number[], number]);
				if (ms === null) missed++;
				else lat.push(ms);
				await sleep(350);
			}
			result.typing = { keys: nKeys, noisePixels: noise, thresholdPixels: threshold, missed, medianMs: round(median(lat)), p95Ms: round(pct(lat, 0.95)), minMs: round(Math.min(...lat)), maxMs: round(Math.max(...lat)), samples: lat.map((x) => round(x)) };
			await shot("03-typed");
			console.log("typing median", result.typing.medianMs, "p95", result.typing.p95Ms, "missed", missed);

			// 5. Scrolling: a long document, then Page Down and Page Up at 6 a second for 10 seconds.
			await s.page.evaluate(() => { for (let i = 0; i < 150; i++) { window.tryReactos.input.key([0x1c]); window.tryReactos.input.key([0x9c]); } });
			await sleep(8000);
			// Ctrl+Home, back to the top.
			await s.page.evaluate(() => { const i = window.tryReactos.input; i.key([0x1d]); i.key([0xe0, 0x47]); i.key([0xe0, 0xc7]); i.key([0x9d]); });
			await sleep(2500);
			const still: any = await s.page.evaluate(() => window.__m.frameRate(3000, [], 1));
			const downs = [KEY.pgdn, KEY.pgdn, KEY.pgdn, KEY.pgup, KEY.pgup, KEY.pgup];
			const mipsBefore = await ev<number>("window.tryReactos.instructions()");
			const t = Date.now();
			const fr: any = await s.page.evaluate(([k, hz]) => window.__m.frameRate(10000, k, hz), [downs, 6] as [number[][], number]);
			const mipsScroll = ((await ev<number>("window.tryReactos.instructions()")) - mipsBefore) / ((Date.now() - t) / 1000) / 1e6;
			result.scroll = { baselineFps: round(still.fps, 2), ...fr, fps: round(fr.fps, 2), keysPerSecond: 6, mipsWhileScrolling: round(mipsScroll) };
			await shot("04-scrolled");
			console.log("scroll fps", result.scroll.fps);
		}
	}

	// 6. Memory and final state.
	const heap = await ev<any>("window.__m.heap()");
	const metrics: any = await s.cdp.send("Performance.getMetrics");
	const procs = await processMemory(s.bcdp);
	result.memory = {
		jsHeapMB: heap ? round(heap.used / 2 ** 20) : null,
		cdpJsHeapMB: round((metrics.metrics.find((m: any) => m.name === "JSHeapUsedSize")?.value ?? 0) / 2 ** 20),
		processes: procs,
		rendererRssMB: procs.filter((p) => p.type === "renderer").reduce((n, p) => Math.max(n, p.rssMB), 0),
		allProcessesRssMB: round(procs.reduce((n, p) => n + p.rssMB, 0)),
	};
	result.idleMipsEnd = round(await mips(3000));
	result.blueScreenScore = round(await ev<number>("window.__m.blueScore()"), 3);
	result.final = await snapStats();
	await shot("05-final");
} catch (e: any) {
	result.error = String(e?.message ?? e);
	console.log("error:", result.error);
	await shot("99-error").catch(() => {});
} finally {
	result.pageErrors = pageErrors.slice(0, 10);
	await flushLog();
	save(resultsDir, `${name}.json`, JSON.stringify(result, null, 1));
	await s.close();
}
console.log(JSON.stringify({ desktop: result.desktop, quiet: result.quiet, appStart: result.appStart, typing: { median: result.typing?.medianMs, p95: result.typing?.p95Ms }, scroll: result.scroll?.fps, memory: result.memory?.rendererRssMB }, null, 1));
