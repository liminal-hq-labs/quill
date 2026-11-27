// Checks the DOS Try Quill page's bridge (SITE-60 to SITE-62) in headless browsers, in each display mode.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// What it checks: the boot has no partition warning on the screens it can catch, FreeDOS's DATE shows the browser's day (the browser's clock is
// fixed to 25 December 2026 so it is not today by chance), a file the page writes on D: is in DOS after SYNC, a file the guest writes on C: is
// read by the page after SYNC, and ?bridge=off gives the page as it was (the date is 25 July 1998, there is no bridge line and the module is
// never fetched). Every request is recorded: all go to this server, all are GET, none carries a body, and none has a query beyond ?v= and ?t=.
//
// Needs the page built (bash scripts/build-try.sh --build, or without --build with the programs named by QUILLDOS_EXE, QUILL_FNT, QUILL_DICT)
// so site/try holds quill.<hash>.jsdos and try-manifest.json. Starts its own server on 127.0.0.1 and writes its process id to PIDFILE.
// Browsers are headless, Chromium from /usr/bin/chromium (CHROMIUM) and Playwright's Firefox. Input goes through the player's command
// interface, never the host desktop. Run with DISPLAY set explicitly and WAYLAND_DISPLAY unset.
//
// usage: env -u WAYLAND_DISPLAY DISPLAY=:97 PLAYWRIGHT_SKIP_BROWSER_GC=1 PORT=18431 PIDFILE=/tmp/x.pid bun check-dos-bridge.ts OUTDIR [--browsers chromium,firefox] [--modes gfx,g16,text] [--baseline-only]
// BASELINE_SITE=DIR (a site folder built from main, served on PORT+1) adds a pixel comparison of Quill's first screen with ?bridge=off.

import fs from "node:fs";
import http from "node:http";
import path from "node:path";
import { createHash } from "node:crypto";
import { chromium, firefox } from "playwright-core";
import type { Browser, Page } from "playwright-core";

const args = process.argv.slice(2);
const outDir = args[0];
if (!outDir || outDir.startsWith("--")) throw new Error("usage: check-dos-bridge.ts OUTDIR [--browsers chromium,firefox] [--modes gfx,g16,text]");
const flag = (name: string, fallback: string): string => {
	const at = args.indexOf(name);
	return at >= 0 ? (args[at + 1] ?? fallback) : fallback;
};
const browserNames = flag("--browsers", "chromium,firefox").split(",");
const modes = flag("--modes", "gfx,g16,text").split(",") as Mode[];
type Mode = "gfx" | "g16" | "text";
const CHROMIUM = process.env.CHROMIUM || "/usr/bin/chromium";
const PORT = Number(process.env.PORT || 18431);
const PIDFILE = process.env.PIDFILE || path.join(outDir, "server.pid");
const SITE = path.resolve(import.meta.dir, "..", "..");
fs.mkdirSync(outDir, { recursive: true });

const failures: string[] = [];
const expect = (ok: boolean, message: string): void => {
	if (!ok) {
		failures.push(message);
		console.error("FAIL " + message);
	}
};
const sleep = (ms: number): Promise<void> => new Promise((r) => setTimeout(r, ms));
const sha256 = (bytes: Uint8Array): string => createHash("sha256").update(bytes).digest("hex");

// The servers: a whole site folder, read only, on this machine only.
const types: Record<string, string> = { ".html": "text/html; charset=utf-8", ".js": "text/javascript; charset=utf-8", ".css": "text/css", ".json": "application/json", ".wasm": "application/wasm", ".png": "image/png", ".svg": "image/svg+xml", ".ico": "image/x-icon", ".jsdos": "application/octet-stream" };
function serve(root: string, port: number): Promise<http.Server> {
	const server = http.createServer((req, res) => {
		const url = new URL(req.url ?? "/", "http://x");
		let file = path.resolve(root, "." + decodeURIComponent(url.pathname));
		if (!file.startsWith(root)) {
			res.writeHead(403).end();
			return;
		}
		if (fs.existsSync(file) && fs.statSync(file).isDirectory()) file = path.join(file, "index.html");
		if (!fs.existsSync(file)) {
			res.writeHead(404).end();
			return;
		}
		res.writeHead(200, { "content-type": types[path.extname(file)] ?? "application/octet-stream", "cache-control": "no-store" });
		fs.createReadStream(file).pipe(res);
	});
	return new Promise((resolve) => server.listen(port, "127.0.0.1", () => resolve(server)));
}
const server = await serve(SITE, PORT);
fs.writeFileSync(PIDFILE, String(process.pid));
const BASE = `http://127.0.0.1:${PORT}/try/`;
// BASELINE_SITE: a site folder built from main (before the bridge), to compare ?bridge=off with.
const BASELINE = process.env.BASELINE_SITE ? path.resolve(process.env.BASELINE_SITE) : null;
const baselineServer = BASELINE ? await serve(BASELINE, PORT + 1) : null;

