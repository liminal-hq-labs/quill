// Level 2 of the pointer's tests: the lab page in headless Chromium and Firefox, scored by the direction error.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

// Usage: env -u WAYLAND_DISPLAY DISPLAY=:99 node pointer-lab.ts [--browser chromium|firefox] [--section follow,capture,behaviour,shots] [--out DIR] [--quick]
// BASE=http://127.0.0.1:PORT uses a server that is already running (scripts/pointer/serve.mjs); without it the script starts one in
// this process on a free port. Chromium is /usr/bin/chromium (CHROMIUM overrides); Firefox is Playwright's build.
//
// Follow, Chromium: trusted page.mouse.move with fractional positions. Follow, Firefox: dispatched PointerEvents with fractional clientX
// (its driver rounds trusted ones). Capture, both: dispatched movementX (neither browser locks headless), as the browser reports it
// under three models: exact, Chromium's and Firefox on Wayland's.
//
// Pass lines (docs/plans/pointer.md, "Measuring it"): p95 error at most 10 degrees at 8 guest pixels, at most 1 percent of windows on one
// axis only, the end point within 1 guest pixel, at most 1 call a frame in Follow and 2 in Capture. Capture under the Firefox-on-Wayland
// model at 27 degrees, 0.8 px/ms, 1000 Hz, scale 1.5 must FAIL them: the run exits 1 if it does not. Exits 1 on any other miss.

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { chromium, firefox } from "playwright-core";
import type { Browser, Page } from "playwright-core";
// @ts-ignore: plain JavaScript modules shared with the Node tests
import { INPUT_MODELS, centredOrigin, handPath } from "../../../scripts/pointer/paths.mjs";
// @ts-ignore
import { startServer } from "../../../scripts/pointer/serve.mjs";

const here = path.dirname(fileURLToPath(import.meta.url));
const args = process.argv.slice(2);
const opt = (name: string, fallback: string) => (args.includes("--" + name) ? args[args.indexOf("--" + name) + 1] : fallback);
const quick = args.includes("--quick");
const outDir = path.resolve(opt("out", path.join(here, "..", "out", "pointer-lab")));
const only = opt("browser", "");
const sections = opt("section", "follow,capture,behaviour,shots").split(",");
fs.mkdirSync(path.join(outDir, "shots"), { recursive: true });

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

interface Row {
	browser: string;
	section: string;
	cell: string;
	windows: number;
	median: number;
	p95: number;
	axisOnly: number;
	endpoint: number[];
	calls: number;
	expectFail: boolean;
	pass: boolean;
}
const rows: Row[] = [];
const checks: { browser: string; name: string; pass: boolean; detail: string }[] = [];

function check(browser: string, name: string, pass: boolean, detail = "") {
	checks.push({ browser, name, pass, detail });
	console.log(`${pass ? "ok  " : "FAIL"} ${browser}: ${name}${detail ? " (" + detail + ")" : ""}`);
}

function meets(metric: any, calls: number, maxCalls: number, tolerance: number) {
	return metric.windows > 0 && metric.p95 <= 10 && metric.axisOnly <= 0.01 && Math.abs(metric.endpoint[0]) <= tolerance && Math.abs(metric.endpoint[1]) <= tolerance && calls <= maxCalls;
}

function record(browser: string, section: string, cell: string, stroke: any, maxCalls: number, expectFail = false, tolerance = 1) {
	if (process.env.DEBUG_FRAMES && stroke && !(meets(stroke.metric, 0, 9, tolerance))) fs.writeFileSync(path.join(outDir, `debug-${browser}-${rows.length}.json`), JSON.stringify(stroke));
	const metric = stroke ? stroke.metric : { windows: 0, median: NaN, p95: NaN, axisOnly: NaN, endpoint: [NaN, NaN] };
	const calls = stroke ? Math.max(...stroke.frames.map((frame: any) => frame.calls || 0)) : 0;
	const met = !!stroke && meets(metric, calls, maxCalls, tolerance);
	const pass = expectFail ? !met : met;
	rows.push({ browser, section, cell, windows: metric.windows, median: metric.median, p95: metric.p95, axisOnly: metric.axisOnly, endpoint: metric.endpoint, calls, expectFail, pass });
	console.log(
		`${pass ? "ok  " : "FAIL"} ${browser} ${section}: ${cell}: windows ${metric.windows}, median ${metric.median?.toFixed(1)}, p95 ${metric.p95?.toFixed(1)}, one axis ${(metric.axisOnly * 100).toFixed(1)}%, end (${metric.endpoint[0]?.toFixed(1)}, ${metric.endpoint[1]?.toFixed(1)}), calls ${calls}${expectFail ? " (must fail the lines)" : ""}`,
	);
}

