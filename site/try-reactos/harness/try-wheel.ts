// The wheel on the Try Quill page (SITE-111): the real page, the real js-dos and Quill, in headless Chromium and Firefox.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

// Usage: env -u WAYLAND_DISPLAY DISPLAY=:99 node try-wheel.ts [--browser chromium,firefox] [--mode gfx,g16,text] [--out DIR]
// BASE=http://127.0.0.1:PORT/try/ uses a server that is already running over site/ with a local bundle (`bash scripts/build-try.sh --build`);
// without it the script starts its own on a free port and stops it at the end. Both browsers run headless.
//
// What it checks, per browser and display mode (js-dos cannot send a wheel to DOSBox, so the page turns it into keys: SITE-111):
//   - with the pointer over the screen a wheel turn does not scroll the page (window.scrollY is the same) and Quill's frame changes (its
//     view scrolled), for a mouse's pixel steps, for Firefox's three-line steps and for a touchpad's small deltas;
//   - a notch is one Ctrl+Page Down (Ctrl+Page Up the other way), which DOS Quill answers by scrolling the view three lines and leaving the
//     caret where it is (SITE-113: the status line's Pg, Ln and Col do not change); a long spin is capped at 20 notches;
//   - with the pointer outside the screen the page scrolls and no key goes to the guest;
//   - Ctrl+wheel (the browser's zoom) is not taken: the event is not cancelled and no key goes to the guest;
//   - in Capture the wheel works the same.

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { chromium, firefox } from "playwright-core";
import type { Browser, Page } from "playwright-core";
// @ts-ignore: plain JavaScript module shared with the Node tests
import { startServer } from "../../../scripts/pointer/serve.mjs";

const here = path.dirname(fileURLToPath(import.meta.url));
const args = process.argv.slice(2);
const opt = (name: string, fallback: string) => (args.includes("--" + name) ? args[args.indexOf("--" + name) + 1] : fallback);
const browsers = opt("browser", "chromium").split(",");
const modes = opt("mode", "gfx,g16,text").split(",");
const outDir = path.resolve(opt("out", path.join(here, "..", "out", "try-wheel")));
fs.mkdirSync(path.join(outDir, "shots"), { recursive: true });

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

interface Check {
	browser: string;
	mode: string;
	name: string;
	pass: boolean;
	detail: string;
}
const checks: Check[] = [];

function check(browser: string, mode: string, name: string, pass: boolean, detail = "") {
	checks.push({ browser, mode, name, pass, detail });
	console.log(`${pass ? "ok  " : "FAIL"} ${browser} ${mode}: ${name}${detail ? " (" + detail + ")" : ""}`);
}

// js-dos key codes the page sends for the wheel.
const CTRL = 341;
const PGUP = 266;
const PGDN = 267;
// The keys come as pairs, Ctrl and then the page key, one pair a notch.
const pairs = (keys: number[], page: number) => keys.length > 0 && keys.length % 2 === 0 && keys.every((k, i) => k === (i % 2 === 0 ? CTRL : page));

// In the page: hand over the ci and log the keys the page sends through it; a stand-in for the pointer lock, which headless browsers refuse.
const INIT = () => {
	let real: any;
	(window as any).__keys = [];
	(window as any).__log = [];
	Object.defineProperty(window, "Dos", {
		configurable: true,
		get() {
			return (
				real &&
				function (host: any, opts: any) {
					const previous = opts.onEvent;
					opts.onEvent = (event: string, arg: any) => {
						if (event === "ci-ready") {
							(window as any).__ci = arg;
							const original = arg.simulateKeyPress.bind(arg);
							arg.simulateKeyPress = (...codes: number[]) => {
								if ((window as any).__logKeys) (window as any).__keys.push(...codes);
								return original(...codes);
							};
						}
						if (previous) previous(event, arg);
					};
					return real(host, opts);
				}
			);
		},
		set(value) {
			real = value;
		},
	});
	if (new URLSearchParams(location.search).get("lockstub") === "1") {
		let locked: Element | null = null;
		Object.defineProperty(document, "pointerLockElement", { configurable: true, get: () => locked });
		Element.prototype.requestPointerLock = function () {
			locked = this;
			document.dispatchEvent(new Event("pointerlockchange"));
			return Promise.resolve();
		} as any;
		document.exitPointerLock = () => {
			locked = null;
			document.dispatchEvent(new Event("pointerlockchange"));
		};
	}
};

