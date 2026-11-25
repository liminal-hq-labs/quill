// Real pointer events, end to end: the browser's mouse event (its own timestamp) to the first canvas frame in which the guest's pointer has moved,
// through the page's own pointer handler (lock, speed, scale, coalescing). Also reports the guest pointer's distance per CSS pixel moved.
// Usage: BASE=http://127.0.0.1:8089 node mouse-real.ts [--label NAME] [--trials 20] [--param k=v ...] [--shot FILE] [--width W --height H --dpr D]
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { PROFILES, launch, pageUrl, sleep, grab, save } from "./lib.ts";

const here = path.dirname(fileURLToPath(import.meta.url));
const args = process.argv.slice(2);
const opt = (k: string, d: string) => (args.includes("--" + k) ? args[args.indexOf("--" + k) + 1] : d);
const label = opt("label", "real");
const trials = Number(opt("trials", "20"));
const params: Record<string, string> = { state: "/images/state-final.l22.bin.zst", disk: "/images/reactos16-trim.img", ram: "256" };
args.forEach((a, i) => {
	if (a === "--param") {
		const eq = args[i + 1].indexOf("=");
		params[args[i + 1].slice(0, eq)] = args[i + 1].slice(eq + 1);
	}
});
const run = `${label}-${Date.now().toString(36)}`;
const s = await launch(run, PROFILES.unthrottled, false);
const ev = <T>(f: string | ((a: any) => unknown), arg?: unknown): Promise<T> => s.page.evaluate(f as never, arg as never) as Promise<T>;
const out: any = { label, params, loadAtStart: os.loadavg().map((x) => x.toFixed(1)).join(" ") };
try {
	await s.page.goto(pageUrl(run, params));
	await s.page.click("[data-start]");
	await s.page.waitForFunction("window.__m.taskbarScore() > 0.15", null, { timeout: 120000 });
	await sleep(3000);
	const box = await s.page.locator("[data-stage]").boundingBox();
	await s.page.mouse.move(box!.x + box!.width / 2, box!.y + box!.height / 2);
	await s.page.mouse.click(box!.x + box!.width / 2, box!.y + box!.height / 2);
	await sleep(1500);
	out.locked = await ev<boolean>(() => !!document.pointerLockElement);
	out.pointer = await ev<any>(() => { const p = (window as any).tryReactos.pointer; return { speed: p.speed, raw: p.raw, local: p.local, calibrated: p.calibrated, x: p.x, y: p.y }; });
	// The page measures; this side moves the real (synthetic CDP) mouse, trial by trial.
	const lat: (number | null)[] = [];
	const moved: number[] = [];
	const handler: number[] = [];
	await ev(() => {
		const T = (window as any).tryReactos;
		const c = T.canvas();
		const g = c.getContext("2d");
		const W = c.width, BAND = 24;
		const w = window as any;
		w.__band = () => g.getImageData(0, 0, W, BAND).data;
		w.__edge = (a: Uint8ClampedArray, b: Uint8ClampedArray) => { let m = -1; for (let y = 0; y < BAND; y++) for (let x = W - 1; x > m; x--) { const i = (y * W + x) * 4; if (a[i] !== b[i] || a[i + 1] !== b[i + 1] || a[i + 2] !== b[i + 2]) { m = x; break; } } return m; };
		w.__park = async () => { for (let i = 0; i < 12; i++) { T.input.mouseDelta(-100, 0); T.input.mouseDelta(0, -100); await new Promise((r) => setTimeout(r, 10)); } await new Promise((r) => setTimeout(r, 400)); };
		w.__ev = 0; w.__h = 0;
		document.addEventListener("pointermove", (e) => { if (!w.__ev) w.__ev = e.timeStamp; }, true);
		document.addEventListener("pointermove", () => { if (!w.__h) w.__h = performance.now(); }, false);
		w.__wait = async (b0: Uint8ClampedArray) => {
			const t0 = performance.now();
			while (!w.__ev && performance.now() - t0 < 1500) await new Promise((r) => setTimeout(r, 1));
			if (!w.__ev) return null;
			const tw = performance.now();
			while (performance.now() - tw < 2000) { await new Promise((r) => requestAnimationFrame(r)); const e = w.__edge(b0, w.__band()); if (e > 24) return { ms: performance.now() - w.__ev, edge: e, handler: w.__h - w.__ev }; }
			return null;
		};
	});
	for (let t = 0; t < trials; t++) {
		await ev(async () => { const w = window as any; await w.__park(); w.__b0 = w.__band(); w.__ev = 0; w.__h = 0; });
		await sleep(150 + Math.random() * 200);
		const cx = box!.x + box!.width / 2, cy = box!.y + box!.height / 2;
		await ev(() => { const w = window as any; w.__p = w.__wait(w.__b0); });
		await s.page.mouse.move(cx + 30, cy);
		const r = await ev<any>(() => (window as any).__p);
		lat.push(r ? r.ms : null);
		if (r) { moved.push(r.edge); handler.push(r.handler); }
		await s.page.mouse.move(cx, cy);
	}
	const ok = lat.filter((x): x is number => x !== null).sort((a, b) => a - b);
	const q = (p: number) => ok[Math.min(ok.length - 1, Math.floor(p * ok.length))];
	out.latency = { n: ok.length, missed: lat.length - ok.length, median: q(0.5), p90: q(0.9), min: ok[0], max: ok[ok.length - 1], handlerMedianMs: handler.sort((a, b) => a - b)[Math.floor(handler.length / 2)] };
	out.edgeAfter30CssPx = moved.sort((a, b) => a - b)[Math.floor(moved.length / 2)];
	out.loadAtEnd = os.loadavg().map((x) => x.toFixed(1)).join(" ");
	console.log(JSON.stringify(out, null, 1));
	fs.mkdirSync(path.join(here, "..", "out", "mouse"), { recursive: true });
	fs.writeFileSync(path.join(here, "..", "out", "mouse", `${label}.json`), JSON.stringify(out, null, 1));
	const shot = opt("shot", "");
	if (shot) await s.page.screenshot({ path: shot, fullPage: true });
} finally {
	await s.close();
}
