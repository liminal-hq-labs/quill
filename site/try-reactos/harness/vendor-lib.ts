// Shared pieces of the vendored host link's checks: a headless Chromium or Firefox on the staged ReactOS desktop, a CD made from the vendored guest
// programs, the Run box, and the page's clipboard writes. Input reaches the guest only through the page's emulator API; pixels come from its canvas.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import fs from "node:fs";
import path from "node:path";
import { execFileSync } from "node:child_process";
import { chromium, firefox } from "playwright-core";
import type { Browser, BrowserContext, Page } from "playwright-core";
import { BASE, CHROMIUM, grab, pageUrl, save, sleep, waitFor } from "./lib.ts";

export { BASE, grab, pageUrl, save, sleep, waitFor };
export const vendorDir = path.resolve(new URL("../vendor/v86-host-link", import.meta.url).pathname);
export const KEY = { ctrl: [0x1d], a: [0x1e], c: [0x2e], v: [0x2f], enter: [0x1c], esc: [0x01], win: [0xe0, 0x5b], r: [0x13], alt: [0x38] };

/** Makes a CD from the vendored guest programs (as `D:\`), under the server's images folder, and returns its address on the server. */
export function makeTestCd(imagesDir: string): string {
	const stage = path.join(imagesDir, "vend-test-cd");
	fs.rmSync(stage, { recursive: true, force: true });
	fs.mkdirSync(stage, { recursive: true });
	for (const name of fs.readdirSync(path.join(vendorDir, "guest"))) fs.copyFileSync(path.join(vendorDir, "guest", name), path.join(stage, name));
	execFileSync("xorriso", ["-as", "mkisofs", "-quiet", "-J", "-r", "-V", "HLTEST", "-o", path.join(imagesDir, "vend-test.iso"), stage], { stdio: "inherit" });
	fs.rmSync(stage, { recursive: true, force: true });
	return "/images/vend-test.iso";
}

export async function openBrowser(browserName: "chromium" | "firefox"): Promise<{ browser: Browser; context: BrowserContext; page: Page }> {
	const browser =
		browserName === "firefox"
			? await firefox.launch({ headless: true, firefoxUserPrefs: { "dom.events.asyncClipboard.clipboardItem": true, "dom.events.testing.asyncClipboard": true, "dom.events.asyncClipboard.readText": true, "dom.events.asyncClipboard.read": true } })
			: await chromium.launch({ executablePath: CHROMIUM, headless: true, args: ["--no-first-run", "--disable-background-timer-throttling", "--disable-renderer-backgrounding"] });
	const context = await browser.newContext({ viewport: { width: 1280, height: 1100 }, serviceWorkers: "block", ...(browserName === "chromium" ? { permissions: ["clipboard-read", "clipboard-write"] } : {}) });
	await context.addInitScript({ path: new URL("./inpage.js", import.meta.url).pathname });
	// What the page writes to the browser's clipboard, so a check can say what Notepad's copy produced.
	await context.addInitScript(() => {
		const w = window as any;
		w.__clip = { writes: [] as string[] };
		const clipboard = navigator.clipboard;
		if (!clipboard) return;
		const write = clipboard.write.bind(clipboard);
		clipboard.write = (items: ClipboardItem[]) => {
			for (const item of items) if (item.types.includes("text/plain")) item.getType("text/plain").then((blob) => blob.text()).then((text) => w.__clip.writes.push(text));
			return write(items);
		};
	});
	return { browser, context, page: await context.newPage() };
}

export function guestTools(page: Page) {
	const press = (codes: number[][]) =>
		page.evaluate((keys: number[][]) => {
			for (const code of keys) window.tryReactos.input.key(code);
			for (const code of [...keys].reverse()) window.tryReactos.input.key(code.map((value: number) => value | 0x80));
		}, codes);
	const typeText = async (text: string, pause = 110) => {
		for (const ch of text) {
			await page.evaluate((c: string) => window.tryReactos.input.text(c), ch);
			await sleep(pause);
		}
	};
	/** The Run box: Win down, R, Win up, then the command typed slowly. */
	const run = async (command: string) => {
		await press([KEY.win, KEY.r]);
		await sleep(3000);
		await typeText(command);
		await sleep(400);
		await press([KEY.enter]);
	};
	return { press, typeText, run };
}

/**
 * Starts the machine from the staged desktop, waits for the taskbar, then clears the interrupt COM1 was saved with (the staged state has one pending, and
 * a port with an interrupt pending never reads what the page sends). This is the harness's own doing, through v86's internals: the page never does it.
 */
export async function startStagedDesktop(page: Page, url: string): Promise<{ ms: number }> {
	await page.goto(url);
	await page.waitForFunction("document.querySelector('[data-build]').textContent !== 'Build: loading'", null, { timeout: 30000 });
	const t0 = Date.now();
	await page.evaluate(() => document.exitPointerLock && document.exitPointerLock());
	await page.click("[data-start]");
	const up = await waitFor(page, "window.__m.taskbarScore() > 0.3", 240000, 200);
	if (up === null) throw new Error("the staged desktop did not come up");
	await page.evaluate(() => {
		const cpu = (window.tryReactos.emulator as any).v86.cpu;
		const uart = cpu.devices.uart0;
		uart.ints = 0;
		uart.iir = 1;
		cpu.device_lower_irq(uart.irq);
	});
	await sleep(2000);
	return { ms: Date.now() - t0 };
}

/** Inserts the test CD (fetched from the server's images folder) as the PC's CD drive. */
export const insertCd = (page: Page, url: string) =>
	page.evaluate(async (u: string) => {
		const buffer = await (await fetch(u)).arrayBuffer();
		await window.tryReactos.emulator.set_cdrom({ buffer });
	}, url);

/** Requests that left the page's own origin (data and blob addresses stay inside the page). */
export const offOrigin = (urls: string[]) => urls.filter((u) => !u.startsWith(BASE) && !u.startsWith("blob:") && !u.startsWith("data:"));

declare global {
	interface Window {
		tryReactos: any;
	}
}