async function labPage(browser: Browser, base: string, dpr: number, query: string, viewport = { width: 900, height: 820 }) {
	const context = await browser.newContext({ viewport, deviceScaleFactor: dpr });
	const page = await context.newPage();
	await page.addInitScript(() => {
		try {
			localStorage.clear();
		} catch (error) {
			// Storage is blocked.
		}
	});
	await page.goto(`${base}/pointer-lab/${query}`);
	await page.waitForFunction("window.__lab && window.__lab.handle");
	return { context, page };
}

const shownOf = (page: Page) => page.evaluate(() => (window as any).__lab.handle.state().shown);

// Waits for a stroke to end (the lab closes it after 250 ms without input) and returns its frames and score.
async function finishStroke(page: Page) {
	await sleep(700);
	return page.evaluate(() => (window as any).__lab.lastStroke());
}

// The path's guest-pixel points as page positions.
function toPage(shown: any, points: { t: number; x: number; y: number }[]) {
	const k = shown.w / 640;
	return points.map((point) => ({ t: point.t, x: shown.left + point.x * k, y: shown.top + point.y * k }));
}

// Follow: the hand moves through the page positions; Chromium with trusted moves, Firefox with dispatched events.
async function stroke(page: Page, name: string, positions: { t: number; x: number; y: number }[], pointerType = "mouse") {
	if (name === "chromium") {
		const begun = Date.now();
		for (const point of positions) {
			const wait = point.t - (Date.now() - begun);
			if (wait > 1) await sleep(wait);
			await page.mouse.move(point.x, point.y);
		}
	} else {
		await page.evaluate(
			async ({ positions, pointerType }) => {
				const stage = document.querySelector("[data-stage]") as HTMLElement;
				const begun = performance.now();
				for (const point of positions) {
					const wait = point.t - (performance.now() - begun);
					if (wait > 1) await new Promise((resolve) => setTimeout(resolve, wait));
					stage.dispatchEvent(new PointerEvent("pointermove", { clientX: point.x, clientY: point.y, bubbles: true, cancelable: true, pointerType, pointerId: 1, isPrimary: true }));
				}
			},
			{ positions, pointerType },
		);
	}
}

async function followMatrix(browser: Browser, name: string, base: string) {
	const angles = quick ? [27, 83] : [0, 7, 27, 45, 63, 83, 90];
	const speeds = quick ? [0.8] : [0.1, 0.8];
	for (const dpr of quick ? [1.5] : [1, 1.5]) {
		const { context, page } = await labPage(browser, base, dpr, "?mouse=follow");
		const shown = await shownOf(page);
		for (const angle of angles)
			for (const speed of speeds) {
				const origin = centredOrigin(angle, speed);
				const points = toPage(shown, handPath(angle, speed, 125, origin));
				// A first visit makes the guest pointer known (the page parks), then the stroke is measured.
				await page.mouse.move(points[0].x, points[0].y);
				await stroke(page, name, [{ t: 0, x: points[0].x, y: points[0].y }]);
				await sleep(500);
				await page.evaluate(() => (window as any).__lab.reset());
				await stroke(page, name, points);
				record(name, "follow", `${angle} degrees, ${speed} px/ms, 125 Hz, scale ${dpr}`, await finishStroke(page), 1);
			}
		// A js-dos-like guest: one count is 1.37 by 1.1 pixels.
		await page.click('input[name="units"][value="jsdos"]');
		const points = toPage(shown, handPath(27, 0.4, 125, centredOrigin(27, 0.4)));
		await stroke(page, name, [{ t: 0, x: points[0].x, y: points[0].y }]);
		await sleep(500);
		await page.evaluate(() => (window as any).__lab.reset());
		await stroke(page, name, points);
		const jsdos = await finishStroke(page);
		// One count is 1.37 by 1.1 pixels here, so the end point is within a pixel and half a count.
		record(name, "follow, js-dos units", `27 degrees, 0.4 px/ms, 125 Hz, scale ${dpr}`, jsdos, 1, false, 1.7);
		await context.close();
	}
}

