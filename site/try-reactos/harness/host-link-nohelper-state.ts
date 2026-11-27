// Makes an "older" saved state, one with no helper running, from the state host-link-boot.ts saved: restores it, ends the helper with
// `HOSTLINK.EXE /quit`, checks the serial port is idle and saves the machine as NAME.bin. check-clipboard.ts restores that state to see
// the page say "guest helper not found" and work as before. Input only through the page's emulator API.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// usage: BASE=http://127.0.0.1:18394 bun host-link-nohelper-state.ts [--disk /images/hl-e2e.img] [--state /images/hl-state.bin] [--name hl-nohelper]
import { BASE, PROFILES, launch, pageUrl, sleep, waitFor } from "./lib.ts";

const args = process.argv.slice(2);
const opt = (key: string, fallback: string) => (args.includes("--" + key) ? args[args.indexOf("--" + key) + 1]! : fallback);
const KEY = { enter: [0x1c], win: [0xe0, 0x5b], r: [0x13] };
const session = await launch("hl-nohelper", PROFILES.unthrottled, false);
const page = session.page;
const press = (codes: number[][]) =>
	page.evaluate((keys: number[][]) => {
		for (const code of keys) window.tryReactos.input.key(code);
		for (const code of [...keys].reverse()) window.tryReactos.input.key(code.map((value: number) => value | 0x80));
	}, codes);
try {
	await page.goto(pageUrl("hl-nohelper", { disk: opt("disk", "/images/hl-e2e.img"), boot: "hd", ram: "256", state: opt("state", "/images/hl-state.bin"), statename: opt("name", "hl-nohelper") }));
	await page.click("[data-start]");
	if ((await waitFor(page, "window.tryReactos.hostLink && window.tryReactos.hostLink.stats().state === 'ready'", 120000, 50)) === null) throw new Error("the helper never answered");
	// The page stops talking first: anything it sent from now would wait in the serial port with nobody to read it.
	await page.evaluate(() => window.tryReactos.hostLink.link.stop());
	await sleep(1500);
	await press([KEY.win, KEY.r]);
	await sleep(2500);
	for (const ch of "C:\\Quill\\HOSTLINK.EXE /quit") {
		await page.evaluate((c: string) => window.tryReactos.input.text(c), ch);
		await sleep(110);
	}
	await sleep(400);
	await press([KEY.enter]);
	await sleep(15000);
	const uart = await page.evaluate(() => {
		const u = window.tryReactos.emulator.v86.cpu.devices.uart0;
		return { ints: u.ints, queued: u.input.length };
	});
	console.log(JSON.stringify({ uartBeforeSave: uart }));
	if (uart.queued !== 0 || uart.ints !== 0) throw new Error("the serial port is not idle");
	const info = await page.evaluate(async (base: string) => {
		await window.tryReactos.emulator.stop();
		const st = await window.tryReactos.emulator.save_state();
		const piece = 8 * 1024 * 1024;
		for (let offset = 0; offset < st.byteLength; offset += piece) {
			const r = await fetch(`${base}/__put/${window.tryReactos.stateName}.bin?offset=${offset}`, { method: "POST", body: st.slice(offset, offset + piece) });
			if (!r.ok) throw new Error("put failed at " + offset);
		}
		return { bytes: st.byteLength };
	}, BASE);
	console.log(JSON.stringify(info));
} finally {
	await session.close();
}

declare global {
	interface Window {
		tryReactos: any;
	}
}