// js-dos key codes (GLFW's): letters and digits are their capitals' codes.
const SHIFT = 340;
const CTRL = 341;
const ENTER = 257;
const SHIFTED: Record<string, number> = { ":": 59, ">": 46, "<": 44, "|": 92, _: 45, $: 52 };
const PLAIN: Record<string, number> = { " ": 32, ".": 46, "-": 45, "/": 47, "\\": 92, "=": 61, ",": 44 };

async function press(page: Page, codes: number[]): Promise<void> {
	await page.evaluate((c) => {
		const ci = (window as any).__ci;
		for (const k of c) ci.sendKeyEvent(k, true);
		for (const k of [...c].reverse()) ci.sendKeyEvent(k, false);
	}, codes);
}

async function type(page: Page, text: string): Promise<void> {
	for (const ch of text) {
		const up = ch.toUpperCase();
		if (ch === "\n") await press(page, [ENTER]);
		else if (SHIFTED[ch] !== undefined) await press(page, [SHIFT, SHIFTED[ch]!]);
		else if (PLAIN[ch] !== undefined) await press(page, [PLAIN[ch]!]);
		else if (/^[A-Z0-9]$/.test(up)) await press(page, [up.charCodeAt(0)]);
		else throw new Error(`no key for ${JSON.stringify(ch)}`);
		await sleep(30);
	}
}

const line = async (page: Page, text: string, wait = 1500): Promise<void> => {
	await type(page, text + "\n");
	await sleep(wait);
};

interface Request {
	method: string;
	url: string;
	body: boolean;
}

interface Row {
	browser: string;
	mode: string;
	bridge: string;
	bridgeState: string;
	bridgeWriteMs: number | null;
	readCMs: number | null;
	date: string;
	fromPc: string;
	toPc: string;
	requests: number;
}
const rows: Row[] = [];

