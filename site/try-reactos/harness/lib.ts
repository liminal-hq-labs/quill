// Shared helpers for the measurement scripts: launching headless Chromium, reading the server's counters and grabbing guest pixels.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import fs from "node:fs";
import path from "node:path";
import { createHash } from "node:crypto";
import { chromium } from "playwright-core";
import type { Browser, BrowserContext, CDPSession, Page } from "playwright-core";

export const BASE = process.env.BASE || "http://127.0.0.1:8086";
export const CHROMIUM = process.env.CHROMIUM || "/usr/bin/chromium";

export const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

export interface Session {
	browser: Browser;
	context: BrowserContext;
	page: Page;
	cdp: CDPSession;
	pid: number;
	bcdp: CDPSession;
	run: string;
	close: () => Promise<void>;
}

export interface Profile {
	name: string;
	// Kilobits per second, in each direction; 0 is unthrottled.
	kbps: number;
	latencyMs: number;
}

export const PROFILES: Record<string, Profile> = {
	unthrottled: { name: "unthrottled", kbps: 0, latencyMs: 0 },
	"50mbit": { name: "50mbit-40ms", kbps: 50_000, latencyMs: 40 },
	"10mbit": { name: "10mbit-80ms", kbps: 10_000, latencyMs: 80 },
};

export async function launch(run: string, profile: Profile, coi: boolean): Promise<Session> {
	const browser = await chromium.launch({
		executablePath: CHROMIUM,
		headless: true,
		args: ["--enable-precise-memory-info", "--js-flags=--expose-gc", "--no-first-run", "--disable-background-timer-throttling", "--disable-renderer-backgrounding"],
	});
	const context = await browser.newContext({ viewport: { width: 1280, height: 1000 }, serviceWorkers: "block" });
	await context.addInitScript({ path: new URL("./inpage.js", import.meta.url).pathname });
	const page = await context.newPage();
	const cdp = await context.newCDPSession(page);
	await cdp.send("Network.enable");
	await cdp.send("Network.setCacheDisabled", { cacheDisabled: true });
	if (profile.kbps > 0) {
		const bps = (profile.kbps * 1000) / 8;
		await cdp.send("Network.emulateNetworkConditions", { offline: false, latency: profile.latencyMs, downloadThroughput: bps, uploadThroughput: bps });
	}
	// The browser process id, so a stuck run can be ended by id and not by name.
	const bcdp = await browser.newBrowserCDPSession();
	const info: any = await bcdp.send("SystemInfo.getProcessInfo" as never).catch(() => null);
	const pid = info?.processInfo?.find((p: any) => p.type === "browser")?.id ?? 0;
	void coi;
	return {
		browser,
		context,
		page,
		cdp,
		pid,
		bcdp,
		run,
		close: async () => {
			await browser.close();
		},
	};
}

export async function stats(run: string): Promise<any> {
	const r = await fetch(`${BASE}/__stats?client=${encodeURIComponent(run)}&run=${encodeURIComponent(run)}`);
	const j: any = await r.json();
	return j.clients[run] ?? null;
}

export async function resetStats(run: string) {
	await fetch(`${BASE}/__stats/reset?client=${encodeURIComponent(run)}&run=${encodeURIComponent(run)}`, { method: "POST" });
}

export async function flushLog() {
	await fetch(`${BASE}/__stats/flush`, { method: "POST" });
}

// The guest's pixels: the emulator canvas as a PNG, plus a hash and a few region summaries.
export interface Frame {
	w: number;
	h: number;
	png: Buffer;
	hash: string;
}

export async function grab(page: Page): Promise<Frame | null> {
	const d = await page.evaluate(() => {
		const c = (window as any).tryReactos?.canvas() as HTMLCanvasElement | null;
		if (!c || !c.width || !c.height) return null;
		return { w: c.width, h: c.height, url: c.toDataURL("image/png") };
	});
	if (!d) return null;
	const png = Buffer.from(d.url.split(",")[1], "base64");
	return { w: d.w, h: d.h, png, hash: createHash("sha1").update(png).digest("hex").slice(0, 12) };
}

export function save(dir: string, name: string, data: Buffer | string) {
	fs.mkdirSync(dir, { recursive: true });
	fs.writeFileSync(path.join(dir, name), data);
}

export function pageUrl(run: string, params: Record<string, string>) {
	const q = new URLSearchParams({ run, ...params });
	return `${BASE}/try-reactos/?${q}`;
}

// Memory of the browser's processes, from /proc, by the process ids the DevTools protocol reports.
export async function processMemory(cdp: CDPSession): Promise<{ type: string; pid: number; rssMB: number }[]> {
	const info: any = await cdp.send("SystemInfo.getProcessInfo" as never).catch(() => null);
	const out = [];
	for (const p of info?.processInfo ?? []) {
		try {
			const s = fs.readFileSync(`/proc/${p.id}/status`, "utf8");
			const kb = Number(/VmRSS:\s+(\d+)/.exec(s)?.[1] ?? 0);
			out.push({ type: p.type as string, pid: p.id as number, rssMB: Math.round(kb / 102.4) / 10 });
		} catch {
			// The process has gone.
		}
	}
	return out;
}

export async function waitFor(page: Page, expr: string, timeoutMs: number, everyMs = 500): Promise<number | null> {
	const t0 = Date.now();
	while (Date.now() - t0 < timeoutMs) {
		if (await page.evaluate(expr)) return Date.now() - t0;
		await sleep(everyMs);
	}
	return null;
}

// Scancode set 1 make codes for the keys the scripts use.
export const KEY = {
	enter: [0x1c],
	esc: [0x01],
	tab: [0x0f],
	backspace: [0x0e],
	win: [0xe0, 0x5b],
	r: [0x13],
	ctrl: [0x1d],
	alt: [0x38],
	f4: [0x3e],
	pgdn: [0xe0, 0x51],
	pgup: [0xe0, 0x49],
	down: [0xe0, 0x50],
	up: [0xe0, 0x48],
	left: [0xe0, 0x4b],
	right: [0xe0, 0x4d],
};
