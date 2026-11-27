// The page side of WP-HOST-1's serial measurements: boots a disk in v86 (the same options the product page uses), collects COM1 (serial 0) and
// COM2 (serial 1) output, and offers timed transfers to the Playwright driver in serial-rate.mjs.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { V86 } from "/try-reactos/v86/libv86.mjs";

const params = new URLSearchParams(location.search);
const MB = 1024 * 1024;
const pattern = (index) => ((index * 31 + 7) & 0xff);

const spike = (window.spike = {
	emulator: null,
	com2: "", // everything the guest wrote to COM2: the probe's live log
	com1: { count: 0, bad: 0, firstBad: -1, first: 0, last: 0 }, // what the guest wrote to COM1, checked against the pattern
	events: [], // screen-set-size events: [ms, width, height]
	t0: performance.now(),
});

const opts = {
	wasm_path: "/try-reactos/v86/v86.wasm",
	memory_size: 256 * MB,
	vga_memory_size: 8 * MB,
	screen: { container: document.getElementById("screen") },
	bios: { url: "/try-reactos/v86/bios/seabios.bin" },
	vga_bios: { url: "/try-reactos/v86/bios/vgabios.bin" },
	acpi: true,
	autostart: true,
	disable_speaker: true,
	uart1: true, // COM2 carries the probe's live log; the product page has COM1 only
	hda: { url: params.get("disk") || "/images/hl-v86.img", async: true, size: Number(params.get("size")) },
};
if (params.get("state")) opts.initial_state = { url: params.get("state") };
const emulator = (spike.emulator = new V86(opts));

emulator.add_listener("serial1-output-byte", (byte) => {
	spike.com2 += String.fromCharCode(byte);
});
emulator.add_listener("serial0-output-byte", (byte) => {
	const c = spike.com1;
	const now = performance.now();
	if (c.count === 0) c.first = now;
	c.last = now;
	if (byte !== pattern(c.count) && spike.checkPattern) {
		c.bad++;
		if (c.firstBad < 0) c.firstBad = c.count;
	}
	c.count++;
	if (spike.onCom1) spike.onCom1(byte);
});
emulator.add_listener("screen-set-size", (size) => spike.events.push([Math.round(performance.now() - spike.t0), size[0], size[1]]));

spike.checkPattern = true;
spike.resetCom1 = () => {
	spike.com1 = { count: 0, bad: 0, firstBad: -1, first: 0, last: 0 };
	spike.onCom1 = null;
};
const sleep = (ms) => new Promise((ok) => setTimeout(ok, ms));
spike.sleep = sleep;

// Scancodes for the Run box: Win+R, then the text one character at a time (the box's auto-complete drops keys typed too fast).
spike.run = async (text, pause = 250) => {
	emulator.keyboard_send_scancodes([0xe0, 0x5b, 0x13, 0x93, 0xe0, 0xdb]);
	await sleep(2500);
	for (const ch of text) {
		emulator.keyboard_send_text(ch);
		await sleep(pause);
	}
	await sleep(300);
	emulator.keyboard_send_scancodes([0x1c, 0x9c]);
};
spike.key = (codes) => emulator.keyboard_send_scancodes(codes);

spike.waitFor = async (regex, timeoutMs, from = 0) => {
	const end = performance.now() + timeoutMs;
	while (performance.now() < end) {
		const m = spike.com2.slice(from).match(regex);
		if (m) return m;
		await sleep(100);
	}
	return null;
};

// Guest to page: the guest writes the pattern to COM1; returns bytes, mismatches and the time between the first and last byte.
spike.sourceTest = async (count, idleMs = 4000, maxMs = 300000) => {
	// The caller resets the counters (spike.resetCom1) before it starts the guest, so no early byte is missed.
	const start = performance.now();
	while (performance.now() - start < maxMs) {
		await sleep(250);
		const c = spike.com1;
		if (c.count >= count) break;
		if (c.count > 0 && performance.now() - c.last > idleMs) break;
	}
	const c = spike.com1;
	return { bytes: c.count, bad: c.bad, firstBad: c.firstBad, ms: Math.round(c.last - c.first), bytesPerSecond: Math.round((c.count * 1000) / Math.max(1, c.last - c.first)) };
};

