// Checks the kit's sound card telemetry (SITE-122, SITE-123) in headless Chromium or Firefox, on the saved desktop and on a cold boot: the Audio row in
// Diagnostics appears only while the panel is open and says "playing" within a second of Critical Stop starting, the kit runs no timer while the panel is
// closed, the "Muted in the PC" badge follows the codec's mute bit, and nothing leaves the origin. Needs staged data/ and web-manifest.json (tools/stage-web.py)
// and the server (bun serve.ts). The saved desktop's codec is unmuted, so its badge is tested by writing the mute bit through the model; on the cold boot the
// power-on state is read as the card registers, and the same write is used once the driver has unmuted it.
// Usage: BASE=http://127.0.0.1:8160 [BROWSER=firefox] [PARTS=saved|cold|both] bun check-kit-audio.ts OUTDIR     (unset WAYLAND_DISPLAY)
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { firefox } from "playwright-core";
import { BASE, PROFILES, launch as launchChromium, pageUrl, save, sleep, waitFor } from "./lib.ts";
import { card, openSounds, playSound } from "./sound-lib.ts";

const outDir = process.argv[2];
if (!outDir) throw new Error("usage: check-kit-audio.ts OUTDIR");
const parts = process.env.PARTS ?? "both";
const failures: string[] = [];
const expect = (ok: boolean, message: string) => {
	if (!ok) {
		failures.push(message);
		console.error("FAIL " + message);
	} else console.log("ok   " + message);
};
const report: Record<string, unknown> = {};

async function launch() {
	if (process.env.BROWSER !== "firefox") return launchChromium("", PROFILES.unthrottled!, false);
	const browser = await firefox.launch({ headless: true });
	const context = await browser.newContext({ viewport: { width: 1280, height: 1000 }, serviceWorkers: "block" });
	await context.addInitScript({ path: new URL("./inpage.js", import.meta.url).pathname });
	const page = await context.newPage();
	return { page, close: () => browser.close() };
}

// Counts the page's intervals: which are live, how often they fire, and whether kit.js made them (its file is in the stack that called setInterval).
const COUNT_TIMERS = () => {
	const live = new Map<unknown, { ms: number; kit: boolean }>();
	const w = window as any;
	w.__timers = live;
	const set = window.setInterval.bind(window);
	const clear = window.clearInterval.bind(window);
	window.setInterval = ((fn: any, ms: number, ...rest: any[]) => {
		const id = set(fn, ms, ...rest);
		live.set(id, { ms, kit: /kit\.js/.test(new Error().stack ?? "") });
		return id;
	}) as typeof window.setInterval;
	window.clearInterval = ((id: any) => {
		live.delete(id);
		return clear(id);
	}) as typeof window.clearInterval;
};
const kitTimers = (page: any): Promise<{ ms: number }[]> => page.evaluate(() => [...((window as any).__timers as Map<unknown, { ms: number; kit: boolean }>).values()].filter((t) => t.kit));
const row = (page: any): Promise<string> => page.evaluate(() => document.querySelector("[data-audio-readout] dd")?.textContent ?? "");
const badge = (page: any): Promise<{ hidden: boolean; text: string; title: string }> =>
	page.evaluate(() => {
		const b = document.querySelector("[data-sound-badge]") as HTMLElement;
		return { hidden: b.hidden || getComputedStyle(b).display === "none", text: b.textContent ?? "", title: b.title };
	});
const setPanel = (page: any, open: boolean) => page.evaluate((o: boolean) => ((document.querySelector("[data-diag]") as HTMLDetailsElement).open = o), open);
const desktop = "window.__m.taskbarScore() > 0.3";
const codecNow = (page: any) => page.evaluate(() => (window as any).tryReactos.soundHandle.codec());
// The test's stand-in for a guest driver writing the master volume: through the model, as a guest's I/O write would.
const writeMaster = (page: any, word: number) => page.evaluate((w: number) => (window as any).tryReactos.soundHandle.model.namWrite(0x02, 2, w), word);

/** Plays Critical Stop once with the sounds tab already open; resolves with how long the row took to say "playing" (null if it never did) and the badge while it played. */
async function playAndWatch(page: any, why: string, shot?: string) {
	const before = (await card(page))!;
	await playSound(page);
	const started = await waitFor(page, `window.tryReactos.soundHandle.stats().counters.dmaFrames > ${before.dma}`, 30000, 50);
	expect(started !== null, `${why}: the guest started playing`);
	const sawPlaying = await waitFor(page, `document.querySelector('[data-audio-readout] dd').textContent.includes(', playing. ')`, 3000, 50);
	const latency = sawPlaying;
	const during = { row: await row(page), badge: await badge(page) };
	if (shot) await save(outDir, shot, await page.screenshot());
	const ended = await waitFor(page, `!window.tryReactos.soundHandle.stats().playing && window.tryReactos.soundHandle.stats().counters.dmaFrames > ${before.dma}`, 30000, 100);
	expect(ended !== null, `${why}: the guest finished playing`);
	await sleep(600);
	return { latency, sawPlaying, during, after: { row: await row(page), badge: await badge(page), card: (await card(page))! } };
}