// Capture: the reports of a hand path as dispatched movement, a frame's worth at a time; the truth is the exact path.
async function captureStroke(page: Page, kind: number, speed: number, hz: number, model: string, dpr: number, shot?: string) {
	const shown = await shownOf(page);
	const k = shown.w / 640;
	const path = handPath(kind, speed, hz, [0, 0]);
	const reports = (INPUT_MODELS as any)[model](path, dpr);
	// Bring the guest to the start of the stroke first.
	const origin = centredOrigin(kind, speed);
	await page.evaluate(
		async ({ ox, oy, k }) => {
			const stage = document.querySelector("[data-stage]") as HTMLElement;
			const lab = (window as any).__lab;
			const [gx, gy] = lab.guestPosition();
			const event = new PointerEvent("pointermove", { bubbles: true, pointerType: "mouse", pointerId: 1 });
			Object.defineProperty(event, "movementX", { value: (ox - gx) * k });
			Object.defineProperty(event, "movementY", { value: (oy - gy) * k });
			stage.dispatchEvent(event);
			await new Promise((resolve) => setTimeout(resolve, 300));
			lab.reset();
		},
		{ ox: origin[0], oy: origin[1], k },
	);
	await sleep(400);
	await page.evaluate(() => (window as any).__lab.reset());
	await page.evaluate(
		async ({ reports, path, k }) => {
			const stage = document.querySelector("[data-stage]") as HTMLElement;
			const lab = (window as any).__lab;
			// The guest's position and the fraction of a count the page still owes it: the stroke's true start.
			const [guestX, guestY] = lab.guestPosition();
			const owed = lab.handle.state().remainder;
			const startX = guestX + owed[0] / lab.units.x;
			const startY = guestY + owed[1] / lab.units.y;
			let at = 0;
			let trueIndex = 0;
			const begun = performance.now();
			const frameEnd = path[path.length - 1].t;
			while (true) {
				const now = performance.now() - begun;
				while (at < reports.length && reports[at].t <= now) {
					const r = reports[at];
					// The constructor cuts movementX and movementY to whole numbers, so the reported values are set on the event itself.
					const event = new PointerEvent("pointermove", { bubbles: true, pointerType: "mouse", pointerId: 1 });
					Object.defineProperty(event, "movementX", { value: r.mx });
					Object.defineProperty(event, "movementY", { value: r.my });
					stage.dispatchEvent(event);
					at++;
				}
				while (trueIndex + 1 < path.length && path[trueIndex + 1].t <= now) trueIndex++;
				lab.setInput(startX + (path[trueIndex].x - path[0].x) / k, startY + (path[trueIndex].y - path[0].y) / k);
				if (now > frameEnd + 20 && at >= reports.length) break;
				await new Promise((resolve) => requestAnimationFrame(resolve));
			}
		},
		{ reports, path, k },
	);
	const result = await finishStroke(page);
	if (shot) await page.screenshot({ path: shot, fullPage: true });
	return result;
}

async function captureMatrix(browser: Browser, name: string, base: string) {
	const cells: [number, number, number, string, number, boolean][] = quick
		? [[27, 0.8, 1000, "firefox", 1.5, true], [27, 0.8, 1000, "exact", 1.5, false]]
		: [
				[27, 0.8, 1000, "firefox", 1.5, true],
				[27, 0.4, 1000, "firefox", 1.5, true],
				[27, 0.8, 1000, "exact", 1.5, false],
				[27, 0.8, 1000, "chromium", 1.5, false],
				[7, 0.1, 1000, "exact", 1, false],
				[45, 0.4, 125, "chromium", 1.25, false],
				[63, 0.1, 1000, "chromium", 2, false],
				[83, 0.8, 125, "exact", 1, false],
			];
	for (const dpr of [1, 1.25, 1.5, 2]) {
		const mine = cells.filter((cell) => cell[4] === dpr);
		if (!mine.length) continue;
		const { context, page } = await labPage(browser, base, dpr, "?mouse=capture&lockless=1");
		for (const [angle, speed, hz, model, scale, expectFail] of mine) {
			const shot = model === "firefox" && angle === 27 && speed === 0.8 ? path.join(outDir, "shots", `${name}-capture-firefox-model-27.png`) : undefined;
			const result = await captureStroke(page, angle, speed, hz, model, scale, shot);
			record(name, "capture", `${model} input, ${angle} degrees, ${speed} px/ms, ${hz} Hz, scale ${scale}`, result, 2, expectFail);
		}
		await context.close();
	}
}

