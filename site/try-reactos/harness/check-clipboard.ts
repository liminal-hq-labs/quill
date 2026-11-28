// The shared clipboard end to end (WP-HOST-4), in headless Chromium or Firefox: the page's own gestures and paste flow against the real guest
// helper (HOSTLINK.EXE, running in the state host-link-boot.ts saved), with Notepad (and Paintbrush) as the applications.
//   copy in Notepad lands on the page clipboard, byte for byte; paste from the page (text with accents, 1 MiB, RTF, a PNG as a DIB) reaches
//   the guest; no duplicates; a restart mid-transfer; the power cycle; no network request carries anything; the clipboard is never read without
//   a gesture; the toggle; an older state with no helper.
// Input into the guest goes through the page's emulator API, except the keys that are the gestures under test (Ctrl+C, Ctrl+V, Enter), which
// Playwright sends as real key events; pixels come from the emulator canvas. Never the host desktop (headless, no display needed).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// usage: BASE=http://127.0.0.1:18394 bun check-clipboard.ts OUTDIR [--browser chromium|firefox] [--state /images/hl-state.bin] [--disk /images/hl-e2e.img]
//          [--only a,b,c] [--nohelper-state /images/hl-nohelper.bin]       (the server: PORT=18394 IMAGES_DIR=... bun serve.ts; the disk and state: host-link-disk.sh, host-link-boot.ts)
import fs from "node:fs";
import path from "node:path";
import { chromium, firefox } from "playwright-core";
import type { Browser, Page } from "playwright-core";
import { CHROMIUM, grab, pageUrl, save, sleep, waitFor } from "./lib.ts";

const args = process.argv.slice(2);
const outDir = args[0];
if (!outDir) throw new Error("usage: check-clipboard.ts OUTDIR [--browser chromium|firefox] [--only a,b]");
const opt = (key: string, fallback: string) => (args.includes("--" + key) ? args[args.indexOf("--" + key) + 1]! : fallback);
const browserName = opt("browser", "chromium") as "chromium" | "firefox";
const only = opt("only", "").split(",").filter(Boolean);
const wanted = (name: string) => only.length === 0 || only.includes(name);
const stateUrl = opt("state", "/images/hl-state.bin");
const diskUrl = opt("disk", "/images/hl-e2e.img");
const noHelperState = opt("nohelper-state", "");
const shotDir = path.join(outDir, browserName);

const KEY = { ctrl: [0x1d], a: [0x1e], c: [0x2e], v: [0x2f], enter: [0x1c], printScreen: [0xe0, 0x37], esc: [0x01], win: [0xe0, 0x5b], r: [0x13], alt: [0x38] };
const results: { name: string; ok: boolean; detail: Record<string, unknown> }[] = [];
const record = (name: string, ok: boolean, detail: Record<string, unknown> = {}) => {
	results.push({ name, ok, detail });
	console.log(JSON.stringify({ browser: browserName, name, ok, ...detail }));
};

const browser: Browser =
	browserName === "firefox"
		? await firefox.launch({ headless: true, firefoxUserPrefs: { "dom.events.asyncClipboard.clipboardItem": true, "dom.events.testing.asyncClipboard": true, "dom.events.asyncClipboard.readText": true, "dom.events.asyncClipboard.read": true } })
		: await chromium.launch({ executablePath: CHROMIUM, headless: true, args: ["--no-first-run", "--disable-background-timer-throttling", "--disable-renderer-backgrounding"] });
