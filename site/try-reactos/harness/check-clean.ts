// Checks the clean address (SITE-14): /try-reactos/ with no query string boots the saved desktop from web-manifest.json and leaves the
// address alone; fields in the address bar override the manifest; a missing manifest and a manifest for another emulator build stop
// with a plain message. Needs `tools/stage-web.py` to have staged data/ and web-manifest.json, and the server (bun serve.ts) running.
// Pixels come from the emulator canvas and a Playwright screenshot of the page, never the host desktop. Exits non-zero on a failed check.
// Usage: BASE=http://127.0.0.1:8095 bun check-clean.ts OUTDIR
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import fs from "node:fs";
import { fileURLToPath } from "node:url";
import { BASE, PROFILES, grab, launch, pageUrl, save, sleep, waitFor } from "./lib.ts";

const outDir = process.argv[2];
if (!outDir) throw new Error("usage: check-clean.ts OUTDIR");

const whiteFraction =
	"(() => { const c = window.tryReactos.canvas(); if (!c || !c.width) return 0; const d = c.getContext('2d').getImageData(0, 0, c.width, c.height).data; let n = 0; for (let i = 0; i < d.length; i += 16) if (d[i] > 250 && d[i+1] > 250 && d[i+2] > 250) n++; return n / (d.length / 16); })()";

const failures: string[] = [];
const expect = (ok: boolean, message: string) => {
	if (!ok) failures.push(message);
};
const field = (page: any, name: string) => page.evaluate((n: string) => (document.querySelector(`[data-config] [name=${n}]`) as HTMLInputElement).value, name);
const settled = (page: any) => page.waitForFunction("document.querySelector('[data-build]').textContent !== 'Build: loading'", null, { timeout: 15000 });

const report: Record<string, unknown> = {};
const session = await launch("", PROFILES.unthrottled, false);
const page = session.page;
const staleFile = fileURLToPath(new URL("../data/stale-check-manifest.json", import.meta.url));
try {
	// 1. The plain address: no query string at all.
	const plain = pageUrl("", {}, { plain: true });
	expect(plain.endsWith("/try-reactos/"), `the plain address is ${plain}`);
	await page.goto(plain);
	await settled(page);
	report.buildLine = await page.textContent("[data-build]");
	report.diskField = await field(page, "disk");
	report.stateField = await field(page, "state");
	expect(/^\/try-reactos\/data\/parts-[0-9a-f]{12}\/\.img\.zst$/.test(report.diskField as string), `the disk field is ${report.diskField}`);
	expect(/^\/try-reactos\/data\/state-[0-9a-f]{12}\.l22\.bin\.zst$/.test(report.stateField as string), `the state field is ${report.stateField}`);
	expect(/^Build \d{4}-/.test(report.buildLine as string), `the build line is ${report.buildLine}`);
	await page.click("[data-start]");
	const desktopMs = await waitFor(page, "window.__m.taskbarScore() > 0.3", 240000);
	expect(desktopMs !== null, "no themed taskbar after the restore");
	const quillMs = await waitFor(page, `${whiteFraction} > 0.3`, 60000);
	expect(quillMs !== null, "no Quill window (a large white document area) after the restore");
	await sleep(3000);
	report.addressAfterStart = page.url();
	expect(page.url() === plain, `the address changed to ${page.url()}`);
	expect(new URL(page.url()).search === "", "the page added a query string");
	const frame = await grab(page);
	if (frame) save(outDir, "clean-restored-canvas.png", frame.png);
	await page.evaluate(() => ((document.querySelector("[data-diag]") as HTMLDetailsElement).open = true));
	save(outDir, "clean-address.png", await page.screenshot({ fullPage: true }));
	report.desktopMs = desktopMs;
	report.quillMs = quillMs;

	// Start works again after Stop, from the same plain address (the manifest is not read twice and the address stays).
	await page.evaluate("window.tryReactos.stop()");
	await page.click("[data-start]");
	expect((await waitFor(page, "window.__m.taskbarScore() > 0.3", 240000)) !== null, "no desktop after Stop and Start");
	expect(page.url() === plain, "the address changed after Stop and Start");
	await page.evaluate("window.tryReactos.stop()");

	// 2. Fields in the address bar override their manifest values; the others still come from the manifest.
	await page.goto(pageUrl("", { state: "/images/other.l22.bin.zst" }, { plain: true }));
	await settled(page);
	expect((await field(page, "state")) === "/images/other.l22.bin.zst", "?state= did not override the manifest's state");
	expect((await field(page, "disk")) === report.diskField, "the disk was not taken from the manifest when only ?state= was given");
	await page.goto(pageUrl("", { state: "" }, { plain: true }));
	await settled(page);
	expect((await field(page, "state")) === "", "an empty ?state= did not switch the saved state off");
	await page.goto(pageUrl("", { manifest: "none" }, { plain: true }));
	await settled(page);
	expect((await field(page, "disk")) === "" && (await field(page, "state")) === "", "?manifest=none still filled the panel");

	// 3. A manifest that is not there: a plain message, the panel open.
	await page.goto(pageUrl("", { manifest: "no-such-manifest.json" }, { plain: true }));
	await settled(page);
	const missing = await page.textContent("[data-status]");
	report.missing = missing;
	expect(/no saved desktop/.test(missing ?? ""), `the missing-manifest message is ${missing}`);
	expect(await page.evaluate(() => (document.querySelector("[data-diag]") as HTMLDetailsElement).open), "the panel did not open for a missing manifest");

	// 4. A manifest whose emulator hashes differ from the files the page has: refuse, in plain words.
	const real = JSON.parse(fs.readFileSync(fileURLToPath(new URL("../web-manifest.json", import.meta.url)), "utf8"));
	real.v86.libv86 = "deadbeef";
	fs.writeFileSync(staleFile, JSON.stringify(real));
	await page.goto(pageUrl("", { manifest: "data/stale-check-manifest.json" }, { plain: true }));
	await settled(page);
	await page.click("[data-start]");
	await page.waitForFunction("document.querySelector('[data-status]').textContent.startsWith('Could not start')", null, { timeout: 30000 });
	const refused = await page.textContent("[data-status]");
	report.refused = refused;
	expect(/different build of the emulator/.test(refused ?? ""), `the stale-emulator message is ${refused}`);
	expect(await page.evaluate("window.tryReactos.emulator === null"), "an emulator started with a stale manifest");
	console.log(JSON.stringify({ ...report, failures }, null, 1));
} finally {
	fs.rmSync(staleFile, { force: true });
	await session.close();
}
if (failures.length > 0) {
	console.error(failures.join("\n"));
	process.exit(1);
}
void BASE;
