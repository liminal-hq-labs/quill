// A disk-touching session from the saved state, to compare disk part sizes: restore, wait for the desktop, then open Quill's File menu and
// Open dialog, the Start menu and its Programs folder, Run, and page through the document. Prints one JSON line: seconds to the first
// frame and to the live desktop, distinct disk bytes fetched, requests, IDE reads. The scripted interaction in measure.ts reads no disk
// after a restore, so it cannot tell part sizes apart; this one does.
// Usage: BASE=http://127.0.0.1:8195 node chunk-session.ts --name N [--profile 50mbit|10mbit] --param disk=/images/parts/c8/.img.zst [--param k=v ...]
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { KEY, PROFILES, launch, pageUrl, sleep, stats, flushLog } from "./lib.ts";

const args = process.argv.slice(2);
const opt = (k: string, d: string) => (args.includes("--" + k) ? args[args.indexOf("--" + k) + 1] : d);
const profile = PROFILES[opt("profile", "unthrottled")];
const params: Record<string, string> = { state: "/images/state.l22.bin.zst", ram: "256" };
args.forEach((a, i) => {
	if (a === "--param") {
		const eq = args[i + 1].indexOf("=");
		params[args[i + 1].slice(0, eq)] = args[i + 1].slice(eq + 1);
	}
});
const run = `${opt("name", "chunk-session")}-${Date.now().toString(36)}`;
const s = await launch(run, profile, false);
const ev = <T>(f: string): Promise<T> => s.page.evaluate(f) as Promise<T>;
const press = (codes: number[][]) =>
	s.page.evaluate((cs) => {
		for (const c of cs) window.tryReactos.input.key(c);
		for (const c of [...cs].reverse()) window.tryReactos.input.key(c.map((v: number) => v | 0x80));
	}, codes);
const white =
	"(() => { const c = window.tryReactos.canvas(); if (!c || !c.width) return false; const d = c.getContext('2d').getImageData(0, 0, c.width, c.height).data; let n = 0; for (let i = 0; i < d.length; i += 16) if (d[i] > 250 && d[i+1] > 250 && d[i+2] > 250) n++; return n / (d.length / 16) > 0.3; })()";
try {
	await s.page.goto(pageUrl(run, params));
	const t0 = Date.now();
	await s.page.click("[data-start]");
	let firstFrame: number | null = null;
	let desktop: number | null = null;
	while (Date.now() - t0 < 300_000) {
		await sleep(100);
		if (firstFrame === null && (await ev<boolean>("(() => { const c = window.tryReactos.canvas(); return Boolean(c && c.width); })()"))) firstFrame = Date.now() - t0;
		if (await ev<boolean>(white)) {
			desktop = Date.now() - t0;
			break;
		}
	}
	if (desktop === null) throw new Error("no desktop");
	const afterRestore = await stats(run);
	// File > Open (Alt+F, O), Escape; Start menu (Win), Programs; Run (Win+R); page down a few times.
	await press([[0x38], [0x21]]);
	await sleep(1500);
	await press([[0x18]]);
	await sleep(6000);
	await press([[0x01]]);
	await sleep(1500);
	await press([[0x01]]);
	await press([KEY.win]);
	await sleep(3000);
	await press([[0x48]]);
	await press([[0x4d]]);
	await sleep(3000);
	await press([[0x01]]);
	await sleep(1000);
	await press([KEY.win, KEY.r]);
	await sleep(3000);
	await press([[0x01]]);
	for (let i = 0; i < 6; i++) {
		await press([KEY.pgdn]);
		await sleep(500);
	}
	await sleep(4000);
	const end = await stats(run);
	const ide = await ev<{ n: number; b: number }>("({ n: window.tryReactos.stats.ideReads, b: window.tryReactos.stats.ideReadBytes })");
	console.log(
		JSON.stringify({
			profile: profile.name,
			firstFrameS: firstFrame === null ? null : firstFrame / 1000,
			desktopS: desktop / 1000,
			stateBytes: afterRestore?.bytes,
			diskDistinctBytes: end?.diskDistinctBytes,
			diskBytes: end?.kinds?.disk?.bytes,
			diskRequests: end?.kinds?.disk?.requests,
			requests: end?.requests,
			servedBytes: end?.bytes,
			ideReads: ide.n,
			ideReadBytes: ide.b,
		}),
	);
} finally {
	await flushLog();
	await s.close();
}
