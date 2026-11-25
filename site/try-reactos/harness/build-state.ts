// Boot the disk in headless Chromium, open Quill32 on the sample letter, let the disk go quiet and save the machine state.
// The page posts the raw state to the server, which stores it in the images folder as NAME.bin.
// Usage: node build-state.ts [--name state-q32] [--manual] [--zero] [--param k=v ...]
// --manual: launch Quill with the Run box from a CD (phase 1). Without it the image opens the demo letter itself, from its Run key.
// --zero: before saving, run the guest's RAM-zeroing step (see the README) so free memory compresses.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import path from "node:path";
import { fileURLToPath } from "node:url";
import { KEY, PROFILES, BASE, grab, launch, pageUrl, save, sleep } from "./lib.ts";

const here = path.dirname(fileURLToPath(import.meta.url));
const args = process.argv.slice(2);
const opt = (k: string, d: string) => (args.includes("--" + k) ? args[args.indexOf("--" + k) + 1] : d);
const name = opt("name", "state-q32");
const params: Record<string, string> = { disk: "/images/reactos16-trim.img", boot: "hd", ram: "256" };
args.forEach((a, i) => {
	if (a === "--param") {
		const eq = args[i + 1].indexOf("=");
		params[args[i + 1].slice(0, eq)] = args[i + 1].slice(eq + 1);
	}
});
params.statename = name;
const run = `${name}-${Date.now().toString(36)}`;
const s = await launch(run, PROFILES.unthrottled, false);
const ev = <T>(f: string): Promise<T> => s.page.evaluate(f) as Promise<T>;
const sendKey = (c: number[]) => s.page.evaluate((k) => { window.tryReactos.input.key(k); window.tryReactos.input.key(k.map((v: number) => v | 0x80)); }, c);
const quiet = async (secs: number) => {
	let last = await ev<number>("window.tryReactos.stats.ideReadBytes");
	let since = Date.now();
	while (Date.now() - since < secs * 1000) {
		await sleep(500);
		const n = await ev<number>("window.tryReactos.stats.ideReadBytes");
		if (n !== last) {
			last = n;
			since = Date.now();
		}
	}
};
try {
	await s.page.goto(pageUrl(run, params));
	await s.page.click("[data-start]");
	while (!(await ev<boolean>("window.__m.taskbarScore() > 0.3"))) await sleep(500);
	await quiet(8);
	await sleep(15000); // let the shell's start-up work finish
	const exe = opt("exe", "D:\\QUILL32.EXE D:\\DEMO.QLD");
	if (!args.includes("--manual")) {
		const whiteExpr = "(() => { const c = window.tryReactos.canvas(); if (!c || !c.width) return false; const d = c.getContext('2d').getImageData(0, 0, c.width, c.height).data; let n = 0; for (let i = 0; i < d.length; i += 16) if (d[i] > 250 && d[i+1] > 250 && d[i+2] > 250) n++; return n / (d.length / 16) > 0.3; })()";
		while (!(await ev<boolean>(whiteExpr))) await sleep(500);
		await sleep(5000);
		await quiet(10);
	} else {
	await s.page.evaluate((c) => { const i = window.tryReactos.input; i.key(c[0]); i.key(c[1]); i.key(c[1].map((v: number) => v | 0x80)); i.key(c[0].map((v: number) => v | 0x80)); }, [KEY.win, KEY.r]);
	await sleep(2500);
	for (const ch of exe) {
		await s.page.evaluate((x) => window.tryReactos.input.text(x), ch);
		await sleep(120);
	}
	await sleep(800);
	await sendKey(KEY.enter);
	await sleep(15000);
	await quiet(10);
	}
	if (args.includes("--zero")) {
		// Touch and release every free page (C:\ReactOS\MEMFILL.EXE), so stale data in free RAM becomes zeros and the state compresses.
		const kc = [KEY.win, KEY.r];
		await s.page.evaluate((c) => { const i = window.tryReactos.input; i.key(c[0]); i.key(c[1]); i.key(c[1].map((v: number) => v | 0x80)); i.key(c[0].map((v: number) => v | 0x80)); }, kc);
		await sleep(2500);
		for (const ch of "C:\\ReactOS\\MEMFILL.EXE") {
			await s.page.evaluate((x) => window.tryReactos.input.text(x), ch);
			await sleep(120);
		}
		await sleep(800);
		await sendKey(KEY.enter);
		await sleep(20000);
		await quiet(10);
	}
	const f = await grab(s.page);
	if (f) save(path.join(here, "..", "out", name), "before-save.png", f.png);
	const t0 = Date.now();
	await s.cdp.send("Network.disable"); // the protocol would otherwise echo the whole state as a request body
	const info = await s.page.evaluate(async (base) => {
		await window.tryReactos.emulator.stop();
		const t = performance.now();
		const st = await window.tryReactos.emulator.save_state();
		const saveMs = performance.now() - t;
		// In 8 MiB pieces: Playwright cannot take one message the size of the whole state.
		const piece = 8 * 1024 * 1024;
		for (let o = 0; o < st.byteLength; o += piece) {
			const r = await fetch(`${base}/__put/${window.tryReactos.stateName}.bin?offset=${o}`, { method: "POST", body: st.slice(o, o + piece) });
			if (!r.ok) throw new Error("put failed at " + o);
		}
		return { bytes: st.byteLength, saveMs };
	}, BASE);
	console.log(JSON.stringify(info), "total ms", Date.now() - t0);
} finally {
	await s.close();
}
