// Diagonal pointer motion, from the guest's pixels: does a diagonal move stay diagonal, on the bus, through the page's lock handler
// (with moves as the browser reports them and as Firefox on Wayland reports them, each cut to whole device pixels), and in follow mode
// (real pointer events, absolute positions)? Prints sent against measured per axis, and the packets per animation frame.
// Usage: BASE=http://127.0.0.1:8093 node mouse-axis.ts [--label NAME] [--param k=v ...] [--check]
// --check: exit 1 unless follow mode lands within 3 guest pixels on both axes and the lock handler sends at most 2 packets a frame.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { PROFILES, launch, pageUrl, sleep } from "./lib.ts";

const here = path.dirname(fileURLToPath(import.meta.url));
const args = process.argv.slice(2);
const opt = (k: string, d: string) => (args.includes("--" + k) ? args[args.indexOf("--" + k) + 1] : d);
const label = opt("label", "axis");
const params: Record<string, string> = { state: "/images/state.l22.bin.zst", disk: "/images/parts/.img.zst", chunk: "1048576", ram: "256" };
args.forEach((a, i) => {
	if (a === "--param") {
		const eq = args[i + 1].indexOf("=");
		params[args[i + 1].slice(0, eq)] = args[i + 1].slice(eq + 1);
	}
});

// The page-side helpers: frames of the guest screen above the taskbar, a park in the top-right corner (away from the desktop icons),
// and the guest pointer's hotspot found from the box of what differs from a reference frame (outside the corner the pointer was
// parked in); the Afterglow arrow's box starts 5 pixels left of and 9 pixels above its hotspot.
function helpers() {
	const T = (window as any).tryReactos;
	const c = T.canvas();
	const g = c.getContext("2d", { willReadFrequently: true });
	const W = c.width;
	const H = c.height - 40;
	const s = (ms: number) => new Promise((r) => setTimeout(r, ms));
	(window as any).__ax = {
		W,
		frame: () => g.getImageData(0, 0, W, H).data,
		parkTopRight: async () => {
			for (let i = 0; i < 14; i++) {
				T.input.mouseDelta(100, -100);
				await s(15);
			}
			await s(700);
			// The reference frame is taken here; then the pointer comes out of the corner, fully into view, to (923, 100).
			const f0 = g.getImageData(0, 0, W, H).data;
			T.input.mouseDelta(-100, 100);
			await s(300);
			return f0;
		},
		find: (a: Uint8ClampedArray, b: Uint8ClampedArray, skip: number[]) => {
			let x0 = 1e9, y0 = 1e9;
			for (let y = 0; y < H; y++)
				for (let x = 0; x < W; x++) {
					if (x >= skip[0] && x <= skip[2] && y >= skip[1] && y <= skip[3]) continue;
					const i = (y * W + x) * 4;
					if (a[i] !== b[i] || a[i + 1] !== b[i + 1] || a[i + 2] !== b[i + 2]) {
						if (x < x0) x0 = x;
						if (y < y0) y0 = y;
					}
				}
			return x0 > W ? null : [x0 + 5, y0 + 9];
		},
	};
}

// A diagonal hand movement at 1000 Hz: per event, (dx, dy) logical pixels. A browser that keeps the position reports whole pixels
// of it, so nothing is lost over a sweep (exact). Firefox on Wayland reports int(d * scale) device pixels per event, each on its
// own, and drops an event whose both axes are 0 (widget/gtk/nsWindowWayland.cpp).
function trace(n: number, dx: number, dy: number, firefoxScale: number) {
	const out: [number, number][] = [];
	let ax = 0, ay = 0, rx = 0, ry = 0;
	for (let i = 0; i < n; i++) {
		if (!firefoxScale) {
			ax += dx;
			ay += dy;
			const mx = Math.round(ax) - rx, my = Math.round(ay) - ry;
			rx += mx;
			ry += my;
			if (mx || my) out.push([mx, my]);
		} else {
			const mx = Math.trunc(dx * firefoxScale);
			const my = Math.trunc(dy * firefoxScale);
			if (mx || my) out.push([mx, my]);
		}
	}
	return out;
}

const out: any = { label, params, bus: [], lock: [], follow: [] };
let failed = false;

async function session(extra: Record<string, string>, body: (s: any) => Promise<void>) {
	const run = `${label}-${Date.now().toString(36)}`;
	const s = await launch(run, PROFILES.unthrottled, false);
	try {
		await s.page.goto(pageUrl(run, { ...params, ...extra }));
		await s.page.click("[data-start]");
		await s.page.waitForFunction("window.__m.taskbarScore() > 0.15", null, { timeout: 300000 });
		await sleep(4000);
		await s.page.evaluate(helpers);
		await body(s);
	} finally {
		await s.close();
	}
}

