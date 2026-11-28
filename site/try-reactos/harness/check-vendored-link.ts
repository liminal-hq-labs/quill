// The vendored host link in the page (Level 4, no image build): the staged ReactOS desktop restored in headless Chromium or Firefox with the page's own
// host-link.js importing vendor/v86-host-link/host-link.js, a CD made from the vendored guest programs, D:\HOSTLINK.EXE started from the Run box, then the
// Link row (the helper's caps and modules), the clipboard round trip with Notepad and the origin of every request. The staged state was saved with COM1's
// interrupt pending; vendor-lib.ts clears it after the restore, in the harness only.
// Needs staged data/ and web-manifest.json (tools/stage-web.py) and the server: PORT=8170 IMAGES_DIR=DIR bun serve.ts (the CD goes into DIR).
// Usage: BASE=http://127.0.0.1:8170 IMAGES_DIR=DIR [BROWSER=firefox] bun check-vendored-link.ts OUTDIR     (unset WAYLAND_DISPLAY)
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import os from "node:os";
import path from "node:path";
import { KEY, guestTools, insertCd, makeTestCd, offOrigin, openBrowser, pageUrl, save, sleep, startStagedDesktop, waitFor, grab } from "./vendor-lib.ts";

const outDir = process.argv[2];
if (!outDir) throw new Error("usage: check-vendored-link.ts OUTDIR");
const browserName = process.env.BROWSER === "firefox" ? "firefox" : "chromium";
const imagesDir = process.env.IMAGES_DIR || path.join(os.homedir(), ".cache/quill-vend-images");
const failures: string[] = [];
const report: Record<string, unknown> = {};
const expect = (ok: boolean, message: string) => {
	if (!ok) failures.push(message);
	console.log((ok ? "ok   " : "FAIL ") + message);
};

const cdUrl = makeTestCd(imagesDir);
const { browser, page } = await openBrowser(browserName);
const requests: string[] = [];
page.on("request", (r) => requests.push(r.url()));
page.on("pageerror", (e) => failures.push("page error: " + String(e)));
const { press, typeText, run } = guestTools(page);
const ev = <T>(expression: string): Promise<T> => page.evaluate(expression) as Promise<T>;
const writes = () => ev<string[]>("window.__clip.writes");
const shot = async (name: string) => {
	const frame = await grab(page);
	if (frame) save(outDir, `${browserName}-${name}.png`, frame.png);
};
try {
	const started = await startStagedDesktop(page, pageUrl("", {}, { plain: true }));
	report.desktopMs = started.ms;
	await page.evaluate(() => ((document.querySelector("[data-diag]") as HTMLDetailsElement).open = true));
	expect((await ev<string>("document.querySelector('[data-link-readout] dd').textContent")).length > 0, "the Link row exists");

	// The helper from the CD.
	await insertCd(page, cdUrl);
	await sleep(6000);
	await run("D:\\HOSTLINK.EXE");
	const up = await waitFor(page, "window.tryReactos.hostLink && window.tryReactos.hostLink.stats().state === 'ready'", 90000, 100);
	expect(up !== null, "the helper from the CD said hello (the link is up)");
	const stats = await ev<any>("window.tryReactos.hostLink.stats()");
	report.peer = stats.peer;
	const caps: string[] = stats.peer?.caps ?? [];
	expect(/^HOSTLINK/.test(stats.peer?.app ?? "") && stats.peer?.proto === 1, `the helper is ${stats.peer?.app}, protocol ${stats.peer?.proto}`);
	expect(caps.includes("clip"), `its caps: ${caps.join(" ")}`);
	report.mods = stats.peer?.mods ?? null;
	const link = await ev<string>("document.querySelector('[data-link-readout] dd').textContent");
	report.linkRow = link;
	expect(/HOSTLINK/.test(link) && /protocol 1, offers clip/.test(link) && /modules HLAUDIO/.test(link), `the Link row reads: ${link.slice(0, 120)}`);
	await shot("01-link-up");
	await save(outDir, `${browserName}-01-panel.png`, await page.locator("[data-diag]").screenshot());

	// The clipboard with Notepad: the guest's copy reaches the page, the page's text reaches the guest, and comes back.
	await page.click("[data-stage]");
	await sleep(300);
	await run("notepad");
	await sleep(8000);
	await typeText("Hello from Notepad", 70);
	await sleep(800);
	const before = (await writes()).length;
	await page.keyboard.press("Control+a");
	await sleep(300);
	await page.keyboard.press("Control+c");
	await waitFor(page, `window.__clip.writes.length > ${before}`, 12000, 50);
	const copied = (await writes())[before];
	expect(copied === "Hello from Notepad", `Notepad's copy reached the page: ${JSON.stringify(copied)}`);
	await shot("02-notepad-copied");

	const sample = "Zoë café — 5 €";
	// The page's paste: the text is on the browser's clipboard, and the visitor's Ctrl+V sends it to the guest (then the guest pastes it).
	await page.evaluate((t: string) => navigator.clipboard.writeText(t), sample);
	await page.keyboard.press("Control+a");
	await sleep(300);
	await page.keyboard.press("Control+v");
	await sleep(3500);
	expect((await ev<any>("window.tryReactos.clipboard.status()")).counters.pastedIn >= 1, "the page sent its text to the guest");
	await shot("03-notepad-pasted");
	const mark = (await writes()).length;
	await page.keyboard.press("Control+a");
	await sleep(300);
	await page.keyboard.press("Control+c");
	await waitFor(page, `window.__clip.writes.length > ${mark}`, 12000, 50);
	const back = (await writes())[mark];
	expect(back === sample, `the text came back out of Notepad unchanged: ${JSON.stringify(back)}`);

	const off = offOrigin(requests);
	report.offOrigin = off;
	expect(off.length === 0, `no request left the page's origin (${requests.length} requests)`);
	expect(requests.some((u) => u.includes("/vendor/v86-host-link/host-link.js")), "the page loaded the vendored module");
	expect(!requests.some((u) => u.includes("/lib/host-link.js")), "the page did not load lib/host-link.js");
} catch (error) {
	failures.push(String(error));
	console.error(error);
} finally {
	save(outDir, `${browserName}-report.json`, JSON.stringify({ report, failures }, null, 2));
	await browser.close();
}
console.log(`${browserName}: ${failures.length === 0 ? "all checks passed" : failures.length + " failed"}`);
process.exit(failures.length === 0 ? 0 : 1);
