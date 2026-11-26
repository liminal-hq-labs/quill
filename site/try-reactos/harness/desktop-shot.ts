// Restore the saved state in headless Chromium, then save the guest's own pixels (the emulator canvas, not a browser screenshot): as it restores
// and after Win+D has shown the desktop with its icons. No host desktop is captured.
// Usage: BASE=http://127.0.0.1:8092 bun desktop-shot.ts OUTDIR [--param k=v ...]
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import path from "node:path";
import { KEY, PROFILES, grab, launch, pageUrl, save, sleep, waitFor } from "./lib.ts";

const args = process.argv.slice(2);
const dir = args[0];
const params: Record<string, string> = { state: "/images/state.l22.bin.zst", disk: "/images/parts/.img.zst", ram: "256" };
args.forEach((a, i) => {
	if (a === "--param") {
		const eq = args[i + 1].indexOf("=");
		params[args[i + 1].slice(0, eq)] = args[i + 1].slice(eq + 1);
	}
});
const run = `desktop-shot-${Date.now().toString(36)}`;
const s = await launch(run, PROFILES.unthrottled, false);
try {
	await s.page.goto(pageUrl(run, params));
	await s.page.click("[data-start]");
	const t = await waitFor(s.page, "window.__m.taskbarScore() > 0.3", 240000);
	console.log("desktop after ms", t);
	await sleep(6000);
	const a = await grab(s.page);
	if (a) save(dir, "restored.png", a.png);
	await s.page.evaluate((c) => {
		const i = window.tryReactos.input;
		i.key(c[0]);
		i.key(c[1]);
		i.key(c[1].map((v: number) => v | 0x80));
		i.key(c[0].map((v: number) => v | 0x80));
	}, [KEY.win, [0x20]]);
	await sleep(4000);
	const b = await grab(s.page);
	if (b) save(dir, "desktop.png", b.png);
	console.log("canvas", b?.w, "x", b?.h, "->", path.resolve(dir));
} finally {
	await s.close();
}