const context = await browser.newContext({ viewport: { width: 1280, height: 1000 }, serviceWorkers: "block", ...(browserName === "chromium" ? { permissions: ["clipboard-read", "clipboard-write"] } : {}) });
// The browser's clipboard as the page uses it: every write is recorded (with its text, or a picture's size) and every read is counted, so the
// checks can say what was written and that nothing was read without a gesture. The page's own calls still go through.
await context.addInitScript({ path: new URL("./inpage.js", import.meta.url).pathname });
await context.addInitScript(() => {
	const w = window as any;
	w.__clip = { writes: [] as any[], reads: 0, readTexts: 0, testing: false, readTimes: [] as number[] };
	const clipboard = navigator.clipboard;
	if (!clipboard) return;
	const write = clipboard.write.bind(clipboard);
	clipboard.write = (items: ClipboardItem[]) => {
		const started = performance.now();
		const done = write(items);
		(async () => {
			const record: any = { at: started, types: {} };
			for (const item of items) {
				for (const type of item.types) {
					const blob = await item.getType(type);
					if (type.startsWith("text/")) record.types[type] = { size: blob.size, text: await blob.text() };
					else {
						const bitmap = await createImageBitmap(blob);
						const canvas = document.createElement("canvas");
						canvas.width = bitmap.width;
						canvas.height = bitmap.height;
						const g = canvas.getContext("2d")!;
						g.drawImage(bitmap, 0, 0);
						const px = g.getImageData(0, 0, bitmap.width, bitmap.height).data;
						let nonBlack = 0;
						for (let i = 0; i < px.length; i += 64) if (px[i]! + px[i + 1]! + px[i + 2]! > 30) nonBlack++;
						record.types[type] = { size: blob.size, width: bitmap.width, height: bitmap.height, nonBlackSamples: nonBlack };
					}
				}
			}
			record.resolvedAt = performance.now();
			w.__clip.writes.push(record);
		})().catch((error) => w.__clip.writes.push({ error: String(error) }));
		return done;
	};
	for (const name of ["read", "readText"] as const) {
		const original = (clipboard as any)[name].bind(clipboard);
		(clipboard as any)[name] = (...a: unknown[]) => {
			// The test's own reads (it checks what the page wrote) are marked and not counted: the page's are what matter.
			if (!w.__clip.testing) {
				w.__clip[name === "read" ? "reads" : "readTexts"]++;
				w.__clip.readTimes.push(performance.now());
			}
			return original(...a);
		};
	}
});
const page: Page = await context.newPage();
const requests: { method: string; url: string; body: boolean }[] = [];
let watching = false;
page.on("request", (request) => {
	if (watching) requests.push({ method: request.method(), url: request.url(), body: request.postData() !== null });
});
page.on("pageerror", (error) => console.log("pageerror:", error.message));
const ev = <T>(expression: string): Promise<T> => page.evaluate(expression) as Promise<T>;
const writes = () => ev<any[]>("window.__clip.writes");
const status = () => ev<any>("window.tryReactos.clipboard.status()");
const stats = () => ev<any>("window.tryReactos.hostLink.stats()");

/** Keys down in order, then up in reverse order, through the page's emulator API (not a gesture). */
const press = (codes: number[][]) =>
	page.evaluate((keys: number[][]) => {
		for (const code of keys) window.tryReactos.input.key(code);
		for (const code of [...keys].reverse()) window.tryReactos.input.key(code.map((value: number) => value | 0x80));
	}, codes);
