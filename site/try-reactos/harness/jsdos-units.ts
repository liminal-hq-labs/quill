// WP-PTR-2: what DOSBox in js-dos does with the page's relative pointer moves. Drives a locally built Try Quill page in a headless browser.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

// Usage: env -u WAYLAND_DISPLAY DISPLAY=:99 node jsdos-units.ts [--browser chromium|firefox] [--mode gfx,g16,text] [--runs 5] [--out DIR]
// BASE=http://127.0.0.1:PORT/try/ is a server already running over site/ (scripts/pointer/serve.mjs) with a local bundle built by
// `bash scripts/build-try.sh --build`; without it the script starts its own server on a free port and stops it at the end.
//
// The page is not changed: a script injected before it loads wraps window.Dos so the player is started with mouseCapture false and the
// script can reach the `ci` (ci.sendMouseRelativeMotion, ci.screenshot). Quill's pointer is found in ci.screenshot() frames by the
// difference against a frame with the pointer parked in the top-left corner (a large negative move clamps it there).
//
// Answers, per display mode: (1) do relative moves act with no lock; (2) pointer units a count, fitted over 1, 2, 4 ... 64 counts and over
// long sweeps; (3) is it linear (one call of n against n calls of 1, fast against slow); (4) does DOSBox keep a fraction (64 calls of
// 0.25 against 16 of 1); (5) does mouseCapture false leave js-dos's mouse handlers running until the stage stops them; (6) where the
// pointer is before the first move. Writes units.json (and a frame or two) to --out.

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { chromium, firefox } from "playwright-core";
import type { Page } from "playwright-core";
// @ts-ignore: plain JavaScript module shared with the Node tests
import { startServer } from "../../../scripts/pointer/serve.mjs";

const here = path.dirname(fileURLToPath(import.meta.url));
const args = process.argv.slice(2);
const opt = (name: string, fallback: string) => (args.includes("--" + name) ? args[args.indexOf("--" + name) + 1] : fallback);
const browserName = opt("browser", "chromium");
const modes = opt("mode", "gfx,g16,text").split(",");
const runs = Number(opt("runs", "5"));
// --bundle FILE answers the page's request for its disk bundle with another one (a local build with other mouse switches).
const bundleFile = opt("bundle", "");
const label = opt("label", "");
// --only clamp runs just the clamp-gap measurement (a quick check of how long a pause the page must leave after a call that clamps).
const only = opt("only", "");
const outDir = path.resolve(opt("out", path.join(here, "..", "out", "jsdos-units")));
fs.mkdirSync(outDir, { recursive: true });

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

// In the page, before anything else: start the player with mouseCapture false and hand over the ci; find the pointer in frames.
// In the page, before anything else: start the player with mouseCapture false and hand over the ci (jsdos-inpage.js has the rest).
const INIT = () => {
	let real: any;
	Object.defineProperty(window, "Dos", {
		configurable: true,
		get() {
			return (
				real &&
				function (host: any, opts: any) {
					opts.mouseCapture = false;
					const previous = opts.onEvent;
					opts.onEvent = (event: string, arg: any) => {
						if (event === "ci-ready") (window as any).__ci = arg;
						if (previous) previous(event, arg);
					};
					const player = real(host, opts);
					(window as any).__dos = player;
					return player;
				}
			);
		},
		set(value) {
			real = value;
		},
	});
};

interface Mode {
	key: string;
	name: string;
	// The window the pointer's blob is looked for in, the rows it is looked for between, and the size of one guest cell in frame pixels
	// (text: 8 by 16 in the 640 by 400 frame; graphics: 1).
	window: [number, number];
	rows: [number, number];
	cell: [number, number];
	// The pointer's own coordinate range and what one of its units is, in the driver's range (graphics 640 by 480, text a virtual 640 by 200).
	range: [number, number];
	// The row (in frame pixels) the pointer is brought below before a measurement, so a long move down stays on the screen.
	baseY: number;
}
const MODES: Record<string, Mode> = {
	gfx: { key: "gfx", name: "graphics, 256 colours", window: [22, 26], rows: [96, 440], cell: [1, 1], range: [640, 480], baseY: 140 },
	g16: { key: "g16", name: "graphics, 16 colours", window: [22, 26], rows: [96, 440], cell: [1, 1], range: [640, 480], baseY: 140 },
	text: { key: "text", name: "text mode", window: [20, 18], rows: [32, 380], cell: [8, 16], range: [640, 200], baseY: 40 },
};