async function behaviour(browser: Browser, name: string, base: string) {
	const { context, page } = await labPage(browser, base, 1, "?mouse=follow");
	const shown = await shownOf(page);
	const k = shown.w / 640;
	const inside = (gx: number, gy: number) => [shown.left + gx * k + 0.5, shown.top + gy * k + 0.5];
	const move = async (x: number, y: number, buttons = 0) => {
		if (name === "chromium") await page.mouse.move(x, y);
		else
			await page.evaluate(
				({ x, y, buttons }) => document.querySelector("[data-stage]")!.dispatchEvent(new PointerEvent("pointermove", { clientX: x, clientY: y, buttons, bubbles: true, pointerType: "mouse", pointerId: 1 })),
				{ x, y, buttons },
			);
	};
	const down = async (x: number, y: number) => {
		if (name === "chromium") {
			await page.mouse.move(x, y);
			await page.mouse.down();
		} else
			await page.evaluate(
				({ x, y }) => document.querySelector("[data-stage]")!.dispatchEvent(new PointerEvent("pointerdown", { clientX: x, clientY: y, buttons: 1, button: 0, bubbles: true, pointerType: "mouse", pointerId: 1 })),
				{ x, y },
			);
	};
	const up = async (x: number, y: number) => {
		if (name === "chromium") {
			await page.mouse.move(x, y);
			await page.mouse.up();
		} else
			await page.evaluate(
				({ x, y }) => document.querySelector("[data-stage]")!.dispatchEvent(new PointerEvent("pointerup", { clientX: x, clientY: y, buttons: 0, button: 0, bubbles: true, pointerType: "mouse", pointerId: 1 })),
				{ x, y },
			);
	};
	const guest = () => page.evaluate(() => (window as any).__lab.guestPosition());
	const buttons = () => page.evaluate(() => (window as any).__lab.guest.buttons);
	const cursor = () => page.evaluate(() => getComputedStyle(document.querySelector("[data-stage]")!).cursor);

	// The visitor's pointer is hidden over the picture and shown over the letterbox.
	await move(...(inside(300, 200) as [number, number]));
	await sleep(150);
	check(name, "the system pointer is hidden over the picture", (await cursor()) === "none", await cursor());
	await move(shown.left - 20, shown.top + 100);
	await sleep(150);
	check(name, "the system pointer is shown over the letterbox", (await cursor()) !== "none", await cursor());

	// The guest pointer is under the visitor's, and again after leaving at one edge and coming back at another.
	await move(...(inside(300, 200) as [number, number]));
	await sleep(250);
	let at = await guest();
	check(name, "the guest pointer is under the visitor's", Math.abs(at[0] - 300) <= 1 && Math.abs(at[1] - 200) <= 1, `at ${at.map((v: number) => v.toFixed(1))}`);
	await move(shown.left + shown.w + 40, shown.top + 100);
	await sleep(150);
	await move(...(inside(80, 470) as [number, number]));
	await sleep(250);
	at = await guest();
	check(name, "after leaving at one edge and entering at another it is under the visitor's again", Math.abs(at[0] - 80) <= 1 && Math.abs(at[1] - 470) <= 1, `at ${at.map((v: number) => v.toFixed(1))}`);

	// A drag released outside the picture leaves no button down, and keeps the guest pointer on the edge.
	const start = inside(200, 300) as [number, number];
	await down(...start);
	await sleep(100);
	check(name, "a press goes to the guest", (await buttons())[0] === true);
	await move(shown.left + 200 * k, shown.top + shown.h + 60, 1);
	await sleep(250);
	at = await guest();
	check(name, "dragged below the picture the guest pointer sits on the bottom edge", at[1] >= 478, `y ${at[1].toFixed(1)}`);
	await up(shown.left + 200 * k, shown.top + shown.h + 60);
	await sleep(150);
	check(name, "no button is down after a release outside", (await buttons()).every((b: boolean) => !b), JSON.stringify(await buttons()));

	// The wheel: nothing before the screen is selected, then a notch per 50 pixels.
	const wheel = (deltaY: number, deltaMode = 0) =>
		page.evaluate(
			({ deltaY, deltaMode }) => {
				const event = new WheelEvent("wheel", { deltaY, deltaMode, bubbles: true, cancelable: true });
				document.querySelector("[data-stage]")!.dispatchEvent(event);
				return { prevented: event.defaultPrevented, wheel: (window as any).__lab.guest.wheel };
			},
			{ deltaY, deltaMode },
		);
	await page.evaluate(() => (window as any).__lab.reset());
	await page.evaluate(() => document.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true, clientX: 2, clientY: 2 })));
	let result = await wheel(120);
	check(name, "the wheel does nothing before the screen is selected", !result.prevented && result.wheel === 0);
	await down(...(inside(300, 200) as [number, number]));
	await up(...(inside(300, 200) as [number, number]));
	await sleep(100);
	result = await wheel(120);
	check(name, "after selecting, 120 pixels is two notches and the event is used", result.prevented && result.wheel === 2, JSON.stringify(result));
	await sleep(400);
	await page.evaluate(() => (window as any).__lab.reset());
	result = await wheel(3, 1);
	result = await wheel(3, 1);
	check(name, "lines count 16 pixels: six lines is one notch", result.wheel === 1, JSON.stringify(result));

	// The raw probe: the first press in a browser that refuses raw capture stays in Follow.
	await context.close();
	const fresh = await labPage(browser, base, 1, "");
	const freshShown = await shownOf(fresh.page);
	const freshAt = [freshShown.left + freshShown.w / 2, freshShown.top + freshShown.h / 2];
	if (name === "chromium") {
		await fresh.page.mouse.move(freshAt[0], freshAt[1]);
		await fresh.page.mouse.down();
		await fresh.page.mouse.up();
	} else
		await fresh.page.evaluate(
			({ x, y }) => {
				const stage = document.querySelector("[data-stage]")!;
				stage.dispatchEvent(new PointerEvent("pointerdown", { clientX: x, clientY: y, buttons: 1, button: 0, bubbles: true, pointerType: "mouse", pointerId: 1 }));
				stage.dispatchEvent(new PointerEvent("pointerup", { clientX: x, clientY: y, buttons: 0, button: 0, bubbles: true, pointerType: "mouse", pointerId: 1 }));
			},
			{ x: freshAt[0], y: freshAt[1] },
		);
	await sleep(500);
	const state = await fresh.page.evaluate(() => {
		const s = (window as any).__lab.handle.state();
		return { mode: s.mode, raw: s.raw, secure: s.secure, coalesced: s.coalesced };
	});
	check(name, "the raw probe is refused and the page stays in Follow", state.mode === "follow" && state.raw === "no", JSON.stringify(state));
	const guestAfterClick = await fresh.page.evaluate(() => (window as any).__lab.guest.calls.length);
	check(name, "the press that probed still reached the guest as a click", guestAfterClick > 0);
	await fresh.context.close();
}