// The page area of Quill's frame (between the top bars and the status line). A caret blinks, so a view that scrolled is told from a
// blink by how many pixels differ from a frame kept earlier: a scroll changes thousands, a caret dozens.
const FRAME_ROWS: Record<string, [number, number]> = { gfx: [150, 440], g16: [150, 440], text: [48, 368] };
// The status line, which says Pg, Ln and Col: the caret's place.
const STATUS_ROWS: Record<string, [number, number]> = { gfx: [462, 480], g16: [462, 480], text: [384, 400] };
const KEEP = (name: string) => `(async () => { (window).__kept = (window).__kept || {}; (window).__kept["${name}"] = await window.__ci.screenshot(); })()`;
const DIFF = (name: string, rows: [number, number]) => `(async () => {
	const a = (window).__kept["${name}"];
	const b = await window.__ci.screenshot();
	let n = 0;
	for (let y = ${rows[0]}; y < ${rows[1]}; y++) for (let x = 0; x < b.width; x++) {
		const i = (y * b.width + x) * 4;
		if (Math.abs(a.data[i] - b.data[i]) + Math.abs(a.data[i + 1] - b.data[i + 1]) + Math.abs(a.data[i + 2] - b.data[i + 2]) > 60) n++;
	}
	return n;
})()`;

// Polls `read` until `ok` says yes, up to `ms`; Quill takes its time over each key, more on a busy machine.
async function settle(read: () => Promise<number>, ok: (n: number) => boolean, ms = 20000) {
	const end = Date.now() + ms;
	let value = await read();
	while (!ok(value) && Date.now() < end) {
		await sleep(500);
		value = await read();
	}
	return value;
}

async function open(base: string, browser: Browser, key: string, query: string) {
	const page: Page = await browser.newPage({ viewport: { width: 1100, height: 900 }, deviceScaleFactor: 1 });
	await page.addInitScript(INIT);
	await page.goto(base + (query ? "?" + query : ""));
	await page.check(`input[name=mode][value=${key}]`);
	await page.click("[data-start]");
	await page.waitForFunction(() => (window as any).__ci, null, { timeout: 90000 });
	await sleep(14000);
	return page;
}

