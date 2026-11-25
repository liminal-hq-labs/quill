// Page screenshots in a phone-sized and a desktop-sized headless Chromium: idle, running, pointer locked (with ?cursor=local), after Reset.
// Usage: BASE=http://127.0.0.1:8089 node shots.ts OUTDIR [--phone | --desktop] [--param k=v ...]
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import fs from "node:fs";
import path from "node:path";
import { chromium } from "playwright-core";
import { BASE, CHROMIUM, sleep } from "./lib.ts";

const args = process.argv.slice(2);
const dir = args[0];
fs.mkdirSync(dir, { recursive: true });
const phone = args.includes("--phone");
const params: Record<string, string> = { state: "/images/state-ag2.l22.bin.zst", disk: "/images/reactos16-afterglow2.img", ram: "256", run: "shots" + Date.now().toString(36) };
args.forEach((a, i) => {
	if (a === "--param") {
		const eq = args[i + 1].indexOf("=");
		params[args[i + 1].slice(0, eq)] = args[i + 1].slice(eq + 1);
	}
});
const name = phone ? "phone" : "desktop";
const browser = await chromium.launch({ executablePath: CHROMIUM, headless: true, args: ["--no-first-run"] });
const context = await browser.newContext(phone ? { viewport: { width: 360, height: 780 }, deviceScaleFactor: 2.8, isMobile: true, hasTouch: true } : { viewport: { width: 1280, height: 900 }, deviceScaleFactor: 1.5 });
await context.addInitScript({ path: new URL("./inpage.js", import.meta.url).pathname });
const page = await context.newPage();
try {
	await page.goto(`${BASE}/try-reactos/?${new URLSearchParams(params)}`);
	await page.screenshot({ path: path.join(dir, `${name}-1-idle.png`), fullPage: true });
	await page.click("[data-start]");
	await page.waitForFunction("window.__m.taskbarScore() > 0.15", null, { timeout: 120000 });
	await sleep(2000);
	await page.screenshot({ path: path.join(dir, `${name}-2-running.png`), fullPage: true });
	if (!phone) {
		const box = await page.locator("[data-stage]").boundingBox();
		await page.mouse.click(box!.x + box!.width / 2, box!.y + box!.height / 2);
		await sleep(1500);
		await page.mouse.move(box!.x + box!.width / 2 + 80, box!.y + box!.height / 2 + 40);
		await sleep(500);
		await page.screenshot({ path: path.join(dir, `${name}-3-locked.png`), fullPage: true });
	}
	await page.evaluate(() => document.querySelector("details")!.setAttribute("open", ""));
	await page.screenshot({ path: path.join(dir, `${name}-4-diag-open.png`), fullPage: true });
	await page.evaluate(() => document.querySelector("details")!.removeAttribute("open"));
	await page.evaluate(() => document.exitPointerLock());
	await sleep(500);
	await page.click("[data-reset]");
	await sleep(1500);
	await page.waitForFunction("window.__m.taskbarScore() > 0.15", null, { timeout: 60000 });
	await sleep(1500);
	await page.screenshot({ path: path.join(dir, `${name}-5-after-reset.png`), fullPage: true });
	console.log("ok", await page.evaluate("document.querySelector('[data-status]').textContent"));
} finally {
	await browser.close();
}