const typeText = async (text: string, pause = 70) => {
	for (const ch of text) {
		if (ch === "\n") await press([KEY.enter]);
		else await page.evaluate((c: string) => window.tryReactos.input.text(c), ch);
		await sleep(pause);
	}
};
/** The Run box, with the Enter either through the emulator API (`real` false: no gesture) or as a real key event (a gesture). */
const runCommand = async (command: string, real = false) => {
	await press([KEY.win, KEY.r]);
	await sleep(2500);
	await typeText(command, 110);
	await sleep(400);
	if (real) await page.keyboard.press("Enter");
	else await press([KEY.enter]);
};
const shot = async (name: string, withPage = false) => {
	const frame = await grab(page);
	if (frame) save(shotDir, name + ".png", frame.png);
	if (withPage) save(shotDir, name + "-page.png", await page.screenshot({ fullPage: false }));
};
const waitWrites = async (count: number, timeoutMs: number) => {
	const t = await waitFor(page, `window.__clip.writes.length >= ${count}`, timeoutMs, 50);
	return t;
};
/** A click on the page's own controls: the visitor lets the pointer go first (Esc), since a locked pointer sends every click to the screen. */
const clickUi = async (selector: string) => {
	await page.evaluate(() => document.exitPointerLock && document.exitPointerLock());
	await sleep(150);
	await page.click(selector);
};
const chip = () => page.textContent("[data-clip-chip]");
const sha = (text: string) => page.evaluate(async (t: string) => Array.from(new Uint8Array(await crypto.subtle.digest("SHA-256", new TextEncoder().encode(t)))).map((b) => b.toString(16).padStart(2, "0")).join(""), text);
const big = (n: number) => {
	const lines: string[] = [];
	for (let k = 0; k < n / 16; k++) lines.push(String(k).padStart(14, "0") + "\r\n");
	return lines.join("").slice(0, n);
};
/** Puts text on the browser's clipboard the way another application would. */
const setBrowserText = (text: string) => page.evaluate((t: string) => navigator.clipboard.writeText(t), text);
/** Counts what the page sends the guest, by op, from the moment it is installed. */
const instrument = () =>
	page.evaluate(() => {
		const w = window as any;
		w.__ops = { set: 0, part: 0, get: 0, pause: 0, resume: 0 };
		w.__pushes = [];
		const host = window.tryReactos.hostLink;
		const send = host.send.bind(host);
		host.send = (channel: string, op: string, ...rest: any[]) => {
			if (w.__ops[op] !== undefined) w.__ops[op]++;
			return send(channel, op, ...rest);
		};
		const sync = window.tryReactos.clipboard;
		const push = sync.push.bind(sync);
		sync.push = async (content: any) => {
			const t0 = performance.now();
			const bytes0 = host.stats().bytesOut;
			const result = await push(content);
			w.__pushes.push({ result, ms: Math.round(performance.now() - t0), wire: host.stats().bytesOut - bytes0 });
			return result;
		};
	});
const pushes = () => ev<any[]>("window.__pushes");
const ops = () => ev<Record<string, number>>("window.__ops");

