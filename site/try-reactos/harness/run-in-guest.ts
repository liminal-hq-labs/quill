// Type one command into the guest's Run box (restored state), wait, and save a screenshot every few seconds.
// Usage: BASE=http://127.0.0.1:8089 node run-in-guest.ts OUTDIR "C:\Quill\QUILL.EXE C:\Quill\DEMO16.QLD" [--seconds 60] [--every 10]
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { KEY, PROFILES, launch, pageUrl, sleep, grab, save } from "./lib.ts";

const [dir, cmd] = process.argv.slice(2);
const args = process.argv.slice(2);
const opt = (k: string, d: number) => (args.includes("--" + k) ? Number(args[args.indexOf("--" + k) + 1]) : d);
const s = await launch("rig" + Date.now().toString(36), PROFILES.unthrottled, false);
try {
	await s.page.goto(pageUrl(s.run, { state: "/images/state-final.l22.bin.zst", disk: "/images/reactos16-trim.img", ram: "256" }));
	await s.page.click("[data-start]");
	await s.page.waitForFunction("window.__m.taskbarScore() > 0.15", null, { timeout: 120000 });
	await sleep(3000);
	const key = (c: number[]) => s.page.evaluate((k) => { const i = (window as any).tryReactos.input; i.key(k); i.key(k.map((v: number) => v | 0x80)); }, c);
	await s.page.evaluate((c) => { const i = (window as any).tryReactos.input; i.key(c[0]); i.key(c[1]); i.key(c[1].map((v: number) => v | 0x80)); i.key(c[0].map((v: number) => v | 0x80)); }, [KEY.win, KEY.r]);
	await sleep(2500);
	for (const ch of cmd) { await s.page.evaluate((x) => (window as any).tryReactos.input.text(x), ch); await sleep(110); }
	await sleep(500);
	await key(KEY.enter);
	const t0 = Date.now();
	for (let i = 0; i * opt("every", 10) < opt("seconds", 60); i++) {
		await sleep(opt("every", 10) * 1000);
		const f = await grab(s.page);
		if (f) save(dir, `t${String((Date.now() - t0) / 1000 | 0).padStart(3, "0")}.png`, f.png);
	}
} finally {
	await s.close();
}
