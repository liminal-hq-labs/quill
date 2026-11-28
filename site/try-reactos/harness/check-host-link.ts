// The host link's end-to-end check (WP-HOST-3): restores the state host-link-boot.ts saved (the guest running HOSTLINK.EXE) in headless v86, and
// drives the product page's own link (`window.tryReactos.hostLink`, the `host-link` package) against the real helper, with Notepad as the
// application: a text copy arrives exactly (ASCII, accents, CRLF), the page's text pastes into Notepad, a 1 MiB text round trip, an RTF offer, a
// Print Screen picture, no second offer for the doubled viewer message, no echo of the helper's own write, and recovery when the page restarts
// in the middle of a transfer. Input goes only through the page's emulator API; pixels only from the emulator canvas.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// usage: BASE=http://127.0.0.1:18391 bun check-host-link.ts OUTDIR [--state /images/hl-state.bin] [--disk /images/hl-e2e.img] [--only a,b,c]
//   (the server: PORT=18391 IMAGES_DIR=~/.cache/quill-host-3 bun serve.ts; the disk and state: host-link-disk.sh, host-link-boot.ts)
import fs from "node:fs";
import path from "node:path";
import type { Page } from "playwright-core";
import { PROFILES, grab, launch, pageUrl, save, sleep, waitFor } from "./lib.ts";

const args = process.argv.slice(2);
const outDir = args[0];
if (!outDir) throw new Error("usage: check-host-link.ts OUTDIR [--state URL] [--disk URL] [--only a,b]");
const opt = (key: string, fallback: string) => (args.includes("--" + key) ? args[args.indexOf("--" + key) + 1] : fallback);
const only = opt("only", "").split(",").filter(Boolean);
const wanted = (name: string) => only.length === 0 || only.includes(name);

const KEY = { ctrl: [0x1d], a: [0x1e], c: [0x2e], v: [0x2f], enter: [0x1c], printScreen: [0xe0, 0x37], esc: [0x01], win: [0xe0, 0x5b], r: [0x13] };
type Codes = number[];

const run = `hl-e2e-${Date.now().toString(36)}`;
const session = await launch(run, PROFILES.unthrottled, false);
const page: Page = session.page;
const results: { name: string; ok: boolean; detail: Record<string, unknown> }[] = [];
const record = (name: string, ok: boolean, detail: Record<string, unknown> = {}) => {
	results.push({ name, ok, detail });
	console.log(JSON.stringify({ name, ok, ...detail }));
};

/** Keys down in order, then up in reverse order. */
const press = (codes: Codes[]) =>
	page.evaluate((keys: Codes[]) => {
		for (const code of keys) window.tryReactos.input.key(code);
		for (const code of [...keys].reverse()) window.tryReactos.input.key(code.map((value: number) => value | 0x80));
		return performance.now();
	}, codes);
const typeText = async (text: string, pause = 70) => {
	for (const ch of text) {
		if (ch === "\n") await press([KEY.enter]);
		else await page.evaluate((c: string) => window.tryReactos.input.text(c), ch);
		await sleep(pause);
	}
};
const runCommand = async (command: string) => {
	await press([KEY.win, KEY.r]);
	await sleep(2500);
	await typeText(command, 110);
	await sleep(400);
	await press([KEY.enter]);
};
const shot = async (name: string) => {
	const frame = await grab(page);
	if (frame) save(outDir, name + ".png", frame.png);
};

interface Seen {
	i: number;
	t: number;
	ch: string;
	op: string;
	f: Record<string, string>;
	len: number;
}
const seenSince = (from: number): Promise<Seen[]> =>
	page.evaluate((index: number) => window.__hl.msgs.slice(index).map((m: any, k: number) => ({ i: index + k, t: m.t, ch: m.channel, op: m.op, f: Object.fromEntries(m.fields), len: m.data.length })), from);