const chunk = (from, length) => {
	const bytes = new Uint8Array(length);
	for (let i = 0; i < length; i++) bytes[i] = pattern(from + i);
	return bytes;
};

// Page to guest with the link's flow control: no more than `window` bytes outstanding (sent minus echoed), sending `credit` bytes at a time as
// the echo comes back. The guest probe echoes every byte it reads (`c e`), so the echo is the credit and also the guest-to-page direction.
spike.echoTest = async (count, windowBytes = 16384, credit = 4096, kickAfterMs = 0, maxMs = 400000) => {
	let kicks = 0;
	spike.resetCom1();
	spike.checkPattern = true;
	let sent = 0;
	const start = performance.now();
	let lastProgress = start;
	let lastCount = 0;
	const stalls = [];
	while (spike.com1.count < count && performance.now() - start < maxMs) {
		const outstanding = sent - spike.com1.count;
		if (sent < count && outstanding + credit <= windowBytes) {
			const n = Math.min(credit, count - sent);
			emulator.serial_send_bytes(0, chunk(sent, n));
			sent += n;
		} else {
			await sleep(2);
		}
		if (spike.com1.count !== lastCount) {
			lastCount = spike.com1.count;
			lastProgress = performance.now();
		} else if (kickAfterMs && performance.now() - lastProgress > kickAfterMs && sent - spike.com1.count > 0) {
			spike.kick();
			kicks++;
			lastProgress = performance.now() - kickAfterMs + 150;
			if (kicks > 200) break;
		} else if (performance.now() - lastProgress > 8000) {
			stalls.push({ sent, echoed: spike.com1.count, atMs: Math.round(performance.now() - start) });
			break;
		}
	}
	const c = spike.com1;
	const ms = Math.round(performance.now() - start);
	return { sent, echoed: c.count, bad: c.bad, firstBad: c.firstBad, ms, bytesPerSecond: Math.round((c.count * 1000) / Math.max(1, ms)), windowBytes, credit, kicks, stalls };
};

// Page to guest with no flow control at all, into a sink (`c s`): everything is queued in one call, or `chunkBytes` at a time every `everyMs`.
spike.blastTest = async (count, chunkBytes = 0, everyMs = 0) => {
	const start = performance.now();
	if (!chunkBytes) {
		emulator.serial_send_bytes(0, chunk(0, count));
	} else {
		for (let sent = 0; sent < count; sent += chunkBytes) {
			emulator.serial_send_bytes(0, chunk(sent, Math.min(chunkBytes, count - sent)));
			await sleep(everyMs);
		}
	}
	return { queuedMs: Math.round(performance.now() - start), count, chunkBytes, everyMs };
};

spike.screen = () => {
	const canvas = document.querySelector("#screen canvas");
	return { w: canvas?.width, h: canvas?.height, css: canvas ? [canvas.style.width, canvas.style.height] : null };
};
spike.ready = true;

spike.pixel = (x, y) => {
	const canvas = document.querySelector("#screen canvas");
	return Array.from(canvas.getContext("2d").getImageData(x, y, 1, 1).data);
};

// Toggles COM1's CTS input, which raises the UART's modem-status interrupt: the guest's interrupt service routine runs and reads the line's other
// causes too, so it can wake a receiver that stopped taking bytes (the question is whether it does).
let cts = false;
spike.kick = () => {
	cts = !cts;
	emulator.bus.send("serial0-clear-to-send-input", cts);
};

// A sink test with every sending pattern in one place: `chunkBytes` per call (0 means all in one call), `everyMs` apart, and while the guest has
// not taken a byte for `kickAfterMs` a CTS toggle every `kickEveryMs`. The guest's own `c s` summary line is the result; the page reports how
// many bytes were still queued in v86's UART when it gave up waiting.
spike.sinkTest = async (count, chunkBytes, everyMs, kickAfterMs = 0, kickEveryMs = 200, maxMs = 60000) => {
	const queued = () => emulator.v86?.cpu?.devices?.uart0?.input?.length ?? -1;
	const start = performance.now();
	let kicks = 0;
	await spike.blastTest(count, chunkBytes, everyMs);
	let last = queued();
	let lastChange = performance.now();
	while (performance.now() - start < maxMs) {
		await sleep(50);
		const now = queued();
		if (now !== last) {
			last = now;
			lastChange = performance.now();
		}
		if (now === 0) break;
		if (kickAfterMs && performance.now() - lastChange > kickAfterMs) {
			spike.kick();
			kicks++;
			await sleep(kickEveryMs);
		} else if (performance.now() - lastChange > 8000) {
			break;
		}
	}
	return { stillQueued: queued(), kicks, ms: Math.round(performance.now() - start) };
};

