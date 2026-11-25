// Mouse event-to-pixel latency: from a mouse delta reaching the emulator bus to the first canvas frame in which the guest's cursor has moved.
// Usage: BASE=http://127.0.0.1:8089 node mouse-latency.ts [--label NAME] [--trials 30] [--reg "reg add ..."] [--serial "cmd"] [--param k=v ...]
// --reg CMD: run CMD (a cmd.exe command line) in the guest's Run box before measuring; its output goes to COM1 and is printed.
// The guest's cursor is parked in the top-left corner (the guest clamps it), and the pixels of a band along the top of the screen are watched.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { KEY, PROFILES, launch, pageUrl, sleep, save } from "./lib.ts";

const here = path.dirname(fileURLToPath(import.meta.url));
const args = process.argv.slice(2);
const opt = (k: string, d: string) => (args.includes("--" + k) ? args[args.indexOf("--" + k) + 1] : d);
const label = opt("label", "mouse");
const trials = Number(opt("trials", "30"));
const params: Record<string, string> = { state: "/images/state-final.l22.bin.zst", disk: "/images/reactos16-trim.img", ram: "256" };
args.forEach((a, i) => {
	if (a === "--param") {
		const eq = args[i + 1].indexOf("=");
		params[args[i + 1].slice(0, eq)] = args[i + 1].slice(eq + 1);
	}
});
const run = `${label}-${Date.now().toString(36)}`;
const s = await launch(run, PROFILES.unthrottled, false);
const ev = <T>(f: string | (() => unknown), arg?: unknown): Promise<T> => s.page.evaluate(f as never, arg as never) as Promise<T>;
const load = os.loadavg().map((x) => x.toFixed(1)).join(" ");

async function guestCmd(cmd: string): Promise<string> {
	await ev(() => {
		(window as any).__ser = "";
		(window as any).tryReactos.emulator.add_listener("serial0-output-byte", (b: number) => ((window as any).__ser += String.fromCharCode(b)));
	});
	await ev((c) => { const i = (window as any).tryReactos.input; i.key(c[0]); i.key(c[1]); i.key(c[1].map((v: number) => v | 0x80)); i.key(c[0].map((v: number) => v | 0x80)); }, [KEY.win, KEY.r]);
	await sleep(2500);
	for (const ch of cmd) {
		await ev((x) => (window as any).tryReactos.input.text(x), ch);
		await sleep(110);
	}
	await sleep(500);
	await ev((k) => { (window as any).tryReactos.input.key(k); (window as any).tryReactos.input.key(k.map((v: number) => v | 0x80)); }, KEY.enter);
	await sleep(6000);
	return ev<string>(() => (window as any).__ser);
}