function stats(values: number[]) {
	const n = values.length;
	const mean = values.reduce((a, b) => a + b, 0) / n;
	const sd = Math.sqrt(values.reduce((a, b) => a + (b - mean) ** 2, 0) / Math.max(1, n - 1));
	return { n, mean, sd, min: Math.min(...values), max: Math.max(...values) };
}

// Least squares through the points, with and without an intercept.
function fit(points: [number, number][]) {
	const n = points.length;
	const sx = points.reduce((a, p) => a + p[0], 0);
	const sy = points.reduce((a, p) => a + p[1], 0);
	const sxx = points.reduce((a, p) => a + p[0] * p[0], 0);
	const sxy = points.reduce((a, p) => a + p[0] * p[1], 0);
	const slope = (n * sxy - sx * sy) / (n * sxx - sx * sx);
	const intercept = (sy - slope * sx) / n;
	const originSlope = sxy / sxx;
	const residual = Math.sqrt(points.reduce((a, p) => a + (p[1] - (slope * p[0] + intercept)) ** 2, 0) / n);
	return { slope, intercept, originSlope, residual };
}

async function openQuill(base: string, key: string, launched: any) {
	const page: Page = await launched.newPage({ viewport: { width: 1000, height: 900 } });
	if (bundleFile) await page.route(/\/quill\.[0-9a-f]{12}\.jsdos/, (route) => route.fulfill({ body: fs.readFileSync(bundleFile), contentType: "application/zip" }));
	// The page's pointer module (WP-PTR-3) would take the mouse events; the spike measures js-dos and the driver on their own.
	await page.route(/\/js\/pointer\.js/, (route) => route.fulfill({ body: "", contentType: "text/javascript" }));
	await page.addInitScript(INIT);
	await page.addInitScript({ path: path.join(here, "jsdos-inpage.js") });
	await page.goto(base);
	await page.check(`input[name=mode][value=${key}]`);
	await page.click("[data-start]");
	await page.waitForFunction(() => (window as any).__ci, null, { timeout: 90000 });
	return page;
}