// Whole-machine snapshots in memory, so each test starts from the same quiet desktop: the guest, the UART (its pending input is not part of a
// state) and the disk's written blocks all come back.
spike.save = async () => {
	spike.base = await emulator.save_state();
	return spike.base.byteLength;
};
spike.load = async () => {
	await emulator.restore_state(spike.base);
	// A state does not hold the UART's pending input, so what the page queued before the restore would be read by the next probe.
	for (const uart of [emulator.v86.cpu.devices.uart0, emulator.v86.cpu.devices.uart1]) {
		uart.input.length = 0;
		uart.lsr &= ~1;
	}
	spike.com2 = "";
	spike.resetCom1();
	await sleep(1500);
};

// The link's own pattern, against `c a`: the guest answers every 4096 bytes it takes off the port with a 16-byte credit. The page keeps at most
// `windowBytes` outstanding and sends `creditBytes` at a time as credits come back (a credit is 16 bytes of acknowledgement).
spike.ackTest = async (count, windowBytes = 16384, creditBytes = 4096, kickAfterMs = 0, maxMs = 400000, port = 0) => {
	spike.resetCom1();
	spike.checkPattern = false;
	let sent = 0;
	let kicks = 0;
	const start = performance.now();
	let lastProgress = start;
	let lastCredits = 0;
	const stalls = [];
	const credits = () => Math.floor(spike.com1.count / 16);
	while (credits() * 4096 < count && performance.now() - start < maxMs) {
		const outstanding = sent - credits() * 4096;
		if (sent < count && outstanding + creditBytes <= windowBytes) {
			const n = Math.min(creditBytes, count - sent);
			emulator.serial_send_bytes(port, chunk(sent, n));
			sent += n;
		} else {
			await sleep(2);
		}
		if (credits() !== lastCredits) {
			lastCredits = credits();
			lastProgress = performance.now();
		} else if (kickAfterMs && performance.now() - lastProgress > kickAfterMs && sent - credits() * 4096 > 0) {
			spike.kick();
			kicks++;
			lastProgress = performance.now() - kickAfterMs + 150;
			if (kicks > 200) break;
		} else if (performance.now() - lastProgress > 8000) {
			stalls.push({ sent, credits: credits(), atMs: Math.round(performance.now() - start) });
			break;
		}
	}
	const ms = Math.round(performance.now() - start);
	return { sent, creditedBytes: credits() * 4096, ms, bytesPerSecond: Math.round((credits() * 4096 * 1000) / Math.max(1, ms)), windowBytes, creditBytes, kicks, stalls };
};

// What v86's COM1 and its interrupt controller look like when the guest has stopped taking bytes. Reads v86's internals (not part of its public
// API), for diagnosis only.
spike.diagnose = () => {
	const devices = emulator.v86.cpu.devices;
	const u = devices.uart0;
	const pickNumbers = (o) => Object.fromEntries(Object.entries(o).filter(([, v]) => typeof v === "number" || typeof v === "boolean"));
	const pic = devices.pic;
	return {
		uart: { ier: u.ier, iir: u.iir, ints: u.ints, lsr: u.lsr, fcr: u.fifo_control, lcr: u.line_control, mcr: u.modem_control, msr: u.modem_status, inputLength: u.input.length },
		pic: pic ? { own: pickNumbers(pic), master: pic.master ? pickNumbers(pic.master) : null, slave: pic.slave ? pickNumbers(pic.slave) : null } : null,
	};
};
// Gives the guest a fresh rising edge on COM1's interrupt line: clears the receive causes, lets the line fall, then raises it again.
spike.repoke = () => {
	const u = emulator.v86.cpu.devices.uart0;
	u.ints &= ~((1 << 4) | (1 << 12));
	u.CheckInterrupt();
	u.ThrowInterrupt(u.fifo_control & 1 ? 12 : 4);
};
