// Levels 2 and 3 of the pointer's tests on the Try Quill page: the real page, the real js-dos and Quill, in headless Chromium and Firefox.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

// Usage: env -u WAYLAND_DISPLAY DISPLAY=:99 node try-pointer.ts [--browser chromium,firefox] [--mode gfx,g16,text] [--out DIR] [--quick]
// BASE=http://127.0.0.1:PORT/try/ uses a server that is already running over site/ with a local bundle (`bash scripts/build-try.sh --build`);
// without it the script starts its own on a free port and stops it at the end. Chromium is /usr/bin/chromium (CHROMIUM overrides); Firefox is
// Playwright's build. Both run headless; nothing is shown on or typed into the host desktop.
//
// The page is driven through the real mouse of Playwright's browser (fractional positions in Chromium; Firefox's driver rounds them, so its
// moves are dispatched PointerEvents with fractional clientX) and the guest is read in js-dos's own frames (ci.screenshot()): Quill's pointer
// is found by the difference against a frame taken with it parked at a known place (jsdos-inpage.js). Level 2 is the calls the page makes to
// `ci` (every sendMouseRelativeMotion and sendMouseButton is logged) and the direction error of the page's trace; level 3 is the pointer in the
// guest's pixels at stops along slow straight strokes, at the edges, after leaving and coming back, and after a button is released outside.
// Capture is tested with dispatched movementX and a stand-in requestPointerLock, because neither headless browser can lock the pointer.
//
// Pass lines (docs/plans/pointer.md, "Measuring it"): every stop within 2 guest pixels (one cell in text mode) of the pixel under the visitor's
// pointer once the pointer's fixed hotspot offset is removed; p95 direction error at most 10 degrees at 8 guest pixels (15 in text mode, whose
// pointer moves a cell, 8 by 16 pixels of the frame, at a time); at most 2 calls an
// animation frame in Follow; no button left down after a release outside; the edge reached from outside; js-dos's overlay gone; Capture moves
// the guest as far as the movement says, and twice as far at speed 2.

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { chromium, firefox } from "playwright-core";
import type { Browser, Page } from "playwright-core";
// @ts-ignore: plain JavaScript modules shared with the Node tests
import { startServer } from "../../../scripts/pointer/serve.mjs";

const here = path.dirname(fileURLToPath(import.meta.url));
const args = process.argv.slice(2);
const opt = (name: string, fallback: string) => (args.includes("--" + name) ? args[args.indexOf("--" + name) + 1] : fallback);
const browsers = opt("browser", "chromium").split(",");
const modes = opt("mode", "gfx,g16,text").split(",");
const quick = args.includes("--quick");
const outDir = path.resolve(opt("out", path.join(here, "..", "out", "try-pointer")));
fs.mkdirSync(path.join(outDir, "shots"), { recursive: true });
await import("../../../site/js/pointer-metric.js");
const { scoreTrace } = (globalThis as any).QuillPointerMetric;

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

interface Check {
	browser: string;
	mode: string;
	name: string;
	pass: boolean;
	detail: string;
}
const checks: Check[] = [];
const numbers: any[] = [];

