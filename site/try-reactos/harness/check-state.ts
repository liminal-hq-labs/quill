// Restore a saved state over the disk parts in headless Chromium and check what the page shows: the themed desktop (taskbar), Quill's window
// with the demo letter, and that File > Open brings up a dialog. Pixels come from the emulator canvas, never from the host desktop; input goes
// through the page's own emulator API. Exits non-zero on a failed check.
// Usage: BASE=http://127.0.0.1:8095 bun check-state.ts OUTDIR [--param k=v ...]
//   defaults: state=/images/state.l22.bin.zst, disk=/images/parts/.img.zst, ram=256
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { BASE, PROFILES, grab, launch, pageUrl, save, sleep, waitFor } from "./lib.ts";

const args = process.argv.slice(2);
const outDir = args[0];
if (!outDir) throw new Error("usage: check-state.ts OUTDIR [--param k=v ...]");
const params: Record<string, string> = { state: "/images/state.l22.bin.zst", disk: "/images/parts/.img.zst", ram: "256" };
args.forEach((arg, index) => {
	if (arg === "--param") {
		const eq = args[index + 1].indexOf("=");
		params[args[index + 1].slice(0, eq)] = args[index + 1].slice(eq + 1);
	}
});

// A state built with the sound card has its registers beside it (build-state.sh); restore them when they are there.
if (!("audio" in params) && (await fetch(new URL("/images/state.audio.json", BASE), { method: "HEAD" }).then((r) => r.ok, () => false))) params.audio = "/images/state.audio.json";

const whiteFraction =
	"(() => { const c = window.tryReactos.canvas(); if (!c || !c.width) return 0; const d = c.getContext('2d').getImageData(0, 0, c.width, c.height).data; let n = 0; for (let i = 0; i < d.length; i += 16) if (d[i] > 250 && d[i+1] > 250 && d[i+2] > 250) n++; return n / (d.length / 16); })()";

const run = `check-state-${Date.now().toString(36)}`;
const session = await launch(run, PROFILES.unthrottled, false);
const failures: string[] = [];
const press = (codes: number[][]) =>
	session.page.evaluate((keys) => {
		for (const code of keys) window.tryReactos.input.key(code);
		for (const code of [...keys].reverse()) window.tryReactos.input.key(code.map((value: number) => value | 0x80));
	}, codes);
try {
	await session.page.goto(pageUrl(run, params));
	await session.page.click("[data-start]");
	const desktopMs = await waitFor(session.page, "window.__m.taskbarScore() > 0.3", 240000);
	if (desktopMs === null) failures.push("no themed taskbar after the restore");
	const quillMs = await waitFor(session.page, `${whiteFraction} > 0.3`, 60000);
	if (quillMs === null) failures.push("no Quill window (a large white document area) after the restore");
	await sleep(4000);
	// With the card's registers the restored machine has the card, and it took them.
	const card = await session.page.evaluate(() => {
		const h = window.tryReactos.soundHandle;
		return h ? { registered: h.stats().registered, stateError: h.stats().stateError, guestRate: h.stats().guestRate } : null;
	});
	if (params.audio && (!card || !card.registered || card.stateError)) failures.push("the sound card did not come back with the state: " + JSON.stringify(card));
	if (!params.audio && card) failures.push("a state without the card's registers got a card");
	const restored = await grab(session.page);
	if (restored) save(outDir, "restored.png", restored.png);
	const quillShare = await session.page.evaluate(whiteFraction);

	// File > Open: Alt+F, O. A dialog changes a large part of the screen and the white share falls or rises; compare the frames.
	const before = await session.page.evaluate(() => {
		const c = window.tryReactos.canvas();
		return Array.from(c.getContext("2d").getImageData(0, 0, c.width, c.height).data.filter((_: number, i: number) => i % 4 === 0 && i % 64 === 0));
	});
	await press([[0x38], [0x21]]);
	await sleep(1200);
	await press([[0x18]]);
	await sleep(6000);
	const opened = await grab(session.page);
	if (opened) save(outDir, "open-dialog.png", opened.png);
	const changedShare = await session.page.evaluate((reference: number[]) => {
		const c = window.tryReactos.canvas();
		const now = c.getContext("2d").getImageData(0, 0, c.width, c.height).data.filter((_: number, i: number) => i % 4 === 0 && i % 64 === 0);
		let changed = 0;
		for (let i = 0; i < reference.length; i++) if (Math.abs(reference[i] - now[i]) > 24) changed++;
		return changed / reference.length;
	}, before);
	if (changedShare < 0.02) failures.push(`File > Open changed only ${(changedShare * 100).toFixed(2)}% of the screen`);
	await press([[0x01]]);
	console.log(JSON.stringify({ desktopMs, quillMs, card, quillWhiteShare: quillShare, openDialogChangedShare: changedShare, failures }));
} finally {
	await session.close();
}
if (failures.length > 0) {
	console.error(failures.join("\n"));
	process.exit(1);
}
