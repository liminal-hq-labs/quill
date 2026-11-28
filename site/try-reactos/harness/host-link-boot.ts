// Cold-boots the host link's test disk (harness/host-link-disk.sh) in headless Chromium, waits for the desktop and for the helper's hello,
// lets the disk go quiet and saves the machine state as NAME.bin in the images folder. check-host-link.ts restores that state, so the
// expensive boot happens once. Input only through the page's own emulator API; pixels only from the emulator canvas.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// usage: BASE=http://127.0.0.1:18391 bun host-link-boot.ts [--disk /images/hl-e2e.img] [--name hl-state] [--shots DIR]      (the server: PORT=18391 IMAGES_DIR=... bun serve.ts)
import path from "node:path";
import { BASE, PROFILES, grab, launch, pageUrl, save, sleep, waitFor } from "./lib.ts";

const args = process.argv.slice(2);
const opt = (key: string, fallback: string) => (args.includes("--" + key) ? args[args.indexOf("--" + key) + 1] : fallback);
const disk = opt("disk", "/images/hl-e2e.img");
const name = opt("name", "hl-state");
const shots = opt("shots", path.join(process.env.HOME ?? ".", ".cache/quill-host-3/out"));
const run = `hl-boot-${Date.now().toString(36)}`;
const session = await launch(run, PROFILES.unthrottled, false);
const ev = <T>(expression: string): Promise<T> => session.page.evaluate(expression) as Promise<T>;
try {
	await session.page.goto(pageUrl(run, { disk, boot: "hd", ram: "256", statename: name }));
	await session.page.click("[data-start]");
	// The page's hello is held back until the guest has said something. v86's serial port keeps what the page sends while the guest's driver is
	// still starting, and a byte waiting when the driver enables its interrupts leaves the line stuck high (docs/host-link.md, "Before the
	// helper is up"), so nothing may be queued before the helper's own hello. The link is stopped before its first retry and started again
	// when the first byte comes from the guest.
	await waitFor(session.page, "!!window.tryReactos.hostLink && !!window.tryReactos.emulator", 60000, 10);
	await session.page.evaluate(() => {
		const host = window.tryReactos.hostLink;
		host.link.stop();
		(window as any).__guestBytes = 0;
		window.tryReactos.emulator.add_listener("serial0-output-byte", () => (window as any).__guestBytes++);
	});
	const started = Date.now();
	const desktop = await waitFor(session.page, "window.__m.taskbarScore() > 0.3", 900000, 2000);
	console.log(JSON.stringify({ desktopMs: desktop }));
	if (desktop === null) throw new Error("the desktop never came up");
	const heard = await waitFor(session.page, "window.__guestBytes > 0", 240000, 500);
	console.log(JSON.stringify({ firstGuestByteMs: heard, since: Date.now() - started }));
	if (heard === null) throw new Error("the helper never spoke");
	await sleep(1000);
	await session.page.evaluate(() => window.tryReactos.hostLink.restart());
	const hello = await waitFor(session.page, "window.tryReactos.hostLink.stats().state === 'ready' && window.tryReactos.hostLink.stats().peer !== null", 30000, 100);
	console.log(JSON.stringify({ helperHelloMs: hello, since: Date.now() - started }));
	const frame = await grab(session.page);
	if (frame) save(shots, "boot-desktop.png", frame.png);
	if (hello === null) throw new Error("the helper never said hello");
	// Let the disk go quiet and the shell finish its start-up work before the state is saved.
	let last = await ev<number>("window.tryReactos.stats.ideReadBytes");
	let since = Date.now();
	while (Date.now() - since < 10000) {
		await sleep(500);
		const n = await ev<number>("window.tryReactos.stats.ideReadBytes");
		if (n !== last) {
			last = n;
			since = Date.now();
		}
	}
	await sleep(10000);
	const uart = await session.page.evaluate(() => {
		const u = window.tryReactos.emulator.v86.cpu.devices.uart0;
		return { ints: u.ints, queued: u.input.length, lsr: u.lsr, iir: u.iir, ier: u.ier };
	});
	console.log(JSON.stringify({ uartBeforeSave: uart }));
	if (uart.queued !== 0 || uart.ints !== 0) throw new Error("the serial port is not idle: a state saved now would restore a stuck line");
	await session.cdp.send("Network.disable");
	const info = await session.page.evaluate(async (base: string) => {
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