// Bus and lock handler, in one session with ?mouse=lock.
await session({ mouse: "lock" }, async (s) => {
	const ev = (f: any, a?: any) => s.page.evaluate(f, a);
	for (const [name, dx, dy, n] of [["H", -4, 0, 100], ["V", 0, 4, 100], ["D", -4, 4, 100], ["D", -2, 1, 150], ["D", -1, 1, 200]] as [string, number, number, number][]) {
		const r = await ev(async ([dx, dy, n]: number[]) => {
			const A = (window as any).__ax, T = (window as any).tryReactos;
			const f0 = await A.parkTopRight();
			for (let k = 0; k < n; k++) {
				T.input.mouseDelta(dx, dy);
				await new Promise((r) => setTimeout(r, 8));
			}
			await new Promise((r) => setTimeout(r, 1200));
			return A.find(f0, A.frame(), [A.W - 40, 0, A.W, 40]);
		}, [dx, dy, n]);
		out.bus.push({ name: `${name} (${dx},${dy})x${n}`, sent: [dx * n, dy * n], measured: r ? [r[0] - 923, r[1] - 100] : null });
	}
	const box = await s.page.locator("[data-stage]").boundingBox();
	await s.page.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
	await sleep(1500);
	out.locked = await ev(() => !!document.pointerLockElement);
	const k = await ev(() => {
		const c = (window as any).tryReactos.canvas();
		const r = c.getBoundingClientRect();
		return ((window as any).tryReactos.pointer.speed * c.width) / Math.min(r.width, (r.height * c.width) / c.height);
	});
	// A diagonal of about 27 degrees at 1000 Hz: slow (0.8 and 0.4 logical pixels a millisecond, a low-DPI mouse) and
	// faster (1.6 and 0.8), 300 ms, as a browser that keeps the position reports it and as Firefox on Wayland at scale 1.5 does.
	for (const [name, dx, dy, ffScale] of [["slow, exact", -0.8, 0.4, 0], ["slow, Firefox Wayland 1.5", -0.8, 0.4, 1.5], ["fast, exact", -1.6, 0.8, 0], ["fast, Firefox Wayland 1.5", -1.6, 0.8, 1.5]] as [string, number, number, number][]) {
		const moves = trace(300, dx, dy, ffScale);
		const r = await ev(async (moves: [number, number][]) => {
			const A = (window as any).__ax, T = (window as any).tryReactos;
			const f0 = await A.parkTopRight();
			const p0 = T.pointer.packets;
			let frames = 0, busy = 0;
			const counting = { on: true };
			const tick = () => { if (!counting.on) return; frames++; requestAnimationFrame(tick); };
			requestAnimationFrame(tick);
			// 1000 Hz, delivered 16 to a task (about one display frame), as the browser hands coalesced moves over.
			for (let i = 0; i < moves.length; i += 16) {
				for (const [mx, my] of moves.slice(i, i + 16)) document.dispatchEvent(new PointerEvent("pointermove", { movementX: mx, movementY: my, bubbles: true }));
				busy++;
				await new Promise((r) => setTimeout(r, 16));
			}
			await new Promise((r) => setTimeout(r, 1200));
			counting.on = false;
			return { at: A.find(f0, A.frame(), [A.W - 40, 0, A.W, 40]), packets: T.pointer.packets - p0, tasks: busy, frames };
		}, moves);
		const css = moves.reduce((a, m) => [a[0] + m[0], a[1] + m[1]], [0, 0]);
		out.lock.push({ name, eventsDelivered: moves.length, cssReported: css, expectedGuest: [Math.round(css[0] * k), Math.round(css[1] * k)], measured: r.at ? [r.at[0] - 923, r.at[1] - 100] : null, packetsPerTask: +(r.packets / r.tasks).toFixed(2) });
		if (r.packets / r.tasks > 2) failed = true;
	}
});

// Follow mode: real pointer events (CDP), fractional positions, a sweep with stops along the way.
await session({ mouse: "follow" }, async (s) => {
	const ev = (f: any, a?: any) => s.page.evaluate(f, a);
	const geo = await ev(() => {
		const c = (window as any).tryReactos.canvas();
		const r = c.getBoundingClientRect();
		const w = Math.min(r.width, (r.height * c.width) / c.height);
		const h = (w * c.height) / c.width;
		return { left: r.left + (r.width - w) / 2, top: r.top + (r.height - h) / 2, k: w / c.width };
	});
	const at = (gx: number, gy: number) => [geo.left + gx * geo.k, geo.top + gy * geo.k];
	const [sx, sy] = at(900, 120);
	await s.page.mouse.move(sx, sy);
	await s.page.mouse.click(sx, sy);
	await sleep(1500);
	out.followMode = await ev(() => ({ mode: (window as any).tryReactos.pointer.mode, following: (window as any).tryReactos.pointer.following }));
	await ev(async () => {
		const A = (window as any).__ax, T = (window as any).tryReactos;
		// A reference frame with the guest pointer out of the way, in the top-left corner (excluded from the search).
		for (let i = 0; i < 12; i++) T.input.mouseDelta(-100, -100);
		await new Promise((r) => setTimeout(r, 900));
		(window as any).__f0 = A.frame();
		// The page's dead reckoning, told where the corner put the guest pointer.
		T.pointer.x = 0;
		T.pointer.y = 0;
	});
	let from = [900, 120];
	for (const [tx, ty, steps] of [[700, 220, 400], [500, 320, 400], [500, 420, 200], [300, 420, 200], [301, 421, 50]] as number[][]) {
		for (let i = 1; i <= steps; i++) {
			const gx = from[0] + ((tx - from[0]) * i) / steps;
			const gy = from[1] + ((ty - from[1]) * i) / steps;
			const [cx, cy] = at(gx, gy);
			await s.page.mouse.move(cx, cy);
		}
		await sleep(1200);
		const r = await ev(() => (window as any).__ax.find((window as any).__f0, (window as any).__ax.frame(), [0, 0, 40, 40]));
		const err = r ? [r[0] - tx, r[1] - ty] : null;
		out.follow.push({ to: [tx, ty], from, measured: r, error: err });
		if (!err || Math.abs(err[0]) > 3 || Math.abs(err[1]) > 3) failed = true;
		from = [tx, ty];
	}
});

console.log(JSON.stringify(out, null, 1));
fs.mkdirSync(path.join(here, "..", "out", "mouse"), { recursive: true });
fs.writeFileSync(path.join(here, "..", "out", "mouse", `${label}.json`), JSON.stringify(out, null, 1));
if (args.includes("--check") && failed) {
	console.error("mouse-axis: check failed");
	process.exit(1);
}
