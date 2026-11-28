// The page's Host Link Kit glue (Level 4, no image build): the staged ReactOS desktop in headless Chromium or Firefox, a CD made from the vendored guest
// programs, D:\HOSTLINK.EXE from the Run box, then the PC sound line, the badge button, Keep in step, the test tone, the System row asked only while Diagnostics
// is open, a switch sending a new hello, the Host Link Kit CD, Shut down the PC ending in the ACPI write, and the origin of every request. COM1's pending
// interrupt in the staged state is cleared by vendor-lib.ts, in the harness only. Needs the page with the sound card (the staged state has one).
// Usage: BASE=http://127.0.0.1:8171 IMAGES_DIR=DIR [BROWSER=firefox] bun check-kit-link.ts OUTDIR     (unset WAYLAND_DISPLAY)
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import os from "node:os";
import path from "node:path";
import { BASE, guestTools, insertCd, makeTestCd, offOrigin, openBrowser, pageUrl, save, sleep, startStagedDesktop, waitFor, grab } from "./vendor-lib.ts";

const outDir = process.argv[2];
if (!outDir) throw new Error("usage: check-kit-link.ts OUTDIR");
const browserName = process.env.BROWSER === "firefox" ? "firefox" : "chromium";
const imagesDir = process.env.IMAGES_DIR || path.join(os.homedir(), ".cache/quill-vend-images");
const failures: string[] = [];
const report: Record<string, unknown> = {};
const expect = (ok: boolean, message: string) => {
	if (!ok) failures.push(message);
	console.log((ok ? "ok   " : "FAIL ") + message);
};

const cdUrl = makeTestCd(imagesDir);
const { browser, page } = await openBrowser(browserName);
const requests: { url: string; at: number }[] = [];
page.on("request", (r) => requests.push({ url: r.url(), at: Date.now() }));
page.on("pageerror", (e) => failures.push("page error: " + String(e)));
// Intervals made by kit-link.js, and what the page sends over COM1 and to the helper, counted from the start.
await page.addInitScript(() => {
	const w = window as any;
	const live = new Map<unknown, { ms: number; kit: boolean }>();
	w.__timers = live;
	const set = window.setInterval.bind(window);
	const clear = window.clearInterval.bind(window);
	window.setInterval = ((fn: any, ms: number, ...rest: any[]) => {
		const id = set(fn, ms, ...rest);
		live.set(id, { ms, kit: /kit-link\.js/.test(new Error().stack ?? "") });
		return id;
	}) as typeof window.setInterval;
	window.clearInterval = ((id: any) => (live.delete(id), clear(id))) as typeof window.clearInterval;
});
const { press, run } = guestTools(page);
const ev = <T>(expression: string): Promise<T> => page.evaluate(expression) as Promise<T>;
const kitTimers = () => page.evaluate(() => [...((window as any).__timers as Map<unknown, { ms: number; kit: boolean }>).values()].filter((t) => t.kit).map((t) => t.ms));
const setPanel = (open: boolean) => page.evaluate((o: boolean) => ((document.querySelector("[data-diag]") as HTMLDetailsElement).open = o), open);
const codec = () => ev<any>("window.tryReactos.soundHandle.codec()");
const frames = () => ev<number>("window.tryReactos.soundHandle.stats().counters.dmaFrames");
const row = (title: string) => page.evaluate((t: string) => [...document.querySelectorAll("[data-kit-diag] dt")].find((d) => d.textContent === t)?.nextElementSibling?.textContent ?? "", title);
const shot = async (name: string) => {
	const frame = await grab(page);
	if (frame) save(outDir, `${browserName}-${name}.png`, frame.png);
};
const shotPage = async (name: string, selector: string) => save(outDir, `${browserName}-${name}.png`, await page.locator(selector).screenshot());
const click = async (selector: string) => {
	await page.evaluate(() => document.exitPointerLock && document.exitPointerLock());
	await sleep(150);
	await page.click(selector);
};

