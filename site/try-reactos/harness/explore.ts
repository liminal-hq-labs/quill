// Interactive exploration: boot the spike disk, wait for the desktop, then run key sequences from a JSON file, screenshotting after each.
// Usage: node explore.ts --name NAME --steps steps.json [--param k=v ...]
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { KEY, PROFILES, grab, launch, pageUrl, save, sleep, stats, waitFor } from "./lib.ts";

const here = path.dirname(fileURLToPath(import.meta.url));
const args = process.argv.slice(2);
const opt = (k: string, d: string) => (args.includes("--" + k) ? args[args.indexOf("--" + k) + 1] : d);
const name = opt("name", "explore");
const steps: any[] = JSON.parse(fs.readFileSync(opt("steps", "steps.json"), "utf8"));
const params: Record<string, string> = { disk: "/images/reactos16-spike-a.img", cdrom: "/images/quill.iso", boot: "hd", ram: "256" };
args.forEach((a, i) => {
	if (a === "--param") {
		const [k, v] = args[i + 1].split("=");
		params[k] = v;
	}
});
const run = `${name}-${Date.now().toString(36)}`;
const out = path.join(here, "..", "out", name);
const s = await launch(run, PROFILES.unthrottled, false);
console.log("browser pid", s.pid);
try {
	await s.page.goto(pageUrl(run, params));
	await s.page.click("[data-start]");
	const t = await waitFor(s.page, "window.__m.taskbarScore() > 0.3", 240000);
	console.log("desktop after ms", t);
	await sleep(8000);
	let n = 0;
	for (const st of steps) {
		if (st.keys) for (const k of st.keys) {
			if (typeof k === "string") { for (const ch of k) { await s.page.evaluate((x) => window.tryReactos.input.text(x), ch); await sleep(110); } }
			else {
				await s.page.evaluate((c) => { window.tryReactos.input.key(c); window.tryReactos.input.key(c.map((v: number) => v | 0x80)); }, k);
			}
			await sleep(150);
		}
		if (st.chord) {
			await s.page.evaluate((c) => { for (const k of c) window.tryReactos.input.key(k); for (const k of [...c].reverse()) window.tryReactos.input.key(k.map((v: number) => v | 0x80)); }, st.chord);
		}
		if (st.mouse) await s.page.evaluate((m) => { window.tryReactos.input.mouseDelta(m[0], m[1]); }, st.mouse);
		if (st.click) await s.page.evaluate((c) => { window.tryReactos.input.mouseClick(c[0], 0, 0); window.tryReactos.input.mouseClick(0, 0, 0); }, st.click);
		await sleep((st.wait ?? 3) * 1000);
		const f = await grab(s.page);
		if (f) save(out, `${String(n++).padStart(2, "0")}-${st.name ?? "step"}.png`, f.png);
		console.log(st.name, f?.w, f?.h, f?.hash);
	}
	console.log(JSON.stringify(await stats(run)).slice(0, 400));
} finally {
	await s.close();
}