async function main() {
	let stop: (() => void) | null = null;
	let base = process.env.BASE || "";
	if (!base) {
		const { server, base: origin } = await startServer({ port: 0 });
		base = origin + "/try/";
		stop = () => server.close();
	}
	const launched = await (browserName === "firefox" ? firefox.launch({ headless: true }) : chromium.launch({ executablePath: process.env.CHROMIUM || "/usr/bin/chromium", headless: true, args: ["--no-sandbox"] }));
	const report: any = { browser: browserName, version: launched.version(), date: new Date().toISOString(), runs, modes: {} };
	try {
		for (const key of modes) {
			const mode = MODES[key];
			console.log(`\n== ${mode.name}`);
			const page = await openQuill(base, key, launched);
			const out: any = { name: mode.name };
			report.modes[key] = out;
			// Quill is up when the guest picture stops being the boot messages: wait for the menu key's mode to start.
			await sleep(14000);
			const send = (dx: number, dy: number) => page.evaluate(([a, b]) => (window as any).__spike.send(a, b), [dx, dy]);
			const sendMany = (n: number, dx: number, dy: number, gap = 0) =>
				page.evaluate(
					async ([count, a, b, ms]) => {
						for (let i = 0; i < count; i++) {
							(window as any).__spike.send(a, b);
							if (ms) await new Promise((r) => setTimeout(r, ms));
						}
					},
					[n, dx, dy, gap],
				);
			const locate = () => page.evaluate(([w, h, top, bottom]) => (window as any).__spike.settled(w, h, top, bottom), [...mode.window, ...mode.rows]);
			// Text mode: the pointer moves a 16-pixel cell down, so a long move down leaves the rows the finder looks at; only across is measured.
			const axes: ("x" | "y")[] = mode.key === "text" ? ["x"] : ["x", "y"];
			const frames = path.join(outDir, key);
			fs.mkdirSync(frames, { recursive: true });
			const shot = async (name: string) => {
				await page.locator("[data-stage]").screenshot({ path: path.join(frames, `${browserName}-${name}.png`) });
			};
			const park = async () => {
				// Paced: DOSBox's PS/2 buffer is small and drops a burst (measured below), so a park of many calls is spread over a second.
				await sendMany(100, -50, -50, 10);
				await sleep(300);
			};
			

			// (6) where the pointer is before any move of ours: a frame, one tiny move, a frame; the difference is where it appears.
			await page.evaluate(() => (window as any).__spike.setRef());
			await shot("before-any-move");
			await send(1, 1);
			await sleep(150);
			await park();
			await page.evaluate(() => (window as any).__spike.setRef());
			// A frame of the parked guest, then the first move out of the corner, so the pointer's size and hotspot are known.
			await sendMany(4, 25, 25);
			await sleep(150);
			const first = await locate();
			out.firstMove = first;
			await shot("parked-then-moved");

			// Where does a pointer that has never moved sit? A second page, the same start, no park: the first move's position minus the move.
			// (Measured below on a fresh page.)

			// A place over the page, clear of the menus and toolbars, from the corner.
			const toBase = async () => {
				await park();
				for (let i = 0; i < 40; i++) {
					const at = i === 0 ? null : await locate();
					const needX = !at || at.x < 150;
					const needY = !at || at.y < mode.baseY;
					if (at && !needX && !needY) return;
					await sendMany(1, needX ? 50 : 0, needY ? 50 : 0, 10);
					if (i === 0) await sendMany(1, 0, 0);
				}
				throw new Error("could not bring the pointer over the page");
			};
			// (1)/(2) units per count on each axis: from a base position, +n then -n, for n = 1 ... 64, runs times.
			const sizes = [1, 2, 4, 8, 16, 32, 64];
			const unitPoints: any = { x: [], y: [] };
			const perRun: any = { x: {}, y: {} };
			await toBase();
			for (let run = 0; run < runs; run++) {
				for (const axis of axes) {
					for (const n of sizes) {
						const before = await locate();
						if (!before) throw new Error("the pointer was not found");
						await send(axis === "x" ? n : 0, axis === "y" ? n : 0);
						await sleep(150);
						const after = await locate();
						if (!after) throw new Error("the pointer was not found after a move");
						const moved = (after[axis] - before[axis]) / mode.cell[axis === "x" ? 0 : 1];
						if (process.env.DEBUG) console.log(axis, n, JSON.stringify(before), JSON.stringify(after));
						unitPoints[axis].push([n, moved]);
						(perRun[axis][n] ||= []).push(moved);
						await send(axis === "x" ? -n : 0, axis === "y" ? -n : 0);
						await sleep(150);
					}
				}
			}
			out.unitsPerCount = {};
			for (const axis of axes) {
				const f = fit(unitPoints[axis]);
				out.unitsPerCount[axis] = {
					unit: mode.cell[axis === "x" ? 0 : 1] === 1 ? "frame pixels per count" : "text cells per count",
					slope: f.slope,
					intercept: f.intercept,
					throughOrigin: f.originSlope,
					residual: f.residual,
					byCount: Object.fromEntries(sizes.map((n) => [n, stats(perRun[axis][n])])),
				};
			}
			console.log("units per count", JSON.stringify(out.unitsPerCount.x.slope), JSON.stringify(out.unitsPerCount.y?.slope), "residual", out.unitsPerCount.x.residual, out.unitsPerCount.y?.residual);

			// A long sweep: from the corner and a base on the other axis, N counts in calls of 20 along one axis; the position against N.
			const sweepPoints: any = { x: [], y: [] };
			for (const axis of axes) {
				for (let run = 0; run < runs; run++) {
					await toBase();
					const origin = await locate();
					if (!origin) throw new Error("the pointer was not found at the sweep's start");
					let sent = 0;
					for (const total of axis === "x" ? [60, 120, 200] : mode.key === "text" ? [10, 20, 40] : [20, 40, 80]) {
						await sendMany((total - sent) / 20, axis === "x" ? 20 : 0, axis === "y" ? 20 : 0, 10);
						sent = total;
						const end = await locate();
						if (process.env.DEBUG) console.log("sweep", axis, run, total, JSON.stringify(origin && [origin.x, origin.y]), JSON.stringify(end && [end.x, end.y]));
						if (!end && mode.key === "text") break;
						if (!end) {
							await shot(`sweep-lost-${axis}-${total}`);
							throw new Error(`the pointer was not found in the sweep (${axis}, ${total} counts)`);
						}
						sweepPoints[axis].push([sent, (end[axis] - origin[axis]) / mode.cell[axis === "x" ? 0 : 1]]);
					}
				}
			}
			out.sweep = {};
			for (const axis of axes) {
				const f = fit(sweepPoints[axis]);
				out.sweep[axis] = { slope: f.slope, intercept: f.intercept, residual: f.residual, points: sweepPoints[axis] };
			}
			console.log("sweep slope", out.sweep.x.slope, out.sweep.y?.slope, "residual", out.sweep.x.residual, out.sweep.y?.residual);

			// Calibration: from an edge, a long paced move (calls of 80 counts, 40 ms apart), so the distance is hundreds of pixels and the units
			// are known to a fraction of a percent. With the patched driver this is the coefficient; with the stock one it is three times that.
			const calibration: any = { x: [], y: [] };
			for (const axis of axes) {
				const total = axis === "x" ? 1200 : 400;
				for (let run = 0; run < Math.min(runs, 3); run++) {
					await toBase();
					// x from the left edge; y from just under the rows the finder looks at (an edge is not in them).
					if (axis === "x") await sendMany(40, -80, 0, 40);
					else await sendMany(1, 0, -40, 0);
					await sleep(300);
					const from = await locate();
					await sendMany(total / 80, axis === "x" ? 80 : 0, axis === "y" ? 80 : 0, 40);
					const to = await locate();
					if (from && to) calibration[axis].push([total, (to[axis] - from[axis]) / mode.cell[axis === "x" ? 0 : 1], from[axis] / mode.cell[axis === "x" ? 0 : 1]]);
				}
			}
			out.calibration = calibration;
			for (const axis of axes) console.log("calibration", axis, JSON.stringify(calibration[axis]), "units a count:", calibration[axis].map((c: number[]) => (c[1] / c[0]).toFixed(4)).join(" "));
			if (only === "calibrate") {
				await page.close();
				continue;
			}

			// How much one report can carry: one call of n counts across, and n as calls of 50 in one burst, against what n counts should move
			// (the patched driver's 7/16 of a pixel a count; the current bundle's speed-up makes these larger, and only their ratio to the
			// smallest is read). A report is a PS/2 packet, whose motion is 9 bits.
			const capacity: any = {};
			for (const n of [100, 200, 250, 255, 256, 300, 400, 800]) {
				const one: number[] = [];
				const burst: number[] = [];
				for (let run = 0; run < Math.min(runs, 3); run++) {
					await toBase();
					const before = await locate();
					await send(n, 0);
					const after = await locate();
					one.push(after && before ? (after.x - before.x) / mode.cell[0] : NaN);
					await toBase();
					const before2 = await locate();
					await sendMany(n / 50, 50, 0);
					const after2 = await locate();
					burst.push(after2 && before2 ? (after2.x - before2.x) / mode.cell[0] : NaN);
				}
				capacity[`${n} counts`] = { oneCall: stats(one), burstOf50: stats(burst) };
			}
			out.capacity = capacity;
			for (const [name, v] of Object.entries(capacity)) console.log("capacity", name.padEnd(11), "one call moves", (v as any).oneCall.mean.toFixed(1), "px; burst of 50s moves", (v as any).burstOf50.mean.toFixed(1), "px");

			// A clamp between two calls: the pointer is driven to the left edge in calls of -100 spaced 40 ms apart (each its own report; a report
			// holds 255 counts at most), the last of them g ms before a call of +100. When the two are separate reports the last -100 clamps and
			// the +100 moves the pointer 100 counts from the edge; when DOSBox joins them into one report they cancel and the pointer stays
			// where the earlier calls left it: at the edge.
			const clampGap: any = {};
			for (const gap of [0, 4, 8, 12, 16, 20, 24, 32, 40]) {
				const values: number[] = [];
				for (let run = 0; run < Math.min(runs, 3); run++) {
					await toBase();
					await sendMany(5, -100, 0, 40);
					await sleep(120);
					await send(-100, 0);
					if (gap) await sleep(gap);
					await send(100, 0);
					const after = await locate();
					values.push(after ? after.x / mode.cell[0] : -1);
				}
				clampGap[`${gap} ms`] = stats(values);
			}
			out.clampGap = clampGap;
			for (const [name, v] of Object.entries(clampGap)) console.log("clamp", name.padEnd(8), "x after:", (v as any).mean.toFixed(1), "(about 0 when the two calls were joined, about 44 when separate)");

			if (only === "clamp") {
				await page.close();
				continue;
			}

			// When do two calls become one report? Calls of 4 (the largest the driver does not speed up), then of 2, spaced g ms apart, 256 counts in
			// all: below the driver's speed-up each count is `slow` pixels, and a pair that DOSBox joins into one report of 8 is speeded up.
			const merge: any = {};
			for (const size of [4, 2]) {
				for (const gap of [2, 4, 6, 8, 10, 12, 16]) {
					const values: number[] = [];
					for (let run = 0; run < Math.min(runs, 3); run++) {
						await toBase();
						const before = await locate();
						await sendMany(256 / size, size, 0, gap);
						const after = await locate();
						values.push((after.x - before.x) / mode.cell[0] / 256);
					}
					merge[`${size} counts every ${gap} ms (x)`] = stats(values);
				}
			}
			out.merge = merge;
			for (const [name, v] of Object.entries(merge)) console.log("merge", name.padEnd(30), (v as any).mean.toFixed(3), "units a count +-", (v as any).sd.toFixed(3));

			// (3) linear? First the speed: 64 counts as calls of n every 16 ms (a frame), so n counts a frame is the speed; the mouse driver doubles
			// the motion above a speed threshold, and the emulator's clock is the host's.
			const rate: any = {};
			for (const axis of axes) {
				for (const n of [1, 2, 4, 8, 16, 32]) {
					const values: number[] = [];
					for (let run = 0; run < runs; run++) {
						await toBase();
						const before = await locate();
						await sendMany(64 / n, axis === "x" ? n : 0, axis === "y" ? n : 0, 16);
						const after = await locate();
						values.push((after[axis] - before[axis]) / mode.cell[axis === "x" ? 0 : 1] / 64);
					}
					rate[`${n} counts a frame (${axis})`] = stats(values);
				}
			}
			out.rate = rate;
			for (const [name, v] of Object.entries(rate)) console.log("rate", name.padEnd(30), (v as any).mean.toFixed(3), "units a count +-", (v as any).sd.toFixed(3));

			// (3) linear? From a base, one call of 32 against 32 calls of 1 (burst, and 12 ms apart), and 64 in one call against 4 of 16.
			const comparison: any = {};
			const trial = async (name: string, act: () => Promise<void>, axis: "x" | "y") => {
				const values: number[] = [];
				for (let run = 0; run < runs; run++) {
					await toBase();
					const before = await locate();
					await act();
					const after = await locate();
					values.push((after[axis] - before[axis]) / mode.cell[axis === "x" ? 0 : 1]);
				}
				comparison[`${name} (${axis})`] = stats(values);
			};
			for (const axis of axes) {
				const dx = axis === "x" ? 1 : 0;
				const dy = axis === "y" ? 1 : 0;
				await trial("1 call of 32", () => send(dx * 32, dy * 32), axis);
				await trial("32 calls of 1, burst", () => sendMany(32, dx, dy), axis);
				await trial("32 calls of 1, 12 ms apart", () => sendMany(32, dx, dy, 12), axis);
				await trial("1 call of 64", () => send(dx * 64, dy * 64), axis);
				await trial("4 calls of 16", () => sendMany(4, dx * 16, dy * 16), axis);
				// (4) fractions: 16 counts as 16 of 1, 64 of 0.25, 160 of 0.1, one call of 16.4 and 40 calls of 0.4.
				await trial("16 calls of 1", () => sendMany(16, dx, dy), axis);
				await trial("64 calls of 0.25, burst", () => sendMany(64, dx * 0.25, dy * 0.25), axis);
				await trial("64 calls of 0.25, 12 ms apart", () => sendMany(64, dx * 0.25, dy * 0.25, 12), axis);
				await trial("160 calls of 0.1, burst", () => sendMany(160, dx * 0.1, dy * 0.1), axis);
				await trial("40 calls of 0.4, burst", () => sendMany(40, dx * 0.4, dy * 0.4), axis);
				await trial("1 call of 16.4", () => send(dx * 16.4, dy * 16.4), axis);
				await trial("1 call of 0.9", () => send(dx * 0.9, dy * 0.9), axis);
			}
			out.comparison = comparison;
			for (const [name, v] of Object.entries(comparison)) console.log(name.padEnd(42), (v as any).mean.toFixed(3), "+-", (v as any).sd.toFixed(3));

			// (5) js-dos's own handlers with mouseCapture false: a real mouse over the picture, with and without the stage stopping the events.
			const canvas = page.locator("[data-dos] canvas").first();
			const box = (await canvas.boundingBox())!;
			out.overlay = await page.evaluate(() => {
				const names: string[] = [];
				document.querySelectorAll("[data-dos] *").forEach((el) => names.push(el.tagName.toLowerCase() + (el.className && typeof el.className === "string" ? "." + el.className.split(" ").join(".") : "")));
				const lockText = /click.*(capture|lock)|capture/i.test((document.querySelector("[data-dos]") as HTMLElement).innerText || "");
				return { elements: names.slice(0, 40), mentionsLock: lockText };
			});
			const realMoves: any = {};
			const moveReal = async (label: string) => {
				await park();
				await page.evaluate(() => (window as any).__spike.setRef());
				const startX = box.x + box.width * 0.4;
				const startY = box.y + box.height * 0.4;
				await page.mouse.move(startX, startY);
				await page.mouse.move(startX + 4, startY + 3, { steps: 4 });
				await page.mouse.move(box.x + box.width * 0.7, box.y + box.height * 0.6, { steps: 30 });
				await sleep(500);
				const found = await locate();
				const anywhere = await page.evaluate(() => (window as any).__spike.locate(24, 24, 0, 100000));
				realMoves[label] = {
					found: found && { x: found.x, y: found.y },
					changedPixelsAnywhere: anywhere ? anywhere.total : 0,
					wouldBeAt: { x: Math.round(0.7 * mode.range[0]), y: Math.round(0.6 * (mode.key === "text" ? 400 : 480)) },
				};
			};
			await moveReal("js-dos handlers running (mouseCapture false, no stop)");
			await page.evaluate(() => {
				const stage = document.querySelector("[data-stage]")!;
				for (const name of ["pointerdown", "pointermove", "pointerup", "mousedown", "mousemove", "mouseup", "mouseleave"]) stage.addEventListener(name, (e) => e.stopPropagation(), { capture: true });
			});
			await moveReal("stage stops the events in the capture phase");
			out.realMoves = realMoves;
			console.log("real mouse", JSON.stringify(realMoves));
			await shot("end");
			await page.close();

			// (6) a fresh page, no park, no ref: the pointer's first appearance after the mode's start.
			const fresh = await openQuill(base, key, launched);
			await sleep(14000);
			await fresh.evaluate(() => (window as any).__spike.setRef());
			await fresh.evaluate(() => (window as any).__spike.send(20, 0));
			const appeared = await fresh.evaluate(([w, h, top, bottom]) => (window as any).__spike.settled(w, h, top, bottom), [...mode.window, ...mode.rows]);
			out.pointerAtStart = appeared;
			await fresh.locator("[data-stage]").screenshot({ path: path.join(frames, `${browserName}-first-move.png`) });
			console.log("pointer appears at", JSON.stringify(appeared));
			await fresh.close();
		}
	} finally {
		fs.writeFileSync(path.join(outDir, `units-${browserName}${label ? "-" + label : ""}.json`), JSON.stringify(report, null, 1));
		await launched.close();
		if (stop) stop();
	}
}

await main();