try {
	const started = await startStagedDesktop(page, pageUrl("", {}, { plain: true }));
	report.desktopMs = started.ms;
	// Before the helper, panel closed: the glue exists but runs nothing.
	expect((await kitTimers()).length === 0, "panel closed, helper not started: the kit glue has no timer");
	await insertCd(page, cdUrl);
	await sleep(6000);
	await run("D:\\HOSTLINK.EXE");
	expect((await waitFor(page, "window.tryReactos.hostLink && window.tryReactos.hostLink.stats().state === 'ready'", 90000, 100)) !== null, "the helper from the CD said hello");
	const peerCaps: string[] = await ev("window.tryReactos.hostLink.stats().peer.caps");
	const mods: string[] = await ev("window.tryReactos.hostLink.stats().peer.mods");
	report.caps = peerCaps;
	report.mods = mods;
	expect(["audio", "audio-set", "audio-play", "power", "power-shutdown", "sys", "log"].every((c) => peerCaps.includes(c)), `the helper's caps: ${peerCaps.join(" ")} (modules ${mods.join(" ")})`);
	// Wait for the page's own caps to have been sent (the card is attached a tick after the link).
	await sleep(1500);
	expect((await kitTimers()).length === 0, "panel closed, helper up: the kit glue still has no timer");
	const sysGets = () => ev<number>("window.__sysGets");
	await page.evaluate(() => {
		const w = window as any;
		w.__sysGets = 0;
		w.__hellos = 0;
		w.__lastHello = "";
		const host = w.tryReactos.hostLink;
		const send = host.send.bind(host);
		host.send = (channel: string, op: string, ...rest: any[]) => {
			if (channel === "sys" && op === "get") w.__sysGets++;
			return send(channel, op, ...rest);
		};
		const emulator = w.tryReactos.emulator;
		const serial = emulator.serial_send_bytes.bind(emulator);
		let text = "";
		emulator.serial_send_bytes = (n: number, bytes: Uint8Array) => {
			text += new TextDecoder().decode(bytes);
			for (let at = text.indexOf("hello"); at >= 0; at = text.indexOf("hello")) {
				const end = text.indexOf("\n", at);
				if (end < 0) break;
				w.__hellos++;
				w.__lastHello = text.slice(at, end);
				text = text.slice(end + 1);
			}
			if (text.length > 4000) text = "";
			return serial(n, bytes);
		};
	});

	// System row: not asked while the panel is closed, asked about once a second while it is open, and not again after it is closed.
	await sleep(3000);
	expect((await sysGets()) === 0, `panel closed: no sys get in 3 s (${await sysGets()})`);
	await setPanel(true);
	await sleep(4500);
	const asked = await sysGets();
	expect(asked >= 3 && asked <= 6, `panel open: ${asked} sys get in 4.5 s`);
	const timers = await kitTimers();
	expect(timers.length === 1 && timers[0] === 1000, `panel open: the kit's timers are [${timers.join(", ")}] ms`);
	const system = await row("System");
	report.system = system;
	expect(/Windows|ReactOS/i.test(system) && /memory/.test(system) && /up /.test(system), `the System row reads: ${system}`);
	const pcSound = await row("PC sound");
	report.pcSound = pcSound;
	expect(/out/.test(pcSound) && /mixer|Volume/.test(pcSound), `the PC sound line reads: ${pcSound}`);
	await shotPage("01-diagnostics", "[data-diag]");
	await setPanel(false);
	await sleep(1500);
	const after = await sysGets();
	await sleep(2500);
	expect((await sysGets()) === after && (await kitTimers()).length === 0, "panel closed again: no more sys get, no kit timer");
	await setPanel(true);
	await sleep(1500);

	// The test sound, counted at the card.
	const before = await frames();
	await click("[data-kit-diag] button:has-text('Play a test sound')");
	const heard = await waitFor(page, `window.tryReactos.soundHandle.stats().counters.dmaFrames > ${before} + 10000`, 30000, 100);
	expect(heard !== null, "Play a test sound: the card took frames");
	await waitFor(page, "/counted/.test(document.querySelector('[data-kit-diag]').textContent)", 30000, 200);
	const note = await ev<string>("document.querySelector('[data-kit-diag]').textContent");
	const counted = /card counted ([\d,]+) frames/.exec(note);
	report.testTone = counted?.[1] ?? null;
	expect(Boolean(counted) && Number(counted![1].replace(/,/g, "")) > 15000, `the tone raised dmaFrames: ${counted?.[1]} frames`);

	// The badge: master muted by `audio set mastermute=1`, a sound played, the badge a button, a click unmutes.
	await page.evaluate(() => window.tryReactos.hostLink.send("audio", "set", [["mastermute", "1"]]));
	expect((await waitFor(page, "window.tryReactos.soundHandle.codec().master.muted", 15000, 100)) !== null, "audio set mastermute=1 muted the guest's master");
	await sleep(1000);
	await click("[data-kit-diag] button:has-text('Play a test sound')");
	await waitFor(page, "!document.querySelector('[data-sound-badge]').hidden", 15000, 100);
	const badge = await page.evaluate(() => {
		const b = document.querySelector("[data-sound-badge]") as HTMLElement;
		return { hidden: b.hidden, role: b.getAttribute("role"), text: b.textContent };
	});
	expect(!badge.hidden && badge.role === "button", `the badge is up as a button: ${JSON.stringify(badge)}`);
	await shotPage("02-badge", "[data-player]");
	await page.evaluate(() => (document.querySelector("[data-sound-badge]") as HTMLElement).click());
	expect((await waitFor(page, "!window.tryReactos.soundHandle.codec().master.muted", 15000, 100)) !== null, "the badge button unmuted the guest's master");
	await waitFor(page, "!/counted/.test('') && !window.tryReactos.kitLink.state().test", 40000, 200);

	// Keep in step: on, the slider's level goes to the guest and the speaker's gain is held at 1; the guest's own change moves the slider.
	await sleep(1500);
	const stepBox = page.locator("[data-kit-diag] label", { hasText: "Keep the PC's volume" }).locator("input");
	await stepBox.check();
	await sleep(800);
	await page.evaluate(() => {
		const s = document.querySelector("[data-volume]") as HTMLInputElement;
		s.value = "50";
		s.dispatchEvent(new Event("input", { bubbles: true }));
	});
	await sleep(1500);
	const level = await ev<number>("Math.round(window.tryReactos.soundHandle.codec().master.left)");
	const speaker = await ev<number>("window.tryReactos.soundHandle.stats().volume");
	expect(Math.abs(level - 74) <= 4 && speaker === 1, `Keep in step on: slider 50 sets the guest's master to ${level} percent (74 expected), the speaker's gain is ${speaker}`);
	expect((await kitTimers()).includes(250), "Keep in step on: the kit watches the card at 250 ms");
	await page.evaluate(() => window.tryReactos.soundHandle.model.namWrite(0x02, 2, 0x0000));
	const followed = await waitFor(page, "Number(document.querySelector('[data-volume]').value) >= 95", 6000, 100);
	expect(followed !== null, "the guest's own volume change moved the slider");
	await stepBox.uncheck();
	await sleep(800);
	const speakerBack = await ev<number>("window.tryReactos.soundHandle.stats().volume");
	expect(speakerBack < 1 + 1e-9 && (await kitTimers()).every((ms) => ms !== 250), `Keep in step off: the speaker's gain is back to ${speakerBack}, no step timer`);
	await setPanel(true);

	// A switch off sends a new hello without that channel's tokens.
	const hellos0 = await ev<number>("window.__hellos");
	await page.locator("[data-kit-diag] .kit-switches label", { hasText: "Power" }).locator("input").uncheck();
	await sleep(1500);
	const hellos1 = await ev<number>("window.__hellos");
	const lastHello = await ev<string>("window.__lastHello");
	expect(hellos1 === hellos0 + 1 && !/power/.test(lastHello), `Power switched off: ${hellos1 - hellos0} new hello, ${lastHello.replace(/\s+/g, " ").slice(0, 100)}`);
	expect(await page.locator("[data-kit-bar] button", { hasText: "Shut down the PC" }).isDisabled(), "Power off: Shut down the PC is disabled");
	await page.locator("[data-kit-diag] .kit-switches label", { hasText: "Power" }).locator("input").check();
	await sleep(1500);
	expect(!(await page.locator("[data-kit-bar] button", { hasText: "Shut down the PC" }).isDisabled()), "Power on again: Shut down the PC is enabled");

	// The kit CD: fetched from this origin only on the click, then the drive holds the kit.
	const isoRequestsBefore = requests.filter((r) => r.url.includes("hostlink-kit.iso")).length;
	await click("[data-kit-bar] button:has-text('Insert the Host Link Kit CD')");
	await waitFor(page, "/Host Link Kit CD is in/.test(document.querySelector('[data-kit-bar]').textContent)", 20000, 100);
	const isoRequests = requests.filter((r) => r.url.includes("hostlink-kit.iso"));
	expect(isoRequestsBefore === 0 && isoRequests.length === 1 && isoRequests[0]!.url.startsWith(BASE), `the kit CD was fetched once, on the click, from this origin (${isoRequests.length} request)`);
	await sleep(6000);
	await page.click("[data-stage]");
	await run("cmd /k dir D:\\WIN32");
	await sleep(7000);
	await shot("03-kit-cd-dir");

	// Shut down the PC: the confirmation, then the guest's ACPI write.
	await page.evaluate(() => {
		const w = window as any;
		w.__acpi = [];
		const ports = w.tryReactos.emulator.v86.cpu.io.ports;
		for (const [width, name] of [[0xb004, "write16"], [0xb004, "write32"]] as const) {
			const slot = ports[width];
			const original = slot[name];
			slot[name] = function (value: number) {
				w.__acpi.push(value);
				return original.apply(this, arguments);
			};
		}
	});
	await setPanel(false);
	await click("[data-kit-bar] button:has-text('Shut down the PC')");
	expect(await page.locator("[data-kit-bar] .kit-confirm").isVisible(), "Shut down the PC asks to confirm");
	await shotPage("04-shutdown-confirm", "[data-kit-bar]");
	await click("[data-kit-bar] .kit-confirm button:has-text('Shut down')");
	const off = await waitFor(page, "window.tryReactos.powerState() === 'shutdown'", 120000, 250);
	const acpi = await ev<number[]>("window.__acpi");
	report.shutdownMs = off;
	report.acpiWrites = acpi.map((v) => v.toString(16));
	expect(off !== null && acpi.some((v) => (v & 0x2000) !== 0), `Shut down ended in the ACPI write (${acpi.map((v) => "0x" + v.toString(16)).join(", ")}) after ${off} ms`);
	await shotPage("05-shut-down", "[data-player]");

	const outside = offOrigin(requests.map((r) => r.url));
	report.offOrigin = outside;
	expect(outside.length === 0, `no request left the page's origin (${requests.length} requests)`);
} catch (error) {
	failures.push(String(error));
	console.error(error);
} finally {
	save(outDir, `${browserName}-report.json`, JSON.stringify({ report, failures }, null, 2));
	await browser.close();
}
console.log(`${browserName}: ${failures.length === 0 ? "all checks passed" : failures.length + " failed"}`);
process.exit(failures.length === 0 ? 0 : 1);