// In the page: hand over the ci and log what the page sends through it; a stand-in for the pointer lock, which headless browsers refuse.
const INIT = () => {
	let real: any;
	(window as any).__calls = [];
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
							for (const kind of ["sendMouseRelativeMotion", "sendMouseButton", "sendMouseMotion"]) {
								const original = arg[kind].bind(arg);
								arg[kind] = (a: number, b: number) => {
									(window as any).__calls.push({ t: performance.now(), kind, a, b });
									return original(a, b);
								};
							}
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

interface Mode {
	key: string;
	frame: [number, number];
	window: [number, number];
	rows: [number, number];
	mask: [number, number];
	cell: [number, number];
	// How far (in frame pixels, after the hotspot offset is removed) a stop may be from where the pointer under the visitor's is.
	tolerance: [number, number];
}
const MODES: Record<string, Mode> = {
	gfx: { key: "gfx", frame: [640, 480], window: [22, 26], rows: [96, 440], mask: [30, 34], cell: [1, 1], tolerance: [2, 2] },
	g16: { key: "g16", frame: [640, 480], window: [22, 26], rows: [96, 440], mask: [30, 34], cell: [1, 1], tolerance: [2, 2] },
	text: { key: "text", frame: [640, 400], window: [20, 18], rows: [32, 380], mask: [24, 40], cell: [8, 16], tolerance: [8, 16] },
};

function check(browser: string, mode: string, name: string, pass: boolean, detail = "") {
	checks.push({ browser, mode, name, pass, detail });
	console.log(`${pass ? "ok  " : "FAIL"} ${browser} ${mode}: ${name}${detail ? " (" + detail + ")" : ""}`);
}

const median = (values: number[]) => [...values].sort((a, b) => a - b)[Math.floor(values.length / 2)];

async function open(base: string, browser: Browser, name: string, key: string, query: string) {
	const page: Page = await browser.newPage({ viewport: { width: 1100, height: 1000 }, deviceScaleFactor: 1.5 });
	await page.addInitScript(INIT);
	await page.addInitScript({ path: path.join(here, "jsdos-inpage.js") });
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
			const mode = MODES[key];
			console.log(`\n== ${browserName}, ${key}`);
			const page = await open(base, browser, browserName, key, "pointer=debug&mouse=follow");
			const box = (await page.locator("[data-dos] canvas").first().boundingBox())!;
			const stage = (await page.locator("[data-stage]").boundingBox())!;
			const cx = (fx: number) => box.x + fx * box.width;
			const cy = (fy: number) => box.y + fy * box.height;
			const dispatched = browserName === "firefox";
			const moveTo = async (x: number, y: number) => {
				if (!dispatched) return page.mouse.move(x, y);
				await page.evaluate(([px, py]) => {
					const stageEl = document.querySelector("[data-stage]")!;
					stageEl.dispatchEvent(new PointerEvent("pointermove", { clientX: px, clientY: py, pointerType: "mouse", pointerId: 1, isPrimary: true, bubbles: true, cancelable: true }));
				}, [x, y]);
			};
			const stroke = async (x0: number, y0: number, x1: number, y1: number, steps: number, gap: number) => {
				for (let i = 0; i <= steps; i++) {
					await moveTo(x0 + ((x1 - x0) * i) / steps, y0 + ((y1 - y0) * i) / steps);
					if (gap) await sleep(gap);
				}
			};
			const locate = () => page.evaluate(([w, h, top, bottom]) => (window as any).__spike.settled(w, h, top, bottom), [...mode.window, ...mode.rows]);
			const expected = (x: number, y: number) => ({ x: Math.floor(((x - box.x) / box.width) * mode.frame[0]), y: Math.floor(((y - box.y) / box.height) * mode.frame[1]) });

			// js-dos's own cover and handlers: no "click to capture" text, and the stage still has the keyboard after a press.
			const cover = await page.evaluate(() => /click|capture|lock/i.test((document.querySelector("[data-dos]") as HTMLElement).innerText || ""));
			check(browserName, key, "js-dos's capture cover is gone", !cover);

			// The reference frame: the visitor's pointer at a place on the page, clear of the menus; its blob is masked out afterwards.
			const p0 = { x: cx(0.88), y: cy(key === "text" ? 0.3 : 0.28) };
			await stroke(cx(0.5), cy(0.5), p0.x, p0.y, 20, 10);
			await sleep(1500);
			await page.evaluate(() => (window as any).__spike.setRef());
			const e0 = expected(p0.x, p0.y);
			await page.evaluate(([x0, y0, x1, y1]) => ((window as any).__spike.mask = { x0, y0, x1, y1 }), [e0.x - mode.mask[0], e0.y - mode.mask[1], e0.x + mode.mask[0], e0.y + mode.mask[1]]);

			// Level 3: slow straight strokes; the pointer in the guest's frame at stops along each, against the pixel under the visitor's.
			const errors: number[][] = [];
			const strokes = quick ? [27] : [10, 27, 45, 63];
			const traceNumbers: any[] = [];
			for (const angle of strokes) {
				const length = key === "text" ? 0.32 : 0.34;
				const dx = Math.cos((angle * Math.PI) / 180) * length * box.width;
				const dy = Math.sin((angle * Math.PI) / 180) * length * box.width;
				const sx = cx(0.12);
				const sy = cy(0.27) + (key === "text" ? 0 : 6);
				await moveTo(sx, sy);
				await sleep(400);
				const stops = 6;
				await page.evaluate(() => ((window as any).__calls.length = 0));
				for (let s = 1; s <= stops; s++) {
					// 0.1 CSS pixels a millisecond: a slow hand.
					await stroke(sx + (dx * (s - 1)) / stops, sy + (dy * (s - 1)) / stops, sx + (dx * s) / stops, sy + (dy * s) / stops, Math.max(20, Math.round(Math.hypot(dx, dy) / stops / 1.6)), 8);
					await sleep(450);
					const guest = await locate();
					if (!guest) {
						errors.push([NaN, NaN]);
						continue;
					}
					const want = expected(sx + (dx * s) / stops, sy + (dy * s) / stops);
					errors.push([guest.x - want.x, guest.y - want.y]);
				}
				const calls = await page.evaluate(() => (window as any).__calls.filter((c: any) => c.kind === "sendMouseRelativeMotion"));
				// Calls an animation frame: the most in any 16.7 ms.
				let perFrame = 0;
				for (let i = 0; i < calls.length; i++) {
					let n = 1;
					while (i + n < calls.length && calls[i + n].t - calls[i].t < 16.7) n++;
					perFrame = Math.max(perFrame, n);
				}
				traceNumbers.push({ angle, calls: calls.length, maxPerFrame: perFrame });
			}
			const finite = errors.filter((e) => !Number.isNaN(e[0]));
			const biasX = median(finite.map((e) => e[0]));
			const biasY = median(finite.map((e) => e[1]));
			const spread = finite.map((e) => [Math.abs(e[0] - biasX), Math.abs(e[1] - biasY)]);
			const worst = [Math.max(...spread.map((s) => s[0])), Math.max(...spread.map((s) => s[1]))];
			check(browserName, key, "every stop is found", finite.length === errors.length, `${finite.length} of ${errors.length}`);
			check(browserName, key, "stops land on the pixel under the pointer", worst[0] <= mode.tolerance[0] && worst[1] <= mode.tolerance[1], `hotspot offset ${biasX},${biasY}; worst ${worst.join(",")}`);

			// The page's trace, scored with the direction error (the model against the visitor's path).
			const downloadPromise = page.waitForEvent("download", { timeout: 10000 });
			// Clicked from the page's own script: a real click would scroll the page and move the mouse, and the stage's place would change.
			await page.evaluate(() => (document.querySelector("[data-trace]") as HTMLElement).click());
			const download = await downloadPromise;
			const tracePath = path.join(outDir, `trace-${browserName}-${key}.json`);
			await download.saveAs(tracePath);
			const trace = JSON.parse(fs.readFileSync(tracePath, "utf8"));
			// The first stroke is the mouse arriving from outside the page and going to the reference place; the strokes after it are the test's.
			const scored = scoreTrace(trace, { window: 8 }).slice(1);
			// Calls an animation frame, as the module makes them (the calls of one flush share a time); the page's pacing then sends them to `ci`
			// in windows of 30 ms, so more than one may reach it at once (traceNumbers above are those).
			// A move longer than the page's call size (80 counts) is split, so a flush may have more than 2 calls only for as many as its counts need.
			const perFlush = new Map<number, { calls: number; x: number; y: number }>();
			for (const o of trace.output) {
				const f = perFlush.get(o.t) || { calls: 0, x: 0, y: 0 };
				perFlush.set(o.t, { calls: f.calls + 1, x: f.x + Math.abs(o.dx), y: f.y + Math.abs(o.dy) });
			}
			let excess = 0;
			let maxPerFlush = 0;
			for (const f of perFlush.values()) {
				maxPerFlush = Math.max(maxPerFlush, f.calls);
				if (f.calls > Math.max(2, Math.ceil(Math.max(f.x, f.y) / 80))) excess++;
			}
			check(browserName, key, "at most 2 calls an animation frame in Follow, more only to split a long move", excess === 0, `most in one frame ${maxPerFlush}; frames over their share ${excess}; at ci, in 16.7 ms: ${traceNumbers.map((t) => `${t.angle}°:${t.maxPerFrame}`).join(" ")}`);
			const p95 = scored.length ? Math.max(...scored.map((s: any) => s.metric.p95)) : NaN;
			const axisOnly = scored.length ? Math.max(...scored.map((s: any) => s.metric.axisOnly)) : NaN;
			check(browserName, key, `direction error of the model at most ${key === "text" ? 15 : 10} degrees (p95, 8 pixels)`, scored.length > 0 && p95 <= (key === "text" ? 15 : 10), `${scored.length} strokes, p95 ${p95?.toFixed?.(1)}, one-axis share ${axisOnly?.toFixed?.(3)}; per stroke ${scored.map((s: any) => s.metric.p95.toFixed(1) + "/" + s.metric.windows).join(" ")}`);

			// Edges: out to the right edge, along it, then to the left edge, and in to a target in the middle. Quill's pointer cannot be seen at the
			// very edge (its arrow is clipped there), so the check is where it lands afterwards: the page drives the guest pointer past the edge
			// each time it arrives, so any drift is gone.
			const row = cy(0.45);
			await moveTo(cx(0.6), row);
			await stroke(cx(0.6), row, box.x + box.width - 0.4, row, 20, 8);
			await stroke(box.x + box.width - 0.4, row, box.x + box.width - 0.4, cy(0.6), 6, 8);
			await stroke(box.x + box.width - 0.4, cy(0.6), box.x + 0.4, cy(0.6), 40, 8);
			await stroke(box.x + 0.4, cy(0.6), cx(0.45), cy(0.5), 20, 8);
			await sleep(2500);
			const afterEdges = await locate();
			if (process.env.DEBUG) {
				console.log(JSON.stringify(await page.evaluate(() => ({ n: (window as any).__calls.length, last: (window as any).__calls.slice(-4), line: document.querySelector("[data-pointer-debug]")!.textContent }))));
				await page.locator("[data-stage]").screenshot({ path: path.join(outDir, "shots", `debug-edges-${browserName}-${key}.png`) });
			}
			const middle = expected(cx(0.45), cy(0.5));
			check(
				browserName,
				key,
				"after touching the right and left edges, the pointer lands on the target",
				!!afterEdges && Math.abs(afterEdges.x - middle.x - biasX) <= mode.tolerance[0] && Math.abs(afterEdges.y - middle.y - biasY) <= mode.tolerance[1],
				afterEdges ? `at ${afterEdges.x},${afterEdges.y}, want ${middle.x + biasX},${middle.y + biasY}` : "not found",
			);

			// Leaving and coming back in elsewhere: out to the right, in again at the top, down to a target.
			await stroke(cx(0.5), cy(0.5), box.x + box.width + 120, cy(0.5), 20, 6);
			await stroke(box.x + box.width + 120, cy(0.5), cx(0.4), box.y - 30, 20, 6);
			await stroke(cx(0.4), box.y - 30, cx(0.4), cy(0.4), 20, 8);
			await sleep(2500);
			const back = await locate();
			const target = expected(cx(0.4), cy(0.4));
			check(
				browserName,
				key,
				"after leaving and coming back in at the top, the pointer is under the visitor's",
				!!back && Math.abs(back.x - target.x - biasX) <= mode.tolerance[0] && Math.abs(back.y - target.y - biasY) <= mode.tolerance[1],
				back ? `at ${back.x},${back.y}, want ${target.x + biasX},${target.y + biasY}` : "not found",
			);

			// A press inside, a move out past the right side and a release outside: the guest's button comes up.
			await page.evaluate(() => ((window as any).__calls.length = 0));
			await moveTo(cx(0.3), cy(0.45));
			await sleep(200);
			await page.mouse.down();
			await stroke(cx(0.3), cy(0.45), cx(0.5), cy(0.6), 10, 8);
			await stroke(cx(0.5), cy(0.6), stage.x + stage.width + 150, cy(0.6), 20, 8);
			await page.mouse.up();
			await sleep(1500);
			const buttons = await page.evaluate(() => (window as any).__calls.filter((c: any) => c.kind === "sendMouseButton"));
			const downs = buttons.filter((b: any) => b.b).length;
			const ups = buttons.filter((b: any) => !b.b).length;
			check(browserName, key, "no button is left down after a release outside", downs >= 1 && downs === ups, `${downs} presses, ${ups} releases`);

			// Text mode: the page's pointer clicks a menu title, a menu item, a dialog's own drop-down and its button, each at the middle of its
			// cell (the 80 by 25 cells of the 640 by 400 frame, 8 by 16 pixels each). A guest pointer a cell off would open the wrong thing or
			// nothing: the Save As dialog of the text screen (DOS-34) once did not answer the pointer at all, and a one-row button is the
			// smallest target there is.
			if (key === "text") {
				// The box is read again for each click: a click on the stage can scroll the page, and a stale box would aim at the wrong cell. The
				// viewport is made tall enough that the whole picture is on it, so the last rows of the dialog can be reached.
				await page.setViewportSize({ width: 1100, height: 1500 });
				await sleep(800);
				const clickCell = async (col: number, row: number) => {
					const now = (await page.locator("[data-dos] canvas").first().boundingBox())!;
					const at = { x: now.x + ((col + 0.5) * now.width) / 80, y: now.y + ((row + 0.5) * now.height) / 25 };
					await stroke(at.x - 40, at.y - 24, at.x, at.y, 12, 8);
					await sleep(500);
					if (dispatched) {
						// Firefox's moves here are dispatched, so its press is too: a real one would be at the real mouse's last place.
						const press = (type: string, buttons: number) =>
							page.evaluate(([t, px, py, b]) => document.querySelector("[data-stage]")!.dispatchEvent(new PointerEvent(t as string, { clientX: px as number, clientY: py as number, button: 0, buttons: b as number, pointerType: "mouse", pointerId: 1, isPrimary: true, bubbles: true, cancelable: true })), [type, at.x, at.y, buttons]);
						await press("pointerdown", 1);
						await sleep(120);
						await press("pointerup", 0);
					} else {
						await page.mouse.down();
						await sleep(120);
						await page.mouse.up();
					}
					await sleep(1500);
				};
				const changed = (c0: number, r0: number, c1: number, r1: number) => page.evaluate(([a, b, c, d]) => (window as any).__spike.changed(a * 8, b * 16, c * 8, d * 16), [c0, r0, c1, r1]);
				await page.evaluate(() => ((window as any).__spike.mask = null));
				await page.evaluate(() => (window as any).__spike.setRef());
				await clickCell(3, 0);
				check(browserName, key, "a click on the File title opens its menu", (await changed(1, 1, 26, 13)) > 4000, `${await changed(1, 1, 26, 13)} pixels changed`);
				await clickCell(4, 6);
				const opened = await changed(1, 3, 76, 20);
				check(browserName, key, "a click on Save As... in the menu opens the file dialog", opened > 20000, `${opened} pixels changed`);
				await page.locator("[data-stage]").screenshot({ path: path.join(outDir, "shots", `text-save-as-${browserName}.png`) });
				await page.evaluate(() => (window as any).__spike.setRef());
				await clickCell(60, 17);
				const dropped = await changed(54, 12, 75, 17);
				check(browserName, key, "a click on the type box of the file dialog drops its list", dropped > 1500, `${dropped} pixels changed`);
				await page.locator("[data-stage]").screenshot({ path: path.join(outDir, "shots", `text-save-as-drop-${browserName}.png`) });
				await clickCell(60, 14);
				const closed = await changed(54, 12, 75, 16);
				check(browserName, key, "a click on a row of the list picks it and shuts the list", closed < 1500, `${closed} pixels changed`);
				await clickCell(67, 19);
				const gone = await changed(1, 3, 76, 20);
				check(browserName, key, "a click on Cancel in the file dialog closes it", gone > 20000, `${gone} pixels of the dialog are gone`);
			}

			// A diagonal selection, for the evidence.
			await moveTo(cx(0.1), cy(0.5));
			await sleep(300);
			await page.mouse.down();
			await stroke(cx(0.1), cy(0.5), cx(0.55), cy(0.78), 60, 10);
			await sleep(600);
			await page.locator("[data-stage]").screenshot({ path: path.join(outDir, "shots", `follow-selection-${browserName}-${key}.png`) });
			await page.mouse.up();
			numbers.push({ browser: browserName, mode: key, hotspotOffset: [biasX, biasY], worstStop: worst, strokes: traceNumbers, trace: { strokes: scored.length, p95, axisOnly }, stops: errors });
			await page.close();

			// Capture: the page's own lock (a stand-in here), movement dispatched as the browser would report it, the slider doubling it.
			const cap = await open(base, browser, browserName, key, "pointer=debug&mouse=capture&lockstub=1");
			const capBox = (await cap.locator("[data-dos] canvas").first().boundingBox())!;
			await cap.mouse.move(capBox.x + 100, capBox.y + 100);
			await cap.mouse.click(capBox.x + 100, capBox.y + 100);
			await sleep(300);
			const capLocate = () => cap.evaluate(([w, h, top, bottom]) => (window as any).__spike.settled(w, h, top, bottom), [...mode.window, ...mode.rows]);
			const moveBy = async (totalX: number, totalY: number, steps: number) => {
				for (let i = 0; i < steps; i++) {
					await cap.evaluate(
						([mx, my]) => {
							const stageEl = document.querySelector("[data-stage]")!;
							stageEl.dispatchEvent(new PointerEvent("pointermove", { movementX: mx, movementY: my, pointerType: "mouse", pointerId: 1, isPrimary: true, bubbles: true, cancelable: true }));
						},
						[totalX / steps, totalY / steps],
					);
					await sleep(20);
				}
				await sleep(300);
			};
			const setRef = (at: { x: number; y: number }) =>
				cap.evaluate(([x, y, mx, my]) => {
					const spike = (window as any).__spike;
					return spike.setRef().then(() => (spike.mask = { x0: x - mx, y0: y - my, x1: x + mx, y1: y + my }));
				}, [at.x, at.y, mode.mask[0], mode.mask[1]]);
			const kx = mode.frame[0] / capBox.width;
			const ky = mode.frame[1] / capBox.height;
			// The first move takes the pointer off Quill's centre; a reference there; then 100 by 60 (shown pixels) at speed 1 and 80 by 50 at 2.
			await cap.evaluate(() => (window as any).__spike.setRef());
			await moveBy(-120, -40, 12);
			const p1 = await capLocate();
			if (!p1) check(browserName, key, "Capture moves the guest", false, "the pointer was not found");
			else {
				await setRef(p1);
				await moveBy(100, 60, 12);
				const p2 = await capLocate();
				await cap.locator("[data-speed-input]").evaluate((el: HTMLInputElement) => {
					el.value = "2";
					el.dispatchEvent(new Event("input", { bubbles: true }));
				});
				if (p2) await setRef(p2);
				await moveBy(80, 50, 12);
				const p3 = p2 && (await capLocate());
				const ok = (got: { x: number; y: number } | null, from: { x: number; y: number }, wantX: number, wantY: number) =>
					!!got && Math.abs(got.x - from.x - wantX) <= Math.max(mode.tolerance[0] + 2, 0.15 * Math.abs(wantX)) && Math.abs(got.y - from.y - wantY) <= Math.max(mode.tolerance[1] + 2, 0.15 * Math.abs(wantY));
				check(browserName, key, "Capture moves the guest as far as the movement says", ok(p2, p1, 100 * kx, 60 * ky), p2 ? `moved ${p2.x - p1.x},${p2.y - p1.y}, want ${(100 * kx).toFixed(0)},${(60 * ky).toFixed(0)}` : "not found");
				check(browserName, key, "the Pointer speed slider doubles it in Capture", !!p2 && ok(p3, p2, 160 * kx, 100 * ky), p2 && p3 ? `moved ${p3.x - p2.x},${p3.y - p2.y}, want ${(160 * kx).toFixed(0)},${(100 * ky).toFixed(0)}` : "not found");
			}
			await cap.close();
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
		fs.writeFileSync(path.join(outDir, "try-pointer.json"), JSON.stringify({ date: new Date().toISOString(), checks, numbers }, null, 1));
		if (stop) stop();
	}
	const failed = checks.filter((c) => !c.pass);
	console.log(`\n${checks.length - failed.length} of ${checks.length} checks passed`);
	if (failed.length) process.exit(1);
}

await main();
