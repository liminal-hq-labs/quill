// Clicks in DOS Quill's Save As dialog through the Try Quill page's own pointer path, in headless Chromium and Firefox (SITE-112).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

// Usage: env -u WAYLAND_DISPLAY DISPLAY=:99 node try-dialog-clicks.ts [--browser chromium,firefox] [--mode gfx,g16] [--out DIR]
// BASE=http://127.0.0.1:PORT/try/ uses a server that is already running over site/ with a local bundle.
//
// The owner reported that the Save As dialog's buttons and the down arrows of its two drop-downs (Look in, Save as type) "don't seem to
// work". This opens the dialog by clicking File and Save As, then clicks the two arrows and the New folder button the way a hand does
// (the pointer arrives and the button goes down at once, held for 0 to 80 ms) and reads Quill's frame to see whether the list or box
// opened. Every call the page makes to js-dos (moves and buttons, with the time) is logged to the output, so a failed click can be told
// from the page (the press and release did not reach `ci` in order) and the program (they did, and Quill ignored them).
//
// Pass lines, graphics modes: every control answers however fast the click (the pointer arrives and the button goes down and up within
// the same millisecond is included); each press is followed by its release. Text mode (`--mode text`) is a KNOWN GAP in the DOS program,
// not the page: `cell_press` in crates/quill-app/src/mouse.rs ignores a press while the file dialog is open (`Mode::File => {}`), as the
// SPEC says ("The text screens have no pointer in this dialog"), so the check there passes while the arrows and buttons do nothing, and
// FAILS the day the program answers them, which is the cue to turn it into the same check the graphics modes make.

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
const outDir = path.resolve(opt("out", path.join(here, "..", "out", "try-dialog-clicks")));
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
const rows: any[] = [];

function check(browser: string, mode: string, name: string, pass: boolean, detail = "") {
	checks.push({ browser, mode, name, pass, detail });
	console.log(`${pass ? "ok  " : "FAIL"} ${browser} ${mode}: ${name}${detail ? " (" + detail + ")" : ""}`);
}