async function run(browserName: string, base: string) {
	const browser = await (browserName === "firefox" ? firefox.launch({ headless: true }) : chromium.launch({ executablePath: process.env.CHROMIUM || "/usr/bin/chromium", headless: true, args: ["--no-sandbox"] }));
	try {
		for (const key of modes) {
			console.log(`\n== ${browserName}, ${key}`);
			for (const capture of [false, true]) {
				const label = capture ? key + " capture" : key;
				const page = await open(base, browser, key, capture ? "mouse=capture&lockstub=1" : "mouse=follow");
				const status = (name: string) => page.evaluate(DIFF(name, STATUS_ROWS[key]) as any) as Promise<number>;
				const keep = (name: string) => page.evaluate(KEEP(name) as any);
				const diff = (name: string) => page.evaluate(DIFF(name, FRAME_ROWS[key]) as any) as Promise<number>;
				const keys = async () => (await page.evaluate(() => (window as any).__keys.splice(0))) as number[];
				const scrollY = () => page.evaluate(() => window.scrollY);
				await page.evaluate(() => ((window as any).__logKeys = true));
				// The screen in view, with room to scroll the page both ways.
				await page.evaluate(() => {
					const stage = document.querySelector("[data-stage]") as HTMLElement;
					// The page's style sheet may smooth scrolling; the test wants the place now.
					stage.scrollIntoView({ block: "start", behavior: "instant" });
					window.scrollBy({ top: -20, behavior: "instant" });
				});
				await sleep(300);
				const maxScroll = await page.evaluate(() => document.documentElement.scrollHeight - window.innerHeight);
				const box = (await page.locator("[data-dos] canvas").first().boundingBox())!;
				const inside = { x: box.x + box.width * 0.5, y: box.y + Math.min(box.height * 0.5, 400) };
				await page.mouse.move(inside.x, inside.y);
				if (capture) {
					await page.mouse.click(inside.x, inside.y);
					await sleep(300);
				}
				await keys();
				const before = await scrollY();
				await keep("top");
				check(browserName, label, "the page can scroll in this window", maxScroll > 300, `${maxScroll} pixels`);

				// A mouse over the screen: ten clicks of 100 pixels (Chromium's step), or Firefox's three lines each.
				const wheelOver = async (deltaY: number, deltaMode: number, times: number, gap = 40) => {
					for (let i = 0; i < times; i++) {
						await page.evaluate(
							([dy, dm, x, y]) => {
								const e = new WheelEvent("wheel", { deltaY: dy, deltaMode: dm, clientX: x, clientY: y, bubbles: true, cancelable: true });
								(document.elementFromPoint(x, y) as Element).dispatchEvent(e);
								(window as any).__lastWheelPrevented = e.defaultPrevented;
							},
							[deltaY, deltaMode, inside.x, inside.y],
						);
						await sleep(gap);
					}
					// The keys go one every 70 ms and Quill takes time over each.
					await sleep(3000);
				};

				// The real wheel of the browser's driver first: the page must not move.
				await page.mouse.wheel(0, 300);
				await sleep(3000);
				check(browserName, label, "the driver's wheel over the screen does not scroll the page", (await scrollY()) === before, `scrollY ${before} to ${await scrollY()}`);
				const sent = await keys();
				check(browserName, label, "and goes to the guest as Ctrl+Page Down, six notches", pairs(sent, PGDN) && sent.length === 12, `${sent.length} codes, first ${sent.slice(0, 6).join(",")}`);

				// Three lines (Firefox's click) is one notch, one Ctrl+Page Down.
				await wheelOver(3, 1, 1);
				const one = await keys();
				check(browserName, label, "one click of three lines is one notch: one Ctrl+Page Down", pairs(one, PGDN) && one.length === 2, `${one.join(",")}`);
				check(browserName, label, "the event is cancelled", await page.evaluate(() => (window as any).__lastWheelPrevented));

				// A touchpad: 40 small deltas of 3 pixels make 120 pixels, two notches and a bit.
				await wheelOver(3, 0, 40);
				const pad = await keys();
				check(browserName, label, "a touchpad's small deltas add up to notches", pairs(pad, PGDN) && pad.length === 4, `${pad.length} codes`);

				// A long spin (events 4 ms apart): 24 notches asked, 20 owed at most.
				await wheelOver(100, 0, 1);
				await keys();
				await wheelOver(100, 0, 12, 4);
				const spin = await keys();
				check(browserName, label, "a long spin is capped (20 owed at most)", pairs(spin, PGDN) && spin.length < 48, `${spin.length / 2} notches of the 24 asked`);

				// Quill's view moved, and the page did not.
				await wheelOver(100, 0, 10);
				await keys();
				const moved = await settle(() => diff("top"), (n) => n > 1500);
				check(browserName, label, "Quill's frame changed (its view scrolled)", moved > 1500, `${moved} pixels differ from the first frame`);
				const caretMoved = await status("top");
				check(browserName, label, "the caret did not move (the status line is the same)", caretMoved === 0, `${caretMoved} pixels of the status line differ`);
				await keep("down");
				check(browserName, label, "the page did not scroll through all of it", (await scrollY()) === before, `scrollY ${await scrollY()}, was ${before}`);
				await page.locator("[data-stage]").screenshot({ path: path.join(outDir, "shots", `after-down-${browserName}-${label.replace(" ", "-")}.png`) });

				// The other way: Up keys, and the frame comes back toward where it was.
				await wheelOver(-100, 0, 3);
				const up = await keys();
				check(browserName, label, "the other way sends Ctrl+Page Up", pairs(up, PGUP), `${up.length / 2} notches`);
				await wheelOver(-100, 0, 30);
				const upKeys = await keys();
				await page.locator("[data-stage]").screenshot({ path: path.join(outDir, "shots", `after-up-${browserName}-${label.replace(" ", "-")}.png`) });
				let back = await settle(() => diff("down"), (n) => n > 1500);
				let note = "";
				if (back <= 1500) {
					// Seen in two of twelve variants of one long run (and in none of five shorter ones): the first spin up changed nothing. One more spin tells
					// whether the guest still takes keys; both results are reported.
					console.log(`     the first spin up changed ${back} pixels (${upKeys.length / 2} notches sent); spinning again`);
					await wheelOver(-100, 0, 12, 4);
					await keys();
					back = await settle(() => diff("down"), (n) => n > 1500);
					note = "; after a second spin";
				}
				check(browserName, label, "and the view scrolls back", back > 1500, `${back} pixels differ from the frame after the downs${note}`);

				// Ctrl+wheel is the browser's own: not cancelled, nothing sent.
				const ctrl = await page.evaluate(
					([x, y]) => {
						const e = new WheelEvent("wheel", { deltaY: -400, ctrlKey: true, clientX: x, clientY: y, bubbles: true, cancelable: true });
						(document.elementFromPoint(x, y) as Element).dispatchEvent(e);
						return e.defaultPrevented;
					},
					[inside.x, inside.y],
				);
				await sleep(500);
				check(browserName, label, "Ctrl+wheel is not cancelled and sends no key", !ctrl && (await keys()).length === 0);

				// Outside the screen: the page scrolls and no key goes.
				const outside = { x: 20, y: Math.max(5, box.y - 12) };
				await page.mouse.move(outside.x, outside.y);
				await page.evaluate(() => window.scrollTo({ top: 0, behavior: "instant" }));
				await sleep(200);
				const top = await scrollY();
				await page.mouse.wheel(0, 200);
				await sleep(800);
				check(browserName, label, "outside the screen the wheel scrolls the page", (await scrollY()) > top, `scrollY ${top} to ${await scrollY()}`);
				check(browserName, label, "and sends no key to the guest", (await keys()).length === 0);
				await page.close();
			}
		}
	} finally {
		await browser.close();
	}
}

async function main() {
	let stop: (() => void) | null = null;
	let base = process.env.BASE || "";
	if (!base) {
		const { server, base: origin } = await startServer({ port: 0 });
		base = origin + "/try/";
		stop = () => server.close();
	}
	try {
		for (const name of browsers) await run(name, base);
	} finally {
		fs.writeFileSync(path.join(outDir, "try-wheel.json"), JSON.stringify({ date: new Date().toISOString(), checks }, null, 1));
		if (stop) stop();
	}
	const failed = checks.filter((c) => !c.pass);
	console.log(`\n${checks.length - failed.length} of ${checks.length} checks passed`);
	if (failed.length) process.exit(1);
}

await main();
