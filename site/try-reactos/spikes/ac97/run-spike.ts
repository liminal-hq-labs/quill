// Drives the AC'97 spike page in headless Chromium: boots the QAGRR parts with the device, plays a sound in the guest, and measures what the page's audio sink receives.
// Usage (from this folder, server already running): BASE=http://127.0.0.1:18101 bun run-spike.ts <scenario> [--out DIR] [--no-ac97] [--attach hook|loaded]
// Scenarios: boot (cold boot, look at the device), see run-spike.ts for the rest.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import fs from "node:fs";
import path from "node:path";
import { chromium } from "../../harness/node_modules/playwright-core/index.mjs";

export const BASE = process.env.BASE || "http://127.0.0.1:18101";
export const CHROMIUM = process.env.CHROMIUM || "/usr/bin/chromium";
export const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const here = path.dirname(new URL(import.meta.url).pathname);

export async function open() {
	// DISPLAY must name the harness's own Xvfb, never the desktop.
	if (!process.env.DISPLAY || process.env.WAYLAND_DISPLAY) throw new Error("set DISPLAY to the harness Xvfb and unset WAYLAND_DISPLAY");
	const browser = await chromium.launch({
		executablePath: CHROMIUM,
		headless: true,
		args: ["--enable-precise-memory-info", "--no-first-run", "--autoplay-policy=no-user-gesture-required", "--disable-background-timer-throttling", "--disable-renderer-backgrounding", "--disable-backgrounding-occluded-windows"],
	});
	const context = await browser.newContext({ viewport: { width: 1100, height: 900 }, serviceWorkers: "block" });
	await context.addInitScript({ path: path.join(here, "../../harness/inpage.js") });
	await context.addInitScript(() => {
		(window as any).tryReactos = { canvas: () => document.querySelector("#screen canvas") };
	});
	const page = await context.newPage();
	page.on("console", (m) => { if (process.env.CONSOLE) console.log("[page]", m.text()); });
	page.on("pageerror", (e) => console.log("[pageerror]", String(e)));
	const ev = <T>(f: string): Promise<T> => page.evaluate(f) as Promise<T>;
	const pid = (await (await browser.newBrowserCDPSession()).send("SystemInfo.getProcessInfo" as never).catch(() => null) as any)?.processInfo?.find((p: any) => p.type === "browser")?.id;
	return { browser, page, ev, pid };
}

export async function shot(page: any, dir: string, name: string) {
	fs.mkdirSync(dir, { recursive: true });
	const url: string | null = await page.evaluate(() => { const c = document.querySelector("#screen canvas") as HTMLCanvasElement | null; return c && c.width ? c.toDataURL("image/png") : null; });
	if (url) fs.writeFileSync(path.join(dir, name), Buffer.from(url.split(",")[1], "base64"));
}

const args = process.argv.slice(2);
const opt = (k: string, d: string) => (args.includes("--" + k) ? args[args.indexOf("--" + k) + 1] : d);
const scenario = args[0];
const out = opt("out", path.join(process.env.HOME!, "Documents/Liminal HQ/Quill/Web demo/WP-WEB-4"));

if (import.meta.main && scenario === "boot") {
	const { browser, page, ev, pid } = await open();
	console.log("browser pid", pid);
	try {
		await page.goto(`${BASE}/try-reactos/spikes/ac97/spike.html`);
		const startOpts = JSON.stringify({ ac97: !args.includes("--no-ac97"), attach: opt("attach", "hook") });
		await page.evaluate(`window.spike.start(${startOpts})`);
		const t0 = Date.now();
		let last = 0;
		while (Date.now() - t0 < Number(opt("secs", "600")) * 1000) {
			await sleep(5000);
			const score = await ev<number>("window.__m.taskbarScore()");
			const ide = await ev<number>("window.spike.ideBytes");
			if (Date.now() - last > 30000) { console.log(Math.round((Date.now() - t0) / 1000), "s taskbar", score.toFixed(2), "ide MB", (ide / 1048576).toFixed(1)); last = Date.now(); }
			if (score > 0.3) break;
		}
		await sleep(15000);
		await shot(page, out, "boot-" + opt("tag", "x") + ".png");
		console.log(JSON.stringify(await page.evaluate("({pci: window.spike.pciDevices, before: window.spike.pciDevicesBefore, id: window.spike.pciId, events: window.spike.events, counts: window.spike.traceCount, counters: window.spike.counters()})"), null, 1));
	} finally {
		await browser.close();
	}
}

// Boot to the desktop with the device and report what the BIOS and the guest did to it.
if (import.meta.main && scenario === "probe") {
	const { browser, page, ev } = await open();
	try {
		await page.goto(`${BASE}/try-reactos/spikes/ac97/spike.html`);
		await page.evaluate(`window.spike.start(${JSON.stringify({ ac97: true, attach: opt("attach", "hook"), state: opt("state", ""), audioState: args.includes("--audio-json") ? JSON.parse(fs.readFileSync(opt("audio-json", ""), "utf8")) : null })})`);
		while ((await ev<number>("window.__m.taskbarScore()")) < 0.3) await sleep(2000);
		console.log("desktop at page ms", await ev<number>("Math.round(performance.now())"));
		await sleep(Number(opt("wait", "60")) * 1000);
		console.log(JSON.stringify(await page.evaluate(`(() => { const s = window.spike; const sp = s.pci.device_spaces[s.pciId]; const isa = new Uint8Array(s.pci.isa_bridge_space.buffer); return {
			config: Array.from(new Uint8Array(sp.buffer)).map(b => b.toString(16).padStart(2, "0")).join(""),
			pirq: Array.from(isa.slice(0x60, 0x64)),
			counts: s.traceCount, counters: s.counters(), first: s.trace.slice(0, 60) }; })()`), null, 0));
		await shot(page, out, "probe.png");
	} finally {
		await browser.close();
	}
}