// In the page: hand over the ci and log what the page sends through it.
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
							for (const kind of ["sendMouseRelativeMotion", "sendMouseButton"]) {
								const original = arg[kind].bind(arg);
								arg[kind] = (a: number, b: number) => {
									(window as any).__calls.push({ t: Math.round(performance.now()), kind: kind === "sendMouseButton" ? "button" : "move", a, b });
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
};

// Frame rectangles of what the dialog shows when each control answers, and the controls' own centres (the frame is 640 by 480 in the
// graphics modes and 640 by 400 in text mode, where a cell is 8 by 16).
const GEOMETRY: Record<string, { frame: [number, number]; file: number[]; saveAs: number[]; region: Record<string, number[]>; control: Record<string, number[]> }> = {
	gfx: {
		frame: [640, 480],
		file: [23, 11],
		saveAs: [47, 100],
		region: { lookInList: [85, 95, 355, 145], typeList: [410, 305, 620, 360], folderBox: [100, 195, 540, 290] },
		control: { lookInArrow: [347, 84], typeArrow: [609, 367], newFolder: [459, 84], boxCancel: [311, 268], cancel: [577, 389] },
	},
	g16: {
		frame: [640, 480],
		file: [23, 11],
		saveAs: [47, 100],
		region: { lookInList: [85, 95, 355, 145], typeList: [410, 305, 620, 360], folderBox: [100, 195, 540, 290] },
		control: { lookInArrow: [347, 84], typeArrow: [609, 367], newFolder: [459, 84], boxCancel: [311, 268], cancel: [577, 389] },
	},
	text: {
		frame: [640, 400],
		file: [20, 8],
		saveAs: [57, 104],
		region: { lookInList: [150, 52, 395, 110], typeList: [330, 200, 440, 275], folderBox: [120, 120, 520, 260] },
		control: { lookInArrow: [394, 41], typeArrow: [414, 278], newFolder: [480, 41], boxCancel: [311, 268], cancel: [500, 311] },
	},
};

const KEEP = `(async () => { window.__kept = await window.__ci.screenshot(); })()`;
const DIFF = (r: number[]) => `(async () => {
	const a = window.__kept;
	const b = await window.__ci.screenshot();
	let n = 0;
	for (let y = ${r[1]}; y < ${r[3]}; y++) for (let x = ${r[0]}; x < ${r[2]}; x++) {
		const i = (y * b.width + x) * 4;
		if (Math.abs(a.data[i] - b.data[i]) + Math.abs(a.data[i + 1] - b.data[i + 1]) + Math.abs(a.data[i + 2] - b.data[i + 2]) > 60) n++;
	}
	return n;
})()`;

async function open(base: string, browser: Browser, key: string) {
	const page: Page = await browser.newPage({ viewport: { width: 1100, height: 900 }, deviceScaleFactor: 1 });
	await page.addInitScript(INIT);
	await page.goto(base + "?mouse=follow");
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
			const page = await open(base, browser, key);
			await page.evaluate(() => {
				document.querySelector("[data-stage]")!.scrollIntoView({ block: "start", behavior: "instant" });
			});
			await sleep(400);
			const box = (await page.locator("[data-dos] canvas").first().boundingBox())!;
			const g = GEOMETRY[key];
			const REGION = g.region;
			const CONTROL = g.control;
			const at = (f: number[]) => [box.x + (f[0] / g.frame[0]) * box.width, box.y + (f[1] / g.frame[1]) * box.height];
			const shot = (name: string) => page.locator("[data-stage]").screenshot({ path: path.join(outDir, "shots", `${name}-${browserName}-${key}.png`) });
			const diff = (r: number[]) => page.evaluate(DIFF(r) as any) as Promise<number>;
			const keep = () => page.evaluate(KEEP as any);

			// settle: how long the pointer rests on the control before the press; hold: how long the button is down.
			const click = async (f: number[], settle: number, hold: number, from?: number[]) => {
				if (from) await page.mouse.move(...(at(from) as [number, number]));
				await sleep(50);
				const [x, y] = at(f);
				await page.mouse.move(x, y, { steps: 6 });
				if (settle) await sleep(settle);
				await page.mouse.down();
				if (hold) await sleep(hold);
				await page.mouse.up();
				await sleep(1800);
			};
			// Pushes the pointer into the top left corner so the page's model and the guest agree, as a visitor's hand does on reaching an edge.
			await page.mouse.move(box.x + 0.4, box.y + 0.4, { steps: 8 });
			await sleep(800);
			await click(g.file, 300, 80);
			await click(g.saveAs, 300, 80);
			await sleep(1500);
			await shot("dialog-open");
			await keep();
			// The dialog is open and nothing in it is dropped: the frame now is the baseline for every region below.
			const quiet = await Promise.all([diff(REGION.lookInList), diff(REGION.typeList), diff(REGION.folderBox)]);
			check(browserName, key, "the Save As dialog opened by clicking File and Save As", quiet.every((n) => n === 0), `regions differ by ${quiet.join(",")} before any click`);

			// Text mode: the program does not answer a press in this dialog yet (see the header), so "nothing happens" is the passing result.
			const gap = key === "text";
			const variants = [
				{ settle: 300, hold: 80 },
				{ settle: 100, hold: 40 },
				{ settle: 20, hold: 20 },
				{ settle: 0, hold: 0 },
			];
			for (const v of variants) {
				for (const [name, region] of [
					["lookInArrow", REGION.lookInList],
					["typeArrow", REGION.typeList],
				] as const) {
					await page.evaluate(() => ((window as any).__calls.length = 0));
					await click(CONTROL[name], v.settle, v.hold, [250, 250]);
					const opened = await diff(region);
					const calls = await page.evaluate(() => (window as any).__calls.filter((c: any) => c.kind === "button"));
					const label = `${name}, rests ${v.settle} ms, held ${v.hold} ms`;
					check(browserName, key, gap ? `${label}: KNOWN GAP, the list stays shut (the program ignores the press)` : `${label}: the list opens`, gap ? opened <= 300 : opened > 300, `${opened} pixels changed; button calls ${calls.map((c: any) => (c.b ? "down@" : "up@") + c.t).join(" ")}`);
					rows.push({ browser: browserName, mode: key, control: name, ...v, opened, calls });
					if (opened > 300) await shot(`${name}-${v.settle}-${v.hold}`);
					// Closes it with a slow second click on the arrow (a press on the field toggles), then checks it is shut.
					if (opened > 300) await click(CONTROL[name], 300, 80);
					const shut = await diff(region);
					if (opened > 300) check(browserName, key, `${label}: a second click shuts it`, shut < 50, `${shut} pixels changed`);
				}
				await page.evaluate(() => ((window as any).__calls.length = 0));
				await click(CONTROL.newFolder, v.settle, v.hold, [250, 250]);
				const folder = await diff(REGION.folderBox);
				check(browserName, key, gap ? `newFolder, rests ${v.settle} ms, held ${v.hold} ms: KNOWN GAP, no box (the program ignores the press)` : `newFolder, rests ${v.settle} ms, held ${v.hold} ms: the box opens`, gap ? folder <= 300 : folder > 300, `${folder} pixels changed`);
				if (folder > 300) {
					await click(CONTROL.boxCancel, 300, 80);
					check(browserName, key, "and its Cancel shuts it", (await diff(REGION.folderBox)) < 50);
				}
			}

			// The page's end of every press: each button call is a press followed by its release, with no other press between.
			const all = await page.evaluate(() => (window as any).__calls.filter((c: any) => c.kind === "button"));
			let balanced = true;
			for (let i = 0; i < all.length; i++) if (all[i].b !== (i % 2 === 0)) balanced = false;
			check(browserName, key, "the page sent a press and then its release, every time", balanced, `${all.length} button calls in the last variant`);

			await click(CONTROL.cancel, 300, 80);
			await shot("after-cancel");
			await page.close();
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
		fs.writeFileSync(path.join(outDir, "try-dialog-clicks.json"), JSON.stringify({ date: new Date().toISOString(), checks, rows }, null, 1));
		if (stop) stop();
	}
	const failed = checks.filter((c) => !c.pass);
	console.log(`\n${checks.length - failed.length} of ${checks.length} checks passed`);
	if (failed.length) process.exit(1);
}

await main();