async function shots(browser: Browser, name: string, base: string) {
	const { context, page } = await labPage(browser, base, 1.5, "?mouse=follow");
	const shown = await shownOf(page);
	// A slow stroke at 27 degrees, stretched to be easy to see.
	const long = handPath(27, 0.2, 125, centredOrigin(27, 0.2)).map((p: any) => ({ t: p.t, x: 320 + (p.x - 320) * 2.5, y: 240 + (p.y - 240) * 2.5 }));
	const points = toPage(shown, long);
	await stroke(page, name, [{ t: 0, x: points[0].x, y: points[0].y }]);
	await sleep(500);
	await page.evaluate(() => (window as any).__lab.reset());
	await stroke(page, name, points);
	await finishStroke(page);
	await page.screenshot({ path: path.join(outDir, "shots", `${name}-follow-27.png`), fullPage: true });
	await context.close();
}

const { server, base } = process.env.BASE ? { server: null, base: process.env.BASE.replace(/\/$/, "") } : await startServer({});
let failed = false;
try {
	for (const name of ["chromium", "firefox"]) {
		if (only && only !== name) continue;
		const browser = name === "chromium" ? await chromium.launch({ headless: true, executablePath: process.env.CHROMIUM || "/usr/bin/chromium" }) : await firefox.launch({ headless: true });
		console.log(`\n${name} ${browser.version()}`);
		try {
			if (sections.includes("follow")) await followMatrix(browser, name, base);
			if (sections.includes("capture")) await captureMatrix(browser, name, base);
			if (sections.includes("behaviour")) await behaviour(browser, name, base);
			if (sections.includes("shots")) await shots(browser, name, base);
		} finally {
			await browser.close();
		}
	}
} finally {
	if (server) server.close();
}

fs.writeFileSync(path.join(outDir, sections.length === 4 && !quick ? "pointer-lab.json" : `pointer-lab-${sections.join("-")}${quick ? "-quick" : ""}.json`), JSON.stringify({ rows, checks }, null, 1));
const misses = rows.filter((row) => !row.pass).length + checks.filter((c) => !c.pass).length;
failed = misses > 0;
console.log(`\n${rows.length} strokes and ${checks.length} checks, ${misses} misses`);
if (failed) process.exit(1);