async function openPage(url: string) {
	const session = await launch();
	const page = session.page;
	const requests: string[] = [];
	page.on("request", (r) => requests.push(r.url()));
	page.on("pageerror", (e) => failures.push("page error: " + String(e)));
	await page.addInitScript(COUNT_TIMERS);
	await page.goto(url);
	await page.waitForFunction("document.querySelector('[data-build]').textContent !== 'Build: loading'", null, { timeout: 15000 });
	return { session, page, requests };
}

const sameOrigin = (requests: string[]) => requests.filter((u) => !u.startsWith(BASE) && !u.startsWith("blob:") && !u.startsWith("data:"));

// ---------------------------------------------------------------------------------------------------------------------------------------------
// The saved desktop.
if (parts !== "cold") {
	const { session, page, requests } = await openPage(pageUrl("", {}, { plain: true }));
	try {
		// Before Start: the row and badge exist and say nothing, and the kit has no timer.
		expect((await row(page)) === "off", `saved desktop, before Start: the Audio row reads "${await row(page)}"`);
		expect((await badge(page)).hidden, "saved desktop, before Start: the badge is hidden");
		expect((await kitTimers(page)).length === 0, "saved desktop, before Start: the kit has no timer");

		await page.click("[data-start]");
		expect((await waitFor(page, desktop, 240000)) !== null, "saved desktop: the themed desktop came up");
		await sleep(2500);
		const c = await codecNow(page);
		report.savedCodec = c;
		expect(c !== null && !c.master.muted && !c.pcm.muted, `saved desktop: the guest's volume is master ${c?.master.left} percent and wave ${c?.pcm.left} percent, unmuted`);

		// Panel closed: no kit timer, the row says nothing about the card, and no work was asked of the card for it.
		expect((await kitTimers(page)).length === 0, "panel closed: the kit has no timer running");
		expect(!/Hz/.test(await row(page)), `panel closed: the Audio row reads "${await row(page)}"`);
		expect((await badge(page)).hidden, "panel closed, idle: the badge is hidden");
		report.nonKitTimersWhileClosed = await page.evaluate(() => [...((window as any).__timers as Map<unknown, { ms: number; kit: boolean }>).values()].filter((t) => !t.kit).map((t) => t.ms));

		// Open the panel: one kit timer at twice a second, and the row fills in.
		await page.click("[data-diag] > summary");
		expect((await waitFor(page, `/Hz/.test(document.querySelector('[data-audio-readout] dd').textContent)`, 3000, 50)) !== null, "panel open: the Audio row fills in within 3 s");
		const timers = await kitTimers(page);
		expect(timers.length === 1 && timers[0]!.ms === 500, `panel open: the kit has ${timers.length} timer at ${timers[0]?.ms} ms`);
		const idleRow = await row(page);
		report.idleRow = idleRow;
		expect(/^\d[\d,]* Hz, 16-bit stereo, idle\. \d+ buffers?, [\d,]+ frames?, \d+ starved\. Output: \d+ underruns? \([\d,]+ frames?\), [\d,]+ frames? dropped, [\d,]+ silent frames?\. PC volume: master \d+ percent, wave \d+ percent, record \d+ percent/.test(idleRow), `panel open: the row reads "${idleRow}"`);
		await save(outDir, "kit-saved-panel-idle.png", await page.locator("[data-diag]").screenshot());

		// Critical Stop with the panel open: the row says so within 1 s of the card starting, then holds the totals.
		await openSounds(page);
		const first = await playAndWatch(page, "saved desktop, Critical Stop", "kit-saved-panel-playing.png");
		const lag = first.latency ?? 99999;
		report.firstPlay = { rowLagMs: first.latency, during: first.during.row, after: first.after.row };
		expect(first.sawPlaying !== null && lag <= 1000, `panel open: the row said "playing" ${lag} ms after the card started`);
		expect(first.during.badge.hidden, "unmuted PC: the badge stays hidden while the card plays");
		const frames = first.after.card.dma;
		expect(first.after.row.includes(frames.toLocaleString("en-CA") + " frames") && first.after.row.includes(", idle. "), `after the sound: the row holds ${frames.toLocaleString("en-CA")} frames and reads idle ("${first.after.row}")`);
		expect(frames > 30000 && frames < 50000, `Critical Stop: ${frames} frames (WP-WEB-4 saw 39,045)`);
		expect(first.after.card.buffers > 100 && first.after.row.includes(first.after.card.buffers + " buffers"), `the row's buffer count is the card's (${first.after.card.buffers})`);

		// The page's own output counters are in the row, and the row follows them.
		const sink: any = await page.evaluate("window.tryReactos.soundHandle.sinkStats()");
		report.sink = sink;
		expect(first.after.row.includes(`Output: ${sink.underrunEvents} underrun`) && first.after.row.includes(`${sink.dropped} frame${sink.dropped === 1 ? "" : "s"} dropped`), `the output's counters are in the row (${sink.underrunEvents} underrun events, ${sink.dropped} dropped)`);

		// Muted in the PC: the codec's mute bit, written through the model as a guest's driver would, while the card plays.
		await writeMaster(page, 0x8000);
		await sleep(200);
		let after = await codecNow(page);
		expect(after.master.muted, "the test muted the master volume through the codec's register");
		expect(await waitFor(page, `/master 100 percent[^.]*; master muted\\./.test(document.querySelector('[data-audio-readout] dd').textContent)`, 3000, 100) !== null, `panel open: the row says the master is muted ("${await row(page)}")`);
		const muted = await playAndWatch(page, "muted master, panel open", "kit-saved-panel-muted.png");
		expect(!muted.during.badge.hidden && muted.during.badge.text === "Muted in the PC", `muted master while the card plays: the badge reads "${muted.during.badge.text}"`);
		expect(/Volume Control/.test(muted.during.badge.title), "the badge's title names the guest's Volume Control");
		await save(outDir, "kit-saved-badge.png", await page.locator("[data-player]").screenshot());
		await sleep(2300);
		expect((await badge(page)).hidden, "2 s after the sound ended: the badge is hidden again");

		// The same with the panel closed: no kit timer, the badge still shows (it follows onCodec and the page's own updates).
		await setPanel(page, false);
		await sleep(300);
		expect((await kitTimers(page)).length === 0, "panel closed again: the kit's timer is gone");
		expect(!/Hz/.test(await row(page)), `panel closed again: the row reads "${await row(page)}"`);
		const closedRow = await row(page);
		const closed = await playAndWatch(page, "muted master, panel closed", "kit-saved-badge-panel-closed.png");
		expect(closed.sawPlaying === null && (await row(page)) === closedRow, "panel closed while the card plays: the row stayed as it was");
		expect(!closed.during.badge.hidden, "muted master, panel closed: the badge shows while the card plays");
		expect((await kitTimers(page)).length === 0, "panel closed while the card plays: still no kit timer");

		// The guest unmutes: the badge goes, from the codec's write alone.
		await playSound(page);
		await waitFor(page, "window.tryReactos.soundHandle.stats().playing", 10000, 50);
		await sleep(400);
		expect(!(await badge(page)).hidden, "playing again: the badge is up");
		await writeMaster(page, 0x0000);
		expect((await waitFor(page, `document.querySelector('[data-sound-badge]').hidden`, 2000, 50)) !== null, "the guest unmutes the master: the badge goes within 2 s");
		await waitFor(page, "!window.tryReactos.soundHandle.stats().playing", 30000, 100);

		// The wave volume alone counts too.
		await page.evaluate(() => (window as any).tryReactos.soundHandle.model.namWrite(0x18, 2, 0x8808));
		const wave = await playAndWatch(page, "muted wave volume", undefined);
		expect(!wave.during.badge.hidden, "muted wave (PCM out) volume while the card plays: the badge shows");
		await page.evaluate(() => (window as any).tryReactos.soundHandle.model.namWrite(0x18, 2, 0x0808));

		const off = sameOrigin(requests);
		expect(off.length === 0, `no request left the origin (${requests.length} requests; off-origin: ${off.join(", ") || "none"})`);
	} catch (error) {
		failures.push("saved desktop: " + String(error));
		console.error(error);
		await save(outDir, "kit-saved-failure.png", await page.screenshot()).catch(() => {});
	} finally {
		await session.close();
	}
}

