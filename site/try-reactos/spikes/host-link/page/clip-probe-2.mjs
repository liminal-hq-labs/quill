// WP-HOST-1 question 8, in headless Chromium and Playwright's Firefox, from a page served at http://127.0.0.1 (a secure context):
//   a. a promised ClipboardItem holding image/png that resolves 3 s after the keydown: is it written, and can it be read back?
//   b. what the paste event and the async read offer when the system clipboard holds native text/rtf. Headless browsers keep the clipboard in
//      the process, so for this one the browsers run headed on a private Xvfb (DISPLAY given by RTF_DISPLAY) where rtf-owner.py owns the X11
//      clipboard with text/rtf, application/rtf, text/html and text/plain, as LibreOffice or Word would.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// usage: RTF_DISPLAY=:97 node clip-probe-2.mjs [--png] [--rtf]
import http from "node:http";
import { chromium, firefox } from "playwright-core";

const PAGE = `<!doctype html><meta charset=utf-8><title>probe</title>
<textarea id=sink rows=3 cols=30></textarea><canvas id=c width=64 height=48></canvas>
<script>
window.log = [];
const L = (k, v) => window.log.push([k, v]);
const sink = document.getElementById("sink");
function pngBlob() {
	const c = document.getElementById("c"), g = c.getContext("2d");
	g.fillStyle = "#c33"; g.fillRect(0, 0, 64, 48); g.fillStyle = "#fff"; g.fillRect(8, 8, 24, 16);
	return new Promise((ok) => c.toBlob(ok, "image/png"));
}
sink.addEventListener("keydown", (e) => {
	if (e.ctrlKey && e.key === "c") {
		const t0 = performance.now();
		L("png keydown active", navigator.userActivation ? navigator.userActivation.isActive : "n/a");
		const promised = new Promise((resolve) => setTimeout(async () => resolve(await pngBlob()), 3000));
		navigator.clipboard.write([new ClipboardItem({ "image/png": promised })]).then(
			() => L("png write", "ok after " + Math.round(performance.now() - t0) + " ms"),
			(err) => L("png write", err.name + ": " + err.message));
	}
});
document.addEventListener("paste", (e) => {
	const dt = e.clipboardData;
	L("paste types", Array.from(dt.types).join(","));
	L("paste items", Array.from(dt.items).map((i) => i.kind + ":" + i.type).join(" "));
	L("paste files", Array.from(dt.files).map((f) => f.type + ":" + f.size).join(" "));
	for (const t of ["text/plain", "text/html", "text/rtf", "application/rtf", "text/richtext"]) L("paste getData " + t, JSON.stringify(dt.getData(t)).slice(0, 90));
	e.preventDefault();
});
window.readAll = async () => {
	try {
		const items = await navigator.clipboard.read(), out = [];
		for (const it of items) for (const t of it.types) { const b = await it.getType(t); out.push(t + "(" + b.size + ")"); }
		return out.join(" ");
	} catch (err) { return err.name + ": " + err.message; }
};
</script>`;

const server = http.createServer((_, r) => { r.writeHead(200, { "content-type": "text/html" }); r.end(PAGE); });
await new Promise((ok) => server.listen(0, "127.0.0.1", ok));
const url = "http://127.0.0.1:" + server.address().port + "/";
const want = (flag) => process.argv.includes(flag);
const results = [];

async function open(kind, headed) {
	const env = { ...process.env, DISPLAY: headed ? process.env.RTF_DISPLAY : (process.env.DISPLAY ?? ":99") };
	delete env.WAYLAND_DISPLAY;
	const opts = { headless: !headed, env };
	if (kind === "chromium") {
		opts.executablePath = "/usr/bin/chromium";
		if (headed) opts.args = ["--ozone-platform=x11", "--no-sandbox", "--disable-gpu", "--disable-dev-shm-usage"];
	}
	const browser = await (kind === "chromium" ? chromium : firefox).launch(opts);
	const context = await browser.newContext(kind === "chromium" ? { permissions: ["clipboard-read", "clipboard-write"] } : {});
	const page = await context.newPage();
	await page.goto(url);
	await page.click("#sink");
	return { browser, page };
}

if (want("--png")) {
	for (const kind of ["chromium", "firefox"]) {
		const { browser, page } = await open(kind, false);
		const out = { test: "promised image/png after 3 s", browser: kind, version: browser.version() };
		await page.keyboard.press("Control+c");
		await page.waitForTimeout(4500);
		out.log = await page.evaluate(() => window.log);
		out.readBack = await page.evaluate(() => window.readAll());
		await page.evaluate(() => { document.getElementById("sink").focus(); });
		await page.keyboard.press("Control+v");
		await page.waitForTimeout(500);
		out.pasteLog = (await page.evaluate(() => window.log)).filter(([k]) => k.startsWith("paste types") || k.startsWith("paste files"));
		await browser.close();
		results.push(out);
	}
}
if (want("--rtf")) {
	if (!process.env.RTF_DISPLAY) throw new Error("set RTF_DISPLAY to the private Xvfb display");
	for (const kind of (process.env.ONLY ?? "chromium,firefox").split(",")) {
		const { browser, page } = await open(kind, true);
		page.setDefaultTimeout(15000);
		const out = { test: "native text/rtf on the X11 clipboard", browser: kind, version: browser.version() };
		await page.click("#sink");
		await page.keyboard.press("Control+v");
		await page.waitForTimeout(800);
		out.log = await page.evaluate(() => window.log);
		await page.click("#sink");
		out.asyncRead = await Promise.race([page.evaluate(() => window.readAll()), new Promise((ok) => setTimeout(() => ok("no answer in 10 s (a permission prompt?)"), 10000))]);
		console.log(JSON.stringify(out, null, 1));
		await browser.close();
		results.push(out);
	}
}
server.close();
process.exit(0);