// Run a command from the Run box (Win+R), the way the other harness scripts do.
export async function runBox(page: any, cmd: string) {
	await page.evaluate(() => (window as any).spike.chord([[0xe0, 0x5b], [0x13]]));
	await sleep(2500);
	for (const ch of cmd) {
		await page.evaluate((x: string) => (window as any).spike.text(x), ch);
		await sleep(90);
	}
	await sleep(600);
	await page.evaluate(() => (window as any).spike.key([0x1c]));
}

// Boot, start the audio sink, run a command, and report what the guest's playback gave the page.
if (import.meta.main && scenario === "play") {
	const cmd = opt("cmd", "mplay32 /play /close C:\\ReactOS\\Media\\ReactOS_Logon.wav");
	const { browser, page, ev } = await open();
	try {
		await page.goto(`${BASE}/try-reactos/spikes/ac97/spike.html`);
		await page.evaluate(`window.spike.start(${JSON.stringify({ ac97: true, attach: opt("attach", "hook"), state: opt("state", ""), audioState: args.includes("--audio-json") ? JSON.parse(fs.readFileSync(opt("audio-json", ""), "utf8")) : null })})`);
		while ((await ev<number>("window.__m.taskbarScore()")) < 0.3) await sleep(2000);
		await sleep(20000);
		console.log("audio context:", await page.evaluate("window.spike.startAudio()"));
		await sleep(2000);
		await page.evaluate("window.spike.traceCount = {}; window.spike.trace = []");
		await page.evaluate("window.spike.sink.record(true)");
		const i0 = await ev<number>("window.spike.instr()");
		const t0 = Date.now();
		await runBox(page, cmd);
		await sleep(Number(opt("wait", "20")) * 1000);
		const mips = ((await ev<number>("window.spike.instr()")) - i0) / ((Date.now() - t0) / 1000) / 1e6;
		const st: any = await page.evaluate("window.spike.sink.stats()");
		console.log(JSON.stringify({ mips, sink: st, counters: await page.evaluate("window.spike.counters()"), counts: await page.evaluate("window.spike.traceCount") }));
		await shot(page, out, "play.png");
	} finally {
		await browser.close();
	}
}

// An interactive session for working out the guest steps: boots, starts the audio sink, then runs each snippet dropped into DIR/cmd.js (an async function body
// with page, ev, sleep, runBox, shot in scope), writing its result to DIR/result.txt. DIR/stop ends the session.
if (import.meta.main && scenario === "session") {
	const dir = opt("dir", "/tmp/claude-1000/w4/session");
	fs.mkdirSync(dir, { recursive: true });
	const { browser, page, ev } = await open();
	const AsyncFunction = Object.getPrototypeOf(async function () {}).constructor;
	try {
		await page.goto(`${BASE}/try-reactos/spikes/ac97/spike.html`);
		await page.evaluate(`window.spike.start(${JSON.stringify({ ac97: !args.includes("--no-ac97"), attach: opt("attach", "hook"), state: opt("state", ""), audioState: args.includes("--audio-json") ? JSON.parse(fs.readFileSync(opt("audio-json", ""), "utf8")) : null })})`);
		while ((await ev<number>("window.__m.taskbarScore()")) < 0.3) await sleep(2000);
		await sleep(Number(opt("settle", "45")) * 1000);
		await page.evaluate("window.spike.startAudio()");
		fs.writeFileSync(path.join(dir, "result.txt"), "ready\n");
		while (!fs.existsSync(path.join(dir, "stop"))) {
			const f = path.join(dir, "cmd.js");
			if (fs.existsSync(f)) {
				const code = fs.readFileSync(f, "utf8");
				fs.unlinkSync(f);
				let res: any;
				try {
					res = await new AsyncFunction("page", "ev", "sleep", "runBox", "shot", "out", code)(page, ev, sleep, runBox, shot, out);
				} catch (e) {
					res = "ERROR " + String(e);
				}
				fs.writeFileSync(path.join(dir, "result.txt"), typeof res === "string" ? res : JSON.stringify(res, null, 1));
			}
			await sleep(500);
		}
	} finally {
		await browser.close();
	}
}

// Idle CPU: boot, settle, and read the guest's instruction rate over a window, with or without the device and the sink. --runs N repeats.
if (import.meta.main && scenario === "mips") {
	const ac97 = !args.includes("--no-ac97");
	const sink = args.includes("--sink");
	const { browser, page, ev } = await open();
	try {
		await page.goto(`${BASE}/try-reactos/spikes/ac97/spike.html`);
		await page.evaluate(`window.spike.start(${JSON.stringify({ ac97, attach: "hook" })})`);
		while ((await ev<number>("window.__m.taskbarScore()")) < 0.3) await sleep(2000);
		await sleep(60000);
		if (sink) await page.evaluate("window.spike.startAudio()");
		const rates: number[] = [];
		for (let n = 0; n < 4; n++) {
			const i0 = await ev<number>("window.spike.instr()");
			const t0 = Date.now();
			await sleep(10000);
			rates.push(((await ev<number>("window.spike.instr()")) - i0) / ((Date.now() - t0) / 1000) / 1e6);
		}
		console.log(JSON.stringify({ ac97, sink, mips: rates.map((r) => Math.round(r * 100) / 100), load: fs.readFileSync("/proc/loadavg", "utf8").trim() }));
	} finally {
		await browser.close();
	}
}