async function runCase(browser: Browser, browserName: string, mode: Mode, bridgeOn: boolean, hold: boolean): Promise<void> {
	const tag = `${browserName}-${mode}-${bridgeOn ? "bridge" : "off"}`;
	const context = await browser.newContext({ viewport: { width: 1000, height: 900 }, serviceWorkers: "block" });
	const page = await context.newPage();
	const requests: Request[] = [];
	page.on("request", (r) => requests.push({ method: r.method(), url: r.url(), body: r.postData() !== null }));
	const problems: string[] = [];
	page.on("pageerror", (e) => problems.push(String(e)));
	if (bridgeOn) await page.clock.setFixedTime(new Date(2026, 11, 25, 10, 0, 0));
	await page.goto(BASE + (bridgeOn ? "" : "?bridge=off"), { waitUntil: "load" });
	const shot = async (name: string): Promise<void> => {
		const canvas = page.locator("[data-dos] canvas").first();
		fs.writeFileSync(path.join(outDir, `${tag}-${name}.png`), await canvas.screenshot());
	};

	// ?bridge=off: nothing of the bridge exists. Then, either way, a hook records the player's command interface for the test's keys.
	const pre = await page.evaluate(() => ({ defined: typeof (window as any).quillBridge, line: (document.querySelector("[data-bridge-state]") as HTMLElement | null)?.hidden }));
	if (!bridgeOn) {
		expect(pre.defined === "undefined", `${tag}: window.quillBridge exists with ?bridge=off`);
		expect(!requests.some((r) => r.url.includes("dos-bridge")), `${tag}: the module was fetched with ?bridge=off`);
	} else {
		expect(pre.defined === "object", `${tag}: window.quillBridge is missing`);
		expect(requests.some((r) => r.url.includes("lib/dos-bridge.js?v=")), `${tag}: the module was not fetched with its version`);
	}
	await page.evaluate((hold) => {
		const w = window as any;
		const wrap = (ci: any) => {
			w.__ci = ci;
			if (hold) {
				const real = ci.simulateKeyPress.bind(ci);
				ci.simulateKeyPress = (...a: number[]) => {
					w.__release = () => real(...a);
				};
			}
		};
		if (w.quillBridge) {
			const attach = w.quillBridge.attach;
			w.quillBridge.attach = function (ci: any) {
				wrap(ci);
				return attach.call(this, ci);
			};
		} else {
			w.quillBridge = { attach: wrap };
		}
	}, hold);

	await page.check(`input[name=mode][value=${mode}]`);
	const started = Date.now();
	await page.click("[data-start]");
	await page.waitForFunction(() => (window as any).__ci, null, { timeout: 120000 });
	const ciReady = Date.now() - started;

	let bridgeState = "";
	let bridgeWriteMs: number | null = null;
	if (bridgeOn) {
		const state = await page.evaluate(async () => {
			const w = window as any;
			for (let i = 0; i < 200 && !w.quillBridge.ready; i++) await new Promise((r) => setTimeout(r, 25));
			return await w.quillBridge.ready;
		});
		bridgeState = await page.locator("[data-bridge-state]").innerText();
		bridgeWriteMs = state.writeMs;
		expect(state.status === "on", `${tag}: the bridge is ${state.status} (${state.error})`);
		expect(state.date === "12-25-2026", `${tag}: the bridge wrote the date ${state.date}, not 12-25-2026`);
		expect(/^bridge: on, D: written \d+ ms, date 12-25-2026$/.test(bridgeState), `${tag}: the diagnostics line says "${bridgeState}"`);
	} else {
		const hidden = await page.evaluate(() => (document.querySelector("[data-bridge-state]") as HTMLElement).hidden);
		expect(hidden, `${tag}: the bridge line shows with ?bridge=off`);
	}

	if (hold) {
		// The boot, frame by frame (the menu is held until the frames are taken): the kernel's messages about the disks go by before the
		// menu clears the screen, and a partition warning would be on one of them. Each frame that differs from the last is kept.
		let last = "";
		let kept = 0;
		for (let i = 0; i < 60 && kept < 10; i++) {
			const frame = await page.locator("[data-dos] canvas").first().screenshot();
			const digest = sha256(frame);
			if (digest !== last) {
				fs.writeFileSync(path.join(outDir, `${tag}-1-boot-${String(kept).padStart(2, "0")}.png`), frame);
				kept++;
				last = digest;
			}
		}
		await page.evaluate(() => (window as any).__release?.());
	}
	await sleep(mode === "text" ? 9000 : 12000);
	await shot("2-quill");

	// Leave Quill (Ctrl+Q) for the prompt.
	await press(page, [CTRL, "Q".charCodeAt(0)]);
	await sleep(3000);
	await shot("3-prompt");

	// The date, read from C: by the page: the guest wrote DATE's answer to a file and flushed with SYNC.
	await line(page, "ECHO.|DATE >C:\\DATE.TXT", 2000);
	await line(page, "SYNC");
	const module = BASE + "lib/dos-bridge.js";
	const readFromC = async (name: string) =>
		page.evaluate(
			async ([url, file]) => {
				const m = await import(/* @vite-ignore */ url!);
				const t = performance.now();
				const bytes: Uint8Array = await (window as any).__ci.fsReadFile("c.img");
				const volume = m.openVolume(bytes);
				let data: number[] | null = null;
				try {
					data = Array.from(m.readFile(volume, file!));
				} catch (e) {
					data = null;
				}
				return { data, ms: Math.round(performance.now() - t) };
			},
			[module, name] as [string, string],
		);
	const dateFile = await readFromC("DATE.TXT");
	const dateText = dateFile.data ? String.fromCharCode(...dateFile.data) : "";
	const dateMatch = /(\d\d-\d\d-\d{4})/.exec(dateText);
	const date = dateMatch?.[1] ?? `(not found in ${JSON.stringify(dateText)})`;
	const want = bridgeOn ? "12-25-2026" : "07-25-1998";
	expect(date === want, `${tag}: DATE says ${date}, wanted ${want}`);

	let fromPc = "n/a";
	let toPc = "n/a";
	let readCMs: number | null = dateFile.ms;
	if (bridgeOn) {
		// The page writes a file onto D: (here from the console; the files panel comes with a later package) and the guest sees it after SYNC.
		const payload = Array.from({ length: 100_000 }, (_, i) => (i * 31 + 7) % 251);
		const written = await page.evaluate(async (data) => {
			const b = (window as any).quillBridge.active();
			const t = performance.now();
			const state = await b.write([{ name: "BIG.BIN", bytes: Uint8Array.from(data), time: new Date() }, { name: "HELLO.TXT", bytes: Uint8Array.from("hello from the page\r\n", (c) => c.charCodeAt(0)), time: new Date() }]);
			return { status: state.status, ms: Math.round(performance.now() - t) };
		}, payload);
		expect(written.status === "on", `${tag}: writing D: gave ${written.status}`);
		await line(page, "SYNC");
		await line(page, "DIR D: /B >C:\\DIRD.TXT");
		await line(page, "COPY D:\\HELLO.TXT C:\\GOT.TXT");
		await line(page, "COPY D:\\BIG.BIN C:\\BIGGOT.BIN", 2500);
		await line(page, "SYNC", 2000);
		const dir = await readFromC("DIRD.TXT");
		const dirText = dir.data ? String.fromCharCode(...dir.data) : "";
		expect(/SETDATE\.BAT/.test(dirText) && /HELLO\.TXT/.test(dirText) && /BIG\.BIN/.test(dirText), `${tag}: DIR D: after SYNC lists ${JSON.stringify(dirText)}`);
		const got = await readFromC("GOT.TXT");
		const gotText = got.data ? String.fromCharCode(...got.data) : "";
		expect(gotText === "hello from the page\r\n", `${tag}: the guest read ${JSON.stringify(gotText)} from D:`);
		const big = await readFromC("BIGGOT.BIN");
		const bigOk = !!big.data && sha256(Uint8Array.from(big.data)) === sha256(Uint8Array.from(payload));
		expect(bigOk, `${tag}: the 100,000-byte file copied D: to C: is not the file the page wrote`);
		fromPc = `${gotText.length} bytes and 100000 bytes (sha-256 ${bigOk ? "equal" : "DIFFERENT"})`;

		// The guest writes C: and the page reads it: the date file above was one; a second written at the prompt, with and without SYNC.
		await line(page, "ECHO written at the prompt>C:\\NOTE.TXT");
		await line(page, "SYNC");
		const note = await readFromC("NOTE.TXT");
		const noteText = note.data ? String.fromCharCode(...note.data) : "";
		expect(/^written at the prompt/.test(noteText), `${tag}: the page read ${JSON.stringify(noteText)} from C: after SYNC`);
		toPc = `${noteText.length} bytes and the date file (${dateText.length} bytes)`;
		readCMs = note.ms;
		// The bridge is still on after all this (the observer did not let go of a running player).
		const still = await page.evaluate(() => (window as any).quillBridge.active()?.state().status);
		expect(still === "on", `${tag}: the bridge is ${still} after the run`);
	}

	await line(page, "ECHO.|DATE", 1500);
	await line(page, "DIR C:", 1500);
	await line(page, "DIR D:", 1500);
	await shot("4-date-and-disks");

	// Stop and restart: the bridge lets go and attaches again (the page's own Restart button).
	if (bridgeOn && mode === "gfx") {
		await page.click("[data-restart]");
		await sleep(500);
		const mid = await page.evaluate(() => ({ active: (window as any).quillBridge.active() !== null, text: (document.querySelector("[data-bridge-state]") as HTMLElement).textContent }));
		expect(!mid.active || mid.text !== "", `${tag}: restart left the bridge in a strange state`);
		await page.waitForFunction(() => (window as any).quillBridge.active()?.state().status === "on", null, { timeout: 120000 });
	}

	// The network: every request to this server, GET, no body, no query but ?v= and ?t=.
	const foreign = requests.filter((r) => !r.url.startsWith(`http://127.0.0.1:${PORT}/`) && !r.url.startsWith("data:") && !r.url.startsWith("blob:"));
	expect(foreign.length === 0, `${tag}: requests left the server: ${foreign.map((r) => r.url).join(", ")}`);
	expect(requests.every((r) => r.method === "GET" && !r.body), `${tag}: a request was not a bodiless GET`);
	const odd = requests.filter((r) => {
		const q = new URL(r.url).search;
		return q !== "" && !/^\?(v=[0-9a-f]+|t=\d+|bridge=off)$/.test(q);
	});
	expect(odd.length === 0, `${tag}: a request carried a query: ${odd.map((r) => r.url).join(", ")}`);
	expect(problems.length === 0, `${tag}: page errors: ${problems.join("; ")}`);

	rows.push({ browser: browserName, mode, bridge: bridgeOn ? "on" : "off", bridgeState, bridgeWriteMs, readCMs, date, fromPc, toPc, requests: requests.length });
	console.log(`${tag}: ci-ready ${ciReady} ms, ${bridgeState || "no bridge"}, DATE ${date}`);
	await context.close();
}