// ---------------------------------------------------------------------------------------------------------------------------------------------
// A cold boot: the machine boots the disk from its first byte and the page adds the card, so the codec starts in its power-on state (master and PCM out muted).
if (parts !== "saved") {
	const { session, page, requests } = await openPage(`${BASE}/try-reactos/?state=`);
	try {
		await page.evaluate(() => {
			const w = window as any;
			w.__codecLog = [];
			const t0 = performance.now();
			const poll = () => {
				const h = w.tryReactos?.soundHandle;
				const codec = h && h.codec();
				if (codec) w.__codecLog.push({ t: Math.round(performance.now() - t0), master: codec.master.raw, pcm: codec.pcm.raw, playing: h.stats().playing });
			};
			setInterval(poll, 200);
		});
		// The panel is open before Start, so the row shows the codec as the card powers on.
		await page.click("[data-diag] > summary");
		await page.click("[data-start]");
		await page.waitForFunction("window.tryReactos.soundHandle && window.tryReactos.soundHandle.stats().registered", null, { timeout: 60000 });
		const first = await codecNow(page);
		report.coldFirstCodec = first;
		expect(first.master.muted && first.pcm.muted, `cold boot: the codec starts with master (0x${first.master.raw.toString(16)}) and wave (0x${first.pcm.raw.toString(16)}) muted`);
		expect((await badge(page)).hidden, "cold boot, nothing playing: the badge is hidden");
		expect((await waitFor(page, `/master, wave and record muted\./.test(document.querySelector('[data-audio-readout] dd').textContent)`, 3000, 100)) !== null, `cold boot: the Audio row says what is muted ("${await row(page)}")`);
		report.coldRowPowerOn = await row(page);
		await save(outDir, "kit-cold-panel-power-on.png", await page.locator("[data-diag]").screenshot());
		expect((await waitFor(page, desktop, 600000, 1000)) !== null, "cold boot: the themed desktop came up");
		const atDesktop = await codecNow(page);
		report.coldMutedAtDesktop = atDesktop.master.muted || atDesktop.pcm.muted;
		console.log(`INFO at the desktop the codec was ${report.coldMutedAtDesktop ? "still muted" : "unmuted"} (master 0x${atDesktop.master.raw.toString(16)}, wave 0x${atDesktop.pcm.raw.toString(16)})`);
		// The driver installs itself on the first boot, after the desktop is up: wait for it to write the volumes, then for the machine to settle.
		const driverAfter = await waitFor(page, "(() => { const c = window.tryReactos.soundHandle.codec(); return !c.master.muted && !c.pcm.muted; })()", 300000, 500);
		report.coldDriverWroteVolumesAfterDesktopMs = driverAfter;
		expect(driverAfter !== null, `cold boot: the guest's driver unmuted the codec ${driverAfter} ms after the desktop came up`);
		await sleep(30000);
		const log: { t: number; master: number; pcm: number; playing: boolean }[] = await page.evaluate("window.__codecLog");
		const unmutedAt = log.find((e) => !(e.master & 0x8000) && !(e.pcm & 0x8000));
		const lastMasterWrite = log.filter((e, i) => i === 0 || e.master !== log[i - 1]!.master || e.pcm !== log[i - 1]!.pcm);
		report.coldCodecChanges = lastMasterWrite;
		report.coldDriverUnmuted = unmutedAt ? { afterMs: unmutedAt.t, master: unmutedAt.master, pcm: unmutedAt.pcm } : false;
		const now = await codecNow(page);
		report.coldCodecAtDesktop = now;
		console.log(unmutedAt ? `INFO the guest's driver unmuted the codec ${unmutedAt.t} ms after Start (master 0x${unmutedAt.master.toString(16)}, wave 0x${unmutedAt.pcm.toString(16)}), before any sound` : "INFO the codec was still muted at the desktop: the driver had not unmuted it");

		expect((await waitFor(page, `/Hz/.test(document.querySelector('[data-audio-readout] dd').textContent)`, 3000, 50)) !== null, "cold boot, panel open: the Audio row fills in");
		await save(outDir, "kit-cold-panel.png", await page.locator("[data-diag]").screenshot());
		report.coldRow = await row(page);
		await openSounds(page);
		if (now.master.muted || now.pcm.muted) {
			const muted = await playAndWatch(page, "cold boot, codec muted", "kit-cold-badge.png");
			expect(!muted.during.badge.hidden, "cold boot, the codec muted when the card plays: the badge shows");
		} else {
			const plain = await playAndWatch(page, "cold boot, codec unmuted by the driver");
			expect(plain.during.badge.hidden, "cold boot, the driver's volume unmuted: the badge stays hidden while the card plays");
			await writeMaster(page, 0x8000);
			await sleep(200);
			const muted = await playAndWatch(page, "cold boot, master muted by the test", "kit-cold-badge.png");
			expect(!muted.during.badge.hidden, "cold boot, master muted through the model: the badge shows while the card plays");
			await writeMaster(page, now.master.raw);
		}
		await save(outDir, "kit-cold-player.png", await page.locator("[data-player]").screenshot());
		const off = sameOrigin(requests);
		expect(off.length === 0, `cold boot: no request left the origin (off-origin: ${off.join(", ") || "none"})`);
	} catch (error) {
		failures.push("cold boot: " + String(error));
		console.error(error);
		await save(outDir, "kit-cold-failure.png", await page.screenshot()).catch(() => {});
	} finally {
		await session.close();
	}
}

save(outDir, `report-${process.env.BROWSER ?? "chromium"}.json`, JSON.stringify(report, null, 2));
console.log(JSON.stringify(report, null, 2));
if (failures.length) {
	console.error(`\n${failures.length} failure(s)`);
	process.exit(1);
}
console.log("\nall checks passed");