try {
	// 0. The restore: the helper is already running in the saved state, and the page's hello is answered at once (the first hello used to
	// wait for the 5 s retry because v86 drops what is sent before its serial port exists).
	await page.goto(pageUrl("clip-" + Date.now().toString(36), { disk: diskUrl, boot: "hd", ram: "256", state: stateUrl }));
	await clickUi("[data-start]");
	const readyMs = await waitFor(page, "window.tryReactos.hostLink && window.tryReactos.hostLink.stats().state === 'ready'", 120000, 20);
	const s0 = await stats();
	record("restore-hello-answered-at-once", readyMs !== null && s0.helloMs !== null && s0.helloMs < 1000 && s0.peer?.app?.startsWith("HOSTLINK/"), { helloMs: s0.helloMs, readyAfterStartMs: readyMs, peer: s0.peer?.app });
	if (readyMs === null) throw new Error("no helper");
	await sleep(1500);
	watching = true;
	await instrument();
	const row = await ev<string>("document.querySelector('[data-clip-readout] dd').textContent");
	const chip0 = await chip();
	record("chip-and-row-before-the-first-click", /connected/.test(chip0 ?? "") && /Click the screen/.test(chip0 ?? "") && /connected/.test(row), { chip: chip0, row });
	await page.evaluate(() => ((document.querySelector("[data-diag]") as HTMLDetailsElement).open = true));
	await shot("00-restored", true);

	// 1. Select the screen (the first click), open Notepad, and copy typed text with a real Ctrl+A, Ctrl+C.
	await page.click("[data-stage]");
	await sleep(300);
	const chip1 = await chip();
	record("first-click-starts-sharing", chip1 === "Clipboard: connected", { chip: chip1 });
	await runCommand("notepad");
	await sleep(7000);
	await shot("01-notepad-open");
	if (wanted("copy")) {
		await typeText("Hello from Notepad\nSecond line 123 !?");
		await sleep(800);
		const before = (await writes()).length;
		await page.keyboard.press("Control+a");
		await sleep(300);
		const keyAt = await ev<number>("performance.now()");
		await page.keyboard.press("Control+c");
		const t = await waitWrites(before + 1, 8000);
		await sleep(2500);
		const all = await writes();
		const text = all[before]?.types?.["text/plain"]?.text;
		const readback = await ev<string>("(async () => { window.__clip.testing = true; try { return await navigator.clipboard.readText(); } finally { window.__clip.testing = false; } })()").catch((e) => "ERR " + e);
		record("copy-in-notepad-lands-on-the-page-clipboard", text === "Hello from Notepad\r\nSecond line 123 !?", { got: text, readback, readbackSame: readback === text, readbackSameModuloCrlf: typeof readback === "string" && readback.replace(/\r\n/g, "\n") === (text ?? "").replace(/\r\n/g, "\n"), latencyMs: all[before] ? Math.round(all[before].at - keyAt) : null, writesForOneCopy: all.length - before, waitedMs: t });
		const st = await status();
		record("one-copy-one-write-no-duplicate", all.length - before === 1 && st.counters.offers === 1 && st.counters.copiedOut === 1, { counters: st.counters, chip: await chip() });
		await shot("02-copied", true);
	}

	// 2. Page to guest: a text with accents, CRLF, an euro sign and a smiley, by the real Ctrl+V; then back out of the guest, equal.
	if (wanted("paste")) {
		const sample = "Zoë café — naïve façade, À la carte: 5 €\r\nSecond line, Ünïcödé ☺ ok\r\n";
		await setBrowserText(sample);
		await page.keyboard.press("Control+a");
		await sleep(300);
		const o0 = await ops();
		await page.keyboard.press("Control+v");
		const done = await waitFor(page, "window.__pushes.length >= 1", 15000, 30);
		await sleep(1500);
		const p = (await pushes())[0];
		const o1 = await ops();
		record("paste-text-with-accents-reaches-the-guest", done !== null && p?.result?.ok === true && o1.set - o0.set === 1, { result: p?.result, ms: p?.ms, sets: o1.set - o0.set });
		await shot("03-pasted-accents", true);
		const before = (await writes()).length;
		await page.keyboard.press("Control+a");
		await sleep(300);
		await page.keyboard.press("Control+c");
		await waitWrites(before + 1, 10000);
		const body = (await writes())[before]?.types?.["text/plain"]?.text;
		record("accents-round-trip-byte-for-byte", body === sample || body === sample.replace(/\r\n$/, ""), { got: body, expected: sample });

		// No duplicates: the browser clipboard now holds what the guest just copied, so pasting it again sends nothing; pasting the same
		// foreign text twice sends it once.
		const sets0 = (await ops()).set;
		await page.keyboard.press("Control+v");
		await sleep(2000);
		const sets1 = (await ops()).set;
		await setBrowserText("Another text\nfor the guest");
		await page.keyboard.press("Control+v");
		await sleep(2500);
		await page.keyboard.press("Control+v");
		await sleep(2500);
		const sets2 = (await ops()).set;
		record("no-duplicate-pastes", sets1 === sets0 && sets2 - sets1 === 1, { guestsOwnCopyPastedAgain: sets1 - sets0, sameForeignTextTwice: sets2 - sets1 });
	}

	// 3. Rich text: the paste event's `text/rtf` goes to the guest as it is (a synthetic paste event stands for LibreOffice's copy), and the guest's
	// RTF comes out as HTML next to the text (an offer made inside a real Enter key's gesture).
	if (wanted("rtf")) {
		const rtf = "{\\rtf1\\ansi{\\fonttbl{\\f0 Arial;}}\\f0\\fs28 from {\\b the page}\\par}";
		const o0 = await ops();
		if (browserName === "chromium") {
			await page.evaluate((rtfText: string) => {
				window.dispatchEvent(new KeyboardEvent("keydown", { key: "v", code: "KeyV", ctrlKey: true, bubbles: true, cancelable: true }));
				const data = new DataTransfer();
				data.setData("text/plain", "from the page");
				data.setData("text/rtf", rtfText);
				document.querySelector("[data-clip-box]")!.dispatchEvent(new ClipboardEvent("paste", { clipboardData: data, bubbles: true, cancelable: true }));
			}, rtf);
		} else {
			// Firefox empties the DataTransfer of a ClipboardEvent a script makes, so the paste handler cannot be driven that way here: the same
			// `push` the handler calls is called with what the paste event would hold (WP-HOST-1 saw Firefox offer text/rtf in a real paste).
			await page.evaluate((rtfText: string) => window.tryReactos.clipboard.push({ text: "from the page", rtf: rtfText }), rtf);
		}
		await waitFor(page, "window.__pushes.length >= 4", 15000, 30);
		const last = (await pushes()).at(-1);
		const o1 = await ops();
		record("paste-event-rtf-sent-as-is", last?.result?.ok === true && o1.part - o0.part === 2, { result: last?.result, parts: o1.part - o0.part, wireBytes: last?.wire });
		await sleep(500);
		const before = (await writes()).length;
		await runCommand("C:\\Quill\\HLCLIP.EXE rtf", true);
		await waitWrites(before + 1, 20000);
		const w = (await writes())[before];
		record("guest-rtf-written-as-html-and-text", w?.types?.["text/plain"]?.text === "plain bold italic" && w?.types?.["text/html"]?.text === "<p>plain <b>bold</b> <i>italic</i></p>", { types: w ? Object.fromEntries(Object.entries(w.types).map(([k, v]: any) => [k, v.text])) : null, note: "the browser cannot write text/rtf, so RTF goes out as text/html" });
		await sleep(1500);
	}

	// 4. Without a gesture nothing is written: the guest copies on its own, 15 s after the last key; the chip offers the button, which writes it.
	if (wanted("gesture")) {
		await sleep(5500);
		const before = (await writes()).length;
		const offers0 = (await status()).counters.offers;
		await runCommand("C:\\Quill\\HLCLIP.EXE rtf", false);
		const offered = await waitFor(page, `window.tryReactos.clipboard.status().counters.offers > ${offers0}`, 30000, 50);
		await sleep(2500);
		const st = await status();
		const unchanged = (await writes()).length === before;
		const chipNote = await chip();
		const disabled = await ev<boolean>("document.querySelector('[data-clip-copy]').disabled");
		record("no-gesture-means-no-write-and-a-button", offered !== null && unchanged && !disabled && /Copy from ReactOS/.test(chipNote ?? ""), { written: !unchanged, chip: chipNote, held: st.held, skipped: st.counters.skipped });
		await shot("04-no-gesture-button", true);
		await clickUi("[data-clip-copy]");
		await waitWrites(before + 1, 5000);
		const w = (await writes())[before];
		record("copy-from-reactos-button-writes-it", w?.types?.["text/plain"]?.text === "plain bold italic", { types: w ? Object.keys(w.types) : null });
	}

	// 5. Pictures: a small one inside the gesture (Alt+Print Screen of the Run box), a full-screen one by the button with a promised PNG.
	if (wanted("dib")) {
		await press([KEY.win, KEY.r]);
		await sleep(2500);
		let before = (await writes()).length;
		await page.keyboard.down("F12");
		await page.keyboard.up("F12");
		await press([KEY.alt, KEY.printScreen]);
		const t = await waitWrites(before + 1, 10000);
		const w = (await writes())[before];
		const png = w?.types?.["image/png"];
		record("small-picture-written-as-png-inside-the-gesture", !!png && png.width > 100 && png.height > 50 && png.nonBlackSamples > 10, { png, waitedMs: t });
		await press([KEY.esc]);
		await sleep(1000);
		before = (await writes()).length;
		const askedAt = await ev<number>("performance.now()");
		await press([KEY.printScreen]);
		await waitFor(page, "window.tryReactos.clipboard.status().held && window.tryReactos.clipboard.status().held.sizes.dib > 3000000", 15000, 50);
		await sleep(1000);
		const noteBefore = await chip();
		record("full-screen-picture-waits-for-the-button", (await writes()).length === before && /Copy from ReactOS/.test(noteBefore ?? ""), { chip: noteBefore, sizes: (await status()).held?.sizes });
		const clickedAt = await ev<number>("performance.now()");
		await clickUi("[data-clip-copy]");
		await sleep(2500);
		const mid = await status();
		const midChip = await chip();
		await shot("05-picture-copying", true);
		await waitWrites(before + 1, 30000);
		const full = (await writes())[before];
		const fp = full?.types?.["image/png"];
		record("full-screen-picture-written-as-a-promised-png", !!fp && fp.width === 1024 && fp.height === 768 && fp.nonBlackSamples > 50, { png: fp, seconds: full ? Number(((full.resolvedAt - clickedAt) / 1000).toFixed(1)) : null, progressMidWay: mid.progress, chipMidWay: midChip, sinceKeyMs: Math.round(clickedAt - askedAt) });
	}

	// 6. A picture from the browser into the guest: a PNG on the clipboard, then the Paste into ReactOS button (the browser's async read), then Paintbrush.
	if (wanted("image")) {
		const pngBytes = await page.evaluate(async () => {
			const canvas = document.createElement("canvas");
			canvas.width = 96;
			canvas.height = 64;
			const g = canvas.getContext("2d")!;
			const gradient = g.createLinearGradient(0, 0, 96, 64);
			gradient.addColorStop(0, "#e02020");
			gradient.addColorStop(0.5, "#20c020");
			gradient.addColorStop(1, "#2020e0");
			g.fillStyle = gradient;
			g.fillRect(0, 0, 96, 64);
			g.fillStyle = "#fff";
			g.fillRect(8, 8, 24, 24);
			const blob: Blob = await new Promise((r) => canvas.toBlob((b) => r(b!), "image/png"));
			await navigator.clipboard.write([new ClipboardItem({ "image/png": blob })]);
			return blob.size;
		});
		const readsBefore = await ev<number>("window.__clip.reads");
		const before = (await pushes()).length;
		await clickUi("[data-clip-paste]");
		await waitFor(page, `window.__pushes.length > ${before}`, 15000, 30);
		const p = (await pushes()).at(-1);
		const expected = 40 + 96 * 3 * 64;
		record("png-from-the-browser-sent-as-a-dib", p?.result?.ok === true && p.result.bytes === expected, { pngBytes, dibBytes: p?.result?.bytes, expected, ms: p?.ms, chip: await chip(), readsByTheButton: (await ev<number>("window.__clip.reads")) - readsBefore });
		// Paintbrush is not in the trimmed image ("The system cannot find the file specified"), so the picture is looked at in the Clipboard
		// Viewer, which shows the guest's own clipboard; where that is missing too, the DIB size above is the check.
		await runCommand("clipbrd");
		await sleep(7000);
		await shot("06-clipboard-viewer-shows-the-picture", true);
		await press([KEY.alt, [0x3e]]);
		await sleep(1500);
	}

	// 7. One mebibyte of text each way, with progress and the numbers.
	if (wanted("mib")) {
		await runCommand("notepad");
		await sleep(6000);
		const text = big(1048576);
		await setBrowserText(text);
		const before = (await pushes()).length;
		await page.keyboard.press("Control+v");
		await sleep(400);
		const chipMid = await chip();
		await shot("07-mib-sending", true);
		await waitFor(page, `window.__pushes.length > ${before}`, 60000, 50);
		const p = (await pushes()).at(-1);
		record("one-mebibyte-paste-ok-with-progress", p?.result?.ok === true && /Sending/.test(chipMid ?? ""), { ms: p?.ms, MBps: p?.ms ? Number((1 / (p.ms / 1000)).toFixed(2)) : null, chipDuringTheSend: chipMid, wire: p?.wire });
		await sleep(15000);
		// Notepad needs a while to take a mebibyte into its Edit control. The copy back normally arrives inside the 5 s window; when the guest is
		// slower than that the page leaves it for the button, and the check presses it (and says so).
		const writesBefore = (await writes()).length;
		const offers0 = (await status()).counters.offers;
		await page.keyboard.press("Control+a");
		await sleep(500);
		const keyAt = await ev<number>("performance.now()");
		await page.keyboard.press("Control+c");
		await waitFor(page, `window.tryReactos.clipboard.status().counters.offers > ${offers0}`, 90000, 100);
		const offerMs = Math.round((await ev<number>("performance.now()")) - keyAt);
		let t = await waitWrites(writesBefore + 1, 8000);
		let viaButton = false;
		if (t === null && (await status()).held) {
			viaButton = true;
			await clickUi("[data-clip-copy]");
			t = await waitWrites(writesBefore + 1, 60000);
		}
		const w = (await writes())[writesBefore];
		const got = w?.types?.["text/plain"]?.text ?? "";
		record("one-mebibyte-copy-back-equal", got.length === text.length && (await sha(got)) === (await sha(text)), { bytes: got.length, viaButton, offerAfterKeyMs: offerMs, writeStartedMs: w ? Math.round(w.at - keyAt) : null, dataCompleteAfterKeyMs: w ? Math.round(w.resolvedAt - keyAt) : null });
		await shot("08-mib-copied", true);
		const tooBig = big(1048576 + 16);
		await setBrowserText(tooBig);
		const sets0 = (await ops()).set;
		await page.keyboard.press("Control+v");
		await sleep(1500);
		const refusal = await chip();
		record("over-the-limit-is-refused-with-the-size", /too large/.test(refusal ?? "") && /limit is 1 MB/.test(refusal ?? "") && (await ops()).set === sets0, { chip: refusal });
		await shot("09-over-limit", true);
	}

	// 8. A restart in the middle of a transfer: the page's link starts again, the paste is cancelled, the guest keeps its clipboard, the next paste works.
	if (wanted("restart")) {
		await setBrowserText(big(1048576).replace(/0/g, "1"));
		const before = (await pushes()).length;
		await page.keyboard.press("Control+v");
		await waitFor(page, "window.tryReactos.hostLink.stats().packetsOut > 120", 30000, 20);
		await page.evaluate(() => window.tryReactos.hostLink.restart());
		await waitFor(page, `window.__pushes.length > ${before}`, 15000, 30);
		const cancelled = (await pushes()).at(-1);
		const ready = await waitFor(page, "window.tryReactos.hostLink.stats().state === 'ready'", 15000, 30);
		await sleep(500);
		await setBrowserText("after the restart: ok");
		const n = (await pushes()).length;
		await page.keyboard.press("Control+v");
		await waitFor(page, `window.__pushes.length > ${n}`, 15000, 30);
		const next = (await pushes()).at(-1);
		record("restart-mid-transfer-recovers", cancelled?.result?.ok === false && cancelled.result.reason === "cancelled" && ready !== null && next?.result?.ok === true, { cancelled: cancelled?.result, helloAnsweredMs: ready, next: next?.result, nextMs: next?.ms });
	}

	// 9. The toggle: off, Ctrl+V reaches the guest as keys (nothing sent), the helper is told to pause, the setting survives a reload.
	if (wanted("toggle")) {
		const o0 = await ops();
		await clickUi("[data-clip-toggle]");
		await sleep(500);
		await setBrowserText("must not be sent");
		await page.keyboard.press("Control+v");
		await sleep(1500);
		const o1 = await ops();
		const stored = await ev<string | null>("localStorage.getItem('quill.try-reactos.clipboard')");
		record("toggle-off-stops-everything", o1.set === o0.set && o1.pause - o0.pause === 1 && stored === "off" && /off/.test((await chip()) ?? ""), { setsSent: o1.set - o0.set, pauses: o1.pause - o0.pause, stored, chip: await chip(), row: await ev("document.querySelector('[data-clip-readout] dd').textContent") });
		await clickUi("[data-clip-toggle]");
		await sleep(500);
		record("toggle-on-again", (await ops()).resume === o1.resume + 1 && (await ev<string | null>("localStorage.getItem('quill.try-reactos.clipboard')")) === "on", { chip: await chip() });
	}

	// 10. Nothing left the browser and nothing was read without a gesture.
	const reads = await ev<{ reads: number; readTexts: number }>("window.__clip");
	const foreign = requests.filter((r) => r.method !== "GET" || r.body || !r.url.startsWith(new URL(page.url()).origin));
	record("no-network-request-carried-anything", foreign.length === 0, { requests: requests.length, notSameOriginGet: foreign.map((r) => r.method + " " + r.url), note: "the only requests are the guest disk's range reads and the page's own files" });
	// The clipboard reads: the Paste into ReactOS button's one read (image test) and the test's own readText calls; the page makes none else.
	const readTimes = await ev<number[]>("window.__clip.readTimes");
	const buttonReads = wanted("image") ? 1 : 0;
	record("clipboard-read-only-by-gestures", reads.reads === buttonReads && reads.readTexts === 0, { reads: reads.reads, readTexts: reads.readTexts, expectedFromTheButton: buttonReads, note: "navigator.clipboard.read and readText are wrapped and counted (the test's own reads are not): the page's only read is the Paste into ReactOS button's click; the paste event reads clipboardData, which is no call", readTimes: readTimes.map((t) => Math.round(t)) });

	// 11. The power cycle: Stop detaches, Start attaches again and the hello is answered at once, Reset likewise, and a shut down page keeps no link.
	if (wanted("power")) {
		const cycle = async (why: string, act: () => Promise<void>) => {
			await act();
			const ready = await waitFor(page, "window.tryReactos.hostLink && window.tryReactos.hostLink.stats().state === 'ready'", 90000, 20);
			const s = ready !== null ? await stats() : null;
			const row = await ev<string>("document.querySelector('[data-clip-readout] dd').textContent");
			record(`power-${why}-reattaches-and-the-hello-is-answered`, ready !== null && s.helloMs < 1000, { helloMs: s?.helloMs, readyMs: ready, row });
		};
		await clickUi("[data-stop]");
		const afterStop = await page.evaluate(() => ({ link: !!window.tryReactos.hostLink, clip: !!window.tryReactos.clipboard, barHidden: (document.querySelector("[data-clip-bar]") as HTMLElement).hidden, row: document.querySelector("[data-clip-readout] dd")!.textContent }));
		record("power-stop-detaches-and-hides-the-chip", !afterStop.link && !afterStop.clip && afterStop.barHidden, afterStop);
		await cycle("start", async () => void (await clickUi("[data-start]")));
		await sleep(2500);
		await cycle("reset", async () => void (await clickUi("[data-reset]")));
		await sleep(2500);
		await page.click("[data-stage]");
		await page.evaluate(() => window.tryReactos.emulator.stop());
		await waitFor(page, "window.tryReactos.powerState() === 'shutdown'", 10000, 100);
		const afterDown = await page.evaluate(() => ({ link: !!window.tryReactos.hostLink, barHidden: (document.querySelector("[data-clip-bar]") as HTMLElement).hidden }));
		record("power-shut-down-detaches-and-hides-the-chip", !afterDown.link && afterDown.barHidden, afterDown);
		await cycle("turn-on", async () => void (await clickUi("[data-start]")));
		await sleep(2500);
		await page.click("[data-stage]");
		await runCommand("notepad");
		await sleep(6000);
		await setBrowserText("works after the power cycle");
		await instrument();
		await page.keyboard.press("Control+v");
		await waitFor(page, "window.__pushes.length >= 1", 15000, 30);
		record("paste-works-after-the-power-cycle", (await pushes())[0]?.result?.ok === true, { result: (await pushes())[0]?.result });
		await shot("10-after-power-cycle", true);
	}

	// 12. An older state with no helper: the page works as before and says so.
	if (wanted("nohelper") && noHelperState) {
		await page.evaluate(() => window.tryReactos.stop());
		await page.goto(pageUrl("clip-nh-" + Date.now().toString(36), { disk: diskUrl, boot: "hd", ram: "256", state: noHelperState }));
		await clickUi("[data-start]");
		const t = await waitFor(page, "window.tryReactos.clipboard && /guest helper not found/.test(document.querySelector('[data-clip-chip]').textContent)", 60000, 250);
		await sleep(1000);
		const text = await chip();
		const row = await ev<string>("document.querySelector('[data-clip-readout] dd').textContent");
		await page.click("[data-stage]");
		await page.keyboard.press("Control+v");
		await sleep(1500);
		record("no-helper-says-so-and-the-page-works-as-before", t !== null && /guest helper not found/.test(row), { afterMs: t, chip: text, row, pasteButtonDisabled: await ev("document.querySelector('[data-clip-paste]').disabled"), desktop: await ev("window.__m.taskbarScore() > 0.3") });
		await shot("11-no-helper", true);
	}

	const final = results.every((r) => r.ok);
	void final;
} catch (error) {
	record("run", false, { error: String(error) });
} finally {
	fs.mkdirSync(shotDir, { recursive: true });
	fs.writeFileSync(path.join(shotDir, "results.json"), JSON.stringify(results, null, 2));
	await browser.close();
}
const failed = results.filter((r) => !r.ok);
console.log(`${browserName}: ${results.length - failed.length} of ${results.length} checks passed`);
if (failed.length > 0) process.exit(1);

declare global {
	interface Window {
		__m: any;
		tryReactos: any;
	}
}
