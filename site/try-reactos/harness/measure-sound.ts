// Measures what the sound card costs (SITE-51): the guest's instruction rate (MIPS), and the browser's CPU time across all its processes, with no card
// (a saved desktop made without it), with the card idle and its output live, with the card playing Critical Stop over and over, and with the output held
// suspended so the card only drains. Four windows of 10 s each, one browser per variant, on the restored saved desktop. Prints one JSON line per variant.
// Needs the server (bun serve.ts) with IMAGES_DIR holding state-nosound.l22.bin.zst (a state built without the card) beside the manifest's own data.
// Usage: BASE=http://127.0.0.1:8161 bun measure-sound.ts [idle-nosound|idle|playing|playing-drained ...]   (DISPLAY set to a harness Xvfb, WAYLAND_DISPLAY unset)
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import fs from "node:fs";
import { PROFILES, launch, pageUrl, sleep, waitFor } from "./lib.ts";
import { card, openSounds, playSound } from "./sound-lib.ts";

const variants = process.argv.slice(2).length ? process.argv.slice(2) : ["idle-nosound", "idle", "playing", "playing-drained"];
const WINDOWS = 4;
const WINDOW_MS = 10000;

async function cpuSeconds(bcdp: any): Promise<number> {
	const info: any = await bcdp.send("SystemInfo.getProcessInfo");
	return info.processInfo.reduce((sum: number, p: any) => sum + p.cpuTime, 0);
}

for (const variant of variants) {
	const session = await launch("", PROFILES.unthrottled, false);
	const page = session.page;
	try {
		if (variant === "playing-drained") {
			await page.addInitScript(() => {
				const Real = window.AudioContext;
				class Held extends Real {
					constructor(options?: AudioContextOptions) {
						super(options);
						this.suspend().catch(() => {});
					}
					resume(): Promise<void> {
						return Promise.reject(new DOMException("held back", "NotAllowedError"));
					}
				}
				(window as any).AudioContext = Held;
			});
		}
		const nosound = variant === "idle-nosound";
		await page.goto(nosound ? pageUrl("", { state: "/images/state-nosound.l22.bin.zst", disk: "/images/parts/.img.zst", ram: "256" }) : pageUrl("", {}, { plain: true }));
		await page.waitForFunction("document.querySelector('[data-build]').textContent !== 'Build: loading'", null, { timeout: 15000 });
		await page.click("[data-start]");
		await waitFor(page, "window.__m.taskbarScore() > 0.3", 240000);
		await sleep(5000);
		const playing = variant.startsWith("playing");
		if (playing) {
			await openSounds(page);
			// Time spent making the card's samples for the output (the worklet path) or draining them (the held path), summed on the page.
			await page.evaluate(() => {
				const h = (window as any).tryReactos.soundHandle;
				const w = window as any;
				w.__renderMs = 0;
				w.__renderCalls = 0;
				if (h.sink) {
					const original = h.sink.onNeed;
					h.sink.onNeed = (n: number) => {
						const t = performance.now();
						const r = original(n);
						w.__renderMs += performance.now() - t;
						w.__renderCalls++;
						return r;
					};
				}
			});
		}
		await sleep(15000);
		const rows: any[] = [];
		for (let n = 0; n < WINDOWS; n++) {
			const i0 = (await page.evaluate("window.tryReactos.instructions()")) as number;
			const c0 = await cpuSeconds(session.bcdp);
			const m0 = (await page.evaluate("window.__renderMs || 0")) as number;
			const k0 = (await card(page))?.dma ?? 0;
			const t0 = Date.now();
			// Critical Stop every 2.3 s through the whole window (the WP-WEB-4 pattern: 1.6 s of sound and a short gap).
			while (Date.now() - t0 < WINDOW_MS) {
				if (playing) await playSound(page);
				await sleep(playing ? 2300 : WINDOW_MS);
			}
			const wall = (Date.now() - t0) / 1000;
			const i1 = (await page.evaluate("window.tryReactos.instructions()")) as number;
			const c1 = await cpuSeconds(session.bcdp);
			const m1 = (await page.evaluate("window.__renderMs || 0")) as number;
			const k1 = (await card(page))?.dma ?? 0;
			rows.push({ mips: Math.round(((i1 - i0) / wall / 1e6) * 100) / 100, browserCpuCores: Math.round(((c1 - c0) / wall) * 1000) / 1000, renderMsPerSecond: Math.round(((m1 - m0) / wall) * 100) / 100, framesTaken: k1 - k0 });
		}
		const mean = (key: string) => Math.round((rows.reduce((s, r) => s + r[key], 0) / rows.length) * 1000) / 1000;
		const c = await card(page);
		const sinkStats = c && c.sink ? await page.evaluate("window.tryReactos.soundHandle.sinkStats()") : null;
		console.log(JSON.stringify({ variant, mips: rows.map((r) => r.mips), meanMips: mean("mips"), browserCpuCores: rows.map((r) => r.browserCpuCores), meanBrowserCpuCores: mean("browserCpuCores"), renderMsPerSecond: mean("renderMsPerSecond"), framesTakenPerWindow: rows.map((r) => r.framesTaken), output: c?.output ?? "no card", drainedFrames: c?.drained ?? 0, sink: sinkStats, load: fs.readFileSync("/proc/loadavg", "utf8").trim() }));
	} finally {
		await session.close();
	}
}