// The page before the bridge against ?bridge=off: Quill's first screen in each mode, pixel for pixel.
async function compareBaseline(browser: Browser, browserName: string, mode: Mode): Promise<void> {
	const frames: Record<string, string> = {};
	for (const [label, url] of [["before", `http://127.0.0.1:${PORT + 1}/try/`], ["off", BASE + "?bridge=off"]] as const) {
		const context = await browser.newContext({ viewport: { width: 1000, height: 900 }, serviceWorkers: "block" });
		const page = await context.newPage();
		await page.goto(url, { waitUntil: "load" });
		await page.check(`input[name=mode][value=${mode}]`);
		await page.click("[data-start]");
		await page.waitForSelector("[data-dos] canvas", { timeout: 120000 });
		await sleep(mode === "text" ? 11000 : 14000);
		const png = await page.locator("[data-dos] canvas").first().screenshot();
		fs.writeFileSync(path.join(outDir, `${browserName}-${mode}-quill-${label}.png`), png);
		frames[label] = sha256(png);
		await context.close();
	}
	// Quill shows its status bar's clock in no mode; the first screen is the same document with the same menus. A difference is a failure.
	expect(frames.before === frames.off, `${browserName}-${mode}: Quill's first screen with ?bridge=off differs from the page before the bridge`);
	console.log(`${browserName}-${mode}: Quill's first screen before the bridge and with ?bridge=off are ${frames.before === frames.off ? "pixel for pixel equal" : "DIFFERENT"}`);
}

try {
	for (const name of browserNames) {
		const browser = name === "firefox" ? await firefox.launch({ headless: true }) : await chromium.launch({ executablePath: CHROMIUM, headless: true, args: ["--no-sandbox"] });
		try {
			if (!args.includes("--baseline-only")) {
				for (const mode of modes) await runCase(browser, name, mode, true, mode === "gfx");
				// ?bridge=off: the page as it was, in the default mode and in text mode.
				for (const mode of modes.filter((m) => m !== "g16")) await runCase(browser, name, mode, false, false);
			}
			if (BASELINE) for (const mode of modes) await compareBaseline(browser, name, mode);
		} finally {
			await browser.close();
		}
	}
} finally {
	fs.writeFileSync(path.join(outDir, "results.json"), JSON.stringify({ rows, failures }, null, "\t") + "\n");
	server.close();
	baselineServer?.close();
	fs.rmSync(PIDFILE, { force: true });
}
console.log(failures.length === 0 ? `ok: ${rows.length} runs` : `${failures.length} failures`);
process.exit(failures.length === 0 ? 0 : 1);