const count = (): Promise<number> => page.evaluate(() => window.__hl.msgs.length);
const waitFor_ = async (from: number, test: (m: Seen) => boolean, timeoutMs: number): Promise<Seen | null> => {
	const end = Date.now() + timeoutMs;
	while (Date.now() < end) {
		const found = (await seenSince(from)).find(test);
		if (found) return found;
		await sleep(150);
	}
	return null;
};
const stats = () => page.evaluate(() => window.tryReactos.hostLink.stats());

try {
	await page.goto(pageUrl(run, { disk: opt("disk", "/images/hl-e2e.img"), boot: "hd", ram: "256", state: opt("state", "/images/hl-state.bin") }));
	await page.evaluate(() => {
		// Everything the helper sends on a channel the page handles, with the time it arrived. Installed once the link exists.
		(window as any).__hl = { msgs: [] as any[] };
		const wait = setInterval(() => {
			const host = window.tryReactos.hostLink;
			if (!host || (window as any).__hl.hooked === host) return;
			(window as any).__hl.hooked = host;
			host.on("message", (m: any) => (window as any).__hl.msgs.push({ t: performance.now(), channel: m.channel, op: m.op, fields: m.fields, data: m.data }));
			void wait;
		}, 20);
		const utf8 = new TextEncoder();
		const hl = (window as any).__hl;
		hl.text = (s: string) => utf8.encode(s);
		hl.sha = async (bytes: Uint8Array) => Array.from(new Uint8Array(await crypto.subtle.digest("SHA-256", bytes))).map((b) => b.toString(16).padStart(2, "0")).join("");
		/** Sends a `set` and its parts the way the page will: the message, then one `part` per format. Returns the time it was queued. */
		hl.set = (seq: number, parts: Record<string, Uint8Array>) => {
			const host = window.tryReactos.hostLink;
			host.send("clip", "set", [["seq", seq], ["formats", Object.keys(parts).join(",")]]);
			for (const [fmt, bytes] of Object.entries(parts)) host.send("clip", "part", [["seq", seq], ["fmt", fmt]], bytes);
			return performance.now();
		};
		/** A deterministic text of exactly `n` bytes: numbered lines of 16 bytes with CRLF line ends. */
		hl.big = (n: number) => {
			const lines: string[] = [];
			for (let k = 0; k < n / 16; k++) lines.push(String(k).padStart(14, "0") + "\r\n");
			return utf8.encode(lines.join("")).slice(0, n);
		};
		hl.bytesOf = (index: number) => hl.msgs[index].data;
	});

	// 0. The restore: the helper is already running in the saved state and answers the page's hello at once.
	await page.click("[data-start]");
	const helloMs = await waitFor(page, "window.__hl && window.tryReactos.hostLink && window.tryReactos.hostLink.stats().state === 'ready'", 120000, 50);
	const s0 = await stats();
	record("restore-hello", helloMs !== null && s0.peer?.app?.startsWith("HOSTLINK/"), { helloMs, peer: s0.peer, window: s0.credit.window });
	if (helloMs === null) throw new Error("no helper");
	// The helper's own answer time: a hello now, on a link that is up, is answered by the helper at once.
	await page.evaluate(() => window.tryReactos.hostLink.restart());
	const answerMs = await waitFor(page, "window.tryReactos.hostLink.stats().state === 'ready' && window.tryReactos.hostLink.stats().peer !== null", 10000, 10);
	record("hello-answer-latency", answerMs !== null && answerMs < 1000, { answerMs });
	await sleep(1500);
	await shot("00-restored");

	// 1. Notepad: ASCII and CRLF typed in the guest, copied, received exactly, once.
	await runCommand("notepad");
	await sleep(7000);
	await shot("01-notepad-open");
	if (wanted("ascii")) {
		await typeText("Hello from Notepad\nSecond line 123 !?");
		await sleep(800);
		const from = await count();
		const keyed = await press([KEY.ctrl, KEY.a]);
		await sleep(300);
		const copiedAt = await press([KEY.ctrl, KEY.c]);
		void keyed;
		const data = await waitFor_(from, (m) => m.op === "data" && m.f.fmt === "text", 8000);
		await sleep(2500);
		const seen = await seenSince(from);
		const offers = seen.filter((m) => m.op === "offer");
		const texts = seen.filter((m) => m.op === "data" && m.f.fmt === "text");
		const body = data ? await page.evaluate((i: number) => new TextDecoder().decode(window.__hl.bytesOf(i)), data.i) : null;
		const expected = "Hello from Notepad\r\nSecond line 123 !?";
		record("ascii-crlf-copy-exact", body === expected, { got: body, offers: offers.length, textMessages: texts.length, offer: offers[0]?.f, latencyMs: data ? Math.round(data.t - copiedAt) : null });
		record("no-duplicate-send-for-the-doubled-message", offers.length === 1 && texts.length === 1, { offers: offers.length, textMessages: texts.length, messages: seen.map((m) => m.ch + "/" + m.op) });
		await shot("02-ascii-copied");
	}

	// 2. Page to guest: Latin-1 accents, CRLF and a euro sign set from the page, pasted in Notepad, copied back out, equal.
	if (wanted("latin1")) {
		const sample = "Zoë café — naïve façade, À la carte: 5 €\r\nSecond line, Ünïcödé ☺ ok\r\n";
		let from = await count();
		const queued = await page.evaluate((text: string) => (window as any).__hl.set(41, { text: (window as any).__hl.text(text) }), sample);
		const ok = await waitFor_(from, (m) => m.op === "ok" && m.f.seq === "41", 8000);
		record("set-text-ok", ok !== null, { latencyMs: ok ? Math.round(ok.t - queued) : null });
		await sleep(1500);
		const echo = (await seenSince(from)).filter((m) => m.op === "offer");
		record("no-echo-of-the-helpers-own-write", echo.length === 0, { offers: echo.length });
		await press([KEY.ctrl, KEY.a]);
		await sleep(300);
		const pastedAt = await press([KEY.ctrl, KEY.v]);
		void pastedAt;
		await sleep(3000);
		await shot("03-latin1-pasted");
		from = await count();
		await press([KEY.ctrl, KEY.a]);
		await sleep(300);
		await press([KEY.ctrl, KEY.c]);
		const data = await waitFor_(from, (m) => m.op === "data" && m.f.fmt === "text", 8000);
		const body = data ? await page.evaluate((i: number) => new TextDecoder().decode(window.__hl.bytesOf(i)), data.i) : null;
		// Notepad's Edit control may end the text without the last CRLF; the accents and the CRLF in the middle are the point.
		record("latin1-round-trip-exact", body === sample || body === sample.replace(/\r\n$/, ""), { got: body, expected: sample });
	}

	// 3. An RTF offer from a program that copies rich text, and a Print Screen picture.
	if (wanted("rtf")) {
		const from = await count();
		await runCommand("C:\\Quill\\HLCLIP.EXE rtf");
		const offer = await waitFor_(from, (m) => m.op === "offer", 20000);
		const rtf = await waitFor_(from, (m) => m.op === "data" && m.f.fmt === "rtf", 8000);
		const text = await waitFor_(from, (m) => m.op === "data" && m.f.fmt === "text", 3000);
		const rtfBody = rtf ? await page.evaluate((i: number) => new TextDecoder().decode(window.__hl.bytesOf(i)), rtf.i) : null;
		const textBody = text ? await page.evaluate((i: number) => new TextDecoder().decode(window.__hl.bytesOf(i)), text.i) : null;
		const wantRtf = "{\\rtf1\\ansi{\\fonttbl{\\f0 Arial;}}\\f0\\fs28 plain {\\b bold} {\\i italic}\\par}";
		record("rtf-offer-and-data", rtfBody === wantRtf && textBody === "plain bold italic", { offer: offer?.f, rtf: rtfBody, text: textBody });
	}
	if (wanted("dib")) {
		const from = await count();
		const pressedAt = await press([KEY.printScreen]);
		const offer = await waitFor_(from, (m) => m.op === "offer" && m.f.dib !== undefined, 15000);
		const autoData = (await seenSince(from)).filter((m) => m.op === "data");
		record("dib-offer-for-print-screen", offer !== null && Number(offer.f.dib) > 3_000_000 && autoData.length === 0, { offer: offer?.f, offerMs: offer ? Math.round(offer.t - pressedAt) : null, unsolicitedData: autoData.length });
		if (offer) {
			const askedFrom = await count();
			const askedAt = await page.evaluate((seq: string) => { window.tryReactos.hostLink.send("clip", "get", [["seq", seq], ["fmt", "dib"]]); return performance.now(); }, offer.f.seq);
			const data = await waitFor_(askedFrom, (m) => m.op === "data" && m.f.fmt === "dib", 60000);
			const header = data ? await page.evaluate((i: number) => { const d = window.__hl.bytesOf(i); const v = new DataView(d.buffer, d.byteOffset); return { size: v.getUint32(0, true), w: v.getInt32(4, true), h: v.getInt32(8, true), bpp: v.getUint16(14, true), bytes: d.length }; }, data.i) : null;
			const seconds = data ? (data.t - askedAt) / 1000 : null;
			record("dib-get-header-and-rate", header !== null && header.size === 40 && header.w === 1024 && Math.abs(header.h) === 768 && header.bpp === 32 && header.bytes === Number(offer.f.dib), { header, seconds, MBps: header && seconds ? Number((header.bytes / 1048576 / seconds).toFixed(2)) : null });
		}
	}

	// 4. One mebibyte of text, page to guest and back (through Notepad's Edit control).
	if (wanted("mib")) {
		await press([KEY.ctrl, KEY.a]);
		let from = await count();
		const queued = await page.evaluate(() => (window as any).__hl.set(77, { text: (window as any).__hl.big(1048576) }));
		const ok = await waitFor_(from, (m) => m.op === "ok" && m.f.seq === "77", 60000);
		const pushMs = ok ? Math.round(ok.t - queued) : null;
		const after = await stats();
		record("one-mebibyte-set-ok", ok !== null, { pushMs, MBps: pushMs ? Number((1 / (pushMs / 1000)).toFixed(2)) : null, errors: after.errors, peerErrors: after.peerErrors, state: after.state });
		await shot("04-mib-set");
		if (ok) {
			await press([KEY.ctrl, KEY.v]);
			await sleep(20000);
			await shot("05-mib-pasted");
			from = await count();
			await press([KEY.ctrl, KEY.a]);
			await sleep(1000);
			await press([KEY.ctrl, KEY.c]);
			const offer = await waitFor_(from, (m) => m.op === "offer", 60000);
			record("one-mebibyte-offer-after-notepad-copy", offer !== null && Number(offer.f.text) === 1048576, { offer: offer?.f });
			if (offer) {
				const askedFrom = await count();
				const askedAt = await page.evaluate((seq: string) => { window.tryReactos.hostLink.send("clip", "get", [["seq", seq], ["fmt", "text"]]); return performance.now(); }, offer.f.seq);
				const data = await waitFor_(askedFrom, (m) => m.op === "data" && m.f.fmt === "text", 90000);
				const verdict = data
					? await page.evaluate(async (i: number) => { const got = window.__hl.bytesOf(i); const want = window.__hl.big(1048576); return { equal: got.length === want.length && (await window.__hl.sha(got)) === (await window.__hl.sha(want)), bytes: got.length }; }, data.i)
					: null;
				const seconds = data ? (data.t - askedAt) / 1000 : null;
				record("one-mebibyte-round-trip-equal", verdict?.equal === true, { verdict, seconds, MBps: seconds ? Number((1 / seconds).toFixed(2)) : null });
			}
		}
	}

	// 5. The page restarts in the middle of a transfer.
	if (wanted("restart")) {
		await press([KEY.ctrl, KEY.a]);
		await press([KEY.esc]);
		let from = await count();
		await page.evaluate(() => (window as any).__hl.set(90, { text: (window as any).__hl.big(1048576) }));
		// About 300 KB in, the page lets go: a new hello, as after a reload.
		await waitFor(page, "window.tryReactos.hostLink.stats().packetsOut > 75", 30000, 20);
		const before = await stats();
		await page.evaluate(() => window.tryReactos.hostLink.restart());
		const readyAt = Date.now();
		const ready = await waitFor(page, "window.tryReactos.hostLink.stats().state === 'ready' && window.tryReactos.hostLink.stats().peer", 15000, 50);
		const nothing = (await seenSince(from)).filter((m) => m.op === "ok");
		record("restart-mid-transfer-hello-answered", ready !== null && nothing.length === 0, { packetsOutBefore: before.packetsOut, readyMs: ready, spuriousOk: nothing.length });
		from = await count();
		const queued = await page.evaluate(() => (window as any).__hl.set(91, { text: (window as any).__hl.text("after the restart: ok\r\n") }));
		const ok = await waitFor_(from, (m) => m.op === "ok" && m.f.seq === "91", 10000);
		record("restart-mid-transfer-next-set-ok", ok !== null, { latencyMs: ok ? Math.round(ok.t - queued) : null, sinceRestartMs: Date.now() - readyAt });
		await press([KEY.ctrl, KEY.a]);
		await sleep(300);
		await press([KEY.ctrl, KEY.v]);
		await sleep(2500);
		await shot("06-after-restart");

		// And a torn frame: the page goes away with half a packet written, then comes back with a hello.
		from = await count();
		await page.evaluate(() => {
			const bytes = new Uint8Array(2000).fill(0x7a);
			const head = new TextEncoder().encode("LHQ-PKT 1 clip 9999 E 4000 00000000\r\n");
			const torn = new Uint8Array(head.length + bytes.length);
			torn.set(head);
			torn.set(bytes, head.length);
			window.tryReactos.emulator.serial_send_bytes(0, torn);
		});
		await sleep(300);
		const tornAt = Date.now();
		await page.evaluate(() => window.tryReactos.hostLink.restart());
		const recovered = await waitFor(page, "window.tryReactos.hostLink.stats().peer !== null && window.tryReactos.hostLink.stats().state === 'ready'", 20000, 100);
		await page.evaluate(() => { window.tryReactos.hostLink.stats(); });
		from = await count();
		const queued2 = await page.evaluate(() => (window as any).__hl.set(92, { text: (window as any).__hl.text("after the torn frame: ok") }));
		const ok2 = await waitFor_(from, (m) => m.op === "ok" && m.f.seq === "92", 12000);
		record("torn-frame-then-hello-recovers", ok2 !== null, { helloAnsweredMs: recovered, totalMs: Date.now() - tornAt, setLatencyMs: ok2 ? Math.round(ok2.t - queued2) : null });
	}

	const final = await stats();
	record("link-errors", Object.keys(final.errors).length === 0, { errors: final.errors, peerErrors: final.peerErrors, bytesIn: final.bytesIn, bytesOut: final.bytesOut, messagesIn: final.messagesIn, messagesOut: final.messagesOut });
} catch (error) {
	record("run", false, { error: String(error) });
} finally {
	fs.mkdirSync(outDir, { recursive: true });
	fs.writeFileSync(path.join(outDir, "results.json"), JSON.stringify(results, null, 2));
	await session.close();
}
const failed = results.filter((r) => !r.ok);
console.log(`${results.length - failed.length} of ${results.length} checks passed`);
if (failed.length > 0) process.exit(1);

declare global {
	interface Window {
		__hl: any;
		__m: any;
		tryReactos: any;
	}
}