const out: any = { label, params, loadAtStart: load };
try {
	await s.page.goto(pageUrl(run, params));
	await s.page.click("[data-start]");
	await s.page.waitForFunction("window.__m.taskbarScore() > 0.15", null, { timeout: 120000 });
	await sleep(4000);
	if (opt("reg", "")) {
		out.regOutput = await guestCmd(opt("reg", ""));
		console.log("guest said:", JSON.stringify(out.regOutput));
	}
	// Page-side measurement code.
	const res = await ev<any>(async (n: number) => {
		const T = (window as any).tryReactos;
		const c = T.canvas();
		const g = c.getContext("2d");
		const W = c.width;
		const BAND = 24;
		const band = () => g.getImageData(0, 0, W, BAND).data;
		const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
		const raf = () => new Promise<number>((r) => requestAnimationFrame(r));
		const park = async () => {
			for (let i = 0; i < 12; i++) { T.input.mouseDelta(-100, 0); T.input.mouseDelta(0, -100); await sleep(10); }
			await sleep(400);
		};
		// The rightmost column that differs between two bands: the cursor's far edge.
		const edge = (a: Uint8ClampedArray, b: Uint8ClampedArray) => {
			let m = -1;
			for (let y = 0; y < BAND; y++) for (let x = W - 1; x > m; x--) { const i = (y * W + x) * 4; if (a[i] !== b[i] || a[i + 1] !== b[i + 1] || a[i + 2] !== b[i + 2]) { m = x; break; } }
			return m;
		};
		const r: any = { lat: [], accel: {}, size: [W, c.height] };
		await park();
		const base = band();
		const baseEdge = (() => { // the cursor's own size at the corner: compare with a band after moving far away
			return -1;
		})();
		void baseEdge;
		// Step response: one event of dx = 60, time to the first frame whose band differs from the parked one beyond the cursor's own box.
		for (let t = 0; t < n; t++) {
			await park();
			const b0 = band();
			await sleep(150 + Math.random() * 200);
			const t0 = performance.now();
			T.input.mouseDelta(60, 0);
			let done = null;
			let polls = 0;
			while (performance.now() - t0 < 2000) {
				await raf();
				polls++;
				const e = edge(b0, band());
				if (e > 66) { done = performance.now() - t0; break; }
			}
			r.lat.push(done);
			r.polls = (r.polls || 0) + polls;
		}
		// Pointer acceleration: a single event of dx, and the same distance as many events of 1; the cursor's far edge tells how far it went.
		const rest = async () => { await park(); return band(); };
		for (const d of [1, 2, 4, 8, 16, 32, 64]) {
			const b0 = await rest();
			T.input.mouseDelta(d, 0);
			await sleep(400);
			r.accel["single" + d] = edge(b0, band());
		}
		for (const [steps, per] of [[16, 1], [16, 2], [16, 4]] as number[][]) {
			const b0 = await rest();
			for (let i = 0; i < steps; i++) { T.input.mouseDelta(per, 0); await sleep(16); }
			await sleep(400);
			r.accel["slow" + steps + "x" + per] = edge(b0, band());
		}
		const bz = await rest();
		T.input.mouseDelta(0, 0);
		r.accel.zero = edge(bz, band());
		// Continuous motion: 3 s at 125 Hz of dx 4 back and forth, counting changed canvas frames and the instruction rate.
		await park();
		const i0 = T.emulator.get_instruction_counter();
		let last = band();
		let changed = 0, frames = 0, sent = 0;
		const tStart = performance.now();
		let nextSend = tStart, dir = 1;
		while (performance.now() - tStart < 3000) {
			const now = performance.now();
			while (nextSend <= now) { T.input.mouseDelta(4 * dir, 0); sent++; nextSend += 8; if (sent % 100 === 0) dir = -dir; }
			await raf();
			frames++;
			const b = band();
			let d = false;
			for (let i = 0; i < b.length; i += 4) if (b[i] !== last[i] || b[i + 1] !== last[i + 1] || b[i + 2] !== last[i + 2]) { d = true; break; }
			if (d) changed++;
			last = b;
		}
		const secs = (performance.now() - tStart) / 1000;
		r.motion = { sent, rafFrames: frames, changedFrames: changed, rafFps: frames / secs, changedFps: changed / secs, mips: (T.emulator.get_instruction_counter() - i0) / secs / 1e6 };
		return r;
	}, trials);
	const ok = (res.lat as (number | null)[]).filter((x): x is number => x !== null).sort((a, b) => a - b);
	const q = (p: number) => ok[Math.min(ok.length - 1, Math.floor(p * ok.length))];
	out.latency = { n: ok.length, missed: res.lat.length - ok.length, median: q(0.5), p90: q(0.9), min: ok[0], max: ok[ok.length - 1], all: res.lat.map((x: number | null) => (x === null ? null : Math.round(x))) };
	out.accel = res.accel;
	out.motion = res.motion;
	out.loadAtEnd = os.loadavg().map((x) => x.toFixed(1)).join(" ");
	console.log(JSON.stringify(out, null, 1));
	fs.mkdirSync(path.join(here, "..", "out", "mouse"), { recursive: true });
	fs.writeFileSync(path.join(here, "..", "out", "mouse", `${label}.json`), JSON.stringify(out, null, 1));
} finally {
	await s.close();
}
