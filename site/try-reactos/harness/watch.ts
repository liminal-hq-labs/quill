// Boot a machine and take a screenshot of the guest every few seconds, with the server's counters beside each one.
// Usage: node watch.ts --name NAME --seconds N [--every S] [--param key=value ...]
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import path from "node:path";
import { fileURLToPath } from "node:url";
import { PROFILES, grab, launch, pageUrl, save, sleep, stats, flushLog } from "./lib.ts";

const here = path.dirname(fileURLToPath(import.meta.url));
const args = process.argv.slice(2);
const opt = (k: string, d: string) => (args.includes("--" + k) ? args[args.indexOf("--" + k) + 1] : d);
const name = opt("name", "watch");
const seconds = Number(opt("seconds", "120"));
const every = Number(opt("every", "10"));
const params: Record<string, string> = {};
args.forEach((a, i) => {
	if (a === "--param") {
		const [k, v] = args[i + 1].split("=");
		params[k] = v;
	}
});
const run = `${name}-${Date.now().toString(36)}`;
const out = path.join(here, "..", "out", name);

const s = await launch(run, PROFILES.unthrottled, false);
console.log("browser pid", s.pid, "run", run);
const errors: string[] = [];
s.page.on("console", (m) => errors.push(`${m.type()}: ${m.text()}`));
s.page.on("pageerror", (e) => errors.push("pageerror: " + e.message));
try {
	await s.page.goto(pageUrl(run, params));
	await s.page.click("[data-start]");
	const t0 = Date.now();
	let last = "";
	const timeline: unknown[] = [];
	let n = 0;
	while ((Date.now() - t0) / 1000 < seconds) {
		await sleep(every * 1000);
		const f = await grab(s.page);
		const st = await stats(run);
		const ps = await s.page.evaluate(() => {
			const t = (window as any).tryReactos;
			return { mips: t.stats.mips, sizes: t.stats.screenSizes.slice(-2), ide: t.stats.ideReadBytes, mouse: t.stats.mouseEnabled };
		});
		const t = Math.round((Date.now() - t0) / 1000);
		const changed = f && f.hash !== last;
		if (f) {
			last = f.hash;
			save(out, `${String(n++).padStart(3, "0")}-t${t}.png`, f.png);
		}
		timeline.push({ t, hash: f?.hash, w: f?.w, h: f?.h, bytes: st?.bytes, requests: st?.requests, ...ps });
		console.log(t + "s", f ? `${f.w}x${f.h} ${f.hash}${changed ? "*" : ""}` : "no canvas", "served", st?.bytes, "mips", ps.mips.toFixed(0), JSON.stringify(ps.sizes));
	}
	save(out, "timeline.json", JSON.stringify({ run, params, timeline, errors }, null, 1));
} finally {
	await flushLog();
	console.log(errors.slice(0, 15).join("\n"));
	await s.close();
}
